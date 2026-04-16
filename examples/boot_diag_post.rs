use std::fs;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = fs::read(kernel_path).expect("kernel");
    let initramfs = fs::read(initramfs_path).ok();

    let bootargs = "console=ttyS0 earlycon=sbi panic=5 quiet";

    let result = geometry_os::riscv::RiscvVm::boot_linux(
        &kernel_image, initramfs.as_deref(), 512, 2_000_000, bootargs,
    );

    match result {
        Ok((mut vm, boot_result)) => {
            eprintln!("[diag] Instructions: {}", boot_result.instructions);
            eprintln!("[diag] Entry phys: 0x{:08X}", boot_result.entry);
            eprintln!("[diag] DTB addr: 0x{:08X}", boot_result.dtb_addr);
            eprintln!("[diag] PC: 0x{:08X}", vm.cpu.pc);
            eprintln!("[diag] Privilege: {:?}", vm.cpu.privilege);
            eprintln!("[diag] SATP: 0x{:08X}", vm.cpu.csr.satp);
            eprintln!("[diag] mepc: 0x{:08X}", vm.cpu.csr.mepc);
            eprintln!("[diag] sepc: 0x{:08X}", vm.cpu.csr.sepc);
            eprintln!("[diag] mstatus: 0x{:08X}", vm.cpu.csr.mstatus);
            eprintln!("[diag] mcause: 0x{:08X}", vm.cpu.csr.mcause);
            eprintln!("[diag] stvec: 0x{:08X}", vm.cpu.csr.stvec);
            eprintln!("[diag] mtvec: 0x{:08X}", vm.cpu.csr.mtvec);
            eprintln!("[diag] medeleg: 0x{:04X}", vm.cpu.csr.medeleg);
            eprintln!("[diag] ECALL count: {}", vm.cpu.ecall_count);
            eprintln!("[diag] UART tx_buf len: {}", vm.bus.uart.tx_buf.len());
            if !vm.bus.uart.tx_buf.is_empty() {
                let s = String::from_utf8_lossy(&vm.bus.uart.tx_buf);
                eprintln!("[UART] {}", s);
            }
            eprintln!("[diag] Syscall log entries: {}", vm.bus.syscall_log.len());
            for (i, evt) in vm.bus.syscall_log.iter().take(10).enumerate() {
                eprintln!("[syscall] #{}: {}({}) = {:?} at PC=0x{:08X}",
                    i, evt.name, evt.nr, evt.ret, evt.pc);
            }
            // Check what instruction is at current PC
            let inst = vm.bus.read_word(vm.cpu.pc as u64).unwrap_or(0);
            eprintln!("[diag] Instruction at PC: 0x{:08X}", inst);
        }
        Err(e) => {
            eprintln!("[diag] Boot error: {:?}", e);
        }
    }
}
