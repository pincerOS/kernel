#!/usr/bin/env bash

set -ex
QEMU_DISPLAY=${QEMU_DISPLAY-"default"} QEMU_DEVICES="-usb -device usb-kbd,pcap=usb-kbd.pcap " "$(dirname "$0")/run.sh"
# QEMU_DISPLAY=${QEMU_DISPLAY-"default"} QEMU_DEVICES="-usb -device usb-kbd -device usb-mouse,pcap=usb-mouse.pcap -device usb-net,netdev=net0 -netdev user,id=net0,hostfwd=tcp::2222-:22 -object filter-dump,id=f1,netdev=net0,file=net0.pcap" "$(dirname "$0")/run.sh"
