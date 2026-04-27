use geometry_os::riscv::RiscvVm;
use std::fs;
use std::time::Instant;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";

    let kernel_data = match fs::read(kernel_path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Kernel not found: {}", e);
            return;
        }
    };
    let initramfs_data = fs::read(initramfs_path).ok();

    let bootargs = "console=ttyS0 earlycon=sbi panic=5 loglevel=7";
    let start = Instant::now();

    // Run 50M instructions to check if timer fix allows progress past WFI
    let (mut vm, result) = RiscvVm::boot_linux(
        &kernel_data,
        initramfs_data.as_deref(),
        512,
        50_000_000,
        bootargs,
    )
    .expect("boot should succeed");

    let elapsed = start.elapsed();
    let mips = result.instructions as f64 / elapsed.as_secs_f64() / 1_000_000.0;
    eprintln!(
        "{} instructions in {:?} = {:.2} MIPS",
        result.instructions, elapsed, mips
    );
    eprintln!("PC: 0x{:08X}, Privilege: {:?}", vm.cpu.pc, vm.cpu.privilege);

    // Check SBI console output
    let sbi_out = String::from_utf8_lossy(&vm.bus.sbi.console_output);
    eprintln!(
        "SBI console output ({} bytes):",
        vm.bus.sbi.console_output.len()
    );
    // Print last 2000 chars of SBI output
    if sbi_out.len() > 2000 {
        eprintln!("...{}...", &sbi_out[sbi_out.len() - 2000..]);
    } else {
        eprintln!("{}", sbi_out);
    }

    // Check UART TX output
    let uart_out = String::from_utf8_lossy(&vm.bus.uart.tx_buf);
    eprintln!(
        "UART TX output ({} bytes): {}",
        vm.bus.uart.tx_buf.len(),
        &uart_out[..uart_out.len().min(1000)]
    );

    // SBI ecall summary
    eprintln!("SBI ecall count: {}", vm.bus.sbi.ecall_log.len());
    eprintln!("CPU ecall count: {}", vm.cpu.ecall_count);

    // Check timer state
    let mip = vm.cpu.csr.mip;
    let mie = vm.cpu.csr.mie;
    let mstatus = vm.cpu.csr.mstatus;
    let sie = (mstatus >> 1) & 1;
    let mtie = (mie >> 7) & 1;
    let stie = (mie >> 5) & 1;
    let mtip = (mip >> 7) & 1;
    let stip = (mip >> 5) & 1;
    let mideleg = vm.cpu.csr.mideleg;
    eprintln!("MIP: 0x{:08X} (MTIP={} STIP={})", mip, mtip, stip);
    eprintln!("MIE: 0x{:08X} (MTIE={} STIE={})", mie, mtie, stie);
    eprintln!("SIE bit: {}", sie);
    eprintln!(
        "mideleg: 0x{:08X} (STI delegated={})",
        mideleg,
        (mideleg >> 5) & 1
    );

    // Check mtime and mtimecmp
    let mtime = vm.bus.clint.mtime;
    let mtimecmp = vm.bus.clint.mtimecmp;
    eprintln!("mtime: {}, mtimecmp: {}", mtime, mtimecmp);

    // Check CSRs
    eprintln!("satp: 0x{:08X}", vm.cpu.csr.satp);
    eprintln!("sstatus: 0x{:08X} (via mstatus)", mstatus);
    eprintln!("mstatus: 0x{:08X}", vm.cpu.csr.mstatus);

    // Last few SBI calls
    let log = &vm.bus.sbi.ecall_log;
    let start = log.len().saturating_sub(20);
    for i in start..log.len() {
        let (a7, a6, a0) = log[i];
        eprintln!("  ecall[{}]: a7=0x{:08X} a6={} a0=0x{:08X}", i, a7, a6, a0);
    }
}
