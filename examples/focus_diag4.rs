use geometry_os::assembler::assemble;
use geometry_os::preprocessor::Preprocessor;
use geometry_os::vm::Vm;

fn main() {
    let source = std::fs::read_to_string("programs/ai_terminal.asm").unwrap();
    let mut pp = Preprocessor::new();
    let preprocessed = pp.preprocess(&source);
    let asm = assemble(&preprocessed, 0).expect("should assemble");

    let mut vm = Vm::new();
    for (i, &word) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = word;
        }
    }

    // Run 20 init frames
    for _ in 0..20 {
        let mut cycles = 0;
        while !vm.yielded && !vm.halted && cycles < 50000 {
            vm.step();
            cycles += 1;
        }
        vm.yielded = false;
    }

    // Inject the /focus 0x40 keys but NOT Enter
    let keys: Vec<u32> = vec![
        b'/' as u32,
        b'f' as u32,
        b'o' as u32,
        b'c' as u32,
        b'u' as u32,
        b's' as u32,
        b' ' as u32,
        b'0' as u32,
        b'x' as u32,
        b'4' as u32,
        b'0' as u32,
    ];

    for &key in &keys {
        vm.push_key(key);
        let mut cycles = 0;
        while !vm.yielded && !vm.halted && cycles < 100000 {
            vm.step();
            cycles += 1;
        }
        vm.yielded = false;
        for _ in 0..3 {
            let mut c = 0;
            while !vm.yielded && !vm.halted && c < 50000 {
                vm.step();
                c += 1;
            }
            vm.yielded = false;
        }
    }

    println!("Before Enter: halted={}, PC={}", vm.halted, vm.pc);

    if vm.halted {
        return;
    }

    // Now inject Enter and trace carefully
    vm.push_key(13);
    let mut step_count = 0;
    let mut last_pc = vm.pc;
    let mut pc_changes: Vec<(u32, u32)> = Vec::new(); // (step, pc)

    while !vm.yielded && !vm.halted && step_count < 10000 {
        let prev_pc = vm.pc;
        vm.step();
        step_count += 1;

        if vm.pc != prev_pc + 1 && vm.pc != prev_pc + 2 && vm.pc != prev_pc + 3 {
            // Non-sequential jump
            if pc_changes.len() < 30 {
                let (mnem, _) = vm.disassemble_at(prev_pc);
                pc_changes.push((prev_pc, vm.pc));
            }
        }

        if vm.halted {
            println!(
                "HALTED at step {} PC={} (was {})",
                step_count, vm.pc, prev_pc
            );
            break;
        }
    }

    println!(
        "Steps: {}, halted={}, yielded={}, PC={}",
        step_count, vm.halted, vm.yielded, vm.pc
    );
    println!("Non-sequential PC changes:");
    for (from, to) in &pc_changes {
        let (mnem, _) = vm.disassemble_at(*from);
        println!("  {} -> {} ({})", from, to, mnem);
    }
}
