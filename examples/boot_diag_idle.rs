// Diagnostic: Boot Linux and watch what happens after 50M instructions.
// The kernel reaches arch_cpu_idle but we need to understand WHY it stays
// in the idle loop instead of completing kernel_init.

use geometry_os::riscv::csr;
use geometry_os::riscv::{
    cpu::{self, StepResult},
    RiscvVm,
};

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";

    let kernel_image = match std::fs::read(kernel_path) {
        Ok(d) => d,
        Err(_) => {
            eprintln!("No kernel");
            return;
        }
    };
    let initramfs = std::fs::read(initramfs_path).ok();

    let bootargs = "console=ttyS0 earlycon=sbi panic=5 loglevel=7";
    let (mut vm, fw_addr, _entry, _dtb_addr) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 512, bootargs)
            .expect("setup");

    let fw_addr_u32 = fw_addr as u32;
    let mut count: u64 = 0;
    let max = 100_000_000;

    // Track unique PCs in the last 1M instructions
    let mut pc_hist: std::collections::HashMap<u32, u64> = std::collections::HashMap::new();
    let mut last_timer_fire: u64 = 0;
    let mut timer_fires: u64 = 0;
    let mut last_sbi_call_count: usize = 0;

    while count < max {
        if vm.bus.sbi.shutdown_requested {
            break;
        }

        // Timer advancement
        vm.bus.tick_clint_n(100);
        vm.bus.sync_mip(&mut vm.cpu.csr.mip);
        vm.bus.current_pc = vm.cpu.pc;

        let step_result = vm.step();

        // Track when CPU reaches fw_addr (M-mode trap handler)
        if vm.cpu.pc == fw_addr_u32 {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            let is_interrupt = (mcause >> 31) & 1 == 1;
            let mpp = (vm.cpu.csr.mstatus >> csr::MSTATUS_MPP_LSB) & 0x3;

            if is_interrupt && cause_code == csr::INT_MTI {
                timer_fires += 1;
                last_timer_fire = count;
            }
        }

        // Count unique PCs every 1M instructions (for the last 1M)
        if count > 0 && count % 1_000_000 == 0 {
            *pc_hist.entry(vm.cpu.pc).or_insert(0) += 1;
        }

        // Log progress every 5M instructions
        if count > 0 && count % 5_000_000 == 0 {
            let sbi_out = vm.bus.sbi.console_output.len();
            let ecall_count = vm.bus.sbi.ecall_log.len();
            let new_ecalls = ecall_count - last_sbi_call_count;
            last_sbi_call_count = ecall_count;

            eprintln!(
                "[{:3}M] PC=0x{:08X} priv={:?} timer_fires={} new_ecalls={} sbi_out={}",
                count / 1_000_000,
                vm.cpu.pc,
                vm.cpu.privilege,
                timer_fires,
                new_ecalls,
                sbi_out
            );

            // Check mip/mie/mstatus
            let mip = vm.cpu.csr.mip;
            let mie = vm.cpu.csr.mie;
            let mstatus = vm.cpu.csr.mstatus;
            let mtip = (mip >> 7) & 1;
            let mtie = (mie >> 7) & 1;
            let mie_en = (mstatus >> csr::MSTATUS_MIE) & 1;
            let sie_en = (mstatus >> csr::MSTATUS_SIE) & 1;
            let mpp = (mstatus >> csr::MSTATUS_MPP_LSB) & 0x3;
            let spp = (mstatus >> csr::MSTATUS_SPP) & 1;

            eprintln!(
                "       MTIP={} MTIE={} MIE={} SIE={} MPP={} SPP={} satp=0x{:08X}",
                mtip, mtie, mie_en, sie_en, mpp, spp, vm.cpu.csr.satp
            );
            eprintln!(
                "       mtime={} mtimecmp={}",
                vm.bus.clint.mtime, vm.bus.clint.mtimecmp
            );
        }

        // Check if new SBI output appeared
        let cur_sbi_len = vm.bus.sbi.console_output.len();
        if count > 0 && count % 10_000_000 == 0 && cur_sbi_len > 0 {
            let s = String::from_utf8_lossy(&vm.bus.sbi.console_output);
            // Print last 200 chars
            let start = s.len().saturating_sub(200);
            eprintln!("       Last SBI output: ...{}", &s[start..]);
        }

        count += 1;
    }

    eprintln!("\n=== Summary ===");
    eprintln!("Total instructions: {}", count);
    eprintln!(
        "Timer interrupts: {} (last at {})",
        timer_fires, last_timer_fire
    );
    eprintln!("SBI calls: {}", vm.bus.sbi.ecall_log.len());
    eprintln!(
        "SBI console output: {} bytes",
        vm.bus.sbi.console_output.len()
    );
    eprintln!("PC: 0x{:08X}, Priv: {:?}", vm.cpu.pc, vm.cpu.privilege);

    // Print top 10 PCs
    let mut top_pcs: Vec<_> = pc_hist.iter().collect();
    top_pcs.sort_by(|a, b| b.1.cmp(a.1));
    eprintln!("\nTop 10 PCs (sampled every 1M):");
    for (pc, hits) in top_pcs.iter().take(10) {
        eprintln!("  PC=0x{:08X}: {} hits", pc, hits);
    }

    // Last 5 SBI calls
    let total_ecalls = vm.bus.sbi.ecall_log.len();
    eprintln!("\nLast 10 SBI calls:");
    let start = total_ecalls.saturating_sub(10);
    for i in start..total_ecalls {
        let (a7, a6, a0) = vm.bus.sbi.ecall_log[i];
        let name = match a7 {
            0x10 => "BASE",
            0x54494D45 => "TIMER",
            0x4442434E => "DBCN",
            0x735049 => "IPI",
            0x52464E43 => "RFNC",
            0x53525354 => "SRST",
            _ => "???",
        };
        eprintln!("  [{}] {} a6={} a0=0x{:08X}", i, name, a6, a0);
    }

    // Full SBI output
    if !vm.bus.sbi.console_output.is_empty() {
        let s = String::from_utf8_lossy(&vm.bus.sbi.console_output);
        eprintln!("\nFull SBI output:\n{}", s);
    }
}
