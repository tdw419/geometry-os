use super::*;

// ============================================================
// Phase 27: IPC Tests
// ============================================================

#[test]
fn test_pipe_create_opcode() {
    // PIPE r5, r6 -- create pipe, r5=read_fd, r6=write_fd
    let source = "
    PIPE r5, r6
    HALT
    ";
    let asm = assemble(source, 0).expect("assembly should succeed");
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() {
        vm.ram[i] = v;
    }
    vm.mode = geometry_os::vm::CpuMode::Kernel;
    vm.step(); // PIPE
    assert_eq!(vm.regs[5], 0x8000, "read fd should be 0x8000");
    assert_eq!(vm.regs[6], 0xC000, "write fd should be 0xC000");
    assert_eq!(vm.pipes.len(), 1, "should have 1 pipe");
    assert_eq!(vm.regs[0], 0, "r0 should be 0 (success)");
}

#[test]
fn test_pipe_create_multiple() {
    let source = "
    PIPE r5, r6
    PIPE r7, r8
    HALT
    ";
    let asm = assemble(source, 0).expect("assembly should succeed");
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() {
        vm.ram[i] = v;
    }
    vm.mode = geometry_os::vm::CpuMode::Kernel;
    vm.step(); // first PIPE
    vm.step(); // second PIPE
    assert_eq!(vm.pipes.len(), 2, "should have 2 pipes");
    assert_eq!(vm.regs[5], 0x8000, "first read fd");
    assert_eq!(vm.regs[6], 0xC000, "first write fd");
    assert_eq!(vm.regs[7], 0x8001, "second read fd");
    assert_eq!(vm.regs[8], 0xC001, "second write fd");
}

#[test]
fn test_pipe_write_and_read() {
    // Create a pipe, write words via WRITE opcode, then read via READ opcode
    let mut vm = Vm::new();
    vm.mode = geometry_os::vm::CpuMode::Kernel;

    // Create pipe via PIPE opcode
    // PIPE r5, r6 => [0x5D, 5, 6]
    vm.ram[0] = 0x5D; // PIPE
    vm.ram[1] = 5; // rd_read
    vm.ram[2] = 6; // rd_write

    // HALT
    vm.ram[3] = 0x00;

    vm.step(); // PIPE
    assert_eq!(vm.regs[5], 0x8000, "read fd");
    assert_eq!(vm.regs[6], 0xC000, "write fd");

    // Write data to RAM buffer
    vm.ram[0x1000] = 42;
    vm.ram[0x1001] = 99;
    vm.ram[0x1002] = 7;

    // WRITE r6, r10, r11 -- write from buf to pipe
    // r6=write_fd(0xC000), r10=0x1000, r11=3
    vm.regs[10] = 0x1000;
    vm.regs[11] = 3;
    vm.pc = 10;
    vm.ram[10] = 0x56; // WRITE
    vm.ram[11] = 6; // fd_reg
    vm.ram[12] = 10; // buf_reg
    vm.ram[13] = 11; // len_reg
    vm.ram[14] = 0x00; // HALT
    vm.halted = false;
    vm.step(); // WRITE
    assert_eq!(vm.regs[0], 3, "should write 3 words");

    // READ r5, r12, r11 -- read from pipe into buffer
    // r5=read_fd(0x8000), r12=0x1100, r11=3
    vm.regs[12] = 0x1100;
    vm.pc = 20;
    vm.ram[20] = 0x55; // READ
    vm.ram[21] = 5; // fd_reg
    vm.ram[22] = 12; // buf_reg
    vm.ram[23] = 11; // len_reg
    vm.ram[24] = 0x00; // HALT
    vm.halted = false;
    vm.step(); // READ
    assert_eq!(vm.regs[0], 3, "should read 3 words");

    // Verify data read back
    assert_eq!(vm.ram[0x1100], 42, "first word");
    assert_eq!(vm.ram[0x1101], 99, "second word");
    assert_eq!(vm.ram[0x1102], 7, "third word");
}

#[test]
fn test_pipe_circular_buffer() {
    // Test the Pipe struct directly
    let mut pipe = geometry_os::vm::Pipe::new(0, 1);
    assert!(pipe.is_empty());
    assert!(!pipe.is_full());

    // Write and read
    assert!(pipe.write_word(42));
    assert!(pipe.write_word(99));
    assert_eq!(pipe.count, 2);
    assert_eq!(pipe.read_word(), Some(42));
    assert_eq!(pipe.read_word(), Some(99));
    assert_eq!(pipe.read_word(), None); // empty
    assert!(pipe.is_empty());
}

#[test]
fn test_pipe_full() {
    let mut pipe = geometry_os::vm::Pipe::new(0, 1);
    // Fill the pipe (256 words)
    for i in 0..256 {
        assert!(pipe.write_word(i as u32), "write {} should succeed", i);
    }
    assert!(pipe.is_full());
    assert!(!pipe.write_word(999), "write to full pipe should fail");
    // Read one and write should succeed again
    pipe.read_word();
    assert!(pipe.write_word(999), "write after read should succeed");
}

#[test]
fn test_msgsnd_delivers_to_target() {
    // Main process sends a message to child PID 1
    // Use inline bytecode: MSGSND r5 (where r5=1, target PID)
    // r1..r4 contain the message data
    let mut vm = Vm::new();
    vm.mode = geometry_os::vm::CpuMode::Kernel;

    // Set up child process
    vm.processes.push(geometry_os::vm::SpawnedProcess {
        pc: 0x200,
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
        signal_handlers: [0; 4],
        vmas: Vec::new(),
        brk_pos: 0,
    });

    // Set up message data in main process registers
    vm.regs[1] = 0xDEAD;
    vm.regs[2] = 0xBEEF;
    vm.regs[3] = 0xCAFE;
    vm.regs[4] = 0xF00D;
    vm.regs[5] = 1; // target PID

    // Create MSGSND bytecode: 0x5E, r5
    vm.ram[0] = 0x5E; // MSGSND
    vm.ram[1] = 5; // pid_reg = r5
    vm.ram[2] = 0x00; // HALT

    vm.step(); // MSGSND
    assert_eq!(vm.regs[0], 0, "MSGSND should succeed");
    assert_eq!(
        vm.processes[0].msg_queue.len(),
        1,
        "child should have 1 message"
    );

    let msg = &vm.processes[0].msg_queue[0];
    assert_eq!(msg.sender, 0, "sender should be PID 0 (main)");
    assert_eq!(msg.data[0], 0xDEAD);
    assert_eq!(msg.data[1], 0xBEEF);
    assert_eq!(msg.data[2], 0xCAFE);
    assert_eq!(msg.data[3], 0xF00D);
}

#[test]
fn test_msgsnd_to_nonexistent_pid_fails() {
    let mut vm = Vm::new();
    vm.mode = geometry_os::vm::CpuMode::Kernel;
    vm.regs[5] = 99; // nonexistent PID
    vm.ram[0] = 0x5E; // MSGSND
    vm.ram[1] = 5; // pid_reg
    vm.step();
    assert_eq!(
        vm.regs[0], 0xFFFFFFFF,
        "MSGSND to nonexistent PID should fail"
    );
}

#[test]
fn test_msgrcv_receives_message() {
    // Child process with a pending message receives it
    let mut vm = Vm::new();
    vm.mode = geometry_os::vm::CpuMode::Kernel;
    vm.current_pid = 1; // running as child

    // Set up child with a pending message
    let msg = geometry_os::vm::Message::new(0, [100, 200, 300, 400]);
    vm.processes.push(geometry_os::vm::SpawnedProcess {
        pc: 0x200,
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
        msg_queue: vec![msg],
        exit_code: 0,
        parent_pid: 0,
        pending_signals: Vec::new(),
        signal_handlers: [0; 4],
        vmas: Vec::new(),
        brk_pos: 0,
    });

    // MSGRCV bytecode: 0x5F (no args)
    vm.ram[0x200] = 0x5F; // MSGRCV
    vm.pc = 0x200;

    vm.step(); // MSGRCV
    assert_eq!(vm.regs[0], 0, "sender should be PID 0");
    assert_eq!(vm.regs[1], 100);
    assert_eq!(vm.regs[2], 200);
    assert_eq!(vm.regs[3], 300);
    assert_eq!(vm.regs[4], 400);
    assert_eq!(
        vm.processes[0].msg_queue.len(),
        0,
        "message should be consumed"
    );
}

#[test]
fn test_msgrcv_blocks_when_empty() {
    // Child process with no messages should block
    let mut vm = Vm::new();
    vm.mode = geometry_os::vm::CpuMode::Kernel;
    vm.current_pid = 1;

    vm.processes.push(geometry_os::vm::SpawnedProcess {
        pc: 0x200,
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
        signal_handlers: [0; 4],
        vmas: Vec::new(),
        brk_pos: 0,
    });

    vm.ram[0x200] = 0x5F; // MSGRCV
    vm.pc = 0x200;
    vm.step(); // MSGRCV -- should block
    assert!(
        vm.processes[0].state == geometry_os::vm::ProcessState::Blocked,
        "process should be blocked"
    );
    // PC should be rewound so it retries
    assert_eq!(vm.pc, 0x200, "PC should be rewound to retry MSGRCV");
}

#[test]
fn test_msgrcv_unblocks_on_msgsnd() {
    // Process A blocks on MSGRCV, then process B sends it a message
    let mut vm = Vm::new();
    vm.mode = geometry_os::vm::CpuMode::Kernel;
    vm.current_pid = 1;

    // Process A (PID 1): blocked on MSGRCV
    vm.processes.push(geometry_os::vm::SpawnedProcess {
        pc: 0x200,
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
        signal_handlers: [0; 4],
        vmas: Vec::new(),
        brk_pos: 0,
    });

    // First block process A
    vm.ram[0x200] = 0x5F; // MSGRCV
    vm.pc = 0x200;
    vm.step();
    assert!(vm.processes[0].state == geometry_os::vm::ProcessState::Blocked);

    // Now switch to main process (PID 0) and send a message
    vm.current_pid = 0;
    vm.pc = 0;
    vm.regs[1] = 111;
    vm.regs[2] = 222;
    vm.regs[3] = 333;
    vm.regs[4] = 444;
    vm.regs[5] = 1; // target PID
    vm.ram[0] = 0x5E; // MSGSND
    vm.ram[1] = 5;
    vm.step(); // MSGSND
    assert_eq!(vm.regs[0], 0, "MSGSND should succeed");
    assert!(
        vm.processes[0].state != geometry_os::vm::ProcessState::Blocked,
        "target should be unblocked after MSGSND"
    );
    assert_eq!(
        vm.processes[0].msg_queue.len(),
        1,
        "message should be in queue"
    );
}

#[test]
fn test_blocked_process_skipped_by_scheduler() {
    // Blocked processes should not get CPU time
    let source = "
    .org 0x200
    LDI r1, 0x1000
    LOAD r2, r1
    LDI r3, 1
    ADD r2, r3
    STORE r1, r2
    JMP 0x200
    ";
    let asm = assemble(source, 0).expect("assembly should succeed");
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() {
        vm.ram[i] = v;
    }
    // Create a blocked process
    vm.processes.push(geometry_os::vm::SpawnedProcess {
        pc: 0x200,
        regs: [0; 32],
        state: geometry_os::vm::ProcessState::Blocked,
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
        signal_handlers: [0; 4],
        vmas: Vec::new(),
        brk_pos: 0,
    });
    // Run scheduler 50 times -- blocked process should not execute
    for _ in 0..50 {
        vm.step_all_processes();
    }
    assert_eq!(vm.ram[0x1000], 0, "blocked process should not have run");
    assert!(
        vm.processes[0].state == geometry_os::vm::ProcessState::Blocked,
        "process should still be blocked"
    );
}

#[test]
fn test_pipe_write_unblocks_blocked_reader() {
    // Process A writes to pipe, Process B is blocked reading from it
    let mut vm = Vm::new();
    vm.mode = geometry_os::vm::CpuMode::Kernel;

    // Create a pipe (read_fd=0x8000, write_fd=0xC000)
    vm.pipes.push(geometry_os::vm::Pipe::new(1, 0));
    vm.pipes[0].alive = true;

    // Process B (PID 1): blocked reading from pipe
    vm.processes.push(geometry_os::vm::SpawnedProcess {
        pc: 0x200,
        regs: [0; 32],
        state: geometry_os::vm::ProcessState::Blocked,
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
        signal_handlers: [0; 4],
        vmas: Vec::new(),
        brk_pos: 0,
    });

    // Main process writes to the pipe
    vm.regs[5] = 0xC000; // write_fd
    vm.ram[0x1000] = 42; // data
    vm.regs[6] = 0x1000; // buf addr
    vm.regs[7] = 1; // len
    vm.ram[0] = 0x56; // WRITE
    vm.ram[1] = 5; // fd_reg
    vm.ram[2] = 6; // buf_reg
    vm.ram[3] = 7; // len_reg

    vm.step(); // WRITE

    assert_eq!(vm.regs[0], 1, "should have written 1 word");
    assert!(
        vm.processes[0].state != geometry_os::vm::ProcessState::Blocked,
        "reader should be unblocked after write"
    );
    assert_eq!(vm.pipes[0].count, 1, "pipe should have 1 word");
    assert_eq!(vm.pipes[0].buffer[0], 42, "pipe should contain 42");
}

#[test]
fn test_pipe_assembles() {
    let source = "
    PIPE r5, r6
    MSGSND r7
    MSGRCV
    HALT
    ";
    let result = assemble(source, 0);
    assert!(result.is_ok(), "PIPE/MSGSND/MSGRCV should assemble");
    let bc = &result.expect("operation should succeed").pixels;
    assert_eq!(bc[0], 0x5D, "PIPE opcode");
    assert_eq!(bc[1], 5, "PIPE read reg");
    assert_eq!(bc[2], 6, "PIPE write reg");
    assert_eq!(bc[3], 0x5E, "MSGSND opcode");
    assert_eq!(bc[4], 7, "MSGSND pid reg");
    assert_eq!(bc[5], 0x5F, "MSGRCV opcode");
    assert_eq!(bc[6], 0x00, "HALT");
}

#[test]
fn test_pipe_close_marks_dead() {
    // CLOSE on a pipe fd should mark the pipe as dead
    let source = "
    PIPE r5, r6
    CLOSE r5
    HALT
    ";
    let asm = assemble(source, 0).expect("assembly should succeed");
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() {
        vm.ram[i] = v;
    }
    vm.mode = geometry_os::vm::CpuMode::Kernel;
    vm.step(); // PIPE
    assert_eq!(vm.pipes.len(), 1);
    assert!(vm.pipes[0].alive, "pipe should be alive after creation");
    vm.step(); // CLOSE r5 (read fd)
    assert!(!vm.pipes[0].alive, "pipe should be dead after close");
    assert_eq!(vm.regs[0], 0, "CLOSE should return 0 (success)");
}

#[test]
fn test_read_from_closed_pipe_returns_error() {
    // Reading from a closed pipe should return 0xFFFFFFFF
    let mut vm = Vm::new();
    vm.mode = geometry_os::vm::CpuMode::Kernel;
    vm.pipes.push(geometry_os::vm::Pipe::new(0, 0));
    vm.pipes[0].alive = false; // pre-close the pipe
    vm.regs[5] = 0x8000; // read fd for pipe 0
    vm.regs[6] = 0x2000; // buf addr
    vm.regs[7] = 1; // len
    vm.ram[0] = 0x55; // READ opcode
    vm.ram[1] = 5; // fd_reg
    vm.ram[2] = 6; // buf_reg
    vm.ram[3] = 7; // len_reg
    vm.step(); // READ
    assert_eq!(vm.regs[0], 0xFFFFFFFF, "read from closed pipe should fail");
}
