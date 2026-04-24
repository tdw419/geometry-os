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

    // Inject /focus 0x40 keys (no Enter)
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

    if vm.halted {
        println!("HALTED before Enter");
        return;
    }

    // Now inject Enter and trace EVERY instruction
    vm.push_key(13);
    let mut step = 0u32;
    let mut trace = Vec::new();

    while !vm.yielded && !vm.halted && step < 5000 {
        let pc = vm.pc;
        let (mnem, _) = vm.disassemble_at(pc);
        trace.push(format!(
            "{:04}: {} (r0={}, r1={}, r5={}, r20={}, r31={})",
            pc, mnem, vm.regs[0], vm.regs[1], vm.regs[5], vm.regs[20], vm.regs[31]
        ));
        vm.step();
        step += 1;
    }

    println!(
        "Traced {} steps, halted={}, yielded={}, final PC={}",
        step, vm.halted, vm.yielded, vm.pc
    );
    println!("RAM[0x7821] = {}", vm.ram[0x7821]);

    // Print last 40 trace entries
    let start = if trace.len() > 40 {
        trace.len() - 40
    } else {
        0
    };
    for t in &trace[start..] {
        println!("{}", t);
    }
}
