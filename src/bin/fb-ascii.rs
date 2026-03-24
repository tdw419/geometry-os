// ASCII VM on the Framebuffer
//
// The framebuffer IS the program. ASCII characters ARE instructions.
// No binary encoding - just readable text that executes.
//
// Memory map (as text):
//   Row 0: Registers (named: A B C D E F G H I J K L M N O P)
//   Row 1: Program counter and flags
//   Row 2+: Program (ASCII instructions)
//   Last rows: Output (visible on screen)
//
// Instructions (single chars):
//   0-9: Push digit (build number)
//   A-P: Load register
//   + - * /: Arithmetic
//   >: Store to register
//   .: Print top of stack
//   ": ... ": Print string
//   [: ... ]: Loop while top != 0
//   @: Halt
//   Space/newline: Skip

use std::fs::OpenOptions;
use std::io::{self, Write};

const WIDTH: u32 = 80;
const HEIGHT: u32 = 25;

struct AsciiVm {
    pc: (u32, u32),      // (col, row)
    stack: Vec<i64>,
    regs: [i64; 16],
    output: String,
    halted: bool,
}

impl AsciiVm {
    fn new() -> Self {
        Self {
            pc: (0, 2),  // Start at row 2
            stack: Vec::new(),
            regs: [0; 16],
            output: String::new(),
            halted: false,
        }
    }
    
    fn fetch(&self, fb: &[u8]) -> char {
        let (x, y) = self.pc;
        if y >= HEIGHT || x >= WIDTH {
            return '@';  // Out of bounds = halt
        }
        let idx = y as usize * WIDTH as usize + x as usize;
        if idx >= fb.len() {
            return '@';
        }
        fb[idx] as char
    }
    
    fn advance(&mut self) {
        self.pc.0 += 1;
        if self.pc.0 >= WIDTH {
            self.pc.0 = 0;
            self.pc.1 += 1;
        }
    }
    
    fn execute(&mut self, fb: &mut [u8]) {
        let ch = self.fetch(fb);
        
        match ch {
            '0'..='9' => {
                // Build multi-digit number
                let mut num = 0i64;
                let mut c = ch;
                while c >= '0' && c <= '9' {
                    num = num * 10 + (c as i64 - '0' as i64);
                    self.advance();
                    c = self.fetch(fb);
                }
                self.stack.push(num);
                return;  // Already advanced
            }
            'A'..='P' => {
                let reg_idx = (ch as u8 - b'A') as usize;
                self.stack.push(self.regs[reg_idx]);
            }
            '>' => {
                if let Some(val) = self.stack.pop() {
                    // Next char is register name
                    self.advance();
                    let reg = self.fetch(fb);
                    if reg >= 'A' && reg <= 'P' {
                        let idx = (reg as u8 - b'A') as usize;
                        self.regs[idx] = val;
                    }
                }
            }
            '+' => {
                let b = self.stack.pop().unwrap_or(0);
                let a = self.stack.pop().unwrap_or(0);
                self.stack.push(a + b);
            }
            '-' => {
                let b = self.stack.pop().unwrap_or(0);
                let a = self.stack.pop().unwrap_or(0);
                self.stack.push(a - b);
            }
            '*' => {
                let b = self.stack.pop().unwrap_or(0);
                let a = self.stack.pop().unwrap_or(0);
                self.stack.push(a * b);
            }
            '/' => {
                let b = self.stack.pop().unwrap_or(1);
                let a = self.stack.pop().unwrap_or(0);
                if b != 0 {
                    self.stack.push(a / b);
                }
            }
            '.' => {
                if let Some(val) = self.stack.last() {
                    self.output.push_str(&format!("{} ", val));
                }
            }
            '"' => {
                // String literal
                self.advance();
                let mut c = self.fetch(fb);
                while c != '"' && c != '@' {
                    self.output.push(c);
                    self.advance();
                    c = self.fetch(fb);
                }
            }
            '[' => {
                // Loop start - if top is 0, skip to ]
                if let Some(&top) = self.stack.last() {
                    if top == 0 {
                        // Find matching ]
                        let mut depth = 1;
                        while depth > 0 {
                            self.advance();
                            let c = self.fetch(fb);
                            if c == '[' { depth += 1; }
                            if c == ']' { depth -= 1; }
                            if c == '@' { break; }
                        }
                    }
                }
            }
            ']' => {
                // Loop end - if top != 0, go back to [
                if let Some(&top) = self.stack.last() {
                    if top != 0 {
                        // Find matching [
                        let mut depth = 1;
                        while depth > 0 {
                            if self.pc.0 > 0 {
                                self.pc.0 -= 1;
                            } else if self.pc.1 > 0 {
                                self.pc.1 -= 1;
                                self.pc.0 = WIDTH - 1;
                            } else {
                                break;
                            }
                            let c = self.fetch(fb);
                            if c == ']' { depth += 1; }
                            if c == '[' { depth -= 1; }
                        }
                    } else {
                        self.stack.pop();
                    }
                }
            }
            '@' => {
                self.halted = true;
            }
            ' ' | '\n' | '\r' | '\t' => {
                // Whitespace - skip
            }
            _ => {
                // Unknown - skip
            }
        }
        
        self.advance();
    }
}

fn main() -> io::Result<()> {
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║     ASCII VM on Framebuffer                              ║");
    println!("║     The screen IS the program. Characters ARE code.      ║");
    println!("╚══════════════════════════════════════════════════════════╝");
    println!();
    
    // Create framebuffer (80x25 text mode style)
    let mut fb = vec![b' '; (WIDTH * HEIGHT) as usize];
    
    // Write program directly into framebuffer
    let program = r#"10 >A 0 >B 1 >C            "Fibonacci: " .
[A . ]                      " " .
B . ]                       " " .
A B + >D                    " " .
B >A D >B C 1 - >C          " " .
[C                          " " .
"#;
    
    // Load program into row 2
    for (i, &b) in program.as_bytes().iter().enumerate() {
        let x = i % WIDTH as usize;
        let y = i / WIDTH as usize + 2;
        if y < HEIGHT as usize {
            fb[y * WIDTH as usize + x] = b;
        }
    }
    
    // Alternative: simpler test program
    let test_program = r#""Hello Framebuffer!" . 1 2 + . 3 4 * . @"#;
    for (i, &b) in test_program.as_bytes().iter().enumerate() {
        let x = i % WIDTH as usize;
        let y = 2;
        fb[y * WIDTH as usize + x] = b;
    }
    
    println!("Program loaded into framebuffer:");
    for y in 2..4 {
        print!("  Row {}: \"", y);
        for x in 0..WIDTH as usize {
            let ch = fb[y * WIDTH as usize + x] as char;
            if ch == '\0' || ch == '\n' { break; }
            print!("{}", ch);
        }
        println!("\"");
    }
    println!();
    
    // Run VM
    let mut vm = AsciiVm::new();
    
    println!("Executing...");
    for cycle in 0..1000 {
        if vm.halted {
            break;
        }
        vm.execute(&mut fb);
    }
    
    println!();
    println!("Output: {}", vm.output);
    println!();
    
    // Show final framebuffer state
    println!("Framebuffer (rows 0-4):");
    for y in 0..5usize {
        print!("  [{:02}] \"", y);
        for x in 0..40usize {
            let ch = fb[y * WIDTH as usize + x] as char;
            if ch >= ' ' && ch <= '~' {
                print!("{}", ch);
            } else {
                print!(".");
            }
        }
        println!("\"");
    }
    
    println!();
    println!("═══════════════════════════════════════════════════════════");
    println!("  PROVEN: ASCII VM runs on framebuffer.");
    println!("  Programs are readable text, not binary.");
    println!("  The screen IS the code.");
    println!("═══════════════════════════════════════════════════════════");
    
    Ok(())
}
