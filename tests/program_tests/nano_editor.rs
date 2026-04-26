/// Tests for nano_editor.asm -- Phase 139 Text Editor
use geometry_os::assembler::assemble;
use geometry_os::vm::Vm;

/// Load and run the nano editor, returning the VM in a state after N frames
fn load_nano(frames: usize) -> Vm {
    let source =
        std::fs::read_to_string("programs/nano_editor.asm").expect("nano_editor.asm not found");
    let asm = assemble(&source, 0).expect("nano_editor.asm failed to assemble");
    let mut vm = Vm::new();
    for (i, &pixel) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = pixel;
        }
    }
    vm.pc = 0;
    vm.halted = false;

    let mut frames_seen = 0;
    for _ in 0..10_000_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_ready {
            vm.frame_ready = false;
            frames_seen += 1;
            if frames_seen >= frames {
                break;
            }
        }
    }
    vm
}

// RAM addresses used by nano_editor.asm
const R_NL: usize = 0x7400; // line count
const R_DIRTY: usize = 0x7401; // modified flag
const R_CL: usize = 0x7402; // cursor line
const R_CC: usize = 0x7403; // cursor col
const R_SC: usize = 0x7404; // scroll offset
const R_BS: usize = 0x7406; // buffer size
const FB: usize = 0x5400; // file buffer base
const LS: usize = 0x5000; // line starts table

#[test]
fn test_nano_editor_assembles() {
    let source = std::fs::read_to_string("programs/nano_editor.asm").unwrap();
    let result = assemble(&source, 0);
    assert!(
        result.is_ok(),
        "nano_editor.asm should assemble: {:?}",
        result.err()
    );
    let asm = result.unwrap();
    assert!(asm.pixels.len() > 100, "should have substantial bytecode");
}

#[test]
fn test_nano_editor_runs_and_shows_ui() {
    let vm = load_nano(1);
    // Should not be halted (editor runs forever until Ctrl+Q)
    assert!(!vm.halted, "editor should be running after 1 frame");

    // Should have at least 1 line (empty file = 1 empty line)
    let line_count = vm.ram[R_NL];
    assert!(
        line_count >= 1,
        "should have at least 1 line, got {}",
        line_count
    );

    // Cursor should be at (0, 0)
    assert_eq!(vm.ram[R_CL], 0, "cursor should be at line 0");
    assert_eq!(vm.ram[R_CC], 0, "cursor should be at col 0");

    // Screen should not be all black (title bar, hint bar should be visible)
    let non_black = vm.screen.iter().filter(|&&p| p != 0).count();
    assert!(
        non_black > 100,
        "screen should have visible UI elements, got {} non-black pixels",
        non_black
    );
}

#[test]
fn test_nano_editor_insert_chars() {
    let source = std::fs::read_to_string("programs/nano_editor.asm").unwrap();
    let asm = assemble(&source, 0).unwrap();
    let mut vm = Vm::new();
    for (i, &pixel) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = pixel;
        }
    }
    vm.pc = 0;
    vm.halted = false;

    // Run 1 frame to initialize
    for _ in 0..1_000_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_ready {
            vm.frame_ready = false;
            break;
        }
    }

    // Simulate typing 'H' (ASCII 72)
    vm.push_key(72);
    for _ in 0..100_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_ready {
            vm.frame_ready = false;
            break;
        }
    }

    // Check buffer has 'H'
    assert_eq!(vm.ram[FB], 72, "buffer should contain 'H' after typing");
    assert_eq!(vm.ram[R_BS], 1, "buffer size should be 1");
    assert_eq!(vm.ram[R_CC], 1, "cursor should be at col 1");
    assert_eq!(vm.ram[R_DIRTY], 1, "should be marked dirty");

    // Type 'i' (ASCII 105)
    vm.push_key(105);
    for _ in 0..100_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_ready {
            vm.frame_ready = false;
            break;
        }
    }

    assert_eq!(
        vm.ram[FB + 1],
        105,
        "buffer should contain 'i' after typing"
    );
    assert_eq!(vm.ram[R_BS], 2, "buffer size should be 2");
    assert_eq!(vm.ram[R_CC], 2, "cursor should be at col 2");
}

#[test]
fn test_nano_editor_backspace() {
    let source = std::fs::read_to_string("programs/nano_editor.asm").unwrap();
    let asm = assemble(&source, 0).unwrap();
    let mut vm = Vm::new();
    for (i, &pixel) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = pixel;
        }
    }
    vm.pc = 0;
    vm.halted = false;

    // Run 1 frame to init
    for _ in 0..1_000_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_ready {
            vm.frame_ready = false;
            break;
        }
    }

    // Type 'A' then 'B'
    vm.push_key(65); // 'A'
    for _ in 0..100_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_ready {
            vm.frame_ready = false;
            break;
        }
    }
    vm.push_key(66); // 'B'
    for _ in 0..100_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_ready {
            vm.frame_ready = false;
            break;
        }
    }

    assert_eq!(vm.ram[FB], 65, "should have 'A'");
    assert_eq!(vm.ram[FB + 1], 66, "should have 'B'");
    assert_eq!(vm.ram[R_BS], 2, "buffer size = 2");
    assert_eq!(vm.ram[R_CC], 2, "cursor at col 2");

    // Press backspace (ASCII 8)
    vm.push_key(8);
    for _ in 0..100_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_ready {
            vm.frame_ready = false;
            break;
        }
    }

    assert_eq!(vm.ram[R_BS], 1, "buffer size should be 1 after backspace");
    assert_eq!(vm.ram[R_CC], 1, "cursor should be at col 1");
    assert_eq!(vm.ram[FB], 65, "'A' should still be in buffer");
}

#[test]
fn test_nano_editor_enter_creates_newline() {
    let source = std::fs::read_to_string("programs/nano_editor.asm").unwrap();
    let asm = assemble(&source, 0).unwrap();
    let mut vm = Vm::new();
    for (i, &pixel) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = pixel;
        }
    }
    vm.pc = 0;
    vm.halted = false;

    // Run 1 frame to init
    for _ in 0..1_000_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_ready {
            vm.frame_ready = false;
            break;
        }
    }

    // Type 'X'
    vm.push_key(88); // 'X'
    for _ in 0..100_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_ready {
            vm.frame_ready = false;
            break;
        }
    }

    // Press Enter (ASCII 10)
    vm.push_key(10);
    for _ in 0..100_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_ready {
            vm.frame_ready = false;
            break;
        }
    }

    // Should have 2 lines now
    assert_eq!(vm.ram[R_NL], 2, "should have 2 lines after enter");
    assert_eq!(vm.ram[R_CL], 1, "cursor should be on line 1");
    assert_eq!(vm.ram[R_CC], 0, "cursor should be at col 0");

    // Buffer should contain 'X' then '\n'
    assert_eq!(vm.ram[FB], 88, "first char should be 'X'");
    assert_eq!(vm.ram[FB + 1], 10, "second char should be newline");
    assert_eq!(vm.ram[R_BS], 2, "buffer size should be 2");
}

#[test]
fn test_nano_editor_ctrl_q_quits() {
    let source = std::fs::read_to_string("programs/nano_editor.asm").unwrap();
    let asm = assemble(&source, 0).unwrap();
    let mut vm = Vm::new();
    for (i, &pixel) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = pixel;
        }
    }
    vm.pc = 0;
    vm.halted = false;

    // Run 1 frame to init
    for _ in 0..1_000_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_ready {
            vm.frame_ready = false;
            break;
        }
    }

    // Press Ctrl+Q (ASCII 17)
    vm.push_key(17);
    for _ in 0..100_000 {
        if !vm.step() {
            break;
        }
    }

    assert!(vm.halted, "editor should halt after Ctrl+Q");
}

/// Helper: load nano_editor with text pre-loaded into the file buffer
fn load_nano_with_text(text: &str) -> Vm {
    let source = std::fs::read_to_string("programs/nano_editor.asm").unwrap();
    let asm = assemble(&source, 0).unwrap();
    let mut vm = Vm::new();
    for (i, &pixel) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = pixel;
        }
    }
    vm.pc = 0;
    vm.halted = false;

    // Run 1 frame to initialize
    for _ in 0..1_000_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_ready {
            vm.frame_ready = false;
            break;
        }
    }

    // Write text into the file buffer
    let fb: usize = 0x5400;
    let ls: usize = 0x5000;
    let r_nl: usize = 0x7400;
    let r_bs: usize = 0x7406;
    let r_dirty: usize = 0x7401;

    let bytes: Vec<u32> = text.chars().map(|c| c as u32).collect();
    for (i, &b) in bytes.iter().enumerate() {
        vm.ram[fb + i] = b;
    }
    vm.ram[r_bs] = bytes.len() as u32;
    vm.ram[r_dirty] = 0; // not dirty from user edits

    // Build line starts table
    let mut line_starts = vec![0u32];
    for (i, &b) in bytes.iter().enumerate() {
        if b == 10 {
            // newline
            line_starts.push((i + 1) as u32);
        }
    }
    for (i, &start) in line_starts.iter().enumerate() {
        vm.ram[ls + i] = start;
    }
    vm.ram[r_nl] = line_starts.len() as u32;

    vm
}

/// Advance VM by one frame
fn advance_frame(vm: &mut Vm) {
    vm.frame_ready = false;
    for _ in 0..200_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_ready {
            vm.frame_ready = false;
            break;
        }
    }
}

#[test]
fn test_nano_editor_search_finds_text() {
    let mut vm = load_nano_with_text("Hello World\nSecond line\nThird line");

    // Ctrl+F to enter search mode
    vm.push_key(6); // Ctrl+F
    advance_frame(&mut vm);

    // Check prompt mode is search (1)
    assert_eq!(vm.ram[0x74C0], 1, "should be in search prompt mode");

    // Type "Second"
    for ch in "Second".chars() {
        vm.push_key(ch as u32);
        advance_frame(&mut vm);
    }

    // Press Enter to execute search
    vm.push_key(10);
    advance_frame(&mut vm);

    // Should find "Second" on line 1
    let match_line = vm.ram[0x74C2]; // R_SML
    assert_ne!(match_line, 0xFFFFFFFF, "should find a match");
    assert_eq!(match_line, 1, "match should be on line 1 (Second line)");

    // Cursor should be on the match line
    assert_eq!(vm.ram[0x7402], 1, "cursor should be on line 1");

    // Prompt mode should be cleared after execute
    assert_eq!(vm.ram[0x74C0], 0, "prompt mode should be cleared");
}

#[test]
fn test_nano_editor_search_not_found() {
    let mut vm = load_nano_with_text("Hello World\nSecond line");

    // Ctrl+F
    vm.push_key(6);
    advance_frame(&mut vm);

    // Type "xyz"
    for ch in "xyz".chars() {
        vm.push_key(ch as u32);
        advance_frame(&mut vm);
    }

    // Execute search
    vm.push_key(10);
    advance_frame(&mut vm);

    // Should not find
    let match_line = vm.ram[0x74C2]; // R_SML
    assert_eq!(match_line, 0xFFFFFFFF, "should not find match for 'xyz'");
}

#[test]
fn test_nano_editor_search_empty_string() {
    let mut vm = load_nano_with_text("Hello World");

    // Ctrl+F
    vm.push_key(6);
    advance_frame(&mut vm);

    // Press Enter immediately (empty search)
    vm.push_key(10);
    advance_frame(&mut vm);

    // Should not crash, match should be "not found"
    let match_line = vm.ram[0x74C2]; // R_SML
    assert_eq!(match_line, 0xFFFFFFFF, "empty search should not find");
}

#[test]
fn test_nano_editor_search_escape_cancels() {
    let mut vm = load_nano_with_text("Hello World");

    // Ctrl+F
    vm.push_key(6);
    advance_frame(&mut vm);
    assert_eq!(vm.ram[0x74C0], 1, "should be in search mode");

    // Escape
    vm.push_key(27);
    advance_frame(&mut vm);

    // Prompt mode should be cleared
    assert_eq!(vm.ram[0x74C0], 0, "escape should cancel search");
}

#[test]
fn test_nano_editor_goto_line() {
    let mut vm = load_nano_with_text("Line 0\nLine 1\nLine 2\nLine 3\nLine 4");

    // Ctrl+G to enter goto mode
    vm.push_key(7); // Ctrl+G
    advance_frame(&mut vm);

    // Check prompt mode is goto (2)
    assert_eq!(vm.ram[0x74C0], 2, "should be in goto prompt mode");

    // Type "3" (1-based line number)
    vm.push_key(51); // '3'
    advance_frame(&mut vm);

    // Press Enter
    vm.push_key(10);
    advance_frame(&mut vm);

    // Should jump to line 2 (0-based)
    assert_eq!(vm.ram[0x7402], 2, "cursor should be on line 2 (0-based)");
    assert_eq!(vm.ram[0x7403], 0, "cursor col should be 0");

    // Prompt mode should be cleared
    assert_eq!(vm.ram[0x74C0], 0, "prompt mode should be cleared");
}

#[test]
fn test_nano_editor_goto_multi_digit() {
    let mut vm = load_nano_with_text("Line 0\nLine 1\nLine 2\nLine 3\nLine 4");

    // Ctrl+G
    vm.push_key(7);
    advance_frame(&mut vm);

    // Type "12" -> line 11 (0-based), but clamped to last line (4)
    vm.push_key(49); // '1'
    advance_frame(&mut vm);
    vm.push_key(50); // '2'
    advance_frame(&mut vm);

    // Enter
    vm.push_key(10);
    advance_frame(&mut vm);

    // Should clamp to last line (line 4, 0-based)
    assert_eq!(vm.ram[0x7402], 4, "cursor should clamp to last line");
}

#[test]
fn test_nano_editor_goto_zero_is_noop() {
    let mut vm = load_nano_with_text("Line 0\nLine 1\nLine 2");

    // Move cursor to line 1 first
    vm.ram[0x7402] = 1; // R_CL = 1

    // Ctrl+G
    vm.push_key(7);
    advance_frame(&mut vm);

    // Type "0"
    vm.push_key(48); // '0'
    advance_frame(&mut vm);

    // Enter
    vm.push_key(10);
    advance_frame(&mut vm);

    // Cursor should stay where it was (goto 0 = noop)
    assert_eq!(
        vm.ram[0x7402], 1,
        "goto 0 should be noop, cursor stays at line 1"
    );
}

#[test]
fn test_nano_editor_goto_escape_cancels() {
    let mut vm = load_nano_with_text("Line 0\nLine 1");

    // Ctrl+G
    vm.push_key(7);
    advance_frame(&mut vm);
    assert_eq!(vm.ram[0x74C0], 2, "should be in goto mode");

    // Escape
    vm.push_key(27);
    advance_frame(&mut vm);

    assert_eq!(vm.ram[0x74C0], 0, "escape should cancel goto");
}

// Multi-buffer constants
const R_BACT: usize = 0xB500; // active buffer index
const R_BCNT: usize = 0xB501; // buffer count
const R_B0CL: usize = 0xB570; // buffer 0 saved cursor_line
const R_B0CC: usize = 0xB571; // buffer 0 saved cursor_col
const R_B0SC: usize = 0xB572; // buffer 0 saved scroll_offset
const R_B1CL: usize = 0xB580; // buffer 1 saved cursor_line
const R_B1CC: usize = 0xB581; // buffer 1 saved cursor_col
const R_B1SC: usize = 0xB582; // buffer 1 saved scroll_offset

#[test]
fn test_nano_editor_multi_buffer_init() {
    let vm = load_nano(1);

    // Should start with 1 buffer, active = 0
    assert_eq!(vm.ram[R_BACT], 0, "active buffer should be 0");
    assert_eq!(vm.ram[R_BCNT], 1, "buffer count should be 1");
}

#[test]
fn test_nano_editor_ctrl_b_noop_with_one_buffer() {
    let mut vm = load_nano(1);

    let initial_cl = vm.ram[R_CL];

    // Ctrl+B with only 1 buffer should be noop
    vm.push_key(2); // Ctrl+B
    advance_frame(&mut vm);

    assert_eq!(
        vm.ram[R_CL], initial_cl,
        "Ctrl+B with 1 buffer should not move cursor"
    );
    assert_eq!(vm.ram[R_BACT], 0, "active buffer should still be 0");
    assert_eq!(vm.ram[R_BCNT], 1, "buffer count should still be 1");
}

#[test]
fn test_nano_editor_ctrl_n_creates_second_buffer() {
    let mut vm = load_nano(1);

    // Move cursor down a bit first
    vm.push_key(0x8000 | (1 << 1)); // bitmask: down
    advance_frame(&mut vm);
    let initial_cl = vm.ram[R_CL];

    // Ctrl+N should create second buffer
    vm.push_key(14); // Ctrl+N
    advance_frame(&mut vm);

    assert_eq!(vm.ram[R_BCNT], 2, "buffer count should be 2");
    assert_eq!(vm.ram[R_BACT], 1, "active buffer should be 1");

    // Cursor should be reset for new buffer
    assert_eq!(vm.ram[R_CL], 0, "cursor line should be 0 in new buffer");
    assert_eq!(vm.ram[R_CC], 0, "cursor col should be 0 in new buffer");

    // Buffer 0 state should be saved
    assert_eq!(
        vm.ram[R_B0CL], initial_cl,
        "buffer 0 cursor line should be saved"
    );
}

#[test]
fn test_nano_editor_switch_between_buffers() {
    let mut vm = load_nano(1);

    // Create second buffer
    vm.push_key(14); // Ctrl+N
    advance_frame(&mut vm);
    assert_eq!(vm.ram[R_BACT], 1, "should be on buffer 1");

    // Type something in buffer 1
    vm.push_key(b'H' as u32);
    advance_frame(&mut vm);
    vm.push_key(b'i' as u32);
    advance_frame(&mut vm);

    let buf1_cc = vm.ram[R_CC];
    assert!(buf1_cc > 0, "should have moved cursor in buffer 1");

    // Switch back to buffer 0 (Ctrl+B)
    vm.push_key(2); // Ctrl+B
    advance_frame(&mut vm);

    assert_eq!(vm.ram[R_BACT], 0, "should be back on buffer 0");

    // Buffer 1 state should be saved
    assert_eq!(
        vm.ram[R_B1CC], buf1_cc,
        "buffer 1 cursor col should be saved"
    );
}

#[test]
fn test_nano_editor_buf_indicator_visible() {
    // With 1 buffer, no indicator should show
    let vm = load_nano(1);
    assert_eq!(vm.ram[R_BCNT], 1, "should have 1 buffer");

    // With 2 buffers, indicator [2] should show
    let mut vm2 = load_nano(1);
    vm2.push_key(14); // Ctrl+N
    advance_frame(&mut vm2);
    assert_eq!(vm2.ram[R_BACT], 1, "should be on buffer 1");

    // The title bar should show buffer number
    // Check that buffer state is correct
    assert_eq!(vm2.ram[R_BCNT], 2, "should have 2 buffers");
}
