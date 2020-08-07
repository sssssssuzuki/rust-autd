/*
 * File: constants.rs
 * Project: src
 * Created Date: 21/11/2019
 * Author: Shun Suzuki
 * -----
 * Last Modified: 07/08/2020
 * Modified By: Shun Suzuki (suzuki@hapis.k.u-tokyo.ac.jp)
 * -----
 * Copyright (c) 2019 Hapis Lab. All rights reserved.
 *
 */

pub const AUTD_WIDTH: f64 = 192.0;
pub const AUTD_HEIGHT: f64 = 151.4;

pub const TRANS_SIZE: f64 = 10.18;
pub const NUM_TRANS_X: usize = 18;
pub const NUM_TRANS_Y: usize = 14;
pub const NUM_TRANS_IN_UNIT: usize = NUM_TRANS_X * NUM_TRANS_Y - 3;
pub const ULTRASOUND_WAVELENGTH: f64 = 8.5;
pub const ULTRASOUND_FREQUENCY: f64 = 40000.0;

pub const MOD_SAMPLING_FREQUENCY: f64 = 4000.0;
pub const MOD_BUF_SIZE: u16 = 4000;

pub const MOD_FRAME_SIZE: usize = 120;
pub const HEADER_SIZE: usize = MOD_FRAME_SIZE + 8;
pub const BODY_SIZE: usize = NUM_TRANS_IN_UNIT * 2;
pub const OUTPUT_FRAME_SIZE: usize = HEADER_SIZE + BODY_SIZE;
pub const INPUT_FRAME_SIZE: usize = 2;

pub const EC_DEVICE_PER_FRAME: usize = 2;
pub const EC_SPEED_BPS: f64 = 100.0 * 1000.0 * 1000.0;
pub const EC_FRAME_LENGTH: usize =
    14 + 2 + (10 + OUTPUT_FRAME_SIZE + INPUT_FRAME_SIZE + 2) * EC_DEVICE_PER_FRAME + 10;
pub const EC_TRAFFIC_DELAY: f64 = (EC_FRAME_LENGTH * 8) as f64 / EC_SPEED_BPS;

pub const POINT_SEQ_BUFFER_SIZE_MAX: usize = 40000;
pub const POINT_SEQ_CLK_IDX_MAX: usize = 40000;

pub const POINT_SEQ_BASE_FREQ: f64 = 40000.0;
pub const POINT_SEQ_BASE_INTERVAL_US: f64 = 1000000.0 / POINT_SEQ_BASE_FREQ; // 25 us
