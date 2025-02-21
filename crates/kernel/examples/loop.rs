#![no_std]
#![no_main]

extern crate alloc;
extern crate kernel;

use kernel::*;

fn test_irq_handler(_ctx: &mut context::Context) {
    //Reset the timer to ping again
    device::system_timer::ARM_GENERIC_TIMERS.with_current(|timer| {
        timer.reset_timer();
    });

    //Do things
    let time = device::system_timer::get_time();
    let core = crate::arch::core_id() & 3;
    println!("Timer interrupt at time: {time} on core {core}");
}

#[no_mangle]
extern "Rust" fn kernel_main(_device_tree: device_tree::DeviceTree) {
    //TODO: Add multiple cores later
    println!("| starting kernel_main");
    println!("| starting timer test");

    let mut start_time = device::system_timer::get_time();

    let first_time_slice = 0x100000;
    let mut iteration = 0;
    let iteration_slice = 0x100000;

    //testing that each interrupt occurs at the correct time
    let mut counter = 0;
    loop {
        let time = device::system_timer::get_time();
        if time - start_time > first_time_slice {
            counter += 1;
            println!("Time slice {counter} at time {time}");
            start_time = time;
        }
    }
}
