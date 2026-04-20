// assembler/graphics_ops.rs -- Graphics instruction match arms
//
// PSET, PSETI, FILL, RECTF, TEXT, LINE, CIRCLE, SCROLL,
// SPRITE, TILEMAP, PEEK, SCREENP

use super::{parse_imm, parse_reg};

type Constants<'a> = &'a std::collections::HashMap<String, u32>;

pub(super) fn try_parse(
    opcode: &str,
    tokens: &[&str],
    bytecode: &mut Vec<u32>,
    constants: Constants,
) -> Result<Option<()>, String> {
    match opcode {
        "PSET" => {
            if tokens.len() < 4 {
                return Err("PSET requires 3 arguments: PSET x_reg, y_reg, color_reg".to_string());
            }
            bytecode.push(0x40);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
            bytecode.push(parse_reg(tokens[3])? as u32);
            Ok(Some(()))
        }

        "PSETI" => {
            if tokens.len() < 4 {
                return Err("PSETI requires 3 arguments: PSETI x, y, color".to_string());
            }
            bytecode.push(0x41);
            bytecode.push(parse_imm(tokens[1], constants)?);
            bytecode.push(parse_imm(tokens[2], constants)?);
            bytecode.push(parse_imm(tokens[3], constants)?);
            Ok(Some(()))
        }

        "FILL" => {
            if tokens.len() < 2 {
                return Err("FILL requires 1 argument: FILL color_reg".to_string());
            }
            bytecode.push(0x42);
            bytecode.push(parse_reg(tokens[1])? as u32);
            Ok(Some(()))
        }

        "RECTF" => {
            if tokens.len() < 6 {
                return Err(
                    "RECTF requires 5 arguments: RECTF x_reg, y_reg, w_reg, h_reg, color_reg"
                        .to_string(),
                );
            }
            bytecode.push(0x43);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
            bytecode.push(parse_reg(tokens[3])? as u32);
            bytecode.push(parse_reg(tokens[4])? as u32);
            bytecode.push(parse_reg(tokens[5])? as u32);
            Ok(Some(()))
        }

        "TEXT" => {
            if tokens.len() < 4 {
                return Err("TEXT requires 3 arguments: TEXT x_reg, y_reg, addr_reg".to_string());
            }
            bytecode.push(0x44);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
            bytecode.push(parse_reg(tokens[3])? as u32);
            Ok(Some(()))
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
            Ok(Some(()))
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
            Ok(Some(()))
        }

        "SCROLL" => {
            if tokens.len() < 2 {
                return Err("SCROLL requires 1 argument: SCROLL nr".to_string());
            }
            bytecode.push(0x47);
            bytecode.push(parse_reg(tokens[1])? as u32);
            Ok(Some(()))
        }

        "SPRITE" => {
            if tokens.len() < 6 {
                return Err(
                    "SPRITE requires 5 arguments: SPRITE x_reg, y_reg, addr_reg, w_reg, h_reg"
                        .to_string(),
                );
            }
            bytecode.push(0x4A);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
            bytecode.push(parse_reg(tokens[3])? as u32);
            bytecode.push(parse_reg(tokens[4])? as u32);
            bytecode.push(parse_reg(tokens[5])? as u32);
            Ok(Some(()))
        }

        "TILEMAP" => {
            if tokens.len() < 9 {
                return Err(
                    "TILEMAP requires 8 arguments: TILEMAP xr, yr, mr, tr, gwr, ghr, twr, thr"
                        .to_string(),
                );
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
            Ok(Some(()))
        }

        "PEEK" => {
            if tokens.len() < 4 {
                return Err("PEEK requires 3 arguments: PEEK rx, ry, rd".to_string());
            }
            bytecode.push(0x4F);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
            bytecode.push(parse_reg(tokens[3])? as u32);
            Ok(Some(()))
        }

        "SCREENP" => {
            if tokens.len() < 4 {
                return Err(
                    "SCREENP requires 3 arguments: SCREENP dest_reg, x_reg, y_reg".to_string(),
                );
            }
            bytecode.push(0x6D);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
            bytecode.push(parse_reg(tokens[3])? as u32);
            Ok(Some(()))
        }

        "HITSET" => {
            if tokens.len() < 6 {
                return Err(
                    "HITSET requires 5 arguments: HITSET x_reg, y_reg, w_reg, h_reg, id".to_string(),
                );
            }
            bytecode.push(0x37);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
            bytecode.push(parse_reg(tokens[3])? as u32);
            bytecode.push(parse_reg(tokens[4])? as u32);
            bytecode.push(parse_imm(tokens[5], constants)?);
            Ok(Some(()))
        }

        "HITQ" => {
            if tokens.len() < 2 {
                return Err("HITQ requires 1 argument: HITQ dest_reg".to_string());
            }
            bytecode.push(0x38);
            bytecode.push(parse_reg(tokens[1])? as u32);
            Ok(Some(()))
        }

        "MOUSEQ" => {
            if tokens.len() < 2 {
                return Err(
                    "MOUSEQ requires 1 argument: MOUSEQ x_reg (Y goes into x_reg+1)".to_string(),
                );
            }
            bytecode.push(0x85);
            bytecode.push(parse_reg(tokens[1])? as u32);
            Ok(Some(()))
        }

        _ => Ok(None),
    }
}
