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

// ── SPAWNC Sandbox Tests (Phase 108: Sandboxed AI Execution) ──────────

/// Helper: write a null-terminated ASCII string into RAM starting at addr.
fn write_string(ram: &mut Vec<u32>, addr: usize, s: &str) {
    for (i, ch) in s.chars().enumerate() {
        ram[addr + i] = ch as u32;
    }
    ram[addr + s.len()] = 0;
}

/// Helper: build sandbox capability list in RAM (mimics build_sandbox_caps in ai_terminal.asm).
/// Returns the address of the capability struct.
fn build_sandbox_caps(ram: &mut Vec<u32>) -> usize {
    let caps_addr = 0x7500;
    let strs_addr = 0x7600;

    // Pattern strings
    write_string(ram, strs_addr, "/tmp/*");
    write_string(ram, strs_addr + 16, "/lib/*");

    // Capability struct: [n_entries, entry_0, entry_1, sentinel]
    // Each entry: [resource_type, pattern_addr, pattern_len, permissions]
    ram[caps_addr] = 2; // n_entries

    // Entry 0: /tmp/* with read+write (0x03)
    ram[caps_addr + 1] = 0; // resource_type = VFS path
    ram[caps_addr + 2] = strs_addr as u32; // pattern_addr
    ram[caps_addr + 3] = 6; // pattern_len
    ram[caps_addr + 4] = 0x03; // read + write

    // Entry 1: /lib/* with read (0x01)
    ram[caps_addr + 5] = 0; // resource_type = VFS path
    ram[caps_addr + 6] = (strs_addr + 16) as u32; // pattern_addr
    ram[caps_addr + 7] = 6; // pattern_len
    ram[caps_addr + 8] = 0x01; // read only

    // Sentinel
    ram[caps_addr + 9] = 0xFFFFFFFF;

    caps_addr
}

#[test]
fn test_spawnc_sandbox_creates_child_with_capabilities() {
    // Write a simple program at 0x1000: LDI r5, 42; HALT
    let mut vm = Vm::new();
    vm.ram[0x1000] = 0x10; // LDI
    vm.ram[0x1001] = 5; // r5
    vm.ram[0x1002] = 42; // value
    vm.ram[0x1003] = 0x00; // HALT

    // Build sandbox capabilities
    let caps_addr = build_sandbox_caps(&mut vm.ram);

    // Set up registers: r10 = 0x1000 (start addr), r11 = caps_addr
    vm.regs[10] = 0x1000;
    vm.regs[11] = caps_addr as u32;

    // Execute SPAWNC r10, r11
    vm.ram[0] = 0xA7; // SPAWNC
    vm.ram[1] = 10; // addr_reg
    vm.ram[2] = 11; // caps_reg
    vm.step();

    // Should have created a child process
    assert_eq!(vm.ram[0xFFA], 1, "SPAWNC should return child PID 1");
    assert_eq!(vm.processes.len(), 1, "should have 1 child process");

    // Child should have capabilities set
    let child = &vm.processes[0];
    assert!(
        child.capabilities.is_some(),
        "child should have capabilities"
    );
    let caps = child.capabilities.as_ref().unwrap();
    assert_eq!(caps.len(), 2, "should have 2 capability entries");
    assert_eq!(caps[0].pattern, "/tmp/*");
    assert_eq!(caps[0].permissions, 0x03); // read+write
    assert_eq!(caps[1].pattern, "/lib/*");
    assert_eq!(caps[1].permissions, 0x01); // read only
}

#[test]
fn test_spawnc_sandbox_child_runs_code() {
    let mut vm = Vm::new();

    // Write a program at 0x1000: LDI r5, 99; HALT
    vm.ram[0x1000] = 0x10; // LDI
    vm.ram[0x1001] = 5; // r5
    vm.ram[0x1002] = 99;
    vm.ram[0x1003] = 0x00; // HALT

    let caps_addr = build_sandbox_caps(&mut vm.ram);
    vm.regs[10] = 0x1000;
    vm.regs[11] = caps_addr as u32;

    // SPAWNC
    vm.ram[0] = 0xA7;
    vm.ram[1] = 10;
    vm.ram[2] = 11;
    vm.step();

    assert_eq!(vm.processes.len(), 1);
    let child_pid = vm.processes[0].pid;

    // Run the scheduler to execute the child
    for _ in 0..20 {
        vm.step_all_processes();
    }

    // Child should have r5 = 99 (executed the LDI) and be halted
    let child = vm.processes.iter().find(|p| p.pid == child_pid).unwrap();
    assert_eq!(child.regs[5], 99, "child should have executed LDI r5, 99");
    assert!(child.is_halted(), "child should have halted");
}

#[test]
fn test_spawnc_sandbox_child_has_memory_isolation() {
    // Parent writes a value to r5, spawns child that writes different value to r5.
    // Parent's r5 should be unchanged (COW isolation).
    let mut vm = Vm::new();
    vm.regs[5] = 0xDEADBEEF; // Parent's r5

    // Child program at 0x1000: LDI r5, 0x1234; HALT
    vm.ram[0x1000] = 0x10; // LDI
    vm.ram[0x1001] = 5; // r5
    vm.ram[0x1002] = 0x1234;
    vm.ram[0x1003] = 0x00; // HALT

    let caps_addr = build_sandbox_caps(&mut vm.ram);
    vm.regs[10] = 0x1000;
    vm.regs[11] = caps_addr as u32;

    vm.ram[0] = 0xA7;
    vm.ram[1] = 10;
    vm.ram[2] = 11;
    vm.step(); // SPAWNC creates child

    // Parent's r5 should be unchanged
    assert_eq!(
        vm.regs[5], 0xDEADBEEF,
        "parent r5 should be untouched after SPAWNC"
    );
}

#[test]
fn test_spawnc_sandbox_denies_vfs_path_outside_capabilities() {
    // Spawn a child with sandbox capabilities, then try to OPEN a path
    // that isn't in the capability list. Should get EPERM.
    let mut vm = Vm::new();

    // Child program: write path string, then OPEN it
    // "/secret/data" at 0x2000
    write_string(&mut vm.ram, 0x2000, "/secret/data");

    // OPEN r0=0x54 path_addr=0x2000 mode=0 (read)
    // LDI r1, 0x2000; OPEN r1, 0; HALT
    vm.ram[0x1000] = 0x10; // LDI
    vm.ram[0x1001] = 1; // r1
    vm.ram[0x1002] = 0x2000;
    vm.ram[0x1003] = 0x54; // OPEN
    vm.ram[0x1004] = 1; // path in r1
    vm.ram[0x1005] = 0; // mode = read
    vm.ram[0x1006] = 0x00; // HALT

    let caps_addr = build_sandbox_caps(&mut vm.ram);
    vm.regs[10] = 0x1000;
    vm.regs[11] = caps_addr as u32;

    // SPAWNC
    vm.ram[0] = 0xA7;
    vm.ram[1] = 10;
    vm.ram[2] = 11;
    vm.step();

    assert_eq!(vm.processes.len(), 1);
    let child_pid = vm.processes[0].pid;

    // Run the scheduler to execute the child
    for _ in 0..20 {
        vm.step_all_processes();
    }

    // Child's r0 should be EPERM (0xFFFFFFFE) because /secret/data
    // doesn't match /tmp/* or /lib/*
    let child = vm.processes.iter().find(|p| p.pid == child_pid).unwrap();
    assert_eq!(
        child.regs[0], 0xFFFFFFFE,
        "child should get EPERM when opening /secret/data -- not in sandbox capabilities"
    );
}

#[test]
fn test_spawnc_sandbox_allows_vfs_tmp_write() {
    // Spawn a child with sandbox capabilities, OPEN /tmp/output.txt for write.
    // Should succeed because /tmp/* is in the capability list with read+write.
    let mut vm = Vm::new();

    write_string(&mut vm.ram, 0x2000, "/tmp/output.txt");

    // Child: LDI r1, 0x2000; OPEN r1, 1 (write); HALT
    vm.ram[0x1000] = 0x10; // LDI
    vm.ram[0x1001] = 1; // r1
    vm.ram[0x1002] = 0x2000;
    vm.ram[0x1003] = 0x54; // OPEN
    vm.ram[0x1004] = 1; // path in r1
    vm.ram[0x1005] = 1; // mode = write
    vm.ram[0x1006] = 0x00; // HALT

    let caps_addr = build_sandbox_caps(&mut vm.ram);
    vm.regs[10] = 0x1000;
    vm.regs[11] = caps_addr as u32;

    // SPAWNC
    vm.ram[0] = 0xA7;
    vm.ram[1] = 10;
    vm.ram[2] = 11;
    vm.step();

    // Run child through scheduler (sets current_capabilities properly)
    for _ in 0..20 {
        vm.step_all_processes();
    }

    // Child should have a valid fd (not EPERM)
    if let Some(child) = vm.processes.first() {
        if child.is_halted() {
            assert_ne!(
                child.regs[0], 0xFFFFFFFE,
                "child should NOT get EPERM for /tmp/output.txt"
            );
            assert_ne!(
                child.regs[0], 0xFFFFFFFF,
                "child should get a valid fd for /tmp/output.txt"
            );
        }
    }
}

#[test]
fn test_ai_terminal_build_sandbox_caps_assembles() {
    // Verify the ai_terminal.asm with the new build_sandbox_caps still assembles
    let source = include_str!("../programs/ai_terminal.asm");
    let mut pp = geometry_os::preprocessor::Preprocessor::new();
    let preprocessed = pp.preprocess(source);
    geometry_os::assembler::assemble(&preprocessed, 0)
        .expect("ai_terminal.asm with sandbox caps should assemble");
}
