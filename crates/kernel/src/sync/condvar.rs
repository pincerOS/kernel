use core::future::Future;

use crate::context::{context_switch, SwitchAction};
use crate::event::Event;

use super::lock::SpinLockGuard;

type EventQueue = crate::scheduler::Queue<Event>;

pub struct CondVar {
    queue: EventQueue,
}

impl CondVar {
    pub const fn new() -> Self {
        Self {
            queue: EventQueue::new(),
        }
    }
    pub fn notify_one(&self) {
        if let Some(t) = self.queue.pop() {
            crate::event::SCHEDULER.add_task(t);
        }
    }
    pub fn notify_all(&self) {
        crate::event::SCHEDULER.add_all(&self.queue);
    }
    pub fn wait<'a, T>(&self, guard: SpinLockGuard<'a, T>) -> SpinLockGuard<'a, T> {
        let lock = guard.lock;
        core::mem::forget(guard);
        context_switch(SwitchAction::QueueAddUnlock(&self.queue, &lock.inner));
        lock.lock()
    }

    pub fn wait_while<'a, T, F>(
        &self,
        mut guard: SpinLockGuard<'a, T>,
        mut condition: F,
    ) -> SpinLockGuard<'a, T>
    where
        F: FnMut(&mut T) -> bool,
    {
        while condition(&mut *guard) {
            guard = self.wait(guard);
        }
        guard
    }

    pub fn wait_async<'a, 'b, T>(
        &'b self,
        guard: SpinLockGuard<'a, T>,
    ) -> impl Future<Output = SpinLockGuard<'a, T>> + Send + Sync + use<'a, 'b, T>
    where
        T: Send + Sync,
    {
        let lock = guard.lock;
        let task = WaitFuture {
            this: self,
            guard: Some(guard),
        };
        async {
            task.await;
            lock.lock()
        }
    }

    pub async fn wait_while_async<'a, T, F>(
        &self,
        mut guard: SpinLockGuard<'a, T>,
        mut condition: F,
    ) -> SpinLockGuard<'a, T>
    where
        F: FnMut(&mut T) -> bool + Send + Sync,
        T: Send + Sync,
    {
        while condition(&mut *guard) {
            guard = self.wait_async(guard).await;
        }
        guard
    }
}

struct WaitFuture<'a, 'b, T> {
    this: &'b CondVar,
    guard: Option<SpinLockGuard<'a, T>>,
}

unsafe impl<T: Send> Send for WaitFuture<'_, '_, T> {}

impl<T> Future for WaitFuture<'_, '_, T> {
    type Output = ();
    fn poll(
        mut self: core::pin::Pin<&mut Self>,
        ctx: &mut core::task::Context<'_>,
    ) -> core::task::Poll<Self::Output> {
        match self.guard.take() {
            Some(guard) => {
                // TODO: this is a hack that only works on our executor,
                // and will break other async libraries
                let task = crate::task::task_id_from_waker(ctx.waker()).unwrap();
                self.this.queue.add(Event::AsyncTask(task));
                drop(guard);
                core::task::Poll::Pending
            }
            None => {
                // TODO: make sure the task wasn't just spuriously polled
                // (track if notify was called on this specific one)
                core::task::Poll::Ready(())
            }
        }
    }
}
