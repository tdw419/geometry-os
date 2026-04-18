// hermes.rs -- Local LLM agent loop (Ollama) for Geometry OS

use crate::assembler;
use crate::canvas::{
    ensure_scroll, handle_terminal_command, list_asm_files, read_canvas_line, source_from_canvas,
    write_line_to_canvas,
};
use crate::preprocessor;
use crate::save::save_screen_png;
use crate::vm;
use std::collections::HashSet;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// instead of stdout. This is the visual/canvas version of run_hermes_loop().
#[allow(clippy::too_many_arguments)]
pub fn run_hermes_canvas(
    initial_prompt: &str,
    vm: &mut vm::Vm,
    canvas_buffer: &mut Vec<u32>,
    output_row: &mut usize,
    scroll_offset: &mut usize,
    loaded_file: &mut Option<PathBuf>,
    canvas_assembled: &mut bool,
    breakpoints: &mut HashSet<u32>,
) {
    *output_row = write_line_to_canvas(
        canvas_buffer,
        *output_row,
        "[hermes] Starting agent loop...",
    );
    *output_row =
        write_line_to_canvas(canvas_buffer, *output_row, "[hermes] Press Escape to stop.");
    ensure_scroll(*output_row, scroll_offset);

    let mut conversation_history = initial_prompt.to_string();

    for iteration in 0..10 {
        // Build context from canvas buffer (not source_text string)
        let source_text = source_from_canvas(canvas_buffer);
        let ctx = build_hermes_context(vm, &source_text, loaded_file);
        let full_system = format!("{}\n\n{}", HERMES_SYSTEM_PROMPT, ctx);

        *output_row = write_line_to_canvas(
            canvas_buffer,
            *output_row,
            &format!("[hermes] --- iteration {} ---", iteration + 1),
        );
        ensure_scroll(*output_row, scroll_offset);

        // Call LLM (this blocks -- curl subprocess)
        let response = match call_ollama(&full_system, &conversation_history) {
            Some(r) => r,
            None => {
                *output_row = write_line_to_canvas(
                    canvas_buffer,
                    *output_row,
                    "[hermes] LLM call failed. Stopping.",
                );
                ensure_scroll(*output_row, scroll_offset);
                break;
            }
        };

        // Strip <think/> blocks
        let response_clean = response
            .replace("\\u003cthink\\u003e", "<think")
            .replace("\\u003c/think\\u003e", "</think");
        let mut commands = String::new();
        let mut in_think = false;
        for line in response_clean.lines() {
            if line.contains("<think") {
                in_think = true;
            }
            if !in_think {
                let trimmed = line.trim();
                if !trimmed.is_empty() && !trimmed.starts_with("//") {
                    commands.push_str(trimmed);
                    commands.push('\n');
                }
            }
            if line.contains("</think") {
                in_think = false;
            }
        }

        if commands.trim().is_empty() {
            *output_row = write_line_to_canvas(
                canvas_buffer,
                *output_row,
                "[hermes] LLM returned no commands. Stopping.",
            );
            ensure_scroll(*output_row, scroll_offset);
            break;
        }

        // Show the commands the LLM wants to run
        for cmd_line in commands.lines() {
            let trimmed = cmd_line.trim();
            if !trimmed.is_empty() {
                *output_row =
                    write_line_to_canvas(canvas_buffer, *output_row, &format!("  > {}", trimmed));
            }
        }
        ensure_scroll(*output_row, scroll_offset);

        // Handle write buffers (LLM can create .asm files)
        let mut write_buffer: Option<(String, String)> = None;
        let mut output_capture = String::new();

        for cmd_line in commands.lines() {
            let cmd_line = cmd_line.trim();
            if cmd_line.is_empty() {
                continue;
            }

            // Handle write command for creating .asm files
            if let Some(ref mut wb) = write_buffer {
                if cmd_line == "ENDWRITE" {
                    match std::fs::write(&wb.0, &wb.1) {
                        Ok(()) => {
                            let msg = format!("Wrote {}", wb.0);
                            *output_row = write_line_to_canvas(canvas_buffer, *output_row, &msg);
                            output_capture.push_str(&msg);
                            output_capture.push('\n');
                        }
                        Err(e) => {
                            let msg = format!("Write error: {}", e);
                            *output_row = write_line_to_canvas(canvas_buffer, *output_row, &msg);
                            output_capture.push_str(&msg);
                            output_capture.push('\n');
                        }
                    }
                    write_buffer = None;
                } else {
                    wb.1.push_str(cmd_line);
                    wb.1.push('\n');
                }
                continue;
            }

            if cmd_line.starts_with("write ") {
                if let Some(filename) = cmd_line.strip_prefix("write ").map(|s| s.trim()) {
                    write_buffer = Some((filename.to_string(), String::new()));
                }
                continue;
            }

            // Execute command through the GUI terminal handler
            let cmd_parts: Vec<&str> = cmd_line.split_whitespace().collect();
            if cmd_parts.is_empty() {
                continue;
            }
            let cmd_word = cmd_parts[0].to_lowercase();

            match cmd_word.as_str() {
                "load" | "run" | "regs" | "peek" | "poke" | "screen" | "save" | "reset"
                | "list" | "ls" | "png" | "disasm" | "step" | "bp" | "bpc" | "trace" => {
                    // Execute through the GUI terminal command handler
                    // We need to capture what it writes, so we use a temporary
                    // approach: record output_row before and after, then extract
                    let row_before = *output_row;
                    let (_hermes_prompt, _go_edit, _quit) = handle_terminal_command(
                        cmd_line,
                        vm,
                        canvas_buffer,
                        output_row,
                        scroll_offset,
                        loaded_file,
                        canvas_assembled,
                        breakpoints,
                    );
                    // Capture output text for LLM context
                    for row in row_before..(*output_row) {
                        let line_text = read_canvas_line(canvas_buffer, row);
                        if !line_text.is_empty() && !line_text.starts_with("geo> ") {
                            output_capture.push_str(&line_text);
                            output_capture.push('\n');
                        }
                    }
                    // handle_terminal_command writes its own "geo> " prompt;
                    // we want to continue writing our output, so back up
                    // to overwrite that prompt on next write
                    if *output_row > 0 {
                        // Check if last written line is a "geo> " prompt from the sub-command
                        let last_text = read_canvas_line(canvas_buffer, *output_row - 1);
                        if last_text.starts_with("geo> ") || last_text == "geo>" {
                            // Don't back up -- we want these prompts visible as markers
                        }
                    }
                    ensure_scroll(*output_row, scroll_offset);
                }
                _ => {
                    // Skip unknown commands silently
                }
            }
        }

        // Handle unclosed write buffer
        if let Some(wb) = write_buffer {
            match std::fs::write(&wb.0, &wb.1) {
                Ok(()) => {
                    *output_row = write_line_to_canvas(
                        canvas_buffer,
                        *output_row,
                        &format!("Wrote {}", wb.0),
                    );
                }
                Err(e) => {
                    *output_row = write_line_to_canvas(
                        canvas_buffer,
                        *output_row,
                        &format!("Write error: {}", e),
                    );
                }
            }
        }

        *output_row = write_line_to_canvas(
            canvas_buffer,
            *output_row,
            "[hermes] Loop complete. Type another prompt or 'stop'.",
        );
        ensure_scroll(*output_row, scroll_offset);

        // For canvas mode: auto-continue for up to 3 iterations,
        // then stop. The user can type "hermes <prompt>" again.
        // (No stdin blocking in GUI mode -- we just run and return)
        if iteration >= 2 {
            *output_row = write_line_to_canvas(
                canvas_buffer,
                *output_row,
                "[hermes] Max iterations reached.",
            );
            break;
        }

        // Feed output back as context for next iteration
        conversation_history = format!(
            "Previous commands output:\n{}\n\nUser instruction: continue",
            output_capture,
        );
    }

    *output_row = write_line_to_canvas(canvas_buffer, *output_row, "[hermes] Agent loop ended.");
    ensure_scroll(*output_row, scroll_offset);
}

pub const HERMES_SYSTEM_PROMPT: &str = r#"You are an agent inside the Geometry OS terminal. You drive a bytecode VM by issuing geo> commands.

## Available commands
- load <file>       Load .asm source (from programs/ dir or absolute path)
- run               Assemble source & run VM
- regs              Show register dump (r0-r31, PC, SP, LR)
- peek <hex_addr>   Read RAM[addr]
- poke <hex_addr> <hex_val>  Write RAM[addr]
- screen [addr]     Dump 16 pixels from screen buffer
- save [file.ppm]   Save screen as PPM image
- png [file.png]    Save screen as PNG image
- reset             Reset VM state
- help              Show commands

## CRITICAL: Register conventions
- r0 is RESERVED for CMP results (-1/0/1). NEVER use r0 as a counter or accumulator.
- Use r1-r9 for hot variables, r10-r26 for general state, r27-r29 for temps.
- r30 = Stack Pointer (SP), r31 = Link Register (LR for CALL/RET).

## CRITICAL: ALL instructions take EXACTLY 2 arguments (except noted)
There is NO 3-argument form for ANY instruction. Every ALU op modifies rd using rs:
- ADD rd, rs     means rd = rd + rs   (NOT ADD rd, rs1, rs2)
- SUB rd, rs     means rd = rd - rs
- MUL rd, rs     means rd = rd * rs
- DIV rd, rs     means rd = rd / rs
- AND rd, rs     means rd = rd & rs
- OR rd, rs      means rd = rd | rs
- XOR rd, rs     means rd = rd ^ rs
- SHL rd, rs     means rd = rd << rs
- SHR rd, rs     means rd = rd >> rs
- MOD rd, rs     means rd = rd % rs
- NEG rd         means rd = -rd (1 arg)
- MOV rd, rs     means rd = rs
To compute x + y into a new register: LDI rd, 0 then ADD rd, rs (or MOV rd, rs then ADD).

## Full instruction set
Data:     LDI reg, imm | LOAD reg, addr_r | STORE addr_r, reg | MOV rd, rs
ALU:      ADD rd, rs | SUB rd, rs | MUL rd, rs | DIV rd, rs | MOD rd, rs
          AND rd, rs | OR rd, rs | XOR rd, rs | SHL rd, rs | SHR rd, rs | NEG rd
Compare:  CMP rd, rs (sets r0 = -1 if rd<rs, 0 if ==, 1 if rd>rs)
Branch:   JMP label | JZ reg, label | JNZ reg, label
          BLT r0, label (branch if r0==0xFFFFFFFF) | BGE r0, label (branch if r0!=0xFFFFFFFF)
Stack:    PUSH reg | POP reg (SP=r30, grows down)
Call:     CALL label | RET (return addr in r31)
Pixel:    PSET xr, yr, cr | PSETI x, y, color | FILL cr
          RECTF xr, yr, wr, hr, cr | TEXT xr, yr, ar
          LINE x0r, y0r, x1r, y1r, cr | CIRCLE xr, yr, rr, cr
          SPRITE xr, yr, addr_r, wr, hr | PEEK xr, yr, dr
Other:    SCROLL nr | IKEY reg | RAND reg | FRAME | BEEP freq_r, dur_r
          SPAWN reg | KILL reg | ASM src_r, dest_r
          HALT | NOP

## Example: fill screen with blue gradient
```asm
LDI r10, 0       ; y = 0
LDI r1, 1        ; increment
LDI r5, 256      ; limit
y_loop:
  LDI r2, 0      ; x = 0
  x_loop:
    MOV r6, r10   ; copy y
    SHL r6, r1    ; r6 = y * 2 (scale blue)
    PSET r2, r10, r6
    ADD r2, r1    ; x++
    CMP r2, r5
    BLT r0, x_loop
  ADD r10, r1     ; y++
  CMP r10, r5
  BLT r0, y_loop
HALT
```

## Example: bouncing ball animation
```asm
LDI r1, 128
LDI r2, 128
LDI r3, 1
LDI r4, 1
LDI r7, 1
LDI r8, 0x00FF00
LDI r9, 0x000000
loop:
  FILL r9
  CIRCLE r1, r2, r7, r8
  ADD r1, r3
  ADD r2, r4
  CMP r1, r5
  BLT r0, skip1
  NEG r3
skip1:
  CMP r2, r5
  BLT r0, skip2
  NEG r4
skip2:
  FRAME
  JMP loop
```

## Response format
Respond with one geo> command per line. No explanation, no markdown, no backticks.
Just the commands you want executed. You can also write new .asm programs by using
the write command:
  write <filename.asm>  (then subsequent lines are the file content, end with ENDWRITE on its own line)

After your commands run, you'll see the output and can issue more commands.
Think step by step but only output commands."#;

pub fn build_hermes_context(
    vm: &vm::Vm,
    source_text: &str,
    loaded_file: &Option<PathBuf>,
) -> String {
    let mut ctx = String::new();

    // VM state
    ctx.push_str("## Current VM State\n");
    for row_group in 0..4 {
        let mut line = String::new();
        for col in 0..8 {
            let i = row_group * 8 + col;
            line.push_str(&format!("r{:02}={:08X} ", i, vm.regs[i]));
        }
        ctx.push_str(&line);
        ctx.push('\n');
    }
    ctx.push_str(&format!(
        "PC={:04X} SP={:04X} LR={:04X}\n",
        vm.pc, vm.regs[30], vm.regs[31]
    ));
    ctx.push_str(&format!("Halted: {}\n", vm.halted));

    // Loaded file
    if let Some(ref f) = loaded_file {
        ctx.push_str(&format!("\n## Loaded file: {}\n", f.display()));
    }

    // Source text (first 100 lines)
    if !source_text.is_empty() {
        ctx.push_str("\n## Current source (first 100 lines)\n");
        for (i, line) in source_text.lines().take(100).enumerate() {
            ctx.push_str(&format!("{:3}: {}\n", i + 1, line));
        }
        let total = source_text.lines().count();
        if total > 100 {
            ctx.push_str(&format!("... ({} more lines)\n", total - 100));
        }
    }

    ctx
}

pub fn call_ollama(system_prompt: &str, user_message: &str) -> Option<String> {
    // Build the JSON payload
    // Escape strings for JSON
    let esc_sys = system_prompt
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\t', "\\t");
    let esc_user = user_message
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\t', "\\t");

    let payload = format!(
        r#"{{"model":"qwen3.5-tools","messages":[{{"role":"system","content":"{}"}},{{"role":"user","content":"{}"}}],"stream":false}}"#,
        esc_sys, esc_user
    );

    // Write payload to temp file to avoid shell escaping issues
    let tmp_path = "/tmp/geo_hermes_payload.json";
    match std::fs::write(tmp_path, &payload) {
        Ok(()) => {}
        Err(e) => {
            println!("[hermes] Error writing payload: {}", e);
            return None;
        }
    }

    let output = match std::process::Command::new("curl")
        .args([
            "-s",
            "-X",
            "POST",
            "http://localhost:11434/api/chat",
            "-d",
            &format!("@{}", tmp_path),
            "-H",
            "Content-Type: application/json",
        ])
        .output()
    {
        Ok(o) => o,
        Err(e) => {
            println!("[hermes] curl failed: {}", e);
            return None;
        }
    };

    // Parse response -- extract message.content
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Simple JSON extraction: find "content":"..."`
    // Look for the content field in the response
    if let Some(start) = stdout.find(r#""content":""#) {
        let content_start = start + r#""content":""#.len();
        // Find the closing quote (handle escaped quotes)
        let mut i = content_start;
        let mut result = String::new();
        let bytes = stdout.as_bytes();
        while i < bytes.len() {
            if bytes[i] == b'\\' && i + 1 < bytes.len() {
                // Escaped character
                match bytes[i + 1] {
                    b'n' => result.push('\n'),
                    b't' => result.push('\t'),
                    b'"' => result.push('"'),
                    b'\\' => result.push('\\'),
                    _ => {
                        result.push(bytes[i] as char);
                        result.push(bytes[i + 1] as char);
                    }
                }
                i += 2;
            } else if bytes[i] == b'"' {
                break;
            } else {
                result.push(bytes[i] as char);
                i += 1;
            }
        }
        Some(result)
    } else {
        println!("[hermes] Could not parse LLM response");
        None
    }
}

pub fn run_hermes_loop(
    initial_prompt: &str,
    vm: &mut vm::Vm,
    source_text: &mut String,
    loaded_file: &mut Option<PathBuf>,
    canvas_assembled: &mut bool,
) {
    println!("[hermes] Starting agent loop (qwen3.5-tools via Ollama)");
    println!("[hermes] Type 'stop' to end the loop, or just press Enter to continue.");

    let mut conversation_history = initial_prompt.to_string();

    for iteration in 0..10 {
        // Build context
        let ctx = build_hermes_context(vm, source_text, loaded_file);
        let full_system = format!("{}\n\n{}", HERMES_SYSTEM_PROMPT, ctx);

        println!("[hermes] --- iteration {} ---", iteration + 1);

        // Call LLM
        let response = match call_ollama(&full_system, &conversation_history) {
            Some(r) => r,
            None => {
                println!("[hermes] LLM call failed. Stopping.");
                break;
            }
        };

        // Strip <think/> blocks (qwen3.5 includes reasoning)
        // Also handle unicode-escaped versions: \u003cthink\u003e
        let response_clean = response
            .replace("\\u003cthink\\u003e", "<think")
            .replace("\\u003c/think\\u003e", "</think");
        let mut commands = String::new();
        let mut in_think = false;
        for line in response_clean.lines() {
            if line.contains("<think") {
                in_think = true;
            }
            if !in_think {
                let trimmed = line.trim();
                if !trimmed.is_empty() && !trimmed.starts_with("//") {
                    commands.push_str(trimmed);
                    commands.push('\n');
                }
            }
            if line.contains("</think") {
                in_think = false;
            }
        }

        if commands.trim().is_empty() {
            println!("[hermes] LLM returned no commands. Stopping.");
            break;
        }

        println!("[hermes] LLM commands:\n{}", commands);

        // Track any write buffers
        let mut write_buffer: Option<(String, String)> = None;

        // Execute each command
        let mut output_capture = String::new();
        for cmd_line in commands.lines() {
            let cmd_line = cmd_line.trim();
            if cmd_line.is_empty() {
                continue;
            }

            // Handle write command for creating .asm files
            if let Some(ref mut wb) = write_buffer {
                if cmd_line == "ENDWRITE" {
                    // Write the file
                    match std::fs::write(&wb.0, &wb.1) {
                        Ok(()) => {
                            let msg = format!("Wrote {}", wb.0);
                            println!("{}", msg);
                            output_capture.push_str(&msg);
                            output_capture.push('\n');
                        }
                        Err(e) => {
                            let msg = format!("Write error: {}", e);
                            println!("{}", msg);
                            output_capture.push_str(&msg);
                            output_capture.push('\n');
                        }
                    }
                    write_buffer = None;
                } else {
                    wb.1.push_str(cmd_line);
                    wb.1.push('\n');
                }
                continue;
            }

            if cmd_line.starts_with("write ") {
                if let Some(filename) = cmd_line.strip_prefix("write ").map(|s| s.trim()) {
                    write_buffer = Some((filename.to_string(), String::new()));
                }
                continue;
            }

            // Skip non-geo commands
            let cmd_parts: Vec<&str> = cmd_line.split_whitespace().collect();
            if cmd_parts.is_empty() {
                continue;
            }
            let cmd_word = cmd_parts[0].to_lowercase();

            // Only execute known geo> commands
            match cmd_word.as_str() {
                "load" | "run" | "regs" | "peek" | "poke" | "screen" | "save" | "reset"
                | "list" | "ls" | "png" => {
                    println!("geo> {}", cmd_line);
                    // Capture output by redirecting through a helper
                    execute_cli_command(
                        cmd_line,
                        vm,
                        source_text,
                        loaded_file,
                        canvas_assembled,
                        &mut output_capture,
                    );
                }
                _ => {
                    // Skip unknown commands silently
                }
            }
        }

        // Handle unclosed write buffer
        if let Some(wb) = write_buffer {
            match std::fs::write(&wb.0, &wb.1) {
                Ok(()) => println!("Wrote {}", wb.0),
                Err(e) => println!("Write error: {}", e),
            }
        }

        // Ask if user wants to continue
        print!("[hermes] Continue? (Enter=continue, stop=quit): ");
        let _ = io::stdout().flush();
        let mut input = String::new();
        if io::stdin().read_line(&mut input).unwrap_or(0) == 0 {
            break;
        }
        let answer = input.trim().to_lowercase();
        if answer == "stop" || answer == "quit" || answer == "exit" || answer == "q" {
            println!("[hermes] Stopped.");
            break;
        }

        // Feed output back as context for next iteration
        conversation_history = format!(
            "Previous commands output:\n{}\n\nUser instruction: {}",
            output_capture,
            if answer.is_empty() {
                "continue"
            } else {
                &answer
            }
        );
    }

    println!("[hermes] Agent loop ended.");
}

/// Execute a single CLI command and capture output.
pub fn execute_cli_command(
    cmd: &str,
    vm: &mut vm::Vm,
    source_text: &mut String,
    loaded_file: &mut Option<PathBuf>,
    canvas_assembled: &mut bool,
    output: &mut String,
) {
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    if parts.is_empty() {
        return;
    }
    let command = parts[0].to_lowercase();

    match command.as_str() {
        "list" | "ls" => {
            let files = list_asm_files("programs");
            if files.is_empty() {
                let msg = "  (no .asm files in programs/)".to_string();
                println!("{}", msg);
                output.push_str(&msg);
                output.push('\n');
            } else {
                for f in &files {
                    let name = Path::new(f)
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| f.clone());
                    let msg = format!("  {}", name);
                    println!("{}", msg);
                    output.push_str(&msg);
                    output.push('\n');
                }
                let msg = format!("  {} programs", files.len());
                println!("{}", msg);
                output.push_str(&msg);
                output.push('\n');
            }
        }
        "load" => {
            if parts.len() < 2 {
                let msg = "Usage: load <file>".to_string();
                println!("{}", msg);
                output.push_str(&msg);
                output.push('\n');
                return;
            }
            let mut filename = parts[1..].join(" ");
            if !filename.ends_with(".asm") {
                filename.push_str(".asm");
            }
            let path = Path::new(&filename);
            let path = if path.exists() {
                path.to_path_buf()
            } else {
                let prefixed = Path::new("programs").join(&filename);
                if prefixed.exists() {
                    prefixed
                } else {
                    let msg = format!("File not found: {}", filename);
                    println!("{}", msg);
                    output.push_str(&msg);
                    output.push('\n');
                    return;
                }
            };
            match std::fs::read_to_string(&path) {
                Ok(src) => {
                    let lines = src.lines().count();
                    *source_text = src;
                    *loaded_file = Some(path.clone());
                    let msg = format!(
                        "Loaded {} ({} lines)",
                        path.file_name()
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_default(),
                        lines
                    );
                    println!("{}", msg);
                    output.push_str(&msg);
                    output.push('\n');
                }
                Err(e) => {
                    let msg = format!("Error: {}", e);
                    println!("{}", msg);
                    output.push_str(&msg);
                    output.push('\n');
                }
            }
        }
        "run" => {
            if source_text.is_empty() {
                let msg = "No source loaded.".to_string();
                println!("{}", msg);
                output.push_str(&msg);
                output.push('\n');
                return;
            }
            // Abstraction Layer: Preprocess macros and variables
            let mut pp = preprocessor::Preprocessor::new();
            let preprocessed_source = pp.preprocess(source_text);

            match assembler::assemble(&preprocessed_source, 0) {
                Ok(asm_result) => {
                    let ram_len = vm.ram.len();
                    let load_addr = 0usize;
                    for v in vm.ram[load_addr..ram_len.min(load_addr + 4096)].iter_mut() {
                        *v = 0;
                    }
                    for (i, &pixel) in asm_result.pixels.iter().enumerate() {
                        let addr = load_addr + i;
                        if addr < ram_len {
                            vm.ram[addr] = pixel;
                        }
                    }
                    vm.pc = load_addr as u32;
                    vm.halted = false;
                    let msg = format!(
                        "Assembled {} bytes at 0x{:04X}",
                        asm_result.pixels.len(),
                        load_addr
                    );
                    println!("{}", msg);
                    output.push_str(&msg);
                    output.push('\n');

                    for _ in 0..10_000_000 {
                        if !vm.step() {
                            break;
                        }
                    }
                    let msg = if vm.halted {
                        format!("Halted at PC=0x{:04X}", vm.pc)
                    } else {
                        format!("Running... PC=0x{:04X}", vm.pc)
                    };
                    println!("{}", msg);
                    output.push_str(&msg);
                    output.push('\n');
                    *canvas_assembled = true;
                }
                Err(e) => {
                    let msg = format!("{}", e);
                    println!("{}", msg);
                    output.push_str(&msg);
                    output.push('\n');
                }
            }
        }
        "regs" => {
            for row_group in 0..4 {
                let mut line = String::new();
                for col in 0..8 {
                    let i = row_group * 8 + col;
                    line.push_str(&format!("r{:02}={:08X} ", i, vm.regs[i]));
                }
                println!("{}", line);
                output.push_str(&line);
                output.push('\n');
            }
            let line = format!(
                "PC={:04X} SP={:04X} LR={:04X}",
                vm.pc, vm.regs[30], vm.regs[31]
            );
            println!("{}", line);
            output.push_str(&line);
            output.push('\n');
        }
        "peek" => {
            if parts.len() < 2 {
                let msg = "Usage: peek <addr>".to_string();
                println!("{}", msg);
                output.push_str(&msg);
                output.push('\n');
                return;
            }
            match u32::from_str_radix(
                parts[1].trim_start_matches("0x").trim_start_matches("0X"),
                16,
            ) {
                Ok(addr) if (addr as usize) < vm.ram.len() => {
                    let val = vm.ram[addr as usize];
                    let msg = format!("RAM[0x{:04X}] = 0x{:08X}", addr, val);
                    println!("{}", msg);
                    output.push_str(&msg);
                    output.push('\n');
                }
                Ok(addr) => {
                    let msg = format!("Address 0x{:04X} out of range", addr);
                    println!("{}", msg);
                    output.push_str(&msg);
                    output.push('\n');
                }
                Err(_) => {
                    let msg = "Invalid address".to_string();
                    println!("{}", msg);
                    output.push_str(&msg);
                    output.push('\n');
                }
            }
        }
        "poke" => {
            if parts.len() < 3 {
                let msg = "Usage: poke <addr> <val>".to_string();
                println!("{}", msg);
                output.push_str(&msg);
                output.push('\n');
                return;
            }
            let addr_str = parts[1].trim_start_matches("0x").trim_start_matches("0X");
            let val_str = parts[2].trim_start_matches("0x").trim_start_matches("0X");
            match (
                u32::from_str_radix(addr_str, 16),
                u32::from_str_radix(val_str, 16),
            ) {
                (Ok(addr), Ok(val)) if (addr as usize) < vm.ram.len() => {
                    vm.ram[addr as usize] = val;
                    let msg = format!("RAM[0x{:04X}] <- 0x{:08X}", addr, val);
                    println!("{}", msg);
                    output.push_str(&msg);
                    output.push('\n');
                }
                _ => {
                    let msg = "Usage: poke <hex_addr> <hex_val>".to_string();
                    println!("{}", msg);
                    output.push_str(&msg);
                    output.push('\n');
                }
            }
        }
        "screen" => {
            let start = if parts.len() >= 2 {
                u32::from_str_radix(
                    parts[1].trim_start_matches("0x").trim_start_matches("0X"),
                    16,
                )
                .unwrap_or(0) as usize
            } else {
                0
            };
            for row in 0..4 {
                let mut line = String::new();
                for col in 0..4 {
                    let idx = start + row * 4 + col;
                    if idx < vm::SCREEN_SIZE {
                        line.push_str(&format!("{:06X} ", vm.screen[idx] & 0xFFFFFF));
                    } else {
                        line.push_str("------ ");
                    }
                }
                println!("{}", line);
                output.push_str(&line);
                output.push('\n');
            }
        }
        "save" => {
            let filename = if parts.len() >= 2 {
                parts[1].to_string()
            } else {
                "output.ppm".to_string()
            };
            match std::fs::File::create(&filename) {
                Ok(mut f) => {
                    let header = "P6\n256 256\n255\n".to_string();
                    use std::io::Write;
                    let _ = f.write_all(header.as_bytes());
                    for pixel in &vm.screen {
                        let r = (pixel >> 16) & 0xFF;
                        let g = (pixel >> 8) & 0xFF;
                        let b = pixel & 0xFF;
                        let _ = f.write_all(&[r as u8, g as u8, b as u8]);
                    }
                    let msg = format!("Saved screen to {}", filename);
                    println!("{}", msg);
                    output.push_str(&msg);
                    output.push('\n');
                }
                Err(e) => {
                    let msg = format!("Error saving: {}", e);
                    println!("{}", msg);
                    output.push_str(&msg);
                    output.push('\n');
                }
            }
        }
        "png" => {
            let filename = if parts.len() >= 2 {
                parts[1].to_string()
            } else {
                "screenshot.png".to_string()
            };
            match save_screen_png(&filename, &vm.screen) {
                Ok(()) => {
                    let msg = format!("Saved screenshot to {}", filename);
                    println!("{}", msg);
                    output.push_str(&msg);
                    output.push('\n');
                }
                Err(e) => {
                    let msg = format!("Error saving PNG: {}", e);
                    println!("{}", msg);
                    output.push_str(&msg);
                    output.push('\n');
                }
            }
        }
        "reset" => {
            vm.reset();
            *canvas_assembled = false;
            let msg = "VM reset".to_string();
            println!("{}", msg);
            output.push_str(&msg);
            output.push('\n');
        }
        _ => {
            let msg = format!("Unknown: {} (skipped)", command);
            println!("{}", msg);
            output.push_str(&msg);
            output.push('\n');
        }
    }
}
