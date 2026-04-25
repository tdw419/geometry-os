use super::*;

// ── Phase 125: Shell VFS Integration tests ─────────────────────────

/// Test that the shell's edit command can read a VFS file
/// Creates a file in VFS, runs the edit portion of the shell, verifies content was read
#[test]
fn test_shell_edit_reads_file() {
    let (mut vm, _dir) = vm_with_vfs();

    // Create a test file in the VFS directory
    std::fs::write(
        vm.vfs.base_dir.join("testdoc.txt"),
        b"Hello World\nLine Two",
    )
    .expect("filesystem operation failed");

    // Assemble a minimal program that simulates what shell's do_edit does:
    // 1. Write filename to RAM at 0x0600
    // 2. Open file for reading
    // 3. Read content into buffer at 0x2000
    // 4. Close file
    // 5. Halt
    use geometry_os::assembler::assemble;

    let source = r#"
    ; Write filename "testdoc.txt" to RAM at 0x0600
    LDI r9, 0x0600
    LDI r0, 116       ; t
    STORE r9, r0
    LDI r9, 0x0601
    LDI r0, 101       ; e
    STORE r9, r0
    LDI r9, 0x0602
    LDI r0, 115       ; s
    STORE r9, r0
    LDI r9, 0x0603
    LDI r0, 116       ; t
    STORE r9, r0
    LDI r9, 0x0604
    LDI r0, 100       ; d
    STORE r9, r0
    LDI r9, 0x0605
    LDI r0, 111       ; o
    STORE r9, r0
    LDI r9, 0x0606
    LDI r0, 99        ; c
    STORE r9, r0
    LDI r9, 0x0607
    LDI r0, 46        ; .
    STORE r9, r0
    LDI r9, 0x0608
    LDI r0, 116       ; t
    STORE r9, r0
    LDI r9, 0x0609
    LDI r0, 120       ; x
    STORE r9, r0
    LDI r9, 0x060A
    LDI r0, 116       ; t
    STORE r9, r0
    LDI r9, 0x060B
    LDI r0, 0         ; null
    STORE r9, r0

    ; Open file for reading
    LDI r1, 0x0600       ; filename addr
    LDI r2, 0            ; mode = read
    OPEN r1, r2           ; r0 = fd

    MOV r5, r0            ; save fd BEFORE CMP clobbers r0

    ; Check fd is valid
    LDI r1, 0xFFFFFFFF
    CMP r5, r1
    JZ r0, open_fail

    ; Read content into buffer at 0x2000
    LDI r3, 0x2000
    LDI r4, 200
    READ r5, r3, r4       ; r0 = bytes read

    ; Close file
    CLOSE r5

    HALT

open_fail:
    LDI r0, 0xDEAD
    HALT
"#;

    let result = assemble(source, 0).expect("assembly should succeed");

    for (i, &word) in result.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = word;
        }
    }
    vm.pc = 0;
    vm.halted = false;

    for _ in 0..50_000 {
        if !vm.step() {
            break;
        }
    }

    assert!(vm.halted, "program should halt");
    assert_ne!(vm.regs[0], 0xDEAD, "file open should succeed");

    // Verify "Hello World" was read into the buffer at 0x2000
    let expected = b"Hello World\nLine Two";
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
}

/// Test that the shell's save command can write data to a VFS file
#[test]
fn test_shell_save_writes_file() {
    let (mut vm, _dir) = vm_with_vfs();

    // Pre-populate buffer at 0x2000 with content to save
    let content = b"Saved by shell!";
    for (i, &ch) in content.iter().enumerate() {
        vm.ram[0x2000 + i] = ch as u32;
    }
    vm.ram[0x2000 + content.len()] = 0; // null terminate

    // Write filename to RAM at 0x0600
    write_string(&mut vm.ram, 0x0600, "output.txt");

    use geometry_os::assembler::assemble;

    let source = r#"
    ; Open file for writing (mode 1)
    LDI r1, 0x0600       ; filename addr
    LDI r2, 1            ; mode = write
    OPEN r1, r2           ; r0 = fd

    MOV r5, r0            ; save fd BEFORE CMP clobbers r0

    ; Check fd
    LDI r1, 0xFFFFFFFF
    CMP r5, r1
    JZ r0, save_fail

    ; Calculate content length (scan for null)
    LDI r10, 0x2000
    LDI r11, 0
scan_loop:
    LOAD r0, r10
    JZ r0, scan_done
    LDI r1, 1
    ADD r10, r1
    ADD r11, r1
    JMP scan_loop

scan_done:
    ; Write content
    MOV r1, r5             ; fd
    LDI r2, 0x2000         ; buf addr
    MOV r3, r11            ; length
    WRITE r1, r2, r3

    CLOSE r5
    HALT

save_fail:
    LDI r0, 0xDEAD
    HALT
"#;

    let result = assemble(source, 0).expect("assembly should succeed");

    for (i, &word) in result.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = word;
        }
    }
    vm.pc = 0;
    vm.halted = false;

    for _ in 0..50_000 {
        if !vm.step() {
            break;
        }
    }

    assert!(vm.halted, "program should halt");
    assert_ne!(vm.regs[0], 0xDEAD, "save should succeed");

    // Verify the file was created on disk
    let saved = std::fs::read(vm.vfs.base_dir.join("output.txt")).expect("output.txt should exist");
    let saved_str = String::from_utf8_lossy(&saved);
    assert!(
        saved_str.contains("Saved by shell!"),
        "file should contain saved content, got: {}",
        saved_str
    );
}

/// Test edit then save roundtrip: write a file, edit (read) it, modify buffer, save to new file
#[test]
fn test_shell_edit_save_roundtrip() {
    let (mut vm, _dir) = vm_with_vfs();

    // Create source file
    std::fs::write(vm.vfs.base_dir.join("source.txt"), b"Original content")
        .expect("filesystem operation failed");

    use geometry_os::assembler::assemble;

    let source = r#"
    ; Write "source.txt" at 0x0600
    LDI r9, 0x0600
    LDI r0, 115       ; s
    STORE r9, r0
    LDI r9, 0x0601
    LDI r0, 111       ; o
    STORE r9, r0
    LDI r9, 0x0602
    LDI r0, 117       ; u
    STORE r9, r0
    LDI r9, 0x0603
    LDI r0, 114       ; r
    STORE r9, r0
    LDI r9, 0x0604
    LDI r0, 99        ; c
    STORE r9, r0
    LDI r9, 0x0605
    LDI r0, 101       ; e
    STORE r9, r0
    LDI r9, 0x0606
    LDI r0, 46        ; .
    STORE r9, r0
    LDI r9, 0x0607
    LDI r0, 116       ; t
    STORE r9, r0
    LDI r9, 0x0608
    LDI r0, 120       ; x
    STORE r9, r0
    LDI r9, 0x0609
    LDI r0, 116       ; t
    STORE r9, r0
    LDI r9, 0x060A
    LDI r0, 0
    STORE r9, r0

    ; Read source file
    LDI r1, 0x0600
    LDI r2, 0
    OPEN r1, r2
    MOV r5, r0            ; save fd BEFORE CMP clobbers r0
    LDI r1, 0xFFFFFFFF
    CMP r5, r1
    JZ r0, fail
    LDI r3, 0x2000
    LDI r4, 200
    READ r5, r3, r4
    CLOSE r5

    ; Write "backup.txt" at 0x0600
    LDI r9, 0x0600
    LDI r0, 98        ; b
    STORE r9, r0
    LDI r9, 0x0601
    LDI r0, 97        ; a
    STORE r9, r0
    LDI r9, 0x0602
    LDI r0, 99        ; c
    STORE r9, r0
    LDI r9, 0x0603
    LDI r0, 107       ; k
    STORE r9, r0
    LDI r9, 0x0604
    LDI r0, 117       ; u
    STORE r9, r0
    LDI r9, 0x0605
    LDI r0, 112       ; p
    STORE r9, r0
    LDI r9, 0x0606
    LDI r0, 46        ; .
    STORE r9, r0
    LDI r9, 0x0607
    LDI r0, 116       ; t
    STORE r9, r0
    LDI r9, 0x0608
    LDI r0, 120       ; x
    STORE r9, r0
    LDI r9, 0x0609
    LDI r0, 116       ; t
    STORE r9, r0
    LDI r9, 0x060A
    LDI r0, 0
    STORE r9, r0

    ; Save to backup file
    LDI r1, 0x0600
    LDI r2, 1            ; write mode
    OPEN r1, r2
    MOV r5, r0            ; save fd BEFORE CMP clobbers r0
    LDI r1, 0xFFFFFFFF
    CMP r5, r1
    JZ r0, fail
    ; Scan length
    LDI r10, 0x2000
    LDI r11, 0
scan2:
    LOAD r0, r10
    JZ r0, scan2_done
    LDI r1, 1
    ADD r10, r1
    ADD r11, r1
    JMP scan2
scan2_done:
    MOV r1, r5
    LDI r2, 0x2000
    MOV r3, r11
    WRITE r1, r2, r3
    CLOSE r5
    HALT

fail:
    LDI r0, 0xDEAD
    HALT
"#;

    let result = assemble(source, 0).expect("assembly should succeed");

    for (i, &word) in result.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = word;
        }
    }
    vm.pc = 0;
    vm.halted = false;

    for _ in 0..100_000 {
        if !vm.step() {
            break;
        }
    }

    assert!(vm.halted, "program should halt");
    assert_ne!(vm.regs[0], 0xDEAD, "operations should succeed");

    // Verify backup file was created with same content
    let backup =
        std::fs::read(vm.vfs.base_dir.join("backup.txt")).expect("backup.txt should exist");
    let backup_str = String::from_utf8_lossy(&backup);
    assert!(
        backup_str.contains("Original content"),
        "backup should contain original content, got: {}",
        backup_str
    );
}

/// Test that the shell's ls command lists VFS files
#[test]
fn test_shell_ls_lists_vfs_files() {
    let (mut vm, _dir) = vm_with_vfs();

    // Create test files
    std::fs::write(vm.vfs.base_dir.join("alpha.txt"), b"file A").expect("write failed");
    std::fs::write(vm.vfs.base_dir.join("beta.dat"), b"file B").expect("write failed");

    use geometry_os::assembler::assemble;

    // Minimal program that calls LS opcode
    let source = r#"
    LDI r1, 0x0800
    LS r1
    HALT
"#;

    let result = assemble(source, 0).expect("assembly should succeed");

    for (i, &word) in result.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = word;
        }
    }
    vm.pc = 0;
    vm.halted = false;

    for _ in 0..10_000 {
        if !vm.step() {
            break;
        }
    }

    assert!(vm.halted, "program should halt");

    // r0 should contain the number of files found (at least 2)
    assert!(
        vm.regs[0] >= 2,
        "LS should find at least 2 files, got {}",
        vm.regs[0]
    );

    // Verify filenames appear in the listing buffer
    // LS writes null-terminated entries one after another; double-null terminates
    let mut entries = Vec::new();
    let mut addr = 0x0800;
    for _ in 0..20 {
        let ch = vm.ram[addr] & 0xFF;
        if ch == 0 {
            break;
        }
        let mut name = String::new();
        loop {
            let c = vm.ram[addr] & 0xFF;
            if c == 0 {
                addr += 1;
                break;
            }
            name.push(c as u8 as char);
            addr += 1;
        }
        entries.push(name);
    }

    assert!(
        entries.iter().any(|e| e == "alpha.txt"),
        "listing should contain alpha.txt, got: {:?}",
        entries
    );
    assert!(
        entries.iter().any(|e| e == "beta.dat"),
        "listing should contain beta.dat, got: {:?}",
        entries
    );
}

/// Test that the shell's rm command deletes a VFS file via UNLINK
#[test]
fn test_shell_rm_deletes_file() {
    let (mut vm, _dir) = vm_with_vfs();

    // Create a file to delete
    std::fs::write(vm.vfs.base_dir.join("todelete.txt"), b"delete me").expect("write failed");

    // Write filename to RAM
    write_string(&mut vm.ram, 0x0600, "todelete.txt");

    use geometry_os::assembler::assemble;

    let source = r#"
    ; Copy filename to clean buffer at 0x1600
    LDI r13, 0x0600
    LDI r14, 0x1600
rm_copy:
    LOAD r0, r13
    JZ r0, rm_copy_done
    STORE r14, r0
    LDI r0, 1
    ADD r13, r0
    ADD r14, r0
    JMP rm_copy
rm_copy_done:
    LDI r0, 0
    STORE r14, r0

    ; UNLINK the file
    LDI r1, 0x1600
    UNLINK r1

    HALT
"#;

    let result = assemble(source, 0).expect("assembly should succeed");

    for (i, &word) in result.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = word;
        }
    }
    vm.pc = 0;
    vm.halted = false;

    for _ in 0..10_000 {
        if !vm.step() {
            break;
        }
    }

    assert!(vm.halted, "program should halt");

    // File should be deleted from disk
    assert!(
        !vm.vfs.base_dir.join("todelete.txt").exists(),
        "todelete.txt should be deleted"
    );
}

/// Test that the shell's cp command copies a VFS file via FCOPY
#[test]
fn test_shell_cp_copies_file() {
    let (mut vm, _dir) = vm_with_vfs();

    // Create source file
    std::fs::write(vm.vfs.base_dir.join("original.txt"), b"copy me!").expect("write failed");

    // Write source filename to 0x0600, dest filename to 0x0800
    write_string(&mut vm.ram, 0x0600, "original.txt");
    write_string(&mut vm.ram, 0x0800, "copied.txt");

    use geometry_os::assembler::assemble;

    let source = r#"
    ; FCOPY src_name_reg, dst_name_reg
    LDI r1, 0x0600
    LDI r2, 0x0800
    FCOPY r1, r2
    HALT
"#;

    let result = assemble(source, 0).expect("assembly should succeed");

    for (i, &word) in result.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = word;
        }
    }
    vm.pc = 0;
    vm.halted = false;

    for _ in 0..10_000 {
        if !vm.step() {
            break;
        }
    }

    assert!(vm.halted, "program should halt");

    // Verify the copied file exists with same content
    let copied =
        std::fs::read(vm.vfs.base_dir.join("copied.txt")).expect("copied.txt should exist");
    let copied_str = String::from_utf8_lossy(&copied);
    assert!(
        copied_str.contains("copy me!"),
        "copied file should contain original content, got: {}",
        copied_str
    );

    // Original should still exist
    assert!(
        vm.vfs.base_dir.join("original.txt").exists(),
        "original file should still exist"
    );
}
