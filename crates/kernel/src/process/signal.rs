

#[derive(Debug)]
pub struct SignalHandlers {
    pub user_page_fault_handler: Option<fn()>,
    pub kill_block_handler: Option<fn()>,
}

//TODO: replace this with a queue
bitflags::bitflags! {
    
    #[derive(Clone, Copy, Debug)]
    pub struct SignalFlags: u8 {
        const IS_KILL = 1 << 0; //can block this
        const IS_DEAD = 1 << 1; //can't block this
        const IN_HANDLER = 1 << 2; //currently not supporting nested signal handlers
    }
}

impl SignalHandlers {
    pub fn new() -> SignalHandlers {
        SignalHandlers { user_page_fault_handler: None, kill_block_handler: None }
    }

}

