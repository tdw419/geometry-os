// Visual Logic Spec - Phase 1: Visual Cognition
// Test if LLM can draw its logic flow in pixels

use reqwest;
use serde_json;

const FEW_SHOT_PREFIX: &str = "VM Code with Visual Output:\n\
    Push 5, halt = 5 @\n\
    Push 5, push 3, add, halt = 5 3 + . @\n\
    Loop: add 1 until 10 = 0a 10i A 1 + a I 1 - i I ? < @\n";

async fn call_llm(prompt: &str) -> Result<String, String> {
    let client = reqwest::Client::new();
    let full_prompt = format!("{}{} =", FEW_SHOT_PREFIX, prompt);
    
    let response = client
        .post("http://localhost:1234/v1/completions")
        .json(&serde_json::json!({
            "model": "tinyllama-1.1b-chat-v1.0",
            "prompt": full_prompt,
            "max_tokens": 150,
            "temperature": 0.0,
            "stop": ["\n\n", "Explanation"]
        }))
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    
    let json: serde_json::Value = response.json().await.map_err(|e| e.to_string())?;
    Ok(json["choices"][0]["text"].as_str().unwrap_or("ERROR").to_string())
}

fn extract_visual_markers(code: &str) -> Vec<(char, usize, usize)> {
    // Find visual markers like ->, =>, <--, etc.
    let mut markers = Vec::new();
    
    for (i, line) in code.lines().enumerate() {
        if line.contains("->") { markers.push(('→', i, line.find("->").unwrap_or(0))); }
        if line.contains("=>") { markers.push(('⇒', i, line.find("=>").unwrap_or(0))); }
        if line.contains("<-") { markers.push(('←', i, line.find("<-").unwrap_or(0))); }
        if line.contains("^^") { markers.push(('↑', i, line.find("^^").unwrap_or(0))); }
        if line.contains("vv") { markers.push(('↓', i, line.find("vv").unwrap_or(0))); }
    }
    
    markers
}

#[tokio::main]
async fn main() -> Result<(), String> {
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║         VISUAL LOGIC SPEC - Flowchart Test               ║");
    println!("╠══════════════════════════════════════════════════════════╣");
    println!("║  Can the LLM visualize its logic flow?                   ║");
    println!("╚══════════════════════════════════════════════════════════╝");
    println!();

    // Test 1: Loop with visual marker
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("[TEST] Loop + Visual Arrow");
    println!("[TEST] Prompt: \"Loop: add 1 to a until 10. Draw -> to show flow.\"");
    
    let output = call_llm("Loop: add 1 to a until 10. Draw arrow -> for flow.").await?;
    println!("[TEST] Raw output: {}", output.lines().take(3).collect::<Vec<_>>().join("\n"));
    
    let markers = extract_visual_markers(&output);
    if !markers.is_empty() {
        println!("[TEST] ✅ Visual markers found: {:?}", markers);
    } else {
        println!("[TEST] ⚠️ No visual markers detected");
    }
    
    println!();
    
    // Test 2: Conditional with visual
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("[TEST] Conditional + Visual Branch");
    println!("[TEST] Prompt: \"If x > 5, print yes. Draw => for true path.\"");
    
    let output2 = call_llm("If x > 5, print yes. Draw => for true branch.").await?;
    println!("[TEST] Raw output: {}", output2.lines().take(3).collect::<Vec<_>>().join("\n"));
    
    let markers2 = extract_visual_markers(&output2);
    if !markers2.is_empty() {
        println!("[TEST] ✅ Visual markers found: {:?}", markers2);
    } else {
        println!("[TEST] ⚠️ No visual markers detected");
    }
    
    println!();
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║                    RESULTS                               ║");
    println!("╠══════════════════════════════════════════════════════════╣");
    
    let total_markers = markers.len() + markers2.len();
    if total_markers > 0 {
        println!("║  ✅ Visual cognition detected: {} markers              ║", total_markers);
        println!("║  The LLM can represent logic visually!                  ║");
    } else {
        println!("║  📊 No visual markers in output                         ║");
        println!("║  May need visual examples in few-shot                   ║");
    }
    println!("╚══════════════════════════════════════════════════════════╝");
    
    Ok(())
}
