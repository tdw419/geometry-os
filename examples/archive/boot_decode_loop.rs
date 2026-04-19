fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1";

    use geometry_os::riscv::cpu::StepResult;
    use geometry_os::riscv::RiscvVm;

    let (mut vm, _) =
        RiscvVm::boot_linux(&kernel_image, initramfs.as_deref(), 256, 1, bootargs).unwrap();

    // Run to the loop
    for _ in 0..100_000 {
        let _ = vm.step();
    }

    // Now trace 10 instructions with full register state
    for i in 0..10 {
        let pc = vm.cpu.pc;
        let insn = vm.bus.read_word(pc as u64).unwrap_or(0);
        println!("--- Step {} ---", i);
        println!(
            "  PC=0x{:08X} insn=0x{:08X} priv={:?}",
            pc, insn, vm.cpu.privilege
        );
        println!(
            "  x[10](a0)=0x{:08X} x[11](a1)=0x{:08X} x[12](a2)=0x{:08X}",
            vm.cpu.x[10], vm.cpu.x[11], vm.cpu.x[12]
        );
        println!(
            "  x[13](a3)=0x{:08X} x[14](a4)=0x{:08X} x[15](a5)=0x{:08X}",
            vm.cpu.x[13], vm.cpu.x[14], vm.cpu.x[15]
        );

        // Try to decode the instruction manually
        let opcode = insn & 0x7F;
        let rd = ((insn >> 7) & 0x1F) as usize;
        let funct3 = (insn >> 12) & 0x7;
        let rs1 = ((insn >> 15) & 0x1F) as usize;
        let rs2 = ((insn >> 20) & 0x1F) as usize;
        let funct7 = (insn >> 25) & 0x7F;

        match opcode {
            0x23 => {
                // S-type (store)
                let imm = (((insn >> 25) as i32) << 5 | ((insn >> 7) & 0x1F) as i32) as i32;
                println!(
                    "  STORE: sw x[{}], {}(x[{}]) -- store 0x{:08X} to 0x{:08X}",
                    rs2,
                    imm,
                    rs1,
                    vm.cpu.x[rs2],
                    (vm.cpu.x[rs1] as i32 + imm) as u32
                );
            }
            0x13 => {
                // I-type (addi etc)
                let imm = ((insn as i32) >> 20) as i32;
                let op = match funct3 {
                    0 => "addi",
                    _ => "op",
                };
                println!(
                    "  {}: x[{}] = x[{}] + {} = 0x{:08X}",
                    op,
                    rd,
                    rs1,
                    imm,
                    (vm.cpu.x[rs1] as i32 + imm) as u32
                );
            }
            0x33 => {
                // R-type
                let op = match (funct7, funct3) {
                    (0, 0) => "add",
                    (1, 0) => "mul",
                    _ => "alu",
                };
                println!("  {}: x[{}] = x[{}] {:?} x[{}]", op, rd, rs1, funct3, rs2);
            }
            _ => println!("  opcode=0x{:02X}", opcode),
        }

        let _ = vm.step();
    }
}
