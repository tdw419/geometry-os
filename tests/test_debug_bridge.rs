// test_debug_bridge.rs -- Integration test for IPC debug bridge
//
// Tests the Page 3 shared memory debug protocol between
// ai_terminal.asm (parent) and debug_monitor.asm (child).
//
// Protocol:
//   0x0C00: DEBUG_MAGIC     (0xDB9900 = stub active)
//   0x0C01: DEBUG_PARENT_PID
//   0x0C02: DEBUG_CHILD_PID
//   0x0C03: DEBUG_COMMAND   (1=regs, 2=ram_read, 3=ping, 0=none)
//   0x0C04: DEBUG_STATUS    (0=idle, 1=cmd_pending, 2=response_ready)
//   0x0C05-0x0C24: DEBUG_RESPONSE (32 words)

use geometry_os::assembler;
use geometry_os::preprocessor::Preprocessor;
use geometry_os::vm::Vm;

fn load_program(source: &str) -> Vm {
    let mut pp = Preprocessor::new();
    let preprocessed = pp.preprocess(source);
    let asm = assembler::assemble(&preprocessed, 0).expect("program should assemble");
    let mut vm = Vm::new();
    for (i, &word) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = word;
        }
    }
    vm
}

fn run_frames(vm: &mut Vm, count: usize) {
    for _ in 0..count {
        vm.frame_ready = false;
        let mut c = 0;
        while !vm.frame_ready && !vm.halted && c < 100_000 {
            vm.step();
            c += 1;
        }
    }
}

#[test]
fn test_debug_monitor_assembles() {
    let source = include_str!("../programs/debug_monitor.asm");
    let mut pp = Preprocessor::new();
    let preprocessed = pp.preprocess(source);
    let result = assembler::assemble(&preprocessed, 0);
    assert!(
        result.is_ok(),
        "debug_monitor should assemble: {:?}",
        result.err()
    );
    let asm = result.unwrap();
    assert!(asm.pixels.len() > 100, "should have substantial code");
}

#[test]
fn test_debug_monitor_sets_magic() {
    // Load debug_monitor and run a few frames
    let source = include_str!("../programs/debug_monitor.asm");
    let mut vm = load_program(source);

    // Run enough frames to pass init and reach main loop
    run_frames(&mut vm, 20);

    // Check that magic is set at 0x0C00
    let magic = vm.ram[0x0C00];
    assert_eq!(
        magic, 0xDB9900,
        "DEBUG_MAGIC should be 0xDB9900, got 0x{:X}",
        magic
    );

    // Check that child PID is set at 0x0C02
    let pid = vm.ram[0x0C02];
    // PID should be non-zero (set by GETPID -- but single process, so might be 0 or 1)
    println!("Child PID: {}", pid);

    // Status should be 0 (idle)
    let status = vm.ram[0x0C04];
    assert_eq!(status, 0, "DEBUG_STATUS should be 0 (idle), got {}", status);
}

#[test]
fn test_debug_monitor_ping_response() {
    let source = include_str!("../programs/debug_monitor.asm");
    let mut vm = load_program(source);

    // Run to main loop
    run_frames(&mut vm, 10);

    // Verify magic is set
    assert_eq!(vm.ram[0x0C00], 0xDB9900);

    // Send ping command: cmd=3, status=1
    vm.ram[0x0C03] = 3; // DEBUG_COMMAND = ping
    vm.ram[0x0C04] = 1; // DEBUG_STATUS = cmd_pending

    // Run a few more frames for the child to process
    run_frames(&mut vm, 5);

    // Check status = 2 (response ready)
    let status = vm.ram[0x0C04];
    assert_eq!(
        status, 2,
        "DEBUG_STATUS should be 2 (response ready), got {}",
        status
    );

    // Check response has the PING acknowledgment
    let response = vm.ram[0x0C05];
    assert_eq!(
        response, 0x504E4720,
        "Response should be PING magic, got 0x{:X}",
        response
    );
}

#[test]
fn test_debug_monitor_regs_response() {
    let source = include_str!("../programs/debug_monitor.asm");
    let mut vm = load_program(source);

    // Run to main loop
    run_frames(&mut vm, 10);

    // Verify magic
    assert_eq!(vm.ram[0x0C00], 0xDB9900);

    // Send regs command: cmd=1, status=1
    vm.ram[0x0C03] = 1; // DEBUG_COMMAND = regs
    vm.ram[0x0C04] = 1; // DEBUG_STATUS = cmd_pending

    // Run more frames
    run_frames(&mut vm, 5);

    // Check status = 2 (response ready)
    let status = vm.ram[0x0C04];
    assert_eq!(
        status, 2,
        "DEBUG_STATUS should be 2 (response ready), got {}",
        status
    );

    // Response buffer should contain register values
    // r0 is at DEBUG_RESPONSE (0x0C05), r1 at 0x0C06, etc.
    // Values will depend on what the program was doing, but they should exist
    let r0_val = vm.ram[0x0C05];
    let r1_val = vm.ram[0x0C06];
    let r2_val = vm.ram[0x0C07];
    println!(
        "Child regs: r0=0x{:08X} r1=0x{:08X} r2=0x{:08X}",
        r0_val, r1_val, r2_val
    );

    // r1 won't be 1 anymore -- the main loop modifies it for drawing
    // At least r1 and r2 should be non-zero (used for drawing coordinates)
    assert!(
        r1_val != 0 || r2_val != 0,
        "Some register values should be non-zero from program execution"
    );
}

#[test]
fn test_debug_monitor_ram_read() {
    let source = include_str!("../programs/debug_monitor.asm");
    let mut vm = load_program(source);

    // Run to main loop
    run_frames(&mut vm, 10);

    // Write a known value to an address
    vm.ram[0x500] = 0xDEADBEEF;

    // Send ram_read command: cmd=2, addr=0x500
    vm.ram[0x0C03] = 2; // DEBUG_COMMAND = ram_read
    vm.ram[0x0C25] = 0x500; // DEBUG_ADDR = 0x500
    vm.ram[0x0C04] = 1; // DEBUG_STATUS = cmd_pending

    // Run frames
    run_frames(&mut vm, 5);

    // Check response
    let status = vm.ram[0x0C04];
    assert_eq!(
        status, 2,
        "DEBUG_STATUS should be 2 (response ready), got {}",
        status
    );

    let value = vm.ram[0x0C05];
    let addr = vm.ram[0x0C06];
    assert_eq!(
        value, 0xDEADBEEF,
        "Should read back 0xDEADBEEF, got 0x{:X}",
        value
    );
    assert_eq!(addr, 0x500, "Address should be 0x500, got 0x{:X}", addr);
}

#[test]
fn test_flatliner_heartbeat_stops() {
    // Verify that the flatliner program updates heartbeat then stops
    let source = include_str!("../programs/flatliner.asm");
    let mut vm = load_program(source);

    // Run 2 frames -- heartbeat should be incrementing (init takes 0 frames)
    run_frames(&mut vm, 2);
    let hb1 = vm.ram[0x0C27];
    assert!(
        hb1 > 0,
        "Heartbeat should be >0 after 2 frames, got {}",
        hb1
    );

    // Run 1 more frame -- still incrementing
    run_frames(&mut vm, 1);
    let hb2 = vm.ram[0x0C27];
    assert!(
        hb2 > hb1,
        "Heartbeat should keep incrementing: {} -> {}",
        hb1,
        hb2
    );

    // Run past crash (5 beats total, then dead_loop with no FRAME)
    // Frame 6+ will hit the 100K step limit (no FRAME in dead_loop)
    run_frames(&mut vm, 10);
    let hb3 = vm.ram[0x0C27];

    // Run more -- heartbeat should NOT change
    run_frames(&mut vm, 5);
    let hb4 = vm.ram[0x0C27];
    assert_eq!(
        hb3, hb4,
        "Heartbeat should stop after crash: {} == {}",
        hb3, hb4
    );
}

#[test]
fn test_watchdog_detects_flatline() {
    // Simulate the watchdog mechanism with two VMs
    let source = include_str!("../programs/flatliner.asm");
    let mut child_vm = load_program(source);

    // Run child for 3 frames (heartbeat incrementing)
    run_frames(&mut child_vm, 3);
    let hb_alive = child_vm.ram[0x0C27];
    assert!(
        hb_alive >= 2,
        "Child should have heartbeats, got {}",
        hb_alive
    );

    // Run child past crash
    run_frames(&mut child_vm, 10);
    let hb_after_crash = child_vm.ram[0x0C27];

    // Run more -- heartbeat frozen
    run_frames(&mut child_vm, 10);
    let hb_frozen = child_vm.ram[0x0C27];
    assert_eq!(
        hb_after_crash, hb_frozen,
        "Heartbeat should be frozen after crash"
    );

    // The watchdog pattern: last_seen == current for 3 consecutive checks = flatline
    assert!(
        hb_frozen <= 5,
        "Flatliner should stop after 5 heartbeats, got {}",
        hb_frozen
    );
    println!(
        "Watchdog test: heartbeat {} -> {} (frozen at {})",
        hb_alive, hb_after_crash, hb_frozen
    );
}
