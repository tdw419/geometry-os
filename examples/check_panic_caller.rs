// Find what calls panic - set breakpoint just before panic loop
use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = std::path::Path::new(".geometry_os/build/linux-6.14/vmlinux");
    let kernel = std::fs::read(kernel_path).expect("kernel");
    let initramfs_path = std::path::Path::new(".geometry_os/fs/linux/rv32/initramfs.cpio.gz");
    let initramfs = std::fs::read(initramfs_path).ok();

    let (mut vm, _fw_addr, _entry, _dtb_addr) = RiscvVm::boot_linux_setup(
        &kernel,
        initramfs.as_deref(),
        128,
        "console=ttyS0 earlycon=sbi",
    )
    .unwrap();

    // The panic function starts around 0xC000252E
    // Let's catch when PC first enters the panic function
    let panic_start: u32 = 0xC000252E;
    let panic_end: u32 = 0xC00027A4;
    let mut count: u64 = 0;
    let max: u64 = 400_000;

    while count < max {
        let pc = vm.cpu.pc;
        if pc >= panic_start && pc < panic_end {
            // We just entered panic() for the first time
            println!("[panic-entry] count={}, PC=0x{:08X}", count, pc);

            // Dump registers - especially the ones that carry panic arguments
            println!(
                "[regs] a0=0x{:08X} a1=0x{:08X} a2=0x{:08X}",
                vm.cpu.x[10], vm.cpu.x[11], vm.cpu.x[12]
            );
            println!(
                "[regs] ra=0x{:08X} sp=0x{:08X} gp=0x{:08X} tp=0x{:08X}",
                vm.cpu.x[1], vm.cpu.x[2], vm.cpu.x[3], vm.cpu.x[4]
            );
            println!(
                "[regs] t0=0x{:08X} t1=0x{:08X} t2=0x{:08X}",
                vm.cpu.x[5], vm.cpu.x[6], vm.cpu.x[7]
            );

            // RA is the return address - tells us who called panic
            let ra = vm.cpu.x[1];
            println!("\n[caller] panic called from RA=0x{:08X}", ra);

            // Disassemble the caller
            let caller_start = if ra > 40 { ra - 40 } else { 0 };
            let caller_end = ra + 20;
            println!("[disasm] Caller context (RA-40 to RA+20):");

            // Walk back through stack frames
            // The caller's caller is on the stack
            let sp = vm.cpu.x[2];
            println!("[stack] sp=0x{:08X}", sp);
            for i in 0..16 {
                let addr = (sp as u64) + (i as u64) * 4;
                if let Ok(val) = vm.bus.read_word(addr) {
                    if val >= 0xC0000000 && val < 0xD0000000 {
                        println!("  sp+{} = 0x{:08X}", i * 4, val);
                    }
                }
            }

            // The panic format string is in a0
            let a0 = vm.cpu.x[10];
            if a0 >= 0xC0000000 && a0 < 0xD0000000 {
                let pa = (a0 - 0xC0000000) as u64;
                let mut s = String::new();
                for j in 0..300 {
                    if let Ok(byte) = vm.bus.read_byte(pa + j as u64) {
                        if byte == 0 {
                            break;
                        }
                        if byte >= 0x20 && byte < 0x7F {
                            s.push(byte as char);
                        } else {
                            s.push('.');
                        }
                    }
                }
                println!("\n[panic-msg] a0 string: \"{}\"", s);
            }

            break;
        }
        let _result = vm.step();
        count += 1;
    }

    if count >= max {
        println!(
            "[info] No panic in {} instructions. PC=0x{:08X}",
            max, vm.cpu.pc
        );
    }
}
