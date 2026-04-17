use geometry_os::riscv::RiscvVm;
use geometry_os::riscv::cpu::StepResult;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();

    let (mut vm, _fw, _entry, _dtb) = RiscvVm::boot_linux_setup(
        &kernel_image,
        initramfs.as_deref(),
        256,
        "loglevel=0 quiet",
    ).unwrap();

    // Patch kernel
    let sw_a5_12 = vm.bus.read_half(0x0040495E).unwrap_or(0);
    let sw_a1_20 = vm.bus.read_half(0x00404968).unwrap_or(0);
    if sw_a5_12 == 0xC4DC && sw_a1_20 == 0xC8CC {
        vm.bus.write_half(0x0040495E, 0x0001).ok();
        vm.bus.write_half(0x00404968, 0x0001).ok();
        vm.bus.write_word(0x00C79E90 + 12, 0).ok();
        vm.bus.write_word(0x00C79E90 + 20, 0xC0000000).ok();
        vm.bus.auto_pte_fixup = false;
        vm.bus.known_pt_pages.clear();
    }

    let mut count: u64 = 0;
    let target: u64 = 742_699;
    
    while count < target - 20 {
        let _ = vm.step();
        count += 1;
    }
    
    while count < target + 20 {
        let pc = vm.cpu.pc;
        let satp = vm.cpu.csr.satp;
        let result = vm.step();
        let last = vm.cpu.last_step.as_ref();
        eprintln!("{:7}: PC=0x{:08X} priv={:?} satp=0x{:08X} -> PC=0x{:08X} result={:?} op={:?}", 
            count, pc, vm.cpu.privilege, satp, vm.cpu.pc, result, 
            last.map(|l| l.op.clone()).unwrap_or(geometry_os::riscv::decode::Operation::Nop));
        count += 1;
    }
}
