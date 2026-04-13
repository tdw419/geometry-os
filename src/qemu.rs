// qemu.rs -- Phase 33: QEMU Bridge
//
// Spawns QEMU as a subprocess, pipes serial console I/O through
// the Geometry OS canvas text surface. Supports ANSI escape sequences
// for proper terminal rendering.
//
// Usage from .asm: HYPERVISOR r0  (r0 = address of config string in RAM)
// Config: "arch=riscv64 kernel=linux.img ram=256M disk=rootfs.ext4"

use std::io::{Read, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

// ── Constants ────────────────────────────────────────────────────
const CANVAS_COLS: usize = 32;
const CANVAS_MAX_ROWS: usize = 128;
const QEMU_READ_BUF_SIZE: usize = 4096;

// ── Architecture mapping ─────────────────────────────────────────

/// Maps a config arch string to (qemu_binary, machine_flag).
/// Returns None for unknown architectures.
fn arch_to_qemu(arch: &str) -> Option<(&'static str, Option<&'static str>)> {
    match arch {
        "riscv64" => Some(("qemu-system-riscv64", Some("-machine virt"))),
        "riscv32" => Some(("qemu-system-riscv32", Some("-machine virt"))),
        "x86_64" => Some(("qemu-system-x86_64", None)),
        "aarch64" => Some(("qemu-system-aarch64", Some("-machine virt"))),
        "arm" => Some(("qemu-system-arm", Some("-machine virt"))),
        "mipsel" => Some(("qemu-system-mipsel", Some("-machine malta"))),
        "mips" => Some(("qemu-system-mips", Some("-machine malta"))),
        "ppc" => Some(("qemu-system-ppc", None)),
        "ppc64" => Some(("qemu-system-ppc64", None)),
        "s390x" => Some(("qemu-system-s390x", None)),
        _ => None,
    }
}

// ── QEMU Config ──────────────────────────────────────────────────

/// Parsed QEMU configuration from a config string.
/// Format: "arch=riscv64 kernel=linux.img ram=256M disk=rootfs.ext4"
#[derive(Debug, Clone, Default)]
pub struct QemuConfig {
    pub arch: String,
    pub kernel: Option<String>,
    pub ram: Option<String>,
    pub disk: Option<String>,
    pub bios: Option<String>,
    pub initrd: Option<String>,
    pub append: Option<String>,
    pub net: Option<String>,
    pub extra_args: Vec<String>,
}

impl QemuConfig {
    /// Parse a config string into a QemuConfig.
    /// Format: "key=value key=value ..."
    /// Unknown keys are stored in extra_args.
    pub fn parse(config_str: &str) -> Result<QemuConfig, String> {
        let mut cfg = QemuConfig::default();
        for token in config_str.split_whitespace() {
            let parts: Vec<&str> = token.splitn(2, '=').collect();
            if parts.len() != 2 || parts[1].is_empty() {
                return Err(format!(
                    "invalid config token: '{}' (expected key=value)",
                    token
                ));
            }
            let key = parts[0].to_lowercase();
            let val = parts[1].to_string();
            match key.as_str() {
                "arch" => cfg.arch = val,
                "kernel" => cfg.kernel = Some(val),
                "ram" | "memory" | "m" => cfg.ram = Some(val),
                "disk" | "drive" | "hda" => cfg.disk = Some(val),
                "bios" => cfg.bios = Some(val),
                "initrd" => cfg.initrd = Some(val),
                "append" | "cmdline" => cfg.append = Some(val),
                "net" | "nic" => cfg.net = Some(val),
                _ => cfg.extra_args.push(token.to_string()),
            }
        }
        if cfg.arch.is_empty() {
            return Err("config must specify arch=<architecture>".into());
        }
        Ok(cfg)
    }

    /// Build the QEMU command from this config.
    pub fn build_command(&self) -> Result<Command, String> {
        let (binary, machine) = arch_to_qemu(&self.arch)
            .ok_or_else(|| format!("unknown architecture: '{}'", self.arch))?;

        let mut cmd = Command::new(binary);
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // Always use nographic serial mode
        cmd.arg("-nographic");
        cmd.arg("-serial").arg("mon:stdio");

        // Machine type
        if let Some(m) = machine {
            let parts: Vec<&str> = m.split_whitespace().collect();
            for p in parts {
                cmd.arg(p);
            }
        }

        // RAM
        if let Some(ref ram) = self.ram {
            cmd.arg("-m").arg(ram);
        }

        // Kernel
        if let Some(ref kernel) = self.kernel {
            cmd.arg("-kernel").arg(kernel);
        }

        // BIOS
        if let Some(ref bios) = self.bios {
            cmd.arg("-bios").arg(bios);
        }

        // Initrd
        if let Some(ref initrd) = self.initrd {
            cmd.arg("-initrd").arg(initrd);
        }

        // Kernel command line
        if let Some(ref append) = self.append {
            cmd.arg("-append").arg(append);
        }

        // Disk
        if let Some(ref disk) = self.disk {
            cmd.args(&[
                "-drive",
                &format!("file={},format=raw,if=virtio", disk),
            ]);
        }

        // Network
        if let Some(ref net) = self.net {
            if net == "none" {
                cmd.arg("-net").arg("none");
            } else {
                cmd.args(&["-netdev", &format!("user,id=net0,{}", net)]);
                cmd.args(&["-device", "virtio-net-device,netdev=net0"]);
            }
        }

        // Extra args
        for arg in &self.extra_args {
            cmd.arg(arg);
        }

        Ok(cmd)
    }
}

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
pub struct Cursor {
    pub row: usize,
    pub col: usize,
}

impl Default for Cursor {
    fn default() -> Self {
        Cursor { row: 0, col: 0 }
    }
}

impl Cursor {
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

impl AnsiHandler {
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
                        if b >= 0x20 && b < 0x7F {
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
                        for i in 0..end {
                            canvas_buffer[i] = 0;
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
        for i in 0..end {
            canvas_buffer[i] = 0;
        }
    }
}

// ── QEMU Bridge ──────────────────────────────────────────────────

/// Manages a QEMU subprocess with piped stdin/stdout.
/// Reads QEMU output and writes it to a canvas buffer via the ANSI handler.
pub struct QemuBridge {
    child: Child,
    stdin: ChildStdin,
    stdout: ChildStdout,
    /// ANSI escape sequence handler.
    ansi: AnsiHandler,
    /// Whether the QEMU process is still running.
    alive: bool,
}

impl QemuBridge {
    /// Spawn a QEMU process from a config string.
    /// Config format: "arch=riscv64 kernel=linux.img ram=256M"
    pub fn spawn(config_str: &str) -> Result<QemuBridge, String> {
        let config = QemuConfig::parse(config_str)?;
        let mut cmd = config.build_command()?;

        let mut child = cmd.spawn().map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                let (binary, _) = arch_to_qemu(&config.arch)
                    .unwrap_or(("qemu-system-unknown", None));
                format!(
                    "QEMU not found: '{}'. Install with: sudo apt install qemu-system-{}",
                    binary, config.arch
                )
            } else {
                format!("failed to spawn QEMU: {}", e)
            }
        })?;

        let stdin = child.stdin.take().ok_or("failed to open QEMU stdin")?;
        let stdout = child.stdout.take().ok_or("failed to open QEMU stdout")?;

        Ok(QemuBridge {
            child,
            stdin,
            stdout,
            ansi: AnsiHandler::new(),
            alive: true,
        })
    }

    /// Read available output from QEMU stdout.
    /// Returns the number of bytes read.
    pub fn read_output(&mut self, canvas_buffer: &mut [u32]) -> usize {
        if !self.alive {
            return 0;
        }

        let mut tmp_buf = [0u8; 1024];
        let mut total_read = 0usize;

        loop {
            match self.stdout.read(&mut tmp_buf) {
                Ok(0) => {
                    self.alive = false;
                    break;
                }
                Ok(n) => {
                    total_read += n;
                    self.ansi.process_bytes(&tmp_buf[..n], canvas_buffer);
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    break;
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => {
                    continue;
                }
                Err(_) => {
                    self.alive = false;
                    break;
                }
            }
        }

        total_read
    }

    /// Write a byte to QEMU stdin.
    pub fn write_byte(&mut self, b: u8) -> std::io::Result<()> {
        if !self.alive {
            return Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "QEMU process is not running",
            ));
        }
        self.stdin.write_all(&[b])?;
        self.stdin.flush()?;
        Ok(())
    }

    /// Write a slice of bytes to QEMU stdin.
    pub fn write_bytes(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        if !self.alive {
            return Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "QEMU process is not running",
            ));
        }
        self.stdin.write_all(bytes)?;
        self.stdin.flush()?;
        Ok(())
    }

    /// Send a key press to QEMU stdin.
    pub fn send_key(&mut self, ascii_byte: u8) -> std::io::Result<()> {
        self.write_byte(ascii_byte)
    }

    /// Check if the QEMU process is still alive.
    pub fn is_alive(&mut self) -> bool {
        if !self.alive {
            return false;
        }
        match self.child.try_wait() {
            Ok(Some(_status)) => {
                self.alive = false;
                false
            }
            Ok(None) => true,
            Err(_) => {
                self.alive = false;
                false
            }
        }
    }

    /// Get the current cursor position.
    pub fn cursor(&self) -> Cursor {
        self.ansi.cursor()
    }

    /// Get the ANSI handler for direct access.
    pub fn ansi_handler(&self) -> &AnsiHandler {
        &self.ansi
    }

    /// Kill the QEMU process.
    pub fn kill(&mut self) -> std::io::Result<()> {
        self.child.kill()?;
        self.alive = false;
        Ok(())
    }
}

impl Drop for QemuBridge {
    fn drop(&mut self) {
        if self.alive {
            let _ = self.child.kill();
        }
    }
}

// ── Tests ────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_canvas() -> Vec<u32> {
        vec![0u32; CANVAS_MAX_ROWS * CANVAS_COLS]
    }

    // ── QemuConfig tests ─────────────────────────────────────────

    #[test]
    fn test_config_parse_minimal() {
        let cfg = QemuConfig::parse("arch=riscv64").unwrap();
        assert_eq!(cfg.arch, "riscv64");
        assert!(cfg.kernel.is_none());
        assert!(cfg.ram.is_none());
    }

    #[test]
    fn test_config_parse_full() {
        let cfg = QemuConfig::parse(
            "arch=x86_64 kernel=bzImage ram=512M disk=rootfs.ext4",
        )
        .unwrap();
        assert_eq!(cfg.arch, "x86_64");
        assert_eq!(cfg.kernel.as_deref(), Some("bzImage"));
        assert_eq!(cfg.ram.as_deref(), Some("512M"));
        assert_eq!(cfg.disk.as_deref(), Some("rootfs.ext4"));
    }

    #[test]
    fn test_config_parse_memory_alias() {
        let cfg = QemuConfig::parse("arch=aarch64 memory=1G").unwrap();
        assert_eq!(cfg.ram.as_deref(), Some("1G"));
    }

    #[test]
    fn test_config_parse_initrd_append() {
        let cfg = QemuConfig::parse(
            "arch=riscv64 kernel=Image initrd=initramfs.cpio.gz append=root=/dev/vda",
        )
        .unwrap();
        assert_eq!(cfg.initrd.as_deref(), Some("initramfs.cpio.gz"));
        assert_eq!(cfg.append.as_deref(), Some("root=/dev/vda"));
    }

    #[test]
    fn test_config_parse_no_arch() {
        let result = QemuConfig::parse("kernel=linux.img");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("arch"));
    }

    #[test]
    fn test_config_parse_invalid_token() {
        let result = QemuConfig::parse("arch=riscv64 nogoodvalue");
        assert!(result.is_err());
    }

    #[test]
    fn test_config_parse_extra_args() {
        let cfg = QemuConfig::parse("arch=riscv64 custom=foo").unwrap();
        assert_eq!(cfg.extra_args, vec!["custom=foo"]);
    }

    // ── Arch mapping tests ───────────────────────────────────────

    #[test]
    fn test_arch_mapping_riscv64() {
        let (bin, machine) = arch_to_qemu("riscv64").unwrap();
        assert_eq!(bin, "qemu-system-riscv64");
        assert_eq!(machine, Some("-machine virt"));
    }

    #[test]
    fn test_arch_mapping_x86_64() {
        let (bin, machine) = arch_to_qemu("x86_64").unwrap();
        assert_eq!(bin, "qemu-system-x86_64");
        assert!(machine.is_none());
    }

    #[test]
    fn test_arch_mapping_aarch64() {
        let (bin, machine) = arch_to_qemu("aarch64").unwrap();
        assert_eq!(bin, "qemu-system-aarch64");
        assert_eq!(machine, Some("-machine virt"));
    }

    #[test]
    fn test_arch_mapping_mipsel() {
        let (bin, machine) = arch_to_qemu("mipsel").unwrap();
        assert_eq!(bin, "qemu-system-mipsel");
        assert_eq!(machine, Some("-machine malta"));
    }

    #[test]
    fn test_arch_mapping_unknown() {
        assert!(arch_to_qemu("nonexistent").is_none());
    }

    // ── QemuConfig build_command tests ───────────────────────────

    #[test]
    fn test_build_command_riscv64() {
        let cfg =
            QemuConfig::parse("arch=riscv64 kernel=Image ram=256M").unwrap();
        let cmd = cfg.build_command().unwrap();
        let args: Vec<String> =
            cmd.get_args().map(|s| s.to_string_lossy().into()).collect();
        assert!(args.contains(&"-nographic".to_string()));
        assert!(args.contains(&"mon:stdio".to_string()));
        assert!(args.contains(&"-machine".to_string()));
        assert!(args.contains(&"virt".to_string()));
        assert!(args.contains(&"-m".to_string()));
        assert!(args.contains(&"256M".to_string()));
        assert!(args.contains(&"-kernel".to_string()));
        assert!(args.contains(&"Image".to_string()));
    }

    #[test]
    fn test_build_command_with_disk() {
        let cfg = QemuConfig::parse("arch=riscv64 disk=rootfs.ext4").unwrap();
        let cmd = cfg.build_command().unwrap();
        let args: Vec<String> =
            cmd.get_args().map(|s| s.to_string_lossy().into()).collect();
        let drive_arg = args.iter().find(|a| a.contains("rootfs.ext4")).unwrap();
        assert!(drive_arg.contains("format=raw"));
        assert!(drive_arg.contains("if=virtio"));
    }

    #[test]
    fn test_build_command_with_net_none() {
        let cfg = QemuConfig::parse("arch=riscv64 net=none").unwrap();
        let cmd = cfg.build_command().unwrap();
        let args: Vec<String> =
            cmd.get_args().map(|s| s.to_string_lossy().into()).collect();
        assert!(args.contains(&"none".to_string()));
    }

    #[test]
    fn test_build_command_unknown_arch() {
        let cfg = QemuConfig::parse("arch=invalid_cpu").unwrap();
        assert!(cfg.build_command().is_err());
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
        let mut data = vec![b'A'; CANVAS_COLS + 1];
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
