#[macro_use]
pub mod uart;
pub mod mailbox;
pub mod timer;
pub mod watchdog;

use device_tree::format::StructEntry;
use device_tree::util::MappingIterator;
use device_tree::DeviceTree;

use crate::memory::map_device;
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

pub fn find_device_addr(
    tree: &DeviceTree<'_>,
    compatible: &[u8],
) -> Result<Option<(usize, usize)>, &'static str> {
    let mut result = None;
    'outer: for mut iter in discover_compatible(tree, compatible)? {
        while let Some(Ok(entry)) = iter.next() {
            match entry {
                StructEntry::Prop { name: "reg", data } => {
                    let addr_size = iter.parse_addr_size(data)?;
                    match iter.map_addr_size(addr_size) {
                        Ok(m) => {
                            result = Some((m.addr as usize, m.size as usize));
                            break 'outer;
                        }
                        Err(_) => (),
                    }
                    break;
                }
                _ => (),
            }
        }
    }
    Ok(result)
}

pub static WATCHDOG: UnsafeInit<SpinLock<watchdog::bcm2835_wdt_driver>> =
    unsafe { UnsafeInit::uninit() };

pub static IRQ_CONTROLLER: UnsafeInit<InterruptSpinLock<timer::bcm2836_l1_intc_driver>> =
    unsafe { UnsafeInit::uninit() };

pub fn init_devices(tree: &DeviceTree<'_>) {
    {
        let (uart_addr, _) = find_device_addr(tree, b"arm,pl011").unwrap().unwrap();
        let uart_base = unsafe { map_device(uart_addr) }.as_ptr();

        unsafe { uart::UART.init(SpinLock::new(uart::UARTInner::new(uart_base))) };
        println!("| initialized UART");
    }

    {
        let (watchdog_addr, _) = find_device_addr(tree, b"brcm,bcm2835-pm-wdt")
            .unwrap()
            .unwrap();
        let watchdog_base = unsafe { map_device(watchdog_addr) }.as_ptr();

        unsafe {
            let watchdog = watchdog::bcm2835_wdt_driver::init(watchdog_base);
            WATCHDOG.init(SpinLock::new(watchdog));
        }
        println!("| initialized power managment watchdog");
        println!("| last reset: {:#08x}", WATCHDOG.get().lock().last_reset());
    }

    {
        println!("| initializing interrupt controller");
        let (intc_addr, _) = find_device_addr(tree, b"brcm,bcm2836-l1-intc")
            .unwrap()
            .unwrap();
        let intc_base = unsafe { map_device(intc_addr) }.as_ptr();
        let intc = timer::bcm2836_l1_intc_driver::new(intc_base);
        let intc = InterruptSpinLock::new(intc);
        unsafe { IRQ_CONTROLLER.init(intc) };
    }
}
