use alloc::borrow::Cow;

use crate::event::async_handler::{run_async_handler, run_event_handler, HandlerContext};
use crate::event::context::Context;
use crate::process::fd::{ArcFd, FdAccessMode, FileKind};
use crate::process::Credential;

bitflags::bitflags! {
    struct DupFlags: u32 {
    }
}

/// syscall dup3(old_fd: u32, new_fd: u32, flags: DupFlags) -> i64
pub unsafe fn sys_dup3(ctx: &mut Context) -> *mut Context {
    let old_fd = ctx.regs[0];
    let new_fd = ctx.regs[1];
    let flags = ctx.regs[2];

    let Some(_flags) = u32::try_from(flags).ok().and_then(DupFlags::from_bits) else {
        ctx.regs[0] = -1i64 as usize;
        return ctx;
    };

    run_event_handler(ctx, move |mut context: HandlerContext<'_>| {
        // TODO: avoid cloning process?  (Partial borrows?)  (get thread directly, then partial)
        let proc = context.cur_process().unwrap().clone();

        let mut guard = proc.file_descriptors.lock();
        let Some(old) = guard.get(old_fd).cloned() else {
            context.regs().regs[0] = -1i64 as usize;
            return context.resume_final();
        };

        let to_close = guard.set(new_fd, old);

        if let Some(desc) = to_close {
            // TODO: we should be careful about where/when fd destructors are run
            drop(desc);
        }

        context.regs().regs[0] = new_fd;
        context.resume_final()
    })
}

/// syscall close(fd: u32) -> i64
pub unsafe fn sys_close(ctx: &mut Context) -> *mut Context {
    let fd = ctx.regs[0];

    run_event_handler(ctx, move |context: HandlerContext<'_>| {
        let proc = context.cur_process().unwrap();

        let removed = proc.file_descriptors.lock().remove(fd);
        if let Some(desc) = removed {
            // TODO: we should be careful about where/when fd destructors are run
            drop(desc);
            context.resume_return(0)
        } else {
            context.resume_return(-1i64 as usize)
        }
    })
}

/// syscall pread(fd: u32, buf: *mut u8, len: u64, offset: u64) -> i64
pub unsafe fn sys_pread(ctx: &mut Context) -> *mut Context {
    let fd = ctx.regs[0].min(u32::MAX as usize) as u32;
    let buf_ptr = ctx.regs[1];
    let buf_len = ctx.regs[2].min(u32::MAX as usize) as u32;
    let offset = ctx.regs[3];
    struct Args {
        fd: u32,
        buf_ptr: usize,
        buf_len: u32,
        offset: usize,
    }

    // Hack to work around Rust's terrible async codegen
    // Hopefully this improves in the future: https://github.com/rust-lang/rust/pull/135527
    let args = Args {
        fd,
        buf_ptr,
        buf_len,
        offset,
    };

    run_async_handler(ctx, async move |mut context: HandlerContext<'_>| {
        let args = args;
        let Args {
            fd,
            buf_ptr,
            buf_len,
            offset,
        } = args;
        let (file, buf) = {
            let proc = context.cur_process().unwrap();
            let file = proc.file_descriptors.lock().get(fd as usize).cloned();
            let Some(file) = file else {
                return context.resume_return(-1i64 as usize);
            };

            // TODO: sound abstraction for usermode buffers...
            // (prevent TOCTOU issues, pin pages to prevent user unmapping them,
            // deal with unmapped pages...)
            // TODO: check user buffers
            let buf =
                unsafe { core::slice::from_raw_parts_mut(buf_ptr as *mut u8, buf_len as usize) };
            (file, buf)
        };

        let res = file.read(offset as u64, buf).await;

        context.regs().regs[0] = res.0 as usize;
        context.resume_final()
    })
}

/// syscall pwrite(fd: u32, buf: *const u8, len: u64, offset: u64) -> i64
pub unsafe fn sys_pwrite(ctx: &mut Context) -> *mut Context {
    let fd = ctx.regs[0];
    let buf_ptr = ctx.regs[1];
    let buf_len = ctx.regs[2];
    let offset = ctx.regs[3];

    run_async_handler(ctx, async move |context: HandlerContext<'_>| {
        let proc = context.cur_process().unwrap();

        let file = proc.file_descriptors.lock().get(fd).cloned();
        let Some(file) = file else {
            return context.resume_return(-1i64 as usize);
        };

        // TODO: sound abstraction for usermode buffers...
        // (prevent TOCTOU issues, pin pages to prevent user unmapping them,
        // deal with unmapped pages...)
        // TODO: check user buffers
        let buf = unsafe { core::slice::from_raw_parts(buf_ptr as *const u8, buf_len) };

        let res = file.write(offset as u64, buf).await;

        context.resume_return(res.0 as usize)
    })
}

bitflags::bitflags! {
    struct OpenFlags: u32 {
    }
    struct OpenMode: u32 {
    }
}

// TODO: impl this like openat2? (man openat2(2))
// (pass a struct, and a size for vesioning?)

struct OpenAtArgs {
    dir_fd: usize,
    path_len: usize,
    path_ptr: *const u8,
    _flags: OpenFlags,
    _mode: OpenMode,
}
unsafe impl Send for OpenAtArgs {}

/// syscall openat(
///     dir_fd: usize,
///     path_len: usize,
///     path_ptr: *const u8,
///     flags: OpenFlags,
///     mode: OpenMode,
/// ) -> i64
pub unsafe fn sys_openat(ctx: &mut Context) -> *mut Context {
    let dir_fd = ctx.regs[0];
    let path_len = ctx.regs[1];
    let path_ptr = ctx.regs[2] as *const u8;
    let flags = ctx.regs[3];
    let mode = ctx.regs[4];

    let Some(flags) = u32::try_from(flags).ok().and_then(OpenFlags::from_bits) else {
        ctx.regs[0] = -1i64 as usize;
        return ctx;
    };
    let Some(mode) = u32::try_from(mode).ok().and_then(OpenMode::from_bits) else {
        ctx.regs[0] = -1i64 as usize;
        return ctx;
    };

    let arg_data = OpenAtArgs {
        dir_fd,
        path_len,
        path_ptr,
        _flags: flags,
        _mode: mode,
    };

    run_async_handler(ctx, async move |context: HandlerContext<'_>| {
        let proc = context.cur_process().unwrap();

        let dir = proc.file_descriptors.lock().get(arg_data.dir_fd).cloned();
        let Some(dir) = dir else {
            return context.resume_return(-1i64 as usize);
        };

        let path = context.with_user_vmem(move || {
            let arg_data = &arg_data;
            // TODO: soundness, check user args
            let path = unsafe { core::slice::from_raw_parts(arg_data.path_ptr, arg_data.path_len) };
            alloc::vec::Vec::from(path)
        });

        // TODO: file creation?
        let cred = proc.credential.lock();
        let new_fd = resolve_path(proc.root.as_ref(), dir, &path, &cred).await;
        drop(cred);
        let new_fd = match new_fd {
            Ok(f) => f,
            Err(_e) => return context.resume_return(-1i64 as usize),
        };

        // Permission check: ensure process credentials allow opening this file
        {
            let cred = proc.credential.lock();
            // Here, we assume read access is requested; adjust AccessMode when write flags are supported
            if !new_fd.can_access(&*cred, crate::process::fd::FdAccessMode::Read) {
                drop(cred);
                return context.resume_return(-1i64 as usize);
            }
        }

        // TODO: close on exec, etc?
        let fd_idx = proc.file_descriptors.lock().insert(new_fd);
        context.resume_return(fd_idx)
    })
}

enum PathSegment<'a> {
    RootDir,
    CurDir,
    ParentDir,
    Normal(&'a [u8]),
    Final(&'a [u8]),
}

fn split_slash(path: &[u8]) -> (&[u8], &[u8], bool) {
    match path.iter().position(|b| *b == b'/') {
        Some(first_slash) => (&path[..first_slash], &path[first_slash + 1..], false),
        None => (path, &path[path.len()..], true),
    }
}

fn skip_slashes(path: &[u8]) -> &[u8] {
    let first_non_slash = path.iter().position(|b| *b != b'/');
    &path[first_non_slash.unwrap_or(path.len())..]
}

fn segments(path: &[u8]) -> SegmentIter<'_> {
    SegmentIter { path }
}

struct SegmentIter<'a> {
    path: &'a [u8],
}

impl<'a> Iterator for SegmentIter<'a> {
    type Item = PathSegment<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.path.is_empty() {
            return None;
        }
        let (seg, is_final);
        (seg, self.path, is_final) = split_slash(self.path);
        self.path = skip_slashes(self.path);

        let seg = match seg {
            b"" => PathSegment::RootDir,
            b"." => PathSegment::CurDir,
            b".." => PathSegment::ParentDir,
            s if is_final => PathSegment::Final(s),
            s => PathSegment::Normal(s),
        };
        Some(seg)
    }
}

enum ResolveError {
    AncestorNotFound,
    NotFound,
    MissingRoot,
    AncestorNotADir,
    ReadError,
}

async fn resolve_path(
    root: Option<&ArcFd>,
    cur: ArcFd,
    path: &[u8],
    cred: &Credential,
) -> Result<ArcFd, ResolveError> {
    // TODO: file creation?

    // TODO: stack-vec to avoid alloc in most cases?
    let mut paths = alloc::vec::Vec::new();
    paths.push((0, Cow::Borrowed(path)));

    let mut cur = cur;

    let mut parent;
    let mut final_segment = None;

    while let Some((idx, path)) = paths.pop() {
        let mut segment_iter = segments(&path[idx..]);
        while let Some(segment) = segment_iter.next() {
            let new_cur = match segment {
                PathSegment::RootDir => root.map(|f| f.clone()).ok_or(ResolveError::MissingRoot)?,
                PathSegment::CurDir => cur.clone(),
                PathSegment::ParentDir => {
                    // TODO: ".." must be handled kernel side:
                    // - mounted filesystems
                    // - chroot (if cur is root, .. goes to root)
                    let is_root = root
                        .as_ref()
                        .map(|r| r.is_same_file(&*cur))
                        .unwrap_or(false);
                    if !is_root {
                        cur.open(b"..")
                            .await
                            .map_err(|()| ResolveError::AncestorNotFound)?
                    } else {
                        cur.clone()
                    }
                }
                PathSegment::Normal(name) => {
                    cur.open(name).await.map_err(|()| ResolveError::NotFound)?
                }
                PathSegment::Final(name) => {
                    // Component without a trailing slash.  If this is the
                    // topmost resolution layer
                    if paths.is_empty() {
                        let base = name.as_ptr().addr() - path.as_ptr().addr();
                        let len = name.len();
                        final_segment = Some((path, base, len));
                        break;
                    } else {
                        cur.open(name).await.map_err(|()| ResolveError::NotFound)?
                    }
                }
            };

            parent = cur;
            cur = new_cur;
            // Directory-traversal permission: require execute/search bit
            if cur.kind() == FileKind::Directory {
                if !cur.can_access(&cred, FdAccessMode::Exec) {
                    return Err(ResolveError::NotFound);
                }
            }
            // Symbolic links and type checking
            match cur.kind() {
                FileKind::SymbolicLink => {
                    cur = parent.clone();
                    let new_path = crate::process::fd::read_all(&*cur)
                        .await
                        .map_err(|_e| ResolveError::ReadError)?;
                    let cur_offset = path.len() - segment_iter.path.len();
                    paths.push((cur_offset, path));
                    paths.push((0, Cow::Owned(new_path)));
                    break;
                }
                FileKind::Directory => (),
                _ => return Err(ResolveError::AncestorNotADir),
            }
        }
    }

    if let Some((path, start, len)) = final_segment {
        let seg = &path[start..][..len];
        // TODO: file creation
        let file = cur.open(seg).await.map_err(|()| ResolveError::NotFound)?;
        Ok(file)
    } else {
        Ok(cur)
    }
}
