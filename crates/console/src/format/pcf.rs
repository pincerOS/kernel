#![allow(dead_code, nonstandard_style)]

// https://fontforge.org/docs/techref/pcf-format.html
// TODO: explicitly little-endian header fields

use core::mem::offset_of;

use bytemuck::{try_cast_slice, try_from_bytes, AnyBitPattern, Pod, Zeroable};

type le_u32 = u32;

#[derive(Copy, Clone, Debug)]
enum Endian {
    Little,
    Big,
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
struct pcf_u32(u32);

impl pcf_u32 {
    fn read(self, endianness: Endian) -> u32 {
        // While this looks completely wrong, Rust doesn't provide
        // from endianness functions on u32s; however, to_le is exactly
        // equivalent to from_le.  (On a LE system, to_le does nothing;
        // on a BE system, it reverses the byte order.)  The same applies
        // to to_be.
        match endianness {
            Endian::Little => self.0.to_le(),
            Endian::Big => self.0.to_be(),
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
struct pcf_i32(i32);

impl pcf_i32 {
    fn read(self, endianness: Endian) -> i32 {
        match endianness {
            Endian::Little => self.0.to_le(),
            Endian::Big => self.0.to_be(),
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
struct unaligned_pcf_u32([u8; 4]);

impl unaligned_pcf_u32 {
    fn read(self, endianness: Endian) -> u32 {
        match endianness {
            Endian::Little => u32::from_le_bytes(self.0),
            Endian::Big => u32::from_be_bytes(self.0),
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
struct pcf_u16(u16);

impl pcf_u16 {
    fn read(self, endianness: Endian) -> u16 {
        match endianness {
            Endian::Little => self.0.to_le(),
            Endian::Big => self.0.to_be(),
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
struct pcf_i16(i16);

impl pcf_i16 {
    fn read(self, endianness: Endian) -> i16 {
        match endianness {
            Endian::Little => self.0.to_le(),
            Endian::Big => self.0.to_be(),
        }
    }
}

// Manually inlined derive impls from bytemuck, since the
// derive macros triple compilation time

unsafe impl Zeroable for pcf_u32 {}
unsafe impl Pod for pcf_u32 {}

unsafe impl Zeroable for pcf_i32 {}
unsafe impl Pod for pcf_i32 {}

unsafe impl Zeroable for unaligned_pcf_u32 {}
unsafe impl Pod for unaligned_pcf_u32 {}

unsafe impl Zeroable for pcf_u16 {}
unsafe impl Pod for pcf_u16 {}

unsafe impl Zeroable for pcf_i16 {}
unsafe impl Pod for pcf_i16 {}

unsafe impl Zeroable for PCFHeader {}
unsafe impl AnyBitPattern for PCFHeader {}
unsafe impl Zeroable for toc_entry {}
unsafe impl AnyBitPattern for toc_entry {}
unsafe impl Zeroable for EntryType {}
unsafe impl AnyBitPattern for EntryType {}
unsafe impl Zeroable for EntryFormat {}
unsafe impl AnyBitPattern for EntryFormat {}
unsafe impl Zeroable for PropertiesTableHeader {}
unsafe impl AnyBitPattern for PropertiesTableHeader {}
unsafe impl Zeroable for Property {}
unsafe impl AnyBitPattern for Property {}
unsafe impl Zeroable for MetricsCompressed {}
unsafe impl AnyBitPattern for MetricsCompressed {}
unsafe impl Zeroable for MetricsUncompressed {}
unsafe impl AnyBitPattern for MetricsUncompressed {}
unsafe impl Zeroable for AcceleratorTable {}
unsafe impl AnyBitPattern for AcceleratorTable {}
unsafe impl Zeroable for AcceleratorInkbounds {}
unsafe impl AnyBitPattern for AcceleratorInkbounds {}
unsafe impl Zeroable for MetricsCompressedHeader {}
unsafe impl AnyBitPattern for MetricsCompressedHeader {}
unsafe impl Zeroable for MetricsUncompressedHeader {}
unsafe impl AnyBitPattern for MetricsUncompressedHeader {}
unsafe impl Zeroable for BitmapTableHeader {}
unsafe impl AnyBitPattern for BitmapTableHeader {}
unsafe impl Zeroable for EncodingTableHeader {}
unsafe impl AnyBitPattern for EncodingTableHeader {}
unsafe impl Zeroable for ScalableWidthHeader {}
unsafe impl AnyBitPattern for ScalableWidthHeader {}
unsafe impl Zeroable for GlyphNamesHeader {}
unsafe impl AnyBitPattern for GlyphNamesHeader {}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
struct PCFHeader {
    header: [u8; 4], /* always "\1fcp" */
    table_count: le_u32,
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
struct toc_entry {
    type_: EntryType,    /* See below, indicates which table */
    format: EntryFormat, /* See below, indicates how the data are formatted in the table */
    size: le_u32,        /* In bytes */
    offset: le_u32,      /* from start of file */
}

#[repr(transparent)]
#[derive(Copy, Clone, Debug)]
struct EntryType(le_u32);

impl EntryType {
    pub const PCF_PROPERTIES: u32 = (1 << 0);
    pub const PCF_ACCELERATORS: u32 = (1 << 1);
    pub const PCF_METRICS: u32 = (1 << 2);
    pub const PCF_BITMAPS: u32 = (1 << 3);
    pub const PCF_INK_METRICS: u32 = (1 << 4);
    pub const PCF_BDF_ENCODINGS: u32 = (1 << 5);
    pub const PCF_SWIDTHS: u32 = (1 << 6);
    pub const PCF_GLYPH_NAMES: u32 = (1 << 7);
    pub const PCF_BDF_ACCELERATORS: u32 = (1 << 8);
}

#[repr(transparent)]
#[derive(Copy, Clone, Debug)]
pub struct EntryFormat(pub le_u32);

impl EntryFormat {
    pub const PCF_DEFAULT_FORMAT: u32 = 0x0000_0000;
    pub const PCF_INKBOUNDS: u32 = 0x0000_0200;
    pub const PCF_ACCEL_W_INKBOUNDS: u32 = 0x0000_0100;
    pub const PCF_COMPRESSED_METRICS: u32 = 0x0000_0100;

    pub const PCF_GLYPH_PAD_MASK: u32 = (3 << 0); /* See the bitmap table for explanation */
    pub const PCF_BYTE_MASK: u32 = (1 << 2); /* If set then Most Sig Byte First */
    pub const PCF_BIT_MASK: u32 = (1 << 3); /* If set then Most Sig Bit First */
    pub const PCF_SCAN_UNIT_MASK: u32 = (3 << 4); /* See the bitmap table for explanation */
}

// Properties table format:
// - PropertiesTableHeader
// - Property[nprops] (note: 9 bytes each, unaligned)
// - pad to 4 byte boundary
// - string_size: u32
// - string data

#[repr(C)]
#[derive(Copy, Clone, Debug)]
struct PropertiesTableHeader {
    format: EntryFormat, /* Always stored with least significant byte first! */
    nprops: pcf_u32,
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
struct Property {
    name_offset: unaligned_pcf_u32, /* Offset into the following string table */
    is_string_prop: u8,             // Why would you do this.
    value: unaligned_pcf_u32,       /* The value for integer props, the offset for string props */
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
struct MetricsCompressed {
    // Note: all fields are offset by 128, so actual value is (field - 128)
    left_sided_bearing: u8,
    right_side_bearing: u8,
    character_width: u8,
    character_ascent: u8,
    character_descent: u8,
    /* Implied character attributes field = 0 */
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
struct MetricsUncompressed {
    left_sided_bearing: pcf_i16,
    right_side_bearing: pcf_i16,
    character_width: pcf_i16,
    character_ascent: pcf_i16,
    character_descent: pcf_i16,
    character_attributes: pcf_u16,
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
struct AcceleratorTable {
    format: EntryFormat, /* Always stored with least significant byte first! */
    noOverlap: u8, /* if for all i, max(metrics[i].rightSideBearing - metrics[i].characterWidth) */
    /*      <= minbounds.leftSideBearing */
    constantMetrics: u8, /* Means the perchar field of the XFontStruct can be NULL */
    terminalFont: u8,    /* constantMetrics true and forall characters: */
    /*      the left side bearing==0 */
    /*      the right side bearing== the character's width */
    /*      the character's ascent==the font's ascent */
    /*      the character's descent==the font's descent */
    constantWidth: u8, /* monospace font like courier */
    inkInside: u8, /* Means that all inked bits are within the rectangle with x between [0,charwidth] */
    /*  and y between [-descent,ascent]. So no ink overlaps another char when drawing */
    inkMetrics: u8,    /* true if the ink metrics differ from the metrics somewhere */
    drawDirection: u8, /* 0=>left to right, 1=>right to left */
    padding: u8,
    fontAscent: pcf_i32,
    fontDescent: pcf_i32,
    maxOverlap: pcf_i32, /* ??? (sic) */
    minbounds: MetricsUncompressed,
    maxbounds: MetricsUncompressed,
}

#[repr(C)]
#[derive(Debug)]
pub struct AcceleratorUnpacked {
    pub format: EntryFormat,
    pub noOverlap: u8,
    pub constantMetrics: u8,
    pub terminalFont: u8,
    pub constantWidth: u8,
    pub inkInside: u8,
    pub inkMetrics: u8,
    pub drawDirection: u8,
    pub padding: u8,
    pub fontAscent: i32,
    pub fontDescent: i32,
    pub maxOverlap: i32,
    pub minbounds: MetricsUnpacked,
    pub maxbounds: MetricsUnpacked,
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
struct AcceleratorInkbounds {
    /* If format is PCF_ACCEL_W_INKBOUNDS then include the following fields */
    /* Otherwise those fields are not in the file and should be filled by duplicating min/maxbounds above */
    ink_minbounds: MetricsUncompressed,
    ink_maxbounds: MetricsUncompressed,
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
struct MetricsCompressedHeader {
    format: EntryFormat,
    metrics_count: pcf_u16,
    // Followed by `metrics_count` MetricsCompressed structs
    metrics_base: (),
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
struct MetricsUncompressedHeader {
    format: EntryFormat,
    metrics_count: pcf_u32,
    // Followed by `metrics_count` MetricsUncompressed structs
    metrics_base: (),
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
struct BitmapTableHeader {
    format: EntryFormat,
    glyph_count: pcf_u32,
    // Followed by glyph_count offsets: [u32; glyph_count]
    /* byte offsets to bitmap data */
    // Followed by bitmapSizes: [u32; 4]
    /* the size the bitmap data will take up depending on various padding options */
    /*  which one is actually used in the file is given by (format&3) */
    // Followed by bitmap_data: [u8; bitmapSizes[format & 0b11]]
    /* the bitmap data. format contains flags that indicate: */
    /* the byte order (format&4 => LSByte first)*/
    /* the bit order (format&8 => LSBit first) */
    /* how each row in each glyph's bitmap is padded (format&3) */
    /*  0=>bytes, 1=>shorts, 2=>ints */
    /* what the bits are stored in (bytes, shorts, ints) (format>>4)&3 */
    /*  0=>bytes, 1=>shorts, 2=>ints */
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
struct EncodingTableHeader {
    format: EntryFormat,
    min_char_or_byte2: pcf_u16, /* As in XFontStruct */
    max_char_or_byte2: pcf_u16, /* As in XFontStruct */
    min_byte1: pcf_u16,         /* As in XFontStruct */
    max_byte1: pcf_u16,         /* As in XFontStruct */
    default_char: pcf_u16,      /* As in XFontStruct */
    // Followed by glyph_indices;
    // array of (max_char_or_byte2 - min_char_or_byte2 + 1)*(max_byte1 - min_byte1 + 1) u16 entries,
    /* Gives the glyph index that corresponds to each encoding value */
    /* a value of 0xffff means no glyph for that encoding */
    glyph_indices_base: (),
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
struct ScalableWidthHeader {
    format: EntryFormat,
    glyph_count: pcf_u32,
    // Followed by swidths: [u32; glyph_count]
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
struct GlyphNamesHeader {
    format: EntryFormat,
    glyph_count: pcf_u32,
    // Followed by glyph_count offsets: [u32; glyph_count]
    // Followed by string_size: u32
    // Followed by string data [u8; string_size]
}

impl EntryFormat {
    fn endianness(&self) -> Endian {
        match (self.0 & EntryFormat::PCF_BYTE_MASK) != 0 {
            true => Endian::Big,
            false => Endian::Little,
        }
    }
}

struct PropertiesData<'a> {
    endian: Endian,
    properties: &'a [Property],
    string_data: &'a [u8],
}
enum PropValue<'a> {
    Number(u32),
    String(&'a [u8]),
}
impl<'a> PropertiesData<'a> {
    fn iter_props(&self) -> impl Iterator<Item = (&'a [u8], PropValue)> {
        self.properties.iter().map(|prop| {
            let name_off = prop.name_offset.read(self.endian) as usize;
            let str_prop = prop.is_string_prop != 0;
            let value = prop.value.read(self.endian);

            let str = core::ffi::CStr::from_bytes_until_nul(&self.string_data[name_off..]).unwrap();
            if str_prop {
                let value =
                    core::ffi::CStr::from_bytes_until_nul(&self.string_data[value as usize..])
                        .unwrap();
                (str.to_bytes(), PropValue::String(value.to_bytes()))
            } else {
                (str.to_bytes(), PropValue::Number(value))
            }
        })
    }
}

struct EncodingData<'a> {
    endian: Endian,
    char_mode: bool,
    min_byte1: u32,
    max_byte1: u32,
    min_byte2: u32,
    max_byte2: u32,
    glyph_indices: &'a [pcf_u16],
}
impl<'a> EncodingData<'a> {
    fn get_glyph(&self, c: char) -> Option<u16> {
        let c = c as u32;
        let index;
        if self.char_mode {
            index = c - self.min_byte2;
            if c > self.max_byte2 {
                return None;
            }
        } else {
            let byte1 = (c >> 8) & 0xFF;
            let byte2 = (c) & 0xFF;
            if byte1 < self.min_byte1
                || byte1 > self.max_byte1
                || byte2 < self.min_byte2
                || byte2 > self.max_byte2
            {
                // TODO: check > vs >=
                return None;
            }
            index = (byte1 - self.min_byte1) * (self.max_byte2 - self.min_byte2 + 1)
                + (byte2 - self.min_byte2);
        }
        let glyph = self
            .glyph_indices
            .get(index as usize)
            .map(|i| i.read(self.endian));
        glyph.filter(|&v| v != 0xFFFF)
    }
}

struct GlyphNameData<'a> {
    endian: Endian,
    offsets: &'a [pcf_u32],
    string_data: &'a [u8],
}
impl<'a> GlyphNameData<'a> {
    fn get_name(&self, glyph: u16) -> Option<&'a [u8]> {
        let offset = self.offsets.get(glyph as usize)?.read(self.endian) as usize;
        let name = core::ffi::CStr::from_bytes_until_nul(&self.string_data[offset..]).unwrap();
        Some(name.to_bytes())
    }
}

enum MetricsData<'a> {
    Compressed(&'a [MetricsCompressed]),
    Uncompressed(&'a [MetricsUncompressed], Endian),
}
#[derive(Debug)]
pub struct MetricsUnpacked {
    pub left_sided_bearing: i16,
    pub right_side_bearing: i16,
    pub character_width: i16,
    pub character_ascent: i16,
    pub character_descent: i16,
    pub character_attributes: u16,
}
impl MetricsCompressed {
    fn unpack(&self) -> MetricsUnpacked {
        MetricsUnpacked {
            left_sided_bearing: self.left_sided_bearing as i16 - 0x80,
            right_side_bearing: self.right_side_bearing as i16 - 0x80,
            character_width: self.character_width as i16 - 0x80,
            character_ascent: self.character_ascent as i16 - 0x80,
            character_descent: self.character_descent as i16 - 0x80,
            character_attributes: 0,
        }
    }
}
impl MetricsUncompressed {
    fn unpack(&self, endian: Endian) -> MetricsUnpacked {
        MetricsUnpacked {
            left_sided_bearing: self.left_sided_bearing.read(endian),
            right_side_bearing: self.right_side_bearing.read(endian),
            character_width: self.character_width.read(endian),
            character_ascent: self.character_ascent.read(endian),
            character_descent: self.character_descent.read(endian),
            character_attributes: self.character_attributes.read(endian),
        }
    }
}
impl<'a> MetricsData<'a> {
    fn get_metrics(&self, glyph: u16) -> Option<MetricsUnpacked> {
        match *self {
            MetricsData::Compressed(data) => Some(data.get(glyph as usize)?.unpack()),
            MetricsData::Uncompressed(data, endian) => {
                Some(data.get(glyph as usize)?.unpack(endian))
            }
        }
    }
}

struct BitmapData<'a> {
    endian: Endian,
    pad_mode: u8,
    size_mode: u8,
    least_bit_first: bool,
    offsets: &'a [pcf_u32],
    bitmap: &'a [u8],
}
impl<'a> BitmapData<'a> {
    pub fn unpack_bitmap(
        &self,
        glyph: u16,
        width: usize,
        height: usize,
        buffer: &mut [bool],
        start: usize,
        buf_row_stride: usize,
        // scale: usize,
    ) -> Option<()> {
        let offset = self.offsets.get(glyph as usize)?.read(self.endian);

        let row_pad = 1 << self.pad_mode;
        let scan = 1 << self.size_mode;

        assert_eq!(scan, 1, "multi-byte scan for PCF not yet implemented");
        assert!(self.least_bit_first, "MSBit first PCF not yet implemented");

        // TODO: caching and performance improvements
        if scan == 1 {
            let rows = height;
            let row_size = width.div_ceil(8);
            let row_stride = row_size.next_multiple_of(row_pad);
            let offset = offset as usize;
            let mut buf_row_start = start;
            for r in 0..rows {
                let row_base = offset + r * row_stride;
                for c in 0..row_size {
                    let val = self.bitmap[row_base + c];
                    for b in 0..8 {
                        if (val >> (7 - b)) & 1 != 0 {
                            let i = buf_row_start + (c * 8 + b);
                            buffer[i] = true;
                        }
                    }
                }
                buf_row_start += buf_row_stride;
            }
        }
        Some(())
    }

    pub fn draw_bitmap(
        &self,
        glyph: u16,
        width: usize,
        height: usize,
        buffer: &mut [u32],
        start: usize,
        buf_row_stride: usize,
        scale: usize,
        color: u32,
    ) -> Option<()> {
        let offset = self.offsets.get(glyph as usize)?.read(self.endian);

        let row_pad = 1 << self.pad_mode;
        let scan = 1 << self.size_mode;

        assert_eq!(scan, 1, "multi-byte scan for PCF not yet implemented");
        assert!(self.least_bit_first, "MSBit first PCF not yet implemented");

        // TODO: caching and performance improvements
        if scan == 1 {
            let rows = height;
            let row_size = width.div_ceil(8);
            let row_stride = row_size.next_multiple_of(row_pad);
            // let safety_margin = (scale - 1) * buf_row_stride + (scale - 1);

            let mut bmp_row_base = offset as usize;
            let mut dst_row_base = start;
            for _ in 0..rows {
                assert!(bmp_row_base + width / 8 < self.bitmap.len());
                for c in 0..width {
                    let (byte, bit) = (c / 8, c % 8);
                    if (self.bitmap[bmp_row_base + byte] >> (7 - bit)) & 1 != 0 {
                        let i = dst_row_base + c * scale;
                        // if i + safety_margin >= buffer.len() {
                        //     continue;
                        // }
                        for ro in 0..scale {
                            for co in 0..scale {
                                // TODO: index out of bounds when writing char partially outside of buffer
                                buffer[i + ro * buf_row_stride + co] = color;
                            }
                        }
                    }
                }
                bmp_row_base += row_stride;
                dst_row_base += buf_row_stride * scale;
            }
        }

        Some(())
    }
}

pub struct LoadedPCF<'a> {
    prop_data: PropertiesData<'a>,
    encoding_data: EncodingData<'a>,
    glyph_name_data: GlyphNameData<'a>,
    metrics_data: MetricsData<'a>,
    bitmap_data: BitmapData<'a>,
    pub accelerator_data: AcceleratorUnpacked,
}

#[derive(Copy, Clone)]
pub struct GlyphInfo {
    pub glyph: u16,
    pub width: usize,
    pub height: usize,
    pub pad_top: usize,
    pub pad_left: usize,
}

impl<'a> LoadedPCF<'a> {
    pub fn dimensions(&self) -> (usize, usize) {
        let bounds = &self.accelerator_data.maxbounds;
        (
            (bounds.character_ascent + bounds.character_descent) as usize,
            bounds.character_width as usize,
        )
    }
    // TODO: proper layouting
    pub fn prep_char(&self, char: char) -> Option<GlyphInfo> {
        let glyph = self.encoding_data.get_glyph(char);
        if let Some(glyph) = glyph {
            let metrics = self.metrics_data.get_metrics(glyph).unwrap();
            let width = metrics.character_width as usize;
            let height = (metrics.character_ascent + metrics.character_descent) as usize;

            let base_ascent = self.accelerator_data.maxbounds.character_ascent as usize;
            let pad_top = base_ascent - metrics.character_ascent as usize;
            let pad_left = metrics.left_sided_bearing as usize;

            Some(GlyphInfo {
                glyph,
                width,
                height,
                pad_top,
                pad_left,
            })
        } else {
            // TODO: fallback glyph (box or something)
            None
        }
    }
    pub fn draw_glyph(
        &self,
        glyph: GlyphInfo,
        buffer: &mut [u32],
        start: usize,
        row_stride: usize,
        scale: usize,
        color: u32,
    ) {
        let start = start + row_stride * scale * glyph.pad_top + glyph.pad_left * scale;
        self.bitmap_data.draw_bitmap(
            glyph.glyph,
            glyph.width,
            glyph.height,
            buffer,
            start,
            row_stride,
            scale,
            color,
        );
    }
    pub fn unpack_glyph(
        &self,
        glyph: GlyphInfo,
        buffer: &mut [bool],
        start: usize,
        row_stride: usize,
        // scale: usize,
    ) {
        let start = start + row_stride * glyph.pad_top + glyph.pad_left;
        self.bitmap_data.unpack_bitmap(
            glyph.glyph,
            glyph.width,
            glyph.height,
            buffer,
            start,
            row_stride,
        );
    }
    pub fn draw_char(
        &self,
        char: char,
        buffer: &mut [u32],
        start: usize,
        row_stride: usize,
        scale: usize,
        color: u32,
    ) -> Option<GlyphInfo> {
        let info = self.prep_char(char);
        if let Some(info) = info {
            self.draw_glyph(info, buffer, start, row_stride, scale, color);
        }
        info
    }
    pub fn draw_string(
        &self,
        str: &str,
        buffer: &mut [u32],
        start: usize,
        wrap_at: Option<usize>,
        row_stride: usize,
        scale: usize,
        color: u32,
    ) -> usize {
        let maxbounds = &self.accelerator_data.maxbounds;
        let line_height =
            scale * (maxbounds.character_ascent + maxbounds.character_descent) as usize;
        let wrap_at = wrap_at.unwrap_or(usize::MAX);
        let mut x = 0;
        let mut y = 0;
        for char in str.chars() {
            match char {
                '\r' => continue,
                '\n' => {
                    x = 0;
                    y += line_height;
                    continue;
                }
                _ => (),
            }
            let glyph = self.prep_char(char);
            if let Some(glyph) = glyph {
                if x + glyph.width * scale >= wrap_at {
                    x = 0;
                    y += line_height;
                }
                self.draw_glyph(
                    glyph,
                    buffer,
                    start + y * row_stride + x,
                    row_stride,
                    scale,
                    color,
                );
                x += glyph.width * scale;
            }
        }
        y + line_height
    }
}

// pub fn debug_pcf(font: LoadedPCF<'_>) {
//     let char = 'a';
//     let glyph = font.encoding_data.get_glyph(char);
//     println!("{:?}: glyph {:?}, name {:?}", char, glyph, glyph.and_then(|g| core::str::from_utf8(font.glyph_name_data.get_name(g)?).ok()));

//     let metrics = glyph.and_then(|g| font.metrics_data.get_metrics(g)).unwrap();
//     println!("Metrics: {:?}", metrics);

//     font.bitmap_data.glyph_bitmap(glyph.unwrap(), metrics.character_width as usize, (metrics.character_ascent + metrics.character_descent) as usize);
// }

pub fn load_pcf(data: &[u8]) -> Result<LoadedPCF<'_>, ()> {
    let header: &PCFHeader = try_from_bytes(&data[..size_of::<PCFHeader>()]).unwrap();
    if header.header != *b"\x01fcp" {
        // println!("Wrong header: {:?}", header.header);
        return Err(());
    }

    // println!("{:?}", header);

    let table_count = header.table_count as usize;
    let toc_base = size_of::<PCFHeader>();
    let toc_size = size_of::<toc_entry>() * table_count as usize;
    let toc: &[toc_entry] = try_cast_slice(&data[toc_base..][..toc_size]).unwrap();

    let mut prop_data = None;
    let mut encoding_data = None;
    let mut glyph_name_data = None;
    let mut metrics_data = None;
    let mut bitmap_data = None;
    let mut accelerator_data = None;

    for entry in toc.iter() {
        // println!("{:?}", toc[i]);
        // TODO: why are the sizes wrong???
        let table_data = &data[entry.offset as usize..];
        let table_data = &table_data[..(entry.size as usize).min(table_data.len())];

        match entry.type_.0 {
            EntryType::PCF_PROPERTIES => {
                let header: &PropertiesTableHeader =
                    try_from_bytes(&table_data[..size_of::<PropertiesTableHeader>()]).unwrap();
                let endian = header.format.endianness();

                let nprops = header.nprops.read(endian) as usize;
                let properties_base = size_of::<PropertiesTableHeader>();
                let properties_end = properties_base + size_of::<Property>() * nprops;
                let properties: &[Property] =
                    try_cast_slice(&table_data[properties_base..properties_end]).unwrap();

                let string_base = properties_end.next_multiple_of(4);
                let string_size: &pcf_u32 =
                    bytemuck::from_bytes(&table_data[string_base..string_base + 4]);
                let string_size = string_size.read(endian) as usize;
                let string_data = &table_data[string_base + 4..string_base + 4 + string_size];

                prop_data = Some(PropertiesData {
                    endian,
                    properties,
                    string_data,
                });
            }
            EntryType::PCF_ACCELERATORS | EntryType::PCF_BDF_ACCELERATORS => {
                let table: &AcceleratorTable =
                    try_from_bytes(&table_data[..size_of::<AcceleratorTable>()]).unwrap();
                let endian = table.format.endianness();

                accelerator_data = Some(AcceleratorUnpacked {
                    format: table.format,
                    noOverlap: table.noOverlap,
                    constantMetrics: table.constantMetrics,
                    terminalFont: table.terminalFont,
                    constantWidth: table.constantWidth,
                    inkInside: table.inkInside,
                    inkMetrics: table.inkMetrics,
                    drawDirection: table.drawDirection,
                    padding: table.padding,
                    fontAscent: table.fontAscent.read(endian),
                    fontDescent: table.fontDescent.read(endian),
                    maxOverlap: table.maxOverlap.read(endian),
                    minbounds: table.minbounds.unpack(endian),
                    maxbounds: table.maxbounds.unpack(endian),
                });
            }
            EntryType::PCF_METRICS => {
                let endian = entry.format.endianness();
                if (entry.format.0 & EntryFormat::PCF_COMPRESSED_METRICS) != 0 {
                    let header: &MetricsCompressedHeader =
                        try_from_bytes(&table_data[..size_of::<MetricsCompressedHeader>()])
                            .unwrap();
                    let metrics_count = header.metrics_count.read(endian) as usize;
                    let metrics_base = offset_of!(MetricsCompressedHeader, metrics_base);

                    let metrics_end = metrics_base + size_of::<MetricsCompressed>() * metrics_count;
                    let metrics: &[MetricsCompressed] =
                        try_cast_slice(&table_data[metrics_base..metrics_end]).unwrap();
                    metrics_data = Some(MetricsData::Compressed(metrics));
                } else {
                    let header: &MetricsUncompressedHeader =
                        try_from_bytes(&table_data[..size_of::<MetricsUncompressedHeader>()])
                            .unwrap();
                    let metrics_count = header.metrics_count.read(endian) as usize;
                    let metrics_base = offset_of!(MetricsCompressedHeader, metrics_base);

                    let metrics_end =
                        metrics_base + size_of::<MetricsUncompressed>() * metrics_count;
                    let metrics: &[MetricsUncompressed] =
                        try_cast_slice(&table_data[metrics_base..metrics_end]).unwrap();
                    metrics_data = Some(MetricsData::Uncompressed(metrics, endian));
                }
            }
            EntryType::PCF_BITMAPS => {
                let header: &BitmapTableHeader =
                    try_from_bytes(&table_data[..size_of::<BitmapTableHeader>()]).unwrap();
                let endian = header.format.endianness();

                let glyph_count = header.glyph_count.read(endian) as usize;
                let offsets_base = size_of::<BitmapTableHeader>();
                let offsets_size = glyph_count * size_of::<pcf_u32>();
                let offsets: &[pcf_u32] =
                    try_cast_slice(&table_data[offsets_base..offsets_base + offsets_size]).unwrap();

                let pad_mode = (header.format.0 & EntryFormat::PCF_GLYPH_PAD_MASK) as u8;
                let size_mode = ((header.format.0 & EntryFormat::PCF_SCAN_UNIT_MASK) >> 3) as u8;
                let least_bit_first = (header.format.0 & EntryFormat::PCF_BIT_MASK) != 0;

                let bitmap_base = offsets_base + offsets_size;
                let bitmap_sizes_size = size_of::<[pcf_u32; 4]>();
                let bitmap_sizes: &[pcf_u32; 4] =
                    try_from_bytes(&table_data[bitmap_base..bitmap_base + bitmap_sizes_size])
                        .unwrap();
                let bitmap_size = bitmap_sizes[pad_mode as usize].read(endian) as usize;
                let bitmap = &table_data[bitmap_base + bitmap_sizes_size
                    ..bitmap_base + bitmap_sizes_size + bitmap_size];

                bitmap_data = Some(BitmapData {
                    endian,
                    pad_mode,
                    size_mode,
                    least_bit_first,
                    offsets,
                    bitmap,
                });
            }
            EntryType::PCF_BDF_ENCODINGS => {
                let header: &EncodingTableHeader =
                    try_from_bytes(&table_data[..size_of::<EncodingTableHeader>()]).unwrap();
                let endian = header.format.endianness();

                let min_byte1 = header.min_byte1.read(endian) as u32;
                let max_byte1 = header.max_byte1.read(endian) as u32;
                let min_byte2 = header.min_char_or_byte2.read(endian) as u32;
                let max_byte2 = header.max_char_or_byte2.read(endian) as u32;

                let indices_base = offset_of!(EncodingTableHeader, glyph_indices_base);
                let table_size = (max_byte2 - min_byte2 + 1) * (max_byte1 - min_byte1 + 1);
                let glyph_indices: &[pcf_u16] =
                    try_cast_slice(&table_data[indices_base..][..table_size as usize]).unwrap();

                let char_mode = min_byte1 == 0 && max_byte1 == 0;

                encoding_data = Some(EncodingData {
                    endian,
                    char_mode,
                    min_byte1,
                    max_byte1,
                    min_byte2,
                    max_byte2,
                    glyph_indices,
                });
            }
            EntryType::PCF_GLYPH_NAMES => {
                let header: &GlyphNamesHeader =
                    try_from_bytes(&table_data[..size_of::<GlyphNamesHeader>()]).unwrap();
                let endian = header.format.endianness();

                let glyph_count = header.glyph_count.read(endian) as usize;
                let offsets_base = size_of::<GlyphNamesHeader>();
                let offsets_size = glyph_count * size_of::<pcf_u32>();
                let offsets: &[pcf_u32] =
                    try_cast_slice(&table_data[offsets_base..offsets_base + offsets_size]).unwrap();

                let string_base = offsets_base + offsets_size;
                let string_size: &pcf_u32 =
                    bytemuck::from_bytes(&table_data[string_base..string_base + 4]);
                let string_size = string_size.read(endian) as usize;
                let string_data = &table_data[string_base + 4..string_base + 4 + string_size];

                glyph_name_data = Some(GlyphNameData {
                    endian,
                    offsets,
                    string_data,
                });
            }
            _ => {
                // panic!();
            }
        }
    }

    let font = LoadedPCF {
        prop_data: prop_data.unwrap(),
        encoding_data: encoding_data.unwrap(),
        glyph_name_data: glyph_name_data.unwrap(),
        metrics_data: metrics_data.unwrap(),
        bitmap_data: bitmap_data.unwrap(),
        accelerator_data: accelerator_data.unwrap(),
    };

    // println!("## Properties:");
    // for (name, prop) in font.prop_data.iter_props() {
    //     match prop {
    //         PropValue::String(s) => println!("{}: {:?}", core::str::from_utf8(name).unwrap(), core::str::from_utf8(s).unwrap()),
    //         PropValue::Number(num) => println!("{}: {:?}", core::str::from_utf8(name).unwrap(), num),
    //     }
    // }

    // println!("{:#?}", font.accelerator_data);

    Ok(font)
}
