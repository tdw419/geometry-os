/// Diagnostic: check what's on the stack at the point where the kernel
/// reads the corrupted RA. The fault happens at count=178510, PC=0xC003F9CA
/// which does "lw ra, 92(sp)". SP=0xC1401E00.
/// Check what's at PA = 0x01401E5C (SP+92 translated through page table).
use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1";

    let (mut vm, fw_addr, _entry, _dtb_addr) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs).unwrap();

    let max = 180_000u64;
    let mut count: u64 = 0;
    let fw_addr_u32 = fw_addr as u32;

    while count < max {
        if vm.bus.sbi.shutdown_requested {
            println!("[diag] Shutdown at count={}", count);
            break;
        }

        // Handle M-mode trap forwarding
        if vm.cpu.pc == fw_addr_u32
            && vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Machine
        {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            if cause_code == 11 {
                let result = vm.bus.sbi.handle_ecall(
                    vm.cpu.x[17],
                    vm.cpu.x[16],
                    vm.cpu.x[10],
                    vm.cpu.x[11],
                    vm.cpu.x[12],
                    vm.cpu.x[13],
                    vm.cpu.x[14],
                    vm.cpu.x[15],
                    &mut vm.bus.uart,
                    &mut vm.bus.clint,
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

        // Stop just before the fault (count=178510 is the fault)
        // The instruction at 0xC003F9CA is "lw ra, 92(sp)"
        // Stop a few instructions before to check state
        if count == 178508 {
            println!(
                "[pre-fault] count={} PC=0x{:08X} SP=0x{:08X}",
                count, vm.cpu.pc, vm.cpu.x[2]
            );

            // Compute PA for SP
            let sp = vm.cpu.x[2] as u64;
            let sp_pa = sp - 0xC0000000 as u64; // L1[772] maps VA 0xC1000000 -> PA 0x01000000
            println!(
                "[pre-fault] SP VA=0x{:08X} -> PA=0x{:08X}",
                sp as u32, sp_pa as u32
            );

            // Read stack contents around SP+92 (where RA will be loaded from)
            for offset in 0..128 {
                let va = sp + offset as u64;
                let pa = va - 0xC0000000 as u64;
                if let Ok(word) = vm.bus.read_word(pa) {
                    if word != 0 {
                        println!(
                            "  [SP+{}] VA=0x{:08X} PA=0x{:08X}: 0x{:08X}",
                            offset, va as u32, pa as u32, word
                        );
                    }
                }
            }
        }

        let _ = vm.step();
        count += 1;
    }

    println!(
        "\n[post-fault] count={} PC=0x{:08X} SP=0x{:08X}",
        count, vm.cpu.pc, vm.cpu.x[2]
    );
    println!(
        "[post-fault] RA=0x{:08X} scause=0x{:08X}",
        vm.cpu.x[1], vm.cpu.csr.scause
    );

    // Also check: what is the kernel expecting RA to be?
    // prb_reserve is called from printk/vprintk. The caller should have saved RA on the stack.
    // If the stack was set up correctly, SP+92 should contain a valid kernel VA.
    // Let's check what the expected return address should be by looking at the caller.
    println!("\n[kernel] To find caller of prb_reserve:");
    println!("  riscv64-linux-gnu-objdump -d -m riscv:rv32imac .geometry_os/build/linux-6.14/vmlinux | grep -B5 'prb_reserve'");
}
