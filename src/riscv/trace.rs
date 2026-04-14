// riscv/trace.rs -- Instruction-level tracing (Phase 41)
//
// Provides a toggleable, low-overhead instruction trace system.
// Each trace entry captures: PC, raw instruction word, decoded opcode name,
// register snapshot, and step result.
//
// Trace output goes to a fixed-size ring buffer. When the buffer is full,
// new entries overwrite the oldest. This keeps memory bounded regardless
// of how many instructions execute.
//
// Overhead when disabled: one bool check per step (~0).
// Overhead when enabled: register snapshot + ring buffer write (~1.5x measured).

use super::cpu::StepResult;
use super::decode::Operation;

/// Default ring buffer capacity (number of trace entries).
pub const TRACE_CAPACITY: usize = 1 << 20; // ~1M entries

/// A single traced instruction.
#[derive(Debug, Clone)]
pub struct TraceEntry {
    /// Program counter at execution time.
    pub pc: u32,
    /// Raw instruction word (32-bit). For compressed instructions (16-bit),
    /// the upper 16 bits are zero.
    pub word: u32,
    /// Decoded operation enum.
    pub op: Operation,
    /// Instruction length in bytes (2 for compressed, 4 for normal).
    pub inst_len: u32,
    /// Register snapshot BEFORE execution (x0..x31).
    pub regs_before: [u32; 32],
    /// Register snapshot AFTER execution (x0..x31).
    pub regs_after: [u32; 32],
    /// PC after execution.
    pub pc_after: u32,
    /// Step result (Ok, Ecall, Ebreak, etc).
    pub result: StepResult,
    /// Monotonic instruction counter at time of trace.
    pub insn_count: u64,
}

/// Fixed-capacity ring buffer for trace entries.
///
/// Uses a circular buffer with power-of-2 capacity for fast modular indexing.
/// When full, new entries overwrite the oldest.
pub struct TraceBuffer {
    entries: Vec<TraceEntry>,
    /// Index of next write position.
    head: usize,
    /// Number of entries written (may exceed capacity, in which case
    /// old entries were overwritten).
    count: u64,
    /// Capacity (always power of 2).
    cap: usize,
}

impl TraceBuffer {
    /// Create a new ring buffer with the given capacity.
    pub fn new(capacity: usize) -> Self {
        let cap = capacity.next_power_of_two();
        // Pre-allocate with placeholder entries.
        let entries = (0..cap)
            .map(|_| TraceEntry {
                pc: 0,
                word: 0,
                op: Operation::Invalid(0),
                inst_len: 0,
                regs_before: [0; 32],
                regs_after: [0; 32],
                pc_after: 0,
                result: StepResult::Ok,
                insn_count: 0,
            })
            .collect();
        Self {
            entries,
            head: 0,
            count: 0,
            cap,
        }
    }

    /// Push a trace entry into the ring buffer.
    #[inline]
    pub fn push(&mut self, entry: TraceEntry) {
        self.entries[self.head] = entry;
        self.head = (self.head + 1) & (self.cap - 1);
        self.count += 1;
    }

    /// Number of entries currently in the buffer.
    pub fn len(&self) -> usize {
        if self.count >= self.cap as u64 {
            self.cap
        } else {
            self.count as usize
        }
    }

    /// Whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Total number of entries ever written (including overwritten).
    pub fn total_written(&self) -> u64 {
        self.count
    }

    /// Whether old entries were overwritten.
    pub fn overflowed(&self) -> bool {
        self.count > self.cap as u64
    }

    /// Return entries in chronological order (oldest first).
    pub fn iter_chronological(&self) -> impl Iterator<Item = &TraceEntry> {
        let len = self.len();
        let start = if self.count > self.cap as u64 {
            self.head // oldest entry is at head (next to be overwritten)
        } else {
            0
        };
        (0..len).map(move |i| &self.entries[(start + i) & (self.cap - 1)])
    }

    /// Clear the buffer.
    pub fn clear(&mut self) {
        self.head = 0;
        self.count = 0;
    }

    /// Dump all trace entries to a string, one per line.
    /// Format: insn_count | PC | opcode | result | changed_regs
    pub fn dump(&self) -> String {
        let mut out = String::with_capacity(self.len() * 120);
        for entry in self.iter_chronological() {
            out.push_str(&format_entry(entry));
            out.push('\n');
        }
        out
    }

    /// Write trace entries to a file.
    pub fn write_to_file(&self, path: &std::path::Path) -> std::io::Result<()> {
        use std::io::Write;
        let mut f = std::fs::File::create(path)?;
        for entry in self.iter_chronological() {
            writeln!(f, "{}", format_entry(entry))?;
        }
        Ok(())
    }
}

/// Return a human-readable name for a decoded Operation.
pub fn op_name(op: &Operation) -> &'static str {
    match op {
        Operation::Add { .. } => "ADD",
        Operation::Sub { .. } => "SUB",
        Operation::Sll { .. } => "SLL",
        Operation::Slt { .. } => "SLT",
        Operation::Sltu { .. } => "SLTU",
        Operation::Xor { .. } => "XOR",
        Operation::Srl { .. } => "SRL",
        Operation::Sra { .. } => "SRA",
        Operation::Or { .. } => "OR",
        Operation::And { .. } => "AND",
        Operation::Mul { .. } => "MUL",
        Operation::Mulh { .. } => "MULH",
        Operation::Mulhu { .. } => "MULHU",
        Operation::Mulhsu { .. } => "MULHSU",
        Operation::Div { .. } => "DIV",
        Operation::Divu { .. } => "DIVU",
        Operation::Rem { .. } => "REM",
        Operation::Remu { .. } => "REMU",
        Operation::LrW { .. } => "LR.W",
        Operation::ScW { .. } => "SC.W",
        Operation::AmoswapW { .. } => "AMOSWAP.W",
        Operation::AmoaddW { .. } => "AMOADD.W",
        Operation::AmoxorW { .. } => "AMOXOR.W",
        Operation::AmoandW { .. } => "AMOAND.W",
        Operation::AmoorW { .. } => "AMOOR.W",
        Operation::AmominW { .. } => "AMOMIN.W",
        Operation::AmomaxW { .. } => "AMOMAX.W",
        Operation::AmominuW { .. } => "AMOMINU.W",
        Operation::AmomaxuW { .. } => "AMOMAXU.W",
        Operation::Addi { .. } => "ADDI",
        Operation::Slti { .. } => "SLTI",
        Operation::Sltiu { .. } => "SLTIU",
        Operation::Xori { .. } => "XORI",
        Operation::Ori { .. } => "ORI",
        Operation::Andi { .. } => "ANDI",
        Operation::Slli { .. } => "SLLI",
        Operation::Srli { .. } => "SRLI",
        Operation::Srai { .. } => "SRAI",
        Operation::Lb { .. } => "LB",
        Operation::Lh { .. } => "LH",
        Operation::Lw { .. } => "LW",
        Operation::Lbu { .. } => "LBU",
        Operation::Lhu { .. } => "LHU",
        Operation::Sb { .. } => "SB",
        Operation::Sh { .. } => "SH",
        Operation::Sw { .. } => "SW",
        Operation::Beq { .. } => "BEQ",
        Operation::Bne { .. } => "BNE",
        Operation::Blt { .. } => "BLT",
        Operation::Bge { .. } => "BGE",
        Operation::Bltu { .. } => "BLTU",
        Operation::Bgeu { .. } => "BGEU",
        Operation::Lui { .. } => "LUI",
        Operation::Auipc { .. } => "AUIPC",
        Operation::Jal { .. } => "JAL",
        Operation::Jalr { .. } => "JALR",
        Operation::Ecall => "ECALL",
        Operation::Ebreak => "EBREAK",
        Operation::Fence => "FENCE",
        Operation::Mret => "MRET",
        Operation::Sret => "SRET",
        Operation::SfenceVma { .. } => "SFENCE.VMA",
        Operation::Csrrw { .. } => "CSRRW",
        Operation::Csrrs { .. } => "CSRRS",
        Operation::Csrrc { .. } => "CSRRC",
        Operation::Csrrwi { .. } => "CSRRWI",
        Operation::Csrrsi { .. } => "CSRRSI",
        Operation::Csrrci { .. } => "CSRRCI",
        Operation::Invalid(_) => "INVALID",
    }
}

/// Result shorthand for trace display.
fn result_name(r: &StepResult) -> &'static str {
    match r {
        StepResult::Ok => "OK",
        StepResult::Ecall => "ECALL",
        StepResult::Ebreak => "EBREAK",
        StepResult::FetchFault => "FETCH_FAULT",
        StepResult::LoadFault => "LOAD_FAULT",
        StepResult::StoreFault => "STORE_FAULT",
    }
}

/// Format a single trace entry as a human-readable line.
///
/// Format: `#insn_count | PC:0xXXXXXXXX | OPCODE | result | changed_regs`
/// Changed regs shown as `rN=0xNEW(0xOLD)` only for registers that changed.
pub fn format_entry(e: &TraceEntry) -> String {
    let mut changes = Vec::new();
    for i in 0..32 {
        if e.regs_before[i] != e.regs_after[i] {
            changes.push(format!(
                "x{}=0x{:08X}(was 0x{:08X})",
                i, e.regs_after[i], e.regs_before[i]
            ));
        }
    }
    let changes_str = if changes.is_empty() {
        "-".to_string()
    } else {
        changes.join(", ")
    };
    format!(
        "#{:<10} PC:0x{:08X} [{:<12}] 0x{:08X} -> PC:0x{:08X} {:14} | {}",
        e.insn_count,
        e.pc,
        op_name(&e.op),
        e.word,
        e.pc_after,
        result_name(&e.result),
        changes_str,
    )
}
