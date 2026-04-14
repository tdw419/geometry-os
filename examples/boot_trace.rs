// Step-by-step Linux boot diagnostic with progress
// cargo run --example boot_trace
use std::fs;
use geometry_os::riscv::RiscvVm;
use geometry_os::riscv::cpu::StepResult;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel = match fs::read(kernel_path) {
        Ok(d) => d,
        Err(e) => { eprintln!("Error: {}", e); std::process::exit(1); }
    };
    let initramfs = fs::read(initramfs_path).ok();

    println!("=== Linux Boot Trace ===");
    println!("Kernel: {} bytes", kernel.len());

    let bootargs = "console=ttyS0 earlycon=sbi panic=5 quiet";
    let (mut vm, result) = RiscvVm::boot_linux(
        &kernel,
        initramfs.as_deref(),
        512,
        0, // 0 = don't run any instructions in boot_linux
        bootargs,
    ).unwrap();

    println!("Entry: 0x{:08X}, DTB: 0x{:08X}", result.entry, result.dtb_addr);
    println!("RAM base: 0x{:08X}", vm.bus.mem.ram_base);

    let max_instr = 20_000_000u64;
    let mut count = 0u64;
    let mut last_pc: u32 = 0xFFFFFFFF;
    let mut pc_loop_count = 0u32;
    let mut fault_count = 0u64;
    let _last_report_pc = vm.cpu.pc;
    let start = std::time::Instant::now();

    // Report milestones
    let milestones: Vec<u64> = vec![
        100, 1000, 10000, 100000, 500000,
        1_000_000, 2_000_000, 5_000_000, 10_000_000, 20_000_000
    ];
    let mut milestone_idx = 0;

    while count < max_instr {
        let _pc_before = vm.cpu.pc;
        let step_result = vm.step();
        count += 1;

        // Detect PC loops (same PC twice = trap loop)
        if vm.cpu.pc == last_pc {
            pc_loop_count += 1;
            if pc_loop_count > 100 {
                println!("\nPC loop detected at 0x{:08X} after {} steps", vm.cpu.pc, count);
                println!("  mcause: 0x{:08X}, mepc: 0x{:08X}",
                         vm.cpu.csr.mcause, vm.cpu.csr.mepc);
                println!("  satp: 0x{:08X}, mstatus: 0x{:08X}",
                         vm.cpu.csr.satp, vm.cpu.csr.mstatus);
                println!("  mtvec: 0x{:08X}", vm.cpu.csr.read(geometry_os::riscv::csr::MTVEC));
                break;
            }
        } else {
            pc_loop_count = 0;
        }
        last_pc = vm.cpu.pc;

        // Check milestones
        while milestone_idx < milestones.len() && count >= milestones[milestone_idx] {
            let elapsed = start.elapsed();
            let ips = count as f64 / elapsed.as_secs_f64();
            println!("[{:>8}] PC=0x{:08X} priv={:?} satp=0x{:08X} mcause=0x{:08X} ({:.0} instr/s)",
                     count, vm.cpu.pc, vm.cpu.privilege, vm.cpu.csr.satp,
                     vm.cpu.csr.mcause, ips);
            milestone_idx += 1;
        }

        match step_result {
            StepResult::Ebreak => {
                println!("\nEBREAK at PC=0x{:08X} after {} instructions", vm.cpu.pc, count);
                break;
            }
            StepResult::FetchFault => {
                fault_count += 1;
                println!("\nFetchFault at PC=0x{:08X} after {} instructions (fault #{})",
                         vm.cpu.pc, count, fault_count);
                println!("  mcause: 0x{:08X}, mepc: 0x{:08X}",
                         vm.cpu.csr.mcause, vm.cpu.csr.mepc);
                break;
            }
            StepResult::LoadFault => {
                fault_count += 1;
                if fault_count <= 5 {
                    println!("  LoadFault at PC=0x{:08X} after {} steps",
                             vm.cpu.pc, count);
                }
            }
            StepResult::StoreFault => {
                fault_count += 1;
                if fault_count <= 5 {
                    println!("  StoreFault at PC=0x{:08X} after {} steps",
                             vm.cpu.pc, count);
                }
            }
            StepResult::Ecall => {
                // Normal during boot, but log first few
                if count < 200_000 {
                    println!("  ECALL at PC=0x{:08X} after {} steps, a7=0x{:08X}",
                             vm.cpu.pc, count, vm.cpu.x[17]);
                }
            }
            StepResult::Ok => {}
        }
    }

    let elapsed = start.elapsed();
    println!("\n=== Summary ===");
    println!("Instructions: {} in {:?}", count, elapsed);
    println!("Final PC: 0x{:08X}", vm.cpu.pc);
    println!("Privilege: {:?}", vm.cpu.privilege);
    println!("mcause: 0x{:08X}, mepc: 0x{:08X}", vm.cpu.csr.mcause, vm.cpu.csr.mepc);
    println!("satp: 0x{:08X}, mstatus: 0x{:08X}", vm.cpu.csr.satp, vm.cpu.csr.mstatus);
    println!("mtvec: 0x{:08X}", vm.cpu.csr.read(geometry_os::riscv::csr::MTVEC));
    println!("Faults: {}", fault_count);
    println!("MIPS: {:.2}", count as f64 / elapsed.as_secs_f64() / 1_000_000.0);

    // Check UART
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
        // Print last 2KB to avoid flooding
        if s.len() > 2048 {
            println!("... (truncated) ...{}", &s[s.len()-2048..]);
        } else {
            println!("{}", s);
        }
    } else {
        println!("\nNo UART output");
    }
}
