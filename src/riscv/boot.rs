// riscv/boot.rs -- Guest and Linux boot methods for RiscvVm
//
// Split from mod.rs for maintainability.
// Contains: boot_guest, boot_linux_setup, boot_linux,
// ELF parsing helpers, and kernel page table fixup.

use super::cpu::{self, StepResult};
use super::csr;
use super::loader;
use super::{dtb, BootResult, RiscvVm};

#[allow(dead_code)]
impl RiscvVm {
    /// Boot a guest OS kernel image.
    ///
    /// 1. Load kernel image (ELF32 or raw binary) into guest RAM
    /// 2. Generate and load a DTB (device tree blob) into guest RAM
    /// 3. Set PC to entry point, a0=0 (hartid), a1=dtb_addr
    /// 4. Run for `max_instructions` steps or until EBREAK/halt
    ///
    /// Returns the number of instructions executed and boot metadata.
    pub fn boot_guest(
        &mut self,
        kernel_image: &[u8],
        ram_size_mb: u32,
        max_instructions: u64,
    ) -> Result<BootResult, loader::LoadError> {
        // 1. Load kernel image.
        let load_info = loader::load_auto(&mut self.bus, kernel_image, 0x8000_0000)?;

        // 2. Generate DTB and load it into guest RAM just after the kernel.
        let dtb_config = dtb::DtbConfig {
            ram_size: ram_size_mb as u64 * 1024 * 1024,
            ..Default::default()
        };
        let dtb_blob = dtb::generate_dtb(&dtb_config);

        // Place DTB at a page-aligned address after the kernel image.
        let dtb_addr = ((load_info.highest_addr + 0xFFF) & !0xFFF) as u64;
        for (i, &byte) in dtb_blob.iter().enumerate() {
            let addr = dtb_addr + i as u64;
            if self.bus.write_byte(addr, byte).is_err() {
                break;
            }
        }

        // 3. Set CPU state for boot.
        self.cpu.pc = load_info.entry;
        self.cpu.x[10] = 0; // a0 = hartid (0)
        self.cpu.x[11] = dtb_addr as u32; // a1 = DTB address
        self.cpu.privilege = cpu::Privilege::Machine;

        // 4. Execute.
        let mut count: u64 = 0;
        while count < max_instructions {
            match self.step() {
                StepResult::Ok
                | StepResult::FetchFault
                | StepResult::LoadFault
                | StepResult::StoreFault => {
                    // Page faults are delivered as traps by translate_va
                    // (mepc/mcause/mtval set, PC jumped to trap vector).
                    // The guest OS trap handler will handle them.
                }
                StepResult::Ebreak => break,
                StepResult::Ecall => {} // ECALL is normal during boot
            }
            count += 1;
        }

        Ok(BootResult {
            instructions: count,
            entry: load_info.entry,
            dtb_addr,
        })
    }

    /// Parse the first PT_LOAD segment's virtual address from an ELF image.
    /// Returns None if the image is too short or has no LOAD segments.
    /// Parse the first PT_LOAD segment's physical address from an ELF image.
    pub(super) fn parse_first_load_paddr(image: &[u8]) -> Option<u64> {
        if image.len() < 52 {
            return None;
        }
        if u32::from_le_bytes([image[0], image[1], image[2], image[3]]) != 0x464C457F {
            return None;
        }
        let phoff = u32::from_le_bytes([image[28], image[29], image[30], image[31]]) as usize;
        let phentsize = u16::from_le_bytes([image[42], image[43]]) as usize;
        let phnum = u16::from_le_bytes([image[44], image[45]]) as usize;

        for i in 0..phnum {
            let off = phoff + i * phentsize;
            if off + phentsize > image.len() {
                break;
            }
            let seg = &image[off..off + phentsize];
            let p_type = u32::from_le_bytes([seg[0], seg[1], seg[2], seg[3]]);
            if p_type == 1 {
                // PT_LOAD
                let p_paddr = u32::from_le_bytes([seg[12], seg[13], seg[14], seg[15]]) as u64;
                return Some(p_paddr);
            }
        }
        None
    }

    /// Parse the highest physical address (paddr + memsz) across all PT_LOAD segments.
    pub(super) fn parse_elf_highest_paddr(image: &[u8]) -> Option<u64> {
        let class = crate::riscv::loader::validate_elf_header(image).ok()?;
        let hdr = crate::riscv::loader::parse_elf_header(image, class);

        let mut highest: u64 = 0;
        for i in 0..hdr.phnum {
            let off = hdr.phoff + i * hdr.phentsize;
            let phdr = crate::riscv::loader::parse_phdr(image, off, class)?;
            if phdr.p_type == 1 {
                // PT_LOAD
                let seg_end = phdr.p_paddr as u64 + phdr.p_memsz as u64;
                if seg_end > highest {
                    highest = seg_end;
                }
            }
        }
        if highest == 0 {
            None
        } else {
            Some(highest)
        }
    }

    /// Convert a virtual entry point to physical using ELF segment mappings.
    /// For Linux, the ELF entry is a virtual address; we find which PT_LOAD
    /// segment contains it and compute phys = entry - p_vaddr + p_paddr.
    /// Supports both ELF32 and ELF64 images.
    pub(super) fn elf_entry_vaddr_to_phys(image: &[u8], entry_vaddr: u32) -> Option<u32> {
        let class = crate::riscv::loader::validate_elf_header(image).ok()?;
        let hdr = crate::riscv::loader::parse_elf_header(image, class);

        for i in 0..hdr.phnum {
            let off = hdr.phoff + i * hdr.phentsize;
            let phdr = crate::riscv::loader::parse_phdr(image, off, class)?;
            if phdr.p_type == 1
                && entry_vaddr >= phdr.p_vaddr
                && entry_vaddr < phdr.p_vaddr.wrapping_add(phdr.p_memsz as u32)
            {
                let offset = entry_vaddr - phdr.p_vaddr;
                return Some(phdr.p_paddr.wrapping_add(offset));
            }
        }
        None
    }

    /// Boot a Linux kernel with initramfs support (associated function).
    ///
    /// This is the main Linux boot entry point. Unlike `boot_guest`, it creates
    /// its own VM with the correct RAM layout for the kernel.
    ///
    /// **Key insight:** The kernel is linked with PAGE_OFFSET (e.g., 0xC0000000).
    /// All code references use virtual addresses in this range. With MMU off in
    /// M-mode, the CPU uses addresses as-is (no translation). So we place RAM
    /// at the kernel's first LOAD segment vaddr, making virtual == physical.
    /// This way, the `J _start_kernel` (which encodes virtual address 0xC00010D0)
    /// fetches from physical 0xC00010D0, which IS in RAM.
    ///
    /// MMIO devices (UART, CLINT, PLIC, virtio) remain at their standard addresses
    /// below 0xC0000000. The bus routes these to device handlers before checking RAM.
    ///
    /// Fix virtual PPNs in a kernel page table after SATP change.
    ///
    /// Linux's setup_vm() creates page table entries using virtual addresses
    /// as physical addresses (because __pa() is a no-op without SBI). For example,
    /// L1[768] = 0x300000EF maps VA 0xC0000000 to "PA" 0xC0000000 (identity),
    /// but the correct PA is 0x00000000. This function scans the page table
    /// and translates any PPNs >= PAGE_OFFSET/4096 by subtracting the offset.
    ///
    /// Called after each SATP change during Linux boot to fix the kernel's
    /// page tables in place.
    fn fixup_kernel_page_table(&mut self, pg_dir_phys: u64) {
        // Delegate to Bus which handles both PTE fixup AND page registration
        // for real-time write interception (demand paging).
        self.bus.fixup_kernel_page_table(pg_dir_phys);
    }

    /// Steps:
    /// Set up the VM for Linux boot without running the instruction loop.
    /// Returns (vm, fw_addr, entry, dtb_addr) so callers can run their own loop.
    pub fn boot_linux_setup(
        kernel_image: &[u8],
        initramfs: Option<&[u8]>,
        ram_size_mb: u32,
        bootargs: &str,
    ) -> Result<(Self, u64, u32, u64), loader::LoadError> {
        // 1. Calculate minimum RAM size from kernel's physical address ranges.
        let highest_paddr = Self::parse_elf_highest_paddr(kernel_image).unwrap_or(64 * 1024 * 1024);
        let min_ram_size = highest_paddr as usize + 4 * 1024 * 1024; // extra for initrd/dtb

        let caller_ram_size = (ram_size_mb as u64) * 1024 * 1024;
        let actual_ram_size = std::cmp::max(min_ram_size, caller_ram_size as usize);

        // 2. Create VM with ram_base=0.
        // This is critical: the kernel computes physical addresses as vaddr - PAGE_OFFSET.
        // With ram_base=0, physical addresses 0x00000000..map directly to RAM,
        // so the kernel's page table writes go to the correct physical locations.
        // Previously ram_base was set to the kernel's first LOAD vaddr (0xC0000000),
        // which caused all physical addresses below 0xC0000000 to be silently discarded.
        let mut vm = Self::new_with_base(0, actual_ram_size);
        vm.bus.low_addr_identity_map = false; // Disabled: trampoline page table handles identity mapping
        vm.bus.auto_pte_fixup = true; // Needed: MMU-level fixup_ppn translates virtual PPNs during page table walks

        // 3. Load kernel ELF at physical addresses (p_paddr).
        // The kernel's ELF has p_paddr = vaddr - PAGE_OFFSET, which are the correct
        // physical addresses for our ram_base=0 setup.
        let load_info = loader::load_elf(&mut vm.bus, kernel_image)?;

        // 4. Get virtual entry point from ELF header.
        // The kernel is linked to run at this virtual address (e.g., 0xC0000000).
        // Our boot page table maps VA -> PA, so the kernel enters at the
        // correct virtual address with all PC-relative addressing intact.
        // (entry_vaddr used implicitly via load_info below)
        let _entry_vaddr: u32 = load_info.entry;

        // 5. Load initramfs at a page-aligned address after the kernel.
        let (initrd_start, initrd_end) = if let Some(initrd_data) = initramfs {
            let initrd_addr = ((load_info.highest_addr + 0xFFF) & !0xFFF) as u64;
            for (i, &byte) in initrd_data.iter().enumerate() {
                let addr = initrd_addr + i as u64;
                if vm.bus.write_byte(addr, byte).is_err() {
                    break;
                }
            }
            let initrd_end_addr = initrd_addr + initrd_data.len() as u64;
            (Some(initrd_addr), Some(initrd_end_addr))
        } else {
            (None, None)
        };

        // 6. Generate DTB.
        //
        // Set memory node base to PA 0 with full RAM size.
        //
        // The kernel's early_init_dt_scan_memory() reads the first memory node
        // and sets phys_ram_base from its address. With mem_base=0,
        // phys_ram_base=0, and setup_bootmem() reserves the kernel image
        // correctly.
        //
        // The risk: memblock_alloc() might return PA 0 for page tables,
        // overwriting kernel code. But our SATP-change fixup logic in
        // boot_linux() replaces any broken L1 entries with correct megapages.
        let ram_size = actual_ram_size as u64;
        let kernel_phys_end = ((load_info.highest_addr + 0xFFF) & !0xFFF) as u64;
        let mem_base: u64 = 0;
        let mem_size = ram_size;

        // Reserve kernel, initramfs, and DTB regions in mem_rsvmap.
        let mut reserved_regions = vec![(0u64, kernel_phys_end)];
        if let (Some(initrd_addr), Some(initrd_end_addr)) = (initrd_start, initrd_end) {
            let initrd_start_aligned = (initrd_addr as u64) & !0xFFF;
            let initrd_end_aligned = ((initrd_end_addr as u64) + 0xFFF) & !0xFFF;
            reserved_regions.push((
                initrd_start_aligned,
                initrd_end_aligned - initrd_start_aligned,
            ));
        }

        let dtb_config = dtb::DtbConfig {
            ram_base: mem_base,
            ram_size: mem_size,
            initrd_start,
            initrd_end,
            bootargs: bootargs.to_string(),
            reserved_regions,
            ..Default::default()
        };
        let dtb_blob = dtb::generate_dtb(&dtb_config);
        eprintln!(
            "[boot] DTB generated: {} bytes, mem_base=0x{:08X}, mem_size=0x{:08X} ({}MB)",
            dtb_blob.len(),
            mem_base,
            mem_size,
            mem_size / (1024 * 1024)
        );

        let dtb_addr = ((initrd_end.unwrap_or(load_info.highest_addr) + 0xFFF) & !0xFFF) as u64;
        for (i, &byte) in dtb_blob.iter().enumerate() {
            let addr = dtb_addr + i as u64;
            if vm.bus.write_byte(addr, byte).is_err() {
                break;
            }
        }

        // 7. Pre-set DTB pointers that OpenSBI normally initializes.
        //
        // The kernel's setup_arch() reads _dtb_early_va and _dtb_early_pa
        // (NOT initial_boot_params!) to pass to early_init_dt_scan().
        // These are BSS variables -- zero in the binary. OpenSBI sets them
        // before jumping to the kernel. Without OpenSBI, we must set them.
        //
        // Without these, early_init_dt_scan() receives NULL, skips DTB
        // parsing, and memblock_add() is never called. Result: memory.cnt=0
        // and every memblock_alloc() panics with "Failed to allocate".
        let dtb_va = (dtb_addr as u32).wrapping_add(0xC0000000);
        let dtb_early_va_pa: u64 = 0x00801008; // _dtb_early_va at VA 0xC0801008
        let dtb_early_pa_pa: u64 = 0x0080100C; // _dtb_early_pa at VA 0xC080100C
        vm.bus.write_word(dtb_early_va_pa, dtb_va).ok();
        vm.bus.write_word(dtb_early_pa_pa, dtb_addr as u32).ok();
        eprintln!(
            "[boot] Pre-set _dtb_early_va = 0x{:08X}, _dtb_early_pa = 0x{:08X}",
            dtb_va, dtb_addr as u32
        );

        // Also set initial_boot_params for compatibility (some kernel paths
        // read it directly).
        let ibp_phys: u64 = 0x00C7A178;
        vm.bus.write_word(ibp_phys, dtb_addr as u32).ok();

        // 8. Set CPU state for boot.
        vm.cpu.x[10] = 0; // a0 = hartid (0)
        vm.cpu.x[11] = dtb_addr as u32; // a1 = DTB physical address

        // Stack for the kernel (mimics OpenSBI).
        let stack_top: u32 = (actual_ram_size as u64 - 4096) as u32;
        vm.cpu.x[2] = stack_top;

        // Install firmware stubs at low addresses that the kernel expects.
        // Linux's early boot code (before SBI is fully initialized) may jump
        // to these addresses expecting OpenSBI firmware to be present.
        // We place C.JR ra (0x8082) at address 0x12 so the kernel's firmware
        // call returns immediately instead of hitting an illegal instruction.
        vm.bus.write_half(0x12, 0x8082).ok();

        // Allocate a boot page table (4KB, 1024 L1 entries) above kernel + initrd.
        let after_dtb = ((dtb_addr + 4096 + 0xFFF) & !0xFFF) as u64;
        let boot_pt_addr: u64 = after_dtb; // Boot page table physical address

        // Create initial page table for early kernel boot.
        // The kernel's _start code uses virtual addresses (e.g., j 0xC0001084)
        // before setup_vm() creates proper page tables. We need VA == PA + 0xC0000000
        // mapping so these jumps work.
        //
        // With ram_base=0: kernel physical address = vaddr - 0xC0000000
        // So VA 0xC0000000 must map to PA 0x00000000.
        //
        // Sv32 megapage mapping: each L1 entry covers 2MB.
        // L1 index = (vaddr >> 22) & 0x3FF
        // VA 0xC0000000: L1 index = 768 (0x300)
        // VA 0xC1400000: L1 index = 775 (0x307)
        //
        // We create megapage entries mapping:
        //   L1[768] = PA 0x00000000 (VA 0xC0000000-0xC01FFFFF)
        //   L1[769] = PA 0x00200000 (VA 0xC0200000-0xC03FFFFF)
        //   L1[770] = PA 0x00400000 (VA 0xC0400000-0xC05FFFFF)
        //   L1[771] = PA 0x00600000 (VA 0xC0600000-0xC07FFFFF)
        //   L1[772] = PA 0x00800000 (VA 0xC0800000-0xC09FFFFF)
        //   L1[773] = PA 0x00A00000 (VA 0xC0A00000-0xC0BFFFFF)
        //   L1[774] = PA 0x00C00000 (VA 0xC0C00000-0xC0DFFFFF)
        //   L1[775] = PA 0x00E00000 (VA 0xC0E00000-0xC0FFFFFF)
        //
        // Also keep low addresses identity-mapped (for DTB, initramfs, etc.):
        //   L1[0] = PA 0x00000000 (VA 0x00000000-0x001FFFFF) -- identity
        //   L1[1] = PA 0x00200000 (VA 0x00200000-0x003FFFFF) -- identity
        //   etc.
        //
        // Megapage PTE format: V=1, R=1, W=1, X=1, A=1, D=1, U=0 = 0xCF
        // PPN = physical page number (bits[31:10] of PTE)
        let mega_pte_base: u32 = 0x0000_00CF; // V+R+W+X+A+D, U=0

        // Kernel virtual range: L1[768..777] -> PA 0x0..0x01200000 (9 megapages, 36MB)
        // Each Sv32 megapage covers 4MB (PPN[19:10] selects 4MB-aligned base).
        // L1[768+i] maps VA (0xC0000000 + i*4MB) to PA (i*4MB).
        // PTE = (i << 20) | flags  -- PPN[19:10] = i
        for i in 0..9 {
            let l1_idx: u32 = 768 + i;
            let pte = mega_pte_base | (i << 20);
            let addr = boot_pt_addr + (l1_idx as u64) * 4;
            vm.bus.write_word(addr, pte).ok();
        }

        // Low address identity mapping: L1[0..64] -> PA 0x0..0x10000000 (256MB)
        // Each Sv32 megapage covers 4MB.
        // L1[i] maps VA (i*4MB) to PA (i*4MB) -- identity.
        for i in 0..64u32 {
            let pte = mega_pte_base | (i << 20);
            let addr = boot_pt_addr + (i as u64) * 4;
            vm.bus.write_word(addr, pte).ok();
        }

        vm.cpu.privilege = cpu::Privilege::Machine;

        // M-mode trap handler (single MRET instruction).
        // Place at a physical address above the boot page table to avoid overlap.
        let fw_addr: u64 = (boot_pt_addr + 4096 + 0xFFF) & !0xFFF;
        vm.bus.write_word(fw_addr, 0x30200073).ok(); // MRET

        // Set mtvec to our trap handler (physical address).
        vm.cpu.csr.write(crate::riscv::csr::MTVEC, fw_addr as u32);

        // Delegate exceptions to S-mode.
        // IMPORTANT: Do NOT delegate ECALL_S (bit 9) to S-mode!
        // ECALL_S is how the kernel calls SBI (console output, timer, etc.).
        // If delegated, the kernel's own S-mode trap handler processes it
        // instead of reaching our M-mode SBI handler, and all SBI calls silently fail.
        // 0xB109 with bit 9 cleared = 0xA109
        vm.cpu.csr.medeleg = 0xA109;
        vm.cpu.csr.mideleg = 0x222;

        // Set SATP to boot page table (Sv32 mode, PPN = boot_pt_addr / 4096).
        // This enables MMU before entering the kernel so that the kernel's
        // _start code can use virtual addresses (e.g., j 0xC0001084).
        let boot_pt_ppn = (boot_pt_addr / 4096) as u32;
        vm.cpu.csr.satp = (1u32 << 31) | boot_pt_ppn; // Sv32 mode

        // --- Kernel binary patch: fix __pa() root cause ---
        //
        // The 32-bit RV32 Linux kernel computes phys_addr as &_start (VA 0xC0000000)
        // instead of the actual physical address (0x00000000). This makes __pa() a
        // no-op: __pa(x) = x - va_pa_offset = x - 0 = x. ALL PTE corruption,
        // stack corruption, and SATP oscillation stem from this one bug.
        //
        // Fix: NOP the two instructions that write phys_addr and va_pa_offset
        // in setup_vm(), then pre-set the correct values in the kernel_map struct.
        //
        // setup_vm() is in arch/riscv/mm/init.c. The relevant instructions are:
        //   PA 0x0040495E: sw a5, 12(s1)  -- writes &_start (0xC0000000) to phys_addr
        //   PA 0x00404968: sw a1, 20(s1)  -- writes PAGE_OFFSET - _start (0) to va_pa_offset
        //
        // kernel_map struct is at VA 0xC0C79E90 (PA 0x00C79E90), layout:
        //   offset 0: page_offset, 4: virt_addr, 8: virt_offset,
        //   12: phys_addr (need 0), 16: size, 20: va_pa_offset (need 0xC0000000), 24: va_kernel_pa_offset
        //
        // The assertion `slli a5, a5, 10; beqz a5` at PA 0x00404972 still passes
        // because a5=0xC0000000 << 10 overflows to 0 in 32-bit.
        let setup_vm_phys_addr_store: u64 = 0x0040495E; // C.SW a5, 12(s1) (2 bytes)
        let setup_vm_va_kernel_pa_store: u64 = 0x00404964; // SW a6, 24(s1) (4 bytes!)
        let setup_vm_va_pa_offset_store: u64 = 0x00404968; // C.SW a1, 20(s1) (2 bytes)
        let kernel_map_phys: u64 = 0x00C79E90;

        // Verify the instructions match before patching (safety check).
        // The two C.SW instructions are 16-bit; the SW a6,24(s1) is 32-bit.
        let sw_a5_12 = vm.bus.read_half(setup_vm_phys_addr_store).unwrap_or(0);
        let sw_a6_24 = vm.bus.read_word(setup_vm_va_kernel_pa_store).unwrap_or(0);
        let sw_a1_20 = vm.bus.read_half(setup_vm_va_pa_offset_store).unwrap_or(0);
        if sw_a5_12 == 0xC4DC && sw_a6_24 == 0x0104AC23 && sw_a1_20 == 0xC8CC {
            // NOP the sw a5, 12(s1) -- prevents writing wrong phys_addr
            vm.bus.write_half(setup_vm_phys_addr_store, 0x0001).ok(); // C.NOP
                                                                      // NOP the sw a6, 24(s1) -- prevents writing wrong va_kernel_pa_offset
            vm.bus
                .write_word(setup_vm_va_kernel_pa_store, 0x00000013)
                .ok(); // 32-bit NOP
                       // NOP the sw a1, 20(s1) -- prevents writing wrong va_pa_offset
            vm.bus.write_half(setup_vm_va_pa_offset_store, 0x0001).ok(); // C.NOP
                                                                         // Pre-set correct values in kernel_map struct.
                                                                         // phys_addr: the kernel's physical base address. Correct: 0.
                                                                         // va_pa_offset: used as __va_to_pa(va) = va - va_pa_offset for VAs >= virt_addr.
                                                                         //   Correct: 0xC0000000 (PAGE_OFFSET), so VA 0xC0000000 -> PA 0.
                                                                         // va_kernel_pa_offset: used in setup_vm to relocate fixmap function pointers
                                                                         //   (pt_ops[0] and pt_ops[4]). The kernel does: func_ptr + va_kernel_pa_offset.
                                                                         //   Must be 0 so function pointers remain as correct VAs.
                                                                         //   If set to 0xC0000000, the addition wraps (e.g., 0xC04046C8 + 0xC0000000 = 0x804046C8).
            vm.bus.write_word(kernel_map_phys + 12, 0x00000000).ok(); // phys_addr = 0
            vm.bus.write_word(kernel_map_phys + 20, 0xC0000000).ok(); // va_pa_offset = 0xC0000000
            vm.bus.write_word(kernel_map_phys + 24, 0x00000000).ok(); // va_kernel_pa_offset = 0
            eprintln!("[boot] Patched kernel_map: phys_addr=0, va_pa_offset=0xC0000000, va_kernel_pa_offset=0");
        } else {
            eprintln!("[boot] WARNING: kernel patch mismatch! sw_a5_12=0x{:04X} sw_a6_24=0x{:08X} sw_a1_20=0x{:04X} (expected 0xC4DC/0x0104AC23/0xC8CC)", sw_a5_12, sw_a6_24, sw_a1_20);
        }

        // With __pa() fixed for kernel_map, most PTEs are correct.
        // auto_pte_fixup remains enabled for:
        // 1. MMU-level fixup_ppn: translates any remaining virtual PPNs during walks
        // 2. Write interception: fixes demand-paged PTEs created by fault handler
        // 3. SATP scan: fixes PTEs on page table switch
        vm.bus.auto_pte_fixup = true;

        // Pre-populate memblock with kernel image reservation.
        //
        // The kernel's early_init_dt_scan() parses the DTB mem_rsvmap and
        // calls memblock_reserve(0, kernel_phys_end) BEFORE setup_vm().
        // If DTB parsing fails (wrong DTB address, bad format), memblock
        // has no reservations and memblock_alloc() returns PA 0 for page
        // tables, overwriting the kernel image.
        //
        // Pre-populate the reservation directly to ensure it exists even
        // if DTB parsing fails.
        //
        // memblock struct at VA 0xC0803448 (PA 0x00803448):
        //   memory:  cnt@0 max@4 regions@8 (each 8 bytes: base u32 + size u32)
        //   reserved: cnt@48 max@52 regions@56
        //
        // We write to the reserved regions array at the next available slot.
        let memblock_pa: u64 = 0x00803448;
        let res_cnt_addr = memblock_pa + 48;
        let res_cnt = vm.bus.read_word(res_cnt_addr).unwrap_or(0);
        let res_region_offset = 56 + (res_cnt as u64) * 8;
        // Reserve PA 0 to kernel_phys_end (kernel image region)
        vm.bus.write_word(memblock_pa + res_region_offset, 0).ok();           // base = 0
        vm.bus.write_word(memblock_pa + res_region_offset + 4, kernel_phys_end as u32).ok(); // size
        vm.bus.write_word(res_cnt_addr, res_cnt + 1).ok();                   // cnt++
        eprintln!("[boot] Pre-populated memblock reserved: PA 0 - PA 0x{:08X} (slot {})", 
            kernel_phys_end, res_cnt);

        // Pre-set riscv_timebase to 10MHz (10000000).
        // The kernel reads this from the DTB's timebase-frequency property.
        // If DTB parsing fails (e.g., page table not yet set up for DTB VA),
        // riscv_timebase stays 0 and calibrate_delay() produces lpj_fine=0,
        // causing udelay() to loop forever. Pre-setting it ensures udelay
        // works even if DTB parsing is delayed.
        let riscv_timebase_pa: u64 = 0x00C79E54;
        vm.bus.write_word(riscv_timebase_pa, 10_000_000).ok();

        // Pre-set lpj_fine to a reasonable default.
        // lpj_fine = loops_per_jiffy for fine-grained delays.
        // With timebase=10MHz and HZ=100 (CONFIG_HZ), one jiffy = 10ms = 100000 ticks.
        // A rough estimate: 100000 ticks * ~4 instructions/tick = 400000 loops/jiffy.
        // This is an approximation — calibrate_delay() will refine it later.
        // If DTB parsing succeeds, calibrate_delay() overwrites this with the correct value.
        let lpj_fine_pa: u64 = 0x01482060;
        vm.bus.write_word(lpj_fine_pa, 400_000).ok();

        // Enter S-mode via MRET.
        // mepc = VIRTUAL entry point (e.g., 0xC0000000).
        // The boot page table maps VA 0xC0000000 -> PA 0x0, so the kernel
        // executes from the correct virtual address. This is critical because
        // the kernel uses PC-relative addressing (auipc, jal, etc.) that
        // only produces correct results at the linked virtual address.
        // Entering at PA 0 (identity-mapped) causes all auipc calculations
        // to be off by 0xC0000000, leading to wrong GP/SP/TP and eventual
        // boot failure when the kernel returns to a corrupted low address.
        let entry_vaddr: u32 = load_info.entry;
        vm.cpu.csr.mepc = entry_vaddr;
        vm.cpu.csr.mstatus = 1u32 << csr::MSTATUS_MPP_LSB;
        vm.cpu.csr.mstatus |= 1 << csr::MSTATUS_MPIE;
        let restored = vm.cpu.csr.trap_return(cpu::Privilege::Machine);
        vm.cpu.pc = vm.cpu.csr.mepc;
        vm.cpu.privilege = restored;

        Ok((vm, fw_addr, entry_vaddr, dtb_addr))
    }

    /// Boot a RISC-V Linux kernel.
    /// 1. Calculate RAM size from kernel's physical address ranges (p_paddr + memsz)
    /// 2. Create VM with ram_base = 0 (so physical addresses map directly to RAM)
    /// 3. Load kernel ELF at physical addresses (p_paddr)
    /// 4. Convert virtual entry point to physical (entry - p_vaddr + p_paddr)
    /// 5. Load initramfs after the kernel
    /// 6. Generate DTB with ram_base=0, initrd info, bootargs
    /// 7. Enter S-mode via MRET, kernel enables MMU and uses virtual addresses
    /// 8. Execute up to max_instructions steps with trap forwarding
    pub fn boot_linux(
        kernel_image: &[u8],
        initramfs: Option<&[u8]>,
        ram_size_mb: u32,
        max_instructions: u64,
        bootargs: &str,
    ) -> Result<(Self, BootResult), loader::LoadError> {
        let (mut vm, fw_addr, entry, dtb_addr) =
            Self::boot_linux_setup(kernel_image, initramfs, ram_size_mb, bootargs)?;

        // 8. Execute with Rust-level trap forwarding (OpenSBI emulation).
        //
        // The CPU's trap_target_priv() won't delegate M-mode traps to S-mode
        // (medeleg only applies to traps from lower privileges). So when the
        // kernel takes any exception while running in M-mode, the trap goes to
        // our M-mode handler at fw_addr.
        //
        // We intercept this here: after each step, if the CPU landed at our
        // trap handler, we forward ALL exceptions to S-mode (except ECALL_M
        // which is an SBI call). This emulates OpenSBI behavior where most
        // M-mode traps are reflected to S-mode so the kernel's own handlers
        // can process them (page faults, access faults, etc.).
        let fw_addr_u32 = fw_addr as u32;
        let mut count: u64 = 0;
        let mut _trap_counts: [u64; 32] = [0; 32]; // cause code counts
        let mut _mmode_trap_count: u64 = 0;
        let mut _sbi_call_count: u64 = 0;
        let mut _forward_count: u64 = 0;
        let mut _ecall_m_count: u64 = 0;
        let mut _smode_fault_count: u64 = 0;
        let mut _last_unique_pc: u32 = 0;
        let mut _same_pc_count: u64 = 0;
        let mut _trampoline_patched: bool = true; // Boot page table already provides initial mapping
        let mut _panic_breakpoint: bool = false;
        let mut _last_satp: u32 = vm.cpu.csr.satp; // Already set by setup
        let dtb_early_va_expected: u32 = (dtb_addr.wrapping_add(0xC0000000)) as u32;
        let dtb_early_pa_expected: u32 = dtb_addr as u32;
        let dtb_early_va_pa: u64 = 0x00801008;
        let dtb_early_pa_pa: u64 = 0x0080100C;
        let mut _dtb_watchdog_triggers: u32 = 0;
        while count < max_instructions {
            // Check for SBI shutdown request
            if vm.bus.sbi.shutdown_requested {
                break;
            }

            // DTB pointer watchdog: continuously protect _dtb_early_va and _dtb_early_pa.
            //
            // The kernel's setup_vm() pt_ops writes 16 bytes at VA 0xC0801000, which
            // clobbers _dtb_early_va (0xC0801008) and _dtb_early_pa (0xC080100C).
            // This happens AFTER the final SATP switch, so the SATP-change re-set
            // comes too late. Without this watchdog, phys_ram_base stays 0 and the
            // kernel can't allocate memory.
            //
            // Check every 100 instructions (cheap: 2 reads + branch).
            // Stop watching once phys_ram_base is set (DTB parsing succeeded).
            if count % 100 == 0 {
                let prb = vm.bus.read_word(0x00C79EACu64).unwrap_or(0);
                if prb == 0 {
                    // DTB parsing hasn't succeeded yet -- protect the pointers
                    let cur_va = vm.bus.read_word(dtb_early_va_pa).unwrap_or(0);
                    if cur_va != dtb_early_va_expected {
                        vm.bus
                            .write_word(dtb_early_va_pa, dtb_early_va_expected)
                            .ok();
                        vm.bus
                            .write_word(dtb_early_pa_pa, dtb_early_pa_expected)
                            .ok();
                        _dtb_watchdog_triggers += 1;
                        if _dtb_watchdog_triggers <= 5 {
                            eprintln!("[boot] DTB watchdog #{} at count={}: restored _dtb_early_va from 0x{:08X} to 0x{:08X}",
                                _dtb_watchdog_triggers, count, cur_va, dtb_early_va_expected);
                        }
                    }
                } else if _dtb_watchdog_triggers > 0 {
                    eprintln!(
                        "[boot] phys_ram_base set to 0x{:08X} at count={} (DTB parsing succeeded)",
                        prb, count
                    );
                    _dtb_watchdog_triggers = 0;
                }
            }

            // On SATP change, inject identity mappings for device regions.
            //
            // With __pa() now correct, the kernel creates proper page tables for
            // its own code/data. But early boot code accesses device registers
            // at physical addresses (CLINT, PLIC, UART) before the kernel's
            // paging_init() creates those mappings. We inject identity megapages
            // for device regions only -- NOT for RAM (the kernel's linear mapping
            // handles RAM via VA 0xC0000000+).
            {
                let cur_satp = vm.cpu.csr.satp;
                if cur_satp != _last_satp {
                    eprintln!(
                        "[boot] SATP changed: 0x{:08X} -> 0x{:08X} at count={}",
                        _last_satp, cur_satp, count
                    );
                    let mode = (cur_satp >> 31) & 1;
                    if mode == 1 {
                        let ppn = cur_satp & 0x3FFFFF;
                        let pg_dir_phys = (ppn as u64) * 4096;

                        // Device regions that need identity mapping (from DTB):
                        // CLINT: 0x02000000 (L1[8])
                        // PLIC:  0x0C000000 (L1[48])
                        // UART:  0x10000000 (L1[64])
                        // Also map low RAM for DTB/initramfs access: L1[0..5]
                        let device_l1_entries: &[u32] = &[
                            0, 1, 2, 3, 4, 5,  // Low RAM (0x0-0x17FFFFF) for DTB/initramfs
                            8,  // CLINT at 0x02000000
                            48, // PLIC at 0x0C000000
                            64, // UART at 0x10000000
                        ];
                        let identity_pte: u32 = 0x0000_00CF; // V+R+W+X+A+D, U=0

                        for &l1_idx in device_l1_entries {
                            let addr = pg_dir_phys + (l1_idx as u64) * 4;
                            let existing = vm.bus.read_word(addr).unwrap_or(0);
                            if (existing & 1) == 0 {
                                // Only inject if not already mapped
                                let pte = identity_pte | (l1_idx << 20);
                                vm.bus.write_word(addr, pte).ok();
                            }
                        }
                        vm.cpu.tlb.flush_all();
                        eprintln!(
                            "[boot] Injected device identity mappings into pg_dir at PA 0x{:08X}",
                            pg_dir_phys
                        );

                        // Fix broken or missing page table entries in the kernel
                        // linear mapping range (L1[768..777], VA 0xC0000000-0xC1BFFFFF).
                        //
                        // The __pa() bug causes two problems:
                        // 1. Non-leaf L1 entries with PPN=0 (L2 table allocated at PA 0)
                        // 2. Completely unmapped L1 entries (V=0) because the kernel
                        //    couldn't allocate page tables at the wrong address
                        //
                        // Fix: scan all L1 entries in the kernel range and replace
                        // any that are unmapped or have broken L2 pointers (PPN=0)
                        // with correct megapage mappings.
                        //
                        // NOTE: We do NOT force all non-leaf entries to megapages.
                        // The MMU now has a fallback that synthesizes megapage
                        // translations when a non-leaf entry has PPN=0 (broken L2
                        // pointer from uninitialized memblock). Forcing all
                        // non-leaf entries to megapages breaks the kernel's own
                        // demand paging, which may create valid L2 entries later.
                        let mega_flags: u32 = 0x0000_00CF; // V+R+W+X+A+D, U=0
                        let mut fixup_count = 0u32;
                        // Kernel linear mapping: L1[768..777] (9 entries, 36MB)
                        // Also check slightly beyond in case kernel is large
                        for l1_scan in 768..780u32 {
                            let scan_addr = pg_dir_phys + (l1_scan as u64) * 4;
                            let entry = vm.bus.read_word(scan_addr).unwrap_or(0);
                            let is_valid = (entry & 1) != 0;
                            let is_non_leaf = is_valid && (entry & 0xE) == 0;
                            let ppn = (entry >> 10) & 0x3FFFFF;
                            let needs_fix = !is_valid                    // Unmapped
                                || (is_non_leaf && ppn == 0);         // Broken L2 at PA 0
                            if !needs_fix {
                                continue;
                            }
                            fixup_count += 1;
                            // Correct megapage: VA (768+i)*4MB -> PA i*4MB
                            let pa_offset = l1_scan - 768;
                            let fixup_pte = mega_flags | (pa_offset << 20);
                            vm.bus.write_word(scan_addr, fixup_pte).ok();
                            if fixup_count <= 10 {
                                eprintln!(
                                    "[boot] Fixed kernel PT: L1[{}] 0x{:08X} -> megapage 0x{:08X} (PA=0x{:08X})",
                                    l1_scan, entry, fixup_pte, (pa_offset as u64) << 22
                                );
                            }
                        }
                        if fixup_count > 0 {
                            eprintln!("[boot] Fixed {} kernel page table entries", fixup_count);
                            vm.cpu.tlb.flush_all();
                        }

                        // Also verify kernel_map wasn't corrupted by the kernel
                        // re-running setup_vm or other init code.
                        let km_phys: u64 = 0x00C79E90;
                        let km_pa = vm.bus.read_word(km_phys + 12).unwrap_or(0);
                        let km_vapo = vm.bus.read_word(km_phys + 20).unwrap_or(0);
                        let km_vkpo = vm.bus.read_word(km_phys + 24).unwrap_or(0);
                        if km_pa != 0 || km_vapo != 0xC0000000 || km_vkpo != 0x00000000 {
                            eprintln!("[boot] WARNING: kernel_map corrupted! pa=0x{:X} vapo=0x{:X} vkpo=0x{:X}, re-patching", km_pa, km_vapo, km_vkpo);
                            vm.bus.write_word(km_phys + 12, 0x00000000).ok();
                            vm.bus.write_word(km_phys + 20, 0xC0000000).ok();
                            vm.bus.write_word(km_phys + 24, 0x00000000).ok();
                        }

                        // Re-set _dtb_early_va and _dtb_early_pa.
                        // setup_vm()'s pt_ops write (16 bytes at 0xC0801000)
                        // overflows into _dtb_early_va (0xC0801008) and
                        // _dtb_early_pa (0xC080100C). We must restore them
                        // before soc_early_init reads _dtb_early_va.
                        vm.bus
                            .write_word(0x00801008, (dtb_addr.wrapping_add(0xC0000000)) as u32)
                            .ok();
                        vm.bus.write_word(0x0080100C, dtb_addr as u32).ok();
                        eprintln!("[boot] Re-set _dtb_early_va/pa after pt_ops overflow");
                    }
                    _last_satp = cur_satp;
                }
            }

            // Detect if we're sitting at the trap handler from a previous step.
            // This happens when a trap was delivered (mepc/mcause/mtval set,
            // PC jumped to mtvec = fw_addr) and we haven't processed it yet.
            if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == cpu::Privilege::Machine {
                let mcause = vm.cpu.csr.mcause;
                let cause_code = mcause & !(1u32 << 31); // strip interrupt bit

                // ECALL from M-mode (cause 11) is an SBI call -- handle by
                // skipping it (the SBI handler runs elsewhere). All other
                // exceptions should be forwarded to S-mode (OpenSBI behavior),
                // BUT ONLY if they originated from S-mode or U-mode.
                //
                // MPP in mstatus records the privilege level when the trap was
                // taken. If MPP=Machine, the trap came from M-mode code and
                // should NOT be forwarded (real OpenSBI handles these in M-mode;
                // our firmware just skips the faulting instruction).
                // If MPP=Supervisor or MPP=User, the trap came from a lower
                // privilege and OpenSBI would reflect it to S-mode.
                if cause_code != csr::CAUSE_ECALL_M {
                    let mpp = (vm.cpu.csr.mstatus & csr::MSTATUS_MPP_MASK) >> csr::MSTATUS_MPP_LSB;

                    if (cause_code as usize) < 32 {
                        _trap_counts[cause_code as usize] += 1;
                    }
                    if mpp == 3 {
                        _mmode_trap_count += 1;
                    }

                    // Log first few illegal instructions for debugging.
                    if cause_code == 2 && _forward_count < 5 {
                        let mepc_val = vm.cpu.csr.mepc;
                        let stvec_val = vm.cpu.csr.stvec;
                        let inst = vm.bus.read_word(mepc_val as u64).unwrap_or(0);
                        // Also check a few surrounding addresses
                        let inst_m4 = vm
                            .bus
                            .read_word((mepc_val as u64).saturating_sub(4))
                            .unwrap_or(0);
                        let inst_p4 = vm.bus.read_word(mepc_val as u64 + 4).unwrap_or(0);
                        eprintln!("[boot] Illegal instruction #{} at count={}: mepc=0x{:08X} stvec=0x{:08X} satap=0x{:08X}",
                        _forward_count + 1, count, mepc_val, stvec_val, vm.cpu.csr.satp);
                        eprintln!(
                            "[boot]   PA[{}-4]=0x{:08X} PA[{}]=0x{:08X} PA[{}+4]=0x{:08X}",
                            mepc_val, inst_m4, mepc_val, inst, mepc_val, inst_p4
                        );
                        eprintln!(
                            "[boot]   priv={:?} mpp={}",
                            vm.cpu.privilege,
                            (vm.cpu.csr.mstatus & csr::MSTATUS_MPP_MASK) >> csr::MSTATUS_MPP_LSB
                        );
                    }

                    // ECALL_S from S-mode is an SBI call -- handle it directly.
                    if cause_code == csr::CAUSE_ECALL_S {
                        _sbi_call_count += 1;
                        let result = vm.bus.sbi.handle_ecall(
                            vm.cpu.x[17],
                            vm.cpu.x[16],
                            vm.cpu.x[10],
                            vm.cpu.x[11],
                            vm.cpu.x[12],
                            vm.cpu.x[13],
                            vm.cpu.x[14],
                            vm.cpu.x[15],
                            &mut vm.bus.uart,
                            &mut vm.bus.clint,
                        );
                        if let Some((a0_val, a1_val)) = result {
                            vm.cpu.x[10] = a0_val;
                            vm.cpu.x[11] = a1_val;
                        }
                        // Fall through to mepc+4 / MRET to return to S-mode.
                    } else if mpp != 3 {
                        // Trap came from S-mode or U-mode.
                        // Check for demand-paged identity mapping at low addresses.
                        //
                        // The kernel's page table doesn't include identity mappings for
                        // low physical addresses (DTB at ~21MB, initramfs, device regions).
                        // Our SATP-change fixup injects these, but the kernel's own
                        // setup_vm() may clear them. When the kernel faults on a low
                        // address, inject the identity megapage on demand.
                        let fault_addr = vm.cpu.csr.mtval;
                        let is_page_fault =
                            cause_code == 12 || cause_code == 13 || cause_code == 15;
                        if is_page_fault && fault_addr < 0x0200_0000 {
                            let satp = vm.cpu.csr.satp;
                            let pg_dir_ppn = (satp & 0x3FFFFF) as u64;
                            if pg_dir_ppn > 0 {
                                let pg_dir_phys = pg_dir_ppn * 4096;
                                let vpn1 = ((fault_addr >> 22) & 0x3FF) as u64;
                                let l1_addr = pg_dir_phys + vpn1 * 4;
                                let existing = vm.bus.read_word(l1_addr).unwrap_or(0);
                                if (existing & 1) == 0 {
                                    // Inject identity megapage: VA -> PA (same)
                                    let pte: u32 = 0x0000_00CF | ((vpn1 as u32) << 20);
                                    vm.bus.write_word(l1_addr, pte).ok();
                                    vm.cpu.tlb.flush_all();
                                    if _smode_fault_count < 10 {
                                        eprintln!("[boot] On-demand identity map: L1[{}] at PA 0x{:08X} = 0x{:08X} (fault VA=0x{:08X})",
                                            vpn1, l1_addr, pte, fault_addr);
                                    }
                                    // Retry the faulting instruction instead of forwarding
                                    // to S-mode. MRET will return to mepc (the faulting PC).
                                    // Fall through to mepc+4/MRET below.
                                } else {
                                    // L1 entry exists but fault still occurred -- might be
                                    // an L2 entry issue. Forward to S-mode handler.
                                    let stvec = vm.cpu.csr.stvec & !0x3u32;
                                    if stvec != 0 {
                                        vm.cpu.csr.sepc = vm.cpu.csr.mepc;
                                        vm.cpu.csr.scause = mcause;
                                        vm.cpu.csr.stval = vm.cpu.csr.mtval;
                                        let spp = if mpp == 1 { 1u32 } else { 0u32 };
                                        vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus
                                            & !(1 << csr::MSTATUS_SPP))
                                            | (spp << csr::MSTATUS_SPP);
                                        let sie = (vm.cpu.csr.mstatus >> csr::MSTATUS_SIE) & 1;
                                        vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus
                                            & !(1 << csr::MSTATUS_SPIE))
                                            | (sie << csr::MSTATUS_SPIE);
                                        vm.cpu.csr.mstatus &= !(1 << csr::MSTATUS_SIE);
                                        vm.cpu.pc = stvec;
                                        vm.cpu.privilege = cpu::Privilege::Supervisor;
                                        vm.cpu.tlb.flush_all();
                                        _forward_count += 1;
                                        _smode_fault_count += 1;
                                        count += 1;
                                        continue;
                                    }
                                }
                            }
                        } else {
                            // Not a low-address page fault -- forward to S-mode.
                            let stvec = vm.cpu.csr.stvec & !0x3u32; // direct mode
                            if stvec != 0 {
                                // Copy M-mode trap info to S-mode CSRs.
                                vm.cpu.csr.sepc = vm.cpu.csr.mepc;
                                vm.cpu.csr.scause = mcause;
                                vm.cpu.csr.stval = vm.cpu.csr.mtval;

                                // Set S-mode trap entry state in mstatus.
                                // SPP = previous privilege (1=S, 0=U) from MPP.
                                let spp = if mpp == 1 { 1u32 } else { 0u32 };
                                vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus
                                    & !(1 << csr::MSTATUS_SPP))
                                    | (spp << csr::MSTATUS_SPP);
                                // SPIE = SIE (save current SIE), SIE = 0 (disable S interrupts)
                                let sie = (vm.cpu.csr.mstatus >> csr::MSTATUS_SIE) & 1;
                                vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus
                                    & !(1 << csr::MSTATUS_SPIE))
                                    | (sie << csr::MSTATUS_SPIE);
                                vm.cpu.csr.mstatus &= !(1 << csr::MSTATUS_SIE);

                                // For timer interrupts: advance mtimecmp to prevent
                                // immediate re-trap. Without this, MTIP stays pending
                                // after forwarding, and the next vm.step() traps back
                                // to M-mode before the S-mode handler can execute.
                                // The kernel will set its own mtimecmp via SBI_SET_TIMER.
                                if cause_code == csr::INT_MTI {
                                    vm.bus.clint.mtimecmp = vm.bus.clint.mtime + 100_000;
                                }

                                // Jump to S-mode trap vector in Supervisor mode.
                                vm.cpu.pc = stvec;
                                vm.cpu.privilege = cpu::Privilege::Supervisor;

                                // Flush TLB -- address space context changed.
                                vm.cpu.tlb.flush_all();
                                _forward_count += 1;
                                count += 1;
                                continue;
                            }
                            // stvec not set yet -- fall through to skip instruction.
                        }
                        // MPP=3: trap came from M-mode. Fall through to skip.
                        // This handles device probes to unmapped addresses (e.g.,
                        // 0xFFFFFFF0 PLIC/DTB probes) during early M-mode boot.
                    }
                }

                // ECALL_M: Handle as SBI call, then skip instruction.
                if cause_code == csr::CAUSE_ECALL_M {
                    _ecall_m_count += 1;
                    // SBI calling convention: a7=extension, a6=function,
                    // a0..a5=args. Return value in a0 (error), a1 (value).
                    let result = vm.bus.sbi.handle_ecall(
                        vm.cpu.x[17], // a7
                        vm.cpu.x[16], // a6
                        vm.cpu.x[10], // a0
                        vm.cpu.x[11], // a1
                        vm.cpu.x[12], // a2
                        vm.cpu.x[13], // a3
                        vm.cpu.x[14], // a4
                        vm.cpu.x[15], // a5
                        &mut vm.bus.uart,
                        &mut vm.bus.clint,
                    );
                    if let Some((a0_val, a1_val)) = result {
                        vm.cpu.x[10] = a0_val; // a0 = error code
                        vm.cpu.x[11] = a1_val; // a1 = return value
                    }
                }

                // ECALL_M or exception with no stvec:
                // Skip the faulting instruction and return via MRET.
                vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
                // The MRET instruction at fw_addr will execute on the next step,
                // returning to mepc (now faulting_pc + 4).
                // Fall through to normal step processing.
            }

            // Advance CLINT timer and sync hardware interrupt state into MIP.
            // Without this, mtime never advances and timer interrupts never fire,
            // causing the kernel to hang waiting for the first timer tick.
            vm.bus.tick_clint();
            vm.bus.sync_mip(&mut vm.cpu.csr.mip);

            let step_result = vm.step();

            // Breakpoint: capture state when panic() is first entered
            if vm.cpu.pc == 0xC000252E && !_panic_breakpoint {
                _panic_breakpoint = true;
                eprintln!("[boot] *** PANIC ENTERED at count={} ***", count);
                eprintln!("[boot]   RA=0x{:08X} (caller of panic)", vm.cpu.x[1]);
                eprintln!("[boot]   SP=0x{:08X}", vm.cpu.x[2]);
                eprintln!("[boot]   A0=0x{:08X} (fmt string ptr)", vm.cpu.x[10]);
                // Read the format string
                let fmt_va = vm.cpu.x[10];
                if fmt_va >= 0xC0000000 {
                    let fmt_pa = fmt_va - 0xC0000000;
                    let mut chars = Vec::new();
                    for j in 0..200 {
                        let b = vm.bus.read_byte(fmt_pa as u64 + j as u64).unwrap_or(0);
                        if b == 0 {
                            break;
                        }
                        if b >= 0x20 && b < 0x7f {
                            chars.push(b as char);
                        } else {
                            break;
                        }
                    }
                    let s: String = chars.iter().collect();
                    eprintln!("[boot]   FMT: \"{}\"", s);
                }
                // Read allocation args from the panic call chain.
                // __memblock_alloc_or_panic stores size at s0-20 = SP+12.
                // The caller (__memblock_alloc_or_panic) saved s0 = SP+32,
                // and the size was stored at s0-20 = original_sp+12.
                // Since we're inside panic() now, we need to walk back.
                // Instead, read the memblock struct to see available memory.
                let memblock_va = 0xC0803448u64;
                let memblock_pa = memblock_va - 0xC0000000;
                // memblock.memory.cnt (at offset 48) and memblock.reserved.cnt (at offset 52)
                let mem_cnt = vm.bus.read_word(memblock_pa + 48).unwrap_or(0);
                let res_cnt = vm.bus.read_word(memblock_pa + 52).unwrap_or(0);
                eprintln!(
                    "[boot]   memblock: memory.regions={} reserved.regions={}",
                    mem_cnt, res_cnt
                );
                // Read total memory and reserved memory from memblock
                // memblock.memory.regions[0] starts at offset 0 (base, size pairs)
                for ri in 0..mem_cnt.min(4) {
                    let base = vm
                        .bus
                        .read_word(memblock_pa + (ri * 16) as u64)
                        .unwrap_or(0);
                    let size = vm
                        .bus
                        .read_word(memblock_pa + (ri * 16 + 4) as u64)
                        .unwrap_or(0);
                    eprintln!(
                        "[boot]   memory[{}]: base=0x{:08X} size=0x{:08X} ({}MB)",
                        ri,
                        base,
                        size,
                        size / (1024 * 1024)
                    );
                }
                for ri in 0..res_cnt.min(8) {
                    let base = vm
                        .bus
                        .read_word(memblock_pa + (0x200 + ri * 16) as u64)
                        .unwrap_or(0);
                    let size = vm
                        .bus
                        .read_word(memblock_pa + (0x200 + ri * 16 + 4) as u64)
                        .unwrap_or(0);
                    if size > 0 {
                        eprintln!(
                            "[boot]   reserved[{}]: base=0x{:08X} size=0x{:08X} ({}KB)",
                            ri,
                            base,
                            size,
                            size / 1024
                        );
                    }
                }
                // Also read phys_ram_base
                let prb_pa = 0x00C79EACu64;
                let prb = vm.bus.read_word(prb_pa).unwrap_or(0);
                eprintln!("[boot]   phys_ram_base=0x{:08X}", prb);
                // Check _dtb_early_va and _dtb_early_pa (the ACTUAL DTB pointers)
                let deva = vm.bus.read_word(0x00801008).unwrap_or(0);
                let depa = vm.bus.read_word(0x0080100C).unwrap_or(0);
                let expected_dtb_va = dtb_addr.wrapping_add(0xC0000000) as u32;
                eprintln!(
                    "[boot]   _dtb_early_va=0x{:08X} (expect 0x{:08X})",
                    deva, expected_dtb_va
                );
                eprintln!(
                    "[boot]   _dtb_early_pa=0x{:08X} (expect 0x{:08X})",
                    depa, dtb_addr as u32
                );
                // Check initial_boot_params (VA 0xC0C7A178, PA 0x00C7A178)
                let ibp_pa = 0x00C7A178u64;
                let ibp = vm.bus.read_word(ibp_pa).unwrap_or(0);
                eprintln!(
                    "[boot]   initial_boot_params=0x{:08X} (expect DTB PA 0x{:08X})",
                    ibp, dtb_addr as u32
                );
                // Verify DTB at the address initial_boot_params points to
                if ibp != 0 {
                    let dtb_magic_pa = ibp as u64;
                    let dtb_magic = vm.bus.read_word(dtb_magic_pa).unwrap_or(0);
                    // DTB magic 0xD00DFEED in BE = 0xEDFE0DD0 in LE read
                    eprintln!(
                        "[boot]   DTB at 0x{:08X}: magic=0x{:08X} (expect 0xEDFE0DD0)",
                        ibp, dtb_magic
                    );
                }
                // Dump register state
                for i in 0..32 {
                    let name = [
                        "zero", "ra", "sp", "gp", "tp", "t0", "t1", "t2", "s0", "s1", "a0", "a1",
                        "a2", "a3", "a4", "a5", "a6", "a7", "s2", "s3", "s4", "s5", "s6", "s7",
                        "s8", "s9", "s10", "s11", "t3", "t4", "t5", "t6",
                    ][i];
                    eprintln!("[boot]   x{} ({}) = 0x{:08X}", i, name, vm.cpu.x[i]);
                }
            }

            match step_result {
                StepResult::Ok => {}
                StepResult::FetchFault | StepResult::LoadFault | StepResult::StoreFault => {
                    // Log S-mode faults for debugging (first 20).
                    if vm.cpu.privilege == cpu::Privilege::Supervisor && _smode_fault_count < 20 {
                        _smode_fault_count += 1;
                        let fault_type = match step_result {
                            StepResult::FetchFault => "fetch",
                            StepResult::LoadFault => "load",
                            StepResult::StoreFault => "store",
                            _ => unreachable!(),
                        };
                        eprintln!("[boot] S-mode {} fault at count={}: PC=0x{:08X} scause=0x{:08X} sepc=0x{:08X} stval=0x{:08X} stvec=0x{:08X}",
                            fault_type, count, vm.cpu.pc, vm.cpu.csr.scause, vm.cpu.csr.sepc, vm.cpu.csr.stval, vm.cpu.csr.stvec);
                    }
                }
                StepResult::Ebreak => break,
                StepResult::Ecall => {} // ECALL is normal during boot
            }

            // Demand-paging is handled at the MMU level via low_addr_identity_map.
            // No need to patch page tables here.

            // Detect spin loops
            if vm.cpu.pc == _last_unique_pc {
                _same_pc_count += 1;
            } else {
                _last_unique_pc = vm.cpu.pc;
                _same_pc_count = 0;
            }
            if _same_pc_count > 0 && count.is_multiple_of(500_000) {
                eprintln!(
                    "[boot] count={} PC=0x{:08X} priv={:?} mstatus=0x{:08X} same_pc={}",
                    count, vm.cpu.pc, vm.cpu.privilege, vm.cpu.csr.mstatus,                    _same_pc_count
                );
            }
            if count.is_multiple_of(2_000_000) && count > 0 {
                eprintln!("[boot] PROGRESS {}M: PC=0x{:08X} priv={:?} SBI={} fwd={}",
                    count / 1_000_000, vm.cpu.pc, vm.cpu.privilege,
                    _sbi_call_count, _forward_count);
            }
            count += 1;
        }

        eprintln!(
            "[boot] Done: SBI_calls={} ECALL_M={} forwards={} mmode_traps={}",
            _sbi_call_count, _ecall_m_count, _forward_count, _mmode_trap_count
        );
        // Post-boot state
        let prb = vm.bus.read_word(0x00C79EACu64).unwrap_or(0);
        let deva = vm.bus.read_word(0x00801008).unwrap_or(0);
        let depa = vm.bus.read_word(0x0080100C).unwrap_or(0);
        eprintln!("[boot] Post-boot: phys_ram_base=0x{:08X} _dtb_early_va=0x{:08X} _dtb_early_pa=0x{:08X}",
            prb, deva, depa);
        // Check memblock state
        let memblock_va = 0xC0803448u64;
        let memblock_pa = memblock_va - 0xC0000000;
        let mem_cnt = vm.bus.read_word(memblock_pa + 48).unwrap_or(0);
        let res_cnt = vm.bus.read_word(memblock_pa + 52).unwrap_or(0);
        eprintln!(
            "[boot] Post-boot: memblock memory.cnt={} reserved.cnt={}",
            mem_cnt, res_cnt
        );
        for (i, c) in _trap_counts.iter().enumerate() {
            if *c > 0 {
                eprintln!("[boot]   cause {}: {} occurrences", i, c);
            }
        }

        Ok((
            vm,
            BootResult {
                instructions: count,
                entry,
                dtb_addr,
            },
        ))
    }
}
