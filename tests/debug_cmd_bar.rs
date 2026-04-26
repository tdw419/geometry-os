use geometry_os::assembler::assemble;
use geometry_os::vm::Vm;

#[test]
fn disasm_around_4091() {
    let source = std::fs::read_to_string("programs/world_desktop.asm").unwrap();
    let asm = assemble(&source, 0).unwrap();
    eprintln!("Bytecode size: {} words", asm.pixels.len());

    let mut vm = Vm::new();
    for (i, &word) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = word;
        }
    }

    // Disassemble around the halt address
    for addr in (4070..4110).step_by(1) {
        let (mnemonic, width) = vm.disassemble_at(addr);
        eprintln!("[{:5}] {} (width={})", addr, mnemonic, width);
    }
}
