
// ============================================
// RISC-V Multi-Tile Executor - GPU Compute Shader
// ============================================
// Each workgroup executes one RISC-V tile (hart).
// Up to 256 tiles run in parallel.
//
// Tile state layout (per-tile storage buffer):
//   [0..31]   = 32 x 32-bit registers (x0-x31)
//   [32]      = PC (program counter)
//   [33]      = STATUS (bit 0=running, bit 1=halted, bit 2=error)
//   [34]      = INSTRUCTION_COUNT
//   [35]      = MAX_STEPS
//   [36]      = TILE_ID
//   [37..164] = UART output buffer (128 words = 512 bytes)
//   [164..]   = RAM (remaining space)
//
// Code/text is loaded into the RAM region.

struct TileState {
    regs: array<u32, 32>,      // 0-31: x0-x31
    pc: u32,                     // 32: program counter
    status: u32,                 // 33: status flags
    instruction_count: u32,      // 34: instructions executed
    max_steps: u32,              // 35: step limit
    tile_id: u32,                // 36: unique tile ID
    uart_len: u32,               // 37: UART output length
    _reserved: array<u32, 2>,    // 38-39: padding
    uart_buf: array<u32, 128>,   // 40-167: UART output buffer
    ram: array<u32, 992>,        // 168-1159: general purpose RAM
};

@group(0) @binding(0) var<storage, read_write> tiles: array<TileState>;

// UART MMIO address
const UART_BASE: u32 = 0x10000000u;

// RISC-V opcodes
const OP_LUI: u32 = 0x37u;
const OP_AUIPC: u32 = 0x17u;
const OP_JAL: u32 = 0x6Fu;
const OP_JALR: u32 = 0x67u;
const OP_BRANCH: u32 = 0x63u;
const OP_LOAD: u32 = 0x03u;
const OP_STORE: u32 = 0x23u;
const OP_OP_IMM: u32 = 0x13u;
const OP_OP: u32 = 0x33u;
const OP_SYSTEM: u32 = 0x73u;

// Read register (x0 always 0)
fn read_reg(tile_idx: u32, reg: u32) -> u32 {
    if reg == 0u { return 0u; }
    return tiles[tile_idx].regs[reg];
}

// Write register (x0 is read-only)
fn write_reg(tile_idx: u32, reg: u32, val: u32) {
    if reg == 0u { return; }
    tiles[tile_idx].regs[reg] = val;
}

// Read from tile RAM
fn read_ram(tile_idx: u32, addr: u32) -> u32 {
    let word_addr = addr / 4u;
    if word_addr < 992u {
        return tiles[tile_idx].ram[word_addr];
    }
    return 0u;
}

// Write to tile RAM (with UART MMIO intercept)
fn write_ram(tile_idx: u32, addr: u32, val: u32) {
    // UART MMIO: write byte to UART buffer
    if addr >= UART_BASE && addr < UART_BASE + 512u {
        let uart_idx = tiles[tile_idx].uart_len;
        if uart_idx < 128u {
            tiles[tile_idx].uart_buf[uart_idx] = val & 0xFFu;
            tiles[tile_idx].uart_len = uart_idx + 1u;
        }
        return;
    }
    let word_addr = addr / 4u;
    if word_addr < 992u {
        tiles[tile_idx].ram[word_addr] = val;
    }
}

// Sign-extend a value from bit N
fn sign_extend(val: u32, bits: u32) -> u32 {
    let sign_bit = 1u << (bits - 1u);
    if (val & sign_bit) != 0u {
        return val | (0xFFFFFFFFu << bits);
    }
    return val;
}

// Execute one instruction for a tile, return true if still running
fn execute_one(tile_idx: u32) -> bool {
    let pc = tiles[tile_idx].pc;
    let inst = read_ram(tile_idx, pc);
    let opcode = inst & 0x7Fu;
    let rd = (inst >> 7u) & 0x1Fu;
    let funct3 = (inst >> 12u) & 0x7u;
    let rs1 = (inst >> 15u) & 0x1Fu;
    let rs2 = (inst >> 20u) & 0x1Fu;
    let funct7 = (inst >> 25u) & 0x7Fu;
    
    // I-type immediate
    let imm_i = sign_extend((inst >> 20u) & 0xFFFu, 12u);
    // S-type immediate
    let imm_s = sign_extend(((inst >> 7u) & 0x1Fu) | (((inst >> 25u) & 0x7Fu) << 5u), 12u);
    // B-type immediate
    let imm_b_raw = (((inst >> 8u) & 0xFu) << 1u) |
                    (((inst >> 25u) & 0x3Fu) << 5u) |
                    (((inst >> 7u) & 0x1u) << 11u) |
                    (((inst >> 31u) & 0x1u) << 12u);
    let imm_b = sign_extend(imm_b_raw, 13u);
    // U-type immediate
    let imm_u = inst & 0xFFFFF000u;
    // J-type immediate
    let imm_j_raw = (((inst >> 21u) & 0x3FFu) << 1u) |
                    (((inst >> 20u) & 0x1u) << 11u) |
                    (((inst >> 12u) & 0xFFu) << 12u) |
                    (((inst >> 31u) & 0x1u) << 20u);
    let imm_j = sign_extend(imm_j_raw, 21u);
    
    let next_pc = pc + 4u;
    
    switch opcode {
        case OP_LUI: {
            write_reg(tile_idx, rd, imm_u);
            tiles[tile_idx].pc = next_pc;
        }
        case OP_AUIPC: {
            write_reg(tile_idx, rd, pc + imm_u);
            tiles[tile_idx].pc = next_pc;
        }
        case OP_JAL: {
            write_reg(tile_idx, rd, next_pc);
            tiles[tile_idx].pc = pc + imm_j;
        }
        case OP_JALR: {
            let jump_target = (read_reg(tile_idx, rs1) + imm_i) & 0xFFFFFFFEu;
            write_reg(tile_idx, rd, next_pc);
            tiles[tile_idx].pc = jump_target;
        }
        case OP_BRANCH: {
            let a = read_reg(tile_idx, rs1);
            let b = read_reg(tile_idx, rs2);
            var take = false;
            
            switch funct3 {
                case 0u: { take = a == b; }            // BEQ
                case 1u: { take = a != b; }            // BNE
                case 4u: { take = i32(a) < i32(b); }   // BLT
                case 5u: { take = i32(a) >= i32(b); }  // BGE
                case 6u: { take = a < b; }             // BLTU
                case 7u: { take = a >= b; }            // BGEU
                default: {}
            }
            
            if take {
                tiles[tile_idx].pc = pc + imm_b;
            } else {
                tiles[tile_idx].pc = next_pc;
            }
        }
        case OP_LOAD: {
            let addr = read_reg(tile_idx, rs1) + imm_i;
            let val = read_ram(tile_idx, addr);
            write_reg(tile_idx, rd, val);
            tiles[tile_idx].pc = next_pc;
        }
        case OP_STORE: {
            let addr = read_reg(tile_idx, rs1) + imm_s;
            let val = read_reg(tile_idx, rs2);
            write_ram(tile_idx, addr, val);
            tiles[tile_idx].pc = next_pc;
        }
        case OP_OP_IMM: {
            let a = read_reg(tile_idx, rs1);
            let shamt = (inst >> 20u) & 0x1Fu;
            var result = 0u;
            
            switch funct3 {
                case 0u: { result = a + imm_i; }      // ADDI
                case 1u: { result = a << shamt; }      // SLLI
                case 2u: { result = select(0u, 1u, i32(a) < i32(imm_i)); } // SLTI
                case 3u: { result = select(0u, 1u, a < imm_i); }  // SLTIU
                case 4u: { result = a ^ imm_i; }       // XORI
                case 5u: {
                    if funct7 == 0x20u {
                        // SRAI (arithmetic right shift)
                        if (a & 0x80000000u) != 0u {
                            result = (a >> shamt) | (0xFFFFFFFFu << (32u - shamt));
                        } else {
                            result = a >> shamt;
                        }
                    } else {
                        result = a >> shamt; // SRLI
                    }
                }
                case 6u: { result = a | imm_i; }       // ORI
                case 7u: { result = a & imm_i; }       // ANDI
                default: {}
            }
            
            write_reg(tile_idx, rd, result);
            tiles[tile_idx].pc = next_pc;
        }
        case OP_OP: {
            let a = read_reg(tile_idx, rs1);
            let b = read_reg(tile_idx, rs2);
            var result = 0u;
            
            if funct7 == 0x01u {
                // M extension (simplified)
                switch funct3 {
                    case 0u: { result = u32(i32(a) * i32(b)); } // MUL
                    case 1u: { result = u32(i32(a) * i32(b)) >> 32u; } // MULH
                    default: {}
                }
            } else if funct7 == 0x20u {
                switch funct3 {
                    case 0u: { result = u32(i32(a) - i32(b)); } // SUB
                    case 5u: {
                        // SRA
                        if (a & 0x80000000u) != 0u {
                            result = (a >> (b & 0x1Fu)) | (0xFFFFFFFFu << (32u - (b & 0x1Fu)));
                        } else {
                            result = a >> (b & 0x1Fu);
                        }
                    }
                    default: {}
                }
            } else {
                switch funct3 {
                    case 0u: { result = a + b; }        // ADD
                    case 1u: { result = a << (b & 0x1Fu); } // SLL
                    case 2u: { result = select(0u, 1u, i32(a) < i32(b)); } // SLT
                    case 3u: { result = select(0u, 1u, a < b); }  // SLTU
                    case 4u: { result = a ^ b; }        // XOR
                    case 5u: { result = a >> (b & 0x1Fu); } // SRL
                    case 6u: { result = a | b; }        // OR
                    case 7u: { result = a & b; }        // AND
                    default: {}
                }
            }
            
            write_reg(tile_idx, rd, result);
            tiles[tile_idx].pc = next_pc;
        }
        case OP_SYSTEM: {
            // ECALL = halt (for our test cartridges)
            // EBREAK = halt with error
            if inst == 0x00000073u {
                // ECALL - treat as halt
                tiles[tile_idx].status = tiles[tile_idx].status | 0x2u; // set halted bit
            } else if inst == 0x00100073u {
                // EBREAK - halt with error
                tiles[tile_idx].status = tiles[tile_idx].status | 0x6u; // halted + error
            }
            tiles[tile_idx].pc = next_pc;
        }
        default: {
            // Unknown instruction - halt with error
            tiles[tile_idx].status = tiles[tile_idx].status | 0x6u;
        }
    }
    
    tiles[tile_idx].instruction_count += 1u;
    
    // Check if still running
    let halted = (tiles[tile_idx].status & 0x2u) != 0u;
    let error = (tiles[tile_idx].status & 0x4u) != 0u;
    let exceeded = tiles[tile_idx].instruction_count >= tiles[tile_idx].max_steps;
    
    if exceeded {
        tiles[tile_idx].status = tiles[tile_idx].status | 0x6u; // halt + error
        return false;
    }
    
    return !halted && !error;
}

@compute @workgroup_size(1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let tile_idx = gid.x;
    if tile_idx >= 256u { return; }
    
    // Check if tile is active
    if (tiles[tile_idx].status & 0x1u) == 0u { return; }
    
    // Execute up to max_steps instructions
    for (var step = 0u; step < tiles[tile_idx].max_steps; step++) {
        if !execute_one(tile_idx) {
            break;
        }
    }
    
    // Clear running bit when done
    tiles[tile_idx].status = tiles[tile_idx].status & ~0x1u;
}
