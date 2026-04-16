mod types;
pub use types::*;


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
    }

    /// Internal helper to log a memory access with a safety cap.
    fn log_access(&mut self, addr: usize, kind: MemAccessKind) {
        if self.debug_mode && self.access_log.len() < 4096 {
            self.access_log.push(MemAccess { addr, kind });
        }
    }
}

mod formula;

impl Vm {

    /// Read a null-terminated string from RAM (one char per u32 word).
    fn read_string_static(ram: &[u32], addr: usize) -> Option<String> {
        let mut s = String::new();
        let mut a = addr;
        while a < ram.len() {
            let ch = (ram[a] & 0xFF) as u8;
            if ch == 0 {
                return Some(s);
            }
            s.push(ch as char);
            a += 1;
        }
        if s.is_empty() {
            None
        } else {
            Some(s)
        }
    }

    /// Read a null-terminated string from self.ram with max length cap.
    fn read_ram_string(&self, addr: usize, max_len: usize) -> Option<String> {
        let mut s = String::new();
        let mut a = addr;
        while a < self.ram.len() && s.len() < max_len {
            let ch = (self.ram[a] & 0xFF) as u8;
            if ch == 0 {
                break;
            }
            s.push(ch as char);
            a += 1;
        }
        if s.is_empty() {
            None
        } else {
            Some(s)
        }
    }
}

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

            // LDI reg, imm  -- load immediate
            0x10 => {
                let reg = self.fetch() as usize;
                let imm = self.fetch();
                if reg < NUM_REGS {
                    self.regs[reg] = imm;
                }
            }

            // LOAD reg, addr_reg  -- load from RAM (page-translated)
            0x11 => {
                let reg = self.fetch() as usize;
                let addr_reg = self.fetch() as usize;
                if reg < NUM_REGS && addr_reg < NUM_REGS {
                    let vaddr = self.regs[addr_reg];
                    match self.translate_va_or_fault(vaddr) {
                        Some(addr) => {
                            // Phase 46: Intercept screen buffer range
                            if (SCREEN_RAM_BASE..SCREEN_RAM_BASE + SCREEN_SIZE).contains(&addr) {
                                self.regs[reg] = self.screen[addr - SCREEN_RAM_BASE];
                                self.log_access(addr, MemAccessKind::Read);
                            } else if addr < self.ram.len() {
                                // Phase 45: Intercept canvas RAM range
                                if (CANVAS_RAM_BASE..CANVAS_RAM_BASE + CANVAS_RAM_SIZE).contains(&addr) {
                                    self.regs[reg] = self.canvas_buffer[addr - CANVAS_RAM_BASE];
                                } else {
                                    self.regs[reg] = self.ram[addr];
                                }
                                self.log_access(addr, MemAccessKind::Read);
                            } else {
                                self.trigger_segfault();
                                return false;
                            }
                        }
                        None => { self.trigger_segfault(); return false; }
                    }
                }
            }

            // STORE addr_reg, reg  -- store to RAM (page-translated, COW-aware)
            0x12 => {
                let addr_reg = self.fetch() as usize;
                let reg = self.fetch() as usize;
                if addr_reg < NUM_REGS && reg < NUM_REGS {
                    let vaddr = self.regs[addr_reg];
                    // Check COW before writing: if the target physical page is shared,
                    // copy it to a private page first
                    self.resolve_cow_if_needed(vaddr);
                    match self.translate_va_or_fault(vaddr) {
                        Some(addr) => {
                            // Phase 46: Intercept screen buffer range
                            if (SCREEN_RAM_BASE..SCREEN_RAM_BASE + SCREEN_SIZE).contains(&addr) {
                                self.screen[addr - SCREEN_RAM_BASE] = self.regs[reg];
                                self.log_access(addr, MemAccessKind::Write);
                            } else if addr < self.ram.len() {
                                if self.mode == CpuMode::User && addr >= 0xFF00 {
                                    self.trigger_segfault();
                                    return false;
                                }
                                // Phase 45: Intercept canvas RAM range
                                if (CANVAS_RAM_BASE..CANVAS_RAM_BASE + CANVAS_RAM_SIZE).contains(&addr) {
                                    let cidx = addr - CANVAS_RAM_BASE;
                                    self.canvas_buffer[cidx] = self.regs[reg];
                                    // Phase 50: Trigger formula recalculation
                                    self.formula_recalc(cidx);
                                } else {
                                    self.ram[addr] = self.regs[reg];
                                }
                                self.log_access(addr, MemAccessKind::Write);
                            } else {
                                self.trigger_segfault();
                                return false;
                            }
                        }
                        None => { self.trigger_segfault(); return false; }
                    }
                }
            }

            // TEXTI x, y, "string" -- render inline text (no RAM setup needed)
            // Encoding: 0x13, x_imm, y_imm, char_count, char1, char2, ...
            0x13 => {
                let x = self.fetch() as usize;
                let y = self.fetch() as usize;
                let count = self.fetch() as usize;
                let mut sx = x;
                let mut sy = y;
                let fg = 0xFFFFFF; // white text
                for _ in 0..count {
                    let ch = self.fetch();
                    if ch == 0 { continue; }
                    let byte = (ch & 0xFF) as u8;
                    if byte == b'\n' {
                        sx = x;
                        sy += 10;
                        continue;
                    }
                    self.draw_char(byte, sx, sy, fg);
                    sx += 6;
                    if sx > 250 {
                        sx = x;
                        sy += 8;
                    }
                }
            }

            // STRO addr_reg, "string" -- store inline string at address in register
            // Encoding: 0x14, addr_reg, char_count, char1, char2, ...
            0x14 => {
                let ar = self.fetch() as usize;
                let count = self.fetch() as usize;
                if ar < NUM_REGS {
                    let mut vaddr = self.regs[ar];
                    for _ in 0..count {
                        let ch = self.fetch();
                        if let Some(addr) = self.translate_va_or_fault(vaddr) {
                                if (SCREEN_RAM_BASE..SCREEN_RAM_BASE + SCREEN_SIZE).contains(&addr) {
                                    self.screen[addr - SCREEN_RAM_BASE] = ch;
                                    self.log_access(addr, MemAccessKind::Write);
                                } else if addr < self.ram.len() {
                                if (CANVAS_RAM_BASE..CANVAS_RAM_BASE + CANVAS_RAM_SIZE).contains(&addr) {
                                    let cidx = addr - CANVAS_RAM_BASE;
                                    self.canvas_buffer[cidx] = ch;
                                    self.formula_recalc(cidx);
                                } else {
                                    self.ram[addr] = ch;
                                }
                                self.log_access(addr, MemAccessKind::Write);
                            }
                        }
                    vaddr = vaddr.wrapping_add(1);
                }
                // null-terminate if possible
                if let Some(addr) = self.translate_va_or_fault(vaddr) {
                if (SCREEN_RAM_BASE..SCREEN_RAM_BASE + SCREEN_SIZE).contains(&addr) {
                    self.screen[addr - SCREEN_RAM_BASE] = 0;
                } else if addr < self.ram.len() {
                    if (CANVAS_RAM_BASE..CANVAS_RAM_BASE + CANVAS_RAM_SIZE).contains(&addr) {
                        let cidx = addr - CANVAS_RAM_BASE;
                        self.canvas_buffer[cidx] = 0;
                        self.formula_recalc(cidx);
                        } else {
                            self.ram[addr] = 0;
                        }
                    }
                }
                }
            }

            // CMPI reg, imm -- compare register against immediate, sets r0
            0x15 => {
                let rd = self.fetch() as usize;
                let imm = self.fetch();
                if rd < NUM_REGS {
                    let a = self.regs[rd] as i32;
                    let b = imm as i32;
                    self.regs[0] = if a < b { 0xFFFFFFFF } else if a > b { 1 } else { 0 };
                }
            }

            // LOADS reg, offset -- load from SP + offset (stack-relative)
            0x16 => {
                let rd = self.fetch() as usize;
                let offset = self.fetch() as i32 as usize;
                if rd < NUM_REGS {
                    let sp = self.regs[30] as usize;
                    let vaddr = if offset < 0x80000000 { sp.wrapping_add(offset) } else { sp.wrapping_sub(0x100000000_usize - offset) };
                    match self.translate_va_or_fault(vaddr as u32) {
                        Some(addr) => {
                            if (SCREEN_RAM_BASE..SCREEN_RAM_BASE + SCREEN_SIZE).contains(&addr) {
                                self.regs[rd] = self.screen[addr - SCREEN_RAM_BASE];
                                self.log_access(addr, MemAccessKind::Read);
                            } else if addr < self.ram.len() {
                                if (CANVAS_RAM_BASE..CANVAS_RAM_BASE + CANVAS_RAM_SIZE).contains(&addr) {
                                    self.regs[rd] = self.canvas_buffer[addr - CANVAS_RAM_BASE];
                                } else {
                                    self.regs[rd] = self.ram[addr];
                                }
                                self.log_access(addr, MemAccessKind::Read);
                            } else {
                                self.trigger_segfault(); return false;
                            }
                        }
                        None => { self.trigger_segfault(); return false; }
                    }
                }
            }

            // STORES offset, reg -- store to SP + offset (stack-relative, COW-aware)
            0x17 => {
                let offset = self.fetch() as i32;
                let rs = self.fetch() as usize;
                if rs < NUM_REGS {
                    let sp = self.regs[30] as i32;
                    let vaddr = sp.wrapping_add(offset) as u32;
                    self.resolve_cow_if_needed(vaddr);
                    match self.translate_va_or_fault(vaddr) {
                        Some(addr) => {
                            if (SCREEN_RAM_BASE..SCREEN_RAM_BASE + SCREEN_SIZE).contains(&addr) {
                                self.screen[addr - SCREEN_RAM_BASE] = self.regs[rs];
                                self.log_access(addr, MemAccessKind::Write);
                            } else if addr < self.ram.len() {
                                if self.mode == CpuMode::User && addr >= 0xFF00 {
                                    self.trigger_segfault();
                                    return false;
                                }
                                if (CANVAS_RAM_BASE..CANVAS_RAM_BASE + CANVAS_RAM_SIZE).contains(&addr) {
                                    let cidx = addr - CANVAS_RAM_BASE;
                                    self.canvas_buffer[cidx] = self.regs[rs];
                                    self.formula_recalc(cidx);
                                } else {
                                    self.ram[addr] = self.regs[rs];
                                }
                                self.log_access(addr, MemAccessKind::Write);
                            } else {
                                self.trigger_segfault();
                                return false;
                            }
                        }
                        None => { self.trigger_segfault(); return false; }
                    }
                }
            }

            // SHLI reg, imm -- shift left by immediate
            0x18 => {
                let rd = self.fetch() as usize;
                let imm = self.fetch();
                if rd < NUM_REGS {
                    self.regs[rd] <<= (imm % 32) as usize;
                }
            }

            // SHRI reg, imm -- logical shift right by immediate
            0x19 => {
                let rd = self.fetch() as usize;
                let imm = self.fetch();
                if rd < NUM_REGS {
                    self.regs[rd] >>= (imm % 32) as usize;
                }
            }

            // SARI reg, imm -- arithmetic shift right by immediate
            0x1A => {
                let rd = self.fetch() as usize;
                let imm = self.fetch();
                if rd < NUM_REGS {
                    let v = self.regs[rd] as i32;
                    self.regs[rd] = (v >> ((imm % 32) as usize)) as u32;
                }
            }

            // ADDI reg, imm -- add immediate to register
            0x1B => {
                let rd = self.fetch() as usize;
                let imm = self.fetch();
                if rd < NUM_REGS {
                    self.regs[rd] = self.regs[rd].wrapping_add(imm);
                }
            }

            // SUBI reg, imm -- subtract immediate from register
            0x1C => {
                let rd = self.fetch() as usize;
                let imm = self.fetch();
                if rd < NUM_REGS {
                    self.regs[rd] = self.regs[rd].wrapping_sub(imm);
                }
            }

            // ANDI reg, imm -- bitwise AND with immediate
            0x1D => {
                let rd = self.fetch() as usize;
                let imm = self.fetch();
                if rd < NUM_REGS {
                    self.regs[rd] &= imm;
                }
            }

            // ORI reg, imm -- bitwise OR with immediate
            0x1E => {
                let rd = self.fetch() as usize;
                let imm = self.fetch();
                if rd < NUM_REGS {
                    self.regs[rd] |= imm;
                }
            }

            // XORI reg, imm -- bitwise XOR with immediate
            0x1F => {
                let rd = self.fetch() as usize;
                let imm = self.fetch();
                if rd < NUM_REGS {
                    self.regs[rd] ^= imm;
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
                            None => { self.trigger_segfault(); return false; }
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
                        None => { self.trigger_segfault(); return false; }
                        _ => {}
                    }
                }
            }

            // PSET x_reg, y_reg, color_reg  -- set pixel on screen
            0x40 => {
                let xr = self.fetch() as usize;
                let yr = self.fetch() as usize;
                let cr = self.fetch() as usize;
                if xr < NUM_REGS && yr < NUM_REGS && cr < NUM_REGS {
                    let x = self.regs[xr] as usize;
                    let y = self.regs[yr] as usize;
                    let color = self.regs[cr];
                    if x < 256 && y < 256 {
                        self.screen[y * 256 + x] = color;
                    }
                }
            }

            // PSETI x, y, color  -- set pixel with immediate values
            0x41 => {
                let x = self.fetch() as usize;
                let y = self.fetch() as usize;
                let color = self.fetch();
                if x < 256 && y < 256 {
                    self.screen[y * 256 + x] = color;
                }
            }

            // FILL color_reg  -- fill entire screen
            0x42 => {
                let cr = self.fetch() as usize;
                if cr < NUM_REGS {
                    let color = self.regs[cr];
                    for pixel in self.screen.iter_mut() {
                        *pixel = color;
                    }
                }
            }

            // RECTF x_reg, y_reg, w_reg, h_reg, color_reg  -- filled rectangle
            0x43 => {
                let xr = self.fetch() as usize;
                let yr = self.fetch() as usize;
                let wr = self.fetch() as usize;
                let hr = self.fetch() as usize;
                let cr = self.fetch() as usize;
                if xr < NUM_REGS && yr < NUM_REGS && wr < NUM_REGS
                    && hr < NUM_REGS && cr < NUM_REGS
                {
                    let x0 = self.regs[xr] as usize;
                    let y0 = self.regs[yr] as usize;
                    let w = self.regs[wr] as usize;
                    let h = self.regs[hr] as usize;
                    let color = self.regs[cr];
                    for dy in 0..h {
                        for dx in 0..w {
                            let px = x0 + dx;
                            let py = y0 + dy;
                            if px < 256 && py < 256 {
                                self.screen[py * 256 + px] = color;
                            }
                        }
                    }
                }
            }

            // TEXT x_reg, y_reg, addr_reg  -- render text from RAM to screen
            0x44 => {
                let xr = self.fetch() as usize;
                let yr = self.fetch() as usize;
                let ar = self.fetch() as usize;
                if xr < NUM_REGS && yr < NUM_REGS && ar < NUM_REGS {
                    let mut sx = self.regs[xr] as usize;
                    let mut sy = self.regs[yr] as usize;
                    let mut addr = self.regs[ar] as usize;
                    let fg = 0xFFFFFF; // white text
                    loop {
                        if addr >= self.ram.len() { break; }
                        let ch = (self.ram[addr] & 0xFF) as u8;
                        if ch == 0 { break; }
                        if ch == b'\n' {
                            sx = self.regs[xr] as usize;
                            sy += 10;
                            addr += 1;
                            continue;
                        }
                        // Render 5x7 glyph at (sx, sy) -- inline for now
                        self.draw_char(ch, sx, sy, fg);
                        sx += 6; // 5 wide + 1 gap
                        if sx > 250 {
                            sx = self.regs[xr] as usize;
                            sy += 8;
                        }
                        addr += 1;
                    }
                }
            }

            // CMP rd, rs  -- set r0 = comparison result (-1, 0, 1)
            0x50 => {
                let rd = self.fetch() as usize;
                let rs = self.fetch() as usize;
                if rd < NUM_REGS && rs < NUM_REGS {
                    let a = self.regs[rd] as i32;
                    let b = self.regs[rs] as i32;
                    self.regs[0] = if a < b { 0xFFFFFFFF } else if a > b { 1 } else { 0 };
                }
            }

            // MOV rd, rs -- rd = rs (register copy)
            0x51 => {
                let rd = self.fetch() as usize % NUM_REGS;
                let rs = self.fetch() as usize % NUM_REGS;
                self.regs[rd] = self.regs[rs];
            }

            // SPRITE x_reg, y_reg, addr_reg, w_reg, h_reg -- blit NxM pixels from RAM to screen
            // Color 0 in RAM means transparent (skip pixel)
            0x4A => {
                let xr = self.fetch() as usize;
                let yr = self.fetch() as usize;
                let ar = self.fetch() as usize;
                let wr = self.fetch() as usize;
                let hr = self.fetch() as usize;
                if xr < NUM_REGS && yr < NUM_REGS && ar < NUM_REGS
                    && wr < NUM_REGS && hr < NUM_REGS
                {
                    let sx = self.regs[xr] as usize;
                    let sy = self.regs[yr] as usize;
                    let mut addr = self.regs[ar] as usize;
                    let w = self.regs[wr] as usize;
                    let h = self.regs[hr] as usize;
                    for dy in 0..h {
                        for dx in 0..w {
                            if addr >= self.ram.len() { break; }
                            let color = self.ram[addr];
                            addr += 1;
                            if color == 0 { continue; } // transparent
                            let px = sx + dx;
                            let py = sy + dy;
                            if px < 256 && py < 256 {
                                self.screen[py * 256 + px] = color;
                            }
                        }
                    }
                }
            }

            // RAND rd  -- rd = next pseudo-random u32 (LCG: state = state*1664525 + 1013904223)
            0x49 => {
                let rd = self.fetch() as usize;
                if rd < NUM_REGS {
                    self.rand_state = self.rand_state
                        .wrapping_mul(1_664_525)
                        .wrapping_add(1_013_904_223);
                    self.regs[rd] = self.rand_state;
                }
            }

            // IKEY reg  -- read keyboard from ring buffer (or legacy RAM[0xFFF] port)
            0x48 => {
                let rd = self.fetch() as usize;
                // Blocked in User mode (hardware port access requires syscall)
                if self.mode == CpuMode::User {
                    self.halted = true;
                    return false;
                }
                if rd < NUM_REGS {
                    // Try ring buffer first
                    if self.key_buffer_head != self.key_buffer_tail {
                        self.regs[rd] = self.key_buffer[self.key_buffer_head];
                        self.key_buffer[self.key_buffer_head] = 0;
                        self.key_buffer_head = (self.key_buffer_head + 1) % self.key_buffer.len();
                    } else {
                        // Fallback to legacy single-key port
                        self.regs[rd] = self.ram[0xFFF];
                    }
                    self.ram[0xFFF] = 0;
                }
            }

            // LINE x0r, y0r, x1r, y1r, cr  -- Bresenham line
            0x45 => {
                let x0r = self.fetch() as usize;
                let y0r = self.fetch() as usize;
                let x1r = self.fetch() as usize;
                let y1r = self.fetch() as usize;
                let cr  = self.fetch() as usize;
                if x0r < NUM_REGS && y0r < NUM_REGS && x1r < NUM_REGS
                    && y1r < NUM_REGS && cr < NUM_REGS
                {
                    let color = self.regs[cr];
                    let mut x0 = self.regs[x0r] as i32;
                    let mut y0 = self.regs[y0r] as i32;
                    let x1 = self.regs[x1r] as i32;
                    let y1 = self.regs[y1r] as i32;
                    let dx = (x1 - x0).abs();
                    let dy = -(y1 - y0).abs();
                    let sx: i32 = if x0 < x1 { 1 } else { -1 };
                    let sy: i32 = if y0 < y1 { 1 } else { -1 };
                    let mut err = dx + dy;
                    loop {
                        if (0..256).contains(&x0) && (0..256).contains(&y0) {
                            self.screen[y0 as usize * 256 + x0 as usize] = color;
                        }
                        if x0 == x1 && y0 == y1 { break; }
                        let e2 = 2 * err;
                        if e2 >= dy { err += dy; x0 += sx; }
                        if e2 <= dx { err += dx; y0 += sy; }
                    }
                }
            }

            // CIRCLE xr, yr, rr, cr  -- midpoint circle
            0x46 => {
                let xr = self.fetch() as usize;
                let yr = self.fetch() as usize;
                let rr = self.fetch() as usize;
                let cr = self.fetch() as usize;
                if xr < NUM_REGS && yr < NUM_REGS && rr < NUM_REGS && cr < NUM_REGS {
                    let cx = self.regs[xr] as i32;
                    let cy = self.regs[yr] as i32;
                    let radius = self.regs[rr] as i32;
                    let color = self.regs[cr];
                    let mut x = radius;
                    let mut y = 0i32;
                    let mut err = 1 - radius;
                    while x >= y {
                        let pts: [(i32, i32); 8] = [
                            (cx + x, cy + y), (cx - x, cy + y),
                            (cx + x, cy - y), (cx - x, cy - y),
                            (cx + y, cy + x), (cx - y, cy + x),
                            (cx + y, cy - x), (cx - y, cy - x),
                        ];
                        for (px, py) in pts {
                            if (0..256).contains(&px) && (0..256).contains(&py) {
                                self.screen[py as usize * 256 + px as usize] = color;
                            }
                        }
                        y += 1;
                        if err < 0 {
                            err += 2 * y + 1;
                        } else {
                            x -= 1;
                            err += 2 * (y - x) + 1;
                        }
                    }
                }
            }

            // SCROLL nr  -- scroll screen up by regs[nr] pixels (wraps 0 in at bottom)
            0x47 => {
                let nr = self.fetch() as usize;
                if nr < NUM_REGS {
                    let n = (self.regs[nr] as usize).min(256);
                    if n > 0 {
                        self.screen.copy_within(n * 256.., 0);
                        for v in self.screen[(256 - n) * 256..].iter_mut() {
                            *v = 0;
                        }
                    }
                }
            }

            // ASM src_reg, dest_reg -- assemble source text from RAM, write bytecode to RAM
            // Source: null-terminated ASCII string at ram[regs[src_reg]]
            // Dest: bytecode written starting at ram[regs[dest_reg]]
            // Result: ram[0xFFD] = bytecode word count (success) or 0xFFFFFFFF (error)
            0x4B => {
                let sr = self.fetch() as usize;
                let dr = self.fetch() as usize;
                if sr < NUM_REGS && dr < NUM_REGS {
                    let src_addr = self.regs[sr] as usize;
                    let dest_addr = self.regs[dr] as usize;
                    // Read null-terminated ASCII string from RAM
                    let mut chars = Vec::new();
                    let mut a = src_addr;
                    while a < self.ram.len() {
                        let byte = (self.ram[a] & 0xFF) as u8;
                        if byte == 0 { break; }
                        chars.push(byte as char);
                        a += 1;
                    }
                    let source: String = chars.into_iter().collect();
                    // Call the assembler (base_addr = dest_addr for correct label resolution)
                    match crate::assembler::assemble(&source, dest_addr) {
                        Ok(result) => {
                            for (i, &word) in result.pixels.iter().enumerate() {
                                let idx = dest_addr + i;
                                if idx < self.ram.len() {
                                    self.ram[idx] = word;
                                }
                            }
                            self.ram[0xFFD] = result.pixels.len() as u32;
                        }
                        Err(_) => {
                            self.ram[0xFFD] = 0xFFFFFFFF;
                        }
                    }
                }
            }

            // TILEMAP xr, yr, mr, tr, gwr, ghr, twr, thr -- grid blit
            0x4C => {
                let xr = self.fetch() as usize;
                let yr = self.fetch() as usize;
                let mr = self.fetch() as usize;
                let tr = self.fetch() as usize;
                let gwr = self.fetch() as usize;
                let ghr = self.fetch() as usize;
                let twr = self.fetch() as usize;
                let thr = self.fetch() as usize;
                if xr < NUM_REGS && yr < NUM_REGS && mr < NUM_REGS && tr < NUM_REGS 
                   && gwr < NUM_REGS && ghr < NUM_REGS && twr < NUM_REGS && thr < NUM_REGS {
                    let x0 = self.regs[xr] as i32;
                    let y0 = self.regs[yr] as i32;
                    let map_base = self.regs[mr] as usize;
                    let tiles_base = self.regs[tr] as usize;
                    let gw = self.regs[gwr] as usize;
                    let gh = self.regs[ghr] as usize;
                    let tw = self.regs[twr] as usize;
                    let th = self.regs[thr] as usize;
                    
                    if tw > 0 && th > 0 {
                        for row in 0..gh {
                            for col in 0..gw {
                                let map_idx = row * gw + col;
                                let ram_map_addr = map_base + map_idx;
                                if ram_map_addr >= self.ram.len() { continue; }
                                
                                self.log_access(ram_map_addr, MemAccessKind::Read);
                                let tile_idx = self.ram[ram_map_addr] as usize;
                                if tile_idx == 0 { continue; } // skip tile 0 (empty)
                                
                                // Tile 1 is at offset 0, tile 2 at (tw*th), etc.
                                let tile_data_offset = (tile_idx - 1) * (tw * th);
                                
                                for ty in 0..th {
                                    for tx in 0..tw {
                                        let pixel_idx = tile_data_offset + ty * tw + tx;
                                        let ram_pixel_addr = tiles_base + pixel_idx;
                                        if ram_pixel_addr >= self.ram.len() { continue; }
                                        
                                        self.log_access(ram_pixel_addr, MemAccessKind::Read);
                                        let color = self.ram[ram_pixel_addr];
                                        if color == 0 { continue; } // transparency
                                        
                                        let px = x0 + (col * tw) as i32 + tx as i32;
                                        let py = y0 + (row * th) as i32 + ty as i32;
                                        
                                        if (0..256).contains(&px) && (0..256).contains(&py) {
                                            self.screen[(py as usize) * 256 + (px as usize)] = color;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // SPAWN addr_reg  -- create child with isolated address space
            // Returns PID (1-based) in RAM[0xFFA], or 0xFFFFFFFF on error
            // Uses copy-on-write: shares parent's physical pages, copies on write.
            //
            // Mapping strategy:
            //  - If start_addr is in pages 0-2: identity-map pages 0-2 as COW.
            //    Virtual addr X == physical addr X, so .org label addresses resolve
            //    correctly for JMP/CALL.  Child PC = start_addr.
            //  - If start_addr is in page 3+: sequential mapping (legacy mode).
            //    vpage N -> physical page (start_page + N).  Only works for
            //    sequential code (no JMP to .org addresses).  Child PC = page_offset.
            0x4D => {
                let ar = self.fetch() as usize;
                if ar < NUM_REGS {
                    let active_count = self.processes.iter().filter(|p| !p.is_halted()).count();
                    if active_count >= MAX_PROCESSES {
                        self.ram[0xFFA] = 0xFFFFFFFF;
                    } else {
                        let start_addr = self.regs[ar];
                        let start_page = (start_addr as usize) / PAGE_SIZE;
                        let page_offset = start_addr % (PAGE_SIZE as u32);
                        let mut pd = vec![PAGE_UNMAPPED; NUM_PAGES];

                        // Determine child PC based on mapping strategy
                        let child_pc: u32;
                        let identity_map = start_page < 3;

                        if identity_map {
                            // Identity-map pages 0-2: virtual addr N == physical addr N
                            for (phys_page, pd_entry) in pd.iter_mut().enumerate().take(3) {
                                if phys_page >= NUM_RAM_PAGES { break; }
                                *pd_entry = phys_page as u32;
                                if self.page_ref_count[phys_page] == 0 {
                                    self.page_ref_count[phys_page] = 1;
                                }
                                self.page_ref_count[phys_page] += 1;
                                self.page_cow |= 1u64 << phys_page;
                            }
                            child_pc = start_addr;
                        } else {
                            // Sequential mapping: vpage N -> phys page (start_page + N)
                            for (vpage, pd_entry) in pd.iter_mut().enumerate().take(PROCESS_PAGES) {
                                let parent_phys = start_page + vpage;
                                if parent_phys >= NUM_RAM_PAGES { break; }
                                if vpage == 3 || parent_phys == 3 {
                                    *pd_entry = 3;
                                    self.page_ref_count[3] += 1;
                                    continue;
                                }
                                *pd_entry = parent_phys as u32;
                                self.page_ref_count[parent_phys] += 1;
                                self.page_cow |= 1u64 << parent_phys;
                            }
                            child_pc = page_offset;
                        }

                        // Page 3 (0xC00-0xFFF): shared region, identity-mapped, NOT COW
                        if !identity_map {
                            // Already handled in loop above for sequential mode
                            // but ensure it's set
                        }
                        // For identity_map mode, page 3 needs explicit setup since
                        // the loop only covers pages 0-2
                        if identity_map {
                            pd[3] = 3;
                            if self.page_ref_count[3] == 0 {
                                self.page_ref_count[3] = 1;
                            }
                            self.page_ref_count[3] += 1;
                        }

                        // Page 63 (hardware ports / syscall table) - always identity-mapped
                        pd[63] = 63;

                        let pid = (self.processes.len() + 1) as u32;
                        self.processes.push(SpawnedProcess {
                            pc: child_pc,
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
                        self.ram[0xFFA] = pid;
                    }
                }
            }

            // KILL pid_reg  -- halt child, free its pages
            // Returns 1 in RAM[0xFFA] on success, 0 if not found
            0x4E => {
                let pr = self.fetch() as usize;
                if pr < NUM_REGS {
                    let target_pid = self.regs[pr];
                    let mut found = false;
                    let mut free_pd: Option<Vec<u32>> = None;
                    for proc in &mut self.processes {
                        if proc.pid == target_pid {
                            free_pd = proc.page_dir.take();
                            proc.state = ProcessState::Zombie;
                            found = true;
                            break;
                        }
                    }
                    if let Some(ref pd) = free_pd {
                        self.free_page_dir(pd);
                    }
                    self.ram[0xFFA] = if found { 1 } else { 0 };
                }
            }

            // PEEK rx, ry, rd  -- read screen pixel at (rx,ry) into rd
            // Out-of-bounds coordinates return 0
            0x4F => {
                let rx = self.fetch() as usize % NUM_REGS;
                let ry = self.fetch() as usize % NUM_REGS;
                let rd = self.fetch() as usize % NUM_REGS;
                let x = self.regs[rx] as usize;
                let y = self.regs[ry] as usize;
                if x < 256 && y < 256 {
                    self.regs[rd] = self.screen[y * 256 + x];
                } else {
                    self.regs[rd] = 0;
                }
            }

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
                    else if (0x8000..0xC000).contains(&fd) || (0xC000..DEVICE_FD_BASE).contains(&fd) {
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
                    let data = [
                        self.regs[1], self.regs[2],
                        self.regs[3], self.regs[4],
                    ];
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
                            0 => { // /dev/screen
                                match cmd {
                                    0 => self.regs[0] = 256, // width
                                    1 => self.regs[0] = 256, // height
                                    _ => self.regs[0] = 0xFFFFFFFF,
                                }
                            }
                            1 => { // /dev/keyboard
                                match cmd {
                                    0 => self.regs[0] = self.ram[0xFF8], // get echo mode
                                    1 => { self.ram[0xFF8] = arg; self.regs[0] = 0; }
                                    _ => self.regs[0] = 0xFFFFFFFF,
                                }
                            }
                            2 => { // /dev/audio
                                match cmd {
                                    0 => self.regs[0] = self.ram[0xFF7], // get volume
                                    1 => { self.ram[0xFF7] = arg.min(100); self.regs[0] = 0; }
                                    _ => self.regs[0] = 0xFFFFFFFF,
                                }
                            }
                            3 => { // /dev/net
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
                                    let active_count = self.processes.iter().filter(|p| !p.is_halted()).count();
                                    if active_count >= MAX_PROCESSES {
                                        self.regs[0] = 0xFFFFFFFF;
                                        self.ram[0xFFA] = 0xFFFFFFFF;
                                    } else {
                                        let page_dir = self.create_process_page_dir();
                                        match page_dir {
                                            Some(pd) => {
                                                let phys_base = (pd[0] as usize) * PAGE_SIZE;
                                                // Copy assembled bytecode into new process's physical memory
                                                for (i, &word) in asm_result.pixels.iter().enumerate() {
                                                    let addr = phys_base + i;
                                                    if addr >= self.ram.len() { break; }
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
                        if (self.ram[a] & 0xFF) == 0 { break; }
                        len += 1;
                        a += 1;
                    }
                    if len > 0 {
                        let n = self.vfs.fwrite(&self.ram, fd, str_addr as u32, len as u32, self.current_pid);
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
                                    let active_count = self.processes.iter().filter(|p| !p.is_halted()).count();
                                    if active_count >= MAX_PROCESSES {
                                        self.regs[0] = 0xFFFFFFFF;
                                    } else {
                                        let page_dir = self.create_process_page_dir();
                                        match page_dir {
                                            Some(pd) => {
                                                let phys_base = (pd[0] as usize) * PAGE_SIZE;
                                                for (i, &word) in asm_result.pixels.iter().enumerate() {
                                                    let addr = phys_base + i;
                                                    if addr >= self.ram.len() { break; }
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
                                                    self.vfs.dup_fd(stdin_fd, 0, child_pid, self.current_pid);
                                                }
                                                if stdout_fd != 0xFFFFFFFF {
                                                    self.vfs.dup_fd(stdout_fd, 1, child_pid, self.current_pid);
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
                    let cwd = self.env_vars.get("CWD").cloned().unwrap_or_else(|| "/".to_string());
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
                let source: String = self.canvas_buffer[..buffer_size.min(self.canvas_buffer.len())]
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
                    let buf_len = crate::inode_fs::FSTAT_ENTRIES.min(self.ram.len().saturating_sub(buf_addr));
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

            // Unknown opcode: halt
            _ => {
                self.halted = true;
                return false;
            }
        }
        true
    }
}

mod scheduler;

impl Vm {

    /// Boot the OS: load init.asm as PID 1, create boot.cfg if missing.
    /// Returns Ok(pid) on success, Err(msg) on failure.
    /// After booting, the VM is in kernel mode with the init process running
    /// as the first child process. The host should call step_all_processes().
    pub fn boot(&mut self) -> Result<u32, String> {
        if self.booted {
            return Err("already booted".into());
        }

        // Ensure boot.cfg exists in the VFS
        self.ensure_boot_config();

        // Assemble and load init.asm as PID 1
        let init_path = std::path::Path::new("programs/init.asm");
        let source = match std::fs::read_to_string(init_path) {
            Ok(s) => s,
            Err(e) => return Err(format!("cannot read init.asm: {}", e)),
        };

        let asm_result = match crate::assembler::assemble(&source, 0) {
            Ok(r) => r,
            Err(e) => return Err(format!("init.asm assembly error: {}", e)),
        };

        let page_dir = match self.create_process_page_dir() {
            Some(pd) => pd,
            None => return Err("no memory for init process".into()),
        };

        let phys_base = (page_dir[0] as usize) * PAGE_SIZE;
        for (i, &word) in asm_result.pixels.iter().enumerate() {
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
            page_dir: Some(page_dir),
            segfaulted: false,
            priority: 2, // init gets higher priority than normal processes
            slice_remaining: 0,
            sleep_until: 0,
            yielded: false,
            kernel_stack: Vec::new(),
            msg_queue: Vec::new(),
            exit_code: 0,
            parent_pid: 0, // init has no parent
            pending_signals: Vec::new(),
            signal_handlers: [0; 4],
            vmas: Process::default_vmas_for_process(),
            brk_pos: PAGE_SIZE as u32,
        });

        // Set default environment
        self.env_vars
            .insert("SHELL".into(), "shell".into());
        self.env_vars.insert("HOME".into(), "/".into());
        self.env_vars.insert("CWD".into(), "/".into());
        self.env_vars.insert("USER".into(), "root".into());

        self.booted = true;
        Ok(pid)
    }

    /// Create default boot.cfg in the VFS if it doesn't exist.
    /// Format: one directive per line, "key=value".
    /// Keys: init, shell, services (comma-separated program names).
    fn ensure_boot_config(&mut self) {
        // Check if boot.cfg exists by trying to open it
        let boot_cfg_path = self.vfs.base_dir.join("boot.cfg");
        if !boot_cfg_path.exists() {
            let default_cfg = "init=init\nshell=shell\nservices=\n";
            let _ = std::fs::write(&boot_cfg_path, default_cfg);
        }
    }

    /// Read a configuration value from boot.cfg in the VFS.
    /// Returns the value for the given key, or None if not found.
    #[allow(dead_code)]
    pub fn read_boot_config(&self, key: &str) -> Option<String> {
        let boot_cfg_path = self.vfs.base_dir.join("boot.cfg");
        let content = std::fs::read_to_string(&boot_cfg_path).ok()?;
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((k, v)) = line.split_once('=') {
                if k.trim() == key {
                    return Some(v.trim().to_string());
                }
            }
        }
        None
    }

    /// Disassemble one instruction starting at `addr` in RAM.
    /// Returns (mnemonic_string, instruction_length_in_words).
    /// Does not mutate VM state.
    pub fn disassemble_at(&self, addr: u32) -> (String, usize) {
        let a = addr as usize;
        if a >= self.ram.len() {
            return ("???".to_string(), 1);
        }
        let op = self.ram[a];
        let ram = |i: usize| -> u32 {
            if i < self.ram.len() { self.ram[i] } else { 0 }
        };
        let reg = |v: u32| -> String { format!("r{}", v) };
        match op {
            0x00 => ("HALT".into(), 1),
            0x01 => ("NOP".into(), 1),
            0x02 => ("FRAME".into(), 1),
            0x03 => {
                let fr = ram(a + 1);
                let dr = ram(a + 2);
                (format!("BEEP {}, {}", reg(fr), reg(dr)), 3)
            }
            0x04 => {
                let dr = ram(a + 1);
                let sr = ram(a + 2);
                let lr = ram(a + 3);
                (format!("MEMCPY {}, {}, {}", reg(dr), reg(sr), reg(lr)), 4)
            }
            0x10 => {
                let r = ram(a + 1);
                let imm = ram(a + 2);
                (format!("LDI {}, 0x{:X}", reg(r), imm), 3)
            }
            0x11 => {
                let r = ram(a + 1);
                let ar = ram(a + 2);
                (format!("LOAD {}, [{}]", reg(r), reg(ar)), 3)
            }
            0x12 => {
                let ar = ram(a + 1);
                let r = ram(a + 2);
                (format!("STORE [{}], {}", reg(ar), reg(r)), 3)
            }
            0x13 => {
                let x = ram(a+1); let y = ram(a+2); let count = ram(a+3) as usize;
                (format!("TEXTI {}, {}, \"{}\"", x, y, (4..4+count.min(32)).map(|i| (ram(a+i + 3) & 0xFF) as u8 as char).collect::<String>()), 4 + count)
            }
            0x14 => {
                let ar = ram(a+1); let count = ram(a+2) as usize;
                (format!("STRO {}, \"{}\"", reg(ar), (3..3+count.min(32)).map(|i| (ram(a+i + 2) & 0xFF) as u8 as char).collect::<String>()), 3 + count)
            }
            0x15 => { let rd = ram(a+1); let imm = ram(a+2); (format!("CMPI {}, {}", reg(rd), imm), 3) }
            0x16 => { let rd = ram(a+1); let off = ram(a+2); (format!("LOADS {}, {}", reg(rd), off as i32), 3) }
            0x17 => { let off = ram(a+1); let rs = ram(a+2); (format!("STORES {}, {}", off as i32, reg(rs)), 3) }
            0x18 => { let rd = ram(a+1); let imm = ram(a+2); (format!("SHLI {}, {}", reg(rd), imm), 3) }
            0x19 => { let rd = ram(a+1); let imm = ram(a+2); (format!("SHRI {}, {}", reg(rd), imm), 3) }
            0x1A => { let rd = ram(a+1); let imm = ram(a+2); (format!("SARI {}, {}", reg(rd), imm), 3) }
            0x1B => { let rd = ram(a+1); let imm = ram(a+2); (format!("ADDI {}, {}", reg(rd), imm), 3) }
            0x1C => { let rd = ram(a+1); let imm = ram(a+2); (format!("SUBI {}, {}", reg(rd), imm), 3) }
            0x1D => { let rd = ram(a+1); let imm = ram(a+2); (format!("ANDI {}, {}", reg(rd), imm), 3) }
            0x1E => { let rd = ram(a+1); let imm = ram(a+2); (format!("ORI {}, {}", reg(rd), imm), 3) }
            0x1F => { let rd = ram(a+1); let imm = ram(a+2); (format!("XORI {}, {}", reg(rd), imm), 3) }
            0x20 => { let rd = ram(a+1); let rs = ram(a+2); (format!("ADD {}, {}", reg(rd), reg(rs)), 3) }
            0x21 => { let rd = ram(a+1); let rs = ram(a+2); (format!("SUB {}, {}", reg(rd), reg(rs)), 3) }
            0x22 => { let rd = ram(a+1); let rs = ram(a+2); (format!("MUL {}, {}", reg(rd), reg(rs)), 3) }
            0x23 => { let rd = ram(a+1); let rs = ram(a+2); (format!("DIV {}, {}", reg(rd), reg(rs)), 3) }
            0x24 => { let rd = ram(a+1); let rs = ram(a+2); (format!("AND {}, {}", reg(rd), reg(rs)), 3) }
            0x25 => { let rd = ram(a+1); let rs = ram(a+2); (format!("OR {}, {}", reg(rd), reg(rs)), 3) }
            0x26 => { let rd = ram(a+1); let rs = ram(a+2); (format!("XOR {}, {}", reg(rd), reg(rs)), 3) }
            0x27 => { let rd = ram(a+1); let rs = ram(a+2); (format!("SHL {}, {}", reg(rd), reg(rs)), 3) }
            0x28 => { let rd = ram(a+1); let rs = ram(a+2); (format!("SHR {}, {}", reg(rd), reg(rs)), 3) }
            0x29 => { let rd = ram(a+1); let rs = ram(a+2); (format!("MOD {}, {}", reg(rd), reg(rs)), 3) }
            0x2A => { let rd = ram(a+1); (format!("NEG {}", reg(rd)), 2) }
            0x2B => { let rd = ram(a+1); let rs = ram(a+2); (format!("SAR {}, {}", reg(rd), reg(rs)), 3) }
            0x30 => { let addr2 = ram(a+1); (format!("JMP 0x{:04X}", addr2), 2) }
            0x31 => { let r = ram(a+1); let addr2 = ram(a+2); (format!("JZ {}, 0x{:04X}", reg(r), addr2), 3) }
            0x32 => { let r = ram(a+1); let addr2 = ram(a+2); (format!("JNZ {}, 0x{:04X}", reg(r), addr2), 3) }
            0x33 => { let addr2 = ram(a+1); (format!("CALL 0x{:04X}", addr2), 2) }
            0x34 => ("RET".into(), 1),
            0x35 => { let r = ram(a+1); let addr2 = ram(a+2); (format!("BLT {}, 0x{:04X}", reg(r), addr2), 3) }
            0x36 => { let r = ram(a+1); let addr2 = ram(a+2); (format!("BGE {}, 0x{:04X}", reg(r), addr2), 3) }
            0x40 => { let xr = ram(a+1); let yr = ram(a+2); let cr = ram(a+3); (format!("PSET {}, {}, {}", reg(xr), reg(yr), reg(cr)), 4) }
            0x41 => { let x = ram(a+1); let y = ram(a+2); let c = ram(a+3); (format!("PSETI {}, {}, 0x{:X}", x, y, c), 4) }
            0x42 => { let cr = ram(a+1); (format!("FILL {}", reg(cr)), 2) }
            0x43 => { let xr = ram(a+1); let yr = ram(a+2); let wr = ram(a+3); let hr = ram(a+4); let cr = ram(a+5); (format!("RECTF {},{},{},{},{}", reg(xr), reg(yr), reg(wr), reg(hr), reg(cr)), 6) }
            0x44 => { let xr = ram(a+1); let yr = ram(a+2); let ar = ram(a+3); (format!("TEXT {},{},[{}]", reg(xr), reg(yr), reg(ar)), 4) }
            0x45 => { let x0r = ram(a+1); let y0r = ram(a+2); let x1r = ram(a+3); let y1r = ram(a+4); let cr = ram(a+5); (format!("LINE {},{},{},{},{}", reg(x0r), reg(y0r), reg(x1r), reg(y1r), reg(cr)), 6) }
            0x46 => { let xr = ram(a+1); let yr = ram(a+2); let rr = ram(a+3); let cr = ram(a+4); (format!("CIRCLE {},{},{},{}", reg(xr), reg(yr), reg(rr), reg(cr)), 5) }
            0x47 => { let nr = ram(a+1); (format!("SCROLL {}", reg(nr)), 2) }
            0x48 => { let rd = ram(a+1); (format!("IKEY {}", reg(rd)), 2) }
            0x49 => { let rd = ram(a+1); (format!("RAND {}", reg(rd)), 2) }
            0x4A => { let xr = ram(a+1); let yr = ram(a+2); let ar = ram(a+3); let wr = ram(a+4); let hr = ram(a+5); (format!("SPRITE {}, {}, {}, {}, {}", reg(xr), reg(yr), reg(ar), reg(wr), reg(hr)), 6) }
            0x4B => { let sr = ram(a+1); let dr = ram(a+2); (format!("ASM {}, {}", reg(sr), reg(dr)), 3) }
            0x4C => { 
                let xr = ram(a+1); let yr = ram(a+2); let mr = ram(a+3); let tr = ram(a+4);
                let gwr = ram(a+5); let ghr = ram(a+6); let twr = ram(a+7); let thr = ram(a+8);
                (format!("TILEMAP {}, {}, {}, {}, {}, {}, {}, {}", reg(xr), reg(yr), reg(mr), reg(tr), reg(gwr), reg(ghr), reg(twr), reg(thr)), 9)
            }
            0x4D => {
                let ar = ram(a + 1);
                (format!("SPAWN {}", reg(ar)), 2)
            }
            0x4E => {
                let pr = ram(a + 1);
                (format!("KILL {}", reg(pr)), 2)
            }
            0x4F => {
                let rx = ram(a + 1);
                let ry = ram(a + 2);
                let rd = ram(a + 3);
                (format!("PEEK {}, {}, {}", reg(rx), reg(ry), reg(rd)), 4)
            }
            0x50 => { let rd = ram(a+1); let rs = ram(a+2); (format!("CMP {}, {}", reg(rd), reg(rs)), 3) }
            0x51 => { let rd = ram(a+1); let rs = ram(a+2); (format!("MOV {}, {}", reg(rd), reg(rs)), 3) }

            0x60 => { let r = ram(a+1); (format!("PUSH {}", reg(r)), 2) }
            0x61 => { let r = ram(a+1); (format!("POP {}", reg(r)), 2) }
            0x52 => {
                let n = ram(a + 1);
                (format!("SYSCALL {}", n), 2)
            }
            0x53 => ("RETK".into(), 1),
            0x54 => {
                let pr = ram(a + 1);
                let mr = ram(a + 2);
                (format!("OPEN {}, {}", reg(pr), reg(mr)), 3)
            }
            0x55 => {
                let fr = ram(a + 1);
                let br = ram(a + 2);
                let lr = ram(a + 3);
                (format!("READ {}, {}, {}", reg(fr), reg(br), reg(lr)), 4)
            }
            0x56 => {
                let fr = ram(a + 1);
                let br = ram(a + 2);
                let lr = ram(a + 3);
                (format!("WRITE {}, {}, {}", reg(fr), reg(br), reg(lr)), 4)
            }
            0x57 => {
                let fr = ram(a + 1);
                (format!("CLOSE {}", reg(fr)), 2)
            }
            0x58 => {
                let fr = ram(a + 1);
                let or_ = ram(a + 2);
                let wr = ram(a + 3);
                (format!("SEEK {}, {}, {}", reg(fr), reg(or_), reg(wr)), 4)
            }
            0x59 => {
                let br = ram(a + 1);
                (format!("LS {}", reg(br)), 2)
            }

            0x5A => ("YIELD".into(), 1),
            0x5B => {
                let r = ram(a + 1);
                (format!("SLEEP {}", reg(r)), 2)
            }
            0x5C => {
                let r = ram(a + 1);
                (format!("SETPRIORITY {}", reg(r)), 2)
            }
            0x5D => {
                let rr = ram(a + 1);
                let rw = ram(a + 2);
                (format!("PIPE {}, {}", reg(rr), reg(rw)), 3)
            }
            0x5E => {
                let r = ram(a + 1);
                (format!("MSGSND {}", reg(r)), 2)
            }
            0x5F => ("MSGRCV".into(), 1),
            0x62 => {
                let fd = ram(a + 1);
                let cmd = ram(a + 2);
                let arg = ram(a + 3);
                (format!("IOCTL {}, {}, {}", reg(fd), reg(cmd), reg(arg)), 4)
            }
            0x63 => {
                let kr = ram(a + 1);
                let vr = ram(a + 2);
                (format!("GETENV {}, {}", reg(kr), reg(vr)), 3)
            }
            0x64 => {
                let kr = ram(a + 1);
                let vr = ram(a + 2);
                (format!("SETENV {}, {}", reg(kr), reg(vr)), 3)
            }
            0x65 => ("GETPID".into(), 1),
            0x66 => {
                let r = ram(a + 1);
                (format!("EXEC {}", reg(r)), 2)
            }
            0x67 => {
                let fr = ram(a + 1);
                let sr = ram(a + 2);
                (format!("WRITESTR {}, {}", reg(fr), reg(sr)), 3)
            }
            0x68 => {
                let br = ram(a + 1);
                let mr = ram(a + 2);
                let pr = ram(a + 3);
                (format!("READLN {}, {}, {}", reg(br), reg(mr), reg(pr)), 4)
            }
            0x69 => {
                let pr = ram(a + 1);
                (format!("WAITPID {}", reg(pr)), 2)
            }
            0x6A => {
                let pr = ram(a + 1);
                let sr = ram(a + 2);
                let dr = ram(a + 3);
                (
                    format!("EXECP {}, {}, {}", reg(pr), reg(sr), reg(dr)),
                    4,
                )
            }
            0x6B => {
                let pr = ram(a + 1);
                (format!("CHDIR {}", reg(pr)), 2)
            }
            0x6C => {
                let br = ram(a + 1);
                (format!("GETCWD {}", reg(br)), 2)
            }
            0x6D => {
                let dr = ram(a + 1);
                let xr = ram(a + 2);
                let yr = ram(a + 3);
                (format!("SCREENP {}, {}, {}", reg(dr), reg(xr), reg(yr)), 4)
            }
            0x6E => ("SHUTDOWN".into(), 1),
            0x6F => {
                let cr = ram(a + 1);
                (format!("EXIT {}", reg(cr)), 2)
            }
            0x70 => {
                let pr = ram(a + 1);
                let sr = ram(a + 2);
                (format!("SIGNAL {}, {}", reg(pr), reg(sr)), 3)
            }
            0x71 => {
                let sr = ram(a + 1);
                let hr = ram(a + 2);
                (format!("SIGSET {}, {}", reg(sr), reg(hr)), 3)
            }

            0x72 => {
                let ar = ram(a + 1);
                (format!("HYPERVISOR {}", reg(ar)), 2)
            }

            0x73 => ("ASMSELF".into(), 1),
            0x74 => ("RUNNEXT".into(), 1),

            0x75 => {
                let ti = ram(a + 1);
                let oc = ram(a + 2);
                let dc = ram(a + 3) as usize;
                let op_name = match oc {
                    0 => "ADD", 1 => "SUB", 2 => "MUL", 3 => "DIV",
                    4 => "AND", 5 => "OR", 6 => "XOR", 7 => "NOT",
                    8 => "COPY", 9 => "MAX", 10 => "MIN", 11 => "MOD",
                    12 => "SHL", 13 => "SHR", _ => "???",
                };
                let total = 4 + dc.min(MAX_FORMULA_DEPS);
                (format!("FORMULA {}, {}, {}", ti, op_name, dc), total)
            }
            0x76 => ("FORMULACLEAR".into(), 1),
            0x77 => {
                let ti = ram(a + 1);
                (format!("FORMULAREM {}", ti), 2)
            }
            0x78 => {
                let pr = ram(a + 1);
                (format!("FMKDIR [{}]", reg(pr)), 2)
            }
            0x79 => {
                let ir = ram(a + 1);
                let br = ram(a + 2);
                (format!("FSTAT {}, [{}]", reg(ir), reg(br)), 3)
            }
            0x7A => {
                let pr = ram(a + 1);
                (format!("FUNLINK [{}]", reg(pr)), 2)
            }

            _ => (format!("??? (0x{:02X})", op), 1),
        }
    }

    fn fetch(&mut self) -> u32 {
        let phys = match self.translate_va(self.pc) {
            Some(addr) if addr < self.ram.len() => addr,
            _ => { self.trigger_segfault(); return 0; }
        };
        let val = self.ram[phys];
        self.pc += 1;
        val
    }

    /// Draw a character to the screen buffer (tiny 5x7 inline font for TEXT opcode)
    fn draw_char(&mut self, ch: u8, x: usize, y: usize, color: u32) {
        // Simple 5x7 font for printable ASCII
        const MINI_FONT: [[u8; 7]; 96] = include!("../mini_font.in");
        let idx = ch as usize;
        if !(32..=127).contains(&idx) {
            return;
        }
        let glyph = &MINI_FONT[idx - 32];
        for (row, &glyph_row) in glyph.iter().enumerate().take(7usize) {
            for col in 0..5usize {
                if glyph_row & (1 << (4 - col)) != 0 {
                    let px = x + col;
                    let py = y + row;
                    if px < 256 && py < 256 {
                        self.screen[py * 256 + px] = color;
                    }
                }
            }
        }
    }

    /// Save VM state to a binary file.
    /// Format: GEOS magic (4) + version u32 (4) + halted u8 (1) + pc u32 (4)
    ///         + regs [u32; 32] (128) + ram [u32; RAM_SIZE] + screen [u32; SCREEN_SIZE]
    ///         + rand_state u32 (4) + frame_count u32 (4)   [version >= 2]
    #[cfg(not(target_arch = "wasm32"))]
    pub fn save_to_file(&self, path: &std::path::Path) -> std::io::Result<()> {
        use std::io::Write;
        let mut f = std::fs::File::create(path)?;
        f.write_all(SAVE_MAGIC)?;
        f.write_all(&SAVE_VERSION.to_le_bytes())?;
        f.write_all(&[if self.halted { 1 } else { 0 }])?;
        f.write_all(&self.pc.to_le_bytes())?;
        for &r in &self.regs {
            f.write_all(&r.to_le_bytes())?;
        }
        for &v in &self.ram {
            f.write_all(&v.to_le_bytes())?;
        }
        for &v in &self.screen {
            f.write_all(&v.to_le_bytes())?;
        }
        // v2 fields: persist RNG state and frame counter
        f.write_all(&self.rand_state.to_le_bytes())?;
        f.write_all(&self.frame_count.to_le_bytes())?;
        Ok(())
    }

    /// Load VM state from a binary file. Returns None if file doesn't exist
    /// or has invalid format.
    #[cfg(not(target_arch = "wasm32"))]
    #[allow(dead_code)]
    pub fn load_from_file(path: &std::path::Path) -> std::io::Result<Self> {
        use std::io::Read;
        let mut data = Vec::new();
        let mut f = std::fs::File::open(path)?;
        f.read_to_end(&mut data)?;

        // Minimum size: magic(4) + version(4) + halted(1) + pc(4) + regs(128) = 141
        let min_size = 4 + 4 + 1 + 4 + NUM_REGS * 4 + RAM_SIZE * 4 + SCREEN_SIZE * 4;
        if data.len() < min_size {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("save file too small: {} bytes (need {})", data.len(), min_size),
            ));
        }
        if &data[0..4] != SAVE_MAGIC {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "invalid magic bytes",
            ));
        }
        let version = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
        // Accept v1 saves (missing rand_state/frame_count) and v2
        if !(1..=SAVE_VERSION).contains(&version) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("unsupported save version: {} (need 1-{})", version, SAVE_VERSION),
            ));
        }

        let mut offset = 8usize;
        let halted = data[offset] != 0;
        offset += 1;
        let pc = u32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]);
        offset += 4;

        let mut regs = [0u32; NUM_REGS];
        for r in regs.iter_mut() {
            *r = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            offset += 4;
        }

        let mut ram = vec![0u32; RAM_SIZE];
        for v in ram.iter_mut() {
            *v = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            offset += 4;
        }

        let mut screen = vec![0u32; SCREEN_SIZE];
        for v in screen.iter_mut() {
            *v = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            offset += 4;
        }

        // v2 fields: rand_state + frame_count (default if v1 save)
        let (rand_state, frame_count) = if version >= 2
            && offset + 8 <= data.len()
        {
            let rs = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            let fc = u32::from_le_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);
            (rs, fc)
        } else {
            (0xDEADBEEF, 0) // v1 defaults
        };

        Ok(Vm {
            ram,
            regs,
            pc,
            screen,
            halted,
            frame_ready: false,
            rand_state,
            frame_count,
            beep: None,
            debug_mode: false,
            access_log: Vec::with_capacity(4096),
            processes: Vec::new(),
            mode: CpuMode::Kernel,
            kernel_stack: Vec::new(),
            allocated_pages: 0b11,
            page_ref_count: {
                let mut rc = [0u32; NUM_RAM_PAGES];
                rc[0] = 1;
                rc[1] = 1;
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
            shutdown_requested: false,
            step_exit_code: None,
            step_zombie: false,
            booted: false,
            hypervisor_active: false,
            hypervisor_config: String::new(),
            hypervisor_mode: HypervisorMode::default(),
            key_buffer: vec![0; 16],
            key_buffer_head: 0,
            key_buffer_tail: 0,
            formulas: Vec::new(),
            formula_dep_index: vec![Vec::new(); CANVAS_RAM_SIZE],
        })
    }
}

#[cfg(test)]
mod tests;
