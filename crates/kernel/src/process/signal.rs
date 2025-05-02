use core::sync::atomic::{Ordering, AtomicU8};

#[derive(Debug)]
pub struct SignalHandlers {
    pub user_page_fault_handler: Option<fn()>,
    pub kill_block_handler: Option<fn()>,
}

//These numbers are somewhat based on the Linux signal numbers
pub enum SignalCode {
    InHandler = u32::MAX as isize,
    Interrupt = 2,
    KillUnblockable = 9,
    PageFault = 11,
    KillBlockable = 15,
}

impl From<u32> for SignalCode {
    fn from(value: u32) -> SignalCode {
        match value {
            u32::MAX => SignalCode::InHandler,
            2 => SignalCode::Interrupt,
            9 => SignalCode::KillUnblockable,
            11 => SignalCode::PageFault,
            15 => SignalCode::KillBlockable,
            _ => panic!("Failed to convert u32 {} into signal code", value),
        }
    }
}

impl From<SignalCode> for u32 {
    fn from(value: SignalCode) -> u32 {
        match value {
            SignalCode::InHandler => u32::MAX,
            SignalCode::Interrupt => 2,
            SignalCode::KillUnblockable => 9,
            SignalCode::PageFault => 11,
            SignalCode::KillBlockable => 15,
        }
    }
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

    pub fn set(&self, flag: SignalFlagOptions, val: bool) {
        if val {
            self.0.fetch_or(flag.bits(), Ordering::SeqCst);
        } else {
            self.0.fetch_and(!flag.bits(), Ordering::SeqCst);
        }
    }

    pub fn contains(&self, flag: SignalFlagOptions) -> bool {
        return (self.0.load(Ordering::SeqCst) & flag.bits()) != 0;
    }
}

