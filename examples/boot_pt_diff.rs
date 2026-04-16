use std::fs;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = fs::read(kernel_path).expect("kernel");
    let initramfs = fs::read(initramfs_path).ok();
    
    let bootargs = "console=ttyS0 earlycon=sbi panic=5 quiet";
    
    let (mut vm, fw_addr, _entry, _dtb_addr) = 
        geometry_os::riscv::RiscvVm::boot_linux_setup(
            &kernel_image, initramfs.as_deref(), 512, bootargs,
        ).unwrap();
    
    let fw_addr_u32 = fw_addr as u32;
    let mut count: u64 = 0;
    let max_instructions: u64 = 178_000u64;
    let mut state = 0; // 0=waiting for call, 1=in setup_vm, 2=done
    
    // Snapshot PT area before setup_vm
    let mut before_pt: Vec<u32> = vec![0; 4096]; // 0x802000, 4KB = 1024 entries
    let mut after_pt: Vec<u32> = vec![0; 4096];
    
    while count < max_instructions {
        if vm.bus.sbi.shutdown_requested { break; }
        
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            let mpp = (vm.cpu.csr.mstatus & 0x3000) >> 12;
            
            if cause_code == 11 || cause_code == 9 {
                let result = vm.bus.sbi.handle_ecall(
                    vm.cpu.x[17], vm.cpu.x[16],
                    vm.cpu.x[10], vm.cpu.x[11],
                    vm.cpu.x[12], vm.cpu.x[13],
                    vm.cpu.x[14], vm.cpu.x[15],
                    &mut vm.bus.uart, &mut vm.bus.clint,
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
                    count += 1;
                    continue;
                }
            }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }
        
        // Before setup_vm call
        if state == 0 && vm.cpu.pc == 0x10EA {
            state = 1;
            // Snapshot early_pg_dir
            for i in 0..1024 {
                let addr = 0x802000u64 + (i as u64) * 4;
                before_pt[i] = vm.bus.read_word(addr).unwrap_or(0);
            }
            eprintln!("[diag] Before setup_vm (count={}): non-zero words in early_pg_dir: {}", 
                count, before_pt.iter().filter(|&&x| x != 0).count());
        }
        
        // After setup_vm return
        if state == 1 && vm.cpu.pc == 0x10EE {
            state = 2;
            // Snapshot again
            for i in 0..1024 {
                let addr = 0x802000u64 + (i as u64) * 4;
                after_pt[i] = vm.bus.read_word(addr).unwrap_or(0);
            }
            let after_count = after_pt.iter().filter(|&&x| x != 0).count();
            eprintln!("[diag] After setup_vm (count={}): non-zero words in early_pg_dir: {}", count, after_count);
            
            // Show differences
            let mut changed = 0;
            for i in 0..1024 {
                if before_pt[i] != after_pt[i] {
                    changed += 1;
                    if true {
                        eprintln!("  [{}] 0x{:08X}: 0x{:08X} -> 0x{:08X}", 
                            i, 0x802000 + i*4, before_pt[i], after_pt[i]);
                    }
                }
            }
            eprintln!("[diag] Changed words: {}", changed);
            
            // Show all non-zero entries
            for i in 0..1024 {
                if after_pt[i] != 0 {
                    let v = after_pt[i];
                    let r = (v >> 1) & 1; let w = (v >> 2) & 1; let x = (v >> 3) & 1;
                    let ppn = v >> 10;
                    let leaf = (r|w|x) != 0;
                    if leaf {
                        eprintln!("  L1[{:3}] = 0x{:08X} MEGAPAGE VA=0x{:06X}xxx PA=0x{:06X}xxx R={}W={}X={}",
                            i, v, i, ppn, r, w, x);
                    } else {
                        eprintln!("  L1[{:3}] = 0x{:08X} -> L2 PA=0x{:08X}", i, v, (ppn as u64)<<12);
                    }
                }
            }
            break;
        }
        
        let step_result = vm.step();
        if matches!(step_result, geometry_os::riscv::cpu::StepResult::Ebreak) { break; }
        count += 1;
    }
}
