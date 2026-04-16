/// Check if TLB has a stale entry causing the wrong translation.
/// Also check if the fixup is actually being applied by checking the raw PTE value.
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
    let mut ra_captured = false;
    let mut capture_count = 0u64;

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
        if vm.cpu.pc == fw_addr_u32
            && vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Machine
        {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
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
                        vm.cpu.csr.mstatus =
                            (vm.cpu.csr.mstatus & !(1 << 5)) | (spp << 5);
                        let sie = (vm.cpu.csr.mstatus >> 1) & 1;
                        vm.cpu.csr.mstatus =
                            (vm.cpu.csr.mstatus & !(1 << 5)) | (sie << 5);
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

        // Check if we're approaching the crash PC
        // The crash happens at PC=0xC003F9CC (approx)
        if !ra_captured && vm.cpu.pc >= 0xC003F900 && vm.cpu.pc <= 0xC003FA00 && vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Supervisor {
            if !checked {
                checked = true;
                eprintln!("[diag] Approaching crash area at count={}: PC=0x{:08X}", count, vm.cpu.pc);
                // Dump the instruction at the current PC
                let inst = vm.bus.read_word(vm.cpu.pc as u64).unwrap_or(0);
                eprintln!("[diag]   instruction at PC (phys read): 0x{:08X}", inst);
                
                // Try to read via MMU translation
                // The step function already does this, but let's check what the MMU returns
                let satp = vm.cpu.csr.satp;
                let ppn = satp & 0x3FFFFF;
                let pg_dir_phys = (ppn as u64) * 4096;
                let l1_768 = vm.bus.read_word(pg_dir_phys + 768 * 4).unwrap_or(0);
                eprintln!("[diag]   L1[768] = 0x{:08X}", l1_768);
                
                // Is the instruction non-zero?
                if inst == 0 {
                    eprintln!("[diag]   *** ZERO INSTRUCTION at PC! MMU may be mapping wrong ***");
                    
                    // Let's also check what physical address the MMU would compute
                    // VA 0xC003F9CC: VPN1=768, VPN0=63
                    let vpn1 = (vm.cpu.pc >> 22) & 0x3FF;
                    let vpn0 = (vm.cpu.pc >> 12) & 0x3FF;
                    eprintln!("[diag]   VPN1={} VPN0={}", vpn1, vpn0);
                    
                    // Read L2 table if L1[768] is non-leaf
                    let l1_pte = l1_768;
                    let l1_is_leaf = (l1_pte & 0xE) != 0;
                    if l1_is_leaf {
                        // Megapage
                        let l1_ppn_raw = (l1_pte & 0xFFFFFC00) >> 10;
                        // With fixup
                        let page_offset_ppn: u32 = 0xC0000000 >> 12;
                        let l1_ppn_fixed = if l1_ppn_raw >= page_offset_ppn {
                            l1_ppn_raw - page_offset_ppn
                        } else {
                            l1_ppn_raw
                        };
                        let ppn_hi = (l1_ppn_fixed >> 10) & 0xFFF;
                        let pa = (ppn_hi as u64) << 22 | ((vpn0 as u64) << 12) | ((vm.cpu.pc & 0xFFF) as u64);
                        eprintln!("[diag]   L1 megapage: raw_ppn=0x{:X} fixed_ppn=0x{:X} ppn_hi={}", l1_ppn_raw, l1_ppn_fixed, ppn_hi);
                        eprintln!("[diag]   Computed PA = 0x{:08X}", pa);
                        let word_at_pa = vm.bus.read_word(pa).unwrap_or(0);
                        eprintln!("[diag]   Word at computed PA = 0x{:08X}", word_at_pa);
                    }
                    
                    // Also check: what if the CPU is reading from PA = VA (no fixup)?
                    let pa_no_fixup = vm.cpu.pc as u64; // identity mapping
                    eprintln!("[diag]   Word at VA-as-PA (0x{:08X}) = 0x{:08X}", pa_no_fixup, vm.bus.read_word(pa_no_fixup).unwrap_or(0));
                    
                    // And what if fixup gives wrong result due to L1[768] being a leaf with VPN0=0?
                    // Let me check: the diagnostic earlier showed VPN0=63 for 0xC003F9CC
                    // But wait, 0xC003F9CC >> 12 = 0xC003F, & 0x3FF = 0x03F = 63. Correct.
                }
            }
        }
        
        let step_result = vm.step();
        
        if !ra_captured && vm.cpu.x[1] == 0x3FFFF000 {
            ra_captured = true;
            capture_count = count;
            eprintln!("[diag] RA became 0x3FFFF000 at count={}", count);
        }
        
        if let StepResult::Ebreak = step_result {
            break;
        }
        _ = step_result;
        count += 1;
    }
    
    if !checked {
        eprintln!("[diag] Never reached crash area");
    }
    if !ra_captured {
        eprintln!("[diag] RA never became 0x3FFFF000");
    }
}
