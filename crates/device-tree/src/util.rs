use endian::u32_be;

use crate::format::{DeviceTree, DeviceTreeIterator, StructEntry};

pub fn parse_u32_be_array(slice: &[u32_be]) -> u128 {
    slice
        .iter()
        .fold(0u128, |acc, addr| (acc << 32) | addr.get() as u128)
}

pub fn parse_u32_be(slice: &[u8]) -> Option<u32> {
    Some(u32::from_be_bytes(*slice.split_first_chunk()?.0))
}

#[derive(Copy, Clone, Debug)]
pub struct Mapping<'a> {
    pub addr_cells: u8,
    pub size_cells: u8,
    pub parent_addr_cells: u8,
    pub mapping: Option<&'a [u32_be]>,
}

const DEFAULT_MAPPING: Mapping<'static> = Mapping {
    addr_cells: 2,
    size_cells: 1,
    parent_addr_cells: 2,
    mapping: None,
};

#[derive(Clone)]
pub struct MappingIterator<'a> {
    inner: DeviceTreeIterator<'a>,
    depth: usize,
    min_depth: usize,
    addr_mappings: [Mapping<'a>; 8],
}

#[derive(Copy, Clone)]
pub struct AddrSizeField {
    pub addr: u128,
    pub size: u128,
    pub addr_cells: u8,
    pub size_cells: u8,
}

pub struct MapRangeField {
    pub child_addr: u128,
    pub parent_addr: u128,
    pub size: u128,
    pub child_addr_cells: u8,
    pub parent_addr_cells: u8,
    pub size_cells: u8,
}

impl core::fmt::Debug for AddrSizeField {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let awidth = self.addr_cells as usize * 8 + 2;
        let swidth = self.size_cells as usize * 8 + 2;
        write!(
            f,
            "{{ addr = {:#0awidth$x}, size = {:#0swidth$x} }}",
            self.addr, self.size
        )
    }
}

impl core::fmt::Debug for MapRangeField {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let cwidth = self.child_addr_cells as usize * 8 + 2;
        let pwidth = self.parent_addr_cells as usize * 8 + 2;
        let swidth = self.size_cells as usize * 8 + 2;
        write!(
            f,
            "{{ child_addr = {:#0cwidth$x}, parent_addr = {:#0pwidth$x}, size = {:#0swidth$x} }}",
            self.child_addr, self.parent_addr, self.size
        )
    }
}

impl<'a> MappingIterator<'a> {
    pub fn new(iter: DeviceTreeIterator<'a>) -> Self {
        Self {
            inner: iter,
            depth: 0,
            min_depth: 0,
            addr_mappings: [DEFAULT_MAPPING; 8],
        }
    }
    pub fn current_depth(&self) -> usize {
        self.depth
    }
    pub fn stop_at_depth(&mut self, depth: usize) {
        self.min_depth = depth;
    }

    pub fn peek_token(&self) -> Option<u32> {
        self.inner.peek_token()
    }

    pub fn current_mapping(&self) -> Option<Mapping<'a>> {
        self.addr_mappings.get(self.depth).copied()
    }

    pub fn parse_addr_size(&self, data: &[u8]) -> Result<AddrSizeField, &'static str> {
        let parent_mapping = self
            .depth
            .checked_sub(1)
            .and_then(|d| self.addr_mappings.get(d))
            .ok_or("missing root-level mapping")?;

        let slice = bytemuck::try_cast_slice::<_, u32_be>(data)
            .map_err(|_| "Invalid `reg` field: wrong cell count")?;

        let addr_cells = parent_mapping.addr_cells as usize;
        let size_cells = parent_mapping.size_cells as usize;
        let total_cells = addr_cells + size_cells;
        if slice.len() % total_cells != 0 {
            return Err("Invalid `reg` field: wrong cell count");
        }
        let addr = parse_u32_be_array(slice.get(..addr_cells).ok_or("missing addr cells?")?);
        let size = parse_u32_be_array(
            slice
                .get(addr_cells..total_cells)
                .ok_or("missing size cells?")?,
        );

        Ok(AddrSizeField {
            addr,
            size,
            addr_cells: addr_cells as u8,
            size_cells: size_cells as u8,
        })
    }

    pub fn map_addr_size(&self, addr_size: AddrSizeField) -> Result<AddrSizeField, &'static str> {
        let mut cur = addr_size;
        for depth in (0..self.depth).rev() {
            let map = self
                .addr_mappings
                .get(depth)
                .ok_or("address resolution nested too deeply")?;
            if let Some(mut ranges) = map.iter_ranges() {
                let range = ranges
                    .find(|r| {
                        cur.addr >= r.child_addr && cur.addr + cur.size <= r.child_addr + r.size
                    })
                    .ok_or("register not in mapped range")?;

                cur = AddrSizeField {
                    addr: cur.addr - range.child_addr + range.parent_addr,
                    size: cur.size,
                    addr_cells: range.parent_addr_cells,
                    size_cells: cur.size_cells,
                };
            }
        }
        Ok(cur)
    }

    pub fn addr_cells(&self) -> Option<u8> {
        let parent_mapping = self
            .depth
            .checked_sub(1)
            .and_then(|d| self.addr_mappings.get(d))?;
        Some(parent_mapping.addr_cells)
    }

    pub fn into_props_iter(
        self,
        depth: usize,
    ) -> impl Iterator<Item = Result<(&'a str, &'a [u8]), &'static str>>
           + core::ops::Deref<Target = MappingIterator<'a>>
           + Into<MappingIterator<'a>> {
        struct PropIter<'a> {
            iter: MappingIterator<'a>,
            prop_depth: usize,
        }

        impl<'a> core::ops::Deref for PropIter<'a> {
            type Target = MappingIterator<'a>;
            fn deref(&self) -> &Self::Target {
                &self.iter
            }
        }
        impl<'a> From<PropIter<'a>> for MappingIterator<'a> {
            fn from(value: PropIter<'a>) -> Self {
                value.iter
            }
        }

        impl<'a> Iterator for PropIter<'a> {
            type Item = Result<(&'a str, &'a [u8]), &'static str>;
            fn next(&mut self) -> Option<Self::Item> {
                loop {
                    match self.iter.next()? {
                        Ok(StructEntry::Prop { name, data }) => {
                            if self.iter.depth == self.prop_depth {
                                return Some(Ok((name, data)));
                            }
                        }
                        Ok(StructEntry::EndNode) => {
                            if self.iter.depth + 1 == self.prop_depth {
                                return None;
                            }
                        }
                        Ok(_) => (),
                        Err(e) => return Some(Err(e)),
                    }
                }
            }
        }

        PropIter {
            prop_depth: depth,
            iter: self,
        }
    }
}

impl<'a> Mapping<'a> {
    pub fn iter_ranges(
        &self,
    ) -> Option<impl Iterator<Item = MapRangeField> + ExactSizeIterator + 'a> {
        let mapping = self.mapping?;
        let addr_cells = self.addr_cells as usize;
        let size_cells = self.size_cells as usize;
        let parent_addr_cells = self.parent_addr_cells as usize;
        let total_cells = parent_addr_cells + addr_cells + size_cells;

        let bound1 = addr_cells;
        let bound2 = addr_cells + parent_addr_cells;
        let data = mapping.chunks_exact(total_cells);
        Some(data.map(move |chunk| MapRangeField {
            child_addr: parse_u32_be_array(&chunk[..bound1]),
            parent_addr: parse_u32_be_array(&chunk[bound1..bound2]),
            size: parse_u32_be_array(&chunk[bound2..]),
            child_addr_cells: addr_cells as u8,
            parent_addr_cells: parent_addr_cells as u8,
            size_cells: size_cells as u8,
        }))
    }
}

impl<'a> Iterator for MappingIterator<'a> {
    type Item = <DeviceTreeIterator<'a> as Iterator>::Item;
    fn next(&mut self) -> Option<Self::Item> {
        let entry = self.inner.next()?;
        match entry {
            Ok(StructEntry::BeginNode { name: _ }) => {
                let parent_depth = self.depth;
                self.depth += 1;

                let parent_mappings = self
                    .addr_mappings
                    .get(parent_depth)
                    .copied()
                    .unwrap_or(DEFAULT_MAPPING);

                if let Some(mapping) = self.addr_mappings.get_mut(self.depth) {
                    mapping.addr_cells = parent_mappings.addr_cells;
                    mapping.size_cells = parent_mappings.size_cells;
                    mapping.parent_addr_cells = parent_mappings.addr_cells;
                    mapping.mapping = None;
                }
            }
            Ok(StructEntry::EndNode) => {
                self.depth -= 1;
                if self.depth == self.min_depth {
                    return None;
                }
            }
            Ok(StructEntry::Prop { name, data }) => {
                if let Some(mapping) = self.addr_mappings.get_mut(self.depth) {
                    if name == "#address-cells" {
                        mapping.addr_cells = match parse_u32_be(data) {
                            Some(s) => s as u8,
                            None => return Some(Err("invalid value for #cells property")),
                        };
                        return self.next();
                    } else if name == "#size-cells" {
                        mapping.size_cells = match parse_u32_be(data) {
                            Some(s) => s as u8,
                            None => return Some(Err("invalid value for #cells property")),
                        };
                        return self.next();
                    } else if name == "ranges" {
                        let slice = match bytemuck::try_cast_slice::<_, u32_be>(data) {
                            Ok(s) => s,
                            Err(_) => return Some(Err("Invalid `ranges` field: wrong cell count")),
                        };
                        let total_cells = mapping.parent_addr_cells as usize
                            + mapping.addr_cells as usize
                            + mapping.size_cells as usize;

                        if slice.len() % total_cells != 0 {
                            return Some(Err("Invalid `ranges` field: wrong cell count"));
                        }

                        mapping.mapping = Some(slice);
                    }
                }
            }
            Err(_) => (),
        }
        Some(entry)
    }
}

pub fn find_node<'a>(
    tree: &DeviceTree<'a>,
    path: &str,
) -> Result<Option<MappingIterator<'a>>, &'static str> {
    let mut iter = MappingIterator::new(tree.iter());

    let mut depth = 0;
    let mut matching_parts = 0;
    let mut path_idx = 0;

    let mut last_start = iter.clone();

    loop {
        if iter.peek_token() == Some(StructEntry::FDT_BEGIN_NODE) {
            let mut last = iter.clone();
            last.stop_at_depth(last.current_depth());
            last_start = last;
        }
        let Some(entry) = iter.next() else {
            break;
        };

        match entry? {
            StructEntry::BeginNode { name } => {
                if path[path_idx..].starts_with(name) {
                    if path_idx + name.len() == path.len() || name.len() == 0 && path == "/" {
                        return Ok(Some(last_start));
                    } else if path[path_idx + name.len()..].chars().next() == Some('/') {
                        path_idx = path_idx + name.len() + 1;
                        depth += 1;
                        matching_parts += 1;
                        continue;
                    }
                }
                depth += 1;
            }
            StructEntry::EndNode => {
                if depth == matching_parts {
                    return Ok(None);
                } else {
                    depth -= 1;
                }
            }
            StructEntry::Prop { .. } => (),
        }
    }
    Ok(None)
}
