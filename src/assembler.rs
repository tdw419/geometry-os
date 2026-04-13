// assembler.rs -- Text to bytecode assembler for Geometry OS
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

pub struct AsmResult {
    pub pixels: Vec<u32>,
}

pub fn assemble(source: &str, base_addr: usize) -> Result<AsmResult, AsmError> {
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

        // Check for label
        if let Some(label_end) = line.find(':') {
            let label_name = line[..label_end].trim().to_lowercase();
            labels.insert(label_name, bytecode.len());
            let rest = line[label_end + 1..].trim();
            if rest.is_empty() || rest.starts_with(';') {
                continue;
            }
            // Parse instruction after label on same line
            if let Err(e) = parse_instruction(rest, &mut bytecode, &mut label_refs, line_num + 1, &constants) {
                return Err(AsmError { line: line_num + 1, message: e });
            }
            continue;
        }

        if let Err(e) = parse_instruction(line, &mut bytecode, &mut label_refs, line_num + 1, &constants) {
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

fn parse_instruction(
    line: &str,
    bytecode: &mut Vec<u32>,
    label_refs: &mut Vec<(usize, String, usize)>,
    line_num: usize,
    constants: &std::collections::HashMap<String, u32>,
) -> Result<(), String> {
    // Strip inline comment
    let line = if let Some(comment_pos) = line.find(';') {
        line[..comment_pos].trim()
    } else {
        line
    };

    let line = line.trim();
    if line.is_empty() {
        return Ok(());
    }

    // Split into tokens: "LDI r0, 10" -> ["LDI", "r0", "10"]
    let tokens: Vec<&str> = line
        .split(|c: char| c == ' ' || c == ',')
        .map(|t| t.trim())
        .filter(|t| !t.is_empty())
        .collect();

    if tokens.is_empty() {
        return Ok(());
    }

    let opcode = tokens[0].to_uppercase();

    match opcode.as_str() {
        "HALT" => bytecode.push(0x00),
        "NOP" => bytecode.push(0x01),
        "FRAME" => bytecode.push(0x02),

        "BEEP" => {
            if tokens.len() < 3 {
                return Err(format!("BEEP requires 2 arguments: BEEP freq_reg, dur_reg"));
            }
            bytecode.push(0x03);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
        }

        "LDI" => {
            if tokens.len() < 3 {
                return Err(format!("LDI requires 2 arguments: LDI reg, imm"));
            }
            bytecode.push(0x10);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_imm(tokens[2], constants)?);
        }

        "LOAD" => {
            if tokens.len() < 3 {
                return Err(format!("LOAD requires 2 arguments: LOAD reg, addr_reg"));
            }
            bytecode.push(0x11);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
        }

        "STORE" => {
            if tokens.len() < 3 {
                return Err(format!("STORE requires 2 arguments: STORE addr_reg, reg"));
            }
            bytecode.push(0x12);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
        }

        "ADD" => {
            if tokens.len() < 3 {
                return Err(format!("ADD requires 2 arguments: ADD rd, rs"));
            }
            bytecode.push(0x20);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
        }

        "SUB" => {
            if tokens.len() < 3 {
                return Err(format!("SUB requires 2 arguments: SUB rd, rs"));
            }
            bytecode.push(0x21);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
        }

        "MUL" => {
            if tokens.len() < 3 {
                return Err(format!("MUL requires 2 arguments: MUL rd, rs"));
            }
            bytecode.push(0x22);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
        }

        "DIV" => {
            if tokens.len() < 3 {
                return Err(format!("DIV requires 2 arguments: DIV rd, rs"));
            }
            bytecode.push(0x23);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
        }

        "AND" => {
            if tokens.len() < 3 {
                return Err(format!("AND requires 2 arguments: AND rd, rs"));
            }
            bytecode.push(0x24);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
        }

        "OR" => {
            if tokens.len() < 3 {
                return Err(format!("OR requires 2 arguments: OR rd, rs"));
            }
            bytecode.push(0x25);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
        }

        "XOR" => {
            if tokens.len() < 3 {
                return Err(format!("XOR requires 2 arguments: XOR rd, rs"));
            }
            bytecode.push(0x26);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
        }

        "SHL" => {
            if tokens.len() < 3 {
                return Err(format!("SHL requires 2 arguments: SHL rd, rs"));
            }
            bytecode.push(0x27);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
        }

        "SHR" => {
            if tokens.len() < 3 {
                return Err(format!("SHR requires 2 arguments: SHR rd, rs"));
            }
            bytecode.push(0x28);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
        }

        "SAR" => {
            if tokens.len() < 3 {
                return Err(format!("SAR requires 2 arguments: SAR rd, rs"));
            }
            bytecode.push(0x2B);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
        }

        "MOD" => {
            if tokens.len() < 3 {
                return Err(format!("MOD requires 2 arguments: MOD rd, rs"));
            }
            bytecode.push(0x29);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
        }

        "JMP" => {
            if tokens.len() < 2 {
                return Err(format!("JMP requires 1 argument: JMP addr"));
            }
            bytecode.push(0x30);
            let pos = bytecode.len();
            if let Ok(addr) = parse_imm(tokens[1], constants) {
                bytecode.push(addr);
            } else {
                // Label reference
                bytecode.push(0); // placeholder
                label_refs.push((pos, tokens[1].to_lowercase(), line_num));
            }
        }

        "JZ" => {
            if tokens.len() < 3 {
                return Err(format!("JZ requires 2 arguments: JZ reg, addr"));
            }
            bytecode.push(0x31);
            bytecode.push(parse_reg(tokens[1])? as u32);
            let pos = bytecode.len();
            if let Ok(addr) = parse_imm(tokens[2], constants) {
                bytecode.push(addr);
            } else {
                bytecode.push(0);
                label_refs.push((pos, tokens[2].to_lowercase(), line_num));
            }
        }

        "JNZ" => {
            if tokens.len() < 3 {
                return Err(format!("JNZ requires 2 arguments: JNZ reg, addr"));
            }
            bytecode.push(0x32);
            bytecode.push(parse_reg(tokens[1])? as u32);
            let pos = bytecode.len();
            if let Ok(addr) = parse_imm(tokens[2], constants) {
                bytecode.push(addr);
            } else {
                bytecode.push(0);
                label_refs.push((pos, tokens[2].to_lowercase(), line_num));
            }
        }

        "CALL" => {
            if tokens.len() < 2 {
                return Err(format!("CALL requires 1 argument: CALL addr"));
            }
            bytecode.push(0x33);
            let pos = bytecode.len();
            if let Ok(addr) = parse_imm(tokens[1], constants) {
                bytecode.push(addr);
            } else {
                bytecode.push(0);
                label_refs.push((pos, tokens[1].to_lowercase(), line_num));
            }
        }

        "RET" => bytecode.push(0x34),

        "BLT" => {
            if tokens.len() < 3 {
                return Err(format!("BLT requires 2 arguments: BLT reg, addr"));
            }
            bytecode.push(0x35);
            bytecode.push(parse_reg(tokens[1])? as u32);
            let pos = bytecode.len();
            if let Ok(addr) = parse_imm(tokens[2], constants) {
                bytecode.push(addr);
            } else {
                bytecode.push(0);
                label_refs.push((pos, tokens[2].to_lowercase(), line_num));
            }
        }

        "BGE" => {
            if tokens.len() < 3 {
                return Err(format!("BGE requires 2 arguments: BGE reg, addr"));
            }
            bytecode.push(0x36);
            bytecode.push(parse_reg(tokens[1])? as u32);
            let pos = bytecode.len();
            if let Ok(addr) = parse_imm(tokens[2], constants) {
                bytecode.push(addr);
            } else {
                bytecode.push(0);
                label_refs.push((pos, tokens[2].to_lowercase(), line_num));
            }
        }

        "PUSH" => {
            if tokens.len() < 2 {
                return Err(format!("PUSH requires 1 argument: PUSH reg"));
            }
            bytecode.push(0x60);
            bytecode.push(parse_reg(tokens[1])? as u32);
        }

        "POP" => {
            if tokens.len() < 2 {
                return Err(format!("POP requires 1 argument: POP reg"));
            }
            bytecode.push(0x61);
            bytecode.push(parse_reg(tokens[1])? as u32);
        }

        "PSET" => {
            if tokens.len() < 4 {
                return Err(format!("PSET requires 3 arguments: PSET x_reg, y_reg, color_reg"));
            }
            bytecode.push(0x40);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
            bytecode.push(parse_reg(tokens[3])? as u32);
        }

        "PSETI" => {
            if tokens.len() < 4 {
                return Err(format!("PSETI requires 3 arguments: PSETI x, y, color"));
            }
            bytecode.push(0x41);
            bytecode.push(parse_imm(tokens[1], constants)?);
            bytecode.push(parse_imm(tokens[2], constants)?);
            bytecode.push(parse_imm(tokens[3], constants)?);
        }

        "FILL" => {
            if tokens.len() < 2 {
                return Err(format!("FILL requires 1 argument: FILL color_reg"));
            }
            bytecode.push(0x42);
            bytecode.push(parse_reg(tokens[1])? as u32);
        }

        "RECTF" => {
            if tokens.len() < 6 {
                return Err(format!("RECTF requires 5 arguments: RECTF x_reg, y_reg, w_reg, h_reg, color_reg"));
            }
            bytecode.push(0x43);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
            bytecode.push(parse_reg(tokens[3])? as u32);
            bytecode.push(parse_reg(tokens[4])? as u32);
            bytecode.push(parse_reg(tokens[5])? as u32);
        }

        "TEXT" => {
            if tokens.len() < 4 {
                return Err(format!("TEXT requires 3 arguments: TEXT x_reg, y_reg, addr_reg"));
            }
            bytecode.push(0x44);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
            bytecode.push(parse_reg(tokens[3])? as u32);
        }

        "CMP" => {
            if tokens.len() < 3 {
                return Err(format!("CMP requires 2 arguments: CMP rd, rs"));
            }
            bytecode.push(0x50);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
        }

        "NEG" => {
            if tokens.len() < 2 {
                return Err(format!("NEG requires 1 argument: NEG rd"));
            }
            bytecode.push(0x2A);
            bytecode.push(parse_reg(tokens[1])? as u32);
        }

        "IKEY" => {
            if tokens.len() < 2 {
                return Err(format!("IKEY requires 1 argument: IKEY reg"));
            }
            bytecode.push(0x48);
            bytecode.push(parse_reg(tokens[1])? as u32);
        }

        "RAND" => {
            if tokens.len() < 2 {
                return Err(format!("RAND requires 1 argument: RAND rd"));
            }
            bytecode.push(0x49);
            bytecode.push(parse_reg(tokens[1])? as u32);
        }

        "LINE" => {
            if tokens.len() < 6 {
                return Err(format!("LINE requires 5 arguments: LINE x0r, y0r, x1r, y1r, cr"));
            }
            bytecode.push(0x45);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
            bytecode.push(parse_reg(tokens[3])? as u32);
            bytecode.push(parse_reg(tokens[4])? as u32);
            bytecode.push(parse_reg(tokens[5])? as u32);
        }

        "CIRCLE" => {
            if tokens.len() < 5 {
                return Err(format!("CIRCLE requires 4 arguments: CIRCLE xr, yr, rr, cr"));
            }
            bytecode.push(0x46);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
            bytecode.push(parse_reg(tokens[3])? as u32);
            bytecode.push(parse_reg(tokens[4])? as u32);
        }

        "SCROLL" => {
            if tokens.len() < 2 {
                return Err(format!("SCROLL requires 1 argument: SCROLL nr"));
            }
            bytecode.push(0x47);
            bytecode.push(parse_reg(tokens[1])? as u32);
        }

        "SPRITE" => {
            if tokens.len() < 6 {
                return Err(format!("SPRITE requires 5 arguments: SPRITE x_reg, y_reg, addr_reg, w_reg, h_reg"));
            }
            bytecode.push(0x4A);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
            bytecode.push(parse_reg(tokens[3])? as u32);
            bytecode.push(parse_reg(tokens[4])? as u32);
            bytecode.push(parse_reg(tokens[5])? as u32);
        }

        "ASM" => {
            if tokens.len() < 3 {
                return Err(format!("ASM requires 2 arguments: ASM src_addr_reg, dest_addr_reg"));
            }
            bytecode.push(0x4B);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
        }

        "TILEMAP" => {
            if tokens.len() < 9 {
                return Err(format!("TILEMAP requires 8 arguments: TILEMAP xr, yr, mr, tr, gwr, ghr, twr, thr"));
            }
            bytecode.push(0x4C);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
            bytecode.push(parse_reg(tokens[3])? as u32);
            bytecode.push(parse_reg(tokens[4])? as u32);
            bytecode.push(parse_reg(tokens[5])? as u32);
            bytecode.push(parse_reg(tokens[6])? as u32);
            bytecode.push(parse_reg(tokens[7])? as u32);
            bytecode.push(parse_reg(tokens[8])? as u32);
        }

        _ => return Err(format!("unknown opcode: {}", opcode)),
    }

    Ok(())
}

/// Parse register: "r0" -> 0, "r31" -> 31, "R5" -> 5
fn parse_reg(s: &str) -> Result<usize, String> {
    let s = s.trim();
    let lower = s.to_lowercase();
    if lower.starts_with('r') {
        if let Ok(n) = lower[1..].parse::<usize>() {
            if n < 32 {
                return Ok(n);
            }
        }
    }
    Err(format!("invalid register: {}", s))
}

/// Parse immediate value: "10", "0xFF", "0b1010"
fn parse_imm(s: &str, constants: &std::collections::HashMap<String, u32>) -> Result<u32, String> {
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
        let result = assemble("HALT", 0).unwrap();
        assert_eq!(result.pixels, vec![0x00]);
    }

    #[test]
    fn test_ldi() {
        let result = assemble("LDI r0, 42", 0).unwrap();
        assert_eq!(result.pixels, vec![0x10, 0, 42]);
    }

    #[test]
    fn test_add() {
        let result = assemble("ADD r0, r1", 0).unwrap();
        assert_eq!(result.pixels, vec![0x20, 0, 1]);
    }

    #[test]
    fn test_multiple_lines() {
        let src = "LDI r0, 10\nLDI r1, 20\nADD r0, r1\nHALT";
        let result = assemble(src, 0).unwrap();
        assert_eq!(result.pixels, vec![0x10, 0, 10, 0x10, 1, 20, 0x20, 0, 1, 0x00]);
    }

    #[test]
    fn test_comments() {
        let src = "; this is a comment\nLDI r0, 5 ; inline comment\nHALT";
        let result = assemble(src, 0).unwrap();
        assert_eq!(result.pixels, vec![0x10, 0, 5, 0x00]);
    }

    #[test]
    fn test_labels() {
        let src = "start:\n  LDI r0, 1\n  JZ r0, start\n  HALT";
        let result = assemble(src, 0).unwrap();
        assert_eq!(result.pixels[0..3], vec![0x10, 0, 1]); // LDI r0, 1
        assert_eq!(result.pixels[3], 0x31); // JZ
        assert_eq!(result.pixels[4], 0);    // r0
        assert_eq!(result.pixels[5], 0);    // -> start (bytecode addr 0)
    }

    #[test]
    fn test_hex_immediate() {
        let result = assemble("LDI r0, 0xFF", 0).unwrap();
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
        let result = assemble(src, 0).unwrap();
        assert_eq!(result.pixels[0], 0x21);
        assert_eq!(result.pixels[3], 0x22);
        assert_eq!(result.pixels[6], 0x23);
    }

    #[test]
    fn test_sar() {
        let src = "SAR r1, r2";
        let result = assemble(src, 0).unwrap();
        assert_eq!(result.pixels, vec![0x2B, 1, 2]);
    }

    #[test]
    fn test_define_constants() {
        let src = "#define SCREEN_WIDTH 256\n#define COLOR 0xFF0000\nLDI r0, SCREEN_WIDTH\nLDI r1, COLOR\nFILL r1";
        let result = assemble(src, 0).unwrap();
        assert_eq!(result.pixels, vec![0x10, 0, 256, 0x10, 1, 0xFF0000, 0x42, 1]);
    }

    #[test]
    fn test_nested_defines() {
        let src = "#define VAL1 10\n#define VAL2 VAL1\nLDI r0, VAL2";
        let result = assemble(src, 0).unwrap();
        assert_eq!(result.pixels, vec![0x10, 0, 10]);
    }
}
