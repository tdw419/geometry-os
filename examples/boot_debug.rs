use std::fs;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = fs::read(kernel_path).unwrap();
    let initramfs = fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1 quiet";
    let (mut vm, _result) = geometry_os::riscv::RiscvVm::boot_linux(
        &kernel_image,
        initramfs.as_deref(),
        256,
        0,
        bootargs,
    ).unwrap();

    // Write a minimal M-mode trap handler at 0x08000000:
    // csrr t0, mepc    # read faulting PC
    // addi t0, t0, 4   # skip faulting instruction
    // csrw mepc, t0    # update mepc
    // mret             # return from trap
    let handler_addr: u64 = 0x08000000;
    let handler: [u32; 4] = [
        0x034222F3, // csrr t0, mepc
        0x00428293, // addi t0, t0, 4
        0x03429073, // csrw mepc, t0
        0x30200073, // mret
    ];
    for (i, &inst) in handler.iter().enumerate() {
        vm.bus.write_word(handler_addr + i as u64 * 4, inst).unwrap();
    }

    // Set MTVEC to handler address (direct mode, not vector)
    vm.cpu.csr.write(geometry_os::riscv::csr::MTVEC, handler_addr as u32);

    // Verify
    println!("MTVEC set to: 0x{:08X}", vm.cpu.csr.read(geometry_os::riscv::csr::MTVEC));
    println!("Handler at 0x{:08X}: {:08X} {:08X} {:08X} {:08X}",
             handler_addr, handler[0], handler[1], handler[2], handler[3]);

    // Reset PC to kernel start
    vm.cpu.pc = 0x00000000;
    vm.cpu.x[10] = 0; // a0 = hartid
    vm.cpu.x[11] = _result.dtb_addr as u32; // a1 = DTB addr

    // Run with progress
    let max_instr = 100_000_000u64;
    let report_interval = 10_000_000u64;
    let mut count = 0u64;
    let mut total_uart = 0usize;
    let mut exception_count = 0u64;
    let start = std::time::Instant::now();

    let mut bridge = geometry_os::riscv::bridge::UartBridge::new();
    let mut canvas = vec![0u32; 128 * 80];

    while count < max_instr {
        let result = vm.step();
        count += 1;

        if count % 500 == 0 {
            let n = bridge.drain_uart_to_canvas(&mut vm.bus, &mut canvas);
            total_uart += n;
        }

        let mcause = vm.cpu.csr.read(geometry_os::riscv::csr::MCAUSE);
        if mcause != 0 {
            exception_count += 1;
            if exception_count <= 3 || exception_count % 1000 == 0 {
                let mepc = vm.cpu.csr.read(geometry_os::riscv::csr::MEPC);
                println!("  Exception #{} at step {}: cause={}, mepc=0x{:08X}",
                         exception_count, count, mcause as i32, mepc);
            }
        }

        if count % report_interval == 0 {
            let elapsed = start.elapsed();
            let ips = (count as f64) / elapsed.as_secs_f64();
            let priv_level = vm.cpu.privilege;
            let mcause = vm.cpu.csr.read(geometry_os::riscv::csr::MCAUSE);
            println!("[{:5.1}s] {}M instrs, PC=0x{:08X}, priv={:?}, UART={}, excepts={}, mcause={}",
                     elapsed.as_secs_f64(), count / 1_000_000, vm.cpu.pc, priv_level,
                     total_uart, exception_count, mcause as i32);
        }

        match result {
            geometry_os::riscv::cpu::StepResult::Ebreak => {
                println!("EBREAK at PC=0x{:08X}, {} instructions", vm.cpu.pc, count);
                break;
            }
            geometry_os::riscv::cpu::StepResult::FetchFault => {
                println!("FetchFault at PC=0x{:08X}, {} instructions", vm.cpu.pc, count);
                break;
            }
            _ => {}
        }
    }

    let elapsed = start.elapsed();
    println!("\nDone: {}M instructions in {:?}", count / 1_000_000, elapsed);
    println!("Final PC: 0x{:08X}", vm.cpu.pc);
    println!("Privilege: {:?}", vm.cpu.privilege);
    println!("Total UART bytes: {}", total_uart);
    println!("Total exceptions: {}", exception_count);
    println!("Instructions/sec: {:.0}", count as f64 / elapsed.as_secs_f64());

    // Print UART output
    let mut found = false;
    for row in 0..128 {
        let s = geometry_os::riscv::bridge::UartBridge::read_canvas_string(&canvas, row, 0, 80);
        if !s.is_empty() {
            if !found { println!("\n=== UART Output ==="); found = true; }
            println!("  {:3}: {}", row, s);
        }
    }
    if !found { println!("\n(no UART output)"); }
}
