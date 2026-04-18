fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1";

    use geometry_os::riscv::cpu::Privilege;
    use geometry_os::riscv::csr;
    use geometry_os::riscv::RiscvVm;

    let (mut vm, fw_addr, _entry, _dtb_addr) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs).unwrap();

    let fw_addr_u32 = fw_addr as u32;
    let mut count: u64 = 0;
    let max_instr = 300_000u64;

    // Watch for the specific return address 0xC08E1EAC
    // When we see ra=0xC08E1EAC at the __memmove entry, log everything
    let fn_entry: u32 = 0xC08E5D58;
    let mut seen_entry = false;

    while count < max_instr {
        if vm.bus.sbi.shutdown_requested {
            break;
        }
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            let mpp = (vm.cpu.csr.mstatus & csr::MSTATUS_MPP_MASK) >> csr::MSTATUS_MPP_LSB;
            if cause_code == csr::CAUSE_ECALL_S || cause_code == csr::CAUSE_ECALL_M {
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
            } else if mpp != 3 {
                let stvec = vm.cpu.csr.stvec & !0x3u32;
                if stvec != 0 {
                    vm.cpu.csr.sepc = vm.cpu.csr.mepc;
                    vm.cpu.csr.scause = mcause;
                    vm.cpu.csr.stval = vm.cpu.csr.mtval;
                    let spp = if mpp == 1 { 1u32 } else { 0u32 };
                    vm.cpu.csr.mstatus =
                        (vm.cpu.csr.mstatus & !(1 << csr::MSTATUS_SPP)) | (spp << csr::MSTATUS_SPP);
                    let sie = (vm.cpu.csr.mstatus >> csr::MSTATUS_SIE) & 1;
                    vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << csr::MSTATUS_SPIE))
                        | (sie << csr::MSTATUS_SPIE);
                    vm.cpu.csr.mstatus &= !(1 << csr::MSTATUS_SIE);
                    vm.cpu.pc = stvec;
                    vm.cpu.privilege = Privilege::Supervisor;
                    vm.cpu.tlb.flush_all();
                    count += 1;
                    continue;
                }
            }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }

        // Detect __memmove entry with a0=0xC1CCA520
        if vm.cpu.pc == fn_entry && vm.cpu.x[10] == 0xC1CCA520 && !seen_entry {
            seen_entry = true;
            eprintln!("[MEMMOVE_ENTRY] count={} PC=0x{:08X}", count, vm.cpu.pc);
            eprintln!(
                "  a0=0x{:08X} a1=0x{:08X} a2=0x{:08X}",
                vm.cpu.x[10], vm.cpu.x[11], vm.cpu.x[12]
            );
            eprintln!(
                "  t3=0x{:08X} t4=0x{:08X} t5=0x{:08X} t6=0x{:08X}",
                vm.cpu.x[28], vm.cpu.x[29], vm.cpu.x[30], vm.cpu.x[31]
            );

            // Single-step and log every instruction until we reach the loop
            for _ in 0..200 {
                let pc = vm.cpu.pc;
                let step_result = vm.step();
                count += 1;

                // Decode the instruction
                let word = match vm.bus.read_word(pc as u64) {
                    Ok(w) => w,
                    Err(_) => 0,
                };
                let is_compressed = (pc & 3) != 3; // approximate
                eprintln!(
                    "  0x{:08X}: 0x{:08X} t3=0x{:08X} t5=0x{:08X} {:?}",
                    pc, word, vm.cpu.x[28], vm.cpu.x[30], step_result
                );

                if vm.cpu.pc == 0xC08E5D6A {
                    eprintln!("  [AT LOOP] t5=0x{:08X}", vm.cpu.x[30]);
                    break;
                }
            }
            break;
        }

        let _ = vm.step();
        count += 1;
    }

    if !seen_entry {
        eprintln!(
            "Never entered __memmove with a0=0xC1CCA520 in {} instructions",
            count
        );
    }
}
