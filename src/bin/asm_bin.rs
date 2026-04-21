fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: asm_bin <input.asm> <output.bin>");
        std::process::exit(1);
    }
    let source = std::fs::read_to_string(&args[1]).unwrap_or_else(|e| {
        eprintln!("Error reading {}: {}", args[1], e);
        std::process::exit(1);
    });
    let mut pp = geometry_os::preprocessor::Preprocessor::new();
    let preprocessed = pp.preprocess(&source);
    match geometry_os::assembler::assemble(&preprocessed, 0x0000) {
        Ok(result) => {
            // Save as u32 words (4 bytes each, little-endian)
            let mut bytes = Vec::with_capacity(result.pixels.len() * 4);
            for &word in &result.pixels {
                bytes.extend_from_slice(&word.to_le_bytes());
            }
            std::fs::write(&args[2], &bytes).unwrap_or_else(|e| {
                eprintln!("Error writing {}: {}", args[2], e);
                std::process::exit(1);
            });
            println!(
                "Assembled {} words ({} bytes) -> {}",
                result.pixels.len(),
                bytes.len(),
                args[2]
            );
        }
        Err(e) => {
            eprintln!("ASM error line {}: {}", e.line, e.message);
            std::process::exit(1);
        }
    }
}
