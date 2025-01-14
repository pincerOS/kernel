use core::arch::asm;
use core::cell::UnsafeCell;
use core::marker::PhantomData;
use core::mem::MaybeUninit;
use core::sync::atomic::{AtomicBool, Ordering};

#[derive(Debug)]
pub struct InterruptsState(u64);

pub fn get_interrupts() -> InterruptsState {
    let daif: u64;
    unsafe {
        asm!(
            "mrs {out}, DAIF",
            out = out(reg) daif,
        );
    }
    InterruptsState(daif)
}

pub fn disable_interrupts() -> InterruptsState {
    let daif: u64;
    unsafe {
        asm!(
            "mrs {out}, DAIF",
            "msr DAIFSet, #0b1111",
            out = out(reg) daif,
        );
    }
    InterruptsState(daif)
}
pub fn restore_interrupts(state: InterruptsState) {
    unsafe {
        asm!("msr DAIF, {state}", state = in(reg) state.0);
    }
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
}

unsafe impl<T: ?Sized, L> Send for Lock<T, L>
where
    T: Send,
    L: Send,
{
}
unsafe impl<T: ?Sized, L> Sync for Lock<T, L>
where
    T: Send,
    L: Sync,
{
}

impl<T: ?Sized, L> core::ops::Deref for LockGuard<'_, T, L>
where
    L: LockImpl,
{
    type Target = T;
    fn deref(&self) -> &T {
        let ptr = self.lock.value.get();
        unsafe { &*ptr }
    }
}
impl<T: ?Sized, L> core::ops::DerefMut for LockGuard<'_, T, L>
where
    L: LockImpl,
{
    fn deref_mut(&mut self) -> &mut T {
        let ptr = self.lock.value.get();
        unsafe { &mut *ptr }
    }
}

impl<T: ?Sized, L> core::ops::Drop for LockGuard<'_, T, L>
where
    L: LockImpl,
{
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
        let mut state = disable_interrupts();
        while !self.try_acquire() {
            restore_interrupts(state);
            while self.flag.load(Ordering::Relaxed) {
                core::hint::spin_loop();
            }
            state = disable_interrupts();
        }
        unsafe {
            self.state.get().write(Some(state));
        }
    }
    pub fn unlock(&self) {
        let state = unsafe { (*self.state.get()).take() };
        self.flag.store(false, Ordering::Release);
        restore_interrupts(state.unwrap())
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

pub struct CondVar {
    queue: crate::thread::ThreadQueue,
}

impl CondVar {
    pub const fn new() -> Self {
        Self {
            queue: crate::thread::ThreadQueue::new(),
        }
    }
    pub fn notify_one(&self) {
        if let Some(t) = self.queue.pop() {
            crate::thread::SCHEDULER.add_task(t);
        }
    }
    pub fn notify_all(&self) {
        crate::thread::SCHEDULER.add_all(&self.queue);
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
