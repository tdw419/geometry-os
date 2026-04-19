// Trace percpu setup after the C.LUI fix
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
    let max = 20_000_000;
    let mut last_ecall = 0u64;

    while count < max {
        if vm.bus.sbi.shutdown_requested {
            break;
        }
        let pc = vm.cpu.pc;

        // Track the C.LUI at 0xC040AF6E specifically
        if pc == 0xC040AF6E {
            eprintln!(
                "[{}] C.LUI a5 at PC=0x{:08X}: a5_before=0x{:08X}",
                count, pc, vm.cpu.x[15]
            );
        }
        if pc == 0xC040AF72 {
            eprintln!(
                "[{}] After AND: s8=0x{:08X} (should be 0xCFDB6000)",
                count, vm.cpu.x[24]
            );
        }

        // Track pcpu_alloc_first_chunk
        if pc == 0xC040AF6C {
            eprintln!(
                "[{}] pcpu_alloc_first_chunk(a0=0x{:08X}, a1=0x{:08X})",
                count, vm.cpu.x[10], vm.cpu.x[11]
            );
        }

        // Track the start_offset branch
        if pc == 0xC040B060 {
            eprintln!(
                "[{}] Reading start_offset from chunk: val=0x{:08X}",
                count, vm.cpu.x[24]
            ); // s4 will be loaded here
        }

        // Track init_IRQ
        if pc == 0xC0403C4C {
            eprintln!("[{}] init_IRQ entered", count);
        }

        // Track panic
        if pc == 0xC000252E {
            eprintln!(
                "[{}] PANIC called! a0=0x{:08X} (string ptr)",
                count, vm.cpu.x[10]
            );
            // Read the panic string
            let pa = vm.cpu.x[10] as u64;
            let mut s = Vec::new();
            for i in 0..80 {
                match vm.bus.read_byte(pa + i) {
                    Ok(b) if b > 0 && b < 128 => s.push(b as u8),
                    _ => break,
                }
            }
            eprintln!("[{}] Panic msg: \"{}\"", count, String::from_utf8_lossy(&s));
        }

        let _ = vm.step();
        count += 1;

        // Count ECALLs
        if pc == 0xC000252E && count > last_ecall + 1000 {
            last_ecall = count;
        }

        if vm.cpu.csr.scause != 0 && vm.cpu.csr.scause != 8 && vm.cpu.csr.scause != 9 {
            eprintln!(
                "[{}] UNEXPECTED scause={} stval=0x{:08X} PC=0x{:08X}",
                count, vm.cpu.csr.scause, vm.cpu.csr.stval, pc
            );
            break;
        }
    }

    eprintln!(
        "Final: {} instructions, scause={}",
        count, vm.cpu.csr.scause
    );
}
