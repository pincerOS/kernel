use core::arch::{asm, global_asm};

use super::context::{deschedule_thread, Context, DescheduleAction, CORES};
use crate::arch::halt;
use crate::sync::HandlerTableInner;
use crate::uart;

global_asm!(
    r"
.macro save_context label, handler, arg
    // TODO: is the stack pointer safe at this point?
    // Depending on the mode, this may be on either SP_EL1
    // or SP_EL0; if this was from an interrupt/exception
    // in EL1 already, we risk overflowing the stack here.
    sub sp, sp, #0x120

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

    add x1, sp, #0x120
    stp x30, x1, [sp, #0xF0]

    mrs x1, ELR_EL1  // Exception link register (return addr)
    mrs x2, SPSR_EL1 // Saved program status (basically x86's EFLAGS)
    mrs x3, ESR_EL1  // Exception data
    mrs x5, SP_EL0   // Saved stack pointer (if coming from SP_EL0)

    stp x1, x2, [sp, #0x100]
    str x5, [sp, #0x110]

    mov x0, sp
    .ifnb \arg
    mov x4, \arg
    .endif

save_context_br_\label:
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
    save_context curEL_sp0_sync, exception_handler_unhandled, 0
.org 0x080 // IRQ, vIRQ
    save_context curEL_sp0_irq, exception_handler_unhandled, 1
.org 0x100 // FIQ, vFIQ
    save_context curEL_sp0_fiq, exception_handler_unhandled, 2
.org 0x180 // SError, vSError
    save_context curEL_sp0_serr, exception_handler_unhandled, 3

// Current exception level, SP_ELx for x > 0
.org 0x200
    save_context curEL_sp1_sync, exception_handler_example, 4
.org 0x280
    save_context curEL_sp1_irq, exception_handler_unhandled, 5
.org 0x300
    save_context curEL_sp1_fiq, exception_handler_unhandled, 6
.org 0x380
    save_context curEL_sp1_serr, exception_handler_unhandled, 7

// Lower exception level, Aarch64
.org 0x400
    save_context lowEL64_sync, exception_handler_user, 8
.org 0x480
    save_context lowEL64_irq, exception_handler_unhandled, 9
.org 0x500
    save_context lowEL64_fiq, exception_handler_unhandled, 10
.org 0x580
    save_context lowEL64_serr, exception_handler_unhandled, 11

// Lower exception level, Aarch32
.org 0x600
    save_context lowEL32_sync, exception_handler_unhandled, 12
.org 0x680
    save_context lowEL32_irq, exception_handler_unhandled, 13
.org 0x700
    save_context lowEL32_fiq, exception_handler_unhandled, 14
.org 0x780
    save_context lowEL32_serr, exception_handler_unhandled, 15
"
);

type ExceptionHandler = unsafe extern "C" fn(
    ctx: &mut Context,
    elr: u64,
    spsr: u64,
    esr: u64,
    arg: u64,
) -> *mut Context;

#[allow(dead_code)]
unsafe extern "C" {
    static mut save_context_br_curEL_sp0_sync: u32;
    static mut save_context_br_curEL_sp0_irq: u32;
    static mut save_context_br_curEL_sp0_fiq: u32;
    static mut save_context_br_curEL_sp0_serr: u32;
    static mut save_context_br_curEL_sp1_sync: u32;
    static mut save_context_br_curEL_sp1_irq: u32;
    static mut save_context_br_curEL_sp1_fiq: u32;
    static mut save_context_br_curEL_sp1_serr: u32;
    static mut save_context_br_lowEL64_sync: u32;
    static mut save_context_br_lowEL64_irq: u32;
    static mut save_context_br_lowEL64_fiq: u32;
    static mut save_context_br_lowEL64_serr: u32;
    static mut save_context_br_lowEL32_sync: u32;
    static mut save_context_br_lowEL32_irq: u32;
    static mut save_context_br_lowEL32_fiq: u32;
    static mut save_context_br_lowEL32_serr: u32;
}

fn encode_bl(addr: usize, caller_addr: u64) -> u32 {
    let diff = addr as i64 - caller_addr as i64;
    let shifted = diff >> 2;
    assert!(
        (diff & 0b11 == 0) && shifted >= (-1 << 25) && shifted < (1 << 25),
        "offset out of range: {diff}"
    );
    ((0b100101 << 26) | (shifted & ((1 << 26) - 1))) as u32
}

fn overwrite_target(target: *mut u32, addr: usize) {
    let value = encode_bl(addr, target as u64);
    unsafe {
        core::ptr::write_volatile(target, value.to_le());
        // Magic sequence to flush instruction cache?
        core::arch::asm!(
            "dc cvau, {0}",
            "dsb ish",
            "ic ivau, {0}",
            "dsb ish",
            "isb",
            in(reg) target,
        );
    }
}

pub unsafe fn override_irq_handler(handler: ExceptionHandler) {
    let addr = handler as usize;
    overwrite_target(&raw mut save_context_br_curEL_sp1_irq, addr);
    overwrite_target(&raw mut save_context_br_lowEL64_irq, addr);
}

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
    arr[0b101100] = "FPE (64 bit)";
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

unsafe fn read_far_el1() -> usize {
    let far: usize;
    unsafe {
        asm!(
            "mrs {}, far_el1",
            out(reg) far,
            options(nomem, nostack, preserves_flags)
        );
    }
    far
}

unsafe fn read_ttbr0_el1() -> usize {
    let ttbr0: usize;
    unsafe {
        asm!(
            "mrs {}, ttbr0_el1",
            out(reg) ttbr0,
            options(nomem, nostack, preserves_flags)
        );
    }
    ttbr0
}

#[unsafe(no_mangle)]
unsafe extern "C" fn exception_handler_example(
    ctx: &mut Context,
    elr: u64,
    spsr: u64,
    esr: u64,
    arg: u64,
) -> *mut Context {
    let far = unsafe { read_far_el1() };

    let exception_class = esr >> 26;
    let class_name = *EXCEPTION_CLASS
        .get(exception_class as usize)
        .unwrap_or(&"unspecified");
    let _insn_len = if ((esr >> 25) & 1) != 0 { 4 } else { 2 };

    if uart::UART.is_initialized() {
        println!("Received exception: elr={elr:#x} spsr={spsr:#010x} esr={esr:#010x} far={far:#010x} (class {exception_class:#x} / {class_name}) {arg}");
        println!("{:#?}", ctx);
    }

    match exception_class {
        _ => halt(),
    }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn exception_handler_unhandled(
    _ctx: &mut Context,
    _elr: u64,
    _spsr: u64,
    _esr: u64,
    _arg: u64,
) -> *mut Context {
    halt();
}

#[unsafe(no_mangle)]
unsafe extern "C" fn exception_handler_user(
    ctx: &mut Context,
    elr: u64,
    spsr: u64,
    esr: u64,
    arg: u64,
) -> *mut Context {
    let far = unsafe { read_far_el1() };
    let ttbr0 = unsafe { read_ttbr0_el1() };

    let exception_class = esr >> 26;
    let class_name = *EXCEPTION_CLASS
        .get(exception_class as usize)
        .unwrap_or(&"unspecified");
    let _insn_len = if ((esr >> 25) & 1) != 0 { 4 } else { 2 };

    match exception_class {
        0x15 => {
            // supervisor call
            let arg = esr & 0xFFFF;
            if let Some(handler) = get_syscall_handler(arg as usize) {
                unsafe { handler(ctx) }
            } else {
                if uart::UART.is_initialized() {
                    println!("Received exception from usermode: elr={elr:#x} spsr={spsr:#010x} esr={esr:#010x} far={far:#010x} (class {exception_class:#x} / {class_name}) {arg}");
                }
                println!("Unknown syscall number {arg:#x}; stopping user thread");

                let thread = CORES.with_current(|core| core.thread.take());
                let mut thread = thread.expect("usermode syscall without active thread");
                unsafe { thread.save_context(ctx.into(), false) };
                unsafe { deschedule_thread(DescheduleAction::FreeThread, Some(thread)) }
            }
        }
        _ => {
            if uart::UART.is_initialized() {
                println!("Received exception from usermode: elr={elr:#x} spsr={spsr:#010x} esr={esr:#010x} far={far:#010x} (class {exception_class:#x} / {class_name}) {arg}");
                println!("ttbr0={ttbr0:#010x}");
                println!("{:#?}", ctx);
            }
            halt()
        }
    }
}

type SyscallHandler = unsafe fn(ctx: &mut Context) -> *mut Context;

pub struct SyscallTable(HandlerTableInner<256>);

impl SyscallTable {
    pub fn get(&self, num: usize) -> Option<SyscallHandler> {
        let v = self.0.get(num);
        (v != 0).then(|| unsafe { core::mem::transmute::<usize, _>(v) })
    }
    pub fn set(&self, num: usize, func: Option<SyscallHandler>) {
        self.0.set(num, func.map(|f| f as usize).unwrap_or(0));
    }
}

static SYSCALL_HANDLERS: SyscallTable = SyscallTable(HandlerTableInner::new(0));

pub unsafe fn register_syscall_handler(num: usize, handler: SyscallHandler) {
    SYSCALL_HANDLERS.set(num, Some(handler));
}

pub fn get_syscall_handler(num: usize) -> Option<SyscallHandler> {
    SYSCALL_HANDLERS.get(num)
}
