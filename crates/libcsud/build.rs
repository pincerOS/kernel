use std::path::PathBuf;

fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rerun-if-changed=csud");

    let crate_root = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let crate_root = PathBuf::from(crate_root);

    println!("cargo::rustc-link-search=native={}", crate_root.join("csud").display());
    println!("cargo::rustc-link-lib=static=csud");
}
