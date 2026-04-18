//! Diagnostic: Watch _dtb_early_va every 1000 instructions to see when/how it gets corrupted.
//! Run: cargo run --example boot_dtb_watch3

use geometry_os::riscv::RiscvVm;
use geometry_os::riscv::cpu::{Privilege, StepResult};
use std::time::Instant;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let kernel_data = std::fs::read(kernel_path).unwrap();
    let ir_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let initramfs_data = if std::path::Path::new(ir_path).exists() {
        Some(std::fs::read(ir_path).unwrap())
    } else { None };

    let bootargs = "console=ttyS0 earlycon=sbi panic=5 quiet";
    let (mut vm, fw_addr, _entry, dtb_addr) = RiscvVm::boot_linux_setup(
        &kernel_data, initramfs_data.as_deref(), 512, bootargs,
    ).expect("boot_linux_setup failed");

    let fw_addr_u32 = fw_addr as u32;
    let dtb_va_expected = (dtb_addr.wrapping_add(0xC0000000)) as u32;
    let dtb_pa_expected = dtb_addr as u32;
    let max_instr = 10_000_000u64;
    let mut count = 0u64;

    // Watch _dtb_early_va continuously around the corruption window
    let mut last_dtb_va = dtb_va_expected;
    let mut last_dtb_pa = dtb_pa_expected;
    let mut restore_count = 0u32;

    loop {
        if count >= max_instr { break; }

        let result = vm.step();
        match result {
            StepResult::Ok => {}
            StepResult::FetchFault | StepResult::LoadFault | StepResult::StoreFault => {
                eprintln!("[{}] Fault at PC=0x{:08X}", count, vm.cpu.pc);
                break;
            }
            _ => {}
        }

        // Trap handling (same as boot.rs)
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            let is_interrupt = (mcause >> 31) & 1 == 1;

            if is_interrupt {
                vm.cpu.csr.mepc = vm.cpu.csr.stvec;
                vm.cpu.csr.mstatus = 1u32 << 7;
                let _ = vm.cpu.csr.trap_return(Privilege::Machine);
                vm.cpu.pc = vm.cpu.csr.mepc;
                vm.cpu.privilege = Privilege::Supervisor;
            } else if cause_code == 11 {
                let a7 = vm.cpu.x[17];
                let a6 = vm.cpu.x[16];
                let a0 = vm.cpu.x[10];
                if a7 == 0x02 && a6 == 0 && a0 != 0 && a0 != 0xFF {
                    eprint!("{}", a0 as u8 as char);
                    use std::io::Write;
                    std::io::stderr().flush().ok();
                }
                vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
            } else {
                vm.cpu.csr.mepc = vm.cpu.csr.stvec;
                vm.cpu.csr.mstatus = 1u32 << 7;
                let _ = vm.cpu.csr.trap_return(Privilege::Machine);
                vm.cpu.pc = vm.cpu.csr.mepc;
                vm.cpu.privilege = Privilege::Supervisor;
            }
        }

        // Check every 100 instructions in the critical window (177000-180000)
        if count >= 177000 && count <= 200000 && count % 100 == 0 {
            let cur_va = vm.bus.read_word(0x00801008).unwrap_or(0);
            let cur_pa = vm.bus.read_word(0x0080100C).unwrap_or(0);
            let prb = vm.bus.read_word(0x00C79EAC).unwrap_or(0);

            if cur_va != last_dtb_va || cur_pa != last_dtb_pa || prb != 0 {
                eprintln!("[{}] _dtb_early_va=0x{:08X} _dtb_early_pa=0x{:08X} phys_ram_base=0x{:08X} PC=0x{:08X} satp=0x{:08X}",
                    count, cur_va, cur_pa, prb, vm.cpu.pc, vm.cpu.csr.satp);
                last_dtb_va = cur_va;
                last_dtb_pa = cur_pa;
            }

            // Immediately restore if corrupted
            if cur_va != dtb_va_expected {
                vm.bus.write_word(0x00801008, dtb_va_expected).ok();
                vm.bus.write_word(0x0080100C, dtb_pa_expected).ok();
                restore_count += 1;
                if restore_count <= 20 {
                    eprintln!("[{}] RESTORED (#{})", count, restore_count);
                }
            }
        }

        // After the critical window, check every 1000
        if count > 200000 && count % 1000 == 0 {
            let prb = vm.bus.read_word(0x00C79EAC).unwrap_or(0);
            if prb != 0 {
                eprintln!("[{}] phys_ram_base SET to 0x{:08X}! PC=0x{:08X}", count, prb, vm.cpu.pc);
                // Continue running to see what happens next
            }
        }

        vm.bus.tick_clint();
        vm.bus.sync_mip(&mut vm.cpu.csr.mip);
        count += 1;
    }

    let prb = vm.bus.read_word(0x00C79EAC).unwrap_or(0);
    eprintln!("\n=== FINAL: {} instr, PC=0x{:08X}, phys_ram_base=0x{:08X}, restores={} ===",
        count, vm.cpu.pc, prb, restore_count);

    let mut uart = Vec::new();
    loop { match vm.bus.uart.read_byte(0) { 0 => break, b => uart.push(b) } }
    if !uart.is_empty() {
        eprintln!("UART ({} bytes): {}", uart.len(), String::from_utf8_lossy(&uart));
    } else {
        eprintln!("No UART output");
    }
}
