/// Diagnostic: count ALL S-mode faults over 5M instructions.
/// Also count unique faulting addresses to understand the pattern.

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

    let max = 5_000_000u64;
    let mut count: u64 = 0;
    let fw_addr_u32 = fw_addr as u32;
    let mut fault_count: u64 = 0;
    let mut ecall_count: u64 = 0;
    let mut fault_addrs: HashSet<u32> = HashSet::new();
    let mut fault_causes: [u64; 16] = [0; 16]; // count by cause code
    let mut prev_scause: u32 = 0;

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

        prev_scause = vm.cpu.csr.scause;
        let prev_ecall = vm.cpu.ecall_count;
        let _ = vm.step();

        // Count ECALLs
        if vm.cpu.ecall_count > prev_ecall {
            ecall_count += 1;
            if ecall_count <= 5 {
                println!("[ecall #{}] count={} PC=0x{:08X} a7=0x{:X} a0=0x{:X}",
                    ecall_count, count, vm.cpu.pc, vm.cpu.x[17], vm.cpu.x[10]);
            }
        }

        // Detect S-mode faults (scause changed from 0 to non-zero)
        if vm.cpu.csr.scause != prev_scause && vm.cpu.csr.scause != 0 {
            let cause = (vm.cpu.csr.scause & 0xFF) as usize;
            if cause < 16 {
                fault_causes[cause] += 1;
            }
            fault_count += 1;
            fault_addrs.insert(vm.cpu.csr.stval);

            if fault_count <= 10 {
                println!("[fault #{}] count={} scause={} sepc=0x{:08X} stval=0x{:08X}",
                    fault_count, count, cause, vm.cpu.csr.sepc, vm.cpu.csr.stval);
            }
        }

        count += 1;
    }

    println!("\n=== Summary ===");
    println!("Total instructions: {}", count);
    println!("S-mode faults: {}", fault_count);
    println!("ECALLs: {}", ecall_count);
    println!("UART output: {} chars", vm.bus.uart.tx_buf.len());
    println!("SBI console: {} chars", vm.bus.sbi.console_output.len());

    println!("\nFault causes:");
    let cause_names = ["insn_misaligned", "insn_access", "illegal_insn", "breakpoint",
        "load_misaligned", "load_access", "store_misaligned", "store_access",
        "ecall_u", "ecall_s", "ecall_m", "insn_page_fault",
        "load_page_fault", "store_page_fault", "reserved14", "reserved15"];
    for i in 0..16 {
        if fault_causes[i] > 0 {
            println!("  cause {} ({}): {}", i, cause_names[i], fault_causes[i]);
        }
    }

    println!("\nUnique faulting addresses (stval): {} total", fault_addrs.len());
    let mut sorted: Vec<_> = fault_addrs.iter().collect();
    sorted.sort();
    for &addr in &sorted {
        println!("  0x{:08X}", addr);
    }

    if !vm.bus.sbi.console_output.is_empty() {
        let s = String::from_utf8_lossy(&vm.bus.sbi.console_output);
        let preview: String = s.chars().take(500).collect();
        println!("\nSBI console output:\n{}", preview);
    }
}
