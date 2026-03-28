//! Executive Commander — The "Mayor" of the Infinite Map
//!
//! A minimal no_std RISC-V module that:
//! 1. Monitors a mailbox memory location for incoming commands
//! 2. Dispatches commands to other tile addresses
//! 3. Reports status via UART
//!
//! Memory layout:
//!   0x3000: Mailbox (command word)
//!   0x3004: Mailbox status (0 = idle, 1 = pending, 2 = ack)
//!   0x3008: Command target (tile ID)
//!   0x300C: Command payload
//!   0x3010: Tick counter
//!   0x3014: Last dispatched command
//!   0x3018-0x303F: Tile status array (8 tiles, 4 bytes each)
//!   0x4000: UART output (MMIO)
//!
//! Commands:
//!   0x01 = PING   — respond with ACK on UART
//!   0x02 = STATUS — print tick count and tile states
//!   0x03 = ASSIGN — set tile status[target] = payload
//!   0x04 = RESET  — zero all tile states
//!   0xFF = HALT   — stop execution

#![no_std]
#![no_main]

use core::panic::PanicInfo;

// Memory-mapped addresses
const MAILBOX_CMD: *mut u32 = 0x3000 as *mut u32;
const MAILBOX_STATUS: *mut u32 = 0x3004 as *mut u32;
const CMD_TARGET: *mut u32 = 0x3008 as *mut u32;
const CMD_PAYLOAD: *mut u32 = 0x300C as *mut u32;
const TICK_COUNTER: *mut u32 = 0x3010 as *mut u32;
const LAST_DISPATCH: *mut u32 = 0x3014 as *mut u32;
const TILE_STATUS_BASE: *mut u32 = 0x3018 as *mut u32;
const UART: *mut u8 = 0x4000 as *mut u8;

const NUM_TILES: u32 = 8;

// Commands
const CMD_PING: u32 = 0x01;
const CMD_STATUS: u32 = 0x02;
const CMD_ASSIGN: u32 = 0x03;
const CMD_RESET: u32 = 0x04;
const CMD_HALT: u32 = 0xFF;

// Mailbox status
const STATUS_IDLE: u32 = 0;
const STATUS_PENDING: u32 = 1;
const STATUS_ACK: u32 = 2;

#[inline(always)]
fn uart_write(byte: u8) {
    unsafe { core::ptr::write_volatile(UART, byte); }
}

fn uart_str(s: &[u8]) {
    for &b in s {
        uart_write(b);
    }
}

fn uart_hex(val: u32) {
    let digits = b"0123456789ABCDEF";
    // Print 8 hex digits
    for i in (0..8).rev() {
        let nibble = ((val >> (i * 4)) & 0xF) as usize;
        uart_write(digits[nibble]);
    }
}

fn uart_dec(mut val: u32) {
    if val == 0 {
        uart_write(b'0');
        return;
    }
    let mut buf = [0u8; 10];
    let mut i = 0;
    while val > 0 {
        buf[i] = b'0' + (val % 10) as u8;
        val /= 10;
        i += 1;
    }
    // Print in reverse
    while i > 0 {
        i -= 1;
        uart_write(buf[i]);
    }
}

fn uart_newline() {
    uart_write(b'\n');
}

#[inline(always)]
unsafe fn read_vol(addr: *mut u32) -> u32 {
    core::ptr::read_volatile(addr)
}

#[inline(always)]
unsafe fn write_vol(addr: *mut u32, val: u32) {
    core::ptr::write_volatile(addr, val);
}

fn tile_status_addr(idx: u32) -> *mut u32 {
    unsafe { TILE_STATUS_BASE.add(idx as usize) }
}

fn handle_ping() {
    uart_str(b"PONG\n");
}

fn handle_status() {
    unsafe {
        let ticks = read_vol(TICK_COUNTER);
        uart_str(b"TICK:");
        uart_dec(ticks);
        uart_newline();

        uart_str(b"TILES:");
        for i in 0..NUM_TILES {
            let status = read_vol(tile_status_addr(i));
            uart_write(b' ');
            uart_dec(status);
        }
        uart_newline();
    }
}

fn handle_assign() {
    unsafe {
        let target = read_vol(CMD_TARGET);
        let payload = read_vol(CMD_PAYLOAD);

        if target < NUM_TILES {
            write_vol(tile_status_addr(target), payload);
            uart_str(b"ASSIGN T");
            uart_dec(target);
            uart_str(b"=");
            uart_dec(payload);
            uart_newline();
        } else {
            uart_str(b"ERR:BAD_TILE\n");
        }

        write_vol(LAST_DISPATCH, CMD_ASSIGN);
    }
}

fn handle_reset() {
    unsafe {
        for i in 0..NUM_TILES {
            write_vol(tile_status_addr(i), 0);
        }
        uart_str(b"RESET OK\n");
    }
}

/// Main sovereignty loop — polls mailbox, dispatches commands
fn commander_loop() -> ! {
    unsafe {
        // Initialize
        write_vol(TICK_COUNTER, 0);
        write_vol(MAILBOX_STATUS, STATUS_IDLE);
        for i in 0..NUM_TILES {
            write_vol(tile_status_addr(i), 0);
        }

        // Boot message
        uart_str(b"EXEC_CMD v1\n");
        uart_str(b"TILES:");
        uart_dec(NUM_TILES);
        uart_newline();

        loop {
            // Increment tick
            let ticks = read_vol(TICK_COUNTER);
            write_vol(TICK_COUNTER, ticks.wrapping_add(1));

            // Check mailbox
            let status = read_vol(MAILBOX_STATUS);
            if status == STATUS_PENDING {
                let cmd = read_vol(MAILBOX_CMD);

                match cmd {
                    CMD_PING => handle_ping(),
                    CMD_STATUS => handle_status(),
                    CMD_ASSIGN => handle_assign(),
                    CMD_RESET => handle_reset(),
                    CMD_HALT => {
                        uart_str(b"HALT\n");
                        // ECALL to halt the RISC-V tile
                        core::arch::asm!("ecall");
                        // unreachable, but loop just in case
                        loop {}
                    }
                    _ => {
                        uart_str(b"ERR:UNK_CMD 0x");
                        uart_hex(cmd);
                        uart_newline();
                    }
                }

                // Acknowledge
                write_vol(MAILBOX_STATUS, STATUS_ACK);
            }

            // Yield — in a real multi-tile system this would be a WFI
            // For now, just spin. The GPU executes one instruction per dispatch,
            // so this loop naturally yields between frames.
        }
    }
}

#[no_mangle]
#[link_section = ".text.start"]
pub extern "C" fn _start() -> ! {
    // Set up stack pointer
    unsafe {
        core::arch::asm!(
            "la sp, _stack_top",
            options(nostack)
        );
    }
    commander_loop()
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    uart_str(b"PANIC!\n");
    unsafe { core::arch::asm!("ecall"); }
    loop {}
}
