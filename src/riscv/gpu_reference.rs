// riscv/gpu_reference.rs -- Reference RISC-V Interpreter for GPU Tile Verification
//
// Minimal RV32I base integer instruction set interpreter.
// Used to verify GPU execution by running the same code on CPU and comparing results.
// Mirrors the WGSL compute shader's instruction set exactly.
//
// This module is always compiled (no gpu feature required) so that
// verification can be used in any test context.

use super::gpu::{RAM_WORDS, STATUS_ERROR, STATUS_HALTED, STATUS_RUNNING};

/// Reference RISC-V VM for verification against GPU execution.
///
/// Implements a minimal subset of RV32I: LUI, AUIPC, JAL, JALR, Branch,
/// Load, Store, OP-IMM, OP, SYSTEM (ECALL/EBREAK). Also includes M extension
/// (MUL, MULH). Designed to produce identical results to the WGSL shader.
pub struct ReferenceVm {
    pub regs: [u32; 32],
    pub pc: u32,
    pub ram: Vec<u32>,
    pub status: u32,
    pub instruction_count: u32,
    pub uart_output: Vec<u8>,
}

impl ReferenceVm {
    /// Create a new reference VM with the given RAM contents.
    pub fn new(ram: Vec<u32>) -> Self {
        ReferenceVm {
            regs: [0; 32],
            pc: 0,
            ram,
            status: STATUS_RUNNING,
            instruction_count: 0,
            uart_output: Vec::new(),
        }
    }

    fn read_reg(&self, reg: u32) -> u32 {
        if reg == 0 {
            0
        } else {
            self.regs[reg as usize]
        }
    }

    fn write_reg(&mut self, reg: u32, val: u32) {
        if reg != 0 {
            self.regs[reg as usize] = val;
        }
    }

    fn read_ram(&self, addr: u32) -> u32 {
        let word_addr = (addr / 4) as usize;
        if word_addr < self.ram.len() {
            self.ram[word_addr]
        } else {
            0
        }
    }

    fn write_ram(&mut self, addr: u32, val: u32) {
        // UART MMIO intercept (match WGSL behavior)
        if addr >= 0x10000000 && addr < 0x10000200 {
            self.uart_output.push((val & 0xFF) as u8);
            return;
        }
        let word_addr = (addr / 4) as usize;
        if word_addr < self.ram.len() {
            self.ram[word_addr] = val;
        }
    }

    /// Execute one instruction. Returns false if the VM is no longer running.
    pub fn step(&mut self) -> bool {
        if self.status != STATUS_RUNNING {
            return false;
        }

        let inst = self.read_ram(self.pc);
        let opcode = inst & 0x7F;
        let rd = (inst >> 7) & 0x1F;
        let funct3 = (inst >> 12) & 0x7;
        let rs1 = (inst >> 15) & 0x1F;
        let rs2 = (inst >> 20) & 0x1F;
        let funct7 = (inst >> 25) & 0x7F;

        let imm_i: i32 = (((inst as i32) >> 20) as u32) as i32;
        let imm_s: i32 = ((((inst >> 7) & 0x1F) | ((inst >> 25) << 5)) as i32) << 20 >> 20;
        let imm_b: i32 = (((((inst >> 7) & 0x1) << 11)
            | (((inst >> 8) & 0xF) << 1)
            | (((inst >> 25) & 0x3F) << 5)
            | ((inst >> 31) << 12)) as i32)
            << 19
            >> 19;
        let imm_u = inst & 0xFFFFF000;
        let imm_j: i32 = (((((inst >> 12) & 0xFF) << 12)
            | (((inst >> 20) & 0x1) << 11)
            | (((inst >> 21) & 0x3FF) << 1)
            | ((inst >> 31) << 20)) as i32)
            << 11
            >> 11;

        let next_pc = self.pc.wrapping_add(4);
        let mut pc_jumped = false;

        match opcode {
            0x37 => {
                // LUI
                self.write_reg(rd, imm_u);
            }
            0x17 => {
                // AUIPC
                self.write_reg(rd, self.pc.wrapping_add(imm_u));
            }
            0x6F => {
                // JAL
                self.write_reg(rd, next_pc);
                self.pc = self.pc.wrapping_add(imm_j as u32);
                pc_jumped = true;
            }
            0x67 => {
                // JALR
                let target = (self.read_reg(rs1).wrapping_add(imm_i as u32)) & !1;
                self.write_reg(rd, next_pc);
                self.pc = target;
                pc_jumped = true;
            }
            0x63 => {
                // BRANCH
                let val1 = self.read_reg(rs1);
                let val2 = self.read_reg(rs2);
                let take = match funct3 {
                    0 => val1 == val2,                   // BEQ
                    1 => val1 != val2,                   // BNE
                    4 => (val1 as i32) < (val2 as i32),  // BLT
                    5 => (val1 as i32) >= (val2 as i32), // BGE
                    6 => val1 < val2,                    // BLTU
                    7 => val1 >= val2,                   // BGEU
                    _ => false,
                };
                if take {
                    self.pc = self.pc.wrapping_add(imm_b as u32);
                    pc_jumped = true;
                }
            }
            0x03 => {
                // LOAD (LW only, matching WGSL)
                let addr = self.read_reg(rs1).wrapping_add(imm_i as u32);
                let val = self.read_ram(addr);
                self.write_reg(rd, val);
            }
            0x23 => {
                // STORE (SW only, matching WGSL)
                let addr = self.read_reg(rs1).wrapping_add(imm_s as u32);
                let val = self.read_reg(rs2);
                self.write_ram(addr, val);
            }
            0x13 => {
                // OP-IMM
                let val1 = self.read_reg(rs1);
                let shamt = rs2; // imm[4:0]
                let result = match funct3 {
                    0 => val1.wrapping_add(imm_i as u32), // ADDI
                    1 => val1 << shamt,                   // SLLI
                    2 => {
                        if (val1 as i32) < imm_i {
                            1
                        } else {
                            0
                        }
                    } // SLTI
                    3 => {
                        if val1 < (imm_i as u32) {
                            1
                        } else {
                            0
                        }
                    } // SLTIU
                    4 => val1 ^ (imm_i as u32),           // XORI
                    5 => {
                        if (inst >> 30) & 1 == 1 {
                            ((val1 as i32) >> shamt) as u32 // SRAI
                        } else {
                            val1 >> shamt // SRLI
                        }
                    }
                    6 => val1 | (imm_i as u32), // ORI
                    7 => val1 & (imm_i as u32), // ANDI
                    _ => 0,
                };
                self.write_reg(rd, result);
            }
            0x33 => {
                // OP
                let val1 = self.read_reg(rs1);
                let val2 = self.read_reg(rs2);
                let alt = (inst >> 30) & 1 == 1;
                let mul = funct7 == 0x01;

                let result = if mul {
                    match funct3 {
                        0 => (val1 as i32).wrapping_mul(val2 as i32) as u32, // MUL
                        1 => (((val1 as i64) * (val2 as i64)) >> 32) as u32, // MULH
                        _ => 0,
                    }
                } else {
                    match funct3 {
                        0 => {
                            if alt {
                                val1.wrapping_sub(val2)
                            } else {
                                val1.wrapping_add(val2)
                            }
                        } // ADD/SUB
                        1 => val1 << (val2 & 0x1F), // SLL
                        2 => {
                            if (val1 as i32) < (val2 as i32) {
                                1
                            } else {
                                0
                            }
                        } // SLT
                        3 => {
                            if val1 < val2 {
                                1
                            } else {
                                0
                            }
                        } // SLTU
                        4 => val1 ^ val2,           // XOR
                        5 => {
                            if alt {
                                ((val1 as i32) >> (val2 & 0x1F)) as u32
                            } else {
                                val1 >> (val2 & 0x1F)
                            }
                        } // SRL/SRA
                        6 => val1 | val2,           // OR
                        7 => val1 & val2,           // AND
                        _ => 0,
                    }
                };
                self.write_reg(rd, result);
            }
            0x73 => {
                // SYSTEM
                if inst == 0x00000073 {
                    // ECALL - halt
                    self.status |= STATUS_HALTED;
                } else if inst == 0x00100073 {
                    // EBREAK - halt with error
                    self.status |= STATUS_HALTED | STATUS_ERROR;
                }
            }
            _ => {
                // Unknown instruction - halt with error (match WGSL)
                self.status |= STATUS_HALTED | STATUS_ERROR;
            }
        }

        if !pc_jumped {
            self.pc = next_pc;
        }

        self.instruction_count += 1;
        self.status == STATUS_RUNNING
    }

    /// Run up to `max_steps` instructions.
    pub fn run(&mut self, max_steps: u32) {
        for _ in 0..max_steps {
            if !self.step() {
                break;
            }
        }
        // Clear running bit when done
        self.status &= !STATUS_RUNNING;
    }

    /// Get the tile state as a u32 array compatible with the GPU tile layout.
    /// Useful for comparing GPU vs reference results.
    pub fn to_tile_state(&self) -> Vec<u32> {
        let mut buf = vec![0u32; super::gpu::TILE_STATE_WORDS];
        buf[0..32].copy_from_slice(&self.regs);
        buf[32] = self.pc;
        buf[33] = self.status;
        buf[34] = self.instruction_count;
        // UART output
        let uart_len = self.uart_output.len().min(super::gpu::UART_BUF_WORDS);
        buf[37] = uart_len as u32;
        for i in 0..uart_len {
            buf[super::gpu::STATE_HEADER_WORDS + i] = self.uart_output[i] as u32;
        }
        // RAM
        let ram_base = super::gpu::STATE_HEADER_WORDS + super::gpu::UART_BUF_WORDS;
        for (i, &word) in self.ram.iter().enumerate() {
            if i < RAM_WORDS {
                buf[ram_base + i] = word;
            }
        }
        buf
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reference_vm_new() {
        let ram = vec![0u32; RAM_WORDS];
        let vm = ReferenceVm::new(ram);
        assert_eq!(vm.pc, 0);
        assert_eq!(vm.status, STATUS_RUNNING);
        assert_eq!(vm.instruction_count, 0);
        assert!(vm.regs.iter().all(|&r| r == 0));
    }

    #[test]
    fn test_reference_addi() {
        // ADDI x1, x0, 42
        let mut ram = vec![0u32; RAM_WORDS];
        ram[0] = 0x02A00093; // ADDI x1, x0, 42
        ram[1] = 0x00000073; // ECALL (halt)

        let mut vm = ReferenceVm::new(ram);
        vm.run(100);

        assert_eq!(vm.regs[1], 42);
        assert_ne!(vm.status & STATUS_HALTED, 0);
    }

    #[test]
    fn test_reference_lui_addi() {
        // LUI x1, 0x10000   -> x1 = 0x10000000 (UART base)
        // ADDI x2, x0, 0x41 -> x2 = 'A'
        // SW x2, 0(x1)      -> write to UART
        // ECALL              -> halt
        let mut ram = vec![0u32; RAM_WORDS];
        ram[0] = 0x100000B7;
        ram[1] = 0x04100113;
        ram[2] = 0x00208023;
        ram[3] = 0x00000073;

        let mut vm = ReferenceVm::new(ram);
        vm.run(100);

        assert_eq!(vm.uart_output, vec![b'A']);
        assert_ne!(vm.status & STATUS_HALTED, 0);
    }

    #[test]
    fn test_reference_fibonacci() {
        // Use the gpu_loader cartridge builder
        let cartridge = super::super::gpu_loader::build_fibonacci_cartridge();
        let mut ram = vec![0u32; RAM_WORDS];
        for (i, &word) in cartridge.iter().enumerate() {
            if i < RAM_WORDS {
                ram[i] = word;
            }
        }

        let mut vm = ReferenceVm::new(ram);
        vm.run(1000);

        assert!(vm.instruction_count > 0);
        assert_ne!(vm.status & STATUS_HALTED, 0);
        assert_eq!(vm.status & STATUS_ERROR, 0);

        let uart_str = String::from_utf8_lossy(&vm.uart_output).to_string();
        assert!(
            uart_str.contains('5'),
            "Fibonacci(10)=55 should contain '5', got: {}",
            uart_str
        );
    }

    #[test]
    fn test_reference_counter() {
        let cartridge = super::super::gpu_loader::build_counter_cartridge(5);
        let mut ram = vec![0u32; RAM_WORDS];
        for (i, &word) in cartridge.iter().enumerate() {
            if i < RAM_WORDS {
                ram[i] = word;
            }
        }

        let mut vm = ReferenceVm::new(ram);
        vm.run(1000);

        assert!(vm.instruction_count > 0);
        assert_ne!(vm.status & STATUS_HALTED, 0);
        assert_eq!(vm.status & STATUS_ERROR, 0);

        // Counter writes 0,1,2,3,4 to UART then halts
        assert_eq!(vm.uart_output.len(), 5);
    }

    #[test]
    fn test_reference_to_tile_state() {
        let cartridge = super::super::gpu_loader::build_counter_cartridge(3);
        let mut ram = vec![0u32; RAM_WORDS];
        for (i, &word) in cartridge.iter().enumerate() {
            if i < RAM_WORDS {
                ram[i] = word;
            }
        }

        let mut vm = ReferenceVm::new(ram);
        vm.run(1000);

        let state = vm.to_tile_state();
        assert_eq!(state.len(), super::super::gpu::TILE_STATE_WORDS);
        assert_eq!(state[32], vm.pc);
        assert_eq!(state[33], vm.status);
        assert_eq!(state[34], vm.instruction_count);
    }
}
