use super::{BufferHeader, EventQueue, GlobalMeta, SemDescriptor, VideoMeta};

pub struct BufferHandle {
    buf: *mut BufferHeader,
    pub global_meta: GlobalMeta,
    pub video_meta: VideoMeta,
    pub present_sem: SemDescriptor,
    pub fds: [u32; 8],
}

unsafe impl Send for BufferHandle {}
unsafe impl Sync for BufferHandle {}

impl BufferHandle {
    pub unsafe fn new(buf: *mut BufferHeader, fds: &[u32]) -> Self {
        let global_meta = unsafe { (&raw const (*buf).meta).read_volatile() };
        let video_meta = unsafe { (&raw const (*buf).video_meta).read_volatile() };
        let present_sem = unsafe { (&raw const (*buf).present_sem).read_volatile() };

        // TODO: ensure this wasn't changed before connection started...
        // TODO: track actual size of the buffer (don't trust values from the buffer itself)
        assert!(global_meta.vmem_offset + global_meta.vmem_size <= global_meta.segment_size);

        let mut local_fds = [u32::MAX; 8];
        local_fds[..fds.len()].copy_from_slice(fds);

        BufferHandle {
            buf,
            global_meta,
            video_meta,
            present_sem,
            fds: local_fds,
        }
    }

    pub const fn client_to_server_queue(&mut self) -> &EventQueue {
        unsafe { &(*self.buf).client_to_server_queue }
    }
    pub const fn server_to_client_queue(&mut self) -> &EventQueue {
        unsafe { &(*self.buf).server_to_client_queue }
    }

    // TODO: this is UB; need a volatile wrapper of some kind
    // TODO: some way to expose this while retaining compiler write
    // coalescing?  (If the wrapper uses volatile, the compiler
    // won't turn writes into simd writes)
    pub fn video_mem(&mut self) -> &mut [u32] {
        assert!(
            self.global_meta.vmem_offset + self.global_meta.vmem_size
                <= self.global_meta.segment_size
        );

        let vmem = self
            .buf
            .cast::<u32>()
            .wrapping_byte_add(self.global_meta.vmem_offset as usize);
        let vmem_slice = core::ptr::slice_from_raw_parts_mut(
            vmem,
            self.global_meta.vmem_size as usize / size_of::<u32>(),
        );
        unsafe { &mut *vmem_slice }
    }

    pub fn video_mem_u128(&mut self) -> &mut [u128] {
        let buf = self.video_mem();
        let len_u32 = buf.len();
        let len_u128 = len_u32 * size_of::<u32>() / size_of::<u128>();
        let ptr = buf.as_mut_ptr().cast::<u128>();
        assert!(len_u32 * size_of::<u32>() == len_u128 * size_of::<u128>());
        assert!(ptr.is_aligned());
        unsafe { core::slice::from_raw_parts_mut(ptr, len_u128) }
    }

    pub fn get_sem_fd(&self, sem: SemDescriptor) -> u32 {
        self.fds[sem.0 as usize]
    }
}
