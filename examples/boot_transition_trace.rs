// Trace the exact transition from BSS clear to the fault
// Captures instructions around the PC jump to 0x00000000
use std::fs;
use geometry_os::riscv::RiscvVm;
use geometry_os::riscv::cpu::StepResult;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel = fs::read(kernel_path).unwrap();
    let initramfs = fs::read(initramfs_path).ok();

    let bootargs = "console=ttyS0 earlycon=sbi panic=5 quiet";
    let (mut vm, result) = RiscvVm::boot_linux(
        &kernel,
        initramfs.as_deref(),
        512,
        0,
        bootargs,
    ).unwrap();

    println!("Entry: 0x{:08X}, DTB: 0x{:08X}", result.entry, result.dtb_addr);
    println!("RAM base: 0x{:08X}", vm.bus.mem.ram_base);

    // Track the last N instructions before a PC jump to low addresses
    let max_instr = 300_000u64;
    let mut count = 0u64;
    let mut history: Vec<(u64, u32, u32, u32)> = Vec::new(); // (step, pc, instruction_word, x[5])
    let history_size = 20;

    while count < max_instr {
        let pc_before = vm.cpu.pc;
        let step_result = vm.step();
        count += 1;

        // Record history
        let instr_word = vm.bus.mem.read_word(pc_before as u64).unwrap_or(0);
        history.push((count, pc_before, instr_word, vm.cpu.x[5]));
        if history.len() > history_size {
            history.remove(0);
        }

        // Detect when PC jumps to a low address (< 0x10000) or we get a fault
        if vm.cpu.pc < 0x10000 && pc_before >= 0xC0000000 {
            println!("\n=== PC jumped from 0x{:08X} to 0x{:08X} at step {} ===",
                     pc_before, vm.cpu.pc, count);
            println!("Instruction at 0x{:08X}: 0x{:08X}", pc_before, instr_word);
            println!("mcause: 0x{:08X}, mepc: 0x{:08X}",
                     vm.cpu.csr.mcause, vm.cpu.csr.mepc);
            println!("satp: 0x{:08X}, mstatus: 0x{:08X}",
                     vm.cpu.csr.satp, vm.cpu.csr.mstatus);
            println!("mtvec: 0x{:08X}, stvec: 0x{:08X}",
                     vm.cpu.csr.read(geometry_os::riscv::csr::MTVEC),
                     vm.cpu.csr.read(geometry_os::riscv::csr::STVEC));
            println!("\nLast {} instructions before jump:", history_size);
            for (step, pc, word, x5) in &history {
                println!("  [{:>6}] PC=0x{:08X} instr=0x{:08X} x5=0x{:08X}", step, pc, word, x5);
            }
            break;
        }

        // Also detect FetchFault
        if matches!(step_result, StepResult::FetchFault) {
            println!("\n=== FetchFault at step {} ===", count);
            println!("PC before: 0x{:08X}, PC after: 0x{:08X}", pc_before, vm.cpu.pc);
            println!("mcause: 0x{:08X}, mepc: 0x{:08X}",
                     vm.cpu.csr.mcause, vm.cpu.csr.mepc);
            println!("\nLast {} instructions:", history_size);
            for (step, pc, word, x5) in &history {
                println!("  [{:>6}] PC=0x{:08X} instr=0x{:08X} x5=0x{:08X}", step, pc, word, x5);
            }
            break;
        }

        // Detect PC loop (same PC 100 times in a row)
        if vm.cpu.pc == pc_before {
            let loop_count = history.iter().rev()
                .take_while(|(_, pc, _, _)| *pc == vm.cpu.pc)
                .count();
            if loop_count >= 100 {
                println!("\n=== PC loop at 0x{:08X} after {} steps ===", vm.cpu.pc, count);
                println!("mcause: 0x{:08X}, mepc: 0x{:08X}",
                         vm.cpu.csr.mcause, vm.cpu.csr.mepc);
                break;
            }
        }
    }

    println!("\n=== Final state after {} steps ===", count);
    println!("PC: 0x{:08X}", vm.cpu.pc);
    println!("Privilege: {:?}", vm.cpu.privilege);
    println!("mcause: 0x{:08X}, mepc: 0x{:08X}", vm.cpu.csr.mcause, vm.cpu.csr.mepc);
    println!("satp: 0x{:08X}", vm.cpu.csr.satp);
    println!("mstatus: 0x{:08X}", vm.cpu.csr.mstatus);
    println!("mtvec: 0x{:08X}", vm.cpu.csr.read(geometry_os::riscv::csr::MTVEC));
    println!("stvec: 0x{:08X}", vm.cpu.csr.read(geometry_os::riscv::csr::STVEC));

    // UART output
    let mut out = Vec::new();
    loop {
        match vm.bus.uart.read_byte(0) {
            0 => break,
            b => out.push(b),
        }
    }
    if !out.is_empty() {
        println!("\n=== UART Output ({} bytes) ===", out.len());
        let s = String::from_utf8_lossy(&out);
        println!("{}", s);
    }
}
