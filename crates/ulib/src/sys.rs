use core::mem::offset_of;
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

fn int_to_error(res: isize) -> Result<usize, usize> {
    match res {
        0.. => Ok(res.unsigned_abs()),
        ..0 => Err(res.unsigned_abs()),
    }
}

pub type FileDesc = u32;

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
pub struct Channels(pub usize, pub usize);

#[repr(C)]
pub struct PipeValues(pub [isize; 2]);

#[repr(C)]
pub struct ArgStr {
    pub len: usize,
    pub ptr: *const u8,
}

syscall!(1 => pub fn sys_shutdown());
syscall!(3 => pub fn sys_yield());
syscall!(5 => pub fn sys_spawn(pc: usize, sp: usize, x0: usize, flags: usize) -> isize);
syscall!(6 => pub fn sys_exit(status: usize));

syscall!(7 => pub fn sys_channel() -> Channels);
syscall!(8 => pub fn sys_send(desc: usize, msg: *const Message, buf: *const u8, buf_len: usize, flags: usize) -> isize);
syscall!(9 => pub fn sys_recv(desc: usize, msg: *mut Message, buf: *mut u8, buf_cap: usize, flags: usize) -> isize);

syscall!(10 => pub fn sys_pread(fd: usize, buf: *mut u8, buf_len: usize, offset: u64) -> isize);
syscall!(11 => pub fn sys_pwrite(fd: usize, buf: *const u8, buf_len: usize, offset: u64) -> isize);

syscall!(12 => pub fn sys_close(fd: usize) -> isize);
syscall!(13 => pub fn sys_dup3(old_fd: usize, new_fd: usize, flags: usize) -> isize);
syscall!(14 => pub fn sys_pipe(flags: usize) -> PipeValues);

syscall!(15 => pub fn sys_openat(
    dir_fd: usize,
    path_len: usize,
    path_ptr: *const u8,
    flags: usize,
    mode: usize,
) -> isize);

syscall!(16 => pub fn sys_execve_fd(
    fd: usize,
    flags: usize,
    argc: usize,
    argv: *const ArgStr,
    envc: usize,
    envp: *const ArgStr,
) -> isize);

syscall!(17 => pub fn sys_wait(fd: usize) -> isize);

syscall!(18 => pub fn sys_mmap(addr: usize, size: usize, prot_flags: usize, flags: usize, fd: usize, offset: usize) -> isize);
syscall!(19 => pub fn sys_munmap(addr: usize) -> isize);

syscall!(21 => pub fn sys_get_time_ms() -> usize);
syscall!(22 => pub fn sys_sleep_ms(time: usize));

// syscall!(23 => pub fn sys_acquire_fb(width: usize, height: usize) -> (usize, usize, usize, usize, usize));
syscall!(24 => pub fn sys_memfd_create() -> isize);

#[repr(C)]
pub struct RawFB {
    pub fd: usize,
    pub size: usize,
    pub pitch: usize,
    pub width: usize,
    pub height: usize,
}

core::arch::global_asm!(
    ".global {name}; {name}:",
    "mov x7, x2",
    "svc #{num}",
    "str x0, [x7, {fd_offset}]",
    "str x1, [x7, {size_offset}]",
    "str x2, [x7, {pitch_offset}]",
    "str x3, [x7, {width_offset}]",
    "str x4, [x7, {height_offset}]",
    "ret",
    name = sym sys_acquire_fb,
    num = const 23,
    fd_offset = const offset_of!(RawFB, fd),
    size_offset = const offset_of!(RawFB, size),
    pitch_offset = const offset_of!(RawFB, pitch),
    width_offset = const offset_of!(RawFB, width),
    height_offset = const offset_of!(RawFB, height),
);
unsafe extern "C" {
    pub fn sys_acquire_fb(width: usize, height: usize, res: *mut RawFB) -> isize;
}

syscall!(25 => pub fn sys_poll_key_event() -> isize);

syscall!(26 => pub fn sys_sem_create(value: usize) -> isize);
syscall!(27 => pub fn sys_sem_up(fd: usize) -> isize);
syscall!(28 => pub fn sys_sem_down(fd: usize) -> isize);

core::arch::global_asm!(
    ".global {name}; {name}:",
    "mov x0, lr", //Read link register value into x0
    "svc #{num}",
    "ret",
    name = sym sys_fork_helper,
    num = const 5,
);
unsafe extern "C" {
    fn sys_fork_helper(pc: usize, sp: usize, x0: usize, flags: usize) -> isize;
}

//Should this save the frame pointer too?
core::arch::global_asm!(
    ".global {name}; {name}:",
    /*
    "push {{x9}}",
    "push {{x10}}",
    "push {{x11}}",
    "push {{x12}}",
    "push {{x13}}",
    "push {{x14}}",
    "push {{x15}}",
    "push {{x19}}",
    "push {{x20}}",
    "push {{x21}}",
    "push {{x22}}",
    "push {{x23}}",
    "push {{x24}}",
    "push {{x25}}",
    "push {{x26}}",
    "push {{x27}}",
    "push {{x28}}",
    */
    "push {{x9, x10, x11, x12, x13, x14, x15, x19, x20, x21, x22, x23, x24, x25, x26, x27, x28}}",
    "mov x1, sp",
    "mov x2, 0", //child should get 0
    "mov x3, 0",
    "bl sys_fork_helper",
    "pop {{x28, x27, x26, x25, x24, x23, x22, x21, x20, x19, x15, x14, x13, x12, x11, x10, x9}}",
    /*
    "pop {{x28}}",
    "pop {{x27}}",
    "pop {{x26}}",
    "pop {{x25}}",
    "pop {{x24}}",
    "pop {{x23}}",
    "pop {{x22}}",
    "pop {{x21}}",
    "pop {{x20}}",
    "pop {{x19}}",
    "pop {{x15}}",
    "pop {{x14}}",
    "pop {{x13}}",
    "pop {{x12}}",
    "pop {{x11}}",
    "pop {{x10}}",
    "pop {{x9}}",
    */
    "ret",
    name = sym sys_fork,
);
unsafe extern "C" {
    pub fn sys_fork() -> isize;
}


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

pub fn sem_create(value: usize) -> Result<FileDesc, usize> {
    let res = unsafe { sys_sem_create(value) };
    int_to_error(res).map(|f| f as FileDesc)
}
pub fn sem_up(fd: FileDesc) -> Result<(), usize> {
    let res = unsafe { sys_sem_up(fd as usize) };
    int_to_error(res).map(|_| ())
}
pub fn sem_down(fd: FileDesc) -> Result<(), usize> {
    let res = unsafe { sys_sem_down(fd as usize) };
    int_to_error(res).map(|_| ())
}

pub struct SpawnArgs {
    pub fd: FileDesc,
    pub stdin: Option<FileDesc>,
    pub stdout: Option<FileDesc>,
}

pub fn spawn_elf(args: &SpawnArgs) -> Result<FileDesc, usize> {
    let current_stack = current_sp();
    let target_pc = exec_child as usize;
    let arg = args as *const SpawnArgs;

    let wait_fd = unsafe { spawn(target_pc, current_stack, arg as usize, 0) };
    wait_fd
}

fn current_sp() -> usize {
    let sp: usize;
    unsafe { core::arch::asm!("mov {0}, sp", out(reg) sp) };
    sp
}

extern "C" fn exec_child(spawn_args: *const SpawnArgs) -> ! {
    let spawn_args = unsafe { &*spawn_args };

    if let Some(fd) = spawn_args.stdin {
        dup3(fd, 0, 0).unwrap();
    }
    if let Some(fd) = spawn_args.stdout {
        dup3(fd, 1, 0).unwrap();
    }

    let flags = 0;
    let args = &[];
    let env = &[];
    let _res = unsafe { execve_fd(spawn_args.fd, flags, args, env) };
    // TODO: notify parent of spawn failure
    exit(1);
}
