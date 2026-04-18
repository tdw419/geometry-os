//! Check boot page table entries and VA->PA mapping.
//! Run: cargo run --example boot_pt_check

use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let ir_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_data = std::fs::read(kernel_path).expect("kernel");
    let initramfs_data = std::path::Path::new(ir_path)
        .exists()
        .then(|| std::fs::read(ir_path).unwrap());

    let (mut vm, _fw_addr, _entry, dtb_addr) =
        RiscvVm::boot_linux_setup(&kernel_data, initramfs_data.as_deref(), 128, "console=ttyS0 earlycon=sbi loglevel=8")
            .expect("boot setup failed");

    // Boot page table is at SATP PPN
    let satp = vm.cpu.csr.satp;
    let boot_pt_ppn = satp & 0x3FFFFF;
    let boot_pt_phys = (boot_pt_ppn as u64) * 4096;
    eprintln!("[pt] SATP = 0x{:08X}, boot PT at PA 0x{:08X}", satp, boot_pt_phys);

    // Check L1 entries 768-777 (kernel VA range)
    for l1_idx in 768..778u32 {
        let addr = boot_pt_phys + (l1_idx as u64) * 4;
        let pte = vm.bus.read_word(addr).unwrap_or(0);
        let va_start = (l1_idx as u64) << 22;
        let pa_base = ((pte >> 10) as u64) << 12; // PPN * 4096 for megapage
        eprintln!("[pt] L1[{}] = 0x{:08X} V={} RWX={:03b} PPN=0x{:05X} PA=0x{:08X} -> VA 0x{:08X}-0x{:08X}",
            l1_idx, pte, pte & 1, (pte >> 1) & 7, (pte >> 10) & 0xFFFFF, pa_base,
            va_start | 0xC0000000, va_start + 0x3FFFFF | 0xC0000000);
    }

    // DTB is at VA 0xC1579000, PA 0x01579000
    let dtb_va = 0xC1579000u32;
    let dtb_l1_idx = (dtb_va >> 22) & 0x3FF;
    eprintln!("\n[pt] DTB VA 0x{:08X}: L1 index = {}", dtb_va, dtb_l1_idx);
    let pte = vm.bus.read_word(boot_pt_phys + (dtb_l1_idx as u64) * 4).unwrap_or(0);
    let pa_base = ((pte >> 10) as u64) << 12;
    let expected_pa = 0x01400000u64; // PA base for this VA range
    eprintln!("[pt] L1[{}] PTE = 0x{:08X}, PA base = 0x{:08X} (expected 0x{:08X})",
        dtb_l1_idx, pte, pa_base, expected_pa);
    eprintln!("[pt] DTB offset in 4MB region: 0x{:06X} (0x{:08X} - 0x{:08X})",
        0x01579000 - 0x01400000, 0x01579000, 0x01400000);

    // Now check if the SATP is set BEFORE or AFTER the kernel's own setup_vm changes it
    // The kernel's setup_vm creates its own page tables and switches SATP
    // Our boot page table should be valid BEFORE setup_vm runs

    // Check if DTB PA is actually within the initramfs region
    // initramfs is loaded after the kernel
    eprintln!("\n[pt] DTB PA = 0x{:08X}", dtb_addr as u32);
    eprintln!("[pt] Is DTB within initramfs? Check if DTB data overlaps initramfs");

    // Direct read test
    let dtb_direct = vm.bus.read_word(dtb_addr).unwrap_or(0);
    let dtb_via_va = vm.bus.read_word(dtb_va as u64).unwrap_or(0);
    eprintln!("[pt] Direct read at PA 0x{:08X}: 0x{:08X}", dtb_addr as u32, dtb_direct);
    eprintln!("[pt] Read at VA 0x{:08X}: 0x{:08X}", dtb_va, dtb_via_va);

    // The boot page table has Sv32 mode (SATP bit 31 set)
    // The MMU should translate VA reads through the page table
    // But does read_word use the MMU? Let me check...
    // read_word goes through bus.read_word which goes to RAM directly (no MMU)
    // The MMU is only in the CPU's instruction fetch and load/store paths!
    // So reading via bus.read_word at a VA address would read from the RAM at that VA offset
    // NOT through the MMU translation.
    eprintln!("\n[pt] *** IMPORTANT: bus.read_word does NOT go through MMU! ***");
    eprintln!("[pt] bus.read_word(VA) reads from RAM at VA offset, not the translated PA");
    eprintln!("[pt] So the kernel's DTB read issue is in the CPU's load path, not bus.read_word");
}
