#![no_std]

pub const MAGIC: [u8; 4] = *b"\x00ARC";

pub const ENDIAN_LE: u8 = 0;
pub const ENDIAN_BE: u8 = 1;
pub const ENDIAN_NATIVE: u8 = if cfg!(target_endian = "little") {
    ENDIAN_LE
} else {
    ENDIAN_BE
};

pub const COMPRESS_NONE: u8 = 0;
pub const COMPRESS_LZ4: u8 = 1;

#[repr(C)]
pub struct ArchiveHeader {
    pub magic: [u8; 4],   // magic (TODO: what value?)
    pub endian: u8,       // 0 for LE, 1 for BE
    pub version: u8,      // version = 1
    pub header_size: u16, // offset to start of file list
    pub total_size: u32,  // total size of the archive
    pub file_count: u32,
    pub file_stride: u32,
    pub strings_size: u32,
}

#[repr(C)]
#[derive(Clone)]
pub struct FileHeader {
    // must be the 1-indexed location of this file within the archive's
    // file list.  (TODO: indices in concatenated archives?)
    pub inode: u32,
    pub mode: u32,
    pub uid: u16,
    pub gid: u16,

    pub name_len: u32,
    // if name_len > 32, this is instead the first 28 bytes of the name,
    // followed by the offset into the name data of the full name.
    // if name_len < 32, the remaining bytes are 0
    pub name_inline: [u8; 32],

    pub compress_mode: u8, // 0 = uncompressed, 1 = LZ4
    pub reserved: [u8; 3],

    // offset of the data from the start of the header
    // if this is a directory, this is instead the number of entries
    // between this entry and the end of the directory
    pub offset: u32,
    // if the entry is a file: the size of the file when uncompressed
    // if the entry is a dir: 0
    pub size: u32,
    // if the entry is a file: the size of the file data stored on disk
    // if the entry is a dir: 0
    pub compressed_size: u32,
}

// [Archive header]
// [file list]
//   (sorted flat list of file metadata?)
//   (full paths, or just names + start/end dir?)
//   (inline strings or string table?)
//   include offset to file start, file size (compressed and uncompressed)
// [/file list]
// [name data]
// ...
// [/name data]
// [file data]
// ...
// [/file data]
// (optional concatenated archives?)

pub struct Archive<'a> {
    pub header: &'a ArchiveHeader,

    strings: &'a [u8],
    data: &'a [u8],

    files_start: usize,
    files_count: usize,
    files_stride: usize,
}

#[derive(Debug)]
pub enum ArchiveError {
    UnalignedInput,
    MagicMismatch,
    InvalidHeader(HeaderError),
    Truncated,
}

#[derive(Debug)]
pub enum HeaderError {
    InvalidSizes,
    UnsupportedEndianness(u8),
    UnsupportedVersion(u8),
}

#[derive(Debug)]
pub enum ReadError {
    BufferTooSmall,
    InvalidFileOffset,
    Lz4Error(lz4::frame::Lz4Error),
    FileSizeMismatch,
    UnsupportedCompressionMode(u8),
    NotADirectory,
}

impl<'a> Archive<'a> {
    pub fn load(data: &'a [u8]) -> Result<Self, ArchiveError> {
        let header = archive_header_from_slice(data).ok_or(ArchiveError::UnalignedInput)?;

        if header.magic != MAGIC {
            return Err(ArchiveError::MagicMismatch);
        }
        if header.version != 1 {
            return Err(ArchiveError::InvalidHeader(
                HeaderError::UnsupportedVersion(header.version),
            ));
        }
        if header.endian != ENDIAN_NATIVE {
            return Err(ArchiveError::InvalidHeader(
                HeaderError::UnsupportedEndianness(header.endian),
            ));
        }

        let checks = header.header_size % 4 == 0
            && header.header_size as usize <= size_of::<ArchiveHeader>()
            && header.file_stride % 4 == 0
            && header.file_stride as usize <= size_of::<FileHeader>();

        if !checks {
            return Err(ArchiveError::InvalidHeader(HeaderError::InvalidSizes));
        }

        if header.total_size as usize > data.len() {
            return Err(ArchiveError::Truncated);
        }

        let files_start = header.header_size as usize;
        let files_count = header.file_count as usize;
        let files_stride = header.file_stride as usize;

        let strings_start = files_start + files_count * files_stride;
        let strings_end = strings_start + header.strings_size as usize;
        let strings = data
            .get(strings_start..strings_end)
            .ok_or(ArchiveError::Truncated)?;

        Ok(Archive {
            header,
            strings,
            data,
            files_start,
            files_count,
            files_stride,
        })
    }

    pub fn get_file(&self, inode: usize) -> Option<&'a FileHeader> {
        if inode == 0 || inode > self.files_count {
            return None;
        }
        let base = self.files_start + (inode - 1) * self.files_stride;
        file_header_from_slice(&self.data[base..])
    }

    pub fn get_file_name(&self, file: &'a FileHeader) -> Option<&'a [u8]> {
        match file.name_len as usize {
            len @ 0..=32 => Some(&file.name_inline[..len]),
            len @ 33.. => {
                let offset_bytes = file.name_inline[28..].try_into().unwrap();
                let str_start = u32::from_le_bytes(offset_bytes) as usize;
                let str_end = str_start.checked_add(len)?;
                self.strings.get(str_start..str_end)
            }
        }
    }

    pub fn iter_files(&self) -> impl Iterator<Item = &FileHeader> {
        (1..=self.files_count).filter_map(|i| self.get_file(i))
    }

    pub fn find_file(&self, name: &[u8]) -> Option<(usize, &FileHeader)> {
        // TODO: a binary search may be better, but this approach is simple
        let target = name;
        let mut i = 1;
        while i <= self.files_count {
            let file = self.get_file(i).unwrap();
            let trunc = file.file_name_trunc();
            let (file_name_trunc, is_trunc) = match trunc {
                Ok(n) => (n, false),
                Err(n) => (n, true),
            };

            if target.starts_with(file_name_trunc) {
                if !is_trunc && target.len() == file_name_trunc.len() {
                    return Some((i, file));
                } else {
                    let file_name = self.get_file_name(file).unwrap();
                    if target == file_name {
                        return Some((i, file));
                    }
                }
            } else if target < file_name_trunc {
                return None;
            } else if file.is_dir() {
                i += file.offset as usize;
                continue;
            }
            i += 1;
        }
        None
    }

    pub fn list_dir(
        &self,
        idx: usize,
    ) -> Result<impl Iterator<Item = (usize, &FileHeader)>, ReadError> {
        self.list_dir_partial(idx, idx + 1)
            .map(|r| r.map(|(hdr, _next)| (hdr.inode as usize, hdr)))
    }

    pub fn list_dir_partial(
        &self,
        idx: usize,
        start: usize,
    ) -> Result<impl Iterator<Item = (&FileHeader, usize)>, ReadError> {
        let dir = self.get_file(idx).unwrap(); // TODO: don't unwrap
        if !dir.is_dir() {
            return Err(ReadError::NotADirectory);
        }

        let mut i = start;
        let end = (self.files_count + 1).min(idx + dir.offset as usize);
        Ok(core::iter::from_fn(move || {
            if i >= end {
                return None;
            }
            let file = self.get_file(i)?;
            if file.is_dir() {
                i += file.offset as usize;
            } else {
                i += 1;
            }
            Some((file, i))
        }))
    }

    pub fn raw_file_data(&self, file: &FileHeader) -> Result<&'a [u8], ReadError> {
        if file.compressed_size == 0 {
            Ok(&[])
        } else {
            self.data
                .get(file.offset as usize..)
                .and_then(|s| s.get(..file.compressed_size as usize))
                .ok_or(ReadError::InvalidFileOffset)
        }
    }

    pub fn read_file<'o>(
        &self,
        file: &FileHeader,
        buf: &'o mut [u8],
    ) -> Result<&'o [u8], ReadError> {
        let out_buf = buf
            .get_mut(..file.size as usize)
            .ok_or(ReadError::BufferTooSmall)?;

        match file.compress_mode {
            self::COMPRESS_NONE => {
                let data = self.raw_file_data(file)?;
                let data_slice = data
                    .get(..file.size as usize)
                    .ok_or(ReadError::FileSizeMismatch)?;
                out_buf.copy_from_slice(data_slice);
                Ok(&*out_buf)
            }
            self::COMPRESS_LZ4 => {
                let data = self.raw_file_data(file)?;
                let res = lz4::decode_into(data, out_buf).map_err(ReadError::Lz4Error)?;

                if res.len() != file.size as usize {
                    return Err(ReadError::FileSizeMismatch);
                }

                Ok(res)
            }
            mode => Err(ReadError::UnsupportedCompressionMode(mode)),
        }
    }
}

impl FileHeader {
    pub fn file_name_trunc(&self) -> Result<&[u8], &[u8]> {
        match self.name_len as usize {
            len @ 0..=32 => Ok(&self.name_inline[..len]),
            _ => Err(&self.name_inline[..28]),
        }
    }
    pub fn is_dir(&self) -> bool {
        ((self.mode & 0xF000) >> 12) == 4
    }
}

pub fn archive_header_from_slice(slice: &[u8]) -> Option<&ArchiveHeader> {
    let ptr = slice.as_ptr().cast::<ArchiveHeader>();
    let valid = slice.len() >= size_of::<ArchiveHeader>() && ptr.is_aligned();
    valid.then(|| unsafe { &*ptr })
}

pub fn file_header_from_slice(slice: &[u8]) -> Option<&FileHeader> {
    let ptr = slice.as_ptr().cast::<FileHeader>();
    let valid = slice.len() >= size_of::<FileHeader>() && ptr.is_aligned();
    valid.then(|| unsafe { &*ptr })
}

pub struct SpanStack<const N: usize, T> {
    vals: [T; N],
    size: usize,
}

impl<const N: usize, T: Default + PartialEq> SpanStack<N, T> {
    pub fn new() -> Self {
        Self {
            vals: core::array::from_fn(|_| Default::default()),
            size: 0,
        }
    }
    pub fn push(&mut self, val: T) {
        if self.size >= N {
            return;
        }
        self.vals[self.size] = val;
        self.size += 1;
    }
    pub fn pop_while_eq(&mut self, val: &T) {
        while self.size > 0 && &self.vals[self.size - 1] == val {
            self.size -= 1;
        }
    }
    pub fn len(&self) -> usize {
        self.size
    }
}

impl<const N: usize, T: core::fmt::Debug> core::fmt::Debug for SpanStack<N, T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_list().entries(&self.vals[..self.size]).finish()
    }
}

#[macro_use]
#[doc(hidden)]
pub mod macros {
    #[repr(C)]
    pub struct AlignedAs<Align, Bytes: ?Sized> {
        pub _align: [Align; 0],
        pub bytes: Bytes,
    }
    #[doc(hidden)]
    #[macro_export]
    macro_rules! __include_bytes_align {
        ($align_ty:ty, $path:literal) => {{
            use $crate::macros::AlignedAs;
            static ALIGNED: &AlignedAs<$align_ty, [u8]> = &AlignedAs {
                _align: [],
                bytes: *include_bytes!($path),
            };
            &ALIGNED.bytes
        }};
    }
}

#[doc(inline)]
pub use crate::__include_bytes_align as include_bytes_align;
