use core::any::Any;
use core::future::Future;

use alloc::sync::Arc;

pub type ArcFd = Arc<dyn FileDescriptor + Send + Sync>;

pub use smallbox::SmallBox;

use crate::sync;
pub type SmallFuture<'a, Out> = SmallBox<dyn Future<Output = Out> + Send + 'a, smallbox::space::S4>;
pub type SmallFutureOwned<Out> = SmallBox<dyn Future<Output = Out> + Send, smallbox::space::S4>;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum FdAccessMode {
    Read,
    Write,
    Exec,
}

pub fn boxed_future<'a, F, Out>(f: F) -> SmallFuture<'a, Out>
where
    F: Future<Output = Out> + Send + 'a,
{
    // Making this convenient requires CoerceUnsized (and smallbox's "coerce" feature)
    // but they provide a macro that works on stable.
    smallbox::smallbox!(f)
}

pub struct FileDescResult(pub i64);

impl FileDescResult {
    pub fn from_result(res: Result<u64, u64>) -> Self {
        match res {
            Ok(v) => Self::ok(v),
            Err(v) => Self::err(v),
        }
    }
    pub fn as_result(self) -> Result<u64, u64> {
        if self.0 < 0 {
            Err(self.0.unsigned_abs())
        } else {
            Ok(self.0 as u64)
        }
    }
    pub fn ok(v: u64) -> Self {
        assert!(v < (1 << 63));
        FileDescResult(v as i64)
    }
    pub fn err(v: u64) -> Self {
        assert!(v > 0 && v <= (1 << 63));
        FileDescResult(-(v as i64))
    }
}

impl From<Result<u64, u64>> for FileDescResult {
    fn from(value: Result<u64, u64>) -> Self {
        Self::from_result(value)
    }
}

pub trait FileDescriptor: Any {
    fn is_same_file(&self, other: &dyn FileDescriptor) -> bool;
    fn kind(&self) -> FileKind;
    fn read<'a>(&'a self, offset: u64, buf: &'a mut [u8]) -> SmallFuture<'a, FileDescResult>;
    fn write<'a>(&'a self, offset: u64, buf: &'a [u8]) -> SmallFuture<'a, FileDescResult>;
    fn size<'a>(&'a self) -> SmallFuture<'a, FileDescResult>;
    fn mmap_page(&self, offset: u64) -> SmallFuture<Option<FileDescResult>>;

    fn open<'a>(&'a self, name: &'a [u8]) -> SmallFuture<'a, Result<ArcFd, ()>> {
        let _ = name;
        boxed_future(async move { Err(()).into() })
    }

    // Permission checking: check if a credential may access this file
    /// Check whether the given credentials have the specified access to this file
    fn can_access(&self, _cred: &crate::process::Credential, _mode: FdAccessMode) -> bool {
        // By default, allow all access; filesystems may override
        true
    }
    // TODO: unneeded after rust 1.86 by trait upcasting
    fn as_any(&self) -> &dyn Any;
}

#[derive(Copy, Clone, PartialEq)]
pub enum FileKind {
    Directory,
    Regular,
    SymbolicLink,
    Other,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DirEntry {
    pub inode: u64,
    pub next_entry_cookie: u64,
    pub rec_len: u16,
    pub name_len: u16,
    pub file_type: u8,
    pub name: [u8; 3],
    // Name is an arbitrary size array; the record is always padded with
    // 0 bytes such that rec_len is a multiple of 8 bytes.
}

unsafe impl bytemuck::Zeroable for DirEntry {}
unsafe impl bytemuck::Pod for DirEntry {}

pub struct DummyFd;

impl FileDescriptor for DummyFd {
    fn is_same_file(&self, other: &dyn FileDescriptor) -> bool {
        other.as_any().is::<Self>()
    }
    fn kind(&self) -> FileKind {
        FileKind::Other
    }
    fn read(&self, _offset: u64, _buf: &mut [u8]) -> SmallFuture<FileDescResult> {
        boxed_future(async move { Ok(0u64).into() })
    }
    fn write(&self, _offset: u64, _buf: &[u8]) -> SmallFuture<FileDescResult> {
        boxed_future(async move { Err(1u64).into() })
    }
    fn size<'a>(&'a self) -> SmallFuture<'a, FileDescResult> {
        boxed_future(async move { Ok(0u64).into() })
    }
    fn mmap_page(&self, _offset: u64) -> SmallFuture<Option<FileDescResult>> {
        boxed_future(async move { None })
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

pub struct UartFd(pub &'static crate::device::uart::UARTLock);

const READ_NO_BLOCK: bool = true;

// TODO: how to handle non-zero offsets for non-seekable files?
impl FileDescriptor for UartFd {
    fn is_same_file(&self, other: &dyn FileDescriptor) -> bool {
        let Some(other) = other.as_any().downcast_ref::<Self>() else {
            return false;
        };
        core::ptr::eq(self, other)
    }
    fn kind(&self) -> FileKind {
        FileKind::Other
    }
    fn read<'a>(&'a self, _offset: u64, buf: &'a mut [u8]) -> SmallFuture<'a, FileDescResult> {
        if buf.is_empty() {
            return boxed_future(async move { FileDescResult::ok(0) });
        }
        let target = &mut buf[0];
        boxed_future(async move {
            if READ_NO_BLOCK {
                let c = self.0.lock().try_getc();
                if let Some(c) = c {
                    *target = c;
                    FileDescResult::ok(1)
                } else {
                    // TODO: proper non-blocking reads, or proper kernel heap...
                    sync::time::sleep(100).await;
                    FileDescResult::ok(0)
                }
            } else {
                // TODO: async UART handling
                let c = self.0.lock().getc();
                *target = c;
                FileDescResult::ok(1)
            }
        })
    }
    fn write<'a>(&'a self, _offset: u64, buf: &'a [u8]) -> SmallFuture<'a, FileDescResult> {
        if buf.is_empty() {
            return boxed_future(async move { FileDescResult::ok(0) });
        }
        boxed_future(async move {
            // TODO: async UART handling
            let v = buf[0];
            self.0.lock().writec(v);
            FileDescResult::ok(1)
        })
    }
    fn size<'a>(&'a self) -> SmallFuture<'a, FileDescResult> {
        boxed_future(async move { Ok(0u64).into() })
    }
    fn mmap_page(&self, _offset: u64) -> SmallFuture<Option<FileDescResult>> {
        boxed_future(async move { None })
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

pub async fn read_all(fd: &(dyn FileDescriptor + Send + Sync)) -> Result<alloc::vec::Vec<u8>, ()> {
    // TODO: specify limits
    let size = fd.size().await.as_result().map_err(|_e| ())?;
    assert!(usize::try_from(size).is_ok());
    let mut file_data = alloc::vec![0; size as usize];
    let mut read = 0;
    while read < size {
        match fd
            .read(read, &mut file_data[read as usize..])
            .await
            .as_result()
        {
            Ok(0) => return Err(()),
            Ok(s) => read += s,
            Err(_e) => return Err(()),
        }
    }
    Ok(file_data)
}
