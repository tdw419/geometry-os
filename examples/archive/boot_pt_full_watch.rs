fn main() {
    // Watch for writes to the page table area (0x1002000-0x1003000)
    // by hooking into the bus write path.
    // Since we can't easily hook bus writes, we'll snapshot the entire L1 table
    // frequently and report any changes.

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
    let max_instr = 17_000_000u64;
    let mut count: u64 = 0;

    // Snapshot L1 table every 500 instructions starting at 16.9M
    let start_watch: u64 = 16_900_000;
    let check_interval: u64 = 500;

    // Read initial snapshot
    let satp = vm.cpu.csr.read(csr::SATP);
    let ppn = satp & 0x3FFFFF;
    let pt_base = (ppn as u64) << 12;

    // Read all 1024 L1 entries
    let mut prev_snapshot = vec![0u32; 1024];
    for i in 0..1024 {
        prev_snapshot[i] = vm.bus.read_word(pt_base + (i as u64) * 4).unwrap_or(0);
    }

    let mut found = false;

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

        let _ = vm.step();
        count += 1;

        if count >= start_watch && count % check_interval == 0 {
            for i in 0..1024 {
                let addr = pt_base + (i as u64) * 4;
                let pte = vm.bus.read_word(addr).unwrap_or(0);
                if pte != prev_snapshot[i] {
                    eprintln!(
                        "[PT_CHANGE] count={} L1[{}] 0x{:08X}->0x{:08X} PC=0x{:08X} priv={:?}",
                        count, i, prev_snapshot[i], pte, vm.cpu.pc, vm.cpu.privilege
                    );
                    eprintln!(
                        "  a0=0x{:08X} a1=0x{:08X} a2=0x{:08X} sp=0x{:08X} ra=0x{:08X}",
                        vm.cpu.x[10], vm.cpu.x[11], vm.cpu.x[12], vm.cpu.x[2], vm.cpu.x[1]
                    );
                    prev_snapshot[i] = pte;
                    if i == 770 {
                        found = true;
                        break;
                    }
                }
            }
            if found {
                break;
            }
        }
    }

    if !found {
        eprintln!("L1[770] didn't change");
    }
}
