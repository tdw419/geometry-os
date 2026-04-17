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
    let max: u64 = 177_400; // Trace past the SATP change
    let mut last_satp = vm.cpu.csr.satp;
    
    while count < max {
        if vm.bus.sbi.shutdown_requested { break; }
        
        let cur_satp = vm.cpu.csr.satp;
        let satp_changed = cur_satp != last_satp;
        
        // Trace everything from count 177310
        if count >= 177310 {
            let result = vm.step();
            let last = vm.cpu.last_step.as_ref().unwrap();
            let satp_note = if satp_changed { format!(" SATP_WAS=0x{:08X}", last_satp) } else { String::new() };
            let cur_satp2 = vm.cpu.csr.satp;
            eprintln!("{:6}: PC=0x{:08X} op={:?} len={} -> 0x{:08X} satp=0x{:08X}{} priv={:?}", 
                count, last.pc, last.op, last.inst_len, last.pc_after, cur_satp2, satp_note, vm.cpu.privilege);
            if satp_changed { last_satp = cur_satp2; }
        } else {
            let _ = vm.step();
            if vm.cpu.csr.satp != last_satp { last_satp = vm.cpu.csr.satp; }
        }
        
        count += 1;
    }
}
