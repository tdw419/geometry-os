// RISC-V Multicore Shader (rv32im)
// Each workgroup is an independent RISC-V core with its own memory partition.
//
// Buffer layout (per tile):
//   TILE_SIZE = 4096 u32 words (16KB per tile)
//   Offset 0:       PC
//   Offset 1:       instruction count
//   Offset 2..33:   registers x0..x31
//   Offset 64..1023:   program text  (addresses 0x1000..0x1EFF)
//   Offset 1024..2047: rodata        (addresses 0x2000..0x2FFF)
//   Offset 2048..3071: RAM           (addresses 0x3000..0x3FFF)
//   Offset 3072..3073: UART cursor + output start
//   Offset 3074..4095: UART output buffer (up to 1022 chars)
//
// Dispatch: dispatch_workgroups(num_tiles, 1, 1)
// Each workgroup_id.x = tile index
//
// ISA bugs fixed (2026-03-24):
//   1. select() args were backwards for SLT/SLTI/SLTU/SLTIU
//   2. BLT/BGE now use signed comparison (XOR-flip trick)
//   3. SRAI implemented (funct7 check for arithmetic right shift)
//   4. LB/LH now sign-extend correctly
//   5. SB/SH now do byte/halfword writes (read-modify-write)
//   6. This is the single canonical shader (riscv-shader.wgsl is legacy)

const TILE_SIZE: u32 = 4096u;
const REG_OFFSET: u32 = 2u;
const TEXT_OFFSET: u32 = 64u;
const RODATA_OFFSET: u32 = 1024u;
const RAM_OFFSET: u32 = 2048u;
const UART_CURSOR_OFFSET: u32 = 3072u;
const UART_OUT_OFFSET: u32 = 3074u;
const UART_OUT_END: u32 = 4095u;
const MAILBOX_OFFSET: u32 = 4000u; // 32 words reserved for mailbox at end of tile (before UART)

// Address space constants (byte addresses used by RISC-V code)
const ADDR_TEXT: u32 = 0x1000u;
const ADDR_RODATA: u32 = 0x2000u;
const ADDR_RAM: u32 = 0x3000u;
const ADDR_UART: u32 = 0x4000u;

struct Config {
    num_tiles: u32,
    max_steps: u32,
    frame: u32,
    _pad: u32,
}

@group(0) @binding(0) var<uniform> config: Config;
@group(0) @binding(1) var<storage, read_write> tiles: array<u32>;

var<private> tile_base: u32;
var<private> regs: array<u32, 32>;
var<private> pc: u32;
var<private> next_pc: u32;

fn tile_read(offset: u32) -> u32 {
    return tiles[tile_base + offset];
}

fn tile_write(offset: u32, value: u32) {
    tiles[tile_base + offset] = value;
}

// ============================================================
// Signed comparison helper
// WGSL only has unsigned u32. To compare as signed i32:
//   flip the sign bit of both operands, then compare unsigned.
//   (a ^ 0x80000000) < (b ^ 0x80000000) iff a <_signed b
// ============================================================
const SIGN_FLIP: u32 = 0x80000000u;

fn signed_lt(a: u32, b: u32) -> bool {
    return (a ^ SIGN_FLIP) < (b ^ SIGN_FLIP);
}

fn signed_ge(a: u32, b: u32) -> bool {
    return (a ^ SIGN_FLIP) >= (b ^ SIGN_FLIP);
}

// ============================================================
// Sign extension helpers
// ============================================================
fn sign_extend_byte(val: u32) -> u32 {
    let b = val & 0xFFu;
    if ((b & 0x80u) != 0u) { return b | 0xFFFFFF00u; }
    return b;
}

fn sign_extend_half(val: u32) -> u32 {
    let h = val & 0xFFFFu;
    if ((h & 0x8000u) != 0u) { return h | 0xFFFF0000u; }
    return h;
}

// ============================================================
// Arithmetic right shift (sign-preserving)
// WGSL >> is logical (zero-fill). For arithmetic shift:
//   if sign bit set, fill upper bits with 1s.
// ============================================================
fn arith_shr(val: u32, shamt: u32) -> u32 {
    let s = shamt & 0x1Fu;
    if (s == 0u) { return val; }
    let shifted = val >> s;
    if ((val & SIGN_FLIP) != 0u) {
        // Fill top s bits with 1s
        let mask = 0xFFFFFFFFu << (32u - s);
        return shifted | mask;
    }
    return shifted;
}

// ============================================================
// Byte-addressable memory
// ============================================================

// Map byte address to tile-local word offset
fn addr_to_offset(addr: u32) -> u32 {
    if (addr >= ADDR_UART) { return 0xFFFFFFFFu; }
    if (addr >= ADDR_RAM) { return RAM_OFFSET + (addr - ADDR_RAM) / 4u; }
    if (addr >= ADDR_RODATA) { return RODATA_OFFSET + (addr - ADDR_RODATA) / 4u; }
    if (addr >= ADDR_TEXT) { return TEXT_OFFSET + (addr - ADDR_TEXT) / 4u; }
    return 0xFFFFFFFFu;
}

// Read aligned word containing byte address
fn mem_read_word(addr: u32) -> u32 {
    let aligned = addr & 0xFFFFFFFCu;
    let off = addr_to_offset(aligned);
    if (off == 0xFFFFFFFFu) { return 0u; }
    return tile_read(off);
}

// Read byte from byte address
fn mem_read_byte(addr: u32) -> u32 {
    let word = mem_read_word(addr);
    let byte_off = addr & 3u;
    return (word >> (byte_off * 8u)) & 0xFFu;
}

// Read halfword from byte address
fn mem_read_half(addr: u32) -> u32 {
    let word = mem_read_word(addr);
    let byte_off = addr & 2u;
    return (word >> (byte_off * 8u)) & 0xFFFFu;
}

// Read full word (for LW — uses aligned address)
fn mem_read(addr: u32) -> u32 {
    return mem_read_word(addr);
}

// Write byte to byte address (read-modify-write)
fn mem_write_byte(addr: u32, value: u32) {
    // UART MMIO
    if (addr >= ADDR_UART && addr < ADDR_UART + 0x10u) {
        let cursor = tile_read(UART_CURSOR_OFFSET);
        let out_pos = UART_OUT_OFFSET + cursor;
        if (out_pos <= UART_OUT_END) {
            tile_write(out_pos, value & 0xFFu);
            tile_write(UART_CURSOR_OFFSET, cursor + 1u);
        }
        return;
    }
    if (addr < ADDR_RAM || addr >= ADDR_UART) { return; }
    let aligned = addr & 0xFFFFFFFCu;
    let off = addr_to_offset(aligned);
    if (off == 0xFFFFFFFFu) { return; }
    let byte_off = addr & 3u;
    let shift = byte_off * 8u;
    let mask = ~(0xFFu << shift);
    let old = tile_read(off);
    tile_write(off, (old & mask) | ((value & 0xFFu) << shift));
}

// Write halfword to byte address (read-modify-write)
fn mem_write_half(addr: u32, value: u32) {
    if (addr >= ADDR_UART && addr < ADDR_UART + 0x10u) {
        let cursor = tile_read(UART_CURSOR_OFFSET);
        let out_pos = UART_OUT_OFFSET + cursor;
        if (out_pos <= UART_OUT_END) {
            tile_write(out_pos, value & 0xFFu);
            tile_write(UART_CURSOR_OFFSET, cursor + 1u);
        }
        return;
    }
    if (addr < ADDR_RAM || addr >= ADDR_UART) { return; }
    let aligned = addr & 0xFFFFFFFCu;
    let off = addr_to_offset(aligned);
    if (off == 0xFFFFFFFFu) { return; }
    let byte_off = addr & 2u;
    let shift = byte_off * 8u;
    let mask = ~(0xFFFFu << shift);
    let old = tile_read(off);
    tile_write(off, (old & mask) | ((value & 0xFFFFu) << shift));
}

// Write full word
fn mem_write_word(addr: u32, value: u32) {
    if (addr >= ADDR_UART && addr < ADDR_UART + 0x10u) {
        let cursor = tile_read(UART_CURSOR_OFFSET);
        let out_pos = UART_OUT_OFFSET + cursor;
        if (out_pos <= UART_OUT_END) {
            tile_write(out_pos, value & 0xFFu);
            tile_write(UART_CURSOR_OFFSET, cursor + 1u);
        }
        return;
    }
    if (addr < ADDR_RAM || addr >= ADDR_UART) { return; }
    let off = addr_to_offset(addr);
    if (off != 0xFFFFFFFFu) {
        tile_write(off, value);
    }
}

fn load_regs() {
    pc = tile_read(0u);
    for (var i = 1u; i < 32u; i++) {
        regs[i] = tile_read(REG_OFFSET + i);
    }
    regs[0u] = 0u;
}

fn save_regs_with_steps(steps: u32) {
    tile_write(0u, next_pc);
    let count = tile_read(1u);
    tile_write(1u, count + steps);
    for (var i = 1u; i < 32u; i++) {
        tile_write(REG_OFFSET + i, regs[i]);
    }
}

// Instruction decode helpers
fn get_opcode(insn: u32) -> u32 { return insn & 0x7Fu; }
fn get_rd(insn: u32) -> u32 { return (insn >> 7u) & 0x1Fu; }
fn get_funct3(insn: u32) -> u32 { return (insn >> 12u) & 0x7u; }
fn get_rs1(insn: u32) -> u32 { return (insn >> 15u) & 0x1Fu; }
fn get_rs2(insn: u32) -> u32 { return (insn >> 20u) & 0x1Fu; }
fn get_funct7(insn: u32) -> u32 { return (insn >> 25u) & 0x7Fu; }

fn imm_i(insn: u32) -> u32 {
    var imm = (insn >> 20u) & 0xFFFu;
    if ((imm & 0x800u) != 0u) { imm = imm | 0xFFFFF000u; }
    return imm;
}

fn imm_s(insn: u32) -> u32 {
    var imm = ((insn >> 7u) & 0x1Fu) | (((insn >> 25u) & 0x7Fu) << 5u);
    if ((imm & 0x800u) != 0u) { imm = imm | 0xFFFFF000u; }
    return imm;
}

fn imm_b(insn: u32) -> u32 {
    var imm = (((insn >> 8u) & 0xFu) << 1u)
            | (((insn >> 25u) & 0x3Fu) << 5u)
            | (((insn >> 7u) & 0x1u) << 11u)
            | (((insn >> 31u) & 0x1u) << 12u);
    if ((imm & 0x1000u) != 0u) { imm = imm | 0xFFFFE000u; }
    return imm;
}

fn imm_u(insn: u32) -> u32 { return insn & 0xFFFFF000u; }

fn imm_j(insn: u32) -> u32 {
    var imm = (((insn >> 21u) & 0x3FFu) << 1u)
            | (((insn >> 20u) & 0x1u) << 11u)
            | (((insn >> 12u) & 0xFFu) << 12u)
            | (((insn >> 31u) & 0x1u) << 20u);
    if ((imm & 0x100000u) != 0u) { imm = imm | 0xFFF00000u; }
    return imm;
}

// Returns true if halt
fn execute_insn(insn: u32) -> bool {
    if (insn == 0u) { return false; }

    let opcode = get_opcode(insn);
    let rd = get_rd(insn);
    let rs1 = get_rs1(insn);
    let rs2 = get_rs2(insn);
    let funct3 = get_funct3(insn);
    let funct7 = get_funct7(insn);

    next_pc = pc + 4u;

    switch (opcode) {
        case 0x37u: { // LUI
            if (rd != 0u) { regs[rd] = imm_u(insn); }
        }
        case 0x17u: { // AUIPC
            if (rd != 0u) { regs[rd] = pc + imm_u(insn); }
        }
        case 0x6Fu: { // JAL
            if (rd != 0u) { regs[rd] = pc + 4u; }
            next_pc = pc + imm_j(insn);
        }
        case 0x67u: { // JALR
            if (rd != 0u) { regs[rd] = pc + 4u; }
            next_pc = (regs[rs1] + imm_i(insn)) & 0xFFFFFFFEu;
        }

        // BRANCH — signed comparison for BLT/BGE
        case 0x63u: {
            let a = regs[rs1];
            let b = regs[rs2];
            var take = false;
            switch (funct3) {
                case 0x0u: { take = a == b; }          // BEQ
                case 0x1u: { take = a != b; }          // BNE
                case 0x4u: { take = signed_lt(a, b); } // BLT  (signed)
                case 0x5u: { take = signed_ge(a, b); } // BGE  (signed)
                case 0x6u: { take = a < b; }           // BLTU (unsigned)
                case 0x7u: { take = a >= b; }          // BGEU (unsigned)
                default: {}
            }
            if (take) { next_pc = pc + imm_b(insn); }
        }

        // LOAD — byte-addressable, sign-extending
        case 0x03u: {
            let addr = regs[rs1] + imm_i(insn);
            if (rd != 0u) {
                switch (funct3) {
                    case 0x0u: { regs[rd] = sign_extend_byte(mem_read_byte(addr)); } // LB
                    case 0x1u: { regs[rd] = sign_extend_half(mem_read_half(addr)); } // LH
                    case 0x2u: { regs[rd] = mem_read(addr); }                        // LW
                    case 0x4u: { regs[rd] = mem_read_byte(addr); }                   // LBU
                    case 0x5u: { regs[rd] = mem_read_half(addr); }                   // LHU
                    default: {}
                }
            }
        }

        // STORE — byte/halfword/word
        case 0x23u: {
            let addr = regs[rs1] + imm_s(insn);
            let val = regs[rs2];
            switch (funct3) {
                case 0x0u: { mem_write_byte(addr, val); } // SB
                case 0x1u: { mem_write_half(addr, val); } // SH
                case 0x2u: { mem_write_word(addr, val); } // SW
                default: {}
            }
        }

        // OP-IMM — correct select() order, SRAI implemented
        case 0x13u: {
            let a = regs[rs1];
            let b = imm_i(insn);
            var result = 0u;
            switch (funct3) {
                case 0x0u: { result = a + b; }                                    // ADDI
                case 0x2u: { result = select(0u, 1u, signed_lt(a, b)); }          // SLTI
                case 0x3u: { result = select(0u, 1u, a < b); }                    // SLTIU
                case 0x4u: { result = a ^ b; }                                    // XORI
                case 0x6u: { result = a | b; }                                    // ORI
                case 0x7u: { result = a & b; }                                    // ANDI
                case 0x1u: { result = a << (b & 0x1Fu); }                         // SLLI
                case 0x5u: {
                    let shamt = b & 0x1Fu;
                    if ((insn & 0x40000000u) != 0u) {
                        result = arith_shr(a, shamt);                              // SRAI
                    } else {
                        result = a >> shamt;                                       // SRLI
                    }
                }
                default: {}
            }
            if (rd != 0u) { regs[rd] = result; }
        }

        // OP — base integer + M extension
        case 0x33u: {
            let a = regs[rs1];
            let b = regs[rs2];
            var result = 0u;

            if (funct7 == 0x01u) {
                // M extension
                switch (funct3) {
                    case 0x0u: { result = a * b; }                                     // MUL
                    case 0x1u: { result = 0u; }                                        // MULH (TODO: needs 64-bit)
                    case 0x2u: { result = 0u; }                                        // MULHSU (TODO)
                    case 0x3u: {                                                       // MULHU
                        let a_lo = a & 0xFFFFu; let a_hi = a >> 16u;
                        let b_lo = b & 0xFFFFu; let b_hi = b >> 16u;
                        let mid = a_hi * b_lo + a_lo * b_hi;
                        result = a_hi * b_hi + (mid >> 16u);
                    }
                    case 0x4u: {                                                       // DIV (signed)
                        if (b == 0u) { result = 0xFFFFFFFFu; }
                        else if (a == 0x80000000u && b == 0xFFFFFFFFu) { result = 0x80000000u; }
                        else {
                            var abs_a = a; var abs_b = b; var neg = false;
                            if ((a & SIGN_FLIP) != 0u) { abs_a = 0u - a; neg = !neg; }
                            if ((b & SIGN_FLIP) != 0u) { abs_b = 0u - b; neg = !neg; }
                            var q = abs_a / abs_b;
                            if (neg) { q = 0u - q; }
                            result = q;
                        }
                    }
                    case 0x5u: {                                                       // DIVU
                        if (b == 0u) { result = 0xFFFFFFFFu; }
                        else { result = a / b; }
                    }
                    case 0x6u: {                                                       // REM (signed)
                        if (b == 0u) { result = a; }
                        else if (a == 0x80000000u && b == 0xFFFFFFFFu) { result = 0u; }
                        else {
                            var abs_a = a; var abs_b = b; var neg = false;
                            if ((a & SIGN_FLIP) != 0u) { abs_a = 0u - a; neg = true; }
                            if ((b & SIGN_FLIP) != 0u) { abs_b = 0u - b; }
                            var r = abs_a % abs_b;
                            if (neg) { r = 0u - r; }
                            result = r;
                        }
                    }
                    case 0x7u: {                                                       // REMU
                        if (b == 0u) { result = a; }
                        else { result = a % b; }
                    }
                    default: {}
                }
            } else {
                // Base integer
                switch (funct3) {
                    case 0x0u: {
                        if (funct7 == 0x20u) { result = a - b; }
                        else { result = a + b; }
                    }
                    case 0x1u: { result = a << (b & 0x1Fu); }                          // SLL
                    case 0x2u: { result = select(0u, 1u, signed_lt(a, b)); }           // SLT
                    case 0x3u: { result = select(0u, 1u, a < b); }                     // SLTU
                    case 0x4u: { result = a ^ b; }                                     // XOR
                    case 0x5u: {
                        if (funct7 == 0x20u) { result = arith_shr(a, b & 0x1Fu); }    // SRA
                        else { result = a >> (b & 0x1Fu); }                            // SRL
                    }
                    case 0x6u: { result = a | b; }                                     // OR
                    case 0x7u: { result = a & b; }                                     // AND
                    default: {}
                }
            }
            if (rd != 0u) { regs[rd] = result; }
        }

        case 0x0Fu: {} // FENCE (nop)
        case 0x73u: { return true; } // ECALL = halt
        
        // Custom Phase 42: Inter-Tile Communication
        case 0x60u: { // SEND rs2 to tile(rs1).mailbox[rd]
            let target_tile = regs[rs1];
            let slot = rd; // Use rd field as slot index (0-31)
            let val = regs[rs2];
            if (target_tile < config.num_tiles && slot < 32u) {
                tiles[target_tile * TILE_SIZE + MAILBOX_OFFSET + slot] = val;
            }
        }
        case 0x61u: { // RECV from tile(rs1).mailbox[rs2] into rd
            let target_tile = regs[rs1];
            let slot = rs2; // Use rs2 field as slot index
            if (rd != 0u) {
                if (target_tile < config.num_tiles && slot < 32u) {
                    regs[rd] = tiles[target_tile * TILE_SIZE + MAILBOX_OFFSET + slot]; tiles[tile_base + 33u] = regs[rd];
                } else {
                    regs[rd] = 0u;
                }
            }
        }
default: {}
    }

    return false;
}

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let tile_id = global_id.x;
    if (tile_id >= config.num_tiles) { return; }

    tile_base = tile_id * TILE_SIZE;

    let current_pc = tile_read(0u);
    if (current_pc == 0xFFFFFFFFu) { return; }

    load_regs();

    var steps_done = 0u;
    for (var step = 0u; step < config.max_steps; step++) {
        let insn = mem_read(pc);
        let halted = execute_insn(insn);
        steps_done = step + 1u;
        if (halted) {
            tile_write(0u, 0xFFFFFFFFu);
            let count = tile_read(1u);
            tile_write(1u, count + steps_done);
            return;
        }
        pc = next_pc;
    }

    next_pc = pc;
    save_regs_with_steps(steps_done);
}
