use core::cell::UnsafeCell;
use core::marker::PhantomData;
use core::sync::atomic::{AtomicBool, Ordering};

use super::interrupts::{disable_interrupts, restore_interrupts, InterruptsState};
use super::RefProvider;

pub trait LockImpl {
    const DEFAULT: Self;
    // const fn new() -> Self;
    fn lock(&self);
    fn unlock(&self);
}

pub struct Lock<T: ?Sized, L> {
    pub(super) inner: L,
    value: UnsafeCell<T>,
}

pub struct LockGuard<'a, T: ?Sized, L: LockImpl> {
    pub(super) lock: &'a Lock<T, L>,
    marker: PhantomData<*mut ()>,
}

pub struct OwnedLockGuard<T: ?Sized, L: LockImpl, P: RefProvider<Lock<T, L>>> {
    pub(super) lock: P,
    marker: PhantomData<(*mut (), Lock<T, L>)>,
}

impl<T, L: LockImpl> Lock<T, L> {
    pub const fn new(value: T) -> Self {
        Lock {
            inner: L::DEFAULT,
            value: UnsafeCell::new(value),
        }
    }
}

impl<T: ?Sized, L: LockImpl> Lock<T, L> {
    pub fn lock(&self) -> LockGuard<'_, T, L> {
        self.inner.lock();
        LockGuard {
            lock: self,
            marker: PhantomData,
        }
    }
    pub fn lock_owned<P>(this: P) -> OwnedLockGuard<T, L, P>
    where
        P: RefProvider<Self>,
    {
        this.provide().inner.lock();
        OwnedLockGuard {
            lock: this,
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

impl<T: ?Sized, L: LockImpl, P: RefProvider<Lock<T, L>>> core::ops::Deref
    for OwnedLockGuard<T, L, P>
{
    type Target = T;
    fn deref(&self) -> &T {
        let ptr = self.lock.provide().value.get();
        unsafe { &*ptr }
    }
}
impl<T: ?Sized, L: LockImpl, P: RefProvider<Lock<T, L>>> core::ops::DerefMut
    for OwnedLockGuard<T, L, P>
{
    fn deref_mut(&mut self) -> &mut T {
        let ptr = self.lock.provide().value.get();
        unsafe { &mut *ptr }
    }
}
impl<T: ?Sized, L: LockImpl, P: RefProvider<Lock<T, L>>> core::ops::Drop
    for OwnedLockGuard<T, L, P>
{
    fn drop(&mut self) {
        self.lock.provide().inner.unlock();
    }
}

impl<T: ?Sized, L: LockImpl, P: RefProvider<Lock<T, L>>> OwnedLockGuard<T, L, P> {
    pub fn unlock(self) -> P {
        self.lock.provide().inner.unlock();
        self.into_inner()
    }
    pub fn into_inner(self) -> P {
        let this = core::mem::ManuallyDrop::new(self);
        // Manually move the lock out of the lock guard without calling
        // its destructor.
        unsafe { core::ptr::read(&this.lock) }
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
    pub fn lock(&self) {
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
    const DEFAULT: Self = Self::new();
    fn lock(&self) {
        self.lock()
    }
    fn unlock(&self) {
        self.unlock()
    }
}

pub type SpinLock<T> = Lock<T, SpinLockInner>;
pub type SpinLockGuard<'a, T> = LockGuard<'a, T, SpinLockInner>;
pub type OwnedSpinLockGuard<'a, T, P> = OwnedLockGuard<T, SpinLockInner, P>;

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
