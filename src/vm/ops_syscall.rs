use super::types::*;
use super::Vm;

impl Vm {
    /// Handle syscall and IPC opcodes (0x52-0x5F).
    /// Returns false if halted (RETK protection fault), true otherwise.
    #[allow(unreachable_patterns)]
    pub(super) fn step_syscall(&mut self, opcode: u32) -> bool {
        match opcode {
            // SYSCALL num  -- trap into kernel mode
            // Reads handler address from RAM[SYSCALL_TABLE + num]
            // Saves return PC on kernel_stack, switches to Kernel, jumps to handler
            0x52 => {
                let num = self.fetch() as usize;
                let table_idx = SYSCALL_TABLE + num;
                if table_idx < self.ram.len() {
                    let handler = self.ram[table_idx];
                    if handler != 0 {
                        // Save return address and current mode
                        self.kernel_stack.push((self.pc, self.mode));
                        self.mode = CpuMode::Kernel;
                        self.pc = handler;
                    } else {
                        // No handler registered: set r0 = 0xFFFFFFFF (error)
                        self.regs[0] = 0xFFFFFFFF;
                    }
                } else {
                    self.regs[0] = 0xFFFFFFFF;
                }
            }

            // RETK  -- return from kernel mode to user mode
            // Pops return PC and saved mode from kernel_stack
            0x53 => {
                if let Some((ret_pc, saved_mode)) = self.kernel_stack.pop() {
                    self.pc = ret_pc;
                    self.mode = saved_mode;
                } else {
                    // Empty kernel stack: halt (protection fault)
                    self.halted = true;
                    return false;
                }
            }

            // OPEN path_reg, mode_reg  -- open file, returns fd in r0
            // path_reg points to null-terminated string in RAM (one char per word)
            // mode: 0=read, 1=write, 2=read+write(append)
            0x54 => {
                let path_reg = self.fetch() as usize;
                let mode_reg = self.fetch() as usize;
                if path_reg < NUM_REGS && mode_reg < NUM_REGS {
                    let path_addr = self.regs[path_reg];
                    let mode = self.regs[mode_reg];
                    // Check if path matches a device name
                    let mut is_device = false;
                    let mut dev_fd = 0xFFFFFFFF;
                    let path_str = Self::read_string_static(&self.ram, path_addr as usize);
                    if let Some(path) = path_str {
                        for (i, &name) in DEVICE_NAMES.iter().enumerate() {
                            if path == name {
                                is_device = true;
                                dev_fd = DEVICE_FD_BASE + i as u32;
                                break;
                            }
                        }
                    }
                    if is_device {
                        self.regs[0] = dev_fd;
                    } else {
                        let pid = self.current_pid;
                        let fd = self.vfs.fopen(&self.ram, path_addr, mode, pid);
                        self.regs[0] = fd;
                    }
                } else {
                    self.regs[0] = 0xFFFFFFFF;
                }
            }

            // READ fd_reg, buf_addr_reg, len_reg  -- read from file into RAM
            // Returns bytes read in r0
            0x55 => {
                let fd_reg = self.fetch() as usize;
                let buf_reg = self.fetch() as usize;
                let len_reg = self.fetch() as usize;
                if fd_reg < NUM_REGS && buf_reg < NUM_REGS && len_reg < NUM_REGS {
                    let fd = self.regs[fd_reg];
                    // Check if this is a device fd (0xE000+idx)
                    let dev_idx_r = fd.wrapping_sub(DEVICE_FD_BASE) as usize;
                    if fd >= DEVICE_FD_BASE && dev_idx_r < DEVICE_COUNT {
                        let dev_idx = fd.wrapping_sub(DEVICE_FD_BASE) as usize;
                        let buf_addr = self.regs[buf_reg] as usize;
                        let len = self.regs[len_reg] as usize;
                        let mut count = 0usize;
                        match dev_idx {
                            1 => {
                                // /dev/keyboard -- read key from RAM[0xFFF]
                                if len > 0 && buf_addr < self.ram.len() {
                                    self.ram[buf_addr] = self.ram[0xFFF];
                                    self.ram[0xFFF] = 0; // clear port like IKEY
                                    count = 1;
                                }
                            }
                            3 => {
                                // /dev/net -- read from RAM[0xFFC]
                                if len > 0 && buf_addr < self.ram.len() {
                                    self.ram[buf_addr] = self.ram[0xFFC];
                                    count = 1;
                                }
                            }
                            _ => {} // other devices: read returns 0
                        }
                        self.regs[0] = count as u32;
                    }
                    // Check if this is a pipe read fd (0x8000+idx)
                    else if (0x8000..0xC000).contains(&fd) {
                        let pipe_idx = (fd & 0x0FFF) as usize;
                        let buf_addr = self.regs[buf_reg] as usize;
                        let len = self.regs[len_reg] as usize;
                        if pipe_idx < self.pipes.len() && self.pipes[pipe_idx].alive {
                            if self.pipes[pipe_idx].is_empty() {
                                // Blocking read: block this process and rewind PC
                                let pid = self.current_pid;
                                if pid > 0 {
                                    if let Some(proc) =
                                        self.processes.iter_mut().find(|p| p.pid == pid)
                                    {
                                        proc.state = ProcessState::Blocked;
                                        // Rewind PC past the READ opcode (4 words: opcode + 3 args)
                                        self.pc -= 4;
                                    }
                                }
                                self.regs[0] = 0; // 0 bytes read (will retry)
                            } else {
                                // Read available words from pipe into RAM
                                let mut count = 0usize;
                                for i in 0..len {
                                    if let Some(word) = self.pipes[pipe_idx].read_word() {
                                        let addr = buf_addr + i;
                                        if addr < self.ram.len() {
                                            self.ram[addr] = word;
                                            count += 1;
                                        }
                                    } else {
                                        break;
                                    }
                                }
                                self.regs[0] = count as u32;
                                // Unblock any process blocked on write to this pipe
                                // (writer may have been blocked if pipe was full)
                            }
                        } else {
                            self.regs[0] = 0xFFFFFFFF; // bad pipe fd
                        }
                    } else {
                        let buf_addr = self.regs[buf_reg];
                        let len = self.regs[len_reg];
                        let pid = self.current_pid;
                        let n = self.vfs.fread(&mut self.ram, fd, buf_addr, len, pid);
                        self.regs[0] = n;
                    }
                } else {
                    self.regs[0] = 0xFFFFFFFF;
                }
            }

            // WRITE fd_reg, buf_addr_reg, len_reg  -- write from RAM to file or pipe
            // Returns bytes written in r0
            0x56 => {
                let fd_reg = self.fetch() as usize;
                let buf_reg = self.fetch() as usize;
                let len_reg = self.fetch() as usize;
                if fd_reg < NUM_REGS && buf_reg < NUM_REGS && len_reg < NUM_REGS {
                    let fd = self.regs[fd_reg];
                    // Check if this is a device fd (0xE000+idx)
                    let dev_idx_w = fd.wrapping_sub(DEVICE_FD_BASE) as usize;
                    if fd >= DEVICE_FD_BASE && dev_idx_w < DEVICE_COUNT {
                        let buf_addr = self.regs[buf_reg] as usize;
                        let len = self.regs[len_reg] as usize;
                        match dev_idx_w {
                            0 => {
                                // /dev/screen -- write (x, y, color) triplets
                                let mut i = 0;
                                while i + 2 < len {
                                    let x_addr = buf_addr + i;
                                    let y_addr = buf_addr + i + 1;
                                    let c_addr = buf_addr + i + 2;
                                    if x_addr < self.ram.len()
                                        && y_addr < self.ram.len()
                                        && c_addr < self.ram.len()
                                    {
                                        let x = self.ram[x_addr] as usize;
                                        let y = self.ram[y_addr] as usize;
                                        let c = self.ram[c_addr];
                                        if x < 256 && y < 256 {
                                            self.screen[y * 256 + x] = c;
                                        }
                                    }
                                    i += 3;
                                }
                                self.regs[0] = i as u32;
                            }
                            2 => {
                                // /dev/audio -- write (freq, duration) pair
                                if len >= 2
                                    && buf_addr < self.ram.len()
                                    && buf_addr + 1 < self.ram.len()
                                {
                                    let freq = self.ram[buf_addr].clamp(20, 20000);
                                    let dur = self.ram[buf_addr + 1].clamp(1, 5000);
                                    self.beep = Some((freq, dur));
                                    self.regs[0] = 2;
                                } else {
                                    self.regs[0] = 0;
                                }
                            }
                            3 => {
                                // /dev/net -- write to RAM[0xFFC]
                                if len > 0 && buf_addr < self.ram.len() {
                                    self.ram[0xFFC] = self.ram[buf_addr];
                                    self.regs[0] = 1;
                                } else {
                                    self.regs[0] = 0;
                                }
                            }
                            _ => {
                                self.regs[0] = 0;
                            }
                        }
                    }
                    // Check if this is a pipe write fd (0xC000+idx)
                    else if (0xC000..DEVICE_FD_BASE).contains(&fd) {
                        let pipe_idx = (fd & 0x0FFF) as usize;
                        let buf_addr = self.regs[buf_reg] as usize;
                        let len = self.regs[len_reg] as usize;
                        if pipe_idx < self.pipes.len() && self.pipes[pipe_idx].alive {
                            let mut count = 0usize;
                            for i in 0..len {
                                let addr = buf_addr + i;
                                if addr >= self.ram.len() {
                                    break;
                                }
                                if self.pipes[pipe_idx].write_word(self.ram[addr]) {
                                    count += 1;
                                } else {
                                    break; // pipe full
                                }
                            }
                            self.regs[0] = count as u32;
                            // Unblock any process blocked on read from this pipe
                            for proc in &mut self.processes {
                                if proc.state == ProcessState::Blocked && !proc.is_halted() {
                                    // Check if this process is blocked reading from this pipe
                                    // (heuristic: unblock all blocked processes -- they'll
                                    // re-block if their pipe is still empty)
                                    proc.state = ProcessState::Ready;
                                }
                            }
                        } else {
                            self.regs[0] = 0xFFFFFFFF; // bad pipe fd or pipe closed
                        }
                    } else {
                        let buf_addr = self.regs[buf_reg];
                        let len = self.regs[len_reg];
                        let pid = self.current_pid;
                        let n = self.vfs.fwrite(&self.ram, fd, buf_addr, len, pid);
                        self.regs[0] = n;
                    }
                } else {
                    self.regs[0] = 0xFFFFFFFF;
                }
            }

            // CLOSE fd_reg  -- close file descriptor, returns 0 in r0 on success
            // Also handles pipe fds (0x8000 read, 0xC000 write)
            0x57 => {
                let fd_reg = self.fetch() as usize;
                if fd_reg < NUM_REGS {
                    let fd = self.regs[fd_reg];
                    let pid = self.current_pid;
                    // Check if this is a device fd (no-op close)
                    let dev_idx_c = fd.wrapping_sub(DEVICE_FD_BASE) as usize;
                    if fd >= DEVICE_FD_BASE && dev_idx_c < DEVICE_COUNT {
                        self.regs[0] = 0; // device close always succeeds
                    }
                    // Check if this is a pipe fd
                    else if (0x8000..0xC000).contains(&fd)
                        || (0xC000..DEVICE_FD_BASE).contains(&fd)
                    {
                        let pipe_idx = (fd & 0x0FFF) as usize;
                        if pipe_idx < self.pipes.len() {
                            // Mark pipe as dead (both read and write ends closed)
                            self.pipes[pipe_idx].alive = false;
                            self.regs[0] = 0;
                        } else {
                            self.regs[0] = 0xFFFFFFFF;
                        }
                    } else {
                        let result = self.vfs.fclose(fd, pid);
                        self.regs[0] = result;
                    }
                } else {
                    self.regs[0] = 0xFFFFFFFF;
                }
            }

            // SEEK fd_reg, offset_reg, whence_reg  -- seek in file
            // whence: 0=SET, 1=CUR, 2=END. Returns new position in r0
            0x58 => {
                let fd_reg = self.fetch() as usize;
                let offset_reg = self.fetch() as usize;
                let whence_reg = self.fetch() as usize;
                if fd_reg < NUM_REGS && offset_reg < NUM_REGS && whence_reg < NUM_REGS {
                    let fd = self.regs[fd_reg];
                    let offset = self.regs[offset_reg];
                    let whence = self.regs[whence_reg];
                    let pid = self.current_pid;
                    let pos = self.vfs.fseek(fd, offset, whence, pid);
                    self.regs[0] = pos;
                } else {
                    self.regs[0] = 0xFFFFFFFF;
                }
            }

            // LS buf_addr_reg  -- list directory entries into RAM buffer
            // Returns entry count in r0
            0x59 => {
                let buf_reg = self.fetch() as usize;
                if buf_reg < NUM_REGS {
                    let buf_addr = self.regs[buf_reg];
                    let count = self.vfs.fls(&mut self.ram, buf_addr);
                    self.regs[0] = count;
                } else {
                    self.regs[0] = 0xFFFFFFFF;
                }
            }

            // YIELD -- cooperative yield, give up remaining time slice
            0x5A => {
                self.yielded = true;
            }

            // SLEEP ticks_reg -- sleep for N scheduler ticks
            0x5B => {
                let tr = self.fetch() as usize;
                if tr < NUM_REGS {
                    self.sleep_frames = self.regs[tr];
                }
            }

            // SETPRIORITY priority_reg -- set current process priority (0-3)
            0x5C => {
                let pr = self.fetch() as usize;
                if pr < NUM_REGS {
                    self.new_priority = self.regs[pr].min(3) as u8;
                }
            }

            // PIPE rd_read, rd_write -- create a unidirectional pipe
            // r0 = read_fd (0x8000+idx) or 0xFFFFFFFF on error, r1 = write_fd (0xC000+idx)
            0x5D => {
                let rr = self.fetch() as usize;
                let rw = self.fetch() as usize;
                if rr < NUM_REGS && rw < NUM_REGS {
                    if self.pipes.len() < MAX_PIPES {
                        let pid = self.current_pid;
                        let idx = self.pipes.len() as u32;
                        self.pipes.push(Pipe::new(pid, pid));
                        self.regs[rr] = 0x8000 | idx;
                        self.regs[rw] = 0xC000 | idx;
                        self.regs[0] = 0; // success
                    } else {
                        self.regs[0] = 0xFFFFFFFF; // too many pipes
                    }
                } else {
                    self.regs[0] = 0xFFFFFFFF;
                }
            }

            // MSGSND pid_reg -- send r1..r4 as a 4-word message to target PID
            // r0 = 0 on success, 0xFFFFFFFF on error
            0x5E => {
                let pid_reg = self.fetch() as usize;
                if pid_reg < NUM_REGS {
                    let target_pid = self.regs[pid_reg];
                    let sender_pid = self.current_pid;
                    let data = [self.regs[1], self.regs[2], self.regs[3], self.regs[4]];
                    // Find target process and deliver message
                    let mut delivered = false;
                    for proc in &mut self.processes {
                        if proc.pid == target_pid && !proc.is_halted() {
                            if proc.msg_queue.len() < MAX_MESSAGES {
                                proc.msg_queue.push(Message::new(sender_pid, data));
                                delivered = true;
                                // If process is blocked waiting for a message, unblock it
                                if proc.state == ProcessState::Blocked {
                                    proc.state = ProcessState::Ready;
                                }
                            }
                            break;
                        }
                    }
                    if delivered {
                        self.regs[0] = 0;
                    } else {
                        self.regs[0] = 0xFFFFFFFF;
                    }
                } else {
                    self.regs[0] = 0xFFFFFFFF;
                }
            }

            // MSGRCV -- receive a message (blocks if none pending)
            // On success: r0 = sender PID, r1..r4 = message data
            // If no message: process blocks (scheduler will retry)
            0x5F => {
                let pid = self.current_pid;
                // Check if this is a child process
                if pid > 0 {
                    if let Some(proc) = self.processes.iter_mut().find(|p| p.pid == pid) {
                        if let Some(msg) = proc.msg_queue.first().cloned() {
                            proc.msg_queue.remove(0);
                            self.regs[0] = msg.sender;
                            self.regs[1] = msg.data[0];
                            self.regs[2] = msg.data[1];
                            self.regs[3] = msg.data[2];
                            self.regs[4] = msg.data[3];
                        } else {
                            // No message: block this process
                            proc.state = ProcessState::Blocked;
                            // Rewind PC so MSGRCV retries after unblock
                            self.pc -= 1;
                        }
                    }
                } else {
                    // Main process: check msg queue on VM (non-blocking for simplicity)
                    self.regs[0] = 0xFFFFFFFF; // main process has no msg queue in current design
                }
            }
            _ => {}
        }
        true
    }
}
