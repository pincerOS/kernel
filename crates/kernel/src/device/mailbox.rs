#![allow(dead_code, nonstandard_style)]

//! Driver for the VideoCore Gpu mailbox system (bcm2835-mbox)
//!
//! Note: not for bcm2836-vchiq or bcm2835-vchiq, those are a different
//! type of mailbox (same device tree node name, though)
//!
//! Primarily based on the documentation from jsandler18's OS:
//! <https://jsandler18.github.io/extra/mailbox.html> and
//! <https://github.com/raspberrypi/firmware/wiki/Mailbox-property-interface>
//! (since this isn't properly documented anywhere...)

use crate::{
    memory::{self, physical_addr},
    sync::Volatile,
};

pub struct VideoCoreMailbox {
    base: *mut u32,
}

pub unsafe trait PropertyRequest: bytemuck::Pod {
    const TAG: u32;
    type Output;
    unsafe fn parse_response(data: &[u8]) -> Self::Output;
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct PropGetPowerState {
    pub device_id: u32,
}
unsafe impl bytemuck::Zeroable for PropGetPowerState {}
unsafe impl bytemuck::Pod for PropGetPowerState {}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct PropGetPowerStateResponse {
    pub device_id: u32,
    pub state: u32,
}
unsafe impl bytemuck::Zeroable for PropGetPowerStateResponse {}
unsafe impl bytemuck::Pod for PropGetPowerStateResponse {}

unsafe impl PropertyRequest for PropGetPowerState {
    const TAG: u32 = 0x00020001;
    type Output = PropGetPowerStateResponse;
    unsafe fn parse_response(data: &[u8]) -> Self::Output {
        *bytemuck::from_bytes(&data[..size_of::<Self::Output>()])
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct PropGetPowerWaitTime {
    pub device_id: u32,
}
unsafe impl bytemuck::Zeroable for PropGetPowerWaitTime {}
unsafe impl bytemuck::Pod for PropGetPowerWaitTime {}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct PropGetPowerWaitTimeResponse {
    pub device_id: u32,
    pub microseconds: u32,
}
unsafe impl bytemuck::Zeroable for PropGetPowerWaitTimeResponse {}
unsafe impl bytemuck::Pod for PropGetPowerWaitTimeResponse {}

unsafe impl PropertyRequest for PropGetPowerWaitTime {
    const TAG: u32 = 0x00020002;
    type Output = PropGetPowerWaitTimeResponse;
    unsafe fn parse_response(data: &[u8]) -> Self::Output {
        *bytemuck::from_bytes(&data[..size_of::<Self::Output>()])
    }
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct PropSetPowerState {
    pub device_id: u32,
    pub state: u32,
}
unsafe impl bytemuck::Zeroable for PropSetPowerState {}
unsafe impl bytemuck::Pod for PropSetPowerState {}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct PropSetPowerStateResponse {
    pub device_id: u32,
    pub state: u32,
}
unsafe impl bytemuck::Zeroable for PropSetPowerStateResponse {}
unsafe impl bytemuck::Pod for PropSetPowerStateResponse {}

unsafe impl PropertyRequest for PropSetPowerState {
    const TAG: u32 = 0x00028001;
    type Output = PropSetPowerStateResponse;
    unsafe fn parse_response(data: &[u8]) -> Self::Output {
        *bytemuck::from_bytes(&data[..size_of::<Self::Output>()])
    }
}

#[derive(Debug)]
pub struct MailboxError;

impl VideoCoreMailbox {
    const MBOX_READ: usize = 0x00;
    const MBOX_POLL: usize = 0x10;
    const MBOX_SENDER: usize = 0x14;
    const MBOX_STATUS: usize = 0x18;
    const MBOX_CONFIG: usize = 0x1C;
    const MBOX_WRITE: usize = 0x20;

    const CHANNEL_MASK: u32 = 0x0000000F;

    const STATUS_FULL: u32 = 0x80000000;
    const STATUS_EMPTY: u32 = 0x40000000;

    const TAG_RESPONSE: u32 = 0x80000000;

    pub unsafe fn init(base: *mut ()) -> Self {
        let base = base.cast::<u32>();
        assert!(base.is_aligned());
        Self { base }
    }

    pub unsafe fn mailbox_call(
        &mut self,
        channel: u8,
        buffer: &mut [u128],
    ) -> Result<(), MailboxError> {
        let status_reg = Volatile(self.base.wrapping_byte_add(Self::MBOX_STATUS));
        let read_reg = Volatile(self.base.wrapping_byte_add(Self::MBOX_READ));
        let write_reg = Volatile(self.base.wrapping_byte_add(Self::MBOX_WRITE));

        unsafe {
            while (status_reg.read() & Self::STATUS_FULL) != 0 {
                crate::arch::wfe();
            }
        }

        let buffer_bytes = size_of_val(buffer);
        let buffer_ptr = buffer.as_mut_ptr();
        let addr = physical_addr(buffer_ptr.addr()).unwrap();
        assert!(addr <= u32::MAX as u64 && addr % 16 == 0);
        assert!(channel as u32 <= Self::CHANNEL_MASK);

        let value = (addr as u32 & !Self::CHANNEL_MASK) | (channel as u32 & Self::CHANNEL_MASK);

        // TODO: are mailbox messages 1-to-1, or can there be spurious
        // messages?  If not 1-to-1, this approach (send message and then
        // immediately block on response) will cause issues.

        // An async approach may be better; messages are returned via the
        // same buffer, so it should be possible to queue them cleanly.

        // TODO: memory barriers:
        // https://github.com/raspberrypi/firmware/wiki/Accessing-mailboxes

        // TODO: translate buffer addresses for non-property calls
        // (GPU memory and CPU memory may have different virtual addresses)

        unsafe {
            memory::clean_physical_buffer_for_device(buffer_ptr.cast(), buffer_bytes);
        }
        unsafe { write_reg.write(value) };

        loop {
            unsafe {
                while (status_reg.read() & Self::STATUS_EMPTY) != 0 {
                    crate::arch::wfe();
                }
            }

            let message = unsafe { read_reg.read() };
            if (message & Self::CHANNEL_MASK) == channel as u32 {
                unsafe {
                    memory::invalidate_physical_buffer_for_device(buffer_ptr.cast(), buffer_bytes);
                }
                break;
            } else {
                println!("Warning: received mailbox message from wrong channel?");
            }
        }

        Ok(())
    }

    unsafe fn get_property_inner<'a>(
        &mut self,
        buffer: &'a mut [u128],
        tag: u32,
        request_data: &[u32],
    ) -> Result<(u32, &'a [u8]), MailboxError> {
        // TODO: "Response may include unsolicited tags."

        let words: &mut [u32] = bytemuck::cast_slice_mut::<_, u32>(&mut *buffer);
        let buffer_size = size_of_val(words) as u32;

        let data_words = (words.len() - 6) as u32;

        let data = [
            buffer_size,
            0x00000000, // request
            tag,
            data_words * size_of::<u32>() as u32,
            0,
        ];

        assert!(
            data.len() + data_words as usize + 1 == words.len()
                && request_data.len() <= data_words as usize
        );

        words[..data.len()].copy_from_slice(&data);
        words[data.len()..][..request_data.len()].copy_from_slice(request_data);
        words[data.len() - 1] = 0;

        unsafe {
            self.mailbox_call(8, &mut *buffer).unwrap();
        }

        let words: &[u32] = bytemuck::cast_slice::<_, u32>(&*buffer);

        let response = words[1];
        let response_size = words[3];
        let response_code = words[4];

        if response_code != 0x80000000 {
            return Err(MailboxError);
        }

        let response_data: &[u8] = &bytemuck::cast_slice(&words[5..])[..response_size as usize];

        // TODO: properly detect & handle responses that require a larger buffer
        // (ie. try requesing the commandline with a small buffer?)
        if response_size > data_words * size_of::<u32>() as u32 {
            // retry?  response was truncated
            return Err(MailboxError);
        }

        Ok((response, response_data))
    }

    pub unsafe fn get_property<T>(&mut self, request: T) -> Result<T::Output, MailboxError>
    where
        T: PropertyRequest,
    {
        const BUFFER_WORDS: usize = 64;
        let mut buffer = [0u128; BUFFER_WORDS / 4];

        let res = unsafe {
            self.get_property_inner(
                &mut buffer,
                T::TAG,
                bytemuck::cast_slice(bytemuck::bytes_of(&request)),
            )
        };

        res.map(|(_code, data)| unsafe { T::parse_response(data) })
    }

    pub unsafe fn get_framebuffer(&mut self) -> Surface {
        // https://github.com/raspberrypi/firmware/wiki/Mailbox-property-interface

        const TAG_ALLOC_BUFFER: u32 = 0x00040001;
        const TAG_RELEASE_BUFFER: u32 = 0x00048001;
        const TAG_BLANK_SCREEN: u32 = 0x00040002;
        const TAG_SET_PHYS_DIMS: u32 = 0x00048003;
        const TAG_SET_VIRT_DIMS: u32 = 0x00048004;
        const TAG_SET_DEPTH: u32 = 0x00048005;
        const TAG_SET_PIXEL_ORDER: u32 = 0x00048006;
        const TAG_GET_PITCH: u32 = 0x00040008;
        const TAG_END: u32 = 0x00000000;

        // let width = 640;
        // let height = 480;
        let width = 1280;
        let height = 720;

        const BUFFER_WORDS: usize = 64;
        let mut buffer = [0u128; BUFFER_WORDS / 4];
        let words: &mut [u32; BUFFER_WORDS] = bytemuck::cast_slice_mut::<_, u32>(&mut buffer)
            .try_into()
            .unwrap();
        let buffer_size = (words.len() * size_of::<u32>()) as u32;

        let data = [
            buffer_size,
            0x00000000, // request
            TAG_SET_PHYS_DIMS,
            8,
            0,
            width as u32,
            height as u32,
            TAG_SET_VIRT_DIMS,
            8,
            0,
            width as u32,
            height as u32,
            TAG_SET_DEPTH,
            4,
            0,
            32,
            TAG_SET_PIXEL_ORDER,
            4,
            0,
            0, // RGB
            TAG_ALLOC_BUFFER,
            8,
            0,
            16,
            0,
            TAG_GET_PITCH,
            4,
            0,
            0,
            TAG_END,
        ];
        words[..data.len()].copy_from_slice(&data);

        // println!("{:?}", words);

        unsafe {
            self.mailbox_call(8, &mut buffer).unwrap();
        }

        let words: &[u32; BUFFER_WORDS] =
            bytemuck::cast_slice::<_, u32>(&buffer).try_into().unwrap();
        // println!("{:?}", words);

        let response = words[1];

        let buffer_ptr = words[23];
        let buffer_size = words[24] as usize;
        let pitch = words[28] as usize;
        assert!(pitch == width * 4);
        let pitch_elems = pitch / 4;

        // println!("{:?}", bytemuck::cast_slice_mut::<_, u32>(&mut buffer));
        println!("Response: {response:#010x}\nbuffer: {buffer_ptr:#010x}, {buffer_size:#010x}, {pitch:#010x}");

        let ptr = unsafe { memory::map_physical_noncacheable(buffer_ptr as usize, buffer_size) };
        let ptr = ptr.as_ptr().cast::<u128>();
        assert!(ptr.is_aligned());

        let array_elems = buffer_size / size_of::<u128>();
        let array = unsafe { core::slice::from_raw_parts_mut(ptr, array_elems) };

        Surface::new(array, width, height, pitch_elems)
    }
}

pub struct Surface {
    alternate: alloc::boxed::Box<[u128]>,
    buffer: &'static mut [u128],
    framerate: usize,
    time_step: usize,
    width: usize,
    height: usize,
    pitch_elems: usize,
}

#[cfg(target_arch = "aarch64")]
fn memcpy128(dst: &mut [u128], src: &[u128]) {
    let len = dst.len();
    assert_eq!(len, src.len());
    assert!(len % 64 == 0);
    unsafe {
        core::arch::asm!(r"
        1:
        ldp {tmp1}, {tmp2}, [{src}, #0]
        stp {tmp1}, {tmp2}, [{dst}, #0]
        ldp {tmp1}, {tmp2}, [{src}, #16]
        stp {tmp1}, {tmp2}, [{dst}, #16]
        ldp {tmp1}, {tmp2}, [{src}, #32]
        stp {tmp1}, {tmp2}, [{dst}, #32]
        ldp {tmp1}, {tmp2}, [{src}, #48]
        stp {tmp1}, {tmp2}, [{dst}, #48]
        add {src}, {src}, #64 // TODO: figure out east way to use index increment
        add {dst}, {dst}, #64
        subs {count}, {count}, #4
        b.hi 1b // if count > 0, loop
        ",
        src = in(reg) src.as_ptr(),
        dst = in(reg) dst.as_mut_ptr(),
        count = in(reg) len,
        tmp1 = out(reg) _, tmp2 = out(reg) _,
        )
    }
}

#[cfg(not(target_arch = "aarch64"))]
fn memcpy128(dst: &mut [u128], src: &[u128]) {
    dst.copy_from_slice(src)
}

impl Surface {
    fn new(buffer: &'static mut [u128], width: usize, height: usize, pitch_elems: usize) -> Self {
        let framerate = 30;
        let time_step = 1_000_000 / framerate;

        let mut alternate = alloc::vec::Vec::new();
        alternate.reserve_exact(buffer.len());
        alternate.resize(height * pitch_elems / 4, 0);

        Self {
            alternate: alternate.into_boxed_slice(),
            buffer,
            width,
            height,
            pitch_elems,
            framerate,
            time_step,
        }
    }
    pub fn dimensions(&self) -> (usize, usize) {
        (self.width, self.height)
    }
    pub fn framerate(&self) -> usize {
        self.framerate
    }
    pub fn buffer(&mut self) -> &mut [u32] {
        bytemuck::cast_slice_mut(&mut self.alternate)
    }
    #[inline(never)]
    pub fn present(&mut self) {
        // Minimize tearing by doing a fast copy from the alternate
        // buffer into the actual framebuffer.
        memcpy128(self.buffer, &self.alternate);

        // unsafe {
        //     memory::clean_physical_buffer_for_device(
        //         self.buffer().as_mut_ptr().cast(),
        //         size_of_val(self.buffer),
        //     );
        // }

        // self.buffer.copy_from_slice(&self.alternate);
        // Force writes to go through
        core::hint::black_box(&mut *self.buffer);
    }
    pub fn wait_for_frame(&self) {
        // TODO: proper vsync IRQs?
        let now = crate::sync::get_time();
        crate::sync::spin_sleep_until(now.next_multiple_of(self.time_step));
    }
}

impl core::ops::Index<(usize, usize)> for Surface {
    type Output = u32;
    fn index(&self, (row, col): (usize, usize)) -> &Self::Output {
        &bytemuck::cast_slice(&self.alternate)[row * self.pitch_elems + col]
    }
}
impl core::ops::IndexMut<(usize, usize)> for Surface {
    fn index_mut(&mut self, (row, col): (usize, usize)) -> &mut Self::Output {
        let idx = row * self.pitch_elems + col;
        &mut bytemuck::cast_slice_mut(&mut self.alternate)[idx]
    }
}
