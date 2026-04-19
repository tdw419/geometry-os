// Trace the percpu crash with ALL registers
use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();

    let (mut vm, _fw_addr, _entry, _dtb_addr) = RiscvVm::boot_linux_setup(
        &kernel_image,
        initramfs.as_deref(),
        256,
        "console=ttyS0 earlycon=sbi loglevel=8",
    )
    .unwrap();

    let mut count: u64 = 0;
    let fast_target = 15_500_000;

    while count < fast_target {
        if vm.bus.sbi.shutdown_requested {
            break;
        }
        let _ = vm.step();
        count += 1;
    }

    let watch_limit = 500_000;
    let mut step_count: u64 = 0;

    while step_count < watch_limit {
        let pc = vm.cpu.pc;
        let _ = vm.step();
        step_count += 1;
        count += 1;

        let scause = vm.cpu.csr.scause;
        if scause != 0 {
            let stval = vm.cpu.csr.stval;
            eprintln!(
                "EXCEPTION at count={} PC=0x{:08X}: scause={} stval=0x{:08X}",
                count, pc, scause, stval
            );

            let names = [
                "zero", "ra", "sp", "gp", "tp", "t0", "t1", "t2",
                "s0", "s1", "a0", "a1", "a2", "a3", "a4", "a5",
                "a6", "a7", "s2", "s3", "s4", "s5", "s6", "s7",
                "s8", "s9", "s10", "s11", "t3", "t4", "t5", "t6",
            ];
            for i in 0..32u32 {
                let val = vm.cpu.x[i as usize];
                if val >= 0xC0000000 {
                    let pa = val - 0xC0000000;
                    eprintln!("  x[{:2}] {} = 0x{:08X}  PA=0x{:08X}", i, names[i as usize], val, pa);
                } else {
                    eprintln!("  x[{:2}] {} = 0x{:08X}", i, names[i as usize], val);
                }
            }

            let s4 = vm.cpu.x[20];
            let s11 = vm.cpu.x[27];
            let s2 = vm.cpu.x[18];
            eprintln!("  RECONSTRUCTION:");
            eprintln!("    s4 (block_idx) = 0x{:08X} ({})", s4, s4);
            eprintln!("    s11 (md_blocks) = 0x{:08X}", s11);
            let computed = s11.wrapping_add(s4.wrapping_mul(32));
            eprintln!("    s11 + s4*32 = 0x{:08X}  (actual s2=0x{:08X})", computed, s2);

            // Read md_blocks array if accessible
            if s11 >= 0xC0000000 && s11 < 0xD0000000 {
                let pa = (s11 - 0xC0000000) as u64;
                eprintln!("    md_blocks PA=0x{:08X}, first entries:", pa);
                for off in (0..64).step_by(4) {
                    let v = vm.bus.read_word(pa + off).unwrap_or(0xDEADDEAD);
                    eprintln!("      [{:3}] 0x{:08X}", off / 4, v);
                }
            }

            break;
        }
    }
}
