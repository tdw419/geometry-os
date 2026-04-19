//! Diagnostic: Read memblock struct from correct offsets.
//! Run: cargo run --example boot_memblock

use geometry_os::riscv::cpu::{Privilege, StepResult};
use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let kernel_data = std::fs::read(kernel_path).unwrap();
    let ir_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let initramfs_data = if std::path::Path::new(ir_path).exists() {
        Some(std::fs::read(ir_path).unwrap())
    } else {
        None
    };

    let bootargs = "console=ttyS0 earlycon=sbi panic=5 quiet";
    let (mut vm, fw_addr, _entry, dtb_addr) =
        RiscvVm::boot_linux_setup(&kernel_data, initramfs_data.as_deref(), 512, bootargs)
            .expect("boot_linux_setup failed");

    let fw_addr_u32 = fw_addr as u32;
    let dtb_va = (dtb_addr.wrapping_add(0xC0000000)) as u32;
    let max_instr = 1_000_000u64;
    let mut count = 0u64;

    // memblock is at VA 0xC0803448, PA 0x00803448
    let memblock_pa = 0x00803448u64;

    // Check at 200K (after early_init_dt_scan) and at 1M
    let mut last_check = 0u64;

    loop {
        if count >= max_instr {
            break;
        }
        let result = vm.step();
        match result {
            StepResult::Ok => {}
            StepResult::FetchFault | StepResult::LoadFault | StepResult::StoreFault => {
                eprintln!("[{}] FAULT at PC=0x{:08X}", count, vm.cpu.pc);
                break;
            }
            _ => {}
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

        // DTB watchdog
        if count % 100 == 0 {
            let prb = vm.bus.read_word(0x00C79EACu64).unwrap_or(0);
            if prb == 0 {
                let cur_va = vm.bus.read_word(0x00801008).unwrap_or(0);
                if cur_va != dtb_va {
                    vm.bus.write_word(0x00801008, dtb_va).ok();
                    vm.bus.write_word(0x0080100C, dtb_addr as u32).ok();
                }
            }
        }

        // Check memblock at various points
        if count - last_check >= 100_000 {
            last_check = count;
            let prb = vm.bus.read_word(0x00C79EACu64).unwrap_or(0);
            // memblock struct (32-bit RV32):
            // offset 0: bottom_up (bool, 4 bytes)
            // offset 4: current_limit (phys_addr_t, 4 bytes)
            // offset 8: memory.cnt (4)
            // offset 12: memory.max (4)
            // offset 16: memory.total_size (4)
            // offset 20: memory.regions (pointer, 4)
            // offset 24: reserved.cnt (4)
            // offset 28: reserved.max (4)
            // offset 32: reserved.total_size (4)
            // offset 36: reserved.regions (pointer, 4)
            let mem_cnt = vm.bus.read_word(memblock_pa + 8).unwrap_or(0);
            let mem_max = vm.bus.read_word(memblock_pa + 12).unwrap_or(0);
            let mem_total = vm.bus.read_word(memblock_pa + 16).unwrap_or(0);
            let mem_regions_ptr = vm.bus.read_word(memblock_pa + 20).unwrap_or(0);
            let res_cnt = vm.bus.read_word(memblock_pa + 24).unwrap_or(0);
            let res_max = vm.bus.read_word(memblock_pa + 28).unwrap_or(0);
            let res_total = vm.bus.read_word(memblock_pa + 32).unwrap_or(0);
            eprintln!("[{}] PC=0x{:08X} prb=0x{:08X}", count, vm.cpu.pc, prb);
            eprintln!(
                "  memblock: bottom_up={} current_limit=0x{:08X}",
                vm.bus.read_word(memblock_pa).unwrap_or(0),
                vm.bus.read_word(memblock_pa + 4).unwrap_or(0)
            );
            eprintln!(
                "  memory: cnt={} max={} total_size=0x{:08X} regions_ptr=0x{:08X}",
                mem_cnt, mem_max, mem_total, mem_regions_ptr
            );
            eprintln!(
                "  reserved: cnt={} max={} total_size=0x{:08X}",
                res_cnt, res_max, res_total
            );

            // If memory has regions, read them
            if mem_cnt > 0 && mem_regions_ptr != 0 {
                let regions_pa = (mem_regions_ptr as u64).wrapping_sub(0xC0000000);
                for i in 0..mem_cnt.min(4) {
                    let base = vm.bus.read_word(regions_pa + (i * 16) as u64).unwrap_or(0);
                    let size = vm
                        .bus
                        .read_word(regions_pa + (i * 16 + 4) as u64)
                        .unwrap_or(0);
                    eprintln!(
                        "  memory[{}]: base=0x{:08X} size=0x{:08X} ({}MB)",
                        i,
                        base,
                        size,
                        size / (1024 * 1024)
                    );
                }
            }
        }

        vm.bus.tick_clint();
        vm.bus.sync_mip(&mut vm.cpu.csr.mip);
        count += 1;
    }

    eprintln!("\nFinal: {} instr, PC=0x{:08X}", count, vm.cpu.pc);
}
