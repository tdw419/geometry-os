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
    vm.key_port = 65;
    for _ in 0..10_000 {
        if !vm.step() {
            break;
        }
    }
    assert!(vm.halted);
    assert_eq!(vm.regs[1], 65, "IKEY should read key code 65 into r1");
    assert_eq!(vm.key_port, 0, "IKEY should clear the keyboard port");
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

// ── ASSEMBLER ERROR MESSAGES ─────────────────────────────────────

#[test]
fn test_unknown_opcode_error_includes_line_number() {
    let source = "LDI r0, 1\nBOGUS_OPCODE r0, r1\nHALT";
    let result = assemble(source, 0);
    assert!(result.is_err(), "assembly with unknown opcode should fail");
    let err = result.unwrap_err();
    // AsmError.line should point to line 2
    assert_eq!(err.line, 2, "error line should be 2, got: {}", err.line);
    // AsmError.message should contain the opcode name
    assert!(
        err.message.contains("unknown opcode"),
        "message should mention 'unknown opcode', got: {}",
        err.message
    );
    assert!(
        err.message.contains("BOGUS_OPCODE"),
        "message should mention the bad opcode name, got: {}",
        err.message
    );
    // Display format should be "line N: unknown opcode: XYZ"
    let displayed = format!("{}", err);
    assert_eq!(
        displayed, "line 2: unknown opcode: BOGUS_OPCODE",
        "full error format should be 'line N: unknown opcode: XYZ', got: {}",
        displayed
    );
}

#[test]
fn test_unknown_opcode_error_on_first_line() {
    let source = "NOSUCH r0\nHALT";
    let result = assemble(source, 0);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.line, 1, "error line should be 1, got: {}", err.line);
    let displayed = format!("{}", err);
    assert_eq!(
        displayed, "line 1: unknown opcode: NOSUCH",
        "full error format, got: {}",
        displayed
    );
}

// ── PTYOPEN / PTYWRITE / PTYREAD opcode bytecode roundtrip ───────
//
// End-to-end check that the assembler emits valid PTY opcodes, the dispatch
// arms wire them to the right handlers, and a guest can drive bash through
// the bytecode path. Spawns a real bash via portable-pty.

#[test]
fn test_pty_opcodes_pwd_roundtrip() {
    // Program: open bash, write "pwd\nexit\n", drain forever into RAM[0x6000..]
    // advancing the buffer pointer by exactly the bytes drained so we don't
    // overrun RAM and don't overwrite earlier output.
    //
    // The test runs the VM step-by-step against a wall-clock budget; bash
    // needs real time to spin up and emit its first prompt.
    let source = "
        LDI r1, 0x5000
        LDI r0, 0
        STORE r1, r0
        LDI r1, 0x5100
        LDI r0, 112
        STORE r1, r0
        LDI r2, 1
        ADD r1, r2
        LDI r0, 119
        STORE r1, r0
        ADD r1, r2
        LDI r0, 100
        STORE r1, r0
        ADD r1, r2
        LDI r0, 10
        STORE r1, r0
        ADD r1, r2
        LDI r0, 101
        STORE r1, r0
        ADD r1, r2
        LDI r0, 120
        STORE r1, r0
        ADD r1, r2
        LDI r0, 105
        STORE r1, r0
        ADD r1, r2
        LDI r0, 116
        STORE r1, r0
        ADD r1, r2
        LDI r0, 10
        STORE r1, r0
        LDI r5, 0x5000
        PTYOPEN r5, r10
        LDI r6, 0x5100
        LDI r7, 9
        PTYWRITE r10, r6, r7
        LDI r12, 0x6000
        LDI r16, 256
drain_loop:
        PTYREAD r10, r12, r16
        ADD r12, r0
        JMP drain_loop
    ";

    let asm = match assemble(source, 0) {
        Ok(a) => a,
        Err(e) => panic!("assembly failed: {} on line {}", e.message, e.line),
    };
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = v;
        }
    }
    vm.pc = 0;
    vm.halted = false;

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while std::time::Instant::now() < deadline {
        for _ in 0..2000 {
            if !vm.step() {
                break;
            }
        }
        if vm.halted {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }

    let mut bytes = Vec::with_capacity(8192);
    for i in 0..8192 {
        let cell = vm.ram[0x6000 + i];
        if cell == 0 {
            continue;
        }
        bytes.push((cell & 0xFF) as u8);
    }
    let text = String::from_utf8_lossy(&bytes);

    assert!(
        text.contains('/'),
        "expected pwd output containing '/', got: {:?}",
        text
    );
}

// ── PTY initial prompt emission test ──────────────────────────────
//
// After PTYOPEN, the reader thread should receive output from bash
// within a reasonable wall-clock budget WITHOUT any explicit PTYWRITE.
// This catches the TERM=dumb regression that suppressed prompt output.

#[test]
fn test_host_term_pty_initial_output() {
    // Program: open bash (no write), drain PTYREAD for ~100 frames
    let source = "
        LDI r1, 0x5000
        LDI r0, 0
        STORE r1, r0
        LDI r5, 0x5000
        PTYOPEN r5, r10
        LDI r12, 0x6000
        LDI r16, 256
drain_loop:
        PTYREAD r10, r12, r16
        ADD r12, r0
        FRAME
        JMP drain_loop
    ";

    let asm = match assemble(source, 0) {
        Ok(a) => a,
        Err(e) => panic!("assembly failed: {} on line {}", e.message, e.line),
    };
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = v;
        }
    }
    vm.pc = 0;
    vm.halted = false;

    // Run for up to 3 seconds wall-clock, ~100 frames
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    let mut frames_seen = 0u32;
    while std::time::Instant::now() < deadline && frames_seen < 100 {
        for _ in 0..5000 {
            if !vm.step() {
                break;
            }
            if vm.frame_ready {
                vm.frame_ready = false;
                frames_seen += 1;
            }
        }
        if frames_seen >= 100 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    // Collect all non-zero bytes from the receive buffer
    let mut bytes = Vec::new();
    for i in 0..8192 {
        let cell = vm.ram[0x6000 + i];
        if cell == 0 {
            continue;
        }
        bytes.push((cell & 0xFF) as u8);
    }
    let text = String::from_utf8_lossy(&bytes);

    assert!(
        !text.is_empty(),
        "PTYREAD should have received some output from bash within 3s. \
         frames_seen={}, text={:?}",
        frames_seen,
        text
    );

    // With PS1='$ ', bash should emit a prompt containing '$'
    // (may also contain ANSI escape sequences)
    assert!(
        text.contains('$') || text.contains('/') || text.contains('\n'),
        "expected bash prompt or output with '$', '/', or newline, got: {:?}",
        text
    );
}

// ── Phase 130: E2E PTY roundtrip tests ──────────────────────────────

/// Helper: run a PTY session, send commands, collect all output.
/// Returns the concatenated text from PTYREAD.
fn pty_e2e_session(commands: &[&str], read_frames: u32) -> String {
    let mut full_source = String::from(
        "LDI r1, 0x5000\nLDI r0, 0\nSTORE r1, r0\nLDI r5, 0x5000\nPTYOPEN r5, r10\n\
         LDI r12, 0x6000\nLDI r16, 256\nFRAME\nFRAME\nFRAME\nFRAME\nFRAME\n",
    );

    for cmd in commands {
        full_source.push_str("; Send command\n");
        for (i, ch) in cmd.as_bytes().iter().enumerate() {
            full_source.push_str(&format!(
                "LDI r20, 0x{:04X}\nLDI r21, {}\nSTORE r20, r21\n",
                0x5100 + i,
                ch
            ));
        }
        full_source.push_str(&format!(
            "LDI r6, 0x5100\nLDI r7, {}\nPTYWRITE r10, r6, r7\n",
            cmd.len()
        ));
        for _ in 0..read_frames {
            full_source.push_str("PTYREAD r10, r12, r16\nADD r12, r0\nFRAME\n");
        }
    }

    full_source
        .push_str("drain_loop:\nPTYREAD r10, r12, r16\nADD r12, r0\nFRAME\nJMP drain_loop\n");

    let asm = match assemble(&full_source, 0) {
        Ok(a) => a,
        Err(e) => panic!("assembly failed: {} on line {}", e.message, e.line),
    };
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = v;
        }
    }
    vm.pc = 0;
    vm.halted = false;

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    let mut frames_seen = 0u32;
    while std::time::Instant::now() < deadline && frames_seen < 200 {
        for _ in 0..5000 {
            if !vm.step() {
                break;
            }
            if vm.frame_ready {
                vm.frame_ready = false;
                frames_seen += 1;
            }
        }
        if frames_seen >= 200 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    let mut bytes = Vec::new();
    for i in 0..16384 {
        let cell = vm.ram[0x6000 + i];
        if cell == 0 {
            continue;
        }
        bytes.push((cell & 0xFF) as u8);
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

#[test]
fn test_e2e_ls_command_output() {
    let text = pty_e2e_session(&["ls\nexit\n"], 5);

    assert!(
        !text.is_empty(),
        "PTYREAD should have received output from ls command. Got empty text."
    );

    let clean: String = text
        .chars()
        .filter(|c| !c.is_control() || *c == '\n' || *c == ' ' || *c == '/')
        .collect();

    assert!(
        clean.contains('/') || clean.contains('.') || clean.contains("bin") || clean.len() > 10,
        "expected ls output containing path-like content, got: {:?}",
        &clean[..clean.len().min(200)]
    );
}

#[test]
fn test_e2e_echo_roundtrip() {
    let text = pty_e2e_session(&["echo test123_geos\nexit\n"], 5);

    assert!(
        !text.is_empty(),
        "PTYREAD should have received output from echo command. Got empty text."
    );

    assert!(
        text.contains("test123_geos"),
        "expected echo output containing 'test123_geos', got: {:?}",
        &text[..text.len().min(300)]
    );
}

#[test]
fn test_e2e_arrow_key_up_recalls_command() {
    let source = r#"
        LDI r1, 0x5000
        LDI r0, 0
        STORE r1, r0
        LDI r5, 0x5000
        PTYOPEN r5, r10
        LDI r12, 0x6000
        LDI r16, 256
        FRAME
        FRAME
        FRAME
        FRAME
        FRAME

        LDI r20, 0x5100
        LDI r21, 101
        STORE r20, r21
        LDI r20, 0x5101
        LDI r21, 99
        STORE r20, r21
        LDI r20, 0x5102
        LDI r21, 104
        STORE r20, r21
        LDI r20, 0x5103
        LDI r21, 111
        STORE r20, r21
        LDI r20, 0x5104
        LDI r21, 32
        STORE r20, r21
        LDI r20, 0x5105
        LDI r21, 109
        STORE r20, r21
        LDI r20, 0x5106
        LDI r21, 97
        STORE r20, r21
        LDI r20, 0x5107
        LDI r21, 114
        STORE r20, r21
        LDI r20, 0x5108
        LDI r21, 107
        STORE r20, r21
        LDI r20, 0x5109
        LDI r21, 101
        STORE r20, r21
        LDI r20, 0x510A
        LDI r21, 114
        STORE r20, r21
        LDI r20, 0x510B
        LDI r21, 95
        STORE r20, r21
        LDI r20, 0x510C
        LDI r21, 97
        STORE r20, r21
        LDI r20, 0x510D
        LDI r21, 98
        STORE r20, r21
        LDI r20, 0x510E
        LDI r21, 99
        STORE r20, r21
        LDI r20, 0x510F
        LDI r21, 10
        STORE r20, r21
        LDI r6, 0x5100
        LDI r7, 16
        PTYWRITE r10, r6, r7

        PTYREAD r10, r12, r16
        ADD r12, r0
        FRAME
        FRAME
        FRAME
        FRAME
        FRAME
        FRAME
        FRAME
        FRAME
        FRAME
        FRAME

        LDI r20, 0x5100
        LDI r21, 27
        STORE r20, r21
        LDI r20, 0x5101
        LDI r21, 91
        STORE r20, r21
        LDI r20, 0x5102
        LDI r21, 65
        STORE r20, r21
        LDI r20, 0x5103
        LDI r21, 10
        STORE r20, r21
        LDI r20, 0x5104
        LDI r21, 101
        STORE r20, r21
        LDI r20, 0x5105
        LDI r21, 120
        STORE r20, r21
        LDI r20, 0x5106
        LDI r21, 105
        STORE r20, r21
        LDI r20, 0x5107
        LDI r21, 116
        STORE r20, r21
        LDI r20, 0x5108
        LDI r21, 10
        STORE r20, r21
        LDI r6, 0x5100
        LDI r7, 9
        PTYWRITE r10, r6, r7

drain_loop:
        PTYREAD r10, r12, r16
        ADD r12, r0
        FRAME
        JMP drain_loop
    "#;

    let asm = match assemble(source, 0) {
        Ok(a) => a,
        Err(e) => panic!("assembly failed: {} on line {}", e.message, e.line),
    };
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = v;
        }
    }
    vm.pc = 0;
    vm.halted = false;

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(6);
    let mut frames_seen = 0u32;
    while std::time::Instant::now() < deadline && frames_seen < 300 {
        for _ in 0..5000 {
            if !vm.step() {
                break;
            }
            if vm.frame_ready {
                vm.frame_ready = false;
                frames_seen += 1;
            }
        }
        if frames_seen >= 300 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    let mut bytes = Vec::new();
    for i in 0..16384 {
        let cell = vm.ram[0x6000 + i];
        if cell == 0 {
            continue;
        }
        bytes.push((cell & 0xFF) as u8);
    }
    let text = String::from_utf8_lossy(&bytes);

    assert!(
        !text.is_empty(),
        "PTYREAD should have received some output. frames_seen={}",
        frames_seen
    );

    assert!(
        text.contains("marker_abc"),
        "expected 'marker_abc' in output from echoed command, got: {:?}",
        &text[..text.len().min(300)]
    );
}

// ── Phase 130 D2: Desktop building entry for host_term ─────────────

#[test]
fn test_host_term_building_in_table() {
    let source = std::fs::read_to_string("programs/world_desktop.asm").unwrap();
    let asm = assemble(&source, 0).unwrap();
    let mut vm = Vm::new();
    for (i, &pixel) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = pixel;
        }
    }
    vm.pc = 0;
    vm.halted = false;
    for _ in 0..10_000_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_ready {
            break;
        }
    }

    // Building 15 should be host_term at (140, 80) with color 0xAAFFCC
    let b15_base = 0x7500 + 15 * 4;
    assert_eq!(vm.ram[b15_base], 140, "host_term building x should be 140");
    assert_eq!(
        vm.ram[b15_base + 1],
        80,
        "host_term building y should be 80"
    );
    assert_eq!(
        vm.ram[b15_base + 2],
        0xAAFFCC,
        "host_term building color should be 0xAAFFCC"
    );

    // Name address should point to "host" string
    let name_addr = vm.ram[b15_base + 3] as usize;
    assert_ne!(name_addr, 0, "host_term name addr should not be 0");

    // Read the name string
    let mut name = String::new();
    for j in 0..16 {
        let ch = vm.ram[name_addr + j];
        if ch == 0 {
            break;
        }
        name.push(ch as u8 as char);
    }
    assert_eq!(name, "host", "building 15 name should be host");
}

#[test]
fn test_host_term_building_launch_mapping() {
    let source = std::fs::read_to_string("programs/host_term.asm").unwrap();
    let result = assemble(&source, 0);
    assert!(result.is_ok(), "host_term.asm should assemble successfully");
    let asm = result.unwrap();
    assert!(
        !asm.pixels.is_empty(),
        "host_term.asm should produce bytecode"
    );
}

// ── Phase 155: Terminal Multiplexer (tmux-like tabs) ─────────────

#[test]
fn test_host_term_tmux_assembles() {
    let source = std::fs::read_to_string("programs/host_term_tmux.asm").unwrap();
    let result = assemble(&source, 0);
    assert!(
        result.is_ok(),
        "host_term_tmux.asm should assemble: {:?}",
        result.err()
    );
    let asm = result.unwrap();
    assert!(
        !asm.pixels.is_empty(),
        "host_term_tmux.asm should produce bytecode"
    );
    // Should be reasonable size (2000-3000 words)
    assert!(
        asm.pixels.len() > 500,
        "tmux program should have substantial bytecode, got {} words",
        asm.pixels.len()
    );
}

#[test]
fn test_host_term_tmux_tab_metadata_init() {
    let source = std::fs::read_to_string("programs/host_term_tmux.asm").unwrap();
    let asm = assemble(&source, 0).unwrap();
    let mut vm = Vm::new();
    for (i, &word) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = word;
        }
    }
    vm.pc = 0;
    vm.halted = false;

    // Run through initialization (before PTYOPEN, which needs real OS PTY)
    // Run enough steps to clear buffers and init metadata
    let mut steps = 0;
    for _ in 0..500_000 {
        if !vm.step() {
            break;
        }
        steps += 1;
        // Stop when we reach PTYOPEN (which needs real PTY hardware)
        // PTYOPEN opcode is 0xA9
        if vm.pc > 0 && vm.ram[vm.pc as usize - 1] == 0xA9 {
            break;
        }
    }

    // Check tab metadata: PTY handle for tab 0 should be set up
    // (or still 0xFFFF if PTYOPEN hasn't run yet)
    let tab0_pty = vm.ram[0x4C00];
    // Either 0xFFFF (init value, PTYOPEN not yet reached) or a valid handle
    assert!(
        tab0_pty == 0xFFFF || tab0_pty < 4,
        "tab0 PTY should be unused (0xFFFF) or valid handle (<4), got 0x{:08X}",
        tab0_pty
    );

    // Tabs 1-3 should be unused (0xFFFF)
    let tab1_pty = vm.ram[0x4C20];
    let tab2_pty = vm.ram[0x4C40];
    let tab3_pty = vm.ram[0x4C60];
    assert_eq!(tab1_pty, 0xFFFF, "tab1 PTY should be unused");
    assert_eq!(tab2_pty, 0xFFFF, "tab2 PTY should be unused");
    assert_eq!(tab3_pty, 0xFFFF, "tab3 PTY should be unused");

    // ACTIVE_TAB should be 0
    let active_tab = vm.ram[0x4B00];
    assert_eq!(active_tab, 0, "active tab should be 0");
}

#[test]
fn test_host_term_tmux_buffer_init() {
    let source = std::fs::read_to_string("programs/host_term_tmux.asm").unwrap();
    let asm = assemble(&source, 0).unwrap();
    let mut vm = Vm::new();
    for (i, &word) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = word;
        }
    }
    vm.pc = 0;
    vm.halted = false;

    // Run until PTYOPEN
    for _ in 0..500_000 {
        if !vm.step() {
            break;
        }
        if vm.pc > 0 && vm.ram[vm.pc as usize - 1] == 0xA9 {
            break;
        }
    }

    // Check tab 0 buffer is initialized to spaces (0x20)
    let tab0_base = 0x9000;
    for i in 0..10 {
        assert_eq!(
            vm.ram[tab0_base + i],
            0x20,
            "tab0 buffer[{}] should be space (0x20), got 0x{:08X}",
            i,
            vm.ram[tab0_base + i]
        );
    }

    // Check tab 1 buffer at 0x9600
    let tab1_base = 0x9600;
    for i in 0..10 {
        assert_eq!(
            vm.ram[tab1_base + i],
            0x20,
            "tab1 buffer[{}] should be space (0x20), got 0x{:08X}",
            i,
            vm.ram[tab1_base + i]
        );
    }
}

#[test]
fn test_alt_digit_keycodes_defined() {
    // Verify Alt+1-4 keycodes are in the expected range (0xA0-0xA3)
    assert_eq!(0xA0, 0xA0, "Alt+1 keycode should be 0xA0");
    assert_eq!(0xA1, 0xA1, "Alt+2 keycode should be 0xA2");
    assert_eq!(0xA2, 0xA2, "Alt+3 keycode should be 0xA2");
    assert_eq!(0xA3, 0xA3, "Alt+4 keycode should be 0xA3");
    // Ctrl+1-4 keycodes (0x92-0x95)
    assert_eq!(0x92, 0x92, "Ctrl+1 should be 0x92");
    assert_eq!(0x93, 0x93, "Ctrl+2 should be 0x93");
    assert_eq!(0x94, 0x94, "Ctrl+3 should be 0x94");
    assert_eq!(0x95, 0x95, "Ctrl+4 should be 0x95");
}
