// Focused boot trace: what happens at the transition point
// cargo run --example boot_focus
use geometry_os::riscv::cpu::StepResult;
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
    let (mut vm, result) = RiscvVm::boot_linux(
        &kernel,
        initramfs.as_deref(),
        512,
        0, // don't run any instructions in boot_linux
        bootargs,
    )
    .unwrap();

    println!(
        "Entry: 0x{:08X}, DTB: 0x{:08X}",
        result.entry, result.dtb_addr
    );
    println!("RAM base: 0x{:08X}", vm.bus.mem.ram_base);

    // Log all CSR writes and branches/jumps near the end
    let mut count: u64 = 0;
    let max_instr = 300_000u64;
    let mut prev_pc: u32 = 0;
    let mut satp_changed = false;
    let mut last_satp: u32 = 0;

    while count < max_instr {
        let pc = vm.cpu.pc;
        let satp = vm.cpu.csr.satp;

        // Log when PC changes from the BSS clear area
        if count > 250_000 && pc != prev_pc {
            let mtvec = vm.cpu.csr.read(geometry_os::riscv::csr::MTVEC);
            let stvec = vm.cpu.csr.read(geometry_os::riscv::csr::STVEC);
            let mstatus = vm.cpu.csr.mstatus;
            let privilege = vm.cpu.privilege;

            if pc < 0xC0001000 || pc > 0xC0001200 {
                println!("[{}] PC=0x{:08X} prev=0x{:08X} priv={:?} satp=0x{:08X} mtvec=0x{:08X} stvec=0x{:08X} mstatus=0x{:08X}",
                    count, pc, prev_pc, privilege, satp, mtvec, stvec, mstatus);
            }

            // Detect satp change
            if satp != last_satp && last_satp == 0 {
                println!(
                    "[{}] *** SATP CHANGED: 0x{:08X} -> 0x{:08X} at PC=0x{:08X}",
                    count, last_satp, satp, pc
                );
                satp_changed = true;
            }
            last_satp = satp;
        }

        prev_pc = pc;

        let step_result = vm.step();
        count += 1;

        if satp_changed && count > 255_000 {
            let pc = vm.cpu.pc;
            let mtvec = vm.cpu.csr.read(geometry_os::riscv::csr::MTVEC);
            let mcause = vm.cpu.csr.mcause;
            let mepc = vm.cpu.csr.mepc;
            println!(
                "[{}] PC=0x{:08X} mtvec=0x{:08X} mcause=0x{:08X} mepc=0x{:08X} priv={:?}",
                count, pc, mtvec, mcause, mepc, vm.cpu.privilege
            );
        }

        match step_result {
            StepResult::FetchFault => {
                println!("[{}] FETCH FAULT at PC=0x{:08X}", count, vm.cpu.pc);
                println!(
                    "  mcause=0x{:08X}, mepc=0x{:08X}, mtvec=0x{:08X}",
                    vm.cpu.csr.mcause,
                    vm.cpu.csr.mepc,
                    vm.cpu.csr.read(geometry_os::riscv::csr::MTVEC)
                );
                println!(
                    "  satp=0x{:08X}, mstatus=0x{:08X}",
                    vm.cpu.csr.satp, vm.cpu.csr.mstatus
                );
                println!(
                    "  stvec=0x{:08X}",
                    vm.cpu.csr.read(geometry_os::riscv::csr::STVEC)
                );
                break;
            }
            StepResult::Ebreak => {
                println!("[{}] EBREAK at PC=0x{:08X}", count, vm.cpu.pc);
                break;
            }
            _ => {}
        }

        // Stop if PC goes to 0
        if vm.cpu.pc == 0 {
            println!("[{}] PC jumped to 0x00000000!", count);
            println!("  prev_pc=0x{:08X}", prev_pc);
            println!(
                "  mcause=0x{:08X}, mepc=0x{:08X}",
                vm.cpu.csr.mcause, vm.cpu.csr.mepc
            );
            break;
        }
    }

    println!("\n=== Final State ===");
    println!("Instructions: {}", count);
    println!("PC: 0x{:08X}", vm.cpu.pc);
    println!("Privilege: {:?}", vm.cpu.privilege);
    println!(
        "mcause: 0x{:08X}, mepc: 0x{:08X}",
        vm.cpu.csr.mcause, vm.cpu.csr.mepc
    );
    println!(
        "mtvec: 0x{:08X}",
        vm.cpu.csr.read(geometry_os::riscv::csr::MTVEC)
    );
    println!(
        "stvec: 0x{:08X}",
        vm.cpu.csr.read(geometry_os::riscv::csr::STVEC)
    );
    println!("satp: 0x{:08X}", vm.cpu.csr.satp);
    println!("mstatus: 0x{:08X}", vm.cpu.csr.mstatus);

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
        if s.len() > 2048 {
            println!("... (truncated) ...{}", &s[s.len() - 2048..]);
        } else {
            println!("{}", s);
        }
    } else {
        println!("\nNo UART output");
    }
}
