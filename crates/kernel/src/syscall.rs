use crate::event::exceptions::register_syscall_handler;

pub mod channel;
pub mod proc;
pub mod sync;

pub unsafe fn register_syscalls() {
    channel::OBJECTS.lock().push(None);

    unsafe {
        register_syscall_handler(1, proc::sys_shutdown);
        register_syscall_handler(3, sync::sys_yield);
        register_syscall_handler(5, proc::sys_spawn);
        register_syscall_handler(6, proc::sys_exit);
        register_syscall_handler(7, channel::sys_channel);
        register_syscall_handler(8, channel::sys_send);
        register_syscall_handler(9, channel::sys_recv);
        register_syscall_handler(10, proc::sys_mmap);
        register_syscall_handler(11, proc::sys_munmap);
    }
}
