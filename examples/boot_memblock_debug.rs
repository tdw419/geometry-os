use geometry_os::riscv::RiscvVm;
use geometry_os::riscv::cpu::Privilege;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();

    let (mut vm, fw_addr_u64, _entry, _dtb_addr) =
        RiscvVm::boot_linux_setup(
            &kernel_image,
            initramfs.as_deref(),
            256,
            "console=ttyS0 loglevel=8",
        ).unwrap();

    let fw_addr = fw_addr_u64 as u32;
    let memblock_pa: u64 = 0x00803448;
    let phys_ram_base_pa: u64 = 0x00C79EAC;
    let kernel_map_pa: u64 = 0x00C79E90;

    let mut count: u64 = 0;
    let mut last_satp: u32 = vm.cpu.csr.satp;
    let mut last_uart: usize = 0;

    let device_l1_entries: &[u32] = &[0, 1, 2, 3, 4, 5, 8, 48, 64];
    let identity_pte: u32 = 0x0000_00CF;
    let mega_flags: u32 = 0x0000_00CF;

    while count < 2_000_000 {
        if vm.bus.sbi.shutdown_requested { break; }

        if vm.cpu.pc == fw_addr && vm.cpu.privilege == Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);

            if cause_code == 9 {
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
                vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
                count += 1;
                continue;
            } else if cause_code != 11 {
                let mpp = (vm.cpu.csr.mstatus >> 11) & 3;
                if mpp != 3 {
                    vm.cpu.csr.mepc = vm.cpu.csr.stvec;
                    vm.cpu.privilege = Privilege::Supervisor;
                    count += 1;
                    continue;
                }
            }
        }

        let current_satp = vm.cpu.csr.satp;
        if current_satp != last_satp {
            eprintln!("[satp] 0x{:08X} -> 0x{:08X} at count={}", last_satp, current_satp, count);
            let mode = (current_satp >> 31) & 1;
            if mode == 1 {
                let ppn = current_satp & 0x3FFFFF;
                let pg_dir_phys = (ppn as u64) * 4096;
                for &l1_idx in device_l1_entries {
                    let addr = pg_dir_phys + (l1_idx as u64) * 4;
                    let existing = vm.bus.read_word(addr).unwrap_or(0);
                    if (existing & 1) == 0 {
                        let pte = identity_pte | (l1_idx << 20);
                        vm.bus.write_word(addr, pte).ok();
                    }
                }
                let mut fixup_count = 0u32;
                for l1_scan in 768..780u32 {
                    let scan_addr = pg_dir_phys + (l1_scan as u64) * 4;
                    let entry = vm.bus.read_word(scan_addr).unwrap_or(0);
                    let is_valid = (entry & 1) != 0;
                    let is_non_leaf = is_valid && (entry & 0xE) == 0;
                    let ppn = (entry >> 10) & 0x3FFFFF;
                    let needs_fix = !is_valid || (is_non_leaf && ppn == 0);
                    if !needs_fix { continue; }
                    fixup_count += 1;
                    let pa_offset = l1_scan - 768;
                    let fixup_pte = mega_flags | (pa_offset << 20);
                    vm.bus.write_word(scan_addr, fixup_pte).ok();
                }
                if fixup_count > 0 {
                    eprintln!("[fixup] Fixed {} kernel L1 entries", fixup_count);
                }
                vm.cpu.tlb.flush_all();
            }
            last_satp = current_satp;
        }

        let uart_bytes = vm.bus.uart.tx_buf.len();
        if uart_bytes > 0 && uart_bytes != last_uart {
            last_uart = uart_bytes;
        }

        let _ = vm.step();
        count += 1;
    }

    eprintln!("[done] count={} PC=0x{:08X} uart={}", count, vm.cpu.pc, last_uart);

    let mem_cnt = vm.bus.read_word(memblock_pa + 48).unwrap_or(0);
    let res_cnt = vm.bus.read_word(memblock_pa + 52).unwrap_or(0);
    eprintln!("[memblock] memory.cnt={} reserved.cnt={}", mem_cnt, res_cnt);

    if mem_cnt > 0 {
        for ri in 0..mem_cnt.min(4) {
            let base = vm.bus.read_word(memblock_pa + (ri * 16) as u64).unwrap_or(0);
            let size = vm.bus.read_word(memblock_pa + (ri * 16 + 4) as u64).unwrap_or(0);
            eprintln!("[memblock] memory[{}]: base=0x{:08X} size=0x{:08X} ({}MB)",
                      ri, base, size, size / (1024 * 1024));
        }
    }

    let prb = vm.bus.read_word(phys_ram_base_pa).unwrap_or(0);
    eprintln!("[kern] phys_ram_base=0x{:08X}", prb);

    eprintln!("[kmap] phys_addr=0x{:08X} va_pa_off=0x{:08X}",
              vm.bus.read_word(kernel_map_pa + 12).unwrap_or(0),
              vm.bus.read_word(kernel_map_pa + 20).unwrap_or(0));
}
