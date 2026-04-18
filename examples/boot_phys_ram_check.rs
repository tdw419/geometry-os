//! Diagnostic: Check phys_ram_base and _dtb_early_va at various points during boot.
//! Run: cargo run --example boot_phys_ram_check

use geometry_os::riscv::{RiscvVm, boot::BootResult};
use std::time::Instant;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let kernel_data = std::fs::read(kernel_path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {}", kernel_path, e));
    eprintln!("Kernel size: {} bytes", kernel_data.len());

    let ir_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let initramfs_data = if std::path::Path::new(ir_path).exists() {
        Some(std::fs::read(ir_path).unwrap())
    } else {
        None
    };
    if let Some(ref ir) = initramfs_data {
        eprintln!("Initramfs size: {} bytes", ir.len());
    }

    let bootargs = "console=ttyS0 earlycon=sbi panic=5 quiet";
    let start = Instant::now();

    // Use boot_linux_setup to get control before the step loop
    let (mut vm, fw_addr, entry, dtb_addr) = RiscvVm::boot_linux_setup(
        &kernel_data,
        initramfs_data.as_deref(),
        512,
        bootargs,
    ).expect("boot_linux_setup failed");

    eprintln!("Entry: 0x{:08X}, DTB: 0x{:08X}, fw_addr: 0x{:08X}", entry, dtb_addr, fw_addr);

    // Check initial state of _dtb_early_va/pa
    let dtb_va_init = vm.bus.read_word(0x00801008).unwrap_or(0);
    let dtb_pa_init = vm.bus.read_word(0x0080100C).unwrap_or(0);
    eprintln!("Initial _dtb_early_va=0x{:08X}, _dtb_early_pa=0x{:08X}", dtb_va_init, dtb_pa_init);

    let fw_addr_u32 = fw_addr as u32;
    let max_instr = 10_000_000;
    let mut count = 0u64;
    let mut last_satp: u32 = 0xFFFFFFFF;
    let check_points: Vec<u64> = vec![100_000, 200_000, 500_000, 1_000_000, 2_000_000, 5_000_000];
    let mut check_idx = 0;

    loop {
        if count >= max_instr {
            break;
        }

        let result = vm.step();
        match result {
            geometry_os::riscv::StepResult::Ok => {}
            geometry_os::riscv::StepResult::Halted => {
                eprintln!("[{}] VM halted at count={}", count, count);
                break;
            }
            geometry_os::riscv::StepResult::Ecall => {}
            _ => {}
        }

        // Detect trap at fw_addr (M-mode trap handler)
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            let is_interrupt = (mcause >> 31) & 1 == 1;

            if is_interrupt {
                // Timer or external interrupt -- forward to S-mode
                vm.cpu.csr.mepc = vm.cpu.pc; // return to same instruction after MRET
                // Set mepc to stvec (S-mode trap handler)
                let stvec = vm.cpu.csr.stvec;
                vm.cpu.csr.mepc = stvec;
                vm.cpu.csr.mstatus = 1u32 << 7; // MPP = Supervisor
                // Execute MRET via CPU
                let _ = vm.cpu.csr.trap_return(geometry_os::riscv::cpu::Privilege::Machine);
                vm.cpu.pc = vm.cpu.csr.mepc;
                vm.cpu.privilege = geometry_os::riscv::cpu::Privilege::Supervisor;
            } else if cause_code == 11 {
                // ECALL_M = SBI call
                let a7 = vm.cpu.x[17];
                let a6 = vm.cpu.x[16];
                let a0 = vm.cpu.x[10];
                if a7 == 0x02 && a6 == 0 {
                    // SBI console putchar
                    if a0 != 0 && a0 != 0xFF {
                        eprint!("{}", a0 as u8 as char);
                    }
                    use std::io::Write;
                    std::io::stderr().flush().ok();
                }
                vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
            } else {
                // Forward exception to S-mode
                vm.cpu.csr.mepc = vm.cpu.pc;
                let stvec = vm.cpu.csr.stvec;
                vm.cpu.csr.mepc = stvec;
                vm.cpu.csr.mstatus = 1u32 << 7;
                let _ = vm.cpu.csr.trap_return(geometry_os::riscv::cpu::Privilege::Machine);
                vm.cpu.pc = vm.cpu.csr.mepc;
                vm.cpu.privilege = geometry_os::riscv::cpu::Privilege::Supervisor;
            }
        }

        // Track SATP changes
        let cur_satp = vm.cpu.csr.satp;
        if cur_satp != last_satp {
            eprintln!("[{}] SATP changed: 0x{:08X} -> 0x{:08X}", count, last_satp, cur_satp);
            last_satp = cur_satp;
        }

        // Check state at checkpoints
        if check_idx < check_points.len() && count == check_points[check_idx] {
            let dtb_va = vm.bus.read_word(0x00801008).unwrap_or(0);
            let dtb_pa = vm.bus.read_word(0x0080100C).unwrap_or(0);
            let phys_ram_base = vm.bus.read_word(0x00C79EAC).unwrap_or(0);
            eprintln!("[{}] _dtb_early_va=0x{:08X} _dtb_early_pa=0x{:08X} phys_ram_base=0x{:08X} PC=0x{:08X}",
                count, dtb_va, dtb_pa, phys_ram_base, vm.cpu.pc);
            check_idx += 1;
        }

        // Advance CLINT timer
        vm.bus.tick_clint();
        vm.bus.sync_mip(&mut vm.cpu.csr.mip);

        count += 1;
    }

    // Final state
    let elapsed = start.elapsed();
    let mips = count as f64 / elapsed.as_secs_f64() / 1_000_000.0;
    eprintln!("\n=== Final State after {} instructions ({:.2} MIPS) ===", count, mips);
    eprintln!("PC=0x{:08X} priv={:?}", vm.cpu.pc, vm.cpu.privilege);
    eprintln!("SATP=0x{:08X}", vm.cpu.csr.satp);
    eprintln!("_dtb_early_va=0x{:08X} _dtb_early_pa=0x{:08X}",
        vm.bus.read_word(0x00801008).unwrap_or(0),
        vm.bus.read_word(0x0080100C).unwrap_or(0));
    eprintln!("phys_ram_base=0x{:08X}", vm.bus.read_word(0x00C79EAC).unwrap_or(0));

    // Check UART
    let mut uart = Vec::new();
    loop {
        match vm.bus.uart.read_byte(0) {
            0 => break,
            b => uart.push(b),
        }
    }
    if !uart.is_empty() {
        eprintln!("UART ({} bytes): {}", uart.len(), String::from_utf8_lossy(&uart));
    } else {
        eprintln!("No UART output");
    }
}
