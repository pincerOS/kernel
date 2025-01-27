use core::cell::UnsafeCell;
use core::mem::MaybeUninit;
use core::sync::atomic::{AtomicU32, Ordering};

use alloc::sync::Arc;

pub struct RingBuffer<const N: usize, T> {
    // TODO: put head and tail in separate cache lines?
    head: AtomicU32,
    tail: AtomicU32,
    elems: UnsafeCell<[MaybeUninit<T>; N]>,
}

// SPSC ringbuffer; each side is expected to have a lock on their local copy.
impl<const N: usize, T> RingBuffer<N, T> {
    pub fn new() -> Self {
        RingBuffer {
            head: AtomicU32::new(0),
            tail: AtomicU32::new(0),
            elems: UnsafeCell::new([const { MaybeUninit::uninit() }; N]),
        }
    }

    fn empty(head: u32, tail: u32) -> bool {
        (head as i64 - tail as i64).rem_euclid(N as i64) == 0
    }

    fn full(head: u32, tail: u32) -> bool {
        (head as i64 - (tail as i64 - 1)).rem_euclid(N as i64) == 0
    }

    // Safety: must be the only sender for this queue
    pub unsafe fn try_send(&self, event: T) -> Result<(), T> {
        let cur_head = self.head.load(Ordering::Relaxed).rem_euclid(N as u32);
        let cur_tail = self.tail.load(Ordering::SeqCst);

        if Self::full(cur_head, cur_tail) {
            return Err(event);
        }

        assert!((cur_head as usize) < N);
        let elems = self.elems.get().cast::<T>();
        let target = elems.wrapping_add(cur_head as usize);

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
        self.head.fetch_add(1, Ordering::SeqCst);

        Ok(())
    }

    // Safety: must be the only reciever for this queue
    pub unsafe fn try_recv(&self) -> Option<T> {
        let cur_head = self.head.load(Ordering::SeqCst);
        let cur_tail = self.tail.load(Ordering::Relaxed).rem_euclid(N as u32);
        // TODO: are the above loads enough of a fence?

        if Self::empty(cur_head, cur_tail) {
            return None;
        }

        assert!((cur_tail as usize) < N);
        let elems = self.elems.get().cast::<T>();
        let target = elems.wrapping_add(cur_tail as usize);

        // Safety: cur_tail is always within range for the elems array.
        // This does not create an intermediate reference, and just writes
        // to a value within an UnsafeCell.  If one side is buggy or malicious,
        // this can only corrupt the data/queue state, and cannot influence
        // the control flow on this side of the channel.
        //
        // Correctness: There is a single consumer for this queue;
        // events within the range (head, tail] are all initialized and unchanging.
        // The producer will only reuse the slot once the tail has been incremented.
        let event = unsafe { target.read_volatile() };

        self.tail.fetch_add(1, Ordering::SeqCst);

        Some(event)
    }
}

pub fn channel<const N: usize, T>() -> (Sender<N, T>, Receiver<N, T>) {
    let inner = Arc::new(ChannelInner {
        buf: RingBuffer::new(),
    });
    let inner2 = Arc::clone(&inner);
    (Sender { inner: inner2 }, Receiver { inner })
}

unsafe impl<const N: usize, T: Send> Send for ChannelInner<N, T> {}
unsafe impl<const N: usize, T: Send> Sync for ChannelInner<N, T> {}

struct ChannelInner<const N: usize, T> {
    buf: RingBuffer<N, T>,
}

pub struct Sender<const N: usize, T> {
    inner: Arc<ChannelInner<N, T>>,
}

pub struct Receiver<const N: usize, T> {
    inner: Arc<ChannelInner<N, T>>,
}

impl<const N: usize, T> Sender<N, T> {
    pub fn try_send(&mut self, event: T) -> Result<(), T> {
        unsafe { self.inner.buf.try_send(event) }
    }
}

impl<const N: usize, T> Receiver<N, T> {
    pub fn try_recv(&mut self) -> Option<T> {
        unsafe { self.inner.buf.try_recv() }
    }
}
