use geometry_os::riscv::cpu::Privilege;
use geometry_os::riscv::RiscvVm;
use std::fs;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = fs::read(kernel_path).expect("kernel");
    let initramfs = fs::read(initramfs_path).ok();

    let bootargs = "console=ttyS0 earlycon=sbi panic=5 quiet";
    let (mut vm, fw_addr, _entry, _dtb_addr) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 512, bootargs).unwrap();

    let fw_addr_u32 = fw_addr as u32;
    let max_count: u64 = 50_000_000;
    let mut count: u64 = 0;

    while count < max_count {
        if vm.bus.sbi.shutdown_requested {
            break;
        }
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let cause_code = vm.cpu.csr.mcause & !(1u32 << 31);
            if cause_code == 9 {
                let result = vm.bus.sbi.handle_ecall(
                    vm.cpu.x[17],
                    vm.cpu.x[16],
                    vm.cpu.x[10],
                    vm.cpu.x[11],
                    vm.cpu.x[12],
                    vm.cpu.x[13],
                    vm.cpu.x[14],
                    vm.cpu.x[15],
                    &mut vm.bus.uart,
                    &mut vm.bus.clint,
                );
                if let Some((a0, a1)) = result {
                    vm.cpu.x[10] = a0;
                    vm.cpu.x[11] = a1;
                }
            }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }
        vm.bus.tick_clint();
        vm.bus.sync_mip(&mut vm.cpu.csr.mip);
        vm.step();
        count += 1;
    }

    // Read lpj_fine from the kernel's memory
    // lpj_fine is at VA 0xC1482060, which is in the linear mapping
    // SATP is the current page table
    let satp = vm.cpu.csr.satp;
    eprintln!("SATP: 0x{:08X}", satp);

    // lpj_fine at VA 0xC1482060 -> PA = VA - 0xC0000000 = 0x1482060
    let lpj_pa: u64 = 0x1482060;
    let lpj_fine = vm.bus.read_word(lpj_pa).unwrap_or(0);
    eprintln!("lpj_fine (PA 0x{:08X}): {}", lpj_pa, lpj_fine);

    // Also check loops_per_jiffy
    let lpj_pa2: u64 = 0x1482064;
    let lpj = vm.bus.read_word(lpj_pa2).unwrap_or(0);
    eprintln!("loops_per_jiffy (PA 0x{:08X}): {}", lpj_pa2, lpj);

    // Check riscv_timebase at the CORRECT address (VA 0xC0C7A058, PA 0x00C7A058)
    let rtb_pa: u64 = 0x00C7A058;
    let rtb = vm.bus.read_word(rtb_pa).unwrap_or(0);
    eprintln!("riscv_timebase (PA 0x{:08X}): {}", rtb_pa, rtb);

    // Check if CLINT timer is advancing
    eprintln!("CLINT mtime: {}", vm.bus.clint.mtime);
    eprintln!("CLINT mtimecmp: {}", vm.bus.clint.mtimecmp);

    // Check timer interrupt pending
    eprintln!("MIP.STIP: {}", (vm.cpu.csr.mip >> 5) & 1);
    eprintln!("MIE.STIE: {}", (vm.cpu.csr.mie >> 5) & 1);
    eprintln!("SIE.STIE: {}", (vm.cpu.csr.mie >> 5) & 1);

    // If lpj_fine is very large, udelay will take forever
    // The kernel calls udelay during early init (console setup, etc.)
    // A typical udelay(100) with lpj_fine=400000 would loop ~20M times
    if lpj_fine > 100_000_000 {
        eprintln!(
            "\n*** WARNING: lpj_fine is VERY large! udelay will take billions of instructions ***"
        );
    }

    let tx = vm.bus.uart.drain_tx();
    eprintln!("\nUART: {} bytes", tx.len());
    if !tx.is_empty() {
        let s = String::from_utf8_lossy(&tx);
        eprintln!("{}", &s[..s.len().min(2000)]);
    }
}
