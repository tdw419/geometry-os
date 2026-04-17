// Diagnostic: narrow down exactly when pt_ops[4] gets corrupted.
// Run: cargo run --example boot_pt_ops_watch2

use std::fs;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";

    let kernel_image = fs::read(kernel_path).expect("kernel");
    let initramfs = fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1 quiet";

    let (mut vm, fw_addr, _entry, _dtb_addr) =
        geometry_os::riscv::RiscvVm::boot_linux_setup(
            &kernel_image,
            initramfs.as_deref(),
            256,
            bootargs,
        ).expect("boot setup failed");

    let fw_addr_u32 = fw_addr as u32;
    let max_instr: u64 = 178_000;
    let mut count: u64 = 0;
    let mut last_satp: u32 = vm.cpu.csr.satp;

    let pt_ops_4_pa: u64 = 0x00801004;
    let mut last_val: u32 = vm.bus.read_word(pt_ops_4_pa).unwrap_or(0);
    let mut found_correct = false;

    use geometry_os::riscv::cpu::StepResult;

    while count < max_instr {
        // SATP change handling
        let cur_satp = vm.cpu.csr.satp;
        if cur_satp != last_satp {
            let mode = (cur_satp >> 31) & 1;
            if mode == 1 {
                let ppn = cur_satp & 0x3FFFFF;
                let pg_dir_phys = (ppn as u64) * 4096;
                let device_l1: &[u32] = &[0, 1, 2, 3, 4, 5, 8, 48, 64];
                for &l1_idx in device_l1 {
                    let addr = pg_dir_phys + (l1_idx as u64) * 4;
                    let existing = vm.bus.read_word(addr).unwrap_or(0);
                    if (existing & 1) == 0 {
                        vm.bus.write_word(addr, 0xCF | (l1_idx << 20)).ok();
                    }
                }
                vm.cpu.tlb.flush_all();
            }
            last_satp = cur_satp;
        }

        // Trap forwarding
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            let mpp = (vm.cpu.csr.mstatus & 0x300) >> 4;
            if cause_code != 11 && mpp != 3 {
                let stvec = vm.cpu.csr.stvec & !0x3u32;
                if stvec != 0 {
                    vm.cpu.csr.sepc = vm.cpu.csr.mepc;
                    vm.cpu.csr.scause = mcause;
                    vm.cpu.csr.stval = vm.cpu.csr.mtval;
                    let spp = if mpp == 1 { 1u32 } else { 0u32 };
                    vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1u32 << 5)) | (spp << 5);
                    let sie = (vm.cpu.csr.mstatus >> 1) & 1;
                    vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1u32 << 5)) | (sie << 5);
                    vm.cpu.csr.mstatus &= !(1u32 << 1);
                    if cause_code == 7 {
                        vm.bus.clint.mtimecmp = vm.bus.clint.mtime + 100_000;
                    }
                    vm.cpu.pc = stvec;
                    vm.cpu.privilege = geometry_os::riscv::cpu::Privilege::Supervisor;
                    vm.cpu.tlb.flush_all();
                    count += 1;
                    continue;
                }
            }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }

        vm.bus.tick_clint();
        vm.bus.sync_mip(&mut vm.cpu.csr.mip);

        // Before stepping, save PC
        let prev_pc = vm.cpu.pc;

        vm.step();

        // Check every instruction once we've seen the correct value
        if found_correct || count > 176800 {
            let cur_val = vm.bus.read_word(pt_ops_4_pa).unwrap_or(0);
            if cur_val == 0xC04046C0 && !found_correct {
                found_correct = true;
                eprintln!("[watch] count={} pt_ops[4] set to 0x{:08X} (correct) PC=0x{:08X}", count, cur_val, prev_pc);
            }
            if cur_val != last_val {
                eprintln!(
                    "[watch] count={} pt_ops[4] CHANGED: 0x{:08X} -> 0x{:08X} prev_PC=0x{:08X}",
                    count, last_val, cur_val, prev_pc
                );
                // Dump registers
                eprintln!(
                    "[watch] s2=0x{:08X} a0=0x{:08X} a1=0x{:08X} a5=0x{:08X} ra=0x{:08X}",
                    vm.cpu.x[18], vm.cpu.x[10], vm.cpu.x[11], vm.cpu.x[15], vm.cpu.x[1]
                );
                // Read instruction at prev_pc
                let instr = vm.bus.read_word(prev_pc as u64).unwrap_or(0);
                eprintln!("[watch] instruction at prev_PC: 0x{:08X}", instr);
                let half = vm.bus.read_half(prev_pc as u64).unwrap_or(0);
                eprintln!("[watch] halfword at prev_PC: 0x{:04X}", half);
                last_val = cur_val;
            }
        }

        count += 1;
    }
}
