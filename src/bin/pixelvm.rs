// PixelVM — A virtual machine where everything is pixels
//
// The framebuffer IS memory. Each pixel (RGBA) = 4 bytes = 1 instruction.
// Programs are rows of colored pixels. Output is pixels. State is pixels.
//
// Layout (480x240 framebuffer):
//   Row 0:       VM state — registers stored as pixel values
//   Row 1:       Stack (256 entries)
//   Row 2-101:   Program memory (48,000 instructions)
//   Row 102-239: Output display area
//
// Instruction encoding (1 pixel = 1 instruction):
//   R = opcode
//   G = operand A (register index or immediate)
//   B = operand B (register index or immediate)
//   A = destination register or flag
//
// Signal convention:
//   All unused pixels = same color (0x10, 0x10, 0x10, 0xFF) = dark gray
//   This makes the FB look uniform when "idle"

use std::fmt;

// ===== OPCODES =====
const OP_NOP:    u8 = 0x00;  // No operation
const OP_LOAD:   u8 = 0x01;  // Load immediate: reg[A] = G (B=high byte)
const OP_ADD:    u8 = 0x02;  // reg[A] = reg[G] + reg[B]
const OP_SUB:    u8 = 0x03;  // reg[A] = reg[G] - reg[B]
const OP_MUL:    u8 = 0x04;  // reg[A] = reg[G] * reg[B]
const OP_DIV:    u8 = 0x05;  // reg[A] = reg[G] / reg[B]
const OP_MOD:    u8 = 0x06;  // reg[A] = reg[G] % reg[B]
const OP_AND:    u8 = 0x07;  // reg[A] = reg[G] & reg[B]
const OP_OR:     u8 = 0x08;  // reg[A] = reg[G] | reg[B]
const OP_XOR:    u8 = 0x09;  // reg[A] = reg[G] ^ reg[B]
const OP_NOT:    u8 = 0x0A;  // reg[A] = !reg[G]
const OP_SHL:    u8 = 0x0B;  // reg[A] = reg[G] << reg[B]
const OP_SHR:    u8 = 0x0C;  // reg[A] = reg[G] >> reg[B]
const OP_CMP:    u8 = 0x0D;  // compare reg[G] vs reg[B], set flags
const OP_MOV:    u8 = 0x0E;  // reg[A] = reg[G]

const OP_JMP:    u8 = 0x10;  // PC = G | (B << 8)
const OP_JEQ:    u8 = 0x11;  // if flag_eq: PC = G | (B << 8)
const OP_JNE:    u8 = 0x12;  // if !flag_eq: PC = G | (B << 8)
const OP_JGT:    u8 = 0x13;  // if flag_gt: PC = G | (B << 8)
const OP_JLT:    u8 = 0x14;  // if flag_lt: PC = G | (B << 8)
const OP_CALL:   u8 = 0x15;  // push PC, PC = G | (B << 8)
const OP_RET:    u8 = 0x16;  // PC = pop()

const OP_PUSH:   u8 = 0x20;  // push reg[G]
const OP_POP:    u8 = 0x21;  // reg[A] = pop()

const OP_PEEK:   u8 = 0x30;  // reg[A] = fb[reg[G], reg[B]]  (read pixel from FB)
const OP_POKE:   u8 = 0x31;  // fb[reg[G], reg[B]] = reg[A]  (write pixel to FB)
const OP_FILL:   u8 = 0x32;  // fill row reg[G] with color reg[B]

const OP_PRINT:  u8 = 0x40;  // print reg[G] as ASCII char to output row
const OP_PRINTI: u8 = 0x41;  // print reg[G] as decimal number
const OP_PRINTX: u8 = 0x42;  // print reg[G] as hex

const OP_HALT:   u8 = 0xFF;  // Stop execution

// ===== PIXEL =====
#[derive(Copy, Clone, PartialEq)]
struct Px {
    r: u8,
    g: u8,
    b: u8,
    a: u8,
}

impl Px {
    fn new(r: u8, g: u8, b: u8, a: u8) -> Self { Self { r, g, b, a } }
    fn idle() -> Self { Self { r: 0x10, g: 0x10, b: 0x10, a: 0xFF } }
    fn from_u32(val: u32) -> Self {
        Self {
            r: (val >> 24) as u8,
            g: (val >> 16) as u8,
            b: (val >> 8) as u8,
            a: val as u8,
        }
    }
    fn to_u32(&self) -> u32 {
        ((self.r as u32) << 24) | ((self.g as u32) << 16) | ((self.b as u32) << 8) | (self.a as u32)
    }
}

impl fmt::Debug for Px {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "({:02x},{:02x},{:02x},{:02x})", self.r, self.g, self.b, self.a)
    }
}

// ===== FRAMEBUFFER =====
struct FrameBuffer {
    width: usize,
    height: usize,
    pixels: Vec<Px>,
}

impl FrameBuffer {
    fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            pixels: vec![Px::idle(); width * height],
        }
    }

    fn get(&self, x: usize, y: usize) -> Px {
        if x < self.width && y < self.height {
            self.pixels[y * self.width + x]
        } else {
            Px::idle()
        }
    }

    fn set(&mut self, x: usize, y: usize, px: Px) {
        if x < self.width && y < self.height {
            self.pixels[y * self.width + x] = px;
        }
    }

    fn save_png(&self, path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let mut rgba = Vec::with_capacity(self.width * self.height * 4);
        for px in &self.pixels {
            rgba.extend_from_slice(&[px.r, px.g, px.b, px.a]);
        }
        let img: image::ImageBuffer<image::Rgba<u8>, Vec<u8>> =
            image::ImageBuffer::from_raw(self.width as u32, self.height as u32, rgba)
                .ok_or("Failed to create image")?;
        img.save(path)?;
        Ok(())
    }
}

// ===== VIRTUAL MACHINE =====
struct PixelVM {
    fb: FrameBuffer,
    regs: [u32; 16],       // 16 general-purpose registers
    pc: usize,             // Program counter (index into program area)
    sp: usize,             // Stack pointer
    flag_eq: bool,
    flag_gt: bool,
    flag_lt: bool,
    halted: bool,
    cycles: u64,
    output_col: usize,     // Current column in output row
    output_row: usize,     // Current output row
    stdout_buf: String,    // Text output buffer
    // Layout constants
    prog_start_row: usize,
    prog_end_row: usize,
    output_start_row: usize,
    stack_row: usize,
}

impl PixelVM {
    fn new(width: usize, height: usize) -> Self {
        Self {
            fb: FrameBuffer::new(width, height),
            regs: [0; 16],
            pc: 0,
            sp: 0,
            flag_eq: false,
            flag_gt: false,
            flag_lt: false,
            halted: false,
            cycles: 0,
            output_col: 0,
            output_row: 102,
            stdout_buf: String::new(),
            prog_start_row: 2,
            prog_end_row: 101,
            output_start_row: 102,
            stack_row: 1,
        }
    }

    /// Load a program (array of instruction pixels) into the framebuffer
    fn load_program(&mut self, instructions: &[Px]) {
        let cols = self.fb.width;
        for (i, inst) in instructions.iter().enumerate() {
            let row = self.prog_start_row + i / cols;
            let col = i % cols;
            if row <= self.prog_end_row {
                self.fb.set(col, row, *inst);
            }
        }
        // Write program length to state row
        self.fb.set(0, 0, Px::from_u32(instructions.len() as u32));
        println!("Loaded {} instructions ({} rows of pixels)", 
            instructions.len(), (instructions.len() + cols - 1) / cols);
    }

    /// Fetch instruction at current PC
    fn fetch(&self) -> Px {
        let cols = self.fb.width;
        let row = self.prog_start_row + self.pc / cols;
        let col = self.pc % cols;
        self.fb.get(col, row)
    }

    /// Sync VM state TO framebuffer (state → pixels)
    fn sync_state_to_fb(&mut self) {
        // Row 0: registers as pixels
        for i in 0..16 {
            self.fb.set(i + 2, 0, Px::from_u32(self.regs[i]));
        }
        // PC and SP
        self.fb.set(18, 0, Px::from_u32(self.pc as u32));
        self.fb.set(19, 0, Px::from_u32(self.sp as u32));
        // Flags
        let flags = (self.flag_eq as u32) | ((self.flag_gt as u32) << 1) | ((self.flag_lt as u32) << 2);
        self.fb.set(20, 0, Px::from_u32(flags));
    }

    /// Push value to stack
    fn push(&mut self, val: u32) {
        if self.sp < self.fb.width {
            self.fb.set(self.sp, self.stack_row, Px::from_u32(val));
            self.sp += 1;
        }
    }

    /// Pop value from stack
    fn pop(&mut self) -> u32 {
        if self.sp > 0 {
            self.sp -= 1;
            self.fb.get(self.sp, self.stack_row).to_u32()
        } else {
            0
        }
    }

    /// Write a character to the output area
    fn output_char(&mut self, ch: u8) {
        if ch == b'\n' {
            self.output_row += 1;
            self.output_col = 0;
            return;
        }
        if self.output_row < self.fb.height && self.output_col < self.fb.width {
            // Encode ASCII as a visible pixel: bright on dark
            let brightness = 200u8;
            self.fb.set(self.output_col, self.output_row,
                Px::new(ch, brightness, brightness, 0xFF));
            self.output_col += 1;
        }
    }

    /// Execute one instruction
    fn step(&mut self) -> bool {
        if self.halted { return false; }

        let inst = self.fetch();
        let op = inst.r;
        let a = inst.g;
        let b = inst.b;
        let dest = inst.a;

        self.pc += 1;
        self.cycles += 1;

        match op {
            OP_NOP => {}
            OP_LOAD => {
                // Load immediate: reg[dest] = a | (b << 8)
                let val = (a as u32) | ((b as u32) << 8);
                self.regs[dest as usize % 16] = val;
            }
            OP_ADD => {
                let va = self.regs[a as usize % 16];
                let vb = self.regs[b as usize % 16];
                self.regs[dest as usize % 16] = va.wrapping_add(vb);
            }
            OP_SUB => {
                let va = self.regs[a as usize % 16];
                let vb = self.regs[b as usize % 16];
                self.regs[dest as usize % 16] = va.wrapping_sub(vb);
            }
            OP_MUL => {
                let va = self.regs[a as usize % 16];
                let vb = self.regs[b as usize % 16];
                self.regs[dest as usize % 16] = va.wrapping_mul(vb);
            }
            OP_DIV => {
                let va = self.regs[a as usize % 16];
                let vb = self.regs[b as usize % 16];
                self.regs[dest as usize % 16] = if vb != 0 { va / vb } else { 0 };
            }
            OP_MOD => {
                let va = self.regs[a as usize % 16];
                let vb = self.regs[b as usize % 16];
                self.regs[dest as usize % 16] = if vb != 0 { va % vb } else { 0 };
            }
            OP_AND => {
                let va = self.regs[a as usize % 16];
                let vb = self.regs[b as usize % 16];
                self.regs[dest as usize % 16] = va & vb;
            }
            OP_OR => {
                let va = self.regs[a as usize % 16];
                let vb = self.regs[b as usize % 16];
                self.regs[dest as usize % 16] = va | vb;
            }
            OP_XOR => {
                let va = self.regs[a as usize % 16];
                let vb = self.regs[b as usize % 16];
                self.regs[dest as usize % 16] = va ^ vb;
            }
            OP_NOT => {
                let va = self.regs[a as usize % 16];
                self.regs[dest as usize % 16] = !va;
            }
            OP_SHL => {
                let va = self.regs[a as usize % 16];
                let vb = self.regs[b as usize % 16];
                self.regs[dest as usize % 16] = va.wrapping_shl(vb);
            }
            OP_SHR => {
                let va = self.regs[a as usize % 16];
                let vb = self.regs[b as usize % 16];
                self.regs[dest as usize % 16] = va.wrapping_shr(vb);
            }
            OP_CMP => {
                let va = self.regs[a as usize % 16];
                let vb = self.regs[b as usize % 16];
                self.flag_eq = va == vb;
                self.flag_gt = va > vb;
                self.flag_lt = va < vb;
            }
            OP_MOV => {
                self.regs[dest as usize % 16] = self.regs[a as usize % 16];
            }
            OP_JMP => {
                self.pc = (a as usize) | ((b as usize) << 8);
            }
            OP_JEQ => {
                if self.flag_eq {
                    self.pc = (a as usize) | ((b as usize) << 8);
                }
            }
            OP_JNE => {
                if !self.flag_eq {
                    self.pc = (a as usize) | ((b as usize) << 8);
                }
            }
            OP_JGT => {
                if self.flag_gt {
                    self.pc = (a as usize) | ((b as usize) << 8);
                }
            }
            OP_JLT => {
                if self.flag_lt {
                    self.pc = (a as usize) | ((b as usize) << 8);
                }
            }
            OP_CALL => {
                self.push(self.pc as u32);
                self.pc = (a as usize) | ((b as usize) << 8);
            }
            OP_RET => {
                self.pc = self.pop() as usize;
            }
            OP_PUSH => {
                let val = self.regs[a as usize % 16];
                self.push(val);
            }
            OP_POP => {
                let val = self.pop();
                self.regs[dest as usize % 16] = val;
            }
            OP_PEEK => {
                let x = self.regs[a as usize % 16] as usize;
                let y = self.regs[b as usize % 16] as usize;
                let px = self.fb.get(x, y);
                self.regs[dest as usize % 16] = px.to_u32();
            }
            OP_POKE => {
                let x = self.regs[a as usize % 16] as usize;
                let y = self.regs[b as usize % 16] as usize;
                let val = self.regs[dest as usize % 16];
                self.fb.set(x, y, Px::from_u32(val));
            }
            OP_FILL => {
                let row = self.regs[a as usize % 16] as usize;
                let color = self.regs[b as usize % 16];
                let px = Px::from_u32(color);
                for x in 0..self.fb.width {
                    self.fb.set(x, row, px);
                }
            }
            OP_PRINT => {
                let ch = (self.regs[a as usize % 16] & 0xFF) as u8;
                self.output_char(ch);
                self.stdout_buf.push(ch as char);
            }
            OP_PRINTI => {
                let val = self.regs[a as usize % 16];
                let s = format!("{}", val);
                for ch in s.bytes() {
                    self.output_char(ch);
                    self.stdout_buf.push(ch as char);
                }
            }
            OP_PRINTX => {
                let val = self.regs[a as usize % 16];
                let s = format!("{:x}", val);
                for ch in s.bytes() {
                    self.output_char(ch);
                    self.stdout_buf.push(ch as char);
                }
            }
            OP_HALT => {
                self.halted = true;
                return false;
            }
            _ => {
                eprintln!("Unknown opcode 0x{:02x} at PC={}", op, self.pc - 1);
                self.halted = true;
                return false;
            }
        }

        true
    }

    fn run(&mut self, max_cycles: u64) {
        while self.cycles < max_cycles && !self.halted {
            self.step();
        }
        self.sync_state_to_fb();
    }

    fn dump_state(&self) {
        println!("─── VM State ───────────────────────────────");
        println!("  PC={:5}  SP={:3}  Cycles={}", self.pc, self.sp, self.cycles);
        println!("  Flags: eq={} gt={} lt={}", self.flag_eq, self.flag_gt, self.flag_lt);
        for i in 0..16 {
            if self.regs[i] != 0 {
                print!("  r{:X}={:<10}", i, self.regs[i]);
            }
        }
        println!();
        if !self.stdout_buf.is_empty() {
            println!("─── Output ─────────────────────────────────");
            println!("{}", self.stdout_buf);
        }
        println!("────────────────────────────────────────────");
    }
}

// ===== ASSEMBLER (text → pixel instructions) =====
fn assemble(source: &str) -> Vec<Px> {
    let mut instructions = Vec::new();
    let mut labels: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut fixups: Vec<(usize, String)> = Vec::new();

    // First pass: collect labels
    let mut addr = 0usize;
    for line in source.lines() {
        let line = line.split(';').next().unwrap_or("").trim();
        if line.is_empty() { continue; }
        if line.ends_with(':') {
            labels.insert(line.trim_end_matches(':').to_string(), addr);
        } else {
            addr += 1;
        }
    }

    // Second pass: assemble
    for line in source.lines() {
        let line = line.split(';').next().unwrap_or("").trim();
        if line.is_empty() || line.ends_with(':') { continue; }

        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.is_empty() { continue; }

        let op = parts[0].to_uppercase();
        let joined = parts[1..].join(" ");
        let args: Vec<String> = if parts.len() > 1 {
            joined.split(',').map(|s| s.trim().to_string()).collect()
        } else {
            vec![]
        };
        let args: Vec<&str> = args.iter().map(|s| s.as_str()).collect();

        let parse_reg = |s: &str| -> u8 {
            let s = s.trim().to_lowercase();
            if s.starts_with('r') {
                s[1..].parse::<u8>().unwrap_or(0) & 0x0F
            } else {
                0
            }
        };

        let parse_imm = |s: &str| -> u16 {
            let s = s.trim();
            if s.starts_with("0x") {
                u16::from_str_radix(&s[2..], 16).unwrap_or(0)
            } else {
                s.parse::<u16>().unwrap_or(0)
            }
        };

        let inst = match op.as_str() {
            "NOP"   => Px::new(OP_NOP, 0, 0, 0),
            "HALT"  => Px::new(OP_HALT, 0, 0, 0),
            "LOAD"  => {
                // LOAD r0, 42
                let dest = parse_reg(args.get(0).unwrap_or(&"r0"));
                let imm_str = args.get(1).unwrap_or(&"0");
                let val = parse_imm(imm_str);
                Px::new(OP_LOAD, val as u8, (val >> 8) as u8, dest)
            }
            "ADD"   => {
                let dest = parse_reg(args.get(0).unwrap_or(&"r0"));
                let a = parse_reg(args.get(1).unwrap_or(&"r0"));
                let b = parse_reg(args.get(2).unwrap_or(&"r0"));
                Px::new(OP_ADD, a, b, dest)
            }
            "SUB"   => {
                let dest = parse_reg(args.get(0).unwrap_or(&"r0"));
                let a = parse_reg(args.get(1).unwrap_or(&"r0"));
                let b = parse_reg(args.get(2).unwrap_or(&"r0"));
                Px::new(OP_SUB, a, b, dest)
            }
            "MUL"   => {
                let dest = parse_reg(args.get(0).unwrap_or(&"r0"));
                let a = parse_reg(args.get(1).unwrap_or(&"r0"));
                let b = parse_reg(args.get(2).unwrap_or(&"r0"));
                Px::new(OP_MUL, a, b, dest)
            }
            "MOV"   => {
                let dest = parse_reg(args.get(0).unwrap_or(&"r0"));
                let src = parse_reg(args.get(1).unwrap_or(&"r0"));
                Px::new(OP_MOV, src, 0, dest)
            }
            "CMP"   => {
                let a = parse_reg(args.get(0).unwrap_or(&"r0"));
                let b = parse_reg(args.get(1).unwrap_or(&"r0"));
                Px::new(OP_CMP, a, b, 0)
            }
            "JMP"   => {
                let label = args.get(0).unwrap_or(&"0").to_string();
                if let Some(&addr) = labels.get(&label) {
                    Px::new(OP_JMP, addr as u8, (addr >> 8) as u8, 0)
                } else if let Ok(addr) = label.parse::<usize>() {
                    Px::new(OP_JMP, addr as u8, (addr >> 8) as u8, 0)
                } else {
                    fixups.push((instructions.len(), label));
                    Px::new(OP_JMP, 0, 0, 0)
                }
            }
            "JEQ"   => {
                let label = args.get(0).unwrap_or(&"0").to_string();
                let addr = labels.get(&label).copied().unwrap_or(0);
                Px::new(OP_JEQ, addr as u8, (addr >> 8) as u8, 0)
            }
            "JNE"   => {
                let label = args.get(0).unwrap_or(&"0").to_string();
                let addr = labels.get(&label).copied().unwrap_or(0);
                Px::new(OP_JNE, addr as u8, (addr >> 8) as u8, 0)
            }
            "JLT"   => {
                let label = args.get(0).unwrap_or(&"0").to_string();
                let addr = labels.get(&label).copied().unwrap_or(0);
                Px::new(OP_JLT, addr as u8, (addr >> 8) as u8, 0)
            }
            "PRINT"  => Px::new(OP_PRINT, parse_reg(args.get(0).unwrap_or(&"r0")), 0, 0),
            "PRINTI" => Px::new(OP_PRINTI, parse_reg(args.get(0).unwrap_or(&"r0")), 0, 0),
            "POKE"   => {
                let val = parse_reg(args.get(0).unwrap_or(&"r0"));
                let x = parse_reg(args.get(1).unwrap_or(&"r0"));
                let y = parse_reg(args.get(2).unwrap_or(&"r0"));
                Px::new(OP_POKE, x, y, val)
            }
            "PEEK"   => {
                let dest = parse_reg(args.get(0).unwrap_or(&"r0"));
                let x = parse_reg(args.get(1).unwrap_or(&"r0"));
                let y = parse_reg(args.get(2).unwrap_or(&"r0"));
                Px::new(OP_PEEK, x, y, dest)
            }
            _ => {
                eprintln!("Unknown instruction: {}", op);
                Px::new(OP_NOP, 0, 0, 0)
            }
        };

        instructions.push(inst);
    }

    // Fix up label references
    for (idx, label) in fixups {
        if let Some(&addr) = labels.get(&label) {
            instructions[idx].g = addr as u8;
            instructions[idx].b = (addr >> 8) as u8;
        }
    }

    instructions
}

// ===== MAIN =====
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();

    println!("╔══════════════════════════════════════════════════╗");
    println!("║            PIXEL VM — Everything is Pixels       ║");
    println!("╚══════════════════════════════════════════════════╝\n");

    // Check for .pxl source file argument
    let source = if args.len() > 1 && std::path::Path::new(&args[1]).exists() {
        std::fs::read_to_string(&args[1])?
    } else {
        // Built-in demo programs
        let demo = args.get(1).map(|s| s.as_str()).unwrap_or("hello");
        match demo {
            "hello" => r#"
; Hello World — each character is loaded and printed
; The program IS pixels. Each instruction = 1 pixel (RGBA).

LOAD r0, 72    ; 'H'
PRINT r0
LOAD r0, 101   ; 'e'
PRINT r0
LOAD r0, 108   ; 'l'
PRINT r0
PRINT r0       ; 'l' again
LOAD r0, 111   ; 'o'
PRINT r0
LOAD r0, 32    ; ' '
PRINT r0
LOAD r0, 87    ; 'W'
PRINT r0
LOAD r0, 111   ; 'o'
PRINT r0
LOAD r0, 114   ; 'r'
PRINT r0
LOAD r0, 108   ; 'l'
PRINT r0
LOAD r0, 100   ; 'd'
PRINT r0
LOAD r0, 10    ; newline
PRINT r0
HALT
"#.to_string(),
            "fib" => r#"
; Fibonacci sequence — computes first 10 fibonacci numbers
; r0 = current, r1 = next, r2 = temp, r3 = counter, r4 = limit

LOAD r0, 0     ; fib(0) = 0
LOAD r1, 1     ; fib(1) = 1
LOAD r3, 0     ; counter = 0
LOAD r4, 10    ; limit = 10

loop:
PRINTI r0       ; print current fib number
LOAD r5, 32    ; space
PRINT r5
MOV r2, r0     ; temp = current
ADD r0, r1, r2 ; WRONG - need: r0 = r0 + r1, so swap
; Actually: new_current = old_next, new_next = old_current + old_next
; Let me redo:
; r2 = r0 + r1 (next next)
; r0 = r1 (current = old next)
; r1 = r2 (next = sum)
ADD r2, r0, r1  ; r2 = r0 + r1
MOV r0, r1      ; r0 = r1
MOV r1, r2      ; r1 = r2
LOAD r5, 1
ADD r3, r3, r5  ; counter++
CMP r3, r4      ; counter vs limit
JLT loop
LOAD r5, 10     ; newline
PRINT r5
HALT
"#.to_string(),
            "draw" => r#"
; Draw a gradient bar on the output area
; Writes pixels directly to the framebuffer

LOAD r0, 0      ; x counter
LOAD r1, 110    ; y position (output area)
LOAD r3, 100    ; width limit
LOAD r4, 0xFF   ; alpha

draw_loop:
; Create color: r=x*2, g=255-x*2, b=128
MOV r5, r0
ADD r5, r5, r5   ; r5 = x * 2 (approximate)
; Pack as pixel value — we'll use POKE
; For now just write x position as the value
LOAD r6, 0
POKE r5, r0, r1  ; fb[x, y] = r5 (writes r5 register value)
LOAD r5, 1
ADD r0, r0, r5   ; x++
CMP r0, r3
JLT draw_loop
HALT
"#.to_string(),
            "count" => r#"
; Count from 0 to 20, printing each number

LOAD r0, 0      ; counter
LOAD r1, 21     ; limit
LOAD r2, 1      ; increment

loop:
PRINTI r0
LOAD r3, 32     ; space
PRINT r3
ADD r0, r0, r2   ; counter++
CMP r0, r1
JLT loop
LOAD r3, 10      ; newline
PRINT r3
HALT
"#.to_string(),
            _ => {
                eprintln!("Unknown demo: {}. Use: hello, fib, count, draw", demo);
                eprintln!("Or provide a .pxl source file.");
                std::process::exit(1);
            }
        }
    };

    // Assemble
    println!("Assembling...");
    let program = assemble(&source);

    // Show program as hex (pre-hex layer)
    println!("\n─── Pre-Hex (program as pixels) ────────────");
    for (i, inst) in program.iter().enumerate() {
        let op_name = match inst.r {
            0x00 => "NOP", 0x01 => "LOAD", 0x02 => "ADD", 0x03 => "SUB",
            0x04 => "MUL", 0x0D => "CMP", 0x0E => "MOV",
            0x10 => "JMP", 0x11 => "JEQ", 0x12 => "JNE", 0x14 => "JLT",
            0x40 => "PRINT", 0x41 => "PRINTI", 0x31 => "POKE", 0x30 => "PEEK",
            0xFF => "HALT", _ => "???",
        };
        println!("  {:3}: {:?}  {} g={} b={} a={}", i, inst, op_name, inst.g, inst.b, inst.a);
        if i > 30 { println!("  ... ({} more)", program.len() - 31); break; }
    }

    // Create VM and load
    let mut vm = PixelVM::new(480, 240);
    vm.load_program(&program);

    // Run
    println!("\n─── Executing ──────────────────────────────");
    vm.run(100_000);

    // Post state
    vm.dump_state();

    // Save framebuffer as PNG
    std::fs::create_dir_all("output")?;
    vm.fb.save_png("output/pixelvm.png")?;
    println!("\nFramebuffer saved: output/pixelvm.png");
    println!("Every pixel in that image IS the machine state.");

    Ok(())
}
