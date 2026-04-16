use geometry_os::riscv::RiscvVm;
use geometry_os::riscv::cpu::{Privilege, StepResult};
use geometry_os::riscv::csr;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1 quiet";

    let (mut vm, fw_addr, _, _) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs).unwrap();

    let fw_addr_u32 = fw_addr as u32;
    let mut count: u64 = 0;
    let max_count: u64 = 5_000_000;
    let mut last_satp = vm.cpu.csr.satp;
    let mut ecall_count = 0u32;
    let mut last_progress = 0u64;
    let mut pc_drops: Vec<(u64, u32, u32)> = Vec::new(); // (count, old_pc, new_pc)

    while count < max_count {
        if vm.bus.sbi.shutdown_requested { break; }

        // Detect trap at fw_addr (M-mode trap handler)
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            let mpp = (vm.cpu.csr.mstatus & 0x300) >> 8;

            if cause_code == 9 { // ECALL_S
                ecall_count += 1;
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
            } else if mpp != 3 {
                // Forward to S-mode
                let stvec = vm.cpu.csr.stvec & !0x3u32;
                if stvec != 0 {
                    vm.cpu.csr.sepc = vm.cpu.csr.mepc;
                    vm.cpu.csr.scause = mcause;
                    vm.cpu.csr.stval = vm.cpu.csr.mtval;
                    let spp = if mpp == 1 { 1u32 } else { 0u32 };
                    vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << 5)) | (spp << 5);
                    let sie = (vm.cpu.csr.mstatus >> 1) & 1;
                    vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << 5)) | (sie << 5);
                    vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << 1)) | 0;
                    vm.cpu.pc = stvec;
                    vm.cpu.privilege = Privilege::Supervisor;
                }
            }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }

        // Watch for SATP changes - inject identity mappings for device regions
        if vm.cpu.csr.satp != last_satp {
            eprintln!("[SATP] count={}: 0x{:08X} -> 0x{:08X} PC=0x{:08X}",
                count, last_satp, vm.cpu.csr.satp, vm.cpu.pc);
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

        // Advance CLINT timer
        vm.bus.tick_clint();
        vm.bus.sync_mip(&mut vm.cpu.csr.mip);

        let old_pc = vm.cpu.pc;
        let step_result = vm.step();
        let new_pc = vm.cpu.pc;

        // Detect PC dropping to low address (< 0x100000)
        if old_pc >= 0xC0000000 && new_pc < 0x00100000 && new_pc > 0 {
            pc_drops.push((count, old_pc, new_pc));
            if pc_drops.len() <= 5 {
                eprintln!("[PC DROP] count={}: 0x{:08X} -> 0x{:08X} RA=0x{:08X} SP=0x{:08X}",
                    count, old_pc, new_pc, vm.cpu.x[1], vm.cpu.x[2]);
            }
        }

        count += 1;

        if count - last_progress >= 500_000 {
            eprintln!("Progress: count={} PC=0x{:08X} SP=0x{:08X} SATP=0x{:08X} ECALLs={}",
                count, vm.cpu.pc, vm.cpu.x[2], vm.cpu.csr.satp, ecall_count);
            last_progress = count;
        }
    }

    println!("=== DONE === count={} ECALLs={}", count, ecall_count);
    println!("PC=0x{:08X} SP=0x{:08X} RA=0x{:08X}", vm.cpu.pc, vm.cpu.x[2], vm.cpu.x[1]);
    println!("SATP=0x{:08X} STVEC=0x{:08X}", vm.cpu.csr.satp, vm.cpu.csr.stvec);
    println!("UART: {} chars", vm.bus.uart.tx_buf.len());
    if !vm.bus.uart.tx_buf.is_empty() {
        let s = String::from_utf8_lossy(&vm.bus.uart.tx_buf);
        println!("UART:\n{}", s);
    }
    println!("PC drops to low addr: {}", pc_drops.len());
}
