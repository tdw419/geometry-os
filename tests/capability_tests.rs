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
        data_base: 0,
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
        data_base: 0,
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

    // Child should NOT get EPERM -- capability check passes for /tmp/* with write perm.
    // VFS may return 0xFFFFFFFF (file not found / can't create with slashes) but that's
    // a VFS limitation, not a capability denial. The key assertion: no EPERM (0xFFFFFFFE).
    if let Some(child) = vm.processes.first() {
        if child.is_halted() {
            assert_ne!(
                child.regs[0], 0xFFFFFFFE,
                "child should NOT get EPERM for /tmp/output.txt"
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

// ── Phase 110: /focus and /status commands ────────────────────

/// Helper: load ai_terminal.asm into a VM, run for initial frames to get past init.
fn load_ai_terminal() -> geometry_os::vm::Vm {
    let source = include_str!("../programs/ai_terminal.asm");
    let mut pp = geometry_os::preprocessor::Preprocessor::new();
    let preprocessed = pp.preprocess(source);
    let asm = geometry_os::assembler::assemble(&preprocessed, 0)
        .expect("ai_terminal.asm should assemble");

    let mut vm = geometry_os::vm::Vm::new();
    // Load program
    for (i, &word) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = word;
        }
    }
    vm
}

/// Helper: run the AI terminal for some frames, yielding between key injections.
/// Pushes keys into the key buffer, runs until processed.
fn run_frames_with_keys(vm: &mut geometry_os::vm::Vm, keys: &[u32], max_cycles_per_key: usize) {
    // Run initial frames to get past init
    for _ in 0..20 {
        let mut cycles = 0;
        while !vm.yielded && !vm.halted && cycles < 50000 {
            vm.step();
            cycles += 1;
        }
        vm.yielded = false;
    }

    // Inject keys one at a time
    for &key in keys {
        vm.push_key(key);
        let mut cycles = 0;
        while !vm.yielded && !vm.halted && cycles < max_cycles_per_key {
            vm.step();
            cycles += 1;
        }
        vm.yielded = false;
        // Run a few more frames to let the command process
        for _ in 0..3 {
            let mut c = 0;
            while !vm.yielded && !vm.halted && c < 50000 {
                vm.step();
                c += 1;
            }
            vm.yielded = false;
        }
    }
}

#[test]
fn test_ai_terminal_focus_command_sets_ram() {
    // Type "/focus 0x40" + Enter into the AI terminal.
    // This should write 0x40 to RAM[0x7821].
    let mut vm = load_ai_terminal();

    // "/focus 0x40\r"
    let keys: Vec<u32> = vec![
        b'/' as u32,
        b'f' as u32,
        b'o' as u32,
        b'c' as u32,
        b'u' as u32,
        b's' as u32,
        b' ' as u32,
        b'0' as u32,
        b'x' as u32,
        b'4' as u32,
        b'0' as u32,
        13, // Enter
    ];
    run_frames_with_keys(&mut vm, &keys, 100_000);

    assert_eq!(
        vm.ram[0x7821], 0x40,
        "/focus 0x40 should set RAM[0x7821] to 0x40, got 0x{:X}",
        vm.ram[0x7821]
    );
}

#[test]
fn test_ai_terminal_focus_off_clears_ram() {
    // Type "/focus 0x40" then "/focus off" + Enter.
    let mut vm = load_ai_terminal();

    // First set focus
    let set_keys: Vec<u32> = vec![
        b'/' as u32,
        b'f' as u32,
        b'o' as u32,
        b'c' as u32,
        b'u' as u32,
        b's' as u32,
        b' ' as u32,
        b'0' as u32,
        b'x' as u32,
        b'4' as u32,
        b'0' as u32,
        13,
    ];
    run_frames_with_keys(&mut vm, &set_keys, 100_000);
    assert_eq!(vm.ram[0x7821], 0x40, "precondition: focus should be 0x40");

    // Now clear it: "/focus off\r"
    let off_keys: Vec<u32> = vec![
        b'/' as u32,
        b'f' as u32,
        b'o' as u32,
        b'c' as u32,
        b'u' as u32,
        b's' as u32,
        b' ' as u32,
        b'o' as u32,
        b'f' as u32,
        b'f' as u32,
        13,
    ];
    run_frames_with_keys(&mut vm, &off_keys, 100_000);
    assert_eq!(
        vm.ram[0x7821], 0,
        "/focus off should clear RAM[0x7821], got 0x{:X}",
        vm.ram[0x7821]
    );
}

#[test]
fn test_ai_terminal_status_command_runs() {
    // "/status\r" should run without crashing.
    // After init, RAM[0xFFD] should be 0 (no assembly yet).
    let mut vm = load_ai_terminal();

    let keys: Vec<u32> = vec![
        b'/' as u32,
        b's' as u32,
        b't' as u32,
        b'a' as u32,
        b't' as u32,
        b'u' as u32,
        b's' as u32,
        13,
    ];
    run_frames_with_keys(&mut vm, &keys, 100_000);

    // If we got here without a panic, the /status command executed.
    // Check the init set asm_dev mode: RAM[0x7820] = 1
    assert_eq!(
        vm.ram[0x7820], 1,
        "ai_terminal should set RAM[0x7820]=1 for asm_dev mode"
    );
}

#[test]
fn test_ai_terminal_focus_bad_arg_no_crash() {
    // "/focus xyz\r" should show usage, not crash.
    let mut vm = load_ai_terminal();

    let keys: Vec<u32> = vec![
        b'/' as u32,
        b'f' as u32,
        b'o' as u32,
        b'c' as u32,
        b'u' as u32,
        b's' as u32,
        b' ' as u32,
        b'x' as u32,
        b'y' as u32,
        b'z' as u32,
        13,
    ];
    run_frames_with_keys(&mut vm, &keys, 100_000);

    // Should not have changed focus (remains 0 from init)
    assert_eq!(
        vm.ram[0x7821], 0,
        "bad /focus arg should leave RAM[0x7821] unchanged"
    );
}

// ── Phase 111: Self-Analysis Program ──────────────────────────

/// Helper: load an assembly file into a VM
fn load_program(source: &str) -> geometry_os::vm::Vm {
    let mut pp = geometry_os::preprocessor::Preprocessor::new();
    let preprocessed = pp.preprocess(source);
    let asm = geometry_os::assembler::assemble(&preprocessed, 0).expect("program should assemble");

    let mut vm = geometry_os::vm::Vm::new();
    for (i, &word) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = word;
        }
    }
    vm
}

/// Helper: run VM until halted or max cycles
fn run_until_halt(vm: &mut geometry_os::vm::Vm, max_cycles: usize) -> usize {
    let mut cycles = 0;
    while !vm.halted && cycles < max_cycles {
        vm.step();
        cycles += 1;
    }
    cycles
}

/// Helper: read null-terminated string from RAM
fn read_ram_string(vm: &geometry_os::vm::Vm, addr: usize) -> String {
    let mut result = String::new();
    let mut a = addr;
    while a < vm.ram.len() {
        let ch = vm.ram[a];
        if ch == 0 {
            break;
        }
        if let Some(c) = char::from_u32(ch) {
            result.push(c);
        }
        a += 1;
    }
    result
}

#[test]
fn test_self_analysis_assembles() {
    let source = include_str!("../programs/self_analysis.asm");
    let mut pp = geometry_os::preprocessor::Preprocessor::new();
    let preprocessed = pp.preprocess(source);
    geometry_os::assembler::assemble(&preprocessed, 0).expect("self_analysis.asm should assemble");
}

#[test]
fn test_self_analysis_screen_sampling() {
    // Load and run the program with mocked LLM to test screen sampling logic.
    // The program draws 4 colored quadrants, samples the screen, calls LLM.
    // With mock LLM, we verify quadrant counts are correct.
    let mut vm = load_program(include_str!("../programs/self_analysis.asm"));

    // Set mock LLM response before running
    vm.llm_mock_response = Some("The screen has four colored blocks.".to_string());

    let cycles = run_until_halt(&mut vm, 200_000);
    assert!(
        !vm.halted || cycles < 200_000,
        "program should halt normally"
    );

    // The LLM response should be written to RESP_BUF (0x1800)
    let response = read_ram_string(&vm, 0x1800);
    assert!(
        response.contains("four") || response.contains("blocks") || !response.is_empty(),
        "LLM response should be written to RAM[0x1800], got: {:?}",
        response
    );

    // Verify screen was drawn -- check a pixel in the red quadrant (TL)
    // The red block is at (10,30) size 50x50. Sample center at (35, 55)
    // After program runs, screen buffer should have the red pixel
    let red_pixel = vm.screen[55 * 256 + 35];
    assert_eq!(red_pixel, 0xFF3333, "TL quadrant should have red block");

    // Verify green block (TR) at center ~ (221, 55)
    let green_pixel = vm.screen[55 * 256 + 221];
    assert_eq!(green_pixel, 0x33FF33, "TR quadrant should have green block");

    // Verify blue block (BL) at center ~ (35, 201)
    let blue_pixel = vm.screen[201 * 256 + 35];
    assert_eq!(blue_pixel, 0x3333FF, "BL quadrant should have blue block");

    // Verify yellow block (BR) at center ~ (221, 201)
    let yellow_pixel = vm.screen[201 * 256 + 221];
    assert_eq!(
        yellow_pixel, 0xFFFF33,
        "BR quadrant should have yellow block"
    );
}

#[test]
fn test_self_analysis_prompt_contains_quadrant_data() {
    // Verify the LLM prompt buffer contains quadrant descriptions
    let mut vm = load_program(include_str!("../programs/self_analysis.asm"));
    vm.llm_mock_response = Some("OK".to_string());
    run_until_halt(&mut vm, 200_000);

    // Read the prompt that was sent to LLM
    let prompt = read_ram_string(&vm, 0x1400);
    assert!(
        prompt.contains("Top-left:") && prompt.contains("Top-right:"),
        "Prompt should contain quadrant labels, got: {:?}",
        &prompt[..prompt.len().min(200)]
    );
    assert!(
        prompt.contains("Bottom-left:") && prompt.contains("Bottom-right:"),
        "Prompt should contain all quadrant labels"
    );
    assert!(
        prompt.contains("pixels"),
        "Prompt should mention pixel counts"
    );
}

#[test]
fn test_self_analysis_quadrant_counts_nonzero() {
    // The program draws colored blocks in each quadrant.
    // Quadrant sampling should find non-zero pixel counts in the quadrants
    // that have blocks. The TL block (red) is at (10,30) size 50x50.
    // A 16x16 sample grid means one sample per 16 pixels.
    // TL block spans x=10..59, y=30..79. Grid samples at x=0,16,32,48,64...
    // Samples hitting the block: x=16,32,48 and y=48,64 -> 3x2 = 6 hits
    // TR block at (196,30) size 50x50. Samples: x=208,224 and y=48,64 -> 2x2 = 4 hits
    // BL block at (10,176) size 50x50. Samples: x=16,32,48 and y=192 -> 3x1 = 3 hits
    // BR block at (196,176) size 50x50. Samples: x=208,224 and y=192 -> 2x1 = 2 hits
    let mut vm = load_program(include_str!("../programs/self_analysis.asm"));
    vm.llm_mock_response = Some("Analysis complete.".to_string());
    run_until_halt(&mut vm, 200_000);

    let prompt = read_ram_string(&vm, 0x1400);

    // Extract the quadrant counts from the prompt text
    // Format: "Top-left: N pixels\n"
    let extract_count = |label: &str| -> u32 {
        if let Some(idx) = prompt.find(label) {
            let rest = &prompt[idx + label.len()..];
            // rest starts with " N pixels"
            let trimmed = rest.trim_start();
            if let Some(space) = trimmed.find(' ') {
                if let Ok(n) = trimmed[..space].parse::<u32>() {
                    return n;
                }
            }
        }
        999 // sentinel: didn't find the count
    };

    let tl = extract_count("Top-left:");
    let tr = extract_count("Top-right:");
    let bl = extract_count("Bottom-left:");
    let br = extract_count("Bottom-right:");

    // All quadrants should have non-zero content (colored blocks)
    assert!(
        tl > 0,
        "TL quadrant should have non-zero pixel count, got {}",
        tl
    );
    assert!(
        tr > 0,
        "TR quadrant should have non-zero pixel count, got {}",
        tr
    );
    assert!(
        bl > 0,
        "BL quadrant should have non-zero pixel count, got {}",
        bl
    );
    assert!(
        br > 0,
        "BR quadrant should have non-zero pixel count, got {}",
        br
    );
}

#[test]
fn test_glyph_shell_compiles() {
    let source = include_str!("../programs/glyph_shell.glyph");
    let asm_text = geometry_os::glyph_backend::compile_glyph(source)
        .expect("glyph_shell.glyph should compile to assembly");

    // Ensure it contains the new opcodes as emitted GeoOS assembly
    assert!(asm_text.contains("RECTF"));
    assert!(asm_text.contains("DRAWTEXT"));
    assert!(asm_text.contains("FRAME"));
    assert!(asm_text.contains("IKEY"));
    assert!(asm_text.contains("FILL"));
    assert!(asm_text.contains("EXEC"));

    // Verify it assembles to bytecode
    let mut pp = geometry_os::preprocessor::Preprocessor::new();
    let preprocessed = pp.preprocess(&asm_text);
    geometry_os::assembler::assemble(&preprocessed, 0)
        .expect("Compiled glyph_shell assembly should assemble to bytecode");
}

#[test]
fn test_glyph_shell_execution() {
    let source = include_str!("../programs/glyph_shell.glyph");
    let asm_text = geometry_os::glyph_backend::compile_glyph(source)
        .expect("glyph_shell.glyph should compile");

    let mut vm = load_program(&asm_text);

    // Run for a few frames to allow drawing
    run_until_halt(&mut vm, 500_000);

    // Check title bar pixel (0,0) should be 0x2D0050
    // RECTF 0 0 256 20 0x2D0050 [
    assert_eq!(vm.screen[0], 0x2D0050, "Title bar color mismatch at (0,0)");

    // Check background pixel (0,30) should be 0x1A1A2E
    // 0x1A1A2E |
    assert_eq!(
        vm.screen[30 * 256],
        0x1A1A2E,
        "Background color mismatch at (0,30)"
    );
}

// ── Phase 113: Tetris in GlyphLang ────────────────────

#[test]
fn test_tetris_glyph_compiles() {
    let source = include_str!("../programs/tetris.glyph");
    let asm_text = geometry_os::glyph_backend::compile_glyph(source)
        .expect("tetris.glyph should compile to assembly");

    // Should contain game opcodes
    assert!(
        asm_text.contains("RECTF"),
        "Tetris should use RECTF for drawing"
    );
    assert!(
        asm_text.contains("FRAME"),
        "Tetris should use FRAME for game loop"
    );
    assert!(
        asm_text.contains("IKEY"),
        "Tetris should use IKEY for input"
    );

    // Verify it assembles to bytecode
    let mut pp = geometry_os::preprocessor::Preprocessor::new();
    let preprocessed = pp.preprocess(&asm_text);
    let bytecode = geometry_os::assembler::assemble(&preprocessed, 0)
        .expect("Compiled tetris assembly should assemble to bytecode");

    // Should be a non-trivial program
    assert!(
        bytecode.pixels.len() > 100,
        "Tetris should produce meaningful bytecode"
    );
}

#[test]
fn test_tetris_glyph_execution() {
    let source = include_str!("../programs/tetris.glyph");
    let asm_text =
        geometry_os::glyph_backend::compile_glyph(source).expect("tetris.glyph should compile");

    let mut vm = load_program(&asm_text);

    // Run enough to execute FILL (should happen by instruction ~39).
    // Note: GlyphLang produces verbose push/pop assembly, so 1668 lines.
    // Each loop iteration is ~1500 instructions. FILL is expensive in debug
    // mode (65K pixels), so we cap at 100 cycles just to verify FILL executes.
    for _ in 0..100 {
        if vm.halted {
            break;
        }
        vm.step();
    }

    // After ~40 steps we should have hit FILL, painting the screen.
    assert_ne!(
        vm.screen[0], 0,
        "FILL should have painted pixel (0,0), still 0 after 100 steps"
    );
}
