use super::*;



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
