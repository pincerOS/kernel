#!/usr/bin/env bash

sudo ip tuntap add dev tap0 mode tap
sudo ip link set tap0 up

sudo brctl addbr br0
sudo brctl addif br0 enp0s31f6
sudo brctl addif br0 tap0

sudo ip link set enp0s31f6 up
sudo ip link set br0 up

sudo ip addr flush dev enp0s31f6

sudo dhclient br0

# may need to edit resolv.conf with nameserver
