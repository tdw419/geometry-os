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
    assert_eq!(vm.ram[0x5000], 0xFFFFFFFF, "top border row should be all walls");

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


// ── INFINITE MAP ──────────────────────────────────────────────

/// Helper: assemble infinite_map.asm, load into a fresh VM, return it.
/// The VM is ready to step but has not been run yet.
fn infinite_map_vm() -> Vm {
    let source = std::fs::read_to_string("programs/infinite_map.asm")
        .expect("infinite_map.asm not found");
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
        if vm.frame_ready { return i; }
        if !vm.step() { return i; }
    }
    max_steps
}

#[test]
fn test_infinite_map_assembles() {
    // Requirement: infinite_map.asm assembles without errors and produces bytecode.
    let source = std::fs::read_to_string("programs/infinite_map.asm")
        .expect("infinite_map.asm not found");
    let asm = assemble(&source, 0).expect("infinite_map.asm should assemble");
    assert!(!asm.pixels.is_empty(), "should produce non-empty bytecode");
    // The program is ~530 lines of asm; expect a substantial bytecode output.
    assert!(asm.pixels.len() > 500, "bytecode should be >500 words, got {}", asm.pixels.len());
}

#[test]
fn test_infinite_map_runs_and_renders() {
    // Requirement: the program runs to completion of a frame and renders non-black pixels.
    let mut vm = infinite_map_vm();

    // No key input -- camera stays at (0,0)
    vm.ram[0xFFB] = 0;

    let steps = step_until_frame(&mut vm, 1_000_000);
    assert!(vm.frame_ready, "should reach FRAME within 1M steps (took {})", steps);

    // Screen should have rendered terrain -- not all black.
    let non_black: usize = vm.screen.iter().filter(|&&p| p != 0).count();
    assert!(non_black > 0, "screen should have non-black pixels after rendering, got 0/{}", 256 * 256);

    // With 64x64 tiles covering the full 256x256 screen, nearly all pixels should be colored.
    // Water at (0,0) still produces non-black blue pixels.
    assert!(non_black > 50000,
        "most of the screen should be colored, got {}/{} non-black pixels",
        non_black, 256 * 256);
}

#[test]
fn test_infinite_map_camera_moves_on_key_input() {
    // Requirement: camera moves when arrow keys are pressed.
    let mut vm = infinite_map_vm();

    // --- Frame 1: press Right (bit 3 = 8) ---
    vm.ram[0xFFB] = 8;
    let steps = step_until_frame(&mut vm, 1_000_000);
    assert!(vm.frame_ready, "frame 1 should render within 1M steps (took {})", steps);
    assert_eq!(vm.ram[0x7800], 1, "camera_x should be 1 after pressing Right");
    assert_eq!(vm.ram[0x7801], 0, "camera_y should be 0 (no vertical input)");

    // --- Frame 2: press Down (bit 1 = 2) ---
    vm.ram[0xFFB] = 2;
    let steps = step_until_frame(&mut vm, 1_000_000);
    assert!(vm.frame_ready, "frame 2 should render within 1M steps (took {})", steps);
    assert_eq!(vm.ram[0x7800], 1, "camera_x should still be 1");
    assert_eq!(vm.ram[0x7801], 1, "camera_y should be 1 after pressing Down");

    // --- Frame 3: press Up+Left (bits 0+2 = 5) ---
    vm.ram[0xFFB] = 5;
    let steps = step_until_frame(&mut vm, 1_000_000);
    assert!(vm.frame_ready, "frame 3 should render within 1M steps (took {})", steps);
    assert_eq!(vm.ram[0x7800], 0, "camera_x should be 0 after pressing Left");
    assert_eq!(vm.ram[0x7801], 0, "camera_y should be 0 after pressing Up");

    // --- Frame 4: no keys, camera stays ---
    vm.ram[0xFFB] = 0;
    let steps = step_until_frame(&mut vm, 1_000_000);
    assert!(vm.frame_ready, "frame 4 should render within 1M steps (took {})", steps);
    assert_eq!(vm.ram[0x7800], 0, "camera_x should stay 0 with no input");
    assert_eq!(vm.ram[0x7801], 0, "camera_y should stay 0 with no input");

    // Frame counter should have incremented each frame.
    assert!(vm.ram[0x7802] >= 4, "frame_counter should be >= 4, got {}", vm.ram[0x7802]);
}

#[test]
fn test_infinite_map_camera_moves_multiple_steps() {
    // Requirement: holding a direction for multiple frames accumulates movement.
    let mut vm = infinite_map_vm();

    // Hold Right for 5 frames.
    for frame in 1..=5 {
        vm.ram[0xFFB] = 8; // Right
        let steps = step_until_frame(&mut vm, 1_000_000);
        assert!(vm.frame_ready, "frame {} should render (took {} steps)", frame, steps);
    }
    assert_eq!(vm.ram[0x7800], 5, "camera_x should be 5 after 5 Right presses");
    assert_eq!(vm.ram[0x7801], 0, "camera_y should still be 0");

    // Now hold Down+Right for 3 frames.
    for frame in 6..=8 {
        vm.ram[0xFFB] = 8 | 2; // Right + Down = 10
        let steps = step_until_frame(&mut vm, 1_000_000);
        assert!(vm.frame_ready, "frame {} should render (took {} steps)", frame, steps);
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
    let same: usize = screen_origin.iter().zip(screen_far.iter())
        .filter(|(a, b)| a == b).count();
    let total = 256 * 256;
    // At most 10% of pixels should be identical between two distant camera positions.
    assert!(same < total / 10,
        "screens at (0,0) vs (50,50) should be mostly different, but {}/{} pixels match", same, total);
}

#[test]
fn test_infinite_map_diagonal_keys_move_camera() {
    // Requirement: dedicated diagonal key bits (4-7) move the camera diagonally.
    let mut vm = infinite_map_vm();

    // --- Frame 1: press Up+Right diagonal (bit 4 = 16) ---
    vm.ram[0xFFB] = 16;
    let steps = step_until_frame(&mut vm, 1_000_000);
    assert!(vm.frame_ready, "frame 1 should render within 1M steps (took {})", steps);
    assert_eq!(vm.ram[0x7800], 1, "camera_x should be 1 after Up+Right diagonal");
    assert_eq!(vm.ram[0x7801], u32::MAX, "camera_y should wrap to u32::MAX after Up+Right diagonal");

    // --- Frame 2: press Down+Right diagonal (bit 5 = 32) ---
    vm.ram[0xFFB] = 32;
    let steps = step_until_frame(&mut vm, 1_000_000);
    assert!(vm.frame_ready, "frame 2 should render within 1M steps (took {})", steps);
    assert_eq!(vm.ram[0x7800], 2, "camera_x should be 2 after Down+Right diagonal");
    assert_eq!(vm.ram[0x7801], 0, "camera_y should be 0 after Down+Right diagonal");

    // --- Frame 3: press Down+Left diagonal (bit 6 = 64) ---
    vm.ram[0xFFB] = 64;
    let steps = step_until_frame(&mut vm, 1_000_000);
    assert!(vm.frame_ready, "frame 3 should render within 1M steps (took {})", steps);
    assert_eq!(vm.ram[0x7800], 1, "camera_x should be 1 after Down+Left diagonal");
    assert_eq!(vm.ram[0x7801], 1, "camera_y should be 1 after Down+Left diagonal");

    // --- Frame 4: press Up+Left diagonal (bit 7 = 128) ---
    vm.ram[0xFFB] = 128;
    let steps = step_until_frame(&mut vm, 1_000_000);
    assert!(vm.frame_ready, "frame 4 should render within 1M steps (took {})", steps);
    assert_eq!(vm.ram[0x7800], 0, "camera_x should be 0 after Up+Left diagonal");
    assert_eq!(vm.ram[0x7801], 0, "camera_y should be 0 after Up+Left diagonal");
}

#[test]
fn test_infinite_map_diagonal_accumulates() {
    // Requirement: diagonal keys accumulate over multiple frames.
    let mut vm = infinite_map_vm();

    // Hold Down+Right diagonal for 3 frames.
    for frame in 1..=3 {
        vm.ram[0xFFB] = 32; // Down+Right
        let steps = step_until_frame(&mut vm, 1_000_000);
        assert!(vm.frame_ready, "frame {} should render (took {} steps)", frame, steps);
    }
    assert_eq!(vm.ram[0x7800], 3, "camera_x should be 3 after 3 Down+Right diagonals");
    assert_eq!(vm.ram[0x7801], 3, "camera_y should be 3 after 3 Down+Right diagonals");

    // Hold Up+Left diagonal for 2 frames to partially reverse.
    for frame in 4..=5 {
        vm.ram[0xFFB] = 128; // Up+Left
        let steps = step_until_frame(&mut vm, 1_000_000);
        assert!(vm.frame_ready, "frame {} should render (took {} steps)", frame, steps);
    }
    assert_eq!(vm.ram[0x7800], 1, "camera_x should be 3-2=1 after 2 Up+Left diagonals");
    assert_eq!(vm.ram[0x7801], 1, "camera_y should be 3-2=1 after 2 Up+Left diagonals");
}

#[test]
fn test_infinite_map_cardinal_and_diagonal_combined() {
    // Requirement: diagonal bits stack with cardinal bits for faster movement.
    // Pressing Right (bit 3) + Down+Right diagonal (bit 5) should move x+2, y+1.
    let mut vm = infinite_map_vm();

    vm.ram[0xFFB] = 8 | 32; // Right + Down+Right diagonal = 40
    let steps = step_until_frame(&mut vm, 1_000_000);
    assert!(vm.frame_ready, "frame should render within 1M steps (took {})", steps);
    assert_eq!(vm.ram[0x7800], 2, "camera_x should be 2 (Right + Down+Right diagonal)");
    assert_eq!(vm.ram[0x7801], 1, "camera_y should be 1 (Down+Right diagonal only)");
}
