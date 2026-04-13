# RISC-V Hypervisor Design for Geometry OS

This document describes how Geometry OS will implement a RISC-V hypervisor capable
of running guest operating systems (Linux, FreeBSD, OpenBSD) inside the existing
Geometry OS VM.

Written for AI agents who need to implement, test, or extend this system.

---

## Why RISC-V

RISC-V is the force multiplier. Once we have a working RISC-V interpreter with
privilege modes and virtual memory:

- **Other OSes are free.** Linux, FreeBSD, OpenBSD, Ubuntu -- they all have
  RISC-V ports. We write one emulator, every OS with a RISC-V build boots on it.

- **Other languages are free.** C, Rust, Python, Go, JavaScript -- they all have
  RISC-V backends. Boot guest Linux, get the entire software ecosystem.

- **No GPU dependencies.** The old prototype (infinite_map_rs) ran RISC-V on GPU
  compute shaders. This rebuild is pure Rust, testable with `cargo test`.

---

## Architecture Overview

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
|  |  |  |  RV32I/RV64 CPU interpreter          | | | |
|  |  |  |  CSR registers, privilege modes      | | | |
|  |  |  +--------------------------------------+ | | |
|  |  |  |  SV32/SV39 MMU                       | | | |
|  |  |  |  Page table walk, TLB cache          | | | |
|  |  |  +--------------------------------------+ | | |
|  |  |  |  Device Emulation                    | | | |
|  |  |  |  UART, CLINT, PLIC, virtio-blk/net   | | | |
|  |  |  +--------------------------------------+ | | |
|  |  |  |  Guest RAM (Vec<u8>, 128MB+)         | | | |
|  |  |  +--------------------------------------+ | | |
|  |  +------------------------------------------+ | |
|  +----------------------------------------------+ |
+--------------------------------------------------+
```

The hypervisor is a module inside Geometry OS, not a separate program. Guest
RAM is a Vec<u8> allocated by the host. Device I/O bridges between the guest
and the existing Geometry OS screen/keyboard/filesystem.

---

## Module Structure

```
src/riscv/
  mod.rs          -- Module exports, RiscvVm struct
  cpu.rs          -- Instruction fetch, decode, execute loop
  decode.rs       -- Instruction decode (opcode -> operation)
  csrs.rs         -- CSR register definitions, read/write logic
  mmu.rs          -- SV32/SV39 page table walk, TLB
  memory.rs       -- Guest RAM (Vec<u8>), byte/half/word access
  uart.rs         -- 16550 UART emulation
  clint.rs        -- Core Local Interruptor (timer, software interrupt)
  plic.rs         -- Platform Level Interrupt Controller
  virtio_blk.rs   -- Virtio block device (disk)
  virtio_net.rs   -- Virtio network device
  dtb.rs          -- Device Tree Blob generation
  loader.rs       -- ELF/binary image loader
  bridge.rs       -- Connect guest I/O to Geometry OS canvas/keyboard
```

Total estimated: ~2,500-3,000 lines of Rust.

---

## CPU Interpreter

### State

```rust
pub struct RiscvCpu {
    /// General purpose registers x[0..32]. x[0] is always 0.
    pub x: [u32; 32],
    /// Program counter
    pub pc: u32,
    /// Current privilege level: 0=User, 1=Supervisor, 3=Machine
    pub privilege: u8,
    /// CSR registers (mstatus, mtvec, mepc, etc.)
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

RV32I encodes all instructions in 32-bit words. The decode strategy:

1. Read 4 bytes from guest RAM at PC (little-endian)
2. Extract opcode bits [6:0]
3. Branch on opcode:
   - 0x33 -> R-type (ADD, SUB, SLL, etc.) -- funct3 + funct7 disambiguate
   - 0x13 -> I-type ALU (ADDI, SLTI, etc.)
   - 0x03 -> I-type Load (LB, LH, LW, LBU, LHU)
   - 0x23 -> S-type Store (SB, SH, SW)
   - 0x63 -> B-type Branch (BEQ, BNE, BLT, etc.)
   - 0x37 -> U-type LUI
   - 0x17 -> U-type AUIPC
   - 0x6F -> J-type JAL
   - 0x67 -> I-type JALR
   - 0x73 -> SYSTEM (ECALL, EBREAK, CSR*)
   - 0x0F -> FENCE (NOP for now)

4. Execute the operation, update registers/PC/memory
5. x[0] is hardwired to zero (write to x0 is discarded)

### Execute Loop

```rust
impl RiscvCpu {
    pub fn step(&mut self) -> Result<(), Trap> {
        let insn = self.memory.read_word(self.pc)?;
        let op = decode(insn)?;
        self.execute(op)?;
        Ok(())
    }

    pub fn run(&mut self, max_cycles: u64) -> Result<u64, Trap> {
        let mut cycles = 0;
        while cycles < max_cycles {
            match self.step() {
                Ok(()) => cycles += 1,
                Err(Trap::Ecall) => self.handle_ecall()?,
                Err(Trap::Ebreak) => self.handle_ebreak()?,
                Err(Trap::PageFault(addr, access)) => self.handle_page_fault(addr, access)?,
                Err(Trap::Interrupt(irq)) => self.handle_interrupt(irq)?,
            }
            // Check timer interrupts every 100 cycles
            if cycles % 100 == 0 {
                self.check_timer_interrupt();
            }
        }
        Ok(cycles)
    }
}
```

---

## Guest Memory

Separate from Geometry OS host RAM. Configurable size (default 128MB).

```rust
pub struct GuestMemory {
    /// Raw byte storage
    ram: Vec<u8>,
    /// Base address where RAM starts (typically 0x80000000)
    ram_base: u64,
}

impl GuestMemory {
    /// Read a byte from guest physical address
    pub fn read_byte(&self, addr: u64) -> u8;
    /// Read a half-word (16-bit, little-endian)
    pub fn read_half(&self, addr: u64) -> u16;
    /// Read a word (32-bit, little-endian)
    pub fn read_word(&self, addr: u64) -> u32;
    /// Write byte/half/word
    pub fn write_byte(&mut self, addr: u64, val: u8);
    pub fn write_half(&mut self, addr: u64, val: u16);
    pub fn write_word(&mut self, addr: u64, val: u32);
    /// Load a slice of bytes (for kernel image loading)
    pub fn load_slice(&mut self, addr: u64, data: &[u8]);
}
```

Address ranges:

```
0x00000000 - 0x00000FFF   Reserved / debug
0x00001000 - 0x00001FFF   QEMU boot ROM (optional)
0x00100000 - 0x00100FFF   UART 16550
0x00100100 - 0x001001FF   Virtio MMIO (disk)
0x00100200 - 0x001002FF   Virtio MMIO (net)
0x00200000 - 0x003FFFFF   CLINT (timer, software interrupt)
0x0C000000 - 0x0FFFFFFF   PLIC (interrupt controller)
0x80000000 - 0x87FFFFFF   Guest RAM (128MB default)
```

---

## Privilege Modes

Three privilege levels, matching the RISC-V spec:

| Level | Name | Who runs here |
|-------|------|---------------|
| 3 | Machine (M) | OpenSBI/firmware (or our hypervisor directly) |
| 1 | Supervisor (S) | Guest OS kernel (Linux) |
| 0 | User (U) | Guest user programs |

### Trap Handling

When a trap occurs (ECALL, page fault, timer interrupt, etc.):

1. Save current PC to mepc (or sepc if delegated to S-mode)
2. Set mcause (or scause) to the trap cause
3. Set mtval (or stval) to the faulting address (if applicable)
4. Set mstatus.MPP (or sstatus.SPP) to current privilege level
5. Set privilege to trap target (M or S depending on delegation)
6. Set PC to mtvec (or stvec) -- the trap vector

### MRET / SRET

Reverse the trap entry:

1. Set PC to mepc (or sepc)
2. Restore privilege from mstatus.MPP (or sstatus.SPP)
3. Clear MPP/SPP

---

## CSR Registers

Core CSRs for running Linux:

| Address | Name | Purpose |
|---------|------|---------|
| 0x300 | mstatus | Machine status (MIE, MPIE, MPP bits) |
| 0x301 | misa | ISA extensions (read-only, reports RV32IMA) |
| 0x302 | medeleg | Trap delegation to S-mode |
| 0x303 | mideleg | Interrupt delegation to S-mode |
| 0x304 | mie | Machine interrupt enable |
| 0x305 | mtvec | Machine trap vector |
| 0x340 | mscratch | Machine scratch (trap handler temp) |
| 0x341 | mepc | Machine exception PC |
| 0x342 | mcause | Machine trap cause |
| 0x343 | mtval | Machine trap value |
| 0x344 | mip | Machine interrupt pending |
| 0x100 | sstatus | Supervisor status (SIE, SPIE, SPP bits) |
| 0x104 | sie | Supervisor interrupt enable |
| 0x105 | stvec | Supervisor trap vector |
| 0x140 | sscratch | Supervisor scratch |
| 0x141 | sepc | Supervisor exception PC |
| 0x142 | scause | Supervisor trap cause |
| 0x143 | stval | Supervisor trap value |
| 0x144 | sip | Supervisor interrupt pending |
| 0x180 | satp | Supervisor address translation (page tables) |

CSR instructions: CSRRW (swap), CSRRS (read-set), CSRRC (read-clear), plus
immediate variants (CSRRWI, CSRRSI, CSRRCI).

---

## Virtual Memory (SV32)

When satp.Mode = 1 (SV32), virtual addresses are translated through 2-level
page tables.

### Virtual Address Format (32-bit)

```
[31:22] VPN[1]   - Level 1 index (10 bits, 1024 entries)
[21:12] VPN[0]   - Level 0 index (10 bits, 1024 entries)
[11:0]  Offset   - Page offset (12 bits, 4KB pages)
```

### Page Table Entry (32-bit)

```
[31:20] PPN      - Physical page number
[19:10] RSW      - Reserved for software
[9]     D        - Dirty (written to)
[8]     A        - Accessed
[7]     G        - Global mapping
[6]     U        - User accessible
[5]     X        - Execute
[4]     W        - Write
[3]     R        - Read
[2]     V        - Valid
[1:0]            - Reserved (must be 0 for leaf, or points to next level)
```

### Translation Steps

1. Read satp for root page table physical address
2. Index into L1 table with VPN[1] -> get PTE
3. If PTE is a leaf (R|W|X != 0): get PPN, done
4. If PTE is a pointer (R|W|X == 0 && V == 1): follow to L2 table
5. Index into L2 table with VPN[0] -> get PTE
6. If PTE is a leaf: get PPN, done
7. Otherwise: page fault

### TLB

64-entry fully-associative TLB. Keyed on (VPN, ASID). Flush on SFENCE.VMA
or ASID change in satp.

---

## Device Emulation

### UART 16550

The simplest device. Linux writes characters to the UART Transmit Holding
Register (THR), we collect them and display on the canvas.

```
MMIO base: 0x10000000
Offset 0x00: THR/RBR (write=read character, read=receive character)
Offset 0x01: IER (interrupt enable)
Offset 0x02: IIR/FCR (interrupt ID / FIFO control)
Offset 0x03: LCR (line control)
Offset 0x04: MCR (modem control)
Offset 0x05: LSR (line status -- THRE bit = TX empty, DR bit = data ready)
Offset 0x06: MSR (modem status)
Offset 0x07: SCR (scratch)
```

Bridge to Geometry OS:
- Guest writes to THR -> character appended to output buffer
- Output buffer periodically flushed to canvas via TEXT opcode
- Geometry OS keyboard input -> written to RBR -> DR bit set in LSR -> UART IRQ

### CLINT (Core Local Interruptor)

Provides timer and software interrupts.

```
MMIO base: 0x02000000
Offset 0x0000: msip (software interrupt pending)
Offset 0x4000: mtimecmp (timer compare value, 64-bit)
Offset 0xBFF8: mtime (timer count, 64-bit, incremented by hypervisor)
```

The hypervisor increments mtime periodically (e.g., every 100 VM cycles).
When mtime >= mtimecmp, set MTIP bit in mip -> triggers timer interrupt.

### PLIC (Platform Level Interrupt Controller)

Manages external interrupts from devices (UART, virtio).

```
MMIO base: 0x0C000000
Priority registers, enable registers, threshold, claim/complete
```

For simplicity, we support 8 interrupt sources:
- IRQ 1: UART
- IRQ 2: Virtio block device
- IRQ 3: Virtio network device

### Virtio Block Device

Virtio MMIO transport. Provides disk access to the guest.

```
MMIO base: 0x10001000
MagicValue, Version, DeviceID, VendorID, DeviceFeatures,
DriverFeatures, QueuePFN, QueueNum, QueueReady, QueueNotify,
InterruptStatus, InterruptACK, Status, Config
```

The disk image is a raw file loaded from the Geometry OS VFS (or host filesystem).
Queue processing follows the virtio spec: guest sets up descriptor rings,
hypervisor processes them.

### Device Tree Blob (DTB)

Generated at boot time, passed to the guest kernel in register a1.

```dts
/dts-v1/;
/ {
    #address-cells = <2>;
    #size-cells = <2>;
    compatible = "geometry-os,hypervisor";
    model = "Geometry OS RISC-V Hypervisor";

    memory@80000000 {
        device_type = "memory";
        reg = <0x0 0x80000000 0x0 0x8000000>; /* 128MB */
    };

    cpus { ... };

    chosen {
        stdout-path = "/soc/uart@10000000";
    };

    soc {
        uart@10000000 { compatible = "ns16550a"; reg = <...>; };
        virtio_blk@10001000 { ... };
        virtio_net@10002000 { ... };
    };
};
```

---

## Integration with Geometry OS

### HYPERVISOR Opcode

New Geometry OS opcode (Phase 37) that creates a RISC-V VM instance:

```
HYPERVISOR config_addr_reg
```

- Reads a config string from host RAM at config_addr_reg
- Config format: "kernel=path/to/Image dtb=path/to/dtb ram=128M"
- Spawns a RiscvVm in a background thread
- Guest UART output streams to the canvas
- Keyboard input forwarded to guest UART

### Bridge Layer (bridge.rs)

Connects the RISC-V guest to Geometry OS:

| Guest Action | Geometry OS Action |
|---|---|
| Write to UART THR | Render character on canvas (TEXT opcode or direct pixel) |
| Read from UART RBR | Return next character from keyboard buffer |
| Timer interrupt | Hypervisor increments mtime per host FRAME |
| Disk read (virtio-blk) | Read from host file via VFS |
| Network send (virtio-net) | Send via Geometry OS UDP (0xFFC port) or loopback |

### Performance Consideration

The interpreter runs in a tight Rust loop. Target: 10-50 million instructions
per second on the RTX 5090's host CPU. A minimal Linux boot needs ~50M
instructions. At 20 MIPS, boot completes in ~2.5 seconds.

---

## Relationship to Old Prototype

The old RISC-V implementation in `geometry_os123456789/systems/infinite_map_rs/src/riscv/`
runs on GPU compute shaders (WebGPU + AMDGPU DRM). Key differences:

| Aspect | Old (infinite_map_rs) | New (Geometry OS) |
|---|---|---|
| Execution | GPU compute shaders | Pure Rust CPU interpreter |
| Dependencies | wgpu, naga, AMDGPU DRM | None (pure Rust) |
| Testing | Requires GPU | cargo test anywhere |
| State | GPU buffer readback | Direct struct access |
| Program format | .rts.png (PNG encoded) | ELF / raw binary |
| Integration | Separate app | Module in Geometry OS |
| Register width | Both RV32 and RV64 | RV32 first, RV64 later |

We reuse the *knowledge* from the old code (CSR layouts, memory maps, UART
register offsets) but rewrite everything from scratch in the Geometry OS style.

---

## Testing Strategy

### Per-Instruction Tests (Phase 33)

Every RV32I instruction gets at least one test:

```rust
#[test]
fn test_add() {
    let mut cpu = RiscvCpu::new(GuestMemory::new(1024));
    cpu.x[1] = 10;
    cpu.x[2] = 20;
    // ADD x3, x1, x2 (funct7=0, rs1=1, rs2=2, funct3=0, rd=3, opcode=0x33)
    let insn = (0 << 25) | (2 << 20) | (1 << 15) | (0 << 12) | (3 << 7) | 0x33;
    cpu.memory.write_word(0x80000000, insn);
    cpu.pc = 0x80000000;
    cpu.step().unwrap();
    assert_eq!(cpu.x[3], 30);
    assert_eq!(cpu.pc, 0x80000004);
}
```

### Integration Tests (Phase 34+)

- Trap entry/return with privilege transitions
- SV32 page table walk with known mappings
- UART output verification
- Full boot test: load OpenSBI + Linux tinyconfig, verify "Linux version..." on UART

### Test Programs

Small RISC-V assembly programs that exercise specific features:

- `fibonacci.rv32` -- exercises ADDI, ADD, BLT, JAL
- `memcpy.rv32` -- exercises LW, SW, ADDI, BNE
- `privilege_trap.rv32` -- exercises ECALL, MRET, CSR access
- `page_table.rv32` -- sets up SV32, reads/writes through translations

---

## Build Path

```
Phase 33: RV32I Core
  -> Can run simple RISC-V programs (fibonacci, memcpy)
  -> Test suite with full instruction coverage

Phase 34: Privilege Modes
  -> Can run code that uses ECALL/MRET for U->S->M transitions
  -> Timer interrupts work

Phase 35: Virtual Memory
  -> Can run code that sets up page tables (SV32)
  -> Page faults handled correctly

Phase 36: Device Emulation
  -> UART output works (characters appear)
  -> Timer interrupts drive guest scheduler
  -> Disk I/O works via virtio-blk

Phase 37: Guest OS Boot
  -> Boot Linux RISC-V kernel
  -> Console output on canvas
  -> Keyboard input to guest
  -> Full interactive session
```

After Phase 37, Geometry OS is simultaneously:
- A real OS (processes, filesystem, shell, drivers)
- A hypervisor (runs guest operating systems)
- A development environment (write Geo asm, compile and run RISC-V programs)

The software ecosystem unlocks: any language with a RISC-V compiler (C, Rust,
Python, Go) can run inside Geometry OS via the guest Linux kernel.

---

## See Also

- **docs/NORTH_STAR.md** -- "After any change, ask: Is Geometry OS more like Linux/Windows/macOS than it was before?" Running Linux inside Geometry OS is the ultimate answer to that question.
- **docs/ARCHITECTURE.md** -- Existing Geometry OS architecture, opcodes, memory map.
- **docs/CANVAS_TEXT_SURFACE.md** -- How text rendering works. Guest UART output uses this pipeline.
- **Old prototype** -- `~/zion/projects/geometry_os/geometry_os123456789/systems/infinite_map_rs/src/riscv/` -- Reference for CSR layouts, device MMIO addresses, UART register offsets.
