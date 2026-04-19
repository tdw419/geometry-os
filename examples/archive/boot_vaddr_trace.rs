use geometry_os::riscv::cpu::Privilege;
use geometry_os::riscv::decode;
use geometry_os::riscv::mmu::{self, AccessType, TranslateResult};
/// Disassemble instructions at virtual addresses using MMU translation.
use geometry_os::riscv::RiscvVm;

fn read_instruction(vm: &mut RiscvVm, vaddr: u32) -> Option<u32> {
    // Translate virtual to physical via MMU
    let satp = vm.cpu.csr.satp;
    let result = mmu::translate(
        vaddr,
        AccessType::Fetch,
        vm.cpu.privilege,
        false,
        false,
        satp,
        &mut vm.bus,
        &mut vm.cpu.tlb,
    );
    match result {
        TranslateResult::Ok(paddr) => vm.bus.read_word(paddr).ok(),
        _ => None,
    }
}

fn disasm_at(vm: &mut RiscvVm, vaddr: u32) -> String {
    match read_instruction(vm, vaddr) {
        Some(word) => format!(
            "VA 0x{:08X}: 0x{:08X} => {:?}",
            vaddr,
            word,
            decode::decode(word)
        ),
        None => format!("VA 0x{:08X}: (unreadable)", vaddr),
    }
}

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

    // Track the PC progression - log every 100K instructions
    let mut milestones: Vec<(u64, u32)> = Vec::new();

    while count < max {
        if vm.bus.sbi.shutdown_requested {
            break;
        }

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

        // Log milestones
        if count > 0 && count % 100_000 == 0 {
            milestones.push((count, vm.cpu.pc));
        }

        count += 1;
    }

    println!(
        "\n[trace] Done: count={} ecall_count={} unique_pcs_hint={}",
        count,
        vm.cpu.ecall_count,
        milestones.len()
    );
    println!(
        "[trace] Final PC=0x{:08X} priv={:?}",
        vm.cpu.pc, vm.cpu.privilege
    );

    // Disassemble at milestones
    println!("\n[trace] PC milestones (disassembled):");
    for (cnt, pc) in &milestones {
        println!("  count={}: {}", cnt, disasm_at(&mut vm, *pc));
    }

    // Disassemble at final PC
    println!("\n[trace] Instructions at final PC (0x{:08X}):", vm.cpu.pc);
    for offset in -2i32..=5i32 {
        let vaddr = (vm.cpu.pc as i32 + offset * 4) as u32;
        println!("  {}", disasm_at(&mut vm, vaddr));
    }

    // Check some key kernel addresses
    println!("\n[trace] Key addresses:");
    println!("  stvec=0x{:08X}", vm.cpu.csr.stvec);
    println!("  sscratch=0x{:08X}", vm.cpu.csr.sscratch);
    println!("  sie=0x{:08X}", vm.cpu.csr.mie);
    println!("  sip=0x{:08X}", vm.cpu.csr.mip);
    println!("  mstatus=0x{:08X}", vm.cpu.csr.mstatus);
    println!("  UART tx_buf: {} chars", vm.bus.uart.tx_buf.len());
    println!("  SBI console: {} chars", vm.bus.sbi.console_output.len());
}
