/// Diagnostic: Check what the kernel's early_pg_dir looks like BEFORE and AFTER fixup.
/// We'll manually walk the page table at PA 0x00802000.
use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1";

    let (mut vm, fw_addr, _entry, _dtb_addr) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs).unwrap();

    let max = 178_000u64;
    let mut count: u64 = 0;
    let fw_addr_u32 = fw_addr as u32;

    while count < max {
        if vm.bus.sbi.shutdown_requested {
            break;
        }

        // Handle M-mode trap forwarding
        if vm.cpu.pc == fw_addr_u32
            && vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Machine
        {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            if cause_code == 11 {
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
            } else {
                let mpp = (vm.cpu.csr.mstatus & 0x1800) >> 11;
                if mpp != 3 {
                    let stvec = vm.cpu.csr.stvec & !0x3u32;
                    if stvec != 0 {
                        vm.cpu.csr.sepc = vm.cpu.csr.mepc;
                        vm.cpu.csr.scause = mcause;
                        vm.cpu.csr.stval = vm.cpu.csr.mtval;
                        let spp = if mpp == 1 { 1u32 } else { 0u32 };
                        vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << 5)) | (spp << 5);
                        let sie = (vm.cpu.csr.mstatus >> 1) & 1;
                        vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << 5)) | (sie << 5);
                        vm.cpu.csr.mstatus &= !(1 << 1);
                        vm.cpu.pc = stvec;
                        vm.cpu.privilege = geometry_os::riscv::cpu::Privilege::Supervisor;
                        vm.cpu.tlb.flush_all();
                        count += 1;
                        continue;
                    }
                }
            }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }

        // Capture state just BEFORE the second SATP write (which triggers fixup on early_pg_dir)
        let cur_satp = vm.cpu.csr.satp;
        if count == 177330 {
            // Just before the SATP change to early_pg_dir
            println!(
                "[pre-fixup] count={} PC=0x{:08X} SATP=0x{:08X}",
                count, vm.cpu.pc, cur_satp
            );

            // Read early_pg_dir BEFORE fixup (PA 0x00802000)
            println!("\n[pre-fixup] early_pg_dir at PA 0x00802000 (BEFORE fixup):");
            let pg_dir_phys: u64 = 0x00802000;
            for i in 0..1024u32 {
                let addr = pg_dir_phys + (i as u64) * 4;
                if let Ok(pte) = vm.bus.read_word(addr) {
                    if pte != 0 {
                        let ppn = ((pte >> 10) & 0x003F_FFFF) as u32;
                        let is_leaf = (pte & 0xE) != 0;
                        let va_start = (i as u64) << 22;
                        let va_end = va_start | 0x3FFFFF;
                        let expected_ppn = if ppn >= 0xC0000 { ppn - 0xC0000 } else { ppn };
                        println!(
                            "  L1[{:3}] VA 0x{:08X}-0x{:08X}: PTE=0x{:08X} PPN=0x{:05X} {} {}",
                            i,
                            va_start as u32,
                            va_end as u32,
                            pte,
                            ppn,
                            if is_leaf { "(mega)" } else { "(L2)" },
                            if ppn >= 0xC0000 {
                                format!("NEEDS_FIXUP -> PPN=0x{:05X}", expected_ppn)
                            } else {
                                String::new()
                            }
                        );
                    }
                }
            }
        }

        let _ = vm.step();

        // After the fixup
        if count == 177335 {
            println!(
                "\n[post-fixup] count={} PC=0x{:08X} SATP=0x{:08X}",
                count, vm.cpu.pc, vm.cpu.csr.satp
            );
            println!("[post-fixup] early_pg_dir at PA 0x00802000 (AFTER fixup):");
            let pg_dir_phys: u64 = 0x00802000;
            for i in 0..1024u32 {
                let addr = pg_dir_phys + (i as u64) * 4;
                if let Ok(pte) = vm.bus.read_word(addr) {
                    if pte != 0 {
                        let ppn = ((pte >> 10) & 0x003F_FFFF) as u32;
                        let is_leaf = (pte & 0xE) != 0;
                        let va_start = (i as u64) << 22;
                        let va_end = va_start | 0x3FFFFF;
                        println!(
                            "  L1[{:3}] VA 0x{:08X}-0x{:08X}: PTE=0x{:08X} PPN=0x{:05X} {}",
                            i,
                            va_start as u32,
                            va_end as u32,
                            pte,
                            ppn,
                            if is_leaf { "(mega)" } else { "(L2)" }
                        );
                    }
                }
            }
        }

        count += 1;
    }
}
