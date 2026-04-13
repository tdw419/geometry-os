# Hypervisor Design for Geometry OS

This document describes how Geometry OS becomes a hypervisor -- capable of
running guest operating systems (Linux, FreeBSD, OpenBSD, Windows) inside
the existing Geometry OS VM.

Two paths, ordered by value:

1. **QEMU Bridge** (Phase 33) -- wrap QEMU subprocess, render console on canvas
2. **RISC-V Interpreter** (Phases 34-37) -- owned stack, pure Rust, WASM-portable

Written for AI agents who need to implement, test, or extend this system.

---

## Part I: QEMU Bridge (Phase 33)

### Architecture

```
+--------------------------------------------------+
|  Host (your machine)                              |
|  +----------------------------------------------+ |
|  |  Geometry OS VM (minifb window)               | |
|  |  +------------------------------------------+ | |
|  |  |  Canvas Text Surface (32x128 grid)       | | |
|  |  |  - Pixel font rendering (font::GLYPHS)   | | |
|  |  |  - Scrolling (128 rows, 32 visible)      | | |
|  |  |  - Keyboard input (key_to_ascii_shifted)  | | |
|  |  +------------------------------------------+ | |
|  |  |  qemu.rs bridge                          | | |
|  |  |  - Subprocess spawn (std::process)        | | |
|  |  |  - stdin/stdout pipes (non-blocking)      | | |
|  |  |  - ANSI escape parser                     | | |
|  |  +------------------------------------------+ | |
|  |  |  QEMU child process                      | | |
|  |  |  - qemu-system-riscv64 (or any arch)      | | |
|  |  |  - -nographic -serial mon:stdio            | | |
|  |  |  - Guest OS kernel                        | | |
|  |  +------------------------------------------+ | |
|  +----------------------------------------------+ |
+--------------------------------------------------+
```

### Output Pipeline: QEMU stdout -> Canvas

```
QEMU stdout (raw bytes)
  |
  v
Non-blocking read into Vec<u8> buffer (each frame tick)
  |
  v
ANSI escape parser (state machine)
  - CSI sequences: cursor move, clear, color
  - Track virtual cursor position (row, col)
  - Track scroll region
  |
  v
For each printable character:
  canvas_buffer[cursor_row * 32 + cursor_col] = char as u32
  cursor_col += 1
  if cursor_col >= 32: cursor_col = 0, cursor_row += 1
  if cursor_row >= 128: scroll up one row
  |
  v
Existing pixel font rendering displays the character
  - font::GLYPHS[byte] for 8x8 bitmap
  - syntax_highlight_color() for color
  - Auto-scroll handled by existing canvas logic
```

### Input Pipeline: Keyboard -> QEMU stdin

```
minifb key event (on_key_press)
  |
  v
key_to_ascii_shifted(key, shift)
  -> returns Option<char>
  |
  v
Write char byte to QEMU stdin pipe
  -> use std::process::ChildStdin.write()
  -> guest OS receives character via serial driver
```

Special keys:
- Enter -> write b'\r' (carriage return, standard for serial terminals)
- Backspace -> write 0x7F (DEL, standard for serial terminals)
- Ctrl+C -> write 0x03 (ETX, sends SIGINT to guest)
- Ctrl+Z -> write 0x1A (SUB, sends SIGTSTP to guest)

### ANSI Escape Parser

State machine for handling terminal escape sequences:

```rust
enum AnsiState {
    Normal,
    Escape,       // saw ESC (0x1B)
    Csi,          // saw ESC [
    CsiParam(u8), // collecting CSI parameters
}
```

Handled sequences:
- `ESC [ A` -- cursor up
- `ESC [ B` -- cursor down
- `ESC [ C` -- cursor right
- `ESC [ D` -- cursor left
- `ESC [ H` -- cursor home
- `ESC [ 2 J` -- clear screen
- `ESC [ K` -- clear to end of line
- `ESC [ m` -- set color (can ignore, we use our own colors)
- `ESC [ ? 25 h/l` -- show/hide cursor (track but don't render differently)

### QEMU Command Construction

From the HYPERVISOR opcode config string:

```
"arch=riscv64 kernel=linux.img ram=256M disk=rootfs.ext4"
```

Constructs:

```bash
qemu-system-riscv64 \
  -machine virt \
  -nographic \
  -serial mon:stdio \
  -kernel linux.img \
  -m 256M \
  -drive file=rootfs.ext4,format=raw,if=virtio \
  -no-reboot
```

Architecture mapping:
- `riscv64` -> `qemu-system-riscv64 -machine virt`
- `riscv32` -> `qemu-system-riscv32 -machine virt`
- `x86_64` -> `qemu-system-x86_64`
- `aarch64` -> `qemu-system-aarch64 -machine virt`
- `mipsel` -> `qemu-system-mipsel -machine malta`

### HYPERVISOR Opcode (0x54)

```rust
// In vm.rs execute step:
0x54 => {
    // Read config string from RAM at address in r0
    let config_addr = self.regs[0] as usize;
    let config = self.read_string_from_ram(config_addr);
    // Spawn QEMU subprocess
    self.hypervisor = Some(QemuBridge::spawn(&config)?);
}
```

The bridge runs QEMU as a child process. Each frame tick:
1. Read available bytes from QEMU stdout (non-blocking)
2. Feed through ANSI parser
3. Write printable chars to canvas_buffer
4. Check keyboard events, write to QEMU stdin

Process lifecycle:
- HYPERVISOR opcode spawns QEMU
- F5 (while hypervisor active) sends SIGTERM to QEMU
- QEMU exit detected -> clear hypervisor state, return to normal canvas

### Testing Strategy

**Unit tests:**
- ANSI escape parser: feed known sequences, verify cursor state
- Config parser: verify QEMU command construction from config strings
- Canvas buffer writes: verify character placement and scrolling

**Integration test:**
- Requires QEMU installed on host
- Spawn QEMU with a tiny Linux kernel (or OpenSBI)
- Verify "Linux version" or "OpenSBI" appears in output buffer within 30 seconds
- Kill QEMU, verify clean shutdown

---

## Part II: RISC-V Interpreter (Phases 34-37)

### Why Both Paths

QEMU is the fast path -- boot any OS today. But it has limitations:
- Requires QEMU installed on the host
- Can't run in WASM (browser Geometry OS)
- We don't own the execution layer (can't inspect guest state)
- No integration with Geometry OS memory/IPC/filesystem

The RISC-V interpreter is the deep path:
- Pure Rust, no subprocess, runs anywhere
- Testable with `cargo test`
- Guest RAM is a Vec<u8> we own
- Can integrate with Geometry OS VFS, IPC, device drivers
- Runs in WASM (browser-based hypervisor)
- Teaches us the full stack

### Architecture

```
+--------------------------------------------------+
|  Host (your machine)                              |
|  +----------------------------------------------+ |
|  |  Geometry OS VM (minifb window)               | |
|  |  +------------------------------------------+ | |
|  |  |  Geo OS Shell / Programs                 | | |
|  |  |  (existing bytecode VM, 71 opcodes)      | | |
|  |  +------------------------------------------+ | |
|  |  |  RISC-V Hypervisor (src/riscv/)           | | |
|  |  |  +--------------------------------------+ | | |
|  |  |  |  RV32I CPU interpreter              | | | |
|  |  |  |  CSR registers, privilege modes      | | | |
|  |  |  +--------------------------------------+ | | |
|  |  |  |  SV32 MMU                           | | | |
|  |  |  |  Page table walk, TLB cache          | | | |
|  |  |  +--------------------------------------+ | | |
|  |  |  |  Device Emulation                    | | | |
|  |  |  |  UART, CLINT, PLIC, virtio-blk       | | | |
|  |  |  +--------------------------------------+ | | |
|  |  |  |  Guest RAM (Vec<u8>, 128MB+)         | | | |
|  |  |  +--------------------------------------+ | | |
|  |  +------------------------------------------+ | |
|  +----------------------------------------------+ |
+--------------------------------------------------+
```

### Module Structure

```
src/riscv/
  mod.rs          -- Module exports, RiscvVm struct
  cpu.rs          -- Instruction fetch, decode, execute loop
  decode.rs       -- Instruction decode (opcode -> operation)
  csrs.rs         -- CSR register definitions, read/write logic
  mmu.rs          -- SV32 page table walk, TLB
  memory.rs       -- Guest RAM (Vec<u8>), byte/half/word access
  uart.rs         -- 16550 UART emulation
  clint.rs        -- Core Local Interruptor (timer, software interrupt)
  plic.rs         -- Platform Level Interrupt Controller
  virtio_blk.rs   -- Virtio block device (disk)
  dtb.rs          -- Device Tree Blob generation
  loader.rs       -- ELF/binary image loader
  bridge.rs       -- Connect guest I/O to Geometry OS canvas/keyboard
```

Total estimated: ~2,500-3,000 lines of Rust.

### CPU State

```rust
pub struct RiscvCpu {
    /// General purpose registers x[0..32]. x[0] is always 0.
    pub x: [u32; 32],
    /// Program counter
    pub pc: u32,
    /// Current privilege level: 0=User, 1=Supervisor, 3=Machine
    pub privilege: u8,
    /// CSR registers
    pub csrs: CsrBank,
    /// Guest RAM
    pub memory: GuestMemory,
    /// Device MMIO region
    pub devices: DeviceBus,
    /// TLB for address translation cache
    pub tlb: Tlb,
}
```

### Instruction Decode

RV32I encodes all instructions in 32-bit words:

1. Read 4 bytes from guest RAM at PC (little-endian)
2. Extract opcode bits [6:0]
3. Branch on opcode:
   - 0x33 -> R-type (ADD, SUB, SLL, etc.)
   - 0x13 -> I-type ALU (ADDI, SLTI, etc.)
   - 0x03 -> I-type Load (LB, LH, LW, LBU, LHU)
   - 0x23 -> S-type Store (SB, SH, SW)
   - 0x63 -> B-type Branch (BEQ, BNE, BLT, etc.)
   - 0x37 -> U-type LUI
   - 0x17 -> U-type AUIPC
   - 0x6F -> J-type JAL
   - 0x67 -> I-type JALR
   - 0x73 -> SYSTEM (ECALL, EBREAK, CSR*)
   - 0x0F -> FENCE (NOP)
4. Execute, update registers/PC/memory
5. x[0] hardwired to zero

### Guest Memory

```rust
pub struct GuestMemory {
    ram: Vec<u8>,        // Raw byte storage
    ram_base: u64,       // Typically 0x80000000
}
```

Address map:
```
0x00000000 - 0x00000FFF   Reserved
0x00100000 - 0x00100FFF   UART 16550
0x00100100 - 0x001001FF   Virtio MMIO (disk)
0x00200000 - 0x003FFFFF   CLINT
0x0C000000 - 0x0FFFFFFF   PLIC
0x80000000 - 0x87FFFFFF   Guest RAM (128MB)
```

### Privilege Modes

| Level | Name | Who runs here |
|-------|------|---------------|
| 3 | Machine (M) | OpenSBI/firmware or hypervisor |
| 1 | Supervisor (S) | Guest OS kernel (Linux) |
| 0 | User (U) | Guest user programs |

### Key CSR Registers

| Address | Name | Purpose |
|---------|------|---------|
| 0x300 | mstatus | Machine status |
| 0x305 | mtvec | Machine trap vector |
| 0x341 | mepc | Machine exception PC |
| 0x342 | mcause | Machine trap cause |
| 0x180 | satp | Page table base |
| 0x100 | sstatus | Supervisor status |
| 0x105 | stvec | Supervisor trap vector |
| 0x141 | sepc | Supervisor exception PC |

### SV32 Virtual Memory

When satp.Mode = 1:
```
Virtual address:
[31:22] VPN[1] - Level 1 index (10 bits)
[21:12] VPN[0] - Level 0 index (10 bits)
[11:0]  Offset  - Page offset (12 bits, 4KB pages)

Page Table Entry:
[31:20] PPN - Physical page number
[9] D, [8] A, [7] G, [6] U, [5] X, [4] W, [3] R, [2] V
```

2-level walk: root -> L1 PTE -> L2 PTE -> physical page.

### Device Emulation

| Device | MMIO Base | Purpose |
|--------|-----------|---------|
| UART 16550 | 0x10000000 | Serial console (THR/RBR/LSR) |
| CLINT | 0x02000000 | Timer (mtime/mtimecmp), software interrupt |
| PLIC | 0x0C000000 | Interrupt routing |
| Virtio-blk | 0x10001000 | Block device (disk from VFS) |

Bridge to Geometry OS:
- Guest writes to UART THR -> character to canvas (same pattern as QEMU bridge)
- Keyboard input -> guest UART RBR
- Disk I/O -> Geometry OS VFS

### ELF Loader

Parse ELF64 RISC-V kernel images:
- Verify ELF magic, machine type (EM_RISCV)
- Load LOAD segments into guest RAM at specified physical addresses
- Set PC to entry point (e1_set)
- Pass DTB address in a1 register

### Performance Target

Pure Rust interpreter: 10-50 million instructions/second on host CPU.
Minimal Linux boot: ~50M instructions.
Expected boot time at 20 MIPS: ~2.5 seconds.

---

## Part III: Shared Bridge Layer

Both QEMU and native RISC-V modes share the same I/O bridge to Geometry OS:

```rust
// bridge.rs -- shared between QEMU and native RISC-V

pub trait HypervisorBridge {
    /// Write a character to the canvas
    fn write_char(&mut self, ch: char);
    /// Read next character from keyboard buffer
    fn read_char(&mut self) -> Option<char>;
    /// Check if guest has output ready
    fn has_output(&self) -> bool;
}
```

- **QemuBridge** implements this by piping QEMU stdout/stdin
- **NativeBridge** implements this by reading UART THR / writing UART RBR

Both feed into the same canvas_buffer rendering pipeline. The canvas doesn't
know or care which backend is running.

---

## Relationship to Old Prototype

The old RISC-V implementation in `geometry_os123456789/systems/infinite_map_rs/src/riscv/`
(2,189 lines) runs on GPU compute shaders. We reuse the *knowledge* (CSR layouts,
memory maps, UART register offsets) but rewrite everything from scratch.

| Aspect | Old (infinite_map_rs) | New (Geometry OS) |
|---|---|---|
| Execution | GPU compute shaders | Pure Rust CPU + QEMU subprocess |
| Dependencies | wgpu, naga, AMDGPU DRM | None (QEMU optional) |
| Testing | Requires GPU | cargo test anywhere |
| Program format | .rts.png (PNG encoded) | ELF / raw binary |
| Integration | Separate app | Module in Geometry OS |
| Multi-arch | RISC-V only | QEMU: any arch. Native: RISC-V |

---

## See Also

- **docs/NORTH_STAR.md** -- Running Linux inside Geometry OS is the ultimate answer to "Is this more like a real OS?"
- **docs/ARCHITECTURE.md** -- Existing Geometry OS architecture, opcodes, memory map
- **docs/CANVAS_TEXT_SURFACE.md** -- How text rendering works. Both QEMU and native modes use this pipeline.
- **Old prototype** -- `~/zion/projects/geometry_os/geometry_os123456789/systems/infinite_map_rs/src/riscv/`
