// Check if low_addr_identity_map is actually being triggered
use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = std::path::Path::new(".geometry_os/build/linux-6.14/vmlinux");
    let kernel = std::fs::read(kernel_path).expect("kernel");
    let initramfs_path = std::path::Path::new(".geometry_os/fs/linux/rv32/initramfs.cpio.gz");
    let initramfs = std::fs::read(initramfs_path).ok();

    let (mut vm, _fw_addr, _entry, _dtb_addr) = RiscvVm::boot_linux_setup(
        &kernel,
        initramfs.as_deref(),
        128,
        "console=ttyS0 earlycon=sbi loglevel=8",
    )
    .unwrap();

    println!("low_addr_identity_map = {}", vm.bus.low_addr_identity_map);

    let mut count: u64 = 0;
    let max: u64 = 250_000;
    let mut prev_log_len = 0usize;

    while count < max {
        let _result = vm.step();
        count += 1;

        // Check MMU log for identity map events
        let log_len = vm.bus.mmu_log.len();
        if log_len > prev_log_len {
            for i in prev_log_len..log_len {
                let entry = &vm.bus.mmu_log[i];
                let desc = match entry {
                    geometry_os::riscv::mmu::MmuEvent::PageFault { va, .. } => {
                        format!("PageFault VA=0x{:08X}", va)
                    }
                    geometry_os::riscv::mmu::MmuEvent::TlbHit { va, pa } => {
                        format!("TlbHit VA=0x{:08X} PA=0x{:08X}", va, pa)
                    }
                    geometry_os::riscv::mmu::MmuEvent::PageTableWalk { va, pa, .. } => {
                        format!("PTWalk VA=0x{:08X} PA=0x{:08X}", va, pa)
                    }
                    _ => format!("Other"),
                };
                if count > 190_000 && count < 210_000 {
                    println!("[mmu] count={} event={}", count, desc);
                }
            }
            prev_log_len = log_len;
        }
    }

    println!("[done] {} instr, PC=0x{:08X}", count, vm.cpu.pc);
}
