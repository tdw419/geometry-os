// riscv_fuzzer.rs -- Oracle-based fuzzer for the RISC-V interpreter
//
// Generates random RV32I + M-extension programs, runs them through
// RiscvVm, and compares results against a pure-Rust reference oracle.
// Any divergence is printed and the process exits non-zero.

use geometry_os::riscv::{self, cpu};

// ─── LCG RNG ───────────────────────────────────────────────────────────────

struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed ^ 0xDEAD_BEEF_CAFE_1234)
    }
    fn next(&mut self) -> u64 {
        self.0 = self.0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0
    }
    fn u32(&mut self) -> u32 {
        self.next() as u32
    }
    fn range(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

// ─── Instruction encoders ──────────────────────────────────────────────────

fn enc_r(funct7: u32, rs2: u8, rs1: u8, funct3: u32, rd: u8, opcode: u32) -> u32 {
    (funct7 << 25)
        | ((rs2 as u32) << 20)
        | ((rs1 as u32) << 15)
        | (funct3 << 12)
        | ((rd as u32) << 7)
        | opcode
}

fn enc_i(imm12: i32, rs1: u8, funct3: u32, rd: u8, opcode: u32) -> u32 {
    let imm = (imm12 as u32) & 0xFFF;
    (imm << 20) | ((rs1 as u32) << 15) | (funct3 << 12) | ((rd as u32) << 7) | opcode
}

fn enc_lui(rd: u8, imm: u32) -> u32 {
    (imm & 0xFFFF_F000) | ((rd as u32) << 7) | 0x37
}

fn enc_addi(rd: u8, rs1: u8, imm: i32) -> u32 {
    enc_i(imm, rs1, 0x0, rd, 0x13)
}

fn enc_ebreak() -> u32 {
    0x00100073
}

// ─── Load arbitrary 32-bit value into a register ──────────────────────────
// Standard RISC-V two-instruction sequence, handling sign-extension of lo12.

fn load_const(rd: u8, value: u32, out: &mut Vec<u32>) {
    // Sign-extend lower 12 bits
    let lo12 = ((value as i32) << 20) >> 20; // sign-extend 12-bit
    // Upper 20 bits, adjusted so that hi20 + lo12 = value
    let hi20 = value.wrapping_sub(lo12 as u32) & 0xFFFF_F000;

    if hi20 != 0 {
        out.push(enc_lui(rd, hi20));
        if lo12 != 0 {
            out.push(enc_addi(rd, rd, lo12));
        }
    } else {
        // No upper bits — ADDI x0, x0, imm (or ADDI rd, x0, imm)
        out.push(enc_addi(rd, 0, lo12));
    }
}

// ─── Oracle (pure Rust reference) ─────────────────────────────────────────

#[derive(Clone)]
struct Oracle {
    x: [u32; 32],
}

impl Oracle {
    fn new() -> Self {
        Self { x: [0u32; 32] }
    }

    fn set(&mut self, rd: u8, val: u32) {
        if rd != 0 {
            self.x[rd as usize] = val;
        }
    }

    fn r(&self, rs: u8) -> u32 {
        self.x[rs as usize]
    }

    /// Apply one oracle operation, return new rd value (or None for stores/branches).
    fn apply(&mut self, op: &OracleOp) {
        match *op {
            OracleOp::Add { rd, rs1, rs2 } => {
                let v = self.r(rs1).wrapping_add(self.r(rs2));
                self.set(rd, v);
            }
            OracleOp::Sub { rd, rs1, rs2 } => {
                let v = self.r(rs1).wrapping_sub(self.r(rs2));
                self.set(rd, v);
            }
            OracleOp::And { rd, rs1, rs2 } => {
                let v = self.r(rs1) & self.r(rs2);
                self.set(rd, v);
            }
            OracleOp::Or { rd, rs1, rs2 } => {
                let v = self.r(rs1) | self.r(rs2);
                self.set(rd, v);
            }
            OracleOp::Xor { rd, rs1, rs2 } => {
                let v = self.r(rs1) ^ self.r(rs2);
                self.set(rd, v);
            }
            OracleOp::Sll { rd, rs1, rs2 } => {
                let shamt = self.r(rs2) & 0x1F;
                let v = self.r(rs1) << shamt;
                self.set(rd, v);
            }
            OracleOp::Srl { rd, rs1, rs2 } => {
                let shamt = self.r(rs2) & 0x1F;
                let v = self.r(rs1) >> shamt;
                self.set(rd, v);
            }
            OracleOp::Sra { rd, rs1, rs2 } => {
                let shamt = self.r(rs2) & 0x1F;
                let v = ((self.r(rs1) as i32) >> shamt) as u32;
                self.set(rd, v);
            }
            OracleOp::Slt { rd, rs1, rs2 } => {
                let v = if (self.r(rs1) as i32) < (self.r(rs2) as i32) { 1 } else { 0 };
                self.set(rd, v);
            }
            OracleOp::Sltu { rd, rs1, rs2 } => {
                let v = if self.r(rs1) < self.r(rs2) { 1 } else { 0 };
                self.set(rd, v);
            }
            OracleOp::Mul { rd, rs1, rs2 } => {
                let v = self.r(rs1).wrapping_mul(self.r(rs2));
                self.set(rd, v);
            }
            OracleOp::Mulh { rd, rs1, rs2 } => {
                let a = self.r(rs1) as i32 as i64;
                let b = self.r(rs2) as i32 as i64;
                let v = ((a * b) >> 32) as u32;
                self.set(rd, v);
            }
            OracleOp::Mulhu { rd, rs1, rs2 } => {
                let a = self.r(rs1) as u64;
                let b = self.r(rs2) as u64;
                let v = ((a * b) >> 32) as u32;
                self.set(rd, v);
            }
            OracleOp::Mulhsu { rd, rs1, rs2 } => {
                let a = self.r(rs1) as i32 as i64 as u64;
                let b = self.r(rs2) as u64;
                let v = (((a.wrapping_mul(b)) as u64) >> 32) as u32;
                self.set(rd, v);
            }
            OracleOp::Div { rd, rs1, rs2 } => {
                let a = self.r(rs1) as i32;
                let b = self.r(rs2) as i32;
                let v = if b == 0 {
                    u32::MAX
                } else if a == i32::MIN && b == -1 {
                    i32::MIN as u32
                } else {
                    a.wrapping_div(b) as u32
                };
                self.set(rd, v);
            }
            OracleOp::Divu { rd, rs1, rs2 } => {
                let b = self.r(rs2);
                let v = if b == 0 { u32::MAX } else { self.r(rs1) / b };
                self.set(rd, v);
            }
            OracleOp::Rem { rd, rs1, rs2 } => {
                let a = self.r(rs1) as i32;
                let b = self.r(rs2) as i32;
                let v = if b == 0 {
                    a as u32
                } else if a == i32::MIN && b == -1 {
                    0
                } else {
                    a.wrapping_rem(b) as u32
                };
                self.set(rd, v);
            }
            OracleOp::Remu { rd, rs1, rs2 } => {
                let b = self.r(rs2);
                let v = if b == 0 { self.r(rs1) } else { self.r(rs1) % b };
                self.set(rd, v);
            }
            OracleOp::Addi { rd, rs1, imm } => {
                let v = self.r(rs1).wrapping_add(imm as u32);
                self.set(rd, v);
            }
            OracleOp::LoadConst { rd, value } => {
                self.set(rd, value);
            }
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
    Addi { rd: u8, rs1: u8, imm: i32 },
    LoadConst { rd: u8, value: u32 },
}

// ─── Program generator ────────────────────────────────────────────────────

struct Program {
    words: Vec<u32>,
    ops: Vec<OracleOp>,
}

fn gen_program(rng: &mut Rng, n_ops: usize) -> Program {
    let mut words: Vec<u32> = Vec::new();
    let mut ops: Vec<OracleOp> = Vec::new();

    // Initialize x1-x8 with random 32-bit values using LUI+ADDI pairs.
    // x0 stays 0 (hardwired). x1-x8 are our working registers.
    for rd in 1u8..=8 {
        let value = rng.u32();
        load_const(rd, value, &mut words);
        ops.push(OracleOp::LoadConst { rd, value });
    }

    // Generate random ALU ops using x1-x8 as operands, writing to x1-x8.
    const NUM_OPS: usize = 18;
    for _ in 0..n_ops {
        let rd  = (rng.range(8) + 1) as u8;  // x1-x8
        let rs1 = (rng.range(8) + 1) as u8;
        let rs2 = (rng.range(8) + 1) as u8;

        let op_idx = rng.range(NUM_OPS as u64) as usize;
        let (word, op) = match op_idx {
            0  => (enc_r(0x00, rs2, rs1, 0x0, rd, 0x33), OracleOp::Add { rd, rs1, rs2 }),
            1  => (enc_r(0x20, rs2, rs1, 0x0, rd, 0x33), OracleOp::Sub { rd, rs1, rs2 }),
            2  => (enc_r(0x00, rs2, rs1, 0x7, rd, 0x33), OracleOp::And { rd, rs1, rs2 }),
            3  => (enc_r(0x00, rs2, rs1, 0x6, rd, 0x33), OracleOp::Or  { rd, rs1, rs2 }),
            4  => (enc_r(0x00, rs2, rs1, 0x4, rd, 0x33), OracleOp::Xor { rd, rs1, rs2 }),
            5  => (enc_r(0x00, rs2, rs1, 0x1, rd, 0x33), OracleOp::Sll { rd, rs1, rs2 }),
            6  => (enc_r(0x00, rs2, rs1, 0x5, rd, 0x33), OracleOp::Srl { rd, rs1, rs2 }),
            7  => (enc_r(0x20, rs2, rs1, 0x5, rd, 0x33), OracleOp::Sra { rd, rs1, rs2 }),
            8  => (enc_r(0x00, rs2, rs1, 0x2, rd, 0x33), OracleOp::Slt { rd, rs1, rs2 }),
            9  => (enc_r(0x00, rs2, rs1, 0x3, rd, 0x33), OracleOp::Sltu { rd, rs1, rs2 }),
            // M extension
            10 => (enc_r(0x01, rs2, rs1, 0x0, rd, 0x33), OracleOp::Mul { rd, rs1, rs2 }),
            11 => (enc_r(0x01, rs2, rs1, 0x1, rd, 0x33), OracleOp::Mulh { rd, rs1, rs2 }),
            12 => (enc_r(0x01, rs2, rs1, 0x3, rd, 0x33), OracleOp::Mulhu { rd, rs1, rs2 }),
            13 => (enc_r(0x01, rs2, rs1, 0x2, rd, 0x33), OracleOp::Mulhsu { rd, rs1, rs2 }),
            14 => (enc_r(0x01, rs2, rs1, 0x4, rd, 0x33), OracleOp::Div { rd, rs1, rs2 }),
            15 => (enc_r(0x01, rs2, rs1, 0x5, rd, 0x33), OracleOp::Divu { rd, rs1, rs2 }),
            16 => (enc_r(0x01, rs2, rs1, 0x6, rd, 0x33), OracleOp::Rem { rd, rs1, rs2 }),
            17 => (enc_r(0x01, rs2, rs1, 0x7, rd, 0x33), OracleOp::Remu { rd, rs1, rs2 }),
            _  => unreachable!(),
        };
        words.push(word);
        ops.push(op);
    }

    words.push(enc_ebreak());

    Program { words, ops }
}

// ─── Run a program through the RISC-V VM ──────────────────────────────────

const RAM_BASE: u64 = 0x8000_0000;
const RAM_SIZE: usize = 65536;

fn run_program(prog: &Program) -> Result<[u32; 32], String> {
    let mut vm = riscv::RiscvVm::new_with_base(RAM_BASE, RAM_SIZE);
    vm.cpu.pc = RAM_BASE as u32;
    // Disable all interrupts
    vm.cpu.csr.satp = 0;
    vm.cpu.csr.mie = 0;
    vm.cpu.csr.mstatus = 0;

    // Write instructions
    for (i, &word) in prog.words.iter().enumerate() {
        let addr = RAM_BASE + (i as u64) * 4;
        vm.bus.write_word(addr, word)
            .map_err(|e| format!("write_word at {:08x}: {:?}", addr, e))?;
    }

    // Run until EBREAK or error
    let max_steps = prog.words.len() + 10;
    for _ in 0..max_steps {
        match vm.step() {
            cpu::StepResult::Ok => {}
            cpu::StepResult::Ebreak => return Ok(vm.cpu.x),
            other => return Err(format!("unexpected StepResult: {:?} at pc={:08x}", other, vm.cpu.pc)),
        }
    }
    Err(format!("program did not EBREAK within {} steps", max_steps))
}

// ─── Compare oracle vs VM ─────────────────────────────────────────────────

fn check_program(prog: &Program, vm_regs: &[u32; 32]) -> bool {
    let mut oracle = Oracle::new();
    let mut ok = true;

    for op in &prog.ops {
        oracle.apply(op);
    }

    for reg in 1u8..=8 {
        let expected = oracle.x[reg as usize];
        let got = vm_regs[reg as usize];
        if expected != got {
            eprintln!("  x{}: oracle={:#010x}  vm={:#010x}", reg, expected, got);
            ok = false;
        }
    }
    ok
}

// ─── Main ─────────────────────────────────────────────────────────────────

fn main() {
    let n_programs: u64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(10_000);

    let n_ops: usize = std::env::args()
        .nth(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(20);

    let seed: u64 = std::env::args()
        .nth(3)
        .and_then(|s| s.parse().ok())
        .unwrap_or(42);

    let mut rng = Rng::new(seed);
    let mut failures = 0u64;

    eprintln!("RISC-V oracle fuzzer: {} programs, {} ops each, seed={}", n_programs, n_ops, seed);

    for i in 0..n_programs {
        let prog = gen_program(&mut rng, n_ops);
        match run_program(&prog) {
            Err(e) => {
                eprintln!("program {}: VM error: {}", i, e);
                failures += 1;
            }
            Ok(vm_regs) => {
                if !check_program(&prog, &vm_regs) {
                    eprintln!("program {}: oracle mismatch (ops below):", i);
                    for op in &prog.ops {
                        eprintln!("  {:?}", op);
                    }
                    failures += 1;
                    if failures >= 5 {
                        eprintln!("too many failures, stopping");
                        std::process::exit(1);
                    }
                }
            }
        }

        if (i + 1) % 1000 == 0 {
            eprintln!("  {} / {} done, {} failures", i + 1, n_programs, failures);
        }
    }

    if failures == 0 {
        eprintln!("OK: {} programs passed", n_programs);
    } else {
        eprintln!("FAILED: {} / {} programs had mismatches", failures, n_programs);
        std::process::exit(1);
    }
}
