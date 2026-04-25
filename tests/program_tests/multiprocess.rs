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
    for (i, &v) in asm.pixels.iter().enumerate() {
        vm.ram[i] = v;
    }
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
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
    for (i, &v) in asm.pixels.iter().enumerate() {
        vm.ram[i] = v;
    }
    for _ in 0..1000 {
        if !vm.step() {
            break;
        }
    }
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
    for (i, &v) in asm.pixels.iter().enumerate() {
        vm.ram[i] = v;
    }
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
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
    for (i, &v) in asm.pixels.iter().enumerate() {
        vm.ram[i] = v;
    }
    // Run main process to completion
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
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
        pc: 0,
        regs: [0; 32],
        state: geometry_os::vm::ProcessState::Ready,
        pid: 1,
        mode: geometry_os::vm::CpuMode::Kernel,
        page_dir: None,
        segfaulted: false,
        priority: 1,
        slice_remaining: 0,
        sleep_until: 0,
        yielded: false,
        kernel_stack: Vec::new(),
        msg_queue: Vec::new(),
        exit_code: 0,
        parent_pid: 0,
        pending_signals: Vec::new(),
        signal_handlers: [0; 4],
        vmas: Vec::new(),
        brk_pos: 0,
        custom_font: None,
        capabilities: None,
        data_base: 0,
    });
    assert_eq!(vm.active_process_count(), 1);
    vm.processes.push(geometry_os::vm::SpawnedProcess {
        pc: 0,
        regs: [0; 32],
        state: geometry_os::vm::ProcessState::Zombie,
        pid: 2,
        mode: geometry_os::vm::CpuMode::Kernel,
        page_dir: None,
        segfaulted: false,
        priority: 1,
        slice_remaining: 0,
        sleep_until: 0,
        yielded: false,
        kernel_stack: Vec::new(),
        msg_queue: Vec::new(),
        exit_code: 0,
        parent_pid: 0,
        pending_signals: Vec::new(),
        signal_handlers: [0; 4],
        vmas: Vec::new(),
        brk_pos: 0,
        custom_font: None,
        capabilities: None,
        data_base: 0,
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
    for (i, &v) in asm.pixels.iter().enumerate() {
        vm.ram[i] = v;
    }
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }

    let pd = vm.processes[0]
        .page_dir
        .as_ref()
        .expect("operation should succeed");
    // With COW, child's virtual page 0 maps to parent's physical page 4 (0x1000/1024=4)
    assert_eq!(pd[0], 4, "child vpage 0 should share parent's phys page 4");
    assert_eq!(pd[1], 5, "child vpage 1 should share parent's phys page 5");
    // Ref count on shared pages should be >= 1 (child's reference)
    assert!(
        vm.page_ref_count[4] >= 1,
        "phys page 4 should have ref count >= 1"
    );
    assert!(
        vm.page_ref_count[5] >= 1,
        "phys page 5 should have ref count >= 1"
    );
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
    for (i, &v) in asm.pixels.iter().enumerate() {
        vm.ram[i] = v;
    }

    // Run main to spawn child
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    assert_eq!(vm.processes.len(), 1);

    let pd_before = vm.processes[0]
        .page_dir
        .as_ref()
        .expect("operation should succeed")
        .clone();
    let shared_phys_page = pd_before[0]; // vpage 0 -> phys page 4 (0x1000/1024=4)

    // Run child to completion
    for _ in 0..100 {
        vm.step_all_processes();
        if vm.processes.iter().all(|p| p.is_halted()) {
            break;
        }
    }

    let pd_after = vm.processes[0]
        .page_dir
        .as_ref()
        .expect("operation should succeed");
    // After writing to vpage 0 (STORE r0, r2 where r0=0, virtual addr 0 -> vpage 0),
    // the child should have a NEW private physical page (COW resolved)
    assert_ne!(
        pd_after[0], shared_phys_page,
        "child should have a new private page after COW write"
    );
    // The new page should NOT be COW
    assert_eq!(
        vm.page_cow & (1u64 << pd_after[0] as u64),
        0,
        "new private page should not be COW"
    );
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
    for (i, &v) in asm.pixels.iter().enumerate() {
        vm.ram[i] = v;
    }

    // Run main
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    assert_eq!(vm.processes.len(), 2);

    // Run children to completion
    for _ in 0..200 {
        vm.step_all_processes();
        if vm.processes.iter().all(|p| p.is_halted()) {
            break;
        }
    }

    assert!(!vm.processes[0].segfaulted, "child 1 should not segfault");
    assert!(!vm.processes[1].segfaulted, "child 2 should not segfault");

    let pd1 = vm.processes[0]
        .page_dir
        .as_ref()
        .expect("operation should succeed");
    let pd2 = vm.processes[1]
        .page_dir
        .as_ref()
        .expect("operation should succeed");

    // After COW resolution, children should have DIFFERENT physical pages
    // (they both wrote to the same shared page, triggering separate copies)
    assert_ne!(
        pd1[0], pd2[0],
        "children should have different physical pages after COW writes"
    );
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
    for (i, &v) in asm.pixels.iter().enumerate() {
        vm.ram[i] = v;
    }

    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }

    let pd_before = vm.processes[0]
        .page_dir
        .as_ref()
        .expect("operation should succeed")
        .clone();

    // Run child (only reads, no writes)
    for _ in 0..100 {
        vm.step_all_processes();
        if vm.processes.iter().all(|p| p.is_halted()) {
            break;
        }
    }

    let pd_after = vm.processes[0]
        .page_dir
        .as_ref()
        .expect("operation should succeed");
    // Page mapping should be unchanged (no COW resolution for reads)
    assert_eq!(
        pd_after[0], pd_before[0],
        "read-only child should still share the same physical page"
    );
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
    for (i, &v) in asm.pixels.iter().enumerate() {
        vm.ram[i] = v;
    }

    // Before spawn: phys page 4 ref count = 0 (not allocated by main process)
    assert_eq!(vm.page_ref_count[4], 0);

    // Step 1: LDI r1, 0x1000 (pc 0->3)
    vm.step();
    // Step 2: SPAWN r1 (pc 3->5) -- child created with COW page_dir
    vm.step();

    // After spawn: phys page 4 ref count should be >= 1 (from child's COW mapping)
    assert!(
        vm.page_ref_count[4] >= 1,
        "ref count should be >= 1 after COW fork"
    );
    let ref_after_spawn = vm.page_ref_count[4];

    // Step 3: LDI r2, 1 (pc 5->8)
    // Step 4: KILL r2 (pc 8->10) -- decrements ref counts
    // Step 5: HALT (pc 10->11, returns false)
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }

    // After kill: ref count on page 4 should be decremented
    assert!(
        vm.page_ref_count[4] < ref_after_spawn,
        "ref count should decrease after child killed"
    );
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
    assert!(
        !vm.processes.is_empty(),
        "primary should have spawned a child process"
    );
    assert!(
        !vm.processes[0].is_halted(),
        "child should still be running"
    );
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
    assert!(
        green_count > 0,
        "window border (green pixels) should be visible"
    );
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
        .flat_map(|y| {
            (0..256usize).filter_map(move |x| {
                if screen[y * 256 + x] == ball_color {
                    Some((x, y))
                } else {
                    None
                }
            })
        })
        .collect();
    assert!(
        !ball_pixels.is_empty(),
        "red ball should be visible on screen"
    );
    // All ball pixels must be inside the window
    for (x, y) in &ball_pixels {
        assert!(
            *x >= win_x && *x < win_x + win_w,
            "ball pixel x={} outside window x={}..{}",
            x,
            win_x,
            win_x + win_w
        );
        assert!(
            *y >= win_y && *y < win_y + win_h,
            "ball pixel y={} outside window y={}..{}",
            y,
            win_y,
            win_y + win_h
        );
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
    for (i, &v) in asm.pixels.iter().enumerate() {
        vm.ram[i] = v;
    }

    // Run main to spawn child
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    assert_eq!(vm.processes.len(), 1, "should have 1 child process");
    assert!(!vm.processes[0].segfaulted, "child should not segfault");
    assert!(
        !vm.processes[0].is_halted(),
        "child should not halt immediately"
    );

    // Run child: JMP child_loop (0x610) -> STORE -> HALT
    for _ in 0..200 {
        vm.step_all_processes();
        if vm.processes.iter().all(|p| p.is_halted()) {
            break;
        }
    }
    // Child should have executed successfully -- no segfault, and it halted
    // (meaning it executed JMP child_loop -> STORE -> HALT correctly)
    assert!(
        !vm.processes[0].segfaulted,
        "child should not segfault after running"
    );
    assert!(
        vm.processes[0].is_halted(),
        "child should have reached HALT via JMP"
    );
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
    for (i, &v) in asm.pixels.iter().enumerate() {
        vm.ram[i] = v;
    }
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    assert!(vm.halted);
    // r4 should contain the red pixel color we wrote
    assert_eq!(
        vm.regs[4], 0xFF0000,
        "PEEK should read back the pixel color"
    );
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
    for (i, &v) in asm.pixels.iter().enumerate() {
        vm.ram[i] = v;
    }
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
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
    for (i, &v) in asm.pixels.iter().enumerate() {
        vm.ram[i] = v;
    }
    for _ in 0..10000 {
        if !vm.step() {
            break;
        }
    }
    assert!(vm.halted);
    // Wall pixel should be red (non-zero)
    assert_ne!(
        vm.regs[8], 0,
        "PEEK at wall should return non-zero (wall color)"
    );
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
    let source =
        std::fs::read_to_string("programs/peek_bounce.asm").expect("peek_bounce.asm should exist");
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
        if found {
            break;
        }
    }
    assert!(found, "white ball should be visible on screen");
    // Ball must be within the playable area (inside the 4px border walls)
    assert!(
        ball_x >= 4 && ball_x <= 251,
        "ball x={} should be inside borders",
        ball_x
    );
    assert!(
        ball_y >= 4 && ball_y <= 251,
        "ball y={} should be inside borders",
        ball_y
    );
}

// === Phase 117: Composite rendering -- terrain below windows ===

#[test]
fn test_screen_space_window_renders_on_top_of_terrain() {
    // Simulate the world_desktop rendering pipeline:
    // 1. Terrain is drawn to the VM screen buffer
    // 2. A screen-space WINSYS window is created with colored content
    // 3. blit_windows() is called (happens at FRAME time)
    // 4. The window pixels should appear on top of the terrain

    let mut vm = Vm::new();

    // 1. Draw terrain to screen: fill screen with a known "terrain" color
    let terrain_color = 0x228811; // green
    for pixel in vm.screen.iter_mut() {
        *pixel = terrain_color;
    }

    // 2. Create a screen-space WINSYS window at (10, 10), size 32x32
    let source = "
    LDI r7, 1
    LDI r1, 10
    LDI r2, 10
    LDI r3, 32
    LDI r4, 32
    LDI r5, 0x2000
    LDI r6, 0
    WINSYS r6
    MOV r10, r0

    ; Write red pixels to the window offscreen buffer
    LDI r3, 0xFF0000
    LDI r4, 0
fill_y:
    LDI r6, 0
fill_x:
    WPIXEL r10, r6, r4, r3
    ADD r6, r7
    LDI r25, 32
    CMP r6, r25
    BLT r0, fill_x
    ADD r4, r7
    LDI r25, 32
    CMP r4, r25
    BLT r0, fill_y

    FRAME
    HALT
    ";
    let asm = assemble(source, 0).expect("assembly should succeed");
    for (i, &v) in asm.pixels.iter().enumerate() {
        vm.ram[i] = v;
    }

    // Run until halt
    for _ in 0..100_000 {
        if !vm.step() {
            break;
        }
    }

    // 3. Verify: window pixels should be on top of terrain
    // The window is at (10, 10) with size 32x32.
    // The title bar takes WINDOW_TITLE_BAR_H pixels at the top.
    // So content starts at y=10+bar_h.
    let bar_h = geometry_os::vm::types::WINDOW_TITLE_BAR_H as usize;
    let content_y = 10 + bar_h;

    // Check a pixel inside the window content area
    let check_x = 20; // within window x range [10, 42)
    let check_y = content_y + 5; // within window y range, below title bar
    let idx = check_y * 256 + check_x;

    // The blit_windows() should have composited the window on top of terrain.
    // The window content is red (0xFF0000), terrain is green (0x228811).
    // Note: blit_windows() skips transparent (0) pixels, but our window has red.
    assert_ne!(
        vm.screen[idx], terrain_color,
        "Window pixel at ({}, {}) should NOT be terrain color -- window should render on top",
        check_x, check_y
    );

    // Check that a pixel OUTSIDE the window is still terrain
    let outside_x = 5;
    let outside_y = 5;
    let outside_idx = outside_y * 256 + outside_x;
    assert_eq!(
        vm.screen[outside_idx], terrain_color,
        "Pixel at ({}, {}) outside window should still be terrain",
        outside_x, outside_y
    );
}

#[test]
fn test_world_space_window_creation_with_world_coords() {
    // Verify that when RAM[0x7810]=1, WINSYS op=0 creates a world-space window
    // with world_x/world_y set instead of screen x/y.

    let source = "
    ; Enable world-space mode
    LDI r17, 0x7810
    LDI r18, 1
    STORE r17, r18

    ; Create window with world coords (50, 60)
    LDI r1, 50
    LDI r2, 60
    LDI r3, 64
    LDI r4, 48
    LDI r5, 0x2000
    LDI r6, 0
    WINSYS r6
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

    // Verify a window was created
    assert_eq!(vm.windows.len(), 1, "One window should be created");
    let win = &vm.windows[0];

    // Verify it's a world-space window
    assert!(win.is_world_space(), "Window should be in world-space mode");

    // Verify world coordinates
    assert_eq!(win.world_x, 50, "world_x should be 50");
    assert_eq!(win.world_y, 60, "world_y should be 60");

    // Verify dimensions
    assert_eq!(win.w, 64, "Window width should be 64");
    assert_eq!(win.h, 48, "Window height should be 48");
}

#[test]
fn test_screen_space_window_below_title_bar_not_terrain() {
    // After blit_windows(), verify the title bar area is drawn with its
    // background color (not terrain), and window content appears below it.

    let mut vm = Vm::new();

    // Fill screen with terrain blue
    let terrain_color = 0x000044;
    for pixel in vm.screen.iter_mut() {
        *pixel = terrain_color;
    }

    let source = "
    LDI r7, 1
    ; Create window at (50, 50), size 40x40
    LDI r1, 50
    LDI r2, 50
    LDI r3, 40
    LDI r4, 40
    LDI r5, 0x2000
    LDI r6, 0
    WINSYS r6
    MOV r10, r0        ; save window id to r10

    ; Write bright green to entire window
    LDI r3, 0x00FF00
    LDI r4, 0
fill2_y:
    LDI r6, 0
fill2_x:
    WPIXEL r10, r6, r4, r3
    ADD r6, r7
    LDI r25, 40
    CMP r6, r25
    BLT r0, fill2_x
    ADD r4, r7
    LDI r25, 40
    CMP r4, r25
    BLT r0, fill2_y

    FRAME
    HALT
    ";
    let asm = assemble(source, 0).expect("assembly should succeed");
    for (i, &v) in asm.pixels.iter().enumerate() {
        vm.ram[i] = v;
    }

    for _ in 0..200_000 {
        if !vm.step() {
            break;
        }
    }

    let bar_h = geometry_os::vm::types::WINDOW_TITLE_BAR_H as usize;

    // Title bar area at (60, 50) should be title bar color, not terrain
    let title_x = 60;
    let title_y = 50; // within title bar
    let title_idx = title_y * 256 + title_x;
    assert_ne!(
        vm.screen[title_idx], terrain_color,
        "Title bar at ({}, {}) should not be terrain -- title bar renders on top",
        title_x, title_y
    );

    // Window content area at (60, 50+bar_h+5) should be window content, not terrain
    let content_x = 55;
    let content_y = 50 + bar_h + 5;
    let content_idx = content_y * 256 + content_x;
    assert_ne!(
        vm.screen[content_idx], terrain_color,
        "Window content at ({}, {}) should not be terrain -- window renders on top of terrain",
        content_x, content_y
    );
}

fn make_child_process(pid: u32, pc: u32) -> geometry_os::vm::SpawnedProcess {
    geometry_os::vm::SpawnedProcess {
        pc: pc,
        regs: [0; 32],
        state: geometry_os::vm::ProcessState::Ready,
        pid: pid,
        mode: geometry_os::vm::CpuMode::Kernel,
        page_dir: None,
        segfaulted: false,
        priority: 1,
        slice_remaining: 0,
        sleep_until: 0,
        yielded: false,
        kernel_stack: Vec::new(),
        msg_queue: Vec::new(),
        exit_code: 0,
        parent_pid: 0,
        pending_signals: Vec::new(),
        signal_handlers: [0; 4],
        vmas: Vec::new(),
        brk_pos: 0,
        custom_font: None,
        capabilities: None,
        data_base: 0,
    }
}

#[test]
fn test_child_halt_does_not_stop_parent() {
    // When a child process (windowed app) halts, the main process (map)
    // must keep running. This is the core invariant for windowed apps.
    let mut vm = Vm::new();

    // Main code at 0x000: FRAME, JMP 0 (infinite loop simulating map)
    vm.ram[0x000] = 0x02; // FRAME
    vm.ram[0x001] = 0x30; // JMP
    vm.ram[0x002] = 0x000; // addr -> main_loop

    // Child at 0x200: LDI r0, 42; HALT
    vm.ram[0x200] = 0x10; // LDI
    vm.ram[0x201] = 0; // r0
    vm.ram[0x202] = 42; // imm
    vm.ram[0x203] = 0x00; // HALT

    vm.processes.push(make_child_process(1, 0x200));

    // Simulate main.rs execution loop
    for _ in 0..10_000 {
        if !vm.step() {
            break;
        }
        vm.step_all_processes();
        if vm.frame_ready {
            vm.frame_ready = false;
        }
    }

    // KEY: main process should NOT be halted
    assert!(
        !vm.halted,
        "main process (map) should still be running after child halts"
    );
    // Child should be halted
    assert!(
        vm.processes[0].is_halted(),
        "child process (app) should have halted"
    );
    // Child should have executed its code
    assert_eq!(
        vm.processes[0].regs[0], 42,
        "child should have set r0=42 before halting"
    );
}

#[test]
fn test_child_halt_does_not_affect_other_children() {
    // When child 1 halts, child 2 should keep running.
    // Simulates two windowed apps where closing one doesn't close the other.
    let mut vm = Vm::new();

    // Main: FRAME, JMP 0
    vm.ram[0x000] = 0x02;
    vm.ram[0x001] = 0x30;
    vm.ram[0x002] = 0x000;

    // Child 1 at 0x200: LDI r0, 111; HALT
    vm.ram[0x200] = 0x10;
    vm.ram[0x201] = 0;
    vm.ram[0x202] = 111;
    vm.ram[0x203] = 0x00;

    // Child 2 at 0x300: LDI r0, 222; FRAME, JMP 0x303
    vm.ram[0x300] = 0x10;
    vm.ram[0x301] = 0;
    vm.ram[0x302] = 222;
    vm.ram[0x303] = 0x02; // FRAME
    vm.ram[0x304] = 0x30; // JMP
    vm.ram[0x305] = 0x303; // -> FRAME

    vm.processes.push(make_child_process(1, 0x200));
    vm.processes.push(make_child_process(2, 0x300));

    // Create windows for both apps
    let win1 = geometry_os::vm::types::Window::new_world(1, 10, 10, 64, 64, 0, 1);
    let win2 = geometry_os::vm::types::Window::new_world(2, 100, 10, 64, 64, 0, 2);
    vm.windows.push(win1);
    vm.windows.push(win2);

    // Run execution loop
    for _ in 0..10_000 {
        if !vm.step() {
            break;
        }
        vm.step_all_processes();
        if vm.frame_ready {
            vm.frame_ready = false;
        }
    }

    // Main should still be running
    assert!(!vm.halted, "main process should still be running");

    // App 1 should be halted
    let app1 = vm.processes.iter().find(|p| p.pid == 1).unwrap();
    assert!(app1.is_halted(), "app 1 should have halted");
    assert_eq!(app1.regs[0], 111, "app 1 should have set r0=111");

    // App 2 should NOT be halted
    let app2 = vm.processes.iter().find(|p| p.pid == 2).unwrap();
    assert!(
        !app2.is_halted(),
        "app 2 should still be running after app 1 halted"
    );
    assert_eq!(app2.regs[0], 222, "app 2 should have set r0=222");

    // Simulate main.rs cleanup: destroy windows owned by halted process
    vm.windows.retain(|w| w.pid != 1);

    // Only app 2's window should remain
    assert_eq!(vm.windows.len(), 1, "only app 2's window should remain");
    assert_eq!(
        vm.windows[0].pid, 2,
        "remaining window should belong to app 2"
    );
}

#[test]
fn test_halted_app_window_cleanup() {
    // Verify the cleanup pattern used in main.rs:
    // detect halted processes, destroy their windows.
    let mut vm = Vm::new();

    // Create 3 processes, process 2 is halted (simulating closed app)
    for pid in 1..=3u32 {
        let halted = pid == 2;
        vm.processes.push(geometry_os::vm::SpawnedProcess {
            pc: 0,
            regs: [0; 32],
            state: if halted {
                geometry_os::vm::ProcessState::Zombie
            } else {
                geometry_os::vm::ProcessState::Ready
            },
            pid,
            mode: geometry_os::vm::CpuMode::Kernel,
            page_dir: None,
            segfaulted: false,
            priority: 1,
            slice_remaining: 0,
            sleep_until: 0,
            yielded: false,
            kernel_stack: Vec::new(),
            msg_queue: Vec::new(),
            exit_code: 0,
            parent_pid: 0,
            pending_signals: Vec::new(),
            signal_handlers: [0; 4],
            vmas: Vec::new(),
            brk_pos: 0,
            custom_font: None,
            capabilities: None,
            data_base: 0,
        });
    }

    // Windows for processes 1 and 2
    vm.windows.push(geometry_os::vm::types::Window::new_world(
        1, 0, 0, 64, 64, 0, 1,
    ));
    vm.windows.push(geometry_os::vm::types::Window::new_world(
        2, 100, 0, 64, 64, 0, 2,
    ));

    assert_eq!(vm.windows.len(), 2);

    // Find halted processes and remove their windows
    let halted_pids: Vec<u32> = vm
        .processes
        .iter()
        .filter(|p| p.is_halted())
        .map(|p| p.pid)
        .collect();
    assert_eq!(halted_pids, vec![2], "only process 2 should be halted");

    for &pid in &halted_pids {
        vm.windows.retain(|w| w.pid != pid);
    }

    // Only process 1's window should remain
    assert_eq!(vm.windows.len(), 1);
    assert_eq!(
        vm.windows[0].pid, 1,
        "only process 1's window should survive"
    );
}

#[test]
fn test_multi_process_launch() {
    // Verify two apps can run simultaneously in separate windows.
    // Each app writes a unique value to its private data region,
    // proving both executed and their data regions don't overlap.

    use geometry_os::vm::types::{Window, APP_DATA_BASE, APP_DATA_SIZE};

    let mut vm = Vm::new();

    // Main process at 0x000: FRAME, JMP 0 (infinite loop simulating map)
    vm.ram[0x000] = 0x02; // FRAME
    vm.ram[0x001] = 0x30; // JMP
    vm.ram[0x002] = 0x000; // -> back to FRAME

    // App 1 at 0x200: write value 0xAAAA to data region offset 0, then loop
    // LDI r1, APP_DATA_BASE (data_base for slot 0)
    vm.ram[0x200] = 0x10; // LDI
    vm.ram[0x201] = 1; // r1
    vm.ram[0x202] = APP_DATA_BASE as u32; // imm = data base
                                          // LDI r0, 0xAAAA
    vm.ram[0x203] = 0x10; // LDI
    vm.ram[0x204] = 0; // r0
    vm.ram[0x205] = 0xAAAA; // imm
                            // STORE [r1], r0
    vm.ram[0x206] = 0x12; // STORE
    vm.ram[0x207] = 1; // addr_reg = r1
    vm.ram[0x208] = 0; // reg = r0
                       // FRAME, JMP 0x203 (loop so it stays alive)
    vm.ram[0x209] = 0x02; // FRAME
    vm.ram[0x20A] = 0x30; // JMP
    vm.ram[0x20B] = 0x203; // -> LDI r0, 0xAAAA

    // App 2 at 0x300: write value 0xBBBB to data region offset 0, then loop
    let data_base_1 = (APP_DATA_BASE + APP_DATA_SIZE) as u32;
    vm.ram[0x300] = 0x10; // LDI
    vm.ram[0x301] = 1; // r1
    vm.ram[0x302] = data_base_1; // data base for slot 1
    vm.ram[0x303] = 0x10; // LDI
    vm.ram[0x304] = 0; // r0
    vm.ram[0x305] = 0xBBBB; // imm
    vm.ram[0x306] = 0x12; // STORE
    vm.ram[0x307] = 1; // addr_reg = r1
    vm.ram[0x308] = 0; // reg = r0
    vm.ram[0x309] = 0x02; // FRAME
    vm.ram[0x30A] = 0x30; // JMP
    vm.ram[0x30B] = 0x303; // -> LDI r0, 0xBBBB

    // Create two processes with separate data regions
    let mut proc1 = make_child_process(1, 0x200);
    proc1.data_base = APP_DATA_BASE as u32;
    let mut proc2 = make_child_process(2, 0x300);
    proc2.data_base = data_base_1;

    vm.processes.push(proc1);
    vm.processes.push(proc2);

    // Create windows for both apps
    let win1 = Window::new_world(1, 10, 10, 64, 64, 0, 1);
    let win2 = Window::new_world(2, 100, 10, 64, 64, 0, 2);
    vm.windows.push(win1);
    vm.windows.push(win2);

    // Run the execution loop (main + children)
    for _ in 0..10_000 {
        if !vm.step() {
            break;
        }
        vm.step_all_processes();
        if vm.frame_ready {
            vm.frame_ready = false;
        }
    }

    // Main process should still be running
    assert!(!vm.halted, "main process should still be running");

    // Both child processes should be alive (they loop)
    let app1 = vm.processes.iter().find(|p| p.pid == 1).unwrap();
    let app2 = vm.processes.iter().find(|p| p.pid == 2).unwrap();
    assert!(!app1.is_halted(), "app 1 should still be running");
    assert!(!app2.is_halted(), "app 2 should still be running");

    // Both apps should have written to their private data regions
    assert_eq!(
        vm.ram[APP_DATA_BASE], 0xAAAA,
        "app 1 should have written 0xAAAA to its data region"
    );
    assert_eq!(
        vm.ram[APP_DATA_BASE + APP_DATA_SIZE],
        0xBBBB,
        "app 2 should have written 0xBBBB to its data region"
    );

    // Data regions should NOT overlap
    assert_ne!(
        vm.ram[APP_DATA_BASE],
        vm.ram[APP_DATA_BASE + APP_DATA_SIZE],
        "data regions must be separate"
    );

    // Both windows should still be active
    assert_eq!(vm.windows.len(), 2, "both windows should be active");
}

#[test]
fn test_window_drag_updates_world_coords() {
    // Simulates what the fullscreen map drag handler does:
    // 1. Create a world-space window at (50, 60)
    // 2. Simulate drag: compute new world coords from pixel delta
    // 3. Verify world_x/world_y update correctly

    let source = "
    ; Enable world-space mode
    LDI r17, 0x7810
    LDI r18, 1
    STORE r17, r18

    ; Create window with world coords (50, 60), size 64x48
    LDI r1, 50
    LDI r2, 60
    LDI r3, 64
    LDI r4, 48
    LDI r5, 0x2000
    LDI r6, 0
    WINSYS r6
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

    // Verify initial state
    assert_eq!(vm.windows.len(), 1);
    let win = &vm.windows[0];
    assert!(win.is_world_space());
    assert_eq!(win.world_x, 50);
    assert_eq!(win.world_y, 60);

    // Simulate drag: new_wx = old_wx + dx, new_wy = old_wy + dy
    // This mirrors main.rs lines 3039-3045:
    //   let dx = (mx - window_drag_start.0) / px_per_tile;
    //   let new_wx = window_drag_world_start.0 + dx as i32;
    //   w.world_x = if new_wx >= 0 { new_wx as u32 } else { 0 };
    let new_wx: i32 = 50 + 15; // dragged 15 tiles right
    let new_wy: i32 = 60 + 10; // dragged 10 tiles down
    let win = vm.windows.iter_mut().find(|w| w.id == 1).unwrap();
    win.world_x = if new_wx >= 0 { new_wx as u32 } else { 0 };
    win.world_y = if new_wy >= 0 { new_wy as u32 } else { 0 };

    // Verify updated world coords
    let win = &vm.windows[0];
    assert_eq!(win.world_x, 65, "world_x should update to 65 after drag");
    assert_eq!(win.world_y, 70, "world_y should update to 70 after drag");
    assert!(
        win.is_world_space(),
        "window should still be world-space after drag"
    );
}

#[test]
fn test_window_drag_negative_clamps_to_zero() {
    // Dragging a window to negative world coords should clamp to 0

    let source = "
    ; Enable world-space mode
    LDI r17, 0x7810
    LDI r18, 1
    STORE r17, r18

    ; Create window at world coords (5, 3)
    LDI r1, 5
    LDI r2, 3
    LDI r3, 64
    LDI r4, 48
    LDI r5, 0x2000
    LDI r6, 0
    WINSYS r6
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

    // Simulate dragging to negative position (dragged further left/up than origin)
    let new_wx: i32 = 5 - 10; // would be -5
    let new_wy: i32 = 3 - 8; // would be -5
    let win = vm.windows.iter_mut().find(|w| w.id == 1).unwrap();
    win.world_x = if new_wx >= 0 { new_wx as u32 } else { 0 };
    win.world_y = if new_wy >= 0 { new_wy as u32 } else { 0 };

    let win = &vm.windows[0];
    assert_eq!(win.world_x, 0, "world_x should clamp to 0 on negative drag");
    assert_eq!(win.world_y, 0, "world_y should clamp to 0 on negative drag");
}

#[test]
fn test_world_space_window_placement() {
    // Verifies world_x/world_y are set correctly across multiple placement scenarios:
    // 1. Screen-space window gets WORLD_COORD_UNSET for world coords
    // 2. World-space window at origin (0, 0)
    // 3. World-space window at large coordinates
    // 4. Two world-space windows at different coords
    // 5. Screen-space window created after disabling world mode

    let unset: u32 = 0xFFFFFFFF;

    // --- Scenario 1: Screen-space window (default, no world-space flag) ---
    let source_screen = "
        LDI r17, 0x7810
        LDI r18, 0
        STORE r17, r18
        ; Create screen-space window at (10, 20)
        LDI r1, 10
        LDI r2, 20
        LDI r3, 32
        LDI r4, 24
        LDI r5, 0x2000
        LDI r6, 0
        WINSYS r6
        HALT
    ";
    let asm = assemble(source_screen, 0).expect("screen-space assembly should succeed");
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() {
        vm.ram[i] = v;
    }
    for _ in 0..1000 {
        if !vm.step() {
            break;
        }
    }

    assert_eq!(vm.windows.len(), 1, "scenario 1: one screen-space window");
    let win = &vm.windows[0];
    assert!(
        !win.is_world_space(),
        "scenario 1: window should NOT be world-space"
    );
    assert_eq!(win.world_x, unset, "scenario 1: world_x should be UNSET");
    assert_eq!(win.world_y, unset, "scenario 1: world_y should be UNSET");
    assert_eq!(win.x, 10, "scenario 1: screen x should be 10");
    assert_eq!(win.y, 20, "scenario 1: screen y should be 20");

    // --- Scenario 2: World-space window at origin (0, 0) ---
    let source_origin = "
        LDI r17, 0x7810
        LDI r18, 1
        STORE r17, r18
        ; Create world-space window at world (0, 0)
        LDI r1, 0
        LDI r2, 0
        LDI r3, 48
        LDI r4, 32
        LDI r5, 0x2000
        LDI r6, 0
        WINSYS r6
        HALT
    ";
    let asm = assemble(source_origin, 0).expect("origin assembly should succeed");
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() {
        vm.ram[i] = v;
    }
    for _ in 0..1000 {
        if !vm.step() {
            break;
        }
    }

    assert_eq!(vm.windows.len(), 1, "scenario 2: one world-space window");
    let win = &vm.windows[0];
    assert!(
        win.is_world_space(),
        "scenario 2: window should be world-space"
    );
    assert_eq!(win.world_x, 0, "scenario 2: world_x should be 0");
    assert_eq!(win.world_y, 0, "scenario 2: world_y should be 0");

    // --- Scenario 3: World-space window at large coordinates ---
    let source_large = "
        LDI r17, 0x7810
        LDI r18, 1
        STORE r17, r18
        ; Create world-space window at (500, 300)
        LDI r1, 500
        LDI r2, 300
        LDI r3, 64
        LDI r4, 48
        LDI r5, 0x2000
        LDI r6, 0
        WINSYS r6
        HALT
    ";
    let asm = assemble(source_large, 0).expect("large coords assembly should succeed");
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() {
        vm.ram[i] = v;
    }
    for _ in 0..1000 {
        if !vm.step() {
            break;
        }
    }

    assert_eq!(vm.windows.len(), 1, "scenario 3: one window");
    let win = &vm.windows[0];
    assert!(
        win.is_world_space(),
        "scenario 3: window should be world-space"
    );
    assert_eq!(win.world_x, 500, "scenario 3: world_x should be 500");
    assert_eq!(win.world_y, 300, "scenario 3: world_y should be 300");
    assert_eq!(win.w, 64, "scenario 3: width should be 64");
    assert_eq!(win.h, 48, "scenario 3: height should be 48");

    // --- Scenario 4: Two world-space windows at different coords ---
    let source_two = "
        LDI r17, 0x7810
        LDI r18, 1
        STORE r17, r18
        ; First window at world (20, 15)
        LDI r1, 20
        LDI r2, 15
        LDI r3, 40
        LDI r4, 30
        LDI r5, 0x2000
        LDI r6, 0
        WINSYS r6
        ; Save first window id
        MOV r10, r0
        ; Second window at world (80, 60)
        LDI r1, 80
        LDI r2, 60
        LDI r3, 50
        LDI r4, 40
        LDI r5, 0x3000
        LDI r6, 0
        WINSYS r6
        HALT
    ";
    let asm = assemble(source_two, 0).expect("two windows assembly should succeed");
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() {
        vm.ram[i] = v;
    }
    for _ in 0..2000 {
        if !vm.step() {
            break;
        }
    }

    assert_eq!(vm.windows.len(), 2, "scenario 4: two windows");
    let win0 = &vm.windows[0];
    let win1 = &vm.windows[1];
    assert!(
        win0.is_world_space(),
        "scenario 4: window 0 should be world-space"
    );
    assert!(
        win1.is_world_space(),
        "scenario 4: window 1 should be world-space"
    );
    assert_eq!(win0.world_x, 20, "scenario 4: win0 world_x should be 20");
    assert_eq!(win0.world_y, 15, "scenario 4: win0 world_y should be 15");
    assert_eq!(win1.world_x, 80, "scenario 4: win1 world_x should be 80");
    assert_eq!(win1.world_y, 60, "scenario 4: win1 world_y should be 60");

    // --- Scenario 5: Switch from world-space to screen-space within same VM ---
    let source_switch = "
        ; First: world-space window
        LDI r17, 0x7810
        LDI r18, 1
        STORE r17, r18
        LDI r1, 100
        LDI r2, 200
        LDI r3, 32
        LDI r4, 32
        LDI r5, 0x2000
        LDI r6, 0
        WINSYS r6
        ; Then: disable world-space and create screen-space window
        LDI r17, 0x7810
        LDI r18, 0
        STORE r17, r18
        LDI r1, 30
        LDI r2, 40
        LDI r3, 24
        LDI r4, 16
        LDI r5, 0x4000
        LDI r6, 0
        WINSYS r6
        HALT
    ";
    let asm = assemble(source_switch, 0).expect("switch assembly should succeed");
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() {
        vm.ram[i] = v;
    }
    for _ in 0..2000 {
        if !vm.step() {
            break;
        }
    }

    assert_eq!(
        vm.windows.len(),
        2,
        "scenario 5: two windows (one world, one screen)"
    );
    let win_ws = &vm.windows[0];
    let win_ss = &vm.windows[1];
    assert!(
        win_ws.is_world_space(),
        "scenario 5: first window should be world-space"
    );
    assert!(
        !win_ss.is_world_space(),
        "scenario 5: second window should be screen-space"
    );
    assert_eq!(
        win_ws.world_x, 100,
        "scenario 5: world-space world_x should be 100"
    );
    assert_eq!(
        win_ws.world_y, 200,
        "scenario 5: world-space world_y should be 200"
    );
    assert_eq!(
        win_ss.world_x, unset,
        "scenario 5: screen-space world_x should be UNSET"
    );
    assert_eq!(
        win_ss.world_y, unset,
        "scenario 5: screen-space world_y should be UNSET"
    );
    assert_eq!(win_ss.x, 30, "scenario 5: screen-space x should be 30");
    assert_eq!(win_ss.y, 40, "scenario 5: screen-space y should be 40");
}
