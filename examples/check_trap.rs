// Find what exception triggered die() - trace S-mode traps
use geometry_os::riscv::RiscvVm;
use geometry_os::riscv::cpu::Privilege;

fn main() {
    let kernel_path = std::path::Path::new(".geometry_os/build/linux-6.14/vmlinux");
    let kernel = std::fs::read(kernel_path).expect("kernel");
    let initramfs_path = std::path::Path::new(".geometry_os/fs/linux/rv32/initramfs.cpio.gz");
    let initramfs = std::fs::read(initramfs_path).ok();

    let (mut vm, _fw_addr, _entry, _dtb_addr) =
        RiscvVm::boot_linux_setup(&kernel, initramfs.as_deref(), 128, "console=ttyS0 earlycon=sbi loglevel=8").unwrap();

    let mut count: u64 = 0;
    let max: u64 = 400_000;
    let mut prev_scause: u32 = 0;
    let mut trap_count = 0u32;
    let mut die_entered = false;

    while count < max {
        let pc = vm.cpu.pc;
        let scause = vm.cpu.csr.scause;
        let stval = vm.cpu.csr.stval;
        let privilege = vm.cpu.privilege;

        // Detect S-mode trap delivery: scause changes
        if scause != prev_scause && scause != 0 && privilege == Privilege::Supervisor {
            trap_count += 1;
            let cause_name = match scause {
                2 => "IllegalInstruction",
                3 => "Breakpoint",
                8 => "EcallU",
                9 => "EcallS",
                12 => "InstrPageFault",
                13 => "LoadPageFault",
                15 => "StorePageFault",
                _ => "Unknown",
            };
            if trap_count <= 50 || count > 200_000 {
                println!(
                    "[trap] #{} at count={}: scause={} ({}) stval=0x{:08X} PC=0x{:08X}",
                    trap_count, count, scause, cause_name, stval, pc
                );
            }
            prev_scause = scause;
        }

        // Check for die() entry
        if !die_entered && pc >= 0xC000A540 && pc < 0xC000A620 {
            die_entered = true;
            println!(
                "\n[die] entered at count={}, PC=0x{:08X} scause=0x{:X} stval=0x{:08X} a0=0x{:08X} a1=0x{:08X} ra=0x{:08X}",
                count, pc, vm.cpu.csr.scause, vm.cpu.csr.stval,
                vm.cpu.x[10], vm.cpu.x[11], vm.cpu.x[1]
            );
        }

        let _result = vm.step();
        count += 1;
    }

    println!(
        "[done] {} instr, {} traps, die={}",
        count, trap_count, die_entered
    );
}
