use geometry_os::riscv::cpu::Privilege;
use geometry_os::riscv::RiscvVm;
use std::fs;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = fs::read(kernel_path).expect("kernel");
    let initramfs = fs::read(initramfs_path).ok();

    let bootargs = "console=ttyS0 earlycon=sbi panic=5 nosmp maxcpus=1 loglevel=8";
    let (mut vm, fw_addr, _entry, _dtb_addr) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 128, bootargs).unwrap();

    let fw_addr_u32 = fw_addr as u32;
    let max_count: u64 = 100_000_000;
    let mut count: u64 = 0;
    let mut panic_captured = false;
    let mut sbi_call_count: u64 = 0;
    let mut prev_pc: u32 = 0;
    let mut report_at: u64 = 5_000_000;

    while count < max_count && !vm.bus.sbi.shutdown_requested {
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            if cause_code == 9 {
                sbi_call_count += 1;
                let result = vm.bus.sbi.handle_ecall(
                    vm.cpu.x[17], vm.cpu.x[16], vm.cpu.x[10],
                    vm.cpu.x[11], vm.cpu.x[12], vm.cpu.x[13],
                    vm.cpu.x[14], vm.cpu.x[15],
                    &mut vm.bus.uart, &mut vm.bus.clint,
                );
                if let Some((phys_addr, num_bytes)) = vm.bus.sbi.dbcn_pending_write.take() {
                    let mut buf = vec![0u8; num_bytes.min(2048)];
                    for (i, byte) in buf.iter_mut().enumerate() {
                        match vm.bus.read_byte(phys_addr + i as u64) {
                            Ok(b) => *byte = b,
                            Err(_) => break,
                        }
                    }
                    let s = String::from_utf8_lossy(&buf);
                    eprintln!("[DBCN #{}] {} bytes: {}", sbi_call_count, buf.len(), &s[..s.len().min(200)]);
                    for &b in &buf {
                        if b != 0 {
                            vm.bus.uart.write_byte(0, b);
                            vm.bus.sbi.console_output.push(b);
                        }
                    }
                    vm.cpu.x[10] = 0;
                    vm.cpu.x[11] = buf.len() as u32;
                } else if let Some((a0, a1)) = result {
                    vm.cpu.x[10] = a0;
                    vm.cpu.x[11] = a1;
                }
            }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }

        vm.bus.tick_clint();
        vm.bus.sync_mip(&mut vm.cpu.csr.mip);

        // Capture panic entry
        if !panic_captured && vm.cpu.pc == 0xC000252E && vm.cpu.privilege == Privilege::Supervisor {
            panic_captured = true;
            let panic_fmt_va = vm.cpu.x[10];
            eprintln!("[PANIC] Entered panic at count={}, PC=0x{:08X}", count, vm.cpu.pc);
            eprintln!("[PANIC] a0(fmt)=0x{:08X} a1=0x{:08X} a2=0x{:08X}", panic_fmt_va, vm.cpu.x[11], vm.cpu.x[12]);
            eprintln!("[PANIC] RA=0x{:08X} SP=0x{:08X} GP=0x{:08X}", vm.cpu.x[1], vm.cpu.x[2], vm.cpu.x[3]);

            // Read format string via MMU translation
            let satp = vm.cpu.csr.satp;
            if satp != 0 {
                let pg_dir_ppn = (satp & 0x3FFFFF) as u64;
                let pg_dir_phys = pg_dir_ppn * 4096;
                let vpn1 = ((panic_fmt_va as u64) >> 22) & 0x3FF;
                let vpn0 = ((panic_fmt_va as u64) >> 12) & 0x3FF;
                let page_offset = (panic_fmt_va as u64) & 0xFFF;

                let l1_entry = vm.bus.read_word(pg_dir_phys + vpn1 * 4).unwrap_or(0);
                eprintln!("[PANIC] SATP=0x{:08X} pg_dir=0x{:08X} L1[{}]=0x{:08X}", satp, pg_dir_phys, vpn1, l1_entry);

                if (l1_entry & 1) != 0 {
                    if l1_entry & 0x10 != 0 {
                        // Megapage
                        let phys: u64 = ((l1_entry >> 10) << 12) as u64 | page_offset;
                        let mut fmt = Vec::new();
                        for i in 0..512 {
                            if let Ok(b) = vm.bus.read_byte(phys + i) {
                                if b == 0 { break; }
                                fmt.push(b);
                            } else { break; }
                        }
                        eprintln!("[PANIC] fmt: {}", String::from_utf8_lossy(&fmt));
                    } else {
                        // L2 page table
                        let l2_phys = ((l1_entry as u64 >> 10) << 12);
                        let l2_entry = vm.bus.read_word(l2_phys + vpn0 * 4).unwrap_or(0);
                        eprintln!("[PANIC] L2[{}]=0x{:08X}", vpn0, l2_entry);
                        if (l2_entry & 1) != 0 {
                            let phys = ((l2_entry as u64 >> 10) << 12) | page_offset;
                            let mut fmt = Vec::new();
                            for i in 0..512 {
                                if let Ok(b) = vm.bus.read_byte(phys + i) {
                                    if b == 0 { break; }
                                    fmt.push(b);
                                } else { break; }
                            }
                            eprintln!("[PANIC] fmt: {}", String::from_utf8_lossy(&fmt));
                        }
                    }
                }
            }

            // Also translate the RA to see who called panic
            let ra = vm.cpu.x[1];
            let ra_vpn1 = ((ra as u64) >> 22) & 0x3FF;
            let satp2 = vm.cpu.csr.satp;
            let pg_dir_ppn2 = (satp2 & 0x3FFFFF) as u64;
            let ra_l1 = vm.bus.read_word(pg_dir_ppn2 * 4096 + ra_vpn1 * 4).unwrap_or(0);
            eprintln!("[PANIC] RA=0x{:08X} L1[{}]=0x{:08X}", ra, ra_vpn1, ra_l1);
        }

        vm.step();
        count += 1;

        if count == report_at {
            eprintln!("[{}M] PC=0x{:08X} SBI={}", count / 1_000_000, vm.cpu.pc, sbi_call_count);
            report_at += 5_000_000;
        }
        prev_pc = vm.cpu.pc;
    }

    eprintln!("\n=== Final: {} instructions, {} SBI calls ===", count, sbi_call_count);
    let tx = vm.bus.uart.drain_tx();
    eprintln!("UART: {} bytes", tx.len());
    if !tx.is_empty() {
        let s = String::from_utf8_lossy(&tx);
        eprintln!("{}", &s[..s.len().min(5000)]);
    }
}
