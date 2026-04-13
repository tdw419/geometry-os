// riscv/mmu.rs -- SV32 page table walk + TLB (Phase 36)
//
// Implements RISC-V Sv32 virtual memory translation:
//   - 2-level page table walk: VPN[1] -> PT1 -> VPN[0] -> PT2 -> PPN + offset
//   - PTE flags: V, R, W, X, U, G, A, D
//   - 64-entry TLB with ASID-aware invalidation
//   - Page fault generation (load, store, instruction)
//   - SFENCE.VMA support (TLB flush)

use super::bus::Bus;
use super::csr;

// ---- PTE flag bits ----
pub const PTE_V: u32 = 1 << 0;
pub const PTE_R: u32 = 1 << 1;
pub const PTE_W: u32 = 1 << 2;
pub const PTE_X: u32 = 1 << 3;
pub const PTE_U: u32 = 1 << 4;
pub const PTE_G: u32 = 1 << 5;
pub const PTE_A: u32 = 1 << 6;
pub const PTE_D: u32 = 1 << 7;

// ---- satp field extraction ----
const SATP_MODE_BIT: u32 = 31;
const SATP_ASID_LSB: u32 = 22;
const SATP_ASID_BITS: u32 = 9;
const SATP_PPN_MASK: u32 = 0x003F_FFFF;

// ---- Sv32 virtual address fields ----
const VA_VPN1_LSB: u32 = 22;
const VA_VPN0_LSB: u32 = 12;
const VA_OFFSET_MASK: u32 = 0xFFF;
const VPN_MASK: u32 = 0x3FF;

// ---- Page constants ----
pub const PAGE_SIZE: usize = 4096;
const PAGE_SHIFT: u32 = 12;

// ---- TLB constants ----
const TLB_SIZE: usize = 64;

/// Access type for permission checks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccessType {
    Fetch,
    Load,
    Store,
}

/// Result of address translation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TranslateResult {
    Ok(u64),
    FetchFault,
    LoadFault,
    StoreFault,
}

/// A single TLB entry.
#[derive(Clone, Copy, Debug)]
struct TlbEntry {
    vpn: u32,
    asid: u32,
    ppn: u32,
    flags: u32,
    valid: bool,
}

impl Default for TlbEntry {
    fn default() -> Self {
        Self { vpn: 0, asid: 0, ppn: 0, flags: 0, valid: false }
    }
}

/// 64-entry fully-associative TLB with ASID-aware invalidation.
pub struct Tlb {
    entries: [TlbEntry; TLB_SIZE],
    next_idx: usize,
}

impl Default for Tlb {
    fn default() -> Self {
        Self::new()
    }
}

impl Tlb {
    pub fn new() -> Self {
        Self { entries: [TlbEntry::default(); TLB_SIZE], next_idx: 0 }
    }

    pub fn lookup(&self, vpn: u32, asid: u32) -> Option<(u32, u32)> {
        for entry in &self.entries {
            if entry.valid && entry.vpn == vpn && (entry.asid == asid || entry.flags & PTE_G != 0) {
                return Some((entry.ppn, entry.flags));
            }
        }
        None
    }

    pub fn insert(&mut self, vpn: u32, asid: u32, ppn: u32, flags: u32) {
        let idx = self.next_idx % TLB_SIZE;
        self.entries[idx] = TlbEntry { vpn, asid, ppn, flags, valid: true };
        self.next_idx = self.next_idx.wrapping_add(1);
    }

    pub fn flush_all(&mut self) {
        for entry in &mut self.entries { entry.valid = false; }
        self.next_idx = 0;
    }

    pub fn flush_asid(&mut self, asid: u32) {
        for entry in &mut self.entries {
            if entry.valid && entry.asid == asid { entry.valid = false; }
        }
    }

    pub fn flush_vpn(&mut self, vpn: u32, asid: u32) {
        for entry in &mut self.entries {
            if entry.valid && entry.vpn == vpn && (entry.asid == asid || entry.flags & PTE_G != 0) {
                entry.valid = false;
            }
        }
    }
}

// ---- Field extraction helpers ----

pub fn va_vpn1(va: u32) -> u32 { (va >> VA_VPN1_LSB) & VPN_MASK }
pub fn va_vpn0(va: u32) -> u32 { (va >> VA_VPN0_LSB) & VPN_MASK }
pub fn va_offset(va: u32) -> u32 { va & VA_OFFSET_MASK }
pub fn va_to_vpn(va: u32) -> u32 { va >> PAGE_SHIFT }
pub fn satp_asid(satp: u32) -> u32 { (satp >> SATP_ASID_LSB) & ((1 << SATP_ASID_BITS) - 1) }
pub fn satp_ppn(satp: u32) -> u32 { satp & SATP_PPN_MASK }
pub fn satp_mode_enabled(satp: u32) -> bool { (satp >> SATP_MODE_BIT) & 1 != 0 }

fn pte_is_leaf(pte: u32) -> bool { (pte & (PTE_R | PTE_W | PTE_X)) != 0 }
fn pte_ppn(pte: u32) -> u32 { (pte >> 10) & 0x003F_FFFF }

fn check_permissions(pte_flags: u32, access: AccessType, is_user: bool) -> bool {
    let u = (pte_flags & PTE_U) != 0;
    let r = (pte_flags & PTE_R) != 0;
    let w = (pte_flags & PTE_W) != 0;
    let x = (pte_flags & PTE_X) != 0;
    match access {
        AccessType::Fetch => if is_user { u && x } else { !u && x },
        AccessType::Load  => if is_user { u && r } else { !u && r },
        AccessType::Store => if is_user { u && w } else { !u && w },
    }
}

fn fault_for_access(access: AccessType) -> TranslateResult {
    match access {
        AccessType::Fetch => TranslateResult::FetchFault,
        AccessType::Load  => TranslateResult::LoadFault,
        AccessType::Store => TranslateResult::StoreFault,
    }
}

/// Translate a virtual address to physical using Sv32 page tables.
pub fn translate(
    va: u32, access: AccessType, is_user: bool, satp: u32, bus: &Bus, tlb: &mut Tlb,
) -> TranslateResult {
    if !satp_mode_enabled(satp) { return TranslateResult::Ok(va as u64); }

    let asid = satp_asid(satp);
    let vpn = va_to_vpn(va);
    let offset = va_offset(va);

    if let Some((ppn, flags)) = tlb.lookup(vpn, asid) {
        if !check_permissions(flags, access, is_user) { return fault_for_access(access); }
        return TranslateResult::Ok(((ppn as u64) << PAGE_SHIFT) | (offset as u64));
    }

    let root_ppn = satp_ppn(satp);
    let vpn1 = va_vpn1(va);
    let vpn0 = va_vpn0(va);

    let l1_addr = (root_ppn as u64) << PAGE_SHIFT | ((vpn1 as u64) * 4);
    let l1_pte = match bus.read_word(l1_addr) { Ok(w) => w, Err(_) => return fault_for_access(access) };
    if (l1_pte & PTE_V) == 0 { return fault_for_access(access); }

    if pte_is_leaf(l1_pte) {
        if !check_permissions(l1_pte & 0x3FF, access, is_user) { return fault_for_access(access); }
        let ppn = pte_ppn(l1_pte);
        let pa = ((ppn as u64) << PAGE_SHIFT) | ((vpn0 as u64) << PAGE_SHIFT) | (offset as u64);
        let mega_ppn = (ppn & !VPN_MASK) | vpn0;
        tlb.insert(vpn, asid, mega_ppn, l1_pte & 0x3FF);
        return TranslateResult::Ok(pa);
    }

    let l2_base = (pte_ppn(l1_pte) as u64) << PAGE_SHIFT;
    let l2_addr = l2_base | ((vpn0 as u64) * 4);
    let l2_pte = match bus.read_word(l2_addr) { Ok(w) => w, Err(_) => return fault_for_access(access) };
    if (l2_pte & PTE_V) == 0 { return fault_for_access(access); }
    if !pte_is_leaf(l2_pte) { return fault_for_access(access); }
    if !check_permissions(l2_pte & 0x3FF, access, is_user) { return fault_for_access(access); }

    let ppn = pte_ppn(l2_pte);
    let pa = ((ppn as u64) << PAGE_SHIFT) | (offset as u64);
    tlb.insert(vpn, asid, ppn, l2_pte & 0x3FF);
    TranslateResult::Ok(pa)
}

pub fn page_fault_cause(access: AccessType) -> u32 {
    match access {
        AccessType::Fetch => csr::CAUSE_FETCH_PAGE_FAULT,
        AccessType::Load  => csr::CAUSE_LOAD_PAGE_FAULT,
        AccessType::Store => csr::CAUSE_STORE_PAGE_FAULT,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test] fn test_va_fields() {
        assert_eq!(va_vpn1(0x0040_0000), 1);
        assert_eq!(va_vpn0(0x0000_1000), 1);
        assert_eq!(va_offset(0x0000_0FFF), 0xFFF);
        assert_eq!(va_to_vpn(0xFFFF_F000), 0xFFFFF);
    }
    #[test] fn test_satp_fields() {
        let s = (1u32 << 31) | (42u32 << 22) | 0x12345;
        assert!(satp_mode_enabled(s)); assert_eq!(satp_asid(s), 42); assert_eq!(satp_ppn(s), 0x12345);
        assert!(!satp_mode_enabled(0));
    }
    #[test] fn test_pte_leaf() { assert!(pte_is_leaf(PTE_V|PTE_R)); assert!(!pte_is_leaf(PTE_V)); }
    #[test] fn test_permissions_user() {
        let f = PTE_V|PTE_R|PTE_U;
        assert!(check_permissions(f, AccessType::Load, true));
        assert!(!check_permissions(f, AccessType::Store, true));
    }
    #[test] fn test_permissions_supervisor() {
        let f = PTE_V|PTE_R;
        assert!(check_permissions(f, AccessType::Load, false));
        assert!(!check_permissions(f, AccessType::Load, true));
    }
    #[test] fn test_tlb_basic() {
        let mut t = Tlb::new();
        assert!(t.lookup(0x100, 0).is_none());
        t.insert(0x100, 0, 0xAAA, PTE_V|PTE_R);
        let (ppn, _) = t.lookup(0x100, 0).unwrap();
        assert_eq!(ppn, 0xAAA);
    }
    #[test] fn test_tlb_flush() {
        let mut t = Tlb::new();
        t.insert(0x100, 0, 0xAAA, PTE_V|PTE_R);
        t.flush_all();
        assert!(t.lookup(0x100, 0).is_none());
    }
    #[test] fn test_tlb_asid() {
        let mut t = Tlb::new();
        t.insert(0x100, 1, 0xAAA, PTE_V|PTE_R);
        t.insert(0x100, 2, 0xBBB, PTE_V|PTE_R);
        assert_eq!(t.lookup(0x100, 1).unwrap().0, 0xAAA);
        assert_eq!(t.lookup(0x100, 2).unwrap().0, 0xBBB);
        assert!(t.lookup(0x100, 3).is_none());
    }
    #[test] fn test_tlb_global() {
        let mut t = Tlb::new();
        t.insert(0x42, 5, 0x100, PTE_V|PTE_R|PTE_G);
        assert!(t.lookup(0x42, 0).is_some());
        assert!(t.lookup(0x42, 99).is_some());
    }
    #[test] fn test_bare_mode() {
        let mut t = Tlb::new();
        let b = Bus::new(0x8000_0000, 4096);
        assert_eq!(translate(0x8000_0000, AccessType::Fetch, false, 0, &b, &mut t), TranslateResult::Ok(0x8000_0000));
    }
    #[test] fn test_page_fault_codes() {
        assert_eq!(page_fault_cause(AccessType::Fetch), 12);
        assert_eq!(page_fault_cause(AccessType::Load), 13);
        assert_eq!(page_fault_cause(AccessType::Store), 15);
    }
}
