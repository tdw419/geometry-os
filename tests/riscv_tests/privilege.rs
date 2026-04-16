use super::*;
// RISC-V privilege constants
#[allow(dead_code)]
pub(crate) const PRIV_USER: u8 = 0;
#[allow(dead_code)]
pub(crate) const PRIV_SUPERVISOR: u8 = 1;
#[allow(dead_code)]
pub(crate) const PRIV_MACHINE: u8 = 3;

// mstatus bit positions
pub(crate) const MSTATUS_MIE_BIT: u32 = 3;
pub(crate) const MSTATUS_MPIE_BIT: u32 = 7;
pub(crate) const MSTATUS_SIE_BIT: u32 = 1;
pub(crate) const MSTATUS_SPIE_BIT: u32 = 5;
pub(crate) const MSTATUS_SPP_BIT: u32 = 8;
pub(crate) const MSTATUS_MPP_LSB_BIT: u32 = 11;

/// Test U->S transition via ECALL when ECALL-U is delegated to S-mode.
/// Setup: CPU in U-mode, medeleg has bit 8 (ECALL-U) set, stvec configured.
/// ECALL should trap to stvec (S-mode handler).
#[test]

fn test_rv32_privilege_ecall_u_to_s() {
    // We need a bigger memory layout:
    // 0x80000000: entry (ECALL instruction)
    // 0x80000200: S-mode trap handler (stvec)
    let mut vm = RiscvVm::new(8192);
    let base = 0x8000_0000u64;

    // Write ECALL at entry point
    vm.bus.write_word(base, ecall()).expect("operation should succeed");

    // Write S-mode handler at 0x80000200: just ebreak
    vm.bus.write_word(base + 0x200, ebreak()).expect("operation should succeed");

    // Configure CPU: start in U-mode with delegation
    vm.cpu.pc = base as u32;
    vm.cpu.privilege = geometry_os::riscv::cpu::Privilege::User;

    // Delegate ECALL-U (cause 8) to S-mode
    vm.cpu.csr.medeleg = 1 << 8;

    // Set stvec to point to S-mode handler
    vm.cpu.csr.stvec = (base as u32) + 0x200;

    // Execute one step
    let result = vm.cpu.step(&mut vm.bus);
    assert_eq!(result, StepResult::Ok);
    assert_eq!(vm.cpu.pc, (base as u32) + 0x200, "should jump to stvec");
    assert_eq!(vm.cpu.privilege, geometry_os::riscv::cpu::Privilege::Supervisor);
    assert_eq!(vm.cpu.csr.scause, 8, "scause should be ECALL-U (8)");
    assert_eq!(vm.cpu.csr.sepc, base as u32, "sepc should be ECALL PC");
}

/// Test S->M transition via ECALL (no delegation for ECALL-S).
/// Setup: CPU in S-mode, mtvec configured. ECALL from S traps to M.
#[test]
fn test_rv32_privilege_ecall_s_to_m() {
    let mut vm = RiscvVm::new(8192);
    let base = 0x8000_0000u64;

    // Write ECALL at entry point
    vm.bus.write_word(base, ecall()).expect("operation should succeed");

    // Write M-mode handler at 0x80000400: just ebreak
    vm.bus.write_word(base + 0x400, ebreak()).expect("operation should succeed");

    vm.cpu.pc = base as u32;
    vm.cpu.privilege = geometry_os::riscv::cpu::Privilege::Supervisor;
    vm.cpu.csr.mtvec = (base as u32) + 0x400;

    // Set a7 to a non-SBI value so ECALL is NOT intercepted by SBI
    vm.cpu.x[17] = 0x999;

    let result = vm.cpu.step(&mut vm.bus);
    assert_eq!(result, StepResult::Ok);
    assert_eq!(vm.cpu.pc, (base as u32) + 0x400, "should jump to mtvec");
    assert_eq!(vm.cpu.privilege, geometry_os::riscv::cpu::Privilege::Machine);
    assert_eq!(vm.cpu.csr.mcause, 9, "mcause should be ECALL-S (9)");
    assert_eq!(vm.cpu.csr.mepc, base as u32, "mepc should be ECALL PC");
}

/// Test MRET: return from M-mode trap back to S-mode.
/// Simulates: M-mode handler runs, MRET returns to S-mode.
#[test]
fn test_rv32_privilege_mret_returns_to_s() {
    let mut vm = RiscvVm::new(8192);
    let base = 0x8000_0000u64;

    // Write MRET at 0x80000000
    vm.bus.write_word(base, mret()).expect("operation should succeed");

    // Write the code to return to at 0x80000200
    vm.bus.write_word(base + 0x200, ebreak()).expect("operation should succeed");

    vm.cpu.pc = base as u32;
    vm.cpu.privilege = geometry_os::riscv::cpu::Privilege::Machine;

    // Simulate state after trap from S to M:
    // MPP = S (01), mepc = return address
    vm.cpu.csr.mepc = (base as u32) + 0x200;
    vm.cpu.csr.mstatus = 0; // Clear mstatus
    vm.cpu.csr.mstatus |= (1u32 << MSTATUS_MPP_LSB_BIT) | (1u32 << MSTATUS_SPP_BIT); // MPP=S, SPP=S

    let result = vm.cpu.step(&mut vm.bus);
    assert_eq!(result, StepResult::Ok);
    assert_eq!(vm.cpu.pc, (base as u32) + 0x200, "MRET should jump to mepc");
    assert_eq!(vm.cpu.privilege, geometry_os::riscv::cpu::Privilege::Supervisor,
        "MRET should restore S-mode from MPP");
}

/// Test SRET: return from S-mode trap back to U-mode.
#[test]
fn test_rv32_privilege_sret_returns_to_u() {
    let mut vm = RiscvVm::new(8192);
    let base = 0x8000_0000u64;

    // Write SRET at 0x80000000
    vm.bus.write_word(base, sret()).expect("operation should succeed");

    // Write the code to return to at 0x80000200
    vm.bus.write_word(base + 0x200, ebreak()).expect("operation should succeed");

    vm.cpu.pc = base as u32;
    vm.cpu.privilege = geometry_os::riscv::cpu::Privilege::Supervisor;

    // Simulate state after trap from U to S:
    // SPP = U (0), sepc = return address
    vm.cpu.csr.sepc = (base as u32) + 0x200;
    vm.cpu.csr.mstatus = 0; // SPP = 0 (U-mode)

    let result = vm.cpu.step(&mut vm.bus);
    assert_eq!(result, StepResult::Ok);
    assert_eq!(vm.cpu.pc, (base as u32) + 0x200, "SRET should jump to sepc");
    assert_eq!(vm.cpu.privilege, geometry_os::riscv::cpu::Privilege::User,
        "SRET should restore U-mode from SPP");
}

/// Test full round-trip: U -> ECALL -> S handler -> SRET -> U
#[test]
fn test_rv32_privilege_u_ecall_sret_roundtrip() {
    let mut vm = RiscvVm::new(8192);
    let base = 0x8000_0000u64;

    // 0x80000000: ECALL (U-mode code)
    vm.bus.write_word(base, ecall()).expect("operation should succeed");
    // 0x80000004: addi x1, x0, 42 (returned here after SRET)
    vm.bus.write_word(base + 4, addi(1, 0, 42)).expect("operation should succeed");
    // 0x80000008: ebreak
    vm.bus.write_word(base + 8, ebreak()).expect("operation should succeed");

    // 0x80000200: S-mode trap handler -- SRET
    vm.bus.write_word(base + 0x200, sret()).expect("operation should succeed");

    vm.cpu.pc = base as u32;
    vm.cpu.privilege = geometry_os::riscv::cpu::Privilege::User;
    vm.cpu.csr.medeleg = 1 << 8; // Delegate ECALL-U to S
    vm.cpu.csr.stvec = (base as u32) + 0x200;
    vm.cpu.csr.mstatus = 1 << MSTATUS_SIE_BIT; // Enable SIE for SPIE save

    // Step 1: ECALL -> trap to S-mode handler
    vm.cpu.step(&mut vm.bus);
    assert_eq!(vm.cpu.privilege, geometry_os::riscv::cpu::Privilege::Supervisor);
    assert_eq!(vm.cpu.pc, (base as u32) + 0x200);

    // Step 2: SRET -> return to U-mode at sepc
    vm.cpu.step(&mut vm.bus);
    assert_eq!(vm.cpu.privilege, geometry_os::riscv::cpu::Privilege::User);
    assert_eq!(vm.cpu.pc, base as u32);

    // Step 3: Re-execute ECALL (sepc pointed to it). Actually sepc was the ECALL pc,
    // so we'll hit ECALL again. Let's instead check state.
    // sepc was set to the ECALL instruction address (0x80000000), so SRET returns
    // to 0x80000000 and we'll re-execute ECALL. Let's just verify the privilege was restored.
    assert_eq!(vm.cpu.privilege, geometry_os::riscv::cpu::Privilege::User);
}

/// Test full privilege chain: U -> ECALL -> S -> ECALL -> M -> MRET -> S -> SRET -> U
///
/// Memory layout:
///   0x80000000: ECALL (U-mode entry, traps to S via medeleg)
///   0x80000004: addi x1, x0, 42 (U-mode resume point)
///   0x80000008: EBREAK
///
///   0x80000200: S-mode handler
///     ECALL (S->M)
///     csrrs x5, x0, SEPC  (read sepc)
///     addi x5, x5, 4      (sepc += 4 to skip original ECALL)
///     csrrw x0, x5, SEPC  (write back)
///     SRET                 (return to U at sepc+4)
///
///   0x80000400: M-mode handler
///     csrrs x6, x0, MEPC  (read mepc)
///     addi x6, x6, 4      (mepc += 4 to skip S-mode ECALL)
///     csrrw x0, x6, MEPC  (write back)
///     MRET                 (return to S at mepc+4)
#[test]
fn test_rv32_privilege_full_chain_u_s_m_s_u() {
    let mut vm = RiscvVm::new(8192);
    let base = 0x8000_0000u64;
    use geometry_os::riscv::cpu::Privilege;

    // ---- U-mode code at base ----
    vm.bus.write_word(base, ecall()).expect("operation should succeed");              // 0x00: ECALL (U->S)
    vm.bus.write_word(base + 4, addi(1, 0, 42)).expect("operation should succeed");  // 0x04: x1 = 42 (after return)
    vm.bus.write_word(base + 8, ebreak()).expect("operation should succeed");         // 0x08: stop

    // ---- S-mode handler at base+0x200 ----
    vm.bus.write_word(base + 0x200, ecall()).expect("operation should succeed");               // ECALL (S->M)
    vm.bus.write_word(base + 0x204, csrrs(5, 0, CSR_SEPC)).expect("operation should succeed"); // x5 = sepc
    vm.bus.write_word(base + 0x208, addi(5, 5, 4)).expect("operation should succeed");         // x5 = sepc + 4
    vm.bus.write_word(base + 0x20C, csrrw(0, 5, CSR_SEPC)).expect("operation should succeed"); // sepc = x5
    vm.bus.write_word(base + 0x210, sret()).expect("operation should succeed");                 // return to U

    // ---- M-mode handler at base+0x400 ----
    vm.bus.write_word(base + 0x400, csrrs(6, 0, CSR_MEPC)).expect("operation should succeed"); // x6 = mepc
    vm.bus.write_word(base + 0x404, addi(6, 6, 4)).expect("operation should succeed");         // x6 = mepc + 4
    vm.bus.write_word(base + 0x408, csrrw(0, 6, CSR_MEPC)).expect("operation should succeed"); // mepc = x6
    vm.bus.write_word(base + 0x40C, mret()).expect("operation should succeed");                 // return to S

    // ---- CPU setup ----
    vm.cpu.pc = base as u32;
    vm.cpu.privilege = Privilege::User;
    vm.cpu.csr.medeleg = 1 << 8;  // Delegate ECALL-U to S
    vm.cpu.csr.stvec = (base as u32) + 0x200;
    vm.cpu.csr.mtvec = (base as u32) + 0x400;

    // Step 1: U-mode ECALL -> traps to S (delegated via medeleg)
    let r = vm.cpu.step(&mut vm.bus);
    assert_eq!(r, StepResult::Ok);
    assert_eq!(vm.cpu.privilege, Privilege::Supervisor, "after U ECALL -> S");
    assert_eq!(vm.cpu.pc, (base as u32) + 0x200, "S handler entry");
    assert_eq!(vm.cpu.csr.scause, 8, "scause = ECALL-U (8)");
    assert_eq!(vm.cpu.csr.sepc, base as u32, "sepc = ECALL PC");
    // SPP should be 0 (came from U), SPIE should hold old SIE
    let spp = (vm.cpu.csr.mstatus >> MSTATUS_SPP_BIT) & 1;
    assert_eq!(spp, 0, "SPP = 0 (came from U)");

    // Step 2: S-mode ECALL -> traps to M (not delegated)
    // Set a7 to a non-SBI value so ECALL is NOT intercepted by SBI
    vm.cpu.x[17] = 0x999;
    let r = vm.cpu.step(&mut vm.bus);
    assert_eq!(r, StepResult::Ok);
    assert_eq!(vm.cpu.privilege, Privilege::Machine, "after S ECALL -> M");
    assert_eq!(vm.cpu.pc, (base as u32) + 0x400, "M handler entry");
    assert_eq!(vm.cpu.csr.mcause, 9, "mcause = ECALL-S (9)");
    assert_eq!(vm.cpu.csr.mepc, (base as u32) + 0x200, "mepc = S-mode ECALL PC");
    // MPP should be 1 (came from S)
    let mpp = (vm.cpu.csr.mstatus >> MSTATUS_MPP_LSB_BIT) & 0x3;
    assert_eq!(mpp, 1, "MPP = 1 (came from S)");

    // Step 3: M-mode reads mepc, adds 4, writes back, then MRET -> returns to S
    vm.cpu.step(&mut vm.bus); // csrrs x6, x0, mepc
    vm.cpu.step(&mut vm.bus); // addi x6, x6, 4
    vm.cpu.step(&mut vm.bus); // csrrw x0, x6, mepc
    let r = vm.cpu.step(&mut vm.bus); // MRET
    assert_eq!(r, StepResult::Ok);
    assert_eq!(vm.cpu.privilege, Privilege::Supervisor, "after MRET -> S");
    assert_eq!(vm.cpu.pc, (base as u32) + 0x204, "S resumes after its ECALL");
    // Verify mepc was advanced
    assert_eq!(vm.cpu.csr.mepc, (base as u32) + 0x204, "mepc advanced past S ECALL");

    // Step 4: S-mode advances sepc and SRET -> returns to U
    vm.cpu.step(&mut vm.bus); // csrrs x5, x0, sepc
    assert_eq!(vm.cpu.x[5], base as u32, "sepc should be original ECALL PC");
    vm.cpu.step(&mut vm.bus); // addi x5, x5, 4
    vm.cpu.step(&mut vm.bus); // csrrw x0, x5, sepc
    let r = vm.cpu.step(&mut vm.bus); // SRET
    assert_eq!(r, StepResult::Ok);
    assert_eq!(vm.cpu.privilege, Privilege::User, "after SRET -> U");
    assert_eq!(vm.cpu.pc, (base as u32) + 4, "U resumes at addi");

    // Step 5: U-mode executes addi x1, x0, 42
    vm.cpu.step(&mut vm.bus);
    assert_eq!(vm.cpu.x[1], 42, "U-mode code runs after full chain");

    // Step 6: EBREAK stops
    let r = vm.cpu.step(&mut vm.bus);
    assert_eq!(r, StepResult::Ebreak, "should stop at EBREAK");
}

/// Test mstatus state preservation across U->S ECALL trap.
/// Verifies SPP, SPIE, SIE bits are set correctly.
#[test]
fn test_rv32_privilege_ecall_u_to_s_mstatus() {
    let mut vm = RiscvVm::new(8192);
    let base = 0x8000_0000u64;
    use geometry_os::riscv::cpu::Privilege;

    vm.bus.write_word(base, ecall()).expect("operation should succeed");
    vm.bus.write_word(base + 0x200, ebreak()).expect("operation should succeed");

    vm.cpu.pc = base as u32;
    vm.cpu.privilege = Privilege::User;
    vm.cpu.csr.medeleg = 1 << 8;
    vm.cpu.csr.stvec = (base as u32) + 0x200;

    // Set SIE=1 before trap so we can verify it saves to SPIE
    vm.cpu.csr.mstatus = 1 << MSTATUS_SIE_BIT;

    vm.cpu.step(&mut vm.bus);

    // SPP = 0 (came from U)
    assert_eq!((vm.cpu.csr.mstatus >> MSTATUS_SPP_BIT) & 1, 0,
        "SPP should be 0 (came from U)");
    // SPIE = old SIE (1)
    assert_eq!((vm.cpu.csr.mstatus >> MSTATUS_SPIE_BIT) & 1, 1,
        "SPIE should be old SIE (1)");
    // SIE = 0 (disabled during trap)
    assert_eq!((vm.cpu.csr.mstatus >> MSTATUS_SIE_BIT) & 1, 0,
        "SIE should be 0 (disabled during trap handler)");
}

/// Test mstatus state preservation across S->M ECALL trap.
/// Verifies MPP, MPIE, MIE bits are set correctly.
#[test]
fn test_rv32_privilege_ecall_s_to_m_mstatus() {
    let mut vm = RiscvVm::new(8192);
    let base = 0x8000_0000u64;
    use geometry_os::riscv::cpu::Privilege;

    vm.bus.write_word(base, ecall()).expect("operation should succeed");
    vm.bus.write_word(base + 0x400, ebreak()).expect("operation should succeed");

    vm.cpu.pc = base as u32;
    vm.cpu.privilege = Privilege::Supervisor;
    vm.cpu.csr.mtvec = (base as u32) + 0x400;

    // Set a7 to a non-SBI value so ECALL is NOT intercepted by SBI
    vm.cpu.x[17] = 0x999;

    // Set MIE=1 before trap
    vm.cpu.csr.mstatus = 1 << MSTATUS_MIE_BIT;

    vm.cpu.step(&mut vm.bus);

    // MPP = 01 (came from S)
    let mpp = (vm.cpu.csr.mstatus >> MSTATUS_MPP_LSB_BIT) & 0x3;
    assert_eq!(mpp, 1, "MPP should be 01 (came from S)");
    // MPIE = old MIE (1)
    assert_eq!((vm.cpu.csr.mstatus >> MSTATUS_MPIE_BIT) & 1, 1,
        "MPIE should be old MIE (1)");
    // MIE = 0 (disabled during trap)
    assert_eq!((vm.cpu.csr.mstatus >> MSTATUS_MIE_BIT) & 1, 0,
        "MIE should be 0 (disabled during trap handler)");
}

/// Test MRET restores mstatus: MIE from MPIE, MPP to privilege, MPIE=1.
#[test]
fn test_rv32_privilege_mret_mstatus_restore() {
    let mut vm = RiscvVm::new(8192);
    let base = 0x8000_0000u64;
    use geometry_os::riscv::cpu::Privilege;

    vm.bus.write_word(base, mret()).expect("operation should succeed");

    vm.cpu.pc = base as u32;
    vm.cpu.privilege = Privilege::Machine;
    vm.cpu.csr.mepc = (base as u32) + 0x100;

    // Simulate trap from S: MPP=S(01), MPIE=1, MIE=0
    vm.cpu.csr.mstatus = 0;
    vm.cpu.csr.mstatus |= 1u32 << MSTATUS_MPP_LSB_BIT; // MPP = S (01)
    vm.cpu.csr.mstatus |= 1u32 << MSTATUS_MPIE_BIT;     // MPIE = 1
    // MIE = 0 (cleared during trap)

    vm.cpu.step(&mut vm.bus);

    assert_eq!(vm.cpu.privilege, Privilege::Supervisor);
    assert_eq!(vm.cpu.pc, (base as u32) + 0x100);
    // MIE restored from MPIE (1)
    assert_eq!((vm.cpu.csr.mstatus >> MSTATUS_MIE_BIT) & 1, 1,
        "MIE should be restored from MPIE");
    // MPIE set to 1
    assert_eq!((vm.cpu.csr.mstatus >> MSTATUS_MPIE_BIT) & 1, 1,
        "MPIE should be 1 after MRET");
    // MPP cleared to U (00)
    assert_eq!((vm.cpu.csr.mstatus >> MSTATUS_MPP_LSB_BIT) & 0x3, 0,
        "MPP should be 0 (U) after MRET");
}

/// Test SRET restores mstatus: SIE from SPIE, SPP to privilege, SPIE=1.
#[test]
fn test_rv32_privilege_sret_mstatus_restore() {
    let mut vm = RiscvVm::new(8192);
    let base = 0x8000_0000u64;
    use geometry_os::riscv::cpu::Privilege;

    vm.bus.write_word(base, sret()).expect("operation should succeed");

    vm.cpu.pc = base as u32;
    vm.cpu.privilege = Privilege::Supervisor;
    vm.cpu.csr.sepc = (base as u32) + 0x100;

    // Simulate trap from U: SPP=0 (U), SPIE=1, SIE=0
    vm.cpu.csr.mstatus = 0;
    vm.cpu.csr.mstatus |= 1u32 << MSTATUS_SPIE_BIT; // SPIE = 1
    // SPP = 0 (U), SIE = 0

    vm.cpu.step(&mut vm.bus);

    assert_eq!(vm.cpu.privilege, Privilege::User);
    assert_eq!(vm.cpu.pc, (base as u32) + 0x100);
    // SIE restored from SPIE (1)
    assert_eq!((vm.cpu.csr.mstatus >> MSTATUS_SIE_BIT) & 1, 1,
        "SIE should be restored from SPIE");
    // SPIE set to 1
    assert_eq!((vm.cpu.csr.mstatus >> MSTATUS_SPIE_BIT) & 1, 1,
        "SPIE should be 1 after SRET");
    // SPP cleared to 0 (U)
    assert_eq!((vm.cpu.csr.mstatus >> MSTATUS_SPP_BIT) & 1, 0,
        "SPP should be 0 (U) after SRET");
}

/// Test that ECALL from U without delegation goes directly to M-mode.
#[test]
fn test_rv32_privilege_ecall_u_to_m_no_delegation() {
    let mut vm = RiscvVm::new(8192);
    let base = 0x8000_0000u64;
    use geometry_os::riscv::cpu::Privilege;

    vm.bus.write_word(base, ecall()).expect("operation should succeed");
    vm.bus.write_word(base + 0x400, ebreak()).expect("operation should succeed");

    vm.cpu.pc = base as u32;
    vm.cpu.privilege = Privilege::User;
    vm.cpu.csr.mtvec = (base as u32) + 0x400;
    // No medeleg: all exceptions go to M

    vm.cpu.step(&mut vm.bus);

    assert_eq!(vm.cpu.privilege, Privilege::Machine,
        "ECALL from U should trap to M when not delegated");
    assert_eq!(vm.cpu.pc, (base as u32) + 0x400);
    assert_eq!(vm.cpu.csr.mcause, 8, "mcause = ECALL-U (8)");
    assert_eq!(vm.cpu.csr.mepc, base as u32);
    // MPP should be 0 (came from U)
    let mpp = (vm.cpu.csr.mstatus >> MSTATUS_MPP_LSB_BIT) & 0x3;
    assert_eq!(mpp, 0, "MPP = 0 (came from U)");
}

/// Test timer interrupt delivery: set MTIP in MIP, enable MTIE in MIE,
/// enable MIE in mstatus. Next step should deliver interrupt to mtvec.
#[test]
fn test_rv32_privilege_timer_interrupt_delivery() {
    let mut vm = RiscvVm::new(8192);
    let base = 0x8000_0000u64;

    // Write NOP at entry (should be preempted by interrupt)
    vm.bus.write_word(base, nop()).expect("operation should succeed");
    // Write handler at 0x80000200
    vm.bus.write_word(base + 0x200, ebreak()).expect("operation should succeed");

    vm.cpu.pc = base as u32;
    vm.cpu.privilege = geometry_os::riscv::cpu::Privilege::Machine;

    // Enable machine timer interrupt
    vm.cpu.csr.mip = 1 << 7;  // MTIP pending (bit 7 = INT_MTI)
    vm.cpu.csr.mie = 1 << 7;  // MTIE enabled
    vm.cpu.csr.mstatus = 1 << MSTATUS_MIE_BIT; // Global MIE enabled
    vm.cpu.csr.mtvec = (base as u32) + 0x200;

    let result = vm.cpu.step(&mut vm.bus);
    assert_eq!(result, StepResult::Ok);
    assert_eq!(vm.cpu.pc, (base as u32) + 0x200,
        "timer interrupt should jump to mtvec");
    assert_eq!(vm.cpu.csr.mcause, 0x80000007,
        "mcause should be interrupt bit | MTI (7)");
    assert_eq!(vm.cpu.csr.mepc, base as u32,
        "mepc should be PC of preempted instruction");
}

/// Test software interrupt delivery: set SSIP, enable SSIE, enable SIE.
#[test]
fn test_rv32_privilege_software_interrupt_delivery() {
    let mut vm = RiscvVm::new(8192);
    let base = 0x8000_0000u64;

    vm.bus.write_word(base, nop()).expect("operation should succeed");
    vm.bus.write_word(base + 0x200, ebreak()).expect("operation should succeed");

    vm.cpu.pc = base as u32;
    vm.cpu.privilege = geometry_os::riscv::cpu::Privilege::User;

    // Enable supervisor software interrupt
    vm.cpu.csr.mip = 1 << 1;  // SSIP pending (bit 1 = INT_SSI)
    vm.cpu.csr.mie = 1 << 1;  // SSIE enabled
    vm.cpu.csr.mstatus = 1 << MSTATUS_SIE_BIT; // Global SIE enabled
    vm.cpu.csr.mtvec = (base as u32) + 0x200;

    // No delegation -- goes to M-mode
    let result = vm.cpu.step(&mut vm.bus);
    assert_eq!(result, StepResult::Ok);
    assert_eq!(vm.cpu.pc, (base as u32) + 0x200,
        "software interrupt should jump to mtvec (M-mode, no delegation)");
    assert_eq!(vm.cpu.csr.mcause, 0x80000001,
        "mcause should be interrupt bit | SSI (1)");
    assert_eq!(vm.cpu.privilege, geometry_os::riscv::cpu::Privilege::Machine);
}

/// Test that MIE/MIP CSR read/write works via instruction.
#[test]
fn test_rv32_csr_mie_mip_rw() {
    let mut vm = test_vm(&[
        addi(1, 0, 1 << 7),      // x1 = 0x80 (MTIE bit)
        csrrw(0, 1, CSR_MIE),    // Write to MIE
        csrrs(2, 0, CSR_MIE),    // Read MIE into x2
        ebreak(),
    ]);
    run(&mut vm, 100);
    assert_eq!(vm.cpu.csr.mie, 1 << 7);
    assert_eq!(vm.cpu.x[2], 1 << 7);
}

/// Test SIE is a restricted view of MIE via instruction.
#[test]
fn test_rv32_csr_sie_view() {
    let mut vm = test_vm(&[
        addi(1, 0, (1 << 7) | (1 << 5)),  // x1 = MTIE | STIE
        csrrw(0, 1, CSR_MIE),              // Write to MIE
        csrrs(2, 0, CSR_SIE),              // Read SIE (restricted view)
        ebreak(),
    ]);
    run(&mut vm, 100);
    // SIE should only show S-mode bits (STIE at bit 5)
    assert_eq!(vm.cpu.x[2], 1 << 5, "SIE should be restricted view of MIE");
}

/// Test medeleg delegation via instruction execution.
#[test]
fn test_rv32_csr_medeleg_rw() {
    let mut vm = test_vm(&[
        addi(1, 0, 1 << 8),           // x1 = delegate ECALL-U
        csrrw(0, 1, CSR_MEDELEG),     // Write to medeleg
        csrrs(2, 0, CSR_MEDELEG),     // Read back
        ebreak(),
    ]);
    run(&mut vm, 100);
    assert_eq!(vm.cpu.csr.medeleg, 1 << 8);
    assert_eq!(vm.cpu.x[2], 1 << 8);
}

/// Test no interrupt fires when globally disabled (MIE bit = 0).
#[test]
fn test_rv32_privilege_no_interrupt_when_disabled() {
    let mut vm = RiscvVm::new(8192);
    let base = 0x8000_0000u64;

    vm.bus.write_word(base, addi(1, 0, 42)).expect("operation should succeed");
    vm.bus.write_word(base + 4, ebreak()).expect("operation should succeed");

    vm.cpu.pc = base as u32;
    vm.cpu.privilege = geometry_os::riscv::cpu::Privilege::Machine;

    // Timer pending and enabled in MIE, but MIE bit in mstatus is 0
    vm.cpu.csr.mip = 1 << 7;  // MTIP pending
    vm.cpu.csr.mie = 1 << 7;  // MTIE enabled
    vm.cpu.csr.mstatus = 0;   // Global MIE disabled!
    vm.cpu.csr.mtvec = (base as u32) + 0x200;

    // Step should execute the instruction normally, not deliver interrupt
    let result = vm.cpu.step(&mut vm.bus);
    assert_eq!(result, StepResult::Ok);
    assert_eq!(vm.cpu.x[1], 42, "instruction should execute normally");
    assert_eq!(vm.cpu.pc, (base as u32) + 4, "PC should advance normally");
}

// ============================================================
// CLINT integration: timer + software interrupt via RiscvVm::step()
// ============================================================

/// Helper: run vm.step() N times.
pub(crate) fn run_vm(vm: &mut RiscvVm, steps: usize) {
    for _ in 0..steps {
        match vm.step() {
            StepResult::Ecall | StepResult::Ebreak | StepResult::FetchFault => break,
            StepResult::Ok | StepResult::LoadFault | StepResult::StoreFault => {}
        }
    }
}
