use core::mem::MaybeUninit;
pub use syscalls::sys::*;

fn int_to_error(res: isize) -> Result<usize, usize> {
    match res {
        0.. => Ok(res.unsigned_abs()),
        ..0 => Err(res.unsigned_abs()),
    }
}

syscall!(18 => pub fn sys_mmap(addr: usize, size: usize, prot_flags: usize, flags: usize, fd: usize, offset: usize) -> isize);
syscall!(19 => pub fn sys_munmap(addr: usize) -> isize);

/* * * * * * * * * * * * * * * * * * * */
/* Syscall wrappers                    */
/* * * * * * * * * * * * * * * * * * * */

const FLAG_NO_BLOCK: usize = 1 << 0;

pub fn channel() -> (ChannelDesc, ChannelDesc) {
    let res = unsafe { sys_channel() };
    (ChannelDesc(res.0 as u32), ChannelDesc(res.1 as u32))
}

pub fn send(desc: ChannelDesc, msg: &Message, buf: &[u8], flags: usize) -> isize {
    unsafe { sys_send(desc.0 as usize, msg, buf.as_ptr(), buf.len(), flags) }
}
pub fn send_block(desc: ChannelDesc, msg: &Message, buf: &[u8]) -> isize {
    send(desc, msg, buf, 0)
}
pub fn send_nonblock(desc: ChannelDesc, msg: &Message, buf: &[u8]) -> isize {
    send(desc, msg, buf, FLAG_NO_BLOCK)
}

pub fn recv(desc: ChannelDesc, buf: &mut [u8], flags: usize) -> Result<(usize, Message), isize> {
    let mut msg = MaybeUninit::uninit();
    let res = unsafe {
        sys_recv(
            desc.0 as usize,
            msg.as_mut_ptr(),
            buf.as_mut_ptr(),
            buf.len(),
            flags,
        )
    };
    if res >= 0 {
        Ok((res as usize, unsafe { msg.assume_init() }))
    } else {
        Err(res)
    }
}
pub fn recv_block(desc: ChannelDesc, buf: &mut [u8]) -> Result<(usize, Message), isize> {
    recv(desc, buf, 0)
}
pub fn recv_nonblock(desc: ChannelDesc, buf: &mut [u8]) -> Result<(usize, Message), isize> {
    recv(desc, buf, FLAG_NO_BLOCK)
}

pub fn shutdown() -> ! {
    unsafe { sys_shutdown() };
    unsafe { core::arch::asm!("udf #2", options(noreturn)) }
}

pub fn yield_() {
    unsafe { sys_yield() }
}

pub unsafe fn spawn(pc: usize, sp: usize, x0: usize, flags: usize) -> Result<FileDesc, usize> {
    let res = unsafe { sys_spawn(pc, sp, x0, flags) };
    int_to_error(res).map(|fd| fd as FileDesc)
}

pub fn exit(status: usize) -> ! {
    unsafe { sys_exit(status) };
    unsafe { core::arch::asm!("udf #2", options(noreturn)) }
}

pub fn pread(fd: FileDesc, buf: &mut [u8], offset: u64) -> Result<usize, usize> {
    let res = unsafe { sys_pread(fd as usize, buf.as_mut_ptr(), buf.len(), offset) };
    int_to_error(res)
}

pub fn pwrite(fd: FileDesc, buf: &[u8], offset: u64) -> Result<usize, usize> {
    let res = unsafe { sys_pwrite(fd as usize, buf.as_ptr(), buf.len(), offset) };
    int_to_error(res)
}

pub fn pwrite_all(fd: FileDesc, buf: &[u8], offset: u64) -> Result<usize, usize> {
    let mut remaining = buf;
    let mut written = 0;
    while !remaining.is_empty() {
        let res = pwrite(fd, remaining, offset + written as u64)?;
        if res == 0 {
            return Err(1); // EOF
        }
        written += res;
        remaining = &remaining[res..];
    }
    Ok(written)
}

pub fn close(fd: FileDesc) -> Result<(), usize> {
    let res = unsafe { sys_close(fd as usize) };
    int_to_error(res).map(|_| ())
}

pub fn dup3(old_fd: FileDesc, new_fd: FileDesc, flags: usize) -> Result<FileDesc, usize> {
    let res = unsafe { sys_dup3(old_fd as usize, new_fd as usize, flags) };
    int_to_error(res).map(|fd| fd as FileDesc)
}

pub fn pipe(flags: usize) -> Result<(FileDesc, FileDesc), usize> {
    let res = unsafe { sys_pipe(flags) };
    let [rx, tx] = res.0;
    if rx < 0 {
        Err(rx.unsigned_abs())
    } else {
        Ok((rx.unsigned_abs() as FileDesc, tx.unsigned_abs() as FileDesc))
    }
}

pub fn openat(dir_fd: FileDesc, path: &[u8], flags: usize, mode: usize) -> Result<FileDesc, usize> {
    let res = unsafe { sys_openat(dir_fd as usize, path.len(), path.as_ptr(), flags, mode) };
    int_to_error(res).map(|fd| fd as FileDesc)
}

pub unsafe fn execve_fd(
    fd: FileDesc,
    flags: usize,
    args: &[ArgStr],
    env: &[ArgStr],
) -> Result<(), usize> {
    let res = unsafe {
        sys_execve_fd(
            fd as usize,
            flags,
            args.len(),
            args.as_ptr(),
            env.len(),
            env.as_ptr(),
        )
    };
    int_to_error(res).map(|_| ())
}

pub fn wait(fd: FileDesc) -> Result<usize, usize> {
    let res = unsafe { sys_wait(fd as usize) };
    int_to_error(res)
}

pub unsafe fn mmap(
    addr: usize,
    size: usize,
    prot_flags: u32,
    flags: u32,
    file_descriptor: FileDesc,
    offset: usize,
) -> Result<*mut (), usize> {
    let res = unsafe {
        sys_mmap(
            addr,
            size,
            prot_flags as usize,
            flags as usize,
            file_descriptor as usize,
            offset,
        )
    };
    int_to_error(res).map(|a| a as *mut ())
}

pub unsafe fn munmap(addr: *mut ()) -> Result<usize, usize> {
    let res = unsafe { sys_munmap(addr.addr()) };
    int_to_error(res)
}
