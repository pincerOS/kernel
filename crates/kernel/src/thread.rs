use alloc::boxed::Box;
use core::ptr::NonNull;

use crate::context::{context_switch, Context, SwitchAction, CORES};
use crate::event::{Event, SCHEDULER};

pub fn yield_() {
    context_switch(SwitchAction::Yield);
}

pub fn stop() -> ! {
    context_switch(SwitchAction::FreeThread);
    unreachable!()
}

pub struct Thread {
    pub last_context: NonNull<Context>,
    pub stack: NonNull<[u128]>,
    // TODO: is this worth storing inline in the struct?
    pub func: Option<Box<dyn FnOnce() + Send>>,
}

unsafe impl Send for Thread {}

impl Thread {
    pub fn new(stack: NonNull<[u128]>, func: Option<Box<dyn FnOnce() + Send>>) -> Self {
        // reuse the lowest region of the stack as the initial context
        assert!(stack.len() >= size_of::<Context>());
        let end = unsafe { stack.cast::<u128>().add(stack.len()) };
        let context = unsafe { end.cast::<Context>().sub(1) };
        assert!(context.is_aligned());

        let data = Context {
            regs: [0; 31],
            sp: end.as_ptr() as usize,
            link_reg: init_thread as usize,
            spsr: 0b0101, // Stay in EL1, using the EL1 sp
        };
        unsafe {
            core::ptr::write(context.as_ptr(), data);
        }

        Thread {
            stack,
            last_context: context,
            func,
        }
    }
}

impl Drop for Thread {
    fn drop(&mut self) {
        let b = unsafe { Box::from_raw(self.stack.as_ptr()) };
        drop(b);
    }
}

const STACK_SIZE: usize = 16384;

pub fn thread<F>(f: F)
where
    F: FnOnce() + Send + 'static,
{
    let stack = Box::<[u128]>::new_uninit_slice(STACK_SIZE / size_of::<u128>());
    let stack = NonNull::new(Box::into_raw(stack) as *mut [u128]).unwrap();
    let thread = Box::new(Thread::new(stack, Some(Box::new(f))));

    SCHEDULER.add_task(Event::ScheduleThread(thread));
}

#[no_mangle]
extern "C" fn init_thread() {
    let func = CORES.with_current(|core| {
        let mut thread = core.thread.take().unwrap();
        let func = thread.func.take().unwrap();
        core.thread.set(Some(thread));
        func
    });

    func();

    stop();
}
