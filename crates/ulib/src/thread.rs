extern crate alloc;

use alloc::boxed::Box;
use core::mem::MaybeUninit;

use crate::sys::{exit, spawn};

pub fn spawn_thread<F>(func: F)
where
    F: FnOnce() + Send + 'static,
{
    let stack = Box::leak(alloc::vec![0; 8 * 8192 / 16].into_boxed_slice());

    let (a, b, _c) = unsafe { stack.align_to_mut::<MaybeUninit<F>>() };
    let func_ptr;
    let sp;
    if size_of::<F>() == 0 {
        func_ptr = a.as_mut_ptr_range().end as usize;
        sp = a.as_mut_ptr_range().end as usize;
    } else {
        let f = b.last_mut().unwrap();
        f.write(func);
        func_ptr = f as *mut _ as usize;
        sp = (f as *mut _ as usize) & !0xF;
    }
    let sp = sp as *const u128;
    assert!(sp.is_aligned());

    extern "C" fn spawn_inner<F>(ptr: *mut F)
    where
        F: FnOnce() + Send + 'static,
    {
        (unsafe { ptr.read() })();

        // TODO: this leaks the stack
        exit(0);
    }

    const FLAG_CLONE: usize = 1;
    unsafe { spawn(spawn_inner::<F> as usize, sp as usize, func_ptr, FLAG_CLONE).unwrap() };
}
