use std::fs;

use elf::{
    elf_header, program_header,
    section_header::{get_section_headers, get_string_table_header},
};

// Readelf-like tool to output the parsed elf structs

fn output_elf_file_header(header: &elf_header::ElfHeader) {
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
    println!("  Flags:                             {}", header.e_flags());
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
}

fn output_section_headers(data: &[u8], elf_header: &elf_header::ElfHeader) {
    let section_headers = match get_section_headers(data, elf_header) {
        Ok(headers) => headers,
        Err(e) => {
            eprintln!("Error: {:?}", e);
            return;
        }
    };
    let string_table = match get_string_table_header(data, elf_header) {
        Ok(header) => header,
        Err(e) => {
            eprintln!("Error: {:?}", e);
            return;
        }
    };
    let is_32_bit = elf_header.ident().class == elf::identity::Class::ELF32;
    println!("Section Headers:");
    if is_32_bit {
        println!(
            "  [Nr] Name              Type            Addr     Off    Size   ES Flg Lk Inf Al"
        );
    } else {
        println!("  [Nr] Name              Type             Address           Offset");
        println!("       Size              EntSize          Flags  Link  Info  Align");
    }
    for (i, header) in section_headers.enumerate() {
        let header = match header {
            Ok(h) => h,
            Err(e) => {
                eprintln!("Error: {:?}", e);
                return;
            }
        };
        let name = match header.name(data, &string_table) {
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
}

fn output_program_headers(data: &[u8], elf_header: &elf_header::ElfHeader) {
    let program_headers = match program_header::get_program_headers(data, elf_header) {
        Ok(headers) => headers,
        Err(e) => {
            eprintln!("Error: {:?}", e);
            return;
        }
    };
    let is_32_bit = elf_header.ident().class == elf::identity::Class::ELF32;
    println!("Program Headers:");
    if is_32_bit {
        println!("  Type           Offset   VirtAddr   PhysAddr   FileSiz MemSiz  Flg Align");
    } else {
        println!("  Type           Offset             VirtAddr           PhysAddr");
        println!("                 FileSiz            MemSiz              Flags  Align");
    }
    for header in program_headers {
        let header = match header {
            Ok(h) => h,
            Err(e) => {
                eprintln!("Error: {:?}", e);
                return;
            }
        };
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
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let data: Vec<u8> = fs::read("crates/elf/examples/x64/simple")?;
    let elf_header = match elf_header::ElfHeader::new(&data) {
        Ok(header) => header,
        Err(e) => {
            eprintln!("Error: {:?}", e);
            return Ok(());
        }
    };
    output_elf_file_header(&elf_header);
    println!();
    output_section_headers(&data, &elf_header);
    println!();
    output_program_headers(&data, &elf_header);
    Ok(())
}
