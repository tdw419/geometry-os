use super::*;

// ── Phase 124: Window Pipeline Improvements ──────────────────────

#[test]
fn test_title_bar_offset_content_below_bar() {
    let mut vm = Vm::new();
    vm.regs[1] = 20; // x
    vm.regs[2] = 20; // y
    vm.regs[3] = 64; // w
    vm.regs[4] = 64; // h
    vm.regs[5] = 0; // no title
    vm.regs[6] = 0; // op = create
    vm.ram[0] = 0x94;
    vm.ram[1] = 6;
    vm.pc = 0;
    vm.halted = false;
    vm.step();
    let win_id = vm.regs[0];
    assert!(win_id > 0, "window should be created");

    // Write a red pixel at (0, 0) in the window buffer
    vm.regs[7] = win_id;
    vm.regs[8] = 0;
    vm.regs[9] = 0;
    vm.regs[10] = 0xFF0000;
    vm.ram[2] = 0x95;
    vm.ram[3] = 7;
    vm.ram[4] = 8;
    vm.ram[5] = 9;
    vm.ram[6] = 10;
    vm.pc = 2;
    vm.halted = false;
    vm.step();

    // Write a blue pixel at (5, 5) in the window buffer
    vm.regs[7] = win_id;
    vm.regs[8] = 5;
    vm.regs[9] = 5;
    vm.regs[10] = 0x0000FF;
    vm.ram[7] = 0x95;
    vm.ram[8] = 7;
    vm.ram[9] = 8;
    vm.ram[10] = 9;
    vm.ram[11] = 10;
    vm.pc = 7;
    vm.halted = false;
    vm.step();

    // FRAME to blit
    vm.ram[12] = 0x02;
    vm.pc = 12;
    vm.halted = false;
    vm.step();

    // Pixel at (0,0) in window -> screen (20, 20+12) = (20, 32) with title bar offset
    assert_eq!(
        vm.screen[32 * 256 + 20],
        0xFF0000,
        "red pixel at (0,0) should be at screen (20, 32) with title bar offset"
    );
    assert_eq!(
        vm.screen[37 * 256 + 25],
        0x0000FF,
        "blue pixel at (5,5) should be at screen (25, 37) with title bar offset"
    );

    // Verify title bar area is NOT zero (has bg/text)
    // Check a pixel in the title bar area that is background color
    // Title bar spans y=20..31, x=20..83 for a 64-wide window
    // Row y=20 is the top highlight edge (0x5A5A7A), row y=21 is regular bg.
    // Check row y=21 at x=60 (middle of title bar, after title text)
    let title_bg_pixel = vm.screen[21 * 256 + 60];
    assert_eq!(
        title_bg_pixel, 0x3A3A5A,
        "title bar bg should be at (60, 21), got {:X}",
        title_bg_pixel
    );

    // Verify top row has highlight edge (clickable affordance)
    let highlight_pixel = vm.screen[20 * 256 + 60];
    assert_eq!(
        highlight_pixel, 0x5A5A7A,
        "title bar top edge should be highlight color at (60, 20), got {:X}",
        highlight_pixel
    );
}

#[test]
fn test_winsys_hittest_title_bar_with_offset() {
    let mut vm = Vm::new();
    vm.regs[1] = 50;
    vm.regs[2] = 50;
    vm.regs[3] = 64;
    vm.regs[4] = 64;
    vm.regs[5] = 0;
    vm.regs[6] = 0;
    vm.ram[0] = 0x94;
    vm.ram[1] = 6;
    vm.pc = 0;
    vm.halted = false;
    vm.step();
    let win_id = vm.regs[0];

    // Title bar hit
    vm.mouse_x = 55;
    vm.mouse_y = 55;
    vm.regs[6] = 4;
    vm.ram[2] = 0x94;
    vm.ram[3] = 6;
    vm.pc = 2;
    vm.halted = false;
    vm.step();

    assert_eq!(vm.regs[0], win_id, "hittest should detect window");
    assert_eq!(
        vm.regs[1], 1,
        "hittest should detect title bar (hit_type=1)"
    );

    // Body hit (below title bar)
    vm.mouse_x = 55;
    vm.mouse_y = 67;
    vm.regs[6] = 4;
    vm.ram[2] = 0x94;
    vm.ram[3] = 6;
    vm.pc = 2;
    vm.halted = false;
    vm.step();

    assert_eq!(vm.regs[0], win_id, "hittest should detect window body");
    assert_eq!(vm.regs[1], 2, "hittest should detect body (hit_type=2)");
}

#[test]
fn test_winsys_hittest_close_button() {
    let mut vm = Vm::new();
    vm.regs[1] = 30;
    vm.regs[2] = 30;
    vm.regs[3] = 64;
    vm.regs[4] = 48;
    vm.regs[5] = 0;
    vm.regs[6] = 0;
    vm.ram[0] = 0x94;
    vm.ram[1] = 6;
    vm.pc = 0;
    vm.halted = false;
    vm.step();
    let win_id = vm.regs[0];

    // Close button at top-right of title bar
    vm.mouse_x = 88;
    vm.mouse_y = 35;

    vm.regs[6] = 4;
    vm.ram[2] = 0x94;
    vm.ram[3] = 6;
    vm.pc = 2;
    vm.halted = false;
    vm.step();

    assert_eq!(vm.regs[0], win_id, "hittest should detect window");
    assert_eq!(
        vm.regs[1], 3,
        "hittest should detect close button (hit_type=3)"
    );
}

#[test]
fn test_bring_to_front_updates_z_order() {
    let mut vm = Vm::new();

    // Window 1
    vm.regs[1] = 10;
    vm.regs[2] = 10;
    vm.regs[3] = 40;
    vm.regs[4] = 40;
    vm.regs[5] = 0;
    vm.regs[6] = 0;
    vm.ram[0] = 0x94;
    vm.ram[1] = 6;
    vm.pc = 0;
    vm.halted = false;
    vm.step();
    let id1 = vm.regs[0];

    // Window 2
    vm.regs[1] = 15;
    vm.regs[2] = 15;
    vm.regs[3] = 40;
    vm.regs[4] = 40;
    vm.regs[5] = 0;
    vm.regs[6] = 0;
    vm.ram[2] = 0x94;
    vm.ram[3] = 6;
    vm.pc = 2;
    vm.halted = false;
    vm.step();
    let id2 = vm.regs[0];

    let z1 = vm
        .windows
        .iter()
        .find(|w| w.id == id1)
        .map(|w| w.z_order)
        .unwrap();
    let z2 = vm
        .windows
        .iter()
        .find(|w| w.id == id2)
        .map(|w| w.z_order)
        .unwrap();
    assert!(
        z2 > z1,
        "window 2 z_order ({}) should be > window 1 ({})",
        z2,
        z1
    );

    // Bring window 1 to front
    vm.regs[0] = id1;
    vm.regs[6] = 2;
    vm.ram[4] = 0x94;
    vm.ram[5] = 6;
    vm.pc = 4;
    vm.halted = false;
    vm.step();

    let z1_new = vm
        .windows
        .iter()
        .find(|w| w.id == id1)
        .map(|w| w.z_order)
        .unwrap();
    assert!(
        z1_new > z2,
        "window 1 z_order ({}) should now be > window 2 ({})",
        z1_new,
        z2
    );
}

#[test]
fn test_title_bar_renders_as_clickable_region() {
    // Verify title bar has visual affordance for clickability:
    // 1. Top row has highlight edge (lighter = raised button look)
    // 2. Left column has highlight edge
    // 3. Right column has shadow edge (darker = 3D raised effect)
    // 4. Close button has a distinct background rectangle
    let mut vm = Vm::new();
    vm.regs[1] = 30; // x
    vm.regs[2] = 40; // y
    vm.regs[3] = 80; // w
    vm.regs[4] = 60; // h
    vm.regs[5] = 0; // no title
    vm.regs[6] = 0; // op = create
    vm.ram[0] = 0x94;
    vm.ram[1] = 6;
    vm.pc = 0;
    vm.halted = false;
    vm.step();
    let _win_id = vm.regs[0];
    assert!(_win_id > 0, "window should be created");

    // FRAME to blit
    vm.ram[2] = 0x02;
    vm.pc = 2;
    vm.halted = false;
    vm.step();

    let highlight: u32 = 0x5A5A7A;
    let shadow: u32 = 0x1A1A2A;
    let close_bg: u32 = 0x4A2A2A;

    // Top edge (y=40) at x=50 should be highlight
    assert_eq!(
        vm.screen[40 * 256 + 50],
        highlight,
        "top row of title bar should be highlight (clickable affordance)"
    );

    // Left edge (x=30) at y=45 should be highlight
    assert_eq!(
        vm.screen[45 * 256 + 30],
        highlight,
        "left column of title bar should be highlight (clickable affordance)"
    );

    // Right edge (x=109) at y=45 should be shadow
    assert_eq!(
        vm.screen[45 * 256 + 109],
        shadow,
        "right column of title bar should be shadow (3D raised effect)"
    );

    // Close button background area: near top-right of title bar
    // x = 30 + 80 - 2 - 8 - 1 = 99, y = 40 + 1 = 41
    assert_eq!(
        vm.screen[41 * 256 + 100],
        close_bg,
        "close button area should have distinct background (clickable indicator)"
    );

    // Middle of title bar (not on edges or text) should be regular bg color
    // y=44 is mid-bar, x=35 is near left edge but past the highlight column
    // Actually x=35 is right after "W" char which starts at x=32, so use x=70 past title text
    assert_eq!(
        vm.screen[44 * 256 + 70],
        0x3A3A5A,
        "middle of title bar (past text) should be regular active bg color"
    );
}

// ── Phase 124: MOUSEQ Window Routing Tests ───────────────────────────

#[test]
fn test_mouseq_routes_to_window_owner_screen_space() {
    // When a process owns a screen-space window, MOUSEQ returns coords
    // relative to the window's top-left corner.
    let mut vm = Vm::new();

    // Create a window at screen position (40, 30), size 64x48, owned by PID 3
    let win = Window::new(1, 40, 30, 64, 48, 0, 3);
    vm.windows.push(win);

    // Set current process to PID 3 (the window owner)
    vm.current_pid = 3;

    // Move mouse to global position (50, 40) -- which is (10, 10) relative to the window
    vm.push_mouse(50, 40);

    // MOUSEQ r5
    vm.ram[0] = 0x85;
    vm.ram[1] = 5;
    vm.ram[2] = 0x00; // HALT
    vm.pc = 0;
    vm.halted = false;
    vm.step();

    assert_eq!(
        vm.regs[5], 10,
        "MOUSEQ x should be window-relative (50 - 40 = 10)"
    );
    assert_eq!(
        vm.regs[6], 10,
        "MOUSEQ y should be window-relative (40 - 30 = 10)"
    );
}

#[test]
fn test_mouseq_routes_to_window_owner_world_space() {
    // When a process owns a world-space window, MOUSEQ computes screen position
    // from camera + world coords, then translates.
    let mut vm = Vm::new();

    // Camera at (5, 3) in tile coords
    vm.ram[0x7800] = 5;
    vm.ram[0x7801] = 3;

    // World-space window at tile (10, 8), size 64x48
    // Screen position = (10 - 5) * 8 = 40, (8 - 3) * 8 = 40
    let win = Window::new_world(1, 10, 8, 64, 48, 0, 5);
    vm.windows.push(win);

    // Set current process to PID 5 (the window owner)
    vm.current_pid = 5;

    // Move mouse to global position (60, 55)
    // Relative to window: (60 - 40, 55 - 40) = (20, 15)
    vm.push_mouse(60, 55);

    // MOUSEQ r10
    vm.ram[0] = 0x85;
    vm.ram[1] = 10;
    vm.ram[2] = 0x00; // HALT
    vm.pc = 0;
    vm.halted = false;
    vm.step();

    assert_eq!(vm.regs[10], 20, "MOUSEQ x should be world-window-relative");
    assert_eq!(vm.regs[11], 15, "MOUSEQ y should be world-window-relative");
}

#[test]
fn test_mouseq_global_when_no_window() {
    // Process with no window gets global coordinates (unchanged behavior)
    let mut vm = Vm::new();
    vm.current_pid = 7; // PID 7 has no window
    vm.push_mouse(100, 200);

    // MOUSEQ r5
    vm.ram[0] = 0x85;
    vm.ram[1] = 5;
    vm.ram[2] = 0x00; // HALT
    vm.pc = 0;
    vm.halted = false;
    vm.step();

    assert_eq!(vm.regs[5], 100, "MOUSEQ x should be global when no window");
    assert_eq!(vm.regs[6], 200, "MOUSEQ y should be global when no window");
}

#[test]
fn test_mouseq_global_when_pid0() {
    // Main/kernel context (PID 0) gets global coordinates
    let mut vm = Vm::new();
    vm.current_pid = 0;
    vm.push_mouse(42, 84);

    // MOUSEQ r1
    vm.ram[0] = 0x85;
    vm.ram[1] = 1;
    vm.ram[2] = 0x00; // HALT
    vm.pc = 0;
    vm.halted = false;
    vm.step();

    assert_eq!(vm.regs[1], 42, "MOUSEQ x should be global for PID 0");
    assert_eq!(vm.regs[2], 84, "MOUSEQ y should be global for PID 0");
}

#[test]
fn test_mouseq_clamps_negative_to_zero() {
    // Mouse position outside window (above/left) should clamp to 0
    let mut vm = Vm::new();

    // Window at (100, 80)
    let win = Window::new(1, 100, 80, 64, 48, 0, 2);
    vm.windows.push(win);
    vm.current_pid = 2;

    // Mouse at (50, 50) -- before the window
    vm.push_mouse(50, 50);

    // MOUSEQ r10
    vm.ram[0] = 0x85;
    vm.ram[1] = 10;
    vm.ram[2] = 0x00; // HALT
    vm.pc = 0;
    vm.halted = false;
    vm.step();

    assert_eq!(
        vm.regs[10], 0,
        "MOUSEQ x should clamp to 0 when mouse is left of window"
    );
    assert_eq!(
        vm.regs[11], 0,
        "MOUSEQ y should clamp to 0 when mouse is above window"
    );
}

#[test]
fn test_mouseq_inactive_window_uses_global() {
    // Inactive window should not intercept -- process gets global coords
    let mut vm = Vm::new();

    let mut win = Window::new(1, 40, 30, 64, 48, 0, 3);
    win.active = false;
    vm.windows.push(win);
    vm.current_pid = 3;
    vm.push_mouse(50, 40);

    // MOUSEQ r5
    vm.ram[0] = 0x85;
    vm.ram[1] = 5;
    vm.ram[2] = 0x00; // HALT
    vm.pc = 0;
    vm.halted = false;
    vm.step();

    assert_eq!(
        vm.regs[5], 50,
        "MOUSEQ x should be global for inactive window"
    );
    assert_eq!(
        vm.regs[6], 40,
        "MOUSEQ y should be global for inactive window"
    );
}

#[test]
fn test_mouse_routing() {
    // Verifies MOUSEQ events reach the correct window when multiple concurrent
    // windows exist. The routing is PID-based: MOUSEQ returns window-relative
    // coords for the window owned by current_pid, regardless of how many other
    // windows are registered.
    let mut vm = Vm::new();

    // Set up 3 overlapping active windows from different PIDs
    let win_a = Window::new(1, 10, 20, 64, 48, 0, 2); // PID 2 at screen (10,20)
    let win_b = Window::new(2, 100, 50, 80, 60, 0, 3); // PID 3 at screen (100,50)
    let win_c = Window::new(3, 200, 10, 48, 48, 0, 5); // PID 5 at screen (200,10)
    vm.windows.push(win_a);
    vm.windows.push(win_b);
    vm.windows.push(win_c);

    // Mouse at global (30, 40)
    vm.push_mouse(30, 40);

    // ── Case 1: PID 2 owns win_a at (10,20). Relative = (30-10, 40-20) = (20, 20)
    vm.current_pid = 2;
    vm.ram[0] = 0x85; // MOUSEQ r5
    vm.ram[1] = 5;
    vm.ram[2] = 0x00; // HALT
    vm.pc = 0;
    vm.halted = false;
    vm.step();
    assert_eq!(
        vm.regs[5], 20,
        "PID 2 MOUSEQ x should be relative to win_a (10,20)"
    );
    assert_eq!(
        vm.regs[6], 20,
        "PID 2 MOUSEQ y should be relative to win_a (10,20)"
    );

    // ── Case 2: Same mouse position, but PID 3 owns win_b at (100,50).
    // Mouse at (30,40) is OUTSIDE win_b, so should clamp.
    // translate_mouse_for_current_process: (30-100).max(0) = 0, (40-50).max(0) = 0
    vm.regs[5] = 0xFFFF;
    vm.regs[6] = 0xFFFF;
    vm.current_pid = 3;
    vm.ram[0] = 0x85; // MOUSEQ r5
    vm.ram[1] = 5;
    vm.ram[2] = 0x00; // HALT
    vm.pc = 0;
    vm.halted = false;
    vm.step();
    assert_eq!(
        vm.regs[5], 0,
        "PID 3 MOUSEQ x should clamp 0 (mouse left of win_b)"
    );
    assert_eq!(
        vm.regs[6], 0,
        "PID 3 MOUSEQ y should clamp 0 (mouse above win_b)"
    );

    // ── Case 3: Move mouse to (130, 70). PID 3 gets relative (130-100, 70-50) = (30, 20)
    vm.push_mouse(130, 70);
    vm.regs[5] = 0;
    vm.regs[6] = 0;
    vm.current_pid = 3;
    vm.ram[0] = 0x85;
    vm.ram[1] = 5;
    vm.ram[2] = 0x00;
    vm.pc = 0;
    vm.halted = false;
    vm.step();
    assert_eq!(vm.regs[5], 30, "PID 3 MOUSEQ x should be 30 (130 - 100)");
    assert_eq!(vm.regs[6], 20, "PID 3 MOUSEQ y should be 20 (70 - 50)");

    // ── Case 4: Same mouse (130,70), switch to PID 2 (win_a at 10,20).
    // Relative = (130-10, 70-20) = (120, 50)
    vm.regs[5] = 0;
    vm.regs[6] = 0;
    vm.current_pid = 2;
    vm.ram[0] = 0x85;
    vm.ram[1] = 5;
    vm.ram[2] = 0x00;
    vm.pc = 0;
    vm.halted = false;
    vm.step();
    assert_eq!(vm.regs[5], 120, "PID 2 MOUSEQ x should be 120 (130 - 10)");
    assert_eq!(vm.regs[6], 50, "PID 2 MOUSEQ y should be 50 (70 - 20)");

    // ── Case 5: PID 4 has no window. Gets global coords.
    vm.regs[5] = 0;
    vm.regs[6] = 0;
    vm.current_pid = 4;
    vm.ram[0] = 0x85;
    vm.ram[1] = 5;
    vm.ram[2] = 0x00;
    vm.pc = 0;
    vm.halted = false;
    vm.step();
    assert_eq!(
        vm.regs[5], 130,
        "PID 4 (no window) MOUSEQ x should be global 130"
    );
    assert_eq!(
        vm.regs[6], 70,
        "PID 4 (no window) MOUSEQ y should be global 70"
    );

    // ── Case 6: PID 5 owns win_c at (200,10). Mouse at (130,70) is outside (left).
    // (130-200).max(0) = 0, (70-10).max(0) = 60
    vm.regs[5] = 0;
    vm.regs[6] = 0;
    vm.current_pid = 5;
    vm.ram[0] = 0x85;
    vm.ram[1] = 5;
    vm.ram[2] = 0x00;
    vm.pc = 0;
    vm.halted = false;
    vm.step();
    assert_eq!(
        vm.regs[5], 0,
        "PID 5 MOUSEQ x should clamp 0 (mouse left of win_c)"
    );
    assert_eq!(vm.regs[6], 60, "PID 5 MOUSEQ y should be 60 (70 - 10)");

    // ── Case 7: Button state is preserved across all PID switches
    vm.push_mouse_button(2); // click
    vm.current_pid = 2;
    vm.ram[0] = 0x85;
    vm.ram[1] = 5;
    vm.ram[2] = 0x00;
    vm.pc = 0;
    vm.halted = false;
    vm.step();
    assert_eq!(vm.regs[7], 2, "MOUSEQ button should be 2 (click) for PID 2");
}
