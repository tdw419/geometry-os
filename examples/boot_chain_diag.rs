use geometry_os::riscv::cpu::{Privilege, StepResult};
use geometry_os::riscv::RiscvVm;
use std::collections::HashMap;
use std::fs;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = fs::read(kernel_path).expect("kernel");
    let initramfs = fs::read(initramfs_path).ok();

    let bootargs = "console=ttyS0 earlycon=sbi panic=5 quiet";

    let (mut vm, fw_addr, _entry, _dtb_addr) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 512, bootargs)
            .expect("setup");

    let fw_addr_u32 = fw_addr as u32;

    let mut trampoline_patched = false;
    let mut last_satp: u32 = 0;
    let mut sbi_count: u64 = 0;
    let mut pc_visits: HashMap<u32, u64> = HashMap::new();

    // Run 5M instructions, sampling PC every 1000 steps
    for count in 0..5_000_000u64 {
        if vm.bus.sbi.shutdown_requested {
            break;
        }

        let pc = vm.cpu.pc;
        let satp = vm.cpu.csr.satp;

        // Phase 1
        if !trampoline_patched
            && pc == 0x10EE
            && vm.cpu.privilege == Privilege::Supervisor
            && satp == 0
        {
            let identity_pte: u32 = 0xCF;
            let l1_entries: &[u32] = &[0, 2, 4, 5, 6, 8, 10];
            for &idx in l1_entries {
                let pte = identity_pte | (idx << 20);
                vm.bus
                    .write_word(0x01484000u64 + (idx * 4) as u64, pte)
                    .ok();
                vm.bus
                    .write_word(0x00802000u64 + (idx * 4) as u64, pte)
                    .ok();
            }
            trampoline_patched = true;
        }

        // Phase 2
        if trampoline_patched && satp != last_satp && satp != 0 {
            let mode = (satp >> 31) & 1;
            if mode == 1 {
                let ppn = satp & 0x3FFFFF;
                let pg_dir = (ppn as u64) * 4096;
                let l1_0 = vm.bus.read_word(pg_dir).unwrap_or(0);
                let patched = (l1_0 & 0xCF) == 0xCF && ((l1_0 >> 20) & 0xFFF) == 0;
                if !patched {
                    let identity_pte: u32 = 0xCF;
                    let l1_entries: &[u32] = &[0, 2, 4, 5, 6, 8, 10];
                    for &idx in l1_entries {
                        let pte = identity_pte | (idx << 20);
                        vm.bus.write_word(pg_dir + (idx * 4) as u64, pte).ok();
                    }
                    vm.cpu.tlb.flush_all();
                    eprintln!(
                        "[diag] Phase 2 injected into pg_dir=0x{:08X} at count={}",
                        pg_dir, count
                    );
                }
            }
        }
        last_satp = satp;

        // M-mode trap handler
        if pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            if cause_code == 9 {
                // ECALL_S
                sbi_count += 1;
                let a7 = vm.cpu.x[17];
                let a0 = vm.cpu.x[10];
                if sbi_count <= 20 {
                    eprintln!(
                        "[diag] SBI #{}: a7={} a0=0x{:02X} at count={}",
                        sbi_count, a7, a0, count
                    );
                }
                if a7 == 1 {
                    let ch = a0 as u8;
                    eprint!(
                        "{}",
                        if ch >= 0x20 && ch < 0x7f {
                            ch as char
                        } else {
                            '.'
                        }
                    );
                    use std::io::Write;
                    std::io::stderr().flush().ok();
                }
                vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
            } else {
                let mpp = (vm.cpu.csr.mstatus >> 11) & 3;
                if mpp != 3 && mpp > 0 {
                    let stvec = vm.cpu.csr.stvec & !0x3u32;
                    if stvec != 0 {
                        vm.cpu.csr.sepc = vm.cpu.csr.mepc;
                        vm.cpu.csr.scause = mcause;
                        vm.cpu.csr.stval = vm.cpu.csr.mtval;
                        vm.cpu.pc = stvec;
                        vm.cpu.privilege = Privilege::Supervisor;
                        vm.cpu.tlb.flush_all();
                        continue;
                    }
                }
                vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
            }
        }

        // Sample PC every 1000 steps
        if count % 1000 == 0 {
            *pc_visits.entry(pc).or_insert(0) += 1;
        }

        let result = vm.step();
        if matches!(result, StepResult::Ebreak) {
            break;
        }
    }

    // Find top PCs
    let mut sorted: Vec<_> = pc_visits.iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(a.1));
    eprintln!("[diag] Top 20 PCs (sampled every 1000 steps):");
    for (pc, cnt) in sorted.iter().take(20) {
        let inst = vm.bus.read_word(**pc as u64).unwrap_or(0);
        eprintln!("  PC=0x{:08X}: {} visits, inst=0x{:08X}", pc, cnt, inst);
    }
    eprintln!("[diag] SBI calls: {}", sbi_count);
}
