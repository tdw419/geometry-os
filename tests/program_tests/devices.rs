use super::*;



// ===================================================================
// Phase 28: Device Driver Abstraction Tests
// ===================================================================

#[test]
fn test_ioctl_assembles() {
    let source = "
    IOCTL r1, r2, r3
    HALT
    ";
    let result = assemble(source, 0);
    assert!(result.is_ok(), "IOCTL should assemble: {:?}", result.err());
    let bc = &result.expect("operation should succeed").pixels;
    assert_eq!(bc[0], 0x62, "IOCTL opcode should be 0x62");
    assert_eq!(bc[1], 1, "fd_reg");
    assert_eq!(bc[2], 2, "cmd_reg");
    assert_eq!(bc[3], 3, "arg_reg");
}

#[test]
fn test_open_device_screen_returns_device_fd() {
    // OPEN /dev/screen should return fd 0xE000
    let mut vm = Vm::new();
    vm.mode = geometry_os::vm::CpuMode::Kernel;
    write_string(&mut vm.ram, 0x1000, "/dev/screen");
    vm.ram[0] = 0x54; // OPEN
    vm.ram[1] = 1;    // path_reg
    vm.ram[2] = 2;    // mode_reg
    vm.regs[1] = 0x1000; // path address
    vm.regs[2] = 0;       // mode
    vm.step();
    assert_eq!(vm.regs[0], 0xE000, "OPEN /dev/screen should return fd 0xE000");
}

#[test]
fn test_open_device_keyboard_returns_device_fd() {
    let mut vm = Vm::new();
    vm.mode = geometry_os::vm::CpuMode::Kernel;
    write_string(&mut vm.ram, 0x1000, "/dev/keyboard");
    vm.ram[0] = 0x54; // OPEN
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.regs[1] = 0x1000;
    vm.regs[2] = 0;
    vm.step();
    assert_eq!(vm.regs[0], 0xE001, "OPEN /dev/keyboard should return fd 0xE001");
}

#[test]
fn test_open_device_audio_returns_device_fd() {
    let mut vm = Vm::new();
    vm.mode = geometry_os::vm::CpuMode::Kernel;
    write_string(&mut vm.ram, 0x1000, "/dev/audio");
    vm.ram[0] = 0x54; // OPEN
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.regs[1] = 0x1000;
    vm.regs[2] = 0;
    vm.step();
    assert_eq!(vm.regs[0], 0xE002, "OPEN /dev/audio should return fd 0xE002");
}

#[test]
fn test_open_device_net_returns_device_fd() {
    let mut vm = Vm::new();
    vm.mode = geometry_os::vm::CpuMode::Kernel;
    write_string(&mut vm.ram, 0x1000, "/dev/net");
    vm.ram[0] = 0x54; // OPEN
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.regs[1] = 0x1000;
    vm.regs[2] = 0;
    vm.step();
    assert_eq!(vm.regs[0], 0xE003, "OPEN /dev/net should return fd 0xE003");
}

#[test]
fn test_ioctl_screen_get_width() {
    // IOCTL /dev/screen cmd=0 (get width) should return 256
    let mut vm = Vm::new();
    vm.mode = geometry_os::vm::CpuMode::Kernel;
    vm.regs[1] = 0xE000; // /dev/screen fd
    vm.regs[2] = 0;      // cmd: get width
    vm.regs[3] = 0;      // arg: unused
    vm.ram[0] = 0x62;    // IOCTL
    vm.ram[1] = 1;       // fd_reg
    vm.ram[2] = 2;       // cmd_reg
    vm.ram[3] = 3;       // arg_reg
    vm.step();
    assert_eq!(vm.regs[0], 256, "IOCTL screen get width should return 256");
}

#[test]
fn test_ioctl_screen_get_height() {
    // IOCTL /dev/screen cmd=1 (get height) should return 256
    let mut vm = Vm::new();
    vm.mode = geometry_os::vm::CpuMode::Kernel;
    vm.regs[1] = 0xE000; // /dev/screen fd
    vm.regs[2] = 1;      // cmd: get height
    vm.regs[3] = 0;      // arg: unused
    vm.ram[0] = 0x62;    // IOCTL
    vm.ram[1] = 1;       // fd_reg
    vm.ram[2] = 2;       // cmd_reg
    vm.ram[3] = 3;       // arg_reg
    vm.step();
    assert_eq!(vm.regs[0], 256, "IOCTL screen get height should return 256");
}

#[test]
fn test_ioctl_screen_invalid_command() {
    // IOCTL /dev/screen cmd=99 should return 0xFFFFFFFF
    let mut vm = Vm::new();
    vm.mode = geometry_os::vm::CpuMode::Kernel;
    vm.regs[1] = 0xE000;
    vm.regs[2] = 99;
    vm.regs[3] = 0;
    vm.ram[0] = 0x62;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 3;
    vm.step();
    assert_eq!(vm.regs[0], 0xFFFFFFFF, "IOCTL screen invalid cmd should return error");
}

#[test]
fn test_ioctl_keyboard_get_echo_mode() {
    // IOCTL /dev/keyboard cmd=0 (get echo) should return value from RAM[0xFF8]
    let mut vm = Vm::new();
    vm.mode = geometry_os::vm::CpuMode::Kernel;
    vm.ram[0xFF8] = 42; // set echo mode
    vm.regs[1] = 0xE001; // /dev/keyboard fd
    vm.regs[2] = 0;      // cmd: get echo mode
    vm.regs[3] = 0;
    vm.ram[0] = 0x62;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 3;
    vm.step();
    assert_eq!(vm.regs[0], 42, "IOCTL keyboard get echo should return 42");
}

#[test]
fn test_ioctl_keyboard_set_echo_mode() {
    // IOCTL /dev/keyboard cmd=1 (set echo) should set RAM[0xFF8]
    let mut vm = Vm::new();
    vm.mode = geometry_os::vm::CpuMode::Kernel;
    vm.regs[1] = 0xE001;
    vm.regs[2] = 1;      // cmd: set echo mode
    vm.regs[3] = 77;     // arg: new echo mode value
    vm.ram[0] = 0x62;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 3;
    vm.step();
    assert_eq!(vm.regs[0], 0, "IOCTL keyboard set echo should return 0 (success)");
    assert_eq!(vm.ram[0xFF8], 77, "RAM[0xFF8] should be set to 77");
}

#[test]
fn test_ioctl_audio_get_volume() {
    // IOCTL /dev/audio cmd=0 (get volume) should return RAM[0xFF7]
    let mut vm = Vm::new();
    vm.mode = geometry_os::vm::CpuMode::Kernel;
    vm.ram[0xFF7] = 50; // set volume
    vm.regs[1] = 0xE002; // /dev/audio fd
    vm.regs[2] = 0;
    vm.regs[3] = 0;
    vm.ram[0] = 0x62;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 3;
    vm.step();
    assert_eq!(vm.regs[0], 50, "IOCTL audio get volume should return 50");
}

#[test]
fn test_ioctl_audio_set_volume() {
    // IOCTL /dev/audio cmd=1 (set volume) should set RAM[0xFF7] and clamp to 100
    let mut vm = Vm::new();
    vm.mode = geometry_os::vm::CpuMode::Kernel;
    vm.regs[1] = 0xE002;
    vm.regs[2] = 1;       // cmd: set volume
    vm.regs[3] = 150;     // arg: volume > 100 (should clamp)
    vm.ram[0] = 0x62;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 3;
    vm.step();
    assert_eq!(vm.regs[0], 0, "IOCTL audio set volume should return 0 (success)");
    assert_eq!(vm.ram[0xFF7], 100, "Volume should be clamped to 100");
}

#[test]
fn test_ioctl_net_get_status() {
    // IOCTL /dev/net cmd=0 (get status) should return 1 (up)
    let mut vm = Vm::new();
    vm.mode = geometry_os::vm::CpuMode::Kernel;
    vm.regs[1] = 0xE003; // /dev/net fd
    vm.regs[2] = 0;
    vm.regs[3] = 0;
    vm.ram[0] = 0x62;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 3;
    vm.step();
    assert_eq!(vm.regs[0], 1, "IOCTL net get status should return 1 (up)");
}

#[test]
fn test_ioctl_non_device_fd_returns_error() {
    // IOCTL on a non-device fd should return 0xFFFFFFFF
    let mut vm = Vm::new();
    vm.mode = geometry_os::vm::CpuMode::Kernel;
    vm.regs[1] = 5; // regular fd, not a device fd
    vm.regs[2] = 0;
    vm.regs[3] = 0;
    vm.ram[0] = 0x62;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 3;
    vm.step();
    assert_eq!(vm.regs[0], 0xFFFFFFFF, "IOCTL on non-device fd should return error");
}

#[test]
fn test_read_device_keyboard() {
    // READ from /dev/keyboard should read RAM[0xFFF] and clear it
    let mut vm = Vm::new();
    vm.mode = geometry_os::vm::CpuMode::Kernel;
    vm.ram[0xFFF] = 65; // 'A' key pressed
    vm.regs[1] = 0xE001; // /dev/keyboard fd
    vm.regs[2] = 0x1000; // buf addr
    vm.regs[3] = 1;      // len
    vm.ram[0] = 0x55;    // READ
    vm.ram[1] = 1;       // fd_reg
    vm.ram[2] = 2;       // buf_reg
    vm.ram[3] = 3;       // len_reg
    vm.step();
    assert_eq!(vm.regs[0], 1, "READ keyboard should return 1 byte");
    assert_eq!(vm.ram[0x1000], 65, "Buffer should contain the key");
    assert_eq!(vm.ram[0xFFF], 0, "Keyboard port should be cleared");
}

#[test]
fn test_read_device_net() {
    // READ from /dev/net should read RAM[0xFFC]
    let mut vm = Vm::new();
    vm.mode = geometry_os::vm::CpuMode::Kernel;
    vm.ram[0xFFC] = 42; // network data
    vm.regs[1] = 0xE003; // /dev/net fd
    vm.regs[2] = 0x1000; // buf addr
    vm.regs[3] = 1;      // len
    vm.ram[0] = 0x55;    // READ
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 3;
    vm.step();
    assert_eq!(vm.regs[0], 1, "READ net should return 1 word");
    assert_eq!(vm.ram[0x1000], 42, "Buffer should contain network data");
}

#[test]
fn test_write_device_screen() {
    // WRITE to /dev/screen should set pixel from (x, y, color) triplet
    let mut vm = Vm::new();
    vm.mode = geometry_os::vm::CpuMode::Kernel;
    vm.regs[1] = 0xE000; // /dev/screen fd
    vm.regs[2] = 0x1000; // buf addr
    vm.regs[3] = 3;      // len (one triplet)
    vm.ram[0x1000] = 10; // x
    vm.ram[0x1001] = 20; // y
    vm.ram[0x1002] = 5;  // color
    vm.ram[0] = 0x56;    // WRITE
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 3;
    vm.step();
    assert_eq!(vm.regs[0], 3, "WRITE screen should return 3 words written");
    assert_eq!(vm.screen[20 * 256 + 10], 5, "Pixel at (10,20) should be color 5");
}

#[test]
fn test_write_device_screen_multiple_triplets() {
    // WRITE to /dev/screen with 6 words = 2 triplets
    let mut vm = Vm::new();
    vm.mode = geometry_os::vm::CpuMode::Kernel;
    vm.regs[1] = 0xE000;
    vm.regs[2] = 0x1000;
    vm.regs[3] = 6; // 2 triplets
    vm.ram[0x1000] = 0;  vm.ram[0x1001] = 0;  vm.ram[0x1002] = 1; // (0,0) = red
    vm.ram[0x1003] = 1;  vm.ram[0x1004] = 0;  vm.ram[0x1005] = 2; // (1,0) = green
    vm.ram[0] = 0x56;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 3;
    vm.step();
    assert_eq!(vm.regs[0], 6, "WRITE screen should return 6 words written");
    assert_eq!(vm.screen[0], 1, "Pixel at (0,0) should be 1");
    assert_eq!(vm.screen[1], 2, "Pixel at (1,0) should be 2");
}

#[test]
fn test_write_device_audio() {
    // WRITE to /dev/audio should set beep
    let mut vm = Vm::new();
    vm.mode = geometry_os::vm::CpuMode::Kernel;
    vm.regs[1] = 0xE002; // /dev/audio fd
    vm.regs[2] = 0x1000; // buf addr
    vm.regs[3] = 2;      // len (freq + duration pair)
    vm.ram[0x1000] = 440; // frequency
    vm.ram[0x1001] = 100; // duration ms
    vm.ram[0] = 0x56;    // WRITE
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 3;
    vm.step();
    assert_eq!(vm.regs[0], 2, "WRITE audio should return 2");
    assert!(vm.beep.is_some(), "beep should be set");
    let (freq, dur) = vm.beep.expect("operation should succeed");
    assert_eq!(freq, 440);
    assert_eq!(dur, 100);
}

#[test]
fn test_write_device_net() {
    // WRITE to /dev/net should write to RAM[0xFFC]
    let mut vm = Vm::new();
    vm.mode = geometry_os::vm::CpuMode::Kernel;
    vm.regs[1] = 0xE003; // /dev/net fd
    vm.regs[2] = 0x1000; // buf addr
    vm.regs[3] = 1;      // len
    vm.ram[0x1000] = 99; // data to send
    vm.ram[0xFFC] = 0;   // clear net port first
    vm.ram[0] = 0x56;    // WRITE
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 3;
    vm.step();
    assert_eq!(vm.regs[0], 1, "WRITE net should return 1");
    assert_eq!(vm.ram[0xFFC], 99, "Net port should contain sent data");
}

#[test]
fn test_open_device_via_assembly_program() {
    // Full assembly program that opens all 4 devices and verifies their fds
    // Using inline assembly without .db (which isn't supported by the assembler)
    let mut vm = Vm::new();
    vm.mode = geometry_os::vm::CpuMode::Kernel;

    // Write device path strings into RAM
    write_string(&mut vm.ram, 0x1000, "/dev/screen");
    write_string(&mut vm.ram, 0x1010, "/dev/keyboard");
    write_string(&mut vm.ram, 0x1020, "/dev/audio");
    write_string(&mut vm.ram, 0x1030, "/dev/net");

    // Open /dev/screen: LDI r1, 0x1000; LDI r2, 0; OPEN r1, r2
    vm.ram[0] = 0x10; vm.ram[1] = 1; vm.ram[2] = 0x1000; // LDI r1, 0x1000
    vm.ram[3] = 0x10; vm.ram[4] = 2; vm.ram[5] = 0;       // LDI r2, 0
    vm.ram[6] = 0x54; vm.ram[7] = 1; vm.ram[8] = 2;       // OPEN r1, r2

    // Open /dev/keyboard
    vm.ram[9]  = 0x10; vm.ram[10] = 1; vm.ram[11] = 0x1010;
    vm.ram[12] = 0x10; vm.ram[13] = 2; vm.ram[14] = 0;
    vm.ram[15] = 0x54; vm.ram[16] = 1; vm.ram[17] = 2;

    // Open /dev/audio
    vm.ram[18] = 0x10; vm.ram[19] = 1; vm.ram[20] = 0x1020;
    vm.ram[21] = 0x10; vm.ram[22] = 2; vm.ram[23] = 0;
    vm.ram[24] = 0x54; vm.ram[25] = 1; vm.ram[26] = 2;

    // Open /dev/net
    vm.ram[27] = 0x10; vm.ram[28] = 1; vm.ram[29] = 0x1030;
    vm.ram[30] = 0x10; vm.ram[31] = 2; vm.ram[32] = 0;
    vm.ram[33] = 0x54; vm.ram[34] = 1; vm.ram[35] = 2;

    vm.ram[36] = 0x00; // HALT

    for _ in 0..1000 {
        if !vm.step() { break; }
    }
    assert_eq!(vm.regs[0], 0xE003, "Last OPEN should return /dev/net fd");
}

#[test]
fn test_ioctl_roundtrip_get_and_set() {
    // Test IOCTL roundtrip: get keyboard echo, set to new value, verify
    let mut vm = Vm::new();
    vm.mode = geometry_os::vm::CpuMode::Kernel;

    // Open /dev/keyboard
    write_string(&mut vm.ram, 0x1000, "/dev/keyboard");
    vm.ram[0] = 0x10; vm.ram[1] = 1; vm.ram[2] = 0x1000; // LDI r1, 0x1000
    vm.ram[3] = 0x10; vm.ram[4] = 2; vm.ram[5] = 0;       // LDI r2, 0
    vm.ram[6] = 0x54; vm.ram[7] = 1; vm.ram[8] = 2;       // OPEN r1, r2
    // r0 now = 0xE001, save it to r10
    vm.ram[9]  = 0x10; vm.ram[10] = 10; vm.ram[11] = 0xE001; // LDI r10, 0xE001 (save fd)

    // Get echo mode: IOCTL r10, r4, r5 (cmd=0)
    vm.ram[12] = 0x10; vm.ram[13] = 4; vm.ram[14] = 0;    // LDI r4, 0
    vm.ram[15] = 0x62; vm.ram[16] = 10; vm.ram[17] = 4; vm.ram[18] = 5; // IOCTL r10, r4, r5

    // Set echo mode to 1: LDI r5, 1; LDI r4, 1; IOCTL r10, r4, r5
    vm.ram[19] = 0x10; vm.ram[20] = 5; vm.ram[21] = 1;    // LDI r5, 1
    vm.ram[22] = 0x10; vm.ram[23] = 4; vm.ram[24] = 1;    // LDI r4, 1
    vm.ram[25] = 0x62; vm.ram[26] = 10; vm.ram[27] = 4; vm.ram[28] = 5; // IOCTL r10, r4, r5

    // Get echo mode again: LDI r4, 0; IOCTL r10, r4, r5
    vm.ram[29] = 0x10; vm.ram[30] = 4; vm.ram[31] = 0;    // LDI r4, 0
    vm.ram[32] = 0x62; vm.ram[33] = 10; vm.ram[34] = 4; vm.ram[35] = 5; // IOCTL r10, r4, r5

    vm.ram[36] = 0x00; // HALT

    for _ in 0..10000 {
        if !vm.step() { break; }
    }
    assert_eq!(vm.regs[0], 1, "Keyboard echo mode should be 1 after set");
}

#[test]
fn test_device_read_keyboard() {
    // READ from /dev/keyboard should read RAM[0xFFF] and clear it
    let mut vm = Vm::new();
    vm.mode = geometry_os::vm::CpuMode::Kernel;
    vm.ram[0xFFF] = 65; // pretend 'A' key pressed

    // READ fd=0xE001 (keyboard), buf=r1, len=r2
    vm.regs[0] = 0xE001; // keyboard fd
    vm.regs[1] = 0x2000; // buf addr
    vm.regs[2] = 1;      // len
    vm.ram[0] = 0x55;    // READ
    vm.ram[1] = 0;       // fd_reg
    vm.ram[2] = 1;       // buf_reg
    vm.ram[3] = 2;       // len_reg
    vm.step();
    assert_eq!(vm.regs[0], 1, "READ should return 1 byte read");
    assert_eq!(vm.ram[0x2000], 65, "Buffer should contain the key value");
    assert_eq!(vm.ram[0xFFF], 0, "Keyboard port should be cleared after read");
}

#[test]
fn test_device_write_screen_pixel() {
    // WRITE to /dev/screen with (x, y, color) triplet should set a pixel
    let mut vm = Vm::new();
    vm.mode = geometry_os::vm::CpuMode::Kernel;

    // Set up pixel data at 0x2000: x=10, y=20, color=0xFF0000
    vm.ram[0x2000] = 10;
    vm.ram[0x2001] = 20;
    vm.ram[0x2002] = 0xFF0000;

    // WRITE fd=0xE000 (screen), buf=r1, len=r2
    vm.regs[0] = 0xE000; // screen fd
    vm.regs[1] = 0x2000; // buf addr
    vm.regs[2] = 3;      // len (x,y,color triplet)
    vm.ram[0] = 0x56;    // WRITE
    vm.ram[1] = 0;       // fd_reg
    vm.ram[2] = 1;       // buf_reg
    vm.ram[3] = 2;       // len_reg
    vm.step();
    assert_eq!(vm.regs[0], 3, "WRITE should return 3 words written");
    assert_eq!(vm.screen[20 * 256 + 10], 0xFF0000, "Screen pixel should be set");
}

#[test]
fn test_device_write_audio_beep() {
    // WRITE to /dev/audio with (freq, duration) should trigger beep
    let mut vm = Vm::new();
    vm.mode = geometry_os::vm::CpuMode::Kernel;

    // Set up audio data at 0x2000: freq=440, dur=100
    vm.ram[0x2000] = 440;
    vm.ram[0x2001] = 100;

    vm.regs[0] = 0xE002; // audio fd
    vm.regs[1] = 0x2000;
    vm.regs[2] = 2;
    vm.ram[0] = 0x56;    // WRITE
    vm.ram[1] = 0;
    vm.ram[2] = 1;
    vm.ram[3] = 2;
    vm.step();
    assert_eq!(vm.regs[0], 2, "WRITE should return 2 words written");
    assert!(vm.beep.is_some(), "Beep should be triggered");
    let (freq, dur) = vm.beep.expect("operation should succeed");
    assert_eq!(freq, 440);
    assert_eq!(dur, 100);
}

#[test]
fn test_device_close_is_noop() {
    // CLOSE on a device fd should always succeed (return 0)
    let mut vm = Vm::new();
    vm.mode = geometry_os::vm::CpuMode::Kernel;
    vm.regs[0] = 0xE001; // keyboard fd
    vm.ram[0] = 0x57;    // CLOSE
    vm.ram[1] = 0;       // fd_reg
    vm.step();
    assert_eq!(vm.regs[0], 0, "CLOSE should return 0 (success) for device fd");
}

#[test]
fn test_ioctl_screen_dimensions() {
    // IOCTL /dev/screen cmd=0 returns width (256), cmd=1 returns height (256)
    let mut vm = Vm::new();
    vm.mode = geometry_os::vm::CpuMode::Kernel;

    // Get width: IOCTL r10=0xE000, r4=0, r5=0
    vm.regs[10] = 0xE000;
    vm.regs[4] = 0; // cmd = get width
    vm.regs[5] = 0;
    vm.ram[0] = 0x62; // IOCTL
    vm.ram[1] = 10;
    vm.ram[2] = 4;
    vm.ram[3] = 5;
    vm.step();
    assert_eq!(vm.regs[0], 256, "Screen width should be 256");
}

#[test]
fn test_ioctl_audio_volume() {
    // IOCTL /dev/audio: set volume to 75, then read it back
    let mut vm = Vm::new();
    vm.mode = geometry_os::vm::CpuMode::Kernel;

    // Set volume: IOCTL r10=0xE002, r4=1 (set), r5=75
    vm.regs[10] = 0xE002;
    vm.regs[4] = 1;
    vm.regs[5] = 75;
    vm.ram[0] = 0x62;
    vm.ram[1] = 10;
    vm.ram[2] = 4;
    vm.ram[3] = 5;
    vm.step();
    assert_eq!(vm.regs[0], 0, "Set volume should succeed");

    // Get volume: IOCTL r10=0xE002, r4=0 (get), r5=0
    vm.regs[4] = 0;
    vm.regs[5] = 0;
    vm.ram[4] = 0x62;
    vm.ram[5] = 10;
    vm.ram[6] = 4;
    vm.ram[7] = 5;
    vm.step();
    assert_eq!(vm.regs[0], 75, "Volume should be 75");
}

#[test]
fn test_ioctl_net_status() {
    // IOCTL /dev/net cmd=0 returns status (1=up)
    let mut vm = Vm::new();
    vm.mode = geometry_os::vm::CpuMode::Kernel;

    vm.regs[10] = 0xE003;
    vm.regs[4] = 0; // cmd = get status
    vm.regs[5] = 0;
    vm.ram[0] = 0x62;
    vm.ram[1] = 10;
    vm.ram[2] = 4;
    vm.ram[3] = 5;
    vm.step();
    assert_eq!(vm.regs[0], 1, "Net status should be 1 (up)");
}
