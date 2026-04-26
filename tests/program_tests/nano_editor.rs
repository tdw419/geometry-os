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
