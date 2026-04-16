use super::*;



// ── ASM OPCODE ──────────────────────────────────────────────────

#[test]
fn test_asm_opcode_basic() {
    let mut vm = Vm::new();
    let source = "LDI r0, 42\nHALT\n";
    for (i, &byte) in source.as_bytes().iter().enumerate() {
        vm.ram[0x0800 + i] = byte as u32;
    }
    vm.ram[0x0800 + source.len()] = 0;
    let prog = assemble("LDI r5, 0x0800\nLDI r6, 0x1000\nASM r5, r6\nHALT\n", 0).expect("assembly should succeed");
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
    let prog = assemble("LDI r5, 0x0800\nLDI r6, 0x1000\nASM r5, r6\nHALT\n", 0).expect("assembly should succeed");
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

#[test]
fn test_phase48_registers_preserved_across_runnext() {
    // Phase 48: registers survive ASMSELF + RUNNEXT transition
    // 1. Set r5 = 12345 in parent context
    // 2. Write canvas code that reads r5 and adds 1 to r10 (r5 must be untouched)
    // 3. ASMSELF assembles canvas -> bytecode at 0x1000
    // 4. RUNNEXT jumps PC to 0x1000
    // 5. Verify: r5 == 12345 (preserved), r10 == 12346 (new code read r5 + 1)

    // Canvas source text: reads r5 into r10, adds 1, halts
    let canvas_source = "MOV r10, r5\nLDI r11, 1\nADD r10, r11\nHALT\n";

    // Bootstrap: ASMSELF then RUNNEXT
    let bootstrap = "ASMSELF\nRUNNEXT\n";

    let boot_asm = assemble(bootstrap, 0).expect("assembly should succeed");

    let mut vm = Vm::new();

    // Load bootstrap bytecode at address 0
    for (i, &w) in boot_asm.pixels.iter().enumerate() {
        vm.ram[i] = w;
    }

    // Write source text into canvas_buffer (ASMSELF reads canvas_buffer as text)
    // Each canvas row is 32 cells. Write chars sequentially, newlines as actual \n bytes.
    for (i, byte) in canvas_source.bytes().enumerate() {
        if i >= vm.canvas_buffer.len() { break; }
        vm.canvas_buffer[i] = byte as u32;
    }

    // Pre-set r5 = 12345
    vm.regs[5] = 12345;
    vm.pc = 0;
    vm.halted = false;

    // Run until halt
    for _ in 0..1000 {
        if !vm.step() { break; }
    }

    // r5 must be unchanged -- registers survive the transition
    assert_eq!(vm.regs[5], 12345,
        "r5 must be preserved across ASMSELF+RUNNEXT: got {}", vm.regs[5]);

    // r10 should be 12346 (new code read r5 and added 1)
    assert_eq!(vm.regs[10], 12346,
        "r10 should be 12346 (r5+1): got {}", vm.regs[10]);

    assert!(vm.halted, "VM should have halted after new code finished");
}

#[test]
fn test_hello_texti_vs_original() {
    // TEXTI hello vs original hello -- TEXTI should be much smaller
    let src_texti = "TEXTI 90, 120, \"Hello, World!\"\nHALT";
    let texti_asm = assemble(src_texti, 0).expect("assembly should succeed");
    let src_original = std::fs::read_to_string("programs/hello.asm").expect("filesystem operation failed");
    let original_asm = assemble(&src_original, 0).expect("assembly should succeed");
    // TEXTI version should be much smaller
    assert!(texti_asm.pixels.len() < original_asm.pixels.len() / 4,
        "TEXTI hello ({} words) should be < 1/4 of original ({} words)",
        texti_asm.pixels.len(), original_asm.pixels.len());
}



// === Self Writer (Phase 49: Pixel Driving Pixels) ===

#[test]
fn test_self_writer() {
    // self_writer.asm writes successor code to canvas via STORE,
    // compiles with ASMSELF, runs with RUNNEXT.
    // Successor: LDI r1, 42 / HALT
    // Expected: r1 = 42
    let vm = compile_run("programs/self_writer.asm");

    assert!(vm.halted, "self_writer should halt after successor runs");

    // r1 = 42 from successor (LDI r1, 42; HALT)
    assert_eq!(vm.regs[1], 42,
        "r1 should be 42: got {}", vm.regs[1]);

    // ASMSELF should have succeeded (bytecode word count > 0, not error)
    assert_ne!(vm.ram[0xFFD], 0xFFFFFFFF,
        "ASMSELF should not report error");
    assert!(vm.ram[0xFFD] > 0,
        "ASMSELF should produce bytecode words");

    // Verify canvas buffer has the successor text written to it
    // First char should be 'L' (76) from "LDI r0, 42"
    assert_eq!(vm.canvas_buffer[0], 76,
        "canvas[0] should be 'L' (76): got {}", vm.canvas_buffer[0]);
    // Second char should be 'D' (68)
    assert_eq!(vm.canvas_buffer[1], 68,
        "canvas[1] should be 'D' (68): got {}", vm.canvas_buffer[1]);
}



// === Evolving Counter (Phase 49: Pixel Driving Pixels) ===

#[test]
fn test_evolving_counter() {
    // evolving_counter.asm reads TICKS from RAM[0xFFE] each frame,
    // converts to 4 decimal ASCII digits, writes to canvas buffer at 0x8000-0x8003.
    // The grid IS the display -- digits change each frame.
    let vm = compile_run("programs/evolving_counter.asm");

    // Program is an infinite loop (FRAME+JMP), won't halt
    assert!(!vm.halted, "evolving_counter should not halt (infinite animation)");

    // After running, frame_count should be > 0 (many FRAME opcodes executed)
    assert!(vm.frame_count > 0, "frame_count should be > 0: got {}", vm.frame_count);

    // Canvas buffer positions 0-3 should contain ASCII digit characters ('0'-'9')
    for i in 0..4 {
        let val = vm.canvas_buffer[i];
        assert!(val >= 0x30 && val <= 0x39,
            "canvas[{}] should be ASCII digit (0x30-0x39): got 0x{:02X} ('{}')",
            i, val, if val >= 0x20 && val < 0x7F { val as u8 as char } else { '?' });
    }

    // Verify the 4 digits actually represent the frame count value
    // Extract the displayed number from canvas buffer
    let displayed = (vm.canvas_buffer[0] - 0x30) * 1000
                  + (vm.canvas_buffer[1] - 0x30) * 100
                  + (vm.canvas_buffer[2] - 0x30) * 10
                  + (vm.canvas_buffer[3] - 0x30);

    // The displayed count should match the frame_count mod 10000
    // (4-digit display wraps at 10000)
    let expected = vm.frame_count % 10000;
    assert_eq!(displayed, expected,
        "canvas digits should show frame_count mod 10000: expected {}, got {} (digits: {}{}{}{})",
        expected, displayed,
        vm.canvas_buffer[0] - 0x30, vm.canvas_buffer[1] - 0x30,
        vm.canvas_buffer[2] - 0x30, vm.canvas_buffer[3] - 0x30);
}



// === Register Dashboard (Phase 50: Pixel Driving Pixels) ===

#[test]
fn test_register_dashboard() {
    // register_dashboard.asm displays 16 registers (r1-r16) as 4-digit
    // decimal ASCII values on the canvas grid. r1 = frame counter,
    // r2-r16 derive from r1 via arithmetic. The grid IS the debug view.
    //
    // Run for limited steps (2000) so exactly 1 frame completes and r1 = 1.
    let source = std::fs::read_to_string("programs/register_dashboard.asm")
        .unwrap_or_else(|e| panic!("failed to read register_dashboard.asm: {}", e));
    let asm = assemble(&source, 0)
        .unwrap_or_else(|e| panic!("assembly failed: {}", e));
    let mut vm = Vm::new();
    for (i, &pixel) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() { vm.ram[i] = pixel; }
    }
    // Run exactly enough steps for 1 frame (682 = 4 init + 87 loop body - 1 JMP + 16*37 sub)
    for _ in 0..682 {
        if !vm.step() { break; }
    }

    // Program is an infinite animation loop (FRAME + JMP main_loop)
    assert!(!vm.halted, "register_dashboard should not halt");
    assert!(vm.frame_count > 0, "frame_count should be > 0: got {}", vm.frame_count);

    // After first FRAME, r1 = 1 (frame counter incremented once)
    // Verify r1's digits at canvas indices 0-3: "0001"
    assert_eq!(vm.canvas_buffer[0], 0x30, "r1 thousands should be '0'");
    assert_eq!(vm.canvas_buffer[1], 0x30, "r1 hundreds should be '0'");
    assert_eq!(vm.canvas_buffer[2], 0x30, "r1 tens should be '0'");
    assert_eq!(vm.canvas_buffer[3], 0x31, "r1 ones should be '1'");

    // r2 = r1*2 = 2 at canvas indices 4-7: "0002"
    assert_eq!(vm.canvas_buffer[7], 0x32, "r2 ones digit should be '2'");

    // r4 = r1*4 = 4 at canvas indices 12-15: "0004"
    assert_eq!(vm.canvas_buffer[15], 0x34, "r4 ones digit should be '4'");

    // r8 = r1<<4 = 16 at canvas indices 28-31: "0016"
    assert_eq!(vm.canvas_buffer[30], 0x31, "r8 tens digit should be '1'");
    assert_eq!(vm.canvas_buffer[31], 0x36, "r8 ones digit should be '6'");

    // r9 = NEG(r1) = 0xFFFFFFFF at canvas indices 32-35
    // 0xFFFFFFFF = 4294967295, last 4 decimal digits = "7295"
    assert_eq!(vm.canvas_buffer[32], 0x37, "r9 thousands digit should be '7'");
    assert_eq!(vm.canvas_buffer[33], 0x32, "r9 hundreds digit should be '2'");
    assert_eq!(vm.canvas_buffer[34], 0x39, "r9 tens digit should be '9'");
    assert_eq!(vm.canvas_buffer[35], 0x35, "r9 ones digit should be '5'");

    // r12 = (r1*r1)>>8 = 0 at canvas indices 44-47: "0000"
    assert_eq!(vm.canvas_buffer[47], 0x30, "r12 ones digit should be '0'");

    // r16 = r8-r1 = 16-1 = 15 at canvas indices 60-63: "0015"
    assert_eq!(vm.canvas_buffer[62], 0x31, "r16 tens digit should be '1'");
    assert_eq!(vm.canvas_buffer[63], 0x35, "r16 ones digit should be '5'");

    // Verify ALL 64 canvas positions (16 regs × 4 digits) contain ASCII digits
    for i in 0..64 {
        let val = vm.canvas_buffer[i];
        assert!(val >= 0x30 && val <= 0x39,
            "canvas[{}] should be ASCII digit (0x30-0x39): got 0x{:02X} ('{}')",
            i, val, if val >= 0x20 && val < 0x7F { val as u8 as char } else { '?' });
    }
}



// === Living Map (stateful world + simulated creatures) ===

#[test]
fn test_living_map_assembles() {
    let source = std::fs::read_to_string("programs/living_map.asm")
        .expect("living_map.asm should exist");
    assemble(&source, 0).expect("living_map.asm should assemble cleanly");
}

#[test]
fn test_living_map_runs() {
    let source = std::fs::read_to_string("programs/living_map.asm").expect("filesystem operation failed");
    let asm = assemble(&source, 0).expect("assembly should succeed");
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() { vm.ram[i] = v; }
    }
    vm.pc = 0;
    vm.halted = false;

    // Run for enough steps for several frames
    for _ in 0..2_000_000 {
        if vm.halted { break; }
        if !vm.step() { break; }
    }

    // Player at center of viewport
    assert_eq!(vm.ram[0x7803], 32, "player_world_x should be 32");
    assert_eq!(vm.ram[0x7804], 32, "player_world_y should be 32");

    // Screen should have terrain
    let non_black = vm.screen.iter().filter(|&&p| p != 0).count();
    assert!(non_black > 100, "Expected terrain on screen, got {} non-black pixels", non_black);

    // Should not halt
    assert!(!vm.halted, "living_map should not halt");
}

#[test]
fn test_living_map_draws_terrain() {
    let source = std::fs::read_to_string("programs/living_map.asm").expect("filesystem operation failed");
    let asm = assemble(&source, 0).expect("assembly should succeed");
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() { vm.ram[i] = v; }
    }

    // Run until first frame completes
    for _ in 0..1_000_000 {
        if vm.halted { break; }
        if !vm.step() { break; }
        if vm.frame_ready {
            vm.frame_ready = false;
            break;
        }
    }

    let non_black = vm.screen.iter().filter(|&&p| p != 0).count();
    assert!(non_black > 1000, "terrain should fill screen, got {} non-black pixels", non_black);
}

#[test]
fn test_living_map_draws_player() {
    let source = std::fs::read_to_string("programs/living_map.asm").expect("filesystem operation failed");
    let asm = assemble(&source, 0).expect("assembly should succeed");
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() { vm.ram[i] = v; }
    }

    for _ in 0..1_000_000 {
        if vm.halted { break; }
        if !vm.step() { break; }
        if vm.frame_ready {
            vm.frame_ready = false;
            break;
        }
    }

    // Player at pixel (128,128) as 4x4 white rectangle
    let white = 0xFFFFFFu32;
    assert_eq!(vm.screen[128 * 256 + 128], white, "player top-left should be white");
    assert_eq!(vm.screen[131 * 256 + 131], white, "player bottom-right should be white");
}

#[test]
fn test_living_map_footstep_trail() {
    let source = std::fs::read_to_string("programs/living_map.asm").expect("filesystem operation failed");
    let asm = assemble(&source, 0).expect("assembly should succeed");
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() { vm.ram[i] = v; }
    }

    // Press Right for several frames
    let mut frames = 0;
    for _ in 0..5_000_000 {
        if vm.halted { break; }
        vm.ram[0xFFB] = 8; // bit 3 = right
        if !vm.step() { break; }
        if vm.frame_ready {
            vm.frame_ready = false;
            frames += 1;
            if frames >= 10 { break; }
        }
    }

    let state_count = vm.ram[0x7807];
    assert!(state_count > 0, "should have footstep entries after moving, got state_count={}", state_count);

    let cam_x = vm.ram[0x7800];
    assert!(cam_x > 0, "camera should have moved right: camera_x={}", cam_x);
}
