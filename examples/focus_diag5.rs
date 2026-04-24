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

    // Disassemble 3900-4000 (the focus command area)
    println!("=== Code around focus handler (3900-4000) ===");
    let mut addr = 3900u32;
    while addr < 4000 {
        let (mnem, len) = vm.disassemble_at(addr);
        println!("{:04}: {} ({})", addr, mnem, len);
        addr += len as u32;
    }

    println!("\n=== Code 3940-3980 ===");
    addr = 3940;
    while addr < 3980 {
        let (mnem, len) = vm.disassemble_at(addr);
        println!("{:04}: {} ({})", addr, mnem, len);
        addr += len as u32;
    }
}
