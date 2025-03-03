
ENTRY(_start);

PHDRS {
    segment_main PT_LOAD;
}

SECTIONS {
    . = 0x300000;
    .text : { *(.text) *(.text*) } :segment_main
    .rodata : ALIGN(8) { *(.rodata) *(.rodata*) } :segment_main
    .data : { *(.data) *(.data.*) } :segment_main

    .bss (NOLOAD) : ALIGN(16) { *(.bss) *(.bss.*) } :segment_main
}
