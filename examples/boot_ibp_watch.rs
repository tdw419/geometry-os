use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();

    let bootargs = "console=ttyS0 earlycon=sbi panic=5 quiet";
    let (mut vm, fw_addr, _entry, dtb_addr) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 128, bootargs).unwrap();

    let fw_addr_u32 = fw_addr as u32;
    let ibp_pa: u64 = 0x00C7A380;
    let mut count: u64 = 0;
    let max_count: u64 = 400_000;

    let mut last_satp: u32 = vm.cpu.csr.satp;

    while count < max_count {
        if vm.bus.sbi.shutdown_requested {
            break;
        }

        // DTB watchdog
        let dtb_va_expected = (dtb_addr.wrapping_add(0xC0000000)) as u32;
        if count % 100 == 0 {
            let cur_va = vm.bus.read_word(0x00801008).unwrap_or(0);
            if cur_va != dtb_va_expected {
                vm.bus.write_word(0x00801008, dtb_va_expected).ok();
                vm.bus.write_word(0x0080100C, dtb_addr as u32).ok();
            }
        }

        // SATP change handler
        let cur_satp = vm.cpu.csr.satp;
        if cur_satp != last_satp {
            last_satp = cur_satp;
        }

        // Check IBP value BEFORE step
        let ibp_before = vm.bus.read_word(ibp_pa).unwrap_or(0xDEAD);

        // Forward M-mode traps to S-mode
        let at_handler = vm.cpu.pc == fw_addr_u32
            && vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Machine;

        if at_handler {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            let mpp = (vm.cpu.csr.mstatus & 0x3000) >> 12;

            if cause_code == 11 {
                // ECALL_M = SBI call
                let result = vm.bus.sbi.handle_ecall(
                    vm.cpu.x[17],
                    vm.cpu.x[16],
                    vm.cpu.x[10],
                    vm.cpu.x[11],
                    &mut vm.bus.clint,
                );
                vm.cpu.x[10] = result.0;
                vm.cpu.x[11] = result.1;
                vm.cpu.pc = vm.cpu.csr.mepc;
                vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !0x1800)
                    | (((vm.cpu.csr.mstatus >> 3) & 0x3) << 11);
                count += 1;
                continue;
            }

            if mpp == 0 || mpp == 1 {
                let stvec = vm.cpu.csr.stvec;
                if stvec != 0 {
                    vm.cpu.csr.sepc = vm.cpu.csr.mepc;
                    vm.cpu.csr.scause = mcause;
                    vm.cpu.csr.stval = vm.cpu.csr.mtval;
                    vm.cpu.pc = stvec;
                    vm.cpu.privilege = geometry_os::riscv::cpu::Privilege::Supervisor;
                    vm.cpu.tlb.flush_all();
                    count += 1;
                    continue;
                }
            }
        }

        vm.bus.tick_clint_n(100);
        vm.bus.sync_interrupts();
        let _ = vm.step();
        count += 1;

        // Check IBP value AFTER step
        let ibp_after = vm.bus.read_word(ibp_pa).unwrap_or(0xDEAD);

        if ibp_before != ibp_after {
            eprintln!(
                "[watch] IBP changed at count={}: 0x{:08X} -> 0x{:08X}, PC=0x{:08X}",
                count, ibp_before, ibp_after, vm.cpu.pc
            );
        }

        // Detect panic
        if vm.bus.read_word(0x00C14000).unwrap_or(0) != 0 && count > 300_000 {
            // Check if we're at the panic handler
            break;
        }
    }

    let final_ibp = vm.bus.read_word(ibp_pa).unwrap_or(0);
    eprintln!("\n[final] initial_boot_params = 0x{:08X} (expected 0x{:08X})", final_ibp, dtb_addr as u32);
    eprintln!("[final] count = {}", count);
    eprintln!("[final] PC = 0x{:08X}", vm.cpu.pc);
}
