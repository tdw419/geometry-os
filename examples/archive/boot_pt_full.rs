use std::fs;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = fs::read(kernel_path).expect("kernel");
    let initramfs = fs::read(initramfs_path).ok();

    let bootargs = "console=ttyS0 earlycon=sbi panic=5 quiet";

    let (mut vm, fw_addr, _entry, _dtb_addr) = geometry_os::riscv::RiscvVm::boot_linux_setup(
        &kernel_image,
        initramfs.as_deref(),
        512,
        bootargs,
    )
    .unwrap();

    let fw_addr_u32 = fw_addr as u32;
    let mut count: u64 = 0;
    let max_instructions: u64 = 178_000u64;
    let mut dumped = false;

    while count < max_instructions {
        if vm.bus.sbi.shutdown_requested {
            break;
        }

        if vm.cpu.pc == fw_addr_u32
            && vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Machine
        {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            let mpp = (vm.cpu.csr.mstatus & 0x3000) >> 12;

            if cause_code == 11 || cause_code == 9 {
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
            } else if mpp != 3 {
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

                    // Dump page table on first forward
                    if !dumped {
                        dumped = true;
                        eprintln!(
                            "[diag] FIRST FORWARD at count={}: cause={} sepc=0x{:08X} mpp={}",
                            count, cause_code, vm.cpu.csr.sepc, mpp
                        );

                        let satap = vm.cpu.csr.satp;
                        let pt_root = ((satap & 0x3FFFFF) as u64) << 12;
                        eprintln!("[diag] satap=0x{:08X} pt_root=0x{:08X}", satap, pt_root);

                        // Dump L1 page table (1024 entries in SV32)
                        let mut l1_valid = 0;
                        for i in 0..1024 {
                            let addr = pt_root + (i as u64) * 4;
                            if let Ok(pte) = vm.bus.read_word(addr) {
                                if pte != 0 {
                                    let v = pte & 1;
                                    let r = (pte >> 1) & 1;
                                    let w = (pte >> 2) & 1;
                                    let x = (pte >> 3) & 1;
                                    let ppn = pte >> 10;
                                    let is_leaf = (r | w | x) != 0;
                                    eprintln!(
                                        "  L1[{:3}] = 0x{:08X} V={} R={} W={} X={} PPN=0x{:06X} {}",
                                        i,
                                        pte,
                                        v,
                                        r,
                                        w,
                                        x,
                                        ppn,
                                        if is_leaf { "MEGAPAGE" } else { "L2->" }
                                    );
                                    l1_valid += 1;

                                    // If L2 pointer, dump first few L2 entries
                                    if !is_leaf && l1_valid <= 5 {
                                        let l2_addr = (ppn as u64) << 12;
                                        for j in 0..16 {
                                            if let Ok(l2pte) =
                                                vm.bus.read_word(l2_addr + (j as u64) * 4)
                                            {
                                                if l2pte != 0 {
                                                    let l2ppn = l2pte >> 10;
                                                    let l2r = (l2pte >> 1) & 1;
                                                    let l2w = (l2pte >> 2) & 1;
                                                    let l2x = (l2pte >> 3) & 1;
                                                    eprintln!("    L2[{:3}] = 0x{:08X} R={} W={} X={} PPN=0x{:06X} PA=0x{:08X}",
                                                        j, l2pte, l2r, l2w, l2x, l2ppn, (l2ppn as u64) << 12);
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        eprintln!("[diag] Total valid L1 entries: {}", l1_valid);

                        // Check what virtual 0x10FA would map to
                        let vpn1 = (0x10FAu32 >> 22) & 0x3FF;
                        eprintln!("[diag] Virtual 0x10FA: vpn1={} (L1 index)", vpn1);
                        let l1pte_addr = pt_root + (vpn1 as u64) * 4;
                        if let Ok(l1pte) = vm.bus.read_word(l1pte_addr) {
                            eprintln!("  L1[{}] = 0x{:08X}", vpn1, l1pte);
                            if l1pte & 1 != 0 {
                                // Check if identity mapping for low addresses exists
                                eprintln!("  Entry is valid!");
                            } else {
                                eprintln!("  Entry is INVALID - page fault!");
                            }
                        }
                    }

                    count += 1;
                    continue;
                }
            }

            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }

        let step_result = vm.step();
        if matches!(step_result, geometry_os::riscv::cpu::StepResult::Ebreak) {
            break;
        }

        count += 1;
    }
}
