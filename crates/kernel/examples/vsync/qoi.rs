const fn rgba(r: u8, g: u8, b: u8, a: u8) -> u32 {
    u32::from_be_bytes([a, r, g, b])
}
const fn de_rgba(color: u32) -> (u8, u8, u8, u8) {
    let [a, r, g, b] = color.to_be_bytes();
    (r, g, b, a)
}

#[derive(Debug)]
pub struct QoiHeader {
    pub magic: [u8; 4], // magic bytes "qoif"
    pub width: u32,     // image width in pixels (BE)
    pub height: u32,    // image height in pixels (BE)
    pub channels: u8,   // 3 = RGB, 4 = RGBA
    pub colorspace: u8, // 0 = sRGB with linear alpha; 1 = all channels linear
}

pub fn read_qoi_header(data: &[u8]) -> Option<(QoiHeader, &[u8])> {
    data.split_at_checked(14)
        .map(|(h, rest)| {
            (
                QoiHeader {
                    magic: h[..4].try_into().unwrap(),
                    width: u32::from_be_bytes(h[4..][..4].try_into().unwrap()),
                    height: u32::from_be_bytes(h[8..][..4].try_into().unwrap()),
                    channels: h[12],
                    colorspace: h[13],
                },
                rest,
            )
        })
        .filter(|(h, _)| h.magic == *b"qoif")
}

fn premultiply(pixel: u32) -> u32 {
    // TODO: sRGB conversions
    let (r, g, b, a) = de_rgba(pixel);
    let (r, g, b) = (
        r as u32 * a as u32 / 255,
        g as u32 * a as u32 / 255,
        b as u32 * a as u32 / 255,
    );
    rgba(r as u8, g as u8, b as u8, a)
}

pub fn decode_qoi(header: &QoiHeader, mut stream: &[u8], output: &mut [u32], out_stride: usize) {
    let mut history = [rgba(0, 0, 0, 0); 64];

    // ...huh.  If the initial alpha is wrong, the hash indexing breaks
    // and makes lots of glitch patterns.
    let mut prev = rgba(0, 0, 0, 0xFF);
    let mut row_idx = 0;
    let mut row_base = 0;
    let img_width = header.width as usize;

    loop {
        let pixel;
        stream = match *stream {
            // QOI_OP_RGB
            [0b1111_1110, r, g, b, ref rest @ ..] => {
                pixel = rgba(r, g, b, (prev >> 24) as u8);
                rest
            }
            // QOI_OP_RGBA
            [0b1111_1111, r, g, b, a, ref rest @ ..] => {
                pixel = rgba(r, g, b, a);
                rest
            }
            // QOI_OP_DIFF
            [diff @ (0b0100_0000..=0b0111_1111), ref rest @ ..] => {
                let db = ((diff >> 0) & 0b11).wrapping_sub(2);
                let dg = ((diff >> 2) & 0b11).wrapping_sub(2);
                let dr = ((diff >> 4) & 0b11).wrapping_sub(2);

                let (pr, pg, pb, pa) = de_rgba(prev);
                pixel = rgba(
                    pr.wrapping_add(dr),
                    pg.wrapping_add(dg),
                    pb.wrapping_add(db),
                    pa,
                );
                rest
            }
            // QOI_OP_LUMA
            [diff_green @ (0b1000_0000..=0b1011_1111), diff, ref rest @ ..] => {
                let dg = (diff_green & 0b0011_1111).wrapping_sub(32);
                let db = ((diff >> 0) & 0b0000_1111).wrapping_sub(8).wrapping_add(dg);
                let dr = ((diff >> 4) & 0b0000_1111).wrapping_sub(8).wrapping_add(dg);

                let (pr, pg, pb, pa) = de_rgba(prev);
                pixel = rgba(
                    pr.wrapping_add(dr),
                    pg.wrapping_add(dg),
                    pb.wrapping_add(db),
                    pa,
                );
                rest
            }
            // QOI_OP_RUN
            [run @ (0b1100_0000..=0b1111_1111), ref rest @ ..] => {
                let run = ((run & 0b0011_1111) + 1) as usize;

                let color = premultiply(prev);

                // let range_start = row_base + row_idx;
                // let range_end = range_start + run;
                // let row_range = range_start / img_width..range_end.div_ceil(img_width);
                // for base in row_range.into_iter().map(|r| r * out_stride) {
                //     for c in base.max(range_start)..(base + img_width).min(range_end) {
                //         output[c] = color;
                //     }
                // }

                let end = row_idx + run;
                let row_end = end.min(img_width);
                for i in row_idx..row_end {
                    output[row_base + i] = color;
                }
                let remainder = end - row_end;
                for row in 0..remainder / img_width {
                    for c in 0..img_width {
                        output[row_base + (1 + row) * out_stride + c] = color;
                    }
                }
                for i in 0..remainder % img_width {
                    output[row_base + (1 + remainder / img_width) * out_stride + i] = color;
                }

                row_base += ((row_idx + run) / img_width) * out_stride;
                row_idx = (row_idx + run) % img_width;

                stream = rest;
                continue;
            }
            [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01] => {
                break;
            }
            // QOI_OP_INDEX
            [idx @ (0b0000_0000..=0b0011_1111), ref rest @ ..] => {
                pixel = history[idx as usize & 0b0011_1111];
                rest
            }
            _ => {
                // invalid operation
                break;
            }
        };

        let (r, g, b, a) = de_rgba(pixel);
        let hash = (r as usize * 3 + g as usize * 5 + b as usize * 7 + a as usize * 11) % 64;
        history[hash] = pixel;
        prev = pixel;

        output[row_base + row_idx] = premultiply(pixel);

        row_idx += 1;

        if row_idx >= header.width as usize {
            row_idx = 0;
            row_base += out_stride;
        }
    }
}
