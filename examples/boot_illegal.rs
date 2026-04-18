use geometry_os::riscv::cpu::{Privilege, StepResult};
use geometry_os::riscv::RiscvVm;
use std::fs;
use std::time::Instant;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = fs::read(kernel_path).expect("kernel");
    let initramfs = fs::read(initramfs_path).ok();

    let (mut vm, fw_addr, _entry, dtb_addr) = RiscvVm::boot_linux_setup(
        &kernel_image,
        initramfs.as_deref(),
        512,
        "console=ttyS0 earlycon=uart8250,mmio32,0x10000000 loglevel=8",
    ).unwrap();

    let fw_addr_u32 = fw_addr as u32;
    let mut count: u64 = 0;
    let mut sbi_count: u64 = 0;
    let mut last_satp: u32 = vm.cpu.csr.satp;
    let mut illegal_count: u64 = 0;

    let start = Instant::now();

    while count < 50_000_000 {
        if vm.bus.sbi.shutdown_requested { break; }

        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);

            if cause_code == 9 {
                sbi_count += 1;
                if sbi_count <= 5 {
                    eprintln!("[sbi] #{} a7=0x{:02X} a6=0x{:02X} a0=0x{:08X}", 
                        sbi_count, vm.cpu.x[17], vm.cpu.x[16], vm.cpu.x[10]);
                }
                let result = vm.bus.sbi.handle_ecall(
                    vm.cpu.x[17], vm.cpu.x[16], vm.cpu.x[10], vm.cpu.x[11],
                    vm.cpu.x[12], vm.cpu.x[13], vm.cpu.x[14], vm.cpu.x[15],
                    &mut vm.bus.uart, &mut vm.bus.clint,
                );
                if let Some((a0, a1)) = result { vm.cpu.x[10] = a0; vm.cpu.x[11] = a1; }
            } else if cause_code != 11 {
                let mpp = (vm.cpu.csr.mstatus >> 11) & 3;
                if mpp != 3 {
                    let stvec = vm.cpu.csr.stvec & !0x3u32;
                    if stvec != 0 {
                        vm.cpu.csr.sepc = vm.cpu.csr.mepc;
                        vm.cpu.csr.scause = mcause;
                        vm.cpu.csr.stval = vm.cpu.csr.mtval;
                        let spp = if mpp == 1 { 1u32 } else { 0u32 };
                        vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << 8)) | (spp << 8);
                        let sie = (vm.cpu.csr.mstatus >> 1) & 1;
                        vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << 5)) | (sie << 5);
                        vm.cpu.csr.mstatus &= !(1 << 1);
                        if cause_code == 7 { vm.bus.clint.mtimecmp = vm.bus.clint.mtime + 100_000; }
                        vm.cpu.pc = stvec;
                        vm.cpu.privilege = Privilege::Supervisor;
                        vm.cpu.tlb.flush_all();
                        count += 1;
                        continue;
                    }
                }
                
                // Log illegal instructions with full page table analysis
                if cause_code == 2 {
                    illegal_count += 1;
                    if illegal_count <= 10 {
                        let mepc = vm.cpu.csr.mepc;
                        eprintln!("[ILLEGAL #{}] count={}: mepc=0x{:08X} mpp={}", illegal_count, count, mepc, mpp);
                        
                        // Read instruction at mepc (physical address)
                        let mepc_pa = if mepc >= 0xC0000000 { mepc - 0xC0000000 } else { mepc };
                        let inst_pa = vm.bus.read_word(mepc_pa as u64).unwrap_or(0);
                        eprintln!("  PA 0x{:08X}: instruction = 0x{:08X}", mepc_pa, inst_pa);
                        
                        // Check what should be there from the ELF
                        // L1 index for the mepc VA
                        let l1_idx = (mepc >> 22) & 0x3FF;
                        let satp = vm.cpu.csr.satp;
                        let pg_dir_ppn = satp & 0x3FFFFF;
                        let pg_dir_phys = (pg_dir_ppn as u64) * 4096;
                        let l1_entry = vm.bus.read_word(pg_dir_phys + (l1_idx as u64) * 4).unwrap_or(0);
                        let is_leaf = (l1_entry & 0xE) != 0;
                        let ppn1 = (l1_entry >> 10) & 0x3FFFFF;
                        eprintln!("  L1[{}] = 0x{:08X} V={} leaf={} ppn1=0x{:06X}", 
                            l1_idx, l1_entry, (l1_entry & 1) != 0, is_leaf, ppn1);
                        
                        if !is_leaf && (l1_entry & 1) != 0 {
                            let l2_idx = (mepc >> 12) & 0x3FF;
                            let l2_entry = vm.bus.read_word((ppn1 as u64) * 4096 + (l2_idx as u64) * 4).unwrap_or(0);
                            let l2_ppn = (l2_entry >> 10) & 0x3FFFFF;
                            eprintln!("  L2[{}] at PA 0x{:08X} = 0x{:08X} V={} leaf={} ppn=0x{:06X}",
                                l2_idx, ppn1 * 4096, l2_entry, (l2_entry & 1) != 0,
                                (l2_entry & 0xE) != 0, l2_ppn);
                            // The final physical address
                            let final_pa = (l2_ppn * 4096) + (mepc & 0xFFF);
                            let inst_at_pa = vm.bus.read_word(final_pa as u64).unwrap_or(0);
                            eprintln!("  Translated PA = 0x{:08X}, inst = 0x{:08X}", final_pa, inst_at_pa);
                        }
                        
                        // Expected PA (from direct mapping)
                        let expected_pa = mepc - 0xC0000000;
                        let expected_inst = vm.bus.read_word(expected_pa as u64).unwrap_or(0);
                        eprintln!("  Expected PA 0x{:08X}: inst = 0x{:08X}", expected_pa, expected_inst);
                    }
                }
            }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }

        // Log SATP changes
        let cur_satp = vm.cpu.csr.satp;
        if cur_satp != last_satp {
            eprintln!("[SATP] 0x{:08X} -> 0x{:08X} at count={}", last_satp, cur_satp, count);
            last_satp = cur_satp;
        }

        vm.bus.tick_clint();
        vm.bus.sync_mip(&mut vm.cpu.csr.mip);
        match vm.step() {
            StepResult::Ebreak => { eprintln!("[EBREAK] at count={}", count); break; }
            _ => {}
        }
        count += 1;
    }
    
    let elapsed = start.elapsed();
    eprintln!("[boot] Done: count={} sbi={} illegal={} time={:?}", count, sbi_count, illegal_count, elapsed);
    
    let tx = vm.bus.uart.drain_tx();
    if !tx.is_empty() {
        let s = String::from_utf8_lossy(&tx);
        eprintln!("[UART] ({} bytes): {}", tx.len(), &s[..s.len().min(2000)]);
    } else {
        eprintln!("[UART] No output");
    }
}
