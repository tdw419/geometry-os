// Phase 32: Signals & Process Lifecycle tests
// Tests for EXIT, SIGNAL, SIGSET opcodes and zombie cleanup via WAITPID

use geometry_os::assembler::assemble;
use geometry_os::vm::Vm;

/// Helper: create a minimal VM with a child process at given address
fn vm_with_child(child_pc: u32) -> Vm {
    let mut vm = Vm::new();
    vm.processes.push(geometry_os::vm::SpawnedProcess {
        pc: child_pc,
        regs: [0; 32],
        state: geometry_os::vm::ProcessState::Ready,
        pid: 1,
        mode: geometry_os::vm::CpuMode::Kernel,
        page_dir: None,
        segfaulted: false,
        priority: 1,
        slice_remaining: 100,
        sleep_until: 0,
        yielded: false,
        kernel_stack: Vec::new(),
        msg_queue: Vec::new(),
        exit_code: 0,
        parent_pid: 0,
        pending_signals: Vec::new(),
        signal_handlers: [0; 4], vmas: Vec::new(), brk_pos: 0,
    });
    vm.current_pid = 1;
    vm
}

#[test]
fn test_exit_opcode_halts_child() {
    let mut vm = vm_with_child(0x100);
    // EXIT r0: opcode 0x6F, arg = r0
    vm.ram[0x100] = 0x6F; // EXIT
    vm.ram[0x101] = 0;    // r0 (exit code register)
    vm.regs[0] = 42;       // exit code = 42
    vm.pc = 0x100;

    vm.step();

    assert!(vm.halted, "child should be halted after EXIT");
    assert_eq!(vm.step_exit_code, Some(42), "exit code should be 42");
    assert!(vm.step_zombie, "child should be marked as zombie");
}

#[test]
fn test_exit_main_process_halts() {
    let mut vm = Vm::new();
    // EXIT r0 on main process (pid 0)
    vm.ram[0] = 0x6F; // EXIT
    vm.ram[1] = 0;    // r0
    vm.regs[0] = 5;
    vm.pc = 0;

    vm.step();

    assert!(vm.halted, "main process should halt on EXIT");
    assert_eq!(vm.step_exit_code, None, "main process should not set step_exit_code");
    assert!(!vm.step_zombie, "main process should not become zombie");
}

#[test]
fn test_waitpid_returns_exit_code() {
    let mut vm = vm_with_child(0x100);
    // Make child exit with code 7
    vm.ram[0x100] = 0x6F; // EXIT
    vm.ram[0x101] = 0;    // r0
    vm.regs[0] = 7;
    vm.current_pid = 1;
    vm.pc = 0x100;

    // Run child step (child exits)
    vm.step();

    // Save child state back to process list
    vm.processes[0].state = geometry_os::vm::ProcessState::Zombie;
    vm.processes[0].exit_code = vm.step_exit_code.unwrap_or(0);
    vm.step_zombie = true;

    // Switch back to main process
    vm.current_pid = 0;
    vm.halted = false;

    // Now WAITPID should reap the zombie
    vm.regs[1] = 1; // target PID
    // WAITPID r1: opcode 0x69, arg = r1
    vm.ram[0] = 0x69;
    vm.ram[1] = 1; // r1
    vm.pc = 0;
    vm.step();

    assert_eq!(vm.regs[0], 1, "r0 should be 1 (done)");
    assert_eq!(vm.regs[1], 7, "r1 should be exit code 7");
    assert_eq!(vm.processes.len(), 0, "zombie should be reaped");
}

#[test]
fn test_waitpid_running_yields() {
    let mut vm = vm_with_child(0x100);
    // Child is running a NOP loop
    vm.ram[0x100] = 0x01; // NOP
    vm.current_pid = 0;
    vm.halted = false;

    // WAITPID r1 (pid 1)
    vm.regs[1] = 1;
    vm.ram[0] = 0x69;
    vm.ram[1] = 1;
    vm.pc = 0;

    vm.step();

    assert_eq!(vm.regs[0], 0, "r0 should be 0 (still running)");
    assert!(vm.yielded, "should yield while waiting");
}

#[test]
fn test_signal_term_halts_process() {
    let mut vm = vm_with_child(0x100);
    // Child is running NOP
    vm.ram[0x100] = 0x01; // NOP
    vm.current_pid = 0;
    vm.halted = false;

    // SIGNAL r1, r2: opcode 0x70, pid_reg=r1, sig_reg=r2
    vm.regs[1] = 1; // target PID
    vm.regs[2] = 0; // signal 0 = TERM
    vm.ram[0] = 0x70;
    vm.ram[1] = 1; // r1
    vm.ram[2] = 2; // r2
    vm.pc = 0;

    vm.step();

    assert_eq!(vm.regs[0], 0, "SIGNAL should succeed");
    assert!(vm.processes[0].is_halted(), "child should be halted by SIGTERM");
    assert_eq!(vm.processes[0].exit_code, 1, "SIGTERM sets exit code 1");
    assert!(vm.processes[0].state == geometry_os::vm::ProcessState::Zombie, "child should be zombie");
}

#[test]
fn test_signal_stop_halts_process() {
    let mut vm = vm_with_child(0x100);
    vm.ram[0x100] = 0x01; // NOP
    vm.current_pid = 0;
    vm.halted = false;

    // SIGNAL r1, r2: signal 3 = STOP
    vm.regs[1] = 1; // target PID
    vm.regs[2] = 3; // signal 3 = STOP
    vm.ram[0] = 0x70;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.pc = 0;

    vm.step();

    assert_eq!(vm.regs[0], 0, "SIGNAL should succeed");
    assert!(vm.processes[0].is_halted(), "child should be halted by SIGSTOP");
    assert_eq!(vm.processes[0].exit_code, 2, "SIGSTOP sets exit code 2");
}

#[test]
fn test_signal_user_ignored_by_default() {
    let mut vm = vm_with_child(0x100);
    vm.ram[0x100] = 0x01; // NOP
    vm.current_pid = 0;
    vm.halted = false;

    // SIGNAL r1, r2: signal 1 = USER1 (default: ignore)
    vm.regs[1] = 1; // target PID
    vm.regs[2] = 1; // signal 1 = USER1
    vm.ram[0] = 0x70;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.pc = 0;

    vm.step();

    assert_eq!(vm.regs[0], 0, "SIGNAL should succeed");
    assert!(!vm.processes[0].is_halted(), "USER1 should not halt by default");
}

#[test]
fn test_signal_invalid_pid() {
    let mut vm = Vm::new();
    vm.current_pid = 0;

    // SIGNAL to non-existent PID 99
    vm.regs[1] = 99;
    vm.regs[2] = 0; // TERM
    vm.ram[0] = 0x70;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.pc = 0;

    vm.step();

    assert_eq!(vm.regs[0], 0xFFFFFFFF, "invalid PID should return error");
}

#[test]
fn test_signal_invalid_signal_number() {
    let mut vm = vm_with_child(0x100);
    vm.current_pid = 0;
    vm.halted = false;

    // SIGNAL with invalid signal number 99
    vm.regs[1] = 1;
    vm.regs[2] = 99;
    vm.ram[0] = 0x70;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.pc = 0;

    vm.step();

    assert_eq!(vm.regs[0], 0xFFFFFFFF, "invalid signal should return error");
}

#[test]
fn test_sigset_registers_handler() {
    let mut vm = vm_with_child(0x100);
    vm.current_pid = 1;
    vm.halted = false;

    // SIGSET r1, r2: set handler for signal 0 (TERM) to address 0x500
    vm.regs[1] = 0;     // signal 0 = TERM
    vm.regs[2] = 0x500; // handler address
    vm.ram[0x100] = 0x71; // SIGSET
    vm.ram[0x101] = 1;    // r1
    vm.ram[0x102] = 2;    // r2
    vm.pc = 0x100;

    vm.step();

    assert_eq!(vm.regs[0], 0, "SIGSET should succeed");
    assert_eq!(vm.processes[0].signal_handlers[0], 0x500, "TERM handler should be 0x500");
}

#[test]
fn test_sigset_ignore_signal() {
    let mut vm = vm_with_child(0x100);
    vm.current_pid = 1;
    vm.halted = false;

    // SIGSET r1, r2: ignore signal 1 (USER1)
    vm.regs[1] = 1;            // signal 1
    vm.regs[2] = 0xFFFFFFFF;   // ignore
    vm.ram[0x100] = 0x71;
    vm.ram[0x101] = 1;
    vm.ram[0x102] = 2;
    vm.pc = 0x100;

    vm.step();

    assert_eq!(vm.regs[0], 0, "SIGSET should succeed");
    assert_eq!(vm.processes[0].signal_handlers[1], 0xFFFFFFFF, "USER1 should be ignored");
}

#[test]
fn test_sigset_main_process_fails() {
    let mut vm = Vm::new();
    vm.current_pid = 0;

    // SIGSET on main process should fail
    vm.regs[1] = 0;
    vm.regs[2] = 0x500;
    vm.ram[0] = 0x71;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.pc = 0;

    vm.step();

    assert_eq!(vm.regs[0], 0xFFFFFFFF, "SIGSET should fail for main process");
}

#[test]
fn test_signal_with_custom_handler() {
    let mut vm = vm_with_child(0x100);
    // Set up custom handler for TERM (signal 0) at address 0x500
    vm.processes[0].signal_handlers[0] = 0x500;

    // Handler at 0x500: just NOP and HALT
    vm.ram[0x500] = 0x01; // NOP
    vm.ram[0x501] = 0x00; // HALT

    vm.current_pid = 0;
    vm.halted = false;

    // SIGNAL r1, r2: send TERM to pid 1
    vm.regs[1] = 1; // target PID
    vm.regs[2] = 0; // signal TERM
    vm.ram[0] = 0x70;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.pc = 0;

    vm.step();

    assert_eq!(vm.regs[0], 0, "SIGNAL should succeed");
    assert!(!vm.processes[0].is_halted(), "custom handler should prevent halt");
    assert_eq!(vm.processes[0].pc, 0x500, "PC should jump to handler");
    assert_eq!(vm.processes[0].regs[0], 0, "r0 should be signal number (0)");
    assert_eq!(vm.processes[0].regs[1], 0, "r1 should be sender PID (0)");
}

#[test]
fn test_exit_assembler() {
    let source = "EXIT r5";
    let result = assemble(source, 0).unwrap();
    assert_eq!(result.pixels[0], 0x6F, "EXIT opcode");
    assert_eq!(result.pixels[1], 5, "register r5");
}

#[test]
fn test_signal_assembler() {
    let source = "SIGNAL r3, r7";
    let result = assemble(source, 0).unwrap();
    assert_eq!(result.pixels[0], 0x70, "SIGNAL opcode");
    assert_eq!(result.pixels[1], 3, "pid register r3");
    assert_eq!(result.pixels[2], 7, "sig register r7");
}

#[test]
fn test_sigset_assembler() {
    let source = "SIGSET r1, r10";
    let result = assemble(source, 0).unwrap();
    assert_eq!(result.pixels[0], 0x71, "SIGSET opcode");
    assert_eq!(result.pixels[1], 1, "sig register r1");
    assert_eq!(result.pixels[2], 10, "handler register r10");
}

#[test]
fn test_signal_enum_from_u32() {
    use geometry_os::vm::Signal;
    assert_eq!(Signal::from_u32(0), Some(Signal::Term));
    assert_eq!(Signal::from_u32(1), Some(Signal::User1));
    assert_eq!(Signal::from_u32(2), Some(Signal::User2));
    assert_eq!(Signal::from_u32(3), Some(Signal::Stop));
    assert_eq!(Signal::from_u32(4), None);
    assert_eq!(Signal::from_u32(99), None);
}

#[test]
fn test_disassemble_exit() {
    let mut vm = Vm::new();
    vm.ram[0] = 0x6F;
    vm.ram[1] = 5;
    let (text, len) = vm.disassemble_at(0);
    assert_eq!(text, "EXIT r5");
    assert_eq!(len, 2);
}

#[test]
fn test_disassemble_signal() {
    let mut vm = Vm::new();
    vm.ram[0] = 0x70;
    vm.ram[1] = 3;
    vm.ram[2] = 1;
    let (text, len) = vm.disassemble_at(0);
    assert_eq!(text, "SIGNAL r3, r1");
    assert_eq!(len, 3);
}

#[test]
fn test_disassemble_sigset() {
    let mut vm = Vm::new();
    vm.ram[0] = 0x71;
    vm.ram[1] = 2;
    vm.ram[2] = 10;
    let (text, len) = vm.disassemble_at(0);
    assert_eq!(text, "SIGSET r2, r10");
    assert_eq!(len, 3);
}
