use geometry_os::riscv::RiscvVm;
use geometry_os::riscv::cpu::Privilege;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();

    let (mut vm, fw_addr_u64, _entry, _dtb_addr) =
        RiscvVm::boot_linux_setup(
            &kernel_image,
            initramfs.as_deref(),
            256,
            "console=ttyS0 loglevel=8",
        ).unwrap();

    let fw_addr = fw_addr_u64 as u32;
    let kernel_map_phys: u64 = 0x00C79E90;

    // Read kernel_map right after boot setup
    eprintln!("[kmap] After boot_linux_setup:");
    eprintln!("[kmap]   page_offset = 0x{:08X}",  vm.bus.read_word(kernel_map_phys + 0).unwrap_or(0));
    eprintln!("[kmap]   virt_addr   = 0x{:08X}",  vm.bus.read_word(kernel_map_phys + 4).unwrap_or(0));
    eprintln!("[kmap]   virt_offset = 0x{:08X}",  vm.bus.read_word(kernel_map_phys + 8).unwrap_or(0));
    eprintln!("[kmap]   phys_addr   = 0x{:08X}",  vm.bus.read_word(kernel_map_phys + 12).unwrap_or(0));
    eprintln!("[kmap]   size        = 0x{:08X}",  vm.bus.read_word(kernel_map_phys + 16).unwrap_or(0));
    eprintln!("[kmap]   va_pa_off   = 0x{:08X}",  vm.bus.read_word(kernel_map_phys + 20).unwrap_or(0));
    eprintln!("[kmap]   va_kern_off = 0x{:08X}",  vm.bus.read_word(kernel_map_phys + 24).unwrap_or(0));

    let mut count: u64 = 0;
    let mut last_satp: u32 = vm.cpu.csr.satp;
    let mut satp_count = 0;
    let target_satp_changes = 3;

    while count < 2_000_000 && satp_count < target_satp_changes {
        if vm.bus.sbi.shutdown_requested { break; }

        if vm.cpu.pc == fw_addr && vm.cpu.privilege == Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);

            if cause_code == 9 {
                let result = vm.bus.sbi.handle_ecall(
                    vm.cpu.x[17], vm.cpu.x[16],
                    vm.cpu.x[10], vm.cpu.x[11],
                    vm.cpu.x[12], vm.cpu.x[13],
                    vm.cpu.x[14], vm.cpu.x[15],
                    &mut vm.bus.uart, &mut vm.bus.clint,
                );
                if let Some((a0, a1)) = result {
                    vm.cpu.x[10] = a0;
                    vm.cpu.x[11] = a1;
                }
                vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
                count += 1;
                continue;
            } else if cause_code != 11 {
                let mpp = (vm.cpu.csr.mstatus >> 11) & 3;
                if mpp != 3 {
                    vm.cpu.csr.mepc = vm.cpu.csr.stvec;
                    vm.cpu.privilege = Privilege::Supervisor;
                    count += 1;
                    continue;
                }
            }
        }

        let current_satp = vm.cpu.csr.satp;
        if current_satp != last_satp {
            satp_count += 1;
            eprintln!(
                "[satp] #{}: 0x{:08X} -> 0x{:08X} at count={}",
                satp_count, last_satp, current_satp, count
            );
            // Check kernel_map after each SATP change
            eprintln!("[kmap] After SATP #{}:", satp_count);
            eprintln!("[kmap]   phys_addr   = 0x{:08X}", vm.bus.read_word(kernel_map_phys + 12).unwrap_or(0));
            eprintln!("[kmap]   va_pa_off   = 0x{:08X}", vm.bus.read_word(kernel_map_phys + 20).unwrap_or(0));
            eprintln!("[kmap]   va_kern_off = 0x{:08X}", vm.bus.read_word(kernel_map_phys + 24).unwrap_or(0));
            last_satp = current_satp;
        }

        let _ = vm.step();
        count += 1;
    }

    // Check kernel_map one final time
    eprintln!("[kmap] Final state at count={}:", count);
    eprintln!("[kmap]   phys_addr   = 0x{:08X}", vm.bus.read_word(kernel_map_phys + 12).unwrap_or(0));
    eprintln!("[kmap]   va_pa_off   = 0x{:08X}", vm.bus.read_word(kernel_map_phys + 20).unwrap_or(0));
    eprintln!("[kmap]   va_kern_off = 0x{:08X}", vm.bus.read_word(kernel_map_phys + 24).unwrap_or(0));

    // Check L1[768] raw value before fixup would run
    let satp = vm.cpu.csr.satp;
    let pg_dir_phys = ((satp & 0x003F_FFFF) as u64) << 12;
    let l1_768 = vm.bus.read_word(pg_dir_phys + 768 * 4).unwrap_or(0);
    eprintln!("[pt] L1[768] raw = 0x{:08X} at PA 0x{:08X}", l1_768, pg_dir_phys + 768 * 4);
    // Check if 0x00000001 is the actual value or if fixup changed it
    // Read a few bytes around it to see if it's really a minimal PTE
    eprintln!("[pt] Words around L1[768]:");
    for i in 767..770 {
        let addr = pg_dir_phys + (i as u64) * 4;
        let val = vm.bus.read_word(addr).unwrap_or(0);
        eprintln!("[pt]   L1[{}] at PA 0x{:08X}: 0x{:08X}", i, addr, val);
    }
}
