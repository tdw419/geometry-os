// Verify GPU-Native HUD with Vision Model
// Reads GPU-rendered HUD and validates register values

use reqwest;
use serde_json;
use base64::{Engine as _, engine::general_purpose};
use std::collections::HashMap;
use std::time::Instant;

const VISION_MODEL: &str = "qwen/qwen3-vl-8b";

const HUD_PROMPT: &str = "Read the register values from this GPU-rendered HUD image.

Look for:
- REGISTERS: followed by register values (A=X B=X C=X ...)
- The values should be 3-digit numbers

Respond in this EXACT format:
REGISTERS: A=X B=X C=X
STACK: X
IP: X
SP: X

Only output the format above.";

async fn verify_gpu_hud(image_path: &str) -> Result<HashMap<char, i32>, String> {
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
    
    // Parse registers
    let mut registers = HashMap::new();
    
    for line in vision_text.lines() {
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
    
    Ok(registers)
}

#[tokio::main]
async fn main() -> Result<(), String> {
    println!();
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║         GPU-NATIVE HUD — VISION VERIFICATION            ║");
    println!("╚══════════════════════════════════════════════════════════╝");
    println!();
    
    // Expected values from program: 9 7 - a 5 2 * b A B + c @
    // A=2, B=10, C=12
    let expected = vec![('A', 2), ('B', 10), ('C', 12)];
    
    println!("[EXPECTED] A=2, B=10, C=12");
    println!();
    
    // Verify GPU-rendered HUD
    let gpu_hud_path = "output/gpu_native_hud.png";
    println!("[GPU HUD] {}", gpu_hud_path);
    
    let registers = verify_gpu_hud(gpu_hud_path).await?;
    println!();
    
    // Display results
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║            GPU-NATIVE HUD VALIDATION                    ║");
    println!("╠══════════════════════════════════════════════════════════╣");
    
    let mut all_match = true;
    for (reg, expected_val) in expected {
        let actual = registers.get(&reg).unwrap_or(&0);
        let status = if *actual == expected_val { "✓" } else { "✗" };
        if *actual != expected_val { all_match = false; }
        println!("║  Register {}: expected {:3} got {:3} {:16} ║", 
            reg, expected_val, actual, status);
    }
    
    println!("╚══════════════════════════════════════════════════════════╝");
    println!();
    
    if all_match {
        println!("✅ GPU-NATIVE HUD VERIFIED");
        println!("   Vision model successfully read shader-rendered registers.");
        println!("   The RTX 5090 now speaks directly to qwen3-vl-8b!");
    } else {
        println!("⚠️  Vision mismatch");
        println!("   Registers detected:");
        for (reg, val) in &registers {
            println!("     {} = {}", reg, val);
        }
    }
    
    Ok(())
}
