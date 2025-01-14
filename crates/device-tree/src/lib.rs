#![no_std]
#![allow(nonstandard_style)]

#[cfg(test)]
extern crate std;

mod be;

pub use be::{u32_be, u64_be};
use core::ptr;

#[derive(Copy, Clone)]
#[repr(C)]
pub struct fdt_header {
    pub magic: u32_be,
    pub totalsize: u32_be,
    pub off_dt_struct: u32_be,
    pub off_dt_strings: u32_be,
    pub off_mem_rsvmap: u32_be,
    pub version: u32_be,
    pub last_comp_version: u32_be,
    pub boot_cpuid_phys: u32_be,
    pub size_dt_strings: u32_be,
    pub size_dt_struct: u32_be,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct fdt_reserve_entry {
    pub address: u64_be,
    pub size: u64_be,
}

unsafe impl bytemuck::Pod for fdt_header {}
unsafe impl bytemuck::Zeroable for fdt_header {}
unsafe impl bytemuck::Pod for fdt_reserve_entry {}
unsafe impl bytemuck::Zeroable for fdt_reserve_entry {}

#[derive(Debug)]
pub enum StructEntry<'a> {
    BeginNode { name: &'a str },
    EndNode,
    Prop { name: &'a str, data: &'a [u8] },
}

impl StructEntry<'_> {
    const FDT_BEGIN_NODE: u32 = 0x0000_0001;
    const FDT_END_NODE: u32 = 0x0000_0002;
    const FDT_PROP: u32 = 0x0000_0003;
    const FDT_NOP: u32 = 0x0000_0004;
    const FDT_END: u32 = 0x0000_0009;
}

const DTB_VERSION: u32 = 17;
const DTB_MAGIC: u32 = 0xD00D_FEED;

pub struct DeviceTree<'a> {
    pub header: fdt_header,
    data: &'a [u64],
    pub reserved_regions: &'a [fdt_reserve_entry],
}

/// # Safety
/// - `base` must be a pointer to a valid device tree
/// - `base` must be valid to construct an immutable reference to
pub unsafe fn load_device_tree<'a>(base: *const u64) -> Result<DeviceTree<'a>, &'static str> {
    if base as usize % size_of::<u64>() != 0 {
        return Err("device tree base not aligned to 8-byte boundary");
    }

    let magic = unsafe { ptr::read(base.cast::<u32_be>()) };
    if magic.get() != DTB_MAGIC {
        return Err("invalid device tree magic bytes");
    }

    let header_ptr = base.cast::<fdt_header>();
    let header = unsafe { ptr::read(header_ptr) };

    let size = header.totalsize.get() as usize;
    let data: &[u64] = unsafe { core::slice::from_raw_parts(base, size) };

    if header.last_comp_version.get() > DTB_VERSION {
        return Err("unsupported device tree version");
    }

    // Count entries in the Memory Reservation Block
    let reserved_base = header.off_mem_rsvmap.get() as usize / size_of::<u64>();
    let count = (0..)
        .into_iter()
        .find(|i| {
            let base = reserved_base + i * 2;
            let addr = data[base];
            let size = data[base + 1];
            addr == 0 && size == 0
        })
        .unwrap_or(0);

    // Create slice of entries for Memory Reservation Block
    let regions_data = &data[reserved_base..reserved_base + count * 2];
    let reserved_regions: &[fdt_reserve_entry] = bytemuck::cast_slice(regions_data);

    Ok(DeviceTree {
        header,
        data,
        reserved_regions,
    })
}

/// Spec notes:
/// - Does not validate the specific ordering of nodes within the structure
///   list (by the spec, BEGIN_NODE must not be immediately followed by
///   END_NODE)
/// - Does not limit property names to 31 characters as required by the spec,
///   because the rpi3b dtb violates this
/// - Allows empty node names, as the root node name is empty
pub fn iter_device_tree(tree: DeviceTree) -> Result<DeviceTreeIterator, &'static str> {
    let header = &tree.header;
    let data = tree.data;

    let strings_block: &[u8] = bytemuck::cast_slice(data);
    let strings_block = &strings_block[header.off_dt_strings.get() as usize..];

    let struct_offset = header.off_dt_struct.get() as usize;
    let struct_size = header.size_dt_struct.get() as usize;

    if struct_offset % size_of::<u32>() != 0 {
        return Err("structure block misaligned");
    }
    if struct_size % size_of::<u32>() != 0 {
        return Err("structure block size misaligned");
    }

    let struct_slice = {
        let u32_slice = bytemuck::cast_slice::<u64, u32_be>(data);
        let base = struct_offset / size_of::<u32_be>();
        let end = base + struct_size / size_of::<u32_be>();
        &u32_slice[base..end]
    };

    Ok(DeviceTreeIterator {
        i: 0,
        struct_slice,
        strings_block,
    })
}

pub struct DeviceTreeIterator<'a> {
    i: usize,
    struct_slice: &'a [u32_be],
    strings_block: &'a [u8],
}

impl<'a> DeviceTreeIterator<'a> {
    fn get_prop_name(&self, off: usize) -> Result<&'a str, &'static str> {
        let bytes = &self.strings_block[off..];
        // by spec this should have a max of 31 chars, but rpi violates that...
        let bytes = match bytes.iter().position(|c| *c == b'\0') {
            Some(i @ 1..) => &bytes[..i],
            Some(_) | None => return Err("invalid property name"),
        };
        verify_property_name(bytes).ok_or("invalid property name")
    }
}

impl<'a> Iterator for DeviceTreeIterator<'a> {
    type Item = Result<StructEntry<'a>, &'static str>;
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let token = self.struct_slice.get(self.i)?;
            self.i += 1;
            match token.get() {
                StructEntry::FDT_BEGIN_NODE => {
                    let name_bytes: &[u8] = bytemuck::cast_slice(&self.struct_slice[self.i..]);
                    // by spec this should have a min of 0 chars, but the root node voilates that
                    let name_bytes = match name_bytes.iter().position(|c| *c == b'\0') {
                        Some(i @ 0..=31) => &name_bytes[..i],
                        Some(_) | None => return Some(Err("invalid node name")),
                    };
                    self.i += (name_bytes.len() + 1).div_ceil(size_of::<u32>());

                    let name_str = verify_node_name(name_bytes).ok_or("invalid node name");
                    return Some(name_str.map(|name| StructEntry::BeginNode { name }));
                }
                StructEntry::FDT_END_NODE => {
                    return Some(Ok(StructEntry::EndNode));
                }
                StructEntry::FDT_PROP => {
                    let (Some(len), Some(nameoff)) = (
                        self.struct_slice.get(self.i),
                        self.struct_slice.get(self.i + 1),
                    ) else {
                        return Some(Err("invalid FDT_PROP field"));
                    };
                    self.i += 2;

                    let name = self.get_prop_name(nameoff.get() as usize);

                    let len = len.get() as usize;
                    let data: &[u8] = bytemuck::cast_slice(&self.struct_slice[self.i..]);
                    let data = data.get(..len).ok_or("invalid data in FDT_PROP field");

                    self.i += len.div_ceil(size_of::<u32>());

                    let entry = name
                        .and_then(|n| Ok((n, data?)))
                        .map(|(name, data)| StructEntry::Prop { name, data });
                    return Some(entry);
                }
                StructEntry::FDT_END => {
                    if self.i != self.struct_slice.len() {
                        return Some(Err("invalid FDT_END field"));
                    }
                    return None;
                }
                StructEntry::FDT_NOP => {}
                _ => return Some(Err("invalid structure token")),
            }
        }
    }
}

fn take_while(input: &[u8], f: impl Fn(u8) -> bool) -> (&[u8], &[u8]) {
    for (i, b) in input.iter().enumerate() {
        if !f(*b) {
            return input.split_at(i);
        }
    }
    (input, &[])
}

fn valid_node_char(b: u8) -> bool {
    matches!(b, b'0'..=b'9' | b'a'..=b'z' | b'A'..=b'Z' | b',' | b'.' | b'_' | b'+' | b'-')
}

fn verify_node_name(mut data: &[u8]) -> Option<&str> {
    let orig = data;
    if data.len() > 31 {
        return None;
    }
    (_, data) = take_while(data, valid_node_char);
    data = match data {
        [b'@', rest @ ..] => rest,
        [] => &[],
        _ => return None,
    };
    (_, data) = take_while(data, valid_node_char);
    if !data.is_empty() {
        return None;
    }
    Some(unsafe { core::str::from_utf8_unchecked(orig) })
}

fn verify_property_name(data: &[u8]) -> Option<&str> {
    let orig = data;
    if data.is_empty() {
        return None;
    }
    let (_, data) = take_while(
        data,
        |b| matches!(b, b'0'..=b'9' | b'a'..=b'z' | b'A'..=b'Z' | b',' | b'.' | b'_' | b'+' | b'-' | b'?' | b'#'),
    );
    if !data.is_empty() {
        return None;
    }
    Some(unsafe { core::str::from_utf8_unchecked(orig) })
}

#[derive(Debug, Copy, Clone)]
enum FieldType {
    Ascii,
    UnsignedInt,
    Unknown,
}

fn guess_field_type(data: &[u8]) -> FieldType {
    if data.is_empty() {
        FieldType::Unknown
    } else if data.last() == Some(&0x00)
        && data[..data.len() - 1]
            .iter()
            .all(|c| c.is_ascii() && !c.is_ascii_control())
    {
        FieldType::Ascii
    } else if data.len() == 4 || data.len() == 8 {
        FieldType::UnsignedInt
    } else {
        FieldType::Unknown
    }
}

fn fmt_field(data: &[u8], field_type: FieldType) -> impl core::fmt::Display + '_ {
    struct Formatter<'a> {
        data: &'a [u8],
        field_type: FieldType,
    }
    impl core::fmt::Display for Formatter<'_> {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            match self.field_type {
                FieldType::Ascii => {
                    let s = core::str::from_utf8(&self.data[..self.data.len() - 1]).unwrap();
                    write!(f, "{:?}", s)
                }
                FieldType::UnsignedInt if self.data.len() <= 16 => {
                    let mut bytes = [0; 16];
                    bytes[16 - self.data.len()..].copy_from_slice(self.data);
                    let value = u128::from_be_bytes(bytes);
                    let width = self.data.len() * 2;
                    write!(f, "{:#0width$x}", value)
                }
                _ => {
                    for c in self
                        .data
                        .iter()
                        .flat_map(|c| core::ascii::escape_default(*c))
                    {
                        write!(f, "{}", c as char)?;
                    }
                    Ok(())
                }
            }
        }
    }
    Formatter { data, field_type }
}

pub fn debug_device_tree(
    tree: DeviceTree<'_>,
    out: &mut dyn core::fmt::Write,
) -> Result<(), &'static str> {
    let mut depth = 0usize;

    #[derive(Copy, Clone)]
    struct Mapping<'a> {
        addr_cells: u32,
        size_cells: u32,
        #[allow(unused)]
        mapping: Option<&'a [u32_be]>,
    }

    let mut addr_mappings = [Mapping {
        addr_cells: 2,
        size_cells: 1,
        mapping: None,
    }; 8];

    for entry in iter_device_tree(tree)? {
        match entry? {
            StructEntry::BeginNode { name } => {
                writeln!(out, "{:width$}{}", "", name, width = depth * 2).ok();
                depth += 1;
            }
            StructEntry::EndNode => {
                depth -= 1;
            }
            StructEntry::Prop { name, data } => {
                write!(out, "{:width$}{}: ", "", name, width = depth * 2).ok();

                let parent_addr_cells =
                    match depth.checked_sub(1).and_then(|i| addr_mappings.get(i)) {
                        Some(map) => map.addr_cells as usize,
                        None => 2,
                    };

                fn parse_u32_be_array(slice: &[u32_be]) -> u128 {
                    slice
                        .iter()
                        .fold(0u128, |acc, addr| (acc << 32) | addr.get() as u128)
                }
                fn parse_u32_be(slice: &[u8]) -> Option<u32> {
                    Some(u32::from_be_bytes(slice.try_into().ok()?))
                }

                if let Some(mapping) = addr_mappings.get_mut(depth) {
                    if name == "#address-cells" {
                        mapping.addr_cells =
                            parse_u32_be(data).ok_or("invalid value for #cells property")?;
                        writeln!(out, "{}", mapping.addr_cells).ok();
                        continue;
                    } else if name == "#size-cells" {
                        mapping.size_cells =
                            parse_u32_be(data).ok_or("invalid value for #cells property")?;
                        writeln!(out, "{}", mapping.size_cells).ok();
                        continue;
                    } else if name == "ranges" {
                        let slice = bytemuck::try_cast_slice::<_, u32_be>(data)
                            .map_err(|_| "Invalid `ranges` field: wrong cell count")?;

                        let addr_cells = mapping.addr_cells as usize;
                        let size_cells = mapping.size_cells as usize;
                        let total_cells = parent_addr_cells + addr_cells + size_cells;

                        if slice.len() % total_cells != 0 {
                            return Err("Invalid `ranges` field: wrong cell count");
                        }
                        let data = slice.chunks(total_cells);
                        write!(out, "[").ok();
                        let mut first = true;
                        for chunk in data {
                            if !first {
                                write!(out, ", ").ok();
                            }
                            first = false;
                            let bound1 = addr_cells;
                            let bound2 = addr_cells + parent_addr_cells;
                            assert!(bound1 <= chunk.len() && bound2 <= chunk.len());

                            let caddr = parse_u32_be_array(&chunk[..bound1]);
                            let paddr = parse_u32_be_array(&chunk[bound1..bound2]);
                            let size = parse_u32_be_array(&chunk[bound2..]);

                            let pawidth = parent_addr_cells * 8 + 2;
                            let cawidth = addr_cells * 8 + 2;
                            let swidth = size_cells * 8 + 2;

                            write!(out, "{{ child_addr = {:#0cawidth$x}, parent_addr = {:#0pawidth$x}, size = {:#0swidth$x} }}", caddr, paddr, size).ok();
                        }
                        writeln!(out, "]").ok();
                        continue;
                    }
                }

                if name == "reg" {
                    // TODO: error handling
                    if let Some(parent_mapping) = addr_mappings.get_mut(depth - 1) {
                        let slice = bytemuck::try_cast_slice::<_, u32_be>(data)
                            .map_err(|_| "Invalid `reg` field: wrong cell count")?;

                        let addr_cells = parent_mapping.addr_cells as usize;
                        let size_cells = parent_mapping.size_cells as usize;
                        let total_cells = addr_cells + size_cells;
                        if slice.len() % total_cells != 0 {
                            return Err("Invalid `reg` field: wrong cell count");
                        }
                        let addr = parse_u32_be_array(slice.get(..addr_cells).expect("TODO"));
                        let size =
                            parse_u32_be_array(slice.get(addr_cells..total_cells).expect("TODO"));

                        let awidth = addr_cells * 8 + 2;
                        let swidth = size_cells * 8 + 2;
                        writeln!(
                            out,
                            "{{ addr = {:#0awidth$x}, size = {:#0swidth$x} }}",
                            addr, size
                        )
                        .ok();
                        continue;
                    }
                } else if name == "phandle" {
                    let phandle = parse_u32_be(data).ok_or("invalid value for property phandle")?;
                    writeln!(out, "<{}>", phandle).ok();
                    continue;
                } else {
                    let ty = guess_field_type(data);
                    write!(out, "{}", fmt_field(data, ty)).ok();
                }
                writeln!(out).ok();
            }
        }
    }

    Ok(())
}

#[test]
fn print_device_tree() -> Result<(), &'static str> {
    use std::prelude::rust_2021::*;

    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    let path = root.join("../../bcm2710-rpi-3-b-plus.dtb");
    let mut data = std::fs::read(path).unwrap();
    data.resize(data.len().next_multiple_of(8), 0);
    let data = bytemuck::cast_slice(&*data);

    let tree = unsafe { load_device_tree(data.as_ptr()).unwrap() };

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

    debug_device_tree(tree, &mut WriteWrapper(std::io::stdout()))?;

    Ok(())
}
