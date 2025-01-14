// TODO: design of context switching and interrupt frames
//
// Option 1:
// - interrupt frame saves all regs on stack
// - context switch by restoring a different frame when
//   returning from the interrupt handler
//
// Option 2:
// - interrupt frame saves all caller-saved regs
// - context switching saves and restores callee-saved regs
// - interrupt return restores caller-saved regs
// - downside: registers are in two places on the stack
//
// Option 3:
// - interrupt frame saves all regs in TCB, not stack
// - context switch by restoring a different frame when
//   returning from the interrupt handler

// # Core design questions:
//
// ## Approach to threading
//
// Preemptive:
// - Less concern for DOS within kernel, easier to write kernel code
// - Increases prevalence of race-conditions / synchronization bugs
//   within the kernel
// - In the simple approach, requires a dedicated stack per kernel thread
//   (though it's straightforward to avoid needing kernel stacks for
//   inactive user threads)
//
// Cooperative (stackful)
// - Similar implementation complexity and design considerations as preemptive
// - Risk of DOS within kernel due to loops/slow code without wait points
// - Reduces race condition prevalence
//
// Cooperative (stackless, async)
// - Hard to write; have to deal with Rust's Pinn / Future apis
//
// Cooperative (continuation-passing-style)
// - Effectively just an event loop, easy to implement
// - Can be used in parallel to other threading systems

use core::arch::{asm, global_asm};
use core::cell::Cell;
use core::ptr::NonNull;

use alloc::boxed::Box;
use alloc::collections::VecDeque;

use crate::sync::{disable_interrupts, restore_interrupts, InterruptSpinLock};

#[repr(C)]
pub struct Context {
    pub regs: [usize; 31],
    pub sp: usize,
    pub link_reg: usize,
    pub spsr: usize,
}

// Assume cache-line size of 64, align to avoid false sharing
// (device tree /cpus/cpu@0/d-cache-line-size and /cpus/l2-cache0/cache-line-size)
#[repr(align(64))]
pub struct CoreInfo {
    pub thread: Cell<Option<Box<Thread>>>,
    pub helper_sp: Cell<usize>,
    pub no_preempt: Cell<bool>,
}

pub struct AllCores([CoreInfo; 4]);

impl AllCores {
    const fn new() -> Self {
        const INIT: CoreInfo = CoreInfo {
            thread: Cell::new(None),
            helper_sp: Cell::new(0),
            no_preempt: Cell::new(false),
        };
        Self([INIT; 4])
    }

    pub fn init(&self) {
        for i in 0..4 {
            let stack = &crate::boot::STACKS[i];
            self.0[i].helper_sp.set(stack.as_ptr_range().end as usize);
        }
    }

    pub fn current(&self) -> &CoreInfo {
        let core_id = crate::core_id() & 0b11;
        &self.0[core_id as usize]
    }
}

/// Safety:
/// - per-core values are only accessible to one core at a time; since
///   current() can return &'static values, that means threads must never
///   move between cores.  (This is currently violated!!!)
/// - To do this safely, current must run with a limited scope, and prevent
///   preemption / yielding during that period
/// - &CoreInfo must not be Send, so the static references to it cannot
///   be passed between threads
///
/// Note that Send/Sync need a single definition of a context; ie.
/// either Send/Sync with respect to threads, or Send/Sync with respect
/// to cores, but we can't have both in the same program, as threads
/// can switch between cores.
unsafe impl Sync for AllCores {}

pub static CORES: AllCores = AllCores::new();

pub struct Thread {
    pub last_context: NonNull<Context>,
    pub stack: NonNull<[u128]>,
    // TODO: inline if small
    // TODO: reuse the stack or thread allocations
    // (can't make Thread unsized until ptr::metadata is stable)
    pub func: Option<Box<dyn FnOnce()>>,
}

impl Thread {
    pub fn new(stack: NonNull<[u128]>, func: Option<Box<dyn FnOnce()>>) -> Self {
        // reuse the lowest region of the stack as the initial context
        assert!(stack.len() >= size_of::<Context>());
        let end = unsafe { stack.cast::<u128>().add(stack.len()) };
        let context = unsafe { end.cast::<Context>().sub(1) };
        assert!(context.is_aligned());

        let data = Context {
            regs: [0; 31],
            sp: end.as_ptr() as usize,
            link_reg: init_thread as usize,
            spsr: 0b0101, // Stay in EL1, using the EL1 sp
        };
        unsafe {
            core::ptr::write(context.as_ptr(), data);
        }

        Thread {
            stack,
            last_context: context,
            func,
        }
    }
}

impl Drop for Thread {
    fn drop(&mut self) {
        let b = unsafe { Box::from_raw(self.stack.as_ptr()) };
        drop(b);
    }
}

pub struct ThreadScheduler {
    queue: ThreadQueue,
}

impl ThreadScheduler {
    const fn new() -> Self {
        ThreadScheduler {
            queue: ThreadQueue::new(),
        }
    }

    pub fn add_task(&self, thread: Box<Thread>) {
        self.queue.add(thread);
        // unblock WFEs on other cores
        unsafe {
            asm!("sev");
        }
    }
    pub fn add_all(&self, queue: &ThreadQueue) {
        self.queue.0.lock().append(&mut *queue.0.lock());
    }

    pub fn wait_for_task(&self) -> Box<Thread> {
        loop {
            if let Some(c) = self.queue.pop() {
                break c;
            }
            // armv8 a-profile reference: G1.19.1 Wait For Event and Send Event
            unsafe {
                asm!("wfe");
            }
        }
    }

    pub unsafe fn run_on_core(&self) -> ! {
        let core = CORES.current();
        unsafe { switch_to_helper(None, None, core.helper_sp.get()) };
    }
}

unsafe impl Sync for ThreadScheduler {}

pub static SCHEDULER: ThreadScheduler = ThreadScheduler::new();

pub struct ThreadQueue(InterruptSpinLock<VecDeque<Box<Thread>>>);

impl ThreadQueue {
    pub const fn new() -> Self {
        Self(InterruptSpinLock::new(VecDeque::new()))
    }
    pub fn add(&self, thread: Box<Thread>) {
        self.0.lock().push_back(thread);
    }
    pub fn pop(&self) -> Option<Box<Thread>> {
        self.0.lock().pop_front()
    }
}

unsafe impl Send for ThreadQueue {}
unsafe impl Sync for ThreadQueue {}

#[allow(improper_ctypes)]
extern "C" {
    pub fn _context_switch(
        src_thread: Option<Box<Thread>>,
        action: Option<&mut SwitchAction>,
        helper_sp: usize,
    );
    pub fn switch_to_helper(
        src_thread: Option<Box<Thread>>,
        action: Option<&mut SwitchAction>,
        helper_sp: usize,
    ) -> !;
    pub fn restore_context(ctx: *mut Context) -> !;
}

global_asm!(
    r#"
// extern "C" fn _context_switch(
//     src_thread: Option<Box<Thread>>, (*mut Thread)
//     action: Option<&mut SwitchAction>, (*mut SwitchAction)
//     helper_sp: usize,
// )
.global _context_switch
_context_switch:
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

.global switch_to_helper
switch_to_helper:
    mov sp, x2
    bl context_switch_inner

    // NOTE: fall-through

.global restore_context
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

pub enum SwitchAction<'a> {
    Yield,
    FreeThread,
    QueueAddUnlock(&'a ThreadQueue, &'a crate::sync::SpinLockInner),
}

#[no_mangle]
unsafe extern "C" fn context_switch_inner(
    thread: Option<Box<Thread>>,
    action: Option<&mut SwitchAction>,
) -> *mut Context {
    let mut action = action
        .map(|ptr| core::mem::replace(ptr, SwitchAction::Yield))
        .unwrap_or(SwitchAction::Yield);

    if let Some(thread) = thread {
        match action {
            SwitchAction::Yield => SCHEDULER.add_task(thread),
            SwitchAction::FreeThread => drop(thread),
            SwitchAction::QueueAddUnlock(queue, lock) => {
                let mut queue_inner = queue.0.lock();
                queue_inner.push_back(thread);
                lock.unlock();
                drop(queue_inner); // TODO: unlocking this is risky, as it could be owned by the thread...
                action = SwitchAction::Yield; // these objects borrow from the calling thread,
                                              // so they must not be used once the thread is on the (unlocked) queue
            }
        }
    }

    let target_thread = match action {
        SwitchAction::Yield | SwitchAction::FreeThread | SwitchAction::QueueAddUnlock(..) => {
            SCHEDULER.wait_for_task()
        }
    };

    let next_ctx = target_thread.last_context.as_ptr();
    let old = CORES.current().thread.replace(Some(target_thread));
    assert!(old.is_none());
    next_ctx
}

const STACK_SIZE: usize = 16384;

pub fn thread<F>(f: F)
where
    F: FnOnce() + Send + 'static,
{
    let stack = Box::<[u128]>::new_uninit_slice(STACK_SIZE / size_of::<u128>());
    let stack = NonNull::new(Box::into_raw(stack) as *mut [u128]).unwrap();
    let thread = Box::new(Thread::new(stack, Some(Box::new(f))));

    SCHEDULER.queue.add(thread);
}

pub fn context_switch(mut action: SwitchAction) {
    let int = disable_interrupts();
    let core = CORES.current();
    let helper_sp = core.helper_sp.get();
    let thread = core.thread.take().unwrap();
    restore_interrupts(int);

    unsafe { _context_switch(Some(thread), Some(&mut action), helper_sp) };
}

pub fn yield_() {
    context_switch(SwitchAction::Yield);
}

pub fn stop() -> ! {
    context_switch(SwitchAction::FreeThread);
    unreachable!()
}

#[no_mangle]
extern "C" fn init_thread() {
    let func;
    {
        let int = disable_interrupts();
        let core = CORES.current();
        core.no_preempt.set(true);
        restore_interrupts(int);

        let mut thread = core.thread.take().unwrap();
        func = thread.func.take().unwrap();
        core.thread.set(Some(thread));

        core.no_preempt.set(false);
    }

    func();

    stop();
}
