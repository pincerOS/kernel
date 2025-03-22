use std::fs;
use std::io::{Seek, SeekFrom, Write};
use std::path::Path;

// Equivalent to 'objcopy -O binary [input] [output]`:
// > objcopy can be used to generate a raw binary file by using an
// > output target of binary (e.g., use -O binary).  When objcopy
// > generates a raw binary file, it will essentially produce a memory
// > dump of the contents of the input object file.  All symbols and
// > relocation information will be discarded.  The memory dump will
// > start at the load address of the lowest section copied into the
// > output file.

fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.len() != 2 {
        println!("Usage: dump_img [input.elf] [output.bin]");
        std::process::exit(1);
    }

    let input = &args[0];
    let output = &args[1];

    if let Err(e) = dump_file(input.as_ref(), output.as_ref()) {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

#[derive(Debug)]
enum DumpError {
    NoProgramHeaders,
    NoLoadableSegments,
    MissingSegmentData,
}

impl core::fmt::Display for DumpError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            DumpError::NoProgramHeaders => write!(f, "input elf has no segments"),
            DumpError::NoLoadableSegments => write!(f, "input elf has no loadable segments"),
            DumpError::MissingSegmentData => write!(f, "missing data for a segment"),
        }
    }
}

impl core::error::Error for DumpError {}

fn dump_file(input: &Path, output: &Path) -> Result<(), Box<dyn core::error::Error>> {
    let data: Vec<u8> = fs::read(input)?;
    let elf = elf::Elf::new(&data)?;

    let phdrs = elf
        .program_headers()
        .ok_or(DumpError::NoProgramHeaders)?
        .collect::<Result<Vec<_>, _>>()?;

    let min_segment_base = phdrs
        .iter()
        .filter(|phdr| matches!(phdr.p_type, elf::program_header::Type::Load))
        .map(|phdr| phdr.p_vaddr)
        .min()
        .ok_or(DumpError::NoLoadableSegments)?;

    let mut output_file = std::fs::OpenOptions::new()
        .write(true)
        .truncate(true)
        .create(true)
        .open(output)?;

    for phdr in phdrs {
        if matches!(phdr.p_type, elf::program_header::Type::Load) {
            let data = elf
                .segment_data(&phdr)
                .ok_or(DumpError::MissingSegmentData)?;

            output_file.seek(SeekFrom::Start(phdr.p_vaddr - min_segment_base))?;
            output_file.write_all(data)?;
            // Skip writing (memsize - filesize) zeroes; instead rely
            // on OOB seeks to zero intermediate regions and truncate
            // the final trailing zeroes.
        }
    }

    Ok(())
}
