//! Reference RISC-V Interpreter for GPU Tile Verification
//!
//! Implementation of RV32I base integer instruction set.
//! Used to verify GPU execution by running the same code on CPU and comparing results.

use crate::gpu::{STATUS_HALTED, STATUS_RUNNING, STATUS_ERROR};

pub struct ReferenceVm {
    pub regs: [u32; 32],
    pub pc: u32,
    pub ram: Vec<u32>,
    pub status: u32,
    pub instruction_count: u32,
    pub uart_output: Vec<u8>,
}

impl ReferenceVm {
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
        if reg == 0 { 0 } else { self.regs[reg as usize] }
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
        let _funct7 = (inst >> 25) & 0x7F;

        let imm_i = (((inst as i32) >> 20) as u32) as i32; // sign extended 12-bit
        let imm_s = (((((inst >> 7) & 0x1F) | ((inst >> 25) << 5)) as i32) << 20 >> 20) as i32;
        let imm_b = (((((inst >> 7) & 0x1) << 11) | (((inst >> 8) & 0xF) << 1) | (((inst >> 25) & 0x3F) << 5) | ((inst >> 31) << 12)) as i32) << 19 >> 19;
        let imm_u = inst & 0xFFFFF000;
        let imm_j = (((((inst >> 12) & 0xFF) << 12) | (((inst >> 20) & 0x1) << 11) | (((inst >> 21) & 0x3FF) << 1) | ((inst >> 31) << 20)) as i32) << 11 >> 11;

        let next_pc = self.pc.wrapping_add(4);
        let mut pc_jumped = false;

        match opcode {
            0x37 => { // LUI
                self.write_reg(rd, imm_u);
            }
            0x17 => { // AUIPC
                self.write_reg(rd, self.pc.wrapping_add(imm_u));
            }
            0x6F => { // JAL
                self.write_reg(rd, next_pc);
                self.pc = self.pc.wrapping_add(imm_j as u32);
                pc_jumped = true;
            }
            0x67 => { // JALR
                let target = (self.read_reg(rs1).wrapping_add(imm_i as u32)) & !1;
                self.write_reg(rd, next_pc);
                self.pc = target;
                pc_jumped = true;
            }
            0x63 => { // BRANCH
                let val1 = self.read_reg(rs1);
                let val2 = self.read_reg(rs2);
                let take = match funct3 {
                    0 => val1 == val2,           // BEQ
                    1 => val1 != val2,           // BNE
                    4 => (val1 as i32) < (val2 as i32), // BLT
                    5 => (val1 as i32) >= (val2 as i32), // BGE
                    6 => val1 < val2,            // BLTU
                    7 => val1 >= val2,           // BGEU
                    _ => false,
                };
                if take {
                    self.pc = self.pc.wrapping_add(imm_b as u32);
                    pc_jumped = true;
                }
            }
            0x03 => { // LOAD
                let addr = self.read_reg(rs1).wrapping_add(imm_i as u32);
                // Simplified LW only (match WGSL)
                let val = self.read_ram(addr);
                self.write_reg(rd, val);
            }
            0x23 => { // STORE
                let addr = self.read_reg(rs1).wrapping_add(imm_s as u32);
                let val = self.read_reg(rs2);
                self.write_ram(addr, val);
            }
            0x13 => { // OP-IMM
                let val1 = self.read_reg(rs1);
                let shamt = rs2; // imm[4:0]
                let result = match funct3 {
                    0 => val1.wrapping_add(imm_i as u32), // ADDI
                    1 => val1 << shamt,                   // SLLI
                    2 => if (val1 as i32) < imm_i { 1 } else { 0 }, // SLTI
                    3 => if val1 < (imm_i as u32) { 1 } else { 0 }, // SLTIU
                    4 => val1 ^ (imm_i as u32),           // XORI
                    5 => if (inst >> 30) & 1 == 1 {
                        ((val1 as i32) >> shamt) as u32 // SRAI
                    } else {
                        val1 >> shamt // SRLI
                    },
                    6 => val1 | (imm_i as u32),           // ORI
                    7 => val1 & (imm_i as u32),           // ANDI
                    _ => 0,
                };
                self.write_reg(rd, result);
            }
            0x33 => { // OP
                let val1 = self.read_reg(rs1);
                let val2 = self.read_reg(rs2);
                let alt = (inst >> 30) & 1 == 1;
                let mul = (inst >> 25) & 1 == 1;

                let result = if mul {
                    match funct3 {
                        0 => (val1 as i32).wrapping_mul(val2 as i32) as u32, // MUL
                        1 => (((val1 as i64) * (val2 as i64)) >> 32) as u32, // MULH
                        _ => 0,
                    }
                } else {
                    match funct3 {
                        0 => if alt { val1.wrapping_sub(val2) } else { val1.wrapping_add(val2) }, // ADD/SUB
                        1 => val1 << (val2 & 0x1F), // SLL
                        2 => if (val1 as i32) < (val2 as i32) { 1 } else { 0 }, // SLT
                        3 => if val1 < val2 { 1 } else { 0 }, // SLTU
                        4 => val1 ^ val2, // XOR
                        5 => if alt { ((val1 as i32) >> (val2 & 0x1F)) as u32 } else { val1 >> (val2 & 0x1F) }, // SRL/SRA
                        6 => val1 | val2, // OR
                        7 => val1 & val2, // AND
                        _ => 0,
                    }
                };
                self.write_reg(rd, result);
            }
            0x73 => { // SYSTEM
                if inst == 0x00000073 {
                    self.status |= STATUS_HALTED;
                } else if inst == 0x00100073 {
                    self.status |= STATUS_HALTED | STATUS_ERROR;
                }
            }
            _ => {
                self.status |= STATUS_HALTED | STATUS_ERROR;
            }
        }

        if !pc_jumped {
            self.pc = next_pc;
        }

        self.instruction_count += 1;
        self.status == STATUS_RUNNING
    }

    pub fn run(&mut self, max_steps: u32) {
        for _ in 0..max_steps {
            if !self.step() {
                break;
            }
        }
        // Clear running bit when done
        self.status &= !STATUS_RUNNING;
    }
}
