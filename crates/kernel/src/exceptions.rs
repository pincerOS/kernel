use core::arch::{asm, global_asm};

use crate::arch::halt;
use crate::context::Context;
use crate::uart;

// TODO:
// - Save FAR_EL1 for instruction abort/data aborts (faulting addr)

global_asm!(
    r"
.macro save_context handler, arg
    // TODO: is the stack pointer safe at this point?
    // Depending on the mode, this may be on either SP_EL1
    // or SP_EL0; if this was from an interrupt/exception
    // in EL1 already, we risk overflowing the stack here.
    sub sp, sp, #0x110

    stp x0, x1, [sp, #0x00]
    stp x2, x3, [sp, #0x10]
    stp x4, x5, [sp, #0x20]
    stp x6, x7, [sp, #0x30]
    stp x8, x9, [sp, #0x40]
    stp x10, x11, [sp, #0x50]
    stp x12, x13, [sp, #0x60]
    stp x14, x15, [sp, #0x70]
    stp x16, x17, [sp, #0x80]
    stp x18, x19, [sp, #0x90]
    stp x20, x21, [sp, #0xA0]
    stp x22, x23, [sp, #0xB0]
    stp x24, x25, [sp, #0xC0]
    stp x26, x27, [sp, #0xD0]
    stp x28, x29, [sp, #0xE0]

    add x1, sp, #0x110
    stp x30, x1, [sp, #0xF0]

    mrs x1, ELR_EL1  // Exception link register (return addr)
    mrs x2, SPSR_EL1 // Saved program status (basically x86's EFLAGS)
    mrs x3, ESR_EL1  // Exception data

    stp x1, x2, [sp, #0x100]

    mov x0, sp
    .ifnb \arg
    mov x4, \arg
    .endif

    // Run handler with args;
    // x0 = saved context ptr
    // x1 = exception pc
    // x2 = program status
    // x3 = exception data
    bl \handler

    b restore_context
.endm


.section .text.exception_vector
.global __exception_vector_start

.balign 2048
__exception_vector_start:

// Current exception level, SP_EL0
.org 0x000 // synchronous exceptions
    save_context exception_handler_unhandled, 0
.org 0x080 // IRQ, vIRQ
    save_context exception_handler_unhandled, 1
.org 0x100 // FIQ, vFIQ
    save_context exception_handler_unhandled, 2
.org 0x180 // SError, vSError
    save_context exception_handler_unhandled, 3

// Current exception level, SP_ELx for x > 0
.org 0x200
    save_context exception_handler_example, 4
.org 0x280
    save_context exception_handler_irq, 5
.org 0x300
    save_context exception_handler_unhandled, 6
.org 0x380
    save_context exception_handler_unhandled, 7

// Lower exception level, Aarch32
.org 0x400
    save_context exception_handler_unhandled, 8
.org 0x480
    save_context exception_handler_unhandled, 9
.org 0x500
    save_context exception_handler_unhandled, 10
.org 0x580
    save_context exception_handler_unhandled, 11

// Lower exception level, Aarch64
.org 0x600
    save_context exception_handler_unhandled, 12
.org 0x680
    save_context exception_handler_unhandled, 13
.org 0x700
    save_context exception_handler_unhandled, 14
.org 0x780
    save_context exception_handler_unhandled, 15
"
);

// Docs: Armv8-A ARM - D23.2.41 ESR_EL2, Exception Syndrome Register (EL2)
static EXCEPTION_CLASS: [&str; 64] = {
    let mut arr = ["unspecified"; 64];
    arr[0b000000] = "unknown";
    arr[0b000001] = "wf* trap";
    arr[0b000011] = "mcr or mrc trap (32 bit)";
    arr[0b000100] = "mcrr or mrrc trap (32 bit)";
    arr[0b000101] = "mcr or mrc trap (32 bit)";
    arr[0b000110] = "ldc/stc trap";
    arr[0b000111] = "fp disabled trap";
    arr[0b001010] = "ld/st64b trap";
    arr[0b001100] = "mrrc trap";
    arr[0b001101] = "bti exception";
    arr[0b001110] = "illegal exec state";
    arr[0b010001] = "SVC (32 bit)";
    arr[0b010100] = "msrr/mrrs trap (64 bit)";
    arr[0b010101] = "SVC (64 bit)";
    arr[0b010100] = "msr/mrs trap (64 bit)";
    arr[0b011001] = "SVE disabled trap";
    arr[0b011011] = "TME fault";
    arr[0b011100] = "PAC failure";
    arr[0b011101] = "SME disabled trap";
    arr[0b100000] = "instruction abort (lower EL)";
    arr[0b100001] = "instruction abort (same EL)";
    arr[0b100010] = "PC alignment fault";
    arr[0b100100] = "data abort (lower EL)";
    arr[0b100101] = "data abort (same EL)";
    arr[0b100110] = "SP alignment fault";
    arr[0b101000] = "FPE (32 bit)";
    arr[0b101100] = "FPE (364 bit)";
    arr[0b101101] = "GCS exception";
    arr[0b101111] = "SError exception";
    arr[0b110000] = "breakpoint (lower EL)";
    arr[0b110001] = "breakpoint (same EL)";
    arr[0b110010] = "software step (lower EL)";
    arr[0b110011] = "software step (same EL)";
    arr[0b110100] = "watchpoint (lower EL)";
    arr[0b110101] = "watchpoint (same EL)";
    arr[0b111000] = "breakpoint (32 bit)";
    arr[0b111100] = "breakpoint (64 bit)";
    arr[0b111101] = "PMU exception";
    arr
};

#[no_mangle]
unsafe extern "C" fn exception_handler_example(
    ctx: &mut Context,
    elr: u64,
    spsr: u64,
    esr: u64,
    arg: u64,
) -> *mut Context {
    // far_el1 should be preserved up to this point
    // TODO: need to ensure that LLVM doesn't reorder this load
    // after an operation that could overwrite it (yield-like ops)
    let far_el1: usize;
    unsafe {
        asm! {
            "mrs {}, far_el1",
            out(reg) far_el1,
            options(nomem, nostack, preserves_flags)
        }
    }

    let exception_class = esr >> 26;
    let class_name = *EXCEPTION_CLASS
        .get(exception_class as usize)
        .unwrap_or(&"unspecified");
    let _insn_len = if ((esr >> 25) & 1) != 0 { 4 } else { 2 };

    if uart::UART.is_initialized() {
        println!("Received exception: elr={elr:#x} spsr={spsr:#010x} esr={esr:#010x} (class {exception_class:#x} / {class_name}) {arg}");
        println!("(Faulting address, if relevant: 0x{far_el1:X})");
    }

    match exception_class {
        0x15 => {
            // supervisor call
            let arg = esr & 0xFFFF;
            println!("Got syscall with number: {arg:#x}");
            match arg {
                1 => {
                    ctx.regs[0] = ctx.regs[0] * ctx.regs[1];
                    return ctx;
                }
                _ => {
                    println!("Unknown syscall number {arg:#x}");
                    halt()
                }
            }
        }
        _ => halt(),
    }
}

#[no_mangle]
unsafe extern "C" fn exception_handler_irq(
    ctx: &mut Context,
    _elr: u64,
    _spsr: u64,
    _esr: u64,
    _arg: u64,
) -> *mut Context {
    let mut irq = crate::device::IRQ_CONTROLLER.get().lock();
    let core = crate::arch::core_id() & 0b11;
    let _source = unsafe { irq.irq_source(core) };
    // source is 2048 for local timer interrupt

    irq.timer_reload();
    drop(irq);

    unsafe { crate::event::timer_handler(ctx) }
}

#[no_mangle]
unsafe extern "C" fn exception_handler_unhandled(
    _ctx: &mut Context,
    _elr: u64,
    _spsr: u64,
    _esr: u64,
    _arg: u64,
) -> *mut Context {
    halt();
}
