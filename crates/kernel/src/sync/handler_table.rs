use core::sync::atomic::{AtomicUsize, Ordering};

pub struct HandlerTableInner<const COUNT: usize> {
    table: [AtomicUsize; COUNT],
}

impl<const COUNT: usize> HandlerTableInner<COUNT> {
    pub const fn new(fallback: usize) -> Self {
        let mut table = [const { AtomicUsize::new(0) }; COUNT];
        let mut i = 0;
        while i < COUNT {
            table[i] = AtomicUsize::new(fallback);
            i += 1;
        }
        HandlerTableInner { table }
    }
    pub fn get(&self, num: usize) -> usize {
        self.table[num % COUNT].load(Ordering::Relaxed)
    }
    pub fn set(&self, num: usize, func: usize) {
        self.table[num].store(func, Ordering::SeqCst);
    }
}
