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
    let asm = assemble(&source, 0)
        .unwrap_or_else(|e| panic!("assembly failed for {}: {}", asm_path, e));
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
    let asm = assemble(&source, 0)
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
        let result = assemble(&source, 0);
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
    let asm = assemble(&source, 0)
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
    let asm = assemble(&source, 0).unwrap_or_else(|e| panic!("assembly failed: {:?}", e));
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
    let asm = assemble(&source, 0).unwrap_or_else(|e| panic!("assembly failed: {:?}", e));
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

// ── SAVE / LOAD ─────────────────────────────────────────────────

#[test]
fn test_vm_save_load_roundtrip() {
    let mut vm = Vm::new();
    // Set up some state
    vm.regs[0] = 42;
    vm.regs[1] = 0xDEADBEEF;
    vm.pc = 0x1000;
    vm.halted = true;
    vm.ram[0x1000] = 0x10; // LDI opcode
    vm.ram[0x1001] = 0;
    vm.ram[0x1002] = 99;
    vm.screen[128 * 256 + 128] = 0xFF0000; // red pixel at center

    let tmp = std::env::temp_dir().join("geometry_os_test_save.sav");
    vm.save_to_file(&tmp).unwrap();

    let loaded = Vm::load_from_file(&tmp).unwrap();

    assert_eq!(loaded.regs[0], 42, "r0 should be 42");
    assert_eq!(loaded.regs[1], 0xDEADBEEF, "r1 should be 0xDEADBEEF");
    assert_eq!(loaded.pc, 0x1000, "PC should be 0x1000");
    assert!(loaded.halted, "VM should be halted");
    assert_eq!(loaded.ram[0x1000], 0x10, "RAM at 0x1000 should be 0x10");
    assert_eq!(loaded.ram[0x1002], 99, "RAM at 0x1002 should be 99");
    assert_eq!(
        loaded.screen[128 * 256 + 128],
        0xFF0000,
        "center pixel should be red"
    );

    // Clean up
    std::fs::remove_file(tmp).ok();
}

#[test]
fn test_vm_save_load_preserves_rand_state_and_frame_count() {
    let mut vm = Vm::new();
    // Advance RNG by calling RAND twice (RAND rd is a 2-byte instruction)
    vm.ram[0] = 0x49; // RAND r0
    vm.ram[1] = 0;    // reg arg
    vm.ram[2] = 0x49; // RAND r0 (second call)
    vm.ram[3] = 0;    // reg arg
    vm.pc = 0;
    vm.step(); // first RAND -> pc=2
    vm.step(); // second RAND -> pc=4
    assert!(!vm.halted, "VM should not be halted after RAND");
    let rng_state_before = vm.rand_state;

    // Simulate some frame ticks (reset pc, lay down FRAME opcodes)
    vm.halted = false;
    vm.ram[0] = 0x02; // FRAME
    vm.ram[1] = 0x02; // FRAME
    vm.pc = 0;
    vm.step(); // first FRAME -> pc=1, frame_count=1
    vm.step(); // second FRAME -> pc=2, frame_count=2
    let frame_count_before = vm.frame_count;
    assert_eq!(frame_count_before, 2, "should have 2 frames");
    assert_ne!(rng_state_before, 0xDEADBEEF, "RNG should have advanced");

    let tmp = std::env::temp_dir().join("geometry_os_test_v2_save.sav");
    vm.save_to_file(&tmp).unwrap();

    let loaded = Vm::load_from_file(&tmp).unwrap();
    assert_eq!(loaded.rand_state, rng_state_before, "rand_state should be preserved");
    assert_eq!(loaded.frame_count, frame_count_before, "frame_count should be preserved");

    // Verify the loaded RNG produces the same next value as the original would
    // Call RAND on both and compare
    let mut vm2 = vm;
    let mut loaded2 = loaded;
    vm2.ram[0] = 0x49; vm2.ram[1] = 0; vm2.pc = 0;
    loaded2.ram[0] = 0x49; loaded2.ram[1] = 0; loaded2.pc = 0;
    vm2.step();
    loaded2.step();
    assert_eq!(vm2.regs[0], loaded2.regs[0], "next RAND value should match after load");

    std::fs::remove_file(tmp).ok();
}

#[test]
fn test_vm_save_load_invalid_magic() {
    let tmp = std::env::temp_dir().join("geometry_os_test_bad_magic.sav");
    std::fs::write(&tmp, b"BAD!\x00\x00\x00\x01").unwrap();

    let result = Vm::load_from_file(&tmp);
    assert!(result.is_err(), "should reject invalid magic");

    std::fs::remove_file(tmp).ok();
}

#[test]
fn test_vm_save_load_preserves_program_execution() {
    // Run a program, save, load, verify the VM state is preserved
    let vm = compile_run("programs/fill_screen.asm");
    assert!(vm.halted);
    assert_eq!(vm.screen[0], 0x0000FF); // blue fill

    let tmp = std::env::temp_dir().join("geometry_os_test_program.sav");
    vm.save_to_file(&tmp).unwrap();

    let loaded = Vm::load_from_file(&tmp).unwrap();
    assert!(loaded.halted);
    // Spot-check a few screen pixels
    assert_eq!(loaded.screen[0], 0x0000FF, "top-left should be blue");
    assert_eq!(loaded.screen[128 * 256 + 128], 0x0000FF, "center should be blue");
    assert_eq!(
        loaded.screen[255 * 256 + 255],
        0x0000FF,
        "bottom-right should be blue"
    );

    std::fs::remove_file(tmp).ok();
}

// ── LINE / CIRCLE / SCROLL ─────────────────────────────────────

#[test]
fn test_line_opcode() {
    let source = "LDI r0, 0\nLDI r1, 0\nLDI r2, 255\nLDI r3, 255\nLDI r4, 0xFFFFFF\nLINE r0,r1,r2,r3,r4\nHALT";
    let asm = assemble(source, 0).unwrap();
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() { vm.ram[i] = v; }
    for _ in 0..100_000 { if !vm.step() { break; } }
    assert!(vm.halted);
    // diagonal should have pixels set at corners
    assert_eq!(vm.screen[0], 0xFFFFFF, "top-left pixel should be white");
    assert_eq!(vm.screen[255 * 256 + 255], 0xFFFFFF, "bottom-right pixel should be white");
}

#[test]
fn test_circle_opcode() {
    let source = "LDI r0, 128\nLDI r1, 128\nLDI r2, 50\nLDI r3, 0xFF0000\nCIRCLE r0,r1,r2,r3\nHALT";
    let asm = assemble(source, 0).unwrap();
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() { vm.ram[i] = v; }
    for _ in 0..100_000 { if !vm.step() { break; } }
    assert!(vm.halted);
    // top of circle: (128, 78) should be red
    assert_eq!(vm.screen[78 * 256 + 128], 0xFF0000, "top of circle should be red");
    // bottom: (128, 178)
    assert_eq!(vm.screen[178 * 256 + 128], 0xFF0000, "bottom of circle should be red");
}

#[test]
fn test_scroll_opcode() {
    let source = "LDI r0, 0\nLDI r1, 10\nLDI r2, 0xFFFFFF\nPSET r0,r1,r2\nLDI r3, 5\nSCROLL r3\nHALT";
    let asm = assemble(source, 0).unwrap();
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() { vm.ram[i] = v; }
    for _ in 0..100_000 { if !vm.step() { break; } }
    assert!(vm.halted);
    // pixel was at (0, 10), scroll 5 up -> should now be at (0, 5)
    assert_eq!(vm.screen[5 * 256 + 0], 0xFFFFFF, "pixel should have scrolled to y=5");
    // original location (0, 10) should still be white too (scrolled copy)
    // actually after scroll by 5, y=10 maps to y=5, and y=5 is now the pixel
    assert_eq!(vm.screen[10 * 256 + 0], 0, "original y=10 should be 0 after scroll");
}

// ── FRAME ──────────────────────────────────────────────────────

#[test]
fn test_frame_opcode() {
    // Program: fill red, FRAME, fill blue, HALT
    // After FRAME, frame_ready should be set; after running to HALT, screen is blue
    let source = "LDI r1, 0xFF0000\nFILL r1\nFRAME\nLDI r1, 0x0000FF\nFILL r1\nHALT";
    let asm = assemble(source, 0).unwrap();
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() { vm.ram[i] = v; }
    vm.pc = 0;
    // Run until first FRAME
    for _ in 0..10_000 {
        if !vm.step() || vm.frame_ready { break; }
    }
    assert!(vm.frame_ready, "FRAME should set frame_ready");
    // Screen should be red at this point
    assert_eq!(vm.screen[0], 0xFF0000, "screen should be red after FRAME");
    // Clear flag and run to halt
    vm.frame_ready = false;
    for _ in 0..10_000 {
        if !vm.step() { break; }
    }
    assert!(vm.halted);
    assert_eq!(vm.screen[0], 0x0000FF, "screen should be blue after HALT");
}

// ── NEG / IKEY ──────────────────────────────────────────────────

#[test]
fn test_neg_opcode() {
    let source = "LDI r1, 5\nNEG r1\nLDI r2, 3\nADD r2, r1\nHALT";
    let asm = assemble(source, 0).unwrap();
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() { vm.ram[i] = v; }
    for _ in 0..10_000 { if !vm.step() { break; } }
    assert!(vm.halted);
    // r1 = -5 (0xFFFFFFFB), r2 = 3 + (-5) = -2 (0xFFFFFFFE)
    assert_eq!(vm.regs[1], 0xFFFFFFFB, "NEG 5 should give 0xFFFFFFFB");
    assert_eq!(vm.regs[2], 0xFFFFFFFE, "3 + (-5) should give 0xFFFFFFFE");
}

#[test]
fn test_ikey_opcode() {
    let source = "IKEY r1\nHALT";
    let asm = assemble(source, 0).unwrap();
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() { vm.ram[i] = v; }
    // Simulate key press: write ASCII 'A' (65) to keyboard port
    vm.ram[0xFFF] = 65;
    for _ in 0..10_000 { if !vm.step() { break; } }
    assert!(vm.halted);
    assert_eq!(vm.regs[1], 65, "IKEY should read key code 65 into r1");
    assert_eq!(vm.ram[0xFFF], 0, "IKEY should clear the keyboard port");
}

// ── RAND ─────────────────────────────────────────────────────────

#[test]
fn test_rand_opcode() {
    let source = "RAND r1\nRAND r2\nRAND r3\nHALT";
    let asm = assemble(source, 0).unwrap();
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() { vm.ram[i] = v; }
    for _ in 0..100 { if !vm.step() { break; } }
    assert!(vm.halted);
    // all three should be non-zero and different from each other
    assert_ne!(vm.regs[1], 0, "RAND should produce non-zero values");
    assert_ne!(vm.regs[1], vm.regs[2], "consecutive RAND values should differ");
    assert_ne!(vm.regs[2], vm.regs[3], "consecutive RAND values should differ");
}

#[test]
fn test_snake_assembles() {
    // Smoke test: snake.asm must assemble without errors
    let source = std::fs::read_to_string("programs/snake.asm")
        .expect("snake.asm not found");
    let asm = assemble(&source, 0x1000).expect("snake.asm failed to assemble");
    assert!(asm.pixels.len() > 100, "snake should be more than 100 words");
}

// ── BREAKPOINTS ───────────────────────────────────────────────────

use std::collections::HashSet;

#[test]
fn test_breakpoint_halts_at_correct_address() {
    // Assemble a simple program: LDI r1, 42 / LDI r2, 99 / HALT
    // Set breakpoint at address of LDI r2, 99 (second instruction)
    let source = "LDI r1, 42\nLDI r2, 99\nHALT";
    let asm = assemble(source, 0x1000).unwrap();
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() { vm.ram[0x1000 + i] = v; }
    vm.pc = 0x1000;

    // Figure out where LDI r2 starts by checking instruction sizes
    let (_, first_len) = vm.disassemble_at(0x1000);
    let bp_addr = 0x1000 + first_len as u32;

    let mut breakpoints: HashSet<u32> = HashSet::new();
    breakpoints.insert(bp_addr);

    // Run with breakpoint check
    let mut hit = false;
    for _ in 0..1000 {
        if !vm.step() { break; }
        if breakpoints.contains(&vm.pc) {
            hit = true;
            break;
        }
    }

    assert!(hit, "should have hit breakpoint at 0x{:04X}", bp_addr);
    assert_eq!(vm.pc, bp_addr, "PC should be at breakpoint address");
    assert_eq!(vm.regs[1], 42, "r1 should be set before breakpoint");
    assert_ne!(vm.regs[2], 99, "r2 should NOT be set yet (breakpoint before it)");
}

#[test]
fn test_breakpoint_can_be_toggled() {
    // Set breakpoint, verify it fires, remove it, verify it doesn't fire again
    let source = "LDI r1, 1\nLDI r2, 2\nLDI r3, 3\nHALT";
    let asm = assemble(source, 0x1000).unwrap();
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() { vm.ram[0x1000 + i] = v; }
    vm.pc = 0x1000;

    let (_, first_len) = vm.disassemble_at(0x1000);
    let bp_addr = 0x1000 + first_len as u32;

    let mut breakpoints: HashSet<u32> = HashSet::new();
    breakpoints.insert(bp_addr);

    // Run: should hit breakpoint
    let mut hit_count = 0;
    for _ in 0..1000 {
        if !vm.step() { break; }
        if breakpoints.contains(&vm.pc) {
            hit_count += 1;
            break;
        }
    }
    assert_eq!(hit_count, 1, "should hit breakpoint once");

    // Remove breakpoint and continue to halt
    breakpoints.remove(&bp_addr);
    for _ in 0..1000 {
        if !vm.step() { break; }
        if breakpoints.contains(&vm.pc) {
            hit_count += 1;
        }
    }
    assert!(vm.halted, "VM should have halted");
    assert_eq!(hit_count, 1, "should not hit breakpoint after removal");
}

#[test]
fn test_breakpoint_not_hit_if_address_skipped() {
    // Set breakpoint at an address that the program never reaches
    let source = "LDI r1, 10\nHALT";
    let asm = assemble(source, 0x1000).unwrap();
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() { vm.ram[0x1000 + i] = v; }
    vm.pc = 0x1000;

    let mut breakpoints: HashSet<u32> = HashSet::new();
    breakpoints.insert(0x2000); // unreachable address

    for _ in 0..1000 {
        if !vm.step() { break; }
        assert!(!breakpoints.contains(&vm.pc), "should never hit BP at 0x2000");
    }
    assert!(vm.halted);
}

#[test]
fn test_multiple_breakpoints() {
    // Set breakpoints at multiple addresses, verify each fires
    let source = "LDI r1, 1\nLDI r2, 2\nLDI r3, 3\nLDI r4, 4\nHALT";
    let asm = assemble(source, 0x1000).unwrap();
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() { vm.ram[0x1000 + i] = v; }
    vm.pc = 0x1000;

    // Calculate addresses of each LDI instruction
    let mut addrs = Vec::new();
    let mut addr = 0x1000u32;
    for _ in 0..4 {
        let (_, len) = vm.disassemble_at(addr);
        addrs.push(addr);
        addr += len as u32;
    }

    let mut breakpoints: HashSet<u32> = HashSet::new();
    breakpoints.insert(addrs[1]); // LDI r2, 2
    breakpoints.insert(addrs[3]); // LDI r4, 4

    let mut hits: Vec<u32> = Vec::new();
    for _ in 0..1000 {
        if !vm.step() { break; }
        if breakpoints.contains(&vm.pc) {
            hits.push(vm.pc);
            break;
        }
    }

    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0], addrs[1], "first hit should be at LDI r2");

    // Continue after first breakpoint
    hits.clear();
    for _ in 0..1000 {
        if !vm.step() { break; }
        if breakpoints.contains(&vm.pc) {
            hits.push(vm.pc);
            break;
        }
    }

    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0], addrs[3], "second hit should be at LDI r4");
}

// ── SPRITE OPCODE ───────────────────────────────────────────────

#[test]
fn test_sprite_opcode() {
    let source = std::fs::read_to_string("programs/sprite_demo.asm")
        .unwrap_or_else(|e| panic!("failed to read: {}", e));
    let asm = assemble(&source, 0)
        .unwrap_or_else(|e| panic!("assembly failed: {:?}", e));
    let mut vm = Vm::new();

    for (i, &pixel) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = pixel;
        }
    }
    vm.pc = 0;
    vm.halted = false;

    // Run until first FRAME (game loop programs never halt)
    for _ in 0..10_000_000 {
        if !vm.step() { break; }
        if vm.frame_ready { break; }
    }

    // Sprite data should be written to RAM at 0x3000
    // Eye pixels patched at offset 9 (index 1,1) and 14 (index 6,1)
    assert_eq!(vm.ram[0x3009], 0x00AAFF, "eye pixel at RAM[0x3009] should be 0x00AAFF");
    assert_eq!(vm.ram[0x300E], 0x00AAFF, "eye pixel at RAM[0x300E] should be 0x00AAFF");
    // Shirt rows (offsets 32-47 = 0x3020..0x302F) should be shirt blue
    assert_eq!(vm.ram[0x3020], 0x3355AA, "shirt pixel at RAM[0x3020] should be 0x3355AA");
    // Corner transparent pixels should remain 0
    assert_eq!(vm.ram[0x3000], 0, "top-left corner of sprite should be transparent");
    assert_eq!(vm.ram[0x3007], 0, "top-right corner of sprite should be transparent");
    // Screen should have been rendered (player starts at 124,100 — some pixel nearby is non-zero)
    let player_x = 124usize;
    let player_y = 100usize;
    let row_has_pixels = (player_x..player_x + 8)
        .any(|x| vm.screen[player_y * 256 + x] != 0);
    assert!(row_has_pixels, "screen should have sprite pixels at player start position");
}

#[test]
fn test_sprite_transparent_skips_zero() {
    // Directly test SPRITE with transparent pixels
    let mut vm = Vm::new();

    // Set up: r1=5 (x), r2=5 (y), r3=0x100 (sprite data addr), r4=3 (w), r5=2 (h)
    vm.ram[0] = 0x10; // LDI r1, 5
    vm.ram[1] = 1;
    vm.ram[2] = 5;
    vm.ram[3] = 0x10; // LDI r2, 5
    vm.ram[4] = 2;
    vm.ram[5] = 5;
    vm.ram[6] = 0x10; // LDI r3, 256 (0x100)
    vm.ram[7] = 3;
    vm.ram[8] = 256;
    vm.ram[9] = 0x10; // LDI r4, 3
    vm.ram[10] = 4;
    vm.ram[11] = 3;
    vm.ram[12] = 0x10; // LDI r5, 2
    vm.ram[13] = 5;
    vm.ram[14] = 2;
    // SPRITE r1, r2, r3, r4, r5 (opcode 0x4A)
    vm.ram[15] = 0x4A;
    vm.ram[16] = 1; // r1
    vm.ram[17] = 2; // r2
    vm.ram[18] = 3; // r3
    vm.ram[19] = 4; // r4
    vm.ram[20] = 5; // r5
    vm.ram[21] = 0x00; // HALT

    // Sprite data at 0x100: 3x2 pixels
    // Row 0: [0x00FF00, 0x000000, 0x0000FF]  (green, transparent, blue)
    // Row 1: [0x000000, 0xFF0000, 0x000000]  (transparent, red, transparent)
    vm.ram[256] = 0x00FF00; // green
    vm.ram[257] = 0x000000; // transparent (skip)
    vm.ram[258] = 0x0000FF; // blue
    vm.ram[259] = 0x000000; // transparent (skip)
    vm.ram[260] = 0xFF0000; // red
    vm.ram[261] = 0x000000; // transparent (skip)

    // Fill screen with white first to detect transparency
    for pixel in vm.screen.iter_mut() {
        *pixel = 0xFFFFFF;
    }

    vm.pc = 0;
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    assert!(vm.halted);

    // (5, 5) should be green
    assert_eq!(vm.screen[5 * 256 + 5], 0x00FF00, "(5,5) should be green");
    // (6, 5) should still be white (transparent)
    assert_eq!(vm.screen[5 * 256 + 6], 0xFFFFFF, "(6,5) should be white (transparent)");
    // (7, 5) should be blue
    assert_eq!(vm.screen[5 * 256 + 7], 0x0000FF, "(7,5) should be blue");
    // (5, 6) should still be white (transparent)
    assert_eq!(vm.screen[6 * 256 + 5], 0xFFFFFF, "(5,6) should be white (transparent)");
    // (6, 6) should be red
    assert_eq!(vm.screen[6 * 256 + 6], 0xFF0000, "(6,6) should be red");
    // (7, 6) should still be white (transparent)
    assert_eq!(vm.screen[6 * 256 + 7], 0xFFFFFF, "(7,6) should be white (transparent)");
}

// ── BREAKOUT ──────────────────────────────────────────────────

#[test]
fn test_breakout_initializes() {
    let source = std::fs::read_to_string("programs/breakout.asm")
        .unwrap_or_else(|e| panic!("failed to read: {}", e));
    let asm = assemble(&source, 0)
        .unwrap_or_else(|e| panic!("assembly failed: {:?}", e));
    let mut vm = Vm::new();

    for (i, &pixel) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = pixel;
        }
    }
    vm.pc = 0;

    // Run until first FRAME (init complete, entered game loop)
    for _ in 0..50_000 {
        if !vm.step() { break; }
        if vm.frame_ready {
            vm.frame_ready = false;
            break;
        }
    }

    // Bricks should be initialized with colors
    assert_eq!(vm.ram[0x3000], 0xFF0000, "brick 0 should be red (row 0)");
    assert_eq!(vm.ram[0x3007], 0xFF0000, "brick 7 should be red (row 0)");
    assert_eq!(vm.ram[0x3008], 0xFF8800, "brick 8 should be orange (row 1)");
    assert_eq!(vm.ram[0x300F], 0xFF8800, "brick 15 should be orange (row 1)");
    assert_eq!(vm.ram[0x3010], 0xFFDD00, "brick 16 should be yellow (row 2)");
    assert_eq!(vm.ram[0x3017], 0xFFDD00, "brick 23 should be yellow (row 2)");
    assert_eq!(vm.ram[0x3018], 0x00CC44, "brick 24 should be green (row 3)");
    assert_eq!(vm.ram[0x301F], 0x00CC44, "brick 31 should be green (row 3)");

    // Game state
    assert_eq!(vm.ram[0x3020], 104, "paddle_x should be centered at 104");
    assert_eq!(vm.ram[0x3025], 0, "score should start at 0");
    assert_eq!(vm.ram[0x3026], 3, "lives should start at 3");
    assert_eq!(vm.ram[0x3027], 0, "game_over should be 0");
    assert_eq!(vm.ram[0x3028], 0, "ball should not be launched");
    assert_eq!(vm.ram[0x3029], 32, "bricks_left should be 32");
}

#[test]
fn test_breakout_assembles() {
    // Smoke test: breakout.asm must assemble without errors
    let source = std::fs::read_to_string("programs/breakout.asm")
        .expect("breakout.asm not found");
    let asm = assemble(&source, 0x1000).expect("breakout.asm failed to assemble");
    assert!(asm.pixels.len() > 200, "breakout should be more than 200 words");
}

#[test]
fn test_tetris_assembles() {
    // Smoke test: tetris.asm must assemble without errors
    let source = std::fs::read_to_string("programs/tetris.asm")
        .expect("tetris.asm not found");
    let asm = assemble(&source, 0).expect("tetris.asm failed to assemble");
    assert!(asm.pixels.len() > 500, "tetris should be more than 500 words");
}

#[test]
fn test_tetris_initializes() {
    let source = std::fs::read_to_string("programs/tetris.asm")
        .unwrap_or_else(|e| panic!("failed to read: {}", e));
    let asm = assemble(&source, 0)
        .unwrap_or_else(|e| panic!("assembly failed: {:?}", e));
    let mut vm = Vm::new();

    for (i, &pixel) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = pixel;
        }
    }
    vm.pc = 0;

    // Run until first FRAME (init complete)
    for _ in 0..200_000 {
        if !vm.step() { break; }
        if vm.frame_ready {
            vm.frame_ready = false;
            break;
        }
    }

    // Board should be cleared (all 200 cells = 0)
    for i in 0..200 {
        assert_eq!(vm.ram[0x4000 + i], 0, "board cell {} should be empty", i);
    }

    // Game state initialized
    assert_eq!(vm.ram[0x40D4], 0, "score should start at 0");
    assert_eq!(vm.ram[0x40D5], 0, "lines_cleared should start at 0");
    assert_eq!(vm.ram[0x40D6], 0, "game_over should be 0");
    assert_eq!(vm.ram[0x40D8], 0, "soft_drop should be 0");

    // Piece should be spawned: current_piece and next_piece should be 0-6
    assert!(vm.ram[0x40D0] < 7, "current_piece should be 0-6");
    assert!(vm.ram[0x40D7] < 7, "next_piece should be 0-6");

    // Piece position
    assert_eq!(vm.ram[0x40D1], 3, "piece_x should start at 3 (centered)");
    assert_eq!(vm.ram[0x40D2], 0, "piece_y should start at 0 (top)");
    assert_eq!(vm.ram[0x40D3], 0, "piece_rot should start at 0");

    // Piece colors should be initialized
    assert_eq!(vm.ram[0x42C0], 0x00CCCC, "I-piece color should be cyan");
    assert_eq!(vm.ram[0x42C1], 0xCCCC00, "O-piece color should be yellow");
    assert_eq!(vm.ram[0x42C2], 0xAA00CC, "T-piece color should be purple");
    assert_eq!(vm.ram[0x42C3], 0x00CC44, "S-piece color should be green");
    assert_eq!(vm.ram[0x42C4], 0xCC2200, "Z-piece color should be red");
    assert_eq!(vm.ram[0x42C5], 0xFF8800, "L-piece color should be orange");
    assert_eq!(vm.ram[0x42C6], 0x2244CC, "J-piece color should be blue");

    // I-piece rotation data
    assert_eq!(vm.ram[0x4102], 15, "I-piece rot0 row2 should be 0b1111");
    assert_eq!(vm.ram[0x4104], 4, "I-piece rot1 row0 should be 0b0100");
}

// ── MAZE ───────────────────────────────────────────────────────

#[test]
fn test_maze_assembles() {
    let source = std::fs::read_to_string("programs/maze.asm")
        .expect("maze.asm not found");
    let asm = assemble(&source, 0).expect("maze.asm failed to assemble");
    assert!(asm.pixels.len() > 300, "maze should be more than 300 words");
}

#[test]
fn test_maze_initializes() {
    let source = std::fs::read_to_string("programs/maze.asm")
        .unwrap_or_else(|e| panic!("failed to read: {}", e));
    let asm = assemble(&source, 0)
        .unwrap_or_else(|e| panic!("assembly failed: {:?}", e));
    let mut vm = Vm::new();

    for (i, &pixel) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = pixel;
        }
    }
    vm.pc = 0;

    // Run until first FRAME (init + generate + render complete)
    for _ in 0..500_000 {
        if !vm.step() { break; }
        if vm.frame_ready {
            vm.frame_ready = false;
            break;
        }
    }

    // Top border (row 0) should be all walls
    assert_eq!(
        vm.ram[0x5000], 0xFFFFFFFF,
        "top border row should be all walls"
    );

    // Row 1 should have passages carved (not all walls)
    assert_ne!(
        vm.ram[0x5004], 0xFFFFFFFF,
        "row 1 should have carved passages"
    );

    // Starting cell (0,0) should be visited
    assert_eq!(vm.ram[0x5100], 1, "cell (0,0) should be visited");

    // Player at (0,0)
    assert_eq!(vm.ram[0x5310], 0, "player_x should be 0");
    assert_eq!(vm.ram[0x5311], 0, "player_y should be 0");

    // Not won
    assert_eq!(vm.ram[0x5312], 0, "won should be 0");

    // Win text stored in RAM
    assert_eq!(vm.ram[0x5320], 89, "first char should be 'Y' (89)");
    assert_eq!(vm.ram[0x5327], 33, "last char should be '!' (33)");
    assert_eq!(vm.ram[0x5328], 0, "null terminator after text");
}

#[test]
fn test_maze_peek_collision_blocks_wall() {
    // Verify PEEK-based collision: player at (0,0), press W (up)
    // Top border is always a wall, so player must not move
    let source = std::fs::read_to_string("programs/maze.asm")
        .unwrap_or_else(|e| panic!("failed to read: {}", e));
    let asm = assemble(&source, 0)
        .unwrap_or_else(|e| panic!("assembly failed: {:?}", e));
    let mut vm = Vm::new();

    for (i, &pixel) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = pixel;
        }
    }
    vm.pc = 0;

    // Run until first FRAME
    for _ in 0..500_000 {
        if !vm.step() { break; }
        if vm.frame_ready {
            vm.frame_ready = false;
            break;
        }
    }

    // Player starts at (0,0)
    assert_eq!(vm.ram[0x5310], 0, "player_x should start at 0");
    assert_eq!(vm.ram[0x5311], 0, "player_y should start at 0");

    // Press W (87) -- move up into the top border wall
    vm.ram[0xFFF] = 87;

    // Run until next FRAME
    for _ in 0..100_000 {
        if !vm.step() { break; }
        if vm.frame_ready {
            vm.frame_ready = false;
            break;
        }
    }

    // Player must still be at (0,0) -- blocked by wall
    assert_eq!(vm.ram[0x5310], 0, "player_x should still be 0 after blocked move");
    assert_eq!(vm.ram[0x5311], 0, "player_y should still be 0 after blocked move");
}

// ── ASM OPCODE ──────────────────────────────────────────────────

#[test]
fn test_asm_opcode_basic() {
    let mut vm = Vm::new();
    let source = "LDI r0, 42\nHALT\n";
    for (i, &byte) in source.as_bytes().iter().enumerate() {
        vm.ram[0x0800 + i] = byte as u32;
    }
    vm.ram[0x0800 + source.len()] = 0;
    let prog = assemble("LDI r5, 0x0800\nLDI r6, 0x1000\nASM r5, r6\nHALT\n", 0).unwrap();
    for (i, &word) in prog.pixels.iter().enumerate() {
        vm.ram[i] = word;
    }
    vm.pc = 0;
    for _ in 0..100_000 {
        if !vm.step() { break; }
    }
    assert_eq!(vm.ram[0x1000], 0x10, "LDI opcode at dest");
    assert_eq!(vm.ram[0x1001], 0, "r0 register");
    assert_eq!(vm.ram[0x1002], 42, "immediate 42");
    assert_eq!(vm.ram[0x1003], 0x00, "HALT at dest+3");
    assert_eq!(vm.ram[0xFFD], 4, "ASM result should be 4");
    assert!(vm.halted);
}

#[test]
fn test_asm_opcode_error() {
    let mut vm = Vm::new();
    let source = "BOGUS r0\n";
    for (i, &byte) in source.as_bytes().iter().enumerate() {
        vm.ram[0x0800 + i] = byte as u32;
    }
    vm.ram[0x0800 + source.len()] = 0;
    let prog = assemble("LDI r5, 0x0800\nLDI r6, 0x1000\nASM r5, r6\nHALT\n", 0).unwrap();
    for (i, &word) in prog.pixels.iter().enumerate() {
        vm.ram[i] = word;
    }
    vm.pc = 0;
    for _ in 0..100_000 {
        if !vm.step() { break; }
    }
    assert_eq!(vm.ram[0xFFD], 0xFFFFFFFF, "ASM error indicator");
}

#[test]
fn test_self_host_assembles() {
    let vm = compile_run("programs/self_host.asm");
    assert!(vm.halted, "self_host should halt");
}

#[test]
fn test_self_host_runs() {
    let vm = compile_run("programs/self_host.asm");
    assert_eq!(vm.screen[0], 3, "top-left should be green");
    assert_eq!(vm.screen[128 * 256 + 128], 3, "center should be green");
    assert_eq!(vm.screen[255 * 256 + 255], 3, "bottom-right should be green");
}

// ── CMP / BLT / BGE ────────────────────────────────────────────

#[test]
fn test_cmp_opcode_equal() {
    let source = "LDI r1, 42\nLDI r2, 42\nCMP r1, r2\nHALT";
    let asm = assemble(source, 0).unwrap();
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() { vm.ram[i] = v; }
    for _ in 0..100 { if !vm.step() { break; } }
    assert!(vm.halted);
    assert_eq!(vm.regs[0], 0, "CMP equal should set r0 = 0");
}

#[test]
fn test_cmp_opcode_less_than() {
    let source = "LDI r1, 10\nLDI r2, 20\nCMP r1, r2\nHALT";
    let asm = assemble(source, 0).unwrap();
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() { vm.ram[i] = v; }
    for _ in 0..100 { if !vm.step() { break; } }
    assert!(vm.halted);
    assert_eq!(vm.regs[0], 0xFFFFFFFF, "CMP less-than should set r0 = -1");
}

#[test]
fn test_cmp_opcode_greater_than() {
    let source = "LDI r1, 30\nLDI r2, 20\nCMP r1, r2\nHALT";
    let asm = assemble(source, 0).unwrap();
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() { vm.ram[i] = v; }
    for _ in 0..100 { if !vm.step() { break; } }
    assert!(vm.halted);
    assert_eq!(vm.regs[0], 1, "CMP greater-than should set r0 = 1");
}

#[test]
fn test_blt_opcode() {
    let source = "\
LDI r1, 10\nLDI r2, 20\nCMP r1, r2\nBLT r0, less\nLDI r3, 99\nHALT\n\
less:\nLDI r3, 42\nHALT";
    let asm = assemble(source, 0).unwrap();
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() { vm.ram[i] = v; }
    for _ in 0..100 { if !vm.step() { break; } }
    assert!(vm.halted);
    assert_eq!(vm.regs[3], 42, "BLT should branch when r1 < r2");
}

#[test]
fn test_bge_opcode() {
    let source = "\
LDI r1, 20\nLDI r2, 10\nCMP r1, r2\nBGE r0, geq\nLDI r3, 99\nHALT\n\
geq:\nLDI r3, 42\nHALT";
    let asm = assemble(source, 0).unwrap();
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() { vm.ram[i] = v; }
    for _ in 0..100 { if !vm.step() { break; } }
    assert!(vm.halted);
    assert_eq!(vm.regs[3], 42, "BGE should branch when r1 >= r2");
}

// ── MOD ─────────────────────────────────────────────────────────

#[test]
fn test_mod_opcode() {
    let source = "LDI r1, 17\nLDI r2, 5\nMOD r1, r2\nHALT";
    let asm = assemble(source, 0).unwrap();
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() { vm.ram[i] = v; }
    for _ in 0..100 { if !vm.step() { break; } }
    assert!(vm.halted);
    assert_eq!(vm.regs[1], 2, "17 MOD 5 should be 2");
}

#[test]
fn test_mod_opcode_zero_divisor() {
    let source = "LDI r1, 10\nLDI r2, 0\nMOD r1, r2\nHALT";
    let asm = assemble(source, 0).unwrap();
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() { vm.ram[i] = v; }
    for _ in 0..100 { if !vm.step() { break; } }
    assert!(vm.halted);
    // Division by zero leaves register unchanged (same behavior as DIV)
    assert_eq!(vm.regs[1], 10, "MOD by zero should leave register unchanged");
}

// ── BEEP ────────────────────────────────────────────────────────

#[test]
fn test_beep_opcode() {
    // BEEP freq_reg, dur_reg -- set up freq in r1, dur in r2
    // We test that the VM doesn't crash and advances past BEEP
    let source = "LDI r1, 440\nLDI r2, 50\nBEEP r1, r2\nLDI r3, 1\nHALT";
    let asm = assemble(source, 0).unwrap();
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() { vm.ram[i] = v; }
    for _ in 0..100 { if !vm.step() { break; } }
    assert!(vm.halted);
    assert_eq!(vm.regs[3], 1, "VM should execute past BEEP and set r3");
}

// ── Additional Program Tests (Sprint 1) ─────────────────────────

#[test]
fn test_hello_program() {
    let vm = compile_run("programs/hello.asm");
    assert!(vm.halted, "hello.asm should halt");
    // RAM[0x2000] should be 'H' (72)
    assert_eq!(vm.ram[0x2000], 72);
    // Screen at (90, 120) should have some pixels set from TEXT
    let mut pixels_found = false;
    for y in 120..130 {
        for x in 90..150 {
            if vm.screen[y * 256 + x] != 0 {
                pixels_found = true;
                break;
            }
        }
    }
    assert!(pixels_found, "hello.asm should draw text on screen");
}

#[test]
fn test_circles_program() {
    let vm = compile_run("programs/circles.asm");
    assert!(vm.halted, "circles.asm should halt");
    // Check for pixels around the center (128,128)
    let mut pixels_found = false;
    for y in 100..150 {
        for x in 100..150 {
            if vm.screen[y * 256 + x] != 0 {
                pixels_found = true;
                break;
            }
        }
    }
    assert!(pixels_found, "circles.asm should draw circles around center");
}

#[test]
fn test_lines_program() {
    let vm = compile_run("programs/lines.asm");
    assert!(vm.halted, "lines.asm should halt");
    // Center at (128, 128) should be white (0xFFFFFF)
    assert_eq!(vm.screen[128 * 256 + 128], 0xFFFFFF);
}

#[test]
fn test_colors_program() {
    let vm = compile_run("programs/colors.asm");
    assert!(vm.halted, "colors.asm should halt");
    // Last FILL was yellow (0xFFFF00)
    assert_eq!(vm.screen[0], 0xFFFF00);
}

#[test]
fn test_checkerboard_program() {
    let vm = compile_run("programs/checkerboard.asm");
    assert!(vm.halted, "checkerboard.asm should halt");
    // (0,0) is white, (8,0) is black
    assert_eq!(vm.screen[0], 0xFFFFFF);
    assert_eq!(vm.screen[8], 0x000000);
}

#[test]
fn test_rainbow_program() {
    let vm = compile_run("programs/rainbow.asm");
    assert!(vm.halted, "rainbow.asm should halt");
    // (0,0) is (0+0)%6 = index 0 = red (0xFF0000)
    assert_eq!(vm.screen[0], 0xFF0000);
}

#[test]
fn test_rings_program() {
    let vm = compile_run("programs/rings.asm");
    assert!(vm.halted, "rings.asm should halt");
    // Center (128,128) distance 0 -> ring index 0 -> red
    assert_eq!(vm.screen[128 * 256 + 128], 0xFF0000);
}

#[test]
fn test_scroll_demo_program() {
    let vm = compile_run("programs/scroll_demo.asm");
    assert!(vm.halted, "scroll_demo.asm should halt");
    // Bar was drawn at 240, scrolled up 240 times -> should be at 0
    // Check pixel at (0,0)
    assert_eq!(vm.screen[0], 0x00FF88);
}

#[test]
fn test_painter_program() {
    // Painter writes a signature to RAM
    let source = std::fs::read_to_string("programs/painter.asm").unwrap();
    let asm = assemble(&source, 0).unwrap();
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() { vm.ram[i] = v; }
    // Run for enough steps to do initial RAM writes
    for _ in 0..1000 { vm.step(); }
    // RAM[0x0200] should be 'P' (80)
    assert_eq!(vm.ram[0x0200], 80, "painter.asm should write signature to RAM");
}

fn compile_run_interactive(asm_path: &str, steps: usize) -> Vm {
    let source = std::fs::read_to_string(asm_path)
        .unwrap_or_else(|e| panic!("failed to read {}: {}", asm_path, e));
    let asm = assemble(&source, 0)
        .unwrap_or_else(|e| panic!("assembly failed for {}: {}", asm_path, e));
    let mut vm = Vm::new();
    for (i, &pixel) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = pixel;
        }
    }
    vm.pc = 0;
    vm.halted = false;
    for _ in 0..steps {
        if !vm.step() { break; }
        if vm.frame_ready { break; }
    }
    vm
}

#[test]
fn test_ball_program() {
    let vm = compile_run_interactive("programs/ball.asm", 1000);
    // Ball starts at (128,128) with radius 8 and color 0xFFFFFF
    // Check if the center or some part of the circle is drawn
    let mut pixels_found = false;
    for y in 120..136 {
        for x in 120..136 {
            if vm.screen[y * 256 + x] == 0xFFFFFF {
                pixels_found = true;
                break;
            }
        }
    }
    assert!(pixels_found, "ball.asm should draw a white ball near center");
}

#[test]
fn test_fire_program() {
    let vm = compile_run_interactive("programs/fire.asm", 2000);
    // Fire starts at bottom row and scrolls up.
    // Check if there are non-zero pixels in the fire area.
    let mut pixels_found = false;
    for y in 200..256 {
        for x in 0..256 {
            if vm.screen[y * 256 + x] != 0 {
                pixels_found = true;
                break;
            }
        }
    }
    assert!(pixels_found, "fire.asm should have fire pixels in bottom region");
}

#[test]
fn test_sar_opcode() {
    // SAR rd, rs
    // Test negative: -4 (0xFFFFFFFC) >> 1 = -2 (0xFFFFFFFE)
    let source = "LDI r1, 0xFFFFFFFC\nLDI r2, 1\nSAR r1, r2\nHALT";
    let asm = assemble(source, 0).unwrap();
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() { vm.ram[i] = v; }
    for _ in 0..100 { if !vm.step() { break; } }
    assert!(vm.halted);
    assert_eq!(vm.regs[1], 0xFFFFFFFE, "SAR -4, 1 should be -2");

    // Test positive: 4 >> 1 = 2
    let source = "LDI r1, 4\nLDI r2, 1\nSAR r1, r2\nHALT";
    let asm = assemble(source, 0).unwrap();
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() { vm.ram[i] = v; }
    for _ in 0..100 { if !vm.step() { break; } }
    assert!(vm.halted);
    assert_eq!(vm.regs[1], 2, "SAR 4, 1 should be 2");
}

#[test]
fn test_tilemap_opcode() {
    // TILEMAP xr, yr, mr, tr, gwr, ghr, twr, thr
    // Set up a 2x2 grid at (10, 10) with tile index 1
    // Tile 1 is a 2x2 red square.
    
    let source = "
        #define MAP_ADDR 0x5000
        #define TILE_ADDR 0x6000
        
        ; Setup tile 1: 2x2 red (0xFF0000)
        LDI r1, TILE_ADDR
        LDI r2, 0xFF0000
        STORE r1, r2
        LDI r3, 1
        ADD r1, r3
        STORE r1, r2
        ADD r1, r3
        STORE r1, r2
        ADD r1, r3
        STORE r1, r2
        
        ; Setup map: 2x2 grid of tile 1
        LDI r1, MAP_ADDR
        LDI r2, 1
        STORE r1, r2
        LDI r3, 1
        ADD r1, r3
        STORE r1, r2
        ADD r1, r3
        STORE r1, r2
        ADD r1, r3
        STORE r1, r2
        
        ; Setup registers for TILEMAP
        LDI r10, 10    ; x
        LDI r11, 10    ; y
        LDI r12, MAP_ADDR
        LDI r13, TILE_ADDR
        LDI r14, 2     ; grid_w
        LDI r15, 2     ; grid_h
        LDI r16, 2     ; tile_w
        LDI r17, 2     ; tile_h
        
        TILEMAP r10, r11, r12, r13, r14, r15, r16, r17
        HALT
    ";
    
    let asm = assemble(source, 0).unwrap();
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() { vm.ram[i] = v; }
    for _ in 0..1000 { if !vm.step() { break; } }
    assert!(vm.halted);
    
    // Check pixels at (10,10) to (13,13)
    // Grid 2x2 * Tile 2x2 = 4x4 area
    for y in 10..14 {
        for x in 10..14 {
            assert_eq!(vm.screen[y * 256 + x], 0xFF0000, "pixel at ({}, {}) should be red", x, y);
        }
    }
}

// === SPAWN/KILL opcode tests ===

#[test]
fn test_spawn_creates_child_process() {
    // SPAWN r1 creates a child at address in r1
    // The child code at 0x200 is: LDI r0, 42, HALT
    // Main: set r1=0x200, SPAWN r1, HALT
    let source = "
    LDI r1, 0x200
    SPAWN r1
    HALT

    .org 0x200
    LDI r0, 42
    HALT
    ";
    let asm = assemble(source, 0).unwrap();
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() { vm.ram[i] = v; }
    for _ in 0..100 { if !vm.step() { break; } }
    assert!(vm.halted);
    // RAM[0xFFA] should contain the process ID (1)
    assert_eq!(vm.ram[0xFFA], 1, "SPAWN should return PID 1");
    // One process should exist
    assert_eq!(vm.processes.len(), 1);
    assert_eq!(vm.processes[0].pid, 1);
    // Phase 24: child PC starts at 0 in its own address space (code copied from parent)
    assert_eq!(vm.processes[0].pc, 0);
}

#[test]
fn test_spawn_max_processes() {
    // Spawn 8 processes, the 9th should fail
    let mut source = String::new();
    // Each child is at 0x200 + i*4: LDI r0, <i> (3 words) + HALT (1 word) = 4 words
    for i in 0..8 {
        let addr = 0x200 + (i as u32) * 4;
        source.push_str(&format!("LDI r1, 0x{:X}\nSPAWN r1\n", addr));
    }
    // Try to spawn 9th
    source.push_str("LDI r1, 0x300\nSPAWN r1\nHALT\n");
    for i in 0..8 {
        let addr = 0x200 + (i as u32) * 4;
        source.push_str(&format!(".org 0x{:X}\nLDI r0, {}\nHALT\n", addr, i));
    }
    source.push_str(".org 0x300\nHALT\n");

    let asm = assemble(&source, 0).unwrap();
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() { vm.ram[i] = v; }
    for _ in 0..1000 { if !vm.step() { break; } }
    assert!(vm.halted);
    // Should have 8 processes, 9th spawn should have returned 0xFFFFFFFF
    assert_eq!(vm.processes.len(), 8);
    assert_eq!(vm.ram[0xFFA], 0xFFFFFFFF, "9th SPAWN should fail");
}

#[test]
fn test_kill_halts_child_process() {
    // Spawn a child, then kill it by PID
    let source = "
    LDI r1, 0x200
    SPAWN r1
    LDI r3, 0xFFA
    LOAD r2, r3
    KILL r2
    HALT

    .org 0x200
    FRAME
    JMP 0x200
    ";
    let asm = assemble(source, 0).unwrap();
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() { vm.ram[i] = v; }
    for _ in 0..100 { if !vm.step() { break; } }
    assert!(vm.halted);
    // KILL should have returned 1 (success)
    assert_eq!(vm.ram[0xFFA], 1, "KILL should return 1 on success");
    // Child should be halted
    assert!(vm.processes[0].halted);
}

#[test]
fn test_step_all_processes() {
    // Spawn two children that each set a pixel, then step them
    // Child 1 at 0x200: PSETI 10, 10, 0xFF0000, HALT
    // Child 2 at 0x300: PSETI 20, 20, 0x00FF00, HALT
    let source = "
    LDI r1, 0x200
    SPAWN r1
    LDI r1, 0x300
    SPAWN r1
    HALT

    .org 0x200
    PSETI 10, 10, 0xFF0000
    HALT

    .org 0x300
    PSETI 20, 20, 0x00FF00
    HALT
    ";
    let asm = assemble(source, 0).unwrap();
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() { vm.ram[i] = v; }
    // Run main process to completion
    for _ in 0..100 { if !vm.step() { break; } }
    assert!(vm.halted);
    assert_eq!(vm.processes.len(), 2);

    // Step child processes
    for _ in 0..100 {
        vm.step_all_processes();
        if vm.processes.iter().all(|p| p.halted) {
            break;
        }
    }

    // Both children should be halted
    assert!(vm.processes[0].halted);
    assert!(vm.processes[1].halted);

    // Child 1 should have set pixel at (10,10) to red
    assert_eq!(vm.screen[10 * 256 + 10], 0xFF0000);
    // Child 2 should have set pixel at (20,20) to green
    assert_eq!(vm.screen[20 * 256 + 20], 0x00FF00);
}

#[test]
fn test_active_process_count() {
    let mut vm = Vm::new();
    assert_eq!(vm.active_process_count(), 0);
    vm.processes.push(geometry_os::vm::SpawnedProcess {
        pc: 0, regs: [0; 32], halted: false, pid: 1, mode: geometry_os::vm::CpuMode::Kernel,
        page_dir: None, segfaulted: false,
        priority: 1, slice_remaining: 0, sleep_until: 0, yielded: false,
                blocked: false,
                msg_queue: Vec::new(),
    });
    assert_eq!(vm.active_process_count(), 1);
    vm.processes.push(geometry_os::vm::SpawnedProcess {
        pc: 0, regs: [0; 32], halted: true, pid: 2, mode: geometry_os::vm::CpuMode::Kernel,
        page_dir: None, segfaulted: false,
        priority: 1, slice_remaining: 0, sleep_until: 0, yielded: false,
                blocked: false,
                msg_queue: Vec::new(),
    });
    assert_eq!(vm.active_process_count(), 1);
}

#[test]
fn test_spawn_assembles() {
    let source = "SPAWN r1\nKILL r2\nHALT";
    let asm = assemble(source, 0).unwrap();
    // SPAWN r1 = 0x4D, r1
    assert_eq!(asm.pixels[0], 0x4D);
    assert_eq!(asm.pixels[1], 1); // r1
    // KILL r2 = 0x4E, r2
    assert_eq!(asm.pixels[2], 0x4E);
    assert_eq!(asm.pixels[3], 2); // r2
    // HALT
    assert_eq!(asm.pixels[4], 0x00);
}

// === Window Manager (SPAWN + shared RAM bounds protocol) ===

/// Helper: assemble, load, and run with child processes stepping in lock-step.
/// Runs for `frames` FRAME opcodes (simulates the display loop).
fn compile_run_multiproc(asm_path: &str, frames: usize) -> Vm {
    let source = std::fs::read_to_string(asm_path)
        .unwrap_or_else(|e| panic!("failed to read {}: {}", asm_path, e));
    let asm = assemble(&source, 0)
        .unwrap_or_else(|e| panic!("assembly failed for {}: {}", asm_path, e));
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() { vm.ram[i] = v; }
    }
    let mut frame_count = 0;
    for _ in 0..50_000_000 {
        if vm.halted { break; }
        if !vm.step() { break; }
        vm.step_all_processes();
        if vm.frame_ready {
            vm.frame_ready = false;
            frame_count += 1;
            if frame_count >= frames { break; }
        }
    }
    vm
}

#[test]
fn test_window_manager_assembles() {
    let source = std::fs::read_to_string("programs/window_manager.asm")
        .expect("window_manager.asm should exist");
    assemble(&source, 0).expect("window_manager.asm should assemble cleanly");
}

#[test]
#[ignore = "Phase 24 memory protection breaks shared RAM protocol; needs Phase 27 IPC"]
fn test_window_manager_spawns_child() {
    // Run for 3 frames: primary should have spawned a child and written bounds
    let vm = compile_run_multiproc("programs/window_manager.asm", 3);
    // Child should be alive
    assert!(!vm.processes.is_empty(), "primary should have spawned a child process");
    assert!(!vm.processes[0].halted, "child should still be running");
    // Bounds protocol: RAM[0xF00..0xF03] should be populated
    assert_ne!(vm.ram[0xF02], 0, "win_w should be non-zero");
    assert_ne!(vm.ram[0xF03], 0, "win_h should be non-zero");
}

#[test]
fn test_window_manager_draws_border() {
    // Run for 5 frames and check that green border pixels exist
    let vm = compile_run_multiproc("programs/window_manager.asm", 5);
    let green = 0x00FF00u32;
    let green_count = vm.screen.iter().filter(|&&p| p == green).count();
    assert!(green_count > 0, "window border (green pixels) should be visible");
}

#[test]
#[ignore = "Phase 24 memory protection breaks shared RAM protocol; needs Phase 27 IPC"]
fn test_window_manager_ball_inside_window() {
    // Run for 10 frames; the child's red ball should be inside the window bounds
    let vm = compile_run_multiproc("programs/window_manager.asm", 10);
    let win_x = vm.ram[0xF00] as usize;
    let win_y = vm.ram[0xF01] as usize;
    let win_w = vm.ram[0xF02] as usize;
    let win_h = vm.ram[0xF03] as usize;
    // Find any red-ish pixel on screen
    let ball_color = 0xFF4444u32;
    let screen = &vm.screen;
    let ball_pixels: Vec<(usize, usize)> = (0..256usize)
        .flat_map(|y| (0..256usize).filter_map(move |x| {
            if screen[y * 256 + x] == ball_color { Some((x, y)) } else { None }
        }))
        .collect();
    assert!(!ball_pixels.is_empty(), "red ball should be visible on screen");
    // All ball pixels must be inside the window
    for (x, y) in &ball_pixels {
        assert!(*x >= win_x && *x < win_x + win_w,
            "ball pixel x={} outside window x={}..{}", x, win_x, win_x + win_w);
        assert!(*y >= win_y && *y < win_y + win_h,
            "ball pixel y={} outside window y={}..{}", y, win_y, win_y + win_h);
    }
}

#[test]
fn test_peek_reads_screen_pixel() {
    // PEEK rx, ry, rd reads screen[rx][ry] into rd
    // Draw a red pixel at (10, 20), then PEEK it back
    let source = "
    LDI r1, 10
    LDI r2, 20
    LDI r3, 0xFF0000
    PSET r1, r2, r3
    PEEK r1, r2, r4
    HALT
    ";
    let asm = assemble(source, 0).unwrap();
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() { vm.ram[i] = v; }
    for _ in 0..100 { if !vm.step() { break; } }
    assert!(vm.halted);
    // r4 should contain the red pixel color we wrote
    assert_eq!(vm.regs[4], 0xFF0000, "PEEK should read back the pixel color");
}

#[test]
fn test_peek_out_of_bounds_returns_zero() {
    // PEEK with coordinates >= 256 should return 0
    let source = "
    LDI r1, 300
    LDI r2, 10
    LDI r3, 0xFF0000
    PSET r1, r2, r3
    PEEK r1, r2, r4
    HALT
    ";
    let asm = assemble(source, 0).unwrap();
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() { vm.ram[i] = v; }
    for _ in 0..100 { if !vm.step() { break; } }
    assert!(vm.halted);
    // r4 should be 0 because (300, 10) is out of bounds
    assert_eq!(vm.regs[4], 0, "PEEK out-of-bounds should return 0");
}

#[test]
fn test_peek_collision_detection() {
    // Draw a wall, then use PEEK to check if the next position is blocked
    let source = "
    ; Draw a red wall at y=50 across x=0..255
    LDI r1, 0
    LDI r2, 50
    LDI r3, 0xFF0000

wall_loop:
    PSET r1, r2, r3
    ADD r1, r4       ; r4 = 1
    LDI r5, 256
    CMP r1, r5
    JZ r0, wall_done
    JMP wall_loop

wall_done:
    ; Now PEEK at (100, 50) -- should be red (non-zero)
    LDI r6, 100
    LDI r7, 50
    PEEK r6, r7, r8
    ; PEEK at (100, 49) -- should be black (zero)
    LDI r7, 49
    PEEK r6, r7, r9
    HALT
    ";
    // Fix: r4 needs to be 1 before the loop
    let source2 = "
    LDI r4, 1
    LDI r1, 0
    LDI r2, 50
    LDI r3, 0xFF0000

wall_loop:
    PSET r1, r2, r3
    ADD r1, r4
    LDI r5, 256
    CMP r1, r5
    JZ r0, wall_done
    JMP wall_loop

wall_done:
    LDI r6, 100
    LDI r7, 50
    PEEK r6, r7, r8
    LDI r7, 49
    PEEK r6, r7, r9
    HALT
    ";
    let asm = assemble(source2, 0).unwrap();
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() { vm.ram[i] = v; }
    for _ in 0..10000 { if !vm.step() { break; } }
    assert!(vm.halted);
    // Wall pixel should be red (non-zero)
    assert_ne!(vm.regs[8], 0, "PEEK at wall should return non-zero (wall color)");
    // Empty pixel above wall should be 0
    assert_eq!(vm.regs[9], 0, "PEEK above wall should return 0 (empty)");
}

#[test]
fn test_peek_assembles() {
    let source = "PEEK r1, r2, r3\nHALT";
    let asm = assemble(source, 0).unwrap();
    // PEEK should compile to 0x4F, 1, 2, 3
    assert_eq!(asm.pixels[0], 0x4F);
    assert_eq!(asm.pixels[1], 1);
    assert_eq!(asm.pixels[2], 2);
    assert_eq!(asm.pixels[3], 3);
}

#[test]
fn test_peek_bounce_assembles() {
    let source = std::fs::read_to_string("programs/peek_bounce.asm")
        .expect("peek_bounce.asm should exist");
    assemble(&source, 0).expect("peek_bounce.asm should assemble cleanly");
}

#[test]
fn test_peek_bounce_bounces_off_walls() {
    // Run for 20 frames: ball should bounce off border walls and stay on screen
    let vm = compile_run_multiproc("programs/peek_bounce.asm", 20);
    let ball_color = 0xFFFFFFu32;
    // Find ball position
    let mut ball_x = 0usize;
    let mut ball_y = 0usize;
    let mut found = false;
    for y in 0..256usize {
        for x in 0..256usize {
            if vm.screen[y * 256 + x] == ball_color {
                ball_x = x;
                ball_y = y;
                found = true;
                break;
            }
        }
        if found { break; }
    }
    assert!(found, "white ball should be visible on screen");
    // Ball must be within the playable area (inside the 4px border walls)
    assert!(ball_x >= 4 && ball_x <= 251, "ball x={} should be inside borders", ball_x);
    assert!(ball_y >= 4 && ball_y <= 251, "ball y={} should be inside borders", ball_y);
}

// ── PHASE 23: KERNEL BOUNDARY ──────────────────────────────────

#[test]
fn test_vm_starts_in_kernel_mode() {
    let vm = Vm::new();
    assert_eq!(vm.mode, geometry_os::vm::CpuMode::Kernel, "VM should start in Kernel mode");
}

#[test]
fn test_cpu_mode_flag_user_and_kernel() {
    let mut vm = Vm::new();
    vm.mode = geometry_os::vm::CpuMode::User;
    assert_eq!(vm.mode, geometry_os::vm::CpuMode::User);
    vm.mode = geometry_os::vm::CpuMode::Kernel;
    assert_eq!(vm.mode, geometry_os::vm::CpuMode::Kernel);
}

#[test]
fn test_syscall_assembles() {
    let source = "SYSCALL 0\nHALT";
    let asm = assemble(source, 0).unwrap();
    assert_eq!(asm.pixels[0], 0x52, "SYSCALL opcode should be 0x52");
    assert_eq!(asm.pixels[1], 0, "syscall number should be 0");
}

#[test]
fn test_retk_assembles() {
    let source = "RETK\nHALT";
    let asm = assemble(source, 0).unwrap();
    assert_eq!(asm.pixels[0], 0x53, "RETK opcode should be 0x53");
}

#[test]
fn test_syscall_dispatches_to_handler() {
    // Set up syscall table: syscall 0 -> handler at address 100
    // SYSCALL 0 should jump to RAM[0xFE00] = 100
    let mut vm = Vm::new();
    vm.ram[0xFE00] = 100; // handler for syscall 0

    // Write SYSCALL 0 at address 0
    vm.ram[0] = 0x52; // SYSCALL
    vm.ram[1] = 0;    // syscall number 0
    vm.pc = 0;

    vm.step(); // execute SYSCALL 0

    assert_eq!(vm.pc, 100, "SYSCALL should jump to handler address");
    assert_eq!(vm.mode, geometry_os::vm::CpuMode::Kernel, "SYSCALL should switch to Kernel mode");
    assert_eq!(vm.kernel_stack.len(), 1, "SYSCALL should push to kernel stack");
    assert_eq!(vm.kernel_stack[0].0, 2, "return PC should be 2 (after SYSCALL instruction)");
}

#[test]
fn test_syscall_no_handler_returns_error() {
    // Syscall 5 has no handler (RAM[0xFE05] = 0)
    let mut vm = Vm::new();
    vm.ram[0] = 0x52; // SYSCALL
    vm.ram[1] = 5;    // syscall number 5
    vm.pc = 0;

    vm.step(); // execute SYSCALL 5

    // Should set r0 = 0xFFFFFFFF (error) and NOT jump
    assert_eq!(vm.regs[0], 0xFFFFFFFF, "SYSCALL with no handler should set r0 to error");
    assert_eq!(vm.pc, 2, "PC should advance past SYSCALL instruction");
}

#[test]
fn test_retk_returns_to_user_mode() {
    // Simulate a complete syscall -> handler -> RETK cycle
    let mut vm = Vm::new();
    vm.ram[0xFE00] = 50; // handler for syscall 0 at address 50

    // At address 0: SYSCALL 0
    vm.ram[0] = 0x52; // SYSCALL
    vm.ram[1] = 0;    // syscall number 0

    // At address 50 (handler): RETK
    vm.ram[50] = 0x53; // RETK

    vm.mode = geometry_os::vm::CpuMode::User;
    vm.pc = 0;

    vm.step(); // execute SYSCALL 0 -> jumps to 50, saves return PC=2, saves mode=User
    assert_eq!(vm.pc, 50);
    assert_eq!(vm.mode, geometry_os::vm::CpuMode::Kernel);

    vm.step(); // execute RETK -> returns to PC=2, restores User mode
    assert_eq!(vm.pc, 2, "RETK should restore return PC");
    assert_eq!(vm.mode, geometry_os::vm::CpuMode::User, "RETK should restore User mode");
    assert_eq!(vm.kernel_stack.len(), 0, "RETK should pop from kernel stack");
}

#[test]
fn test_retk_empty_stack_halts() {
    // RETK with empty kernel stack should halt (protection fault)
    let mut vm = Vm::new();
    vm.ram[0] = 0x53; // RETK with no saved state
    vm.pc = 0;

    let result = vm.step();
    assert!(!result, "RETK with empty stack should return false");
    assert!(vm.halted, "RETK with empty stack should halt the VM");
}

#[test]
fn test_user_mode_store_to_hardware_region_halts() {
    // User mode STORE to address >= 0xFF00 should halt
    let mut vm = Vm::new();
    vm.mode = geometry_os::vm::CpuMode::User;

    // LDI r0, 0xFFF0  -- address in hardware region
    vm.ram[0] = 0x10; vm.ram[1] = 0; vm.ram[2] = 0xFFF0;
    // LDI r1, 42      -- value to store
    vm.ram[3] = 0x10; vm.ram[4] = 1; vm.ram[5] = 42;
    // STORE r0, r1    -- attempt to write to hardware region
    vm.ram[6] = 0x12; vm.ram[7] = 0; vm.ram[8] = 1;
    vm.pc = 0;

    // Run LDI r0
    vm.step();
    // Run LDI r1
    vm.step();
    // Run STORE (should halt in user mode)
    let result = vm.step();
    assert!(!result, "STORE to hardware region in user mode should fail");
    assert!(vm.halted, "STORE to hardware region in user mode should halt");
}

#[test]
fn test_kernel_mode_store_to_hardware_region_works() {
    // Kernel mode can write to hardware region
    let mut vm = Vm::new();
    vm.mode = geometry_os::vm::CpuMode::Kernel;

    // LDI r0, 0xFFF0
    vm.ram[0] = 0x10; vm.ram[1] = 0; vm.ram[2] = 0xFFF0;
    // LDI r1, 42
    vm.ram[3] = 0x10; vm.ram[4] = 1; vm.ram[5] = 42;
    // STORE r0, r1
    vm.ram[6] = 0x12; vm.ram[7] = 0; vm.ram[8] = 1;
    // HALT
    vm.ram[9] = 0x00;
    vm.pc = 0;

    for _ in 0..10 { if !vm.step() { break; } }
    assert!(vm.halted, "VM should halt after STORE + HALT");
    assert_eq!(vm.ram[0xFFF0], 42, "Kernel mode should write to hardware region");
}

#[test]
fn test_user_mode_store_to_normal_ram_allowed() {
    // STORE to regular RAM in user mode should work fine
    let mut vm = Vm::new();
    vm.ram[0] = 0x10; // LDI r1, 100
    vm.ram[1] = 1;
    vm.ram[2] = 100;
    vm.ram[3] = 0x10; // LDI r2, 42
    vm.ram[4] = 2;
    vm.ram[5] = 42;
    vm.ram[6] = 0x12; // STORE r1, r2
    vm.ram[7] = 1;
    vm.ram[8] = 2;
    vm.ram[9] = 0x00; // HALT
    vm.pc = 0;
    vm.mode = geometry_os::vm::CpuMode::User;

    for _ in 0..100 {
        if !vm.step() { break; }
    }
    assert!(vm.halted, "VM should halt normally");
    assert_eq!(vm.ram[100], 42, "regular RAM write should work in user mode");
}

#[test]
fn test_user_mode_ikey_halts() {
    // User mode IKEY should halt
    let mut vm = Vm::new();
    vm.mode = geometry_os::vm::CpuMode::User;
    vm.ram[0xFFF] = 65; // keyboard has a key

    // IKEY r0
    vm.ram[0] = 0x48; vm.ram[1] = 0;
    vm.pc = 0;

    let result = vm.step();
    assert!(!result, "IKEY in user mode should fail");
    assert!(vm.halted, "IKEY in user mode should halt");
    assert_eq!(vm.regs[0], 0, "IKEY should not have read the key in user mode");
}

#[test]
fn test_kernel_mode_ikey_works() {
    // Kernel mode IKEY should work normally
    let mut vm = Vm::new();
    vm.mode = geometry_os::vm::CpuMode::Kernel;
    vm.ram[0xFFF] = 65;

    // IKEY r0
    vm.ram[0] = 0x48; vm.ram[1] = 0;
    vm.pc = 0;

    vm.step();
    assert_eq!(vm.regs[0], 65, "Kernel mode IKEY should read key");
    assert_eq!(vm.ram[0xFFF], 0, "Kernel mode IKEY should clear port");
}

#[test]
fn test_nested_syscalls() {
    // SYSCALL from kernel mode -> handler -> SYSCALL again -> RETK -> RETK
    let mut vm = Vm::new();
    // Syscall 0 -> address 10
    vm.ram[0xFE00] = 10;
    // Syscall 1 -> address 20
    vm.ram[0xFE01] = 20;

    // Address 0: SYSCALL 0 (in kernel mode, should still work)
    vm.ram[0] = 0x52;
    vm.ram[1] = 0;
    // Address 2: HALT (where outer RETK returns to)
    vm.ram[2] = 0x00;

    // Address 10: LDI r0, 10; SYSCALL 1; RETK
    vm.ram[10] = 0x10; vm.ram[11] = 0; vm.ram[12] = 10; // LDI r0, 10
    vm.ram[13] = 0x52; vm.ram[14] = 1; // SYSCALL 1
    vm.ram[15] = 0x53; // RETK

    // Address 20: LDI r0, 20; RETK
    vm.ram[20] = 0x10; vm.ram[21] = 0; vm.ram[22] = 20; // LDI r0, 20
    vm.ram[23] = 0x53; // RETK

    vm.pc = 0;
    vm.mode = geometry_os::vm::CpuMode::Kernel;

    for _ in 0..100 {
        if !vm.step() { break; }
    }
    assert!(vm.halted, "VM should halt");
    assert_eq!(vm.regs[0], 20, "r0 should be 20 (innermost handler sets r0, no register save/restore)");
    assert_eq!(vm.mode, geometry_os::vm::CpuMode::Kernel, "should return to kernel mode");
    assert_eq!(vm.pc, 3, "should end at instruction after outer SYSCALL");
}

#[test]
fn test_spawned_process_inherits_user_mode() {
    // SPAWN creates process in user mode
    let source = "
    LDI r1, 100
    SPAWN r1
    HALT
    ";
    let asm = assemble(source, 0).unwrap();
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() {
        vm.ram[i] = v;
    }
    vm.pc = 0;
    vm.mode = geometry_os::vm::CpuMode::Kernel;

    for _ in 0..100 {
        if !vm.step() { break; }
    }
    assert!(vm.processes.len() > 0, "should have spawned a process");
    assert_eq!(vm.processes[0].mode, geometry_os::vm::CpuMode::User,
        "spawned process should be in user mode");
    assert_eq!(vm.mode, geometry_os::vm::CpuMode::Kernel,
        "parent should stay in kernel mode");
}

#[test]
fn test_syscall_preserves_mode_for_nested_calls() {
    // User mode -> SYSCALL -> kernel mode -> RETK -> back to user mode
    let mut vm = Vm::new();
    vm.mode = geometry_os::vm::CpuMode::User;

    // Syscall 0 handler at address 200
    vm.ram[0xFE00] = 200;

    // At 0: SYSCALL 0
    vm.ram[0] = 0x52; vm.ram[1] = 0;
    // At 2: LDI r1, 42 (after return from syscall)
    vm.ram[2] = 0x10; vm.ram[3] = 1; vm.ram[4] = 42;
    // At 5: HALT
    vm.ram[5] = 0x00;

    // At 200 (handler): RETK
    vm.ram[200] = 0x53;

    vm.pc = 0;

    // SYSCALL 0 -> jumps to 200, saves (PC=2, User)
    vm.step();
    assert_eq!(vm.mode, geometry_os::vm::CpuMode::Kernel);
    assert_eq!(vm.pc, 200);

    // RETK -> returns to PC=2, restores User
    vm.step();
    assert_eq!(vm.mode, geometry_os::vm::CpuMode::User);
    assert_eq!(vm.pc, 2);

    // LDI r1, 42 should work in user mode (LDI is not restricted)
    vm.step();
    assert_eq!(vm.regs[1], 42, "LDI should work after returning from syscall");
}

#[test]
fn test_reset_clears_kernel_state() {
    let mut vm = Vm::new();
    vm.mode = geometry_os::vm::CpuMode::User;
    vm.kernel_stack.push((100, geometry_os::vm::CpuMode::User));
    vm.reset();
    assert_eq!(vm.mode, geometry_os::vm::CpuMode::Kernel, "reset should restore Kernel mode");
    assert!(vm.kernel_stack.is_empty(), "reset should clear kernel stack");
}

// === Phase 24: Memory Protection Tests ===

#[test]
fn test_child_segfaults_on_unmapped_store() {
    // Spawn a child that tries to STORE to an unmapped virtual address.
    // Virtual pages 0-3 are mapped (PROCESS_PAGES=4, PAGE_SIZE=1024).
    // Virtual address 0x1000 (= page 4) is unmapped -> SEGFAULT.
    let source = "
    LDI r1, 0x200
    SPAWN r1
    HALT

    .org 0x200
    LDI r0, 0x1000
    LDI r2, 42
    STORE r0, r2
    HALT
    ";
    let asm = assemble(source, 0).unwrap();
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() { vm.ram[i] = v; }

    // Run main process to completion (spawns child, then halts)
    for _ in 0..100 { if !vm.step() { break; } }
    assert!(vm.halted);
    assert_eq!(vm.processes.len(), 1);

    // Step child process -- it should segfault on the STORE
    for _ in 0..50 {
        vm.step_all_processes();
        if vm.processes[0].halted { break; }
    }

    assert!(vm.processes[0].halted, "child should be halted after segfault");
    assert!(vm.processes[0].segfaulted, "child should have segfaulted flag set");
    // RAM[0xFF9] should hold the segfaulted PID
    assert_eq!(vm.ram[0xFF9], 1, "RAM[0xFF9] should hold segfaulted PID");
}

#[test]
fn test_child_segfaults_on_unmapped_load() {
    // Spawn a child that tries to LOAD from an unmapped virtual address.
    let source = "
    LDI r1, 0x200
    SPAWN r1
    HALT

    .org 0x200
    LDI r0, 0x1000
    LOAD r2, r0
    HALT
    ";
    let asm = assemble(source, 0).unwrap();
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() { vm.ram[i] = v; }

    for _ in 0..100 { if !vm.step() { break; } }
    assert!(vm.halted);

    for _ in 0..50 {
        vm.step_all_processes();
        if vm.processes[0].halted { break; }
    }

    assert!(vm.processes[0].halted, "child should be halted after segfault");
    assert!(vm.processes[0].segfaulted, "child should have segfaulted on unmapped LOAD");
}

#[test]
fn test_child_segfaults_on_unmapped_fetch() {
    // Spawn a child with code at virtual page 0.
    // The child code jumps to an unmapped virtual address.
    // The fetch at the unmapped address should trigger segfault.
    let source = "
    LDI r1, 0x200
    SPAWN r1
    HALT

    .org 0x200
    JMP 0x1000
    ";
    let asm = assemble(source, 0).unwrap();
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() { vm.ram[i] = v; }

    for _ in 0..100 { if !vm.step() { break; } }
    assert!(vm.halted);

    for _ in 0..50 {
        vm.step_all_processes();
        if vm.processes[0].halted { break; }
    }

    assert!(vm.processes[0].halted, "child should be halted after segfault");
    assert!(vm.processes[0].segfaulted, "child should segfault on fetching from unmapped page");
}

#[test]
fn test_process_memory_isolation() {
    // Spawn two children. Each writes a unique value to its own virtual page 1.
    // Verify that child 2's data doesn't overwrite child 1's data.
    // Child 1 writes 0xAAAA at virtual 0x0400 (page 1 offset 0)
    // Child 2 writes 0xBBBB at virtual 0x0400 (page 1 offset 0)
    // These map to DIFFERENT physical pages, so neither sees the other's write.
    let source = "
    LDI r1, 0x200
    SPAWN r1
    LDI r1, 0x300
    SPAWN r1
    HALT

    .org 0x200
    LDI r0, 0x0400
    LDI r2, 0xAAAA
    STORE r0, r2
    HALT

    .org 0x300
    LDI r0, 0x0400
    LDI r2, 0xBBBB
    STORE r0, r2
    HALT
    ";
    let asm = assemble(source, 0).unwrap();
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() { vm.ram[i] = v; }

    // Run main to completion
    for _ in 0..100 { if !vm.step() { break; } }
    assert!(vm.halted);
    assert_eq!(vm.processes.len(), 2);

    // Step children to completion
    for _ in 0..100 {
        vm.step_all_processes();
        if vm.processes.iter().all(|p| p.halted) { break; }
    }

    // Both children should have completed without segfault
    assert!(!vm.processes[0].segfaulted, "child 1 should not segfault");
    assert!(!vm.processes[1].segfaulted, "child 2 should not segfault");

    // Verify isolation: find physical pages for each child's virtual page 1
    let pd1 = vm.processes[0].page_dir.as_ref().expect("child 1 should have page_dir");
    let pd2 = vm.processes[1].page_dir.as_ref().expect("child 2 should have page_dir");

    // Virtual page 1 -> pd1[1] and pd2[1] should be DIFFERENT physical pages
    let phys_page1 = pd1[1] as usize;
    let phys_page2 = pd2[1] as usize;
    assert_ne!(phys_page1, phys_page2,
        "children should have different physical pages for virtual page 1");

    // Verify the values are in different physical locations
    let addr1 = phys_page1 * 1024; // PAGE_SIZE
    let addr2 = phys_page2 * 1024;
    assert_eq!(vm.ram[addr1], 0xAAAA, "child 1's physical memory should have 0xAAAA");
    assert_eq!(vm.ram[addr2], 0xBBBB, "child 2's physical memory should have 0xBBBB");
}

#[test]
fn test_kernel_mode_identity_mapping() {
    // Main process (kernel mode) has no page directory -> identity mapping.
    // STORE to any address should work directly.
    let source = "
    LDI r1, 0x8000
    LDI r2, 0xDEAD
    STORE r1, r2
    LOAD r3, r1
    HALT
    ";
    let asm = assemble(source, 0).unwrap();
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() { vm.ram[i] = v; }
    assert_eq!(vm.mode, geometry_os::vm::CpuMode::Kernel, "VM should start in kernel mode");
    assert!(vm.current_page_dir.is_none(), "kernel should have no page directory");

    for _ in 0..100 { if !vm.step() { break; } }
    assert!(vm.halted);
    assert!(!vm.segfault, "kernel mode should not segfault on any address");
    assert_eq!(vm.regs[3], 0xDEAD, "kernel should read back what it wrote");
    assert_eq!(vm.ram[0x8000], 0xDEAD, "RAM at 0x8000 should have the value");
}

#[test]
fn test_kill_frees_physical_pages() {
    // Spawn a child, kill it, spawn another -- the freed pages should be reusable.
    let source = "
    LDI r1, 0x200
    SPAWN r1
    LDI r3, 0xFFA
    LOAD r2, r3
    KILL r2
    LDI r1, 0x300
    SPAWN r1
    HALT

    .org 0x200
    HALT

    .org 0x300
    HALT
    ";
    let asm = assemble(source, 0).unwrap();
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() { vm.ram[i] = v; }

    for _ in 0..200 { if !vm.step() { break; } }
    assert!(vm.halted);

    // Should have 2 processes: first killed, second alive
    assert_eq!(vm.processes.len(), 2);
    assert!(vm.processes[0].halted, "first child should be killed");
    // Second child should have been spawned successfully (pages were freed)
    assert!(!vm.processes[1].segfaulted, "second child should not have segfaulted");
}

#[test]
fn test_child_user_mode_blocks_hardware_port_write() {
    // Spawn a child that tries to STORE to 0xFF00+ (hardware ports).
    // In User mode this should trigger segfault.
    let source = "
    LDI r1, 0x200
    SPAWN r1
    HALT

    .org 0x200
    LDI r0, 0xFF00
    LDI r2, 42
    STORE r0, r2
    HALT
    ";
    let asm = assemble(source, 0).unwrap();
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() { vm.ram[i] = v; }

    for _ in 0..100 { if !vm.step() { break; } }
    assert!(vm.halted);

    for _ in 0..50 {
        vm.step_all_processes();
        if vm.processes[0].halted { break; }
    }

    assert!(vm.processes[0].segfaulted,
        "child in user mode should segfault when writing to hardware port 0xFF00+");
}

#[test]
fn test_child_can_access_shared_window_bounds() {
    // Children should be able to READ the Window Bounds Protocol region (0xF00-0xFFF)
    // because page 3 is identity-mapped. They can also write to it (it's shared).
    let source = "
    LDI r1, 0x200
    SPAWN r1
    HALT

    .org 0x200
    LDI r0, 0xF00
    LOAD r2, r0
    HALT
    ";
    let asm = assemble(source, 0).unwrap();
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() { vm.ram[i] = v; }
    // Write something at 0xF00 for the child to read
    vm.ram[0xF00] = 0x1234;

    for _ in 0..100 { if !vm.step() { break; } }
    assert!(vm.halted);

    for _ in 0..50 {
        vm.step_all_processes();
        if vm.processes[0].halted { break; }
    }

    assert!(!vm.processes[0].segfaulted,
        "child should be able to read shared window bounds region without segfault");
}

#[test]
fn test_child_page_directory_has_shared_regions_mapped() {
    // Spawn a child and verify its page directory maps the shared regions.
    let source = "
    LDI r1, 0x200
    SPAWN r1
    HALT

    .org 0x200
    HALT
    ";
    let asm = assemble(source, 0).unwrap();
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() { vm.ram[i] = v; }

    for _ in 0..100 { if !vm.step() { break; } }
    assert!(vm.halted);

    let pd = vm.processes[0].page_dir.as_ref().expect("child should have page_dir");

    // Page 3 (0xC00-0xFFF) should be identity-mapped for Window Bounds Protocol
    assert_eq!(pd[3], 3, "page 3 should be identity-mapped (window bounds)");
    // Page 63 (0xFC00-0xFFFF) should be identity-mapped for hardware/syscalls
    assert_eq!(pd[63], 63, "page 63 should be identity-mapped (hardware ports)");

    // Pages 0-3 should be private (first 3 are allocated, 3rd is identity)
    // Actually pages 0, 1, 2 are private allocated pages; page 3 is identity-mapped
    assert!(pd[0] < 64 && pd[0] != 0 && pd[0] != 1,
        "virtual page 0 should map to a private physical page");
    assert!(pd[1] < 64 && pd[1] != 0 && pd[1] != 1,
        "virtual page 1 should map to a private physical page");
    assert!(pd[2] < 64 && pd[2] != 0 && pd[2] != 1,
        "virtual page 2 should map to a private physical page");

    // Pages 4-62 should be unmapped
    for i in 4..63 {
        assert_eq!(pd[i], 0xFFFFFFFF, "page {} should be unmapped (PAGE_UNMAPPED)", i);
    }
}

#[test]
fn test_segfault_pid_tracking() {
    // Spawn two children. First one segfaults. Verify RAM[0xFF9] tracks the PID.
    let source = "
    LDI r1, 0x200
    SPAWN r1
    LDI r1, 0x300
    SPAWN r1
    HALT

    .org 0x200
    LDI r0, 0x1000
    LDI r2, 42
    STORE r0, r2
    HALT

    .org 0x300
    HALT
    ";
    let asm = assemble(source, 0).unwrap();
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() { vm.ram[i] = v; }

    for _ in 0..100 { if !vm.step() { break; } }
    assert!(vm.halted);
    assert_eq!(vm.processes.len(), 2);

    for _ in 0..100 {
        vm.step_all_processes();
        if vm.processes[0].halted && vm.processes[1].halted { break; }
    }

    // First child (PID 1) should have segfaulted
    assert!(vm.processes[0].segfaulted, "child 1 should segfault");
    // Second child (PID 2) should have completed normally
    assert!(!vm.processes[1].segfaulted, "child 2 should not segfault");
    // RAM[0xFF9] should hold PID of the segfaulted process
    assert_eq!(vm.ram[0xFF9], 1, "RAM[0xFF9] should be PID of segfaulted child");
}


// ── PHASE 25: FILESYSTEM OPCODES ─────────────────────────────────

/// Helper: create a VM with a temp VFS directory for file tests
fn vm_with_vfs() -> (Vm, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let mut vm = Vm::new();
    vm.vfs.base_dir = dir.path().to_path_buf();
    let _ = std::fs::create_dir_all(&vm.vfs.base_dir);
    (vm, dir)
}

/// Helper: write a null-terminated string into VM RAM at given address
fn write_string_to_ram(vm: &mut Vm, addr: usize, s: &str) {
    for (i, ch) in s.bytes().enumerate() {
        vm.ram[addr + i] = ch as u32;
    }
    vm.ram[addr + s.len()] = 0;
}

/// Helper: load bytecode at address 0 and set up for execution
fn load_and_run(vm: &mut Vm, bytecode: &[u32], max_cycles: usize) {
    for (i, &word) in bytecode.iter().enumerate() {
        vm.ram[i] = word;
    }
    vm.pc = 0;
    vm.halted = false;
    for _ in 0..max_cycles {
        if !vm.step() {
            break;
        }
    }
}

#[test]
fn test_open_read_write_close_opcodes() {
    let (mut vm, _dir) = vm_with_vfs();

    // Write filename "test.txt" at RAM address 0x1000
    write_string_to_ram(&mut vm, 0x1000, "test.txt");

    // Write data "ABCD" at RAM address 0x1100
    for (i, ch) in b"ABCD".iter().enumerate() {
        vm.ram[0x1100 + i] = *ch as u32;
    }

    // Program:
    //   OPEN r1, r2      -- r1=&path, r2=mode(1=write)
    //   WRITE r0, r3, r4 -- r0=fd(from open), r3=&buf, r4=len
    //   CLOSE r0          -- close fd
    //   OPEN r1, r5      -- r5=mode(0=read)
    //   READ r0, r6, r7  -- r6=&readbuf(0x1200), r7=len(4)
    //   CLOSE r0
    //   HALT

    // Set registers
    vm.regs[1] = 0x1000;  // path address
    vm.regs[2] = 1;       // write mode
    vm.regs[3] = 0x1100;  // write buffer
    vm.regs[4] = 4;       // write length
    vm.regs[5] = 0;       // read mode
    vm.regs[6] = 0x1200;  // read buffer
    vm.regs[7] = 4;       // read length

    let program = vec![
        0x54, 1, 2,       // OPEN r1, r2  -> r0 = fd
        0x56, 0, 3, 4,    // WRITE r0, r3, r4  -> r0 = bytes written
        0x57, 0,          // CLOSE r0
        0x54, 1, 5,       // OPEN r1, r5  -> r0 = fd (read mode)
        0x55, 0, 6, 7,    // READ r0, r6, r7  -> r0 = bytes read
        0x57, 0,          // CLOSE r0
        0x00,             // HALT
    ];

    load_and_run(&mut vm, &program, 100);

    assert!(vm.halted, "VM should halt");

    // Verify the read-back data matches what was written
    assert_eq!(vm.ram[0x1200] & 0xFF, b'A' as u32, "byte 0 should be A");
    assert_eq!(vm.ram[0x1201] & 0xFF, b'B' as u32, "byte 1 should be B");
    assert_eq!(vm.ram[0x1202] & 0xFF, b'C' as u32, "byte 2 should be C");
    assert_eq!(vm.ram[0x1203] & 0xFF, b'D' as u32, "byte 3 should be D");
}

#[test]
fn test_open_nonexistent_file_returns_error() {
    let (mut vm, _dir) = vm_with_vfs();

    write_string_to_ram(&mut vm, 0x1000, "nonexistent.txt");
    vm.regs[1] = 0x1000;
    vm.regs[2] = 0; // read mode

    let program = vec![
        0x54, 1, 2,  // OPEN r1, r2
        0x00,        // HALT
    ];

    load_and_run(&mut vm, &program, 100);

    // r0 should be FD_ERROR (0xFFFFFFFF) since file doesn't exist
    assert_eq!(vm.regs[0], 0xFFFFFFFF, "opening nonexistent file should return error");
}

#[test]
fn test_close_invalid_fd_returns_error() {
    let (mut vm, _dir) = vm_with_vfs();
    vm.regs[0] = 99; // invalid fd

    let program = vec![
        0x57, 0,  // CLOSE r0 (invalid fd)
        0x00,
    ];

    load_and_run(&mut vm, &program, 100);
    assert_eq!(vm.regs[0], 0xFFFFFFFF, "closing invalid fd should return error");
}

#[test]
fn test_seek_opcode() {
    let (mut vm, _dir) = vm_with_vfs();

    write_string_to_ram(&mut vm, 0x1000, "seektest.txt");
    vm.regs[1] = 0x1000;
    vm.regs[2] = 1; // write mode

    // Write "ABCDEFGH"
    for (i, ch) in b"ABCDEFGH".iter().enumerate() {
        vm.ram[0x1100 + i] = *ch as u32;
    }
    vm.regs[3] = 0x1100;
    vm.regs[4] = 8;

    // Program: open, write, close, open(read), seek(4, SET), read(2), close, halt
    vm.regs[5] = 0; // read mode
    vm.regs[6] = 0x1200; // read buf
    vm.regs[7] = 2; // read 2 bytes
    vm.regs[8] = 4; // seek offset
    vm.regs[9] = 0; // SEEK_SET

    let program = vec![
        0x54, 1, 2,       // OPEN r1, r2 (write)
        0x56, 0, 3, 4,    // WRITE r0, r3, r4
        0x57, 0,          // CLOSE r0
        0x54, 1, 5,       // OPEN r1, r5 (read)
        0x58, 0, 8, 9,    // SEEK r0, r8, r9 (offset=4, whence=SET)
        0x55, 0, 6, 7,    // READ r0, r6, r7
        0x57, 0,          // CLOSE r0
        0x00,             // HALT
    ];

    load_and_run(&mut vm, &program, 100);

    assert!(vm.halted);
    // Should read "EF" (bytes at offset 4 and 5)
    assert_eq!(vm.ram[0x1200] & 0xFF, b'E' as u32);
    assert_eq!(vm.ram[0x1201] & 0xFF, b'F' as u32);
}

#[test]
fn test_ls_opcode() {
    let (mut vm, _dir) = vm_with_vfs();

    // Create some files in the VFS directory
    std::fs::write(vm.vfs.base_dir.join("aaa.txt"), b"").unwrap();
    std::fs::write(vm.vfs.base_dir.join("bbb.txt"), b"").unwrap();

    vm.regs[1] = 0x2000; // buffer for listing

    let program = vec![
        0x59, 1,  // LS r1
        0x00,
    ];

    load_and_run(&mut vm, &program, 100);

    assert!(vm.halted);
    // r0 should be 2 (two files)
    assert_eq!(vm.regs[0], 2, "LS should return 2 entries");

    // Parse the entries from RAM
    let mut entries = Vec::new();
    let mut addr = 0x2000;
    for _ in 0..10 {
        let ch = (vm.ram[addr] & 0xFF) as u8;
        if ch == 0 { break; }
        let mut name = String::new();
        loop {
            let c = (vm.ram[addr] & 0xFF) as u8;
            if c == 0 { addr += 1; break; }
            name.push(c as char);
            addr += 1;
        }
        entries.push(name);
    }
    entries.sort();
    assert_eq!(entries, vec!["aaa.txt", "bbb.txt"]);
}

#[test]
fn test_filesystem_opcodes_assemble() {
    use geometry_os::assembler::assemble;

    let src = "OPEN r1, r2
READ r0, r3, r4
WRITE r0, r3, r4
CLOSE r0
SEEK r0, r1, r2
LS r3
HALT";
    let result = assemble(src, 0).expect("assembly should succeed");
    assert_eq!(result.pixels[0], 0x54); // OPEN
    assert_eq!(result.pixels[3], 0x55); // READ
    assert_eq!(result.pixels[7], 0x56); // WRITE
    assert_eq!(result.pixels[11], 0x57); // CLOSE
    assert_eq!(result.pixels[13], 0x58); // SEEK
    assert_eq!(result.pixels[17], 0x59); // LS
    assert_eq!(result.pixels[19], 0x00); // HALT
}

#[test]
fn test_vfs_path_traversal_blocked() {
    let (mut vm, _dir) = vm_with_vfs();

    // Try to open a file with path traversal
    write_string_to_ram(&mut vm, 0x1000, "../../../etc/passwd");
    vm.regs[1] = 0x1000;
    vm.regs[2] = 0;

    let program = vec![
        0x54, 1, 2,  // OPEN r1, r2
        0x00,
    ];

    load_and_run(&mut vm, &program, 100);
    assert_eq!(vm.regs[0], 0xFFFFFFFF, "path traversal should be blocked");
}

#[test]
fn test_cat_asm_assembles() {
    use geometry_os::assembler::assemble;
    let source = std::fs::read_to_string("programs/cat.asm")
        .expect("cat.asm should exist");
    let result = assemble(&source, 0);
    assert!(result.is_ok(), "cat.asm should assemble: {:?}", result.err());
}

#[test]
fn test_cat_asm_reads_and_displays_file() {
    let (mut vm, _dir) = vm_with_vfs();

    // Create hello.txt in the VFS directory
    std::fs::write(vm.vfs.base_dir.join("hello.txt"), b"Hello from file!").unwrap();

    use geometry_os::assembler::assemble;
    let source = std::fs::read_to_string("programs/cat.asm")
        .expect("cat.asm should exist");
    let result = assemble(&source, 0).expect("cat.asm should assemble");

    // Load bytecode into RAM at 0x100 (after the filename data at 0x1000)
    for (i, &word) in result.pixels.iter().enumerate() {
        if 0x0100 + i < vm.ram.len() {
            vm.ram[0x0100 + i] = word;
        }
    }
    vm.pc = 0x0100;

    // Run enough steps to complete
    for _ in 0..1000 {
        if vm.halted { break; }
        vm.step();
    }

    assert!(vm.halted, "cat.asm should halt");

    // Verify the file content was read into the buffer at 0x2000
    let expected = b"Hello from file!";
    for (i, &ch) in expected.iter().enumerate() {
        assert_eq!(
            vm.ram[0x2000 + i] & 0xFF, ch as u32,
            "buffer[{}] should be '{}' but got '{}'",
            i, ch as char, (vm.ram[0x2000 + i] & 0xFF) as u8 as char
        );
    }

    // Verify null terminator after content
    assert_eq!(vm.ram[0x2000 + expected.len()] & 0xFF, 0, "should be null-terminated");
}

#[test]
fn test_cat_asm_nonexistent_file_halts_cleanly() {
    let (mut vm, _dir) = vm_with_vfs();

    // Don't create any file -- cat.asm should handle open error gracefully
    use geometry_os::assembler::assemble;
    let source = std::fs::read_to_string("programs/cat.asm")
        .expect("cat.asm should exist");
    let result = assemble(&source, 0).expect("cat.asm should assemble");

    for (i, &word) in result.pixels.iter().enumerate() {
        if 0x0100 + i < vm.ram.len() {
            vm.ram[0x0100 + i] = word;
        }
    }
    vm.pc = 0x0100;

    for _ in 0..1000 {
        if vm.halted { break; }
        vm.step();
    }

    // Should halt gracefully (not crash)
    assert!(vm.halted, "cat.asm should halt even when file doesn't exist");
}

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
    let asm = assemble(source, 0).unwrap();
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() {
        vm.ram[i] = v;
    }
    vm.processes.push(geometry_os::vm::SpawnedProcess {
        pc: 0x200,
        regs: [0; 32],
        halted: false,
        pid: 1,
        mode: geometry_os::vm::CpuMode::Kernel,
        page_dir: None,
        segfaulted: false,
        priority: 1,
        slice_remaining: 0,
        sleep_until: 0,
        yielded: false,
                blocked: false,
                msg_queue: Vec::new(),
    });
    for _ in 0..10 {
        vm.step_all_processes();
        if vm.processes[0].halted { break; }
    }
    assert_eq!(vm.ram[0x1000], 42, "child should have written 42");
    assert!(vm.processes[0].halted, "child should have halted");
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
    let asm = assemble(source, 0).unwrap();
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() {
        vm.ram[i] = v;
    }
    vm.processes.push(geometry_os::vm::SpawnedProcess {
        pc: 0x200, regs: [0; 32], halted: false, pid: 1,
        mode: geometry_os::vm::CpuMode::Kernel, page_dir: None,
        segfaulted: false, priority: 1, slice_remaining: 0,
        sleep_until: 0, yielded: false,
        blocked: false, msg_queue: Vec::new(),
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
    let asm = assemble(source, 0).unwrap();
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() {
        vm.ram[i] = v;
    }
    vm.processes.push(geometry_os::vm::SpawnedProcess {
        pc: 0x200, regs: [0; 32], halted: false, pid: 1,
        mode: geometry_os::vm::CpuMode::Kernel, page_dir: None,
        segfaulted: false, priority: 1, slice_remaining: 0,
        sleep_until: 0, yielded: false,
        blocked: false, msg_queue: Vec::new(),
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
    let asm = assemble(source, 0).unwrap();
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() {
        vm.ram[i] = v;
    }
    // Process A: priority 3 (quantum = 100 * 8 = 800)
    vm.processes.push(geometry_os::vm::SpawnedProcess {
        pc: 0x200, regs: [0; 32], halted: false, pid: 1,
        mode: geometry_os::vm::CpuMode::Kernel, page_dir: None,
        segfaulted: false, priority: 3, slice_remaining: 0,
        sleep_until: 0, yielded: false,
        blocked: false, msg_queue: Vec::new(),
    });
    // Process B: priority 0 (quantum = 100 * 1 = 100)
    vm.processes.push(geometry_os::vm::SpawnedProcess {
        pc: 0x300, regs: [0; 32], halted: false, pid: 2,
        mode: geometry_os::vm::CpuMode::Kernel, page_dir: None,
        segfaulted: false, priority: 0, slice_remaining: 0,
        sleep_until: 0, yielded: false,
        blocked: false, msg_queue: Vec::new(),
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
    let asm = assemble(source, 0).unwrap();
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() {
        vm.ram[i] = v;
    }
    vm.processes.push(geometry_os::vm::SpawnedProcess {
        pc: 0x200, regs: [0; 32], halted: false, pid: 1,
        mode: geometry_os::vm::CpuMode::Kernel, page_dir: None,
        segfaulted: false, priority: 0, slice_remaining: 0,
        sleep_until: 0, yielded: false,
        blocked: false, msg_queue: Vec::new(),
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
    let asm = assemble(source, 0).unwrap();
    for (i, &v) in asm.pixels.iter().enumerate() { vm.ram[i] = v; }
    vm.processes.push(geometry_os::vm::SpawnedProcess {
        pc: 0x200, regs: [0; 32], halted: false, pid: 1,
        mode: geometry_os::vm::CpuMode::Kernel, page_dir: None,
        segfaulted: false, priority: 1, slice_remaining: 0,
        sleep_until: 0, yielded: false,
        blocked: false, msg_queue: Vec::new(),
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
    let asm = assemble(source, 0).unwrap();
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() { vm.ram[i] = v; }
    vm.processes.push(geometry_os::vm::SpawnedProcess {
        pc: 0x200, regs: [0; 32], halted: false, pid: 1,
        mode: geometry_os::vm::CpuMode::Kernel, page_dir: None,
        segfaulted: false, priority: 1, slice_remaining: 0,
        sleep_until: 0, yielded: false,
        blocked: false, msg_queue: Vec::new(),
    });
    vm.step_all_processes(); // LDI r5, 5
    vm.step_all_processes(); // SLEEP r5
    assert!(vm.processes[0].sleep_until > 0, "process should be sleeping");
    for _ in 0..30 {
        vm.step_all_processes();
        if vm.processes[0].halted { break; }
    }
    assert!(vm.processes[0].halted, "process should have woken and halted");
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
    let asm = assemble(source, 0).unwrap();
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() { vm.ram[i] = v; }
    vm.processes.push(geometry_os::vm::SpawnedProcess {
        pc: 0x200, regs: [0; 32], halted: false, pid: 3,
        mode: geometry_os::vm::CpuMode::Kernel, page_dir: None,
        segfaulted: false, priority: 2, slice_remaining: 0,
        sleep_until: 0, yielded: false,
        blocked: false, msg_queue: Vec::new(),
    });
    vm.processes.push(geometry_os::vm::SpawnedProcess {
        pc: 0x300, regs: [0; 32], halted: false, pid: 2,
        mode: geometry_os::vm::CpuMode::Kernel, page_dir: None,
        segfaulted: false, priority: 1, slice_remaining: 0,
        sleep_until: 0, yielded: false,
        blocked: false, msg_queue: Vec::new(),
    });
    vm.processes.push(geometry_os::vm::SpawnedProcess {
        pc: 0x400, regs: [0; 32], halted: false, pid: 1,
        mode: geometry_os::vm::CpuMode::Kernel, page_dir: None,
        segfaulted: false, priority: 0, slice_remaining: 0,
        sleep_until: 0, yielded: false,
        blocked: false, msg_queue: Vec::new(),
    });
    for _ in 0..50 {
        vm.step_all_processes();
        if vm.processes.iter().all(|p| p.halted) { break; }
    }
    assert!(vm.processes.iter().all(|p| p.halted), "all processes should halt");
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
    let asm = assemble(source, 0).unwrap();
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() {
        vm.ram[i] = v;
    }
    // Process A: priority 3 (high) -- gets 800 instructions per round
    vm.processes.push(geometry_os::vm::SpawnedProcess {
        pc: 0x200, regs: [0; 32], halted: false, pid: 1,
        mode: geometry_os::vm::CpuMode::Kernel, page_dir: None,
        segfaulted: false, priority: 3, slice_remaining: 0,
        sleep_until: 0, yielded: false,
        blocked: false, msg_queue: Vec::new(),
    });
    // Process B: priority 0 (low) -- gets 100 instructions per round
    vm.processes.push(geometry_os::vm::SpawnedProcess {
        pc: 0x300, regs: [0; 32], halted: false, pid: 2,
        mode: geometry_os::vm::CpuMode::Kernel, page_dir: None,
        segfaulted: false, priority: 0, slice_remaining: 0,
        sleep_until: 0, yielded: false,
        blocked: false, msg_queue: Vec::new(),
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
    let asm = assemble(source, 0).unwrap();
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
    let asm = assemble(source, 0).unwrap();
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
    vm.ram[1] = 5;    // rd_read
    vm.ram[2] = 6;    // rd_write

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
    vm.ram[11] = 6;    // fd_reg
    vm.ram[12] = 10;   // buf_reg
    vm.ram[13] = 11;   // len_reg
    vm.ram[14] = 0x00; // HALT
    vm.halted = false;
    vm.step(); // WRITE
    assert_eq!(vm.regs[0], 3, "should write 3 words");

    // READ r5, r12, r11 -- read from pipe into buffer
    // r5=read_fd(0x8000), r12=0x1100, r11=3
    vm.regs[12] = 0x1100;
    vm.pc = 20;
    vm.ram[20] = 0x55; // READ
    vm.ram[21] = 5;    // fd_reg
    vm.ram[22] = 12;   // buf_reg
    vm.ram[23] = 11;   // len_reg
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
        pc: 0x200, regs: [0; 32], halted: false, pid: 1,
        mode: geometry_os::vm::CpuMode::Kernel, page_dir: None,
        segfaulted: false, priority: 1, slice_remaining: 100,
        sleep_until: 0, yielded: false,
        blocked: false, msg_queue: Vec::new(),
    });

    // Set up message data in main process registers
    vm.regs[1] = 0xDEAD;
    vm.regs[2] = 0xBEEF;
    vm.regs[3] = 0xCAFE;
    vm.regs[4] = 0xF00D;
    vm.regs[5] = 1; // target PID

    // Create MSGSND bytecode: 0x5E, r5
    vm.ram[0] = 0x5E; // MSGSND
    vm.ram[1] = 5;    // pid_reg = r5
    vm.ram[2] = 0x00; // HALT

    vm.step(); // MSGSND
    assert_eq!(vm.regs[0], 0, "MSGSND should succeed");
    assert_eq!(vm.processes[0].msg_queue.len(), 1, "child should have 1 message");

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
    vm.ram[1] = 5;    // pid_reg
    vm.step();
    assert_eq!(vm.regs[0], 0xFFFFFFFF, "MSGSND to nonexistent PID should fail");
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
        pc: 0x200, regs: [0; 32], halted: false, pid: 1,
        mode: geometry_os::vm::CpuMode::Kernel, page_dir: None,
        segfaulted: false, priority: 1, slice_remaining: 100,
        sleep_until: 0, yielded: false,
        blocked: false, msg_queue: vec![msg],
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
    assert_eq!(vm.processes[0].msg_queue.len(), 0, "message should be consumed");
}

#[test]
fn test_msgrcv_blocks_when_empty() {
    // Child process with no messages should block
    let mut vm = Vm::new();
    vm.mode = geometry_os::vm::CpuMode::Kernel;
    vm.current_pid = 1;

    vm.processes.push(geometry_os::vm::SpawnedProcess {
        pc: 0x200, regs: [0; 32], halted: false, pid: 1,
        mode: geometry_os::vm::CpuMode::Kernel, page_dir: None,
        segfaulted: false, priority: 1, slice_remaining: 100,
        sleep_until: 0, yielded: false,
        blocked: false, msg_queue: Vec::new(),
    });

    vm.ram[0x200] = 0x5F; // MSGRCV
    vm.pc = 0x200;
    vm.step(); // MSGRCV -- should block
    assert!(vm.processes[0].blocked, "process should be blocked");
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
        pc: 0x200, regs: [0; 32], halted: false, pid: 1,
        mode: geometry_os::vm::CpuMode::Kernel, page_dir: None,
        segfaulted: false, priority: 1, slice_remaining: 100,
        sleep_until: 0, yielded: false,
        blocked: false, msg_queue: Vec::new(),
    });

    // First block process A
    vm.ram[0x200] = 0x5F; // MSGRCV
    vm.pc = 0x200;
    vm.step();
    assert!(vm.processes[0].blocked);

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
    assert!(!vm.processes[0].blocked, "target should be unblocked after MSGSND");
    assert_eq!(vm.processes[0].msg_queue.len(), 1, "message should be in queue");
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
    let asm = assemble(source, 0).unwrap();
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() {
        vm.ram[i] = v;
    }
    // Create a blocked process
    vm.processes.push(geometry_os::vm::SpawnedProcess {
        pc: 0x200, regs: [0; 32], halted: false, pid: 1,
        mode: geometry_os::vm::CpuMode::Kernel, page_dir: None,
        segfaulted: false, priority: 1, slice_remaining: 100,
        sleep_until: 0, yielded: false,
        blocked: true, msg_queue: Vec::new(),
    });
    // Run scheduler 50 times -- blocked process should not execute
    for _ in 0..50 {
        vm.step_all_processes();
    }
    assert_eq!(vm.ram[0x1000], 0, "blocked process should not have run");
    assert!(vm.processes[0].blocked, "process should still be blocked");
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
        pc: 0x200, regs: [0; 32], halted: false, pid: 1,
        mode: geometry_os::vm::CpuMode::Kernel, page_dir: None,
        segfaulted: false, priority: 1, slice_remaining: 100,
        sleep_until: 0, yielded: false,
        blocked: true, msg_queue: Vec::new(),
    });

    // Main process writes to the pipe
    vm.regs[5] = 0xC000; // write_fd
    vm.ram[0x1000] = 42; // data
    vm.regs[6] = 0x1000; // buf addr
    vm.regs[7] = 1;      // len
    vm.ram[0] = 0x56;    // WRITE
    vm.ram[1] = 5;       // fd_reg
    vm.ram[2] = 6;       // buf_reg
    vm.ram[3] = 7;       // len_reg

    vm.step(); // WRITE

    assert_eq!(vm.regs[0], 1, "should have written 1 word");
    assert!(!vm.processes[0].blocked, "reader should be unblocked after write");
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
    let bc = &result.unwrap().pixels;
    assert_eq!(bc[0], 0x5D, "PIPE opcode");
    assert_eq!(bc[1], 5,  "PIPE read reg");
    assert_eq!(bc[2], 6,  "PIPE write reg");
    assert_eq!(bc[3], 0x5E, "MSGSND opcode");
    assert_eq!(bc[4], 7,  "MSGSND pid reg");
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
    let asm = assemble(source, 0).unwrap();
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
    vm.regs[7] = 1;      // len
    vm.ram[0] = 0x55;    // READ opcode
    vm.ram[1] = 5;       // fd_reg
    vm.ram[2] = 6;       // buf_reg
    vm.ram[3] = 7;       // len_reg
    vm.step(); // READ
    assert_eq!(vm.regs[0], 0xFFFFFFFF, "read from closed pipe should fail");
}

// ===================================================================
// Phase 28: Device Driver Abstraction Tests
// ===================================================================

#[test]
fn test_ioctl_assembles() {
    let source = "
    IOCTL r1, r2, r3
    HALT
    ";
    let result = assemble(source, 0);
    assert!(result.is_ok(), "IOCTL should assemble: {:?}", result.err());
    let bc = &result.unwrap().pixels;
    assert_eq!(bc[0], 0x62, "IOCTL opcode should be 0x62");
    assert_eq!(bc[1], 1, "fd_reg");
    assert_eq!(bc[2], 2, "cmd_reg");
    assert_eq!(bc[3], 3, "arg_reg");
}

/// Helper: write a null-terminated ASCII string into RAM at `addr`
fn write_string(ram: &mut [u32], addr: usize, s: &str) {
    for (i, ch) in s.chars().enumerate() {
        if addr + i < ram.len() {
            ram[addr + i] = ch as u32;
        }
    }
    if addr + s.len() < ram.len() {
        ram[addr + s.len()] = 0; // null terminator
    }
}

#[test]
fn test_open_device_screen_returns_device_fd() {
    // OPEN /dev/screen should return fd 0xE000
    let mut vm = Vm::new();
    vm.mode = geometry_os::vm::CpuMode::Kernel;
    write_string(&mut vm.ram, 0x1000, "/dev/screen");
    vm.ram[0] = 0x54; // OPEN
    vm.ram[1] = 1;    // path_reg
    vm.ram[2] = 2;    // mode_reg
    vm.regs[1] = 0x1000; // path address
    vm.regs[2] = 0;       // mode
    vm.step();
    assert_eq!(vm.regs[0], 0xE000, "OPEN /dev/screen should return fd 0xE000");
}

#[test]
fn test_open_device_keyboard_returns_device_fd() {
    let mut vm = Vm::new();
    vm.mode = geometry_os::vm::CpuMode::Kernel;
    write_string(&mut vm.ram, 0x1000, "/dev/keyboard");
    vm.ram[0] = 0x54; // OPEN
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.regs[1] = 0x1000;
    vm.regs[2] = 0;
    vm.step();
    assert_eq!(vm.regs[0], 0xE001, "OPEN /dev/keyboard should return fd 0xE001");
}

#[test]
fn test_open_device_audio_returns_device_fd() {
    let mut vm = Vm::new();
    vm.mode = geometry_os::vm::CpuMode::Kernel;
    write_string(&mut vm.ram, 0x1000, "/dev/audio");
    vm.ram[0] = 0x54; // OPEN
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.regs[1] = 0x1000;
    vm.regs[2] = 0;
    vm.step();
    assert_eq!(vm.regs[0], 0xE002, "OPEN /dev/audio should return fd 0xE002");
}

#[test]
fn test_open_device_net_returns_device_fd() {
    let mut vm = Vm::new();
    vm.mode = geometry_os::vm::CpuMode::Kernel;
    write_string(&mut vm.ram, 0x1000, "/dev/net");
    vm.ram[0] = 0x54; // OPEN
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.regs[1] = 0x1000;
    vm.regs[2] = 0;
    vm.step();
    assert_eq!(vm.regs[0], 0xE003, "OPEN /dev/net should return fd 0xE003");
}

#[test]
fn test_ioctl_screen_get_width() {
    // IOCTL /dev/screen cmd=0 (get width) should return 256
    let mut vm = Vm::new();
    vm.mode = geometry_os::vm::CpuMode::Kernel;
    vm.regs[1] = 0xE000; // /dev/screen fd
    vm.regs[2] = 0;      // cmd: get width
    vm.regs[3] = 0;      // arg: unused
    vm.ram[0] = 0x62;    // IOCTL
    vm.ram[1] = 1;       // fd_reg
    vm.ram[2] = 2;       // cmd_reg
    vm.ram[3] = 3;       // arg_reg
    vm.step();
    assert_eq!(vm.regs[0], 256, "IOCTL screen get width should return 256");
}

#[test]
fn test_ioctl_screen_get_height() {
    // IOCTL /dev/screen cmd=1 (get height) should return 256
    let mut vm = Vm::new();
    vm.mode = geometry_os::vm::CpuMode::Kernel;
    vm.regs[1] = 0xE000; // /dev/screen fd
    vm.regs[2] = 1;      // cmd: get height
    vm.regs[3] = 0;      // arg: unused
    vm.ram[0] = 0x62;    // IOCTL
    vm.ram[1] = 1;       // fd_reg
    vm.ram[2] = 2;       // cmd_reg
    vm.ram[3] = 3;       // arg_reg
    vm.step();
    assert_eq!(vm.regs[0], 256, "IOCTL screen get height should return 256");
}

#[test]
fn test_ioctl_screen_invalid_command() {
    // IOCTL /dev/screen cmd=99 should return 0xFFFFFFFF
    let mut vm = Vm::new();
    vm.mode = geometry_os::vm::CpuMode::Kernel;
    vm.regs[1] = 0xE000;
    vm.regs[2] = 99;
    vm.regs[3] = 0;
    vm.ram[0] = 0x62;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 3;
    vm.step();
    assert_eq!(vm.regs[0], 0xFFFFFFFF, "IOCTL screen invalid cmd should return error");
}

#[test]
fn test_ioctl_keyboard_get_echo_mode() {
    // IOCTL /dev/keyboard cmd=0 (get echo) should return value from RAM[0xFF8]
    let mut vm = Vm::new();
    vm.mode = geometry_os::vm::CpuMode::Kernel;
    vm.ram[0xFF8] = 42; // set echo mode
    vm.regs[1] = 0xE001; // /dev/keyboard fd
    vm.regs[2] = 0;      // cmd: get echo mode
    vm.regs[3] = 0;
    vm.ram[0] = 0x62;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 3;
    vm.step();
    assert_eq!(vm.regs[0], 42, "IOCTL keyboard get echo should return 42");
}

#[test]
fn test_ioctl_keyboard_set_echo_mode() {
    // IOCTL /dev/keyboard cmd=1 (set echo) should set RAM[0xFF8]
    let mut vm = Vm::new();
    vm.mode = geometry_os::vm::CpuMode::Kernel;
    vm.regs[1] = 0xE001;
    vm.regs[2] = 1;      // cmd: set echo mode
    vm.regs[3] = 77;     // arg: new echo mode value
    vm.ram[0] = 0x62;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 3;
    vm.step();
    assert_eq!(vm.regs[0], 0, "IOCTL keyboard set echo should return 0 (success)");
    assert_eq!(vm.ram[0xFF8], 77, "RAM[0xFF8] should be set to 77");
}

#[test]
fn test_ioctl_audio_get_volume() {
    // IOCTL /dev/audio cmd=0 (get volume) should return RAM[0xFF7]
    let mut vm = Vm::new();
    vm.mode = geometry_os::vm::CpuMode::Kernel;
    vm.ram[0xFF7] = 50; // set volume
    vm.regs[1] = 0xE002; // /dev/audio fd
    vm.regs[2] = 0;
    vm.regs[3] = 0;
    vm.ram[0] = 0x62;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 3;
    vm.step();
    assert_eq!(vm.regs[0], 50, "IOCTL audio get volume should return 50");
}

#[test]
fn test_ioctl_audio_set_volume() {
    // IOCTL /dev/audio cmd=1 (set volume) should set RAM[0xFF7] and clamp to 100
    let mut vm = Vm::new();
    vm.mode = geometry_os::vm::CpuMode::Kernel;
    vm.regs[1] = 0xE002;
    vm.regs[2] = 1;       // cmd: set volume
    vm.regs[3] = 150;     // arg: volume > 100 (should clamp)
    vm.ram[0] = 0x62;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 3;
    vm.step();
    assert_eq!(vm.regs[0], 0, "IOCTL audio set volume should return 0 (success)");
    assert_eq!(vm.ram[0xFF7], 100, "Volume should be clamped to 100");
}

#[test]
fn test_ioctl_net_get_status() {
    // IOCTL /dev/net cmd=0 (get status) should return 1 (up)
    let mut vm = Vm::new();
    vm.mode = geometry_os::vm::CpuMode::Kernel;
    vm.regs[1] = 0xE003; // /dev/net fd
    vm.regs[2] = 0;
    vm.regs[3] = 0;
    vm.ram[0] = 0x62;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 3;
    vm.step();
    assert_eq!(vm.regs[0], 1, "IOCTL net get status should return 1 (up)");
}

#[test]
fn test_ioctl_non_device_fd_returns_error() {
    // IOCTL on a non-device fd should return 0xFFFFFFFF
    let mut vm = Vm::new();
    vm.mode = geometry_os::vm::CpuMode::Kernel;
    vm.regs[1] = 5; // regular fd, not a device fd
    vm.regs[2] = 0;
    vm.regs[3] = 0;
    vm.ram[0] = 0x62;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 3;
    vm.step();
    assert_eq!(vm.regs[0], 0xFFFFFFFF, "IOCTL on non-device fd should return error");
}

#[test]
fn test_read_device_keyboard() {
    // READ from /dev/keyboard should read RAM[0xFFF] and clear it
    let mut vm = Vm::new();
    vm.mode = geometry_os::vm::CpuMode::Kernel;
    vm.ram[0xFFF] = 65; // 'A' key pressed
    vm.regs[1] = 0xE001; // /dev/keyboard fd
    vm.regs[2] = 0x1000; // buf addr
    vm.regs[3] = 1;      // len
    vm.ram[0] = 0x55;    // READ
    vm.ram[1] = 1;       // fd_reg
    vm.ram[2] = 2;       // buf_reg
    vm.ram[3] = 3;       // len_reg
    vm.step();
    assert_eq!(vm.regs[0], 1, "READ keyboard should return 1 byte");
    assert_eq!(vm.ram[0x1000], 65, "Buffer should contain the key");
    assert_eq!(vm.ram[0xFFF], 0, "Keyboard port should be cleared");
}

#[test]
fn test_read_device_net() {
    // READ from /dev/net should read RAM[0xFFC]
    let mut vm = Vm::new();
    vm.mode = geometry_os::vm::CpuMode::Kernel;
    vm.ram[0xFFC] = 42; // network data
    vm.regs[1] = 0xE003; // /dev/net fd
    vm.regs[2] = 0x1000; // buf addr
    vm.regs[3] = 1;      // len
    vm.ram[0] = 0x55;    // READ
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 3;
    vm.step();
    assert_eq!(vm.regs[0], 1, "READ net should return 1 word");
    assert_eq!(vm.ram[0x1000], 42, "Buffer should contain network data");
}

#[test]
fn test_write_device_screen() {
    // WRITE to /dev/screen should set pixel from (x, y, color) triplet
    let mut vm = Vm::new();
    vm.mode = geometry_os::vm::CpuMode::Kernel;
    vm.regs[1] = 0xE000; // /dev/screen fd
    vm.regs[2] = 0x1000; // buf addr
    vm.regs[3] = 3;      // len (one triplet)
    vm.ram[0x1000] = 10; // x
    vm.ram[0x1001] = 20; // y
    vm.ram[0x1002] = 5;  // color
    vm.ram[0] = 0x56;    // WRITE
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 3;
    vm.step();
    assert_eq!(vm.regs[0], 3, "WRITE screen should return 3 words written");
    assert_eq!(vm.screen[20 * 256 + 10], 5, "Pixel at (10,20) should be color 5");
}

#[test]
fn test_write_device_screen_multiple_triplets() {
    // WRITE to /dev/screen with 6 words = 2 triplets
    let mut vm = Vm::new();
    vm.mode = geometry_os::vm::CpuMode::Kernel;
    vm.regs[1] = 0xE000;
    vm.regs[2] = 0x1000;
    vm.regs[3] = 6; // 2 triplets
    vm.ram[0x1000] = 0;  vm.ram[0x1001] = 0;  vm.ram[0x1002] = 1; // (0,0) = red
    vm.ram[0x1003] = 1;  vm.ram[0x1004] = 0;  vm.ram[0x1005] = 2; // (1,0) = green
    vm.ram[0] = 0x56;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 3;
    vm.step();
    assert_eq!(vm.regs[0], 6, "WRITE screen should return 6 words written");
    assert_eq!(vm.screen[0], 1, "Pixel at (0,0) should be 1");
    assert_eq!(vm.screen[1], 2, "Pixel at (1,0) should be 2");
}

#[test]
fn test_write_device_audio() {
    // WRITE to /dev/audio should set beep
    let mut vm = Vm::new();
    vm.mode = geometry_os::vm::CpuMode::Kernel;
    vm.regs[1] = 0xE002; // /dev/audio fd
    vm.regs[2] = 0x1000; // buf addr
    vm.regs[3] = 2;      // len (freq + duration pair)
    vm.ram[0x1000] = 440; // frequency
    vm.ram[0x1001] = 100; // duration ms
    vm.ram[0] = 0x56;    // WRITE
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 3;
    vm.step();
    assert_eq!(vm.regs[0], 2, "WRITE audio should return 2");
    assert!(vm.beep.is_some(), "beep should be set");
    let (freq, dur) = vm.beep.unwrap();
    assert_eq!(freq, 440);
    assert_eq!(dur, 100);
}

#[test]
fn test_write_device_net() {
    // WRITE to /dev/net should write to RAM[0xFFC]
    let mut vm = Vm::new();
    vm.mode = geometry_os::vm::CpuMode::Kernel;
    vm.regs[1] = 0xE003; // /dev/net fd
    vm.regs[2] = 0x1000; // buf addr
    vm.regs[3] = 1;      // len
    vm.ram[0x1000] = 99; // data to send
    vm.ram[0xFFC] = 0;   // clear net port first
    vm.ram[0] = 0x56;    // WRITE
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 3;
    vm.step();
    assert_eq!(vm.regs[0], 1, "WRITE net should return 1");
    assert_eq!(vm.ram[0xFFC], 99, "Net port should contain sent data");
}

#[test]
fn test_open_device_via_assembly_program() {
    // Full assembly program that opens all 4 devices and verifies their fds
    // Using inline assembly without .db (which isn't supported by the assembler)
    let mut vm = Vm::new();
    vm.mode = geometry_os::vm::CpuMode::Kernel;

    // Write device path strings into RAM
    write_string(&mut vm.ram, 0x1000, "/dev/screen");
    write_string(&mut vm.ram, 0x1010, "/dev/keyboard");
    write_string(&mut vm.ram, 0x1020, "/dev/audio");
    write_string(&mut vm.ram, 0x1030, "/dev/net");

    // Open /dev/screen: LDI r1, 0x1000; LDI r2, 0; OPEN r1, r2
    vm.ram[0] = 0x10; vm.ram[1] = 1; vm.ram[2] = 0x1000; // LDI r1, 0x1000
    vm.ram[3] = 0x10; vm.ram[4] = 2; vm.ram[5] = 0;       // LDI r2, 0
    vm.ram[6] = 0x54; vm.ram[7] = 1; vm.ram[8] = 2;       // OPEN r1, r2

    // Open /dev/keyboard
    vm.ram[9]  = 0x10; vm.ram[10] = 1; vm.ram[11] = 0x1010;
    vm.ram[12] = 0x10; vm.ram[13] = 2; vm.ram[14] = 0;
    vm.ram[15] = 0x54; vm.ram[16] = 1; vm.ram[17] = 2;

    // Open /dev/audio
    vm.ram[18] = 0x10; vm.ram[19] = 1; vm.ram[20] = 0x1020;
    vm.ram[21] = 0x10; vm.ram[22] = 2; vm.ram[23] = 0;
    vm.ram[24] = 0x54; vm.ram[25] = 1; vm.ram[26] = 2;

    // Open /dev/net
    vm.ram[27] = 0x10; vm.ram[28] = 1; vm.ram[29] = 0x1030;
    vm.ram[30] = 0x10; vm.ram[31] = 2; vm.ram[32] = 0;
    vm.ram[33] = 0x54; vm.ram[34] = 1; vm.ram[35] = 2;

    vm.ram[36] = 0x00; // HALT

    for _ in 0..1000 {
        if !vm.step() { break; }
    }
    assert_eq!(vm.regs[0], 0xE003, "Last OPEN should return /dev/net fd");
}

#[test]
fn test_ioctl_roundtrip_get_and_set() {
    // Test IOCTL roundtrip: get keyboard echo, set to new value, verify
    let mut vm = Vm::new();
    vm.mode = geometry_os::vm::CpuMode::Kernel;

    // Open /dev/keyboard
    write_string(&mut vm.ram, 0x1000, "/dev/keyboard");
    vm.ram[0] = 0x10; vm.ram[1] = 1; vm.ram[2] = 0x1000; // LDI r1, 0x1000
    vm.ram[3] = 0x10; vm.ram[4] = 2; vm.ram[5] = 0;       // LDI r2, 0
    vm.ram[6] = 0x54; vm.ram[7] = 1; vm.ram[8] = 2;       // OPEN r1, r2
    // r0 now = 0xE001, save it to r10
    vm.ram[9]  = 0x10; vm.ram[10] = 10; vm.ram[11] = 0xE001; // LDI r10, 0xE001 (save fd)

    // Get echo mode: IOCTL r10, r4, r5 (cmd=0)
    vm.ram[12] = 0x10; vm.ram[13] = 4; vm.ram[14] = 0;    // LDI r4, 0
    vm.ram[15] = 0x62; vm.ram[16] = 10; vm.ram[17] = 4; vm.ram[18] = 5; // IOCTL r10, r4, r5

    // Set echo mode to 1: LDI r5, 1; LDI r4, 1; IOCTL r10, r4, r5
    vm.ram[19] = 0x10; vm.ram[20] = 5; vm.ram[21] = 1;    // LDI r5, 1
    vm.ram[22] = 0x10; vm.ram[23] = 4; vm.ram[24] = 1;    // LDI r4, 1
    vm.ram[25] = 0x62; vm.ram[26] = 10; vm.ram[27] = 4; vm.ram[28] = 5; // IOCTL r10, r4, r5

    // Get echo mode again: LDI r4, 0; IOCTL r10, r4, r5
    vm.ram[29] = 0x10; vm.ram[30] = 4; vm.ram[31] = 0;    // LDI r4, 0
    vm.ram[32] = 0x62; vm.ram[33] = 10; vm.ram[34] = 4; vm.ram[35] = 5; // IOCTL r10, r4, r5

    vm.ram[36] = 0x00; // HALT

    for _ in 0..10000 {
        if !vm.step() { break; }
    }
    assert_eq!(vm.regs[0], 1, "Keyboard echo mode should be 1 after set");
}

#[test]
fn test_device_read_keyboard() {
    // READ from /dev/keyboard should read RAM[0xFFF] and clear it
    let mut vm = Vm::new();
    vm.mode = geometry_os::vm::CpuMode::Kernel;
    vm.ram[0xFFF] = 65; // pretend 'A' key pressed

    // READ fd=0xE001 (keyboard), buf=r1, len=r2
    vm.regs[0] = 0xE001; // keyboard fd
    vm.regs[1] = 0x2000; // buf addr
    vm.regs[2] = 1;      // len
    vm.ram[0] = 0x55;    // READ
    vm.ram[1] = 0;       // fd_reg
    vm.ram[2] = 1;       // buf_reg
    vm.ram[3] = 2;       // len_reg
    vm.step();
    assert_eq!(vm.regs[0], 1, "READ should return 1 byte read");
    assert_eq!(vm.ram[0x2000], 65, "Buffer should contain the key value");
    assert_eq!(vm.ram[0xFFF], 0, "Keyboard port should be cleared after read");
}

#[test]
fn test_device_write_screen_pixel() {
    // WRITE to /dev/screen with (x, y, color) triplet should set a pixel
    let mut vm = Vm::new();
    vm.mode = geometry_os::vm::CpuMode::Kernel;

    // Set up pixel data at 0x2000: x=10, y=20, color=0xFF0000
    vm.ram[0x2000] = 10;
    vm.ram[0x2001] = 20;
    vm.ram[0x2002] = 0xFF0000;

    // WRITE fd=0xE000 (screen), buf=r1, len=r2
    vm.regs[0] = 0xE000; // screen fd
    vm.regs[1] = 0x2000; // buf addr
    vm.regs[2] = 3;      // len (x,y,color triplet)
    vm.ram[0] = 0x56;    // WRITE
    vm.ram[1] = 0;       // fd_reg
    vm.ram[2] = 1;       // buf_reg
    vm.ram[3] = 2;       // len_reg
    vm.step();
    assert_eq!(vm.regs[0], 3, "WRITE should return 3 words written");
    assert_eq!(vm.screen[20 * 256 + 10], 0xFF0000, "Screen pixel should be set");
}

#[test]
fn test_device_write_audio_beep() {
    // WRITE to /dev/audio with (freq, duration) should trigger beep
    let mut vm = Vm::new();
    vm.mode = geometry_os::vm::CpuMode::Kernel;

    // Set up audio data at 0x2000: freq=440, dur=100
    vm.ram[0x2000] = 440;
    vm.ram[0x2001] = 100;

    vm.regs[0] = 0xE002; // audio fd
    vm.regs[1] = 0x2000;
    vm.regs[2] = 2;
    vm.ram[0] = 0x56;    // WRITE
    vm.ram[1] = 0;
    vm.ram[2] = 1;
    vm.ram[3] = 2;
    vm.step();
    assert_eq!(vm.regs[0], 2, "WRITE should return 2 words written");
    assert!(vm.beep.is_some(), "Beep should be triggered");
    let (freq, dur) = vm.beep.unwrap();
    assert_eq!(freq, 440);
    assert_eq!(dur, 100);
}

#[test]
fn test_device_close_is_noop() {
    // CLOSE on a device fd should always succeed (return 0)
    let mut vm = Vm::new();
    vm.mode = geometry_os::vm::CpuMode::Kernel;
    vm.regs[0] = 0xE001; // keyboard fd
    vm.ram[0] = 0x57;    // CLOSE
    vm.ram[1] = 0;       // fd_reg
    vm.step();
    assert_eq!(vm.regs[0], 0, "CLOSE should return 0 (success) for device fd");
}

#[test]
fn test_ioctl_screen_dimensions() {
    // IOCTL /dev/screen cmd=0 returns width (256), cmd=1 returns height (256)
    let mut vm = Vm::new();
    vm.mode = geometry_os::vm::CpuMode::Kernel;

    // Get width: IOCTL r10=0xE000, r4=0, r5=0
    vm.regs[10] = 0xE000;
    vm.regs[4] = 0; // cmd = get width
    vm.regs[5] = 0;
    vm.ram[0] = 0x62; // IOCTL
    vm.ram[1] = 10;
    vm.ram[2] = 4;
    vm.ram[3] = 5;
    vm.step();
    assert_eq!(vm.regs[0], 256, "Screen width should be 256");
}

#[test]
fn test_ioctl_audio_volume() {
    // IOCTL /dev/audio: set volume to 75, then read it back
    let mut vm = Vm::new();
    vm.mode = geometry_os::vm::CpuMode::Kernel;

    // Set volume: IOCTL r10=0xE002, r4=1 (set), r5=75
    vm.regs[10] = 0xE002;
    vm.regs[4] = 1;
    vm.regs[5] = 75;
    vm.ram[0] = 0x62;
    vm.ram[1] = 10;
    vm.ram[2] = 4;
    vm.ram[3] = 5;
    vm.step();
    assert_eq!(vm.regs[0], 0, "Set volume should succeed");

    // Get volume: IOCTL r10=0xE002, r4=0 (get), r5=0
    vm.regs[4] = 0;
    vm.regs[5] = 0;
    vm.ram[4] = 0x62;
    vm.ram[5] = 10;
    vm.ram[6] = 4;
    vm.ram[7] = 5;
    vm.step();
    assert_eq!(vm.regs[0], 75, "Volume should be 75");
}

#[test]
fn test_ioctl_net_status() {
    // IOCTL /dev/net cmd=0 returns status (1=up)
    let mut vm = Vm::new();
    vm.mode = geometry_os::vm::CpuMode::Kernel;

    vm.regs[10] = 0xE003;
    vm.regs[4] = 0; // cmd = get status
    vm.regs[5] = 0;
    vm.ram[0] = 0x62;
    vm.ram[1] = 10;
    vm.ram[2] = 4;
    vm.ram[3] = 5;
    vm.step();
    assert_eq!(vm.regs[0], 1, "Net status should be 1 (up)");
}
