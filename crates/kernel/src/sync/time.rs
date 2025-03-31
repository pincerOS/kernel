use core::future::Future;
use core::sync::atomic::{AtomicU64, Ordering};
use core::task::Poll;

use alloc::boxed::Box;
use alloc::collections::binary_heap::BinaryHeap;
use alloc::vec::Vec;

use crate::arch::{get_freq_ticks, get_time_ticks, yield_};

use super::SpinLock;

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
        unsafe { yield_() };
    }
}

pub async fn sleep(μs: u64) {
    SCHEDULER.sleep(μs).await;
}
pub async fn sleep_until(target: u64) {
    SCHEDULER.sleep_until(target).await;
}

pub trait TimerBackend {
    fn get_time(&self) -> u64;
}

pub struct SystemTimerInterface;

impl TimerBackend for SystemTimerInterface {
    fn get_time(&self) -> u64 {
        crate::device::system_timer::get_time()
    }
}

struct TimerEvent {
    _target: u64,
    waker: core::task::Waker,
}

#[derive(Copy, Clone, Debug)]
struct EventIndex(usize);

#[derive(Debug)]
struct TimerQueueItem {
    time: u64,
    index: EventIndex,
}
impl PartialOrd for TimerQueueItem {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for TimerQueueItem {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.time.cmp(&other.time).reverse()
    }
}
impl PartialEq for TimerQueueItem {
    fn eq(&self, other: &Self) -> bool {
        self.time == other.time
    }
}
impl Eq for TimerQueueItem {}

pub struct TimerScheduler {
    backend: Box<dyn TimerBackend + Send + Sync>,
    tasks: SpinLock<Vec<Option<TimerEvent>>>,
    events: SpinLock<BinaryHeap<TimerQueueItem>>,
    min_time: AtomicU64,
}

const fn system_timer_box() -> Box<SystemTimerInterface> {
    assert!(size_of::<SystemTimerInterface>() == 0);
    unsafe {
        core::mem::transmute::<*mut SystemTimerInterface, Box<SystemTimerInterface>>(
            core::ptr::dangling_mut(),
        )
    }
}

pub static SCHEDULER: TimerScheduler = TimerScheduler::new(system_timer_box());

impl TimerScheduler {
    pub const fn new(backend: Box<dyn TimerBackend + Send + Sync>) -> Self {
        Self {
            backend,
            tasks: SpinLock::new(Vec::new()),
            events: SpinLock::new(BinaryHeap::new()),
            min_time: AtomicU64::new(u64::MAX),
        }
    }

    pub fn is_ready(&self) -> bool {
        let cur_time = self.backend.get_time();
        self.min_time.load(Ordering::Acquire) <= cur_time
    }

    pub fn run(&self) {
        let mut tasks = self.tasks.lock();
        let mut events = self.events.lock();
        // TODO: ensure that time is monotonically increasing, and consistent
        // across cores.  (So that other cores don't see an older time than
        // this when waking and go back to sleep forever.)
        let now = self.backend.get_time();
        while let Some(ev) = events.peek() {
            if ev.time > now {
                break;
            }
            let event = ev.index;
            println!("Waking event with time {} (at time {})", ev.time, now);
            events.pop();
            if let Some(task) = tasks.get_mut(event.0).and_then(|o| o.take()) {
                task.waker.wake();
            }
        }
        let next_time = events.peek().map(|ev| ev.time).unwrap_or(u64::MAX);
        self.min_time.store(next_time, Ordering::Release);
    }

    fn register(&self, target: u64, waker: core::task::Waker) -> EventIndex {
        // TODO: proper slab allocator
        let mut guard = self.tasks.lock();
        guard.push(Some(TimerEvent {
            _target: target,
            waker,
        }));
        let idx = EventIndex(guard.len() - 1);
        drop(guard);
        self.events.lock().push(TimerQueueItem {
            time: target,
            index: idx,
        });
        self.min_time.fetch_min(target, Ordering::Release);
        idx
    }

    fn unregister(&self, idx: EventIndex) {
        // TODO: proper slab allocator
        let mut guard = self.tasks.lock();
        guard[idx.0] = None;
        // TODO: remove from heap?
    }

    fn timer_future(&'static self, target: Option<u64>, period: Option<u64>) -> TimerFuture {
        TimerFuture {
            scheduler: self,
            target,
            period: period.unwrap_or(u64::MAX),
            state: TimerFutureState::Unregistered,
        }
    }

    pub fn sleep(&'static self, duration: u64) -> TimerFuture {
        let cur_time = self.backend.get_time();
        let target = cur_time + duration;
        self.timer_future(Some(target), None)
    }
    pub fn sleep_until(&'static self, target: u64) -> TimerFuture {
        self.timer_future(Some(target), None)
    }
    pub fn interval(&'static self, interval: u64) -> Interval {
        let cur_time = self.backend.get_time();
        println!("Interval: {interval}, time {}", cur_time);
        Interval(self.timer_future(Some(cur_time + interval), Some(interval)))
    }
}

pub struct TimerFuture {
    scheduler: &'static TimerScheduler,
    target: Option<u64>,
    period: u64,
    state: TimerFutureState,
}

#[derive(Debug)]
enum TimerFutureState {
    Unregistered,
    Registered {
        idx: EventIndex,
        waker: core::task::Waker,
    },
}

impl TimerFuture {
    fn poll_next(&mut self, cx: &mut core::task::Context<'_>) -> Poll<()> {
        let Some(target) = self.target else {
            return Poll::Pending;
        };

        let now = self.scheduler.backend.get_time();
        if now >= target {
            if let Some(next_target) = target.checked_add(self.period) {
                if let TimerFutureState::Registered { idx, .. } = self.state {
                    self.scheduler.unregister(idx);
                }

                self.target = Some(next_target);

                let waker = cx.waker();
                let idx = self.scheduler.register(next_target, waker.clone());
                self.state = TimerFutureState::Registered {
                    idx,
                    waker: waker.clone(),
                };
            } else {
                self.target = None;
            }
            return Poll::Ready(());
        }

        match self.state {
            TimerFutureState::Unregistered => {
                let waker = cx.waker();
                let idx = self.scheduler.register(target, waker.clone());
                self.state = TimerFutureState::Registered {
                    idx,
                    waker: waker.clone(),
                };
            }
            TimerFutureState::Registered {
                ref mut idx,
                ref mut waker,
            } => {
                if !cx.waker().will_wake(waker) {
                    let new_waker = cx.waker();
                    self.scheduler.unregister(*idx);
                    *idx = self.scheduler.register(target, new_waker.clone());
                    *waker = new_waker.clone();
                }
            }
        }
        Poll::Pending
    }
}

impl Future for TimerFuture {
    type Output = ();
    fn poll(
        self: core::pin::Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> Poll<Self::Output> {
        self.get_mut().poll_next(cx)
    }
}

impl Drop for TimerFuture {
    fn drop(&mut self) {
        if let TimerFutureState::Registered { idx, .. } = self.state {
            self.scheduler.unregister(idx);
        }
    }
}

pub struct Interval(TimerFuture);

impl Interval {
    pub fn tick(&mut self) -> impl Future<Output = bool> + '_ {
        core::future::poll_fn(|cx| self.0.poll_next(cx).map(|_| true))
    }
}
