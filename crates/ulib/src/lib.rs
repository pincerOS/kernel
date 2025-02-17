#![no_std]

pub mod sys;

#[cfg(feature = "runtime")]
pub mod runtime;

pub struct Stdout;

impl core::fmt::Write for Stdout {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let msg = sys::Message {
            tag: 0,
            objects: [0; 4],
        };
        let chan = sys::ChannelDesc(1);
        let res = sys::send_block(chan, &msg, s.as_bytes());
        assert!(res >= 0);
        Ok(())
    }
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {{
        use core::fmt::Write;
        write!($crate::Stdout, $($arg)*).ok();
    }};
}

#[macro_export]
macro_rules! println {
    ($($arg:tt)*) => {{
        use core::fmt::Write;
        writeln!($crate::Stdout, $($arg)*).ok();
    }};
}

pub fn format_ascii(s: &[u8]) -> impl core::fmt::Display + '_ {
    struct Formatter<'a>(&'a [u8]);
    impl core::fmt::Display for Formatter<'_> {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            let s = self.0;
            let mut i = 0;
            while i < s.len() {
                match s[i] {
                    b if !b.is_ascii() || b.is_ascii_control() => {
                        write!(f, "\\x{:02x}", b)?;
                        i += 1;
                    },
                    _ => {
                        let end = s[i + 1..].iter().position(|b| !b.is_ascii() || b.is_ascii_control())
                            .unwrap_or(s[i + 1..].len()) + i + 1;
                        f.write_str(unsafe { core::str::from_utf8_unchecked(&s[i..end]) })?;
                        i = end;
                    }
                }
            }
            Ok(())
        }
    }
    Formatter(s)
}
