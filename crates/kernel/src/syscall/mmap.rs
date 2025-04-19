use crate::event::async_handler::{run_async_handler, HandlerContext};
use crate::event::context::Context;
use crate::process::mem::MappingKind;

bitflags::bitflags! {
    struct ProtFlags: u32 {
    }
    struct MmapFlags: u32 {
        const MAP_FIXED = 1 << 0;
        const MAP_ANONYMOUS = 1 << 1; //if not set this indicates file
        const MAP_SHARED = 1 << 2; //if not set indicates private mapping
    }
}

// syscall sys_mmap(addr: *mut (), size: usize, prot: ProtFlags, flags: Flags, fd: u32, offset: u64)
pub unsafe fn sys_mmap(ctx: &mut Context) -> *mut Context {
    let request_addr = ctx.regs[0];
    let request_size = ctx.regs[1];

    let prot_flags = ctx.regs[2];
    let Some(_prot_flags) = u32::try_from(prot_flags)
        .ok()
        .and_then(ProtFlags::from_bits)
    else {
        ctx.regs[0] = i64::from(-1) as usize;
        return ctx;
    };

    let flags = ctx.regs[3];
    let Some(flags) = u32::try_from(flags).ok().and_then(MmapFlags::from_bits) else {
        ctx.regs[0] = i64::from(-1) as usize;
        return ctx;
    };

    let fd = ctx.regs[4];
    let offset = ctx.regs[5];

    run_async_handler(ctx, async move |mut context: HandlerContext<'_>| {
        let proc = context.cur_process().unwrap();

        let _is_shared: bool = flags.contains(MmapFlags::MAP_SHARED);

        let kind = if flags.contains(MmapFlags::MAP_ANONYMOUS) {
            MappingKind::Anon
        } else {
            let file = proc.file_descriptors.lock().get(fd).cloned();
            let Some(file) = file else {
                context.regs().regs[0] = i64::from(-1) as usize;
                return context.resume_final();
            };
            MappingKind::File(file)
        };

        let res;
        if flags.contains(MmapFlags::MAP_FIXED) {
            res = proc
                .mem
                .lock()
                .mmap(Some(request_addr), request_size, kind, offset);
        } else {
            // TODO: try to respect hint?
            res = proc.mem.lock().mmap(None, request_size, kind, offset);
        }

        match res {
            Ok(addr) => {
                context.regs().regs[0] = addr;
                context.resume_final()
            }
            Err(e) => {
                let code = match e {
                    _ => -1,
                };
                context.regs().regs[0] = code as usize;
                context.resume_final()
            }
        }
    })
}

// TODO: partial unmap?
// syscall sys_munmap(addr: *mut ())
pub unsafe fn sys_munmap(ctx: &mut Context) -> *mut Context {
    let addr = ctx.regs[0];

    run_async_handler(ctx, async move |mut context: HandlerContext<'_>| {
        let proc = context.cur_process().unwrap();

        let res = proc.mem.lock().unmap(addr);

        match res {
            Ok(()) => {
                context.regs().regs[0] = 0;
                context.resume_final()
            }
            Err(e) => {
                let code = match e {
                    _ => -1,
                };
                context.regs().regs[0] = code as usize;
                context.resume_final()
            }
        }
    })
}
