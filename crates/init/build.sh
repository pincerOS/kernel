#!/usr/bin/env bash

set -ex

BIN="init"
TARGET=aarch64-unknown-none-softfloat
PROFILE=${PROFILE-"release"}

mkdir -p fs

./shell.rs
cp shell.elf fs/
./ls.rs
cp ls.elf fs/
./sharedMemTest.rs
cp sharedMemTest.elf fs/

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

# equivalent to 'objcopy -I elf64-little -O binary "${BINARY}" init.bin'
cargo run -p elf --bin dump_img -- "${BINARY}" init.bin
