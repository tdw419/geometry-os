// Quick check: L1[769] in swapper_pg_dir after 3rd SATP change.
use std::fs;
use geometry_os::riscv::RiscvVm;
use geometry_os::riscv::cpu::StepResult;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = fs::read(kernel_path).expect("kernel");
    let initramfs = fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1 quiet";

    let (mut vm, fw_addr, _entry, _dtb_addr) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs)
            .expect("boot setup failed");

    let fw_addr_u32 = fw_addr as u32;
    let mut count: u64 = 0;
    let mut last_satp: u32 = vm.cpu.csr.satp;
    let mut satp_changes: u32 = 0;

    while count < 760_000 {
        let cur_satp = vm.cpu.csr.satp;
        if cur_satp != last_satp {
            satp_changes += 1;
            eprintln!("[diag] SATP change #{} at count={}: 0x{:08X} -> 0x{:08X}", satp_changes, count, last_satp, cur_satp);
            let mode = (cur_satp >> 31) & 1;
            if mode == 1 {
                let ppn = cur_satp & 0x3FFFFF;
                let pg_dir_phys = (ppn as u64) * 4096;
                let device_l1_entries: &[u32] = &[0, 1, 2, 3, 4, 5, 8, 48, 64];
                let identity_pte: u32 = 0x0000_00CF;
                for &l1_idx in device_l1_entries {
                    let addr = pg_dir_phys + (l1_idx as u64) * 4;
                    let existing = vm.bus.read_word(addr).unwrap_or(0);
                    if (existing & 1) == 0 {
                        let pte = identity_pte | (l1_idx << 20);
                        vm.bus.write_word(addr, pte).ok();
                    }
                }
                vm.cpu.tlb.flush_all();

                // After 3rd SATP change (swapper_pg_dir), check key L1 entries
                if satp_changes >= 3 {
                    eprintln!("[diag] Page table at PA 0x{:08X}", pg_dir_phys);
                    // Check L1 entries for kernel VA regions
                    for l1_idx in [768, 769, 770, 771, 772, 773] {
                        let l1_addr = pg_dir_phys + (l1_idx as u64) * 4;
                        let l1_pte = vm.bus.read_word(l1_addr).unwrap_or(0);
                        let ppn = l1_pte >> 10;
                        let flags = l1_pte & 0x3FF;
                        let is_leaf = (l1_pte & (1 << 1)) != 0 || (l1_pte & (1 << 3)) != 0;
                        eprintln!("[diag]   L1[{}] (VA 0x{:08X}): pte=0x{:08X} ppn=0x{:X} flags=0x{:03X} is_leaf={}",
                            l1_idx, l1_idx << 22, l1_pte, ppn, flags, is_leaf);
                        
                        if !is_leaf && (l1_pte & 1) != 0 {
                            // Check L2 entries
                            let l2_base = (ppn as u64) << 12;
                            // Check the specific L2 entry for the faulting VA
                            // VA 0xC0404E12: VPN0 = (0xC0404E12 >> 12) & 0x3FF = 0x4E = 78
                            let vpn0 = (0xC0404E12u32 >> 12) & 0x3FF;
                            let l2_addr = l2_base + (vpn0 as u64) * 4;
                            let l2_pte = vm.bus.read_word(l2_addr).unwrap_or(0);
                            eprintln!("[diag]     L2[{}] at PA 0x{:08X}: pte=0x{:08X} ppn=0x{:X}",
                                vpn0, l2_addr, l2_pte, l2_pte >> 10);
                        }
                    }
                }
            }
            last_satp = cur_satp;
        }

        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            if cause_code == 11 {
                let result = vm.bus.sbi.handle_ecall(
                    vm.cpu.x[17], vm.cpu.x[16],
                    vm.cpu.x[10], vm.cpu.x[11],
                    vm.cpu.x[12], vm.cpu.x[13],
                    vm.cpu.x[14], vm.cpu.x[15],
                    &mut vm.bus.uart, &mut vm.bus.clint,
                );
                if let Some((a0, a1)) = result { vm.cpu.x[10] = a0; vm.cpu.x[11] = a1; }
            } else {
                let mpp = (vm.cpu.csr.mstatus & 0x3000) >> 12;
                if mpp != 3 && (vm.cpu.csr.stvec & !0x3) != 0 {
                    vm.cpu.csr.sepc = vm.cpu.csr.mepc;
                    vm.cpu.csr.scause = mcause;
                    vm.cpu.csr.stval = vm.cpu.csr.mtval;
                    let spp = if mpp == 1 { 1u32 } else { 0u32 };
                    vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << 5)) | (spp << 5);
                    let sie = (vm.cpu.csr.mstatus >> 1) & 1;
                    vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << 5)) | (sie << 5);
                    vm.cpu.csr.mstatus &= !(1 << 1);
                    if cause_code == 7 { vm.bus.clint.mtimecmp = vm.bus.clint.mtime + 100_000; }
                    vm.cpu.pc = vm.cpu.csr.stvec & !0x3;
                    vm.cpu.privilege = geometry_os::riscv::cpu::Privilege::Supervisor;
                    vm.cpu.tlb.flush_all();
                    count += 1;
                    continue;
                }
            }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }

        vm.bus.tick_clint();
        vm.bus.sync_mip(&mut vm.cpu.csr.mip);
        let step_result = vm.step();

        if satp_changes >= 3 && count < 751500 && count >= 751470 {
            match step_result {
                StepResult::FetchFault | StepResult::LoadFault | StepResult::StoreFault => {
                    eprintln!("[diag] Fault at count={}: PC=0x{:08X} sepc=0x{:08X} stval=0x{:08X}",
                        count, vm.cpu.pc, vm.cpu.csr.sepc, vm.cpu.csr.stval);
                }
                StepResult::Ok => {
                    if vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Supervisor {
                        eprintln!("[diag] OK at count={}: PC=0x{:08X}", count, vm.cpu.pc);
                    }
                }
                _ => {}
            }
        }

        count += 1;
    }
    eprintln!("[diag] Done at count={}", count);
}
