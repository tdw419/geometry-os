// qemu/ansi.rs -- ANSI escape sequence handler and canvas cursor
//
// Parses ANSI escape sequences from QEMU stdout and writes
// printable characters into the Geometry OS canvas text buffer.

// ── Constants ────────────────────────────────────────────────────
pub(crate) const CANVAS_COLS: usize = 32;
pub(crate) const CANVAS_MAX_ROWS: usize = 128;

// ── ANSI Escape State Machine ────────────────────────────────────

/// States for the ANSI escape sequence parser.
#[derive(Debug, Clone, Copy, PartialEq)]
enum AnsiState {
    /// Normal text processing.
    Normal,
    /// Received ESC (0x1B), waiting for next char.
    Escape,
    /// Received ESC [, collecting CSI parameters.
    Csi,
    /// Received CSI ?, collecting private mode parameters.
    CsiPrivate,
}

/// Virtual cursor position for the canvas text surface.
#[derive(Debug, Clone, Copy)]
#[derive(Default)]
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

/// ANSI escape sequence handler with canvas buffer writing.
pub struct AnsiHandler {
    state: AnsiState,
    cursor: Cursor,
    /// CSI parameter digits being collected.
    csi_params: String,
    /// Saved cursor position for ESC 7 / ESC 8.
    saved_cursor: Cursor,
    /// Scroll region top (inclusive).
    scroll_top: usize,
    /// Scroll region bottom (inclusive).
    scroll_bottom: usize,
}

impl Default for AnsiHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl AnsiHandler {
    /// Create a new ANSI handler with default state.
    pub fn new() -> Self {
        AnsiHandler {
            state: AnsiState::Normal,
            cursor: Cursor::new(),
            csi_params: String::new(),
            saved_cursor: Cursor::new(),
            scroll_top: 0,
            scroll_bottom: CANVAS_MAX_ROWS - 1,
        }
    }

    /// Get the current cursor position.
    pub fn cursor(&self) -> Cursor {
        self.cursor
    }

    /// Set cursor position directly.
    pub fn set_cursor(&mut self, row: usize, col: usize) {
        self.cursor.row = row;
        self.cursor.col = col;
        self.cursor.clamp();
    }

    /// Process a slice of bytes from QEMU stdout.
    /// Writes printable characters into canvas_buffer.
    pub fn process_bytes(&mut self, bytes: &[u8], canvas_buffer: &mut [u32]) {
        for &b in bytes {
            self.process_byte(b, canvas_buffer);
        }
    }

    /// Process a single byte.
    fn process_byte(&mut self, b: u8, canvas_buffer: &mut [u32]) {
        match self.state {
            AnsiState::Normal => {
                match b {
                    0x1B => {
                        self.state = AnsiState::Escape;
                    }
                    0x0A => {
                        self.cursor.newline();
                        self.auto_scroll(canvas_buffer);
                    }
                    0x0D => {
                        self.cursor.carriage_return();
                    }
                    0x08 => {
                        if self.cursor.col > 0 {
                            self.cursor.col -= 1;
                        }
                    }
                    0x09 => {
                        let next_tab = ((self.cursor.col / 8) + 1) * 8;
                        self.cursor.col = next_tab.min(CANVAS_COLS - 1);
                    }
                    0x07 => {
                        // Bell -- ignore
                    }
                    _ => {
                        if (0x20..0x7F).contains(&b) {
                            if self.cursor.row < CANVAS_MAX_ROWS {
                                let idx =
                                    self.cursor.row * CANVAS_COLS + self.cursor.col;
                                if idx < canvas_buffer.len() {
                                    canvas_buffer[idx] = b as u32;
                                }
                            }
                            self.cursor.advance();
                            self.auto_scroll(canvas_buffer);
                        }
                    }
                }
            }
            AnsiState::Escape => {
                match b {
                    b'[' => {
                        self.state = AnsiState::Csi;
                        self.csi_params.clear();
                    }
                    b'7' => {
                        self.saved_cursor = self.cursor;
                        self.state = AnsiState::Normal;
                    }
                    b'8' => {
                        self.cursor = self.saved_cursor;
                        self.state = AnsiState::Normal;
                    }
                    b'D' => {
                        self.cursor.newline();
                        self.auto_scroll(canvas_buffer);
                        self.state = AnsiState::Normal;
                    }
                    b'M' => {
                        if self.cursor.row > self.scroll_top {
                            self.cursor.row -= 1;
                        } else {
                            self.scroll_down(canvas_buffer);
                        }
                        self.state = AnsiState::Normal;
                    }
                    b'c' => {
                        self.cursor = Cursor::new();
                        self.saved_cursor = Cursor::new();
                        self.scroll_top = 0;
                        self.scroll_bottom = CANVAS_MAX_ROWS - 1;
                        self.state = AnsiState::Normal;
                    }
                    _ => {
                        self.state = AnsiState::Normal;
                    }
                }
            }
            AnsiState::Csi => {
                if b == b'?' {
                    self.state = AnsiState::CsiPrivate;
                    return;
                }
                if b.is_ascii_digit() || b == b';' {
                    self.csi_params.push(b as char);
                    return;
                }
                self.handle_csi(b, canvas_buffer);
                self.state = AnsiState::Normal;
            }
            AnsiState::CsiPrivate => {
                if b.is_ascii_digit() || b == b';' {
                    self.csi_params.push(b as char);
                    return;
                }
                self.handle_csi_private(b, canvas_buffer);
                self.state = AnsiState::Normal;
            }
        }
    }

    /// Parse CSI parameters into a list of integers.
    fn parse_params(&self, defaults: &[u32]) -> Vec<u32> {
        if self.csi_params.is_empty() {
            return defaults.to_vec();
        }
        let parts: Vec<&str> = self.csi_params.split(';').collect();
        let mut result = Vec::with_capacity(parts.len().max(defaults.len()));
        for (i, part) in parts.iter().enumerate() {
            if part.is_empty() {
                result.push(defaults.get(i).copied().unwrap_or(0));
            } else {
                result.push(
                    part.parse()
                        .unwrap_or(defaults.get(i).copied().unwrap_or(0)),
                );
            }
        }
        while result.len() < defaults.len() {
            result.push(defaults[result.len()]);
        }
        result
    }

    /// Handle a CSI sequence final character.
    fn handle_csi(&mut self, final_byte: u8, canvas_buffer: &mut [u32]) {
        match final_byte {
            b'A' => {
                let params = self.parse_params(&[1]);
                let n = params[0] as usize;
                self.cursor.row = self.cursor.row.saturating_sub(n);
                if self.cursor.row < self.scroll_top {
                    self.cursor.row = self.scroll_top;
                }
            }
            b'B' => {
                let params = self.parse_params(&[1]);
                let n = params[0] as usize;
                self.cursor.row = (self.cursor.row + n).min(self.scroll_bottom);
            }
            b'C' => {
                let params = self.parse_params(&[1]);
                let n = params[0] as usize;
                self.cursor.col = (self.cursor.col + n).min(CANVAS_COLS - 1);
            }
            b'D' => {
                let params = self.parse_params(&[1]);
                let n = params[0] as usize;
                self.cursor.col = self.cursor.col.saturating_sub(n);
            }
            b'E' => {
                let params = self.parse_params(&[1]);
                let n = params[0] as usize;
                self.cursor.col = 0;
                self.cursor.row = (self.cursor.row + n).min(self.scroll_bottom);
            }
            b'F' => {
                let params = self.parse_params(&[1]);
                let n = params[0] as usize;
                self.cursor.col = 0;
                self.cursor.row = self.cursor.row.saturating_sub(n);
                if self.cursor.row < self.scroll_top {
                    self.cursor.row = self.scroll_top;
                }
            }
            b'G' => {
                let params = self.parse_params(&[1]);
                self.cursor.col =
                    (params[0] as usize).saturating_sub(1).min(CANVAS_COLS - 1);
            }
            b'd' => {
                let params = self.parse_params(&[1]);
                self.cursor.row =
                    (params[0] as usize).saturating_sub(1).min(CANVAS_MAX_ROWS - 1);
            }
            b'H' | b'f' => {
                let params = self.parse_params(&[1, 1]);
                self.cursor.row =
                    (params[0] as usize).saturating_sub(1).min(CANVAS_MAX_ROWS - 1);
                self.cursor.col =
                    (params[1] as usize).saturating_sub(1).min(CANVAS_COLS - 1);
            }
            b'J' => {
                let params = self.parse_params(&[0]);
                match params[0] {
                    0 => {
                        // Clear from cursor to end of screen
                        for c in self.cursor.col..CANVAS_COLS {
                            let idx = self.cursor.row * CANVAS_COLS + c;
                            if idx < canvas_buffer.len() {
                                canvas_buffer[idx] = 0;
                            }
                        }
                        for r in (self.cursor.row + 1)..CANVAS_MAX_ROWS {
                            for c in 0..CANVAS_COLS {
                                let idx = r * CANVAS_COLS + c;
                                if idx < canvas_buffer.len() {
                                    canvas_buffer[idx] = 0;
                                }
                            }
                        }
                    }
                    1 => {
                        // Clear from start of screen to cursor
                        for r in 0..self.cursor.row {
                            for c in 0..CANVAS_COLS {
                                let idx = r * CANVAS_COLS + c;
                                if idx < canvas_buffer.len() {
                                    canvas_buffer[idx] = 0;
                                }
                            }
                        }
                        for c in 0..=self.cursor.col {
                            let idx = self.cursor.row * CANVAS_COLS + c;
                            if idx < canvas_buffer.len() {
                                canvas_buffer[idx] = 0;
                            }
                        }
                    }
                    2 | 3 => {
                        // Clear entire screen
                        let end = canvas_buffer.len().min(CANVAS_MAX_ROWS * CANVAS_COLS);
                        for cell in canvas_buffer.iter_mut().take(end) {
                            *cell = 0;
                        }
                        self.cursor.row = 0;
                        self.cursor.col = 0;
                    }
                    _ => {}
                }
            }
            b'K' => {
                let params = self.parse_params(&[0]);
                match params[0] {
                    0 => {
                        for c in self.cursor.col..CANVAS_COLS {
                            let idx = self.cursor.row * CANVAS_COLS + c;
                            if idx < canvas_buffer.len() {
                                canvas_buffer[idx] = 0;
                            }
                        }
                    }
                    1 => {
                        for c in 0..=self.cursor.col {
                            let idx = self.cursor.row * CANVAS_COLS + c;
                            if idx < canvas_buffer.len() {
                                canvas_buffer[idx] = 0;
                            }
                        }
                    }
                    2 => {
                        for c in 0..CANVAS_COLS {
                            let idx = self.cursor.row * CANVAS_COLS + c;
                            if idx < canvas_buffer.len() {
                                canvas_buffer[idx] = 0;
                            }
                        }
                    }
                    _ => {}
                }
            }
            b'L' => {
                let params = self.parse_params(&[1]);
                let n = (params[0] as usize).min(CANVAS_MAX_ROWS - self.cursor.row);
                for r in (self.cursor.row..CANVAS_MAX_ROWS - n).rev() {
                    for c in 0..CANVAS_COLS {
                        let dst = (r + n) * CANVAS_COLS + c;
                        let src = r * CANVAS_COLS + c;
                        if dst < canvas_buffer.len() && src < canvas_buffer.len() {
                            canvas_buffer[dst] = canvas_buffer[src];
                        }
                    }
                }
                for r in self.cursor.row..self.cursor.row + n {
                    for c in 0..CANVAS_COLS {
                        let idx = r * CANVAS_COLS + c;
                        if idx < canvas_buffer.len() {
                            canvas_buffer[idx] = 0;
                        }
                    }
                }
            }
            b'M' => {
                let params = self.parse_params(&[1]);
                let n = (params[0] as usize).min(CANVAS_MAX_ROWS - self.cursor.row);
                for r in self.cursor.row..CANVAS_MAX_ROWS - n {
                    for c in 0..CANVAS_COLS {
                        let src = (r + n) * CANVAS_COLS + c;
                        let dst = r * CANVAS_COLS + c;
                        if src < canvas_buffer.len() && dst < canvas_buffer.len() {
                            canvas_buffer[dst] = canvas_buffer[src];
                        }
                    }
                }
                for r in (CANVAS_MAX_ROWS - n)..CANVAS_MAX_ROWS {
                    for c in 0..CANVAS_COLS {
                        let idx = r * CANVAS_COLS + c;
                        if idx < canvas_buffer.len() {
                            canvas_buffer[idx] = 0;
                        }
                    }
                }
            }
            b'P' => {
                let params = self.parse_params(&[1]);
                let n = (params[0] as usize).min(CANVAS_COLS - self.cursor.col);
                let row_start = self.cursor.row * CANVAS_COLS;
                for c in self.cursor.col..CANVAS_COLS - n {
                    let src = row_start + c + n;
                    let dst = row_start + c;
                    if src < canvas_buffer.len() && dst < canvas_buffer.len() {
                        canvas_buffer[dst] = canvas_buffer[src];
                    }
                }
                for c in (CANVAS_COLS - n)..CANVAS_COLS {
                    let idx = row_start + c;
                    if idx < canvas_buffer.len() {
                        canvas_buffer[idx] = 0;
                    }
                }
            }
            b'@' => {
                let params = self.parse_params(&[1]);
                let n = (params[0] as usize).min(CANVAS_COLS - self.cursor.col);
                let row_start = self.cursor.row * CANVAS_COLS;
                for c in (self.cursor.col..CANVAS_COLS - n).rev() {
                    let src = row_start + c;
                    let dst = row_start + c + n;
                    if src < canvas_buffer.len() && dst < canvas_buffer.len() {
                        canvas_buffer[dst] = canvas_buffer[src];
                    }
                }
                for c in self.cursor.col..self.cursor.col + n {
                    let idx = row_start + c;
                    if idx < canvas_buffer.len() {
                        canvas_buffer[idx] = 0;
                    }
                }
            }
            b'S' => {
                let params = self.parse_params(&[1]);
                let n = (params[0] as usize).min(CANVAS_MAX_ROWS);
                for _ in 0..n {
                    self.scroll_up(canvas_buffer);
                }
            }
            b'T' => {
                let params = self.parse_params(&[1]);
                let n = (params[0] as usize).min(CANVAS_MAX_ROWS);
                for _ in 0..n {
                    self.scroll_down(canvas_buffer);
                }
            }
            b'm' => {
                // SGR (color/style) -- ignore, we only render text
            }
            b'r' => {
                let params =
                    self.parse_params(&[1, CANVAS_MAX_ROWS as u32]);
                self.scroll_top = (params[0] as usize).saturating_sub(1);
                self.scroll_bottom = (params[1] as usize)
                    .saturating_sub(1)
                    .min(CANVAS_MAX_ROWS - 1);
                if self.scroll_top >= self.scroll_bottom {
                    self.scroll_top = 0;
                    self.scroll_bottom = CANVAS_MAX_ROWS - 1;
                }
                self.cursor.row = self.scroll_top;
                self.cursor.col = 0;
            }
            b's' => {
                self.saved_cursor = self.cursor;
            }
            b'u' => {
                self.cursor = self.saved_cursor;
            }
            _ => {
                // Unknown CSI -- ignore
            }
        }
    }

    /// Handle a private CSI sequence (ESC [ ? ...).
    fn handle_csi_private(
        &mut self,
        final_byte: u8,
        _canvas_buffer: &mut [u32],
    ) {
        match final_byte {
            b'h' | b'l' | b'J' => {
                // DEC private mode set/reset, erase scrollback -- ignore
            }
            _ => {
                // Unknown private CSI -- ignore
            }
        }
    }

    /// Auto-scroll when cursor moves past CANVAS_MAX_ROWS.
    fn auto_scroll(&mut self, canvas_buffer: &mut [u32]) {
        if self.cursor.row >= CANVAS_MAX_ROWS {
            self.scroll_up(canvas_buffer);
            self.cursor.row = CANVAS_MAX_ROWS - 1;
        }
    }

    /// Scroll the canvas up by one line.
    pub fn scroll_up(&self, canvas_buffer: &mut [u32]) {
        for r in 0..CANVAS_MAX_ROWS - 1 {
            for c in 0..CANVAS_COLS {
                let dst = r * CANVAS_COLS + c;
                let src = (r + 1) * CANVAS_COLS + c;
                if src < canvas_buffer.len() && dst < canvas_buffer.len() {
                    canvas_buffer[dst] = canvas_buffer[src];
                }
            }
        }
        let last_row = (CANVAS_MAX_ROWS - 1) * CANVAS_COLS;
        for c in 0..CANVAS_COLS {
            let idx = last_row + c;
            if idx < canvas_buffer.len() {
                canvas_buffer[idx] = 0;
            }
        }
    }

    /// Scroll the canvas down by one line.
    fn scroll_down(&self, canvas_buffer: &mut [u32]) {
        for r in (1..CANVAS_MAX_ROWS).rev() {
            for c in 0..CANVAS_COLS {
                let dst = r * CANVAS_COLS + c;
                let src = (r - 1) * CANVAS_COLS + c;
                if src < canvas_buffer.len() && dst < canvas_buffer.len() {
                    canvas_buffer[dst] = canvas_buffer[src];
                }
            }
        }
        for c in 0..CANVAS_COLS {
            if c < canvas_buffer.len() {
                canvas_buffer[c] = 0;
            }
        }
    }

    /// Clear the entire canvas buffer.
    pub fn clear_screen(&self, canvas_buffer: &mut [u32]) {
        let end = canvas_buffer.len().min(CANVAS_MAX_ROWS * CANVAS_COLS);
        for cell in canvas_buffer.iter_mut().take(end) {
            *cell = 0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_canvas() -> Vec<u32> {
        vec![0u32; CANVAS_MAX_ROWS * CANVAS_COLS]
    }

    // ── AnsiHandler tests ────────────────────────────────────────

    #[test]
    fn test_ansi_basic_text() {
        let mut handler = AnsiHandler::new();
        let mut buf = make_canvas();
        handler.process_bytes(b"Hello", &mut buf);
        assert_eq!(buf[0], b'H' as u32);
        assert_eq!(buf[1], b'e' as u32);
        assert_eq!(buf[2], b'l' as u32);
        assert_eq!(buf[3], b'l' as u32);
        assert_eq!(buf[4], b'o' as u32);
        let c = handler.cursor();
        assert_eq!(c.row, 0);
        assert_eq!(c.col, 5);
    }

    #[test]
    fn test_ansi_newline() {
        let mut handler = AnsiHandler::new();
        let mut buf = make_canvas();
        handler.process_bytes(b"AB\nCD", &mut buf);
        assert_eq!(buf[0], b'A' as u32);
        assert_eq!(buf[1], b'B' as u32);
        assert_eq!(buf[CANVAS_COLS], b'C' as u32);
        assert_eq!(buf[CANVAS_COLS + 1], b'D' as u32);
        let c = handler.cursor();
        assert_eq!(c.row, 1);
        assert_eq!(c.col, 2);
    }

    #[test]
    fn test_ansi_carriage_return() {
        let mut handler = AnsiHandler::new();
        let mut buf = make_canvas();
        handler.process_bytes(b"AB\rX", &mut buf);
        assert_eq!(buf[0], b'X' as u32); // CR moved to col 0, X overwrites A
        assert_eq!(buf[1], b'B' as u32);
        let c = handler.cursor();
        assert_eq!(c.col, 1); // X at col 0, cursor advances to 1
    }

    #[test]
    fn test_ansi_backspace() {
        let mut handler = AnsiHandler::new();
        let mut buf = make_canvas();
        handler.process_bytes(b"ABC\x08X", &mut buf);
        assert_eq!(buf[0], b'A' as u32);
        assert_eq!(buf[1], b'B' as u32);
        assert_eq!(buf[2], b'X' as u32);
        let c = handler.cursor();
        assert_eq!(c.col, 3);
    }

    #[test]
    fn test_ansi_cursor_up() {
        let mut handler = AnsiHandler::new();
        let mut buf = make_canvas();
        handler.process_bytes(b"Line1\nLine2\x1B[A", &mut buf);
        let c = handler.cursor();
        assert_eq!(c.row, 0); // Was at row 1 col 5, ESC[A moves up to row 0
    }

    #[test]
    fn test_ansi_cursor_down() {
        let mut handler = AnsiHandler::new();
        let mut buf = make_canvas();
        handler.process_bytes(b"AB\x1B[B", &mut buf);
        let c = handler.cursor();
        assert_eq!(c.row, 1);
    }

    #[test]
    fn test_ansi_cursor_right() {
        let mut handler = AnsiHandler::new();
        let mut buf = make_canvas();
        handler.process_bytes(b"AB\x1B[C", &mut buf);
        let c = handler.cursor();
        assert_eq!(c.col, 3);
    }

    #[test]
    fn test_ansi_cursor_left() {
        let mut handler = AnsiHandler::new();
        let mut buf = make_canvas();
        handler.process_bytes(b"ABC\x1B[D", &mut buf);
        let c = handler.cursor();
        assert_eq!(c.col, 2);
    }

    #[test]
    fn test_ansi_cursor_home() {
        let mut handler = AnsiHandler::new();
        let mut buf = make_canvas();
        handler.process_bytes(b"AB\nCD\x1B[H", &mut buf);
        let c = handler.cursor();
        assert_eq!(c.row, 0);
        assert_eq!(c.col, 0);
    }

    #[test]
    fn test_ansi_cursor_position() {
        let mut handler = AnsiHandler::new();
        let mut buf = make_canvas();
        handler.process_bytes(b"\x1B[5;10H", &mut buf);
        let c = handler.cursor();
        assert_eq!(c.row, 4);
        assert_eq!(c.col, 9);
    }

    #[test]
    fn test_ansi_clear_screen() {
        let mut handler = AnsiHandler::new();
        let mut buf = make_canvas();
        handler.process_bytes(b"Hello World\x1B[2J", &mut buf);
        for i in 0..100 {
            assert_eq!(buf[i], 0, "buffer[{}] should be 0 after clear", i);
        }
    }

    #[test]
    fn test_ansi_clear_from_cursor() {
        let mut handler = AnsiHandler::new();
        let mut buf = make_canvas();
        handler.process_bytes(b"ABCDE\x1B[1;3H\x1B[0J", &mut buf);
        assert_eq!(buf[0], b'A' as u32);
        assert_eq!(buf[1], b'B' as u32);
        assert_eq!(buf[2], 0);
        assert_eq!(buf[3], 0);
        assert_eq!(buf[4], 0);
    }

    #[test]
    fn test_ansi_clear_line() {
        let mut handler = AnsiHandler::new();
        let mut buf = make_canvas();
        handler.process_bytes(b"ABCDE\x1B[1;3H\x1B[K", &mut buf);
        assert_eq!(buf[0], b'A' as u32);
        assert_eq!(buf[1], b'B' as u32);
        assert_eq!(buf[2], 0);
        assert_eq!(buf[3], 0);
        assert_eq!(buf[4], 0);
    }

    #[test]
    fn test_ansi_clear_entire_line() {
        let mut handler = AnsiHandler::new();
        let mut buf = make_canvas();
        handler.process_bytes(b"ABCDE\x1B[1;3H\x1B[2K", &mut buf);
        for i in 0..CANVAS_COLS {
            assert_eq!(buf[i], 0, "buffer[{}] should be 0", i);
        }
    }

    #[test]
    fn test_ansi_tab() {
        let mut handler = AnsiHandler::new();
        let mut buf = make_canvas();
        handler.process_bytes(b"A\tB", &mut buf);
        let c = handler.cursor();
        assert_eq!(c.col, 9);
    }

    #[test]
    fn test_ansi_save_restore_cursor() {
        let mut handler = AnsiHandler::new();
        let mut buf = make_canvas();
        handler.process_bytes(b"Hello\n\x1B7World\n\x1B8Restored", &mut buf);
        assert_eq!(buf[CANVAS_COLS + 0], b'R' as u32);
        assert_eq!(buf[CANVAS_COLS + 1], b'e' as u32);
    }

    #[test]
    fn test_ansi_bell_ignored() {
        let mut handler = AnsiHandler::new();
        let mut buf = make_canvas();
        handler.process_bytes(b"Hi\x07!", &mut buf);
        assert_eq!(buf[0], b'H' as u32);
        assert_eq!(buf[1], b'i' as u32);
        assert_eq!(buf[2], b'!' as u32);
    }

    #[test]
    fn test_ansi_sgr_ignored() {
        let mut handler = AnsiHandler::new();
        let mut buf = make_canvas();
        handler.process_bytes(b"\x1B[1;31mRed\x1B[0mNormal", &mut buf);
        assert_eq!(buf[0], b'R' as u32);
        assert_eq!(buf[1], b'e' as u32);
        assert_eq!(buf[2], b'd' as u32);
        assert_eq!(buf[3], b'N' as u32);
        assert_eq!(buf[4], b'o' as u32);
        assert_eq!(buf[5], b'r' as u32);
        assert_eq!(buf[6], b'm' as u32);
        assert_eq!(buf[7], b'a' as u32);
        assert_eq!(buf[8], b'l' as u32);
    }

    #[test]
    fn test_ansi_cursor_up_default() {
        let mut handler = AnsiHandler::new();
        let mut buf = make_canvas();
        handler.process_bytes(b"\n\n\n\x1B[A", &mut buf);
        let c = handler.cursor();
        assert_eq!(c.row, 2);
    }

    #[test]
    fn test_ansi_cursor_up_multi() {
        let mut handler = AnsiHandler::new();
        let mut buf = make_canvas();
        handler.process_bytes(b"\n\n\n\n\x1B[3A", &mut buf);
        let c = handler.cursor();
        assert_eq!(c.row, 1);
    }

    #[test]
    fn test_ansi_unknown_sequence_ignored() {
        let mut handler = AnsiHandler::new();
        let mut buf = make_canvas();
        handler.process_bytes(b"\x1B[Xgarbage", &mut buf);
        assert_eq!(buf[0], b'g' as u32);
    }

    #[test]
    fn test_ansi_csi_private_cursor_hide() {
        let mut handler = AnsiHandler::new();
        let mut buf = make_canvas();
        handler.process_bytes(b"AB\x1B[?25lCD", &mut buf);
        assert_eq!(buf[0], b'A' as u32);
        assert_eq!(buf[1], b'B' as u32);
        assert_eq!(buf[2], b'C' as u32);
        assert_eq!(buf[3], b'D' as u32);
    }

    #[test]
    fn test_ansi_insert_delete_chars() {
        let mut handler = AnsiHandler::new();
        let mut buf = make_canvas();
        handler.process_bytes(b"ABCDE\x1B[1;3H\x1B[2@", &mut buf);
        assert_eq!(buf[0], b'A' as u32);
        assert_eq!(buf[1], b'B' as u32);
        assert_eq!(buf[2], 0);
        assert_eq!(buf[3], 0);
        assert_eq!(buf[4], b'C' as u32);
        assert_eq!(buf[5], b'D' as u32);
        assert_eq!(buf[6], b'E' as u32);
    }

    #[test]
    fn test_ansi_scroll_up() {
        let mut handler = AnsiHandler::new();
        let mut buf = make_canvas();
        handler.process_bytes(b"Row0\n", &mut buf);
        for _ in 0..CANVAS_MAX_ROWS {
            handler.process_bytes(b"X\n", &mut buf);
        }
        assert_ne!(buf[0], b'R' as u32);
    }

    #[test]
    fn test_ansi_line_wrap() {
        let mut handler = AnsiHandler::new();
        let mut buf = make_canvas();
        let data = vec![b'A'; CANVAS_COLS + 1];
        handler.process_bytes(&data, &mut buf);
        for i in 0..CANVAS_COLS {
            assert_eq!(buf[i], b'A' as u32);
        }
        assert_eq!(buf[CANVAS_COLS], b'A' as u32);
        let c = handler.cursor();
        assert_eq!(c.row, 1);
        assert_eq!(c.col, 1);
    }

    #[test]
    fn test_ansi_cursor_horizontal_absolute() {
        let mut handler = AnsiHandler::new();
        let mut buf = make_canvas();
        handler.process_bytes(b"ABCDE\x1B[3G", &mut buf);
        let c = handler.cursor();
        assert_eq!(c.col, 2); // ESC[3G = column 3 (1-based) = col 2 (0-based)
    }

    #[test]
    fn test_ansi_cursor_vertical_absolute() {
        let mut handler = AnsiHandler::new();
        let mut buf = make_canvas();
        handler.process_bytes(b"\x1B[5d", &mut buf);
        let c = handler.cursor();
        assert_eq!(c.row, 4);
    }

    // ── Cursor tests ─────────────────────────────────────────────

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

    // ── Integration tests ────────────────────────────────────────

    #[test]
    fn test_canvas_hello_world() {
        let mut handler = AnsiHandler::new();
        let mut buf = make_canvas();
        handler.process_bytes(b"Hello\nWorld", &mut buf);
        assert_eq!(buf[0], b'H' as u32);
        assert_eq!(buf[1], b'e' as u32);
        assert_eq!(buf[2], b'l' as u32);
        assert_eq!(buf[3], b'l' as u32);
        assert_eq!(buf[4], b'o' as u32);
        assert_eq!(buf[CANVAS_COLS], b'W' as u32);
        assert_eq!(buf[CANVAS_COLS + 1], b'o' as u32);
        assert_eq!(buf[CANVAS_COLS + 2], b'r' as u32);
        assert_eq!(buf[CANVAS_COLS + 3], b'l' as u32);
        assert_eq!(buf[CANVAS_COLS + 4], b'd' as u32);
    }

    #[test]
    fn test_canvas_linux_boot_sequence() {
        let mut handler = AnsiHandler::new();
        let mut buf = make_canvas();
        let boot = b"[    0.000000] Linux version 6.1.0\r\n\x1B[2J";
        handler.process_bytes(boot, &mut buf);
        let c = handler.cursor();
        assert_eq!(c.row, 0);
        assert_eq!(c.col, 0);
    }

    #[test]
    fn test_canvas_ansi_cursor_movement_text() {
        let mut handler = AnsiHandler::new();
        let mut buf = make_canvas();
        handler.process_bytes(b"\x1B[6;11Htest", &mut buf);
        assert_eq!(buf[5 * CANVAS_COLS + 10], b't' as u32);
        assert_eq!(buf[5 * CANVAS_COLS + 11], b'e' as u32);
        assert_eq!(buf[5 * CANVAS_COLS + 12], b's' as u32);
        assert_eq!(buf[5 * CANVAS_COLS + 13], b't' as u32);
    }

    #[test]
    fn test_canvas_ansi_mixed_sequences() {
        let mut handler = AnsiHandler::new();
        let mut buf = make_canvas();
        handler.process_bytes(b"\x1B[2;5Hmid\x1B[1;1Htop\x1B[3;1Hbot", &mut buf);
        assert_eq!(buf[0 * CANVAS_COLS + 0], b't' as u32);
        assert_eq!(buf[0 * CANVAS_COLS + 1], b'o' as u32);
        assert_eq!(buf[0 * CANVAS_COLS + 2], b'p' as u32);
        assert_eq!(buf[1 * CANVAS_COLS + 4], b'm' as u32);
        assert_eq!(buf[1 * CANVAS_COLS + 5], b'i' as u32);
        assert_eq!(buf[1 * CANVAS_COLS + 6], b'd' as u32);
        assert_eq!(buf[2 * CANVAS_COLS + 0], b'b' as u32);
        assert_eq!(buf[2 * CANVAS_COLS + 1], b'o' as u32);
        assert_eq!(buf[2 * CANVAS_COLS + 2], b't' as u32);
    }
}
