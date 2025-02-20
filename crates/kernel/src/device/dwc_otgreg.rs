/*-
 * SPDX-License-Identifier: BSD-2-Clause
 *
 * Copyright (c) 2010,2011 Aleksandr Rybalko. All rights reserved.
 * 
 * Modified in 2025 for Rust compatibility and PincerOS by Aaron Lo <aaronlo0929@gmail.com>
 *
 * Redistribution and use in source and binary forms, with or without
 * modification, are permitted provided that the following conditions
 * are met:
 * 1. Redistributions of source code must retain the above copyright
 *    notice, this list of conditions and the following disclaimer.
 * 2. Redistributions in binary form must reproduce the above copyright
 *    notice, this list of conditions and the following disclaimer in the
 *    documentation and/or other materials provided with the distribution.
 *
 * THIS SOFTWARE IS PROVIDED BY THE AUTHOR AND CONTRIBUTORS ``AS IS'' AND
 * ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
 * IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE
 * ARE DISCLAIMED.  IN NO EVENT SHALL THE AUTHOR OR CONTRIBUTORS BE LIABLE
 * FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
 * DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS
 * OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION)
 * HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT
 * LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY
 * OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF
 * SUCH DAMAGE.
 */

pub const EINVAL: u32 = 22;

pub const fn GRSTCTL_TXFIFO(n: u32) -> u32 { ((n) & 31) << 6}
pub const GRSTCTL_TXFFLSH: u32 = 1 << 5;
pub const GRSTCTL_RXFFLSH: u32 = 1 << 4;

pub const GINTMSK_HCHINTMSK: u32 = 1 << 25;
pub const GINTMSK_IEPINTMSK: u32 = 1 << 18;

pub const GOTGCTL_BSESVLD: u32 = 1 << 19;
pub const GOTGCTL_ASESVLD: u32 = 1 << 18;

pub const HCINT_DEFAULT_MASK: u32 = 
    HCINT_STALL | HCINT_BBLERR |
    HCINT_XACTERR | HCINT_NAK | HCINT_ACK | HCINT_NYET |
    HCINT_CHHLTD | HCINT_FRMOVRUN |
    HCINT_DATATGLERR;

pub const HCINT_SOFTWARE_ONLY: u32 = 1 << 20;
pub const HCINT_DATATGLERR: u32 = 1 << 10;
pub const HCINT_FRMOVRUN: u32 = 1 << 9;
pub const HCINT_BBLERR: u32 = 1 << 8;
pub const HCINT_XACTERR: u32 = 1 << 7;
pub const HCINT_NYET: u32 = 1 << 6;
pub const HCINT_ACK: u32 = 1 << 5;
pub const HCINT_NAK: u32 = 1 << 4;
pub const HCINT_STALL: u32 = 1 << 3;
pub const HCINT_AHBERR: u32 = 1 << 2;
pub const HCINT_CHHLTD: u32 = 1 << 1;
pub const HCINT_XFERCOMPL: u32 = 1 << 0;
