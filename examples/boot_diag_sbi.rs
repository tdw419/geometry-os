// Diagnostic: Check SBI calls and UART output during Linux boot
use geometry_os::riscv::cpu::Privilege;
use geometry_os::riscv::RiscvVm;
use std::fs;
use std::time::Instant;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = fs::read(kernel_path).expect("kernel");
    let initramfs = fs::read(initramfs_path).ok();

    let bootargs = "console=ttyS0 earlycon=sbi panic=5 quiet";
    let (mut vm, fw_addr, _entry, _dtb_addr) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 128, bootargs).unwrap();

    let fw_addr_u32 = fw_addr as u32;
    let max_count: u64 = 2_000_000;
    let mut count: u64 = 0;
    let mut start = Instant::now();

    while count < max_count {
        if vm.bus.sbi.shutdown_requested {
            break;
        }

        // Handle traps at fw_addr
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            if cause_code == 9 {
                // ECALL_S that wasn't handled inline by step()
                eprintln!(
                    "[diag] ECALL_S at fw_addr count={}: a7=0x{:08X} a6=0x{:08X} a0=0x{:08X}",
                    count, vm.cpu.x[17], vm.cpu.x[16], vm.cpu.x[10]
                );
            }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }

        let _ = vm.step();

        // Handle DBCN pending writes
        if let Some((phys_addr, num_bytes)) = vm.bus.sbi.dbcn_pending_write.take() {
            for i in 0..num_bytes {
                if let Ok(b) = vm.bus.read_byte(phys_addr + i as u64) {
                    if b != 0 {
                        vm.bus.uart.write_byte(0, b);
                        vm.bus.sbi.console_output.push(b);
                    }
                }
            }
            vm.cpu.x[10] = 0; // SBI_SUCCESS
            vm.cpu.x[11] = num_bytes as u32;
        }

        count += 1;
    }

    let elapsed = start.elapsed();
    eprintln!(
        "\n=== Diagnostic Results ({} instructions, {:.1}s) ===",
        count,
        elapsed.as_secs_f64()
    );
    eprintln!("SBI ecall_log: {} entries", vm.bus.sbi.ecall_log.len());

    // Show first 20 SBI calls
    for (i, &(a7, a6, a0)) in vm.bus.sbi.ecall_log.iter().take(20).enumerate() {
        eprintln!(
            "  SBI #{}: a7=0x{:08X} a6=0x{:08X} a0=0x{:08X}",
            i, a7, a6, a0
        );
    }

    // Show UART output
    let tx = vm.bus.uart.drain_tx();
    eprintln!("\nUART: {} bytes", tx.len());
    if !tx.is_empty() {
        let s = String::from_utf8_lossy(&tx);
        eprintln!("{}", &s[..s.len().min(3000)]);
    }

    // Show final CPU state
    eprintln!(
        "\nCPU: PC=0x{:08X} SP=0x{:08X} priv={:?}",
        vm.cpu.pc, vm.cpu.x[2], vm.cpu.privilege
    );
    eprintln!(
        "SATP=0x{:08X} medeleg=0x{:04X}",
        vm.cpu.csr.satp, vm.cpu.csr.medeleg
    );
    eprintln!(
        "mcause=0x{:08X} sepc=0x{:08X} scause=0x{:08X}",
        vm.cpu.csr.mcause, vm.cpu.csr.sepc, vm.cpu.csr.scause
    );
}
