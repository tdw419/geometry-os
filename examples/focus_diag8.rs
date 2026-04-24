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

    // Disassemble 3979-4050 (after extract_cmd, should have do_newline)
    let mut addr = 3979u32;
    while addr < 4050 {
        let (mnem, len) = vm.disassemble_at(addr);
        println!("{:04}: {} ({})", addr, mnem, len);
        addr += len as u32;
    }

    // Also check 4090-4100 to see what's at 4095
    println!("\n=== 4090-4100 ===");
    addr = 4090;
    while addr < 4100 {
        let (mnem, len) = vm.disassemble_at(addr);
        println!(
            "{:04}: {} ({}) [raw: {},{},{}]",
            addr,
            mnem,
            len,
            vm.ram[addr as usize],
            vm.ram.get((addr + 1) as usize).unwrap_or(&0),
            vm.ram.get((addr + 2) as usize).unwrap_or(&0)
        );
        addr += len as u32;
    }
}
