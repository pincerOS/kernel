#![allow(nonstandard_style)]

use core::ptr;
use endian::{u32_be, u64_be};

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
    pub const FDT_BEGIN_NODE: u32 = 0x0000_0001;
    pub const FDT_END_NODE: u32 = 0x0000_0002;
    pub const FDT_PROP: u32 = 0x0000_0003;
    pub const FDT_NOP: u32 = 0x0000_0004;
    pub const FDT_END: u32 = 0x0000_0009;
}

const DTB_VERSION: u32 = 17;
const DTB_MAGIC: u32 = 0xD00D_FEED;

pub struct DeviceTree<'a> {
    header: &'a fdt_header,
    data: &'a [u64],
    reserved_regions: &'a [fdt_reserve_entry],
    strings_block: &'a [u8],
    struct_block: &'a [u32_be],
}

impl<'a> DeviceTree<'a> {
    /// # Safety
    /// - `base` must be a pointer to a valid device tree
    /// - `base` must be valid to construct an immutable reference to
    pub unsafe fn load(base: *const u64) -> Result<DeviceTree<'a>, &'static str> {
        if base as usize % size_of::<u64>() != 0 {
            return Err("device tree base not aligned to 8-byte boundary");
        }

        let magic = unsafe { ptr::read(base.cast::<u32_be>()) };
        if magic.get() != DTB_MAGIC {
            return Err("invalid device tree magic bytes");
        }

        let header_ptr = base.cast::<fdt_header>();
        let header = unsafe { &*header_ptr };

        let size = header.totalsize.get() as usize;
        let data: &[u64] = unsafe { core::slice::from_raw_parts(base, size) };

        if header.last_comp_version.get() > DTB_VERSION {
            return Err("unsupported device tree version");
        }

        let strings_start = header.off_dt_strings.get() as usize;
        let strings_size = header.size_dt_strings.get() as usize;
        let strings_block = bytemuck::cast_slice(data)
            .get(strings_start..)
            .and_then(|s| s.get(..strings_size))
            .ok_or("invalid strings block size")?;

        let struct_offset = header.off_dt_struct.get() as usize;
        let struct_size = header.size_dt_struct.get() as usize;
        if struct_offset % size_of::<u32>() != 0 || struct_size % size_of::<u32>() != 0 {
            return Err("structure block misaligned");
        }
        let struct_block = {
            let u32_slice = bytemuck::cast_slice::<u64, u32_be>(data);
            let struct_start_idx = struct_offset / size_of::<u32_be>();
            let struct_size_idx = struct_size / size_of::<u32_be>();
            u32_slice
                .get(struct_start_idx..)
                .and_then(|s| s.get(..struct_size_idx))
                .ok_or("invalid structure block size")?
        };

        // Count entries in the Memory Reservation Block
        let reserved_map_offset = header.off_mem_rsvmap.get() as usize;
        if reserved_map_offset % size_of::<u64>() != 0 {
            return Err("reserved map misaligned");
        }
        let reserved_base = reserved_map_offset / size_of::<u64>();
        let max_idx = data.len().saturating_sub(reserved_base.saturating_mul(2));
        let count = (0..max_idx)
            .into_iter()
            .find(|i| {
                let base = reserved_base + i * 2;
                let addr = data[base];
                let size = data[base + 1];
                addr == 0 && size == 0
            })
            .unwrap_or(0);

        // Create slice of entries for Memory Reservation Block
        let regions_data = data
            .get(reserved_base..reserved_base + count * 2)
            .ok_or("invalid reserved map size")?;
        let reserved_regions: &[fdt_reserve_entry] = bytemuck::cast_slice(regions_data);

        Ok(DeviceTree {
            header,
            data,
            strings_block,
            struct_block,
            reserved_regions,
        })
    }

    pub fn header(&self) -> &fdt_header {
        &self.header
    }
    pub fn raw_data(&self) -> &'a [u64] {
        self.data
    }
    pub fn reserved_regions(&self) -> &'a [fdt_reserve_entry] {
        self.reserved_regions
    }
    pub fn strings_block(&self) -> &'a [u8] {
        self.strings_block
    }
    pub fn struct_block(&self) -> &'a [u32_be] {
        self.struct_block
    }

    /// Spec notes:
    /// - Does not validate the specific ordering of nodes within the structure
    ///   list (by the spec, BEGIN_NODE must not be immediately followed by
    ///   END_NODE)
    /// - Does not limit property names to 31 characters as required by the spec,
    ///   because the rpi3b dtb violates this
    /// - Allows empty node names, as the root node name is empty
    pub fn iter(&self) -> DeviceTreeIterator<'a> {
        DeviceTreeIterator {
            i: 0,
            struct_slice: self.struct_block,
            strings_block: self.strings_block,
        }
    }
}

#[derive(Clone)]
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
    pub fn peek_token(&self) -> Option<u32> {
        self.struct_slice.get(self.i).map(|b| b.get())
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
