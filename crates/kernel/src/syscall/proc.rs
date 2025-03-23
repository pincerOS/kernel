use alloc::sync::Arc;

use crate::event::async_handler::{run_async_handler, HandlerContext};
use crate::event::context::{deschedule_thread, Context, DescheduleAction, CORES};
use crate::process::fd::{self, FileDescriptor};
use crate::process::ExitStatus;
use crate::sync::once_cell::BlockingOnceCell;
use crate::{event, event::thread, shutdown};

pub unsafe fn sys_shutdown(_ctx: &mut Context) -> *mut Context {
    shutdown();
}

pub unsafe fn sys_exit(ctx: &mut Context) -> *mut Context {
    let thread = CORES.with_current(|core| core.thread.take());
    let mut thread = thread.expect("usermode syscall without active thread");

    let status = ctx.regs[0];

    // TODO: split exit into process exit and thread exit?
    // TODO: ensure processes can't exit without setting this
    let exit_code = &thread.process.as_ref().unwrap().exit_code;
    exit_code.set(crate::process::ExitStatus {
        status: status as u32,
    });

    unsafe { thread.save_context(ctx.into(), false) };
    unsafe { deschedule_thread(DescheduleAction::FreeThread, Some(thread)) }
}

pub unsafe fn sys_spawn(ctx: &mut Context) -> *mut Context {
    let user_entry = ctx.regs[0];
    let user_sp = ctx.regs[1];
    let user_x0 = ctx.regs[2];
    let flags = ctx.regs[3];

    let cur_process = CORES.with_current(|core| {
        let thread = core.thread.take().unwrap();
        // TODO: don't require cloning here
        // TODO: how to make longer periods of access to the current thread sound?
        // (ie. either internal mutability, or can't yield/preempt/check preempt status...)
        let cur_process = thread.process.clone();
        core.thread.set(Some(thread));
        cur_process
    });
    let old_process = cur_process.unwrap();

    let wait_fd;
    let process;

    if flags == 1 {
        // Same process, shared memory
        process = old_process;
        wait_fd = (-1isize) as usize;
    } else {
        process = Arc::new(old_process.fork());
        let descriptor = WaitFd(process.exit_code.clone());
        let fd = old_process
            .file_descriptors
            .lock()
            .insert(Arc::new(descriptor));
        wait_fd = fd;
    }

    println!(
        "Creating new process with page dir {:#010}",
        process.get_ttbr0()
    );
    let mut user_thread = unsafe { thread::Thread::new_user(process, user_sp, user_entry) };
    user_thread.context.as_mut().unwrap().regs[0] = user_x0;
    event::SCHEDULER.add_task(event::Event::ScheduleThread(user_thread));

    ctx.regs[0] = wait_fd;
    ctx
}

/// syscall wait(fd: u32) -> i64
pub unsafe fn sys_wait(ctx: &mut Context) -> *mut Context {
    let fd = ctx.regs[0];

    run_async_handler(ctx, async move |mut context: HandlerContext<'_>| {
        let proc = context.cur_process().unwrap();

        let file = proc.file_descriptors.lock().get(fd).cloned();
        let Some(file) = file else {
            context.regs().regs[0] = i64::from(-1) as usize;
            return context.resume_final();
        };
        let Some(file) = file.as_any().downcast_ref::<WaitFd>() else {
            context.regs().regs[0] = i64::from(-1) as usize;
            return context.resume_final();
        };

        let status = file.0.get().await;

        context.regs().regs[0] = status.status as usize;
        context.resume_final()
    })
}

pub unsafe fn sys_mmap(ctx: &mut Context) -> *mut Context {
    let req_start_addr: usize = ctx.regs[0];
    let req_size: usize = ctx.regs[1];
    //TODO: update this to be flags later
    let fill_pages: bool = ctx.regs[2] == 1;

    let cur_process = CORES.with_current(|core| {
        let thread = core.thread.take().unwrap();
        // TODO: don't require cloning here
        // TODO: how to make longer periods of access to the current thread sound?
        // (ie. either internal mutability, or can't yield/preempt/check preempt status...)
        let cur_process = thread.process.clone();
        core.thread.set(Some(thread));
        cur_process
    });

    let range_start: usize = match cur_process.unwrap().page_table.lock().reserve_memory_range(
        req_start_addr,
        req_size,
        u32::MAX, //TODO: update this for file support
        fill_pages,
    ) {
        Ok(start_addr) => start_addr,
        Err(e) => {
            //For debug
            println!("Error: {}", e);
            //TODO: find a better way to tell the user what went wrong
            usize::MAX
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
    
    let retval: usize = match cur_process.unwrap().page_table.lock().map_to_physical_range(req_virtual_addr, req_physical_addr) {
        Ok(()) => 0,
        Err(e) => { 
            //For debug
            println!("Error: {}", e);
            //TODO: find a better way to tell the user what went wrong
            usize::MAX
        }
    };
    ctx.regs[0] = retval;
    ctx
}

pub unsafe fn sys_munmap(ctx: &mut Context) -> *mut Context {
    let req_addr: usize = ctx.regs[0];

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
        .page_table
        .lock()
        .unmap_memory_range(req_addr)
    {
        Ok(()) => 0,
        Err(e) => {
            //For debug
            println!("Error: {}", e);
            //TODO: find a better way to tell the user what went wrong
            usize::MAX
        }
    };
    ctx.regs[0] = retval;
    ctx
}

struct WaitFd(Arc<BlockingOnceCell<ExitStatus>>);

impl FileDescriptor for WaitFd {
    fn is_same_file(&self, other: &dyn FileDescriptor) -> bool {
        let other = other.as_any().downcast_ref::<Self>();
        other.map(|o| Arc::ptr_eq(&self.0, &o.0)).unwrap_or(true)
    }
    fn kind(&self) -> fd::FileKind {
        fd::FileKind::Other
    }
    fn read<'a>(
        &'a self,
        _offset: u64,
        _buf: &'a mut [u8],
    ) -> fd::SmallFuture<'a, fd::FileDescResult> {
        fd::boxed_future(async move { Err(1).into() })
    }
    fn write<'a>(
        &'a self,
        _offset: u64,
        _buf: &'a [u8],
    ) -> fd::SmallFuture<'a, fd::FileDescResult> {
        fd::boxed_future(async move { Err(1).into() })
    }
    fn size<'a>(&'a self) -> fd::SmallFuture<'a, fd::FileDescResult> {
        fd::boxed_future(async move { Err(1).into() })
    }
    fn mmap_page(&self, _offset: u64) -> fd::SmallFuture<Option<fd::FileDescResult>> {
        fd::boxed_future(async move { None })
    }
    fn as_any(&self) -> &dyn core::any::Any {
        self
    }
}
