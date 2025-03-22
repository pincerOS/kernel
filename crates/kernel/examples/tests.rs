#![no_std]
#![no_main]

extern crate alloc;
#[macro_use]
extern crate kernel;

use kernel::test::TESTS;

#[path = "tests/mod.rs"]
mod tests;

pub const BOLD: &str = "\x1b[1m";
pub const GREEN: &str = "\x1b[32;1m";
pub const RED: &str = "\x1b[31;1m";
pub const YELLOW: &str = "\x1b[33;1m";
pub const RESET: &str = "\x1b[0m";

fn format_micros(duration: u64) -> impl core::fmt::Display {
    struct Duration(u64);
    impl core::fmt::Display for Duration {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            let millis = self.0 / 1000;
            write!(f, "{:01}.{:03}s", millis / 1000, millis % 1000)
        }
    }
    Duration(duration)
}

#[no_mangle]
extern "Rust" fn kernel_main(_device_tree: device_tree::DeviceTree<'static>) {
    kernel::event::task::spawn_async(async move {
        run_tests().await;
        kernel::shutdown();
    });
    kernel::event::thread::stop();
}

async fn run_tests() {
    let tests = TESTS.array();

    let timer = kernel::device::system_timer::SYSTEM_TIMER.get();
    let start = timer.get_time();

    println!("{GREEN}Running{RESET} {} tests", tests.len());

    let mut passed = 0;
    let mut failed = 0;

    for test in TESTS.array() {
        println!("{BOLD}=== Starting test{RESET} {}...", test.name);
        let (tx, mut rx) = kernel::ringbuffer::channel();

        let test_start = timer.get_time();

        test.test.run(tx);
        let result = rx.recv().await;

        let test_end = timer.get_time();
        let duration = format_micros(test_end - test_start);

        if let Err(e) = result {
            println!("{BOLD}==={RESET} {RED}failed{RESET} in {}: {}", duration, e);
            failed += 1;
        } else {
            println!("{BOLD}==={RESET} {GREEN}passed{RESET} in {}", duration);
            passed += 1;
        }
    }

    let end = timer.get_time();
    let elapsed = end - start;

    println!("=== All tests completed");
    println!("| Finished in {}", format_micros(elapsed));
    println!(
        "| Total {} tests, {} passed, {} failed",
        passed + failed,
        passed,
        failed
    );
}
