// Register HUD - Visual display of register values in framebuffer
// Part of Visual Cognition Phase 1
// Maps registers A-Z to color-coded bar at top of screen

use std::collections::HashMap;

const FB_WIDTH: usize = 640;
const FB_HEIGHT: usize = 480;
const HUD_HEIGHT: usize = 40;
const REGISTERS: &[char] = &['A','B','C','D','E','F','G','H','I','J','K','L','M',
                              'N','O','P','Q','R','S','T','U','V','W','X','Y','Z'];

#[derive(Debug, Clone)]
pub struct Register {
    pub name: char,
    pub value: i32,
    pub color: (u8, u8, u8),
}

impl Register {
    fn new(name: char, index: usize) -> Self {
        // Color based on value intensity
        let hue = (index * 10) as f32 / 360.0;
        let (r, g, b) = hsl_to_rgb(hue, 0.7, 0.5);
        Register { name, value: 0, color: (r, g, b) }
    }

    fn update_color(&mut self) {
        // Dynamic color based on value magnitude
        let intensity = (self.value.abs() as f32 / 100.0).min(1.0);
        let hue = if self.value >= 0 { 0.3 } else { 0.0 }; // Green for positive, red for negative
        let (r, g, b) = hsl_to_rgb(hue, 0.8, 0.3 + intensity * 0.4);
        self.color = (r, g, b);
    }
}

fn hsl_to_rgb(h: f32, s: f32, l: f32) -> (u8, u8, u8) {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((h * 6.0) % 2.0 - 1.0).abs());
    let m = l - c / 2.0;

    let (r, g, b) = if h < 1.0/6.0 { (c, x, 0.0) }
    else if h < 2.0/6.0 { (x, c, 0.0) }
    else if h < 3.0/6.0 { (0.0, c, x) }
    else if h < 4.0/6.0 { (0.0, x, c) }
    else if h < 5.0/6.0 { (x, 0.0, c) }
    else { (c, 0.0, x) };

    ((r + m) as u8 * 255, (g + m) as u8 * 255, (b + m) as u8 * 255)
}

pub struct RegisterHUD {
    registers: HashMap<char, Register>,
    framebuffer: Vec<u8>,
}

impl RegisterHUD {
    pub fn new() -> Self {
        let mut registers = HashMap::new();
        for (i, &name) in REGISTERS.iter().enumerate() {
            registers.insert(name, Register::new(name, i));
        }

        RegisterHUD {
            registers,
            framebuffer: vec![0u8; FB_WIDTH * FB_HEIGHT * 4],
        }
    }

    pub fn set(&mut self, name: char, value: i32) {
        if let Some(reg) = self.registers.get_mut(&name) {
            reg.value = value;
            reg.update_color();
        }
    }

    pub fn get(&self, name: char) -> i32 {
        self.registers.get(&name).map(|r| r.value).unwrap_or(0)
    }

    /// Render HUD to framebuffer top rows
    pub fn render(&mut self) -> &[u8] {
        // Clear framebuffer to dark teal
        for pixel in self.framebuffer.chunks_exact_mut(4) {
            pixel[0] = 20;  // R
            pixel[1] = 60;  // G
            pixel[2] = 80;  // B
            pixel[3] = 255; // A
        }

        // Draw register bars
        let bar_width = FB_WIDTH / REGISTERS.len();
        let bar_spacing = 2;

        for (i, &name) in REGISTERS.iter().enumerate() {
            let reg = self.registers.get(&name).unwrap();
            let x_start = i * bar_width + bar_spacing;
            let x_end = (i + 1) * bar_width - bar_spacing;

            // Bar height based on value (clamped to HUD_HEIGHT)
            let bar_height = ((reg.value.abs() as f32 / 50.0).min(1.0) * (HUD_HEIGHT - 10) as f32) as usize;
            let y_start = HUD_HEIGHT - bar_height - 2;
            let y_end = HUD_HEIGHT - 2;

            // Draw bar
            for y in y_start..y_end {
                for x in x_start..x_end.min(FB_WIDTH) {
                    let idx = (y * FB_WIDTH + x) * 4;
                    if idx + 3 < self.framebuffer.len() {
                        self.framebuffer[idx] = reg.color.0;
                        self.framebuffer[idx + 1] = reg.color.1;
                        self.framebuffer[idx + 2] = reg.color.2;
                        self.framebuffer[idx + 3] = 255;
                    }
                }
            }

            // Draw register name below bar (simplified - just colored dot)
            let label_y = HUD_HEIGHT - 4;
            let label_x = x_start + (bar_width - bar_spacing) / 2;
            if label_x < FB_WIDTH {
                let idx = (label_y * FB_WIDTH + label_x) * 4;
                if idx + 3 < self.framebuffer.len() {
                    self.framebuffer[idx] = 255;
                    self.framebuffer[idx + 1] = 255;
                    self.framebuffer[idx + 2] = 255;
                }
            }
        }

        // Draw separator line
        for x in 0..FB_WIDTH {
            let idx = (HUD_HEIGHT * FB_WIDTH + x) * 4;
            if idx + 3 < self.framebuffer.len() {
                self.framebuffer[idx] = 100;
                self.framebuffer[idx + 1] = 150;
                self.framebuffer[idx + 2] = 180;
                self.framebuffer[idx + 3] = 255;
            }
        }

        &self.framebuffer
    }

    pub fn save_png(&self, path: &str) -> Result<(), String> {
        // Simple PPM output for testing (can convert to PNG with ImageMagick)
        let ppm_path = path.replace(".png", ".ppm");
        let mut output = format!("P6\n{} {}\n255\n", FB_WIDTH, FB_HEIGHT);

        for pixel in self.framebuffer.chunks_exact(4) {
            output.push(pixel[0] as char);
            output.push(pixel[1] as char);
            output.push(pixel[2] as char);
        }

        std::fs::write(&ppm_path, output.into_bytes()).map_err(|e| e.to_string())?;
        println!("Saved HUD to {}", ppm_path);
        Ok(())
    }

    /// Generate ASCII representation of current state
    pub fn to_ascii(&self) -> String {
        let mut result = String::new();
        result.push_str("╔════════════════════════════════════════════════════════════╗\n");
        result.push_str("║                    REGISTER HUD                            ║\n");
        result.push_str("╠════════════════════════════════════════════════════════════╣\n");
        result.push_str("║  ");

        for (i, &name) in REGISTERS.iter().enumerate() {
            if i == 13 { result.push_str("\n║  "); }
            let reg = self.registers.get(&name).unwrap();
            let bar = match reg.value {
                v if v > 50 => "████",
                v if v > 25 => "███░",
                v if v > 10 => "██░░",
                v if v > 0 => "█░░░",
                v if v < -50 => "▄▄▄▄",
                v if v < -25 => "▄▄▄░",
                v if v < -10 => "▄▄░░",
                v if v < 0 => "▄░░░",
                _ => "░░░░",
            };
            result.push_str(&format!("{}:{}{} ", name, bar, reg.value.abs() % 100));
        }

        result.push_str("\n╚════════════════════════════════════════════════════════════╝\n");
        result
    }
}

/// Simple VM that updates registers and renders HUD
pub struct HUDVM {
    hud: RegisterHUD,
    stack: Vec<i32>,
    ip: usize,
    code: Vec<String>,
}

impl HUDVM {
    pub fn new(code: &str) -> Self {
        HUDVM {
            hud: RegisterHUD::new(),
            stack: Vec::new(),
            ip: 0,
            code: code.split_whitespace().map(|s| s.to_string()).collect(),
        }
    }

    pub fn step(&mut self) -> bool {
        if self.ip >= self.code.len() { return false; }

        let token = &self.code[self.ip];
        self.ip += 1;

        // Parse token
        if let Ok(n) = token.parse::<i32>() {
            self.stack.push(n);
        } else {
            match token.as_str() {
                "+" => { let b = self.stack.pop().unwrap_or(0); let a = self.stack.pop().unwrap_or(0); self.stack.push(a + b); }
                "-" => { let b = self.stack.pop().unwrap_or(0); let a = self.stack.pop().unwrap_or(0); self.stack.push(a - b); }
                "*" => { let b = self.stack.pop().unwrap_or(0); let a = self.stack.pop().unwrap_or(0); self.stack.push(a * b); }
                "/" => { let b = self.stack.pop().unwrap_or(0); let a = self.stack.pop().unwrap_or(0); self.stack.push(a / b.max(1)); }
                "." => { /* print - ignore */ }
                ":" => { if let Some(&v) = self.stack.last() { self.stack.push(v); } }
                "@" => return false, // halt
                _ => {
                    // Register operations
                    let c = token.chars().next().unwrap();
                    if c.is_ascii_lowercase() {
                        // Store to register
                        let val = self.stack.pop().unwrap_or(0);
                        self.hud.set(c.to_ascii_uppercase(), val);
                    } else if c.is_ascii_uppercase() {
                        // Load from register
                        self.stack.push(self.hud.get(c));
                    }
                }
            }
        }

        true
    }

    pub fn run(&mut self) -> i32 {
        while self.step() {}
        self.stack.last().copied().unwrap_or(0)
    }

    pub fn hud(&self) -> &RegisterHUD {
        &self.hud
    }

    pub fn hud_mut(&mut self) -> &mut RegisterHUD {
        &mut self.hud
    }
}

fn main() {
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║              REGISTER HUD - Visual Cognition            ║");
    println!("╠══════════════════════════════════════════════════════════╣");
    println!("║  Phase 1: Color-coded register display                  ║");
    println!("╚══════════════════════════════════════════════════════════╝");
    println!();

    // Test programs
    let tests = vec![
        ("Counter", "5 a 10 b 15 c A B + d @"),
        ("Fibonacci", "0 a 1 b 10 n A B + c b a B c @"),
        ("Factorial", "5 n 1 r N 1 - n N R n * r ? < @"),
    ];

    for (name, code) in tests {
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("[TEST: {}]", name);
        println!("[CODE] {}", code);

        let mut vm = HUDVM::new(code);
        let result = vm.run();

        println!("[RESULT] {}", result);
        println!("{}", vm.hud().to_ascii());
    }

    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║                    HUD FEATURES                          ║");
    println!("╠══════════════════════════════════════════════════════════╣");
    println!("║  ✅ 26 registers (A-Z) with color-coded values           ║");
    println!("║  ✅ Visual bar height based on magnitude                 ║");
    println!("║  ✅ Positive = green, Negative = red                     ║");
    println!("║  ✅ ASCII representation for terminal output             ║");
    println!("║  ✅ Framebuffer rendering (640x480)                      ║");
    println!("╚══════════════════════════════════════════════════════════╝");
}
