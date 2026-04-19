// assembler/core_ops.rs -- Core instruction match arms
//
// HALT, NOP, FRAME, BEEP, MEMCPY, LDI, LOAD, STORE, MOV,
// arithmetic (ADD..MOD), jumps (JMP, JZ, JNZ, CALL, RET, BLT, BGE),
// stack (PUSH, POP), CMP, NEG, IKEY, RAND

use super::{parse_imm, parse_reg};

type Constants<'a> = &'a std::collections::HashMap<String, u32>;

pub(super) fn try_parse(
    opcode: &str,
    tokens: &[&str],
    bytecode: &mut Vec<u32>,
    label_refs: &mut Vec<(usize, String, usize)>,
    line_num: usize,
    constants: Constants,
) -> Result<Option<()>, String> {
    match opcode {
        "HALT" => {
            bytecode.push(0x00);
            Ok(Some(()))
        }
        "NOP" => {
            bytecode.push(0x01);
            Ok(Some(()))
        }
        "FRAME" => {
            bytecode.push(0x02);
            Ok(Some(()))
        }

        "BEEP" => {
            if tokens.len() < 3 {
                return Err("BEEP requires 2 arguments: BEEP freq_reg, dur_reg".to_string());
            }
            bytecode.push(0x03);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
            Ok(Some(()))
        }

        "NOTE" => {
            if tokens.len() < 4 {
                return Err(
                    "NOTE requires 3 arguments: NOTE wave_reg, freq_reg, dur_reg".to_string(),
                );
            }
            bytecode.push(0x7E);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
            bytecode.push(parse_reg(tokens[3])? as u32);
            Ok(Some(()))
        }

        "MEMCPY" => {
            if tokens.len() < 4 {
                return Err(
                    "MEMCPY requires 3 arguments: MEMCPY dst_reg, src_reg, len_reg".to_string(),
                );
            }
            bytecode.push(0x04);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
            bytecode.push(parse_reg(tokens[3])? as u32);
            Ok(Some(()))
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
                bytecode.push(0);
                label_refs.push((pos, tokens[2].to_lowercase(), line_num));
            }
            Ok(Some(()))
        }

        "LOAD" => {
            if tokens.len() < 3 {
                return Err("LOAD requires 2 arguments: LOAD reg, addr_reg".to_string());
            }
            bytecode.push(0x11);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
            Ok(Some(()))
        }

        "STORE" => {
            if tokens.len() < 3 {
                return Err("STORE requires 2 arguments: STORE addr_reg, reg".to_string());
            }
            bytecode.push(0x12);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
            Ok(Some(()))
        }

        "MOV" => {
            if tokens.len() < 3 {
                return Err("MOV requires 2 arguments: MOV rd, rs".to_string());
            }
            bytecode.push(0x51);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
            Ok(Some(()))
        }

        // Arithmetic: ADD, SUB, MUL, DIV, AND, OR, XOR, SHL, SHR, SAR, MOD
        "ADD" | "SUB" | "MUL" | "DIV" | "AND" | "OR" | "XOR" | "SHL" | "SHR" | "SAR" | "MOD" => {
            if tokens.len() < 3 {
                let name = opcode;
                return Err(format!("{} requires 2 arguments: {} rd, rs", name, name));
            }
            let op_byte = match opcode {
                "ADD" => 0x20,
                "SUB" => 0x21,
                "MUL" => 0x22,
                "DIV" => 0x23,
                "AND" => 0x24,
                "OR" => 0x25,
                "XOR" => 0x26,
                "SHL" => 0x27,
                "SHR" => 0x28,
                "SAR" => 0x2B,
                "MOD" => 0x29,
                _ => unreachable!(),
            };
            bytecode.push(op_byte);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
            Ok(Some(()))
        }

        "CMP" => {
            if tokens.len() < 3 {
                return Err("CMP requires 2 arguments: CMP rd, rs".to_string());
            }
            bytecode.push(0x50);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
            Ok(Some(()))
        }

        "NEG" => {
            if tokens.len() < 2 {
                return Err("NEG requires 1 argument: NEG rd".to_string());
            }
            bytecode.push(0x2A);
            bytecode.push(parse_reg(tokens[1])? as u32);
            Ok(Some(()))
        }

        // Jumps with label resolution: JMP, JZ, JNZ, CALL, BLT, BGE
        "JMP" => {
            if tokens.len() < 2 {
                return Err("JMP requires 1 argument: JMP addr".to_string());
            }
            bytecode.push(0x30);
            let pos = bytecode.len();
            if let Ok(addr) = parse_imm(tokens[1], constants) {
                bytecode.push(addr);
            } else {
                bytecode.push(0);
                label_refs.push((pos, tokens[1].to_lowercase(), line_num));
            }
            Ok(Some(()))
        }

        "JZ" | "JNZ" | "BLT" | "BGE" => {
            if tokens.len() < 3 {
                let name = opcode;
                return Err(format!("{} requires 2 arguments: {} reg, addr", name, name));
            }
            let op_byte = match opcode {
                "JZ" => 0x31,
                "JNZ" => 0x32,
                "BLT" => 0x35,
                "BGE" => 0x36,
                _ => unreachable!(),
            };
            bytecode.push(op_byte);
            bytecode.push(parse_reg(tokens[1])? as u32);
            let pos = bytecode.len();
            if let Ok(addr) = parse_imm(tokens[2], constants) {
                bytecode.push(addr);
            } else {
                bytecode.push(0);
                label_refs.push((pos, tokens[2].to_lowercase(), line_num));
            }
            Ok(Some(()))
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
            Ok(Some(()))
        }

        "RET" => {
            bytecode.push(0x34);
            Ok(Some(()))
        }

        "PUSH" => {
            if tokens.len() < 2 {
                return Err("PUSH requires 1 argument: PUSH reg".to_string());
            }
            bytecode.push(0x60);
            bytecode.push(parse_reg(tokens[1])? as u32);
            Ok(Some(()))
        }

        "POP" => {
            if tokens.len() < 2 {
                return Err("POP requires 1 argument: POP reg".to_string());
            }
            bytecode.push(0x61);
            bytecode.push(parse_reg(tokens[1])? as u32);
            Ok(Some(()))
        }

        "IKEY" => {
            if tokens.len() < 2 {
                return Err("IKEY requires 1 argument: IKEY reg".to_string());
            }
            bytecode.push(0x48);
            bytecode.push(parse_reg(tokens[1])? as u32);
            Ok(Some(()))
        }

        "RAND" => {
            if tokens.len() < 2 {
                return Err("RAND requires 1 argument: RAND rd".to_string());
            }
            bytecode.push(0x49);
            bytecode.push(parse_reg(tokens[1])? as u32);
            Ok(Some(()))
        }

        _ => Ok(None),
    }
}
