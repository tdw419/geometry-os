use super::types::*;
use super::Vm;

impl Vm {
    /// Handle graphics and pixel opcodes (0x40-0x51).
    /// Returns false if halted (user-mode IKEY), true otherwise.
    #[allow(unreachable_patterns)]
    pub(super) fn step_graphics(&mut self, opcode: u32) -> bool {
        match opcode {
            // PSET x_reg, y_reg, color_reg  -- set pixel on screen
            0x40 => {
                let xr = self.fetch() as usize;
                let yr = self.fetch() as usize;
                let cr = self.fetch() as usize;
                if xr < NUM_REGS && yr < NUM_REGS && cr < NUM_REGS {
                    let x = self.regs[xr] as usize;
                    let y = self.regs[yr] as usize;
                    let color = self.regs[cr];
                    if x < 256 && y < 256 {
                        self.screen[y * 256 + x] = color;
                        if self.trace_recording {
                            self.pixel_write_log.push(
                                x as u16,
                                y as u16,
                                self.trace_buffer.step_counter().saturating_sub(1),
                                0x40,
                                color,
                            );
                        }
                    }
                }
            }

            // PSETI x, y, color  -- set pixel with immediate values
            0x41 => {
                let x = self.fetch() as usize;
                let y = self.fetch() as usize;
                let color = self.fetch();
                if x < 256 && y < 256 {
                    self.screen[y * 256 + x] = color;
                    if self.trace_recording {
                        self.pixel_write_log.push(
                            x as u16,
                            y as u16,
                            self.trace_buffer.step_counter().saturating_sub(1),
                            0x41,
                            color,
                        );
                    }
                }
            }

            // FILL color_reg  -- fill entire screen
            0x42 => {
                let cr = self.fetch() as usize;
                if cr < NUM_REGS {
                    let color = self.regs[cr];
                    for pixel in self.screen.iter_mut() {
                        *pixel = color;
                    }
                }
            }

            // RECTF x_reg, y_reg, w_reg, h_reg, color_reg  -- filled rectangle
            0x43 => {
                let xr = self.fetch() as usize;
                let yr = self.fetch() as usize;
                let wr = self.fetch() as usize;
                let hr = self.fetch() as usize;
                let cr = self.fetch() as usize;
                if xr < NUM_REGS && yr < NUM_REGS && wr < NUM_REGS && hr < NUM_REGS && cr < NUM_REGS
                {
                    let x0 = self.regs[xr] as usize;
                    let y0 = self.regs[yr] as usize;
                    let w = self.regs[wr] as usize;
                    let h = self.regs[hr] as usize;
                    let color = self.regs[cr];
                    for dy in 0..h {
                        for dx in 0..w {
                            let px = x0 + dx;
                            let py = y0 + dy;
                            if px < 256 && py < 256 {
                                self.screen[py * 256 + px] = color;
                            }
                        }
                    }
                }
            }

            // TEXT x_reg, y_reg, addr_reg  -- render text from RAM to screen
            0x44 => {
                let xr = self.fetch() as usize;
                let yr = self.fetch() as usize;
                let ar = self.fetch() as usize;
                if xr < NUM_REGS && yr < NUM_REGS && ar < NUM_REGS {
                    let mut sx = self.regs[xr] as usize;
                    let mut sy = self.regs[yr] as usize;
                    let mut addr = self.regs[ar] as usize;
                    let fg = 0xFFFFFF; // white text
                    loop {
                        if addr >= self.ram.len() {
                            break;
                        }
                        let ch = (self.ram[addr] & 0xFF) as u8;
                        if ch == 0 {
                            break;
                        }
                        if ch == b'\n' {
                            sx = self.regs[xr] as usize;
                            sy += 10;
                            addr += 1;
                            continue;
                        }
                        // Render 5x7 glyph at (sx, sy) -- inline for now
                        self.draw_char(ch, sx, sy, fg);
                        sx += 6; // 5 wide + 1 gap
                        if sx > 250 {
                            sx = self.regs[xr] as usize;
                            sy += 8;
                        }
                        addr += 1;
                    }
                }
            }

            // CMP rd, rs  -- set r0 = comparison result (-1, 0, 1)
            0x50 => {
                let rd = self.fetch() as usize;
                let rs = self.fetch() as usize;
                if rd < NUM_REGS && rs < NUM_REGS {
                    let a = self.regs[rd] as i32;
                    let b = self.regs[rs] as i32;
                    self.regs[0] = if a < b {
                        0xFFFFFFFF
                    } else if a > b {
                        1
                    } else {
                        0
                    };
                }
            }

            // MOV rd, rs -- rd = rs (register copy)
            0x51 => {
                let rd = self.fetch() as usize % NUM_REGS;
                let rs = self.fetch() as usize % NUM_REGS;
                self.regs[rd] = self.regs[rs];
            }

            // SPRITE x_reg, y_reg, addr_reg, w_reg, h_reg -- blit NxM pixels from RAM to screen
            // Color 0 in RAM means transparent (skip pixel)
            0x4A => {
                let xr = self.fetch() as usize;
                let yr = self.fetch() as usize;
                let ar = self.fetch() as usize;
                let wr = self.fetch() as usize;
                let hr = self.fetch() as usize;
                if xr < NUM_REGS && yr < NUM_REGS && ar < NUM_REGS && wr < NUM_REGS && hr < NUM_REGS
                {
                    let sx = self.regs[xr] as usize;
                    let sy = self.regs[yr] as usize;
                    let mut addr = self.regs[ar] as usize;
                    let w = self.regs[wr] as usize;
                    let h = self.regs[hr] as usize;
                    for dy in 0..h {
                        for dx in 0..w {
                            if addr >= self.ram.len() {
                                break;
                            }
                            let color = self.ram[addr];
                            addr += 1;
                            if color == 0 {
                                continue;
                            } // transparent
                            let px = sx + dx;
                            let py = sy + dy;
                            if px < 256 && py < 256 {
                                self.screen[py * 256 + px] = color;
                            }
                        }
                    }
                }
            }

            // RAND rd  -- rd = next pseudo-random u32 (LCG: state = state*1664525 + 1013904223)
            0x49 => {
                let rd = self.fetch() as usize;
                if rd < NUM_REGS {
                    self.rand_state = self
                        .rand_state
                        .wrapping_mul(1_664_525)
                        .wrapping_add(1_013_904_223);
                    self.regs[rd] = self.rand_state;
                }
            }

            // IKEY reg  -- read keyboard from ring buffer (or legacy RAM[0xFFF] port)
            0x48 => {
                let rd = self.fetch() as usize;
                // Blocked in User mode (hardware port access requires syscall)
                if self.mode == CpuMode::User {
                    self.halted = true;
                    return false;
                }
                if rd < NUM_REGS {
                    // Try ring buffer first
                    if self.key_buffer_head != self.key_buffer_tail {
                        self.regs[rd] = self.key_buffer[self.key_buffer_head];
                        self.key_buffer[self.key_buffer_head] = 0;
                        self.key_buffer_head = (self.key_buffer_head + 1) % self.key_buffer.len();
                    } else {
                        // Fallback to legacy single-key port
                        self.regs[rd] = self.ram[0xFFF];
                    }
                    self.ram[0xFFF] = 0;
                }
            }

            // LINE x0r, y0r, x1r, y1r, cr  -- Bresenham line
            0x45 => {
                let x0r = self.fetch() as usize;
                let y0r = self.fetch() as usize;
                let x1r = self.fetch() as usize;
                let y1r = self.fetch() as usize;
                let cr = self.fetch() as usize;
                if x0r < NUM_REGS
                    && y0r < NUM_REGS
                    && x1r < NUM_REGS
                    && y1r < NUM_REGS
                    && cr < NUM_REGS
                {
                    let color = self.regs[cr];
                    let mut x0 = self.regs[x0r] as i32;
                    let mut y0 = self.regs[y0r] as i32;
                    let x1 = self.regs[x1r] as i32;
                    let y1 = self.regs[y1r] as i32;
                    let dx = (x1 - x0).abs();
                    let dy = -(y1 - y0).abs();
                    let sx: i32 = if x0 < x1 { 1 } else { -1 };
                    let sy: i32 = if y0 < y1 { 1 } else { -1 };
                    let mut err = dx + dy;
                    loop {
                        if (0..256).contains(&x0) && (0..256).contains(&y0) {
                            self.screen[y0 as usize * 256 + x0 as usize] = color;
                        }
                        if x0 == x1 && y0 == y1 {
                            break;
                        }
                        let e2 = 2 * err;
                        if e2 >= dy {
                            err += dy;
                            x0 += sx;
                        }
                        if e2 <= dx {
                            err += dx;
                            y0 += sy;
                        }
                    }
                }
            }

            // CIRCLE xr, yr, rr, cr  -- midpoint circle
            0x46 => {
                let xr = self.fetch() as usize;
                let yr = self.fetch() as usize;
                let rr = self.fetch() as usize;
                let cr = self.fetch() as usize;
                if xr < NUM_REGS && yr < NUM_REGS && rr < NUM_REGS && cr < NUM_REGS {
                    let cx = self.regs[xr] as i32;
                    let cy = self.regs[yr] as i32;
                    let radius = self.regs[rr] as i32;
                    let color = self.regs[cr];
                    let mut x = radius;
                    let mut y = 0i32;
                    let mut err = 1 - radius;
                    while x >= y {
                        let pts: [(i32, i32); 8] = [
                            (cx + x, cy + y),
                            (cx - x, cy + y),
                            (cx + x, cy - y),
                            (cx - x, cy - y),
                            (cx + y, cy + x),
                            (cx - y, cy + x),
                            (cx + y, cy - x),
                            (cx - y, cy - x),
                        ];
                        for (px, py) in pts {
                            if (0..256).contains(&px) && (0..256).contains(&py) {
                                self.screen[py as usize * 256 + px as usize] = color;
                            }
                        }
                        y += 1;
                        if err < 0 {
                            err += 2 * y + 1;
                        } else {
                            x -= 1;
                            err += 2 * (y - x) + 1;
                        }
                    }
                }
            }

            // SCROLL nr  -- scroll screen up by regs[nr] pixels (wraps 0 in at bottom)
            0x47 => {
                let nr = self.fetch() as usize;
                if nr < NUM_REGS {
                    let n = (self.regs[nr] as usize).min(256);
                    if n > 0 {
                        self.screen.copy_within(n * 256.., 0);
                        for v in self.screen[(256 - n) * 256..].iter_mut() {
                            *v = 0;
                        }
                    }
                }
            }

            // ASM src_reg, dest_reg -- assemble source text from RAM, write bytecode to RAM
            // Source: null-terminated ASCII string at ram[regs[src_reg]]
            // Dest: bytecode written starting at ram[regs[dest_reg]]
            // Result: ram[0xFFD] = bytecode word count (success) or 0xFFFFFFFF (error)
            0x4B => {
                let sr = self.fetch() as usize;
                let dr = self.fetch() as usize;
                if sr < NUM_REGS && dr < NUM_REGS {
                    let src_addr = self.regs[sr] as usize;
                    let dest_addr = self.regs[dr] as usize;
                    // Read null-terminated ASCII string from RAM
                    let mut chars = Vec::new();
                    let mut a = src_addr;
                    while a < self.ram.len() {
                        let byte = (self.ram[a] & 0xFF) as u8;
                        if byte == 0 {
                            break;
                        }
                        chars.push(byte as char);
                        a += 1;
                    }
                    let source: String = chars.into_iter().collect();
                    // Call the assembler (base_addr = dest_addr for correct label resolution)
                    match crate::assembler::assemble(&source, dest_addr) {
                        Ok(result) => {
                            for (i, &word) in result.pixels.iter().enumerate() {
                                let idx = dest_addr + i;
                                if idx < self.ram.len() {
                                    self.ram[idx] = word;
                                }
                            }
                            self.ram[0xFFD] = result.pixels.len() as u32;
                        }
                        Err(_) => {
                            self.ram[0xFFD] = 0xFFFFFFFF;
                        }
                    }
                }
            }

            // TILEMAP xr, yr, mr, tr, gwr, ghr, twr, thr -- grid blit
            0x4C => {
                let xr = self.fetch() as usize;
                let yr = self.fetch() as usize;
                let mr = self.fetch() as usize;
                let tr = self.fetch() as usize;
                let gwr = self.fetch() as usize;
                let ghr = self.fetch() as usize;
                let twr = self.fetch() as usize;
                let thr = self.fetch() as usize;
                if xr < NUM_REGS
                    && yr < NUM_REGS
                    && mr < NUM_REGS
                    && tr < NUM_REGS
                    && gwr < NUM_REGS
                    && ghr < NUM_REGS
                    && twr < NUM_REGS
                    && thr < NUM_REGS
                {
                    let x0 = self.regs[xr] as i32;
                    let y0 = self.regs[yr] as i32;
                    let map_base = self.regs[mr] as usize;
                    let tiles_base = self.regs[tr] as usize;
                    let gw = self.regs[gwr] as usize;
                    let gh = self.regs[ghr] as usize;
                    let tw = self.regs[twr] as usize;
                    let th = self.regs[thr] as usize;

                    if tw > 0 && th > 0 {
                        for row in 0..gh {
                            for col in 0..gw {
                                let map_idx = row * gw + col;
                                let ram_map_addr = map_base + map_idx;
                                if ram_map_addr >= self.ram.len() {
                                    continue;
                                }

                                self.log_access(ram_map_addr, MemAccessKind::Read);
                                let tile_idx = self.ram[ram_map_addr] as usize;
                                if tile_idx == 0 {
                                    continue;
                                } // skip tile 0 (empty)

                                // Tile 1 is at offset 0, tile 2 at (tw*th), etc.
                                let tile_data_offset = (tile_idx - 1) * (tw * th);

                                for ty in 0..th {
                                    for tx in 0..tw {
                                        let pixel_idx = tile_data_offset + ty * tw + tx;
                                        let ram_pixel_addr = tiles_base + pixel_idx;
                                        if ram_pixel_addr >= self.ram.len() {
                                            continue;
                                        }

                                        self.log_access(ram_pixel_addr, MemAccessKind::Read);
                                        let color = self.ram[ram_pixel_addr];
                                        if color == 0 {
                                            continue;
                                        } // transparency

                                        let px = x0 + (col * tw) as i32 + tx as i32;
                                        let py = y0 + (row * th) as i32 + ty as i32;

                                        if (0..256).contains(&px) && (0..256).contains(&py) {
                                            self.screen[(py as usize) * 256 + (px as usize)] =
                                                color;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // SPAWN addr_reg  -- create child with isolated address space
            // Returns PID (1-based) in RAM[0xFFA], or 0xFFFFFFFF on error
            // Uses copy-on-write: shares parent's physical pages, copies on write.
            //
            // Mapping strategy:
            //  - If start_addr is in pages 0-2: identity-map pages 0-2 as COW.
            //    Virtual addr X == physical addr X, so .org label addresses resolve
            //    correctly for JMP/CALL.  Child PC = start_addr.
            //  - If start_addr is in page 3+: sequential mapping (legacy mode).
            //    vpage N -> physical page (start_page + N).  Only works for
            //    sequential code (no JMP to .org addresses).  Child PC = page_offset.
            0x4D => {
                let ar = self.fetch() as usize;
                if ar < NUM_REGS {
                    let active_count = self.processes.iter().filter(|p| !p.is_halted()).count();
                    if active_count >= MAX_PROCESSES {
                        self.ram[0xFFA] = 0xFFFFFFFF;
                    } else {
                        let start_addr = self.regs[ar];
                        let start_page = (start_addr as usize) / PAGE_SIZE;
                        let page_offset = start_addr % (PAGE_SIZE as u32);
                        let mut pd = vec![PAGE_UNMAPPED; NUM_PAGES];

                        // Determine child PC based on mapping strategy
                        let child_pc: u32;
                        let identity_map = start_page < 3;

                        if identity_map {
                            // Identity-map pages 0-2: virtual addr N == physical addr N
                            for (phys_page, pd_entry) in pd.iter_mut().enumerate().take(3) {
                                if phys_page >= NUM_RAM_PAGES {
                                    break;
                                }
                                *pd_entry = phys_page as u32;
                                if self.page_ref_count[phys_page] == 0 {
                                    self.page_ref_count[phys_page] = 1;
                                }
                                self.page_ref_count[phys_page] += 1;
                                self.page_cow |= 1u64 << phys_page;
                            }
                            child_pc = start_addr;
                        } else {
                            // Sequential mapping: vpage N -> phys page (start_page + N)
                            for (vpage, pd_entry) in pd.iter_mut().enumerate().take(PROCESS_PAGES) {
                                let parent_phys = start_page + vpage;
                                if parent_phys >= NUM_RAM_PAGES {
                                    break;
                                }
                                if vpage == 3 || parent_phys == 3 {
                                    *pd_entry = 3;
                                    self.page_ref_count[3] += 1;
                                    continue;
                                }
                                *pd_entry = parent_phys as u32;
                                self.page_ref_count[parent_phys] += 1;
                                self.page_cow |= 1u64 << parent_phys;
                            }
                            child_pc = page_offset;
                        }

                        // Page 3 (0xC00-0xFFF): shared region, identity-mapped, NOT COW
                        if !identity_map {
                            // Already handled in loop above for sequential mode
                            // but ensure it's set
                        }
                        // For identity_map mode, page 3 needs explicit setup since
                        // the loop only covers pages 0-2
                        if identity_map {
                            pd[3] = 3;
                            if self.page_ref_count[3] == 0 {
                                self.page_ref_count[3] = 1;
                            }
                            self.page_ref_count[3] += 1;
                        }

                        // Page 63 (hardware ports / syscall table) - always identity-mapped
                        pd[63] = 63;

                        let pid = (self.processes.len() + 1) as u32;
                        self.processes.push(SpawnedProcess {
                            pc: child_pc,
                            regs: [0; NUM_REGS],
                            state: ProcessState::Ready,
                            pid,
                            mode: CpuMode::User,
                            page_dir: Some(pd),
                            segfaulted: false,
                            priority: 1,
                            slice_remaining: 0,
                            sleep_until: 0,
                            yielded: false,
                            kernel_stack: Vec::new(),
                            msg_queue: Vec::new(),
                            exit_code: 0,
                            parent_pid: self.current_pid,
                            pending_signals: Vec::new(),
                            signal_handlers: [0; 4],
                            vmas: Process::default_vmas_for_process(),
                            brk_pos: PAGE_SIZE as u32,
                        });
                        self.ram[0xFFA] = pid;
                    }
                }
            }

            // KILL pid_reg  -- halt child, free its pages
            // Returns 1 in RAM[0xFFA] on success, 0 if not found
            0x4E => {
                let pr = self.fetch() as usize;
                if pr < NUM_REGS {
                    let target_pid = self.regs[pr];
                    let mut found = false;
                    let mut free_pd: Option<Vec<u32>> = None;
                    for proc in &mut self.processes {
                        if proc.pid == target_pid {
                            free_pd = proc.page_dir.take();
                            proc.state = ProcessState::Zombie;
                            found = true;
                            break;
                        }
                    }
                    if let Some(ref pd) = free_pd {
                        self.free_page_dir(pd);
                    }
                    self.ram[0xFFA] = if found { 1 } else { 0 };
                }
            }

            // PEEK rx, ry, rd  -- read screen pixel at (rx,ry) into rd
            // Out-of-bounds coordinates return 0
            0x4F => {
                let rx = self.fetch() as usize % NUM_REGS;
                let ry = self.fetch() as usize % NUM_REGS;
                let rd = self.fetch() as usize % NUM_REGS;
                let x = self.regs[rx] as usize;
                let y = self.regs[ry] as usize;
                if x < 256 && y < 256 {
                    self.regs[rd] = self.screen[y * 256 + x];
                } else {
                    self.regs[rd] = 0;
                }
            }
            _ => {}
        }
        true
    }
}
