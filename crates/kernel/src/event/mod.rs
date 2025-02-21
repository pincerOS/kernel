pub mod context;
pub mod exceptions;
pub mod scheduler;
pub mod task;
pub mod thread;

use alloc::boxed::Box;

use context::{deschedule_thread, Context, DescheduleAction, CORES};
use scheduler::Scheduler;

pub static SCHEDULER: Scheduler<Event> = Scheduler::new();

pub enum Event {
    Function(Box<dyn FnOnce() + Send + 'static>),
    AsyncTask(task::TaskId),
    ScheduleThread(Box<thread::Thread>),
}

pub fn schedule<F>(f: F)
where
    F: FnOnce() + Send + 'static,
{
    let ev = Event::Function(Box::new(f));
    SCHEDULER.add_task(ev);
}

pub unsafe extern "C" fn run_event_loop() -> ! {
    loop {
        let ev = SCHEDULER.wait_for_task();
        match ev {
            Event::Function(func) => {
                func();
            }
            Event::ScheduleThread(thread) => {
                unsafe { thread.enter_thread() };
            }
            Event::AsyncTask(task_id) => {
                let waker = task::create_waker(task_id);
                let mut context = core::task::Context::from_waker(&waker);

                if let Some(mut task) = task::TASKS.take_task(task_id) {
                    match task.poll(&mut context) {
                        core::task::Poll::Ready(()) => {
                            task::TASKS.remove_task(task_id);
                        }
                        core::task::Poll::Pending => {
                            let woken = task::TASKS.return_task(task_id, task);
                            if woken {
                                SCHEDULER.add_task(Event::AsyncTask(task_id));
                            }
                        }
                    }
                }
            }
        }
    }
}

pub unsafe fn timer_handler(ctx: &mut Context) -> *mut Context {
    // - if current core is running a thread:
    //    - suspend the thread, save its state
    //    - exit the interrupt handler
    //    - return to running the event loop
    // - otherwise, do nothing

    let thread = CORES.with_current(|core| core.thread.take());

    if let Some(mut thread) = thread {
        let stacks = &raw const crate::arch::boot::STACKS;
        let ptr_range = stacks as usize..stacks.wrapping_add(1) as usize;
        // TODO: ensure that this can't happen
        if ptr_range.contains(&ctx.sp) {
            CORES.with_current(|core| core.thread.set(Some(thread)));
            return ctx;
        }

        unsafe { thread.save_context(ctx.into()) };
        unsafe { deschedule_thread(DescheduleAction::Yield, Some(thread)) };
    } else {
        ctx
    }
}
