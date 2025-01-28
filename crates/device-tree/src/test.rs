#[cfg(test)]
extern crate std;
use std::prelude::rust_2021::*;

use crate::debug::{debug_device_tree, debug_node};
use crate::util::find_node;
use crate::DeviceTree;

struct WriteWrapper<W>(W);
impl<W> core::fmt::Write for WriteWrapper<W>
where
    W: std::io::Write,
{
    fn write_fmt(&mut self, args: core::fmt::Arguments<'_>) -> core::fmt::Result {
        self.0.write_fmt(args).map_err(|_| core::fmt::Error)
    }
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        self.0.write_all(s.as_bytes()).map_err(|_| core::fmt::Error)
    }
}

#[test]
fn rpi3b_tree_find() -> Result<(), &'static str> {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    let mut data = std::fs::read(root.join("../kernel/bcm2710-rpi-3-b-plus.dtb")).unwrap();
    data.resize(data.len().next_multiple_of(8), 0);
    let data = bytemuck::cast_slice(&*data);

    let tree = unsafe { DeviceTree::load(data.as_ptr()).unwrap() };

    let res = find_node(&tree, "/cpus")?.unwrap();
    debug_node(res, &mut WriteWrapper(std::io::stdout()))?;

    Ok(())
}

#[test]
fn print_rpi3b_tree() -> Result<(), &'static str> {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    let mut data = std::fs::read(root.join("../kernel/bcm2710-rpi-3-b-plus.dtb")).unwrap();
    data.resize(data.len().next_multiple_of(8), 0);
    let data = bytemuck::cast_slice(&*data);

    let tree = unsafe { DeviceTree::load(data.as_ptr()).unwrap() };
    debug_device_tree(&tree, &mut WriteWrapper(std::io::stdout()))?;

    Ok(())
}

#[test]
fn print_rpi4b_tree() -> Result<(), &'static str> {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    let mut data = std::fs::read(root.join("../kernel/bcm2711-rpi-4-b.dtb")).unwrap();
    data.resize(data.len().next_multiple_of(8), 0);
    let data = bytemuck::cast_slice(&*data);

    let tree = unsafe { DeviceTree::load(data.as_ptr()).unwrap() };
    debug_device_tree(&tree, &mut WriteWrapper(std::io::stdout()))?;

    Ok(())
}
