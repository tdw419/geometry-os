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

    // Disassemble around PC=4096
    println!("=== Instructions around 4090-4110 ===");
    let mut addr = 4090u32;
    while addr < 4110 {
        let (mnem, len) = vm.disassemble_at(addr);
        let marker = if addr == 4096 { " <<< HALTS HERE" } else { "" };
        println!("{:04}: {} ({}){}", addr, mnem, len, marker);
        addr += len as u32;
    }

    // Also check the do_enter label address
    // Look at the program source to understand the flow
    println!("\n=== Checking extract_cmd and do_enter flow ===");
    println!("What is at address of do_enter?");

    // Check if extract_cmd might be the issue - it CALLs extract_cmd
    // Let me trace the CALL addresses
    for addr in 390..415 {
        let (mnem, len) = vm.disassemble_at(addr);
        if mnem.contains("CALL") || mnem.contains("JMP") {
            println!("{:04}: {}", addr, mnem);
        }
    }
}
