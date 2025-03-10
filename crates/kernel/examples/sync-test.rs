#![no_std]
#![no_main]

extern crate alloc;
extern crate kernel;

use kernel::*;
use alloc::sync::Arc;
use alloc::vec::Vec;

#[no_mangle]
extern "Rust" fn kernel_main(_device_tree: device_tree::DeviceTree) {
    println!("*** starting spinlock test");
    let counter = Arc::new(sync::SpinLock::new(0));
    for i in 0..100 {
        let counter_clone = counter.clone();
        thread::thread(move || {
            let mut guard = counter_clone.lock();
            *guard += 1;
            // let core_id = crate::arch::core_id();
            // println!("Core {} incremented counter to {}", core_id, *guard);
            if *guard == 100 {
                println!("Incremented to 100");
            }
        });
    }
    
    sync::spin_sleep(1_000_000);
    
    let final_value = *counter.lock();
    println!("Final counter value: {}", final_value);
    assert_eq!(final_value, 100, "should be 100");
    
    println!("*** spinlock test completed!");

    println!("*** starting barrier test");
    let counter = Arc::new(sync::SpinLock::new(0));
    let barrier = Arc::new(sync::Barrier::new(100));
    let other_barrier = Arc::new(sync::Barrier::new(101));
    
    for i in 0..100 {
        let counter_clone = counter.clone();
        let barrier_clone = barrier.clone();
        let other_barrier_clone = other_barrier.clone();
        thread::thread(move || {
            {
                let mut guard = counter_clone.lock();
                *guard += 1;
            }
            
            barrier_clone.sync(); 
            
            let value = *counter_clone.lock();
            if value != 100 {
                println!("Error: Expected 100, got {}", value);
            }
            other_barrier_clone.sync();
        });
    }
    other_barrier.sync();

    let final_value = *counter.lock();
    println!("Final counter value: {}", final_value);
    assert_eq!(final_value, 100, "should be 100");

    println!("*** barrier test completed!");

    println!("*** starting blocking lock test");
    let locks: Arc<Vec<sync::blocking_lock::BlockingLockInner<>>> = Arc::new((0..10).map(|_| sync::blocking_lock::BlockingLockInner::new()).collect());
    
    for i in 1..10 {
        locks[i].lock();
    }
    
    for i in 0..10 {
        let locks = locks.clone();
        thread::thread(move || {
            println!("Thread {} waiting...", i);
            
            if i > 0 {
                locks[i].lock();
            }
            
            println!("Thread {} running!", i);
            
            sync::spin_sleep(100_000);
            
            if i < 9 {
                locks[i + 1].unlock();
            }
        });
    }

    sync::spin_sleep(2_000_000);
    println!("*** blocking lock test completed!");

    println!("*** starting semaphore test");
    struct Semaphore {
        count: sync::SpinLock<usize>,
        condvar: sync::CondVar,
    }

    impl Semaphore {
        const fn new(initial: usize) -> Self {
            Self {
                count: sync::SpinLock::new(initial),
                condvar: sync::CondVar::new(),
            }
        }

        fn acquire(&self) {
            let mut guard = self.count.lock();
            while *guard == 0 {
                guard = self.condvar.wait(guard);
            }
            *guard -= 1;
        }

        fn release(&self) {
            let mut guard = self.count.lock();
            *guard += 1;
            self.condvar.notify_one();
        }
    }

    let sem = Arc::new(Semaphore::new(3));
    let counter = Arc::new(sync::SpinLock::new(0));
    
    for i in 0..10 {
        let sem = sem.clone();
        let counter = counter.clone();
        thread::thread(move || {
            println!("Thread {} waiting for semaphore...", i);
            sem.acquire();
            
            println!("Thread {} acquired semaphore", i);
            {
                let mut count = counter.lock();
                *count += 1;
                assert!(*count <= 3, "Too many threads running concurrently!");
            }
            
            sync::spin_sleep(100_000);
            
            {
                let mut count = counter.lock();
                *count -= 1;
            }
            
            sem.release();
            println!("Thread {} released semaphore", i);
        });
    }

    sync::spin_sleep(2_000_000);
    println!("*** semaphore test completed!");
}
