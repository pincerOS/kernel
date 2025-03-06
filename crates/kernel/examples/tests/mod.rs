use core::error::Error;

use alloc::boxed::Box;
use alloc::sync::Arc;
use kernel::{event, sync};

#[derive(Debug)]
struct AssertError(alloc::string::String);

impl core::fmt::Display for AssertError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Display::fmt(&self.0, f)
    }
}

impl core::error::Error for AssertError {}

macro_rules! kassert_eq {
    ($lhs:expr, $rhs:expr) => {{
        let lhs = $lhs;
        let rhs = $rhs;
        if lhs != rhs {
            let msg = alloc::format!(
                "Assertion failed at at {}:{}: {:?}\n    lhs = {:?}\n    rhs = {:?}",
                core::file!(), core::line!(),
                stringify!($lhs != $rhs),
                lhs, rhs,
            );
            return Err($crate::tests::AssertError(msg).into());
        }
    }};
    ($lhs:expr, $rhs:expr, $context:literal $(, $($args:tt)*)?) => {{
        let lhs = $lhs;
        let rhs = $rhs;
        if lhs != rhs {
            let msg = alloc::format!(
                "Assertion failed at at {}:{}: {:?}\n    lhs = {:?}\n    rhs = {:?}\n{}",
                core::file!(), core::line!(),
                stringify!($lhs != $rhs),
                lhs, rhs,
                format_args!($context, $($args)*),
            );
            return Err($crate::tests::AssertError(msg).into());
        }
    }};
}

test_case!(example_test);
fn example_test() {
    println!("Running test one");
}

// test_case!(async example_failure);
// async fn example_failure() -> Result<(), &'static str> {
//     println!("Running test two");
//     sync::spin_sleep(500000);
//     Err("hello!")
// }

test_case!(async async_barrier);
async fn async_barrier() -> Result<(), Box<dyn Error + Send + Sync>> {
    use core::sync::atomic::{AtomicU32, Ordering};

    let count = 16;
    let barrier = Arc::new(sync::Barrier::new(count + 1));
    let reached = Arc::new(AtomicU32::new(0));

    for i in 0..count {
        let r = reached.clone();
        let b: Arc<sync::Barrier> = barrier.clone();
        event::task::spawn_async(async move {
            println!("Starting thread {i}");

            // TODO: non-spinning sleep
            sync::spin_sleep(500_000);

            println!("Ending thread {i}");

            r.fetch_add(i, Ordering::SeqCst);
            b.sync().await;
        });
    }

    barrier.sync().await;

    kassert_eq!(reached.load(Ordering::SeqCst), count * (count - 1) / 2);

    Ok(())
}

// TODO: proper oneshot SPSC channel (single-use version of Future for cs439)
fn spawn_thread<F, O>(f: F) -> kernel::ringbuffer::Receiver<2, O>
where
    F: FnOnce() -> O + Send + 'static,
    O: Send + 'static,
{
    let (mut tx, rx) = kernel::ringbuffer::channel();
    event::thread::thread(move || {
        let res = f();
        tx.try_send(res).map_err(|_| ()).unwrap();
    });
    rx
}

test_case!(async thread_barrier);
async fn thread_barrier() -> Result<(), Box<dyn Error + Send + Sync>> {
    use core::sync::atomic::{AtomicU32, Ordering};

    let mut res = spawn_thread(move || {
        let count = 32;
        let barrier = Arc::new(sync::Barrier::new(count + 1));
        let reached = Arc::new(AtomicU32::new(0));

        for i in 0..count {
            let r = reached.clone();
            let b: Arc<sync::Barrier> = barrier.clone();
            event::thread::thread(move || {
                println!("Starting thread {i}");

                // TODO: non-spinning sleep
                sync::spin_sleep(500_000);

                println!("Ending thread {i}");

                r.fetch_add(i, Ordering::SeqCst);
                b.sync_blocking();
            });
        }
        barrier.sync_blocking();
        println!("End of preemption test");

        kassert_eq!(reached.load(Ordering::SeqCst), count * (count - 1) / 2);

        Ok(())
    });

    let res = res.recv().await;
    res
}
