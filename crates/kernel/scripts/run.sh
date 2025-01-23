#!/usr/bin/env bash

set -e

QEMU_DEBUG=${QEMU_DEBUG-"mmu,guest_errors"}
QEMU_DISPLAY=${QEMU_DISPLAY-"-display none"}
DEBUG_ARGS=${DEBUG_ARGS-"-s"}

# DEBUG_ARGS="-s -S"  (wait until connected)
# DEBUG_ARGS="-s"     (run, attach debugger later)
# DEBUG_ARGS=""       (no debugging)

# QEMU_DEBUG="mmu,guest_errors,int"  (also log interrupts)

# QEMU_DISPLAY=""  (show the framebuffer)

if test -z "$QEMU_DEBUG"; then
    QEMU_DEBUG_PFX=""
else
    QEMU_DEBUG_PFX="-d"
fi

if test "$DEBUG_ARGS" = "-s -S" ; then
    echo "# Waiting for debugger; run:"
    echo 'gdb kernel.elf -ex "target remote localhost:1234"'
fi

qemu-system-aarch64 \
    -M raspi3b -dtb bcm2710-rpi-3-b-plus.dtb \
    -kernel kernel.bin \
    -serial stdio \
    ${QEMU_DISPLAY} \
    ${QEMU_DEBUG_PFX} ${QEMU_DEBUG} \
    ${DEBUG_ARGS}
