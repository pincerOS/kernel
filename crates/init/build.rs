fn main() {
    let crate_root = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    println!("cargo::rerun-if-changed=./script.ld");
    println!("cargo::rustc-link-arg-bins=-T{crate_root}/script.ld");
    println!("cargo::rustc-link-arg-bins=-n");
}
