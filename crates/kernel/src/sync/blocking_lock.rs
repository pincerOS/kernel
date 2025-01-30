use super::condvar::CondVar;
use super::lock::{Lock, LockGuard, LockImpl, SpinLock};

pub struct BlockingLockInner {
    lock: SpinLock<bool>,
    condvar: CondVar,
}
impl BlockingLockInner {
    pub const fn new() -> Self {
        Self {
            lock: SpinLock::new(false),
            condvar: CondVar::new(),
        }
    }
    pub fn lock(&self) {
        let guard = self.lock.lock();
        self.condvar
            .wait_while(guard, |locked| core::mem::replace(locked, true));
    }
    pub fn unlock(&self) {
        let mut guard = self.lock.lock();
        assert!(*guard);
        *guard = false;
    }
}

impl LockImpl for BlockingLockInner {
    const DEFAULT: Self = Self::new();
    fn lock(&self) {
        self.lock()
    }
    fn unlock(&self) {
        self.unlock()
    }
}

pub type BlockingLock<T> = Lock<T, BlockingLockInner>;
pub type BlockingLockGuard<'a, T> = LockGuard<'a, T, BlockingLockInner>;
