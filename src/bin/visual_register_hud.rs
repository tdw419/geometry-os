// Visual Register HUD - Complete Pipeline
// Executes program, writes registers to framebuffer, vision reads them

use reqwest;
use serde_json;
use base64::{Engine as _, engine::general_purpose};
use image::{ImageBuffer, Rgba, DynamicImage};
use std::collections::HashMap;
use std::time::Instant;

const VISION_MODEL: &str = "qwen/qwen3-vl-8b";
const WIDTH: u32 = 640;
const HEIGHT: u32 = 480;

const HUD_PROMPT: &str = "Read the register values from this image.

The top section shows:
- REGISTERS: A=X B=X C=X (register name = value)
- STACK: [values] (stack contents)
- IP: X (instruction pointer)
- SP: X (stack pointer)

Extract the EXACT values shown. Respond in this format:
REGISTERS: A=X B=X C=X
STACK: [values]
IP: X
SP: X

Only output the format above.";

#[derive(Debug, Default)]
struct VMState {
    registers: HashMap<char, i32>,
    stack: Vec<i32>,
    ip: usize,
}

fn execute_program(code: &str) -> VMState {
    let mut state = VMState::default();
    let tokens: Vec<&str> = code.split_whitespace().collect();
    
    while state.ip < tokens.len() {
        let token = tokens[state.ip];
        
        match token {
            // Push number
            n if n.parse::<i32>().is_ok() => {
                state.stack.push(n.parse().unwrap());
            }
            
            // Register store (a, b, c, etc.)
            reg if reg.len() == 1 && reg.chars().next().unwrap().is_ascii_lowercase() => {
                let reg_name = reg.chars().next().unwrap().to_ascii_uppercase();
                if let Some(value) = state.stack.last().copied() {
                    state.registers.insert(reg_name, value);
                }
            }
            
            // Register load (A, B, C, etc.)
            reg if reg.len() == 1 && reg.chars().next().unwrap().is_ascii_uppercase() => {
                let reg_name = reg.chars().next().unwrap();
                if let Some(&value) = state.registers.get(&reg_name) {
                    state.stack.push(value);
                }
            }
            
            // Arithmetic
            "+" | "add" | "." => {
                if state.stack.len() >= 2 {
                    let b = state.stack.pop().unwrap();
                    let a = state.stack.pop().unwrap();
                    state.stack.push(a + b);
                }
            }
            "-" | "sub" => {
                if state.stack.len() >= 2 {
                    let b = state.stack.pop().unwrap();
                    let a = state.stack.pop().unwrap();
                    state.stack.push(a - b);
                }
            }
            "*" | "mul" => {
                if state.stack.len() >= 2 {
                    let b = state.stack.pop().unwrap();
                    let a = state.stack.pop().unwrap();
                    state.stack.push(a * b);
                }
            }
            "/" | "div" => {
                if state.stack.len() >= 2 {
                    let b = state.stack.pop().unwrap();
                    let a = state.stack.pop().unwrap();
                    if b != 0 {
                        state.stack.push(a / b);
                    }
                }
            }
            
            // Halt
            "@" => break,
            
            _ => {}
        }
        
        state.ip += 1;
    }
    
    state
}

fn draw_text(img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, text: &str, x: u32, y: u32, color: Rgba<u8>) {
    // Simple 5x7 bitmap font for digits and letters
    let font: HashMap<char, Vec<u8>> = [
        ('0', vec![0x3E, 0x51, 0x49, 0x45, 0x3E]), // 0
        ('1', vec![0x42, 0x7F, 0x40, 0x00, 0x00]), // 1
        ('2', vec![0x62, 0x51, 0x49, 0x49, 0x46]), // 2
        ('3', vec![0x22, 0x49, 0x49, 0x49, 0x36]), // 3
        ('4', vec![0x18, 0x14, 0x12, 0x7F, 0x10]), // 4
        ('5', vec![0x27, 0x45, 0x45, 0x45, 0x39]), // 5
        ('6', vec![0x3E, 0x49, 0x49, 0x49, 0x32]), // 6
        ('7', vec![0x01, 0x71, 0x09, 0x05, 0x03]), // 7
        ('8', vec![0x36, 0x49, 0x49, 0x49, 0x36]), // 8
        ('9', vec![0x26, 0x49, 0x49, 0x49, 0x3E]), // 9
        ('A', vec![0x7E, 0x11, 0x11, 0x11, 0x7E]), // A
        ('B', vec![0x7F, 0x49, 0x49, 0x49, 0x36]), // B
        ('C', vec![0x3E, 0x41, 0x41, 0x41, 0x22]), // C
        ('D', vec![0x7F, 0x41, 0x41, 0x22, 0x1C]), // D
        ('E', vec![0x7F, 0x49, 0x49, 0x49, 0x41]), // E
        ('F', vec![0x7F, 0x09, 0x09, 0x09, 0x01]), // F
        ('G', vec![0x3E, 0x41, 0x49, 0x49, 0x7A]), // G
        ('H', vec![0x7F, 0x08, 0x08, 0x08, 0x7F]), // H
        ('I', vec![0x41, 0x7F, 0x41, 0x00, 0x00]), // I
        ('J', vec![0x20, 0x40, 0x41, 0x3F, 0x01]), // J
        ('K', vec![0x7F, 0x08, 0x14, 0x22, 0x41]), // K
        ('L', vec![0x7F, 0x40, 0x40, 0x40, 0x40]), // L
        ('M', vec![0x7F, 0x02, 0x0C, 0x02, 0x7F]), // M
        ('N', vec![0x7F, 0x04, 0x08, 0x10, 0x7F]), // N
        ('O', vec![0x3E, 0x41, 0x41, 0x41, 0x3E]), // O
        ('P', vec![0x7F, 0x09, 0x09, 0x09, 0x06]), // P
        ('Q', vec![0x3E, 0x41, 0x51, 0x21, 0x5E]), // Q
        ('R', vec![0x7F, 0x09, 0x19, 0x29, 0x46]), // R
        ('S', vec![0x26, 0x49, 0x49, 0x49, 0x32]), // S
        ('T', vec![0x01, 0x01, 0x7F, 0x01, 0x01]), // T
        ('U', vec![0x3F, 0x40, 0x40, 0x40, 0x3F]), // U
        ('V', vec![0x1F, 0x20, 0x40, 0x20, 0x1F]), // V
        ('W', vec![0x3F, 0x40, 0x38, 0x40, 0x3F]), // W
        ('X', vec![0x63, 0x14, 0x08, 0x14, 0x63]), // X
        ('Y', vec![0x07, 0x08, 0x70, 0x08, 0x07]), // Y
        ('Z', vec![0x61, 0x51, 0x49, 0x45, 0x43]), // Z
        ('=', vec![0x00, 0x7F, 0x00, 0x7F, 0x00]), // =
        (' ', vec![0x00, 0x00, 0x00, 0x00, 0x00]), // space
        (':', vec![0x00, 0x36, 0x36, 0x00, 0x00]), // :
        ('[', vec![0x3E, 0x41, 0x41, 0x41, 0x3E]), // [ (same as 0)
        (']', vec![0x3E, 0x41, 0x41, 0x41, 0x3E]), // ]
        (',', vec![0x00, 0x80, 0x6C, 0x00, 0x00]), // ,
    ].iter().cloned().collect();
    
    let mut cursor_x = x;
    for ch in text.chars() {
        if let Some(bitmap) = font.get(&ch) {
            for (col, byte) in bitmap.iter().enumerate() {
                for row in 0..7 {
                    if (byte >> row) & 1 != 0 {
                        let px = cursor_x + col as u32;
                        let py = y + row;
                        if px < WIDTH && py < HEIGHT {
                            img.put_pixel(px, py, color);
                        }
                    }
                }
            }
        }
        cursor_x += 6;
    }
}

fn render_hud_to_framebuffer(state: &VMState) -> ImageBuffer<Rgba<u8>, Vec<u8>> {
    let mut img = ImageBuffer::from_pixel(WIDTH, HEIGHT, Rgba([20, 30, 40, 255]));
    
    // Header
    draw_text(&mut img, "REGISTERS:", 20, 20, Rgba([0, 200, 255, 255]));
    
    // Register values
    let mut x = 20;
    let mut y = 50;
    for reg in "ABCDEFGHIJ".chars() {
        let value = state.registers.get(&reg).unwrap_or(&0);
        let text = format!("{}={} ", reg, value);
        draw_text(&mut img, &text, x, y, Rgba([255, 255, 255, 255]));
        x += 60;
        if x > WIDTH - 80 {
            x = 20;
            y += 20;
        }
    }
    
    // Stack
    y += 40;
    draw_text(&mut img, "STACK:", 20, y, Rgba([0, 200, 255, 255]));
    y += 20;
    let stack_str: Vec<String> = state.stack.iter().map(|v| v.to_string()).collect();
    draw_text(&mut img, &format!("[{}]", stack_str.join(",")), 20, y, Rgba([255, 255, 255, 255]));
    
    // IP and SP
    y += 40;
    draw_text(&mut img, "IP:", 20, y, Rgba([0, 200, 255, 255]));
    draw_text(&mut img, &state.ip.to_string(), 60, y, Rgba([255, 255, 255, 255]));
    draw_text(&mut img, "SP:", 120, y, Rgba([0, 200, 255, 255]));
    draw_text(&mut img, &state.stack.len().to_string(), 160, y, Rgba([255, 255, 255, 255]));
    
    img
}

async fn vision_read_hud(image_path: &str) -> Result<String, String> {
    let client = reqwest::Client::new();
    let image_data = std::fs::read(image_path).map_err(|e| e.to_string())?;
    let base64_image = general_purpose::STANDARD.encode(&image_data);
    
    let start = Instant::now();
    let response = client
        .post("http://localhost:1234/v1/chat/completions")
        .json(&serde_json::json!({
            "model": VISION_MODEL,
            "messages": [{
                "role": "user",
                "content": [
                    {"type": "image_url", "image_url": {"url": format!("data:image/png;base64,{}", base64_image)}},
                    {"type": "text", "text": HUD_PROMPT}
                ]
            }],
            "max_tokens": 200,
            "temperature": 0.0
        }))
        .timeout(std::time::Duration::from_secs(90))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    
    let json: serde_json::Value = response.json().await.map_err(|e| e.to_string())?;
    let vision_text = json["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("ERROR")
        .to_string();
    
    println!("[VISION] {}ms", start.elapsed().as_millis());
    println!("[RAW] {}", vision_text.lines().take(4).collect::<Vec<_>>().join(" | "));
    
    Ok(vision_text)
}

fn parse_vision_response(text: &str) -> HashMap<char, i32> {
    let mut registers = HashMap::new();
    
    for line in text.lines() {
        if line.contains("REGISTERS:") {
            let rest = line.split("REGISTERS:").nth(1).unwrap_or("");
            for part in rest.split_whitespace() {
                if let Some((name, value)) = part.split_once('=') {
                    if let (Some(ch), Ok(val)) = (name.chars().next(), value.parse::<i32>()) {
                        registers.insert(ch.to_ascii_uppercase(), val);
                    }
                }
            }
        }
    }
    
    registers
}

#[tokio::main]
async fn main() -> Result<(), String> {
    println!();
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║          VISUAL REGISTER HUD - BIOS MODE                ║");
    println!("╠══════════════════════════════════════════════════════════╣");
    println!("║  Vision: {} (The Eye of the Kernel)      ║", VISION_MODEL);
    println!("╚══════════════════════════════════════════════════════════╝");
    println!();
    
    // Test program: 9 7 - a 5 2 * b A B + c @
    // Expected: a=2, b=10, c=12
    let test_program = "9 7 - a 5 2 * b A B + c @";
    println!("[PROGRAM] {}", test_program);
    println!("[EXPECT]  a=2, b=10, c=12");
    println!();
    
    // Step 1: Execute program
    println!("[EXECUTE] Running program...");
    let state = execute_program(test_program);
    println!("[STATE]   A={:?} B={:?} C={:?}", 
        state.registers.get(&'A'),
        state.registers.get(&'B'),
        state.registers.get(&'C'));
    println!("[STACK]   {:?}", state.stack);
    println!();
    
    // Step 2: Render HUD to framebuffer
    println!("[RENDER]  Writing registers to framebuffer...");
    let img = render_hud_to_framebuffer(&state);
    let output_path = "/home/jericho/zion/projects/ascii_world/gpu/output/hud_framebuffer.png";
    img.save(output_path).map_err(|e| e.to_string())?;
    println!("[OUTPUT]  {}", output_path);
    println!();
    
    // Step 3: Vision reads framebuffer
    println!("[VISION]  qwen3-vl-8b reading framebuffer...");
    let vision_text = vision_read_hud(output_path).await?;
    println!();
    
    // Step 4: Parse vision response
    let vision_registers = parse_vision_response(&vision_text);
    
    // Step 5: Display HUD
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║            VISUAL REGISTER HUD (BIOS MODE)              ║");
    println!("╠══════════════════════════════════════════════════════════╣");
    print!("║  ");
    for reg in "ABCDEFGHIJ".chars() {
        let value = vision_registers.get(&reg).unwrap_or(&0);
        print!("{}:{:3}  ", reg, value);
    }
    println!("║");
    println!("╚══════════════════════════════════════════════════════════╝");
    println!();
    
    // Step 6: Validation
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║                  VALIDATION CHECK                       ║");
    println!("╠══════════════════════════════════════════════════════════╣");
    
    let expected = vec![('A', 2), ('B', 10), ('C', 12)];
    let mut all_match = true;
    
    for (reg, expected_val) in expected {
        let actual = vision_registers.get(&reg).unwrap_or(&0);
        let status = if *actual == expected_val { "✓" } else { "✗" };
        if *actual != expected_val { all_match = false; }
        println!("║  Register {}: expected {:3} got {:3} {:16} ║", 
            reg, expected_val, actual, status);
    }
    
    println!("╚══════════════════════════════════════════════════════════╝");
    println!();
    
    if all_match {
        println!("✅ VISUAL PROPRIOCEPTION ACHIEVED");
        println!("   The kernel can now see its own register state.");
        println!("   Vision model successfully read: A=2, B=10, C=12");
    } else {
        println!("⚠️  Vision mismatch - registers detected:");
        for (reg, val) in &vision_registers {
            println!("   {} = {}", reg, val);
        }
    }
    
    Ok(())
}
