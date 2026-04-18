use geometry_os::riscv::RiscvVm;
use std::fs;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = fs::read(kernel_path).unwrap();
    let initramfs = fs::read(initramfs_path).ok();

    let (mut vm, _fw_addr, entry, dtb_addr) = RiscvVm::boot_linux_setup(
        &kernel_image,
        initramfs.as_deref(),
        256,
        "console=ttyS0 earlycon=sbi panic=1 quiet",
    )
    .unwrap();

    // Verify patches
    println!("=== Verify Kernel Patches ===");
    let val_10da = vm.bus.read_word(0x10DA).unwrap_or(0);
    let val_10de = vm.bus.read_word(0x10DE).unwrap_or(0);
    let val_10f6 = vm.bus.read_word(0x10F6).unwrap_or(0);
    let val_10fa = vm.bus.read_word(0x10FA).unwrap_or(0);
    println!(
        "PA 0x10DA (auipc a3): 0x{:08X} (expect 0x00210197)",
        val_10da
    );
    println!(
        "PA 0x10DE (addi a3):  0x{:08X} (expect 0xE3A68693)",
        val_10de
    );
    println!(
        "PA 0x10F6 (JAL->NOP): 0x{:08X} (expect 0x00000013)",
        val_10f6
    );
    println!(
        "PA 0x10FA (JAL->NOP): 0x{:08X} (expect 0x00000013)",
        val_10fa
    );

    let max = 2_000_000;
    let report_interval = 200_000;
    let mut count = 0u64;
    let mut last_pc = 0u32;
    let mut spin_count = 0u32;
    let mut ecall_count = 0u64;
    let mut trap_count = 0u64;

    while count < max {
        let pc_before = vm.cpu.pc;
        let _ = vm.step();
        count += 1;

        // Detect ECALL (PC doesn't advance, instruction is ECALL)
        if vm.cpu.pc == pc_before {
            let instr = vm.bus.read_word(vm.cpu.pc as u64).unwrap_or(0);
            if instr & 0x7F == 0x73 {
                ecall_count += 1;
                if ecall_count <= 5 {
                    println!(
                        "ECALL #{} at PC=0x{:08X} instr=0x{:08X}",
                        ecall_count, vm.cpu.pc, instr
                    );
                }
            }
        }

        // Detect spin loops
        if vm.cpu.pc == last_pc {
            spin_count += 1;
            if spin_count > 50000 {
                println!("\nSPIN at PC=0x{:08X} after {} instrs", vm.cpu.pc, count);
                // Disassemble around PC
                for i in -4..4i64 {
                    let addr = (vm.cpu.pc as i64 + i * 4) as u64;
                    let instr = vm.bus.read_word(addr).unwrap_or(0);
                    println!("  0x{:08X}: 0x{:08X}", addr, instr);
                }
                break;
            }
        } else {
            spin_count = 0;
        }
        last_pc = vm.cpu.pc;

        if count % report_interval == 0 {
            println!(
                "After {} instrs: PC=0x{:08X} SP=0x{:08X} stvec=0x{:08X} ecalls={}",
                count,
                vm.cpu.pc,
                vm.cpu.x[2],
                vm.cpu.csr.read(0x005),
                ecall_count
            );
        }
    }

    println!("\n=== FINAL STATE ===");
    println!("Instructions: {}", count);
    println!("PC: 0x{:08X}", vm.cpu.pc);
    println!("SP: 0x{:08X}", vm.cpu.x[2]);
    println!("STVEC: 0x{:08X}", vm.cpu.csr.read(0x005));
    println!("SATP: 0x{:08X}", vm.cpu.csr.satp);
    println!("ECALLs: {}", ecall_count);

    let uart_bytes = vm.bus.uart.drain_tx();
    let uart_str: String = uart_bytes.iter().map(|&b| b as char).collect();
    if !uart_str.is_empty() {
        println!("\nUART output ({} bytes):", uart_bytes.len());
        // Show up to 1000 chars
        let show = &uart_str[..uart_str.len().min(1000)];
        println!("{}", show);
    } else {
        println!("\nNo UART output");
    }
}
