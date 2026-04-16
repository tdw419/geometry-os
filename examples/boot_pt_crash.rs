/// Check what the kernel's page table L1[768] looks like at the crash point.
use geometry_os::riscv::RiscvVm;
use geometry_os::riscv::cpu::StepResult;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=uart8250,mmio,0x10000000 panic=1";

    let (mut vm, fw_addr, _entry, _dtb) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs).unwrap();

    let fw_addr_u32 = fw_addr as u32;
    let mut count: u64 = 0;
    let mut last_satp: u32 = vm.cpu.csr.satp;
    let mut checked = false;

    while count < 185_000 {
        if vm.bus.sbi.shutdown_requested {
            break;
        }

        // SATP change handling
        {
            let cur_satp = vm.cpu.csr.satp;
            if cur_satp != last_satp {
                let mode = (cur_satp >> 31) & 1;
                if mode == 1 {
                    let ppn = cur_satp & 0x3FFFFF;
                    let pg_dir_phys = (ppn as u64) * 4096;
                    let l1_0_val = vm.bus.read_word(pg_dir_phys).unwrap_or(0);
                    let already_patched =
                        (l1_0_val & 0xCF) == 0xCF && ((l1_0_val >> 20) & 0xFFF) == 0;
                    if !already_patched {
                        let l1_entries: &[u32] = &[
                            0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 16, 32, 48, 64, 80, 96, 112, 127,
                        ];
                        for &l1_idx in l1_entries {
                            let pte = 0xCF | (l1_idx << 20);
                            vm.bus
                                .write_word(pg_dir_phys + (l1_idx * 4) as u64, pte)
                                .ok();
                        }
                        vm.cpu.tlb.flush_all();
                    }
                }
                last_satp = cur_satp;
            }
        }

        // M-mode trap handler
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Machine {
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
                if let Some((a0_val, a1_val)) = result {
                    vm.cpu.x[10] = a0_val;
                    vm.cpu.x[11] = a1_val;
                }
            } else if cause_code != 11 {
                let mpp = (vm.cpu.csr.mstatus & 0x1800) >> 11;
                if cause_code == 8 && mpp != 3 {
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

        vm.step();
        count += 1;

        // After RA changes, dump page table state
        if !checked && count >= 178_500 && vm.cpu.x[1] == 0x3FFFF000 {
            checked = true;
            let satp = vm.cpu.csr.satp;
            let ppn = satp & 0x3FFFFF;
            let pg_dir_phys = (ppn as u64) * 4096;
            
            eprintln!("[diag] CRASH ANALYSIS at count={}", count);
            eprintln!("[diag] PC=0x{:08X} RA=0x{:08X} SP=0x{:08X}", vm.cpu.pc, vm.cpu.x[1], vm.cpu.x[2]);
            eprintln!("[diag] SATP=0x{:08X} pg_dir_phys=0x{:08X}", satp, pg_dir_phys);
            
            // Check L1[768] which covers VA 0xC0000000-0xC01FFFFF
            // The crash is at PC=0xC003F9CC
            let l1_768 = vm.bus.read_word(pg_dir_phys + 768 * 4).unwrap_or(0);
            eprintln!("[diag] L1[768] (covers 0xC0000000-0xC01FFFFF) = 0x{:08X}", l1_768);
            
            let l1_ppn = (l1_768 & 0xFFFFFC00) >> 10;
            eprintln!("[diag]   L1[768] PPN = 0x{:08X} (phys = 0x{:08X})", l1_ppn, (l1_ppn as u64) << 12);
            
            // Check if it's a megapage or L2 pointer
            let is_leaf = (l1_768 & 0xE) != 0;
            eprintln!("[diag]   is_leaf={}, V={}", is_leaf, (l1_768 & 1) != 0);
            
            if !is_leaf && (l1_768 & 1) != 0 {
                // L2 pointer - dump first few L2 entries
                let l2_base = (l1_ppn as u64) << 12;
                eprintln!("[diag]   L2 table at PA 0x{:08X}", l2_base);
                
                // VA 0xC003F9CC: VPN0 = (0xC003F9CC >> 12) & 0x3FF = 0x03F
                let vpn0_crash = (0xC003F9CC >> 12) & 0x3FF;
                eprintln!("[diag]   crash VPN0 = {} (0x{:03X})", vpn0_crash, vpn0_crash);
                
                for i in 0..64 {
                    let l2_entry = vm.bus.read_word(l2_base + i * 4).unwrap_or(0);
                    if l2_entry != 0 || i <= 3 {
                        let l2_ppn = (l2_entry & 0xFFFFFC00) >> 10;
                        eprintln!("[diag]   L2[{}] = 0x{:08X} PPN=0x{:08X} V={} R={} W={} X={}", 
                            i, l2_entry, l2_ppn,
                            (l2_entry & 1) != 0,
                            (l2_entry & 2) != 0,
                            (l2_entry & 4) != 0,
                            (l2_entry & 8) != 0);
                    }
                }
                
                // Also read the physical memory at the crash VA
                // With fixup, if L2[63] has PPN with virtual offset, the PA should be computed correctly
                let l2_crash = vm.bus.read_word(l2_base + vpn0_crash * 4).unwrap_or(0);
                eprintln!("[diag]   L2[{}] (for crash VA) = 0x{:08X}", vpn0_crash, l2_crash);
            }
            
            // Check what's at physical address 0x3F9CC (where crash VA 0xC003F9CC should map)
            let pa_direct = 0x3F9CCu64;
            let val_at_pa = vm.bus.read_word(pa_direct).unwrap_or(0);
            eprintln!("[diag]   word at PA 0x{:08X} = 0x{:08X}", pa_direct, val_at_pa);
            
            // Check what's at physical address 0x003F9CC
            let val_at_pa2 = vm.bus.read_word(0x003F9CC).unwrap_or(0);
            eprintln!("[diag]   word at PA 0x003F9CC = 0x{:08X}", val_at_pa2);
            
            break;
        }
    }
}
