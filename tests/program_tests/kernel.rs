use super::*;

// ── PHASE 23: KERNEL BOUNDARY ──────────────────────────────────

#[test]
fn test_vm_starts_in_kernel_mode() {
    let vm = Vm::new();
    assert_eq!(
        vm.mode,
        geometry_os::vm::CpuMode::Kernel,
        "VM should start in Kernel mode"
    );
}

#[test]
fn test_cpu_mode_flag_user_and_kernel() {
    let mut vm = Vm::new();
    vm.mode = geometry_os::vm::CpuMode::User;
    assert_eq!(vm.mode, geometry_os::vm::CpuMode::User);
    vm.mode = geometry_os::vm::CpuMode::Kernel;
    assert_eq!(vm.mode, geometry_os::vm::CpuMode::Kernel);
}

#[test]
fn test_syscall_assembles() {
    let source = "SYSCALL 0\nHALT";
    let asm = assemble(source, 0).expect("assembly should succeed");
    assert_eq!(asm.pixels[0], 0x52, "SYSCALL opcode should be 0x52");
    assert_eq!(asm.pixels[1], 0, "syscall number should be 0");
}

#[test]
fn test_retk_assembles() {
    let source = "RETK\nHALT";
    let asm = assemble(source, 0).expect("assembly should succeed");
    assert_eq!(asm.pixels[0], 0x53, "RETK opcode should be 0x53");
}

#[test]
fn test_syscall_dispatches_to_handler() {
    // Set up syscall table: syscall 0 -> handler at address 100
    // SYSCALL 0 should jump to RAM[0xFE00] = 100
    let mut vm = Vm::new();
    vm.ram[0xFE00] = 100; // handler for syscall 0

    // Write SYSCALL 0 at address 0
    vm.ram[0] = 0x52; // SYSCALL
    vm.ram[1] = 0; // syscall number 0
    vm.pc = 0;

    vm.step(); // execute SYSCALL 0

    assert_eq!(vm.pc, 100, "SYSCALL should jump to handler address");
    assert_eq!(
        vm.mode,
        geometry_os::vm::CpuMode::Kernel,
        "SYSCALL should switch to Kernel mode"
    );
    assert_eq!(
        vm.kernel_stack.len(),
        1,
        "SYSCALL should push to kernel stack"
    );
    assert_eq!(
        vm.kernel_stack[0].0, 2,
        "return PC should be 2 (after SYSCALL instruction)"
    );
}

#[test]
fn test_syscall_no_handler_returns_error() {
    // Syscall 5 has no handler (RAM[0xFE05] = 0)
    let mut vm = Vm::new();
    vm.ram[0] = 0x52; // SYSCALL
    vm.ram[1] = 5; // syscall number 5
    vm.pc = 0;

    vm.step(); // execute SYSCALL 5

    // Should set r0 = 0xFFFFFFFF (error) and NOT jump
    assert_eq!(
        vm.regs[0], 0xFFFFFFFF,
        "SYSCALL with no handler should set r0 to error"
    );
    assert_eq!(vm.pc, 2, "PC should advance past SYSCALL instruction");
}

#[test]
fn test_retk_returns_to_user_mode() {
    // Simulate a complete syscall -> handler -> RETK cycle
    let mut vm = Vm::new();
    vm.ram[0xFE00] = 50; // handler for syscall 0 at address 50

    // At address 0: SYSCALL 0
    vm.ram[0] = 0x52; // SYSCALL
    vm.ram[1] = 0; // syscall number 0

    // At address 50 (handler): RETK
    vm.ram[50] = 0x53; // RETK

    vm.mode = geometry_os::vm::CpuMode::User;
    vm.pc = 0;

    vm.step(); // execute SYSCALL 0 -> jumps to 50, saves return PC=2, saves mode=User
    assert_eq!(vm.pc, 50);
    assert_eq!(vm.mode, geometry_os::vm::CpuMode::Kernel);

    vm.step(); // execute RETK -> returns to PC=2, restores User mode
    assert_eq!(vm.pc, 2, "RETK should restore return PC");
    assert_eq!(
        vm.mode,
        geometry_os::vm::CpuMode::User,
        "RETK should restore User mode"
    );
    assert_eq!(
        vm.kernel_stack.len(),
        0,
        "RETK should pop from kernel stack"
    );
}

#[test]
fn test_retk_empty_stack_halts() {
    // RETK with empty kernel stack should halt (protection fault)
    let mut vm = Vm::new();
    vm.ram[0] = 0x53; // RETK with no saved state
    vm.pc = 0;

    let result = vm.step();
    assert!(!result, "RETK with empty stack should return false");
    assert!(vm.halted, "RETK with empty stack should halt the VM");
}

#[test]
fn test_user_mode_store_to_hardware_region_halts() {
    // User mode STORE to address >= 0xFF00 should halt
    let mut vm = Vm::new();
    vm.mode = geometry_os::vm::CpuMode::User;

    // LDI r0, 0xFFF0  -- address in hardware region
    vm.ram[0] = 0x10;
    vm.ram[1] = 0;
    vm.ram[2] = 0xFFF0;
    // LDI r1, 42      -- value to store
    vm.ram[3] = 0x10;
    vm.ram[4] = 1;
    vm.ram[5] = 42;
    // STORE r0, r1    -- attempt to write to hardware region
    vm.ram[6] = 0x12;
    vm.ram[7] = 0;
    vm.ram[8] = 1;
    vm.pc = 0;

    // Run LDI r0
    vm.step();
    // Run LDI r1
    vm.step();
    // Run STORE (should halt in user mode)
    let result = vm.step();
    assert!(!result, "STORE to hardware region in user mode should fail");
    assert!(
        vm.halted,
        "STORE to hardware region in user mode should halt"
    );
}

#[test]
fn test_kernel_mode_store_to_hardware_region_works() {
    // Kernel mode can write to hardware region
    let mut vm = Vm::new();
    vm.mode = geometry_os::vm::CpuMode::Kernel;

    // LDI r0, 0xFFF0
    vm.ram[0] = 0x10;
    vm.ram[1] = 0;
    vm.ram[2] = 0xFFF0;
    // LDI r1, 42
    vm.ram[3] = 0x10;
    vm.ram[4] = 1;
    vm.ram[5] = 42;
    // STORE r0, r1
    vm.ram[6] = 0x12;
    vm.ram[7] = 0;
    vm.ram[8] = 1;
    // HALT
    vm.ram[9] = 0x00;
    vm.pc = 0;

    for _ in 0..10 {
        if !vm.step() {
            break;
        }
    }
    assert!(vm.halted, "VM should halt after STORE + HALT");
    assert_eq!(
        vm.ram[0xFFF0], 42,
        "Kernel mode should write to hardware region"
    );
}

#[test]
fn test_user_mode_store_to_normal_ram_allowed() {
    // STORE to regular RAM in user mode should work fine
    let mut vm = Vm::new();
    vm.ram[0] = 0x10; // LDI r1, 100
    vm.ram[1] = 1;
    vm.ram[2] = 100;
    vm.ram[3] = 0x10; // LDI r2, 42
    vm.ram[4] = 2;
    vm.ram[5] = 42;
    vm.ram[6] = 0x12; // STORE r1, r2
    vm.ram[7] = 1;
    vm.ram[8] = 2;
    vm.ram[9] = 0x00; // HALT
    vm.pc = 0;
    vm.mode = geometry_os::vm::CpuMode::User;

    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    assert!(vm.halted, "VM should halt normally");
    assert_eq!(
        vm.ram[100], 42,
        "regular RAM write should work in user mode"
    );
}

#[test]
fn test_user_mode_ikey_halts() {
    // User mode IKEY should halt
    let mut vm = Vm::new();
    vm.mode = geometry_os::vm::CpuMode::User;
    vm.ram[0xFFF] = 65; // keyboard has a key

    // IKEY r0
    vm.ram[0] = 0x48;
    vm.ram[1] = 0;
    vm.pc = 0;

    let result = vm.step();
    assert!(!result, "IKEY in user mode should fail");
    assert!(vm.halted, "IKEY in user mode should halt");
    assert_eq!(
        vm.regs[0], 0,
        "IKEY should not have read the key in user mode"
    );
}

#[test]
fn test_kernel_mode_ikey_works() {
    // Kernel mode IKEY should work normally
    let mut vm = Vm::new();
    vm.mode = geometry_os::vm::CpuMode::Kernel;
    vm.ram[0xFFF] = 65;

    // IKEY r0
    vm.ram[0] = 0x48;
    vm.ram[1] = 0;
    vm.pc = 0;

    vm.step();
    assert_eq!(vm.regs[0], 65, "Kernel mode IKEY should read key");
    assert_eq!(vm.ram[0xFFF], 0, "Kernel mode IKEY should clear port");
}

#[test]
fn test_nested_syscalls() {
    // SYSCALL from kernel mode -> handler -> SYSCALL again -> RETK -> RETK
    let mut vm = Vm::new();
    // Syscall 0 -> address 10
    vm.ram[0xFE00] = 10;
    // Syscall 1 -> address 20
    vm.ram[0xFE01] = 20;

    // Address 0: SYSCALL 0 (in kernel mode, should still work)
    vm.ram[0] = 0x52;
    vm.ram[1] = 0;
    // Address 2: HALT (where outer RETK returns to)
    vm.ram[2] = 0x00;

    // Address 10: LDI r0, 10; SYSCALL 1; RETK
    vm.ram[10] = 0x10;
    vm.ram[11] = 0;
    vm.ram[12] = 10; // LDI r0, 10
    vm.ram[13] = 0x52;
    vm.ram[14] = 1; // SYSCALL 1
    vm.ram[15] = 0x53; // RETK

    // Address 20: LDI r0, 20; RETK
    vm.ram[20] = 0x10;
    vm.ram[21] = 0;
    vm.ram[22] = 20; // LDI r0, 20
    vm.ram[23] = 0x53; // RETK

    vm.pc = 0;
    vm.mode = geometry_os::vm::CpuMode::Kernel;

    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    assert!(vm.halted, "VM should halt");
    assert_eq!(
        vm.regs[0], 20,
        "r0 should be 20 (innermost handler sets r0, no register save/restore)"
    );
    assert_eq!(
        vm.mode,
        geometry_os::vm::CpuMode::Kernel,
        "should return to kernel mode"
    );
    assert_eq!(vm.pc, 3, "should end at instruction after outer SYSCALL");
}

#[test]
fn test_spawned_process_inherits_user_mode() {
    // SPAWN creates process in user mode
    let source = "
    LDI r1, 100
    SPAWN r1
    HALT
    ";
    let asm = assemble(source, 0).expect("assembly should succeed");
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() {
        vm.ram[i] = v;
    }
    vm.pc = 0;
    vm.mode = geometry_os::vm::CpuMode::Kernel;

    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    assert!(vm.processes.len() > 0, "should have spawned a process");
    assert_eq!(
        vm.processes[0].mode,
        geometry_os::vm::CpuMode::User,
        "spawned process should be in user mode"
    );
    assert_eq!(
        vm.mode,
        geometry_os::vm::CpuMode::Kernel,
        "parent should stay in kernel mode"
    );
}

#[test]
fn test_syscall_preserves_mode_for_nested_calls() {
    // User mode -> SYSCALL -> kernel mode -> RETK -> back to user mode
    let mut vm = Vm::new();
    vm.mode = geometry_os::vm::CpuMode::User;

    // Syscall 0 handler at address 200
    vm.ram[0xFE00] = 200;

    // At 0: SYSCALL 0
    vm.ram[0] = 0x52;
    vm.ram[1] = 0;
    // At 2: LDI r1, 42 (after return from syscall)
    vm.ram[2] = 0x10;
    vm.ram[3] = 1;
    vm.ram[4] = 42;
    // At 5: HALT
    vm.ram[5] = 0x00;

    // At 200 (handler): RETK
    vm.ram[200] = 0x53;

    vm.pc = 0;

    // SYSCALL 0 -> jumps to 200, saves (PC=2, User)
    vm.step();
    assert_eq!(vm.mode, geometry_os::vm::CpuMode::Kernel);
    assert_eq!(vm.pc, 200);

    // RETK -> returns to PC=2, restores User
    vm.step();
    assert_eq!(vm.mode, geometry_os::vm::CpuMode::User);
    assert_eq!(vm.pc, 2);

    // LDI r1, 42 should work in user mode (LDI is not restricted)
    vm.step();
    assert_eq!(
        vm.regs[1], 42,
        "LDI should work after returning from syscall"
    );
}

#[test]
fn test_reset_clears_kernel_state() {
    let mut vm = Vm::new();
    vm.mode = geometry_os::vm::CpuMode::User;
    vm.kernel_stack.push((100, geometry_os::vm::CpuMode::User));
    vm.reset();
    assert_eq!(
        vm.mode,
        geometry_os::vm::CpuMode::Kernel,
        "reset should restore Kernel mode"
    );
    assert!(
        vm.kernel_stack.is_empty(),
        "reset should clear kernel stack"
    );
}

// === Phase 24: Memory Protection Tests ===

#[test]
fn test_child_segfaults_on_unmapped_store() {
    // Spawn a child that tries to STORE to an unmapped virtual address.
    // Virtual pages 0-3 are mapped (PROCESS_PAGES=4, PAGE_SIZE=1024).
    // Virtual address 0x1000 (= page 4) is unmapped -> SEGFAULT.
    let source = "
    LDI r1, 0x200
    SPAWN r1
    HALT

    .org 0x200
    LDI r0, 0x1000
    LDI r2, 42
    STORE r0, r2
    HALT
    ";
    let asm = assemble(source, 0).expect("assembly should succeed");
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() {
        vm.ram[i] = v;
    }

    // Run main process to completion (spawns child, then halts)
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    assert!(vm.halted);
    assert_eq!(vm.processes.len(), 1);

    // Step child process -- it should segfault on the STORE
    for _ in 0..50 {
        vm.step_all_processes();
        if vm.processes[0].is_halted() {
            break;
        }
    }

    assert!(
        vm.processes[0].is_halted(),
        "child should be halted after segfault"
    );
    assert!(
        vm.processes[0].segfaulted,
        "child should have segfaulted flag set"
    );
    // RAM[0xFF9] should hold the segfaulted PID
    assert_eq!(vm.ram[0xFF9], 1, "RAM[0xFF9] should hold segfaulted PID");
}

#[test]
fn test_child_segfaults_on_unmapped_load() {
    // Spawn a child that tries to LOAD from an unmapped virtual address.
    let source = "
    LDI r1, 0x200
    SPAWN r1
    HALT

    .org 0x200
    LDI r0, 0x1000
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
    assert!(vm.halted);

    for _ in 0..50 {
        vm.step_all_processes();
        if vm.processes[0].is_halted() {
            break;
        }
    }

    assert!(
        vm.processes[0].is_halted(),
        "child should be halted after segfault"
    );
    assert!(
        vm.processes[0].segfaulted,
        "child should have segfaulted on unmapped LOAD"
    );
}

#[test]
fn test_child_segfaults_on_unmapped_fetch() {
    // Spawn a child with code at virtual page 0.
    // The child code jumps to an unmapped virtual address.
    // The fetch at the unmapped address should trigger segfault.
    let source = "
    LDI r1, 0x200
    SPAWN r1
    HALT

    .org 0x200
    JMP 0x1000
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

    for _ in 0..50 {
        vm.step_all_processes();
        if vm.processes[0].is_halted() {
            break;
        }
    }

    assert!(
        vm.processes[0].is_halted(),
        "child should be halted after segfault"
    );
    assert!(
        vm.processes[0].segfaulted,
        "child should segfault on fetching from unmapped page"
    );
}

#[test]
fn test_process_memory_isolation() {
    // Spawn two children. Each writes a unique value to its own virtual page 1.
    // Verify that child 2's data doesn't overwrite child 1's data.
    // Child 1 writes 0xAAAA at virtual 0x0400 (page 1 offset 0)
    // Child 2 writes 0xBBBB at virtual 0x0400 (page 1 offset 0)
    // These map to DIFFERENT physical pages, so neither sees the other's write.
    let source = "
    LDI r1, 0x200
    SPAWN r1
    LDI r1, 0x300
    SPAWN r1
    HALT

    .org 0x200
    LDI r0, 0x0400
    LDI r2, 0xAAAA
    STORE r0, r2
    HALT

    .org 0x300
    LDI r0, 0x0400
    LDI r2, 0xBBBB
    STORE r0, r2
    HALT
    ";
    let asm = assemble(source, 0).expect("assembly should succeed");
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() {
        vm.ram[i] = v;
    }

    // Run main to completion
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    assert!(vm.halted);
    assert_eq!(vm.processes.len(), 2);

    // Step children to completion
    for _ in 0..100 {
        vm.step_all_processes();
        if vm.processes.iter().all(|p| p.is_halted()) {
            break;
        }
    }

    // Both children should have completed without segfault
    assert!(!vm.processes[0].segfaulted, "child 1 should not segfault");
    assert!(!vm.processes[1].segfaulted, "child 2 should not segfault");

    // Verify isolation: find physical pages for each child's virtual page 1
    let pd1 = vm.processes[0]
        .page_dir
        .as_ref()
        .expect("child 1 should have page_dir");
    let pd2 = vm.processes[1]
        .page_dir
        .as_ref()
        .expect("child 2 should have page_dir");

    // Virtual page 1 -> pd1[1] and pd2[1] should be DIFFERENT physical pages
    let phys_page1 = pd1[1] as usize;
    let phys_page2 = pd2[1] as usize;
    assert_ne!(
        phys_page1, phys_page2,
        "children should have different physical pages for virtual page 1"
    );

    // Verify the values are in different physical locations
    let addr1 = phys_page1 * 1024; // PAGE_SIZE
    let addr2 = phys_page2 * 1024;
    assert_eq!(
        vm.ram[addr1], 0xAAAA,
        "child 1's physical memory should have 0xAAAA"
    );
    assert_eq!(
        vm.ram[addr2], 0xBBBB,
        "child 2's physical memory should have 0xBBBB"
    );
}

#[test]
fn test_kernel_mode_identity_mapping() {
    // Main process (kernel mode) has no page directory -> identity mapping.
    // STORE to any address should work directly.
    let source = "
    LDI r1, 0x8000
    LDI r2, 0xDEAD
    STORE r1, r2
    LOAD r3, r1
    HALT
    ";
    let asm = assemble(source, 0).expect("assembly should succeed");
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() {
        vm.ram[i] = v;
    }
    assert_eq!(
        vm.mode,
        geometry_os::vm::CpuMode::Kernel,
        "VM should start in kernel mode"
    );
    assert!(
        vm.current_page_dir.is_none(),
        "kernel should have no page directory"
    );

    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    assert!(vm.halted);
    assert!(
        !vm.segfault,
        "kernel mode should not segfault on any address"
    );
    assert_eq!(vm.regs[3], 0xDEAD, "kernel should read back what it wrote");
    assert_eq!(
        vm.canvas_buffer[0], 0xDEAD,
        "canvas_buffer[0] should have the value (0x8000 is canvas RAM range)"
    );
}

#[test]
fn test_kill_frees_physical_pages() {
    // Spawn a child, kill it, spawn another -- the freed pages should be reusable.
    let source = "
    LDI r1, 0x200
    SPAWN r1
    LDI r3, 0xFFA
    LOAD r2, r3
    KILL r2
    LDI r1, 0x300
    SPAWN r1
    HALT

    .org 0x200
    HALT

    .org 0x300
    HALT
    ";
    let asm = assemble(source, 0).expect("assembly should succeed");
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() {
        vm.ram[i] = v;
    }

    for _ in 0..200 {
        if !vm.step() {
            break;
        }
    }
    assert!(vm.halted);

    // Should have 2 processes: first killed, second alive
    assert_eq!(vm.processes.len(), 2);
    assert!(vm.processes[0].is_halted(), "first child should be killed");
    // Second child should have been spawned successfully (pages were freed)
    assert!(
        !vm.processes[1].segfaulted,
        "second child should not have segfaulted"
    );
}

#[test]
fn test_child_user_mode_blocks_hardware_port_write() {
    // Spawn a child that tries to STORE to 0xFF00+ (hardware ports).
    // In User mode this should trigger segfault.
    let source = "
    LDI r1, 0x200
    SPAWN r1
    HALT

    .org 0x200
    LDI r0, 0xFF00
    LDI r2, 42
    STORE r0, r2
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

    for _ in 0..50 {
        vm.step_all_processes();
        if vm.processes[0].is_halted() {
            break;
        }
    }

    assert!(
        vm.processes[0].segfaulted,
        "child in user mode should segfault when writing to hardware port 0xFF00+"
    );
}

#[test]
fn test_child_can_access_shared_window_bounds() {
    // Children should be able to READ the Window Bounds Protocol region (0xF00-0xFFF)
    // because page 3 is identity-mapped. They can also write to it (it's shared).
    let source = "
    LDI r1, 0x200
    SPAWN r1
    HALT

    .org 0x200
    LDI r0, 0xF00
    LOAD r2, r0
    HALT
    ";
    let asm = assemble(source, 0).expect("assembly should succeed");
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() {
        vm.ram[i] = v;
    }
    // Write something at 0xF00 for the child to read
    vm.ram[0xF00] = 0x1234;

    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    assert!(vm.halted);

    for _ in 0..50 {
        vm.step_all_processes();
        if vm.processes[0].is_halted() {
            break;
        }
    }

    assert!(
        !vm.processes[0].segfaulted,
        "child should be able to read shared window bounds region without segfault"
    );
}

#[test]
fn test_child_page_directory_has_shared_regions_mapped() {
    // Spawn a child and verify its page directory maps the shared regions.
    // With COW fork, the child shares the parent's physical pages.
    let source = "
    LDI r1, 0x200
    SPAWN r1
    HALT

    .org 0x200
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

    let pd = vm.processes[0]
        .page_dir
        .as_ref()
        .expect("child should have page_dir");

    // Page 3 (0xC00-0xFFF) should be identity-mapped for Window Bounds Protocol
    assert_eq!(pd[3], 3, "page 3 should be identity-mapped (window bounds)");
    // Page 63 (0xFC00-0xFFFF) should be identity-mapped for hardware/syscalls
    assert_eq!(
        pd[63], 63,
        "page 63 should be identity-mapped (hardware ports)"
    );

    // With COW fork, virtual pages 0-2 share parent's physical pages 0-2
    assert_eq!(
        pd[0], 0,
        "virtual page 0 should share parent's physical page 0 (COW)"
    );
    assert_eq!(
        pd[1], 1,
        "virtual page 1 should share parent's physical page 1 (COW)"
    );
    assert_eq!(
        pd[2], 2,
        "virtual page 2 should share parent's physical page 2 (COW)"
    );

    // Pages 4-62 should be unmapped
    for i in 4..63 {
        assert_eq!(
            pd[i], 0xFFFFFFFF,
            "page {} should be unmapped (PAGE_UNMAPPED)",
            i
        );
    }
}

#[test]
fn test_segfault_pid_tracking() {
    // Spawn two children. First one segfaults. Verify RAM[0xFF9] tracks the PID.
    let source = "
    LDI r1, 0x200
    SPAWN r1
    LDI r1, 0x300
    SPAWN r1
    HALT

    .org 0x200
    LDI r0, 0x1000
    LDI r2, 42
    STORE r0, r2
    HALT

    .org 0x300
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
    assert_eq!(vm.processes.len(), 2);

    for _ in 0..100 {
        vm.step_all_processes();
        if vm.processes[0].is_halted() && vm.processes[1].is_halted() {
            break;
        }
    }

    // First child (PID 1) should have segfaulted
    assert!(vm.processes[0].segfaulted, "child 1 should segfault");
    // Second child (PID 2) should have completed normally
    assert!(!vm.processes[1].segfaulted, "child 2 should not segfault");
    // RAM[0xFF9] should hold PID of the segfaulted process
    assert_eq!(
        vm.ram[0xFF9], 1,
        "RAM[0xFF9] should be PID of segfaulted child"
    );
}
