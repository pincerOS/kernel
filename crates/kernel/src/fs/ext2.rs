#![allow(dead_code, nonstandard_style)]

use crate::process;

use filesystem::{*};
use alloc::sync::{Arc, Weak};
use process::fd::{FileDescriptor, boxed_future};

use crate::{device::sdcard::bcm2711_emmc2_driver, sync::SpinLock};

pub struct Ext2File {
    pub ext2: Arc<SpinLock<Ext2<bcm2711_emmc2_driver>>>,
    pub inode_wrapper: Arc<SpinLock<INodeWrapper>>,
}

impl FileDescriptor for Ext2File {
    fn is_same_file(&self, other: &dyn FileDescriptor) -> bool {
        if let Some(other) = other.as_any().downcast_ref::<Ext2File>() {
            // Note: removed the Arc::ptr_eq check for ext2, lmk if this is a problem
            let self_inode_num = self.inode_wrapper.lock()._inode_num;
            let other_inode_num = other.inode_wrapper.lock()._inode_num;
            
            self_inode_num == other_inode_num
        } else {
            false
        }
    }

    fn kind(&self) -> crate::process::fd::FileKind {
        match self.inode_wrapper.lock().inode.i_mode & 0xF000 {
            i_mode::EXT2_S_IFDIR => crate::process::fd::FileKind::Directory,
            i_mode::EXT2_S_IFREG => crate::process::fd::FileKind::Regular,
            i_mode::EXT2_S_IFLNK => crate::process::fd::FileKind::SymbolicLink,
            _ => crate::process::fd::FileKind::Other,
        }
        
    }

    fn read<'a>(&'a self, offset: u64, buf: &'a mut [u8]) -> crate::process::fd::SmallFuture<'a, crate::process::fd::FileDescResult> {
        if self.inode_wrapper.lock().is_dir() {
            todo!()
        } else {
            todo!()
        }
    }

    fn write<'a>(&'a self, offset: u64, buf: &'a [u8]) -> crate::process::fd::SmallFuture<'a, crate::process::fd::FileDescResult> {
        todo!()
    }

    fn size<'a>(&'a self) -> crate::process::fd::SmallFuture<'a, crate::process::fd::FileDescResult> {
        let size = self.inode_wrapper.lock().size();
        crate::process::fd::boxed_future(async move { Ok(size).into() })
    }

    fn mmap_page(&self, offset: u64) -> crate::process::fd::SmallFuture<Option<crate::process::fd::FileDescResult>> {
        todo!()
    }

    fn as_any(&self) -> &dyn core::any::Any {
        self
    }
}