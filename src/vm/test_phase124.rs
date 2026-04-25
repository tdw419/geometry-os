use super::*;

// ── Phase 124: Window Pipeline Improvements ──────────────────────

#[test]
fn test_title_bar_offset_content_below_bar() {
    let mut vm = Vm::new();
    vm.regs[1] = 20; // x
    vm.regs[2] = 20; // y
    vm.regs[3] = 64; // w
    vm.regs[4] = 64; // h
    vm.regs[5] = 0;  // no title
    vm.regs[6] = 0;  // op = create
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
        vm.screen[32 * 256 + 20], 0xFF0000,
        "red pixel at (0,0) should be at screen (20, 32) with title bar offset"
    );
    assert_eq!(
        vm.screen[37 * 256 + 25], 0x0000FF,
        "blue pixel at (5,5) should be at screen (25, 37) with title bar offset"
    );

    // Verify title bar area is NOT zero (has bg/text)
    // Check a pixel in the title bar area that is background color
    // Title bar spans y=20..31, x=20..83 for a 64-wide window
    // Row y=20 (first title bar row), at x=60 (after the title text)
    let title_bg_pixel = vm.screen[20 * 256 + 60];
    assert_eq!(
        title_bg_pixel, 0x3A3A5A,
        "title bar bg should be at (60, 20), got {:X}",
        title_bg_pixel
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

    let z1 = vm.windows.iter().find(|w| w.id == id1).map(|w| w.z_order).unwrap();
    let z2 = vm.windows.iter().find(|w| w.id == id2).map(|w| w.z_order).unwrap();
    assert!(z2 > z1, "window 2 z_order ({}) should be > window 1 ({})", z2, z1);

    // Bring window 1 to front
    vm.regs[0] = id1;
    vm.regs[6] = 2;
    vm.ram[4] = 0x94;
    vm.ram[5] = 6;
    vm.pc = 4;
    vm.halted = false;
    vm.step();

    let z1_new = vm.windows.iter().find(|w| w.id == id1).map(|w| w.z_order).unwrap();
    assert!(z1_new > z2, "window 1 z_order ({}) should now be > window 2 ({})", z1_new, z2);
}
