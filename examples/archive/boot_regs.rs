// Comprehensive register dump at fault time
// cargo run --example boot_regs
use geometry_os::riscv::RiscvVm;
use std::fs;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel = match fs::read(kernel_path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };
    let initramfs = fs::read(initramfs_path).ok();

    let bootargs = "console=ttyS0 earlycon=sbi panic=5 quiet";
    let (mut vm, _result) =
        RiscvVm::boot_linux(&kernel, initramfs.as_deref(), 512, 0, bootargs).unwrap();

    let mut count: u64 = 0;
    let max_instr = 300_000u64;
    let mut prev_mcause: u32 = 0;

    while count < max_instr {
        let _pc = vm.cpu.pc;
        vm.step();
        count += 1;

        let mcause = vm.cpu.csr.mcause;
        if mcause != prev_mcause && mcause != 0 {
            let is_timer = (mcause & 0x80000000) != 0;
            if !is_timer {
                println!("TRAP at instruction {}: mcause=0x{:08X}", count, mcause);
                println!(
                    "  mepc=0x{:08X} mtval=0x{:08X} mtvec=0x{:08X}",
                    vm.cpu.csr.mepc, vm.cpu.csr.mtval, vm.cpu.csr.mtvec
                );
                println!(
                    "  stvec=0x{:08X} satp=0x{:08X} priv={:?}",
                    vm.cpu.csr.stvec, vm.cpu.csr.satp, vm.cpu.privilege
                );
                println!(
                    "  medeleg=0x{:08X} mideleg=0x{:08X}",
                    vm.cpu.csr.medeleg, vm.cpu.csr.mideleg
                );
                println!();
                let names = [
                    "zero", "ra", "sp", "gp", "tp", "t0", "t1", "t2", "s0", "s1", "a0", "a1", "a2",
                    "a3", "a4", "a5", "a6", "a7", "s2", "s3", "s4", "s5", "s6", "s7", "s8", "s9",
                    "s10", "s11", "t3", "t4", "t5", "t6",
                ];
                for i in 0..32 {
                    println!("  x{:2} ({:>3}): 0x{:08X}", i, names[i], vm.cpu.x[i]);
                }

                // Decode the faulting instruction
                let mepc = vm.cpu.csr.mepc;
                if let Ok(inst) = vm.bus.read_word(mepc as u64) {
                    let opcode = inst & 0x7F;
                    let rd = (inst >> 7) & 0x1F;
                    let funct3 = (inst >> 12) & 0x7;
                    let rs1 = (inst >> 15) & 0x1F;
                    let rs2 = (inst >> 20) & 0x1F;
                    let funct7 = (inst >> 25) & 0x7F;

                    println!(
                        "\n  Faulting instruction at mepc=0x{:08X}: 0x{:08X}",
                        mepc, inst
                    );
                    println!(
                        "  opcode=0x{:02X} rd=x{} funct3={} rs1=x{} rs2=x{} funct7=0x{:02X}",
                        opcode, rd, funct3, rs1, rs2, funct7
                    );

                    if opcode == 0x23 {
                        // Store
                        let imm = ((funct7 << 5) | rd) as i32 as i64;
                        let rs1_usize = rs1 as usize;
                        let rs2_usize = rs2 as usize;
                        let store_addr = (vm.cpu.x[rs1_usize] as i64 + imm) as u64;
                        println!(
                            "  SW x{}, {}(x{}) -> addr = 0x{:X} + {} = 0x{:X}",
                            rs2, imm, rs1, vm.cpu.x[rs1_usize], imm, store_addr
                        );
                        println!(
                            "  x{} = 0x{:08X}, value to store (x{}) = 0x{:08X}",
                            rs1, vm.cpu.x[rs1_usize], rs2, vm.cpu.x[rs2_usize]
                        );
                    }
                }
                break;
            }
            prev_mcause = mcause;
        }

        if vm.cpu.pc == 0 {
            println!("PC=0 at instruction {}", count);
            break;
        }
    }
}
