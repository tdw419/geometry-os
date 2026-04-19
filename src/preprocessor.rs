// preprocessor.rs -- Macro expansion and variable resolution for Geometry OS
//
// This sits between the canvas color grid and the final assembler.
// It translates high-level constructs (SET, GET, variables) into raw opcodes.
// It uses the same syntax tokenization as the rendering pipeline (the "font colors").

use std::collections::HashMap;

/// Valid opcodes for syntax highlighting and preprocessing.
/// Must be kept in sync with assembler.rs opcode list.
pub const OPCODES: &[&str] = &[
    "HALT",
    "NOP",
    "FRAME",
    "LDI",
    "LOAD",
    "STORE",
    "ADD",
    "SUB",
    "MUL",
    "DIV",
    "AND",
    "OR",
    "XOR",
    "SHL",
    "SHR",
    "MOD",
    "SAR",
    "MOV",
    "JMP",
    "JZ",
    "JNZ",
    "CALL",
    "RET",
    "BLT",
    "BGE",
    "PSET",
    "PSETI",
    "FILL",
    "RECTF",
    "TEXT",
    "LINE",
    "CIRCLE",
    "SCROLL",
    "IKEY",
    "RAND",
    "NEG",
    "CMP",
    "PUSH",
    "POP",
    "BEEP",
    "ASM",
    "MEMCPY",
    "SPAWN",
    "KILL",
    "PEEK",
    "SPRITE",
    "TILEMAP",
    // New immediate-form opcodes
    "TEXTI",
    "STRO",
    "CMPI",
    "LOADS",
    "STORES",
    "SHLI",
    "SHRI",
    "SARI",
    "ADDI",
    "SUBI",
    "ANDI",
    "ORI",
    "XORI",
    "SYSCALL",
    "RETK",
    "OPEN",
    "READ",
    "WRITE",
    "CLOSE",
    "SEEK",
    "LS",
    "YIELD",
    "SLEEP",
    "SETPRIORITY",
    "PIPE",
    "MSGSND",
    "MSGRCV",
    "IOCTL",
    "GETENV",
    "SETENV",
    "GETPID",
    "EXEC",
    "WRITESTR",
    "READLN",
    "WAITPID",
    "EXECP",
    "CHDIR",
    "GETCWD",
    "SHUTDOWN",
    "EXIT",
    "SIGNAL",
    "SIGSET",
    "HYPERVISOR",
    "ASMSELF",
    "RUNNEXT",
    "FORMULA",
    "FORMULACLEAR",
    "FORMULAREM",
    "FMKDIR",
    "FSTAT",
    "FUNLINK",
    "SNAP_TRACE",
    "REPLAY",
    "FORK",
    "NOTE",
    "CONNECT",
    "SOCKSEND",
    "SOCKRECV",
    "DISCONNECT",
    "TRACE_READ",
    // Preprocessor macros (not real opcodes, but recognized as Opcode tokens)
    "VAR",
    "SET",
    "GET",
    "INC",
    "DEC",
];

/// Token types produced by the syntax highlighter.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SynTok {
    Opcode,
    Register,
    Number,
    Label,
    Comment,
    Formula,
    Default,
}

/// A single token with its start column and length.
#[derive(Debug)]
pub struct SynSpan {
    pub kind: SynTok,
    pub start: usize,
    pub len: usize,
    pub text: String,
}

/// Parse a line of assembly text into syntax spans for highlighting and preprocessing.
pub fn parse_syntax_line(line: &str) -> Vec<SynSpan> {
    let mut spans: Vec<SynSpan> = Vec::new();
    let trimmed = line.trim();

    if trimmed.is_empty() {
        return spans;
    }

    // Check if entire line (after trim) is a comment
    if trimmed.starts_with(';') {
        spans.push(SynSpan {
            kind: SynTok::Comment,
            start: 0,
            len: line.len(),
            text: line.to_string(),
        });
        return spans;
    }

    // Check if entire line (after trim) is a formula expression: starts with '='
    // e.g. "= 10 + 20" or "= score * bonus" or "= ~ 42"
    if trimmed.starts_with('=') {
        let eq_pos = line.find('=').unwrap_or(0);
        // Push the '=' token
        spans.push(SynSpan {
            kind: SynTok::Formula,
            start: eq_pos,
            len: 1,
            text: "=".to_string(),
        });
        // Tokenize everything after '='
        let rest = &line[eq_pos + 1..];
        let mut fpos = 0usize;
        let ftokens: Vec<&str> = rest
            .split([' ', '\t', ','])
            .filter(|s| !s.is_empty())
            .collect();
        for ftoken in &ftokens {
            let rel = rest[fpos..].find(*ftoken).unwrap_or(0);
            let abs = eq_pos + 1 + fpos + rel;
            let kind = if ftoken.starts_with('r') || ftoken.starts_with('R') {
                let rp = &ftoken[1..];
                if rp.parse::<u32>().is_ok() {
                    SynTok::Register
                } else {
                    SynTok::Label
                }
            } else if ftoken.chars().next().is_some_and(|c| c.is_ascii_digit())
                || ftoken.starts_with("0x")
                || ftoken.starts_with("0X")
                || ftoken.starts_with("0b")
                || ftoken.starts_with("0B")
            {
                SynTok::Number
            } else if [
                "+", "-", "*", "/", "&", "|", "^", "~", "%", "<<", ">>", "MAX", "MIN", "max",
                "min", "ADD", "SUB", "MUL", "DIV", "AND", "OR", "XOR", "NOT", "MOD", "SHL", "SHR",
            ]
            .contains(ftoken)
            {
                SynTok::Formula
            } else {
                SynTok::Label
            };
            spans.push(SynSpan {
                kind,
                start: abs,
                len: ftoken.len(),
                text: ftoken.to_string(),
            });
            fpos = fpos + rel + ftoken.len();
        }
        return spans;
    }

    // Check for label definition: word followed by ':'
    let first_start = line.len() - trimmed.len();
    let mut pos = first_start;

    // Check if line starts with a label (identifier followed by ':')
    if let Some(colon_pos) = line[pos..].find(':') {
        let label_end = pos + colon_pos;
        if line[pos..label_end]
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_')
        {
            spans.push(SynSpan {
                kind: SynTok::Label,
                start: pos,
                len: colon_pos,
                text: line[pos..label_end].to_string(),
            });
            pos = label_end + 1; // skip the colon
            while pos < line.len() && line.as_bytes()[pos] == b' ' {
                pos += 1;
            }
        }
    }

    // Now parse instruction tokens from current position
    let comment_start = line[pos..].find(';').map(|i| pos + i);
    let code_end = comment_start.unwrap_or(line.len());
    let code = &line[pos..code_end];
    let code_offset = pos;

    if code.is_empty() {
        if let Some(cs) = comment_start {
            spans.push(SynSpan {
                kind: SynTok::Comment,
                start: cs,
                len: line.len() - cs,
                text: line[cs..].to_string(),
            });
        }
        return spans;
    }

    let mut token_pos = 0;
    let mut is_first_token = true;
    let tokens_str: Vec<&str> = code
        .split([',', ' ', '\t'])
        .filter(|s| !s.is_empty())
        .collect();

    for token in &tokens_str {
        let relative_start = code[token_pos..].find(*token).unwrap_or(0);
        let abs_start = code_offset + token_pos + relative_start;

        if is_first_token {
            let upper: String = token.chars().map(|c| c.to_ascii_uppercase()).collect();
            if OPCODES.contains(&upper.as_str()) {
                spans.push(SynSpan {
                    kind: SynTok::Opcode,
                    start: abs_start,
                    len: token.len(),
                    text: token.to_string(),
                });
            } else {
                spans.push(SynSpan {
                    kind: SynTok::Default,
                    start: abs_start,
                    len: token.len(),
                    text: token.to_string(),
                });
            }
            is_first_token = false;
        } else {
            if token.starts_with('r') || token.starts_with('R') {
                let reg_part = &token[1..];
                if reg_part.parse::<u32>().is_ok() {
                    spans.push(SynSpan {
                        kind: SynTok::Register,
                        start: abs_start,
                        len: token.len(),
                        text: token.to_string(),
                    });
                    token_pos = token_pos + relative_start + token.len();
                    continue;
                }
            }
            let is_number = token.chars().next().is_some_and(|c| c.is_ascii_digit())
                || token.starts_with("0x")
                || token.starts_with("0X")
                || token.starts_with("0b")
                || token.starts_with("0B")
                || (token.starts_with('-')
                    && token.len() > 1
                    && token[1..]
                        .chars()
                        .next()
                        .is_some_and(|c| c.is_ascii_digit()));
            if is_number {
                spans.push(SynSpan {
                    kind: SynTok::Number,
                    start: abs_start,
                    len: token.len(),
                    text: token.to_string(),
                });
            } else {
                spans.push(SynSpan {
                    kind: SynTok::Label,
                    start: abs_start,
                    len: token.len(),
                    text: token.to_string(),
                });
            }
        }
        token_pos = token_pos + relative_start + token.len();
    }

    if let Some(cs) = comment_start {
        spans.push(SynSpan {
            kind: SynTok::Comment,
            start: cs,
            len: line.len() - cs,
            text: line[cs..].to_string(),
        });
    }

    spans
}

pub struct Preprocessor {
    pub variables: HashMap<String, u32>,
}

impl Default for Preprocessor {
    fn default() -> Self {
        Self::new()
    }
}

impl Preprocessor {
    /// Create a new preprocessor with an empty variable table.
    pub fn new() -> Self {
        Self {
            variables: HashMap::new(),
        }
    }

    /// Preprocess source text based on syntax token types (the "font colors").
    pub fn preprocess(&mut self, source: &str) -> String {
        let mut output = String::new();

        for line in source.lines() {
            let spans = parse_syntax_line(line);
            if spans.is_empty() {
                output.push('\n');
                continue;
            }

            // Check if this is a formula line: first token is '='
            if spans[0].kind == SynTok::Formula && spans[0].text == "=" {
                // Collect formula tokens (everything after the '=')
                let ftokens: Vec<&SynSpan> = spans[1..]
                    .iter()
                    .filter(|s| s.kind != SynTok::Comment)
                    .collect();

                // Try to parse the formula expression and generate FORMULA directive
                if let Some(formula_asm) = self.parse_formula(&ftokens) {
                    output.push_str(&formula_asm);
                    output.push('\n');
                } else {
                    // Unparseable formula -- emit as comment for debugging
                    output.push_str("; [formula parse error] ");
                    output.push_str(
                        &spans
                            .iter()
                            .map(|s| s.text.as_str())
                            .collect::<Vec<_>>()
                            .join(" "),
                    );
                    output.push('\n');
                }
                continue;
            }

            // A line is a directive/macro if the first token is an Opcode from our macro set
            if spans[0].kind == SynTok::Opcode {
                let cmd = spans[0].text.to_uppercase();
                let handled = match cmd.as_str() {
                    "VAR" => {
                        // Pattern: VAR Label Number
                        if spans.len() >= 3
                            && spans[1].kind == SynTok::Label
                            && spans[2].kind == SynTok::Number
                        {
                            let name = spans[1].text.clone();
                            if let Ok(addr) = self.parse_imm(&spans[2].text) {
                                self.variables.insert(name, addr);
                            }
                        }
                        output.push_str(&format!(
                            "; VAR definition: {}\n",
                            if spans.len() > 1 { &spans[1].text } else { "" }
                        ));
                        true
                    }
                    "SET" => {
                        // Pattern: SET Label (Value)
                        if spans.len() >= 3 && spans[1].kind == SynTok::Label {
                            let var_name = &spans[1].text;
                            let val = &spans[2].text;
                            if let Some(&addr) = self.variables.get(var_name) {
                                output.push_str(&format!("LDI r28, {}\n", val));
                                output.push_str(&format!("LDI r29, 0x{:X}\n", addr));
                                output.push_str("STORE r29, r28\n");
                                true
                            } else {
                                false
                            }
                        } else {
                            false
                        }
                    }
                    "GET" => {
                        // Pattern: GET Register Label
                        if spans.len() >= 3
                            && spans[1].kind == SynTok::Register
                            && spans[2].kind == SynTok::Label
                        {
                            let reg = &spans[1].text;
                            let var_name = &spans[2].text;
                            if let Some(&addr) = self.variables.get(var_name) {
                                output.push_str(&format!("LDI r29, 0x{:X}\n", addr));
                                output.push_str(&format!("LOAD {}, r29\n", reg));
                                true
                            } else {
                                false
                            }
                        } else {
                            false
                        }
                    }
                    "INC" => {
                        // Pattern: INC Label
                        if spans.len() >= 2 && spans[1].kind == SynTok::Label {
                            let var_name = &spans[1].text;
                            if let Some(&addr) = self.variables.get(var_name) {
                                output.push_str(&format!("LDI r29, 0x{:X}\n", addr));
                                output.push_str("LOAD r28, r29\n");
                                output.push_str("LDI r27, 1\n");
                                output.push_str("ADD r28, r27\n");
                                output.push_str("STORE r29, r28\n");
                                true
                            } else {
                                false
                            }
                        } else {
                            false
                        }
                    }
                    "DEC" => {
                        // Pattern: DEC Label
                        if spans.len() >= 2 && spans[1].kind == SynTok::Label {
                            let var_name = &spans[1].text;
                            if let Some(&addr) = self.variables.get(var_name) {
                                output.push_str(&format!("LDI r29, 0x{:X}\n", addr));
                                output.push_str("LOAD r28, r29\n");
                                output.push_str("LDI r27, 1\n");
                                output.push_str("SUB r28, r27\n");
                                output.push_str("STORE r29, r28\n");
                                true
                            } else {
                                false
                            }
                        } else {
                            false
                        }
                    }
                    _ => false,
                };

                if handled {
                    continue;
                }
                // If not handled (unknown macro or missing variable), fall through to passthrough
            }

            // Normal line: preserve original text, only substitute known variable names
            // in Label-kind tokens. This keeps commas, whitespace, and unknown tokens intact.
            let mut result = line.to_string();
            // Iterate spans in reverse so substitutions don't shift positions
            for span in spans.iter().rev() {
                if span.kind == SynTok::Label {
                    if let Some(&addr) = self.variables.get(&span.text) {
                        // Replace the token at [span.start, span.start + span.len) with the address
                        let addr_str = format!("0x{:X}", addr);
                        result.replace_range(span.start..span.start + span.len, &addr_str);
                    }
                }
            }
            output.push_str(&result);
            output.push('\n');
        }

        output
    }

    /// Parse a formula expression from spans and generate FORMULA assembler directive.
    ///
    /// Supported forms:
    ///   = <dep> <op> <dep>       -- binary operation (ADD, SUB, MUL, DIV, AND, OR, XOR, MAX, MIN, MOD, SHL, SHR)
    ///   = <op> <dep>             -- unary operation (NOT, COPY/~)
    ///   = <dep>                  -- identity copy (single dep)
    ///
    /// Dependencies can be:
    ///   - Numbers: canvas buffer indices (e.g., 32, 0x100)
    ///   - Variable names: resolved via self.variables to canvas addresses
    ///   - NOT supported: register names (r0, r1) -- formulas operate on canvas cells, not registers
    fn parse_formula(&self, ftokens: &[&SynSpan]) -> Option<String> {
        if ftokens.is_empty() {
            return None;
        }

        // Resolve a dependency token to a canvas buffer index
        let resolve_dep = |span: &SynSpan| -> Option<usize> {
            match span.kind {
                SynTok::Number => {
                    if let Ok(val) = self.parse_imm(&span.text) {
                        Some(val as usize)
                    } else {
                        None
                    }
                }
                SynTok::Label => {
                    // Try to resolve as a variable name
                    self.variables.get(&span.text).map(|&v| v as usize)
                }
                _ => None,
            }
        };

        // Map operator token text to FormulaOp name
        let op_from_token = |token: &str| -> Option<&'static str> {
            match token.to_uppercase().as_str() {
                "+" | "ADD" => Some("ADD"),
                "-" | "SUB" => Some("SUB"),
                "*" | "MUL" => Some("MUL"),
                "/" | "DIV" => Some("DIV"),
                "&" | "AND" => Some("AND"),
                "|" | "OR" => Some("OR"),
                "^" | "XOR" => Some("XOR"),
                "%" | "MOD" => Some("MOD"),
                "<<" | "SHL" => Some("SHL"),
                ">>" | "SHR" => Some("SHR"),
                "MAX" => Some("MAX"),
                "MIN" => Some("MIN"),
                _ => None,
            }
        };

        // Determine the formula form
        if ftokens.len() == 1 {
            // Form: = <dep> -- identity/copy
            if let Some(dep_idx) = resolve_dep(ftokens[0]) {
                return Some(format!(
                    "; formula: copy from {}\nFORMULA 0, COPY, {}",
                    dep_idx, dep_idx
                ));
            }
            return None;
        }

        if ftokens.len() == 2 {
            // Form: = <unary_op> <dep>  or  = <dep> <unary_op>
            // Check for NOT/~
            let first_text = ftokens[0].text.trim();
            if first_text == "~" || first_text.to_uppercase() == "NOT" {
                if let Some(dep_idx) = resolve_dep(ftokens[1]) {
                    return Some(format!(
                        "; formula: NOT {}\nFORMULA 0, NOT, {}",
                        dep_idx, dep_idx
                    ));
                }
            }
            return None;
        }

        if ftokens.len() == 3 {
            // Form: = <dep0> <op> <dep1> -- standard binary formula
            let dep0 = resolve_dep(ftokens[0])?;
            let op_name = op_from_token(&ftokens[1].text)?;
            let dep1 = resolve_dep(ftokens[2])?;
            return Some(format!(
                "; formula: {} {} {}\nFORMULA 0, {}, {}, {}",
                dep0,
                ftokens[1].text.trim(),
                dep1,
                op_name,
                dep0,
                dep1
            ));
        }

        // Longer expressions not yet supported (would need chained formulas)
        None
    }

    fn parse_imm(&self, s: &str) -> Result<u32, String> {
        let s = s.trim();
        if s.starts_with("0x") || s.starts_with("0X") {
            u32::from_str_radix(&s[2..], 16).map_err(|e| e.to_string())
        } else if s.starts_with("0b") || s.starts_with("0B") {
            u32::from_str_radix(&s[2..], 2).map_err(|e| e.to_string())
        } else {
            s.parse::<u32>().map_err(|e| e.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_passthrough_preserves_commas() {
        let mut pp = Preprocessor::new();
        let src = "  MOV r6, r1\n  ADD r0, r1\n";
        let result = pp.preprocess(src);
        assert!(
            result.contains("MOV r6, r1"),
            "passthrough should preserve commas, got: {:?}",
            result
        );
        assert!(
            result.contains("ADD r0, r1"),
            "passthrough should preserve commas, got: {:?}",
            result
        );
    }

    #[test]
    fn test_var_set_get() {
        let mut pp = Preprocessor::new();
        let src = "VAR score 0x4000\nSET score, 42\nGET r10, score\nHALT\n";
        let result = pp.preprocess(src);
        assert!(
            result.contains("LDI r28, 42"),
            "SET should expand, got: {:?}",
            result
        );
        assert!(
            result.contains("LDI r29, 0x4000"),
            "SET should use r29, got: {:?}",
            result
        );
        assert!(
            result.contains("STORE r29, r28"),
            "SET should store, got: {:?}",
            result
        );
        assert!(
            result.contains("LOAD r10, r29"),
            "GET should load, got: {:?}",
            result
        );
    }

    #[test]
    fn test_inc_dec() {
        let mut pp = Preprocessor::new();
        let src = "VAR counter 0x5000\nINC counter\nDEC counter\n";
        let result = pp.preprocess(src);
        assert!(
            result.contains("ADD r28, r27"),
            "INC should add 1, got: {:?}",
            result
        );
        assert!(
            result.contains("SUB r28, r27"),
            "DEC should sub 1, got: {:?}",
            result
        );
    }

    #[test]
    fn test_unknown_opcode_passthrough() {
        let mut pp = Preprocessor::new();
        let src = "FOOBAR r1, r2\n";
        let result = pp.preprocess(src);
        assert!(
            result.contains("FOOBAR r1, r2"),
            "unknown opcodes should pass through verbatim, got: {:?}",
            result
        );
    }

    #[test]
    fn test_variable_resolution_in_args() {
        let mut pp = Preprocessor::new();
        let src = "VAR dst 0x4000\nLDI r4, dst\nSTORE r4, r1\n";
        let result = pp.preprocess(src);
        assert!(
            result.contains("LDI r4, 0x4000"),
            "variable in arg should resolve, got: {:?}",
            result
        );
    }

    #[test]
    fn test_formula_binary_add() {
        let mut pp = Preprocessor::new();
        let src = "= 10 + 20\n";
        let result = pp.preprocess(src);
        assert!(
            result.contains("FORMULA 0, ADD, 10, 20"),
            "binary add formula should generate FORMULA directive, got: {:?}",
            result
        );
    }

    #[test]
    fn test_formula_binary_mul() {
        let mut pp = Preprocessor::new();
        let src = "= 5 * 8\n";
        let result = pp.preprocess(src);
        assert!(
            result.contains("FORMULA 0, MUL, 5, 8"),
            "binary mul formula should generate FORMULA directive, got: {:?}",
            result
        );
    }

    #[test]
    fn test_formula_with_variables() {
        let mut pp = Preprocessor::new();
        pp.variables.insert("score".to_string(), 0x100);
        pp.variables.insert("bonus".to_string(), 0x200);
        let src = "= score + bonus\n";
        let result = pp.preprocess(src);
        assert!(
            result.contains("FORMULA 0, ADD, 256, 512"),
            "formula with variable deps should resolve to addresses, got: {:?}",
            result
        );
    }

    #[test]
    fn test_formula_unary_not() {
        let mut pp = Preprocessor::new();
        let src = "= ~ 42\n";
        let result = pp.preprocess(src);
        assert!(
            result.contains("FORMULA 0, NOT, 42"),
            "unary NOT formula should generate FORMULA directive, got: {:?}",
            result
        );
    }

    #[test]
    fn test_formula_identity_copy() {
        let mut pp = Preprocessor::new();
        let src = "= 99\n";
        let result = pp.preprocess(src);
        assert!(
            result.contains("FORMULA 0, COPY, 99"),
            "single dep formula should generate COPY directive, got: {:?}",
            result
        );
    }

    #[test]
    fn test_formula_all_binary_ops() {
        let ops = [
            ("+", "ADD"),
            ("-", "SUB"),
            ("*", "MUL"),
            ("/", "DIV"),
            ("&", "AND"),
            ("|", "OR"),
            ("^", "XOR"),
            ("%", "MOD"),
            ("<<", "SHL"),
            (">>", "SHR"),
        ];
        for (sym, name) in &ops {
            let mut pp = Preprocessor::new();
            let src = format!("= 1 {} 2\n", sym);
            let result = pp.preprocess(&src);
            assert!(
                result.contains(&format!("FORMULA 0, {}, 1, 2", name)),
                "formula with {} should generate {}, got: {:?}",
                sym,
                name,
                result
            );
        }
    }

    #[test]
    fn test_formula_max_min() {
        let mut pp = Preprocessor::new();
        let src = "= 10 MAX 20\n";
        let result = pp.preprocess(src);
        assert!(
            result.contains("FORMULA 0, MAX, 10, 20"),
            "MAX formula should generate FORMULA directive, got: {:?}",
            result
        );

        let mut pp2 = Preprocessor::new();
        let src2 = "= 10 MIN 20\n";
        let result2 = pp2.preprocess(&src2);
        assert!(
            result2.contains("FORMULA 0, MIN, 10, 20"),
            "MIN formula should generate FORMULA directive, got: {:?}",
            result2
        );
    }

    #[test]
    fn test_formula_parse_error_passthrough() {
        let mut pp = Preprocessor::new();
        // Unresolvable variable in formula
        let src = "= unknown_var + 10\n";
        let result = pp.preprocess(src);
        assert!(
            result.contains("formula parse error"),
            "unparseable formula should emit error comment, got: {:?}",
            result
        );
    }

    #[test]
    fn test_formula_mixed_with_normal_code() {
        let mut pp = Preprocessor::new();
        let src = "LDI r1, 10\n= 5 + 6\nSTORE r1, r2\n";
        let result = pp.preprocess(src);
        assert!(
            result.contains("LDI r1, 10"),
            "normal code should pass through, got: {:?}",
            result
        );
        assert!(
            result.contains("FORMULA 0, ADD, 5, 6"),
            "formula should be expanded, got: {:?}",
            result
        );
        assert!(
            result.contains("STORE r1, r2"),
            "code after formula should pass through, got: {:?}",
            result
        );
    }

    #[test]
    fn test_formula_hex_deps() {
        let mut pp = Preprocessor::new();
        let src = "= 0x10 + 0x20\n";
        let result = pp.preprocess(src);
        assert!(
            result.contains("FORMULA 0, ADD, 16, 32"),
            "hex deps should be resolved to decimal, got: {:?}",
            result
        );
    }
}
