use std::fs;
use geometry_os::riscv::RiscvVm;
use geometry_os::riscv::decode;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = fs::read(kernel_path).unwrap();
    let initramfs = fs::read(initramfs_path).ok();
    
    let (mut vm, _fw_addr, _entry, _dtb_addr) = RiscvVm::boot_linux_setup(
        &kernel_image, initramfs.as_deref(), 256,
        "console=ttyS0 earlycon=sbi panic=1 quiet",
    ).unwrap();
    
    let max = 200_000;
    let mut count = 0u64;
    let mut last_pc = 0u32;
    let mut spin_count = 0u32;
    
    while count < max {
        let pc = vm.cpu.pc;
        if pc == 0xC00010D8 {
            let _ = vm.step(); count += 1; // mv a0, a1
            assert_eq!(vm.cpu.pc, 0xC00010DA);
            
            // Manually decode the instruction
            let word = 0x00210197u32;
            let opcode = word & 0x7F;
            let rd = ((word >> 7) & 0x1F) as u8;
            let imm = word & 0xFFFF_F000;
            println!("Manual decode of 0x{:08X}: opcode=0x{:02X} rd={} imm=0x{:08X}", 
                word, opcode, rd, imm);
            println!("  auipc result: PC(0x{:08X}) + imm(0x{:08X}) = 0x{:08X}",
                pc, imm, pc.wrapping_add(imm));
            
            let op = decode::decode(word);
            println!("  Decoded operation: {:?}", op);
            
            // Check if maybe the fetched word is different
            // Read what the MMU gives us for this PC
            // We can't call translate_va (private), but we can check after step
            println!("\nBefore step: a3=0x{:08X} PC=0x{:08X}", vm.cpu.x[13], vm.cpu.pc);
            
            let _ = vm.step(); count += 1;
            
            println!("After step:  a3=0x{:08X} PC=0x{:08X}", vm.cpu.x[13], vm.cpu.pc);
            
            if let Some(ref ls) = vm.cpu.last_step {
                println!("  Fetched word: 0x{:08X}", ls.word);
                println!("  Operation: {:?}", ls.op);
                println!("  Inst len: {}", ls.inst_len);
                println!("  regs_before[13]: 0x{:08X}", ls.regs_before[13]);
                println!("  regs_after[13]: 0x{:08X}", ls.regs_after[13]);
                println!("  Result: {:?}", ls.result);
            }
            break;
        }
        
        let _ = vm.step();
        count += 1;
        
        if vm.cpu.pc == last_pc {
            spin_count += 1;
            if spin_count > 50000 { break; }
        } else { spin_count = 0; }
        last_pc = vm.cpu.pc;
    }
}
