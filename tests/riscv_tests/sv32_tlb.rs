use super::sv32::sfence_vma;
use super::*;
use geometry_os::riscv::mmu;

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
    assert!(
        tlb.lookup(0x100, 1).is_none(),
        "non-global ASID 1 entry should be flushed"
    );
    assert!(
        tlb.lookup(0x200, 1).is_some(),
        "global entry should survive ASID flush"
    );
    assert!(
        tlb.lookup(0x200, 2).is_some(),
        "global entry should match any ASID"
    );
    assert!(
        tlb.lookup(0x300, 2).is_some(),
        "ASID 2 entry should be untouched"
    );
}

#[test]
fn test_tlb_flush_asid_preserves_others() {
    let mut tlb = mmu::Tlb::new();
    for asid in 1u16..=5 {
        tlb.insert(
            asid as u32 * 0x100,
            asid,
            0x1000 + asid as u32,
            mmu::PTE_V | mmu::PTE_R,
        );
    }
    assert_eq!(tlb.valid_count(), 5);
    tlb.flush_asid(3);
    assert_eq!(
        tlb.valid_count(),
        4,
        "only ASID 3 entries should be removed"
    );
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
    assert!(
        tlb.lookup(0x100, 1).is_none(),
        "VPN 0x100 ASID 1 should be flushed"
    );
    assert!(
        tlb.lookup(0x100, 2).is_some(),
        "VPN 0x100 ASID 2 should survive"
    );
    assert!(
        tlb.lookup(0x200, 1).is_some(),
        "VPN 0x200 ASID 1 should survive"
    );
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
    vm.cpu
        .tlb
        .insert(0x300, 1, 0xCCC, mmu::PTE_V | mmu::PTE_R | mmu::PTE_G);
    // ADDI x2, x0, 1  -- x2 = ASID 1
    vm.bus
        .write_word(base, addi(2, 0, 1))
        .expect("operation should succeed");
    // SFENCE.VMA x0, x2 -- flush ASID 1
    vm.bus
        .write_word(base + 4, sfence_vma(0, 2))
        .expect("operation should succeed");
    // EBREAK
    vm.bus
        .write_word(base + 8, ebreak())
        .expect("operation should succeed");
    vm.cpu.pc = base as u32;
    for _ in 0..5 {
        match vm.step() {
            StepResult::Ebreak => break,
            StepResult::Ok => {}
            o => panic!("Unexpected: {:?}", o),
        }
    }
    // ASID 1 non-global entry should be gone.
    assert!(
        vm.cpu.tlb.lookup(0x100, 1).is_none(),
        "ASID 1 non-global should be flushed"
    );
    // ASID 1 global entry should survive.
    assert!(
        vm.cpu.tlb.lookup(0x300, 1).is_some(),
        "ASID 1 global entry should survive"
    );
    // ASID 2 entry should be untouched.
    assert!(
        vm.cpu.tlb.lookup(0x200, 2).is_some(),
        "ASID 2 entry should be untouched"
    );
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
    vm.bus
        .write_word(base, lui(1, 0x100_000))
        .expect("operation should succeed");
    // ADDI x2, x0, 1 -- ASID 1
    vm.bus
        .write_word(base + 4, addi(2, 0, 1))
        .expect("operation should succeed");
    // SFENCE.VMA x1, x2
    vm.bus
        .write_word(base + 8, sfence_vma(1, 2))
        .expect("operation should succeed");
    // EBREAK
    vm.bus
        .write_word(base + 12, ebreak())
        .expect("operation should succeed");
    vm.cpu.pc = base as u32;
    for _ in 0..5 {
        match vm.step() {
            StepResult::Ebreak => break,
            StepResult::Ok => {}
            o => panic!("Unexpected: {:?}", o),
        }
    }
    // VPN 0x100 + ASID 1 should be flushed.
    assert!(
        vm.cpu.tlb.lookup(0x100, 1).is_none(),
        "VPN 0x100 ASID 1 should be flushed"
    );
    // VPN 0x100 + ASID 2 should survive (different ASID).
    assert!(
        vm.cpu.tlb.lookup(0x100, 2).is_some(),
        "VPN 0x100 ASID 2 should survive"
    );
    // VPN 0x200 + ASID 1 should survive (different VPN).
    assert!(
        vm.cpu.tlb.lookup(0x200, 1).is_some(),
        "VPN 0x200 ASID 1 should survive"
    );
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
    assert_eq!(
        tlb.lookup(0x100, 1).expect("operation should succeed").0,
        0x1000
    );
    // Looking up as ASID 2 gives PPN 0x2000
    assert_eq!(
        tlb.lookup(0x100, 2).expect("operation should succeed").0,
        0x2000
    );
    // Looking up as ASID 3 gives nothing
    assert!(tlb.lookup(0x100, 3).is_none());
}
