#!/usr/bin/env bash

set -ex
DEBUG_ARGS="-s -S" QEMU_DISPLAY="" $(dirname "$0")/run.sh
