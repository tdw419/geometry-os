// scrollback.rs -- Terminal scrollback ring buffer for Geometry OS
// Stores past terminal output lines so PageUp/PageDown can scroll through history.

/// Canvas dimensions (must match render.rs SCROLLBACK_COLS/SCROLLBACK_VISIBLE_ROWS).
pub const SCROLLBACK_COLS: usize = 32;
pub const SCROLLBACK_VISIBLE_ROWS: usize = 32;

/// Number of lines stored in the scrollback ring buffer.
pub const SCROLLBACK_CAPACITY: usize = 1024;

/// A ring buffer of terminal output lines for scrollback navigation.
/// Each line is SCROLLBACK_COLS u32 values (matching canvas_buffer format).
pub struct ScrollbackBuffer {
    /// Ring buffer of lines. lines[slot][0..SCROLLBACK_COLS] = one row of output.
    lines: Vec<[u32; SCROLLBACK_COLS]>,
    /// Total number of lines ever written (may exceed capacity).
    total_written: usize,
    /// Index of the oldest line in the ring buffer.
    oldest: usize,
    /// Number of valid lines currently in the buffer (min(total_written, capacity)).
    count: usize,
}

impl ScrollbackBuffer {
    pub fn new() -> Self {
        let mut lines = Vec::with_capacity(SCROLLBACK_CAPACITY);
        for _ in 0..SCROLLBACK_CAPACITY {
            lines.push([0u32; SCROLLBACK_COLS]);
        }
        ScrollbackBuffer {
            lines,
            total_written: 0,
            oldest: 0,
            count: 0,
        }
    }

    /// Push a single line into the ring buffer.
    /// `line` is SCROLLBACK_COLS u32 values from the canvas buffer.
    pub fn push_line(&mut self, line: &[u32]) {
        let idx = (self.oldest + self.count) % SCROLLBACK_CAPACITY;
        let dest = &mut self.lines[idx];
        dest.copy_from_slice(&line[..SCROLLBACK_COLS]);
        if self.count < SCROLLBACK_CAPACITY {
            self.count += 1;
        } else {
            self.oldest = (self.oldest + 1) % SCROLLBACK_CAPACITY;
        }
        self.total_written += 1;
    }

    /// Push all non-empty rows from a canvas buffer section [start_row..end_row).
    /// Empty rows (all zeros) are skipped.
    pub fn push_canvas_rows(&mut self, canvas_buffer: &[u32], start_row: usize, end_row: usize) {
        let max_row = canvas_buffer.len() / SCROLLBACK_COLS;
        let end_row = end_row.min(max_row);
        for row in start_row..end_row {
            let offset = row * SCROLLBACK_COLS;
            // Check if row has any non-zero content
            let mut has_content = false;
            for col in 0..SCROLLBACK_COLS {
                if canvas_buffer[offset + col] != 0 {
                    has_content = true;
                    break;
                }
            }
            if has_content {
                let mut line = [0u32; SCROLLBACK_COLS];
                line.copy_from_slice(&canvas_buffer[offset..offset + SCROLLBACK_COLS]);
                self.push_line(&line);
            }
        }
    }

    /// Number of valid lines in the buffer.
    pub fn len(&self) -> usize {
        self.count
    }

    /// Total lines ever written (for debugging).
    pub fn total_written(&self) -> usize {
        self.total_written
    }

    /// Get a line by absolute index (0 = oldest line in buffer).
    /// Returns None if index >= count.
    pub fn get_line(&self, index: usize) -> Option<&[u32; SCROLLBACK_COLS]> {
        if index >= self.count {
            return None;
        }
        let idx = (self.oldest + index) % SCROLLBACK_CAPACITY;
        Some(&self.lines[idx])
    }

    /// Get lines for display at a given scroll position.
    /// Returns up to SCROLLBACK_VISIBLE_ROWS lines starting from `scroll_offset` in the buffer.
    /// The returned slice contains references to the ring buffer entries.
    pub fn get_page(&self, scroll_offset: usize) -> Vec<&[u32; SCROLLBACK_COLS]> {
        let mut result = Vec::with_capacity(SCROLLBACK_VISIBLE_ROWS);
        for i in 0..SCROLLBACK_VISIBLE_ROWS {
            if let Some(line) = self.get_line(scroll_offset + i) {
                result.push(line);
            } else {
                break;
            }
        }
        result
    }

    /// Maximum scroll position (count - visible_rows).
    pub fn max_scroll(&self) -> usize {
        if self.count > SCROLLBACK_VISIBLE_ROWS {
            self.count - SCROLLBACK_VISIBLE_ROWS
        } else {
            0
        }
    }

    /// Check if the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scrollback_push_and_get() {
        let mut sb = ScrollbackBuffer::new();
        let mut line = [0u32; SCROLLBACK_COLS];
        line[0] = b'H' as u32;
        line[1] = b'i' as u32;
        sb.push_line(&line);
        assert_eq!(sb.len(), 1);
        let got = sb.get_line(0).unwrap();
        assert_eq!(got[0], b'H' as u32);
        assert_eq!(got[1], b'i' as u32);
    }

    #[test]
    fn test_scrollback_ring_wrap() {
        let mut sb = ScrollbackBuffer::new();
        // Fill beyond capacity
        for i in 0..SCROLLBACK_CAPACITY + 100 {
            let mut line = [0u32; SCROLLBACK_COLS];
            line[0] = (i as u32) & 0xFF;
            sb.push_line(&line);
        }
        assert_eq!(sb.len(), SCROLLBACK_CAPACITY);
        assert_eq!(sb.total_written(), SCROLLBACK_CAPACITY + 100);
        // First line should be the (100)th entry
        let first = sb.get_line(0).unwrap();
        assert_eq!(first[0], 100);
    }

    #[test]
    fn test_scrollback_page() {
        let mut sb = ScrollbackBuffer::new();
        for i in 0..50 {
            let mut line = [0u32; SCROLLBACK_COLS];
            line[0] = i as u32;
            sb.push_line(&line);
        }
        let page = sb.get_page(0);
        assert_eq!(page.len(), SCROLLBACK_VISIBLE_ROWS.min(50));
        assert_eq!(page[0][0], 0);
    }

    #[test]
    fn test_scrollback_max_scroll() {
        let mut sb = ScrollbackBuffer::new();
        for _ in 0..50 {
            let line = [0u32; SCROLLBACK_COLS];
            sb.push_line(&line);
        }
        let expected = if 50 > SCROLLBACK_VISIBLE_ROWS {
            50 - SCROLLBACK_VISIBLE_ROWS
        } else {
            0
        };
        assert_eq!(sb.max_scroll(), expected);
    }

    #[test]
    fn test_scrollback_push_canvas_rows() {
        let mut sb = ScrollbackBuffer::new();
        let mut canvas = vec![0u32; 10 * SCROLLBACK_COLS];
        // Write "Hello" at row 2
        canvas[2 * SCROLLBACK_COLS] = b'H' as u32;
        canvas[2 * SCROLLBACK_COLS + 1] = b'e' as u32;
        // Write "World" at row 5
        canvas[5 * SCROLLBACK_COLS] = b'W' as u32;
        canvas[5 * SCROLLBACK_COLS + 1] = b'o' as u32;

        sb.push_canvas_rows(&canvas, 0, 8);
        assert_eq!(sb.len(), 2); // only rows 2 and 5 have content
        assert_eq!(sb.get_line(0).unwrap()[0], b'H' as u32);
        assert_eq!(sb.get_line(1).unwrap()[0], b'W' as u32);
    }

    #[test]
    fn test_scrollback_1000_lines() {
        let mut sb = ScrollbackBuffer::new();
        for i in 0..1100 {
            let mut line = [0u32; SCROLLBACK_COLS];
            line[0] = (i % 256) as u32;
            line[1] = ((i / 256) % 256) as u32;
            sb.push_line(&line);
        }
        assert_eq!(sb.len(), SCROLLBACK_CAPACITY);
        assert!(sb.max_scroll() >= SCROLLBACK_CAPACITY - SCROLLBACK_VISIBLE_ROWS);
        // Can scroll through all valid lines
        let page = sb.get_page(sb.max_scroll());
        assert!(!page.is_empty());
    }
}
