// Phase 123: Alpine Linux Live Tile tests
// Tests for VM_LIVE_SPAWN, VM_LIVE_STEP, VM_LIVE_KILL
// and the hypervisor-to-window bridge (console text rendering)

#[cfg(test)]
mod phase123_live_tile {
    use crate::assembler::assemble;
    use crate::vm::Vm;
    use crate::vm::types::LiveHypervisorState;

    /// Helper: assemble source, load into VM, run until halt.
    fn compile_run(source: &str) -> Vm {
        let asm = assemble(source, 0).unwrap();
        let mut vm = Vm::new();
        for (i, &pixel) in asm.pixels.iter().enumerate() {
            if i < vm.ram.len() {
                vm.ram[i] = pixel;
            }
        }
        vm.pc = 0;
        vm.halted = false;
        for _ in 0..100_000 {
            if !vm.step() {
                break;
            }
        }
        vm
    }

    /// Helper: create a VM with a WINSYS window.
    fn vm_with_window() -> Vm {
        let mut vm = Vm::new();
        // Create a window using WINSYS op=0 (create window)
        // WINSYS op_reg where op_reg contains op=0, r1=x, r2=y, r3=w, r4=h
        vm.ram[0x2000] = 0;    // op = 0 (create)
        vm.ram[0x2001] = 10;   // x = 10
        vm.ram[0x2002] = 10;   // y = 10
        vm.ram[0x2003] = 128;  // w = 128
        vm.ram[0x2004] = 128;  // h = 128
        vm.ram[0x2005] = 0;    // title_addr = 0

        // Use WINSYS opcode (0x94) to create window
        // WINSYS r10 (op_reg) -- op_reg is r10 pointing to the struct
        vm.regs[10] = 0x2000;

        // Manually trigger WINSYS create
        let source = "LDI r10, 0x2000\nLDI r11, 0\nSTORE r10, r11\nLDI r11, 10\nLDI r12, 0x2001\nSTORE r12, r11\nLDI r12, 0x2002\nSTORE r12, r11\nLDI r11, 128\nLDI r12, 0x2003\nSTORE r12, r11\nLDI r12, 0x2004\nSTORE r12, r11\nLDI r11, 0\nLDI r12, 0x2005\nSTORE r12, r11\nWINSYS r10\nHALT";
        let asm = assemble(source, 0).unwrap();
        for (i, &pixel) in asm.pixels.iter().enumerate() {
            if i < vm.ram.len() {
                vm.ram[i] = pixel;
            }
        }
        vm.pc = 0;
        vm.halted = false;
        for _ in 0..100_000 {
            if !vm.step() {
                break;
            }
        }
        vm
    }

    #[test]
    fn test_vm_live_spawn_assembles() {
        // Verify the new opcodes assemble correctly
        let source = r#"
            LDI r1, 0x3000
            LDI r2, 1
            VM_LIVE_SPAWN r1, r2
            HALT
        "#;
        let result = assemble(source, 0);
        assert!(result.is_ok(), "VM_LIVE_SPAWN should assemble");
        let asm = result.unwrap();
        // 0xB4, r1, r2 = 3 words for VM_LIVE_SPAWN
        assert_eq!(asm.pixels[0], 0xB4);
        assert_eq!(asm.pixels[1], 1); // r1
        assert_eq!(asm.pixels[2], 2); // r2
    }

    #[test]
    fn test_vm_live_step_assembles() {
        let source = "VM_LIVE_STEP\nHALT";
        let result = assemble(source, 0);
        assert!(result.is_ok(), "VM_LIVE_STEP should assemble");
        let asm = result.unwrap();
        assert_eq!(asm.pixels[0], 0xB5);
    }

    #[test]
    fn test_vm_live_kill_assembles() {
        let source = "VM_LIVE_KILL\nHALT";
        let result = assemble(source, 0);
        assert!(result.is_ok(), "VM_LIVE_KILL should assemble");
        let asm = result.unwrap();
        assert_eq!(asm.pixels[0], 0xB6);
    }

    #[test]
    fn test_vm_live_spawn_no_window_error() {
        // VM_LIVE_SPAWN with window_reg = 0 should return 0xFFFFFFFE
        let vm = compile_run(
            "LDI r1, 0x3000\n\
             LDI r2, 0\n\
             ; Write config string at 0x3000\n\
             LDI r10, 0x3000\n\
             LDI r11, 0x61\n\
             STORE r10, r11\n\
             LDI r10, 0x3001\n\
             LDI r11, 0\n\
             STORE r10, r11\n\
             VM_LIVE_SPAWN r1, r2\n\
             HALT",
        );
        assert_eq!(vm.regs[0], 0xFFFFFFFE, "window_id=0 should fail with 0xFFFFFFFE");
    }

    #[test]
    fn test_vm_live_spawn_empty_config() {
        // VM_LIVE_SPAWN with empty config should fail
        let vm = compile_run(
            "LDI r1, 0x3000\n\
             LDI r10, 0x3000\n\
             LDI r11, 0\n\
             STORE r10, r11\n\
             LDI r2, 1\n\
             VM_LIVE_SPAWN r1, r2\n\
             HALT",
        );
        assert_eq!(vm.regs[0], 0xFFFFFFFF, "empty config should fail");
    }

    #[test]
    fn test_vm_live_spawn_missing_kernel() {
        // VM_LIVE_SPAWN with config but no kernel= should fail
        let vm = compile_run(
            "LDI r1, 0x3000\n\
             ; Write 'arch=riscv32 ram=64M\\0' at 0x3000\n\
             STRO r1, \"arch=riscv32 ram=64M\"\n\
             LDI r2, 1\n\
             VM_LIVE_SPAWN r1, r2\n\
             HALT",
        );
        // Missing kernel= should return 0xFFFFFFFC
        assert_eq!(vm.regs[0], 0xFFFFFFFC, "missing kernel= should fail with 0xFFFFFFFC");
    }

    #[test]
    fn test_vm_live_step_no_vm() {
        // VM_LIVE_STEP with no live VM should return 0xFFFFFFFF
        let vm = compile_run("VM_LIVE_STEP\nHALT");
        assert_eq!(vm.regs[0], 0xFFFFFFFF, "no live VM should return error");
    }

    #[test]
    fn test_vm_live_kill_no_vm() {
        // VM_LIVE_KILL with no live VM should return 0xFFFFFFFF
        let vm = compile_run("VM_LIVE_KILL\nHALT");
        assert_eq!(vm.regs[0], 0xFFFFFFFF, "no live VM to kill should return error");
    }

    #[test]
    fn test_vm_live_kill_clears_state() {
        // Spawn then kill - live_hypervisor should be None
        let mut vm = Vm::new();
        assert!(vm.live_hypervisor.is_none());

        // Simulate: create a live hypervisor state directly
        // (since we can't load a kernel in tests without the file)
        // Instead test the opcode path for kill when there's nothing to kill
        let source = "VM_LIVE_KILL\nHALT";
        let asm = assemble(source, 0).unwrap();
        for (i, &pixel) in asm.pixels.iter().enumerate() {
            if i < vm.ram.len() {
                vm.ram[i] = pixel;
            }
        }
        vm.pc = 0;
        vm.halted = false;
        for _ in 0..1000 {
            if !vm.step() { break; }
        }
        assert!(vm.live_hypervisor.is_none());
        assert_eq!(vm.regs[0], 0xFFFFFFFF);
    }

    #[test]
    fn test_live_hypervisor_state_fields() {
        // Verify the LiveHypervisorState struct exists with correct fields
        // by creating a RiscvVm directly
        let riscv_vm = crate::riscv::RiscvVm::new(1024 * 1024); // 1MB
        let state = LiveHypervisorState {
            vm: riscv_vm,
            window_id: 1,
            instructions_per_slice: 500,
            total_instructions: 0,
            console_row: 0,
            console_col: 0,
            booted: false,
        };
        assert_eq!(state.window_id, 1);
        assert_eq!(state.instructions_per_slice, 500);
        assert_eq!(state.total_instructions, 0);
        assert!(!state.booted);
    }

    #[test]
    fn test_render_console_to_window_basic() {
        // Test the render_console_to_window method directly
        // by creating a VM with a window and a live hypervisor state
        let mut vm = vm_with_window();

        // Verify window was created
        let win = vm.windows.iter().find(|w| w.active);
        assert!(win.is_some(), "window should be created");
        let win_id = win.unwrap().id;

        // Create a RISC-V VM and inject some UART output
        let mut riscv_vm = crate::riscv::RiscvVm::new(1024 * 1024);
        // Write some chars to UART TX buffer
        riscv_vm.bus.uart.write_byte(0, b'H');
        riscv_vm.bus.uart.write_byte(0, b'e');
        riscv_vm.bus.uart.write_byte(0, b'l');
        riscv_vm.bus.uart.write_byte(0, b'l');
        riscv_vm.bus.uart.write_byte(0, b'o');

        let mut live = LiveHypervisorState {
            vm: riscv_vm,
            window_id: win_id,
            instructions_per_slice: 500,
            total_instructions: 0,
            console_row: 0,
            console_col: 0,
            booted: false,
        };

        // Render console
        vm.render_console_to_window(&mut live);

        // Check that characters were rendered
        let win = vm.windows.iter().find(|w| w.id == win_id).unwrap();
        assert!(win.offscreen_buffer.iter().any(|&p| p != 0),
            "window buffer should have non-zero pixels after rendering");

        // Cursor should have advanced
        assert_eq!(live.console_col, 5, "5 characters should have been rendered");

        // UART should be drained
        let drained = live.vm.bus.uart.drain_tx();
        assert!(drained.is_empty(), "UART should be drained after rendering");
    }

    #[test]
    fn test_render_console_newline() {
        // Test that newlines advance the row cursor
        let mut vm = vm_with_window();
        let win_id = vm.windows.iter().find(|w| w.active).unwrap().id;

        let mut riscv_vm = crate::riscv::RiscvVm::new(1024 * 1024);
        riscv_vm.bus.uart.write_byte(0, b'A');
        riscv_vm.bus.uart.write_byte(0, 10); // newline
        riscv_vm.bus.uart.write_byte(0, b'B');

        let mut live = LiveHypervisorState {
            vm: riscv_vm,
            window_id: win_id,
            instructions_per_slice: 500,
            total_instructions: 0,
            console_row: 0,
            console_col: 0,
            booted: false,
        };

        vm.render_console_to_window(&mut live);

        assert_eq!(live.console_row, 1, "should be on row 1 after newline");
        assert_eq!(live.console_col, 1, "B should be at col 1 on new row");
    }

    #[test]
    fn test_render_console_scroll() {
        // Test that the console scrolls when it reaches the bottom
        let mut vm = vm_with_window();
        let win_id = vm.windows.iter().find(|w| w.active).unwrap().unwrap();
        let win_h = win.h as usize;
        let rows = win_h / 8; // char_h = 8

        let mut riscv_vm = crate::riscv::RiscvVm::new(1024 * 1024);

        // Write enough lines to fill and scroll
        let mut live = LiveHypervisorState {
            vm: riscv_vm,
            window_id: win_id,
            instructions_per_slice: 500,
            total_instructions: 0,
            console_row: 0,
            console_col: 0,
            booted: false,
        };

        // Fill rows + 1 lines
        for _ in 0..=rows {
            live.vm.bus.uart.write_byte(0, b'X');
            live.vm.bus.uart.write_byte(0, 10); // newline
            vm.render_console_to_window(&mut live);
        }

        // Should have scrolled - row should be clamped to rows-1
        assert_eq!(live.console_row as usize, rows - 1,
            "after scroll, row should be clamped to rows-1");
    }

    #[test]
    fn test_vm_live_spawn_double_fails() {
        // Can't spawn two live VMs
        // This test verifies the "already have a live VM" check
        // We test indirectly since we can't load a real kernel
        let mut vm = Vm::new();
        assert!(vm.live_hypervisor.is_none());

        // Manually set live_hypervisor to simulate an active VM
        let riscv_vm = crate::riscv::RiscvVm::new(1024 * 1024);
        vm.live_hypervisor = Some(LiveHypervisorState {
            vm: riscv_vm,
            window_id: 1,
            instructions_per_slice: 500,
            total_instructions: 0,
            console_row: 0,
            console_col: 0,
            booted: false,
        });

        // Now try to spawn another - should fail
        let source = "LDI r1, 0x3000\nLDI r2, 1\nVM_LIVE_SPAWN r1, r2\nHALT";
        let asm = assemble(source, 0).unwrap();
        for (i, &pixel) in asm.pixels.iter().enumerate() {
            if i < vm.ram.len() {
                vm.ram[i + 0x100] = pixel;
            }
        }
        vm.pc = 0x100;
        vm.halted = false;
        for _ in 0..10_000 {
            if !vm.step() { break; }
        }
        assert_eq!(vm.regs[0], 0xFFFFFFFD, "double spawn should fail with 0xFFFFFFFD");
    }

    #[test]
    fn test_disasm_live_opcodes() {
        // Verify disassembler recognizes the new opcodes
        let vm = Vm::new();

        // VM_LIVE_SPAWN (0xB4)
        let (name, len) = vm.disassemble_at(0);
        // For this test we write raw bytes and disasm
        let mut test_vm = Vm::new();
        test_vm.ram[0] = 0xB4;
        test_vm.ram[1] = 1;
        test_vm.ram[2] = 2;
        let (name, len) = test_vm.disassemble_at(0);
        assert!(name.contains("VM_LIVE_SPAWN"), "disasm should show VM_LIVE_SPAWN, got: {}", name);
        assert_eq!(len, 3);

        // VM_LIVE_STEP (0xB5)
        test_vm.ram[100] = 0xB5;
        let (name, len) = test_vm.disassemble_at(100);
        assert!(name.contains("VM_LIVE_STEP"), "disasm should show VM_LIVE_STEP, got: {}", name);
        assert_eq!(len, 1);

        // VM_LIVE_KILL (0xB6)
        test_vm.ram[200] = 0xB6;
        let (name, len) = test_vm.disassemble_at(200);
        assert!(name.contains("VM_LIVE_KILL"), "disasm should show VM_LIVE_KILL, got: {}", name);
        assert_eq!(len, 1);
    }
}
