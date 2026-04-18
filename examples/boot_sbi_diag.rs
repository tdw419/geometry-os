use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();

    let (mut vm, _, _, _) = RiscvVm::boot_linux_setup(
        &kernel_image,
        initramfs.as_deref(),
        256,
        "console=ttyS0 loglevel=8 earlycon=sbi",
    )
    .unwrap();

    let max = 500_000u64;
    let mut count = 0u64;
    let mut scause_hist: std::collections::HashMap<u32, u64> = std::collections::HashMap::new();
    let mut sbi_calls = 0u64;
    let mut last_ecall_info = String::new();

    // Track ECALL instructions to see if kernel makes any SBI calls
    // Also track what happens around instruction page faults

    while count < max {
        vm.bus.tick_clint();
        vm.bus.sync_mip(&mut vm.cpu.csr.mip);

        let pc_before = vm.cpu.pc;
        let priv_before = vm.cpu.privilege;
        let result = vm.step();

        // Count scause values when in S-mode
        if vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Supervisor {
            *scause_hist.entry(vm.cpu.csr.scause).or_insert(0) += 1;
        }

        // Log ECALL instructions
        if matches!(result, geometry_os::riscv::cpu::StepResult::Ecall) {
            if sbi_calls < 20 {
                eprintln!(
                    "[{}] ECALL at PC=0x{:08X} priv={:?} a7=0x{:08X} a0=0x{:08X} a1=0x{:08X}",
                    count, pc_before, priv_before, vm.cpu.x[17], vm.cpu.x[10], vm.cpu.x[11]
                );
            }
            sbi_calls += 1;
        }

        count += 1;

        // Sample around interesting boundaries
        if count == 200_000 || count == 300_000 || count == 400_000 {
            eprintln!(
                "[{}] Sample: PC=0x{:08X} priv={:?} scause=0x{:08X}",
                count, vm.cpu.pc, vm.cpu.privilege, vm.cpu.csr.scause
            );
        }
    }

    eprintln!(
        "
=== Summary ==="
    );
    eprintln!("SBI/ECALL calls: {}", sbi_calls);
    eprintln!(
        "UART: {} bytes, SBI console: {} bytes",
        vm.bus.uart.tx_buf.len(),
        vm.bus.sbi.console_output.len()
    );
    eprintln!(
        "
scause histogram:"
    );
    let mut causes: Vec<_> = scause_hist.iter().collect();
    causes.sort_by_key(|(_, &v)| std::cmp::Reverse(v));
    for (cause, cnt) in causes.iter().take(10) {
        let name = match *cause {
            0 => "none",
            2 => "illegal_inst",
            3 => "breakpoint",
            5 => "load_access",
            12 => "inst_page_fault",
            13 => "load_page_fault",
            15 => "store_page_fault",
            0x80000000 => "s_soft_intr",
            0x80000005 => "s_timer_intr",
            0x80000007 => "s_ext_intr",
            _ => "other",
        };
        eprintln!("  scause=0x{:08X} ({}) count={}", cause, name, cnt);
    }
}
