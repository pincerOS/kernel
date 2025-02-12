use core::cell::UnsafeCell;
use core::mem::MaybeUninit;
use core::sync::atomic::{AtomicBool, Ordering};

pub struct UnsafeInit<T> {
    inner: UnsafeCell<MaybeUninit<T>>,
    initialized: AtomicBool,
}

impl<T> UnsafeInit<T> {
    /// Safety: init must be called before before the first use of the value
    pub const unsafe fn uninit() -> Self {
        Self {
            inner: UnsafeCell::new(MaybeUninit::uninit()),
            initialized: AtomicBool::new(false),
        }
    }
    /// Safety:
    /// - Must be called before any uses of the value
    /// - Must be called exactly once
    pub unsafe fn init(&self, value: T) {
        unsafe {
            (*self.inner.get()).write(value);
        }
        assert!(!self.initialized.swap(true, Ordering::SeqCst));
    }
    #[track_caller]
    pub fn get(&self) -> &T {
        debug_assert!(
            self.initialized.load(Ordering::Relaxed),
            "attempt to use an uninitialized UnsafeInit instance"
        );
        unsafe { (*self.inner.get()).assume_init_ref() }
    }
    pub fn is_initialized(&self) -> bool {
        self.initialized.load(Ordering::SeqCst)
    }
}

impl<T> Drop for UnsafeInit<T> {
    fn drop(&mut self) {
        if self.initialized.load(Ordering::SeqCst) {
            unsafe {
                self.inner.get_mut().assume_init_drop();
            }
        }
    }
}

unsafe impl<T> Sync for UnsafeInit<T> where T: Sync {}
unsafe impl<T> Send for UnsafeInit<T> where T: Send {}
