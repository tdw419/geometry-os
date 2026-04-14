// Detailed fault diagnostic for Linux boot
// cargo run --example boot_debug_fault
use std::fs;
use geometry_os::riscv::RiscvVm;
use geometry_os::riscv::cpu::StepResult;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel = match fs::read(kernel_path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Error reading kernel: {}", e);
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
    let mut prev_pc: u32 = 0;
    let mut last_20_pcs: Vec<(u64, u32)> = Vec::new();
    let mut trap_count = 0u64;
    let mut ecall_count = 0u64;

    while count < max_instr {
        let pc = vm.cpu.pc;
        let prev_satp = vm.cpu.csr.satp;
        let step_result = vm.step();
        count += 1;

        // Detect trap delivery (mcause changed or PC jumped to trap vector)
        let mcause = vm.cpu.csr.mcause;
        if mcause != prev_mcause && mcause != 0 {
            trap_count += 1;

            // Only print interesting traps (not timer interrupts)
            let is_timer = (mcause & 0x80000000) != 0
                && ((mcause & 0xFF) == 5 || (mcause & 0xFF) == 7);

            if !is_timer && trap_count <= 20 {
                println!(
                    "[{}] TRAP: mcause=0x{:08X}, mepc=0x{:08X}, mtval=0x{:08X}, pc 0x{:08X}->0x{:08X}, priv={:?}",
                    count, mcause, vm.cpu.csr.mepc, vm.cpu.csr.mtval, pc, vm.cpu.pc, vm.cpu.privilege
                );
            }

            // Check for the critical fault: store/access fault to low address
            if !is_timer && (mcause == 7 || mcause == 5 || mcause == 3 || mcause == 1) {
                println!("\n=== CRITICAL TRAP at instruction {} ===", count);
                println!("  mcause: 0x{:08X}", mcause);
                println!("  mepc:   0x{:08X}", vm.cpu.csr.mepc);
                println!("  mtval:  0x{:08X}", vm.cpu.csr.mtval);
                println!("  mtvec:  0x{:08X}", vm.cpu.csr.mtvec);
                println!("  stvec:  0x{:08X}", vm.cpu.csr.stvec);
                println!("  satp:   0x{:08X}", vm.cpu.csr.satp);
                println!("  priv:   {:?}", vm.cpu.privilege);
                println!("  medeleg:0x{:08X}", vm.cpu.csr.medeleg);
                println!();

                // Print key registers
                println!("  Key registers:");
                for i in [1, 2, 8, 9, 10, 11, 15, 17, 28, 29, 30, 31] {
                    let names = ["zero","ra","sp","gp","tp","t0","t1","t2","s0","s1","a0","a1","a2","a3","a4","a5","a6","a7",
                                 "s2","s3","s4","s5","s6","s7","s8","s9","s10","s11","t3","t4","t5","t6"];
                    println!("    x{:2} ({:>3}): 0x{:08X}", i, names[i], vm.cpu.x[i]);
                }

                println!("\n  Last 20 PCs:");
                for &(c, p) in last_20_pcs.iter().rev() {
                    println!("    [{:>6}] 0x{:08X}", c, p);
                }

                // If PC went to 0, it's a fatal loop - stop
                if vm.cpu.pc == 0 || vm.cpu.pc < 0x100 {
                    println!("\n  FATAL: PC jumped to 0x{:08X} - infinite trap loop!", vm.cpu.pc);
                    break;
                }
            }
            prev_mcause = mcause;
        }

        // Track satp changes
        if vm.cpu.csr.satp != prev_satp && vm.cpu.csr.satp != 0 {
            println!(
                "[{}] SATP changed: 0x{:08X} -> 0x{:08X} (MMU enabled)",
                count, prev_satp, vm.cpu.csr.satp
            );
        }

        // Track ECALLs
        match step_result {
            StepResult::Ecall => {
                ecall_count += 1;
            }
            StepResult::Ebreak => {
                println!("[{}] EBREAK at 0x{:08X}", count, vm.cpu.pc);
                break;
            }
            StepResult::FetchFault | StepResult::LoadFault | StepResult::StoreFault => {
                println!("[{}] FAULT at 0x{:08X}", count, vm.cpu.pc);
                break;
            }
            _ => {}
        }

        last_20_pcs.push((count, pc));
        if last_20_pcs.len() > 20 {
            last_20_pcs.remove(0);
        }

        prev_pc = pc;

        // Detect infinite loop (PC stuck at same address)
        if vm.cpu.pc == 0 && prev_pc != 0 {
            println!("\nFATAL: PC = 0 at instruction {}", count);
            break;
        }
    }

    println!("\n=== Summary ===");
    println!("  Instructions executed: {}", count);
    println!("  Traps delivered: {}", trap_count);
    println!("  ECALLs: {}", ecall_count);
    println!("  Final PC: 0x{:08X}", vm.cpu.pc);
    println!("  Final privilege: {:?}", vm.cpu.privilege);
    println!("  Final satp: 0x{:08X}", vm.cpu.csr.satp);
    println!("  Final mcause: 0x{:08X}", vm.cpu.csr.mcause);
}
