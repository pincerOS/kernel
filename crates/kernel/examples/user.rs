#![no_std]
#![no_main]

extern crate alloc;
extern crate kernel;

use alloc::sync::Arc;
use event::thread;
use kernel::*;
use memory::with_user_vmem;
use process::mem::MappingKind;

static INIT_CODE: &[u8] = kernel::util::include_bytes_align!(u32, "../../init/init.bin");

#[no_mangle]
extern "Rust" fn kernel_main(_device_tree: device_tree::DeviceTree) {
    println!("| starting kernel_main");
    crate::event::task::spawn_async(async move {
        main().await;
    });
    crate::event::thread::stop();
}

async fn main() {
    unsafe { syscall::register_syscalls() };

    // Create a user process
    let mut process = crate::process::Process::new();

    let user_region_len = INIT_CODE.len();
    let user_region = process
        .mem
        .lock()
        .mmap(Some(0x20_0000), user_region_len, MappingKind::Anon)
        .unwrap();

    let mem = process.mem.lock();
    let vme = mem.get_vme(user_region).unwrap();
    mem.populate_range(vme, vme.start, vme.size).await.unwrap();
    drop(mem);

    let user_region = user_region as *mut u8;

    let ttbr0 = process.get_ttbr0();
    let callback = || {
        println!("User ptr: {:p}", user_region);
        println!(
            "Physical addr: {:?}",
            memory::physical_addr(user_region.addr())
        );
        let access = crate::arch::memory::at_s1e0r(user_region.addr());
        println!(
            "user access: {:?}",
            access.map(|b| b.bits()).map_err(|e| e.bits())
        );

        let start = sync::get_time();
        unsafe {
            core::ptr::copy_nonoverlapping(
                INIT_CODE.as_ptr(),
                user_region.cast::<u8>(),
                INIT_CODE.len(),
            );
        }
        let end = sync::get_time();

        println!("Done copying user data, took {:4}µs", end - start);
    };
    unsafe { with_user_vmem(ttbr0, callback) };

    static ARCHIVE: &[u8] = initfs::include_bytes_align!(u32, "../../init/fs.arc");
    let fs = fs::initfs::InitFs::new(&ARCHIVE).unwrap();
    let root = fs.root();

    process.root = Some(root.clone());
    process.current_dir.lock().replace(root.clone());

    {
        let mut fds = process.file_descriptors.lock();
        let uart_fd = Arc::new(process::fd::UartFd(device::uart::UART.get())) as Arc<_>;
        let _ = fds.set(0, Arc::clone(&uart_fd));
        let _ = fds.set(1, Arc::clone(&uart_fd));
        let _ = fds.set(2, uart_fd);
        let _ = fds.set(3, root);
    }

    let stack_size = 0x20_0000;
    let stack_start = 0x100_0000;
    process
        .mem
        .lock()
        .mmap(
            Some(stack_start - stack_size),
            stack_size,
            MappingKind::Anon,
        )
        .unwrap();

    let user_sp = stack_start;
    let user_entry = 0x20_0000;

    let user_thread = unsafe { thread::Thread::new_user(Arc::new(process), user_sp, user_entry) };

    event::SCHEDULER.add_task(event::Event::schedule_thread(user_thread));
}
