use super::*;



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
    let asm = assemble(source, 0).expect("assembly should succeed");
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() { vm.ram[i] = v; }
    for _ in 0..100 { if !vm.step() { break; } }
    assert!(vm.halted);
    // RAM[0xFFA] should contain the process ID (1)
    assert_eq!(vm.ram[0xFFA], 1, "SPAWN should return PID 1");
    // One process should exist
    assert_eq!(vm.processes.len(), 1);
    assert_eq!(vm.processes[0].pid, 1);
    // With COW fork, child PC starts at the offset within the first shared page
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

    let asm = assemble(&source, 0).expect("assembly should succeed");
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
    let asm = assemble(source, 0).expect("assembly should succeed");
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() { vm.ram[i] = v; }
    for _ in 0..100 { if !vm.step() { break; } }
    assert!(vm.halted);
    // KILL should have returned 1 (success)
    assert_eq!(vm.ram[0xFFA], 1, "KILL should return 1 on success");
    // Child should be halted
    assert!(vm.processes[0].is_halted());
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
    let asm = assemble(source, 0).expect("assembly should succeed");
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() { vm.ram[i] = v; }
    // Run main process to completion
    for _ in 0..100 { if !vm.step() { break; } }
    assert!(vm.halted);
    assert_eq!(vm.processes.len(), 2);

    // Step child processes
    for _ in 0..100 {
        vm.step_all_processes();
        if vm.processes.iter().all(|p| p.is_halted()) {
            break;
        }
    }

    // Both children should be halted
    assert!(vm.processes[0].is_halted());
    assert!(vm.processes[1].is_halted());

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
        pc: 0, regs: [0; 32], state: geometry_os::vm::ProcessState::Ready, pid: 1, mode: geometry_os::vm::CpuMode::Kernel,
        page_dir: None, segfaulted: false,
        priority: 1, slice_remaining: 0, sleep_until: 0, yielded: false,
                kernel_stack: Vec::new(),
                msg_queue: Vec::new(),
                                exit_code: 0,
                                parent_pid: 0,
                                pending_signals: Vec::new(),
                                signal_handlers: [0; 4], vmas: Vec::new(), brk_pos: 0,
    });
    assert_eq!(vm.active_process_count(), 1);
    vm.processes.push(geometry_os::vm::SpawnedProcess {
        pc: 0, regs: [0; 32], state: geometry_os::vm::ProcessState::Zombie, pid: 2, mode: geometry_os::vm::CpuMode::Kernel,
        page_dir: None, segfaulted: false,
        priority: 1, slice_remaining: 0, sleep_until: 0, yielded: false,
                kernel_stack: Vec::new(),
                msg_queue: Vec::new(),
                                exit_code: 0,
                                parent_pid: 0,
                                pending_signals: Vec::new(),
                                signal_handlers: [0; 4], vmas: Vec::new(), brk_pos: 0,
    });
    assert_eq!(vm.active_process_count(), 1);
}

#[test]
fn test_spawn_assembles() {
    let source = "SPAWN r1\nKILL r2\nHALT";
    let asm = assemble(source, 0).expect("assembly should succeed");
    // SPAWN r1 = 0x4D, r1
    assert_eq!(asm.pixels[0], 0x4D);
    assert_eq!(asm.pixels[1], 1); // r1
    // KILL r2 = 0x4E, r2
    assert_eq!(asm.pixels[2], 0x4E);
    assert_eq!(asm.pixels[3], 2); // r2
    // HALT
    assert_eq!(asm.pixels[4], 0x00);
}



// === Copy-on-Write (COW) Fork Tests ===

#[test]
fn test_cow_fork_shares_physical_pages() {
    // After SPAWN, child should share parent's physical pages (not allocate new ones)
    // Use start_addr=0x1000 (page 4) to avoid conflicts with shared region at page 3
    let source = "
    LDI r1, 0x1000
    SPAWN r1
    HALT

    .org 0x1000
    HALT
    ";
    let asm = assemble(source, 0).expect("assembly should succeed");
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() { vm.ram[i] = v; }
    for _ in 0..100 { if !vm.step() { break; } }

    let pd = vm.processes[0].page_dir.as_ref().expect("operation should succeed");
    // With COW, child's virtual page 0 maps to parent's physical page 4 (0x1000/1024=4)
    assert_eq!(pd[0], 4, "child vpage 0 should share parent's phys page 4");
    assert_eq!(pd[1], 5, "child vpage 1 should share parent's phys page 5");
    // Ref count on shared pages should be >= 1 (child's reference)
    assert!(vm.page_ref_count[4] >= 1, "phys page 4 should have ref count >= 1");
    assert!(vm.page_ref_count[5] >= 1, "phys page 5 should have ref count >= 1");
    // COW flag should be set
    assert_ne!(vm.page_cow & (1u64 << 4), 0, "phys page 4 should be COW");
    assert_ne!(vm.page_cow & (1u64 << 5), 0, "phys page 5 should be COW");
}

#[test]
fn test_cow_write_triggers_page_copy() {
    // When a child writes to a shared (COW) page, it should get a private copy
    // Use start_addr=0x1000 (page 4) to avoid shared region at page 3
    let source = "
    LDI r1, 0x1000
    SPAWN r1
    HALT

    .org 0x1000
    LDI r2, 0xDEAD
    STORE r0, r2
    HALT
    ";
    let asm = assemble(source, 0).expect("assembly should succeed");
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() { vm.ram[i] = v; }

    // Run main to spawn child
    for _ in 0..100 { if !vm.step() { break; } }
    assert_eq!(vm.processes.len(), 1);

    let pd_before = vm.processes[0].page_dir.as_ref().expect("operation should succeed").clone();
    let shared_phys_page = pd_before[0]; // vpage 0 -> phys page 4 (0x1000/1024=4)

    // Run child to completion
    for _ in 0..100 {
        vm.step_all_processes();
        if vm.processes.iter().all(|p| p.is_halted()) { break; }
    }

    let pd_after = vm.processes[0].page_dir.as_ref().expect("operation should succeed");
    // After writing to vpage 0 (STORE r0, r2 where r0=0, virtual addr 0 -> vpage 0),
    // the child should have a NEW private physical page (COW resolved)
    assert_ne!(pd_after[0], shared_phys_page,
        "child should have a new private page after COW write");
    // The new page should NOT be COW
    assert_eq!(vm.page_cow & (1u64 << pd_after[0] as u64), 0,
        "new private page should not be COW");
}

#[test]
fn test_cow_isolation_between_children() {
    // Two children sharing the same physical page write different values.
    // Each should get its own private copy via COW.
    let source = "
    LDI r1, 0x1000
    SPAWN r1
    LDI r1, 0x1000
    SPAWN r1
    HALT

    .org 0x1000
    LDI r2, 0xAAAA
    STORE r0, r2
    HALT
    ";
    let asm = assemble(source, 0).expect("assembly should succeed");
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() { vm.ram[i] = v; }

    // Run main
    for _ in 0..100 { if !vm.step() { break; } }
    assert_eq!(vm.processes.len(), 2);

    // Run children to completion
    for _ in 0..200 {
        vm.step_all_processes();
        if vm.processes.iter().all(|p| p.is_halted()) { break; }
    }

    assert!(!vm.processes[0].segfaulted, "child 1 should not segfault");
    assert!(!vm.processes[1].segfaulted, "child 2 should not segfault");

    let pd1 = vm.processes[0].page_dir.as_ref().expect("operation should succeed");
    let pd2 = vm.processes[1].page_dir.as_ref().expect("operation should succeed");

    // After COW resolution, children should have DIFFERENT physical pages
    // (they both wrote to the same shared page, triggering separate copies)
    assert_ne!(pd1[0], pd2[0],
        "children should have different physical pages after COW writes");
}

#[test]
fn test_cow_read_does_not_trigger_copy() {
    // Reading from a shared page should NOT trigger a page copy
    // Use start_addr=0x1000 (page 4) to avoid shared region at page 3
    let source = "
    LDI r1, 0x1000
    SPAWN r1
    HALT

    .org 0x1000
    LOAD r2, r0
    HALT
    ";
    let asm = assemble(source, 0).expect("assembly should succeed");
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() { vm.ram[i] = v; }

    for _ in 0..100 { if !vm.step() { break; } }

    let pd_before = vm.processes[0].page_dir.as_ref().expect("operation should succeed").clone();

    // Run child (only reads, no writes)
    for _ in 0..100 {
        vm.step_all_processes();
        if vm.processes.iter().all(|p| p.is_halted()) { break; }
    }

    let pd_after = vm.processes[0].page_dir.as_ref().expect("operation should succeed");
    // Page mapping should be unchanged (no COW resolution for reads)
    assert_eq!(pd_after[0], pd_before[0],
        "read-only child should still share the same physical page");
}

#[test]
fn test_cow_kill_decrements_ref_count() {
    // Killing a COW child should decrement ref counts, not free shared pages
    // Use start_addr=0x1000 (page 4)
    let source = "
    LDI r1, 0x1000
    SPAWN r1
    LDI r2, 1
    KILL r2
    HALT

    .org 0x1000
    HALT
    ";
    let asm = assemble(source, 0).expect("assembly should succeed");
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() { vm.ram[i] = v; }

    // Before spawn: phys page 4 ref count = 0 (not allocated by main process)
    assert_eq!(vm.page_ref_count[4], 0);

    // Step 1: LDI r1, 0x1000 (pc 0->3)
    vm.step();
    // Step 2: SPAWN r1 (pc 3->5) -- child created with COW page_dir
    vm.step();

    // After spawn: phys page 4 ref count should be >= 1 (from child's COW mapping)
    assert!(vm.page_ref_count[4] >= 1, "ref count should be >= 1 after COW fork");
    let ref_after_spawn = vm.page_ref_count[4];

    // Step 3: LDI r2, 1 (pc 5->8)
    // Step 4: KILL r2 (pc 8->10) -- decrements ref counts
    // Step 5: HALT (pc 10->11, returns false)
    for _ in 0..100 { if !vm.step() { break; } }

    // After kill: ref count on page 4 should be decremented
    assert!(vm.page_ref_count[4] < ref_after_spawn, "ref count should decrease after child killed");
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
    assert!(!vm.processes[0].is_halted(), "child should still be running");
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
fn test_spawn_non_page_aligned_org() {
    // Regression test: .org 0x600 (non-page-aligned) used to set child PC to
    // page_offset (0x200) instead of start_addr (0x600), causing immediate HALT.
    // With identity mapping (start_page < 3), virtual addr == physical addr so
    // JMP targets assembled with .org resolve correctly.
    let source = "
    LDI r1, 0x600
    SPAWN r1
    HALT

    .org 0x600
    LDI r2, 0xBEEF
    JMP child_loop

    .org 0x610
child_loop:
    STORE r0, r2
    HALT
    ";
    let asm = assemble(source, 0).expect("assembly should succeed");
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() { vm.ram[i] = v; }

    // Run main to spawn child
    for _ in 0..100 { if !vm.step() { break; } }
    assert_eq!(vm.processes.len(), 1, "should have 1 child process");
    assert!(!vm.processes[0].segfaulted, "child should not segfault");
    assert!(!vm.processes[0].is_halted(), "child should not halt immediately");

    // Run child: JMP child_loop (0x610) -> STORE -> HALT
    for _ in 0..200 {
        vm.step_all_processes();
        if vm.processes.iter().all(|p| p.is_halted()) { break; }
    }
    // Child should have executed successfully -- no segfault, and it halted
    // (meaning it executed JMP child_loop -> STORE -> HALT correctly)
    assert!(!vm.processes[0].segfaulted, "child should not segfault after running");
    assert!(vm.processes[0].is_halted(), "child should have reached HALT via JMP");
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
    let asm = assemble(source, 0).expect("assembly should succeed");
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
    let asm = assemble(source, 0).expect("assembly should succeed");
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
    let _source = "
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
    let asm = assemble(source2, 0).expect("assembly should succeed");
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
    let asm = assemble(source, 0).expect("assembly should succeed");
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
