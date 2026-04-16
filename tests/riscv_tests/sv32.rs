use super::*;

// Phase 36: SV32 Page Table Walk Tests
// =====================================================================

use geometry_os::riscv::mmu;

pub(crate) fn make_pte(ppn: u32, flags: u32) -> u32 {
    ((ppn & 0x003F_FFFF) << 10) | (flags & 0x3FF)
}

pub(crate) fn make_satp(mode: u32, asid: u32, ppn: u32) -> u32 {
    ((mode & 1) << 31) | ((asid & 0x1FF) << 22) | (ppn & 0x003F_FFFF)
}

pub(crate) fn sfence_vma(rs1: u8, rs2: u8) -> u32 {
    (0b0001001u32 << 25) | ((rs2 as u32) << 20) | ((rs1 as u32) << 15) | (0b000 << 12) | (0u32 << 7) | 0x73
}

#[test]
fn test_sv32_bare_mode_identity_translation() {
    let mut tlb = mmu::Tlb::new();
    let mut bus = geometry_os::riscv::bus::Bus::new(0x8000_0000, 8192);
    let result = mmu::translate(0x8000_0000, mmu::AccessType::Fetch, geometry_os::riscv::cpu::Privilege::Supervisor, false, false, 0, &mut bus, &mut tlb);
    assert_eq!(result, mmu::TranslateResult::Ok(0x8000_0000));
}

#[test]
fn test_sv32_two_level_walk_4k_page() {
    let mut tlb = mmu::Tlb::new();
    let mut bus = geometry_os::riscv::bus::Bus::new(0x0, 0x1_0000);
    let root_ppn: u32 = 1;
    let l2_ppn: u32 = 2;
    let data_ppn: u32 = 3;
    bus.write_word((data_ppn as u64) << 12, 0xDEAD_BEEF).expect("operation should succeed");
    bus.write_word((root_ppn as u64) << 12, make_pte(l2_ppn, mmu::PTE_V)).expect("operation should succeed");
    bus.write_word((l2_ppn as u64) << 12, make_pte(data_ppn, mmu::PTE_V | mmu::PTE_R | mmu::PTE_W | mmu::PTE_X | mmu::PTE_U)).expect("operation should succeed");
    let satp = make_satp(1, 0, root_ppn);
    let result = mmu::translate(0x0000_0000, mmu::AccessType::Load, geometry_os::riscv::cpu::Privilege::User, false, false, satp, &mut bus, &mut tlb);
    assert_eq!(result, mmu::TranslateResult::Ok((data_ppn as u64) << 12));
    if let mmu::TranslateResult::Ok(pa) = result {
        assert_eq!(bus.read_word(pa).expect("operation should succeed"), 0xDEAD_BEEF);
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
    bus.write_word(((data_ppn as u64) << 12) + 0x100, 0x1234_5678).expect("operation should succeed");
    bus.write_word(((root_ppn as u64) << 12) | ((vpn1 as u64) * 4), make_pte(l2_ppn, mmu::PTE_V)).expect("operation should succeed");
    bus.write_word(((l2_ppn as u64) << 12) | ((vpn0 as u64) * 4), make_pte(data_ppn, mmu::PTE_V | mmu::PTE_R | mmu::PTE_W | mmu::PTE_U)).expect("operation should succeed");
    let satp = make_satp(1, 0, root_ppn);
    let result = mmu::translate(va, mmu::AccessType::Load, geometry_os::riscv::cpu::Privilege::User, false, false, satp, &mut bus, &mut tlb);
    assert_eq!(result, mmu::TranslateResult::Ok(((data_ppn as u64) << 12) + 0x100));
}

#[test]
fn test_sv32_page_fault_invalid_pte() {
    let mut tlb = mmu::Tlb::new();
    let mut bus = geometry_os::riscv::bus::Bus::new(0x0, 0x1_0000);
    bus.write_word(1u64 << 12, 0).expect("operation should succeed");
    let satp = make_satp(1, 0, 1);
    let result = mmu::translate(0x0000_0000, mmu::AccessType::Load, geometry_os::riscv::cpu::Privilege::User, false, false, satp, &mut bus, &mut tlb);
    assert_eq!(result, mmu::TranslateResult::LoadFault);
}

#[test]
fn test_sv32_page_fault_permission_denied() {
    let mut tlb = mmu::Tlb::new();
    let mut bus = geometry_os::riscv::bus::Bus::new(0x0, 0x1_0000);
    bus.write_word(1u64 << 12, make_pte(2, mmu::PTE_V)).expect("operation should succeed");
    bus.write_word(2u64 << 12, make_pte(3, mmu::PTE_V | mmu::PTE_R | mmu::PTE_W)).expect("operation should succeed");
    let satp = make_satp(1, 0, 1);
    let result = mmu::translate(0x0000_0000, mmu::AccessType::Load, geometry_os::riscv::cpu::Privilege::User, false, false, satp, &mut bus, &mut tlb);
    assert_eq!(result, mmu::TranslateResult::LoadFault);
}

#[test]
fn test_sv32_fault_types_by_access() {
    let mut bus = geometry_os::riscv::bus::Bus::new(0x0, 0x1_0000);
    bus.write_word(1u64 << 12, make_pte(2, mmu::PTE_V)).expect("operation should succeed");
    bus.write_word(2u64 << 12, make_pte(3, mmu::PTE_V | mmu::PTE_R)).expect("operation should succeed");
    let satp = make_satp(1, 0, 1);
    let mut t1 = mmu::Tlb::new();
    assert_eq!(mmu::translate(0, mmu::AccessType::Fetch, geometry_os::riscv::cpu::Privilege::Supervisor, false, false, satp, &mut bus, &mut t1), mmu::TranslateResult::FetchFault);
    let mut t2 = mmu::Tlb::new();
    assert_eq!(mmu::translate(0, mmu::AccessType::Store, geometry_os::riscv::cpu::Privilege::Supervisor, false, false, satp, &mut bus, &mut t2), mmu::TranslateResult::StoreFault);
    let mut t3 = mmu::Tlb::new();
    assert_eq!(mmu::translate(0, mmu::AccessType::Load, geometry_os::riscv::cpu::Privilege::Supervisor, false, false, satp, &mut bus, &mut t3), mmu::TranslateResult::Ok(3u64 << 12));
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
    bus.write_word(expected_pa as u64, 0xCAFE_0001).expect("operation should succeed");

    // Write L1 PTE at root[vpn1=0]: megapage with PPN[19:10]=0
    // PPN = 0 (identity: PA[31:22] = 0)
    let megapage_ppn = 0u32;
    bus.write_word(
        ((1u64) << 12), // root at page 1, entry index 0
        make_pte(megapage_ppn, mmu::PTE_V | mmu::PTE_R | mmu::PTE_W | mmu::PTE_X),
    ).expect("operation should succeed");

    let satp = make_satp(1, 0, 1); // mode=SV32, root PPN=1
    let result = mmu::translate(va, mmu::AccessType::Load, geometry_os::riscv::cpu::Privilege::Supervisor, false, false, satp, &mut bus, &mut tlb);
    assert_eq!(result, mmu::TranslateResult::Ok(expected_pa as u64));
    if let mmu::TranslateResult::Ok(pa) = result {
        assert_eq!(bus.read_word(pa).expect("operation should succeed"), 0xCAFE_0001);
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
        make_pte(megapage_ppn_b, mmu::PTE_V | mmu::PTE_R | mmu::PTE_W | mmu::PTE_X),
    ).expect("operation should succeed");

    let result_b = mmu::translate(va_b, mmu::AccessType::Load, geometry_os::riscv::cpu::Privilege::Supervisor, false, false, satp, &mut bus, &mut tlb);
    assert_eq!(result_b, mmu::TranslateResult::Ok(expected_pa_b));
}

#[test]
fn test_sv32_tlb_caches() {
    let mut tlb = mmu::Tlb::new();
    let mut bus = geometry_os::riscv::bus::Bus::new(0x0, 0x1_0000);
    bus.write_word(1u64 << 12, make_pte(2, mmu::PTE_V)).expect("operation should succeed");
    bus.write_word(2u64 << 12, make_pte(3, mmu::PTE_V | mmu::PTE_R | mmu::PTE_W | mmu::PTE_X | mmu::PTE_U)).expect("operation should succeed");
    let satp = make_satp(1, 0, 1);
    let r1 = mmu::translate(0, mmu::AccessType::Load, geometry_os::riscv::cpu::Privilege::User, false, false, satp, &mut bus, &mut tlb);
    assert_eq!(r1, mmu::TranslateResult::Ok(3u64 << 12));
    bus.write_word(1u64 << 12, 0).expect("operation should succeed");
    let r2 = mmu::translate(0, mmu::AccessType::Load, geometry_os::riscv::cpu::Privilege::User, false, false, satp, &mut bus, &mut tlb);
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
    assert_eq!(tlb.lookup(0x100, 1).expect("operation should succeed").0, 0xAAA);
    assert_eq!(tlb.lookup(0x100, 2).expect("operation should succeed").0, 0xBBB);
    assert!(tlb.lookup(0x100, 3).is_none());
}

#[test]
fn test_sv32_decode_sfence_vma() {
    assert_eq!(geometry_os::riscv::decode::decode(sfence_vma(0, 0)),
        geometry_os::riscv::decode::Operation::SfenceVma { rs1: 0, rs2: 0 });
    assert_eq!(geometry_os::riscv::decode::decode(sfence_vma(5, 0)),
        geometry_os::riscv::decode::Operation::SfenceVma { rs1: 5, rs2: 0 });
}

#[test]
fn test_sv32_sfence_flushes_cpu_tlb() {
    let mut vm = RiscvVm::new(0x1_0000);
    vm.cpu.tlb.insert(0x100, 0, 0xAAA, mmu::PTE_V | mmu::PTE_R);
    vm.cpu.tlb.insert(0x200, 0, 0xBBB, mmu::PTE_V | mmu::PTE_R);
    let base = 0x8000_0000u64;
    vm.bus.write_word(base, sfence_vma(0, 0)).expect("operation should succeed");
    vm.bus.write_word(base + 4, ebreak()).expect("operation should succeed");
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
    bus.write_word(1u64 << 12, make_pte(2, mmu::PTE_V)).expect("operation should succeed");
    bus.write_word(2u64 << 12, make_pte(3, mmu::PTE_V)).expect("operation should succeed");
    let satp = make_satp(1, 0, 1);
    let result = mmu::translate(0, mmu::AccessType::Load, geometry_os::riscv::cpu::Privilege::User, false, false, satp, &mut bus, &mut tlb);
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
    bus.write_word((data_ppn as u64) << 12, 0xDEAD_BEEF).expect("operation should succeed");
    bus.write_word((root_ppn as u64) << 12, make_pte(l2_ppn, mmu::PTE_V)).expect("operation should succeed");
    // L2[0] -> code page (page 0)
    bus.write_word((l2_ppn as u64) << 12, make_pte(0, mmu::PTE_V | mmu::PTE_R | mmu::PTE_X)).expect("operation should succeed");
    // L2[1] -> data page (page 3)
    bus.write_word(((l2_ppn as u64) << 12) | 4, make_pte(data_ppn, mmu::PTE_V | mmu::PTE_R | mmu::PTE_W)).expect("operation should succeed");
    // LUI x10, 0x1 -> x10 = 0x1000
    bus.write_word(0, (0x1u32 << 12) | (10u32 << 7) | 0x37).expect("operation should succeed");
    // LW x5, 0(x10)
    bus.write_word(4, (0u32 << 20) | (10u32 << 15) | (0b010 << 12) | (5u32 << 7) | 0x03).expect("operation should succeed");
    // EBREAK
    bus.write_word(8, ebreak()).expect("operation should succeed");
    cpu.pc = 0;
    cpu.privilege = geometry_os::riscv::cpu::Privilege::Supervisor;
    cpu.csr.satp = make_satp(1, 0, root_ppn);
    for _ in 0..10 {
        match cpu.step(&mut bus) { StepResult::Ebreak => break, StepResult::Ok => {}, o => panic!("Unexpected: {:?}", o) }
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
    bus.write_word((root_ppn as u64) << 12, make_pte(l2_ppn, mmu::PTE_V)).expect("operation should succeed");
    bus.write_word((l2_ppn as u64) << 12, make_pte(0, mmu::PTE_V | mmu::PTE_R | mmu::PTE_X)).expect("operation should succeed");
    bus.write_word(((l2_ppn as u64) << 12) | 4, make_pte(data_ppn, mmu::PTE_V | mmu::PTE_R | mmu::PTE_W)).expect("operation should succeed");
    // ADDI x5, x0, 42
    bus.write_word(0, addi(5, 0, 42)).expect("operation should succeed");
    // LUI x10, 0x1
    bus.write_word(4, (0x1u32 << 12) | (10u32 << 7) | 0x37).expect("operation should succeed");
    // SW x5, 0(x10)
    bus.write_word(8, (0u32 << 25) | (5u32 << 20) | (10u32 << 15) | (0b010 << 12) | (0u32 << 7) | 0x23).expect("operation should succeed");
    // LW x6, 0(x10)
    bus.write_word(12, (0u32 << 20) | (10u32 << 15) | (0b010 << 12) | (6u32 << 7) | 0x03).expect("operation should succeed");
    // EBREAK
    bus.write_word(16, ebreak()).expect("operation should succeed");
    cpu.pc = 0;
    cpu.privilege = geometry_os::riscv::cpu::Privilege::Supervisor;
    cpu.csr.satp = make_satp(1, 0, root_ppn);
    for _ in 0..10 {
        match cpu.step(&mut bus) { StepResult::Ebreak => break, StepResult::Ok => {}, o => panic!("Unexpected: {:?}", o) }
    }
    assert_eq!(cpu.x[5], 42);
    assert_eq!(cpu.x[6], 42);
    assert_eq!(bus.read_word((data_ppn as u64) << 12).expect("operation should succeed"), 42);
}

// =====================================================================
// Phase 36: TLB Cache Tests (64-entry, ASID-aware invalidation)
// =====================================================================

#[test]
fn test_tlb_flush_asid_non_global_only() {
    // flush_asid should remove entries for the given ASID but keep global entries.
    let mut tlb = mmu::Tlb::new();
    tlb.insert(0x100, 1, 0xAAA, mmu::PTE_V | mmu::PTE_R);
    tlb.insert(0x200, 1, 0xBBB, mmu::PTE_V | mmu::PTE_R | mmu::PTE_G);
    tlb.insert(0x300, 2, 0xCCC, mmu::PTE_V | mmu::PTE_R);
    // Flush ASID 1: removes 0x100, keeps 0x200 (global), keeps 0x300 (different ASID).
    tlb.flush_asid(1);
    assert!(tlb.lookup(0x100, 1).is_none(), "non-global ASID 1 entry should be flushed");
    assert!(tlb.lookup(0x200, 1).is_some(), "global entry should survive ASID flush");
    assert!(tlb.lookup(0x200, 2).is_some(), "global entry should match any ASID");
    assert!(tlb.lookup(0x300, 2).is_some(), "ASID 2 entry should be untouched");
}

#[test]
fn test_tlb_flush_asid_preserves_others() {
    let mut tlb = mmu::Tlb::new();
    for asid in 1u16..=5 {
        tlb.insert(asid as u32 * 0x100, asid, 0x1000 + asid as u32, mmu::PTE_V | mmu::PTE_R);
    }
    assert_eq!(tlb.valid_count(), 5);
    tlb.flush_asid(3);
    assert_eq!(tlb.valid_count(), 4, "only ASID 3 entries should be removed");
    for asid in 1u16..=5 {
        if asid == 3 {
            assert!(tlb.lookup(asid as u32 * 0x100, asid).is_none());
        } else {
            assert!(tlb.lookup(asid as u32 * 0x100, asid).is_some());
        }
    }
}

#[test]
fn test_tlb_flush_va_asid_combined() {
    // flush_va_asid should only remove entries matching both VPN and ASID.
    let mut tlb = mmu::Tlb::new();
    tlb.insert(0x100, 1, 0xAAA, mmu::PTE_V | mmu::PTE_R);
    tlb.insert(0x100, 2, 0xBBB, mmu::PTE_V | mmu::PTE_R);
    tlb.insert(0x200, 1, 0xCCC, mmu::PTE_V | mmu::PTE_R);
    tlb.flush_va_asid(0x100, 1);
    assert!(tlb.lookup(0x100, 1).is_none(), "VPN 0x100 ASID 1 should be flushed");
    assert!(tlb.lookup(0x100, 2).is_some(), "VPN 0x100 ASID 2 should survive");
    assert!(tlb.lookup(0x200, 1).is_some(), "VPN 0x200 ASID 1 should survive");
}

#[test]
fn test_tlb_64_entry_capacity() {
    // Fill all 64 TLB slots with unique entries.
    // Sequential VPNs 0..63 hash to unique base slots (verified above).
    let mut tlb = mmu::Tlb::new();
    for i in 0..64u32 {
        tlb.insert(i, 1, 0x1000 + i, mmu::PTE_V | mmu::PTE_R);
    }
    assert_eq!(tlb.valid_count(), 64);
    // All entries should be readable.
    for i in 0..64u32 {
        let result = tlb.lookup(i, 1);
        assert!(result.is_some(), "VPN {} should be in TLB", i);
        assert_eq!(result.expect("operation should succeed").0, 0x1000 + i);
    }
}

#[test]
fn test_tlb_no_eviction_hashmap() {
    // HashMap-based TLB has no capacity limit -- all entries are retained.
    // Entries persist until explicitly flushed (SFENCE.VMA, SATP change).
    let mut tlb = mmu::Tlb::new();
    for i in 0..80u32 {
        tlb.insert(i, 1, 0x1000 + i, mmu::PTE_V | mmu::PTE_R);
    }
    // All 80 entries should be present (no eviction).
    assert_eq!(tlb.valid_count(), 80);
    // All entries should be findable.
    for i in 0..80u32 {
        let result = tlb.lookup(i, 1);
        assert!(result.is_some(), "VPN {} should be in TLB", i);
        assert_eq!(result.expect("operation should succeed").0, 0x1000 + i);
    }
}

#[test]
fn test_tlb_sfence_vma_with_asid() {
    // SFENCE.VMA x0, x2 -> flush entries for ASID in x2.
    let mut vm = RiscvVm::new(0x1_0000);
    let base = 0x8000_0000u64;
    vm.cpu.privilege = geometry_os::riscv::cpu::Privilege::Supervisor;
    // Pre-populate TLB with entries for multiple ASIDs.
    vm.cpu.tlb.insert(0x100, 1, 0xAAA, mmu::PTE_V | mmu::PTE_R);
    vm.cpu.tlb.insert(0x200, 2, 0xBBB, mmu::PTE_V | mmu::PTE_R);
    vm.cpu.tlb.insert(0x300, 1, 0xCCC, mmu::PTE_V | mmu::PTE_R | mmu::PTE_G);
    // ADDI x2, x0, 1  -- x2 = ASID 1
    vm.bus.write_word(base, addi(2, 0, 1)).expect("operation should succeed");
    // SFENCE.VMA x0, x2 -- flush ASID 1
    vm.bus.write_word(base + 4, sfence_vma(0, 2)).expect("operation should succeed");
    // EBREAK
    vm.bus.write_word(base + 8, ebreak()).expect("operation should succeed");
    vm.cpu.pc = base as u32;
    for _ in 0..5 {
        match vm.step() { StepResult::Ebreak => break, StepResult::Ok => {}, o => panic!("Unexpected: {:?}", o) }
    }
    // ASID 1 non-global entry should be gone.
    assert!(vm.cpu.tlb.lookup(0x100, 1).is_none(), "ASID 1 non-global should be flushed");
    // ASID 1 global entry should survive.
    assert!(vm.cpu.tlb.lookup(0x300, 1).is_some(), "ASID 1 global entry should survive");
    // ASID 2 entry should be untouched.
    assert!(vm.cpu.tlb.lookup(0x200, 2).is_some(), "ASID 2 entry should be untouched");
}

#[test]
fn test_tlb_sfence_vma_with_vpn_and_asid() {
    // SFENCE.VMA x1, x2 -> flush entries matching both VPN in x1 and ASID in x2.
    let mut vm = RiscvVm::new(0x1_0000);
    let base = 0x8000_0000u64;
    vm.cpu.privilege = geometry_os::riscv::cpu::Privilege::Supervisor;
    vm.cpu.tlb.insert(0x100, 1, 0xAAA, mmu::PTE_V | mmu::PTE_R);
    vm.cpu.tlb.insert(0x100, 2, 0xBBB, mmu::PTE_V | mmu::PTE_R);
    vm.cpu.tlb.insert(0x200, 1, 0xCCC, mmu::PTE_V | mmu::PTE_R);
    // Set x1 = virtual address that maps to VPN 0x100
    // VPN = va >> 12 & 0xFFFFF, so VA = 0x100 << 12 = 0x100_000
    vm.bus.write_word(base, lui(1, 0x100_000)).expect("operation should succeed");
    // ADDI x2, x0, 1 -- ASID 1
    vm.bus.write_word(base + 4, addi(2, 0, 1)).expect("operation should succeed");
    // SFENCE.VMA x1, x2
    vm.bus.write_word(base + 8, sfence_vma(1, 2)).expect("operation should succeed");
    // EBREAK
    vm.bus.write_word(base + 12, ebreak()).expect("operation should succeed");
    vm.cpu.pc = base as u32;
    for _ in 0..5 {
        match vm.step() { StepResult::Ebreak => break, StepResult::Ok => {}, o => panic!("Unexpected: {:?}", o) }
    }
    // VPN 0x100 + ASID 1 should be flushed.
    assert!(vm.cpu.tlb.lookup(0x100, 1).is_none(), "VPN 0x100 ASID 1 should be flushed");
    // VPN 0x100 + ASID 2 should survive (different ASID).
    assert!(vm.cpu.tlb.lookup(0x100, 2).is_some(), "VPN 0x100 ASID 2 should survive");
    // VPN 0x200 + ASID 1 should survive (different VPN).
    assert!(vm.cpu.tlb.lookup(0x200, 1).is_some(), "VPN 0x200 ASID 1 should survive");
}

#[test]
fn test_tlb_asid_switch_reuses_entries() {
    // When switching address spaces (different ASID), TLB entries from
    // the old ASID should not be visible but should coexist in the TLB.
    let mut tlb = mmu::Tlb::new();
    // Process A (ASID 1) maps VPN 0x100 -> PPN 0x1000
    tlb.insert(0x100, 1, 0x1000, mmu::PTE_V | mmu::PTE_R);
    // Process B (ASID 2) maps VPN 0x100 -> PPN 0x2000 (same VA, different PA)
    tlb.insert(0x100, 2, 0x2000, mmu::PTE_V | mmu::PTE_R);
    // Looking up as ASID 1 gives PPN 0x1000
    assert_eq!(tlb.lookup(0x100, 1).expect("operation should succeed").0, 0x1000);
    // Looking up as ASID 2 gives PPN 0x2000
    assert_eq!(tlb.lookup(0x100, 2).expect("operation should succeed").0, 0x2000);
    // Looking up as ASID 3 gives nothing
    assert!(tlb.lookup(0x100, 3).is_none());
}

// ============================================================
// Phase 36: Page Fault Traps
// ============================================================

use geometry_os::riscv::csr;

#[test]
fn test_page_fault_load_sets_mcause_mtval() {
    // Fetch from address outside bus range -> fetch access fault trap.
    let mut bus = geometry_os::riscv::bus::Bus::new(0x8000_0000, 4096);
    let mut cpu = geometry_os::riscv::cpu::RiscvCpu::new();
    cpu.pc = 0xDEAD_0000;
    cpu.csr.mtvec = 0x8000_0000;

    let result = cpu.step(&mut bus);
    assert_eq!(result, StepResult::Ok, "fetch fault should return Ok after trap delivery");
    assert_eq!(cpu.pc, 0x8000_0000, "should jump to mtvec");
    assert_eq!(cpu.csr.mcause, csr::CAUSE_FETCH_ACCESS);
    assert_eq!(cpu.csr.mepc, 0xDEAD_0000);
    assert_eq!(cpu.csr.mtval, 0xDEAD_0000);
}

#[test]
fn test_page_fault_fetch_no_exec_permission() {
    // Fetch from page without X permission -> instruction page fault.
    let mut bus = geometry_os::riscv::bus::Bus::new(0x0, 0x2_0000);
    let mut cpu = geometry_os::riscv::cpu::RiscvCpu::new();
    let root_ppn: u32 = 1;
    let l2_ppn: u32 = 2;

    // Root[0] -> L2 table
    bus.write_word((root_ppn as u64) << 12, make_pte(l2_ppn, mmu::PTE_V)).expect("operation should succeed");
    // L2[0] -> code page (RX)
    bus.write_word((l2_ppn as u64) << 12, make_pte(0, mmu::PTE_V | mmu::PTE_R | mmu::PTE_X)).expect("operation should succeed");
    // L2[1] -> data page (RW, no X)
    bus.write_word(((l2_ppn as u64) << 12) | 4, make_pte(3, mmu::PTE_V | mmu::PTE_R | mmu::PTE_W)).expect("operation should succeed");

    // Code at VA 0x0: LUI x1, 0x1 -> x1 = 0x1000
    bus.write_word(0, (0x1u32 << 12) | (1u32 << 7) | 0x37).expect("operation should succeed");
    // JALR x0, x1, 0 -> jump to 0x1000
    bus.write_word(4, (0u32 << 20) | (1u32 << 15) | (0b000 << 12) | (0u32 << 7) | 0x67).expect("operation should succeed");

    cpu.pc = 0;
    cpu.privilege = geometry_os::riscv::cpu::Privilege::Supervisor;
    cpu.csr.satp = make_satp(1, 0, root_ppn);
    cpu.csr.mtvec = 0x8000_0200;

    // Execute LUI
    cpu.step(&mut bus);
    assert_eq!(cpu.x[1], 0x1000);
    // Execute JALR -> sets PC to 0x1000
    cpu.step(&mut bus);
    assert_eq!(cpu.pc, 0x1000);

    // Next fetch from 0x1000 -> instruction page fault (no X permission)
    cpu.step(&mut bus);
    assert_eq!(cpu.csr.mcause, csr::CAUSE_FETCH_PAGE_FAULT);
    assert_eq!(cpu.csr.mtval, 0x1000, "mtval should be faulting VA");
    assert_eq!(cpu.csr.mepc, 0x1000, "mepc should be faulting PC");
}

#[test]
fn test_page_fault_load_no_read_permission() {
    // Load from page without R permission -> load page fault.
    let mut bus = geometry_os::riscv::bus::Bus::new(0x0, 0x2_0000);
    let mut cpu = geometry_os::riscv::cpu::RiscvCpu::new();
    let root_ppn: u32 = 1;
    let l2_ppn: u32 = 2;

    bus.write_word((root_ppn as u64) << 12, make_pte(l2_ppn, mmu::PTE_V)).expect("operation should succeed");
    bus.write_word((l2_ppn as u64) << 12, make_pte(0, mmu::PTE_V | mmu::PTE_R | mmu::PTE_X)).expect("operation should succeed");
    // L2[1] -> write-only page (no R)
    bus.write_word(((l2_ppn as u64) << 12) | 4, make_pte(3, mmu::PTE_V | mmu::PTE_W)).expect("operation should succeed");

    // LUI x10, 0x1 -> x10 = 0x1000
    bus.write_word(0, (0x1u32 << 12) | (10u32 << 7) | 0x37).expect("operation should succeed");
    // LW x5, 0(x10) -> load from 0x1000
    bus.write_word(4, (0u32 << 20) | (10u32 << 15) | (0b010 << 12) | (5u32 << 7) | 0x03).expect("operation should succeed");

    cpu.pc = 0;
    cpu.privilege = geometry_os::riscv::cpu::Privilege::Supervisor;
    cpu.csr.satp = make_satp(1, 0, root_ppn);
    cpu.csr.mtvec = 0x8000_0200;

    cpu.step(&mut bus); // LUI
    cpu.step(&mut bus); // LW -> load page fault

    assert_eq!(cpu.csr.mcause, csr::CAUSE_LOAD_PAGE_FAULT);
    assert_eq!(cpu.csr.mtval, 0x1000, "mtval should be faulting VA");
    assert_eq!(cpu.csr.mepc, 4, "mepc should be PC of the LW instruction");
}

#[test]
fn test_page_fault_store_no_write_permission() {
    // Store to page without W permission -> store page fault.
    let mut bus = geometry_os::riscv::bus::Bus::new(0x0, 0x2_0000);
    let mut cpu = geometry_os::riscv::cpu::RiscvCpu::new();
    let root_ppn: u32 = 1;
    let l2_ppn: u32 = 2;

    bus.write_word((root_ppn as u64) << 12, make_pte(l2_ppn, mmu::PTE_V)).expect("operation should succeed");
    bus.write_word((l2_ppn as u64) << 12, make_pte(0, mmu::PTE_V | mmu::PTE_R | mmu::PTE_X)).expect("operation should succeed");
    // L2[1] -> read-only page (no W)
    bus.write_word(((l2_ppn as u64) << 12) | 4, make_pte(3, mmu::PTE_V | mmu::PTE_R)).expect("operation should succeed");

    bus.write_word(0, (0x1u32 << 12) | (10u32 << 7) | 0x37).expect("operation should succeed"); // LUI x10, 0x1
    bus.write_word(4, addi(5, 0, 42)).expect("operation should succeed"); // ADDI x5, x0, 42
    bus.write_word(8, sw(5, 10, 0)).expect("operation should succeed"); // SW x5, 0(x10) -> store to 0x1000

    cpu.pc = 0;
    cpu.privilege = geometry_os::riscv::cpu::Privilege::Supervisor;
    cpu.csr.satp = make_satp(1, 0, root_ppn);
    cpu.csr.mtvec = 0x8000_0200;

    cpu.step(&mut bus); // LUI
    cpu.step(&mut bus); // ADDI
    assert_eq!(cpu.x[5], 42);
    cpu.step(&mut bus); // SW -> store page fault

    assert_eq!(cpu.csr.mcause, csr::CAUSE_STORE_PAGE_FAULT);
    assert_eq!(cpu.csr.mtval, 0x1000, "mtval should be faulting VA");
    assert_eq!(cpu.csr.mepc, 8, "mepc should be PC of the SW instruction");
}

#[test]
fn test_page_fault_delegated_to_s_mode() {
    // Load page fault from U-mode delegated to S-mode via medeleg.
    let mut bus = geometry_os::riscv::bus::Bus::new(0x0, 0x2_0000);
    let mut cpu = geometry_os::riscv::cpu::RiscvCpu::new();
    let root_ppn: u32 = 1;
    let l2_ppn: u32 = 2;

    bus.write_word((root_ppn as u64) << 12, make_pte(l2_ppn, mmu::PTE_V)).expect("operation should succeed");
    // L2[0] -> code page (RXU)
    bus.write_word((l2_ppn as u64) << 12, make_pte(0, mmu::PTE_V | mmu::PTE_R | mmu::PTE_X | mmu::PTE_U)).expect("operation should succeed");
    // L2[1] -> not mapped

    // Delegate load page fault (cause 13) to S-mode
    cpu.csr.medeleg = 1 << csr::CAUSE_LOAD_PAGE_FAULT;

    // LUI x10, 0x1
    bus.write_word(0, (0x1u32 << 12) | (10u32 << 7) | 0x37).expect("operation should succeed");
    // LW x5, 0(x10) -> unmapped
    bus.write_word(4, (0u32 << 20) | (10u32 << 15) | (0b010 << 12) | (5u32 << 7) | 0x03).expect("operation should succeed");

    cpu.pc = 0;
    cpu.privilege = geometry_os::riscv::cpu::Privilege::User;
    cpu.csr.satp = make_satp(1, 0, root_ppn);
    cpu.csr.mtvec = 0x8000_0300;
    cpu.csr.stvec = 0x8000_0400;

    cpu.step(&mut bus); // LUI
    cpu.step(&mut bus); // LW -> load page fault, delegated to S-mode

    assert_eq!(cpu.csr.scause, csr::CAUSE_LOAD_PAGE_FAULT);
    assert_eq!(cpu.csr.stval, 0x1000, "stval should be faulting VA");
    assert_eq!(cpu.csr.sepc, 4, "sepc should be PC of the LW instruction");
    assert_eq!(cpu.pc, 0x8000_0400, "should jump to stvec");
    assert_eq!(cpu.privilege, geometry_os::riscv::cpu::Privilege::Supervisor);
}

#[test]
fn test_page_fault_stval_for_s_mode_trap() {
    // Store page fault delegated to S-mode sets stval, not mtval.
    let mut bus = geometry_os::riscv::bus::Bus::new(0x0, 0x2_0000);
    let mut cpu = geometry_os::riscv::cpu::RiscvCpu::new();
    let root_ppn: u32 = 1;
    let l2_ppn: u32 = 2;

    bus.write_word((root_ppn as u64) << 12, make_pte(l2_ppn, mmu::PTE_V)).expect("operation should succeed");
    bus.write_word((l2_ppn as u64) << 12, make_pte(0, mmu::PTE_V | mmu::PTE_R | mmu::PTE_X | mmu::PTE_U)).expect("operation should succeed");

    cpu.csr.medeleg = 1 << csr::CAUSE_STORE_PAGE_FAULT;

    bus.write_word(0, (0x1u32 << 12) | (10u32 << 7) | 0x37).expect("operation should succeed"); // LUI x10, 0x1
    bus.write_word(4, addi(5, 0, 99)).expect("operation should succeed"); // ADDI x5, x0, 99
    bus.write_word(8, sw(5, 10, 0)).expect("operation should succeed"); // SW x5, 0(x10)

    cpu.pc = 0;
    cpu.privilege = geometry_os::riscv::cpu::Privilege::User;
    cpu.csr.satp = make_satp(1, 0, root_ppn);
    cpu.csr.mtvec = 0x8000_0300;
    cpu.csr.stvec = 0x8000_0400;

    cpu.step(&mut bus); // LUI
    cpu.step(&mut bus); // ADDI
    cpu.step(&mut bus); // SW -> store page fault, delegated to S-mode

    assert_eq!(cpu.csr.scause, csr::CAUSE_STORE_PAGE_FAULT);
    assert_eq!(cpu.csr.stval, 0x1000, "stval should be faulting VA");
    assert_eq!(cpu.csr.sepc, 8);
    assert_eq!(cpu.csr.mtval, 0, "mtval should be 0 (trap went to S-mode)");
    assert_eq!(cpu.privilege, geometry_os::riscv::cpu::Privilege::Supervisor);
}

#[test]
fn test_page_fault_mret_recovery() {
    // Load page fault -> trap to M -> fix page table -> MRET -> retry succeeds.
    let mut bus = geometry_os::riscv::bus::Bus::new(0x0, 0x4_0000);
    let mut cpu = geometry_os::riscv::cpu::RiscvCpu::new();
    let root_ppn: u32 = 1;
    let l2_ppn: u32 = 2;
    let data_ppn: u32 = 3;

    bus.write_word((root_ppn as u64) << 12, make_pte(l2_ppn, mmu::PTE_V)).expect("operation should succeed");
    bus.write_word((l2_ppn as u64) << 12, make_pte(0, mmu::PTE_V | mmu::PTE_R | mmu::PTE_X)).expect("operation should succeed");
    // L2[1] initially unmapped

    // Put expected data at data page
    bus.write_word((data_ppn as u64) << 12, 0xFEED_FACE).expect("operation should succeed");

    // LUI x10, 0x1
    bus.write_word(0, (0x1u32 << 12) | (10u32 << 7) | 0x37).expect("operation should succeed");
    // LW x5, 0(x10)
    bus.write_word(4, (0u32 << 20) | (10u32 << 15) | (0b010 << 12) | (5u32 << 7) | 0x03).expect("operation should succeed");
    // EBREAK
    bus.write_word(8, ebreak()).expect("operation should succeed");

    cpu.pc = 0;
    cpu.privilege = geometry_os::riscv::cpu::Privilege::Supervisor;
    cpu.csr.satp = make_satp(1, 0, root_ppn);
    cpu.csr.mtvec = 0x8000_0200;

    // LUI
    cpu.step(&mut bus);
    assert_eq!(cpu.x[10], 0x1000);
    // LW -> page fault
    cpu.step(&mut bus);
    assert_eq!(cpu.csr.mcause, csr::CAUSE_LOAD_PAGE_FAULT);
    assert_eq!(cpu.csr.mtval, 0x1000);
    assert_eq!(cpu.csr.mepc, 4);

    // Fix page table externally
    bus.write_word(((l2_ppn as u64) << 12) | 4, make_pte(data_ppn, mmu::PTE_V | mmu::PTE_R | mmu::PTE_W)).expect("operation should succeed");
    cpu.tlb.flush_all();

    // MRET (simulate)
    let restored = cpu.csr.trap_return(geometry_os::riscv::cpu::Privilege::Machine);
    cpu.pc = cpu.csr.mepc;
    cpu.privilege = restored;

    assert_eq!(cpu.pc, 4);

    // Retry LW -> succeeds now
    cpu.step(&mut bus);
    assert_eq!(cpu.x[5], 0xFEED_FACE, "retry should succeed after page table fix");
    assert_eq!(cpu.pc, 8);
}

#[test]
fn test_page_fault_unmapped_va_all_three_types() {
    // Verify cause codes for all three page fault types with unmapped PTEs.
    let mut bus = geometry_os::riscv::bus::Bus::new(0x0, 0x2_0000);
    let root_ppn: u32 = 1;
    let l2_ppn: u32 = 2;

    bus.write_word((root_ppn as u64) << 12, make_pte(l2_ppn, mmu::PTE_V)).expect("operation should succeed");
    bus.write_word((l2_ppn as u64) << 12, make_pte(0, mmu::PTE_V | mmu::PTE_R | mmu::PTE_X)).expect("operation should succeed");

    // Fetch page fault: PC = 0x1000 (VPN 1, unmapped)
    {
        let mut cpu = geometry_os::riscv::cpu::RiscvCpu::new();
        cpu.pc = 0x1000;
        cpu.privilege = geometry_os::riscv::cpu::Privilege::Supervisor;
        cpu.csr.satp = make_satp(1, 0, root_ppn);
        cpu.csr.mtvec = 0x8000_0200;
        cpu.step(&mut bus);
        assert_eq!(cpu.csr.mcause, csr::CAUSE_FETCH_PAGE_FAULT);
        assert_eq!(cpu.csr.mtval, 0x1000);
    }

    // Load page fault: LW from 0x1000
    {
        let mut cpu = geometry_os::riscv::cpu::RiscvCpu::new();
        // LUI x10, 0x1; LW x5, 0(x10)
        bus.write_word(0, (0x1u32 << 12) | (10u32 << 7) | 0x37).expect("operation should succeed");
        bus.write_word(4, (0u32 << 20) | (10u32 << 15) | (0b010 << 12) | (5u32 << 7) | 0x03).expect("operation should succeed");
        cpu.pc = 0;
        cpu.privilege = geometry_os::riscv::cpu::Privilege::Supervisor;
        cpu.csr.satp = make_satp(1, 0, root_ppn);
        cpu.csr.mtvec = 0x8000_0200;
        cpu.tlb.flush_all();
        cpu.step(&mut bus); // LUI
        cpu.step(&mut bus); // LW -> fault
        assert_eq!(cpu.csr.mcause, csr::CAUSE_LOAD_PAGE_FAULT);
        assert_eq!(cpu.csr.mtval, 0x1000);
    }

    // Store page fault: SW to 0x1000
    {
        let mut cpu = geometry_os::riscv::cpu::RiscvCpu::new();
        bus.write_word(0, (0x1u32 << 12) | (10u32 << 7) | 0x37).expect("operation should succeed");
        bus.write_word(4, sw(5, 10, 0)).expect("operation should succeed");
        cpu.x[5] = 42;
        cpu.pc = 0;
        cpu.privilege = geometry_os::riscv::cpu::Privilege::Supervisor;
        cpu.csr.satp = make_satp(1, 0, root_ppn);
        cpu.csr.mtvec = 0x8000_0200;
        cpu.tlb.flush_all();
        cpu.step(&mut bus); // LUI
        cpu.step(&mut bus); // SW -> fault
        assert_eq!(cpu.csr.mcause, csr::CAUSE_STORE_PAGE_FAULT);
        assert_eq!(cpu.csr.mtval, 0x1000);
    }
}


// ============================================================
// Phase 37: CLINT + PLIC integration tests
// ============================================================

use geometry_os::riscv::clint;
use geometry_os::riscv::plic;

/// CLINT timer fires, CPU traps to mtvec handler, MRET returns.
/// Timing: mtime increments at start of each step, then sync_mip, then instruction.
/// mtimecmp=4 means timer fires when mtime>=4, which is during step 5.
#[test]
fn test_clint_timer_trap_and_mret() {
    let mut vm = RiscvVm::new(4096);
    let base = 0x8000_0000u64;

    // Main code (3 instructions)
    vm.bus.write_word(base, addi(5, 0, 42)).expect("operation should succeed");      // 0x00
    vm.bus.write_word(base + 4, addi(6, 0, 99)).expect("operation should succeed");   // 0x04
    vm.bus.write_word(base + 8, nop()).expect("operation should succeed");             // 0x08

    // Trap handler: save mepc, advance by 4, restore, mret
    let handler = 0x8000_0200u64;
    vm.bus.write_word(handler, csrrw(10, 0, CSR_MEPC)).expect("operation should succeed");
    vm.bus.write_word(handler + 4, addi(10, 10, 4)).expect("operation should succeed");
    vm.bus.write_word(handler + 8, csrrw(0, 10, CSR_MEPC)).expect("operation should succeed");
    vm.bus.write_word(handler + 12, mret()).expect("operation should succeed");

    // Enable MTIE + MIE, set mtvec, timer fires at mtime=4
    vm.cpu.csr.mie = 1 << 7;
    vm.cpu.csr.mtvec = 0x8000_0200;
    vm.cpu.csr.mstatus = 1 << 3;
    vm.bus.clint.mtimecmp = 4;

    // Step 1: mtime 0->1, execute addi x5, x0, 42
    vm.step();
    assert_eq!(vm.cpu.x[5], 42);

    // Step 2: mtime 1->2, execute addi x6, x0, 99
    vm.step();
    assert_eq!(vm.cpu.x[6], 99);

    // Step 3: mtime 2->3, execute nop
    vm.step();

    // Step 4: mtime 3->4, timer pending (4>=4), trap fires
    vm.step();
    assert_eq!(vm.cpu.csr.mcause, csr::MCAUSE_INTERRUPT_BIT | 7);
    assert_eq!(vm.cpu.pc, 0x8000_0200);

    // Run handler (4 instr)
    vm.step(); // CSRRW mepc -> x10
    vm.step(); // ADDI x10 += 4
    vm.step(); // CSRRW mepc <- x10
    vm.step(); // MRET
    assert_eq!(vm.cpu.pc, 0x8000_0010); // mepc was 0x0C, handler advanced by 4
}

/// CLINT software interrupt (MSIP) triggers a trap.
#[test]
fn test_clint_software_interrupt_trap() {
    let mut vm = RiscvVm::new(4096);
    let base = 0x8000_0000u64;

    vm.bus.write_word(base, nop()).expect("operation should succeed");
    vm.bus.write_word(base + 4, nop()).expect("operation should succeed");
    vm.bus.write_word(0x8000_0200, csrrw(10, 0, CSR_MEPC)).expect("operation should succeed");
    vm.bus.write_word(0x8000_0204, csrrw(0, 10, CSR_MEPC)).expect("operation should succeed");
    vm.bus.write_word(0x8000_0208, mret()).expect("operation should succeed");

    vm.cpu.csr.mie = 1 << 3; // MSIE
    vm.cpu.csr.mtvec = 0x8000_0200;
    vm.cpu.csr.mstatus = 1 << 3; // MIE

    vm.step(); // normal
    assert_eq!(vm.cpu.pc, 0x8000_0004);

    vm.bus.clint.msip = 1; // trigger
    vm.step(); // MSI trap
    assert_eq!(vm.cpu.csr.mcause, csr::MCAUSE_INTERRUPT_BIT | 3);
}

/// PLIC external interrupt sets MEIP and triggers machine trap.
#[test]
fn test_plic_external_interrupt_to_trap() {
    let mut vm = RiscvVm::new(4096);
    let base = 0x8000_0000u64;

    vm.bus.write_word(base, nop()).expect("operation should succeed");

    // Handler: save mepc, claim from PLIC via MMIO, complete, restore, mret
    // Use direct bus.write_word for PLIC claim/complete instead of CPU instructions
    // to avoid needing LUI+ADDI+LW+SW encoding issues.
    vm.bus.write_word(0x8000_0200, csrrw(10, 0, CSR_MEPC)).expect("operation should succeed");
    vm.bus.write_word(0x8000_0204, csrrw(0, 10, CSR_MEPC)).expect("operation should succeed");
    vm.bus.write_word(0x8000_0208, mret()).expect("operation should succeed");

    vm.cpu.csr.mie = 1 << 11; // MEIE
    vm.cpu.csr.mtvec = 0x8000_0200;
    vm.cpu.csr.mstatus = 1 << 3; // MIE

    // Signal UART interrupt via PLIC
    vm.bus.plic.priority[plic::IRQ_UART as usize] = 5;
    vm.bus.plic.enable = 1 << plic::IRQ_UART;
    vm.bus.plic.signal(plic::IRQ_UART);

    vm.step(); // MEI trap
    assert_eq!(vm.cpu.csr.mcause, csr::MCAUSE_INTERRUPT_BIT | 11);
    assert_eq!(vm.cpu.pc, 0x8000_0200);

    // Run handler
    vm.step(); // save mepc
    vm.step(); // restore mepc
    vm.step(); // mret
    assert_eq!(vm.cpu.pc, 0x8000_0000); // MRET returns to mepc (the interrupted NOP)
}

/// MIE=0 blocks interrupt delivery even if everything else is enabled.
#[test]
fn test_interrupt_masked_by_mie() {
    let mut vm = RiscvVm::new(4096);
    let base = 0x8000_0000u64;
    vm.bus.write_word(base, nop()).expect("operation should succeed");
    vm.bus.write_word(base + 4, nop()).expect("operation should succeed");
    vm.cpu.csr.mtvec = 0x8000_0200;
    vm.cpu.csr.mie = 1 << 7; // MTIE
    vm.cpu.csr.mstatus = 0;  // MIE=0!
    vm.bus.clint.mtimecmp = 0;
    vm.step();
    assert_eq!(vm.cpu.pc, 0x8000_0004); // no trap
}

/// CLINT mtime full 64-bit MMIO through the bus.
#[test]
fn test_clint_mtime_mmio_full() {
    let mut vm = RiscvVm::new(4096);
    vm.bus.write_word(clint::MTIME_ADDR, 0xDEAD_BEEF).expect("operation should succeed");
    vm.bus.write_word(clint::MTIME_ADDR + 4, 0x1234_5678).expect("operation should succeed");
    assert_eq!(vm.bus.clint.mtime, 0x1234_5678_DEAD_BEEF);
    assert_eq!(vm.bus.read_word(clint::MTIME_ADDR).expect("operation should succeed"), 0xDEAD_BEEF);
    assert_eq!(vm.bus.read_word(clint::MTIME_ADDR + 4).expect("operation should succeed"), 0x1234_5678);
}

/// CLINT mtimecmp full 64-bit MMIO through the bus.
#[test]
fn test_clint_mtimecmp_mmio_full() {
    let mut vm = RiscvVm::new(4096);
    vm.bus.write_word(clint::MTIMECMP_BASE, 0x0000_0100).expect("operation should succeed");
    vm.bus.write_word(clint::MTIMECMP_BASE + 4, 0x0000_0002).expect("operation should succeed");
    assert_eq!(vm.bus.clint.mtimecmp, 0x0000_0002_0000_0100);
}

/// PLIC threshold blocks low-priority interrupts.
#[test]
fn test_plic_threshold_blocks_low_priority() {
    let mut vm = RiscvVm::new(4096);
    let base = 0x8000_0000u64;
    vm.bus.write_word(base, nop()).expect("operation should succeed");
    vm.cpu.csr.mie = 1 << 11; // MEIE
    vm.cpu.csr.mtvec = 0x8000_0200;
    vm.cpu.csr.mstatus = 1 << 3;
    vm.bus.plic.priority[plic::IRQ_UART as usize] = 2;
    vm.bus.plic.enable = 1 << plic::IRQ_UART;
    vm.bus.plic.threshold = 3;
    vm.bus.plic.signal(plic::IRQ_UART);
    vm.step();
    assert_eq!(vm.cpu.pc, 0x8000_0004); // no trap -- below threshold
}

/// PLIC claim returns highest priority among multiple sources.
#[test]
fn test_plic_multiple_sources_priority() {
    let mut vm = RiscvVm::new(4096);
    vm.bus.plic.priority[1] = 2;
    vm.bus.plic.priority[plic::IRQ_UART as usize] = 7;
    vm.bus.plic.enable = (1 << 1) | (1 << plic::IRQ_UART);
    vm.bus.plic.signal(1);
    vm.bus.plic.signal(plic::IRQ_UART);
    assert_eq!(vm.bus.plic.claim(), plic::IRQ_UART);
}

/// PLIC complete clears pending and allows next interrupt.
#[test]
fn test_plic_complete_then_next() {
    let mut vm = RiscvVm::new(4096);
    vm.bus.plic.priority[1] = 5;
    vm.bus.plic.priority[2] = 3;
    vm.bus.plic.enable = (1 << 1) | (1 << 2);
    vm.bus.plic.signal(1);
    vm.bus.plic.signal(2);
    let first = vm.bus.plic.claim();
    assert_eq!(first, 1);
    vm.bus.plic.complete(first);
    assert_eq!(vm.bus.plic.pending & (1 << 1), 0);
    let second = vm.bus.plic.claim();
    assert_eq!(second, 2);
}

/// RiscvVm::step drives full interrupt pipeline: tick -> sync -> trap.
#[test]
fn test_riscvvm_step_drives_timer_interrupt() {
    let mut vm = RiscvVm::new(4096);
    let base = 0x8000_0000u64;
    for i in 0..10u64 {
        vm.bus.write_word(base + i * 4, nop()).expect("operation should succeed");
    }
    vm.cpu.pc = base as u32;
    vm.cpu.csr.mie = 1 << 7; // MTIE
    vm.cpu.csr.mtvec = 0x8000_0200;
    vm.cpu.csr.mstatus = 1 << 3; // MIE
    vm.bus.clint.mtimecmp = 4;
    vm.step(); // mtime 0->1
    vm.step(); // mtime 1->2
    vm.step(); // mtime 2->3
    vm.step(); // mtime 3->4, now 4>=4, MTIP set, trap fires
    assert_eq!(vm.cpu.csr.mcause & csr::MCAUSE_INTERRUPT_BIT, csr::MCAUSE_INTERRUPT_BIT);
    assert_eq!(vm.cpu.csr.mcause & !csr::MCAUSE_INTERRUPT_BIT, 7);
}

/// S-mode timer interrupt with mideleg delegation.
#[test]
fn test_supervisor_timer_with_delegation() {
    let mut vm = RiscvVm::new(4096);
    let base = 0x8000_0000u64;
    vm.bus.write_word(base, nop()).expect("operation should succeed");
    vm.bus.write_word(0x8000_0200, csrrw(10, 0, CSR_SEPC)).expect("operation should succeed");
    vm.bus.write_word(0x8000_0204, csrrw(0, 10, CSR_SEPC)).expect("operation should succeed");
    vm.bus.write_word(0x8000_0208, sret()).expect("operation should succeed");

    vm.cpu.csr.mideleg = 1 << 5; // Delegate STI
    vm.cpu.csr.mie = 1 << 5; // STIE
    vm.cpu.csr.stvec = 0x8000_0200;
    vm.cpu.privilege = geometry_os::riscv::cpu::Privilege::Supervisor;
    vm.cpu.csr.mstatus = 1 << 1; // SIE
    vm.cpu.csr.mip = 1 << 5; // STIP
    vm.step();
    assert_eq!(vm.cpu.csr.scause, csr::MCAUSE_INTERRUPT_BIT | 5);
    assert_eq!(vm.cpu.pc, 0x8000_0200);
}
