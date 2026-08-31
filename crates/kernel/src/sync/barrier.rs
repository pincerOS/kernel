use super::condvar::Condvar;
use super::lock::SpinLock;

/// A synchronization primitive to wait until all tasks have reached
/// a point before continuing.
///
/// A barrier is constructed with an initial `count`, the number of
/// tasks for which it should wait.  Each task that calls [`sync`] on
/// the `Barrier` will wait until all `count` tasks have called [`sync`]
/// before continuing.
///
/// [`sync`]: Self::sync
pub struct Barrier {
    count: SpinLock<u32>,
    condvar: Condvar,
}

impl Barrier {
    /// Create a new barrier with the given initial count.
    pub const fn new(count: u32) -> Self {
        Self {
            count: SpinLock::new(count),
            condvar: Condvar::new(),
        }
    }

    /// Synchronize with the barrier.
    ///
    /// This will wait until `count` tasks have reached the barrier
    /// before resuming.  If `count` tasks have already reached the
    /// barrier, this function will panic.
    pub async fn sync(&self) {
        let mut guard = self.count.lock();
        assert!(*guard > 0);
        *guard -= 1;
        if *guard == 0 {
            self.condvar.notify_all();
            drop(guard);
        } else {
            self.condvar.wait_while(guard, |count| *count > 0).await;
        }
    }

    /// Synchronize with the barrier, blocking the current thread.
    ///
    /// See [`sync`] for the method documentation.
    ///
    /// If this is called from outside of a threaded environment, this
    /// function will panic.
    ///
    /// [`sync`]: Self::sync
    pub fn sync_blocking(&self) {
        let mut guard = self.count.lock();
        assert!(*guard > 0);
        *guard -= 1;
        if *guard == 0 {
            self.condvar.notify_all();
        } else {
            self.condvar.wait_while_blocking(guard, |count| *count > 0);
        }
    }
}

test_case!(async async_barrier);
async fn async_barrier() -> Result<(), crate::test::BoxError> {
    use crate::event::task::spawn_async;
    use alloc::sync::Arc;
    use core::sync::atomic::{AtomicU32, Ordering};

    let count = 16;
    let barrier = Arc::new(Barrier::new(count + 1));
    let reached = Arc::new(AtomicU32::new(0));

    for i in 0..count {
        let r = reached.clone();
        let b: Arc<Barrier> = barrier.clone();
        spawn_async(async move {
            r.fetch_add(i, Ordering::SeqCst);
            b.sync().await;
        });
    }

    barrier.sync().await;
    kassert_eq!(reached.load(Ordering::SeqCst), count * (count - 1) / 2)?;

    Ok(())
}

// TODO: better built-in threaded tests
test_case!(thread thread_barrier);
fn thread_barrier() -> Result<(), crate::test::BoxError> {
    use alloc::sync::Arc;
    use core::sync::atomic::{AtomicU32, Ordering};

    let count = 32;
    let barrier = Arc::new(Barrier::new(count + 1));
    let reached = Arc::new(AtomicU32::new(0));

    for i in 0..count {
        let r = reached.clone();
        let b: Arc<Barrier> = barrier.clone();
        crate::event::thread::thread(move || {
            // println!("Starting thread {i}");
            // TODO: non-spinning sleep
            crate::sync::spin_sleep(100_000);
            // println!("Ending thread {i}");

            r.fetch_add(i, Ordering::SeqCst);
            b.sync_blocking();
        });
    }
    barrier.sync_blocking();
    println!("End of preemption test");

    kassert_eq!(reached.load(Ordering::SeqCst), count * (count - 1) / 2)?;
    Ok(())
}
