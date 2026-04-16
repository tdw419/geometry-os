// assembler/instructions.rs -- Opcode dispatcher
//
// Tokenizes assembly lines and dispatches to categorized sub-modules.
// Split from the original monolithic match block for readability.

use super::core_ops;
use super::formula_ops;
use super::graphics_ops;
use super::immediate_ops;
use super::system_ops;

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
