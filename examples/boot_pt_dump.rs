/// Diagnostic: check page table state after setup_vm and after relocate_enable_mmu.
/// Run until the kernel returns from relocate_enable_mmu (PC in 0xC00010FA range).

use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=uart8250,mmio,0x10000000 panic=1";

    let (mut vm, fw_addr, _entry, dtb_addr) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs).unwrap();

    let max = 1_000_000u64;
    let mut count: u64 = 0;
    let fw_addr_u32 = fw_addr as u32;
    let mut last_satp: u32 = vm.cpu.csr.satp;
    let mut satp_changes: u64 = 0;

    while count < max {
        if vm.bus.sbi.shutdown_requested { break; }

        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Machine {
            let cause_code = vm.cpu.csr.mcause & !(1u32 << 31);
            if cause_code == 9 || cause_code == 11 {
                let result = vm.bus.sbi.handle_ecall(
                    vm.cpu.x[17], vm.cpu.x[16],
                    vm.cpu.x[10], vm.cpu.x[11],
                    vm.cpu.x[12], vm.cpu.x[13],
                    vm.cpu.x[14], vm.cpu.x[15],
                    &mut vm.bus.uart, &mut vm.bus.clint,
                );
                if let Some((a0, a1)) = result {
                    vm.cpu.x[10] = a0; vm.cpu.x[11] = a1;
                }
                println!("[diag] ECALL at count={}: cause={} a7=0x{:X} PC=0x{:08X}",
                    count, cause_code, vm.cpu.x[17], vm.cpu.csr.mepc);
            } else {
                let mpp = (vm.cpu.csr.mstatus & 0x1800) >> 11;
                if mpp != 3 {
                    let stvec = vm.cpu.csr.stvec & !0x3u32;
                    if stvec != 0 {
                        vm.cpu.csr.sepc = vm.cpu.csr.mepc;
                        vm.cpu.csr.scause = vm.cpu.csr.mcause;
                        vm.cpu.csr.stval = vm.cpu.csr.mtval;
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
            satp_changes += 1;
            let ppn = cur_satp & 0x3FFFFF;
            let pg_dir_phys = (ppn as u64) * 4096;
            println!("[diag] SATP #{}: 0x{:08X} -> 0x{:08X} pg_dir=PA 0x{:08X} count={} PC=0x{:08X}",
                satp_changes, last_satp, cur_satp, pg_dir_phys, count, vm.cpu.pc);

            // Dump first 16 L1 entries of the new page table
            println!("[diag] Page table at PA 0x{:08X}:", pg_dir_phys);
            for i in 0..16 {
                let pte = vm.bus.read_word(pg_dir_phys + (i as u64) * 4).unwrap_or(0);
                if pte != 0 {
                    let v = (pte >> 0) & 1;
                    let r = (pte >> 1) & 1;
                    let w = (pte >> 2) & 1;
                    let x = (pte >> 3) & 1;
                    let ppn = (pte >> 10) & 0x3FFFFF;
                    println!("  L1[{:>3}] = 0x{:08X} V={} R={} W={} X={} PPN=0x{:05X} -> PA 0x{:08X}",
                        i, pte, v, r, w, x, ppn, (ppn as u64) << 12);
                }
            }

            last_satp = cur_satp;
        }

        let _ = vm.step();
        count += 1;
    }

    // Final state
    println!("\n[diag] Final: count={} PC=0x{:08X} priv={:?} SATP=0x{:08X}",
        count, vm.cpu.pc, vm.cpu.privilege, vm.cpu.csr.satp);
    println!("SIE={} STVEC=0x{:08X}", (vm.cpu.csr.mstatus >> 1) & 1, vm.cpu.csr.stvec);
    println!("UART: ier={} lcr={} lsr=0x{:02X} tx={}",
        vm.bus.uart.ier, vm.bus.uart.lcr, vm.bus.uart.lsr, vm.bus.uart.tx_buf.len());

    // Check current page table
    let cur_ppn = vm.cpu.csr.satp & 0x3FFFFF;
    let cur_pg_dir = (cur_ppn as u64) * 4096;
    println!("\n[diag] Current page table at PA 0x{:08X}:", cur_pg_dir);
    for i in (760..780).chain(0..8) {
        let pte = vm.bus.read_word(cur_pg_dir + (i as u64) * 4).unwrap_or(0);
        if pte != 0 {
            let v = (pte >> 0) & 1;
            let r = (pte >> 1) & 1;
            let w = (pte >> 2) & 1;
            let x = (pte >> 3) & 1;
            let ppn = (pte >> 10) & 0x3FFFFF;
            println!("  L1[{:>3}] = 0x{:08X} V={} R={} W={} X={} PPN=0x{:05X}",
                i, pte, v, r, w, x, ppn);
        }
    }
}
