// preprocessor.rs -- Macro expansion and variable resolution for Geometry OS
//
// This sits between the canvas color grid and the final assembler.
// It translates high-level constructs (SET, GET, variables) into raw opcodes.
// It uses the same syntax tokenization as the rendering pipeline (the "font colors").

use std::collections::HashMap;

/// Valid opcodes for syntax highlighting and preprocessing.
/// Must be kept in sync with assembler.rs opcode list.
pub const OPCODES: &[&str] = &[
    "HALT", "NOP", "FRAME", "LDI", "LOAD", "STORE", "ADD", "SUB", "MUL", "DIV",
    "AND", "OR", "XOR", "SHL", "SHR", "MOD", "SAR", "MOV",
    "JMP", "JZ", "JNZ", "CALL", "RET", "BLT", "BGE",
    "PSET", "PSETI", "FILL", "RECTF", "TEXT", "LINE", "CIRCLE", "SCROLL",
    "IKEY", "RAND", "NEG", "CMP", "PUSH", "POP", "BEEP", "ASM",
    "SPAWN", "KILL", "PEEK", "SPRITE", "TILEMAP",
    "SYSCALL", "RETK", "OPEN", "READ", "WRITE", "CLOSE", "SEEK", "LS",
    "YIELD", "SLEEP", "SETPRIORITY",
    "PIPE", "MSGSND", "MSGRCV", "IOCTL",
    "GETENV", "SETENV", "GETPID",
    "EXEC", "WRITESTR", "READLN", "WAITPID", "EXECP", "CHDIR", "GETCWD",
    "SHUTDOWN",
    // Preprocessor macros (not real opcodes, but recognized as Opcode tokens)
    "VAR", "SET", "GET", "INC", "DEC",
];

/// Token types produced by the syntax highlighter.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SynTok {
    Opcode,
    Register,
    Number,
    Label,
    Comment,
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
        spans.push(SynSpan { kind: SynTok::Comment, start: 0, len: line.len(), text: line.to_string() });
        return spans;
    }

    // Check for label definition: word followed by ':'
    let first_start = line.len() - trimmed.len();
    let mut pos = first_start;

    // Check if line starts with a label (identifier followed by ':')
    if let Some(colon_pos) = line[pos..].find(':') {
        let label_end = pos + colon_pos;
        if line[pos..label_end].chars().all(|c| c.is_alphanumeric() || c == '_') {
            spans.push(SynSpan { kind: SynTok::Label, start: pos, len: colon_pos, text: line[pos..label_end].to_string() });
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
            spans.push(SynSpan { kind: SynTok::Comment, start: cs, len: line.len() - cs, text: line[cs..].to_string() });
        }
        return spans;
    }

    let mut token_pos = 0;
    let mut is_first_token = true;
    let tokens_str: Vec<&str> = code.split(|c: char| c == ',' || c == ' ' || c == '\t')
        .filter(|s| !s.is_empty())
        .collect();

    for token in &tokens_str {
        let relative_start = code[token_pos..].find(*token).unwrap_or(0);
        let abs_start = code_offset + token_pos + relative_start;

        if is_first_token {
            let upper: String = token.chars().map(|c| c.to_ascii_uppercase()).collect();
            if OPCODES.contains(&upper.as_str()) {
                spans.push(SynSpan { kind: SynTok::Opcode, start: abs_start, len: token.len(), text: token.to_string() });
            } else {
                spans.push(SynSpan { kind: SynTok::Default, start: abs_start, len: token.len(), text: token.to_string() });
            }
            is_first_token = false;
        } else {
            if token.starts_with('r') || token.starts_with('R') {
                let reg_part = &token[1..];
                if reg_part.parse::<u32>().is_ok() {
                    spans.push(SynSpan { kind: SynTok::Register, start: abs_start, len: token.len(), text: token.to_string() });
                    token_pos = token_pos + relative_start + token.len();
                    continue;
                }
            }
            let is_number = token.chars().next().map_or(false, |c| c.is_ascii_digit())
                || token.starts_with("0x") || token.starts_with("0X")
                || token.starts_with("0b") || token.starts_with("0B")
                || (token.starts_with('-') && token.len() > 1 && token[1..].chars().next().map_or(false, |c| c.is_ascii_digit()));
            if is_number {
                spans.push(SynSpan { kind: SynTok::Number, start: abs_start, len: token.len(), text: token.to_string() });
            } else {
                spans.push(SynSpan { kind: SynTok::Label, start: abs_start, len: token.len(), text: token.to_string() });
            }
        }
        token_pos = token_pos + relative_start + token.len();
    }

    if let Some(cs) = comment_start {
        spans.push(SynSpan { kind: SynTok::Comment, start: cs, len: line.len() - cs, text: line[cs..].to_string() });
    }

    spans
}

pub struct Preprocessor {
    pub variables: HashMap<String, u32>,
}

impl Preprocessor {
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

            // A line is a directive/macro if the first token is an Opcode from our macro set
            if spans[0].kind == SynTok::Opcode {
                let cmd = spans[0].text.to_uppercase();
                let handled = match cmd.as_str() {
                    "VAR" => {
                        // Pattern: VAR Label Number
                        if spans.len() >= 3 && spans[1].kind == SynTok::Label && spans[2].kind == SynTok::Number {
                            let name = spans[1].text.clone();
                            if let Ok(addr) = self.parse_imm(&spans[2].text) {
                                self.variables.insert(name, addr);
                            }
                        }
                        output.push_str(&format!("; VAR definition: {}\n", if spans.len() > 1 { &spans[1].text } else { "" }));
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
                        if spans.len() >= 3 && spans[1].kind == SynTok::Register && spans[2].kind == SynTok::Label {
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
        assert!(result.contains("MOV r6, r1"), "passthrough should preserve commas, got: {:?}", result);
        assert!(result.contains("ADD r0, r1"), "passthrough should preserve commas, got: {:?}", result);
    }

    #[test]
    fn test_var_set_get() {
        let mut pp = Preprocessor::new();
        let src = "VAR score 0x4000\nSET score, 42\nGET r10, score\nHALT\n";
        let result = pp.preprocess(src);
        assert!(result.contains("LDI r28, 42"), "SET should expand, got: {:?}", result);
        assert!(result.contains("LDI r29, 0x4000"), "SET should use r29, got: {:?}", result);
        assert!(result.contains("STORE r29, r28"), "SET should store, got: {:?}", result);
        assert!(result.contains("LOAD r10, r29"), "GET should load, got: {:?}", result);
    }

    #[test]
    fn test_inc_dec() {
        let mut pp = Preprocessor::new();
        let src = "VAR counter 0x5000\nINC counter\nDEC counter\n";
        let result = pp.preprocess(src);
        assert!(result.contains("ADD r28, r27"), "INC should add 1, got: {:?}", result);
        assert!(result.contains("SUB r28, r27"), "DEC should sub 1, got: {:?}", result);
    }

    #[test]
    fn test_unknown_opcode_passthrough() {
        let mut pp = Preprocessor::new();
        let src = "FOOBAR r1, r2\n";
        let result = pp.preprocess(src);
        assert!(result.contains("FOOBAR r1, r2"), "unknown opcodes should pass through verbatim, got: {:?}", result);
    }

    #[test]
    fn test_variable_resolution_in_args() {
        let mut pp = Preprocessor::new();
        let src = "VAR dst 0x4000\nLDI r4, dst\nSTORE r4, r1\n";
        let result = pp.preprocess(src);
        assert!(result.contains("LDI r4, 0x4000"), "variable in arg should resolve, got: {:?}", result);
    }
}
