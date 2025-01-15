#!/usr/bin/env bash
cargo clean
cargo rustc --release -- -C link-arg=--library-path=. -C link-arg=--script=script.ld
llvm-objcopy -O binary target/aarch64-unknown-none-softfloat/release/kernel kernel.bin
