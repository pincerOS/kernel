#!/usr/bin/env bash

set -ex
QEMU_DISPLAY=${QEMU_DISPLAY-"default"} QEMU_DEVICES="-usb -device usb-kbd" "$(dirname "$0")/run.sh"
