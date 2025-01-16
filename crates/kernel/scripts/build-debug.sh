#!/usr/bin/env bash
cargo clean
cargo rustc --target aarch64-unknown-none-softfloat -- \
    -C link-arg=--script=./crates/kernel//script.ld \
    -C relocation-model=pic
llvm-objcopy -O binary ../../target/aarch64-unknown-none-softfloat/debug/kernel kernel.bin
