#!/usr/bin/env bash

set -e

# Check if sdcard.img exists, create it if not
if [ ! -f sdcard.img ]; then
    echo "Creating SD card image"
    dd if=/dev/zero of=sdcard.img bs=1M count=4096
    mkfs.fat -F 32 sdcard.img
    echo "SD card image created."
fi

QEMU_TARGET_HARDWARE=${QEMU_TARGET_HARDWARE-"-M raspi4b -dtb bcm2711-rpi-4-b.dtb"}
QEMU_DEBUG=${QEMU_DEBUG-"mmu,guest_errors"}
QEMU_DISPLAY=${QEMU_DISPLAY-"none"}
DEBUG_ARGS=${DEBUG_ARGS-"-s"}

# DEBUG_ARGS="-s -S"  (wait until connected)
# DEBUG_ARGS="-s"     (run, attach debugger later)
# DEBUG_ARGS=""       (no debugging)

# QEMU_DEBUG="mmu,guest_errors,int,trace:bcm2835*"  (also log interrupts)

# QEMU_DISPLAY="default"  (show the framebuffer)

if test -z "$QEMU_DEBUG"; then
    QEMU_DEBUG_PFX=""
else
    QEMU_DEBUG_PFX="-d"
fi

if test "$DEBUG_ARGS" = "-s -S" ; then
    echo "# Waiting for debugger; run:"
    echo 'gdb kernel.elf -ex "target remote localhost:1234"'
fi

# TODO: qemu's pipe handling drops characters, doesn't flush
# UART_PIPE="uart2"
# if test ! -p "$UART_PIPE" ; then
#     mkfifo "$UART_PIPE"
# fi
# SERIAL_ALT="pipe:$UART_PIPE"
# SERIAL_ALT="tcp:127.0.0.1:5377" # listen with nc -kl 5377

qemu-system-aarch64 \
    ${QEMU_TARGET_HARDWARE} \
    -kernel kernel.bin \
    # -serial stdio \
    -display "${QEMU_DISPLAY}" \
    -sd sdcard.img \
    -monitor stdio \
    "${QEMU_DEBUG_PFX}" "${QEMU_DEBUG}" \
    ${DEBUG_ARGS}
    
