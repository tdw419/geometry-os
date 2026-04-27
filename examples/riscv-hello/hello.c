/*
 * hello.c -- Bare-metal C hello world for Geometry OS RISC-V hypervisor.
 *
 * Uses SBI (Supervisor Binary Interface) instead of raw UART MMIO:
 *   a7=1 (SBI_CONSOLE_PUTCHAR), a0=char  -> print one character
 *   a7=8 (SBI_SHUTDOWN)                  -> clean halt
 *
 * The Geometry OS SBI dispatcher (src/riscv/sbi.rs) intercepts the ecall,
 * routes the byte through the UART, and UartBridge renders it on the canvas.
 */

#include "libgeos.h"

void c_start(void) {
    geos_puts("hello from C\n");
    sbi_shutdown();
}
