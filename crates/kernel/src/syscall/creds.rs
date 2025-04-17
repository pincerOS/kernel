use crate::event::async_handler::{run_event_handler, HandlerContext};
use crate::event::context::Context;

/// setuid(uid: u32) -> i64
///
/// Sets the real, effective, and saved user IDs of the calling process to `uid`.
///
/// # Arguments
/// - `uid` (in `ctx.regs[0]`): The user ID to set.
///
/// # Return value
/// - Returns 0 on success.
/// - Returns -1 on failure (e.g., insufficient permissions).
///
/// # Semantics
/// - If the process is running as root (effective UID 0), all three UIDs (real, effective, saved) are set to `uid`.
/// - If the process is not root, the effective user ID can only be changed to the real user ID, the effective user ID, or the saved set-user-ID. If this is not permitted, the call fails with -1.
pub unsafe fn setuid(ctx: &mut Context) -> *mut Context {
    let uid = ctx.regs[0] as u32;

    run_event_handler(ctx, move |context: HandlerContext<'_>| {
        let proc = context.cur_process().unwrap();
        let mut cred = proc.credential.lock();

        let result = {
            // handle the case where the process is privileged (root)
            if cred.euid == 0 {
                cred.euid = uid;
                cred.ruid = uid;
                cred.suid = uid;

                0
            } else {
                // handle the case where the process is unprivileged
                match cred.try_set_reuid(None, Some(uid)) {
                    Ok(_) => 0,
                    Err(_) => -1isize as usize,
                }
            }
        };

        drop(cred);
        context.resume_return(result)
    })
}

/// setreuid(ruid: u32, euid: u32) -> i64
///
/// Sets the real and/or effective user IDs of the calling process.
///
/// # Arguments
/// - `ruid` (in `ctx.regs[0]`): The real user ID to set.
/// - `euid` (in `ctx.regs[1]`): The effective user ID to set.
///
/// # Return value
/// - Returns 0 on success.
/// - Returns -1 on failure (e.g., insufficient permissions).
///
/// # Semantics
/// - If the process is running as root (effective UID 0), it may set either or both UIDs to any value.
/// - If not root, the real UID may only be set to the real or effective UID, and the effective UID may only be set to the real, effective, or saved set-user-ID.
/// - If the requested change is not permitted, the call fails with -1.
pub unsafe fn setreuid(ctx: &mut Context) -> *mut Context {
    let ruid = ctx.regs[0] as u32;
    let euid = ctx.regs[1] as u32;

    run_event_handler(ctx, move |context: HandlerContext<'_>| {
        let proc = context.cur_process().unwrap();
        let mut cred = proc.credential.lock();

        let result = match cred.try_set_reuid(Some(ruid), Some(euid)) {
            Ok(_) => 0,
            Err(_) => -1isize as usize,
        };

        drop(cred);
        context.resume_return(result)
    })
}

/// geteuid() -> u32
///
/// Returns the current effective user ID of the calling process.
pub unsafe fn geteuid(ctx: &mut Context) -> *mut Context {
    run_event_handler(ctx, |context: HandlerContext<'_>| {
        let proc = context.cur_process().unwrap();
        let cred = proc.credential.lock();
        let euid = cred.euid;
        drop(cred);
        context.resume_return(euid as usize)
    })
}

/// getruid() -> u32
///
/// Returns the current real user ID of the calling process.
pub unsafe fn getruid(ctx: &mut Context) -> *mut Context {
    run_event_handler(ctx, |context: HandlerContext<'_>| {
        let proc = context.cur_process().unwrap();
        let cred = proc.credential.lock();
        let euid = cred.euid;
        drop(cred);
        context.resume_return(euid as usize)
    })
}

/// getsuid() -> u32
///
/// Returns the current saved user ID of the calling process.
pub unsafe fn getsuid(ctx: &mut Context) -> *mut Context {
    run_event_handler(ctx, |context: HandlerContext<'_>| {
        let proc = context.cur_process().unwrap();
        let cred = proc.credential.lock();
        let euid = cred.suid;
        drop(cred);
        context.resume_return(euid as usize)
    })
}
