
ENTRY(entry);

PHDRS {
    segment_main PT_LOAD;
}

SECTIONS {
    . = 0x200000;

    .text : { *(.text.entry) *(.text) *(.text*) } :segment_main
    .rodata : ALIGN(8) { *(.rodata) *(.rodata*) } :segment_main
    .data : { *(.data) *(.data.*) } :segment_main

    .bss (NOLOAD) : ALIGN(16) {
        __bss_start = .;
        . = . + 16;
        *(.bss) *(.bss.*)
        . = ALIGN(16);
        __bss_end = .;
    } :segment_main
}
