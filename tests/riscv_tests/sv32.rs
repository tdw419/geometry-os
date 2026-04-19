use super::*;

// Phase 36: SV32 Page Table Walk Tests
// =====================================================================

use geometry_os::riscv::mmu;

pub fn make_pte(ppn: u32, flags: u32) -> u32 {
    ((ppn & 0x003F_FFFF) << 10) | (flags & 0x3FF)
}

pub fn make_satp(mode: u32, asid: u32, ppn: u32) -> u32 {
    ((mode & 1) << 31) | ((asid & 0x1FF) << 22) | (ppn & 0x003F_FFFF)
}

pub fn sfence_vma(rs1: u8, rs2: u8) -> u32 {
    (0b0001001u32 << 25)
        | ((rs2 as u32) << 20)
        | ((rs1 as u32) << 15)
        | (0b000 << 12)
        | (0u32 << 7)
        | 0x73
}

#[test]
fn test_sv32_bare_mode_identity_translation() {
    let mut tlb = mmu::Tlb::new();
    let mut bus = geometry_os::riscv::bus::Bus::new(0x8000_0000, 8192);
    let result = mmu::translate(
        0x8000_0000,
        mmu::AccessType::Fetch,
        geometry_os::riscv::cpu::Privilege::Supervisor,
        false,
        false,
        0,
        &mut bus,
        &mut tlb,
    );
    assert_eq!(result, mmu::TranslateResult::Ok(0x8000_0000));
}

#[test]
fn test_sv32_two_level_walk_4k_page() {
    let mut tlb = mmu::Tlb::new();
    let mut bus = geometry_os::riscv::bus::Bus::new(0x0, 0x1_0000);
    let root_ppn: u32 = 1;
    let l2_ppn: u32 = 2;
    let data_ppn: u32 = 3;
    bus.write_word((data_ppn as u64) << 12, 0xDEAD_BEEF)
        .expect("operation should succeed");
    bus.write_word((root_ppn as u64) << 12, make_pte(l2_ppn, mmu::PTE_V))
        .expect("operation should succeed");
    bus.write_word(
        (l2_ppn as u64) << 12,
        make_pte(
            data_ppn,
            mmu::PTE_V | mmu::PTE_R | mmu::PTE_W | mmu::PTE_X | mmu::PTE_U,
        ),
    )
    .expect("operation should succeed");
    let satp = make_satp(1, 0, root_ppn);
    let result = mmu::translate(
        0x0000_0000,
        mmu::AccessType::Load,
        geometry_os::riscv::cpu::Privilege::User,
        false,
        false,
        satp,
        &mut bus,
        &mut tlb,
    );
    assert_eq!(result, mmu::TranslateResult::Ok((data_ppn as u64) << 12));
    if let mmu::TranslateResult::Ok(pa) = result {
        assert_eq!(
            bus.read_word(pa).expect("operation should succeed"),
            0xDEAD_BEEF
        );
    }
}

#[test]
fn test_sv32_nonzero_vpn_and_offset() {
    let mut tlb = mmu::Tlb::new();
    let mut bus = geometry_os::riscv::bus::Bus::new(0x0, 0x2_0000);
    let root_ppn: u32 = 1;
    let l2_ppn: u32 = 2;
    let data_ppn: u32 = 4;
    let va: u32 = 0x0040_1100;
    let vpn1 = (va >> 22) & 0x3FF;
    let vpn0 = (va >> 12) & 0x3FF;
    bus.write_word(((data_ppn as u64) << 12) + 0x100, 0x1234_5678)
        .expect("operation should succeed");
    bus.write_word(
        ((root_ppn as u64) << 12) | ((vpn1 as u64) * 4),
        make_pte(l2_ppn, mmu::PTE_V),
    )
    .expect("operation should succeed");
    bus.write_word(
        ((l2_ppn as u64) << 12) | ((vpn0 as u64) * 4),
        make_pte(data_ppn, mmu::PTE_V | mmu::PTE_R | mmu::PTE_W | mmu::PTE_U),
    )
    .expect("operation should succeed");
    let satp = make_satp(1, 0, root_ppn);
    let result = mmu::translate(
        va,
        mmu::AccessType::Load,
        geometry_os::riscv::cpu::Privilege::User,
        false,
        false,
        satp,
        &mut bus,
        &mut tlb,
    );
    assert_eq!(
        result,
        mmu::TranslateResult::Ok(((data_ppn as u64) << 12) + 0x100)
    );
}

#[test]
fn test_sv32_page_fault_invalid_pte() {
    let mut tlb = mmu::Tlb::new();
    let mut bus = geometry_os::riscv::bus::Bus::new(0x0, 0x1_0000);
    bus.write_word(1u64 << 12, 0)
        .expect("operation should succeed");
    let satp = make_satp(1, 0, 1);
    let result = mmu::translate(
        0x0000_0000,
        mmu::AccessType::Load,
        geometry_os::riscv::cpu::Privilege::User,
        false,
        false,
        satp,
        &mut bus,
        &mut tlb,
    );
    assert_eq!(result, mmu::TranslateResult::LoadFault);
}

#[test]
fn test_sv32_page_fault_permission_denied() {
    let mut tlb = mmu::Tlb::new();
    let mut bus = geometry_os::riscv::bus::Bus::new(0x0, 0x1_0000);
    bus.write_word(1u64 << 12, make_pte(2, mmu::PTE_V))
        .expect("operation should succeed");
    bus.write_word(
        2u64 << 12,
        make_pte(3, mmu::PTE_V | mmu::PTE_R | mmu::PTE_W),
    )
    .expect("operation should succeed");
    let satp = make_satp(1, 0, 1);
    let result = mmu::translate(
        0x0000_0000,
        mmu::AccessType::Load,
        geometry_os::riscv::cpu::Privilege::User,
        false,
        false,
        satp,
        &mut bus,
        &mut tlb,
    );
    assert_eq!(result, mmu::TranslateResult::LoadFault);
}

#[test]
fn test_sv32_fault_types_by_access() {
    let mut bus = geometry_os::riscv::bus::Bus::new(0x0, 0x1_0000);
    bus.write_word(1u64 << 12, make_pte(2, mmu::PTE_V))
        .expect("operation should succeed");
    bus.write_word(2u64 << 12, make_pte(3, mmu::PTE_V | mmu::PTE_R))
        .expect("operation should succeed");
    let satp = make_satp(1, 0, 1);
    let mut t1 = mmu::Tlb::new();
    assert_eq!(
        mmu::translate(
            0,
            mmu::AccessType::Fetch,
            geometry_os::riscv::cpu::Privilege::Supervisor,
            false,
            false,
            satp,
            &mut bus,
            &mut t1
        ),
        mmu::TranslateResult::FetchFault
    );
    let mut t2 = mmu::Tlb::new();
    assert_eq!(
        mmu::translate(
            0,
            mmu::AccessType::Store,
            geometry_os::riscv::cpu::Privilege::Supervisor,
            false,
            false,
            satp,
            &mut bus,
            &mut t2
        ),
        mmu::TranslateResult::StoreFault
    );
    let mut t3 = mmu::Tlb::new();
    assert_eq!(
        mmu::translate(
            0,
            mmu::AccessType::Load,
            geometry_os::riscv::cpu::Privilege::Supervisor,
            false,
            false,
            satp,
            &mut bus,
            &mut t3
        ),
        mmu::TranslateResult::Ok(3u64 << 12)
    );
}

#[test]
fn test_sv32_megapage() {
    let mut tlb = mmu::Tlb::new();
    // Use a larger bus to fit the identity-mapped megapage region
    let mut bus = geometry_os::riscv::bus::Bus::new(0x0, 0x8_0000); // 512KB

    // SV32 megapage: L1 leaf PTE maps a 4MB region.
    // PA[31:22] = PTE.PPN[19:10], PA[21:12] = VA.VPN0, PA[11:0] = VA.offset
    // For identity mapping: use VPN1=0, so VA and PA are in low memory.
    // PTE.PPN[19:10] = 0, PTE.PPN[9:0] = 0
    let vpn1 = 0u32;
    let vpn0 = 4u32;
    let offset = 0x100u32;
    let va = (vpn1 << 22) | (vpn0 << 12) | offset; // 0x00004100
    let expected_pa = va; // identity mapping

    // Write test data at the expected PA
    bus.write_word(expected_pa as u64, 0xCAFE_0001)
        .expect("operation should succeed");

    // Write L1 PTE at root[vpn1=0]: megapage with PPN[19:10]=0
    // PPN = 0 (identity: PA[31:22] = 0)
    let megapage_ppn = 0u32;
    bus.write_word(
        (1u64) << 12, // root at page 1, entry index 0
        make_pte(
            megapage_ppn,
            mmu::PTE_V | mmu::PTE_R | mmu::PTE_W | mmu::PTE_X,
        ),
    )
    .expect("operation should succeed");

    let satp = make_satp(1, 0, 1); // mode=SV32, root PPN=1
    let result = mmu::translate(
        va,
        mmu::AccessType::Load,
        geometry_os::riscv::cpu::Privilege::Supervisor,
        false,
        false,
        satp,
        &mut bus,
        &mut tlb,
    );
    assert_eq!(result, mmu::TranslateResult::Ok(expected_pa as u64));
    if let mmu::TranslateResult::Ok(pa) = result {
        assert_eq!(
            bus.read_word(pa).expect("operation should succeed"),
            0xCAFE_0001
        );
    }

    // Also test with a non-zero VPN1 that maps to a different PA.
    // VPN1=2, PTE.PPN[19:10]=3 → VA 0x00800100 → PA 0x00C00100
    let vpn1_b = 2u32;
    let vpn0_b = 1u32;
    let va_b = (vpn1_b << 22) | (vpn0_b << 12) | 0x100; // 0x00801100
                                                        // PA = (3 << 22) | (1 << 12) | 0x100 = 0x00C01100
    let expected_pa_b = (3u64 << 22) | (1u64 << 12) | 0x100;
    // This PA is beyond bus size but we just test the translation math, not the read

    // Write L1 PTE at root[vpn1=2]
    let megapage_ppn_b = 3u32 << 10; // PPN[19:10]=3, PPN[9:0]=0
    bus.write_word(
        ((1u64) << 12) + (vpn1_b as u64) * 4,
        make_pte(
            megapage_ppn_b,
            mmu::PTE_V | mmu::PTE_R | mmu::PTE_W | mmu::PTE_X,
        ),
    )
    .expect("operation should succeed");

    let result_b = mmu::translate(
        va_b,
        mmu::AccessType::Load,
        geometry_os::riscv::cpu::Privilege::Supervisor,
        false,
        false,
        satp,
        &mut bus,
        &mut tlb,
    );
    assert_eq!(result_b, mmu::TranslateResult::Ok(expected_pa_b));
}

#[test]
fn test_sv32_tlb_caches() {
    let mut tlb = mmu::Tlb::new();
    let mut bus = geometry_os::riscv::bus::Bus::new(0x0, 0x1_0000);
    bus.write_word(1u64 << 12, make_pte(2, mmu::PTE_V))
        .expect("operation should succeed");
    bus.write_word(
        2u64 << 12,
        make_pte(
            3,
            mmu::PTE_V | mmu::PTE_R | mmu::PTE_W | mmu::PTE_X | mmu::PTE_U,
        ),
    )
    .expect("operation should succeed");
    let satp = make_satp(1, 0, 1);
    let r1 = mmu::translate(
        0,
        mmu::AccessType::Load,
        geometry_os::riscv::cpu::Privilege::User,
        false,
        false,
        satp,
        &mut bus,
        &mut tlb,
    );
    assert_eq!(r1, mmu::TranslateResult::Ok(3u64 << 12));
    bus.write_word(1u64 << 12, 0)
        .expect("operation should succeed");
    let r2 = mmu::translate(
        0,
        mmu::AccessType::Load,
        geometry_os::riscv::cpu::Privilege::User,
        false,
        false,
        satp,
        &mut bus,
        &mut tlb,
    );
    assert_eq!(r2, mmu::TranslateResult::Ok(3u64 << 12));
}

#[test]
fn test_sv32_tlb_flush_sfence() {
    let mut tlb = mmu::Tlb::new();
    // Use VPNs that don't hash to the same TLB slot.
    // Hash: (vpn + asid * 2654435761) % 64
    // vpn=0x10, asid=1 -> idx 43; vpn=0x20, asid=1 -> idx 59
    tlb.insert(0x10, 1, 0xAAA, mmu::PTE_V | mmu::PTE_R);
    tlb.insert(0x20, 1, 0xBBB, mmu::PTE_V | mmu::PTE_R);
    assert!(tlb.lookup(0x10, 1).is_some());
    assert!(tlb.lookup(0x20, 1).is_some());
    tlb.flush_all();
    assert!(tlb.lookup(0x10, 1).is_none());
    assert!(tlb.lookup(0x20, 1).is_none());
}

#[test]
fn test_sv32_tlb_asid_isolation() {
    let mut tlb = mmu::Tlb::new();
    tlb.insert(0x100, 1, 0xAAA, mmu::PTE_V | mmu::PTE_R);
    tlb.insert(0x100, 2, 0xBBB, mmu::PTE_V | mmu::PTE_R);
    assert_eq!(
        tlb.lookup(0x100, 1).expect("operation should succeed").0,
        0xAAA
    );
    assert_eq!(
        tlb.lookup(0x100, 2).expect("operation should succeed").0,
        0xBBB
    );
    assert!(tlb.lookup(0x100, 3).is_none());
}

#[test]
fn test_sv32_decode_sfence_vma() {
    assert_eq!(
        geometry_os::riscv::decode::decode(sfence_vma(0, 0)),
        geometry_os::riscv::decode::Operation::SfenceVma { rs1: 0, rs2: 0 }
    );
    assert_eq!(
        geometry_os::riscv::decode::decode(sfence_vma(5, 0)),
        geometry_os::riscv::decode::Operation::SfenceVma { rs1: 5, rs2: 0 }
    );
}

#[test]
fn test_sv32_sfence_flushes_cpu_tlb() {
    let mut vm = RiscvVm::new(0x1_0000);
    vm.cpu.tlb.insert(0x100, 0, 0xAAA, mmu::PTE_V | mmu::PTE_R);
    vm.cpu.tlb.insert(0x200, 0, 0xBBB, mmu::PTE_V | mmu::PTE_R);
    let base = 0x8000_0000u64;
    vm.bus
        .write_word(base, sfence_vma(0, 0))
        .expect("operation should succeed");
    vm.bus
        .write_word(base + 4, ebreak())
        .expect("operation should succeed");
    vm.cpu.pc = base as u32;
    vm.cpu.privilege = geometry_os::riscv::cpu::Privilege::Supervisor;
    vm.step();
    assert!(vm.cpu.tlb.lookup(0x100, 0).is_none());
    assert!(vm.cpu.tlb.lookup(0x200, 0).is_none());
}

#[test]
fn test_sv32_nonleaf_at_l2_is_fault() {
    let mut tlb = mmu::Tlb::new();
    let mut bus = geometry_os::riscv::bus::Bus::new(0x0, 0x1_0000);
    bus.write_word(1u64 << 12, make_pte(2, mmu::PTE_V))
        .expect("operation should succeed");
    bus.write_word(2u64 << 12, make_pte(3, mmu::PTE_V))
        .expect("operation should succeed");
    let satp = make_satp(1, 0, 1);
    let result = mmu::translate(
        0,
        mmu::AccessType::Load,
        geometry_os::riscv::cpu::Privilege::User,
        false,
        false,
        satp,
        &mut bus,
        &mut tlb,
    );
    assert_eq!(result, mmu::TranslateResult::LoadFault);
}

#[test]
fn test_sv32_tlb_global_entry() {
    let mut tlb = mmu::Tlb::new();
    tlb.insert(0x42, 5, 0x100, mmu::PTE_V | mmu::PTE_R | mmu::PTE_G);
    assert!(tlb.lookup(0x42, 0).is_some());
    assert!(tlb.lookup(0x42, 99).is_some());
    assert!(tlb.lookup(0x43, 5).is_none());
}

#[test]
fn test_sv32_satp_and_va_field_extraction() {
    let satp = make_satp(1, 42, 0x12345);
    assert!(mmu::satp_mode_enabled(satp));
    assert_eq!(mmu::satp_asid(satp), 42);
    assert_eq!(mmu::satp_ppn(satp), 0x12345);
    assert!(!mmu::satp_mode_enabled(0));
    assert_eq!(mmu::va_vpn1(0x0040_1100), 1);
    assert_eq!(mmu::va_vpn0(0x0040_1100), 1);
    assert_eq!(mmu::va_offset(0x0040_1100), 0x100);
    assert_eq!(mmu::va_to_vpn(0x0040_1100), 0x00401);
}

#[test]
fn test_sv32_cpu_load_through_page_table() {
    let mut bus = geometry_os::riscv::bus::Bus::new(0x0, 0x1_0000);
    let mut cpu = geometry_os::riscv::cpu::RiscvCpu::new();
    let root_ppn: u32 = 1;
    let l2_ppn: u32 = 2;
    let data_ppn: u32 = 3;
    bus.write_word((data_ppn as u64) << 12, 0xDEAD_BEEF)
        .expect("operation should succeed");
    bus.write_word((root_ppn as u64) << 12, make_pte(l2_ppn, mmu::PTE_V))
        .expect("operation should succeed");
    // L2[0] -> code page (page 0)
    bus.write_word(
        (l2_ppn as u64) << 12,
        make_pte(0, mmu::PTE_V | mmu::PTE_R | mmu::PTE_X),
    )
    .expect("operation should succeed");
    // L2[1] -> data page (page 3)
    bus.write_word(
        ((l2_ppn as u64) << 12) | 4,
        make_pte(data_ppn, mmu::PTE_V | mmu::PTE_R | mmu::PTE_W),
    )
    .expect("operation should succeed");
    // LUI x10, 0x1 -> x10 = 0x1000
    bus.write_word(0, (0x1u32 << 12) | (10u32 << 7) | 0x37)
        .expect("operation should succeed");
    // LW x5, 0(x10)
    bus.write_word(
        4,
        (0u32 << 20) | (10u32 << 15) | (0b010 << 12) | (5u32 << 7) | 0x03,
    )
    .expect("operation should succeed");
    // EBREAK
    bus.write_word(8, ebreak())
        .expect("operation should succeed");
    cpu.pc = 0;
    cpu.privilege = geometry_os::riscv::cpu::Privilege::Supervisor;
    cpu.csr.satp = make_satp(1, 0, root_ppn);
    for _ in 0..10 {
        match cpu.step(&mut bus) {
            StepResult::Ebreak => break,
            StepResult::Ok => {}
            o => panic!("Unexpected: {:?}", o),
        }
    }
    assert_eq!(cpu.x[5], 0xDEAD_BEEF);
}

#[test]
fn test_sv32_cpu_store_through_page_table() {
    let mut bus = geometry_os::riscv::bus::Bus::new(0x0, 0x1_0000);
    let mut cpu = geometry_os::riscv::cpu::RiscvCpu::new();
    let root_ppn: u32 = 1;
    let l2_ppn: u32 = 2;
    let data_ppn: u32 = 3;
    bus.write_word((root_ppn as u64) << 12, make_pte(l2_ppn, mmu::PTE_V))
        .expect("operation should succeed");
    bus.write_word(
        (l2_ppn as u64) << 12,
        make_pte(0, mmu::PTE_V | mmu::PTE_R | mmu::PTE_X),
    )
    .expect("operation should succeed");
    bus.write_word(
        ((l2_ppn as u64) << 12) | 4,
        make_pte(data_ppn, mmu::PTE_V | mmu::PTE_R | mmu::PTE_W),
    )
    .expect("operation should succeed");
    // ADDI x5, x0, 42
    bus.write_word(0, addi(5, 0, 42))
        .expect("operation should succeed");
    // LUI x10, 0x1
    bus.write_word(4, (0x1u32 << 12) | (10u32 << 7) | 0x37)
        .expect("operation should succeed");
    // SW x5, 0(x10)
    bus.write_word(
        8,
        (0u32 << 25) | (5u32 << 20) | (10u32 << 15) | (0b010 << 12) | (0u32 << 7) | 0x23,
    )
    .expect("operation should succeed");
    // LW x6, 0(x10)
    bus.write_word(
        12,
        (0u32 << 20) | (10u32 << 15) | (0b010 << 12) | (6u32 << 7) | 0x03,
    )
    .expect("operation should succeed");
    // EBREAK
    bus.write_word(16, ebreak())
        .expect("operation should succeed");
    cpu.pc = 0;
    cpu.privilege = geometry_os::riscv::cpu::Privilege::Supervisor;
    cpu.csr.satp = make_satp(1, 0, root_ppn);
    for _ in 0..10 {
        match cpu.step(&mut bus) {
            StepResult::Ebreak => break,
            StepResult::Ok => {}
            o => panic!("Unexpected: {:?}", o),
        }
    }
    assert_eq!(cpu.x[5], 42);
    assert_eq!(cpu.x[6], 42);
    assert_eq!(
        bus.read_word((data_ppn as u64) << 12)
            .expect("operation should succeed"),
        42
    );
}
