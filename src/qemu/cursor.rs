// qemu/cursor.rs -- Virtual cursor for the canvas text surface
//
// Tracks row/col position with helpers for advancing, newlines,
// carriage returns, and bounds clamping.

// ── Constants ────────────────────────────────────────────────────
pub const CANVAS_COLS: usize = 32;
pub const CANVAS_MAX_ROWS: usize = 128;

/// Virtual cursor position for the canvas text surface.
#[derive(Debug, Clone, Copy, Default)]
pub struct Cursor {
    pub row: usize,
    pub col: usize,
}

impl Cursor {
    /// Create a new cursor at position (0, 0).
    pub fn new() -> Self {
        Cursor::default()
    }

    /// Advance cursor by one character, wrapping at CANVAS_COLS.
    pub fn advance(&mut self) {
        self.col += 1;
        if self.col >= CANVAS_COLS {
            self.col = 0;
            self.row += 1;
        }
    }

    /// Newline: move to start of next row.
    pub fn newline(&mut self) {
        self.col = 0;
        self.row += 1;
    }

    /// Carriage return: move to start of current row.
    pub fn carriage_return(&mut self) {
        self.col = 0;
    }

    /// Clamp cursor position to valid canvas bounds.
    pub fn clamp(&mut self) {
        if self.row >= CANVAS_MAX_ROWS {
            self.row = CANVAS_MAX_ROWS - 1;
        }
        if self.col >= CANVAS_COLS {
            self.col = CANVAS_COLS - 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cursor_advance_no_wrap() {
        let mut cursor = Cursor::new();
        cursor.col = 10;
        cursor.advance();
        assert_eq!(cursor.col, 11);
    }

    #[test]
    fn test_cursor_advance_wrap() {
        let mut cursor = Cursor::new();
        cursor.col = CANVAS_COLS - 1;
        cursor.advance();
        assert_eq!(cursor.col, 0);
        assert_eq!(cursor.row, 1);
    }

    #[test]
    fn test_cursor_newline() {
        let mut cursor = Cursor::new();
        cursor.col = 15;
        cursor.newline();
        assert_eq!(cursor.col, 0);
        assert_eq!(cursor.row, 1);
    }

    #[test]
    fn test_cursor_carriage_return() {
        let mut cursor = Cursor::new();
        cursor.col = 20;
        cursor.carriage_return();
        assert_eq!(cursor.col, 0);
        assert_eq!(cursor.row, 0);
    }

    #[test]
    fn test_cursor_clamp() {
        let mut cursor = Cursor::new();
        cursor.row = CANVAS_MAX_ROWS + 5;
        cursor.col = CANVAS_COLS + 5;
        cursor.clamp();
        assert_eq!(cursor.row, CANVAS_MAX_ROWS - 1);
        assert_eq!(cursor.col, CANVAS_COLS - 1);
    }
}
