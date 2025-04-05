use core::arch::asm;
use core::future::Future;
use core::pin::Pin;
use core::task::Poll;

use crate::arch::memory;

pub use memory::init;
pub use memory::{
    clean_physical_buffer_for_device, invalidate_physical_buffer_for_device, physical_addr,
};
pub use memory::{map_device, map_device_block, map_physical, map_physical_noncacheable};

unsafe fn enable_user_vmem(user_ttbr0: usize) {
    // TODO: restore old ttbr0?
    let cur_ttbr0: usize;
    unsafe { asm!("mrs {0}, TTBR0_EL1", out(reg) cur_ttbr0) };

    // If the page table has changed, switch back to this thread's address space.
    if cur_ttbr0 != user_ttbr0 {
        unsafe {
            asm!("msr TTBR0_EL1, {0}", "isb", "dsb sy", "tlbi vmalle1is", "dsb sy", in(reg) user_ttbr0)
        };
    }
}

pub unsafe fn with_user_vmem<F, O>(ttbr0: usize, callback: F) -> O
where
    F: FnOnce() -> O,
{
    unsafe { enable_user_vmem(ttbr0) };
    callback()
}

pub unsafe fn with_user_vmem_async<F, O>(
    ttbr0: usize,
    callback: F,
) -> impl Future<Output = O> + use<F, O>
where
    F: Future<Output = O>,
{
    struct UserVmemWrap<F> {
        ttbr0: usize,
        fut: F,
    }
    impl<F, O> Future for UserVmemWrap<F>
    where
        F: Future<Output = O>,
    {
        type Output = O;
        fn poll(self: Pin<&mut Self>, cx: &mut core::task::Context<'_>) -> Poll<Self::Output> {
            unsafe { enable_user_vmem(self.as_ref().ttbr0) };
            let inner = unsafe { self.map_unchecked_mut(|f| &mut f.fut) };
            inner.poll(cx)
        }
    }
    UserVmemWrap {
        ttbr0,
        fut: callback,
    }
}
