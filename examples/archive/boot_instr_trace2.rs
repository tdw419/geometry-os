use geometry_os::riscv::cpu::Privilege;
use geometry_os::riscv::decode;
/// Trace the last 100 instructions before count 500K to see what the kernel is doing.
use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1";

    let (mut vm, fw_addr, _entry, _dtb_addr) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs).unwrap();

    let max = 500_000u64;
    let mut count: u64 = 0;
    let fw_addr_u32 = fw_addr as u32;
    let mut last_satp: u32 = vm.cpu.csr.satp;
    let mut last_pc: u32 = 0;
    let mut same_pc: u64 = 0;
    let mut unique_pcs: std::collections::HashSet<u32> = std::collections::HashSet::new();

    while count < max {
        if vm.bus.sbi.shutdown_requested {
            break;
        }

        // Handle M-mode trap forwarding
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
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
                        vm.cpu.privilege = Privilege::Supervisor;
                        vm.cpu.tlb.flush_all();
                        count += 1;
                        continue;
                    }
                }
            }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }

        vm.step();

        if vm.cpu.csr.satp != last_satp {
            println!(
                "[trace] SATP: 0x{:08X} -> 0x{:08X} at count={}",
                last_satp, vm.cpu.csr.satp, count
            );
            last_satp = vm.cpu.csr.satp;
        }

        // Track unique PCs
        unique_pcs.insert(vm.cpu.pc);

        // Spin loop detection with details
        if vm.cpu.pc == last_pc {
            same_pc += 1;
            if same_pc == 1000 {
                // Disassemble a few instructions around the spinning PC
                println!(
                    "[trace] SPIN at PC=0x{:08X} ({} consecutive), count={}",
                    vm.cpu.pc, same_pc, count
                );
                // Read the instruction at PC and nearby
                for offset in -4i64..=8i64 {
                    let addr = (vm.cpu.pc as i64 + offset * 4) as u64;
                    if let Ok(word) = vm.bus.read_word(addr) {
                        let op = decode::decode(word);
                        println!("  PA 0x{:08X}: 0x{:08X} => {:?}", addr, word, op);
                    }
                }
            }
        } else {
            if same_pc > 0 && same_pc < 1000 {
                // Short spin that resolved
            }
            same_pc = 0;
            last_pc = vm.cpu.pc;
        }

        count += 1;
    }

    println!(
        "\n[trace] Done: count={} unique_pcs={} ecall_count={}",
        count,
        unique_pcs.len(),
        vm.cpu.ecall_count
    );
    println!(
        "[trace] Final PC=0x{:08X} priv={:?} SATP=0x{:08X}",
        vm.cpu.pc, vm.cpu.privilege, vm.cpu.csr.satp
    );

    // Disassemble last few instructions
    println!("\n[trace] Instructions at final PC:");
    for offset in 0i64..=12i64 {
        let addr = (vm.cpu.pc as i64 + offset * 4) as u64;
        if let Ok(word) = vm.bus.read_word(addr) {
            let op = decode::decode(word);
            println!("  PA 0x{:08X}: 0x{:08X} => {:?}", addr, word, op);
        }
    }
}
