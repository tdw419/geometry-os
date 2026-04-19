// cli.rs -- Headless CLI mode for Geometry OS

use crate::assembler;
use crate::canvas::list_asm_files;
use crate::hermes::run_hermes_loop;
use crate::preprocessor;
use crate::save::{load_state, save_state};
use crate::vm;
use geometry_os::qemu::QemuBridge;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

const SAVE_FILE: &str = "geometry_os.sav";

/// Scan config string for kernel=/path.rts.png or initrd=/path.rts.png
/// and auto-decode pixel images to temp files.
fn resolve_pixel_paths(config: &str) -> String {
    let mut result = config.to_string();
    for key in &["kernel", "initrd", "dtb", "drive"] {
        // Find key=value in the config string
        if let Some(start) = result.find(&format!("{}=", key)) {
            let val_start = start + key.len() + 1;
            // Value runs until next space or end of string
            let val_end = result[val_start..]
                .find(' ')
                .map(|i| val_start + i)
                .unwrap_or(result.len());
            let value = &result[val_start..val_end];

            if value.to_lowercase().ends_with(".rts.png") {
                match geometry_os::pixel::decode_rts_to_temp(value) {
                    Ok(temp_path) => {
                        println!(
                            "[pixel] Decoded {} -> {} ({} bytes)",
                            value,
                            temp_path,
                            std::fs::metadata(&temp_path).map(|m| m.len()).unwrap_or(0)
                        );
                        result.replace_range(val_start..val_end, &temp_path);
                    }
                    Err(e) => {
                        eprintln!("[pixel] Failed to decode {}: {}", value, e);
                    }
                }
            }
        }
    }
    result
}

pub fn cli_main(extra_args: &[String]) {
    let mut vm = vm::Vm::new();
    let mut canvas_assembled = false;
    let mut loaded_file: Option<PathBuf> = None;
    let mut source_text = String::new(); // holds the currently loaded source
    let mut cli_breakpoints: Vec<u32> = Vec::new();
    let mut canvas_buffer: Vec<u32> = vec![0; 4096];
    let mut qemu_bridge: Option<QemuBridge> = None;

    // If extra args given, treat first as a file to load
    if !extra_args.is_empty() {
        let path = PathBuf::from(&extra_args[0]);
        match std::fs::read_to_string(&path) {
            Ok(src) => {
                source_text = src;
                loaded_file = Some(path);
            }
            Err(e) => {
                eprintln!("Error reading {}: {}", extra_args[0], e);
            }
        }
    }

    println!("Geometry OS v1.0.0 CLI");
    println!("40 opcodes | 32 regs | 256x256");
    println!("Type 'help' for commands.");
    println!();

    let stdin = io::stdin();
    loop {
        // Poll QEMU output before showing prompt
        if let Some(ref mut bridge) = qemu_bridge {
            if bridge.is_alive() {
                let output = bridge.read_output_text();
                if !output.is_empty() {
                    print!("{}", output);
                    let _ = io::stdout().flush();
                }
            }
        }

        print!("geo> ");
        let _ = io::stdout().flush();

        let mut line = String::new();
        if stdin.read_line(&mut line).unwrap_or(0) == 0 {
            break; // EOF
        }
        let cmd = line.trim();
        if cmd.is_empty() {
            continue;
        }

        // If QEMU is running and user types a non-qemu command, forward to QEMU stdin
        if let Some(ref mut bridge) = qemu_bridge {
            if bridge.is_alive()
                && !cmd.starts_with("qemu")
                && !cmd.starts_with("quit")
                && !cmd.starts_with("exit")
            {
                // Forward the line to QEMU as stdin + newline
                let _ = bridge.write_bytes(format!("{}\n", cmd).as_bytes());
                // Give QEMU a moment to process
                std::thread::sleep(std::time::Duration::from_millis(10));
                // Read any output
                let output = bridge.read_output_text();
                if !output.is_empty() {
                    print!("{}", output);
                    let _ = io::stdout().flush();
                }
                continue;
            }
        }

        let parts: Vec<&str> = cmd.split_whitespace().collect();
        if parts.is_empty() {
            continue;
        }

        let command = parts[0].to_lowercase();
        match command.as_str() {
            "help" | "?" => {
                println!("Commands:");
                println!("  list              List .asm programs");
                println!("  load <file>       Load .asm source");
                println!("  run               Assemble source & run VM");
                println!("  regs              Show register dump");
                println!("  peek <addr>       Read RAM[addr]");
                println!("  poke <addr> <val> Write RAM[addr]");
                println!("  screen <addr>     Dump 16 pixels from screen buffer");
                println!("  reset             Reset VM state");
                println!("  step              Step one instruction");
                println!("  trace [n]         Execute n instructions with log");
                println!("  bp [addr]         Toggle/list breakpoints");
                println!("  bpc               Clear all breakpoints");
                println!("  disasm [addr] [n] Disassemble n instrs");
                println!("  qemu boot <cfg>   Boot QEMU VM (e.g. qemu boot arch=riscv64 kernel=Image ram=256M)");
                println!("  qemu kill         Kill running QEMU");
                println!("  qemu status       Show QEMU status");
                println!("  quit              Exit");
            }
            "list" | "ls" => {
                let files = list_asm_files("programs");
                if files.is_empty() {
                    println!("  (no .asm files in programs/)");
                } else {
                    for f in &files {
                        let name = Path::new(f)
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_else(|| f.clone());
                        println!("  {}", name);
                    }
                    println!("  {} programs", files.len());
                }
            }
            "load" => {
                if parts.len() < 2 {
                    println!("Usage: load <file.asm> or load <slot>");
                    continue;
                }
                let filename_arg = parts[1..].join(" ");
                if filename_arg.ends_with(".asm")
                    || filename_arg.contains('/')
                    || filename_arg.contains('\\')
                {
                    let filename = filename_arg.clone();
                    let path = Path::new(&filename);
                    let path = if path.exists() {
                        path.to_path_buf()
                    } else {
                        let prefixed = Path::new("programs").join(&filename);
                        if prefixed.exists() {
                            prefixed
                        } else {
                            println!("File not found: {}", filename);
                            continue;
                        }
                    };
                    match std::fs::read_to_string(&path) {
                        Ok(src) => {
                            let lines = src.lines().count();
                            source_text = src;
                            loaded_file = Some(path.clone());
                            println!(
                                "Loaded {} ({} lines)",
                                path.file_name()
                                    .map(|n| n.to_string_lossy().into_owned())
                                    .unwrap_or_default(),
                                lines
                            );
                        }
                        Err(e) => {
                            println!("Error: {}", e);
                        }
                    }
                } else {
                    // Assume it's a state slot
                    let filename = format!("geometry_os_{}.sav", filename_arg);
                    match load_state(&filename) {
                        Ok((saved_vm, saved_canvas, saved_assembled)) => {
                            vm = saved_vm;
                            canvas_buffer = saved_canvas;
                            canvas_assembled = saved_assembled;
                            println!("Loaded state from {}", filename);
                        }
                        Err(_) => {
                            // Fallback: try loading as .asm if slot not found
                            let mut filename = filename_arg.clone();
                            filename.push_str(".asm");
                            let path = Path::new("programs").join(&filename);
                            if path.exists() {
                                if let Ok(src) = std::fs::read_to_string(&path) {
                                    source_text = src;
                                    loaded_file = Some(path.clone());
                                    println!("Loaded programs/{}", filename);
                                } else {
                                    println!(
                                        "Slot {} not found and could not read .asm",
                                        filename_arg
                                    );
                                }
                            } else {
                                println!("Slot or file {} not found", filename_arg);
                            }
                        }
                    }
                }
            }
            "run" => {
                if source_text.is_empty() {
                    println!("No source loaded. Use 'load <file>' first.");
                    continue;
                }
                // Abstraction Layer: Preprocess macros and variables
                let mut pp = preprocessor::Preprocessor::new();
                let preprocessed_source = pp.preprocess(&source_text);

                match assembler::assemble(&preprocessed_source, 0) {
                    Ok(asm_result) => {
                        // Clear bytecode region (load at 0 so labels resolve correctly)
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

                        println!(
                            "Assembled {} bytes at 0x{:04X}",
                            asm_result.pixels.len(),
                            load_addr
                        );

                        // Phase 45: Sync canvas buffer TO VM before execution
                        vm.canvas_buffer.copy_from_slice(&canvas_buffer);

                        // Run the VM
                        let mut hit_bp = false;
                        for _ in 0..10_000_000 {
                            if !vm.step() {
                                break;
                            }
                            if cli_breakpoints.contains(&vm.pc) {
                                hit_bp = true;
                                break;
                            }
                        }

                        // Phase 45: Sync canvas buffer FROM VM after execution
                        canvas_buffer.copy_from_slice(&vm.canvas_buffer);

                        if hit_bp {
                            println!("BREAK @ PC=0x{:04X}", vm.pc);
                        } else if vm.halted {
                            println!("Halted at PC=0x{:04X}", vm.pc);
                        } else {
                            println!("Running... PC=0x{:04X}", vm.pc);
                        }
                        canvas_assembled = true;
                    }
                    Err(e) => {
                        println!("{}", e);
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
                }
                println!(
                    "PC={:04X} SP={:04X} LR={:04X}",
                    vm.pc, vm.regs[30], vm.regs[31]
                );
            }
            "peek" => {
                if parts.len() < 2 {
                    println!("Usage: peek <addr>");
                    continue;
                }
                match u32::from_str_radix(
                    parts[1].trim_start_matches("0x").trim_start_matches("0X"),
                    16,
                ) {
                    Ok(addr) if (addr as usize) < vm.ram.len() => {
                        let val = vm.ram[addr as usize];
                        println!("RAM[0x{:04X}] = 0x{:08X}", addr, val);
                    }
                    Ok(addr) => {
                        println!("Address 0x{:04X} out of range", addr);
                    }
                    Err(_) => {
                        println!("Invalid address");
                    }
                }
            }
            "poke" => {
                if parts.len() < 3 {
                    println!("Usage: poke <addr> <val>");
                    continue;
                }
                let addr_str = parts[1].trim_start_matches("0x").trim_start_matches("0X");
                let val_str = parts[2].trim_start_matches("0x").trim_start_matches("0X");
                match (
                    u32::from_str_radix(addr_str, 16),
                    u32::from_str_radix(val_str, 16),
                ) {
                    (Ok(addr), Ok(val)) if (addr as usize) < vm.ram.len() => {
                        vm.ram[addr as usize] = val;
                        println!("RAM[0x{:04X}] <- 0x{:08X}", addr, val);
                    }
                    _ => {
                        println!("Usage: poke <hex_addr> <hex_val>");
                    }
                }
            }
            "screen" => {
                // Dump 16 pixels from the screen buffer starting at addr
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
                }
            }
            "save" => {
                let slot = parts.get(1).copied();
                let filename = match slot {
                    Some(s) => format!("geometry_os_{}.sav", s),
                    None => SAVE_FILE.to_string(),
                };
                match save_state(&filename, &vm, &canvas_buffer, canvas_assembled) {
                    Ok(()) => println!("Saved state to {}", filename),
                    Err(e) => println!("Error saving state: {}", e),
                }
            }
            "ppm" => {
                let filename = if parts.len() >= 2 {
                    parts[1].to_string()
                } else {
                    "output.ppm".to_string()
                };
                match std::fs::File::create(&filename) {
                    Ok(mut f) => {
                        // PPM P6 format
                        let header = "P6\n256 256\n255\n".to_string();
                        use std::io::Write;
                        if f.write_all(header.as_bytes()).is_err() {
                            println!("Error writing PPM header");
                            continue;
                        }
                        for pixel in &vm.screen {
                            let r = (pixel >> 16) & 0xFF;
                            let g = (pixel >> 8) & 0xFF;
                            let b = pixel & 0xFF;
                            if f.write_all(&[r as u8, g as u8, b as u8]).is_err() {
                                println!("Error writing PPM data");
                                break;
                            }
                        }
                        println!("Saved screen to {}", filename);
                    }
                    Err(e) => println!("Error saving: {}", e),
                }
            }
            "step" => {
                if vm.halted {
                    println!("VM halted. Use reset to restart.");
                } else {
                    // Phase 45: Sync canvas buffer TO VM before execution
                    vm.canvas_buffer.copy_from_slice(&canvas_buffer);

                    vm.step();

                    // Phase 45: Sync canvas buffer FROM VM after execution
                    canvas_buffer.copy_from_slice(&vm.canvas_buffer);

                    println!("step -> PC=0x{:04X}", vm.pc);
                }
            }
            "bp" => {
                if parts.len() < 2 {
                    if cli_breakpoints.is_empty() {
                        println!("  No breakpoints set");
                    } else {
                        for &addr in &cli_breakpoints {
                            println!("  BP @ 0x{:04X}", addr);
                        }
                    }
                } else {
                    match u32::from_str_radix(
                        parts[1].trim_start_matches("0x").trim_start_matches("0X"),
                        16,
                    ) {
                        Ok(addr) => {
                            if let Some(pos) = cli_breakpoints.iter().position(|&a| a == addr) {
                                cli_breakpoints.remove(pos);
                                println!("Cleared BP @ 0x{:04X}", addr);
                            } else {
                                cli_breakpoints.push(addr);
                                println!("Set BP @ 0x{:04X}", addr);
                            }
                        }
                        Err(_) => println!("Invalid address"),
                    }
                }
            }
            "bpc" => {
                cli_breakpoints.clear();
                println!("Breakpoints cleared");
            }
            "trace" => {
                // trace [count] — execute N instructions, logging each one
                let count = if parts.len() >= 2 {
                    parts[1].parse::<usize>().unwrap_or(20)
                } else {
                    20
                };
                if vm.halted {
                    println!("VM halted. Use reset to restart.");
                } else {
                    for i in 0..count {
                        let addr_before = vm.pc;
                        let (mnemonic, _len) = vm.disassemble_at(vm.pc);
                        if !vm.step() {
                            println!("{:04} {:04X} {:30} -> HALTED", i, addr_before, mnemonic);
                            break;
                        }
                        // Show non-zero registers (up to 4 most interesting)
                        let mut reg_info = String::new();
                        let mut shown = 0;
                        // Always show PC and any regs that were likely modified
                        for r in 0..8 {
                            if vm.regs[r] != 0 && shown < 4 {
                                reg_info.push_str(&format!(" r{}={:X}", r, vm.regs[r]));
                                shown += 1;
                            }
                        }
                        if reg_info.is_empty() {
                            reg_info = " (no regs changed)".to_string();
                        }
                        println!(
                            "{:04} {:04X} {:30} -> {:04X}{}",
                            i, addr_before, mnemonic, vm.pc, reg_info
                        );
                        if cli_breakpoints.contains(&vm.pc) {
                            println!("BREAK @ PC=0x{:04X}", vm.pc);
                            break;
                        }
                    }
                }
            }
            "disasm" => {
                // disasm [addr] [count] — defaults to PC, 10 lines
                let start_addr = if parts.len() >= 2 {
                    u32::from_str_radix(parts[1].trim_start_matches("0x"), 16).unwrap_or(vm.pc)
                } else {
                    vm.pc
                };
                let count = if parts.len() >= 3 {
                    parts[2].parse::<usize>().unwrap_or(10)
                } else {
                    10
                };
                let mut addr = start_addr;
                for _ in 0..count {
                    if addr as usize >= vm.ram.len() {
                        break;
                    }
                    let (mnemonic, len) = vm.disassemble_at(addr);
                    let marker = if addr == vm.pc { ">" } else { " " };
                    println!(" {}{:04X} {}", marker, addr, mnemonic);
                    addr += len as u32;
                }
            }
            "reset" => {
                vm.reset();
                canvas_assembled = false;
                println!("VM reset");
            }
            "hermes" => {
                if parts.len() < 2 {
                    println!("Usage: hermes <prompt>");
                    println!("  Starts an agent loop driven by a local LLM.");
                    println!("  The LLM can run geo> commands to accomplish tasks.");
                    println!("  Requires Ollama running locally (qwen3.5-tools).");
                    continue;
                }
                let user_prompt = parts[1..].join(" ");
                run_hermes_loop(
                    &user_prompt,
                    &mut vm,
                    &mut source_text,
                    &mut loaded_file,
                    &mut canvas_assembled,
                );
            }
            "qemu" => {
                let subcmd = parts.get(1).copied().unwrap_or("");
                match subcmd {
                    "boot" => {
                        if parts.len() < 3 {
                            println!("Usage: qemu boot <config>");
                            println!(
                                "  e.g. qemu boot arch=riscv64 kernel=/path/to/Image ram=256M"
                            );
                            println!("  e.g. qemu boot arch=riscv64 kernel=Image initrd=initrd.gz append='console=ttyS0'");
                            continue;
                        }
                        // Kill any existing QEMU first
                        if let Some(ref mut bridge) = qemu_bridge {
                            let _ = bridge.kill();
                        }
                        qemu_bridge = None;

                        let mut config_str = parts[2..].join(" ");
                        // Auto-decode .rts.png files to temp files
                        config_str = resolve_pixel_paths(&config_str);
                        match QemuBridge::spawn(&config_str) {
                            Ok(mut bridge) => {
                                // Drain QEMU boot output. OpenSBI starts ~1s,
                                // kernel+rootfs can take 5-10s for first output.
                                let boot_start = std::time::Instant::now();
                                let drain_timeout = std::time::Duration::from_secs(10);
                                let mut got_output = false;
                                while boot_start.elapsed() < drain_timeout {
                                    std::thread::sleep(std::time::Duration::from_millis(200));
                                    let output = bridge.read_output_text();
                                    if !output.is_empty() {
                                        print!("{}", output);
                                        let _ = io::stdout().flush();
                                        got_output = true;
                                        // Once we get output, keep draining for 2 more seconds
                                        let drain_more = std::time::Duration::from_secs(2);
                                        let drain_start = std::time::Instant::now();
                                        while drain_start.elapsed() < drain_more {
                                            std::thread::sleep(std::time::Duration::from_millis(
                                                100,
                                            ));
                                            let more = bridge.read_output_text();
                                            if !more.is_empty() {
                                                print!("{}", more);
                                                let _ = io::stdout().flush();
                                            }
                                        }
                                        break;
                                    }
                                }
                                if !got_output {
                                    println!(
                                        "[qemu] No output after 10s -- QEMU may still be booting"
                                    );
                                }
                                qemu_bridge = Some(bridge);
                                println!("[qemu] Booted: {}", config_str);
                                let _ = io::stdout().flush();
                            }
                            Err(e) => {
                                println!("[qemu] Error: {}", e);
                            }
                        }
                    }
                    "kill" => {
                        if let Some(ref mut bridge) = qemu_bridge {
                            match bridge.kill() {
                                Ok(()) => println!("[qemu] Killed"),
                                Err(e) => println!("[qemu] Kill error: {}", e),
                            }
                            qemu_bridge = None;
                        } else {
                            println!("[qemu] No QEMU running");
                        }
                    }
                    "status" => match qemu_bridge {
                        Some(ref mut bridge) => {
                            if bridge.is_alive() {
                                let cursor = bridge.cursor();
                                println!(
                                    "[qemu] Running (cursor: row={}, col={})",
                                    cursor.row, cursor.col
                                );
                            } else {
                                println!("[qemu] Process exited");
                                qemu_bridge = None;
                            }
                        }
                        None => println!("[qemu] Not running"),
                    },
                    "traps" => {
                        let sub = parts.get(2).map(|s| *s).unwrap_or("");
                        if sub == "analyze" || sub == "ai" {
                            // ── AI-driven behavioral analysis ──
                            let log_path = std::env::temp_dir().join("geo_qemu_trace.log");
                            match std::fs::read_to_string(&log_path) {
                                Ok(content) => {
                                    let lines: Vec<&str> = content.lines().collect();
                                    let total = lines.len();
                                    if total == 0 {
                                        println!("[qemu] Trace log is empty (boot with trace=int to enable)");
                                    } else {
                                        // Parse all entries
                                        let mut entries: Vec<TrapEntry> = Vec::with_capacity(total);
                                        for line in &lines {
                                            if let Some(r) = parse_trap_line(line) {
                                                entries.push(r);
                                            }
                                        }
                                        let n = entries.len();
                                        if n == 0 {
                                            println!("[qemu] No parseable entries found");
                                        } else {
                                            analyze_traps(&entries);
                                        }
                                    }
                                }
                                Err(_) => {
                                    println!(
                                        "[qemu] No trace log found (boot with trace=int to enable)"
                                    );
                                }
                            }
                        } else {
                            // ── Raw trap view (original behavior) ──
                            let n: usize =
                                sub.parse().unwrap_or(if sub.is_empty() { 20 } else { 0 });
                            let log_path = std::env::temp_dir().join("geo_qemu_trace.log");
                            match std::fs::read_to_string(&log_path) {
                                Ok(content) => {
                                    let lines: Vec<&str> = content.lines().collect();
                                    let total = lines.len();
                                    if total == 0 {
                                        println!("[qemu] Trace log is empty (boot with trace=int to enable)");
                                    } else {
                                        // Count trap types
                                        let mut counts: std::collections::HashMap<&str, usize> =
                                            std::collections::HashMap::new();
                                        for line in &lines {
                                            if let Some(idx) = line.find("desc=") {
                                                let desc = &line[idx + 5..];
                                                *counts.entry(desc).or_insert(0) += 1;
                                            }
                                        }
                                        println!(
                                            "[qemu] Trace log: {} entries, {} trap types",
                                            total,
                                            counts.len()
                                        );
                                        // Show top trap types
                                        let mut sorted: Vec<_> = counts.iter().collect();
                                        sorted.sort_by(|a, b| b.1.cmp(a.1));
                                        for (desc, count) in sorted.iter().take(5) {
                                            println!("  {:>6}x {}", count, desc);
                                        }
                                        // Show last N entries
                                        println!("\nLast {} entries:", n.min(total));
                                        for line in lines.iter().rev().take(n).rev() {
                                            if let Some(idx) = line.find("cause:") {
                                                let short = &line[idx..];
                                                println!("  {}", short);
                                            } else {
                                                println!("  {}", line);
                                            }
                                        }
                                    }
                                }
                                Err(_) => {
                                    println!(
                                        "[qemu] No trace log found (boot with trace=int to enable)"
                                    );
                                }
                            }
                        }
                    }
                    _ => {
                        println!("Usage: qemu <boot|kill|status|traps>");
                        println!("  qemu boot arch=riscv64 kernel=Image ram=256M [trace=int]");
                        println!("  qemu traps [N]               -- show last N trap entries");
                        println!("  qemu traps analyze           -- AI-driven behavioral analysis");
                    }
                }
            }
            "quit" | "exit" => {
                // Clean up QEMU
                if let Some(ref mut bridge) = qemu_bridge {
                    let _ = bridge.kill();
                }
                break;
            }
            _ => {
                println!("Unknown: {} (try help)", command);
            }
        }
    }
}

// ── Trap analysis helpers ──

struct TrapEntry {
    #[allow(dead_code)]
    cause: u64,
    epc: u64,
    tval: u64,
    desc: String,
}

/// Parse a single QEMU trap log line.
/// Format: riscv_cpu_do_interrupt: hart:0, async:0, cause:XXXXXXXX, epc:0xXXXX, tval:0xXXXX, desc=XXXX
fn parse_trap_line(line: &str) -> Option<TrapEntry> {
    let cause = line.find("cause:").and_then(|i| {
        let s = &line[i + 6..];
        let end = s.find(',').unwrap_or(s.len());
        u64::from_str_radix(s[..end].trim(), 16).ok()
    })?;
    let epc = line.find("epc:0x").and_then(|i| {
        let s = &line[i + 6..];
        let end = s.find(',').unwrap_or(s.len());
        u64::from_str_radix(s[..end].trim(), 16).ok()
    })?;
    let tval = line.find("tval:0x").and_then(|i| {
        let s = &line[i + 7..];
        let end = s.find(',').unwrap_or(s.len());
        u64::from_str_radix(s[..end].trim(), 16).ok()
    })?;
    let desc = line.find("desc=").map(|i| line[i + 5..].to_string())?;
    Some(TrapEntry {
        cause,
        epc,
        tval,
        desc,
    })
}

fn analyze_traps(entries: &[TrapEntry]) {
    use std::collections::HashMap;

    let n = entries.len();
    println!("[qemu] Analyzing {} trap entries...", n);

    // ── 1. Trap type distribution ──
    let mut desc_counts: HashMap<&str, usize> = HashMap::new();
    for e in entries {
        *desc_counts.entry(&e.desc).or_insert(0) += 1;
    }
    let mut sorted_descs: Vec<_> = desc_counts.iter().collect();
    sorted_descs.sort_by(|a, b| b.1.cmp(a.1));

    println!("\n  TRAP DISTRIBUTION");
    println!("  {:>10}  {:>6}  {}", "Count", "Pct", "Type");
    println!("  {}", "-".repeat(40));
    for (desc, count) in &sorted_descs {
        let pct = **count as f64 / n as f64 * 100.0;
        println!("  {:>10}  {:>5.1}%  {}", count, pct, desc);
    }

    // ── 2. Boot phase analysis (10 temporal windows) ──
    println!("\n  BOOT PHASES (temporal windows)");
    println!(
        "  {:>6}  {:>8}  {:>8}  {:>8}  {:>8}  {:>8}  {:>6}",
        "Phase", "user", "s_ecall", "store_pf", "load_pf", "exec_pf", "timer"
    );
    println!("  {}", "-".repeat(60));
    let windows = 10;
    let window_size = n / windows;
    for w in 0..windows {
        let start = w * window_size;
        let end = if w < windows - 1 {
            start + window_size
        } else {
            n
        };
        let chunk = &entries[start..end];
        let mut wc: HashMap<&str, usize> = HashMap::new();
        for e in chunk {
            *wc.entry(&e.desc).or_insert(0) += 1;
        }
        println!(
            "  {:>3}-{:<3}%  {:>8}  {:>8}  {:>8}  {:>8}  {:>8}  {:>6}",
            w * 10,
            (w + 1) * 10,
            wc.get("user_ecall").unwrap_or(&0),
            wc.get("supervisor_ecall").unwrap_or(&0),
            wc.get("store_page_fault").unwrap_or(&0),
            wc.get("load_page_fault").unwrap_or(&0),
            wc.get("exec_page_fault").unwrap_or(&0),
            wc.get("s_timer").unwrap_or(&0)
        );
    }

    // ── 3. Boot transition markers ──
    let first_user = entries.iter().position(|e| e.desc == "user_ecall");
    let first_timer = entries.iter().position(|e| e.desc == "s_timer");

    println!("\n  BOOT TRANSITIONS");
    if let Some(idx) = first_user {
        println!(
            "  First user_ecall at #{} ({:.1}%) -- userspace starts",
            format!("{}", idx),
            idx as f64 / n as f64 * 100.0
        );
        // Pre-userspace summary
        let pre: Vec<_> = entries[..idx].iter().collect();
        let mut pre_counts: HashMap<&str, usize> = HashMap::new();
        for e in &pre {
            *pre_counts.entry(&e.desc).or_insert(0) += 1;
        }
        let mut pre_sorted: Vec<_> = pre_counts.iter().collect();
        pre_sorted.sort_by(|a, b| b.1.cmp(a.1));
        let summary: Vec<String> = pre_sorted
            .iter()
            .map(|(k, v)| format!("{}x {}", v, k))
            .collect();
        println!("  Kernel phase: {}", summary.join(", "));
    }
    if let Some(idx) = first_timer {
        println!(
            "  First s_timer at #{} ({:.1}%) -- scheduler active",
            format!("{}", idx),
            idx as f64 / n as f64 * 100.0
        );
    }

    // ── 4. Top syscall hotspots (by 4K code page) ──
    let mut page_counts: HashMap<u64, usize> = HashMap::new();
    for e in entries {
        if e.desc == "user_ecall" {
            *page_counts.entry(e.epc & !0xFFF).or_insert(0) += 1;
        }
    }
    let mut top_pages: Vec<_> = page_counts.iter().collect();
    top_pages.sort_by(|a, b| b.1.cmp(a.1));

    println!("\n  SYSCALL HOTSPOTS (top 10 code pages)");
    println!("  {:>18}  {:>8}  {}", "Page", "Calls", "Role");
    println!("  {}", "-".repeat(50));
    for (page, count) in top_pages.iter().take(10) {
        let role = classify_address(**page);
        println!("  0x{:016X}  {:>8}  {}", page, count, role);
    }
    println!("  Total unique code pages: {}", page_counts.len());

    // ── 5. Supervisor ecall sites ──
    let mut s_epc_counts: HashMap<u64, usize> = HashMap::new();
    for e in entries {
        if e.desc == "supervisor_ecall" {
            *s_epc_counts.entry(e.epc).or_insert(0) += 1;
        }
    }
    let mut s_top: Vec<_> = s_epc_counts.iter().collect();
    s_top.sort_by(|a, b| b.1.cmp(a.1));

    if !s_top.is_empty() {
        println!("\n  SBI/SYSCALL ENTRY POINTS (supervisor_ecall)");
        for (epc, count) in s_top.iter().take(5) {
            println!("  0x{:016X}  {:>6}x", epc, count);
        }
    }

    // ── 6. Memory working set growth ──
    println!("\n  MEMORY WORKING SET GROWTH");
    println!(
        "  {:>6}  {:>12}  {:>12}",
        "Phase", "Total pages", "New pages"
    );
    println!("  {}", "-".repeat(36));
    let ws_windows = 20;
    let ws_size = n / ws_windows;
    let mut seen_pages: std::collections::HashSet<u64> = std::collections::HashSet::new();
    let mut prev = 0;
    for w in 0..ws_windows {
        let start = w * ws_size;
        let end = if w < ws_windows - 1 {
            start + ws_size
        } else {
            n
        };
        for e in &entries[start..end] {
            if e.desc.contains("page_fault") {
                seen_pages.insert(e.tval & !0xFFF);
            }
        }
        let total = seen_pages.len();
        println!(
            "  {:>3}%    {:>12}  {:>12}",
            (w + 1) * 5,
            total,
            total - prev
        );
        prev = total;
    }

    // ── 7. Anomalies ──
    println!("\n  ANOMALIES");

    // Illegal instructions
    let illegals: Vec<_> = entries
        .iter()
        .filter(|e| e.desc == "illegal_instruction")
        .collect();
    if !illegals.is_empty() {
        println!("  Illegal instructions: {}", illegals.len());
        for e in illegals.iter().take(5) {
            let opcode = (e.tval & 0x7F) as u8;
            println!(
                "    epc=0x{:016X}  tval=0x{:08X}  opcode=0x{:02X}",
                e.epc, e.tval, opcode
            );
        }
        if illegals.iter().any(|e| e.epc >= 0xFFFFFFFF80000000) {
            println!("    -> Kernel probes for unsupported CPU extensions");
        }
    }

    // Store page fault storms (5+ consecutive)
    let mut storm_count = 0;
    let mut i = 0;
    while i < entries.len() {
        if entries[i].desc == "store_page_fault" {
            let start = i;
            while i < entries.len() && entries[i].desc == "store_page_fault" {
                i += 1;
            }
            if i - start >= 5 {
                storm_count += 1;
            }
        } else {
            i += 1;
        }
    }
    if storm_count > 0 {
        println!(
            "  Store page fault storms (5+ consecutive): {}",
            storm_count
        );
    }

    // Breakpoints
    let bps: Vec<_> = entries.iter().filter(|e| e.desc == "breakpoint").collect();
    if !bps.is_empty() {
        println!("  Breakpoints: {}", bps.len());
        for e in &bps {
            println!("    epc=0x{:016X}", e.epc);
        }
    }

    if illegals.is_empty() && storm_count == 0 && bps.is_empty() {
        println!("  None detected");
    }
}

fn classify_address(addr: u64) -> &'static str {
    if addr >= 0xFFFFFFFF80000000 {
        "kernel (high mapping)"
    } else if addr >= 0xFFFFFF0000000000 {
        "kernel text"
    } else if addr >= 0x7FF000000000 && addr <= 0x7FFFFFFFFFFF {
        "user library (libc/ld)"
    } else if addr >= 0x555500000000 && addr <= 0x5556FFFFFFFF {
        "user binary (.text)"
    } else if addr < 0x100000000 {
        "lowmem"
    } else {
        "user process"
    }
}
