#!/usr/bin/env bash

set -ex
cd "$(dirname "$0")"

BIN="display-server"
TARGET=aarch64-unknown-none-softfloat
PROFILE=${PROFILE-"release"}

cargo rustc --profile="${PROFILE}" \
    --target="${TARGET}" \
    --bin="${BIN}" \
    -- -C relocation-model=static

if test "$PROFILE" = "dev" ; then
    BINARY=../../target/${TARGET}/debug/${BIN}
else
    BINARY=../../target/${TARGET}/${PROFILE}/${BIN}
fi

cp "${BINARY}" "${BIN}".elf

if test -n "$DESTDIR" ; then
    cp "${BINARY}" "$DESTDIR/"
fi
