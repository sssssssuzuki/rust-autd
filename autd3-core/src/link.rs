/*
 * File: link.rs
 * Project: src
 * Created Date: 27/04/2022
 * Author: Shun Suzuki
 * -----
 * Last Modified: 31/05/2022
 * Modified By: Shun Suzuki (suzuki@hapis.k.u-tokyo.ac.jp)
 * -----
 * Copyright (c) 2022 Shun Suzuki. All rights reserved.
 *
 */

use anyhow::Result;
use autd3_driver::{RxDatagram, TxDatagram};

/// Link is a interface to the AUTD device.
pub trait Link: Send {
    fn open(&mut self) -> Result<()>;
    fn close(&mut self) -> Result<()>;
    fn send(&mut self, tx: &TxDatagram) -> Result<bool>;
    fn receive(&mut self, rx: &mut RxDatagram) -> Result<bool>;
    fn cycle_ticks(&self) -> u16;
    fn is_open(&self) -> bool;
}
