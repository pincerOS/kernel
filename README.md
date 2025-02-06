<!-- LOGO -->
<br />
<h1>
<p align="center">
  <img src="/img/logo.png" alt="Logo" width="140" height="140">
  <br>PincerOS
</h1>
  <p align="center">
    Bare metal microkernel-based multi-core operating system written in Rust for the Raspberry Pi 4b.
    <br />
    </p>
</p>
<p align="center">
  <a href="#about-the-project">About The Project</a> •
  <a href="#features">Features</a> •
  <a href="#architecture">Architecture</a> •
  <a href="#installation">Installation</a> •
  <a href="#development-status">Development Status</a> •
  <a href="#credits">Credits</a> •
  <a href="#license">License</a>
</p>  

<!--
<p align="center">
  add clip here when we have something cool to show
![screenshot](clip.gif)
</p>                                                                                                                             
                                                                                                                                                      

                                                                                                                                                      -->
# About The Project 🦀

PincerOS is a bare-metal microkernel-based multi-core operating system written from the ground up in Rust targeting the Raspberry Pi 4b. The project aims to be a distributed, scalable, and secure operating system for general-purpose use. We aim to support a wide range of applications such as networked video games, distributed computing, and more.

## Features ✨

- Microkernel Architecture
- Multi-core Support
- Memory Management
- Process Scheduling
- Ext2 File System with Journaling
- Inter-process Communication (IPC)
- Device Drivers
- Networking
- Security

## Architecture 📐
PincerOS follows a microkernel architecture with the following key components:

- Kernel Core: Handles basic system operations, syscalls, scheduling, and IPC
- Memory Management: Implements virtual memory and memory protection
- Device Drivers: Manages hardware interfaces
- Network Stack: Provides networking capabilities
- Security Module: Handles access control and system security

# Installation 📦
Currently, the project can be tested on QEMU version 9.0 or higher. If your package manager doesn't have it, you will have to build QEMU from source.

## Dependencies
- Rust toolchain (https://www.rust-lang.org/tools/install)
- QEMU >= 9.0 (https://www.qemu.org/download/)
- llvm (https://llvm.org/docs/GettingStarted.html):
`brew install llvm` or `sudo apt-get install llvm`
- on MacOS, for a temporary fix for issues related to llvm-objcopy:
`brew install binutils`
`sudo ln -s $(which gobjcopy) /usr/local/bin/llvm-objcopy`

## Setup
1. Install Rust target:
`rustup target add aarch64-unknown-none-softfloat`
2. Clone the repository:
`git clone https://github.com/pincerOS/kernel.git`
3. Build the kernel:
`./scripts/build.sh` to build and
`./scripts/run.sh` to run.

We also provide scripts for debugging and running with ui:
`build-debug.sh`, `debug-ui.sh`, `run-rpi3b.sh`, `run-ui.sh`.


# Development Status 🚧

- [x] Basic kernel functionality
- [x] Multi-core support
- [ ] Network stack
- [ ] Application support
- [ ] File system
- [ ] Device drivers
- [ ] Security module
- [ ] Distributed computing support


# Credits 🎓
This project is a collaboration between students at the University of Texas at Austin. 🤘

- Aaron Lo
- Alex Meyer
- Anthony Wang
- Bobby Youstra
- Caleb Eden
- Elie Soloveichik
- Hunter Ross
- Joyce Lai
- Neil Allavarpu
- Sasha Huang
- Slava Andrianov

# License 📝

This project is licensed under the MIT License.

---

> _"Rust never sleeps." -Neil Young_