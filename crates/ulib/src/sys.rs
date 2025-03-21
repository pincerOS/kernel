use core::mem::MaybeUninit;

macro_rules! syscall {
    ($num:literal => $vis:vis fn $ident:ident ( $($arg:ident : $ty:ty),* $(,)? ) $( -> $ret:ty )?) => {
        core::arch::global_asm!(
            ".global {name}; {name}: svc #{num}; ret",
            name = sym $ident,
            num = const $num,
        );
        unsafe extern "C" {
            $vis fn $ident( $($arg: $ty,)* ) $(-> $ret)?;
        }
    };
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct ChannelDesc(pub u32);

#[repr(C)]
#[derive(Debug)]
pub struct Message {
    pub tag: u64,
    pub objects: [u32; 4],
}

#[repr(C)]
struct Channels(usize, usize);

syscall!(1 => pub fn shutdown());
syscall!(3 => pub fn yield_());
syscall!(5 => pub fn spawn(pc: usize, sp: usize, x0: usize, flags: usize) -> usize);
syscall!(6 => pub fn exit(status: usize));

syscall!(7 => fn _channel() -> Channels);
pub fn channel() -> (ChannelDesc, ChannelDesc) {
    let res = unsafe { _channel() };
    (ChannelDesc(res.0 as u32), ChannelDesc(res.1 as u32))
}

const FLAG_NO_BLOCK: usize = 1 << 0;

syscall!(8 => pub fn _send(desc: ChannelDesc, msg: *const Message, buf: *const u8, buf_len: usize, flags: usize) -> isize);
syscall!(9 => pub fn _recv(desc: ChannelDesc, msg: *mut Message, buf: *mut u8, buf_cap: usize, flags: usize) -> isize);

pub fn send(desc: ChannelDesc, msg: &Message, buf: &[u8]) -> isize {
    unsafe { _send(desc, msg, buf.as_ptr(), buf.len(), FLAG_NO_BLOCK) }
}
pub fn send_block(desc: ChannelDesc, msg: &Message, buf: &[u8]) -> isize {
    unsafe { _send(desc, msg, buf.as_ptr(), buf.len(), 0) }
}
pub fn recv(desc: ChannelDesc, buf: &mut [u8]) -> Result<(isize, Message), isize> {
    let mut msg = MaybeUninit::uninit();
    let res = unsafe {
        _recv(
            desc,
            msg.as_mut_ptr(),
            buf.as_mut_ptr(),
            buf.len(),
            FLAG_NO_BLOCK,
        )
    };
    if res >= 0 {
        Ok((res, unsafe { msg.assume_init() }))
    } else {
        Err(res)
    }
}
pub fn recv_block(desc: ChannelDesc, buf: &mut [u8]) -> Result<(usize, Message), isize> {
    let mut msg = MaybeUninit::uninit();
    let res = unsafe { _recv(desc, msg.as_mut_ptr(), buf.as_mut_ptr(), buf.len(), 0) };
    if res >= 0 {
        Ok((res as usize, unsafe { msg.assume_init() }))
    } else {
        Err(res)
    }
}

syscall!(10 => pub fn pread(fd: usize, buf: *mut u8, buf_len: usize, offset: u64) -> isize);
syscall!(11 => pub fn pwrite(fd: usize, buf: *const u8, buf_len: usize, offset: u64) -> isize);
syscall!(12 => pub fn close(fd: usize) -> isize);
syscall!(13 => pub fn dup3(old_fd: usize, new_fd: usize, flags: usize) -> isize);
syscall!(14 => pub fn pipe(flags: usize) -> PipeValues);

#[repr(C)]
pub struct PipeValues([isize; 2]);

pub unsafe fn pwrite_all(fd: usize, buf: &[u8], offset: u64) -> isize {
    let mut remaining = buf;
    let mut written = 0;
    while !remaining.is_empty() {
        match unsafe { pwrite(fd, remaining.as_ptr(), remaining.len(), offset + written) } {
            i @ (..=-1) => return i,
            i @ (1..) => {
                written += i as u64;
                remaining = &remaining[i as usize..];
            }
            0 => return -1, // EOF
        }
    }
    written as isize
}

syscall!(15 => pub fn openat(
    dir_fd: usize,
    path_len: usize,
    path_ptr: *const u8,
    flags: usize,
    mode: usize,
) -> isize);

#[repr(C)]
pub struct ArgStr {
    len: usize,
    ptr: *const u8,
}

syscall!(16 => pub fn execve_fd(
    fd: usize,
    flags: usize,
    argc: usize,
    argv: *const ArgStr,
    envc: usize,
    envp: *const ArgStr,
) -> isize);

syscall!(17 => pub fn wait(fd: usize) -> isize);
