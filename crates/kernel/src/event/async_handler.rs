use core::arch::asm;
use core::future::Future;
use core::pin::Pin;
use core::task::Poll;

use alloc::boxed::Box;

use super::context::{enter_event_loop, Context, CORES};
use super::scheduler::Priority;
use super::thread::UserRegs;
use super::{task, thread};
use crate::event;

pub trait AsyncFnCustomSend<Args> {
    type Output;
    type CallOnceFuture: Future<Output = Self::Output> + Send;
    fn call(self, args: Args) -> Self::CallOnceFuture;
}

impl<A, F, Fu> AsyncFnCustomSend<A> for F
where
    F: FnOnce(A) -> Fu,
    Fu: Future + Send,
{
    type Output = Fu::Output;
    type CallOnceFuture = Fu;
    fn call(self, args: A) -> Self::CallOnceFuture {
        self(args)
    }
}

/// Run an asynchronous exception handler for a thread, such as a syscall
/// or page fault handler.
///
/// This should be called directly from a handler function, and its
/// return value should be returned from the handler.  The async closure
/// will be polled once; if it finished immediately (does not yield), it
/// will directly return back to the user thread without a full context
/// switch.  If the async closure yields, it will suspend and save the
/// state of the user thread.
///
/// The closure may at any point resume the execution of the user thread
/// with [`HandlerContext::resume`] or [`HandlerContext::resume_final`];
/// `resume` will schedule the thread to be run (or run it immediately,
/// if the closure hasn't yet yielded) while leaving the rest of the
/// closure to run concurrently; `resume_final` will schedule the thread
/// to run after the closure completes.
///
/// The closure may access and modify the registers and memory of the
/// associated thread, through [`HandlerContext::regs`] and
/// [`HandlerContext::with_user_vmem`].  `regs` returns a reference to
/// the register context, though it cannot be stored across `await`
/// points (since the context is moved at the first yield-point).
///
/// [`HandlerContext::with_user_vmem`] runs a callback with access to
/// the virtual address space of the user.  It is not sound to access
/// user pointers from outside of these callbacks, as the user address
/// space may not have been restored during task context switches within
/// the kernel.
///
/// The handler must return the [`ResumedContext`] struct returned by
/// either [`HandlerContext::resume`] or [`HandlerContext::resume_final`].
///
/// TODO: expose a mechanism to take manual control over the thread --
/// adding it to wait queues, freeing it, etc.
#[must_use]
pub fn run_async_handler<Fn>(ctx: &mut Context, handler: Fn) -> *mut Context
where
    Fn: for<'a> AsyncFnCustomSend<HandlerContext<'a>, Output = ResumedContext> + 'static,
{
    // TODO: ensure preemption/interrupts disabled before this point
    let thread = CORES.with_current(|core| core.thread.take());
    let mut thread = thread.expect("usermode syscall without active thread");
    let mut ctx = core::ptr::NonNull::from(ctx);
    thread.last_context = ctx;

    // TODO: only save user regs if the handler accesses them?
    unsafe { thread.save_user_regs() };

    unsafe { crate::sync::enable_interrupts() };

    // TODO: avoid allocating in the syscall handler?
    let mut future = new_handler_future(thread, handler);

    let task_id = task::TASKS.alloc_task_slot();
    let priority = Priority::Normal;
    let waker = task::create_waker(task_id, priority);
    let mut context = core::task::Context::from_waker(&waker);

    // Run the handler future until it suspends once
    match Future::poll(future.as_mut(), &mut context) {
        Poll::Ready(()) => {
            // If the handler finished immediately, remove the task
            // and return back to the user directly.
            task::TASKS.remove_task(task_id);

            let thread = unsafe { &mut *future.data.thread.get() }.take().unwrap();

            if future.data.user_regs_changed.get() {
                if let Some(regs) = &thread.user_regs {
                    unsafe { thread::Thread::restore_user_regs(regs, ctx.as_mut()) };
                }
            }

            CORES.with_current(|core| core.thread.set(Some(thread)));
            ctx.as_ptr()
        }
        Poll::Pending => {
            future.data.in_handler.set(false);

            if !future.data.suspend_thread.get() {
                let thread = unsafe { &mut *future.data.thread.get() }.take().unwrap();
                CORES.with_current(|core| core.thread.set(Some(thread)));

                let woken = task::TASKS.return_task(task_id, task::Task::new(future, priority));
                if woken {
                    event::SCHEDULER.add_task(event::Event::async_task(task_id, priority));
                }

                // Return back to the user context
                ctx.as_ptr()
            } else {
                // The handler yielded; suspend the current thread, and set
                // up the future to reschedule the thread when it finishes.
                let thread = unsafe { &mut *future.data.thread.get() }.as_mut().unwrap();
                unsafe { thread.save_context(ctx, thread.is_kernel_thread()) };

                let woken = task::TASKS.return_task(task_id, task::Task::new(future, priority));
                if woken {
                    event::SCHEDULER.add_task(event::Event::async_task(task_id, priority));
                }

                // Switch back to the event loop.
                unsafe { enter_event_loop() };
            }
        }
    }
}

pub fn run_event_handler<Fn>(ctx: &mut Context, handler: Fn) -> *mut Context
where
    Fn: for<'a> FnOnce(HandlerContext<'a>) -> ResumedContext + 'static,
{
    // TODO: ensure preemption/interrupts disabled before this point
    let thread = CORES.with_current(|core| core.thread.take());
    let mut thread = thread.expect("usermode syscall without active thread");
    let mut ctx = core::ptr::NonNull::from(ctx);
    thread.last_context = ctx;

    // TODO: only save user regs if the handler accesses them?
    unsafe { thread.save_user_regs() };

    unsafe { crate::sync::enable_interrupts() };

    let data = new_handlers(thread);
    let handler_ctx = HandlerContext { outer_data: &data };

    let _resumed = handler(handler_ctx);

    let thread = unsafe { &mut *data.thread.get() }.take().unwrap();

    if data.user_regs_changed.get() {
        if let Some(regs) = &thread.user_regs {
            unsafe { thread::Thread::restore_user_regs(regs, ctx.as_mut()) };
        }
    }

    CORES.with_current(|core| core.thread.set(Some(thread)));

    ctx.as_ptr()
}

struct OuterData {
    in_handler: core::cell::Cell<bool>,
    suspend_thread: core::cell::Cell<bool>,
    user_regs_changed: core::cell::Cell<bool>,
    thread: core::cell::UnsafeCell<Option<Box<thread::Thread>>>,
}

pub struct HandlerContext<'a> {
    outer_data: &'a OuterData,
}

impl<'outer> HandlerContext<'outer> {
    fn cur_thread_mut(&mut self) -> &mut Option<Box<thread::Thread>> {
        unsafe { &mut *self.outer_data.thread.get() }
    }
    pub fn cur_thread(&self) -> &thread::Thread {
        unsafe { &*self.outer_data.thread.get() }
            .as_deref()
            .unwrap()
    }
    pub fn cur_process(&self) -> Option<&alloc::sync::Arc<crate::process::Process>> {
        self.cur_thread().process.as_ref()
    }

    /// Save the current context into a stable location in the thread,
    /// and return it; for re-scheduling a thread elsewhere.
    pub fn detach_thread(mut self) -> Box<thread::Thread> {
        let mut thread = self.cur_thread_mut().take().unwrap();
        let stable = thread.is_kernel_thread();
        unsafe { thread.save_context(thread.last_context, stable) };
        thread
    }

    pub async fn resume(mut self) -> ResumedContext {
        if self.outer_data.in_handler.get() {
            // Haven't yielded yet; yield to the handler and tell
            // it to resume running the user thread.
            // TODO: priority of user thread vs kernel task?
            self.outer_data.suspend_thread.set(false);
            task::yield_future().await;
        } else {
            let thread = self.cur_thread_mut().take().unwrap();
            event::SCHEDULER.add_task(event::Event::schedule_thread(thread));
        }
        ResumedContext(())
    }

    pub fn resume_final(self) -> ResumedContext {
        ResumedContext(())
    }

    pub fn resume_return(mut self, x0: usize) -> ResumedContext {
        self.regs().regs[0] = x0;
        ResumedContext(())
    }

    pub fn regs(&mut self) -> ThreadRefMut<'_, Context> {
        let thread = self.cur_thread_mut().as_mut().unwrap();
        ThreadRefMut {
            inner: unsafe { thread.last_context.as_mut() },
            marker: core::marker::PhantomData,
        }
    }

    pub fn user_regs(&mut self) -> ThreadRefMut<'_, Option<UserRegs>> {
        self.outer_data.user_regs_changed.set(true);
        let thread = self.cur_thread_mut().as_mut().unwrap();
        ThreadRefMut {
            inner: &mut thread.user_regs,
            marker: core::marker::PhantomData,
        }
    }

    fn enable_user_vmem(&self) {
        if self.outer_data.in_handler.get() {
            // User virtual memory is still enabled, haven't yielded yet
        } else {
            // This handler has already yielded; user virtual memory likely
            // hasn't been switched back to the correct address space.
            let thread = self.cur_thread();
            let user_ttbr0 = thread.user_regs.as_ref().unwrap().ttbr0_el1;

            let cur_ttbr0: usize;
            unsafe { asm!("mrs {0}, TTBR0_EL1", out(reg) cur_ttbr0) };

            // If the page table has changed, switch back to this thread's
            // address space.
            if cur_ttbr0 != user_ttbr0 {
                unsafe {
                    asm!("msr TTBR0_EL1, {0}", "isb", "dsb sy", "tlbi vmalle1is", "dsb sy", in(reg) user_ttbr0)
                };
            }
        }
    }

    pub fn with_user_vmem<F, O>(&self, callback: F) -> O
    where
        F: FnOnce() -> O,
    {
        self.enable_user_vmem();
        callback()
    }

    pub fn with_user_vmem_async<F, O>(
        &self,
        callback: F,
    ) -> impl Future<Output = O> + use<'outer, F, O>
    where
        F: Future<Output = O>,
    {
        struct UserVmemWrap<'a, F> {
            this: HandlerContext<'a>,
            fut: F,
        }
        impl<'a, F, O> Future for UserVmemWrap<'a, F>
        where
            F: Future<Output = O>,
        {
            type Output = O;
            fn poll(self: Pin<&mut Self>, cx: &mut core::task::Context<'_>) -> Poll<Self::Output> {
                self.as_ref().this.enable_user_vmem();
                let inner = unsafe { self.map_unchecked_mut(|f| &mut f.fut) };
                inner.poll(cx)
            }
        }
        UserVmemWrap {
            this: HandlerContext {
                outer_data: self.outer_data,
            },
            fut: callback,
        }
    }
}

unsafe impl Send for HandlerContext<'_> {}

pub struct ThreadRefMut<'a, T> {
    inner: &'a mut T,
    // To ensure ThreadRefMut: !Send, and can't be kept across await points.
    marker: core::marker::PhantomData<*mut ()>,
}
impl<T> core::ops::Deref for ThreadRefMut<'_, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &*self.inner
    }
}
impl<T> core::ops::DerefMut for ThreadRefMut<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut *self.inner
    }
}

pub struct ResumedContext(());

struct HandlerFuture<F> {
    inner: core::mem::MaybeUninit<F>,
    data: OuterData,
}

fn new_handler_future<F>(
    thread: Box<thread::Thread>,
    f: F,
) -> Pin<Box<HandlerFuture<<F as AsyncFnCustomSend<HandlerContext<'static>>>::CallOnceFuture>>>
where
    F: for<'a> AsyncFnCustomSend<HandlerContext<'a>, Output = ResumedContext>,
{
    let mut this = Box::pin(HandlerFuture {
        inner: core::mem::MaybeUninit::uninit(),
        data: OuterData {
            thread: core::cell::UnsafeCell::new(Some(thread)),
            in_handler: core::cell::Cell::new(true),
            suspend_thread: core::cell::Cell::new(true),
            user_regs_changed: core::cell::Cell::new(false),
        },
    });

    let this_ref = unsafe { this.as_mut().get_unchecked_mut() };
    let context = HandlerContext {
        outer_data: &this_ref.data,
    };
    // Convert the self-referential context to a 'static context
    // Since the function F accepts SyscallContext values of any lifetime,
    // its use of the lifetime will be limited to the bounds of the function,
    // making this sound.  (The lifetime 'a in the function is technically
    // 'static, but it is impossible to observe that from within the function.)
    let fake_context = unsafe { core::mem::transmute::<HandlerContext, HandlerContext>(context) };
    this_ref.inner.write(f.call(fake_context));
    this
}

fn new_handlers(thread: Box<thread::Thread>) -> OuterData {
    OuterData {
        thread: core::cell::UnsafeCell::new(Some(thread)),
        in_handler: core::cell::Cell::new(true),
        suspend_thread: core::cell::Cell::new(true),
        user_regs_changed: core::cell::Cell::new(false),
    }
}

unsafe impl<F: Send> Send for HandlerFuture<F> {}

impl<F> Future for HandlerFuture<F>
where
    F: Future<Output = ResumedContext>,
{
    type Output = ();
    fn poll(self: Pin<&mut Self>, ctx: &mut core::task::Context<'_>) -> Poll<Self::Output> {
        let this = unsafe { Pin::into_inner_unchecked(self) };
        let inner = unsafe { Pin::new_unchecked(this.inner.assume_init_mut()) };

        let _context = core::task::ready!(inner.poll(ctx));

        // Finished running the task; if the thread wasn't detatched / resumed
        // separately, schedule it now.
        if !this.data.in_handler.get() {
            if let Some(thread) = unsafe { &mut *this.data.thread.get() }.take() {
                event::SCHEDULER.add_task(event::Event::schedule_thread(thread));
            }
        }
        ().into()
    }
}
