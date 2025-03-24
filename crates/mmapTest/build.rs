//use std::path::PathBuf;
//use std::process::Command;

fn main() {
    let crate_root = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    println!("cargo::rerun-if-changed=./script.ld");
    println!("cargo::rustc-link-arg-bins=-T{crate_root}/script.ld");
    println!("cargo::rustc-link-arg-bins=-n");

    /*
    println!("cargo::rerun-if-changed=./example.rs");

    let crate_root = PathBuf::from(crate_root);
    let status = Command::new(crate_root.join("example.rs"))
        .status()
        .expect("failed to execute process");
    assert!(status.success());
    */
}
