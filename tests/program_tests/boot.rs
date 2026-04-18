use super::*;

// ── PHASE 30: BOOT SEQUENCE & INIT ────────────────────────────

#[test]
fn test_shutdown_kernel_mode_halts_all() {
    // SHUTDOWN in kernel mode should halt the VM, set shutdown_requested
    let mut vm = Vm::new();
    // SHUTDOWN opcode (0x6E)
    vm.ram[0] = 0x6E;
    vm.pc = 0;
    vm.mode = geometry_os::vm::CpuMode::Kernel;

    let result = vm.step();
    assert!(!result, "SHUTDOWN should return false");
    assert!(vm.halted, "SHUTDOWN should halt the VM");
    assert!(
        vm.shutdown_requested,
        "SHUTDOWN should set shutdown_requested"
    );
}

#[test]
fn test_shutdown_user_mode_returns_error() {
    // SHUTDOWN in user mode should set r0 = 0xFFFFFFFF (permission denied)
    let mut vm = Vm::new();
    vm.ram[0] = 0x6E;
    vm.ram[1] = 0x00; // HALT after
    vm.pc = 0;
    vm.mode = geometry_os::vm::CpuMode::User;

    vm.step(); // SHUTDOWN (should fail, not halt)
    assert!(!vm.halted, "SHUTDOWN in user mode should not halt");
    assert_eq!(
        vm.regs[0], 0xFFFFFFFF,
        "SHUTDOWN in user mode should set r0 to error"
    );
    assert!(
        !vm.shutdown_requested,
        "SHUTDOWN in user mode should not set shutdown_requested"
    );
}

#[test]
fn test_shutdown_kills_child_processes() {
    // SHUTDOWN should halt all child processes and free their pages
    let source = "
    LDI r1, 0x200
    SPAWN r1
    LDI r1, 0x300
    SPAWN r1
    SHUTDOWN
    HALT

    .org 0x200
    LDI r9, 100
    SLEEP r9
    HALT

    .org 0x300
    LDI r9, 100
    SLEEP r9
    HALT
    ";
    let asm = assemble(source, 0).expect("assembly should succeed");
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() {
        vm.ram[i] = v;
    }
    vm.pc = 0;
    vm.mode = geometry_os::vm::CpuMode::Kernel;

    // Run until SHUTDOWN
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    assert!(vm.shutdown_requested, "SHUTDOWN should be requested");
    assert!(vm.halted, "VM should be halted");
    // Both children should be halted
    for proc in &vm.processes {
        assert!(
            proc.is_halted(),
            "child process should be halted after SHUTDOWN"
        );
    }
}

#[test]
fn test_shutdown_assembles() {
    let source = "SHUTDOWN\nHALT";
    let asm = assemble(source, 0).expect("assembly should succeed");
    assert_eq!(asm.pixels[0], 0x6E, "SHUTDOWN should assemble to 0x6E");
}

#[test]
fn test_shutdown_disassembles() {
    let mut vm = Vm::new();
    vm.ram[0] = 0x6E; // SHUTDOWN
    let (mnemonic, len) = vm.disassemble_at(0);
    assert_eq!(mnemonic, "SHUTDOWN");
    assert_eq!(len, 1);
}

#[test]
fn test_init_asm_assembles() {
    let source = std::fs::read_to_string("programs/init.asm").expect("init.asm should exist");
    assemble(&source, 0).expect("init.asm should assemble cleanly");
}

#[test]
fn test_boot_creates_init_process() {
    let mut vm = Vm::new();
    let pid = vm.boot().expect("boot should succeed");
    assert_eq!(pid, 1, "init process should get PID 1");
    assert!(vm.booted, "VM should be marked as booted");
    assert_eq!(
        vm.processes.len(),
        1,
        "should have exactly one child process"
    );
    assert_eq!(vm.processes[0].pid, 1);
    assert!(
        !vm.processes[0].is_halted(),
        "init process should be running"
    );
    assert_eq!(vm.processes[0].priority, 2, "init gets priority 2");
    assert_eq!(vm.processes[0].mode, geometry_os::vm::CpuMode::User);
}

#[test]
fn test_boot_sets_environment() {
    let mut vm = Vm::new();
    vm.boot().expect("boot should succeed");
    assert_eq!(vm.env_vars.get("SHELL").map(|s| s.as_str()), Some("shell"));
    assert_eq!(vm.env_vars.get("HOME").map(|s| s.as_str()), Some("/"));
    assert_eq!(vm.env_vars.get("CWD").map(|s| s.as_str()), Some("/"));
    assert_eq!(vm.env_vars.get("USER").map(|s| s.as_str()), Some("root"));
}

#[test]
fn test_boot_twice_fails() {
    let mut vm = Vm::new();
    vm.boot().expect("first boot should succeed");
    let result = vm.boot();
    assert!(result.is_err(), "second boot should fail");
}

#[test]
fn test_read_boot_config() {
    let mut vm = Vm::new();
    vm.boot().expect("boot should succeed");
    // Default boot.cfg has init=init, shell=shell
    assert_eq!(vm.read_boot_config("init"), Some("init".to_string()));
    assert_eq!(vm.read_boot_config("shell"), Some("shell".to_string()));
    assert_eq!(vm.read_boot_config("nonexistent"), None);
}

#[test]
fn test_shutdown_clears_pipes() {
    // SHUTDOWN should clear all pipes
    let source = "
    LDI r5, 0
    LDI r6, 0
    PIPE r5, r6
    SHUTDOWN
    HALT
    ";
    let asm = assemble(source, 0).expect("assembly should succeed");
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() {
        vm.ram[i] = v;
    }
    vm.pc = 0;
    vm.mode = geometry_os::vm::CpuMode::Kernel;

    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    assert!(vm.shutdown_requested);
    assert!(vm.pipes.is_empty(), "SHUTDOWN should clear all pipes");
}
