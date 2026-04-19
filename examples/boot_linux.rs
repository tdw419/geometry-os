use geometry_os::riscv::cpu::Privilege;
use geometry_os::riscv::RiscvVm;
use std::fs;
use std::time::Instant;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = fs::read(kernel_path).expect("kernel");
    let initramfs = fs::read(initramfs_path).ok();

    // Use 128MB to reduce page table setup time
    let bootargs = "console=ttyS0 earlycon=sbi panic=5 quiet";
    let (mut vm, fw_addr, _entry, _dtb_addr) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 128, bootargs).unwrap();

    let fw_addr_u32 = fw_addr as u32;
    let max_count: u64 = 200_000_000; // 200M instructions
    let mut count: u64 = 0;
    let mut sbi_count: u64 = 0;
    let mut ecall_m_count: u64 = 0;
    let mut smode_trap_count: u64 = 0;
    let mut last_satp: u32 = vm.cpu.csr.satp;
    let mut satp_changes: u32 = 0;
    let mut last_medeleg: u32 = vm.cpu.csr.medeleg;
    let mut start = Instant::now();
    let mut next_report: u64 = 10_000_000;

    while count < max_count {
        if vm.bus.sbi.shutdown_requested {
            break;
        }

        // Handle M-mode traps at fw_addr
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            let is_interrupt = (mcause >> 31) & 1 == 1;
            if !is_interrupt {
                match cause_code {
                    9 => {
                        // ECALL_S = SBI call
                        sbi_count += 1;
                        let result = vm.bus.sbi.handle_ecall(
                            vm.cpu.x[17],
                            vm.cpu.x[16],
                            vm.cpu.x[10],
                            vm.cpu.x[11],
                            vm.cpu.x[12],
                            vm.cpu.x[13],
                            vm.cpu.x[14],
                            vm.cpu.x[15],
                            &mut vm.bus.uart,
                            &mut vm.bus.clint,
                        );
                        if let Some((a0, a1)) = result {
                            vm.cpu.x[10] = a0;
                            vm.cpu.x[11] = a1;
                        }
                    }
                    11 => {
                        // ECALL_M
                        ecall_m_count += 1;
                    }
                    _ => {}
                }
            }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }

        // Use vm.step() which handles tick_clint + sync_mip internally
        let _step_result = vm.step();

        // Track SATP changes
        if vm.cpu.csr.satp != last_satp {
            satp_changes += 1;
            eprintln!(
                "[satp] #{} at count={}: 0x{:08X} -> 0x{:08X} PC=0x{:08X} medeleg=0x{:04X}",
                satp_changes, count, last_satp, vm.cpu.csr.satp, vm.cpu.pc, vm.cpu.csr.medeleg
            );
            last_satp = vm.cpu.csr.satp;
        }

        // Track medeleg changes
        if vm.cpu.csr.medeleg != last_medeleg && count > 1000 {
            eprintln!(
                "[medeleg] Changed to 0x{:04X} at count={} PC=0x{:08X}",
                vm.cpu.csr.medeleg, count, vm.cpu.pc
            );
            last_medeleg = vm.cpu.csr.medeleg;
        }

        // Detect kernel panic (PC in panic function)
        if (0xC000252E..=0xC00027A0).contains(&vm.cpu.pc) && count > 1_000_000 {
            if sbi_count == 0 {
                // First time hitting panic - dump registers
                eprintln!(
                    "\n!!! KERNEL PANIC detected at count={} PC=0x{:08X} !!!",
                    count, vm.cpu.pc
                );
                eprintln!(
                    "    SP=0x{:08X} RA=0x{:08X} GP=0x{:08X} TP=0x{:08X}",
                    vm.cpu.x[2], vm.cpu.x[1], vm.cpu.x[3], vm.cpu.x[4]
                );
                eprintln!(
                    "    T0=0x{:08X} T1=0x{:08X} T2=0x{:08X} A0=0x{:08X}",
                    vm.cpu.x[5], vm.cpu.x[6], vm.cpu.x[7], vm.cpu.x[10]
                );
                eprintln!(
                    "    A1=0x{:08X} A2=0x{:08X} S0=0x{:08X} S1=0x{:08X}",
                    vm.cpu.x[11], vm.cpu.x[12], vm.cpu.x[8], vm.cpu.x[9]
                );
                eprintln!(
                    "    mcause=0x{:08X} sepc=0x{:08X} scause=0x{:08X}",
                    vm.cpu.csr.mcause, vm.cpu.csr.sepc, vm.cpu.csr.scause
                );
                // Check stack for panic message pointer (s3 register in panic)
                eprintln!(
                    "    S2=0x{:08X} S3=0x{:08X} S4=0x{:08X} S5=0x{:08X}",
                    vm.cpu.x[18], vm.cpu.x[19], vm.cpu.x[20], vm.cpu.x[21]
                );
                // Try to read panic message from the stack or registers
                // In panic(), a0 = the panic string pointer
                let panic_str_ptr = vm.cpu.x[10]; // a0 usually has the format string
                if panic_str_ptr > 0xC0000000 && panic_str_ptr < 0xC2000000 {
                    let pa = (panic_str_ptr - 0xC0000000) as u64;
                    let mut msg_bytes = Vec::new();
                    for i in 0..128u64 {
                        if let Ok(byte_val) = vm.bus.read_byte(pa + i) {
                            if byte_val == 0 {
                                break;
                            }
                            msg_bytes.push(byte_val);
                        } else {
                            break;
                        }
                    }
                    if let Ok(msg) = String::from_utf8(msg_bytes.clone()) {
                        eprintln!("    A0 string: '{}'", &msg[..msg.len().min(200)]);
                    }
                }
                // Check UART for any output before panic
                let tx = vm.bus.uart.drain_tx();
                if !tx.is_empty() {
                    let s = String::from_utf8_lossy(&tx);
                    eprintln!("    UART before panic: {}", &s[..s.len().min(2000)]);
                }
                break; // Stop execution on panic
            }
        }

        count += 1;

        if count == next_report {
            let elapsed = start.elapsed();
            let ips = count as f64 / elapsed.as_secs_f64();
            let priv_str = match vm.cpu.privilege {
                Privilege::Machine => "M",
                Privilege::Supervisor => "S",
                Privilege::User => "U",
            };
            eprintln!("[{}M] PC=0x{:08X} SP=0x{:08X} RA=0x{:08X} SBI={} SATP=0x{:08X} medeleg=0x{:04X} priv={} uart={}",
                count / 1_000_000, vm.cpu.pc, vm.cpu.x[2], vm.cpu.x[1],
                sbi_count, vm.cpu.csr.satp, vm.cpu.csr.medeleg, priv_str,
                vm.bus.uart.tx_buf.len());
            next_report += 10_000_000;
        }
    }

    let elapsed = start.elapsed();
    eprintln!(
        "\n=== Final State ({}M instructions, {:.1}s) ===",
        count / 1_000_000,
        elapsed.as_secs_f64()
    );
    eprintln!(
        "PC: 0x{:08X} SP: 0x{:08X} RA: 0x{:08X}",
        vm.cpu.pc, vm.cpu.x[2], vm.cpu.x[1]
    );
    eprintln!("SATP: 0x{:08X}", vm.cpu.csr.satp);
    eprintln!("medeleg: 0x{:04X}", vm.cpu.csr.medeleg);
    eprintln!("stvec: 0x{:08X}", vm.cpu.csr.stvec);
    eprintln!("SBI calls: {}", sbi_count);
    eprintln!("ECALL_M: {}", ecall_m_count);
    eprintln!("SATP changes: {}", satp_changes);
    eprintln!("CLINT mtime: {}", vm.bus.clint.mtime);
    eprintln!("MIP: 0x{:08X}", vm.cpu.csr.mip);

    let tx = vm.bus.uart.drain_tx();
    eprintln!("\nUART: {} bytes", tx.len());
    if !tx.is_empty() {
        let s = String::from_utf8_lossy(&tx);
        eprintln!("{}", &s[..s.len().min(5000)]);
    }
}
