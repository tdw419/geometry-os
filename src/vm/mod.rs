use std::io::Write;
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
    /// Cached capabilities of the currently executing context.
    /// Set by the scheduler when dispatching child processes.
    /// None = no capabilities (full access, backward compatible).
    pub current_capabilities: Option<Vec<crate::vm::types::Capability>>,
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
    /// Window ID to render hypervisor output into (Phase 86).
    /// 0 = full canvas (default), >0 = target WINSYS window offscreen buffer.
    pub hypervisor_window_id: u32,
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
    /// Legacy single-key port (replaces RAM[0xFFF] to avoid bytecode overlap).
    /// Programs loaded at address 0 may extend past 0xFFF (e.g., ai_terminal.asm
    /// at 4479 words). Using a separate field prevents IKEY/push_key from
    /// clobbering bytecode at that address.
    pub key_port: u32,
    /// Active formulas on canvas cells (Phase 50: Reactive Canvas).
    pub formulas: Vec<Formula>,
    /// Reverse dependency index: dep_idx -> list of formula indices in self.formulas.
    /// Used to quickly find which formulas need recalculation when a cell changes.
    /// Reverse dependency index for reactive canvas formulas.
    /// Maps canvas cell index -> list of formula indices that depend on it.
    /// Uses HashMap for lazy allocation (no 4096 empty Vecs at construction).
    pub formula_dep_index: std::collections::HashMap<usize, Vec<usize>>,
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
    /// Network inbox: received pixel frames waiting to be consumed by NET_RECV.
    /// Each entry is a Vec<u32> containing a pixel protocol frame.
    pub net_inbox: Vec<Vec<u32>>,
    /// Managed windows (Phase 68: WINSYS opcode).
    /// Max MAX_WINDOWS active at once. Window IDs are 1-based.
    pub windows: Vec<Window>,
    /// Next window ID to assign (monotonically increasing).
    pub next_window_id: u32,
    /// Mock LLM response for testing. When set, the LLM opcode returns this
    /// instead of making a real API call. Cleared after use.
    pub llm_mock_response: Option<String>,
    /// LLM configuration URL. Defaults to provider.json or local Ollama.
    /// Can be overridden by tests or host. Format: "url model api_key"
    pub llm_config: Option<String>,
    /// Background hypervisor VM instances (Phase 87: Multi-Hypervisor).
    /// Each building on the map can host one. Host time-slices between active ones.
    pub background_vms: Vec<BackgroundVm>,
    /// Next background VM ID to assign (monotonically increasing, 1-based).
    pub next_bg_vm_id: u32,

    // ── Inter-Tile Mailbox (Phase 4.1) ──────────────────────────
    /// Write buffer for mailbox sends. Messages are committed on FRAME.
    pub mailbox_write_buf: Vec<MailboxEntry>,
    /// Read buffer for mailbox receives. Swapped from write_buf on FRAME.
    pub mailbox_read_buf: Vec<MailboxEntry>,

    // ── Crash Recovery (Phase 104) ───────────────────────────────
    /// Virtual address that caused the last segfault (0 if none).
    /// Set by trigger_segfault_with_addr.
    pub segfault_addr: u32,
    /// Circular buffer of last 16 PC values executed (across all processes).
    /// Used to reconstruct the instruction trace in core dumps.
    pub pc_trace: [u32; 16],
    /// Current write index into pc_trace (wraps around).
    pub pc_trace_idx: usize,
    /// True when a crash dialog is being displayed on screen.
    pub crash_dialog_active: bool,
    /// PID of the process whose crash dialog is displayed.
    pub crash_dialog_pid: u32,
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
            current_capabilities: None,
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
            hypervisor_window_id: 0,
            opcode_histogram: [0; 256],
            key_buffer: vec![0; 16],
            key_buffer_head: 0,
            key_buffer_tail: 0,
            key_port: 0,
            formulas: Vec::new(),
            formula_dep_index: std::collections::HashMap::new(),
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
            net_inbox: Vec::new(),
            windows: Vec::with_capacity(MAX_WINDOWS),
            next_window_id: 1,
            llm_mock_response: None,
            llm_config: None,
            background_vms: Vec::new(),
            next_bg_vm_id: 1,
            mailbox_write_buf: vec![MailboxEntry::default(); MAILBOX_SIZE],
            mailbox_read_buf: vec![MailboxEntry::default(); MAILBOX_SIZE],
            segfault_addr: 0,
            pc_trace: [0; 16],
            pc_trace_idx: 0,
            crash_dialog_active: false,
            crash_dialog_pid: 0,
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
        // Write to key_port field (separate from RAM to avoid bytecode overlap)
        self.key_port = key;
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
        self.hypervisor_window_id = 0;
        self.opcode_histogram = [0; 256];
        self.formulas.clear();
        for dep_list in self.formula_dep_index.values_mut() {
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
        self.net_inbox.clear();
        self.llm_mock_response = None;
        self.hit_regions.clear();
        self.llm_config = None;
        self.background_vms.clear();
        self.next_bg_vm_id = 1;
        self.mailbox_write_buf = vec![MailboxEntry::default(); MAILBOX_SIZE];
        self.mailbox_read_buf = vec![MailboxEntry::default(); MAILBOX_SIZE];
        self.segfault_addr = 0;
        self.pc_trace = [0; 16];
        self.pc_trace_idx = 0;
        self.crash_dialog_active = false;
        self.crash_dialog_pid = 0;
        self.key_port = 0;
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
pub mod types;
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

        // Record PC in circular trace buffer (Phase 104: Crash Recovery)
        self.pc_trace[self.pc_trace_idx] = pc_addr as u32;
        self.pc_trace_idx = (self.pc_trace_idx + 1) % self.pc_trace.len();

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
                // Phase 4.1: swap mailbox buffers on FRAME boundary
                // write_buf -> read_buf for consumption next frame, clear write_buf
                std::mem::swap(&mut self.mailbox_write_buf, &mut self.mailbox_read_buf);
                for entry in self.mailbox_write_buf.iter_mut() {
                    entry.valid = false;
                    entry.sender_id = 0;
                    entry.data = 0;
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
                    let result: i32;
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
                            // When RAM[WINDOW_WORLD_COORDS_ADDR] == 1, r1/r2 are world coords.
                            // Phase 107: if RAM[0x7810] == 1, r1/r2 are world-space coords
                            let active_count = self.windows.iter().filter(|w| w.active).count();
                            if active_count >= MAX_WINDOWS {
                                self.regs[0] = 0; // no slots
                            } else {
                                let id = self.next_window_id;
                                self.next_window_id += 1;
                                let arg1 = if 1 < NUM_REGS { self.regs[1] } else { 0 };
                                let arg2 = if 2 < NUM_REGS { self.regs[2] } else { 0 };
                                let w = if 3 < NUM_REGS { self.regs[3] } else { 64 };
                                let h = if 4 < NUM_REGS { self.regs[4] } else { 48 };
                                let title_addr = if 5 < NUM_REGS { self.regs[5] } else { 0 };
                                let max_z =
                                    self.windows.iter().map(|w| w.z_order).max().unwrap_or(0);

                                // Check world-space flag: RAM[0x7810]
                                let world_mode = self.ram.get(0x7810).copied().unwrap_or(0) == 1;
                                let mut win = if world_mode {
                                    // r1=world_x, r2=world_y (tile coords)
                                    Window::new_world(
                                        id,
                                        arg1,
                                        arg2,
                                        w,
                                        h,
                                        title_addr,
                                        self.current_pid,
                                    )
                                } else {
                                    // r1=screen_x, r2=screen_y (legacy)
                                    Window::new(id, arg1, arg2, w, h, title_addr, self.current_pid)
                                };
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
                            // For world-space windows, updates world_x/world_y.
                            let win_id = self.regs[0];
                            let new_x = if 1 < NUM_REGS { self.regs[1] } else { 0 };
                            let new_y = if 2 < NUM_REGS { self.regs[2] } else { 0 };
                            if let Some(w) =
                                self.windows.iter_mut().find(|w| w.id == win_id && w.active)
                            {
                                if w.is_world_space() {
                                    w.world_x = new_x;
                                    w.world_y = new_y;
                                } else {
                                    w.x = new_x;
                                    w.y = new_y;
                                }
                                self.regs[0] = 1; // success
                            } else {
                                self.regs[0] = 0; // not found
                            }
                        }
                        6 => {
                            // WINFO: Get window info.
                            // r0=win_id. Writes [x, y, w, h, z_order, pid, world_x, world_y] to RAM
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
                                let info =
                                    [w.x, w.y, w.w, w.h, w.z_order, w.pid, w.world_x, w.world_y];
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
                        7 => {
                            // RESIZE: Resize window offscreen buffer.
                            // r0=win_id, r1=new_w, r2=new_h.
                            let win_id = self.regs[0];
                            let new_w = if 1 < NUM_REGS { self.regs[1] } else { 64 };
                            let new_h = if 2 < NUM_REGS { self.regs[2] } else { 48 };
                            if new_w == 0 || new_h == 0 || new_w > 256 || new_h > 256 {
                                self.regs[0] = 0; // invalid size
                            } else if let Some(w) =
                                self.windows.iter_mut().find(|w| w.id == win_id && w.active)
                            {
                                w.w = new_w;
                                w.h = new_h;
                                w.offscreen_buffer
                                    .resize((new_w as usize) * (new_h as usize), 0);
                                self.regs[0] = 1; // success
                            } else {
                                self.regs[0] = 0; // not found
                            }
                        }
                        8 => {
                            // VFS_BLIT: Paint the VFS pixel surface into a window.
                            // r0=win_id. Reads all files from .geometry_os/fs/,
                            // encodes them as RGBA pixels, and writes into the
                            // window's offscreen buffer. "Pixels move pixels" --
                            // the files ARE the visible pixels.
                            // r0=1 on success, 0 on error.
                            let win_id = self.regs[0];
                            if let Some(w) =
                                self.windows.iter_mut().find(|w| w.id == win_id && w.active)
                            {
                                let surface =
                                    crate::vfs::encode_pixel_surface(w.w as usize, w.h as usize);
                                // Blit surface into window offscreen buffer
                                let buf_len = w.offscreen_buffer.len();
                                for (i, &px) in surface.iter().enumerate() {
                                    if i < buf_len {
                                        w.offscreen_buffer[i] = px;
                                    }
                                }
                                self.regs[0] = 1; // success
                            } else {
                                self.regs[0] = 0; // window not found
                            }
                        }
                        9 => {
                            // VFS_SYNC: Write the window's pixels back to the VFS files.
                            // Inverse of VFS_BLIT. r0=win_id.
                            let win_id = self.regs[0];
                            if let Some(w) =
                                self.windows.iter_mut().find(|w| w.id == win_id && w.active)
                            {
                                crate::vfs::decode_pixel_surface(
                                    w.w as usize,
                                    w.h as usize,
                                    &w.offscreen_buffer,
                                );
                                self.regs[0] = 1; // success
                            } else {
                                self.regs[0] = 0; // window not found
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

            // SPRBLT sheet_addr_reg, sprite_id_reg, x_reg, y_reg (0x97)
            // Blit a 16x16 sprite from a sprite sheet in RAM to the screen.
            // Sprite sheet: contiguous array of 16x16 pixel sprites.
            // Sprite data starts at: sheet_addr + sprite_id * 256
            // Each sprite is 16x16 = 256 u32 pixels (row-major).
            // Pixels with value 0 are transparent (skipped).
            // Clipped to screen boundaries (0..256).
            0x97 => {
                let sheet_r = self.fetch() as usize;
                let sid_r = self.fetch() as usize;
                let xr = self.fetch() as usize;
                let yr = self.fetch() as usize;
                if sheet_r < NUM_REGS && sid_r < NUM_REGS && xr < NUM_REGS && yr < NUM_REGS {
                    let sheet_addr = self.regs[sheet_r] as usize;
                    let sprite_id = self.regs[sid_r] as usize;
                    let sx = self.regs[xr] as i32;
                    let sy = self.regs[yr] as i32;

                    let sprite_offset = sprite_id * 256; // 16x16 pixels per sprite
                    let data_start = sheet_addr + sprite_offset;

                    for dy in 0..16usize {
                        for dx in 0..16usize {
                            let ram_addr = data_start + dy * 16 + dx;
                            if ram_addr >= self.ram.len() {
                                break;
                            }
                            let color = self.ram[ram_addr];
                            if color == 0 {
                                continue; // transparent
                            }
                            let px = sx + dx as i32;
                            let py = sy + dy as i32;
                            if (0..256).contains(&px) && (0..256).contains(&py) {
                                self.screen[(py as usize) * 256 + (px as usize)] = color;
                            }
                        }
                    }
                }
            }

            // SCRSHOT path_addr_reg (0x98) -- Screenshot: save screen to VFS file
            // Reads null-terminated path from RAM at address in register.
            // Writes 256x256 raw RGBA u32 pixels to the VFS file.
            // Returns fd in r0 (or 0xFFFFFFFF on error).
            0x98 => {
                let pr = self.fetch() as usize;
                if pr < NUM_REGS {
                    let path_addr = self.regs[pr] as usize;
                    // Read the current process's PID for VFS
                    let pid = self.current_pid;
                    let fd = self.vfs.fopen(&self.ram, path_addr as u32, 1, pid); // FOPEN_WRITE
                    if fd != 0xFFFFFFFF {
                        // Write screen pixels to file
                        // Pack screen as bytes: each u32 pixel = 4 bytes (RGBA)
                        let mut pixel_bytes: Vec<u8> = Vec::with_capacity(256 * 256 * 4);
                        for &pixel in self.screen.iter() {
                            pixel_bytes.push((pixel >> 24) as u8); // A
                            pixel_bytes.push((pixel >> 16) as u8); // R
                            pixel_bytes.push((pixel >> 8) as u8); // G
                            pixel_bytes.push(pixel as u8); // B
                        }
                        // Write bytes to VFS via fwrite - need to stage in RAM first
                        // Use a temporary RAM region to stage bytes
                        let stage_base = 0x9000u32;
                        let chunk_size = 512u32; // write 512 bytes at a time
                        let mut written: u32 = 0;
                        let total_bytes = pixel_bytes.len() as u32;
                        let mut offset = 0u32;
                        while offset < total_bytes {
                            let end = std::cmp::min(offset + chunk_size, total_bytes);
                            let len = end - offset;
                            // Copy bytes to staging area
                            for i in 0..len {
                                let addr = (stage_base as usize) + (i as usize);
                                if addr < self.ram.len() {
                                    self.ram[addr] =
                                        pixel_bytes[(offset as usize) + (i as usize)] as u32;
                                }
                            }
                            let n = self.vfs.fwrite(&self.ram, fd, stage_base, len, pid);
                            if n == 0xFFFFFFFF {
                                let _ = self.vfs.fclose(fd, pid);
                                self.regs[0] = 0xFFFFFFFF;
                                written = 0;
                                break;
                            }
                            written += n;
                            offset = end;
                        }
                        self.vfs.fclose(fd, pid);
                        self.regs[0] = written; // total bytes written
                    } else {
                        self.regs[0] = 0xFFFFFFFF;
                    }
                }
            }

            // NET_SEND addr_reg, len_reg, dest_reg (0x99)
            //
            // Send pixel data to a connected peer via the pixel protocol.
            // Reads `len` u32 words from RAM starting at `addr_reg`, wraps them
            // in a pixel protocol frame, and sends via the TCP connection
            // identified by `dest_reg` (connection fd).
            //
            // Pixel protocol frame format (all u32 words):
            //   [0] = header: (frame_type << 24) | (width << 16) | (height << 8) | flags
            //         frame_type: 0=screen_share, 1=chat, 2=file
            //         flags: bit 0 = compressed (future)
            //   [1..] = pixel data (width * height u32 RGBA values)
            //
            // For simple sends, width=1 and height=len provides a raw data transfer.
            // r0 = NET_OK on success, error code on failure.
            // After success, r0 = number of u32 words sent (including header).
            0x99 => {
                let ar = self.fetch() as usize;
                let lr = self.fetch() as usize;
                let dr = self.fetch() as usize;

                if ar >= NUM_REGS || lr >= NUM_REGS || dr >= NUM_REGS {
                    self.regs[0] = 0xFFFFFFFF;
                } else {
                    let buf_addr = self.regs[ar] as usize;
                    let len = self.regs[lr] as usize;
                    let fd = self.regs[dr] as usize;

                    if fd >= crate::vm::net::MAX_TCP_CONNECTIONS
                        || self.tcp_connections[fd].is_none()
                    {
                        self.regs[0] = crate::vm::net::NET_ERR_INVALID_FD;
                    } else {
                        // Build the pixel protocol frame
                        // Header: type=0 (screen_share), width=len, height=1, flags=0
                        let header =
                            ((0u32) << 24) | ((len.min(255) as u32) << 16) | (1u32 << 8) | 0u32;
                        let mut frame = vec![0u8; 4 + len.min(65536) * 4];
                        // Write header as big-endian u32
                        frame[0] = (header >> 24) as u8;
                        frame[1] = (header >> 16) as u8;
                        frame[2] = (header >> 8) as u8;
                        frame[3] = header as u8;
                        // Write pixel data as little-endian u32 array
                        let data_len = len.min(65536);
                        for i in 0..data_len {
                            let idx = buf_addr + i;
                            let word = if idx < self.ram.len() {
                                self.ram[idx]
                            } else {
                                0
                            };
                            let off = 4 + i * 4;
                            frame[off] = word as u8;
                            frame[off + 1] = (word >> 8) as u8;
                            frame[off + 2] = (word >> 16) as u8;
                            frame[off + 3] = (word >> 24) as u8;
                        }
                        let frame_bytes = 4 + data_len * 4;

                        if let Some(ref mut stream) = self.tcp_connections[fd] {
                            match stream.write_all(&frame[..frame_bytes]) {
                                Ok(()) => {
                                    self.regs[0] = (data_len + 1) as u32; // words sent (header + data)
                                }
                                Err(_) => {
                                    self.regs[0] = crate::vm::net::NET_ERR_SEND_FAILED;
                                }
                            }
                        } else {
                            self.regs[0] = crate::vm::net::NET_ERR_INVALID_FD;
                        }
                    }
                }
            }

            // NET_RECV addr_reg, max_len_reg (0x9A)
            //
            // Receive pending pixel data from the network inbox into RAM.
            // Non-blocking: reads the oldest frame from the inbox queue.
            // Stores the pixel data (without header) starting at RAM[addr_reg].
            //
            // r0 = number of u32 words received (0 if inbox empty).
            // The frame header is written to RAM[addr_reg - 4..addr_reg] if there's room:
            //   RAM[addr-4] = frame_type, RAM[addr-3] = width, RAM[addr-2] = height, RAM[addr-1] = flags
            // Or the caller can check r0 for the data length.
            //
            // For testing without a network: push frames directly into vm.net_inbox.
            0x9A => {
                let ar = self.fetch() as usize;
                let mr = self.fetch() as usize;

                if ar >= NUM_REGS || mr >= NUM_REGS {
                    self.regs[0] = 0;
                } else if self.net_inbox.is_empty() {
                    self.regs[0] = 0; // nothing to receive
                } else {
                    let buf_addr = self.regs[ar] as usize;
                    let max_len = self.regs[mr] as usize;

                    let frame = self.net_inbox.remove(0);
                    if frame.len() < 1 {
                        self.regs[0] = 0;
                    } else {
                        // Frame format: first word is header, rest is pixel data
                        // Header: (type << 24) | (width << 16) | (height << 8) | flags
                        let header = frame[0];
                        // Write header to RAM at buf_addr..buf_addr+4
                        if buf_addr + 3 < self.ram.len() {
                            self.ram[buf_addr] = (header >> 24) & 0xFF; // type
                            self.ram[buf_addr + 1] = (header >> 16) & 0xFF; // width
                            self.ram[buf_addr + 2] = (header >> 8) & 0xFF; // height
                            self.ram[buf_addr + 3] = header & 0xFF; // flags
                        }
                        // Write pixel data starting at buf_addr + 4
                        let data_len = (frame.len() - 1).min(max_len as usize);
                        for i in 0..data_len {
                            let idx = buf_addr + 4 + i;
                            if idx < self.ram.len() {
                                self.ram[idx] = frame[1 + i];
                            }
                        }
                        self.regs[0] = (data_len + 4) as u32; // total words written
                    }
                }
            }

            // PROCLS buf_reg (0x9B) -- list running process PIDs into RAM buffer
            // Writes PID of each active process (including main PID 0) as u32 words
            // starting at RAM[buf_reg]. Returns count in r0.
            0x9B => {
                let br = self.fetch() as usize;
                if br < NUM_REGS {
                    let mut buf_addr = self.regs[br] as usize;
                    let mut count: u32 = 0;
                    // Write main process PID (0)
                    if buf_addr < self.ram.len() {
                        self.ram[buf_addr] = 0;
                        count += 1;
                        buf_addr += 1;
                    }
                    // Write spawned process PIDs
                    for p in &self.processes {
                        if buf_addr < self.ram.len() {
                            self.ram[buf_addr] = p.pid;
                            count += 1;
                            buf_addr += 1;
                        }
                    }
                    self.regs[0] = count;
                } else {
                    self.regs[0] = 0;
                }
            }

            // LLM prompt_addr_reg, response_addr_reg, max_len_reg (0x9C)
            // Sends null-terminated prompt string from RAM to an LLM API.
            // Response written to RAM at response_addr. r0 = response length (0 on error).
            // Uses llm_mock_response if set (for testing), otherwise calls curl.
            0x9C => {
                let r_prompt = self.fetch() as usize;
                let r_response = self.fetch() as usize;
                let r_max_len = self.fetch() as usize;
                if r_prompt < NUM_REGS && r_response < NUM_REGS && r_max_len < NUM_REGS {
                    let prompt_addr = self.regs[r_prompt] as usize;
                    let response_addr = self.regs[r_response] as usize;
                    let max_len = self.regs[r_max_len] as usize;

                    // Read null-terminated prompt string from RAM
                    let mut prompt = String::new();
                    let mut addr = prompt_addr;
                    while addr < self.ram.len() {
                        let ch = self.ram[addr];
                        if ch == 0 {
                            break;
                        }
                        if let Some(c) = char::from_u32(ch) {
                            prompt.push(c);
                        } else {
                            prompt.push('?');
                        }
                        addr += 1;
                    }

                    // Get response: use mock if available, otherwise call LLM
                    let response = if let Some(mock) = self.llm_mock_response.take() {
                        mock
                    } else if prompt.is_empty() {
                        String::new()
                    } else {
                        // Call external LLM via curl (like hermes.rs call_llm pattern)
                        self.call_llm_external(&prompt).unwrap_or_default()
                    };

                    // Write response to RAM, one char per u32 word
                    let write_len = response.len().min(max_len);
                    for (i, byte) in response.bytes().take(write_len).enumerate() {
                        let dest = response_addr + i;
                        if dest < self.ram.len() {
                            self.ram[dest] = byte as u32;
                        }
                    }
                    // Null-terminate if space allows
                    if response_addr + write_len < self.ram.len() {
                        self.ram[response_addr + write_len] = 0;
                    }
                    self.regs[0] = write_len as u32;
                } else {
                    self.regs[0] = 0; // error: invalid registers
                }
            }

            // HTPARSE src_addr_reg, dest_addr_reg, max_lines_reg  (0x9D)
            // Parse HTML from RAM at src_addr into styled lines at dest_addr.
            // Each line = 33 u32 words: [fg_color, char0, char1, ..., char31].
            // Links are registered in hit_regions for HITQ click detection.
            // Returns: r0 = number of parsed lines.
            0x9D => {
                let sr = self.fetch() as usize;
                let dr = self.fetch() as usize;
                let mr = self.fetch() as usize;
                if sr < NUM_REGS && dr < NUM_REGS && mr < NUM_REGS {
                    let src_addr = self.regs[sr] as usize;
                    let dest_addr = self.regs[dr] as usize;
                    let max_lines = self.regs[mr] as usize;

                    // Read HTML from RAM
                    let mut html = String::new();
                    let mut a = src_addr;
                    while a < self.ram.len() {
                        let ch = self.ram[a];
                        if ch == 0 {
                            break;
                        }
                        if let Some(c) = char::from_u32(ch) {
                            html.push(c);
                        } else {
                            html.push('?');
                        }
                        a += 1;
                    }

                    // Parse HTML into styled lines
                    let parsed = self.parse_html_to_lines(&html, max_lines, dest_addr);

                    // Write styled lines to dest_addr
                    let line_size = 33;
                    for (line_idx, line) in parsed.iter().enumerate() {
                        let base = dest_addr + line_idx * line_size;
                        if base + line_size > self.ram.len() {
                            break;
                        }
                        self.ram[base] = line.fg_color;
                        for (j, &ch) in line.chars.iter().enumerate() {
                            if j < 32 {
                                self.ram[base + 1 + j] = ch;
                            }
                        }
                        for j in line.chars.len()..32 {
                            if base + 1 + j < self.ram.len() {
                                self.ram[base + 1 + j] = 0;
                            }
                        }
                    }

                    self.regs[0] = parsed.len() as u32;
                } else {
                    self.regs[0] = 0;
                }
            }

            // HITCLR  (0x9E) -- clear all hit-test regions
            0x9E => {
                self.hit_regions.clear();
            }

            // ── Phase 87: Multi-Hypervisor Opcodes ──────────────────────

            // VM_SPAWN config_reg, window_reg (0x9F) -- Create background hypervisor VM.
            // Reads config string from RAM at address in config_reg.
            // window_reg: WINSYS window_id (0 = full canvas).
            // Returns VM instance ID in r0 (1-based). 0xFFFFFFFF on error.
            // Max 4 concurrent VMs. Config must have arch= parameter.
            // Encoding: 3 words [0x9F, config_reg, window_reg]
            0x9F => {
                let config_reg = self.fetch() as usize;
                let win_reg = self.fetch() as usize;
                let window_id = if win_reg < NUM_REGS {
                    self.regs[win_reg]
                } else {
                    0
                };
                const MAX_BG_VMS: usize = 4;
                if config_reg >= NUM_REGS {
                    self.regs[0] = 0xFFFFFFFF;
                } else if self.background_vms.len() >= MAX_BG_VMS {
                    self.regs[0] = 0xFFFFFFFE; // max VMs reached
                } else {
                    let addr = self.regs[config_reg] as usize;
                    let config = Self::read_string_static(&self.ram, addr);
                    match config {
                        Some(cfg) => {
                            let has_arch = cfg
                                .split_whitespace()
                                .any(|t| t.to_lowercase().starts_with("arch=") && t.len() > 5);
                            if !has_arch {
                                self.regs[0] = 0xFFFFFFFD; // missing arch=
                            } else {
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
                                let id = self.next_bg_vm_id;
                                self.next_bg_vm_id += 1;
                                let bg_vm = BackgroundVm {
                                    id,
                                    config: cfg,
                                    mode,
                                    window_id,
                                    state: BgVmState::Paused,
                                    instructions_per_frame: 1000,
                                    total_instructions: 0,
                                    frames_active: 0,
                                };
                                self.background_vms.push(bg_vm);
                                self.regs[0] = id; // success
                            }
                        }
                        None => {
                            self.regs[0] = 0xFFFFFFFF; // empty/null config
                        }
                    }
                }
            }

            // VM_KILL id_reg (0xA0) -- Kill a background VM by ID.
            // Returns 0 in r0 on success, 0xFFFFFFFF if not found.
            // Encoding: 2 words [0xA0, id_reg]
            0xA0 => {
                let id_reg = self.fetch() as usize;
                if id_reg < NUM_REGS {
                    let vm_id = self.regs[id_reg];
                    let before = self.background_vms.len();
                    self.background_vms.retain(|v| v.id != vm_id);
                    if self.background_vms.len() < before {
                        self.regs[0] = 0; // success
                    } else {
                        self.regs[0] = 0xFFFFFFFF; // not found
                    }
                } else {
                    self.regs[0] = 0xFFFFFFFF;
                }
            }

            // VM_STATUS id_reg (0xA1) -- Query background VM status.
            // Returns in r0: 0=not found, 1=Running, 2=Paused, 3=Saved.
            // Also writes total_instructions to RAM at address in r1 (if r1 != 0).
            // Encoding: 2 words [0xA1, id_reg]
            0xA1 => {
                let id_reg = self.fetch() as usize;
                if id_reg < NUM_REGS {
                    let vm_id = self.regs[id_reg];
                    match self.background_vms.iter().find(|v| v.id == vm_id) {
                        Some(bg) => {
                            self.regs[0] = match bg.state {
                                BgVmState::Running => 1,
                                BgVmState::Paused => 2,
                                BgVmState::Saved => 3,
                            };
                            // Also write stats to r1 if it points to a valid RAM region
                            if NUM_REGS > 1 {
                                let stats_addr = self.regs[1] as usize;
                                if stats_addr > 0 && stats_addr + 1 < self.ram.len() {
                                    self.ram[stats_addr] = bg.total_instructions as u32;
                                    self.ram[stats_addr + 1] = bg.frames_active as u32;
                                }
                            }
                        }
                        None => {
                            self.regs[0] = 0; // not found
                        }
                    }
                } else {
                    self.regs[0] = 0;
                }
            }

            // VM_PAUSE id_reg (0xA2) -- Pause a running background VM.
            // Returns 0 on success, 0xFFFFFFFF if not found or already paused.
            // Encoding: 2 words [0xA2, id_reg]
            0xA2 => {
                let id_reg = self.fetch() as usize;
                if id_reg < NUM_REGS {
                    let vm_id = self.regs[id_reg];
                    match self.background_vms.iter_mut().find(|v| v.id == vm_id) {
                        Some(bg) => {
                            if bg.state == BgVmState::Running {
                                bg.state = BgVmState::Paused;
                                self.regs[0] = 0;
                            } else {
                                self.regs[0] = 0xFFFFFFFE; // wrong state
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

            // VM_RESUME id_reg (0xA3) -- Resume a paused/saved background VM.
            // Returns 0 on success, 0xFFFFFFFF if not found or already running.
            // Encoding: 2 words [0xA3, id_reg]
            0xA3 => {
                let id_reg = self.fetch() as usize;
                if id_reg < NUM_REGS {
                    let vm_id = self.regs[id_reg];
                    match self.background_vms.iter_mut().find(|v| v.id == vm_id) {
                        Some(bg) => {
                            if bg.state != BgVmState::Running {
                                bg.state = BgVmState::Running;
                                self.regs[0] = 0;
                            } else {
                                self.regs[0] = 0xFFFFFFFE; // already running
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

            // VM_SET_BUDGET id_reg, budget_reg (0xA4) -- Set instructions-per-frame budget.
            // budget_reg holds the new instruction budget (must be > 0).
            // Returns 0 on success, 0xFFFFFFFF if not found, 0xFFFFFFFE if budget == 0.
            // Encoding: 3 words [0xA4, id_reg, budget_reg]
            0xA4 => {
                let id_reg = self.fetch() as usize;
                let budget_reg = self.fetch() as usize;
                if id_reg < NUM_REGS && budget_reg < NUM_REGS {
                    let vm_id = self.regs[id_reg];
                    let budget = self.regs[budget_reg];
                    match self.background_vms.iter_mut().find(|v| v.id == vm_id) {
                        Some(bg) => {
                            if budget == 0 {
                                self.regs[0] = 0xFFFFFFFE;
                            } else {
                                bg.instructions_per_frame = budget;
                                self.regs[0] = 0;
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

            // VM_LIST addr_reg (0xA5) -- List all background VM IDs to RAM.
            // Writes up to 4 VM IDs starting at RAM address in addr_reg.
            // Returns count of VMs in r0.
            // Encoding: 2 words [0xA5, addr_reg]
            0xA5 => {
                let addr_reg = self.fetch() as usize;
                if addr_reg < NUM_REGS {
                    let base_addr = self.regs[addr_reg] as usize;
                    let count = self.background_vms.len().min(4);
                    for (i, bg) in self.background_vms.iter().take(4).enumerate() {
                        if base_addr + i < self.ram.len() {
                            self.ram[base_addr + i] = bg.id;
                        }
                    }
                    self.regs[0] = count as u32;
                } else {
                    self.regs[0] = 0;
                }
            }

            // ── Phase 89: AI Agent Input ──

            // AI_INJECT op_reg (0xA6) -- AI programmatic input injection
            // op=0: inject key event. r1=keycode, r2=shift_state. Calls push_key().
            // op=1: inject mouse move. r1=x, r2=y. Calls push_mouse().
            // op=2: inject mouse click. r1=x, r2=y, r3=button. Calls push_mouse() + push_mouse_button().
            // op=3: inject text string. r1=addr of null-terminated string, pushes each char via push_key().
            // Returns: r0=1 on success, 0 on failure.
            // Encoding: 2 words [0xA6, op_reg]
            0xA6 => {
                let op_reg = self.fetch() as usize;
                if op_reg >= NUM_REGS {
                    self.regs[0] = 0; // invalid register
                } else {
                    let op = self.regs[op_reg];
                    match op {
                        // op=0: inject key event
                        0 => {
                            if op_reg + 2 < NUM_REGS {
                                let keycode = self.regs[op_reg + 1];
                                let _shift = self.regs[op_reg + 2];
                                let ok = self.push_key(keycode);
                                self.regs[0] = if ok { 1 } else { 0 };
                            } else {
                                self.regs[0] = 0;
                            }
                        }
                        // op=1: inject mouse move
                        1 => {
                            if op_reg + 2 < NUM_REGS {
                                let x = self.regs[op_reg + 1];
                                let y = self.regs[op_reg + 2];
                                self.push_mouse(x, y);
                                self.regs[0] = 1;
                            } else {
                                self.regs[0] = 0;
                            }
                        }
                        // op=2: inject mouse click
                        2 => {
                            if op_reg + 3 < NUM_REGS {
                                let x = self.regs[op_reg + 1];
                                let y = self.regs[op_reg + 2];
                                let button = self.regs[op_reg + 3];
                                self.push_mouse(x, y);
                                self.push_mouse_button(button);
                                self.regs[0] = 1;
                            } else {
                                self.regs[0] = 0;
                            }
                        }
                        // op=3: inject text string (null-terminated in RAM)
                        3 => {
                            if op_reg + 1 < NUM_REGS {
                                let mut addr = self.regs[op_reg + 1] as usize;
                                let mut count = 0u32;
                                // Push each character as a key event
                                while addr < self.ram.len() {
                                    let ch = self.ram[addr];
                                    if ch == 0 {
                                        break;
                                    }
                                    if !self.push_key(ch) {
                                        break;
                                    } // buffer full
                                    count += 1;
                                    addr += 1;
                                }
                                self.regs[0] = count;
                            } else {
                                self.regs[0] = 0;
                            }
                        }
                        _ => {
                            self.regs[0] = 0; // unknown op
                        }
                    }
                }
            }

            // ── Phase 102: Permissions and Capability System ──

            // SPAWNC addr_reg, caps_reg (0xA7) -- Spawn with capabilities.
            // Like SPAWN but parent passes a capability list from RAM.
            // RAM layout at caps_reg:
            //   [n_entries, entry_0, entry_1, ..., entry_n-1, 0xFFFFFFFF sentinel]
            //   Each entry: [resource_type, pattern_addr, pattern_len, permissions]
            //   where pattern is a null-terminated string at pattern_addr (pattern_len chars).
            //   resource_type: 0 = VFS path, 1 = opcode restriction.
            //   permissions bitmask: bit 0 = read, bit 1 = write, bit 2 = exec.
            // Returns child PID in RAM[0xFFA], or 0xFFFFFFFF on error.
            // Encoding: 3 words [0xA7, addr_reg, caps_reg]
            0xA7 => {
                let addr_reg = self.fetch() as usize;
                let caps_reg = self.fetch() as usize;
                if addr_reg < NUM_REGS && caps_reg < NUM_REGS {
                    let active_count = self.processes.iter().filter(|p| !p.is_halted()).count();
                    if active_count >= MAX_PROCESSES {
                        self.ram[0xFFA] = 0xFFFFFFFF;
                    } else {
                        let start_addr = self.regs[addr_reg];
                        let caps_addr = self.regs[caps_reg] as usize;

                        // Parse capability list from RAM
                        let mut capabilities = Vec::new();
                        if caps_addr < self.ram.len() {
                            let n_entries = self.ram[caps_addr] as usize;
                            let mut entry_offset = caps_addr + 1;
                            for _ in 0..n_entries {
                                if entry_offset + 3 >= self.ram.len() {
                                    break;
                                }
                                let res_type = self.ram[entry_offset] as u8;
                                let pat_addr = self.ram[entry_offset + 1] as usize;
                                let pat_len = self.ram[entry_offset + 2] as usize;
                                let perms = self.ram[entry_offset + 3] as u8;

                                // Read pattern string from RAM
                                let mut pattern = String::new();
                                for i in 0..pat_len {
                                    if pat_addr + i >= self.ram.len() {
                                        break;
                                    }
                                    let ch = self.ram[pat_addr + i];
                                    if ch == 0 {
                                        break;
                                    }
                                    if let Some(c) = char::from_u32(ch) {
                                        pattern.push(c);
                                    }
                                }

                                capabilities.push(crate::vm::types::Capability {
                                    resource_type: res_type,
                                    pattern,
                                    permissions: perms,
                                });
                                entry_offset += 4;
                            }
                        }

                        // Same page directory logic as SPAWN
                        let start_page = (start_addr as usize) / PAGE_SIZE;
                        let page_offset = start_addr % (PAGE_SIZE as u32);
                        let mut pd = vec![PAGE_UNMAPPED; NUM_PAGES];
                        let child_pc: u32;
                        let identity_map = start_page < 3;

                        if identity_map {
                            for (phys_page, pd_entry) in pd.iter_mut().enumerate().take(3) {
                                if phys_page >= NUM_RAM_PAGES {
                                    break;
                                }
                                *pd_entry = phys_page as u32;
                                if self.page_ref_count[phys_page] == 0 {
                                    self.page_ref_count[phys_page] = 1;
                                }
                                self.page_ref_count[phys_page] += 1;
                                self.page_cow |= 1u64 << phys_page;
                            }
                            child_pc = start_addr;
                        } else {
                            for (vpage, pd_entry) in pd.iter_mut().enumerate().take(PROCESS_PAGES) {
                                let parent_phys = start_page + vpage;
                                if parent_phys >= NUM_RAM_PAGES {
                                    break;
                                }
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

                        // Page 3 shared region
                        if identity_map {
                            pd[3] = 3;
                            if self.page_ref_count[3] == 0 {
                                self.page_ref_count[3] = 1;
                            }
                            self.page_ref_count[3] += 1;
                        }

                        // Page 63 (hardware ports)
                        pd[63] = 63;

                        let pid = (self.processes.len() + 1) as u32;
                        self.processes.push(crate::vm::types::SpawnedProcess {
                            pc: child_pc,
                            regs: [0; NUM_REGS],
                            state: crate::vm::types::ProcessState::Ready,
                            pid,
                            mode: crate::vm::types::CpuMode::User,
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
                            vmas: crate::vm::types::Process::default_vmas_for_process(),
                            brk_pos: PAGE_SIZE as u32,
                            custom_font: None,
                            capabilities: if capabilities.is_empty() {
                                None
                            } else {
                                Some(capabilities)
                            },
                        });
                        self.ram[0xFFA] = pid;
                    }
                } else {
                    self.ram[0xFFA] = 0xFFFFFFFF;
                }
            }

            // ── Phase 88: AI Vision Bridge ──

            // AI_AGENT op_reg (0xB0) -- AI vision operations
            // op=0: screenshot to VFS file. r1=path_addr. Returns fd in r0.
            // op=1: canvas checksum. Returns FNV-1a hash in r0.
            // op=2: diff two screens. r1=addr of saved checksum (u32). Returns changed pixel count in r0.
            // op=3: call external vision API with screenshot + prompt from RAM.
            //       r1=prompt_addr, r2=response_addr, r3=max_len. Returns response length in r0.
            0xB0 => {
                let op_reg = self.fetch() as usize;
                if op_reg >= NUM_REGS {
                    self.regs[0] = 0xFFFFFFFF;
                } else {
                    let op = self.regs[op_reg];
                    match op {
                        0 => {
                            // Screenshot to VFS as PNG
                            // Read path from r1
                            if op_reg + 1 < NUM_REGS {
                                let path_addr = self.regs[op_reg + 1] as usize;
                                let pid = self.current_pid;

                                // Encode screen as PNG
                                let png_bytes = crate::vision::encode_png(&self.screen);

                                // Write PNG to VFS file
                                // First, create the file
                                let fd = self.vfs.fopen(&self.ram, path_addr as u32, 1, pid); // FOPEN_WRITE
                                if fd != 0xFFFFFFFF {
                                    // Stage PNG bytes in RAM at a temporary area, write in chunks
                                    let stage_base = 0x9000u32;
                                    let chunk_size = 512u32;
                                    let mut written: u32 = 0;
                                    let total_bytes = png_bytes.len() as u32;
                                    let mut offset = 0u32;
                                    while offset < total_bytes {
                                        let end = std::cmp::min(offset + chunk_size, total_bytes);
                                        let n = end - offset;
                                        // Stage bytes into RAM as u32 words (4 bytes per word)
                                        let mut stage_idx = 0u32;
                                        while stage_idx < n {
                                            let byte_off = offset + stage_idx;
                                            let b0 = if (byte_off as usize) < png_bytes.len() {
                                                png_bytes[byte_off as usize]
                                            } else {
                                                0u8
                                            };
                                            let b1 = if (byte_off as usize) + 1 < png_bytes.len() {
                                                png_bytes[byte_off as usize + 1]
                                            } else {
                                                0u8
                                            };
                                            let b2 = if (byte_off as usize) + 2 < png_bytes.len() {
                                                png_bytes[byte_off as usize + 2]
                                            } else {
                                                0u8
                                            };
                                            let b3 = if (byte_off as usize) + 3 < png_bytes.len() {
                                                png_bytes[byte_off as usize + 3]
                                            } else {
                                                0u8
                                            };
                                            let word = (b0 as u32)
                                                | ((b1 as u32) << 8)
                                                | ((b2 as u32) << 16)
                                                | ((b3 as u32) << 24);
                                            let ram_addr = (stage_base + stage_idx / 4) as usize;
                                            if ram_addr < self.ram.len() {
                                                self.ram[ram_addr] = word;
                                            }
                                            stage_idx += 4;
                                        }
                                        let bytes_written = self.vfs.fwrite(
                                            &self.ram,
                                            fd,
                                            stage_base,
                                            (n + 3) / 4,
                                            pid,
                                        );
                                        written += bytes_written;
                                        offset = end;
                                    }
                                    self.vfs.fclose(fd, pid);
                                    self.regs[0] = written; // total bytes written
                                } else {
                                    self.regs[0] = 0xFFFFFFFF; // error
                                }
                            } else {
                                self.regs[0] = 0xFFFFFFFF;
                            }
                        }
                        1 => {
                            // Canvas checksum (FNV-1a)
                            let hash = crate::vision::canvas_checksum(&self.screen);
                            self.regs[0] = hash;
                        }
                        2 => {
                            // Diff: compare current screen against saved checksum in RAM[r1]
                            // Returns count of pixels that differ from expected pattern
                            // (Since we can't store a full screen, this returns a simple
                            // changed-pixel count vs the last saved checksum metadata)
                            // For now: compute current checksum and return pixel diff stats
                            // r1 = addr of previous screen data in RAM (256x256 u32 words starting at addr)
                            // Returns count of changed pixels in r0
                            if op_reg + 1 < NUM_REGS {
                                let prev_addr = self.regs[op_reg + 1] as usize;
                                let mut changed: u32 = 0;
                                for i in 0..256 * 256 {
                                    let prev_pixel = if prev_addr + i < self.ram.len() {
                                        self.ram[prev_addr + i]
                                    } else {
                                        0
                                    };
                                    if self.screen[i] != prev_pixel {
                                        changed += 1;
                                    }
                                }
                                self.regs[0] = changed;
                            } else {
                                self.regs[0] = 0xFFFFFFFF;
                            }
                        }
                        3 => {
                            // Vision API call: screenshot + prompt -> LLM response
                            // r1=prompt_addr (null-terminated string in RAM)
                            // r2=response_addr (where to write response in RAM)
                            // r3=max_len (max response words)
                            // Returns response length in r0, or 0xFFFFFFFF on error
                            if op_reg + 3 < NUM_REGS {
                                let prompt_addr = self.regs[op_reg + 1] as usize;
                                let _response_addr = self.regs[op_reg + 2] as usize;
                                let max_len = self.regs[op_reg + 3] as usize;

                                // Read prompt from RAM
                                let mut prompt = String::new();
                                let mut pa = prompt_addr;
                                while pa < self.ram.len() {
                                    let ch = self.ram[pa];
                                    if ch == 0 {
                                        break;
                                    }
                                    if let Some(c) = char::from_u32(ch) {
                                        prompt.push(c);
                                    }
                                    pa += 1;
                                }

                                // Encode screenshot as base64 PNG
                                let _screenshot_b64 =
                                    crate::vision::encode_png_base64(&self.screen);

                                // Check for mock response (testing)
                                if let Some(ref mock) = self.llm_mock_response {
                                    let response = mock.clone();
                                    let resp_bytes = response.as_bytes();
                                    let write_len = resp_bytes.len().min(max_len);
                                    let wa = _response_addr;
                                    for i in 0..write_len {
                                        if wa + i < self.ram.len() {
                                            self.ram[wa + i] = resp_bytes[i] as u32;
                                        }
                                    }
                                    self.regs[0] = write_len as u32;
                                    self.llm_mock_response = None;
                                } else {
                                    // No mock set -- would call external API
                                    // For now, return error (API not available in VM)
                                    self.regs[0] = 0xFFFFFFFF;
                                }
                            } else {
                                self.regs[0] = 0xFFFFFFFF;
                            }
                        }
                        _ => {
                            self.regs[0] = 0xFFFFFFFF; // unknown op
                        }
                    }
                }
            }

            // LOADPNG path_reg, dest_addr_reg (0xB1) -- Load pixelpack-encoded PNG to RAM
            // Reads a PNG file path from RAM at path_reg, decodes pixelpack seeds to bytes,
            // writes bytecode to RAM starting at dest_addr_reg.
            // Returns byte count in r0 (0xFFFFFFFF on error).
            // Encoding: 3 words [0xB1, path_reg, dest_addr_reg]
            0xB1 => {
                let path_reg = self.fetch() as usize;
                let dest_reg = self.fetch() as usize;
                if path_reg >= NUM_REGS || dest_reg >= NUM_REGS {
                    self.regs[0] = 0xFFFFFFFF;
                } else {
                    let path_addr = self.regs[path_reg] as usize;
                    let dest_addr = self.regs[dest_reg] as usize;

                    // Read path string from RAM (null-terminated)
                    let mut path_str = String::new();
                    let mut pa = path_addr;
                    while pa < self.ram.len() {
                        let ch = self.ram[pa];
                        if ch == 0 {
                            break;
                        }
                        if let Some(c) = char::from_u32(ch) {
                            path_str.push(c);
                        }
                        pa += 1;
                    }

                    if path_str.is_empty() {
                        self.regs[0] = 0xFFFFFFFF;
                    } else {
                        // Try to decode as pixelpack PNG
                        match crate::pixel::decode_pixelpack_file(&path_str) {
                            Ok(bytes) => {
                                let byte_count = bytes.len();
                                let words = crate::pixel::load_bytecode_to_ram(
                                    &bytes,
                                    &mut self.ram,
                                    dest_addr,
                                );
                                self.regs[0] = byte_count as u32;
                                let _ = words; // words written (for debugging)
                            }
                            Err(_) => {
                                self.regs[0] = 0xFFFFFFFF;
                            }
                        }
                    }
                }
            }

            // LOADSRCIMG path_reg (0xB2) -- Load pixelpack-encoded source PNG to canvas
            // Reads a PNG file path from RAM at path_reg, decodes pixelpack seeds as UTF-8 source,
            // writes the source text onto the canvas buffer (128x32 grid),
            // then assembles it to bytecode at 0x1000.
            // Returns bytecode word count in r0 (0xFFFFFFFF on error).
            // Encoding: 2 words [0xB2, path_reg]
            0xB2 => {
                let path_reg = self.fetch() as usize;
                if path_reg >= NUM_REGS {
                    self.regs[0] = 0xFFFFFFFF;
                } else {
                    let path_addr = self.regs[path_reg] as usize;

                    // Read path string from RAM (null-terminated)
                    let mut path_str = String::new();
                    let mut pa = path_addr;
                    while pa < self.ram.len() {
                        let ch = self.ram[pa];
                        if ch == 0 {
                            break;
                        }
                        if let Some(c) = char::from_u32(ch) {
                            path_str.push(c);
                        }
                        pa += 1;
                    }

                    if path_str.is_empty() {
                        self.regs[0] = 0xFFFFFFFF;
                    } else {
                        // Decode as source text
                        match crate::pixel::decode_pixelpack_source_file(&path_str) {
                            Ok(source) => {
                                // Write source to canvas buffer (128x32 grid)
                                crate::pixel::load_source_to_canvas_buffer(
                                    &source,
                                    &mut self.canvas_buffer,
                                );

                                // Assemble the source to bytecode at 0x1000
                                match crate::assembler::assemble(&source, 0x1000) {
                                    Ok(asm_result) => {
                                        let word_count = asm_result.pixels.len().min(4096);
                                        for i in 0..word_count {
                                            self.ram[0x1000 + i] = asm_result.pixels[i];
                                        }
                                        self.regs[0] = word_count as u32;
                                    }
                                    Err(_) => {
                                        self.regs[0] = 0xFFFFFFFF;
                                    }
                                }
                            }
                            Err(_) => {
                                self.regs[0] = 0xFFFFFFFF;
                            }
                        }
                    }
                }
            }

            // ASM_RAM src_addr_reg (0xB3) -- Assemble source string from RAM.
            // Reads null-terminated assembly source from RAM at the address in src_addr_reg.
            // Runs preprocessor + assembler, writes bytecode to 0x1000.
            // RAM[0xFFD] = bytecode word count on success, 0xFFFFFFFF on error.
            // r0 = same value as RAM[0xFFD].
            // Encoding: 2 words [0xB3, src_addr_reg]
            0xB3 => {
                let src_reg = self.fetch() as usize;
                if src_reg < NUM_REGS {
                    let addr = self.regs[src_reg] as usize;
                    // Read null-terminated string from RAM
                    let mut source = String::new();
                    let mut a = addr;
                    while a < self.ram.len() {
                        let ch = self.ram[a];
                        if ch == 0 {
                            break;
                        }
                        if let Some(c) = char::from_u32(ch) {
                            source.push(c);
                        } else {
                            source.push('?');
                        }
                        a += 1;
                    }

                    // Strip ```asm / ``` fences if present (AI responses)
                    let source = source.replace("```asm", "").replace("```", "");

                    // Run preprocessor then assembler
                    let mut pp = crate::preprocessor::Preprocessor::new();
                    let preprocessed = pp.preprocess(&source);

                    match crate::assembler::assemble(&preprocessed, 0x1000) {
                        Ok(asm_result) => {
                            // Clear bytecode region
                            let end = (0x1000 + 4096).min(self.ram.len());
                            for baddr in 0x1000..end {
                                self.ram[baddr] = 0;
                            }
                            // Write bytecode
                            let word_count = asm_result.pixels.len().min(4096);
                            for (i, &word) in asm_result.pixels.iter().enumerate().take(word_count)
                            {
                                let dest = 0x1000 + i;
                                if dest < self.ram.len() {
                                    self.ram[dest] = word;
                                }
                            }
                            self.ram[0xFFD] = word_count as u32;
                            self.regs[0] = word_count as u32;
                        }
                        Err(_) => {
                            self.ram[0xFFD] = 0xFFFFFFFF;
                            self.regs[0] = 0xFFFFFFFF;
                        }
                    }
                } else {
                    self.ram[0xFFD] = 0xFFFFFFFF;
                    self.regs[0] = 0xFFFFFFFF;
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
    /// Phase 107: World-space windows (is_world_space() == true) are skipped here
    /// and composited separately by the host renderer via blit_world_windows().
    pub fn blit_windows(&mut self) {
        // Collect (id, x, y, w, h, z_order) for active SCREEN-SPACE windows, sorted by z_order ascending
        let mut wins: Vec<(u32, u32, u32, u32, u32, u32)> = self
            .windows
            .iter()
            .filter(|w| w.active && !w.is_world_space())
            .map(|w| (w.id, w.x, w.y, w.w, w.h, w.z_order))
            .collect();
        wins.sort_by_key(|w| w.5); // sort by z_order ascending (lowest first)

        for (win_id, _wx, _wy, _ww, wh, _z) in wins {
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

    /// Call an external LLM API via curl. Returns the response text or None on error.
    /// Uses provider.json config (same as hermes agent) or falls back to local Ollama.
    /// The prompt is sent as a user message with a minimal system prompt.
    fn call_llm_external(&self, prompt: &str) -> Option<String> {
        // Use model_choice library for smart provider routing, fallback, and
        // Ollama lifecycle management. Falls back to raw curl if model_choice
        // is unavailable.
        //
        // model_choice handles: provider selection (Ollama/ZAI/cloud), auto-start
        // Ollama, rate limiting, response caching, and fallback chains.
        //
        // We call it via Python subprocess since Geometry OS is Rust.
        // The prompt and system context are written to temp files.

        let prompt_path = "/tmp/geo_llm_prompt.txt";
        let system_path = "/tmp/geo_llm_system.txt";
        if std::fs::write(prompt_path, prompt).is_err() {
            return None;
        }

        // Build world-aware system prompt from VM state
        let system = self.build_llm_system_prompt();
        if std::fs::write(system_path, &system).is_err() {
            return None;
        }

        // Python script: load system + user prompt, call generate with system prompt
        let py_code = "\
import sys\n\
from model_choice import generate\n\
p = open('/tmp/geo_llm_prompt.txt').read()\n\
s = open('/tmp/geo_llm_system.txt').read()\n\
r = generate(p, complexity='balanced', system=s)\n\
print(r if r else '')";

        let output = match std::process::Command::new("python3")
            .args(["-c", py_code])
            .env("MODEL_CHOICE_TEMPLATE", "agent")
            .output()
        {
            Ok(o) => o,
            Err(_) => {
                return self.call_llm_curl(prompt);
            }
        };

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let result = stdout.trim().to_string();

        if result.is_empty() {
            return self.call_llm_curl(prompt);
        }

        let cleaned = strip_think_blocks(&result);
        Some(cleaned)
    }

    /// Build a world-aware system prompt from current VM state.
    /// Reads player position, camera, biome, and nearby buildings from RAM.
    fn build_llm_system_prompt(&self) -> String {
        // Prompt mode hint: RAM[0x7820]. 0 = oracle (world guide, default),
        // 1 = asm_dev (assembly pair programmer). Apps that want developer-mode
        // responses write 1 to this slot before issuing the LLM opcode.
        if self.ram.get(0x7820).copied().unwrap_or(0) == 1 {
            return self.asm_dev_system_prompt();
        }

        let px = self.ram.get(0x7808).copied().unwrap_or(0);
        let py = self.ram.get(0x7809).copied().unwrap_or(0);
        let cam_x = self.ram.get(0x7800).copied().unwrap_or(0);
        let cam_y = self.ram.get(0x7801).copied().unwrap_or(0);
        let zoom = self.ram.get(0x7812).copied().unwrap_or(2);
        let frame = self.ram.get(0x7802).copied().unwrap_or(0);

        // Biome lookup: same mapping as world_desktop.asm color table at RAM[0x7000]
        let biome_names = [
            "Deep Ocean",
            "Ocean",
            "Beach",
            "Desert",
            "Sandy Desert",
            "Oasis",
            "Grassland",
            "Dark Grass",
            "Swamp",
            "Deep Swamp",
            "Forest",
            "Dense Forest",
            "Mushroom Grove",
            "Mountain",
            "Snowy Peak",
            "Tundra",
            "Lava Field",
            "Cooled Lava",
            "Volcanic Rock",
            "Snowfield",
            "Packed Snow",
            "Fresh Snow",
            "Coral Reef",
            "Ancient Ruins",
            "Crystal Cave",
            "Deep Crystal",
            "Ash Wastes",
            "Deadlands",
            "Barren Deadlands",
            "Mystic Grove",
            "Unknown",
            "Unknown",
        ];
        let biome_idx =
            (px.wrapping_add(py.wrapping_mul(79007)).wrapping_mul(12345) >> 27) as usize;
        let biome_name = biome_names.get(biome_idx.min(31)).unwrap_or(&"Unknown");

        // Building list
        let bldg_count = self.ram.get(0x7580).copied().unwrap_or(0).min(32);
        let mut buildings = Vec::new();
        for i in 0..bldg_count as usize {
            let base = 0x7500 + i * 4;
            let bx = self.ram.get(base).copied().unwrap_or(0);
            let by = self.ram.get(base + 1).copied().unwrap_or(0);
            let name_addr = self.ram.get(base + 3).copied().unwrap_or(0) as usize;
            let mut name = String::new();
            for j in 0..12 {
                if name_addr + j >= self.ram.len() {
                    break;
                }
                let ch = self.ram[name_addr + j];
                if ch == 0 || ch > 127 {
                    break;
                }
                name.push(ch as u8 as char);
            }
            if !name.is_empty() {
                let dist = ((bx as i32 - px as i32).abs() + (by as i32 - py as i32).abs()) as u32;
                buildings.push(format!("{} ({},{}) dist={}", name, bx, by, dist));
            }
        }

        let mut sys = format!(
            "You are the Oracle of Geometry OS, an AI guide inside a procedurally generated infinite world. \
             You speak concisely (1-3 sentences). You have spatial awareness of the world.\n\n\
             Current world state:\n\
             - Player position: ({}, {})\n\
             - Camera: ({}, {}) zoom={}\n\
             - Frame: {}\n\
             - Player biome: {} (index {})\n",
            px, py, cam_x, cam_y, zoom, frame, biome_name, biome_idx
        );

        if !buildings.is_empty() {
            sys.push_str("- Buildings on map:\n");
            for b in &buildings {
                sys.push_str(&format!("  - {}\n", b));
            }
        }

        // Check if player is near a building
        let nearby_flag = self.ram.get(0x7588).copied().unwrap_or(0);
        if nearby_flag == 1 {
            let nearby_idx = self.ram.get(0x7584).copied().unwrap_or(0) as usize;
            let base = 0x7500 + nearby_idx * 4;
            let name_addr = self.ram.get(base + 3).copied().unwrap_or(0) as usize;
            let mut name = String::new();
            for j in 0..12 {
                if name_addr + j >= self.ram.len() {
                    break;
                }
                let ch = self.ram[name_addr + j];
                if ch == 0 || ch > 127 {
                    break;
                }
                name.push(ch as u8 as char);
            }
            if !name.is_empty() {
                sys.push_str(&format!(
                    "- Player is currently near the {} building\n",
                    name
                ));
            }
        }

        sys.push_str("\nAnswer questions about the world, suggest where to explore, describe the terrain, or help with anything the player asks.");
        sys
    }

    /// System prompt for assembly pair-programming mode (RAM[0x7820] == 1).
    /// Target audience: a user inside the AI Terminal asking for Geometry OS
    /// assembly code. Kept tight (~2KB) — enough opcode reference to produce
    /// runnable programs without blowing context.
    fn asm_dev_system_prompt(&self) -> String {
        let frame = self.ram.get(0x7802).copied().unwrap_or(0);
        let opcode_inventory = build_opcode_inventory_line();
        let focus = self.ram.get(0x7821).copied().unwrap_or(0);
        let focus_hint = if focus != 0 && focus <= 0xFF {
            let name = crate::vm::disasm::valid_opcode_mnemonics()
                .into_iter()
                .find(|(op, _)| *op as u32 == focus)
                .map(|(_, n)| n)
                .unwrap_or_else(|| "???".to_string());
            format!(
                "\n# Focus opcode\nThe user is debugging opcode 0x{:02X} ({}). Prefer programs that exercise this opcode at boundary conditions and write pass/fail markers to RAM so the caller can inspect results.\n",
                focus, name
            )
        } else {
            String::new()
        };
        let asm_status = self.ram.get(0xFFD).copied().unwrap_or(0);
        let asm_status_line = if asm_status != 0 {
            let note = if asm_status == 0xFFFFFFFF {
                "assembly FAILED"
            } else {
                "assembly succeeded"
            };
            format!(
                "\n# Last assemble result (RAM[0xFFD])\n0x{:08X} ({}). Use this when the user asks why their last generation didn't run.\n",
                asm_status, note
            )
        } else {
            String::new()
        };
        format!(
            "You are an assembly pair programmer for Geometry OS, a custom 32-bit VM.\n\
             Output ONLY code inside a fenced ```asm block unless the user explicitly asks for prose.\n\
             Assume the user will paste your code straight into the assembler — no placeholders, no TODOs.\n\
             \n\
             # VM model\n\
             - 32 registers r0..r31. r30 = stack pointer. r31 = call return.\n\
             - r27..r29 are reserved for preprocessor macros — do not use.\n\
             - RAM: 65536 u32 words. Screen: 256x256 u32 (ARGB).\n\
             - Programs load at 0x1000 by default.\n\
             \n\
             # RAM conventions\n\
             - 0x0000-0x0FFF : reserved / zero page\n\
             - 0x1000-0x3FFF : program bytecode (default .org)\n\
             - 0x4000-0x7EFF : free scratch / app data\n\
             - 0x7500-0x758F : building table. Each entry = [world_x, world_y, type_color, name_addr], 4 words. Count at 0x7580, max 32.\n\
             - 0x7600-0x76CF : building name strings (16 chars each). 0x76D0+ for dynamic buildings.\n\
             - 0x7800-0x782F : world/VM state (cam, frame, player, prompt-mode)\n\
             - 0x7808        : player_x (tile coords). Write to teleport.\n\
             - 0x7809        : player_y (tile coords). Write to teleport.\n\
             - 0x780A        : player_facing (0=down,1=up,2=left,3=right)\n\
             - 0x7820        : prompt mode (0=oracle, 1=asm_dev). Already set by AI Terminal.\n\
             - 0xF00-0xF03   : window bounds protocol (x,y,w,h)\n\
             - 0xFFA-0xFFF   : hardware ports (keyboard=0xFFF, ticks=0xFFE, multi-key=0xFFB)\n\
             \n\
             # Core instruction forms\n\
             LDI rD, imm            ; load 32-bit immediate\n\
             LOAD rD, rAddr         ; rD = RAM[rAddr]\n\
             STORE rAddr, rSrc      ; RAM[rAddr] = rSrc\n\
             MOV rD, rS\n\
             ADD/SUB/MUL/DIV rD, rS ; rD = rD op rS\n\
             ADDI/SUBI rD, imm      ; immediate forms\n\
             AND/OR/XOR/SHL/SHR rD, rS\n\
             CMP rA, rB             ; sets r0 flag: 0 eq, 1 lt, 2 gt\n\
             CMPI rA, imm\n\
             JMP label              ; unconditional\n\
             JZ rCond, label / JNZ rCond, label\n\
             BLT rFlag, label / BGE rFlag, label   ; after CMP\n\
             CALL label / RET       ; uses r31\n\
             PUSH rS / POP rD       ; r30 stack\n\
             \n\
             # Graphics\n\
             FILL rColor                                ; clear screen\n\
             PSET rX, rY, rColor / PSETI x, y, color\n\
             RECTF rX, rY, rW, rH, rColor               ; filled rect\n\
             LINE rX1, rY1, rX2, rY2, rColor\n\
             CIRCLE rCX, rCY, rRad, rColor\n\
             DRAWTEXT rX, rY, rStrAddr, rFg, rBg        ; null-terminated string in RAM\n\
             STRO rAddr, \"literal\"                    ; write string literal to RAM\n\
             \n\
             # Interaction\n\
             IKEY rD                 ; non-blocking key read (0 if none)\n\
             HITSET rX, rY, rW, rH, id / HITQ rD   ; mouse hit regions\n\
             FRAME                   ; end of frame, present buffer\n\
             RAND rD                 ; pseudorandom u32\n\
             \n\
             # Recursion (the useful bit for self-building)\n\
             LLM rPromptAddr, rRespAddr, rMaxLen ; call cloud AI, writes response to RAM\n\
             ASMSELF rSrcAddr, rDstAddr          ; assemble source string in RAM\n\
             RUNNEXT rAddr                       ; jump into assembled bytecode\n\
             VM_SPAWN rConfigAddr, rWindowId     ; launch a background VM\n\
             \n\
             # Program skeleton\n\
             LDI r1, 1\n\
             LDI r30, 0xFD00       ; stack\n\
             LDI r0, 0x101820\n\
             FILL r0\n\
             main_loop:\n\
                 FRAME\n\
                 IKEY r5\n\
                 CMPI r5, 0\n\
                 JZ r0, main_loop\n\
                 HALT\n\
             \n\
             # Valid opcodes (complete inventory — anything not here does not exist)\n\
             {}\n\
             {}{}\n\
             Current frame: {}. If the user's request is ambiguous, ask ONE clarifying question.\n\
             Prefer short, runnable programs. Use #define for constants at the top.\n\
             Never invent opcodes — if a name isn't in the inventory above, it doesn't exist. Use CALL into a helper label instead.",
            opcode_inventory, focus_hint, asm_status_line, frame
        )
    }

    /// Fallback: raw curl-based LLM call using provider.json config.
    /// Used when model_choice is unavailable.
    fn call_llm_curl(&self, prompt: &str) -> Option<String> {
        let (base_url, model, api_key) = self.load_llm_config();

        let esc_prompt = prompt
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
            .replace('\t', "\\t");

        let system_msg = "Reply with ONLY the requested information. No explanation, no thinking.";
        let esc_sys = system_msg
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
            .replace('\t', "\\t");

        let payload = format!(
            r#"{{"model":"{}","messages":[{{"role":"system","content":"{}"}},{{"role":"user","content":"{}"}}],"stream":false,"max_tokens":512,"temperature":0.3}}"#,
            model, esc_sys, esc_prompt
        );

        let tmp_path = "/tmp/geo_llm_payload.json";
        if std::fs::write(tmp_path, &payload).is_err() {
            return None;
        }

        let data_arg = format!("@{}", tmp_path);
        let mut curl_args: Vec<&str> = vec![
            "-s",
            "-X",
            "POST",
            &base_url,
            "-d",
            &data_arg,
            "-H",
            "Content-Type: application/json",
            "--max-time",
            "30",
        ];

        let auth_header;
        if !api_key.is_empty() {
            auth_header = format!("Authorization: Bearer {}", api_key);
            curl_args.push("-H");
            curl_args.push(&auth_header);
        }

        let output = match std::process::Command::new("curl").args(&curl_args).output() {
            Ok(o) => o,
            Err(_) => return None,
        };

        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.contains("\"error\"") {
            return None;
        }

        let mut extracted: Option<String> = None;
        for field in &["\"content\":\"", "\"reasoning_content\":\""] {
            if let Some(start) = stdout.find(field) {
                let content_start = start + field.len();
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
                if !result.is_empty() {
                    let cleaned = strip_think_blocks(&result);
                    let final_text = if field == &"\"reasoning_content\":\"" {
                        cleaned
                            .lines()
                            .filter(|l| !l.trim().is_empty())
                            .last()
                            .map(|s| s.to_string())
                            .unwrap_or(cleaned)
                    } else {
                        cleaned
                    };
                    if !final_text.is_empty() {
                        extracted = Some(final_text);
                        break;
                    }
                }
            }
        }
        extracted
    }

    /// Load LLM config from provider.json or self.llm_config override.
    /// Returns (base_url, model, api_key).
    fn load_llm_config(&self) -> (String, String, String) {
        // Check for runtime override first
        if let Some(ref cfg) = self.llm_config {
            let parts: Vec<&str> = cfg.splitn(3, ' ').collect();
            if parts.len() >= 2 {
                return (
                    parts[0].to_string(),
                    parts[1].to_string(),
                    parts.get(2).unwrap_or(&"").to_string(),
                );
            }
        }
        // Try loading provider.json
        let config_path =
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("provider.json");
        if config_path.exists() {
            if let Ok(contents) = std::fs::read_to_string(&config_path) {
                let base_url = extract_json_str(&contents, "base_url")
                    .unwrap_or_else(|| "http://localhost:11434/api/chat".to_string());
                let model = extract_json_str(&contents, "model")
                    .unwrap_or_else(|| "qwen3.5-tools".to_string());
                let api_key = extract_json_str(&contents, "api_key").unwrap_or_default();
                return (base_url, model, api_key);
            }
        }
        // Default to local Ollama
        (
            "http://localhost:11434/api/chat".to_string(),
            "qwen3.5-tools".to_string(),
            String::new(),
        )
    }

    /// Parse HTML into styled lines for the browser (Phase 82).
    /// Supports: p, br, h1-h3, b, i, a href, img src, hr, ul/li.
    /// Colors: h1=green, h2=yellow, h3=orange, body=white, links=cyan.
    /// Links are registered as hit_regions for click detection.
    fn parse_html_to_lines(
        &mut self,
        html: &str,
        max_lines: usize,
        dest_base: usize,
    ) -> Vec<crate::vm::types::StyledLine> {
        use crate::vm::types::{HtmlLink, StyledLine};

        const COLOR_H1: u32 = 0x00FF00;
        const COLOR_H2: u32 = 0xFFFF00;
        const COLOR_H3: u32 = 0xFF8800;
        const COLOR_BODY: u32 = 0xFFFFFF;
        const COLOR_LINK: u32 = 0x00FFFF;
        const COLOR_BOLD: u32 = 0xFFFFFF;
        const COLOR_ITALIC: u32 = 0xAAAAAA;
        const COLOR_HR: u32 = 0x666666;
        const CHARS_PER_LINE: usize = 32;

        let mut lines: Vec<StyledLine> = Vec::new();
        let mut links: Vec<HtmlLink> = Vec::new();
        let mut tag_stack: Vec<String> = Vec::new();
        let mut current_color = COLOR_BODY;
        let mut current_link_href: Option<String> = None;
        let mut _link_char_start: usize = 0;
        let mut line_chars: Vec<u32> = Vec::new();
        let mut line_color = COLOR_BODY;
        let mut pos = 0;
        let chars: Vec<char> = html.chars().collect();

        let flush_line =
            |lines: &mut Vec<StyledLine>, lc: &mut Vec<u32>, lcol: &mut u32, ccol: u32| {
                if !lc.is_empty() || lines.is_empty() {
                    lines.push(StyledLine {
                        fg_color: *lcol,
                        chars: lc.clone(),
                    });
                    lc.clear();
                    *lcol = ccol;
                }
            };

        while pos < chars.len() && lines.len() < max_lines {
            if chars[pos] == '<' {
                let tag_start = pos + 1;
                let mut tag_end = tag_start;
                while tag_end < chars.len() && chars[tag_end] != '>' {
                    tag_end += 1;
                }
                if tag_end >= chars.len() {
                    pos += 1;
                    continue;
                }

                let tag_content: String = chars[tag_start..tag_end].iter().collect();
                pos = tag_end + 1;
                let is_closing = tag_content.starts_with('/');
                let tag_text = if is_closing {
                    &tag_content[1..]
                } else {
                    &tag_content[..]
                };
                let tag_name = tag_text
                    .split_whitespace()
                    .next()
                    .unwrap_or("")
                    .to_lowercase();

                match tag_name.as_str() {
                    "br" | "br/" => {
                        lines.push(StyledLine {
                            fg_color: line_color,
                            chars: line_chars.clone(),
                        });
                        line_chars.clear();
                        line_color = current_color;
                    }
                    "p" => {
                        if is_closing {
                            lines.push(StyledLine {
                                fg_color: line_color,
                                chars: line_chars.clone(),
                            });
                            line_chars.clear();
                            if lines.len() < max_lines {
                                lines.push(StyledLine {
                                    fg_color: COLOR_BODY,
                                    chars: Vec::new(),
                                });
                            }
                            tag_stack.pop();
                            current_color = COLOR_BODY;
                            for t in &tag_stack {
                                match t.as_str() {
                                    "h1" => current_color = COLOR_H1,
                                    "h2" => current_color = COLOR_H2,
                                    "h3" => current_color = COLOR_H3,
                                    "a" => current_color = COLOR_LINK,
                                    "b" => current_color = COLOR_BOLD,
                                    "i" => current_color = COLOR_ITALIC,
                                    _ => {}
                                }
                            }
                            line_color = current_color;
                        } else {
                            tag_stack.push("p".to_string());
                            line_color = COLOR_BODY;
                            current_color = COLOR_BODY;
                        }
                    }
                    "h1" | "h2" | "h3" => {
                        if is_closing {
                            lines.push(StyledLine {
                                fg_color: line_color,
                                chars: line_chars.clone(),
                            });
                            line_chars.clear();
                            if lines.len() < max_lines {
                                lines.push(StyledLine {
                                    fg_color: COLOR_BODY,
                                    chars: Vec::new(),
                                });
                            }
                            tag_stack.pop();
                            current_color = COLOR_BODY;
                            line_color = COLOR_BODY;
                        } else {
                            flush_line(&mut lines, &mut line_chars, &mut line_color, current_color);
                            tag_stack.push(tag_name.clone());
                            current_color = match tag_name.as_str() {
                                "h1" => COLOR_H1,
                                "h2" => COLOR_H2,
                                "h3" => COLOR_H3,
                                _ => COLOR_BODY,
                            };
                            line_color = current_color;
                        }
                    }
                    "b" => {
                        if is_closing {
                            if !line_chars.is_empty() {
                                flush_line(
                                    &mut lines,
                                    &mut line_chars,
                                    &mut line_color,
                                    current_color,
                                );
                            }
                            tag_stack.retain(|t| t != "b");
                            current_color = COLOR_BODY;
                            for t in &tag_stack {
                                match t.as_str() {
                                    "h1" => current_color = COLOR_H1,
                                    "h2" => current_color = COLOR_H2,
                                    "h3" => current_color = COLOR_H3,
                                    "a" => current_color = COLOR_LINK,
                                    _ => {}
                                }
                            }
                        } else {
                            tag_stack.push("b".to_string());
                            current_color = COLOR_BOLD;
                        }
                        line_color = current_color;
                    }
                    "i" => {
                        if is_closing {
                            if !line_chars.is_empty() {
                                flush_line(
                                    &mut lines,
                                    &mut line_chars,
                                    &mut line_color,
                                    current_color,
                                );
                            }
                            tag_stack.retain(|t| t != "i");
                            current_color = COLOR_BODY;
                            for t in &tag_stack {
                                match t.as_str() {
                                    "h1" => current_color = COLOR_H1,
                                    "h2" => current_color = COLOR_H2,
                                    "h3" => current_color = COLOR_H3,
                                    "a" => current_color = COLOR_LINK,
                                    _ => {}
                                }
                            }
                        } else {
                            tag_stack.push("i".to_string());
                            current_color = COLOR_ITALIC;
                        }
                        line_color = current_color;
                    }
                    "a" => {
                        if is_closing {
                            // Flush accumulated text with link color BEFORE resetting
                            if !line_chars.is_empty() {
                                flush_line(
                                    &mut lines,
                                    &mut line_chars,
                                    &mut line_color,
                                    current_color,
                                );
                            }
                            if let Some(href) = current_link_href.take() {
                                let char_end = 0; // already flushed
                                links.push(HtmlLink {
                                    href,
                                    line_index: if lines.len() > 0 { lines.len() - 1 } else { 0 },
                                    char_start: 0,
                                    char_end,
                                });
                            }
                            tag_stack.retain(|t| t != "a");
                            current_color = COLOR_BODY;
                            for t in &tag_stack {
                                match t.as_str() {
                                    "h1" => current_color = COLOR_H1,
                                    "h2" => current_color = COLOR_H2,
                                    "h3" => current_color = COLOR_H3,
                                    "b" => current_color = COLOR_BOLD,
                                    "i" => current_color = COLOR_ITALIC,
                                    _ => {}
                                }
                            }
                            line_color = current_color;
                        } else {
                            tag_stack.push("a".to_string());
                            current_color = COLOR_LINK;
                            line_color = COLOR_LINK;
                            _link_char_start = line_chars.len();
                            current_link_href = None;
                            if let Some(hpos) = tag_text.find("href") {
                                let rest = &tag_text[hpos + 4..];
                                let rest = rest.trim_start_matches(|c: char| c == ' ' || c == '=');
                                let rest = rest.trim_start_matches('"');
                                if let Some(end) = rest.find('"') {
                                    current_link_href = Some(rest[..end].to_string());
                                } else if let Some(end) = rest.find(' ') {
                                    current_link_href = Some(rest[..end].to_string());
                                }
                            }
                        }
                    }
                    "img" => {
                        let mut alt_text = String::from("IMAGE");
                        if let Some(apos) = tag_text.find("alt") {
                            let rest = &tag_text[apos + 3..];
                            let rest = rest.trim_start_matches(|c: char| c == ' ' || c == '=');
                            let rest = rest.trim_start_matches('"');
                            if let Some(end) = rest.find('"') {
                                alt_text = rest[..end].to_string();
                            }
                        }
                        let img_label = format!("[{}]", alt_text);
                        for c in img_label.chars() {
                            if line_chars.len() < CHARS_PER_LINE {
                                line_chars.push(c as u32);
                            }
                        }
                    }
                    "hr" => {
                        flush_line(&mut lines, &mut line_chars, &mut line_color, current_color);
                        let mut hr_chars = Vec::new();
                        for _ in 0..CHARS_PER_LINE.min(30) {
                            hr_chars.push('-' as u32);
                        }
                        lines.push(StyledLine {
                            fg_color: COLOR_HR,
                            chars: hr_chars,
                        });
                    }
                    "ul" => {
                        if is_closing {
                            tag_stack.retain(|t| t != "ul");
                        } else {
                            tag_stack.push("ul".to_string());
                        }
                    }
                    "li" => {
                        flush_line(&mut lines, &mut line_chars, &mut line_color, current_color);
                        line_chars.push('*' as u32);
                        line_chars.push(' ' as u32);
                        line_color = COLOR_BODY;
                    }
                    "title" => {
                        if !is_closing {
                            tag_stack.push("title".to_string());
                        } else {
                            flush_line(&mut lines, &mut line_chars, &mut line_color, current_color);
                            tag_stack.retain(|t| t != "title");
                        }
                    }
                    _ => {}
                }
            } else {
                let c = chars[pos];
                if c == '\n' {
                    lines.push(StyledLine {
                        fg_color: line_color,
                        chars: line_chars.clone(),
                    });
                    line_chars.clear();
                    line_color = current_color;
                } else if c == '\r' {
                    // skip carriage return
                } else {
                    if line_chars.len() >= CHARS_PER_LINE {
                        lines.push(StyledLine {
                            fg_color: line_color,
                            chars: line_chars.clone(),
                        });
                        line_chars.clear();
                        line_color = current_color;
                    }
                    line_chars.push(c as u32);
                }
                pos += 1;
            }
        }

        if !line_chars.is_empty() {
            lines.push(StyledLine {
                fg_color: line_color,
                chars: line_chars,
            });
        }

        // Register links as hit regions for HITQ click detection
        let line_height: u32 = 8;
        let char_width: u32 = 6;
        for (link_idx, link) in links.iter().enumerate() {
            let line_y = (link.line_index as u32) * line_height;
            let x_start = (link.char_start as u32) * char_width;
            let x_end = (link.char_end as u32) * char_width;
            if self.hit_regions.len() < types::MAX_HIT_REGIONS {
                self.hit_regions.push(types::HitRegion {
                    x: x_start,
                    y: line_y,
                    w: x_end.saturating_sub(x_start),
                    h: line_height,
                    id: link_idx as u32,
                });
            }
            // Store link href in RAM after the styled lines data
            let href_base = dest_base + max_lines * 33 + link_idx * 64;
            for (j, byte) in link.href.bytes().enumerate() {
                if j < 63 && href_base + j < self.ram.len() {
                    self.ram[href_base + j] = byte as u32;
                }
            }
            let null_pos = href_base + link.href.len().min(63);
            if null_pos < self.ram.len() {
                self.ram[null_pos] = 0;
            }
        }

        lines
    }

    // ── Crash Recovery (Phase 104) ────────────────────────────────

    /// Write a core dump file to InodeFS for a segfaulted process.
    /// Creates /var/core/ directory if needed, then writes PID.txt with:
    /// PID, exit code, PC, faulting address, all 32 registers,
    /// and the last 16 PC values from the instruction trace buffer.
    /// Returns true if the core dump was written successfully.
    pub fn write_core_dump(&mut self, proc_info: &types::Process) -> bool {
        // Ensure /var/core/ directory exists
        if self.inode_fs.resolve("/var").is_none() {
            self.inode_fs.mkdir("/var");
        }
        if self.inode_fs.resolve("/var/core").is_none() {
            self.inode_fs.mkdir("/var/core");
        }

        // Build core dump content as a string
        let mut dump = String::new();
        dump.push_str("=== CORE DUMP ===\n");
        dump.push_str(&format!("PID: {}\n", proc_info.pid));
        dump.push_str("Exit: segfault\n");
        dump.push_str(&format!("PC: 0x{:04X}\n", proc_info.pc));
        dump.push_str(&format!("Fault: 0x{:04X}\n", self.segfault_addr));
        dump.push_str(&format!("Tick: {}\n", self.sched_tick));
        dump.push_str("Registers:\n");
        for i in 0..32 {
            dump.push_str(&format!("  r{:02}: 0x{:08X}\n", i, proc_info.regs[i]));
        }
        dump.push_str("PC Trace (last 16, oldest first):\n");
        let start = self.pc_trace_idx; // next write position = oldest
        for i in 0..16 {
            let idx = (start + i) % 16;
            let pc = self.pc_trace[idx];
            if pc != 0 {
                dump.push_str(&format!("  {:2}: 0x{:04X}\n", i, pc));
            }
        }
        dump.push_str("=== END DUMP ===\n");

        // Convert string to u32 words (one ASCII char per word, null terminated)
        let words: Vec<u32> = dump
            .bytes()
            .map(|b| b as u32)
            .chain(std::iter::once(0))
            .collect();

        // Create the file
        let path = format!("/var/core/{}.txt", proc_info.pid);
        let ino = self.inode_fs.create(&path);
        if ino == 0 {
            return false;
        }

        // Write the data
        let written = self.inode_fs.write_inode(ino, 0, &words);
        written > 0
    }

    /// Render a crash dialog on the 256x256 screen showing segfault info.
    /// Draws a dark red dialog box with white text for PID, PC, fault address.
    /// Sets crash_dialog_active and crash_dialog_pid for host to check.
    pub fn render_crash_dialog(&mut self, proc_info: &types::Process) {
        let dx = 16u32;
        let dy = 80u32;
        let dw = 224u32;
        let dh = 96u32;

        // Dark background (0x1A1A2E)
        for y in dy..dy + dh {
            for x in dx..dx + dw {
                if (x as usize) < 256 && (y as usize) < 256 {
                    self.screen[y as usize * 256 + x as usize] = 0x1A1A2E;
                }
            }
        }

        // Red border (0xFF0000)
        let border = 0xFF0000u32;
        for x in dx..dx + dw {
            if (x as usize) < 256 {
                if (dy as usize) < 256 {
                    self.screen[dy as usize * 256 + x as usize] = border;
                }
                let by = (dy + dh - 1) as usize;
                if by < 256 {
                    self.screen[by * 256 + x as usize] = border;
                }
            }
        }
        for y in dy..dy + dh {
            if (y as usize) < 256 {
                self.screen[y as usize * 256 + dx as usize] = border;
                self.screen[y as usize * 256 + (dx + dw - 1) as usize] = border;
            }
        }

        // Red title bar
        for x in (dx + 1)..(dx + dw - 1) {
            if (x as usize) < 256 {
                self.screen[(dy + 1) as usize * 256 + x as usize] = 0xCC0000;
            }
        }

        // Render text lines using pixel font (white on dark)
        let white = 0xFFFFFFu32;
        let gray = 0xAAAAAAu32;

        // Line 1: "SEGFAULT" in title bar
        let text_y = dy + 3;
        for (i, ch) in b"SEGFAULT!".iter().enumerate() {
            let tx = dx + 8 + (i as u32) * 10;
            Self::render_char(&mut self.screen, *ch, tx, text_y, white);
        }

        // Line 2: PID
        let pid_str = format!("PID: {}  Parent: {}", proc_info.pid, proc_info.parent_pid);
        let text2_y = text_y + 18;
        for (i, ch) in pid_str.bytes().enumerate() {
            let tx = dx + 8 + (i as u32) * 8;
            Self::render_char(&mut self.screen, ch, tx, text2_y, gray);
        }

        // Line 3: PC and fault address
        let crash_str = format!(
            "PC=0x{:04X}  FAULT=0x{:04X}",
            proc_info.pc, self.segfault_addr
        );
        let text3_y = text2_y + 14;
        for (i, ch) in crash_str.bytes().enumerate() {
            let tx = dx + 8 + (i as u32) * 8;
            Self::render_char(&mut self.screen, ch, tx, text3_y, white);
        }

        // Line 4: r0 value
        let r0_str = format!("r0=0x{:08X}", proc_info.regs[0]);
        let text4_y = text3_y + 14;
        for (i, ch) in r0_str.bytes().enumerate() {
            let tx = dx + 8 + (i as u32) * 8;
            Self::render_char(&mut self.screen, ch, tx, text4_y, gray);
        }

        // Line 5: dismiss prompt
        let text5_y = text4_y + 16;
        for (i, ch) in b"Press any key to dismiss".iter().enumerate() {
            let tx = dx + 8 + (i as u32) * 8;
            if tx + 8 < 256 {
                Self::render_char(&mut self.screen, *ch, tx, text5_y, 0x666666);
            }
        }

        self.crash_dialog_active = true;
        self.crash_dialog_pid = proc_info.pid;
    }

    /// Render a single 8x8 character glyph on the screen buffer.
    fn render_char(screen: &mut [u32], ch: u8, x: u32, y: u32, color: u32) {
        let glyph_idx = (ch as usize).min(127);
        let glyph = &crate::font::GLYPHS[glyph_idx];
        for row in 0..8u32 {
            let row_bits = glyph[row as usize];
            for col in 0..8u32 {
                if row_bits & (0x80 >> col) != 0 {
                    let px = x + col;
                    let py = y + row;
                    if (px as usize) < 256 && (py as usize) < 256 {
                        screen[py as usize * 256 + px as usize] = color;
                    }
                }
            }
        }
    }
}

/// Strip <think/> and <think ...>...</think blocks from text.
pub(crate) fn strip_think_blocks(text: &str) -> String {
    let mut result = text.to_string();
    // Strip <think/> or <think /> (self-closing)
    loop {
        if let Some(pos) = result.find("<think/>") {
            result.replace_range(pos..pos + 8, "");
        } else if let Some(pos) = result.find("<think />") {
            result.replace_range(pos..pos + 9, "");
        } else {
            break;
        }
    }
    // Strip <think ...>...</think or <think...</think (non-greedy)
    // Handles both proper XML (<think reasoning here</think) and
    // malformed (<think...</think without closing >)
    loop {
        let start = result.find("<think");
        if let Some(s) = start {
            // Skip self-closing tags (already handled above)
            let rest = &result[s + 6..];
            if rest.starts_with("/>") || rest.starts_with(" />") {
                break;
            }
            // Find </think closing tag
            if let Some(close_offset) = result[s..].find("</think") {
                let close_start = s + close_offset;
                // Find the > after </think (or end of tag)
                let after_close = &result[close_start + 7..];
                let end_len = if let Some(gt) = after_close.find('>') {
                    close_start + 7 + gt + 1 - s
                } else if after_close.starts_with(' ') {
                    // </think without > but followed by space - strip to end of </think
                    let sp_end = after_close
                        .find(|c: char| !c.is_whitespace())
                        .unwrap_or(after_close.len());
                    close_start + 7 + sp_end - s
                } else {
                    close_start + 7 - s
                };
                if s + end_len <= result.len() {
                    result.replace_range(s..s + end_len, "");
                    continue;
                }
            }
        }
        break;
    }
    result.trim().to_string()
}

/// Extract a string value from JSON by key name (minimal parser, no serde dependency).
pub(crate) fn extract_json_str(json: &str, key: &str) -> Option<String> {
    let search = format!("\"{}\":\"", key);
    let start = json.find(&search)?;
    let val_start = start + search.len();
    let mut i = val_start;
    let bytes = json.as_bytes();
    let mut result = String::new();
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
}

/// Build a compact, deduplicated, comma-separated list of valid opcode mnemonics
/// with their hex codes. Emitted into the asm_dev system prompt as the authoritative
/// inventory the LLM may emit. Sorted by opcode.
fn build_opcode_inventory_line() -> String {
    let mut pairs = crate::vm::disasm::valid_opcode_mnemonics();
    pairs.sort_by_key(|(op, _)| *op);
    let mut out = String::with_capacity(pairs.len() * 16);
    for (i, (op, name)) in pairs.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        out.push_str(&format!("{}(0x{:02X})", name, op));
    }
    out
}

mod boot;
mod disasm;
mod net;
pub(crate) use net::MAX_TCP_CONNECTIONS;
mod scheduler;

#[cfg(test)]
mod browser_tests;
#[cfg(test)]
mod http_tests;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod tests_bgvm;
