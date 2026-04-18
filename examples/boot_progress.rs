use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();

    let bootargs = "console=ttyS0 earlycon=sbi panic=5 quiet";
    
    let (mut vm, _fw, _entry, _dtb) = RiscvVm::boot_linux_setup(
        &kernel_image, initramfs.as_deref(), 256, bootargs,
    ).unwrap();

    // Minimal step loop with progress tracking
    let mut count: u64 = 0;
    let max: u64 = 15_000_000;
    let mut last_satp: u32 = vm.cpu.csr.satp;
    
    while count < max {
        if vm.bus.sbi.shutdown_requested { break; }
        
        // Handle M-mode traps (same as boot_linux)
        let fw = vm.bus.read_word(0x0157C000).unwrap_or(0x30200073);
        let fw_u32 = 0x0157C000u32;
        
        if vm.cpu.pc == fw_u32 && vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cc = mcause & !(1u32 << 31);
            if cc == 9 || cc == 11 {
                let r = vm.bus.sbi.handle_ecall(
                    vm.cpu.x[17], vm.cpu.x[16], vm.cpu.x[10], vm.cpu.x[11],
                    vm.cpu.x[12], vm.cpu.x[13], vm.cpu.x[14], vm.cpu.x[15],
                    &mut vm.bus.uart, &mut vm.bus.clint,
                );
                if let Some((a0, a1)) = r {
                    vm.cpu.x[10] = a0;
                    vm.cpu.x[11] = a1;
                }
                eprintln!(
                    "[prog] SBI at count={}: a7={} a6={} a0={}",
                    count, vm.cpu.x[17], vm.cpu.x[16], vm.cpu.x[10]
                );
            } else {
                let mpp = (vm.cpu.csr.mstatus >> 11) & 3;
                if mpp != 3 {
                    let stvec = vm.cpu.csr.stvec & !0x3u32;
                    if stvec != 0 {
                        vm.cpu.csr.sepc = vm.cpu.csr.mepc;
                        vm.cpu.csr.scause = mcause;
                        vm.cpu.csr.stval = vm.cpu.csr.mtval;
                        let spp = if mpp == 1 { 1u32 } else { 0u32 };
                        vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << 8)) | (spp << 8);
                        let sie = (vm.cpu.csr.mstatus >> 1) & 1;
                        vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << 5)) | (sie << 5);
                        vm.cpu.csr.mstatus &= !(1 << 1);
                        if cc == 7 { vm.bus.clint.mtimecmp = vm.bus.clint.mtime + 100_000; }
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

        let cur_satp = vm.cpu.csr.satp;
        if cur_satp != last_satp {
            eprintln!("[prog] SATP: 0x{:08X} -> 0x{:08X} at count={}", last_satp, cur_satp, count);
            last_satp = cur_satp;
        }

        vm.bus.tick_clint();
        vm.bus.sync_mip(&mut vm.cpu.csr.mip);
        let sr = vm.step();
        
        if let geometry_os::riscv::cpu::StepResult::Ebreak = sr { break; }
        
        count += 1;
        if count % 1_000_000 == 0 {
            let cur_priv = vm.cpu.privilege;
            let pc = vm.cpu.pc;
            let mtime = vm.bus.clint.mtime;
            let mtimecmp = vm.bus.clint.mtimecmp;
            let mip = vm.cpu.csr.mip;
            let sie = (vm.cpu.csr.mstatus >> 1) & 1;
            let stie = (vm.cpu.csr.mie >> 5) & 1;
            eprintln!("[prog] {}M: PC=0x{:08X} priv={:?} mtime={} mtimecmp={} mip=0x{:X} SIE={} STIE={}",
                count / 1_000_000, pc, cur_priv, mtime, mtimecmp, mip, sie, stie);
        }
    }

    eprintln!("[prog] Done at count={}", count);
    let uart = vm.bus.uart.drain_tx();
    if !uart.is_empty() {
        eprintln!("[prog] UART ({}): {}", uart.len(), String::from_utf8_lossy(&uart));
    } else {
        eprintln!("[prog] No UART");
    }
}
