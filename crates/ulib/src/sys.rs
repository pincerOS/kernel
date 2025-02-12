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

syscall!(1 => pub fn shutdown());
syscall!(2 => pub fn hello_world());
syscall!(3 => pub fn yield_());
syscall!(4 => pub fn print(buf: *const u8, len: usize));
syscall!(5 => pub fn spawn(pc: usize, sp: usize, flags: usize));
syscall!(6 => pub fn exit());
