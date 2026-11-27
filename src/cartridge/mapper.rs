//! Mapper state and register handling.

use serde::{Deserialize, Serialize};

use super::audio::{Mmc5Audio, Vrc6Audio};
use super::Mirroring;

/// Mappers implemented by NES Core. Support is listed in the README and is only
/// claimed for titles that have actually been exercised.
pub const SUPPORTED_MAPPERS: [u16; 9] = [0, 1, 2, 3, 4, 5, 7, 24, 26];

/// MMC5 keeps 1 KB of internal extended RAM mirrored at `$5C00-$5FFF`.
pub const MMC5_EXRAM_LEN: usize = 1024;

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

/// MMC5 configuration and bank registers. The 8x8 multiplier, scanline counter
/// and 1 KB extended RAM live here too so save states capture all of them.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mmc5State {
    pub prg_mode: u8,
    pub chr_mode: u8,
    pub prg_ram_protect: u8,
    pub exram_mode: u8,
    pub nametable_map: u8,
    pub fill_tile: u8,
    pub fill_color: u8,
    pub prg_ram_bank: u8,
    pub prg_banks: [u8; 4],
    pub chr_banks: [u16; 12],
    pub upper_chr: u8,
    pub split_control: u8,
    pub split_scroll: u8,
    pub split_bank: u8,
    pub irq_compare: u8,
    pub irq_enabled: bool,
    pub irq_pending: bool,
    pub multiplier_a: u8,
    pub multiplier_b: u8,
    pub scanline: u8,
    pub in_frame: bool,
    pub exram: Vec<u8>,
    pub audio: Mmc5Audio,
}

impl Default for Mmc5State {
    fn default() -> Self {
        Self {
            prg_mode: 3,
            chr_mode: 3,
            prg_ram_protect: 0,
            exram_mode: 0,
            nametable_map: 0x44,
            fill_tile: 0,
            fill_color: 0,
            prg_ram_bank: 0,
            prg_banks: [0x80, 0x80, 0x80, 0x80],
            chr_banks: [0; 12],
            upper_chr: 0,
            split_control: 0,
            split_scroll: 0,
            split_bank: 0,
            irq_compare: 0,
            irq_enabled: false,
            irq_pending: false,
            multiplier_a: 0,
            multiplier_b: 0,
            scanline: 0,
            in_frame: false,
            exram: vec![0; MMC5_EXRAM_LEN],
            audio: Mmc5Audio::default(),
        }
    }
}

impl Mmc5State {
    pub fn product(&self) -> u16 {
        u16::from(self.multiplier_a) * u16::from(self.multiplier_b)
    }

    pub fn clock_scanline(&mut self) {
        self.scanline = self.scanline.wrapping_add(1);
        self.in_frame = self.scanline < 240;
        if self.irq_enabled && self.irq_compare != 0 && self.scanline == self.irq_compare {
            self.irq_pending = true;
        }
    }
}

/// Background substitution data for one visible scanline. The PPU copies this
/// out of the cartridge so it can render without holding a borrow.
#[derive(Debug, Clone)]
pub struct Mmc5Render {
    pub split_enabled: bool,
    pub split_left: bool,
    pub split_threshold: usize,
    pub split_scroll: u8,
    pub split_bank: u8,
    pub extended_attributes: bool,
    pub upper_chr: u8,
    pub exram: [u8; MMC5_EXRAM_LEN],
}

impl Mmc5Render {
    fn from_state(state: &Mmc5State) -> Self {
        let mut exram = [0u8; MMC5_EXRAM_LEN];
        let len = state.exram.len().min(MMC5_EXRAM_LEN);
        exram[..len].copy_from_slice(&state.exram[..len]);
        Self {
            // RAM modes %10 and %11 disable split screen.
            split_enabled: state.split_control & 0x80 != 0 && state.exram_mode & 0x02 == 0,
            split_left: state.split_control & 0x40 == 0,
            split_threshold: usize::from(state.split_control & 0x3F),
            split_scroll: state.split_scroll,
            split_bank: state.split_bank,
            extended_attributes: state.exram_mode & 0x03 == 0x01,
            upper_chr: state.upper_chr,
            exram,
        }
    }

    /// Whether a screen tile column falls inside the split region.
    pub fn split_region(&self, screen_tile: usize) -> bool {
        if !self.split_enabled {
            return false;
        }
        if self.split_left {
            screen_tile < self.split_threshold
        } else {
            screen_tile >= self.split_threshold
        }
    }

    fn split_row(&self, scanline: usize) -> usize {
        (usize::from(self.split_scroll) + scanline) % 240
    }

    /// Tile byte and palette for the split region; the split nametable is ExRAM
    /// with the usual 960 bytes of tiles followed by 64 bytes of attributes.
    pub fn split_tile(&self, scanline: usize, column: usize) -> (u8, u8) {
        let row = self.split_row(scanline);
        let column = column.min(31);
        let tile = self.exram[(row / 8) * 32 + column];
        let attribute = self.exram[960 + (row / 4).min(7) * 8 + column / 4];
        let shift = ((row % 4) / 2) * 4 + ((column % 4) / 2) * 2;
        (tile, (attribute >> shift) & 0x03)
    }

    /// Row inside the tile, taken from the split counter rather than PPU fine Y.
    pub fn split_fine_row(&self, scanline: usize) -> usize {
        self.split_row(scanline) & 7
    }

    /// Split region CHR uses one 4 KB bank from `$5202`.
    pub fn split_pattern_base(&self, tile: u8) -> u16 {
        u16::from(self.split_bank) * 0x1000 + u16::from(tile) * 16
    }

    /// Extended attribute palette and 4 KB CHR bank, 1-screen mirrored.
    pub fn extended_attribute(&self, coarse_y: usize, tile_x: usize) -> (u8, u16) {
        let attribute = self.exram[coarse_y * 32 + tile_x];
        let bank = u16::from(attribute & 0x3F) | (u16::from(self.upper_chr & 0x03) << 6);
        (attribute >> 6, bank)
    }
}

/// VRC6 registers shared by the VRC6a (mapper 24) and VRC6b (mapper 26) boards.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vrc6State {
    /// VRC6b (mapper 26) swaps address lines A0 and A1.
    pub variant_b: bool,
    pub prg_16k: u8,
    pub prg_8k: u8,
    pub banking: u8,
    pub chr: [u8; 8],
    pub irq_latch: u8,
    pub irq_counter: u8,
    pub irq_mode_cycle: bool,
    pub irq_enabled: bool,
    pub irq_pending: bool,
    pub audio: Vrc6Audio,
}

impl Default for Vrc6State {
    fn default() -> Self {
        Self {
            variant_b: false,
            prg_16k: 0,
            prg_8k: 0,
            banking: 0x20,
            chr: [0; 8],
            irq_latch: 0,
            irq_counter: 0,
            irq_mode_cycle: false,
            irq_enabled: false,
            irq_pending: false,
            audio: Vrc6Audio::default(),
        }
    }
}

impl Vrc6State {
    /// Scanline-mode IRQ. Each call stands for one rendered scanline; the latch
    /// is treated as a scanline count rather than a CPU-cycle count.
    pub fn clock_irq(&mut self) {
        if self.irq_counter == 0 {
            self.irq_counter = self.irq_latch;
        } else {
            self.irq_counter -= 1;
            if self.irq_counter == 0 {
                self.irq_pending = true;
            }
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
    Mmc5(Mmc5State),
    Vrc6(Vrc6State),
}

#[derive(Debug, Clone)]
pub enum Mapper {
    Nrom,
    Uxrom { bank: u8 },
    Axrom { bank: u8, upper: bool },
    Cnrom { bank: u8 },
    Mmc1(Mmc1State),
    Mmc3(Mmc3State),
    Mmc5(Mmc5State),
    Vrc6(Vrc6State),
}

impl Mapper {
    pub fn new(number: u16) -> Self {
        match number {
            1 => Mapper::Mmc1(Mmc1State::default()),
            2 => Mapper::Uxrom { bank: 0 },
            3 => Mapper::Cnrom { bank: 0 },
            4 => Mapper::Mmc3(Mmc3State::default()),
            5 => Mapper::Mmc5(Mmc5State::default()),
            7 => Mapper::Axrom {
                bank: 0,
                upper: false,
            },
            24 => Mapper::Vrc6(Vrc6State::default()),
            26 => Mapper::Vrc6(Vrc6State {
                variant_b: true,
                ..Vrc6State::default()
            }),
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
            Mapper::Mmc5(_) => 5,
            Mapper::Axrom { .. } => 7,
            Mapper::Vrc6(state) => {
                if state.variant_b {
                    26
                } else {
                    24
                }
            }
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Mapper::Nrom => "NROM (0)",
            Mapper::Mmc1(_) => "MMC1 (1)",
            Mapper::Uxrom { .. } => "UxROM (2)",
            Mapper::Cnrom { .. } => "CNROM (3)",
            Mapper::Mmc3(_) => "MMC3 (4)",
            Mapper::Mmc5(_) => "MMC5 (5)",
            Mapper::Axrom { .. } => "AxROM (7)",
            Mapper::Vrc6(state) => {
                if state.variant_b {
                    "VRC6b (26)"
                } else {
                    "VRC6a (24)"
                }
            }
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
            // MMC5 keeps its registers in `$5000-$5FFF`; see `write_register`.
            Mapper::Mmc5(_) => {}
            Mapper::Vrc6(state) => {
                let reg = usize::from(if state.variant_b {
                    ((addr & 1) << 1) | ((addr >> 1) & 1)
                } else {
                    addr & 3
                });
                match addr & 0xF000 {
                    0x8000 => state.prg_16k = value & 0x0F,
                    0x9000 | 0xA000 => state.audio.write(addr & 0xF000, reg, value),
                    0xB000 if reg == 3 => state.banking = value,
                    0xB000 => state.audio.write(0xB000, reg, value),
                    0xC000 => state.prg_8k = value & 0x3F,
                    0xD000 => state.chr[reg] = value,
                    0xE000 => state.chr[4 + reg] = value,
                    0xF000 => match reg {
                        0 => state.irq_latch = value,
                        1 => {
                            state.irq_mode_cycle = value & 0x04 != 0;
                            state.irq_enabled = value & 0x02 != 0;
                            if value & 0x01 != 0 {
                                state.irq_pending = false;
                            }
                        }
                        _ => state.irq_pending = false,
                    },
                    _ => {}
                }
            }
        }
    }

    /// MMC5 register window (`$5000-$5FFF`).
    pub fn write_register(&mut self, addr: u16, value: u8) {
        let Mapper::Mmc5(state) = self else {
            return;
        };
        match addr {
            0x5100 => state.prg_mode = value & 0x03,
            0x5101 => state.chr_mode = value & 0x03,
            0x5102 | 0x5103 => {
                state.prg_ram_protect = (state.prg_ram_protect << 1) | (value & 0x03)
            }
            0x5104 => state.exram_mode = value & 0x03,
            0x5105 => state.nametable_map = value,
            0x5106 => state.fill_tile = value,
            0x5107 => state.fill_color = value & 0x03,
            0x5113 => state.prg_ram_bank = value & 0x07,
            0x5114..=0x5117 => state.prg_banks[usize::from(addr - 0x5114)] = value,
            0x5120..=0x512B => {
                let index = usize::from(addr - 0x5120);
                state.chr_banks[index] = u16::from(value) | (u16::from(state.upper_chr) << 8);
            }
            0x5130 => state.upper_chr = value & 0x03,
            0x5200 => state.split_control = value,
            0x5201 => state.split_scroll = value,
            0x5202 => state.split_bank = value,
            0x5203 => state.irq_compare = value,
            0x5204 => {
                state.irq_enabled = value & 0x80 != 0;
                if state.irq_enabled {
                    state.scanline = 0;
                    state.irq_pending = false;
                }
            }
            0x5205 => state.multiplier_a = value,
            0x5206 => state.multiplier_b = value,
            0x5000..=0x5015 => state.audio.write(addr, value),
            0x5C00..=0x5FFF => {
                let index = usize::from(addr - 0x5C00) & (MMC5_EXRAM_LEN - 1);
                state.exram[index] = value;
            }
            _ => {}
        }
    }

    /// Readable MMC5 registers. Everything else in `$5000-$5FFF` reads as zero.
    pub fn read_register(&mut self, addr: u16) -> u8 {
        let Mapper::Mmc5(state) = self else {
            return 0;
        };
        match addr {
            0x5010 | 0x5015 => state.audio.read(addr),
            0x5204 => (u8::from(state.irq_pending) << 7) | (u8::from(state.in_frame) << 6),
            0x5205 => state.product() as u8,
            0x5206 => (state.product() >> 8) as u8,
            0x5C00..=0x5FFF => {
                let index = usize::from(addr - 0x5C00) & (MMC5_EXRAM_LEN - 1);
                state.exram[index]
            }
            _ => 0,
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
            Mapper::Vrc6(state) => Some(match state.banking & 0x0C {
                0x04 => Mirroring::Horizontal,
                0x08 => Mirroring::OneScreenLower,
                0x0C => Mirroring::OneScreenUpper,
                _ => Mirroring::Vertical,
            }),
            // MMC5 nametables are mapped per table in `Cartridge::nametable_index`.
            _ => None,
        }
    }

    /// Called once for each rendered scanline so mappers can drive their IRQ.
    pub fn on_scanline(&mut self) {
        match self {
            Mapper::Mmc3(state) => state.clock_irq(),
            Mapper::Mmc5(state) => state.clock_scanline(),
            Mapper::Vrc6(state) if state.irq_enabled && !state.irq_mode_cycle => state.clock_irq(),
            _ => {}
        }
    }

    pub fn irq_pending(&self) -> bool {
        match self {
            Mapper::Mmc3(state) => state.irq_pending,
            Mapper::Mmc5(state) => state.irq_pending || state.audio.irq_pending(),
            Mapper::Vrc6(state) => state.irq_pending,
            _ => false,
        }
    }

    pub fn prg_ram_disabled(&self) -> bool {
        false
    }

    /// Advances expansion audio by one CPU cycle.
    pub fn clock_audio(&mut self) {
        match self {
            Mapper::Mmc5(state) => state.audio.clock(),
            Mapper::Vrc6(state) => state.audio.clock(),
            _ => {}
        }
    }

    pub fn audio_output(&self) -> f32 {
        match self {
            Mapper::Mmc5(state) => state.audio.output(),
            Mapper::Vrc6(state) => state.audio.output(),
            _ => 0.0,
        }
    }

    pub fn has_expansion_audio(&self) -> bool {
        matches!(self, Mapper::Mmc5(_) | Mapper::Vrc6(_))
    }

    /// Background substitution state for the current scanline, if this is MMC5.
    pub fn mmc5_render(&self) -> Option<Mmc5Render> {
        match self {
            Mapper::Mmc5(state) => Some(Mmc5Render::from_state(state)),
            _ => None,
        }
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
            Mapper::Mmc5(state) => MapperState::Mmc5(state.clone()),
            Mapper::Vrc6(state) => MapperState::Vrc6(state.clone()),
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
            (Mapper::Mmc5(current), MapperState::Mmc5(mut saved)) => {
                if saved.exram.len() != MMC5_EXRAM_LEN {
                    saved.exram.resize(MMC5_EXRAM_LEN, 0);
                }
                *current = saved;
            }
            (Mapper::Vrc6(current), MapperState::Vrc6(mut saved)) => {
                saved.variant_b = current.variant_b;
                *current = saved;
            }
            (Mapper::Nrom, MapperState::Nrom) => {}
            _ => {}
        }
    }
}
