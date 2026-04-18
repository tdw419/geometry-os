// vm/trace.rs -- Execution Trace Ring Buffer (Phase 38a)
//                 Frame Checkpointing (Phase 38b)
//
// Records every instruction execution to a fixed-size ring buffer.
// Zero overhead when recording is off (one bool check per step).
// Ring buffer allocated once, never grows. No heap allocation in the hot path.
//
// Phase 38b: Snapshots the full screen buffer at every FRAME opcode.
// Combined with the trace ring buffer, you can reconstruct the screen
// at any point without re-executing.

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
    head: usize,   // next write position
    len: usize,    // number of valid entries (up to capacity)
    step_counter: u64, // monotonically increasing step counter
}

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
    head: usize,  // next write position
    len: usize,   // number of valid entries (up to capacity)
}

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
