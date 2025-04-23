use crate::device::usb::device::net::interface;
use crate::event::thread;
use crate::sync;

pub fn socket_send_loop() {
    for (_, socket) in &mut interface().sockets {
        let _ = socket.send();
    }

    // WARN: this is not good
    thread::thread(move || {
        sync::spin_sleep(500_000);
        socket_send_loop();
    });
}
