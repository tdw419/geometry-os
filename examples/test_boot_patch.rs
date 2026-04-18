use geometry_os::riscv::cpu::{Privilege, StepResult};
use geometry_os::riscv::RiscvVm;
use std::fs;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = fs::read(kernel_path).unwrap();
    let initramfs = fs::read(initramfs_path).ok();

    let (mut vm, fw_addr, _entry, _dtb_addr) = RiscvVm::boot_linux_setup(
        &kernel_image,
        initramfs.as_deref(),
        256,
        "console=ttyS0 earlycon=sbi panic=1 quiet",
    )
    .unwrap();
    let fw_addr_u32 = fw_addr as u32;

    let max = 250_000u64;
    let mut count = 0u64;
    let mut last_satp = vm.cpu.csr.satp;
    let mut satp_count = 0u32;
    let mut trace_mode = false;
    let mut last_ra = 0u32;
    let mut trap_count = 0u32;

    while count < max {
        if vm.bus.sbi.shutdown_requested {
            break;
        }

        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            let mpp = (vm.cpu.csr.mstatus & 0x300) >> 8;
            trap_count += 1;

            eprintln!(
                "[TRAP #{}] mcause=0x{:08X} code={} mpp={} mepc=0x{:08X} RA=0x{:08X} at count={}",
                trap_count, mcause, cause_code, mpp, vm.cpu.csr.mepc, vm.cpu.x[1], count
            );

            if cause_code == 9 {
                let result = vm.bus.sbi.handle_ecall(
                    vm.cpu.x[17],
                    vm.cpu.x[16],
                    vm.cpu.x[10],
                    vm.cpu.x[11],
                    vm.cpu.x[12],
                    vm.cpu.x[13],
                    0,
                    0,
                    &mut vm.bus.uart,
                    &mut vm.bus.clint,
                );
                if let Some((a0, a1)) = result {
                    vm.cpu.x[10] = a0;
                    vm.cpu.x[11] = a1;
                }
            } else if mpp != 3 {
                let stvec = vm.cpu.csr.stvec & !0x3u32;
                if stvec != 0 && cause_code == 7 {
                    vm.bus.clint.mtimecmp = vm.bus.clint.mtime + 100_000;
                }
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
            } else {
                vm.cpu.x[10] = mcause as u32;
                vm.cpu.pc = vm.cpu.csr.mtvec & !0x3u32;
            }
            continue;
        }

        let result = vm.step();
        count += 1;

        if vm.cpu.csr.satp != last_satp {
            satp_count += 1;
            last_satp = vm.cpu.csr.satp;
            if satp_count >= 2 {
                trace_mode = true;
                last_ra = vm.cpu.x[1];
                eprintln!("[SATP #{}] trace mode ON", satp_count);
            }
        }

        if trace_mode && vm.cpu.pc < 0xC0000000 || vm.cpu.pc > 0xD0000000 {
            eprintln!(
                "[{}] PC OUT OF RANGE: 0x{:08X} RA=0x{:08X} SP=0x{:08X} priv={:?}",
                count, vm.cpu.pc, vm.cpu.x[1], vm.cpu.x[2], vm.cpu.privilege
            );
            break;
        }

        if trace_mode {
            let ra = vm.cpu.x[1];
            if ra != last_ra {
                eprintln!(
                    "[{}] RA: 0x{:08X} -> 0x{:08X} PC=0x{:08X}",
                    count, last_ra, ra, vm.cpu.pc
                );
                last_ra = ra;
            }
        }

        if matches!(
            result,
            StepResult::FetchFault | StepResult::LoadFault | StepResult::StoreFault
        ) {
            eprintln!(
                "[FAULT] {:?} at count={} PC=0x{:08X}",
                result, count, vm.cpu.pc
            );
            break;
        }
    }
    eprintln!("\nTotal traps forwarded: {}", trap_count);
}
