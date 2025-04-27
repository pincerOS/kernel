#!/usr/bin/env bash

SERIAL_DEV=/dev/serial/by-id/usb-FTDI_FT232R_USB_UART_B0044ASI-if00-port0
LOGFILE=log/$(date -Is).log
echo "Logging to $LOGFILE"
tio "$SERIAL_DEV" -b 115200 -d 8 -s 1 -p none -f none -m INLCRNL -L --log-file="$LOGFILE"
echo "Log saved to $LOGFILE"
