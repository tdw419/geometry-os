//! Check CSR state after boot to understand why no M-mode traps occur.
//! Run: cargo run --example boot_csr_check

use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let ir_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_data = std::fs::read(kernel_path).expect("kernel");
    let initramfs_data = std::path::Path::new(ir_path)
        .exists()
        .then(|| std::fs::read(ir_path).unwrap());

    eprintln!("[csr_check] Starting boot with 5M instruction limit...");
    let result = RiscvVm::boot_linux(
        &kernel_data,
        initramfs_data.as_deref(),
        512,
        5_000_000,
        "console=ttyS0 earlycon=sbi loglevel=7",
    );

    match result {
        Ok((mut vm, _stats)) => {
            eprintln!("\n[csr_check] Post-boot CSR state:");
            eprintln!("  mtvec = 0x{:08X}", vm.cpu.csr.mtvec);
            eprintln!("  stvec = 0x{:08X}", vm.cpu.csr.stvec);
            eprintln!(
                "  medeleg = 0x{:08X} (binary: {:032b})",
                vm.cpu.csr.medeleg, vm.cpu.csr.medeleg
            );
            eprintln!("  mideleg = 0x{:08X}", vm.cpu.csr.mideleg);
            eprintln!("  satp = 0x{:08X}", vm.cpu.csr.satp);
            eprintln!("  mstatus = 0x{:08X}", vm.cpu.csr.mstatus);
            eprintln!("  privilege = {:?}", vm.cpu.privilege);
            eprintln!("  PC = 0x{:08X}", vm.cpu.pc);

            // Check specific medeleg bits
            let ecall_s_bit = (vm.cpu.csr.medeleg >> 9) & 1;
            let ecall_u_bit = (vm.cpu.csr.medeleg >> 8) & 1;
            let inst_pf_bit = (vm.cpu.csr.medeleg >> 12) & 1;
            let load_pf_bit = (vm.cpu.csr.medeleg >> 13) & 1;
            let store_pf_bit = (vm.cpu.csr.medeleg >> 15) & 1;
            let break_bit = (vm.cpu.csr.medeleg >> 3) & 1;
            let illegal_bit = (vm.cpu.csr.medeleg >> 2) & 1;
            eprintln!("\n[csr_check] medeleg bits:");
            eprintln!(
                "  ECALL_U (bit 8) = {} (should be 1 for syscall delegation)",
                ecall_u_bit
            );
            eprintln!(
                "  ECALL_S (bit 9) = {} (should be 0 for SBI calls)",
                ecall_s_bit
            );
            eprintln!("  Inst PF (bit 12) = {}", inst_pf_bit);
            eprintln!("  Load PF (bit 13) = {}", load_pf_bit);
            eprintln!("  Store PF (bit 15) = {}", store_pf_bit);
            eprintln!("  Breakpoint (bit 3) = {}", break_bit);
            eprintln!("  Illegal inst (bit 2) = {}", illegal_bit);

            // Check SBI console output
            let sbi_str: String = vm
                .bus
                .sbi
                .console_output
                .iter()
                .map(|&b| b as char)
                .collect();
            eprintln!("\n[csr_check] SBI console: {} chars", sbi_str.len());
            if !sbi_str.is_empty() {
                eprintln!("{}", &sbi_str[..sbi_str.len().min(2000)]);
            }

            // Check if mtvec still points to our firmware
            // fw_addr is computed in boot_linux_setup, we can't access it directly
            // but we can check if mtvec is reasonable
            let fw_area = vm.cpu.csr.mtvec;
            let fw_inst = vm.bus.read_word(fw_area as u64).unwrap_or(0);
            eprintln!(
                "\n[csr_check] Instruction at mtvec: 0x{:08X} (expect 0x30200073 = MRET)",
                fw_inst
            );

            // Check kernel_map
            let km_phys: u64 = 0x00C79E90;
            let km_pa = vm.bus.read_word(km_phys + 12).unwrap_or(0);
            let km_vapo = vm.bus.read_word(km_phys + 20).unwrap_or(0);
            eprintln!(
                "\n[csr_check] kernel_map: phys_addr=0x{:08X} va_pa_offset=0x{:08X}",
                km_pa, km_vapo
            );

            // Check phys_ram_base
            let prb = vm.bus.read_word(0x00C79EACu64).unwrap_or(0);
            eprintln!("[csr_check] phys_ram_base = 0x{:08X}", prb);
        }
        Err(e) => {
            eprintln!("[csr_check] Boot error: {:?}", e);
        }
    }
}
