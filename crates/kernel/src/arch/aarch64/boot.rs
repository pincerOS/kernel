use super::memory::{INIT_TCR_EL1, INIT_TRANSLATION};
use core::arch::global_asm;

#[allow(unused)]
unsafe extern "C" {
    pub fn kernel_entry();
    pub fn kernel_entry_alt();
}

const STACK_SIZE_LOG2: usize = 16;
const STACK_SIZE: usize = 1 << STACK_SIZE_LOG2;

#[unsafe(no_mangle)]
#[unsafe(link_section = ".bss")]
pub static mut STACKS: [[u128; STACK_SIZE / 16]; 4] = [[0u128; STACK_SIZE / 16]; 4];

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


    // fill initial page table with 16 2mb pages for the kernel
    ldr x6, ={TRANSLATION_ENTRY}
    adr x5, kernel_entry
    bfc x5, 0, 21 // clears low bits to mask entry to 2MB page boundary
    orr x5, x5, x6 // apply rounded physical address to the translation entry
    adrl x6, KERNEL_TRANSLATION_TABLE
    add x7, x6, #16 * 8

    str x5, [x6], #8

1:  add x5, x5, #1 << 21
    str x5, [x6], #8
    cmp x6, x7
    b.ne 1b

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

drop_to_el1:
    mov x5, #(1 << 31)
    // orr x5, x5, #0x38
    msr hcr_el2, x5

    // Enable FPU
    // TODO: ref https://docs.kernel.org/5.19/arm64/booting.html for register init
    mov x5, #0
    msr cptr_el2, x5

    mrs x5, cpacr_el1
    mov x6, #(3 << 20) // Enable FPU in EL0 and EL1
    orr x5, x5, x6
    msr cpacr_el1, x5

    ldr x5, ={SCTLR_EL1}
    msr SCTLR_EL1, x5
    ldr x5, ={TCR_EL1}
    msr TCR_EL1, x5
    adrl x5, KERNEL_TRANSLATION_TABLE
    msr TTBR1_EL1, x5
    ldr x5, ={MAIR_EL1}
    msr MAIR_EL1, x5

    mov x5, #0b0101
    msr SPSR_EL2, x5

    ldr x5, =0xFFFFFFFFFE000000 // TODO: slightly cleaner way of encoding this?
    orr lr, lr, x5
    msr ELR_EL2, lr
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

// fn switch_kernel_vmem(ttbr1_el1: usize, tcr_el1: usize)
.global switch_kernel_vmem
switch_kernel_vmem:
    stp x29, x30, [sp, #-16]!
    mov x29, sp

    // Mask all interrupts
    mrs x6, DAIF
    msr DAIFSet, #0b1111

    mrs x4, TTBR0_EL1
    mrs x3, TTBR1_EL1
    msr TTBR0_EL1, x3

    isb
    dsb sy
    tlbi vmalle1is
    dsb sy

    adrl x5, switch_kernel_vmem_in_phys
    and x5, x5, ((1 << 25) - 1)
    blr x5

    msr TTBR0_EL1, x4

    isb
    dsb sy
    tlbi vmalle1is
    dsb sy

    // Restore interrupt mask
    msr DAIF, x6

    ldp x29, x30, [sp], #16
    ret

switch_kernel_vmem_in_phys:
    msr TCR_EL1, x1
    msr TTBR1_EL1, x0

    isb
    dsb sy
    tlbi vmalle1is
    dsb sy

    ret

// fn switch_user_tcr_el1(tcr_el1: usize)
.global switch_user_tcr_el1
switch_user_tcr_el1:
    msr TCR_EL1, x0

    isb
    dsb sy
    tlbi vmalle1is
    dsb sy

    ret
",
    STACK_SIZE_LOG2 = const STACK_SIZE_LOG2,
    TCR_EL1 = const INIT_TCR_EL1,
    TRANSLATION_ENTRY = const INIT_TRANSLATION,
    SCTLR_EL1 = const (
        (1 << 12) | // enable instruction caching
        (1 << 4) | // enable EL0 stack pointer alignment
        (1 << 3) | // enable EL1 stack pointer alignment
        (1 << 2) | // enable data caching
        (1 << 1) | // enable alignment faults
        1           // enable EL1&0 virtual memory
    ),
    MAIR_EL1 = const (
        (0b01000100 << 16) | // Entry 2: normal noncacheable memory
        (0b00000000 << 8)  | // Entry 1: device memory
        (0b11111111 << 0)    // Entry 0: normal memory
    )
);
