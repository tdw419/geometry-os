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
    /// result = first dep + second dep
    Add,
    /// result = first dep - second dep
    Sub,
    /// result = first dep * second dep
    Mul,
    /// result = first dep / second dep (0 on div-by-zero)
    Div,
    /// result = first dep & second dep
    And,
    /// result = first dep | second dep
    Or,
    /// result = first dep ^ second dep
    Xor,
    /// result = !first dep (bitwise NOT, single dep)
    Not,
    /// result = first dep (identity/copy, single dep)
    Copy,
    /// result = max(first dep, second dep)
    Max,
    /// result = min(first dep, second dep)
    Min,
    /// result = first dep % second dep
    Mod,
    /// result = first dep << second dep
    Shl,
    /// result = first dep >> second dep
    Shr,
}
/// Maximum number of concurrently spawned child processes
pub const MAX_PROCESSES: usize = 8;
/// Syscall dispatch table base address in RAM.
/// RAM[SYSCALL_TABLE + N] = handler address for syscall number N.
pub const SYSCALL_TABLE: usize = 0xFE00;

/// Maximum number of hit-test regions (GUI buttons, clickable areas).
pub const MAX_HIT_REGIONS: usize = 64;

/// A rectangular hit-test region for GUI interaction.
/// Registered by HITSET, queried by HITQ.
#[derive(Debug, Clone, Copy)]
pub struct HitRegion {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    pub id: u32,
}

/// A styled text line produced by the HTML parser (Phase 82).
/// Each line has a foreground color and up to 32 characters.
#[derive(Debug, Clone)]
pub struct StyledLine {
    pub fg_color: u32,
    pub chars: Vec<u32>,
}

/// A link extracted from HTML during parsing.
/// Registered as a HITSET region for click detection.
#[derive(Debug, Clone)]
pub struct HtmlLink {
    pub href: String,
    pub line_index: usize,
    pub char_start: usize,
    pub char_end: usize,
}

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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
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

/// Mailbox constants (Phase 4.1: Inter-tile mailbox communication).
/// Each tile has one incoming message slot. Messages are (sender_id, data) pairs.
/// Double-buffered: writes go to write_buf, FRAME swaps write_buf into read_buf.
/// Max 256 tiles (supports 16x16 tile grid at 16px tiles).
pub const MAILBOX_SIZE: usize = 256;
pub const MAX_HOST_FILES: usize = 16;
/// MMIO base address for mailbox region (documented convention).
pub const MAILBOX_RAM_BASE: usize = 0x5000;
/// A single mailbox message entry.
/// Stored per-destination-tile. sender_id identifies the source tile.
/// valid=false means no message pending (slot empty).
#[derive(Debug, Clone, Copy)]
pub struct MailboxEntry {
    pub valid: bool,
    pub sender_id: u32,
    pub data: u32,
}

impl Default for MailboxEntry {
    fn default() -> Self {
        Self {
            valid: false,
            sender_id: 0,
            data: 0,
        }
    }
}

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
///   Zombie -> gone         (parent calls WAITPID, reaps exit code)
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
        Vma {
            vtype,
            start_page,
            current_end,
            max_end,
        }
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
        if !self.contains_page(vpage) {
            return false;
        }
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
/// A capability granting access to a VFS path pattern.
/// Used by Phase 102 (Permissions and Capability System).
///
/// `resource_type`: 0 = VFS path, 1 = opcode restriction.
/// `pattern`: glob-like pattern for VFS paths (e.g. "/tmp/*", "/lib/fonts/*").
/// `permissions`: bitmask — bit 0 = read, bit 1 = write, bit 2 = execute.
#[derive(Debug, Clone)]
pub struct Capability {
    pub resource_type: u8,
    pub pattern: String,
    pub permissions: u8,
}

impl Capability {
    pub const PERM_READ: u8 = 0x01;
    pub const PERM_WRITE: u8 = 0x02;
    #[allow(dead_code)]
    pub const PERM_EXEC: u8 = 0x04;

    /// Check if a given path matches this capability's pattern.
    /// Supports simple glob: trailing `*` matches anything.
    pub fn matches_path(&self, path: &str) -> bool {
        if self.resource_type != 0 {
            return false;
        }
        if self.pattern.ends_with('*') {
            let prefix = &self.pattern[..self.pattern.len() - 1];
            path.starts_with(prefix)
        } else {
            path == self.pattern
        }
    }

    /// Check if this capability allows the given permission bits.
    pub fn allows(&self, perm: u8) -> bool {
        (self.permissions & perm) != 0
    }
}

/// Check if a process with the given capabilities can access a path with a specific permission.
/// Returns true if access is allowed, false if denied.
/// If capabilities is None, all access is allowed (backward compatible).
pub fn check_path_capability(caps: &Option<Vec<Capability>>, path: &str, perm: u8) -> bool {
    match caps {
        None => true, // No capability list = full access
        Some(cap_list) => {
            // Must find at least one matching capability that grants the permission
            cap_list
                .iter()
                .any(|c| c.matches_path(path) && c.allows(perm))
        }
    }
}

/// Check if a process with the given capabilities can execute a restricted opcode.
/// Returns true if allowed, false if denied.
#[allow(dead_code)]
pub fn check_opcode_capability(caps: &Option<Vec<Capability>>, opcode: u8) -> bool {
    match caps {
        None => true,
        Some(cap_list) => {
            // Check if any capability restricts this opcode
            // Resource type 1 = opcode restriction. Pattern is the opcode number as string.
            // If a restriction exists and doesn't grant EXEC, deny.
            let restricted = cap_list.iter().filter(|c| c.resource_type == 1);
            for r in restricted {
                if let Ok(restricted_op) = r.pattern.parse::<u8>() {
                    if restricted_op == opcode && !r.allows(Capability::PERM_EXEC) {
                        return false;
                    }
                }
            }
            true
        }
    }
}

/// Per-process capability table. On SPAWN, parent specifies allowed VFS paths (wildcard patterns) and restricted opcodes.
/// Stores as a list of (resource_type, pattern, permissions) tuples.
///
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

    // ── Windowed App Data Region (Phase 107) ──────────────────────
    /// Base address of this process's private data region in RAM.
    /// For windowed apps: 0x9000 + slot * 0x1000 (4096 words each).
    /// 0 = unset (main process, no private region).
    pub data_base: u32,

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

    // ── Fonts (Phase 98: Custom Bitmap Fonts) ──────────────────────
    /// Per-process custom font override. 128 glyphs × 8 rows each.
    /// None = use built-in GLYPHS array. Set via /dev/screen IOCTL cmd 2.
    pub custom_font: Option<Vec<[u8; 8]>>,

    // ── Capabilities (Phase 102: Permissions and Capability System) ─
    /// Per-process capability list. None = full access (backward compatible).
    /// Each capability grants access to a VFS path pattern with specific permissions.
    pub capabilities: Option<Vec<Capability>>,
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
            custom_font: None,
            capabilities: None,
            data_base: 0,
        }
    }

    /// Convenience: is this process halted (zombie, segfaulted, or stopped)?
    /// The scheduler skips halted processes.
    pub fn is_halted(&self) -> bool {
        matches!(self.state, ProcessState::Zombie | ProcessState::Stopped) || self.segfaulted
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
            Vma::new(
                VmaType::Stack,
                PROCESS_PAGES - 1,
                PROCESS_PAGES - 1,
                PROCESS_PAGES - 2,
            ),
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

/// Maximum number of windows that can be active simultaneously.
pub const MAX_WINDOWS: usize = 8;

/// Window title bar height in pixels.
#[allow(dead_code)]
pub const WINDOW_TITLE_BAR_H: u32 = 12;
/// Window border width in pixels.
#[allow(dead_code)]
pub const WINDOW_BORDER_W: u32 = 1;

/// Taskbar occupies the bottom 16 rows of the 256x256 screen.
/// This region must always render on top of windows (Z-order: terrain < windows < taskbar).
pub const TASKBAR_Y: usize = 240;
pub const TASKBAR_H: usize = 16;

/// A managed window with position, size, Z-order, and offscreen buffer.
/// Created by WINSYS op=0, destroyed by WINSYS op=1.
///
/// Phase 107: Windows can live in world-space (infinite map coordinates)
/// or screen-space (256x256 framebuffer). When `world_x`/`world_y` are set,
/// the window is positioned in the infinite map and pans with the camera.
/// RAM[0x7810] = 1 enables world-space mode for WINSYS op=0.
#[derive(Debug, Clone)]
pub struct Window {
    /// Window ID (1-based, 0 = invalid/no window).
    pub id: u32,
    /// Left edge X coordinate (screen-space, or computed from world coords).
    pub x: u32,
    /// Top edge Y coordinate (screen-space, or computed from world coords).
    pub y: u32,
    /// Window width in pixels.
    pub w: u32,
    /// Window height in pixels.
    pub h: u32,
    /// Z-order: higher = on top. Auto-assigned on create/bring-to-front.
    pub z_order: u32,
    /// RAM address of null-terminated title string.
    #[allow(dead_code)]
    pub title_addr: u32,
    /// PID of the process that created this window.
    pub pid: u32,
    /// Whether this window is active (alive).
    pub active: bool,
    /// Offscreen pixel buffer (w * h pixels). Initialized to all black (0).
    /// Programs write pixels here; FRAME blits to main screen in Z-order.
    /// Can be larger than 256x256; viewport_x/y controls which portion is displayed.
    pub offscreen_buffer: Vec<u32>,
    // ── Phase 107: World-space coordinates ──
    /// World-space X position (tile coordinate). 0xFFFFFFFF = unset (screen-space window).
    pub world_x: u32,
    /// World-space Y position (tile coordinate). 0xFFFFFFFF = unset (screen-space window).
    pub world_y: u32,
    // ── Phase 133: Viewport offset for wider/taller windows ──
    /// Horizontal scroll offset within the offscreen buffer (pixels).
    /// Controls which portion of a wider buffer is visible on the 256x256 screen.
    pub viewport_x: u32,
    /// Vertical scroll offset within the offscreen buffer (pixels).
    pub viewport_y: u32,
    // ── Phase 153: Maximize/restore ──
    /// Whether this window is currently maximized.
    pub maximized: bool,
    /// Saved position/size before maximize, for restore. None if never maximized.
    pub restore_rect: Option<(u32, u32, u32, u32)>, // (x, y, w, h)
}

/// Sentinel value: world coords unset, window uses screen-space positioning.
pub const WORLD_COORD_UNSET: u32 = 0xFFFFFFFF;

/// RAM address for the window world-coords flag.
/// When RAM[0x7810] == 1, WINSYS op=0 uses r1/r2 as world-space coordinates.
pub const WINDOW_WORLD_COORDS_ADDR: usize = 0x7810;

/// Base address for windowed app data regions.
/// Each app gets APP_DATA_SIZE words of private data starting at APP_DATA_BASE + slot * APP_DATA_SIZE.
/// Placed at 0x9000 to avoid canvas RAM (0x8000-0x8FFF) and hardware ports (0xFF00+).
pub const APP_DATA_BASE: usize = 0x9000;
/// Size of each windowed app's private data region (4096 words = 16KB).
pub const APP_DATA_SIZE: usize = 0x1000;

impl Window {
    /// Create a new window with the given parameters.
    /// Offscreen buffer is initialized to transparent black (0x00000000).
    pub fn new(id: u32, x: u32, y: u32, w: u32, h: u32, title_addr: u32, pid: u32) -> Self {
        let buf_size = (w as usize) * (h as usize);
        Window {
            id,
            x,
            y,
            w,
            h,
            z_order: 0,
            title_addr,
            pid,
            active: true,
            offscreen_buffer: vec![0u32; buf_size],
            world_x: WORLD_COORD_UNSET,
            world_y: WORLD_COORD_UNSET,
            viewport_x: 0,
            viewport_y: 0,
            maximized: false,
            restore_rect: None,
        }
    }

    /// Create a new window positioned in world-space.
    /// Screen x,y will be computed from viewport at render time.
    pub fn new_world(
        id: u32,
        world_x: u32,
        world_y: u32,
        w: u32,
        h: u32,
        title_addr: u32,
        pid: u32,
    ) -> Self {
        let buf_size = (w as usize) * (h as usize);
        Window {
            id,
            x: 0, // computed at render time
            y: 0,
            w,
            h,
            z_order: 0,
            title_addr,
            pid,
            active: true,
            offscreen_buffer: vec![0u32; buf_size],
            world_x,
            world_y,
            viewport_x: 0,
            viewport_y: 0,
            maximized: false,
            restore_rect: None,
        }
    }

    /// Returns true if this window is positioned in world-space.
    pub fn is_world_space(&self) -> bool {
        self.world_x != WORLD_COORD_UNSET && self.world_y != WORLD_COORD_UNSET
    }

    /// Read the window title from RAM (up to 32 chars, null-terminated).
    /// Returns empty string if title_addr is 0 or invalid.
    pub fn read_title(&self, ram: &[u32]) -> String {
        let mut title = String::new();
        if self.title_addr > 0 && (self.title_addr as usize) < ram.len() {
            for j in 0..32 {
                let addr = self.title_addr as usize + j;
                if addr >= ram.len() {
                    break;
                }
                let ch = ram[addr];
                if ch == 0 || ch > 127 {
                    break;
                }
                title.push(ch as u8 as char);
            }
        }
        title
    }

    /// Toggle maximize/restore for this window.
    /// When maximizing, saves current rect to restore_rect.
    /// When restoring, resets from restore_rect.
    /// Returns true if state changed.
    /// `max_w` and `max_h` are the screen dimensions to fill (e.g., 256, 240 with taskbar).
    pub fn toggle_maximize(&mut self, max_w: u32, max_h: u32) -> bool {
        if self.maximized {
            // Restore
            if let Some((rx, ry, rw, rh)) = self.restore_rect {
                self.x = rx;
                self.y = ry;
                self.w = rw;
                self.h = rh;
                self.maximized = false;
                // Rebuild offscreen buffer to match restored size
                let new_size = (rw as usize) * (rh as usize);
                self.offscreen_buffer.resize(new_size, 0);
                return true;
            }
            false
        } else {
            // Maximize: save current rect, fill screen (minus taskbar)
            self.restore_rect = Some((self.x, self.y, self.w, self.h));
            self.x = 0;
            self.y = 0;
            self.w = max_w;
            self.h = max_h;
            self.maximized = true;
            // Rebuild offscreen buffer to match new size
            let new_size = (max_w as usize) * (max_h as usize);
            self.offscreen_buffer.resize(new_size, 0);
            true
        }
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HypervisorMode {
    /// Use QEMU subprocess for guest execution (any architecture).
    #[default]
    Qemu,
    /// Use built-in RISC-V interpreter (Phases 34-36, pure Rust, WASM-portable).
    Native,
}

/// State of a background hypervisor VM (Phase 87: Multi-Hypervisor).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BgVmState {
    /// VM is actively executing (time-sliced by host).
    Running,
    /// VM is paused (not executing, state preserved).
    Paused,
    /// VM state was saved to RAM, VM is dormant.
    #[allow(dead_code)]
    Saved,
}

impl Default for BgVmState {
    fn default() -> Self {
        BgVmState::Paused
    }
}

/// A background hypervisor VM instance (Phase 87: Multi-Hypervisor).
/// Each building on the map can host one of these. The host time-slices
/// between active instances, giving each N instructions per frame.
#[derive(Debug, Clone)]
pub struct BackgroundVm {
    /// Unique instance ID (assigned by VM_SPAWN, 1-based).
    pub id: u32,
    /// Config string: "arch=riscv64 [kernel=file.img] [ram=256M] [mode=native|qemu]"
    #[allow(dead_code)]
    pub config: String,
    /// Execution mode (Qemu or Native).
    #[allow(dead_code)]
    pub mode: HypervisorMode,
    /// Window ID to render output into (0 = full canvas, >0 = WINSYS window).
    #[allow(dead_code)]
    pub window_id: u32,
    /// Current state (Running, Paused, Saved).
    pub state: BgVmState,
    /// Instructions per frame budget for time-slicing (default: 1000).
    pub instructions_per_frame: u32,
    /// Total instructions executed across all time slices.
    pub total_instructions: u64,
    /// Number of frames this VM has been active for.
    pub frames_active: u64,
}

impl Default for BackgroundVm {
    fn default() -> Self {
        BackgroundVm {
            id: 0,
            config: String::new(),
            mode: HypervisorMode::default(),
            window_id: 0,
            state: BgVmState::default(),
            instructions_per_frame: 1000,
            total_instructions: 0,
            frames_active: 0,
        }
    }
}

// ── Phase 123: Live Hypervisor State ──────────────────────────

/// State for a live RISC-V hypervisor VM running inside a WINSYS window.
/// Managed by VM_LIVE_SPAWN / VM_LIVE_STEP / VM_LIVE_KILL opcodes.
/// Only one live hypervisor can be active at a time.
pub struct LiveHypervisorState {
    /// The RISC-V VM instance
    pub vm: crate::riscv::RiscvVm,
    /// WINSYS window ID to render console output into
    pub window_id: u32,
    /// How many RISC-V instructions to execute per VM_LIVE_STEP
    pub instructions_per_slice: u32,
    /// Cumulative instruction count across all time slices
    pub total_instructions: u64,
    /// Current console cursor row (in character cells)
    pub console_row: u32,
    /// Current console cursor column (in character cells)
    pub console_col: u32,
    /// Whether the guest has booted (set after first successful step)
    pub booted: bool,
}

impl std::fmt::Debug for LiveHypervisorState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LiveHypervisorState")
            .field("window_id", &self.window_id)
            .field("instructions_per_slice", &self.instructions_per_slice)
            .field("total_instructions", &self.total_instructions)
            .field("console_row", &self.console_row)
            .field("console_col", &self.console_col)
            .field("booted", &self.booted)
            .finish_non_exhaustive()
    }
}
