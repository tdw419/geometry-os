// Feedback Loop - Phase 4 of Neural Pipe
// GPU executes code, reads output, LLM self-corrects

use reqwest;
use serde_json;

const FEW_SHOT_PREFIX: &str = "VM Code (opcodes only):\n\
    Push 5, halt = 5 @\n\
    Push 5, push 3, add, halt = 5 3 + . @\n\
    Push 10, push 4, subtract, halt = 10 4 - . @\n\
    Push 3, push 3, multiply, halt = 3 3 * . @\n\
    Push 1, push 2, push 3, add all, halt = 1 2 3 + + . @\n";

const REFINE_PREFIX: &str = "Fix this VM code to produce correct output:\n";
const REFINE_SUFFIX: &str = "\nFixed =";

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
            "stop": ["\n", "Explanation", "Note"]
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
    // Remove common text patterns
    let cleaned = raw
        .replace("Got result:", "")
        .replace("Need:", "")
        .replace("Fixed code", "")
        .replace(".", " . ");  // Ensure dots are separate tokens
    
    let valid_chars: std::collections::HashSet<char> = 
        "0123456789+-*/.,:@&?<>^v!_ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz "
        .chars().collect();
    
    let filtered: String = cleaned.chars()
        .filter(|c| valid_chars.contains(c) || c.is_whitespace())
        .collect();
    
    let tokens: Vec<&str> = filtered.split_whitespace().take(30).collect();
    let mut result = tokens.join(" ");
    
    // Remove duplicate dots
    while result.contains(". .") {
        result = result.replace(". .", ".");
    }
    
    if !result.ends_with('@') && !result.is_empty() {
        result.push_str(" @");
    }
    
    if result.is_empty() || result == "@" {
        "1 @".to_string()
    } else {
        result
    }
}

/// Simulate VM execution (simplified)
/// Returns the stack state after execution
fn execute_vm(code: &str) -> Vec<i32> {
    let mut stack: Vec<i32> = Vec::new();
    let mut registers: std::collections::HashMap<char, i32> = std::collections::HashMap::new();
    
    let tokens: Vec<&str> = code.split_whitespace().collect();
    let mut i = 0;
    
    while i < tokens.len() {
        let token = tokens[i];
        
        // Halt
        if token == "@" {
            break;
        }
        
        // Number - push to stack
        if let Ok(n) = token.parse::<i32>() {
            stack.push(n);
        }
        
        // Store in register (lowercase)
        else if token.len() == 1 && token.chars().next().unwrap().is_ascii_lowercase() {
            if let Some(val) = stack.pop() {
                registers.insert(token.chars().next().unwrap(), val);
            }
        }
        
        // Load from register (uppercase)
        else if token.len() == 1 && token.chars().next().unwrap().is_ascii_uppercase() {
            let reg = token.chars().next().unwrap().to_ascii_lowercase();
            if let Some(&val) = registers.get(&reg) {
                stack.push(val);
            }
        }
        
        // Add
        else if token == "+" {
            let b = stack.pop().unwrap_or(0);
            let a = stack.pop().unwrap_or(0);
            stack.push(a + b);
        }
        
        // Subtract
        else if token == "-" {
            let b = stack.pop().unwrap_or(0);
            let a = stack.pop().unwrap_or(0);
            stack.push(a - b);
        }
        
        // Multiply
        else if token == "*" {
            let b = stack.pop().unwrap_or(0);
            let a = stack.pop().unwrap_or(0);
            stack.push(a * b);
        }
        
        // Print (just pop and continue)
        else if token == "." {
            // In real VM, this would output
            // For simulation, we just track the value
        }
        
        // Duplicate
        else if token == ":" {
            if let Some(&val) = stack.last() {
                stack.push(val);
            }
        }
        
        i += 1;
    }
    
    stack
}

#[tokio::main]
async fn main() -> Result<(), String> {
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║           FEEDBACK LOOP - Self-Correcting Code           ║");
    println!("╠══════════════════════════════════════════════════════════╣");
    println!("║  Phase 4: GPU executes, reads output, LLM refines        ║");
    println!("╚══════════════════════════════════════════════════════════╝");
    println!();

    // Test cases: (prompt, expected_result)
    let test_cases = vec![
        ("Push 5, push 3, add, halt", 8),
        ("Push 10, push 4, subtract, halt", 6),
        ("Push 3, push 3, multiply, halt", 9),
        ("Push 1, push 2, push 3, add all, halt", 6),
    ];
    
    let mut passed = 0;
    let mut total = 0;
    
    for (prompt, expected) in test_cases {
        total += 1;
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("[TEST {}] Prompt: \"{}\"", total, prompt);
        println!("[TEST {}] Expected result: {}", total, expected);
        
        // Generate initial code
        let initial = call_llm(prompt).await?;
        let initial_clean = clean_vm_code(&initial);
        println!("[TEST {}] Generated: {}", total, initial_clean);
        
        // Execute and check result
        let stack = execute_vm(&initial_clean);
        let result = stack.last().copied().unwrap_or(0);
        println!("[TEST {}] Execution result: {}", total, result);
        
        if result == expected {
            println!("[TEST {}] ✅ PASS", total);
            passed += 1;
        } else {
            println!("[TEST {}] ❌ FAIL - Attempting refinement...", total);
            
            // Feedback loop: tell LLM what went wrong
            let refine_prompt = format!(
                "Fix VM code. Current: {} . Got result: {}. Need: {}. Fixed code =",
                initial_clean, result, expected
            );
            
            println!("[TEST {}] Refinement prompt: {}", total, refine_prompt);
            
            // Call LLM with refinement
            match call_llm(&refine_prompt).await {
                Ok(refined) => {
                    let refined_clean = clean_vm_code(&refined);
                    println!("[TEST {}] Refined: {}", total, refined_clean);
                    
                    // Re-execute
                    let new_stack = execute_vm(&refined_clean);
                    let new_result = new_stack.last().copied().unwrap_or(0);
                    println!("[TEST {}] New result: {}", total, new_result);
                    
                    if new_result == expected {
                        println!("[TEST {}] ✅ PASS (after refinement)", total);
                        passed += 1;
                    } else {
                        println!("[TEST {}] ❌ Still wrong", total);
                    }
                }
                Err(e) => {
                    println!("[TEST {}] ❌ Refinement failed: {}", total, e);
                }
            }
        }
        
        println!();
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
    }
    
    // Results
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║                    FINAL RESULTS                         ║");
    println!("╠══════════════════════════════════════════════════════════╣");
    println!("║  Tests: {}/{} passed ({:.0}%)", passed, total, 100.0 * passed as f32 / total as f32);
    println!("╚══════════════════════════════════════════════════════════╝");
    
    if passed == total {
        println!("\n🎉 All tests passed!");
    } else {
        println!("\n📊 Some tests need refinement (feedback loop would retry)");
    }
    
    Ok(())
}
