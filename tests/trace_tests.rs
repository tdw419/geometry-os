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
    let regs = [
        1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25,
        26, 27, 28, 29, 30, 31, 32,
    ];
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
    let a = TraceEntry {
        step_number: 5,
        pc: 100,
        regs,
        opcode: 0x01,
    };
    let b = TraceEntry {
        step_number: 5,
        pc: 100,
        regs: [1u32; 16],
        opcode: 0x01,
    };
    assert_eq!(a, b);
}

#[test]
fn test_trace_entry_inequality() {
    let regs = [1u32; 16];
    let a = TraceEntry {
        step_number: 5,
        pc: 100,
        regs,
        opcode: 0x01,
    };
    let b = TraceEntry {
        step_number: 6,
        pc: 100,
        regs: [1u32; 16],
        opcode: 0x01,
    };
    assert_ne!(a, b);
}

// --- SNAP_TRACE opcode (0x7B) integration tests ---

#[test]
fn test_snap_trace_start_recording() {
    let mut vm = Vm::new();
    load_asm(
        &mut vm,
        "
        LDI r1, 1
        SNAP_TRACE r1
        LDI r2, 42
        LDI r3, 99
        HALT
    ",
        0x100,
    );

    // Run until halted
    loop {
        if !vm.step() {
            break;
        }
    }

    // Trace should have recorded LDI r2 and LDI r3 (2 instructions after SNAP_TRACE)
    assert!(
        vm.trace_buffer.len() >= 2,
        "should have at least 2 traced instructions, got {}",
        vm.trace_buffer.len()
    );
    assert!(
        vm.trace_recording,
        "recording should be on after SNAP_TRACE 1"
    );
}

#[test]
fn test_snap_trace_stop_recording() {
    let mut vm = Vm::new();
    load_asm(
        &mut vm,
        "
        LDI r1, 1
        SNAP_TRACE r1
        LDI r2, 42
        LDI r1, 0
        SNAP_TRACE r1
        LDI r3, 99
        HALT
    ",
        0x100,
    );

    loop {
        if !vm.step() {
            break;
        }
    }

    let len_after_stop = vm.trace_buffer.len();
    // LDI r3 after stop should NOT be recorded
    // Only LDI r2 should be recorded (between start and stop)
    assert!(
        len_after_stop >= 1,
        "should have at least 1 traced instruction"
    );
    assert!(!vm.trace_recording, "recording should be off");
}

#[test]
fn test_snap_trace_snapshot_and_clear() {
    let mut vm = Vm::new();
    load_asm(
        &mut vm,
        "
        LDI r1, 1
        SNAP_TRACE r1
        LDI r2, 10
        LDI r3, 20
        LDI r4, 30
        LDI r1, 2
        SNAP_TRACE r1
        HALT
    ",
        0x100,
    );

    loop {
        if !vm.step() {
            break;
        }
    }

    // r0 should hold the count of entries captured before clear
    assert!(
        vm.regs[0] >= 3,
        "r0 should have entry count (>=3), got {}",
        vm.regs[0]
    );
    // Buffer should be cleared
    assert_eq!(
        vm.trace_buffer.len(),
        0,
        "buffer should be empty after clear"
    );
    assert!(
        !vm.trace_recording,
        "recording should be off after snapshot-clear"
    );
}

#[test]
fn test_snap_trace_invalid_mode() {
    let mut vm = Vm::new();
    load_asm(
        &mut vm,
        "
        LDI r1, 99
        SNAP_TRACE r1
        HALT
    ",
        0x100,
    );

    loop {
        if !vm.step() {
            break;
        }
    }

    // r0 should be 0xFFFFFFFF for invalid mode
    assert_eq!(
        vm.regs[0], 0xFFFFFFFF,
        "invalid mode should return 0xFFFFFFFF"
    );
}

#[test]
fn test_snap_trace_returns_entry_count() {
    let mut vm = Vm::new();
    load_asm(
        &mut vm,
        "
        LDI r1, 1
        SNAP_TRACE r1
        LDI r2, 1
        LDI r3, 2
        LDI r4, 3
        LDI r1, 0
        SNAP_TRACE r1
        HALT
    ",
        0x100,
    );

    loop {
        if !vm.step() {
            break;
        }
    }

    // Second SNAP_TRACE (mode 0) returns count in r0
    assert!(
        vm.regs[0] >= 3,
        "r0 should have count of traced entries (>=3), got {}",
        vm.regs[0]
    );
}

#[test]
fn test_trace_disabled_by_default() {
    let mut vm = Vm::new();
    load_asm(
        &mut vm,
        "
        LDI r1, 42
        LDI r2, 99
        HALT
    ",
        0x100,
    );

    loop {
        if !vm.step() {
            break;
        }
    }

    assert_eq!(
        vm.trace_buffer.len(),
        0,
        "no entries when trace is disabled"
    );
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

    assert!(
        !vm.trace_recording,
        "trace_recording should be false after reset"
    );
    assert_eq!(
        vm.trace_buffer.len(),
        0,
        "trace buffer should be cleared after reset"
    );
}

// --- Phase 38b: Frame Checkpointing tests ---

use geometry_os::vm::FrameCheckBuffer;

#[test]
fn test_frame_check_buffer_new() {
    let buf = FrameCheckBuffer::new(10);
    assert_eq!(buf.len(), 0);
    assert!(buf.is_empty());
}

#[test]
fn test_frame_check_buffer_push_single() {
    let mut buf = FrameCheckBuffer::new(10);
    let screen = vec![0x112233; 65536];
    buf.push(100, 1, &screen);

    assert_eq!(buf.len(), 1);
    assert!(!buf.is_empty());

    let cp = buf.get_recent(0).unwrap();
    assert_eq!(cp.step_number, 100);
    assert_eq!(cp.frame_count, 1);
    assert_eq!(cp.screen[0], 0x112233);
    assert_eq!(cp.screen.len(), 65536);
}

#[test]
fn test_frame_check_buffer_push_multiple() {
    let mut buf = FrameCheckBuffer::new(10);
    let screen1 = vec![0x111111; 65536];
    let screen2 = vec![0x222222; 65536];
    let screen3 = vec![0x333333; 65536];

    buf.push(10, 1, &screen1);
    buf.push(20, 2, &screen2);
    buf.push(30, 3, &screen3);

    assert_eq!(buf.len(), 3);

    // get_recent(0) = newest
    assert_eq!(buf.get_recent(0).unwrap().frame_count, 3);
    assert_eq!(buf.get_recent(0).unwrap().screen[0], 0x333333);
    // get_recent(2) = oldest
    assert_eq!(buf.get_recent(2).unwrap().frame_count, 1);
    assert_eq!(buf.get_recent(2).unwrap().screen[0], 0x111111);
    // Out of bounds
    assert!(buf.get_recent(3).is_none());
}

#[test]
fn test_frame_check_buffer_wrap_around() {
    let capacity = 5;
    let mut buf = FrameCheckBuffer::new(capacity);

    // Push 8 frames into a 5-capacity buffer
    for i in 0..8u32 {
        let screen = vec![i as u32; 65536];
        buf.push(i as u64 * 10, i + 1, &screen);
    }

    assert_eq!(buf.len(), 5); // capped at capacity

    // Should have frames 3,4,5,6,7 (oldest 0,1,2 were overwritten)
    // get_recent(0) = newest = frame_count 8
    assert_eq!(buf.get_recent(0).unwrap().frame_count, 8);
    // get_recent(4) = oldest = frame_count 4
    assert_eq!(buf.get_recent(4).unwrap().frame_count, 4);
    // Verify the screen pixel values match
    assert_eq!(buf.get_recent(4).unwrap().screen[0], 3); // i=3 -> pixel val 3
}

#[test]
fn test_frame_check_buffer_clear() {
    let mut buf = FrameCheckBuffer::new(10);
    let screen = vec![0u32; 65536];
    buf.push(1, 1, &screen);
    buf.push(2, 2, &screen);
    assert_eq!(buf.len(), 2);

    buf.clear();
    assert_eq!(buf.len(), 0);
    assert!(buf.is_empty());
}

#[test]
fn test_frame_check_buffer_iter() {
    let mut buf = FrameCheckBuffer::new(10);
    for i in 0..3u32 {
        let screen = vec![i; 65536];
        buf.push(i as u64, i + 1, &screen);
    }

    // Iter should go oldest to newest: frame_count 1,2,3
    let counts: Vec<u32> = buf.iter().map(|c| c.frame_count).collect();
    assert_eq!(counts, vec![1, 2, 3]);
}

#[test]
fn test_frame_check_buffer_iter_after_wrap() {
    let mut buf = FrameCheckBuffer::new(3);
    for i in 0..6u32 {
        let screen = vec![i; 65536];
        buf.push(i as u64, i + 1, &screen);
    }

    // Buffer has frames 4,5,6 (frame_count: 4,5,6)
    let counts: Vec<u32> = buf.iter().map(|c| c.frame_count).collect();
    assert_eq!(counts, vec![4, 5, 6]);
}

#[test]
fn test_frame_check_minimum_capacity() {
    let buf = FrameCheckBuffer::new(0);
    assert_eq!(buf.len(), 0);

    let mut buf = FrameCheckBuffer::new(0);
    let screen = vec![42u32; 65536];
    buf.push(1, 1, &screen);
    assert_eq!(buf.len(), 1);
}

#[test]
fn test_frame_checkpoint_captured_on_frame_opcode() {
    let mut vm = Vm::new();
    load_asm(
        &mut vm,
        "
        LDI r1, 1
        SNAP_TRACE r1
        FRAME
        FRAME
        FRAME
        HALT
    ",
        0x100,
    );

    loop {
        if !vm.step() {
            break;
        }
    }

    // Should have captured 3 frame checkpoints
    assert_eq!(
        vm.frame_checkpoints.len(),
        3,
        "should have 3 frame checkpoints"
    );

    // Check frame_count values: 1, 2, 3
    let oldest = vm.frame_checkpoints.get_recent(2).unwrap();
    assert_eq!(oldest.frame_count, 1);
    assert_eq!(vm.frame_checkpoints.get_recent(1).unwrap().frame_count, 2);
    assert_eq!(vm.frame_checkpoints.get_recent(0).unwrap().frame_count, 3);

    // Step numbers should be increasing
    assert!(oldest.step_number < vm.frame_checkpoints.get_recent(1).unwrap().step_number);
}

// --- Phase 38c: Backward Replay tests ---

#[test]
fn test_frame_check_not_captured_when_recording_off() {
    let mut vm = Vm::new();
    load_asm(
        &mut vm,
        "
        FRAME
        FRAME
        FRAME
        HALT
    ",
        0x100,
    );

    loop {
        if !vm.step() {
            break;
        }
    }

    // trace_recording is off by default, so no checkpoints
    assert_eq!(
        vm.frame_checkpoints.len(),
        0,
        "no checkpoints when recording is off"
    );
}

#[test]
fn test_frame_check_eviction_300_frames() {
    // Use a small buffer capacity to test eviction
    let mut vm = Vm::new();
    vm.frame_checkpoints = FrameCheckBuffer::new(10);
    vm.trace_recording = true;

    // Simulate 300 frames: each FRAME opcode captures a checkpoint
    // The buffer holds 10, so only the last 10 should remain
    for i in 0..300u32 {
        // Write a unique value to screen[0] to identify each frame
        vm.screen[0] = i;
        // Manually trigger the frame checkpoint logic
        let step = vm.trace_buffer.step_counter();
        vm.frame_count = vm.frame_count.wrapping_add(1);
        vm.frame_checkpoints.push(step, vm.frame_count, &vm.screen);
    }

    assert_eq!(vm.frame_checkpoints.len(), 10);

    // Newest checkpoint should be frame 300
    assert_eq!(vm.frame_checkpoints.get_recent(0).unwrap().frame_count, 300);
    assert_eq!(vm.frame_checkpoints.get_recent(0).unwrap().screen[0], 299);

    // Oldest checkpoint should be frame 291
    assert_eq!(vm.frame_checkpoints.get_recent(9).unwrap().frame_count, 291);
    assert_eq!(vm.frame_checkpoints.get_recent(9).unwrap().screen[0], 290);

    // Iter should be oldest to newest
    let counts: Vec<u32> = vm.frame_checkpoints.iter().map(|c| c.frame_count).collect();
    assert_eq!(counts.len(), 10);
    assert_eq!(counts[0], 291);
    assert_eq!(counts[9], 300);
}

#[test]
fn test_frame_check_cleared_on_reset() {
    let mut vm = Vm::new();
    vm.trace_recording = true;
    let screen = vec![42u32; 65536];
    vm.frame_checkpoints.push(1, 1, &screen);
    assert_eq!(vm.frame_checkpoints.len(), 1);

    vm.reset();

    assert_eq!(
        vm.frame_checkpoints.len(),
        0,
        "frame checkpoints should be cleared after reset"
    );
}

#[test]
fn test_frame_check_step_numbers_monotonic() {
    let mut buf = FrameCheckBuffer::new(5);
    for i in 0..8u64 {
        let screen = vec![0u32; 65536];
        buf.push(i * 100, (i + 1) as u32, &screen);
    }

    let steps: Vec<u64> = buf.iter().map(|c| c.step_number).collect();
    for window in steps.windows(2) {
        assert!(window[1] > window[0], "step numbers must be monotonic");
    }
}

// --- Phase 38c: Backward Replay ---

#[test]
fn test_replay_from_basic() {
    let mut buf = TraceBuffer::new(100);
    let regs = [0u32; 32];

    // Push 10 entries with PCs 0..10
    for i in 0..10u32 {
        buf.push(i, &regs, 0x01);
    }

    // Replay from step 9 (newest), limit 3 -> entries with step 9, 8, 7
    let entries = buf.replay_from(9, 3);
    assert_eq!(entries.len(), 3);
    assert_eq!(entries[0].pc, 9); // newest first
    assert_eq!(entries[1].pc, 8);
    assert_eq!(entries[2].pc, 7);
}

#[test]
fn test_replay_from_middle() {
    let mut buf = TraceBuffer::new(100);
    let regs = [0u32; 32];

    for i in 0..10u32 {
        buf.push(i, &regs, 0x01);
    }

    // Replay from step 5, limit 3 -> entries with step 5, 4, 3
    let entries = buf.replay_from(5, 3);
    assert_eq!(entries.len(), 3);
    assert_eq!(entries[0].pc, 5);
    assert_eq!(entries[1].pc, 4);
    assert_eq!(entries[2].pc, 3);
}

#[test]
fn test_replay_from_empty_buffer() {
    let buf = TraceBuffer::new(100);
    let entries = buf.replay_from(0, 10);
    assert!(entries.is_empty());
}

#[test]
fn test_replay_from_step_beyond_counter() {
    let mut buf = TraceBuffer::new(100);
    let regs = [0u32; 32];

    for i in 0..5u32 {
        buf.push(i, &regs, 0x01);
    }

    // Step 999 is beyond counter (5), should start from newest
    let entries = buf.replay_from(999, 3);
    assert_eq!(entries.len(), 3);
    assert_eq!(entries[0].pc, 4); // newest
    assert_eq!(entries[1].pc, 3);
    assert_eq!(entries[2].pc, 2);
}

#[test]
fn test_replay_from_wrapped_buffer() {
    let mut buf = TraceBuffer::new(5);
    let regs = [0u32; 32];

    // Push 10 entries into a 5-capacity buffer
    // Entries 5..10 survive (PCs 5..9)
    for i in 0..10u32 {
        buf.push(i, &regs, 0x01);
    }

    let entries = buf.replay_from(8, 2);
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].pc, 8);
    assert_eq!(entries[1].pc, 7);
}

#[test]
fn test_replay_from_limit_exceeds_buffer() {
    let mut buf = TraceBuffer::new(100);
    let regs = [0u32; 32];

    for i in 0..5u32 {
        buf.push(i, &regs, 0x01);
    }

    // Limit 100 but only 5 entries
    let entries = buf.replay_from(4, 100);
    assert_eq!(entries.len(), 5);
    assert_eq!(entries[0].pc, 4);
    assert_eq!(entries[4].pc, 0);
}

#[test]
fn test_iter_rev() {
    let mut buf = TraceBuffer::new(100);
    let regs = [0u32; 32];

    for i in 0..5u32 {
        buf.push(i, &regs, 0x01);
    }

    // iter_rev should go newest to oldest: 4, 3, 2, 1, 0
    let pcs: Vec<u32> = buf.iter_rev().map(|e| e.pc).collect();
    assert_eq!(pcs, vec![4, 3, 2, 1, 0]);
}

#[test]
fn test_replay_frame_basic() {
    let mut buf = FrameCheckBuffer::new(10);

    // Push 3 frames with different colors
    let screen1 = vec![0x111111u32; 65536];
    let screen2 = vec![0x222222u32; 65536];
    let screen3 = vec![0x333333u32; 65536];

    buf.push(10, 1, &screen1);
    buf.push(20, 2, &screen2);
    buf.push(30, 3, &screen3);

    // Replay frame 0 (newest) -> screen3
    let replayed = buf.replay_frame(0).unwrap();
    assert_eq!(replayed[0], 0x333333);

    // Replay frame 2 (oldest) -> screen1
    let replayed = buf.replay_frame(2).unwrap();
    assert_eq!(replayed[0], 0x111111);

    // Out of bounds
    assert!(buf.replay_frame(3).is_none());
}

#[test]
fn test_replay_frame_empty() {
    let buf = FrameCheckBuffer::new(10);
    assert!(buf.replay_frame(0).is_none());
}

#[test]
fn test_replay_frame_wrapped() {
    let mut buf = FrameCheckBuffer::new(3);

    for i in 0..6u32 {
        let screen = vec![i; 65536];
        buf.push(i as u64, i + 1, &screen);
    }

    // Buffer has frames 4,5,6 (frame_count 5,6,7)
    // replay_frame(0) = newest = pixel val 5
    let newest = buf.replay_frame(0).unwrap();
    assert_eq!(newest[0], 5);

    // replay_frame(2) = oldest = pixel val 3
    let oldest = buf.replay_frame(2).unwrap();
    assert_eq!(oldest[0], 3);
}

// --- REPLAY opcode (0x7C) integration tests ---

#[test]
fn test_replay_opcode_success() {
    let mut vm = Vm::new();
    // Set up trace recording and capture 3 frames
    load_asm(
        &mut vm,
        "
        LDI r1, 1
        SNAP_TRACE r1
        LDI r5, 0xFF0000
        SCREENP r5, r5, r5
        FRAME
        LDI r5, 0x00FF00
        SCREENP r5, r5, r5
        FRAME
        LDI r5, 0x0000FF
        SCREENP r5, r5, r5
        FRAME
        LDI r1, 0
        SNAP_TRACE r1
        LDI r2, 0
        REPLAY r2
        HALT
    ",
        0x100,
    );

    loop {
        if !vm.step() {
            break;
        }
    }

    // REPLAY with index 0 should load the most recent checkpoint
    // r0 should be the frame count (3)
    assert_eq!(vm.regs[0], 3, "r0 should be frame_count after REPLAY");
    assert!(vm.frame_ready, "frame_ready should be set");
}

#[test]
fn test_replay_opcode_invalid_index() {
    let mut vm = Vm::new();
    load_asm(
        &mut vm,
        "
        LDI r1, 1
        SNAP_TRACE r1
        FRAME
        LDI r1, 0
        SNAP_TRACE r1
        LDI r2, 99
        REPLAY r2
        HALT
    ",
        0x100,
    );

    loop {
        if !vm.step() {
            break;
        }
    }

    // Index 99 is out of bounds (only 1 frame checkpoint)
    assert_eq!(
        vm.regs[0], 0xFFFFFFFF,
        "r0 should be 0xFFFFFFFF for invalid index"
    );
}

#[test]
fn test_replay_opcode_no_checkpoints() {
    let mut vm = Vm::new();
    load_asm(
        &mut vm,
        "
        LDI r2, 0
        REPLAY r2
        HALT
    ",
        0x100,
    );

    loop {
        if !vm.step() {
            break;
        }
    }

    // No trace recording was on, so no checkpoints
    assert_eq!(
        vm.regs[0], 0xFFFFFFFF,
        "r0 should be 0xFFFFFFFF with no checkpoints"
    );
}

#[test]
fn test_checkpoint_replay_pipeline() {
    // Full pipeline: record frames -> stop -> replay each frame -> verify screen
    let mut vm = Vm::new();
    vm.trace_recording = true;

    // Manually create 3 distinct frames
    for color in [0x111111u32, 0x222222, 0x333333].iter() {
        vm.screen[0] = *color;
        vm.screen[65535] = *color;
        let step = vm.trace_buffer.step_counter();
        vm.frame_count = vm.frame_count.wrapping_add(1);
        vm.frame_checkpoints.push(step, vm.frame_count, &vm.screen);
    }

    vm.trace_recording = false;
    assert_eq!(vm.frame_checkpoints.len(), 3);

    // Now set screen to something else
    vm.screen[0] = 0xDEAD;
    vm.screen[65535] = 0xDEAD;

    // Replay frame 2 (oldest, color 0x111111)
    let replayed = vm.frame_checkpoints.replay_frame(2).unwrap();
    assert_eq!(replayed[0], 0x111111);
    assert_eq!(replayed[65535], 0x111111);

    // Replay frame 0 (newest, color 0x333333)
    let replayed = vm.frame_checkpoints.replay_frame(0).unwrap();
    assert_eq!(replayed[0], 0x333333);
    assert_eq!(replayed[65535], 0x333333);
}

// --- Phase 38d: Timeline Forking tests ---

use geometry_os::vm::MAX_SNAPSHOTS;

#[test]
fn test_snapshot_restore_roundtrip() {
    let mut vm = Vm::new();
    vm.ram[100] = 42;
    vm.ram[200] = 99;
    vm.regs[5] = 0x12345678;
    vm.pc = 500;
    vm.rand_state = 0xCAFEBABE;
    vm.frame_count = 17;
    vm.screen[0] = 0xFF000000;
    vm.screen[65535] = 0x00FF0000;

    let snap = vm.snapshot();
    assert_eq!(snap.ram[100], 42);
    assert_eq!(snap.ram[200], 99);
    assert_eq!(snap.regs[5], 0x12345678);
    assert_eq!(snap.pc, 500);
    assert_eq!(snap.rand_state, 0xCAFEBABE);
    assert_eq!(snap.frame_count, 17);
    assert_eq!(snap.screen[0], 0xFF000000);
    assert_eq!(snap.screen[65535], 0x00FF0000);

    // Mutate
    vm.ram[100] = 0;
    vm.ram[200] = 0;
    vm.regs[5] = 0;
    vm.pc = 0;
    vm.screen[0] = 0;
    vm.screen[65535] = 0;

    // Restore
    vm.restore(&snap);
    assert_eq!(vm.ram[100], 42);
    assert_eq!(vm.ram[200], 99);
    assert_eq!(vm.regs[5], 0x12345678);
    assert_eq!(vm.pc, 500);
    assert_eq!(vm.rand_state, 0xCAFEBABE);
    assert_eq!(vm.frame_count, 17);
    assert_eq!(vm.screen[0], 0xFF000000);
    assert_eq!(vm.screen[65535], 0x00FF0000);
}

#[test]
fn test_multiple_snapshots() {
    let mut vm = Vm::new();

    vm.regs[1] = 10;
    let snap0 = vm.snapshot();

    vm.regs[1] = 20;
    let snap1 = vm.snapshot();

    vm.regs[1] = 30;
    let snap2 = vm.snapshot();

    vm.restore(&snap0);
    assert_eq!(vm.regs[1], 10);

    vm.restore(&snap2);
    assert_eq!(vm.regs[1], 30);

    vm.restore(&snap1);
    assert_eq!(vm.regs[1], 20);
}

#[test]
fn test_restore_from_middle() {
    let mut vm = Vm::new();
    for i in 0..100 {
        vm.ram[i] = i as u32;
    }
    let snap = vm.snapshot();

    for i in 0..100 {
        vm.ram[i] = 0xFFFFFFFF;
    }

    vm.restore(&snap);
    for i in 0..100 {
        assert_eq!(vm.ram[i], i as u32, "RAM[{}] should be {}", i, i);
    }
}

#[test]
fn test_fork_opcode_save() {
    let mut vm = Vm::new();
    load_asm(
        &mut vm,
        "
        LDI r1, 42
        LDI r7, 0
        FORK r7
        HALT
    ",
        0x100,
    );

    loop {
        if !vm.step() {
            break;
        }
    }

    assert_eq!(vm.snapshots.len(), 1, "should have 1 snapshot");
    assert_eq!(vm.regs[0], 0, "r0 should be slot index 0");
    assert_eq!(vm.snapshots[0].regs[1], 42);
}

#[test]
fn test_fork_opcode_list() {
    let mut vm = Vm::new();
    load_asm(
        &mut vm,
        "
        LDI r7, 0
        FORK r7
        LDI r7, 2
        FORK r7
        HALT
    ",
        0x100,
    );

    loop {
        if !vm.step() {
            break;
        }
    }

    assert_eq!(vm.regs[0], 1, "should have 1 snapshot");
    assert_eq!(vm.snapshots.len(), 1);
}

#[test]
fn test_fork_opcode_clear() {
    let mut vm = Vm::new();
    load_asm(
        &mut vm,
        "
        LDI r7, 0
        FORK r7
        LDI r7, 3
        FORK r7
        HALT
    ",
        0x100,
    );

    loop {
        if !vm.step() {
            break;
        }
    }

    assert_eq!(vm.regs[0], 0, "clear should succeed");
    assert_eq!(vm.snapshots.len(), 0, "snapshots should be empty");
}

#[test]
fn test_fork_opcode_max_snapshots() {
    let mut vm = Vm::new();
    for i in 0..MAX_SNAPSHOTS {
        vm.regs[1] = i as u32;
        vm.snapshots.push(vm.snapshot());
    }

    assert_eq!(vm.snapshots.len(), MAX_SNAPSHOTS);
    load_asm(
        &mut vm,
        "
        LDI r7, 0
        FORK r7
        HALT
    ",
        0x200,
    );

    loop {
        if !vm.step() {
            break;
        }
    }

    assert_eq!(
        vm.regs[0], 0xFFFFFFFF,
        "should fail when max snapshots reached"
    );
    assert_eq!(vm.snapshots.len(), MAX_SNAPSHOTS, "should not exceed max");
}

#[test]
fn test_fork_opcode_invalid_mode() {
    let mut vm = Vm::new();
    load_asm(
        &mut vm,
        "
        LDI r7, 99
        FORK r7
        HALT
    ",
        0x100,
    );

    loop {
        if !vm.step() {
            break;
        }
    }

    assert_eq!(vm.regs[0], 0xFFFFFFFF, "invalid mode should return error");
}

#[test]
fn test_fork_opcode_invalid_slot() {
    let mut vm = Vm::new();
    load_asm(
        &mut vm,
        "
        LDI r7, 0
        FORK r7
        LDI r7, 1
        LDI r1, 5
        FORK r7
        HALT
    ",
        0x100,
    );

    loop {
        if !vm.step() {
            break;
        }
    }

    assert_eq!(vm.regs[0], 0xFFFFFFFF, "invalid slot should return error");
}

#[test]
fn test_fork_restore_api_roundtrip() {
    let mut vm = Vm::new();

    vm.regs[1] = 111;
    vm.regs[2] = 222;
    vm.ram[1000] = 0xABCD;
    vm.pc = 0x50;

    let snap = vm.snapshot();
    vm.snapshots.push(snap);

    vm.regs[1] = 999;
    vm.regs[2] = 888;
    vm.ram[1000] = 0;
    vm.pc = 0x99;

    let snap = vm.snapshots[0].clone();
    vm.restore(&snap);

    assert_eq!(vm.regs[1], 111);
    assert_eq!(vm.regs[2], 222);
    assert_eq!(vm.ram[1000], 0xABCD);
    assert_eq!(vm.pc, 0x50);
}

#[test]
fn test_fork_demo_assembles() {
    let source =
        std::fs::read_to_string("programs/fork_demo.asm").expect("fork_demo.asm should exist");
    let result = assemble(&source, 0);
    assert!(
        result.is_ok(),
        "fork_demo.asm should assemble: {:?}",
        result.err()
    );
}

#[test]
fn test_snapshots_cleared_on_reset() {
    let mut vm = Vm::new();
    vm.snapshots.push(vm.snapshot());
    vm.snapshots.push(vm.snapshot());
    assert_eq!(vm.snapshots.len(), 2);

    vm.reset();
    assert_eq!(
        vm.snapshots.len(),
        0,
        "snapshots should be cleared on reset"
    );
}

// --- TRACE_READ opcode (0x83) integration tests ---

#[test]
fn test_trace_read_query_count_empty() {
    let mut vm = Vm::new();
    load_asm(
        &mut vm,
        "
        LDI r1, 0
        TRACE_READ r1
        HALT
    ",
        0x100,
    );

    loop {
        if !vm.step() {
            break;
        }
    }

    // r0 should be 0 (empty trace buffer)
    assert_eq!(vm.regs[0], 0, "trace buffer should be empty");
}

#[test]
fn test_trace_read_query_count_after_recording() {
    let mut vm = Vm::new();
    load_asm(
        &mut vm,
        "
        LDI r1, 1
        SNAP_TRACE r1
        LDI r2, 42
        LDI r3, 99
        LDI r1, 0
        TRACE_READ r1
        HALT
    ",
        0x100,
    );

    loop {
        if !vm.step() {
            break;
        }
    }

    // Should have recorded 2 instructions (LDI r2, LDI r3)
    assert!(
        vm.regs[0] >= 2,
        "should have at least 2 entries, got {}",
        vm.regs[0]
    );
}

#[test]
fn test_trace_read_entry_at_index() {
    let mut vm = Vm::new();
    load_asm(
        &mut vm,
        "
        LDI r1, 1
        SNAP_TRACE r1
        LDI r4, 42
        LDI r5, 99
        LDI r1, 0
        SNAP_TRACE r1
        ; Read the first entry (index 0 = oldest) into RAM at 0x7000
        LDI r1, 1       ; mode = 1 (read entry)
        LDI r2, 0       ; index = 0
        LDI r3, 0x7000  ; dest
        TRACE_READ r1
        HALT
    ",
        0x100,
    );

    loop {
        if !vm.step() {
            break;
        }
    }

    // r0 should be 0 (success)
    assert_eq!(vm.regs[0], 0, "read should succeed");

    // Verify data was written: check the opcode field at 0x7000+19
    assert_eq!(vm.ram[0x7000 + 19], 0x10, "opcode should be LDI (0x10)");
}

#[test]
fn test_trace_read_entry_valid_data() {
    let mut vm = Vm::new();
    load_asm(
        &mut vm,
        "
        LDI r1, 1
        SNAP_TRACE r1
        LDI r2, 42
        LDI r3, 99
        LDI r1, 0
        SNAP_TRACE r1
        ; Read entry 0 (oldest) into RAM at 0x7000
        LDI r1, 1       ; mode = 1 (read entry)
        LDI r2, 0       ; index = 0 (oldest entry)
        LDI r3, 0x7000  ; dest address
        TRACE_READ r1
        HALT
    ",
        0x100,
    );

    loop {
        if !vm.step() {
            break;
        }
    }

    // r0 should be 0 (success)
    assert_eq!(vm.regs[0], 0, "read should succeed");

    // Check that data was written at 0x7000
    // Format: [step_lo, step_hi, pc, r0..r15, opcode]
    let step_lo = vm.ram[0x7000];
    let step_hi = vm.ram[0x7001];
    let pc = vm.ram[0x7002];
    let opcode = vm.ram[0x7000 + 19];

    // step_number should be 0 (first traced instruction starts at step 0)
    assert_eq!(step_lo, 0, "first entry step_number should be 0");
    assert_eq!(step_hi, 0, "first entry step_number hi should be 0");
    // PC should be the LDI r2 instruction (first traced instruction)
    // After LDI r1,1 (3 words) and SNAP_TRACE r1 (2 words) = 5 words
    // So LDI r2 starts at 0x100 + 5 = 0x105
    assert_eq!(pc, 0x100 + 5, "PC should be the LDI r2 instruction");
    assert_eq!(opcode, 0x10, "opcode should be LDI (0x10)");
}

#[test]
fn test_trace_read_entry_out_of_range() {
    let mut vm = Vm::new();
    load_asm(
        &mut vm,
        "
        LDI r1, 1
        SNAP_TRACE r1
        LDI r2, 42
        LDI r1, 0
        SNAP_TRACE r1
        ; Try to read entry 999 (doesn't exist)
        LDI r1, 1
        LDI r2, 999     ; index = 999
        LDI r3, 0x7000  ; dest address
        TRACE_READ r1
        HALT
    ",
        0x100,
    );

    loop {
        if !vm.step() {
            break;
        }
    }

    // r0 should be 0xFFFFFFFF (error: index out of range)
    assert_eq!(
        vm.regs[0],
        0xFFFFFFFF,
        "should return error for out-of-range index"
    );
}

#[test]
fn test_trace_read_count_opcode() {
    let mut vm = Vm::new();
    load_asm(
        &mut vm,
        "
        LDI r1, 1
        SNAP_TRACE r1
        LDI r2, 42
        LDI r3, 99
        ADD r4, r2, r3
        LDI r1, 0
        SNAP_TRACE r1
        ; Count how many LDI (0x10) opcodes were recorded
        LDI r1, 2
        LDI r2, 0x10     ; target opcode = LDI
        TRACE_READ r1
        HALT
    ",
        0x100,
    );

    loop {
        if !vm.step() {
            break;
        }
    }

    // Should have recorded 3 LDI instructions (LDI r2, LDI r3, ADD) -- wait, ADD is not LDI
    // Actually: LDI r2, 42 (0x10), LDI r3, 99 (0x10), ADD r4, r2, r3 (not 0x10)
    // Plus the SNAP_TRACE r1 instructions (0x7B) are NOT traced (trace happens before opcode execution,
    // and the SNAP_TRACE itself is being executed). Actually, let me re-check.
    // The trace records every instruction while recording is on. So:
    // - LDI r2, 42 (0x10) -> recorded
    // - LDI r3, 99 (0x10) -> recorded
    // - ADD r4, r2, r3 (0x08) -> recorded
    // - LDI r1, 0 (0x10) -> recorded
    // So there should be 3 LDI entries.
    assert_eq!(
        vm.regs[0], 3,
        "should find 3 LDI opcodes, got {}",
        vm.regs[0]
    );
}

#[test]
fn test_trace_read_find_opcode_indices() {
    let mut vm = Vm::new();
    load_asm(
        &mut vm,
        "
        LDI r1, 1
        SNAP_TRACE r1
        LDI r2, 42
        LDI r3, 99
        ADD r4, r2, r3
        LDI r1, 0
        SNAP_TRACE r1
        ; Find indices of LDI (0x10) entries
        LDI r1, 3
        LDI r2, 0x10     ; target opcode = LDI
        LDI r3, 0x7000  ; dest address
        TRACE_READ r1
        HALT
    ",
        0x100,
    );

    loop {
        if !vm.step() {
            break;
        }
    }

    // r0 should be 3 (3 LDI entries found)
    assert_eq!(vm.regs[0], 3, "should find 3 LDI indices");

    // Indices should be written to RAM[0x7000..0x7002]
    // Index 0 should be the first LDI (LDI r2, 42)
    assert!(vm.ram[0x7000] < vm.ram[0x7001], "indices should be in order");
    assert!(vm.ram[0x7001] < vm.ram[0x7002], "indices should be in order");
}

#[test]
fn test_trace_read_invalid_mode() {
    let mut vm = Vm::new();
    load_asm(
        &mut vm,
        "
        LDI r1, 99
        TRACE_READ r1
        HALT
    ",
        0x100,
    );

    loop {
        if !vm.step() {
            break;
        }
    }

    // r0 should be 0xFFFFFFFF (invalid mode)
    assert_eq!(
        vm.regs[0],
        0xFFFFFFFF,
        "should return error for invalid mode"
    );
}

#[test]
fn test_trace_read_assembler() {
    let result = assemble(
        "
        LDI r1, 0
        TRACE_READ r1
        HALT
    ",
        0x100,
    );
    assert!(result.is_ok(), "TRACE_READ should assemble: {:?}", result.err());
}

#[test]
fn test_trace_read_disassembler() {
    let mut vm = Vm::new();
    vm.ram[0x100] = 0x83;
    vm.ram[0x101] = 1; // TRACE_READ r1
    let (mnemonic, len) = vm.disassemble_at(0x100);
    assert_eq!(len, 2, "TRACE_READ should be 2 words");
    assert_eq!(mnemonic, "TRACE_READ r1");
}
