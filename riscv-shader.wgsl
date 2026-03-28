// DEPRECATED: Use riscv-multicore.wgsl instead.
// This file has known ISA bugs (signed comparison, SB/SH, SRAI, select() order).
// Kept for backwards compatibility with riscv.rs single-core host.
//
// RISC-V CPU in the Framebuffer
// The GPU executes RISC-V instructions stored as pixels.
//
// Memory map:
//   0x00001000 - 0x00001FFF: Program text (instructions)
//   0x00002000 - 0x00002FFF: Read-only data
//   0x00003000 - 0x00003FFF: RAM (data/heap/stack)
//   0x00004000 - 0x0000400F: MMIO - UART (serial output)
//
// Pixel layout (at 640x480):
//   Row 0: CPU state
//     (0,0): PC (low 16 bits | high 16 bits << 16)
//     (1,0): instruction count
//     (2..34, 0): registers x0..x31 (x0 is always 0, not stored)
//   Row 4+: Memory (address = (row-4) * WIDTH * 4 + col * 4 + 0x1000)

struct Config {
    width: u32,
    height: u32,
    frame: u32,
    _pad: u32,
}

@group(0) @binding(0) var<uniform> config: Config;
@group(0) @binding(1) var<storage, read> buf_in: array<u32>;
@group(0) @binding(2) var<storage, read_write> buf_out: array<u32>;

fn px_idx(x: u32, y: u32) -> u32 {
    return y * config.width + x;
}

// Convert byte address to pixel coordinates in memory region (row 4+)
fn addr_to_pixel(addr: u32) -> vec2<u32> {
    let pixel_offset = (addr - 0x1000u) / 4u;
    let x = pixel_offset % config.width;
    let y = (pixel_offset / config.width) + 4u;
    return vec2<u32>(x, y);
}

fn mem_read(addr: u32) -> u32 {
    if (addr < 0x1000u) { return 0u; }
    let pos = addr_to_pixel(addr);
    if (pos.y >= config.height) { return 0u; }
    return buf_in[px_idx(pos.x, pos.y)];
}

fn mem_write(addr: u32, value: u32) {
    if (addr < 0x3000u) { return; }  // Protect text/rodata
    if (addr >= 0x4000u && addr < 0x4010u) {
        // UART MMIO - write to output region
        uart_output(value & 0xFFu);
        return;
    }
    let pos = addr_to_pixel(addr);
    if (pos.y >= config.height) { return; }
    buf_out[px_idx(pos.x, pos.y)] = value;
}

// UART output: write byte to last row
var<private> uart_cursor: u32 = 0u;

fn uart_output(byte: u32) {
    let out_y = config.height - 1u;
    let max_x = config.width - 1u;
    let cursor = uart_cursor;
    if (cursor < max_x) {
        buf_out[px_idx(cursor, out_y)] = 0xFF000000u | byte;
        uart_cursor = cursor + 1u;
    }
}

// CPU state (private)
var<private> regs: array<u32, 32>;
var<private> pc: u32;
var<private> next_pc: u32;

fn load_state() {
    let cpu0 = buf_in[px_idx(0u, 0u)];
    pc = cpu0;
    
    // Load registers x1..x31 from pixels 3..33
    for (var i = 1u; i < 32u; i++) {
        regs[i] = buf_in[px_idx(2u + i, 0u)];
    }
    regs[0u] = 0u;  // x0 is always 0
}

fn save_state() {
    buf_out[px_idx(0u, 0u)] = next_pc;
    buf_out[px_idx(1u, 0u)] = buf_in[px_idx(1u, 0u)] + 1u;  // Increment instruction count
    for (var i = 1u; i < 32u; i++) {
        buf_out[px_idx(2u + i, 0u)] = regs[i];
    }
}

// RISC-V instruction decode
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
    var imm = (((insn >> 7u) & 0x1Fu) | (((insn >> 25u) & 0x7Fu) << 5u));
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

fn imm_u(insn: u32) -> u32 {
    return insn & 0xFFFFF000u;
}

fn imm_j(insn: u32) -> u32 {
    var imm = (((insn >> 21u) & 0x3FFu) << 1u)
            | (((insn >> 20u) & 0x1u) << 11u)
            | (((insn >> 12u) & 0xFFu) << 12u)
            | (((insn >> 31u) & 0x1u) << 20u);
    if ((imm & 0x100000u) != 0u) { imm = imm | 0xFFF00000u; }
    return imm;
}

// Execute one instruction. Returns true if halt.
fn execute_insn(insn: u32) -> bool {
    if (insn == 0u) { return false; }  // NOP
    
    let opcode = get_opcode(insn);
    let rd = get_rd(insn);
    let rs1 = get_rs1(insn);
    let rs2 = get_rs2(insn);
    let funct3 = get_funct3(insn);
    let funct7 = get_funct7(insn);
    
    next_pc = pc + 4u;
    
    switch (opcode) {
        // LUI
        case 0x37u: {
            if (rd != 0u) { regs[rd] = imm_u(insn); }
        }
        // AUIPC
        case 0x17u: {
            if (rd != 0u) { regs[rd] = pc + imm_u(insn); }
        }
        // JAL
        case 0x6Fu: {
            if (rd != 0u) { regs[rd] = pc + 4u; }
            next_pc = pc + imm_j(insn);
        }
        // JALR
        case 0x67u: {
            if (rd != 0u) { regs[rd] = pc + 4u; }
            next_pc = (regs[rs1] + imm_i(insn)) & 0xFFFFFFFEu;
        }
        // BRANCH
        case 0x63u: {
            let a = regs[rs1];
            let b = regs[rs2];
            var take = false;
            switch (funct3) {
                case 0x0u: { take = a == b; }      // BEQ
                case 0x1u: { take = a != b; }      // BNE
                case 0x4u: { take = a < b; }       // BLT (treat as unsigned for now)
                case 0x5u: { take = a >= b; }      // BGE
                case 0x6u: { take = a < b; }       // BLTU
                case 0x7u: { take = a >= b; }      // BGEU
                default: {}
            }
            if (take) { next_pc = pc + imm_b(insn); }
        }
        // LOAD
        case 0x03u: {
            let addr = regs[rs1] + imm_i(insn);
            let val = mem_read(addr);
            if (rd != 0u) {
                switch (funct3) {
                    case 0x0u: { regs[rd] = val & 0xFFu; }        // LB
                    case 0x1u: { regs[rd] = val & 0xFFFFu; }      // LH
                    case 0x2u: { regs[rd] = val; }                // LW
                    case 0x4u: { regs[rd] = val & 0xFFu; }        // LBU
                    case 0x5u: { regs[rd] = val & 0xFFFFu; }      // LHU
                    default: {}
                }
            }
        }
        // STORE
        case 0x23u: {
            let addr = regs[rs1] + imm_s(insn);
            mem_write(addr, regs[rs2]);
        }
        // OP-IMM (ALU immediate)
        case 0x13u: {
            let a = regs[rs1];
            let b = imm_i(insn);
            var result = 0u;
            switch (funct3) {
                case 0x0u: { result = a + b; }             // ADDI
                case 0x2u: { result = select(1u, 0u, a < b); } // SLTI
                case 0x3u: { result = select(1u, 0u, a < b); } // SLTIU
                case 0x4u: { result = a ^ b; }             // XORI
                case 0x6u: { result = a | b; }             // ORI
                case 0x7u: { result = a & b; }             // ANDI
                case 0x1u: { result = a << (b & 0x1Fu); }  // SLLI
                case 0x5u: { result = a >> (b & 0x1Fu); }  // SRLI/SRAI
                default: {}
            }
            if (rd != 0u) { regs[rd] = result; }
        }
        // OP (ALU register-register)
        case 0x33u: {
            let a = regs[rs1];
            let b = regs[rs2];
            var result = 0u;
            switch (funct3) {
                case 0x0u: {
                    if (funct7 == 0x00u) { result = a + b; }
                    else if (funct7 == 0x20u) { result = a - b; }
                }
                case 0x1u: { result = a << (b & 0x1Fu); }  // SLL
                case 0x2u: { result = select(1u, 0u, a < b); } // SLT
                case 0x3u: { result = select(1u, 0u, a < b); } // SLTU
                case 0x4u: { result = a ^ b; }              // XOR
                case 0x5u: { result = a >> (b & 0x1Fu); }   // SRL/SRA
                case 0x6u: { result = a | b; }              // OR
                case 0x7u: { result = a & b; }              // AND
                default: {}
            }
            if (rd != 0u) { regs[rd] = result; }
        }
        // FENCE (NOP)
        case 0x0Fu: {}
        // SYSTEM (ECALL = halt)
        case 0x73u: {
            return true;  // Halt
        }
        default: {}
    }
    
    return false;
}

@compute @workgroup_size(1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let x = gid.x;
    let y = gid.y;
    
    // Only pixel (0,0) executes - the CPU
    if (x != 0u || y != 0u) {
        // All other pixels: copy through
        let idx = px_idx(x, y);
        buf_out[idx] = buf_in[idx];
        return;
    }
    
    // === CPU PIXEL ===
    
    // First: copy row 0-3 to preserve state and memory
    for (var i = 0u; i < config.width; i = i + 1u) {
        buf_out[px_idx(i, 0u)] = buf_in[px_idx(i, 0u)];
        buf_out[px_idx(i, 1u)] = buf_in[px_idx(i, 1u)];
        buf_out[px_idx(i, 2u)] = buf_in[px_idx(i, 2u)];
        buf_out[px_idx(i, 3u)] = buf_in[px_idx(i, 3u)];
    }
    
    // Load CPU state
    pc = buf_in[px_idx(0u, 0u)];
    
    // Check for halt
    if (pc == 0xFFFFFFFFu) {
        return;  // Already halted
    }
    
    // Load registers
    load_state();
    
    // Fetch instruction
    let insn = mem_read(pc);
    
    // Execute
    let halted = execute_insn(insn);
    
    // Save state
    if (halted) {
        buf_out[px_idx(0u, 0u)] = 0xFFFFFFFFu;
    } else {
        save_state();
    }
    
    // Increment instruction count
    let count = buf_in[px_idx(1u, 0u)];
    buf_out[px_idx(1u, 0u)] = count + 1u;
}
