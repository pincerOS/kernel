#![no_std]

pub mod color;
pub mod format;

pub fn blit_buffer(
    dst: &mut [u32],
    dst_w: usize,
    dst_h: usize,
    dst_stride: usize,
    dst_x: usize,
    dst_y: usize,
    src: &[u32],
    src_w: usize,
    src_h: usize,
    src_stride: usize,
) {
    let effective_width = src_w.min(dst_w.saturating_sub(dst_x));
    let effective_height = src_h.min(dst_h.saturating_sub(dst_y));
    for r in 0..effective_height {
        let src_row = &src[r * src_stride..][..effective_width];
        let dst_row = &mut dst[(r + dst_y) * dst_stride + dst_x..][..effective_width];
        dst_row.copy_from_slice(src_row);
    }
}

pub fn blit_buffer_blend(
    dst: &mut [u32],
    dst_w: usize,
    dst_h: usize,
    dst_stride: usize,
    dst_x: usize,
    dst_y: usize,
    src: &[u32],
    src_w: usize,
    src_h: usize,
    src_stride: usize,
) {
    let effective_width = src_w.min(dst_w.saturating_sub(dst_x));
    let effective_height = src_h.min(dst_h.saturating_sub(dst_y));
    if effective_width == 0 || effective_height == 0 {
        return;
    }
    for r in 0..effective_height {
        let src_row = &src[r * src_stride..][..effective_width];
        let dst_row = &mut dst[(r + dst_y) * dst_stride + dst_x..][..effective_width];
        for (a, b) in src_row.iter().zip(dst_row) {
            *b = color::blend(*a, *b);
        }
    }
}
