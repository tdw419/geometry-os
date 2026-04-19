/// Diagnostic: Count M-mode traps by type and watch for SBI calls.
/// Runs for 2M instructions (short) to capture trap statistics before panic loop.

use geometry_os::riscv::RiscvVm;
use std::time::Instant;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";

    let kernel = std::fs::read(kernel_path).expect("kernel not found");
    let initramfs = std::fs::read(initramfs_path).expect("initramfs not found");

    eprintln!("[trap-diag] Loading kernel ({} bytes) + initramfs ({} bytes)...",
        kernel.len(), initramfs.len());

    let (mut vm, fw_addr, _entry, _dtb_addr) = RiscvVm::boot_linux_setup(
        &kernel,
        Some(&initramfs),
        128,
        "console=ttyS0 earlycon=sbi panic=5 quiet nosmp",
    ).expect("boot setup failed");

    let fw_addr_u32 = fw_addr as u32;

    let mut count: u64 = 0;
    let max: u64 = 2_000_000;
    let mut trap_counts: [u64; 32] = [0; 32];
    let mut sbi_count: u64 = 0;
    let mut forward_count: u64 = 0;
    let mut ecall_m_count: u64 = 0;
    let mut mmode_trap_count: u64 = 0;
    let mut last_satap: u32 = vm.cpu.csr.satp;

    let start = Instant::now();

    while count < max {
        // Check for M-mode trap at our handler
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = (mcause & !(1u32 << 31)) as usize;
            let mpp = (vm.cpu.csr.mstatus >> 11) & 0x3;

            if cause_code < 32 {
                trap_counts[cause_code] += 1;
            }

            // Log first 20 traps of each type
            if cause_code < 32 && trap_counts[cause_code] <= 20 {
                eprintln!("[trap-diag] #{:3} cause={} mpp={} mepc=0x{:08X} stval=0x{:08X} satp=0x{:08X}",
                    trap_counts[cause_code], cause_code, mpp,
                    vm.cpu.csr.mepc, vm.cpu.csr.mtval, vm.cpu.csr.satp);
            }

            if cause_code == 9 { // ECALL_S = SBI call
                sbi_count += 1;
                if sbi_count <= 5 {
                    eprintln!("[trap-diag] SBI call: a7={} a6={} a0={} a1={}",
                        vm.cpu.x[17], vm.cpu.x[16], vm.cpu.x[10], vm.cpu.x[11]);
                }
            } else if cause_code == 11 { // ECALL_M
                ecall_m_count += 1;
            }

            if mpp == 3 {
                mmode_trap_count += 1;
            }

            // Forward to S-mode or handle
            if cause_code == 9 {
                // SBI - handle
                let _ = geometry_os::riscv::sbi::SbiHandler::handle_ecall_static(
                    vm.cpu.x[17], vm.cpu.x[16], vm.cpu.x[10],
                    vm.cpu.x[11], vm.cpu.x[12], vm.cpu.x[13],
                    vm.cpu.x[14], vm.cpu.x[15],
                    &mut vm.bus.uart, &mut vm.bus.clint,
                );
            }

            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }

        // SATP change detection
        let cur_satp = vm.cpu.csr.satap;
        if cur_satp != last_satap {
            eprintln!("[trap-diag] SATP changed: 0x{:08X} -> 0x{:08X} at count={}", last_satap, cur_satp, count);
            last_satap = cur_satp;
        }

        vm.bus.tick_clint();
        vm.bus.sync_mip(&mut vm.cpu.csr.mip);
        let _ = vm.step();
        count += 1;

        // Progress every 500K
        if count % 500_000 == 0 {
            eprintln!("[trap-diag] {}K: PC=0x{:08X} priv={:?} SBI={}",
                count / 1000, vm.cpu.pc, vm.cpu.privilege, sbi_count);
        }
    }

    let elapsed = start.elapsed();
    eprintln!("\n[trap-diag] === SUMMARY ({} instructions, {:.1}s) ===", count, elapsed.as_secs_f64());
    eprintln!("SBI calls: {}", sbi_count);
    eprintln!("ECALL_M:    {}", ecall_m_count);
    eprintln!("M-mode traps (from M-mode): {}", mmode_trap_count);
    eprintln!("Forwards:   {}", forward_count);
    eprintln!("Final PC:   0x{:08X}", vm.cpu.pc);
    eprintln!("Final priv: {:?}", vm.cpu.privilege);
    eprintln!("\nTrap counts by cause:");
    let names = ["misalign_fetch", "fetch_fault", "illegal_instr", "breakpoint",
        "misalign_load", "load_fault", "misalign_store", "store_fault",
        "ecall_u", "ecall_s", "reserved", "ecall_m",
        "fetch_page_fault", "load_page_fault", "(reserved)", "store_page_fault"];
    for i in 0..16 {
        if trap_counts[i] > 0 {
            let name = if i < names.len() { names[i] } else { "?" };
            eprintln!("  cause {} ({:20}): {}", i, name, trap_counts[i]);
        }
    }
    eprintln!("\nUART TX: {} bytes", vm.bus.uart.tx_count());
    eprintln!("UART buffer: {:?}", String::from_utf8_lossy(&vm.bus.uart.tx_buffer()));
}
