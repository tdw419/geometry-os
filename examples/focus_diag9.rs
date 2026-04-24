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

    // Check raw bytes at addresses 660-675
    println!("Raw bytes 660-675:");
    for i in 660..675 {
        println!("  [{:04}] = {} (0x{:X})", i, vm.ram[i], vm.ram[i]);
    }

    // Disassemble from a known good boundary
    println!("\nDisasm from 665:");
    let mut addr = 665u32;
    while addr < 700 {
        let (mnem, len) = vm.disassemble_at(addr);
        println!("{:04}: {} ({})", addr, mnem, len);
        addr += len as u32;
    }

    // Check the labels table from assembler
    // Actually, let me just find all CALL targets in the do_enter area
    println!("\n=== All CALL/JMP in do_enter area (356-520) ===");
    let source_lines: Vec<&str> = source.lines().collect();
    for (i, line) in source_lines.iter().enumerate() {
        let l = line.trim();
        if l.contains("CALL") || l.contains("JMP") {
            if i >= 355 && i <= 520 {
                println!("  line {}: {}", i + 1, l);
            }
        }
    }
}
