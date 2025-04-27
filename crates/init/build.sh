#!/usr/bin/env bash

set -ex
cd "$(dirname "$0")"

BIN="init"
TARGET=aarch64-unknown-none-softfloat
PROFILE=${PROFILE-"release"}

FS_PATH="../../disk-image"
mkdir -p "$FS_PATH"

./sharedMemTest.rs
cp sharedMemTest.elf "$FS_PATH/"


cargo run -q -p initfs --bin util --release \
    -- create --compress --out fs.arc --root "$FS_PATH" "$FS_PATH" --verbose

# cargo clean
cargo rustc --profile="${PROFILE}" \
    --target="${TARGET}" \
    --bin="${BIN}" \
    -- -C relocation-model=static

if test "$PROFILE" = "dev" ; then
    BINARY=../../target/${TARGET}/debug/${BIN}
else
    BINARY=../../target/${TARGET}/${PROFILE}/${BIN}
fi

cp "${BINARY}" init.elf

# equivalent to 'objcopy -I elf64-little -O binary "${BINARY}" init.bin'
cargo run -p elf --bin dump_img -- "${BINARY}" init.bin
