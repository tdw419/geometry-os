// Diagnostic: verify kernel_map patch is applied correctly
use std::fs;
use std::time::Instant;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = fs::read(kernel_path).expect("kernel");
    let initramfs = fs::read(initramfs_path).ok();

    let bootargs = "console=ttyS0 earlycon=sbi panic=5 quiet";
    let (mut vm, result) = geometry_os::riscv::RiscvVm::boot_linux(
        &kernel_image,
        initramfs.as_deref(),
        512,
        5_000_000u64, // Only 5M instructions - should be before 3rd SATP
        bootargs,
    )
    .unwrap();

    println!(
        "Boot: {} instructions, PC: 0x{:08X}",
        result.instructions, vm.cpu.pc
    );

    // Check kernel_map struct BEFORE setup_vm runs
    let km_phys: u64 = 0x00C79E90;
    println!("\nkernel_map at PA 0x{:08X}:", km_phys);
    let km_phys_addr = vm.bus.read_word(km_phys + 12).unwrap_or(0);
    let km_va_pa_offset = vm.bus.read_word(km_phys + 20).unwrap_or(0);
    let km_va_kernel_pa_offset = vm.bus.read_word(km_phys + 24).unwrap_or(0);
    println!("  phys_addr:          0x{:08X}", km_phys_addr);
    println!("  va_pa_offset:       0x{:08X}", km_va_pa_offset);
    println!("  va_kernel_pa_offset:0x{:08X}", km_va_kernel_pa_offset);

    // Check if the patch instructions are in place
    let setup_vm_phys_addr_store: u64 = 0x0040495E;
    let inst = vm.bus.read_half(setup_vm_phys_addr_store).unwrap_or(0);
    println!(
        "\nPatched instruction at PA 0x{:08X}: 0x{:04X} (expect 0x0001 = C.NOP)",
        setup_vm_phys_addr_store, inst
    );

    let setup_vm_va_pa_offset_store: u64 = 0x00404968;
    let inst2 = vm.bus.read_half(setup_vm_va_pa_offset_store).unwrap_or(0);
    println!(
        "Patched instruction at PA 0x{:08X}: 0x{:04X} (expect 0x0001 = C.NOP)",
        setup_vm_va_pa_offset_store, inst2
    );

    // Now run more instructions to get past setup_vm (after 2nd SATP change)
    let (mut vm2, result2) = geometry_os::riscv::RiscvVm::boot_linux(
        &kernel_image,
        initramfs.as_deref(),
        512,
        200_000u64, // Past setup_vm
        bootargs,
    )
    .unwrap();

    println!(
        "\n--- After setup_vm ({} instructions) ---",
        result2.instructions
    );

    // Check kernel_map AFTER setup_vm
    let km_phys_addr2 = vm2.bus.read_word(km_phys + 12).unwrap_or(0);
    let km_va_pa_offset2 = vm2.bus.read_word(km_phys + 20).unwrap_or(0);
    let km_va_kernel_pa_offset2 = vm2.bus.read_word(km_phys + 24).unwrap_or(0);
    println!("kernel_map after setup_vm:");
    println!("  phys_addr:          0x{:08X}", km_phys_addr2);
    println!("  va_pa_offset:       0x{:08X}", km_va_pa_offset2);
    println!("  va_kernel_pa_offset:0x{:08X}", km_va_kernel_pa_offset2);

    // Check the page tables that setup_vm created
    // The kernel's L1 page table is at trampoline_pg_dir or early_pg_dir
    // Let's check what the early_pg_dir L1 entries look like
    let early_pg_dir_pa = 0x00802000u64;
    println!("\nearly_pg_dir at PA 0x{:08X}:", early_pg_dir_pa);
    for i in 768..775 {
        let entry = vm2
            .bus
            .read_word(early_pg_dir_pa + (i as u64) * 4)
            .unwrap_or(0);
        if entry != 0 {
            let ppn = (entry >> 10) & 0x3FFFFF;
            let is_leaf = (entry & 0xE) != 0;
            let ppn_hi = (ppn >> 10) & 0xFFF;
            println!(
                "  L1[{}] = 0x{:08X} PPN=0x{:06X} ppn_hi={} leaf={}",
                i, entry, ppn, ppn_hi, is_leaf
            );
        }
    }

    // For L1[768] (kernel base), check what PA it maps to
    // Should map VA 0xC0000000 -> PA 0x00000000
    let l1_768 = vm2.bus.read_word(early_pg_dir_pa + 768 * 4).unwrap_or(0);
    let is_leaf_768 = (l1_768 & 0xE) != 0;
    if is_leaf_768 {
        let ppn = (l1_768 >> 10) & 0x3FFFFF;
        let ppn_hi = (ppn >> 10) & 0xFFF;
        let pa = (ppn_hi as u64) << 22;
        println!(
            "\n  L1[768] maps VA 0xC0000000 -> PA 0x{:08X} (ppn_hi={})",
            pa, ppn_hi
        );
        println!("  Expected: PA 0x00000000 (ppn_hi=0)");
        if ppn_hi != 0 {
            println!("  *** WRONG! The kernel's __pa() is still returning VAs ***");
        }
    }
}
