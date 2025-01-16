
# Based on the linker script from rust-embeded/rust-raspberrypi-OS-tutorials:
# https://github.com/rust-embedded/rust-raspberrypi-OS-tutorials/blob/master/02_runtime_init/src/bsp/raspberrypi/kernel.ld
#
# The license of the original is preserved as follows:
#
/* SPDX-License-Identifier: MIT OR Apache-2.0
 *
 * Copyright (c) 2018-2022 Andre Richter <andre.o.richter@gmail.com>
 */

PAGE_SIZE = 64K;

__rpi_phys_dram_start_addr = 0x00000;

# The base address at which the Raspberry pi bootloader loads the
# kernel into physical memory.
__rpi_phys_binary_start_addr = 0x80000;

ENTRY(__rpi_phys_binary_start_addr)

/* Flags:
 *     4 == R
 *     5 == RX
 *     6 == RW
 *
 * Segments are marked PT_LOAD below so that the ELF file provides virtual and physical addresses.
 * It doesn't mean all of them need actually be loaded.
 */
PHDRS {
    segment_stack PT_LOAD FLAGS(6);
    segment_code  PT_LOAD FLAGS(5);
    segment_data  PT_LOAD FLAGS(6);
}

SECTIONS {
    . = __rpi_phys_dram_start_addr;

    . = __rpi_phys_binary_start_addr;
    __code_start = .;
    .text : {
        KEEP(*(.text.kernel_entry))
        *(.text*)
    } :segment_code

    .rodata : ALIGN(8) { *(.rodata*) } :segment_code

    . = ALIGN(PAGE_SIZE);
    __code_end = .;

    __data_start = .;
    .data : { *(.data) *(.data.*) } :segment_data

    .bss (NOLOAD) : ALIGN(16) {
        __bss_start = .;
        . = . + 16;
        *(.bss) *(.bss.*)
        . = ALIGN(16);
        __bss_end = .;
    } :segment_data

    . = ALIGN(PAGE_SIZE);
    __data_end = .;
}
