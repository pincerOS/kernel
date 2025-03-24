#!/usr/bin/env bash

set -ex

EXAMPLE=${1-"main"}
TARGET=aarch64-unknown-none-softfloat
PROFILE=${PROFILE-"release"}

# cargo clean
cargo rustc --profile="${PROFILE}" --example="${EXAMPLE}" \
    --target="${TARGET}" -- \
    -C relocation-model=static \
    -C force-frame-pointers=true

if test "$PROFILE" = "dev" ; then
    BINARY="../../target/${TARGET}/debug/examples/${EXAMPLE}"
else
    BINARY="../../target/${TARGET}/${PROFILE}/examples/${EXAMPLE}"
fi

cp "${BINARY}" kernel.elf

# equivalent to 'objcopy -I elf64-little -O binary "${BINARY}" init.bin'
cargo dump-img "${BINARY}" kernel.bin
