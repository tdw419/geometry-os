use geometry_os::riscv::RiscvVm;
use std::fs;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = fs::read(kernel_path).expect("kernel");
    let initramfs = fs::read(initramfs_path).ok();

    let bootargs = "console=ttyS0 earlycon=uart8250,mmio32,0x10000000 panic=5 nosmp maxcpus=1 loglevel=8";
    let (mut vm, _fw_addr, _entry, _dtb_addr) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 128, bootargs).unwrap();

    let max_count: u64 = 15_000_000;
    let mut count: u64 = 0;

    while count < max_count && !vm.bus.sbi.shutdown_requested {
        vm.bus.tick_clint();
        vm.bus.sync_mip(&mut vm.cpu.csr.mip);

        // Watch for die() entry at 0xC000B172
        if vm.cpu.pc == 0xC000B172 {
            let a0 = vm.cpu.x[10]; // regs pointer
            let a1 = vm.cpu.x[11]; // str pointer
            let sp = vm.cpu.x[2];
            eprintln!(
                "[die] count={} PC=0xC000B172 a0(regs)=0x{:08X} a1(str)=0x{:08X} RA=0x{:08X} SP=0x{:08X}",
                count, a0, a1, vm.cpu.x[1], sp
            );
            // Read the error string from memory (a1 is a VA, translate to PA)
            let str_va = a1 as u64;
            if str_va != 0 {
                let str_pa = str_va.wrapping_sub(0xC0000000);
                let mut msg = Vec::new();
                for i in 0..256 {
                    if let Ok(b) = vm.bus.read_byte(str_pa + i) {
                        if b == 0 { break; }
                        msg.push(b);
                    } else { break; }
                }
                eprintln!("[die] str: {}", String::from_utf8_lossy(&msg));
            }
            // Read the pt_regs struct to find the trap info
            if a0 != 0 {
                let regs_pa = (a0 as u64).wrapping_sub(0xC0000000);
                // pt_regs layout for RV32: sepc at offset 0, scause at offset 36, stval at offset 40
                // Actually, let me read a few words to see the register values
                if let Ok(sepc) = vm.bus.read_word(regs_pa) {
                    eprintln!("[die] pt_regs.sepc=0x{:08X}", sepc);
                }
                // sstatus is at offset 32
                if let Ok(sstatus) = vm.bus.read_word(regs_pa + 32) {
                    eprintln!("[die] pt_regs.sstatus=0x{:08X}", sstatus);
                }
                // scause at offset 36
                if let Ok(scause) = vm.bus.read_word(regs_pa + 36) {
                    eprintln!("[die] pt_regs.scause=0x{:08X} ({})", scause,
                        match scause {
                            2 => "illegal_instruction",
                            12 => "instruction_page_fault",
                            13 => "load_page_fault",
                            15 => "store_page_fault",
                            8 => "ecall_u",
                            9 => "ecall_s",
                            _ => "unknown",
                        });
                }
                // stval at offset 40
                if let Ok(stval) = vm.bus.read_word(regs_pa + 40) {
                    eprintln!("[die] pt_regs.stval=0x{:08X}", stval);
                }
            }
        }

        // Also watch for _printk calls to see if the kernel tries to print
        if vm.cpu.pc == 0xC0003002 {
            let a0 = vm.cpu.x[10]; // format string
            let pa = (a0 as u64).wrapping_sub(0xC0000000);
            let mut msg = Vec::new();
            for i in 0..80 {
                if let Ok(b) = vm.bus.read_byte(pa + i) {
                    if b == 0 { break; }
                    msg.push(b);
                } else { break; }
            }
            eprintln!("[printk] count={} fmt: {}", count, String::from_utf8_lossy(&msg));
        }

        vm.step();
        count += 1;

        if count == 5_000_000 || count == 10_000_000 {
            eprintln!("[{}M] PC=0x{:08X} uart_writes={}", count / 1_000_000, vm.cpu.pc, vm.bus.uart.write_count);
        }
    }

    eprintln!("Done: {} instructions", count);
    eprintln!("Final PC=0x{:08X} uart_writes={}", vm.cpu.pc, vm.bus.uart.write_count);
    let tx = vm.bus.uart.drain_tx();
    eprintln!("UART: {} bytes", tx.len());
    if !tx.is_empty() {
        let s = String::from_utf8_lossy(&tx);
        eprintln!("{}", &s[..s.len().min(3000)]);
    }
}
