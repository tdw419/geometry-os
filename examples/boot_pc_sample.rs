/// Diagnostic: sample PCs every 1000 instructions to understand
/// what code the kernel is executing after the fault.

use geometry_os::riscv::RiscvVm;
use std::collections::HashMap;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1";

    let (mut vm, fw_addr, _entry, _dtb_addr) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs).unwrap();

    let max = 1_000_000u64;
    let mut count: u64 = 0;
    let fw_addr_u32 = fw_addr as u32;

    // Phase 1: Run to the fault, then sample PCs
    let mut pc_samples: Vec<(u64, u32)> = Vec::new();
    let mut pc_histogram: HashMap<u32, u64> = HashMap::new();
    let mut last_sample_count: u64 = 0;

    while count < max {
        if vm.bus.sbi.shutdown_requested {
            println!("[diag] Shutdown at count={}", count);
            break;
        }

        // Handle M-mode trap forwarding
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            if cause_code == 11 {
                let result = vm.bus.sbi.handle_ecall(
                    vm.cpu.x[17], vm.cpu.x[16],
                    vm.cpu.x[10], vm.cpu.x[11],
                    vm.cpu.x[12], vm.cpu.x[13],
                    vm.cpu.x[14], vm.cpu.x[15],
                    &mut vm.bus.uart, &mut vm.bus.clint,
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
                        vm.cpu.privilege = geometry_os::riscv::cpu::Privilege::Supervisor;
                        vm.cpu.tlb.flush_all();
                        count += 1;
                        continue;
                    }
                }
            }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }

        let _ = vm.step();

        // Sample PCs after the fault (count > 178510)
        if count > 178510 && count - last_sample_count >= 1000 {
            pc_samples.push((count, vm.cpu.pc));
            *pc_histogram.entry(vm.cpu.pc).or_insert(0) += 1;
            last_sample_count = count;
        }

        count += 1;
    }

    // Report unique PC ranges (group by 0x1000 pages)
    println!("=== PC Distribution ({} samples) ===", pc_samples.len());
    let mut page_histogram: HashMap<u32, u64> = HashMap::new();
    for &(_, pc) in &pc_samples {
        let page = pc & !0xFFF;
        *page_histogram.entry(page).or_insert(0) += 1;
    }
    let mut sorted_pages: Vec<_> = page_histogram.iter().collect();
    sorted_pages.sort_by(|a, b| b.1.cmp(a.1));
    for &(page, cnt) in &sorted_pages {
        println!("  Page 0x{:08X}: {} samples ({:.1}%)", page, cnt, *cnt as f64 / pc_samples.len() as f64 * 100.0);
    }

    // Show first 20 and last 20 samples
    println!("\n=== First 20 PC samples ===");
    for &(cnt, pc) in pc_samples.iter().take(20) {
        println!("  count={}: PC=0x{:08X}", cnt, pc);
    }
    println!("\n=== Last 20 PC samples ===");
    for &(cnt, pc) in pc_samples.iter().rev().take(20).rev() {
        println!("  count={}: PC=0x{:08X}", cnt, pc);
    }
}
