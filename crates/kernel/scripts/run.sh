#!/usr/bin/env bash

set -e

# Check if sdcard.img exists, create it if not
if [ ! -f sdcard.img ]; then
    echo "Creating SD card image"
    mkfs.ext2 -q -b 4096 -i 4096 -I 128 -r 0 -t ext2 -d ../init/fs sdcard.img 32m
    echo "SD card image created"
fi

QEMU_TARGET_HARDWARE=${QEMU_TARGET_HARDWARE-"-M raspi4b -dtb bcm2711-rpi-4-b.dtb"}
QEMU_DEBUG=${QEMU_DEBUG-"mmu,guest_errors"}
QEMU_DISPLAY=${QEMU_DISPLAY-"none"}
DEBUG_ARGS=${DEBUG_ARGS-"-s"}
QEMU_DEVICES=${QEMU_DEVICES-""}

# DEBUG_ARGS="-s -S"  (wait until connected)
# DEBUG_ARGS="-s"     (run, attach debugger later)
# DEBUG_ARGS=""       #(no debugging)

# QEMU_DEBUG="guest_errors,trace:usb_dwc2_wakeup_endpoint,trace:usb_hub_control,trace:usb_dwc2_work_bh_next,trace:usb_dwc2_work_bh_service,trace:usb_dwc2_packet_done,trace:usb_dwc2_packet_next,trace:usb_packet_state_fault,trace:usb_packet_state_change,trace:usb_dwc2_device_not_found,trace:usb_dwc2_device_found,trace:usb_dwc2_port_disabled,trace:usb_dwc2_async_packet_complete,trace:usb_dwc2_enable_chan,trace:usb_dwc2_work_bh,trace:usb_dwc2_memory_write,trace:usb_dwc2_async_packet,trace:usb_dwc2_packet_error,trace:usb_dwc2_handle_packet,trace:usb_dwc2_memory_read,trace:usb_dwc2_packet_status,trace:usb_dwc2_attach_speed"  #(also log interrupts)
# QEMU_DEBUG="guest_errors"
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
#UART_PIPE="uart2"
#if test ! -p "$UART_PIPE" ; then
#    mkfifo "$UART_PIPE"
#fi
#SERIAL_ALT="pipe:$UART_PIPE"
# SERIAL_ALT="tcp:127.0.0.1:5377" # listen with nc -kl 5377

qemu-system-aarch64 \
    ${QEMU_TARGET_HARDWARE} \
    -kernel kernel.bin \
    -drive file=sdcard.img,if=sd,format=raw \
    -serial null \
    -serial stdio \
    -display "${QEMU_DISPLAY}" \
    "${QEMU_DEBUG_PFX}" "${QEMU_DEBUG}" \
    ${QEMU_DEVICES} \
    ${DEBUG_ARGS}

# -device usb-kbd \
# -device usb-net,netdev=net0 \
# -netdev user,id=net0,hostfwd=tcp::2222-:22 \
# -object filter-dump,id=f1,netdev=net0,file=net0.pcap \
# -trace enable=net* \

rm -f sdcard.img
