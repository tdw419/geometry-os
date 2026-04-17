use std::fs;
use geometry_os::riscv::RiscvVm;
use geometry_os::riscv::cpu::{StepResult, Privilege::{Machine}};
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
    let max = 2_000_000u64;
    
    // Track PC histogram by 2MB region
    let mut pc_regions: std::collections::HashMap<u32, u64> = std::collections::HashMap::new();
    let mut ecall_count = 0u64;
    let mut last_ecall_pc = 0u32;
    let mut last_ecall_a7 = 0u32;
    
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
        
        let pre_ecall = vm.cpu.ecall_count;
        let r = vm.step();
        
        // Track ECALLs
        if vm.cpu.ecall_count > pre_ecall {
            ecall_count += 1;
            last_ecall_pc = vm.cpu.pc;
            last_ecall_a7 = vm.cpu.x[17];
            if ecall_count <= 20 {
                eprintln!("[{}] ECALL: PC=0x{:08X} a7=0x{:08X} a0=0x{:08X} a1=0x{:08X}", 
                    count, vm.cpu.pc, vm.cpu.x[17], vm.cpu.x[10], vm.cpu.x[11]);
            }
        }
        
        // PC region histogram
        let region = vm.cpu.pc >> 21; // 2MB regions
        *pc_regions.entry(region).or_insert(0) += 1;
        
        count += 1;
    }
    
    println!("Total ECALLs: {} (cpu.ecall_count={})", ecall_count, vm.cpu.ecall_count);
    println!("SBI output: {} bytes", vm.bus.sbi.console_output.len());
    println!();
    println!("PC region histogram (top 20):");
    let mut sorted: Vec<_> = pc_regions.iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(a.1));
    for (region, hits) in sorted.iter().take(20) {
        let base = *region << 21;
        println!("  VA 0x{:08X}: {} ({}%)", base, hits, *hits * 100 / max as u64);
    }
}
