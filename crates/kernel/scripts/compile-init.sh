#!/usr/bin/env bash

set -ex
cd "$(dirname "$0")/.."

FS_PATH="../../disk-image"

DESTDIR="$FS_PATH" ../shell/build.sh
DESTDIR="$FS_PATH" ../console/build.sh
DESTDIR="$FS_PATH" ../display-server/build.sh
DESTDIR="$FS_PATH" ../paint/build.sh
DESTDIR="$FS_PATH" ../show/build.sh
../init/build.sh
