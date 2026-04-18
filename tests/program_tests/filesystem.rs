use super::*;

#[test]
fn test_open_read_write_close_opcodes() {
    let (mut vm, _dir) = vm_with_vfs();

    // Write filename "test.txt" at RAM address 0x1000
    write_string_to_ram(&mut vm, 0x1000, "test.txt");

    // Write data "ABCD" at RAM address 0x1100
    for (i, ch) in b"ABCD".iter().enumerate() {
        vm.ram[0x1100 + i] = *ch as u32;
    }

    // Program:
    //   OPEN r1, r2      -- r1=&path, r2=mode(1=write)
    //   WRITE r0, r3, r4 -- r0=fd(from open), r3=&buf, r4=len
    //   CLOSE r0          -- close fd
    //   OPEN r1, r5      -- r5=mode(0=read)
    //   READ r0, r6, r7  -- r6=&readbuf(0x1200), r7=len(4)
    //   CLOSE r0
    //   HALT

    // Set registers
    vm.regs[1] = 0x1000; // path address
    vm.regs[2] = 1; // write mode
    vm.regs[3] = 0x1100; // write buffer
    vm.regs[4] = 4; // write length
    vm.regs[5] = 0; // read mode
    vm.regs[6] = 0x1200; // read buffer
    vm.regs[7] = 4; // read length

    let program = vec![
        0x54, 1, 2, // OPEN r1, r2  -> r0 = fd
        0x56, 0, 3, 4, // WRITE r0, r3, r4  -> r0 = bytes written
        0x57, 0, // CLOSE r0
        0x54, 1, 5, // OPEN r1, r5  -> r0 = fd (read mode)
        0x55, 0, 6, 7, // READ r0, r6, r7  -> r0 = bytes read
        0x57, 0,    // CLOSE r0
        0x00, // HALT
    ];

    load_and_run(&mut vm, &program, 100);

    assert!(vm.halted, "VM should halt");

    // Verify the read-back data matches what was written
    assert_eq!(vm.ram[0x1200] & 0xFF, b'A' as u32, "byte 0 should be A");
    assert_eq!(vm.ram[0x1201] & 0xFF, b'B' as u32, "byte 1 should be B");
    assert_eq!(vm.ram[0x1202] & 0xFF, b'C' as u32, "byte 2 should be C");
    assert_eq!(vm.ram[0x1203] & 0xFF, b'D' as u32, "byte 3 should be D");
}

#[test]
fn test_open_nonexistent_file_returns_error() {
    let (mut vm, _dir) = vm_with_vfs();

    write_string_to_ram(&mut vm, 0x1000, "nonexistent.txt");
    vm.regs[1] = 0x1000;
    vm.regs[2] = 0; // read mode

    let program = vec![
        0x54, 1, 2,    // OPEN r1, r2
        0x00, // HALT
    ];

    load_and_run(&mut vm, &program, 100);

    // r0 should be FD_ERROR (0xFFFFFFFF) since file doesn't exist
    assert_eq!(
        vm.regs[0], 0xFFFFFFFF,
        "opening nonexistent file should return error"
    );
}

#[test]
fn test_close_invalid_fd_returns_error() {
    let (mut vm, _dir) = vm_with_vfs();
    vm.regs[0] = 99; // invalid fd

    let program = vec![
        0x57, 0, // CLOSE r0 (invalid fd)
        0x00,
    ];

    load_and_run(&mut vm, &program, 100);
    assert_eq!(
        vm.regs[0], 0xFFFFFFFF,
        "closing invalid fd should return error"
    );
}

#[test]
fn test_seek_opcode() {
    let (mut vm, _dir) = vm_with_vfs();

    write_string_to_ram(&mut vm, 0x1000, "seektest.txt");
    vm.regs[1] = 0x1000;
    vm.regs[2] = 1; // write mode

    // Write "ABCDEFGH"
    for (i, ch) in b"ABCDEFGH".iter().enumerate() {
        vm.ram[0x1100 + i] = *ch as u32;
    }
    vm.regs[3] = 0x1100;
    vm.regs[4] = 8;

    // Program: open, write, close, open(read), seek(4, SET), read(2), close, halt
    vm.regs[5] = 0; // read mode
    vm.regs[6] = 0x1200; // read buf
    vm.regs[7] = 2; // read 2 bytes
    vm.regs[8] = 4; // seek offset
    vm.regs[9] = 0; // SEEK_SET

    let program = vec![
        0x54, 1, 2, // OPEN r1, r2 (write)
        0x56, 0, 3, 4, // WRITE r0, r3, r4
        0x57, 0, // CLOSE r0
        0x54, 1, 5, // OPEN r1, r5 (read)
        0x58, 0, 8, 9, // SEEK r0, r8, r9 (offset=4, whence=SET)
        0x55, 0, 6, 7, // READ r0, r6, r7
        0x57, 0,    // CLOSE r0
        0x00, // HALT
    ];

    load_and_run(&mut vm, &program, 100);

    assert!(vm.halted);
    // Should read "EF" (bytes at offset 4 and 5)
    assert_eq!(vm.ram[0x1200] & 0xFF, b'E' as u32);
    assert_eq!(vm.ram[0x1201] & 0xFF, b'F' as u32);
}

#[test]
fn test_ls_opcode() {
    let (mut vm, _dir) = vm_with_vfs();

    // Create some files in the VFS directory
    std::fs::write(vm.vfs.base_dir.join("aaa.txt"), b"").expect("filesystem operation failed");
    std::fs::write(vm.vfs.base_dir.join("bbb.txt"), b"").expect("filesystem operation failed");

    vm.regs[1] = 0x2000; // buffer for listing

    let program = vec![
        0x59, 1, // LS r1
        0x00,
    ];

    load_and_run(&mut vm, &program, 100);

    assert!(vm.halted);
    // r0 should be 2 (two files)
    assert_eq!(vm.regs[0], 2, "LS should return 2 entries");

    // Parse the entries from RAM
    let mut entries = Vec::new();
    let mut addr = 0x2000;
    for _ in 0..10 {
        let ch = (vm.ram[addr] & 0xFF) as u8;
        if ch == 0 {
            break;
        }
        let mut name = String::new();
        loop {
            let c = (vm.ram[addr] & 0xFF) as u8;
            if c == 0 {
                addr += 1;
                break;
            }
            name.push(c as char);
            addr += 1;
        }
        entries.push(name);
    }
    entries.sort();
    assert_eq!(entries, vec!["aaa.txt", "bbb.txt"]);
}

#[test]
fn test_filesystem_opcodes_assemble() {
    use geometry_os::assembler::assemble;

    let src = "OPEN r1, r2
READ r0, r3, r4
WRITE r0, r3, r4
CLOSE r0
SEEK r0, r1, r2
LS r3
HALT";
    let result = assemble(src, 0).expect("assembly should succeed");
    assert_eq!(result.pixels[0], 0x54); // OPEN
    assert_eq!(result.pixels[3], 0x55); // READ
    assert_eq!(result.pixels[7], 0x56); // WRITE
    assert_eq!(result.pixels[11], 0x57); // CLOSE
    assert_eq!(result.pixels[13], 0x58); // SEEK
    assert_eq!(result.pixels[17], 0x59); // LS
    assert_eq!(result.pixels[19], 0x00); // HALT
}

#[test]
fn test_vfs_path_traversal_blocked() {
    let (mut vm, _dir) = vm_with_vfs();

    // Try to open a file with path traversal
    write_string_to_ram(&mut vm, 0x1000, "../../../etc/passwd");
    vm.regs[1] = 0x1000;
    vm.regs[2] = 0;

    let program = vec![
        0x54, 1, 2, // OPEN r1, r2
        0x00,
    ];

    load_and_run(&mut vm, &program, 100);
    assert_eq!(vm.regs[0], 0xFFFFFFFF, "path traversal should be blocked");
}

#[test]
fn test_cat_asm_assembles() {
    use geometry_os::assembler::assemble;
    let source = std::fs::read_to_string("programs/cat.asm").expect("cat.asm should exist");
    let result = assemble(&source, 0);
    assert!(
        result.is_ok(),
        "cat.asm should assemble: {:?}",
        result.err()
    );
}

#[test]
fn test_cat_asm_reads_and_displays_file() {
    let (mut vm, _dir) = vm_with_vfs();

    // Create hello.txt in the VFS directory
    std::fs::write(vm.vfs.base_dir.join("hello.txt"), b"Hello from file!")
        .expect("filesystem operation failed");

    use geometry_os::assembler::assemble;
    let source = std::fs::read_to_string("programs/cat.asm").expect("cat.asm should exist");
    let result = assemble(&source, 0).expect("cat.asm should assemble");

    // Load bytecode into RAM at 0x100 (after the filename data at 0x1000)
    for (i, &word) in result.pixels.iter().enumerate() {
        if 0x0100 + i < vm.ram.len() {
            vm.ram[0x0100 + i] = word;
        }
    }
    vm.pc = 0x0100;

    // Run enough steps to complete
    for _ in 0..1000 {
        if vm.halted {
            break;
        }
        vm.step();
    }

    assert!(vm.halted, "cat.asm should halt");

    // Verify the file content was read into the buffer at 0x2000
    let expected = b"Hello from file!";
    for (i, &ch) in expected.iter().enumerate() {
        assert_eq!(
            vm.ram[0x2000 + i] & 0xFF,
            ch as u32,
            "buffer[{}] should be '{}' but got '{}'",
            i,
            ch as char,
            (vm.ram[0x2000 + i] & 0xFF) as u8 as char
        );
    }

    // Verify null terminator after content
    assert_eq!(
        vm.ram[0x2000 + expected.len()] & 0xFF,
        0,
        "should be null-terminated"
    );
}

#[test]
fn test_cat_asm_nonexistent_file_halts_cleanly() {
    let (mut vm, _dir) = vm_with_vfs();

    // Don't create any file -- cat.asm should handle open error gracefully
    use geometry_os::assembler::assemble;
    let source = std::fs::read_to_string("programs/cat.asm").expect("cat.asm should exist");
    let result = assemble(&source, 0).expect("cat.asm should assemble");

    for (i, &word) in result.pixels.iter().enumerate() {
        if 0x0100 + i < vm.ram.len() {
            vm.ram[0x0100 + i] = word;
        }
    }
    vm.pc = 0x0100;

    for _ in 0..1000 {
        if vm.halted {
            break;
        }
        vm.step();
    }

    // Should halt gracefully (not crash)
    assert!(
        vm.halted,
        "cat.asm should halt even when file doesn't exist"
    );
}
