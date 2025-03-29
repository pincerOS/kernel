use super::{Condvar, SpinLock};

pub struct Semaphore {
    count: SpinLock<isize>,
    cvar: Condvar,
}

impl Semaphore {
    pub const fn new(value: isize) -> Self {
        Semaphore {
            count: SpinLock::new(value),
            cvar: Condvar::new(),
        }
    }
    pub async fn down(&self) {
        let mut count = self.count.lock();
        count = self.cvar.wait_while(count, |count| *count <= 0).await;
        *count -= 1;
    }
    pub fn down_blocking(&self) {
        let mut count = self.count.lock();
        count = self.cvar.wait_while_blocking(count, |count| *count <= 0);
        *count -= 1;
    }
    pub fn up(&self) {
        let mut count = self.count.lock();
        *count += 1;
        self.cvar.notify_one();
        drop(count);
    }
}

pub struct BinarySemaphore {
    state: SpinLock<bool>,
    cvar: Condvar,
}

impl BinarySemaphore {
    pub const fn new(value: bool) -> Self {
        BinarySemaphore {
            state: SpinLock::new(value),
            cvar: Condvar::new(),
        }
    }
    pub async fn down(&self) {
        let mut state = self.state.lock();
        state = self.cvar.wait_while(state, |state| !*state).await;
        *state = false;
    }
    pub fn down_blocking(&self) {
        let mut state = self.state.lock();
        state = self.cvar.wait_while_blocking(state, |state| !*state);
        *state = false;
    }
    pub fn up(&self) {
        let mut state = self.state.lock();
        *state = true;
        self.cvar.notify_one();
        drop(state);
    }
}
