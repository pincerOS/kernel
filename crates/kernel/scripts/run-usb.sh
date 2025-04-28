#!/usr/bin/env bash
set -ex
QEMU_DISPLAY=${QEMU_DISPLAY-"default"} QEMU_DEVICES="-device usb-net,netdev=net0 -netdev user,id=net0,hostfwd=tcp::2222-:22 -object filter-dump,id=f1,netdev=net0,file=net0.pcap" "$(dirname "$0")/run.sh"
# QEMU_DISPLAY=${QEMU_DISPLAY-"default"} QEMU_DEVICES="-usb -device usb-kbd -device usb-mouse -device usb-net,netdev=net0 -netdev tap,id=net0,ifname=tap0,script=no,downscript=no -object filter-dump,id=f1,netdev=net0,file=net0.pcap" "$(dirname "$0")/run.sh"

# QEMU_DISPLAY=${QEMU_DISPLAY-"default"} QEMU_DEVICES="-usb -device usb-kbd -device usb-mouse -device usb-net,netdev=net0 -netdev bridge,id=net0,br=br0 -object filter-dump,id=f1,netdev=net0,file=net0.pcap" "$(dirname "$0")/run.sh"
# QEMU_DISPLAY=${QEMU_DISPLAY-"default"} QEMU_DEVICES="-device usb-hub,id=hub1 -device usb-kbd -device usb-mouse" "$(dirname "$0")/run.sh"
# QEMU_DISPLAY=${QEMU_DISPLAY-"default"} QEMU_DEVICES="-usb -device usb-kbd -device usb-mouse,pcap=usb-mouse.pcap -device usb-net,netdev=net0 -netdev user,id=net0,hostfwd=tcp::2222-:22 -object filter-dump,id=f1,netdev=net0,file=net0.pcap" "$(dirname "$0")/run.sh"
# # QEMU_DISPLAY=${QEMU_DISPLAY-"default"} QEMU_DEVICES="-usb -device usb-kbd -device usb-mouse -device usb-net,netdev=net0 -netdev tap,id=net0,ifname=tap0,script=no,downscript=no -object filter-dump,id=f1,netdev=net0,file=net0.pcap" "$(dirname "$0")/run.sh"
# QEMU_DISPLAY=${QEMU_DISPLAY-"default"} QEMU_DEVICES="-device usb-net,netdev=net0 -netdev tap,id=net0,ifname=tap0,script=no,downscript=no -object filter-dump,id=f1,netdev=net0,file=net0.pcap" "$(dirname "$0")/run.sh"
