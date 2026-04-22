// ── Phase 102: Capability System Tests ──────────────────────────────

use geometry_os::vm::Vm;
use geometry_os::vm::*;

// ── Capability struct tests ──────────────────────────────────────

#[test]
fn test_capability_path_match_exact() {
    let cap = Capability {
        resource_type: 0,
        pattern: "/tmp/test.txt".to_string(),
        permissions: Capability::PERM_READ,
    };
    assert!(cap.matches_path("/tmp/test.txt"));
    assert!(!cap.matches_path("/tmp/other.txt"));
}

#[test]
fn test_capability_path_match_glob() {
    let cap = Capability {
        resource_type: 0,
        pattern: "/tmp/*".to_string(),
        permissions: Capability::PERM_READ | Capability::PERM_WRITE,
    };
    assert!(cap.matches_path("/tmp/foo"));
    assert!(cap.matches_path("/tmp/bar.txt"));
    assert!(cap.matches_path("/tmp/subdir/file"));
    assert!(!cap.matches_path("/var/log"));
}

#[test]
fn test_capability_opcode_resource_no_path_match() {
    let cap = Capability {
        resource_type: 1,
        pattern: "82".to_string(),
        permissions: 0,
    };
    assert!(!cap.matches_path("/tmp/test"));
}

#[test]
fn test_capability_allows_permissions() {
    let ro = Capability {
        resource_type: 0,
        pattern: "/tmp/*".to_string(),
        permissions: Capability::PERM_READ,
    };
    assert!(ro.allows(Capability::PERM_READ));
    assert!(!ro.allows(Capability::PERM_WRITE));
}

#[test]
fn test_check_path_capability_none_is_full_access() {
    assert!(check_path_capability(
        &None,
        "/tmp/test",
        Capability::PERM_READ
    ));
    assert!(check_path_capability(
        &None,
        "/secret",
        Capability::PERM_WRITE
    ));
}

#[test]
fn test_check_path_capability_with_caps() {
    let caps = Some(vec![Capability {
        resource_type: 0,
        pattern: "/tmp/*".to_string(),
        permissions: Capability::PERM_READ,
    }]);
    assert!(check_path_capability(
        &caps,
        "/tmp/file.txt",
        Capability::PERM_READ
    ));
    assert!(!check_path_capability(
        &caps,
        "/tmp/file.txt",
        Capability::PERM_WRITE
    ));
    assert!(!check_path_capability(
        &caps,
        "/var/log",
        Capability::PERM_READ
    ));
}

#[test]
fn test_check_path_capability_multiple_caps() {
    let caps = Some(vec![
        Capability {
            resource_type: 0,
            pattern: "/tmp/*".to_string(),
            permissions: Capability::PERM_READ | Capability::PERM_WRITE,
        },
        Capability {
            resource_type: 0,
            pattern: "/lib/fonts/*".to_string(),
            permissions: Capability::PERM_READ,
        },
    ]);
    assert!(check_path_capability(
        &caps,
        "/tmp/art/pic.raw",
        Capability::PERM_WRITE
    ));
    assert!(check_path_capability(
        &caps,
        "/lib/fonts/mono.bdf",
        Capability::PERM_READ
    ));
    assert!(!check_path_capability(
        &caps,
        "/lib/fonts/mono.bdf",
        Capability::PERM_WRITE
    ));
    assert!(!check_path_capability(
        &caps,
        "/bin/shell",
        Capability::PERM_READ
    ));
}

#[test]
fn test_check_opcode_capability_none_allows_all() {
    assert!(check_opcode_capability(&None, 82));
}

#[test]
fn test_check_opcode_capability_restriction() {
    let caps = Some(vec![Capability {
        resource_type: 1,
        pattern: "82".to_string(),
        permissions: 0,
    }]);
    assert!(!check_opcode_capability(&caps, 82));
    assert!(check_opcode_capability(&caps, 77));
}

// ── SPAWNC opcode tests (using manual RAM + process setup) ─────

#[test]
fn test_spawnc_creates_process_with_capabilities() {
    // Verify that a process created with capabilities stores them correctly
    let caps = Some(vec![
        Capability {
            resource_type: 0,
            pattern: "/tmp/*".to_string(),
            permissions: Capability::PERM_READ | Capability::PERM_WRITE,
        },
        Capability {
            resource_type: 0,
            pattern: "/lib/fonts/*".to_string(),
            permissions: Capability::PERM_READ,
        },
    ]);

    let mut vm = Vm::new();
    vm.processes.push(SpawnedProcess {
        pc: 0x100,
        regs: [0; NUM_REGS],
        state: ProcessState::Ready,
        pid: 1,
        mode: CpuMode::User,
        page_dir: None,
        segfaulted: false,
        priority: 1,
        slice_remaining: 0,
        sleep_until: 0,
        yielded: false,
        kernel_stack: Vec::new(),
        msg_queue: Vec::new(),
        exit_code: 0,
        parent_pid: 0,
        pending_signals: Vec::new(),
        signal_handlers: [0; 4],
        vmas: Vec::new(),
        brk_pos: 0,
        custom_font: None,
        capabilities: caps.clone(),
    });

    assert_eq!(vm.processes.len(), 1);
    let child = &vm.processes[0];
    assert!(child.capabilities.is_some());
    let child_caps = child.capabilities.as_ref().unwrap();
    assert_eq!(child_caps.len(), 2);
    assert_eq!(child_caps[0].pattern, "/tmp/*");
    assert_eq!(child_caps[0].permissions, 0x03);
    assert_eq!(child_caps[1].pattern, "/lib/fonts/*");
    assert_eq!(child_caps[1].permissions, 0x01);
}

#[test]
fn test_spawnc_no_capabilities_is_none() {
    let mut vm = Vm::new();
    vm.processes.push(SpawnedProcess {
        pc: 0x100,
        regs: [0; NUM_REGS],
        state: ProcessState::Ready,
        pid: 1,
        mode: CpuMode::User,
        page_dir: None,
        segfaulted: false,
        priority: 1,
        slice_remaining: 0,
        sleep_until: 0,
        yielded: false,
        kernel_stack: Vec::new(),
        msg_queue: Vec::new(),
        exit_code: 0,
        parent_pid: 0,
        pending_signals: Vec::new(),
        signal_handlers: [0; 4],
        vmas: Vec::new(),
        brk_pos: 0,
        custom_font: None,
        capabilities: None,
    });

    assert!(vm.processes[0].capabilities.is_none());
    // No caps = full access
    assert!(check_path_capability(
        &vm.processes[0].capabilities,
        "/anything",
        Capability::PERM_WRITE
    ));
}

#[test]
fn test_sandboxed_paint_capabilities() {
    let caps = Some(vec![
        Capability {
            resource_type: 0,
            pattern: "/tmp/art/*".to_string(),
            permissions: Capability::PERM_READ | Capability::PERM_WRITE,
        },
        Capability {
            resource_type: 0,
            pattern: "/lib/fonts/*".to_string(),
            permissions: Capability::PERM_READ,
        },
    ]);
    assert!(check_path_capability(
        &caps,
        "/tmp/art/canvas.raw",
        Capability::PERM_WRITE
    ));
    assert!(check_path_capability(
        &caps,
        "/lib/fonts/mono.bdf",
        Capability::PERM_READ
    ));
    assert!(!check_path_capability(
        &caps,
        "/lib/fonts/mono.bdf",
        Capability::PERM_WRITE
    ));
    assert!(!check_path_capability(
        &caps,
        "/bin/shell",
        Capability::PERM_READ
    ));
    assert!(!check_path_capability(
        &caps,
        "/tmp/other",
        Capability::PERM_WRITE
    ));
}

#[test]
fn test_spawnc_assembles() {
    let result = geometry_os::assembler::assemble("SPAWNC r10, r11", 0).expect("should assemble");
    assert!(result.pixels.len() >= 3);
    assert_eq!(result.pixels[0], 0xA7);
    assert_eq!(result.pixels[1], 10);
    assert_eq!(result.pixels[2], 11);
}

#[test]
fn test_spawnc_disasm() {
    // Assemble and verify the opcode bytes are correct
    let result = geometry_os::assembler::assemble("SPAWNC r5, r6", 0).expect("should assemble");
    assert_eq!(result.pixels[0], 0xA7);
    assert_eq!(result.pixels[1], 5);
    assert_eq!(result.pixels[2], 6);
}

#[test]
fn test_capability_read_only_denies_write() {
    let caps = Some(vec![Capability {
        resource_type: 0,
        pattern: "/tmp/readonly/*".to_string(),
        permissions: Capability::PERM_READ,
    }]);
    // Can read
    assert!(check_path_capability(
        &caps,
        "/tmp/readonly/doc.txt",
        Capability::PERM_READ
    ));
    // Cannot write
    assert!(!check_path_capability(
        &caps,
        "/tmp/readonly/doc.txt",
        Capability::PERM_WRITE
    ));
}

#[test]
fn test_capability_write_only_denies_read() {
    let caps = Some(vec![Capability {
        resource_type: 0,
        pattern: "/tmp/writeonly/*".to_string(),
        permissions: Capability::PERM_WRITE,
    }]);
    assert!(!check_path_capability(
        &caps,
        "/tmp/writeonly/log.txt",
        Capability::PERM_READ
    ));
    assert!(check_path_capability(
        &caps,
        "/tmp/writeonly/log.txt",
        Capability::PERM_WRITE
    ));
}
