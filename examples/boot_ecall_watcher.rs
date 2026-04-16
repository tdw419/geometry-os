use geometry_os::riscv::RiscvVm;
use geometry_os::riscv::cpu::Privilege;
use geometry_os::riscv::csr;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();

    let (mut vm, fw_addr, _, _) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256,
            "console=ttyS0 earlycon=sbi panic=1 quiet").unwrap();
    let fw_addr_u32 = fw_addr as u32;

    let max_count: u64 = 300_000;
    let mut count: u64 = 0;
    let mut last_satp = vm.cpu.csr.satp;
    let mut reached_va = false;
    let mut va_first_count = 0u64;
    let mut last_progress = 0u64;

    while count < max_count {
        if vm.bus.sbi.shutdown_requested { break; }

        // M-mode trap handler
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            let mpp = (vm.cpu.csr.mstatus & csr::MSTATUS_MPP_MASK) >> csr::MSTATUS_MPP_LSB;
            if cause_code != csr::CAUSE_ECALL_M && mpp != 3 {
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
                for &l1_idx in &[0u32, 1, 2, 3, 4, 5, 8, 48, 64] {
                    let addr = pg_dir_phys + (l1_idx as u64) * 4;
                    let existing = vm.bus.read_word(addr).unwrap_or(0);
                    if (existing & 1) == 0 {
                        vm.bus.write_word(addr, 0x0000_00CF | (l1_idx << 20)).ok();
                    }
                }
                vm.cpu.tlb.flush_all();
            }
            last_satp = cur_satp;
        }

        vm.bus.tick_clint();
        vm.bus.sync_mip(&mut vm.cpu.csr.mip);

        // Track first time we enter kernel VA space
        if !reached_va && vm.cpu.pc >= 0xC0000000 && vm.cpu.privilege == Privilege::Supervisor {
            reached_va = true;
            va_first_count = count;
            eprintln!("[VA] First kernel VA at count={}, PC=0x{:08X}", count, vm.cpu.pc);
        }

        // Track ECALLs
        let prev_ecall = vm.cpu.ecall_count;

        let _ = vm.step();
        count += 1;

        if vm.cpu.ecall_count > prev_ecall {
            eprintln!("[ECALL@{}] count={} PC=0x{:08X} a7={} a0={}",
                count, vm.cpu.ecall_count, vm.cpu.pc, vm.cpu.x[17], vm.cpu.x[10]);
        }

        // Track UART
        let uart_len = vm.bus.uart.tx_buf.len() + vm.bus.sbi.console_output.len();
        if uart_len > 0 {
            let s = String::from_utf8_lossy(&vm.bus.sbi.console_output);
            eprintln!("[UART@{}] SBI={}: {}", count, vm.bus.sbi.console_output.len(),
                &s[..s.len().min(200)]);
            break;
        }

        if count - last_progress >= 50_000 {
            eprintln!("Progress: count={} PC=0x{:08X} priv={:?} SATP=0x{:08X} ecall={}",
                count, vm.cpu.pc, vm.cpu.privilege, vm.cpu.csr.satp, vm.cpu.ecall_count);
            last_progress = count;
        }
    }

    println!("=== DONE === count={} ecall={}", count, vm.cpu.ecall_count);
    if reached_va {
        println!("First VA entry at count={}", va_first_count);
    }
}
