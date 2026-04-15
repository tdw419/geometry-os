fn main() {
    // Check the stack and key registers when the PTE corruption happens
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1";

    use geometry_os::riscv::RiscvVm;
    use geometry_os::riscv::cpu::StepResult;
    use geometry_os::riscv::csr;

    let (mut vm, _) = RiscvVm::boot_linux(&kernel_image, initramfs.as_deref(), 256, 16_500_000, bootargs).unwrap();

    let satp = vm.cpu.csr.read(csr::SATP);
    let ppn = satp & 0x3FFFFF;
    let pt_base = (ppn as u64) << 12;
    let pte_pa = pt_base + 770u64 * 4;
    let mut last_pte = vm.bus.read_word(pte_pa).unwrap_or(0);

    for count in 16_500_000..17_200_000 {
        let step_result = vm.step();

        if count % 1000 == 0 {
            let pte = vm.bus.read_word(pte_pa).unwrap_or(0);
            if pte != last_pte {
                println!("\n=== PTE CORRUPTION at count={} ===", count);
                println!("L1[770]: 0x{:08X} -> 0x{:08X}", last_pte, pte);
                println!("PC=0x{:08X}", vm.cpu.pc);
                
                // Dump all registers
                println!("\nRegisters:");
                for i in 0..32 {
                    let names = ["zero","ra","sp","gp","tp","t0","t1","t2","s0","s1",
                        "a0","a1","a2","a3","a4","a5","a6","a7","s2","s3","s4","s5",
                        "s6","s7","s8","s9","s10","s11","t3","t4","t5","t6"];
                    println!("  x{}({:>3}) = 0x{:08X}", i, names[i], vm.cpu.x[i]);
                }
                
                // Dump CSRs
                println!("\nCSRs:");
                println!("  satp   = 0x{:08X}", vm.cpu.csr.read(csr::SATP));
                println!("  stvec  = 0x{:08X}", vm.cpu.csr.stvec);
                println!("  sstatus= 0x{:08X}", vm.cpu.csr.read(csr::SSTATUS));
                println!("  sie    = 0x{:08X}", vm.cpu.csr.read(csr::SIE));
                println!("  sip    = 0x{:08X}", vm.cpu.csr.read(csr::SIP));
                println!("  sscratch=0x{:08X}", vm.cpu.csr.read(csr::SSCRATCH));
                
                // Read stack contents
                let sp = vm.cpu.x[2];
                println!("\nStack at sp=0x{:08X}:", sp);
                for i in 0..16 {
                    let addr = sp.wrapping_add((i as u32) * 4);
                    match vm.bus.read_word(addr as u64) {
                        Ok(val) => println!("  [sp+0x{:03X}] = 0x{:08X}", i*4, val),
                        Err(_) => println!("  [sp+0x{:03X}] = FAULT", i*4),
                    }
                }
                
                last_pte = pte;
                break;
            }
        }

        match step_result {
            StepResult::FetchFault | StepResult::LoadFault | StepResult::StoreFault => {
                println!("\n[FAULT] count={} PC=0x{:08X}", count, vm.cpu.pc);
                break;
            }
            StepResult::Ebreak => break,
            _ => {}
        }
    }
}
