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
    .expect("setup");

    let fw_addr_u32 = fw_addr as u32;
    let mut trampoline_patched = false;
    let mut last_satp: u32 = 0;
    let mut count: u64 = 0;
    let mut trap_count: u64 = 0;

    // Phase 1: Run to Phase 1 injection point, then trace every instruction
    let max = 300_000u64;
    let mut fine_trace = false;
    let mut fine_start: u64 = 0;

    while count < max {
        if vm.bus.sbi.shutdown_requested {
            break;
        }

        // Identity mapping injection
        {
            let cur_satp = vm.cpu.csr.satp;
            if !trampoline_patched
                && vm.cpu.pc == 0x10EE
                && vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Supervisor
                && cur_satp == 0
            {
                let identity_pte: u32 = 0x0000_00CF;
                let l1_entries: &[u32] = &[0, 2, 4, 5, 6, 8, 10];
                for &idx in l1_entries {
                    let pte = identity_pte | (idx << 20);
                    vm.bus
                        .write_word(0x0148_4000u64 + (idx * 4) as u64, pte)
                        .ok();
                    vm.bus
                        .write_word(0x0080_2000u64 + (idx * 4) as u64, pte)
                        .ok();
                }
                trampoline_patched = true;
                eprintln!("[boot] Phase 1 at count={}", count);
                fine_trace = true;
                fine_start = count;
            }
            if trampoline_patched && cur_satp != last_satp && cur_satp != 0 {
                eprintln!(
                    "[boot] SATP: 0x{:08X} -> 0x{:08X} at count={}",
                    last_satp, cur_satp, count
                );
                let ppn = cur_satp & 0x3FFFFF;
                let pg_dir = (ppn as u64) * 4096;
                let l1_0 = vm.bus.read_word(pg_dir).unwrap_or(0);
                if (l1_0 & 0xCF) != 0xCF {
                    let identity_pte: u32 = 0x0000_00CF;
                    let l1_entries: &[u32] = &[0, 2, 4, 5, 6, 8, 10];
                    for &idx in l1_entries {
                        let pte = identity_pte | (idx << 20);
                        vm.bus.write_word(pg_dir + (idx * 4) as u64, pte).ok();
                    }
                    vm.cpu.tlb.flush_all();
                }
            }
            last_satp = cur_satp;
        }

        // M-mode trap handler
        if vm.cpu.pc == fw_addr_u32
            && vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Machine
        {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            trap_count += 1;
            if trap_count <= 10 {
                let mpp = (vm.cpu.csr.mstatus >> 11) & 3;
                eprintln!(
                    "[TRAP] #{} cause={} mepc=0x{:08X} mpp={} at count={}",
                    trap_count, cause_code, vm.cpu.csr.mepc, mpp, count
                );
            }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }

        // Fine trace: log every instruction after Phase 1
        if fine_trace {
            let rel = count - fine_start;
            if rel < 500 || (rel < 5000 && rel % 10 == 0) || (rel < 50000 && rel % 100 == 0) {
                let pc = vm.cpu.pc;
                let satp = vm.cpu.csr.satp;
                let prv = match vm.cpu.privilege {
                    geometry_os::riscv::cpu::Privilege::Machine => "M",
                    geometry_os::riscv::cpu::Privilege::Supervisor => "S",
                    geometry_os::riscv::cpu::Privilege::User => "U",
                };
                eprintln!(
                    "[{:>6}] PC=0x{:08X} satp=0x{:08X} priv={}",
                    rel, pc, satp, prv
                );
            }
            // Stop fine trace once we see PC go below 0x1000 with SATP active
            if vm.cpu.csr.satp != 0 && vm.cpu.pc < 0x1000 && count > fine_start + 100 {
                let inst = vm.bus.read_word(vm.cpu.pc as u64).unwrap_or(0);
                eprintln!(
                    "[!] LOW PC at +{}: PC=0x{:08X} inst=0x{:08X}",
                    count - fine_start,
                    vm.cpu.pc,
                    inst
                );
                // Log a few more instructions
                for _ in 0..20 {
                    let r = vm.step();
                    count += 1;
                    let inst2 = vm.bus.read_word(vm.cpu.pc as u64).unwrap_or(0);
                    eprintln!("[!]   -> PC=0x{:08X} inst=0x{:08X}", vm.cpu.pc, inst2);
                    if matches!(r, geometry_os::riscv::cpu::StepResult::Ebreak) {
                        break;
                    }
                }
                break;
            }
        }

        let result = vm.step();
        if matches!(result, geometry_os::riscv::cpu::StepResult::Ebreak) {
            break;
        }
        count += 1;
    }

    eprintln!("\nTotal: {} instructions, {} traps", count, trap_count);
    eprintln!("CPU ecall_count: {}", vm.cpu.ecall_count);
    eprintln!(
        "Final PC: 0x{:08X} SATP: 0x{:08X}",
        vm.cpu.pc, vm.cpu.csr.satp
    );
}
