// vm/trace.rs -- Execution Trace Ring Buffer (Phase 38a)
//
// Records every instruction execution to a fixed-size ring buffer.
// Zero overhead when recording is off (one bool check per step).
// Ring buffer allocated once, never grows. No heap allocation in the hot path.

/// Default ring buffer capacity (entries).
pub const DEFAULT_TRACE_CAPACITY: usize = 10_000;

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
