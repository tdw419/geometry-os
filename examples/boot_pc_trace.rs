use geometry_os::riscv::RiscvVm;
use std::fs;

fn main() {
    let kernel_data = fs::read(".geometry_os/build/linux-6.14/vmlinux").unwrap();
    let initramfs_data = fs::read(".geometry_os/fs/linux/rv32/initramfs.cpio.gz").ok();

    let bootargs = "console=ttyS0 earlycon=sbi panic=5 loglevel=7";

    let (mut vm, result) = RiscvVm::boot_linux(
        &kernel_data,
        initramfs_data.as_deref(),
        512,
        100_000_000,
        bootargs,
    )
    .expect("boot should succeed");

    eprintln!("PC: 0x{:08X}, Privilege: {:?}", vm.cpu.pc, vm.cpu.privilege);
    eprintln!("Instructions: {}", result.instructions);
    eprintln!("SBI ecall count: {}", vm.bus.sbi.ecall_log.len());

    // Last 10 SBI calls
    let log = &vm.bus.sbi.ecall_log;
    let start = log.len().saturating_sub(10);
    for i in start..log.len() {
        let (a7, a6, a0) = log[i];
        eprintln!("  ecall[{}]: a7=0x{:08X} a6={} a0=0x{:08X}", i, a7, a6, a0);
    }

    // Console output - last unique lines
    let sbi = String::from_utf8_lossy(&vm.bus.sbi.console_output);
    let lines: Vec<&str> = sbi.lines().collect();
    eprintln!("\nLast 5 unique console lines:");
    let mut seen = std::collections::HashSet::new();
    let mut unique_last = Vec::new();
    for line in lines.iter().rev() {
        if seen.insert(*line) {
            unique_last.push(*line);
            if unique_last.len() >= 5 {
                break;
            }
        }
    }
    for line in unique_last.iter().rev() {
        eprintln!("  {}", line);
    }

    // Disassemble at PC
    let pc = vm.cpu.pc as u64;
    match vm.bus.read_word(pc) {
        Ok(word) => {
            let hw = (word & 0xFFFF) as u16;
            let is_c = (hw & 0x3) != 0x3;
            eprintln!(
                "\nInstruction at PC 0x{:08X}: word=0x{:08X} compressed={}",
                pc, word, is_c
            );
            if is_c {
                eprintln!("  Quadrant={}, funct3={}", hw & 0x3, (hw >> 13) & 0x7);
            }
        }
        Err(e) => eprintln!("Could not read at PC: {}", e),
    }

    // CSRs
    eprintln!("\nCSRs:");
    eprintln!(
        "  mstatus={:08X} satp={:08X}",
        vm.cpu.csr.mstatus, vm.cpu.csr.satp
    );
    eprintln!(
        "  scause={:08X} sepc={:08X}",
        vm.cpu.csr.scause, vm.cpu.csr.sepc
    );
    eprintln!(
        "  stvec={:08X} sscratch={:08X}",
        vm.cpu.csr.stvec, vm.cpu.csr.sscratch
    );

    // Timer
    eprintln!(
        "\nTimer: mtime={}, mtimecmp={}",
        vm.bus.clint.mtime, vm.bus.clint.mtimecmp
    );
    eprintln!(
        "MIP={:08X} MIE={:08X} mideleg={:08X}",
        vm.cpu.csr.mip, vm.cpu.csr.mie, vm.cpu.csr.mideleg
    );
}
