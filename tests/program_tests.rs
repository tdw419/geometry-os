// Integration tests for Geometry OS programs
//
// Each test assembles a .asm file, loads it into the VM, runs it,
// and verifies the output (screen pixels, register values, etc.)

use geometry_os::assembler::assemble;
use geometry_os::vm::{Vm, CpuMode, SYSCALL_TABLE, HW_REGS_START};

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
    assert_eq!(vm.processes[0].pc, 0x200);
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
        pc: 0, regs: [0; 32], halted: false, pid: 1,
    });
    assert_eq!(vm.active_process_count(), 1);
    vm.processes.push(geometry_os::vm::SpawnedProcess {
        pc: 0, regs: [0; 32], halted: true, pid: 2,
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


// == Phase 23: Kernel Boundary Tests ==

#[test]
fn test_vm_starts_in_kernel_mode() {
    let vm = Vm::new();
    assert_eq!(vm.cpu_mode, CpuMode::Kernel, "VM should start in Kernel mode (backward compatible)");
}

#[test]
fn test_vm_reset_returns_to_kernel_mode() {
    let mut vm = Vm::new();
    vm.cpu_mode = CpuMode::User;
    vm.syscall_return_pc = 42;
    vm.reset();
    assert_eq!(vm.cpu_mode, CpuMode::Kernel, "reset() should restore Kernel mode");
    assert_eq!(vm.syscall_return_pc, 0, "reset() should clear syscall_return_pc");
}

#[test]
fn test_syscall_assembles() {
    let source = "SYSCALL 1\nHALT";
    let result = assemble(source, 0);
    assert!(result.is_ok(), "SYSCALL should assemble");
    let pixels = &result.unwrap().pixels;
    assert_eq!(pixels[0], 0x52, "SYSCALL opcode should be 0x52");
    assert_eq!(pixels[1], 1, "SYSCALL argument should be the syscall number");
}

#[test]
fn test_retk_assembles() {
    let source = "RETK\nHALT";
    let result = assemble(source, 0);
    assert!(result.is_ok(), "RETK should assemble");
    let pixels = &result.unwrap().pixels;
    assert_eq!(pixels[0], 0x53, "RETK opcode should be 0x53");
}

#[test]
fn test_syscall_dispatches_to_handler() {
    let mut vm = Vm::new();
    vm.ram[SYSCALL_TABLE] = 100;
    vm.ram[0] = 0x52;
    vm.ram[1] = 0;
    vm.ram[2] = 0x00;
    vm.ram[100] = 0x00;
    vm.pc = 0;
    vm.cpu_mode = CpuMode::User;

    vm.step();

    assert_eq!(vm.cpu_mode, CpuMode::Kernel, "SYSCALL should switch to Kernel mode");
    assert_eq!(vm.pc, 100, "SYSCALL should jump to handler address");
    assert_eq!(vm.syscall_return_pc, 2, "SYSCALL should save return PC");
}

#[test]
fn test_retk_returns_to_user_mode() {
    let mut vm = Vm::new();
    vm.cpu_mode = CpuMode::Kernel;
    vm.syscall_return_pc = 50;
    vm.ram[0] = 0x53;
    vm.pc = 0;

    vm.step();

    assert_eq!(vm.cpu_mode, CpuMode::User, "RETK should switch to User mode");
    assert_eq!(vm.pc, 50, "RETK should jump to saved return PC");
}

#[test]
fn test_syscall_retk_roundtrip() {
    let mut vm = Vm::new();
    vm.cpu_mode = CpuMode::User;
    vm.ram[SYSCALL_TABLE + 5] = 200;
    vm.ram[0] = 0x52;
    vm.ram[1] = 5;
    vm.ram[2] = 0x00;
    vm.ram[200] = 0x53;
    vm.pc = 0;

    vm.step();
    assert_eq!(vm.cpu_mode, CpuMode::Kernel);
    assert_eq!(vm.pc, 200);

    vm.step();
    assert_eq!(vm.cpu_mode, CpuMode::User);
    assert_eq!(vm.pc, 2);
}

#[test]
fn test_syscall_unregistered_returns_error() {
    let mut vm = Vm::new();
    vm.cpu_mode = CpuMode::User;
    vm.ram[SYSCALL_TABLE + 10] = 0;
    vm.ram[0] = 0x52;
    vm.ram[1] = 10;
    vm.ram[2] = 0x00;
    vm.pc = 0;

    vm.step();

    assert_eq!(vm.regs[0], 0xFFFFFFFF, "Unregistered syscall should return error in r0");
    assert_eq!(vm.cpu_mode, CpuMode::User, "Should stay in user mode");
    assert_eq!(vm.pc, 2, "Should continue to next instruction");
}

#[test]
fn test_syscall_out_of_bounds_returns_error() {
    let mut vm = Vm::new();
    vm.cpu_mode = CpuMode::User;
    vm.ram[0] = 0x52;
    vm.ram[1] = 300;
    vm.ram[2] = 0x00;
    vm.pc = 0;

    vm.step();

    assert_eq!(vm.regs[0], 0xFFFFFFFF, "Out-of-bounds syscall should return error");
    assert_eq!(vm.cpu_mode, CpuMode::User);
}

#[test]
fn test_ikey_blocked_in_user_mode() {
    let mut vm = Vm::new();
    vm.cpu_mode = CpuMode::User;
    vm.ram[0xFFF] = 65;
    vm.ram[0] = 0x48;
    vm.ram[1] = 1;
    vm.pc = 0;

    vm.step();

    assert!(vm.halted, "IKEY in user mode should cause protection fault");
    assert_eq!(vm.regs[1], 0, "Should NOT read the key");
}

#[test]
fn test_ikey_allowed_in_kernel_mode() {
    let mut vm = Vm::new();
    vm.cpu_mode = CpuMode::Kernel;
    vm.ram[0xFFF] = 65;
    vm.ram[0] = 0x48;
    vm.ram[1] = 1;
    vm.pc = 0;

    vm.step();

    assert!(!vm.halted, "IKEY in kernel mode should not halt");
    assert_eq!(vm.regs[1], 65, "Should read the key");
}

#[test]
fn test_store_to_hw_regs_blocked_in_user_mode() {
    let mut vm = Vm::new();
    vm.cpu_mode = CpuMode::User;
    vm.regs[0] = 0xFFA;
    vm.regs[1] = 42;
    vm.ram[0] = 0x12;
    vm.ram[1] = 0;
    vm.ram[2] = 1;
    vm.pc = 0;

    vm.step();

    assert!(vm.halted, "STORE to HW regs in user mode should cause protection fault");
    assert_ne!(vm.ram[0xFFA], 42, "Should NOT write");
}

#[test]
fn test_store_to_normal_ram_allowed_in_user_mode() {
    let mut vm = Vm::new();
    vm.cpu_mode = CpuMode::User;
    vm.regs[0] = 100;
    vm.regs[1] = 42;
    vm.ram[0] = 0x12;
    vm.ram[1] = 0;
    vm.ram[2] = 1;
    vm.pc = 0;

    vm.step();

    assert!(!vm.halted, "STORE to normal RAM in user mode should work");
    assert_eq!(vm.ram[100], 42);
}

#[test]
fn test_store_to_hw_regs_allowed_in_kernel_mode() {
    let mut vm = Vm::new();
    vm.cpu_mode = CpuMode::Kernel;
    vm.regs[0] = 0xFFA;
    vm.regs[1] = 42;
    vm.ram[0] = 0x12;
    vm.ram[1] = 0;
    vm.ram[2] = 1;
    vm.pc = 0;

    vm.step();

    assert!(!vm.halted, "STORE to HW regs in kernel mode should work");
    assert_eq!(vm.ram[0xFFA], 42);
}

#[test]
fn test_syscall_handler_can_use_restricted_opcodes() {
    let mut vm = Vm::new();
    vm.cpu_mode = CpuMode::User;
    vm.ram[0xFFF] = 72;
    vm.ram[SYSCALL_TABLE] = 100;
    vm.ram[0] = 0x52;
    vm.ram[1] = 0;
    vm.ram[2] = 0x00;
    vm.ram[100] = 0x48;
    vm.ram[101] = 1;
    vm.ram[102] = 0x53;
    vm.pc = 0;

    vm.step(); // SYSCALL -> kernel
    assert_eq!(vm.cpu_mode, CpuMode::Kernel);

    vm.step(); // IKEY r1 (allowed in kernel)
    assert_eq!(vm.regs[1], 72, "Handler should read keyboard");
    assert_eq!(vm.ram[0xFFF], 0, "IKEY should clear port");

    vm.step(); // RETK -> user
    assert_eq!(vm.cpu_mode, CpuMode::User);
    assert_eq!(vm.pc, 2);
}

#[test]
fn test_syscall_passes_arguments_via_registers() {
    let mut vm = Vm::new();
    vm.cpu_mode = CpuMode::User;
    vm.regs[1] = 100;
    vm.regs[2] = 200;
    vm.regs[3] = 300;
    vm.ram[SYSCALL_TABLE + 1] = 50;
    vm.ram[0] = 0x52;
    vm.ram[1] = 1;
    vm.ram[2] = 0x00;
    vm.ram[50] = 0x20; vm.ram[51] = 0; vm.ram[52] = 1; // ADD r0, r1
    vm.ram[53] = 0x20; vm.ram[54] = 0; vm.ram[55] = 2; // ADD r0, r2
    vm.ram[56] = 0x20; vm.ram[57] = 0; vm.ram[58] = 3; // ADD r0, r3
    vm.ram[59] = 0x53; // RETK
    vm.pc = 0;

    for _ in 0..20 {
        if !vm.step() { break; }
    }

    assert_eq!(vm.regs[0], 600, "r0 = 100+200+300");
    assert_eq!(vm.cpu_mode, CpuMode::User);
}

#[test]
fn test_disassemble_syscall_and_retk() {
    let mut vm = Vm::new();
    vm.ram[0] = 0x52;
    vm.ram[1] = 5;
    vm.ram[2] = 0x53;

    let (mnemonic, len) = vm.disassemble_at(0);
    assert_eq!(len, 2, "SYSCALL should be 2 words");
    assert!(mnemonic.contains("SYSCALL"), "got: {}", mnemonic);

    let (mnemonic2, len2) = vm.disassemble_at(2);
    assert_eq!(len2, 1, "RETK should be 1 word");
    assert!(mnemonic2.contains("RETK"), "got: {}", mnemonic2);
}

// ── SYSCALL / RETK / CPU MODE ──────────────────────────────────

#[test]
fn test_vm_starts_in_kernel_mode() {
    let vm = Vm::new();
    assert_eq!(vm.cpu_mode, CpuMode::Kernel, "VM should start in Kernel mode");
}

#[test]
fn test_syscall_assembles() {
    let source = "SYSCALL 0\nSYSCALL 5\nHALT";
    let asm = assemble(source, 0).unwrap();
    assert_eq!(asm.pixels[0], 0x52, "SYSCALL opcode");
    assert_eq!(asm.pixels[1], 0, "syscall number 0");
    assert_eq!(asm.pixels[2], 0x52, "SYSCALL opcode");
    assert_eq!(asm.pixels[3], 5, "syscall number 5");
    assert_eq!(asm.pixels[4], 0x00, "HALT opcode");
}

#[test]
fn test_retk_assembles() {
    let source = "RETK\nHALT";
    let asm = assemble(source, 0).unwrap();
    assert_eq!(asm.pixels[0], 0x53, "RETK opcode");
    assert_eq!(asm.pixels[1], 0x00, "HALT opcode");
}

#[test]
fn test_syscall_unregistered_returns_error() {
    let source = "SYSCALL 42\nHALT";
    let asm = assemble(source, 0).unwrap();
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() { vm.ram[i] = v; }
    for _ in 0..100 { if !vm.step() { break; } }
    assert!(vm.halted);
    assert_eq!(vm.regs[0], 0xFFFFFFFF, "unregistered syscall should set r0 = error");
    assert_eq!(vm.cpu_mode, CpuMode::Kernel, "mode should remain kernel");
}

#[test]
fn test_syscall_dispatches_to_handler() {
    let mut vm = Vm::new();
    vm.ram[SYSCALL_TABLE + 1] = 20; // handler at addr 20
    // Code at 0: SYSCALL 1, LDI r2, 0xBAD, HALT
    vm.ram[0] = 0x52; vm.ram[1] = 1;
    vm.ram[2] = 0x10; vm.ram[3] = 2; vm.ram[4] = 0xBAD;
    vm.ram[5] = 0x00; // HALT
    // Handler at 20: LDI r0, 99, RETK
    vm.ram[20] = 0x10; vm.ram[21] = 0; vm.ram[22] = 99;
    vm.ram[23] = 0x53; // RETK
    vm.pc = 0;
    vm.step(); // SYSCALL 1
    assert_eq!(vm.cpu_mode, CpuMode::Kernel, "should be in kernel after SYSCALL");
    assert_eq!(vm.pc, 20, "should jump to handler");
    vm.step(); // LDI r0, 99
    assert_eq!(vm.regs[0], 99, "handler should set r0=99");
    vm.step(); // RETK
    assert_eq!(vm.cpu_mode, CpuMode::User, "should be in user mode after RETK");
    assert_eq!(vm.pc, 2, "should return to instruction after SYSCALL");
    vm.step(); // LDI r2, 0xBAD
    assert_eq!(vm.regs[2], 0xBAD, "LDI should work in user mode");
    vm.step(); // HALT
    assert!(vm.halted);
}

#[test]
fn test_retk_returns_to_saved_pc() {
    let mut vm = Vm::new();
    vm.syscall_return_pc = 50;
    vm.cpu_mode = CpuMode::Kernel;
    vm.ram[200] = 0x53;
    vm.pc = 200;
    vm.step();
    assert_eq!(vm.cpu_mode, CpuMode::User, "RETK should switch to user mode");
    assert_eq!(vm.pc, 50, "RETK should jump to saved return PC");
}

#[test]
fn test_ikey_blocked_in_user_mode() {
    let source = "IKEY r1\nHALT";
    let asm = assemble(source, 0).unwrap();
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() { vm.ram[i] = v; }
    vm.cpu_mode = CpuMode::User;
    vm.ram[0xFFF] = 65;
    for _ in 0..100 { if !vm.step() { break; } }
    assert!(vm.halted, "IKEY in user mode should cause protection fault halt");
    assert_eq!(vm.regs[1], 0, "IKEY should not have executed in user mode");
}

#[test]
fn test_store_to_hw_regs_blocked_in_user_mode() {
    let source = "LDI r1, 0xFF01\nLDI r2, 42\nSTORE r1, r2\nHALT";
    let asm = assemble(source, 0).unwrap();
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() { vm.ram[i] = v; }
    vm.cpu_mode = CpuMode::User;
    for _ in 0..100 { if !vm.step() { break; } }
    assert!(vm.halted, "STORE to hw regs in user mode should fault");
    assert_eq!(vm.ram[0xFF01], 0, "STORE should not have written");
}

#[test]
fn test_store_to_normal_ram_ok_in_user_mode() {
    let source = "LDI r1, 0x0200\nLDI r2, 42\nSTORE r1, r2\nHALT";
    let asm = assemble(source, 0).unwrap();
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() { vm.ram[i] = v; }
    vm.cpu_mode = CpuMode::User;
    for _ in 0..100 { if !vm.step() { break; } }
    assert!(vm.halted);
    assert_eq!(vm.ram[0x0200], 42, "STORE to normal RAM should work in user mode");
}

#[test]
fn test_ikey_works_in_kernel_mode() {
    let source = "IKEY r1\nHALT";
    let asm = assemble(source, 0).unwrap();
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() { vm.ram[i] = v; }
    vm.cpu_mode = CpuMode::Kernel;
    vm.ram[0xFFF] = 65;
    for _ in 0..100 { if !vm.step() { break; } }
    assert!(vm.halted);
    assert_eq!(vm.regs[1], 65, "IKEY should work in kernel mode");
    assert_eq!(vm.ram[0xFFF], 0, "IKEY should clear port");
}

#[test]
fn test_disassemble_syscall() {
    let mut vm = Vm::new();
    vm.ram[500] = 0x52; vm.ram[501] = 7;
    let (mnem, length) = vm.disassemble_at(500);
    assert_eq!(mnem, "SYSCALL 7");
    assert_eq!(length, 2);
}

#[test]
fn test_disassemble_retk() {
    let mut vm = Vm::new();
    vm.ram[600] = 0x53;
    let (mnem, length) = vm.disassemble_at(600);
    assert_eq!(mnem, "RETK");
    assert_eq!(length, 1);
}

#[test]
fn test_syscall_table_constants() {
    assert_eq!(SYSCALL_TABLE, 0xFE00);
    assert_eq!(HW_REGS_START, 0xFF00);
    assert_eq!(HW_REGS_START - SYSCALL_TABLE, 256, "256 syscall slots");
}

#[test]
fn test_vm_reset_clears_cpu_mode() {
    let mut vm = Vm::new();
    vm.cpu_mode = CpuMode::User;
    vm.syscall_return_pc = 999;
    vm.reset();
    assert_eq!(vm.cpu_mode, CpuMode::Kernel);
    assert_eq!(vm.syscall_return_pc, 0);
}
