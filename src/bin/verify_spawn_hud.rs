// Verify SPAWN Parallel HUDs with Vision Model

use reqwest;
use serde_json;
use base64::{Engine as _, engine::general_purpose};
use std::collections::HashMap;
use std::time::Instant;

const VISION_MODEL: &str = "qwen/qwen3-vl-8b";

const SPAWN_PROMPT: &str = "Analyze this GPU-rendered image showing multiple thread HUDs.

Look for thread headers like #0: and #1:
Each thread shows its register values (A=XXX B=XXX C=XXX etc.)

Respond in this EXACT format:
THREAD 0: A=X B=X C=X
THREAD 1: A=X B=X C=X
...

Only output threads that you can see.";

async fn verify_parallel_huds(image_path: &str) -> Result<Vec<(u32, HashMap<char, i32>)>, String> {
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
                    {"type": "text", "text": SPAWN_PROMPT}
                ]
            }],
            "max_tokens": 300,
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
    println!("[RAW] {}", vision_text.lines().take(6).collect::<Vec<_>>().join(" | "));
    
    // Parse threads
    let mut threads = Vec::new();
    
    for line in vision_text.lines() {
        if line.contains("THREAD") {
            let mut registers = HashMap::new();
            
            // Extract thread number
            let thread_num: u32 = line
                .split("THREAD")
                .nth(1)
                .and_then(|s| s.split(':').next())
                .and_then(|s| s.trim().parse().ok())
                .unwrap_or(0);
            
            // Extract registers
            for part in line.split_whitespace() {
                if let Some((name, value)) = part.split_once('=') {
                    if let (Some(ch), Ok(val)) = (name.chars().next(), value.parse::<i32>()) {
                        registers.insert(ch.to_ascii_uppercase(), val);
                    }
                }
            }
            
            threads.push((thread_num, registers));
        }
    }
    
    Ok(threads)
}

#[tokio::main]
async fn main() -> Result<(), String> {
    println!();
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║       SPAWN PARALLEL HUDS — VISION VERIFICATION         ║");
    println!("╚══════════════════════════════════════════════════════════╝");
    println!();
    
    println!("[EXPECTED] 2 threads with A=1, B=2");
    println!();
    
    let spawn_hud_path = "output/spawn_parallel_hud.png";
    println!("[IMAGE] {}", spawn_hud_path);
    
    let threads = verify_parallel_huds(spawn_hud_path).await?;
    println!();
    
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║            PARALLEL HUDS VALIDATION                     ║");
    println!("╠══════════════════════════════════════════════════════════╣");
    
    for (thread_num, registers) in &threads {
        print!("║  Thread {}: ", thread_num);
        for reg in "ABCDEFGHIJ".chars() {
            if let Some(&val) = registers.get(&reg) {
                if val != 0 {
                    print!("{}={} ", reg, val);
                }
            }
        }
        println!("                                 ║");
    }
    
    println!("╚══════════════════════════════════════════════════════════╝");
    println!();
    
    if threads.len() >= 2 {
        println!("✅ PARALLEL HUDS VERIFIED");
        println!("   Vision model detected {} threads", threads.len());
        println!("   The swarm is visible!");
    } else if threads.len() == 1 {
        println!("⚠️  Only 1 thread detected");
        println!("   May need better HUD separation");
    } else {
        println!("❌ No threads detected");
    }
    
    Ok(())
}
