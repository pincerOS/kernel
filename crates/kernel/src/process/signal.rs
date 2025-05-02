use core::sync::atomic::{Ordering, AtomicU8};

#[derive(Debug)]
pub struct SignalHandlers {
    pub user_page_fault_handler: Option<fn()>,
    pub kill_block_handler: Option<fn()>,
}

//These numbers are somewhat based on the Linux signal numbers
pub enum SignalCode {
    InHandler = -1,
    Interrupt = 2,
    KilledUnblockable = 9,
    PageFault = -4, //For compatibility with page fault handler
    KilledBlockable = 15,
}

//TODO: replace this with a queue
bitflags::bitflags! {
    
    #[derive(Clone, Copy, Debug)]
    pub struct SignalFlagOptions: u8 {
        const IS_KILL = 1 << 0; //can block this
        const IS_DEAD = 1 << 1; //can't block this
        const IS_PAGE_FAULT = 1 << 2;
        //currently not supporting nested signal handlers
        //WARN: this may break with multiple threads in a process
        const IN_HANDLER = 1 << 3; 
    }
}

impl SignalHandlers {
    pub fn new() -> SignalHandlers {
        SignalHandlers { user_page_fault_handler: None, kill_block_handler: None }
    }

}

pub struct SignalFlags(AtomicU8);

impl SignalFlags {
    pub fn empty() -> Self {
        return SignalFlags(AtomicU8::new(0));
    }

    pub fn set(&self, option: SignalFlagOptions, val: bool) {
        if val {
            self.0.fetch_or(option as u8, Ordering::SeqCst);
        } else {
            self.0.fetch_and(!(option as u8), Ordering::SeqCst);
        }
    }

    pub fn contains(&self, flag: SignalFlagOptions) -> bool {
        return self.0.load(Ordering::SeqCst) & (flag as u8);
    }
}

