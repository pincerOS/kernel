#!/usr/bin/bash

qemu-system-aarch64 -M raspi3b -dtb bcm2710-rpi-3-b-plus.dtb -display none -serial stdio -kernel kernel.bin \
    -d mmu,guest_errors

#-d int
