SERIAL_DEV=/dev/tty.usbserial-B0044ASI
LOGFILE=log/$(date -Is).log
echo "Logging to $LOGFILE"
tio "$SERIAL_DEV" -b 115200 -d 8 -s 1 -p none -f none -m INLCRNL -L --log-file="$LOGFILE"
echo "Log saved to $LOGFILE"
