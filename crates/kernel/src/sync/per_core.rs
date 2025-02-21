pub trait ConstInit {
    const INIT: Self;
}

#[repr(align(64))]
struct PerCoreInner<T>(core::cell::RefCell<T>);

pub struct PerCore<T>([PerCoreInner<T>; 4]);

impl<T> PerCore<T>
where
    T: ConstInit,
{
    pub const fn new() -> Self {
        Self([const { PerCoreInner(core::cell::RefCell::new(ConstInit::INIT)) }; 4])
    }

    pub fn with_current<F, O>(&self, f: F) -> O
    where
        F: FnOnce(&mut T) -> O,
    {
        let state = unsafe { crate::sync::disable_interrupts() };
        let core_id = crate::arch::core_id() & 0b11;
        let mut inner = self.0[core_id as usize].0.borrow_mut();
        let res = f(&mut *inner);
        drop(inner);
        unsafe { crate::sync::restore_interrupts(state) };
        res
    }
}

unsafe impl<T> Sync for PerCore<T> where T: Send {}
