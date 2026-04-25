// qemu/ansi.rs -- ANSI escape sequence handler for canvas text surface
//
// State machine that processes bytes from QEMU stdout (or RISC-V UART),
// interprets ANSI escape sequences, and writes printable characters into
// a canvas buffer (Vec<u32>).

use super::cursor::{Cursor, CANVAS_COLS, CANVAS_MAX_ROWS};

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
    /// Received ESC ], collecting OSC string (terminated by BEL or ST).
    Osc,
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

#[allow(dead_code)]
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
                                let idx = self.cursor.row * CANVAS_COLS + self.cursor.col;
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
            AnsiState::Escape => match b {
                b'[' => {
                    self.state = AnsiState::Csi;
                    self.csi_params.clear();
                }
                b']' => {
                    // OSC (Operating System Command) -- consume until BEL or ST
                    self.state = AnsiState::Osc;
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
            },
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
            AnsiState::Osc => {
                // Consume OSC string until BEL (0x07) or ST (ESC \)
                if b == 0x07 {
                    // BEL terminates OSC
                    self.state = AnsiState::Normal;
                } else if b == 0x1B {
                    // ESC inside OSC: transition to Escape state.
                    // Next byte will be '\' (ST terminator) or a new ESC sequence.
                    self.state = AnsiState::Escape;
                }
                // All other bytes inside OSC are consumed silently
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
                self.cursor.col = (params[0] as usize).saturating_sub(1).min(CANVAS_COLS - 1);
            }
            b'd' => {
                let params = self.parse_params(&[1]);
                self.cursor.row = (params[0] as usize)
                    .saturating_sub(1)
                    .min(CANVAS_MAX_ROWS - 1);
            }
            b'H' | b'f' => {
                let params = self.parse_params(&[1, 1]);
                self.cursor.row = (params[0] as usize)
                    .saturating_sub(1)
                    .min(CANVAS_MAX_ROWS - 1);
                self.cursor.col = (params[1] as usize).saturating_sub(1).min(CANVAS_COLS - 1);
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
                let params = self.parse_params(&[1, CANVAS_MAX_ROWS as u32]);
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
    fn handle_csi_private(&mut self, final_byte: u8, _canvas_buffer: &mut [u32]) {
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

    // ── Phase 130: Additional ANSI parser unit tests ────────────────

    #[test]
    fn test_ansi_osc_title_set_bel() {
        // OSC title: ESC ] 0 ; title BEL -- should not render anything
        let mut handler = AnsiHandler::new();
        let mut buf = make_canvas();
        handler.process_bytes(b"before\x1B]0;window title\x07after", &mut buf);
        assert_eq!(buf[0], b'b' as u32);
        assert_eq!(buf[1], b'e' as u32);
        assert_eq!(buf[2], b'f' as u32);
        assert_eq!(buf[3], b'o' as u32);
        assert_eq!(buf[4], b'r' as u32);
        assert_eq!(buf[5], b'e' as u32);
        assert_eq!(buf[6], b'a' as u32); // "after" starts right after "before"
        assert_eq!(buf[7], b'f' as u32);
        assert_eq!(buf[8], b't' as u32);
        assert_eq!(buf[9], b'e' as u32);
        assert_eq!(buf[10], b'r' as u32);
    }

    #[test]
    fn test_ansi_osc_title_set_st() {
        // OSC title: ESC ] 0 ; title ESC \ (String Terminator = ST)
        let mut handler = AnsiHandler::new();
        let mut buf = make_canvas();
        handler.process_bytes(b"before\x1B]2;my title\x1B\\after", &mut buf);
        assert_eq!(buf[0], b'b' as u32);
        assert_eq!(buf[5], b'e' as u32);
        assert_eq!(buf[6], b'a' as u32); // "after" immediately after "before"
    }

    #[test]
    fn test_ansi_partial_esc_sequence() {
        // ESC at end of input should not crash, just change state
        let mut handler = AnsiHandler::new();
        let mut buf = make_canvas();
        handler.process_bytes(b"AB\x1B", &mut buf);
        assert_eq!(buf[0], b'A' as u32);
        assert_eq!(buf[1], b'B' as u32);
        // State should be Escape -- cursor hasn't moved past the ESC
        let c = handler.cursor();
        assert_eq!(c.col, 2);
    }

    #[test]
    fn test_ansi_partial_csi_sequence() {
        // ESC [ at end of input should not crash
        let mut handler = AnsiHandler::new();
        let mut buf = make_canvas();
        handler.process_bytes(b"AB\x1B[", &mut buf);
        assert_eq!(buf[0], b'A' as u32);
        assert_eq!(buf[1], b'B' as u32);
        let c = handler.cursor();
        assert_eq!(c.col, 2);
    }

    #[test]
    fn test_ansi_partial_csi_params_only() {
        // ESC [ 3 ; (partial params, no final byte) should not crash
        let mut handler = AnsiHandler::new();
        let mut buf = make_canvas();
        handler.process_bytes(b"AB\x1B[3;", &mut buf);
        assert_eq!(buf[0], b'A' as u32);
        assert_eq!(buf[1], b'B' as u32);
    }

    #[test]
    fn test_ansi_invalid_esc_char() {
        // ESC followed by unknown char (not '[' or known single-char)
        let mut handler = AnsiHandler::new();
        let mut buf = make_canvas();
        handler.process_bytes(b"AB\x1BZtext", &mut buf);
        assert_eq!(buf[0], b'A' as u32);
        assert_eq!(buf[1], b'B' as u32);
        assert_eq!(buf[2], b't' as u32); // 'text' rendered after ESC Z is ignored
    }

    #[test]
    fn test_ansi_csi_cursor_up_at_row_zero() {
        // Cursor up at row 0 should stay at row 0
        let mut handler = AnsiHandler::new();
        let mut buf = make_canvas();
        handler.process_bytes(b"\x1B[5A", &mut buf);
        let c = handler.cursor();
        assert_eq!(c.row, 0);
        assert_eq!(c.col, 0);
    }

    #[test]
    fn test_ansi_csi_cursor_left_at_col_zero() {
        // Cursor left at col 0 should stay at 0
        let mut handler = AnsiHandler::new();
        let mut buf = make_canvas();
        handler.process_bytes(b"\x1B[10D", &mut buf);
        let c = handler.cursor();
        assert_eq!(c.col, 0);
    }

    #[test]
    fn test_ansi_csi_cursor_right_at_max_col() {
        // Cursor right should not exceed CANVAS_COLS-1
        let mut handler = AnsiHandler::new();
        let mut buf = make_canvas();
        // Move to near end, then try to go way past
        handler.set_cursor(0, CANVAS_COLS - 2);
        handler.process_bytes(b"\x1B[100C", &mut buf);
        let c = handler.cursor();
        assert_eq!(c.col, CANVAS_COLS - 1);
    }

    #[test]
    fn test_ansi_erase_in_line_to_start() {
        // ESC[1K clears from start of line to cursor
        let mut handler = AnsiHandler::new();
        let mut buf = make_canvas();
        handler.process_bytes(b"ABCDE\x1B[1;3H\x1B[1K", &mut buf);
        // Cols 0,1,2 should be cleared; cursor at row 0, col 2
        assert_eq!(buf[0], 0);
        assert_eq!(buf[1], 0);
        assert_eq!(buf[2], 0);
        assert_eq!(buf[3], b'D' as u32);
        assert_eq!(buf[4], b'E' as u32);
    }

    #[test]
    fn test_ansi_erase_from_cursor_to_screen_end() {
        // ESC[0J (default) clears from cursor to end
        let mut handler = AnsiHandler::new();
        let mut buf = make_canvas();
        handler.process_bytes(b"Row0\x1B[2;1HMore\x1B[1;3H\x1B[0J", &mut buf);
        // Row 0 cols 0-1 preserved, col 2+ cleared, row 1+ all cleared
        assert_eq!(buf[0], b'R' as u32);
        assert_eq!(buf[1], b'o' as u32);
        assert_eq!(buf[2], 0); // cleared from cursor
        assert_eq!(buf[3], 0);
        // Row 1 should be fully cleared
        assert_eq!(buf[CANVAS_COLS], 0);
    }

    #[test]
    fn test_ansi_scroll_region_set() {
        // ESC[5;10r sets scroll region
        let mut handler = AnsiHandler::new();
        let mut buf = make_canvas();
        handler.process_bytes(b"\x1B[5;10r", &mut buf);
        let c = handler.cursor();
        // After setting scroll region, cursor goes to scroll_top
        assert_eq!(c.row, 4); // 5-1 = row 4
        assert_eq!(c.col, 0);
    }

    #[test]
    fn test_ansi_delete_lines() {
        // ESC[1M deletes one line at cursor
        let mut handler = AnsiHandler::new();
        let mut buf = make_canvas();
        handler.process_bytes(b"Row0\nRow1\nRow2\x1B[2;1H\x1B[1M", &mut buf);
        // After deleting row 1 (cursor row), Row2 shifts up
        assert_eq!(buf[0 * CANVAS_COLS], b'R' as u32); // Row0 intact
        assert_eq!(buf[1 * CANVAS_COLS], b'R' as u32); // Row2 moved to row 1
    }

    #[test]
    fn test_ansi_insert_lines() {
        // ESC[1L inserts one blank line at cursor
        let mut handler = AnsiHandler::new();
        let mut buf = make_canvas();
        handler.process_bytes(b"Row0\nRow1\nRow2\x1B[2;1H\x1B[1L", &mut buf);
        // After inserting at row 1, Row1 shifts to row 2, Row2 to row 3
        assert_eq!(buf[0 * CANVAS_COLS], b'R' as u32); // Row0 intact
        assert_eq!(buf[1 * CANVAS_COLS], 0); // New blank line
        assert_eq!(buf[2 * CANVAS_COLS], b'R' as u32); // Was Row1
    }

    #[test]
    fn test_ansi_delete_chars() {
        // ESC[2P deletes 2 chars at cursor
        let mut handler = AnsiHandler::new();
        let mut buf = make_canvas();
        handler.process_bytes(b"ABCDE\x1B[1;2H\x1B[2P", &mut buf);
        assert_eq!(buf[0], b'A' as u32);
        assert_eq!(buf[1], b'D' as u32); // C and D shifted left over B,C
        assert_eq!(buf[2], b'E' as u32);
    }

    #[test]
    fn test_ansi_multiple_sgr_sequences() {
        // Multiple SGR sequences should all be ignored, text flows through
        let mut handler = AnsiHandler::new();
        let mut buf = make_canvas();
        handler.process_bytes(
            b"\x1B[1m\x1B[31m\x1B[4mBoldRedUnderline\x1B[0mNormal",
            &mut buf,
        );
        let expected = b"BoldRedUnderlineNormal";
        for (i, &byte) in expected.iter().enumerate() {
            assert_eq!(
                buf[i], byte as u32,
                "char at {} should be '{}'",
                i, byte as char
            );
        }
    }

    #[test]
    fn test_ansi_cursor_next_line() {
        // ESC[E moves cursor N lines down, column 0
        let mut handler = AnsiHandler::new();
        let mut buf = make_canvas();
        handler.process_bytes(b"Hello\x1B[3E", &mut buf);
        let c = handler.cursor();
        assert_eq!(c.row, 3);
        assert_eq!(c.col, 0);
    }

    #[test]
    fn test_ansi_cursor_prev_line() {
        // ESC[F moves cursor N lines up, column 0
        let mut handler = AnsiHandler::new();
        let mut buf = make_canvas();
        handler.process_bytes(b"\n\n\nHello\x1B[2F", &mut buf);
        let c = handler.cursor();
        assert_eq!(c.row, 1);
        assert_eq!(c.col, 0);
    }

    #[test]
    fn test_ansi_esc_save_restore_cursor_cs() {
        // ESC[s and ESC[u (CSI save/restore, not ESC 7/8)
        let mut handler = AnsiHandler::new();
        let mut buf = make_canvas();
        handler.process_bytes(b"Hello\x1B[s\nWorld\x1B[uX", &mut buf);
        // After ESC[s at col 5, then newline writes World on row 1,
        // ESC[u restores cursor to row 0 col 5, 'X' writes at row 0 col 5
        assert_eq!(buf[5], b'X' as u32);
    }

    #[test]
    fn test_ansi_scroll_up_command() {
        // ESC[1S scrolls entire screen up by 1 line
        let mut handler = AnsiHandler::new();
        let mut buf = make_canvas();
        handler.process_bytes(b"Row0\nRow1\nRow2\n\x1B[1S", &mut buf);
        // Row0 gone, Row1 at row 0, Row2 at row 1
        assert_eq!(buf[0 * CANVAS_COLS], b'R' as u32); // "Row1"
        assert_eq!(buf[0 * CANVAS_COLS + 1], b'o' as u32);
        assert_eq!(buf[1 * CANVAS_COLS], b'R' as u32); // "Row2"
    }

    #[test]
    fn test_ansi_consecutive_esc_sequences() {
        // Multiple back-to-back ESC sequences
        let mut handler = AnsiHandler::new();
        let mut buf = make_canvas();
        handler.process_bytes(b"\x1B[5;10H\x1B[1;31m\x1B[4mHi\x1B[0m", &mut buf);
        assert_eq!(buf[4 * CANVAS_COLS + 9], b'H' as u32);
        assert_eq!(buf[4 * CANVAS_COLS + 10], b'i' as u32);
    }

    #[test]
    fn test_ansi_csi_private_mode_set_reset() {
        // ESC[?25h (show cursor) and ESC[?25l (hide cursor) should be silently handled
        let mut handler = AnsiHandler::new();
        let mut buf = make_canvas();
        handler.process_bytes(b"A\x1B[?25lB\x1B[?25hC", &mut buf);
        assert_eq!(buf[0], b'A' as u32);
        assert_eq!(buf[1], b'B' as u32);
        assert_eq!(buf[2], b'C' as u32);
    }

    #[test]
    fn test_ansi_csi_private_erase_scrollback() {
        // ESC[?3J (erase scrollback) should be silently handled
        let mut handler = AnsiHandler::new();
        let mut buf = make_canvas();
        handler.process_bytes(b"Test\x1B[?3JMore", &mut buf);
        assert_eq!(buf[0], b'T' as u32);
        assert_eq!(buf[4], b'M' as u32);
    }

    #[test]
    fn test_ansi_carriage_return_newline_combo() {
        // \r\n (CR+LF) is the standard line ending from terminals
        let mut handler = AnsiHandler::new();
        let mut buf = make_canvas();
        handler.process_bytes(b"Line1\r\nLine2\r\nLine3", &mut buf);
        assert_eq!(buf[0 * CANVAS_COLS], b'L' as u32);
        assert_eq!(buf[1 * CANVAS_COLS], b'L' as u32);
        assert_eq!(buf[2 * CANVAS_COLS], b'L' as u32);
        let c = handler.cursor();
        assert_eq!(c.row, 2);
        assert_eq!(c.col, 5); // "Line3" = 5 chars
    }

    #[test]
    fn test_ansi_overwrite_with_cr() {
        // Classic overwriting pattern: write, CR, overwrite
        let mut handler = AnsiHandler::new();
        let mut buf = make_canvas();
        handler.process_bytes(b"XXXXX\rABC", &mut buf);
        assert_eq!(buf[0], b'A' as u32);
        assert_eq!(buf[1], b'B' as u32);
        assert_eq!(buf[2], b'C' as u32);
        assert_eq!(buf[3], b'X' as u32); // Not overwritten
        assert_eq!(buf[4], b'X' as u32);
    }

    #[test]
    fn test_ansi_real_ps1_prompt() {
        // Simulate a real bash PS1 prompt with ANSI color codes
        let mut handler = AnsiHandler::new();
        let mut buf = make_canvas();
        let prompt = b"\x1B[01;32muser@host\x1B[00m:\x1B[01;34m~\x1B[00m$ ";
        handler.process_bytes(prompt, &mut buf);
        // Should strip all SGR and render: "user@host:~$ "
        let expected = b"user@host:~$ ";
        for (i, &byte) in expected.iter().enumerate() {
            assert_eq!(
                buf[i], byte as u32,
                "char at {} should be '{}'",
                i, byte as char
            );
        }
    }

    #[test]
    fn test_ansi_full_screen_clear_and_rewrite() {
        // Simulate a full-screen app (like vim, top) clearing and rewriting
        let mut handler = AnsiHandler::new();
        let mut buf = make_canvas();
        // Draw some content
        handler.process_bytes(b"Old content", &mut buf);
        // Clear entire screen
        handler.process_bytes(b"\x1B[2J", &mut buf);
        // Move to top-left and draw new content
        handler.process_bytes(b"\x1B[1;1HNew content", &mut buf);
        assert_eq!(buf[0], b'N' as u32);
        assert_eq!(buf[1], b'e' as u32);
        assert_eq!(buf[2], b'w' as u32);
    }

    #[test]
    fn test_ansi_cursor_position_with_both_params() {
        // ESC[row;colH with explicit params
        let mut handler = AnsiHandler::new();
        let mut buf = make_canvas();
        handler.process_bytes(b"\x1B[10;20H", &mut buf);
        let c = handler.cursor();
        assert_eq!(c.row, 9); // 1-based to 0-based
        assert_eq!(c.col, 19);
    }

    #[test]
    fn test_ansi_cursor_position_with_row_only() {
        // ESC[row;H (col defaults to 1)
        let mut handler = AnsiHandler::new();
        let mut buf = make_canvas();
        handler.process_bytes(b"\x1B[5;H", &mut buf);
        let c = handler.cursor();
        assert_eq!(c.row, 4);
        assert_eq!(c.col, 0);
    }

    #[test]
    fn test_ansi_esc_reset() {
        // ESC c = reset terminal (full reset)
        // Feed content, then OSC, then ESC c to reset
        let mut handler = AnsiHandler::new();
        let mut buf = make_canvas();
        // Write "Some text" then OSC title then ESC c reset
        let input = b"Some text\x1B]0;title\x07";
        handler.process_bytes(input, &mut buf);
        // Now send ESC c (reset) separately to avoid \x1B confusion
        handler.process_bytes(&[0x1B, b'c'], &mut buf);
        let c = handler.cursor();
        assert_eq!(c.row, 0);
        assert_eq!(c.col, 0);
    }

    #[test]
    fn test_ansi_esc_index_down() {
        // ESC D = index (move cursor down one line, scroll if at bottom)
        let mut handler = AnsiHandler::new();
        let mut buf = make_canvas();
        handler.process_bytes(b"Hello\x1BD\x1BD", &mut buf);
        let c = handler.cursor();
        assert_eq!(c.row, 2);
    }

    #[test]
    fn test_ansi_esc_reverse_index() {
        // ESC M = reverse index (move cursor up one line, scroll if at top)
        let mut handler = AnsiHandler::new();
        let mut buf = make_canvas();
        handler.process_bytes(b"\n\nHello\x1BM", &mut buf);
        let c = handler.cursor();
        assert_eq!(c.row, 1); // Was at row 2, moved up to 1
    }

    #[test]
    fn test_ansi_tab_advances_to_next_8_boundary() {
        let mut handler = AnsiHandler::new();
        let mut buf = make_canvas();
        handler.process_bytes(b"A\tB", &mut buf);
        assert_eq!(buf[0], b'A' as u32);
        let c = handler.cursor();
        assert_eq!(c.col, 9); // Tab from col 1 to col 8, then 'B' at col 8 -> cursor at 9
        assert_eq!(buf[8], b'B' as u32);
    }

    #[test]
    fn test_ansi_tab_at_boundary() {
        // Tab at col 0 should advance to col 8
        let mut handler = AnsiHandler::new();
        let mut buf = make_canvas();
        handler.process_bytes(b"\tX", &mut buf);
        assert_eq!(buf[8], b'X' as u32);
    }

    #[test]
    fn test_ansi_empty_csi_defaults() {
        // ESC[H with no params defaults to row 1, col 1
        let mut handler = AnsiHandler::new();
        let mut buf = make_canvas();
        handler.process_bytes(b"\n\n\n\x1B[H", &mut buf);
        let c = handler.cursor();
        assert_eq!(c.row, 0);
        assert_eq!(c.col, 0);
    }

    #[test]
    fn test_ansi_insert_chars_shift() {
        // ESC[3@ inserts 3 blank chars at cursor, shifting right
        let mut handler = AnsiHandler::new();
        let mut buf = make_canvas();
        handler.process_bytes(b"ABCDE\x1B[1;2H\x1B[3@", &mut buf);
        assert_eq!(buf[0], b'A' as u32);
        assert_eq!(buf[1], 0); // 3 blanks inserted at col 1
        assert_eq!(buf[2], 0);
        assert_eq!(buf[3], 0);
        assert_eq!(buf[4], b'B' as u32); // shifted right by 3
    }

    #[test]
    fn test_ansi_multiple_scrolls() {
        // Fill more rows than buffer, verify oldest rows are gone
        let mut handler = AnsiHandler::new();
        let mut buf = make_canvas();
        for i in 0..=CANVAS_MAX_ROWS {
            let line = format!("Row{}\n", i);
            handler.process_bytes(line.as_bytes(), &mut buf);
        }
        // After scrolling, row 0 should NOT contain "Row0" (the first row scrolled off)
        // All rows start with 'R', so check the full "Row0" string
        let row0_text: Vec<u32> = (0..4).map(|c| buf[c]).collect();
        let row0_str: String = row0_text
            .iter()
            .map(|&v| {
                if v >= 0x20 && v < 0x7F {
                    v as u8 as char
                } else {
                    '?'
                }
            })
            .collect();
        assert_ne!(
            row0_str, "Row0",
            "Row0 should have scrolled off, but found '{}'",
            row0_str
        );
    }

    #[test]
    fn test_ansi_process_bytes_empty() {
        // Empty input should not crash
        let mut handler = AnsiHandler::new();
        let mut buf = make_canvas();
        handler.process_bytes(b"", &mut buf);
        let c = handler.cursor();
        assert_eq!(c.row, 0);
        assert_eq!(c.col, 0);
    }

    #[test]
    fn test_ansi_non_printable_low_chars() {
        // Chars 0x00-0x06 (except 0x07 bell, 0x08 BS, 0x09 TAB, 0x0A LF, 0x0D CR) ignored
        let mut handler = AnsiHandler::new();
        let mut buf = make_canvas();
        handler.process_bytes(b"A\x00\x01\x02\x03\x04\x05\x06B", &mut buf);
        assert_eq!(buf[0], b'A' as u32);
        assert_eq!(buf[1], b'B' as u32); // Control chars ignored, B at col 1
    }

    #[test]
    fn test_ansi_high_ascii_ignored() {
        // Bytes >= 0x7F (DEL and above) should not be rendered as printable
        let mut handler = AnsiHandler::new();
        let mut buf = make_canvas();
        handler.process_bytes(b"AB\x7F\x80\xFFCD", &mut buf);
        assert_eq!(buf[0], b'A' as u32);
        assert_eq!(buf[1], b'B' as u32);
        assert_eq!(buf[2], b'C' as u32); // 0x7F, 0x80, 0xFF not in 0x20..0x7F range
        assert_eq!(buf[3], b'D' as u32);
    }
}
