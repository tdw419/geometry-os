use geometry_os::riscv::cpu::{Privilege, StepResult};
use geometry_os::riscv::csr;
use geometry_os::riscv::decode;
use geometry_os::riscv::mmu::{translate, AccessType, TranslateResult};
/// Watch for writes to PA 0x01401D8C (where 0x3FFFF000 appears on the stack)
use geometry_os::riscv::RiscvVm;

const WATCH_PA: u64 = 0x01401D8C;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=5 quiet";

    let (mut vm, fw_addr, _, _) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs).unwrap();
    let fw_addr_u32 = fw_addr as u32;

    let max_count = 178_510u64;
    let mut count: u64 = 0;
    let mut last_val: u32 = 0;
    let mut found = false;

    // Also watch nearby addresses
    let watch_start: u64 = 0x01401D80;
    let watch_end: u64 = 0x01401DA0;

    while count < max_count && !found {
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

        // Check before step
        let prev_val = vm.bus.read_word(WATCH_PA).unwrap_or(0);

        vm.step();

        // Check after step
        let new_val = vm.bus.read_word(WATCH_PA).unwrap_or(0);
        if new_val != prev_val {
            // Decode the instruction that caused the write
            let inst_word = {
                let satp = vm.cpu.csr.satp;
                match translate(
                    vm.cpu.pc,
                    AccessType::Fetch,
                    vm.cpu.privilege,
                    false,
                    false,
                    satp,
                    &mut vm.bus,
                    &mut vm.cpu.tlb,
                ) {
                    TranslateResult::Ok(pa) => vm.bus.read_word(pa).unwrap_or(0),
                    _ => 0,
                }
            };
            let half = (inst_word & 0xFFFF) as u16;
            let op_str = if decode::is_compressed(half) {
                format!("{:?}", decode::decode_c(half))
            } else {
                format!("{:?}", decode::decode(inst_word))
            };

            eprintln!(
                "[{}] WRITE to PA 0x{:08X}: 0x{:08X} -> 0x{:08X} at PC=0x{:08X} inst={}",
                count, WATCH_PA, prev_val, new_val, vm.cpu.pc, op_str
            );
            eprintln!(
                "    SP=0x{:08X} RA=0x{:08X} S0=0x{:08X}",
                vm.cpu.x[2], vm.cpu.x[1], vm.cpu.x[8]
            );

            if new_val == 0x3FFFF000 {
                found = true;
                eprintln!("    *** FOUND THE WRITE OF 0x3FFFF000! ***");
            }
        }

        count += 1;
    }

    if !found {
        eprintln!(
            "Did not find write of 0x3FFFF000 to PA 0x{:08X} in {} steps",
            WATCH_PA, count
        );
        eprintln!(
            "Final value: 0x{:08X}",
            vm.bus.read_word(WATCH_PA).unwrap_or(0)
        );
    }
}
