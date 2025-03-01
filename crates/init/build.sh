#!/usr/bin/env bash

set -ex

BIN="init"
TARGET=aarch64-unknown-none-softfloat
PROFILE=${PROFILE-"release"}

mkdir -p fs

./example.rs
cp example.elf fs/

cargo run -q -p initfs --bin util \
    -- create --compress --out fs.arc --root fs fs

# cargo clean
cargo rustc --profile="${PROFILE}" \
    --target=${TARGET} -- \
    -C relocation-model=static

if test "$PROFILE" = "dev" ; then
    BINARY=../../target/${TARGET}/debug/${BIN}
else
    BINARY=../../target/${TARGET}/${PROFILE}/${BIN}
fi

cp "${BINARY}" init.elf
objcopy -I elf32-little -O binary "${BINARY}" init.bin
