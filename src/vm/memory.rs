use super::types::*;
use super::Vm;

impl Vm {
    /// Allocate `count` contiguous physical pages. Returns start page index or None.
    /// Starts scanning from page 2 (pages 0-1 reserved for kernel/main).
    /// Only scans up to NUM_RAM_PAGES (0-63).
    pub(super) fn alloc_pages(&mut self, count: usize) -> Option<usize> {
        'outer: for start in 2..=(NUM_RAM_PAGES - count) {
            for i in 0..count {
                if self.allocated_pages & (1u64 << (start + i)) != 0 {
                    continue 'outer;
                }
            }
            for i in 0..count {
                self.allocated_pages |= 1u64 << (start + i);
                self.page_ref_count[start + i] = 1;
            }
            return Some(start);
        }
        None
    }

    /// Free physical pages mapped by a page directory, respecting COW reference counts.
    /// Only actually frees a page when its reference count drops to 0.
    pub(super) fn free_page_dir(&mut self, pd: &[u32]) {
        for &entry in pd {
            let ppage = entry as usize;
            if ppage < NUM_RAM_PAGES {
                if self.page_ref_count[ppage] > 1 {
                    // Page is shared (COW) -- just decrement ref count
                    self.page_ref_count[ppage] -= 1;
                    // Clear COW flag if only one reference remains
                    if self.page_ref_count[ppage] == 1 {
                        self.page_cow &= !(1u64 << ppage);
                    }
                } else {
                    // Last reference -- actually free the page
                    self.allocated_pages &= !(1u64 << ppage);
                    self.page_ref_count[ppage] = 0;
                    self.page_cow &= !(1u64 << ppage);
                }
            }
        }
    }

    /// Translate virtual address using current page directory.
    /// Returns None if unmapped (triggers segfault).
    pub(super) fn translate_va(&self, vaddr: u32) -> Option<usize> {
        match &self.current_page_dir {
            None => Some(vaddr as usize), // identity mapping (kernel)
            Some(pd) => {
                let vpage = (vaddr as usize) / PAGE_SIZE;
                let offset = (vaddr as usize) % PAGE_SIZE;
                if vpage >= pd.len() {
                    return None;
                }
                let ppage = pd[vpage] as usize;
                if ppage >= NUM_PAGES {
                    return None;
                } // PAGE_UNMAPPED sentinel
                Some(ppage * PAGE_SIZE + offset)
            }
        }
    }

    /// Create a page directory for a new process: allocate PROCESS_PAGES contiguous
    /// physical pages, map virtual pages 0..PROCESS_PAGES to them, rest unmapped.
    /// The shared region (page containing 0xF00-0xFFF for Window Bounds Protocol,
    /// page containing 0xFF00+ for hardware ports) is identity-mapped so processes
    /// can communicate and access hardware through syscalls.
    pub(super) fn create_process_page_dir(&mut self) -> Option<Vec<u32>> {
        let start = self.alloc_pages(PROCESS_PAGES)?;
        let mut pd = vec![PAGE_UNMAPPED; NUM_PAGES];
        for (i, pd_entry) in pd.iter_mut().enumerate().take(PROCESS_PAGES) {
            *pd_entry = (start + i) as u32;
        }
        // Identity-map shared regions so child processes can access them
        // Page 3 (0xC00-0xFFF): contains Window Bounds Protocol at 0xF00-0xFFF
        pd[3] = 3;
        // Release the private page we allocated for virtual page 3
        let private_page = start + 3;
        if private_page < NUM_PAGES {
            self.allocated_pages &= !(1u64 << private_page);
        }
        // Page 63 (0xFC00-0xFFFF): hardware ports (0xFF00+) and syscall table (0xFE00+)
        // This is already outside PROCESS_PAGES range so it's PAGE_UNMAPPED.
        // Identity-map it so syscalls work.
        pd[63] = 63;
        // Don't allocate page 63 -- it's always the kernel's hardware page
        // (main process uses it via identity mapping)
        Some(pd)
    }

    /// Handle a write to a copy-on-write page.
    ///
    /// Called when STORE targets a physical page marked as COW.
    /// Allocates a new physical page, copies the data, updates the
    /// current page directory to point to the new page, and decrements
    /// the ref count on the old page.
    ///
    /// Returns true if the COW was resolved (page now writable), false on allocation failure.
    pub(super) fn handle_cow_write(&mut self, vaddr: u32) -> bool {
        let vpage = (vaddr as usize) / PAGE_SIZE;

        // Extract old physical page from current page directory
        let old_phys = match &self.current_page_dir {
            Some(pd) => {
                if vpage >= pd.len() || vpage >= NUM_PAGES {
                    return false;
                }
                let p = pd[vpage] as usize;
                if p >= NUM_PAGES {
                    return false;
                }
                p
            }
            None => return false,
        };

        // Check if this page is actually COW
        if self.page_cow & (1u64 << old_phys) == 0 {
            return false;
        }

        // Allocate a new physical page for the private copy
        let new_phys = match self.alloc_pages(1) {
            Some(p) => p,
            None => return false,
        };

        // Copy the page contents
        let old_base = old_phys * PAGE_SIZE;
        let new_base = new_phys * PAGE_SIZE;
        for i in 0..PAGE_SIZE {
            if old_base + i < self.ram.len() && new_base + i < self.ram.len() {
                self.ram[new_base + i] = self.ram[old_base + i];
            }
        }

        // Update page directory to point to the new private page
        if let Some(ref mut pd) = self.current_page_dir {
            pd[vpage] = new_phys as u32;
        }

        // Decrement ref count on old page, clear COW if last reference
        self.page_ref_count[old_phys] -= 1;
        if self.page_ref_count[old_phys] <= 1 {
            self.page_cow &= !(1u64 << old_phys);
        }

        // New page is NOT COW (ref_count = 1 from alloc_pages)
        self.page_cow &= !(1u64 << new_phys);

        true
    }

    /// Check if a write to the given virtual address targets a COW page.
    /// If so, resolve the COW by copying the page to a private one.
    pub(super) fn resolve_cow_if_needed(&mut self, vaddr: u32) {
        let phys = match self.translate_va(vaddr) {
            Some(addr) => addr / PAGE_SIZE,
            None => return,
        };
        if phys < NUM_RAM_PAGES && (self.page_cow & (1u64 << phys)) != 0 {
            self.handle_cow_write(vaddr);
        }
    }

    /// Try to handle a page fault for the given virtual address.
    ///
    /// When translate_va returns None (unmapped page), this method checks if
    /// the faulting page could be resolved by allocating a new physical page.
    /// It uses a simple rule based on the process memory layout:
    /// - Pages 0..1 are code/heap (pre-allocated at spawn)
    /// - Page 2 is stack (pre-allocated at spawn)
    /// - Pages in the range PROCESS_PAGES..up to 60 are eligible for demand allocation
    ///   (this covers heap growth beyond the initial 4 pages, stack growth, and mmap)
    ///
    /// Returns true if the fault was resolved (page now mapped), false otherwise.
    pub(super) fn handle_page_fault(&mut self, vaddr: u32) -> bool {
        let vpage = (vaddr as usize) / PAGE_SIZE;

        // Kernel mode has no page directory -- no fault handling needed
        match &self.current_page_dir {
            None => return false,
            Some(pd) => {
                if vpage >= pd.len() || vpage >= NUM_PAGES {
                    return false;
                }
                // Don't re-allocate already-mapped pages
                if (pd[vpage] as usize) < NUM_PAGES {
                    return false;
                }
                // Don't allocate for kernel pages (63) or above
                if vpage > 62 {
                    return false;
                }
            }
        }

        // If this process has VMAs, only allocate if a VMA covers this page and permits growth.
        // An empty VMA list means the old behavior (no VMA restrictions) applies.
        if !self.current_vmas.is_empty() {
            let allowed = self.current_vmas.iter().any(|vma| vma.can_grow_to(vpage));
            if !allowed {
                return false;
            }
        }

        // Allocate a single physical page
        let ppage = match self.alloc_pages(1) {
            Some(p) => p,
            None => return false,
        };

        // Map it in the page directory
        if let Some(ref mut pd) = self.current_page_dir {
            if vpage < pd.len() {
                pd[vpage] = ppage as u32;
            }
        }

        // Zero the newly allocated page
        let phys_base = ppage * PAGE_SIZE;
        for i in 0..PAGE_SIZE {
            if phys_base + i < self.ram.len() {
                self.ram[phys_base + i] = 0;
            }
        }

        true
    }

    /// Translate virtual address, attempting page fault resolution on miss.
    /// Returns None only if the fault cannot be resolved (triggers segfault).
    pub(super) fn translate_va_or_fault(&mut self, vaddr: u32) -> Option<usize> {
        // First try normal translation
        match self.translate_va(vaddr) {
            Some(addr) => Some(addr),
            None => {
                // Try to resolve the page fault
                if self.handle_page_fault(vaddr) {
                    // Retry translation after mapping the page
                    self.translate_va(vaddr)
                } else {
                    None
                }
            }
        }
    }

    /// Trigger a segfault: set flag and halt the process.
    pub(super) fn trigger_segfault(&mut self) {
        self.segfault = true;
        self.halted = true;
    }
}
