use core::future::Future;

use crate::event::context::{context_switch, SwitchAction};
use crate::event::task::event_for_waker;
use crate::event::{scheduler::Queue, Event, SCHEDULER};

use super::lock::SpinLockGuard;
use super::SpinLock;

/// A condition variable.
///
/// A condition variable is an abstraction over a wait queue, and
/// allows for three basic operations:
/// - [`wait`], to add the current task to the wait queue and then
///   unlock the passed spinlock;
/// - [`notify_one`], to wake one task from the wait queue;
/// - [`notify_all`], to wake all tasks from the wait queue.
///
/// When a task is woken, [`wait`] will re-lock the spinlock before
/// continuing, and will return the lock guard.  Note that another task
/// could acquire the spinlock between a call to [`notify_one`] and the
/// scheduled task resuming, so the condition may no longer be satisfied
/// when the woken task runs.  As such, [`wait`] should always be called
/// in a loop that checks the related condition.
///
/// [`wait_while`] is a convenience function for wrapping [`wait`] in a
/// loop checking the condition; its implementation is simple:
/// ```rust
/// while condition(&mut *guard) {
///    guard = self.wait(guard).await;
/// }
/// ```
///
/// For a more thorough explanation of this style of condition variable
/// api, see [`std::sync::Condvar`].
///
/// [`wait`]: Self::wait
/// [`wait_while`]: Self::wait_while
/// [`notify_one`]: Self::notify_one
/// [`notify_all`]: Self::notify_all
/// [`std::sync::Condvar`]: https://doc.rust-lang.org/std/sync/struct.Condvar.html
pub struct Condvar {
    queue: Queue<Event>,
}

impl Condvar {
    /// Create a new condition variable.
    pub const fn new() -> Self {
        Self {
            queue: Queue::new(),
        }
    }
    /// Wakes one blocked task from the wait queue.
    pub fn notify_one(&self) {
        if let Some(t) = self.queue.pop() {
            SCHEDULER.add_task(t);
        }
    }
    /// Wakes all blocked tasks currently on the wait queue.
    pub fn notify_all(&self) {
        SCHEDULER.add_all(&self.queue);
    }

    /// Wait until the condition variable has been notified.
    ///
    /// This will add the task to the wait queue before unlocking the
    /// lock.  The lock will be re-locked before returning.
    pub fn wait<'a, 'b, T>(
        &'b self,
        guard: SpinLockGuard<'a, T>,
    ) -> impl Future<Output = SpinLockGuard<'a, T>> + Send + Sync + use<'a, 'b, T>
    where
        T: Send + Sync,
    {
        WaitFuture {
            this: Some(self),
            lock: guard.lock,
        }
    }

    /// Wait on the condition variable until the specified condition is
    /// false.
    ///
    /// This will repeatedly wait on the condition variable, each time
    /// checking if the condition specified by the passed closure is
    /// satisfied.  When the condition is false, this will return a
    /// newly locked lock-guard to the caller.
    pub fn wait_while<'a, 'b, T, F>(
        &'b self,
        guard: SpinLockGuard<'a, T>,
        condition: F,
    ) -> impl Future<Output = SpinLockGuard<'a, T>> + use<'a, 'b, T, F>
    where
        F: FnMut(&mut T) -> bool + Send + Sync,
        T: Send + Sync,
    {
        let lock = guard.lock;
        core::mem::forget(guard);
        WaitWhileFuture {
            this: self,
            lock,
            first_time: true,
            condition,
        }
    }

    /// Block the current thread until the condition variable has been
    /// notified.
    ///
    /// This function will panic if called from a context other than a thread.
    ///
    /// See [`wait`][Self::wait] for more detailed documentation.
    pub fn wait_blocking<'a, T>(&self, guard: SpinLockGuard<'a, T>) -> SpinLockGuard<'a, T> {
        let lock = guard.lock;
        core::mem::forget(guard);
        let inner = unsafe { lock.get_inner() };
        context_switch(SwitchAction::QueueAddUnlock(&self.queue, inner));
        lock.lock()
    }

    /// Block the current thread until the specified condition is false.
    ///
    /// This function will panic if called from a context other than a thread.
    ///
    /// See [`wait_while`][Self::wait_while] for more detailed documentation.
    pub fn wait_while_blocking<'a, T, F>(
        &self,
        mut guard: SpinLockGuard<'a, T>,
        mut condition: F,
    ) -> SpinLockGuard<'a, T>
    where
        F: FnMut(&mut T) -> bool,
    {
        while condition(&mut *guard) {
            guard = self.wait_blocking(guard);
        }
        guard
    }
}

struct WaitFuture<'a, 'b, T> {
    this: Option<&'b Condvar>,
    lock: &'a SpinLock<T>,
}

unsafe impl<T: Send> Send for WaitFuture<'_, '_, T> {}

impl<'a, T> Future for WaitFuture<'a, '_, T> {
    type Output = SpinLockGuard<'a, T>;
    fn poll(
        mut self: core::pin::Pin<&mut Self>,
        ctx: &mut core::task::Context<'_>,
    ) -> core::task::Poll<Self::Output> {
        match self.this.take() {
            Some(this) => {
                // TODO: this is a hack that only works on our executor,
                // and will break other async libraries
                let task = event_for_waker(ctx.waker()).unwrap();
                this.queue.add(task);
                let inner = unsafe { self.lock.get_inner() };
                inner.unlock();
                core::task::Poll::Pending
            }
            None => {
                // TODO: make sure the task wasn't just spuriously polled
                // (track if notify was called on this specific one)
                let inner = self.lock.lock();
                core::task::Poll::Ready(inner)
            }
        }
    }
}

// This is just:
// async move {
//     while condition(&mut *guard) {
//         guard = self.wait(guard).await;
//     }
//     guard
// }
// But Rust is currently really bad at small future sizes:
// https://github.com/rust-lang/rust/issues/62958
struct WaitWhileFuture<'a, 'b, T, F> {
    this: &'b Condvar,
    lock: &'a SpinLock<T>,
    condition: F,
    first_time: bool,
}

unsafe impl<T: Send, F: Send> Send for WaitWhileFuture<'_, '_, T, F> {}

impl<'a, T, F> Future for WaitWhileFuture<'a, '_, T, F>
where
    F: FnMut(&mut T) -> bool + Send + Sync,
{
    type Output = SpinLockGuard<'a, T>;
    fn poll(
        self: core::pin::Pin<&mut Self>,
        ctx: &mut core::task::Context<'_>,
    ) -> core::task::Poll<Self::Output> {
        let this = unsafe { self.get_unchecked_mut() };
        let mut guard = if this.first_time {
            // already locked
            this.first_time = false;
            unsafe { this.lock.force_acquire() }
        } else {
            this.lock.lock()
        };

        if !(this.condition)(&mut *guard) {
            core::task::Poll::Ready(guard)
        } else {
            // TODO: this is a hack that only works on our executor,
            // and will break other async libraries
            let task = event_for_waker(ctx.waker()).unwrap();
            this.this.queue.add(task);
            drop(guard);
            core::task::Poll::Pending
        }
    }
}
