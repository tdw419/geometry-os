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

    println!(
        "BEFORE Enter: RAM[4095]={}, RAM[4094]={}, RAM[4093]={}, RAM[4096]={}",
        vm.ram[4095], vm.ram[4094], vm.ram[4093], vm.ram[4096]
    );

    // Inject Enter
    vm.push_key(13);
    let mut step = 0u32;

    // Trace with RAM monitoring at 4095
    while !vm.yielded && !vm.halted && step < 5000 {
        let prev_ram_4095 = vm.ram[4095];
        vm.step();
        step += 1;
        if vm.ram[4095] != prev_ram_4095 {
            println!(
                "Step {}: RAM[4095] changed from {} to {}!",
                step - 1,
                prev_ram_4095,
                vm.ram[4095]
            );
        }
    }

    println!("\nAFTER: halted={}, PC={}, step={}", vm.halted, vm.pc, step);
    println!(
        "RAM[4095]={}, RAM[4094]={}, RAM[4093]={}, RAM[4096]={}",
        vm.ram[4095], vm.ram[4094], vm.ram[4093], vm.ram[4096]
    );
}
