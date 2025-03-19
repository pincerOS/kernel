use alloc::boxed::Box;
use core::ptr::NonNull;

use crate::process::ProcessRef;

use super::context::{context_switch, Context, SwitchAction, CORES};
use super::{Event, SCHEDULER};

/// A handle for a kernel or user thread, which owns its stack, and
/// while the thread isn't running, stores the saved register state of
/// the thread.
pub struct Thread {
    pub last_context: NonNull<Context>,
    pub stack: NonNull<[u128]>,
    // Stored on the thread's stack
    func: Option<NonNull<dyn Callback + Send>>,

    pub context: Option<Context>,
    pub user_regs: Option<UserRegs>,
    pub process: Option<crate::process::ProcessRef>,
}

pub struct UserRegs {
    pub sp_el0: usize,
    pub ttbr0_el1: usize,
    pub usermode: bool,
}

unsafe impl Send for Thread {}
unsafe impl Sync for Thread {}

impl Thread {
    /// Create a kernel thread from the given stack and closure
    unsafe fn from_fn<F>(stack: NonNull<[u128]>, func: F) -> Box<Self>
    where
        F: FnOnce() + Send + 'static,
    {
        let init_end = stack.len() * size_of::<u128>();
        let end = unsafe { stack.cast::<u128>().byte_add(init_end) };

        let align = align_of::<F>();
        let align_off = end.cast::<u8>().align_offset(align);
        let offset = (align - align_off).rem_euclid(align) + size_of::<F>();
        let ptr = unsafe { end.cast::<F>().byte_sub(offset) };

        assert!(ptr.is_aligned());
        assert!(ptr.addr() >= stack.addr() && (ptr.addr().get() + size_of::<F>()) <= end.addr().get(),
            "misaligned Thread::from_fn; ptr: {ptr:p}; stack: {stack:p}; end: {end:p}; size: {}, align: {}",
            size_of::<F>(), align_of::<F>());
        unsafe { ptr.write(func) };

        let fn_ptr = NonNull::new(ptr.as_ptr() as *mut (dyn Callback + Send)).unwrap();

        let stack_offset = offset.next_multiple_of(size_of::<u128>());
        unsafe { Self::new(stack, stack_offset, Some(fn_ptr)) }
    }

    /// Create a new user thread with the given stack pointer, initial
    /// program counter, and initial page table (`ttbr0`).
    pub unsafe fn new_user(process: ProcessRef, sp: usize, entry: usize) -> Box<Self> {
        let data = Context {
            regs: [0; 31],
            sp: 0,
            link_reg: entry,
            spsr: 0b0000, // Run in EL0
        };

        let mut thread = Box::new(Thread {
            stack: (&mut [] as &mut [u128]).into(),
            last_context: NonNull::dangling(),
            func: None,
            context: Some(data),
            user_regs: Some(UserRegs {
                sp_el0: sp,
                ttbr0_el1: process.get_ttbr0(),
                usermode: true,
            }),
            process: Some(process),
        });
        thread.last_context = thread.context.as_mut().unwrap().into();
        thread
    }

    /// Create a new kernel thread with the given stack and a function
    /// to run when starting the thread.
    ///
    /// Stack must have been created with [`Box::into_raw`]
    unsafe fn new(
        stack: NonNull<[u128]>,
        stack_offset: usize,
        func: Option<NonNull<dyn Callback + Send>>,
    ) -> Box<Self> {
        let init_end = stack.len() * size_of::<u128>() - stack_offset;
        let end = unsafe { stack.cast::<u128>().byte_add(init_end) };

        // reuse the lowest region of the stack as the initial context
        assert!(init_end >= size_of::<Context>());
        let context = unsafe { end.cast::<Context>().sub(1) };
        assert!(context.is_aligned());

        let data = Context {
            regs: [0; 31],
            sp: end.as_ptr() as usize,
            link_reg: init_thread as usize,
            spsr: 0b0101, // Stay in EL1, using the EL1 sp
        };
        unsafe { core::ptr::write(context.as_ptr(), data) };

        Box::new(Thread {
            stack,
            last_context: context,
            func,
            context: None,
            user_regs: None,
            process: None,
        })
    }

    pub fn is_kernel_thread(&self) -> bool {
        self.user_regs.as_ref().map(|u| !u.usermode).unwrap_or(true)
    }
    pub fn is_user_thread(&self) -> bool {
        !self.is_kernel_thread()
    }

    /// Save the given register context into the thread's state.
    ///
    /// `stable` indicates whether `context` points to a stable location
    /// that will be accessible at the next call to `enter_thread`.  If
    /// you aren't sure, false is always a safe option.
    ///
    /// `stable` should be true if the thread is a kernel thread, as the
    /// context will be saved at the top of the stack associated with
    /// this thread.
    /// `stable` should be false if the thread is a user thread, or if
    /// the context for a kernel thread was saved anywhere but the top
    /// of its stack.
    ///
    /// If `stable` is false, this copies the register context into a
    /// space in the [`Thread`] struct; otherwise, it saves a pointer to
    /// the provided context.
    ///
    /// If this is a user thread, it saves the *current* values of
    /// `TTBR0_EL1` and `SP_EL0` into the user's stack.
    pub unsafe fn save_context(&mut self, context: NonNull<Context>, stable: bool) {
        if let Some(user) = &mut self.user_regs {
            unsafe { core::arch::asm!("mrs {}, TTBR0_EL1", out(reg) user.ttbr0_el1) };
            unsafe { core::arch::asm!("mrs {}, SP_EL0", out(reg) user.sp_el0) };
        }

        if stable {
            // context is on the allocated stack in the heap, not the per-core
            // stack; it will not be overwritten before being used again.
            self.last_context = context;
        } else {
            // context is on the temporary kernel stack, so we have to copy it
            // into a more permanent location
            self.context = Some(unsafe { context.read() });
            self.last_context = self.context.as_mut().unwrap().into();
        }
    }

    /// Switch into the thread, restoring its context
    pub unsafe fn enter_thread(self: Box<Self>) -> ! {
        let next_ctx = self.last_context.as_ptr();

        // Disable interrupts (preemption) until context is
        // restored.  (interrupts will be re-enabled by eret)
        // The timer interrupt assumes that if CORE.thread is set,
        // then there is a preemptable thread running.
        unsafe { crate::sync::disable_interrupts() };

        if let Some(user) = &self.user_regs {
            let ctx = unsafe { &mut *next_ctx };

            // TODO: proper tlb flush stuff for switching address spaces
            unsafe {
                core::arch::asm!(
                    "msr TTBR0_EL1, {0}",
                    "dsb sy",
                    "tlbi vmalle1is",
                    "dsb sy",
                    in(reg) user.ttbr0_el1
                )
            };

            // TODO: assert that we aren't running on SP_EL0 already...
            // (kernel threads should probably run in EL1/SP_EL0 mode)
            unsafe { core::arch::asm!("msr SP_EL0, {}", in(reg) user.sp_el0) };

            if user.usermode {
                let core_sp = CORES.with_current(|core| core.core_sp.get());
                ctx.sp = core_sp;
            }
        }

        let old = CORES.with_current(|core| core.thread.replace(Some(self)));
        assert!(old.is_none());

        // switch into the thread
        unsafe { super::context::restore_context(next_ctx) };
        // unreachable
    }
}

impl Drop for Thread {
    fn drop(&mut self) {
        let b = unsafe { Box::from_raw(self.stack.as_ptr()) };
        drop(b);
    }
}

// https://users.rust-lang.org/t/invoke-mut-dyn-fnonce/59356
trait Callback: FnOnce() {
    unsafe fn call(&mut self);
}

impl<F: FnOnce()> Callback for F {
    unsafe fn call(&mut self) {
        unsafe { core::ptr::read(self)() }
    }
}

const STACK_SIZE: usize = 16384;

/// Spawn a kernel thread that runs the given closure.
pub fn thread<F>(f: F)
where
    F: FnOnce() + Send + 'static,
{
    let stack = Box::<[u128]>::new_uninit_slice(STACK_SIZE / size_of::<u128>());
    let stack = NonNull::new(Box::into_raw(stack) as *mut [u128]).unwrap();
    let thread = unsafe { Thread::from_fn(stack, f) };

    SCHEDULER.add_task(Event::ScheduleThread(thread));
}

#[unsafe(no_mangle)]
extern "C" fn init_thread() {
    let func = CORES.with_current(|core| {
        let mut thread = core.thread.take().unwrap();
        let func = thread.func.take().unwrap();
        core.thread.set(Some(thread));
        func
    });

    unsafe { (*func.as_ptr()).call() };

    stop();
}

/// Yield control of the current thread, running it again in the future.
pub fn yield_() {
    context_switch(SwitchAction::Yield);
}

/// Exit the current thread
pub fn stop() -> ! {
    context_switch(SwitchAction::FreeThread);
    unreachable!()
}
