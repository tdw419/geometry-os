// assembler/system_ops.rs -- System/OS instruction match arms
//
// SYSCALL, RETK, file I/O (OPEN, READ, WRITE, CLOSE, SEEK, LS),
// scheduling (YIELD, SLEEP, SETPRIORITY), IPC (PIPE, MSGSND, MSGRCV),
// IOCTL, env (GETENV, SETENV), process control (GETPID, EXEC, WRITESTR,
// READLN, WAITPID, EXECP, CHDIR, GETCWD, SPAWN, KILL),
// signals (SHUTDOWN, EXIT, SIGNAL, SIGSET), HYPERVISOR, ASM, ASMSELF, RUNNEXT

use super::{parse_imm, parse_reg};

type Constants<'a> = &'a std::collections::HashMap<String, u32>;

pub(super) fn try_parse(
    opcode: &str,
    tokens: &[&str],
    bytecode: &mut Vec<u32>,
    constants: Constants,
) -> Result<Option<()>, String> {
    match opcode {
        "ASM" => {
            if tokens.len() < 3 {
                return Err("ASM requires 2 arguments: ASM src_addr_reg, dest_addr_reg".to_string());
            }
            bytecode.push(0x4B);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
            Ok(Some(()))
        }

        "SPAWN" => {
            if tokens.len() < 2 {
                return Err("SPAWN requires 1 argument: SPAWN addr_reg".to_string());
            }
            bytecode.push(0x4D);
            bytecode.push(parse_reg(tokens[1])? as u32);
            Ok(Some(()))
        }

        "KILL" => {
            if tokens.len() < 2 {
                return Err("KILL requires 1 argument: KILL pid_reg".to_string());
            }
            bytecode.push(0x4E);
            bytecode.push(parse_reg(tokens[1])? as u32);
            Ok(Some(()))
        }

        "SYSCALL" => {
            if tokens.len() < 2 {
                return Err("SYSCALL requires 1 argument: SYSCALL num".to_string());
            }
            bytecode.push(0x52);
            bytecode.push(parse_imm(tokens[1], constants)?);
            Ok(Some(()))
        }

        "RETK" => {
            bytecode.push(0x53);
            Ok(Some(()))
        }

        "OPEN" => {
            if tokens.len() < 3 {
                return Err("OPEN requires 2 arguments: OPEN path_reg, mode_reg".to_string());
            }
            bytecode.push(0x54);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
            Ok(Some(()))
        }

        "READ" => {
            if tokens.len() < 4 {
                return Err("READ requires 3 arguments: READ fd_reg, buf_reg, len_reg".to_string());
            }
            bytecode.push(0x55);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
            bytecode.push(parse_reg(tokens[3])? as u32);
            Ok(Some(()))
        }

        "WRITE" => {
            if tokens.len() < 4 {
                return Err(
                    "WRITE requires 3 arguments: WRITE fd_reg, buf_reg, len_reg".to_string()
                );
            }
            bytecode.push(0x56);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
            bytecode.push(parse_reg(tokens[3])? as u32);
            Ok(Some(()))
        }

        "CLOSE" => {
            if tokens.len() < 2 {
                return Err("CLOSE requires 1 argument: CLOSE fd_reg".to_string());
            }
            bytecode.push(0x57);
            bytecode.push(parse_reg(tokens[1])? as u32);
            Ok(Some(()))
        }

        "SEEK" => {
            if tokens.len() < 4 {
                return Err(
                    "SEEK requires 3 arguments: SEEK fd_reg, offset_reg, whence_reg".to_string(),
                );
            }
            bytecode.push(0x58);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
            bytecode.push(parse_reg(tokens[3])? as u32);
            Ok(Some(()))
        }

        "LS" => {
            if tokens.len() < 2 {
                return Err("LS requires 1 argument: LS buf_reg".to_string());
            }
            bytecode.push(0x59);
            bytecode.push(parse_reg(tokens[1])? as u32);
            Ok(Some(()))
        }

        "YIELD" => {
            bytecode.push(0x5A);
            Ok(Some(()))
        }

        "SLEEP" => {
            if tokens.len() < 2 {
                return Err("SLEEP requires 1 argument: SLEEP ticks_reg".to_string());
            }
            bytecode.push(0x5B);
            bytecode.push(parse_reg(tokens[1])? as u32);
            Ok(Some(()))
        }

        "SETPRIORITY" => {
            if tokens.len() < 2 {
                return Err("SETPRIORITY requires 1 argument: SETPRIORITY priority_reg".to_string());
            }
            bytecode.push(0x5C);
            bytecode.push(parse_reg(tokens[1])? as u32);
            Ok(Some(()))
        }

        "PIPE" => {
            if tokens.len() < 3 {
                return Err("PIPE requires 2 arguments: PIPE read_fd_reg, write_fd_reg".to_string());
            }
            bytecode.push(0x5D);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
            Ok(Some(()))
        }

        "MSGSND" => {
            if tokens.len() < 2 {
                return Err("MSGSND requires 1 argument: MSGSND pid_reg".to_string());
            }
            bytecode.push(0x5E);
            bytecode.push(parse_reg(tokens[1])? as u32);
            Ok(Some(()))
        }

        "MSGRCV" => {
            bytecode.push(0x5F);
            Ok(Some(()))
        }

        "IOCTL" => {
            if tokens.len() < 4 {
                return Err(
                    "IOCTL requires 3 arguments: IOCTL fd_reg, cmd_reg, arg_reg".to_string()
                );
            }
            bytecode.push(0x62);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
            bytecode.push(parse_reg(tokens[3])? as u32);
            Ok(Some(()))
        }

        "GETENV" => {
            if tokens.len() < 3 {
                return Err(
                    "GETENV requires 2 arguments: GETENV key_addr_reg, val_addr_reg".to_string(),
                );
            }
            bytecode.push(0x63);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
            Ok(Some(()))
        }

        "SETENV" => {
            if tokens.len() < 3 {
                return Err(
                    "SETENV requires 2 arguments: SETENV key_addr_reg, val_addr_reg".to_string(),
                );
            }
            bytecode.push(0x64);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
            Ok(Some(()))
        }

        "GETPID" => {
            bytecode.push(0x65);
            Ok(Some(()))
        }
        "PROCLS" => {
            if tokens.len() != 2 {
                return Err("PROCLS requires 1 argument: PROCLS buf_reg".to_string());
            }
            bytecode.push(0x9B);
            bytecode.push(parse_reg(tokens[1])? as u32);
            Ok(Some(()))
        }

        "EXEC" => {
            if tokens.len() != 2 {
                return Err("EXEC requires 1 argument: EXEC path_addr_reg".to_string());
            }
            bytecode.push(0x66);
            let r = parse_reg(tokens[1])?;
            bytecode.push(r as u32);
            Ok(Some(()))
        }

        "WRITESTR" => {
            if tokens.len() != 3 {
                return Err(
                    "WRITESTR requires 2 arguments: WRITESTR fd_reg, str_addr_reg".to_string(),
                );
            }
            bytecode.push(0x67);
            let r1 = parse_reg(tokens[1])?;
            let r2 = parse_reg(tokens[2])?;
            bytecode.push(r1 as u32);
            bytecode.push(r2 as u32);
            Ok(Some(()))
        }

        "READLN" => {
            if tokens.len() != 4 {
                return Err(
                    "READLN requires 3 arguments: READLN buf_reg, max_len_reg, pos_reg".to_string(),
                );
            }
            bytecode.push(0x68);
            let r1 = parse_reg(tokens[1])?;
            let r2 = parse_reg(tokens[2])?;
            let r3 = parse_reg(tokens[3])?;
            bytecode.push(r1 as u32);
            bytecode.push(r2 as u32);
            bytecode.push(r3 as u32);
            Ok(Some(()))
        }

        "WAITPID" => {
            if tokens.len() != 2 {
                return Err("WAITPID requires 1 argument: WAITPID pid_reg".to_string());
            }
            bytecode.push(0x69);
            let r = parse_reg(tokens[1])?;
            bytecode.push(r as u32);
            Ok(Some(()))
        }

        "EXECP" => {
            if tokens.len() != 4 {
                return Err(
                    "EXECP requires 3 arguments: EXECP path_reg, stdin_fd_reg, stdout_fd_reg"
                        .to_string(),
                );
            }
            bytecode.push(0x6A);
            let r1 = parse_reg(tokens[1])?;
            let r2 = parse_reg(tokens[2])?;
            let r3 = parse_reg(tokens[3])?;
            bytecode.push(r1 as u32);
            bytecode.push(r2 as u32);
            bytecode.push(r3 as u32);
            Ok(Some(()))
        }

        "CHDIR" => {
            if tokens.len() != 2 {
                return Err("CHDIR requires 1 argument: CHDIR path_reg".to_string());
            }
            bytecode.push(0x6B);
            let r = parse_reg(tokens[1])?;
            bytecode.push(r as u32);
            Ok(Some(()))
        }

        "GETCWD" => {
            if tokens.len() != 2 {
                return Err("GETCWD requires 1 argument: GETCWD buf_reg".to_string());
            }
            bytecode.push(0x6C);
            let r = parse_reg(tokens[1])?;
            bytecode.push(r as u32);
            Ok(Some(()))
        }

        "SHUTDOWN" => {
            bytecode.push(0x6E);
            Ok(Some(()))
        }

        "EXIT" => {
            if tokens.len() < 2 {
                return Err("EXIT requires 1 argument: EXIT code_reg".to_string());
            }
            bytecode.push(0x6F);
            bytecode.push(parse_reg(tokens[1])? as u32);
            Ok(Some(()))
        }

        "SIGNAL" => {
            if tokens.len() < 3 {
                return Err("SIGNAL requires 2 arguments: SIGNAL pid_reg sig_reg".to_string());
            }
            bytecode.push(0x70);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
            Ok(Some(()))
        }

        "SIGSET" => {
            if tokens.len() < 3 {
                return Err("SIGSET requires 2 arguments: SIGSET sig_reg handler_reg".to_string());
            }
            bytecode.push(0x71);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
            Ok(Some(()))
        }

        "HYPERVISOR" => {
            if tokens.len() < 2 {
                return Err(
                    "HYPERVISOR requires 1-2 arguments: HYPERVISOR addr_reg [, win_id_reg]"
                        .to_string(),
                );
            }
            bytecode.push(0x72);
            bytecode.push(parse_reg(tokens[1])? as u32);
            // Optional window_id register (default r0 = no window = full canvas)
            if tokens.len() >= 3 {
                bytecode.push(parse_reg(tokens[2])? as u32);
            } else {
                bytecode.push(0); // r0 = window_id 0 = full canvas
            }
            Ok(Some(()))
        }

        "ASMSELF" => {
            bytecode.push(0x73);
            Ok(Some(()))
        }

        "RUNNEXT" => {
            bytecode.push(0x74);
            Ok(Some(()))
        }

        "SNAP_TRACE" => {
            if tokens.len() < 2 {
                return Err("SNAP_TRACE requires 1 argument: SNAP_TRACE mode_reg".to_string());
            }
            bytecode.push(0x7B);
            bytecode.push(parse_reg(tokens[1])? as u32);
            Ok(Some(()))
        }

        "REPLAY" => {
            if tokens.len() < 2 {
                return Err("REPLAY requires 1 argument: REPLAY frame_idx_reg".to_string());
            }
            bytecode.push(0x7C);
            bytecode.push(parse_reg(tokens[1])? as u32);
            Ok(Some(()))
        }

        "FORK" => {
            if tokens.len() < 2 {
                return Err("FORK requires 1 argument: FORK mode_reg".to_string());
            }
            bytecode.push(0x7D);
            bytecode.push(parse_reg(tokens[1])? as u32);
            Ok(Some(()))
        }

        "STRCMP" => {
            if tokens.len() < 3 {
                return Err("STRCMP requires 2 arguments: STRCMP addr1_reg, addr2_reg".to_string());
            }
            bytecode.push(0x86);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
            Ok(Some(()))
        }

        "ABS" => {
            if tokens.len() < 2 {
                return Err("ABS requires 1 argument: ABS rd".to_string());
            }
            bytecode.push(0x87);
            bytecode.push(parse_reg(tokens[1])? as u32);
            Ok(Some(()))
        }

        "MIN" => {
            if tokens.len() < 3 {
                return Err("MIN requires 2 arguments: MIN rd, rs".to_string());
            }
            bytecode.push(0x89);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
            Ok(Some(()))
        }

        "MAX" => {
            if tokens.len() < 3 {
                return Err("MAX requires 2 arguments: MAX rd, rs".to_string());
            }
            bytecode.push(0x8A);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
            Ok(Some(()))
        }

        "CLAMP" => {
            if tokens.len() < 4 {
                return Err("CLAMP requires 3 arguments: CLAMP rd, min_reg, max_reg".to_string());
            }
            bytecode.push(0x8B);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
            bytecode.push(parse_reg(tokens[3])? as u32);
            Ok(Some(()))
        }

        "BITSET" => {
            if tokens.len() < 3 {
                return Err("BITSET requires 2 arguments: BITSET rd, bit_reg".to_string());
            }
            bytecode.push(0x8D);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
            Ok(Some(()))
        }

        "BITCLR" => {
            if tokens.len() < 3 {
                return Err("BITCLR requires 2 arguments: BITCLR rd, bit_reg".to_string());
            }
            bytecode.push(0x8E);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
            Ok(Some(()))
        }

        "BITTEST" => {
            if tokens.len() < 3 {
                return Err("BITTEST requires 2 arguments: BITTEST rd, bit_reg".to_string());
            }
            bytecode.push(0x8F);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
            Ok(Some(()))
        }

        "NOT" => {
            if tokens.len() < 2 {
                return Err("NOT requires 1 argument: NOT rd".to_string());
            }
            bytecode.push(0x90);
            bytecode.push(parse_reg(tokens[1])? as u32);
            Ok(Some(()))
        }

        "INV" => {
            bytecode.push(0x91);
            Ok(Some(()))
        }

        "MATVEC" => {
            if tokens.len() < 6 {
                return Err("MATVEC requires 5 arguments: MATVEC r_weight, r_input, r_output, r_rows, r_cols".to_string());
            }
            bytecode.push(0x92);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
            bytecode.push(parse_reg(tokens[3])? as u32);
            bytecode.push(parse_reg(tokens[4])? as u32);
            bytecode.push(parse_reg(tokens[5])? as u32);
            Ok(Some(()))
        }

        "RELU" => {
            if tokens.len() < 2 {
                return Err("RELU requires 1 argument: RELU rd".to_string());
            }
            bytecode.push(0x93);
            bytecode.push(parse_reg(tokens[1])? as u32);
            Ok(Some(()))
        }

        "WINSYS" => {
            if tokens.len() < 2 {
                return Err("WINSYS requires 1 argument: WINSYS op_reg".to_string());
            }
            bytecode.push(0x94);
            bytecode.push(parse_reg(tokens[1])? as u32);
            Ok(Some(()))
        }

        "WPIXEL" => {
            if tokens.len() < 5 {
                return Err(
                    "WPIXEL requires 4 arguments: WPIXEL win_id_reg, x_reg, y_reg, color_reg"
                        .to_string(),
                );
            }
            bytecode.push(0x95);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
            bytecode.push(parse_reg(tokens[3])? as u32);
            bytecode.push(parse_reg(tokens[4])? as u32);
            Ok(Some(()))
        }

        "WREAD" => {
            if tokens.len() < 5 {
                return Err(
                    "WREAD requires 4 arguments: WREAD win_id_reg, x_reg, y_reg, dest_reg"
                        .to_string(),
                );
            }
            bytecode.push(0x96);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
            bytecode.push(parse_reg(tokens[3])? as u32);
            bytecode.push(parse_reg(tokens[4])? as u32);
            Ok(Some(()))
        }

        "SCRSHOT" => {
            if tokens.len() < 2 {
                return Err("SCRSHOT requires 1 argument: SCRSHOT path_addr_reg".to_string());
            }
            bytecode.push(0x98);
            bytecode.push(parse_reg(tokens[1])? as u32);
            Ok(Some(()))
        }

        "LLM" => {
            if tokens.len() != 4 {
                return Err(
                    "LLM requires 3 arguments: LLM prompt_addr_reg, response_addr_reg, max_len_reg"
                        .to_string(),
                );
            }
            bytecode.push(0x9C);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
            bytecode.push(parse_reg(tokens[3])? as u32);
            Ok(Some(()))
        }

        "HTPARSE" => {
            if tokens.len() != 4 {
                return Err(
                    "HTPARSE requires 3 arguments: HTPARSE src_reg, dest_reg, max_lines_reg"
                        .to_string(),
                );
            }
            bytecode.push(0x9D);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
            bytecode.push(parse_reg(tokens[3])? as u32);
            Ok(Some(()))
        }

        "HITCLR" => {
            bytecode.push(0x9E);
            Ok(Some(()))
        }

        // ── Phase 87: Multi-Hypervisor Opcodes ──
        "VM_SPAWN" => {
            // VM_SPAWN config_reg, window_reg
            // 3 words: [0x9F, config_reg, window_reg]
            if tokens.len() < 3 {
                return Err(
                    "VM_SPAWN requires 2 arguments: VM_SPAWN config_reg, window_reg".to_string(),
                );
            }
            bytecode.push(0x9F);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
            Ok(Some(()))
        }

        "VM_KILL" => {
            // VM_KILL id_reg
            // 2 words: [0xA0, id_reg]
            if tokens.len() < 2 {
                return Err("VM_KILL requires 1 argument: VM_KILL id_reg".to_string());
            }
            bytecode.push(0xA0);
            bytecode.push(parse_reg(tokens[1])? as u32);
            Ok(Some(()))
        }

        "VM_STATUS" => {
            // VM_STATUS id_reg
            // 2 words: [0xA1, id_reg]
            if tokens.len() < 2 {
                return Err("VM_STATUS requires 1 argument: VM_STATUS id_reg".to_string());
            }
            bytecode.push(0xA1);
            bytecode.push(parse_reg(tokens[1])? as u32);
            Ok(Some(()))
        }

        "VM_PAUSE" => {
            // VM_PAUSE id_reg
            // 2 words: [0xA2, id_reg]
            if tokens.len() < 2 {
                return Err("VM_PAUSE requires 1 argument: VM_PAUSE id_reg".to_string());
            }
            bytecode.push(0xA2);
            bytecode.push(parse_reg(tokens[1])? as u32);
            Ok(Some(()))
        }

        "VM_RESUME" => {
            // VM_RESUME id_reg
            // 2 words: [0xA3, id_reg]
            if tokens.len() < 2 {
                return Err("VM_RESUME requires 1 argument: VM_RESUME id_reg".to_string());
            }
            bytecode.push(0xA3);
            bytecode.push(parse_reg(tokens[1])? as u32);
            Ok(Some(()))
        }

        "VM_SET_BUDGET" => {
            // VM_SET_BUDGET id_reg, budget_reg
            // 3 words: [0xA4, id_reg, budget_reg]
            if tokens.len() < 3 {
                return Err(
                    "VM_SET_BUDGET requires 2 arguments: VM_SET_BUDGET id_reg, budget_reg"
                        .to_string(),
                );
            }
            bytecode.push(0xA4);
            bytecode.push(parse_reg(tokens[1])? as u32);
            bytecode.push(parse_reg(tokens[2])? as u32);
            Ok(Some(()))
        }

        "VM_LIST" => {
            // VM_LIST addr_reg
            // 2 words: [0xA5, addr_reg]
            if tokens.len() < 2 {
                return Err("VM_LIST requires 1 argument: VM_LIST addr_reg".to_string());
            }
            bytecode.push(0xA5);
            bytecode.push(parse_reg(tokens[1])? as u32);
            Ok(Some(()))
        }

        // ── Phase 88: AI Vision Bridge ──
        "AI_AGENT" => {
            // AI_AGENT op_reg
            // 2 words: [0xB0, op_reg]
            // op_reg contains the operation number:
            //   0 = screenshot to VFS (path addr in r[op_reg+1])
            //   1 = canvas checksum -> r0
            //   2 = canvas diff (prev screen addr in r[op_reg+1]) -> r0
            //   3 = vision API call (prompt_addr in r[op_reg+1], response_addr in r[op_reg+2], max_len in r[op_reg+3])
            if tokens.len() < 2 {
                return Err("AI_AGENT requires 1 argument: AI_AGENT op_reg".to_string());
            }
            bytecode.push(0xB0);
            bytecode.push(parse_reg(tokens[1])? as u32);
            Ok(Some(()))
        }

        _ => Ok(None),
    }
}
