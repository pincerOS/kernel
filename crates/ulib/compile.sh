#!/usr/bin/env bash

set -e
ULIB_DIR=$(cd "$(dirname -- "$0")" && pwd -P)
SOURCE_DIR=$(cd "$(dirname -- "$1")" && pwd -P)
SOURCE="$SOURCE_DIR/$(basename -- "$1")"
NAME=$(basename -- "$SOURCE" .rs)

MANIFEST="$(cat)"

LINKER_SCRIPT="$ULIB_DIR/script.ld"

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
ELF_FILE="${SOURCE%.rs}.elf"
cp "${BIN_PATH}" "$ELF_FILE"

if test -f "$ELF_FILE" ; then
    # MacOS compat -- try everything, hope at least one works
    SIZE=$(stat -c%s -- "$ELF_FILE" 2>/dev/null || stat -f%z -- "$ELF_FILE" 2>/dev/null || find "$ELF_FILE" -printf "%s")
    SIZE=$(echo "${SIZE}" | python3 -c \
        "(lambda f:f(f,float(input()),0))\
        (lambda f,i,j:print('%.4g'%i,'BKMGTPE'[j]+'iB' if j else 'bytes')\
        if i<1024 else f(f,i/1024,j+1))"
    )
    RELATIVE=$(realpath --relative-to=. -- "$SOURCE" || echo "$SOURCE")
    echo "Built ${RELATIVE%.rs}.elf, file size ${SIZE}"
else
    echo "Failed to build $ELF_FILE?"
    exit 1
fi
