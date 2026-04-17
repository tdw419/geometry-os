//! Diagnostic: check raw page table at PA 0x01485000 BEFORE SATP fixup runs.
//! We intercept at the instruction level: detect when the kernel writes SATP,
//! dump the page table, then let the fixup proceed.

use geometry_os::riscv::RiscvVm;
use geometry_os::riscv::cpu::Privilege;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1";

    let (mut vm, fw_addr, _entry, dtb_addr) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs).unwrap();

    // Check DTB reservation map
    eprintln!("=== DTB at PA 0x{:08X} ===", dtb_addr);
    let totalsize = vm.bus.read_word(dtb_addr + 4).unwrap_or(0);
    let off_mem_rsvmap = vm.bus.read_word(dtb_addr + 16).unwrap_or(0);
    let off_dt_struct = vm.bus.read_word(dtb_addr + 8).unwrap_or(0);
    eprintln!("totalsize={} off_mem_rsvmap=0x{:X} off_dt_struct=0x{:X}", totalsize, off_mem_rsvmap, off_dt_struct);

    // Read reservation map entries
    let rsvmap_base = dtb_addr + off_mem_rsvmap as u64;
    eprintln!("\nReservation map entries:");
    for i in 0..10 {
        let entry_addr = rsvmap_base + (i as u64) * 16;
        let addr = vm.bus.read_word(entry_addr as u64 + 4).unwrap_or(0) as u64
            | ((vm.bus.read_word(entry_addr).unwrap_or(0) as u64) << 32);
        let size = vm.bus.read_word(entry_addr as u64 + 12).unwrap_or(0) as u64
            | ((vm.bus.read_word(entry_addr as u64 + 8).unwrap_or(0) as u64) << 32);
        eprintln!("  [{}] addr=0x{:08X} size=0x{:08X} ({}KB)", i, addr, size, size / 1024);
        if addr == 0 && size == 0 {
            eprintln!("  (terminator)");
            break;
        }
    }

    // Check what's at the page table location before boot
    // The kernel's final pg_dir at PA 0x01485000 shouldn't exist yet
    let pg_dir_phys: u64 = 0x01485000;
    let first_word = vm.bus.read_word(pg_dir_phys).unwrap_or(0);
    eprintln!("\nPre-boot: word at PA 0x{:08X} = 0x{:08X}", pg_dir_phys, first_word);

    let fw_addr_u32 = fw_addr as u32;
    let mut count: u64 = 0;
    let max: u64 = 2_000_000;
    let mut last_satp: u32 = vm.cpu.csr.satp;
    let mut satp_count: u32 = 0;
    let mut dumped: bool = false;

    while count < max {
        if vm.bus.sbi.shutdown_requested {
            break;
        }

        // Detect PC about to write SATP (CSRW instruction)
        // Actually, just detect SATP changes like before
        let cur_satp = vm.cpu.csr.satp;
        if cur_satp != last_satp {
            satp_count += 1;
            eprintln!(
                "\n[diag] SATP #{} at count={}: 0x{:08X} -> 0x{:08X}",
                satp_count, count, last_satp, cur_satp
            );

            if satp_count == 3 && !dumped {
                dumped = true;
                let new_ppn = cur_satp & 0x003F_FFFF;
                let pg_dir_phys2 = (new_ppn as u64) * 4096;
                eprintln!("[diag] SATP PPN=0x{:05X} -> pg_dir PA 0x{:08X}", new_ppn, pg_dir_phys2);

                // Dump all non-zero L1 entries
                eprintln!("\n[diag] L1 entries at PA 0x{:08X}:", pg_dir_phys2);
                let mut non_zero_count = 0u32;
                for i in 0..1024u32 {
                    let l1_pte = vm.bus.read_word(pg_dir_phys2 + (i as u64) * 4).unwrap_or(0);
                    if l1_pte != 0 {
                        non_zero_count += 1;
                        let ppn = (l1_pte >> 10) & 0x3FF_FFF;
                        let is_leaf = (l1_pte & 0xE) != 0;
                        let v = (l1_pte & 1) != 0;
                        eprintln!(
                            "  L1[{}] = 0x{:08X} PPN=0x{:05X} V={} leaf={}",
                            i, l1_pte, ppn, v, is_leaf
                        );
                    }
                }
                eprintln!("[diag] Total non-zero L1 entries: {}", non_zero_count);

                // Check kernel_map values
                let km_phys: u64 = 0x00C79E90;
                let km_pa = vm.bus.read_word(km_phys + 12).unwrap_or(0);
                let km_va = vm.bus.read_word(km_phys + 4).unwrap_or(0);
                let km_voff = vm.bus.read_word(km_phys + 8).unwrap_or(0);
                let km_vapo = vm.bus.read_word(km_phys + 20).unwrap_or(0);
                let km_vkpo = vm.bus.read_word(km_phys + 24).unwrap_or(0);
                eprintln!(
                    "\n[diag] kernel_map at PA 0x{:08X}:",
                    km_phys
                );
                eprintln!(
                    "  phys_addr=0x{:X} virt_addr=0x{:X} virt_offset=0x{:X} va_pa_offset=0x{:X} va_kernel_pa_offset=0x{:X}",
                    km_pa, km_va, km_voff, km_vapo, km_vkpo
                );
                break;
            }
            last_satp = cur_satp;
        }

        // M-mode trap forwarding
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let cause_code = vm.cpu.csr.mcause & !(1u32 << 31);
            if cause_code == 9 || cause_code == 11 {
                let mpp = (vm.cpu.csr.mstatus & 0x1800) >> 11;
                if mpp != 3 {
                    let stvec = vm.cpu.csr.stvec & !0x3u32;
                    if stvec != 0 {
                        vm.cpu.csr.sepc = vm.cpu.csr.mepc;
                        vm.cpu.csr.scause = vm.cpu.csr.mcause;
                        vm.cpu.csr.stval = vm.cpu.csr.mtval;
                        vm.cpu.pc = stvec;
                        vm.cpu.privilege = Privilege::Supervisor;
                        vm.cpu.tlb.flush_all();
                        count += 1;
                        continue;
                    }
                }
            }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }

        vm.bus.tick_clint();
        vm.bus.sync_mip(&mut vm.cpu.csr.mip);
        let _ = vm.step();
        count += 1;
    }
}
