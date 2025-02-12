#!/usr/bin/env bash

set -e
SOURCE=$(realpath "$1")
NAME=$(basename "$SOURCE" .rs)
RELATIVE=$(realpath --relative-to=. "$SOURCE")

MANIFEST="$(cat)"

LINKER_SCRIPT="$(dirname "$0")/script.ld"

TEMP_DIR="$(mktemp -d)"
trap 'rm -rf -- "$TEMP_DIR"' EXIT
MANIFEST_PATH="$TEMP_DIR/Cargo.toml"
echo "$MANIFEST" > "$MANIFEST_PATH"

TARGET_DIR=$(cargo metadata --format-version 1 --no-deps | \
    python3 -c 'print(__import__("json").loads(input())["target_directory"])')

CARGO_TARGET_DIR="$TARGET_DIR" RUSTC_BOOTSTRAP=1 cargo rustc \
    --manifest-path="$MANIFEST_PATH" \
    --target=aarch64-unknown-none-softfloat \
    --profile=standalone \
    --bin "$NAME" \
    -- \
    -C link-arg=-T"${LINKER_SCRIPT}" -C link-args='-zmax-page-size=0x1000' \

BIN_PATH="${TARGET_DIR}/aarch64-unknown-none-softfloat/standalone/${NAME}"
cp "${BIN_PATH}" "${SOURCE%.rs}.elf"

SIZE=$(stat -c %s "${SOURCE%.rs}.elf" | python3 -c \
    "(lambda f:f(f,float(input()),0))\
     (lambda f,i,j:print('%.4g'%i,'BKMGTPE'[j]+'iB' if j else 'bytes')\
     if i<1024 else f(f,i/1024,j+1))"
)
echo "Built ${RELATIVE%.rs}.elf, file size ${SIZE}"
