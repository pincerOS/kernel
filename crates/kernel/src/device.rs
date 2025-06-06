#[macro_use]
pub mod macros;

pub mod bcm2835_aux;
pub mod bcm2836_intc;
pub mod gic;
pub mod gpio;
pub mod mailbox;
pub mod rng;
pub mod sdcard;
pub mod system_timer;
pub mod uart;
pub mod usb;
pub mod watchdog;

use crate::device::usb::usbd::device::UsbBus;
use crate::memory;
use crate::memory::{map_device, map_device_block};
use crate::sync::{InterruptSpinLock, UnsafeInit};
use alloc::boxed::Box;
use alloc::vec::Vec;
use device_tree::format::StructEntry;
use device_tree::util::MappingIterator;
use device_tree::DeviceTree;
use usb::usbd::endpoint::endpoint_descriptor;

const ENABLE_USB: bool = true;

// TODO: a non-O(n²) approach to device discovery and registration
pub fn discover_compatible<'a, 'b>(
    tree: &DeviceTree<'a>,
    compatible: &'b [u8],
) -> Result<impl Iterator<Item = MappingIterator<'a>> + use<'a, 'b>, &'static str> {
    struct DiscoverIter<'a, 'b> {
        inner: MappingIterator<'a>,
        last_node_start: Option<MappingIterator<'a>>,
        target: &'b [u8],
    }

    impl<'a> Iterator for DiscoverIter<'a, '_> {
        type Item = MappingIterator<'a>;
        fn next(&mut self) -> Option<Self::Item> {
            loop {
                if self.inner.peek_token() == Some(StructEntry::FDT_BEGIN_NODE) {
                    let mut last = self.inner.clone();
                    last.stop_at_depth(last.current_depth());
                    self.last_node_start = Some(last);
                }
                let Some(entry) = self.inner.next() else {
                    break;
                };

                match entry.ok()? {
                    StructEntry::BeginNode { name: _ } => (),
                    StructEntry::EndNode => (),
                    StructEntry::Prop { name, data } => {
                        if name == "compatible" {
                            let mut parts =
                                data[..data.len().saturating_sub(1)].split(|b| *b == b'\x00');

                            if parts.any(|p| p == self.target) {
                                if let Some(node) = self.last_node_start.take() {
                                    return Some(node);
                                }
                            }
                        }
                    }
                }
            }
            None
        }
    }

    let iter = MappingIterator::new(tree.iter());
    Ok(DiscoverIter {
        inner: iter.clone(),
        last_node_start: None,
        target: compatible,
    })
}

pub fn find_device_addr(iter: MappingIterator) -> Result<Option<(usize, usize)>, &'static str> {
    let depth = iter.current_depth() + 1;
    let mut props = iter.into_props_iter(depth);
    while let Some(Ok((name, data))) = props.next() {
        if name == "reg" {
            let addr_size = props.parse_addr_size(data)?;
            if let Ok(m) = props.map_addr_size(addr_size) {
                return Ok(Some((m.addr as usize, m.size as usize)));
            }
            break;
        }
    }
    Ok(None)
}

pub static WATCHDOG: UnsafeInit<InterruptSpinLock<watchdog::bcm2835_wdt_driver>> =
    unsafe { UnsafeInit::uninit() };

type InitTask = Box<dyn Fn() + Send + Sync>;
pub static PER_CORE_INIT: UnsafeInit<Vec<InitTask>> = unsafe { UnsafeInit::uninit() };

pub static GPIO: UnsafeInit<InterruptSpinLock<gpio::bcm2711_gpio_driver>> =
    unsafe { UnsafeInit::uninit() };
pub static MAILBOX: UnsafeInit<InterruptSpinLock<mailbox::VideoCoreMailbox>> =
    unsafe { UnsafeInit::uninit() };

pub static BOX: UnsafeInit<Box<UsbBus>> = unsafe { UnsafeInit::uninit() };

pub fn init_devices(tree: &DeviceTree<'_>) {
    let mut init_fns: Vec<InitTask> = Vec::new();

    {
        let watchdog = discover_compatible(tree, b"brcm,bcm2835-pm-wdt")
            .unwrap()
            .next()
            .unwrap();
        let (watchdog_addr, _) = find_device_addr(watchdog).unwrap().unwrap();
        let watchdog_base = unsafe { map_device(watchdog_addr) }.as_ptr();

        unsafe {
            let watchdog = watchdog::bcm2835_wdt_driver::init(watchdog_base);
            WATCHDOG.init(InterruptSpinLock::new(watchdog));
        }
        // println!("| initialized power managment watchdog");
        // println!("| last reset: {:#010x}", WATCHDOG.get().lock().last_reset());
    }

    {
        let mailbox = discover_compatible(&tree, b"brcm,bcm2835-mbox")
            .unwrap()
            .next()
            .unwrap();
        let (mailbox_addr, _) = find_device_addr(mailbox).unwrap().unwrap();
        let mailbox_base = unsafe { memory::map_device(mailbox_addr) }.as_ptr();
        unsafe {
            MAILBOX.init(InterruptSpinLock::new(mailbox::VideoCoreMailbox::init(
                mailbox_base,
            )));
        }
    }

    {
        let gpio = discover_compatible(tree, b"brcm,bcm2711-gpio")
            .unwrap()
            .next()
            .unwrap();
        let (gpio_addr, _) = find_device_addr(gpio).unwrap().unwrap();
        let gpio_base = unsafe { map_device(gpio_addr) }.as_ptr();
        let gpio = unsafe { gpio::bcm2711_gpio_driver::init(gpio_base) };
        unsafe { GPIO.init(InterruptSpinLock::new(gpio)) };
    }

    let mut uarts = discover_compatible(tree, b"arm,pl011").unwrap();
    {
        let uart = uarts.next().unwrap();
        let (uart_addr, _) = find_device_addr(uart).unwrap().unwrap();
        let uart_base = unsafe { map_device(uart_addr) }.as_ptr();

        unsafe { uart::UART.init(uart::UARTLock::new(uart::UARTInner::new(uart_base))) };
        // println!("| initialized UART");
    }

    // TODO: don't hardcode which uart is used for prints

    let mut miniuarts = discover_compatible(tree, b"brcm,bcm2835-aux").unwrap();
    if let Some(miniuart) = miniuarts.next() {
        let (miniuart_addr, _) = find_device_addr(miniuart).unwrap().unwrap();
        let miniuart_base = unsafe { map_device(miniuart_addr) }.as_ptr();
        let miniuart = unsafe { bcm2835_aux::MiniUart::new(miniuart_base) };
        unsafe { bcm2835_aux::MINI_UART.init(bcm2835_aux::MiniUartLock::new(miniuart)) };
        println!("| initialized Mini UART");
    }

    {
        let mut guard = MAILBOX.get().lock();
        // TODO: this freezes after clock 11?
        // for clock in 0x01..=0x0E {
        //     // let state = unsafe { guard.get_property(mailbox::PropGetClockState { id: clock }).unwrap() };
        //     let rate = unsafe { guard.get_property(mailbox::PropGetClockRate { id: clock }).unwrap() };
        //     // let measured = unsafe { guard.get_property(mailbox::PropGetClockRateMeasured { id: clock }).unwrap() };
        //     let min = unsafe { guard.get_property(mailbox::PropGetMinClockRate { id: clock }).unwrap() };
        //     let max = unsafe { guard.get_property(mailbox::PropGetMaxClockRate { id: clock }).unwrap() };
        //     println!("Clock {clock}: state = {}, rate = {}, measured = {}, min = {}, max = {}", 0, rate.rate, 0, min.rate, max.rate);
        // }

        let clock_rate_req = mailbox::PropGetClockRate {
            id: mailbox::CLOCK_ARM,
        };
        let cur_rate = unsafe { guard.get_property(clock_rate_req).unwrap() }.rate;
        let max_rate_req = mailbox::PropGetMaxClockRate {
            id: mailbox::CLOCK_ARM,
        };
        let _max_rate = unsafe { guard.get_property(max_rate_req).unwrap() }.rate;
        let target_rate = 1_500_000_000;

        let set_rate_req = mailbox::PropSetClockRate {
            id: mailbox::CLOCK_ARM,
            rate: target_rate,
            skip_setting_turbo: 0,
        };
        println!("| Changing arm clock from {cur_rate} to {target_rate}");
        let new_rate = unsafe { guard.get_property(set_rate_req).unwrap() };
        println!("| Set clock rate; new rate = {}", new_rate.rate);
    }

    if let Some(gic) = discover_compatible(tree, b"arm,gic-400").unwrap().next() {
        println!("| initializing GIC-400 interrupt controller");
        let (gic_addr, _) = find_device_addr(gic).unwrap().unwrap();
        let gic_base = unsafe { map_device_block(gic_addr, 0x8000) }.as_ptr();

        println!("| GIC-400 addr: {:#010x}", gic_addr);

        let gic = unsafe { gic::Gic400Driver::init(gic_base) };
        unsafe { gic::GIC.init(gic) };

        unsafe { crate::event::exceptions::override_irq_handler(gic::gic_irq_handler) }

        init_fns.push(Box::new(|| {
            gic::GIC.get().init_per_core();
        }));
    } else if let Some(intc) = discover_compatible(tree, b"brcm,bcm2836-l1-intc")
        .unwrap()
        .next()
    {
        println!("| initializing local interrupt controller");
        let (intc_addr, _) = find_device_addr(intc).unwrap().unwrap();
        let intc_base = unsafe { map_device(intc_addr) }.as_ptr();
        println!("| INT controller addr: {:#010x}", intc_addr);
        println!("| INT controller base: {:#010x}", intc_base as usize);
        let intc = bcm2836_intc::bcm2836_l1_intc_driver::new(intc_base);
        unsafe { bcm2836_intc::LOCAL_INTC.init(intc) };

        unsafe {
            crate::event::exceptions::override_irq_handler(
                bcm2836_intc::exception_handler_bcm2836_intc_irq,
            )
        }
    }

    // println!("| acquiring framebuffer");
    // let mut surface = unsafe { MAILBOX.get().lock().map_framebuffer_kernel(640, 480) };
    // println!("| surface constructed");
    // let (width, height) = surface.dimensions();

    // for i in 0.. {
    //     println!("| drawing frame");
    //     let color = 0xFFFF0000 | (i as i32 % 512 - 256).abs().min(255) as u32;
    //     let color2 = 0xFF0000FF | (((i as i32 % 512 - 256).abs().min(255) as u32) << 16);
    //     let stripe_width = width / 20;
    //     let offset = i * (120 / surface.framerate());
    //     for r in 0..height {
    //         for c in 0..width {
    //             let cluster = (c + offset % (2 * stripe_width)) / stripe_width;
    //             let color = if cluster % 2 == 0 { color } else { color2 };
    //             surface[(r, c)] = color;
    //         }
    //     }

    //     surface.present();
    // }
    // let console = console::init();
    // unsafe { CONSOLE.init(InterruptSpinLock::new(console)) };
    // for i in 0.. {
    // println!("Line {i}");
    // console.input(alloc::format!("Line {i}\n").as_bytes());
    // console.render();
    // }

    {
        println!("| initializing timer");
        let timer_iter = discover_compatible(tree, b"brcm,bcm2835-system-timer")
            .unwrap()
            .next()
            .unwrap();
        let (timer_addr, _) = find_device_addr(timer_iter).unwrap().unwrap();
        let timer_base = unsafe { map_device(timer_addr) }.as_ptr();
        println!("| timer addr: {:#010x}", timer_addr);
        unsafe { system_timer::initialize_system_timer(timer_base) };
        let time = system_timer::get_time();
        println!("| timer initialized, time: {time}");
    }

    if ENABLE_USB {
        println!("| Initializing USB");
        let usb = discover_compatible(tree, b"brcm,bcm2835-usb")
            .unwrap()
            .next()
            .or_else(|| {
                discover_compatible(tree, b"brcm,bcm2708-usb")
                    .unwrap()
                    .next()
            })
            .unwrap();

        let (usb_addr, _) = find_device_addr(usb).unwrap().unwrap();
        let usb_base = unsafe { map_device_block(usb_addr, 0x11000) }.as_ptr(); //size is from core gloabl to dev ep 15

        let bus = usb::usb_init(usb_base);
        unsafe { BOX.init(bus) };
    }

    // Set up the interrupt controllers to preempt on the arm generic
    // timer interrupt.
    if gic::GIC.is_initialized() {
        init_fns.push(Box::new(|| {
            // gic::GIC.get().register_isr(30, timer_handler);
            gic::GIC
                .get()
                .register_isr(30, system_timer::timer_scheduler_handler);
        }));
    } else {
        let irq = bcm2836_intc::LOCAL_INTC.get();
        irq.isr_table
            .set(bcm2836_intc::IRQ_CNTPNSIRQ, timer_handler);

        init_fns.push(Box::new(|| {
            let id = crate::arch::core_id() & 0b11;
            irq.enable_irq_cntpnsirq(id as usize);
        }));
    }

    init_fns.push(Box::new(|| {
        // Run the generic timer at a 1ms interval for preemption
        system_timer::ARM_GENERIC_TIMERS.with_current(|timer| {
            timer.intialize_timer();
        });

        system_timer::TIMER_SCHEDULER.with_current(|timer_scheduler| {
            timer_scheduler.intialize_timer();
            timer_scheduler.add_timer_event(
                1,
                preemption_callback,
                endpoint_descriptor::new(),
                true,
            );
        });
    }));

    unsafe { PER_CORE_INIT.init(init_fns) };
}

fn preemption_callback(_endpoint: endpoint_descriptor) {
    //Do nothing
}

pub fn timer_handler(ctx: &mut crate::event::context::Context) {
    // TODO: will this break batched interrupts?
    // This will generally not return, and instead switch into the event loop
    unsafe { crate::event::timer_handler(ctx) };
}

/// Discovers and starts all cores, and returns the number of cores found.
pub fn enable_cpus(tree: &device_tree::DeviceTree<'_>, start_fn: unsafe extern "C" fn()) -> usize {
    use crate::memory::map_physical;
    use device_tree::format::StructEntry;
    use device_tree::util::find_node;

    let physical_start = crate::memory::physical_addr(start_fn as usize).unwrap();

    let mut core_count = 0;

    let mut iter = find_node(tree, "/cpus").unwrap().unwrap();
    iter.next(); // skip the BeginNode for "cpus" itself

    while let Some(Ok(entry)) = iter.next() {
        let StructEntry::BeginNode { name } = entry else {
            // skip inline properties
            continue;
        };

        let depth = iter.current_depth();
        let mut props = iter.into_props_iter(depth);

        let mut device_type = None;
        let mut method = None;
        let mut release_addr = None;
        while let Some(Ok((name, data))) = props.next() {
            match name {
                "device_type" => device_type = Some(data),
                "enable-method" => method = Some(data),
                "cpu-release-addr" => {
                    let addr: endian::u64_be = bytemuck::pod_read_unaligned(data);
                    release_addr = Some(addr.get() as usize);
                }
                _ => (),
            }
        }

        iter = props.into();

        match (device_type, method, release_addr) {
            (Some(b"cpu\0"), Some(b"spin-table\0"), Some(release_addr)) => {
                println!("| Waking cpu {:?}", name);

                let start = unsafe { map_physical(release_addr, 8).cast::<u64>() };
                unsafe { core::ptr::write_volatile(start.as_ptr(), physical_start) };
                unsafe { memory::invalidate_physical_buffer_for_device(start.as_ptr().cast(), 8) };

                core_count += 1;
            }
            (Some(b"cpu\0"), ..) => println!("| Could not wake cpu {:?}", name),
            _ => (),
        }
    }

    println!("| discovered {core_count} cores");
    println!("| started cores, waiting for init.");

    unsafe { crate::arch::sev() };

    core_count
}

pub fn init_devices_per_core() {
    let local = PER_CORE_INIT.get();
    for init in local {
        init();
    }
}
