//! Watch for reads of _dtb_early_va and initial_boot_params.
use geometry_os::riscv::RiscvVm;
fn main() {
    let kernel_data = std::fs::read(".geometry_os/build/linux-6.14/vmlinux").expect("kernel");
    let (mut vm, _fw, _entry, dtb_addr) =
        RiscvVm::boot_linux_setup(&kernel_data, None, 512, "console=ttyS0 earlycon=sbi nosmp")
            .expect("setup");

    // Run step by step, checking for SBI ECALLs and output
    let max = 10_000_000u64;
    for count in 0..max {
        if vm.bus.sbi.shutdown_requested {
            break;
        }
        vm.bus.tick_clint_n(100);
        vm.bus.sync_mip(&mut vm.cpu.csr.mip);
        let _ = vm.step();

        // Print when SBI console output appears
        let sbi_len = vm.bus.sbi.console_output.len();
        if sbi_len > 0 && count % 10000 == 0 {
            let s: String = vm
                .bus
                .sbi
                .console_output
                .iter()
                .map(|&b| b as char)
                .collect();
            eprintln!(
                "[{}] SBI output ({} bytes): {}",
                count,
                sbi_len,
                &s[..s.len().min(200)]
            );
        }

        // Print ECALL details
        let ecall = vm.cpu.ecall_count;
        if ecall > 0 && ecall <= 30 && count % 10000 == 0 {
            // Log what ECALLs we've seen
        }

        if count == 178000 {
            let ibp = vm.bus.read_word(0x00C7A380).unwrap_or(0);
            let deva = vm.bus.read_word(0x00801008).unwrap_or(0);
            let depa = vm.bus.read_word(0x0080100C).unwrap_or(0);
            let dtb_magic = vm.bus.read_word(dtb_addr).unwrap_or(0);
            eprintln!(
                "[{}] State: ibp=0x{:08X} deva=0x{:08X} depa=0x{:08X} dtb_magic=0x{:08X} ecall={}",
                count, ibp, deva, depa, dtb_magic, vm.cpu.ecall_count
            );
            eprintln!(
                "[{}] PC=0x{:08X} priv={:?}",
                count, vm.cpu.pc, vm.cpu.privilege
            );
        }
    }

    let sbi_out: String = vm
        .bus
        .sbi
        .console_output
        .iter()
        .map(|&b| b as char)
        .collect();
    eprintln!(
        "SBI output ({} bytes): {}",
        vm.bus.sbi.console_output.len(),
        if sbi_out.is_empty() {
            "(none)".to_string()
        } else {
            sbi_out[..sbi_out.len().min(500)].to_string()
        }
    );
    eprintln!("ECALLs: {}, PC: 0x{:08X}", vm.cpu.ecall_count, vm.cpu.pc);
}
