use core::future::Future;
use core::pin::Pin;
use core::task::Poll;

use alloc::boxed::Box;

use crate::context::{deschedule_thread, Context, DescheduleAction, CORES};
use crate::{event, task, thread};

pub fn run_async_syscall<const N: usize, F>(ctx: &mut Context, future: F) -> *mut Context
where
    F: Future<Output = [usize; N]> + Send + Sync + 'static,
{
    // TODO: avoid allocating in the syscall handler?
    let mut future = Box::pin(HandlerWrapper {
        inner: future,
        data: HandlerData {
            returns: None,
            should_schedule: None,
        },
    });

    let task_id = task::TASKS.alloc_task_slot();
    let waker = task::create_waker(task_id);
    let mut context = core::task::Context::from_waker(&waker);

    // Run the handler future until it suspends once
    match Future::poll(future.as_mut(), &mut context) {
        Poll::Ready(()) => {
            // If the handler finished immediately, remove the task
            // and return back to the user directly.
            task::TASKS.remove_task(task_id);
            let data = future.as_mut().get_data();
            let return_arr = data.returns.take().unwrap();

            ctx.regs[..N].copy_from_slice(&return_arr);
            ctx
        }
        Poll::Pending => {
            // The handler yielded; suspend the current thread, and set
            // up the future to reschedule the thread when it finishes.

            let (core_sp, thread) =
                CORES.with_current(|core| (core.core_sp.get(), core.thread.take()));
            let mut thread = thread.expect("usermode syscall without active thread");
            unsafe { thread.save_context(ctx.into()) };

            let data = future.as_mut().get_data();
            data.should_schedule = Some(thread);

            let woken = task::TASKS.return_task(task_id, task::Task::new(future));
            if woken {
                event::SCHEDULER.add_task(event::Event::AsyncTask(task_id));
            }

            // Switch back to the event loop.
            unsafe { deschedule_thread(core_sp, None, DescheduleAction::FreeThread) }
        }
    }
}

struct HandlerData<const N: usize> {
    returns: Option<[usize; N]>,
    should_schedule: Option<Box<thread::Thread>>,
}

struct HandlerWrapper<const N: usize, F> {
    inner: F,
    data: HandlerData<N>,
}

impl<const N: usize, F> HandlerWrapper<N, F> {
    fn get_data(self: Pin<&mut Self>) -> &mut HandlerData<N> {
        unsafe { &mut self.get_unchecked_mut().data }
    }
}

impl<const N: usize, F> Future for HandlerWrapper<N, F>
where
    F: Future<Output = [usize; N]>,
{
    type Output = ();
    fn poll(self: Pin<&mut Self>, ctx: &mut core::task::Context<'_>) -> Poll<Self::Output> {
        let this = unsafe { Pin::into_inner_unchecked(self) };
        let inner = unsafe { Pin::new_unchecked(&mut this.inner) };

        let arr = core::task::ready!(inner.poll(ctx));
        if let Some(mut thread) = this.data.should_schedule.take() {
            let ctx = thread.context.as_mut().unwrap();
            ctx.regs[..N].copy_from_slice(&arr);
            event::SCHEDULER.add_task(event::Event::ScheduleThread(thread));
        } else {
            this.data.returns = Some(arr);
        }
        ().into()
    }
}
