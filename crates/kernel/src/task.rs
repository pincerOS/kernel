use alloc::boxed::Box;
use alloc::collections::btree_map::BTreeMap;
use core::future::Future;
use core::pin::Pin;
use core::sync::atomic::{AtomicUsize, Ordering};
use core::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

use crate::event::SCHEDULER;
use crate::sync::SpinLock;

pub fn spawn_async(future: impl Future<Output = ()> + Send + Sync + 'static) {
    let task = TASKS.add_task(Task {
        future: Box::pin(future),
    });
    SCHEDULER.add_task(crate::event::Event::AsyncTask(task));
}

pub static TASKS: TaskList = TaskList::new();

pub struct Task {
    future: Pin<Box<dyn Future<Output = ()> + Send + Sync>>,
}

impl Task {
    pub fn poll(&mut self, context: &mut Context) -> Poll<()> {
        self.future.as_mut().poll(context)
    }
}

enum TaskState {
    Ready(Task),
    Running { woken: usize },
}

pub struct TaskList {
    // TODO: generational arena
    count: AtomicUsize,
    tasks: SpinLock<BTreeMap<TaskId, TaskState>>,
}

impl TaskList {
    pub const fn new() -> Self {
        TaskList {
            count: AtomicUsize::new(0),
            tasks: SpinLock::new(BTreeMap::new()),
        }
    }

    pub fn add_task(&self, task: Task) -> TaskId {
        let id = self.count.fetch_add(1, Ordering::Relaxed);
        let id = TaskId(id);
        self.tasks.lock().insert(id, TaskState::Ready(task));
        id
    }

    pub fn remove_task(&self, id: TaskId) {
        self.tasks.lock().remove(&id);
    }

    pub fn take_task(&self, id: TaskId) -> Option<Task> {
        let mut guard = self.tasks.lock();
        let state = guard.get_mut(&id)?;
        match core::mem::replace(state, TaskState::Running { woken: 0 }) {
            TaskState::Ready(task) => Some(task),
            TaskState::Running { woken } => {
                // Ensure that if the waker is called while the future is running,
                // the notifications will not be lost.
                *state = TaskState::Running { woken: woken + 1 };
                None
            }
        }
    }

    /// Returns whether the task received a wake notification while it was running.
    #[must_use]
    pub fn return_task(&self, id: TaskId, task: Task) -> bool {
        let mut guard = self.tasks.lock();
        let state = guard
            .get_mut(&id)
            .expect("attempt to return a removed task");
        match core::mem::replace(state, TaskState::Ready(task)) {
            TaskState::Ready(_) => panic!("invalid return_task call, task already ready"),
            TaskState::Running { woken } => woken > 0,
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct TaskId(usize);

fn wake_task(task: TaskId) {
    SCHEDULER.add_task(crate::event::Event::AsyncTask(task));
}

static WAKER_VTABLE: RawWakerVTable = RawWakerVTable::new(
    |this_clone| RawWaker::new(this_clone, &WAKER_VTABLE),
    |this_wake_own| wake_task(TaskId(this_wake_own as usize)),
    |this_wake_ref| wake_task(TaskId(this_wake_ref as usize)),
    |_this_drop| (),
);

pub fn create_waker(task: TaskId) -> Waker {
    // ... it doesn't have to actually point to memory ...
    let data = task.0 as *const ();
    let raw_waker = RawWaker::new(data, &WAKER_VTABLE);
    unsafe { Waker::from_raw(raw_waker) }
}

pub fn task_id_from_waker(waker: &Waker) -> Option<TaskId> {
    if core::ptr::eq(waker.vtable(), &WAKER_VTABLE) {
        Some(TaskId(waker.data() as usize))
    } else {
        None
    }
}
