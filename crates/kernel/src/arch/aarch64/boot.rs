use crate::memory::{INIT_TCR_EL1, INIT_TRANSLATION};
use core::arch::global_asm;

#[allow(unused)]
extern "C" {
    pub fn kernel_entry();
    pub fn kernel_entry_alt();
}

const STACK_SIZE_LOG2: usize = 16;
const STACK_SIZE: usize = 1 << STACK_SIZE_LOG2;

#[no_mangle]
#[link_section = ".bss"]
pub static STACKS: [[u128; STACK_SIZE / 16]; 4] = [[0u128; STACK_SIZE / 16]; 4];

global_asm!(
    r"
    // Version of armv7's adrl instruction; always expands to 2
    // instructions, even when it may not need to.
    // (TODO: is a better version possible without potentially
    // requiring load-time relocations?)
    .macro adrl, dst, symbol
    adrp \dst, \symbol
    add  \dst, \dst, :lo12:\symbol
    .endm

    .section .text.kernel_entry

    // Called from armstub8.S:
    // https://github.com/raspberrypi/tools/blob/439b6198a9b340de5998dd14a26a0d9d38a6bcac/armstubs/armstub8.S#L92
    // See also: Linux's init code... https://github.com/raspberrypi/linux/blob/dfff38316c1284c30c68d02cc424bad0562cf253/arch/arm64/kernel/head.S
.global kernel_entry
kernel_entry:
    // Check core id, halt if non-zero
    // (non-portable, assumes raspi 4 cores)
    mrs x6, mpidr_el1
    and x6, x6, #0x3
    cbnz x6, halt

    // zero bss region
    adrl x5, __bss_start
    adrl x6, __bss_end
1:  stp xzr, xzr, [x5], #16
    cmp x5, x6
    b.ne 1b


    // set kernel 2mb mapping
    adr x5, kernel_entry
    bfc x5, 0, 21 // clears low bits to mask entry to 2MB page boundary
    ldr x6, ={TRANSLATION_ENTRY}
    orr x5, x5, x6 // apply rounded physical address to the translation entry
    adr x6, KERNEL_TRANSLATION_TABLE
    str x5, [x6]

    ldr w4, [x0, 4] // load size of DTB (as big-endian u32) into register for memory mapping in rust code

    bl drop_to_el1

    adrl x5, __exception_vector_start
    msr VBAR_EL1, x5

    bl init_core_sp

    b kernel_entry_rust

.global kernel_entry_alt
kernel_entry_alt:
    bl drop_to_el1

    adrl x5, __exception_vector_start
    msr VBAR_EL1, x5

    bl init_core_sp

    b kernel_entry_rust_alt

    // TODO: somehow this sometimes gets triggered twice on core 0?
    // Taking exception 1 [Undefined Instruction] on CPU 0
    // ...from EL1 to EL1
    // ...with ESR 0x0/0x2000000
    // ...with ELR 0x8005c
    // ...to EL1 PC 0x80a00 PSTATE 0x3c5
drop_to_el1:

    mov x5, #(1 << 31)
    // orr x5, x5, #0x38
    msr hcr_el2, x5
    ldr x5, ={SCTLR_EL1}
    msr SCTLR_EL1, x5
    ldr x5, ={TCR_EL1}
    msr TCR_EL1, x5
    adr x5, KERNEL_TRANSLATION_TABLE
    msr TTBR1_EL1, x5
    mov x5, #0b0101
    msr SPSR_EL2, x5
    ldr x5, =0xFFFFFFFFFE000000 // TODO: slightly cleaner way of encoding this?
    orr lr, lr, x5
    msr ELR_EL2, lr
    ldr x5, =0b010001000000000011111111 // Entry 0 is normal memory, entry 1 is device memory, 2 = normal noncacheable memory
    msr MAIR_EL1, x5
    isb
    eret

init_core_sp:
    mrs x6, mpidr_el1
    and x6, x6, #0x3
    add x6, x6, 1

    adrl x5, STACKS
    lsl x6, x6, #{STACK_SIZE_LOG2}
    add sp, x5, x6
    ret

.global halt
halt:
    nop
1:  wfe
    b 1b
",
    STACK_SIZE_LOG2 = const STACK_SIZE_LOG2,
    TCR_EL1 = const INIT_TCR_EL1,
    TRANSLATION_ENTRY = const INIT_TRANSLATION,
    SCTLR_EL1 = const (
        (1 << 11) | // enable instruction caching
        (1 << 4) | // enable EL0 stack pointer alignment
        (1 << 3) | // enable EL1 stack pointer alignment
        (1 << 2) | // enable data caching
        (1 << 1) | // enable alignment faults
        1           // enable EL1&0 virtual memory
    )
);
