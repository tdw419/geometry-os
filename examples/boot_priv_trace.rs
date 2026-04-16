/// Trace PC values between trampoline patch and first illegal instruction.

use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1";

    let (mut vm, fw_addr, _entry, _dtb) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs)
            .expect("boot setup failed");

    let fw_addr_u32 = fw_addr as u32;
    let mut count: u64 = 0;
    let max_instructions: u64 = 178000;
    let mut trampoline_patched = false;
    let mut last_satp: u32 = 0;
    let mut last_pc: u32 = 0;
    let mut first_user_pc: Option<(u64, u32)> = None;
    let mut trap_count: u64 = 0;

    while count < max_instructions {
        if vm.bus.sbi.shutdown_requested { break; }

        if !trampoline_patched
            && vm.cpu.pc == 0x10EE
            && vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Supervisor
            && vm.cpu.csr.satp == 0
        {
            let identity_pte: u32 = 0x0000_00EF;
            let l1_entries: &[u64] = &[0, 2, 4, 5, 6, 8, 10];
            let trampoline_phys = 0x0148_4000u64;
            let early_pg_dir_phys = 0x0080_2000u64;
            for &l1_idx in l1_entries {
                let pte = identity_pte | ((l1_idx as u32) << 20);
                let addr_offset = (l1_idx * 4) as u64;
                vm.bus.write_word(trampoline_phys + addr_offset, pte).ok();
                vm.bus.write_word(early_pg_dir_phys + addr_offset, pte).ok();
            }
            trampoline_patched = true;
        }

        let cur_satp = vm.cpu.csr.satp;
        last_satp = cur_satp;

        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            if cause_code != 11 {
                let mpp = (vm.cpu.csr.mstatus & 0x300) >> 8;
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
                        trap_count += 1;
                        if trap_count <= 3 {
                            eprintln!("[trap#{}] count={} mepc=0x{:08X} cause={} mpp={}",
                                trap_count, count, vm.cpu.csr.sepc, cause_code, mpp);
                        }
                        count += 1;
                        continue;
                    }
                }
                vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
            } else {
                let result = vm.bus.sbi.handle_ecall(
                    vm.cpu.x[17], vm.cpu.x[16], vm.cpu.x[10], vm.cpu.x[11],
                    vm.cpu.x[12], vm.cpu.x[13], vm.cpu.x[14], vm.cpu.x[15],
                    &mut vm.bus.uart, &mut vm.bus.clint,
                );
                if let Some((a0_val, a1_val)) = result {
                    vm.cpu.x[10] = a0_val;
                    vm.cpu.x[11] = a1_val;
                }
            }
        }

        // Log first time we see User mode
        if first_user_pc.is_none() && vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::User {
            first_user_pc = Some((count, vm.cpu.pc));
            eprintln!("[user] First User mode at count={}, PC=0x{:08X}", count, vm.cpu.pc);
            eprintln!("  x[10]=0x{:08X} x[11]=0x{:08X} x[1]=0x{:08X}", vm.cpu.x[10], vm.cpu.x[11], vm.cpu.x[1]);
        }

        // Log privilege transitions
        if vm.cpu.privilege != geometry_os::riscv::cpu::Privilege::Supervisor
            && count > 177200 && count < 177600
        {
            if count % 1000 == 0 {
                eprintln!("[priv] count={} priv={:?} PC=0x{:08X}", count, vm.cpu.privilege, vm.cpu.pc);
            }
        }

        // Log when PC is at a low address (< 0x10000)
        if count > 177200 && vm.cpu.pc < 0x10000 {
            eprintln!("[low] count={} priv={:?} PC=0x{:08X}", count, vm.cpu.privilege, vm.cpu.pc);
        }

        last_pc = vm.cpu.pc;
        let _ = vm.step();
        count += 1;
    }

    eprintln!("\nSummary:");
    eprintln!("  Trap forwards: {}", trap_count);
    eprintln!("  First User mode: {:?}", first_user_pc);
    eprintln!("  Final PC: 0x{:08X}", vm.cpu.pc);
    eprintln!("  Final priv: {:?}", vm.cpu.privilege);
    eprintln!("  SBI console: {} chars", vm.bus.sbi.console_output.len());
}
