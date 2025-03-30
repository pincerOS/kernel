use alloc::collections::VecDeque;

use crate::arch::{sev, wfe};
use crate::sync::InterruptSpinLock;

pub struct Scheduler<E> {
    rt_queue: Queue<E>,
    queue: Queue<E>,
}

impl<E> Scheduler<E> {
    pub const fn new() -> Self {
        Scheduler {
            rt_queue: Queue::new(),
            queue: Queue::new(),
        }
    }
    pub fn add_rt_task(&self, event: E) {
        self.rt_queue.add(event);
        // unblock WFEs on other cores
        unsafe { sev() };
    }
    pub fn add_task(&self, event: E) {
        self.queue.add(event);
        // unblock WFEs on other cores
        unsafe { sev() };
    }
    pub fn add_all(&self, queue: &Queue<E>) {
        self.queue.0.lock().append(&mut *queue.0.lock());
    }

    pub fn wait_for_task(&self) -> E {
        loop {
            if let Some(c) = self.rt_queue.pop() {
                break c;
            }
            if let Some(c) = self.queue.pop() {
                break c;
            }
            // TODO: race condition here
            unsafe { wfe() };
        }
    }
}

pub struct Queue<E>(pub InterruptSpinLock<VecDeque<E>>);

impl<E> Queue<E> {
    pub const fn new() -> Self {
        Self(InterruptSpinLock::new(VecDeque::new()))
    }
    pub fn add(&self, event: E) {
        self.0.lock().push_back(event);
    }
    pub fn pop(&self) -> Option<E> {
        self.0.lock().pop_front()
    }
    pub fn add_then(&self, event: E, then: impl FnOnce()) {
        let mut guard = self.0.lock();
        guard.push_back(event);
        then()
    }
}

unsafe impl<E> Sync for Scheduler<E> where E: Send {}
unsafe impl<E> Send for Queue<E> where E: Send {}
unsafe impl<E> Sync for Queue<E> where E: Send {}
