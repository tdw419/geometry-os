// riscv_fuzzer.rs -- Oracle-based fuzzer for the RISC-V interpreter
//
// Generates random RV32I + M-extension programs, runs them through
// RiscvVm, and compares results against a pure-Rust reference oracle.
// Any divergence is printed and the process exits non-zero.
//
// Memory layout (all within 64K RAM at RAM_BASE = 0x8000_0000):
//   0x0000..0x7FFF  CODE region  (max 8192 instructions)
//   0x8000..0xFFFF  DATA region  (x9 = RAM_BASE + 0x8000, shadow oracle)

use geometry_os::riscv::{self, cpu};

const RAM_BASE: u64    = 0x8000_0000;
const RAM_SIZE: usize  = 65536;
const DATA_OFF: u64    = 0x8000;  // data region starts here (relative to RAM_BASE)
const DATA_SIZE: usize = 0x8000;  // 32K data region
// x9 is the dedicated data-base register (never overwritten by random ops)
const BASE_REG: u8     = 9;
// Data region slot counts
const WORD_SLOTS: u32  = 64;   // word-aligned slots  (offsets 0,4,8,...,252)
const HALF_SLOTS: u32  = 128;  // half-aligned slots  (offsets 0,2,4,...,254)
const BYTE_SLOTS: u32  = 256;  // byte slots          (offsets 0..255)

// ─── LCG RNG ───────────────────────────────────────────────────────────────

struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self { Self(seed ^ 0xDEAD_BEEF_CAFE_1234) }
    fn next(&mut self) -> u64 {
        self.0 = self.0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0
    }
    fn u32(&mut self) -> u32 { self.next() as u32 }
    fn range(&mut self, n: u64) -> u64 { self.next() % n }
}

// ─── Instruction encoders ──────────────────────────────────────────────────

fn enc_r(funct7: u32, rs2: u8, rs1: u8, funct3: u32, rd: u8, opcode: u32) -> u32 {
    (funct7 << 25) | ((rs2 as u32) << 20) | ((rs1 as u32) << 15)
        | (funct3 << 12) | ((rd as u32) << 7) | opcode
}

fn enc_i(imm12: i32, rs1: u8, funct3: u32, rd: u8, opcode: u32) -> u32 {
    let imm = (imm12 as u32) & 0xFFF;
    (imm << 20) | ((rs1 as u32) << 15) | (funct3 << 12) | ((rd as u32) << 7) | opcode
}

// S-type: SW, SH, SB
fn enc_s(imm12: i32, rs2: u8, rs1: u8, funct3: u32) -> u32 {
    let imm = (imm12 as u32) & 0xFFF;
    let imm_11_5 = imm >> 5;
    let imm_4_0  = imm & 0x1F;
    (imm_11_5 << 25) | ((rs2 as u32) << 20) | ((rs1 as u32) << 15)
        | (funct3 << 12) | (imm_4_0 << 7) | 0x23
}

fn enc_lui(rd: u8, imm: u32) -> u32 {
    (imm & 0xFFFF_F000) | ((rd as u32) << 7) | 0x37
}

fn enc_addi(rd: u8, rs1: u8, imm: i32) -> u32 {
    enc_i(imm, rs1, 0x0, rd, 0x13)
}

fn enc_ebreak() -> u32 { 0x00100073 }

// Load 32-bit constant into rd using LUI + ADDI, handling sign-extension.
fn load_const(rd: u8, value: u32, out: &mut Vec<u32>) {
    let lo12 = ((value as i32) << 20) >> 20; // sign-extend lower 12 bits
    let hi20 = value.wrapping_sub(lo12 as u32) & 0xFFFF_F000;
    if hi20 != 0 {
        out.push(enc_lui(rd, hi20));
        if lo12 != 0 {
            out.push(enc_addi(rd, rd, lo12));
        }
    } else {
        out.push(enc_addi(rd, 0, lo12));
    }
}

// ─── Oracle ────────────────────────────────────────────────────────────────

struct Oracle {
    x:   [u32; 32],
    mem: [u8; DATA_SIZE], // shadow of data region
}

impl Oracle {
    fn new() -> Self {
        Self { x: [0u32; 32], mem: [0u8; DATA_SIZE] }
    }

    fn set(&mut self, rd: u8, val: u32) {
        if rd != 0 { self.x[rd as usize] = val; }
    }

    fn r(&self, rs: u8) -> u32 { self.x[rs as usize] }

    // Data-region memory access helpers
    fn mem_write_word(&mut self, off: usize, val: u32) {
        let b = val.to_le_bytes();
        self.mem[off..off+4].copy_from_slice(&b);
    }
    fn mem_read_word(&self, off: usize) -> u32 {
        u32::from_le_bytes(self.mem[off..off+4].try_into().unwrap())
    }
    fn mem_write_half(&mut self, off: usize, val: u16) {
        let b = val.to_le_bytes();
        self.mem[off..off+2].copy_from_slice(&b);
    }
    fn mem_read_half(&self, off: usize) -> u16 {
        u16::from_le_bytes(self.mem[off..off+2].try_into().unwrap())
    }
    fn mem_write_byte(&mut self, off: usize, val: u8) {
        self.mem[off] = val;
    }
    fn mem_read_byte(&self, off: usize) -> u8 {
        self.mem[off]
    }

    fn apply(&mut self, op: &OracleOp) {
        match *op {
            // ── ALU R-type ──
            OracleOp::Add   { rd, rs1, rs2 } => { let v = self.r(rs1).wrapping_add(self.r(rs2)); self.set(rd, v); }
            OracleOp::Sub   { rd, rs1, rs2 } => { let v = self.r(rs1).wrapping_sub(self.r(rs2)); self.set(rd, v); }
            OracleOp::And   { rd, rs1, rs2 } => { let v = self.r(rs1) & self.r(rs2); self.set(rd, v); }
            OracleOp::Or    { rd, rs1, rs2 } => { let v = self.r(rs1) | self.r(rs2); self.set(rd, v); }
            OracleOp::Xor   { rd, rs1, rs2 } => { let v = self.r(rs1) ^ self.r(rs2); self.set(rd, v); }
            OracleOp::Sll   { rd, rs1, rs2 } => { let v = self.r(rs1) << (self.r(rs2) & 0x1F); self.set(rd, v); }
            OracleOp::Srl   { rd, rs1, rs2 } => { let v = self.r(rs1) >> (self.r(rs2) & 0x1F); self.set(rd, v); }
            OracleOp::Sra   { rd, rs1, rs2 } => { let v = ((self.r(rs1) as i32) >> (self.r(rs2) & 0x1F)) as u32; self.set(rd, v); }
            OracleOp::Slt   { rd, rs1, rs2 } => { let v = ((self.r(rs1) as i32) < (self.r(rs2) as i32)) as u32; self.set(rd, v); }
            OracleOp::Sltu  { rd, rs1, rs2 } => { let v = (self.r(rs1) < self.r(rs2)) as u32; self.set(rd, v); }
            // ── M extension ──
            OracleOp::Mul   { rd, rs1, rs2 } => { let v = self.r(rs1).wrapping_mul(self.r(rs2)); self.set(rd, v); }
            OracleOp::Mulh  { rd, rs1, rs2 } => {
                let v = (((self.r(rs1) as i32 as i64) * (self.r(rs2) as i32 as i64)) >> 32) as u32;
                self.set(rd, v);
            }
            OracleOp::Mulhu { rd, rs1, rs2 } => {
                let v = (((self.r(rs1) as u64) * (self.r(rs2) as u64)) >> 32) as u32;
                self.set(rd, v);
            }
            OracleOp::Mulhsu { rd, rs1, rs2 } => {
                let a = self.r(rs1) as i32 as i64 as u64;
                let b = self.r(rs2) as u64;
                let v = ((a.wrapping_mul(b)) >> 32) as u32;
                self.set(rd, v);
            }
            OracleOp::Div   { rd, rs1, rs2 } => {
                let (a, b) = (self.r(rs1) as i32, self.r(rs2) as i32);
                let v = if b == 0 { u32::MAX } else if a == i32::MIN && b == -1 { i32::MIN as u32 } else { a.wrapping_div(b) as u32 };
                self.set(rd, v);
            }
            OracleOp::Divu  { rd, rs1, rs2 } => {
                let b = self.r(rs2);
                let v = if b == 0 { u32::MAX } else { self.r(rs1) / b };
                self.set(rd, v);
            }
            OracleOp::Rem   { rd, rs1, rs2 } => {
                let (a, b) = (self.r(rs1) as i32, self.r(rs2) as i32);
                let v = if b == 0 { a as u32 } else if a == i32::MIN && b == -1 { 0 } else { a.wrapping_rem(b) as u32 };
                self.set(rd, v);
            }
            OracleOp::Remu  { rd, rs1, rs2 } => {
                let b = self.r(rs2);
                let v = if b == 0 { self.r(rs1) } else { self.r(rs1) % b };
                self.set(rd, v);
            }
            // ── Load/store (offsets relative to data base) ──
            OracleOp::Sw  { rs2, off } => { let v = self.r(rs2); self.mem_write_word(off, v); }
            OracleOp::Sh  { rs2, off } => { let v = self.r(rs2) as u16; self.mem_write_half(off, v); }
            OracleOp::Sb  { rs2, off } => { let v = self.r(rs2) as u8; self.mem_write_byte(off, v); }
            OracleOp::Lw  { rd,  off } => { let v = self.mem_read_word(off); self.set(rd, v); }
            OracleOp::Lh  { rd,  off } => { let v = self.mem_read_half(off) as i16 as u32; self.set(rd, v); }
            OracleOp::Lhu { rd,  off } => { let v = self.mem_read_half(off) as u32; self.set(rd, v); }
            OracleOp::Lb  { rd,  off } => { let v = self.mem_read_byte(off) as i8 as u32; self.set(rd, v); }
            OracleOp::Lbu { rd,  off } => { let v = self.mem_read_byte(off) as u32; self.set(rd, v); }
            // ── Init ──
            OracleOp::LoadConst { rd, value } => { self.set(rd, value); }
        }
    }
}

#[derive(Clone, Debug)]
enum OracleOp {
    Add { rd: u8, rs1: u8, rs2: u8 },
    Sub { rd: u8, rs1: u8, rs2: u8 },
    And { rd: u8, rs1: u8, rs2: u8 },
    Or  { rd: u8, rs1: u8, rs2: u8 },
    Xor { rd: u8, rs1: u8, rs2: u8 },
    Sll { rd: u8, rs1: u8, rs2: u8 },
    Srl { rd: u8, rs1: u8, rs2: u8 },
    Sra { rd: u8, rs1: u8, rs2: u8 },
    Slt { rd: u8, rs1: u8, rs2: u8 },
    Sltu { rd: u8, rs1: u8, rs2: u8 },
    Mul { rd: u8, rs1: u8, rs2: u8 },
    Mulh { rd: u8, rs1: u8, rs2: u8 },
    Mulhu { rd: u8, rs1: u8, rs2: u8 },
    Mulhsu { rd: u8, rs1: u8, rs2: u8 },
    Div { rd: u8, rs1: u8, rs2: u8 },
    Divu { rd: u8, rs1: u8, rs2: u8 },
    Rem { rd: u8, rs1: u8, rs2: u8 },
    Remu { rd: u8, rs1: u8, rs2: u8 },
    // Memory — off is byte offset into data region
    Sw  { rs2: u8, off: usize },
    Sh  { rs2: u8, off: usize },
    Sb  { rs2: u8, off: usize },
    Lw  { rd: u8,  off: usize },
    Lh  { rd: u8,  off: usize },
    Lhu { rd: u8,  off: usize },
    Lb  { rd: u8,  off: usize },
    Lbu { rd: u8,  off: usize },
    // Init
    LoadConst { rd: u8, value: u32 },
}

// ─── Program generator ────────────────────────────────────────────────────

struct Program {
    words: Vec<u32>,
    ops:   Vec<OracleOp>,
}

fn gen_program(rng: &mut Rng, n_ops: usize) -> Program {
    let mut words: Vec<u32> = Vec::new();
    let mut ops:   Vec<OracleOp> = Vec::new();

    // x1-x8: random data registers
    for rd in 1u8..=8 {
        let value = rng.u32();
        load_const(rd, value, &mut words);
        ops.push(OracleOp::LoadConst { rd, value });
    }

    // x9: data base pointer = RAM_BASE + DATA_OFF (constant throughout program)
    let data_base = (RAM_BASE + DATA_OFF) as u32;
    load_const(BASE_REG, data_base, &mut words);
    ops.push(OracleOp::LoadConst { rd: BASE_REG, value: data_base });

    const N_ALU: usize = 18;
    const N_MEM: usize = 8; // Sw Sh Sb Lw Lh Lhu Lb Lbu
    const N_TOTAL: usize = N_ALU + N_MEM;

    for _ in 0..n_ops {
        let op_idx = rng.range(N_TOTAL as u64) as usize;

        if op_idx < N_ALU {
            // ALU op on x1-x8
            let rd  = (rng.range(8) + 1) as u8;
            let rs1 = (rng.range(8) + 1) as u8;
            let rs2 = (rng.range(8) + 1) as u8;
            let (word, oracle_op) = match op_idx {
                0  => (enc_r(0x00, rs2, rs1, 0x0, rd, 0x33), OracleOp::Add   { rd, rs1, rs2 }),
                1  => (enc_r(0x20, rs2, rs1, 0x0, rd, 0x33), OracleOp::Sub   { rd, rs1, rs2 }),
                2  => (enc_r(0x00, rs2, rs1, 0x7, rd, 0x33), OracleOp::And   { rd, rs1, rs2 }),
                3  => (enc_r(0x00, rs2, rs1, 0x6, rd, 0x33), OracleOp::Or    { rd, rs1, rs2 }),
                4  => (enc_r(0x00, rs2, rs1, 0x4, rd, 0x33), OracleOp::Xor   { rd, rs1, rs2 }),
                5  => (enc_r(0x00, rs2, rs1, 0x1, rd, 0x33), OracleOp::Sll   { rd, rs1, rs2 }),
                6  => (enc_r(0x00, rs2, rs1, 0x5, rd, 0x33), OracleOp::Srl   { rd, rs1, rs2 }),
                7  => (enc_r(0x20, rs2, rs1, 0x5, rd, 0x33), OracleOp::Sra   { rd, rs1, rs2 }),
                8  => (enc_r(0x00, rs2, rs1, 0x2, rd, 0x33), OracleOp::Slt   { rd, rs1, rs2 }),
                9  => (enc_r(0x00, rs2, rs1, 0x3, rd, 0x33), OracleOp::Sltu  { rd, rs1, rs2 }),
                10 => (enc_r(0x01, rs2, rs1, 0x0, rd, 0x33), OracleOp::Mul   { rd, rs1, rs2 }),
                11 => (enc_r(0x01, rs2, rs1, 0x1, rd, 0x33), OracleOp::Mulh  { rd, rs1, rs2 }),
                12 => (enc_r(0x01, rs2, rs1, 0x3, rd, 0x33), OracleOp::Mulhu { rd, rs1, rs2 }),
                13 => (enc_r(0x01, rs2, rs1, 0x2, rd, 0x33), OracleOp::Mulhsu{ rd, rs1, rs2 }),
                14 => (enc_r(0x01, rs2, rs1, 0x4, rd, 0x33), OracleOp::Div   { rd, rs1, rs2 }),
                15 => (enc_r(0x01, rs2, rs1, 0x5, rd, 0x33), OracleOp::Divu  { rd, rs1, rs2 }),
                16 => (enc_r(0x01, rs2, rs1, 0x6, rd, 0x33), OracleOp::Rem   { rd, rs1, rs2 }),
                17 => (enc_r(0x01, rs2, rs1, 0x7, rd, 0x33), OracleOp::Remu  { rd, rs1, rs2 }),
                _  => unreachable!(),
            };
            words.push(word);
            ops.push(oracle_op);
        } else {
            // Memory op
            let mem_op = op_idx - N_ALU;
            let rd  = (rng.range(8) + 1) as u8;  // x1-x8
            let rs2 = (rng.range(8) + 1) as u8;  // x1-x8

            match mem_op {
                0 => { // SW: store word at word-aligned slot
                    let slot = rng.range(WORD_SLOTS as u64) as u32;
                    let off = (slot * 4) as usize;
                    let imm = off as i32;
                    words.push(enc_s(imm, rs2, BASE_REG, 0x2)); // SW
                    ops.push(OracleOp::Sw { rs2, off });
                }
                1 => { // SH: store half at half-aligned slot
                    let slot = rng.range(HALF_SLOTS as u64) as u32;
                    let off = (slot * 2) as usize;
                    let imm = off as i32;
                    words.push(enc_s(imm, rs2, BASE_REG, 0x1)); // SH
                    ops.push(OracleOp::Sh { rs2, off });
                }
                2 => { // SB: store byte
                    let off = rng.range(BYTE_SLOTS as u64) as usize;
                    words.push(enc_s(off as i32, rs2, BASE_REG, 0x0)); // SB
                    ops.push(OracleOp::Sb { rs2, off });
                }
                3 => { // LW
                    let slot = rng.range(WORD_SLOTS as u64) as u32;
                    let off = (slot * 4) as usize;
                    words.push(enc_i(off as i32, BASE_REG, 0x2, rd, 0x03)); // LW
                    ops.push(OracleOp::Lw { rd, off });
                }
                4 => { // LH signed
                    let slot = rng.range(HALF_SLOTS as u64) as u32;
                    let off = (slot * 2) as usize;
                    words.push(enc_i(off as i32, BASE_REG, 0x1, rd, 0x03)); // LH
                    ops.push(OracleOp::Lh { rd, off });
                }
                5 => { // LHU unsigned
                    let slot = rng.range(HALF_SLOTS as u64) as u32;
                    let off = (slot * 2) as usize;
                    words.push(enc_i(off as i32, BASE_REG, 0x5, rd, 0x03)); // LHU
                    ops.push(OracleOp::Lhu { rd, off });
                }
                6 => { // LB signed
                    let off = rng.range(BYTE_SLOTS as u64) as usize;
                    words.push(enc_i(off as i32, BASE_REG, 0x0, rd, 0x03)); // LB
                    ops.push(OracleOp::Lb { rd, off });
                }
                7 => { // LBU unsigned
                    let off = rng.range(BYTE_SLOTS as u64) as usize;
                    words.push(enc_i(off as i32, BASE_REG, 0x4, rd, 0x03)); // LBU
                    ops.push(OracleOp::Lbu { rd, off });
                }
                _ => unreachable!(),
            }
        }
    }

    words.push(enc_ebreak());
    Program { words, ops }
}

// ─── Run a program through the RISC-V VM ──────────────────────────────────

fn run_program(prog: &Program) -> Result<([u32; 32], Box<[u8]>), String> {
    let mut vm = riscv::RiscvVm::new_with_base(RAM_BASE, RAM_SIZE);
    vm.cpu.pc = RAM_BASE as u32;
    vm.cpu.csr.satp    = 0;
    vm.cpu.csr.mie     = 0;
    vm.cpu.csr.mstatus = 0;

    // Verify code fits before data region
    let code_bytes = prog.words.len() * 4;
    if code_bytes as u64 > DATA_OFF {
        return Err(format!("program too large: {} bytes > {}", code_bytes, DATA_OFF));
    }

    for (i, &word) in prog.words.iter().enumerate() {
        let addr = RAM_BASE + (i as u64) * 4;
        vm.bus.write_word(addr, word)
            .map_err(|e| format!("write at {:08x}: {:?}", addr, e))?;
    }

    let max_steps = prog.words.len() + 10;
    for _ in 0..max_steps {
        match vm.step() {
            cpu::StepResult::Ok    => {}
            cpu::StepResult::Ebreak => {
                // Snapshot data region from bus
                let _data_start = (RAM_BASE + DATA_OFF) as u32;
                let mut data = vec![0u8; DATA_SIZE];
                for i in 0..DATA_SIZE {
                    data[i] = vm.bus.read_byte((RAM_BASE + DATA_OFF + i as u64) as u64)
                        .map_err(|e| format!("read data[{}]: {:?}", i, e))?;
                }
                return Ok((vm.cpu.x, data.into_boxed_slice()));
            }
            other => return Err(format!("StepResult::{:?} at pc={:08x}", other, vm.cpu.pc)),
        }
    }
    Err(format!("no EBREAK within {} steps", max_steps))
}

// ─── Check oracle vs VM ───────────────────────────────────────────────────

fn check_program(prog: &Program, vm_regs: &[u32; 32], vm_data: &[u8]) -> bool {
    let mut oracle = Oracle::new();
    for op in &prog.ops {
        oracle.apply(op);
    }

    let mut ok = true;

    // Check x1-x9 (x9 is data base, should be unchanged)
    for reg in 1u8..=9 {
        let expected = oracle.x[reg as usize];
        let got      = vm_regs[reg as usize];
        if expected != got {
            eprintln!("  x{}: oracle={:#010x}  vm={:#010x}", reg, expected, got);
            ok = false;
        }
    }

    // Check data region
    for i in 0..DATA_SIZE {
        if oracle.mem[i] != vm_data[i] {
            eprintln!("  mem[{}]: oracle={:#04x}  vm={:#04x}", i, oracle.mem[i], vm_data[i]);
            ok = false;
            break; // report first divergence only
        }
    }

    ok
}

// ─── Main ─────────────────────────────────────────────────────────────────

fn main() {
    let n_programs: u64 = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(10_000);
    let n_ops:  usize   = std::env::args().nth(2).and_then(|s| s.parse().ok()).unwrap_or(20);
    let seed:   u64     = std::env::args().nth(3).and_then(|s| s.parse().ok()).unwrap_or(42);

    let mut rng = Rng::new(seed);
    let mut failures = 0u64;

    eprintln!("RISC-V oracle fuzzer: {} programs × {} ops, seed={}", n_programs, n_ops, seed);

    for i in 0..n_programs {
        let prog = gen_program(&mut rng, n_ops);
        match run_program(&prog) {
            Err(e) => {
                eprintln!("program {}: VM error: {}", i, e);
                failures += 1;
            }
            Ok((vm_regs, vm_data)) => {
                if !check_program(&prog, &vm_regs, &vm_data) {
                    eprintln!("program {}: oracle mismatch:", i);
                    for op in &prog.ops {
                        eprintln!("  {:?}", op);
                    }
                    failures += 1;
                    if failures >= 5 {
                        eprintln!("aborting after 5 failures");
                        std::process::exit(1);
                    }
                }
            }
        }

        if (i + 1) % 1000 == 0 {
            eprintln!("  {}/{} done, {} failures", i + 1, n_programs, failures);
        }
    }

    if failures == 0 {
        eprintln!("OK: {} programs passed", n_programs);
    } else {
        eprintln!("FAILED: {}/{} mismatches", failures, n_programs);
        std::process::exit(1);
    }
}
