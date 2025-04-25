use crate::device::usb::device::net::get_interface_mut;
use crate::event::thread;
use crate::sync;

use crate::networking::socket::TaggedSocket;

use alloc::vec::Vec;

pub fn socket_send_loop() {
    let interface = get_interface_mut();

    let to_send: Vec<_> = {
        let mut sockets = interface.sockets.lock();
        sockets
            .iter_mut()
            .map(|(_, socket)| socket as *mut TaggedSocket)
            .collect()
    };

    for &socket_ptr in &to_send {
        let socket: &mut TaggedSocket = unsafe { &mut *socket_ptr };
        let _ = socket.send(interface);
    }

    // WARN: this is not good
    thread::thread(move || {
        sync::spin_sleep(500_000);
        socket_send_loop();
    });
}
