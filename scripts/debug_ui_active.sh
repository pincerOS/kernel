#!/usr/bin/env bash

WAYLAND_DISPLAY= GDK_SCALE=1 qemu-system-aarch64 \
    -M raspi3b -dtb bcm2710-rpi-3-b-plus.dtb \
    -kernel kernel.bin \
    -serial stdio \
    -d mmu,guest_errors \
    -s
