use core::arch::global_asm;
use core::cell::Cell;

use alloc::boxed::Box;

use crate::event::{run_event_loop, Event, SCHEDULER};
use crate::sync::{disable_interrupts, restore_interrupts};
use crate::thread::Thread;

pub static CORES: AllCores = AllCores::new();

// Assume cache-line size of 64, align to avoid false sharing
// (device tree /cpus/cpu@0/d-cache-line-size and /cpus/l2-cache0/cache-line-size)
#[repr(align(64))]
pub struct CoreInfo {
    pub thread: Cell<Option<Box<Thread>>>,
    pub core_sp: Cell<usize>,
}

pub struct AllCores([CoreInfo; 4]);

impl AllCores {
    const fn new() -> Self {
        const INIT: CoreInfo = CoreInfo {
            thread: Cell::new(None),
            core_sp: Cell::new(0),
        };
        Self([INIT; 4])
    }

    /// Safety: Must only be called once, before any other accesses.
    pub unsafe fn init(&self) {
        for i in 0..4 {
            // TODO: UB
            let stack = unsafe { &crate::arch::boot::STACKS[i] };
            self.0[i].core_sp.set(stack.as_ptr_range().end as usize);
        }
    }

    pub fn with_current<F, T>(&self, f: F) -> T
    where
        F: FnOnce(&CoreInfo) -> T,
    {
        let state = unsafe { disable_interrupts() };
        let core_id = crate::arch::core_id() & 0b11;
        let res = f(&self.0[core_id as usize]);
        unsafe { restore_interrupts(state) };
        res
    }
}

/// Safety:
/// - per-core values are only accessible to one core at a time; since
///   `with_current` disables interrupts around any operations accessing
///   the per-core values, and references to those values cannot escape
///   the closure, they will remain on that core.
///   TODO: ensure that functions like yield cannot be called from within
///   `with_current`.
/// - &CoreInfo must not be Send, so the static references to it cannot
///   be passed between threads
///
/// Note that Send/Sync need a single definition of a context; ie.
/// either Send/Sync with respect to threads, or Send/Sync with respect
/// to cores, but we can't have both in the same program, as threads
/// can switch between cores.
unsafe impl Sync for AllCores {}

#[repr(C)]
pub struct Context {
    pub regs: [usize; 31],
    pub sp: usize,
    pub link_reg: usize,
    pub spsr: usize,
}

type EventQueue = crate::scheduler::Queue<Event>;

pub enum SwitchAction<'a> {
    Yield,
    FreeThread,
    QueueAddUnlock(&'a EventQueue, &'a crate::sync::SpinLockInner),
}

pub fn context_switch(mut action: SwitchAction) {
    let (core_sp, thread) = CORES.with_current(|core| (core.core_sp.get(), core.thread.take()));
    let thread = thread.expect("attempt to context switch from an event");
    unsafe { asm_context_switch(Some(thread), Some(&mut action), core_sp) };
}

#[allow(improper_ctypes)]
extern "C" {
    pub fn asm_context_switch(
        src_thread: Option<Box<Thread>>,
        action: Option<&mut SwitchAction>,
        core_sp: usize,
    );
    pub fn switch_to_helper(
        src_thread: Option<Box<Thread>>,
        action: Option<&mut SwitchAction>,
        core_sp: usize,
    ) -> !;
    pub fn restore_context(ctx: *mut Context) -> !;
}

global_asm!(
    r#"
.global asm_context_switch
.global switch_to_helper
.global restore_context

// extern "C" fn asm_context_switch(
//     src_thread: Option<Box<Thread>>, (*mut Thread)
//     action: Option<&mut SwitchAction>, (*mut SwitchAction)
//     core_sp: usize,
// )
asm_context_switch:
    sub sp, sp, #0x110

    # stp x0, x1, [sp, #0x00]
    # stp x2, x3, [sp, #0x10]
    # stp x4, x5, [sp, #0x20]
    # stp x6, x7, [sp, #0x30]
    # stp x8, x9, [sp, #0x40]
    # stp x10, x11, [sp, #0x50]
    # stp x12, x13, [sp, #0x60]
    # stp x14, x15, [sp, #0x70]
    # stp x16, x17, [sp, #0x80]
    stp x18, x19, [sp, #0x90]
    stp x20, x21, [sp, #0xA0]
    stp x22, x23, [sp, #0xB0]
    stp x24, x25, [sp, #0xC0]
    stp x26, x27, [sp, #0xD0]
    stp x28, x29, [sp, #0xE0]

    add x4, sp, #0x110
    stp x30, x4, [sp, #0xF0]

    // TODO: how to restore the state of PSTATE, at least parts that
    // need to be preserved by context switches?

    mov x4, lr       // Fake exception link register
    mov x5, #0b0101  // fake program status, staying in EL1 (TODO)
    stp x4, x5, [sp, #0x100]

    mov x4, sp
    str x4, [x0, #{thread_ctx_offset}]
    # ldr x0, [x1, #{thread_ctx_offset}]

    // NOTE: fall-through

switch_to_helper:
    mov sp, x2
    bl context_switch_inner

    // NOTE: fall-through

restore_context:
    ldp x1, x2, [x0, #0x100]
    msr ELR_EL1, x1
    msr SPSR_EL1, x2

    ldp x2, x3, [x0, #0x10]
    ldp x4, x5, [x0, #0x20]
    ldp x6, x7, [x0, #0x30]
    ldp x8, x9, [x0, #0x40]
    ldp x10, x11, [x0, #0x50]
    ldp x12, x13, [x0, #0x60]
    ldp x14, x15, [x0, #0x70]
    ldp x16, x17, [x0, #0x80]
    ldp x18, x19, [x0, #0x90]
    ldp x20, x21, [x0, #0xA0]
    ldp x22, x23, [x0, #0xB0]
    ldp x24, x25, [x0, #0xC0]
    ldp x26, x27, [x0, #0xD0]
    ldp x28, x29, [x0, #0xE0]
    ldp x30, x1, [x0, #0xF0]
    mov sp, x1

    ldp x0, x1, [x0, #0x00]

    eret
"#,
    thread_ctx_offset = const core::mem::offset_of!(Thread, last_context),
);

#[no_mangle]
unsafe extern "C" fn context_switch_inner(
    thread: Option<Box<Thread>>,
    action: Option<&mut SwitchAction>,
) -> *mut Context {
    let action = action
        .map(|ptr| core::mem::replace(ptr, SwitchAction::Yield))
        .unwrap_or(SwitchAction::Yield);

    if let Some(thread) = thread {
        match action {
            SwitchAction::Yield => SCHEDULER.add_task(Event::ScheduleThread(thread)),
            SwitchAction::FreeThread => drop(thread),
            SwitchAction::QueueAddUnlock(queue, lock) => {
                let mut queue_inner = queue.0.lock();
                queue_inner.push_back(Event::ScheduleThread(thread));
                lock.unlock();

                // TODO: unlocking this is risky, as it could be owned
                // by the thread being added to the queue.
                drop(queue_inner);

                // These objects borrow from the calling thread, so they
                // must not be used once the thread is on the (unlocked)
                // queue.
                drop(action);
            }
        }
    }

    unsafe { crate::event::run_event_loop() };
}

#[repr(C)]
pub enum DescheduleAction {
    Yield,
    FreeThread,
}

#[allow(improper_ctypes)]
extern "C" {
    pub fn deschedule_thread(
        core_sp: usize,
        thread: Option<Box<Thread>>,
        action: DescheduleAction,
    ) -> !;
}

core::arch::global_asm!(
    "
.global deschedule_thread
deschedule_thread:
    mov sp, x0
    bl deschedule_thread_inner
    udf #0
"
);

#[no_mangle]
unsafe extern "C" fn deschedule_thread_inner(
    _core_sp: usize,
    thread: Option<Box<Thread>>,
    action: DescheduleAction,
) -> ! {
    unsafe { crate::sync::enable_interrupts() };
    if let Some(thread) = thread {
        match action {
            DescheduleAction::Yield => SCHEDULER.add_task(Event::ScheduleThread(thread)),
            DescheduleAction::FreeThread => drop(thread),
        }
    }
    unsafe { run_event_loop() }
}
