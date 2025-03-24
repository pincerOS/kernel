use core::cell::UnsafeCell;
use core::marker::PhantomData;
use core::sync::atomic::{AtomicBool, Ordering};

use super::interrupts::{disable_interrupts, restore_interrupts, InterruptsState};

pub trait LockImpl {
    const DEFAULT: Self;
    // const fn new() -> Self;
    fn lock(&self);
    fn unlock(&self);
}

pub struct Lock<T: ?Sized, L> {
    inner: L,
    value: UnsafeCell<T>,
}

pub struct LockGuard<'a, T: ?Sized, L: LockImpl> {
    pub(super) lock: &'a Lock<T, L>,
    marker: PhantomData<*mut ()>,
}

impl<T, L: LockImpl> Lock<T, L> {
    pub const fn new(value: T) -> Self {
        Lock {
            inner: L::DEFAULT,
            value: UnsafeCell::new(value),
        }
    }
    pub unsafe fn get_inner(&self) -> &L {
        &self.inner
    }
    pub unsafe fn force_acquire(&self) -> LockGuard<'_, T, L> {
        LockGuard {
            lock: self,
            marker: PhantomData,
        }
    }
}

impl<T: ?Sized, L: LockImpl> Lock<T, L> {
    #[track_caller]
    pub fn lock(&self) -> LockGuard<'_, T, L> {
        self.inner.lock();
        LockGuard {
            lock: self,
            marker: PhantomData,
        }
    }
}

unsafe impl<T: Send + ?Sized, L: Send> Send for Lock<T, L> {}
unsafe impl<T: Send + ?Sized, L: Sync> Sync for Lock<T, L> {}

unsafe impl<T: Send + ?Sized, L: LockImpl + Send> Send for LockGuard<'_, T, L> {}
unsafe impl<T: Sync + ?Sized, L: LockImpl + Sync> Sync for LockGuard<'_, T, L> {}

impl<T: ?Sized, L: LockImpl> core::ops::Deref for LockGuard<'_, T, L> {
    type Target = T;
    fn deref(&self) -> &T {
        let ptr = self.lock.value.get();
        unsafe { &*ptr }
    }
}
impl<T: ?Sized, L: LockImpl> core::ops::DerefMut for LockGuard<'_, T, L> {
    fn deref_mut(&mut self) -> &mut T {
        let ptr = self.lock.value.get();
        unsafe { &mut *ptr }
    }
}
impl<T: ?Sized, L: LockImpl> core::ops::Drop for LockGuard<'_, T, L> {
    fn drop(&mut self) {
        self.lock.inner.unlock();
    }
}

pub struct SpinLockInner {
    flag: AtomicBool,
}

impl SpinLockInner {
    pub const fn new() -> Self {
        SpinLockInner {
            flag: AtomicBool::new(false),
        }
    }
    pub fn try_acquire(&self) -> bool {
        self.flag
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
    }
    #[track_caller]
    pub fn lock(&self) {
        // crate::event::assert_non_preemptible();
        crate::event::assert_not_in_interrupt();
        while !self.try_acquire() {
            while self.flag.load(Ordering::Relaxed) {
                core::hint::spin_loop();
            }
        }
    }
    pub fn unlock(&self) {
        self.flag.store(false, Ordering::Release);
    }
}

impl LockImpl for SpinLockInner {
    #[allow(clippy::declare_interior_mutable_const)]
    const DEFAULT: Self = Self::new();
    #[track_caller]
    fn lock(&self) {
        self.lock()
    }
    fn unlock(&self) {
        self.unlock()
    }
}

pub type SpinLock<T> = Lock<T, SpinLockInner>;
pub type SpinLockGuard<'a, T> = LockGuard<'a, T, SpinLockInner>;

pub struct InterruptSpinLockInner {
    flag: AtomicBool,
    state: UnsafeCell<Option<InterruptsState>>,
}

impl InterruptSpinLockInner {
    pub const fn new() -> Self {
        InterruptSpinLockInner {
            flag: AtomicBool::new(false),
            state: UnsafeCell::new(None),
        }
    }
    pub fn try_acquire(&self) -> bool {
        self.flag
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
    }
    pub fn lock(&self) {
        let mut state = unsafe { disable_interrupts() };
        while !self.try_acquire() {
            unsafe { restore_interrupts(state) };
            while self.flag.load(Ordering::Relaxed) {
                core::hint::spin_loop();
            }
            state = unsafe { disable_interrupts() };
        }
        unsafe {
            self.state.get().write(Some(state));
        }
    }
    pub fn unlock(&self) {
        let state = unsafe { (*self.state.get()).take() };
        self.flag.store(false, Ordering::Release);
        unsafe { restore_interrupts(state.unwrap()) }
    }
}

impl LockImpl for InterruptSpinLockInner {
    #[allow(clippy::declare_interior_mutable_const)]
    const DEFAULT: Self = Self::new();
    fn lock(&self) {
        self.lock()
    }
    fn unlock(&self) {
        self.unlock()
    }
}

pub type InterruptSpinLock<T> = Lock<T, InterruptSpinLockInner>;
pub type InterruptSpinLockGuard<'a, T> = LockGuard<'a, T, InterruptSpinLockInner>;

unsafe impl Send for InterruptSpinLockInner {}
unsafe impl Sync for InterruptSpinLockInner {}
