fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1";

    use geometry_os::riscv::RiscvVm;
    use geometry_os::riscv::cpu::{Privilege, StepResult};
    use geometry_os::riscv::csr;

    let (mut vm, fw_addr, _entry, _dtb_addr) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs).unwrap();

    let fw_addr_u32 = fw_addr as u32;
    let max_instr = 20_000_000u64;
    let mut count: u64 = 0;
    let mut last_valid: Option<bool> = None;
    let mut change_count: u64 = 0;
    let check_interval: u64 = 50_000;

    while count < max_instr {
        // Check for SBI shutdown
        if vm.bus.sbi.shutdown_requested {
            break;
        }

        // Handle M-mode traps (OpenSBI emulation)
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            let mpp = (vm.cpu.csr.mstatus & csr::MSTATUS_MPP_MASK) >> csr::MSTATUS_MPP_LSB;

            if cause_code == csr::CAUSE_ECALL_S || cause_code == csr::CAUSE_ECALL_M {
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
            } else if mpp != 3 {
                // Forward S/U mode traps to S-mode
                let stvec = vm.cpu.csr.stvec & !0x3u32;
                if stvec != 0 {
                    vm.cpu.csr.sepc = vm.cpu.csr.mepc;
                    vm.cpu.csr.scause = mcause;
                    vm.cpu.csr.stval = vm.cpu.csr.mtval;
                    let spp = if mpp == 1 { 1u32 } else { 0u32 };
                    vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << csr::MSTATUS_SPP))
                        | (spp << csr::MSTATUS_SPP);
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
            // MRET will execute on next step
        }

        // Periodically check L1[770] PTE
        if count % check_interval == 0 {
            let satp = vm.cpu.csr.read(csr::SATP);
            if (satp >> 31) & 1 != 0 {
                let ppn = satp & 0x3FFFFF;
                let pt_base = (ppn as u64) << 12;
                let l1_addr = pt_base + (770u64) * 4;
                let l1_pte = vm.bus.read_word(l1_addr).unwrap_or(0);
                let is_valid = (l1_pte & 1) != 0;

                if last_valid.is_some() && last_valid.unwrap() != is_valid {
                    change_count += 1;
                    eprintln!("[PTE] count={} L1[770] changed: V={}->{} PTE=0x{:08X} PC=0x{:08X} priv={:?} sepc=0x{:08X} scause=0x{:08X}",
                        count, last_valid.unwrap(), is_valid, l1_pte,
                        vm.cpu.pc, vm.cpu.privilege,
                        vm.cpu.csr.sepc, vm.cpu.csr.scause);
                }
                last_valid = Some(is_valid);
            }
        }

        let _ = vm.step();
        count += 1;
    }

    // Final state
    let satp = vm.cpu.csr.read(csr::SATP);
    let ppn = satp & 0x3FFFFF;
    let pt_base = (ppn as u64) << 12;
    let l1_addr = pt_base + (770u64) * 4;
    let l1_pte = vm.bus.read_word(l1_addr).unwrap_or(0);
    eprintln!("[DONE] count={} PC=0x{:08X} priv={:?}", count, vm.cpu.pc, vm.cpu.privilege);
    eprintln!("[DONE] satp=0x{:08X} L1[770]=0x{:08X} V={} changes={}", satp, l1_pte, l1_pte & 1, change_count);
    eprintln!("[DONE] UART: {} chars", vm.bus.uart.tx_buf.len());
}
