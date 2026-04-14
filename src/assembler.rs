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

/// Assemble source with an optional library search path for .include directives.
/// When `lib_dir` is Some, .include "file.asm" will look in that directory.
pub fn assemble_with_lib(source: &str, base_addr: usize, lib_dir: Option<&str>) -> Result<AsmResult, AsmError> {
    // Pre-process: resolve .include directives by inlining file contents
    let expanded = resolve_includes(source, lib_dir, 0)?;
    assemble_inner(&expanded, base_addr)
}

/// Backward-compatible assemble() with no library path.
pub fn assemble(source: &str, base_addr: usize) -> Result<AsmResult, AsmError> {
    assemble_with_lib(source, base_addr, None)
}

/// Maximum include depth to prevent recursive includes.
const MAX_INCLUDE_DEPTH: usize = 8;

/// Resolve .include directives by recursively inlining file contents.
fn resolve_includes(source: &str, lib_dir: Option<&str>, depth: usize) -> Result<String, AsmError> {
    if depth > MAX_INCLUDE_DEPTH {
        return Err(AsmError { line: 0, message: "include depth exceeded (possible circular include)".into() });
    }

    let mut output = String::new();
    for (line_num, raw_line) in source.lines().enumerate() {
        let trimmed = raw_line.trim();
        // .lib name -- shorthand for .include "lib/name.asm"
        if trimmed.to_lowercase().starts_with(".lib") {
            let rest = trimmed[4..].trim();
            if rest.is_empty() {
                return Err(AsmError { line: line_num + 1, message: ".lib requires a library name".into() });
            }
            let name = if (rest.starts_with('"') && rest.ends_with('"')) || (rest.starts_with('\'') && rest.ends_with('\'')) {
                &rest[1..rest.len()-1]
            } else {
                rest
            };
            let filename = format!("lib/{}.asm", name);
            // Search for the file (same logic as .include)
            let filepath = if let Some(dir) = lib_dir {
                let p = std::path::Path::new(dir).join(&filename);
                if p.exists() { Some(p) } else { std::path::Path::new(&filename).exists().then(|| std::path::PathBuf::from(&filename)) }
            } else {
                std::path::Path::new(&filename).exists().then(|| std::path::PathBuf::from(&filename))
            };
            match filepath {
                Some(path) => {
                    let included = std::fs::read_to_string(&path)
                        .map_err(|e| AsmError { line: line_num + 1, message: format!("cannot read lib file '{}': {}", filename, e) })?;
                    let expanded = resolve_includes(&included, lib_dir, depth + 1)?;
                    output.push_str("; --- begin lib: ");
                    output.push_str(&filename);
                    output.push_str(" ---\n");
                    output.push_str(&expanded);
                    if !expanded.ends_with('\n') { output.push('\n'); }
                    output.push_str("; --- end lib: ");
                    output.push_str(&filename);
                    output.push_str(" ---\n");
                }
                None => {
                    return Err(AsmError { line: line_num + 1, message: format!("lib not found: '{}'", filename) });
                }
            }
        } else if trimmed.to_lowercase().starts_with(".include") {
            // Parse: .include "filename" or .include filename
            let rest = trimmed[8..].trim();
            let filename = if (rest.starts_with('"') && rest.ends_with('"')) || (rest.starts_with('\'') && rest.ends_with('\'')) {
                &rest[1..rest.len()-1]
            } else {
                rest
            };
            let filename = filename.trim();
            if filename.is_empty() {
                return Err(AsmError { line: line_num + 1, message: ".include requires a filename".into() });
            }
            // Search for the file
            let filepath = if let Some(dir) = lib_dir {
                let p = std::path::Path::new(dir).join(filename);
                if p.exists() { Some(p) } else { std::path::Path::new(filename).exists().then(|| std::path::PathBuf::from(filename)) }
            } else {
                std::path::Path::new(filename).exists().then(|| std::path::PathBuf::from(filename))
            };
            match filepath {
                Some(path) => {
                    let included = std::fs::read_to_string(&path)
                        .map_err(|e| AsmError { line: line_num + 1, message: format!("cannot read include file '{}': {}", filename, e) })?;
                    let expanded = resolve_includes(&included, lib_dir, depth + 1)?;
                    output.push_str("; --- begin included: ");
                    output.push_str(filename);
                    output.push_str(" ---\n");
                    output.push_str(&expanded);
                    if !expanded.ends_with('\n') { output.push('\n'); }
                    output.push_str("; --- end included: ");
                    output.push_str(filename);
                    output.push_str(" ---\n");
                }
                None => {
                    return Err(AsmError { line: line_num + 1, message: format!("include file not found: '{}'", filename) });
                }
            }
        } else {
            output.push_str(raw_line);
            output.push('\n');
        }
    }
    Ok(output)
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
        .split(|c: char| [' ', ','].contains(&c))
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

        "MEMCPY" => {
            if tokens.len() < 4 {
                return Err("MEMCPY requires 3 arguments: MEMCPY dst_reg, src_reg, len_reg".to_string());
            }
            bytecode.push(0x04);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
            bytecode.push(parse_reg(tokens[3])? as u32);
        }

        "BEEP" => {
            if tokens.len() < 3 {
                return Err("BEEP requires 2 arguments: BEEP freq_reg, dur_reg".to_string());
            }
            bytecode.push(0x03);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
        }

        "MEMCPY" => {
            if tokens.len() < 4 {
                return Err("MEMCPY requires 3 arguments: MEMCPY dst_reg, src_reg, len_reg".to_string());
            }
            bytecode.push(0x04);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
            bytecode.push(parse_reg(tokens[3])? as u32);
        }

        "LDI" => {
            if tokens.len() < 3 {
                return Err("LDI requires 2 arguments: LDI reg, imm".to_string());
            }
            bytecode.push(0x10);
            bytecode.push(parse_reg(tokens[1])? as u32);
            let pos = bytecode.len();
            if let Ok(imm) = parse_imm(tokens[2], constants) {
                bytecode.push(imm);
            } else {
                // Label reference (e.g. LDI r6, my_label)
                bytecode.push(0); // placeholder
                label_refs.push((pos, tokens[2].to_lowercase(), line_num));
            }
        }

        "LOAD" => {
            if tokens.len() < 3 {
                return Err("LOAD requires 2 arguments: LOAD reg, addr_reg".to_string());
            }
            bytecode.push(0x11);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
        }

        "STORE" => {
            if tokens.len() < 3 {
                return Err("STORE requires 2 arguments: STORE addr_reg, reg".to_string());
            }
            bytecode.push(0x12);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
        }

        "MOV" => {
            if tokens.len() < 3 {
                return Err("MOV requires 2 arguments: MOV rd, rs".to_string());
            }
            bytecode.push(0x51);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
        }

        "ADD" => {
            if tokens.len() < 3 {
                return Err("ADD requires 2 arguments: ADD rd, rs".to_string());
            }
            bytecode.push(0x20);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
        }

        "SUB" => {
            if tokens.len() < 3 {
                return Err("SUB requires 2 arguments: SUB rd, rs".to_string());
            }
            bytecode.push(0x21);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
        }

        "MUL" => {
            if tokens.len() < 3 {
                return Err("MUL requires 2 arguments: MUL rd, rs".to_string());
            }
            bytecode.push(0x22);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
        }

        "DIV" => {
            if tokens.len() < 3 {
                return Err("DIV requires 2 arguments: DIV rd, rs".to_string());
            }
            bytecode.push(0x23);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
        }

        "AND" => {
            if tokens.len() < 3 {
                return Err("AND requires 2 arguments: AND rd, rs".to_string());
            }
            bytecode.push(0x24);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
        }

        "OR" => {
            if tokens.len() < 3 {
                return Err("OR requires 2 arguments: OR rd, rs".to_string());
            }
            bytecode.push(0x25);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
        }

        "XOR" => {
            if tokens.len() < 3 {
                return Err("XOR requires 2 arguments: XOR rd, rs".to_string());
            }
            bytecode.push(0x26);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
        }

        "SHL" => {
            if tokens.len() < 3 {
                return Err("SHL requires 2 arguments: SHL rd, rs".to_string());
            }
            bytecode.push(0x27);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
        }

        "SHR" => {
            if tokens.len() < 3 {
                return Err("SHR requires 2 arguments: SHR rd, rs".to_string());
            }
            bytecode.push(0x28);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
        }

        "SAR" => {
            if tokens.len() < 3 {
                return Err("SAR requires 2 arguments: SAR rd, rs".to_string());
            }
            bytecode.push(0x2B);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
        }

        "MOD" => {
            if tokens.len() < 3 {
                return Err("MOD requires 2 arguments: MOD rd, rs".to_string());
            }
            bytecode.push(0x29);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
        }

        "JMP" => {
            if tokens.len() < 2 {
                return Err("JMP requires 1 argument: JMP addr".to_string());
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
                return Err("JZ requires 2 arguments: JZ reg, addr".to_string());
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
                return Err("JNZ requires 2 arguments: JNZ reg, addr".to_string());
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
                return Err("CALL requires 1 argument: CALL addr".to_string());
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
                return Err("BLT requires 2 arguments: BLT reg, addr".to_string());
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
                return Err("BGE requires 2 arguments: BGE reg, addr".to_string());
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
                return Err("PUSH requires 1 argument: PUSH reg".to_string());
            }
            bytecode.push(0x60);
            bytecode.push(parse_reg(tokens[1])? as u32);
        }

        "POP" => {
            if tokens.len() < 2 {
                return Err("POP requires 1 argument: POP reg".to_string());
            }
            bytecode.push(0x61);
            bytecode.push(parse_reg(tokens[1])? as u32);
        }

        "PSET" => {
            if tokens.len() < 4 {
                return Err("PSET requires 3 arguments: PSET x_reg, y_reg, color_reg".to_string());
            }
            bytecode.push(0x40);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
            bytecode.push(parse_reg(tokens[3])? as u32);
        }

        "PSETI" => {
            if tokens.len() < 4 {
                return Err("PSETI requires 3 arguments: PSETI x, y, color".to_string());
            }
            bytecode.push(0x41);
            bytecode.push(parse_imm(tokens[1], constants)?);
            bytecode.push(parse_imm(tokens[2], constants)?);
            bytecode.push(parse_imm(tokens[3], constants)?);
        }

        "FILL" => {
            if tokens.len() < 2 {
                return Err("FILL requires 1 argument: FILL color_reg".to_string());
            }
            bytecode.push(0x42);
            bytecode.push(parse_reg(tokens[1])? as u32);
        }

        "RECTF" => {
            if tokens.len() < 6 {
                return Err("RECTF requires 5 arguments: RECTF x_reg, y_reg, w_reg, h_reg, color_reg".to_string());
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
                return Err("TEXT requires 3 arguments: TEXT x_reg, y_reg, addr_reg".to_string());
            }
            bytecode.push(0x44);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
            bytecode.push(parse_reg(tokens[3])? as u32);
        }

        // TEXTI x, y, "string" -- render inline text (no RAM setup needed)
        // Encoding: 0x13, x_imm, y_imm, char_count, char1, char2, ..., 0 (null term)
        "TEXTI" => {
            if tokens.len() < 4 {
                return Err("TEXTI requires 3 args: TEXTI x, y, \"string\"".to_string());
            }
            let x = parse_imm(tokens[1], constants)?;
            let y = parse_imm(tokens[2], constants)?;
            // Reconstruct the string from remaining tokens (handles commas in strings)
            let rest = tokens[3..].join(",");
            let s = rest.trim();
            if !((s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\''))) {
                return Err("TEXTI requires a quoted string: TEXTI x, y, \"text\"".to_string());
            }
            let text = &s[1..s.len()-1];
            bytecode.push(0x13); // TEXTI opcode
            bytecode.push(x);
            bytecode.push(y);
            bytecode.push(text.len() as u32);
            for ch in text.bytes() {
                bytecode.push(ch as u32);
            }
        }

        // STRO addr_reg, "string" -- store inline string at address in register
        // Encoding: 0x14, addr_reg, char_count, char1, char2, ...
        "STRO" => {
            if tokens.len() < 3 {
                return Err("STRO requires 2 args: STRO addr_reg, \"string\"".to_string());
            }
            let reg = parse_reg(tokens[1])?;
            let rest = tokens[2..].join(",");
            let s = rest.trim();
            if !((s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\''))) {
                return Err("STRO requires a quoted string: STRO addr_reg, \"text\"".to_string());
            }
            let text = &s[1..s.len()-1];
            bytecode.push(0x14); // STRO opcode
            bytecode.push(reg as u32);
            bytecode.push(text.len() as u32);
            for ch in text.bytes() {
                bytecode.push(ch as u32);
            }
        }

        // CMPI reg, imm -- compare register against immediate, sets r0
        // Encoding: 0x15, reg, imm
        "CMPI" => {
            if tokens.len() < 3 {
                return Err("CMPI requires 2 arguments: CMPI reg, imm".to_string());
            }
            bytecode.push(0x15);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_imm(tokens[2], constants)?);
        }

        // LOADS reg, offset -- load from SP+offset (r30 + signed offset)
        // Encoding: 0x16, reg, offset
        "LOADS" => {
            if tokens.len() < 3 {
                return Err("LOADS requires 2 arguments: LOADS reg, offset".to_string());
            }
            bytecode.push(0x16);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_imm(tokens[2], constants)?);
        }

        // STORES offset, reg -- store to SP+offset (r30 + signed offset)
        // Encoding: 0x17, offset, reg
        "STORES" => {
            if tokens.len() < 3 {
                return Err("STORES requires 2 arguments: STORES offset, reg".to_string());
            }
            bytecode.push(0x17);
            bytecode.push(parse_imm(tokens[1], constants)?);
            bytecode.push(parse_reg(tokens[2])? as u32);
        }

        // SHLI reg, imm -- shift left by immediate
        // Encoding: 0x18, reg, imm
        "SHLI" => {
            if tokens.len() < 3 {
                return Err("SHLI requires 2 arguments: SHLI reg, imm".to_string());
            }
            bytecode.push(0x18);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_imm(tokens[2], constants)?);
        }

        // SHRI reg, imm -- shift right (logical) by immediate
        // Encoding: 0x19, reg, imm
        "SHRI" => {
            if tokens.len() < 3 {
                return Err("SHRI requires 2 arguments: SHRI reg, imm".to_string());
            }
            bytecode.push(0x19);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_imm(tokens[2], constants)?);
        }

        // SARI reg, imm -- arithmetic shift right by immediate
        // Encoding: 0x1A, reg, imm
        "SARI" => {
            if tokens.len() < 3 {
                return Err("SARI requires 2 arguments: SARI reg, imm".to_string());
            }
            bytecode.push(0x1A);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_imm(tokens[2], constants)?);
        }

        // ADDI reg, imm -- add immediate to register
        // Encoding: 0x1B, reg, imm
        "ADDI" => {
            if tokens.len() < 3 {
                return Err("ADDI requires 2 arguments: ADDI reg, imm".to_string());
            }
            bytecode.push(0x1B);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_imm(tokens[2], constants)?);
        }

        // SUBI reg, imm -- subtract immediate from register
        // Encoding: 0x1C, reg, imm
        "SUBI" => {
            if tokens.len() < 3 {
                return Err("SUBI requires 2 arguments: SUBI reg, imm".to_string());
            }
            bytecode.push(0x1C);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_imm(tokens[2], constants)?);
        }

        // ANDI reg, imm -- bitwise AND with immediate
        // Encoding: 0x1D, reg, imm
        "ANDI" => {
            if tokens.len() < 3 {
                return Err("ANDI requires 2 arguments: ANDI reg, imm".to_string());
            }
            bytecode.push(0x1D);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_imm(tokens[2], constants)?);
        }

        // ORI reg, imm -- bitwise OR with immediate
        // Encoding: 0x1E, reg, imm
        "ORI" => {
            if tokens.len() < 3 {
                return Err("ORI requires 2 arguments: ORI reg, imm".to_string());
            }
            bytecode.push(0x1E);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_imm(tokens[2], constants)?);
        }

        // XORI reg, imm -- bitwise XOR with immediate
        // Encoding: 0x1F, reg, imm
        "XORI" => {
            if tokens.len() < 3 {
                return Err("XORI requires 2 arguments: XORI reg, imm".to_string());
            }
            bytecode.push(0x1F);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_imm(tokens[2], constants)?);
        }

        "CMP" => {
            if tokens.len() < 3 {
                return Err("CMP requires 2 arguments: CMP rd, rs".to_string());
            }
            bytecode.push(0x50);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
        }

        "NEG" => {
            if tokens.len() < 2 {
                return Err("NEG requires 1 argument: NEG rd".to_string());
            }
            bytecode.push(0x2A);
            bytecode.push(parse_reg(tokens[1])? as u32);
        }

        "IKEY" => {
            if tokens.len() < 2 {
                return Err("IKEY requires 1 argument: IKEY reg".to_string());
            }
            bytecode.push(0x48);
            bytecode.push(parse_reg(tokens[1])? as u32);
        }

        "RAND" => {
            if tokens.len() < 2 {
                return Err("RAND requires 1 argument: RAND rd".to_string());
            }
            bytecode.push(0x49);
            bytecode.push(parse_reg(tokens[1])? as u32);
        }

        "LINE" => {
            if tokens.len() < 6 {
                return Err("LINE requires 5 arguments: LINE x0r, y0r, x1r, y1r, cr".to_string());
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
                return Err("CIRCLE requires 4 arguments: CIRCLE xr, yr, rr, cr".to_string());
            }
            bytecode.push(0x46);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
            bytecode.push(parse_reg(tokens[3])? as u32);
            bytecode.push(parse_reg(tokens[4])? as u32);
        }

        "SCROLL" => {
            if tokens.len() < 2 {
                return Err("SCROLL requires 1 argument: SCROLL nr".to_string());
            }
            bytecode.push(0x47);
            bytecode.push(parse_reg(tokens[1])? as u32);
        }

        "SPRITE" => {
            if tokens.len() < 6 {
                return Err("SPRITE requires 5 arguments: SPRITE x_reg, y_reg, addr_reg, w_reg, h_reg".to_string());
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
                return Err("ASM requires 2 arguments: ASM src_addr_reg, dest_addr_reg".to_string());
            }
            bytecode.push(0x4B);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
        }

        "TILEMAP" => {
            if tokens.len() < 9 {
                return Err("TILEMAP requires 8 arguments: TILEMAP xr, yr, mr, tr, gwr, ghr, twr, thr".to_string());
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

        "SPAWN" => {
            if tokens.len() < 2 {
                return Err("SPAWN requires 1 argument: SPAWN addr_reg".to_string());
            }
            bytecode.push(0x4D);
            bytecode.push(parse_reg(tokens[1])? as u32);
        }

        "KILL" => {
            if tokens.len() < 2 {
                return Err("KILL requires 1 argument: KILL pid_reg".to_string());
            }
            bytecode.push(0x4E);
            bytecode.push(parse_reg(tokens[1])? as u32);
        }

        "PEEK" => {
            if tokens.len() < 4 {
                return Err("PEEK requires 3 arguments: PEEK rx, ry, rd".to_string());
            }
            bytecode.push(0x4F);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
            bytecode.push(parse_reg(tokens[3])? as u32);
        }

        "SYSCALL" => {
            if tokens.len() < 2 {
                return Err("SYSCALL requires 1 argument: SYSCALL num".to_string());
            }
            bytecode.push(0x52);
            bytecode.push(parse_imm(tokens[1], constants)?);
        }

        "RETK" => {
            bytecode.push(0x53);
        }

        "OPEN" => {
            if tokens.len() < 3 {
                return Err("OPEN requires 2 arguments: OPEN path_reg, mode_reg".to_string());
            }
            bytecode.push(0x54);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
        }

        "READ" => {
            if tokens.len() < 4 {
                return Err("READ requires 3 arguments: READ fd_reg, buf_reg, len_reg".to_string());
            }
            bytecode.push(0x55);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
            bytecode.push(parse_reg(tokens[3])? as u32);
        }

        "WRITE" => {
            if tokens.len() < 4 {
                return Err("WRITE requires 3 arguments: WRITE fd_reg, buf_reg, len_reg".to_string());
            }
            bytecode.push(0x56);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
            bytecode.push(parse_reg(tokens[3])? as u32);
        }

        "CLOSE" => {
            if tokens.len() < 2 {
                return Err("CLOSE requires 1 argument: CLOSE fd_reg".to_string());
            }
            bytecode.push(0x57);
            bytecode.push(parse_reg(tokens[1])? as u32);
        }

        "SEEK" => {
            if tokens.len() < 4 {
                return Err("SEEK requires 3 arguments: SEEK fd_reg, offset_reg, whence_reg".to_string());
            }
            bytecode.push(0x58);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
            bytecode.push(parse_reg(tokens[3])? as u32);
        }

        "LS" => {
            if tokens.len() < 2 {
                return Err("LS requires 1 argument: LS buf_reg".to_string());
            }
            bytecode.push(0x59);
            bytecode.push(parse_reg(tokens[1])? as u32);
        }

        "YIELD" => {
            bytecode.push(0x5A);
        }

        "SLEEP" => {
            if tokens.len() < 2 {
                return Err("SLEEP requires 1 argument: SLEEP ticks_reg".to_string());
            }
            bytecode.push(0x5B);
            bytecode.push(parse_reg(tokens[1])? as u32);
        }

        "SETPRIORITY" => {
            if tokens.len() < 2 {
                return Err("SETPRIORITY requires 1 argument: SETPRIORITY priority_reg".to_string());
            }
            bytecode.push(0x5C);
            bytecode.push(parse_reg(tokens[1])? as u32);
        }

        "PIPE" => {
            if tokens.len() < 3 {
                return Err("PIPE requires 2 arguments: PIPE read_fd_reg, write_fd_reg".to_string());
            }
            bytecode.push(0x5D);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
        }

        "MSGSND" => {
            if tokens.len() < 2 {
                return Err("MSGSND requires 1 argument: MSGSND pid_reg".to_string());
            }
            bytecode.push(0x5E);
            bytecode.push(parse_reg(tokens[1])? as u32);
        }

        "MSGRCV" => {
            bytecode.push(0x5F);
        }

        "IOCTL" => {
            if tokens.len() < 4 {
                return Err("IOCTL requires 3 arguments: IOCTL fd_reg, cmd_reg, arg_reg".to_string());
            }
            bytecode.push(0x62);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
            bytecode.push(parse_reg(tokens[3])? as u32);
        }

        "GETENV" => {
            if tokens.len() < 3 {
                return Err("GETENV requires 2 arguments: GETENV key_addr_reg, val_addr_reg".to_string());
            }
            bytecode.push(0x63);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
        }

        "SETENV" => {
            if tokens.len() < 3 {
                return Err("SETENV requires 2 arguments: SETENV key_addr_reg, val_addr_reg".to_string());
            }
            bytecode.push(0x64);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
        }

        "GETPID" => {
            bytecode.push(0x65);
        }

        "EXEC" => {
            if tokens.len() != 2 {
                return Err("EXEC requires 1 argument: EXEC path_addr_reg".to_string());
            }
            bytecode.push(0x66);
            let r = parse_reg(tokens[1])?;
            bytecode.push(r as u32);
        }

        "WRITESTR" => {
            if tokens.len() != 3 {
                return Err("WRITESTR requires 2 arguments: WRITESTR fd_reg, str_addr_reg".to_string());
            }
            bytecode.push(0x67);
            let r1 = parse_reg(tokens[1])?;
            let r2 = parse_reg(tokens[2])?;
            bytecode.push(r1 as u32);
            bytecode.push(r2 as u32);
        }

        "READLN" => {
            if tokens.len() != 4 {
                return Err("READLN requires 3 arguments: READLN buf_reg, max_len_reg, pos_reg".to_string());
            }
            bytecode.push(0x68);
            let r1 = parse_reg(tokens[1])?;
            let r2 = parse_reg(tokens[2])?;
            let r3 = parse_reg(tokens[3])?;
            bytecode.push(r1 as u32);
            bytecode.push(r2 as u32);
            bytecode.push(r3 as u32);
        }

        "WAITPID" => {
            if tokens.len() != 2 {
                return Err("WAITPID requires 1 argument: WAITPID pid_reg".to_string());
            }
            bytecode.push(0x69);
            let r = parse_reg(tokens[1])?;
            bytecode.push(r as u32);
        }

        "EXECP" => {
            if tokens.len() != 4 {
                return Err("EXECP requires 3 arguments: EXECP path_reg, stdin_fd_reg, stdout_fd_reg".to_string());
            }
            bytecode.push(0x6A);
            let r1 = parse_reg(tokens[1])?;
            let r2 = parse_reg(tokens[2])?;
            let r3 = parse_reg(tokens[3])?;
            bytecode.push(r1 as u32);
            bytecode.push(r2 as u32);
            bytecode.push(r3 as u32);
        }

        "CHDIR" => {
            if tokens.len() != 2 {
                return Err("CHDIR requires 1 argument: CHDIR path_reg".to_string());
            }
            bytecode.push(0x6B);
            let r = parse_reg(tokens[1])?;
            bytecode.push(r as u32);
        }

        "GETCWD" => {
            if tokens.len() != 2 {
                return Err("GETCWD requires 1 argument: GETCWD buf_reg".to_string());
            }
            bytecode.push(0x6C);
            let r = parse_reg(tokens[1])?;
            bytecode.push(r as u32);
        }

        "SCREENP" => {
            // SCREENP dest, x, y -- read screen pixel at (x,y) into dest
            if tokens.len() < 4 {
                return Err("SCREENP requires 3 arguments: SCREENP dest_reg, x_reg, y_reg".to_string());
            }
            bytecode.push(0x6D);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
            bytecode.push(parse_reg(tokens[3])? as u32);
        }

        "SHUTDOWN" => {
            bytecode.push(0x6E);
        }
        "EXIT" => {
            if tokens.len() < 2 {
                return Err("EXIT requires 1 argument: EXIT code_reg".to_string());
            }
            bytecode.push(0x6F);
            bytecode.push(parse_reg(tokens[1])? as u32);
        }
        "SIGNAL" => {
            if tokens.len() < 3 {
                return Err("SIGNAL requires 2 arguments: SIGNAL pid_reg sig_reg".to_string());
            }
            bytecode.push(0x70);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
        }
        "SIGSET" => {
            if tokens.len() < 3 {
                return Err("SIGSET requires 2 arguments: SIGSET sig_reg handler_reg".to_string());
            }
            bytecode.push(0x71);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
        }

        "HYPERVISOR" => {
            if tokens.len() < 2 {
                return Err("HYPERVISOR requires 1 argument: HYPERVISOR addr_reg".to_string());
            }
            bytecode.push(0x72);
            bytecode.push(parse_reg(tokens[1])? as u32);
        }

        "ASMSELF" => {
            // No operands -- assembles canvas text into bytecode at 0x1000
            bytecode.push(0x73);
        }

        "RUNNEXT" => {
            // No operands -- sets PC to 0x1000 to execute newly assembled bytecode
            bytecode.push(0x74);
        }

        "FORMULA" => {
            // FORMULA target_idx, op, dep0, [dep1, ...]
            // target_idx: canvas buffer index (immediate)
            // op: ADD/SUB/MUL/DIV/AND/OR/XOR/NOT/COPY/MAX/MIN/MOD/SHL/SHR
            // deps: 1-8 canvas buffer indices
            if tokens.len() < 4 {
                return Err("FORMULA requires: target_idx, op, dep0, [dep1, ...]".into());
            }
            let target_idx = parse_imm(tokens[1], constants)? as u32;
            let op_name = tokens[2].trim().to_uppercase();
            let op_code = match op_name.as_str() {
                "ADD" => 0,
                "SUB" => 1,
                "MUL" => 2,
                "DIV" => 3,
                "AND" => 4,
                "OR" => 5,
                "XOR" => 6,
                "NOT" => 7,
                "COPY" => 8,
                "MAX" => 9,
                "MIN" => 10,
                "MOD" => 11,
                "SHL" => 12,
                "SHR" => 13,
                _ => return Err(format!("FORMULA: unknown op '{}'", op_name)),
            };
            let deps: Vec<u32> = tokens[3..].iter()
                .map(|a| parse_imm(a, constants).map(|v| v as u32))
                .collect::<Result<Vec<u32>, String>>()?;
            if deps.len() > 8 {
                return Err("FORMULA: too many dependencies (max 8)".into());
            }
            bytecode.push(0x75);
            bytecode.push(target_idx);
            bytecode.push(op_code);
            bytecode.push(deps.len() as u32);
            for d in &deps {
                bytecode.push(*d);
            }
        }

        "FORMULACLEAR" => {
            // No operands -- clear all formulas
            bytecode.push(0x76);
        }

        "FORMULAREM" => {
            // FORMULAREM target_idx -- remove formula from cell
            if tokens.len() < 2 {
                return Err("FORMULAREM requires: target_idx".into());
            }
            let target_idx = parse_imm(tokens[1], constants)? as u32;
            bytecode.push(0x77);
            bytecode.push(target_idx);
        }

        _ => return Err(format!("unknown opcode: {}", opcode)),
    }

    Ok(())
}
/// Parse register: "r0" -> 0, "r31" -> 31, "R5" -> 5
fn parse_reg(s: &str) -> Result<usize, String> {
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