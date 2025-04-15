#!/usr/bin/env bash

sudo ip tuntap add dev tap0 mode tap
sudo ip link set tap0 up
sudo brctl addbr br0
sudo brctl addif br0 eth0
sudo brctl addif br0 tap0
sudo ip link set br0 up