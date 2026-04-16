// glyph_backend.rs -- GlyphLang backend for Geometry OS
//
// Translates GlyphLang spatial assembly (stack-based) into Geometry OS assembly.
//
// Mapping:
//   GlyphLang Data Stack -> Geometry OS Stack (r30 = SP)
//   GlyphLang Registers (a-z) -> Geometry OS Registers (r1-r26)
//   r0 -> reserved for CMP results
//   r27-r29 -> reserved for backend scratch

pub fn translate_glyph(source: &str) -> String {
    let mut asm = String::new();
    asm.push_str("; Translated from GlyphLang\n");
    asm.push_str("LDI r30, 0x8000 ; Initialize Stack Pointer\n\n");

    for c in source.chars() {
        match c {
            '0'..='9' => {
                let val = c.to_digit(10).unwrap_or(0);
                asm.push_str(&format!("LDI r27, {}\n", val));
                asm.push_str("PUSH r27\n");
            }
            '+' => {
                asm.push_str("POP r28 ; b\n");
                asm.push_str("POP r27 ; a\n");
                asm.push_str("ADD r27, r28\n");
                asm.push_str("PUSH r27\n");
            }
            '-' => {
                asm.push_str("POP r28 ; b\n");
                asm.push_str("POP r27 ; a\n");
                asm.push_str("SUB r27, r28\n");
                asm.push_str("PUSH r27\n");
            }
            '*' => {
                asm.push_str("POP r28 ; b\n");
                asm.push_str("POP r27 ; a\n");
                asm.push_str("MUL r27, r28\n");
                asm.push_str("PUSH r27\n");
            }
            '/' => {
                asm.push_str("POP r28 ; b\n");
                asm.push_str("POP r27 ; a\n");
                asm.push_str("DIV r27, r28\n");
                asm.push_str("PUSH r27\n");
            }
            '=' => {
                asm.push_str("POP r28 ; b\n");
                asm.push_str("POP r27 ; a\n");
                asm.push_str("CMP r27, r28\n");
                asm.push_str("JZ r0, .push_1\n");
                asm.push_str("LDI r27, 0\n");
                asm.push_str("JMP .done_eq\n");
                asm.push_str(".push_1:\n");
                asm.push_str("LDI r27, 1\n");
                asm.push_str(".done_eq:\n");
                asm.push_str("PUSH r27\n");
            }
            'a'..='z' => {
                let reg_idx = (c as u8 - b'a' + 1) as usize;
                asm.push_str(&format!("POP r{}\n", reg_idx));
            }
            'A'..='Z' => {
                let reg_idx = (c as u8 - b'A' + 1) as usize;
                asm.push_str(&format!("PUSH r{}\n", reg_idx));
            }
            '.' => {
                // Output: for now, just pop and ignore or set a pixel at (0,0) for debug
                asm.push_str("POP r27\n");
                asm.push_str("LDI r28, 0\n");
                asm.push_str("PSET r28, r28, r27 ; Debug output to (0,0)\n");
            }
            '@' => {
                asm.push_str("HALT\n");
            }
            ' ' | '\n' | '\t' => {} // ignore whitespace
            _ => {
                asm.push_str(&format!("; Unknown GlyphLang opcode: {}\n", c));
            }
        }
    }

    asm
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_translation() {
        let glyph = "3 4 + . @";
        let translated = translate_glyph(glyph);
        assert!(translated.contains("ADD r27, r28"));
        assert!(translated.contains("HALT"));
    }
}
