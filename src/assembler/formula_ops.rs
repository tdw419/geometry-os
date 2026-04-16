// assembler/formula_ops.rs -- Formula/filesystem instruction match arms
//
// FORMULA, FORMULACLEAR, FORMULAREM, FMKDIR, FSTAT, FUNLINK

use super::{parse_imm, parse_reg};

type Constants<'a> = &'a std::collections::HashMap<String, u32>;

pub(super) fn try_parse(
    opcode: &str,
    tokens: &[&str],
    bytecode: &mut Vec<u32>,
    constants: Constants,
) -> Result<Option<()>, String> {
    match opcode {
        "FORMULA" => {
            // FORMULA target_idx, op, dep0, [dep1, ...]
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
            let deps: Vec<u32> = tokens[3..]
                .iter()
                .map(|a| parse_imm(a, constants))
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
            Ok(Some(()))
        }

        "FORMULACLEAR" => {
            bytecode.push(0x76);
            Ok(Some(()))
        }

        "FORMULAREM" => {
            if tokens.len() < 2 {
                return Err("FORMULAREM requires: target_idx".into());
            }
            let target_idx = parse_imm(tokens[1], constants)? as u32;
            bytecode.push(0x77);
            bytecode.push(target_idx);
            Ok(Some(()))
        }

        "FMKDIR" => {
            if tokens.len() < 2 {
                return Err("FMKDIR requires: path_reg".into());
            }
            let path_reg = parse_reg(tokens[1])?;
            bytecode.push(0x78);
            bytecode.push(path_reg as u32);
            Ok(Some(()))
        }

        "FSTAT" => {
            if tokens.len() < 3 {
                return Err("FSTAT requires: ino_reg, buf_reg".into());
            }
            let ino_reg = parse_reg(tokens[1])?;
            let buf_reg = parse_reg(tokens[2])?;
            bytecode.push(0x79);
            bytecode.push(ino_reg as u32);
            bytecode.push(buf_reg as u32);
            Ok(Some(()))
        }

        "FUNLINK" => {
            if tokens.len() < 2 {
                return Err("FUNLINK requires: path_reg".into());
            }
            let path_reg = parse_reg(tokens[1])?;
            bytecode.push(0x7A);
            bytecode.push(path_reg as u32);
            Ok(Some(()))
        }

        _ => Ok(None),
    }
}
