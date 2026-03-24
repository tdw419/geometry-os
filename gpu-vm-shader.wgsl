// GPU-Powered Pixel VM
// The framebuffer IS the computer. Pixels are code, data, and state.
// The GPU executes by reading pixel values as instructions.
//
// Layout:
//   Pixel (0,0): CPU — contains PC in g, flags in b
//   Pixels (1..16, 0): Registers r0-r15 — value stored in g|(b<<8)
//   Pixel (17,0): Output cursor position
//   Pixels (0..W, 1): Stack
//   Pixels (0..W, 2..H-10): Program memory (1 pixel = 1 instruction)
//   Pixels (0..W, H-10..H): Output display
//
// Instruction encoding (per pixel):
//   r = opcode
//   g = operand A
//   b = operand B  
//   a = destination / flags
//
// One GPU frame = one instruction cycle.
// The program IS the pixels. The GPU powers the computation.

struct Pixel {
    r: u32,
    g: u32,
    b: u32,
    a: u32,
}

struct Config {
    width: u32,
    height: u32,
    time: f32,
    frame: u32,
    mode: u32,
}

@group(0) @binding(0) var<storage, read> buf_in: array<Pixel>;
@group(0) @binding(1) var<storage, read_write> buf_out: array<Pixel>;
@group(0) @binding(2) var<storage, read> bytecode: array<u32>;
@group(0) @binding(3) var<storage, read> constants: array<f32>;
@group(0) @binding(4) var<uniform> config: Config;

fn px_idx(x: u32, y: u32) -> u32 {
    return y * config.width + x;
}

fn read_px(x: u32, y: u32) -> Pixel {
    if (x < config.width && y < config.height) {
        return buf_in[px_idx(x, y)];
    }
    var empty: Pixel;
    empty.r = 0u; empty.g = 0u; empty.b = 0u; empty.a = 0u;
    return empty;
}

fn read_reg(reg: u32) -> u32 {
    // Registers are pixels (1..16, 0)
    let r = (reg & 0xFu) + 1u;
    let px = read_px(r, 0u);
    return px.g | (px.b << 8u);
}

fn read_prog(addr: u32) -> Pixel {
    // Program starts at row 2
    let prog_width = config.width;
    let x = addr % prog_width;
    let y = 2u + addr / prog_width;
    return read_px(x, y);
}

// VM opcodes
const VM_NOP: u32    = 0x00u;
const VM_HALT: u32   = 0xFFu;
const VM_LOAD: u32   = 0x01u;  // reg[a] = g | (b<<8)
const VM_ADD: u32    = 0x02u;  // reg[a] = reg[g] + reg[b]
const VM_SUB: u32    = 0x03u;  // reg[a] = reg[g] - reg[b]
const VM_MUL: u32    = 0x04u;  // reg[a] = reg[g] * reg[b]
const VM_AND: u32    = 0x07u;  // reg[a] = reg[g] & reg[b]
const VM_OR: u32     = 0x08u;  // reg[a] = reg[g] | reg[b]
const VM_XOR: u32    = 0x09u;  // reg[a] = reg[g] ^ reg[b]
const VM_CMP: u32    = 0x0Du;  // compare reg[g] vs reg[b], set flags
const VM_MOV: u32    = 0x0Eu;  // reg[a] = reg[g]
const VM_JMP: u32    = 0x10u;  // PC = g | (b<<8)
const VM_JEQ: u32    = 0x11u;  // if eq: PC = g | (b<<8)
const VM_JNE: u32    = 0x12u;  // if !eq: PC = g | (b<<8)
const VM_JLT: u32    = 0x14u;  // if lt: PC = g | (b<<8)
const VM_PRINT: u32  = 0x40u;  // output reg[g] as ASCII char
const VM_PRINTI: u32 = 0x41u;  // output reg[g] as decimal number
const VM_POKE: u32   = 0x31u;  // pixel[reg[g], reg[b]] = reg[a]

// Flag bits stored in CPU pixel's b channel
const FLAG_EQ: u32 = 1u;
const FLAG_GT: u32 = 2u;
const FLAG_LT: u32 = 4u;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let x = gid.x;
    let y = gid.y;
    if (x >= config.width || y >= config.height) { return; }

    let idx = px_idx(x, y);
    let cell = buf_in[idx];

    // === CPU PIXEL (0,0) — executes one instruction per frame ===
    if (x == 0u && y == 0u) {
        // First: copy all of row 0 to output (preserve registers)
        for (var i = 0u; i < config.width; i = i + 1u) {
            buf_out[px_idx(i, 0u)] = buf_in[px_idx(i, 0u)];
        }

        let pc = cell.g | (cell.b << 8u);
        let flags = cell.a;

        // Check halt
        if (cell.r == VM_HALT) {
            buf_out[idx] = cell; // Stay halted
            return;
        }

        // Fetch instruction from program memory
        let inst = read_prog(pc);
        let op = inst.r;
        let opa = inst.g;
        let opb = inst.b;
        let dest = inst.a;

        var new_pc = pc + 1u;
        var new_flags = flags;
        var new_halt = 0u;

        switch (op) {
            case VM_NOP: {}
            case VM_HALT: {
                new_halt = VM_HALT;
            }
            case VM_LOAD: {
                // reg[dest] = opa | (opb << 8)
                let val = opa | (opb << 8u);
                let reg_idx = (dest & 0xFu) + 1u;
                var reg_px: Pixel;
                reg_px.r = 0x01u; // Mark as register
                reg_px.g = val & 0xFFu;
                reg_px.b = (val >> 8u) & 0xFFu;
                reg_px.a = 254u;
                buf_out[px_idx(reg_idx, 0u)] = reg_px;
            }
            case VM_ADD: {
                let va = read_reg(opa);
                let vb = read_reg(opb);
                let result = va + vb;
                let reg_idx = (dest & 0xFu) + 1u;
                var reg_px: Pixel;
                reg_px.r = 0x01u;
                reg_px.g = result & 0xFFu;
                reg_px.b = (result >> 8u) & 0xFFu;
                reg_px.a = 254u;
                buf_out[px_idx(reg_idx, 0u)] = reg_px;
            }
            case VM_SUB: {
                let va = read_reg(opa);
                let vb = read_reg(opb);
                let result = va - vb;
                let reg_idx = (dest & 0xFu) + 1u;
                var reg_px: Pixel;
                reg_px.r = 0x01u;
                reg_px.g = result & 0xFFu;
                reg_px.b = (result >> 8u) & 0xFFu;
                reg_px.a = 254u;
                buf_out[px_idx(reg_idx, 0u)] = reg_px;
            }
            case VM_MUL: {
                let va = read_reg(opa);
                let vb = read_reg(opb);
                let result = va * vb;
                let reg_idx = (dest & 0xFu) + 1u;
                var reg_px: Pixel;
                reg_px.r = 0x01u;
                reg_px.g = result & 0xFFu;
                reg_px.b = (result >> 8u) & 0xFFu;
                reg_px.a = 254u;
                buf_out[px_idx(reg_idx, 0u)] = reg_px;
            }
            case VM_MOV: {
                let val = read_reg(opa);
                let reg_idx = (dest & 0xFu) + 1u;
                var reg_px: Pixel;
                reg_px.r = 0x01u;
                reg_px.g = val & 0xFFu;
                reg_px.b = (val >> 8u) & 0xFFu;
                reg_px.a = 254u;
                buf_out[px_idx(reg_idx, 0u)] = reg_px;
            }
            case VM_CMP: {
                let va = read_reg(opa);
                let vb = read_reg(opb);
                new_flags = 0u;
                if (va == vb) { new_flags = new_flags | FLAG_EQ; }
                if (va > vb)  { new_flags = new_flags | FLAG_GT; }
                if (va < vb)  { new_flags = new_flags | FLAG_LT; }
            }
            case VM_JMP: {
                new_pc = opa | (opb << 8u);
            }
            case VM_JEQ: {
                if ((flags & FLAG_EQ) != 0u) {
                    new_pc = opa | (opb << 8u);
                }
            }
            case VM_JNE: {
                if ((flags & FLAG_EQ) == 0u) {
                    new_pc = opa | (opb << 8u);
                }
            }
            case VM_JLT: {
                if ((flags & FLAG_LT) != 0u) {
                    new_pc = opa | (opb << 8u);
                }
            }
            case VM_PRINT: {
                // Write ASCII char to output area
                let ch = read_reg(opa) & 0xFFu;
                let cursor_px = read_px(17u, 0u);
                let cursor_x = cursor_px.g;
                let cursor_y = cursor_px.b;

                let out_y = config.height - 10u + cursor_y;
                let out_x = cursor_x;

                if (out_x < config.width && out_y < config.height) {
                    var char_px: Pixel;
                    char_px.r = ch;
                    char_px.g = 0u;
                    char_px.b = 220u;
                    char_px.a = 254u;
                    buf_out[px_idx(out_x, out_y)] = char_px;
                }

                // Advance cursor
                var new_cursor: Pixel;
                if (ch == 10u) { // newline
                    new_cursor.g = 0u;
                    new_cursor.b = cursor_y + 1u;
                } else {
                    new_cursor.g = cursor_x + 1u;
                    new_cursor.b = cursor_y;
                }
                new_cursor.r = 0x02u;
                new_cursor.a = 254u;
                buf_out[px_idx(17u, 0u)] = new_cursor;
            }
            case VM_PRINTI: {
                // Write number as decimal digits to output
                let val = read_reg(opa);
                let cursor_px = read_px(17u, 0u);
                var cx = cursor_px.g;
                let cy = cursor_px.b;
                let out_y = config.height - 10u + cy;

                // Extract digits (up to 5 digits for 16-bit values)
                var digits: array<u32, 6>;
                var num = val;
                var n_digits = 0u;

                if (num == 0u) {
                    digits[0] = 0u;
                    n_digits = 1u;
                } else {
                    for (var di = 0u; di < 6u && num > 0u; di = di + 1u) {
                        digits[di] = num % 10u;
                        num = num / 10u;
                        n_digits = di + 1u;
                    }
                }

                // Write digits in reverse order
                for (var di = 0u; di < n_digits; di = di + 1u) {
                    let digit = digits[n_digits - 1u - di];
                    let ch = digit + 48u; // '0' = 48
                    if (cx < config.width && out_y < config.height) {
                        var char_px: Pixel;
                        char_px.r = ch;
                        char_px.g = 0u;
                        char_px.b = 220u;
                        char_px.a = 254u;
                        buf_out[px_idx(cx, out_y)] = char_px;
                    }
                    cx = cx + 1u;
                }

                // Update cursor
                var new_cursor: Pixel;
                new_cursor.r = 0x02u;
                new_cursor.g = cx;
                new_cursor.b = cy;
                new_cursor.a = 254u;
                buf_out[px_idx(17u, 0u)] = new_cursor;
            }
            case VM_POKE: {
                // Write to arbitrary pixel
                let px_x = read_reg(opa);
                let px_y = read_reg(opb);
                let val = read_reg(dest);
                if (px_x < config.width && px_y < config.height) {
                    var poke_px: Pixel;
                    poke_px.r = val & 0xFFu;
                    poke_px.g = (val >> 8u) & 0xFFu;
                    poke_px.b = (val >> 16u) & 0xFFu;
                    poke_px.a = 254u;
                    buf_out[px_idx(px_x, px_y)] = poke_px;
                }
            }
            default: {
                new_halt = VM_HALT; // Unknown opcode = halt
            }
        }

        // Write CPU state back
        var cpu: Pixel;
        cpu.r = new_halt;
        cpu.g = new_pc & 0xFFu;
        cpu.b = (new_pc >> 8u) & 0xFFu;
        cpu.a = new_flags;
        buf_out[idx] = cpu;
        return;
    }

    // === ROW 0: CPU-owned. Only CPU pixel (0,0) writes here. ===
    // Other pixels in row 0 do NOT self-copy to avoid write races.
    if (y == 0u) {
        // Don't write — let CPU pixel be the sole writer to row 0.
        return;
    }

    // === ALL OTHER PIXELS: pass through (they are memory) ===
    buf_out[idx] = cell;
}
