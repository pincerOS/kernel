use crate::format::{DeviceTree, StructEntry};
use crate::util::{parse_u32_be, Mapping, MappingIterator};

#[derive(Debug, Copy, Clone)]
pub enum FieldType {
    Ascii,
    UnsignedInt,
    Unknown,
}

pub fn guess_field_type(data: &[u8]) -> FieldType {
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

pub fn fmt_field(data: &[u8], field_type: FieldType) -> impl core::fmt::Display + '_ {
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

pub fn debug_node(
    mut iter: MappingIterator<'_>,
    out: &mut dyn core::fmt::Write,
) -> Result<(), &'static str> {
    let mut depth = 0usize;
    while let Some(entry) = iter.next() {
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

                if name == "ranges" {
                    if let Some(mapping) = iter.current_mapping() {
                        if let Some(mut ranges) = mapping.iter_ranges() {
                            if ranges.len() <= 1 {
                                write!(out, "[").ok();
                                if let Some(range) = ranges.next() {
                                    write!(out, "{:?}", range).ok();
                                }
                                for range in ranges {
                                    write!(out, ", ").ok();
                                    write!(out, "{:?}", range).ok();
                                }
                                writeln!(out, "]").ok();
                            } else {
                                writeln!(out, "[").ok();
                                for range in ranges {
                                    writeln!(out, "{:width$}  {:?},", "", range, width = depth * 2)
                                        .ok();
                                }
                                writeln!(out, "{:width$}]", "", width = depth * 2).ok();
                            }
                            continue;
                        }
                    }
                } else if name == "dma-ranges" {
                    if let Some(mapping) = iter.current_mapping() {
                        let dma_mapping = Mapping {
                            mapping: Some(bytemuck::cast_slice(data)),
                            ..mapping
                        };
                        if let Some(mut ranges) = dma_mapping.iter_ranges() {
                            if ranges.len() <= 1 {
                                write!(out, "[").ok();
                                if let Some(range) = ranges.next() {
                                    write!(out, "{:?}", range).ok();
                                }
                                for range in ranges {
                                    write!(out, ", ").ok();
                                    write!(out, "{:?}", range).ok();
                                }
                                writeln!(out, "]").ok();
                            } else {
                                writeln!(out, "[").ok();
                                for range in ranges {
                                    writeln!(out, "{:width$}  {:?},", "", range, width = depth * 2)
                                        .ok();
                                }
                                writeln!(out, "{:width$}]", "", width = depth * 2).ok();
                            }
                            continue;
                        }
                    }
                } else if name == "reg" {
                    let addr_size = iter.parse_addr_size(data)?;
                    match iter.map_addr_size(addr_size) {
                        Ok(m) => writeln!(out, "mapped {:?}", m).ok(),
                        Err(e) => writeln!(out, "raw {:?} ({e})", addr_size).ok(),
                    };
                    continue;
                } else if name == "phandle" {
                    let phandle = parse_u32_be(data).ok_or("invalid value for property phandle")?;
                    writeln!(out, "<{}>", phandle).ok();
                    continue;
                } else if name == "compatible" {
                    let mut parts = data[..data.len().saturating_sub(1)].split(|b| *b == b'\x00');
                    write!(out, "[").ok();
                    if let Some(p) = parts.next() {
                        // TODO: debug print invalid bytes
                        write!(out, "{:?}", core::str::from_utf8(p).unwrap_or("INVALID")).ok();
                    }
                    for p in parts {
                        write!(out, ", {:?}", core::str::from_utf8(p).unwrap_or("INVALID")).ok();
                    }
                    writeln!(out, "]").ok();
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

pub fn debug_device_tree(
    tree: &DeviceTree<'_>,
    out: &mut dyn core::fmt::Write,
) -> Result<(), &'static str> {
    let iter = MappingIterator::new(tree.iter());
    debug_node(iter, out)?;
    Ok(())
}
