#[macro_use]
pub mod uart;
pub mod bcm2835_aux;
pub mod gic;
pub mod mailbox;
pub mod timer;
pub mod watchdog;

use device_tree::format::StructEntry;
use device_tree::util::MappingIterator;
use device_tree::DeviceTree;

use crate::device::gic::gic_init;
use crate::memory::{map_device, map_device_block};
use crate::sync::{InterruptSpinLock, UnsafeInit};
use crate::SpinLock;

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

    impl<'a, 'b> Iterator for DiscoverIter<'a, 'b> {
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
            if let Some(m) = props.map_addr_size(addr_size).ok() {
                return Ok(Some((m.addr as usize, m.size as usize)));
            }
            break;
        }
    }
    Ok(None)
}

pub static WATCHDOG: UnsafeInit<SpinLock<watchdog::bcm2835_wdt_driver>> =
    unsafe { UnsafeInit::uninit() };

pub static IRQ_CONTROLLER: UnsafeInit<InterruptSpinLock<timer::bcm2836_l1_intc_driver>> =
    unsafe { UnsafeInit::uninit() };

pub fn init_devices(tree: &DeviceTree<'_>) {
    let mut uarts = discover_compatible(tree, b"arm,pl011").unwrap();
    {
        let uart = uarts.next().unwrap();
        let (uart_addr, _) = find_device_addr(uart).unwrap().unwrap();
        let uart_base = unsafe { map_device(uart_addr) }.as_ptr();

        unsafe { uart::UART.init(SpinLock::new(uart::UARTInner::new(uart_base))) };
        println!("| initialized UART");
    }

    let mut miniuarts = discover_compatible(tree, b"brcm,bcm2835-aux").unwrap();
    if let Some(miniuart) = miniuarts.next() {
        use core::fmt::Write;
        let (miniuart_addr, _) = find_device_addr(miniuart).unwrap().unwrap();
        let miniuart_base = unsafe { map_device(miniuart_addr) }.as_ptr();
        let mut miniuart = unsafe { bcm2835_aux::MiniUart::new(miniuart_base) };
        writeln!(miniuart, "| initialized Mini UART (bcm2835-aux)").ok();
        println!("| initialized Mini UART");
    }

    {
        let watchdog = discover_compatible(tree, b"brcm,bcm2835-pm-wdt")
            .unwrap()
            .next()
            .unwrap();
        let (watchdog_addr, _) = find_device_addr(watchdog).unwrap().unwrap();
        let watchdog_base = unsafe { map_device(watchdog_addr) }.as_ptr();

        unsafe {
            let watchdog = watchdog::bcm2835_wdt_driver::init(watchdog_base);
            WATCHDOG.init(SpinLock::new(watchdog));
        }
        println!("| initialized power managment watchdog");
        println!("| last reset: {:#010x}", WATCHDOG.get().lock().last_reset());
    }

    {
        println!("| initializing interrupt controller");
        let intc = discover_compatible(tree, b"brcm,bcm2836-l1-intc")
            .unwrap()
            .next()
            .unwrap();
        let (intc_addr, _) = find_device_addr(intc).unwrap().unwrap();
        let intc_base = unsafe { map_device(intc_addr) }.as_ptr();
        println!("| INT controller addr: {:#010x}", intc_addr as usize);
        println!("| INT controller base: {:#010x}", intc_base as usize);
        let intc = timer::bcm2836_l1_intc_driver::new(intc_base);
        let intc = InterruptSpinLock::new(intc);
        unsafe { IRQ_CONTROLLER.init(intc) };
    }

    {
        println!("| initializing GIC-400 interrupt controller");
        let gic_iter = discover_compatible(tree, b"arm,gic-400")
            .unwrap()
            .next()
            .unwrap();
        let (gic_addr, _) = find_device_addr(gic_iter).unwrap().unwrap();
        let gic_base = unsafe { map_device_block(gic_addr, 0x8000) }.as_ptr();

        println!("| GIC-400 addr: {:#010x}", gic_addr as usize);
        println!("| GIC-400 base: {:#010x}", gic_base as usize);
        unsafe { gic_init(gic_base) };
    }
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
