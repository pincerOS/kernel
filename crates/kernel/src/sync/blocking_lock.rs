use super::condvar::Condvar;
use super::lock::{Lock, LockGuard, LockImpl, SpinLock};

pub struct BlockingLockInner {
    lock: SpinLock<bool>,
    condvar: Condvar,
}
impl BlockingLockInner {
    pub const fn new() -> Self {
        Self {
            lock: SpinLock::new(false),
            condvar: Condvar::new(),
        }
    }
    pub fn lock_blocking(&self) {
        let guard = self.lock.lock();
        self.condvar
            .wait_while_blocking(guard, |locked| core::mem::replace(locked, true));
    }
    pub async fn lock(&self) {
        let guard = self.lock.lock();
        self.condvar
            .wait_while(guard, |locked| core::mem::replace(locked, true))
            .await;
    }
    pub fn unlock(&self) {
        let mut guard = self.lock.lock();
        assert!(*guard);
        *guard = false;
    }
}

impl LockImpl for BlockingLockInner {
    #[allow(clippy::declare_interior_mutable_const)]
    const DEFAULT: Self = Self::new();
    fn lock(&self) {
        self.lock_blocking()
    }
    fn unlock(&self) {
        self.unlock()
    }
}

pub type BlockingLock<T> = Lock<T, BlockingLockInner>;
pub type BlockingLockGuard<'a, T> = LockGuard<'a, T, BlockingLockInner>;
