// glyph_backend.rs -- GlyphLang compiler for Geometry OS
//
// Translates .glyph spatial assembly (stack-based) into Geometry OS assembly.
// Then assembles to bytecode that runs on the VM.
//
// Mapping:
//   GlyphLang Data Stack -> Geometry OS Stack (r30 = SP, grows down from 0x8000)
//   GlyphLang Registers (a-z) -> Geometry OS Registers (r1-r26)
//   r0 -> reserved for CMP results (NEVER use for variables)
//   r27-r29 -> reserved for backend scratch (transient, not preserved)
//
// GlyphLang opcodes (from spatial assembly spec):
//   0-9   Push single-digit integer
//   +-*/  Arithmetic (a b -- r)
//   >=<   Comparison (a b -- bool, pushes 1 or 0)
//   ?     Conditional (c t f -- r): if c!=0 push t, else push f
//   L     Loop (s e -- [r]): generate range from s to e
//   S     Mitosis/Spawn (o -- id): clone VM at offset o
//   M     Mutator (v o -- ): overwrite code at IP+offset
//   .     Output (v -- ): draw value as pixel
//   @     Halt
//   a-z   Store: pop stack -> register
//   A-Z   Load: push register -> stack

// ── Token ──────────────────────────────────────────────────────

/// A single GlyphLang token produced by the lexer.
#[derive(Debug, Clone, PartialEq)]
pub enum GlyphToken {
    /// Push a number onto the stack
    Number(u32),
    /// Arithmetic: +, -, *, /
    Op(char),
    /// Comparison: >, <, =
    Cmp(char),
    /// Conditional: ? (c t f -- r)
    Conditional,
    /// Loop: L (s e -- [r])
    Loop,
    /// Mitosis/Spawn: S (o -- id)
    Spawn,
    /// Mutator: M (v o -- )
    Mutate,
    /// Output: . (v -- )
    Output,
    /// Halt: @
    Halt,
    /// Store to register (lowercase a-z) -> register index 1-26
    StoreReg(u8),
    /// Load from register (uppercase A-Z) -> register index 1-26
    LoadReg(u8),
    /// Push a string address onto the stack
    StringLiteral(String),
    /// Graphics: [ (x y w h color -- )
    Rectf,
    /// Text: { (x y addr color bg -- )
    DrawText,
    /// Frame: ! (yield)
    Frame,
    /// Input: ^ (-- key)
    Ikey,
    /// Screen: | (color -- )
    Fill,
    /// Filesystem: $ (addr -- count)
    Ls,
    /// Label definition: :name
    Label(String),
    /// Jump to label: >name
    Jump(String),
    /// Execution: & (addr -- )
    Exec,
    /// Jump if zero: (name (pop stack, if 0 jump to name)
    Jz(String),
    /// Jump if not zero: )name (pop stack, if !0 jump to name)
    Jnz(String),
}

// ── Lexer Error ────────────────────────────────────────────────

#[derive(Debug)]
pub struct GlyphLexError {
    pub pos: usize,
    pub message: String,
}

impl std::fmt::Display for GlyphLexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "GlyphLang lexer error at position {}: {}",
            self.pos, self.message
        )
    }
}

// ── Lexer ──────────────────────────────────────────────────────

/// Tokenize a .glyph source string into a Vec of GlyphTokens.
/// Ignores whitespace, comments (# to end of line), and unknown characters (with warning).
pub fn lex(source: &str) -> Result<Vec<GlyphToken>, GlyphLexError> {
    let mut tokens = Vec::new();
    let mut chars = source.char_indices().peekable();

    while let Some((_pos, ch)) = chars.next() {
        match ch {
            // Whitespace - skip
            ' ' | '\t' | '\n' | '\r' => {}

            // Comment - skip to end of line
            '#' => {
                while let Some(&(_, c)) = chars.peek() {
                    if c == '\n' {
                        break;
                    }
                    chars.next();
                }
            }

            // Numbers 0-9 and hex 0x
            '0'..='9' => {
                let mut s = ch.to_string();
                while let Some(&(_, c)) = chars.peek() {
                    if c.is_ascii_hexdigit() || c == 'x' || c == 'X' {
                        s.push(c);
                        chars.next();
                    } else {
                        break;
                    }
                }

                let val = if s.starts_with("0x") || s.starts_with("0X") {
                    u32::from_str_radix(&s[2..], 16).unwrap_or(0)
                } else {
                    s.parse::<u32>().unwrap_or(0)
                };
                tokens.push(GlyphToken::Number(val));
            }

            // Arithmetic operators
            '+' | '-' | '*' | '/' => {
                tokens.push(GlyphToken::Op(ch));
            }

            // Comparison operators
            '>' | '<' | '=' => {
                tokens.push(GlyphToken::Cmp(ch));
            }

            // Control flow
            '?' => tokens.push(GlyphToken::Conditional),
            'L' => tokens.push(GlyphToken::Loop),

            // Metamorphic
            'M' => tokens.push(GlyphToken::Mutate),

            // Biological
            'S' => tokens.push(GlyphToken::Spawn),

            // I/O
            '.' => tokens.push(GlyphToken::Output),
            '@' => tokens.push(GlyphToken::Halt),

            // Register store (lowercase a-z)
            'a'..='z' => {
                let reg_idx = (ch as u8 - b'a' + 1) as u8; // a=1, b=2, ..., z=26
                tokens.push(GlyphToken::StoreReg(reg_idx));
            }

            // Register load (uppercase A-Z)
            'A'..='Z' => {
                let reg_idx = (ch as u8 - b'A' + 1) as u8; // A=1, B=2, ..., Z=26
                tokens.push(GlyphToken::LoadReg(reg_idx));
            }

            // String literals: "hello"
            '"' => {
                let mut s = String::new();
                while let Some((_, c)) = chars.next() {
                    if c == '"' {
                        break;
                    }
                    s.push(c);
                }
                tokens.push(GlyphToken::StringLiteral(s));
            }

            // New opcodes
            '[' => tokens.push(GlyphToken::Rectf),
            '{' => tokens.push(GlyphToken::DrawText),
            '!' => tokens.push(GlyphToken::Frame),
            '^' => tokens.push(GlyphToken::Ikey),
            '|' => tokens.push(GlyphToken::Fill),
            '$' => tokens.push(GlyphToken::Ls),

            // Labels: :start
            ':' => {
                let mut name = String::new();
                while let Some(&(_, c)) = chars.peek() {
                    if c.is_alphanumeric() || c == '_' {
                        name.push(c);
                        chars.next();
                    } else {
                        break;
                    }
                }
                tokens.push(GlyphToken::Label(name));
            }

            // Jumps: ~start
            '~' => {
                let mut name = String::new();
                while let Some(&(_, c)) = chars.peek() {
                    if c.is_alphanumeric() || c == '_' {
                        name.push(c);
                        chars.next();
                    } else {
                        break;
                    }
                }
                tokens.push(GlyphToken::Jump(name));
            }

            '&' => tokens.push(GlyphToken::Exec),

            '(' => {
                let mut name = String::new();
                while let Some(&(_, c)) = chars.peek() {
                    if c.is_alphanumeric() || c == '_' {
                        name.push(c);
                        chars.next();
                    } else {
                        break;
                    }
                }
                tokens.push(GlyphToken::Jz(name));
            }

            ')' => {
                let mut name = String::new();
                while let Some(&(_, c)) = chars.peek() {
                    if c.is_alphanumeric() || c == '_' {
                        name.push(c);
                        chars.next();
                    } else {
                        break;
                    }
                }
                tokens.push(GlyphToken::Jnz(name));
            }

            // Unknown character - skip silently
            _ => {}
        }
    }

    Ok(tokens)
}

// ── Compiler Error ─────────────────────────────────────────────

#[derive(Debug)]
pub struct GlyphCompileError {
    pub message: String,
}

impl std::fmt::Display for GlyphCompileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "GlyphLang compile error: {}", self.message)
    }
}

// ── Compiler (Token -> Assembly) ───────────────────────────────

/// Compile a .glyph source string into Geometry OS assembly text.
/// The output can be fed to the assembler directly.
pub fn compile_glyph(source: &str) -> Result<String, GlyphCompileError> {
    let tokens = lex(source).map_err(|e| GlyphCompileError {
        message: e.to_string(),
    })?;
    compile_tokens(&tokens)
}

/// Compile pre-lexed tokens into Geometry OS assembly.
pub fn compile_tokens(tokens: &[GlyphToken]) -> Result<String, GlyphCompileError> {
    let mut asm = String::new();
    let mut strings = Vec::new();
    let mut label_counter: u32 = 0;

    // Fresh label generator to avoid collisions
    let next_label = |counter: &mut u32| -> String {
        let label = format!("_gl{}", *counter);
        *counter += 1;
        label
    };

    asm.push_str("; Compiled from GlyphLang\n");
    asm.push_str("LDI r30, 0x8000 ; Initialize Stack Pointer\n");
    asm.push_str("LDI r29, 0 ; output x counter\n");
    asm.push_str("LDI r28, 0 ; output y counter\n");
    asm.push('\n');

    for token in tokens {
        match token {
            GlyphToken::Number(n) => {
                asm.push_str(&format!("LDI r27, {}\n", n));
                asm.push_str("PUSH r27\n");
            }

            GlyphToken::Op(op) => {
                // All binary ops: pop b, pop a, compute, push result
                asm.push_str("POP r28 ; b\n");
                asm.push_str("POP r27 ; a\n");
                match op {
                    '+' => asm.push_str("ADD r27, r28\n"),
                    '-' => asm.push_str("SUB r27, r28\n"),
                    '*' => asm.push_str("MUL r27, r28\n"),
                    '/' => asm.push_str("DIV r27, r28\n"),
                    _ => {
                        return Err(GlyphCompileError {
                            message: format!("Unknown arithmetic operator: {}", op),
                        })
                    }
                }
                asm.push_str("PUSH r27\n");
            }

            GlyphToken::Cmp(op) => {
                // Comparison: pop b, pop a, CMP, push 1 or 0
                asm.push_str("POP r28 ; b\n");
                asm.push_str("POP r27 ; a\n");
                asm.push_str("CMP r27, r28\n");
                match op {
                    '>' => {
                        // r0 = 1 if a > b, so push 1 if r0 == 1
                        let lbl_true = next_label(&mut label_counter);
                        let lbl_done = next_label(&mut label_counter);
                        // r0 == 1 means greater
                        asm.push_str(&format!("LDI r27, 1\n"));
                        asm.push_str(&format!("CMP r0, r27\n")); // compare CMP result with 1
                        asm.push_str(&format!("JZ r0, {} ; greater\n", lbl_true));
                        asm.push_str(&format!("LDI r27, 0\n"));
                        asm.push_str(&format!("JMP {}\n", lbl_done));
                        asm.push_str(&format!("{}:\n", lbl_true));
                        asm.push_str(&format!("LDI r27, 1\n"));
                        asm.push_str(&format!("{}:\n", lbl_done));
                    }
                    '<' => {
                        // r0 = 0xFFFFFFFF if a < b
                        let lbl_true = next_label(&mut label_counter);
                        let lbl_done = next_label(&mut label_counter);
                        asm.push_str("LDI r27, 0xFFFFFFFF\n");
                        asm.push_str("CMP r0, r27\n");
                        asm.push_str(&format!("JZ r0, {} ; less-than\n", lbl_true));
                        asm.push_str("LDI r27, 0\n");
                        asm.push_str(&format!("JMP {}\n", lbl_done));
                        asm.push_str(&format!("{}:\n", lbl_true));
                        asm.push_str("LDI r27, 1\n");
                        asm.push_str(&format!("{}:\n", lbl_done));
                    }
                    '=' => {
                        // r0 = 0 if a == b
                        let lbl_done = next_label(&mut label_counter);
                        asm.push_str("LDI r27, 1 ; assume equal\n");
                        asm.push_str(&format!("JZ r0, {} ; is zero = equal\n", lbl_done));
                        asm.push_str("LDI r27, 0 ; not equal\n");
                        asm.push_str(&format!("{}:\n", lbl_done));
                    }
                    _ => {
                        return Err(GlyphCompileError {
                            message: format!("Unknown comparison operator: {}", op),
                        })
                    }
                }
                asm.push_str("PUSH r27\n");
            }

            GlyphToken::Conditional => {
                // ? pops condition c, true_val t, false_val f
                // Stack order: c was pushed first, then t, then f
                // So pop order: f (top), t, c (bottom)
                // Wait - per the spec: ? ( c t f -- r): if c!=0 push t, else push f
                // The stack has c at bottom, then t, then f at top
                // So: pop f, pop t, pop c
                asm.push_str("POP r28 ; f (false value)\n");
                asm.push_str("POP r27 ; t (true value)\n");
                asm.push_str("POP r29 ; c (condition)\n");
                let lbl_true = next_label(&mut label_counter);
                let lbl_done = next_label(&mut label_counter);
                // If c != 0, push t; else push f
                asm.push_str(&format!("JNZ r29, {}\n", lbl_true));
                asm.push_str("PUSH r28 ; push f\n");
                asm.push_str(&format!("JMP {}\n", lbl_done));
                asm.push_str(&format!("{}:\n", lbl_true));
                asm.push_str("PUSH r27 ; push t\n");
                asm.push_str(&format!("{}:\n", lbl_done));
            }

            GlyphToken::Loop => {
                // L (s e -- [r]): pop end, pop start, generate range
                // For now, output a simple counted loop that draws each value
                // This is a complex opcode - we generate a loop structure
                let lbl_loop = next_label(&mut label_counter);
                let lbl_done = next_label(&mut label_counter);
                asm.push_str("POP r28 ; e (end)\n");
                asm.push_str("POP r27 ; s (start)\n");
                asm.push_str(&format!("{}:\n", lbl_loop));
                // Draw current value
                asm.push_str("PUSH r27\n");
                // Output the current value
                asm.push_str("POP r29 ; value to output\n");
                asm.push_str("LDI r25, 0x00FF00 ; green pixel\n");
                asm.push_str("PSET r29, r28, r25 ; draw at (value, end)\n");
                asm.push_str("LDI r26, 1\n");
                asm.push_str("ADD r27, r26 ; i++\n");
                asm.push_str("CMP r27, r28\n");
                // BLT branches when r0 == 0xFFFFFFFF (less than)
                asm.push_str(&format!("BLT r0, {}\n", lbl_loop));
                asm.push_str(&format!("{}:\n", lbl_done));
            }

            GlyphToken::Spawn => {
                // S (o -- id): spawn a child process at offset
                // For now, emit SPAWN and push the PID
                asm.push_str("POP r27 ; offset\n");
                // Note: SPAWN takes address in register, not offset
                // We treat the value as the address directly
                asm.push_str("SPAWN r27\n");
                asm.push_str("PUSH r27 ; push PID\n");
            }

            GlyphToken::Mutate => {
                // M (v o -- ): self-modifying code
                // Pop offset and value, store value at address (base + offset)
                // This maps to STORE at computed address
                asm.push_str("POP r27 ; offset\n");
                asm.push_str("POP r28 ; value\n");
                // Store value at offset (treat as RAM address)
                asm.push_str("STORE r27, r28\n");
            }

            GlyphToken::Output => {
                // . (v -- ): output value as a colored pixel
                asm.push_str("POP r27 ; value to output\n");
                // Draw the value as a pixel at current output position
                // Use a simple pattern: value mod 256 for x, y increments
                asm.push_str(
                    "PSET r27, r28, r27 ; draw pixel at (value, y_counter) with value as color\n",
                );
            }

            GlyphToken::Halt => {
                asm.push_str("HALT\n");
            }

            GlyphToken::StoreReg(idx) => {
                // Pop stack into register
                asm.push_str(&format!("POP r{}\n", idx));
            }

            GlyphToken::LoadReg(idx) => {
                // Push register onto stack
                asm.push_str(&format!("PUSH r{}\n", idx));
            }

            GlyphToken::StringLiteral(s) => {
                let lbl = format!("_str{}", strings.len());
                strings.push((lbl.clone(), s.clone()));
                asm.push_str(&format!("LDI r27, {}\n", lbl));
                asm.push_str("PUSH r27\n");
            }

            GlyphToken::Rectf => {
                // x y w h color [
                asm.push_str("POP r29 ; color\n");
                asm.push_str("POP r28 ; h\n");
                asm.push_str("POP r27 ; w\n");
                asm.push_str("POP r26 ; y\n");
                asm.push_str("POP r25 ; x\n");
                asm.push_str("RECTF r25, r26, r27, r28, r29\n");
            }

            GlyphToken::DrawText => {
                // x y addr color bg {
                asm.push_str("POP r29 ; bg\n");
                asm.push_str("POP r28 ; color\n");
                asm.push_str("POP r27 ; addr\n");
                asm.push_str("POP r26 ; y\n");
                asm.push_str("POP r25 ; x\n");
                asm.push_str("DRAWTEXT r25, r26, r27, r28, r29\n");
            }

            GlyphToken::Frame => {
                asm.push_str("FRAME\n");
            }

            GlyphToken::Ikey => {
                asm.push_str("IKEY r27\n");
                asm.push_str("PUSH r27\n");
            }

            GlyphToken::Fill => {
                asm.push_str("POP r27 ; color\n");
                asm.push_str("FILL r27\n");
            }

            GlyphToken::Ls => {
                // addr $ -- count
                asm.push_str("POP r27 ; addr\n");
                asm.push_str("LS r27\n");
                asm.push_str("PUSH r0 ; push file count result\n");
            }

            GlyphToken::Label(name) => {
                asm.push_str(&format!("{}:\n", name));
            }

            GlyphToken::Jump(name) => {
                asm.push_str(&format!("JMP {}\n", name));
            }

            GlyphToken::Exec => {
                // addr &
                asm.push_str("POP r27 ; addr\n");
                asm.push_str("EXEC r27\n");
            }

            GlyphToken::Jz(name) => {
                asm.push_str("POP r27 ; condition\n");
                asm.push_str(&format!("JZ r27, {}\n", name));
            }

            GlyphToken::Jnz(name) => {
                asm.push_str("POP r27 ; condition\n");
                asm.push_str(&format!("JNZ r27, {}\n", name));
            }
        }
    }

    // Ensure program always ends with HALT
    if !asm.contains("HALT") {
        asm.push_str("HALT\n");
    }

    // Append strings
    if !strings.is_empty() {
        asm.push('\n');
        for (lbl, s) in strings {
            asm.push_str(&format!("{}:\n", lbl));
            asm.push_str(&format!("  .str \"{}\"\n", s));
        }
    }

    Ok(asm)
}

// ── Full Pipeline: GlyphLang source -> bytecode ────────────────

/// Compile GlyphLang source directly to bytecode using the Geometry OS assembler.
/// Returns the assembled bytecode ready to load into a VM.
pub fn compile_glyph_to_bytecode(
    source: &str,
) -> Result<crate::assembler::AsmResult, GlyphCompileError> {
    let asm_text = compile_glyph(source)?;
    crate::assembler::assemble(&asm_text, 0).map_err(|e| GlyphCompileError {
        message: format!("Assembly failed: {}", e.message),
    })
}

// ── Tests ──────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Lexer Tests ─────────────────────────────────────────

    #[test]
    fn test_lex_simple_numbers() {
        let tokens = lex("3 4").unwrap();
        assert_eq!(tokens, vec![GlyphToken::Number(3), GlyphToken::Number(4),]);
    }

    #[test]
    fn test_lex_multi_digit_numbers() {
        let tokens = lex("42 100 7").unwrap();
        assert_eq!(
            tokens,
            vec![
                GlyphToken::Number(42),
                GlyphToken::Number(100),
                GlyphToken::Number(7),
            ]
        );
    }

    #[test]
    fn test_lex_operators() {
        let tokens = lex("+ - * /").unwrap();
        assert_eq!(
            tokens,
            vec![
                GlyphToken::Op('+'),
                GlyphToken::Op('-'),
                GlyphToken::Op('*'),
                GlyphToken::Op('/'),
            ]
        );
    }

    #[test]
    fn test_lex_comparisons() {
        let tokens = lex("> < =").unwrap();
        assert_eq!(
            tokens,
            vec![
                GlyphToken::Cmp('>'),
                GlyphToken::Cmp('<'),
                GlyphToken::Cmp('='),
            ]
        );
    }

    #[test]
    fn test_lex_control_flow() {
        let tokens = lex("? L").unwrap();
        assert_eq!(tokens, vec![GlyphToken::Conditional, GlyphToken::Loop,]);
    }

    #[test]
    fn test_lex_special() {
        let tokens = lex("S M . @").unwrap();
        assert_eq!(
            tokens,
            vec![
                GlyphToken::Spawn,
                GlyphToken::Mutate,
                GlyphToken::Output,
                GlyphToken::Halt,
            ]
        );
    }

    #[test]
    fn test_lex_registers() {
        let tokens = lex("a B z A").unwrap();
        assert_eq!(
            tokens,
            vec![
                GlyphToken::StoreReg(1),  // a -> r1
                GlyphToken::LoadReg(2),   // B -> r2
                GlyphToken::StoreReg(26), // z -> r26
                GlyphToken::LoadReg(1),   // A -> r1
            ]
        );
    }

    #[test]
    fn test_lex_comments_and_whitespace() {
        let tokens = lex("# comment\n3 # inline\n4 # another\n+ @").unwrap();
        assert_eq!(
            tokens,
            vec![
                GlyphToken::Number(3),
                GlyphToken::Number(4),
                GlyphToken::Op('+'),
                GlyphToken::Halt,
            ]
        );
    }

    #[test]
    fn test_lex_unknown_chars_ignored() {
        let tokens = lex("3,4+").unwrap();
        // comma is ignored, so "3" then comma skipped, then "4", then "+"
        assert_eq!(
            tokens,
            vec![
                GlyphToken::Number(3),
                GlyphToken::Number(4),
                GlyphToken::Op('+'),
            ]
        );
    }

    #[test]
    fn test_lex_empty_source() {
        let tokens = lex("").unwrap();
        assert!(tokens.is_empty());
    }

    #[test]
    fn test_lex_whitespace_only() {
        let tokens = lex("   \n\t  ").unwrap();
        assert!(tokens.is_empty());
    }

    #[test]
    fn test_lex_comment_only() {
        let tokens = lex("# just a comment").unwrap();
        assert!(tokens.is_empty());
    }

    // ── Compiler Tests ──────────────────────────────────────

    #[test]
    fn test_compile_simple_add() {
        let asm = compile_glyph("3 4 + @").unwrap();
        assert!(asm.contains("LDI r27, 3"));
        assert!(asm.contains("LDI r27, 4"));
        assert!(asm.contains("ADD r27, r28"));
        assert!(asm.contains("HALT"));
    }

    #[test]
    fn test_compile_subtract() {
        let asm = compile_glyph("10 3 - @").unwrap();
        assert!(asm.contains("SUB r27, r28"));
        assert!(asm.contains("HALT"));
    }

    #[test]
    fn test_compile_multiply() {
        let asm = compile_glyph("6 7 * @").unwrap();
        assert!(asm.contains("MUL r27, r28"));
    }

    #[test]
    fn test_compile_divide() {
        let asm = compile_glyph("20 4 / @").unwrap();
        assert!(asm.contains("DIV r27, r28"));
    }

    #[test]
    fn test_compile_comparison_equal() {
        let asm = compile_glyph("5 5 = @").unwrap();
        assert!(asm.contains("CMP r27, r28"));
    }

    #[test]
    fn test_compile_comparison_greater() {
        let asm = compile_glyph("5 3 > @").unwrap();
        assert!(asm.contains("CMP r27, r28"));
    }

    #[test]
    fn test_compile_comparison_less() {
        let asm = compile_glyph("3 5 < @").unwrap();
        assert!(asm.contains("CMP r27, r28"));
    }

    #[test]
    fn test_compile_conditional() {
        let asm = compile_glyph("1 42 0 ? @").unwrap();
        assert!(asm.contains("POP r28 ; f (false value)"));
        assert!(asm.contains("POP r27 ; t (true value)"));
        assert!(asm.contains("POP r29 ; c (condition)"));
    }

    #[test]
    fn test_compile_output() {
        let asm = compile_glyph("42 . @").unwrap();
        assert!(asm.contains("POP r27 ; value to output"));
    }

    #[test]
    fn test_compile_register_store_load() {
        let asm = compile_glyph("5 a A @").unwrap();
        assert!(asm.contains("POP r1")); // store to register a=r1
        assert!(asm.contains("PUSH r1")); // load from register A=r1
    }

    #[test]
    fn test_compile_halt_always_present() {
        let asm = compile_glyph("3 4 +").unwrap();
        assert!(asm.contains("HALT")); // auto-added
    }

    #[test]
    fn test_lex_new_opcodes() {
        let tokens = lex("[ { ! ^ |").unwrap();
        assert_eq!(
            tokens,
            vec![
                GlyphToken::Rectf,
                GlyphToken::DrawText,
                GlyphToken::Frame,
                GlyphToken::Ikey,
                GlyphToken::Fill,
            ]
        );
    }

    #[test]
    fn test_lex_string_literal() {
        let tokens = lex("\"hello world\"").unwrap();
        assert_eq!(
            tokens,
            vec![GlyphToken::StringLiteral("hello world".to_string())]
        );
    }

    #[test]
    fn test_compile_rectf() {
        let asm = compile_glyph("0 0 10 10 0xFF [ @").unwrap();
        assert!(asm.contains("RECTF r25, r26, r27, r28, r29"));
    }

    #[test]
    fn test_compile_drawtext_with_string() {
        let asm = compile_glyph("0 0 \"hi\" 0xFF 0 { @").unwrap();
        assert!(asm.contains("DRAWTEXT r25, r26, r27, r28, r29"));
        assert!(asm.contains("_str0:"));
        assert!(asm.contains(".str \"hi\""));
    }

    #[test]
    fn test_compile_frame_ikey_fill() {
        let asm = compile_glyph("! ^ | @").unwrap();
        assert!(asm.contains("FRAME"));
        assert!(asm.contains("IKEY r27"));
        assert!(asm.contains("FILL r27"));
    }

    #[test]
    fn test_compile_ls() {
        let asm = compile_glyph("0x4000 $ @").unwrap();
        assert!(asm.contains("LS r27"));
        assert!(asm.contains("PUSH r0"));
    }

    #[test]
    fn test_compile_labels_and_jumps() {
        let asm = compile_glyph(":start 1 1 + ~start @").unwrap();
        assert!(asm.contains("start:"));
        assert!(asm.contains("JMP start"));
    }

    #[test]
    fn test_compile_exec() {
        let asm = compile_glyph("\"shell\" & @").unwrap();
        assert!(asm.contains("EXEC r27"));
    }

    // ── Full Pipeline Tests (Lexer -> Compiler -> Assembler -> VM) ─

    #[test]
    fn test_pipeline_simple_arithmetic() {
        // 3 4 + . @ should compute 7 and draw a pixel
        let result = compile_glyph_to_bytecode("3 4 + . @");
        assert!(result.is_ok(), "Pipeline should succeed");
        let asm = result.unwrap();
        assert!(!asm.pixels.is_empty(), "Should produce bytecode");
    }

    #[test]
    fn test_pipeline_add_and_halt() {
        // 3 4 + @ should compile and assemble cleanly
        let result = compile_glyph_to_bytecode("3 4 + @");
        assert!(result.is_ok());
    }

    #[test]
    fn test_pipeline_complex_expression() {
        // 3 4 + 2 * . @ = (3+4)*2 = 14
        let result = compile_glyph_to_bytecode("3 4 + 2 * . @");
        assert!(result.is_ok());
    }

    #[test]
    fn test_pipeline_comparison() {
        // 5 5 = . @ should push 1 (equal) and output it
        let result = compile_glyph_to_bytecode("5 5 = . @");
        assert!(result.is_ok());
    }

    #[test]
    fn test_pipeline_conditional_true() {
        // 1 42 0 ? . @ should output 42 (condition is true)
        let result = compile_glyph_to_bytecode("1 42 0 ? . @");
        assert!(result.is_ok());
    }

    #[test]
    fn test_pipeline_conditional_false() {
        // 0 42 99 ? . @ should output 99 (condition is false)
        let result = compile_glyph_to_bytecode("0 42 99 ? . @");
        assert!(result.is_ok());
    }

    #[test]
    fn test_pipeline_register_roundtrip() {
        // 42 a A . @ should store 42 in register a, load it, output it
        let result = compile_glyph_to_bytecode("42 a A . @");
        assert!(result.is_ok());
    }

    // ── VM Execution Tests ──────────────────────────────────

    #[test]
    fn test_vm_runs_glyph_add() {
        // 3 4 + . @ should halt cleanly
        let result = compile_glyph_to_bytecode("3 4 + . @").unwrap();
        let mut vm = crate::vm::Vm::new();
        for (i, &word) in result.pixels.iter().enumerate() {
            if i < vm.ram.len() {
                vm.ram[i] = word;
            }
        }
        vm.pc = 0;
        vm.halted = false;
        for _ in 0..100_000 {
            if !vm.step() {
                break;
            }
        }
        assert!(vm.halted, "GlyphLang program should halt");
    }

    #[test]
    fn test_vm_runs_glyph_multi_op() {
        // 3 4 + 2 * . @ = 14
        let result = compile_glyph_to_bytecode("3 4 + 2 * . @").unwrap();
        let mut vm = crate::vm::Vm::new();
        for (i, &word) in result.pixels.iter().enumerate() {
            if i < vm.ram.len() {
                vm.ram[i] = word;
            }
        }
        vm.pc = 0;
        vm.halted = false;
        for _ in 0..100_000 {
            if !vm.step() {
                break;
            }
        }
        assert!(vm.halted, "Multi-op GlyphLang program should halt");
    }

    #[test]
    fn test_vm_runs_glyph_conditional() {
        // 1 42 99 ? . @ should output 42
        let result = compile_glyph_to_bytecode("1 42 99 ? . @").unwrap();
        let mut vm = crate::vm::Vm::new();
        for (i, &word) in result.pixels.iter().enumerate() {
            if i < vm.ram.len() {
                vm.ram[i] = word;
            }
        }
        vm.pc = 0;
        vm.halted = false;
        for _ in 0..100_000 {
            if !vm.step() {
                break;
            }
        }
        assert!(vm.halted, "Conditional GlyphLang program should halt");
    }

    #[test]
    fn test_vm_runs_glyph_comparison() {
        // 3 5 < . @ should output 1 (3 < 5 is true)
        let result = compile_glyph_to_bytecode("3 5 < . @").unwrap();
        let mut vm = crate::vm::Vm::new();
        for (i, &word) in result.pixels.iter().enumerate() {
            if i < vm.ram.len() {
                vm.ram[i] = word;
            }
        }
        vm.pc = 0;
        vm.halted = false;
        for _ in 0..100_000 {
            if !vm.step() {
                break;
            }
        }
        assert!(vm.halted, "Comparison GlyphLang program should halt");
    }
}
