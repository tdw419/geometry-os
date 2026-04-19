use std::fs;
use std::time::Instant;
use geometry_os::riscv::RiscvVm;
use geometry_os::riscv::cpu::{Privilege, StepResult};

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = fs::read(kernel_path).expect("kernel");
    let initramfs = fs::read(initramfs_path).ok();

    let bootargs = "console=ttyS0 earlycon=sbi panic=5 quiet";
    let (mut vm, fw_addr, _entry, dtb_addr) = RiscvVm::boot_linux_setup(
        &kernel_image, initramfs.as_deref(), 512, bootargs,
    ).unwrap();

    let fw_addr_u32 = fw_addr as u32;
    let max_count: u64 = 5_000_000;
    let mut count: u64 = 0;
    let mut sbi_count: u64 = 0;
    let mut ecall_s_count: u64 = 0;
    let mut last_satp: u32 = vm.cpu.csr.satp;
    let mut satp_changes: u32 = 0;
    let mut prev_pc: u32 = 0;
    let mut pc_set: std::collections::HashSet<u32> = std::collections::HashSet::new();
    let mut next_report: u64 = 500_000;

    while count < max_count {
        if vm.bus.sbi.shutdown_requested { break; }

        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            if cause_code == 9 {
                // ECALL_S = SBI call
                sbi_count += 1;
                ecall_s_count += 1;
                let result = vm.bus.sbi.handle_ecall(
                    vm.cpu.x[17], vm.cpu.x[16], vm.cpu.x[10], vm.cpu.x[11],
                    vm.cpu.x[12], vm.cpu.x[13], vm.cpu.x[14], vm.cpu.x[15],
                    &mut vm.bus.uart, &mut vm.bus.clint,
                );
                if let Some((a0, a1)) = result { vm.cpu.x[10] = a0; vm.cpu.x[11] = a1; }
            }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }

        vm.bus.tick_clint();
        vm.bus.sync_mip(&mut vm.cpu.csr.mip);
        let step_result = vm.step();

        // Track SATP changes
        if vm.cpu.csr.satp != last_satp {
            satp_changes += 1;
            eprintln!("[satp] #{} at count={}: 0x{:08X} -> 0x{:08X} PC=0x{:08X}", 
                satp_changes, count, last_satp, vm.cpu.csr.satp, vm.cpu.pc);
            last_satp = vm.cpu.csr.satp;
        }

        // Track unique PCs
        if count < 1_000_000 {
            pc_set.insert(vm.cpu.pc);
        }

        count += 1;

        if count == next_report {
            eprintln!("[{}] PC=0x{:08X} SP=0x{:08X} RA=0x{:08X} SBI={} SATP=0x{:08X} unique_PCs={}",
                count / 1_000_000, vm.cpu.pc, vm.cpu.x[2], vm.cpu.x[1], sbi_count, vm.cpu.csr.satp, pc_set.len());
            next_report += 500_000;
        }

        prev_pc = vm.cpu.pc;
    }

    eprintln!("\n=== Final State ===");
    eprintln!("Count: {}", count);
    eprintln!("PC: 0x{:08X} SP: 0x{:08X} RA: 0x{:08X}", vm.cpu.pc, vm.cpu.x[2], vm.cpu.x[1]);
    eprintln!("SATP: 0x{:08X}", vm.cpu.csr.satp);
    eprintln!("SBI calls: {}", sbi_count);
    eprintln!("ECALL_S count: {}", ecall_s_count);
    eprintln!("SATP changes: {}", satp_changes);
    eprintln!("Unique PCs (first 1M): {}", pc_set.len());

    // Check what function PC is in
    let pc = vm.cpu.pc;
    if pc >= 0xC0404900 && pc < 0xC0404A00 {
        eprintln!("PC is in create_pgd_mapping (0xC0404948 - 0xC0404A00)");
    } else if pc >= 0xC000EB00 && pc < 0xC000F000 {
        eprintln!("PC is in vm_get_page_prot/__set_fixmap (0xC000EB06 - 0xC000EB70)");
    } else if pc >= 0xC0404A00 && pc < 0xC0404C00 {
        eprintln!("PC is in setup_vm (0xC0404A70 - 0xC0404BD2)");
    } else if pc >= 0xC0001000 && pc < 0xC0001200 {
        eprintln!("PC is in _start_kernel/relocate_enable_mmu (0xC0001000 - 0xC0001200)");
    } else if pc >= 0xC0402DB0 && pc < 0xC0403000 {
        eprintln!("PC is in soc_early_init (0xC0402DBA)");
    } else if pc >= 0xC0400780 && pc < 0xC0400800 {
        eprintln!("PC is in start_kernel (0xC040078E)");
    }

    // Check UART output
    let tx = vm.bus.uart.drain_tx();
    if !tx.is_empty() {
        let s = String::from_utf8_lossy(&tx);
        eprintln!("\nUART output ({} bytes):\n{}", tx.len(), &s[..s.len().min(2000)]);
    } else {
        eprintln!("\nNo UART output");
    }

    let elapsed = Instant::now().elapsed();
    eprintln!("Time: {:?}", elapsed);
    eprintln!("IPS: {:.0}", count as f64 / elapsed.as_secs_f64());
}
