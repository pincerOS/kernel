use alloc::borrow::ToOwned;
use alloc::vec;

use crate::event::async_handler::{run_async_handler, HandlerContext};
use crate::event::context::Context;
use crate::process::mem::{MappingKind, UserAddrSpace};

bitflags::bitflags! {
    struct ExecFlags: u32 {
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
struct StringPair {
    len: usize,
    ptr: *const u8,
}

// using usize to smuggle pointer into async callbacks
struct ExecArgs {
    fd: usize,
    _flags: ExecFlags,
    args_len: usize,
    args_ptr: *const StringPair,
    env_len: usize,
    env_ptr: *const StringPair,
}
unsafe impl Send for ExecArgs {}

unsafe fn from_raw_parts_or_empty<'a, T>(data: *const T, len: usize) -> &'a [T] {
    if len == 0 {
        &[]
    } else {
        unsafe { core::slice::from_raw_parts(data, len) }
    }
}

/// syscall execve_fd(
///     fd: usize,
///     flags: ExecFlags,
///     argc: usize,
///     argv: *const (usize, *const u8),
///     envc: usize,
///     envp: *const (usize, *const u8),
/// ) -> i64
pub unsafe fn sys_execve_fd(ctx: &mut Context) -> *mut Context {
    let fd = ctx.regs[0];
    let flags = ctx.regs[1];

    let args_len = ctx.regs[2];
    let args_ptr = ctx.regs[3] as *const StringPair;

    let env_len = ctx.regs[4];
    let env_ptr = ctx.regs[5] as *const StringPair;

    let Some(flags) = u32::try_from(flags).ok().and_then(ExecFlags::from_bits) else {
        ctx.regs[0] = -1i64 as usize;
        return ctx;
    };

    let arg_data = ExecArgs {
        fd,
        _flags: flags,
        args_len,
        args_ptr,
        env_len,
        env_ptr: env_ptr,
    };

    let mut kernel_args = vec![];

    run_async_handler(ctx, async move |mut context: HandlerContext<'_>| {
        let proc = context.cur_process().unwrap();

        let file = proc.file_descriptors.lock().get(arg_data.fd).cloned();
        let Some(file) = file else {
            return context.resume_return(-1i64 as usize);
        };

        context.with_user_vmem(|| {
            let arg_data = &arg_data;
            // TODO: soundness, check user args

            let args = unsafe {
                from_raw_parts_or_empty(arg_data.args_ptr as *const StringPair, arg_data.args_len)
            };

            for pair in args {
                let slice = unsafe { from_raw_parts_or_empty(pair.ptr, pair.len) };

                kernel_args.push(slice.to_owned());
            }

            let _env = unsafe { from_raw_parts_or_empty(arg_data.env_ptr, arg_data.env_len) };
        });

        let file_data = match crate::process::fd::read_all(&*file).await {
            Ok(f) => f,
            Err(_e) => return context.resume_return(-1i64 as usize),
        };

        // TODO: only read parts of file that are necessary
        let elf = elf::Elf::new(&file_data).unwrap();

        // TODO: precise behavior of exec regarding processes
        // TODO: fd close on exec

        // TODO: error handling
        // TODO: create new address space rather than modifying current
        // TODO: how to deal with other threads of same process???

        let mut new_mem = UserAddrSpace::new();
        let ttbr0 = new_mem.get_ttbr0();
        let callback = async {
            let phdrs = elf.program_headers().unwrap();
            for phdr in phdrs {
                let phdr = phdr.unwrap();
                if matches!(phdr.p_type, elf::program_header::Type::Load) {
                    let data = elf.segment_data(&phdr).unwrap();
                    let memsize = (phdr.p_memsz as usize).next_multiple_of(4096).max(4096);

                    let base = new_mem
                        .mmap(Some(phdr.p_vaddr as usize), memsize, MappingKind::Anon)
                        .unwrap();

                    // TODO: figure out how to handle user page faults when in the kernel
                    // (need to track current address space, even if there isn't an active process)
                    // (in this case, the current layout would be unsound -- new_mem is owned
                    // by the current task)

                    let vme = new_mem.get_vme(base).unwrap();
                    // new_mem.populate_range(vme, base, data.len()).await.unwrap();
                    new_mem.populate_range(vme, base, memsize).await.unwrap();

                    let addr = (phdr.p_vaddr as usize) as *mut u8;
                    let mapping: &mut [u8] =
                        unsafe { core::slice::from_raw_parts_mut(addr, memsize) };
                    mapping[..data.len()].copy_from_slice(data);
                    // TODO: make sure anonymous pages are zeroed
                    mapping[data.len()..].fill(0);
                }
            }
        };
        unsafe { crate::memory::with_user_vmem_async(ttbr0, callback).await };

        let stack_size = 0x20_0000;
        let stack_start = 0x100_0000;
        new_mem
            .mmap(
                Some(stack_start - stack_size),
                stack_size,
                MappingKind::Anon,
            )
            .unwrap();

        let mut user_sp = stack_start;
        let mut argc = 0;
        let mut argv = 0;
        let setup_stack = async {
            // println!("In stack callback");
            let stack_vme = new_mem.get_vme(stack_start - stack_size).unwrap();
            new_mem
                .populate_range(stack_vme, stack_start - stack_size, stack_size)
                .await
                .unwrap();

            // println!("Arg ptr: {args_ptr:#x} arg len: {args_len:#x}");
            // for i in 0..kernel_args.len() {
            //     let arg = kernel_args.get(i).unwrap();
            //     println!("Arg {}: {}", i, arg);
            // }

            let mut user_args = alloc::vec::Vec::with_capacity(kernel_args.len());
            for arg in kernel_args.iter().rev() {
                let arg_str_len = arg.len() + 1;
                user_sp -= arg_str_len;

                let ptr = user_sp as *mut u8;
                unsafe { core::ptr::copy_nonoverlapping(arg.as_ptr(), ptr, arg.len()) };
                unsafe { ptr.byte_add(arg.len()).write_volatile(b'\0') };
                user_args.push(user_sp);
            }
            // align sp
            user_sp &= !0b111;
            user_sp -= core::mem::size_of::<usize>();
            let ptr = (user_sp) as *mut usize;
            // println!("Writing NULL at addr {}", user_sp);
            unsafe { ptr.write_volatile(0) };
            for arg_ptr in user_args.iter() {
                user_sp -= core::mem::size_of::<usize>();
                let ptr = (user_sp) as *mut usize;
                // println!("Writing arg pointer {:p} at addr {:p}", *arg_ptr as *const usize, user_sp as *const usize);
                unsafe { ptr.write_volatile(*arg_ptr) };
            }
            argv = user_sp;
            user_sp -= core::mem::size_of::<usize>();
            let ptr = (user_sp) as *mut usize;
            // println!("Writing argv {} at addr {:p}", argv, user_sp as *const usize);
            unsafe { ptr.write_volatile(argv as usize) };

            user_sp -= core::mem::size_of::<usize>();
            let ptr = (user_sp) as *mut usize;
            // println!("Writing argc {} at addr {:p}", arg_data.args_len, user_sp as *const usize);
            unsafe { ptr.write_volatile(arg_data.args_len) };
            // println!("Stack has been set up!");
            argc = arg_data.args_len;
        };

        unsafe { crate::memory::with_user_vmem_async(ttbr0, setup_stack).await };

        let old = core::mem::replace(&mut *proc.mem.lock(), new_mem);
        drop(old);

        let user_entry = elf.elf_header().e_entry();

        {
            let mut regs = context.regs();
            regs.regs = [0; 31];
            regs.regs[0] = argc;
            regs.regs[1] = argv;
            regs.elr = user_entry as usize;
            regs.spsr = 0b0000; // TODO: standardize initial SPSR values
            regs.sp_el0 = user_sp;
            context.user_regs().as_mut().unwrap().ttbr0_el1 = ttbr0;
        }

        // println!("Jumping to user entry point: {user_entry:#x}");
        context.resume_final()
    })
}
