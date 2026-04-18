use super::types::*;
use super::trace::MAX_SNAPSHOTS;
use super::Vm;

impl Vm {
    /// Handle extended opcodes (0x62-0x7D): IOCTL, env, process control,
    /// signals, hypervisor, self-hosting, reactive formulas, inode ops, trace,
    /// timeline forking.
    /// Returns false if halted, true otherwise.
    #[allow(unreachable_patterns)]
    pub(super) fn step_extended(&mut self, opcode: u32) -> bool {
        match opcode {
            // IOCTL fd_reg, cmd_reg, arg_reg  -- device-specific control operations
            // r0 = result (device-dependent), 0xFFFFFFFF on error
            // Screen: cmd 0 = get width in r0, cmd 1 = get height in r0
            // Keyboard: cmd 0 = get echo mode, cmd 1 = set echo mode (arg)
            // Audio: cmd 0 = get volume, cmd 1 = set volume (arg 0-100)
            // Net: cmd 0 = get status
            0x62 => {
                let fd_reg = self.fetch() as usize;
                let cmd_reg = self.fetch() as usize;
                let arg_reg = self.fetch() as usize;
                if fd_reg < NUM_REGS && cmd_reg < NUM_REGS && arg_reg < NUM_REGS {
                    let fd = self.regs[fd_reg];
                    let cmd = self.regs[cmd_reg];
                    let arg = self.regs[arg_reg];
                    // Must be a device fd
                    let dev_idx = fd.wrapping_sub(DEVICE_FD_BASE) as usize;
                    if fd >= DEVICE_FD_BASE && dev_idx < DEVICE_COUNT {
                        match dev_idx {
                            0 => {
                                // /dev/screen
                                match cmd {
                                    0 => self.regs[0] = 256, // width
                                    1 => self.regs[0] = 256, // height
                                    _ => self.regs[0] = 0xFFFFFFFF,
                                }
                            }
                            1 => {
                                // /dev/keyboard
                                match cmd {
                                    0 => self.regs[0] = self.ram[0xFF8], // get echo mode
                                    1 => {
                                        self.ram[0xFF8] = arg;
                                        self.regs[0] = 0;
                                    }
                                    _ => self.regs[0] = 0xFFFFFFFF,
                                }
                            }
                            2 => {
                                // /dev/audio
                                match cmd {
                                    0 => self.regs[0] = self.ram[0xFF7], // get volume
                                    1 => {
                                        self.ram[0xFF7] = arg.min(100);
                                        self.regs[0] = 0;
                                    }
                                    _ => self.regs[0] = 0xFFFFFFFF,
                                }
                            }
                            3 => {
                                // /dev/net
                                match cmd {
                                    0 => self.regs[0] = 1, // status: up
                                    _ => self.regs[0] = 0xFFFFFFFF,
                                }
                            }
                            _ => self.regs[0] = 0xFFFFFFFF,
                        }
                    } else {
                        self.regs[0] = 0xFFFFFFFF; // not a device fd
                    }
                } else {
                    self.regs[0] = 0xFFFFFFFF;
                }
            }

            // GETENV key_addr_reg, val_addr_reg  -- read environment variable
            // Reads null-terminated key from RAM[key_addr], writes value to RAM[val_addr].
            // r0 = value string length, or 0xFFFFFFFF if not found.
            // Max key/value length: 64 chars.
            0x63 => {
                let kr = self.fetch() as usize;
                let vr = self.fetch() as usize;
                if kr < NUM_REGS && vr < NUM_REGS {
                    let key_addr = self.regs[kr] as usize;
                    let val_addr = self.regs[vr] as usize;
                    let key = self.read_ram_string(key_addr, 64);
                    if let Some(k) = &key {
                        if let Some(val) = self.env_vars.get(k) {
                            let bytes = val.as_bytes();
                            let len = bytes.len().min(64);
                            for (i, &byte) in bytes.iter().enumerate().take(len) {
                                let addr = val_addr + i;
                                if addr < self.ram.len() {
                                    self.ram[addr] = byte as u32;
                                }
                            }
                            // Null terminate
                            if val_addr + len < self.ram.len() {
                                self.ram[val_addr + len] = 0;
                            }
                            self.regs[0] = len as u32;
                        } else {
                            self.regs[0] = 0xFFFFFFFF;
                        }
                    } else {
                        self.regs[0] = 0xFFFFFFFF;
                    }
                }
            }

            // SETENV key_addr_reg, val_addr_reg  -- set environment variable
            // Reads null-terminated key and value from RAM.
            // r0 = 0 on success, 0xFFFFFFFF on error.
            // Max key/value length: 64 chars. Max 32 env vars.
            0x64 => {
                let kr = self.fetch() as usize;
                let vr = self.fetch() as usize;
                if kr < NUM_REGS && vr < NUM_REGS {
                    let key_addr = self.regs[kr] as usize;
                    let val_addr = self.regs[vr] as usize;
                    let key = self.read_ram_string(key_addr, 64);
                    let val = self.read_ram_string(val_addr, 64);
                    match (key, val) {
                        (Some(k), Some(v)) => {
                            if self.env_vars.len() < 32 || self.env_vars.contains_key(&k) {
                                self.env_vars.insert(k, v);
                                self.regs[0] = 0;
                            } else {
                                self.regs[0] = 0xFFFFFFFF; // too many env vars
                            }
                        }
                        _ => self.regs[0] = 0xFFFFFFFF,
                    }
                }
            }

            // GETPID -- get current process ID
            // r0 = PID (0 = main/kernel context, 1+ = spawned child)
            0x65 => {
                self.regs[0] = self.current_pid;
            }

            // EXEC path_addr_reg  -- assemble and spawn a program from the programs/ directory
            // Reads null-terminated filename from RAM[path_addr]. Appends ".asm" if needed.
            // Assembles the source, creates a new process, copies bytecode in.
            // r0 = PID on success, 0xFFFFFFFF on error.
            // RAM[0xFFA] = PID on success, 0xFFFFFFFF on error.
            0x66 => {
                let pr = self.fetch() as usize;
                if pr < NUM_REGS {
                    let path_addr = self.regs[pr] as usize;
                    let filename = self.read_ram_string(path_addr, 64);
                    match filename {
                        Some(mut fname) => {
                            // Append .asm if not already present
                            if !fname.ends_with(".asm") {
                                fname.push_str(".asm");
                            }
                            let prog_path = std::path::Path::new("programs").join(&fname);
                            let source = match std::fs::read_to_string(&prog_path) {
                                Ok(s) => s,
                                Err(_) => {
                                    self.regs[0] = 0xFFFFFFFF;
                                    self.ram[0xFFA] = 0xFFFFFFFF;
                                    return true;
                                }
                            };
                            match crate::assembler::assemble(&source, 0) {
                                Ok(asm_result) => {
                                    let active_count =
                                        self.processes.iter().filter(|p| !p.is_halted()).count();
                                    if active_count >= MAX_PROCESSES {
                                        self.regs[0] = 0xFFFFFFFF;
                                        self.ram[0xFFA] = 0xFFFFFFFF;
                                    } else {
                                        let page_dir = self.create_process_page_dir();
                                        match page_dir {
                                            Some(pd) => {
                                                let phys_base = (pd[0] as usize) * PAGE_SIZE;
                                                // Copy assembled bytecode into new process's physical memory
                                                for (i, &word) in
                                                    asm_result.pixels.iter().enumerate()
                                                {
                                                    let addr = phys_base + i;
                                                    if addr >= self.ram.len() {
                                                        break;
                                                    }
                                                    self.ram[addr] = word;
                                                }
                                                let pid = (self.processes.len() + 1) as u32;
                                                self.processes.push(SpawnedProcess {
                                                    pc: 0,
                                                    regs: [0; NUM_REGS],
                                                    state: ProcessState::Ready,
                                                    pid,
                                                    mode: CpuMode::User,
                                                    page_dir: Some(pd),
                                                    segfaulted: false,
                                                    priority: 1,
                                                    slice_remaining: 0,
                                                    sleep_until: 0,
                                                    yielded: false,
                                                    kernel_stack: Vec::new(),
                                                    msg_queue: Vec::new(),
                                                    exit_code: 0,
                                                    parent_pid: self.current_pid,
                                                    pending_signals: Vec::new(),
                                                    signal_handlers: [0; 4],
                                                    vmas: Process::default_vmas_for_process(),
                                                    brk_pos: PAGE_SIZE as u32,
                                                });
                                                self.regs[0] = pid;
                                                self.ram[0xFFA] = pid;
                                            }
                                            None => {
                                                self.regs[0] = 0xFFFFFFFF;
                                                self.ram[0xFFA] = 0xFFFFFFFF;
                                            }
                                        }
                                    }
                                }
                                Err(_) => {
                                    self.regs[0] = 0xFFFFFFFF;
                                    self.ram[0xFFA] = 0xFFFFFFFF;
                                }
                            }
                        }
                        None => {
                            self.regs[0] = 0xFFFFFFFF;
                            self.ram[0xFFA] = 0xFFFFFFFF;
                        }
                    }
                }
            }

            // WRITESTR fd_reg, str_addr_reg  -- write null-terminated string to file descriptor
            // Scans RAM from str_addr until null byte, writes all bytes to fd.
            // r0 = bytes written, or 0xFFFFFFFF on error.
            0x67 => {
                let fr = self.fetch() as usize;
                let sr = self.fetch() as usize;
                if fr < NUM_REGS && sr < NUM_REGS {
                    let fd = self.regs[fr];
                    let str_addr = self.regs[sr] as usize;
                    // Measure string length
                    let mut len = 0usize;
                    let mut a = str_addr;
                    while a < self.ram.len() && len < 1024 {
                        if (self.ram[a] & 0xFF) == 0 {
                            break;
                        }
                        len += 1;
                        a += 1;
                    }
                    if len > 0 {
                        let n = self.vfs.fwrite(
                            &self.ram,
                            fd,
                            str_addr as u32,
                            len as u32,
                            self.current_pid,
                        );
                        self.regs[0] = n;
                    } else {
                        self.regs[0] = 0; // empty string, 0 bytes written
                    }
                } else {
                    self.regs[0] = 0xFFFFFFFF;
                }
            }

            // READLN buf_addr_reg, max_len_reg, pos_addr_reg
            // Read one character from keyboard into line buffer.
            // Uses: r0 = buffer start addr, r1 = max length, r2 = pointer to current position.
            // Keyboard char read from RAM[0xFFF].
            // r0 return: 0 = waiting/char stored, >0 = line length (Enter pressed).
            // Sets self.yielded when no key or waiting for child.
            0x68 => {
                let br = self.fetch() as usize;
                let mr = self.fetch() as usize;
                let pr = self.fetch() as usize;
                if br < NUM_REGS && mr < NUM_REGS && pr < NUM_REGS {
                    let buf_addr = self.regs[br] as usize;
                    let max_len = self.regs[mr] as usize;
                    let pos_addr = self.regs[pr] as usize;
                    let pos = self.ram[pos_addr] as usize;
                    let key = self.ram[0xFFF];

                    if key == 0 {
                        // No key available -- yield
                        self.regs[0] = 0;
                        self.yielded = true;
                    } else if key == 13 {
                        // Enter -- terminate line
                        if pos < self.ram.len() {
                            self.ram[buf_addr + pos] = 0; // null terminate
                        }
                        self.regs[0] = pos as u32;
                        self.ram[pos_addr] = 0; // reset position
                        self.ram[0xFFF] = 0; // consume key
                    } else if key == 8 {
                        // Backspace
                        if pos > 0 {
                            self.ram[pos_addr] = (pos - 1) as u32;
                        }
                        self.regs[0] = 0;
                        self.ram[0xFFF] = 0;
                    } else if key >= 32 && pos < max_len {
                        // Printable character
                        if buf_addr + pos < self.ram.len() {
                            self.ram[buf_addr + pos] = key;
                        }
                        self.ram[pos_addr] = (pos + 1) as u32;
                        self.regs[0] = 0;
                        self.ram[0xFFF] = 0;
                    } else {
                        // Non-printable or buffer full -- discard
                        self.regs[0] = 0;
                        self.ram[0xFFF] = 0;
                    }
                } else {
                    self.regs[0] = 0xFFFFFFFF;
                }
            }

            // WAITPID pid_reg -- wait for child process to halt.
            // r0 = 0 if process still running (yields), 1 if halted/not found.
            // r1 = exit code of the child (0 if still running or not found).
            // Reaps zombie processes (frees pages, removes from list).
            0x69 => {
                let pr = self.fetch() as usize;
                if pr < NUM_REGS {
                    let target_pid = self.regs[pr];
                    let mut found_running = false;
                    let mut found_zombie = false;
                    let mut zombie_exit_code = 0u32;
                    let mut zombie_page_dir: Option<Vec<u32>> = None;
                    for proc in &self.processes {
                        if proc.pid == target_pid {
                            if proc.state == ProcessState::Zombie {
                                found_zombie = true;
                                zombie_exit_code = proc.exit_code;
                                zombie_page_dir = proc.page_dir.clone();
                            } else if !proc.is_halted() {
                                found_running = true;
                            } else {
                                self.regs[0] = 1;
                                self.regs[1] = proc.exit_code;
                            }
                            break;
                        }
                    }
                    if found_zombie {
                        self.regs[0] = 1;
                        self.regs[1] = zombie_exit_code;
                        if let Some(pd) = zombie_page_dir {
                            self.free_page_dir(&pd);
                        }
                        self.vfs.close_all(target_pid);
                        self.processes.retain(|p| p.pid != target_pid);
                    } else if found_running {
                        self.regs[0] = 0;
                        self.regs[1] = 0;
                        self.yielded = true;
                    } else {
                        self.regs[0] = 1;
                        self.regs[1] = 0;
                    }
                } else {
                    self.regs[0] = 1;
                    self.regs[1] = 0;
                }
            }

            // EXECP path_reg, stdin_fd_reg, stdout_fd_reg
            // Like EXEC but with fd redirection for pipes/redirects.
            // Assembles and spawns a program from programs/ directory.
            // stdin_fd/stdout_fd: 0xFFFFFFFF = default, otherwise fd to dup into child's fd 0/1.
            0x6A => {
                let path_r = self.fetch() as usize;
                let stdin_r = self.fetch() as usize;
                let stdout_r = self.fetch() as usize;
                if path_r < NUM_REGS && stdin_r < NUM_REGS && stdout_r < NUM_REGS {
                    let path_addr = self.regs[path_r] as usize;
                    let stdin_fd = self.regs[stdin_r];
                    let stdout_fd = self.regs[stdout_r];
                    let filename = self.read_ram_string(path_addr, 64);
                    match filename {
                        Some(mut fname) => {
                            if !fname.ends_with(".asm") {
                                fname.push_str(".asm");
                            }
                            let prog_path = std::path::Path::new("programs").join(&fname);
                            let source = match std::fs::read_to_string(&prog_path) {
                                Ok(s) => s,
                                Err(_) => {
                                    self.regs[0] = 0xFFFFFFFF;
                                    return true;
                                }
                            };
                            match crate::assembler::assemble(&source, 0) {
                                Ok(asm_result) => {
                                    let active_count =
                                        self.processes.iter().filter(|p| !p.is_halted()).count();
                                    if active_count >= MAX_PROCESSES {
                                        self.regs[0] = 0xFFFFFFFF;
                                    } else {
                                        let page_dir = self.create_process_page_dir();
                                        match page_dir {
                                            Some(pd) => {
                                                let phys_base = (pd[0] as usize) * PAGE_SIZE;
                                                for (i, &word) in
                                                    asm_result.pixels.iter().enumerate()
                                                {
                                                    let addr = phys_base + i;
                                                    if addr >= self.ram.len() {
                                                        break;
                                                    }
                                                    self.ram[addr] = word;
                                                }
                                                let pid = (self.processes.len() + 1) as u32;
                                                self.processes.push(SpawnedProcess {
                                                    pc: 0,
                                                    regs: [0; NUM_REGS],
                                                    state: ProcessState::Ready,
                                                    pid,
                                                    mode: CpuMode::User,
                                                    page_dir: Some(pd),
                                                    segfaulted: false,
                                                    priority: 1,
                                                    slice_remaining: 0,
                                                    sleep_until: 0,
                                                    yielded: false,
                                                    kernel_stack: Vec::new(),
                                                    msg_queue: Vec::new(),
                                                    exit_code: 0,
                                                    parent_pid: self.current_pid,
                                                    pending_signals: Vec::new(),
                                                    signal_handlers: [0; 4],
                                                    vmas: Process::default_vmas_for_process(),
                                                    brk_pos: PAGE_SIZE as u32,
                                                });
                                                // Set up fd redirection for the new child
                                                let child_pid = pid;
                                                if stdin_fd != 0xFFFFFFFF {
                                                    self.vfs.dup_fd(
                                                        stdin_fd,
                                                        0,
                                                        child_pid,
                                                        self.current_pid,
                                                    );
                                                }
                                                if stdout_fd != 0xFFFFFFFF {
                                                    self.vfs.dup_fd(
                                                        stdout_fd,
                                                        1,
                                                        child_pid,
                                                        self.current_pid,
                                                    );
                                                }
                                                self.regs[0] = pid;
                                                self.ram[0xFFA] = pid;
                                            }
                                            None => {
                                                self.regs[0] = 0xFFFFFFFF;
                                                self.ram[0xFFA] = 0xFFFFFFFF;
                                            }
                                        }
                                    }
                                }
                                Err(_) => {
                                    self.regs[0] = 0xFFFFFFFF;
                                    self.ram[0xFFA] = 0xFFFFFFFF;
                                }
                            }
                        }
                        None => {
                            self.regs[0] = 0xFFFFFFFF;
                        }
                    }
                } else {
                    self.regs[0] = 0xFFFFFFFF;
                }
            }

            // CHDIR path_reg -- change current working directory.
            // Reads null-terminated path from RAM. Stores in env_vars["CWD"].
            // r0 = 0 on success, 0xFFFFFFFF on error.
            0x6B => {
                let pr = self.fetch() as usize;
                if pr < NUM_REGS {
                    let path_addr = self.regs[pr] as usize;
                    let path = self.read_ram_string(path_addr, 256);
                    match path {
                        Some(p) if !p.is_empty() => {
                            self.env_vars.insert("CWD".to_string(), p);
                            self.regs[0] = 0;
                        }
                        _ => {
                            self.regs[0] = 0xFFFFFFFF;
                        }
                    }
                } else {
                    self.regs[0] = 0xFFFFFFFF;
                }
            }

            // GETCWD buf_reg -- write current working directory to RAM buffer.
            // Reads null-terminated CWD from env_vars, writes to buf.
            // r0 = string length, 0 if no CWD set.
            0x6C => {
                let br = self.fetch() as usize;
                if br < NUM_REGS {
                    let buf_addr = self.regs[br] as usize;
                    let cwd = self
                        .env_vars
                        .get("CWD")
                        .cloned()
                        .unwrap_or_else(|| "/".to_string());
                    let bytes = cwd.as_bytes();
                    for (i, &b) in bytes.iter().enumerate() {
                        if buf_addr + i < self.ram.len() {
                            self.ram[buf_addr + i] = b as u32;
                        }
                    }
                    if buf_addr + bytes.len() < self.ram.len() {
                        self.ram[buf_addr + bytes.len()] = 0; // null terminate
                    }
                    self.regs[0] = bytes.len() as u32;
                } else {
                    self.regs[0] = 0;
                }
            }

            // PEEK dest_reg, x_reg, y_reg  -- read pixel from screen buffer
            // r[dest_reg] = screen[y * 256 + x], or 0 if out of bounds.
            // This is the read counterpart to PSET: lets programs inspect what's
            // on screen for collision detection, copy, or visual queries.
            // On unified hardware (memory = display), this would be a normal memory load.
            0x6D => {
                let dr = self.fetch() as usize;
                let xr = self.fetch() as usize;
                let yr = self.fetch() as usize;
                if dr < NUM_REGS && xr < NUM_REGS && yr < NUM_REGS {
                    let x = self.regs[xr] as usize;
                    let y = self.regs[yr] as usize;
                    if x < 256 && y < 256 {
                        self.regs[dr] = self.screen[y * 256 + x];
                    } else {
                        self.regs[dr] = 0; // out of bounds returns black/transparent
                    }
                }
            }

            // SHUTDOWN -- gracefully stop all processes and halt the system
            // Only works in Kernel mode. In User mode, sets r0 = 0xFFFFFFFF.
            // Kills all child processes, closes all file descriptors, then halts.
            // The host (main.rs) can check vm.shutdown_requested to react.
            0x6E => {
                if self.mode != CpuMode::Kernel {
                    self.regs[0] = 0xFFFFFFFF;
                } else {
                    // Collect page dirs to free and PIDs to close
                    let page_dirs: Vec<Vec<u32>> = self
                        .processes
                        .iter()
                        .filter(|p| !p.is_halted())
                        .filter_map(|p| p.page_dir.clone())
                        .collect();
                    let pids: Vec<u32> = self
                        .processes
                        .iter()
                        .filter(|p| !p.is_halted())
                        .map(|p| p.pid)
                        .collect();
                    // Free page directories
                    for pd in page_dirs {
                        self.free_page_dir(&pd);
                    }
                    // Halt all processes
                    for proc in &mut self.processes {
                        proc.state = ProcessState::Zombie;
                    }
                    // Close all open file descriptors
                    self.vfs.close_all(0); // main process (pid 0)
                    for pid in pids {
                        self.vfs.close_all(pid);
                    }
                    // Clear all pipes
                    self.pipes.clear();
                    self.shutdown_requested = true;
                    self.halted = true;
                    return false;
                }
            }

            // EXIT code_reg -- exit with status code.
            // Child processes become zombies (parent reaps via WAITPID).
            // Main process just halts.
            0x6F => {
                let cr = self.fetch() as usize;
                if cr < NUM_REGS {
                    let code = self.regs[cr];
                    self.halted = true;
                    if self.current_pid > 0 {
                        self.step_exit_code = Some(code);
                        self.step_zombie = true;
                    }
                    return false;
                }
            }

            // SIGNAL pid_reg, sig_reg -- send signal to process.
            // Signal 0 (TERM): halt with exit code 1. Signal 3 (STOP): halt with exit code 2.
            // Signals 1-2 (USER): jump to handler if set, else ignore.
            // r0 = 0 on success, 0xFFFFFFFF on error.
            0x70 => {
                let pr = self.fetch() as usize;
                let sr = self.fetch() as usize;
                if pr < NUM_REGS && sr < NUM_REGS {
                    let target_pid = self.regs[pr];
                    let sig_num = self.regs[sr];
                    let mut delivered = false;
                    if let Some(signal) = Signal::from_u32(sig_num) {
                        for proc in &mut self.processes {
                            if proc.pid == target_pid && !proc.is_halted() {
                                let handler = proc.signal_handlers[signal as usize];
                                if handler == 0xFFFFFFFF {
                                    delivered = true;
                                } else if handler != 0 {
                                    proc.regs[0] = signal as u32;
                                    proc.regs[1] = self.current_pid;
                                    proc.pc = handler;
                                    delivered = true;
                                } else {
                                    match signal {
                                        Signal::Term => {
                                            proc.state = ProcessState::Zombie;
                                            proc.exit_code = 1;
                                            delivered = true;
                                        }
                                        Signal::Stop => {
                                            proc.state = ProcessState::Zombie;
                                            proc.exit_code = 2;
                                            delivered = true;
                                        }
                                        Signal::User1 | Signal::User2 => {
                                            delivered = true;
                                        }
                                    }
                                }
                                break;
                            }
                        }
                    }
                    self.regs[0] = if delivered { 0 } else { 0xFFFFFFFF };
                }
            }

            // SIGSET sig_reg, handler_reg -- register signal handler for current process.
            // sig_reg: signal number (0-3). handler_reg: address, 0=default, 0xFFFFFFFF=ignore.
            // r0 = 0 on success, 0xFFFFFFFF on error.
            0x71 => {
                let sr = self.fetch() as usize;
                let hr = self.fetch() as usize;
                if sr < NUM_REGS && hr < NUM_REGS {
                    let sig_num = self.regs[sr];
                    let handler = self.regs[hr];
                    if let Some(signal) = Signal::from_u32(sig_num) {
                        if self.current_pid > 0 {
                            for proc in &mut self.processes {
                                if proc.pid == self.current_pid {
                                    proc.signal_handlers[signal as usize] = handler;
                                    break;
                                }
                            }
                            self.regs[0] = 0;
                        } else {
                            self.regs[0] = 0xFFFFFFFF;
                        }
                    } else {
                        self.regs[0] = 0xFFFFFFFF;
                    }
                }
            }

            0x72 => {
                // HYPERVISOR: read config string from RAM at address in r0
                // Config format: "arch=riscv64 [kernel=file.img] [ram=256M] [mode=native|qemu]"
                // Validates arch= parameter is present. Kernel file existence checked at launch time.
                // Mode detection: mode=native -> HypervisorMode::Native, otherwise HypervisorMode::Qemu
                let addr_reg = self.fetch() as usize;
                if addr_reg < NUM_REGS {
                    let addr = self.regs[addr_reg] as usize;
                    let config = Self::read_string_static(&self.ram, addr);
                    match config {
                        Some(cfg) => {
                            // Validate arch= parameter is present
                            let has_arch = cfg
                                .split_whitespace()
                                .any(|t| t.to_lowercase().starts_with("arch=") && t.len() > 5);
                            if !has_arch {
                                self.regs[0] = 0xFFFFFFFD; // missing arch
                                return true;
                            }
                            // Detect mode from config string
                            let mode = cfg
                                .split_whitespace()
                                .find(|t| t.to_lowercase().starts_with("mode="))
                                .map(|t| {
                                    let val = t.split('=').nth(1).unwrap_or("").to_lowercase();
                                    if val == "native" {
                                        HypervisorMode::Native
                                    } else {
                                        HypervisorMode::Qemu
                                    }
                                })
                                .unwrap_or(HypervisorMode::Qemu);
                            self.hypervisor_config = cfg.to_string();
                            self.hypervisor_mode = mode;
                            self.hypervisor_active = true;
                            self.regs[0] = 0; // success
                        }
                        None => {
                            self.regs[0] = 0xFFFFFFFF; // error
                        }
                    }
                }
            }

            // ASMSELF (0x73) -- Self-assembly opcode
            // Reads the canvas buffer as text, runs it through the preprocessor
            // and assembler, writes bytecode to 0x1000.
            // Status: RAM[0xFFD] = bytecode word count (success) or 0xFFFFFFFF (error).
            0x73 => {
                // Canvas grid dimensions (must match main.rs constants)
                const CANVAS_COLS: usize = 32;
                const CANVAS_MAX_ROWS: usize = 128;
                const CANVAS_BYTECODE_ADDR: usize = 0x1000;
                const ASM_STATUS_PORT: usize = 0xFFD;

                // Convert canvas buffer to text string (same logic as F8 handler)
                let buffer_size = CANVAS_MAX_ROWS * CANVAS_COLS;
                let source: String = self.canvas_buffer
                    [..buffer_size.min(self.canvas_buffer.len())]
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

                // Collapse consecutive newlines (same as F8 handler)
                let source = source.replace("\n\n", "\n");

                // Run preprocessor then assembler
                let mut pp = crate::preprocessor::Preprocessor::new();
                let preprocessed = pp.preprocess(&source);

                match crate::assembler::assemble(&preprocessed, CANVAS_BYTECODE_ADDR) {
                    Ok(asm_result) => {
                        // Clear the bytecode region first
                        let end = (CANVAS_BYTECODE_ADDR + 4096).min(self.ram.len());
                        for addr in CANVAS_BYTECODE_ADDR..end {
                            self.ram[addr] = 0;
                        }
                        // Write assembled bytecode
                        for (i, &word) in asm_result.pixels.iter().enumerate() {
                            let addr = CANVAS_BYTECODE_ADDR + i;
                            if addr < self.ram.len() {
                                self.ram[addr] = word;
                            }
                        }
                        // Write success status: bytecode word count
                        if ASM_STATUS_PORT < self.ram.len() {
                            self.ram[ASM_STATUS_PORT] = asm_result.pixels.len() as u32;
                        }
                    }
                    Err(_e) => {
                        // Write error status
                        if ASM_STATUS_PORT < self.ram.len() {
                            self.ram[ASM_STATUS_PORT] = 0xFFFFFFFF;
                        }
                    }
                }
            }

            // RUNNEXT (0x74) -- Self-execution opcode
            // Sets PC to the canvas bytecode region (0x1000) and continues execution.
            // Combined with ASMSELF, a program can write new code, compile it, and run it.
            // Registers and stack are preserved -- the new program inherits all state.
            0x74 => {
                self.pc = 0x1000;
            }

            // FORMULA (0x75) -- Reactive canvas formula registration
            // Encoding: 0x75, target_idx, op_code, dep_count, dep0, dep1, ...
            // target_idx: canvas buffer index (0..4095) to attach the formula to
            // op_code: 0=ADD, 1=SUB, 2=MUL, 3=DIV, 4=AND, 5=OR, 6=XOR, 7=NOT,
            //          8=COPY, 9=MAX, 10=MIN, 11=MOD, 12=SHL, 13=SHR
            // dep_count: number of dependency indices (0..8)
            // dep0..depN: canvas buffer indices the formula reads from
            // Returns 1 in r0 on success, 0 on failure (cycle/limits exceeded)
            0x75 => {
                let target_idx = self.fetch() as usize;
                let op_code = self.fetch();
                let dep_count = self.fetch() as usize;
                let mut deps = Vec::with_capacity(dep_count.min(MAX_FORMULA_DEPS));
                for _ in 0..dep_count.min(MAX_FORMULA_DEPS) {
                    deps.push(self.fetch() as usize);
                }
                let op = match op_code {
                    0 => FormulaOp::Add,
                    1 => FormulaOp::Sub,
                    2 => FormulaOp::Mul,
                    3 => FormulaOp::Div,
                    4 => FormulaOp::And,
                    5 => FormulaOp::Or,
                    6 => FormulaOp::Xor,
                    7 => FormulaOp::Not,
                    8 => FormulaOp::Copy,
                    9 => FormulaOp::Max,
                    10 => FormulaOp::Min,
                    11 => FormulaOp::Mod,
                    12 => FormulaOp::Shl,
                    13 => FormulaOp::Shr,
                    _ => FormulaOp::Copy,
                };
                let ok = self.formula_register(target_idx, deps, op);
                self.regs[0] = if ok { 1 } else { 0 };
            }

            // FORMULACLEAR (0x76) -- Clear all formulas
            // Encoding: 0x76
            0x76 => {
                self.formula_clear_all();
            }

            // FORMULAREM (0x77) -- Remove formula from a canvas cell
            // Encoding: 0x77, target_idx
            0x77 => {
                let target_idx = self.fetch() as usize;
                self.formula_remove(target_idx);
            }

            // FMKDIR path_reg  (0x78) -- Create directory in inode filesystem
            // Encoding: 0x78, path_reg
            // path_reg points to null-terminated path string in RAM
            // Returns inode number in r0, or 0 on error
            0x78 => {
                let path_reg = self.fetch() as usize;
                if path_reg < NUM_REGS {
                    let path_addr = self.regs[path_reg];
                    let path_str = Self::read_string_static(&self.ram, path_addr as usize);
                    match path_str {
                        Some(path) => {
                            let ino = self.inode_fs.mkdir(&path);
                            self.regs[0] = ino;
                        }
                        None => {
                            self.regs[0] = 0;
                        }
                    }
                } else {
                    self.regs[0] = 0;
                }
            }

            // FSTAT ino_reg, buf_reg  (0x79) -- Get inode metadata into RAM buffer
            // Encoding: 0x79, ino_reg, buf_reg
            // buf_reg points to 6-word buffer: [ino, itype, size, ref_count, parent_ino, num_children]
            // Returns 1 in r0 on success, 0 on error
            0x79 => {
                let ino_reg = self.fetch() as usize;
                let buf_reg = self.fetch() as usize;
                if ino_reg < NUM_REGS && buf_reg < NUM_REGS {
                    let ino = self.regs[ino_reg];
                    let buf_addr = self.regs[buf_reg] as usize;
                    let buf_len =
                        crate::inode_fs::FSTAT_ENTRIES.min(self.ram.len().saturating_sub(buf_addr));
                    let mut buf = vec![0u32; buf_len];
                    if self.inode_fs.fstat(ino, &mut buf) {
                        for (i, &val) in buf.iter().enumerate() {
                            let addr = buf_addr + i;
                            if addr < self.ram.len() {
                                self.ram[addr] = val;
                            }
                        }
                        self.regs[0] = 1;
                    } else {
                        self.regs[0] = 0;
                    }
                } else {
                    self.regs[0] = 0;
                }
            }

            // FUNLINK path_reg  (0x7A) -- Remove file or empty directory from inode filesystem
            // Encoding: 0x7A, path_reg
            // path_reg points to null-terminated path string in RAM
            // Returns 1 in r0 on success, 0 on error
            0x7A => {
                let path_reg = self.fetch() as usize;
                if path_reg < NUM_REGS {
                    let path_addr = self.regs[path_reg];
                    let path_str = Self::read_string_static(&self.ram, path_addr as usize);
                    match path_str {
                        Some(path) => {
                            if self.inode_fs.unlink(&path) {
                                self.regs[0] = 1;
                            } else {
                                self.regs[0] = 0;
                            }
                        }
                        None => {
                            self.regs[0] = 0;
                        }
                    }
                } else {
                    self.regs[0] = 0;
                }
            }

            // SNAP_TRACE mode_reg  (0x7B) -- Toggle execution trace recording.
            // Encoding: 0x7B, mode_reg
            // mode_reg value: 0 = stop recording, 1 = start recording,
            //                 2 = snapshot-and-clear (stops recording, r0 = entries captured)
            // r0 = number of entries currently in buffer (after mode change)
            0x7B => {
                let mode_reg = self.fetch() as usize;
                let mode = if mode_reg < NUM_REGS { self.regs[mode_reg] } else { 0 };
                match mode {
                    0 => {
                        self.trace_recording = false;
                        self.regs[0] = self.trace_buffer.len() as u32;
                    }
                    1 => {
                        self.trace_recording = true;
                        self.regs[0] = self.trace_buffer.len() as u32;
                    }
                    2 => {
                        let count = self.trace_buffer.len();
                        self.trace_recording = false;
                        self.trace_buffer.clear();
                        self.regs[0] = count as u32;
                    }
                    _ => {
                        self.regs[0] = 0xFFFFFFFF; // invalid mode
                    }
                }
            }

            // REPLAY frame_idx_reg  (0x7C) -- Load a checkpoint screen onto the live display.
            // Encoding: 0x7C, frame_idx_reg
            // frame_idx: 0 = most recent checkpoint, 1 = second most recent, etc.
            // On success: r0 = frame_count (number of available checkpoints), frame_ready = true
            // On failure: r0 = 0xFFFFFFFF
            0x7C => {
                let idx_reg = self.fetch() as usize;
                let frame_idx = if idx_reg < NUM_REGS {
                    self.regs[idx_reg] as usize
                } else {
                    0
                };

                match self.frame_checkpoints.replay_frame(frame_idx) {
                    Some(screen) => {
                        self.screen.copy_from_slice(&screen);
                        self.frame_ready = true;
                        self.regs[0] = self.frame_checkpoints.len() as u32;
                    }
                    None => {
                        self.regs[0] = 0xFFFFFFFF;
                    }
                }
            }

            // FORK mode_reg  (0x7D) -- Timeline forking: save/restore full VM state.
            // Encoding: 0x7D, mode_reg
            // mode_reg value: 0 = save snapshot to next slot
            //                 1 = restore from snapshot slot (r1 = slot index)
            //                 2 = list saved snapshots (r0 = count)
            //                 3 = clear all snapshots
            // r0 = slot index on save, 0 on restore/clear, count on list, 0xFFFFFFFF on error
            0x7D => {
                let mode_reg = self.fetch() as usize;
                let mode = if mode_reg < NUM_REGS {
                    self.regs[mode_reg]
                } else {
                    0
                };
                match mode {
                    0 => {
                        // Save snapshot to next slot
                        if self.snapshots.len() < MAX_SNAPSHOTS {
                            let snap = self.snapshot();
                            let slot = self.snapshots.len();
                            self.snapshots.push(snap);
                            self.regs[0] = slot as u32; // return slot index
                        } else {
                            self.regs[0] = 0xFFFFFFFF; // too many snapshots
                        }
                    }
                    1 => {
                        // Restore from snapshot slot (slot index in r1)
                        let slot = self.regs[1] as usize;
                        if slot < self.snapshots.len() {
                            let snap = self.snapshots[slot].clone();
                            self.restore(&snap);
                            self.regs[0] = 0; // success
                        } else {
                            self.regs[0] = 0xFFFFFFFF; // invalid slot
                        }
                    }
                    2 => {
                        // List: return count of saved snapshots
                        self.regs[0] = self.snapshots.len() as u32;
                    }
                    3 => {
                        // Clear all snapshots
                        self.snapshots.clear();
                        self.regs[0] = 0;
                    }
                    _ => {
                        self.regs[0] = 0xFFFFFFFF; // invalid mode
                    }
                }
            }

            // Unknown opcode: halt
            _ => {
                self.halted = true;
                return false;
            }
        }
        true
    }
}
