use geometry_os::riscv::cpu::{Privilege, StepResult};
use geometry_os::riscv::RiscvVm;
use std::fs;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = fs::read(kernel_path).expect("kernel");
    let initramfs = fs::read(initramfs_path).ok();

    let (mut vm, fw_addr, _entry, _dtb_addr) = RiscvVm::boot_linux_setup(
        &kernel_image,
        initramfs.as_deref(),
        256,
        "console=ttyS0 earlycon=sbi loglevel=8",
    ).unwrap();

    let fw_addr_u32 = fw_addr as u32;
    let mut count: u64 = 0;
    let mut last_pc: u32 = 0;
    let mut func_entries: Vec<(u64, u32, u32)> = Vec::new(); // (count, pc, ra)
    let mut seen_ras: std::collections::HashSet<u32> = std::collections::HashSet::new();

    while count < 300_000 {
        if vm.bus.sbi.shutdown_requested { break; }

        // Detect function entries: JAL/JALR instructions that set RA
        let pc = vm.cpu.pc;
        let inst = vm.bus.read_word(if pc >= 0xC0000000 { (pc - 0xC0000000) as u64 } else { pc as u64 }).unwrap_or(0);
        let opcode = inst & 0x7F;
        
        // JAL (opcode 0x6F) or JALR (opcode 0x67)
        if opcode == 0x6F || opcode == 0x67 {
            let ra = vm.cpu.x[1];
            let sp = vm.cpu.x[2];
            // Only log if RA looks like a kernel text address and SP looks like a stack
            if ra > 0xC0000000 && ra < 0xC2000000 && sp > 0xC1000000 && sp < 0xC2000000 {
                if !seen_ras.contains(&pc) {
                    seen_ras.insert(pc);
                    func_entries.push((count, pc, ra));
                    if func_entries.len() <= 50 {
                        eprintln!("[func] count={}: call PC=0x{:08X} RA=0x{:08X}", count, pc, ra);
                    }
                }
            }
        }

        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            eprintln!("[TRAP] count={} cause={} mepc=0x{:08X}", count, cause_code, vm.cpu.csr.mepc);
            
            if cause_code == 9 {
                let result = vm.bus.sbi.handle_ecall(
                    vm.cpu.x[17], vm.cpu.x[16], vm.cpu.x[10], vm.cpu.x[11],
                    vm.cpu.x[12], vm.cpu.x[13], vm.cpu.x[14], vm.cpu.x[15],
                    &mut vm.bus.uart, &mut vm.bus.clint,
                );
                if let Some((a0, a1)) = result { vm.cpu.x[10] = a0; vm.cpu.x[11] = a1; }
                count += 1;
                continue;
            } else if cause_code != 11 {
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
                        if cause_code == 7 { vm.bus.clint.mtimecmp = vm.bus.clint.mtime + 100_000; }
                        vm.cpu.pc = stvec;
                        vm.cpu.privilege = Privilege::Supervisor;
                        vm.cpu.tlb.flush_all();
                        count += 1;
                        continue;
                    }
                }
            }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }

        vm.bus.tick_clint();
        vm.bus.sync_mip(&mut vm.cpu.csr.mip);
        match vm.step() {
            StepResult::Ebreak => { eprintln!("[EBREAK]"); break; }
            _ => {}
        }
        count += 1;
    }
    
    eprintln!("[done] count={} funcs_seen={}", count, func_entries.len());
    eprintln!("SBI console: {} bytes", vm.bus.sbi.console_output.len());
}
