#![no_std]

extern crate alloc;
extern crate core;
extern crate std;

use core::error::Error;

use alloc::borrow::Cow;
use alloc::borrow::ToOwned;
use alloc::boxed::Box;
use alloc::collections::BinaryHeap;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use std::io::{Read, Seek};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::{eprintln, println};

use initfs::ArchiveHeader;

#[path = "util/args.rs"]
mod args;

fn help_message(arg0: &str) {
    println!(
        "\
Usage: {arg0} create -o <dest> [files...]

Global options:
    -h, --help      show this message
    -v, --verbose   verbose mode

Options for create subcommand:
    -r, --root=DIR  specify the root path of the archive
    -o, --out=FILE  specify the destination of the archive file (default .)
    -z, --compress  enable or disable compression (default on)
    -f, --follow    follow symlinks
"
    );
}

struct CommandParser {
    command: Command,
    verbose: bool,
}

enum Command {
    None,
    List {
        files: Vec<PathBuf>,
    },
    Create {
        files: Vec<PathBuf>,
        root: Option<PathBuf>,
        out: Option<PathBuf>,
        compress: Option<bool>,
        follow_links: Option<bool>,
    },
}

impl CommandParser {
    fn handle_flag(
        &mut self,
        flag: &str,
        inline: Option<&str>,
        args: &mut impl Iterator<Item = String>,
        arg0: &str,
    ) -> Result<Option<()>, Box<dyn Error>> {
        match (&mut self.command, flag, inline) {
            (_, "h" | "-help", None) => {
                help_message(arg0);
                return Ok(None);
            }
            (_, "v" | "-verbose", _) => {
                self.verbose = args::parse_flag_optional_bool(inline)?;
            }
            (Command::Create { root, .. }, "r" | "-root", _) => {
                *root = Some(args::parse_param("-root", args, inline)?.into());
            }
            (Command::Create { out, .. }, "o" | "-out" | "-output", _) => {
                *out = Some(args::parse_param("-output", args, inline)?.into());
            }
            (Command::Create { compress, .. }, "z" | "-compress", _) => {
                *compress = Some(args::parse_flag_optional_bool(inline)?);
            }
            (Command::Create { follow_links, .. }, "f" | "-follow", _) => {
                *follow_links = Some(args::parse_flag_optional_bool(inline)?);
            }
            (Command::Create { .. }, c, _) => {
                return Err(format!("Unknown option for subcommand create: '-{}'", c).into())
            }
            (_, c, _) => return Err(format!("Unknown global option: '-{}'", c).into()),
        }
        Ok(Some(()))
    }
    fn handle_pos(&mut self, index: usize, arg: String) -> Result<Option<()>, Box<dyn Error>> {
        if index == 0 {
            match &*arg {
                "c" | "create" => {
                    self.command = Command::Create {
                        files: Vec::new(),
                        root: None,
                        out: None,
                        compress: None,
                        follow_links: None,
                    };
                }
                "l" | "list" => {
                    self.command = Command::List { files: Vec::new() };
                }
                _ => return Err(format!("Unknown subcommand {:?}", arg).into()),
            }
            return Ok(Some(()));
        }
        match self.command {
            Command::None => return Err("Subcommand takes no positional args".into()),
            Command::Create { ref mut files, .. } => {
                files.push(arg.into());
            }
            Command::List { ref mut files, .. } => {
                files.push(arg.into());
            }
        }
        Ok(Some(()))
    }
}

fn parse_args(args: impl Iterator<Item = String>) -> Result<Option<CommandParser>, Box<dyn Error>> {
    let parser = core::cell::RefCell::new(CommandParser {
        command: Command::None,
        verbose: false,
    });

    let res = args::parse_args(
        args,
        |flag, inline, args, arg0| parser.borrow_mut().handle_flag(flag, inline, args, arg0),
        |index, arg| parser.borrow_mut().handle_pos(index, arg),
    )?;

    if res.is_none() {
        return Ok(None);
    }

    let parser = parser.into_inner();

    Ok(Some(parser))
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

    match args.command {
        Command::Create {
            files,
            root,
            out,
            compress,
            follow_links,
        } => {
            let output = out.expect("missing --output");
            let compress = compress.unwrap_or(false);
            let follow_links = follow_links.unwrap_or(false);
            let res = create_archive(files, root, output, compress, args.verbose, follow_links);
            if let Err(e) = res {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Command::List { files } => {
            let multiple = files.len() > 1;
            for file in files {
                if multiple {
                    println!("File {}:", file.display());
                }
                let res = list_files(&file);
                if let Err(e) = res {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
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
    verbose: bool,
    follow_links: bool,
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
        fn cmp(&self, other: &Self) -> core::cmp::Ordering {
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

    if !queue
        .peek()
        .map(|q| match q {
            QueueEntry::File(l) => l.as_os_str().is_empty(),
            QueueEntry::EndDir(l, _) => l.as_os_str().is_empty(),
        })
        .unwrap_or(false)
    {
        i += 1;
        files.push(initfs::FileHeader {
            inode: i,
            mode: 0x4000 | 0o777,
            uid: 0,
            gid: 0,
            name_len: 0,
            name_inline: [0; 32],
            compress_mode: 0,
            reserved: [0; 3],
            offset: 0,
            size: 0,
            compressed_size: 0,
        });
    }

    queue.push(QueueEntry::EndDir("".into(), 0));

    // TODO: push ancestors of each specified dir

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
        let stat = if follow_links {
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
                compress_mode = initfs::COMPRESS_LZ4;
            } else {
                open_file.read_to_end(&mut data)?;
                compress_mode = initfs::COMPRESS_NONE;
            }

            size = len as usize;
            compressed_size = data.len() - offset;
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
    if compress {
        println!(
            "Compressed {} data bytes to {} bytes ==> {:5.2}%",
            orig_size,
            total_size,
            compression_ratio * 100.0
        );
    } else {
        println!("Created uncompressed bundle of size {} bytes", total_size);
    }

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

fn list_files(file: &Path) -> Result<(), std::io::Error> {
    let file = std::fs::read(file)?;
    let archive = initfs::Archive::load(&file).unwrap();

    let width = (archive.header.file_count.max(1)).ilog10() as usize + 1;

    for file in archive.iter_files() {
        let name = archive
            .get_file_name(file)
            .map(|s| Cow::Borrowed(core::str::from_utf8(s).expect("TODO")))
            .unwrap_or_else(|| format!("<missing>: {:?}", file.file_name_trunc()).into());

        println!("| {:width$} {}", file.inode, name)
    }

    Ok(())
}
