// Run with nosmp and watch for panic or progress
use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = std::path::Path::new(".geometry_os/build/linux-6.14/vmlinux");
    let kernel = std::fs::read(kernel_path).expect("kernel");
    let initramfs_path = std::path::Path::new(".geometry_os/fs/linux/rv32/initramfs.cpio.gz");
    let initramfs = std::fs::read(initramfs_path).ok();

    let (mut vm, _fw_addr, _entry, _dtb_addr) =
        RiscvVm::boot_linux_setup(&kernel, initramfs.as_deref(), 128, "console=ttyS0 earlycon=sbi loglevel=8").unwrap();

    let panic_start: u32 = 0xC000252E;
    let panic_end: u32 = 0xC00027A4;
    let mut count: u64 = 0;
    let max: u64 = 2_000_000;
    let mut ecall_count: u64 = 0;
    let mut hit_panic = false;
    let mut last_pc = 0u32;
    let mut stall_count = 0u64;

    while count < max {
        let pc = vm.cpu.pc;
        
        // Detect stall
        if pc == last_pc {
            stall_count += 1;
            if stall_count > 1000 {
                println!("[stall] PC stuck at 0x{:08X} for {} steps", pc, stall_count);
                break;
            }
        } else {
            stall_count = 0;
        }
        last_pc = pc;
        
        let _result = vm.step();
        count += 1;
        
        // Track ECALLs
        if vm.cpu.ecall_count > ecall_count {
            ecall_count = vm.cpu.ecall_count;
            let ext = vm.cpu.x[17];
            let func = vm.cpu.x[16];
            let ext_name = match ext {
                0x02 => "CONSOLE",
                0x10 => "RFENCE",
                0x05 => "RESET",
                0x08 => "HSM",
                0x04 => "IPI",
                0x00 => "SET_TIMER",
                _ => "UNKNOWN",
            };
            println!("[ecall] #{} at count={}, PC=0x{:08X}: ext=0x{:02X}({}) func={}",
                ecall_count, count, pc, ext, ext_name, func);
        }
        
        // Check for panic
        if !hit_panic && pc >= panic_start && pc < panic_end {
            hit_panic = true;
            let a0 = vm.cpu.x[10];
            if a0 >= 0xC0000000 && a0 < 0xD0000000 {
                let pa = (a0 - 0xC0000000) as u64;
                let mut s = String::new();
                for j in 0..300 {
                    if let Ok(byte) = vm.bus.read_byte(pa + j as u64) {
                        if byte == 0 { break; }
                        if byte >= 0x20 && byte < 0x7F {
                            s.push(byte as char);
                        } else {
                            s.push('.');
                        }
                    }
                }
                println!("[PANIC] at count={}, PC=0x{:08X}: \"{}\"", count, pc, s);
            } else {
                println!("[PANIC] at count={}, PC=0x{:08X}: a0=0x{:08X} (not a string)", count, pc, a0);
            }
        }
        
        // Progress reports
        if count % 500_000 == 0 {
            println!("[progress] {} instr, PC=0x{:08X}, ECALLs={}", count, vm.cpu.pc, ecall_count);
        }
    }
    
    println!("[done] {} instr, PC=0x{:08X}, ECALLs={}, panic={}", count, vm.cpu.pc, ecall_count, hit_panic);
}
