use super::Vm;
use super::types::*;

impl Vm {
    /// Handle memory opcodes (0x10-0x1F).
    /// Returns false if halted (segfault), true otherwise.
    #[allow(unreachable_patterns)]
    pub(super) fn step_memory(&mut self, opcode: u32) -> bool {
        match opcode {
                        // LDI reg, imm  -- load immediate
                        0x10 => {
                            let reg = self.fetch() as usize;
                            let imm = self.fetch();
                            if reg < NUM_REGS {
                                self.regs[reg] = imm;
                            }
                        }

                        // LOAD reg, addr_reg  -- load from RAM (page-translated)
                        0x11 => {
                            let reg = self.fetch() as usize;
                            let addr_reg = self.fetch() as usize;
                            if reg < NUM_REGS && addr_reg < NUM_REGS {
                                let vaddr = self.regs[addr_reg];
                                match self.translate_va_or_fault(vaddr) {
                                    Some(addr) => {
                                        // Phase 46: Intercept screen buffer range
                                        if (SCREEN_RAM_BASE..SCREEN_RAM_BASE + SCREEN_SIZE).contains(&addr) {
                                            self.regs[reg] = self.screen[addr - SCREEN_RAM_BASE];
                                            self.log_access(addr, MemAccessKind::Read);
                                        } else if addr < self.ram.len() {
                                            // Phase 45: Intercept canvas RAM range
                                            if (CANVAS_RAM_BASE..CANVAS_RAM_BASE + CANVAS_RAM_SIZE).contains(&addr) {
                                                self.regs[reg] = self.canvas_buffer[addr - CANVAS_RAM_BASE];
                                            } else {
                                                self.regs[reg] = self.ram[addr];
                                            }
                                            self.log_access(addr, MemAccessKind::Read);
                                        } else {
                                            self.trigger_segfault();
                                            return false;
                                        }
                                    }
                                    None => { self.trigger_segfault(); return false; }
                                }
                            }
                        }

                        // STORE addr_reg, reg  -- store to RAM (page-translated, COW-aware)
                        0x12 => {
                            let addr_reg = self.fetch() as usize;
                            let reg = self.fetch() as usize;
                            if addr_reg < NUM_REGS && reg < NUM_REGS {
                                let vaddr = self.regs[addr_reg];
                                // Check COW before writing: if the target physical page is shared,
                                // copy it to a private page first
                                self.resolve_cow_if_needed(vaddr);
                                match self.translate_va_or_fault(vaddr) {
                                    Some(addr) => {
                                        // Phase 46: Intercept screen buffer range
                                        if (SCREEN_RAM_BASE..SCREEN_RAM_BASE + SCREEN_SIZE).contains(&addr) {
                                            self.screen[addr - SCREEN_RAM_BASE] = self.regs[reg];
                                            self.log_access(addr, MemAccessKind::Write);
                                        } else if addr < self.ram.len() {
                                            if self.mode == CpuMode::User && addr >= 0xFF00 {
                                                self.trigger_segfault();
                                                return false;
                                            }
                                            // Phase 45: Intercept canvas RAM range
                                            if (CANVAS_RAM_BASE..CANVAS_RAM_BASE + CANVAS_RAM_SIZE).contains(&addr) {
                                                let cidx = addr - CANVAS_RAM_BASE;
                                                self.canvas_buffer[cidx] = self.regs[reg];
                                                // Phase 50: Trigger formula recalculation
                                                self.formula_recalc(cidx);
                                            } else {
                                                self.ram[addr] = self.regs[reg];
                                            }
                                            self.log_access(addr, MemAccessKind::Write);
                                        } else {
                                            self.trigger_segfault();
                                            return false;
                                        }
                                    }
                                    None => { self.trigger_segfault(); return false; }
                                }
                            }
                        }

                        // TEXTI x, y, "string" -- render inline text (no RAM setup needed)
                        // Encoding: 0x13, x_imm, y_imm, char_count, char1, char2, ...
                        0x13 => {
                            let x = self.fetch() as usize;
                            let y = self.fetch() as usize;
                            let count = self.fetch() as usize;
                            let mut sx = x;
                            let mut sy = y;
                            let fg = 0xFFFFFF; // white text
                            for _ in 0..count {
                                let ch = self.fetch();
                                if ch == 0 { continue; }
                                let byte = (ch & 0xFF) as u8;
                                if byte == b'\n' {
                                    sx = x;
                                    sy += 10;
                                    continue;
                                }
                                self.draw_char(byte, sx, sy, fg);
                                sx += 6;
                                if sx > 250 {
                                    sx = x;
                                    sy += 8;
                                }
                            }
                        }

                        // STRO addr_reg, "string" -- store inline string at address in register
                        // Encoding: 0x14, addr_reg, char_count, char1, char2, ...
                        0x14 => {
                            let ar = self.fetch() as usize;
                            let count = self.fetch() as usize;
                            if ar < NUM_REGS {
                                let mut vaddr = self.regs[ar];
                                for _ in 0..count {
                                    let ch = self.fetch();
                                    if let Some(addr) = self.translate_va_or_fault(vaddr) {
                                            if (SCREEN_RAM_BASE..SCREEN_RAM_BASE + SCREEN_SIZE).contains(&addr) {
                                                self.screen[addr - SCREEN_RAM_BASE] = ch;
                                                self.log_access(addr, MemAccessKind::Write);
                                            } else if addr < self.ram.len() {
                                            if (CANVAS_RAM_BASE..CANVAS_RAM_BASE + CANVAS_RAM_SIZE).contains(&addr) {
                                                let cidx = addr - CANVAS_RAM_BASE;
                                                self.canvas_buffer[cidx] = ch;
                                                self.formula_recalc(cidx);
                                            } else {
                                                self.ram[addr] = ch;
                                            }
                                            self.log_access(addr, MemAccessKind::Write);
                                        }
                                    }
                                vaddr = vaddr.wrapping_add(1);
                            }
                            // null-terminate if possible
                            if let Some(addr) = self.translate_va_or_fault(vaddr) {
                            if (SCREEN_RAM_BASE..SCREEN_RAM_BASE + SCREEN_SIZE).contains(&addr) {
                                self.screen[addr - SCREEN_RAM_BASE] = 0;
                            } else if addr < self.ram.len() {
                                if (CANVAS_RAM_BASE..CANVAS_RAM_BASE + CANVAS_RAM_SIZE).contains(&addr) {
                                    let cidx = addr - CANVAS_RAM_BASE;
                                    self.canvas_buffer[cidx] = 0;
                                    self.formula_recalc(cidx);
                                    } else {
                                        self.ram[addr] = 0;
                                    }
                                }
                            }
                            }
                        }

                        // CMPI reg, imm -- compare register against immediate, sets r0
                        0x15 => {
                            let rd = self.fetch() as usize;
                            let imm = self.fetch();
                            if rd < NUM_REGS {
                                let a = self.regs[rd] as i32;
                                let b = imm as i32;
                                self.regs[0] = if a < b { 0xFFFFFFFF } else if a > b { 1 } else { 0 };
                            }
                        }

                        // LOADS reg, offset -- load from SP + offset (stack-relative)
                        0x16 => {
                            let rd = self.fetch() as usize;
                            let offset = self.fetch() as i32 as usize;
                            if rd < NUM_REGS {
                                let sp = self.regs[30] as usize;
                                let vaddr = if offset < 0x80000000 { sp.wrapping_add(offset) } else { sp.wrapping_sub(0x100000000_usize - offset) };
                                match self.translate_va_or_fault(vaddr as u32) {
                                    Some(addr) => {
                                        if (SCREEN_RAM_BASE..SCREEN_RAM_BASE + SCREEN_SIZE).contains(&addr) {
                                            self.regs[rd] = self.screen[addr - SCREEN_RAM_BASE];
                                            self.log_access(addr, MemAccessKind::Read);
                                        } else if addr < self.ram.len() {
                                            if (CANVAS_RAM_BASE..CANVAS_RAM_BASE + CANVAS_RAM_SIZE).contains(&addr) {
                                                self.regs[rd] = self.canvas_buffer[addr - CANVAS_RAM_BASE];
                                            } else {
                                                self.regs[rd] = self.ram[addr];
                                            }
                                            self.log_access(addr, MemAccessKind::Read);
                                        } else {
                                            self.trigger_segfault(); return false;
                                        }
                                    }
                                    None => { self.trigger_segfault(); return false; }
                                }
                            }
                        }

                        // STORES offset, reg -- store to SP + offset (stack-relative, COW-aware)
                        0x17 => {
                            let offset = self.fetch() as i32;
                            let rs = self.fetch() as usize;
                            if rs < NUM_REGS {
                                let sp = self.regs[30] as i32;
                                let vaddr = sp.wrapping_add(offset) as u32;
                                self.resolve_cow_if_needed(vaddr);
                                match self.translate_va_or_fault(vaddr) {
                                    Some(addr) => {
                                        if (SCREEN_RAM_BASE..SCREEN_RAM_BASE + SCREEN_SIZE).contains(&addr) {
                                            self.screen[addr - SCREEN_RAM_BASE] = self.regs[rs];
                                            self.log_access(addr, MemAccessKind::Write);
                                        } else if addr < self.ram.len() {
                                            if self.mode == CpuMode::User && addr >= 0xFF00 {
                                                self.trigger_segfault();
                                                return false;
                                            }
                                            if (CANVAS_RAM_BASE..CANVAS_RAM_BASE + CANVAS_RAM_SIZE).contains(&addr) {
                                                let cidx = addr - CANVAS_RAM_BASE;
                                                self.canvas_buffer[cidx] = self.regs[rs];
                                                self.formula_recalc(cidx);
                                            } else {
                                                self.ram[addr] = self.regs[rs];
                                            }
                                            self.log_access(addr, MemAccessKind::Write);
                                        } else {
                                            self.trigger_segfault();
                                            return false;
                                        }
                                    }
                                    None => { self.trigger_segfault(); return false; }
                                }
                            }
                        }

                        // SHLI reg, imm -- shift left by immediate
                        0x18 => {
                            let rd = self.fetch() as usize;
                            let imm = self.fetch();
                            if rd < NUM_REGS {
                                self.regs[rd] <<= (imm % 32) as usize;
                            }
                        }

                        // SHRI reg, imm -- logical shift right by immediate
                        0x19 => {
                            let rd = self.fetch() as usize;
                            let imm = self.fetch();
                            if rd < NUM_REGS {
                                self.regs[rd] >>= (imm % 32) as usize;
                            }
                        }

                        // SARI reg, imm -- arithmetic shift right by immediate
                        0x1A => {
                            let rd = self.fetch() as usize;
                            let imm = self.fetch();
                            if rd < NUM_REGS {
                                let v = self.regs[rd] as i32;
                                self.regs[rd] = (v >> ((imm % 32) as usize)) as u32;
                            }
                        }

                        // ADDI reg, imm -- add immediate to register
                        0x1B => {
                            let rd = self.fetch() as usize;
                            let imm = self.fetch();
                            if rd < NUM_REGS {
                                self.regs[rd] = self.regs[rd].wrapping_add(imm);
                            }
                        }

                        // SUBI reg, imm -- subtract immediate from register
                        0x1C => {
                            let rd = self.fetch() as usize;
                            let imm = self.fetch();
                            if rd < NUM_REGS {
                                self.regs[rd] = self.regs[rd].wrapping_sub(imm);
                            }
                        }

                        // ANDI reg, imm -- bitwise AND with immediate
                        0x1D => {
                            let rd = self.fetch() as usize;
                            let imm = self.fetch();
                            if rd < NUM_REGS {
                                self.regs[rd] &= imm;
                            }
                        }

                        // ORI reg, imm -- bitwise OR with immediate
                        0x1E => {
                            let rd = self.fetch() as usize;
                            let imm = self.fetch();
                            if rd < NUM_REGS {
                                self.regs[rd] |= imm;
                            }
                        }

                        // XORI reg, imm -- bitwise XOR with immediate
                        0x1F => {
                            let rd = self.fetch() as usize;
                            let imm = self.fetch();
                            if rd < NUM_REGS {
                                self.regs[rd] ^= imm;
                            }
                        }
            _ => {}
        }
        true
    }
}
