use crate::event::exceptions::register_syscall_handler;

pub mod channel;
pub mod exec;
pub mod fb_hack;
pub mod file;
pub mod mmap;
pub mod pipe;
pub mod proc;
pub mod semaphore;
pub mod sync;
pub mod time;

pub unsafe fn register_syscalls() {
    unsafe {
        register_syscall_handler(1, proc::sys_shutdown);
        register_syscall_handler(3, sync::sys_yield);
        register_syscall_handler(5, proc::sys_spawn);
        register_syscall_handler(6, proc::sys_exit);
        register_syscall_handler(7, channel::sys_channel);
        register_syscall_handler(8, channel::sys_send);
        register_syscall_handler(9, channel::sys_recv);

        register_syscall_handler(10, file::sys_pread);
        register_syscall_handler(11, file::sys_pwrite);
        register_syscall_handler(12, file::sys_close);
        register_syscall_handler(13, file::sys_dup3);
        register_syscall_handler(14, pipe::sys_pipe);

        register_syscall_handler(15, file::sys_openat);
        register_syscall_handler(16, exec::sys_execve_fd);
        register_syscall_handler(17, proc::sys_wait);
        register_syscall_handler(18, mmap::sys_mmap);
        register_syscall_handler(19, mmap::sys_munmap);

        register_syscall_handler(21, time::sys_get_time_ms);
        register_syscall_handler(22, time::sys_sleep_ms);

        register_syscall_handler(23, fb_hack::sys_acquire_fb);
        register_syscall_handler(24, fb_hack::sys_memfd_create);
        register_syscall_handler(25, fb_hack::sys_poll_key_event);

        register_syscall_handler(26, semaphore::sys_sem_create);
        register_syscall_handler(27, semaphore::sys_sem_up);
        register_syscall_handler(28, semaphore::sys_sem_down);

        register_syscall_handler(30, fb_hack::sys_poll_mouse_event);
    }
}
