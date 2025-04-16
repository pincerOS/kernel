#!/usr/bin/env bash

set -ex

TARGET=aarch64-unknown-none-softfloat
PROFILE=${PROFILE:-release}

for src in src/bin/*.rs; do
    BIN=$(basename "${src}" .rs)

    cargo rustc --profile="${PROFILE}" \
        --bin "${BIN}" \
        --target=${TARGET} -- \
        -C relocation-model=static

    if [ "$PROFILE" = "dev" ]; then
        BINARY=../../target/${TARGET}/debug/${BIN}
    else
        BINARY=../../target/${TARGET}/${PROFILE}/${BIN}
    fi

    cp "${BINARY}" "${BIN}.elf"
done
