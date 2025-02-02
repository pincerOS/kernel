#!/usr/bin/env bash

set -e

QEMU_TARGET_HARDWARE=${QEMU_TARGET_HARDWARE-"-M raspi4b -dtb bcm2711-rpi-4-b.dtb"}
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
    ${QEMU_TARGET_HARDWARE} \
    -kernel kernel.bin \
    -serial stdio \
    ${QEMU_DISPLAY} \
    ${QEMU_DEBUG_PFX} ${QEMU_DEBUG} \
    ${DEBUG_ARGS}
