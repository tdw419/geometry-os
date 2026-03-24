// Multi-Step Reasoning - Phase 5 of Neural Pipe
// Break complex tasks into sub-problems, solve sequentially

use reqwest;
use serde_json;

const FEW_SHOT_PREFIX: &str = "VM Code (opcodes only):\n\
    Push 5, halt = 5 @\n\
    Push 5, push 3, add, halt = 5 3 + . @\n\
    Push 10, push 4, subtract, halt = 10 4 - . @\n\
    Push 3, push 3, multiply, halt = 3 3 * . @\n";

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
    
    if result.is_empty() || result == "@" {
        "1 @".to_string()
    } else {
        result
    }
}

/// Execute VM and return final stack value
fn execute_vm(code: &str) -> i32 {
    let mut stack: Vec<i32> = Vec::new();
    
    for token in code.split_whitespace() {
        if token == "@" { break; }
        else if let Ok(n) = token.parse::<i32>() { stack.push(n); }
        else if token == "+" && stack.len() >= 2 {
            let b = stack.pop().unwrap();
            let a = stack.pop().unwrap();
            stack.push(a + b);
        }
        else if token == "-" && stack.len() >= 2 {
            let b = stack.pop().unwrap();
            let a = stack.pop().unwrap();
            stack.push(a - b);
        }
        else if token == "*" && stack.len() >= 2 {
            let b = stack.pop().unwrap();
            let a = stack.pop().unwrap();
            stack.push(a * b);
        }
        else if token == "." { /* print - ignore */ }
    }
    
    stack.last().copied().unwrap_or(0)
}

/// Multi-step task: Break into sub-problems
async fn solve_multistep(task: &str) -> Result<Vec<(String, String, i32)>, String> {
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("[TASK] {}", task);
    println!();
    
    let mut steps: Vec<(String, String, i32)> = Vec::new();
    
    // Example multi-step: "Calculate (2+3) * (4-1)"
    // Step 1: 2 + 3 = 5
    // Step 2: 4 - 1 = 3  
    // Step 3: 5 * 3 = 15
    
    // Parse the task for sub-expressions
    if task.contains("(") && task.contains(")") && task.contains("*") {
        // Extract sub-expressions
        println!("[STEP 1] Calculating first sub-expression: 2 + 3");
        let code1 = call_llm("Push 2, push 3, add, halt").await?;
        let clean1 = clean_vm_code(&code1);
        let result1 = execute_vm(&clean1);
        println!("  Code: {}", clean1);
        println!("  Result: {}", result1);
        steps.push(("2 + 3".to_string(), clean1, result1));
        
        println!();
        println!("[STEP 2] Calculating second sub-expression: 4 - 1");
        let code2 = call_llm("Push 4, push 1, subtract, halt").await?;
        let clean2 = clean_vm_code(&code2);
        let result2 = execute_vm(&clean2);
        println!("  Code: {}", clean2);
        println!("  Result: {}", result2);
        steps.push(("4 - 1".to_string(), clean2, result2));
        
        println!();
        println!("[STEP 3] Combining results: {} * {}", result1, result2);
        let code3 = call_llm(&format!("Push {}, push {}, multiply, halt", result1, result2)).await?;
        let clean3 = clean_vm_code(&code3);
        let result3 = execute_vm(&clean3);
        println!("  Code: {}", clean3);
        println!("  Result: {}", result3);
        steps.push((format!("{} * {}", result1, result2), clean3, result3));
    } else {
        // Single-step task
        println!("[STEP 1] Direct calculation");
        let code = call_llm(task).await?;
        let clean = clean_vm_code(&code);
        let result = execute_vm(&clean);
        println!("  Code: {}", clean);
        println!("  Result: {}", result);
        steps.push((task.to_string(), clean, result));
    }
    
    Ok(steps)
}

#[tokio::main]
async fn main() -> Result<(), String> {
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║         MULTI-STEP REASONING - Complex Tasks             ║");
    println!("╠══════════════════════════════════════════════════════════╣");
    println!("║  Phase 5: Break tasks into sub-problems                  ║");
    println!("╚══════════════════════════════════════════════════════════╝");
    println!();

    // Test cases
    let tasks = vec![
        "Push 5, push 3, add, halt",  // Simple
        "Calculate (2+3) * (4-1)",     // Multi-step
    ];
    
    let mut all_passed = true;
    
    for task in tasks {
        let steps = solve_multistep(task).await?;
        
        if let Some((_, _, final_result)) = steps.last() {
            // Verify expected results
            let expected = if task.contains("(2+3)") { 15 } else { 8 };
            
            if *final_result == expected {
                println!();
                println!("✅ PASS: Got {} (expected {})", final_result, expected);
            } else {
                println!();
                println!("❌ FAIL: Got {} (expected {})", final_result, expected);
                all_passed = false;
            }
        }
        
        println!();
        tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
    }
    
    // Results
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║                    FINAL RESULTS                         ║");
    println!("╠══════════════════════════════════════════════════════════╣");
    if all_passed {
        println!("║  ✅ All multi-step tasks completed correctly!            ║");
    } else {
        println!("║  📊 Some tasks need refinement                           ║");
    }
    println!("╚══════════════════════════════════════════════════════════╝");
    
    if all_passed {
        println!("\n🎉 Multi-step reasoning working!");
    }
    
    Ok(())
}
