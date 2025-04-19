use alloc::collections::btree_map::BTreeMap;
use alloc::sync::{Arc, Weak};
use alloc::vec::Vec;
use initfs::{Archive, ArchiveError};

use crate::arch::memory::palloc::{Size4KiB, PAGE_ALLOCATOR};
use crate::process::fd::{
    boxed_future, ArcFd, DirEntry, FileDescResult, FileDescriptor, FileKind, SmallFuture,
};
use crate::sync::SpinLock;

// // Self-referential owned archive
// struct OwnedArchive {
//     archive: Archive<'static>,
//     _data: Vec<u8>,
// }

// impl OwnedArchive {
//     fn new(data: Vec<u8>) -> Result<Self, ArchiveError> {
//         let archive = Archive::load(&*data)?;
//         Ok(OwnedArchive {
//             archive: unsafe { core::mem::transmute(archive) },
//             _data: data,
//         })
//     }
// }

// impl core::ops::Deref for OwnedArchive {
//     type Target = Archive<'static>;
//     fn deref(&self) -> &Self::Target {
//         &self.archive
//     }
// }

#[derive(PartialEq, Eq, Hash, PartialOrd, Ord)]
struct Inode(u64);

pub struct InitFs {
    this: Weak<InitFs>,
    inner: Archive<'static>,
    cache: SpinLock<BTreeMap<Inode, Weak<InitFsFile>>>,
}

impl InitFs {
    pub fn new(data: &'static [u8]) -> Result<alloc::sync::Arc<Self>, ArchiveError> {
        let inner = Archive::load(data)?;

        Ok(Arc::new_cyclic(|this| InitFs {
            this: this.clone(),
            inner,
            cache: SpinLock::new(BTreeMap::new()),
        }))
    }
    fn construct_inode(&self, num: u64) -> Option<Arc<InitFsFile>> {
        let hdr = self.inner.get_file(num as usize)?;
        Some(Arc::new(InitFsFile {
            fs: self.this.upgrade().unwrap(),
            inode: Inode(num),
            header: hdr.clone(),
            data: SpinLock::new(None),
        }))
    }
    fn get_inode(&self, num: u64) -> Option<Arc<InitFsFile>> {
        use alloc::collections::btree_map::Entry;
        let mut cache = self.cache.lock();
        match cache.entry(Inode(num)) {
            Entry::Occupied(mut slot) => {
                if let Some(file) = slot.get().upgrade() {
                    Some(file)
                } else {
                    let node = self.construct_inode(num)?;
                    slot.insert(Arc::downgrade(&node));
                    Some(node)
                }
            }
            Entry::Vacant(slot) => {
                let node = self.construct_inode(num)?;
                slot.insert(Arc::downgrade(&node));
                Some(node)
            }
        }
    }
    pub fn root(&self) -> ArcFd {
        self.get_inode(1).unwrap() as Arc<_>
    }
}

struct InitFsFile {
    fs: Arc<InitFs>,
    inode: Inode,
    header: initfs::FileHeader,
    // TODO: OnceCell
    data: SpinLock<Option<Vec<u8>>>,
}

impl FileDescriptor for InitFsFile {
    fn is_same_file(&self, other: &dyn FileDescriptor) -> bool {
        let Some(other) = other.as_any().downcast_ref::<Self>() else {
            return false;
        };
        self.inode == other.inode
    }
    fn kind(&self) -> FileKind {
        match (self.header.mode & 0xF000) >> 12 {
            4 => FileKind::Directory,
            8 => FileKind::Regular,
            10 => FileKind::SymbolicLink,
            _ => FileKind::Other,
        }
    }
    fn read<'a>(&'a self, offset: u64, buf: &'a mut [u8]) -> SmallFuture<'a, FileDescResult> {
        if self.header.is_dir() {
            // TODO: max filename length?
            const MAX_FILENAME: usize = 4096;
            const MAX_ENTRY_SIZE: u64 = (size_of::<DirEntry>() + MAX_FILENAME) as u64;

            let cookie = offset;
            let index = cookie / MAX_ENTRY_SIZE;
            let index = if index == 0 {
                self.inode.0 as u64 + 1
            } else {
                index
            };

            let list = self
                .fs
                .inner
                .list_dir_partial(self.inode.0 as usize, index as usize);

            match list {
                Err(_e) => boxed_future(async move { Err(1).into() }),
                Ok(list) => {
                    let mut cur_idx = 0;
                    let mut failed = false;
                    let mut list = list.peekable();
                    while let Some((file, next)) = list.next() {
                        let name_len: u16 = file.name_len.try_into().unwrap(); // TODO
                        let rec_len =
                            (size_of::<DirEntry>() as u16 - 3 + name_len).next_multiple_of(8);
                        if buf.len() - cur_idx < rec_len as usize {
                            // TODO: what if the buffer is too small for a single entry?
                            failed = true;
                            break;
                        }
                        let next = if list.peek().is_some() { next } else { 0 };
                        let record_start = DirEntry {
                            inode: file.inode as u64,
                            next_entry_cookie: MAX_ENTRY_SIZE * next as u64,
                            rec_len,
                            name_len,
                            file_type: (file.mode >> 12) as u8,
                            name: [0; 3],
                        };
                        let name = self.fs.inner.get_file_name(file).unwrap(); // TODO
                        assert_eq!(name.len(), name_len as usize); // TODO

                        let slice = &mut buf[cur_idx..][..rec_len as usize];
                        slice[..size_of::<DirEntry>()]
                            .copy_from_slice(bytemuck::bytes_of(&record_start));
                        slice[core::mem::offset_of!(DirEntry, name)..][..name_len as usize]
                            .copy_from_slice(name);
                        slice[core::mem::offset_of!(DirEntry, name) + name_len as usize..].fill(0);

                        cur_idx += rec_len as usize;
                    }
                    if cur_idx == 0 && failed {
                        boxed_future(async move { Err(1).into() })
                    } else {
                        boxed_future(async move { Ok(cur_idx as u64).into() })
                    }
                }
            }
        } else {
            let mut guard = self.data.lock();
            if guard.is_none() {
                let mut data = alloc::vec![0; self.header.size as usize];
                let res = self
                    .fs
                    .inner
                    .read_file(&self.header, &mut data)
                    .expect("TODO");
                assert_eq!(res.len(), self.header.size as usize);
                *guard = Some(data);
            }
            let data = guard.as_deref().unwrap();
            let data_start = offset as usize;
            let data_end = (offset as usize + buf.len()).min(data.len());
            let buf_end = data_end.saturating_sub(data_start);
            if data_end > data_start {
                buf[..buf_end].copy_from_slice(&data[data_start..data_end]);
            }
            drop(guard);
            boxed_future(async move { Ok(buf_end as u64).into() })
        }
    }
    fn write<'a>(&'a self, _offset: u64, _buf: &'a [u8]) -> SmallFuture<'a, FileDescResult> {
        boxed_future(async move { Err(1u64).into() })
    }
    fn size<'a>(&'a self) -> SmallFuture<'a, FileDescResult> {
        let size = self.header.size;
        boxed_future(async move { Ok(size as u64).into() })
    }
    fn open<'a>(&'a self, name: &'a [u8]) -> SmallFuture<'a, Result<ArcFd, ()>> {
        // println!("Opening {:?}", core::str::from_utf8(name).unwrap());
        if self.header.is_dir() {
            let cur_name = self.fs.inner.get_file_name(&self.header).unwrap();
            // println!("cur dir {:?}", core::str::from_utf8(cur_name).unwrap());
            let pfx_len = if cur_name.is_empty() {
                0
            } else {
                cur_name.len() + 1
            };
            let files = self.fs.inner.list_dir(self.inode.0 as usize).expect("TODO");
            for (inode, file) in files {
                if file.name_len as usize == pfx_len + name.len()
                    && self.fs.inner.get_file_name(file).map(|b| &b[pfx_len..]) == Some(name)
                {
                    // println!("found {:?}", self.fs.inner.get_file_name(file).map(|i| core::str::from_utf8(i).unwrap()));
                    return boxed_future(async move {
                        self.fs
                            .get_inode(inode as u64)
                            .ok_or(())
                            .map(|f| f as ArcFd)
                    });
                }
            }
            boxed_future(async move { Err(()) })
        } else {
            boxed_future(async move { Err(()) })
        }
    }
    fn mmap_page(&self, _offset: u64) -> SmallFuture<Option<FileDescResult>> {
        if self.header.is_dir() {
            return boxed_future(async move { None });
        }

        //File case, need to call read
        boxed_future(async move {
            let page = PAGE_ALLOCATOR.get().alloc_mapped_frame::<Size4KiB>();
            let page_paddr = page.paddr;
            let page_virt = PAGE_ALLOCATOR.get().get_mapped_frame::<Size4KiB>(page);
            let buf_ref = unsafe { core::slice::from_raw_parts_mut(page_virt as *mut u8, 4096) };
            match self.read(_offset, buf_ref).await.as_result() {
                Ok(_val) => {
                    return Some(FileDescResult::ok(page_paddr as u64));
                }
                Err(_val) => {
                    println!("Read failed");
                    return None;
                }
            }
        })
    }
    fn as_any(&self) -> &dyn core::any::Any {
        self
    }
}
