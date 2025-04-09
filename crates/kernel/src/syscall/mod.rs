use syscalls::{register_variants, SysCall};

use crate::event::exceptions::register_syscall_handler;

pub mod channel;
pub mod exec;
pub mod file;
pub mod mmap;
pub mod pipe;
pub mod proc;
pub mod sync;

pub unsafe fn register_syscalls() {
    unsafe {
        register_variants!(
            SysCall::SHUTDOWN => proc::sys_shutdown,
            SysCall::YIELD => sync::sys_yield,
            SysCall::SPAWN => proc::sys_spawn,
            SysCall::EXIT => proc::sys_exit,
            SysCall::CHANNEL => channel::sys_channel,
            SysCall::SEND => channel::sys_send,
            SysCall::RECV => channel::sys_recv,
            SysCall::PREAD => file::sys_pread,
            SysCall::PWRITE => file::sys_pwrite,
            SysCall::CLOSE => file::sys_close,
            SysCall::DUP3 => file::sys_dup3,
            SysCall::PIPE => pipe::sys_pipe,
            SysCall::OPENAT => file::sys_openat,
            SysCall::EXECVE_FD => exec::sys_execve_fd,
            SysCall::WAIT => proc::sys_wait
        );

        register_syscall_handler(18, mmap::sys_mmap);
        register_syscall_handler(19, mmap::sys_munmap);
    }
}
