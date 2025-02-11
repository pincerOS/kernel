pub mod barrier;
pub mod blocking_lock;
pub mod condvar;
pub mod init;
pub mod lock;
pub mod per_core;
pub mod time;

pub use crate::arch::interrupts;

pub use barrier::Barrier;
pub use blocking_lock::{BlockingLock, BlockingLockGuard};
pub use condvar::CondVar;
pub use init::UnsafeInit;
pub use interrupts::{disable_interrupts, enable_interrupts, restore_interrupts, InterruptsState};
pub use lock::{InterruptSpinLock, InterruptSpinLockGuard, InterruptSpinLockInner};
pub use lock::{Lock, LockGuard, LockImpl};
pub use lock::{SpinLock, SpinLockGuard, SpinLockInner};
pub use per_core::{ConstInit, PerCore};
pub use time::{get_time, spin_sleep, spin_sleep_until};

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
