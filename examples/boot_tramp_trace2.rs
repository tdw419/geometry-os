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
    // Run to 178505, then trace
    while count < 178505 {
        let _ = vm.step();
        count += 1;
    }
    
    // Now trace 30 instructions
    for i in 0..30 {
        let pc = vm.cpu.pc;
        let result = vm.step();
        let last = vm.cpu.last_step.as_ref().unwrap();
        eprintln!("{:6}: PC=0x{:08X} op={:?} -> 0x{:08X} priv={:?} result={:?}", 
            count + i as u64, pc, last.op, vm.cpu.pc, vm.cpu.privilege, result);
        if vm.cpu.pc == 0x3FFFF000 || vm.cpu.pc == 0x3FFFF002 {
            eprintln!("  >>> REACHED 0x3FFFF000! x1={:08X} x5={:08X}", vm.cpu.x[1], vm.cpu.x[5]);
            // Dump regs
            for r in 0..32 {
                if vm.cpu.x[r] != 0 {
                    eprintln!("  x{}=0x{:08X}", r, vm.cpu.x[r]);
                }
            }
        }
    }
}
