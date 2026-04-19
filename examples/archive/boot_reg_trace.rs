// Trace registers around the boot fault transition
// Focus on x1 (ra), x10-x17 (a0-a7) to understand the C.JR x1 → 0 jump
use geometry_os::riscv::cpu::StepResult;
use geometry_os::riscv::RiscvVm;
use std::fs;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel = fs::read(kernel_path).unwrap();
    let initramfs = fs::read(initramfs_path).ok();

    let bootargs = "console=ttyS0 earlycon=sbi panic=5 quiet";
    let (mut vm, result) =
        RiscvVm::boot_linux(&kernel, initramfs.as_deref(), 512, 0, bootargs).unwrap();

    println!(
        "Entry: 0x{:08X}, DTB: 0x{:08X}",
        result.entry, result.dtb_addr
    );

    // Run until PC jumps to low address or fault
    let max_instr = 300_000u64;
    let mut count = 0u64;
    let mut history: Vec<(u64, u32, u32, [u32; 18])> = Vec::new(); // step, pc, instr, regs[0..17]
    let history_size = 30;

    while count < max_instr {
        let pc_before = vm.cpu.pc;
        let instr_word = vm.bus.mem.read_word(pc_before as u64).unwrap_or(0);

        // Snapshot key registers
        let regs: [u32; 18] = std::array::from_fn(|i| vm.cpu.x[i]);

        let step_result = vm.step();
        count += 1;

        history.push((count, pc_before, instr_word, regs));
        if history.len() > history_size {
            history.remove(0);
        }

        // Detect PC jump to low address
        if vm.cpu.pc < 0x10000 && pc_before >= 0xC0000000 {
            println!(
                "\n=== PC jumped from 0x{:08X} to 0x{:08X} at step {} ===",
                pc_before, vm.cpu.pc, count
            );
            println!(
                "satp=0x{:08X} mstatus=0x{:08X}",
                vm.cpu.csr.satp, vm.cpu.csr.mstatus
            );
            println!(
                "mtvec=0x{:08X} stvec=0x{:08X}",
                vm.cpu.csr.read(geometry_os::riscv::csr::MTVEC),
                vm.cpu.csr.read(geometry_os::riscv::csr::STVEC)
            );
            println!("\nLast {} instructions:", history_size);
            println!(
                "{:>6}  {:>10}  {:>10}  x1(ra)   a0(x10)  a1(x11)  a2(x12)  a7(x17)",
                "step", "PC", "instr"
            );
            for (step, pc, word, regs) in &history {
                println!(
                    "{:>6}  0x{:08X}  0x{:08X}  0x{:08X}  0x{:08X}  0x{:08X}  0x{:08X}  0x{:08X}",
                    step, pc, word, regs[1], regs[10], regs[11], regs[12], regs[17]
                );
            }
            break;
        }

        if matches!(step_result, StepResult::FetchFault) {
            println!("\n=== FetchFault at step {} ===", count);
            break;
        }

        // Detect trap loop
        if vm.cpu.pc == pc_before && count > 255000 {
            let loop_count = history
                .iter()
                .rev()
                .take_while(|(_, pc, _, _)| *pc == vm.cpu.pc)
                .count();
            if loop_count >= 100 {
                println!("\n=== Trap loop at 0x{:08X} ===", vm.cpu.pc);
                println!(
                    "mcause=0x{:08X} mepc=0x{:08X}",
                    vm.cpu.csr.mcause, vm.cpu.csr.mepc
                );
                break;
            }
        }
    }

    println!(
        "\n=== Final: {} steps, PC=0x{:08X}, mcause=0x{:08X} ===",
        count, vm.cpu.pc, vm.cpu.csr.mcause
    );

    // UART output
    let mut out = Vec::new();
    loop {
        match vm.bus.uart.read_byte(0) {
            0 => break,
            b => out.push(b),
        }
    }
    if !out.is_empty() {
        println!(
            "\nUART ({} bytes): {}",
            out.len(),
            String::from_utf8_lossy(&out)
        );
    }
}
