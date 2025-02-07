#!/usr/bin/env bash

set -ex

EXAMPLE=${1-"user"}
TARGET=aarch64-unknown-none-softfloat
PROFILE=${PROFILE-"release"}

# cargo clean
cargo rustc --profile="${PROFILE}" --example="${EXAMPLE}" \
    --target=${TARGET} -- \
    -C relocation-model=static

if test "$PROFILE" = "dev" ; then
    BINARY=../../target/${TARGET}/debug/examples/${EXAMPLE}
else
    BINARY=../../target/${TARGET}/${PROFILE}/examples/${EXAMPLE}
fi

cp "${BINARY}" kernel.elf
llvm-objcopy -O binary "${BINARY}" kernel.bin
