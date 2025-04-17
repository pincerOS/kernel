use core::cell::RefCell;

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

#[derive(Clone)]
pub struct Credential {
    /// Real UID: the UID of the user who started the process
    pub ruid: u32,
    /// Real GID: the GID of the user who started the process
    pub rgid: u32,
    /// Saved UID: the prior UID that the process was running as (for future permission raising)
    pub suid: u32,
    /// Saved GID: the prior GID that the process was running as (for future permission raising)
    pub sgid: u32,
    /// Effective UID: the UID that the process is running as
    pub euid: u32,
    /// Effective GID: the GID that the process is running as
    pub egid: u32,
    // todo: capabilities
}

impl Credential {
    pub fn new() -> Self {
        Credential {
            ruid: 0,
            rgid: 0,
            suid: 0,
            sgid: 0,
            euid: 0,
            egid: 0,
        }
    }

    pub fn try_set_reuid(&mut self, ruid: Option<u32>, euid: Option<u32>) -> Result<(), ()> {
        let executing_euid = self.euid;

        if let Some(euid) = euid {
            // Unprivileged processes may only set the **effective** user ID to the
            // real user ID, the effective user ID, or the saved set-user-ID.
            if executing_euid == 0 || [self.ruid, self.euid, self.suid].contains(&euid) {
                self.euid = euid;

                if self.euid != self.ruid {
                    self.suid = self.euid;
                }
            } else {
                return Err(());
            }
        }

        if let Some(ruid) = ruid {
            // Unprivileged users may only set the **real** user ID to the real user
            // ID or the effective user ID.
            if executing_euid == 0 || [self.ruid, executing_euid].contains(&ruid) {
                self.ruid = ruid;
                // Save the effective user id
                self.suid = self.euid;
            } else {
                return Err(());
            }
        }

        Ok(())
    }
}

pub struct Process {
    pub mem: SpinLock<mem::UserAddrSpace>,
    pub root: Option<fd::ArcFd>,
    pub file_descriptors: SpinLock<FileDescriptorList>,
    pub exit_code: Arc<BlockingOnceCell<ExitStatus>>,
    pub credential: SpinLock<Credential>,
}

impl Process {
    pub fn new() -> Self {
        let mem = mem::UserAddrSpace::new();

        Process {
            mem: SpinLock::new(mem),
            root: None,
            file_descriptors: SpinLock::new(FileDescriptorList { desc: Vec::new() }),
            exit_code: Arc::new(BlockingOnceCell::new()),
            credential: SpinLock::new(Credential::new()),
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

        let creds = self.credential.lock();

        let new_process = Process {
            mem: SpinLock::new(new_mem),
            root: self.root.clone(),
            file_descriptors: SpinLock::new(new_fds),
            exit_code: Arc::new(BlockingOnceCell::new()),
            credential: SpinLock::new(creds.clone()),
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
