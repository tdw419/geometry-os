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
    let max_instructions: u64 = 500_000u64;
    let mut forward_count: u64 = 0;
    let mut prev_satap: u32 = 0;
    let mut mmu_enabled_at: u64 = 0;
    let mut last_interesting_pc: u32 = 0;
    let mut uart_output_count: u64 = 0;

    while count < max_instructions {
        if vm.bus.sbi.shutdown_requested {
            eprintln!("[diag] SHUTDOWN requested at count={}", count);
            break;
        }

        // Count UART output
        let uart_len = vm.bus.uart.tx_buf.len();
        if uart_len as u64 > uart_output_count {
            let new_bytes = &vm.bus.uart.tx_buf[uart_output_count as usize..];
            let s = String::from_utf8_lossy(new_bytes);
            for line in s.lines() {
                eprintln!("[UART] {}", line);
            }
            uart_output_count = uart_len as u64;
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
                    forward_count += 1;

                    if forward_count <= 10 {
                        eprintln!(
                            "[diag] Forward #{} at count={}: cause={} sepc=0x{:08X} stvec=0x{:08X}",
                            forward_count, count, cause_code, vm.cpu.csr.sepc, stvec
                        );
                    }

                    count += 1;
                    continue;
                }
            }

            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }

        // Track SATAP changes
        let cur_satap = vm.cpu.csr.satp;
        if cur_satap != prev_satap && cur_satap != 0 {
            let pt_root = ((cur_satap & 0x3FFFFF) as u64) << 12;
            eprintln!("[diag] SATAP changed at count={}: 0x{:08X} -> 0x{:08X} PC=0x{:08X} pt_root=0x{:08X}",
                count, prev_satap, cur_satap, vm.cpu.pc, pt_root);

            // Dump trampoline L1[0] and L1[768]
            if let Ok(l1_0) = vm.bus.read_word(0x01484000) {
                eprintln!("[diag] trampoline L1[0] = 0x{:08X}", l1_0);
            }
            if let Ok(l1_768) = vm.bus.read_word(0x01484C00) {
                eprintln!("[diag] trampoline L1[768] = 0x{:08X}", l1_768);
            }

            mmu_enabled_at = count;
            prev_satap = cur_satap;
        }

        // Log interesting PCs periodically
        if count % 100000 == 0 && count > 0 {
            eprintln!(
                "[diag] count={} PC=0x{:08X} priv={:?} satap=0x{:08X}",
                count, vm.cpu.pc, vm.cpu.privilege, vm.cpu.csr.satp
            );
        }

        let step_result = vm.step();
        if matches!(step_result, geometry_os::riscv::cpu::StepResult::Ebreak) {
            eprintln!("[diag] EBREAK at count={}", count);
            break;
        }

        count += 1;
    }

    // Print final UART output
    let remaining = &vm.bus.uart.tx_buf[uart_output_count as usize..];
    let s = String::from_utf8_lossy(remaining);
    for line in s.lines() {
        eprintln!("[UART] {}", line);
    }

    eprintln!(
        "[diag] Final: count={} PC=0x{:08X} priv={:?} satap=0x{:08X} forwards={}",
        count, vm.cpu.pc, vm.cpu.privilege, vm.cpu.csr.satp, forward_count
    );
    eprintln!("[diag] UART bytes: {}", vm.bus.uart.tx_buf.len());
}
