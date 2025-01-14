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
    mov x5, #0b0101
    msr ELR_EL2, lr
    msr SPSR_EL2, x5
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
    STACK_SIZE_LOG2 = const STACK_SIZE_LOG2
);
