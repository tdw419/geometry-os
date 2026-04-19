/// Diagnostic: dump the kernel's page table at SATP 0x80000802 and check
/// which L1 entries are populated. Also check what instruction the kernel
/// is actually executing (via PA computed from page table).
use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1";

    let (mut vm, fw_addr, _entry, _dtb_addr) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs).unwrap();

    let max = 5_000_000u64;
    let mut count: u64 = 0;
    let fw_addr_u32 = fw_addr as u32;
    let mut first_fault: Option<(u64, u32, u32)> = None; // (count, scause, stval)

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

        // Track S-mode faults
        let prev_scause = vm.cpu.csr.scause;
        let prev_pc = vm.cpu.pc;
        let _ = vm.step();

        // Check if an S-mode fault just happened (scause changed and we're at stvec)
        if vm.cpu.csr.scause != prev_scause && prev_scause == 0 {
            let cause = vm.cpu.csr.scause & 0xFF;
            if cause >= 12 && cause <= 15 && first_fault.is_none() {
                first_fault = Some((count, cause, vm.cpu.csr.stval));
                println!("[fault] First S-mode fault at count={}: scause={} stval=0x{:08X} sepc=0x{:08X} stvec=0x{:08X}",
                    count, cause, vm.cpu.csr.stval, vm.cpu.csr.sepc, vm.cpu.csr.stvec);
            }
        }

        count += 1;
    }

    // Dump L1 page table
    let satp_val = vm.cpu.csr.satp;
    let pt_root_ppn = (satp_val & 0x003F_FFFF) as u64;
    let pt_root_phys = pt_root_ppn * 4096;

    println!(
        "\n[pt] Page table at PA 0x{:08X} (SATP=0x{:08X})",
        pt_root_phys as u32, satp_val
    );
    println!("[pt] Non-zero L1 entries:");

    let mut populated = 0;
    for i in 0..1024u32 {
        let addr = pt_root_phys + (i as u64) * 4;
        if let Ok(pte) = vm.bus.read_word(addr) {
            if pte != 0 {
                let ppn = ((pte >> 10) & 0x003F_FFFF) as u32;
                let v = pte & 1;
                let r = (pte >> 1) & 1;
                let w = (pte >> 2) & 1;
                let x = (pte >> 3) & 1;
                let is_leaf = (pte & 0xE) != 0;
                let va_start = (i as u64) << 22;
                let va_end = va_start | 0x3FFFFF;
                println!("  L1[{:3}] VA 0x{:08X}-0x{:08X}: PTE=0x{:08X} V={} R={} W={} X={} PPN=0x{:05X} {}",
                    i, va_start as u32, va_end as u32, pte, v, r, w, x, ppn,
                    if is_leaf { "(megapage)" } else { "(L2 pointer)" });
                populated += 1;
            }
        }
    }
    println!("[pt] {} non-zero L1 entries out of 1024", populated);

    // Check the specific PC region
    println!("\n[kernel] Final state:");
    println!(
        "  PC=0x{:08X} priv={:?} SATP=0x{:08X}",
        vm.cpu.pc, vm.cpu.privilege, satp_val
    );
    println!(
        "  scause=0x{:08X} sepc=0x{:08X} stval=0x{:08X} stvec=0x{:08X}",
        vm.cpu.csr.scause, vm.cpu.csr.sepc, vm.cpu.csr.stval, vm.cpu.csr.stvec
    );
    println!("  ecall_count={}", vm.cpu.ecall_count);

    // Try to compute the PA for the current PC and read the instruction
    let pc = vm.cpu.pc as u64;
    let vpn1 = ((pc >> 22) & 0x3FF) as u64;
    let l1_addr = pt_root_phys + vpn1 * 4;
    if let Ok(l1_pte) = vm.bus.read_word(l1_addr) {
        let l1_ppn = ((l1_pte >> 10) & 0x003F_FFFF) as u64;
        let is_leaf = (l1_pte & 0xE) != 0;
        if is_leaf {
            let pa = (l1_ppn << 12) | (pc & 0x3FFFFF);
            if let Ok(word) = vm.bus.read_word(pa) {
                println!(
                    "  Instruction at PC: VA=0x{:08X} -> PA=0x{:08X} -> 0x{:08X}",
                    pc as u32, pa as u32, word
                );
            }
        } else {
            let vpn0 = ((pc >> 12) & 0x3FF) as u64;
            let l2_addr = (l1_ppn << 12) + vpn0 * 4;
            if let Ok(l2_pte) = vm.bus.read_word(l2_addr) {
                let l2_ppn = ((l2_pte >> 10) & 0x003F_FFFF) as u64;
                let pa = (l2_ppn << 12) | (pc & 0xFFF);
                if let Ok(word) = vm.bus.read_word(pa) {
                    println!("  Instruction at PC: VA=0x{:08X} -> L2 PA=0x{:08X} -> PA=0x{:08X} -> 0x{:08X}",
                        pc as u32, pa as u32, pa as u32, word);
                }
            } else {
                println!(
                    "  L2[{}] at PA 0x{:08X}: unreadable!",
                    vpn0 as u32, l2_addr as u32
                );
            }
        }
    }

    // Check CLINT
    println!(
        "\n[clint] mtime=0x{:016X} mtimecmp=0x{:016X}",
        vm.bus.clint.mtime, vm.bus.clint.mtimecmp
    );

    // UART and SBI
    println!(
        "[uart] tx_buf={} sbi_console={}",
        vm.bus.uart.tx_buf.len(),
        vm.bus.sbi.console_output.len()
    );
}
