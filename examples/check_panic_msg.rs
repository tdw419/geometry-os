// Find the panic message by reading the panic string from RAM
use geometry_os::riscv::RiscvVm;

fn read_cstring(vm: &mut RiscvVm, pa: u64, max_len: usize) -> String {
    let mut s = String::new();
    for j in 0..max_len {
        if let Ok(byte) = vm.bus.read_byte(pa + j as u64) {
            if byte == 0 { break; }
            if byte >= 0x20 && byte < 0x7F {
                s.push(byte as char);
            } else {
                s.push('.');
            }
        }
    }
    s
}

fn va_to_pa(va: u32) -> u64 {
    (va - 0xC0000000) as u64
}

fn main() {
    let kernel_path = std::path::Path::new(".geometry_os/build/linux-6.14/vmlinux");
    let kernel = std::fs::read(kernel_path).expect("kernel");
    let initramfs_path = std::path::Path::new(".geometry_os/fs/linux/rv32/initramfs.cpio.gz");
    let initramfs = std::fs::read(initramfs_path).ok();

    let (mut vm, _fw_addr, _entry, _dtb_addr) =
        RiscvVm::boot_linux_setup(&kernel, initramfs.as_deref(), 128, "console=ttyS0 earlycon=sbi").unwrap();

    let panic_loop: u32 = 0xC000278A;
    let mut count: u64 = 0;
    let max: u64 = 400_000;

    while count < max {
        let _result = vm.step();
        count += 1;
        if vm.cpu.pc >= panic_loop && vm.cpu.pc <= 0xC00027A0 {
            println!("[panic] Hit panic loop at count={}, PC=0x{:08X}", count, vm.cpu.pc);
            
            let s1 = vm.cpu.x[9];
            let s0 = vm.cpu.x[8];
            let a0 = vm.cpu.x[10];
            let sp = vm.cpu.x[2];
            println!("[panic] s1=0x{:08X} s0=0x{:08X} a0=0x{:08X} sp=0x{:08X}", s1, s0, a0, sp);
            
            // Dump stack and look for kernel VA pointers
            for i in 0..32 {
                let addr = (sp as u64) + (i as u64) * 4;
                if let Ok(val) = vm.bus.read_word(addr) {
                    if val >= 0xC0000000 && val < 0xD0000000 {
                        let s = read_cstring(&mut vm, va_to_pa(val), 200);
                        if s.len() > 5 && s.contains(|c: char| c.is_ascii_alphabetic()) {
                            println!("  sp+{} = 0x{:08X} -> \"{}\"", i*4, val, s);
                        }
                    }
                }
            }
            
            // Try reading from s1 (it's the string pointer in panic, but advanced)
            if s1 >= 0xC0000000 && s1 < 0xD0000000 {
                // s1 has been advanced, go back to find start
                let start = if s1 > 300 { s1 - 300 } else { 0xC0000000 };
                let s = read_cstring(&mut vm, va_to_pa(start), 600);
                if s.len() > 5 {
                    println!("[panic] near s1: \"{}\"", s);
                }
            }
            
            // Also check s0
            if s0 >= 0xC0000000 && s0 < 0xD0000000 {
                let s = read_cstring(&mut vm, va_to_pa(s0), 200);
                if s.len() > 3 {
                    println!("[panic] s0 string: \"{}\"", s);
                }
            }
            
            break;
        }
    }
    
    if count >= max {
        println!("[panic] Did not reach panic in {} instructions. PC=0x{:08X}", max, vm.cpu.pc);
    }
}
