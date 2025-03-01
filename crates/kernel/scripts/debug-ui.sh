#!/usr/bin/env bash

set -ex
DEBUG_ARGS="-s -S" QEMU_DISPLAY="default" "$(dirname "$0")/run.sh"
