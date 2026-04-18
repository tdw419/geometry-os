use geometry_os::riscv::RiscvVm;
use std::fs;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = fs::read(kernel_path).unwrap();
    let initramfs = fs::read(initramfs_path).ok();

    let (mut vm, _fw_addr, _entry, _dtb_addr) = RiscvVm::boot_linux_setup(
        &kernel_image,
        initramfs.as_deref(),
        256,
        "console=ttyS0 earlycon=sbi panic=1 quiet",
    )
    .unwrap();

    let max = 200_000;
    let mut count = 0u64;
    let mut last_pc = 0u32;
    let mut spin_count = 0u32;

    while count < max {
        let pc = vm.cpu.pc;
        if pc == 0xC00010D8 {
            // Execute mv a0, a1
            let _ = vm.step();
            count += 1;
            assert_eq!(vm.cpu.pc, 0xC00010DA);

            println!("[{}] At auipc PC=0x{:08X}", count, vm.cpu.pc);
            println!("Privilege: {:?}", vm.cpu.privilege);
            println!("SATP: 0x{:08X}", vm.cpu.csr.satp);
            println!("a3 before: 0x{:08X}", vm.cpu.x[13]);

            // Execute the auipc and check last_step
            let _ = vm.step();
            count += 1;

            if let Some(ref ls) = vm.cpu.last_step {
                println!("Fetched word: 0x{:08X}", ls.word);
                println!("PC before: 0x{:08X}", ls.pc);
                println!("PC after: 0x{:08X}", ls.pc_after);
                println!("Inst len: {}", ls.inst_len);
            }
            println!("a3 after: 0x{:08X}", vm.cpu.x[13]);

            // Execute addi and check
            let _ = vm.step();
            count += 1;
            if let Some(ref ls) = vm.cpu.last_step {
                println!("\nAddi fetched word: 0x{:08X}", ls.word);
                println!("PC before: 0x{:08X}", ls.pc);
            }
            println!("a3 after addi: 0x{:08X}", vm.cpu.x[13]);

            break;
        }

        let _ = vm.step();
        count += 1;

        if vm.cpu.pc == last_pc {
            spin_count += 1;
            if spin_count > 50000 {
                break;
            }
        } else {
            spin_count = 0;
        }
        last_pc = vm.cpu.pc;
    }
}
