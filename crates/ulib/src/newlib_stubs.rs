#![allow(nonstandard_style, unused_variables)]

use core::ffi::{c_char, c_int};
use core::sync::atomic::{AtomicUsize, Ordering};

use crate::sys;

const ENOMEM: i32 = 12;
const EAGAIN: i32 = 11;
const EINVAL: i32 = 22;
const EMLINK: i32 = 31;
const ENOENT: i32 = 2;
const ECHILD: i32 = 10;

extern "C" {
    static mut errno: c_int;

}

#[repr(transparent)]
pub struct UnsafeStatic<T>(T);
unsafe impl<T> Sync for UnsafeStatic<T> {}

type c_size_t = usize;
type c_off_t = usize;
type clock_t = usize;
type c_mode_t = usize;
struct stat;
struct tms;

// #[unsafe(no_mangle)]
// static mut __env: UnsafeStatic<[*const c_char; 1]> = UnsafeStatic([core::ptr::null()]);
// #[unsafe(no_mangle)]
// static mut environ: UnsafeStatic<*const *const c_char> = UnsafeStatic((&raw const __env).cast());

#[unsafe(no_mangle)]
unsafe extern "C" fn _exit(status: c_int) -> ! {
    crate::println!("exit({status})");
    sys::exit(status as usize)
}
#[unsafe(no_mangle)]
unsafe extern "C" fn _close(file: c_int) -> c_int {
    sys::sys_close(file as usize) as c_int
}
#[unsafe(no_mangle)]
unsafe extern "C" fn _execve(
    name: *const c_char,
    argv: *const *const c_char,
    env: *const *const c_char,
) -> c_int {
    errno = ENOMEM;
    return -1;
}
#[unsafe(no_mangle)]
unsafe extern "C" fn _fork() -> c_int {
    errno = EAGAIN;
    return -1;
}
#[unsafe(no_mangle)]
unsafe extern "C" fn _getpid() -> c_int {
    return 1;
}
#[unsafe(no_mangle)]
unsafe extern "C" fn _isatty(file: c_int) -> c_int {
    return 1;
}
#[unsafe(no_mangle)]
unsafe extern "C" fn _kill(pid: c_int, sig: c_int) -> c_int {
    errno = EINVAL;
    return -1;
}
#[unsafe(no_mangle)]
unsafe extern "C" fn _link(old: *const c_char, new: *const c_char) -> c_int {
    errno = EMLINK;
    return -1;
}
#[unsafe(no_mangle)]
unsafe extern "C" fn _open(name: *const c_char, flags: c_int, mode: c_int) -> c_int {
    let str = unsafe { core::ffi::CStr::from_ptr(name) };
    let str = str.to_bytes();
    crate::println!(
        "open(name={:?}, flags={}, mode={})",
        core::str::from_utf8(str),
        flags,
        mode
    );
    let opened = sys::openat(3, str, 0, 0);
    opened.map(|o| o as c_int).unwrap_or_else(|e| -(e as c_int))
}

static OFFSETS: [crate::spinlock::SpinLock<u64>; 4096] =
    [const { crate::spinlock::SpinLock::new(0) }; 4096];

#[unsafe(no_mangle)]
unsafe extern "C" fn _read(file: c_int, ptr: *mut (), len: c_size_t) -> c_int {
    let mut offset = OFFSETS[file as usize].lock();
    let res = unsafe { sys::sys_pread(file as usize, ptr.cast(), len, *offset) };
    if res > 0 {
        *offset = *offset + res as u64;
    }
    res as c_int
}
#[unsafe(no_mangle)]
unsafe extern "C" fn _lseek(file: c_int, ptr: c_off_t, dir: c_int) -> c_off_t {
    match dir {
        0 => {
            *OFFSETS[file as usize].lock() = ptr as u64;
            return 0;
        }
        1 => {
            let mut offset = OFFSETS[file as usize].lock();
            *offset = *offset + ptr as u64;
            return 0;
        }
        2 => {
            // let end =
            // let mut offset = OFFSETS[file as usize].lock();

            // unimplemented!("seek from end")
            return usize::MAX; // TODO
        }
        _ => unimplemented!("seek mode {dir}"),
    }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn _fstat(file: c_int, st: *mut stat) -> c_int {
    // st.st_mode = S_IFCHR;
    return 0;
}
#[unsafe(no_mangle)]
unsafe extern "C" fn _stat(file: *const c_char, st: *mut stat) -> c_int {
    // st.st_mode = S_IFCHR;
    return 0;
}
#[unsafe(no_mangle)]
unsafe extern "C" fn _times(buf: *mut tms) -> clock_t {
    return usize::MAX;
}

#[unsafe(no_mangle)]
unsafe extern "C" fn _unlink(name: *const c_char) -> c_int {
    errno = ENOENT;
    return -1;
}

#[unsafe(no_mangle)]
unsafe extern "C" fn _wait(status: *mut c_int) -> c_int {
    errno = ECHILD;
    return -1;
}

#[unsafe(no_mangle)]
unsafe extern "C" fn _write(file: c_int, ptr: *const (), len: usize) -> c_int {
    sys::sys_pwrite(file as usize, ptr.cast(), len, 0) as c_int
}

#[unsafe(no_mangle)]
unsafe extern "C" fn mkdir(_path: *const c_char, __mode: c_mode_t) -> c_int {
    return -1;
}

static mut HEAP: [u8; 8 << 20] = [0; 8 << 20];
static HEAP_END: AtomicUsize = AtomicUsize::new(0);

#[unsafe(no_mangle)]
unsafe extern "C" fn _sbrk(incr: usize) -> *mut () {
    if HEAP_END.load(Ordering::SeqCst) == 0 {
        let _ = HEAP_END.compare_exchange(
            0,
            &raw mut HEAP as usize,
            Ordering::SeqCst,
            Ordering::SeqCst,
        );
    }

    let max = (&raw const HEAP).add(1).cast::<u8>() as usize;

    let ptr = HEAP_END.fetch_add(incr, Ordering::SeqCst);
    let end = ptr + incr;
    if end > max {
        // TODO: less hacky approach
        HEAP_END.fetch_sub(incr, Ordering::SeqCst);
        crate::println!("sbrk({incr}) failed");
        core::ptr::without_provenance_mut(-1isize as usize)
    } else {
        crate::println!("sbrk({incr}) -> {ptr:#010x}");
        let base = ptr - (&raw const HEAP) as usize;
        for i in base..base + incr {
            assert_eq!(HEAP[i], 0);
        }
        ptr as *mut ()
    }

    // // extern char _end;		/* Defined by the linker */
    // static char *heap_end;
    // char *prev_heap_end;

    // if (heap_end == 0) {
    //     // heap_end = &_end;
    //     heap_end = &HEAP[0];
    // }
    // prev_heap_end = heap_end;
    // // if (heap_end + incr > stack_ptr) {
    // //     write (1, "Heap and stack collision\n", 25);
    // //     abort ();
    // // }

    // heap_end += incr;
    // return (void*) prev_heap_end;
}

#[cfg(feature = "newlib-stub")]
extern "C" {
    fn main(argc: c_int, argv: *const *const c_char, envp: *const *const c_char) -> c_int;
}

#[unsafe(no_mangle)]
#[cfg(feature = "newlib-stub")]
unsafe extern "C" fn _start() -> ! {
    let ret = main(0, core::ptr::null(), core::ptr::null());
    _exit(ret);
}

#[unsafe(no_mangle)]
unsafe extern "C" fn _init() {}
#[unsafe(no_mangle)]
unsafe extern "C" fn _fini() {}
