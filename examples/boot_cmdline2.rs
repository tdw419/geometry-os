//! Check if kernel parsed bootargs (saved_command_line).
use geometry_os::riscv::RiscvVm;
fn main() {
    let kernel_data = std::fs::read(".geometry_os/build/linux-6.14/vmlinux").expect("kernel");
    let (mut vm, _fw, _entry, dtb_addr) = RiscvVm::boot_linux_setup(
        &kernel_data, None, 512,
        "console=ttyS0 earlycon=sbi nosmp",
    ).expect("setup");

    // saved_command_line at VA 0xC0C7A048 -> PA 0x00C7A048
    // saved_command_line_len at VA 0xC0C7A04C -> PA 0x00C7A04C
    // boot_command_line at VA 0xC0800018 -> PA 0x00800018
    let scl_pa = 0x00C7A048u64;
    let scl_len_pa = 0x00C7A04Cu64;
    let bcl_pa = 0x00800018u64;

    println!("BEFORE boot:");
    println!("  boot_command_line (PA 0x{:08X}): ptr=0x{:08X}", bcl_pa, vm.bus.read_word(bcl_pa).unwrap_or(0));
    println!("  saved_command_line (PA 0x{:08X}): empty (BSS)", scl_pa);

    // Run 5M instructions
    for _ in 0..5_000_000u64 {
        if vm.bus.sbi.shutdown_requested { break; }
        vm.bus.tick_clint_n(100);
        vm.bus.sync_mip(&mut vm.cpu.csr.mip);
        let _ = vm.step();
    }

    println!("\nAFTER 5M instructions:");
    // Read saved_command_line pointer
    let scl_ptr = vm.bus.read_word(scl_pa).unwrap_or(0);
    let scl_len = vm.bus.read_word(scl_len_pa).unwrap_or(0);
    let bcl_ptr = vm.bus.read_word(bcl_pa).unwrap_or(0);
    println!("  boot_command_line ptr = 0x{:08X}", bcl_ptr);
    println!("  saved_command_line ptr = 0x{:08X}", scl_ptr);
    println!("  saved_command_line_len = {}", scl_len);

    // If the pointer is set, read the string
    if scl_ptr >= 0xC0000000 && scl_len > 0 && scl_len < 512 {
        let scl_data_pa = (scl_ptr - 0xC0000000) as u64;
        let mut s = Vec::new();
        for i in 0..scl_len as usize {
            let b = vm.bus.read_byte(scl_data_pa + i as u64).unwrap_or(0);
            s.push(b);
            if b == 0 { break; }
        }
        let str_s = String::from_utf8_lossy(&s);
        println!("  saved_command_line content: {:?}", str_s);
    } else if scl_ptr > 0 && scl_ptr < 0xC0000000 {
        // Using PA directly
        let mut s = Vec::new();
        for i in 0..scl_len as usize {
            let b = vm.bus.read_byte(scl_ptr as u64 + i as u64).unwrap_or(0);
            s.push(b);
            if b == 0 { break; }
        }
        let str_s = String::from_utf8_lossy(&s);
        println!("  saved_command_line content (PA): {:?}", str_s);
    } else {
        println!("  saved_command_line NOT populated!");
        // saved_command_line is a char array, not a pointer. Read directly.
        let mut s = Vec::new();
        for i in 0..256 {
            let b = vm.bus.read_byte(scl_pa + i as u64).unwrap_or(0);
            s.push(b);
            if b == 0 { break; }
        }
        let str_s = String::from_utf8_lossy(&s);
        println!("  saved_command_line (direct): {:?}", str_s);
    }

    // Also check earlycon struct and sbi state
    println!("\nSBI state:");
    println!("  sbi_debug_console_available = {}", vm.bus.read_word(0x014820A0).unwrap_or(0));
    println!("  ECALLs: {}, SBI output: {} bytes", vm.cpu.ecall_count, vm.bus.sbi.console_output.len());
    println!("  PC: 0x{:08X}", vm.cpu.pc);

    // Check nr_cpu_ids (should be 1 with nosmp)
    // riscv_nohz is not relevant. Let's check __num_online_cpus or cpu_possible_mask
    // Actually, check the DTB cpu node parsing
    println!("\nCPU state:");
    let nr_cpus_pa = 0x00C7A0C8u64; // nr_cpu_ids is typically near other BSS vars
    // Let's try a different approach - check if early_init_dt_scan_chosen was called
    // by looking at chosen_stdout_path
}
