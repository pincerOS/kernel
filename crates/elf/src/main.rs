use std::fs;

use elf::header;

// Readelf-like tool to output the parsed elf structs

fn output_elf_file_header(data: &[u8]) -> Result<(), header::ElfHeaderError> {
    let header = header::ElfHeader::new(data)?;

    println!("ELF Header:");
    print!("  Magic:   ");
    for byte in header.ident_bytes() {
        print!("{:02x} ", byte);
    }
    println!();
    println!(
        "  Class:                             {:?}",
        header.ident().class
    );
    println!(
        "  Data:                              {}",
        header.ident().data
    );
    println!(
        "  Version:                           {}",
        header.ident().version
    );
    println!(
        "  OS/ABI:                            {}",
        header.ident().os_abi
    );
    println!(
        "  ABI Version:                       {}",
        header.ident().abi_version
    );
    println!("  Type:                              {}", header.e_type());
    println!(
        "  Machine:                           {}",
        header.e_machine()
    );
    println!(
        "  Version:                           {}",
        header.e_version()
    );
    println!(
        "  Entry point address:               0x{:x}",
        header.e_entry()
    );
    println!(
        "  Start of program headers:          {} (bytes into file)",
        header.e_phoff()
    );
    println!(
        "  Start of section headers:          {} (bytes into file)",
        header.e_shoff()
    );
    println!(
        "  Flags:                             0x{:x}",
        header.e_flags()
    );
    println!(
        "  Size of this header:               {} (bytes)",
        header.e_ehsize()
    );
    println!(
        "  Size of program headers:           {} (bytes)",
        header.e_phentsize()
    );
    println!("  Number of program headers:         {}", header.e_phnum());
    println!(
        "  Size of section headers:           {} (bytes)",
        header.e_shentsize()
    );
    println!("  Number of section headers:         {}", header.e_shnum());
    println!(
        "  Section header string table index: {}",
        header.e_shstrndx()
    );

    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let data: Vec<u8> = fs::read("crates/elf/examples/x64/simple")?;
    match output_elf_file_header(&data) {
        Ok(_) => (),
        Err(e) => {
            eprintln!("Error: {:?}", e);
            return Ok(());
        }
    }
    Ok(())
}
