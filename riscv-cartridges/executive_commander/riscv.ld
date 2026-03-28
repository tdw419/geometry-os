/* Linker script for RISC-V GPU cartridges
 *
 * Memory map (matches riscv-shader.wgsl):
 *   0x00001000 - 0x00001FFF: .text (program instructions)
 *   0x00002000 - 0x00002FFF: .rodata (read-only data)
 *   0x00003000 - 0x00003FFF: .data/.bss/stack (RAM)
 *   0x00004000 - 0x0000400F: MMIO (UART output)
 */

MEMORY
{
    TEXT   (rx)  : ORIGIN = 0x00001000, LENGTH = 4K
    RODATA (r)   : ORIGIN = 0x00002000, LENGTH = 4K
    RAM    (rwx) : ORIGIN = 0x00003000, LENGTH = 4K
}

ENTRY(_start)

SECTIONS
{
    .text : {
        *(.text.start)
        *(.text .text.*)
    } > TEXT

    .rodata : {
        *(.rodata .rodata.*)
    } > RODATA

    .data : {
        *(.data .data.*)
    } > RAM

    .bss : {
        __bss_start = .;
        *(.bss .bss.*)
        *(COMMON)
        __bss_end = .;
    } > RAM

    _stack_top = ORIGIN(RAM) + LENGTH(RAM);
}
