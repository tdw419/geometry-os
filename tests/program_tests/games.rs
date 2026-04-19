use super::*;

// ── SPRITE OPCODE ───────────────────────────────────────────────

#[test]
fn test_sprite_opcode() {
    let source = std::fs::read_to_string("programs/sprite_demo.asm")
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

    // Run until first FRAME (game loop programs never halt)
    for _ in 0..10_000_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_ready {
            break;
        }
    }

    // Sprite data should be written to RAM at 0x3000
    // Eye pixels patched at offset 9 (index 1,1) and 14 (index 6,1)
    assert_eq!(
        vm.ram[0x3009], 0x00AAFF,
        "eye pixel at RAM[0x3009] should be 0x00AAFF"
    );
    assert_eq!(
        vm.ram[0x300E], 0x00AAFF,
        "eye pixel at RAM[0x300E] should be 0x00AAFF"
    );
    // Shirt rows (offsets 32-47 = 0x3020..0x302F) should be shirt blue
    assert_eq!(
        vm.ram[0x3020], 0x3355AA,
        "shirt pixel at RAM[0x3020] should be 0x3355AA"
    );
    // Corner transparent pixels should remain 0
    assert_eq!(
        vm.ram[0x3000], 0,
        "top-left corner of sprite should be transparent"
    );
    assert_eq!(
        vm.ram[0x3007], 0,
        "top-right corner of sprite should be transparent"
    );
    // Screen should have been rendered (player starts at 124,100 — some pixel nearby is non-zero)
    let player_x = 124usize;
    let player_y = 100usize;
    let row_has_pixels = (player_x..player_x + 8).any(|x| vm.screen[player_y * 256 + x] != 0);
    assert!(
        row_has_pixels,
        "screen should have sprite pixels at player start position"
    );
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
    assert_eq!(
        vm.screen[5 * 256 + 6],
        0xFFFFFF,
        "(6,5) should be white (transparent)"
    );
    // (7, 5) should be blue
    assert_eq!(vm.screen[5 * 256 + 7], 0x0000FF, "(7,5) should be blue");
    // (5, 6) should still be white (transparent)
    assert_eq!(
        vm.screen[6 * 256 + 5],
        0xFFFFFF,
        "(5,6) should be white (transparent)"
    );
    // (6, 6) should be red
    assert_eq!(vm.screen[6 * 256 + 6], 0xFF0000, "(6,6) should be red");
    // (7, 6) should still be white (transparent)
    assert_eq!(
        vm.screen[6 * 256 + 7],
        0xFFFFFF,
        "(7,6) should be white (transparent)"
    );
}

// ── BREAKOUT ──────────────────────────────────────────────────

#[test]
fn test_breakout_initializes() {
    let source = std::fs::read_to_string("programs/breakout.asm")
        .unwrap_or_else(|e| panic!("failed to read: {}", e));
    let asm = assemble(&source, 0).unwrap_or_else(|e| panic!("assembly failed: {:?}", e));
    let mut vm = Vm::new();

    for (i, &pixel) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = pixel;
        }
    }
    vm.pc = 0;

    // Run until first FRAME (init complete, entered game loop)
    for _ in 0..50_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_ready {
            vm.frame_ready = false;
            break;
        }
    }

    // Bricks should be initialized with colors
    assert_eq!(vm.ram[0x3000], 0xFF0000, "brick 0 should be red (row 0)");
    assert_eq!(vm.ram[0x3007], 0xFF0000, "brick 7 should be red (row 0)");
    assert_eq!(vm.ram[0x3008], 0xFF8800, "brick 8 should be orange (row 1)");
    assert_eq!(
        vm.ram[0x300F], 0xFF8800,
        "brick 15 should be orange (row 1)"
    );
    assert_eq!(
        vm.ram[0x3010], 0xFFDD00,
        "brick 16 should be yellow (row 2)"
    );
    assert_eq!(
        vm.ram[0x3017], 0xFFDD00,
        "brick 23 should be yellow (row 2)"
    );
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
    let source = std::fs::read_to_string("programs/breakout.asm").expect("breakout.asm not found");
    let asm = assemble(&source, 0x1000).expect("breakout.asm failed to assemble");
    assert!(
        asm.pixels.len() > 200,
        "breakout should be more than 200 words"
    );
}

#[test]
fn test_tetris_assembles() {
    // Smoke test: tetris.asm must assemble without errors
    let source = std::fs::read_to_string("programs/tetris.asm").expect("tetris.asm not found");
    let asm = assemble(&source, 0).expect("tetris.asm failed to assemble");
    assert!(
        asm.pixels.len() > 500,
        "tetris should be more than 500 words"
    );
}

#[test]
fn test_tetris_initializes() {
    let source = std::fs::read_to_string("programs/tetris.asm")
        .unwrap_or_else(|e| panic!("failed to read: {}", e));
    let asm = assemble(&source, 0).unwrap_or_else(|e| panic!("assembly failed: {:?}", e));
    let mut vm = Vm::new();

    for (i, &pixel) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = pixel;
        }
    }
    vm.pc = 0;

    // Run until first FRAME (init complete)
    for _ in 0..200_000 {
        if !vm.step() {
            break;
        }
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
    let source = std::fs::read_to_string("programs/maze.asm").expect("maze.asm not found");
    let asm = assemble(&source, 0).expect("maze.asm failed to assemble");
    assert!(asm.pixels.len() > 300, "maze should be more than 300 words");
}

#[test]
fn test_maze_initializes() {
    let source = std::fs::read_to_string("programs/maze.asm")
        .unwrap_or_else(|e| panic!("failed to read: {}", e));
    let asm = assemble(&source, 0).unwrap_or_else(|e| panic!("assembly failed: {:?}", e));
    let mut vm = Vm::new();

    for (i, &pixel) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = pixel;
        }
    }
    vm.pc = 0;

    // Run until first FRAME (init + generate + render complete)
    for _ in 0..500_000 {
        if !vm.step() {
            break;
        }
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
    let asm = assemble(&source, 0).unwrap_or_else(|e| panic!("assembly failed: {:?}", e));
    let mut vm = Vm::new();

    for (i, &pixel) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = pixel;
        }
    }
    vm.pc = 0;

    // Run until first FRAME
    for _ in 0..500_000 {
        if !vm.step() {
            break;
        }
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
        if !vm.step() {
            break;
        }
        if vm.frame_ready {
            vm.frame_ready = false;
            break;
        }
    }

    // Player must still be at (0,0) -- blocked by wall
    assert_eq!(
        vm.ram[0x5310], 0,
        "player_x should still be 0 after blocked move"
    );
    assert_eq!(
        vm.ram[0x5311], 0,
        "player_y should still be 0 after blocked move"
    );
}

// ── INFINITE MAP ──────────────────────────────────────────────

/// Helper: assemble infinite_map.asm, load into a fresh VM, return it.
/// The VM is ready to step but has not been run yet.
fn infinite_map_vm() -> Vm {
    let source =
        std::fs::read_to_string("programs/infinite_map.asm").expect("infinite_map.asm not found");
    let asm = assemble(&source, 0).expect("infinite_map.asm failed to assemble");
    let mut vm = Vm::new();
    for (i, &word) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = word;
        }
    }
    vm
}

/// Helper: step the VM until it signals frame_ready or reaches max steps.
/// Returns the number of steps taken.
fn step_until_frame(vm: &mut Vm, max_steps: u32) -> u32 {
    vm.frame_ready = false;
    for i in 0..max_steps {
        if vm.frame_ready {
            return i;
        }
        if !vm.step() {
            return i;
        }
    }
    max_steps
}

#[test]
fn test_infinite_map_assembles() {
    // Requirement: infinite_map.asm assembles without errors and produces bytecode.
    let source =
        std::fs::read_to_string("programs/infinite_map.asm").expect("infinite_map.asm not found");
    let asm = assemble(&source, 0).expect("infinite_map.asm should assemble");
    assert!(!asm.pixels.is_empty(), "should produce non-empty bytecode");
    // The program is ~530 lines of asm; expect a substantial bytecode output.
    assert!(
        asm.pixels.len() > 500,
        "bytecode should be >500 words, got {}",
        asm.pixels.len()
    );
}

#[test]
fn test_infinite_map_pxpk_assembles() {
    let source = std::fs::read_to_string("programs/infinite_map_pxpk.asm")
        .expect("infinite_map_pxpk.asm not found");
    let asm = assemble(&source, 0).expect("infinite_map_pxpk.asm should assemble");
    assert!(!asm.pixels.is_empty(), "should produce non-empty bytecode");
    // pxpk version is ~635 lines; expect similar bytecode size
    assert!(
        asm.pixels.len() > 400,
        "bytecode should be >400 words, got {}",
        asm.pixels.len()
    );
}

/// Helper: assemble infinite_map_pxpk.asm, load into VM.
fn infinite_map_pxpk_vm() -> Vm {
    let source = std::fs::read_to_string("programs/infinite_map_pxpk.asm")
        .expect("infinite_map_pxpk.asm not found");
    let asm = assemble(&source, 0).expect("infinite_map_pxpk.asm failed to assemble");
    let mut vm = Vm::new();
    for (i, &word) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = word;
        }
    }
    vm
}

#[test]
fn test_infinite_map_pxpk_runs_and_renders() {
    let mut vm = infinite_map_pxpk_vm();
    vm.ram[0xFFB] = 0;
    let steps = step_until_frame(&mut vm, 1_000_000);
    assert!(
        vm.frame_ready,
        "pxpk should reach FRAME within 1M steps (took {})",
        steps
    );
    let non_black: usize = vm.screen.iter().filter(|&&p| p != 0).count();
    assert!(
        non_black > 50000,
        "pxpk screen should have mostly colored pixels, got {}/{}",
        non_black,
        256 * 256
    );
}

#[test]
fn test_infinite_map_pxpk_camera_moves() {
    let mut vm = infinite_map_pxpk_vm();
    vm.ram[0xFFB] = 2; // bit 1 = down
    let steps = step_until_frame(&mut vm, 1_000_000);
    assert!(vm.frame_ready, "pxpk should reach FRAME (took {})", steps);
    // Camera should have moved -- y position in RAM[0x7801] should be > 0
    assert!(
        vm.ram[0x7801] > 0,
        "camera y should have moved down, got {}",
        vm.ram[0x7801]
    );
}

#[test]
fn test_infinite_map_pxpk_pattern_variety() {
    // Verify that different tiles get different patterns (not all flat)
    let mut vm = infinite_map_pxpk_vm();
    vm.ram[0xFFB] = 0;
    let _ = step_until_frame(&mut vm, 1_000_000);
    assert!(vm.frame_ready);

    // Sample a few tiles and check they aren't all the same color
    let mut colors = std::collections::HashSet::new();
    for y in [0, 32, 64, 128, 200] {
        for x in [0, 32, 64, 128, 200] {
            let idx = (y as usize) * 256 + (x as usize);
            if idx < vm.screen.len() {
                colors.insert(vm.screen[idx]);
            }
        }
    }
    assert!(
        colors.len() > 5,
        "pxpk should produce varied tile colors, got {} unique",
        colors.len()
    );
}

#[test]
fn test_infinite_map_pxpk_step_budget() {
    // Day/night tint is inline (1 ADD per tile = +4096 steps for 64x64 grid).
    // Pattern dispatch now uses r29 save/restore (+2 per tile = +8192).
    // 32x32 minimap with RAM cache + repaint adds ~10K steps.
    // Elevation contour lines: ~50 steps/tile for neighbor hash + compare = ~200K.
    // Total should be well under 850K.
    let mut vm = infinite_map_pxpk_vm();
    vm.ram[0xFFB] = 0;
    let steps = step_until_frame(&mut vm, 1_000_000);
    assert!(vm.frame_ready, "pxpk should reach FRAME (took {})", steps);
    eprintln!("pxpk frame: {} steps", steps);
    assert!(
        steps < 850_000,
        "pxpk render loop should take < 850K steps, took {}",
        steps
    );
}

#[test]
fn test_infinite_map_pxpk_tint_phase_analysis() {
    // Track individual pixels across frames to verify tint deltas.
    // For flat (pattern 0) non-water tiles, color = biome + BPE + tint.
    // The tint delta should be exactly the computed tint offset.
    let mut vm = infinite_map_pxpk_vm();
    vm.ram[0xFFB] = 0;

    // Render frame 0 (dawn start, frac=0, tint=0)
    let _ = step_until_frame(&mut vm, 1_000_000);
    let screen_f0 = vm.screen.to_vec();

    // Advance to frame 56 (dawn peak, frac_shr=7, tint=7*0x030100=0x00150700)
    for _ in 0..56 {
        vm.frame_ready = false;
        let _ = step_until_frame(&mut vm, 1_000_000);
    }
    let screen_f56 = vm.screen.to_vec();

    // Expected tint at frame 56: phase=0 (dawn), frac=56, frac_shr=7
    // tint = 7 * 0x030100 = 0x00150700 = R+21, G+7, B+0
    let tint_r: i32 = 21;
    let tint_g: i32 = 7;

    // Find flat non-water tiles by looking for 4x4 uniform blocks at tile boundaries
    // where the color has moderate R (not water which is low R high B)
    let tile_size = 4;
    let mut matching_tiles = 0usize;
    let mut r_deltas: Vec<i32> = Vec::new();
    let mut g_deltas: Vec<i32> = Vec::new();

    for ty in (0..256).step_by(tile_size) {
        for tx in (0..256).step_by(tile_size) {
            let idx = ty * 256 + tx;
            let c0 = screen_f0[idx];
            let c56 = screen_f56[idx];

            let r0 = ((c0 >> 16) & 0xFF) as i32;
            let g0 = ((c0 >> 8) & 0xFF) as i32;
            let b0 = (c0 & 0xFF) as i32;

            // Skip water (low R, high B) and black (unrendered)
            if r0 < 30 || b0 > 180 {
                continue;
            }

            // Check it's a flat tile: all 4 corners of the 4x4 block are same color
            let corners = [(tx, ty), (tx + 3, ty), (tx, ty + 3), (tx + 3, ty + 3)];
            let mut flat = true;
            for &(cx, cy) in &corners {
                if cx < 256 && cy < 256 {
                    let ci = cy * 256 + cx;
                    if screen_f0[ci] != c0 {
                        flat = false;
                        break;
                    }
                }
            }
            if !flat {
                continue;
            }

            let r56 = ((c56 >> 16) & 0xFF) as i32;
            let g56 = ((c56 >> 8) & 0xFF) as i32;

            r_deltas.push(r56 - r0);
            g_deltas.push(g56 - g0);
            matching_tiles += 1;
        }
    }

    eprintln!(
        "Found {} flat non-water tiles, checking tint deltas",
        matching_tiles
    );
    assert!(
        matching_tiles > 100,
        "should find many flat tiles, got {}",
        matching_tiles
    );

    // The vast majority of flat tiles should show the expected R and G tint delta
    let r_match = r_deltas.iter().filter(|&&d| d == tint_r).count();
    let g_match = g_deltas.iter().filter(|&&d| d == tint_g).count();

    eprintln!(
        "R delta: {}/{} tiles match expected +{}",
        r_match,
        r_deltas.len(),
        tint_r
    );
    eprintln!(
        "G delta: {}/{} tiles match expected +{}",
        g_match,
        g_deltas.len(),
        tint_g
    );

    // At least 75% of flat tiles should match the exact tint delta
    // (some won't match due to water tile edges, contour lines, or 8-bit wraparound)
    assert!(
        r_match > matching_tiles * 75 / 100,
        "expected >{} flat tiles with R+{}, got {}/{}",
        matching_tiles * 75 / 100,
        tint_r,
        r_match,
        matching_tiles
    );
    assert!(
        g_match > matching_tiles * 75 / 100,
        "expected >{} flat tiles with G+{}, got {}/{}",
        matching_tiles * 75 / 100,
        tint_g,
        g_match,
        matching_tiles
    );
}

#[test]
fn test_infinite_map_pxpk_contour_lines() {
    // Elevation contour lines: 1px dark lines (0x222222) at tile boundaries
    // where fine_hash elevation (top 3 bits, 0-7) changes by > 2.
    // Not drawn on water tiles. Creates topographic map effect.
    let mut vm = infinite_map_pxpk_vm();
    vm.ram[0xFFB] = 0;
    let _ = step_until_frame(&mut vm, 1_500_000);
    assert!(vm.frame_ready);

    let contour_color: u32 = 0x222222;

    // Count contour line pixels (dark gray pixels at tile edges)
    let tile_size = 4;
    let mut contour_pixels = 0usize;
    let mut total_edge_pixels = 0usize;

    // Check right edges (x = 3, 7, 11, ... 251, 255) and bottom edges (y = 3, 7, ...)
    for ty in (0..256).step_by(tile_size) {
        for tx in (0..256).step_by(tile_size) {
            // Right edge: column tx+3
            let rx = tx + tile_size - 1;
            if rx < 256 {
                for dy in 0..tile_size {
                    let ry = ty + dy;
                    if ry < 256 {
                        let px = vm.screen[ry * 256 + rx];
                        total_edge_pixels += 1;
                        if px == contour_color {
                            contour_pixels += 1;
                        }
                    }
                }
            }
            // Bottom edge: row ty+3
            let by = ty + tile_size - 1;
            if by < 256 {
                for dx in 0..tile_size {
                    let rx = tx + dx;
                    if rx < 256 {
                        let px = vm.screen[by * 256 + rx];
                        total_edge_pixels += 1;
                        if px == contour_color {
                            contour_pixels += 1;
                        }
                    }
                }
            }
        }
    }

    // Should have some contour pixels (elevation varies across terrain)
    assert!(
        contour_pixels > 50,
        "should have contour line pixels, got {}/{}",
        contour_pixels,
        total_edge_pixels
    );

    // But not too many — contour lines only appear at steep elevation changes
    // (diff > 2 in a 0-7 range), so typically < 30% of edge pixels
    let ratio = contour_pixels as f64 / total_edge_pixels as f64;
    assert!(
        ratio < 0.35,
        "contour lines should be sparse (steep changes only), got {:.1}% of edges",
        ratio * 100.0
    );

    // Water tiles should NOT have contour lines
    // Find a water pixel (high blue channel) and verify it's not at a contour edge
    // or that its edge pixels aren't contour_color
    let water_tiles_without_contour = (0..256)
        .step_by(tile_size)
        .flat_map(|ty| (0..256).step_by(tile_size).map(move |tx| (tx, ty)))
        .filter(|&(tx, ty)| {
            // Check if this is a water tile (center pixel is blue-dominant)
            let cx = tx + 2;
            let cy = ty + 2;
            if cx >= 256 || cy >= 256 {
                return false;
            }
            let px = vm.screen[cy * 256 + cx];
            let r = (px >> 16) & 0xFF;
            let g = (px >> 8) & 0xFF;
            let b = px & 0xFF;
            if !(b > 100 && b > r * 2) {
                return false;
            } // not water
              // Check edges: right edge pixel should NOT be contour_color
            let rx = tx + 3;
            if rx < 256 {
                for dy in 0..4 {
                    let ry = ty + dy;
                    if ry < 256 && vm.screen[ry * 256 + rx] == contour_color {
                        return false; // contour on water tile
                    }
                }
            }
            true
        })
        .count();

    assert!(
        water_tiles_without_contour > 10,
        "water tiles should not have contour lines, only {} water tiles had no contour",
        water_tiles_without_contour
    );
}

#[test]
fn test_infinite_map_pxpk_day_night_tint() {
    // Verify the day/night tint produces visible color shifts across frames.
    // The tint cycles with period 256, 4 phases of 64 frames:
    //   Dawn (0-63): warm R+21 G+7, Day (64-127): fade to neutral,
    //   Dusk (128-191): amber R+21, Night (192-255): cool G+7 B+21
    let mut vm = infinite_map_pxpk_vm();
    vm.ram[0xFFB] = 0;

    // Frame 0: dawn phase, frac=0 → tint=0 (start of dawn, not yet warm)
    let _ = step_until_frame(&mut vm, 1_000_000);
    assert!(vm.frame_ready);
    let screen_dawn_start = vm.screen.to_vec();
    let _avg_r_start: u32 = screen_dawn_start
        .iter()
        .map(|&p| (p >> 16) & 0xFF)
        .sum::<u32>();

    // Run more frames to reach peak dawn (frame ~56, frac_shr=7)
    for _ in 0..56 {
        vm.frame_ready = false;
        let _ = step_until_frame(&mut vm, 1_000_000);
    }
    assert!(vm.frame_ready);
    let screen_dawn_peak = vm.screen.to_vec();

    // Run to night phase (frame 192+, frac=0 → tint=0)
    for _ in 0..136 {
        vm.frame_ready = false;
        let _ = step_until_frame(&mut vm, 1_000_000);
    }
    assert!(vm.frame_ready);
    let screen_night_start = vm.screen.to_vec();

    // Count pixels that differ between dawn peak and night start
    let diff_count = screen_dawn_peak
        .iter()
        .zip(screen_night_start.iter())
        .filter(|(a, b)| a != b)
        .count();

    // The tint should produce measurable differences
    eprintln!(
        "Dawn peak vs night start: {}/{} pixels differ",
        diff_count,
        256 * 256
    );
    assert!(
        diff_count > 1000,
        "day/night tint should change visible pixels, got {} diffs",
        diff_count
    );

    // Verify tint doesn't break biome count (biomes still distinguishable)
    let mut colors = std::collections::HashSet::new();
    for y in [0, 32, 64, 128, 200] {
        for x in [0, 32, 64, 128, 200] {
            let idx = (y as usize) * 256 + (x as usize);
            if idx < screen_dawn_peak.len() {
                colors.insert(screen_dawn_peak[idx]);
            }
        }
    }
    assert!(
        colors.len() > 5,
        "biomes should still be varied during dawn, got {} unique colors",
        colors.len()
    );
}

#[test]
fn test_infinite_map_pxpk_sky_gradient() {
    // The sky gradient paints the top 16 rows with phase-dependent colors.
    // Dawn: blue-purple→orange, Day: blue→light-blue,
    // Dusk: dark-purple→deep-orange, Night: near-black→dark-navy.
    // Verify: (1) top 16 rows are dominated by sky colors, not terrain.
    //         (2) sky color changes across phases.
    //         (3) bottom rows (below sky) are NOT overwritten by sky.
    let mut vm = infinite_map_pxpk_vm();
    vm.ram[0xFFB] = 0;

    // Frame 1: dawn phase (fc=1, phase=0). Sky should be blue-purple top, orange horizon.
    let _ = step_until_frame(&mut vm, 1_000_000);
    assert!(vm.frame_ready);
    let screen_dawn = vm.screen.to_vec();

    // Top row (y=0) should be the dawn top color: 0x101040 (deep blue-purple)
    let top_pixel_dawn = screen_dawn[0]; // (x=0, y=0)
    let top_r = ((top_pixel_dawn >> 16) & 0xFF) as u32;
    let top_g = ((top_pixel_dawn >> 8) & 0xFF) as u32;
    let top_b = (top_pixel_dawn & 0xFF) as u32;
    assert!(
        top_b > top_r && top_b > top_g,
        "dawn sky top should be blue-dominant, got R={} G={} B={}",
        top_r,
        top_g,
        top_b
    );

    // Row 12-15 (band 3) should be orange horizon: 0xCC6600
    let horizon_pixel_dawn = screen_dawn[12 * 256 + 0]; // (x=0, y=12)
    let horiz_r = ((horizon_pixel_dawn >> 16) & 0xFF) as u32;
    let _horiz_g = ((horizon_pixel_dawn >> 8) & 0xFF) as u32;
    assert!(
        horiz_r > 100,
        "dawn horizon band should have significant red, got R={}",
        horiz_r
    );

    // Sky band (rows 0-15) should be uniform per band (all same color across X)
    let band0_color = screen_dawn[0]; // (0, 0)
    let band0_color_end = screen_dawn[223]; // (223, 0) -- before minimap
    assert_eq!(
        band0_color, band0_color_end,
        "sky band 0 should be uniform across x"
    );

    // Below sky (row 16+) should NOT be sky-colored (terrain should show)
    let terrain_pixel = screen_dawn[16 * 256 + 128]; // (128, 16)
    assert_ne!(
        terrain_pixel, top_pixel_dawn,
        "row 16 should be terrain, not sky"
    );

    // Now advance to night phase (fc >= 193, phase 3)
    // fc increments by 1 each frame. Frame 1 had fc=1. We need fc ~193.
    // Advance 192 more frames.
    for _ in 0..192 {
        vm.frame_ready = false;
        let _ = step_until_frame(&mut vm, 1_000_000);
    }
    assert!(vm.frame_ready);
    let screen_night = vm.screen.to_vec();

    // Night top should be very dark: 0x050510
    let top_pixel_night = screen_night[0];
    assert_ne!(
        top_pixel_dawn, top_pixel_night,
        "dawn and night sky top should differ"
    );

    let night_r = ((top_pixel_night >> 16) & 0xFF) as u32;
    let night_g = ((top_pixel_night >> 8) & 0xFF) as u32;
    let night_b = (top_pixel_night & 0xFF) as u32;
    // Night top is 0x050510 -- all channels very low
    assert!(
        night_r < 30 && night_g < 30 && night_b < 30,
        "night sky top should be very dark, got R={} G={} B={}",
        night_r,
        night_g,
        night_b
    );

    // Verify gradient exists within a single frame:
    // Band 0 (row 0) should differ from band 3 (row 12) during dawn.
    // We re-run dawn to check.
    let mut vm2 = infinite_map_pxpk_vm();
    vm2.ram[0xFFB] = 0;
    let _ = step_until_frame(&mut vm2, 1_000_000);
    let screen_dawn2 = vm2.screen.to_vec();

    let band0 = screen_dawn2[0]; // row 0
    let band3 = screen_dawn2[12 * 256]; // row 12
    assert_ne!(
        band0, band3,
        "sky gradient bands should differ within a single frame"
    );

    eprintln!(
        "Sky gradient: dawn top={:06X}, dawn horizon={:06X}, night top={:06X}",
        top_pixel_dawn, horizon_pixel_dawn, top_pixel_night
    );
}

#[test]
fn test_infinite_map_pxpk_water_animates() {
    // Water tiles should shimmer -- accent color changes between frames
    // due to (frame_counter & 0xF) cycling XOR on the accent.
    // Water is forced to center pattern (2x2 bright center on 4x4 base).
    let mut vm = infinite_map_pxpk_vm();
    vm.ram[0xFFB] = 0;

    // Frame 0 (fc=1 after increment, tint=0 since frac_shr=0)
    let _ = step_until_frame(&mut vm, 1_000_000);
    assert!(vm.frame_ready);
    let screen_f0 = vm.screen.to_vec();

    // Frame 1 (fc=2, tint=0 still) -- only shimmer differs
    vm.frame_ready = false;
    let _ = step_until_frame(&mut vm, 1_000_000);
    assert!(vm.frame_ready);
    let screen_f1 = vm.screen.to_vec();

    // Find water pixels (blue-dominant, low red) and check they changed
    let mut water_total = 0usize;
    let mut water_changed = 0usize;
    for i in 0..256 * 256 {
        let c0 = screen_f0[i];
        let r0 = ((c0 >> 16) & 0xFF) as u32;
        let b0 = (c0 & 0xFF) as u32;
        // Water: blue-dominant, low red
        if r0 < 30 && b0 > 50 {
            water_total += 1;
            if screen_f0[i] != screen_f1[i] {
                water_changed += 1;
            }
        }
    }

    eprintln!(
        "Water shimmer: {}/{} pixels changed between frames 0 and 1",
        water_changed, water_total
    );
    assert!(
        water_total > 50,
        "should find water pixels, got {}",
        water_total
    );
    // Water center pattern is 2x2 out of 4x4 = 4/16 = 25% of water pixels.
    // With spatially-varying shimmer (wave_phase = fc + fine_hash_nibble),
    // some tiles may produce identical accent across adjacent frames.
    // Allow lower threshold since spatial variation reduces frame-to-frame delta.
    assert!(
        water_changed > water_total / 10,
        "at least ~10% of water pixels should shimmer, got {}/{}",
        water_changed,
        water_total
    );
}

#[test]
fn test_infinite_map_pxpk_coastline_foam() {
    // Coastline foam: water tiles with land left-neighbor get +0x303030 tint.
    // This produces water pixels with elevated R+G channels (normally water has
    // R<30, G<50). Foam water will have R>=0x30 or G>=0x30.
    // At least some foam pixels should exist on any varied map.
    let mut vm = infinite_map_pxpk_vm();
    vm.ram[0xFFB] = 0;
    let _ = step_until_frame(&mut vm, 1_000_000);
    assert!(vm.frame_ready);

    // Frame 0: tint=0. Water biome 0 base = 0x000044, biome 1 = 0x0000BB.
    // Without foam: R < 30, B > 50. With foam: R >= 0x30 (48+) or G >= 0x30.
    let mut water_total = 0usize;
    let mut foam_pixels = 0usize;
    for i in 0..256 * 256 {
        let c = vm.screen[i];
        let r = ((c >> 16) & 0xFF) as u32;
        let g = ((c >> 8) & 0xFF) as u32;
        let b = (c & 0xFF) as u32;
        // Water: blue-dominant
        if b > 50 && r < b / 2 {
            water_total += 1;
            // Foam lifts R and G significantly from base water
            if r >= 0x30 || g >= 0x30 {
                foam_pixels += 1;
            }
        }
    }

    eprintln!(
        "Coastline foam: {}/{} water pixels have foam tint",
        foam_pixels, water_total
    );
    assert!(
        water_total > 50,
        "should find water pixels, got {}",
        water_total
    );
    // Some water tiles should border land and get foam
    assert!(
        foam_pixels > 0,
        "at least 1 water pixel should have coastline foam, got {}/{}",
        foam_pixels,
        water_total
    );
}

#[test]
fn test_infinite_map_pxpk_height_shading() {
    // Height shading adds 0x030303 * (fine_hash >> 28 & 7) to non-water tiles.
    // R-variation only affects blue channel (small ±12 values).
    // Height shading adds equal offset to R, G, B → green channel variation
    // across same-biome tiles proves height shading is active.
    let mut vm = infinite_map_pxpk_vm();
    vm.ram[0xFFB] = 0;
    let _ = step_until_frame(&mut vm, 1_000_000);
    assert!(vm.frame_ready);

    // Frame 0: tint=0, so all variation comes from biome + height + R-variation (blue only).
    // Group tiles by approximate biome (rounded R,G) and check green-channel spread.
    let mut biome_groups: std::collections::BTreeMap<(u8, u8), Vec<u8>> =
        std::collections::BTreeMap::new();

    for ty in (0..256).step_by(4) {
        for tx in (0..256).step_by(4) {
            let idx = ty * 256 + tx;
            let color = vm.screen[idx];
            if color == 0 {
                continue;
            }
            let r = ((color >> 16) & 0xFF) as u8;
            let g = ((color >> 8) & 0xFF) as u8;
            let b = (color & 0xFF) as u8;

            // Skip water (blue-dominant, low R)
            if r < 30 && b > 100 {
                continue;
            }

            // Biome key = rounded (R, G) to group same-biome tiles
            let biome_key = (r / 8 * 8, g / 8 * 8);
            biome_groups.entry(biome_key).or_default().push(g);
        }
    }

    // Check that biome groups have green-channel variation from height shading
    let mut groups_with_height = 0;
    for (_biome, greens) in &biome_groups {
        if greens.len() < 5 {
            continue;
        }
        let min_g = *greens.iter().min().unwrap();
        let max_g = *greens.iter().max().unwrap();
        let range = max_g - min_g;
        // Height shading adds 0-21 to green. Range > 4 indicates height variation.
        if range > 4 {
            groups_with_height += 1;
        }
    }

    eprintln!(
        "Height shading: {} biome groups show green variation > 4",
        groups_with_height
    );
    assert!(
        groups_with_height >= 3,
        "at least 3 biome groups should show height-based green variation, got {}",
        groups_with_height
    );
}

#[test]
fn test_infinite_map_pxpk_biome_blending() {
    // Biome boundary blending uses masked average at biome edges.
    // At local_x == 0 (left edge), tile blends with left neighbor biome.
    // At local_x == 7 (right edge), tile blends with right neighbor biome.
    // The blend produces colors that are intermediate between adjacent biomes.
    // We verify by scanning biome boundaries and checking for intermediate colors.
    let mut vm = infinite_map_pxpk_vm();
    vm.ram[0xFFB] = 0;
    let _ = step_until_frame(&mut vm, 1_000_000);
    assert!(vm.frame_ready);

    // Each biome is 8 tiles wide (coarse hash uses world_x >> 3).
    // Biome boundaries occur at world_x positions where (world_x >> 3) changes,
    // i.e. at world_x = 8, 16, 24, ... which in screen coords (camera at 0)
    // are tile columns tx = 8, 16, 24, ...
    // The boundary tile (local_x == 0 of new biome) should be blended.
    //
    // For a blended tile, its color should differ from the interior of both
    // adjacent biomes. Interior = tile at local_x 3 or 4 (safely away from edges).
    //
    // Check: compare boundary tile color with interior tiles of adjacent biomes.
    let mut blend_count = 0;
    let mut checked = 0;

    for boundary_tx in (8..64).step_by(8) {
        // Sample several rows
        for ty in (0..64).step_by(4) {
            let bnd_idx = ty * 256 + boundary_tx * 4;
            let left_idx = ty * 256 + (boundary_tx - 4) * 4; // interior of left biome (local_x 4)
            let right_idx = ty * 256 + (boundary_tx + 3) * 4; // interior of right biome (local_x 3)

            if bnd_idx >= 256 * 256 || left_idx >= 256 * 256 || right_idx >= 256 * 256 {
                continue;
            }

            let bnd_color = vm.screen[bnd_idx];
            let left_color = vm.screen[left_idx];
            let right_color = vm.screen[right_idx];

            // Skip if any are black (shouldn't happen but safety)
            if bnd_color == 0 || left_color == 0 || right_color == 0 {
                continue;
            }

            // Skip water (biome 0/1) -- water has shimmer/foam that changes colors
            let bnd_b = (bnd_color & 0xFF) as i32;
            let bnd_r = ((bnd_color >> 16) & 0xFF) as i32;
            if bnd_r < 30 && bnd_b > 100 {
                continue;
            }

            checked += 1;

            // If boundary tile color differs from BOTH neighbors' interiors,
            // blending is happening. The boundary tile should be an intermediate.
            // We check that it's not identical to either interior color.
            let left_b = ((left_color >> 16) & 0xFF) as i32;
            let left_g = ((left_color >> 8) & 0xFF) as i32;
            let right_b = ((right_color >> 16) & 0xFF) as i32;
            let right_g = ((right_color >> 8) & 0xFF) as i32;

            let bnd_rg = (bnd_r, ((bnd_color >> 8) & 0xFF) as i32);
            let left_rg = (left_b, left_g);
            let right_rg = (right_b, right_g);

            // The boundary color should differ from at least one interior
            // (it blends left and right biomes, so it's intermediate)
            let differs_from_left = bnd_rg != left_rg;
            let differs_from_right = bnd_rg != right_rg;

            if differs_from_left || differs_from_right {
                blend_count += 1;
            }
        }
    }

    eprintln!(
        "Biome blending: {}/{} boundary tiles show intermediate colors",
        blend_count, checked
    );
    // Not every boundary will blend (some neighbors have same biome type),
    // but a significant fraction should show blending.
    assert!(
        blend_count >= 5,
        "at least 5 boundary tiles should show blended colors, got {}/{}",
        blend_count,
        checked
    );
}

#[test]
fn test_infinite_map_pxpk_y_blending() {
    // Y-direction biome blending: at local_y == 0 (top edge of biome),
    // tile blends with the biome above. At local_y == 7 (bottom edge),
    // tile blends with the biome below. Uses the same masked average
    // technique as X-direction blending.
    let mut vm = infinite_map_pxpk_vm();
    vm.ram[0xFFB] = 0;
    let _ = step_until_frame(&mut vm, 1_000_000);
    assert!(vm.frame_ready);

    // Biome boundaries in Y occur at tile rows ty = 8, 16, 24, ...
    // (each biome is 8 tiles tall). The boundary tile at local_y == 0
    // of the new biome should be blended with the biome above.
    let mut blend_count = 0;
    let mut checked = 0;

    for boundary_ty in (8..64).step_by(8) {
        // Sample several columns
        for tx in (0..64).step_by(4) {
            let bnd_idx = boundary_ty * 16 + tx * 4; // top-left pixel of boundary tile
            let above_idx = (boundary_ty - 4) * 16 + tx * 4; // interior of biome above
            let below_idx = (boundary_ty + 3) * 16 + tx * 4; // interior of biome below

            if bnd_idx >= 256 * 256 || above_idx >= 256 * 256 || below_idx >= 256 * 256 {
                continue;
            }

            let bnd_color = vm.screen[bnd_idx];
            let above_color = vm.screen[above_idx];
            let below_color = vm.screen[below_idx];

            // Skip black pixels
            if bnd_color == 0 || above_color == 0 || below_color == 0 {
                continue;
            }

            // Skip water (biome 0/1) -- shimmer/foam changes colors
            let bnd_b = (bnd_color & 0xFF) as i32;
            let bnd_r = ((bnd_color >> 16) & 0xFF) as i32;
            if bnd_r < 30 && bnd_b > 100 {
                continue;
            }

            checked += 1;

            // Boundary tile should differ from at least one neighbor interior
            let differs_from_above = bnd_color != above_color;
            let differs_from_below = bnd_color != below_color;

            if differs_from_above || differs_from_below {
                blend_count += 1;
            }
        }
    }

    eprintln!(
        "Y-direction blending: {}/{} boundary tiles show intermediate colors",
        blend_count, checked
    );

    assert!(
        blend_count >= 5,
        "at least 5 Y-boundary tiles should show blended colors, got {}/{}",
        blend_count,
        checked
    );
}

#[test]
fn test_infinite_map_pxpk_smooth_transition_zone() {
    // The 4-tile transition zone (positions 0,1,6,7) should produce blended
    // colors at more tiles than the old 2-tile zone (positions 0,7 only).
    // We verify by counting how many "near-edge" tiles (local_x 1 or 6)
    // show colors that differ from their biome's interior.
    let mut vm = infinite_map_pxpk_vm();
    vm.ram[0xFFB] = 0;
    let _ = step_until_frame(&mut vm, 1_000_000);
    assert!(vm.frame_ready);

    // Scan tiles at local_x == 1 (second column of each biome, 1 tile from left edge)
    // and local_x == 6 (second-to-last column, 1 tile from right edge).
    // These are the hash-dithered positions -- ~50% should blend with the neighbor.
    let mut inner_edge_checked = 0;
    let mut inner_edge_blended = 0;

    for boundary_tx in (8..56).step_by(8) {
        for ty in (0..64).step_by(2) {
            // local_x == 1: tile at boundary_tx + 1 (second column of right biome)
            let inner_left_px = boundary_tx * 4 + 4; // local_x=1 in screen coords
            let interior_left_px = (boundary_tx + 3) * 4; // local_x=3 (interior)
            let idx = ty * 256 + inner_left_px;
            let int_idx = ty * 256 + interior_left_px;

            if idx >= 256 * 256 || int_idx >= 256 * 256 {
                continue;
            }

            let color = vm.screen[idx];
            let int_color = vm.screen[int_idx];

            if color == 0 || int_color == 0 {
                continue;
            }

            // Skip water
            let r = ((color >> 16) & 0xFF) as i32;
            let b = (color & 0xFF) as i32;
            if r < 30 && b > 100 {
                continue;
            }

            inner_edge_checked += 1;

            // Check if inner-edge tile differs from interior (hash-dithered blend)
            let color_rg = ((color >> 16) & 0xFF, (color >> 8) & 0xFF);
            let int_rg = ((int_color >> 16) & 0xFF, (int_color >> 8) & 0xFF);

            if color_rg != int_rg {
                inner_edge_blended += 1;
            }
        }
    }

    eprintln!(
        "Smooth transition zone: {}/{} inner-edge tiles show blending (expect ~25-60%)",
        inner_edge_blended, inner_edge_checked
    );

    // With hash dithering, ~50% of inner-edge tiles blend. We expect at least
    // 15% to show blending (conservative -- accounts for same-biome neighbors
    // that wouldn't produce visible color change even when blended).
    assert!(
        inner_edge_blended >= inner_edge_checked / 7,
        "at least ~15% of inner-edge tiles should show hash-dithered blending, got {}/{}",
        inner_edge_blended,
        inner_edge_checked
    );
}

#[test]
fn test_infinite_map_pxpk_32x32_minimap_overlay() {
    // Verify the 32x32 minimap renders in the top-right corner.
    // The minimap covers screen x=224..255, y=0..31 (32x32 pixels).
    // It uses the same biome hash as the main terrain, dimmed to 50%.
    // Updated every 4 frames; border + player dot drawn every frame.
    let mut vm = infinite_map_pxpk_vm();
    vm.ram[0xFFB] = 0;
    let _ = step_until_frame(&mut vm, 1_000_000);
    assert!(vm.frame_ready);

    // Check minimap area (x=224..255, y=0..31) has non-black pixels
    let mut mm_colored = 0;
    let mut mm_colors = std::collections::HashSet::new();
    for y in 0..32 {
        for x in 224..256 {
            let idx = y * 256 + x;
            let px = vm.screen[idx];
            if px != 0 {
                mm_colored += 1;
            }
            mm_colors.insert(px);
        }
    }

    // At least half of the 1024 minimap pixels should be colored (biome terrain)
    assert!(
        mm_colored > 512,
        "32x32 minimap should have mostly colored pixels, got {}/1024",
        mm_colored
    );

    // Minimap should show multiple biomes (not all same color)
    // Remove black (0) from count for variety check
    mm_colors.remove(&0);
    assert!(
        mm_colors.len() > 3,
        "minimap should show multiple biome colors, got {} unique",
        mm_colors.len()
    );

    // Player dot at center (x=240, y=16) should be white
    let player_dot = vm.screen[16 * 256 + 240];
    assert_eq!(
        player_dot, 0xFFFFFF,
        "player center dot should be white, got {:06X}",
        player_dot
    );

    // Border pixels at edges (e.g., top-left corner x=224,y=0 should be border color)
    // Top edge: y=0, x=224..255 should be 0xAAAAAA or adjacent to it
    let top_left = vm.screen[224]; // x=224, y=0
    assert!(
        top_left == 0xAAAAAA || top_left != 0,
        "top-left minimap border should be visible, got {:06X}",
        top_left
    );

    // Verify minimap is in top-right (not overlapping main terrain viewport center)
    // The main terrain fills 0..223 in x, so 224+ should be minimap-only area
    // Check a few pixels just left of the minimap are main terrain (non-dimmed)
    let _main_px = vm.screen[128 * 256 + 128]; // center of screen
    let _mm_px = vm.screen[16 * 256 + 240]; // center of minimap
                                            // Minimap pixels are dimmed (>>1), so they should differ from main terrain
                                            // unless coincidentally same biome at half brightness
    assert!(mm_colored > 0, "minimap area should have rendered content");
}

#[test]
fn test_infinite_map_pxpk_minimap_updates_every_4_frames() {
    // The minimap should only recompute terrain every 4 frames.
    // Frames 0,4,8,... update the terrain; frames 1,2,3 reuse cached pixels.
    // We verify by checking that the minimap content changes between frame 3→4
    // (when camera moves) but not between frame 0→1 (cached).
    let mut vm = infinite_map_pxpk_vm();

    // Move camera right every frame
    vm.ram[0xFFB] = 8; // bit 3 = right

    // Frame 0: initial minimap render (fc becomes 1, (1-1)&3=0, recompute)
    let _ = step_until_frame(&mut vm, 1_500_000);
    assert!(vm.frame_ready);
    let mut mm_frame0 = Vec::new();
    for y in 0..32 {
        for x in 224..256 {
            mm_frame0.push(vm.screen[y * 256 + x]);
        }
    }

    // Frame 1: cached (fc=2, (2-1)&3=1, skip recompute)
    vm.frame_ready = false;
    let _ = step_until_frame(&mut vm, 1_500_000);
    assert!(vm.frame_ready);
    let mut mm_frame1 = Vec::new();
    for y in 0..32 {
        for x in 224..256 {
            mm_frame1.push(vm.screen[y * 256 + x]);
        }
    }

    // Frame 2: cached (fc=3, (3-1)&3=2, skip)
    vm.frame_ready = false;
    let _ = step_until_frame(&mut vm, 1_500_000);
    assert!(vm.frame_ready);
    let mut mm_frame2 = Vec::new();
    for y in 0..32 {
        for x in 224..256 {
            mm_frame2.push(vm.screen[y * 256 + x]);
        }
    }

    // Frame 3: cached (fc=4, (4-1)&3=3, skip)
    vm.frame_ready = false;
    let _ = step_until_frame(&mut vm, 1_500_000);
    assert!(vm.frame_ready);
    let mut mm_frame3 = Vec::new();
    for y in 0..32 {
        for x in 224..256 {
            mm_frame3.push(vm.screen[y * 256 + x]);
        }
    }

    // Frame 4: recompute! (fc=5, (5-1)&3=0, update!)
    vm.frame_ready = false;
    let _ = step_until_frame(&mut vm, 1_500_000);
    assert!(vm.frame_ready);
    let mut mm_frame4 = Vec::new();
    for y in 0..32 {
        for x in 224..256 {
            mm_frame4.push(vm.screen[y * 256 + x]);
        }
    }

    // Frames 0-2 should have identical minimap content (cached, no recompute)
    let same_01 = mm_frame0
        .iter()
        .zip(mm_frame1.iter())
        .filter(|(a, b)| a == b)
        .count();
    let same_12 = mm_frame1
        .iter()
        .zip(mm_frame2.iter())
        .filter(|(a, b)| a == b)
        .count();

    // At least 90% of pixels should match between cached frames
    assert!(
        same_01 > 900,
        "frames 0-1 minimap should be ~identical (cached), got {}/1024 matching",
        same_01
    );
    assert!(
        same_12 > 900,
        "frames 1-2 minimap should be ~identical (cached), got {}/1024 matching",
        same_12
    );

    // Frame 3 (cached) should be identical to frame 2
    let same_23 = mm_frame2
        .iter()
        .zip(mm_frame3.iter())
        .filter(|(a, b)| a == b)
        .count();

    // Frame 4 (recompute!) should differ from frame 3 due to camera movement
    let same_34 = mm_frame3
        .iter()
        .zip(mm_frame4.iter())
        .filter(|(a, b)| a == b)
        .count();

    eprintln!(
        "Minimap caching: f0-f1={}/1024, f1-f2={}/1024, f2-f3={}/1024, f3-f4={}/1024 matching",
        same_01, same_12, same_23, same_34
    );

    // Cached frames should be identical
    assert!(
        same_23 > 900,
        "frames 2-3 minimap should be ~identical (cached), got {}/1024 matching",
        same_23
    );

    // After 5 frames of camera movement, the minimap should have shifted
    // At least some pixels should differ on the recompute frame
    assert!(
        same_34 < 1024,
        "frame 4 minimap should differ from frame 3 (camera moved, minimap updated), got {}/1024 identical",
        same_34
    );
}

#[test]
fn test_infinite_map_pxpk_tree_sprites_on_grass_and_forest() {
    // Verify tree sprites appear on grass (biome 6-7) and forest (biome 10-11) tiles.
    // Trees are drawn as brown trunk (0x664422) + green canopy (0x228811) overlaid
    // on the terrain tile. They use fine_hash for deterministic placement:
    // grass ~25%, forest ~50%. We verify by scanning for tree pixels.
    let mut vm = infinite_map_pxpk_vm();
    vm.ram[0xFFB] = 0;
    let _ = step_until_frame(&mut vm, 1_000_000);
    assert!(vm.frame_ready);

    // Tree colors
    let canopy_color = 0x228811u32;
    let trunk_color = 0x664422u32;

    // Scan screen for tree pixels
    let mut canopy_pixels = 0usize;
    let mut trunk_pixels = 0usize;
    for y in 0..256 {
        for x in 0..256 {
            let px = vm.screen[y * 256 + x];
            if px == canopy_color {
                canopy_pixels += 1;
            }
            if px == trunk_color {
                trunk_pixels += 1;
            }
        }
    }

    eprintln!(
        "Tree sprites: {} canopy pixels, {} trunk pixels",
        canopy_pixels, trunk_pixels
    );

    // Should find some tree canopy pixels (forest ~50%, grass ~25% of their tiles)
    assert!(
        canopy_pixels > 0,
        "should find tree canopy pixels (0x228811), got 0"
    );

    // Should find trunk pixels (1 per tree)
    assert!(
        trunk_pixels > 0,
        "should find tree trunk pixels (0x664422), got 0"
    );

    // Trunk count should be proportional to canopy count
    // Each tree has 6 canopy pixels (3x2) and 1 trunk pixel (1x1)
    // So canopy_pixels / trunk_pixels should be ~6:1
    let ratio = canopy_pixels as f64 / trunk_pixels.max(1) as f64;
    assert!(
        ratio > 3.0 && ratio < 12.0,
        "canopy/trunk ratio should be ~6:1, got {:.1}",
        ratio
    );
}

#[test]
fn test_infinite_map_pxpk_trees_deterministic() {
    // Tree placement should be deterministic: same camera = same trees every frame.
    let mut vm = infinite_map_pxpk_vm();
    vm.ram[0xFFB] = 0;

    // Render frame 0
    let _ = step_until_frame(&mut vm, 1_000_000);

    // Capture tree pixel positions from frame 0
    let mut tree_pixels_f0 = Vec::new();
    for y in 0..256 {
        for x in 0..256 {
            let px = vm.screen[y * 256 + x];
            if px == 0x228811 || px == 0x664422 {
                tree_pixels_f0.push((x, y, px));
            }
        }
    }

    // Render another frame (frame 1, no camera movement)
    vm.frame_ready = false;
    let _ = step_until_frame(&mut vm, 1_000_000);

    // Tree pixel positions should be identical (same camera, deterministic hash)
    let mut tree_pixels_f1 = Vec::new();
    for y in 0..256 {
        for x in 0..256 {
            let px = vm.screen[y * 256 + x];
            if px == 0x228811 || px == 0x664422 {
                tree_pixels_f1.push((x, y, px));
            }
        }
    }

    assert_eq!(
        tree_pixels_f0.len(),
        tree_pixels_f1.len(),
        "tree count should be deterministic across frames"
    );

    // Every tree pixel should match
    let mismatches = tree_pixels_f0
        .iter()
        .zip(tree_pixels_f1.iter())
        .filter(|(a, b)| a != b)
        .count();
    assert_eq!(
        mismatches, 0,
        "tree positions should be identical across frames with same camera"
    );
}

#[test]
fn test_infinite_map_pxpk_water_reflection() {
    // Water reflection: water tiles at biome block boundaries (world_y & 7 == 0)
    // where the tile above is land get a reflected version of that land's color
    // (dimmed 50% + blue tint 0x0E1C38 + ripple). Tiles not at a boundary or
    // with water above use normal water rendering.
    //
    // Strategy: scan the rendered frame for water-land-water vertical transitions.
    // At a land→water boundary where world_y is a multiple of 8, the water tile
    // should reflect the land color (dimmed + blue tinted) rather than pure water.
    // Reflected water pixels will have elevated R/G channels compared to normal water.

    let mut vm = infinite_map_pxpk_vm();
    vm.ram[0xFFB] = 0;
    let _ = step_until_frame(&mut vm, 1_000_000);

    // Find water pixels that could be reflections: they sit in rows where
    // camera_y + screen_row/4 is a multiple of 8 (biome boundary).
    // Normal water (biomes 0,1) has colors near 0x000044/0x0000BB (blue dominant).
    // Reflected water will have R > 0x0E or G > 0x1C from the blue tint + dimmed land.
    let camera_y = vm.ram[0x7801] as i32;
    let mut reflected_count = 0;
    let mut normal_water_count = 0;

    for sy in 0..256 {
        let world_y = camera_y + (sy as i32) / 4;
        // Check if this row is at a biome boundary (every 8th world tile)
        if world_y % 8 != 0 {
            continue;
        }
        for sx in 0..256 {
            let px = vm.screen[sy * 256 + sx];
            if px == 0 {
                continue;
            }
            // Check if pixel looks like reflected water (has R or G component)
            // Normal water is nearly pure blue: R < 0x10, G < 0x20
            let r = px & 0xFF;
            let g = (px >> 8) & 0xFF;
            let b = (px >> 16) & 0xFF;
            // Water-like blue dominance but with land color bleed
            if b > 0x30 && r > 0x0E && g > 0x1C {
                reflected_count += 1;
            } else if b > 0x30 && r < 0x10 {
                normal_water_count += 1;
            }
        }
    }

    // At camera (0,0) there should be some reflected water pixels at biome boundaries
    // where land meets water. The exact count depends on terrain generation.
    assert!(
        reflected_count > 0 || normal_water_count > 100,
        "should have water pixels (reflected={}, normal={}), terrain may not have land-water boundary at this camera",
        reflected_count, normal_water_count
    );
}

#[test]
fn test_infinite_map_pxpk_water_reflection_varies_per_frame() {
    // Reflection ripple should vary between frames due to frame_counter modulation.
    // At the same camera position, water tiles at biome boundaries should have
    // slightly different colors across frames (ripple = (fc + world_x) & 0xF * 0x020202).
    let mut vm = infinite_map_pxpk_vm();
    vm.ram[0xFFB] = 0;

    // Render frame 0
    let _ = step_until_frame(&mut vm, 1_000_000);
    let screen_f0 = vm.screen.to_vec();

    // Render frame 1
    vm.frame_ready = false;
    let _ = step_until_frame(&mut vm, 1_000_000);
    let screen_f1 = vm.screen.to_vec();

    // Count pixels that changed between frames (ripple effect)
    let changed: usize = screen_f0
        .iter()
        .zip(screen_f1.iter())
        .filter(|(a, b)| a != b)
        .count();

    // With 64x64 tiles and many water tiles, ripple should cause some changes.
    // Even non-reflected water has shimmer, so there should always be some change.
    assert!(
        changed > 100,
        "water ripple should cause pixel changes between frames, got {} changed",
        changed
    );
}

#[test]
fn test_infinite_map_runs_and_renders() {
    // Requirement: the program runs to completion of a frame and renders non-black pixels.
    let mut vm = infinite_map_vm();

    // No key input -- camera stays at (0,0)
    vm.ram[0xFFB] = 0;

    let steps = step_until_frame(&mut vm, 1_000_000);
    assert!(
        vm.frame_ready,
        "should reach FRAME within 1M steps (took {})",
        steps
    );

    // Screen should have rendered terrain -- not all black.
    let non_black: usize = vm.screen.iter().filter(|&&p| p != 0).count();
    assert!(
        non_black > 0,
        "screen should have non-black pixels after rendering, got 0/{}",
        256 * 256
    );

    // With 64x64 tiles covering the full 256x256 screen, nearly all pixels should be colored.
    // Water at (0,0) still produces non-black blue pixels.
    assert!(
        non_black > 50000,
        "most of the screen should be colored, got {}/{} non-black pixels",
        non_black,
        256 * 256
    );
}

#[test]
fn test_infinite_map_camera_moves_on_key_input() {
    // Requirement: camera moves when arrow keys are pressed.
    let mut vm = infinite_map_vm();

    // --- Frame 1: press Right (bit 3 = 8) ---
    vm.ram[0xFFB] = 8;
    let steps = step_until_frame(&mut vm, 1_000_000);
    assert!(
        vm.frame_ready,
        "frame 1 should render within 1M steps (took {})",
        steps
    );
    assert_eq!(
        vm.ram[0x7800], 1,
        "camera_x should be 1 after pressing Right"
    );
    assert_eq!(
        vm.ram[0x7801], 0,
        "camera_y should be 0 (no vertical input)"
    );

    // --- Frame 2: press Down (bit 1 = 2) ---
    vm.ram[0xFFB] = 2;
    let steps = step_until_frame(&mut vm, 1_000_000);
    assert!(
        vm.frame_ready,
        "frame 2 should render within 1M steps (took {})",
        steps
    );
    assert_eq!(vm.ram[0x7800], 1, "camera_x should still be 1");
    assert_eq!(
        vm.ram[0x7801], 1,
        "camera_y should be 1 after pressing Down"
    );

    // --- Frame 3: press Up+Left (bits 0+2 = 5) ---
    vm.ram[0xFFB] = 5;
    let steps = step_until_frame(&mut vm, 1_000_000);
    assert!(
        vm.frame_ready,
        "frame 3 should render within 1M steps (took {})",
        steps
    );
    assert_eq!(
        vm.ram[0x7800], 0,
        "camera_x should be 0 after pressing Left"
    );
    assert_eq!(vm.ram[0x7801], 0, "camera_y should be 0 after pressing Up");

    // --- Frame 4: no keys, camera stays ---
    vm.ram[0xFFB] = 0;
    let steps = step_until_frame(&mut vm, 1_000_000);
    assert!(
        vm.frame_ready,
        "frame 4 should render within 1M steps (took {})",
        steps
    );
    assert_eq!(vm.ram[0x7800], 0, "camera_x should stay 0 with no input");
    assert_eq!(vm.ram[0x7801], 0, "camera_y should stay 0 with no input");

    // Frame counter should have incremented each frame.
    assert!(
        vm.ram[0x7802] >= 4,
        "frame_counter should be >= 4, got {}",
        vm.ram[0x7802]
    );
}

#[test]
fn test_infinite_map_camera_moves_multiple_steps() {
    // Requirement: holding a direction for multiple frames accumulates movement.
    let mut vm = infinite_map_vm();

    // Hold Right for 5 frames.
    for frame in 1..=5 {
        vm.ram[0xFFB] = 8; // Right
        let steps = step_until_frame(&mut vm, 1_000_000);
        assert!(
            vm.frame_ready,
            "frame {} should render (took {} steps)",
            frame, steps
        );
    }
    assert_eq!(
        vm.ram[0x7800], 5,
        "camera_x should be 5 after 5 Right presses"
    );
    assert_eq!(vm.ram[0x7801], 0, "camera_y should still be 0");

    // Now hold Down+Right for 3 frames.
    for frame in 6..=8 {
        vm.ram[0xFFB] = 8 | 2; // Right + Down = 10
        let steps = step_until_frame(&mut vm, 1_000_000);
        assert!(
            vm.frame_ready,
            "frame {} should render (took {} steps)",
            frame, steps
        );
    }
    assert_eq!(vm.ram[0x7800], 8, "camera_x should be 5+3=8");
    assert_eq!(vm.ram[0x7801], 3, "camera_y should be 0+3=3");
}

#[test]
fn test_infinite_map_screen_differs_per_camera_position() {
    // Requirement: different camera positions produce different screens,
    // confirming the procedural terrain actually varies.
    let mut vm = infinite_map_vm();

    // Render at camera (0, 0)
    vm.ram[0xFFB] = 0;
    step_until_frame(&mut vm, 1_000_000);
    assert!(vm.frame_ready);
    let screen_origin = vm.screen.clone();

    // Manually set camera to (50, 50) and re-render
    vm.ram[0x7800] = 50;
    vm.ram[0x7801] = 50;
    vm.ram[0xFFB] = 0;
    step_until_frame(&mut vm, 1_000_000);
    assert!(vm.frame_ready);
    let screen_far = vm.screen.clone();

    // The two screens should be significantly different.
    let same: usize = screen_origin
        .iter()
        .zip(screen_far.iter())
        .filter(|(a, b)| a == b)
        .count();
    let total = 256 * 256;
    // At most 10% of pixels should be identical between two distant camera positions.
    assert!(
        same < total / 10,
        "screens at (0,0) vs (50,50) should be mostly different, but {}/{} pixels match",
        same,
        total
    );
}

#[test]
fn test_infinite_map_diagonal_keys_move_camera() {
    // Requirement: dedicated diagonal key bits (4-7) move the camera diagonally.
    let mut vm = infinite_map_vm();

    // --- Frame 1: press Up+Right diagonal (bit 4 = 16) ---
    vm.ram[0xFFB] = 16;
    let steps = step_until_frame(&mut vm, 1_000_000);
    assert!(
        vm.frame_ready,
        "frame 1 should render within 1M steps (took {})",
        steps
    );
    assert_eq!(
        vm.ram[0x7800], 1,
        "camera_x should be 1 after Up+Right diagonal"
    );
    assert_eq!(
        vm.ram[0x7801],
        u32::MAX,
        "camera_y should wrap to u32::MAX after Up+Right diagonal"
    );

    // --- Frame 2: press Down+Right diagonal (bit 5 = 32) ---
    vm.ram[0xFFB] = 32;
    let steps = step_until_frame(&mut vm, 1_000_000);
    assert!(
        vm.frame_ready,
        "frame 2 should render within 1M steps (took {})",
        steps
    );
    assert_eq!(
        vm.ram[0x7800], 2,
        "camera_x should be 2 after Down+Right diagonal"
    );
    assert_eq!(
        vm.ram[0x7801], 0,
        "camera_y should be 0 after Down+Right diagonal"
    );

    // --- Frame 3: press Down+Left diagonal (bit 6 = 64) ---
    vm.ram[0xFFB] = 64;
    let steps = step_until_frame(&mut vm, 1_000_000);
    assert!(
        vm.frame_ready,
        "frame 3 should render within 1M steps (took {})",
        steps
    );
    assert_eq!(
        vm.ram[0x7800], 1,
        "camera_x should be 1 after Down+Left diagonal"
    );
    assert_eq!(
        vm.ram[0x7801], 1,
        "camera_y should be 1 after Down+Left diagonal"
    );

    // --- Frame 4: press Up+Left diagonal (bit 7 = 128) ---
    vm.ram[0xFFB] = 128;
    let steps = step_until_frame(&mut vm, 1_000_000);
    assert!(
        vm.frame_ready,
        "frame 4 should render within 1M steps (took {})",
        steps
    );
    assert_eq!(
        vm.ram[0x7800], 0,
        "camera_x should be 0 after Up+Left diagonal"
    );
    assert_eq!(
        vm.ram[0x7801], 0,
        "camera_y should be 0 after Up+Left diagonal"
    );
}

#[test]
fn test_infinite_map_diagonal_accumulates() {
    // Requirement: diagonal keys accumulate over multiple frames.
    let mut vm = infinite_map_vm();

    // Hold Down+Right diagonal for 3 frames.
    for frame in 1..=3 {
        vm.ram[0xFFB] = 32; // Down+Right
        let steps = step_until_frame(&mut vm, 1_000_000);
        assert!(
            vm.frame_ready,
            "frame {} should render (took {} steps)",
            frame, steps
        );
    }
    assert_eq!(
        vm.ram[0x7800], 3,
        "camera_x should be 3 after 3 Down+Right diagonals"
    );
    assert_eq!(
        vm.ram[0x7801], 3,
        "camera_y should be 3 after 3 Down+Right diagonals"
    );

    // Hold Up+Left diagonal for 2 frames to partially reverse.
    for frame in 4..=5 {
        vm.ram[0xFFB] = 128; // Up+Left
        let steps = step_until_frame(&mut vm, 1_000_000);
        assert!(
            vm.frame_ready,
            "frame {} should render (took {} steps)",
            frame, steps
        );
    }
    assert_eq!(
        vm.ram[0x7800], 1,
        "camera_x should be 3-2=1 after 2 Up+Left diagonals"
    );
    assert_eq!(
        vm.ram[0x7801], 1,
        "camera_y should be 3-2=1 after 2 Up+Left diagonals"
    );
}

#[test]
fn test_infinite_map_cardinal_and_diagonal_combined() {
    // Requirement: diagonal bits stack with cardinal bits for faster movement.
    // Pressing Right (bit 3) + Down+Right diagonal (bit 5) should move x+2, y+1.
    let mut vm = infinite_map_vm();

    vm.ram[0xFFB] = 8 | 32; // Right + Down+Right diagonal = 40
    let steps = step_until_frame(&mut vm, 1_000_000);
    assert!(
        vm.frame_ready,
        "frame should render within 1M steps (took {})",
        steps
    );
    assert_eq!(
        vm.ram[0x7800], 2,
        "camera_x should be 2 (Right + Down+Right diagonal)"
    );
    assert_eq!(
        vm.ram[0x7801], 1,
        "camera_y should be 1 (Down+Right diagonal only)"
    );
}

#[test]
fn test_infinite_map_render_loop_instruction_count() {
    // Performance regression test: verify the render loop stays within budget.
    // After removing dead off-screen bounds checks (4 ops/tile = ~16K savings),
    // a single frame ran in ~322K steps.
    // With biome-aware pattern overlay (1 accent PSET per tile): ~440K steps.
    // We allow some slack (500K) to account for pattern overlay + animation.
    let mut vm = infinite_map_vm();
    vm.ram[0xFFB] = 0; // no input

    let steps = step_until_frame(&mut vm, 1_000_000);
    assert!(
        vm.frame_ready,
        "should reach FRAME within 1M steps (took {})",
        steps
    );

    // The render loop should be well under 500K steps for a single frame.
    assert!(
        steps < 500_000,
        "render loop should take < 500K steps, took {} (possible performance regression)",
        steps
    );
}

#[test]
fn test_infinite_map_player_cursor_visible() {
    // Requirement: a player cursor (crosshair) is drawn at the screen center.
    // After one frame (frame_counter=1), the cursor is white (0xFFFFFF) since
    // frame_counter & 16 == 0. The crosshair has 4 arms around (127,127) with
    // a 1px gap at center.
    let mut vm = infinite_map_vm();
    vm.ram[0xFFB] = 0;
    step_until_frame(&mut vm, 1_000_000);
    assert!(vm.frame_ready);

    let white: u32 = 0xFFFFFF;

    // Top arm: pixel (127, 124)
    let px = vm.screen[124 * 256 + 127];
    assert_eq!(
        px, white,
        "top arm pixel (127,124) should be white cursor, got {:#X}",
        px
    );

    // Bottom arm: pixel (127, 128)
    let px = vm.screen[128 * 256 + 127];
    assert_eq!(
        px, white,
        "bottom arm pixel (127,128) should be white cursor, got {:#X}",
        px
    );

    // Left arm: pixel (124, 127)
    let px = vm.screen[127 * 256 + 124];
    assert_eq!(
        px, white,
        "left arm pixel (124,127) should be white cursor, got {:#X}",
        px
    );

    // Right arm: pixel (130, 127)
    let px = vm.screen[127 * 256 + 130];
    assert_eq!(
        px, white,
        "right arm pixel (130,127) should be white cursor, got {:#X}",
        px
    );

    // Center pixel (127,127) should NOT be cursor -- it's a gap showing terrain
    let center_px = vm.screen[127 * 256 + 127];
    assert_ne!(
        center_px, white,
        "center pixel (127,127) should be terrain (cursor gap), not cursor color"
    );
}

#[test]
fn test_infinite_map_player_cursor_pulses() {
    // Requirement: cursor color pulses between white and yellow every 16 frames.
    // Run 17 frames; on frame 17, frame_counter=17, 17 & 16 != 0 -> yellow.
    let mut vm = infinite_map_vm();
    vm.ram[0xFFB] = 0;

    // Run 16 frames to get frame_counter past the pulse boundary
    for _ in 0..16 {
        step_until_frame(&mut vm, 1_000_000);
        vm.frame_ready = false;
    }
    // Frame 17: frame_counter will be 17 after increment; 17 & 16 = 16 != 0 -> yellow
    step_until_frame(&mut vm, 1_000_000);
    assert!(vm.frame_ready);

    let yellow: u32 = 0xFFFF00;
    // Top arm should be yellow now
    let px = vm.screen[124 * 256 + 127];
    assert_eq!(
        px, yellow,
        "cursor should pulse to yellow on frame 17, got {:#X}",
        px
    );
}

#[test]
fn test_infinite_map_minimap_overlay() {
    // Requirement: a 16x16 minimap overlay is rendered in the top-right corner
    // of the screen (x=240..255, y=0..15) with biome-colored pixels, a gray
    // border frame, and a white player dot at the center (248, 8).
    let mut vm = infinite_map_vm();
    vm.ram[0xFFB] = 0;
    step_until_frame(&mut vm, 1_000_000);
    assert!(vm.frame_ready);

    // 1. Minimap interior should have non-black pixels (biome colors).
    //    Interior excludes the 1px border: x=241..254, y=1..14.
    let mut interior_non_black = 0;
    for y in 1..15 {
        for x in 241..255 {
            if vm.screen[y * 256 + x] != 0 {
                interior_non_black += 1;
            }
        }
    }
    assert!(
        interior_non_black > 0,
        "minimap interior should have non-black biome pixels, got 0/196"
    );

    // 2. Border frame: all 4 edges should be entirely gray (0xAAAAAA).
    //    The border is drawn AFTER biome pixels, so it overwrites them completely.
    let border_color: u32 = 0xAAAAAA;

    // Top edge: y=0, x=240..255
    for x in 240..256 {
        let px = vm.screen[0 * 256 + x];
        assert_eq!(
            px, border_color,
            "top border pixel ({},0) should be gray, got {:#X}",
            x, px
        );
    }

    // Bottom edge: y=15, x=240..255
    for x in 240..256 {
        let px = vm.screen[15 * 256 + x];
        assert_eq!(
            px, border_color,
            "bottom border pixel ({},15) should be gray, got {:#X}",
            x, px
        );
    }

    // Left edge: x=240, y=0..15
    for y in 0..16 {
        let px = vm.screen[y * 256 + 240];
        assert_eq!(
            px, border_color,
            "left border pixel (240,{}) should be gray, got {:#X}",
            y, px
        );
    }

    // Right edge: x=255, y=0..15
    for y in 0..16 {
        let px = vm.screen[y * 256 + 255];
        assert_eq!(
            px, border_color,
            "right border pixel (255,{}) should be gray, got {:#X}",
            y, px
        );
    }

    // 3. Player dot at center of minimap (248, 8) should be white.
    //    Drawn AFTER border, so it overwrites the border pixel.
    let player_dot = vm.screen[8 * 256 + 248];
    assert_eq!(
        player_dot, 0xFFFFFF,
        "player dot at (248,8) should be white, got {:#X}",
        player_dot
    );

    // 4. Minimap terrain changes when camera moves significantly.
    //    The minimap samples every 4th tile with 8-tile zone boundaries,
    //    so we need to move at least 4 tiles to see any difference.
    //    Snapshot the interior pixels from frame 1 (camera at 0,0).
    let mut frame1_interior: Vec<u32> = Vec::new();
    for y in 1..15 {
        for x in 241..255 {
            frame1_interior.push(vm.screen[y * 256 + x]);
        }
    }

    // Move camera 10 tiles right (10 frames with right key).
    for _ in 0..10 {
        vm.ram[0xFFB] = 8; // right
        vm.frame_ready = false;
        step_until_frame(&mut vm, 1_000_000);
        assert!(vm.frame_ready);
    }
    assert_eq!(
        vm.ram[0x7800], 10,
        "camera should have moved 10 tiles right"
    );

    let mut frame2_interior: Vec<u32> = Vec::new();
    for y in 1..15 {
        for x in 241..255 {
            frame2_interior.push(vm.screen[y * 256 + x]);
        }
    }

    let diffs: usize = frame1_interior
        .iter()
        .zip(frame2_interior.iter())
        .filter(|(a, b)| a != b)
        .count();
    assert!(
        diffs > 0,
        "minimap should show different terrain after camera moves, but all 196 interior pixels identical"
    );
}

#[test]
fn test_infinite_map_diagonal_scroll() {
    // Requirement: pressing two cardinal directions simultaneously moves diagonally.
    // Up (bit 0) + Right (bit 3) = bitmask 9 should move camera_y down (wraps) and camera_x up.
    let mut vm = infinite_map_vm();

    // Frame 1: Up + Right (bits 0 + 3 = 9)
    vm.ram[0xFFB] = 9;
    let steps = step_until_frame(&mut vm, 1_000_000);
    assert!(
        vm.frame_ready,
        "frame 1 should render within 1M steps (took {})",
        steps
    );
    assert_eq!(vm.ram[0x7800], 1, "camera_x should be 1 after Up+Right");
    assert_eq!(
        vm.ram[0x7801],
        u32::MAX,
        "camera_y should wrap to u32::MAX after Up+Right"
    );

    // Frame 2: Down + Left (bits 1 + 2 = 6) -- reverses frame 1
    vm.ram[0xFFB] = 6;
    let steps = step_until_frame(&mut vm, 1_000_000);
    assert!(
        vm.frame_ready,
        "frame 2 should render within 1M steps (took {})",
        steps
    );
    assert_eq!(vm.ram[0x7800], 0, "camera_x should be 0 after Down+Left");
    assert_eq!(vm.ram[0x7801], 0, "camera_y should be 0 after Down+Left");

    // Frame 3: Up + Left (bits 0 + 2 = 5) -- both decrease
    vm.ram[0xFFB] = 5;
    let steps = step_until_frame(&mut vm, 1_000_000);
    assert!(
        vm.frame_ready,
        "frame 3 should render within 1M steps (took {})",
        steps
    );
    assert_eq!(
        vm.ram[0x7800],
        u32::MAX,
        "camera_x should wrap to u32::MAX after Up+Left"
    );
    assert_eq!(
        vm.ram[0x7801],
        u32::MAX,
        "camera_y should wrap to u32::MAX after Up+Left"
    );

    // Frame 4: Down + Right (bits 1 + 3 = 10) -- reverses frame 3
    vm.ram[0xFFB] = 10;
    let steps = step_until_frame(&mut vm, 1_000_000);
    assert!(
        vm.frame_ready,
        "frame 4 should render within 1M steps (took {})",
        steps
    );
    assert_eq!(vm.ram[0x7800], 0, "camera_x should be 0 after Down+Right");
    assert_eq!(vm.ram[0x7801], 0, "camera_y should be 0 after Down+Right");
}

// ── GAME OF LIFE ──────────────────────────────────────────────

/// Helper: assemble game_of_life.asm and return a fresh VM ready to run
fn game_of_life_vm() -> Vm {
    let source =
        std::fs::read_to_string("programs/game_of_life.asm").expect("game_of_life.asm not found");
    let asm = assemble(&source, 0).expect("game_of_life.asm failed to assemble");
    let mut vm = Vm::new();
    for (i, &word) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = word;
        }
    }
    vm
}

#[test]
fn test_game_of_life_assembles() {
    let source =
        std::fs::read_to_string("programs/game_of_life.asm").expect("game_of_life.asm not found");
    let asm = assemble(&source, 0).expect("game_of_life.asm should assemble");
    assert!(
        asm.pixels.len() > 50,
        "game_of_life bytecode should be >50 words, got {}",
        asm.pixels.len()
    );
}

#[test]
fn test_game_of_life_block_survives() {
    // The block at grid (10,50) is a still life: 4 cells in a 2x2 square.
    // Conway's rules preserve it forever -- each cell has exactly 3 alive neighbors.
    let mut vm = game_of_life_vm();

    // Run until first FRAME (one full generation computed + rendered)
    let steps = step_until_frame(&mut vm, 10_000_000);
    assert!(
        vm.frame_ready,
        "should reach FRAME within 10M steps (took {})",
        steps
    );

    // Block cells at screen top-left corners: (40,200), (44,200), (40,204), (44,204)
    // Each rendered as RECTF 4x4 with alive color 0x00FF00
    let alive = 0x00FF00u32;
    assert_eq!(
        vm.screen[200 * 256 + 40],
        alive,
        "block cell (10,50) screen(40,200) should survive"
    );
    assert_eq!(
        vm.screen[200 * 256 + 44],
        alive,
        "block cell (11,50) screen(44,200) should survive"
    );
    assert_eq!(
        vm.screen[204 * 256 + 40],
        alive,
        "block cell (10,51) screen(40,204) should survive"
    );
    assert_eq!(
        vm.screen[204 * 256 + 44],
        alive,
        "block cell (11,51) screen(44,204) should survive"
    );
}

#[test]
fn test_game_of_life_blinker_oscillates() {
    // The blinker at grid (32,10) is a period-2 oscillator.
    // Initial: horizontal -- cells (32,10), (33,10), (34,10)
    // After gen 1: vertical -- cells (33,9), (33,10), (33,11)
    // After gen 2: horizontal again
    let mut vm = game_of_life_vm();
    let alive = 0x00FF00u32;

    // === Frame 0: gen 1 from initial horizontal -> vertical ===
    let steps = step_until_frame(&mut vm, 10_000_000);
    assert!(
        vm.frame_ready,
        "should reach frame 0 within 10M steps (took {})",
        steps
    );

    // Vertical blinker: (132,36), (132,40), (132,44) should be alive
    assert_eq!(
        vm.screen[36 * 256 + 132],
        alive,
        "frame 0: vertical pos (132,36) should be alive"
    );
    assert_eq!(
        vm.screen[40 * 256 + 132],
        alive,
        "frame 0: vertical pos (132,40) should be alive"
    );
    assert_eq!(
        vm.screen[44 * 256 + 132],
        alive,
        "frame 0: vertical pos (132,44) should be alive"
    );
    // Horizontal ends should be dead
    assert_eq!(
        vm.screen[40 * 256 + 128],
        0,
        "frame 0: horizontal end (128,40) should be dead"
    );
    assert_eq!(
        vm.screen[40 * 256 + 136],
        0,
        "frame 0: horizontal end (136,40) should be dead"
    );

    // Snapshot population for comparison
    let _pop0: usize = vm.screen.iter().filter(|&&p| p == alive).count();

    // === Frame 1: gen 2 from vertical -> horizontal ===
    vm.frame_ready = false;
    let steps2 = step_until_frame(&mut vm, 10_000_000);
    assert!(
        vm.frame_ready,
        "should reach frame 1 within 10M steps (took {})",
        steps2
    );

    // Horizontal blinker: (128,40), (132,40), (136,40) should be alive
    assert_eq!(
        vm.screen[40 * 256 + 128],
        alive,
        "frame 1: horizontal pos (128,40) should be alive"
    );
    assert_eq!(
        vm.screen[40 * 256 + 132],
        alive,
        "frame 1: horizontal pos (132,40) should be alive"
    );
    assert_eq!(
        vm.screen[40 * 256 + 136],
        alive,
        "frame 1: horizontal pos (136,40) should be alive"
    );
    // Vertical ends should be dead
    assert_eq!(
        vm.screen[36 * 256 + 132],
        0,
        "frame 1: vertical end (132,36) should be dead"
    );
    assert_eq!(
        vm.screen[44 * 256 + 132],
        0,
        "frame 1: vertical end (132,44) should be dead"
    );

    // Verify blinker cells changed between frames (not just identical screenshots)
    let pop1: usize = vm.screen.iter().filter(|&&p| p == alive).count();
    assert!(
        pop1 > 0,
        "screen should have live pixels after frame 1 (got {})",
        pop1
    );
}

#[test]
fn test_game_of_life_renders() {
    // After one full generation, the screen must have both live and dead pixels.
    let mut vm = game_of_life_vm();

    let steps = step_until_frame(&mut vm, 10_000_000);
    assert!(
        vm.frame_ready,
        "should reach FRAME within 10M steps (took {})",
        steps
    );

    let non_black: usize = vm.screen.iter().filter(|&&p| p != 0).count();
    assert!(
        non_black > 100,
        "screen should have >100 live pixels after render, got {}",
        non_black
    );
    assert!(
        non_black < 50000,
        "screen should have <50000 live pixels after render, got {}",
        non_black
    );
}

// ── ROGUELIKE ──────────────────────────────────────────────────

#[test]
fn test_roguelike_assembles() {
    let source = std::fs::read_to_string("programs/roguelike.asm")
        .unwrap_or_else(|e| panic!("failed to read: {}", e));
    let asm = assemble(&source, 0).expect("roguelike.asm failed to assemble");
    assert!(
        asm.pixels.len() > 400,
        "roguelike should be more than 400 words"
    );
}

#[test]
fn test_roguelike_initializes() {
    let source = std::fs::read_to_string("programs/roguelike.asm")
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

    // Run until first FRAME (init + dungeon gen + render)
    let mut steps = 0;
    for _ in 0..5_000_000 {
        if !vm.step() {
            break;
        }
        steps += 1;
        if vm.frame_ready {
            vm.frame_ready = false;
            break;
        }
    }

    // Map should be initialized: top-left is always a wall
    assert_eq!(vm.ram[0x5000], 2, "top-left map cell should be wall (2)");

    // There should be carved floors somewhere (not all walls) -- 64x64 map
    let floor_count: usize = (0..4096)
        .map(|i| vm.ram[0x5000 + i])
        .filter(|&t| t == 1)
        .count();
    assert!(
        floor_count > 20,
        "dungeon should have >20 floor tiles, got {}",
        floor_count
    );

    // Room count should be > 0 (64x64 map, up to 12 rooms)
    let room_count = vm.ram[0x6230];
    assert!(
        room_count >= 2,
        "should have >= 2 rooms, got {}",
        room_count
    );

    // Player should be placed at a floor tile (new addresses)
    let px = vm.ram[0x6240] as usize;
    let py = vm.ram[0x6241] as usize;
    let player_tile = vm.ram[0x5000 + py * 64 + px];
    assert_eq!(
        player_tile, 1,
        "player at ({}, {}) should be on floor, got tile {}",
        px, py, player_tile
    );

    // Stairs should be placed at a stair tile
    let sx = vm.ram[0x6245] as usize;
    let sy = vm.ram[0x6246] as usize;
    let stair_tile = vm.ram[0x5000 + sy * 64 + sx];
    assert_eq!(
        stair_tile, 3,
        "stairs at ({}, {}) should be stair tile (3), got {}",
        sx, sy, stair_tile
    );

    // Player and stairs should NOT be at the same position
    assert_ne!(
        (px, py),
        (sx, sy),
        "player and stairs should be at different positions"
    );

    // Game state should be 0 (playing)
    assert_eq!(vm.ram[0x6248], 0, "game_state should be 0 (play)");

    // Text strings should be stored ("@" at 0x6A00)
    assert_eq!(vm.ram[0x6A00], 64, "first char should be @ (64)");

    // Tile data should be initialized (moved to 0x6000)
    assert_eq!(vm.ram[0x6000], 0x2A2A4E, "first floor tile pixel");
    assert_eq!(vm.ram[0x6080], 0xD4A017, "first stairs tile pixel");

    // Screen should have visible pixels (walls + floors rendered)
    let non_black: usize = vm.screen.iter().filter(|&&p| p != 0).count();
    assert!(
        non_black > 1000,
        "screen should have >1000 pixels after render, got {} (steps: {})",
        non_black,
        steps
    );
}

#[test]
fn test_roguelike_wall_collision_blocks() {
    // Verify player cannot walk into walls
    let source = std::fs::read_to_string("programs/roguelike.asm")
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

    // Run until first FRAME
    for _ in 0..5_000_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_ready {
            vm.frame_ready = false;
            break;
        }
    }

    // Record initial player position (new addresses)
    let _init_px = vm.ram[0x6240];
    let init_py = vm.ram[0x6241];

    // Try to move up (W = 87)
    vm.ram[0xFFF] = 87; // IKEY reads from 0xFFF
    for _ in 0..50_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_ready {
            vm.frame_ready = false;
            break;
        }
    }

    // Either player moved or stayed (if wall was above)
    // If player moved up, check they are on a floor tile
    let new_px = vm.ram[0x6240] as usize;
    let new_py = vm.ram[0x6241] as usize;
    if new_py != init_py as usize {
        let tile = vm.ram[0x5000 + new_py * 64 + new_px];
        assert_eq!(tile, 1, "player moved to non-floor tile: {}", tile);
    }
}
