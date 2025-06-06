#!/usr/bin/env bash

echo "Enabling IP forwarding..."
sudo bash -c "echo 1 > /proc/sys/net/ipv4/ip_forward"

echo "Appending FORWARD rule: ACCEPT output to interface: tap0..."
sudo iptables -A FORWARD -o tap0 -j ACCEPT

echo "Appending FORWARD rule: ACCEPT input via interface: tap0..."
sudo iptables -A FORWARD -i tap0 -j ACCEPT

# POSTROUTING: alter packets as they leave firewall's external device.
echo "Appending POSTROUTING rule into NAT table: mask source to 192.0.2.0/24 and output to interface: eth0..."
# Replace output interface if your machine has different name below: 
sudo iptables -t nat -A POSTROUTING -s 192.0.2.0/24 -o wlp0s20f3 -j MASQUERADE

# # Revert
# sudo iptables -D FORWARD -o tap0 -j ACCEPT
# sudo iptables -D FORWARD -i tap0 -j ACCEPT
# sudo iptables -t nat -D POSTROUTING -s 192.0.2.0/24 -o wlp0s20f3 -j MASQUERADE

