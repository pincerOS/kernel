/* SPDX-License-Identifier: GPL-2.0+ */
/*
 * Copyright (c) 2013 Google, Inc
 *
 * (C) Copyright 2012
 * Pavel Herrmann <morpheus.ibis@gmail.com>
 * Marek Vasut <marex@denx.de>
 */

//from u-boot/include/dm/device.h

#![allow(non_camel_case_types)]

use core::ptr::NonNull;
use core::ffi::c_void;

#[repr(C)]
pub struct list_head {
    pub next: *mut list_head,
    pub prev: *mut list_head,
}

#[repr(C)]
pub struct udevice {
    pub driver: *const driver,
    pub name: *const u8,
    pub plat_: *mut c_void,
    pub parent_plat_: *mut c_void,
    pub uclass_plat_: *mut c_void,
    pub driver_data: u64,
    pub parent: *mut udevice,
    pub priv_: *mut c_void,
    pub uclass: *mut uclass,
    pub uclass_priv_: *mut c_void,
    pub parent_priv_: *mut c_void,
    pub uclass_node: list_head,
    pub child_head: list_head,
    pub sibling_node: list_head,
    #[cfg(not(CONFIG_IS_ENABLED_OF_PLATDATA_RT))]
    pub flags_: u32,
    pub seq_: i32,
    #[cfg(CONFIG_IS_ENABLED_OF_REAL)]
    pub node_: ofnode,
    #[cfg(CONFIG_IS_ENABLED_DEVRES)]
    pub devres_head: list_head,
    #[cfg(CONFIG_IS_ENABLED_DM_DMA)]
    pub dma_offset: u64,
    #[cfg(CONFIG_IS_ENABLED_IOMMU)]
    pub iommu: *mut udevice,
}

#[repr(C)]
pub struct driver;

#[repr(C)]
pub struct uclass;

#[repr(C)]
pub struct ofnode;