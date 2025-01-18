#!/usr/bin/env bash
cargo clean
cargo rustc --release --target aarch64-unknown-none-softfloat -- \
    -C link-arg=--script=./crates/kernel/script.ld \
    -C relocation-model=static
llvm-objcopy -O binary ../../target/aarch64-unknown-none-softfloat/release/kernel kernel.bin
