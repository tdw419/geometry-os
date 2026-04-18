//! Watch initial_boot_params through setup_vm().
//! cargo run --example watch_ibp

use geometry_os::riscv::RiscvVm;
use geometry_os::riscv::cpu::{StepResult, Privilege};

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let ir_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_data = std::fs::read(kernel_path).expect("kernel");
    let initramfs_data = std::path::Path::new(ir_path)
        .exists()
        .then(|| std::fs::read(ir_path).unwrap());

    let (mut vm, fw_addr, _entry, dtb_addr) =
        RiscvVm::boot_linux_setup(
            &kernel_data,
            initramfs_data.as_deref(),
            512,
            "console=ttyS0 earlycon=sbi",
        ).expect("setup failed");

    let ibp_pa = 0x00C7A178u64;
    let deva_pa = 0x00801008u64;
    let depa_pa = 0x0080100Cu64;
    let prb_pa = 0x00C79EACu64;
    let mb_pa = 0x00803448u64;
    let fw_u32 = fw_addr as u32;
    let dtb_va_exp = dtb_addr.wrapping_add(0xC0000000) as u32;
    let mut last_satp = vm.cpu.csr.satp;

    // Check every 1000 instructions around the setup_vm() region
    let mut count: u64 = 0;
    let max = 300_000u64;
    let mut last_ibp: u32 = vm.bus.read_word(ibp_pa).unwrap_or(0);

    while count < max {
        // Trap handling
        if vm.cpu.pc == fw_u32 && vm.cpu.privilege == Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            if cause_code != 11 {
                let mpp = (vm.cpu.csr.mstatus >> 11) & 3;
                if mpp != 3 {
                    let stvec = vm.cpu.csr.stvec & !0x3u32;
                    if stvec != 0 {
                        vm.cpu.csr.sepc = vm.cpu.csr.mepc;
                        vm.cpu.csr.scause = mcause;
                        vm.cpu.csr.stval = vm.cpu.csr.mtval;
                        vm.cpu.pc = stvec;
                        vm.cpu.privilege = Privilege::Supervisor;
                        vm.cpu.tlb.flush_all();
                        count += 1;
                        continue;
                    }
                }
                let fault_addr = vm.cpu.csr.mtval;
                let is_pf = cause_code == 12 || cause_code == 13 || cause_code == 15;
                if is_pf && fault_addr < 0x0200_0000 {
                    let satp = vm.cpu.csr.satp;
                    let pg_dir_ppn = (satp & 0x3FFFFF) as u64;
                    if pg_dir_ppn > 0 {
                        let pg_dir_phys = pg_dir_ppn * 4096;
                        let vpn1 = ((fault_addr >> 22) & 0x3FF) as u64;
                        let l1_addr = pg_dir_phys + vpn1 * 4;
                        let existing = vm.bus.read_word(l1_addr).unwrap_or(0);
                        if (existing & 1) == 0 {
                            let pte: u32 = 0x0000_00CF | ((vpn1 as u32) << 20);
                            vm.bus.write_word(l1_addr, pte).ok();
                            vm.cpu.tlb.flush_all();
                        }
                    }
                }
            }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }

        vm.bus.tick_clint();
        vm.bus.sync_mip(&mut vm.cpu.csr.mip);
        let _ = vm.step();

        // Check state every 1000 instructions
        if count % 1000 == 0 {
            let ibp = vm.bus.read_word(ibp_pa).unwrap_or(0);
            let deva = vm.bus.read_word(deva_pa).unwrap_or(0);
            let depa = vm.bus.read_word(depa_pa).unwrap_or(0);
            let prb = vm.bus.read_word(prb_pa).unwrap_or(0);
            let mem_cnt = vm.bus.read_word(mb_pa + 8).unwrap_or(0);
            let satp = vm.cpu.csr.satp;

            if ibp != last_ibp || satp != last_satp || mem_cnt > 0 || prb != 0 {
                eprintln!("[{}] PC=0x{:08X} satp=0x{:08X} ibp=0x{:08X} deva=0x{:08X} depa=0x{:08X} prb=0x{:08X} mem.cnt={}",
                    count, vm.cpu.pc, satp, ibp, deva, depa, prb, mem_cnt);
                last_ibp = ibp;
                last_satp = satp;
            }
        }

        count += 1;
    }

    eprintln!("\nFinal state at {}:", count);
    eprintln!("  PC=0x{:08X} priv={:?}", vm.cpu.pc, vm.cpu.privilege);
    eprintln!("  satp=0x{:08X}", vm.cpu.csr.satp);
    eprintln!("  stvec=0x{:08X}", vm.cpu.csr.stvec);
    eprintln!("  initial_boot_params=0x{:08X}", vm.bus.read_word(ibp_pa).unwrap_or(0));
    eprintln!("  _dtb_early_va=0x{:08X}", vm.bus.read_word(deva_pa).unwrap_or(0));
    eprintln!("  _dtb_early_pa=0x{:08X}", vm.bus.read_word(depa_pa).unwrap_or(0));
    eprintln!("  phys_ram_base=0x{:08X}", vm.bus.read_word(prb_pa).unwrap_or(0));
    eprintln!("  memblock memory.cnt={}", vm.bus.read_word(mb_pa + 8).unwrap_or(0));

    // Try reading DTB at initial_boot_params address through current page table
    let ibp_val = vm.bus.read_word(ibp_pa).unwrap_or(0);
    if ibp_val != 0 {
        // DTB is at PA ibp_val. Read magic.
        let magic = vm.bus.read_word(ibp_val as u64).unwrap_or(0);
        eprintln!("  DTB magic at PA 0x{:08X}: 0x{:08X}", ibp_val, magic);
    }
}
