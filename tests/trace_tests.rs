// Phase 38a: Execution Trace Ring Buffer tests
// Tests for TraceBuffer, TraceEntry, TraceIter, and SNAP_TRACE opcode (0x7B)

use geometry_os::assembler::assemble;
use geometry_os::vm::{TraceBuffer, TraceEntry, Vm};

/// Helper: assemble and load bytecode into VM ram at base address, set pc.
fn load_asm(vm: &mut Vm, source: &str, base: usize) {
    let result = assemble(source, base).expect("assemble failed");
    for (i, &word) in result.pixels.iter().enumerate() {
        vm.ram[base + i] = word;
    }
    vm.pc = base as u32;
}

#[test]
fn test_trace_buffer_new() {
    let buf = TraceBuffer::new(100);
    assert_eq!(buf.len(), 0);
    assert!(buf.is_empty());
    assert_eq!(buf.step_counter(), 0);
}

#[test]
fn test_trace_buffer_push_single() {
    let mut buf = TraceBuffer::new(100);
    let regs = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16,
                17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32];
    buf.push(0x100, &regs, 0x01);

    assert_eq!(buf.len(), 1);
    assert!(!buf.is_empty());
    assert_eq!(buf.step_counter(), 1);

    let entry = buf.get_recent(0).unwrap();
    assert_eq!(entry.step_number, 0);
    assert_eq!(entry.pc, 0x100);
    assert_eq!(entry.opcode, 0x01);
    assert_eq!(entry.regs[0], 1);
    assert_eq!(entry.regs[15], 16);
    // Only first 16 registers stored
    assert_eq!(entry.regs.len(), 16);
}

#[test]
fn test_trace_buffer_push_multiple() {
    let mut buf = TraceBuffer::new(100);
    let regs = [0u32; 32];

    for i in 0..5 {
        buf.push(0x100 + i, &regs, 0x01);
    }

    assert_eq!(buf.len(), 5);
    assert_eq!(buf.step_counter(), 5);

    // get_recent(0) = newest
    assert_eq!(buf.get_recent(0).unwrap().pc, 0x104);
    // get_recent(4) = oldest
    assert_eq!(buf.get_recent(4).unwrap().pc, 0x100);
    // Out of bounds
    assert!(buf.get_recent(5).is_none());
}

#[test]
fn test_trace_buffer_wrap_around() {
    let capacity = 5;
    let mut buf = TraceBuffer::new(capacity);
    let regs = [0u32; 32];

    // Push 8 entries into a 5-capacity buffer
    for i in 0..8 {
        buf.push(i as u32, &regs, 0x01);
    }

    assert_eq!(buf.len(), 5); // capped at capacity
    assert_eq!(buf.step_counter(), 8);

    // Should have entries 3,4,5,6,7 (oldest 0,1,2 were overwritten)
    let recent: Vec<u32> = (0..5).map(|i| buf.get_recent(i).unwrap().pc).collect();
    assert_eq!(recent, vec![7, 6, 5, 4, 3]);

    // Iter should go oldest to newest: 3,4,5,6,7
    let iter_pcs: Vec<u32> = buf.iter().map(|e| e.pc).collect();
    assert_eq!(iter_pcs, vec![3, 4, 5, 6, 7]);
}

#[test]
fn test_trace_buffer_clear() {
    let mut buf = TraceBuffer::new(100);
    let regs = [0u32; 32];

    for i in 0..10 {
        buf.push(i as u32, &regs, 0x01);
    }
    assert_eq!(buf.len(), 10);
    assert_eq!(buf.step_counter(), 10);

    buf.clear();
    assert_eq!(buf.len(), 0);
    assert!(buf.is_empty());
    assert_eq!(buf.step_counter(), 0);
}

#[test]
fn test_trace_buffer_iter_before_wrap() {
    let mut buf = TraceBuffer::new(10);
    let regs = [0u32; 32];

    for i in 0..3 {
        buf.push(i as u32, &regs, 0x01);
    }

    let entries: Vec<u32> = buf.iter().map(|e| e.pc).collect();
    assert_eq!(entries, vec![0, 1, 2]);
}

#[test]
fn test_trace_buffer_iter_after_wrap() {
    let mut buf = TraceBuffer::new(3);
    let regs = [0u32; 32];

    for i in 0..6 {
        buf.push(i as u32, &regs, 0x01);
    }

    // Buffer has entries 3,4,5
    let entries: Vec<u32> = buf.iter().map(|e| e.pc).collect();
    assert_eq!(entries, vec![3, 4, 5]);
}

#[test]
fn test_trace_buffer_step_numbers_monotonic() {
    let mut buf = TraceBuffer::new(10);
    let regs = [0u32; 32];

    for i in 0..15 {
        buf.push(i as u32, &regs, 0x01);
    }

    // After wrapping, step numbers should still be monotonic
    let entries: Vec<u64> = buf.iter().map(|e| e.step_number).collect();
    for window in entries.windows(2) {
        assert!(window[1] > window[0], "step numbers must be monotonic");
    }
    // Last 10 entries should have step_number 5..=14
    assert_eq!(entries, vec![5, 6, 7, 8, 9, 10, 11, 12, 13, 14]);
}

#[test]
fn test_trace_buffer_minimum_capacity() {
    let buf = TraceBuffer::new(0);
    assert_eq!(buf.len(), 0);

    // Capacity should be clamped to 1
    let mut buf = TraceBuffer::new(0);
    let regs = [0u32; 32];
    buf.push(42, &regs, 0x01);
    assert_eq!(buf.len(), 1);
    assert_eq!(buf.get_recent(0).unwrap().pc, 42);
}

#[test]
fn test_trace_entry_equality() {
    let regs = [1u32; 16];
    let a = TraceEntry { step_number: 5, pc: 100, regs, opcode: 0x01 };
    let b = TraceEntry { step_number: 5, pc: 100, regs: [1u32; 16], opcode: 0x01 };
    assert_eq!(a, b);
}

#[test]
fn test_trace_entry_inequality() {
    let regs = [1u32; 16];
    let a = TraceEntry { step_number: 5, pc: 100, regs, opcode: 0x01 };
    let b = TraceEntry { step_number: 6, pc: 100, regs: [1u32; 16], opcode: 0x01 };
    assert_ne!(a, b);
}

// --- SNAP_TRACE opcode (0x7B) integration tests ---

#[test]
fn test_snap_trace_start_recording() {
    let mut vm = Vm::new();
    load_asm(&mut vm, "
        LDI r1, 1
        SNAP_TRACE r1
        LDI r2, 42
        LDI r3, 99
        HALT
    ", 0x100);

    // Run until halted
    loop {
        if !vm.step() { break; }
    }

    // Trace should have recorded LDI r2 and LDI r3 (2 instructions after SNAP_TRACE)
    assert!(vm.trace_buffer.len() >= 2,
        "should have at least 2 traced instructions, got {}", vm.trace_buffer.len());
    assert!(vm.trace_recording, "recording should be on after SNAP_TRACE 1");
}

#[test]
fn test_snap_trace_stop_recording() {
    let mut vm = Vm::new();
    load_asm(&mut vm, "
        LDI r1, 1
        SNAP_TRACE r1
        LDI r2, 42
        LDI r1, 0
        SNAP_TRACE r1
        LDI r3, 99
        HALT
    ", 0x100);

    loop {
        if !vm.step() { break; }
    }

    let len_after_stop = vm.trace_buffer.len();
    // LDI r3 after stop should NOT be recorded
    // Only LDI r2 should be recorded (between start and stop)
    assert!(len_after_stop >= 1, "should have at least 1 traced instruction");
    assert!(!vm.trace_recording, "recording should be off");
}

#[test]
fn test_snap_trace_snapshot_and_clear() {
    let mut vm = Vm::new();
    load_asm(&mut vm, "
        LDI r1, 1
        SNAP_TRACE r1
        LDI r2, 10
        LDI r3, 20
        LDI r4, 30
        LDI r1, 2
        SNAP_TRACE r1
        HALT
    ", 0x100);

    loop {
        if !vm.step() { break; }
    }

    // r0 should hold the count of entries captured before clear
    assert!(vm.regs[0] >= 3,
        "r0 should have entry count (>=3), got {}", vm.regs[0]);
    // Buffer should be cleared
    assert_eq!(vm.trace_buffer.len(), 0, "buffer should be empty after clear");
    assert!(!vm.trace_recording, "recording should be off after snapshot-clear");
}

#[test]
fn test_snap_trace_invalid_mode() {
    let mut vm = Vm::new();
    load_asm(&mut vm, "
        LDI r1, 99
        SNAP_TRACE r1
        HALT
    ", 0x100);

    loop {
        if !vm.step() { break; }
    }

    // r0 should be 0xFFFFFFFF for invalid mode
    assert_eq!(vm.regs[0], 0xFFFFFFFF, "invalid mode should return 0xFFFFFFFF");
}

#[test]
fn test_snap_trace_returns_entry_count() {
    let mut vm = Vm::new();
    load_asm(&mut vm, "
        LDI r1, 1
        SNAP_TRACE r1
        LDI r2, 1
        LDI r3, 2
        LDI r4, 3
        LDI r1, 0
        SNAP_TRACE r1
        HALT
    ", 0x100);

    loop {
        if !vm.step() { break; }
    }

    // Second SNAP_TRACE (mode 0) returns count in r0
    assert!(vm.regs[0] >= 3,
        "r0 should have count of traced entries (>=3), got {}", vm.regs[0]);
}

#[test]
fn test_trace_disabled_by_default() {
    let mut vm = Vm::new();
    load_asm(&mut vm, "
        LDI r1, 42
        LDI r2, 99
        HALT
    ", 0x100);

    loop {
        if !vm.step() { break; }
    }

    assert_eq!(vm.trace_buffer.len(), 0, "no entries when trace is disabled");
    assert!(!vm.trace_recording);
}

#[test]
fn test_trace_buffer_get_recent_order() {
    let mut buf = TraceBuffer::new(100);
    let regs = [0u32; 32];

    // Push 5 entries with increasing PCs
    for i in 0..5u32 {
        buf.push(100 + i, &regs, 0x01);
    }

    // get_recent(0) = newest (PC=104), get_recent(4) = oldest (PC=100)
    assert_eq!(buf.get_recent(0).unwrap().pc, 104);
    assert_eq!(buf.get_recent(1).unwrap().pc, 103);
    assert_eq!(buf.get_recent(2).unwrap().pc, 102);
    assert_eq!(buf.get_recent(3).unwrap().pc, 101);
    assert_eq!(buf.get_recent(4).unwrap().pc, 100);
    assert!(buf.get_recent(5).is_none());
}

#[test]
fn test_trace_recording_cleared_on_vm_reset() {
    let mut vm = Vm::new();
    let regs = [0u32; 32];
    vm.trace_recording = true;
    vm.trace_buffer.push(100, &regs, 0x01);
    assert_eq!(vm.trace_buffer.len(), 1);

    vm.reset();

    assert!(!vm.trace_recording, "trace_recording should be false after reset");
    assert_eq!(vm.trace_buffer.len(), 0, "trace buffer should be cleared after reset");
}
