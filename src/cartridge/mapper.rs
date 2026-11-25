//! Mapper state and register handling.

use serde::{Deserialize, Serialize};

use super::Mirroring;

/// Mappers implemented by NES Core. Support is listed in the README and is only
/// claimed for titles that have actually been exercised.
pub const SUPPORTED_MAPPERS: [u16; 6] = [0, 1, 2, 3, 4, 7];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mmc1State {
    pub shift: u8,
    pub count: u8,
    pub control: u8,
    pub chr_bank0: u8,
    pub chr_bank1: u8,
    pub prg_bank: u8,
}

impl Default for Mmc1State {
    fn default() -> Self {
        Self {
            shift: 0x10,
            count: 0,
            control: 0x0C,
            chr_bank0: 0,
            chr_bank1: 0,
            prg_bank: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mmc3State {
    pub bank_select: u8,
    pub banks: [u8; 8],
    pub mirroring: Mirroring,
    pub irq_latch: u8,
    pub irq_counter: u8,
    pub irq_reload: bool,
    pub irq_enabled: bool,
    pub irq_pending: bool,
    pub prg_mode: bool,
    pub chr_mode: bool,
}

impl Default for Mmc3State {
    fn default() -> Self {
        Self {
            bank_select: 0,
            banks: [0; 8],
            mirroring: Mirroring::Vertical,
            irq_latch: 0,
            irq_counter: 0,
            irq_reload: false,
            irq_enabled: false,
            irq_pending: false,
            prg_mode: false,
            chr_mode: false,
        }
    }
}

impl Mmc3State {
    pub fn clock_irq(&mut self) {
        if self.irq_counter == 0 || self.irq_reload {
            self.irq_counter = self.irq_latch;
            self.irq_reload = false;
        } else {
            self.irq_counter -= 1;
        }
        if self.irq_counter == 0 && self.irq_enabled {
            self.irq_pending = true;
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub enum MapperState {
    #[default]
    Nrom,
    Uxrom {
        bank: u8,
    },
    Axrom {
        bank: u8,
        upper: bool,
    },
    Cnrom {
        bank: u8,
    },
    Mmc1(Mmc1State),
    Mmc3(Mmc3State),
}

#[derive(Debug, Clone)]
pub enum Mapper {
    Nrom,
    Uxrom { bank: u8 },
    Axrom { bank: u8, upper: bool },
    Cnrom { bank: u8 },
    Mmc1(Mmc1State),
    Mmc3(Mmc3State),
}

impl Mapper {
    pub fn new(number: u16) -> Self {
        match number {
            1 => Mapper::Mmc1(Mmc1State::default()),
            2 => Mapper::Uxrom { bank: 0 },
            3 => Mapper::Cnrom { bank: 0 },
            4 => Mapper::Mmc3(Mmc3State::default()),
            7 => Mapper::Axrom {
                bank: 0,
                upper: false,
            },
            _ => Mapper::Nrom,
        }
    }

    pub fn number(&self) -> u16 {
        match self {
            Mapper::Nrom => 0,
            Mapper::Mmc1(_) => 1,
            Mapper::Uxrom { .. } => 2,
            Mapper::Cnrom { .. } => 3,
            Mapper::Mmc3(_) => 4,
            Mapper::Axrom { .. } => 7,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Mapper::Nrom => "NROM (0)",
            Mapper::Mmc1(_) => "MMC1 (1)",
            Mapper::Uxrom { .. } => "UxROM (2)",
            Mapper::Cnrom { .. } => "CNROM (3)",
            Mapper::Mmc3(_) => "MMC3 (4)",
            Mapper::Axrom { .. } => "AxROM (7)",
        }
    }

    pub fn write_prg(
        &mut self,
        addr: u16,
        value: u8,
        prg_banks_8k: usize,
        _chr_banks_1k: usize,
        _chr_is_ram: bool,
    ) {
        match self {
            Mapper::Nrom => {}
            Mapper::Uxrom { bank } => {
                let mask = (prg_banks_8k.saturating_mul(2).max(1) - 1) as u8;
                *bank = (value & 0x0F) & mask.min(0x0F);
            }
            Mapper::Axrom { bank, upper } => {
                let banks_32k = (prg_banks_8k / 4).max(1) as u8;
                *bank = (value & 0x07) % banks_32k;
                *upper = value & 0x10 != 0;
            }
            Mapper::Cnrom { bank } => {
                *bank = value & 0x03;
            }
            Mapper::Mmc1(state) => {
                if value & 0x80 != 0 {
                    state.shift = 0x10;
                    state.count = 0;
                    state.control |= 0x0C;
                    return;
                }
                state.shift |= (value & 1) << state.count;
                state.count += 1;
                if state.count == 5 {
                    let v = state.shift & 0x1F;
                    match (addr >> 13) & 0x03 {
                        0 => state.control = v,
                        1 => state.chr_bank0 = v,
                        2 => state.chr_bank1 = v,
                        _ => state.prg_bank = v,
                    }
                    state.shift = 0x10;
                    state.count = 0;
                }
            }
            Mapper::Mmc3(state) => {
                let even = addr & 1 == 0;
                match addr & 0xE000 {
                    0x8000 if even => {
                        state.bank_select = value;
                        state.prg_mode = value & 0x40 != 0;
                        state.chr_mode = value & 0x80 != 0;
                    }
                    0x8000 => {
                        let reg = usize::from(state.bank_select & 0x07);
                        state.banks[reg] = value;
                    }
                    0xA000 if even => {
                        state.mirroring = if value & 1 == 0 {
                            Mirroring::Vertical
                        } else {
                            Mirroring::Horizontal
                        };
                    }
                    0xA000 => {}
                    0xC000 if even => state.irq_latch = value,
                    0xC000 => state.irq_reload = true,
                    0xE000 if even => {
                        state.irq_enabled = false;
                        state.irq_pending = false;
                    }
                    0xE000 => state.irq_enabled = true,
                    _ => {}
                }
            }
        }
    }

    pub fn forced_mirroring(&self) -> Option<Mirroring> {
        match self {
            Mapper::Axrom { upper, .. } => Some(if *upper {
                Mirroring::OneScreenUpper
            } else {
                Mirroring::OneScreenLower
            }),
            Mapper::Mmc3(state) => Some(state.mirroring),
            Mapper::Mmc1(state) => Some(match state.control & 0x03 {
                0 => Mirroring::OneScreenLower,
                1 => Mirroring::OneScreenUpper,
                2 => Mirroring::Vertical,
                _ => Mirroring::Horizontal,
            }),
            _ => None,
        }
    }

    pub fn irq_pending(&self) -> bool {
        matches!(self, Mapper::Mmc3(state) if state.irq_pending)
    }

    pub fn prg_ram_disabled(&self) -> bool {
        false
    }

    pub fn state(&self) -> MapperState {
        match self {
            Mapper::Nrom => MapperState::Nrom,
            Mapper::Uxrom { bank } => MapperState::Uxrom { bank: *bank },
            Mapper::Axrom { bank, upper } => MapperState::Axrom {
                bank: *bank,
                upper: *upper,
            },
            Mapper::Cnrom { bank } => MapperState::Cnrom { bank: *bank },
            Mapper::Mmc1(state) => MapperState::Mmc1(state.clone()),
            Mapper::Mmc3(state) => MapperState::Mmc3(state.clone()),
        }
    }

    pub fn set_state(&mut self, state: MapperState) {
        match (self, state) {
            (Mapper::Uxrom { bank }, MapperState::Uxrom { bank: b }) => *bank = b,
            (Mapper::Axrom { bank, upper }, MapperState::Axrom { bank: b, upper: u }) => {
                *bank = b;
                *upper = u;
            }
            (Mapper::Cnrom { bank }, MapperState::Cnrom { bank: b }) => *bank = b,
            (Mapper::Mmc1(current), MapperState::Mmc1(saved)) => *current = saved,
            (Mapper::Mmc3(current), MapperState::Mmc3(saved)) => *current = saved,
            (Mapper::Nrom, MapperState::Nrom) => {}
            _ => {}
        }
    }
}
