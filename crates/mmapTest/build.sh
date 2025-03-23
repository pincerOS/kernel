#!/usr/bin/env bash

set -ex

BIN="mmapTest"
TARGET=aarch64-unknown-none-softfloat
PROFILE=${PROFILE-"release"}

# cargo clean
cargo rustc --profile="${PROFILE}" \
    --target=${TARGET} -- \
    -C relocation-model=static

if test "$PROFILE" = "dev" ; then
    BINARY=../../target/${TARGET}/debug/${BIN}
else
    BINARY=../../target/${TARGET}/${PROFILE}/${BIN}
fi

cp "${BINARY}" mmapTest.elf
objcopy -I elf32-little -O binary "${BINARY}" mmapTest.bin
