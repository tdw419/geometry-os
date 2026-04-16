use std::fs;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = fs::read(kernel_path).expect("kernel");
    let initramfs = fs::read(initramfs_path).ok();
    
    let bootargs = "console=ttyS0 earlycon=sbi panic=5 quiet";
    
    let (mut vm, fw_addr, _entry, _dtb) = 
        geometry_os::riscv::RiscvVm::boot_linux_setup(
            &kernel_image, initramfs.as_deref(), 512, bootargs
        ).expect("setup failed");
    
    let fw_addr_u32 = fw_addr as u32;
    let mut count: u64 = 0;
    let max: u64 = 250_000;
    let mut last_satp: u32 = vm.cpu.csr.satp;
    
    while count < max {
        if vm.bus.sbi.shutdown_requested { break; }
        
        // Identity mapping injection on SATP change
        {
            let cur_satp = vm.cpu.csr.satp;
            if cur_satp != last_satp {
                eprintln!("[boot] SATP changed: 0x{:08X} -> 0x{:08X} at count={}", last_satp, cur_satp, count);
                let mode = (cur_satp >> 31) & 1;
                if mode == 1 {
                    let ppn = cur_satp & 0x3FFFFF;
                    let pg_dir_phys = (ppn as u64) * 4096;
                    let l1_0 = vm.bus.read_word(pg_dir_phys).unwrap_or(0);
                    let already = (l1_0 & 0xCF) == 0xCF && ((l1_0 >> 20) & 0xFFF) == 0;
                    if !already {
                        let identity_pte: u32 = 0x0000_00CF;
                        let l1_entries: &[u32] = &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10,
                            16, 32, 48, 64, 80, 96, 112, 127];
                        for &l1_idx in l1_entries {
                            let pte = identity_pte | (l1_idx << 20);
                            vm.bus.write_word(pg_dir_phys + (l1_idx as u64) * 4, pte).ok();
                        }
                        vm.cpu.tlb.flush_all();
                        eprintln!("[boot] Injected into pg_dir 0x{:08X}", pg_dir_phys);
                    }
                }
            }
            last_satp = cur_satp;
        }
        
        // Trap handling
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            eprintln!("[trap] count={} cause={} mepc=0x{:08X} mtval=0x{:08X}", 
                count, cause_code, vm.cpu.csr.mepc, vm.cpu.csr.mtval);
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
            }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }
        
        // Log every instruction from count 177000 to 178000
        if count >= 177000 && count <= 178500 {
            eprintln!("[{}] PC=0x{:08X} satp=0x{:08X} sp=0x{:08X}", 
                count, vm.cpu.pc, vm.cpu.csr.satp, vm.cpu.x[2]);
        }
        
        vm.step();
        count += 1;
    }
    
    eprintln!("\n[state] PC=0x{:08X} satp=0x{:08X}", vm.cpu.pc, vm.cpu.csr.satp);
}
