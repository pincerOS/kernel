use crate::{shutdown, SpinLock};

//https://elixir.bootlin.com/freebsd/v14.2/source/sys/dev/usb/usb_hub.c#L957
pub fn uhub_root_intr() {
    println!("| USB: uhub_root_intr");
    println!("| FUnction not implemented");
    //not too sure what this does, TODO: implement
}