use geometry_os::riscv::cpu::{Privilege, StepResult};
use geometry_os::riscv::csr;
/// Check what's on the kernel stack at the time of the fault
use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=5 quiet";

    let (mut vm, fw_addr, _, _) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs).unwrap();
    let fw_addr_u32 = fw_addr as u32;

    // Run to 178K steps (just before the fault)
    let max_count = 178_500u64;
    let mut count: u64 = 0;
    while count < max_count {
        if vm.bus.sbi.shutdown_requested {
            break;
        }
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            if cause_code == csr::CAUSE_ECALL_M {
                let r = vm.bus.sbi.handle_ecall(
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
                if let Some((a0, a1)) = r {
                    vm.cpu.x[10] = a0;
                    vm.cpu.x[11] = a1;
                }
            } else {
                let mpp = (vm.cpu.csr.mstatus & csr::MSTATUS_MPP_MASK) >> csr::MSTATUS_MPP_LSB;
                if cause_code == csr::CAUSE_ECALL_S {
                    let r = vm.bus.sbi.handle_ecall(
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
                    if let Some((a0, a1)) = r {
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
            }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
            count += 1;
            continue;
        }
        vm.step();
        count += 1;
    }

    eprintln!("=== State at count {} ===", count);
    eprintln!(
        "PC=0x{:08X} SP=0x{:08X} RA=0x{:08X}",
        vm.cpu.pc, vm.cpu.x[2], vm.cpu.x[1]
    );
    eprintln!("S0=0x{:08X} (frame pointer)", vm.cpu.x[8]);

    // Dump stack contents around SP
    let sp = vm.cpu.x[2];
    eprintln!("\nStack (SP=0x{:08X}):", sp);
    // SP is a virtual address. Translate to physical.
    // SP=0xC1401E00, L1[773] maps VA 0xC1400000-0xC15FFFFF to PA 0x14000000-0x15FFFFF
    // PA = SP - 0xC0000000 = 0x1401E00
    let sp_pa = (sp as u64) - 0xC0000000;
    for offset in -16..=16i32 {
        let addr = sp_pa.wrapping_add((offset as i64 * 4) as u64);
        let val = vm.bus.read_word(addr).unwrap_or(0);
        let marker = if offset == 0 { " <-- SP" } else { "" };
        eprintln!(
            "  [SP{:+3}] VA 0x{:08X} PA 0x{:08X} = 0x{:08X}{}",
            offset * 4,
            sp.wrapping_add((offset * 4) as u32),
            addr,
            val,
            marker
        );
    }

    // Also dump around S0 (frame pointer)
    let s0 = vm.cpu.x[8];
    let s0_pa = (s0 as u64) - 0xC0000000;
    eprintln!("\nFrame (S0=0x{:08X}):", s0);
    for offset in -20..=4i32 {
        let addr = s0_pa.wrapping_add((offset as i64 * 4) as u64);
        let val = vm.bus.read_word(addr).unwrap_or(0);
        let marker = if offset == 0 { " <-- S0" } else { "" };
        if val != 0 {
            eprintln!(
                "  [S0{:+3}] VA 0x{:08X} PA 0x{:08X} = 0x{:08X}{}",
                offset * 4,
                s0.wrapping_add((offset * 4) as u32),
                addr,
                val,
                marker
            );
        }
    }

    // The fault is at sepc=0x3FFFF000, which means RA=0x3FFFF000
    // Let's search for 0x3FFFF000 in the stack region
    eprintln!("\nSearching for 0x3FFFF000 in stack region (PA 0x14018000-0x14020000):");
    for addr in (0x14018000u64..0x14020000).step_by(4) {
        let val = vm.bus.read_word(addr).unwrap_or(0);
        if val == 0x3FFFF000 {
            let va = 0xC0000000u32 + addr as u32;
            eprintln!("  FOUND at PA 0x{:08X} (VA 0x{:08X})", addr, va);
        }
    }
}
