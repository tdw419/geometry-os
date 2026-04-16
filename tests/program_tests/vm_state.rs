use super::*;



// ── SAVE / LOAD ─────────────────────────────────────────────────

#[test]
fn test_vm_save_load_roundtrip() {
    let mut vm = Vm::new();
    // Set up some state
    vm.regs[0] = 42;
    vm.regs[1] = 0xDEADBEEF;
    vm.pc = 0x1000;
    vm.halted = true;
    vm.ram[0x1000] = 0x10; // LDI opcode
    vm.ram[0x1001] = 0;
    vm.ram[0x1002] = 99;
    vm.screen[128 * 256 + 128] = 0xFF0000; // red pixel at center

    let tmp = std::env::temp_dir().join("geometry_os_test_save.sav");
    vm.save_to_file(&tmp).expect("VM save should succeed");

    let loaded = Vm::load_from_file(&tmp).expect("VM load should succeed");

    assert_eq!(loaded.regs[0], 42, "r0 should be 42");
    assert_eq!(loaded.regs[1], 0xDEADBEEF, "r1 should be 0xDEADBEEF");
    assert_eq!(loaded.pc, 0x1000, "PC should be 0x1000");
    assert!(loaded.halted, "VM should be halted");
    assert_eq!(loaded.ram[0x1000], 0x10, "RAM at 0x1000 should be 0x10");
    assert_eq!(loaded.ram[0x1002], 99, "RAM at 0x1002 should be 99");
    assert_eq!(
        loaded.screen[128 * 256 + 128],
        0xFF0000,
        "center pixel should be red"
    );

    // Clean up
    std::fs::remove_file(tmp).ok();
}

#[test]
fn test_vm_save_load_preserves_rand_state_and_frame_count() {
    let mut vm = Vm::new();
    // Advance RNG by calling RAND twice (RAND rd is a 2-byte instruction)
    vm.ram[0] = 0x49; // RAND r0
    vm.ram[1] = 0;    // reg arg
    vm.ram[2] = 0x49; // RAND r0 (second call)
    vm.ram[3] = 0;    // reg arg
    vm.pc = 0;
    vm.step(); // first RAND -> pc=2
    vm.step(); // second RAND -> pc=4
    assert!(!vm.halted, "VM should not be halted after RAND");
    let rng_state_before = vm.rand_state;

    // Simulate some frame ticks (reset pc, lay down FRAME opcodes)
    vm.halted = false;
    vm.ram[0] = 0x02; // FRAME
    vm.ram[1] = 0x02; // FRAME
    vm.pc = 0;
    vm.step(); // first FRAME -> pc=1, frame_count=1
    vm.step(); // second FRAME -> pc=2, frame_count=2
    let frame_count_before = vm.frame_count;
    assert_eq!(frame_count_before, 2, "should have 2 frames");
    assert_ne!(rng_state_before, 0xDEADBEEF, "RNG should have advanced");

    let tmp = std::env::temp_dir().join("geometry_os_test_v2_save.sav");
    vm.save_to_file(&tmp).expect("VM save should succeed");

    let loaded = Vm::load_from_file(&tmp).expect("VM load should succeed");
    assert_eq!(loaded.rand_state, rng_state_before, "rand_state should be preserved");
    assert_eq!(loaded.frame_count, frame_count_before, "frame_count should be preserved");

    // Verify the loaded RNG produces the same next value as the original would
    // Call RAND on both and compare
    let mut vm2 = vm;
    let mut loaded2 = loaded;
    vm2.ram[0] = 0x49; vm2.ram[1] = 0; vm2.pc = 0;
    loaded2.ram[0] = 0x49; loaded2.ram[1] = 0; loaded2.pc = 0;
    vm2.step();
    loaded2.step();
    assert_eq!(vm2.regs[0], loaded2.regs[0], "next RAND value should match after load");

    std::fs::remove_file(tmp).ok();
}

#[test]
fn test_vm_save_load_invalid_magic() {
    let tmp = std::env::temp_dir().join("geometry_os_test_bad_magic.sav");
    std::fs::write(&tmp, b"BAD!\x00\x00\x00\x01").expect("filesystem operation failed");

    let result = Vm::load_from_file(&tmp);
    assert!(result.is_err(), "should reject invalid magic");

    std::fs::remove_file(tmp).ok();
}

#[test]
fn test_vm_save_load_preserves_program_execution() {
    // Run a program, save, load, verify the VM state is preserved
    let vm = compile_run("programs/fill_screen.asm");
    assert!(vm.halted);
    assert_eq!(vm.screen[0], 0x0000FF); // blue fill

    let tmp = std::env::temp_dir().join("geometry_os_test_program.sav");
    vm.save_to_file(&tmp).expect("VM save should succeed");

    let loaded = Vm::load_from_file(&tmp).expect("VM load should succeed");
    assert!(loaded.halted);
    // Spot-check a few screen pixels
    assert_eq!(loaded.screen[0], 0x0000FF, "top-left should be blue");
    assert_eq!(loaded.screen[128 * 256 + 128], 0x0000FF, "center should be blue");
    assert_eq!(
        loaded.screen[255 * 256 + 255],
        0x0000FF,
        "bottom-right should be blue"
    );

    std::fs::remove_file(tmp).ok();
}

#[test]
fn test_snake_assembles() {
    // Smoke test: snake.asm must assemble without errors
    let source = std::fs::read_to_string("programs/snake.asm")
        .expect("snake.asm not found");
    let asm = assemble(&source, 0x1000).expect("snake.asm failed to assemble");
    assert!(asm.pixels.len() > 100, "snake should be more than 100 words");
}

#[test]
fn test_breakpoint_halts_at_correct_address() {
    // Assemble a simple program: LDI r1, 42 / LDI r2, 99 / HALT
    // Set breakpoint at address of LDI r2, 99 (second instruction)
    let source = "LDI r1, 42\nLDI r2, 99\nHALT";
    let asm = assemble(source, 0x1000).expect("assembly should succeed");
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() { vm.ram[0x1000 + i] = v; }
    vm.pc = 0x1000;

    // Figure out where LDI r2 starts by checking instruction sizes
    let (_, first_len) = vm.disassemble_at(0x1000);
    let bp_addr = 0x1000 + first_len as u32;

    let mut breakpoints: HashSet<u32> = HashSet::new();
    breakpoints.insert(bp_addr);

    // Run with breakpoint check
    let mut hit = false;
    for _ in 0..1000 {
        if !vm.step() { break; }
        if breakpoints.contains(&vm.pc) {
            hit = true;
            break;
        }
    }

    assert!(hit, "should have hit breakpoint at 0x{:04X}", bp_addr);
    assert_eq!(vm.pc, bp_addr, "PC should be at breakpoint address");
    assert_eq!(vm.regs[1], 42, "r1 should be set before breakpoint");
    assert_ne!(vm.regs[2], 99, "r2 should NOT be set yet (breakpoint before it)");
}

#[test]
fn test_breakpoint_can_be_toggled() {
    // Set breakpoint, verify it fires, remove it, verify it doesn't fire again
    let source = "LDI r1, 1\nLDI r2, 2\nLDI r3, 3\nHALT";
    let asm = assemble(source, 0x1000).expect("assembly should succeed");
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() { vm.ram[0x1000 + i] = v; }
    vm.pc = 0x1000;

    let (_, first_len) = vm.disassemble_at(0x1000);
    let bp_addr = 0x1000 + first_len as u32;

    let mut breakpoints: HashSet<u32> = HashSet::new();
    breakpoints.insert(bp_addr);

    // Run: should hit breakpoint
    let mut hit_count = 0;
    for _ in 0..1000 {
        if !vm.step() { break; }
        if breakpoints.contains(&vm.pc) {
            hit_count += 1;
            break;
        }
    }
    assert_eq!(hit_count, 1, "should hit breakpoint once");

    // Remove breakpoint and continue to halt
    breakpoints.remove(&bp_addr);
    for _ in 0..1000 {
        if !vm.step() { break; }
        if breakpoints.contains(&vm.pc) {
            hit_count += 1;
        }
    }
    assert!(vm.halted, "VM should have halted");
    assert_eq!(hit_count, 1, "should not hit breakpoint after removal");
}

#[test]
fn test_breakpoint_not_hit_if_address_skipped() {
    // Set breakpoint at an address that the program never reaches
    let source = "LDI r1, 10\nHALT";
    let asm = assemble(source, 0x1000).expect("assembly should succeed");
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() { vm.ram[0x1000 + i] = v; }
    vm.pc = 0x1000;

    let mut breakpoints: HashSet<u32> = HashSet::new();
    breakpoints.insert(0x2000); // unreachable address

    for _ in 0..1000 {
        if !vm.step() { break; }
        assert!(!breakpoints.contains(&vm.pc), "should never hit BP at 0x2000");
    }
    assert!(vm.halted);
}

#[test]
fn test_multiple_breakpoints() {
    // Set breakpoints at multiple addresses, verify each fires
    let source = "LDI r1, 1\nLDI r2, 2\nLDI r3, 3\nLDI r4, 4\nHALT";
    let asm = assemble(source, 0x1000).expect("assembly should succeed");
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() { vm.ram[0x1000 + i] = v; }
    vm.pc = 0x1000;

    // Calculate addresses of each LDI instruction
    let mut addrs = Vec::new();
    let mut addr = 0x1000u32;
    for _ in 0..4 {
        let (_, len) = vm.disassemble_at(addr);
        addrs.push(addr);
        addr += len as u32;
    }

    let mut breakpoints: HashSet<u32> = HashSet::new();
    breakpoints.insert(addrs[1]); // LDI r2, 2
    breakpoints.insert(addrs[3]); // LDI r4, 4

    let mut hits: Vec<u32> = Vec::new();
    for _ in 0..1000 {
        if !vm.step() { break; }
        if breakpoints.contains(&vm.pc) {
            hits.push(vm.pc);
            break;
        }
    }

    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0], addrs[1], "first hit should be at LDI r2");

    // Continue after first breakpoint
    hits.clear();
    for _ in 0..1000 {
        if !vm.step() { break; }
        if breakpoints.contains(&vm.pc) {
            hits.push(vm.pc);
            break;
        }
    }

    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0], addrs[3], "second hit should be at LDI r4");
}
