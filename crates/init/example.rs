#!/usr/bin/env bash
#![doc = "<!-- Absolutely cursed hacks:
/*usr/bin/env true <<'END_BASH_COMMENT' # -->"]
#![no_std]
#![no_main]

mod runtime {
    #[rustfmt::skip]
    static _COMPILE_SCRIPT: () = { r##"
END_BASH_COMMENT

set -e
SOURCE=$(realpath "$0")
NAME=$(basename "$SOURCE" .rs)
RELATIVE=$(realpath --relative-to=. "$SOURCE")

MANIFEST=$(
cat <<END_MANIFEST
[package]
name = "$NAME"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "$NAME"
path = "$SOURCE"

[dependencies]

[profile.standalone]
inherits = "release"
opt-level = 2
panic = "abort"
strip = "debuginfo"

END_MANIFEST
)

TEMP_DIR="$(mktemp -d)"
trap 'rm -rf -- "$TEMP_DIR"' EXIT
MANIFEST_PATH="$TEMP_DIR/Cargo.toml"
echo "$MANIFEST" > "$MANIFEST_PATH"

TARGET_DIR=$(cargo metadata --format-version 1 --no-deps | \
    python3 -c 'print(__import__("json").loads(input())["target_directory"])')

CARGO_TARGET_DIR="$TARGET_DIR" RUSTC_BOOTSTRAP=1 cargo rustc \
    --manifest-path="$MANIFEST_PATH" \
    --target=aarch64-unknown-none-softfloat \
    --profile=standalone \
    --bin "$NAME" \
    -- \
    -C link-arg=-T"${SOURCE}" -C link-args='-zmax-page-size=0x1000' \

BIN_PATH="${TARGET_DIR}/aarch64-unknown-none-softfloat/standalone/${NAME}"
cp "${BIN_PATH}" "${SOURCE%.rs}.elf"

SIZE=$(stat -c %s "${SOURCE%.rs}.elf" | python3 -c \
    "(lambda f:f(f,float(input()),0))\
     (lambda f,i,j:print('%.4g'%i,'BKMGTPE'[j]+'iB' if j else 'bytes')\
     if i<1024 else f(f,i/1024,j+1))"
)
echo "Built ${RELATIVE%.rs}.elf, file size ${SIZE}"
exit
    "##; };

    #[rustfmt::skip]
    static _LINKER_SCRIPT: () = { r"*/
    ENTRY(_start);

    PHDRS {
        segment_main PT_LOAD;
    }

    SECTIONS {
        . = 0x300000;
        .text : { *(.text) *(.text*) } :segment_main
        .rodata : ALIGN(8) { *(.rodata) *(.rodata*) } :segment_main
        .data : { *(.data) *(.data.*) } :segment_main

        .bss (NOLOAD) : ALIGN(16) { *(.bss) *(.bss.*) } :segment_main
    }
    /*"; };

    #[no_mangle]
    extern "C" fn _start(x0: usize) -> ! {
        let channel = ChannelDesc(x0 as u32);
        crate::main(channel);
        unsafe { exit() };
        loop {}
    }

    pub struct Stdout;
    impl core::fmt::Write for Stdout {
        fn write_str(&mut self, s: &str) -> core::fmt::Result {
            let msg = Message {
                tag: 0,
                objects: [0; 4],
            };
            let chan = ChannelDesc(1);
            unsafe { send_block(chan, &msg, s.as_bytes()) };
            Ok(())
        }
    }
    #[macro_export]
    macro_rules! print {
        ($($arg:tt)*) => {{
            use core::fmt::Write;
            write!($crate::runtime::Stdout, $($arg)*).ok();
        }};
    }
    #[macro_export]
    macro_rules! println {
        ($($arg:tt)*) => {{
            use core::fmt::Write;
            writeln!($crate::runtime::Stdout, $($arg)*).ok();
        }};
    }

    #[cfg(not(test))]
    #[panic_handler]
    fn panic_handler(info: &core::panic::PanicInfo) -> ! {
        if let Some(loc) = info.location() {
            println!("Panic at {}:{}:{}; {}", loc.file(), loc.line(), loc.column(), info.message());
        } else {
            println!("Panic; {}", info.message());
        }
        loop {}
    }
    
    macro_rules! syscall {
        ($num:literal => $vis:vis fn $ident:ident ( $($arg:ident : $ty:ty),* $(,)? ) $( -> $ret:ty )?) => {
            core::arch::global_asm!(
                ".global {name}; {name}: svc #{num}; ret",
                name = sym $ident,
                num = const $num,
            );
            extern "C" {
                $vis fn $ident( $($arg: $ty,)* ) $(-> $ret)?;
            }
        };
    }

    #[repr(C)]
    #[derive(Debug, Copy, Clone)]
    pub struct ChannelDesc(pub u32);

    #[repr(C)]
    #[derive(Debug)]
    pub struct Message {
        pub tag: u64,
        pub objects: [u32; 4],
    }

    #[repr(C)]
    struct Channels(usize, usize);

    syscall!(1 => pub fn shutdown());
    syscall!(3 => pub fn yield_());
    syscall!(5 => pub fn spawn(pc: usize, sp: usize, x0: usize, flags: usize));
    syscall!(6 => pub fn exit());
    
    syscall!(7 => fn _channel() -> Channels);
    pub unsafe fn channel() -> (ChannelDesc, ChannelDesc) {
        let res = unsafe { _channel() };
        (ChannelDesc(res.0 as u32), ChannelDesc(res.1 as u32))
    }

    const FLAG_NO_BLOCK: usize = 1 << 0;

    syscall!(8 => pub fn _send(desc: ChannelDesc, msg: &Message, buf: *const u8, buf_len: usize, flags: usize) -> isize);
    syscall!(9 => pub fn _recv(desc: ChannelDesc, msg: &mut Message, buf: *mut u8, buf_cap: usize, flags: usize) -> isize);

    pub unsafe fn send(desc: ChannelDesc, msg: &Message, buf: &[u8]) -> isize {
        unsafe { _send(desc, msg, buf.as_ptr(), buf.len(), FLAG_NO_BLOCK) }
    }
    pub unsafe fn send_block(desc: ChannelDesc, msg: &Message, buf: &[u8]) -> isize {
        unsafe { _send(desc, msg, buf.as_ptr(), buf.len(), 0) }
    }
    pub unsafe fn recv(desc: ChannelDesc, msg: &mut Message, buf: &mut [u8]) -> isize {
        unsafe { _recv(desc, msg, buf.as_mut_ptr(), buf.len(), FLAG_NO_BLOCK) }
    }
    pub unsafe fn recv_block(desc: ChannelDesc, msg: &mut Message, buf: &mut [u8]) -> isize {
        unsafe { _recv(desc, msg, buf.as_mut_ptr(), buf.len(), 0) }
    }
}

fn recv_wrap(chan: runtime::ChannelDesc, buf: &mut [u8]) -> (runtime::Message, isize) {
    let mut msg = runtime::Message {
        tag: 0,
        objects: [0; 4],
    };
    let res = unsafe { runtime::recv_block(chan, &mut msg, buf) };
    (msg, res)
}

fn try_read_stdin(buf: &mut [u8]) -> isize {
    let mut msg = runtime::Message {
        tag: 0,
        objects: [0; 4],
    };
    let chan = runtime::ChannelDesc(1);
    let res = unsafe { runtime::recv_block(chan, &mut msg, buf) };
    res
}

struct LineReader {
    buf: [u8; 4096],
    cursor: usize,
    processed: usize,
    cur_base: usize,
}
impl LineReader {
    fn shift(&mut self) {
        if self.cur_base != 0 {
            self.buf[..self.cursor].copy_within(self.cur_base.., 0);
            self.cursor -= self.cur_base;
            self.processed -= self.cur_base;
            self.cur_base = 0;
        }
    }
}

fn readline(reader: &mut LineReader) -> Result<&[u8], isize> {
    reader.shift();
    loop {
        while reader.processed < reader.cursor {
            let i = reader.processed;
            reader.processed += 1;
            match reader.buf[i] {
                b'\r' => {
                    let base = reader.cur_base;
                    reader.cur_base = i + 1;
                    return Ok(&reader.buf[base..i]);
                }
                b'\x7f' => print!("^?"),
                c if c.is_ascii_control() => print!("^{}", (c + 64) as char),
                c => print!("{}", c as char),
            }
        }

        match try_read_stdin(&mut reader.buf[reader.cursor..]) {
            -2 => continue,
            err @ (..=-1) => return Err(err),
            read @ 0.. => reader.cursor += read as usize,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone)]
struct ReadAt {
    file_id: u32,
    amount: u32,
    offset: u64,
}
#[repr(C)]
#[derive(Copy, Clone)]
struct Open {
    path_len: u32,
}

fn spawn_child(files: runtime::ChannelDesc, procs: runtime::ChannelDesc, file_path: &str) -> runtime::ChannelDesc {
    let mut buf = [0; 512];
    buf[..4].copy_from_slice(&u32::to_le_bytes(file_path.len() as u32));
    buf[4..][..file_path.len()].copy_from_slice(file_path.as_bytes());

    let status = unsafe { runtime::send_block(files, &runtime::Message {
        tag: u64::from_be_bytes(*b"OPEN----"),
        objects: [0; 4],
    }, &buf[.. 4 + file_path.len()]) };

    let mut file_id = [0u8; 4];
    let (msg, _) = recv_wrap(files, &mut file_id);
    assert_eq!(msg.tag, u64::from_be_bytes(*b"OPENSUCC"));
    let file_id = u32::from_le_bytes(file_id);

    let status = unsafe { runtime::send_block(procs, &runtime::Message {
        tag: u64::from_be_bytes(*b"SPAWN---"),
        objects: [0; 4],
    }, &u32::to_le_bytes(file_id)) };

    let (msg, _) = recv_wrap(procs, &mut []);
    assert_eq!(msg.tag, u64::from_be_bytes(*b"SUCCESS-"));
    let child_handle = runtime::ChannelDesc(msg.objects[0]);
    child_handle
}

fn main(chan: runtime::ChannelDesc) {
    println!("Starting 🐚");

    let status = unsafe { runtime::send_block(chan, &runtime::Message {
        tag: u64::from_be_bytes(*b"CONNREQ-"),
        objects: [0; 4],
    }, b"FILES---") };

    let (msg, _) = recv_wrap(chan, &mut []);
    let filesystem = runtime::ChannelDesc(msg.objects[0]);

    let status = unsafe { runtime::send_block(filesystem, &runtime::Message {
        tag: u64::from_be_bytes(*b"OPEN----"),
        objects: [0; 4],
    }, "\x08\x00\x00\x00test.txt".as_bytes()) };

    let mut file_id = [0u8; 4];
    let (msg, _) = recv_wrap(filesystem, &mut file_id);
    assert_eq!(msg.tag, u64::from_be_bytes(*b"OPENSUCC"));
    let file_id = u32::from_le_bytes(file_id);

    let read = ReadAt {
        file_id,
        amount: 4096,
        offset: 0,
    };
    let status = unsafe { runtime::send_block(filesystem, &runtime::Message {
        tag: u64::from_be_bytes(*b"READAT--"),
        objects: [0; 4],
    }, unsafe { core::slice::from_raw_parts(&read as *const _ as *const u8, size_of::<ReadAt>()) }) };

    let mut data = [0u8; 4096];
    let (msg, len) = recv_wrap(filesystem, &mut data);
    assert_eq!(msg.tag, u64::from_be_bytes(*b"DATA----"));
    let file_content = &data[..len as usize];

    println!("File content:\n{}", core::str::from_utf8(file_content).unwrap());

    println!("Attempting to spawn child");

    let status = unsafe { runtime::send_block(chan, &runtime::Message {
        tag: u64::from_be_bytes(*b"CONNREQ-"),
        objects: [0; 4],
    }, b"PROCS---") };

    let (msg, _) = recv_wrap(chan, &mut []);
    let procs = runtime::ChannelDesc(msg.objects[0]);

    let child_handle = spawn_child(filesystem, procs, "hello.elf");

    let (msg, _) = recv_wrap(child_handle, &mut []);
    println!("from child: {}", core::str::from_utf8(&msg.tag.to_be_bytes()).unwrap());


    let mut reader = LineReader {
        buf: [0; 4096],
        cursor: 0,
        processed: 0,
        cur_base: 0,
    };
    loop {
        print!("$ ");
        let line = match readline(&mut reader) {
            Ok(line) => line,
            Err(err) => {
                println!("Error: {err}");
                break;
            }
        };
        println!();
        let line = unsafe {
            core::str::from_utf8_unchecked(&line)
        };
        if line.trim().is_empty() {
            continue;
        }
        let mut split = line.split_ascii_whitespace();
        let cmd = split.next().unwrap_or(line);
        println!("cmd: {}, line: {}", cmd, line);
    }

    unsafe { runtime::exit() };
}

// I'll just leave this here: */
