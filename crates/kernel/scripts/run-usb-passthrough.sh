#!/usr/bin/env bash

set -ex
QEMU_DISPLAY=${QEMU_DISPLAY-"default"} \
QEMU_DEVICES="-usb -device usb-host,hostbus=3,hostport=4.3" "$(dirname "$0")/run.sh"
#QEMU_DEVICES="-usb -device usb-host,vendorid=0x1532,productid=0x025e" "$(dirname "$0")/run.sh"
#QEMU_DEVICES="-usb -device usb-host,vendorid=0x2109,productid=0x2817" "$(dirname "$0")/run.sh"

