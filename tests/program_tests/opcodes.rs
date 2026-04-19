use super::*;

// ── LINE / CIRCLE / SCROLL ─────────────────────────────────────

#[test]
fn test_line_opcode() {
    let source = "LDI r0, 0\nLDI r1, 0\nLDI r2, 255\nLDI r3, 255\nLDI r4, 0xFFFFFF\nLINE r0,r1,r2,r3,r4\nHALT";
    let asm = assemble(source, 0).expect("assembly should succeed");
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() {
        vm.ram[i] = v;
    }
    for _ in 0..100_000 {
        if !vm.step() {
            break;
        }
    }
    assert!(vm.halted);
    // diagonal should have pixels set at corners
    assert_eq!(vm.screen[0], 0xFFFFFF, "top-left pixel should be white");
    assert_eq!(
        vm.screen[255 * 256 + 255],
        0xFFFFFF,
        "bottom-right pixel should be white"
    );
}

#[test]
fn test_circle_opcode() {
    let source = "LDI r0, 128\nLDI r1, 128\nLDI r2, 50\nLDI r3, 0xFF0000\nCIRCLE r0,r1,r2,r3\nHALT";
    let asm = assemble(source, 0).expect("assembly should succeed");
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() {
        vm.ram[i] = v;
    }
    for _ in 0..100_000 {
        if !vm.step() {
            break;
        }
    }
    assert!(vm.halted);
    // top of circle: (128, 78) should be red
    assert_eq!(
        vm.screen[78 * 256 + 128],
        0xFF0000,
        "top of circle should be red"
    );
    // bottom: (128, 178)
    assert_eq!(
        vm.screen[178 * 256 + 128],
        0xFF0000,
        "bottom of circle should be red"
    );
}

#[test]
fn test_scroll_opcode() {
    let source =
        "LDI r0, 0\nLDI r1, 10\nLDI r2, 0xFFFFFF\nPSET r0,r1,r2\nLDI r3, 5\nSCROLL r3\nHALT";
    let asm = assemble(source, 0).expect("assembly should succeed");
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() {
        vm.ram[i] = v;
    }
    for _ in 0..100_000 {
        if !vm.step() {
            break;
        }
    }
    assert!(vm.halted);
    // pixel was at (0, 10), scroll 5 up -> should now be at (0, 5)
    assert_eq!(
        vm.screen[5 * 256 + 0],
        0xFFFFFF,
        "pixel should have scrolled to y=5"
    );
    // original location (0, 10) should still be white too (scrolled copy)
    // actually after scroll by 5, y=10 maps to y=5, and y=5 is now the pixel
    assert_eq!(
        vm.screen[10 * 256 + 0],
        0,
        "original y=10 should be 0 after scroll"
    );
}

// ── FRAME ──────────────────────────────────────────────────────

#[test]
fn test_frame_opcode() {
    // Program: fill red, FRAME, fill blue, HALT
    // After FRAME, frame_ready should be set; after running to HALT, screen is blue
    let source = "LDI r1, 0xFF0000\nFILL r1\nFRAME\nLDI r1, 0x0000FF\nFILL r1\nHALT";
    let asm = assemble(source, 0).expect("assembly should succeed");
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() {
        vm.ram[i] = v;
    }
    vm.pc = 0;
    // Run until first FRAME
    for _ in 0..10_000 {
        if !vm.step() || vm.frame_ready {
            break;
        }
    }
    assert!(vm.frame_ready, "FRAME should set frame_ready");
    // Screen should be red at this point
    assert_eq!(vm.screen[0], 0xFF0000, "screen should be red after FRAME");
    // Clear flag and run to halt
    vm.frame_ready = false;
    for _ in 0..10_000 {
        if !vm.step() {
            break;
        }
    }
    assert!(vm.halted);
    assert_eq!(vm.screen[0], 0x0000FF, "screen should be blue after HALT");
}

// ── NEG / IKEY ──────────────────────────────────────────────────

#[test]
fn test_neg_opcode() {
    let source = "LDI r1, 5\nNEG r1\nLDI r2, 3\nADD r2, r1\nHALT";
    let asm = assemble(source, 0).expect("assembly should succeed");
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() {
        vm.ram[i] = v;
    }
    for _ in 0..10_000 {
        if !vm.step() {
            break;
        }
    }
    assert!(vm.halted);
    // r1 = -5 (0xFFFFFFFB), r2 = 3 + (-5) = -2 (0xFFFFFFFE)
    assert_eq!(vm.regs[1], 0xFFFFFFFB, "NEG 5 should give 0xFFFFFFFB");
    assert_eq!(vm.regs[2], 0xFFFFFFFE, "3 + (-5) should give 0xFFFFFFFE");
}

#[test]
fn test_ikey_opcode() {
    let source = "IKEY r1\nHALT";
    let asm = assemble(source, 0).expect("assembly should succeed");
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() {
        vm.ram[i] = v;
    }
    // Simulate key press: write ASCII 'A' (65) to keyboard port
    vm.ram[0xFFF] = 65;
    for _ in 0..10_000 {
        if !vm.step() {
            break;
        }
    }
    assert!(vm.halted);
    assert_eq!(vm.regs[1], 65, "IKEY should read key code 65 into r1");
    assert_eq!(vm.ram[0xFFF], 0, "IKEY should clear the keyboard port");
}

// ── RAND ─────────────────────────────────────────────────────────

#[test]
fn test_rand_opcode() {
    let source = "RAND r1\nRAND r2\nRAND r3\nHALT";
    let asm = assemble(source, 0).expect("assembly should succeed");
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() {
        vm.ram[i] = v;
    }
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    assert!(vm.halted);
    // all three should be non-zero and different from each other
    assert_ne!(vm.regs[1], 0, "RAND should produce non-zero values");
    assert_ne!(
        vm.regs[1], vm.regs[2],
        "consecutive RAND values should differ"
    );
    assert_ne!(
        vm.regs[2], vm.regs[3],
        "consecutive RAND values should differ"
    );
}

// ── BEEP ────────────────────────────────────────────────────────

#[test]
fn test_beep_opcode() {
    // BEEP freq_reg, dur_reg -- set up freq in r1, dur in r2
    // We test that the VM doesn't crash and advances past BEEP
    let source = "LDI r1, 440\nLDI r2, 50\nBEEP r1, r2\nLDI r3, 1\nHALT";
    let asm = assemble(source, 0).expect("assembly should succeed");
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() {
        vm.ram[i] = v;
    }
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    assert!(vm.halted);
    assert_eq!(vm.regs[3], 1, "VM should execute past BEEP and set r3");
}

// ── NOTE ────────────────────────────────────────────────────────

#[test]
fn test_note_opcode_sine() {
    // NOTE wave_reg=0(sine), freq_reg=440, dur_reg=100
    let source = "LDI r1, 0\nLDI r2, 440\nLDI r3, 100\nNOTE r1, r2, r3\nLDI r4, 1\nHALT";
    let asm = assemble(source, 0).expect("assembly should succeed");
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() {
        vm.ram[i] = v;
    }
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    assert!(vm.halted);
    assert_eq!(vm.regs[4], 1, "VM should execute past NOTE and set r4");
    assert_eq!(
        vm.note,
        Some((0, 440, 100)),
        "NOTE should set note field to (sine, 440, 100)"
    );
}

#[test]
fn test_note_opcode_square() {
    let source = "LDI r1, 1\nLDI r2, 880\nLDI r3, 50\nNOTE r1, r2, r3\nHALT";
    let asm = assemble(source, 0).expect("assembly should succeed");
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() {
        vm.ram[i] = v;
    }
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    assert!(vm.halted);
    assert_eq!(
        vm.note,
        Some((1, 880, 50)),
        "NOTE should set note field to (square, 880, 50)"
    );
}

#[test]
fn test_note_opcode_triangle() {
    let source = "LDI r1, 2\nLDI r2, 220\nLDI r3, 200\nNOTE r1, r2, r3\nHALT";
    let asm = assemble(source, 0).expect("assembly should succeed");
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() {
        vm.ram[i] = v;
    }
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    assert!(vm.halted);
    assert_eq!(
        vm.note,
        Some((2, 220, 200)),
        "NOTE should set note field to (triangle, 220, 200)"
    );
}

#[test]
fn test_note_opcode_sawtooth() {
    let source = "LDI r1, 3\nLDI r2, 110\nLDI r3, 150\nNOTE r1, r2, r3\nHALT";
    let asm = assemble(source, 0).expect("assembly should succeed");
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() {
        vm.ram[i] = v;
    }
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    assert!(vm.halted);
    assert_eq!(
        vm.note,
        Some((3, 110, 150)),
        "NOTE should set note field to (sawtooth, 110, 150)"
    );
}

#[test]
fn test_note_opcode_noise() {
    let source = "LDI r1, 4\nLDI r2, 1000\nLDI r3, 75\nNOTE r1, r2, r3\nHALT";
    let asm = assemble(source, 0).expect("assembly should succeed");
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() {
        vm.ram[i] = v;
    }
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    assert!(vm.halted);
    assert_eq!(
        vm.note,
        Some((4, 1000, 75)),
        "NOTE should set note field to (noise, 1000, 75)"
    );
}

#[test]
fn test_note_clamps_frequency() {
    // Frequency below 20 should clamp to 20, above 20000 should clamp to 20000
    let source = "LDI r1, 0\nLDI r2, 5\nLDI r3, 100\nNOTE r1, r2, r3\nHALT";
    let asm = assemble(source, 0).expect("assembly should succeed");
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() {
        vm.ram[i] = v;
    }
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    assert!(vm.halted);
    assert_eq!(
        vm.note,
        Some((0, 20, 100)),
        "NOTE freq=5 should clamp to 20"
    );

    // Test upper clamp
    let source2 = "LDI r1, 0\nLDI r2, 99999\nLDI r3, 100\nNOTE r1, r2, r3\nHALT";
    let asm2 = assemble(source2, 0).expect("assembly should succeed");
    let mut vm2 = Vm::new();
    for (i, &v) in asm2.pixels.iter().enumerate() {
        vm2.ram[i] = v;
    }
    for _ in 0..100 {
        if !vm2.step() {
            break;
        }
    }
    assert!(vm2.halted);
    assert_eq!(
        vm2.note,
        Some((0, 20000, 100)),
        "NOTE freq=99999 should clamp to 20000"
    );
}

#[test]
fn test_note_clamps_duration() {
    // Duration below 1 should clamp to 1, above 5000 should clamp to 5000
    let source = "LDI r1, 0\nLDI r2, 440\nLDI r3, 0\nNOTE r1, r2, r3\nHALT";
    let asm = assemble(source, 0).expect("assembly should succeed");
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() {
        vm.ram[i] = v;
    }
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    assert!(vm.halted);
    assert_eq!(vm.note, Some((0, 440, 1)), "NOTE dur=0 should clamp to 1");
}

#[test]
fn test_note_clamps_waveform() {
    // Waveform > 4 should clamp to 4 (noise)
    let source = "LDI r1, 99\nLDI r2, 440\nLDI r3, 100\nNOTE r1, r2, r3\nHALT";
    let asm = assemble(source, 0).expect("assembly should succeed");
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() {
        vm.ram[i] = v;
    }
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    assert!(vm.halted);
    assert_eq!(
        vm.note,
        Some((4, 440, 100)),
        "NOTE wave=99 should clamp to 4 (noise)"
    );
}

#[test]
fn test_note_assembles() {
    let source = "NOTE r1, r2, r3";
    let asm = assemble(source, 0).expect("assembly should succeed");
    assert_eq!(asm.pixels[0], 0x7E, "NOTE should assemble to 0x7E");
    assert_eq!(asm.pixels[1], 1, "wave register should be r1");
    assert_eq!(asm.pixels[2], 2, "freq register should be r2");
    assert_eq!(asm.pixels[3], 3, "dur register should be r3");
}

#[test]
fn test_note_disassembles() {
    let mut vm = Vm::new();
    vm.ram[0] = 0x7E; // NOTE
    vm.ram[1] = 1; // r1
    vm.ram[2] = 2; // r2
    vm.ram[3] = 3; // r3
    let (mnemonic, len) = vm.disassemble_at(0);
    assert_eq!(mnemonic, "NOTE r1, r2, r3");
    assert_eq!(len, 4);
}

#[test]
fn test_beep_still_works_after_note() {
    // BEEP opcode should still work -- backward compatibility
    let source = "LDI r1, 440\nLDI r2, 50\nBEEP r1, r2\nNOTE r0, r1, r2\nHALT";
    let asm = assemble(source, 0).expect("assembly should succeed");
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() {
        vm.ram[i] = v;
    }
    // Set up wave=0 (sine) for NOTE
    vm.regs[0] = 0;
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    assert!(vm.halted);
    // BEEP was executed first, then NOTE overwrites with note field
    // beep should have been set then consumed or still set
    // The NOTE should set the note field
    assert!(vm.note.is_some(), "NOTE should set the note field");
}

// ── SOUND DEMO PROGRAMS (Phase 39b) ──────────────────────────────

#[test]
fn test_sfx_demo_assembles_and_runs() {
    // sfx_demo.asm should assemble and run through all 10 SFX effects
    let source =
        std::fs::read_to_string("programs/sfx_demo.asm").expect("sfx_demo.asm should exist");
    let asm = assemble(&source, 0).expect("sfx_demo.asm should assemble");
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = v;
        }
    }
    // Run until halted (should play 10 notes then halt)
    for _ in 0..100_000 {
        if !vm.step() {
            break;
        }
    }
    assert!(vm.halted, "sfx_demo should halt after playing all effects");
    // The last SFX is triangle bass at (2, 55, 600)
    assert_eq!(
        vm.note,
        Some((2, 55, 600)),
        "last SFX should be triangle bass: waveform=2, freq=55, dur=600"
    );
    // Should have rendered exactly 10 frames (one per SFX)
    assert!(
        vm.frame_count >= 10,
        "should have at least 10 frames, got {}",
        vm.frame_count
    );
}

#[test]
fn test_music_demo_assembles_and_runs() {
    // music_demo.asm should assemble and play Mary Had a Little Lamb
    let source =
        std::fs::read_to_string("programs/music_demo.asm").expect("music_demo.asm should exist");
    let asm = assemble(&source, 0).expect("music_demo.asm should assemble");
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = v;
        }
    }
    // Run until halted (should play 26 notes then halt)
    for _ in 0..500_000 {
        if !vm.step() {
            break;
        }
    }
    assert!(vm.halted, "music_demo should halt after melody");
    // The last note is C4 (262 Hz) half duration (800 ms), square wave (1)
    assert_eq!(
        vm.note,
        Some((1, 262, 800)),
        "last note should be square C4 half: waveform=1, freq=262, dur=800"
    );
    // Should have rendered at least 26 frames (one per note) + 1 final
    assert!(
        vm.frame_count >= 26,
        "should have at least 26 frames, got {}",
        vm.frame_count
    );
}

#[test]
fn test_sar_opcode() {
    // SAR rd, rs
    // Test negative: -4 (0xFFFFFFFC) >> 1 = -2 (0xFFFFFFFE)
    let source = "LDI r1, 0xFFFFFFFC\nLDI r2, 1\nSAR r1, r2\nHALT";
    let asm = assemble(source, 0).expect("assembly should succeed");
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() {
        vm.ram[i] = v;
    }
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    assert!(vm.halted);
    assert_eq!(vm.regs[1], 0xFFFFFFFE, "SAR -4, 1 should be -2");

    // Test positive: 4 >> 1 = 2
    let source = "LDI r1, 4\nLDI r2, 1\nSAR r1, r2\nHALT";
    let asm = assemble(source, 0).expect("assembly should succeed");
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() {
        vm.ram[i] = v;
    }
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
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

    let asm = assemble(source, 0).expect("assembly should succeed");
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() {
        vm.ram[i] = v;
    }
    for _ in 0..1000 {
        if !vm.step() {
            break;
        }
    }
    assert!(vm.halted);

    // Check pixels at (10,10) to (13,13)
    // Grid 2x2 * Tile 2x2 = 4x4 area
    for y in 10..14 {
        for x in 10..14 {
            assert_eq!(
                vm.screen[y * 256 + x],
                0xFF0000,
                "pixel at ({}, {}) should be red",
                x,
                y
            );
        }
    }
}

// ── CMP / BLT / BGE ────────────────────────────────────────────

#[test]
fn test_cmp_opcode_equal() {
    let source = "LDI r1, 42\nLDI r2, 42\nCMP r1, r2\nHALT";
    let asm = assemble(source, 0).expect("assembly should succeed");
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() {
        vm.ram[i] = v;
    }
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    assert!(vm.halted);
    assert_eq!(vm.regs[0], 0, "CMP equal should set r0 = 0");
}

#[test]
fn test_cmp_opcode_less_than() {
    let source = "LDI r1, 10\nLDI r2, 20\nCMP r1, r2\nHALT";
    let asm = assemble(source, 0).expect("assembly should succeed");
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() {
        vm.ram[i] = v;
    }
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    assert!(vm.halted);
    assert_eq!(vm.regs[0], 0xFFFFFFFF, "CMP less-than should set r0 = -1");
}

#[test]
fn test_cmp_opcode_greater_than() {
    let source = "LDI r1, 30\nLDI r2, 20\nCMP r1, r2\nHALT";
    let asm = assemble(source, 0).expect("assembly should succeed");
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() {
        vm.ram[i] = v;
    }
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    assert!(vm.halted);
    assert_eq!(vm.regs[0], 1, "CMP greater-than should set r0 = 1");
}

#[test]
fn test_blt_opcode() {
    let source = "\
LDI r1, 10\nLDI r2, 20\nCMP r1, r2\nBLT r0, less\nLDI r3, 99\nHALT\n\
less:\nLDI r3, 42\nHALT";
    let asm = assemble(source, 0).expect("assembly should succeed");
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() {
        vm.ram[i] = v;
    }
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    assert!(vm.halted);
    assert_eq!(vm.regs[3], 42, "BLT should branch when r1 < r2");
}

#[test]
fn test_bge_opcode() {
    let source = "\
LDI r1, 20\nLDI r2, 10\nCMP r1, r2\nBGE r0, geq\nLDI r3, 99\nHALT\n\
geq:\nLDI r3, 42\nHALT";
    let asm = assemble(source, 0).expect("assembly should succeed");
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() {
        vm.ram[i] = v;
    }
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    assert!(vm.halted);
    assert_eq!(vm.regs[3], 42, "BGE should branch when r1 >= r2");
}

// ── MOD ─────────────────────────────────────────────────────────

#[test]
fn test_mod_opcode() {
    let source = "LDI r1, 17\nLDI r2, 5\nMOD r1, r2\nHALT";
    let asm = assemble(source, 0).expect("assembly should succeed");
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() {
        vm.ram[i] = v;
    }
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    assert!(vm.halted);
    assert_eq!(vm.regs[1], 2, "17 MOD 5 should be 2");
}

#[test]
fn test_mod_opcode_zero_divisor() {
    let source = "LDI r1, 10\nLDI r2, 0\nMOD r1, r2\nHALT";
    let asm = assemble(source, 0).expect("assembly should succeed");
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() {
        vm.ram[i] = v;
    }
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    assert!(vm.halted);
    // Division by zero leaves register unchanged (same behavior as DIV)
    assert_eq!(
        vm.regs[1], 10,
        "MOD by zero should leave register unchanged"
    );
}

#[test]
fn test_screenp_reads_screen_pixel() {
    // SCREENP dest, x, y -- reads screen[y*256+x] into dest
    let mut vm = Vm::new();
    // Draw a pixel at (10, 20) with color 42
    vm.screen[20 * 256 + 10] = 42;

    // SCREENP r0, r1, r2 (dest=r0, x=r1, y=r2)
    vm.ram[0] = 0x6D; // SCREENP
    vm.ram[1] = 0; // dest = r0
    vm.ram[2] = 1; // x = r1
    vm.ram[3] = 2; // y = r2
    vm.regs[1] = 10; // x = 10
    vm.regs[2] = 20; // y = 20
    vm.pc = 0;

    vm.step();
    assert_eq!(
        vm.regs[0], 42,
        "SCREENP should read screen pixel at (10,20)"
    );
}

#[test]
fn test_screenp_out_of_bounds_returns_zero() {
    let mut vm = Vm::new();
    vm.screen[0] = 99; // set pixel at (0,0)

    // SCREENP r0, r1, r2 with x=300 (out of bounds)
    vm.ram[0] = 0x6D;
    vm.ram[1] = 0;
    vm.ram[2] = 1;
    vm.ram[3] = 2;
    vm.regs[1] = 300; // x = 300 (out of bounds)
    vm.regs[2] = 0;
    vm.pc = 0;

    vm.step();
    assert_eq!(vm.regs[0], 0, "SCREENP out of bounds should return 0");
}

#[test]
fn test_screenp_assembles() {
    let source = "SCREENP r0, r1, r2\nHALT";
    let asm = assemble(source, 0).expect("assembly should succeed");
    assert_eq!(asm.pixels[0], 0x6D, "SCREENP should assemble to 0x6D");
    assert_eq!(asm.pixels[1], 0, "dest register should be r0");
    assert_eq!(asm.pixels[2], 1, "x register should be r1");
    assert_eq!(asm.pixels[3], 2, "y register should be r2");
}

#[test]
fn test_screenp_disassembles() {
    let mut vm = Vm::new();
    vm.ram[0] = 0x6D; // SCREENP
    vm.ram[1] = 5; // r5
    vm.ram[2] = 3; // r3
    vm.ram[3] = 7; // r7
    let (mnemonic, len) = vm.disassemble_at(0);
    assert_eq!(mnemonic, "SCREENP r5, r3, r7");
    assert_eq!(len, 4);
}

// ============================================================
// Tests for new immediate-form opcodes (TEXTI, STRO, CMPI, etc.)
// ============================================================

#[test]
fn test_texti_renders_inline_string() {
    let src = "TEXTI 10, 20, \"Hi!\"\nHALT";
    let asm = assemble(src, 0).expect("assembly should succeed");
    let mut vm = Vm::new();
    for (i, &w) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = w;
        }
    }
    vm.pc = 0;
    vm.halted = false;
    for _ in 0..1000 {
        if !vm.step() {
            break;
        }
    }
    // "H" should be drawn at (10, 20) in white (0xFFFFFF)
    // Check that at least some pixels near (10,20) are non-black
    let mut found_white = false;
    for dy in 0..8 {
        for dx in 0..5 {
            if vm.screen[(20 + dy) * 256 + (10 + dx)] != 0 {
                found_white = true;
            }
        }
    }
    assert!(found_white, "TEXTI should render 'H' at (10,20)");
    assert!(vm.halted);
}

#[test]
fn test_stro_stores_string_to_ram() {
    let src = "LDI r9, 0x2000\nSTRO r9, \"ABC\"\nHALT";
    let asm = assemble(src, 0).expect("assembly should succeed");
    let mut vm = Vm::new();
    for (i, &w) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = w;
        }
    }
    vm.pc = 0;
    vm.halted = false;
    for _ in 0..1000 {
        if !vm.step() {
            break;
        }
    }
    assert_eq!(vm.ram[0x2000], 65); // 'A'
    assert_eq!(vm.ram[0x2001], 66); // 'B'
    assert_eq!(vm.ram[0x2002], 67); // 'C'
    assert_eq!(vm.ram[0x2003], 0); // null terminator
}

#[test]
fn test_cmpi_less() {
    let src = "LDI r5, 10\nCMPI r5, 20\nHALT";
    let asm = assemble(src, 0).expect("assembly should succeed");
    let mut vm = Vm::new();
    for (i, &w) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = w;
        }
    }
    vm.pc = 0;
    vm.halted = false;
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    assert_eq!(vm.regs[0], 0xFFFFFFFF, "10 < 20 should set r0 to -1");
}

#[test]
fn test_cmpi_equal() {
    let src = "LDI r5, 42\nCMPI r5, 42\nHALT";
    let asm = assemble(src, 0).expect("assembly should succeed");
    let mut vm = Vm::new();
    for (i, &w) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = w;
        }
    }
    vm.pc = 0;
    vm.halted = false;
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    assert_eq!(vm.regs[0], 0, "42 == 42 should set r0 to 0");
}

#[test]
fn test_cmpi_greater() {
    let src = "LDI r5, 100\nCMPI r5, 50\nHALT";
    let asm = assemble(src, 0).expect("assembly should succeed");
    let mut vm = Vm::new();
    for (i, &w) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = w;
        }
    }
    vm.pc = 0;
    vm.halted = false;
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    assert_eq!(vm.regs[0], 1, "100 > 50 should set r0 to 1");
}

#[test]
fn test_cmpi_with_blt() {
    let src = "LDI r5, 5\nloop:\n  ADDI r5, 1\n  CMPI r5, 10\n  BLT r0, loop\nHALT";
    let asm = assemble(src, 0).expect("assembly should succeed");
    let mut vm = Vm::new();
    for (i, &w) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = w;
        }
    }
    vm.pc = 0;
    vm.halted = false;
    for _ in 0..1000 {
        if !vm.step() {
            break;
        }
    }
    assert_eq!(vm.regs[5], 10, "loop should stop when r5 reaches 10");
    assert!(vm.halted);
}

#[test]
fn test_addi() {
    let src = "LDI r1, 10\nADDI r1, 5\nHALT";
    let asm = assemble(src, 0).expect("assembly should succeed");
    let mut vm = Vm::new();
    for (i, &w) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = w;
        }
    }
    vm.pc = 0;
    vm.halted = false;
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    assert_eq!(vm.regs[1], 15, "10 + 5 = 15");
}

#[test]
fn test_subi() {
    let src = "LDI r1, 100\nSUBI r1, 30\nHALT";
    let asm = assemble(src, 0).expect("assembly should succeed");
    let mut vm = Vm::new();
    for (i, &w) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = w;
        }
    }
    vm.pc = 0;
    vm.halted = false;
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    assert_eq!(vm.regs[1], 70, "100 - 30 = 70");
}

#[test]
fn test_shli() {
    let src = "LDI r1, 3\nSHLI r1, 4\nHALT";
    let asm = assemble(src, 0).expect("assembly should succeed");
    let mut vm = Vm::new();
    for (i, &w) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = w;
        }
    }
    vm.pc = 0;
    vm.halted = false;
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    assert_eq!(vm.regs[1], 48, "3 << 4 = 48");
}

#[test]
fn test_shri() {
    let src = "LDI r1, 0xFF\nSHRI r1, 4\nHALT";
    let asm = assemble(src, 0).expect("assembly should succeed");
    let mut vm = Vm::new();
    for (i, &w) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = w;
        }
    }
    vm.pc = 0;
    vm.halted = false;
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    assert_eq!(vm.regs[1], 0x0F, "0xFF >> 4 = 0x0F");
}

#[test]
fn test_andi() {
    let src = "LDI r1, 0xAB\nANDI r1, 0x0F\nHALT";
    let asm = assemble(src, 0).expect("assembly should succeed");
    let mut vm = Vm::new();
    for (i, &w) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = w;
        }
    }
    vm.pc = 0;
    vm.halted = false;
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    assert_eq!(vm.regs[1], 0x0B, "0xAB & 0x0F = 0x0B");
}

#[test]
fn test_ori() {
    let src = "LDI r1, 0xF0\nORI r1, 0x0F\nHALT";
    let asm = assemble(src, 0).expect("assembly should succeed");
    let mut vm = Vm::new();
    for (i, &w) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = w;
        }
    }
    vm.pc = 0;
    vm.halted = false;
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    assert_eq!(vm.regs[1], 0xFF, "0xF0 | 0x0F = 0xFF");
}

#[test]
fn test_xori() {
    let src = "LDI r1, 0xFF\nXORI r1, 0x0F\nHALT";
    let asm = assemble(src, 0).expect("assembly should succeed");
    let mut vm = Vm::new();
    for (i, &w) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = w;
        }
    }
    vm.pc = 0;
    vm.halted = false;
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    assert_eq!(vm.regs[1], 0xF0, "0xFF ^ 0x0F = 0xF0");
}

#[test]
fn test_loads_stores() {
    // Store a value at SP+0, then load it back into another register
    let src = "LDI r30, 0xFF00\nLDI r1, 42\nSTORES 0, r1\nLOADS r2, 0\nHALT";
    let asm = assemble(src, 0).expect("assembly should succeed");
    let mut vm = Vm::new();
    for (i, &w) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = w;
        }
    }
    vm.pc = 0;
    vm.halted = false;
    for _ in 0..1000 {
        if !vm.step() {
            break;
        }
    }
    assert_eq!(vm.regs[2], 42, "LOADS should read back what STORES wrote");
}
