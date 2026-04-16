// assembler -- Text to bytecode assembler for Geometry OS
//
// Reads assembly source text and produces a Vec<u32> of bytecode.
// This is the same assembler used by the canvas text surface (F8),
// the editor (F9), and the REPL (F6).
//
// Assembly syntax:
//   LDI r0, 10        ; load immediate
//   ADD r0, r1        ; add registers
//   HALT               ; stop execution
//   ; comment          ; lines starting with ; are ignored
//   label:             ; labels for jumps
//   JMP label          ; jump to label

mod includes;
mod instructions;
mod core_ops;
mod graphics_ops;
mod immediate_ops;
mod system_ops;
mod formula_ops;

#[derive(Debug)]
pub struct AsmError {
    pub line: usize,
    pub message: String,
}

impl std::fmt::Display for AsmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}

impl std::error::Error for AsmError {}

pub struct AsmResult {
    pub pixels: Vec<u32>,
}

/// Assemble source with an optional library search path for .include directives.
/// When `lib_dir` is Some, .include "file.asm" will look in that directory.
pub fn assemble_with_lib(source: &str, base_addr: usize, lib_dir: Option<&str>) -> Result<AsmResult, AsmError> {
    // Pre-process: resolve .include directives by inlining file contents
    let expanded = includes::resolve_includes(source, lib_dir, 0)?;
    assemble_inner(&expanded, base_addr)
}

/// Backward-compatible assemble() with no library path.
pub fn assemble(source: &str, base_addr: usize) -> Result<AsmResult, AsmError> {
    assemble_with_lib(source, base_addr, None)
}

fn assemble_inner(source: &str, base_addr: usize) -> Result<AsmResult, AsmError> {
    let mut bytecode: Vec<u32> = Vec::new();
    let mut labels: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut label_refs: Vec<(usize, String, usize)> = Vec::new(); // (bytecode_pos, label_name, source_line)
    let mut constants: std::collections::HashMap<String, u32> = std::collections::HashMap::new();

    // Pass 0: collect #define constants
    for (line_num, raw_line) in source.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with(';') {
            continue;
        }
        if line.to_lowercase().starts_with("#define") {
            let tokens: Vec<&str> = line.split_whitespace().collect();
            if tokens.len() < 3 {
                return Err(AsmError { line: line_num + 1, message: "#define requires NAME and VALUE".into() });
            }
            let name = tokens[1].to_string();
            // Constant value can be a literal or another constant
            match parse_imm(tokens[2], &constants) {
                Ok(val) => { constants.insert(name, val); }
                Err(e) => { return Err(AsmError { line: line_num + 1, message: format!("invalid constant {}: {}", name, e) }); }
            }
        }
    }

    // Pass 1: collect labels, emit bytecode, record label references
    for (line_num, raw_line) in source.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with(';') || line.to_lowercase().starts_with("#define") {
            continue;
        }

        // Strip inline comment before any further processing so that colons
        // inside comments are not misidentified as label delimiters.
        let line = if let Some(c) = line.find(';') { line[..c].trim() } else { line };
        if line.is_empty() { continue; }

        // .org addr -- advance bytecode position (pad with zeros)
        if line.to_lowercase().starts_with(".org") {
            let tokens: Vec<&str> = line.split_whitespace().collect();
            if tokens.len() < 2 {
                return Err(AsmError { line: line_num + 1, message: ".org requires an address".into() });
            }
            match parse_imm(tokens[1], &constants) {
                Ok(addr) => {
                    let target = addr as usize;
                    if target < bytecode.len() {
                        return Err(AsmError { line: line_num + 1, message: format!(".org 0x{:X} is behind current position 0x{:X}", target, bytecode.len()) });
                    }
                    while bytecode.len() < target { bytecode.push(0); }
                }
                Err(e) => return Err(AsmError { line: line_num + 1, message: format!("invalid .org address: {}", e) }),
            }
            continue;
        }

        // .byte val1, val2, ... -- emit raw byte values (each becomes a u32 word)
        if line.to_lowercase().starts_with(".byte") {
            let rest = line[5..].trim();
            let parts: Vec<&str> = rest.split(',').collect();
            if parts.is_empty() {
                return Err(AsmError { line: line_num + 1, message: ".byte requires at least one value".into() });
            }
            for part in parts {
                let val_str = part.trim();
                match parse_imm(val_str, &constants) {
                    Ok(v) => bytecode.push(v & 0xFF),
                    Err(e) => return Err(AsmError { line: line_num + 1, message: format!("invalid .byte value '{}': {}", val_str, e) }),
                }
            }
            continue;
        }

        // .str "text" -- emit null-terminated string (each char as a u32 word)
        if line.to_lowercase().starts_with(".str") {
            let rest = line[4..].trim();
            if !((rest.starts_with('"') && rest.ends_with('"')) || (rest.starts_with('\'') && rest.ends_with('\''))) {
                return Err(AsmError { line: line_num + 1, message: ".str requires a quoted string: .str \"text\"".into() });
            }
            let s = &rest[1..rest.len()-1];
            for ch in s.bytes() {
                bytecode.push(ch as u32);
            }
            bytecode.push(0); // null terminator
            continue;
        }

        // Check for label
        if let Some(label_end) = line.find(':') {
            let label_name = line[..label_end].trim().to_lowercase();
            labels.insert(label_name, bytecode.len());
            let rest = line[label_end + 1..].trim();
            if rest.is_empty() || rest.starts_with(';') {
                continue;
            }
            // Parse instruction after label on same line
            if let Err(e) = instructions::parse_instruction(rest, &mut bytecode, &mut label_refs, line_num + 1, &constants) {
                return Err(AsmError { line: line_num + 1, message: e });
            }
            continue;
        }

        if let Err(e) = instructions::parse_instruction(line, &mut bytecode, &mut label_refs, line_num + 1, &constants) {
            return Err(AsmError { line: line_num + 1, message: e });
        }
    }

    // Pass 2: resolve label references (add base_addr so jumps target correct RAM address)
    for (pos, label_name, line) in &label_refs {
        if let Some(&target) = labels.get(label_name) {
            bytecode[*pos] = (base_addr + target) as u32;
        } else {
            return Err(AsmError {
                line: *line,
                message: format!("undefined label: {}", label_name),
            });
        }
    }

    Ok(AsmResult { pixels: bytecode })
}

/// Parse register: "r0" -> 0, "r31" -> 31, "R5" -> 5
pub(crate) fn parse_reg(s: &str) -> Result<usize, String> {
    let s = s.trim();
    let lower = s.to_lowercase();
    if let Some(rest) = lower.strip_prefix('r') {
        if let Ok(n) = rest.parse::<usize>() {
            if n < 32 {
                return Ok(n);
            }
        }
    }
    Err(format!("invalid register: {}", s))
}

/// Parse immediate value: "10", "0xFF", "0b1010"
pub(crate) fn parse_imm(s: &str, constants: &std::collections::HashMap<String, u32>) -> Result<u32, String> {
    let s = s.trim();

    // Check constants first
    if let Some(&val) = constants.get(s) {
        return Ok(val);
    }

    if s.starts_with("0x") || s.starts_with("0X") {
        u32::from_str_radix(&s[2..], 16).map_err(|_| format!("invalid hex: {}", s))
    } else if s.starts_with("0b") || s.starts_with("0B") {
        u32::from_str_radix(&s[2..], 2).map_err(|_| format!("invalid binary: {}", s))
    } else {
        s.parse::<u32>().map_err(|_| format!("invalid number or undefined constant: {}", s))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_halt() {
        let result = assemble("HALT", 0).expect("assembly should succeed");
        assert_eq!(result.pixels, vec![0x00]);
    }

    #[test]
    fn test_ldi() {
        let result = assemble("LDI r0, 42", 0).expect("assembly should succeed");
        assert_eq!(result.pixels, vec![0x10, 0, 42]);
    }

    #[test]
    fn test_add() {
        let result = assemble("ADD r0, r1", 0).expect("assembly should succeed");
        assert_eq!(result.pixels, vec![0x20, 0, 1]);
    }

    #[test]
    fn test_multiple_lines() {
        let src = "LDI r0, 10\nLDI r1, 20\nADD r0, r1\nHALT";
        let result = assemble(src, 0).expect("assembly should succeed");
        assert_eq!(result.pixels, vec![0x10, 0, 10, 0x10, 1, 20, 0x20, 0, 1, 0x00]);
    }

    #[test]
    fn test_comments() {
        let src = "; this is a comment\nLDI r0, 5 ; inline comment\nHALT";
        let result = assemble(src, 0).expect("assembly should succeed");
        assert_eq!(result.pixels, vec![0x10, 0, 5, 0x00]);
    }

    #[test]
    fn test_labels() {
        let src = "start:\n  LDI r0, 1\n  JZ r0, start\n  HALT";
        let result = assemble(src, 0).expect("assembly should succeed");
        assert_eq!(result.pixels[0..3], vec![0x10, 0, 1]); // LDI r0, 1
        assert_eq!(result.pixels[3], 0x31); // JZ
        assert_eq!(result.pixels[4], 0);    // r0
        assert_eq!(result.pixels[5], 0);    // -> start (bytecode addr 0)
    }

    #[test]
    fn test_hex_immediate() {
        let result = assemble("LDI r0, 0xFF", 0).expect("assembly should succeed");
        assert_eq!(result.pixels, vec![0x10, 0, 255]);
    }

    #[test]
    fn test_unknown_opcode() {
        let result = assemble("BLAH r0", 0);
        assert!(result.is_err());
    }

    #[test]
    fn test_undefined_label() {
        let result = assemble("JMP nowhere", 0);
        assert!(result.is_err());
    }

    #[test]
    fn test_sub_mul_div() {
        let src = "SUB r1, r2\nMUL r3, r4\nDIV r5, r6";
        let result = assemble(src, 0).expect("assembly should succeed");
        assert_eq!(result.pixels[0], 0x21);
        assert_eq!(result.pixels[3], 0x22);
        assert_eq!(result.pixels[6], 0x23);
    }

    #[test]
    fn test_sar() {
        let src = "SAR r1, r2";
        let result = assemble(src, 0).expect("assembly should succeed");
        assert_eq!(result.pixels, vec![0x2B, 1, 2]);
    }

    #[test]
    fn test_define_constants() {
        let src = "#define SCREEN_WIDTH 256\n#define COLOR 0xFF0000\nLDI r0, SCREEN_WIDTH\nLDI r1, COLOR\nFILL r1";
        let result = assemble(src, 0).expect("assembly should succeed");
        assert_eq!(result.pixels, vec![0x10, 0, 256, 0x10, 1, 0xFF0000, 0x42, 1]);
    }

    #[test]
    fn test_nested_defines() {
        let src = "#define VAL1 10\n#define VAL2 VAL1\nLDI r0, VAL2";
        let result = assemble(src, 0).expect("assembly should succeed");
        assert_eq!(result.pixels, vec![0x10, 0, 10]);
    }
}
