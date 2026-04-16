/// Diagnostic: Check if the kernel is making forward progress
/// by sampling PCs every N instructions and checking for loops.

use geometry_os::riscv::RiscvVm;
use std::collections::HashSet;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1";

    let (mut vm, fw_addr, _entry, _dtb_addr) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs).unwrap();

    let max = 20_000_000u64;
    let mut count: u64 = 0;
    let mut last_satp: u32 = vm.cpu.csr.satp;
    let mut trap_count: u64 = 0;
    let fw_addr_u32 = fw_addr as u32;

    // Track unique PCs to detect if we're making progress
    let mut pc_samples: Vec<u32> = Vec::new();
    let mut unique_pcs: HashSet<u32> = HashSet::new();

    while count < max {
        if vm.bus.sbi.shutdown_requested {
            println!("[diag] Shutdown requested at count={}", count);
            break;
        }

        // Handle M-mode trap forwarding
        if vm.cpu.pc == fw_addr_u32
            && vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Machine
        {
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
                        vm.cpu.csr.mstatus =
                            (vm.cpu.csr.mstatus & !(1 << 5)) | (spp << 5);
                        let sie = (vm.cpu.csr.mstatus >> 1) & 1;
                        vm.cpu.csr.mstatus =
                            (vm.cpu.csr.mstatus & !(1 << 5)) | (sie << 5);
                        vm.cpu.csr.mstatus &= !(1 << 1);
                        vm.cpu.pc = stvec;
                        vm.cpu.privilege = geometry_os::riscv::cpu::Privilege::Supervisor;
                        vm.cpu.tlb.flush_all();
                        trap_count += 1;
                        if trap_count <= 30 {
                            eprintln!(
                                "[diag] Trap #{} at count={}: cause={} mepc=0x{:08X} -> stvec=0x{:08X} mtval=0x{:08X}",
                                trap_count, count, cause_code, vm.cpu.csr.sepc, stvec, vm.cpu.csr.stval
                            );
                        }
                        count += 1;
                        continue;
                    }
                }
            }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }

        // SATP change detection
        let cur_satp = vm.cpu.csr.satp;
        if cur_satp != last_satp {
            println!(
                "[diag] SATP change: 0x{:08X} -> 0x{:08X} at count={} PC=0x{:08X}",
                last_satp, cur_satp, count, vm.cpu.pc
            );
            last_satp = cur_satp;
        }

        let _step_result = vm.step();
        count += 1;

        // Sample PCs every 10000 instructions
        if count % 10_000 == 0 {
            let pc = vm.cpu.pc;
            pc_samples.push(pc);
            unique_pcs.insert(pc);
        }

        // Report every 5M
        if count % 5_000_000 == 0 {
            println!(
                "[diag] count={} PC=0x{:08X} ecall={} traps={} unique_pcs={}",
                count,
                vm.cpu.pc,
                vm.cpu.ecall_count,
                trap_count,
                unique_pcs.len()
            );
            if !vm.bus.sbi.console_output.is_empty() {
                let s = String::from_utf8_lossy(&vm.bus.sbi.console_output);
                let preview: String = s.chars().take(500).collect();
                println!("[diag] SBI output:\n{}", preview);
            }
        }
    }

    println!(
        "\n[diag] Final: count={} PC=0x{:08X} ecall={} traps={}",
        count,
        vm.cpu.pc,
        vm.cpu.ecall_count,
        trap_count
    );
    println!(
        "[diag] Unique PCs sampled: {} out of {} samples",
        unique_pcs.len(),
        pc_samples.len()
    );

    // Show the last 20 PC samples to see what region the kernel is in
    let last20: Vec<u32> = pc_samples.iter().rev().take(20).cloned().collect();
    println!("[diag] Last 20 sampled PCs:");
    for (i, pc) in last20.iter().rev().enumerate() {
        println!("  sample[-{}]: 0x{:08X}", i, pc);
    }

    // Check scause for any pending faults
    println!(
        "[diag] scause=0x{:08X} sepc=0x{:08X} stvec=0x{:08X}",
        vm.cpu.csr.scause, vm.cpu.csr.sepc, vm.cpu.csr.stvec
    );
    println!(
        "[diag] SP=0x{:08X} GP=0x{:08X} TP=0x{:08X} RA=0x{:08X}",
        vm.cpu.x[2], vm.cpu.x[3], vm.cpu.x[4], vm.cpu.x[1]
    );

    // Show UART and SBI output
    if !vm.bus.sbi.console_output.is_empty() {
        let s = String::from_utf8_lossy(&vm.bus.sbi.console_output);
        println!("[diag] SBI console output ({} bytes):\n{}", vm.bus.sbi.console_output.len(), s);
    }
    if !vm.bus.uart.tx_buf.is_empty() {
        let s = String::from_utf8_lossy(&vm.bus.uart.tx_buf);
        println!("[diag] UART tx_buf ({} bytes):\n{}", vm.bus.uart.tx_buf.len(), s);
    }
}
