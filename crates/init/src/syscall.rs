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
syscall!(5 => pub fn spawn(pc: usize, sp: usize, flags: usize));
syscall!(6 => pub fn exit());

syscall!(7 => fn _channel() -> Channels);
pub unsafe fn channel() -> (ChannelDesc, ChannelDesc) {
    let res = unsafe { _channel() };
    (ChannelDesc(res.0 as u32), ChannelDesc(res.1 as u32))
}

syscall!(8 => pub fn send(desc: ChannelDesc, msg: &Message, buf: *const u8, buf_len: usize) -> isize);
syscall!(9 => pub fn recv(desc: ChannelDesc, msg: &mut Message, buf: *mut u8, buf_cap: usize) -> isize);
