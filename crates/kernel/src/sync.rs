use core::arch::asm;
use core::cell::UnsafeCell;
use core::marker::PhantomData;
use core::mem::MaybeUninit;
use core::sync::atomic::{AtomicBool, Ordering};

use alloc::sync::Arc;

use crate::event::Event;

pub struct InterruptsState(pub u64);

impl core::fmt::Debug for InterruptsState {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("InterruptsState")
            .field(&format_args!("{:#08x}", self.0))
            .finish()
    }
}

extern "C" {
    fn get_interrupts_asm() -> u64;
    fn set_interrupts_asm(state: u64);
    fn disable_interrupts_asm();
    fn enable_interrupts_asm();
}

core::arch::global_asm!(
    "get_interrupts_asm: mrs x0, DAIF; ret",
    "set_interrupts_asm: msr DAIF, x0; ret",
    "disable_interrupts_asm: msr DAIFSet, #0b1111; ret",
    "enable_interrupts_asm: msr DAIFClr, #0b1111; ret",
);

pub fn get_interrupts() -> InterruptsState {
    InterruptsState(unsafe { get_interrupts_asm() })
}
pub unsafe fn disable_interrupts() -> InterruptsState {
    let state = get_interrupts();
    unsafe { disable_interrupts_asm() };
    state
}
pub unsafe fn enable_interrupts() -> InterruptsState {
    let state = get_interrupts();
    unsafe { enable_interrupts_asm() };
    state
}
pub unsafe fn restore_interrupts(state: InterruptsState) {
    unsafe { set_interrupts_asm(state.0) };
}

#[derive(Copy, Clone)]
pub struct Volatile<T>(pub *mut T);

impl<T> Volatile<T> {
    pub unsafe fn read(self) -> T {
        unsafe { core::ptr::read_volatile(self.0) }
    }
    pub unsafe fn write(self, value: T) {
        unsafe { core::ptr::write_volatile(self.0, value) }
    }
}

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
    lock: &'a Lock<T, L>,
    marker: PhantomData<*mut ()>,
}

pub struct OwnedLockGuard<T: ?Sized, L: LockImpl, P: RefProvider<Lock<T, L>>> {
    lock: P,
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
    pub fn get(&self) -> &T {
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

fn get_time_ticks() -> usize {
    let time;
    unsafe { asm!("mrs {time}, cntpct_el0", time = out(reg) time) };
    time
}
fn get_freq_ticks() -> usize {
    let freq;
    unsafe { asm!("mrs {freq}, cntfrq_el0", freq = out(reg) freq) };
    freq
}
fn convert_time_to_ticks(μs: usize) -> usize {
    (get_freq_ticks() / 250_000) * μs / 4
}
fn convert_ticks_to_time(ticks: usize) -> usize {
    // TODO: reduce chances of overflow
    (ticks * 1_000_000) / get_freq_ticks()
}

pub fn get_time() -> usize {
    convert_ticks_to_time(get_time_ticks())
}

pub fn spin_sleep(μs: usize) {
    let target = get_time() + μs;
    spin_sleep_until(target)
}

pub fn spin_sleep_until(target: usize) {
    let target = convert_time_to_ticks(target);
    while get_time_ticks() < target {
        // TODO: yield vs wfe/wfi?
        unsafe {
            asm!("yield");
        }
    }
}

/// Safety: The reference returned by provide must be valid
/// for the entire lifetime of the object, even if the object
/// has been moved.
// TODO: how does this relate to Pin?
pub unsafe trait RefProvider<T: ?Sized> {
    fn provide(&self) -> &T;
}

unsafe impl<T> RefProvider<T> for &T {
    fn provide(&self) -> &T {
        *self
    }
}
unsafe impl<T> RefProvider<T> for alloc::boxed::Box<T> {
    fn provide(&self) -> &T {
        &*self
    }
}
unsafe impl<T> RefProvider<T> for Arc<T> {
    fn provide(&self) -> &T {
        &**self
    }
}
unsafe impl<T> RefProvider<T> for alloc::rc::Rc<T> {
    fn provide(&self) -> &T {
        &**self
    }
}

type EventQueue = crate::scheduler::Queue<Event>;

pub struct CondVar {
    queue: EventQueue,
}

impl CondVar {
    pub const fn new() -> Self {
        Self {
            queue: EventQueue::new(),
        }
    }
    pub fn notify_one(&self) {
        if let Some(t) = self.queue.pop() {
            crate::event::SCHEDULER.add_task(t);
        }
    }
    pub fn notify_all(&self) {
        crate::event::SCHEDULER.add_all(&self.queue);
    }
    pub fn wait<'a, T>(&self, guard: SpinLockGuard<'a, T>) -> SpinLockGuard<'a, T> {
        let lock = guard.lock;
        core::mem::forget(guard);
        crate::thread::context_switch(crate::thread::SwitchAction::QueueAddUnlock(
            &self.queue,
            &lock.inner,
        ));
        lock.lock()
    }
    pub fn wait_while<'a, T, F>(
        &self,
        mut guard: SpinLockGuard<'a, T>,
        mut condition: F,
    ) -> SpinLockGuard<'a, T>
    where
        F: FnMut(&mut T) -> bool,
    {
        while condition(&mut *guard) {
            guard = self.wait(guard);
        }
        guard
    }

    pub fn wait_then<T, F, P, L>(&self, guard: OwnedSpinLockGuard<T, P>, f: F)
    where
        F: FnOnce(OwnedSpinLockGuard<T, P>) + Send + 'static,
        T: Send + 'static,
        P: RefProvider<Lock<T, SpinLockInner>> + Send + 'static,
    {
        let lock = guard.into_inner();
        let lock_ref = lock.provide();
        let lock_ptr = core::ptr::from_ref(lock_ref);

        let wrap = move || {
            let g = SpinLock::lock_owned(lock);
            f(g);
        };
        let event = Event::Function(alloc::boxed::Box::new(wrap));
        self.queue.add_then(event, move || {
            // Safety: the queue must not drop or release the event before
            // this function is called, so the ref provider must still be
            // valid.
            unsafe { &*lock_ptr }.inner.unlock()
        });
    }

    pub fn wait_then_owned<T, F, P, P2>(this: P, guard: OwnedSpinLockGuard<T, P2>, f: F)
    where
        P: RefProvider<Self> + Send + 'static,
        P2: RefProvider<Lock<T, SpinLockInner>> + Send + 'static,
        F: FnOnce(P, OwnedSpinLockGuard<T, P2>) + Send + 'static,
        T: Send + 'static,
    {
        // TODO: The arc should be downgraded to a weak pointer while in
        // the queue, such that the queue doesn't become a leaked ref cycle,
        // but there's no guarantee that the condvar is kept alive after
        // notify is called, so the Event must keep the condvar alive.
        let cond = this;
        let cond_ref = cond.provide();
        let cond_ptr = core::ptr::from_ref(cond_ref);

        let lock = guard.into_inner();
        let lock_ref = lock.provide();
        let lock_ptr = core::ptr::from_ref(lock_ref);

        let wrap = move || {
            let g = SpinLock::lock_owned(lock);
            f(cond, g);
        };
        let event = Event::Function(alloc::boxed::Box::new(wrap));

        // TODO: this may be UB, depending on the example implementation
        // of the queue's add -- there's no reasonable situation where it
        // would happen, as this event must be the last thing keeping the
        // queue alive, but if the queue implementation drops the thread,
        // then it would free itself and &self would become an invalid ref.
        unsafe { &(*cond_ptr) }.queue.add_then(event, move || {
            // Safety: the queue must not drop or release the event before
            // this function is called, so the ref provider must still be
            // valid.
            unsafe { &*lock_ptr }.inner.unlock();
        });
    }

    pub fn wait_while_then<'a, T, P, P2, Cond, Then>(
        this: P,
        mut guard: OwnedSpinLockGuard<T, P2>,
        mut condition: Cond,
        f: Then,
    ) where
        P: RefProvider<Self> + Send + 'static,
        Cond: FnMut(&mut T) -> bool + Send + 'static,
        Then: FnOnce(OwnedSpinLockGuard<T, P2>) + Send + 'static,
        P2: RefProvider<Lock<T, SpinLockInner>> + Send + 'static,
        T: Send + 'static,
    {
        if condition(&mut *guard) {
            Self::wait_then_owned(this, guard, move |this, guard| {
                Self::wait_while_then(this, guard, condition, f);
            });
        } else {
            f(guard);
        }
    }
}

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
}

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
