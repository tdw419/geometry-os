/// Disassemble the instruction at 0xC08EFFD2 that jumps to 0.
fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1";

    use geometry_os::riscv::RiscvVm;
    use geometry_os::riscv::cpu::Privilege;
    use geometry_os::riscv::csr;
    use geometry_os::riscv::decode;

    let (mut vm, fw_addr, _entry, _dtb_addr) = RiscvVm::boot_linux_setup(
        &kernel_image, initramfs.as_deref(), 256, bootargs
    ).unwrap();
    let fw_addr_u32 = fw_addr as u32;

    // Disassemble the entire trap handler from 0xC08EFF1C to 0xC08EFFD4
    eprintln!("=== Full handler disassembly (0xC08EFF1C - 0xC08EFFD8) ===");
    let mut addr = 0xC08EFF1Cu64;
    while addr <= 0xC08EFFD8 {
        // Read the 32-bit word at aligned address
        let aligned = addr & !3;
        if let Ok(word) = vm.bus.read_word(aligned) {
            // Extract the halfword at the actual address
            let offset = (addr - aligned) as usize;
            let halfword = ((word >> (offset * 8)) & 0xFFFF) as u16;
            
            let is_compressed = decode::is_compressed(halfword);
            let (op, inst_len) = if is_compressed {
                (decode::decode_c(halfword), 2u32)
            } else {
                (decode::decode(word), 4u32)
            };
            
            let marker = if addr == 0xC08EFFD2 { " <<< JUMP TO ZERO" } else { "" };
            eprintln!("  0x{:08X}: halfword=0x{:04X} (compressed={}) len={} {:?}{}",
                addr, halfword, is_compressed, inst_len, op, marker);
            addr += inst_len as u64;
        } else {
            break;
        }
    }

    // Also disassemble as raw hex for reference
    eprintln!("\n=== Raw bytes 0xC08EFF1C - 0xC08EFFD8 ===");
    for addr in (0xC08EFF1C..=0xC08EFFD8).step_by(4) {
        if let Ok(word) = vm.bus.read_word(addr) {
            eprintln!("  0x{:08X}: 0x{:08X}", addr, word);
        }
    }

    // Run to the fault, dump registers just before the crash instruction
    eprintln!("\n=== Running to crash point ===");
    let mut count: u64 = 0;
    let max_instructions: u64 = 20_000_000;

    while count < max_instructions {
        if vm.bus.sbi.shutdown_requested {
            break;
        }

        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            if cause_code == csr::CAUSE_ECALL_S {
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
            } else if cause_code != csr::CAUSE_ECALL_M {
                let mpp = (vm.cpu.csr.mstatus & csr::MSTATUS_MPP_MASK) >> csr::MSTATUS_MPP_LSB;
                if mpp != 3 {
                    let stvec = vm.cpu.csr.stvec & !0x3u32;
                    if stvec != 0 {
                        vm.cpu.csr.sepc = vm.cpu.csr.mepc;
                        vm.cpu.csr.scause = mcause;
                        vm.cpu.csr.stval = vm.cpu.csr.mtval;
                        let spp = if mpp == 1 { 1u32 } else { 0u32 };
                        vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << csr::MSTATUS_SPP))
                            | (spp << csr::MSTATUS_SPP);
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
            } else {
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
            }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }

        use geometry_os::riscv::cpu::StepResult;
        let pc_before = vm.cpu.pc;
        let sr = vm.step();

        // Look for the specific crash
        if vm.cpu.pc == 0 && pc_before != 0 {
            eprintln!("\n*** CRASH: instruction at 0x{:08X} jumped to 0x00000000 ***", pc_before);
            // Disassemble the crashing instruction
            let aligned = pc_before as u64 & !3;
            if let Ok(word) = vm.bus.read_word(aligned) {
                let offset = (pc_before as u64 - aligned) as usize;
                let halfword = ((word >> (offset * 8)) & 0xFFFF) as u16;
                let is_c = decode::is_compressed(halfword);
                if is_c {
                    let op = decode::decode_c(halfword);
                    eprintln!("  Compressed: halfword=0x{:04X} -> {:?}", halfword, op);
                } else {
                    let op = decode::decode(word);
                    eprintln!("  32-bit: word=0x{:08X} -> {:?}", word, op);
                }
            }
            
            // Dump registers at crash point
            eprintln!("  Registers at crash:");
            eprintln!("    ra=0x{:08X} sp=0x{:08X} tp=0x{:08X}", vm.cpu.x[1], vm.cpu.x[2], vm.cpu.x[4]);
            eprintln!("    t0=0x{:08X} t1=0x{:08X} t2=0x{:08X}", vm.cpu.x[5], vm.cpu.x[6], vm.cpu.x[7]);
            eprintln!("    a0=0x{:08X} a1=0x{:08X} a2=0x{:08X}", vm.cpu.x[10], vm.cpu.x[11], vm.cpu.x[12]);
            eprintln!("    s0=0x{:08X} s1=0x{:08X}", vm.cpu.x[8], vm.cpu.x[9]);
            break;
        }

        count += 1;
        if count % 5_000_000 == 0 {
            eprintln!("[run] count={}M", count / 1_000_000);
        }
    }
}
