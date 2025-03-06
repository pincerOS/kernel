use core::cell::UnsafeCell;
use core::mem::MaybeUninit;
use core::sync::atomic::{AtomicU8, Ordering};

/// A value that must be initialized before any possible use.
///
/// [`get`] is safe, but only because the callers of [`uninit`] and
/// [`init`] *must* guarantee that [`init`] will be called before
/// any calls to [`get`].
///
/// This is intended for initialization of variables that are expected
/// to exist in all cases, and need high performance access, but cannot
/// be initialized as a const value.  For example, the UART, heap, and
/// interrupt controller data structures use this for initialization.
///
/// [`uninit`]: [Self::uninit]
/// [`init`]: [Self::init]
/// [`get`]: [Self::get]
pub struct UnsafeInit<T> {
    inner: UnsafeCell<MaybeUninit<T>>,
    /// Initialization state:
    /// - 0: uninitialized
    /// - 1: initializing
    /// - 2: initialized
    initialized: AtomicU8,
}

impl<T> UnsafeInit<T> {
    /// Create a new `UnsafeInit` instance, starting out uninitialized.
    ///
    /// # Safety
    ///
    /// [`init`] must be called before before [`get`] can ever be called
    ///
    /// [`init`]: [Self::init]
    /// [`get`]: [Self::get]
    pub const unsafe fn uninit() -> Self {
        Self {
            inner: UnsafeCell::new(MaybeUninit::uninit()),
            initialized: AtomicU8::new(0),
        }
    }

    /// Set the initial value of this cell.
    ///
    /// # Safety
    ///
    /// - Must be called before any calls to [`get`]
    /// - Must be called exactly once
    ///
    /// [`get`]: [Self::get]
    pub unsafe fn init(&self, value: T) {
        assert!(self.initialized.swap(1, Ordering::SeqCst) == 0);

        unsafe { (*self.inner.get()).write(value) };

        assert!(self.initialized.swap(2, Ordering::SeqCst) == 1);
    }

    /// Get a reference to the value stored in the cell.
    ///
    /// This must only be called after the value is initialized, but
    /// that is guaranteed by the caller of [`uninit`] and [`init`].
    ///
    /// [`uninit`]: [Self::uninit]
    /// [`init`]: [Self::init]
    #[track_caller]
    pub fn get(&self) -> &T {
        debug_assert!(
            self.initialized.load(Ordering::Relaxed) == 2,
            "attempt to use an uninitialized UnsafeInit instance"
        );
        unsafe { (*self.inner.get()).assume_init_ref() }
    }
    pub fn is_initialized(&self) -> bool {
        self.initialized.load(Ordering::SeqCst) == 2
    }
}

impl<T> Drop for UnsafeInit<T> {
    fn drop(&mut self) {
        if self.initialized.load(Ordering::SeqCst) == 2 {
            unsafe { self.inner.get_mut().assume_init_drop() };
        }
    }
}

unsafe impl<T> Sync for UnsafeInit<T> where T: Sync {}
unsafe impl<T> Send for UnsafeInit<T> where T: Send {}
