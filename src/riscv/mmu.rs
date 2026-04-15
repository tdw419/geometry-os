// riscv/mmu.rs -- SV32 Memory Management Unit (Phase 36)
//
// Implements SV32 virtual memory translation for RISC-V:
//   - 2-level page table walk (10-bit VPN indices)
//   - Page table entry flags: V, R, W, X, U, G, A, D
//   - TLB with ASID-aware invalidation
//   - Page fault generation (instruction, load, store)
//
// SV32 virtual address format (32 bits):
//   [31:22] VPN[1] (10 bits)
//   [21:12] VPN[0] (10 bits)
//   [11:0]  page offset (12 bits)
//
// SV32 page table entry (32 bits):
//   [31:20] PPN[1] (12 bits)
//   [19:10] PPN[0] (10 bits)
//   [9:8]   RSW (reserved for software)
//   [7]     D (dirty)
//   [6]     A (accessed)
//   [5]     G (global)
//   [4]     U (user)
//   [3]     X (execute)
//   [2]     W (write)
//   [1]     R (read)
//   [0]     V (valid)

use super::bus::Bus;
use super::cpu::Privilege;

// ---- PTE flag constants ----

pub const PTE_V: u32 = 1 << 0;
pub const PTE_R: u32 = 1 << 1;
pub const PTE_W: u32 = 1 << 2;
pub const PTE_X: u32 = 1 << 3;
pub const PTE_U: u32 = 1 << 4;
pub const PTE_G: u32 = 1 << 5;
pub const PTE_A: u32 = 1 << 6;
pub const PTE_D: u32 = 1 << 7;

// ---- satp field extraction ----

/// Check if SV32 mode is enabled (bit 31 of satp).
pub fn satp_mode_enabled(satp: u32) -> bool {
    (satp >> 31) & 1 != 0
}

/// Extract ASID from satp (bits [30:22]).
pub fn satp_asid(satp: u32) -> u16 {
    ((satp >> 22) & 0x1FF) as u16
}

/// Extract root page table PPN from satp (bits [21:0]).
pub fn satp_ppn(satp: u32) -> u32 {
    satp & 0x003F_FFFF
}

// ---- VA field extraction ----

/// Extract VPN[1] from a virtual address (bits [31:22]).
pub fn va_vpn1(va: u32) -> u32 {
    (va >> 22) & 0x3FF
}

/// Extract VPN[0] from a virtual address (bits [21:12]).
pub fn va_vpn0(va: u32) -> u32 {
    (va >> 12) & 0x3FF
}

/// Extract page offset from a virtual address (bits [11:0]).
pub fn va_offset(va: u32) -> u32 {
    va & 0xFFF
}

/// Combine VPN[1] and VPN[0] into a single VPN value for TLB lookup.
pub fn va_to_vpn(va: u32) -> u32 {
    (va >> 12) & 0xFFFFF // 20-bit combined VPN
}

// ---- Access type ----

/// Memory access type (determines fault cause code).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccessType {
    Fetch,
    Load,
    Store,
}

// ---- Translation result ----

/// Result of a virtual address translation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TranslateResult {
    /// Translation succeeded. Contains the physical address.
    Ok(u64),
    /// Instruction fetch page fault.
    FetchFault,
    /// Load page fault.
    LoadFault,
    /// Store/AMO page fault.
    StoreFault,
}
/// MMU trace event (Phase 41).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MmuEvent {
    /// SATP register written.
    SatpWrite {
        old: u32,
        new: u32,
    },
    /// Walk completed successfully.
    PageTableWalk {
        va: u32,
        pa: u64,
        ptes: Vec<u32>,
    },
    /// Walk failed with a page fault.
    PageFault {
        va: u32,
        access: AccessType,
        ptes: Vec<u32>,
    },
    /// Translation hit in the TLB.
    TlbHit {
        va: u32,
        pa: u64,
    },
}

// ---- TLB ----
//
// Uses a HashMap instead of a fixed-size array. This matches QEMU's behavior:
// TLB entries persist until explicitly flushed (SFENCE.VMA, SATP change).
// No capacity-based eviction. Linux modifies page table entries without
// SFENCE.VMA during boot, relying on stale TLB entries to remain valid
// until the kernel finishes the update and flushes. A fixed-size TLB with
// eviction breaks this pattern.

use std::collections::HashMap;

/// TLB key: (vpn, asid). Global entries use asid=0.
type TlbKey = (u32, u16);

/// A single TLB entry.
#[derive(Clone, Copy, Debug)]
struct TlbEntry {
    ppn: u32,
    flags: u32,
}

/// Translation Lookaside Buffer.
/// Caches virtual-to-physical mappings with ASID tagging.
/// Global entries (PTE_G) match any ASID.
/// No capacity-based eviction — entries live until explicitly flushed.
#[derive(Clone, Debug)]
pub struct Tlb {
    entries: HashMap<TlbKey, TlbEntry>,
}

impl Default for Tlb {
    fn default() -> Self {
        Self::new()
    }
}

impl Tlb {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// Look up a VPN/ASID in the TLB.
    /// Returns (ppn, flags) if found, None if not.
    /// Global entries (PTE_G) match any ASID — they're stored at asid=0.
    pub fn lookup(&self, vpn: u32, asid: u16) -> Option<(u32, u32)> {
        // Check exact (vpn, asid) match first
        if let Some(entry) = self.entries.get(&(vpn, asid)) {
            return Some((entry.ppn, entry.flags));
        }
        // For non-zero ASID, also check global entries (asid=0 with PTE_G set)
        if asid != 0 {
            if let Some(entry) = self.entries.get(&(vpn, 0)) {
                if (entry.flags & PTE_G) != 0 {
                    return Some((entry.ppn, entry.flags));
                }
            }
        }
        None
    }

    /// Insert an entry into the TLB.
    /// Global entries (PTE_G) are stored at asid=0 so any lookup can find them.
    /// If an entry already exists for this key, it is updated (not duplicated).
    pub fn insert(&mut self, vpn: u32, asid: u16, ppn: u32, flags: u32) {
        let insert_asid = if (flags & PTE_G) != 0 { 0 } else { asid };
        self.entries.insert(
            (vpn, insert_asid),
            TlbEntry { ppn, flags },
        );
    }

    /// Flush all TLB entries.
    pub fn flush_all(&mut self) {
        self.entries.clear();
    }

    /// Flush entries for a specific virtual address.
    pub fn flush_va(&mut self, vpn: u32) {
        self.entries.retain(|&(v, _), _| v != vpn);
    }

    /// Flush entries for a specific ASID (non-global only).
    pub fn flush_asid(&mut self, asid: u16) {
        self.entries.retain(|&(_, a), entry| {
            a != asid || (entry.flags & PTE_G) != 0
        });
    }

    /// Flush entries matching both a specific VPN and ASID.
    pub fn flush_va_asid(&mut self, vpn: u32, asid: u16) {
        self.entries.remove(&(vpn, asid));
    }

    /// Count valid entries (for testing capacity).
    pub fn valid_count(&self) -> usize {
        self.entries.len()
    }
}

// ---- Translation ----

/// PPN mask from a PTE (bits [31:10]).
const PPN_MASK: u32 = 0xFFFF_FC00;

/// Extract PPN from a PTE.
fn pte_ppn(pte: u32) -> u32 {
    (pte & PPN_MASK) >> 10
}

/// Translate a virtual address to a physical address.
///
/// If satp MODE is 0 (bare), returns va unchanged.
/// Otherwise performs SV32 page table walk.
///
/// # Arguments
/// * `va` - Virtual address to translate
/// * `access_type` - Type of access (fetch/load/store)
/// * `effective_priv` - Effective privilege level for this access
/// * `sum` - SUM bit from mstatus (allow S-mode to access U pages)
/// * `satp` - Current satp CSR value
/// * `bus` - Memory bus for page table walks
/// * `tlb` - TLB for caching translations
pub fn translate(
    va: u32,
    access_type: AccessType,
    effective_priv: Privilege,
    sum: bool,
    mxr: bool,
    satp: u32,
    bus: &mut Bus,
    tlb: &mut Tlb,
) -> TranslateResult {
    // Bare mode: no translation.
    if !satp_mode_enabled(satp) {
        return TranslateResult::Ok(va as u64);
    }

    let vpn1 = va_vpn1(va);
    let vpn0 = va_vpn0(va);
    let offset = va_offset(va);
    let combined_vpn = va_to_vpn(va);
    let asid = satp_asid(satp);

    // Check TLB first.
    if let Some((ppn, flags)) = tlb.lookup(combined_vpn, asid) {
        if let Some(fault) = check_permissions(flags, access_type, effective_priv, sum, mxr) {
            bus.mmu_log.push(MmuEvent::PageFault {
                va,
                access: access_type,
                ptes: Vec::new(),
            });
            return fault;
        }
        let pa = ((ppn as u64) << 12) | (offset as u64);
        bus.mmu_log.push(MmuEvent::TlbHit { va, pa });
        return TranslateResult::Ok(pa);
    }

    // TLB miss: walk page tables.
    let root_ppn = satp_ppn(satp);
    let root_addr = (root_ppn as u64) << 12;

    // Level 1: read PTE at root[VPN[1]].
    let l1_addr = root_addr | ((vpn1 as u64) << 2);
    let l1_pte = match bus.read_word(l1_addr) {
        Ok(w) => w,
        Err(_) => {
            bus.mmu_log.push(MmuEvent::PageFault {
                va,
                access: access_type,
                ptes: Vec::new(),
            });
            return fault_for(access_type);
        }
    };

    if (l1_pte & PTE_V) == 0 {
        bus.mmu_log.push(MmuEvent::PageFault {
            va,
            access: access_type,
            ptes: vec![l1_pte],
        });
        return fault_for(access_type);
    }

    let is_leaf_l1 = (l1_pte & (PTE_R | PTE_W | PTE_X)) != 0;

    if is_leaf_l1 {
        // Megapage (2MB superpage in SV32).
        // PA[31:22] = PTE.PPN[19:10], PA[21:12] = VA.VPN0[9:0], PA[11:0] = VA.offset
        // The lower 10 bits of PTE.PPN are reserved (should be zero for megapages).
        let ppn_hi = (l1_pte >> 20) & 0xFFF; // PTE.PPN[19:10] → PA[31:22]
        let pa = ((ppn_hi as u64) << 22) | ((vpn0 as u64) << 12) | (offset as u64);
        let flags = l1_pte & 0xFF;

        if let Some(fault) = check_permissions(flags, access_type, effective_priv, sum, mxr) {
            bus.mmu_log.push(MmuEvent::PageFault {
                va,
                access: access_type,
                ptes: vec![l1_pte],
            });
            return fault;
        }

        // A/D bit updates DISABLED for testing.
        // The PTE flags are used as-read (no write-back).
        let flags = l1_pte & 0xFF;

        // For TLB: store the effective PPN for this specific VPN (includes VPN0).
        // Each TLB entry covers one 4KB page, so megapage hits insert per-VPN0.
        let eff_ppn = (pa >> 12) as u32;
        tlb.insert(combined_vpn, asid, eff_ppn, flags);
        bus.mmu_log.push(MmuEvent::PageTableWalk {
            va,
            pa,
            ptes: vec![l1_pte],
        });
        return TranslateResult::Ok(pa);
    }

    // Non-leaf: follow pointer to level 2.
    let l2_base = (pte_ppn(l1_pte) as u64) << 12;
    let l2_addr = l2_base | ((vpn0 as u64) << 2);
    let l2_pte = match bus.read_word(l2_addr) {
        Ok(w) => w,
        Err(_) => {
            bus.mmu_log.push(MmuEvent::PageFault {
                va,
                access: access_type,
                ptes: vec![l1_pte],
            });
            return fault_for(access_type);
        }
    };

    if (l2_pte & PTE_V) == 0 {
        bus.mmu_log.push(MmuEvent::PageFault {
            va,
            access: access_type,
            ptes: vec![l1_pte, l2_pte],
        });
        return fault_for(access_type);
    }

    // Level 2 must be a leaf.
    let is_leaf_l2 = (l2_pte & (PTE_R | PTE_W | PTE_X)) != 0;
    if !is_leaf_l2 {
        bus.mmu_log.push(MmuEvent::PageFault {
            va,
            access: access_type,
            ptes: vec![l1_pte, l2_pte],
        });
        return fault_for(access_type);
    }

    let flags = l2_pte & 0xFF;

    if let Some(fault) = check_permissions(flags, access_type, effective_priv, sum, mxr) {
        bus.mmu_log.push(MmuEvent::PageFault {
            va,
            access: access_type,
            ptes: vec![l1_pte, l2_pte],
        });
        return fault;
    }

    // A/D bit updates DISABLED for testing (L2 leaf).
    let flags = l2_pte & 0xFF;
    let ppn = pte_ppn(l2_pte);

    tlb.insert(combined_vpn, asid, ppn, flags);
    let pa = ((ppn as u64) << 12) | (offset as u64);
    bus.mmu_log.push(MmuEvent::PageTableWalk {
        va,
        pa,
        ptes: vec![l1_pte, l2_pte],
    });
    TranslateResult::Ok(pa)
}

/// Check page permissions.
/// Returns Some(fault) if the access should fault, None if OK.
///
/// When `sum` is true (S-mode, SUM=1), S-mode can access U-mode pages.
/// M-mode (effective_priv == Machine) bypasses all permission checks.
fn check_permissions(
    flags: u32,
    access_type: AccessType,
    effective_priv: Privilege,
    sum: bool,
    mxr: bool,
) -> Option<TranslateResult> {
    // M-mode bypasses all permission checks.
    if effective_priv == Privilege::Machine {
        return None;
    }
    // U-mode can only access user pages (PTE_U set).
    if effective_priv == Privilege::User && (flags & PTE_U) == 0 {
        return Some(fault_for(access_type));
    }
    // S-mode can access supervisor pages. With SUM=1, also user pages.
    if effective_priv == Privilege::Supervisor && (flags & PTE_U) != 0 && !sum {
        return Some(fault_for(access_type));
    }
    // Check access type against R/W/X bits.
    match access_type {
        AccessType::Fetch => {
            if (flags & PTE_X) == 0 {
                return Some(TranslateResult::FetchFault);
            }
        }
        AccessType::Load => {
            // MXR: Make eXecutable Readable. When set, S-mode can read
            // from pages with X=1 even if R=0.
            if (flags & PTE_R) == 0 && !(mxr && (flags & PTE_X) != 0) {
                return Some(TranslateResult::LoadFault);
            }
        }
        AccessType::Store => {
            if (flags & PTE_W) == 0 {
                return Some(TranslateResult::StoreFault);
            }
        }
    }
    None
}

/// Get the appropriate fault variant for an access type.
fn fault_for(access_type: AccessType) -> TranslateResult {
    match access_type {
        AccessType::Fetch => TranslateResult::FetchFault,
        AccessType::Load => TranslateResult::LoadFault,
        AccessType::Store => TranslateResult::StoreFault,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bare_mode_identity() {
        let mut tlb = Tlb::new();
        let mut bus = Bus::new(0x8000_0000, 8192);
        let result = translate(0x8000_0000, AccessType::Fetch, Privilege::Machine, false, false, 0, &mut bus, &mut tlb);
        assert_eq!(result, TranslateResult::Ok(0x8000_0000));
    }

    #[test]
    fn satp_field_extraction() {
        let satp = (1u32 << 31) | (42u32 << 22) | 0x12345;
        assert!(satp_mode_enabled(satp));
        assert_eq!(satp_asid(satp), 42);
        assert_eq!(satp_ppn(satp), 0x12345);
        assert!(!satp_mode_enabled(0));
    }

    #[test]
    fn va_field_extraction() {
        assert_eq!(va_vpn1(0x0040_1100), 1);
        assert_eq!(va_vpn0(0x0040_1100), 1);
        assert_eq!(va_offset(0x0040_1100), 0x100);
        assert_eq!(va_to_vpn(0x0040_1100), 0x00401);
    }

    #[test]
    fn tlb_insert_lookup() {
        let mut tlb = Tlb::new();
        tlb.insert(0x100, 1, 0xAAA, PTE_V | PTE_R);
        assert_eq!(tlb.lookup(0x100, 1), Some((0xAAA, PTE_V | PTE_R)));
    }

    #[test]
    fn tlb_flush_all() {
        let mut tlb = Tlb::new();
        tlb.insert(0x100, 1, 0xAAA, PTE_V | PTE_R);
        tlb.insert(0x200, 1, 0xBBB, PTE_V | PTE_R);
        tlb.flush_all();
        assert!(tlb.lookup(0x100, 1).is_none());
        assert!(tlb.lookup(0x200, 1).is_none());
    }

    #[test]
    fn tlb_asid_isolation() {
        let mut tlb = Tlb::new();
        tlb.insert(0x100, 1, 0xAAA, PTE_V | PTE_R);
        tlb.insert(0x100, 2, 0xBBB, PTE_V | PTE_R);
        assert_eq!(tlb.lookup(0x100, 1).unwrap().0, 0xAAA);
        assert_eq!(tlb.lookup(0x100, 2).unwrap().0, 0xBBB);
        assert!(tlb.lookup(0x100, 3).is_none());
    }

    #[test]
    fn tlb_global_entry() {
        let mut tlb = Tlb::new();
        tlb.insert(0x42, 5, 0x100, PTE_V | PTE_R | PTE_G);
        assert!(tlb.lookup(0x42, 0).is_some());
        assert!(tlb.lookup(0x42, 99).is_some());
        assert!(tlb.lookup(0x43, 5).is_none());
    }

    #[test]
    fn page_table_walk_logging() {
        let mut tlb = Tlb::new();
        let mut bus = Bus::new(0x0, 0x1_0000);
        // Map 0x1000 to 0x5000 via megapage
        let l1_addr = 0x0;
        let pte = (0x5u32 << 20) | PTE_V | PTE_R | PTE_X;
        bus.write_word(l1_addr, pte).unwrap();
        
        let satp = make_satp(1, 0, 0);
        let result = translate(0x1000, AccessType::Fetch, Privilege::Supervisor, false, false, satp, &mut bus, &mut tlb);
        assert!(matches!(result, TranslateResult::Ok(0x0140_1000)));
        
        assert_eq!(bus.mmu_log.len(), 1);
        if let MmuEvent::PageTableWalk { va, pa, ptes } = &bus.mmu_log[0] {
            assert_eq!(*va, 0x1000);
            assert_eq!(*pa, 0x0140_1000);
            assert_eq!(ptes.len(), 1);
            assert_eq!(ptes[0], pte);
        } else {
            panic!("Expected PageTableWalk event");
        }
    }

    fn make_satp(mode: u32, asid: u32, ppn: u32) -> u32 {
        ((mode & 1) << 31) | ((asid & 0x1FF) << 22) | (ppn & 0x003F_FFFF)
    }
}