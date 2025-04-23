use core::cell::UnsafeCell;
use core::mem::MaybeUninit;
use core::sync::atomic::{AtomicBool, Ordering};

use super::{Condvar, SpinLock};

pub struct BlockingOnceCell<T> {
    condvar: Condvar,
    ready_skip: AtomicBool,
    ready: SpinLock<bool>,
    data: UnsafeCell<MaybeUninit<T>>,
}

impl<T> BlockingOnceCell<T> {
    pub fn new() -> Self {
        BlockingOnceCell {
            condvar: Condvar::new(),
            ready_skip: AtomicBool::new(false),
            ready: SpinLock::new(false),
            data: UnsafeCell::new(MaybeUninit::uninit()),
        }
    }

    pub fn set(&self, value: T) {
        let mut guard = self.ready.lock();
        assert!(!*guard);
        unsafe { (*self.data.get()).write(value) };
        *guard = true;
        drop(guard);
        self.ready_skip.store(true, Ordering::Release);
        self.condvar.notify_all();
    }

    pub fn try_set(&self, value: T) -> Result<(), T> {
        let mut guard = self.ready.lock();
        if *guard {
            Err(value)
        } else {
            unsafe { (*self.data.get()).write(value) };
            *guard = true;
            drop(guard);
            self.ready_skip.store(true, Ordering::Release);
            self.condvar.notify_all();
            Ok(())
        }
    }

    pub async fn get(&self) -> &T {
        // TODO: avoid the lock?
        if self.ready_skip.load(Ordering::Acquire) {
            return unsafe { (&*self.data.get()).assume_init_ref() };
        }
        let guard = self.ready.lock();
        self.condvar
            .wait_while(guard, |ready| *ready == false)
            .await;
        unsafe { (&*self.data.get()).assume_init_ref() }
    }

    pub fn try_get(&self) -> Option<&T> {
        if self.ready_skip.load(Ordering::Acquire) {
            return Some(unsafe { (&*self.data.get()).assume_init_ref() });
        } else {
            None
        }
    }
}

unsafe impl<T: Send> Send for BlockingOnceCell<T> {}
unsafe impl<T: Sync> Sync for BlockingOnceCell<T> {}
