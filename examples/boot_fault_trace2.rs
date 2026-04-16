/// Diagnostic: trace the exact moment the kernel faults.
/// Run until the first S-mode fault, then dump full state.

use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1";

    let (mut vm, fw_addr, _entry, _dtb_addr) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs).unwrap();

    let max = 200_000u64;
    let mut count: u64 = 0;
    let fw_addr_u32 = fw_addr as u32;
    let mut prev_pc: u32 = 0;
    let mut prev_prev_pc: u32 = 0;
    let mut last_satp: u32 = vm.cpu.csr.satp;
    let mut fault_count: u64 = 0;

    while count < max {
        if vm.bus.sbi.shutdown_requested {
            println!("[diag] Shutdown at count={}", count);
            break;
        }

        // Handle M-mode trap forwarding
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            if cause_code == 11 {
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
            } else {
                let mpp = (vm.cpu.csr.mstatus & 0x1800) >> 11;
                if mpp != 3 {
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
                        count += 1;
                        continue;
                    }
                }
            }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }

        let before_scause = vm.cpu.csr.scause;
        let before_pc = vm.cpu.pc;
        let _ = vm.step();

        // Detect fault: PC jumped to stvec
        if vm.cpu.csr.scause != before_scause && before_scause == 0 {
            fault_count += 1;
            let cause = vm.cpu.csr.scause & 0xFF;
            println!("[fault #{}] count={} scause={} sepc=0x{:08X} stval=0x{:08X} stvec=0x{:08X}",
                fault_count, count, cause, vm.cpu.csr.sepc, vm.cpu.csr.stval, vm.cpu.csr.stvec);

            if fault_count <= 5 {
                // Dump registers at fault time
                println!("  RA=0x{:08X} SP=0x{:08X} GP=0x{:08X} TP=0x{:08X}",
                    vm.cpu.x[1], vm.cpu.x[2], vm.cpu.x[3], vm.cpu.x[4]);
                println!("  A0=0x{:08X} A1=0x{:08X} A7=0x{:08X} S0=0x{:08X}",
                    vm.cpu.x[10], vm.cpu.x[11], vm.cpu.x[17], vm.cpu.x[8]);

                // Show call chain: prev_prev_pc -> prev_pc -> before_pc
                println!("  Call chain: 0x{:08X} -> 0x{:08X} -> 0x{:08X} (faulting)",
                    prev_prev_pc, prev_pc, before_pc);
            }
        }

        // Track SATP changes
        let cur_satp = vm.cpu.csr.satp;
        if cur_satp != last_satp {
            println!("[satp] count={} SATP: 0x{:08X} -> 0x{:08X} PC=0x{:08X}",
                count, last_satp, cur_satp, before_pc);
            last_satp = cur_satp;

            // Dump L1 entries of the new page table
            let pt_root_ppn = (cur_satp & 0x003F_FFFF) as u64;
            let pt_root_phys = pt_root_ppn * 4096;
            println!("[satp] Page table root PA=0x{:08X}", pt_root_phys as u32);
            for i in 0..1024u32 {
                let addr = pt_root_phys + (i as u64) * 4;
                if let Ok(pte) = vm.bus.read_word(addr) {
                    if pte != 0 {
                        let ppn = ((pte >> 10) & 0x003F_FFFF) as u32;
                        let is_leaf = (pte & 0xE) != 0;
                        let va_start = (i as u64) << 22;
                        println!("  L1[{:3}] VA 0x{:08X}: PTE=0x{:08X} PPN=0x{:05X} {}",
                            i, va_start as u32, pte, ppn,
                            if is_leaf { "(mega)" } else { "(L2)" });
                    }
                }
            }
        }

        prev_prev_pc = prev_pc;
        prev_pc = before_pc;
        count += 1;
    }

    println!("\n[final] count={} faults={}", count, fault_count);
    println!("[final] PC=0x{:08X} SATP=0x{:08X} ecall={}", vm.cpu.pc, vm.cpu.csr.satp, vm.cpu.ecall_count);
}
