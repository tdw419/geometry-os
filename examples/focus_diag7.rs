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

    // Disassemble 660-680
    let mut addr = 660u32;
    while addr < 685 {
        let (mnem, len) = vm.disassemble_at(addr);
        println!("{:04}: {} ({})", addr, mnem, len);
        addr += len as u32;
    }

    // Also check what the do_enter code looks like
    println!("\n=== do_enter area (330-380) ===");
    addr = 330;
    while addr < 380 {
        let (mnem, len) = vm.disassemble_at(addr);
        println!("{:04}: {} ({})", addr, mnem, len);
        addr += len as u32;
    }
}
