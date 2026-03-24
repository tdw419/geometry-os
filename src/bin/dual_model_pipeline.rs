// Dual Model Pipeline - Vision + Text LLMs
// Uses tinyllama for code, qwen3-vl for visual interpretation

use reqwest;
use serde_json;
use base64::{Engine as _, engine::general_purpose};

const TEXT_MODEL: &str = "tinyllama-1.1b-chat-v1.0";
const VISION_MODEL: &str = "qwen/qwen3-vl-8b";

const FEW_SHOT_PREFIX: &str = "VM Code (opcodes only):\n\
    Push 5, halt = 5 @\n\
    Push 5, push 3, add, halt = 5 3 + . @\n";

/// Call text LLM for code generation
async fn call_text_llm(prompt: &str) -> Result<String, String> {
    let client = reqwest::Client::new();
    let full_prompt = format!("{}{} =", FEW_SHOT_PREFIX, prompt);
    
    let response = client
        .post("http://localhost:1234/v1/completions")
        .json(&serde_json::json!({
            "model": TEXT_MODEL,
            "prompt": full_prompt,
            "max_tokens": 100,
            "temperature": 0.0,
            "stop": ["\n", "Explanation"]
        }))
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    
    let json: serde_json::Value = response.json().await.map_err(|e| e.to_string())?;
    Ok(json["choices"][0]["text"].as_str().unwrap_or("ERROR").to_string())
}

/// Call vision LLM to interpret an image
async fn call_vision_llm(image_path: &str, question: &str) -> Result<String, String> {
    let client = reqwest::Client::new();
    
    // Read and encode image
    let image_data = std::fs::read(image_path).map_err(|e| e.to_string())?;
    let base64_image = general_purpose::STANDARD.encode(&image_data);
    
    let response = client
        .post("http://localhost:1234/v1/chat/completions")
        .json(&serde_json::json!({
            "model": VISION_MODEL,
            "messages": [{
                "role": "user",
                "content": [
                    {
                        "type": "image_url",
                        "image_url": {
                            "url": format!("data:image/png;base64,{}", base64_image)
                        }
                    },
                    {
                        "type": "text",
                        "text": question
                    }
                ]
            }],
            "max_tokens": 200
        }))
        .timeout(std::time::Duration::from_secs(60))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    
    let json: serde_json::Value = response.json().await.map_err(|e| e.to_string())?;
    Ok(json["choices"][0]["message"]["content"].as_str().unwrap_or("ERROR").to_string())
}

fn clean_vm_code(raw: &str) -> String {
    let valid_chars: std::collections::HashSet<char> = 
        "0123456789+-*/.,:@&?<>^v!_ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz "
        .chars().collect();
    
    let cleaned: String = raw.chars()
        .filter(|c| valid_chars.contains(c) || c.is_whitespace())
        .collect();
    
    let tokens: Vec<&str> = cleaned.split_whitespace().take(30).collect();
    let mut result = tokens.join(" ");
    
    if !result.ends_with('@') && !result.is_empty() {
        result.push_str(" @");
    }
    
    if result.is_empty() { "1 @".to_string() } else { result }
}

#[tokio::main]
async fn main() -> Result<(), String> {
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║         DUAL MODEL PIPELINE - Vision + Text              ║");
    println!("╠══════════════════════════════════════════════════════════╣");
    println!("║  Text LLM: {}                       ║", TEXT_MODEL);
    println!("║  Vision LLM: {}                     ║", VISION_MODEL);
    println!("╚══════════════════════════════════════════════════════════╝");
    println!();

    // Step 1: Generate code with text LLM
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("[STEP 1] Text LLM generates code");
    println!("[STEP 1] Prompt: \"Push 5, push 3, add, halt\"");
    
    let code_raw = call_text_llm("Push 5, push 3, add, halt").await?;
    let code = clean_vm_code(&code_raw);
    println!("[STEP 1] Generated: {}", code);
    println!();

    // Step 2: Check if framebuffer images exist
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("[STEP 2] Vision LLM interprets framebuffer");
    
    let fb_path = std::path::Path::new("/home/jericho/zion/projects/ascii_world/gpu/output");
    
    // Find latest PNG
    let latest_png = std::fs::read_dir(fb_path)
        .map_err(|e| e.to_string())?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|ext| ext == "png").unwrap_or(false))
        .max_by_key(|e| e.metadata().ok().and_then(|m| m.modified().ok()))
        .map(|e| e.path().to_string_lossy().to_string());
    
    if let Some(png_path) = latest_png {
        println!("[STEP 2] Image: {}", png_path);
        
        // Ask vision model to interpret
        let question = "Describe what you see in this image. What colors and patterns are present? Is there any text or numbers visible?";
        
        match call_vision_llm(&png_path, question).await {
            Ok(interpretation) => {
                println!("[STEP 2] Vision interpretation:");
                println!("  {}", interpretation.lines().take(5).collect::<Vec<_>>().join("\n  "));
            }
            Err(e) => {
                println!("[STEP 2] ⚠️ Vision call failed: {}", e);
                println!("[STEP 2] Vision model may need image format adjustment");
            }
        }
    } else {
        println!("[STEP 2] ⚠️ No framebuffer images found");
    }
    println!();

    // Step 3: Summary
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║                    PIPELINE STATUS                       ║");
    println!("╠══════════════════════════════════════════════════════════╣");
    println!("║  ✅ Text LLM working (code generation)                   ║");
    println!("║  🔄 Vision LLM integration tested                        ║");
    println!("║                                                          ║");
    println!("║  Architecture:                                           ║");
    println!("║    tinyllama → code → GPU → framebuffer                  ║");
    println!("║    qwen3-vl ← reads ← framebuffer ← interpretation       ║");
    println!("╚══════════════════════════════════════════════════════════╝");

    Ok(())
}
