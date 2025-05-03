<!-- LOGO -->
<br />
<h1>
<p align="center">
  <img src="/img/pinceros.svg" alt="Logo" width="140" height="140">
  <br>PincerOS
</h1>
  <p align="center">
    Bare metal microkernel-based multi-core operating system written in Rust for the Raspberry Pi 4b.
    <br />
    </p>
</p>
<p align="center">
  <a href="#about-the-project-">About The Project</a> •
  <a href="#targeted-features-">Features</a> •
  <a href="#architecture-">Architecture</a> •
  <a href="#installation-">Installation</a> •
  <a href="#development-status-">Development Status</a> •
  <a href="#credits-">Credits</a> •
  <a href="#license-">License</a>
</p>

<!--
<p align="center">
  add clip here when we have something cool to show
![screenshot](clip.gif)
</p>


                                                                                                                                                      -->
# About The Project 🦀

PincerOS is a bare-metal monolithic kernel multi-core operating system written from the ground up in Rust targeting the Raspberry Pi 4B. The project aims to be a distributed, scalable, and secure operating system for general-purpose use. We aim to support a wide range of applications such as networked video games, distributed computing, and more.

For more information about our kernel, its features, and its development, please visit the [PincerOS Blog](https://pinceros.github.io/)!

## Targeted Features ✨

- Monolithic Kernel Architecture
- Multi-core Support
- Memory Management
- Process Scheduling
- File System with Journaling Support
- Inter-process Communication (IPC)
- Device Drivers
- Networking
- Security

## Kernel Architecture 📐
PincerOS has the following key kernel components:

- Kernel Core: Handles basic system operations, syscalls, scheduling, and IPC
- Memory Management: Implements virtual memory and memory protection
- Device Drivers: Manages hardware interfaces
- Network Stack: Provides networking capabilities
- Security: Handles access control and system security

## Userspace Features
PincerOS makes the following features and applications availabile in its userspace

- ulib - a userspce library which provides user level applications with an API to use system calls
- Display Server - Allows for multiple processes to have graphical windows which simultaneously display content on a monitor. Please view the demo on the PincerOS blog to see the display sever in action for applications such as Doom, a drawing application, and more!
- Shell - a user space shell with common utilities

# Installation 📦
Currently, the project can be tested on QEMU version 9.0 or higher. If your package manager doesn't have it, you will have to build QEMU from source.

## Dependencies
- Rust toolchain (https://www.rust-lang.org/tools/install)
- QEMU >= 9.0 (https://www.qemu.org/download/)
- Just (https://github.com/casey/just?tab=readme-ov-file#packages)

## Setup
<!-- 1. Install Rust target:
```rustup target add aarch64-unknown-none-softfloat``` -->

1. Clone the repository:
```git clone https://github.com/pincerOS/kernel.git```

2. Build  and run the kernel

This can be accomplished by navigating to the cloned kernel directory and then running the following series of shell commands:

```bash
./crates/kernel/scripts/compile-init.sh
./crates/kernel/scripts/build.sh user
./crates/kernel/scripts/run-usb.sh
```

Alternativley, you can also use Just

To build and run the main example:

```bash
cd crates/kernel
just build-and-run

```


We also provide scripts for debugging and running with ui:
```bash
just --list
Available recipes:
    build example=example profile=profile target=target # Build the kernel
    build-and-run example=example profile=profile # Build and run in one command
    build-and-run-debug example=example           # Build and run with debug profile
    build-debug example=example target=target     # Build with debug profile
    debug                                         # Run with debug mode (wait for debugger)
    debug-ui                                      # Run with debug mode and UI display
    default                                       # Default: build and run the kernel
    run qemu_target=qemu_target qemu_debug=qemu_debug qemu_display=qemu_display debug_args=debug_args # Run the kernel in QEMU
    run-rpi3b                                     # Run on Raspberry Pi 3B
    run-ui                                        # Run with UI display
```

# Development Status 🚧

- [x] Basic kernel functionality
- [x] Multi-core support
- [x] Network stack
- [x] Application support
- [x] File system
- [x] Device drivers
- [ ] Security
- [ ] Distributed computing support


# Credits 🎓
This project is a collaboration between students at the University of Texas at Austin. 🤘

- Aaron Lo (@22aronl)
- Alex Meyer (@ameyer1024)
- Anthony Wang (@honyant)
- Caleb Eden (@calebeden)
- Hunter Ross (@hunteross)
- @InsightGit
- Joyce Lai (@hexatedjuice)
- Neil Allavarpu (@NeilAllavarpu)
- @Razboy20
- Sasha Huang (@umbresp)
- Slava Andrianov (@Slava-A1)


This project also incorporates code from [CSUD](https://github.com/Chadderz121/csud/tree/master), made by Alex Chadwick and licensed under the MIT License (See CSUD_LICENSE for more details). CSUD was used as a base for the USB implementation and additional support of Interrupt & Bulk Endpoints were added on.
# License 📝

This project is licensed under the MIT License.

---

> _"Rust never sleeps." -Neil Young_
