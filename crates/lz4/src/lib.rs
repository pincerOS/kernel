#![no_std]

pub mod block;
pub mod compress;
pub mod frame;
pub mod xxh;

pub fn decode_into<'a>(data: &'_ [u8], buf: &'a mut [u8]) -> Result<&'a [u8], frame::Lz4Error> {
    let (hdr, data) = frame::read_frame(data)?;

    if let Some(size) = hdr.content_size() {
        if size > buf.len() as u64 {
            return Err(frame::Lz4Error::OutOfSpace);
        }
    }

    let validate = frame::ValidateMode::Checksums;
    let length = frame::decode_frames(&hdr, data, buf, 0, validate)?;
    Ok(&buf[..length])
}

pub fn compress_into<'a>(
    frame: &frame::FrameOptions,
    data: &'_ [u8],
    buf: &'a mut [u8],
) -> Result<&'a [u8], frame::CompressError> {
    let header = frame::FrameHeader::new(frame);
    let off = frame::write_header(buf, &header)?;
    let off = frame::encode_frames(frame, data, buf, off)?;
    Ok(&buf[..off])
}
