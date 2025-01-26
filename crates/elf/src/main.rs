use std::fs;

use elf::{section_header, Elf};

// Readelf-like tool to output the parsed elf structs

fn output_elf_file_header(elf: &Elf) {
    println!("ELF Header:");
    print!("  Magic:   ");
    for byte in elf.identity().bytes() {
        print!("{:02x} ", byte);
    }
    println!();
    println!(
        "  Class:                             {:?}",
        elf.identity().class
    );
    println!(
        "  Data:                              {}",
        elf.identity().data
    );
    println!(
        "  Version:                           {}",
        elf.identity().version
    );
    println!(
        "  OS/ABI:                            {}",
        elf.identity().os_abi
    );
    println!(
        "  ABI Version:                       {}",
        elf.identity().abi_version
    );
    println!(
        "  Type:                              {}",
        elf.elf_header().e_type()
    );
    println!(
        "  Machine:                           {}",
        elf.elf_header().e_machine()
    );
    println!(
        "  Version:                           {}",
        elf.elf_header().e_version()
    );
    println!(
        "  Entry point address:               0x{:x}",
        elf.elf_header().e_entry()
    );
    println!(
        "  Start of program headers:          {} (bytes into file)",
        elf.elf_header().e_phoff()
    );
    println!(
        "  Start of section headers:          {} (bytes into file)",
        elf.elf_header().e_shoff()
    );
    println!(
        "  Flags:                             {}",
        elf.elf_header().e_flags()
    );
    println!(
        "  Size of this header:               {} (bytes)",
        elf.elf_header().e_ehsize()
    );
    println!(
        "  Size of program headers:           {} (bytes)",
        elf.elf_header().e_phentsize()
    );
    println!(
        "  Number of program headers:         {}",
        elf.elf_header().e_phnum()
    );
    println!(
        "  Size of section headers:           {} (bytes)",
        elf.elf_header().e_shentsize()
    );
    println!(
        "  Number of section headers:         {}",
        elf.elf_header().e_shnum()
    );
    println!(
        "  Section header string table index: {}",
        elf.elf_header().e_shstrndx()
    );
}

fn output_section_headers<'a>(elf: &'a Elf) -> Result<(), elf::ElfError<'a>> {
    let section_string_table = match elf
        .section_headers()?
        .nth(elf.elf_header().e_shstrndx() as usize)
    {
        Some(section_header) => section_header?,
        None => {
            return Err(elf::ElfError::SectionHeaderError(
                section_header::SectionHeaderError::InvalidIndex,
            ))
        }
    };
    let is_32_bit = matches!(elf.identity().class, elf::identity::Class::ELF32);
    println!("Section Headers:");
    if is_32_bit {
        println!(
            "  [Nr] Name              Type            Addr     Off    Size   ES Flg Lk Inf Al"
        );
    } else {
        println!("  [Nr] Name              Type             Address           Offset");
        println!("       Size              EntSize          Flags  Link  Info  Align");
    }
    for (i, header) in elf.section_headers()?.enumerate() {
        let header = header?;
        let name = match header.name(&section_string_table) {
            Ok(name) => name,
            Err(_) => "",
        };
        if is_32_bit {
            print!("  [{i:2}] ");
            if name.len() > 17 {
                print!("{:17} ", format!("{}[...]", &name[..12]));
            } else {
                print!("{:17} ", name);
            }
            print!("{:15} ", format!("{}", header.sh_type));
            print!("{:08x} ", header.sh_addr);
            print!("{:06x} ", header.sh_offset);
            print!("{:06x} ", header.sh_size);
            print!("{:02x} ", header.sh_entsize);
            print!("{:3} ", format!("{:>3}", format!("{}", header.sh_flags)));
            print!("{:2} ", header.sh_link);
            print!("{:3} ", header.sh_info);
            print!("{:2} ", header.sh_addralign);
            println!();
        } else {
            print!("  [{i:2}] ");
            if name.len() > 17 {
                print!("{:17} ", format!("{}[...]", &name[..12]));
            } else {
                print!("{:17} ", name);
            }
            print!("{:15}  ", format!("{}", header.sh_type));
            print!("{:016x}  ", header.sh_addr);
            print!("{:08x} ", header.sh_offset);
            println!();
            print!("       ");
            print!("{:016x}  ", header.sh_size);
            print!("{:016x} ", header.sh_entsize);
            print!("{:8} ", format!("{:>3}", format!("{}", header.sh_flags)));
            print!("{:2}   ", header.sh_link);
            print!("{:3}    ", header.sh_info);
            print!("{:2} ", header.sh_addralign);
            println!();
        }
    }
    println!("Key to Flags:");
    println!("  W (write), A (alloc), X (execute), M (merge), S (strings), I (info),");
    println!("  L (link order), O (extra OS processing required), G (group), T (TLS),");
    println!("  C (compressed), x (unknown), o (OS specific), E (exclude),");
    println!("  D (mbind), p (processor specific)");
    Ok(())
}

fn output_program_headers<'a>(elf: &'a Elf) -> Result<(), elf::ElfError<'a>> {
    let is_32_bit = matches!(elf.identity().class, elf::identity::Class::ELF32);
    println!("Program Headers:");
    if is_32_bit {
        println!("  Type           Offset   VirtAddr   PhysAddr   FileSiz MemSiz  Flg Align");
    } else {
        println!("  Type           Offset             VirtAddr           PhysAddr");
        println!("                 FileSiz            MemSiz              Flags  Align");
    }
    for header in elf.program_headers()? {
        let header = header?;
        if is_32_bit {
            print!("  ");
            print!("{:14} ", format!("{}", header.p_type));
            print!("0x{:06x} ", header.p_offset);
            print!("0x{:08x} ", header.p_vaddr);
            print!("0x{:08x} ", header.p_paddr);
            print!("0x{:05x} ", header.p_filesz);
            print!("0x{:05x} ", header.p_memsz);
            print!("{:3} ", format!("{}", header.p_flags));
            print!("0x{:x} ", header.p_align);
            println!();
        } else {
            print!("  ");
            print!("{:14} ", format!("{}", header.p_type));
            print!("0x{:016x} ", header.p_offset);
            print!("0x{:016x} ", header.p_vaddr);
            print!("0x{:016x} ", header.p_paddr);
            println!();
            print!("                 ");
            print!("0x{:016x} ", header.p_filesz);
            print!("0x{:016x}  ", header.p_memsz);
            print!("{:3}    ", format!("{}", header.p_flags));
            print!("0x{:x} ", header.p_align);
            println!();
        }
    }
    Ok(())
}

fn display_elf_file<'a>(elf: &'a Elf) -> Result<(), Box<dyn std::error::Error + 'a>> {
    output_elf_file_header(&elf);
    println!();
    output_section_headers(&elf)?;
    println!();
    output_program_headers(&elf)?;
    Ok(())
}

fn main() {
    let data: Vec<u8> = match fs::read("crates/elf/examples/x64/simple") {
        Ok(data) => data,
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };
    let elf = match elf::Elf::new(&data) {
        Ok(elf) => elf,
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };
    match display_elf_file(&elf) {
        Ok(_) => {}
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
    drop(elf);
}
