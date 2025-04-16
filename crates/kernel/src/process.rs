use alloc::sync::Arc;
use alloc::vec::Vec;

use crate::sync::once_cell::BlockingOnceCell;
use crate::sync::SpinLock;

pub mod fd;
pub mod mem;

pub type ProcessRef = Arc<Process>;

pub struct FileDescriptorList {
    pub desc: Vec<Option<fd::ArcFd>>,
}

pub struct ExitStatus {
    pub status: u32,
}

pub struct Process {
    pub mem: SpinLock<mem::UserAddrSpace>,
    pub root: Option<fd::ArcFd>,
    pub current_dir: SpinLock<usize>,
    pub file_descriptors: SpinLock<FileDescriptorList>,
    pub exit_code: Arc<BlockingOnceCell<ExitStatus>>,
}

impl Process {
    pub fn new() -> Self {
        let mem = mem::UserAddrSpace::new();

        Process {
            mem: SpinLock::new(mem),
            root: None,
            current_dir: SpinLock::new(0),
            file_descriptors: SpinLock::new(FileDescriptorList { desc: Vec::new() }),
            exit_code: Arc::new(BlockingOnceCell::new()),
        }
    }

    pub fn get_ttbr0(&self) -> usize {
        self.mem.lock().get_ttbr0()
    }

    pub async fn fork(&self) -> Process {
        let new_mem = self.mem.lock().fork().await;

        let mut new_fds = FileDescriptorList { desc: Vec::new() };
        {
            let old_fds = self.file_descriptors.lock();
            for (idx, desc) in old_fds
                .desc
                .iter()
                .enumerate()
                .filter_map(|(idx, desc)| Some((idx, desc.as_ref()?)))
            {
                let _ = new_fds.set(idx, desc.clone());
            }
        }

        let cur_fd = self.current_dir.lock().clone();
        let new_process = Process {
            mem: SpinLock::new(new_mem),
            root: self.root.clone(),
            current_dir: SpinLock::new(cur_fd),
            file_descriptors: SpinLock::new(new_fds),
            exit_code: Arc::new(BlockingOnceCell::new()),
        };

        new_process
    }
}

impl FileDescriptorList {
    pub fn get(&self, idx: usize) -> Option<&fd::ArcFd> {
        self.desc.get(idx).and_then(|s| s.as_ref())
    }
    #[must_use]
    pub fn set(&mut self, idx: usize, descriptor: fd::ArcFd) -> Option<fd::ArcFd> {
        match self.desc.get_mut(idx) {
            Some(slot) => slot.replace(descriptor),
            None => {
                // TODO: this is an easy DOS vector
                self.desc.resize(idx + 1, None);
                self.desc[idx] = Some(descriptor);
                None
            }
        }
    }
    pub fn insert(&mut self, descriptor: fd::ArcFd) -> usize {
        for (i, slot) in self.desc.iter_mut().enumerate() {
            if slot.is_none() {
                *slot = Some(descriptor);
                return i;
            }
        }
        let idx = self.desc.len();
        self.desc.push(Some(descriptor));
        idx
    }
    #[must_use]
    pub fn remove(&mut self, idx: usize) -> Option<fd::ArcFd> {
        match self.desc.get_mut(idx) {
            Some(slot) => slot.take(),
            None => None,
        }
    }
}
