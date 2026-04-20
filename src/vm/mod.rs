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
    /// Incremented each time FRAME fires; mirrored to RAM\[0xFFE\]
    pub frame_count: u32,
    /// Set by BEEP opcode: (freq_hz, duration_ms). Consumed and cleared by host.
    pub beep: Option<(u32, u32)>,
    /// Set by NOTE opcode: (waveform, freq_hz, duration_ms). Consumed by host.
    /// waveform: 0=sine, 1=square, 2=triangle, 3=sawtooth, 4=noise.
    pub note: Option<(u32, u32, u32)>,
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
    /// Opcode execution histogram: counts how many times each opcode (0x00-0xFF) was dispatched.
    /// Zero overhead -- just an array increment per step.
    pub opcode_histogram: [u64; 256],
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
    /// Pixel write history log (Phase 54: Pixel Write History).
    /// Records every PSET/PSETI when trace_recording is on.
    /// Hit-test regions for GUI interaction (HITSET/HITQ opcodes).
    pub hit_regions: Vec<HitRegion>,
    /// Current mouse/touch cursor X position, set by host via push_mouse().
    /// Queried by HITQ to find which region was clicked.
    pub mouse_x: u32,
    /// Current mouse/touch cursor Y position.
    pub mouse_y: u32,
    /// Mouse button state: 0=none, 1=left down, 2=left click (consumed on read).
    /// Set by host via push_mouse_button(). Queried by MOUSEQ into reg+2.
    pub mouse_button: u32,
    pub pixel_write_log: PixelWriteLog,
    /// Active TCP connections (Phase 41: Networking).
    /// Up to 8 simultaneous connections, indexed by fd.
    pub tcp_connections: Vec<Option<std::net::TcpStream>>,
    /// Managed windows (Phase 68: WINSYS opcode).
    /// Max MAX_WINDOWS active at once. Window IDs are 1-based.
    pub windows: Vec<Window>,
    /// Next window ID to assign (monotonically increasing).
    pub next_window_id: u32,
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
            note: None,
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
            opcode_histogram: [0; 256],
            key_buffer: vec![0; 16],
            key_buffer_head: 0,
            key_buffer_tail: 0,
            formulas: Vec::new(),
            formula_dep_index: vec![Vec::new(); CANVAS_RAM_SIZE],
            trace_recording: false,
            trace_buffer: TraceBuffer::new(DEFAULT_TRACE_CAPACITY),
            frame_checkpoints: FrameCheckBuffer::new(DEFAULT_FRAME_CHECK_CAPACITY),
            snapshots: Vec::new(),
            pixel_write_log: PixelWriteLog::new(DEFAULT_PIXEL_WRITE_CAPACITY),
            hit_regions: Vec::with_capacity(MAX_HIT_REGIONS),
            mouse_x: 0,
            mouse_y: 0,
            mouse_button: 0,
            tcp_connections: (0..MAX_TCP_CONNECTIONS).map(|_| None).collect(),
            windows: Vec::with_capacity(MAX_WINDOWS),
            next_window_id: 1,
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

    /// Update mouse/touch cursor position. Called by host on mouse events.
    /// The cursor is read by HITQ to determine which region was clicked.
    pub fn push_mouse(&mut self, x: u32, y: u32) {
        self.mouse_x = x;
        self.mouse_y = y;
        // Also mirror to RAM ports for direct access
        if (0xFF9) < self.ram.len() {
            self.ram[0xFF9] = x;
        }
        if (0xFFA) < self.ram.len() {
            self.ram[0xFFA] = y;
        }
    }

    /// Update mouse button state. Called by host on mouse button events.
    /// button: 0=none/release, 1=left down, 2=left click.
    pub fn push_mouse_button(&mut self, button: u32) {
        self.mouse_button = button;
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
        self.note = None;
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
        self.opcode_histogram = [0; 256];
        self.formulas.clear();
        for dep_list in self.formula_dep_index.iter_mut() {
            dep_list.clear();
        }
        self.trace_recording = false;
        self.trace_buffer.clear();
        self.frame_checkpoints.clear();
        self.snapshots.clear();
        self.pixel_write_log.clear();
        self.windows.clear();
        self.next_window_id = 1;
        self.mouse_button = 0;
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

        // Track opcode execution for diagnostic context
        self.opcode_histogram[opcode as usize] += 1;

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
                // Phase 68: blit active windows to screen in Z-order (lowest z first)
                self.blit_windows();
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

            // HITSET xr, yr, wr, hr, id  -- register a hit-test region
            // Adds a rectangular region to the hit table. Used for buttons,
            // clickable areas, and GUI elements. Max MAX_HIT_REGIONS regions.
            0x37 => {
                let xr = self.fetch() as usize;
                let yr = self.fetch() as usize;
                let wr = self.fetch() as usize;
                let hr = self.fetch() as usize;
                let id = self.fetch();
                if xr < NUM_REGS && yr < NUM_REGS && wr < NUM_REGS && hr < NUM_REGS {
                    if self.hit_regions.len() < MAX_HIT_REGIONS {
                        self.hit_regions.push(HitRegion {
                            x: self.regs[xr],
                            y: self.regs[yr],
                            w: self.regs[wr],
                            h: self.regs[hr],
                            id,
                        });
                    }
                }
            }

            // HITQ rd  -- query cursor against hit regions, write matching id to rd
            // Checks if current mouse position (self.mouse_x/y, set by host via
            // push_mouse) falls inside any registered HitRegion.
            // rd = region id if hit, 0 if no match. First match wins.
            0x38 => {
                let rd = self.fetch() as usize;
                if rd < NUM_REGS {
                    let mx = self.mouse_x;
                    let my = self.mouse_y;
                    let mut found_id = 0u32;
                    for region in &self.hit_regions {
                        if mx >= region.x
                            && mx < region.x + region.w
                            && my >= region.y
                            && my < region.y + region.h
                        {
                            found_id = region.id;
                            break;
                        }
                    }
                    self.regs[rd] = found_id;
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
            // NOTE wave_reg, freq_reg, dur_reg -- play a note with selectable waveform
            // wave_reg: 0=sine, 1=square, 2=triangle, 3=sawtooth, 4=noise
            // freq in Hz (20-20000), dur in ms (1-5000)
            0x7E => {
                let wr = self.fetch() as usize;
                let fr = self.fetch() as usize;
                let dr = self.fetch() as usize;
                if wr < NUM_REGS && fr < NUM_REGS && dr < NUM_REGS {
                    let wave = self.regs[wr].min(4);
                    let freq = self.regs[fr].clamp(20, 20000);
                    let dur = self.regs[dr].clamp(1, 5000);
                    self.note = Some((wave, freq, dur));
                }
            }
            // CONNECT addr_reg, port_reg, fd_reg  (0x7F) -- TCP connect
            // Reads null-terminated IP string from RAM[addr_reg], connects to port.
            // Returns fd in fd_reg, status in r0 (0=ok).
            0x7F => {
                self.op_connect();
            }
            // SOCKSEND fd_reg, buf_reg, len_reg, sent_reg  (0x80) -- TCP send
            // Sends len bytes from RAM[buf_reg]. Returns bytes sent in sent_reg.
            0x80 => {
                self.op_socksend();
            }
            // SOCKRECV fd_reg, buf_reg, max_len_reg, recv_reg  (0x81) -- TCP recv
            // Receives up to max_len bytes into RAM[buf_reg]. Returns bytes recv in recv_reg.
            0x81 => {
                self.op_sockrecv();
            }
            // DISCONNECT fd_reg  (0x82) -- TCP close
            // Closes connection and frees slot. Status in r0.
            0x82 => {
                self.op_disconnect();
            }
            // TRACE_READ mode_reg  (0x83) -- Query execution trace buffer from assembly.
            // Encoding: 0x83, mode_reg
            // mode_reg value:
            //   0 = query count: r0 = number of entries in trace buffer
            //   1 = read entry: r2 = index (0=oldest), r3 = dest RAM address
            //       Writes 20 words: [step_lo, step_hi, pc, r0..r15, opcode]
            //       r0 = 0 on success, 0xFFFFFFFF if index out of range
            //   2 = count opcode: r2 = target opcode, r0 = count of matching entries
            //   3 = find opcode indices: r2 = target opcode, r3 = dest RAM address
            //       Writes up to 256 entry indices (oldest to newest)
            //       r0 = number of matches written
            0x83 => {
                let mode_reg = self.fetch() as usize;
                let mode = if mode_reg < NUM_REGS {
                    self.regs[mode_reg]
                } else {
                    0
                };
                match mode {
                    0 => {
                        // Query: return number of entries
                        self.regs[0] = self.trace_buffer.len() as u32;
                    }
                    1 => {
                        // Read entry at index into RAM
                        let idx = self.regs[2] as usize;
                        let dest = self.regs[3] as usize;
                        if let Some(entry) = self.trace_buffer.get_at(idx) {
                            let step_lo = (entry.step_number & 0xFFFFFFFF) as u32;
                            let step_hi = ((entry.step_number >> 32) & 0xFFFFFFFF) as u32;
                            if dest + 20 <= self.ram.len() {
                                self.ram[dest] = step_lo;
                                self.ram[dest + 1] = step_hi;
                                self.ram[dest + 2] = entry.pc;
                                for i in 0..16 {
                                    self.ram[dest + 3 + i] = entry.regs[i];
                                }
                                self.ram[dest + 19] = entry.opcode;
                                self.regs[0] = 0; // success
                            } else {
                                self.regs[0] = 0xFFFFFFFF; // dest out of range
                            }
                        } else {
                            self.regs[0] = 0xFFFFFFFF; // index out of range
                        }
                    }
                    2 => {
                        // Count entries with specific opcode
                        let target = self.regs[2];
                        self.regs[0] = self.trace_buffer.count_opcode(target) as u32;
                    }
                    3 => {
                        // Find entries with specific opcode, write indices to RAM
                        let target = self.regs[2];
                        let dest = self.regs[3] as usize;
                        let indices = self.trace_buffer.find_opcode_indices(target, 256);
                        let count = indices.len().min(256);
                        if dest + count <= self.ram.len() {
                            for (i, &idx) in indices.iter().enumerate().take(count) {
                                self.ram[dest + i] = idx as u32;
                            }
                            self.regs[0] = count as u32;
                        } else {
                            self.regs[0] = 0xFFFFFFFF; // dest out of range
                        }
                    }
                    _ => {
                        self.regs[0] = 0xFFFFFFFF; // invalid mode
                    }
                }
            }
            // PIXEL_HISTORY mode_reg  (0x84) -- Query pixel write history.
            // Delegates to step_extended which has the full implementation.
            0x84 => {
                if !self.step_extended(opcode) {
                    return false;
                }
            }
            // MOUSEQ x_reg  (0x85) -- Query mouse position and button.
            // Reads current mouse X into x_reg, mouse Y into x_reg+1, button into x_reg+2.
            // Button: 0=none, 1=left down, 2=left click (auto-cleared after read).
            // Set by host via push_mouse(x, y) and push_mouse_button(btn).
            0x85 => {
                let xr = self.fetch() as usize;
                if xr < NUM_REGS && xr + 2 < NUM_REGS {
                    self.regs[xr] = self.mouse_x;
                    self.regs[xr + 1] = self.mouse_y;
                    self.regs[xr + 2] = self.mouse_button;
                    // Auto-clear click state after read (but not down state)
                    if self.mouse_button == 2 {
                        self.mouse_button = 1; // was click, now just down
                    }
                }
            }

            // STRCMP addr1_reg, addr2_reg -- compare two null-terminated strings
            // Sets r0: 0 if equal, 1 if s1 > s2, 0xFFFFFFFF (-1) if s1 < s2
            0x86 => {
                let a1 = self.fetch() as usize;
                let a2 = self.fetch() as usize;
                if a1 < NUM_REGS && a2 < NUM_REGS {
                    let mut addr1 = self.regs[a1] as usize;
                    let mut addr2 = self.regs[a2] as usize;
                    let mut result: i32 = 0;
                    loop {
                        let c1 = if addr1 < self.ram.len() {
                            (self.ram[addr1] & 0xFF) as u8
                        } else {
                            0
                        };
                        let c2 = if addr2 < self.ram.len() {
                            (self.ram[addr2] & 0xFF) as u8
                        } else {
                            0
                        };
                        if c1 == 0 && c2 == 0 {
                            result = 0; // equal (both null)
                            break;
                        }
                        if c1 < c2 {
                            result = -1;
                            break;
                        }
                        if c1 > c2 {
                            result = 1;
                            break;
                        }
                        addr1 += 1;
                        addr2 += 1;
                    }
                    self.regs[0] = result as u32;
                }
            }

            // ABS rd  (0x87) -- absolute value: rd = |rd|
            // Handles i32::MIN edge case (0x80000000) by returning itself
            0x87 => {
                let rd = self.fetch() as usize;
                if rd < NUM_REGS {
                    let val = self.regs[rd] as i32;
                    self.regs[rd] = val.wrapping_abs() as u32;
                }
            }

            // RECT x, y, w, h, color  (0x88) -- outline rectangle (4 edges only)
            0x88 => {
                let xr = self.fetch() as usize;
                let yr = self.fetch() as usize;
                let wr = self.fetch() as usize;
                let hr = self.fetch() as usize;
                let cr = self.fetch() as usize;
                if xr < NUM_REGS && yr < NUM_REGS && wr < NUM_REGS && hr < NUM_REGS && cr < NUM_REGS
                {
                    let x0 = self.regs[xr] as usize;
                    let y0 = self.regs[yr] as usize;
                    let w = self.regs[wr] as usize;
                    let h = self.regs[hr] as usize;
                    let color = self.regs[cr];
                    if w > 0 && h > 0 {
                        // Top edge
                        for dx in 0..w {
                            let px = x0 + dx;
                            if px < 256 && y0 < 256 {
                                self.screen[y0 * 256 + px] = color;
                            }
                        }
                        // Bottom edge
                        let by = y0 + h - 1;
                        for dx in 0..w {
                            let px = x0 + dx;
                            if px < 256 && by < 256 {
                                self.screen[by * 256 + px] = color;
                            }
                        }
                        // Left edge (excluding corners already drawn)
                        for dy in 1..h.saturating_sub(1) {
                            let py = y0 + dy;
                            if x0 < 256 && py < 256 {
                                self.screen[py * 256 + x0] = color;
                            }
                        }
                        // Right edge (excluding corners already drawn)
                        let rx = x0 + w - 1;
                        for dy in 1..h.saturating_sub(1) {
                            let py = y0 + dy;
                            if rx < 256 && py < 256 {
                                self.screen[py * 256 + rx] = color;
                            }
                        }
                    }
                }
            }

            // MIN rd, rs  (0x89) -- rd = min(rd, rs) as signed i32
            0x89 => {
                let rd = self.fetch() as usize;
                let rs = self.fetch() as usize;
                if rd < NUM_REGS && rs < NUM_REGS {
                    let a = self.regs[rd] as i32;
                    let b = self.regs[rs] as i32;
                    self.regs[rd] = a.min(b) as u32;
                }
            }

            // MAX rd, rs  (0x8A) -- rd = max(rd, rs) as signed i32
            0x8A => {
                let rd = self.fetch() as usize;
                let rs = self.fetch() as usize;
                if rd < NUM_REGS && rs < NUM_REGS {
                    let a = self.regs[rd] as i32;
                    let b = self.regs[rs] as i32;
                    self.regs[rd] = a.max(b) as u32;
                }
            }

            // CLAMP rd, min_reg, max_reg  (0x8B) -- rd = clamp(rd, min, max) as signed i32
            0x8B => {
                let rd = self.fetch() as usize;
                let min_r = self.fetch() as usize;
                let max_r = self.fetch() as usize;
                if rd < NUM_REGS && min_r < NUM_REGS && max_r < NUM_REGS {
                    let val = self.regs[rd] as i32;
                    let lo = self.regs[min_r] as i32;
                    let hi = self.regs[max_r] as i32;
                    self.regs[rd] = val.clamp(lo, hi) as u32;
                }
            }

            // DRAWTEXT x_reg, y_reg, addr_reg, fg_reg, bg_reg  (0x8C)
            // Render text from RAM with fg/bg colors. bg=0 means transparent.
            0x8C => {
                let xr = self.fetch() as usize;
                let yr = self.fetch() as usize;
                let ar = self.fetch() as usize;
                let fgr = self.fetch() as usize;
                let bgr = self.fetch() as usize;
                if xr < NUM_REGS
                    && yr < NUM_REGS
                    && ar < NUM_REGS
                    && fgr < NUM_REGS
                    && bgr < NUM_REGS
                {
                    let mut sx = self.regs[xr] as usize;
                    let mut sy = self.regs[yr] as usize;
                    let mut addr = self.regs[ar] as usize;
                    let fg = self.regs[fgr];
                    let bg_val = self.regs[bgr];
                    let bg = if bg_val == 0 { None } else { Some(bg_val) };
                    loop {
                        if addr >= self.ram.len() {
                            break;
                        }
                        let ch = (self.ram[addr] & 0xFF) as u8;
                        if ch == 0 {
                            break;
                        }
                        if ch == b'\n' {
                            // fill bg for rest of line if bg set
                            if let Some(bg_color) = bg {
                                for col in 0..6 {
                                    let px = sx + col;
                                    if px < 256 && sy < 256 && (sy + 7) < 256 {
                                        for row in 0..8 {
                                            self.screen[(sy + row) * 256 + px] = bg_color;
                                        }
                                    }
                                }
                            }
                            sx = self.regs[xr] as usize;
                            sy += 10;
                            addr += 1;
                            continue;
                        }
                        self.draw_char_with_bg(ch, sx, sy, fg, bg);
                        sx += 6;
                        if sx > 250 {
                            sx = self.regs[xr] as usize;
                            sy += 8;
                        }
                        addr += 1;
                    }
                }
            }

            // BITSET rd, bit_reg  (0x8D) -- rd |= 1 << bit_reg
            0x8D => {
                let rd = self.fetch() as usize;
                let br = self.fetch() as usize;
                if rd < NUM_REGS && br < NUM_REGS {
                    let bit = self.regs[br] & 31; // clamp to 0-31
                    self.regs[rd] |= 1 << bit;
                }
            }

            // BITCLR rd, bit_reg  (0x8E) -- rd &= !(1 << bit_reg)
            0x8E => {
                let rd = self.fetch() as usize;
                let br = self.fetch() as usize;
                if rd < NUM_REGS && br < NUM_REGS {
                    let bit = self.regs[br] & 31;
                    self.regs[rd] &= !(1 << bit);
                }
            }

            // BITTEST rd, bit_reg  (0x8F) -- r0 = (rd >> bit_reg) & 1
            0x8F => {
                let rd = self.fetch() as usize;
                let br = self.fetch() as usize;
                if rd < NUM_REGS && br < NUM_REGS {
                    let bit = self.regs[br] & 31;
                    self.regs[0] = (self.regs[rd] >> bit) & 1;
                }
            }

            // NOT rd  (0x90) -- rd = !rd (bitwise complement)
            0x90 => {
                let rd = self.fetch() as usize;
                if rd < NUM_REGS {
                    self.regs[rd] = !self.regs[rd];
                }
            }

            // INV  (0x91) -- invert all screen pixels (XOR 0xFFFFFF)
            0x91 => {
                for pixel in self.screen.iter_mut() {
                    *pixel ^= 0x00FFFFFF;
                }
            }

            // MATVEC r_weight, r_input, r_output, r_rows, r_cols (0x92)
            // Matrix-vector multiply using fixed-point 16.16 arithmetic.
            // output[i] = sum(weight[i*cols + j] * input[j]) >> 16
            // Addresses taken from registers, rows/cols from registers.
            0x92 => {
                let r_weight = self.fetch() as usize;
                let r_input = self.fetch() as usize;
                let r_output = self.fetch() as usize;
                let r_rows = self.fetch() as usize;
                let r_cols = self.fetch() as usize;
                if r_weight < NUM_REGS
                    && r_input < NUM_REGS
                    && r_output < NUM_REGS
                    && r_rows < NUM_REGS
                    && r_cols < NUM_REGS
                {
                    let weight_base = self.regs[r_weight] as usize;
                    let input_base = self.regs[r_input] as usize;
                    let output_base = self.regs[r_output] as usize;
                    let rows = self.regs[r_rows] as usize;
                    let cols = self.regs[r_cols] as usize;
                    for i in 0..rows {
                        let mut sum: i64 = 0;
                        for j in 0..cols {
                            let w_addr = weight_base + i * cols + j;
                            let i_addr = input_base + j;
                            if w_addr < self.ram.len() && i_addr < self.ram.len() {
                                // Fixed-point 16.16 multiply
                                let w = self.ram[w_addr] as i32;
                                let x = self.ram[i_addr] as i32;
                                sum += (w as i64 * x as i64) >> 16;
                            }
                        }
                        let o_addr = output_base + i;
                        if o_addr < self.ram.len() {
                            self.ram[o_addr] = sum as u32;
                        }
                    }
                }
            }

            // RELU rd (0x93) -- ReLU activation: if rd < 0 (signed), rd = 0
            0x93 => {
                let rd = self.fetch() as usize;
                if rd < NUM_REGS {
                    if (self.regs[rd] as i32) < 0 {
                        self.regs[rd] = 0;
                    }
                }
            }

            // WINSYS op_reg (0x94) -- Window management operations.
            // op=0: create window (r1=x, r2=y, r3=w, r4=h, r5=title_addr) -> r0=window_id
            // op=1: destroy window (r0=win_id)
            // op=2: bring to front (r0=win_id)
            // op=3: list windows (r0=addr to write list of u32: count, id1, id2, ...)
            0x94 => {
                let op_reg = self.fetch() as usize;
                if op_reg < NUM_REGS {
                    let op = self.regs[op_reg];
                    match op {
                        0 => {
                            // CREATE: r1=x, r2=y, r3=w, r4=h, r5=title_addr
                            let active_count = self.windows.iter().filter(|w| w.active).count();
                            if active_count >= MAX_WINDOWS {
                                self.regs[0] = 0; // no slots
                            } else {
                                let id = self.next_window_id;
                                self.next_window_id += 1;
                                let x = if 1 < NUM_REGS { self.regs[1] } else { 0 };
                                let y = if 2 < NUM_REGS { self.regs[2] } else { 0 };
                                let w = if 3 < NUM_REGS { self.regs[3] } else { 64 };
                                let h = if 4 < NUM_REGS { self.regs[4] } else { 48 };
                                let title_addr = if 5 < NUM_REGS { self.regs[5] } else { 0 };
                                let max_z =
                                    self.windows.iter().map(|w| w.z_order).max().unwrap_or(0);
                                let mut win =
                                    Window::new(id, x, y, w, h, title_addr, self.current_pid);
                                win.z_order = max_z + 1;
                                self.windows.push(win);
                                self.regs[0] = id;
                            }
                        }
                        1 => {
                            // DESTROY: r0=win_id
                            let win_id = self.regs[0];
                            if let Some(w) =
                                self.windows.iter_mut().find(|w| w.id == win_id && w.active)
                            {
                                w.active = false;
                            }
                        }
                        2 => {
                            // BRING TO FRONT: r0=win_id
                            let win_id = self.regs[0];
                            let max_z = self.windows.iter().map(|w| w.z_order).max().unwrap_or(0);
                            if let Some(w) =
                                self.windows.iter_mut().find(|w| w.id == win_id && w.active)
                            {
                                w.z_order = max_z + 1;
                            }
                        }
                        3 => {
                            // LIST: r0=addr to write list
                            let addr = self.regs[0] as usize;
                            let active: Vec<u32> = self
                                .windows
                                .iter()
                                .filter(|w| w.active)
                                .map(|w| w.id)
                                .collect();
                            if addr < self.ram.len() {
                                self.ram[addr] = active.len() as u32;
                            }
                            for (i, &id) in active.iter().enumerate() {
                                let slot = addr + 1 + i;
                                if slot < self.ram.len() {
                                    self.ram[slot] = id;
                                }
                            }
                        }
                        4 => {
                            // HITTEST: Check which window the mouse is over.
                            // Uses mouse_x, mouse_y. Iterates windows front-to-back
                            // (highest z_order first). Returns in r0: window_id (0=none).
                            // In r1: hit_type (0=none, 1=title bar, 2=body).
                            // Title bar = top 12 pixels of window (including border).
                            let mx = self.mouse_x;
                            let my = self.mouse_y;
                            let mut best_id: u32 = 0;
                            let mut best_hit: u32 = 0;
                            let mut best_z: u32 = 0;
                            for w in &self.windows {
                                if !w.active {
                                    continue;
                                }
                                // Check if mouse is within window bounds
                                let in_x = mx >= w.x && mx < w.x + w.w;
                                let in_y = my >= w.y && my < w.y + w.h;
                                if in_x && in_y && w.z_order > best_z {
                                    best_z = w.z_order;
                                    best_id = w.id;
                                    // Title bar = top 12 pixels
                                    if my < w.y + 12 {
                                        best_hit = 1; // title bar
                                    } else {
                                        best_hit = 2; // body
                                    }
                                }
                            }
                            self.regs[0] = best_id;
                            self.regs[1] = best_hit;
                        }
                        5 => {
                            // MOVETO: Move window to new position.
                            // r0=win_id, r1=new_x, r2=new_y.
                            let win_id = self.regs[0];
                            let new_x = if 1 < NUM_REGS { self.regs[1] } else { 0 };
                            let new_y = if 2 < NUM_REGS { self.regs[2] } else { 0 };
                            if let Some(w) =
                                self.windows.iter_mut().find(|w| w.id == win_id && w.active)
                            {
                                w.x = new_x;
                                w.y = new_y;
                                self.regs[0] = 1; // success
                            } else {
                                self.regs[0] = 0; // not found
                            }
                        }
                        6 => {
                            // WINFO: Get window info.
                            // r0=win_id. Writes [x, y, w, h, z_order, pid] to RAM
                            // starting at address in r1.
                            let win_id = self.regs[0];
                            let addr = if 1 < NUM_REGS {
                                self.regs[1] as usize
                            } else {
                                0
                            };
                            if let Some(w) =
                                self.windows.iter().find(|w| w.id == win_id && w.active)
                            {
                                let info = [w.x, w.y, w.w, w.h, w.z_order, w.pid];
                                for (i, &val) in info.iter().enumerate() {
                                    let slot = addr + i;
                                    if slot < self.ram.len() {
                                        self.ram[slot] = val;
                                    }
                                }
                                self.regs[0] = 1; // success
                            } else {
                                self.regs[0] = 0; // not found
                            }
                        }
                        _ => {
                            // Unknown op -- r0 = 0 (error)
                            self.regs[0] = 0;
                        }
                    }
                }
            }

            // WPIXEL win_id_reg, x_reg, y_reg, color_reg (0x95)
            // Write a pixel to a window's offscreen buffer.
            // If x or y is out of bounds for the window, the pixel is silently dropped.
            0x95 => {
                let wid_r = self.fetch() as usize;
                let xr = self.fetch() as usize;
                let yr = self.fetch() as usize;
                let cr = self.fetch() as usize;
                if wid_r < NUM_REGS && xr < NUM_REGS && yr < NUM_REGS && cr < NUM_REGS {
                    let win_id = self.regs[wid_r];
                    let px = self.regs[xr];
                    let py = self.regs[yr];
                    let color = self.regs[cr];
                    if let Some(win) = self.windows.iter_mut().find(|w| w.id == win_id && w.active)
                    {
                        let px_u = px as usize;
                        let py_u = py as usize;
                        let w_u = win.w as usize;
                        let h_u = win.h as usize;
                        if px_u < w_u && py_u < h_u {
                            let idx = py_u * w_u + px_u;
                            if idx < win.offscreen_buffer.len() {
                                win.offscreen_buffer[idx] = color;
                            }
                        }
                    }
                }
            }

            // WREAD win_id_reg, x_reg, y_reg, dest_reg (0x96)
            // Read a pixel from a window's offscreen buffer into dest_reg.
            // Out-of-bounds reads set dest to 0.
            0x96 => {
                let wid_r = self.fetch() as usize;
                let xr = self.fetch() as usize;
                let yr = self.fetch() as usize;
                let dr = self.fetch() as usize;
                if wid_r < NUM_REGS && xr < NUM_REGS && yr < NUM_REGS && dr < NUM_REGS {
                    let win_id = self.regs[wid_r];
                    let px = self.regs[xr];
                    let py = self.regs[yr];
                    if let Some(win) = self.windows.iter().find(|w| w.id == win_id && w.active) {
                        let px_u = px as usize;
                        let py_u = py as usize;
                        let w_u = win.w as usize;
                        let h_u = win.h as usize;
                        if px_u < w_u && py_u < h_u {
                            let idx = py_u * w_u + px_u;
                            if idx < win.offscreen_buffer.len() {
                                self.regs[dr] = win.offscreen_buffer[idx];
                            } else {
                                self.regs[dr] = 0;
                            }
                        } else {
                            self.regs[dr] = 0;
                        }
                    } else {
                        self.regs[dr] = 0;
                    }
                }
            }

            _ => {
                self.halted = true;
                return false;
            }
        }
        true
    }

    /// Blit all active windows to the screen in Z-order (lowest z first).
    /// Non-zero pixels in the offscreen buffer overwrite the screen.
    /// Zero pixels (0x00000000) are transparent -- they don't overwrite.
    /// Clip at screen edges (256x256).
    pub fn blit_windows(&mut self) {
        // Collect (id, x, y, w, h, z_order) for active windows, sorted by z_order ascending
        let mut wins: Vec<(u32, u32, u32, u32, u32, u32)> = self
            .windows
            .iter()
            .filter(|w| w.active)
            .map(|w| (w.id, w.x, w.y, w.w, w.h, w.z_order))
            .collect();
        wins.sort_by_key(|w| w.5); // sort by z_order ascending (lowest first)

        for (win_id, wx, wy, ww, wh, _z) in wins {
            // Find the window and blit its offscreen buffer
            // We need to clone the relevant parts to avoid borrow issues
            let win_data: Option<(u32, u32, u32, Vec<u32>)> = self
                .windows
                .iter()
                .find(|w| w.id == win_id)
                .map(|w| (w.x, w.y, w.w, w.offscreen_buffer.clone()));
            if let Some((wx, wy, ww, buf)) = win_data {
                let w_usize = ww as usize;
                for py in 0..wh as usize {
                    for px in 0..ww as usize {
                        let color = buf[py * w_usize + px];
                        if color != 0 {
                            // Transparent pixels (0x00000000) don't overwrite
                            let sx = wx as i32 + px as i32;
                            let sy = wy as i32 + py as i32;
                            if sx >= 0 && sx < 256 && sy >= 0 && sy < 256 {
                                self.screen[(sy as usize) * 256 + (sx as usize)] = color;
                            }
                        }
                    }
                }
            }
        }
    }
}

mod boot;
mod disasm;
mod net;
pub(crate) use net::MAX_TCP_CONNECTIONS;
mod scheduler;

#[cfg(test)]
mod tests;
