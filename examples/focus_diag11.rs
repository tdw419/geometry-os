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

    // Disassemble 4090-4120 to see the code at 4095
    let mut addr = 4090u32;
    while addr < 4120 {
        let (mnem, len) = vm.disassemble_at(addr);
        println!("{:04}: {} ({})", addr, mnem, len);
        addr += len as u32;
    }

    // Check the label map from the assembler
    println!("\n=== Label addresses ===");
    for (label, addr) in &asm.labels {
        if *addr >= 3970 && *addr <= 4110 {
            println!("{}: {}", label, addr);
        }
    }
}
