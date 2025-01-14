#!/usr/bin/bash
cargo clean
cargo rustc -- -C link-arg=--library-path=. -C link-arg=--script=script.ld
llvm-objcopy -O binary target/aarch64-unknown-none-softfloat/debug/kernel kernel.bin
