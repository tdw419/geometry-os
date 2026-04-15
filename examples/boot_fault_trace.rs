/// Capture the first S-mode page fault with full register dump,
/// then trace 200 more instructions to see what the handler does.
fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1";

    use geometry_os::riscv::RiscvVm;
    use geometry_os::riscv::cpu::{Privilege, StepResult};
    use geometry_os::riscv::csr;

    let (mut vm, fw_addr, _entry, _dtb_addr) = RiscvVm::boot_linux_setup(
        &kernel_image, initramfs.as_deref(), 256, bootargs
    ).unwrap();

    let fw_addr_u32 = fw_addr as u32;
    let mut count: u64 = 0;
    let max_instructions: u64 = 20_000_000;
    let mut sbi_call_count: u64 = 0;
    let mut forward_count: u64 = 0;
    let mut fault_seen = false;
    let mut smode_faults: u64 = 0;

    while count < max_instructions {
        if vm.bus.sbi.shutdown_requested {
            eprintln!("[trace] SBI shutdown at count={}", count);
            break;
        }

        // Handle M-mode trap
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);

            if cause_code == csr::CAUSE_ECALL_S {
                sbi_call_count += 1;
                let result = vm.bus.sbi.handle_ecall(
                    vm.cpu.x[17], vm.cpu.x[16],
                    vm.cpu.x[10], vm.cpu.x[11],
                    vm.cpu.x[12], vm.cpu.x[13],
                    vm.cpu.x[14], vm.cpu.x[15],
                    &mut vm.bus.uart, &mut vm.bus.clint,
                );
                if let Some((a0_val, a1_val)) = result {
                    vm.cpu.x[10] = a0_val;
                    vm.cpu.x[11] = a1_val;
                }
            } else if cause_code != csr::CAUSE_ECALL_M {
                let mpp = (vm.cpu.csr.mstatus & csr::MSTATUS_MPP_MASK) >> csr::MSTATUS_MPP_LSB;
                if mpp != 3 {
                    let stvec = vm.cpu.csr.stvec & !0x3u32;
                    if stvec != 0 {
                        vm.cpu.csr.sepc = vm.cpu.csr.mepc;
                        vm.cpu.csr.scause = mcause;
                        vm.cpu.csr.stval = vm.cpu.csr.mtval;
                        let spp = if mpp == 1 { 1u32 } else { 0u32 };
                        vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << csr::MSTATUS_SPP))
                            | (spp << csr::MSTATUS_SPP);
                        let sie = (vm.cpu.csr.mstatus >> csr::MSTATUS_SIE) & 1;
                        vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << csr::MSTATUS_SPIE))
                            | (sie << csr::MSTATUS_SPIE);
                        vm.cpu.csr.mstatus &= !(1 << csr::MSTATUS_SIE);
                        vm.cpu.pc = stvec;
                        vm.cpu.privilege = Privilege::Supervisor;
                        vm.cpu.tlb.flush_all();
                        forward_count += 1;
                        count += 1;
                        continue;
                    }
                }
            } else {
                let result = vm.bus.sbi.handle_ecall(
                    vm.cpu.x[17], vm.cpu.x[16],
                    vm.cpu.x[10], vm.cpu.x[11],
                    vm.cpu.x[12], vm.cpu.x[13],
                    vm.cpu.x[14], vm.cpu.x[15],
                    &mut vm.bus.uart, &mut vm.bus.clint,
                );
                if let Some((a0_val, a1_val)) = result {
                    vm.cpu.x[10] = a0_val;
                    vm.cpu.x[11] = a1_val;
                }
            }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }

        let pc_before = vm.cpu.pc;
        let step_result = vm.step();
        let pc_after = vm.cpu.pc;

        // Check for S-mode faults
        match &step_result {
            StepResult::FetchFault | StepResult::LoadFault | StepResult::StoreFault => {
                if vm.cpu.privilege == Privilege::Supervisor {
                    smode_faults += 1;
                    
                    if !fault_seen {
                        fault_seen = true;
                        let fault_type = match step_result {
                            StepResult::FetchFault => "FETCH",
                            StepResult::LoadFault => "LOAD",
                            StepResult::StoreFault => "STORE",
                            _ => unreachable!(),
                        };
                        eprintln!("\n=== FIRST S-MODE {} FAULT at count={} ===", fault_type, count);
                        eprintln!("PC before step: 0x{:08X}", pc_before);
                        eprintln!("PC after step:  0x{:08X} (should be stvec)", pc_after);
                        eprintln!("sepc=0x{:08X} scause=0x{:08X} stval=0x{:08X} stvec=0x{:08X}",
                            vm.cpu.csr.sepc, vm.cpu.csr.scause, vm.cpu.csr.stval, vm.cpu.csr.stvec);
                        eprintln!("SSCRATCH=0x{:08X}", vm.cpu.csr.read(csr::SSCRATCH));
                        eprintln!("SATP=0x{:08X} SSTATUS=0x{:08X}",
                            vm.cpu.csr.read(csr::SATP), vm.cpu.csr.read(csr::SSTATUS));
                        
                        // Dump all registers
                        let names = ["zero","ra","sp","gp","tp","t0","t1","t2","s0","s1",
                            "a0","a1","a2","a3","a4","a5","a6","a7","s2","s3",
                            "s4","s5","s6","s7","s8","s9","s10","s11","t3","t4","t5","t6"];
                        for i in 0..32 {
                            eprintln!("  x{:2} ({:4}) = 0x{:08X}", i, names[i], vm.cpu.x[i]);
                        }
                        
                        // Now trace 200 more instructions
                        eprintln!("\n=== Tracing 200 instructions after fault ===");
                        for j in 0..200 {
                            let trace_pc = vm.cpu.pc;
                            let trace_priv = vm.cpu.privilege;
                            
                            // Handle M-mode trap in trace
                            if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
                                let mcause = vm.cpu.csr.mcause;
                                let cause_code = mcause & !(1u32 << 31);
                                if cause_code == csr::CAUSE_ECALL_S {
                                    sbi_call_count += 1;
                                    let result = vm.bus.sbi.handle_ecall(
                                        vm.cpu.x[17], vm.cpu.x[16],
                                        vm.cpu.x[10], vm.cpu.x[11],
                                        vm.cpu.x[12], vm.cpu.x[13],
                                        vm.cpu.x[14], vm.cpu.x[15],
                                        &mut vm.bus.uart, &mut vm.bus.clint,
                                    );
                                    if let Some((a0_val, a1_val)) = result {
                                        vm.cpu.x[10] = a0_val;
                                        vm.cpu.x[11] = a1_val;
                                    }
                                }
                                vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
                            }
                            
                            let sr = vm.step();
                            let new_pc = vm.cpu.pc;
                            let new_priv = vm.cpu.privilege;
                            
                            if j < 80 || matches!(sr, StepResult::FetchFault | StepResult::LoadFault | StepResult::StoreFault) {
                                let fault_marker = match sr {
                                    StepResult::FetchFault => " [FETCH FAULT]",
                                    StepResult::LoadFault => " [LOAD FAULT]",
                                    StepResult::StoreFault => " [STORE FAULT]",
                                    _ => "",
                                };
                                eprintln!("  [{:3}] PC 0x{:08X} -> 0x{:08X} priv={:?}{}",
                                    j, trace_pc, new_pc, trace_priv, fault_marker);
                            }
                            
                            if matches!(sr, StepResult::FetchFault | StepResult::LoadFault | StepResult::StoreFault) {
                                eprintln!("        sepc=0x{:08X} scause=0x{:08X} stval=0x{:08X}",
                                    vm.cpu.csr.sepc, vm.cpu.csr.scause, vm.cpu.csr.stval);
                            }
                            
                            count += 1;
                        }
                        
                        eprintln!("\n=== State after 200 trace instructions ===");
                        eprintln!("PC=0x{:08X} priv={:?}", vm.cpu.pc, vm.cpu.privilege);
                        eprintln!("sp=0x{:08X} tp=0x{:08X} ra=0x{:08X}",
                            vm.cpu.x[2], vm.cpu.x[4], vm.cpu.x[1]);
                        eprintln!("SSCRATCH=0x{:08X}", vm.cpu.csr.read(csr::SSCRATCH));
                        
                        break;
                    }
                }
            }
            _ => {}
        }

        count += 1;

        if count % 5_000_000 == 0 {
            eprintln!("[trace] count={}M PC=0x{:08X} priv={:?} ecall={} sbi={} smode_faults={}",
                count / 1_000_000, vm.cpu.pc, vm.cpu.privilege, vm.cpu.ecall_count, sbi_call_count, smode_faults);
        }
    }
}
