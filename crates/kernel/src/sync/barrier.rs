use crate::sync::lock::Lock;

use super::condvar::CondVar;
use super::lock::SpinLock;
use super::RefProvider;

pub struct Barrier {
    count: SpinLock<u32>,
    condvar: CondVar,
}
impl Barrier {
    pub const fn new(val: u32) -> Self {
        Self {
            count: SpinLock::new(val),
            condvar: CondVar::new(),
        }
    }
    pub fn sync(&self) {
        let mut guard = self.count.lock();
        assert!(*guard > 0);
        *guard -= 1;
        if *guard == 0 {
            self.condvar.notify_all();
        } else {
            self.condvar.wait_while(guard, |count| *count > 0);
        }
    }

    pub fn sync_then(
        this: impl RefProvider<Self> + Clone + Send + 'static,
        f: impl FnOnce() + Send + 'static,
    ) {
        #[derive(Clone)]
        struct ProvideProject<T>(T);
        unsafe impl<T: RefProvider<Barrier>> RefProvider<CondVar> for ProvideProject<T> {
            fn provide(&self) -> &CondVar {
                &self.0.provide().condvar
            }
        }
        unsafe impl<T: RefProvider<Barrier>> RefProvider<SpinLock<u32>> for ProvideProject<T> {
            fn provide(&self) -> &SpinLock<u32> {
                &self.0.provide().count
            }
        }
        let ref2 = this.clone();

        let mut guard = Lock::lock_owned(ProvideProject(this));
        assert!(*guard > 0);
        *guard -= 1;
        if *guard == 0 {
            let inner = guard.unlock().0;
            inner.provide().condvar.notify_all();
            f();
        } else {
            CondVar::wait_while_then(
                ProvideProject(ref2),
                guard,
                |count| *count > 0,
                |guard| {
                    drop(guard);
                    f();
                },
            );
        }
    }

    pub async fn sync_async(&self) {
        let mut guard = self.count.lock();
        assert!(*guard > 0);
        *guard -= 1;
        if *guard == 0 {
            self.condvar.notify_all();
            drop(guard);
        } else {
            self.condvar
                .wait_while_async(guard, |count| *count > 0)
                .await;
        }
    }
}
