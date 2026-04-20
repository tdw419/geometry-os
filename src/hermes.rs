// hermes.rs -- Local LLM agent loop (Ollama) for Geometry OS

use crate::assembler;
use crate::canvas::{
    ensure_scroll, handle_terminal_command, list_asm_files, read_canvas_line, source_from_canvas,
    write_line_to_canvas,
};
use crate::preprocessor;
use crate::save::save_screen_png;
use crate::vm;
use geometry_os::episode_log;
use std::collections::HashSet;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

// --- Provider Configuration ---

/// LLM provider configuration. Loaded from provider.json in the project root.
/// Falls back to local Ollama if no config found.
#[derive(Clone)]
pub struct ProviderConfig {
    pub base_url: String,
    pub model: String,
    pub api_key: String,
    pub max_tokens: u32,
    pub temperature: f32,
    pub fallback: Option<Box<ProviderConfig>>,
}

impl ProviderConfig {
    /// Load from provider.json next to Cargo.toml, or use local Ollama defaults.
    pub fn load() -> Self {
        let root = get_project_root();
        let config_path = root.join("provider.json");
        if config_path.exists() {
            match std::fs::read_to_string(&config_path) {
                Ok(contents) => Self::from_json(&contents),
                Err(e) => {
                    println!(
                        "[hermes] Could not read provider.json: {}. Using local Ollama.",
                        e
                    );
                    Self::local_ollama()
                }
            }
        } else {
            println!("[hermes] No provider.json found. Using local Ollama.");
            Self::local_ollama()
        }
    }

    pub fn from_json(json: &str) -> Self {
        let base_url = extract_json_string(json, "base_url")
            .unwrap_or_else(|| "http://localhost:11434/api/chat".to_string());
        let model =
            extract_json_string(json, "model").unwrap_or_else(|| "qwen3.5-tools".to_string());
        let api_key = extract_json_string(json, "api_key").unwrap_or_default();
        let max_tokens = extract_json_number(json, "max_tokens").unwrap_or(8192);
        let temperature = extract_json_float(json, "temperature").unwrap_or(0.3);

        let fallback = if let Some(fb_start) = json.find("\"fallback\"") {
            if let Some(obj_start) = json[fb_start..].find('{') {
                let fb_json = &json[fb_start + obj_start..];
                Some(Box::new(ProviderConfig {
                    base_url: extract_json_string(fb_json, "base_url")
                        .unwrap_or_else(|| "http://localhost:11434/api/chat".to_string()),
                    model: extract_json_string(fb_json, "model")
                        .unwrap_or_else(|| "qwen3.5-tools".to_string()),
                    api_key: extract_json_string(fb_json, "api_key").unwrap_or_default(),
                    max_tokens: extract_json_number(fb_json, "max_tokens").unwrap_or(8192),
                    temperature: extract_json_float(fb_json, "temperature").unwrap_or(0.3),
                    fallback: None,
                }))
            } else {
                None
            }
        } else {
            None
        };

        println!("[hermes] Loaded provider: model={} url={}", model, base_url);
        ProviderConfig {
            base_url,
            model,
            api_key,
            max_tokens,
            temperature,
            fallback,
        }
    }

    pub fn local_ollama() -> Self {
        ProviderConfig {
            base_url: "http://localhost:11434/api/chat".to_string(),
            model: "qwen3.5-tools".to_string(),
            api_key: String::new(),
            max_tokens: 8192,
            temperature: 0.3,
            fallback: None,
        }
    }

    pub fn label(&self) -> String {
        format!("{} @ {}", self.model, self.base_url)
    }
}

/// Extract a string value from JSON by key. Minimal parser.
fn extract_json_string(json: &str, key: &str) -> Option<String> {
    let pattern = format!("\"{}\"", key);
    let start = json.find(&pattern)?;
    let after_key = &json[start + pattern.len()..];
    let after_colon = after_key.trim_start().strip_prefix(':')?;
    let after_colon = after_colon.trim_start();
    if !after_colon.starts_with('"') {
        return None;
    }
    let bytes = after_colon.as_bytes();
    let mut i = 1; // skip opening quote
    let mut result = String::new();
    while i < bytes.len() {
        if bytes[i] == b'\\' && i + 1 < bytes.len() {
            match bytes[i + 1] {
                b'"' => result.push('"'),
                b'\\' => result.push('\\'),
                b'n' => result.push('\n'),
                b't' => result.push('\t'),
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
}

/// Extract a numeric value from JSON by key.
fn extract_json_number(json: &str, key: &str) -> Option<u32> {
    let pattern = format!("\"{}\"", key);
    let start = json.find(&pattern)?;
    let after_key = &json[start + pattern.len()..];
    let after_colon = after_key.trim_start().strip_prefix(':')?;
    let after_colon = after_colon.trim_start();
    let end = after_colon
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(after_colon.len());
    after_colon[..end].parse().ok()
}

/// Extract a float value from JSON by key.
fn extract_json_float(json: &str, key: &str) -> Option<f32> {
    let pattern = format!("\"{}\"", key);
    let start = json.find(&pattern)?;
    let after_key = &json[start + pattern.len()..];
    let after_colon = after_key.trim_start().strip_prefix(':')?;
    let after_colon = after_colon.trim_start();
    let end = after_colon
        .find(|c: char| !c.is_ascii_digit() && c != '.')
        .unwrap_or(after_colon.len());
    after_colon[..end].parse().ok()
}

/// Get the project root directory (where Cargo.toml lives).
fn get_project_root() -> PathBuf {
    let mut dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    loop {
        if dir.join("Cargo.toml").exists() {
            return dir;
        }
        if !dir.pop() {
            return std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        }
    }
}

/// Check if a path is safe to write (within the project root or /tmp).
fn validate_write_path(filename: &str) -> Result<PathBuf, String> {
    let path = Path::new(filename);
    if filename.contains("..") {
        return Err("Path traversal (..) not allowed".to_string());
    }
    if path.is_absolute() {
        let root = get_project_root();
        let canonical_root = root.canonicalize().unwrap_or(root);
        let canonical_path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        if canonical_path.starts_with("/tmp") {
            return Ok(path.to_path_buf());
        }
        if canonical_path.starts_with(&canonical_root) {
            return Ok(path.to_path_buf());
        }
        return Err(format!(
            "Write outside project root not allowed: {}",
            filename
        ));
    }
    let root = get_project_root();
    let resolved = root.join(path);
    let canonical_root = root.canonicalize().unwrap_or(root);
    let canonical_resolved = resolved.canonicalize().unwrap_or(resolved.clone());
    if canonical_resolved.starts_with(&canonical_root) {
        return Ok(resolved);
    }
    Err(format!(
        "Write outside project root not allowed: {} (resolves to {:?})",
        filename, canonical_resolved
    ))
}

/// Safely write a file with path validation.
fn safe_write_file(filename: &str, content: &str) -> Result<usize, String> {
    let validated_path = validate_write_path(filename)?;
    std::fs::write(&validated_path, content)
        .map(|_| content.len())
        .map_err(|e| format!("Write error for {}: {}", filename, e))
}

/// Get git diff stat of uncommitted changes.
fn get_git_diff() -> String {
    match std::process::Command::new("git")
        .args(["diff", "--stat"])
        .current_dir(get_project_root())
        .output()
    {
        Ok(output) => {
            let out = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if out.is_empty() {
                "(no uncommitted changes)".to_string()
            } else {
                out
            }
        }
        Err(e) => format!("git diff failed: {}", e),
    }
}

/// Auto-commit changes.
fn auto_commit(message: &str) -> String {
    let root = get_project_root();
    let has_changes = std::process::Command::new("git")
        .args(["diff", "--quiet"])
        .current_dir(&root)
        .output()
        .map(|o| !o.status.success())
        .unwrap_or(false);
    if !has_changes {
        let has_untracked = std::process::Command::new("git")
            .args(["ls-files", "--others", "--exclude-standard"])
            .current_dir(&root)
            .output()
            .map(|o| !String::from_utf8_lossy(&o.stdout).trim().is_empty())
            .unwrap_or(false);
        if !has_untracked {
            return "(no changes to commit)".to_string();
        }
    }
    if let Err(e) = std::process::Command::new("git")
        .args(["add", "-A"])
        .current_dir(&root)
        .output()
    {
        return format!("git add failed: {}", e);
    }
    let msg = format!(
        "[build-agent] {}",
        if message.len() > 72 {
            &message[..72]
        } else {
            message
        }
    );
    match std::process::Command::new("git")
        .args(["commit", "-m", &msg])
        .current_dir(&root)
        .output()
    {
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            if output.status.success() {
                format!("Committed: {}", msg)
            } else {
                format!("Commit failed: {}", stderr)
            }
        }
        Err(e) => format!("git commit failed: {}", e),
    }
}

/// Rollback uncommitted changes.
fn git_rollback() -> String {
    let root = get_project_root();
    let mut results = Vec::new();
    match std::process::Command::new("git")
        .args(["checkout", "--", "."])
        .current_dir(&root)
        .output()
    {
        Ok(output) if output.status.success() => {
            results.push("Discarded tracked changes".to_string())
        }
        Ok(output) => results.push(format!(
            "checkout failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )),
        Err(e) => results.push(format!("checkout error: {}", e)),
    }
    match std::process::Command::new("git")
        .args(["clean", "-fd"])
        .current_dir(&root)
        .output()
    {
        Ok(output) if output.status.success() => {
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !stdout.is_empty() {
                results.push(format!("Removed: {}", stdout));
            }
        }
        Ok(output) => results.push(format!(
            "clean failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )),
        Err(e) => results.push(format!("clean error: {}", e)),
    }
    results.join("; ")
}

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
                    match safe_write_file(&wb.0, &wb.1) {
                        Ok(bytes) => {
                            let msg = format!("Wrote {} ({} bytes)", wb.0, bytes);
                            *output_row = write_line_to_canvas(canvas_buffer, *output_row, &msg);
                            output_capture.push_str(&msg);
                            output_capture.push('\n');
                        }
                        Err(e) => {
                            let msg = format!("Write blocked: {}", e);
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
                    if let Err(e) = validate_write_path(filename) {
                        let msg = format!("[hermes] Write blocked: {}", e);
                        *output_row = write_line_to_canvas(canvas_buffer, *output_row, &msg);
                        output_capture.push_str(&msg);
                        output_capture.push('\n');
                    } else {
                        write_buffer = Some((filename.to_string(), String::new()));
                    }
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
            match safe_write_file(&wb.0, &wb.1) {
                Ok(bytes) => {
                    *output_row = write_line_to_canvas(
                        canvas_buffer,
                        *output_row,
                        &format!("Wrote {} ({} bytes)", wb.0, bytes),
                    );
                }
                Err(e) => {
                    *output_row = write_line_to_canvas(
                        canvas_buffer,
                        *output_row,
                        &format!("Write blocked: {}", e),
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
          LOADS reg, addr_r | STORES addr_r, reg (string ops)
ALU:      ADD rd, rs | SUB rd, rs | MUL rd, rs | DIV rd, rs | MOD rd, rs
          AND rd, rs | OR rd, rs | XOR rd, rs | SHL rd, rs | SHR rd, rs | NEG rd
          NOT rd | SAR rd, rs | MIN rd, rs | MAX rd, rs
Immediate: ADDI rd, imm | SUBI rd, imm | CMPI rd, imm | ANDI rd, imm | ORI rd, imm
          XORI rd, imm | SHLI rd, imm | SHRI rd, imm | SARI rd, imm
Compare:  CMP rd, rs (sets r0 = -1 if rd<rs, 0 if ==, 1 if rd>rs)
Branch:   JMP label | JZ reg, label | JNZ reg, label
          BLT r0, label (branch if r0==0xFFFFFFFF) | BGE r0, label (branch if r0!=0xFFFFFFFF)
Stack:    PUSH reg | POP reg (SP=r30, grows down)
Call:     CALL label | RET (return addr in r31)
Pixel:    PSET xr, yr, cr | PSETI x, y, color | FILL cr
          RECTF xr, yr, wr, hr, cr | TEXT xr, yr, ar
          LINE x0r, y0r, x1r, y1r, cr | CIRCLE xr, yr, rr, cr
          SPRITE xr, yr, addr_r, wr, hr | PEEK xr, yr, dr
Other:    SCROLL nr | IKEY reg | RAND reg | FRAME | BEEP freq_r, dur_r | NOTE wave_r, freq_r, dur_r
          SPAWN reg | KILL reg | ASM src_r, dest_r | YIELD | SLEEP reg | GETPID reg
          SIGNAL sig_reg, pid_reg | SIGSET handler_reg | WAITPID pid_reg
          OPEN path_r, mode_r | READ fd_r, buf_r, len_r | WRITE fd_r, buf_r, len_r | CLOSE fd_r
          SEEK fd_r, offset_r | IOCTL fd_r, cmd_r
          MEMCPY dst_r, src_r, len_r
          TEXTI xr, yr, addr_r, len_r | STRO addr_r, char_r | SCREENP xr, yr, dr
          TILEMAP xr, yr, map_r, tw_r, th_r | REPLAY slot_r | SNAP_TRACE
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

## Diagnostic discipline
You are a DIAGNOSTIC agent, not a creative one. When something is wrong, your job is to FIND OUT WHY, not to paint over it.
- If the framebuffer is 100% black after execution, do NOT write a new program. Read the opcode summary and diagnose.
- If a program doesn't produce expected output, check registers with regs, peek at RAM with peek, examine the screen with screen.
- Verify your assumptions before acting on them. A black screen means either (a) nothing was drawn, or (b) something was drawn then erased. The opcode summary tells you which.
- NEVER replace a broken program with a new one. Fix the broken one. That's how you learn.

## Response format
Respond with one geo> command per line. No explanation, no markdown, no backticks.
Just the commands you want executed. You can also write new .asm programs by using
the write command:
  write <filename.asm>  (then subsequent lines are the file content, end with ENDWRITE on its own line)

After your commands run, you'll see the output and can issue more commands.
Think step by step but only output commands."#;

pub const HERMES_BUILD_SYSTEM_PROMPT: &str = r#"You are an agent that modifies the Geometry OS Rust source code to add features and fix bugs. You have full access to read source files, write changes, and run builds.

## Project Structure (modular)
- src/vm/ -- Virtual Machine core (mod.rs, ops_extended.rs, ops_graphics.rs, ops_memory.rs, ops_syscall.rs, disasm.rs, scheduler.rs, formula.rs, memory.rs, io.rs, trace.rs, boot.rs, types.rs)
- src/assembler/ -- Two-pass assembler (mod.rs, core_ops.rs, graphics_ops.rs, immediate_ops.rs, system_ops.rs, formula_ops.rs, instructions.rs, includes.rs)
- src/riscv/ -- RISC-V RV32IMAC interpreter (mod.rs, boot.rs, bus.rs, mmu.rs, loader.rs, sbi.rs, syscall.rs, clint.rs, plic.rs, uart.rs, virtio_blk.rs, dtb.rs, bridge.rs, trace.rs, tests.rs)
- src/qemu/ -- QEMU bridge (mod.rs, bridge.rs, config.rs, ansi.rs, cursor.rs)
- src/main.rs -- GUI window, rendering, input, terminal mode
- src/canvas.rs -- Canvas buffer, terminal command handler, file loading
- src/hermes.rs -- LLM agent loop (this file)
- src/cli.rs -- CLI mode (headless REPL)
- src/preprocessor.rs -- Macro expansion (VAR/SET/GET), .include/.lib directives
- src/inode_fs.rs -- In-memory inode filesystem
- src/vfs.rs -- Virtual filesystem layer
- src/font.rs -- 8x8 VGA bitmaps
- src/save.rs -- PNG/PPM save
- src/render.rs -- Rendering pipeline
- src/audio.rs -- Sound (BEEP/NOTE opcodes)
- src/keys.rs -- Keyboard input
- src/pixel.rs -- Pixel color utilities
- src/fuzzer.rs -- VM fuzzer
- src/riscv_fuzzer.rs -- RISC-V fuzzer
- src/lib.rs -- pub mod declarations
- tests/program_tests/ -- Integration tests (basic_programs, opcodes, games, multiprocess, filesystem, shell, kernel, hypervisor, etc.)
- tests/riscv_tests.rs -- RISC-V tests
- tests/trace_tests.rs -- Trace tests

## Commands available
- readfile <path>        Read a source file (shows up to 3000 chars)
- files                  List all .rs files with line counts
- write <path>           Start writing a file (subsequent lines = content, ENDWRITE to finish)
- shell <command>        Run a shell command (cargo build, cargo test, grep, etc.)
- diff                   Show uncommitted changes (git diff --stat)
- commit [message]       Auto-commit all changes (git add -A + commit)
- rollback               Discard all uncommitted changes (git checkout + clean)
- load/run/regs/peek/poke/screen/reset  Standard VM commands

## Build workflow
1. readfile to understand the code you need to modify
2. shell grep -rn "pattern" src/ to find specific locations (use -rn for recursive search across modules)
3. write the modified file (use write + content + ENDWRITE)
4. shell cargo build to verify compilation
5. shell cargo test to verify all tests pass
6. If tests fail, readfile the error output, fix, and retry

## Key VM Architecture
- 64K u32 RAM, 32 registers (r0-r31)
- r0 = CMP result register, r30 = SP, r31 = LR
- All ALU ops: 2-argument form (ADD rd, rs means rd = rd + rs)
- Adding a new opcode:
  1. Add bytecode handler in src/vm/ops_*.rs (pick the right file based on opcode category)
  2. Add disassembler entry in src/vm/disasm.rs
  3. Add assembler entry in src/assembler/*_ops.rs (pick the right file)
  4. Add any needed preprocessor macros in src/preprocessor.rs
  5. Write tests in tests/program_tests/
- Write tests in tests/program_tests/ (organized by category: opcodes.rs, games.rs, filesystem.rs, etc.)

## Safety rules
- ALL writes are sandboxed to the project root directory (no escaping via .. or absolute paths)
- /tmp writes are allowed for build artifacts
- ALWAYS run shell cargo build after writing any Rust file
- ALWAYS run shell cargo test to verify nothing breaks
- Read files BEFORE modifying them -- understand the existing code
- Do NOT modify Cargo.toml or add new dependencies
- Keep changes minimal and focused on the requested feature
- Use `diff` to review changes before committing
- Use `commit <message>` to save progress
- Use `rollback` to discard changes if something goes wrong

## Response format
Respond with one command per line. No explanation, no markdown, no backticks.
Just the commands you want executed, in order."#;

/// Human-readable mnemonic for an opcode byte.
/// Derived from src/vm/disasm.rs -- the authoritative ISA mapping.
/// Any gap = opcode not yet assigned (shows as "???").
fn opcode_name(op: u8) -> &'static str {
    match op {
        0x00 => "HALT",
        0x01 => "NOP",
        0x02 => "FRAME",
        0x03 => "BEEP",
        0x04 => "MEMCPY",
        0x10 => "LDI",
        0x11 => "LOAD",
        0x12 => "STORE",
        0x13 => "TEXTI",
        0x14 => "STRO",
        0x15 => "CMPI",
        0x16 => "LOADS",
        0x17 => "STORES",
        0x18 => "SHLI",
        0x19 => "SHRI",
        0x1A => "SARI",
        0x1B => "ADDI",
        0x1C => "SUBI",
        0x1D => "ANDI",
        0x1E => "ORI",
        0x1F => "XORI",
        0x20 => "ADD",
        0x21 => "SUB",
        0x22 => "MUL",
        0x23 => "DIV",
        0x24 => "AND",
        0x25 => "OR",
        0x26 => "XOR",
        0x27 => "SHL",
        0x28 => "SHR",
        0x29 => "MOD",
        0x2A => "NEG",
        0x2B => "SAR",
        0x30 => "JMP",
        0x31 => "JZ",
        0x32 => "JNZ",
        0x33 => "CALL",
        0x34 => "RET",
        0x35 => "BLT",
        0x36 => "BGE",
        0x40 => "PSET",
        0x41 => "PSETI",
        0x42 => "FILL",
        0x43 => "RECTF",
        0x44 => "TEXT",
        0x45 => "LINE",
        0x46 => "CIRCLE",
        0x47 => "SCROLL",
        0x48 => "IKEY",
        0x49 => "RAND",
        0x4A => "SPRITE",
        0x4B => "ASM",
        0x4C => "TILEMAP",
        0x4D => "SPAWN",
        0x4E => "KILL",
        0x4F => "PEEK",
        0x50 => "CMP",
        0x51 => "MOV",
        0x52 => "SYSCALL",
        0x53 => "RETK",
        0x54 => "OPEN",
        0x55 => "READ",
        0x56 => "WRITE",
        0x57 => "CLOSE",
        0x58 => "SEEK",
        0x59 => "LS",
        0x5A => "YIELD",
        0x5B => "SLEEP",
        0x5C => "SETPRIORITY",
        0x5D => "PIPE",
        0x5E => "MSGSND",
        0x5F => "MSGRCV",
        0x60 => "PUSH",
        0x61 => "POP",
        0x62 => "IOCTL",
        0x63 => "GETENV",
        0x64 => "SETENV",
        0x65 => "GETPID",
        0x66 => "EXEC",
        0x67 => "WRITESTR",
        0x68 => "READLN",
        0x69 => "WAITPID",
        0x6A => "EXECP",
        0x6B => "CHDIR",
        0x6C => "GETCWD",
        0x6D => "SCREENP",
        0x6E => "SHUTDOWN",
        0x6F => "EXIT",
        0x70 => "SIGNAL",
        0x71 => "SIGSET",
        0x72 => "HYPERVISOR",
        0x73 => "ASMSELF",
        0x74 => "RUNNEXT",
        0x75 => "FORMULA",
        0x76 => "FORMULACLEAR",
        0x77 => "FORMULAREM",
        0x78 => "FMKDIR",
        0x79 => "FSTAT",
        0x7A => "FUNLINK",
        0x7B => "SNAP_TRACE",
        0x7C => "REPLAY",
        0x7D => "FORK",
        0x7E => "NOTE",
        0x7F => "CONNECT",
        0x80 => "SOCKSEND",
        0x81 => "SOCKRECV",
        0x82 => "DISCONNECT",
        0x83 => "TRACE_READ",
        0x84 => "PIXEL_HISTORY",
        _ => "???",
    }
}

/// Build top-N opcode list from VM histogram (with mnemonic names).
fn build_top_opcodes(vm: &vm::Vm, n: usize) -> Vec<(u8, String, u64, f64)> {
    let total_ops: u64 = vm.opcode_histogram.iter().sum();
    let mut ranked: Vec<(u8, u64)> = vm
        .opcode_histogram
        .iter()
        .enumerate()
        .filter(|(_, &c)| c > 0)
        .map(|(op, &c)| (op as u8, c))
        .collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1));
    ranked
        .iter()
        .take(n)
        .map(|(op, count)| {
            let name = opcode_name(*op).to_string();
            let pct = if total_ops > 0 {
                *count as f64 / total_ops as f64 * 100.0
            } else {
                0.0
            };
            (*op, name, *count, pct)
        })
        .collect()
}

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

    // Screen delta detection: warn if framebuffer is all zeros post-execution
    if !vm.screen.is_empty() {
        let non_black: usize = vm.screen.iter().filter(|&&p| p != 0).count();
        if non_black == 0 && vm.halted {
            ctx.push_str(
                "\nWARNING: Framebuffer is 100% black after execution completed (halted).\n",
            );
            ctx.push_str("Possible causes: premature FILL/CLEAR before HALT, draw calls before screen init,\n");
            ctx.push_str("or program halted mid-animation loop (strobe effect). Check opcode summary below.\n");
        } else if non_black > 0 {
            let pct = non_black as f64 / vm.screen.len() as f64 * 100.0;
            ctx.push_str(&format!(
                "\nFramebuffer: {}/{} pixels drawn ({:.1}%)\n",
                non_black,
                vm.screen.len(),
                pct
            ));
        }
    }

    // Opcode execution summary: top opcodes by count (diagnostic for the AI agent)
    let total_ops: u64 = vm.opcode_histogram.iter().sum();
    if total_ops > 0 {
        ctx.push_str(&format!(
            "\n## Opcode Execution Summary ({} total instructions)\n",
            total_ops
        ));
        // Collect (opcode, count) pairs, sort by count descending, show top 15
        let mut ranked: Vec<(u8, u64)> = vm
            .opcode_histogram
            .iter()
            .enumerate()
            .filter(|(_, &c)| c > 0)
            .map(|(op, &c)| (op as u8, c))
            .collect();
        ranked.sort_by(|a, b| b.1.cmp(&a.1));
        for (op, count) in ranked.iter().take(15) {
            let name = opcode_name(*op);
            let pct = *count as f64 / total_ops as f64 * 100.0;
            ctx.push_str(&format!(
                "  {:04X} {:<12} {:>6} ({:.1}%)\n",
                op, name, count, pct
            ));
        }
        if ranked.len() > 15 {
            ctx.push_str(&format!("  ... and {} more opcodes\n", ranked.len() - 15));
        }
    }

    // Loaded file
    if let Some(ref f) = loaded_file {
        ctx.push_str(&format!("\n## Loaded file: {}\n", f.display()));

        // Inject episodic memory: past run history for this program
        if let Some(name) = f.file_name() {
            let prog_name = name.to_string_lossy();
            let episode_ctx = episode_log::format_episode_context(&prog_name, 10);
            if !episode_ctx.is_empty() {
                ctx.push_str(&episode_ctx);
            }
        }
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

/// Build context for the build agent (project files, git log, test status)
/// Downsample 256x256 screen to ASCII art (48 cols x 24 rows).
/// Uses luminance: 0.299*R + 0.587*G + 0.114*B mapped to 10-level grayscale ramp.
fn screen_to_ascii(screen: &[u32]) -> String {
    const COLS: usize = 48;
    const ROWS: usize = 24;
    const W: usize = 256;
    const H: usize = 256;
    const CELL_W: usize = W / COLS; // 5
    const CELL_H: usize = H / ROWS; // ~10
    const RAMP: &[u8] = b" .:-=+*#%@";

    let mut out = String::with_capacity(ROWS * (COLS + 1));
    for row in 0..ROWS {
        for col in 0..COLS {
            let sx = (col * CELL_W + CELL_W / 2).min(W - 1);
            let sy = (row * CELL_H + CELL_H / 2).min(H - 1);
            let px = screen[sy * W + sx];
            if px == 0 {
                out.push(' ');
                continue;
            }
            let r = ((px >> 16) & 0xFF) as f32;
            let g = ((px >> 8) & 0xFF) as f32;
            let b = (px & 0xFF) as f32;
            let lum = 0.299 * r + 0.587 * g + 0.114 * b;
            let idx = ((lum / 255.0) * (RAMP.len() - 1) as f32).round() as usize;
            out.push(RAMP[idx.min(RAMP.len() - 1)] as char);
        }
        out.push('\n');
    }
    out
}

fn build_build_context(vm: &vm::Vm) -> String {
    let mut ctx = String::new();

    // Screen state -- let the AI see what it drew
    ctx.push_str("## Current Screen (48x24 ASCII downsample of 256x256 framebuffer)\n");
    if !vm.screen.is_empty() {
        ctx.push_str("```\n");
        ctx.push_str(&screen_to_ascii(&vm.screen));
        ctx.push_str("```\n");
        // Quick stats
        let non_black = vm.screen.iter().filter(|&&p| p != 0).count();
        let pct = (non_black as f64 / vm.screen.len() as f64 * 100.0) as u32;
        ctx.push_str(&format!(
            "  {}% pixels non-black, PC=0x{:04X}, halted={}\n",
            pct, vm.pc, vm.halted
        ));
    } else {
        ctx.push_str("  (screen not initialized)\n");
    }

    // Pixel Provenance -- trace data for the agent to reason about
    if !vm.pixel_write_log.is_empty() {
        ctx.push_str("\n## Pixel Provenance\n");
        let pw_len = vm.pixel_write_log.len();
        let pw_cap = vm.pixel_write_log.capacity();
        let evicted = pw_len >= pw_cap;
        ctx.push_str(&format!(
            "  {}/{} pixel writes{}. Use 'who_wrote <x> <y>' to query any pixel.\n",
            pw_len,
            pw_cap,
            if evicted {
                " -- OLDER WRITES EVICTED, early history may be incomplete"
            } else {
                ""
            }
        ));
        ctx.push_str(&format!(
            "  {} frame checkpoints, {} VM snapshots, {} trace entries.\n",
            vm.frame_checkpoints.len(),
            vm.snapshots.len(),
            vm.trace_buffer.len()
        ));
    }

    // List source files with line counts
    ctx.push_str("\n## Project Files\n");
    let mut total_lines = 0;
    let mut file_count = 0;
    if let Ok(rd) = std::fs::read_dir("src") {
        let mut entries: Vec<_> = rd
            .flatten()
            .filter(|e| e.path().extension().map(|ext| ext == "rs").unwrap_or(false))
            .collect();
        entries.sort_by_key(|e| e.file_name());
        for entry in &entries {
            if let Ok(content) = std::fs::read_to_string(entry.path()) {
                let lines = content.lines().count();
                let name = entry.file_name().to_string_lossy().to_string();
                ctx.push_str(&format!("  src/{:<25} {:>5} lines\n", name, lines));
                total_lines += lines;
                file_count += 1;
            }
        }
    }
    ctx.push_str(&format!(
        "  {} files, {} total lines\n",
        file_count, total_lines
    ));

    // Recent git log
    ctx.push_str("\n## Recent Changes\n");
    if let Ok(output) = std::process::Command::new("git")
        .args(["log", "--oneline", "-5"])
        .output()
    {
        let log = String::from_utf8_lossy(&output.stdout);
        if !log.is_empty() {
            ctx.push_str(&log);
        } else {
            ctx.push_str("  (no git history)\n");
        }
    }

    ctx
}

/// Shared command execution for build loops. Takes a command line, executes it,
/// and returns the captured output string.
fn execute_build_command(
    cmd_line: &str,
    vm: &mut vm::Vm,
    source_text: &mut String,
    loaded_file: &mut Option<PathBuf>,
    canvas_assembled: &mut bool,
) -> String {
    let mut output = String::new();
    execute_cli_command(
        cmd_line,
        vm,
        source_text,
        loaded_file,
        canvas_assembled,
        &mut output,
    );
    output
}

/// Core LLM call -- provider-agnostic. Works with any OpenAI-compatible endpoint.
/// Handles: Ollama, ZAI, OpenAI, Anthropic (via compatible layer), etc.
pub fn call_llm(
    config: &ProviderConfig,
    system_prompt: &str,
    user_message: &str,
) -> Option<String> {
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

    // Build OpenAI-compatible payload
    let payload = format!(
        r#"{{"model":"{}","messages":[{{"role":"system","content":"{}"}},{{"role":"user","content":"{}"}}],"stream":false,"max_tokens":{},"temperature":{}}}"#,
        config.model, esc_sys, esc_user, config.max_tokens, config.temperature
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

    // Build curl args -- add Authorization header if API key present
    let mut curl_args = vec![
        "-s".to_string(),
        "-X".to_string(),
        "POST".to_string(),
        config.base_url.clone(),
        "-d".to_string(),
        format!("@{}", tmp_path),
        "-H".to_string(),
        "Content-Type: application/json".to_string(),
        "--max-time".to_string(),
        "120".to_string(),
    ];
    if !config.api_key.is_empty() {
        curl_args.push("-H".to_string());
        curl_args.push(format!("Authorization: Bearer {}", config.api_key));
    }

    let output = match std::process::Command::new("curl").args(&curl_args).output() {
        Ok(o) => o,
        Err(e) => {
            println!("[hermes] curl failed: {}", e);
            return None;
        }
    };

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Check for HTTP errors
    if !output.status.success() {
        println!("[hermes] curl exit code: {}", output.status);
    }
    if stdout.contains("\"error\"") {
        // Try to extract error message
        if let Some(e) = extract_json_string(&stdout, "message") {
            println!("[hermes] API error: {}", e);
        } else {
            println!(
                "[hermes] API error: {}...",
                &stdout[..stdout.len().min(200)]
            );
        }
        // Try fallback if available
        if let Some(ref fb) = config.fallback {
            println!("[hermes] Trying fallback: {}", fb.label());
            return call_llm(fb, system_prompt, user_message);
        }
        return None;
    }

    // Parse response -- extract message.content (works for both Ollama and OpenAI formats)
    // Try "content" field (Ollama format: {"message":{"content":"..."}})
    // Also works for OpenAI format: {"choices":[{"message":{"content":"..."}}]}
    if let Some(start) = stdout.find(r#""content":""#) {
        let content_start = start + r#""content":""#.len();
        let mut i = content_start;
        let mut result = String::new();
        let bytes = stdout.as_bytes();
        while i < bytes.len() {
            if bytes[i] == b'\\' && i + 1 < bytes.len() {
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
        // Try fallback if available
        if let Some(ref fb) = config.fallback {
            println!("[hermes] Trying fallback: {}", fb.label());
            return call_llm(fb, system_prompt, user_message);
        }
        None
    }
}

/// Backward-compatible wrapper: load provider config and call LLM.
pub fn call_ollama(system_prompt: &str, user_message: &str) -> Option<String> {
    let config = ProviderConfig::load();
    call_llm(&config, system_prompt, user_message)
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

        // Debug: show if screen vision was included
        let non_black = vm.screen.iter().filter(|&&p| p != 0).count();
        println!(
            "[hermes-vision] screen pixels non-zero: {}/{}",
            non_black,
            vm.screen.len()
        );
        if ctx.contains("## Screen") {
            println!("[hermes-vision] Screen dump included in context");
        } else {
            println!("[hermes-vision] No screen data in context");
        }

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
                    match safe_write_file(&wb.0, &wb.1) {
                        Ok(bytes) => {
                            let msg = format!("Wrote {} ({} bytes)", wb.0, bytes);
                            println!("{}", msg);
                            output_capture.push_str(&msg);
                            output_capture.push('\n');
                        }
                        Err(e) => {
                            let msg = format!("Write blocked: {}", e);
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
                    if let Err(e) = validate_write_path(filename) {
                        println!("[hermes] Write blocked: {}", e);
                        output_capture.push_str(&format!("[hermes] Write blocked: {}\n", e));
                    } else {
                        write_buffer = Some((filename.to_string(), String::new()));
                    }
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
            match safe_write_file(&wb.0, &wb.1) {
                Ok(bytes) => println!("Wrote {} ({} bytes)", wb.0, bytes),
                Err(e) => println!("Write blocked: {}", e),
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

                    // Enable trace recording for pixel provenance + time-travel
                    vm.trace_recording = true;

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

                    // Stop recording (but keep data for queries)
                    vm.trace_recording = false;
                    let msg = if vm.halted {
                        format!("Halted at PC=0x{:04X}", vm.pc)
                    } else {
                        format!("Running... PC=0x{:04X}", vm.pc)
                    };
                    println!("{}", msg);
                    output.push_str(&msg);
                    output.push('\n');
                    *canvas_assembled = true;

                    // Log diagnostic episode for cross-session memory
                    let prog_name = loaded_file
                        .as_ref()
                        .and_then(|p| p.file_name())
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_else(|| "unknown".to_string());
                    let total_ops: u64 = vm.opcode_histogram.iter().sum();
                    let screen_non_black = vm.screen.iter().filter(|&&p| p != 0).count();
                    let top_ops = build_top_opcodes(vm, 10);
                    let episode = episode_log::build_episode_raw(
                        &prog_name,
                        total_ops,
                        top_ops,
                        screen_non_black,
                        vm.screen.len(),
                        vm.pc,
                        vm.halted,
                        None, // fix is set later by the agent
                    );
                    episode_log::log_episode(&episode);
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
        "shell" => {
            // Execute host shell command, capture stdout+stderr
            let cmd_rest = parts[1..].join(" ");
            if cmd_rest.is_empty() {
                let msg = "Usage: shell <command>".to_string();
                println!("{}", msg);
                output.push_str(&msg);
                output.push('\n');
                return;
            }
            match std::process::Command::new("sh")
                .arg("-c")
                .arg(&cmd_rest)
                .current_dir(
                    std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")),
                )
                .output()
            {
                Ok(out) => {
                    let stdout = String::from_utf8_lossy(&out.stdout);
                    let stderr = String::from_utf8_lossy(&out.stderr);
                    let mut combined = String::new();
                    if !stdout.is_empty() {
                        combined.push_str(&stdout);
                    }
                    if !stderr.is_empty() {
                        if !combined.is_empty() {
                            combined.push('\n');
                        }
                        combined.push_str("[stderr] ");
                        combined.push_str(&stderr);
                    }
                    // Truncate to 2000 chars
                    if combined.len() > 2000 {
                        combined.truncate(2000);
                        combined.push_str("\n... (truncated)");
                    }
                    let msg = if out.status.success() {
                        format!("[exit 0] {}", combined.trim())
                    } else {
                        format!(
                            "[exit {}] {}",
                            out.status.code().unwrap_or(-1),
                            combined.trim()
                        )
                    };
                    println!("{}", msg);
                    output.push_str(&msg);
                    output.push('\n');
                }
                Err(e) => {
                    let msg = format!("Shell error: {}", e);
                    println!("{}", msg);
                    output.push_str(&msg);
                    output.push('\n');
                }
            }
        }
        "readfile" => {
            if parts.len() < 2 {
                let msg = "Usage: readfile <path>".to_string();
                println!("{}", msg);
                output.push_str(&msg);
                output.push('\n');
                return;
            }
            let path = parts[1..].join(" ");
            match std::fs::read_to_string(&path) {
                Ok(content) => {
                    let total_lines = content.lines().count();
                    let total_chars = content.len();
                    // Truncate to 3000 chars
                    let display = if content.len() > 3000 {
                        let truncated: String = content.chars().take(3000).collect();
                        format!(
                            "{}...\n[{} lines, {} chars total, showing first 3000]",
                            truncated, total_lines, total_chars
                        )
                    } else {
                        content
                    };
                    println!("{}", display);
                    output.push_str(&display);
                    output.push('\n');
                }
                Err(e) => {
                    let msg = format!("Error reading {}: {}", path, e);
                    println!("{}", msg);
                    output.push_str(&msg);
                    output.push('\n');
                }
            }
        }
        "files" => {
            // List .rs files with line counts
            let mut entries = Vec::new();
            if let Ok(rd) = std::fs::read_dir("src") {
                for entry in rd.flatten() {
                    let p = entry.path();
                    if p.extension().map(|e| e == "rs").unwrap_or(false) {
                        if let Ok(content) = std::fs::read_to_string(&p) {
                            let lines = content.lines().count();
                            let name = p.file_name().unwrap().to_string_lossy();
                            entries.push((name.to_string(), lines));
                        }
                    }
                }
            }
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            let mut total = 0;
            for (name, lines) in &entries {
                let msg = format!("  src/{:<25} {:>5} lines", name, lines);
                println!("{}", msg);
                output.push_str(&msg);
                output.push('\n');
                total += lines;
            }
            let msg = format!("  {} source files, {} total lines", entries.len(), total);
            println!("{}", msg);
            output.push_str(&msg);
            output.push('\n');
        }
        "diff" => {
            let out = get_git_diff();
            println!("{}", out);
            output.push_str(&out);
            output.push('\n');
        }
        "commit" => {
            let msg = if parts.len() > 1 {
                parts[1..].join(" ")
            } else {
                "build agent changes".to_string()
            };
            let out = auto_commit(&msg);
            println!("{}", out);
            output.push_str(&out);
            output.push('\n');
        }
        "who_wrote" => {
            // who_wrote <x> <y> -- pixel provenance query
            // Shows which instructions wrote to pixel (x,y), newest first
            if parts.len() < 3 {
                let msg = "Usage: who_wrote <x> <y>".to_string();
                println!("{}", msg);
                output.push_str(&msg);
                output.push('\n');
                return;
            }
            let x: u16 = match parts[1].parse() {
                Ok(v) if v < 256 => v,
                _ => {
                    let msg = "x must be 0-255".to_string();
                    println!("{}", msg);
                    output.push_str(&msg);
                    output.push('\n');
                    return;
                }
            };
            let y: u16 = match parts[2].parse() {
                Ok(v) if v < 256 => v,
                _ => {
                    let msg = "y must be 0-255".to_string();
                    println!("{}", msg);
                    output.push_str(&msg);
                    output.push('\n');
                    return;
                }
            };
            let total_writes = vm.pixel_write_log.count_at(x, y);
            let current_color = vm.screen[y as usize * 256 + x as usize];
            let buffer_full = vm.pixel_write_log.is_full();
            if total_writes == 0 {
                if current_color != 0 {
                    let msg = format!(
                        "Pixel ({},{}) has 0 recorded writes BUT is colored 0x{:08X}!",
                        x, y, current_color
                    );
                    println!("{}", msg);
                    output.push_str(&msg);
                    output.push('\n');
                    let msg2 = "WARNING: pixel is colored but provenance data was evicted (buffer wrapped before query).";
                    println!("{}", msg2);
                    output.push_str(msg2);
                    output.push('\n');
                } else {
                    let msg = format!("Pixel ({},{}) was never written (black/initial)", x, y);
                    println!("{}", msg);
                    output.push_str(&msg);
                    output.push('\n');
                }
                let msg3 = format!("  Current color: 0x{:08X}", current_color);
                println!("{}", msg3);
                output.push_str(&msg3);
                output.push('\n');
            } else {
                let caveat = if buffer_full {
                    " (buffer full -- earlier writes may exist)"
                } else {
                    ""
                };
                let msg = format!(
                    "Pixel ({},{}) was written {} time(s){}:",
                    x, y, total_writes, caveat
                );
                println!("{}", msg);
                output.push_str(&msg);
                output.push('\n');

                // Show last 10 writes (newest first)
                let entries = vm.pixel_write_log.recent_at(x, y, 10);
                for (i, entry) in entries.iter().enumerate() {
                    let op_name = opcode_name(entry.opcode);
                    let label = if i == 0 { "last" } else { "prev" };
                    let msg = format!(
                        "  [{}] {} at step {} wrote color 0x{:08X}",
                        label,
                        op_name,
                        entry.step(),
                        entry.color
                    );
                    println!("{}", msg);
                    output.push_str(&msg);
                    output.push('\n');
                }

                // Current value
                let color = vm.screen[y as usize * 256 + x as usize];
                let msg2 = format!("  Current color: 0x{:08X}", color);
                println!("{}", msg2);
                output.push_str(&msg2);
                output.push('\n');
            }
        }
        "trace" => {
            // trace [x] [y] -- timeline provenance
            // Without args: summary of trace data available
            // With x,y: same as who_wrote (alias)
            if parts.len() >= 3 {
                // Alias for who_wrote
                let x: u16 = match parts[1].parse() {
                    Ok(v) if v < 256 => v,
                    _ => {
                        let msg = "x must be 0-255".to_string();
                        println!("{}", msg);
                        output.push_str(&msg);
                        output.push('\n');
                        return;
                    }
                };
                let y: u16 = match parts[2].parse() {
                    Ok(v) if v < 256 => v,
                    _ => {
                        let msg = "y must be 0-255".to_string();
                        println!("{}", msg);
                        output.push_str(&msg);
                        output.push('\n');
                        return;
                    }
                };
                let total_writes = vm.pixel_write_log.count_at(x, y);
                let current_color = vm.screen[y as usize * 256 + x as usize];
                let buffer_full = vm.pixel_write_log.is_full();
                if total_writes == 0 {
                    if current_color != 0 {
                        let msg = format!(
                            "Pixel ({},{}) has 0 recorded writes BUT is colored 0x{:08X}!",
                            x, y, current_color
                        );
                        println!("{}", msg);
                        output.push_str(&msg);
                        output.push('\n');
                        let msg2 = "WARNING: pixel is colored but provenance data was evicted (buffer wrapped before query).";
                        println!("{}", msg2);
                        output.push_str(msg2);
                        output.push('\n');
                    } else {
                        let msg = format!("Pixel ({},{}) was never written (black/initial)", x, y);
                        println!("{}", msg);
                        output.push_str(&msg);
                        output.push('\n');
                    }
                } else {
                    let caveat = if buffer_full {
                        " (buffer full -- earlier writes may exist)"
                    } else {
                        ""
                    };
                    let entries = vm.pixel_write_log.recent_at(x, y, 10);
                    let msg = format!(
                        "Pixel ({},{}) -- {} writes{}, showing last {}:",
                        x,
                        y,
                        total_writes,
                        caveat,
                        entries.len()
                    );
                    println!("{}", msg);
                    output.push_str(&msg);
                    output.push('\n');
                    for (i, entry) in entries.iter().enumerate() {
                        let op_name = opcode_name(entry.opcode);
                        let label = if i == 0 { "last" } else { "prev" };
                        let msg = format!(
                            "  [{}] {} at step {} -> 0x{:08X}",
                            label,
                            op_name,
                            entry.step(),
                            entry.color
                        );
                        println!("{}", msg);
                        output.push_str(&msg);
                        output.push('\n');
                    }
                }
            } else {
                // Summary of trace data
                let msg = format!(
                    "Trace buffer: {} instruction entries",
                    vm.trace_buffer.len()
                );
                println!("{}", msg);
                output.push_str(&msg);
                output.push('\n');
                let msg = format!(
                    "Frame checkpoints: {} frames saved",
                    vm.frame_checkpoints.len()
                );
                println!("{}", msg);
                output.push_str(&msg);
                output.push('\n');
                let msg = format!(
                    "Pixel write log: {} entries (use 'who_wrote <x> <y>' to query)",
                    vm.pixel_write_log.len()
                );
                println!("{}", msg);
                output.push_str(&msg);
                output.push('\n');
                let msg = format!(
                    "Timeline forks: {} snapshots (use FORK opcode to save/restore)",
                    vm.snapshots.len()
                );
                println!("{}", msg);
                output.push_str(&msg);
                output.push('\n');
                if vm.pixel_write_log.is_empty() {
                    let msg = "Note: No pixel writes recorded. Run a program first (trace is auto-enabled during 'run').";
                    println!("{}", msg);
                    output.push_str(msg);
                    output.push('\n');
                }
            }
        }
        "rollback" => {
            let out = git_rollback();
            println!("{}", out);
            output.push_str(&out);
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

/// CLI build loop -- uses HERMES_BUILD_SYSTEM_PROMPT for self-modifying the OS.
pub fn run_build_loop(
    initial_prompt: &str,
    vm: &mut vm::Vm,
    source_text: &mut String,
    loaded_file: &mut Option<PathBuf>,
    canvas_assembled: &mut bool,
) {
    println!("[build] Starting build agent loop (qwen3.5-tools via Ollama)");
    println!("[build] Type 'stop' to end the loop, or just press Enter to continue.");

    let mut conversation_history = initial_prompt.to_string();

    for iteration in 0..5 {
        let ctx = build_build_context(vm);
        let full_system = format!("{}\n\n{}", HERMES_BUILD_SYSTEM_PROMPT, ctx);

        println!("[build] --- iteration {} ---", iteration + 1);

        let response = match call_ollama(&full_system, &conversation_history) {
            Some(r) => r,
            None => {
                println!("[build] LLM call failed. Stopping.");
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
            println!("[build] LLM returned no commands. Stopping.");
            break;
        }

        println!("[build] LLM commands:\n{}", commands);

        // Execute commands (reuse write buffer + command whitelist logic)
        let mut write_buffer: Option<(String, String)> = None;
        let mut output_capture = String::new();

        for cmd_line in commands.lines() {
            let cmd_line = cmd_line.trim();
            if cmd_line.is_empty() {
                continue;
            }

            // Handle write buffer
            if let Some(ref mut wb) = write_buffer {
                if cmd_line == "ENDWRITE" {
                    match safe_write_file(&wb.0, &wb.1) {
                        Ok(bytes) => {
                            let msg = format!("Wrote {} ({} bytes)", wb.0, bytes);
                            println!("{}", msg);
                            output_capture.push_str(&msg);
                            output_capture.push('\n');
                        }
                        Err(e) => {
                            let msg = format!("Write blocked: {}", e);
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
                    if let Err(e) = validate_write_path(filename) {
                        println!("[build] Write blocked: {}", e);
                        output_capture.push_str(&format!("[build] Write blocked: {}\n", e));
                    } else {
                        write_buffer = Some((filename.to_string(), String::new()));
                    }
                }
                continue;
            }

            // Execute command
            let cmd_parts: Vec<&str> = cmd_line.split_whitespace().collect();
            if cmd_parts.is_empty() {
                continue;
            }
            let cmd_word = cmd_parts[0].to_lowercase();

            match cmd_word.as_str() {
                "shell" | "readfile" | "files" | "load" | "run" | "regs" | "peek" | "poke"
                | "screen" | "save" | "reset" | "list" | "ls" | "png" | "diff" | "commit"
                | "rollback" => {
                    println!("> {}", cmd_line);
                    let out = execute_build_command(
                        cmd_line,
                        vm,
                        source_text,
                        loaded_file,
                        canvas_assembled,
                    );
                    output_capture.push_str(&out);
                }
                _ => {
                    // Skip unknown commands
                }
            }
        }

        // Handle unclosed write buffer
        if let Some(wb) = write_buffer {
            match safe_write_file(&wb.0, &wb.1) {
                Ok(bytes) => println!("Wrote {} ({} bytes)", wb.0, bytes),
                Err(e) => println!("Write blocked: {}", e),
            }
        }

        // Ask if user wants to continue
        print!("[build] Continue? (Enter=continue, stop=quit): ");
        let _ = io::stdout().flush();
        let mut input = String::new();
        if io::stdin().read_line(&mut input).unwrap_or(0) == 0 {
            break;
        }
        let answer = input.trim().to_lowercase();
        if answer == "stop" || answer == "quit" || answer == "exit" || answer == "q" {
            println!("[build] Stopped.");
            break;
        }

        // Feed output back as context for next iteration
        let user_msg = if answer.is_empty() {
            "continue"
        } else {
            &answer
        };
        // Truncate output capture to avoid context explosion
        if output_capture.len() > 4000 {
            output_capture.truncate(4000);
            output_capture.push_str("\n... (output truncated)");
        }
        conversation_history = format!(
            "Previous commands output:\n{}\n\nUser instruction: {}",
            output_capture, user_msg,
        );
    }

    println!("[build] Agent loop ended.");
}

/// Canvas build loop -- visual version of run_build_loop for the GUI terminal.
#[allow(clippy::too_many_arguments)]
pub fn run_build_canvas(
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
        "[build] Starting self-build agent loop...",
    );
    *output_row = write_line_to_canvas(canvas_buffer, *output_row, "[build] Press Escape to stop.");
    ensure_scroll(*output_row, scroll_offset);

    let mut conversation_history = initial_prompt.to_string();

    for iteration in 0..3 {
        let ctx = build_build_context(vm);
        let full_system = format!("{}\n\n{}", HERMES_BUILD_SYSTEM_PROMPT, ctx);

        *output_row = write_line_to_canvas(
            canvas_buffer,
            *output_row,
            &format!("[build] --- iteration {} ---", iteration + 1),
        );
        ensure_scroll(*output_row, scroll_offset);

        let response = match call_ollama(&full_system, &conversation_history) {
            Some(r) => r,
            None => {
                *output_row = write_line_to_canvas(
                    canvas_buffer,
                    *output_row,
                    "[build] LLM call failed. Stopping.",
                );
                ensure_scroll(*output_row, scroll_offset);
                break;
            }
        };

        // Strip think blocks
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
                "[build] LLM returned no commands. Stopping.",
            );
            ensure_scroll(*output_row, scroll_offset);
            break;
        }

        // Show commands
        for cmd_line in commands.lines() {
            let trimmed = cmd_line.trim();
            if !trimmed.is_empty() {
                *output_row =
                    write_line_to_canvas(canvas_buffer, *output_row, &format!("  > {}", trimmed));
            }
        }
        ensure_scroll(*output_row, scroll_offset);

        // Execute commands
        let mut write_buffer: Option<(String, String)> = None;
        let mut output_capture = String::new();

        for cmd_line in commands.lines() {
            let cmd_line = cmd_line.trim();
            if cmd_line.is_empty() {
                continue;
            }

            // Handle write buffer
            if let Some(ref mut wb) = write_buffer {
                if cmd_line == "ENDWRITE" {
                    match safe_write_file(&wb.0, &wb.1) {
                        Ok(bytes) => {
                            let msg = format!("Wrote {} ({} bytes)", wb.0, bytes);
                            *output_row = write_line_to_canvas(canvas_buffer, *output_row, &msg);
                            output_capture.push_str(&msg);
                            output_capture.push('\n');
                        }
                        Err(e) => {
                            let msg = format!("Write blocked: {}", e);
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
                    if let Err(e) = validate_write_path(filename) {
                        *output_row = write_line_to_canvas(
                            canvas_buffer,
                            *output_row,
                            &format!("[build] Write blocked: {}", e),
                        );
                    } else {
                        write_buffer = Some((filename.to_string(), String::new()));
                    }
                }
                continue;
            }

            // For shell/readfile/files: execute and capture output
            let cmd_parts: Vec<&str> = cmd_line.split_whitespace().collect();
            if cmd_parts.is_empty() {
                continue;
            }
            let cmd_word = cmd_parts[0].to_lowercase();

            match cmd_word.as_str() {
                "shell" | "readfile" | "files" | "diff" | "commit" | "rollback" => {
                    let out = execute_build_command(
                        cmd_line,
                        vm,
                        &mut String::new(), // build mode doesn't use source_text
                        &mut None,          // build mode doesn't use loaded_file
                        canvas_assembled,
                    );
                    // Show output on canvas (truncated)
                    for line in out.lines().take(5) {
                        *output_row = write_line_to_canvas(
                            canvas_buffer,
                            *output_row,
                            &format!("    {}", line),
                        );
                    }
                    if out.lines().count() > 5 {
                        *output_row = write_line_to_canvas(
                            canvas_buffer,
                            *output_row,
                            &format!("    ... ({} more lines)", out.lines().count() - 5),
                        );
                    }
                    output_capture.push_str(&out);
                }
                // Standard VM commands go through the GUI handler
                "load" | "run" | "regs" | "peek" | "poke" | "screen" | "save" | "reset"
                | "list" | "ls" | "png" | "disasm" | "step" | "bp" | "bpc" | "trace" => {
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
                    for row in row_before..(*output_row) {
                        let line_text = read_canvas_line(canvas_buffer, row);
                        if !line_text.is_empty() && !line_text.starts_with("geo> ") {
                            output_capture.push_str(&line_text);
                            output_capture.push('\n');
                        }
                    }
                    ensure_scroll(*output_row, scroll_offset);
                }
                _ => {}
            }
        }

        // Handle unclosed write buffer
        if let Some(wb) = write_buffer {
            match safe_write_file(&wb.0, &wb.1) {
                Ok(bytes) => {
                    *output_row = write_line_to_canvas(
                        canvas_buffer,
                        *output_row,
                        &format!("Wrote {} ({} bytes)", wb.0, bytes),
                    );
                }
                Err(e) => {
                    *output_row = write_line_to_canvas(
                        canvas_buffer,
                        *output_row,
                        &format!("Write blocked: {}", e),
                    );
                }
            }
        }

        *output_row =
            write_line_to_canvas(canvas_buffer, *output_row, "[build] Iteration complete.");
        ensure_scroll(*output_row, scroll_offset);

        // Feed output back
        if output_capture.len() > 4000 {
            output_capture.truncate(4000);
            output_capture.push_str("\n... (output truncated)");
        }
        conversation_history = format!(
            "Previous commands output:\n{}\n\nUser instruction: continue",
            output_capture,
        );
    }

    *output_row = write_line_to_canvas(canvas_buffer, *output_row, "[build] Agent loop ended.");
    ensure_scroll(*output_row, scroll_offset);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_screen_to_ascii_blank() {
        let screen = vec![0u32; 256 * 256];
        let ascii = screen_to_ascii(&screen);
        // All spaces
        assert!(ascii.trim().is_empty());
        // Should be 24 rows
        assert_eq!(ascii.lines().count(), 24);
    }

    #[test]
    fn test_screen_to_ascii_full_white() {
        let screen = vec![0xFFFFFFu32; 256 * 256];
        let ascii = screen_to_ascii(&screen);
        // All @ (max brightness)
        for line in ascii.lines() {
            assert!(line.chars().all(|c| c == '@'), "line not all @: {:?}", line);
        }
    }

    #[test]
    fn test_screen_to_ascii_single_pixel() {
        let mut screen = vec![0u32; 256 * 256];
        screen[125 * 256 + 122] = 0xFFFFFF; // center of cell (24, 12)
        let ascii = screen_to_ascii(&screen);
        // Should have exactly one non-space character
        let non_space: Vec<_> = ascii.chars().filter(|&c| c != ' ' && c != '\n').collect();
        assert_eq!(
            non_space.len(),
            1,
            "expected 1 non-space char, got {}: {:?}",
            non_space.len(),
            non_space
        );
    }

    #[test]
    fn test_screen_to_ascii_dimensions() {
        let screen = vec![0u32; 256 * 256];
        let ascii = screen_to_ascii(&screen);
        let lines: Vec<&str> = ascii.lines().collect();
        assert_eq!(lines.len(), 24, "expected 24 rows");
        for line in &lines {
            assert_eq!(line.len(), 48, "expected 48 cols");
        }
    }

    /// Canary test: opcode_name must agree with the disassembler.
    /// If opcodes are added to disasm.rs but not opcode_name (or vice versa),
    /// this test catches the drift. Samples representative opcodes across the
    /// full ISA range rather than testing all 103 (disasm requires a Vm + RAM).
    #[test]
    fn test_opcode_name_matches_disasm() {
        use crate::vm::Vm;
        // Canaries: one from each ISA range, plus some that were wrong before
        let canaries: &[(u8, &str)] = &[
            (0x00, "HALT"),
            (0x02, "FRAME"),
            (0x14, "STRO"), // was wrong: was TEXT
            (0x1B, "ADDI"),
            (0x20, "ADD"),
            (0x29, "MOD"),
            (0x30, "JMP"),      // was wrong: was ADDI
            (0x33, "CALL"),     // was wrong: was DIVI
            (0x40, "PSET"),     // was wrong: was PLOT
            (0x42, "FILL"),     // was wrong: was LINE
            (0x45, "LINE"),     // was wrong: was GETPIX
            (0x52, "SYSCALL"),  // was wrong: was IKEY
            (0x60, "PUSH"),     // was wrong: was FOPEN
            (0x6E, "SHUTDOWN"), // was wrong: was SETENV
            (0x75, "FORMULA"),
            (0x7E, "NOTE"),
        ];
        for &(op, _expected) in canaries {
            let name = opcode_name(op);
            // Disassemble by putting the opcode in RAM and calling disassemble_at
            let mut vm2 = Vm::new();
            let addr = 100u32;
            vm2.ram[addr as usize] = op as u32;
            let (disasm_str, _) = vm2.disassemble_at(addr);
            // disasm_str starts with the mnemonic (before any spaces/args)
            let disasm_mnemonic = disasm_str.split_whitespace().next().unwrap_or("???");
            assert_eq!(
                name, disasm_mnemonic,
                "opcode_name(0x{:02X}) = {:?} but disasm says {:?}",
                op, name, disasm_mnemonic
            );
        }
    }
}
