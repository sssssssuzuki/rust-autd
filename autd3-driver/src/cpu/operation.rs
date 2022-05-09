/*
 * File: operation.rs
 * Project: cpu
 * Created Date: 02/05/2022
 * Author: Shun Suzuki
 * -----
 * Last Modified: 07/05/2022
 * Modified By: Shun Suzuki (suzuki@hapis.k.u-tokyo.ac.jp)
 * -----
 * Copyright (c) 2022 Hapis Lab. All rights reserved.
 *
 */

use crate::{
    cpu::{
        error::CPUError, CPUControlFlags, TxDatagram, MOD_BODY_DATA_SIZE, MOD_HEAD_DATA_SIZE,
        MSG_CLEAR, MSG_RD_CPU_VERSION, MSG_RD_FPGA_FUNCTION, MSG_RD_FPGA_VERSION,
    },
    fpga::{
        Duty, FPGAControlFlags, FPGAError, LegacyDrive, Phase, MOD_SAMPLING_FREQ_DIV_MIN,
        SILENCER_CYCLE_MIN,
    },
    hardware::NUM_TRANS_IN_UNIT,
    SeqFocus, POINT_STM_BODY_DATA_SIZE, POINT_STM_HEAD_DATA_SIZE, STM_SAMPLING_FREQ_DIV_MIN,
};

use anyhow::Result;

pub fn clear(tx: &mut TxDatagram) {
    tx.header_mut().msg_id = MSG_CLEAR;
    tx.num_bodies = 0;
}

pub fn sync(
    msg_id: u8,
    sync_cycle_ticks: u16,
    cycles: &[[u16; NUM_TRANS_IN_UNIT]],
    tx: &mut TxDatagram,
) -> Result<()> {
    if cycles.len() != tx.body().len() {
        return Err(CPUError::DeviceNumberNotCorrect {
            a: tx.body().len(),
            b: cycles.len(),
        }
        .into());
    }

    tx.header_mut().msg_id = msg_id;
    tx.header_mut().cpu_flag.set(CPUControlFlags::DO_SYNC, true);
    tx.header_mut().sync_header_mut().ecat_sync_cycle_ticks = sync_cycle_ticks;

    tx.body_mut()
        .iter_mut()
        .zip(cycles.iter())
        .for_each(|(d, s)| d.data.clone_from(s));

    tx.num_bodies = tx.body().len();

    Ok(())
}

pub fn modulation(
    msg_id: u8,
    mod_data: &[u8],
    is_first_frame: bool,
    freq_div: u32,
    is_last_frame: bool,
    tx: &mut TxDatagram,
) -> Result<()> {
    tx.header_mut().msg_id = msg_id;
    tx.header_mut().cpu_flag.remove(CPUControlFlags::DO_SYNC);
    tx.header_mut()
        .cpu_flag
        .remove(CPUControlFlags::CONFIG_SILENCER);

    if is_first_frame && mod_data.len() > MOD_HEAD_DATA_SIZE {
        return Err(CPUError::ModulationHeadDataSizeOutOfRange(mod_data.len()).into());
    }

    if !is_first_frame && mod_data.len() > MOD_BODY_DATA_SIZE {
        return Err(CPUError::ModulationBodyDataSizeOutOfRange(mod_data.len()).into());
    }

    if is_first_frame {
        if freq_div < MOD_SAMPLING_FREQ_DIV_MIN {
            return Err(FPGAError::ModFreqDivOutOfRange(freq_div).into());
        }

        tx.header_mut()
            .cpu_flag
            .set(CPUControlFlags::MOD_BEGIN, true);
        tx.header_mut().mod_head_mut().freq_div = freq_div;
        tx.header_mut().mod_head_mut().data[0..mod_data.len()].copy_from_slice(mod_data);
    } else {
        tx.header_mut().mod_body_mut().data[0..mod_data.len()].copy_from_slice(mod_data);
    }
    tx.header_mut().size = mod_data.len() as _;

    if is_last_frame {
        tx.header_mut().cpu_flag.set(CPUControlFlags::MOD_END, true);
    }

    Ok(())
}

pub fn config_silencer(msg_id: u8, cycle: u16, step: u16, tx: &mut TxDatagram) -> Result<()> {
    if cycle < SILENCER_CYCLE_MIN {
        return Err(FPGAError::SilencerCycleOutOfRange(cycle).into());
    }

    tx.header_mut().msg_id = msg_id;
    tx.header_mut().cpu_flag.remove(CPUControlFlags::DO_SYNC);
    tx.header_mut()
        .cpu_flag
        .set(CPUControlFlags::CONFIG_SILENCER, true);

    tx.header_mut().silencer_header_mut().cycle = cycle;
    tx.header_mut().silencer_header_mut().step = step;

    Ok(())
}

pub fn normal_legacy(msg_id: u8, drive: &[LegacyDrive], tx: &mut TxDatagram) -> Result<()> {
    if drive.len() / NUM_TRANS_IN_UNIT != tx.body().len() {
        return Err(CPUError::DeviceNumberNotCorrect {
            a: tx.body().len(),
            b: drive.len() / NUM_TRANS_IN_UNIT,
        }
        .into());
    }

    tx.header_mut().msg_id = msg_id;

    tx.header_mut()
        .fpga_flag
        .set(FPGAControlFlags::LEGACY_MODE, true);
    tx.header_mut().fpga_flag.remove(FPGAControlFlags::STM_MODE);

    tx.body_mut()
        .iter_mut()
        .zip(drive.chunks(NUM_TRANS_IN_UNIT))
        .for_each(|(d, s)| d.legacy_drives_mut().copy_from_slice(s));

    tx.num_bodies = tx.body().len();

    Ok(())
}

pub fn normal_duty(msg_id: u8, drive: &[Duty], tx: &mut TxDatagram) -> Result<()> {
    if drive.len() / NUM_TRANS_IN_UNIT != tx.body().len() {
        return Err(CPUError::DeviceNumberNotCorrect {
            a: tx.body().len(),
            b: drive.len() / NUM_TRANS_IN_UNIT,
        }
        .into());
    }

    tx.header_mut().msg_id = msg_id;

    tx.header_mut()
        .fpga_flag
        .remove(FPGAControlFlags::LEGACY_MODE);
    tx.header_mut().fpga_flag.remove(FPGAControlFlags::STM_MODE);

    tx.header_mut().cpu_flag.set(CPUControlFlags::IS_DUTY, true);

    tx.body_mut()
        .iter_mut()
        .zip(drive.chunks(NUM_TRANS_IN_UNIT))
        .for_each(|(d, s)| d.duties_mut().copy_from_slice(s));

    tx.num_bodies = tx.body().len();

    Ok(())
}

pub fn normal_phase(msg_id: u8, drive: &[Phase], tx: &mut TxDatagram) -> Result<()> {
    if drive.len() / NUM_TRANS_IN_UNIT != tx.body().len() {
        return Err(CPUError::DeviceNumberNotCorrect {
            a: tx.body().len(),
            b: drive.len() / NUM_TRANS_IN_UNIT,
        }
        .into());
    }

    tx.header_mut().msg_id = msg_id;

    tx.header_mut()
        .fpga_flag
        .remove(FPGAControlFlags::LEGACY_MODE);
    tx.header_mut().fpga_flag.remove(FPGAControlFlags::STM_MODE);

    tx.header_mut().cpu_flag.remove(CPUControlFlags::IS_DUTY);

    tx.body_mut()
        .iter_mut()
        .zip(drive.chunks(NUM_TRANS_IN_UNIT))
        .for_each(|(d, s)| d.phases_mut().copy_from_slice(s));

    tx.num_bodies = tx.body().len();

    Ok(())
}

pub fn point_stm(
    msg_id: u8,
    points: &[Vec<SeqFocus>],
    is_first_frame: bool,
    freq_div: u32,
    sound_speed: f64,
    is_last_frame: bool,
    tx: &mut TxDatagram,
) -> Result<()> {
    tx.header_mut().msg_id = msg_id;

    tx.header_mut()
        .fpga_flag
        .set(FPGAControlFlags::STM_MODE, true);
    tx.header_mut()
        .fpga_flag
        .remove(FPGAControlFlags::STM_GAIN_MODE);

    if is_first_frame {
        for s in points {
            if s.len() > POINT_STM_HEAD_DATA_SIZE {
                return Err(CPUError::PointSTMHeadDataSizeOutOfRange(s.len()).into());
            }
        }
    }

    if !is_first_frame {
        for s in points {
            if s.len() > POINT_STM_BODY_DATA_SIZE {
                return Err(CPUError::PointSTMBodyDataSizeOutOfRange(s.len()).into());
            }
        }
    }

    if is_first_frame {
        if freq_div < STM_SAMPLING_FREQ_DIV_MIN {
            return Err(FPGAError::STMFreqDivOutOfRange(freq_div).into());
        }
        tx.header_mut()
            .cpu_flag
            .set(CPUControlFlags::STM_BEGIN, true);
        let sound_speed = (sound_speed * 1024.0).round() as u32;
        tx.body_mut().iter_mut().zip(points).for_each(|(d, s)| {
            d.point_stm_head_mut().set_size(s.len() as _);
            d.point_stm_head_mut().set_freq_div(freq_div);
            d.point_stm_head_mut().set_sound_speed(sound_speed);
            d.point_stm_head_mut().set_points(s);
        });
    } else {
        tx.body_mut().iter_mut().zip(points).for_each(|(d, s)| {
            d.point_stm_body_mut().set_size(s.len() as _);
            d.point_stm_body_mut().set_points(s);
        });
    }

    if is_last_frame {
        tx.header_mut().cpu_flag.set(CPUControlFlags::STM_END, true);
    }

    tx.num_bodies = tx.body().len();

    Ok(())
}

pub fn gain_stm_legacy(
    msg_id: u8,
    gain: &[LegacyDrive],
    is_first_frame: bool,
    freq_div: u32,
    is_last_frame: bool,
    tx: &mut TxDatagram,
) -> Result<()> {
    tx.header_mut().msg_id = msg_id;

    tx.header_mut()
        .fpga_flag
        .set(FPGAControlFlags::LEGACY_MODE, true);
    tx.header_mut()
        .fpga_flag
        .set(FPGAControlFlags::STM_MODE, true);
    tx.header_mut()
        .fpga_flag
        .set(FPGAControlFlags::STM_GAIN_MODE, true);

    if is_first_frame {
        if freq_div < STM_SAMPLING_FREQ_DIV_MIN {
            return Err(FPGAError::STMFreqDivOutOfRange(freq_div).into());
        }
        tx.header_mut()
            .cpu_flag
            .set(CPUControlFlags::STM_BEGIN, true);
        tx.body_mut().iter_mut().for_each(|d| {
            d.gain_stm_head_mut().set_freq_div(freq_div);
        });
    } else {
        tx.body_mut()
            .iter_mut()
            .zip(gain.chunks(NUM_TRANS_IN_UNIT))
            .for_each(|(d, s)| {
                d.gain_stm_body_mut()
                    .legacy_drives_mut()
                    .clone_from_slice(s);
            });
    }

    if is_last_frame {
        tx.header_mut().cpu_flag.set(CPUControlFlags::STM_END, true);
    }

    tx.num_bodies = tx.body().len();

    Ok(())
}

pub fn gain_stm_normal_phase(
    msg_id: u8,
    phase: &[Phase],
    is_first_frame: bool,
    freq_div: u32,
    tx: &mut TxDatagram,
) -> Result<()> {
    tx.header_mut().msg_id = msg_id;

    tx.header_mut().cpu_flag.remove(CPUControlFlags::IS_DUTY);

    tx.header_mut()
        .fpga_flag
        .remove(FPGAControlFlags::LEGACY_MODE);
    tx.header_mut()
        .fpga_flag
        .set(FPGAControlFlags::STM_MODE, true);
    tx.header_mut()
        .fpga_flag
        .set(FPGAControlFlags::STM_GAIN_MODE, true);

    if is_first_frame {
        if freq_div < STM_SAMPLING_FREQ_DIV_MIN {
            return Err(FPGAError::STMFreqDivOutOfRange(freq_div).into());
        }
        tx.header_mut()
            .cpu_flag
            .set(CPUControlFlags::STM_BEGIN, true);
        tx.body_mut().iter_mut().for_each(|d| {
            d.gain_stm_head_mut().set_freq_div(freq_div);
        });
    } else {
        tx.body_mut()
            .iter_mut()
            .zip(phase.chunks(NUM_TRANS_IN_UNIT))
            .for_each(|(d, s)| {
                d.gain_stm_body_mut().phases_mut().clone_from_slice(s);
            });
    }

    tx.num_bodies = tx.body().len();

    Ok(())
}

pub fn gain_stm_normal_duty(
    msg_id: u8,
    duty: &[Duty],
    is_first_frame: bool,
    freq_div: u32,
    is_last_frame: bool,
    tx: &mut TxDatagram,
) -> Result<()> {
    tx.header_mut().msg_id = msg_id;

    tx.header_mut().cpu_flag.set(CPUControlFlags::IS_DUTY, true);

    tx.header_mut()
        .fpga_flag
        .remove(FPGAControlFlags::LEGACY_MODE);
    tx.header_mut()
        .fpga_flag
        .set(FPGAControlFlags::STM_MODE, true);
    tx.header_mut()
        .fpga_flag
        .set(FPGAControlFlags::STM_GAIN_MODE, true);

    if is_first_frame {
        if freq_div < STM_SAMPLING_FREQ_DIV_MIN {
            return Err(FPGAError::STMFreqDivOutOfRange(freq_div).into());
        }
        tx.header_mut()
            .cpu_flag
            .set(CPUControlFlags::STM_BEGIN, true);
        tx.body_mut().iter_mut().for_each(|d| {
            d.gain_stm_head_mut().set_freq_div(freq_div);
        });
    } else {
        tx.body_mut()
            .iter_mut()
            .zip(duty.chunks(NUM_TRANS_IN_UNIT))
            .for_each(|(d, s)| {
                d.gain_stm_body_mut().duties_mut().clone_from_slice(s);
            });
    }

    if is_last_frame {
        tx.header_mut().cpu_flag.set(CPUControlFlags::STM_END, true);
    }

    tx.num_bodies = tx.body().len();

    Ok(())
}

pub fn force_fan(tx: &mut TxDatagram, value: bool) {
    tx.header_mut()
        .fpga_flag
        .set(FPGAControlFlags::FORCE_FAN, value);
}

pub fn reads_fpga_info(tx: &mut TxDatagram, value: bool) {
    tx.header_mut()
        .cpu_flag
        .set(CPUControlFlags::READS_FPGA_INFO, value);
}

pub fn cpu_version(tx: &mut TxDatagram) {
    tx.header_mut().msg_id = MSG_RD_CPU_VERSION;
    tx.header_mut().cpu_flag = CPUControlFlags::from_bits(0x02).unwrap(); // For backward compatibility before 1.9
    tx.num_bodies = 0;
}

pub fn fpga_version(tx: &mut TxDatagram) {
    tx.header_mut().msg_id = MSG_RD_FPGA_VERSION;
    tx.header_mut().cpu_flag = CPUControlFlags::from_bits(0x04).unwrap(); // For backward compatibility before 1.9
    tx.num_bodies = 0;
}

pub fn fpga_functions(tx: &mut TxDatagram) {
    tx.header_mut().msg_id = MSG_RD_FPGA_FUNCTION;
    tx.header_mut().cpu_flag = CPUControlFlags::from_bits(0x05).unwrap(); // For backward compatibility before 1.9
    tx.num_bodies = 0;
}