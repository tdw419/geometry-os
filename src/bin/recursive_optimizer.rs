// Recursive Optimizer - Phase 3 of Neural Pipe
// LLM generates code, reads it back, and optimizes itself

use reqwest;
use serde_json;

// Few-shot completion prompt for clean opcode output
const FEW_SHOT_PREFIX: &str = "VM Code (opcodes only):\n\
    Push 5, halt = 5 @\n\
    Push 1, push 2, add, print, halt = 1 2 + . @\n\
    Push 10, store x, halt = 10 x ! @\n\
    Load x, print, halt = X . @\n\
    Fibonacci 10 = 0a 1b 10i A B + : . b a I 1 - i I ? < @\n";

// Optimization prompts - start with Fibonacci
const INITIAL_PROMPT: &str = "Fibonacci 5";

const OPTIMIZE_PREFIX: &str = "Optimize VM code (remove redundancy, keep result):\n\
    Before: 1 1 + 2 2 + . . @\n\
    After: 1 + 2 + . . @\n\
    \n";
const OPTIMIZE_SUFFIX: &str = "\nAfter =";

async fn call_llm(prompt: &str) -> Result<String, String> {
    let client = reqwest::Client::new();
    
    let full_prompt = format!("{}{} =", FEW_SHOT_PREFIX, prompt);
    
    let response = client
        .post("http://localhost:1234/v1/completions")
        .json(&serde_json::json!({
            "model": "tinyllama-1.1b-chat-v1.0",
            "prompt": full_prompt,
            "max_tokens": 100,
            "temperature": 0.0,
            "stop": ["\n", "Explanation", "Note", "This"]
        }))
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    
    let json: serde_json::Value = response.json().await.map_err(|e| e.to_string())?;
    
    Ok(json["choices"][0]["text"]
        .as_str()
        .unwrap_or("ERROR")
        .to_string())
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
    
    if result.is_empty() {
        "1 @".to_string()
    } else {
        result
    }
}

#[tokio::main]
async fn main() -> Result<(), String> {
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║         RECURSIVE OPTIMIZER - Self-Improving Code        ║");
    println!("╠══════════════════════════════════════════════════════════╣");
    println!("║  Phase 3: LLM optimizes its own generated code           ║");
    println!("╚══════════════════════════════════════════════════════════╝");
    println!();

    // Initial code generation
    println!("[ITER 0] Generating initial code...");
    println!("[ITER 0] Prompt: \"{}\"", INITIAL_PROMPT);
    
    let initial = call_llm(INITIAL_PROMPT).await?;
    let initial_clean = clean_vm_code(&initial);
    
    println!("[ITER 0] Raw: {}", initial.trim());
    println!("[ITER 0] Cleaned: {}", initial_clean);
    println!("[ITER 0] Length: {} chars, {} tokens", initial_clean.len(), initial_clean.split_whitespace().count());
    println!();
    
    let mut current_code = initial_clean.clone();
    let mut best_code = current_code.clone();
    let mut best_len = current_code.split_whitespace().count();
    
    // Optimization loop (5 iterations)
    for i in 1..=5 {
        println!("[ITER {}] Optimizing previous code...", i);
        
        // Create optimization prompt with current code as context
        let full_optimize = format!("{}{}{}", 
            OPTIMIZE_PREFIX, 
            current_code, 
            OPTIMIZE_SUFFIX
        );
        
        let optimized = call_llm(&full_optimize).await?;
        let optimized_clean = clean_vm_code(&optimized);
        
        let before_tokens = current_code.split_whitespace().count();
        let after_tokens = optimized_clean.split_whitespace().count();
        
        println!("[ITER {}] Before: {}", i, current_code);
        println!("[ITER {}] After:  {}", i, optimized_clean);
        println!("[ITER {}] Tokens: {} → {}", i, before_tokens, after_tokens);
        
        // Check if improvement (fewer tokens = better)
        if after_tokens < best_len && after_tokens >= 2 {
            best_code = optimized_clean.clone();
            best_len = after_tokens;
            println!("[ITER {}] ✅ NEW BEST! Saved optimization", i);
        } else if after_tokens == before_tokens {
            println!("[ITER {}] ➡️ No change", i);
        } else {
            println!("[ITER {}] ⬆️ Got longer, keeping previous", i);
        }
        
        current_code = optimized_clean;
        println!();
        
        tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
    }
    
    // Results
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║                    FINAL RESULTS                         ║");
    println!("╠══════════════════════════════════════════════════════════╣");
    println!("║  Initial: {}", initial_clean);
    println!("║  Best:    {}", best_code);
    
    let initial_tokens = initial_clean.split_whitespace().count();
    let improvement = 100.0 * (1.0 - best_len as f32 / initial_tokens as f32);
    
    println!("║  Tokens:  {} → {} ({:.0}% reduction)", 
        initial_tokens, best_len, improvement
    );
    println!("╚══════════════════════════════════════════════════════════╝");
    
    if best_len < initial_tokens {
        println!("\n🎉 SUCCESS: Code optimized by {:.0}%!", improvement);
    } else {
        println!("\n📊 No improvement found. Code may already be optimal.");
    }
    
    Ok(())
}
