// vm/trace.rs -- Execution Trace Ring Buffer (Phase 38a)
//                 Frame Checkpointing (Phase 38b)
//                 Timeline Forking (Phase 38d)
//
// Records every instruction execution to a fixed-size ring buffer.
// Zero overhead when recording is off (one bool check per step).
// Ring buffer allocated once, never grows. No heap allocation in the hot path.
//
// Phase 38b: Snapshots the full screen buffer at every FRAME opcode.
// Combined with the trace ring buffer, you can reconstruct the screen
// at any point without re-executing.
//
// Phase 38d: Full VM state snapshots for timeline forking.
// Save the complete state (RAM, registers, PC, screen, all config) and
// restore it later to explore alternate execution paths.

/// Default ring buffer capacity (entries).
pub const DEFAULT_TRACE_CAPACITY: usize = 10_000;

/// Default frame checkpoint capacity (frames).
/// At 256x256 screen (65536 u32s per frame), 60 frames ≈ 15MB.
pub const DEFAULT_FRAME_CHECK_CAPACITY: usize = 60;

/// A single recorded execution step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceEntry {
    /// Monotonically increasing step counter at time of recording.
    pub step_number: u64,
    /// Program counter value before this instruction executed.
    pub pc: u32,
    /// First 16 registers (r0-r15) at time of recording.
    pub regs: [u32; 16],
    /// The opcode that was executed.
    pub opcode: u32,
}

/// Fixed-size circular buffer of TraceEntry.
///
/// Pre-allocated to `capacity` entries. Old entries are overwritten
/// when the buffer wraps around. No heap allocation after construction.
#[derive(Debug)]
pub struct TraceBuffer {
    entries: Vec<TraceEntry>,
    capacity: usize,
    head: usize,       // next write position
    len: usize,        // number of valid entries (up to capacity)
    step_counter: u64, // monotonically increasing step counter
}

#[allow(dead_code)]
impl TraceBuffer {
    /// Create a new ring buffer with the given capacity.
    pub fn new(capacity: usize) -> Self {
        let capacity = capacity.max(1);
        TraceBuffer {
            entries: (0..capacity)
                .map(|_| TraceEntry {
                    step_number: 0,
                    pc: 0,
                    regs: [0; 16],
                    opcode: 0,
                })
                .collect(),
            capacity,
            head: 0,
            len: 0,
            step_counter: 0,
        }
    }

    /// Push a new entry into the ring buffer.
    /// Overwrites the oldest entry if the buffer is full.
    /// No heap allocation -- writes into pre-allocated slot.
    #[inline]
    pub fn push(&mut self, pc: u32, regs: &[u32; 32], opcode: u32) {
        let entry = &mut self.entries[self.head];
        entry.step_number = self.step_counter;
        entry.pc = pc;
        entry.regs.copy_from_slice(&regs[..16]);
        entry.opcode = opcode;

        self.head = (self.head + 1) % self.capacity;
        if self.len < self.capacity {
            self.len += 1;
        }
        self.step_counter += 1;
    }

    /// Number of valid entries currently in the buffer.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Current step counter value.
    pub fn step_counter(&self) -> u64 {
        self.step_counter
    }

    /// Clear all entries and reset the step counter.
    pub fn clear(&mut self) {
        self.head = 0;
        self.len = 0;
        self.step_counter = 0;
    }

    /// Iterate over entries from oldest to newest.
    pub fn iter(&self) -> TraceIter<'_> {
        let start = if self.len < self.capacity {
            0
        } else {
            self.head // oldest is at head (which wrapped around)
        };
        TraceIter {
            buffer: self,
            pos: 0,
            start,
        }
    }

    /// Get the Nth most recent entry (0 = newest).
    /// Returns None if index >= len.
    pub fn get_recent(&self, index: usize) -> Option<&TraceEntry> {
        if index >= self.len {
            return None;
        }
        let idx = (self.head + self.capacity - 1 - index) % self.capacity;
        Some(&self.entries[idx])
    }

    /// Get entry by absolute index (0 = oldest).
    /// Used by TRACE_READ opcode for sequential access.
    /// Returns None if index >= len.
    pub fn get_at(&self, index: usize) -> Option<&TraceEntry> {
        if index >= self.len {
            return None;
        }
        let start = if self.len < self.capacity {
            0
        } else {
            self.head
        };
        let idx = (start + index) % self.capacity;
        Some(&self.entries[idx])
    }

    /// Count entries with a specific opcode.
    pub fn count_opcode(&self, target: u32) -> usize {
        self.iter().filter(|e| e.opcode == target).count()
    }

    /// Collect indices of entries with a specific opcode.
    /// Returns at most `max_results` indices (oldest to newest).
    pub fn find_opcode_indices(&self, target: u32, max_results: usize) -> Vec<usize> {
        let mut result = Vec::with_capacity(max_results);
        for (i, entry) in self.iter().enumerate() {
            if entry.opcode == target {
                result.push(i);
                if result.len() >= max_results {
                    break;
                }
            }
        }
        result
    }

    /// Replay backward from a given step number.
    /// Returns entries in reverse chronological order (newest first) starting
    /// at or before the given step number. Limited to `limit` entries.
    /// If step > current step_counter, starts from the most recent entry.
    pub fn replay_from(&self, step: u64, limit: usize) -> Vec<TraceEntry> {
        if self.len == 0 {
            return Vec::new();
        }

        // Find the starting index: the most recent entry with step_number <= step
        let start_idx = if step >= self.step_counter {
            // Start from most recent
            0
        } else {
            // Walk from newest backward to find first entry with step_number <= step
            let mut found = 0;
            for i in 0..self.len {
                if let Some(entry) = self.get_recent(i) {
                    if entry.step_number <= step {
                        found = i;
                        break;
                    }
                }
            }
            found
        };

        let limit = limit.min(self.len - start_idx);
        let mut result = Vec::with_capacity(limit);
        for i in start_idx..(start_idx + limit) {
            if let Some(entry) = self.get_recent(i) {
                result.push(entry.clone());
            }
        }
        result
    }

    /// Iterate over entries from newest to oldest (reverse order).
    pub fn iter_rev(&self) -> TraceRevIter<'_> {
        TraceRevIter {
            buffer: self,
            pos: 0,
        }
    }
}

/// Iterator over trace entries from oldest to newest.
#[allow(dead_code)]
pub struct TraceIter<'a> {
    buffer: &'a TraceBuffer,
    pos: usize,
    start: usize,
}

impl<'a> Iterator for TraceIter<'a> {
    type Item = &'a TraceEntry;

    fn next(&mut self) -> Option<Self::Item> {
        if self.pos >= self.buffer.len {
            return None;
        }
        let idx = (self.start + self.pos) % self.buffer.capacity;
        self.pos += 1;
        Some(&self.buffer.entries[idx])
    }
}

/// Iterator over trace entries from newest to oldest.
#[allow(dead_code)]
pub struct TraceRevIter<'a> {
    buffer: &'a TraceBuffer,
    pos: usize,
}

impl<'a> Iterator for TraceRevIter<'a> {
    type Item = &'a TraceEntry;

    fn next(&mut self) -> Option<Self::Item> {
        if self.pos >= self.buffer.len {
            return None;
        }
        let idx = (self.buffer.head + self.buffer.capacity - 1 - self.pos) % self.buffer.capacity;
        self.pos += 1;
        Some(&self.buffer.entries[idx])
    }
}

// --- Phase 38b: Frame Checkpointing ---

/// A snapshot of the screen buffer captured at a FRAME opcode.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct FrameCheckpoint {
    /// Step number at which this frame was captured.
    pub step_number: u64,
    /// The frame_count value when this checkpoint was taken.
    pub frame_count: u32,
    /// Full screen buffer snapshot (256x256 = 65536 u32s).
    pub screen: Vec<u32>,
}

/// Ring buffer of frame checkpoints.
///
/// Unlike TraceBuffer (which pre-allocates entries), this uses a Vec ring
/// buffer because each frame is 256KB. Frames are only allocated when pushed
/// (only when trace_recording is on and a FRAME opcode fires).
#[derive(Debug)]
pub struct FrameCheckBuffer {
    entries: Vec<Option<FrameCheckpoint>>,
    capacity: usize,
    head: usize, // next write position
    len: usize,  // number of valid entries (up to capacity)
}

#[allow(dead_code)]
impl FrameCheckBuffer {
    /// Create a new frame checkpoint buffer with the given capacity.
    pub fn new(capacity: usize) -> Self {
        let capacity = capacity.max(1);
        FrameCheckBuffer {
            entries: (0..capacity).map(|_| None).collect(),
            capacity,
            head: 0,
            len: 0,
        }
    }

    /// Push a new frame checkpoint into the ring buffer.
    /// Overwrites the oldest entry if the buffer is full.
    /// The `None` slot is reused; the `Some` slot's Vec is replaced.
    pub fn push(&mut self, step_number: u64, frame_count: u32, screen: &[u32]) {
        let checkpoint = FrameCheckpoint {
            step_number,
            frame_count,
            screen: screen.to_vec(),
        };
        self.entries[self.head] = Some(checkpoint);
        self.head = (self.head + 1) % self.capacity;
        if self.len < self.capacity {
            self.len += 1;
        }
    }

    /// Number of valid frame checkpoints currently in the buffer.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Clear all frame checkpoints.
    pub fn clear(&mut self) {
        for entry in self.entries.iter_mut() {
            entry.take();
        }
        self.head = 0;
        self.len = 0;
    }

    /// Get the Nth most recent frame checkpoint (0 = newest).
    /// Returns None if index >= len.
    pub fn get_recent(&self, index: usize) -> Option<&FrameCheckpoint> {
        if index >= self.len {
            return None;
        }
        let idx = (self.head + self.capacity - 1 - index) % self.capacity;
        self.entries[idx].as_ref()
    }

    /// Iterate over frame checkpoints from oldest to newest.
    pub fn iter(&self) -> FrameCheckIter<'_> {
        let start = if self.len < self.capacity {
            0
        } else {
            self.head
        };
        FrameCheckIter {
            buffer: self,
            pos: 0,
            start,
        }
    }

    /// Replay a frame: return a cloned screen buffer for the Nth most recent checkpoint.
    /// Returns None if index >= len.
    pub fn replay_frame(&self, index: usize) -> Option<Vec<u32>> {
        self.get_recent(index).map(|cp| cp.screen.clone())
    }
}

/// Iterator over frame checkpoints from oldest to newest.
#[allow(dead_code)]
pub struct FrameCheckIter<'a> {
    buffer: &'a FrameCheckBuffer,
    pos: usize,
    start: usize,
}

impl<'a> Iterator for FrameCheckIter<'a> {
    type Item = &'a FrameCheckpoint;

    fn next(&mut self) -> Option<Self::Item> {
        if self.pos >= self.buffer.len {
            return None;
        }
        let idx = (self.start + self.pos) % self.buffer.capacity;
        self.pos += 1;
        self.buffer.entries[idx].as_ref()
    }
}

// --- Phase 38d: Timeline Forking ---

/// Maximum number of VM snapshots that can be stored simultaneously.
/// At 256KB per snapshot (64K u32 RAM), 16 snapshots = 4MB total.
pub const MAX_SNAPSHOTS: usize = 16;

// --- Phase 54: Pixel Write History ---

/// Default pixel write log capacity (entries).
/// Each entry is 20 bytes (x: u16, y: u16, step_lo: u32, step_hi: u32, opcode: u8, color: u32).
/// 50K entries = ~1MB. Records individual pixel writes from PSET/PSETI.
pub const DEFAULT_PIXEL_WRITE_CAPACITY: usize = 50_000;

/// A single recorded pixel write event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PixelWriteEntry {
    /// X coordinate of the pixel written (0-255).
    pub x: u16,
    /// Y coordinate of the pixel written (0-255).
    pub y: u16,
    /// Step number when this write occurred (low 32 bits).
    pub step_lo: u32,
    /// Step number when this write occurred (high 32 bits).
    pub step_hi: u32,
    /// The opcode that caused this write (e.g., 0x40 for PSET, 0x41 for PSETI).
    pub opcode: u8,
    /// The color value written to the pixel.
    pub color: u32,
}

impl PixelWriteEntry {
    /// Get the full step number as u64.
    pub fn step(&self) -> u64 {
        (self.step_hi as u64) << 32 | (self.step_lo as u64)
    }
}

/// Fixed-size ring buffer of pixel write events.
///
/// Records every individual pixel write (PSET/PSETI) when trace recording is on.
/// Zero overhead when off (one bool check per PSET). Ring buffer allocated once,
/// never grows. Bulk operations (FILL, RECTF, etc.) are NOT logged per-pixel --
/// they record a single "region marker" entry at the origin pixel.
#[derive(Debug)]
pub struct PixelWriteLog {
    entries: Vec<PixelWriteEntry>,
    capacity: usize,
    head: usize, // next write position
    len: usize,  // number of valid entries (up to capacity)
}

#[allow(dead_code)]
impl PixelWriteLog {
    /// Create a new pixel write log with the given capacity.
    pub fn new(capacity: usize) -> Self {
        let capacity = capacity.max(1);
        PixelWriteLog {
            entries: (0..capacity)
                .map(|_| PixelWriteEntry {
                    x: 0,
                    y: 0,
                    step_lo: 0,
                    step_hi: 0,
                    opcode: 0,
                    color: 0,
                })
                .collect(),
            capacity,
            head: 0,
            len: 0,
        }
    }

    /// Record a pixel write event.
    /// Only call this when trace recording is on.
    #[inline]
    pub fn push(&mut self, x: u16, y: u16, step: u64, opcode: u8, color: u32) {
        let entry = &mut self.entries[self.head];
        entry.x = x;
        entry.y = y;
        entry.step_lo = step as u32;
        entry.step_hi = (step >> 32) as u32;
        entry.opcode = opcode;
        entry.color = color;

        self.head = (self.head + 1) % self.capacity;
        if self.len < self.capacity {
            self.len += 1;
        }
    }

    /// Number of valid entries currently in the buffer.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Clear all entries.
    pub fn clear(&mut self) {
        self.head = 0;
        self.len = 0;
    }

    /// Get entry by absolute index (0 = oldest).
    /// Returns None if index >= len.
    pub fn get_at(&self, index: usize) -> Option<&PixelWriteEntry> {
        if index >= self.len {
            return None;
        }
        let start = if self.len < self.capacity {
            0
        } else {
            self.head
        };
        let idx = (start + index) % self.capacity;
        Some(&self.entries[idx])
    }

    /// Count writes to a specific pixel (x, y).
    pub fn count_at(&self, x: u16, y: u16) -> usize {
        self.iter().filter(|e| e.x == x && e.y == y).count()
    }

    /// Get the N most recent writes to a specific pixel (x, y).
    /// Returns entries in reverse chronological order (newest first).
    /// Limits results to `max_results`.
    pub fn recent_at(&self, x: u16, y: u16, max_results: usize) -> Vec<PixelWriteEntry> {
        let mut result = Vec::with_capacity(max_results);
        for entry in self.iter_rev() {
            if entry.x == x && entry.y == y {
                result.push(entry.clone());
                if result.len() >= max_results {
                    break;
                }
            }
        }
        result
    }

    /// Iterate over entries from oldest to newest.
    pub fn iter(&self) -> PixelWriteIter<'_> {
        let start = if self.len < self.capacity {
            0
        } else {
            self.head
        };
        PixelWriteIter {
            log: self,
            pos: 0,
            start,
        }
    }

    /// Iterate over entries from newest to oldest.
    pub fn iter_rev(&self) -> PixelWriteRevIter<'_> {
        PixelWriteRevIter {
            log: self,
            pos: 0,
        }
    }
}

/// Iterator over pixel write entries from oldest to newest.
pub struct PixelWriteIter<'a> {
    log: &'a PixelWriteLog,
    pos: usize,
    start: usize,
}

impl<'a> Iterator for PixelWriteIter<'a> {
    type Item = &'a PixelWriteEntry;

    fn next(&mut self) -> Option<Self::Item> {
        if self.pos >= self.log.len {
            return None;
        }
        let idx = (self.start + self.pos) % self.log.capacity;
        self.pos += 1;
        Some(&self.log.entries[idx])
    }
}

/// Iterator over pixel write entries from newest to oldest.
pub struct PixelWriteRevIter<'a> {
    log: &'a PixelWriteLog,
    pos: usize,
}

impl<'a> Iterator for PixelWriteRevIter<'a> {
    type Item = &'a PixelWriteEntry;

    fn next(&mut self) -> Option<Self::Item> {
        if self.pos >= self.log.len {
            return None;
        }
        let idx = (self.log.head + self.log.capacity - 1 - self.pos) % self.log.capacity;
        self.pos += 1;
        Some(&self.log.entries[idx])
    }
}

// --- Phase 38d: Timeline Forking ---

/// A complete snapshot of VM state, used for timeline forking.
///
/// Captures everything needed to restore the VM to an exact prior state:
/// full RAM, screen buffer, all registers, PC, and configuration fields.
/// This enables "what if" exploration -- save a snapshot, run forward,
/// then restore and try a different path.
///
/// Size: ~384KB per snapshot (64K RAM words + 64K screen words + overhead).
/// Delta compression can reduce this later; for now, keep it simple.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct VmSnapshot {
    /// Full RAM contents (65536 u32 words = 256KB).
    pub ram: Vec<u32>,
    /// Full screen buffer (65536 u32 words = 256KB).
    pub screen: Vec<u32>,
    /// All 32 general-purpose registers.
    pub regs: [u32; 32],
    /// Program counter.
    pub pc: u32,
    /// CPU privilege mode (Kernel/User).
    pub mode: super::types::CpuMode,
    /// Whether the VM was halted.
    pub halted: bool,
    /// Frame counter value.
    pub frame_count: u32,
    /// LCG random state.
    pub rand_state: u32,
    /// Current PID.
    pub current_pid: u32,
    /// Step number when this snapshot was taken (from trace buffer).
    pub step_number: u64,
}
