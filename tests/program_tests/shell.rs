use super::*;

#[test]
fn test_readln_no_key_yields() {
    let mut vm = Vm::new();
    vm.regs[0] = 0x200;  // buf addr
    vm.regs[1] = 64;     // max len
    vm.regs[2] = 0x300;  // pos addr
    vm.ram[0x300] = 0;   // pos = 0
    step_readln(&mut vm, 0); // no key
    assert_eq!(vm.regs[0], 0, "READLN should return 0 when no key");
    assert!(vm.yielded, "READLN should yield when no key available");
}

#[test]
fn test_readln_char_accumulation() {
    let mut vm = Vm::new();
    vm.regs[0] = 0x200;
    vm.regs[1] = 64;
    vm.regs[2] = 0x300;
    vm.ram[0x300] = 0;

    // Simulate 'H' (72)
    step_readln(&mut vm, 72);
    assert_eq!(vm.ram[0x200], 72, "Should store 'H' in buffer");
    assert_eq!(vm.ram[0x300], 1, "Position should be 1");
    assert_eq!(vm.ram[0xFFF], 0, "Key port should be cleared");

    // Re-set r0 (buf addr) since READLN returns result in r0
    vm.regs[0] = 0x200;

    // Simulate 'i' (105)
    step_readln(&mut vm, 105);
    assert_eq!(vm.ram[0x201], 105, "Should store 'i'");
    assert_eq!(vm.ram[0x300], 2, "Position should be 2");
}

#[test]
fn test_readln_enter_terminates() {
    let mut vm = Vm::new();
    vm.regs[0] = 0x200;
    vm.regs[1] = 64;
    vm.regs[2] = 0x300;
    vm.ram[0x300] = 0;

    // Pre-load "Hi" at buffer
    vm.ram[0x200] = 72;  // H
    vm.ram[0x201] = 105; // i
    vm.ram[0x300] = 2;   // pos = 2

    // Send Enter (13)
    step_readln(&mut vm, 13);
    assert_eq!(vm.regs[0], 2, "Should return length 2");
    assert_eq!(vm.ram[0x202], 0, "Should null-terminate");
    assert_eq!(vm.ram[0x300], 0, "Position should be reset");
}

#[test]
fn test_readln_backspace() {
    let mut vm = Vm::new();
    vm.regs[0] = 0x200;
    vm.regs[1] = 64;
    vm.regs[2] = 0x300;
    vm.ram[0x300] = 3; // pos = 3

    // Send Backspace (8)
    step_readln(&mut vm, 8);
    assert_eq!(vm.regs[0], 0, "Should return 0 for backspace");
    assert_eq!(vm.ram[0x300], 2, "Position should decrement to 2");
}

#[test]
fn test_readln_max_len() {
    let mut vm = Vm::new();
    vm.regs[0] = 0x200;
    vm.regs[1] = 2;      // max len = 2
    vm.regs[2] = 0x300;
    vm.ram[0x300] = 2;   // already at max

    // Try to add another char -- should be rejected
    step_readln(&mut vm, 88); // 'X'
    assert_eq!(vm.ram[0x300], 2, "Position should stay at max");
    assert_eq!(vm.ram[0x202], 0, "Buffer at pos 2 should not be written");
}



// ── READLN non-printable discard test ────────────────────────

#[test]
fn test_readln_non_printable_discarded() {
    let mut vm = Vm::new();
    vm.regs[0] = 0x200;
    vm.regs[1] = 64;
    vm.regs[2] = 0x300;
    vm.ram[0x300] = 0;

    // Send ESC (27) -- non-printable, should be discarded
    step_readln(&mut vm, 27);
    assert_eq!(vm.ram[0x300], 0, "Position should stay 0 for non-printable");
    assert_eq!(vm.ram[0xFFF], 0, "Key should be consumed");
}



// ── READLN buffer full test ──────────────────────────────────

#[test]
fn test_readln_buffer_full_no_overflow() {
    let mut vm = Vm::new();
    vm.regs[0] = 0x200;
    vm.regs[1] = 2; // max_len = 2
    vm.regs[2] = 0x300;
    vm.ram[0x300] = 2; // already at max

    step_readln(&mut vm, 88); // 'X'
    assert_eq!(vm.ram[0x300], 2, "Position should stay at max");
    assert_eq!(vm.ram[0x202], 0, "Buffer at pos 2 should not be written");
}



// ── WAITPID opcode ──────────────────────────────────────────────

#[test]
fn test_waitpid_not_running() {
    let mut vm = Vm::new();
    vm.regs[1] = 42; // PID that doesn't exist
    step_waitpid(&mut vm);
    assert_eq!(vm.regs[0], 1, "WAITPID should return 1 for non-existent PID");
}

#[test]
fn test_waitpid_still_running() {
    let mut vm = Vm::new();
    vm.processes.push(geometry_os::vm::SpawnedProcess {
        pc: 0,
        regs: [0; 32],
        state: geometry_os::vm::ProcessState::Ready,
        pid: 1,
        mode: geometry_os::vm::CpuMode::User,
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
                                signal_handlers: [0; 4], vmas: Vec::new(), brk_pos: 0,
    });
    vm.regs[1] = 1; // PID of running process
    step_waitpid(&mut vm);
    assert_eq!(vm.regs[0], 0, "WAITPID should return 0 for running PID");
    assert!(vm.yielded, "WAITPID should yield when process is running");
}

#[test]
fn test_waitpid_halted_process() {
    let mut vm = Vm::new();
    vm.processes.push(geometry_os::vm::SpawnedProcess {
        pc: 0,
        regs: [0; 32],
        state: geometry_os::vm::ProcessState::Zombie,
        pid: 1,
        mode: geometry_os::vm::CpuMode::User,
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
                                signal_handlers: [0; 4], vmas: Vec::new(), brk_pos: 0,
    });
    vm.regs[1] = 1;
    step_waitpid(&mut vm);
    assert_eq!(vm.regs[0], 1, "WAITPID should return 1 for halted PID");
}



// ── Shell assembly test ─────────────────────────────────────────

#[test]
fn test_shell_assembles() {
    let source = std::fs::read_to_string("programs/shell.asm")
        .unwrap_or_else(|e| panic!("failed to read shell.asm: {}", e));
    let result = assemble(&source, 0);
    assert!(result.is_ok(), "shell.asm should assemble: {:?}", result.err());
}

#[test]
fn test_readln_waitpid_assembler_entries() {
    let src = "READLN r0, r1, r2\nWAITPID r3\nHALT";
    let result = assemble(src, 0).expect("assembly should succeed");
    assert_eq!(result.pixels[0], 0x68, "READLN opcode");
    assert_eq!(result.pixels[4], 0x69, "WAITPID opcode");
    assert_eq!(result.pixels[6], 0x00, "HALT opcode");
}



// ── CHDIR/GETCWD opcode tests ────────────────────────────────

#[test]
fn test_chdir_sets_cwd() {
    let mut vm = Vm::new();
    // Write "/home" at RAM 0x200
    let path = b"/home";
    for (i, &b) in path.iter().enumerate() {
        vm.ram[0x200 + i] = b as u32;
    }
    vm.ram[0x200 + path.len()] = 0;

    vm.regs[0] = 0x200;
    vm.ram[vm.pc as usize] = 0x6B; // CHDIR
    vm.ram[vm.pc as usize + 1] = 0; // r0
    vm.step();
    assert_eq!(vm.regs[0], 0, "CHDIR should return 0 on success");
    assert_eq!(
        vm.env_vars.get("CWD").expect("map entry should exist"),
        "/home",
        "CHDIR should set CWD env var"
    );
}

#[test]
fn test_getcwd_reads_cwd() {
    let mut vm = Vm::new();
    vm.env_vars.insert("CWD".to_string(), "/tmp".to_string());

    vm.regs[0] = 0x200;
    vm.ram[vm.pc as usize] = 0x6C; // GETCWD
    vm.ram[vm.pc as usize + 1] = 0; // r0
    vm.step();

    assert_eq!(vm.regs[0], 4, "GETCWD should return length 4");
    assert_eq!(vm.ram[0x200] as u8, b'/' as u8);
    assert_eq!(vm.ram[0x201] as u8, b't' as u8);
    assert_eq!(vm.ram[0x202] as u8, b'm' as u8);
    assert_eq!(vm.ram[0x203] as u8, b'p' as u8);
    assert_eq!(vm.ram[0x204], 0, "Should be null-terminated");
}

#[test]
fn test_getcwd_default_root() {
    let mut vm = Vm::new();
    // No CWD set -- should default to "/"
    vm.regs[0] = 0x200;
    vm.ram[vm.pc as usize] = 0x6C; // GETCWD
    vm.ram[vm.pc as usize + 1] = 0;
    vm.step();

    assert_eq!(vm.regs[0], 1, "GETCWD should return length 1 for default");
    assert_eq!(vm.ram[0x200] as u8, b'/' as u8);
}



// ── EXECP assembler test ─────────────────────────────────────

#[test]
fn test_execp_assembles() {
    let src = "EXECP r0, r1, r2\nHALT";
    let result = assemble(src, 0).expect("assembly should succeed");
    assert_eq!(result.pixels[0], 0x6A, "EXECP opcode");
    assert_eq!(result.pixels[1], 0, "path reg");
    assert_eq!(result.pixels[2], 1, "stdin_fd reg");
    assert_eq!(result.pixels[3], 2, "stdout_fd reg");
}



// ── CHDIR/GETCWD assembler test ──────────────────────────────

#[test]
fn test_chdir_getcwd_assembles() {
    let src = "CHDIR r0\nGETCWD r1\nHALT";
    let result = assemble(src, 0).expect("assembly should succeed");
    assert_eq!(result.pixels[0], 0x6B, "CHDIR opcode");
    assert_eq!(result.pixels[1], 0, "path reg");
    assert_eq!(result.pixels[2], 0x6C, "GETCWD opcode");
    assert_eq!(result.pixels[3], 1, "buf reg");
    assert_eq!(result.pixels[4], 0x00, "HALT");
}



// ============================================================
// Phase 29: Shell opcodes -- GETPID, GETENV, SETENV
// ============================================================

#[test]
fn test_getpid_assembles() {
    let source = "
    GETPID
    HALT
    ";
    let result = assemble(source, 0);
    assert!(result.is_ok(), "GETPID should assemble: {:?}", result.err());
    let bc = &result.expect("operation should succeed").pixels;
    assert_eq!(bc[0], 0x65, "GETPID opcode should be 0x65");
}

#[test]
fn test_getpid_returns_zero_in_kernel_mode() {
    // In kernel mode (no spawned process), PID should be 0
    let mut vm = Vm::new();
    vm.ram[0] = 0x65; // GETPID
    vm.step();
    assert_eq!(vm.regs[0], 0, "GETPID should return 0 in kernel context");
}

#[test]
fn test_getpid_returns_pid_in_spawned_process() {
    // SPAWN a child process, then GETPID inside it should return its PID
    let mut vm = Vm::new();
    vm.mode = geometry_os::vm::CpuMode::Kernel;

    // Write a small program at address 0x200: GETPID, HALT
    vm.ram[0x200] = 0x65; // GETPID
    vm.ram[0x201] = 0x00; // HALT

    // At address 0: LDI r1, 0x200; SPAWN r1; HALT
    vm.ram[0] = 0x10;    // LDI r1, imm16
    vm.ram[1] = 1;       // r1
    vm.ram[2] = 0x200;   // address
    vm.ram[3] = 0x4D;    // SPAWN
    vm.ram[4] = 1;       // r1
    vm.ram[5] = 0x00;    // HALT

    // Run main process to completion
    for _ in 0..20 { if !vm.step() { break; } }

    let child_pid = vm.ram[0xFFA];
    assert!(child_pid > 0 && child_pid != 0xFFFFFFFF, "SPAWN should set RAM[0xFFA] to a valid PID, got {}", child_pid);
    assert_eq!(vm.processes.len(), 1);

    // Run the child process through the scheduler
    for _ in 0..20 {
        vm.step_all_processes();
        if vm.processes[0].is_halted() { break; }
    }

    let child = &vm.processes[0];
    assert_eq!(child.regs[0], child_pid, "Child GETPID should return its spawned PID");
}

#[test]
fn test_getenv_assembles() {
    let source = "
    GETENV r1, r2
    HALT
    ";
    let result = assemble(source, 0);
    assert!(result.is_ok(), "GETENV should assemble: {:?}", result.err());
    let bc = &result.expect("operation should succeed").pixels;
    assert_eq!(bc[0], 0x63, "GETENV opcode should be 0x63");
    assert_eq!(bc[1], 1, "key_reg");
    assert_eq!(bc[2], 2, "val_reg");
}

#[test]
fn test_setenv_assembles() {
    let source = "
    SETENV r1, r2
    HALT
    ";
    let result = assemble(source, 0);
    assert!(result.is_ok(), "SETENV should assemble: {:?}", result.err());
    let bc = &result.expect("operation should succeed").pixels;
    assert_eq!(bc[0], 0x64, "SETENV opcode should be 0x64");
    assert_eq!(bc[1], 1, "key_reg");
    assert_eq!(bc[2], 2, "val_reg");
}

#[test]
fn test_setenv_and_getenv_roundtrip() {
    let mut vm = Vm::new();

    // Set PATH=/bin
    write_string(&mut vm.ram, 0x1000, "PATH");
    write_string(&mut vm.ram, 0x1100, "/bin");
    vm.regs[1] = 0x1000; // key addr
    vm.regs[2] = 0x1100; // val addr

    vm.ram[0] = 0x64; // SETENV
    vm.ram[1] = 1;    // key_reg
    vm.ram[2] = 2;    // val_reg
    vm.step();
    assert_eq!(vm.regs[0], 0, "SETENV should return 0 on success");

    // Now GETENV
    write_string(&mut vm.ram, 0x1200, "PATH"); // key to look up
    vm.regs[1] = 0x1200; // key addr
    vm.regs[3] = 0x2000; // output buffer
    vm.pc = 10;
    vm.ram[10] = 0x63; // GETENV
    vm.ram[11] = 1;    // key_reg
    vm.ram[12] = 3;    // val_reg
    vm.step();
    assert_eq!(vm.regs[0], 4, "GETENV should return length 4 for '/bin'");

    // Read back the value from RAM
    let mut result = String::new();
    for i in 0..4 {
        result.push((vm.ram[0x2000 + i] & 0xFF) as u8 as char);
    }
    assert_eq!(result, "/bin", "GETENV should write '/bin' to output buffer");
}

#[test]
fn test_getenv_not_found() {
    let mut vm = Vm::new();
    write_string(&mut vm.ram, 0x1000, "NONEXISTENT");
    vm.regs[1] = 0x1000;
    vm.regs[2] = 0x2000;

    vm.ram[0] = 0x63; // GETENV
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.step();
    assert_eq!(vm.regs[0], 0xFFFFFFFF, "GETENV should return 0xFFFFFFFF when key not found");
}

#[test]
fn test_setenv_overwrite() {
    let mut vm = Vm::new();

    // Set HOME=/root
    write_string(&mut vm.ram, 0x1000, "HOME");
    write_string(&mut vm.ram, 0x1100, "/root");
    vm.regs[1] = 0x1000;
    vm.regs[2] = 0x1100;
    vm.ram[0] = 0x64; // SETENV
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.step();

    // Overwrite HOME=/home/user
    write_string(&mut vm.ram, 0x1100, "/home/user");
    vm.regs[1] = 0x1000;
    vm.regs[2] = 0x1100;
    vm.pc = 10;
    vm.ram[10] = 0x64;
    vm.ram[11] = 1;
    vm.ram[12] = 2;
    vm.step();
    assert_eq!(vm.regs[0], 0, "SETENV overwrite should succeed");

    // Verify GETENV returns new value
    write_string(&mut vm.ram, 0x1200, "HOME");
    vm.regs[1] = 0x1200;
    vm.regs[3] = 0x2000;
    vm.pc = 20;
    vm.ram[20] = 0x63;
    vm.ram[21] = 1;
    vm.ram[22] = 3;
    vm.step();
    assert_eq!(vm.regs[0], 10, "GETENV should return length 10 for '/home/user'");

    let mut result = String::new();
    for i in 0..10 {
        result.push((vm.ram[0x2000 + i] & 0xFF) as u8 as char);
    }
    assert_eq!(result, "/home/user");
}

#[test]
fn test_getenv_null_terminates_output() {
    let mut vm = Vm::new();

    // Set SHELL=/sh
    write_string(&mut vm.ram, 0x1000, "SHELL");
    write_string(&mut vm.ram, 0x1100, "/sh");
    vm.regs[1] = 0x1000;
    vm.regs[2] = 0x1100;
    vm.ram[0] = 0x64; // SETENV
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.step();

    // GETENV
    write_string(&mut vm.ram, 0x1200, "SHELL");
    vm.regs[1] = 0x1200;
    vm.regs[3] = 0x2000;
    vm.pc = 10;
    vm.ram[10] = 0x63;
    vm.ram[11] = 1;
    vm.ram[12] = 3;
    vm.step();

    assert_eq!(vm.ram[0x2000 + 3], 0, "GETENV should null-terminate output");
}

#[test]
fn test_setenv_max_32_vars() {
    let mut vm = Vm::new();

    // Set 32 different env vars -- should all succeed
    for i in 0..32 {
        let key = format!("VAR{}", i);
        let val = format!("val{}", i);
        write_string(&mut vm.ram, 0x1000, &key);
        write_string(&mut vm.ram, 0x1100, &val);
        vm.regs[1] = 0x1000;
        vm.regs[2] = 0x1100;
        vm.pc = 10;
        vm.ram[10] = 0x64;
        vm.ram[11] = 1;
        vm.ram[12] = 2;
        vm.step();
        assert_eq!(vm.regs[0], 0, "SETENV #{} should succeed", i);
    }

    // 33rd unique var should fail
    write_string(&mut vm.ram, 0x1000, "TOO_MANY");
    write_string(&mut vm.ram, 0x1100, "nope");
    vm.regs[1] = 0x1000;
    vm.regs[2] = 0x1100;
    vm.pc = 10;
    vm.ram[10] = 0x64;
    vm.ram[11] = 1;
    vm.ram[12] = 2;
    vm.step();
    assert_eq!(vm.regs[0], 0xFFFFFFFF, "33rd SETENV should fail (max 32)");
}

#[test]
fn test_env_vars_persist_across_processes() {
    // SETENV in main process, GETENV in spawned child
    let mut vm = Vm::new();
    vm.mode = geometry_os::vm::CpuMode::Kernel;

    // Set USER=alice in main
    write_string(&mut vm.ram, 0x1000, "USER");
    write_string(&mut vm.ram, 0x1100, "alice");
    vm.regs[1] = 0x1000;
    vm.regs[2] = 0x1100;
    vm.ram[0] = 0x64; // SETENV
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.step();

    // Child at 0x200: GETENV r1, r2; HALT
    write_string(&mut vm.ram, 0x300, "USER"); // key for child to look up
    vm.ram[0x200] = 0x63; // GETENV
    vm.ram[0x201] = 1;    // key_reg (r1)
    vm.ram[0x202] = 2;    // val_reg (r2)
    vm.ram[0x203] = 0x00; // HALT

    // SPAWN at address 5
    vm.regs[1] = 0x200;
    vm.pc = 5;
    vm.ram[5] = 0x4D;  // SPAWN
    vm.ram[6] = 1;     // r1 = child addr
    vm.ram[7] = 0x00;  // HALT
    vm.step(); // SPAWN

    // Set child's r1 to key addr, r2 to output buffer
    vm.processes[0].regs[1] = 0x300; // key addr
    vm.processes[0].regs[2] = 0x400; // output buffer

    for _ in 0..20 {
        vm.step_all_processes();
        if vm.processes[0].is_halted() { break; }
    }

    let child = &vm.processes[0];
    assert_eq!(child.regs[0], 5, "Child should see USER=value of length 5");

    let mut result = String::new();
    for i in 0..5 {
        result.push((vm.ram[0x400 + i] & 0xFF) as u8 as char);
    }
    assert_eq!(result, "alice", "Child should read 'alice' from parent's env");
}
