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

def_enum!(
    pub SysCall => u8 {
        SHUTDOWN => 1,
        YIELD => 3,
        SPAWN => 5,
        EXIT => 6,
        CHANNEL => 7,
        SEND => 8,
        RECV => 9,
        PREAD => 10,
        PWRITE => 11,
        CLOSE => 12,
        DUP3 => 13,
        PIPE => 14,
        OPENAT => 15,
        EXECVE_FD => 16,
        WAIT => 17,
    }
);

#[cfg(feature = "runtime")]
define_variants!(SysCall::VARIANTS,
    SysCall::SHUTDOWN => pub fn sys_shutdown(),
    SysCall::YIELD => pub fn sys_yield(),
    SysCall::SPAWN => pub fn sys_spawn(pc: usize, sp: usize, x0: usize, flags: usize) -> isize,
    SysCall::EXIT => pub fn sys_exit(status: usize),
    SysCall::CHANNEL => pub fn sys_channel() -> Channels,
    SysCall::SEND => pub fn sys_send(desc: usize, msg: *const Message, buf: *const u8, buf_len: usize, flags: usize) -> isize,
    SysCall::RECV => pub fn sys_recv(desc: usize, msg: *mut Message, buf: *mut u8, buf_cap: usize, flags: usize) -> isize,
    SysCall::PREAD => pub fn sys_pread(fd: usize, buf: *mut u8, buf_len: usize, offset: u64) -> isize,
    SysCall::PWRITE => pub fn sys_pwrite(fd: usize, buf: *const u8, buf_len: usize, offset: u64) -> isize,
    SysCall::CLOSE => pub fn sys_close(fd: usize) -> isize,
    SysCall::DUP3 => pub fn sys_dup3(old_fd: usize, new_fd: usize, flags: usize) -> isize,
    SysCall::PIPE => pub fn sys_pipe(flags: usize) -> PipeValues,
    SysCall::OPENAT => pub fn sys_openat(dir_fd: usize, path_len: usize, path_ptr: *const u8, flags: usize, mode: usize) -> isize,
    SysCall::EXECVE_FD => pub fn sys_execve_fd(fd: usize, flags: usize, argc: usize, argv: *const ArgStr, envc: usize, envp: *const ArgStr) -> isize,
    SysCall::WAIT => pub fn sys_wait(fd: usize) -> isize
);
