// RISC-V Multi-Core Processor on the Infinite Map
//
// Each workgroup (tile) is an independent RISC-V core.
// Each tile has its own 4KB memory space (1024 words).

struct Config {
    grid_width: u32,  // Number of tiles horizontally
    grid_height: u32, // Number of tiles vertically
    frame: u32,
    _pad: u32,
}

@group(0) @binding(0) var<uniform> config: Config;
@group(0) @binding(1) var<storage, read> buf_in: array<u32>;
@group(0) @binding(2) var<storage, read_write> buf_out: array<u32>;

const TILE_SIZE: u32 = 1024u; // 4KB / 4

fn get_tile_offset(gid: vec3<u32>) -> u32 {
    return (gid.y * config.grid_width + gid.x) * TILE_SIZE;
}

fn mem_read(tile_offset: u32, addr: u32) -> u32 {
    let word_offset = addr / 4u;
    if (word_offset >= TILE_SIZE) { return 0u; }
    return buf_in[tile_offset + word_offset];
}

fn mem_write(tile_offset: u32, addr: u32, value: u32) {
    let word_offset = addr / 4u;
    if (word_offset >= TILE_SIZE) { return; }
    // MMIO UART: write to the very last word of the tile
    if (addr == 0x4000u) {
        buf_out[tile_offset + TILE_SIZE - 1u] = 0xFF000000u | (value & 0xFFu);
        return;
    }
    buf_out[tile_offset + word_offset] = value;
}

// CPU state (private to this invocation)
var<private> regs: array<u32, 32>;
var<private> pc: u32;
var<private> next_pc: u32;

fn load_state(tile_offset: u32) {
    pc = buf_in[tile_offset];
    for (var i = 1u; i < 32u; i = i + 1u) {
        regs[i] = buf_in[tile_offset + 2u + i];
    }
    regs[0] = 0u;
}

fn save_state(tile_offset: u32) {
    buf_out[tile_offset] = next_pc;
    buf_out[tile_offset + 1u] = buf_in[tile_offset + 1u] + 1u; // Tick
    for (var i = 1u; i < 32u; i = i + 1u) {
        buf_out[tile_offset + 2u + i] = regs[i];
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

fn execute_insn(tile_offset: u32, insn: u32) -> bool {
    if (insn == 0u) { return false; }
    
    let opcode = get_opcode(insn);
    let rd = get_rd(insn);
    let rs1 = get_rs1(insn);
    let rs2 = get_rs2(insn);
    let funct3 = get_funct3(insn);
    let funct7 = get_funct7(insn);
    
    next_pc = pc + 4u;
    
    switch (opcode) {
        case 0x37u: { if (rd != 0u) { regs[rd] = imm_u(insn); } }
        case 0x17u: { if (rd != 0u) { regs[rd] = pc + imm_u(insn); } }
        case 0x6Fu: { if (rd != 0u) { regs[rd] = pc + 4u; } next_pc = pc + imm_j(insn); }
        case 0x67u: { if (rd != 0u) { regs[rd] = pc + 4u; } next_pc = (regs[rs1] + imm_i(insn)) & 0xFFFFFFFEu; }
        case 0x63u: {
            let a = regs[rs1]; let b = regs[rs2]; var take = false;
            switch (funct3) {
                case 0x0u: { take = a == b; }
                case 0x1u: { take = a != b; }
                case 0x4u: { take = a < b; }
                case 0x5u: { take = a >= b; }
                case 0x6u: { take = a < b; }
                case 0x7u: { take = a >= b; }
                default: {}
            }
            if (take) { next_pc = pc + imm_b(insn); }
        }
        case 0x03u: {
            let addr = regs[rs1] + imm_i(insn);
            let val = mem_read(tile_offset, addr);
            if (rd != 0u) {
                switch (funct3) {
                    case 0x0u: { regs[rd] = val & 0xFFu; }
                    case 0x1u: { regs[rd] = val & 0xFFFFu; }
                    case 0x2u: { regs[rd] = val; }
                    case 0x4u: { regs[rd] = val & 0xFFu; }
                    case 0x5u: { regs[rd] = val & 0xFFFFu; }
                    default: {}
                }
            }
        }
        case 0x23u: { let addr = regs[rs1] + imm_s(insn); mem_write(tile_offset, addr, regs[rs2]); }
        case 0x13u: {
            let a = regs[rs1]; let b = imm_i(insn); var result = 0u;
            switch (funct3) {
                case 0x0u: { result = a + b; }
                case 0x2u: { result = select(1u, 0u, a < b); }
                case 0x3u: { result = select(1u, 0u, a < b); }
                case 0x4u: { result = a ^ b; }
                case 0x6u: { result = a | b; }
                case 0x7u: { result = a & b; }
                case 0x1u: { result = a << (b & 0x1Fu); }
                case 0x5u: { result = a >> (b & 0x1Fu); }
                default: {}
            }
            if (rd != 0u) { regs[rd] = result; }
        }
        case 0x33u: {
            let a = regs[rs1]; let b = regs[rs2]; var result = 0u;
            switch (funct3) {
                case 0x0u: { if (funct7 == 0x00u) { result = a + b; } else if (funct7 == 0x20u) { result = a - b; } }
                case 0x1u: { result = a << (b & 0x1Fu); }
                case 0x2u: { result = select(1u, 0u, a < b); }
                case 0x3u: { result = select(1u, 0u, a < b); }
                case 0x4u: { result = a ^ b; }
                case 0x5u: { result = a >> (b & 0x1Fu); }
                case 0x6u: { result = a | b; }
                case 0x7u: { result = a & b; }
                default: {}
            }
            if (rd != 0u) { regs[rd] = result; }
        }
        case 0x0Fu: {}
        case 0x73u: { return true; }
        default: {}
    }
    return false;
}

@compute @workgroup_size(1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (gid.x >= config.grid_width || gid.y >= config.grid_height) { return; }

    let tile_offset = get_tile_offset(gid);
    
    // Copy the entire tile state to output initially (pass-through)
    for (var i = 0u; i < TILE_SIZE; i = i + 1u) {
        buf_out[tile_offset + i] = buf_in[tile_offset + i];
    }

    pc = buf_in[tile_offset];
    if (pc == 0xFFFFFFFFu) { return; }

    load_state(tile_offset);
    let insn = mem_read(tile_offset, pc);
    let halted = execute_insn(tile_offset, insn);

    if (halted) {
        buf_out[tile_offset] = 0xFFFFFFFFu;
    } else {
        save_state(tile_offset);
    }
}
