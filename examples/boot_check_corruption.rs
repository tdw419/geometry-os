use geometry_os::riscv::cpu::{Privilege, StepResult};
use geometry_os::riscv::RiscvVm;
use std::fs;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = fs::read(kernel_path).expect("kernel");
    let initramfs = fs::read(initramfs_path).ok();

    let (mut vm, fw_addr, _entry, dtb_addr) = RiscvVm::boot_linux_setup(
        &kernel_image,
        initramfs.as_deref(),
        256,
        "console=ttyS0 loglevel=8",
    ).unwrap();

    // Save the original instruction at PA 0x30018
    let orig_3018 = vm.bus.read_word(0x30018).unwrap_or(0);
    let orig_D338 = vm.bus.read_word(0xD338).unwrap_or(0);
    eprintln!("At boot setup: PA 0x30018 = 0x{:08X}, PA 0xD338 = 0x{:08X}", orig_3018, orig_D338);

    let fw_addr_u32 = fw_addr as u32;
    let mut count: u64 = 0;
    let mut last_satp: u32 = vm.cpu.csr.satp;

    while count < 50_000_000 {
        if vm.bus.sbi.shutdown_requested { break; }

        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);

            if cause_code == 9 {
                let result = vm.bus.sbi.handle_ecall(
                    vm.cpu.x[17], vm.cpu.x[16], vm.cpu.x[10], vm.cpu.x[11],
                    vm.cpu.x[12], vm.cpu.x[13], vm.cpu.x[14], vm.cpu.x[15],
                    &mut vm.bus.uart, &mut vm.bus.clint,
                );
                if let Some((a0, a1)) = result { vm.cpu.x[10] = a0; vm.cpu.x[11] = a1; }
            } else if cause_code != 11 {
                let mpp = (vm.cpu.csr.mstatus >> 11) & 3;
                if mpp != 3 {
                    let stvec = vm.cpu.csr.stvec & !0x3u32;
                    if stvec != 0 {
                        vm.cpu.csr.sepc = vm.cpu.csr.mepc;
                        vm.cpu.csr.scause = mcause;
                        vm.cpu.csr.stval = vm.cpu.csr.mtval;
                        let spp = if mpp == 1 { 1u32 } else { 0u32 };
                        vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << 8)) | (spp << 8);
                        let sie = (vm.cpu.csr.mstatus >> 1) & 1;
                        vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << 5)) | (sie << 5);
                        vm.cpu.csr.mstatus &= !(1 << 1);
                        if cause_code == 7 { vm.bus.clint.mtimecmp = vm.bus.clint.mtime + 100_000; }
                        vm.cpu.pc = stvec;
                        vm.cpu.privilege = Privilege::Supervisor;
                        vm.cpu.tlb.flush_all();
                        count += 1;
                        continue;
                    }
                }
                
                // On first illegal instruction, check if PA 0x30018 was overwritten
                if cause_code == 2 {
                    let mepc = vm.cpu.csr.mepc;
                    let mepc_pa = if mepc >= 0xC0000000 { mepc - 0xC0000000 } else { mepc };
                    let cur_val = vm.bus.read_word(mepc_pa as u64).unwrap_or(0);
                    eprintln!("[ILLEGAL] count={} mepc=0x{:08X} PA=0x{:08X} orig=0x{:08X} cur=0x{:08X} {}",
                        count, mepc, mepc_pa, 
                        if mepc_pa == 0x30018 { orig_3018 } else { orig_D338 },
                        cur_val,
                        if cur_val == 0 { "ZERO (corrupted!)" } else { "OK" });
                    
                    // Check a few addresses around it
                    for offset in [-16, -12, -8, -4, 0, 4, 8, 12, 16] {
                        let pa = mepc_pa as i64 + offset;
                        if pa < 0 { continue; }
                        let val = vm.bus.read_word(pa as u64).unwrap_or(0);
                        let marker = if offset == 0 { ">>>" } else { "   " };
                        eprintln!("  {} PA 0x{:08X}: 0x{:08X}", marker, pa, val);
                    }
                    
                    // Check memblock
                    let memblock_pa = 0x00803448u64;
                    let mem_cnt = vm.bus.read_word(memblock_pa + 48).unwrap_or(0);
                    let res_cnt = vm.bus.read_word(memblock_pa + 52).unwrap_or(0);
                    eprintln!("  memblock: memory.cnt={} reserved.cnt={}", mem_cnt, res_cnt);
                    
                    // Check DTB magic at the DTB address
                    let dtb_magic = vm.bus.read_word(dtb_addr).unwrap_or(0);
                    eprintln!("  DTB at PA 0x{:08X}: magic=0x{:08X} (expect 0xD00DFEED)", dtb_addr, dtb_magic);
                    
                    // Stop after first illegal
                    break;
                }
            }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }

        vm.bus.tick_clint();
        vm.bus.sync_mip(&mut vm.cpu.csr.mip);
        match vm.step() {
            StepResult::Ebreak => { eprintln!("[EBREAK]"); break; }
            _ => {}
        }
        count += 1;
    }
    eprintln!("[done] count={}", count);
}
