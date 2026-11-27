//! iNES cartridge images and the mappers NES Core supports.

mod audio;
mod mapper;

pub use mapper::{Mapper, MapperState, Mmc5Render};

use std::fmt;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

const INES_HEADER_LEN: usize = 16;
const TRAINER_LEN: usize = 512;
const PRG_RAM_SIZE: usize = 8 * 1024;
const MMC5_PRG_RAM_SIZE: usize = 64 * 1024;
const CHR_RAM_SIZE: usize = 8 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RomError {
    Io(String),
    TooShort,
    BadMagic,
    UnsupportedMapper(u16),
    UnsupportedNes2,
    EmptyPrg,
    TruncatedPrg { expected: usize, actual: usize },
    TruncatedChr { expected: usize, actual: usize },
    UnsupportedChrSize(usize),
}

impl fmt::Display for RomError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RomError::Io(e) => write!(f, "could not read ROM file: {e}"),
            RomError::TooShort => write!(f, "file is too small to be an iNES ROM"),
            RomError::BadMagic => {
                write!(f, "missing iNES signature \"NES<EOF>\"; not a .nes file")
            }
            RomError::UnsupportedMapper(n) => {
                write!(f, "mapper {n} is not supported by this build")
            }
            RomError::UnsupportedNes2 => {
                write!(f, "this NES 2.0 image uses an unsupported header variant")
            }
            RomError::EmptyPrg => write!(f, "ROM contains no PRG data"),
            RomError::TruncatedPrg { expected, actual } => write!(
                f,
                "PRG data is truncated: header expects {expected} bytes, file has {actual}"
            ),
            RomError::TruncatedChr { expected, actual } => write!(
                f,
                "CHR data is truncated: header expects {expected} bytes, file has {actual}"
            ),
            RomError::UnsupportedChrSize(n) => write!(f, "unsupported CHR size of {n} bytes"),
        }
    }
}

impl std::error::Error for RomError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mirroring {
    Horizontal,
    Vertical,
    FourScreen,
    OneScreenLower,
    OneScreenUpper,
}

impl Mirroring {
    pub fn label(self) -> &'static str {
        match self {
            Mirroring::Horizontal => "horizontal",
            Mirroring::Vertical => "vertical",
            Mirroring::FourScreen => "four-screen",
            Mirroring::OneScreenLower => "one-screen (lower)",
            Mirroring::OneScreenUpper => "one-screen (upper)",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Header {
    pub mapper: u16,
    pub prg_banks_16k: u8,
    pub chr_banks_8k: u8,
    pub mirroring: Mirroring,
    pub battery: bool,
    pub four_screen: bool,
    pub trainer: bool,
    pub nes2: bool,
}

/// Persistent mapper state. Static PRG/CHR ROM contents are not duplicated in
/// save states; they are reloaded from the ROM file and validated by fingerprint.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CartridgeState {
    pub prg_ram: Vec<u8>,
    pub chr_ram: Vec<u8>,
    pub mapper: MapperState,
    pub mirroring: Option<Mirroring>,
}

#[derive(Debug)]
pub struct Cartridge {
    pub header: Header,
    pub mapper: Mapper,
    pub mirroring: Mirroring,
    pub path: Option<std::path::PathBuf>,
    prg_rom: Vec<u8>,
    chr_memory: Vec<u8>,
    chr_is_ram: bool,
    prg_ram: Vec<u8>,
}

impl Cartridge {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, RomError> {
        let path = path.as_ref();
        let data = fs::read(path).map_err(|e| RomError::Io(e.to_string()))?;
        let mut cart = Self::from_bytes(&data)?;
        cart.path = Some(path.to_path_buf());
        Ok(cart)
    }

    pub fn from_bytes(data: &[u8]) -> Result<Self, RomError> {
        if data.len() < INES_HEADER_LEN {
            return Err(RomError::TooShort);
        }
        if &data[0..4] != b"NES\x1a" {
            return Err(RomError::BadMagic);
        }

        let flags6 = data[6];
        let flags7 = data[7];
        let nes2 = (flags7 & 0x0C) == 0x08;
        let mut mapper = u16::from(flags6 >> 4) | u16::from(flags7 & 0xF0);
        let mut prg_banks = u32::from(data[4]);
        let mut chr_banks = u32::from(data[5]);

        if nes2 {
            mapper |= u16::from(data[8] & 0x0F) << 8;
            // Upper nibbles are exponents for power-of-two bank counts.
            prg_banks |= u32::from(data[9] & 0x0F) << 8;
            chr_banks |= u32::from(data[9] >> 4) << 8;
            if (data[9] & 0x0F) == 0x0F || (data[9] >> 4) == 0x0F {
                return Err(RomError::UnsupportedNes2);
            }
        }

        if prg_banks == 0 {
            return Err(RomError::EmptyPrg);
        }

        let four_screen = flags6 & 0x08 != 0;
        let mirroring = if four_screen {
            Mirroring::FourScreen
        } else if flags6 & 0x01 != 0 {
            Mirroring::Vertical
        } else {
            Mirroring::Horizontal
        };

        let supported = mapper::SUPPORTED_MAPPERS.contains(&mapper);
        if !supported {
            return Err(RomError::UnsupportedMapper(mapper));
        }

        let trainer = flags6 & 0x04 != 0;
        let prg_len = prg_banks as usize * 16 * 1024;
        let chr_len = chr_banks as usize * 8 * 1024;
        let mut offset = INES_HEADER_LEN + usize::from(trainer) * TRAINER_LEN;

        if data.len() < offset + prg_len {
            return Err(RomError::TruncatedPrg {
                expected: prg_len,
                actual: data.len().saturating_sub(offset),
            });
        }
        let prg_rom = data[offset..offset + prg_len].to_vec();
        offset += prg_len;

        let chr_is_ram = chr_banks == 0;
        let chr_memory = if chr_is_ram {
            vec![0; CHR_RAM_SIZE]
        } else {
            if data.len() < offset + chr_len {
                return Err(RomError::TruncatedChr {
                    expected: chr_len,
                    actual: data.len().saturating_sub(offset),
                });
            }
            data[offset..offset + chr_len].to_vec()
        };

        if chr_memory.len() % 1024 != 0 {
            return Err(RomError::UnsupportedChrSize(chr_memory.len()));
        }

        let header = Header {
            mapper,
            prg_banks_16k: prg_banks.min(255) as u8,
            chr_banks_8k: chr_banks.min(255) as u8,
            mirroring,
            battery: flags6 & 0x02 != 0,
            four_screen,
            trainer,
            nes2,
        };

        Ok(Self {
            mapper: Mapper::new(mapper),
            header,
            mirroring,
            path: None,
            prg_rom,
            chr_memory,
            chr_is_ram,
            prg_ram: vec![
                0;
                if mapper == 5 {
                    MMC5_PRG_RAM_SIZE
                } else {
                    PRG_RAM_SIZE
                }
            ],
        })
    }

    pub fn title(&self) -> String {
        self.path
            .as_ref()
            .and_then(|p| p.file_stem())
            .map(|s| s.to_string_lossy().trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "Untitled ROM".to_string())
    }

    pub fn mapper_name(&self) -> &'static str {
        self.mapper.name()
    }

    pub fn prg_rom_len(&self) -> usize {
        self.prg_rom.len()
    }

    pub fn chr_len(&self) -> usize {
        self.chr_memory.len()
    }

    pub fn chr_is_ram(&self) -> bool {
        self.chr_is_ram
    }

    pub fn battery(&self) -> bool {
        self.header.battery
    }

    pub fn prg_ram_mut(&mut self) -> &mut [u8] {
        &mut self.prg_ram
    }

    pub fn fingerprint(&self) -> u64 {
        let mut hash = 0xcbf2_9ce4_8422_2325u64;
        for byte in self.prg_rom.iter().chain(self.chr_memory.iter()).chain(
            [
                self.header.mapper as u8,
                self.header.prg_banks_16k,
                self.header.chr_banks_8k,
            ]
            .iter(),
        ) {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        hash
    }

    /// CPU-side cartridge read for $4020-$FFFF. Reading `$5010` acknowledges the
    /// MMC5 PCM IRQ, so this takes the cartridge mutably.
    pub fn cpu_read(&mut self, addr: u16) -> u8 {
        match addr {
            0x4020..=0x4FFF => 0,
            0x5000..=0x5FFF => self.mapper.read_register(addr),
            0x6000..=0x7FFF => match self.prg_ram_offset(addr) {
                Some(index) => self.prg_ram[index],
                None => 0,
            },
            0x8000..=0xFFFF => {
                let offset = self.prg_offset(addr);
                self.prg_rom[offset]
            }
            _ => 0,
        }
    }

    /// PRG RAM index for `$6000-$7FFF`, honouring mapper bank registers.
    fn prg_ram_offset(&self, addr: u16) -> Option<usize> {
        if self.mapper.prg_ram_disabled() || self.prg_ram.is_empty() {
            return None;
        }
        let index = match &self.mapper {
            Mapper::Mmc5(state) => {
                usize::from(state.prg_ram_bank & 0x07) * 0x2000 + usize::from(addr - 0x6000)
            }
            _ => usize::from(addr - 0x6000),
        };
        Some(index % self.prg_ram.len())
    }

    pub fn cpu_write(&mut self, addr: u16, value: u8) {
        match addr {
            0x5000..=0x5FFF => self.mapper.write_register(addr, value),
            0x6000..=0x7FFF => {
                if let Some(index) = self.prg_ram_offset(addr) {
                    self.prg_ram[index] = value;
                }
            }
            0x8000..=0xFFFF => {
                let prg_banks = (self.prg_rom.len() / 0x2000).max(1);
                let chr_banks_1k = (self.chr_memory.len() / 0x400).max(1);
                self.mapper
                    .write_prg(addr, value, prg_banks, chr_banks_1k, self.chr_is_ram);
                if let Some(mirroring) = self.mapper.forced_mirroring() {
                    self.mirroring = mirroring;
                }
            }
            _ => {}
        }
    }

    /// PPU pattern-table read ($0000-$1FFF).
    pub fn chr_read(&self, addr: u16) -> u8 {
        let offset = self.chr_offset(addr, false);
        self.chr_memory[offset]
    }

    /// Pattern fetch made by the PPU while rendering. MMC2/MMC4 latch their CHR
    /// banks from these fetches, and the fetch that flips a latch returns the
    /// old bank, so the value is read before the mapper is told about it.
    pub fn chr_read_ppu(&mut self, addr: u16, sprite: bool) -> u8 {
        let value = if sprite {
            self.chr_read_sprite(addr)
        } else {
            self.chr_read(addr)
        };
        self.mapper.notify_ppu_read(addr);
        value
    }

    /// Pattern fetch made on behalf of sprites; MMC5 keeps separate banks for
    /// sprite and background fetches.
    pub fn chr_read_sprite(&self, addr: u16) -> u8 {
        let offset = self.chr_offset(addr, true);
        self.chr_memory[offset]
    }

    /// Physical CHR read used by MMC5 background substitution: extended
    /// attributes and the split region select a complete 4 KB bank themselves,
    /// so the normal CHR banking registers must be bypassed.
    pub fn chr_read_direct(&self, addr: u16) -> u8 {
        self.chr_memory[usize::from(addr) % self.chr_memory.len().max(1)]
    }

    /// Writes only reach CHR when the cartridge has CHR RAM.
    pub fn chr_write(&mut self, addr: u16, value: u8) {
        if !self.chr_is_ram {
            return;
        }
        let offset = self.chr_offset(addr, false);
        self.chr_memory[offset] = value;
    }

    fn prg_offset(&self, addr: u16) -> usize {
        let prg_len = self.prg_rom.len().max(1);
        let bank_16k = addr & 0x3FFF;
        let offset = match &self.mapper {
            Mapper::Nrom => usize::from(addr - 0x8000) % prg_len,
            Mapper::Uxrom { bank } | Mapper::Axrom { bank, .. } => {
                if matches!(self.mapper, Mapper::Axrom { .. }) || addr < 0xC000 {
                    let bank = usize::from(*bank) % (prg_len / 0x4000).max(1);
                    bank * 0x4000 + usize::from(addr & 0x3FFF)
                } else {
                    (prg_len - 0x4000) + usize::from(addr & 0x3FFF)
                }
            }
            Mapper::Cnrom { .. } => usize::from(addr - 0x8000) % prg_len,
            Mapper::ColorDreams { prg, .. } | Mapper::Gxrom { prg, .. } => {
                let banks = (prg_len / 0x8000).max(1);
                (usize::from(*prg) % banks) * 0x8000 + usize::from(addr - 0x8000)
            }
            Mapper::Camerica { bank, .. } => {
                let banks = (prg_len / 0x2000).max(1);
                let slot = usize::from((addr - 0x8000) / 0x2000);
                let bank = if slot < 2 {
                    banks.saturating_sub(2) + slot
                } else {
                    usize::from(*bank) * 2 + slot - 2
                };
                (bank % banks) * 0x2000 + usize::from(addr & 0x1FFF)
            }
            Mapper::Mmc2(state) => {
                let banks = (prg_len / 0x2000).max(1);
                let slot = usize::from((addr - 0x8000) / 0x2000);
                let bank = if state.variant_4 {
                    if slot < 2 {
                        usize::from(state.prg_bank & 0x0F) * 2 + slot
                    } else {
                        banks.saturating_sub(2) + slot - 2
                    }
                } else if slot == 0 {
                    usize::from(state.prg_bank & 0x07)
                } else {
                    banks.saturating_sub(3) + slot - 1
                };
                (bank % banks) * 0x2000 + usize::from(addr & 0x1FFF)
            }
            Mapper::Mmc1(state) => {
                let mode = (state.control >> 2) & 0x03;
                let bank = usize::from(state.prg_bank & 0x0F);
                match (mode, addr) {
                    (0 | 1, _) => {
                        let bank = (bank >> 1) % (prg_len / 0x8000).max(1);
                        bank * 0x8000 + usize::from(addr - 0x8000)
                    }
                    (2, 0x8000..=0xBFFF) => usize::from(addr & 0x3FFF),
                    (2, _) => (bank % (prg_len / 0x4000).max(1)) * 0x4000 + usize::from(bank_16k),
                    (3, 0x8000..=0xBFFF) => {
                        (bank % (prg_len / 0x4000).max(1)) * 0x4000 + usize::from(bank_16k)
                    }
                    _ => usize::from(bank_16k) + prg_len - 0x4000,
                }
            }
            Mapper::Mmc5(state) => {
                let banks = (prg_len / 0x2000).max(1);
                let slot = usize::from((addr - 0x8000) / 0x2000);
                let register = |value: u8| usize::from(value & 0x7F);
                let bank = match state.prg_mode & 0x03 {
                    0 => (register(state.prg_banks[3]) >> 2) * 4 + slot,
                    1 => {
                        if addr < 0xC000 {
                            (register(state.prg_banks[1]) >> 1) * 2 + slot
                        } else {
                            (register(state.prg_banks[3]) >> 1) * 2 + slot - 2
                        }
                    }
                    2 => {
                        if addr < 0xC000 {
                            (register(state.prg_banks[1]) >> 1) * 2 + slot
                        } else if addr < 0xE000 {
                            register(state.prg_banks[2])
                        } else {
                            register(state.prg_banks[3])
                        }
                    }
                    _ => register(state.prg_banks[slot]),
                };
                bank % banks * 0x2000 + usize::from(addr & 0x1FFF)
            }
            Mapper::Vrc6(state) => {
                let banks = (prg_len / 0x2000).max(1);
                let slot = usize::from((addr - 0x8000) / 0x2000);
                let bank = match slot {
                    0 | 1 => usize::from(state.prg_16k & 0x0F) * 2 + slot,
                    2 => usize::from(state.prg_8k & 0x3F),
                    _ => banks - 1,
                };
                bank % banks * 0x2000 + usize::from(addr & 0x1FFF)
            }
            Mapper::Mmc3(state) => {
                let banks = prg_len / 0x2000;
                let last = banks.saturating_sub(1);
                let second_last = banks.saturating_sub(2);
                let fixed_second = !state.prg_mode;
                let slot = usize::from((addr - 0x8000) / 0x2000);
                let bank = match (slot, fixed_second) {
                    (0, true) => usize::from(state.banks[6] & 0x3E) % banks,
                    (0, false) => second_last,
                    (1, _) => {
                        let base = if fixed_second {
                            usize::from(state.banks[6] & 0x3E) + 1
                        } else {
                            usize::from(state.banks[7])
                        };
                        base % banks
                    }
                    (2, true) => usize::from(state.banks[7]) % banks,
                    (2, false) => usize::from(state.banks[6] & 0x3E) % banks,
                    (3, _) => {
                        if fixed_second {
                            last
                        } else {
                            (usize::from(state.banks[6] & 0x3E) + 1) % banks
                        }
                    }
                    _ => last,
                };
                bank * 0x2000 + usize::from(addr & 0x1FFF)
            }
        };
        offset % prg_len
    }

    fn chr_offset(&self, addr: u16, sprite: bool) -> usize {
        let addr = usize::from(addr & 0x1FFF);
        let len = self.chr_memory.len().max(1);
        let offset = match &self.mapper {
            Mapper::Nrom | Mapper::Uxrom { .. } | Mapper::Axrom { .. } => addr % len,
            Mapper::Mmc5(state) => {
                let banks_1k = (len / 0x400).max(1);
                let slot = addr / 0x400;
                let register = if sprite { slot } else { 8 + slot % 4 };
                (usize::from(state.chr_banks[register]) % banks_1k) * 0x400 + (addr & 0x3FF)
            }
            Mapper::Vrc6(state) => {
                let banks_1k = (len / 0x400).max(1);
                let slot = addr / 0x400;
                (usize::from(state.chr[slot]) % banks_1k) * 0x400 + (addr & 0x3FF)
            }
            Mapper::Cnrom { bank } => {
                let banks_8k = (len / 0x2000).max(1);
                (usize::from(*bank) % banks_8k) * 0x2000 + addr
            }
            Mapper::ColorDreams { chr, .. } | Mapper::Gxrom { chr, .. } => {
                let banks_8k = (len / 0x2000).max(1);
                (usize::from(*chr) % banks_8k) * 0x2000 + addr
            }
            Mapper::Camerica { .. } => addr % len,
            Mapper::Mmc2(state) => {
                let banks_4k = (len / 0x1000).max(1);
                let half = usize::from(addr >= 0x1000);
                let bank = state.chr_banks[half * 2 + usize::from(state.latch[half])];
                (usize::from(bank) % banks_4k) * 0x1000 + (addr & 0x0FFF)
            }
            Mapper::Mmc1(state) => {
                if state.control & 0x10 == 0 {
                    let banks_8k = (len / 0x2000).max(1);
                    let bank = usize::from(state.chr_bank0 >> 1) % banks_8k;
                    bank * 0x2000 + addr
                } else if addr < 0x1000 {
                    let banks_4k = (len / 0x1000).max(1);
                    let bank = usize::from(state.chr_bank0) % banks_4k;
                    bank * 0x1000 + addr
                } else {
                    let banks_4k = (len / 0x1000).max(1);
                    let bank = usize::from(state.chr_bank1) % banks_4k;
                    bank * 0x1000 + (addr - 0x1000)
                }
            }
            Mapper::Mmc3(state) => {
                let banks_1k = (len / 0x400).max(1);
                let slot = addr / 0x400;
                let bank = match (state.chr_mode, slot) {
                    (false, 0) | (false, 1) => usize::from(state.banks[0] & 0xFE) + slot,
                    (false, 2) | (false, 3) | (false, 4) | (false, 5) => {
                        usize::from(state.banks[slot])
                    }
                    (false, _) => usize::from(state.banks[1] & 0xFE) + slot - 6,
                    (true, 0..=3) => usize::from(state.banks[2 + slot]),
                    (true, 4) | (true, 5) => usize::from(state.banks[0] & 0xFE) + slot - 4,
                    _ => usize::from(state.banks[1] & 0xFE) + slot - 6,
                };
                (bank % banks_1k) * 0x400 + (addr & 0x3FF)
            }
        };
        offset % len
    }

    /// Maps a nametable address ($2000-$3EFF) onto the cartridge's 4KB VRAM.
    pub fn nametable_index(&self, addr: u16) -> usize {
        let addr = (addr & 0x0FFF) as usize;
        let table = addr / 0x400;
        let offset = addr % 0x400;
        if let Mapper::Mmc5(state) = &self.mapper {
            // 0 and 1 select a CIRAM page; ExRAM and fill mode are approximated
            // with page 0 because NES Core does not render ExRAM nametables.
            let page = (state.nametable_map >> (table * 2)) & 0x03;
            return usize::from(page == 1) * 0x400 + offset;
        }
        let physical = match self.mirroring {
            Mirroring::Horizontal => [0, 0, 1, 1][table],
            Mirroring::Vertical => [0, 1, 0, 1][table],
            Mirroring::FourScreen => table,
            Mirroring::OneScreenLower => 0,
            Mirroring::OneScreenUpper => 1,
        };
        physical * 0x400 + offset
    }

    /// Advances the mapper's expansion audio by one CPU cycle.
    pub fn clock_audio(&mut self) {
        self.mapper.clock_audio();
    }

    pub fn audio_output(&self) -> f32 {
        self.mapper.audio_output()
    }

    pub fn has_expansion_audio(&self) -> bool {
        self.mapper.has_expansion_audio()
    }

    /// MMC5 background substitution data for the current scanline.
    pub fn mmc5_render(&self) -> Option<Mmc5Render> {
        self.mapper.mmc5_render()
    }

    /// Mapper IRQ lines are clocked once per rendered scanline (A12 rise).
    pub fn on_scanline(&mut self) {
        self.mapper.on_scanline();
    }

    pub fn irq_pending(&self) -> bool {
        self.mapper.irq_pending()
    }

    pub fn save_state(&self) -> CartridgeState {
        CartridgeState {
            prg_ram: self.prg_ram.clone(),
            chr_ram: if self.chr_is_ram {
                self.chr_memory.clone()
            } else {
                Vec::new()
            },
            mapper: self.mapper.state(),
            mirroring: Some(self.mirroring),
        }
    }

    pub fn load_state(&mut self, state: &CartridgeState) {
        if state.prg_ram.len() == self.prg_ram.len() {
            self.prg_ram.copy_from_slice(&state.prg_ram);
        }
        if self.chr_is_ram && state.chr_ram.len() == self.chr_memory.len() {
            self.chr_memory.copy_from_slice(&state.chr_ram);
        }
        self.mapper.set_state(state.mapper.clone());
        if let Some(mirroring) = state.mirroring {
            self.mirroring = mirroring;
        }
    }
}
