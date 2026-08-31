use crate::event::task::spawn_async;
use crate::ringbuffer::Sender;
use alloc::boxed::Box;
use alloc::string::String;
use core::error::Error;
use core::fmt::Display;

pub type TestResult = Result<(), String>;
pub type TestSender = Sender<2, TestResult>;

pub trait TestImpl: Sync {
    fn run(&'static self, channel: TestSender);
}

pub trait Failure {
    fn as_error(&self) -> Option<&dyn Display>;
}

// TODO: requires specialization (or macro hacks w/ Deref)
// impl<E: core::error::Error> Failure for Result<(), E> {
//     fn as_error(&self) -> Option<&dyn Display> {
//         self.as_ref().err().map(|e| e as &dyn Display)
//     }
// }
impl Failure for Result<(), &'static str> {
    fn as_error(&self) -> Option<&dyn Display> {
        self.as_ref().err().map(|e| e as &dyn Display)
    }
}
impl Failure for Result<(), alloc::boxed::Box<dyn core::error::Error>> {
    fn as_error(&self) -> Option<&dyn Display> {
        self.as_ref().err().map(|e| e as &dyn Display)
    }
}
impl Failure for Result<(), alloc::boxed::Box<dyn core::error::Error + Send + Sync>> {
    fn as_error(&self) -> Option<&dyn Display> {
        self.as_ref().err().map(|e| e as &dyn Display)
    }
}

impl Failure for () {
    fn as_error(&self) -> Option<&dyn Display> {
        None
    }
}

impl<F, E> TestImpl for F
where
    F: Fn() -> E + Sync,
    E: Failure,
{
    fn run(&'static self, mut channel: TestSender) {
        spawn_async(async move {
            let result = match self().as_error() {
                Some(e) => Err(alloc::format!("{}", e)),
                None => Ok(()),
            };
            channel.send(result).await;
        });
    }
}

pub trait AsyncFnCustomSend<Args> {
    type Output;
    type CallRefFuture: core::future::Future<Output = Self::Output> + Send;
    fn call(&self, args: Args) -> Self::CallRefFuture;
}

impl<F, Fu> AsyncFnCustomSend<()> for F
where
    F: Fn() -> Fu,
    Fu: core::future::Future + Send,
{
    type Output = Fu::Output;
    type CallRefFuture = Fu;
    fn call(&self, _args: ()) -> Self::CallRefFuture {
        self()
    }
}

pub struct AsyncImpl<F>(pub F);

impl<T, E> AsyncImpl<T>
where
    T: AsyncFnCustomSend<(), Output = E> + Sync,
    E: Failure + Send,
{
    pub const fn new(f: T) -> Self {
        AsyncImpl(f)
    }
}

impl<F, E> TestImpl for AsyncImpl<F>
where
    F: AsyncFnCustomSend<(), Output = E> + Sync,
    E: Failure + Send,
{
    fn run(&'static self, mut channel: TestSender) {
        spawn_async(async move {
            let result = self.0.call(()).await;
            let result = match result.as_error() {
                Some(e) => Err(alloc::format!("{}", e)),
                None => Ok(()),
            };
            channel.send(result).await;
        });
    }
}

pub struct ThreadImpl<F>(pub F);

impl<T, E> ThreadImpl<T>
where
    T: Fn() -> E + Sync,
    E: Failure + Send,
{
    pub const fn new(f: T) -> Self {
        ThreadImpl(f)
    }
}

impl<F, E> TestImpl for ThreadImpl<F>
where
    F: Fn() -> E + Sync,
    E: Failure + Send,
{
    fn run(&'static self, mut channel: TestSender) {
        crate::event::thread::thread(move || {
            let result = (self.0)();
            let result = match result.as_error() {
                Some(e) => Err(alloc::format!("{}", e)),
                None => Ok(()),
            };
            channel.try_send(result).map_err(|_| ()).unwrap();
        });
    }
}

pub struct TestCase {
    pub name: &'static str,
    pub test: &'static dyn TestImpl,
}

pub struct StaticArray<TestCase> {
    start: *const TestCase,
    end: *const TestCase,
}

unsafe impl<'a, T: 'a> Send for StaticArray<T> where &'a [T]: Send {}
unsafe impl<'a, T: 'a> Sync for StaticArray<T> where &'a [T]: Sync {}

impl<T> StaticArray<T> {
    pub fn array(&self) -> &'static [T] {
        unsafe {
            let len = (self.end).offset_from(self.start) as usize;
            core::slice::from_raw_parts(self.start, len)
        }
    }
}

pub static TESTS: StaticArray<TestCase> = {
    #[used]
    #[unsafe(link_section = ".test_array")]
    static mut __ARRAY: [TestCase; 0] = [];
    unsafe extern "Rust" {
        #[link_name = "__test_array_start"]
        static __START: [TestCase; 0];
        #[link_name = "__test_array_end"]
        static __END: [TestCase; 0];
    }
    assert!(size_of::<TestCase>() > 0);
    StaticArray {
        start: (&raw const __START).cast(),
        end: (&raw const __END).cast(),
    }
};

#[macro_export]
macro_rules! test_case {
    (async $name:ident) => {
        $crate::test_case!(@ $name , $crate::test::AsyncImpl::new($name));
    };
    (thread $name:ident) => {
        $crate::test_case!(@ $name , $crate::test::ThreadImpl::new($name));
    };
    ($name:ident) => {
        $crate::test_case!(@ $name , $name);
    };
    (@ $name:ident , $inner:expr) => {
        const _: () = {
            #[used]
            #[unsafe(link_section = ".test_array")]
            static TEST: $crate::test::TestCase = $crate::test::TestCase {
                name: ::core::stringify!($name),
                test: &$inner as &dyn $crate::test::TestImpl,
            };
        };
    };
}

#[derive(Debug)]
pub struct AssertError(pub alloc::string::String);

impl core::fmt::Display for AssertError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Display::fmt(&self.0, f)
    }
}

impl core::error::Error for AssertError {}

pub type BoxError = Box<dyn Error + Send + Sync>;

#[macro_export]
macro_rules! kassert_eq {
    ($lhs:expr, $rhs:expr) => {{
        $crate::kassert_eq!($lhs, $rhs, "")
    }};
    ($lhs:expr, $rhs:expr, $context:literal $(, $($args:tt)*)?) => {{
        let lhs = $lhs;
        let rhs = $rhs;
        if lhs != rhs {
            let msg = alloc::format!(
                "Assertion failed at at {}:{}: {:?}\n    lhs = {:?}\n    rhs = {:?}{}{}",
                core::file!(), core::line!(),
                core::stringify!($lhs != $rhs),
                lhs, rhs,
                if $context == "" { "" } else { "\n" },
                core::format_args!($context, $($args)*),
            );
            Err($crate::test::AssertError(msg))
        } else {
            Ok(())
        }
    }};
}

#[macro_export]
macro_rules! kassert {
    ($cond:expr) => {{
        $crate::kassert!($cond, "")
    }};
    ($cond:expr, $context:literal $(, $($args:tt)*)?) => {{
        let cond = $cond;
        if !cond {
            let msg = alloc::format!(
                "Assertion failed at at {}:{}: {:?}{}{}",
                core::file!(), core::line!(),
                core::stringify!($cond),
                if $context == "" { "" } else { "\n" },
                core::format_args!($context, $($args)*),
            );
            Err($crate::test::AssertError(msg))
        } else {
            Ok(())
        }
    }};
}
