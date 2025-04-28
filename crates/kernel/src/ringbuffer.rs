use core::cell::UnsafeCell;
use core::mem::MaybeUninit;
use core::sync::atomic::{AtomicU32, Ordering};
use Ordering::{Relaxed, SeqCst};

use alloc::sync::Arc;

use crate::sync::{Condvar, SpinLock};

pub struct SpscRingBuffer<const N: usize, T> {
    // TODO: put head and tail in separate cache lines?
    head: AtomicU32,
    tail: AtomicU32,
    elems: UnsafeCell<[MaybeUninit<T>; N]>,
}

// SPSC ringbuffer; each side is expected to have a lock on their local copy.
impl<const N: usize, T> SpscRingBuffer<N, T> {
    const _ASSERT: () = const { assert!(N.next_power_of_two() == N) };
    const _ASSERT_DROP: () = const { assert!(!core::mem::needs_drop::<T>()) };

    pub const fn new() -> Self {
        SpscRingBuffer {
            head: AtomicU32::new(0),
            tail: AtomicU32::new(0),
            elems: UnsafeCell::new([const { MaybeUninit::uninit() }; N]),
        }
    }

    fn empty(head: u32, tail: u32) -> bool {
        head == tail
    }

    fn full(head: u32, tail: u32) -> bool {
        head == tail.wrapping_add(N as u32)
    }

    // Safety: must be the only sender for this queue
    pub unsafe fn try_send(&self, event: T) -> Result<(), T> {
        let cur_head = self.head.load(Relaxed);
        let cur_tail = self.tail.load(SeqCst);

        if Self::full(cur_head, cur_tail) {
            return Err(event);
        }

        let head_idx = (cur_head as usize).rem_euclid(N);
        assert!(head_idx < N);
        let elems = self.elems.get().cast::<T>();
        let target = elems.wrapping_add(head_idx);

        // Safety: cur_head is always within range for the elems array.
        // This does not create an intermediate reference, and just writes
        // to a value within an UnsafeCell.  If one side is buggy or malicious,
        // this can only corrupt the data/queue state, and cannot influence
        // the control flow on this side of the channel.
        //
        // Correctness: There is a single producer for this queue, so
        // we can write data to the unused region without synchronization
        // concerns.  The tail pointer will only shrink the region, and
        // not grow backwards; as this target is outside of the head..tail
        // region, it is unused.
        unsafe {
            target.write_volatile(event);
        }

        // After writing the event, fence and increment the head to make
        // the event visible to the consumer.
        // TODO: is SeqCst enough of a fence for this?
        self.head.fetch_add(1, SeqCst);

        Ok(())
    }

    // Safety: must be the only reciever for this queue
    pub unsafe fn try_recv(&self) -> Option<T> {
        let cur_head = self.head.load(SeqCst);
        let cur_tail = self.tail.load(Relaxed);
        // TODO: are the above loads enough of a fence?

        if Self::empty(cur_head, cur_tail) {
            return None;
        }

        let tail_idx = (cur_tail as usize).rem_euclid(N);
        assert!(tail_idx < N);
        let elems = self.elems.get().cast::<MaybeUninit<T>>();
        let target = elems.wrapping_add(tail_idx);

        // Safety: cur_tail is always within range for the elems array.
        // This does not create an intermediate reference, and just writes
        // to a value within an UnsafeCell.  If one side is buggy or malicious,
        // this can only corrupt the data/queue state, and cannot influence
        // the control flow on this side of the channel.
        //
        // Correctness: There is a single consumer for this queue;
        // events within the range (head, tail] are all initialized and unchanging.
        // The producer will only reuse the slot once the tail has been incremented.
        let event = unsafe { target.read_volatile().assume_init() };

        self.tail.fetch_add(1, SeqCst);

        Some(event)
    }
}

/// A single producer single consumer ring buffer, which overwrites
/// messages when the buffer is full.
///
/// Does not currently call messages' destructors when dropped or
/// overwritten, and does not allow messages to implement Drop.
pub struct SpscOverwritingRingBuffer<const N: usize, T> {
    // TODO: put head and tail in separate cache lines?
    head: AtomicU32,
    tail: AtomicU32,
    elems: UnsafeCell<[MaybeUninit<T>; N]>,
}

// SPSC ringbuffer; each side is expected to have a lock on their local copy.
impl<const N: usize, T> SpscOverwritingRingBuffer<N, T> {
    const _ASSERT: () = const { assert!(N.next_power_of_two() == N) };
    const _ASSERT_DROP: () = const { assert!(!core::mem::needs_drop::<T>()) };

    pub const fn new() -> Self {
        SpscOverwritingRingBuffer {
            head: AtomicU32::new(0),
            tail: AtomicU32::new(0),
            elems: UnsafeCell::new([const { MaybeUninit::uninit() }; N]),
        }
    }

    fn empty(head: u32, tail: u32) -> bool {
        head == tail
    }

    fn full(head: u32, tail: u32) -> bool {
        head == tail.wrapping_add(N as u32 - 1)
    }

    // Safety: must be the only sender for this queue
    pub unsafe fn send_overwrite(&self, event: T) {
        let cur_head = self.head.load(Relaxed);
        let cur_tail = self.tail.load(SeqCst);

        if Self::full(cur_head, cur_tail) {
            // If full, clear out a space.  If there is a concurrent recv call,
            // it will check at the end and retry.  (TODO: exp backoff?
            // as it is, this could DOS receivers)
            self.tail.fetch_add(1, SeqCst);
        }

        let head_idx = (cur_head as usize).rem_euclid(N);
        assert!(head_idx < N);
        let elems = self.elems.get().cast::<T>();
        let target = elems.wrapping_add(head_idx);

        // Safety: cur_head is always within range for the elems array.
        // This does not create an intermediate reference, and just writes
        // to a value within an UnsafeCell.  If one side is buggy or malicious,
        // this can only corrupt the data/queue state, and cannot influence
        // the control flow on this side of the channel.
        //
        // Correctness: There is a single producer for this queue, so
        // we can write data to the unused region without synchronization
        // concerns.  The tail pointer will only shrink the region, and
        // not grow backwards; as this target is outside of the head..tail
        // region, it is unused.
        unsafe {
            target.write_volatile(event);
        }

        // After writing the event, fence and increment the head to make
        // the event visible to the consumer.
        // TODO: is SeqCst enough of a fence for this?
        self.head.fetch_add(1, SeqCst);
    }

    // Safety: must be the only reciever for this queue
    pub unsafe fn try_recv(&self) -> Option<T> {
        loop {
            let cur_head = self.head.load(SeqCst);
            let cur_tail = self.tail.load(Relaxed);
            // TODO: are the above loads enough of a fence?

            if Self::empty(cur_head, cur_tail) {
                return None;
            }

            let tail_idx = (cur_tail as usize).rem_euclid(N);
            assert!(tail_idx < N);
            let elems = self.elems.get().cast::<MaybeUninit<T>>();
            let target = elems.wrapping_add(tail_idx);

            // Safety: cur_tail is always within range for the elems array.
            // This does not create an intermediate reference, and just writes
            // to a value within an UnsafeCell.  If one side is buggy or malicious,
            // this can only corrupt the data/queue state, and cannot influence
            // the control flow on this side of the channel.
            let event = unsafe { target.read_volatile() };

            if self
                .tail
                .compare_exchange(cur_tail, cur_tail.wrapping_add(1), SeqCst, SeqCst)
                .is_ok()
            {
                let event = unsafe { event.assume_init() };
                return Some(event);
            }
            // The buffer was full and a sender dropped a message (or another
            // receive call occurred, which shouldn't be allowed);
            // Retry.
        }
    }
}

pub fn channel<const N: usize, T>() -> (Sender<N, T>, Receiver<N, T>) {
    let inner = Arc::new(ChannelInner {
        buf: SpscRingBuffer::new(),
        len: SpinLock::new(0),
        cond: Condvar::new(),
    });
    let inner2 = Arc::clone(&inner);
    (Sender { inner: inner2 }, Receiver { inner })
}

unsafe impl<const N: usize, T: Send> Send for ChannelInner<N, T> {}
unsafe impl<const N: usize, T: Send> Sync for ChannelInner<N, T> {}

struct ChannelInner<const N: usize, T> {
    buf: SpscRingBuffer<N, T>,
    len: SpinLock<usize>,
    cond: Condvar,
}

pub struct Sender<const N: usize, T> {
    inner: Arc<ChannelInner<N, T>>,
}

pub struct Receiver<const N: usize, T> {
    inner: Arc<ChannelInner<N, T>>,
}

impl<const N: usize, T> Clone for Sender<N, T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}
impl<const N: usize, T> Sender<N, T> {
    pub fn try_send(&mut self, event: T) -> Result<(), T> {
        let res = unsafe { self.inner.buf.try_send(event) };
        if res.is_ok() {
            let mut guard = self.inner.len.lock();
            let old_len = *guard;
            *guard += 1;
            drop(guard);
            if old_len == 0 {
                self.inner.cond.notify_one();
            }
        }
        res
    }

    pub async fn send(&mut self, event: T) {
        let mut guard = self.inner.len.lock();
        guard = self
            .inner
            .cond
            .wait_while(guard, |len| *len == (N - 1))
            .await;

        let res = unsafe { self.inner.buf.try_send(event) };
        assert!(res.is_ok());

        let old_len = *guard;
        *guard += 1;
        drop(guard);

        if old_len == 0 {
            self.inner.cond.notify_one();
        }
    }
}

impl<const N: usize, T> Clone for Receiver<N, T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<const N: usize, T> Receiver<N, T> {
    pub fn try_recv(&mut self) -> Option<T> {
        let res = unsafe { self.inner.buf.try_recv() };
        if res.is_some() {
            let mut guard = self.inner.len.lock();
            let old_len = *guard;
            *guard -= 1;
            drop(guard);
            if old_len == N - 1 {
                self.inner.cond.notify_one();
            }
        }
        res
    }

    pub async fn recv(&mut self) -> T {
        let mut guard = self.inner.len.lock();
        guard = self.inner.cond.wait_while(guard, |len| *len == 0).await;

        let res = unsafe { self.inner.buf.try_recv() };
        let res = res.unwrap();

        let old_len = *guard;
        *guard -= 1;
        drop(guard);
        if old_len == N - 1 {
            self.inner.cond.notify_one();
        }

        res
    }

    pub fn recv_blocking(&mut self) -> T {
        let mut guard = self.inner.len.lock();
        guard = self.inner.cond.wait_while_blocking(guard, |len| *len == 0);

        let res = unsafe { self.inner.buf.try_recv() };
        let res = res.unwrap();

        let old_len = *guard;
        *guard -= 1;
        drop(guard);
        if old_len == N - 1 {
            self.inner.cond.notify_one();
        }

        res
    }
}

// TODO: proper oneshot SPSC channel (single-use version of Future for cs439)
pub fn spawn_thread<F, O>(f: F) -> Receiver<2, O>
where
    F: FnOnce() -> O + Send + 'static,
    O: Send + 'static,
{
    let (mut tx, rx) = channel();
    crate::event::thread::thread(move || {
        let res = f();
        tx.try_send(res).map_err(|_| ()).unwrap();
    });
    rx
}
