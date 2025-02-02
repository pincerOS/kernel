#!/usr/bin/env bash

set -ex
QEMU_TARGET_HARDWARE="-M raspi3b -dtb bcm2710-rpi-3-b-plus.dtb" $(dirname "$0")/run.sh
