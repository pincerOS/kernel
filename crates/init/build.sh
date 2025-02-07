#!/usr/bin/env bash

set -ex

BIN="user"
TARGET=aarch64-unknown-none-softfloat
PROFILE=${PROFILE-"release"}

./example.rs

# cargo clean
cargo rustc --profile=${PROFILE} \
    --target=${TARGET} -- \
    -C relocation-model=static

if test "$PROFILE" = "dev" ; then
    BINARY=../../target/${TARGET}/debug/${BIN}
else
    BINARY=../../target/${TARGET}/${PROFILE}/${BIN}
fi

cp ${BINARY} init.elf
llvm-objcopy -O binary ${BINARY} init.bin
