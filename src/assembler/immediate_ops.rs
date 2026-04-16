// assembler/immediate_ops.rs -- Immediate-operand instruction match arms
//
// TEXTI, STRO, CMPI, LOADS, STORES, SHLI, SHRI, SARI,
// ADDI, SUBI, ANDI, ORI, XORI

use super::{parse_imm, parse_reg};

type Constants<'a> = &'a std::collections::HashMap<String, u32>;

pub(super) fn try_parse(
    opcode: &str,
    tokens: &[&str],
    bytecode: &mut Vec<u32>,
    constants: Constants,
) -> Result<Option<()>, String> {
    match opcode {
        // TEXTI x, y, "string" -- render inline text (no RAM setup needed)
        // Encoding: 0x13, x_imm, y_imm, char_count, char1, char2, ..., 0 (null term)
        "TEXTI" => {
            if tokens.len() < 4 {
                return Err("TEXTI requires 3 args: TEXTI x, y, \"string\"".to_string());
            }
            let x = parse_imm(tokens[1], constants)?;
            let y = parse_imm(tokens[2], constants)?;
            let rest = tokens[3..].join(",");
            let s = rest.trim();
            if !((s.starts_with('"') && s.ends_with('"'))
                || (s.starts_with('\'') && s.ends_with('\'')))
            {
                return Err(
                    "TEXTI requires a quoted string: TEXTI x, y, \"text\"".to_string(),
                );
            }
            let text = &s[1..s.len() - 1];
            bytecode.push(0x13);
            bytecode.push(x);
            bytecode.push(y);
            bytecode.push(text.len() as u32);
            for ch in text.bytes() {
                bytecode.push(ch as u32);
            }
            Ok(Some(()))
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
            if !((s.starts_with('"') && s.ends_with('"'))
                || (s.starts_with('\'') && s.ends_with('\'')))
            {
                return Err(
                    "STRO requires a quoted string: STRO addr_reg, \"text\"".to_string(),
                );
            }
            let text = &s[1..s.len() - 1];
            bytecode.push(0x14);
            bytecode.push(reg as u32);
            bytecode.push(text.len() as u32);
            for ch in text.bytes() {
                bytecode.push(ch as u32);
            }
            Ok(Some(()))
        }

        // CMPI reg, imm -- compare register against immediate, sets r0
        "CMPI" => {
            if tokens.len() < 3 {
                return Err("CMPI requires 2 arguments: CMPI reg, imm".to_string());
            }
            bytecode.push(0x15);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_imm(tokens[2], constants)?);
            Ok(Some(()))
        }

        // LOADS reg, offset -- load from SP+offset (r30 + signed offset)
        "LOADS" => {
            if tokens.len() < 3 {
                return Err("LOADS requires 2 arguments: LOADS reg, offset".to_string());
            }
            bytecode.push(0x16);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_imm(tokens[2], constants)?);
            Ok(Some(()))
        }

        // STORES offset, reg -- store to SP+offset (r30 + signed offset)
        "STORES" => {
            if tokens.len() < 3 {
                return Err("STORES requires 2 arguments: STORES offset, reg".to_string());
            }
            bytecode.push(0x17);
            bytecode.push(parse_imm(tokens[1], constants)?);
            bytecode.push(parse_reg(tokens[2])? as u32);
            Ok(Some(()))
        }

        // Immediate arithmetic: SHLI, SHRI, SARI, ADDI, SUBI, ANDI, ORI, XORI
        "SHLI" | "SHRI" | "SARI" | "ADDI" | "SUBI" | "ANDI" | "ORI" | "XORI" => {
            if tokens.len() < 3 {
                let name = opcode;
                return Err(format!(
                    "{} requires 2 arguments: {} reg, imm",
                    name, name
                ));
            }
            let op_byte = match opcode {
                "SHLI" => 0x18,
                "SHRI" => 0x19,
                "SARI" => 0x1A,
                "ADDI" => 0x1B,
                "SUBI" => 0x1C,
                "ANDI" => 0x1D,
                "ORI" => 0x1E,
                "XORI" => 0x1F,
                _ => unreachable!(),
            };
            bytecode.push(op_byte);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_imm(tokens[2], constants)?);
            Ok(Some(()))
        }

        _ => Ok(None),
    }
}
