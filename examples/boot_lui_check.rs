// Read the actual instruction bytes at PC 0xC040AF6E and decode
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

    // Read the instruction bytes at VA 0xC040AF6E (PA 0x040AF6E)
    let pa: u64 = 0x040AF6E;
    let instr_half = vm.bus.read_word(pa).unwrap(); // reads 4 bytes but we only need 2
    let byte0 = (instr_half >> 24) & 0xFF;
    let byte1 = (instr_half >> 16) & 0xFF;
    let byte2 = (instr_half >> 8) & 0xFF;
    let byte3 = instr_half & 0xFF;
    
    // Actually, read_word reads a 32-bit value at the given address
    // In little-endian, bytes at pa are [b0, b1, b2, b3]
    // read_word returns them as b0 | (b1 << 8) | (b2 << 16) | (b3 << 24)
    let b0 = instr_half & 0xFF;
    let b1 = (instr_half >> 8) & 0xFF;
    let c_instr = b0 as u16 | ((b1 as u16) << 8);
    
    eprintln!("Bytes at 0x040AF6E: {:02X} {:02X} {:02X} {:02X}", b0, b1, 
        (instr_half >> 16) & 0xFF, (instr_half >> 24) & 0xFF);
    eprintln!("16-bit instruction: 0x{:04X}", c_instr);
    eprintln!("Binary: {:016b}", c_instr);
    
    // Decode
    let opcode = c_instr & 0x3;
    let funct3 = (c_instr >> 13) & 0x7;
    eprintln!("opcode quadrant: {} funct3: {}", opcode, funct3);
    
    if opcode == 1 && funct3 == 3 {
        // C.LUI
        let rd = (c_instr >> 7) & 0x1F;
        let nzimm5 = (c_instr >> 12) & 1;
        let nzimm_low = (c_instr >> 2) & 0x1F;
        let imm = ((nzimm5 as u32) << 17) | ((nzimm_low as u32) << 12);
        eprintln!("C.LUI: rd=x{} imm=0x{:08X} (result=0x{:08X})", rd, imm, imm);
    }
    
    // Also check: what does our CPU actually do with this instruction?
    // Run to just before it
    let mut count: u64 = 0;
    while count < 15_560_000 {
        if vm.bus.sbi.shutdown_requested { break; }
        let _ = vm.step();
        count += 1;
    }
    
    while count < 16_000_000 {
        let pc = vm.cpu.pc;
        if pc == 0xC040AF6E {
            let a5_before = vm.cpu.x[15];
            eprintln!("Before instr at 0xC040AF6E: a5=0x{:08X}", a5_before);
            let _ = vm.step();
            let a5_after = vm.cpu.x[15];
            eprintln!("After: a5=0x{:08X}", a5_after);
            break;
        }
        let _ = vm.step();
        count += 1;
    }
}
