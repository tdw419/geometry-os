use geometry_os::riscv::cpu::Privilege;
use geometry_os::riscv::RiscvVm;
use std::fs;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let kernel_image = fs::read(kernel_path).expect("kernel");
    let bootargs = "console=ttyS0 earlycon=sbi panic=5 nosmp maxcpus=1 loglevel=8";
    let (mut vm, fw_addr, _entry, _dtb_addr) =
        RiscvVm::boot_linux_setup(&kernel_image, None, 32, bootargs).unwrap();
    let fw_addr_u32 = fw_addr as u32;
    let max_count: u64 = 50_000_000;
    let mut count: u64 = 0;
    let mut timer_fwd_count: u64 = 0;
    let mut next_report: u64 = 500_000;
    let mut last_uart = String::new();
    while count < max_count {
        if vm.bus.sbi.shutdown_requested {
            break;
        }
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            let is_interrupt = (mcause >> 31) & 1 == 1;
            if is_interrupt && cause_code == 7 {
                timer_fwd_count += 1;
                vm.cpu.csr.mip |= 1 << 5;
                vm.bus.clint.mtimecmp = u64::MAX;
            } else if !is_interrupt {
                vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
            }
        }
        vm.step();
        count += 1;
        if count == next_report {
            let uart_bytes = vm.bus.uart.drain_tx();
            if !uart_bytes.is_empty() {
                let s = String::from_utf8_lossy(&uart_bytes);
                last_uart.push_str(&s);
            }
            eprintln!(
                "[{}K] PC=0x{:08X} ECALLS={} TMRFWD={} mtimecmp=0x{:016X}",
                count / 1000,
                vm.cpu.pc,
                vm.cpu.ecall_count,
                timer_fwd_count,
                vm.bus.clint.mtimecmp
            );
            next_report += 500_000;
        }
        if (0xC000252E..=0xC00025A0).contains(&vm.cpu.pc) && count > 500_000 {
            eprintln!(
                "PANIC at {}K ECALLS={} TMRFWD={}",
                count / 1000,
                vm.cpu.ecall_count,
                timer_fwd_count
            );
            break;
        }
    }
    let tx = vm.bus.uart.drain_tx();
    if !tx.is_empty() {
        last_uart.push_str(&String::from_utf8_lossy(&tx));
    }
    eprintln!(
        "Total: {}K ECALLS={} TMRFWD={}",
        count / 1000,
        vm.cpu.ecall_count,
        timer_fwd_count
    );
    if !last_uart.is_empty() {
        eprintln!("=== UART OUTPUT ===");
        eprintln!("{}", &last_uart[..last_uart.len().min(5000)]);
    }
}
