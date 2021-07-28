/*
 * File: soem_link.rs
 * Project: src
 * Created Date: 02/09/2019
 * Author: Shun Suzuki
 * -----
 * Last Modified: 21/07/2021
 * Modified By: Shun Suzuki (suzuki@hapis.k.u-tokyo.ac.jp)
 * -----
 * Copyright (c) 2019 Hapis Lab. All rights reserved.
 *
 */

use anyhow::Result;

use autd3_core::{
    ec_config::{
        BODY_SIZE, EC_INPUT_FRAME_SIZE, EC_OUTPUT_FRAME_SIZE, EC_SM3_CYCLE_TIME_NANO_SEC,
        EC_SYNC0_CYCLE_TIME_NANO_SEC, HEADER_SIZE,
    },
    error::AutdError,
    link::Link,
};
use autd3_timer::{Timer, TimerCallback};

use std::{
    sync::atomic::{AtomicBool, Ordering},
    usize,
    vec::Vec,
};

use libc::{c_char, c_void};

use crate::error::SoemError;
use crate::native_methods::*;

struct SoemCallback<F: Fn(&str) + Send> {
    lock: AtomicBool,
    expected_wkc: i32,
    error_handle: Option<F>,
}

impl<F: Fn(&str) + Send> TimerCallback for SoemCallback<F> {
    fn rt_thread(&mut self) {
        unsafe {
            if let Ok(false) =
                self.lock
                    .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            {
                ec_send_processdata();
                if self.expected_wkc != ec_receive_processdata(EC_TIMEOUTRET as i32)
                    && !self.error_handle()
                {
                    return;
                }

                self.lock.store(false, Ordering::Release);
            }
        }
    }
}

impl<F: Fn(&str) + Send> SoemCallback<F> {
    unsafe fn error_handle(&self) -> bool {
        ec_group[0].docheckstate = 0;
        ec_readstate();
        let mut msg = String::new();
        for (i, slave) in ec_slave
            .iter_mut()
            .enumerate()
            .take(ec_slavecount as usize + 1)
            .skip(1)
        {
            if slave.state != ec_state_EC_STATE_OPERATIONAL as _ {
                ec_group[0].docheckstate = 1;
                if slave.state == ec_state_EC_STATE_SAFE_OP as u16 + ec_state_EC_STATE_ERROR as u16
                {
                    msg.push_str(&format!(
                        "ERROR : slave {} is in SAFE_OP + ERROR, attempting ack\n",
                        i
                    ));
                    slave.state = ec_state_EC_STATE_SAFE_OP as u16 + ec_state_EC_STATE_ACK as u16;
                    ec_writestate(i as _);
                } else if slave.state == ec_state_EC_STATE_SAFE_OP as _ {
                    msg.push_str(&format!(
                        "ERROR : slave {} is in SAFE_OP, change to OPERATIONAL\n",
                        i
                    ));
                    slave.state = ec_state_EC_STATE_OPERATIONAL as _;
                    ec_writestate(i as _);
                } else if slave.state > ec_state_EC_STATE_NONE as _ {
                    if ec_reconfig_slave(i as _, 500) != 0 {
                        slave.islost = 0;
                        msg.push_str(&format!("MESSAGE : slave {} reconfigured\n", i));
                    }
                } else if slave.islost == 0 {
                    ec_statecheck(
                        i as _,
                        ec_state_EC_STATE_OPERATIONAL as _,
                        EC_TIMEOUTRET as _,
                    );
                    if slave.state == ec_state_EC_STATE_NONE as _ {
                        slave.islost = 1;
                        msg.push_str(&format!("ERROR : slave {} lost\n", i));
                    }
                }
            }
            if slave.islost != 0 {
                if slave.state == ec_state_EC_STATE_NONE as _ {
                    if ec_recover_slave(i as _, 500) != 0 {
                        slave.islost = 0;
                        msg.push_str(&format!("MESSAGE : slave {} recovered\n", i));
                    }
                } else {
                    slave.islost = 0;
                    msg.push_str(&format!("MESSAGE : slave {} found\n", i));
                }
            }
        }

        if ec_group[0].docheckstate == 0 {
            return true;
        }

        if let Some(f) = &self.error_handle {
            f(&msg);
        }

        false
    }
}

pub struct SoemLink<F: Fn(&str) + Send> {
    timer_handle: Option<Box<Timer<SoemCallback<F>>>>,
    error_handle: Option<F>,
    is_open: bool,
    ifname: std::ffi::CString,
    dev_num: u16,
    ec_sync0_cyctime_ns: u32,
    ec_sm2_cyctime_ns: u32,
    io_map: Vec<u8>,
}

impl<F: Fn(&str) + Send> SoemLink<F> {
    pub fn new(ifname: &str, dev_num: u16, cycle_ticks: u32, error_handle: F) -> Self {
        Self {
            dev_num,
            ec_sm2_cyctime_ns: EC_SM3_CYCLE_TIME_NANO_SEC * cycle_ticks,
            ec_sync0_cyctime_ns: EC_SYNC0_CYCLE_TIME_NANO_SEC * cycle_ticks,
            timer_handle: None,
            error_handle: Some(error_handle),
            is_open: false,
            ifname: std::ffi::CString::new(ifname.to_string()).unwrap(),
            io_map: vec![],
        }
    }

    unsafe fn setup_sync0(activate: u8, dev_num: u16, cycle_time: u32) {
        for slave in 1..=dev_num {
            ec_dcsync0(slave, activate, cycle_time, 0);
        }
    }

    unsafe fn write_header_body(
        src: &[u8],
        dst: *mut u8,
        dev_num: usize,
        header_size: usize,
        body_size: usize,
    ) {
        for i in 0..dev_num {
            std::ptr::copy_nonoverlapping(
                src.as_ptr().add(header_size + body_size * i),
                dst.add((header_size + body_size) * i),
                body_size,
            );
            std::ptr::copy_nonoverlapping(
                src.as_ptr(),
                dst.add((header_size + body_size) * i + body_size),
                header_size,
            );
        }
    }

    unsafe fn write_header(
        src: &[u8],
        dst: *mut u8,
        dev_num: usize,
        header_size: usize,
        body_size: usize,
    ) {
        for i in 0..dev_num {
            std::ptr::copy_nonoverlapping(
                src.as_ptr(),
                dst.add((header_size + body_size) * i + body_size),
                header_size,
            );
        }
    }
}

impl<F: Fn(&str) + Send> Link for SoemLink<F> {
    fn open(&mut self) -> Result<()> {
        let size = (EC_OUTPUT_FRAME_SIZE + EC_INPUT_FRAME_SIZE) * self.dev_num as usize;

        unsafe {
            if ec_init(self.ifname.as_ptr() as *const c_char) != 1 {
                return Err(SoemError::NoSocketConnection(
                    self.ifname.to_str().unwrap().to_string(),
                )
                .into());
            }

            self.io_map = vec![0x00; size];
            let wc = ec_config(0, self.io_map.as_mut_ptr() as *mut c_void) as u16;
            if wc != self.dev_num {
                return Err(SoemError::SlaveNotFound(wc, self.dev_num).into());
            }

            ec_configdc();
            ec_statecheck(
                0,
                ec_state_EC_STATE_SAFE_OP as u16,
                EC_TIMEOUTSTATE as i32 * 4,
            );

            ec_slave[0].state = ec_state_EC_STATE_OPERATIONAL as u16;
            ec_send_processdata();
            ec_receive_processdata(EC_TIMEOUTRET as i32);

            ec_writestate(0);

            let mut chk = 200;
            ec_statecheck(0, ec_state_EC_STATE_OPERATIONAL as u16, 50000);
            while chk > 0 && (ec_slave[0].state != ec_state_EC_STATE_OPERATIONAL as u16) {
                ec_statecheck(0, ec_state_EC_STATE_OPERATIONAL as u16, 50000);
                chk -= 1;
            }

            if ec_slave[0].state != ec_state_EC_STATE_OPERATIONAL as u16 {
                return Err(SoemError::NotResponding.into());
            }

            Self::setup_sync0(1, self.dev_num, self.ec_sync0_cyctime_ns);
        }

        self.is_open = true;
        let expected_wkc = unsafe { (ec_group[0].outputsWKC * 2 + ec_group[0].inputsWKC) as i32 };
        self.timer_handle = Some(Timer::start(
            SoemCallback {
                lock: AtomicBool::new(false),
                expected_wkc,
                error_handle: self.error_handle.take(),
            },
            self.ec_sm2_cyctime_ns,
        )?);

        Ok(())
    }

    fn close(&mut self) -> Result<()> {
        if !self.is_open {
            return Ok(());
        }
        self.is_open = false;

        unsafe {
            std::ptr::write_bytes(
                self.io_map.as_mut_ptr(),
                0x00,
                self.dev_num as usize * EC_OUTPUT_FRAME_SIZE,
            );
        }

        if let Some(timer) = self.timer_handle.take() {
            timer.close()?;
        }

        unsafe {
            Self::setup_sync0(0, self.dev_num, self.ec_sync0_cyctime_ns);

            ec_slave[0].state = ec_state_EC_STATE_INIT as u16;
            ec_writestate(0);
            ec_statecheck(0, ec_state_EC_STATE_INIT as u16, EC_TIMEOUTSTATE as i32);
            ec_close();
        }

        Ok(())
    }

    fn send(&mut self, data: &[u8]) -> Result<bool> {
        if !self.is_open {
            return Err(AutdError::LinkClosed.into());
        }

        unsafe {
            if data.len() > HEADER_SIZE {
                Self::write_header_body(
                    data,
                    self.io_map.as_mut_ptr(),
                    self.dev_num as usize,
                    HEADER_SIZE,
                    BODY_SIZE,
                );
            } else {
                Self::write_header(
                    data,
                    self.io_map.as_mut_ptr(),
                    self.dev_num as usize,
                    HEADER_SIZE,
                    BODY_SIZE,
                );
            }
        }

        Ok(true)
    }

    fn read(&mut self, data: &mut [u8]) -> Result<bool> {
        if !self.is_open {
            return Err(AutdError::LinkClosed.into());
        }

        unsafe {
            std::ptr::copy_nonoverlapping(
                self.io_map
                    .as_ptr()
                    .add(EC_OUTPUT_FRAME_SIZE * self.dev_num as usize),
                data.as_mut_ptr(),
                data.len(),
            );
        }
        Ok(true)
    }

    fn is_open(&self) -> bool {
        self.is_open
    }
}