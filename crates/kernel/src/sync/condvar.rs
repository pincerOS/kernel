use core::future::Future;

use crate::context::{context_switch, SwitchAction};
use crate::event::Event;

use super::lock::{Lock, OwnedSpinLockGuard, SpinLock, SpinLockGuard, SpinLockInner};
use super::RefProvider;

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

    pub fn wait_then<T, F, P, L>(&self, guard: OwnedSpinLockGuard<T, P>, f: F)
    where
        F: FnOnce(OwnedSpinLockGuard<T, P>) + Send + 'static,
        T: Send + 'static,
        P: RefProvider<Lock<T, SpinLockInner>> + Send + 'static,
    {
        let lock = guard.into_inner();
        let lock_ref = lock.provide();
        let lock_ptr = core::ptr::from_ref(lock_ref);

        let wrap = move || {
            let g = SpinLock::lock_owned(lock);
            f(g);
        };
        let event = Event::Function(alloc::boxed::Box::new(wrap));
        self.queue.add_then(event, move || {
            // Safety: the queue must not drop or release the event before
            // this function is called, so the ref provider must still be
            // valid.
            unsafe { &*lock_ptr }.inner.unlock()
        });
    }

    pub fn wait_then_owned<T, F, P, P2>(this: P, guard: OwnedSpinLockGuard<T, P2>, f: F)
    where
        P: RefProvider<Self> + Send + 'static,
        P2: RefProvider<Lock<T, SpinLockInner>> + Send + 'static,
        F: FnOnce(P, OwnedSpinLockGuard<T, P2>) + Send + 'static,
        T: Send + 'static,
    {
        // TODO: The arc should be downgraded to a weak pointer while in
        // the queue, such that the queue doesn't become a leaked ref cycle,
        // but there's no guarantee that the condvar is kept alive after
        // notify is called, so the Event must keep the condvar alive.
        let cond = this;
        let cond_ref = cond.provide();
        let cond_ptr = core::ptr::from_ref(cond_ref);

        let lock = guard.into_inner();
        let lock_ref = lock.provide();
        let lock_ptr = core::ptr::from_ref(lock_ref);

        let wrap = move || {
            let g = SpinLock::lock_owned(lock);
            f(cond, g);
        };
        let event = Event::Function(alloc::boxed::Box::new(wrap));

        // TODO: this may be UB, depending on the example implementation
        // of the queue's add -- there's no reasonable situation where it
        // would happen, as this event must be the last thing keeping the
        // queue alive, but if the queue implementation drops the thread,
        // then it would free itself and &self would become an invalid ref.
        unsafe { &(*cond_ptr) }.queue.add_then(event, move || {
            // Safety: the queue must not drop or release the event before
            // this function is called, so the ref provider must still be
            // valid.
            unsafe { &*lock_ptr }.inner.unlock();
        });
    }

    pub fn wait_while_then<'a, T, P, P2, Cond, Then>(
        this: P,
        mut guard: OwnedSpinLockGuard<T, P2>,
        mut condition: Cond,
        f: Then,
    ) where
        P: RefProvider<Self> + Send + 'static,
        Cond: FnMut(&mut T) -> bool + Send + 'static,
        Then: FnOnce(OwnedSpinLockGuard<T, P2>) + Send + 'static,
        P2: RefProvider<Lock<T, SpinLockInner>> + Send + 'static,
        T: Send + 'static,
    {
        if condition(&mut *guard) {
            Self::wait_then_owned(this, guard, move |this, guard| {
                Self::wait_while_then(this, guard, condition, f);
            });
        } else {
            f(guard);
        }
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

unsafe impl<'a, 'b, T: Send> Send for WaitFuture<'a, 'b, T> {}

impl<'a, 'b, T> Future for WaitFuture<'a, 'b, T> {
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
