// Quick inline diagnostic
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
    for frame in 0..20 {
        let mut cycles = 0;
        while !vm.yielded && !vm.halted && cycles < 50000 {
            vm.step();
            cycles += 1;
        }
        vm.yielded = false;
        if vm.halted {
            println!(
                "VM halted at frame {} after {} cycles, PC={}",
                frame, cycles, vm.pc
            );
            break;
        }
    }

    println!(
        "After init: halted={}, PC={}, RAM[0x7820]={}, RAM[0x7821]{}",
        vm.halted, vm.pc, vm.ram[0x7820], vm.ram[0x7821]
    );

    if vm.halted {
        println!("VM is halted after init - cannot process commands");
        return;
    }

    // Inject /focus 0x40
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
        vm.push_key(key);
        let mut cycles = 0;
        while !vm.yielded && !vm.halted && cycles < 100000 {
            vm.step();
            cycles += 1;
        }
        vm.yielded = false;
        // Extra frames
        for _ in 0..3 {
            let mut c = 0;
            while !vm.yielded && !vm.halted && c < 50000 {
                vm.step();
                c += 1;
            }
            vm.yielded = false;
        }
        if vm.halted {
            println!(
                "VM halted at key {} (0x{:02X}) after {} cycles, PC={}",
                ki, key, cycles, vm.pc
            );
            break;
        }
    }

    println!(
        "After /focus 0x40: halted={}, PC={}, RAM[0x7821]={}",
        vm.halted, vm.pc, vm.ram[0x7821]
    );
}
