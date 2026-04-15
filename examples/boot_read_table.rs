fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1";

    use geometry_os::riscv::RiscvVm;
    use geometry_os::riscv::cpu::{StepResult, Privilege};
    use geometry_os::riscv::csr;

    let (mut vm, fw_addr, _, _) = 
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs).unwrap();
    let fw_addr_u32 = fw_addr as u32;

    let max_instructions = 17_200_000u64;
    let mut count = 0u64;
    let mut first_fault_at: Option<u64> = None;
    let mut handler_step = 0u64;

    while count < max_instructions {
        if vm.bus.sbi.shutdown_requested { break; }
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            if cause_code == csr::CAUSE_ECALL_S {
                let result = vm.bus.sbi.handle_ecall(vm.cpu.x[17], vm.cpu.x[16], vm.cpu.x[10], vm.cpu.x[11],
                    vm.cpu.x[12], vm.cpu.x[13], vm.cpu.x[14], vm.cpu.x[15],
                    &mut vm.bus.uart, &mut vm.bus.clint);
                if let Some((a0, a1)) = result { vm.cpu.x[10] = a0; vm.cpu.x[11] = a1; }
                vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
                count += 1; continue;
            } else if cause_code != csr::CAUSE_ECALL_M {
                let mpp = (vm.cpu.csr.mstatus & csr::MSTATUS_MPP_MASK) >> csr::MSTATUS_MPP_LSB;
                if mpp != 3 {
                    let stvec = vm.cpu.csr.stvec & !0x3u32;
                    if stvec != 0 {
                        vm.cpu.csr.sepc = vm.cpu.csr.mepc;
                        vm.cpu.csr.scause = mcause;
                        vm.cpu.csr.stval = vm.cpu.csr.mtval;
                        let spp = if mpp == 1 { 1u32 } else { 0u32 };
                        vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << csr::MSTATUS_SPP)) | (spp << csr::MSTATUS_SPP);
                        let sie = (vm.cpu.csr.mstatus >> csr::MSTATUS_SIE) & 1;
                        vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << csr::MSTATUS_SPIE)) | (sie << csr::MSTATUS_SPIE);
                        vm.cpu.csr.mstatus &= !(1 << csr::MSTATUS_SIE);
                        vm.cpu.pc = stvec;
                        vm.cpu.privilege = Privilege::Supervisor;
                        vm.cpu.tlb.flush_all();
                        count += 1; continue;
                    }
                }
                vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
                count += 1; continue;
            }
            let result = vm.bus.sbi.handle_ecall(vm.cpu.x[17], vm.cpu.x[16], vm.cpu.x[10], vm.cpu.x[11],
                vm.cpu.x[12], vm.cpu.x[13], vm.cpu.x[14], vm.cpu.x[15],
                &mut vm.bus.uart, &mut vm.bus.clint);
            if let Some((a0, a1)) = result { vm.cpu.x[10] = a0; vm.cpu.x[11] = a1; }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
            count += 1; continue;
        }

        let pc_before = vm.cpu.pc;
        let step_result = vm.step();
        
        if first_fault_at.is_none() && matches!(step_result, StepResult::FetchFault | StepResult::LoadFault | StepResult::StoreFault) {
            first_fault_at = Some(count);
            handler_step = 0;
            // The fault is at __memmove. Let me check the scause to understand which handler table entry is used.
            eprintln!("[FAULT] scause=0x{:X} (code={})", vm.cpu.csr.scause, vm.cpu.csr.scause & 0xFF);
            // scause=0xD = load page fault = code 13
            // The handler table is indexed by cause code
            // x5 will be computed as: table_base + (cause_code * entry_size)
        }
        
        if first_fault_at.is_some() {
            handler_step += 1;
            if handler_step == 64 {
                // At H64, LW x6, [x5+0] loads x6 from address in x5
                eprintln!("[H64] About to load x6 from addr x5=0x{:08X}", vm.cpu.x[5]);
            }
            if handler_step == 65 {
                eprintln!("[H65] x6=0x{:08X} (loaded from 0x{:08X})", vm.cpu.x[6], 0xC1400AE8u32);
                // Read the handler table entry at 0xC1400AE8
                // First, read via physical address. Need to figure out the physical mapping.
                // The table is in virtual address space. Let me read it through the bus using virtual addr.
                // Actually, I can read directly from bus if I know the physical addr.
                // For now, let me read the surrounding table entries
                eprintln!("\n--- Handler table around 0xC1400AE8 ---");
                // Read entries at x5 and nearby
                for offset in -16i32..=16 {
                    if offset % 4 != 0 { continue; }
                    let addr = 0xC1400AE8u64 + offset as i64 as u64;
                    match vm.bus.read_word(addr) {
                        Ok(val) => {
                            let marker = if offset == 0 { " <-- faulting entry" } else { "" };
                            eprintln!("  [0x{:08X}] = 0x{:08X}{}", (0xC1400AE8u32 as i32 + offset) as u32, val, marker);
                        }
                        Err(_) => eprintln!("  [0x{:08X}] = ERR", (0xC1400AE8u32 as i32 + offset) as u32),
                    }
                }
                
                // Also read from the original table start
                // x20 (scause) was 0x0D at H64. The table index is cause code.
                // From the disasm: H57: SLLI x5, x20, 2 (shift left by 2 = *4)
                // H58: AUIPC x6, 0xB1131 -> x6 = 0xC08EFFB8 + 0xB1131000 = ... 
                // Actually AUIPC adds upper immediate to PC.
                // 0xC08EFFB8 + 0xB1131000 = 0x17BA1EFB8... that overflows u32
                // Let me compute: 0xB1131 is the upper imm. Actually in the inst 0xB11317:
                // AUIPC rd=x6, imm = bits[31:12] = 0xB1131
                // x6 = PC + (imm << 12) = 0xC08EFFB8 + (0xB1131 << 12) = 0xC08EFFB8 + 0xB1131000
                // = 0xC08EFFB8 + 0xB1131000 = overflow in 32-bit
                // Actually: 0xC08EFFB8 + 0xB1131000 = 0x1_7BA1EFB8, truncated to 32 bits = 0x7BA1EFB8
                // Wait, let me recalculate. AUIPC imm is sign-extended 20-bit value.
                // inst = 0x0B11317 -> opcode = 0x17 (AUIPC)
                // rd = 6, imm[31:12] = 0x0B113
                // imm = 0x0B113 << 12 = 0x0B113000
                // x6 = 0xC08EFFB8 + 0x0B113000 = 0xCBA0EFB8
                // Hmm, that doesn't match x6 = 0xC1400FB8 from the trace.
                // Let me just look at what was actually loaded.
                eprintln!("\nActual x6 after AUIPC: 0xC1400FB8");
                eprintln!("Then ADDI x6, x6, -1284: 0xC1400FB8 + (-1284) = 0x{:08X}", 
                    0xC1400FB8u32.wrapping_add((-1284i32) as u32));
                eprintln!("Then ADD x5, x5, x6: table_base + offset");
                eprintln!("x5 before ADD: 0x{:08X} (= scause_code << 2 = 0x0D << 2 = 0x34)", 0x34u32);
                eprintln!("x6 before ADD: 0x{:08X}", 0xC1400FB8u32.wrapping_add((-1284i32) as u32));
                eprintln!("x5 after ADD: 0x{:08X}", 0x34u32.wrapping_add(0xC1400FB8u32.wrapping_add((-1284i32) as u32)));
                
                break;
            }
        }
        count += 1;
    }
}
