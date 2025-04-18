#![no_std]
#![no_main]

#[macro_use]
extern crate ulib;
extern crate alloc;

use alloc::vec::Vec;
use ulib::sys::FileDesc;

const STDIN_FD: FileDesc = 0;

fn try_read_stdin(buf: &mut [u8]) -> Result<usize, usize> {
    ulib::sys::pread(STDIN_FD, buf, 0)
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

fn readline(reader: &mut LineReader) -> Result<&[u8], usize> {
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
                b'\x7f' => {
                    if reader.processed >= 2 {
                        reader.processed -= 2;
                        reader.cursor -= 2;
                        print!("\x08 \x08");
                    } else {
                        reader.processed -= 1;
                        reader.cursor -= 1;
                    }
                }
                c if c.is_ascii_control() => print!("^{}", (c + 64) as char),
                c => print!("{}", c as char),
            }
        }

        let read = try_read_stdin(&mut reader.buf[reader.cursor..])?;
        reader.cursor += read;
    }
}

struct redirection<'a> {
    redirect_type: i32,
    file: &'a str
}

struct simple_command<'a> {
    command: &'a str,
    args: Vec<&'a str>,
    redirections: Vec<redirection<'a>>,
}


#[no_mangle]
fn main() {
    println!("Starting 🐚");

    let root = 3;
    let path = "test.txt";
    let fd = ulib::sys::openat(root, path.as_bytes(), 0, 0).unwrap();

    println!("File: {}", fd);

    let mut data = [0; 4096];

    let mut read = 0;
    while read < data.len() {
        match ulib::sys::pread(fd, &mut data[read..], read as u64) {
            Err(_) => break,
            Ok(0) => break,
            Ok(i) => read += i,
        }
    }

    println!(
        "File content: ======\n{}\n====================",
        core::str::from_utf8(&data[..read]).unwrap()
    );

    let stdout = 1;
    let buf = b"Stdout write test\n";
    ulib::sys::pwrite_all(stdout, buf, 0).unwrap();

    let mut reader = LineReader {
        buf: [0; 4096],
        cursor: 0,
        processed: 0,
        cur_base: 0,
    };

    let root_fd = 3;

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

        let line = unsafe { core::str::from_utf8_unchecked(&line) };
        if line.trim().is_empty() {
            continue;
        }

        fn is_pipe(c: u8) -> bool {
            c == b'|'
        }

        fn is_redirection(c: u8) -> bool {
            c == b'>' || c == b'<'
        }

        fn is_background(c: u8) -> bool {
            c == b'&'
        }

        fn is_special_char(c: u8) -> bool {
            is_pipe(c) || is_redirection(c) || is_background(c)
        }

        if line == "exit" {
            break;
        } else {
            let mut run_background = false;
            let split: Vec<&str> = line.split_ascii_whitespace().collect(); //TODO: Introduce regex for split and grammar for actual parsing
            let mut queue = Vec::new();
            
            let mut i = 0;
            while i < split.len() {
                let c = split[i];
                let mut j = i + 1;
                while j < split.len() && !is_special_char(split[j].as_bytes()[0]) {
                    j += 1;
                }

                let mut redirects = Vec::new();
                let mut k = j;
                while k < split.len() && is_redirection(split[k].as_bytes()[0]) {
                    let redirection_str = split[k];
                    let redirection_type;
                    if redirection_str == "<" {
                        redirection_type = 0;
                    } else if redirection_str == "<<" {
                        redirection_type = 1;
                    } else if redirection_str == ">" {
                        redirection_type = 2;
                    } else if redirection_str == "<<" {
                        redirection_type = 3;
                    } else {
                        redirection_type = -1;
                    }

                    k += 1;
                    redirects.push(redirection {
                        redirect_type: redirection_type,
                        file: split[k],
                    });
                    k += 1;
                }

                if k < split.len() && is_background(split[k].as_bytes()[0]) {
                    run_background = true;
                    k += 1;
                }
                
                // println!("Command: {}, Args: {:?}, i {} j {}", c, &split[(i + 1)..j], i, j);
                queue.push(simple_command {
                    command: c,
                    args: split[(i + 1)..j].to_vec(),
                    redirections: redirects,
                });
                i = k;

                if i < split.len() && split[i].as_bytes()[0] == b'|' {
                    i += 1;
                }
            }

            let mut next_pipe = None;
            for i in 0..queue.len() {
                let next = queue.get(i).unwrap();
                let has_next = i < queue.len() - 1;

                let command = next.command;
                let args = &next.args;
                if command == "cd" {
                    let err = ulib::sys::chdir(args[0].as_bytes());
                    if let Err(err1) = err {
                        if (err1 as i32) == -1 {
                            println!("cd: no such file or directory: {}", args[0]);
                        } else if (err1 as i32) == -2 {
                            println!("cd: not a directory: {}", args[0]);
                        } else {
                            println!("cd: unknown error: {}", err1);
                        }
                    }
                } else if let Ok(file) = ulib::sys::openat(root_fd, command.as_bytes(), 0, 0) {
                    use ulib::sys::ArgStr;

                    let argstrs: Vec<ArgStr> = args
                        .iter()
                        .map(|s| ArgStr {
                            len: s.len(),
                            ptr: s.as_ptr(),
                        })
                        .collect();
                    
                    let mut cur_stdout;
                    let mut future_next_pipe = None;

                    if has_next {
                        let (read_fd, write_fd) = ulib::sys::pipe(0).unwrap();
                        cur_stdout = Some(write_fd);
                        future_next_pipe = Some(read_fd);
                    } else {
                        cur_stdout = None;
                    }

                    if next.redirections.len() > 0 {
                        for redirect in &next.redirections {
                            match redirect.redirect_type {
                                0 => { //<
                                    if let Ok(redirect_file) = ulib::sys::open(redirect.file.as_bytes(), 0) {
                                        next_pipe = Some(redirect_file);
                                    } else {
                                        println!("failed to open file for redirection: {}", redirect.file);
                                    }
                                }
                                1 => { //<<
                                    println!("<< redirection not implemented");
                                    continue;
                                }
                                2 => { //>
                                    if let Ok(redirect_file) = ulib::sys::open(redirect.file.as_bytes(), 0) {
                                        cur_stdout = Some(redirect_file);
                                    } else {
                                        println!("failed to open file for redirection: {}", redirect.file);
                                    }
                                }
                                3 => { //<<
                                    println!("<< redirection not implemented");
                                    continue;
                                }
                                _ => {
                                    println!("unknown redirection type: {}", redirect.redirect_type);
                                    continue;
                                }
                            }
                        }
                    }

                    let child = ulib::sys::spawn_elf(&ulib::sys::SpawnArgs {
                        fd: file,
                        stdin: next_pipe,
                        stdout: cur_stdout,
                        stderr: None,
                        args: &argstrs,
                    })
                    .unwrap();

                    next_pipe = future_next_pipe;

                    if !run_background { //TODO: Hacky solution -> need tracking
                        let status = ulib::sys::wait(child).unwrap();
                        println!("child exited with code {}", status);
                    }
                } else {
                    println!("unknown command: {:?}", command);
                }
            }
        }
    }

    ulib::sys::exit(15);
}
