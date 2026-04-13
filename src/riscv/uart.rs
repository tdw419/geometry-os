// riscv/uart.rs -- UART 16550 serial port emulation (Phase 36)
//
// Emulates a minimal NS16550A UART at MMIO 0x1000_0000.
// Provides character I/O for guest OS console output.
//
// MMIO register map (byte offsets from base):
//   0x00  THR/RBR  Transmit/Receive buffer (write=read data)
//   0x01  IER      Interrupt Enable Register
//   0x02  IIR/FCR  Interrupt ID / FIFO Control
//   0x03  LCR      Line Control Register
//   0x04  MCR      Modem Control Register
//   0x05  LSR      Line Status Register
//   0x06  MSR      Modem Status Register
//   0x07  SCR      Scratch Register
//
// We only emulate what a guest OS actually needs:
//   - THR writes: characters sent by guest (collected into output buffer)
//   - RBR reads: characters received from host (keyboard input)
//   - LSR: THRE (transmit holding register empty) and DR (data ready) bits
//   - IER: interrupt enable for TX/RX
//   - LCR, FCR: accept writes, return stored values

/// UART MMIO base address (matches QEMU virt machine).
pub const UART_BASE: u64 = 0x1000_0000;
/// UART MMIO size (8 bytes of registers).
pub const UART_SIZE: u64 = 8;

/// Register offsets.
const THR_RBR: u64 = 0x00; // Transmit Holding / Receive Buffer
const IER: u64 = 0x01; // Interrupt Enable
const IIR_FCR: u64 = 0x02; // Interrupt ID / FIFO Control
const LCR: u64 = 0x03; // Line Control
const MCR: u64 = 0x04; // Modem Control
const LSR: u64 = 0x05; // Line Status
const MSR: u64 = 0x06; // Modem Status
const SCR: u64 = 0x07; // Scratch

/// LSR bits.
const LSR_DR: u8 = 1 << 0; // Data Ready
const LSR_THRE: u8 = 1 << 5; // THR Empty

/// IIR bits.
const IIR_NO_INTERRUPT: u8 = 0x01; // No interrupt pending

/// UART 16550 serial port.
#[derive(Clone, Debug)]
pub struct Uart {
    /// Interrupt Enable Register.
    pub ier: u8,
    /// Line Control Register.
    pub lcr: u8,
    /// Modem Control Register.
    pub mcr: u8,
    /// Line Status Register.
    pub lsr: u8,
    /// Modem Status Register.
    pub msr: u8,
    /// Scratch Register.
    pub scr: u8,
    /// FIFO Control Register (write-only).
    pub fcr: u8,
    /// Input buffer (characters received from host, e.g. keyboard).
    pub rx_buf: Vec<u8>,
    /// Output buffer (characters transmitted by guest).
    pub tx_buf: Vec<u8>,
}

impl Default for Uart {
    fn default() -> Self {
        Self::new()
    }
}

impl Uart {
    pub fn new() -> Self {
        Self {
            ier: 0,
            lcr: 0,
            mcr: 0,
            lsr: LSR_THRE, // THR is always empty (ready to transmit)
            msr: 0,
            scr: 0,
            fcr: 0,
            rx_buf: Vec::new(),
            tx_buf: Vec::new(),
        }
    }

    /// Check if the given address falls within UART MMIO range.
    pub fn contains(addr: u64) -> bool {
        addr >= UART_BASE && addr < UART_BASE + UART_SIZE
    }

    /// Read a byte from UART MMIO space.
    pub fn read_byte(&mut self, offset: u64) -> u8 {
        match offset {
            THR_RBR => {
                // Read from receive buffer (FIFO: first in, first out).
                if !self.rx_buf.is_empty() {
                    let b = self.rx_buf.remove(0);
                    if self.rx_buf.is_empty() {
                        self.lsr &= !LSR_DR; // Clear Data Ready
                    }
                    b
                } else {
                    0
                }
            }
            IER => self.ier,
            IIR_FCR => {
                // Return interrupt ID. We report no interrupt unless RX has data.
                if !self.rx_buf.is_empty() && (self.ier & 0x01) != 0 {
                    0x04 // Receive data available
                } else if self.lsr & LSR_THRE != 0 && (self.ier & 0x02) != 0 {
                    0x02 // THR empty
                } else {
                    IIR_NO_INTERRUPT
                }
            }
            LCR => self.lcr,
            MCR => self.mcr,
            LSR => self.lsr,
            MSR => self.msr,
            SCR => self.scr,
            _ => 0,
        }
    }

    /// Write a byte to UART MMIO space.
    pub fn write_byte(&mut self, offset: u64, val: u8) {
        match offset {
            THR_RBR => {
                // Transmit: add to output buffer.
                self.tx_buf.push(val);
                // In real hardware, THR would become not-empty, then empty
                // after "transmitting". We're instant, so THRE stays set.
            }
            IER => {
                self.ier = val & 0x0F; // Only lower 4 bits writable
            }
            IIR_FCR => {
                self.fcr = val;
                // FCR bit 1: clear RX FIFO
                if (val & 0x02) != 0 {
                    self.rx_buf.clear();
                    self.lsr &= !LSR_DR;
                }
                // FCR bit 2: clear TX FIFO
                if (val & 0x04) != 0 {
                    self.tx_buf.clear();
                }
            }
            LCR => {
                self.lcr = val;
            }
            MCR => {
                self.mcr = val;
            }
            LSR => {
                // LSR is read-only in real hardware, but some guests write to it.
            }
            MSR => {
                // Read-only.
            }
            SCR => {
                self.scr = val;
            }
            _ => {}
        }
    }

    /// Read a 32-bit word (little-endian) from UART MMIO space.
    /// Offset must be word-aligned relative to UART_BASE.
    pub fn read_word(&mut self, addr: u64) -> Option<u32> {
        let offset = addr - UART_BASE;
        let b0 = self.read_byte(offset) as u32;
        let b1 = self.read_byte(offset + 1) as u32;
        let b2 = self.read_byte(offset + 2) as u32;
        let b3 = self.read_byte(offset + 3) as u32;
        Some(b0 | (b1 << 8) | (b2 << 16) | (b3 << 24))
    }

    /// Write a 32-bit word (little-endian) to UART MMIO space.
    pub fn write_word(&mut self, addr: u64, val: u32) -> bool {
        let offset = addr - UART_BASE;
        self.write_byte(offset, (val & 0xFF) as u8);
        true
    }

    /// Enqueue a byte from the host (e.g., keyboard input).
    /// Sets the Data Ready bit in LSR.
    pub fn receive_byte(&mut self, b: u8) {
        self.rx_buf.push(b);
        self.lsr |= LSR_DR;
    }

    /// Drain all transmitted characters and return them.
    /// Clears the TX buffer.
    pub fn drain_tx(&mut self) -> Vec<u8> {
        let out = self.tx_buf.clone();
        self.tx_buf.clear();
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_uart_defaults() {
        let uart = Uart::new();
        assert_eq!(uart.ier, 0);
        assert_eq!(uart.lcr, 0);
        assert_eq!(uart.lsr, LSR_THRE);
        assert!(uart.rx_buf.is_empty());
        assert!(uart.tx_buf.is_empty());
    }

    #[test]
    fn transmit_stores_in_tx_buf() {
        let mut uart = Uart::new();
        uart.write_byte(THR_RBR, b'H');
        uart.write_byte(THR_RBR, b'i');
        assert_eq!(uart.tx_buf, vec![b'H', b'i']);
    }

    #[test]
    fn drain_tx_returns_and_clears() {
        let mut uart = Uart::new();
        uart.write_byte(THR_RBR, b'A');
        uart.write_byte(THR_RBR, b'B');
        let drained = uart.drain_tx();
        assert_eq!(drained, vec![b'A', b'B']);
        assert!(uart.tx_buf.is_empty());
    }

    #[test]
    fn receive_sets_data_ready() {
        let mut uart = Uart::new();
        assert_eq!(uart.lsr & LSR_DR, 0);
        uart.receive_byte(b'X');
        assert_ne!(uart.lsr & LSR_DR, 0);
    }

    #[test]
    fn read_rbr_returns_received_byte() {
        let mut uart = Uart::new();
        uart.receive_byte(b'Z');
        let b = uart.read_byte(THR_RBR);
        assert_eq!(b, b'Z');
        // After reading, DR should be cleared.
        assert_eq!(uart.lsr & LSR_DR, 0);
    }

    #[test]
    fn lsr_thre_always_set() {
        let mut uart = Uart::new();
        assert_ne!(uart.read_byte(LSR) & LSR_THRE, 0);
        uart.write_byte(THR_RBR, b'!');
        assert_ne!(uart.read_byte(LSR) & LSR_THRE, 0);
    }

    #[test]
    fn write_read_ier() {
        let mut uart = Uart::new();
        uart.write_byte(IER, 0x0F);
        assert_eq!(uart.read_byte(IER), 0x0F);
        uart.write_byte(IER, 0xFF);
        assert_eq!(uart.read_byte(IER), 0x0F); // masked to 4 bits
    }

    #[test]
    fn fcr_clears_fifos() {
        let mut uart = Uart::new();
        uart.write_byte(THR_RBR, b'X');
        uart.receive_byte(b'Y');
        // FCR bit 1 + 2: clear both FIFOs
        uart.write_byte(IIR_FCR, 0x06);
        assert!(uart.tx_buf.is_empty());
        assert!(uart.rx_buf.is_empty());
    }

    #[test]
    fn write_read_scratch() {
        let mut uart = Uart::new();
        uart.write_byte(SCR, 0x42);
        assert_eq!(uart.read_byte(SCR), 0x42);
    }

    #[test]
    fn write_read_lcr() {
        let mut uart = Uart::new();
        uart.write_byte(LCR, 0x03); // 8N1
        assert_eq!(uart.read_byte(LCR), 0x03);
    }

    #[test]
    fn read_word_little_endian() {
        let mut uart = Uart::new();
        uart.write_byte(IER, 0x01);
        uart.write_byte(IIR_FCR, 0x02);
        // Reading a word at UART_BASE reads THR_RBR, IER, IIR_FCR, LCR
        let word = uart.read_word(UART_BASE).unwrap();
        // Byte 0 = THR_RBR (0, no data), Byte 1 = IER (0x01), Byte 2 = IIR_FCR, Byte 3 = LCR
        assert_eq!((word >> 8) & 0xFF, 0x01); // IER value
    }
}
