#[derive(Debug)]
pub struct Vm {
    pub ram: Vec<u32>,
    pub regs: [u32; NUM_REGS],
    pub pc: u32,
    pub screen: Vec<u32>,
    pub halted: bool,
    /// Set by FRAME opcode; cleared by the host after rendering
    pub frame_ready: bool,
    /// LCG state for RAND opcode
    pub rand_state: u32,
    /// Incremented each time FRAME fires; mirrored to RAM[TICKS_PORT]
    pub frame_count: u32,
    /// Set by BEEP opcode: (freq_hz, duration_ms). Consumed and cleared by host.
    pub beep: Option<(u32, u32)>,
    /// When true, log RAM accesses to access_log (off by default for performance)
    pub debug_mode: bool,
    /// Frame-scoped log of RAM accesses for the visual debugger
    pub access_log: Vec<MemAccess>,
    /// Secondary execution contexts spawned by SPATIAL_SPAWN
    pub processes: Vec<SpawnedProcess>,
    /// CPU privilege mode -- kernel can do anything, user is restricted
    pub mode: CpuMode,
    /// Kernel stack: saves (return_pc, saved_mode) on SYSCALL, restored by RETK
    pub kernel_stack: Vec<(u32, CpuMode)>,
    /// Bitmap of allocated physical pages (bit N = page N in use)
    pub allocated_pages: u64,
    /// Reference count per physical page for COW fork support.
    /// When a page is shared between processes, ref_count > 1.
    /// A write to a COW page triggers a copy (ref_count decremented on original).
    pub page_ref_count: [u32; NUM_RAM_PAGES],
    /// Bitmap of physical pages marked as copy-on-write.
    /// Bit N = 1 means physical page N is shared and should be copied on write.
    pub page_cow: u64,
    /// Current page directory for address translation (None = identity mapping)
    pub current_page_dir: Option<Vec<u32>>,
    /// VMA list for the currently executing process (used by page fault handler)
    pub current_vmas: Vec<Vma>,
    /// PID of last process that segfaulted
    pub segfault_pid: u32,
    /// True when a segfault occurred this step
    pub segfault: bool,
    /// Virtual filesystem for file I/O operations
    pub vfs: crate::vfs::Vfs,
    /// In-memory inode filesystem for directory tree and inode operations
    pub inode_fs: crate::inode_fs::InodeFs,
    /// PID of currently executing context (0 = main, 1+ = children)
    pub current_pid: u32,
    /// Monotonically increasing scheduler tick (incremented each step)
    pub sched_tick: u64,
    /// Base time slice length for priority-1 processes
    pub default_time_slice: u32,
    /// Per-step scheduler flag: process yielded voluntarily
    pub yielded: bool,
    /// Per-step scheduler value: sleep for this many sched_ticks
    pub sleep_frames: u32,
    /// Per-step scheduler value: new priority requested by SETPRIORITY
    pub new_priority: u8,
    /// System-wide pipe table (Phase 27: IPC)
    pub pipes: Vec<Pipe>,
    /// Mirror of the canvas grid (Phase 45: Pixel Driving Pixels)
    pub canvas_buffer: Vec<u32>,
    /// Per-step IPC flag: set by PIPE opcode to signal pipe creation
    pub pipe_created: bool,
    /// Per-step IPC value: sender PID for MSGSND
    pub msg_sender: u32,
    /// Per-step IPC value: message data for MSGSND
    pub msg_data: [u32; MSG_WORDS],
    /// Per-step IPC flag: MSGRCV requested
    pub msg_recv_requested: bool,
    /// Environment variables for shell support (Phase 29).
    /// Shared across all processes; SETENV by any process is visible to all.
    pub env_vars: std::collections::HashMap<String, String>,
    /// Boot state: true when VM has been booted (init process started)
    pub booted: bool,
    /// Shutdown requested by SHUTDOWN opcode (Phase 30). Host checks this.
    pub shutdown_requested: bool,
    /// Per-step transient: exit code from EXIT opcode.
    pub step_exit_code: Option<u32>,
    /// Per-step transient: zombie flag from EXIT opcode.
    pub step_zombie: bool,
    /// Hypervisor active flag (Phase 33: QEMU Bridge).
    /// Set by HYPERVISOR opcode, checked by host to pipe I/O.
    pub hypervisor_active: bool,
    /// Hypervisor config string read from RAM (Phase 33).
    pub hypervisor_config: String,
    /// Hypervisor mode: Qemu (Phase 33) or Native RISC-V (Phase 37).
    /// Detected from config string's mode= parameter.
    pub hypervisor_mode: HypervisorMode,
    /// Key ring buffer: host pushes keystrokes, IKEY reads them in order.
    /// Supports up to 16 queued keys so rapid typing doesn't drop inputs.
    pub key_buffer: Vec<u32>,
    /// Key buffer head (next read position)
    pub key_buffer_head: usize,
    /// Key buffer tail (next write position)
    pub key_buffer_tail: usize,
    /// Active formulas on canvas cells (Phase 50: Reactive Canvas).
    pub formulas: Vec<Formula>,
    /// Reverse dependency index: dep_idx -> list of formula indices in self.formulas.
    /// Used to quickly find which formulas need recalculation when a cell changes.
    pub formula_dep_index: Vec<Vec<usize>>,
    /// When true, every instruction execution is recorded to trace_buffer.
    /// Off by default for zero-overhead forward execution.
    pub trace_recording: bool,
    /// Execution trace ring buffer (Phase 38a: Time-Travel Debugger).
    pub trace_buffer: TraceBuffer,
    /// Frame checkpoint ring buffer (Phase 38b: Frame Checkpointing).
    /// Snapshots the full screen at every FRAME opcode when trace_recording is on.
    pub frame_checkpoints: FrameCheckBuffer,
    /// Saved VM snapshots for timeline forking (Phase 38d).
    /// Max 16 snapshots; each captures full RAM + screen + registers.
    pub snapshots: Vec<VmSnapshot>,
}

impl Default for Vm {
    fn default() -> Self {
        Self::new()
    }
}

impl Vm {
    /// Create a new VM with zeroed RAM, registers, and screen buffer.
    pub fn new() -> Self {
        Vm {
            ram: vec![0; RAM_SIZE],
            regs: [0; NUM_REGS],
            pc: 0,
            screen: vec![0; SCREEN_SIZE],
            halted: false,
            frame_ready: false,
            rand_state: 0xDEADBEEF,
            frame_count: 0,
            beep: None,
            debug_mode: false,
            access_log: Vec::with_capacity(4096),
            processes: Vec::new(),
            mode: CpuMode::Kernel,
            kernel_stack: Vec::new(),
            allocated_pages: 0b11, // pages 0-1 used by main process
            page_ref_count: {
                let mut rc = [0u32; NUM_RAM_PAGES];
                rc[0] = 1; // page 0 used by main process
                rc[1] = 1; // page 1 used by main process
                rc
            },
            page_cow: 0,
            current_page_dir: None,
            current_vmas: Vec::new(),
            segfault_pid: 0,
            segfault: false,
            vfs: crate::vfs::Vfs::new(),
            inode_fs: crate::inode_fs::InodeFs::new(),
            current_pid: 0,
            sched_tick: 0,
            default_time_slice: DEFAULT_TIME_SLICE,
            yielded: false,
            sleep_frames: 0,
            new_priority: 0,
            pipes: Vec::new(),
            canvas_buffer: vec![0; CANVAS_RAM_SIZE],
            pipe_created: false,
            msg_sender: 0,
            msg_data: [0; MSG_WORDS],
            msg_recv_requested: false,
            env_vars: std::collections::HashMap::new(),
            booted: false,
            shutdown_requested: false,
            step_exit_code: None,
            step_zombie: false,
            hypervisor_active: false,
            hypervisor_config: String::new(),
            hypervisor_mode: HypervisorMode::default(),
            key_buffer: vec![0; 16],
            key_buffer_head: 0,
            key_buffer_tail: 0,
            formulas: Vec::new(),
            formula_dep_index: vec![Vec::new(); CANVAS_RAM_SIZE],
            trace_recording: false,
            trace_buffer: TraceBuffer::new(DEFAULT_TRACE_CAPACITY),
            frame_checkpoints: FrameCheckBuffer::new(DEFAULT_FRAME_CHECK_CAPACITY),
            snapshots: Vec::new(),
        }
    }

    /// Push a keystroke into the ring buffer. Called by host on key events.
    /// Returns false if the buffer is full (key dropped).
    pub fn push_key(&mut self, key: u32) -> bool {
        let next_tail = (self.key_buffer_tail + 1) % self.key_buffer.len();
        if next_tail == self.key_buffer_head {
            return false; // buffer full
        }
        self.key_buffer[self.key_buffer_tail] = key;
        self.key_buffer_tail = next_tail;
        // Also write to legacy RAM[0xFFF] port for backward compatibility
        self.ram[0xFFF] = key;
        true
    }

    /// Reset the VM to initial state (zeroed RAM, registers, screen, halted=false).
    #[allow(dead_code)]
    pub fn reset(&mut self) {
        for r in self.ram.iter_mut() {
            *r = 0;
        }
        for s in self.screen.iter_mut() {
            *s = 0;
        }
        self.regs = [0; NUM_REGS];
        self.pc = 0;
        self.halted = false;
        self.frame_ready = false;
        self.rand_state = 0xDEADBEEF;
        self.frame_count = 0;
        self.beep = None;
        self.access_log.clear();
        self.processes.clear();
        self.mode = CpuMode::Kernel;
        self.kernel_stack.clear();
        self.allocated_pages = 0b11;
        self.page_ref_count = {
            let mut rc = [0u32; NUM_RAM_PAGES];
            rc[0] = 1;
            rc[1] = 1;
            rc
        };
        self.page_cow = 0;
        self.current_page_dir = None;
        self.current_vmas = Vec::new();
        self.segfault_pid = 0;
        self.segfault = false;
        self.pipes.clear();
        self.pipe_created = false;
        self.msg_sender = 0;
        self.msg_data = [0; MSG_WORDS];
        self.msg_recv_requested = false;
        self.env_vars.clear();
        self.booted = false;
        self.shutdown_requested = false;
        self.hypervisor_active = false;
        self.hypervisor_config.clear();
        self.hypervisor_mode = HypervisorMode::default();
        self.formulas.clear();
        for dep_list in self.formula_dep_index.iter_mut() {
            dep_list.clear();
        }
        self.trace_recording = false;
        self.trace_buffer.clear();
        self.frame_checkpoints.clear();
        self.snapshots.clear();
    }

    /// Internal helper to log a memory access with a safety cap.
    fn log_access(&mut self, addr: usize, kind: MemAccessKind) {
        if self.debug_mode && self.access_log.len() < 4096 {
            self.access_log.push(MemAccess { addr, kind });
        }
    }

    /// Take a full snapshot of VM state for timeline forking (Phase 38d).
    /// Returns the VmSnapshot capturing RAM, screen, registers, PC, and config.
    /// The snapshot can be restored later with `restore()`.
    pub fn snapshot(&self) -> VmSnapshot {
        VmSnapshot {
            ram: self.ram.clone(),
            screen: self.screen.clone(),
            regs: self.regs,
            pc: self.pc,
            mode: self.mode,
            halted: self.halted,
            frame_count: self.frame_count,
            rand_state: self.rand_state,
            current_pid: self.current_pid,
            step_number: self.trace_buffer.step_counter(),
        }
    }

    /// Restore VM state from a snapshot (Phase 38d).
    /// Overwrites RAM, screen, registers, PC, and config with the snapshot values.
    /// Does NOT restore child processes, pipes, VFS, or other system state --
    /// only the execution state of the current context.
    pub fn restore(&mut self, snap: &VmSnapshot) {
        self.ram.copy_from_slice(&snap.ram);
        self.screen.copy_from_slice(&snap.screen);
        self.regs = snap.regs;
        self.pc = snap.pc;
        self.mode = snap.mode;
        self.halted = snap.halted;
        self.frame_count = snap.frame_count;
        self.rand_state = snap.rand_state;
        self.current_pid = snap.current_pid;
    }
}
mod types;
pub use types::*;

// Execution trace ring buffer (Phase 38a)
mod trace;
pub use trace::*;

// Opcode handler submodules
mod ops_extended;
mod ops_graphics;
mod ops_memory;
mod ops_syscall;

mod formula;
mod io;
mod memory;

impl Vm {
    /// Execute one instruction. Returns false if halted.
    pub fn step(&mut self) -> bool {
        if self.halted || self.pc as usize >= self.ram.len() {
            self.halted = true;
            return false;
        }

        // Log the instruction fetch for the visual debugger
        let pc_addr = self.pc as usize;
        self.log_access(pc_addr, MemAccessKind::Read);

        let opcode = self.fetch();

        // Execution trace: record (pc, regs, opcode) if recording is enabled.
        // Zero overhead when off (single bool check).
        if self.trace_recording {
            self.trace_buffer.push(pc_addr as u32, &self.regs, opcode);
        }

        match opcode {
            // HALT
            0x00 => {
                self.halted = true;
                return false;
            }

            // NOP
            0x01 => {}

            // FRAME -- signal host to display current screen; execution continues
            0x02 => {
                self.frame_count = self.frame_count.wrapping_add(1);
                self.ram[0xFFE] = self.frame_count;
                self.frame_ready = true;
                self.access_log.clear(); // Reset for next frame
                                         // Phase 38b: snapshot screen if trace recording is on
                if self.trace_recording {
                    let step = self.trace_buffer.step_counter();
                    self.frame_checkpoints
                        .push(step, self.frame_count, &self.screen);
                }
                return true; // keep running (host checks frame_ready to pace rendering)
            }

            // BEEP freq_reg, dur_reg  -- play a sine-wave tone (freq Hz, dur ms)
            0x03 => {
                let fr = self.fetch() as usize;
                let dr = self.fetch() as usize;
                if fr < NUM_REGS && dr < NUM_REGS {
                    let freq = self.regs[fr].clamp(20, 20000);
                    let dur = self.regs[dr].clamp(1, 5000);
                    self.beep = Some((freq, dur));
                }
            }

            // MEMCPY dst_reg, src_reg, len_reg -- copy len words from [src] to [dst]
            0x04 => {
                let dr = self.fetch() as usize;
                let sr = self.fetch() as usize;
                let lr = self.fetch() as usize;
                if dr < NUM_REGS && sr < NUM_REGS && lr < NUM_REGS {
                    let mut dst = self.regs[dr] as usize;
                    let mut src = self.regs[sr] as usize;
                    let len = self.regs[lr] as usize;
                    // Clamp to RAM bounds to prevent runaway copies
                    let max_copy = self.ram.len().min(len);
                    for _ in 0..max_copy {
                        if dst < self.ram.len() && src < self.ram.len() {
                            self.ram[dst] = self.ram[src];
                        }
                        dst += 1;
                        src += 1;
                    }
                }
            }
            0x10..=0x1F => {
                if !self.step_memory(opcode) {
                    return false;
                }
            }

            // ADD rd, rs  -- rd = rd + rs
            0x20 => {
                let rd = self.fetch() as usize;
                let rs = self.fetch() as usize;
                if rd < NUM_REGS && rs < NUM_REGS {
                    self.regs[rd] = self.regs[rd].wrapping_add(self.regs[rs]);
                }
            }

            // SUB rd, rs
            0x21 => {
                let rd = self.fetch() as usize;
                let rs = self.fetch() as usize;
                if rd < NUM_REGS && rs < NUM_REGS {
                    self.regs[rd] = self.regs[rd].wrapping_sub(self.regs[rs]);
                }
            }

            // MUL rd, rs
            0x22 => {
                let rd = self.fetch() as usize;
                let rs = self.fetch() as usize;
                if rd < NUM_REGS && rs < NUM_REGS {
                    self.regs[rd] = self.regs[rd].wrapping_mul(self.regs[rs]);
                }
            }

            // DIV rd, rs
            0x23 => {
                let rd = self.fetch() as usize;
                let rs = self.fetch() as usize;
                if rd < NUM_REGS && rs < NUM_REGS && self.regs[rs] != 0 {
                    self.regs[rd] /= self.regs[rs];
                }
            }

            // AND rd, rs
            0x24 => {
                let rd = self.fetch() as usize;
                let rs = self.fetch() as usize;
                if rd < NUM_REGS && rs < NUM_REGS {
                    self.regs[rd] &= self.regs[rs];
                }
            }

            // OR rd, rs
            0x25 => {
                let rd = self.fetch() as usize;
                let rs = self.fetch() as usize;
                if rd < NUM_REGS && rs < NUM_REGS {
                    self.regs[rd] |= self.regs[rs];
                }
            }

            // XOR rd, rs
            0x26 => {
                let rd = self.fetch() as usize;
                let rs = self.fetch() as usize;
                if rd < NUM_REGS && rs < NUM_REGS {
                    self.regs[rd] ^= self.regs[rs];
                }
            }

            // SHL rd, rs  -- rd = rd << rs (logical shift left)
            0x27 => {
                let rd = self.fetch() as usize;
                let rs = self.fetch() as usize;
                if rd < NUM_REGS && rs < NUM_REGS {
                    let shift = self.regs[rs] % 32;
                    self.regs[rd] = self.regs[rd].wrapping_shl(shift);
                }
            }

            // SHR rd, rs  -- rd = rd >> rs (logical shift right)
            0x28 => {
                let rd = self.fetch() as usize;
                let rs = self.fetch() as usize;
                if rd < NUM_REGS && rs < NUM_REGS {
                    let shift = self.regs[rs] % 32;
                    self.regs[rd] = self.regs[rd].wrapping_shr(shift);
                }
            }

            // MOD rd, rs  -- rd = rd % rs
            0x29 => {
                let rd = self.fetch() as usize;
                let rs = self.fetch() as usize;
                if rd < NUM_REGS && rs < NUM_REGS && self.regs[rs] != 0 {
                    self.regs[rd] %= self.regs[rs];
                }
            }

            // NEG rd  -- rd = -rd (two's complement)
            0x2A => {
                let rd = self.fetch() as usize;
                if rd < NUM_REGS {
                    self.regs[rd] = self.regs[rd].wrapping_neg();
                }
            }

            // SAR rd, rs  -- rd = rd >> rs (arithmetic shift right)
            0x2B => {
                let rd = self.fetch() as usize;
                let rs = self.fetch() as usize;
                if rd < NUM_REGS && rs < NUM_REGS {
                    let shift = self.regs[rs] % 32;
                    self.regs[rd] = ((self.regs[rd] as i32) >> shift) as u32;
                }
            }

            // JMP addr
            0x30 => {
                let addr = self.fetch();
                self.pc = addr;
                return true; // don't increment PC again
            }

            // JZ reg, addr  -- jump if reg == 0
            0x31 => {
                let reg = self.fetch() as usize;
                let addr = self.fetch();
                if reg < NUM_REGS && self.regs[reg] == 0 {
                    self.pc = addr;
                    return true;
                }
            }

            // JNZ reg, addr  -- jump if reg != 0
            0x32 => {
                let reg = self.fetch() as usize;
                let addr = self.fetch();
                if reg < NUM_REGS && self.regs[reg] != 0 {
                    self.pc = addr;
                    return true;
                }
            }

            // CALL addr
            0x33 => {
                let addr = self.fetch();
                // Push return address to r31 (link register)
                if NUM_REGS > 0 {
                    self.regs[31] = self.pc;
                }
                self.pc = addr;
                return true;
            }

            // RET  -- jump to r31
            0x34 => {
                self.pc = self.regs[31];
                return true;
            }

            // BLT reg, addr  -- branch if CMP result < 0 (r0 == 0xFFFFFFFF)
            0x35 => {
                let _reg = self.fetch() as usize;
                let addr = self.fetch();
                if self.regs[0] == 0xFFFFFFFF {
                    self.pc = addr;
                    return true;
                }
            }

            // BGE reg, addr  -- branch if CMP result >= 0 (r0 != 0xFFFFFFFF)
            0x36 => {
                let _reg = self.fetch() as usize;
                let addr = self.fetch();
                if self.regs[0] != 0xFFFFFFFF {
                    self.pc = addr;
                    return true;
                }
            }

            // PUSH reg  -- push onto stack (r30=SP, page-translated)
            0x60 => {
                let reg = self.fetch() as usize;
                if reg < NUM_REGS {
                    let sp = self.regs[30];
                    if sp > 0 {
                        let new_sp = sp - 1;
                        match self.translate_va_or_fault(new_sp) {
                            Some(addr) if addr < self.ram.len() => {
                                self.ram[addr] = self.regs[reg];
                                self.regs[30] = new_sp;
                            }
                            None => {
                                self.trigger_segfault();
                                return false;
                            }
                            _ => {}
                        }
                    }
                }
            }

            // POP reg  -- pop from stack (r30=SP, page-translated)
            0x61 => {
                let reg = self.fetch() as usize;
                if reg < NUM_REGS {
                    let sp = self.regs[30];
                    match self.translate_va_or_fault(sp) {
                        Some(addr) if addr < self.ram.len() => {
                            self.regs[reg] = self.ram[addr];
                            self.regs[30] = sp + 1;
                        }
                        None => {
                            self.trigger_segfault();
                            return false;
                        }
                        _ => {}
                    }
                }
            }
            0x40..=0x51 => {
                if !self.step_graphics(opcode) {
                    return false;
                }
            }
            0x52..=0x5F => {
                if !self.step_syscall(opcode) {
                    return false;
                }
            }
            0x62..=0x7D => {
                if !self.step_extended(opcode) {
                    return false;
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

mod boot;
mod disasm;
mod scheduler;

#[cfg(test)]
mod tests;
