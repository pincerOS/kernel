use core::fmt::Display;

use alloc::string::String;
use event::task::spawn_async;
use kernel::*;

pub type TestResult = Result<(), String>;
pub type TestSender = ringbuffer::Sender<2, TestResult>;

pub trait TestImpl: Sync {
    fn run(&'static self, channel: TestSender);
}

pub trait Failure {
    fn as_error(&self) -> Option<&dyn Display>;
}

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

// pub fn check_impls<T, E>(f: T) -> T
// where
//     T: AsyncFnCustomSend<(), Output = E> + Sync,
//     E: Failure + Send,
// {
//     f
// }

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
    #[link_section = ".test_array"]
    static mut __ARRAY: [TestCase; 0] = [];
    extern "Rust" {
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
    ($name:ident) => {
        const _: () = {
            #[used]
            #[link_section = ".test_array"]
            static TEST: $crate::test_impl::TestCase = $crate::test_impl::TestCase {
                name: ::core::stringify!($name),
                test: &$name as &dyn $crate::test_impl::TestImpl,
            };
        };
    };
    (async $name:ident) => {
        const _: () = {
            #[used]
            #[link_section = ".test_array"]
            static TEST: $crate::test_impl::TestCase = $crate::test_impl::TestCase {
                name: ::core::stringify!($name),
                test: &$crate::test_impl::AsyncImpl($name) as &dyn $crate::test_impl::TestImpl,
            };
        };
    };
}
