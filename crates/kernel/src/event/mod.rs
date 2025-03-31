pub mod async_handler;
pub mod context;
pub mod exceptions;
pub mod scheduler;
pub mod task;
pub mod thread;

use alloc::boxed::Box;

use context::{deschedule_thread, Context, DescheduleAction, CORES};
use scheduler::{Priority, Scheduler};

pub static SCHEDULER: Scheduler = Scheduler::new();

pub struct Event {
    pub priority: Priority,
    pub kind: EventKind,
}

impl Event {
    pub fn function(f: Box<dyn FnOnce() + Send + 'static>, priority: Priority) -> Self {
        Self {
            priority,
            kind: EventKind::Function(f),
        }
    }
    pub fn async_task(task: task::TaskId, priority: Priority) -> Self {
        Self {
            priority,
            kind: EventKind::AsyncTask(task),
        }
    }
    pub fn schedule_thread(thread: Box<thread::Thread>) -> Self {
        Self {
            priority: thread.priority,
            kind: EventKind::ScheduleThread(thread),
        }
    }
}

pub enum EventKind {
    Function(Box<dyn FnOnce() + Send + 'static>),
    AsyncTask(task::TaskId),
    ScheduleThread(Box<thread::Thread>),
}

pub fn schedule<F>(f: F)
where
    F: FnOnce() + Send + 'static,
{
    let ev = Event::function(Box::new(f), Priority::Normal);
    SCHEDULER.add_task(ev);
}

pub fn schedule_rt<F>(f: F)
where
    F: FnOnce() + Send + 'static,
{
    let ev = Event::function(Box::new(f), Priority::Realtime);
    SCHEDULER.add_task(ev);
}

pub unsafe extern "C" fn run_event_loop() -> ! {
    loop {
        let ev = SCHEDULER.wait_for_task();
        match ev.kind {
            EventKind::Function(func) => {
                func();
            }
            EventKind::ScheduleThread(thread) => {
                unsafe { thread.enter_thread() };
            }
            EventKind::AsyncTask(task_id) => {
                run_async_task(task_id);
            }
        }
    }
}

fn run_async_task(task_id: task::TaskId) {
    if let Some(mut task) = task::TASKS.take_task(task_id) {
        let priority = task.priority;
        let waker = task::create_waker(task_id, priority);
        let mut context = core::task::Context::from_waker(&waker);

        match task.poll(&mut context) {
            core::task::Poll::Ready(()) => {
                task::TASKS.remove_task(task_id);
            }
            core::task::Poll::Pending => {
                let woken = task::TASKS.return_task(task_id, task);
                if woken {
                    SCHEDULER.add_task(Event::async_task(task_id, priority));
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

    if crate::sync::time::TIMER_SCHEDULER.is_ready() {
        // TODO: wait-free schedule here?
        // TODO: guarantee no allocation at compile time

        const fn guarantee_zst<T>(t: T) -> T {
            assert!(size_of::<T>() == 0);
            t
        }
        let callback = const {
            guarantee_zst(|| {
                crate::sync::time::TIMER_SCHEDULER.run();
            })
        };
        let callback = Box::new(callback);
        assert_eq!(size_of_val(&*callback), 0);
        SCHEDULER.add_task(Event::function(callback, Priority::Realtime));
    }

    let thread = CORES.with_current(|core| core.thread.take());

    let Some(mut thread) = thread else {
        // Not running a thread
        return ctx;
    };

    if thread.is_user_thread() && ctx.current_el() > context::ExceptionLevel::EL0 {
        // Currently in an exception handler in a user thread, resume running
        CORES.with_current(|core| core.thread.set(Some(thread)));
        return ctx;
    }

    if ctx.current_el() == context::ExceptionLevel::EL1 {
        let stacks = &raw const crate::arch::boot::STACKS;
        let ptr_range = stacks as usize..stacks.wrapping_add(1) as usize;
        let kernel_sp = ctx.kernel_sp;
        assert!(
            !ptr_range.contains(&kernel_sp),
            "Attempted to preempt core on kernel stack; kernel-thread: {:?}, el: {:?}, sp: {:?}",
            thread.is_kernel_thread(),
            ctx.current_el(),
            kernel_sp
        );
    }

    unsafe { thread.save_context(ctx.into(), thread.is_kernel_thread()) };
    unsafe { deschedule_thread(DescheduleAction::Yield, Some(thread)) };
}

#[track_caller]
pub fn assert_non_preemptible() {
    let all_interrupts_disabled = crate::sync::interrupts::get_interrupts().0 == 0b1111;
    let in_thread = crate::event::context::CORES.with_current(|core| {
        let thread = core.thread.take();
        let res = thread.is_some();
        core.thread.set(thread);
        res
    });
    if in_thread {
        assert!(all_interrupts_disabled);
    }
}

#[track_caller]
pub fn assert_not_in_interrupt() {
    let all_interrupts_enabled = crate::sync::interrupts::get_interrupts().0 == 0b0000;
    assert!(all_interrupts_enabled);
}
