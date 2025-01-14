#![allow(dead_code, nonstandard_style)]

use core::ptr;

mod be {
    #![allow(non_camel_case_types)]

    #[derive(Copy, Clone, PartialEq, Eq)]
    #[repr(transparent)]
    pub struct u32_be {
        _inner: u32,
    }
    pub const fn u32_be(val: u32) -> u32_be {
        u32_be {
            _inner: u32::to_be(val),
        }
    }
    impl u32_be {
        pub fn get(self) -> u32 {
            u32::from_be(self._inner)
        }
    }
    unsafe impl bytemuck::Pod for u32_be {}
    unsafe impl bytemuck::Zeroable for u32_be {}

    #[derive(Copy, Clone, PartialEq, Eq)]
    #[repr(transparent)]
    pub struct u64_be {
        _inner: u64,
    }
    pub const fn u64_be(val: u64) -> u64_be {
        u64_be {
            _inner: u64::to_be(val),
        }
    }
    impl u64_be {
        pub fn get(self) -> u64 {
            u64::from_be(self._inner)
        }
    }
    unsafe impl bytemuck::Pod for u64_be {}
    unsafe impl bytemuck::Zeroable for u64_be {}

    impl core::fmt::Debug for u32_be {
        fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
            core::fmt::Debug::fmt(&self.get(), f)
        }
    }
    impl core::fmt::Debug for u64_be {
        fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
            core::fmt::Debug::fmt(&self.get(), f)
        }
    }
}
pub use be::{u32_be, u64_be};

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
    const FDT_BEGIN_NODE: u32 = 0x00000001;
    const FDT_END_NODE: u32 = 0x00000002;
    const FDT_PROP: u32 = 0x00000003;
    const FDT_NOP: u32 = 0x00000004;
    const FDT_END: u32 = 0x00000009;
}

const DTB_VERSION: u32 = 17;

pub struct DeviceTree<'a> {
    pub header: fdt_header,
    data: &'a [u64],
    pub reserved_regions: &'a [fdt_reserve_entry],
}

/// Safety:
/// - `base` must be a pointer to a valid device tree
/// - `base` must be valid to construct an immutable reference to
pub unsafe fn load_device_tree<'a>(base: *const u64) -> Result<DeviceTree<'a>, &'static str> {
    if base as usize % size_of::<u64>() != 0 {
        return Err("device tree base not aligned to 8-byte boundary");
    }

    let magic = unsafe { ptr::read(base.cast::<u32_be>()) };
    if magic.get() != 0xd00dfeed {
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
pub fn iter_device_tree<'a>(
    tree: DeviceTree<'a>,
    emit: &mut dyn FnMut(StructEntry<'a>) -> Result<(), &'static str>,
) -> Result<(), &'static str> {
    let header = &tree.header;
    let data = tree.data;

    let strings_block: &[u8] = bytemuck::cast_slice(data);
    let strings_block = &strings_block[header.off_dt_strings.get() as usize..];

    let get_prop_name = |off| {
        let bytes = &strings_block[off..];
        // by spec this should have a max of 31 chars, but rpi violates that...
        let bytes = match bytes.iter().position(|c| *c == b'\0') {
            Some(i @ 1..) => &bytes[..i],
            Some(_) | None => return Err("invalid property name"),
        };
        verify_property_name(bytes).ok_or("invalid property name")
    };

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

    let mut i = 0;
    while i < struct_slice.len() {
        let token = struct_slice[i];
        i += 1;
        match token.get() {
            StructEntry::FDT_BEGIN_NODE => {
                let name_bytes: &[u8] = bytemuck::cast_slice(&struct_slice[i..]);
                // by spec this should have a min of 0 chars, but the root node voilates that
                let name_bytes = match name_bytes.iter().position(|c| *c == b'\0') {
                    Some(i @ 0..=31) => &name_bytes[..i],
                    Some(_) | None => return Err("invalid node name"),
                };
                let name_str = verify_node_name(name_bytes).ok_or("invalid node name")?;

                emit(StructEntry::BeginNode { name: name_str })?;

                i += (name_bytes.len() + 1).div_ceil(size_of::<u32>());
            }
            StructEntry::FDT_END_NODE => {
                emit(StructEntry::EndNode)?;
            }
            StructEntry::FDT_PROP => {
                let (Some(len), Some(nameoff)) = (struct_slice.get(i), struct_slice.get(i + 1))
                else {
                    return Err("invalid FDT_PROP field");
                };
                i += 2;

                let name = get_prop_name(nameoff.get() as usize)?;

                let len = len.get() as usize;
                let data: &[u8] = bytemuck::cast_slice(&struct_slice[i..]);
                let data = data
                    .get(..len)
                    .ok_or("invalid data in FDT_PROP field")?;

                emit(StructEntry::Prop { name, data })?;

                i += len.div_ceil(size_of::<u32>());
            }
            StructEntry::FDT_NOP => (),
            StructEntry::FDT_END => {
                if i != struct_slice.len() {
                    return Err("invalid FDT_END field");
                }
                break;
            }
            _ => return Err("invalid structure token"),
        }
    }

    Ok(())
}

fn verify_node_name(data: &[u8]) -> Option<&str> {
    if data.len() > 31 {
        return None;
    }
    let mut i = 0;
    while let Some(b'0'..=b'9' | b'a'..=b'z' | b'A'..=b'Z' | b',' | b'.' | b'_' | b'+' | b'-') =
        data.get(i)
    {
        i += 1;
    }
    match data.get(i) {
        Some(b'@') => i += 1,
        Some(_) => return None, // invalid char
        None => return Some(unsafe { core::str::from_utf8_unchecked(data) }),
    }
    while let Some(b'0'..=b'9' | b'a'..=b'z' | b'A'..=b'Z' | b',' | b'.' | b'_' | b'+' | b'-') =
        data.get(i)
    {
        i += 1;
    }
    match data.get(i) {
        Some(_) => None, // invalid char
        None => Some(unsafe { core::str::from_utf8_unchecked(data) }),
    }
}

fn verify_property_name(data: &[u8]) -> Option<&str> {
    if data.is_empty() {
        return None;
    }
    let mut i = 0;
    while let Some(
        b'0'..=b'9' | b'a'..=b'z' | b'A'..=b'Z' | b',' | b'.' | b'_' | b'+' | b'-' | b'?' | b'#',
    ) = data.get(i)
    {
        i += 1;
    }
    match data.get(i) {
        Some(_) => None, // invalid char
        None => Some(unsafe { core::str::from_utf8_unchecked(data) }),
    }
}

pub fn debug_device_tree(tree: DeviceTree<'_>) -> Result<(), &str> {
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

    iter_device_tree(tree, &mut |entry| {
        match entry {
            StructEntry::BeginNode { name } => {
                println!("{:width$}{}", "", name, width = depth * 2);
                depth += 1;
            }
            StructEntry::EndNode => {
                depth -= 1;
            }
            StructEntry::Prop { name, data } => {
                print!("{:width$}{}: ", "", name, width = depth * 2);

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
                        println!("{}", mapping.addr_cells);
                        return Ok(());
                    } else if name == "#size-cells" {
                        mapping.size_cells =
                            parse_u32_be(data).ok_or("invalid value for #cells property")?;
                        println!("{}", mapping.size_cells);
                        return Ok(());
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
                        print!("[");
                        let mut first = true;
                        for chunk in data {
                            if !first {
                                print!(", ");
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

                            print!("{{ child_addr = {:#0cawidth$x}, parent_addr = {:#0pawidth$x}, size = {:#0swidth$x} }}", caddr, paddr, size);
                        }
                        println!("]");
                        return Ok(());
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
                        println!(
                            "{{ addr = {:#0awidth$x}, size = {:#0swidth$x} }}",
                            addr, size
                        );
                        return Ok(());
                    }
                } else if name == "phandle" {
                    let phandle = parse_u32_be(data).ok_or("invalid value for property phandle")?;
                    println!("<{}>", phandle);
                    return Ok(());
                }

                for b in data {
                    for c in core::ascii::escape_default(*b) {
                        print!("{}", c as char);
                    }
                }
                println!();
            }
        }
        Ok(())
    })
}
