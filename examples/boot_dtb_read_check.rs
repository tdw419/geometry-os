//! Diagnostic: Verify DTB is readable at the address _dtb_early_va points to.
//! Run: cargo run --example boot_dtb_read_check

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

    // Verify DTB is at PA dtb_addr
    let dtb_magic = vm.bus.read_word(dtb_addr).unwrap_or(0);
    eprintln!("DTB at PA 0x{:08X}: first word = 0x{:08X} (expect 0xEDFE0DD0 for FDT magic)", dtb_addr, dtb_magic);

    // Check _dtb_early_va PA
    let dtb_early_va = vm.bus.read_word(0x00801008).unwrap_or(0);
    let dtb_early_pa = vm.bus.read_word(0x0080100C).unwrap_or(0);
    eprintln!("_dtb_early_va=0x{:08X} (expect VA of DTB = 0x{:08X})", dtb_early_va, dtb_addr.wrapping_add(0xC0000000) as u32);
    eprintln!("_dtb_early_pa=0x{:08X} (expect PA of DTB = 0x{:08X})", dtb_early_pa, dtb_addr as u32);

    // Read a few bytes from the DTB to verify content
    let mut hdr = Vec::new();
    for i in 0..28 {
        let b = vm.bus.read_byte(dtb_addr + i).unwrap_or(0);
        hdr.push(b);
    }
    eprintln!("DTB header bytes: {:02X?}", hdr);

    // The DTB magic in big-endian is 0xD00DFEED
    // In little-endian u32 read: 0xEDFE0DD0
    if dtb_magic == 0xEDFE0DD0 {
        eprintln!("DTB magic OK!");
        // Read totalsize (bytes 4-7)
        let totalsize = vm.bus.read_word(dtb_addr + 4).unwrap_or(0);
        eprintln!("DTB totalsize: {} bytes", totalsize);
    }

    // Now run boot and check at various points
    let fw_addr_u32 = fw_addr as u32;
    let dtb_va_expected = (dtb_addr.wrapping_add(0xC0000000)) as u32;
    let dtb_pa_expected = dtb_addr as u32;
    let max_instr = 1_000_000u64;
    let mut count = 0u64;
    let check_points: Vec<u64> = vec![177_000, 177_300, 177_400, 178_000, 179_000, 180_000, 200_000, 500_000];
    let mut check_idx = 0;

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

        // Watchdog
        if count % 100 == 0 {
            let prb = vm.bus.read_word(0x00C79EACu64).unwrap_or(0);
            if prb == 0 {
                let cur_va = vm.bus.read_word(0x00801008).unwrap_or(0);
                if cur_va != dtb_va_expected {
                    vm.bus.write_word(0x00801008, dtb_va_expected).ok();
                    vm.bus.write_word(0x0080100C, dtb_pa_expected).ok();
                    eprintln!("[{}] RESTORED _dtb_early_va", count);
                }
            }
        }

        // Checkpoints
        if check_idx < check_points.len() && count == check_points[check_idx] {
            let prb = vm.bus.read_word(0x00C79EACu64).unwrap_or(0);
            let deva = vm.bus.read_word(0x00801008).unwrap_or(0);
            let depa = vm.bus.read_word(0x0080100C).unwrap_or(0);
            // Read DTB through the PA (bypassing MMU) to verify it's still there
            let dtb_word0 = vm.bus.read_word(dtb_addr).unwrap_or(0);
            // Read what _dtb_early_va points to (at PA)
            if deva != 0 {
                let dtb_va_pa = (deva as u64).wrapping_sub(0xC0000000);
                let read_via_va = vm.bus.read_word(dtb_va_pa).unwrap_or(0);
                eprintln!("[{}] PC=0x{:08X} satp=0x{:08X} prb=0x{:08X} deva=0x{:08X} DTB[0]=0x{:08X} DTB_via_va[0]=0x{:08X}",
                    count, vm.cpu.pc, vm.cpu.csr.satp, prb, deva, dtb_word0, read_via_va);
            } else {
                eprintln!("[{}] PC=0x{:08X} satp=0x{:08X} prb=0x{:08X} deva=0x{:08X} DTB[0]=0x{:08X}",
                    count, vm.cpu.pc, vm.cpu.csr.satp, prb, deva, dtb_word0);
            }
            check_idx += 1;
        }

        vm.bus.tick_clint();
        vm.bus.sync_mip(&mut vm.cpu.csr.mip);
        count += 1;
    }

    eprintln!("\nFinal: {} instr, PC=0x{:08X}", count, vm.cpu.pc);
}
