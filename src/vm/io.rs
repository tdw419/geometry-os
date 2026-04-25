use super::trace::*;
use super::types::*;
use super::Vm;

impl Vm {
    /// Read a null-terminated string from RAM (one char per u32 word).
    pub(super) fn read_string_static(ram: &[u32], addr: usize) -> Option<String> {
        let mut s = String::new();
        let mut a = addr;
        while a < ram.len() {
            let ch = (ram[a] & 0xFF) as u8;
            if ch == 0 {
                return Some(s);
            }
            s.push(ch as char);
            a += 1;
        }
        if s.is_empty() {
            None
        } else {
            Some(s)
        }
    }

    /// Read a null-terminated string from self.ram with max length cap.
    pub(super) fn read_ram_string(&self, addr: usize, max_len: usize) -> Option<String> {
        let mut s = String::new();
        let mut a = addr;
        while a < self.ram.len() && s.len() < max_len {
            let ch = (self.ram[a] & 0xFF) as u8;
            if ch == 0 {
                break;
            }
            s.push(ch as char);
            a += 1;
        }
        if s.is_empty() {
            None
        } else {
            Some(s)
        }
    }
}

impl Vm {
    pub(super) fn fetch(&mut self) -> u32 {
        // Fast path: kernel mode (no page directory) uses identity mapping.
        // Avoids Option match + virtual address translation per instruction word.
        let phys = if self.current_page_dir.is_none() {
            let addr = self.pc as usize;
            if addr >= self.ram.len() {
                self.trigger_segfault();
                return 0;
            }
            addr
        } else {
            match self.translate_va(self.pc) {
                Some(addr) if addr < self.ram.len() => addr,
                _ => {
                    self.trigger_segfault();
                    return 0;
                }
            }
        };
        let val = self.ram[phys];
        self.pc += 1;
        val
    }

    /// Draw a character to the screen buffer (tiny 5x7 inline font for TEXT opcode)
    pub(super) fn draw_char(&mut self, ch: u8, x: usize, y: usize, color: u32) {
        self.draw_char_with_bg(ch, x, y, color, None);
    }

    /// Draw a character with optional background color
    pub(super) fn draw_char_with_bg(
        &mut self,
        ch: u8,
        x: usize,
        y: usize,
        fg: u32,
        bg: Option<u32>,
    ) {
        const MINI_FONT: [[u8; 7]; 96] = include!("../mini_font.in");
        let idx = ch as usize;
        if !(32..=127).contains(&idx) {
            return;
        }

        // Check if current process has a custom font (Phase 98)
        // Custom font overrides the built-in mini font with 8x8 glyphs
        let custom_font = if self.current_pid == 0 {
            self.processes.first().and_then(|p| p.custom_font.as_ref())
        } else {
            self.processes
                .iter()
                .find(|p| p.pid == self.current_pid)
                .and_then(|p| p.custom_font.as_ref())
        };

        if let Some(font) = custom_font {
            // Use custom 8x8 font
            let glyph = &font[idx];
            for (row, &glyph_row) in glyph.iter().enumerate().take(8usize) {
                for col in 0..8usize {
                    let px = x + col;
                    let py = y + row;
                    if px < 256 && py < 256 {
                        let on = glyph_row & (1 << (7 - col)) != 0;
                        if on {
                            self.screen[py * 256 + px] = fg;
                        } else if let Some(bg_color) = bg {
                            self.screen[py * 256 + px] = bg_color;
                        }
                    }
                }
            }
        } else {
            // Use built-in 5x7 mini font
            let glyph = &MINI_FONT[idx - 32];
            for (row, &glyph_row) in glyph.iter().enumerate().take(7usize) {
                for col in 0..5usize {
                    let px = x + col;
                    let py = y + row;
                    if px < 256 && py < 256 {
                        let on = glyph_row & (1 << (4 - col)) != 0;
                        if on {
                            self.screen[py * 256 + px] = fg;
                        } else if let Some(bg_color) = bg {
                            self.screen[py * 256 + px] = bg_color;
                        }
                    }
                }
            }
        }
    }

    /// Draw a character using the medium 5x7 font (for MEDTEXT opcode).
    /// Advance is 6 pixels (5 wide + 1 spacing), giving 42 columns in 256px.
    pub(super) fn draw_char_medium(
        &mut self,
        ch: u8,
        x: usize,
        y: usize,
        fg: u32,
        bg: Option<u32>,
    ) {
        const MED_FONT: [[u8; 7]; 96] = include!("../med_font.in");
        let idx = ch as usize;
        if !(32..=127).contains(&idx) {
            return;
        }
        let glyph = &MED_FONT[idx - 32];
        for (row, &glyph_row) in glyph.iter().enumerate().take(7usize) {
            for col in 0..5usize {
                let px = x + col;
                let py = y + row;
                if px < 256 && py < 256 {
                    let on = glyph_row & (1 << (4 - col)) != 0;
                    if on {
                        self.screen[py * 256 + px] = fg;
                    } else if let Some(bg_color) = bg {
                        self.screen[py * 256 + px] = bg_color;
                    }
                }
            }
        }
    }

    /// Draw a character using the tiny 3x5 font (for SMALLTEXT opcode).
    /// Advance is 3 pixels (no spacing), giving 85 columns in 256px.
    pub(super) fn draw_char_tiny(&mut self, ch: u8, x: usize, y: usize, fg: u32, bg: Option<u32>) {
        const TINY_FONT: [[u8; 5]; 96] = include!("../tiny_font.in");
        let idx = ch as usize;
        if !(32..=127).contains(&idx) {
            return;
        }
        let glyph = &TINY_FONT[idx - 32];
        for (row, &glyph_row) in glyph.iter().enumerate().take(5usize) {
            for col in 0..3usize {
                let px = x + col;
                let py = y + row;
                if px < 256 && py < 256 {
                    let on = glyph_row & (1 << (2 - col)) != 0;
                    if on {
                        self.screen[py * 256 + px] = fg;
                    } else if let Some(bg_color) = bg {
                        self.screen[py * 256 + px] = bg_color;
                    }
                }
            }
        }
    }

    /// Save VM state to a binary file.
    /// Format: GEOS magic (4) + version u32 (4) + halted u8 (1) + pc u32 (4)
    ///         + regs [u32; 32] (128) + ram [u32; RAM_SIZE] + screen [u32; SCREEN_SIZE]
    ///         + rand_state u32 (4) + frame_count u32 (4)   [version >= 2]
    #[cfg(not(target_arch = "wasm32"))]
    pub fn save_to_file(&self, path: &std::path::Path) -> std::io::Result<()> {
        use std::io::Write;
        let mut f = std::fs::File::create(path)?;
        f.write_all(SAVE_MAGIC)?;
        f.write_all(&SAVE_VERSION.to_le_bytes())?;
        f.write_all(&[if self.halted { 1 } else { 0 }])?;
        f.write_all(&self.pc.to_le_bytes())?;
        for &r in &self.regs {
            f.write_all(&r.to_le_bytes())?;
        }
        for &v in &self.ram {
            f.write_all(&v.to_le_bytes())?;
        }
        for &v in &self.screen {
            f.write_all(&v.to_le_bytes())?;
        }
        // v2 fields: persist RNG state and frame counter
        f.write_all(&self.rand_state.to_le_bytes())?;
        f.write_all(&self.frame_count.to_le_bytes())?;
        Ok(())
    }

    /// Load VM state from a binary file. Returns None if file doesn't exist
    /// or has invalid format.
    #[cfg(not(target_arch = "wasm32"))]
    #[allow(dead_code)]
    pub fn load_from_file(path: &std::path::Path) -> std::io::Result<Self> {
        use std::io::Read;
        let mut data = Vec::new();
        let mut f = std::fs::File::open(path)?;
        f.read_to_end(&mut data)?;

        // Minimum size: magic(4) + version(4) + halted(1) + pc(4) + regs(128) = 141
        let min_size = 4 + 4 + 1 + 4 + NUM_REGS * 4 + RAM_SIZE * 4 + SCREEN_SIZE * 4;
        if data.len() < min_size {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "save file too small: {} bytes (need {})",
                    data.len(),
                    min_size
                ),
            ));
        }
        if &data[0..4] != SAVE_MAGIC {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "invalid magic bytes",
            ));
        }
        let version = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
        // Accept v1 saves (missing rand_state/frame_count) and v2
        if !(1..=SAVE_VERSION).contains(&version) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "unsupported save version: {} (need 1-{})",
                    version, SAVE_VERSION
                ),
            ));
        }

        let mut offset = 8usize;
        let halted = data[offset] != 0;
        offset += 1;
        let pc = u32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]);
        offset += 4;

        let mut regs = [0u32; NUM_REGS];
        for r in regs.iter_mut() {
            *r = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            offset += 4;
        }

        let mut ram = vec![0u32; RAM_SIZE];
        for v in ram.iter_mut() {
            *v = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            offset += 4;
        }

        let mut screen = vec![0u32; SCREEN_SIZE];
        for v in screen.iter_mut() {
            *v = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            offset += 4;
        }

        // v2 fields: rand_state + frame_count (default if v1 save)
        let (rand_state, frame_count) = if version >= 2 && offset + 8 <= data.len() {
            let rs = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            let fc = u32::from_le_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);
            (rs, fc)
        } else {
            (0xDEADBEEF, 0) // v1 defaults
        };

        Ok(Vm {
            ram,
            regs,
            pc,
            screen,
            halted,
            frame_ready: false,
            rand_state,
            frame_count,
            beep: None,
            note: None,
            debug_mode: false,
            access_log: Vec::with_capacity(4096),
            processes: Vec::new(),
            mode: CpuMode::Kernel,
            kernel_stack: Vec::new(),
            allocated_pages: 0b11,
            page_ref_count: {
                let mut rc = [0u32; NUM_RAM_PAGES];
                rc[0] = 1;
                rc[1] = 1;
                rc
            },
            page_cow: 0,
            current_page_dir: None,
            current_vmas: Vec::new(),
            segfault_pid: 0,
            segfault: false,
            vfs: crate::vfs::Vfs::new(),
            inode_fs: crate::inode_fs::InodeFs::new(),
            current_pid: 0,
            current_capabilities: None,
            sched_tick: 0,
            default_time_slice: DEFAULT_TIME_SLICE,
            yielded: false,
            sleep_frames: 0,
            new_priority: 0,
            pipes: Vec::new(),
            canvas_buffer: vec![0; CANVAS_RAM_SIZE],
            pipe_created: false,
            msg_sender: 0,
            msg_data: [0; MSG_WORDS],
            msg_recv_requested: false,
            env_vars: std::collections::HashMap::new(),
            shutdown_requested: false,
            step_exit_code: None,
            step_zombie: false,
            booted: false,
            hypervisor_active: false,
            hypervisor_config: String::new(),
            hypervisor_mode: HypervisorMode::default(),
            hypervisor_window_id: 0,
            opcode_histogram: [0; 256],
            key_buffer: vec![0; 16],
            key_buffer_head: 0,
            key_buffer_tail: 0,
            key_port: 0,
            formulas: Vec::new(),
            formula_dep_index: std::collections::HashMap::new(),
            trace_recording: false,
            trace_buffer: TraceBuffer::new(DEFAULT_TRACE_CAPACITY),
            frame_checkpoints: FrameCheckBuffer::new(DEFAULT_FRAME_CHECK_CAPACITY),
            snapshots: Vec::new(),
            pixel_write_log: PixelWriteLog::new(DEFAULT_PIXEL_WRITE_CAPACITY),
            tcp_connections: (0..super::MAX_TCP_CONNECTIONS).map(|_| None).collect(),
            pty_slots: (0..super::ops_pty::MAX_PTY_SLOTS).map(|_| None).collect(),
            net_inbox: Vec::new(),
            hit_regions: Vec::with_capacity(super::MAX_HIT_REGIONS),
            mouse_x: 0,
            mouse_y: 0,
            mouse_button: 0,
            windows: Vec::with_capacity(super::MAX_WINDOWS),
            next_window_id: 1,
            llm_mock_response: None,
            llm_config: None,
            hermes_mock_response: None,
            hermes_session_id: None,
            background_vms: Vec::new(),
            next_bg_vm_id: 1,
            live_hypervisor: None,
            segfault_addr: 0,
            pc_trace: [0; 16],
            pc_trace_idx: 0,
            crash_dialog_active: false,
            crash_dialog_pid: 0,
            mailbox_write_buf: vec![
                super::types::MailboxEntry::default();
                super::types::MAILBOX_SIZE
            ],
            mailbox_read_buf: vec![
                super::types::MailboxEntry::default();
                super::types::MAILBOX_SIZE
            ],
        })
    }
}
