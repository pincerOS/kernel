use std::io::Write;

use lz4::frame::FrameOptions;

fn main() {
    let mut args = std::env::args().skip(1);
    let input_path = args.next().unwrap();
    let output_path = args.next().unwrap();

    let input = std::fs::read(input_path).unwrap();

    let frame = FrameOptions {
        block_indep: true,
        block_checksum: true,
        content_checksum: true,
        content_size: lz4::frame::ContentSize::Known(input.len() as u64),
        max_block_size: lz4::frame::MaxBlockSize::Size4MiB,
    };

    let mut output = vec![0; frame.max_compressed_size(input.len())];
    let compressed = lz4::compress_into(&frame, &input, &mut output).unwrap();

    let mut check = vec![0; input.len()];
    let data = lz4::decode_into(compressed, &mut check).unwrap();
    assert!(input == data);

    let mut file = std::fs::File::create_new(output_path).unwrap();
    file.write_all(compressed).unwrap();
}
