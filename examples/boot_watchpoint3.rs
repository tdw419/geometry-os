fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1";

    use geometry_os::riscv::cpu::Privilege;
    use geometry_os::riscv::csr;
    use geometry_os::riscv::RiscvVm;

    let (mut vm, fw_addr, _entry, _dtb_addr) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs).unwrap();

    let fw_addr_u32 = fw_addr as u32;
    vm.bus.write_watch_addr = Some(0x1002C08);

    let max_instr = 20_000_000u64;
    let mut count: u64 = 0;
    let mut last_pte_v: Option<bool> = None;

    while count < max_instr {
        if vm.bus.sbi.shutdown_requested {
            break;
        }
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            let mpp = (vm.cpu.csr.mstatus & csr::MSTATUS_MPP_MASK) >> csr::MSTATUS_MPP_LSB;
            if cause_code == csr::CAUSE_ECALL_S || cause_code == csr::CAUSE_ECALL_M {
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
            } else if mpp != 3 {
                let stvec = vm.cpu.csr.stvec & !0x3u32;
                if stvec != 0 {
                    vm.cpu.csr.sepc = vm.cpu.csr.mepc;
                    vm.cpu.csr.scause = mcause;
                    vm.cpu.csr.stval = vm.cpu.csr.mtval;
                    let spp = if mpp == 1 { 1u32 } else { 0u32 };
                    vm.cpu.csr.mstatus =
                        (vm.cpu.csr.mstatus & !(1 << csr::MSTATUS_SPP)) | (spp << csr::MSTATUS_SPP);
                    let sie = (vm.cpu.csr.mstatus >> csr::MSTATUS_SIE) & 1;
                    vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << csr::MSTATUS_SPIE))
                        | (sie << csr::MSTATUS_SPIE);
                    vm.cpu.csr.mstatus &= !(1 << csr::MSTATUS_SIE);
                    vm.cpu.pc = stvec;
                    vm.cpu.privilege = Privilege::Supervisor;
                    vm.cpu.tlb.flush_all();
                    count += 1;
                    continue;
                }
            }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }

        let pc_before = vm.cpu.pc;
        let _ = vm.step();
        count += 1;

        if vm.bus.write_watch_hit {
            let pte = vm.bus.read_word(0x1002C08).unwrap_or(0);
            let is_v = (pte & 1) != 0;
            let should_report = last_pte_v.is_some() && last_pte_v.unwrap() != is_v;

            if should_report {
                // Dump ALL registers
                eprintln!(
                    "[WATCH] count={} PC=0x{:08X} val=0x{:08X} V={}->{}",
                    count,
                    pc_before,
                    vm.bus.write_watch_val,
                    last_pte_v.unwrap(),
                    is_v
                );
                for i in 0..32 {
                    let name = match i {
                        0 => "zero",
                        1 => "ra",
                        2 => "sp",
                        3 => "gp",
                        4 => "tp",
                        5 => "t0",
                        6 => "t1",
                        7 => "t2",
                        8 => "s0",
                        9 => "s1",
                        10 => "a0",
                        11 => "a1",
                        12 => "a2",
                        13 => "a3",
                        14 => "a4",
                        15 => "a5",
                        16 => "a6",
                        17 => "a7",
                        18 => "s2",
                        19 => "s3",
                        20 => "s4",
                        21 => "s5",
                        22 => "s6",
                        23 => "s7",
                        24 => "s8",
                        25 => "s9",
                        26 => "s10",
                        27 => "s11",
                        28 => "t3",
                        29 => "t4",
                        30 => "t5",
                        31 => "t6",
                        _ => "?",
                    };
                    if vm.cpu.x[i] != 0 {
                        eprintln!("  x{}({}) = 0x{:08X}", i, name, vm.cpu.x[i]);
                    }
                }
                eprintln!("  priv={:?}", vm.cpu.privilege);

                // Translate the store address (t4 = x29)
                let store_va = vm.cpu.x[29]; // t4
                eprintln!("  Store VA (t4/x29): 0x{:08X}", store_va);

                // Manual page table walk
                let satp = vm.cpu.csr.read(csr::SATP);
                let root_ppn = satp & 0x3FFFFF;
                let root_addr = (root_ppn as u64) << 12;
                let vpn1 = ((store_va >> 22) & 0x3FF) as u64;
                let vpn0 = ((store_va >> 12) & 0x3FF) as u64;
                let l1_addr = root_addr + vpn1 * 4;
                let l1_pte = vm.bus.read_word(l1_addr).unwrap_or(0);
                eprintln!("  VA 0x{:X}: VPN1={} VPN0={}", store_va, vpn1, vpn0);
                eprintln!("  L1[{}] at PA 0x{:X} = 0x{:08X}", vpn1, l1_addr, l1_pte);

                if (l1_pte & 1) != 0 && (l1_pte & 0xE) != 0 {
                    let ppn_hi = ((l1_pte >> 20) & 0xFFF) as u64;
                    let pa = (ppn_hi << 22) | (vpn0 << 12);
                    eprintln!("  Megapage -> PA 0x{:X}", pa);
                }

                break;
            }
            last_pte_v = Some(is_v);
            vm.bus.write_watch_hit = false;
        }
    }
}
