fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1";

    use geometry_os::riscv::cpu::{Privilege, StepResult};
    use geometry_os::riscv::csr;
    use geometry_os::riscv::RiscvVm;

    let (mut vm, fw_addr, _entry, _dtb_addr) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs).unwrap();

    let fw_addr_u32 = fw_addr as u32;
    let max_instr = 18_000_000u64;
    let mut count: u64 = 0;

    // The PTE we're watching is at physical 0x1002C08 (L1[770] of root at 0x1002000)
    // But we need to watch for writes to this physical address.
    // Since the bus doesn't have write hooks, we'll snapshot the PTE periodically
    // and when it changes, dump the last N instructions.

    // Strategy: snapshot every 1000 instructions around the change window (16.5M - 17.5M)
    let start_watch: u64 = 16_500_000;
    let watch_addr: u64 = 0x1002C08; // L1[770] physical address

    let mut last_pte: u32 = 0;
    let mut last_valid = true;
    let mut found = false;
    let mut prev_pcs: std::collections::VecDeque<u32> =
        std::collections::VecDeque::with_capacity(20);

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

        // Track PC history
        if count >= start_watch {
            prev_pcs.push_back(vm.cpu.pc);
            if prev_pcs.len() > 20 {
                prev_pcs.pop_front();
            }
        }

        let _ = vm.step();
        count += 1;

        // Check PTE every 1000 instructions in the watch window
        if count >= start_watch && count % 1000 == 0 {
            let satp = vm.cpu.csr.read(csr::SATP);
            if (satp >> 31) & 1 != 0 {
                let pte = vm.bus.read_word(watch_addr).unwrap_or(0);
                let is_valid = (pte & 1) != 0;
                if is_valid != last_valid {
                    eprintln!(
                        "[!] count={} PTE changed: 0x{:08X} -> 0x{:08X} V={} PC=0x{:08X}",
                        count, last_pte, pte, is_valid, vm.cpu.pc
                    );
                    eprintln!("[!] Last 20 PCs:");
                    for (i, pc) in prev_pcs.iter().enumerate() {
                        eprintln!("    [{}] 0x{:08X}", i, pc);
                    }
                    // Dump registers
                    eprintln!("[!] a0=0x{:08X} a1=0x{:08X} a2=0x{:08X} a3=0x{:08X} a4=0x{:08X} a5=0x{:08X}",
                        vm.cpu.x[10], vm.cpu.x[11], vm.cpu.x[12], vm.cpu.x[13], vm.cpu.x[14], vm.cpu.x[15]);
                    eprintln!("[!] sp=0x{:08X} ra=0x{:08X}", vm.cpu.x[2], vm.cpu.x[1]);
                    found = true;
                    break;
                }
                last_pte = pte;
                last_valid = is_valid;
            }
        }
    }

    if !found {
        eprintln!("PTE didn't change in watch window");
        let satp = vm.cpu.csr.read(csr::SATP);
        let pte = vm.bus.read_word(watch_addr).unwrap_or(0);
        eprintln!(
            "Final: satp=0x{:08X} L1[770]=0x{:08X} V={}",
            satp,
            pte,
            pte & 1
        );
    }
}
