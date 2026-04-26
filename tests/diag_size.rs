use geometry_os::assembler;

#[test]
fn check_bytecode_size() {
    let source = std::fs::read_to_string("programs/world_desktop.asm").unwrap();
    let asm = assembler::assemble(&source, 0).unwrap();
    eprintln!("Bytecode size: {} words", asm.pixels.len());
    eprintln!("Last few words: {:?}", &asm.pixels[asm.pixels.len() - 5..]);

    // Check if bytecode overlaps with hardware ports
    // 0xFFF = 4095 (key port)
    // 0xFFE = 4094 (TICKS)
    // 0xFFD = 4093 (ASM result)
    // 0xFFB = 4091 (key bitmask)
    // 0xF00-0xF03 = Window Bounds
    if asm.pixels.len() > 0xF00 {
        eprintln!(
            "WARNING: Bytecode extends past 0xF00 ({} > {})",
            asm.pixels.len(),
            0xF00
        );
    }
    if asm.pixels.len() > 0xFFE {
        eprintln!("CRITICAL: Bytecode overwrites TICKS port at 0xFFE!");
    }
    if asm.pixels.len() > 0xFFF {
        eprintln!("CRITICAL: Bytecode overwrites key port at 0xFFF!");
    }

    panic!("SIZE CHECK DONE");
}
