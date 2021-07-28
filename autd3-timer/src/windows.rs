/*
 * File: windows.rs
 * Project: src
 * Created Date: 24/05/2021
 * Author: Shun Suzuki
 * -----
 * Last Modified: 29/05/2021
 * Modified By: Shun Suzuki (suzuki@hapis.k.u-tokyo.ac.jp)
 * -----
 * Copyright (c) 2021 Hapis Lab. All rights reserved.
 *
 */

mod bindings {
    windows::include_bindings!();
}

use bindings::{
    Windows::Win32::Media::Multimedia::timeBeginPeriod,
    Windows::Win32::Media::Multimedia::timeEndPeriod,
    Windows::Win32::Media::Multimedia::TIMERR_NOERROR,
    Windows::Win32::System::SystemServices::timeKillEvent,
    Windows::Win32::System::SystemServices::timeSetEvent,
    Windows::Win32::System::SystemServices::LPTIMECALLBACK,
    Windows::Win32::System::SystemServices::TIME_CALLBACK_FUNCTION,
    Windows::Win32::System::SystemServices::TIME_KILL_SYNCHRONOUS,
    Windows::Win32::System::SystemServices::TIME_PERIODIC,
    Windows::Win32::System::Threading::GetCurrentProcess,
    Windows::Win32::System::Threading::SetPriorityClass,
    Windows::Win32::System::Threading::REALTIME_PRIORITY_CLASS,
};

use crate::error::TimerError;

pub struct NativeTimerWrapper {
    timer_id: u32,
}

impl NativeTimerWrapper {
    pub fn new() -> NativeTimerWrapper {
        NativeTimerWrapper { timer_id: 0 }
    }

    pub fn start<P>(
        &mut self,
        cb: LPTIMECALLBACK,
        period_ns: u32,
        lp_param: *mut P,
    ) -> Result<bool, TimerError> {
        unsafe {
            let h_process = GetCurrentProcess();
            SetPriorityClass(h_process, REALTIME_PRIORITY_CLASS);

            let u_resolution = 1;
            timeBeginPeriod(u_resolution);

            let timer_id = timeSetEvent(
                period_ns / 1000 / 1000,
                u_resolution,
                Some(cb),
                lp_param as usize,
                TIME_PERIODIC | TIME_CALLBACK_FUNCTION | TIME_KILL_SYNCHRONOUS,
            );

            if timer_id == 0 {
                return Err(TimerError::CreationFailed());
            }

            self.timer_id = timer_id;
            Ok(true)
        }
    }

    pub fn close(&mut self) -> Result<(), TimerError> {
        unsafe {
            if self.timer_id != 0 && timeKillEvent(self.timer_id) != TIMERR_NOERROR {
                return Err(TimerError::DeleteFailed());
            }
            timeEndPeriod(1);
        }
        Ok(())
    }
}

impl Drop for NativeTimerWrapper {
    fn drop(&mut self) {
        let _ = self.close();
    }
}