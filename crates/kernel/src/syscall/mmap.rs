use crate::event::context::{Context, CORES};

pub unsafe fn sys_mmap(ctx: &mut Context) -> *mut Context {
    let req_start_addr: usize = ctx.regs[0];
    let req_size: usize = ctx.regs[1];
    //TODO: use this later
    let _prot_flags: u32 = ctx.regs[2].try_into().unwrap();
    //TODO: update this to be flags later
    let fill_pages: bool = ctx.regs[3] != 0;
    let fd_index: u32 = ctx.regs[4].try_into().unwrap();
    let _offset: usize = ctx.regs[5];

    let cur_process = CORES.with_current(|core| {
        let thread = core.thread.take().unwrap();
        // TODO: don't require cloning here
        // TODO: how to make longer periods of access to the current thread sound?
        // (ie. either internal mutability, or can't yield/preempt/check preempt status...)
        let cur_process = thread.process.clone();
        core.thread.set(Some(thread));
        cur_process
    });

    let range_start: usize = match cur_process.unwrap().mem.lock().reserve_memory_range(
        req_start_addr,
        req_size,
        fd_index,
        fill_pages,
    ) {
        Ok(start_addr) => start_addr,
        Err(e) => {
            //For debug
            println!("Error: {:?}", e);
            //TODO: find a better way to tell the user what went wrong
            i64::from(-1) as usize
        }
    };
    ctx.regs[0] = range_start;
    ctx
}

pub unsafe fn sys_map_physical_range(ctx: &mut Context) -> *mut Context {
    let req_virtual_addr: usize = ctx.regs[0];
    let req_physical_addr: usize = ctx.regs[1];

    let cur_process = CORES.with_current(|core| {
        let thread = core.thread.take().unwrap();
        // TODO: don't require cloning here
        // TODO: how to make longer periods of access to the current thread sound?
        // (ie. either internal mutability, or can't yield/preempt/check preempt status...)
        let cur_process = thread.process.clone();
        core.thread.set(Some(thread));
        cur_process
    });

    let retval: usize = match cur_process
        .unwrap()
        .mem
        .lock()
        .map_to_physical_range(req_virtual_addr, req_physical_addr)
    {
        Ok(()) => 0,
        Err(e) => {
            //For debug
            println!("Error: {:?}", e);
            //TODO: find a better way to tell the user what went wrong
            i64::from(-1) as usize
        }
    };
    ctx.regs[0] = retval;
    ctx
}

pub unsafe fn sys_munmap(ctx: &mut Context) -> *mut Context {
    let req_addr: usize = ctx.regs[0];
    //Currently not used
    let _len: usize = ctx.regs[1];

    let cur_process = CORES.with_current(|core| {
        let thread = core.thread.take().unwrap();
        // TODO: don't require cloning here
        // TODO: how to make longer periods of access to the current thread sound?
        // (ie. either internal mutability, or can't yield/preempt/check preempt status...)
        let cur_process = thread.process.clone();
        core.thread.set(Some(thread));
        cur_process
    });

    let retval: usize = match cur_process.unwrap().mem.lock().unmap_memory_range(req_addr) {
        Ok(()) => 0,
        Err(e) => {
            //For debug
            println!("Error: {:?}", e);
            //TODO: find a better way to tell the user what went wrong
            i64::from(-1) as usize
        }
    };
    ctx.regs[0] = retval;
    ctx
}
