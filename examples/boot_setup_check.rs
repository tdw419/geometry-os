//! Diagnostic: Check if kernel reaches setup_arch and early_init_dt_scan
//! Run: cargo run --example boot_setup_check

use geometry_os::riscv::RiscvVm;
use geometry_os::riscv::cpu::{Privilege, StepResult};

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

    // Watch for specific PCs
    let setup_arch_pc = 0xC0403C7Au32;
    let early_init_dt_scan_call = 0xC0403CAEu32;
    let fdt_check_header = 0xC01EB8E2u32;
    let mut reached_setup_arch = false;
    let mut reached_early_init_dt_scan = false;
    let mut reached_fdt_check_header = false;
    let mut sbi_count = 0u64;

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

        // Track specific PCs
        if !reached_setup_arch && vm.cpu.pc == setup_arch_pc {
            reached_setup_arch = true;
            eprintln!("[{}] Reached setup_arch at PC=0x{:08X}", count, vm.cpu.pc);
        }
        if !reached_early_init_dt_scan && vm.cpu.pc == early_init_dt_scan_call {
            reached_early_init_dt_scan = true;
            // Print a0 (DTB pointer) at the call
            eprintln!("[{}] Calling early_init_dt_scan(a0=0x{:08X}) at PC=0x{:08X}", count, vm.cpu.x[10], vm.cpu.pc);
        }
        if !reached_fdt_check_header && vm.cpu.pc == fdt_check_header {
            reached_fdt_check_header = true;
            eprintln!("[{}] Entered fdt_check_header(a0=0x{:08X})", count, vm.cpu.x[10]);
            // Read what a0 points to
            let dtb_ptr = vm.cpu.x[10] as u64;
            let w0 = vm.bus.read_word(dtb_ptr).unwrap_or(0);
            let w4 = vm.bus.read_word(dtb_ptr + 4).unwrap_or(0);
            eprintln!("[{}]   DTB at 0x{:08X}: magic=0x{:08X} size_word=0x{:08X}", count, dtb_ptr, w0, w4);
        }

        // Trap handling
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            if (mcause >> 31) & 1 == 1 {
                vm.cpu.csr.mepc = vm.cpu.csr.stvec;
                vm.cpu.csr.mstatus = 1u32 << 7;
                let _ = vm.cpu.csr.trap_return(Privilege::Machine);
                vm.cpu.pc = vm.cpu.csr.mepc;
                vm.cpu.privilege = Privilege::Supervisor;
            } else if cause_code == 11 {
                sbi_count += 1;
                if sbi_count <= 5 {
                    eprintln!("[{}] SBI: a7=0x{:02X} a6={} a0=0x{:08X}", count, vm.cpu.x[17], vm.cpu.x[16], vm.cpu.x[10]);
                }
                if vm.cpu.x[17] == 0x02 && vm.cpu.x[16] == 0 && vm.cpu.x[10] != 0 && vm.cpu.x[10] != 0xFF {
                    eprint!("{}", vm.cpu.x[10] as u8 as char);
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

        // DTB watchdog
        if count % 100 == 0 {
            let prb = vm.bus.read_word(0x00C79EACu64).unwrap_or(0);
            if prb == 0 {
                let cur_va = vm.bus.read_word(0x00801008).unwrap_or(0);
                if cur_va != dtb_va_expected {
                    vm.bus.write_word(0x00801008, dtb_va_expected).ok();
                    vm.bus.write_word(0x0080100C, dtb_pa_expected).ok();
                    eprintln!("[{}] RESTORED _dtb_early_va", count);
                }
            } else {
                eprintln!("[{}] phys_ram_base=0x{:08X}", count, prb);
            }
        }

        vm.bus.tick_clint();
        vm.bus.sync_mip(&mut vm.cpu.csr.mip);
        count += 1;
    }

    eprintln!("\nFinal: {} instr, PC=0x{:08X}", count, vm.cpu.pc);
    eprintln!("reached_setup_arch={}, reached_early_init_dt_scan={}, reached_fdt_check_header={}",
        reached_setup_arch, reached_early_init_dt_scan, reached_fdt_check_header);
    eprintln!("sbi_count={}", sbi_count);
}
