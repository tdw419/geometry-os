// Trace the last N instructions before the fault to understand the context
// cargo run --example boot_pre_fault
use std::fs;
use geometry_os::riscv::RiscvVm;
use geometry_os::riscv::cpu::StepResult;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel = match fs::read(kernel_path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };
    let initramfs = fs::read(initramfs_path).ok();

    let bootargs = "console=ttyS0 earlycon=sbi panic=5 quiet";
    let (mut vm, _result) =
        RiscvVm::boot_linux(&kernel, initramfs.as_deref(), 512, 0, bootargs).unwrap();

    // Ring buffer of last 50 instructions
    let mut history: Vec<(u64, u32, u32)> = Vec::new(); // (count, pc, instruction)
    let mut count: u64 = 0;
    let max_instr = 260_000u64;

    while count < max_instr {
        let pc = vm.cpu.pc;
        let step_result = vm.step();
        count += 1;

        // Record instruction (read from PC in bare mode)
        if let Ok(inst) = vm.bus.read_word(pc as u64) {
            history.push((count, pc, inst));
            if history.len() > 50 {
                history.remove(0);
            }
        }

        // Check for fault
        if vm.cpu.pc == 0 && pc != 0 {
            println!("FAULT at instruction {}", count);
            println!(
                "  mcause=0x{:08X}, mepc=0x{:08X}, mtval=0x{:08X}",
                vm.cpu.csr.mcause, vm.cpu.csr.mepc, vm.cpu.csr.mtval
            );
            println!("\nLast 30 instructions before fault:");
            for (i, (cnt, addr, inst)) in history.iter().enumerate() {
                let opcode = inst & 0x7F;
                let rd = (inst >> 7) & 0x1F;
                let rs1 = (inst >> 15) & 0x1F;
                let rs2 = (inst >> 20) & 0x1F;
                let funct3 = (inst >> 12) & 0x7;
                let funct7 = (inst >> 25) & 0x7F;

                let desc = match opcode {
                    0x37 => format!("lui x{}, 0x{:X}", rd, inst & 0xFFFFF000),
                    0x17 => format!("auipc x{}, 0x{:X}", rd, inst & 0xFFFFF000),
                    0x6F => format!("jal x{}", rd),
                    0x67 => {
                        let imm = ((inst >> 20) & 0xFFF) as i32;
                        let imm = if imm & 0x800 != 0 { imm - 0x1000 } else { imm };
                        format!("jalr x{}, x{}, {}", rd, rs1, imm)
                    }
                    0x63 => {
                        let names = ["beq", "bne", "", "blt", "bge", "", "bltu", "bgeu"];
                        let name = if funct3 < 8 { names[funct3 as usize] } else { "?" };
                        format!("{} x{}, x{}", name, rs1, rs2)
                    }
                    0x03 => {
                        let names = ["lb", "lh", "lw", "", "lbu", "lhu"];
                        let name = if funct3 < 6 { names[funct3 as usize] } else { "?" };
                        format!("{} x{}, ...", name, rd)
                    }
                    0x23 => {
                        let names = ["sb", "sh", "sw"];
                        let name = if funct3 < 3 { names[funct3 as usize] } else { "?" };
                        format!("{} x{}, ...", name, rs2)
                    }
                    0x13 => {
                        let names = ["addi", "slli", "slti", "sltiu", "xori", "srli/srai", "ori", "andi"];
                        let name = if funct3 < 8 { names[funct3 as usize] } else { "?" };
                        format!("{} x{}, x{}, ...", name, rd, rs1)
                    }
                    0x33 => {
                        let mut name = match (funct3, funct7) {
                            (0, 0) => "add",
                            (0, 0x20) => "sub",
                            (1, 0) => "sll",
                            (2, 0) => "slt",
                            (3, 0) => "sltu",
                            (4, 0) => "xor",
                            (5, 0) => "srl",
                            (5, 0x20) => "sra",
                            (6, 0) => "or",
                            (7, 0) => "and",
                            _ => "?",
                        };
                        format!("{} x{}, x{}, x{}", name, rd, rs1, rs2)
                    }
                    0x73 => {
                        if funct3 == 0 && funct7 == 0x08 {
                            "ecall".to_string()
                        } else if funct3 == 0 && funct7 == 0x09 {
                            "ebreak".to_string()
                        } else if funct3 == 5 && funct7 == 0x302 {
                            "mret".to_string()
                        } else if funct3 == 5 && funct7 == 0x102 {
                            "sret".to_string()
                        } else {
                            let csr = (inst >> 20) & 0xFFF;
                            let csr_names = [
                                (0x100, "sstatus"), (0x104, "sie"), (0x105, "stvec"),
                                (0x141, "sepc"), (0x142, "scause"), (0x143, "stval"),
                                (0x180, "satp"), (0x300, "mstatus"), (0x302, "medeleg"),
                                (0x303, "mideleg"), (0x304, "mie"), (0x305, "mtvec"),
                                (0x341, "mepc"), (0x342, "mcause"), (0x343, "mtval"),
                                (0x344, "mip"),
                            ];
                            let csr_name = csr_names.iter().find(|(a, _)| *a == csr)
                                .map(|(_, n)| *n).unwrap_or("???");
                            let op = match funct3 {
                                1 => "csrw",
                                2 => "csrs",
                                3 => "csrc",
                                _ => "csr?",
                            };
                            format!("{} {}, x{}", op, csr_name, rs1)
                        }
                    }
                    _ => format!("op=0x{:02X}", opcode),
                };

                // Mark the faulting instruction
                let marker = if *addr == vm.cpu.csr.mepc { " <<< FAULT" } else { "" };
                println!(
                    "  [{:>6}] 0x{:08X}: 0x{:08X}  {}{}",
                    cnt, addr, inst, desc, marker
                );
            }
            break;
        }

        match step_result {
            StepResult::FetchFault => {
                println!("[{}] FETCH FAULT at PC=0x{:08X}", count, vm.cpu.pc);
                break;
            }
            StepResult::Ebreak => {
                println!("[{}] EBREAK at PC=0x{:08X}", count, vm.cpu.pc);
                break;
            }
            _ => {}
        }
    }
}
