// canvas.rs -- Canvas buffer helpers and terminal command handler for Geometry OS

use crate::assembler;
use crate::preprocessor;
use crate::render::{CANVAS_BYTECODE_ADDR, CANVAS_COLS, CANVAS_MAX_ROWS, CANVAS_ROWS};
use crate::save::{load_state, save_state};
use crate::vm;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

const SAVE_FILE: &str = "geometry_os.sav";

pub fn write_line_to_canvas(canvas_buffer: &mut [u32], row: usize, text: &str) -> usize {
    let mut r = row;
    if r >= CANVAS_MAX_ROWS {
        return r;
    }
    let bytes = text.as_bytes();
    let mut col = 0usize;
    for &b in bytes {
        if b == b'\n' || col >= CANVAS_COLS {
            // Pad rest of row with zeros
            while col < CANVAS_COLS {
                canvas_buffer[r * CANVAS_COLS + col] = 0;
                col += 1;
            }
            r += 1;
            if r >= CANVAS_MAX_ROWS {
                return r;
            }
            col = 0;
            if b == b'\n' {
                continue;
            }
            // b didn't fit, write it on new line
            canvas_buffer[r * CANVAS_COLS + col] = b as u32;
            col += 1;
        } else {
            canvas_buffer[r * CANVAS_COLS + col] = b as u32;
            col += 1;
        }
    }
    // Pad rest of this row
    while col < CANVAS_COLS {
        canvas_buffer[r * CANVAS_COLS + col] = 0;
        col += 1;
    }
    r + 1
}

/// Read text from a canvas buffer row (up to first null/newline).
pub fn read_canvas_line(canvas_buffer: &[u32], row: usize) -> String {
    let mut s = String::new();
    for col in 0..CANVAS_COLS {
        let val = canvas_buffer[row * CANVAS_COLS + col];
        let byte = (val & 0xFF) as u8;
        if byte == 0 || byte == 0x0A {
            break;
        }
        s.push(byte as char);
    }
    s
}

/// Handle a terminal command. Returns (switch_to_editor, should_quit).

pub fn ensure_scroll(output_row: usize, scroll_offset: &mut usize) {
    if output_row >= *scroll_offset + CANVAS_ROWS {
        *scroll_offset = output_row - CANVAS_ROWS + 1;
    }
}

pub fn source_from_canvas(canvas_buffer: &[u32]) -> String {
    let buffer_size = CANVAS_MAX_ROWS * CANVAS_COLS;
    let source: String = canvas_buffer[..buffer_size]
        .iter()
        .map(|&cell| {
            let val = cell & 0xFF;
            if val == 0 || val == 0x0A {
                '\n'
            } else {
                (val as u8) as char
            }
        })
        .collect();
    source.replace("\n\n", "\n")
}

/// Run the Hermes LLM agent loop, but write all output to the canvas buffer
/// instead of stdout. This is the visual/canvas version of run_hermes_loop().

pub fn ensure_cursor_visible(cursor_row: &usize, scroll_offset: &mut usize) {
    if *cursor_row < *scroll_offset {
        *scroll_offset = *cursor_row;
    } else if *cursor_row >= *scroll_offset + CANVAS_ROWS {
        *scroll_offset = *cursor_row - CANVAS_ROWS + 1;
    }
}

// ── Load source text from a string onto the canvas grid ──────────
pub fn load_source_to_canvas(
    canvas_buffer: &mut [u32],
    source: &str,
    cursor_row: &mut usize,
    cursor_col: &mut usize,
) {
    // Clear canvas buffer
    for cell in canvas_buffer.iter_mut() {
        *cell = 0;
    }

    let mut row = 0usize;
    let mut col = 0usize;

    for ch in source.chars() {
        if row >= CANVAS_MAX_ROWS {
            break;
        }
        if ch == '\n' {
            row += 1;
            col = 0;
        } else if col < CANVAS_COLS {
            canvas_buffer[row * CANVAS_COLS + col] = ch as u32;
            col += 1;
        }
        // characters beyond column 32 on a single line are dropped
    }

    *cursor_row = 0;
    *cursor_col = 0;
}

pub fn paste_text_to_canvas(
    canvas_buffer: &mut [u32],
    text: &str,
    cursor_row: &mut usize,
    cursor_col: &mut usize,
) -> usize {
    let mut row = *cursor_row;
    let mut col = *cursor_col;
    let mut count = 0usize;

    for ch in text.chars() {
        if row >= CANVAS_MAX_ROWS {
            break;
        }
        if ch == '\n' {
            row += 1;
            col = 0;
        } else if ch == '\r' {
            // Skip carriage returns
            continue;
        } else if col < CANVAS_COLS {
            canvas_buffer[row * CANVAS_COLS + col] = ch as u32;
            col += 1;
            if col >= CANVAS_COLS {
                row += 1;
                col = 0;
            }
            count += 1;
        }
    }

    *cursor_row = row.min(CANVAS_MAX_ROWS - 1);
    *cursor_col = col.min(CANVAS_COLS - 1);
    count
}

pub fn canvas_assemble(
    canvas_buffer: &[u32],
    vm: &mut vm::Vm,
    canvas_assembled: &mut bool,
    status_msg: &mut String,
) {
    let buffer_size = CANVAS_MAX_ROWS * CANVAS_COLS;
    let source: String = canvas_buffer[..buffer_size]
        .iter()
        .map(|&cell| {
            let val = cell & 0xFF;
            if val == 0 || val == 0x0A {
                '\n'
            } else {
                (val as u8) as char
            }
        })
        .collect();

    let source = source.replace("\n\n", "\n");

    // Abstraction Layer: Preprocess macros and variables
    let mut pp = preprocessor::Preprocessor::new();
    let preprocessed_source = pp.preprocess(&source);

    match assembler::assemble(&preprocessed_source, CANVAS_BYTECODE_ADDR) {
        Ok(asm_result) => {
            let ram_len = vm.ram.len();
            for v in
                vm.ram[CANVAS_BYTECODE_ADDR..ram_len.min(CANVAS_BYTECODE_ADDR + 4096)].iter_mut()
            {
                *v = 0;
            }
            for (i, &pixel) in asm_result.pixels.iter().enumerate() {
                let addr = CANVAS_BYTECODE_ADDR + i;
                if addr < ram_len {
                    vm.ram[addr] = pixel;
                }
            }
            *canvas_assembled = true;
            vm.pc = CANVAS_BYTECODE_ADDR as u32;
            vm.halted = false;
            *status_msg = format!(
                "[OK: {} bytes at 0x{:04X}]",
                asm_result.pixels.len(),
                CANVAS_BYTECODE_ADDR
            );
        }
        Err(e) => {
            *status_msg = format!("[ASM ERROR line {}: {}]", e.line, e.message);
        }
    }
}

pub fn advance_cursor(
    _canvas_buffer: &mut Vec<u32>,
    row: &mut usize,
    col: &mut usize,
    scroll_offset: &mut usize,
) {
    *col += 1;
    if *col >= CANVAS_COLS {
        *col = 0;
        *row += 1;
        if *row >= CANVAS_MAX_ROWS {
            *row = CANVAS_MAX_ROWS - 1;
        }
    }
    ensure_cursor_visible(row, scroll_offset);
}

pub fn list_asm_files(dir: &str) -> Vec<String> {
    let mut files = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(ext) = path.extension() {
                if ext == "asm" {
                    if let Some(name) = path.to_str() {
                        files.push(name.to_string());
                    }
                }
            }
        }
    }
    files.sort();
    files
}

pub fn handle_terminal_command(
    cmd: &str,
    vm: &mut vm::Vm,
    canvas_buffer: &mut Vec<u32>,
    output_row: &mut usize,
    scroll_offset: &mut usize,
    loaded_file: &mut Option<PathBuf>,
    canvas_assembled: &mut bool,
    breakpoints: &mut HashSet<u32>,
) -> (Option<String>, bool, bool) {
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    if parts.is_empty() {
        // Write a new "geo> " prompt
        *output_row = write_line_to_canvas(canvas_buffer, *output_row, "geo> ");
        ensure_scroll(*output_row, scroll_offset);
        return (None, false, false);
    }

    let command = parts[0].to_lowercase();
    match command.as_str() {
        "help" | "?" => {
            *output_row = write_line_to_canvas(canvas_buffer, *output_row, "Commands:");
            *output_row = write_line_to_canvas(
                canvas_buffer,
                *output_row,
                "  list              List .asm programs",
            );
            *output_row = write_line_to_canvas(
                canvas_buffer,
                *output_row,
                "  load <file>       Load .asm onto canvas",
            );
            *output_row = write_line_to_canvas(
                canvas_buffer,
                *output_row,
                "  run               Assemble canvas & run",
            );
            *output_row = write_line_to_canvas(
                canvas_buffer,
                *output_row,
                "  edit              Switch to canvas editor",
            );
            *output_row = write_line_to_canvas(
                canvas_buffer,
                *output_row,
                "  regs              Show register dump",
            );
            *output_row = write_line_to_canvas(
                canvas_buffer,
                *output_row,
                "  peek <addr>       Read RAM[addr]",
            );
            *output_row = write_line_to_canvas(
                canvas_buffer,
                *output_row,
                "  poke <addr> <val> Write RAM[addr]",
            );
            *output_row = write_line_to_canvas(
                canvas_buffer,
                *output_row,
                "  step              Step one instruction",
            );
            *output_row = write_line_to_canvas(
                canvas_buffer,
                *output_row,
                "  bp [addr]         Toggle/list breakpoints",
            );
            *output_row = write_line_to_canvas(
                canvas_buffer,
                *output_row,
                "  bpc               Clear all breakpoints",
            );
            *output_row = write_line_to_canvas(
                canvas_buffer,
                *output_row,
                "  trace [n]         Execute n steps with log",
            );
            *output_row = write_line_to_canvas(
                canvas_buffer,
                *output_row,
                "  disasm [addr] [n] Disassemble n instrs",
            );
            *output_row = write_line_to_canvas(
                canvas_buffer,
                *output_row,
                "  reset             Reset VM state",
            );
            *output_row = write_line_to_canvas(
                canvas_buffer,
                *output_row,
                "  save [slot]       Save state to slot",
            );
            *output_row = write_line_to_canvas(
                canvas_buffer,
                *output_row,
                "  load [slot]       Load state from slot",
            );
            *output_row = write_line_to_canvas(
                canvas_buffer,
                *output_row,
                "  clear             Clear terminal",
            );
            *output_row = write_line_to_canvas(
                canvas_buffer,
                *output_row,
                "  quit              Exit Geometry OS",
            );
            *output_row = write_line_to_canvas(
                canvas_buffer,
                *output_row,
                "  hermes <prompt>   Ask local LLM for help",
            );
            *output_row = write_line_to_canvas(
                canvas_buffer,
                *output_row,
                "  qemu boot <cfg>   Boot QEMU VM on canvas",
            );
            *output_row = write_line_to_canvas(canvas_buffer, *output_row, "geo> ");
            ensure_scroll(*output_row, scroll_offset);
            (None, false, false)
        }
        "list" | "ls" => {
            let files = list_asm_files("programs");
            if files.is_empty() {
                *output_row = write_line_to_canvas(
                    canvas_buffer,
                    *output_row,
                    "  (no .asm files in programs/)",
                );
            } else {
                for f in &files {
                    let name = Path::new(f)
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| f.clone());
                    *output_row =
                        write_line_to_canvas(canvas_buffer, *output_row, &format!("  {}", name));
                }
                *output_row = write_line_to_canvas(
                    canvas_buffer,
                    *output_row,
                    &format!("  {} programs", files.len()),
                );
            }
            *output_row = write_line_to_canvas(canvas_buffer, *output_row, "geo> ");
            ensure_scroll(*output_row, scroll_offset);
            (None, false, false)
        }
        "load" => {
            if parts.len() < 2 {
                *output_row = write_line_to_canvas(
                    canvas_buffer,
                    *output_row,
                    "Usage: load <file.asm> or load <slot>",
                );
                *output_row = write_line_to_canvas(canvas_buffer, *output_row, "geo> ");
                ensure_scroll(*output_row, scroll_offset);
                return (None, false, false);
            }
            let filename_arg = parts[1..].join(" ");

            // If it ends in .asm or contains a path separator, assume source file
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
                        *output_row = write_line_to_canvas(
                            canvas_buffer,
                            *output_row,
                            &format!("File not found: {}", filename),
                        );
                        *output_row = write_line_to_canvas(canvas_buffer, *output_row, "geo> ");
                        ensure_scroll(*output_row, scroll_offset);
                        return (None, false, false);
                    }
                };

                match std::fs::read_to_string(&path) {
                    Ok(source) => {
                        let mut cr = 0usize;
                        let mut cc = 0usize;
                        load_source_to_canvas(canvas_buffer, &source, &mut cr, &mut cc);
                        *loaded_file = Some(path.clone());
                        let name = path
                            .file_name()
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_default();
                        let lines = source.lines().count();
                        *output_row = write_line_to_canvas(
                            canvas_buffer,
                            *output_row,
                            &format!("Loaded {} ({} lines)", name, lines),
                        );
                    }
                    Err(e) => {
                        *output_row = write_line_to_canvas(
                            canvas_buffer,
                            *output_row,
                            &format!("Error: {}", e),
                        );
                    }
                }
            } else {
                // Assume it's a state slot
                let filename = format!("geometry_os_{}.sav", filename_arg);
                match load_state(&filename) {
                    Ok((saved_vm, saved_canvas, saved_assembled)) => {
                        *vm = saved_vm;
                        *canvas_buffer = saved_canvas;
                        *canvas_assembled = saved_assembled;
                        let msg = format!("Loaded state from {}", filename);
                        *output_row = write_line_to_canvas(canvas_buffer, *output_row, &msg);
                    }
                    Err(_) => {
                        // Fallback: try loading as .asm if slot not found
                        let mut filename = filename_arg.clone();
                        filename.push_str(".asm");
                        let path = Path::new("programs").join(&filename);
                        if path.exists() {
                            if let Ok(source) = std::fs::read_to_string(&path) {
                                let mut cr = 0usize;
                                let mut cc = 0usize;
                                load_source_to_canvas(canvas_buffer, &source, &mut cr, &mut cc);
                                *loaded_file = Some(path.clone());
                                *output_row = write_line_to_canvas(
                                    canvas_buffer,
                                    *output_row,
                                    &format!("Loaded programs/{}", filename),
                                );
                            } else {
                                *output_row = write_line_to_canvas(
                                    canvas_buffer,
                                    *output_row,
                                    &format!(
                                        "Slot {} not found and could not read .asm",
                                        filename_arg
                                    ),
                                );
                            }
                        } else {
                            *output_row = write_line_to_canvas(
                                canvas_buffer,
                                *output_row,
                                &format!("Slot or file {} not found", filename_arg),
                            );
                        }
                    }
                }
            }
            *output_row = write_line_to_canvas(canvas_buffer, *output_row, "geo> ");
            ensure_scroll(*output_row, scroll_offset);
            (None, false, false)
        }
        "run" => {
            let buffer_size = CANVAS_MAX_ROWS * CANVAS_COLS;
            let source: String = canvas_buffer[..buffer_size]
                .iter()
                .map(|&cell| {
                    let val = cell & 0xFF;
                    if val == 0 || val == 0x0A {
                        '\n'
                    } else {
                        (val as u8) as char
                    }
                })
                .collect();
            let source = source.replace("\n\n", "\n");

            match assembler::assemble(&source, CANVAS_BYTECODE_ADDR) {
                Ok(asm_result) => {
                    let ram_len = vm.ram.len();
                    for v in vm.ram[CANVAS_BYTECODE_ADDR..ram_len.min(CANVAS_BYTECODE_ADDR + 4096)]
                        .iter_mut()
                    {
                        *v = 0;
                    }
                    for (i, &pixel) in asm_result.pixels.iter().enumerate() {
                        let addr = CANVAS_BYTECODE_ADDR + i;
                        if addr < ram_len {
                            vm.ram[addr] = pixel;
                        }
                    }
                    vm.pc = CANVAS_BYTECODE_ADDR as u32;
                    vm.halted = false;
                    *output_row = write_line_to_canvas(
                        canvas_buffer,
                        *output_row,
                        &format!("Assembled {} bytes at 0x1000", asm_result.pixels.len()),
                    );

                    // Phase 45: Sync canvas buffer TO VM before execution
                    vm.canvas_buffer.copy_from_slice(canvas_buffer);

                    // Run the VM
                    for _ in 0..10_000_000 {
                        if !vm.step() {
                            break;
                        }
                    }

                    // Phase 45: Sync canvas buffer FROM VM after execution
                    canvas_buffer.copy_from_slice(&vm.canvas_buffer);

                    if vm.halted {
                        *output_row = write_line_to_canvas(
                            canvas_buffer,
                            *output_row,
                            &format!("Halted at PC=0x{:04X}", vm.pc),
                        );
                    } else {
                        *output_row = write_line_to_canvas(
                            canvas_buffer,
                            *output_row,
                            &format!("Running... PC=0x{:04X}", vm.pc),
                        );
                    }
                    *canvas_assembled = true;
                }
                Err(e) => {
                    *output_row =
                        write_line_to_canvas(canvas_buffer, *output_row, &format!("{}", e));
                }
            }
            *output_row = write_line_to_canvas(canvas_buffer, *output_row, "geo> ");
            ensure_scroll(*output_row, scroll_offset);
            (None, false, false)
        }
        "edit" => (None, true, false),
        "regs" => {
            for row_group in 0..4 {
                let mut line = String::new();
                for col in 0..8 {
                    let i = row_group * 8 + col;
                    line.push_str(&format!("r{:02}={:08X} ", i, vm.regs[i]));
                }
                *output_row = write_line_to_canvas(canvas_buffer, *output_row, &line);
            }
            *output_row = write_line_to_canvas(
                canvas_buffer,
                *output_row,
                &format!(
                    "PC={:04X} SP={:04X} LR={:04X}",
                    vm.pc, vm.regs[30], vm.regs[31]
                ),
            );
            *output_row = write_line_to_canvas(canvas_buffer, *output_row, "geo> ");
            ensure_scroll(*output_row, scroll_offset);
            (None, false, false)
        }
        "peek" => {
            if parts.len() < 2 {
                *output_row =
                    write_line_to_canvas(canvas_buffer, *output_row, "Usage: peek <addr>");
                *output_row = write_line_to_canvas(canvas_buffer, *output_row, "geo> ");
                ensure_scroll(*output_row, scroll_offset);
                return (None, false, false);
            }
            match u32::from_str_radix(
                parts[1].trim_start_matches("0x").trim_start_matches("0X"),
                16,
            ) {
                Ok(addr) if (addr as usize) < vm.ram.len() => {
                    let val = vm.ram[addr as usize];
                    *output_row = write_line_to_canvas(
                        canvas_buffer,
                        *output_row,
                        &format!("RAM[0x{:04X}] = 0x{:08X}", addr, val),
                    );
                }
                Ok(addr) => {
                    *output_row = write_line_to_canvas(
                        canvas_buffer,
                        *output_row,
                        &format!("Address 0x{:04X} out of range", addr),
                    );
                }
                Err(_) => {
                    *output_row =
                        write_line_to_canvas(canvas_buffer, *output_row, "Invalid address");
                }
            }
            *output_row = write_line_to_canvas(canvas_buffer, *output_row, "geo> ");
            ensure_scroll(*output_row, scroll_offset);
            (None, false, false)
        }
        "poke" => {
            if parts.len() < 3 {
                *output_row =
                    write_line_to_canvas(canvas_buffer, *output_row, "Usage: poke <addr> <val>");
                *output_row = write_line_to_canvas(canvas_buffer, *output_row, "geo> ");
                ensure_scroll(*output_row, scroll_offset);
                return (None, false, false);
            }
            let addr_str = parts[1].trim_start_matches("0x").trim_start_matches("0X");
            let val_str = parts[2].trim_start_matches("0x").trim_start_matches("0X");
            match (
                u32::from_str_radix(addr_str, 16),
                u32::from_str_radix(val_str, 16),
            ) {
                (Ok(addr), Ok(val)) if (addr as usize) < vm.ram.len() => {
                    vm.ram[addr as usize] = val;
                    *output_row = write_line_to_canvas(
                        canvas_buffer,
                        *output_row,
                        &format!("RAM[0x{:04X}] <- 0x{:08X}", addr, val),
                    );
                }
                _ => {
                    *output_row = write_line_to_canvas(
                        canvas_buffer,
                        *output_row,
                        "Usage: poke <hex_addr> <hex_val>",
                    );
                }
            }
            *output_row = write_line_to_canvas(canvas_buffer, *output_row, "geo> ");
            ensure_scroll(*output_row, scroll_offset);
            (None, false, false)
        }
        "step" => {
            if vm.halted {
                *output_row = write_line_to_canvas(
                    canvas_buffer,
                    *output_row,
                    "VM halted. Use reset to restart.",
                );
            } else {
                // Phase 45: Sync canvas buffer TO VM before execution
                vm.canvas_buffer.copy_from_slice(canvas_buffer);

                vm.step();

                // Phase 45: Sync canvas buffer FROM VM after execution
                canvas_buffer.copy_from_slice(&vm.canvas_buffer);

                *output_row = write_line_to_canvas(
                    canvas_buffer,
                    *output_row,
                    &format!("step -> PC=0x{:04X}", vm.pc),
                );
            }
            *output_row = write_line_to_canvas(canvas_buffer, *output_row, "geo> ");
            ensure_scroll(*output_row, scroll_offset);
            (None, false, false)
        }
        "bp" => {
            if parts.len() < 2 {
                // List breakpoints
                if breakpoints.is_empty() {
                    *output_row =
                        write_line_to_canvas(canvas_buffer, *output_row, "  No breakpoints set");
                } else {
                    let mut sorted: Vec<u32> = breakpoints.iter().copied().collect();
                    sorted.sort();
                    for addr in sorted {
                        *output_row = write_line_to_canvas(
                            canvas_buffer,
                            *output_row,
                            &format!("  BP @ 0x{:04X}", addr),
                        );
                    }
                }
            } else {
                match u32::from_str_radix(
                    parts[1].trim_start_matches("0x").trim_start_matches("0X"),
                    16,
                ) {
                    Ok(addr) => {
                        if breakpoints.contains(&addr) {
                            breakpoints.remove(&addr);
                            *output_row = write_line_to_canvas(
                                canvas_buffer,
                                *output_row,
                                &format!("Cleared BP @ 0x{:04X}", addr),
                            );
                        } else {
                            breakpoints.insert(addr);
                            *output_row = write_line_to_canvas(
                                canvas_buffer,
                                *output_row,
                                &format!("Set BP @ 0x{:04X}", addr),
                            );
                        }
                    }
                    Err(_) => {
                        *output_row = write_line_to_canvas(
                            canvas_buffer,
                            *output_row,
                            "Usage: bp <hex_addr>",
                        );
                    }
                }
            }
            *output_row = write_line_to_canvas(canvas_buffer, *output_row, "geo> ");
            ensure_scroll(*output_row, scroll_offset);
            (None, false, false)
        }
        "bpc" => {
            let n = breakpoints.len();
            breakpoints.clear();
            *output_row = write_line_to_canvas(
                canvas_buffer,
                *output_row,
                &format!("Cleared {} breakpoint(s)", n),
            );
            *output_row = write_line_to_canvas(canvas_buffer, *output_row, "geo> ");
            ensure_scroll(*output_row, scroll_offset);
            (None, false, false)
        }
        "reset" => {
            vm.reset();
            *canvas_assembled = false;
            *output_row = write_line_to_canvas(canvas_buffer, *output_row, "VM reset");
            *output_row = write_line_to_canvas(canvas_buffer, *output_row, "geo> ");
            ensure_scroll(*output_row, scroll_offset);
            (None, false, false)
        }
        "save" => {
            let slot = parts.get(1).copied();
            let filename = match slot {
                Some(s) => format!("geometry_os_{}.sav", s),
                None => SAVE_FILE.to_string(),
            };
            match save_state(&filename, vm, canvas_buffer, *canvas_assembled) {
                Ok(()) => {
                    let msg = format!("Saved state to {}", filename);
                    *output_row = write_line_to_canvas(canvas_buffer, *output_row, &msg);
                }
                Err(e) => {
                    let msg = format!("Save error: {}", e);
                    *output_row = write_line_to_canvas(canvas_buffer, *output_row, &msg);
                }
            }
            *output_row = write_line_to_canvas(canvas_buffer, *output_row, "geo> ");
            ensure_scroll(*output_row, scroll_offset);
            (None, false, false)
        }

        "clear" | "cls" => {
            for cell in canvas_buffer.iter_mut() {
                *cell = 0;
            }
            *output_row = 0;
            *output_row = write_line_to_canvas(canvas_buffer, 0, "geo> ");
            *scroll_offset = 0;
            (None, false, false)
        }
        "hermes" => {
            if parts.len() < 2 {
                *output_row =
                    write_line_to_canvas(canvas_buffer, *output_row, "Usage: hermes <prompt>");
                *output_row = write_line_to_canvas(
                    canvas_buffer,
                    *output_row,
                    "  Asks a local LLM to help you via Ollama.",
                );
                *output_row = write_line_to_canvas(canvas_buffer, *output_row, "geo> ");
                ensure_scroll(*output_row, scroll_offset);
                return (None, false, false);
            }
            let user_prompt = parts[1..].join(" ");
            // Return the prompt so main.rs can call run_hermes_canvas (avoids circular dep)
            (Some(user_prompt), false, false)
        }
        "quit" | "exit" => (None, false, true),
        _ => {
            *output_row = write_line_to_canvas(
                canvas_buffer,
                *output_row,
                &format!("Unknown: {} (try help)", command),
            );
            *output_row = write_line_to_canvas(canvas_buffer, *output_row, "geo> ");
            ensure_scroll(*output_row, scroll_offset);
            (None, false, false)
        }
    }
}
