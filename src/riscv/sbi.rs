// riscv/sbi.rs -- Minimal RISC-V SBI (Supervisor Binary Interface) implementation
//
// Handles SBI ECALLs from the kernel and provides HTIF-compatible
// memory-mapped I/O for early boot console writes.
//
// SBI Specification: https://github.com/riscv-software-src/riscv-sbi-doc
//
// Currently implements:
// - SBI v0.1 legacy console (putchar/getchar)
// - SBI v0.2 base (get_sbi_spec_version, get_sbi_impl_id, etc.)
// - SBI v0.2 hart state (hart_start, hart_stop)
// - SBI v0.2 system reset (shutdown, reboot)
// - SBI v0.2 timer (set_timer)
// - HTIF tohost/fromhost memory-mapped writes (silently accepted)
//
// COMPATIBILITY NOTE: We do NOT support SBI v0.2 extension 0x02 (console
// putchar). The extension ID 0x02 collides with legacy a7=2 (console getchar)
// at the same numeric value. Our bare-metal guest programs use legacy a7=1
// for putchar and a7=2 for getchar. Linux earlycon=sbi uses extension 0x02
// with a6=0 for putchar, which would be misrouted to getchar. If Linux boot
// support is ever needed, the SBI dispatch must be restructured to separate
// legacy (a7 < 0x10, no a6) from extension (a7 >= 0x10, uses a6) namespaces.

use super::uart::Uart;

/// SBI error codes (negative values returned in a0).
pub const SBI_SUCCESS: i32 = 0;
pub const SBI_ERR_FAILURE: i32 = -1;
pub const SBI_ERR_NOT_SUPPORTED: i32 = -2;
pub const SBI_ERR_INVALID_PARAM: i32 = -3;
pub const SBI_ERR_DENIED: i32 = -4;
pub const SBI_ERR_INVALID_ADDRESS: i32 = -5;
pub const SBI_ERR_ALREADY_AVAILABLE: i32 = -6;

// SBI v0.1 legacy extension IDs
const SBI_SET_TIMER: u32 = 0;
const SBI_CONSOLE_PUTCHAR: u32 = 1;
const SBI_CONSOLE_GETCHAR: u32 = 2;
const SBI_CLEAR_IPI: u32 = 3;
const SBI_SEND_IPI: u32 = 4;
const SBI_REMOTE_FENCE_I: u32 = 5;
const SBI_REMOTE_SFENCE_VMA: u32 = 6;
const SBI_REMOTE_SFENCE_VMA_ASID: u32 = 7;
const SBI_SHUTDOWN: u32 = 8;

// SBI v0.2 extension IDs
const SBI_EXT_BASE: u32 = 0x10;
const SBI_EXT_CONSOLE_PUTCHAR: u32 = 0x02;
const SBI_EXT_HART_STATE: u32 = 0x48534F; // "HSM"
const SBI_EXT_SYSTEM_RESET: u32 = 0x53525354; // "SRST"
const SBI_EXT_TIMER: u32 = 0x54494D45; // "TIME"
const SBI_EXT_IPI: u32 = 0x735049; // "sPI" (note: lowercase to avoid confusion)
const SBI_EXT_RFENCE: u32 = 0x52464E43; // "RFNC"
const SBI_EXT_DBCN: u32 = 0x4442434E; // "DBCN"
const SBI_DBCN_CONSOLE_WRITE: u32 = 0;
const SBI_DBCN_CONSOLE_READ: u32 = 1;
const SBI_DBCN_CONSOLE_WRITE_BYTE: u32 = 2;

// Geometry OS-specific SBI extension. EID is ASCII "GEO\0".
// Function 0: GEO_VFS_READ (DEPRECATED -- use Pixel VFS Surface at 0x7000_0000)
// Pixel access is via the MMIO framebuffer at 0x6000_0000 (256x256 RGBA).
const SBI_EXT_GEOMETRY: u32 = 0x47454F00; // "GEO\0"
const GEO_FN_VFS_READ: u32 = 0;

/// Pending GEO_VFS_READ request: set by handle_ecall, fulfilled by the caller
/// which has access to guest memory. The caller reads the filename bytes from
/// `name_addr..name_addr+name_len`, looks up the file in the host VFS, writes up
/// to `buf_len` bytes to `buf_addr`, and overwrites a0 with the byte count (or
/// SBI_ERR_INVALID_PARAM / SBI_ERR_FAILURE on error).
///
/// DEPRECATED: GEO_VFS_READ is replaced by the Pixel VFS Surface at 0x7000_0000.
/// This struct is retained for reference but the ecall now returns NOT_SUPPORTED.
#[derive(Debug, Clone, Copy)]
#[deprecated(note = "Use Pixel VFS Surface at 0x7000_0000 instead")]
pub struct GeoVfsReadReq {
    pub name_addr: u64,
    pub name_len: u32,
    pub buf_addr: u64,
    pub buf_len: u32,
}

/// SBI device: handles ECALL-based SBI calls and HTIF memory-mapped I/O.
pub struct Sbi {
    /// UART for console output (shared reference via write callback).
    /// We store characters to be read by the bus/bridge later.
    pub console_output: Vec<u8>,
    /// Whether the guest has requested shutdown.
    pub shutdown_requested: bool,
    /// Log of SBI ECALL arguments (a7, a6, a0) for debugging.
    #[allow(dead_code)]
    pub ecall_log: Vec<(u32, u32, u32)>,
    /// Pending DBCN console write: (physical_address, num_bytes).
    /// Set by handle_ecall when DBCN_CONSOLE_WRITE is called.
    /// The caller must read from bus memory and fulfill the request.
    pub dbcn_pending_write: Option<(u64, usize)>,
    /// Pending DBCN console read byte.
    /// Set by handle_ecall when DBCN_CONSOLE_READ has data.
    pub dbcn_pending_read: Option<u8>,
    /// Pending GEO_VFS_READ request. DEPRECATED -- no longer set by handle_ecall.
    /// Retained for struct compatibility. Will be removed in a future version.
    #[deprecated(note = "Use Pixel VFS Surface")]
    pub geo_vfs_read_pending: Option<GeoVfsReadReq>,
}

impl Sbi {
    /// Create a new SBI handler with empty console output.
    #[allow(deprecated)]
    pub fn new() -> Self {
        #[allow(deprecated)]
        Self {
            console_output: Vec::new(),
            shutdown_requested: false,
            ecall_log: Vec::new(),
            dbcn_pending_write: None,
            dbcn_pending_read: None,
            geo_vfs_read_pending: None,
        }
    }

    /// Handle an SBI ECALL.
    ///
    /// Called from the CPU when an ECALL from S-mode is detected.
    /// The CPU should check if a7 contains an SBI extension ID before
    /// delivering the trap normally.
    ///
    /// Arguments:
    /// - a7: SBI extension ID (function group)
    /// - a6: SBI function ID (within the extension)
    /// - a0..a5: function-specific arguments
    ///
    /// Returns (a0, a1) pair to set in registers after the ECALL.
    /// If this is NOT an SBI call, returns None and the CPU should
    /// handle the ECALL as a normal trap.
    #[allow(clippy::too_many_arguments)]
    pub fn handle_ecall(
        &mut self,
        a7: u32,
        a6: u32,
        a0: u32,
        _a1: u32,
        _a2: u32,
        _a3: u32,
        _a4: u32,
        _a5: u32,
        uart: &mut Uart,
        clint: &mut super::clint::Clint,
    ) -> Option<(u32, u32)> {
        // Log ECALL arguments for debugging (before they're modified)
        self.ecall_log.push((a7, a6, a0));
        match a7 {
            // SBI v0.1 legacy calls (extension ID is the function ID, a6=0)
            SBI_CONSOLE_PUTCHAR => {
                // a0 = character to print
                let ch = a0 as u8;
                if ch != 0 {
                    uart.write_byte(0, ch);
                    self.console_output.push(ch);
                }
                Some((SBI_SUCCESS as u32, 0))
            }
            SBI_CONSOLE_GETCHAR => {
                // Legacy SBI_CONSOLE_GETCHAR (a7=2): always read from UART rx_buf.
                // Note: SBI v0.2 extension 0x02 (console putchar) is handled separately
                // in the extension match block below. Legacy a7=2 is getchar only.
                if !uart.rx_buf.is_empty() {
                    let ch = uart.rx_buf.remove(0) as u32;
                    if uart.rx_buf.is_empty() {
                        uart.lsr &= !0x01; // Clear Data Ready
                    }
                    Some((ch, 0))
                } else {
                    Some((0xFFFFFFFF_u32, 0)) // -1 = no char available
                }
            }
            SBI_SET_TIMER => {
                // Set the timer: a0:a1 = 64-bit next timer event (absolute time)
                // a0 = low bits, a1 = high bits
                clint.mtimecmp = (_a1 as u64) << 32 | (a0 as u64);
                Some((SBI_SUCCESS as u32, 0))
            }
            SBI_CLEAR_IPI => Some((SBI_SUCCESS as u32, 0)),
            SBI_SEND_IPI => Some((SBI_SUCCESS as u32, 0)),
            SBI_REMOTE_FENCE_I => Some((SBI_SUCCESS as u32, 0)),
            SBI_REMOTE_SFENCE_VMA => Some((SBI_SUCCESS as u32, 0)),
            SBI_REMOTE_SFENCE_VMA_ASID => Some((SBI_SUCCESS as u32, 0)),
            SBI_SHUTDOWN => {
                self.shutdown_requested = true;
                Some((SBI_SUCCESS as u32, 0))
            }

            // SBI v0.2 extensions
            // IMPORTANT: SBI v0.2 return convention is (a0=error, a1=value).
            // a0=0 means success, a0<0 means error. The actual return value
            // goes in a1. The Linux kernel's __sbi_base_ecall wrapper checks
            // a0 for error and returns a1 on success.
            SBI_EXT_BASE => {
                match a6 {
                    // SBI_BASE_GET_SPEC_VERSION (0)
                    // Encoded as (major << 24 | minor). Report v2.0 so the kernel
                    // probes DBCN (Debug Console) extension for earlycon output.
                    // Note: major shift is 24 (not 16) per SBI spec v0.2+.
                    0 => Some((0, 0x02000000)), // a0=success, a1=version 2.0
                    // SBI_BASE_GET_IMPL_ID (1)
                    1 => Some((0, 0)), // a0=success, a1=implementation ID 0
                    // SBI_BASE_GET_IMPL_VERSION (2)
                    2 => Some((0, 1)), // a0=success, a1=implementation version 1
                    // SBI_BASE_PROBE_EXTENSION (3)
                    3 => {
                        // Probe if extension `a0` is available
                        // NOTE: SBI_EXT_GEOMETRY removed -- GEO_VFS_READ deprecated.
                        //       Use Pixel VFS Surface (0x7000_0000) instead.
                        let available = matches!(
                            a0,
                            SBI_EXT_BASE
                                | SBI_CONSOLE_PUTCHAR
                                | SBI_EXT_CONSOLE_PUTCHAR
                                | SBI_EXT_TIMER
                                | SBI_EXT_HART_STATE
                                | SBI_EXT_SYSTEM_RESET
                                | SBI_SET_TIMER
                                | SBI_SHUTDOWN
                                | SBI_EXT_RFENCE
                                | SBI_EXT_IPI
                                | SBI_EXT_DBCN
                                | SBI_EXT_GEOMETRY
                        );
                        Some((0, if available { 1 } else { 0 }))
                    }
                    // SBI_BASE_GET_MVENDORID (4)
                    4 => Some((0, 0)),
                    // SBI_BASE_GET_MARCHID (5)
                    5 => Some((0, 0x80000000)), // generic RISC-V
                    // SBI_BASE_GET_MIMPID (6)
                    6 => Some((0, 0)),
                    _ => Some((SBI_ERR_NOT_SUPPORTED as u32, 0)),
                }
            }
            // SBI_EXT_CONSOLE_PUTCHAR (0x02) is the same numeric value as
            // SBI_CONSOLE_GETCHAR (2). Both handled by the legacy arm above.
            // Linux earlycon=sbi would use a7=0x02,a6=0 for putchar, but our
            // guest programs use legacy a7=1 for putchar and a7=2 for getchar.
            SBI_EXT_TIMER => {
                // Timer extension: function 0 = sbi_set_timer
                // a0:a1 = 64-bit next timer event (absolute time)
                if a6 == 0 {
                    clint.mtimecmp = (_a1 as u64) << 32 | (a0 as u64);
                }
                Some((SBI_SUCCESS as u32, 0))
            }
            SBI_EXT_HART_STATE => {
                match a6 {
                    // hart_start (0) -- only allow hart 0 (ourselves)
                    0 => {
                        if a0 == 0 {
                            Some((SBI_SUCCESS as u32, 0))
                        } else {
                            // Reject secondary CPU start -- we only emulate 1 hart
                            Some((SBI_ERR_FAILURE as u32, 0))
                        }
                    }
                    // hart_stop (1)
                    1 => Some((SBI_SUCCESS as u32, 0)),
                    // hart_get_status (2)
                    2 => {
                        if a0 == 0 {
                            Some((0, 0)) // started
                        } else {
                            Some((SBI_ERR_FAILURE as u32, 0)) // not available
                        }
                    }
                    _ => Some((SBI_ERR_NOT_SUPPORTED as u32, 0)),
                }
            }
            SBI_EXT_SYSTEM_RESET => {
                match a6 {
                    // system_reset (0)
                    0 => {
                        self.shutdown_requested = true;
                        Some((SBI_SUCCESS as u32, 0))
                    }
                    _ => Some((SBI_ERR_NOT_SUPPORTED as u32, 0)),
                }
            }
            // RFENCE extension - remote fence operations (single-hart, NOP)
            SBI_EXT_RFENCE => Some((SBI_SUCCESS as u32, 0)), // single-hart, NOP
            // IPI extension - inter-processor interrupts (single-hart, NOP)
            SBI_EXT_IPI => Some((SBI_SUCCESS as u32, 0)),
            // DBCN extension - Debug Console (SBI v2.0)
            // On RV32: a0=num_bytes, a1=low32(phys_addr), a2=high32(phys_addr)
            // We store the request for the caller to fulfill via bus memory read.
            SBI_EXT_DBCN => {
                match a6 {
                    SBI_DBCN_CONSOLE_WRITE => {
                        // Store pending DBCN write request for caller to fulfill.
                        // Return success immediately -- the caller (step function)
                        // will read from guest memory and output to UART.
                        let num_bytes = a0 as usize;
                        let base_low = _a1 as u64;
                        let base_high = (_a2 as u64) << 32;
                        let phys_addr = base_high | base_low;
                        self.dbcn_pending_write = Some((phys_addr, num_bytes));
                        // Return success -- caller handles the actual write
                        Some((SBI_SUCCESS as u32, num_bytes as u32))
                    }
                    SBI_DBCN_CONSOLE_READ => {
                        // Read bytes from UART rx_buf.
                        // Since handle_ecall doesn't have &mut Bus, we use
                        // a simpler approach: return the first available byte in a0.
                        if !uart.rx_buf.is_empty() {
                            let b = uart.rx_buf.remove(0);
                            if uart.rx_buf.is_empty() {
                                uart.lsr &= !0x01; // Clear Data Ready
                            }
                            // Return (success, num_bytes_read=1, byte in a0 via pending)
                            self.dbcn_pending_read = Some(b);
                            Some((SBI_SUCCESS as u32, 1))
                        } else {
                            Some((SBI_ERR_FAILURE as u32, 0))
                        }
                    }
                    SBI_DBCN_CONSOLE_WRITE_BYTE => {
                        // Write single byte: a0 = byte value
                        let ch = a0 as u8;
                        if ch != 0 {
                            uart.write_byte(0, ch);
                            self.console_output.push(ch);
                        }
                        Some((SBI_SUCCESS as u32, 0))
                    }
                    _ => Some((SBI_ERR_NOT_SUPPORTED as u32, 0)),
                }
            }
            SBI_EXT_GEOMETRY => match a6 {
                GEO_FN_VFS_READ => {
                    // DEPRECATED: GEO_VFS_READ is replaced by the Pixel VFS Surface
                    // at 0x7000_0000. Guests should use lw/sw to read/write file data
                    // as pixels instead of using this ecall. See docs/design/pixel-vfs-surface.md.
                    eprintln!(
                        "WARNING: GEO_VFS_READ ecall is deprecated. \
                         Use Pixel VFS Surface at 0x7000_0000 instead."
                    );
                    Some((SBI_ERR_NOT_SUPPORTED as u32, 0))
                }
                _ => Some((SBI_ERR_NOT_SUPPORTED as u32, 0)),
            },
            _ => None, // Not an SBI call
        }
    }
}

impl Default for Sbi {
    #[allow(deprecated)]
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::super::clint::Clint;
    use super::*;

    #[test]
    fn test_sbi_console_putchar() {
        let mut sbi = Sbi::new();
        let mut uart = Uart::new();
        let mut clint = Clint::new();
        let result = sbi.handle_ecall(
            SBI_CONSOLE_PUTCHAR,
            0,
            b'A' as u32,
            0,
            0,
            0,
            0,
            0,
            &mut uart,
            &mut clint,
        );
        assert!(result.is_some());
        let (a0, a1) = result.expect("operation should succeed");
        assert_eq!(a0, SBI_SUCCESS as u32);
        assert_eq!(a1, 0);
        assert_eq!(sbi.console_output, vec![b'A']);
    }

    #[test]
    fn test_sbi_console_putchar_null() {
        let mut sbi = Sbi::new();
        let mut uart = Uart::new();
        let mut clint = Clint::new();
        sbi.handle_ecall(
            SBI_CONSOLE_PUTCHAR,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            &mut uart,
            &mut clint,
        );
        assert!(sbi.console_output.is_empty());
    }

    #[test]
    fn test_sbi_getchar() {
        let mut sbi = Sbi::new();
        let mut uart = Uart::new();
        let mut clint = Clint::new();
        // Legacy SBI_CONSOLE_GETCHAR uses a6!=0 (e.g., a6=1)
        let result = sbi.handle_ecall(
            SBI_CONSOLE_GETCHAR,
            1,
            0,
            0,
            0,
            0,
            0,
            0,
            &mut uart,
            &mut clint,
        );
        assert!(result.is_some());
        let (a0, _) = result.expect("operation should succeed");
        assert_eq!(a0, 0xFFFFFFFF); // -1 = no char
    }

    #[test]
    fn test_sbi_v02_console_putchar() {
        let mut sbi = Sbi::new();
        let mut uart = Uart::new();
        let mut clint = Clint::new();
        // Legacy SBI console putchar: a7=1, a0=char
        let result = sbi.handle_ecall(
            SBI_CONSOLE_PUTCHAR,
            0,
            b'X' as u32,
            0,
            0,
            0,
            0,
            0,
            &mut uart,
            &mut clint,
        );
        assert!(result.is_some());
        let (a0, _) = result.expect("operation should succeed");
        assert_eq!(a0, SBI_SUCCESS as u32);
        assert_eq!(sbi.console_output, vec![b'X']);
    }

    #[test]
    fn test_sbi_base_get_spec_version() {
        let mut sbi = Sbi::new();
        let mut uart = Uart::new();
        let mut clint = Clint::new();
        let result = sbi.handle_ecall(SBI_EXT_BASE, 0, 0, 0, 0, 0, 0, 0, &mut uart, &mut clint);
        assert_eq!(result, Some((0, 0x02000000))); // a0=success, a1=version 2.0
    }

    #[test]
    fn test_sbi_unknown_extension() {
        let mut sbi = Sbi::new();
        let mut uart = Uart::new();
        let mut clint = Clint::new();
        let result = sbi.handle_ecall(0x999, 0, 0, 0, 0, 0, 0, 0, &mut uart, &mut clint);
        assert!(result.is_none());
    }

    #[test]
    fn test_sbi_shutdown() {
        let mut sbi = Sbi::new();
        let mut uart = Uart::new();
        let mut clint = Clint::new();
        sbi.handle_ecall(SBI_SHUTDOWN, 0, 0, 0, 0, 0, 0, 0, &mut uart, &mut clint);
        assert!(sbi.shutdown_requested);
    }

    #[test]
    fn test_sbi_system_reset() {
        let mut sbi = Sbi::new();
        let mut uart = Uart::new();
        let mut clint = Clint::new();
        sbi.handle_ecall(
            SBI_EXT_SYSTEM_RESET,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            &mut uart,
            &mut clint,
        );
        assert!(sbi.shutdown_requested);
    }

    #[test]
    fn test_sbi_base_probe_extension() {
        let mut sbi = Sbi::new();
        let mut uart = Uart::new();
        let mut clint = Clint::new();

        // Probe known extension
        let result = sbi.handle_ecall(
            SBI_EXT_BASE,
            3,
            SBI_EXT_CONSOLE_PUTCHAR,
            0,
            0,
            0,
            0,
            0,
            &mut uart,
            &mut clint,
        );
        assert_eq!(result, Some((0, 1))); // a0=success, a1=available

        // Probe unknown extension
        let result = sbi.handle_ecall(SBI_EXT_BASE, 3, 0x999, 0, 0, 0, 0, 0, &mut uart, &mut clint);
        assert_eq!(result, Some((0, 0))); // a0=success, a1=not available
    }

    #[test]
    fn test_sbi_set_timer_64bit() {
        let mut sbi = Sbi::new();
        let mut uart = Uart::new();
        let mut clint = Clint::new();
        // Test with 64-bit value: a0=low bits, a1=high bits
        let result = sbi.handle_ecall(
            SBI_SET_TIMER,
            0,
            0xDEAD,
            0xBEEF,
            0,
            0,
            0,
            0,
            &mut uart,
            &mut clint,
        );
        assert_eq!(result, Some((SBI_SUCCESS as u32, 0)));
        assert_eq!(clint.mtimecmp, 0xBEEF0000DEAD);
    }

    #[test]
    fn test_sbi_set_timer_v02_ext_64bit() {
        let mut sbi = Sbi::new();
        let mut uart = Uart::new();
        let mut clint = Clint::new();
        // SBI v0.2 timer extension: a0=low, a1=high
        let result = sbi.handle_ecall(
            SBI_EXT_TIMER,
            0,
            0xCAFE,
            0xF00D,
            0,
            0,
            0,
            0,
            &mut uart,
            &mut clint,
        );
        assert_eq!(result, Some((SBI_SUCCESS as u32, 0)));
        assert_eq!(clint.mtimecmp, 0xF00D0000CAFE);
    }

    #[test]
    fn test_sbi_hart_state_start_primary() {
        let mut sbi = Sbi::new();
        let mut uart = Uart::new();
        let mut clint = Clint::new();
        // hart 0 (our CPU) should succeed
        let r = sbi.handle_ecall(
            SBI_EXT_HART_STATE,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            &mut uart,
            &mut clint,
        );
        assert_eq!(r, Some((SBI_SUCCESS as u32, 0)));
        // hart 1 (secondary) should fail -- we only emulate 1 hart
        let r = sbi.handle_ecall(
            SBI_EXT_HART_STATE,
            0,
            1,
            0,
            0,
            0,
            0,
            0,
            &mut uart,
            &mut clint,
        );
        assert_eq!(r, Some((SBI_ERR_FAILURE as u32, 0)));
    }

    #[test]
    fn test_sbi_hart_state_get_status() {
        let mut sbi = Sbi::new();
        let mut uart = Uart::new();
        let mut clint = Clint::new();
        // hart 0 should report started
        let r = sbi.handle_ecall(
            SBI_EXT_HART_STATE,
            2,
            0,
            0,
            0,
            0,
            0,
            0,
            &mut uart,
            &mut clint,
        );
        assert_eq!(r, Some((0, 0)));
        // hart 1 should report not available
        let r = sbi.handle_ecall(
            SBI_EXT_HART_STATE,
            2,
            1,
            0,
            0,
            0,
            0,
            0,
            &mut uart,
            &mut clint,
        );
        assert_eq!(r, Some((SBI_ERR_FAILURE as u32, 0)));
    }
}
