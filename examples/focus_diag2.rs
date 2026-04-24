// Diagnose why VM halts at PC=4096
use geometry_os::assembler::assemble;
use geometry_os::preprocessor::Preprocessor;
use geometry_os::vm::Vm;

fn main() {
    let source = std::fs::read_to_string("programs/ai_terminal.asm").unwrap();
    let mut pp = Preprocessor::new();
    let preprocessed = pp.preprocess(&source);
    let asm = assemble(&preprocessed, 0).expect("should assemble");

    println!("Program size: {} words", asm.pixels.len());
    println!(
        "Last non-zero word at: {}",
        asm.pixels.iter().rposition(|&w| w != 0).unwrap_or(0)
    );

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

    println!("After init: halted={}, PC={}", vm.halted, vm.pc);

    // Inject /focus 0x40 one key at a time with careful tracing
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
        13,
    ];

    for (ki, &key) in keys.iter().enumerate() {
        let was_halted = vm.halted;
        vm.push_key(key);
        let mut cycles = 0;
        while !vm.yielded && !vm.halted && cycles < 100000 {
            vm.step();
            cycles += 1;
        }
        if vm.yielded {
            vm.yielded = false;
        }

        // Extra frames
        for _ in 0..3 {
            let mut c = 0;
            while !vm.yielded && !vm.halted && c < 50000 {
                vm.step();
                c += 1;
            }
            if vm.yielded {
                vm.yielded = false;
            }
        }

        println!(
            "Key {} (0x{:02X}='{}'): halted={}, PC={}, cycles={}, RAM[0x7821]={}",
            ki,
            key,
            if key >= 32 && key < 127 {
                key as u8 as char
            } else {
                '?'
            },
            vm.halted,
            vm.pc,
            cycles,
            vm.ram[0x7821]
        );

        if vm.halted {
            // Check what's at PC - is it zero?
            println!(
                "  RAM[PC]={}, RAM[PC+1]={}, RAM[PC+2]={}",
                vm.ram.get(vm.pc as usize).unwrap_or(&0),
                vm.ram.get((vm.pc + 1) as usize).unwrap_or(&0),
                vm.ram.get((vm.pc + 2) as usize).unwrap_or(&0)
            );

            // Check if the disassembly makes sense at PC
            let (mnem, len) = vm.disassemble_at(vm.pc);
            println!("  Disasm at PC: {} ({} words)", mnem, len);
            break;
        }
    }
}
