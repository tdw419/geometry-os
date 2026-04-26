use geometry_os::assembler;

#[test]
fn find_addr_mapping() {
    let source = std::fs::read_to_string("programs/world_desktop.asm").unwrap();
    let asm = assembler::assemble(&source, 0).unwrap();

    // Find where value 0x02 appears in the bytecode
    // But 0x02 could be a FRAME opcode OR an argument to another instruction
    // Let me look for the sequence that the trace shows

    // The trace shows: pc=4091 = JNZ r18, 0x1009, then pc=4094 = FRAME
    // So bytes [4091, 4092, 4093] = JNZ instruction, byte 4094 = FRAME
    // JNZ encoding: opcode(0x32), reg, addr = 3 words
    // So ram[4091] = 0x32, ram[4092] = 18, ram[4093] = 0x1009
    // And ram[4094] = 0x02 = FRAME

    eprintln!(
        "ram[4091] = {} (expected 0x32=50 for JNZ)",
        asm.pixels[4091]
    );
    eprintln!("ram[4092] = {} (expected 18=r18)", asm.pixels[4092]);
    eprintln!("ram[4093] = {} (expected 0x1009=4105)", asm.pixels[4093]);
    eprintln!("ram[4094] = {} (expected 0x02=FRAME)", asm.pixels[4094]);

    // Find the ASM source line that generates address 4094
    // The assembler processes source lines sequentially, generating bytecode
    // Each instruction generates 1-9 words
    // I need to find which source line produced address 4094

    // Let's search for pattern type dispatch with value 5
    eprintln!("\nSearching for '0x5' in pattern dispatch context...");

    panic!("ADDR MAP DONE");
}
