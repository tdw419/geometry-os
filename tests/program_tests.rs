// Integration tests for Geometry OS programs
//
// Each test assembles a .asm file, loads it into the VM, runs it,
// and verifies the output (screen pixels, register values, etc.)

use geometry_os::assembler::assemble;
use geometry_os::vm::Vm;

/// Helper: assemble a .asm file and run it in the VM
fn compile_run(asm_path: &str) -> Vm {
    let source = std::fs::read_to_string(asm_path)
        .unwrap_or_else(|e| panic!("failed to read {}: {}", asm_path, e));
    let asm = assemble(&source)
        .unwrap_or_else(|e| panic!("assembly failed for {}: line {} {}", asm_path, e.line, e.message));
    let mut vm = Vm::new();
    // Load bytecode at address 0
    for (i, &pixel) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = pixel;
        }
    }
    vm.pc = 0;
    vm.halted = false;
    // Run up to 10M cycles
    for _ in 0..10_000_000 {
        if !vm.step() {
            break;
        }
    }
    vm
}

// ── FILL_SCREEN ──────────────────────────────────────────────────

#[test]
fn test_fill_screen() {
    let vm = compile_run("programs/fill_screen.asm");
    assert!(vm.halted, "VM should halt");
    // All screen pixels should be blue
    let blue = 0x0000FF;
    for i in 0..256 {
        for j in 0..256 {
            assert_eq!(
                vm.screen[j * 256 + i], blue,
                "pixel at ({}, {}) should be blue", i, j
            );
        }
    }
}

// ── BORDER ───────────────────────────────────────────────────────

#[test]
fn test_border() {
    let vm = compile_run("programs/border.asm");
    assert!(vm.halted, "VM should halt");
    let green = 0x00FF00;

    // Top border: row 0-3, all columns
    for x in 0..256 {
        for y in 0..4 {
            assert_eq!(vm.screen[y * 256 + x], green,
                "top border pixel at ({}, {}) should be green", x, y);
        }
    }

    // Bottom border: row 252-255
    for x in 0..256 {
        for y in 252..256 {
            assert_eq!(vm.screen[y * 256 + x], green,
                "bottom border pixel at ({}, {}) should be green", x, y);
        }
    }

    // Left border: col 0-3, rows 4-251
    for x in 0..4 {
        for y in 4..252 {
            assert_eq!(vm.screen[y * 256 + x], green,
                "left border pixel at ({}, {}) should be green", x, y);
        }
    }

    // Right border: col 252-255, rows 4-251
    for x in 252..256 {
        for y in 4..252 {
            assert_eq!(vm.screen[y * 256 + x], green,
                "right border pixel at ({}, {}) should be green", x, y);
        }
    }

    // Center pixel should be black
    assert_eq!(vm.screen[128 * 256 + 128], 0, "center should be black");
}

// ── DIAGONAL_LINE ────────────────────────────────────────────────

#[test]
fn test_diagonal() {
    let vm = compile_run("programs/diagonal.asm");
    assert!(vm.halted, "VM should halt");
    let green = 0x00FF00;

    // Diagonal pixels at (i, i) for i in 0..255 should be green
    for i in 0..256 {
        assert_eq!(vm.screen[i * 256 + i], green,
            "diagonal pixel at ({}, {}) should be green", i, i);
    }

    // Off-diagonal pixels should be black
    assert_eq!(vm.screen[0 * 256 + 1], 0, "(1, 0) should be black");
    assert_eq!(vm.screen[1 * 256 + 0], 0, "(0, 1) should be black");
}

// ── GRADIENT ─────────────────────────────────────────────────────

#[test]
fn test_gradient() {
    let vm = compile_run("programs/gradient.asm");
    assert!(vm.halted, "VM should halt");

    // Column 0 should be 0 (black)
    assert_eq!(vm.screen[0 * 256 + 0], 0, "column 0 should be black");
    // Column 255 should be 255 (blue)
    assert_eq!(vm.screen[0 * 256 + 255], 255, "column 255 should be 0xFF");
    // Column 128 should be 128
    assert_eq!(vm.screen[0 * 256 + 128], 128, "column 128 should be 0x80");

    // Every pixel in a column should have the same color (vertical line)
    for x in 0..256u32 {
        let expected = x;
        for y in 0..256 {
            assert_eq!(vm.screen[y * 256 + x as usize], expected,
                "gradient pixel at ({}, {}) should be {}", x, y, expected);
        }
    }
}

// ── STRIPES ──────────────────────────────────────────────────────

#[test]
fn test_stripes() {
    let vm = compile_run("programs/stripes.asm");
    assert!(vm.halted, "VM should halt");
    let red = 0xFF0000;
    let blue = 0x0000FF;

    // Rows 0-15 should be red
    for y in 0..16 {
        assert_eq!(vm.screen[y * 256 + 128], red,
            "row {} should be red", y);
    }
    // Rows 16-31 should be blue
    for y in 16..32 {
        assert_eq!(vm.screen[y * 256 + 128], blue,
            "row {} should be blue", y);
    }
    // Rows 32-47 should be red again
    for y in 32..48 {
        assert_eq!(vm.screen[y * 256 + 128], red,
            "row {} should be red", y);
    }
}

// ── NESTED_RECTS ─────────────────────────────────────────────────

#[test]
fn test_nested_rects() {
    let vm = compile_run("programs/nested_rects.asm");
    assert!(vm.halted, "VM should halt");

    // Corner pixels should be red (outer)
    assert_eq!(vm.screen[0], 0xFF0000, "top-left should be red");
    assert_eq!(vm.screen[255], 0xFF0000, "top-right should be red");
    assert_eq!(vm.screen[255 * 256], 0xFF0000, "bottom-left should be red");
    assert_eq!(vm.screen[255 * 256 + 255], 0xFF0000, "bottom-right should be red");

    // Inside green rectangle
    assert_eq!(vm.screen[30 * 256 + 30], 0x00FF00, "(30,30) should be green");

    // Inside blue rectangle
    assert_eq!(vm.screen[50 * 256 + 50], 0x0000FF, "(50,50) should be blue");

    // Center should be white
    assert_eq!(vm.screen[128 * 256 + 128], 0xFFFFFF, "center should be white");
}

// ── BLINK ─────────────────────────────────────────────────────────

#[test]
fn test_blink_with_keys() {
    let source = std::fs::read_to_string("programs/blink.asm")
        .unwrap_or_else(|e| panic!("failed to read: {}", e));
    let asm = assemble(&source)
        .unwrap_or_else(|e| panic!("assembly failed: {:?}", e));
    let mut vm = Vm::new();

    // Load program at address 0
    for (i, &pixel) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = pixel;
        }
    }
    vm.pc = 0;
    vm.halted = false;

    let green = 0x00FF00u32;
    let black = 0u32;
    let key_port = 0xFFFFusize;
    let center_pixel = 128 * 256 + 128;

    // Run until first poll cycle (need enough cycles for setup code)
    // Setup: ~30 instructions (constants + signature + initial PSET)
    for _ in 0..100 {
        if !vm.step() { break; }
    }

    // After setup, pixel should be green
    assert_eq!(vm.screen[center_pixel], green, "initial pixel should be green");

    // Simulate 3 keypresses, each followed by enough cycles to process
    for toggle_num in 0..3 {
        // Inject key into keyboard port
        vm.ram[key_port] = 65; // 'A'

        // Run enough cycles for the program to:
        // - LOAD the key, CMP against 0, detect key pressed
        // - Clear port, check toggle state, toggle pixel, increment counter
        // - Check if done, either loop back or halt
        for _ in 0..200 {
            if !vm.step() { break; }
        }

        // Verify port was cleared (program acknowledges the key)
        assert_eq!(vm.ram[key_port], 0, "port should be cleared after toggle {}", toggle_num + 1);

        // After each toggle, pixel alternates: green -> black -> green -> black
        let expected = if toggle_num % 2 == 0 { black } else { green };
        assert_eq!(
            vm.screen[center_pixel], expected,
            "after toggle {}, pixel should be {}",
            toggle_num + 1,
            if toggle_num % 2 == 0 { "black" } else { "green" }
        );
    }

    // After 3 toggles, program should have halted
    assert!(vm.halted, "VM should halt after 3 toggles");

    // Verify the "BLINK" signature was written
    assert_eq!(vm.ram[0x0200], 66, "B");
    assert_eq!(vm.ram[0x0201], 76, "L");
    assert_eq!(vm.ram[0x0202], 73, "I");
    assert_eq!(vm.ram[0x0203], 78, "N");
    assert_eq!(vm.ram[0x0204], 75, "K");
}

// ── SHIFT (SHL/SHR) ──────────────────────────────────────────────

#[test]
fn test_shift_operations() {
    let vm = compile_run("programs/shift_test.asm");
    assert!(vm.halted, "VM should halt");

    // Test 1: 1 << 4 = 16
    assert_eq!(vm.ram[0x0200], 16, "1 SHL 4 should be 16");

    // Test 2: 16 >> 2 = 4
    assert_eq!(vm.ram[0x0201], 4, "16 SHR 2 should be 4");

    // Test 3: 5 << 0 = 5
    assert_eq!(vm.ram[0x0202], 5, "5 SHL 0 should be 5");

    // Test 4: 1 << (36 % 32) = 1 << 4 = 16
    assert_eq!(vm.ram[0x0203], 16, "1 SHL 36 should be 16 (mod 32)");

    // Test 5: 0xFFFF >> 1 = 0x7FFF (logical shift, no sign extension)
    assert_eq!(vm.ram[0x0204], 0x7FFF, "0xFFFF SHR 1 should be 0x7FFF");

    // Test 6: (1 << 8) >> 4 = 16
    assert_eq!(vm.ram[0x0205], 16, "(1 SHL 8) SHR 4 should be 16");
}

// ── ASSEMBLER TESTS ──────────────────────────────────────────────

#[test]
fn test_all_programs_assemble() {
    let programs = [
        "programs/fill_screen.asm",
        "programs/border.asm",
        "programs/diagonal.asm",
        "programs/gradient.asm",
        "programs/stripes.asm",
        "programs/nested_rects.asm",
        "programs/blink.asm",
        "programs/painter.asm",
        "programs/calculator.asm",
        "programs/shift_test.asm",
        "programs/push_pop_test.asm",
    ];
    for path in programs {
        let source = std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("failed to read {}: {}", path, e));
        let result = assemble(&source);
        assert!(result.is_ok(), "{} should assemble: {:?}", path, result.err());
    }
}

// ── PUSH/POP ──────────────────────────────────────────────────────

#[test]
fn test_push_pop() {
    let vm = compile_run("programs/push_pop_test.asm");
    assert!(vm.halted, "VM should halt");

    // Test 1: LIFO order -- push 100, 200, 300 -> pop 300, 200, 100
    assert_eq!(vm.ram[0x0200], 300, "first pop should be 300");
    assert_eq!(vm.ram[0x0201], 200, "second pop should be 200");
    assert_eq!(vm.ram[0x0202], 100, "third pop should be 100");

    // Test 2: Same register pushed multiple times
    assert_eq!(vm.ram[0x0203], 2, "first pop of same-reg test = 2");
    assert_eq!(vm.ram[0x0204], 1, "second pop of same-reg test = 1");
    assert_eq!(vm.ram[0x0205], 0, "third pop of same-reg test = 0");

    // Test 3: SP balanced after push/pop -- push 42 then pop gives 42
    assert_eq!(vm.ram[0x0206], 42, "SP should be balanced, push/pop 42");

    // Test 4: PUSH preserves value across register reuse
    assert_eq!(vm.ram[0x0207], 777, "pushed value preserved after register clobber");

    // Test 5: Push 5 values (10,20,30,40,50), pop and sum = 150
    assert_eq!(vm.ram[0x0208], 150, "sum of 5 pushed values should be 150");
}

// ── PAINTER ────────────────────────────────────────────────────

#[test]
fn test_painter() {
    let source = std::fs::read_to_string("programs/painter.asm")
        .unwrap_or_else(|e| panic!("failed to read: {}", e));
    let asm = assemble(&source)
        .unwrap_or_else(|e| panic!("assembly failed: {:?}", e));
    let mut vm = Vm::new();

    // Load program at address 0
    for (i, &pixel) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = pixel;
        }
    }
    vm.pc = 0;
    vm.halted = false;

    let key_port = 0xFFFFusize;
    let cyan = 0x00FFFFu32;
    let center_pixel = 128 * 256 + 128;

    // Run setup (~50 instructions: constants + signature + initial PSET)
    for _ in 0..200 {
        if !vm.step() { break; }
    }

    // After setup, cursor should be at (128, 128) drawn in cyan
    assert_eq!(vm.screen[center_pixel], cyan, "initial cursor should be cyan at center");
    assert_eq!(vm.ram[0x0200], 80, "P");
    assert_eq!(vm.ram[0x0201], 65, "A");
    assert_eq!(vm.ram[0x0202], 73, "I");
    assert_eq!(vm.ram[0x0203], 78, "N");
    assert_eq!(vm.ram[0x0204], 84, "T");
    assert_eq!(vm.ram[0x0205], 69, "E");
    assert_eq!(vm.ram[0x0206], 82, "R");

    // Inject 'D' key (68) to move cursor right by 4
    vm.ram[key_port] = 68;
    for _ in 0..300 {
        if !vm.step() { break; }
    }
    assert_eq!(vm.ram[key_port], 0, "port should be cleared after D key");

    // Cursor should have moved to (132, 128) and drawn cyan there
    let moved_pixel = 128 * 256 + 132;
    assert_eq!(vm.screen[moved_pixel], cyan,
        "cursor should be at (132, 128) after D key");

    // Inject 'S' key (83) to move cursor down by 4
    vm.ram[key_port] = 83;
    for _ in 0..300 {
        if !vm.step() { break; }
    }
    assert_eq!(vm.ram[key_port], 0, "port should be cleared after S key");

    // Cursor should be at (132, 132)
    let moved_pixel2 = 132 * 256 + 132;
    assert_eq!(vm.screen[moved_pixel2], cyan,
        "cursor should be at (132, 132) after S key");

    // Inject 'W' key (87) to move cursor up by 4 (back to 128)
    vm.ram[key_port] = 87;
    for _ in 0..300 {
        if !vm.step() { break; }
    }

    // Inject 'A' key (65) to move cursor left by 4 (back to 128)
    vm.ram[key_port] = 65;
    for _ in 0..300 {
        if !vm.step() { break; }
    }

    // Cursor should be back at (128, 128)
    assert_eq!(vm.screen[center_pixel], cyan,
        "cursor should be back at (128, 128) after W+A");

    // Now paint 5 pixels with Space (32)
    for paint_num in 0..5 {
        vm.ram[key_port] = 32; // Space
        for _ in 0..300 {
            if !vm.step() { break; }
        }
        assert_eq!(vm.ram[key_port], 0,
            "port should be cleared after paint {}", paint_num + 1);
    }

    // After 5 paints, program should have halted
    assert!(vm.halted, "VM should halt after 5 paint operations");

    // The pixel at (128, 128) should be nonzero (painted)
    assert_ne!(vm.screen[center_pixel], 0,
        "pixel at cursor should be painted after space key");
}

// ── CALCULATOR ──────────────────────────────────────────────────

#[test]
fn test_calculator_add() {
    let source = std::fs::read_to_string("programs/calculator.asm")
        .unwrap_or_else(|e| panic!("failed to read: {}", e));
    let asm = assemble(&source).unwrap_or_else(|e| panic!("assembly failed: {:?}", e));
    let mut vm = Vm::new();

    for (i, &pixel) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = pixel;
        }
    }
    vm.pc = 0;
    vm.halted = false;

    let key_port = 0xFFFFusize;

    // Run setup (constants + state init)
    for _ in 0..200 {
        if !vm.step() {
            break;
        }
    }

    // Enter "12+5=": '1'=49, '2'=50, '+'=43, '5'=53, '='=61
    for &key in &[49u32, 50, 43, 53, 61] {
        vm.ram[key_port] = key;
        for _ in 0..500 {
            if !vm.step() {
                break;
            }
        }
    }

    // Extra cycles for compute + display build + TEXT render
    for _ in 0..10000 {
        if !vm.step() {
            break;
        }
    }

    assert!(vm.halted, "VM should halt after calculation");

    // Verify display string in RAM at 0x0300: "12+5=17\0"
    assert_eq!(vm.ram[0x0300], 49, "expect '1'");
    assert_eq!(vm.ram[0x0301], 50, "expect '2'");
    assert_eq!(vm.ram[0x0302], 43, "expect '+'");
    assert_eq!(vm.ram[0x0303], 53, "expect '5'");
    assert_eq!(vm.ram[0x0304], 61, "expect '='");
    assert_eq!(vm.ram[0x0305], 49, "expect '1'");
    assert_eq!(vm.ram[0x0306], 55, "expect '7'");
    assert_eq!(vm.ram[0x0307], 0, "expect null terminator");
}

#[test]
fn test_calculator_subtract() {
    let source = std::fs::read_to_string("programs/calculator.asm")
        .unwrap_or_else(|e| panic!("failed to read: {}", e));
    let asm = assemble(&source).unwrap_or_else(|e| panic!("assembly failed: {:?}", e));
    let mut vm = Vm::new();

    for (i, &pixel) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = pixel;
        }
    }
    vm.pc = 0;
    vm.halted = false;

    let key_port = 0xFFFFusize;

    // Run setup
    for _ in 0..200 {
        if !vm.step() {
            break;
        }
    }

    // Enter "20-8=": '2'=50, '0'=48, '-'=45, '8'=56, '='=61
    for &key in &[50u32, 48, 45, 56, 61] {
        vm.ram[key_port] = key;
        for _ in 0..500 {
            if !vm.step() {
                break;
            }
        }
    }

    for _ in 0..10000 {
        if !vm.step() {
            break;
        }
    }

    assert!(vm.halted, "VM should halt after subtraction");

    // Verify display string: "20-8=12\0"
    assert_eq!(vm.ram[0x0300], 50, "expect '2'");
    assert_eq!(vm.ram[0x0301], 48, "expect '0'");
    assert_eq!(vm.ram[0x0302], 45, "expect '-'");
    assert_eq!(vm.ram[0x0303], 56, "expect '8'");
    assert_eq!(vm.ram[0x0304], 61, "expect '='");
    assert_eq!(vm.ram[0x0305], 49, "expect '1'");
    assert_eq!(vm.ram[0x0306], 50, "expect '2'");
    assert_eq!(vm.ram[0x0307], 0, "expect null terminator");
}
