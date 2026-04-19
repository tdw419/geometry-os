// Check: is the AND instruction working correctly?
// Also check the actual a0 value at pcpu_alloc_first_chunk entry
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
    while count < 15_560_000 {
        if vm.bus.sbi.shutdown_requested {
            break;
        }
        let _ = vm.step();
        count += 1;
    }

    // Watch at pcpu_alloc_first_chunk entry
    let mut step_count: u64 = 0;
    while step_count < 500_000 {
        let pc = vm.cpu.pc;
        let _ = vm.step();
        step_count += 1;
        count += 1;

        // Entry
        if pc == 0xC040AF6C {
            eprintln!(
                "[{}] ENTRY: a0=0x{:08X} a1=0x{:08X}",
                count, vm.cpu.x[10], vm.cpu.x[11]
            );
        }

        // After and s8, a0, a5
        if pc == 0xC040AF74 {
            eprintln!(
                "[{}] After AND: s8=0x{:08X} (should be a0 & 0xFFFFF000)",
                count, vm.cpu.x[24]
            );
        }

        // After sub s4, a0, s8
        if pc == 0xC040AF7E {
            eprintln!(
                "[{}] After SUB: s4=0x{:08X} (should be start_offset)",
                count, vm.cpu.x[20]
            );
        }

        // Right before jal
        if pc == 0xC040AFBE {
            eprintln!(
                "[{}] Before JAL: s8=0x{:08X} s4=0x{:08X}",
                count, vm.cpu.x[24], vm.cpu.x[20]
            );
        }

        if vm.cpu.csr.scause != 0 {
            break;
        }
    }
}
