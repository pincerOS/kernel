#![no_std]
#![no_main]

extern crate alloc;
extern crate kernel;

use core::ffi::c_char;
use core::slice;

use kernel::*;
use memory::map_physical;

#[no_mangle]
extern "Rust" fn kernel_main(device_tree: device_tree::DeviceTree<'static>) {
    println!("| starting kernel_main");
    crate::event::task::spawn_async(async move {
        main(device_tree).await;
    });
    crate::event::thread::stop();
}

async fn main(tree: device_tree::DeviceTree<'static>) {
    let usb = device::discover_compatible(&tree, b"brcm,bcm2835-usb")
        .unwrap()
        .next()
        .or_else(|| {
            device::discover_compatible(&tree, b"brcm,bcm2708-usb")
                .unwrap()
                .next()
        })
        .unwrap();

    {
        let mut uart = device::uart::UART.get().lock();
        device_tree::debug::debug_node(usb.clone(), &mut *uart).unwrap();
    }

    let (usb_addr, _) = device::find_device_addr(usb).unwrap().unwrap();
    println!("| Found usb device at {usb_addr:#x}");
    unsafe { libcsud::UsbInitialise(usb_addr as *mut ()) };

    println!("| Usb init done; checking changes");

    unsafe { libcsud::UsbCheckForChange() };

    println!("| usb check done");
    println!("Kbd count: {}", unsafe { libcsud::KeyboardCount() });
}

#[repr(C)]
struct HeapHeader {
    len: usize,
    pad: usize,
}

#[no_mangle]
unsafe extern "C" fn MemoryAllocate(len: u32) -> *mut () {
    let align = 16;
    let size = len.next_multiple_of(16) as usize + 16;
    let metadata = HeapHeader {
        len: size,
        pad: 0xA010F48D14F4FA5B,
    };
    let layout = core::alloc::Layout::from_size_align(size, align).unwrap();
    let ptr = unsafe { alloc::alloc::alloc_zeroed(layout) };
    unsafe { core::ptr::write(ptr.cast::<HeapHeader>(), metadata) };
    let ptr = unsafe { ptr.byte_add(size_of::<HeapHeader>()).cast() };
    println!("MemoryAllocate({}) -> {:p}", len, ptr);
    ptr
}

#[no_mangle]
unsafe extern "C" fn MemoryDeallocate(addr: *mut ()) {
    println!("MemoryDeallocate({addr:p})");
    let header_ptr = unsafe { addr.byte_sub(size_of::<HeapHeader>()) }.cast::<HeapHeader>();
    let header = unsafe { header_ptr.read() };
    assert!(header.pad == 0xA010F48D14F4FA5B);
    let len = header.len;
    let align = 16;
    let layout = core::alloc::Layout::from_size_align(len, align).unwrap();
    unsafe { alloc::alloc::dealloc(header_ptr.cast(), layout) };
}

#[no_mangle]
unsafe extern "C" fn MemoryReserve(len: u32, pa: usize) -> *mut () {
    println!("csud: MemoryReserve(len: {}, pa: {:#x})", len, pa);
    let addr = map_physical(pa, len as usize);
    addr.as_ptr()
}

#[no_mangle]
unsafe extern "C" fn MemoryCopy(dst: *mut (), src: *const (), len: u32) {
    unsafe { core::ptr::copy(src.cast::<u8>(), dst.cast::<u8>(), len as usize) };
}

#[no_mangle]
unsafe extern "C" fn LogPrint(message: *const c_char, message_length: u32) {
    let text = unsafe { slice::from_raw_parts(message.cast::<u8>(), message_length as usize) };
    let str = core::str::from_utf8(text).unwrap();
    print!("{}", str);
}

#[no_mangle]
unsafe extern "C" fn PowerOnUsb() -> i32 {
    println!("csud: PowerOnUsb()");

    let msg = device::mailbox::PropSetPowerState {
        device_id: 0x03,
        state: 1 | (1 << 1),
    };

    let resp;
    {
        let mut mailbox = device::MAILBOX.get().lock();
        resp = unsafe { mailbox.get_property::<device::mailbox::PropSetPowerState>(msg) };
    }

    //TODO: Ignore on QEMU for now
    match resp {
        Ok(output) => {
            println!("| csud HCD: Power on successful {}", output.state);
        }
        Err(_) => {
            println!("| csud HCD ERROR: Power on failed");
        }
    }

    return 0;
}

#[no_mangle]
unsafe extern "C" fn PowerOffUsb() {
    println!("csud: PowerOffUsb()");
}

#[no_mangle]
unsafe extern "C" fn MicroDelay(delay: u32) {
    sync::spin_sleep(delay as usize);
}
