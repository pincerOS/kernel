#!/usr/bin/env sh

SOURCE="$0" NAME="$(basename "$0" .rs)"
DIR=$(cd "$(dirname "$0")" || exit ; pwd -P)
ULIB_DIR="$(git rev-parse --show-toplevel)/crates/ulib"
exec "$ULIB_DIR/compile.sh" "$0" <<END_MANIFEST
[package]
name = "$NAME"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "$NAME"
path = "$DIR/$NAME.rs"

[dependencies]
ulib = { path = "$ULIB_DIR" }

[profile.standalone]
inherits = "release"
opt-level = 0
panic = "abort"
strip = "debuginfo"

END_MANIFEST
exit