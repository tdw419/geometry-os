// Boot debug: trace the exact instruction that faults
// cargo run --example boot_debug2
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

    let mut count: u64 = 0;
    let max_instr = 260_000u64;

    while count < max_instr {
        let pc = vm.cpu.pc;
        let step_result = vm.step();
        count += 1;

        // After step, check if PC jumped to 0
        if vm.cpu.pc == 0 && pc != 0 {
            println!("[{}] PC jumped from 0x{:08X} to 0x{:08X}", count, pc, vm.cpu.pc);
            println!("  mcause=0x{:08X}, mepc=0x{:08X}", vm.cpu.csr.mcause, vm.cpu.csr.mepc);
            println!("  mtvec=0x{:08X}, stvec=0x{:08X}",
                vm.cpu.csr.read(geometry_os::riscv::csr::MTVEC),
                vm.cpu.csr.read(geometry_os::riscv::csr::STVEC));
            println!("  satp=0x{:08X}, mstatus=0x{:08X}", vm.cpu.csr.satp, vm.cpu.csr.mstatus);
            println!("  privilege={:?}", vm.cpu.privilege);

            // Read the instruction at mepc (the faulting instruction)
            let mepc = vm.cpu.csr.mepc;
            // In bare mode, mepc IS the physical address
            match vm.bus.read_word(mepc as u64) {
                Ok(inst) => println!("  Instruction at mepc: 0x{:08X}", inst),
                Err(e) => println!("  Cannot read instruction at mepc: {:?}", e),
            }

            // Read mtval for store fault address
            println!("  mtval=0x{:08X}", vm.cpu.csr.mtval);

            // Try to read/write the fault address
            let fault_addr = vm.cpu.csr.mtval as u64;
            match vm.bus.read_word(fault_addr) {
                Ok(val) => println!("  Read from fault addr 0x{:08X}: OK (0x{:08X})", fault_addr, val),
                Err(e) => println!("  Read from fault addr 0x{:08X}: FAIL ({:?})", fault_addr, e),
            }
            match vm.bus.write_word(fault_addr, 0xDEADBEEF) {
                Ok(()) => println!("  Write to fault addr 0x{:08X}: OK", fault_addr),
                Err(e) => println!("  Write to fault addr 0x{:08X}: FAIL ({:?})", fault_addr, e),
            }

            break;
        }

        match step_result {
            StepResult::FetchFault => {
                println!("[{}] FETCH FAULT", count);
                break;
            }
            StepResult::Ebreak => {
                println!("[{}] EBREAK", count);
                break;
            }
            _ => {}
        }
    }

    if count >= max_instr {
        println!("Reached max instructions without fault");
        println!("Final PC: 0x{:08X}", vm.cpu.pc);
    }
}
