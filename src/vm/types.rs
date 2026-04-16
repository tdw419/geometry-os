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
            /// Create a new pipe with the given reader/writer PIDs.
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

            /// Returns true if the pipe buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

            /// Returns true if the pipe buffer is full.
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
            /// Create a new message with the given sender PID and payload.
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
            /// Create a new virtual memory area.
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
