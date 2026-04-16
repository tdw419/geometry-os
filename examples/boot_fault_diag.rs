/// Diagnostic: trace exactly what happens around the first S-mode fault at 0x3FFFF000
use geometry_os::riscv::RiscvVm;
use geometry_os::riscv::cpu::{StepResult, Privilege};
use geometry_os::riscv::csr;
use geometry_os::riscv::decode;
use geometry_os::riscv::mmu::{translate, AccessType, TranslateResult};

fn disasm_phys(vm: &mut RiscvVm, vaddr: u32) -> String {
    let satp = vm.cpu.csr.satp;
    let result = translate(vaddr, AccessType::Fetch, vm.cpu.privilege, false, false, satp, &mut vm.bus, &mut vm.cpu.tlb);
    match result {
        TranslateResult::Ok(paddr) => {
            let word = vm.bus.read_word(paddr).unwrap_or(0);
            format!("VA 0x{:08X} -> PA 0x{:08X}: 0x{:08X} {:?}", vaddr, paddr, word, decode::decode(word))
        }
        _ => format!("VA 0x{:08X}: (fault)", vaddr),
    }
}

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=5 quiet";

    let (mut vm, fw_addr, _, _) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs).unwrap();
    let fw_addr_u32 = fw_addr as u32;

    let max_count = 250_000u64;
    let mut count: u64 = 0;
    let mut first_fault_count: u64 = 0;
    let mut fault_logged: u32 = 0;

    while count < max_count {
        if vm.bus.sbi.shutdown_requested { break; }

        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);

            if cause_code == csr::CAUSE_ECALL_M {
                let r = vm.bus.sbi.handle_ecall(
                    vm.cpu.x[17], vm.cpu.x[16], vm.cpu.x[10], vm.cpu.x[11],
                    vm.cpu.x[12], vm.cpu.x[13], vm.cpu.x[14], vm.cpu.x[15],
                    &mut vm.bus.uart, &mut vm.bus.clint);
                if let Some((a0, a1)) = r { vm.cpu.x[10] = a0; vm.cpu.x[11] = a1; }
            } else {
                let mpp = (vm.cpu.csr.mstatus & csr::MSTATUS_MPP_MASK) >> csr::MSTATUS_MPP_LSB;
                if cause_code == csr::CAUSE_ECALL_S {
                    let r = vm.bus.sbi.handle_ecall(
                        vm.cpu.x[17], vm.cpu.x[16], vm.cpu.x[10], vm.cpu.x[11],
                        vm.cpu.x[12], vm.cpu.x[13], vm.cpu.x[14], vm.cpu.x[15],
                        &mut vm.bus.uart, &mut vm.bus.clint);
                    if let Some((a0, a1)) = r { vm.cpu.x[10] = a0; vm.cpu.x[11] = a1; }
                } else if mpp != 3 {
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
                        count += 1;
                        continue;
                    }
                }
            }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
            count += 1;
            continue;
        }

        let result = vm.step();
        match result {
            StepResult::FetchFault | StepResult::LoadFault | StepResult::StoreFault => {
                if vm.cpu.privilege == Privilege::Supervisor && fault_logged < 5 {
                    let ft = match result {
                        StepResult::FetchFault => "fetch",
                        StepResult::LoadFault => "load",
                        StepResult::StoreFault => "store",
                        _ => "",
                    };
                    if first_fault_count == 0 { first_fault_count = count; }
                    fault_logged += 1;
                    eprintln!("[{}] S-mode {} fault #{}: sepc=0x{:08X} stval=0x{:08X} scause=0x{:08X} stvec=0x{:08X}",
                        count, ft, fault_logged, vm.cpu.csr.sepc, vm.cpu.csr.stval, vm.cpu.csr.scause, vm.cpu.csr.stvec);
                    eprintln!("    SP=0x{:08X} RA=0x{:08X} GP=0x{:08X} TP=0x{:08X}",
                        vm.cpu.x[2], vm.cpu.x[1], vm.cpu.x[3], vm.cpu.x[4]);
                    eprintln!("    T0=0x{:08X} T1=0x{:08X} T2=0x{:08X} A0=0x{:08X}",
                        vm.cpu.x[5], vm.cpu.x[6], vm.cpu.x[7], vm.cpu.x[10]);
                    eprintln!("    satp=0x{:08X} mstatus=0x{:08X}", vm.cpu.csr.satp, vm.cpu.csr.mstatus);

                    let sepc = vm.cpu.csr.sepc;
                    eprintln!("    At sepc: {}", disasm_phys(&mut vm, sepc));
                    eprintln!("    At sepc-4: {}", disasm_phys(&mut vm, sepc.wrapping_sub(4)));
                    eprintln!("    At sepc+4: {}", disasm_phys(&mut vm, sepc.wrapping_add(4)));
                }
            }
            StepResult::Ebreak => break,
            _ => {}
        }
        count += 1;
    }

    eprintln!("\n=== STATE AT COUNT {} ===", count);
    eprintln!("PC=0x{:08X} priv={:?}", vm.cpu.pc, vm.cpu.privilege);
    eprintln!("SP=0x{:08X} RA=0x{:08X} GP=0x{:08X}", vm.cpu.x[2], vm.cpu.x[1], vm.cpu.x[3]);
    eprintln!("satp=0x{:08X} stvec=0x{:08X}", vm.cpu.csr.satp, vm.cpu.csr.stvec);
    eprintln!("uart={}", vm.bus.sbi.console_output.len());
    eprintln!("First fault at count={}", first_fault_count);
}
