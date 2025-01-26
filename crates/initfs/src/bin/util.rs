use std::collections::BinaryHeap;
use std::error::Error;
use std::io::{Read, Seek};
use std::os::unix::fs::MetadataExt;
use std::path::PathBuf;

use initfs::ArchiveHeader;

#[path = "util/args.rs"]
mod args;

#[derive(Debug)]
struct Args {
    cmd: Command,
}

#[derive(Debug)]
enum Command {
    None,
    Create {
        files: Vec<PathBuf>,
        root: Option<PathBuf>,
        out: Option<PathBuf>,
        compress: bool,
    },
}

fn help_message(arg0: &str) {
    println!(
        "\
Usage: {arg0} create -o <dest> [files...]

Options:
    -h, --help      show this message
    -r, --root=DIR  specify the root path of the archive
    -o, --out=FILE  specify the destination of the archive file (default .)
    -z, --compress  enable or disable compression (default on)
"
    );
}

fn parse_args(args: impl Iterator<Item = String>) -> Result<Option<Args>, Box<dyn Error>> {
    let mut command = Command::None;
    let mut root = None;
    let mut out = None;
    let mut compress = None;

    let res = args::parse_args(
        args,
        |flag, inline, args, arg0| -> Result<_, Box<dyn Error>> {
            match (flag, inline) {
                ("h" | "-help", None) => {
                    help_message(arg0);
                    return Ok(None);
                }
                ("r" | "-root", _) => {
                    root = Some(args::parse_param("-root", args, inline)?);
                }
                ("o" | "-out" | "-output", _) => {
                    out = Some(args::parse_param("-output", args, inline)?);
                }
                ("z" | "-compress", _) => {
                    compress = Some(args::parse_flag_optional_bool(inline)?);
                }
                (c, _) => return Err(format!("Unknown flag '-{}'", c).into()),
            }
            Ok(Some(()))
        },
        |index, arg| {
            if index == 0 {
                match &*arg {
                    "c" | "create" => {
                        command = Command::Create {
                            files: Vec::new(),
                            root: None,
                            out: None,
                            compress: true,
                        }
                    }
                    _ => return Err(format!("Unknown subcommand {:?}", arg).into()),
                }
            } else {
                match &mut command {
                    Command::None => {
                        return Err("Subcommand takes no positional args".into())
                    }
                    Command::Create { files, .. } => {
                        files.push(arg.into());
                    }
                }
            }
            Ok(Some(()))
        },
    )?;

    if res.is_none() {
        return Ok(None);
    }

    if let Some(r) = root {
        match &mut command {
            Command::Create { root, .. } => *root = Some(r.into()),
            _ => return Err("--root only applies for create subcommand".into()),
        }
    }
    if let Some(o) = out {
        match &mut command {
            Command::Create { out, .. } => *out = Some(o.into()),
            _ => return Err("--out only applies for create subcommand".into()),
        }
    }
    if let Some(c) = compress {
        match &mut command {
            Command::Create { compress, .. } => *compress = c,
            _ => return Err("--compress only applies for create subcommand".into()),
        }
    }

    Ok(Some(Args { cmd: command }))
}

fn main() {
    let args = match parse_args(std::env::args()) {
        Ok(Some(args)) => args,
        Ok(None) => return,
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };

    match args.cmd {
        Command::Create {
            files,
            root,
            out,
            compress,
        } => {
            let res = create_archive(files, root, out.expect("missing --output"), compress);
            if let Err(e) = res {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Command::None => {
            help_message(&std::env::args().next().unwrap_or_default());
        }
    }
}

fn create_archive(
    files: Vec<PathBuf>,
    root: Option<PathBuf>,
    out: PathBuf,
    compress: bool,
) -> Result<(), std::io::Error> {
    #[derive(PartialEq, Eq)]
    enum QueueEntry {
        File(PathBuf),
        EndDir(PathBuf, usize),
    }
    impl core::cmp::PartialOrd for QueueEntry {
        fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
            Some(self.cmp(other))
        }
    }
    impl core::cmp::Ord for QueueEntry {
        fn cmp(&self, other: &Self) -> std::cmp::Ordering {
            match (self, other) {
                (QueueEntry::File(a), QueueEntry::File(b)) => a.cmp(b).reverse(),
                (QueueEntry::File(a), QueueEntry::EndDir(b, _)) => {
                    if a.starts_with(b) {
                        core::cmp::Ordering::Less.reverse()
                    } else {
                        a.cmp(b).reverse()
                    }
                }
                (QueueEntry::EndDir(a, _), QueueEntry::File(b)) => {
                    if b.starts_with(a) {
                        core::cmp::Ordering::Greater.reverse()
                    } else {
                        a.cmp(b).reverse()
                    }
                }
                (QueueEntry::EndDir(a, _), QueueEntry::EndDir(b, _)) => a.cmp(b),
            }
        }
    }

    let verbose = true;
    let follow_symlinks = false;
    let root = root.unwrap_or_else(|| PathBuf::from(".")).canonicalize()?;

    let iter = files.into_iter().map(|f| f.canonicalize()).map(|f| {
        f.and_then(|f| {
            f.strip_prefix(&root)
                .map(|f| f.to_owned())
                .map_err(|_| std::io::ErrorKind::InvalidInput.into())
        })
    });

    let mut queue: BinaryHeap<_> = iter
        .map(|r| r.map(QueueEntry::File))
        .collect::<Result<_, _>>()?;

    let mut files: Vec<initfs::FileHeader> = Vec::new();
    let mut strings: Vec<u8> = Vec::new();
    let mut data: Vec<u8> = Vec::new();

    let mut i = 0;

    while let Some(file) = queue.pop() {
        let file = match file {
            QueueEntry::File(f) => f,
            QueueEntry::EndDir(_, idx) => {
                files[idx].offset = (files.len() - idx) as u32;
                continue;
            }
        };

        i += 1;

        if verbose {
            println!("{}", file.display());
        }

        let bytes = file.as_os_str().as_encoded_bytes();
        let name_len = bytes.len();
        let mut name_inline = [0; 32];
        name_inline[..name_len.min(32)].copy_from_slice(&bytes[..name_len.min(32)]);

        if name_len > 32 {
            let idx = strings.len();
            strings.extend(bytes);
            name_inline[28..32].copy_from_slice(&u32::to_le_bytes(idx as u32));
        }

        let offset;
        let size;
        let compressed_size;
        let compress_mode;

        let path = root.join(&file);
        let stat = if follow_symlinks {
            std::fs::metadata(&path)?
        } else {
            std::fs::symlink_metadata(&path)?
        };
        if stat.is_dir() {
            // TODO: end of dir file list?
            offset = 0;
            size = 0;
            compressed_size = 0;
            compress_mode = 0;

            for entry in path.read_dir()? {
                queue.push(QueueEntry::File(file.join(entry?.file_name())));
            }
            queue.push(QueueEntry::EndDir(file, files.len()));
        } else if stat.is_file() {
            if data.len() % 16 != 0 {
                data.resize(data.len().next_multiple_of(16), 0);
            }

            offset = data.len();

            let mut open_file = std::fs::File::open(&path)?;
            let len = open_file.seek(std::io::SeekFrom::End(0))?;
            open_file.seek(std::io::SeekFrom::Start(0))?;

            if compress {
                let frame = lz4::frame::FrameOptions {
                    block_indep: true,
                    block_checksum: true,
                    content_checksum: true,
                    content_size: lz4::frame::ContentSize::Known(len),
                    max_block_size: lz4::frame::MaxBlockSize::Size4MiB,
                };
                let mut input = Vec::new(); // TODO: reuse buffer
                open_file.read_to_end(&mut input)?;
                assert_eq!(input.len() as u64, len);

                data.resize(data.len() + frame.max_compressed_size(len as usize), 0);
                let res = lz4::compress_into(&frame, &input, &mut data[offset..]).unwrap();
                let len = res.len();
                data.truncate(offset + len);
            } else {
                open_file.read_to_end(&mut data)?;
            }

            size = len as usize;
            compressed_size = data.len() - offset;
            compress_mode = initfs::COMPRESS_LZ4;
        } else {
            offset = 0;
            size = 0;
            compressed_size = 0;
            compress_mode = 0;
        }

        files.push(initfs::FileHeader {
            inode: i,
            mode: stat.mode(),
            uid: stat.uid() as u16,
            gid: stat.gid() as u16,
            name_len: name_len as u32,
            name_inline,
            compress_mode,
            reserved: [0; 3],
            offset: offset as u32,
            size: size as u32,
            compressed_size: compressed_size as u32,
        });
    }

    if data.len() % 16 != 0 {
        data.resize(data.len().next_multiple_of(16), 0);
    }
    if strings.len() % 16 != 0 {
        strings.resize(strings.len().next_multiple_of(16), 0);
    }

    let header_size = size_of::<ArchiveHeader>();
    let file_stride = size_of::<initfs::FileHeader>();
    let files_size = files.len() * file_stride;
    let strings_size = strings.len();
    let data_size = data.len();
    let data_start_offset = header_size + files_size + strings_size;
    let total_size = data_start_offset + data_size;

    let header = ArchiveHeader {
        magic: initfs::MAGIC,
        endian: initfs::ENDIAN_LE,
        version: 1,
        header_size: header_size as u16,
        total_size: total_size as u32,
        file_count: files.len() as u32,
        file_stride: file_stride as u32,
        strings_size: strings_size as u32,
    };

    let mut orig_size = 0u64;
    for file in files.iter_mut() {
        if file.compressed_size != 0 {
            file.offset += data_start_offset as u32;
        }
        orig_size += file.size as u64;
    }

    let compression_ratio = total_size as f64 / orig_size as f64;
    println!(
        "Compressed {} data bytes to {} bytes ==> {:5.2}%",
        orig_size,
        total_size,
        compression_ratio * 100.0
    );

    let mut output: Vec<u8> = Vec::with_capacity(total_size);
    {
        let header_bytes = unsafe {
            core::slice::from_raw_parts(
                (&raw const header).cast::<u8>(),
                size_of::<ArchiveHeader>(),
            )
        };
        output.extend(header_bytes);
        output.resize(header_size, 0);
    }
    {
        assert_eq!(size_of_val(&*files), files_size);
        let files_bytes =
            unsafe { core::slice::from_raw_parts(files.as_ptr().cast::<u8>(), files_size) };
        output.extend(files_bytes);
    }
    output.extend(strings);

    output.extend(data);

    assert_eq!(total_size, output.len());

    std::fs::write(out, output)?;

    Ok(())
}
