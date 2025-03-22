use core::arch::global_asm;
use core::cell::Cell;

use alloc::boxed::Box;

use super::thread::Thread;
use super::{run_event_loop, Event, SCHEDULER};
use crate::sync::{disable_interrupts, restore_interrupts};

/// Per-core threading data, indicating the per-core stack pointers
/// and the active thread for each core.
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
        #[allow(clippy::declare_interior_mutable_const)]
        const INIT: CoreInfo = CoreInfo {
            thread: Cell::new(None),
            core_sp: Cell::new(0),
        };
        Self([INIT; 4])
    }

    /// Safety: Must only be called once, before any other accesses.
    pub unsafe fn init(&self) {
        for i in 0..4 {
            let stack = unsafe { &raw const crate::arch::boot::STACKS[i] };
            let stack_end = stack.wrapping_add(1);
            self.0[i].core_sp.set(stack_end as usize);
        }
    }

    // TODO: Make this non-reentrant?  (To prevent yield, etc from being
    // called within the callback)
    /// Run a callback with the current core's information.  The callback
    /// is run with interrupts disabled.
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

/// The register context of a thread.
#[repr(C)]
pub struct Context {
    pub regs: [usize; 31],
    /// Note: don't use this, use a helper method on HandlerContext
    /// or an equivalent.  (This will always resolve to the kernel sp,
    /// even for user threads.)
    #[deprecated = "Don't use the context sp directly; use get_sp/set_sp"]
    pub kernel_sp: usize,
    pub link_reg: usize,
    pub spsr: usize,
}

#[derive(Debug, PartialEq, PartialOrd)]
pub enum ExceptionLevel {
    EL0 = 0,
    EL1 = 1,
    EL2 = 2,
    EL3 = 3,
}

impl Context {
    pub fn current_el(&self) -> ExceptionLevel {
        let m = self.spsr & 0b1111;
        let el = m >> 2;
        let _sp_el0 = (m & 1) == 0;
        match el {
            0 => ExceptionLevel::EL0,
            1 => ExceptionLevel::EL1,
            2 => ExceptionLevel::EL2,
            3 => ExceptionLevel::EL3,
            _ => unreachable!(),
        }
    }
}

type EventQueue = super::scheduler::Queue<Event>;

/// An action to take on the thread descheduled by [`context_switch`].
pub enum SwitchAction<'a> {
    /// Re-add the thread to the scheduler's queue.
    Yield,
    /// Exit the thread and free its resources.
    FreeThread,
    /// Add the thread to the given wait queue, then unlock the spinlock.
    QueueAddUnlock(&'a EventQueue, &'a crate::sync::SpinLockInner),
}

/// An action to take on the thread descheduled by [`deschedule_thread`].
// Note: this must not have fields, as it must be passed in a single
// register.
#[repr(usize)]
pub enum DescheduleAction {
    /// Re-add the thread to the scheduler's queue.
    Yield,
    /// Exit the thread and free its resources.
    FreeThread,
}

/// Context switch away from the current thread,
pub fn context_switch(mut action: SwitchAction) {
    let (core_sp, thread) = CORES.with_current(|core| (core.core_sp.get(), core.thread.take()));
    let thread = thread.expect("attempt to context switch from an event");
    assert!(
        thread.is_kernel_thread(),
        "attempt to context switch with a user thread TCB"
    );
    unsafe { asm_context_switch(Some(thread), Some(&mut action), core_sp) }
}

/// Switch into the event loop for the current core, then operate on the
/// passed thread as specified by the [`DescheduleAction`]
pub unsafe fn deschedule_thread(action: DescheduleAction, thread: Option<Box<Thread>>) -> ! {
    let (core_sp, active_thread) = CORES.with_current(|c| (c.core_sp.get(), c.thread.take()));
    assert!(active_thread.is_none());
    unsafe { asm_deschedule_thread(thread, action, core_sp) }
}

/// Switch into the event loop for the current core.
pub unsafe fn enter_event_loop() -> ! {
    let (core_sp, active_thread) = CORES.with_current(|c| (c.core_sp.get(), c.thread.take()));
    assert!(active_thread.is_none());
    unsafe { asm_deschedule_thread(None, DescheduleAction::Yield, core_sp) }
}

#[allow(improper_ctypes)]
unsafe extern "C" {
    fn asm_context_switch(
        src_thread: Option<Box<Thread>>,
        action: Option<&mut SwitchAction>,
        core_sp: usize,
    );

    fn asm_deschedule_thread(
        thread: Option<Box<Thread>>,
        action: DescheduleAction,
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
    // Shift the stack pointer to make room for the saved context
    // (not all of the context is used, but the entire space needs
    // to be reserved.)
    sub sp, sp, #0x110

    // TODO: ... this will actually load uninitialized stack values
    // into the unused registers ... but they're caller-saved, so the
    // caller shouldn't use them ...

    // Save all callee-saved registers (x19-x29)
    str x19, [sp, #0x98]
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

    // Write the context address on the thread's stack into last_context
    // in the Thread struct.
    mov x4, sp
    str x4, [x0, #{thread_ctx_offset}]

    // NOTE: fall-through

switch_to_helper:
    // Switch to the provided stack, then run the context switch callback
    mov sp, x2
    bl context_switch_callback

    // NOTE: fall-through

restore_context:
    // Restore ELR and SPSR
    ldp x1, x2, [x0, #0x100]
    msr ELR_EL1, x1
    msr SPSR_EL1, x2

    // Restore all registers
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

    // Exception return; returns to the code at ELR_EL1,
    // and runs with the privileges and modes specified in SPSR_EL1
    eret

asm_deschedule_thread:
    mov sp, x2
    bl deschedule_thread_callback
    udf #0

"#,
    thread_ctx_offset = const core::mem::offset_of!(Thread, last_context),
);

/// Run by [`asm_context_switch`] after it saves the context and switches
/// to the per-core stack; it then moves the passed [`SwitchAction`] onto
/// the local stack, and calls [`context_switch_inner`].
#[unsafe(no_mangle)]
unsafe extern "C" fn context_switch_callback(
    thread: Option<Box<Thread>>,
    action: Option<&mut SwitchAction>,
) -> *mut Context {
    let action = action
        .map(|ptr| core::mem::replace(ptr, SwitchAction::Yield))
        .unwrap_or(SwitchAction::Yield);

    context_switch_inner(thread, action)
}

/// Run by [`asm_deschedule_thread`] after it switches to the per-core
/// stack; runs [`context_switch_inner`] with a translated action.
#[unsafe(no_mangle)]
unsafe extern "C" fn deschedule_thread_callback(
    thread: Option<Box<Thread>>,
    action: DescheduleAction,
) -> *mut Context {
    let action = match action {
        DescheduleAction::Yield => SwitchAction::Yield,
        DescheduleAction::FreeThread => SwitchAction::FreeThread,
    };
    context_switch_inner(thread, action)
}

/// Execute some post-context switch operations, then enter the event
/// loop.  This must be run on the correct per-core stack.
fn context_switch_inner(thread: Option<Box<Thread>>, action: SwitchAction<'_>) -> ! {
    if let Some(thread) = thread {
        match action {
            SwitchAction::Yield => {
                // Re-schedule the thread
                SCHEDULER.add_task(Event::ScheduleThread(thread))
            }
            SwitchAction::FreeThread => {
                // Free the thread
                drop(thread)
            }
            SwitchAction::QueueAddUnlock(queue, lock) => {
                // Add the thread to a queue, then unlock the lock.

                let mut queue_inner = queue.0.lock();
                queue_inner.push_back(Event::ScheduleThread(thread));
                lock.unlock();

                // TODO: unlocking this is risky, as it could be owned
                // by the thread being added to the queue.
                drop(queue_inner);

                // These objects borrow from the calling thread, so they
                // must not be used once the thread is on the (unlocked)
                // queue.
                #[allow(clippy::drop_non_drop)]
                drop(action);
            }
        }
    }

    // re-enable interrupts and run the event loop on this stack.
    unsafe { crate::sync::enable_interrupts() };
    unsafe { run_event_loop() }
}
