use core::mem::MaybeUninit;

macro_rules! syscall {
    ($num:literal => $vis:vis fn $ident:ident ( $($arg:ident : $ty:ty),* $(,)? ) $( -> $ret:ty )?) => {
        core::arch::global_asm!(
            ".global {name}; {name}: svc #{num}; ret",
            name = sym $ident,
            num = const $num,
        );
        extern "C" {
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
syscall!(5 => pub fn spawn(pc: usize, sp: usize, x0: usize, flags: usize));
syscall!(6 => pub fn exit());

syscall!(7 => fn _channel() -> Channels);
pub fn channel() -> (ChannelDesc, ChannelDesc) {
    let res = unsafe { _channel() };
    (ChannelDesc(res.0 as u32), ChannelDesc(res.1 as u32))
}

const FLAG_NO_BLOCK: usize = 1 << 0;

syscall!(8 => pub fn _send(desc: ChannelDesc, msg: *const Message, buf: *const u8, buf_len: usize, flags: usize) -> isize);
syscall!(9 => pub fn _recv(desc: ChannelDesc, msg: *mut Message, buf: *mut u8, buf_cap: usize, flags: usize) -> isize);

syscall!(10 => pub fn mmap(req_addr: usize, size: usize, fill_pages: bool) -> usize);
syscall!(11 => pub fn munmap(req_addr: usize) -> usize);

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
