//! Audio Processing Unit: two pulses, triangle, noise and DMC.
//!
//! Channels are clocked at CPU rate with the APU's half-rate timers, mixed with
//! the documented non-linear mixer, and downsampled to the output sample rate.

use serde::{Deserialize, Serialize};

pub const CPU_CLOCK_HZ: f64 = 1_789_773.0;
pub const DEFAULT_SAMPLE_RATE: u32 = 44_100;

pub(crate) const LENGTH_TABLE: [u8; 32] = [
    10, 254, 20, 2, 40, 4, 80, 6, 160, 8, 60, 10, 14, 12, 26, 14, 12, 16, 24, 18, 48, 20, 96, 22,
    192, 24, 72, 26, 16, 28, 32, 30,
];

pub(crate) const DUTY_TABLE: [[u8; 8]; 4] = [
    [0, 1, 0, 0, 0, 0, 0, 0],
    [0, 1, 1, 0, 0, 0, 0, 0],
    [0, 1, 1, 1, 1, 0, 0, 0],
    [1, 0, 0, 1, 1, 1, 1, 1],
];

const TRIANGLE_TABLE: [u8; 32] = [
    15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12,
    13, 14, 15,
];

const NOISE_PERIODS: [u16; 16] = [
    4, 8, 16, 32, 64, 96, 128, 160, 202, 254, 380, 508, 762, 1016, 2034, 4068,
];

const DMC_RATES: [u16; 16] = [
    428, 380, 340, 320, 286, 254, 226, 214, 190, 160, 142, 128, 106, 84, 72, 54,
];

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Envelope {
    pub start: bool,
    pub divider: u8,
    pub decay: u8,
    pub constant_volume: bool,
    pub volume: u8,
    pub loop_flag: bool,
}

impl Envelope {
    fn clock(&mut self) {
        if self.start {
            self.start = false;
            self.decay = 15;
            self.divider = self.volume;
        } else if self.divider == 0 {
            self.divider = self.volume;
            if self.decay > 0 {
                self.decay -= 1;
            } else if self.loop_flag {
                self.decay = 15;
            }
        } else {
            self.divider -= 1;
        }
    }

    fn output(&self) -> u8 {
        if self.constant_volume {
            self.volume
        } else {
            self.decay
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Pulse {
    pub enabled: bool,
    pub duty: u8,
    pub duty_pos: u8,
    pub timer_period: u16,
    pub timer: u16,
    pub length_counter: u8,
    pub length_halt: bool,
    pub envelope: Envelope,
    pub sweep_enabled: bool,
    pub sweep_period: u8,
    pub sweep_negate: bool,
    pub sweep_shift: u8,
    pub sweep_reload: bool,
    pub sweep_divider: u8,
    pub is_pulse2: bool,
}

impl Pulse {
    fn clock_timer(&mut self) {
        if self.timer == 0 {
            self.timer = self.timer_period;
            self.duty_pos = (self.duty_pos + 1) & 7;
        } else {
            self.timer -= 1;
        }
    }

    fn clock_length(&mut self) {
        if !self.length_halt && self.length_counter > 0 {
            self.length_counter -= 1;
        }
    }

    fn sweep_target(&self) -> u16 {
        let change = self.timer_period >> self.sweep_shift;
        if self.sweep_negate {
            self.timer_period
                .wrapping_sub(change + u16::from(!self.is_pulse2))
        } else {
            self.timer_period.wrapping_add(change)
        }
    }

    fn sweep_muted(&self) -> bool {
        self.timer_period < 8 || self.sweep_target() > 0x7FF
    }

    fn clock_sweep(&mut self) {
        if self.sweep_divider == 0
            && self.sweep_enabled
            && self.sweep_shift > 0
            && !self.sweep_muted()
        {
            let target = self.sweep_target();
            if target <= 0x7FF {
                self.timer_period = target;
            }
        }
        if self.sweep_divider == 0 || self.sweep_reload {
            self.sweep_divider = self.sweep_period;
            self.sweep_reload = false;
        } else {
            self.sweep_divider -= 1;
        }
    }

    fn output(&self) -> u8 {
        if !self.enabled
            || self.length_counter == 0
            || self.timer_period < 8
            || self.sweep_muted()
            || DUTY_TABLE[usize::from(self.duty & 3)][usize::from(self.duty_pos)] == 0
        {
            0
        } else {
            self.envelope.output()
        }
    }

    fn write(&mut self, reg: u16, value: u8) {
        match reg {
            0 => {
                self.duty = value >> 6;
                self.length_halt = value & 0x20 != 0;
                self.envelope.loop_flag = value & 0x20 != 0;
                self.envelope.constant_volume = value & 0x10 != 0;
                self.envelope.volume = value & 0x0F;
            }
            1 => {
                self.sweep_enabled = value & 0x80 != 0;
                self.sweep_period = (value >> 4) & 0x07;
                self.sweep_negate = value & 0x08 != 0;
                self.sweep_shift = value & 0x07;
                self.sweep_reload = true;
            }
            2 => {
                self.timer_period = (self.timer_period & 0x0700) | u16::from(value);
            }
            3 => {
                self.timer_period = (self.timer_period & 0x00FF) | (u16::from(value & 0x07) << 8);
                if self.enabled {
                    self.length_counter = LENGTH_TABLE[usize::from(value >> 3)];
                }
                self.duty_pos = 0;
                self.envelope.start = true;
            }
            _ => {}
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Triangle {
    pub enabled: bool,
    pub control: bool,
    pub linear_reload_value: u8,
    pub linear_reload: bool,
    pub linear_counter: u8,
    pub length_counter: u8,
    pub timer_period: u16,
    pub timer: u16,
    pub step: u8,
}

impl Triangle {
    fn clock_timer(&mut self) {
        if self.timer == 0 {
            self.timer = self.timer_period;
            if self.length_counter > 0 && self.linear_counter > 0 {
                self.step = (self.step + 1) & 31;
            }
        } else {
            self.timer -= 1;
        }
    }

    fn clock_linear(&mut self) {
        if self.linear_reload {
            self.linear_counter = self.linear_reload_value;
        } else if self.linear_counter > 0 {
            self.linear_counter -= 1;
        }
        if !self.control {
            self.linear_reload = false;
        }
    }

    fn clock_length(&mut self) {
        if !self.control && self.length_counter > 0 {
            self.length_counter -= 1;
        }
    }

    fn output(&self) -> u8 {
        if !self.enabled || self.length_counter == 0 || self.linear_counter == 0 {
            0
        } else {
            TRIANGLE_TABLE[usize::from(self.step)]
        }
    }

    fn write(&mut self, reg: u16, value: u8) {
        match reg {
            0 => {
                self.control = value & 0x80 != 0;
                self.linear_reload_value = value & 0x7F;
            }
            1 => {}
            2 => self.timer_period = (self.timer_period & 0x0700) | u16::from(value),
            3 => {
                self.timer_period = (self.timer_period & 0x00FF) | (u16::from(value & 0x07) << 8);
                if self.enabled {
                    self.length_counter = LENGTH_TABLE[usize::from(value >> 3)];
                }
                self.linear_reload = true;
            }
            _ => {}
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Noise {
    pub enabled: bool,
    pub mode: bool,
    pub period_index: u8,
    pub timer_period: u16,
    pub timer: u16,
    pub shift: u16,
    pub length_counter: u8,
    pub length_halt: bool,
    pub envelope: Envelope,
}

impl Noise {
    fn new() -> Self {
        Self {
            shift: 1,
            ..Default::default()
        }
    }

    fn clock_timer(&mut self) {
        if self.timer == 0 {
            self.timer = self.timer_period;
            let bit = if self.mode {
                (self.shift & 1) ^ ((self.shift >> 6) & 1)
            } else {
                (self.shift & 1) ^ ((self.shift >> 1) & 1)
            };
            self.shift = (self.shift >> 1) | (bit << 14);
        } else {
            self.timer -= 1;
        }
    }

    fn clock_length(&mut self) {
        if !self.length_halt && self.length_counter > 0 {
            self.length_counter -= 1;
        }
    }

    fn output(&self) -> u8 {
        if !self.enabled || self.length_counter == 0 || self.shift & 1 != 0 {
            0
        } else {
            self.envelope.output()
        }
    }

    fn write(&mut self, reg: u16, value: u8) {
        match reg {
            0 => {
                self.length_halt = value & 0x20 != 0;
                self.envelope.loop_flag = value & 0x20 != 0;
                self.envelope.constant_volume = value & 0x10 != 0;
                self.envelope.volume = value & 0x0F;
            }
            1 => {}
            2 => {
                self.mode = value & 0x80 != 0;
                self.period_index = value & 0x0F;
                self.timer_period = NOISE_PERIODS[usize::from(self.period_index)];
            }
            3 => {
                if self.enabled {
                    self.length_counter = LENGTH_TABLE[usize::from(value >> 3)];
                }
                self.envelope.start = true;
            }
            _ => {}
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dmc {
    pub enabled: bool,
    pub irq_enable: bool,
    pub loop_flag: bool,
    pub rate_index: u8,
    pub timer_period: u16,
    pub timer: u16,
    pub sample_address: u16,
    pub sample_length: u16,
    pub current_address: u16,
    pub bytes_remaining: u16,
    pub shift: u8,
    pub bits_remaining: u8,
    pub silence: bool,
    pub output_level: u8,
    pub irq_pending: bool,
    pub buffer: Option<u8>,
    #[serde(skip)]
    pub request: Option<u16>,
}

impl Default for Dmc {
    fn default() -> Self {
        Self {
            enabled: false,
            irq_enable: false,
            loop_flag: false,
            rate_index: 0,
            timer_period: DMC_RATES[0],
            timer: 0,
            sample_address: 0xC000,
            sample_length: 1,
            current_address: 0xC000,
            bytes_remaining: 0,
            shift: 0,
            bits_remaining: 8,
            silence: true,
            output_level: 0,
            irq_pending: false,
            buffer: None,
            request: None,
        }
    }
}

impl Dmc {
    fn clock_timer(&mut self) {
        if self.timer == 0 {
            self.timer = self.timer_period;
            if !self.silence {
                if self.shift & 1 != 0 {
                    self.output_level = (self.output_level + 2).min(127);
                } else {
                    self.output_level = self.output_level.saturating_sub(2);
                }
            }
            self.shift >>= 1;
            if self.bits_remaining == 0 {
                self.bits_remaining = 8;
                if let Some(sample) = self.buffer.take() {
                    self.shift = sample;
                    self.silence = false;
                } else {
                    self.silence = true;
                }
            }
            if self.bits_remaining > 0 {
                self.bits_remaining -= 1;
            }
            self.request_fill();
        } else {
            self.timer -= 1;
        }
    }

    fn request_fill(&mut self) {
        if self.buffer.is_none() && self.bytes_remaining > 0 {
            self.request = Some(self.current_address);
        }
    }

    /// Called by the bus when the CPU memory read for the DMC has completed.
    pub fn deliver_sample(&mut self, byte: u8) {
        self.buffer = Some(byte);
        self.bytes_remaining -= 1;
        self.current_address = if self.current_address == 0xFFFF {
            0x8000
        } else {
            self.current_address + 1
        };
        if self.bytes_remaining == 0 {
            if self.loop_flag {
                self.current_address = self.sample_address;
                self.bytes_remaining = self.sample_length;
            } else if self.irq_enable {
                self.irq_pending = true;
            }
        }
    }

    fn restart(&mut self) {
        self.current_address = self.sample_address;
        self.bytes_remaining = self.sample_length;
        self.request_fill();
    }

    fn write(&mut self, reg: u16, value: u8) {
        match reg {
            0 => {
                self.irq_enable = value & 0x80 != 0;
                self.loop_flag = value & 0x40 != 0;
                self.rate_index = value & 0x0F;
                self.timer_period = DMC_RATES[usize::from(self.rate_index)];
                if !self.irq_enable {
                    self.irq_pending = false;
                }
            }
            1 => self.output_level = value & 0x7F,
            2 => self.sample_address = 0xC000 | (u16::from(value) << 6),
            3 => {
                self.sample_length = (u16::from(value) << 4) | 1;
            }
            _ => {}
        }
    }

    fn output(&self) -> u8 {
        self.output_level
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApuState {
    pub pulse1: Pulse,
    pub pulse2: Pulse,
    pub triangle: Triangle,
    pub noise: Noise,
    pub dmc: Dmc,
    pub frame_cycle: u32,
    pub frame_mode: bool,
    pub frame_irq_inhibit: bool,
    pub frame_irq: bool,
    pub cycle: u64,
    #[serde(skip)]
    pub sample_accumulator: f64,
    #[serde(skip)]
    pub buffer: Vec<f32>,
}

pub struct Apu {
    pub state: ApuState,
    pub sample_rate: u32,
    pub volume: f32,
    pub muted: bool,
    pub last_mixed: f32,
    /// Cartridge expansion audio (MMC5 or VRC6) for the current CPU cycle.
    pub expansion: f32,
}

impl Apu {
    pub fn new(sample_rate: u32) -> Self {
        let mut noise = Noise::new();
        noise.timer_period = NOISE_PERIODS[0];
        // The noise channel starts with its LFSR seeded; Default gives zero.
        Self {
            state: ApuState {
                pulse1: Pulse::default(),
                pulse2: Pulse {
                    is_pulse2: true,
                    ..Default::default()
                },
                triangle: Triangle::default(),
                noise,
                dmc: Dmc::default(),
                frame_cycle: 0,
                frame_mode: false,
                frame_irq_inhibit: false,
                frame_irq: false,
                cycle: 0,
                sample_accumulator: 0.0,
                buffer: Vec::new(),
            },
            sample_rate,
            volume: 0.7,
            muted: false,
            last_mixed: 0.0,
            expansion: 0.0,
        }
    }

    pub fn reset(&mut self) {
        let sample_rate = self.sample_rate;
        *self = Apu::new(sample_rate);
    }

    pub fn read_status(&mut self) -> u8 {
        let mut value = 0u8;
        if self.state.pulse1.length_counter > 0 {
            value |= 0x01;
        }
        if self.state.pulse2.length_counter > 0 {
            value |= 0x02;
        }
        if self.state.triangle.length_counter > 0 {
            value |= 0x04;
        }
        if self.state.noise.length_counter > 0 {
            value |= 0x08;
        }
        if self.state.dmc.bytes_remaining > 0 {
            value |= 0x10;
        }
        if self.state.frame_irq {
            value |= 0x40;
        }
        if self.state.dmc.irq_pending {
            value |= 0x80;
        }
        self.state.frame_irq = false;
        value
    }

    pub fn write_register(&mut self, addr: u16, value: u8) {
        match addr {
            0x4000 => self.state.pulse1.write(0, value),
            0x4001 => self.state.pulse1.write(1, value),
            0x4002 => self.state.pulse1.write(2, value),
            0x4003 => self.state.pulse1.write(3, value),
            0x4004 => self.state.pulse2.write(0, value),
            0x4005 => self.state.pulse2.write(1, value),
            0x4006 => self.state.pulse2.write(2, value),
            0x4007 => self.state.pulse2.write(3, value),
            0x4008 => self.state.triangle.write(0, value),
            0x400A => self.state.triangle.write(2, value),
            0x400B => self.state.triangle.write(3, value),
            0x400C => self.state.noise.write(0, value),
            0x400E => self.state.noise.write(2, value),
            0x400F => self.state.noise.write(3, value),
            0x4010 => self.state.dmc.write(0, value),
            0x4011 => self.state.dmc.write(1, value),
            0x4012 => self.state.dmc.write(2, value),
            0x4013 => self.state.dmc.write(3, value),
            0x4015 => {
                let p1 = value & 0x01 != 0;
                let p2 = value & 0x02 != 0;
                let tri = value & 0x04 != 0;
                let noise = value & 0x08 != 0;
                let dmc = value & 0x10 != 0;
                self.state.pulse1.enabled = p1;
                if !p1 {
                    self.state.pulse1.length_counter = 0;
                }
                self.state.pulse2.enabled = p2;
                if !p2 {
                    self.state.pulse2.length_counter = 0;
                }
                self.state.triangle.enabled = tri;
                if !tri {
                    self.state.triangle.length_counter = 0;
                }
                self.state.noise.enabled = noise;
                if !noise {
                    self.state.noise.length_counter = 0;
                }
                let was_enabled = self.state.dmc.enabled;
                self.state.dmc.enabled = dmc;
                if !dmc {
                    self.state.dmc.bytes_remaining = 0;
                } else if !was_enabled {
                    self.state.dmc.restart();
                }
                self.state.dmc.irq_pending = false;
            }
            0x4017 => {
                self.state.frame_mode = value & 0x80 != 0;
                self.state.frame_irq_inhibit = value & 0x40 != 0;
                if self.state.frame_irq_inhibit {
                    self.state.frame_irq = false;
                }
                self.state.frame_cycle = 0;
                if self.state.frame_mode {
                    self.clock_quarter_frame();
                    self.clock_half_frame();
                }
            }
            _ => {}
        }
    }

    pub fn irq_pending(&self) -> bool {
        self.state.frame_irq || self.state.dmc.irq_pending
    }

    pub fn take_dmc_request(&mut self) -> Option<u16> {
        self.state.dmc.request.take()
    }

    pub fn deliver_dmc_sample(&mut self, byte: u8) {
        self.state.dmc.deliver_sample(byte);
    }

    fn clock_quarter_frame(&mut self) {
        self.state.pulse1.envelope.clock();
        self.state.pulse2.envelope.clock();
        self.state.noise.envelope.clock();
        self.state.triangle.clock_linear();
    }

    fn clock_half_frame(&mut self) {
        self.state.pulse1.clock_length();
        self.state.pulse2.clock_length();
        self.state.triangle.clock_length();
        self.state.noise.clock_length();
        self.state.pulse1.clock_sweep();
        self.state.pulse2.clock_sweep();
    }

    fn clock_frame_sequencer(&mut self) {
        self.state.frame_cycle += 1;
        let cycle = self.state.frame_cycle;
        if !self.state.frame_mode {
            // 4-step sequence (~29830 CPU cycles per step group).
            match cycle {
                7457 | 22371 => self.clock_quarter_frame(),
                14913 => self.clock_half_frame(),
                29829 => {
                    self.clock_quarter_frame();
                    self.clock_half_frame();
                    if !self.state.frame_irq_inhibit {
                        self.state.frame_irq = true;
                    }
                }
                29830 => self.state.frame_cycle = 0,
                _ => {}
            }
        } else {
            match cycle {
                7457 | 22371 => self.clock_quarter_frame(),
                14913 => self.clock_half_frame(),
                29829 => self.clock_quarter_frame(),
                37281 => {
                    self.clock_quarter_frame();
                    self.clock_half_frame();
                    self.state.frame_cycle = 0;
                }
                _ => {}
            }
        }
    }

    fn mix(&self) -> f32 {
        let p1 = f32::from(self.state.pulse1.output());
        let p2 = f32::from(self.state.pulse2.output());
        let triangle = f32::from(self.state.triangle.output());
        let noise = f32::from(self.state.noise.output());
        let dmc = f32::from(self.state.dmc.output());

        let pulse_sum = p1 + p2;
        let pulse_out = if pulse_sum > 0.0 {
            95.88 / (8128.0 / pulse_sum + 100.0)
        } else {
            0.0
        };
        let tnd_sum = triangle / 8227.0 + noise / 12241.0 + dmc / 22638.0;
        let tnd_out = if tnd_sum > 0.0 {
            159.79 / (1.0 / tnd_sum + 100.0)
        } else {
            0.0
        };
        (pulse_out + tnd_out + self.expansion).clamp(0.0, 1.0)
    }

    /// Advances one CPU cycle.
    pub fn step(&mut self) {
        self.state.cycle = self.state.cycle.wrapping_add(1);

        if self.state.cycle & 1 == 0 {
            self.state.pulse1.clock_timer();
            self.state.pulse2.clock_timer();
            self.state.triangle.clock_timer();
            self.state.noise.clock_timer();
        }

        self.state.dmc.clock_timer();

        self.clock_frame_sequencer();

        self.last_mixed = self.mix();
        self.state.sample_accumulator += f64::from(self.sample_rate) / CPU_CLOCK_HZ;
        while self.state.sample_accumulator >= 1.0 {
            self.state.sample_accumulator -= 1.0;
            let sample = (self.last_mixed * 2.0 - 0.35).clamp(-1.0, 1.0);
            let sample = if self.muted {
                0.0
            } else {
                sample * self.volume
            };
            self.state.buffer.push(sample);
        }

        if self.state.buffer.len() > self.sample_rate as usize * 2 {
            // Never let an unread queue grow without bound.
            self.state.buffer.drain(..self.sample_rate as usize);
        }
    }

    pub fn drain_samples(&mut self, out: &mut Vec<f32>) {
        out.append(&mut self.state.buffer);
    }

    pub fn save_state(&self) -> ApuState {
        let mut state = self.state.clone();
        state.buffer.clear();
        state.dmc.request = None;
        state
    }

    pub fn load_state(&mut self, state: &ApuState) {
        self.state = state.clone();
        self.state.buffer.clear();
        self.state.dmc.request = None;
        self.state.dmc.buffer = None;
    }
}
