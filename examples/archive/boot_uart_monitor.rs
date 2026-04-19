use geometry_os::riscv::cpu::Privilege;
use geometry_os::riscv::csr;
use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();

    let (mut vm, fw_addr, _, _) = RiscvVm::boot_linux_setup(
        &kernel_image,
        initramfs.as_deref(),
        256,
        "console=ttyS0 earlycon=sbi panic=1 quiet",
    )
    .unwrap();
    let fw_addr_u32 = fw_addr as u32;

    let max_count: u64 = 5_000_000;
    let mut count: u64 = 0;
    let mut last_satp = vm.cpu.csr.satp;
    let mut last_uart_len = 0usize;
    let mut last_progress = 0u64;
    let mut trap_count = 0u32;

    while count < max_count {
        if vm.bus.sbi.shutdown_requested {
            break;
        }

        // M-mode trap handler (fw_addr)
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            trap_count += 1;
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            let mpp = (vm.cpu.csr.mstatus & csr::MSTATUS_MPP_MASK) >> csr::MSTATUS_MPP_LSB;

            if cause_code == csr::CAUSE_ECALL_M {
                // ECALL_M: handled inline by step(), shouldn't reach here
            } else if mpp != 3 {
                // Forward S-mode/U-mode traps to S-mode handler
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

                    // Timer interrupt: advance mtimecmp
                    if cause_code == csr::INT_MTI {
                        vm.bus.clint.mtimecmp = vm.bus.clint.mtime + 100_000;
                    }

                    vm.cpu.pc = stvec;
                    vm.cpu.privilege = Privilege::Supervisor;
                }
            }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }

        // Inject identity mappings on SATP change
        if vm.cpu.csr.satp != last_satp {
            let cur_satp = vm.cpu.csr.satp;
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
            }
            last_satp = cur_satp;
        }

        vm.bus.tick_clint();
        vm.bus.sync_mip(&mut vm.cpu.csr.mip);

        // Check UART output changes
        let uart_len = vm.bus.uart.tx_buf.len() + vm.bus.sbi.console_output.len();
        if uart_len > last_uart_len {
            let sbi_len = vm.bus.sbi.console_output.len();
            let new_data: Vec<u8> = if sbi_len > last_uart_len {
                vm.bus.sbi.console_output[last_uart_len..].to_vec()
            } else {
                vm.bus.uart.tx_buf[last_uart_len.saturating_sub(sbi_len)..].to_vec()
            };
            let s = String::from_utf8_lossy(&new_data);
            let preview: String = s.chars().take(500).collect();
            eprintln!(
                "[UART@{}] (sbi={} uart={}) {}",
                count,
                sbi_len,
                vm.bus.uart.tx_buf.len(),
                preview
            );
            last_uart_len = uart_len;
        }

        let _ = vm.step();
        count += 1;

        if count - last_progress >= 500_000 {
            eprintln!(
                "Progress: count={} PC=0x{:08X} UART={} SBI={} traps={}",
                count,
                vm.cpu.pc,
                vm.bus.uart.tx_buf.len(),
                vm.bus.sbi.console_output.len(),
                trap_count
            );
            last_progress = count;
        }
    }

    println!(
        "=== DONE === count={} ecall_count={}",
        count, vm.cpu.ecall_count
    );
    println!("UART tx_buf: {} chars", vm.bus.uart.tx_buf.len());
    println!("SBI console: {} chars", vm.bus.sbi.console_output.len());
    if !vm.bus.sbi.console_output.is_empty() {
        let s = String::from_utf8_lossy(&vm.bus.sbi.console_output);
        println!("SBI output:\n{}", &s[..s.len().min(5000)]);
    }
    if !vm.bus.uart.tx_buf.is_empty() {
        let s = String::from_utf8_lossy(&vm.bus.uart.tx_buf);
        println!("UART output:\n{}", &s[..s.len().min(5000)]);
    }
}
