use super::*;



// ========================================================================
// Phase 26: Preemptive Scheduler Tests
// ========================================================================

#[test]
fn test_scheduler_basic_child_execution() {
    let source = "
    .org 0x200
    LDI r1, 0x1000
    LDI r2, 42
    STORE r1, r2
    HALT
    ";
    let asm = assemble(source, 0).expect("assembly should succeed");
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() {
        vm.ram[i] = v;
    }
    vm.processes.push(geometry_os::vm::SpawnedProcess {
        pc: 0x200,
        regs: [0; 32],
        state: geometry_os::vm::ProcessState::Ready,
        pid: 1,
        mode: geometry_os::vm::CpuMode::Kernel,
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
    for _ in 0..10 {
        vm.step_all_processes();
        if vm.processes[0].is_halted() { break; }
    }
    assert_eq!(vm.ram[0x1000], 42, "child should have written 42");
    assert!(vm.processes[0].is_halted(), "child should have halted");
}

#[test]
fn test_yield_forfeits_time_slice() {
    let source = "
    .org 0x200
    LDI r1, 0x1000
    LOAD r2, r1
    LDI r3, 1
    ADD r2, r3
    STORE r1, r2
    LDI r4, 5
    CMP r2, r4
    BGE r0, 0x21C
    JMP 0x200

    .org 0x21C
    YIELD
    JMP 0x21C
    ";
    let asm = assemble(source, 0).expect("assembly should succeed");
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() {
        vm.ram[i] = v;
    }
    vm.processes.push(geometry_os::vm::SpawnedProcess {
        pc: 0x200, regs: [0; 32], state: geometry_os::vm::ProcessState::Ready, pid: 1,
        mode: geometry_os::vm::CpuMode::Kernel, page_dir: None,
        segfaulted: false, priority: 1, slice_remaining: 0,
        sleep_until: 0, yielded: false,
        kernel_stack: Vec::new(), msg_queue: Vec::new(),
                                exit_code: 0,
                                parent_pid: 0,
                                pending_signals: Vec::new(),
                                signal_handlers: [0; 4], vmas: Vec::new(), brk_pos: 0,
    });
    for _ in 0..100 {
        vm.step_all_processes();
        if vm.processes[0].yielded { break; }
    }
    assert_eq!(vm.ram[0x1000], 5, "counter should be 5 after yield");
    assert!(vm.processes[0].yielded, "process should have yielded");

    let counter_after_yield = vm.ram[0x1000];
    vm.step_all_processes();
    vm.step_all_processes();
    assert_eq!(vm.ram[0x1000], counter_after_yield,
        "counter should not change during post-yield spin loop");
}

#[test]
fn test_sleep_skips_process_until_wake() {
    let source = "
    .org 0x200
    LDI r1, 0x1100
    LOAD r2, r1
    LDI r3, 1
    ADD r2, r3
    STORE r1, r2
    LDI r5, 10
    SLEEP r5
    JMP 0x200
    ";
    let asm = assemble(source, 0).expect("assembly should succeed");
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() {
        vm.ram[i] = v;
    }
    vm.processes.push(geometry_os::vm::SpawnedProcess {
        pc: 0x200, regs: [0; 32], state: geometry_os::vm::ProcessState::Ready, pid: 1,
        mode: geometry_os::vm::CpuMode::Kernel, page_dir: None,
        segfaulted: false, priority: 1, slice_remaining: 0,
        sleep_until: 0, yielded: false,
        kernel_stack: Vec::new(), msg_queue: Vec::new(),
                                exit_code: 0,
                                parent_pid: 0,
                                pending_signals: Vec::new(),
                                signal_handlers: [0; 4], vmas: Vec::new(), brk_pos: 0,
    });
    for _ in 0..20 {
        vm.step_all_processes();
        if vm.processes[0].sleep_until > 0 { break; }
    }
    assert!(vm.processes[0].sleep_until > 0, "process should be sleeping");
    let counter_at_sleep = vm.ram[0x1100];
    assert_eq!(counter_at_sleep, 1, "counter should be 1 before sleep");

    for _ in 0..5 { vm.step_all_processes(); }
    assert_eq!(vm.ram[0x1100], counter_at_sleep,
        "counter should not change while process is sleeping");

    for _ in 0..30 {
        vm.step_all_processes();
        if vm.processes[0].sleep_until == 0 && vm.ram[0x1100] > counter_at_sleep { break; }
    }
    assert!(vm.ram[0x1100] > counter_at_sleep,
        "counter should increment after sleep expires");
}

#[test]
fn test_priority_quantum_allocation() {
    // Verify that higher priority processes get larger time quantums.
    // Priority 3 = 8x multiplier, Priority 0 = 1x multiplier.
    // After first step, check slice_remaining reflects the difference.
    let source = "
    .org 0x200
    LDI r1, 0x1200
    LOAD r2, r1
    LDI r3, 1
    ADD r2, r3
    STORE r1, r2
    JMP 0x200

    .org 0x300
    LDI r1, 0x1300
    LOAD r2, r1
    LDI r3, 1
    ADD r2, r3
    STORE r1, r2
    JMP 0x300
    ";
    let asm = assemble(source, 0).expect("assembly should succeed");
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() {
        vm.ram[i] = v;
    }
    // Process A: priority 3 (quantum = 100 * 8 = 800)
    vm.processes.push(geometry_os::vm::SpawnedProcess {
        pc: 0x200, regs: [0; 32], state: geometry_os::vm::ProcessState::Ready, pid: 1,
        mode: geometry_os::vm::CpuMode::Kernel, page_dir: None,
        segfaulted: false, priority: 3, slice_remaining: 0,
        sleep_until: 0, yielded: false,
        kernel_stack: Vec::new(), msg_queue: Vec::new(),
                                exit_code: 0,
                                parent_pid: 0,
                                pending_signals: Vec::new(),
                                signal_handlers: [0; 4], vmas: Vec::new(), brk_pos: 0,
    });
    // Process B: priority 0 (quantum = 100 * 1 = 100)
    vm.processes.push(geometry_os::vm::SpawnedProcess {
        pc: 0x300, regs: [0; 32], state: geometry_os::vm::ProcessState::Ready, pid: 2,
        mode: geometry_os::vm::CpuMode::Kernel, page_dir: None,
        segfaulted: false, priority: 0, slice_remaining: 0,
        sleep_until: 0, yielded: false,
        kernel_stack: Vec::new(), msg_queue: Vec::new(),
                                exit_code: 0,
                                parent_pid: 0,
                                pending_signals: Vec::new(),
                                signal_handlers: [0; 4], vmas: Vec::new(), brk_pos: 0,
    });

    // Run one scheduling round to allocate quantums
    vm.step_all_processes();

    // Priority 3: quantum = 100 * (1 << 3) = 800, minus 1 for the step just taken
    let quantum_high = vm.processes[0].slice_remaining;
    // Priority 0: quantum = 100 * (1 << 0) = 100, minus 1 for the step just taken
    let quantum_low = vm.processes[1].slice_remaining;

    assert!(quantum_high > quantum_low,
        "priority 3 quantum ({}) should exceed priority 0 quantum ({})",
        quantum_high, quantum_low);
    // Verify exact values: 800-1=799 and 100-1=99
    assert_eq!(quantum_high, 799, "priority 3 should have quantum 799 (800-1)");
    assert_eq!(quantum_low, 99, "priority 0 should have quantum 99 (100-1)");
}

#[test]
fn test_setpriority_changes_priority() {
    let source = "
    .org 0x200
    LDI r1, 0x1400
    LOAD r2, r1
    LDI r3, 1
    ADD r2, r3
    STORE r1, r2
    LDI r4, 5
    CMP r2, r4
    BGE r0, 0x21C
    JMP 0x200

    .org 0x21C
    LDI r6, 3
    SETPRIORITY r6
    JMP 0x200
    ";
    let asm = assemble(source, 0).expect("assembly should succeed");
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() {
        vm.ram[i] = v;
    }
    vm.processes.push(geometry_os::vm::SpawnedProcess {
        pc: 0x200, regs: [0; 32], state: geometry_os::vm::ProcessState::Ready, pid: 1,
        mode: geometry_os::vm::CpuMode::Kernel, page_dir: None,
        segfaulted: false, priority: 0, slice_remaining: 0,
        sleep_until: 0, yielded: false,
        kernel_stack: Vec::new(), msg_queue: Vec::new(),
                                exit_code: 0,
                                parent_pid: 0,
                                pending_signals: Vec::new(),
                                signal_handlers: [0; 4], vmas: Vec::new(), brk_pos: 0,
    });
    for _ in 0..100 { vm.step_all_processes(); }
    assert_eq!(vm.processes[0].priority, 3, "priority should be upgraded to 3");
    assert!(vm.ram[0x1400] > 5, "counter should be > 5 after priority change");
}

#[test]
fn test_scheduler_tick_increments() {
    let mut vm = Vm::new();
    let initial_tick = vm.sched_tick;
    let source = ".org 0x200\nHALT\n";
    let asm = assemble(source, 0).expect("assembly should succeed");
    for (i, &v) in asm.pixels.iter().enumerate() { vm.ram[i] = v; }
    vm.processes.push(geometry_os::vm::SpawnedProcess {
        pc: 0x200, regs: [0; 32], state: geometry_os::vm::ProcessState::Ready, pid: 1,
        mode: geometry_os::vm::CpuMode::Kernel, page_dir: None,
        segfaulted: false, priority: 1, slice_remaining: 0,
        sleep_until: 0, yielded: false,
        kernel_stack: Vec::new(), msg_queue: Vec::new(),
                                exit_code: 0,
                                parent_pid: 0,
                                pending_signals: Vec::new(),
                                signal_handlers: [0; 4], vmas: Vec::new(), brk_pos: 0,
    });
    vm.step_all_processes();
    assert!(vm.sched_tick > initial_tick, "sched_tick should increment");
    let tick_after_one = vm.sched_tick;
    vm.step_all_processes();
    assert!(vm.sched_tick > tick_after_one,
        "sched_tick should keep incrementing even with halted processes");
}

#[test]
fn test_sleep_wakes_and_halts() {
    let source = "
    .org 0x200
    LDI r5, 5
    SLEEP r5
    HALT
    ";
    let asm = assemble(source, 0).expect("assembly should succeed");
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() { vm.ram[i] = v; }
    vm.processes.push(geometry_os::vm::SpawnedProcess {
        pc: 0x200, regs: [0; 32], state: geometry_os::vm::ProcessState::Ready, pid: 1,
        mode: geometry_os::vm::CpuMode::Kernel, page_dir: None,
        segfaulted: false, priority: 1, slice_remaining: 0,
        sleep_until: 0, yielded: false,
        kernel_stack: Vec::new(), msg_queue: Vec::new(),
                                exit_code: 0,
                                parent_pid: 0,
                                pending_signals: Vec::new(),
                                signal_handlers: [0; 4], vmas: Vec::new(), brk_pos: 0,
    });
    vm.step_all_processes(); // LDI r5, 5
    vm.step_all_processes(); // SLEEP r5
    assert!(vm.processes[0].sleep_until > 0, "process should be sleeping");
    for _ in 0..30 {
        vm.step_all_processes();
        if vm.processes[0].is_halted() { break; }
    }
    assert!(vm.processes[0].is_halted(), "process should have woken and halted");
    assert_eq!(vm.processes[0].sleep_until, 0, "sleep_until should be cleared");
}

#[test]
fn test_priority_execution_order() {
    let source = "
    .org 0x200
    LDI r1, 0x1500
    LDI r2, 3
    STORE r1, r2
    HALT

    .org 0x300
    LDI r1, 0x1501
    LDI r2, 2
    STORE r1, r2
    HALT

    .org 0x400
    LDI r1, 0x1502
    LDI r2, 1
    STORE r1, r2
    HALT
    ";
    let asm = assemble(source, 0).expect("assembly should succeed");
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() { vm.ram[i] = v; }
    vm.processes.push(geometry_os::vm::SpawnedProcess {
        pc: 0x200, regs: [0; 32], state: geometry_os::vm::ProcessState::Ready, pid: 3,
        mode: geometry_os::vm::CpuMode::Kernel, page_dir: None,
        segfaulted: false, priority: 2, slice_remaining: 0,
        sleep_until: 0, yielded: false,
        kernel_stack: Vec::new(), msg_queue: Vec::new(),
                                exit_code: 0,
                                parent_pid: 0,
                                pending_signals: Vec::new(),
                                signal_handlers: [0; 4], vmas: Vec::new(), brk_pos: 0,
    });
    vm.processes.push(geometry_os::vm::SpawnedProcess {
        pc: 0x300, regs: [0; 32], state: geometry_os::vm::ProcessState::Ready, pid: 2,
        mode: geometry_os::vm::CpuMode::Kernel, page_dir: None,
        segfaulted: false, priority: 1, slice_remaining: 0,
        sleep_until: 0, yielded: false,
        kernel_stack: Vec::new(), msg_queue: Vec::new(),
                                exit_code: 0,
                                parent_pid: 0,
                                pending_signals: Vec::new(),
                                signal_handlers: [0; 4], vmas: Vec::new(), brk_pos: 0,
    });
    vm.processes.push(geometry_os::vm::SpawnedProcess {
        pc: 0x400, regs: [0; 32], state: geometry_os::vm::ProcessState::Ready, pid: 1,
        mode: geometry_os::vm::CpuMode::Kernel, page_dir: None,
        segfaulted: false, priority: 0, slice_remaining: 0,
        sleep_until: 0, yielded: false,
        kernel_stack: Vec::new(), msg_queue: Vec::new(),
                                exit_code: 0,
                                parent_pid: 0,
                                pending_signals: Vec::new(),
                                signal_handlers: [0; 4], vmas: Vec::new(), brk_pos: 0,
    });
    for _ in 0..50 {
        vm.step_all_processes();
        if vm.processes.iter().all(|p| p.is_halted()) { break; }
    }
    assert!(vm.processes.iter().all(|p| p.is_halted()), "all processes should halt");
    assert_eq!(vm.ram[0x1500], 3, "priority-2 process should have written PID 3");
    assert_eq!(vm.ram[0x1501], 2, "priority-1 process should have written PID 2");
    assert_eq!(vm.ram[0x1502], 1, "priority-0 process should have written PID 1");
}

#[test]
fn test_priority_higher_gets_more_instructions() {
    // Two processes with different priorities running counting loops.
    // With round-based scheduling: priority 0 gets 100 instructions per round,
    // priority 3 gets 800 instructions per round. Over 200 rounds the
    // priority-3 process should execute significantly more instructions.
    let source = "
    .org 0x200
    LDI r1, 0x1200
    LOAD r2, r1
    LDI r3, 1
    ADD r2, r3
    STORE r1, r2
    JMP 0x200

    .org 0x300
    LDI r1, 0x1300
    LOAD r2, r1
    LDI r3, 1
    ADD r2, r3
    STORE r1, r2
    JMP 0x300
    ";
    let asm = assemble(source, 0).expect("assembly should succeed");
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() {
        vm.ram[i] = v;
    }
    // Process A: priority 3 (high) -- gets 800 instructions per round
    vm.processes.push(geometry_os::vm::SpawnedProcess {
        pc: 0x200, regs: [0; 32], state: geometry_os::vm::ProcessState::Ready, pid: 1,
        mode: geometry_os::vm::CpuMode::Kernel, page_dir: None,
        segfaulted: false, priority: 3, slice_remaining: 0,
        sleep_until: 0, yielded: false,
        kernel_stack: Vec::new(), msg_queue: Vec::new(),
                                exit_code: 0,
                                parent_pid: 0,
                                pending_signals: Vec::new(),
                                signal_handlers: [0; 4], vmas: Vec::new(), brk_pos: 0,
    });
    // Process B: priority 0 (low) -- gets 100 instructions per round
    vm.processes.push(geometry_os::vm::SpawnedProcess {
        pc: 0x300, regs: [0; 32], state: geometry_os::vm::ProcessState::Ready, pid: 2,
        mode: geometry_os::vm::CpuMode::Kernel, page_dir: None,
        segfaulted: false, priority: 0, slice_remaining: 0,
        sleep_until: 0, yielded: false,
        kernel_stack: Vec::new(), msg_queue: Vec::new(),
                                exit_code: 0,
                                parent_pid: 0,
                                pending_signals: Vec::new(),
                                signal_handlers: [0; 4], vmas: Vec::new(), brk_pos: 0,
    });
    // Run enough rounds for priority-0 to exhaust twice (200 calls)
    for _ in 0..200 {
        vm.step_all_processes();
    }
    let count_a = vm.ram[0x1200];
    let count_b = vm.ram[0x1300];
    assert!(count_a > 0, "high-priority process should have run");
    assert!(count_a > count_b,
        "high-priority (count={}, pri=3) should exceed low-priority (count={}, pri=0)",
        count_a,
        count_b
    );
}
