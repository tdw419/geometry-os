// assembler/instructions.rs -- Opcode dispatcher
//
// Tokenizes assembly lines and dispatches to categorized sub-modules.
// Split from the original monolithic match block for readability.

use super::core_ops;
use super::formula_ops;
use super::graphics_ops;
use super::immediate_ops;
use super::system_ops;

/// Tokenize an assembly line, preserving quoted strings (double or single) as single tokens.
/// Splits on spaces and commas, but treats content between matching quotes as one token.
/// e.g. `STRO r20, "hello world"` -> ["STRO", "r20", "\"hello world\""]
fn tokenize_with_quotes(line: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_quote = false;
    let mut quote_char = ' ';
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let ch = chars[i];

        if in_quote {
            current.push(ch);
            if ch == quote_char {
                // Check for escaped quote (e.g. \\")
                if i + 1 < chars.len() && chars[i + 1] == '\\' {
                    // skip -- the backslash will be pushed next iteration
                } else {
                    in_quote = false;
                }
            }
        } else if ch == '"' || ch == '\'' {
            in_quote = true;
            quote_char = ch;
            current.push(ch);
        } else if ch == ' ' || ch == ',' {
            if !current.is_empty() {
                tokens.push(current.trim().to_string());
                current.clear();
            }
            // skip separator
        } else {
            current.push(ch);
        }
        i += 1;
    }

    if !current.is_empty() {
        tokens.push(current.trim().to_string());
    }

    tokens.retain(|t| !t.is_empty());
    tokens
}

pub(super) fn parse_instruction(
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

    // Split into tokens, preserving quoted strings as single tokens.
    // e.g. 'STRO r20, "hello world"' -> ["STRO", "r20", "\"hello world\""]
    let token_strings = tokenize_with_quotes(line);
    let tokens: Vec<&str> = token_strings.iter().map(|s| s.as_str()).collect();

    if tokens.is_empty() {
        return Ok(());
    }

    let opcode = tokens[0].to_uppercase();

    // Try each category in turn. Each returns:
    //   Ok(Some(()))  -- opcode handled successfully
    //   Ok(None)      -- opcode not in this category
    //   Err(msg)      -- parse error (propagated via ?)
    if core_ops::try_parse(&opcode, &tokens, bytecode, label_refs, line_num, constants)?.is_some() {
        return Ok(());
    }
    if graphics_ops::try_parse(&opcode, &tokens, bytecode, constants)?.is_some() {
        return Ok(());
    }
    if immediate_ops::try_parse(&opcode, &tokens, bytecode, constants)?.is_some() {
        return Ok(());
    }
    if system_ops::try_parse(&opcode, &tokens, bytecode, constants)?.is_some() {
        return Ok(());
    }
    if formula_ops::try_parse(&opcode, &tokens, bytecode, constants)?.is_some() {
        return Ok(());
    }

    Err(format!("unknown opcode: {}", opcode))
}
