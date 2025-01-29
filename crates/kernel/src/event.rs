use alloc::boxed::Box;

use crate::scheduler::Scheduler;
use crate::thread::CORES;

pub static SCHEDULER: Scheduler<Event> = Scheduler::new();

pub enum Event {
    Function(Box<dyn FnOnce() + Send + 'static>),
    AsyncTask(crate::task::TaskId),
    ScheduleThread(Box<crate::thread::Thread>),
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
                let next_ctx = thread.last_context.as_ptr();

                // Disable interrupts (preemption) until context is
                // restored.  (interrupts will be re-enabled by eret)
                // The timer interrupt assumes that if CORE.thread is set,
                // then there is a preemptable thread running.
                unsafe { crate::sync::disable_interrupts() };

                let old = CORES.with_current(|core| core.thread.replace(Some(thread)));
                assert!(old.is_none());

                // switch into the thread
                unsafe { crate::thread::restore_context(next_ctx) };
                // unreachable
            }
            Event::AsyncTask(task_id) => {
                let waker = crate::task::create_waker(task_id);
                let mut context = core::task::Context::from_waker(&waker);

                if let Some(mut task) = crate::task::TASKS.take_task(task_id) {
                    match task.poll(&mut context) {
                        core::task::Poll::Ready(()) => {
                            crate::task::TASKS.remove_task(task_id);
                        }
                        core::task::Poll::Pending => {
                            let woken = crate::task::TASKS.return_task(task_id, task);
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

#[allow(improper_ctypes)]
extern "C" {
    pub fn deschedule_thread(core_sp: usize, thread: Box<crate::thread::Thread>) -> !;
}

core::arch::global_asm!(
    "
.global deschedule_thread
deschedule_thread:
    mov sp, x0
    bl deschedule_thread_inner
    udf #0"
);

#[no_mangle]
unsafe extern "C" fn deschedule_thread_inner(
    _core_sp: usize,
    thread: Box<crate::thread::Thread>,
) -> ! {
    unsafe { crate::sync::enable_interrupts() };
    SCHEDULER.add_task(Event::ScheduleThread(thread));
    unsafe { run_event_loop() }
}

pub unsafe fn timer_handler(ctx: &mut crate::thread::Context) -> *mut crate::thread::Context {
    // - if current core is running a thread:
    //    - suspend the thread, save its state
    //    - exit the interrupt handler
    //    - return to running the event loop
    // - otherwise, do nothing

    let (helper_sp, thread) = CORES.with_current(|core| (core.helper_sp.get(), core.thread.take()));

    if let Some(mut thread) = thread {
        thread.last_context = ctx.into();
        unsafe { deschedule_thread(helper_sp, thread) };
    } else {
        ctx
    }
}
