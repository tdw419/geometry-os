use std::fs;
use geometry_os::riscv::RiscvVm;
use geometry_os::riscv::cpu::{StepResult, Privilege::{self, Supervisor, Machine}};
use geometry_os::riscv::csr;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = fs::read(kernel_path).unwrap();
    let initramfs = fs::read(initramfs_path).ok();
    
    let (mut vm, fw_addr, _, _) = RiscvVm::boot_linux_setup(
        &kernel_image, initramfs.as_deref(), 256, "console=ttyS0 earlycon=sbi loglevel=8"
    ).unwrap();
    
    let fw_u32 = fw_addr as u32;
    let mut count: u64 = 0;
    let max = 200_000u64;
    let mut last_satp = vm.cpu.csr.satp;
    let mut satp_changes = 0;
    
    while count < max {
        if vm.bus.sbi.shutdown_requested { break; }
        
        // M-mode trap handling
        if vm.cpu.pc == fw_u32 && vm.cpu.privilege == Machine {
            let cause_code = vm.cpu.csr.mcause & !(1u32 << 31);
            if cause_code == csr::CAUSE_ECALL_M {
                let r = vm.bus.sbi.handle_ecall(
                    vm.cpu.x[17], vm.cpu.x[16], vm.cpu.x[10], vm.cpu.x[11],
                    vm.cpu.x[12], vm.cpu.x[13], vm.cpu.x[14], vm.cpu.x[15],
                    &mut vm.bus.uart, &mut vm.bus.clint,
                );
                if let Some((a0, a1)) = r { vm.cpu.x[10] = a0; vm.cpu.x[11] = a1; }
            }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }
        
        vm.bus.tick_clint();
        vm.bus.sync_mip(&mut vm.cpu.csr.mip);
        
        // Track what PC does before SATP changes
        let pre_pc = vm.cpu.pc;
        let pre_satp = vm.cpu.csr.satp;
        let r = vm.step();
        
        if vm.cpu.csr.satp != last_satp {
            satp_changes += 1;
            if satp_changes <= 10 {
                eprintln!("[{}] SATP: 0x{:08X}->0x{:08X} PC was=0x{:08X} now=0x{:08X} scause=0x{:08X} sepc=0x{:08X}",
                    count, pre_satp, vm.cpu.csr.satp, pre_pc, vm.cpu.pc,
                    vm.cpu.csr.scause, vm.cpu.csr.sepc);
            }
            last_satp = vm.cpu.csr.satp;
        }
        
        // Log ECALLs
        match r {
            StepResult::Ecall => {
                if count < 500 {
                    eprintln!("[{}] ECALL at PC=0x{:08X} priv={:?} a7=0x{:08X} a0=0x{:08X}",
                        count, vm.cpu.pc, vm.cpu.privilege, vm.cpu.x[17], vm.cpu.x[10]);
                }
            }
            _ => {}
        }
        
        count += 1;
    }
    
    println!("Total SATP changes: {}", satp_changes);
    println!("CPU ecall_count: {}", vm.cpu.ecall_count);
    println!("SBI output: {} bytes", vm.bus.sbi.console_output.len());
}
