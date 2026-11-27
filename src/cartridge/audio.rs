//! Expansion audio for MMC5 and VRC6.
//!
//! Both chips are clocked once per CPU cycle by the bus. The output is a
//! normalised level (0..1) that the APU adds to its own mixer; pulse channels
//! use the same non-linear DAC formula as the 2A03 so their relative volume
//! matches the hardware description, and the PCM/sawtooth outputs are added
//! linearly as documented for those DACs.

use serde::{Deserialize, Serialize};

use crate::apu::{DUTY_TABLE, LENGTH_TABLE};

/// MMC5 envelope/length clock: M2 divided by 7424, about 241 Hz.
const MMC5_FRAME_PERIOD: u32 = 7424;

/// One of the MMC5's two pulse channels, which are 2A03 pulses without sweeps.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Mmc5Pulse {
    pub enabled: bool,
    pub duty: u8,
    pub duty_pos: u8,
    pub timer_period: u16,
    pub timer: u16,
    pub length_counter: u8,
    pub length_halt: bool,
    pub constant_volume: bool,
    pub volume: u8,
    pub decay: u8,
    pub divider: u8,
    pub start: bool,
}

impl Mmc5Pulse {
    fn write_control(&mut self, value: u8) {
        self.duty = value >> 6;
        self.length_halt = value & 0x20 != 0;
        self.constant_volume = value & 0x10 != 0;
        self.volume = value & 0x0F;
    }

    fn write_timer_high(&mut self, value: u8) {
        self.timer_period = (self.timer_period & 0x00FF) | (u16::from(value & 0x07) << 8);
        if self.enabled {
            self.length_counter = LENGTH_TABLE[usize::from(value >> 3)];
        }
        self.duty_pos = 0;
        self.start = true;
    }

    fn clock_timer(&mut self) {
        if self.timer == 0 {
            self.timer = self.timer_period;
            self.duty_pos = (self.duty_pos + 1) & 7;
        } else {
            self.timer -= 1;
        }
    }

    fn clock_envelope(&mut self) {
        if self.start {
            self.start = false;
            self.decay = 15;
            self.divider = self.volume;
        } else if self.divider == 0 {
            self.divider = self.volume;
            if self.decay > 0 {
                self.decay -= 1;
            } else if self.length_halt {
                self.decay = 15;
            }
        } else {
            self.divider -= 1;
        }
    }

    fn clock_length(&mut self) {
        if !self.length_halt && self.length_counter > 0 {
            self.length_counter -= 1;
        }
    }

    fn output(&self) -> u8 {
        if !self.enabled
            || self.length_counter == 0
            || DUTY_TABLE[usize::from(self.duty & 3)][usize::from(self.duty_pos)] == 0
        {
            0
        } else if self.constant_volume {
            self.volume
        } else {
            self.decay
        }
    }
}

/// MMC5 sound: two pulses plus the 8-bit raw PCM channel of `$5011`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Mmc5Audio {
    pub pulse_a: Mmc5Pulse,
    pub pulse_b: Mmc5Pulse,
    /// `$5015`: bit 0 pulse A, bit 1 pulse B, bit 2 PCM.
    pub enable: u8,
    pub pcm: u8,
    pub pcm_mode_read: bool,
    pub pcm_irq_enable: bool,
    pub pcm_irq_trip: bool,
    pub phase: bool,
    pub frame_counter: u32,
}

impl Mmc5Audio {
    pub fn write(&mut self, addr: u16, value: u8) {
        match addr {
            0x5000 => self.pulse_a.write_control(value),
            // `$5001` has no sweep unit on the MMC5 and is not implemented.
            0x5001 => {}
            0x5002 => {
                self.pulse_a.timer_period = (self.pulse_a.timer_period & 0x0700) | u16::from(value)
            }
            0x5003 => self.pulse_a.write_timer_high(value),
            0x5004 => self.pulse_b.write_control(value),
            0x5005 => {}
            0x5006 => {
                self.pulse_b.timer_period = (self.pulse_b.timer_period & 0x0700) | u16::from(value)
            }
            0x5007 => self.pulse_b.write_timer_high(value),
            0x5010 => {
                self.pcm_irq_enable = value & 0x80 != 0;
                self.pcm_mode_read = value & 0x01 != 0;
            }
            0x5011 => {
                if !self.pcm_mode_read {
                    if value == 0 {
                        self.pcm_irq_trip = true;
                    } else {
                        self.pcm = value;
                        self.pcm_irq_trip = false;
                    }
                }
            }
            0x5015 => {
                self.enable = value & 0x07;
                self.pulse_a.enabled = value & 0x01 != 0;
                if !self.pulse_a.enabled {
                    self.pulse_a.length_counter = 0;
                }
                self.pulse_b.enabled = value & 0x02 != 0;
                if !self.pulse_b.enabled {
                    self.pulse_b.length_counter = 0;
                }
            }
            _ => {}
        }
    }

    pub fn read(&mut self, addr: u16) -> u8 {
        match addr {
            0x5010 => {
                let value = u8::from(self.pcm_irq_enable && self.pcm_irq_trip) << 7;
                self.pcm_irq_trip = false;
                value
            }
            0x5015 => {
                u8::from(self.pulse_a.length_counter > 0)
                    | (u8::from(self.pulse_b.length_counter > 0) << 1)
            }
            _ => 0,
        }
    }

    pub fn irq_pending(&self) -> bool {
        self.pcm_irq_enable && self.pcm_irq_trip
    }

    /// Advances one CPU cycle.
    pub fn clock(&mut self) {
        self.phase = !self.phase;
        if !self.phase {
            self.pulse_a.clock_timer();
            self.pulse_b.clock_timer();
        }
        self.frame_counter += 1;
        if self.frame_counter >= MMC5_FRAME_PERIOD {
            self.frame_counter = 0;
            // Length counters run at twice the APU rate, i.e. this same clock.
            for pulse in [&mut self.pulse_a, &mut self.pulse_b] {
                pulse.clock_envelope();
                pulse.clock_length();
            }
        }
    }

    pub fn output(&self) -> f32 {
        let a = if self.enable & 0x01 != 0 {
            f32::from(self.pulse_a.output())
        } else {
            0.0
        };
        let b = if self.enable & 0x02 != 0 {
            f32::from(self.pulse_b.output())
        } else {
            0.0
        };
        let sum = a + b;
        let pulses = if sum > 0.0 {
            95.88 / (8128.0 / sum + 100.0)
        } else {
            0.0
        };
        // The PCM DAC is AC-coupled in hardware; centre it to avoid a DC step.
        let pcm = if self.enable & 0x04 != 0 {
            (f32::from(self.pcm) - 128.0) / 255.0 * 0.5
        } else {
            0.0
        };
        pulses + pcm
    }
}

/// One VRC6 pulse channel: a 12-bit divider driving a 16-step duty generator.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Vrc6Pulse {
    pub enabled: bool,
    pub ignore_duty: bool,
    pub duty: u8,
    pub volume: u8,
    pub timer_period: u16,
    pub timer: u16,
    pub step: u8,
}

impl Vrc6Pulse {
    fn clock(&mut self, period: u16) {
        if self.timer == 0 {
            self.timer = period;
            self.step = if self.step == 0 { 15 } else { self.step - 1 };
        } else {
            self.timer -= 1;
        }
    }

    fn output(&self) -> u8 {
        if !self.enabled {
            0
        } else if self.ignore_duty || self.step <= self.duty {
            self.volume
        } else {
            0
        }
    }
}

/// VRC6 sawtooth: an 8-bit accumulator stepped by the accumulator rate.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Vrc6Saw {
    pub enabled: bool,
    pub rate: u8,
    pub timer_period: u16,
    pub timer: u16,
    pub accumulator: u8,
    pub step: u8,
    pub phase: bool,
}

impl Vrc6Saw {
    fn clock(&mut self, period: u16) {
        if self.timer == 0 {
            self.timer = period;
            // The accumulator only reacts on every second divider expiry, which
            // is what makes the saw divide by 14 rather than 16.
            self.phase = !self.phase;
            if self.phase {
                if self.step == 6 {
                    self.accumulator = 0;
                    self.step = 0;
                } else {
                    self.accumulator = self.accumulator.wrapping_add(self.rate);
                    self.step += 1;
                }
            }
        } else {
            self.timer -= 1;
        }
    }

    fn output(&self) -> u8 {
        if self.enabled {
            self.accumulator >> 3
        } else {
            0
        }
    }
}

/// VRC6 expansion audio: two pulses and a sawtooth, plus `$9003` scaling.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Vrc6Audio {
    pub pulse1: Vrc6Pulse,
    pub pulse2: Vrc6Pulse,
    pub saw: Vrc6Saw,
    /// `$9003` controls the shared frequency scaling and the halt flag.
    pub frequency_control: u8,
}

impl Vrc6Audio {
    /// Writes one of the three audio register groups; `reg` is already
    /// normalised for the VRC6b address swap, and `group` is `$9000`, `$A000`
    /// or `$B000`.
    pub fn write(&mut self, group: u16, reg: usize, value: u8) {
        match group {
            0x9000 if reg >= 3 => self.frequency_control = value & 0x07,
            0x9000 => self.write_pulse(false, reg, value),
            0xA000 => self.write_pulse(true, reg, value),
            0xB000 => match reg {
                0 => self.saw.rate = value & 0x3F,
                1 => self.saw.timer_period = (self.saw.timer_period & 0x0F00) | u16::from(value),
                2 => {
                    self.saw.timer_period =
                        (self.saw.timer_period & 0x00FF) | (u16::from(value & 0x0F) << 8);
                    self.saw.enabled = value & 0x80 != 0;
                    if !self.saw.enabled {
                        self.saw.accumulator = 0;
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }

    fn write_pulse(&mut self, second: bool, reg: usize, value: u8) {
        let pulse = if second {
            &mut self.pulse2
        } else {
            &mut self.pulse1
        };
        match reg {
            0 => {
                pulse.ignore_duty = value & 0x80 != 0;
                pulse.duty = (value >> 4) & 0x07;
                pulse.volume = value & 0x0F;
            }
            1 => pulse.timer_period = (pulse.timer_period & 0x0F00) | u16::from(value),
            2 => {
                pulse.timer_period = (pulse.timer_period & 0x00FF) | (u16::from(value & 0x0F) << 8);
                pulse.enabled = value & 0x80 != 0;
                if !pulse.enabled {
                    pulse.step = 0;
                }
            }
            _ => {}
        }
    }

    /// `$9003`: bit 0 halts the oscillators, bit 1 is 16x, bit 2 is 256x.
    fn scaled(&self, period: u16) -> u16 {
        if self.frequency_control & 0x04 != 0 {
            period >> 8
        } else if self.frequency_control & 0x02 != 0 {
            period >> 4
        } else {
            period
        }
    }

    /// Advances one CPU cycle.
    pub fn clock(&mut self) {
        if self.frequency_control & 0x01 != 0 {
            return;
        }
        let pulse1 = self.scaled(self.pulse1.timer_period);
        let pulse2 = self.scaled(self.pulse2.timer_period);
        self.pulse1.clock(pulse1);
        self.pulse2.clock(pulse2);
        let saw = self.scaled(self.saw.timer_period);
        self.saw.clock(saw);
    }

    pub fn output(&self) -> f32 {
        let level = u16::from(self.pulse1.output())
            + u16::from(self.pulse2.output())
            + u16::from(self.saw.output());
        f32::from(level) / 61.0 * 0.4
    }
}
