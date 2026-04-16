// riscv/bridge.rs -- UART-to-canvas bridge (Phase 36)
//
// Connects the native RISC-V UART 16550 output to the canvas text surface.
// Reuses the AnsiHandler from Phase 33's QEMU bridge pattern:
//
//   Guest writes bytes to UART THR (MMIO 0x1000_0000)
//     -> UART stores in tx_buf
//     -> Bridge drains tx_buf each tick
//     -> Bytes fed through AnsiHandler
//     -> Printable chars written to canvas_buffer as u32
//
// Keyboard input flows the opposite direction:
//   Host keypress -> receive_byte() -> UART rx_buf -> guest reads RBR

use crate::qemu::{AnsiHandler, Cursor};

use super::bus::Bus;

/// Canvas dimensions (same as Phase 33).
const CANVAS_COLS: usize = 32;
const CANVAS_MAX_ROWS: usize = 128;

/// Bridge connecting the RISC-V UART to the canvas text surface.
///
/// After each step of the RISC-V VM, call `drain_uart_to_canvas()` to
/// flush any characters the guest wrote to the UART THR onto the canvas.
/// Call `forward_key()` to inject keyboard input into the guest UART.
pub struct UartBridge {
    /// ANSI escape handler (reused from Phase 33 QEMU bridge).
    ansi: AnsiHandler,
}

impl UartBridge {
    /// Create a new UART bridge.
    pub fn new() -> Self {
        Self {
            ansi: AnsiHandler::new(),
        }
    }

    /// Drain all pending UART TX output and render it on the canvas.
    ///
    /// Call this after running the VM for some number of steps.
    /// Returns the number of bytes drained.
    pub fn drain_uart_to_canvas(&mut self, bus: &mut Bus, canvas_buffer: &mut [u32]) -> usize {
        let bytes = bus.uart.drain_tx();
        if bytes.is_empty() {
            return 0;
        }
        self.ansi.process_bytes(&bytes, canvas_buffer);
        bytes.len()
    }

    /// Forward a keyboard byte to the guest UART.
    ///
    /// The byte is queued in the UART RX buffer. On the next guest
    /// read of the RBR register, it will be consumed.
    pub fn forward_key(&mut self, bus: &mut Bus, ascii_byte: u8) {
        bus.uart.receive_byte(ascii_byte);
    }

    /// Forward multiple keyboard bytes to the guest UART.
    pub fn forward_keys(&mut self, bus: &mut Bus, bytes: &[u8]) {
        for &b in bytes {
            bus.uart.receive_byte(b);
        }
    }

    /// Get the current cursor position on the canvas.
    pub fn cursor(&self) -> Cursor {
        self.ansi.cursor()
    }

    /// Set the cursor position directly.
    pub fn set_cursor(&mut self, row: usize, col: usize) {
        self.ansi.set_cursor(row, col);
    }

    /// Clear the canvas buffer.
    pub fn clear_screen(&self, canvas_buffer: &mut [u32]) {
        self.ansi.clear_screen(canvas_buffer);
    }

    /// Read a character from the canvas at the given position.
    /// Returns 0 if the position is out of bounds or empty.
    pub fn read_canvas_char(canvas_buffer: &[u32], row: usize, col: usize) -> u32 {
        if row >= CANVAS_MAX_ROWS || col >= CANVAS_COLS {
            return 0;
        }
        let idx = row * CANVAS_COLS + col;
        if idx < canvas_buffer.len() {
            canvas_buffer[idx]
        } else {
            0
        }
    }

    /// Extract a string from the canvas starting at (row, col) for `max_len` chars.
    /// Stops at row boundary or null characters.
    pub fn read_canvas_string(canvas_buffer: &[u32], row: usize, col: usize, max_len: usize) -> String {
        let mut s = String::new();
        let mut c = col;
        let mut r = row;
        for _ in 0..max_len {
            if r >= CANVAS_MAX_ROWS {
                break;
            }
            let ch = Self::read_canvas_char(canvas_buffer, r, c);
            if ch == 0 {
                break;
            }
            s.push(ch as u8 as char);
            c += 1;
            if c >= CANVAS_COLS {
                c = 0;
                r += 1;
            }
        }
        s
    }
}

impl Default for UartBridge {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::riscv::RiscvVm;

    fn make_canvas() -> Vec<u32> {
        vec![0u32; CANVAS_MAX_ROWS * CANVAS_COLS]
    }

    #[test]
    fn bridge_new_defaults() {
        let bridge = UartBridge::new();
        let cur = bridge.cursor();
        assert_eq!(cur.row, 0);
        assert_eq!(cur.col, 0);
    }

    #[test]
    fn drain_empty_uart_noop() {
        let mut vm = RiscvVm::new(4096);
        let mut bridge = UartBridge::new();
        let mut canvas = make_canvas();
        let n = bridge.drain_uart_to_canvas(&mut vm.bus, &mut canvas);
        assert_eq!(n, 0);
        assert!(canvas.iter().all(|&c| c == 0));
    }

    #[test]
    fn drain_renders_printable_chars() {
        let mut vm = RiscvVm::new(4096);
        let mut bridge = UartBridge::new();
        let mut canvas = make_canvas();

        // Guest writes "Hi" to UART THR
        vm.bus.uart.write_byte(0, b'H');
        vm.bus.uart.write_byte(0, b'i');

        let n = bridge.drain_uart_to_canvas(&mut vm.bus, &mut canvas);
        assert_eq!(n, 2);

        // Canvas should have 'H' at (0,0) and 'i' at (0,1)
        assert_eq!(canvas[0], b'H' as u32);
        assert_eq!(canvas[1], b'i' as u32);
    }

    #[test]
    fn drain_handles_newline() {
        let mut vm = RiscvVm::new(4096);
        let mut bridge = UartBridge::new();
        let mut canvas = make_canvas();

        vm.bus.uart.write_byte(0, b'A');
        vm.bus.uart.write_byte(0, b'\n');
        vm.bus.uart.write_byte(0, b'B');

        bridge.drain_uart_to_canvas(&mut vm.bus, &mut canvas);

        // 'A' at row 0 col 0, newline, 'B' at row 1 col 0
        assert_eq!(canvas[0], b'A' as u32);
        assert_eq!(canvas[CANVAS_COLS], b'B' as u32);
    }

    #[test]
    fn drain_handles_carriage_return() {
        let mut vm = RiscvVm::new(4096);
        let mut bridge = UartBridge::new();
        let mut canvas = make_canvas();

        vm.bus.uart.write_byte(0, b'X');
        vm.bus.uart.write_byte(0, b'Y');
        vm.bus.uart.write_byte(0, b'\r');
        vm.bus.uart.write_byte(0, b'Z');

        bridge.drain_uart_to_canvas(&mut vm.bus, &mut canvas);

        // After XY, CR goes to col 0, Z overwrites X at (0,0)
        assert_eq!(canvas[0], b'Z' as u32);
        assert_eq!(canvas[1], b'Y' as u32);
    }

    #[test]
    fn drain_handles_ansi_clear_screen() {
        let mut vm = RiscvVm::new(4096);
        let mut bridge = UartBridge::new();
        let mut canvas = make_canvas();

        // Write some text first
        vm.bus.uart.write_byte(0, b'H');
        vm.bus.uart.write_byte(0, b'i');
        bridge.drain_uart_to_canvas(&mut vm.bus, &mut canvas);
        assert_eq!(canvas[0], b'H' as u32);

        // ESC[2J = clear screen
        vm.bus.uart.write_byte(0, 0x1B);
        vm.bus.uart.write_byte(0, b'[');
        vm.bus.uart.write_byte(0, b'2');
        vm.bus.uart.write_byte(0, b'J');
        bridge.drain_uart_to_canvas(&mut vm.bus, &mut canvas);

        assert!(canvas.iter().all(|&c| c == 0));
    }

    #[test]
    fn forward_key_injects_into_uart() {
        let mut vm = RiscvVm::new(4096);
        let mut bridge = UartBridge::new();

        bridge.forward_key(&mut vm.bus, b'Q');

        // Guest reads RBR
        let b = vm.bus.uart.read_byte(0);
        assert_eq!(b, b'Q');
    }

    #[test]
    fn forward_keys_injects_multiple() {
        let mut vm = RiscvVm::new(4096);
        let mut bridge = UartBridge::new();

        bridge.forward_keys(&mut vm.bus, b"abc");

        assert_eq!(vm.bus.uart.read_byte(0), b'a');
        assert_eq!(vm.bus.uart.read_byte(0), b'b');
        assert_eq!(vm.bus.uart.read_byte(0), b'c');
    }

    #[test]
    fn read_canvas_char_bounds() {
        let canvas = make_canvas();
        assert_eq!(UartBridge::read_canvas_char(&canvas, 0, 0), 0);
        assert_eq!(UartBridge::read_canvas_char(&canvas, 200, 0), 0);
        assert_eq!(UartBridge::read_canvas_char(&canvas, 0, 50), 0);
    }

    #[test]
    fn read_canvas_string_extracts_text() {
        let mut canvas = make_canvas();
        canvas[0] = b'H' as u32;
        canvas[1] = b'e' as u32;
        canvas[2] = b'l' as u32;
        canvas[3] = b'l' as u32;
        canvas[4] = b'o' as u32;

        let s = UartBridge::read_canvas_string(&canvas, 0, 0, 10);
        assert_eq!(s, "Hello");
    }

    #[test]
    fn read_canvas_string_stops_at_null() {
        let mut canvas = make_canvas();
        canvas[0] = b'A' as u32;
        canvas[1] = b'B' as u32;
        // canvas[2] = 0 (null, default)

        let s = UartBridge::read_canvas_string(&canvas, 0, 0, 10);
        assert_eq!(s, "AB");
    }

    #[test]
    fn drain_clears_tx_buf() {
        let mut vm = RiscvVm::new(4096);
        let mut bridge = UartBridge::new();
        let mut canvas = make_canvas();

        vm.bus.uart.write_byte(0, b'!');
        bridge.drain_uart_to_canvas(&mut vm.bus, &mut canvas);

        // Second drain should return 0 (TX buf was cleared)
        let n = bridge.drain_uart_to_canvas(&mut vm.bus, &mut canvas);
        assert_eq!(n, 0);
    }

    #[test]
    fn full_roundtrip_guest_output_host_input() {
        let mut vm = RiscvVm::new(4096);
        let mut bridge = UartBridge::new();
        let mut canvas = make_canvas();

        // Guest writes a prompt
        for &b in b"echo> " {
            vm.bus.uart.write_byte(0, b);
        }
        bridge.drain_uart_to_canvas(&mut vm.bus, &mut canvas);

        // Verify prompt on canvas
        let prompt = UartBridge::read_canvas_string(&canvas, 0, 0, 6);
        assert_eq!(prompt, "echo> ");

        // Host types "hi"
        bridge.forward_keys(&mut vm.bus, b"hi\r");

        // Guest reads it back
        assert_eq!(vm.bus.uart.read_byte(0), b'h');
        assert_eq!(vm.bus.uart.read_byte(0), b'i');
        assert_eq!(vm.bus.uart.read_byte(0), b'\r');
    }

    #[test]
    fn guest_uart_via_cpu_store_renders_on_canvas() {
        // End-to-end: CPU executes a store to UART THR, bridge drains it to canvas.
        use crate::riscv::cpu::StepResult;

        let mut vm = RiscvVm::new(4096);
        let mut bridge = UartBridge::new();
        let mut canvas = make_canvas();

        let base = 0x8000_0000u64;

        // LUI x1, 0x10000     -> x1 = 0x1000_0000 (UART base)
        //   encoding: (0x10000 << 12) | (1 << 7) | 0x37 = 0x100000B7
        // ADDI x2, x0, 0x41   -> x2 = 'A'
        //   encoding: (0x41 << 20) | (0 << 15) | (0 << 12) | (2 << 7) | 0x13 = 0x04100113
        // SW x2, 0(x1)        -> store 'A' to UART THR
        //   encoding: 0x00208023
        // EBREAK
        vm.bus.write_word(base, 0x100000B7).expect("operation should succeed");
        vm.bus.write_word(base + 4, 0x04100113).expect("operation should succeed");
        vm.bus.write_word(base + 8, 0x00208023).expect("operation should succeed");
        vm.bus.write_word(base + 12, 0x00100073).expect("operation should succeed");

        vm.cpu.pc = base as u32;
        for _ in 0..10 {
            match vm.step() {
                StepResult::Ebreak => break,
                _ => {}
            }
        }

        // Bridge should drain the 'A'
        let n = bridge.drain_uart_to_canvas(&mut vm.bus, &mut canvas);
        assert_eq!(n, 1);
        assert_eq!(canvas[0], b'A' as u32);
    }
}
