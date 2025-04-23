pub struct Framebuffer {
    #[allow(unused)]
    pub fd: usize,
    pub width: usize,
    pub height: usize,
    pub stride: usize,
    pub data: &'static mut [u128],
}

pub fn present(fb: &mut Framebuffer, buf: &[u128]) {
    proto::memcpy128(&mut fb.data, &buf);
    // Force writes to go through
    core::hint::black_box(&mut *fb);
}

pub fn init_fb(width: usize, height: usize) -> Framebuffer {
    let mut fb = ulib::sys::RawFB {
        fd: 0,
        size: 0,
        width: 0,
        height: 0,
        pitch: 0,
    };
    let buffer_fd = unsafe { ulib::sys::sys_acquire_fb(width, height, &mut fb) };
    println!("Buffer: {:?}", buffer_fd);
    println!(
        "buffer_size {}, width {}, height {}, pitch {}",
        fb.size, fb.width, fb.height, fb.pitch
    );

    let mapped = unsafe { ulib::sys::mmap(0, fb.size, 0, 0, buffer_fd as u32, 0).unwrap() };
    let framebuf = unsafe {
        core::slice::from_raw_parts_mut::<u128>(mapped.cast(), fb.size / size_of::<u128>())
    };

    Framebuffer {
        fd: buffer_fd as usize,
        width: fb.width,
        height: fb.height,
        stride: fb.pitch / size_of::<u32>(),
        data: framebuf,
    }
}
