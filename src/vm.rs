// vm.rs -- Geometry OS Virtual Machine
//
// Executes bytecode assembled from the canvas text surface.
// The VM is simple: fetch one u32 from RAM at PC, decode as opcode, execute.
// 32 registers (r0-r31), 64K RAM, 256x256 screen buffer.

pub const RAM_SIZE: usize = 0x10000; // 65536 u32 cells
pub const SCREEN_SIZE: usize = 256 * 256;
pub const NUM_REGS: usize = 32;

/// Magic bytes for save files
pub const SAVE_MAGIC: &[u8; 4] = b"GEOS";
/// Save file format version
pub const SAVE_VERSION: u32 = 1;

#[derive(Debug, Clone)]
pub struct Vm {
    pub ram: Vec<u32>,
    pub regs: [u32; NUM_REGS],
    pub pc: u32,
    pub screen: Vec<u32>,
    pub halted: bool,
    /// Set by FRAME opcode; cleared by the host after rendering
    pub frame_ready: bool,
}

impl Vm {
    pub fn new() -> Self {
        Vm {
            ram: vec![0; RAM_SIZE],
            regs: [0; NUM_REGS],
            pc: 0,
            screen: vec![0; SCREEN_SIZE],
            halted: false,
            frame_ready: false,
        }
    }

    #[allow(dead_code)]
    pub fn reset(&mut self) {
        for r in self.ram.iter_mut() { *r = 0; }
        for s in self.screen.iter_mut() { *s = 0; }
        self.regs = [0; NUM_REGS];
        self.pc = 0;
        self.halted = false;
        self.frame_ready = false;
    }

    /// Execute one instruction. Returns false if halted.
    pub fn step(&mut self) -> bool {
        if self.halted || self.pc as usize >= self.ram.len() {
            self.halted = true;
            return false;
        }

        let opcode = self.fetch();
        match opcode {
            // HALT
            0x00 => {
                self.halted = true;
                return false;
            }

            // NOP
            0x01 => {}

            // FRAME -- signal host to display current screen; execution continues
            0x02 => {
                self.frame_ready = true;
                return true; // keep running (host checks frame_ready to pace rendering)
            }

            // LDI reg, imm  -- load immediate
            0x10 => {
                let reg = self.fetch() as usize;
                let imm = self.fetch();
                if reg < NUM_REGS {
                    self.regs[reg] = imm;
                }
            }

            // LOAD reg, addr_reg  -- load from RAM
            0x11 => {
                let reg = self.fetch() as usize;
                let addr_reg = self.fetch() as usize;
                if reg < NUM_REGS && addr_reg < NUM_REGS {
                    let addr = self.regs[addr_reg] as usize;
                    if addr < self.ram.len() {
                        self.regs[reg] = self.ram[addr];
                    }
                }
            }

            // STORE addr_reg, reg  -- store to RAM
            0x12 => {
                let addr_reg = self.fetch() as usize;
                let reg = self.fetch() as usize;
                if addr_reg < NUM_REGS && reg < NUM_REGS {
                    let addr = self.regs[addr_reg] as usize;
                    if addr < self.ram.len() {
                        self.ram[addr] = self.regs[reg];
                    }
                }
            }

            // ADD rd, rs  -- rd = rd + rs
            0x20 => {
                let rd = self.fetch() as usize;
                let rs = self.fetch() as usize;
                if rd < NUM_REGS && rs < NUM_REGS {
                    self.regs[rd] = self.regs[rd].wrapping_add(self.regs[rs]);
                }
            }

            // SUB rd, rs
            0x21 => {
                let rd = self.fetch() as usize;
                let rs = self.fetch() as usize;
                if rd < NUM_REGS && rs < NUM_REGS {
                    self.regs[rd] = self.regs[rd].wrapping_sub(self.regs[rs]);
                }
            }

            // MUL rd, rs
            0x22 => {
                let rd = self.fetch() as usize;
                let rs = self.fetch() as usize;
                if rd < NUM_REGS && rs < NUM_REGS {
                    self.regs[rd] = self.regs[rd].wrapping_mul(self.regs[rs]);
                }
            }

            // DIV rd, rs
            0x23 => {
                let rd = self.fetch() as usize;
                let rs = self.fetch() as usize;
                if rd < NUM_REGS && rs < NUM_REGS && self.regs[rs] != 0 {
                    self.regs[rd] = self.regs[rd] / self.regs[rs];
                }
            }

            // AND rd, rs
            0x24 => {
                let rd = self.fetch() as usize;
                let rs = self.fetch() as usize;
                if rd < NUM_REGS && rs < NUM_REGS {
                    self.regs[rd] &= self.regs[rs];
                }
            }

            // OR rd, rs
            0x25 => {
                let rd = self.fetch() as usize;
                let rs = self.fetch() as usize;
                if rd < NUM_REGS && rs < NUM_REGS {
                    self.regs[rd] |= self.regs[rs];
                }
            }

            // XOR rd, rs
            0x26 => {
                let rd = self.fetch() as usize;
                let rs = self.fetch() as usize;
                if rd < NUM_REGS && rs < NUM_REGS {
                    self.regs[rd] ^= self.regs[rs];
                }
            }

            // SHL rd, rs  -- rd = rd << rs (logical shift left)
            0x27 => {
                let rd = self.fetch() as usize;
                let rs = self.fetch() as usize;
                if rd < NUM_REGS && rs < NUM_REGS {
                    let shift = self.regs[rs] % 32;
                    self.regs[rd] = self.regs[rd].wrapping_shl(shift);
                }
            }

            // SHR rd, rs  -- rd = rd >> rs (logical shift right)
            0x28 => {
                let rd = self.fetch() as usize;
                let rs = self.fetch() as usize;
                if rd < NUM_REGS && rs < NUM_REGS {
                    let shift = self.regs[rs] % 32;
                    self.regs[rd] = self.regs[rd].wrapping_shr(shift);
                }
            }

            // MOD rd, rs  -- rd = rd % rs
            0x29 => {
                let rd = self.fetch() as usize;
                let rs = self.fetch() as usize;
                if rd < NUM_REGS && rs < NUM_REGS && self.regs[rs] != 0 {
                    self.regs[rd] = self.regs[rd] % self.regs[rs];
                }
            }

            // NEG rd  -- rd = -rd (two's complement)
            0x2A => {
                let rd = self.fetch() as usize;
                if rd < NUM_REGS {
                    self.regs[rd] = self.regs[rd].wrapping_neg();
                }
            }

            // JMP addr
            0x30 => {
                let addr = self.fetch();
                self.pc = addr;
                return true; // don't increment PC again
            }

            // JZ reg, addr  -- jump if reg == 0
            0x31 => {
                let reg = self.fetch() as usize;
                let addr = self.fetch();
                if reg < NUM_REGS && self.regs[reg] == 0 {
                    self.pc = addr;
                    return true;
                }
            }

            // JNZ reg, addr  -- jump if reg != 0
            0x32 => {
                let reg = self.fetch() as usize;
                let addr = self.fetch();
                if reg < NUM_REGS && self.regs[reg] != 0 {
                    self.pc = addr;
                    return true;
                }
            }

            // CALL addr
            0x33 => {
                let addr = self.fetch();
                // Push return address to r31 (link register)
                if NUM_REGS > 0 {
                    self.regs[31] = self.pc;
                }
                self.pc = addr;
                return true;
            }

            // RET  -- jump to r31
            0x34 => {
                self.pc = self.regs[31];
                return true;
            }

            // BLT reg, addr  -- branch if CMP result < 0 (r0 == 0xFFFFFFFF)
            0x35 => {
                let _reg = self.fetch() as usize;
                let addr = self.fetch();
                if self.regs[0] == 0xFFFFFFFF {
                    self.pc = addr;
                    return true;
                }
            }

            // BGE reg, addr  -- branch if CMP result >= 0 (r0 != 0xFFFFFFFF)
            0x36 => {
                let _reg = self.fetch() as usize;
                let addr = self.fetch();
                if self.regs[0] != 0xFFFFFFFF {
                    self.pc = addr;
                    return true;
                }
            }

            // PUSH reg  -- push register onto stack (r30 is SP, grows down)
            0x60 => {
                let reg = self.fetch() as usize;
                if reg < NUM_REGS {
                    // Decrement SP (r30)
                    let sp = self.regs[30] as usize;
                    if sp > 0 && sp <= self.ram.len() {
                        let new_sp = sp - 1;
                        self.ram[new_sp] = self.regs[reg];
                        self.regs[30] = new_sp as u32;
                    }
                }
            }

            // POP reg  -- pop from stack into register (r30 is SP)
            0x61 => {
                let reg = self.fetch() as usize;
                if reg < NUM_REGS {
                    let sp = self.regs[30] as usize;
                    if sp < self.ram.len() {
                        self.regs[reg] = self.ram[sp];
                        self.regs[30] = (sp + 1) as u32;
                    }
                }
            }

            // PSET x_reg, y_reg, color_reg  -- set pixel on screen
            0x40 => {
                let xr = self.fetch() as usize;
                let yr = self.fetch() as usize;
                let cr = self.fetch() as usize;
                if xr < NUM_REGS && yr < NUM_REGS && cr < NUM_REGS {
                    let x = self.regs[xr] as usize;
                    let y = self.regs[yr] as usize;
                    let color = self.regs[cr];
                    if x < 256 && y < 256 {
                        self.screen[y * 256 + x] = color;
                    }
                }
            }

            // PSETI x, y, color  -- set pixel with immediate values
            0x41 => {
                let x = self.fetch() as usize;
                let y = self.fetch() as usize;
                let color = self.fetch();
                if x < 256 && y < 256 {
                    self.screen[y * 256 + x] = color;
                }
            }

            // FILL color_reg  -- fill entire screen
            0x42 => {
                let cr = self.fetch() as usize;
                if cr < NUM_REGS {
                    let color = self.regs[cr];
                    for pixel in self.screen.iter_mut() {
                        *pixel = color;
                    }
                }
            }

            // RECTF x_reg, y_reg, w_reg, h_reg, color_reg  -- filled rectangle
            0x43 => {
                let xr = self.fetch() as usize;
                let yr = self.fetch() as usize;
                let wr = self.fetch() as usize;
                let hr = self.fetch() as usize;
                let cr = self.fetch() as usize;
                if xr < NUM_REGS && yr < NUM_REGS && wr < NUM_REGS
                    && hr < NUM_REGS && cr < NUM_REGS
                {
                    let x0 = self.regs[xr] as usize;
                    let y0 = self.regs[yr] as usize;
                    let w = self.regs[wr] as usize;
                    let h = self.regs[hr] as usize;
                    let color = self.regs[cr];
                    for dy in 0..h {
                        for dx in 0..w {
                            let px = x0 + dx;
                            let py = y0 + dy;
                            if px < 256 && py < 256 {
                                self.screen[py * 256 + px] = color;
                            }
                        }
                    }
                }
            }

            // TEXT x_reg, y_reg, addr_reg  -- render text from RAM to screen
            0x44 => {
                let xr = self.fetch() as usize;
                let yr = self.fetch() as usize;
                let ar = self.fetch() as usize;
                if xr < NUM_REGS && yr < NUM_REGS && ar < NUM_REGS {
                    let mut sx = self.regs[xr] as usize;
                    let mut sy = self.regs[yr] as usize;
                    let mut addr = self.regs[ar] as usize;
                    let fg = 0xFFFFFF; // white text
                    loop {
                        if addr >= self.ram.len() { break; }
                        let ch = (self.ram[addr] & 0xFF) as u8;
                        if ch == 0 { break; }
                        if ch == b'\n' {
                            sx = self.regs[xr] as usize;
                            sy += 10;
                            addr += 1;
                            continue;
                        }
                        // Render 5x7 glyph at (sx, sy) -- inline for now
                        self.draw_char(ch, sx, sy, fg);
                        sx += 6; // 5 wide + 1 gap
                        if sx > 250 {
                            sx = self.regs[xr] as usize;
                            sy += 8;
                        }
                        addr += 1;
                    }
                }
            }

            // CMP rd, rs  -- set r0 = comparison result (-1, 0, 1)
            0x50 => {
                let rd = self.fetch() as usize;
                let rs = self.fetch() as usize;
                if rd < NUM_REGS && rs < NUM_REGS {
                    let a = self.regs[rd] as i32;
                    let b = self.regs[rs] as i32;
                    self.regs[0] = if a < b { 0xFFFFFFFF } else if a > b { 1 } else { 0 };
                }
            }

            // IKEY reg  -- read keyboard port (RAM[0xFFF]) into reg, then clear port
            0x48 => {
                let rd = self.fetch() as usize;
                if rd < NUM_REGS {
                    self.regs[rd] = self.ram[0xFFF];
                    self.ram[0xFFF] = 0;
                }
            }

            // LINE x0r, y0r, x1r, y1r, cr  -- Bresenham line
            0x45 => {
                let x0r = self.fetch() as usize;
                let y0r = self.fetch() as usize;
                let x1r = self.fetch() as usize;
                let y1r = self.fetch() as usize;
                let cr  = self.fetch() as usize;
                if x0r < NUM_REGS && y0r < NUM_REGS && x1r < NUM_REGS
                    && y1r < NUM_REGS && cr < NUM_REGS
                {
                    let color = self.regs[cr];
                    let mut x0 = self.regs[x0r] as i32;
                    let mut y0 = self.regs[y0r] as i32;
                    let x1 = self.regs[x1r] as i32;
                    let y1 = self.regs[y1r] as i32;
                    let dx = (x1 - x0).abs();
                    let dy = -(y1 - y0).abs();
                    let sx: i32 = if x0 < x1 { 1 } else { -1 };
                    let sy: i32 = if y0 < y1 { 1 } else { -1 };
                    let mut err = dx + dy;
                    loop {
                        if x0 >= 0 && x0 < 256 && y0 >= 0 && y0 < 256 {
                            self.screen[y0 as usize * 256 + x0 as usize] = color;
                        }
                        if x0 == x1 && y0 == y1 { break; }
                        let e2 = 2 * err;
                        if e2 >= dy { err += dy; x0 += sx; }
                        if e2 <= dx { err += dx; y0 += sy; }
                    }
                }
            }

            // CIRCLE xr, yr, rr, cr  -- midpoint circle
            0x46 => {
                let xr = self.fetch() as usize;
                let yr = self.fetch() as usize;
                let rr = self.fetch() as usize;
                let cr = self.fetch() as usize;
                if xr < NUM_REGS && yr < NUM_REGS && rr < NUM_REGS && cr < NUM_REGS {
                    let cx = self.regs[xr] as i32;
                    let cy = self.regs[yr] as i32;
                    let radius = self.regs[rr] as i32;
                    let color = self.regs[cr];
                    let mut x = radius;
                    let mut y = 0i32;
                    let mut err = 1 - radius;
                    while x >= y {
                        let pts: [(i32, i32); 8] = [
                            (cx + x, cy + y), (cx - x, cy + y),
                            (cx + x, cy - y), (cx - x, cy - y),
                            (cx + y, cy + x), (cx - y, cy + x),
                            (cx + y, cy - x), (cx - y, cy - x),
                        ];
                        for (px, py) in pts {
                            if px >= 0 && px < 256 && py >= 0 && py < 256 {
                                self.screen[py as usize * 256 + px as usize] = color;
                            }
                        }
                        y += 1;
                        if err < 0 {
                            err += 2 * y + 1;
                        } else {
                            x -= 1;
                            err += 2 * (y - x) + 1;
                        }
                    }
                }
            }

            // SCROLL nr  -- scroll screen up by regs[nr] pixels (wraps 0 in at bottom)
            0x47 => {
                let nr = self.fetch() as usize;
                if nr < NUM_REGS {
                    let n = (self.regs[nr] as usize).min(256);
                    if n > 0 {
                        self.screen.copy_within(n * 256.., 0);
                        for v in self.screen[(256 - n) * 256..].iter_mut() {
                            *v = 0;
                        }
                    }
                }
            }

            // Unknown opcode: halt
            _ => {
                self.halted = true;
                return false;
            }
        }
        true
    }

    /// Disassemble one instruction starting at `addr` in RAM.
    /// Returns (mnemonic_string, instruction_length_in_words).
    /// Does not mutate VM state.
    pub fn disassemble_at(&self, addr: u32) -> (String, usize) {
        let a = addr as usize;
        if a >= self.ram.len() {
            return (format!("???"), 1);
        }
        let op = self.ram[a];
        let ram = |i: usize| -> u32 {
            if i < self.ram.len() { self.ram[i] } else { 0 }
        };
        let reg = |v: u32| -> String { format!("r{}", v) };
        match op {
            0x00 => ("HALT".into(), 1),
            0x01 => ("NOP".into(), 1),
            0x02 => ("FRAME".into(), 1),
            0x10 => {
                let r = ram(a + 1);
                let imm = ram(a + 2);
                (format!("LDI {}, 0x{:X}", reg(r), imm), 3)
            }
            0x11 => {
                let r = ram(a + 1);
                let ar = ram(a + 2);
                (format!("LOAD {}, [{}]", reg(r), reg(ar)), 3)
            }
            0x12 => {
                let ar = ram(a + 1);
                let r = ram(a + 2);
                (format!("STORE [{}], {}", reg(ar), reg(r)), 3)
            }
            0x20 => { let rd = ram(a+1); let rs = ram(a+2); (format!("ADD {}, {}", reg(rd), reg(rs)), 3) }
            0x21 => { let rd = ram(a+1); let rs = ram(a+2); (format!("SUB {}, {}", reg(rd), reg(rs)), 3) }
            0x22 => { let rd = ram(a+1); let rs = ram(a+2); (format!("MUL {}, {}", reg(rd), reg(rs)), 3) }
            0x23 => { let rd = ram(a+1); let rs = ram(a+2); (format!("DIV {}, {}", reg(rd), reg(rs)), 3) }
            0x24 => { let rd = ram(a+1); let rs = ram(a+2); (format!("AND {}, {}", reg(rd), reg(rs)), 3) }
            0x25 => { let rd = ram(a+1); let rs = ram(a+2); (format!("OR {}, {}", reg(rd), reg(rs)), 3) }
            0x26 => { let rd = ram(a+1); let rs = ram(a+2); (format!("XOR {}, {}", reg(rd), reg(rs)), 3) }
            0x27 => { let rd = ram(a+1); let rs = ram(a+2); (format!("SHL {}, {}", reg(rd), reg(rs)), 3) }
            0x28 => { let rd = ram(a+1); let rs = ram(a+2); (format!("SHR {}, {}", reg(rd), reg(rs)), 3) }
            0x29 => { let rd = ram(a+1); let rs = ram(a+2); (format!("MOD {}, {}", reg(rd), reg(rs)), 3) }
            0x2A => { let rd = ram(a+1); (format!("NEG {}", reg(rd)), 2) }
            0x30 => { let addr2 = ram(a+1); (format!("JMP 0x{:04X}", addr2), 2) }
            0x31 => { let r = ram(a+1); let addr2 = ram(a+2); (format!("JZ {}, 0x{:04X}", reg(r), addr2), 3) }
            0x32 => { let r = ram(a+1); let addr2 = ram(a+2); (format!("JNZ {}, 0x{:04X}", reg(r), addr2), 3) }
            0x33 => { let addr2 = ram(a+1); (format!("CALL 0x{:04X}", addr2), 2) }
            0x34 => ("RET".into(), 1),
            0x35 => { let r = ram(a+1); let addr2 = ram(a+2); (format!("BLT {}, 0x{:04X}", reg(r), addr2), 3) }
            0x36 => { let r = ram(a+1); let addr2 = ram(a+2); (format!("BGE {}, 0x{:04X}", reg(r), addr2), 3) }
            0x40 => { let xr = ram(a+1); let yr = ram(a+2); let cr = ram(a+3); (format!("PSET {}, {}, {}", reg(xr), reg(yr), reg(cr)), 4) }
            0x41 => { let x = ram(a+1); let y = ram(a+2); let c = ram(a+3); (format!("PSETI {}, {}, 0x{:X}", x, y, c), 4) }
            0x42 => { let cr = ram(a+1); (format!("FILL {}", reg(cr)), 2) }
            0x43 => { let xr = ram(a+1); let yr = ram(a+2); let wr = ram(a+3); let hr = ram(a+4); let cr = ram(a+5); (format!("RECTF {},{},{},{},{}", reg(xr), reg(yr), reg(wr), reg(hr), reg(cr)), 6) }
            0x44 => { let xr = ram(a+1); let yr = ram(a+2); let ar = ram(a+3); (format!("TEXT {},{},[{}]", reg(xr), reg(yr), reg(ar)), 4) }
            0x45 => { let x0r = ram(a+1); let y0r = ram(a+2); let x1r = ram(a+3); let y1r = ram(a+4); let cr = ram(a+5); (format!("LINE {},{},{},{},{}", reg(x0r), reg(y0r), reg(x1r), reg(y1r), reg(cr)), 6) }
            0x46 => { let xr = ram(a+1); let yr = ram(a+2); let rr = ram(a+3); let cr = ram(a+4); (format!("CIRCLE {},{},{},{}", reg(xr), reg(yr), reg(rr), reg(cr)), 5) }
            0x47 => { let nr = ram(a+1); (format!("SCROLL {}", reg(nr)), 2) }
            0x48 => { let rd = ram(a+1); (format!("IKEY {}", reg(rd)), 2) }
            0x50 => { let rd = ram(a+1); let rs = ram(a+2); (format!("CMP {}, {}", reg(rd), reg(rs)), 3) }
            0x60 => { let r = ram(a+1); (format!("PUSH {}", reg(r)), 2) }
            0x61 => { let r = ram(a+1); (format!("POP {}", reg(r)), 2) }
            _ => (format!("??? (0x{:02X})", op), 1),
        }
    }

    fn fetch(&mut self) -> u32 {
        let val = if (self.pc as usize) < self.ram.len() {
            self.ram[self.pc as usize]
        } else {
            0
        };
        self.pc += 1;
        val
    }

    /// Draw a character to the screen buffer (tiny 5x7 inline font for TEXT opcode)
    fn draw_char(&mut self, ch: u8, x: usize, y: usize, color: u32) {
        // Simple 5x7 font for printable ASCII
        const MINI_FONT: [[u8; 7]; 96] = include!("mini_font.in");
        let idx = ch as usize;
        if idx < 32 || idx > 127 {
            return;
        }
        let glyph = &MINI_FONT[idx - 32];
        for row in 0..7usize {
            for col in 0..5usize {
                if glyph[row] & (1 << (4 - col)) != 0 {
                    let px = x + col;
                    let py = y + row;
                    if px < 256 && py < 256 {
                        self.screen[py * 256 + px] = color;
                    }
                }
            }
        }
    }

    /// Save VM state to a binary file.
    /// Format: GEOS magic (4) + version u32 (4) + halted u8 (1) + pc u32 (4)
    ///         + regs [u32; 32] (128) + ram [u32; RAM_SIZE] + screen [u32; SCREEN_SIZE]
    pub fn save_to_file(&self, path: &std::path::Path) -> std::io::Result<()> {
        use std::io::Write;
        let mut f = std::fs::File::create(path)?;
        f.write_all(SAVE_MAGIC)?;
        f.write_all(&SAVE_VERSION.to_le_bytes())?;
        f.write_all(&[if self.halted { 1 } else { 0 }])?;
        f.write_all(&self.pc.to_le_bytes())?;
        for &r in &self.regs {
            f.write_all(&r.to_le_bytes())?;
        }
        for &v in &self.ram {
            f.write_all(&v.to_le_bytes())?;
        }
        for &v in &self.screen {
            f.write_all(&v.to_le_bytes())?;
        }
        Ok(())
    }

    /// Load VM state from a binary file. Returns None if file doesn't exist
    /// or has invalid format.
    pub fn load_from_file(path: &std::path::Path) -> std::io::Result<Self> {
        use std::io::Read;
        let mut data = Vec::new();
        let mut f = std::fs::File::open(path)?;
        f.read_to_end(&mut data)?;

        // Minimum size: magic(4) + version(4) + halted(1) + pc(4) + regs(128) = 141
        let min_size = 4 + 4 + 1 + 4 + NUM_REGS * 4 + RAM_SIZE * 4 + SCREEN_SIZE * 4;
        if data.len() < min_size {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("save file too small: {} bytes (need {})", data.len(), min_size),
            ));
        }
        if &data[0..4] != SAVE_MAGIC {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "invalid magic bytes",
            ));
        }
        let version = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
        if version != SAVE_VERSION {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("unsupported save version: {} (need {})", version, SAVE_VERSION),
            ));
        }

        let mut offset = 8usize;
        let halted = data[offset] != 0;
        offset += 1;
        let pc = u32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]);
        offset += 4;

        let mut regs = [0u32; NUM_REGS];
        for r in regs.iter_mut() {
            *r = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            offset += 4;
        }

        let mut ram = vec![0u32; RAM_SIZE];
        for v in ram.iter_mut() {
            *v = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            offset += 4;
        }

        let mut screen = vec![0u32; SCREEN_SIZE];
        for v in screen.iter_mut() {
            *v = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            offset += 4;
        }

        Ok(Vm {
            ram,
            regs,
            pc,
            screen,
            halted,
            frame_ready: false,
        })
    }
}
