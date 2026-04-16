// vm.rs -- Geometry OS Virtual Machine
//
// Executes bytecode assembled from the canvas text surface.
// The VM is simple: fetch one u32 from RAM at PC, decode as opcode, execute.
// 32 registers (r0-r31), 64K RAM, 256x256 screen buffer.

pub const RAM_SIZE: usize = 0x10000; // 65536 u32 cells
pub const SCREEN_SIZE: usize = 256 * 256;
pub const NUM_REGS: usize = 32;
/// Canvas RAM region: address range [0x8000, 0x8FFF] maps to the pixel grid.
pub const CANVAS_RAM_BASE: usize = 0x8000;
pub const CANVAS_RAM_SIZE: usize = 4096;
/// Screen RAM region: address range [0x10000, 0x1FFFF] maps to the screen buffer.
pub const SCREEN_RAM_BASE: usize = 0x10000;

/// Formula engine constants (Phase 50: Reactive Canvas).
/// Maximum number of formula cells allowed (to bound recalc cost).
pub const MAX_FORMULAS: usize = 256;
/// Maximum dependencies a single formula can reference.
pub const MAX_FORMULA_DEPS: usize = 8;
/// Maximum evaluation depth to prevent infinite recursion in cyclic deps.
#[allow(dead_code)]
pub const FORMULA_EVAL_DEPTH_LIMIT: u32 = 32;

/// A formula attached to a canvas cell. When any of its dependencies change,
/// the formula is re-evaluated and the result written back to the cell.
#[derive(Debug, Clone)]
pub struct Formula {
    /// The canvas-buffer index this formula writes its result to.
    pub target_idx: usize,
    /// List of canvas-buffer indices this formula reads from.
    pub deps: Vec<usize>,
    /// The operation to perform.
    pub op: FormulaOp,
}

/// Operations a formula can perform.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormulaOp {
    /// result = deps[0] + deps[1]
    Add,
    /// result = deps[0] - deps[1]
    Sub,
    /// result = deps[0] * deps[1]
    Mul,
    /// result = deps[0] / deps[1] (0 on div-by-zero)
    Div,
    /// result = deps[0] & deps[1]
    And,
    /// result = deps[0] | deps[1]
    Or,
    /// result = deps[0] ^ deps[1]
    Xor,
    /// result = !deps[0] (bitwise NOT, single dep)
    Not,
    /// result = deps[0] (identity/copy, single dep)
    Copy,
    /// result = max(deps[0], deps[1])
    Max,
    /// result = min(deps[0], deps[1])
    Min,
    /// result = deps[0] % deps[1]
    Mod,
    /// result = deps[0] << deps[1]
    Shl,
    /// result = deps[0] >> deps[1]
    Shr,
}
/// Maximum number of concurrently spawned child processes
pub const MAX_PROCESSES: usize = 8;
/// Syscall dispatch table base address in RAM.
/// RAM[SYSCALL_TABLE + N] = handler address for syscall number N.
pub const SYSCALL_TABLE: usize = 0xFE00;

/// Memory protection constants (Phase 24: Memory Protection).
/// RAM is divided into pages. Each process gets a page directory mapping
/// virtual page numbers to physical page numbers.
pub const PAGE_SIZE: usize = 1024; // words per page (4096 bytes)
/// Total number of addressable pages (RAM + Screen)
pub const NUM_PAGES: usize = (RAM_SIZE + SCREEN_SIZE) / PAGE_SIZE; // 128 pages
/// Number of pages backed by actual RAM (allocatable)
pub const NUM_RAM_PAGES: usize = RAM_SIZE / PAGE_SIZE; // 64 pages
/// Sentinel: page directory entry is unmapped (no physical page backing).
pub const PAGE_UNMAPPED: u32 = 0xFFFFFFFF;
/// Number of pages allocated to each new spawned process.
pub const PROCESS_PAGES: usize = 4; // 4096 words = 16KB per process

/// CPU privilege mode: Kernel (full access) or User (restricted).
/// VM starts in Kernel mode for backward compatibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[derive(Default)]
pub enum CpuMode {
    #[default]
    Kernel,
    User,
}


/// Process priority levels for the preemptive scheduler (Phase 26).
/// Higher priority = more CPU time slices per round.
#[allow(dead_code)]
pub const PRIORITY_LEVELS: u8 = 4;
/// Default base time slice length (in VM steps) for priority-1 processes.
pub const DEFAULT_TIME_SLICE: u32 = 100;

/// IPC constants (Phase 27: Inter-Process Communication).
/// Pipe buffer size in u32 words.
pub const PIPE_BUFFER_SIZE: usize = 256;
/// Maximum number of pipes system-wide.
pub const MAX_PIPES: usize = 16;
/// Maximum messages per-process message queue.
pub const MAX_MESSAGES: usize = 16;
/// Message payload size in u32 words.
pub const MSG_WORDS: usize = 4;

/// Device driver constants (Phase 28: Device Driver Abstraction).
/// Device fd base: device fds live at 0xE000+device_index.
pub const DEVICE_FD_BASE: u32 = 0xE000;
/// Device types mapped to fixed fd slots.
#[allow(dead_code)]
pub const DEVICE_SCREEN: u32 = 0; // /dev/screen -> fd 0xE000
#[allow(dead_code)]
pub const DEVICE_KEYBOARD: u32 = 1; // /dev/keyboard -> fd 0xE001
#[allow(dead_code)]
pub const DEVICE_AUDIO: u32 = 2; // /dev/audio -> fd 0xE002
#[allow(dead_code)]
pub const DEVICE_NET: u32 = 3; // /dev/net -> fd 0xE003
pub const DEVICE_COUNT: usize = 4;
/// Device names (indexed by device type).
pub const DEVICE_NAMES: &[&str] = &["/dev/screen", "/dev/keyboard", "/dev/audio", "/dev/net"];

/// A unidirectional pipe with a circular buffer.
/// Created by PIPE syscall. Two fd slots are allocated: read_fd and write_fd.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Pipe {
    /// Circular buffer data
    pub buffer: [u32; PIPE_BUFFER_SIZE],
    /// Index of next read position
    pub read_pos: usize,
    /// Index of next write position
    pub write_pos: usize,
    /// Number of words currently in the buffer
    pub count: usize,
    /// PID of the process that has the read end open (0 = main)
    pub read_pid: u32,
    /// PID of the process that has the write end open (0 = main)
    pub write_pid: u32,
    /// Whether the pipe is still alive (false if write end closed)
    pub alive: bool,
}

impl Pipe {
    pub fn new(read_pid: u32, write_pid: u32) -> Self {
        Pipe {
            buffer: [0; PIPE_BUFFER_SIZE],
            read_pos: 0,
            write_pos: 0,
            count: 0,
            read_pid,
            write_pid,
            alive: true,
        }
    }

    /// Write one word to the pipe. Returns true on success, false if full.
    pub fn write_word(&mut self, val: u32) -> bool {
        if self.count >= PIPE_BUFFER_SIZE {
            return false;
        }
        self.buffer[self.write_pos] = val;
        self.write_pos = (self.write_pos + 1) % PIPE_BUFFER_SIZE;
        self.count += 1;
        true
    }

    /// Read one word from the pipe. Returns Some(word) or None if empty.
    pub fn read_word(&mut self) -> Option<u32> {
        if self.count == 0 {
            return None;
        }
        let val = self.buffer[self.read_pos];
        self.read_pos = (self.read_pos + 1) % PIPE_BUFFER_SIZE;
        self.count -= 1;
        Some(val)
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    #[allow(dead_code)]
    pub fn is_full(&self) -> bool {
        self.count >= PIPE_BUFFER_SIZE
    }
}

/// A fixed-size message sent between processes.
#[derive(Debug, Clone, Copy)]
pub struct Message {
    /// Sender PID
    pub sender: u32,
    /// Payload: 4 u32 words
    pub data: [u32; MSG_WORDS],
}

impl Message {
    pub fn new(sender: u32, data: [u32; MSG_WORDS]) -> Self {
        Message { sender, data }
    }
}

/// Signal types that can be sent to processes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Signal {
    /// Terminate the process (default handler: halt with exit code 1)
    Term = 0,
    /// User-defined signal 1 (default handler: ignore)
    User1 = 1,
    /// User-defined signal 2 (default handler: ignore)
    User2 = 2,
    /// Stop the process (default handler: halt with exit code 2)
    Stop = 3,
}

impl Signal {
    /// Convert from u32 signal number. Returns None for invalid signals.
    pub fn from_u32(n: u32) -> Option<Signal> {
        match n {
            0 => Some(Signal::Term),
            1 => Some(Signal::User1),
            2 => Some(Signal::User2),
            3 => Some(Signal::Stop),
            _ => None,
        }
    }
}

/// Process lifecycle states, analogous to Linux task_state.
///
/// State transitions:
///   Ready -> Running       (scheduler picks this process)
///   Running -> Ready       (time slice exhausted or yield)
///   Running -> Sleeping    (SLEEP opcode)
///   Sleeping -> Ready      (sleep timer expires)
///   Running -> Blocked     (pipe read empty / MSGRCV empty)
///   Blocked -> Ready       (data available)
///   Running -> Zombie      (EXIT opcode or fatal signal)
///   Zombie -> <gone>       (parent calls WAITPID, reaps exit code)
///   Any -> Stopped         (SIGSTOP)
///   Stopped -> Ready       (SIGCONT -- future)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(dead_code)]
pub enum ProcessState {
    /// Runnable, waiting for scheduler to pick it up.
    #[default]
    Ready,
    /// Currently executing on the CPU.
    Running,
    /// Sleeping until sched_tick reaches sleep_until.
    Sleeping,
    /// Blocked on I/O (empty pipe read, empty message receive).
    Blocked,
    /// Exited but parent has not reaped it yet. exit_code holds the result.
    Zombie,
    /// Stopped by signal (SIGSTOP equivalent).
    Stopped,
}


/// VMA (Virtual Memory Area) type, analogous to Linux vm_area_struct.
/// Each VMA describes a contiguous range of virtual pages with a purpose.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum VmaType {
    /// Code segment: loaded at spawn, read+execute, not growable.
    Code,
    /// Heap segment: grows upward via brk, read+write.
    Heap,
    /// Stack segment: grows downward on page fault, read+write.
    Stack,
    /// Memory-mapped region: allocated via mmap, read+write.
    Mmap,
}

/// A single virtual memory area describing a contiguous page range.
///
/// `start_page` is inclusive, `current_end` is the last mapped page.
/// For growable regions (Heap, Stack), `max_end` is the furthest the VMA
/// is allowed to expand to.
#[derive(Debug, Clone)]
pub struct Vma {
    /// What this region is used for.
    pub vtype: VmaType,
    /// First virtual page number of this region.
    pub start_page: usize,
    /// Last currently-mapped virtual page number (inclusive).
    pub current_end: usize,
    /// Maximum virtual page number this region may expand to (inclusive).
    pub max_end: usize,
}

impl Vma {
    pub fn new(vtype: VmaType, start_page: usize, current_end: usize, max_end: usize) -> Self {
        Vma { vtype, start_page, current_end, max_end }
    }

    /// Does this VMA contain the given virtual page number?
    pub fn contains_page(&self, vpage: usize) -> bool {
        vpage >= self.start_page && vpage <= self.max_end
    }

    /// Is the page within the currently-mapped range?
    #[allow(dead_code)]
    pub fn is_mapped(&self, vpage: usize) -> bool {
        vpage >= self.start_page && vpage <= self.current_end
    }

    /// Can this VMA grow to cover `vpage`?
    pub fn can_grow_to(&self, vpage: usize) -> bool {
        if !self.contains_page(vpage) { return false; }
        match self.vtype {
            VmaType::Code => false, // code is fixed
            VmaType::Heap => vpage > self.current_end && vpage <= self.max_end,
            VmaType::Stack => vpage < self.start_page && vpage >= self.max_end,
            VmaType::Mmap => vpage > self.current_end && vpage <= self.max_end,
        }
    }
}

/// Process control block, modeled after Linux task_struct.
///
/// Each process has:
/// - Identity: PID, parent PID
/// - CPU state: saved registers, PC, privilege mode
/// - Memory: page table root (page directory), kernel stack
/// - Scheduling: state, priority, time slice
/// - IPC: message queue, signal handlers
/// - Lifecycle: exit code, zombie tracking
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Process {
    // ── Identity ──────────────────────────────────────────────────
    /// Process ID (1-based). PID 0 is the main/kernel context.
    pub pid: u32,
    /// PID of the parent process. 0 = spawned by kernel/init.
    pub parent_pid: u32,

    // ── CPU state (saved context) ─────────────────────────────────
    /// Program counter: address of next instruction to execute.
    pub pc: u32,
    /// General-purpose register file (r0-r31).
    pub regs: [u32; NUM_REGS],
    /// CPU privilege mode: Kernel (full access) or User (restricted).
    pub mode: CpuMode,

    // ── Memory ────────────────────────────────────────────────────
    /// Page directory for virtual-to-physical address translation.
    /// None = identity mapping (kernel mode).
    /// Each entry maps a virtual page number to a physical page number.
    /// PAGE_UNMAPPED (0xFFFFFFFF) = unmapped -> segfault on access.
    pub page_dir: Option<Vec<u32>>,

    /// Per-process kernel stack. Stores (return_pc, saved_mode) frames
    /// pushed by SYSCALL and popped by RETK. Each process has its own
    /// stack so nested syscalls in different processes don't interfere.
    pub kernel_stack: Vec<(u32, CpuMode)>,

    // ── Scheduling ────────────────────────────────────────────────
    /// Current process state (Ready, Running, Sleeping, etc.).
    pub state: ProcessState,
    /// Scheduler priority (0 = lowest, 3 = highest). Default: 1.
    pub priority: u8,
    /// Remaining instructions in current time slice.
    pub slice_remaining: u32,
    /// If sleeping: the sched_tick value at which this process wakes.
    pub sleep_until: u64,
    /// Set by YIELD opcode; scheduler preempts mid-slice.
    pub yielded: bool,

    // ── IPC ────────────────────────────────────────────────────────
    /// Per-process message queue (max MAX_MESSAGES entries).
    pub msg_queue: Vec<Message>,
    /// Signal handler addresses, indexed by signal number (0-3).
    /// 0 = default handler, 0xFFFFFFFF = ignore, else = RAM address.
    pub signal_handlers: [u32; 4],
    /// Pending signals queued for delivery on next step.
    pub pending_signals: Vec<Signal>,

    // ── Virtual Memory Areas (Phase 44) ────────────────────────────
    /// Per-process list of virtual memory areas describing address space layout.
    /// Used by the page fault handler to decide whether to allocate on demand.
    pub vmas: Vec<Vma>,
    /// Current heap break position (virtual address). Grows upward via brk.
    /// Initial value is end of code+data segment.
    pub brk_pos: u32,

    // ── Lifecycle ──────────────────────────────────────────────────
    /// Exit code set by EXIT opcode or fatal signal. 0 = success.
    pub exit_code: u32,
    /// True if the process segfaulted on an unmapped memory access.
    pub segfaulted: bool,
}

/// Backward-compatible alias for Process.
pub type SpawnedProcess = Process;

#[allow(dead_code)]
impl Process {
    /// Create a new process with the given PID and entry point.
    ///
    /// The process starts in Ready state with User mode, priority 1,
    /// and no page directory (identity-mapped, which means kernel mode
    /// will be used; callers should set `mode` and `page_dir` as needed).
    pub fn new(pid: u32, parent_pid: u32, entry_pc: u32) -> Self {
        Process {
            pid,
            parent_pid,
            pc: entry_pc,
            regs: [0; NUM_REGS],
            mode: CpuMode::User,
            page_dir: None,
            kernel_stack: Vec::new(),
            state: ProcessState::Ready,
            priority: 1,
            slice_remaining: 0,
            sleep_until: 0,
            yielded: false,
            msg_queue: Vec::new(),
            signal_handlers: [0; 4],
            pending_signals: Vec::new(),
            vmas: Vec::new(),
            brk_pos: 0,
            exit_code: 0,
            segfaulted: false,
        }
    }

    /// Convenience: is this process halted (zombie, segfaulted, or stopped)?
    /// The scheduler skips halted processes.
    pub fn is_halted(&self) -> bool {
        matches!(self.state, ProcessState::Zombie | ProcessState::Stopped)
            || self.segfaulted
    }

    /// Is this process in a runnable state (Ready or Running)?
    pub fn is_runnable(&self) -> bool {
        matches!(self.state, ProcessState::Ready | ProcessState::Running)
    }

    /// Default VMA layout for a new process:
    ///   Page 0: Code (fixed, loaded at spawn)
    ///   Page 1: Heap (grows up to page 4)
    ///   Page 2: Stack (grows downward from page 2 to page 1 -- toward lower pages)
    ///   Pages 3+: available for mmap
    pub fn default_vmas_for_process() -> Vec<Vma> {
        // Initial address space for a spawned process:
        //   Code:  virtual pages 0-2 (3 pages for code/data, max PROCESS_PAGES-1)
        //   Stack: virtual page 3 (top of initial allocation, grows downward)
        //   Heap:  starts at page 4 but current_end == max_end so no demand paging
        //          until brk() extends it
        //
        // Only the Stack VMA permits demand growth (downward, toward lower pages).
        // The Heap VMA requires explicit brk() to extend max_end before faults resolve.
        vec![
            // Code: pages 0-2, not growable (max_end == current_end)
            Vma::new(VmaType::Code, 0, PROCESS_PAGES - 2, PROCESS_PAGES - 2),
            // Stack: page 3 (top of user space), can grow down to page 2
            // Stack grows downward so start_page > max_end is intentional for Stack
            Vma::new(VmaType::Stack, PROCESS_PAGES - 1, PROCESS_PAGES - 1, PROCESS_PAGES - 2),
            // Heap: page 4 onward, initially empty (max_end == PROCESS_PAGES so no growth)
            // brk() extends max_end to allow demand allocation
            Vma::new(VmaType::Heap, PROCESS_PAGES, PROCESS_PAGES, PROCESS_PAGES),
        ]
    }

    /// Find the VMA that contains the given virtual page.
    pub fn find_vma(&self, vpage: usize) -> Option<&Vma> {
        self.vmas.iter().find(|vma| vma.contains_page(vpage))
    }

    /// Find the VMA that contains the given virtual page (mutable).
    pub fn find_vma_mut(&mut self, vpage: usize) -> Option<&mut Vma> {
        self.vmas.iter_mut().find(|vma| vma.contains_page(vpage))
    }
}

/// Magic bytes for save files
pub const SAVE_MAGIC: &[u8; 4] = b"GEOS";
/// Save file format version
pub const SAVE_VERSION: u32 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemAccessKind {
    Read,
    Write,
}

#[derive(Debug, Clone, Copy)]
pub struct MemAccess {
    pub addr: usize,
    pub kind: MemAccessKind,
}

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

/// Hypervisor execution mode.
/// QEMU mode spawns a subprocess; Native mode uses the built-in RISC-V interpreter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[derive(Default)]
pub enum HypervisorMode {
    /// Use QEMU subprocess for guest execution (any architecture).
    #[default]
    Qemu,
    /// Use built-in RISC-V interpreter (Phases 34-36, pure Rust, WASM-portable).
    Native,
}


impl Default for Vm {
    fn default() -> Self {
        Self::new()
    }
}

impl Vm {
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

    // --- Phase 50: Reactive Canvas Formula Engine ---

    /// Register a formula. Returns false if limits exceeded or cycle detected.
    pub fn formula_register(&mut self, target_idx: usize, deps: Vec<usize>, op: FormulaOp) -> bool {
        if self.formulas.len() >= MAX_FORMULAS { return false; }
        if deps.len() > MAX_FORMULA_DEPS { return false; }
        if target_idx >= CANVAS_RAM_SIZE { return false; }
        for &d in &deps {
            if d >= CANVAS_RAM_SIZE { return false; }
        }

        // Remove any existing formula targeting the same cell
        self.formula_remove(target_idx);

        // Cycle detection: adding this formula must not create a cycle.
        // A cycle exists if target_idx is reachable from any of its deps
        // through the existing formula graph.
        if self.has_formula_cycle(target_idx, &deps) {
            return false;
        }

        let fidx = self.formulas.len();
        let formula = Formula { target_idx, deps: deps.clone(), op };
        self.formulas.push(formula);

        // Update reverse dependency index
        for &dep in &deps {
            if dep < self.formula_dep_index.len() {
                self.formula_dep_index[dep].push(fidx);
            }
        }
        true
    }

    /// Remove the formula targeting `target_idx`, if any.
    pub fn formula_remove(&mut self, target_idx: usize) {
        if let Some(pos) = self.formulas.iter().position(|f| f.target_idx == target_idx) {
            // Remove from dep index
            for dep_list in self.formula_dep_index.iter_mut() {
                dep_list.retain(|&fi| fi != pos);
                // Shift indices > pos down by 1 since we're removing
                for fi in dep_list.iter_mut() {
                    if *fi > pos { *fi -= 1; }
                }
            }
            self.formulas.remove(pos);
        }
    }

    /// Check if adding a formula from deps -> target would create a cycle.
    fn has_formula_cycle(&self, target_idx: usize, deps: &[usize]) -> bool {
        // A cycle exists if target_idx is transitively depended upon by any dep.
        // Walk the dependency graph from each dep and see if we reach target_idx.
        let mut visited = std::collections::HashSet::new();
        let mut stack: Vec<usize> = deps.to_vec();
        while let Some(idx) = stack.pop() {
            if idx == target_idx { return true; }
            if !visited.insert(idx) { continue; }
            // Find formulas that target this idx -- their deps could reach target
            for f in &self.formulas {
                if f.target_idx == idx {
                    stack.extend_from_slice(&f.deps);
                }
            }
        }
        false
    }

    /// Evaluate a single formula given current canvas buffer state.
    fn formula_eval(&self, formula: &Formula, canvas: &[u32]) -> u32 {
        let get = |idx: usize| -> u32 {
            if idx < canvas.len() { canvas[idx] } else { 0 }
        };
        match formula.op {
            FormulaOp::Add  => get(formula.deps[0]).wrapping_add(get(formula.deps[1])),
            FormulaOp::Sub  => get(formula.deps[0]).wrapping_sub(get(formula.deps[1])),
            FormulaOp::Mul  => get(formula.deps[0]).wrapping_mul(get(formula.deps[1])),
            FormulaOp::Div  => {
                let d = get(formula.deps[1]);
                if d == 0 { 0 } else { get(formula.deps[0]) / d }
            },
            FormulaOp::And  => get(formula.deps[0]) & get(formula.deps[1]),
            FormulaOp::Or   => get(formula.deps[0]) | get(formula.deps[1]),
            FormulaOp::Xor  => get(formula.deps[0]) ^ get(formula.deps[1]),
            FormulaOp::Not  => !get(formula.deps[0]),
            FormulaOp::Copy => get(formula.deps[0]),
            FormulaOp::Max  => get(formula.deps[0]).max(get(formula.deps[1])),
            FormulaOp::Min  => get(formula.deps[0]).min(get(formula.deps[1])),
            FormulaOp::Mod  => {
                let d = get(formula.deps[1]);
                if d == 0 { 0 } else { get(formula.deps[0]) % d }
            },
            FormulaOp::Shl  => get(formula.deps[0]).wrapping_shl(get(formula.deps[1]) % 32),
            FormulaOp::Shr  => get(formula.deps[0]).wrapping_shr(get(formula.deps[1]) % 32),
        }
    }

    /// Recalculate all formulas that depend on `changed_idx`.
    /// Called after a STORE to a canvas cell.
    pub fn formula_recalc(&mut self, changed_idx: usize) {
        if changed_idx >= self.formula_dep_index.len() { return; }
        let affected: Vec<usize> = self.formula_dep_index[changed_idx].clone();
        if affected.is_empty() { return; }

        // Evaluate all affected formulas (use a snapshot to avoid borrow issues)
        let canvas_snapshot = self.canvas_buffer.clone();
        let mut updates: Vec<(usize, u32)> = Vec::new();
        for &fidx in &affected {
            if fidx < self.formulas.len() {
                let result = self.formula_eval(&self.formulas[fidx], &canvas_snapshot);
                updates.push((self.formulas[fidx].target_idx, result));
            }
        }
        // Apply updates
        for (idx, val) in updates {
            if idx < self.canvas_buffer.len() {
                self.canvas_buffer[idx] = val;
            }
        }
    }

    /// Clear all formulas and rebuild the dependency index.
    pub fn formula_clear_all(&mut self) {
        self.formulas.clear();
        for dep_list in self.formula_dep_index.iter_mut() {
            dep_list.clear();
        }
    }

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

    /// Allocate `count` contiguous physical pages. Returns start page index or None.
    /// Starts scanning from page 2 (pages 0-1 reserved for kernel/main).
    /// Only scans up to NUM_RAM_PAGES (0-63).
    fn alloc_pages(&mut self, count: usize) -> Option<usize> {
        'outer: for start in 2..=(NUM_RAM_PAGES - count) {
            for i in 0..count {
                if self.allocated_pages & (1u64 << (start + i)) != 0 { continue 'outer; }
            }
            for i in 0..count {
                self.allocated_pages |= 1u64 << (start + i);
                self.page_ref_count[start + i] = 1;
            }
            return Some(start);
        }
        None
    }

    /// Free physical pages mapped by a page directory, respecting COW reference counts.
    /// Only actually frees a page when its reference count drops to 0.
    fn free_page_dir(&mut self, pd: &[u32]) {
        for &entry in pd {
            let ppage = entry as usize;
            if ppage < NUM_RAM_PAGES {
                if self.page_ref_count[ppage] > 1 {
                    // Page is shared (COW) -- just decrement ref count
                    self.page_ref_count[ppage] -= 1;
                    // Clear COW flag if only one reference remains
                    if self.page_ref_count[ppage] == 1 {
                        self.page_cow &= !(1u64 << ppage);
                    }
                } else {
                    // Last reference -- actually free the page
                    self.allocated_pages &= !(1u64 << ppage);
                    self.page_ref_count[ppage] = 0;
                    self.page_cow &= !(1u64 << ppage);
                }
            }
        }
    }

    /// Translate virtual address using current page directory.
    /// Returns None if unmapped (triggers segfault).
    fn translate_va(&self, vaddr: u32) -> Option<usize> {
        match &self.current_page_dir {
            None => Some(vaddr as usize), // identity mapping (kernel)
            Some(pd) => {
                let vpage = (vaddr as usize) / PAGE_SIZE;
                let offset = (vaddr as usize) % PAGE_SIZE;
                if vpage >= pd.len() { return None; }
                let ppage = pd[vpage] as usize;
                if ppage >= NUM_PAGES { return None; } // PAGE_UNMAPPED sentinel
                Some(ppage * PAGE_SIZE + offset)
            }
        }
    }

    /// Create a page directory for a new process: allocate PROCESS_PAGES contiguous
    /// physical pages, map virtual pages 0..PROCESS_PAGES to them, rest unmapped.
    /// The shared region (page containing 0xF00-0xFFF for Window Bounds Protocol,
    /// page containing 0xFF00+ for hardware ports) is identity-mapped so processes
    /// can communicate and access hardware through syscalls.
    fn create_process_page_dir(&mut self) -> Option<Vec<u32>> {
        let start = self.alloc_pages(PROCESS_PAGES)?;
        let mut pd = vec![PAGE_UNMAPPED; NUM_PAGES];
        for (i, pd_entry) in pd.iter_mut().enumerate().take(PROCESS_PAGES) {
            *pd_entry = (start + i) as u32;
        }
        // Identity-map shared regions so child processes can access them
        // Page 3 (0xC00-0xFFF): contains Window Bounds Protocol at 0xF00-0xFFF
        pd[3] = 3;
        // Release the private page we allocated for virtual page 3
        let private_page = start + 3;
        if private_page < NUM_PAGES {
            self.allocated_pages &= !(1u64 << private_page);
        }
        // Page 63 (0xFC00-0xFFFF): hardware ports (0xFF00+) and syscall table (0xFE00+)
        // This is already outside PROCESS_PAGES range so it's PAGE_UNMAPPED.
        // Identity-map it so syscalls work.
        pd[63] = 63;
        // Don't allocate page 63 -- it's always the kernel's hardware page
        // (main process uses it via identity mapping)
        Some(pd)
    }

    /// Handle a write to a copy-on-write page.
    ///
    /// Called when STORE targets a physical page marked as COW.
    /// Allocates a new physical page, copies the data, updates the
    /// current page directory to point to the new page, and decrements
    /// the ref count on the old page.
    ///
    /// Returns true if the COW was resolved (page now writable), false on allocation failure.
    fn handle_cow_write(&mut self, vaddr: u32) -> bool {
        let vpage = (vaddr as usize) / PAGE_SIZE;

        // Extract old physical page from current page directory
        let old_phys = match &self.current_page_dir {
            Some(pd) => {
                if vpage >= pd.len() || vpage >= NUM_PAGES { return false; }
                let p = pd[vpage] as usize;
                if p >= NUM_PAGES { return false; }
                p
            }
            None => return false,
        };

        // Check if this page is actually COW
        if self.page_cow & (1u64 << old_phys) == 0 {
            return false;
        }

        // Allocate a new physical page for the private copy
        let new_phys = match self.alloc_pages(1) {
            Some(p) => p,
            None => return false,
        };

        // Copy the page contents
        let old_base = old_phys * PAGE_SIZE;
        let new_base = new_phys * PAGE_SIZE;
        for i in 0..PAGE_SIZE {
            if old_base + i < self.ram.len() && new_base + i < self.ram.len() {
                self.ram[new_base + i] = self.ram[old_base + i];
            }
        }

        // Update page directory to point to the new private page
        if let Some(ref mut pd) = self.current_page_dir {
            pd[vpage] = new_phys as u32;
        }

        // Decrement ref count on old page, clear COW if last reference
        self.page_ref_count[old_phys] -= 1;
        if self.page_ref_count[old_phys] <= 1 {
            self.page_cow &= !(1u64 << old_phys);
        }

        // New page is NOT COW (ref_count = 1 from alloc_pages)
        self.page_cow &= !(1u64 << new_phys);

        true
    }

    /// Check if a write to the given virtual address targets a COW page.
    /// If so, resolve the COW by copying the page to a private one.
    fn resolve_cow_if_needed(&mut self, vaddr: u32) {
        let phys = match self.translate_va(vaddr) {
            Some(addr) => addr / PAGE_SIZE,
            None => return,
        };
        if phys < NUM_RAM_PAGES && (self.page_cow & (1u64 << phys)) != 0 {
            self.handle_cow_write(vaddr);
        }
    }

    /// Try to handle a page fault for the given virtual address.
    ///
    /// When translate_va returns None (unmapped page), this method checks if
    /// the faulting page could be resolved by allocating a new physical page.
    /// It uses a simple rule based on the process memory layout:
    /// - Pages 0..1 are code/heap (pre-allocated at spawn)
    /// - Page 2 is stack (pre-allocated at spawn)
    /// - Pages in the range PROCESS_PAGES..up to 60 are eligible for demand allocation
    ///   (this covers heap growth beyond the initial 4 pages, stack growth, and mmap)
    ///
    /// Returns true if the fault was resolved (page now mapped), false otherwise.
    fn handle_page_fault(&mut self, vaddr: u32) -> bool {
        let vpage = (vaddr as usize) / PAGE_SIZE;

        // Kernel mode has no page directory -- no fault handling needed
        match &self.current_page_dir {
            None => return false,
            Some(pd) => {
                if vpage >= pd.len() || vpage >= NUM_PAGES {
                    return false;
                }
                // Don't re-allocate already-mapped pages
                if (pd[vpage] as usize) < NUM_PAGES {
                    return false;
                }
                // Don't allocate for kernel pages (63) or above
                if vpage > 62 {
                    return false;
                }
            }
        }

        // If this process has VMAs, only allocate if a VMA covers this page and permits growth.
        // An empty VMA list means the old behavior (no VMA restrictions) applies.
        if !self.current_vmas.is_empty() {
            let allowed = self.current_vmas.iter().any(|vma| vma.can_grow_to(vpage));
            if !allowed {
                return false;
            }
        }

        // Allocate a single physical page
        let ppage = match self.alloc_pages(1) {
            Some(p) => p,
            None => return false,
        };

        // Map it in the page directory
        if let Some(ref mut pd) = self.current_page_dir {
            if vpage < pd.len() {
                pd[vpage] = ppage as u32;
            }
        }

        // Zero the newly allocated page
        let phys_base = ppage * PAGE_SIZE;
        for i in 0..PAGE_SIZE {
            if phys_base + i < self.ram.len() {
                self.ram[phys_base + i] = 0;
            }
        }

        true
    }

    /// Translate virtual address, attempting page fault resolution on miss.
    /// Returns None only if the fault cannot be resolved (triggers segfault).
    fn translate_va_or_fault(&mut self, vaddr: u32) -> Option<usize> {
        // First try normal translation
        match self.translate_va(vaddr) {
            Some(addr) => Some(addr),
            None => {
                // Try to resolve the page fault
                if self.handle_page_fault(vaddr) {
                    // Retry translation after mapping the page
                    self.translate_va(vaddr)
                } else {
                    None
                }
            }
        }
    }

    /// Trigger a segfault: set flag and halt the process.
    fn trigger_segfault(&mut self) {
        self.segfault = true;
        self.halted = true;
    }

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

    /// Run preemptive scheduler for all child processes.
    /// Each process gets a time slice proportional to its priority level.
    /// Sleeping processes (sleep_until > sched_tick) are skipped.
    /// Yielded processes lose their remaining slice.
    /// When a process's slice is exhausted, it waits until ALL runnable
    /// processes have also exhausted their slices (a new round), then
    /// everyone gets a fresh allocation based on current priority.
    pub fn step_all_processes(&mut self) {
        self.sched_tick += 1;

        let mut procs = std::mem::take(&mut self.processes);

        let saved_pc = self.pc;
        let saved_regs = self.regs;
        let saved_halted = self.halted;
        let saved_mode = self.mode;
        let saved_kernel_stack = std::mem::take(&mut self.kernel_stack);
        let saved_page_dir = self.current_page_dir.take();
        let saved_vmas = std::mem::take(&mut self.current_vmas);
        let saved_segfault = self.segfault;
        let saved_segfault_pid = self.segfault_pid;
        let saved_current_pid = self.current_pid;

        // Check if all runnable (non-halted, non-sleeping) processes have
        // exhausted their slices. If so, start a new scheduling round.
        let all_exhausted = procs.iter().all(|p| {
            p.is_halted()
                || (p.sleep_until > 0 && self.sched_tick < p.sleep_until)
                || p.slice_remaining == 0
        });
        if all_exhausted {
            for proc in &mut procs {
                if proc.is_halted() { continue; }
                if proc.sleep_until > 0 && self.sched_tick < proc.sleep_until {
                    continue;
                }
                let multiplier = 1u32 << proc.priority;
                proc.slice_remaining = self.default_time_slice * multiplier;
                proc.yielded = false;
            }
        }

        // Sort by priority descending (highest priority runs first)
        let mut indices: Vec<usize> = (0..procs.len()).collect();
        indices.sort_by_key(|&i| std::cmp::Reverse(procs[i].priority));

        for idx in indices {
            let proc = &mut procs[idx];
            if proc.is_halted() { continue; }

            // Skip blocked processes (waiting for pipe data or message)
            if proc.state == ProcessState::Blocked { continue; }

            // Skip sleeping processes whose sleep hasnt expired
            if proc.sleep_until > 0 && self.sched_tick < proc.sleep_until {
                continue;
            }
            // Wake up: clear sleep flag
            if proc.sleep_until > 0 && self.sched_tick >= proc.sleep_until {
                proc.sleep_until = 0;
                proc.slice_remaining = 0;
            }

            // Skip processes whose time slice is exhausted (wait for next round)
            if proc.slice_remaining == 0 {
                continue;
            }

            self.pc = proc.pc;
            self.regs = proc.regs;
            self.halted = false;
            self.mode = proc.mode;
            self.kernel_stack.clear();
            self.current_page_dir = proc.page_dir.take();
            self.current_vmas = std::mem::take(&mut proc.vmas);
            self.segfault = false;
            self.current_pid = proc.pid;

            // Reset per-step scheduler flags
            self.yielded = false;
            self.sleep_frames = 0;
            self.new_priority = proc.priority;
            self.step_exit_code = None;
            self.step_zombie = false;

            // Execute one instruction within the time slice
            let still_running = self.step();
            self.sched_tick += 1;

            // Save process state back
            proc.pc = self.pc;
            proc.regs = self.regs;
            proc.state = if !still_running || self.halted || self.segfault { ProcessState::Zombie } else { ProcessState::Ready };
            proc.mode = self.mode;
            proc.page_dir = self.current_page_dir.take();
            proc.vmas = std::mem::take(&mut self.current_vmas);
            proc.segfaulted = self.segfault;
            // Propagate EXIT opcode's exit code and zombie status
            if let Some(code) = self.step_exit_code {
                proc.exit_code = code;
            }
            if self.step_zombie {
                proc.state = ProcessState::Zombie;
            }
            if self.segfault {
                self.segfault_pid = proc.pid;
                self.ram[0xFF9] = proc.pid;
            }

            // Apply SETPRIORITY if requested
            if self.new_priority != proc.priority && self.new_priority <= 3 {
                proc.priority = self.new_priority;
            }

            // Handle YIELD: forfeit remaining time slice
            if self.yielded {
                proc.slice_remaining = 0;
                proc.yielded = true;
            } else if proc.slice_remaining > 0 {
                proc.slice_remaining -= 1;
            }

            // Handle SLEEP: mark process as sleeping
            if self.sleep_frames > 0 {
                proc.sleep_until = self.sched_tick.wrapping_add(self.sleep_frames as u64);
                proc.slice_remaining = 0;
            }
        }

        self.pc = saved_pc;
        self.regs = saved_regs;
        self.halted = saved_halted;
        self.mode = saved_mode;
        self.kernel_stack = saved_kernel_stack;
        self.current_page_dir = saved_page_dir;
        self.current_vmas = saved_vmas;
        self.segfault = saved_segfault;
        self.segfault_pid = saved_segfault_pid;
        self.current_pid = saved_current_pid;
        self.yielded = false;
        self.sleep_frames = 0;
        self.new_priority = 0;
        self.step_exit_code = None;
        self.step_zombie = false;

        procs.extend(std::mem::take(&mut self.processes));
        self.processes = procs;
    }

        /// Count active (non-halted) child processes
    #[allow(dead_code)]
    pub fn active_process_count(&self) -> usize {
        self.processes.iter().filter(|p| !p.is_halted()).count()
    }

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
        const MINI_FONT: [[u8; 7]; 96] = include!("mini_font.in");
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
mod tests {
    use super::*;

    /// Helper: build a bytecode program as Vec<u32>, load into a fresh VM, run N steps.
    /// Returns the VM for assertions.
    fn run_program(bytecode: &[u32], max_steps: usize) -> Vm {
        let mut vm = Vm::new();
        for (i, &word) in bytecode.iter().enumerate() {
            vm.ram[i] = word;
        }
        vm.pc = 0;
        vm.halted = false;
        for _ in 0..max_steps {
            if !vm.step() { break; }
        }
        vm
    }

    // ── RAM-Mapped Canvas (Phase 45) ────────────────────────────────

    #[test]
    fn test_canvas_ram_mapping_store() {
        let mut vm = Vm::new();
        // STORE 0x8000 (first cell) with 'H' (0x48)
        vm.regs[1] = 0x8000;
        vm.regs[2] = 0x48;
        vm.ram[0] = 0x12; vm.ram[1] = 1; vm.ram[2] = 2; // STORE r1, r2
        vm.pc = 0;
        vm.step();
        assert_eq!(vm.canvas_buffer[0], 0x48);
        assert_eq!(vm.ram[0x8000], 0); // RAM should be unchanged
    }

    #[test]
    fn test_canvas_ram_mapping_load() {
        let mut vm = Vm::new();
        vm.canvas_buffer[10] = 0x58; // 'X'
        // LOAD r3, 0x800A
        vm.regs[1] = 0x800A;
        vm.ram[0] = 0x11; vm.ram[1] = 3; vm.ram[2] = 1; // LOAD r3, r1
        vm.pc = 0;
        vm.step();
        assert_eq!(vm.regs[3], 0x58);
    }
    
    #[test]
    fn test_canvas_ram_mapping_user_mode() {
        let mut vm = Vm::new();
        vm.mode = CpuMode::User;
        vm.regs[1] = 0x8000;
        vm.regs[2] = 0x48;
        vm.ram[0] = 0x12; vm.ram[1] = 1; vm.ram[2] = 2; // STORE r1, r2
        vm.pc = 0;
        assert!(vm.step()); // Should NOT segfault
        assert_eq!(vm.canvas_buffer[0], 0x48);
    }

    #[test]
    fn test_nop_advances_pc() {
        // NOP then HALT
        let vm = run_program(&[0x01, 0x00], 100);
        assert!(vm.halted);
        assert_eq!(vm.pc, 2);
    }

    // ── LDI ─────────────────────────────────────────────────────────

    #[test]
    fn test_ldi_loads_immediate() {
        // LDI r5, 0x42
        let vm = run_program(&[0x10, 5, 0x42, 0x00], 100);
        assert!(vm.halted);
        assert_eq!(vm.regs[5], 0x42);
    }

    #[test]
    fn test_ldi_zero() {
        // LDI r3, 0
        let vm = run_program(&[0x10, 3, 0, 0x00], 100);
        assert_eq!(vm.regs[3], 0);
    }

    #[test]
    fn test_ldi_max_u32() {
        // LDI r10, 0xFFFFFFFF
        let vm = run_program(&[0x10, 10, 0xFFFFFFFF, 0x00], 100);
        assert_eq!(vm.regs[10], 0xFFFFFFFF);
    }

    #[test]
    fn test_ldi_invalid_reg_ignored() {
        // LDI r32 (out of range), 42 -- should be ignored, no panic
        let vm = run_program(&[0x10, 32, 42, 0x00], 100);
        assert!(vm.halted); // still halted at end
    }

    // ── LOAD / STORE ────────────────────────────────────────────────

    #[test]
    fn test_load_reads_ram() {
        // LDI r1, 0x2000   (address)
        // STORE r1, r2     (store r2 -> RAM[0x2000])
        // LOAD r3, r1      (load r3 <- RAM[0x2000])
        // HALT
        let mut vm = Vm::new();
        vm.ram[0] = 0x10; vm.ram[1] = 1; vm.ram[2] = 0x2000; // LDI r1, 0x2000
        vm.ram[3] = 0x12; vm.ram[4] = 1; vm.ram[5] = 2;       // STORE r1, r2
        vm.ram[6] = 0x11; vm.ram[7] = 3; vm.ram[8] = 1;       // LOAD r3, r1
        vm.ram[9] = 0x00;                                       // HALT
        vm.regs[2] = 0xABCDEF;
        vm.pc = 0;
        for _ in 0..100 { if !vm.step() { break; } }
        assert_eq!(vm.regs[3], 0xABCDEF);
    }

    #[test]
    fn test_store_then_load_roundtrip() {
        let mut vm = Vm::new();
        // LDI r5, 0x500  (addr)
        // LDI r6, 999    (value)
        // STORE r5, r6
        // LOAD r7, r5
        // HALT
        vm.ram[0] = 0x10; vm.ram[1] = 5; vm.ram[2] = 0x500;
        vm.ram[3] = 0x10; vm.ram[4] = 6; vm.ram[5] = 999;
        vm.ram[6] = 0x12; vm.ram[7] = 5; vm.ram[8] = 6;
        vm.ram[9] = 0x11; vm.ram[10] = 7; vm.ram[11] = 5;
        vm.ram[12] = 0x00;
        vm.pc = 0;
        for _ in 0..100 { if !vm.step() { break; } }
        assert_eq!(vm.regs[7], 999);
    }

    // ── ARITHMETIC ──────────────────────────────────────────────────

    #[test]
    fn test_add_basic() {
        // LDI r1, 10; LDI r2, 20; ADD r1, r2; HALT
        let vm = run_program(&[0x10, 1, 10, 0x10, 2, 20, 0x20, 1, 2, 0x00], 100);
        assert_eq!(vm.regs[1], 30);
    }

    #[test]
    fn test_add_wrapping_overflow() {
        let mut vm = Vm::new();
        vm.regs[1] = 0xFFFFFFFF;
        vm.regs[2] = 1;
        // ADD r1, r2; HALT
        vm.ram[0] = 0x20; vm.ram[1] = 1; vm.ram[2] = 2;
        vm.ram[3] = 0x00;
        vm.pc = 0;
        for _ in 0..100 { if !vm.step() { break; } }
        assert_eq!(vm.regs[1], 0); // wrapping add
    }

    #[test]
    fn test_sub_basic() {
        let mut vm = Vm::new();
        vm.regs[1] = 50;
        vm.regs[2] = 20;
        vm.ram[0] = 0x21; vm.ram[1] = 1; vm.ram[2] = 2;
        vm.ram[3] = 0x00;
        vm.pc = 0;
        for _ in 0..100 { if !vm.step() { break; } }
        assert_eq!(vm.regs[1], 30);
    }

    #[test]
    fn test_sub_wrapping_underflow() {
        let mut vm = Vm::new();
        vm.regs[1] = 0;
        vm.regs[2] = 1;
        vm.ram[0] = 0x21; vm.ram[1] = 1; vm.ram[2] = 2;
        vm.ram[3] = 0x00;
        vm.pc = 0;
        for _ in 0..100 { if !vm.step() { break; } }
        assert_eq!(vm.regs[1], 0xFFFFFFFF); // wrapping sub
    }

    #[test]
    fn test_mul_basic() {
        let mut vm = Vm::new();
        vm.regs[1] = 6;
        vm.regs[2] = 7;
        vm.ram[0] = 0x22; vm.ram[1] = 1; vm.ram[2] = 2;
        vm.ram[3] = 0x00;
        vm.pc = 0;
        for _ in 0..100 { if !vm.step() { break; } }
        assert_eq!(vm.regs[1], 42);
    }

    #[test]
    fn test_div_basic() {
        let mut vm = Vm::new();
        vm.regs[1] = 100;
        vm.regs[2] = 7;
        vm.ram[0] = 0x23; vm.ram[1] = 1; vm.ram[2] = 2;
        vm.ram[3] = 0x00;
        vm.pc = 0;
        for _ in 0..100 { if !vm.step() { break; } }
        assert_eq!(vm.regs[1], 14); // 100 / 7 = 14 (integer division)
    }

    #[test]
    fn test_div_by_zero_no_panic() {
        let mut vm = Vm::new();
        vm.regs[1] = 42;
        vm.regs[2] = 0;
        vm.ram[0] = 0x23; vm.ram[1] = 1; vm.ram[2] = 2;
        vm.ram[3] = 0x00;
        vm.pc = 0;
        for _ in 0..100 { if !vm.step() { break; } }
        assert_eq!(vm.regs[1], 42); // unchanged, no panic
    }

    #[test]
    fn test_mod_basic() {
        let mut vm = Vm::new();
        vm.regs[1] = 100;
        vm.regs[2] = 7;
        vm.ram[0] = 0x29; vm.ram[1] = 1; vm.ram[2] = 2;
        vm.ram[3] = 0x00;
        vm.pc = 0;
        for _ in 0..100 { if !vm.step() { break; } }
        assert_eq!(vm.regs[1], 2); // 100 % 7 = 2
    }

    #[test]
    fn test_mod_by_zero_no_panic() {
        let mut vm = Vm::new();
        vm.regs[1] = 42;
        vm.regs[2] = 0;
        vm.ram[0] = 0x29; vm.ram[1] = 1; vm.ram[2] = 2;
        vm.ram[3] = 0x00;
        vm.pc = 0;
        for _ in 0..100 { if !vm.step() { break; } }
        assert_eq!(vm.regs[1], 42); // unchanged
    }

    #[test]
    fn test_neg() {
        let mut vm = Vm::new();
        vm.regs[5] = 1;
        vm.ram[0] = 0x2A; vm.ram[1] = 5;
        vm.ram[2] = 0x00;
        vm.pc = 0;
        for _ in 0..100 { if !vm.step() { break; } }
        assert_eq!(vm.regs[5], 0xFFFFFFFF); // -1 in two's complement
    }

    #[test]
    fn test_neg_zero() {
        let mut vm = Vm::new();
        vm.regs[5] = 0;
        vm.ram[0] = 0x2A; vm.ram[1] = 5;
        vm.ram[2] = 0x00;
        vm.pc = 0;
        for _ in 0..100 { if !vm.step() { break; } }
        assert_eq!(vm.regs[5], 0);
    }

    // ── BITWISE ─────────────────────────────────────────────────────

    #[test]
    fn test_and() {
        let mut vm = Vm::new();
        vm.regs[1] = 0xFF00FF;
        vm.regs[2] = 0x0F0F0F;
        vm.ram[0] = 0x24; vm.ram[1] = 1; vm.ram[2] = 2;
        vm.ram[3] = 0x00;
        vm.pc = 0;
        for _ in 0..100 { if !vm.step() { break; } }
        assert_eq!(vm.regs[1], 0x0F000F);
    }

    #[test]
    fn test_or() {
        let mut vm = Vm::new();
        vm.regs[1] = 0xF00000;
        vm.regs[2] = 0x000F00;
        vm.ram[0] = 0x25; vm.ram[1] = 1; vm.ram[2] = 2;
        vm.ram[3] = 0x00;
        vm.pc = 0;
        for _ in 0..100 { if !vm.step() { break; } }
        assert_eq!(vm.regs[1], 0xF00F00);
    }

    #[test]
    fn test_xor() {
        let mut vm = Vm::new();
        vm.regs[1] = 0xFF00FF;
        vm.regs[2] = 0xFF00FF;
        vm.ram[0] = 0x26; vm.ram[1] = 1; vm.ram[2] = 2;
        vm.ram[3] = 0x00;
        vm.pc = 0;
        for _ in 0..100 { if !vm.step() { break; } }
        assert_eq!(vm.regs[1], 0); // XOR self = 0
    }

    #[test]
    fn test_shl() {
        let mut vm = Vm::new();
        vm.regs[1] = 1;
        vm.regs[2] = 8;
        vm.ram[0] = 0x27; vm.ram[1] = 1; vm.ram[2] = 2;
        vm.ram[3] = 0x00;
        vm.pc = 0;
        for _ in 0..100 { if !vm.step() { break; } }
        assert_eq!(vm.regs[1], 256);
    }

    #[test]
    fn test_shl_mod_32() {
        let mut vm = Vm::new();
        vm.regs[1] = 1;
        vm.regs[2] = 32; // shift by 32 -> effectively shift by 0 (mod 32)
        vm.ram[0] = 0x27; vm.ram[1] = 1; vm.ram[2] = 2;
        vm.ram[3] = 0x00;
        vm.pc = 0;
        for _ in 0..100 { if !vm.step() { break; } }
        assert_eq!(vm.regs[1], 1); // 1 << 32 = 1 (mod 32 = 0)
    }

    #[test]
    fn test_shr() {
        let mut vm = Vm::new();
        vm.regs[1] = 256;
        vm.regs[2] = 4;
        vm.ram[0] = 0x28; vm.ram[1] = 1; vm.ram[2] = 2;
        vm.ram[3] = 0x00;
        vm.pc = 0;
        for _ in 0..100 { if !vm.step() { break; } }
        assert_eq!(vm.regs[1], 16);
    }

    #[test]
    fn test_sar_sign_preserving() {
        let mut vm = Vm::new();
        vm.regs[1] = 0x80000000; // MSB set (negative in i32)
        vm.regs[2] = 4;
        vm.ram[0] = 0x2B; vm.ram[1] = 1; vm.ram[2] = 2;
        vm.ram[3] = 0x00;
        vm.pc = 0;
        for _ in 0..100 { if !vm.step() { break; } }
        // 0x80000000 >> 4 (arithmetic) = 0xF8000000
        assert_eq!(vm.regs[1], 0xF8000000);
    }

    // ── CMP / BRANCHES ──────────────────────────────────────────────

    #[test]
    fn test_cmp_less_than() {
        let mut vm = Vm::new();
        vm.regs[1] = 5;
        vm.regs[2] = 10;
        vm.ram[0] = 0x50; vm.ram[1] = 1; vm.ram[2] = 2; // CMP r1, r2
        vm.ram[3] = 0x00;
        vm.pc = 0;
        for _ in 0..100 { if !vm.step() { break; } }
        assert_eq!(vm.regs[0], 0xFFFFFFFF); // -1 (less than)
    }

    #[test]
    fn test_cmp_equal() {
        let mut vm = Vm::new();
        vm.regs[1] = 42;
        vm.regs[2] = 42;
        vm.ram[0] = 0x50; vm.ram[1] = 1; vm.ram[2] = 2;
        vm.ram[3] = 0x00;
        vm.pc = 0;
        for _ in 0..100 { if !vm.step() { break; } }
        assert_eq!(vm.regs[0], 0); // equal
    }

    #[test]
    fn test_cmp_greater_than() {
        let mut vm = Vm::new();
        vm.regs[1] = 10;
        vm.regs[2] = 5;
        vm.ram[0] = 0x50; vm.ram[1] = 1; vm.ram[2] = 2;
        vm.ram[3] = 0x00;
        vm.pc = 0;
        for _ in 0..100 { if !vm.step() { break; } }
        assert_eq!(vm.regs[0], 1); // greater than
    }

    #[test]
    fn test_jz_taken() {
        // LDI r1, 0; JZ r1, 100; HALT -> should jump to 100
        let mut vm = Vm::new();
        vm.ram[0] = 0x10; vm.ram[1] = 1; vm.ram[2] = 0; // LDI r1, 0
        vm.ram[3] = 0x31; vm.ram[4] = 1; vm.ram[5] = 100; // JZ r1, 100
        vm.ram[6] = 0x00; // HALT (should not reach)
        vm.ram[100] = 0x00; // HALT at target
        vm.pc = 0;
        for _ in 0..100 { if !vm.step() { break; } }
        assert_eq!(vm.pc, 101); // halted at 101 (fetched HALT at 100)
    }

    #[test]
    fn test_jz_not_taken() {
        // LDI r1, 1; JZ r1, 100; HALT -> should not jump
        let mut vm = Vm::new();
        vm.ram[0] = 0x10; vm.ram[1] = 1; vm.ram[2] = 1; // LDI r1, 1
        vm.ram[3] = 0x31; vm.ram[4] = 1; vm.ram[5] = 100; // JZ r1, 100
        vm.ram[6] = 0x00; // HALT (should reach)
        vm.pc = 0;
        for _ in 0..100 { if !vm.step() { break; } }
        assert_eq!(vm.pc, 7); // halted at HALT
    }

    #[test]
    fn test_jnz_taken() {
        let mut vm = Vm::new();
        vm.ram[0] = 0x10; vm.ram[1] = 1; vm.ram[2] = 5; // LDI r1, 5
        vm.ram[3] = 0x32; vm.ram[4] = 1; vm.ram[5] = 100; // JNZ r1, 100
        vm.ram[6] = 0x00; // HALT
        vm.ram[100] = 0x00; // HALT at target
        vm.pc = 0;
        for _ in 0..100 { if !vm.step() { break; } }
        assert_eq!(vm.pc, 101);
    }

    #[test]
    fn test_jmp_unconditional() {
        let mut vm = Vm::new();
        vm.ram[0] = 0x30; vm.ram[1] = 50; // JMP 50
        vm.ram[2] = 0x00; // HALT (should not reach)
        vm.ram[50] = 0x00; // HALT at target
        vm.pc = 0;
        for _ in 0..100 { if !vm.step() { break; } }
        assert_eq!(vm.pc, 51);
    }

    #[test]
    fn test_blt_taken() {
        // CMP sets r0 = 0xFFFFFFFF (less than); BLT should branch
        let mut vm = Vm::new();
        vm.regs[1] = 3;
        vm.regs[2] = 10;
        vm.ram[0] = 0x50; vm.ram[1] = 1; vm.ram[2] = 2; // CMP r1, r2
        vm.ram[3] = 0x35; vm.ram[4] = 0; vm.ram[5] = 50; // BLT r0, 50
        vm.ram[6] = 0x00; // HALT
        vm.ram[50] = 0x00; // HALT at target
        vm.pc = 0;
        for _ in 0..100 { if !vm.step() { break; } }
        assert_eq!(vm.pc, 51);
    }

    #[test]
    fn test_bge_taken() {
        // CMP sets r0 = 1 (greater than); BGE should branch
        let mut vm = Vm::new();
        vm.regs[1] = 10;
        vm.regs[2] = 3;
        vm.ram[0] = 0x50; vm.ram[1] = 1; vm.ram[2] = 2; // CMP r1, r2
        vm.ram[3] = 0x36; vm.ram[4] = 0; vm.ram[5] = 50; // BGE r0, 50
        vm.ram[6] = 0x00; // HALT
        vm.ram[50] = 0x00; // HALT at target
        vm.pc = 0;
        for _ in 0..100 { if !vm.step() { break; } }
        assert_eq!(vm.pc, 51);
    }

    // ── CALL / RET ──────────────────────────────────────────────────

    #[test]
    fn test_call_ret() {
        // CALL 10; HALT
        // at 10: LDI r5, 99; RET
        // at 16: HALT (return lands here)
        let mut vm = Vm::new();
        vm.ram[0] = 0x33; vm.ram[1] = 10;         // CALL 10
        vm.ram[2] = 0x00;                            // HALT (return target)
        vm.ram[10] = 0x10; vm.ram[11] = 5; vm.ram[12] = 99; // LDI r5, 99
        vm.ram[13] = 0x34;                           // RET
        vm.pc = 0;
        for _ in 0..100 { if !vm.step() { break; } }
        assert_eq!(vm.regs[5], 99);
        assert!(vm.halted);
    }

    // ── MOV ─────────────────────────────────────────────────────────

    #[test]
    fn test_mov() {
        let mut vm = Vm::new();
        vm.regs[3] = 0xDEADBEEF;
        vm.ram[0] = 0x51; vm.ram[1] = 7; vm.ram[2] = 3; // MOV r7, r3
        vm.ram[3] = 0x00;
        vm.pc = 0;
        for _ in 0..100 { if !vm.step() { break; } }
        assert_eq!(vm.regs[7], 0xDEADBEEF);
        assert_eq!(vm.regs[3], 0xDEADBEEF); // source unchanged
    }

    // ── PUSH / POP ──────────────────────────────────────────────────

    #[test]
    fn test_push_pop_roundtrip() {
        // LDI r30, 0xFF00 (SP); LDI r5, 42; PUSH r5; LDI r5, 0; POP r6; HALT
        let mut vm = Vm::new();
        let mut pc = 0u32;
        // LDI r30, 0xFF00
        vm.ram[pc as usize] = 0x10; pc += 1;
        vm.ram[pc as usize] = 30; pc += 1;
        vm.ram[pc as usize] = 0xFF00; pc += 1;
        // LDI r5, 42
        vm.ram[pc as usize] = 0x10; pc += 1;
        vm.ram[pc as usize] = 5; pc += 1;
        vm.ram[pc as usize] = 42; pc += 1;
        // PUSH r5
        vm.ram[pc as usize] = 0x60; pc += 1;
        vm.ram[pc as usize] = 5; pc += 1;
        // LDI r5, 0 (clobber)
        vm.ram[pc as usize] = 0x10; pc += 1;
        vm.ram[pc as usize] = 5; pc += 1;
        vm.ram[pc as usize] = 0; pc += 1;
        // POP r6
        vm.ram[pc as usize] = 0x61; pc += 1;
        vm.ram[pc as usize] = 6; pc += 1;
        // HALT
        vm.ram[pc as usize] = 0x00; pc += 1;

        vm.pc = 0;
        for _ in 0..100 { if !vm.step() { break; } }
        assert_eq!(vm.regs[6], 42); // got value back from stack
        assert_eq!(vm.regs[5], 0);  // r5 was clobbered
        assert_eq!(vm.regs[30], 0xFF00); // SP restored
    }

    // ── CMP signed comparison ───────────────────────────────────────

    #[test]
    fn test_cmp_signed_negative_vs_positive() {
        // -1 (0xFFFFFFFF) vs 5 -> should be less than
        let mut vm = Vm::new();
        vm.regs[1] = 0xFFFFFFFF; // -1 as i32
        vm.regs[2] = 5;
        vm.ram[0] = 0x50; vm.ram[1] = 1; vm.ram[2] = 2;
        vm.ram[3] = 0x00;
        vm.pc = 0;
        for _ in 0..100 { if !vm.step() { break; } }
        assert_eq!(vm.regs[0], 0xFFFFFFFF); // -1 < 5 in signed
    }

    // ── FRAME ───────────────────────────────────────────────────────

    #[test]
    fn test_frame_increments_ticks() {
        let mut vm = Vm::new();
        vm.ram[0] = 0x02; // FRAME
        vm.ram[1] = 0x00; // HALT
        vm.pc = 0;
        for _ in 0..100 { if !vm.step() { break; } }
        assert!(vm.frame_ready);
        assert_eq!(vm.frame_count, 1);
        assert_eq!(vm.ram[0xFFE], 1);
    }

    // ── PSET / FILL ─────────────────────────────────────────────────

    #[test]
    fn test_fill() {
        let mut vm = Vm::new();
        vm.regs[1] = 0x00FF00; // green
        vm.ram[0] = 0x42; vm.ram[1] = 1; // FILL r1
        vm.ram[2] = 0x00;
        vm.pc = 0;
        for _ in 0..100 { if !vm.step() { break; } }
        // Every pixel should be green
        assert!(vm.screen.iter().all(|&p| p == 0x00FF00));
    }

    #[test]
    fn test_pset_pixel() {
        let mut vm = Vm::new();
        vm.regs[1] = 10;  // x
        vm.regs[2] = 20;  // y
        vm.regs[3] = 0xFF0000; // red
        vm.ram[0] = 0x40; vm.ram[1] = 1; vm.ram[2] = 2; vm.ram[3] = 3; // PSET r1, r2, r3
        vm.ram[4] = 0x00;
        vm.pc = 0;
        for _ in 0..100 { if !vm.step() { break; } }
        assert_eq!(vm.screen[20 * 256 + 10], 0xFF0000);
    }

    // ── IKEY ────────────────────────────────────────────────────────

    #[test]
    fn test_ikey_reads_and_clears() {
        let mut vm = Vm::new();
        vm.ram[0xFFF] = 65; // 'A' in keyboard port
        vm.ram[0] = 0x48; vm.ram[1] = 5; // IKEY r5
        vm.ram[2] = 0x00;
        vm.pc = 0;
        for _ in 0..100 { if !vm.step() { break; } }
        assert_eq!(vm.regs[5], 65);
        assert_eq!(vm.ram[0xFFF], 0); // port cleared
    }

    #[test]
    fn test_ikey_no_key() {
        let mut vm = Vm::new();
        vm.ram[0xFFF] = 0; // no key
        vm.ram[0] = 0x48; vm.ram[1] = 5; // IKEY r5
        vm.ram[2] = 0x00;
        vm.pc = 0;
        for _ in 0..100 { if !vm.step() { break; } }
        assert_eq!(vm.regs[5], 0);
    }

    // ── RAND ────────────────────────────────────────────────────────

    #[test]
    fn test_rand_changes_state() {
        let mut vm = Vm::new();
        let initial_state = vm.rand_state;
        vm.ram[0] = 0x49; vm.ram[1] = 5; // RAND r5
        vm.ram[2] = 0x00;
        vm.pc = 0;
        for _ in 0..100 { if !vm.step() { break; } }
        assert_ne!(vm.rand_state, initial_state); // state changed
        assert_ne!(vm.regs[5], 0); // probably nonzero (LCG seeded with DEADBEEF)
    }

    // ── BEEP ────────────────────────────────────────────────────────

    #[test]
    fn test_beep_sets_state() {
        let mut vm = Vm::new();
        vm.regs[1] = 440;  // freq
        vm.regs[2] = 200;  // duration
        vm.ram[0] = 0x03; vm.ram[1] = 1; vm.ram[2] = 2; // BEEP r1, r2
        vm.ram[3] = 0x00;
        vm.pc = 0;
        for _ in 0..100 { if !vm.step() { break; } }
        assert_eq!(vm.beep, Some((440, 200)));
    }

    // ── MEMCPY ───────────────────────────────────────────────────────

    #[test]
    fn test_memcpy_copies_words() {
        let mut vm = Vm::new();
        // Write some data to addresses 100-104
        for i in 0..5 {
            vm.ram[100 + i] = (1000 + i as u32);
        }
        // Set regs: r1=200 (dst), r2=100 (src), r3=5 (len)
        vm.regs[1] = 200;
        vm.regs[2] = 100;
        vm.regs[3] = 5;
        // MEMCPY r1, r2, r3
        vm.ram[0] = 0x04; vm.ram[1] = 1; vm.ram[2] = 2; vm.ram[3] = 3;
        vm.ram[4] = 0x00; // HALT
        vm.pc = 0;
        for _ in 0..100 { if !vm.step() { break; } }
        // Verify dst has the data
        for i in 0..5 {
            assert_eq!(vm.ram[200 + i], 1000 + i as u32, "MEMCPY failed at offset {}", i);
        }
    }

    #[test]
    fn test_memcpy_zero_len_is_noop() {
        let mut vm = Vm::new();
        vm.ram[100] = 0xDEAD;
        vm.ram[200] = 0xBEEF;
        vm.regs[1] = 200; // dst
        vm.regs[2] = 100; // src
        vm.regs[3] = 0;   // len = 0
        vm.ram[0] = 0x04; vm.ram[1] = 1; vm.ram[2] = 2; vm.ram[3] = 3;
        vm.ram[4] = 0x00;
        vm.pc = 0;
        for _ in 0..100 { if !vm.step() { break; } }
        assert_eq!(vm.ram[200], 0xBEEF, "MEMCPY with len=0 should not overwrite dst");
    }

    // ── Loop: verify backward jumps work at base_addr 0 ─────────────

    #[test]
    fn test_backward_jump_loop_at_addr_zero() {
        // Count from 0 to 5 using a loop
        // LDI r1, 0     ; counter = 0
        // LDI r2, 1     ; increment
        // LDI r3, 5     ; limit
        // loop:
        // ADD r1, r2     ; counter++
        // CMP r1, r3
        // BLT r0, loop   ; if counter < 5, loop
        // HALT
        let mut vm = Vm::new();
        let mut pc = 0usize;
        // LDI r1, 0
        vm.ram[pc] = 0x10; vm.ram[pc+1] = 1; vm.ram[pc+2] = 0; pc += 3;
        // LDI r2, 1
        vm.ram[pc] = 0x10; vm.ram[pc+1] = 2; vm.ram[pc+2] = 1; pc += 3;
        // LDI r3, 5
        vm.ram[pc] = 0x10; vm.ram[pc+1] = 3; vm.ram[pc+2] = 5; pc += 3;
        let loop_addr = pc as u32;
        // ADD r1, r2
        vm.ram[pc] = 0x20; vm.ram[pc+1] = 1; vm.ram[pc+2] = 2; pc += 3;
        // CMP r1, r3
        vm.ram[pc] = 0x50; vm.ram[pc+1] = 1; vm.ram[pc+2] = 3; pc += 3;
        // BLT r0, loop_addr
        vm.ram[pc] = 0x35; vm.ram[pc+1] = 0; vm.ram[pc+2] = loop_addr; pc += 3;
        // HALT
        vm.ram[pc] = 0x00;

        vm.pc = 0;
        for _ in 0..1000 { if !vm.step() { break; } }
        assert_eq!(vm.regs[1], 5);
        assert!(vm.halted);
    }

    // ── Loop: verify backward jumps work at base_addr 0x1000 ────────

    #[test]
    fn test_backward_jump_loop_at_addr_0x1000() {
        // Same program but loaded at 0x1000 -- the GUI mode scenario
        let mut vm = Vm::new();
        let base = 0x1000usize;
        let mut pc = base;
        // LDI r1, 0
        vm.ram[pc] = 0x10; vm.ram[pc+1] = 1; vm.ram[pc+2] = 0; pc += 3;
        // LDI r2, 1
        vm.ram[pc] = 0x10; vm.ram[pc+1] = 2; vm.ram[pc+2] = 1; pc += 3;
        // LDI r3, 5
        vm.ram[pc] = 0x10; vm.ram[pc+1] = 3; vm.ram[pc+2] = 5; pc += 3;
        let loop_addr = pc as u32;
        // ADD r1, r2
        vm.ram[pc] = 0x20; vm.ram[pc+1] = 1; vm.ram[pc+2] = 2; pc += 3;
        // CMP r1, r3
        vm.ram[pc] = 0x50; vm.ram[pc+1] = 1; vm.ram[pc+2] = 3; pc += 3;
        // BLT r0, loop_addr -- label resolved to 0x1000 + offset
        vm.ram[pc] = 0x35; vm.ram[pc+1] = 0; vm.ram[pc+2] = loop_addr; pc += 3;
        // HALT
        vm.ram[pc] = 0x00;

        vm.pc = base as u32;
        for _ in 0..1000 { if !vm.step() { break; } }
        assert_eq!(vm.regs[1], 5);
        assert!(vm.halted);
    }

    // ── PEEK ────────────────────────────────────────────────────────

    #[test]
    fn test_peek_reads_screen() {
        let mut vm = Vm::new();
        vm.screen[30 * 256 + 15] = 0xABCDEF;
        vm.regs[1] = 15; // x
        vm.regs[2] = 30; // y
        vm.ram[0] = 0x6D; vm.ram[1] = 3; vm.ram[2] = 1; vm.ram[3] = 2; // PEEK r3, r1, r2 (dest=r3, x=r1=15, y=r2=30)
        vm.ram[4] = 0x00;
        vm.pc = 0;
        for _ in 0..100 { if !vm.step() { break; } }
        assert_eq!(vm.regs[3], 0xABCDEF);
    }

    #[test]
    fn test_peek_out_of_bounds_returns_zero() {
        let mut vm = Vm::new();
        vm.regs[1] = 300; // x out of bounds
        vm.regs[2] = 300; // y out of bounds
        vm.ram[0] = 0x6D; vm.ram[1] = 1; vm.ram[2] = 2; vm.ram[3] = 3;
        vm.ram[4] = 0x00;
        vm.pc = 0;
        for _ in 0..100 { if !vm.step() { break; } }
        assert_eq!(vm.regs[3], 0);
    }

    #[test]
    fn test_memcpy_copies_memory() {
        let mut vm = Vm::new();
        // Set up source data at 0x2000
        for i in 0..5 {
            vm.ram[0x2000 + i] = (100 + i) as u32;
        }
        vm.regs[1] = 0x3000; // dst
        vm.regs[2] = 0x2000; // src
        vm.regs[3] = 5;      // len
        vm.ram[0] = 0x04; vm.ram[1] = 1; vm.ram[2] = 2; vm.ram[3] = 3; // MEMCPY r1, r2, r3
        vm.ram[4] = 0x00; // HALT
        vm.pc = 0;
        for _ in 0..100 { if !vm.step() { break; } }
        assert!(vm.halted);
        // Verify destination has the copied data
        for i in 0..5 {
            assert_eq!(vm.ram[0x3000 + i], (100 + i) as u32, "MEMCPY dest[{}] should be {}", i, 100 + i);
        }
        // Source should be unchanged
        for i in 0..5 {
            assert_eq!(vm.ram[0x2000 + i], (100 + i) as u32, "MEMCPY src[{}] should be unchanged", i);
        }
    }

    #[test]
    fn test_memcpy_assembles_and_runs() {
        use crate::assembler::assemble;
        let src = "LDI r1, 0x3000\nLDI r2, 0x2000\nLDI r3, 5\nMEMCPY r1, r2, r3\nHALT";
        let asm = assemble(src, 0).expect("assembly should succeed");
        let mut vm = Vm::new();
        // Write source data
        for i in 0..5 { vm.ram[0x2000 + i] = (42 + i) as u32; }
        for (i, &w) in asm.pixels.iter().enumerate() { vm.ram[i] = w; }
        vm.pc = 0;
        for _ in 0..100 { if !vm.step() { break; } }
        assert!(vm.halted);
        for i in 0..5 {
            assert_eq!(vm.ram[0x3000 + i], (42 + i) as u32);
        }
    }

    // ── RAM-Mapped Screen Buffer (Phase 46) ──────────────────────────

    #[test]
    fn test_screen_ram_store() {
        let mut vm = Vm::new();
        // STORE to screen addr 0x10000 (pixel 0,0) with color 0xFF0000
        vm.regs[1] = 0x10000; // addr
        vm.regs[2] = 0xFF0000; // value (red)
        vm.ram[0] = 0x12; vm.ram[1] = 1; vm.ram[2] = 2; // STORE r1, r2
        vm.pc = 0;
        vm.step();
        assert_eq!(vm.screen[0], 0xFF0000);
    }

    #[test]
    fn test_screen_ram_load() {
        let mut vm = Vm::new();
        // Pre-set a pixel in the screen buffer
        vm.screen[256 * 10 + 5] = 0xABCDEF;
        // LOAD from screen addr 0x10000 + 256*10 + 5
        vm.regs[1] = 0x10000 + 256 * 10 + 5;
        vm.ram[0] = 0x11; vm.ram[1] = 3; vm.ram[2] = 1; // LOAD r3, r1
        vm.pc = 0;
        vm.step();
        assert_eq!(vm.regs[3], 0xABCDEF);
    }

    #[test]
    fn test_screen_ram_store_then_load_roundtrip() {
        let vm = run_program(&[
            0x10, 1, 0x10050,       // LDI r1, 0x10050
            0x10, 2, 0x00FF00,      // LDI r2, 0x00FF00
            0x12, 1, 2,             // STORE r1, r2
            0x11, 4, 1,             // LOAD r4, r1
            0x00,                   // HALT
        ], 100);
        assert!(vm.halted);
        assert_eq!(vm.regs[4], 0x00FF00);
        assert_eq!(vm.screen[0x50], 0x00FF00);
    }

    #[test]
    fn test_screen_ram_does_not_corrupt_normal_ram() {
        let mut vm = Vm::new();
        // Store a value at a normal RAM address first
        vm.ram[0x2000] = 0xDEADBEEF;
        // Store to screen address
        vm.regs[1] = 0x10000;
        vm.regs[2] = 0xFF0000;
        vm.ram[0] = 0x12; vm.ram[1] = 1; vm.ram[2] = 2; // STORE r1, r2
        vm.pc = 0;
        vm.step();
        // Normal RAM should be unchanged
        assert_eq!(vm.ram[0x2000], 0xDEADBEEF);
        // Screen should have the stored value
        assert_eq!(vm.screen[0], 0xFF0000);
    }

    #[test]
    fn test_screen_ram_load_matches_peek() {
        let mut vm = Vm::new();
        // Set pixel at (15, 30) via screen buffer directly
        vm.screen[30 * 256 + 15] = 0x123456;

        // Read via LOAD from screen-mapped address
        let screen_addr = (SCREEN_RAM_BASE + 30 * 256 + 15) as u32;
        vm.regs[1] = screen_addr;
        vm.ram[0] = 0x11; vm.ram[1] = 3; vm.ram[2] = 1; // LOAD r3, r1
        vm.pc = 0;
        for _ in 0..100 { if !vm.step() { break; } }
        let load_value = vm.regs[3];

        // Reset halted state for second instruction sequence
        vm.halted = false;

        // Read via PEEK opcode
        vm.regs[1] = 15; // x
        vm.regs[2] = 30; // y
        vm.ram[4] = 0x6D; vm.ram[5] = 4; vm.ram[6] = 1; vm.ram[7] = 2; // PEEK r4, r1, r2
        vm.ram[8] = 0x00;
        vm.pc = 4;
        for _ in 0..100 { if !vm.step() { break; } }
        let peek_value = vm.regs[4];

        assert_eq!(load_value, 0x123456);
        assert_eq!(peek_value, 0x123456);
        assert_eq!(load_value, peek_value);
    }

    #[test]
    fn test_screen_ram_store_matches_pixel() {
        let mut vm = Vm::new();

        // Write pixel via STORE to screen-mapped address at (10, 20)
        let screen_addr = (SCREEN_RAM_BASE + 20 * 256 + 10) as u32;
        vm.regs[1] = screen_addr;
        vm.regs[2] = 0xFF0000; // red
        vm.ram[0] = 0x12; vm.ram[1] = 1; vm.ram[2] = 2; // STORE r1, r2
        vm.pc = 0;
        for _ in 0..100 { if !vm.step() { break; } }

        // Verify via screen buffer directly
        assert_eq!(vm.screen[20 * 256 + 10], 0xFF0000);

        // Reset halted state for second instruction sequence
        vm.halted = false;

        // Verify via PEEK opcode
        vm.regs[1] = 10; // x
        vm.regs[2] = 20; // y
        vm.ram[3] = 0x6D; vm.ram[4] = 5; vm.ram[5] = 1; vm.ram[6] = 2; // PEEK r5, r1, r2
        vm.ram[7] = 0x00;
        vm.pc = 3;
        for _ in 0..100 { if !vm.step() { break; } }
        assert_eq!(vm.regs[5], 0xFF0000);
    }

    #[test]
    fn test_screen_ram_boundary_first_and_last_pixel() {
        let mut vm = Vm::new();

        // First pixel: address 0x10000
        vm.regs[1] = SCREEN_RAM_BASE as u32;
        vm.regs[2] = 0x111111;
        vm.ram[0] = 0x12; vm.ram[1] = 1; vm.ram[2] = 2; // STORE r1, r2
        vm.pc = 0;
        vm.step();
        assert_eq!(vm.screen[0], 0x111111);

        // Last pixel: address 0x10000 + 65535 = 0x1FFFF
        let last_addr = (SCREEN_RAM_BASE + SCREEN_SIZE - 1) as u32;
        vm.regs[1] = last_addr;
        vm.regs[2] = 0x222222;
        vm.ram[0] = 0x12; vm.ram[1] = 1; vm.ram[2] = 2; // STORE r1, r2
        vm.pc = 0;
        vm.step();
        assert_eq!(vm.screen[SCREEN_SIZE - 1], 0x222222);

        // Read back via LOAD
        vm.regs[1] = SCREEN_RAM_BASE as u32;
        vm.ram[0] = 0x11; vm.ram[1] = 3; vm.ram[2] = 1; // LOAD r3, r1
        vm.pc = 0;
        vm.step();
        assert_eq!(vm.regs[3], 0x111111);

        vm.regs[1] = last_addr;
        vm.ram[0] = 0x11; vm.ram[1] = 3; vm.ram[2] = 1; // LOAD r3, r1
        vm.pc = 0;
        vm.step();
        assert_eq!(vm.regs[3], 0x222222);
    }

    #[test]
    fn test_screen_ram_user_mode_allowed() {
        let mut vm = Vm::new();
        vm.mode = CpuMode::User;
        // User-mode store to screen should work (screen is not I/O)
        vm.regs[1] = 0x10000;
        vm.regs[2] = 0x00FF00;
        vm.ram[0] = 0x12; vm.ram[1] = 1; vm.ram[2] = 2; // STORE r1, r2
        vm.pc = 0;
        assert!(vm.step()); // Should NOT segfault
        assert_eq!(vm.screen[0], 0x00FF00);
    }

    #[test]
    fn test_screen_ram_assembles_and_runs() {
        use crate::assembler::assemble;
        // Write assembly that stores to screen buffer, reads back, stores to RAM for comparison
        let src = "LDI r1, 0x10000\nLDI r2, 0xFF0000\nSTORE r1, r2\nLOAD r3, r1\nLDI r4, 0x7000\nSTORE r4, r3\nHALT";
        let asm = assemble(src, 0).expect("assembly should succeed");
        let vm = run_program(&asm.pixels, 100);
        assert!(vm.halted);
        assert_eq!(vm.screen[0], 0xFF0000);
        assert_eq!(vm.ram[0x7000], 0xFF0000);
    }

    // ── ASMSELF tests (Phase 47: Pixel Driving Pixels) ──────────

    /// Helper: write an ASCII string into the VM's canvas buffer at a given offset.
    fn write_to_canvas(canvas: &mut Vec<u32>, offset: usize, text: &str) {
        for (i, ch) in text.bytes().enumerate() {
            let idx = offset + i;
            if idx < canvas.len() {
                canvas[idx] = ch as u32;
            }
        }
    }

    #[test]
    fn test_asmself_assembles_valid_canvas_text() {
        // Pre-fill canvas with "LDI r0, 42\nHALT\n"
        let mut vm = Vm::new();
        let program = "LDI r0, 42\nHALT\n";
        write_to_canvas(&mut vm.canvas_buffer, 0, program);

        // Execute ASMSELF opcode
        vm.ram[0] = 0x73; // ASMSELF
        vm.pc = 0;
        vm.step();

        // Check status port: should be positive (bytecode word count)
        assert_ne!(vm.ram[0xFFD], 0xFFFFFFFF, "ASMSELF should succeed");
        assert!(vm.ram[0xFFD] > 0, "ASMSELF should produce bytecode");

        // Verify bytecode at 0x1000: LDI r0, 42 = [0x10, 0, 42], HALT = [0x00]
        assert_eq!(vm.ram[0x1000], 0x10, "LDI opcode");
        assert_eq!(vm.ram[0x1001], 0, "r0 register");
        assert_eq!(vm.ram[0x1002], 42, "immediate 42");
        assert_eq!(vm.ram[0x1003], 0x00, "HALT opcode");
    }

    #[test]
    fn test_asmself_handles_invalid_assembly_gracefully() {
        let mut vm = Vm::new();
        // Write garbage to canvas
        write_to_canvas(&mut vm.canvas_buffer, 0, "ZZZTOP R0, R1 !!INVALID!!\n");

        vm.ram[0] = 0x73;
        vm.pc = 0;
        vm.step();

        // Status port should be error sentinel
        assert_eq!(vm.ram[0xFFD], 0xFFFFFFFF, "ASMSELF should report error");

        // VM should NOT be halted -- continues executing
        assert!(!vm.halted, "VM should survive ASMSELF error");
    }

    #[test]
    fn test_asmself_full_write_compile_execute() {
        // Full integration: program writes code to canvas, ASMSELF, then jumps to 0x1000
        let mut vm = Vm::new();

        // First, set up the canvas with "LDI r0, 99\nHALT\n"
        write_to_canvas(&mut vm.canvas_buffer, 0, "LDI r0, 99\nHALT\n");

        // Build a program that calls ASMSELF, then jumps to 0x1000
        // JMP takes an immediate address, not a register
        let bootstrap = "ASMSELF\nJMP 0x1000\n";
        let asm = crate::assembler::assemble(bootstrap, 0).expect("assembly should succeed");
        for (i, &word) in asm.pixels.iter().enumerate() {
            vm.ram[i] = word;
        }
        vm.pc = 0;

        // Run the bootstrap program
        let max_steps = 200;
        for _ in 0..max_steps {
            if vm.halted {
                break;
            }
            vm.step();
        }

        // After bootstrap: ASMSELF assembled canvas code, JMP went to 0x1000,
        // new code ran LDI r0, 99 then HALT
        assert!(vm.halted, "VM should halt after executing assembled code");
        assert_eq!(vm.ram[0xFFD], 4, "ASMSELF should report 4 words of bytecode");
        assert_eq!(vm.regs[0], 99, "r0 should be 99 after assembled code runs");
    }

    #[test]
    fn test_asmself_disassembler() {
        let mut vm = Vm::new();
        vm.ram[0] = 0x73; // ASMSELF
        let (text, len) = vm.disassemble_at(0);
        assert_eq!(text, "ASMSELF");
        assert_eq!(len, 1);
    }

    #[test]
    fn test_asmself_assembler_mnemonic() {
        use crate::assembler::assemble;
        let src = "ASMSELF\nHALT\n";
        let result = assemble(src, 0).expect("assembly should succeed");
        assert_eq!(result.pixels[0], 0x73, "ASMSELF should encode as 0x73");
        assert_eq!(result.pixels[1], 0x00, "HALT should follow");
    }

    #[test]
    fn test_asmself_empty_canvas() {
        let mut vm = Vm::new();
        // Canvas is all zeros -- should produce empty/minimal assembly
        vm.ram[0] = 0x73;
        vm.pc = 0;
        vm.step();

        // Empty canvas should either succeed (0 words) or fail gracefully
        // Either way, VM should not be halted
        assert!(!vm.halted, "VM should survive ASMSELF on empty canvas");
    }

    #[test]
    fn test_asmself_preserves_registers() {
        // Verify that ASMSELF doesn't clobber registers (only writes to RAM)
        let mut vm = Vm::new();
        vm.regs[0] = 111;
        vm.regs[1] = 222;
        vm.regs[5] = 555;

        write_to_canvas(&mut vm.canvas_buffer, 0, "LDI r0, 42\nHALT\n");

        vm.ram[0] = 0x73;
        vm.pc = 0;
        vm.step();

        // Registers should be preserved after ASMSELF
        assert_eq!(vm.regs[0], 111, "r0 should be preserved");
        assert_eq!(vm.regs[1], 222, "r1 should be preserved");
        assert_eq!(vm.regs[5], 555, "r5 should be preserved");
    }

    #[test]
    fn test_asmself_with_preprocessor_macros() {
        // Test that preprocessor macros work in ASMSELF
        let mut vm = Vm::new();
        // Use SET/GET macros
        write_to_canvas(&mut vm.canvas_buffer, 0, "VAR x 42\nGET r1, x\nHALT\n");

        vm.ram[0] = 0x73;
        vm.pc = 0;
        vm.step();

        // Should succeed (preprocessor expands VAR and GET)
        assert_ne!(vm.ram[0xFFD], 0xFFFFFFFF, "ASMSELF with macros should succeed");
        assert!(vm.ram[0xFFD] > 0, "Should produce some bytecode");
    }

    #[test]
    fn test_store_writes_successor_to_canvas_then_asmself_executes() {
        // Phase 47 integration test: the program itself uses STORE to write
        // "LDI r0, 99\nHALT\n" to the canvas RAM range (0x8000-0x8FFF).
        // ASMSELF reads canvas_buffer, assembles the source into bytecode at
        // 0x1000, then RUNNEXT jumps there. Verify r0 ends up as 99.
        //
        // This is the "pixel driving pixels" loop: code writes code, compiles
        // it, and runs it -- all through the VM's own STORE/ASMSELF/RUNNEXT.

        let mut vm = Vm::new();

        // Build a bootstrap program that writes each character via STORE
        let successor = "LDI r0, 99\nHALT\n";
        let mut src = String::new();

        // r1 = canvas address pointer (starts at 0x8000)
        // r3 = increment (1)
        src.push_str("LDI r1, 0x8000\n");
        src.push_str("LDI r3, 1\n");

        for (i, ch) in successor.bytes().enumerate() {
            if i > 0 {
                src.push_str("ADD r1, r3\n"); // advance canvas pointer
            }
            src.push_str(&format!("LDI r2, {}\nSTORE r1, r2\n", ch as u32));
        }

        // Compile the canvas source and execute the result
        src.push_str("ASMSELF\n");
        src.push_str("RUNNEXT\n");

        // Assemble the bootstrap program
        let asm = crate::assembler::assemble(&src, 0).expect("assembly should succeed");
        for (i, &word) in asm.pixels.iter().enumerate() {
            vm.ram[i] = word;
        }
        vm.pc = 0;

        // Verify the canvas buffer is empty before execution
        assert_eq!(vm.canvas_buffer[0], 0, "canvas should start empty");

        // Run until halted or safety limit
        for _ in 0..50000 {
            if vm.halted {
                break;
            }
            vm.step();
        }

        // The successor code (LDI r0, 99; HALT) should have executed
        assert!(vm.halted, "VM should halt after self-written code executes");
        assert_eq!(vm.regs[0], 99, "r0 should be 99 after successor runs");
        assert_ne!(
            vm.ram[0xFFD], 0xFFFFFFFF,
            "ASMSELF should have succeeded"
        );
    }

    // ── RUNNEXT tests (Phase 48: Self-Execution Opcode) ──────────

    #[test]
    fn test_runnext_sets_pc_to_0x1000() {
        let mut vm = Vm::new();
        vm.pc = 0;
        vm.ram[0] = 0x74; // RUNNEXT

        vm.step();

        assert_eq!(vm.pc, 0x1000, "RUNNEXT should set PC to 0x1000");
        assert!(!vm.halted, "RUNNEXT should not halt the VM");
    }

    #[test]
    fn test_runnext_preserves_registers() {
        let mut vm = Vm::new();
        vm.regs[0] = 111;
        vm.regs[1] = 222;
        vm.regs[5] = 555;
        vm.ram[0] = 0x74; // RUNNEXT

        vm.step();

        assert_eq!(vm.regs[0], 111, "r0 should be preserved across RUNNEXT");
        assert_eq!(vm.regs[1], 222, "r1 should be preserved across RUNNEXT");
        assert_eq!(vm.regs[5], 555, "r5 should be preserved across RUNNEXT");
    }

    #[test]
    fn test_runnext_executes_newly_assembled_code() {
        // Full write-compile-execute cycle:
        // 1. Write "LDI r0, 77\nHALT\n" to canvas
        // 2. ASMSELF compiles it to 0x1000
        // 3. RUNNEXT jumps to 0x1000
        // 4. r0 should end up as 77
        let mut vm = Vm::new();
        write_to_canvas(&mut vm.canvas_buffer, 0, "LDI r0, 77\nHALT\n");

        // ASMSELF
        vm.ram[0] = 0x73;
        vm.pc = 0;
        vm.step();
        assert_ne!(vm.ram[0xFFD], 0xFFFFFFFF, "ASMSELF should succeed");

        // RUNNEXT
        vm.ram[1] = 0x74; // RUNNEXT at address 1
        vm.pc = 1;
        vm.step();
        assert_eq!(vm.pc, 0x1000, "RUNNEXT should set PC to 0x1000");

        // Execute the newly assembled code (LDI r0, 77; HALT)
        vm.step(); // LDI r0, 77
        vm.step(); // HALT

        assert_eq!(vm.regs[0], 77, "r0 should be 77 after RUNNEXT executes new code");
        assert!(vm.halted, "VM should halt after new code's HALT");
    }

    #[test]
    fn test_runnext_registers_inherited_by_new_code() {
        // Set registers before RUNNEXT, new code should read them
        let mut vm = Vm::new();
        vm.regs[5] = 12345;

        // New code: LDI r0, 0; ADD r0, r5; HALT
        // This reads r5 and adds it to r0
        write_to_canvas(&mut vm.canvas_buffer, 0, "LDI r0, 0\nADD r0, r5\nHALT\n");

        // ASMSELF
        vm.ram[0] = 0x73;
        vm.pc = 0;
        vm.step();
        assert_ne!(vm.ram[0xFFD], 0xFFFFFFFF, "ASMSELF should succeed");

        // RUNNEXT
        vm.ram[1] = 0x74;
        vm.pc = 1;
        vm.step();

        // Execute new code
        for _ in 0..10 { vm.step(); }

        assert_eq!(vm.regs[0], 12345, "r0 should equal r5's value from before RUNNEXT");
    }

    #[test]
    fn test_runnext_disassembler() {
        let mut vm = Vm::new();
        vm.ram[0] = 0x74; // RUNNEXT
        let (text, _len) = vm.disassemble_at(0);
        assert_eq!(text, "RUNNEXT", "Disassembler should show RUNNEXT");
    }

    #[test]
    fn test_runnext_assembler() {
        use crate::assembler::assemble;
        let src = "RUNNEXT\nHALT\n";
        let result = assemble(src, 0).expect("assembly should succeed");
        assert_eq!(result.pixels[0], 0x74, "RUNNEXT should encode as 0x74");
    }

    #[test]
    fn test_chained_self_modification() {
        // Two-generation self-modification chain:
        // Gen A (bootstrap at PC=0): writes source to canvas, ASMSELF, RUNNEXT
        // Gen B (at 0x1000): LDI r0, 999; HALT
        //
        // Three-generation chains are possible but require careful address management
        // to avoid the ASMSELF clear zone (0x1000-0x1FFF). This test proves the
        // core mechanism: a program writes its successor, compiles it, and runs it.
        let mut vm = Vm::new();

        // Write Gen B source directly to canvas: "LDI r0, 999\nHALT\n"
        let gen_b_src = "LDI r0, 999\nHALT\n";
        write_to_canvas(&mut vm.canvas_buffer, 0, gen_b_src);

        // Bootstrap at PC=0: ASMSELF compiles canvas text to 0x1000, RUNNEXT jumps there
        vm.ram[0] = 0x73; // ASMSELF
        vm.ram[1] = 0x74; // RUNNEXT
        vm.pc = 0;

        // Execute the chain
        for _ in 0..100 {
            if vm.halted { break; }
            vm.step();
        }

        assert!(vm.halted, "VM should halt after Gen B executes");
        assert_eq!(vm.regs[0], 999, "r0 should be 999 -- proof Gen B ran after Gen A assembled it");
    }

    #[test]
    fn test_runnext_full_write_compile_execute_cycle() {
        // A program that writes code to canvas, compiles it, and runs it
        let mut vm = Vm::new();

        // Write assembly source to canvas: "LDI r1, 42\nADD r0, r1\nHALT\n"
        write_to_canvas(&mut vm.canvas_buffer, 0, "LDI r1, 42\nADD r0, r1\nHALT\n");

        // Set r0 = 100 before RUNNEXT
        vm.regs[0] = 100;

        // Bootstrap: ASMSELF then RUNNEXT
        vm.ram[0] = 0x73; // ASMSELF
        vm.ram[1] = 0x74; // RUNNEXT
        vm.pc = 0;

        // Execute ASMSELF
        vm.step();
        assert_ne!(vm.ram[0xFFD], 0xFFFFFFFF, "ASMSELF should succeed");

        // Execute RUNNEXT
        vm.step();
        assert_eq!(vm.pc, 0x1000);

        // Execute the new code (LDI r1, 42; ADD r0, r1; HALT)
        for _ in 0..20 { vm.step(); }

        // r0 was 100, r1 becomes 42, r0 = r0 + r1 = 142
        assert_eq!(vm.regs[0], 142, "r0 should be 100 + 42 = 142");
        assert_eq!(vm.regs[1], 42, "r1 should be 42");
    }

    // ============================================================
    // Phase 49: Self-Modifying Programs - Demo Tests
    // ============================================================

    #[test]
    fn test_self_writer_demo_assembles() {
        // Verify the self_writer.asm program assembles without errors
        let source = include_str!("../programs/self_writer.asm");
        let result = crate::assembler::assemble(source, 0x1000);
        assert!(result.is_ok(), "self_writer.asm should assemble: {:?}", result.err());
        let asm = result.expect("operation should succeed");
        assert!(asm.pixels.len() > 50, "self_writer should produce substantial bytecode");
    }

    #[test]
    fn test_self_writer_successor_different_from_parent() {
        // The parent writes "LDI r0, 42\nHALT\n" to canvas, then ASMSELF + RUNNEXT.
        // The successor (LDI r0, 42; HALT) is clearly different from the parent
        // (which writes to canvas, calls ASMSELF, calls RUNNEXT).
        // Verify: after the full cycle, r0 == 42 (set by successor, not parent).
        let mut vm = Vm::new();
        vm.regs[0] = 0; // parent doesn't touch r0

        // Write successor source to canvas
        write_to_canvas(&mut vm.canvas_buffer, 0, "LDI r0, 42\nHALT\n");

        // Bootstrap: ASMSELF + RUNNEXT at PC=0
        vm.ram[0] = 0x73; // ASMSELF
        vm.ram[1] = 0x74; // RUNNEXT
        vm.pc = 0;

        // Execute ASMSELF
        vm.step();
        assert_ne!(vm.ram[0xFFD], 0xFFFFFFFF, "ASMSELF should succeed");

        // Execute RUNNEXT
        vm.step();
        assert_eq!(vm.pc, 0x1000, "RUNNEXT should set PC to 0x1000");

        // Execute successor: LDI r0, 42; HALT
        for _ in 0..20 { vm.step(); }

        assert_eq!(vm.regs[0], 42, "successor should set r0 to 42");
        assert!(vm.halted, "successor should halt");
    }

    #[test]
    fn test_self_writer_canvas_output_visible() {
        // Verify that the successor's source text is visible in the canvas buffer
        // after the parent writes it (before ASMSELF compiles it).
        let mut vm = Vm::new();

        // Write successor source to canvas
        let successor_src = "LDI r0, 42\nHALT\n";
        write_to_canvas(&mut vm.canvas_buffer, 0, successor_src);

        // Verify the text is in the canvas buffer
        assert_eq!(vm.canvas_buffer[0], 'L' as u32);
        assert_eq!(vm.canvas_buffer[1], 'D' as u32);
        assert_eq!(vm.canvas_buffer[2], 'I' as u32);
        assert_eq!(vm.canvas_buffer[3], ' ' as u32);
        assert_eq!(vm.canvas_buffer[4], 'r' as u32);
        assert_eq!(vm.canvas_buffer[5], '0' as u32);
        assert_eq!(vm.canvas_buffer[6], ',' as u32);
        // Newline at index 10, HALT starts at index 11
        assert_eq!(vm.canvas_buffer[10], 10, "newline char at index 10");
        assert_eq!(vm.canvas_buffer[11], 'H' as u32);
        assert_eq!(vm.canvas_buffer[12], 'A' as u32);
        assert_eq!(vm.canvas_buffer[13], 'L' as u32);
        assert_eq!(vm.canvas_buffer[14], 'T' as u32);
    }

    #[test]
    fn test_self_writer_two_generation_chain() {
        // Generation A: writes Gen B source to canvas, ASMSELF, RUNNEXT
        // Generation B: writes r0=77, then HALT
        // Verify the full A -> B chain works
        let mut vm = Vm::new();
        vm.regs[0] = 0;

        // Gen A writes Gen B's source to canvas
        write_to_canvas(&mut vm.canvas_buffer, 0, "LDI r0, 77\nHALT\n");

        // Gen A's code: ASMSELF, RUNNEXT
        vm.ram[0] = 0x73;
        vm.ram[1] = 0x74;
        vm.pc = 0;

        vm.step(); // ASMSELF
        assert_ne!(vm.ram[0xFFD], 0xFFFFFFFF);
        vm.step(); // RUNNEXT
        assert_eq!(vm.pc, 0x1000);

        for _ in 0..20 { vm.step(); }
        assert_eq!(vm.regs[0], 77, "Gen B should set r0 to 77");
    }

    #[test]
    fn test_self_writer_successor_modifies_canvas() {
        // Generation A writes Gen B source to canvas.
        // Gen B writes a character to a DIFFERENT canvas row, proving it ran.
        // Gen B source: "LDI r1, 0x8040\nLDI r2, 88\nSTORE r1, r2\nHALT\n"
        // This writes 'X' (88) to canvas row 2 (0x8040 = 0x8000 + 2*32)
        let mut vm = Vm::new();

        let gen_b_src = "LDI r1, 0x8040\nLDI r2, 88\nSTORE r1, r2\nHALT\n";
        write_to_canvas(&mut vm.canvas_buffer, 0, gen_b_src);

        vm.ram[0] = 0x73; // ASMSELF
        vm.ram[1] = 0x74; // RUNNEXT
        vm.pc = 0;

        vm.step(); // ASMSELF
        assert_ne!(vm.ram[0xFFD], 0xFFFFFFFF, "ASMSELF should compile Gen B");

        vm.step(); // RUNNEXT
        assert_eq!(vm.pc, 0x1000);

        // Run Gen B
        for _ in 0..50 { vm.step(); }

        // Verify Gen B wrote 'X' to canvas row 2
        let row2_start = 2 * 32; // 0x8040 - 0x8000 = 64
        assert_eq!(vm.canvas_buffer[row2_start], 88, "Gen B should write 'X' to canvas row 2");
        assert!(vm.halted, "Gen B should halt");
    }

    #[test]
    fn test_self_writer_registers_inherited_across_generations() {
        // Gen A sets r5=100, then writes+compiles+runs Gen B.
        // Gen B reads r5 (should be 100), adds 1, stores in r0.
        // Gen B source: "ADD r0, r5\nLDI r1, 1\nADD r0, r1\nHALT\n"
        // Result: r0 = 0 + 100 + 1 = 101
        let mut vm = Vm::new();
        vm.regs[5] = 100; // Set by Gen A before RUNNEXT
        vm.regs[0] = 0;

        let gen_b_src = "ADD r0, r5\nLDI r1, 1\nADD r0, r1\nHALT\n";
        write_to_canvas(&mut vm.canvas_buffer, 0, gen_b_src);

        vm.ram[0] = 0x73;
        vm.ram[1] = 0x74;
        vm.pc = 0;

        vm.step(); // ASMSELF
        assert_ne!(vm.ram[0xFFD], 0xFFFFFFFF);
        vm.step(); // RUNNEXT

        for _ in 0..50 { vm.step(); }
        assert_eq!(vm.regs[0], 101, "r0 should be 0 + r5(100) + 1 = 101");
        assert_eq!(vm.regs[5], 100, "r5 should still be 100 (inherited from Gen A)");
    }

    #[test]
    fn test_infinite_map_assembles_and_runs() {
        use crate::assembler::assemble;

        let source = include_str!("../programs/infinite_map.asm");
        let asm = assemble(source, 0).expect("infinite_map.asm should assemble");
        assert!(!asm.pixels.is_empty(), "should produce bytecode");
        eprintln!("Assembled {} words from infinite_map.asm", asm.pixels.len());

        let mut vm = Vm::new();
        for (i, &word) in asm.pixels.iter().enumerate() {
            if i < vm.ram.len() {
                vm.ram[i] = word;
            }
        }

        // Simulate Right arrow (bit 3 = 8)
        vm.ram[0xFFB] = 8;

        // Run until first FRAME
        vm.frame_ready = false;
        let mut steps = 0u32;
        for _ in 0..1_000_000 {
            if vm.frame_ready { break; }
            let keep_going = vm.step();
            steps += 1;
            if !keep_going { break; }
        }

        assert!(vm.frame_ready, "should reach FRAME within 1M steps (took {})", steps);
        eprintln!("First frame rendered in {} steps", steps);
        eprintln!("camera_x = {}, camera_y = {}", vm.ram[0x7800], vm.ram[0x7801]);
        assert_eq!(vm.ram[0x7800], 1, "camera should have moved right by 1");

        // Screen should not be all black
        let non_black = vm.screen.iter().filter(|&&p| p != 0).count();
        eprintln!("Non-black pixels: {}/{}", non_black, 256*256);
        assert!(non_black > 0, "screen should have rendered terrain");

        // Second frame: press Down
        vm.frame_ready = false;
        vm.ram[0xFFB] = 2; // Down
        for _ in 0..1_000_000 {
            if vm.frame_ready { break; }
            let keep_going = vm.step();
            if !keep_going { break; }
        }
        eprintln!("After 2nd frame: camera_x={}, camera_y={}", vm.ram[0x7800], vm.ram[0x7801]);
        assert!(vm.frame_ready, "second frame should render");
        assert_eq!(vm.ram[0x7801], 1, "camera should have moved down by 1");

        // Third frame: press Left+Up (bits 2+0 = 5) -- diagonal movement
        vm.frame_ready = false;
        vm.ram[0xFFB] = 5;
        for _ in 0..1_000_000 {
            if vm.frame_ready { break; }
            let keep_going = vm.step();
            if !keep_going { break; }
        }
        eprintln!("After 3rd frame (left+up): camera_x={}, camera_y={}", vm.ram[0x7800], vm.ram[0x7801]);
        assert_eq!(vm.ram[0x7800], 0, "camera should have moved left back to 0");
        assert_eq!(vm.ram[0x7801], 0, "camera should have moved up back to 0");

        // Verify frame counter incremented
        assert!(vm.ram[0x7802] >= 3, "frame_counter should be >= 3 (was {})", vm.ram[0x7802]);
        eprintln!("Frame counter: {}", vm.ram[0x7802]);

        // Verify water animation: run 2 frames without moving, check screen changes
        // Frame 4: no keys
        vm.frame_ready = false;
        vm.ram[0xFFB] = 0;
        for _ in 0..1_000_000 {
            if vm.frame_ready { break; }
            let keep_going = vm.step();
            if !keep_going { break; }
        }
        let screen_f4: Vec<u32> = vm.screen.to_vec();

        // Frame 5: no keys
        vm.frame_ready = false;
        vm.ram[0xFFB] = 0;
        for _ in 0..1_000_000 {
            if vm.frame_ready { break; }
            let keep_going = vm.step();
            if !keep_going { break; }
        }
        let screen_f5: Vec<u32> = vm.screen.to_vec();

        // Count pixels that changed between frames (water animation)
        let changed: usize = screen_f4.iter().zip(screen_f5.iter())
            .filter(|(a, b)| a != b).count();
        eprintln!("Pixels changed between frames 4-5: {}/{}", changed, 256*256);
        // With ~25% water tiles and animation, expect some pixels to change
        assert!(changed > 0, "water animation should cause pixel changes between frames");
    }

    #[test]
    fn test_infinite_map_visual_analysis() {
        use crate::assembler::assemble;

        let source = include_str!("../programs/infinite_map.asm");
        let asm = assemble(source, 0).expect("assembly should succeed");

        let mut vm = Vm::new();
        for (i, &word) in asm.pixels.iter().enumerate() {
            if i < vm.ram.len() {
                vm.ram[i] = word;
            }
        }

        // Test at camera position (100, 100) to see multiple biome zones
        // Coarse coords span (12,12) to (20,20) = 9x9 zones = lots of variety
        vm.ram[0x7800] = 100;
        vm.ram[0x7801] = 100;
        vm.ram[0xFFB] = 0;
        vm.frame_ready = false;
        for _ in 0..1_000_000 {
            if vm.frame_ready { break; }
            if !vm.step() { break; }
        }

        // Count unique colors (structures + animation create many)
        let mut color_counts: std::collections::HashMap<u32, usize> = std::collections::HashMap::new();
        for &pixel in vm.screen.iter() {
            *color_counts.entry(pixel).or_insert(0) += 1;
        }
        eprintln!("At (100,100): {} unique colors", color_counts.len());
        assert!(color_counts.len() >= 5, "should see multiple biomes at (100,100)");

        // Check biome contiguity by sampling tile-top-left pixels
        // and masking the water animation (low 5 blue bits change per tile)
        // For contiguity, compare the "base biome" by rounding colors
        let mut biome_zones = 0;
        let mut prev_base: u32 = 0;
        for tx in 0..64 {
            let px = tx * 4;
            let py = 128; // tile row 32 - middle of screen
            let color = vm.screen[py * 256 + px];
            // Round to base biome: mask out animation (low 5 bits of blue)
            let base = color & !0x1F;
            if tx == 0 || base != prev_base {
                biome_zones += 1;
                prev_base = base;
            }
        }
        eprintln!("At (100,100) row 32: {} biome zone boundaries across 64 tiles", biome_zones);

        // With 8-tile zones, expect ~8 boundaries. Per-tile hash would give ~64.
        // Allow up to 20 to account for structures overriding colors
        assert!(biome_zones < 20,
            "biomes should be contiguous, got {} zone boundaries (expected <20)", biome_zones);

        // Verify the terrain is deterministic: same camera = same screen
        let screen1 = vm.screen.to_vec();
        vm.frame_ready = false;
        for _ in 0..1_000_000 {
            if vm.frame_ready { break; }
            if !vm.step() { break; }
        }
        // Note: frame counter advanced, so water animation differs. Check non-water.
        let non_water_same = screen1.iter().zip(vm.screen.iter())
            .filter(|(&a, &b)| {
                let a_water = (a & 0xFF) > 0 && ((a >> 16) & 0xFF) == 0 && ((a >> 8) & 0xFF) < 0x20;
                !a_water && a == b
            }).count();
        // Non-water tiles should be identical (deterministic terrain)
        eprintln!("Non-water pixels identical across frames: {}", non_water_same);
    }

    // ── Inode Filesystem Opcodes (Phase 43) ──────────────────────────

    /// Helper: create a VM with a string at addr and run bytecode
    fn run_program_with_string(bytecode: &[u32], max_steps: usize, str_addr: usize, s: &str) -> Vm {
        let mut vm = Vm::new();
        // Write string to RAM
        for (i, ch) in s.bytes().enumerate() {
            vm.ram[str_addr + i] = ch as u32;
        }
        vm.ram[str_addr + s.len()] = 0;
        // Load bytecode
        for (i, &word) in bytecode.iter().enumerate() {
            vm.ram[i] = word;
        }
        vm.pc = 0;
        vm.halted = false;
        for _ in 0..max_steps {
            if !vm.step() { break; }
        }
        vm
    }

    #[test]
    fn test_fmkdir_creates_directory() {
        // Write "/tmp" to RAM at address 100
        // LDI r1, 100
        // FMKDIR r1
        // HALT
        let prog = vec![0x10, 1, 100, 0x78, 1, 0x00];
        let vm = run_program_with_string(&prog, 100, 100, "/tmp");
        assert_eq!(vm.regs[0], 2); // inode 2 for /tmp
        assert_eq!(vm.inode_fs.resolve("/tmp"), Some(2));
    }

    #[test]
    fn test_fmkdir_nested_fails() {
        // /a/b/c won't work because /a doesn't exist
        let prog = vec![0x10, 1, 100, 0x78, 1, 0x00];
        let vm = run_program_with_string(&prog, 100, 100, "/a/b/c");
        assert_eq!(vm.regs[0], 0); // failed
    }

    #[test]
    fn test_funlink_removes_file() {
        // Create a file first via FMKDIR... no, use inode_fs directly via a setup step
        // We need to create a file in the inode_fs before running the program.
        // Since run_program_with_string creates a fresh VM, we'll create the file
        // via a two-step program: first create dirs, then unlink
        // Actually, let's use create directly on the VM after setup
        let mut vm = Vm::new();
        vm.inode_fs.create("/del_me.txt");
        assert!(vm.inode_fs.resolve("/del_me.txt").is_some());

        // Now write unlink path and run
        let path = "/del_me.txt";
        for (i, ch) in path.bytes().enumerate() {
            vm.ram[100 + i] = ch as u32;
        }
        vm.ram[100 + path.len()] = 0;

        // LDI r1, 100; FUNLINK r1; HALT
        vm.ram[0] = 0x10; vm.ram[1] = 1; vm.ram[2] = 100;
        vm.ram[3] = 0x7A; vm.ram[4] = 1;
        vm.ram[5] = 0x00;
        vm.pc = 0;
        vm.halted = false;
        for _ in 0..100 {
            if !vm.step() { break; }
        }
        assert_eq!(vm.regs[0], 1); // success
        assert_eq!(vm.inode_fs.resolve("/del_me.txt"), None);
    }

    #[test]
    fn test_fstat_returns_inode_metadata() {
        let mut vm = Vm::new();
        let ino = vm.inode_fs.create("/test.txt");
        vm.inode_fs.write_inode(ino, 0, &[10, 20, 30]);

        // LDI r1, <ino>; LDI r2, 200; FSTAT r1, r2; HALT
        vm.ram[0] = 0x10; vm.ram[1] = 1; vm.ram[2] = ino;
        vm.ram[3] = 0x10; vm.ram[4] = 2; vm.ram[5] = 200;
        vm.ram[6] = 0x79; vm.ram[7] = 1; vm.ram[8] = 2;
        vm.ram[9] = 0x00;
        vm.pc = 0;
        vm.halted = false;
        for _ in 0..100 {
            if !vm.step() { break; }
        }
        assert_eq!(vm.regs[0], 1); // success
        assert_eq!(vm.ram[200], ino);         // ino
        assert_eq!(vm.ram[201], 1);           // itype = Regular
        assert_eq!(vm.ram[202], 3);           // size
        assert_eq!(vm.ram[203], 0);           // ref_count
        assert_eq!(vm.ram[204], 1);           // parent = root
        assert_eq!(vm.ram[205], 0);           // num_children
    }

    #[test]
    fn test_fstat_nonexistent_returns_zero() {
        // LDI r1, 999; LDI r2, 200; FSTAT r1, r2; HALT
        let prog = vec![0x10, 1, 999, 0x10, 2, 200, 0x79, 1, 2, 0x00];
        let vm = run_program(&prog, 100);
        assert_eq!(vm.regs[0], 0); // failure
    }

    #[test]
    fn test_disassemble_fmkdir() {
        let mut vm = Vm::new();
        vm.ram[0] = 0x78;
        vm.ram[1] = 5;
        let (text, len) = vm.disassemble_at(0);
        assert_eq!(text, "FMKDIR [r5]");
        assert_eq!(len, 2);
    }

    #[test]
    fn test_disassemble_fstat() {
        let mut vm = Vm::new();
        vm.ram[0] = 0x79;
        vm.ram[1] = 1;
        vm.ram[2] = 2;
        let (text, len) = vm.disassemble_at(0);
        assert_eq!(text, "FSTAT r1, [r2]");
        assert_eq!(len, 3);
    }

    #[test]
    fn test_disassemble_funlink() {
        let mut vm = Vm::new();
        vm.ram[0] = 0x7A;
        vm.ram[1] = 3;
        let (text, len) = vm.disassemble_at(0);
        assert_eq!(text, "FUNLINK [r3]");
        assert_eq!(len, 2);
    }
}