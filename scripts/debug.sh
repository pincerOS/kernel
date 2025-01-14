#!/usr/bin/bash

qemu-system-aarch64 -M raspi3b -dtb bcm2710-rpi-3-b-plus.dtb -display none -serial stdio -kernel kernel.bin \
    -d int,mmu,guest_errors -s -S
