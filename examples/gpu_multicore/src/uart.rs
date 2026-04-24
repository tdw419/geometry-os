//! UART Output Extraction
//!
//! Extracts and formats UART output from tile state buffers.

use crate::gpu::{STATE_HEADER_WORDS, UART_BUF_WORDS};

/// Extract UART output from a single tile's state buffer
pub fn extract_uart(tile_state: &[u32]) -> String {
    let uart_len = tile_state[37] as usize;
    let mut s = String::new();
    let uart_base = STATE_HEADER_WORDS;
    for i in 0..uart_len.min(UART_BUF_WORDS) {
        let byte = tile_state[uart_base + i] & 0xFF;
        if byte >= 0x20 && byte < 0x7F {
            s.push(byte as u8 as char);
        } else if byte != 0 {
            s.push_str(&format!("[{:02x}]", byte));
        }
    }
    s
}
