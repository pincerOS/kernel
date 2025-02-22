use super::condvar::CondVar;
use super::lock::SpinLock;

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
    pub fn sync_blocking(&self) {
        let mut guard = self.count.lock();
        assert!(*guard > 0);
        *guard -= 1;
        if *guard == 0 {
            self.condvar.notify_all();
        } else {
            self.condvar.wait_while_blocking(guard, |count| *count > 0);
        }
    }
    pub async fn sync(&self) {
        let mut guard = self.count.lock();
        assert!(*guard > 0);
        *guard -= 1;
        if *guard == 0 {
            self.condvar.notify_all();
            drop(guard);
        } else {
            self.condvar.wait_while(guard, |count| *count > 0).await;
        }
    }
}
