use std::path::PathBuf;

fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rerun-if-changed=doomgeneric/doomgeneric");

    let crate_root: PathBuf = std::env::var_os("CARGO_MANIFEST_DIR").unwrap().into();
    let newlib_root = crate_root.join("../../../newlib-aarch64/aarch64");
    let libgcc_include = "/usr/lib/gcc/aarch64-linux-gnu/13/include".into();

    let files = std::fs::read_dir("doomgeneric/doomgeneric").unwrap();
    let files = files
        .map(|f| f.map(|d| d.path()))
        .collect::<Result<Vec<_>, _>>()
        .unwrap();

    let files = files
        .into_iter()
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("c"))
        .collect::<Vec<_>>();

    let mut build = cc::Build::new();
    build
        .compiler("clang") // TODO: don't hard-code this
        .files(files)
        .define("NORMALUNIX", None)
        .define("LINUX", None)
        .define("SNDSERV", None)
        .define("_DEFAULT_SOURCE", None)
        .define("DOOMGENERIC_RESX", Some("320"))
        .define("DOOMGENERIC_RESY", Some("240"))
        .includes([newlib_root.join("include"), libgcc_include]);

    let debug = std::env::var("DEBUG").unwrap();
    if debug.parse::<usize>() == Ok(1) {
        build.flag("-g");
    }

    build
        .flag("-nostdinc")
        .flag("-mgeneral-regs-only")
        .flag("-msoft-float")
        .flag("-mstrict-align")
        // .flag("-mfloat-abi=soft")
        // .flag("-mfpu=none")
        // .flag("-march=armv8-a+nofp+nosimd")
        .flag("-w") // TODO: don't hide errors
        .compile("doomgeneric");

    let crate_root = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    println!("cargo::rerun-if-changed=../ulib/script.ld");
    println!("cargo::rustc-link-arg-bins=-T{crate_root}/../ulib/script.ld");
    println!("cargo::rustc-link-arg-bins=-n");

    println!("cargo::rustc-link-arg-bins=--verbose");
    println!(
        "cargo::rustc-link-search=native={}",
        newlib_root.join("lib").to_str().unwrap()
    );
    println!("cargo::rustc-link-lib=static=c");
}
