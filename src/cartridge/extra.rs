//! Additional mappers that share one table-driven implementation.
//!
//! Every mapper here is expressed as four 8 KB PRG slots and eight 1 KB CHR
//! slots. A register write recomputes those tables, and the cartridge then
//! resolves addresses through them, so adding a simple mapper only needs a new
//! decode arm. Bus conflicts are not modelled.

use serde::{Deserialize, Serialize};

use super::Mirroring;

/// Mapper numbers handled by [`ExtraState`].
pub const EXTRA_MAPPERS: [u16; 54] = [
    13, 16, 18, 21, 22, 23, 25, 32, 33, 34, 38, 48, 65, 67, 69, 70, 72, 73, 75, 76, 78, 79, 85, 86,
    87, 88, 89, 92, 93, 94, 95, 107, 112, 113, 133, 140, 146, 148, 149, 151, 152, 154, 159, 180,
    184, 200, 201, 202, 203, 206, 210, 225, 229, 240,
];

/// More mappers, kept in a second list so the first stays readable.
pub const EXTRA_MAPPERS_2: [u16; 3] = [241, 242, 246];

/// True when `kind` is handled by [`ExtraState`].
pub fn handles(kind: u16) -> bool {
    EXTRA_MAPPERS.contains(&kind) || EXTRA_MAPPERS_2.contains(&kind)
}

/// PRG bank marker meaning "the last 8 KB bank".
const LAST: u16 = 0xFFFF;
/// PRG bank marker meaning "the second-last 8 KB bank".
const SECOND_LAST: u16 = 0xFFFE;

/// Sizes the register decoder needs from the cartridge.
#[derive(Debug, Clone, Copy)]
pub struct Geometry {
    pub prg_8k: usize,
    pub chr_1k: usize,
    pub chr_ram: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtraState {
    pub kind: u16,
    /// 8 KB PRG bank for each of `$8000`, `$A000`, `$C000` and `$E000`.
    pub prg: [u16; 4],
    /// 1 KB CHR bank for each eighth of the pattern tables.
    pub chr: [u16; 8],
    pub mirroring: Option<Mirroring>,
    pub regs: [u8; 16],
    pub select: u8,
    pub irq_enabled: bool,
    pub irq_ack_enable: bool,
    pub irq_cycle_mode: bool,
    pub irq_counting: bool,
    pub irq_pending: bool,
    pub irq_counter: u16,
    pub irq_latch: u16,
    /// Width mask of the IRQ counter (Jaleco SS88006 selects 4/8/12/16 bits).
    #[serde(default = "full_mask")]
    pub irq_mask: u16,
    /// Per-nametable CIRAM page (0 or 1) for mappers that set it from CHR bank bits.
    #[serde(default)]
    pub nt: Option<[u8; 4]>,
}

fn full_mask() -> u16 {
    0xFFFF
}

fn mirroring_from(value: u8) -> Mirroring {
    match value & 3 {
        0 => Mirroring::Vertical,
        1 => Mirroring::Horizontal,
        2 => Mirroring::OneScreenLower,
        _ => Mirroring::OneScreenUpper,
    }
}

pub fn name(kind: u16) -> &'static str {
    match kind {
        16 => "Bandai FCG (16)",
        21 => "VRC4a/c (21)",
        22 => "VRC2a (22)",
        23 => "VRC2b/VRC4e (23)",
        25 => "VRC4b/d (25)",
        33 => "Taito TC0190 (33)",
        34 => "BNROM/NINA-001 (34)",
        69 => "Sunsoft FME-7 (69)",
        70 => "Bandai 74161 (70)",
        75 => "VRC1 (75)",
        78 => "Irem 74161 (78)",
        79 => "NINA-03/06 (79)",
        85 => "VRC7 (85)",
        87 => "Jaleco JF-05 (87)",
        88 => "Namco 118 (88)",
        89 => "Sunsoft-2 (89)",
        93 => "Sunsoft-2 (93)",
        94 => "UN1ROM (94)",
        113 => "NINA-03/06 (113)",
        140 => "Jaleco JF-11 (140)",
        146 => "Sachen (146)",
        152 => "Bandai 74161 (152)",
        154 => "Namco 118 (154)",
        159 => "Bandai FCG (159)",
        180 => "UNROM reversed (180)",
        184 => "Sunsoft-1 (184)",
        206 => "DxROM (206)",
        13 => "CPROM (13)",
        18 => "Jaleco SS88006 (18)",
        32 => "Irem G-101 (32)",
        38 => "Bit Corp. (38)",
        48 => "Taito TC0190+IRQ (48)",
        65 => "Irem H3001 (65)",
        67 => "Sunsoft-3 (67)",
        72 => "Jaleco JF-17 (72)",
        73 => "VRC3 (73)",
        76 => "Namco 109 (76)",
        86 => "Jaleco JF-13 (86)",
        92 => "Jaleco JF-19 (92)",
        95 => "Namco 3425 (95)",
        107 => "Magic Dragon (107)",
        112 => "Asder (112)",
        133 => "Sachen SA72008 (133)",
        148 => "Sachen SA-0037 (148)",
        149 => "Sachen SA-0036 (149)",
        151 => "VRC1 (151)",
        200 => "1200-in-1 (200)",
        201 => "21-in-1 (201)",
        202 => "150-in-1 (202)",
        203 => "35-in-1 (203)",
        210 => "Namco 175/340 (210)",
        225 => "64-in-1 (225)",
        229 => "31-in-1 (229)",
        240 => "Multicart (240)",
        241 => "BNROM-like (241)",
        242 => "Waixing (242)",
        246 => "Fong Shen Bang (246)",
        _ => "Unknown",
    }
}

impl ExtraState {
    pub fn new(kind: u16) -> Self {
        let kind = match kind {
            146 => 79,
            151 => 75,
            other => other,
        };
        let prg = match kind {
            34 | 79 | 113 | 140 | 87 | 184 | 13 | 38 | 86 | 107 | 133 | 148 | 149 | 240 | 241
            | 242 | 201 | 225 | 229 => [0, 1, 2, 3],
            180 | 92 | 200 | 202 | 203 => [0, 1, 0, 1],
            _ => [0, 1, SECOND_LAST, LAST],
        };
        Self {
            kind,
            prg,
            chr: [0, 1, 2, 3, 4, 5, 6, 7],
            mirroring: None,
            regs: [0; 16],
            select: 0,
            irq_enabled: false,
            irq_ack_enable: false,
            irq_cycle_mode: false,
            irq_counting: false,
            irq_pending: false,
            irq_counter: 0,
            irq_latch: 0,
            irq_mask: 0xFFFF,
            nt: None,
        }
    }

    pub fn name(&self) -> &'static str {
        name(self.kind)
    }

    /// The mapper number this state was created for.
    pub fn number(&self) -> u16 {
        self.kind
    }

    /// Resolves PRG slot `slot` (0-3) to a physical 8 KB bank.
    pub fn prg_bank(&self, slot: usize, banks: usize) -> usize {
        let banks = banks.max(1);
        let bank = match self.prg[slot & 3] {
            LAST => banks - 1,
            SECOND_LAST => banks.saturating_sub(2),
            n => usize::from(n),
        };
        bank % banks
    }

    /// Resolves CHR slot `slot` (0-7) to a physical 1 KB bank.
    pub fn chr_bank(&self, slot: usize, banks: usize) -> usize {
        let value = self.chr[slot & 7];
        let value = if self.kind == 22 { value >> 1 } else { value };
        usize::from(value) % banks.max(1)
    }

    fn set_prg32(&mut self, bank: u8) {
        let b = u16::from(bank) * 4;
        self.prg = [b, b + 1, b + 2, b + 3];
    }

    /// Switchable 16 KB bank at `$8000` with the last 16 KB fixed at `$C000`.
    fn set_prg16_low(&mut self, bank: u8) {
        let b = u16::from(bank) * 2;
        self.prg[0] = b;
        self.prg[1] = b + 1;
        self.prg[2] = SECOND_LAST;
        self.prg[3] = LAST;
    }

    fn set_chr8(&mut self, bank: u16) {
        for i in 0..8u16 {
            self.chr[usize::from(i)] = bank * 8 + i;
        }
    }

    fn set_chr4(&mut self, half: usize, bank: u16) {
        for i in 0..4u16 {
            self.chr[half * 4 + usize::from(i)] = bank * 4 + i;
        }
    }

    fn set_chr2(&mut self, index: usize, bank: u16) {
        self.chr[index * 2] = bank * 2;
        self.chr[index * 2 + 1] = bank * 2 + 1;
    }

    /// Register writes in `$4020-$7FFF`.
    pub fn write_low(&mut self, addr: u16, value: u8, g: Geometry) {
        match self.kind {
            34 if !g.chr_ram => match addr {
                0x7FFD => self.set_prg32(value & 1),
                0x7FFE => self.set_chr4(0, u16::from(value & 0x0F)),
                0x7FFF => self.set_chr4(1, u16::from(value & 0x0F)),
                _ => {}
            },
            79 if addr & 0xE100 == 0x4100 => {
                self.set_prg32((value >> 3) & 1);
                self.set_chr8(u16::from(value & 7));
            }
            113 if addr & 0xE100 == 0x4100 => {
                self.set_prg32((value >> 3) & 7);
                self.set_chr8(u16::from((value & 7) | ((value >> 3) & 8)));
                self.mirroring = Some(if value & 0x80 != 0 {
                    Mirroring::Vertical
                } else {
                    Mirroring::Horizontal
                });
            }
            87 if (0x6000..=0x7FFF).contains(&addr) => {
                self.set_chr8(u16::from(((value & 1) << 1) | ((value >> 1) & 1)));
            }
            140 if (0x6000..=0x7FFF).contains(&addr) => {
                self.set_prg32((value >> 4) & 3);
                self.set_chr8(u16::from(value & 0x0F));
            }
            184 if (0x6000..=0x7FFF).contains(&addr) => {
                self.set_chr4(0, u16::from(value & 7));
                self.set_chr4(1, u16::from(4 | ((value >> 4) & 3)));
            }
            16 | 159 if (0x6000..=0x7FFF).contains(&addr) => self.bandai_write(addr & 0x0F, value),
            38 if (0x7000..=0x7FFF).contains(&addr) => {
                self.set_prg32(value & 3);
                self.set_chr8(u16::from((value >> 2) & 3));
            }
            86 if (0x6000..=0x6FFF).contains(&addr) => {
                self.set_prg32((value >> 4) & 3);
                self.set_chr8(u16::from((value & 3) | ((value >> 4) & 4)));
            }
            133 if addr & 0xE100 == 0x4100 => {
                self.set_prg32((value >> 2) & 1);
                self.set_chr8(u16::from(value & 3));
            }
            240 if (0x4020..=0x5FFF).contains(&addr) => {
                self.set_prg32(value >> 4);
                self.set_chr8(u16::from(value & 0x0F));
            }
            246 if (0x6000..=0x67FF).contains(&addr) => {
                let reg = usize::from(addr & 7);
                if reg < 4 {
                    self.prg[reg] = u16::from(value);
                } else {
                    self.set_chr2(reg - 4, u16::from(value));
                }
            }
            _ => {}
        }
    }

    /// Register writes in `$8000-$FFFF`.
    pub fn write_prg(&mut self, addr: u16, value: u8, g: Geometry) {
        match self.kind {
            34 => {
                if g.chr_ram {
                    self.set_prg32(value);
                }
            }
            70 => {
                self.set_prg16_low((value >> 4) & 0x0F);
                self.set_chr8(u16::from(value & 0x0F));
            }
            152 => {
                self.set_prg16_low((value >> 4) & 7);
                self.set_chr8(u16::from(value & 0x0F));
                self.mirroring = Some(if value & 0x80 != 0 {
                    Mirroring::OneScreenUpper
                } else {
                    Mirroring::OneScreenLower
                });
            }
            78 => {
                self.set_prg16_low(value & 7);
                self.set_chr8(u16::from((value >> 4) & 0x0F));
                self.mirroring = Some(if value & 8 != 0 {
                    Mirroring::Vertical
                } else {
                    Mirroring::Horizontal
                });
            }
            89 => {
                self.set_prg16_low((value >> 4) & 7);
                self.set_chr8(u16::from((value & 7) | ((value >> 4) & 8)));
                self.mirroring = Some(if value & 8 != 0 {
                    Mirroring::OneScreenUpper
                } else {
                    Mirroring::OneScreenLower
                });
            }
            93 => self.set_prg16_low((value >> 4) & 7),
            94 => self.set_prg16_low((value >> 2) & 7),
            180 => {
                let b = u16::from(value & 7) * 2;
                self.prg[2] = b;
                self.prg[3] = b + 1;
            }
            206 | 88 | 154 | 76 | 95 => self.namco118_write(addr, value),
            13 => self.set_chr4(1, u16::from(value & 3)),
            72 | 92 => {
                let rising = value & !self.regs[0] & 0xC0;
                if rising & 0x80 != 0 {
                    if self.kind == 72 {
                        self.set_prg16_low(value & 0x0F);
                    } else {
                        let b = u16::from(value & 0x0F) * 2;
                        self.prg = [0, 1, b, b + 1];
                    }
                }
                if rising & 0x40 != 0 {
                    self.set_chr8(u16::from(value & 0x0F));
                }
                self.regs[0] = value & 0xC0;
            }
            107 => {
                self.set_prg32(value >> 1);
                self.set_chr8(u16::from(value));
            }
            148 => {
                self.set_prg32((value >> 3) & 1);
                self.set_chr8(u16::from(value & 7));
            }
            149 => self.set_chr8(u16::from((value >> 7) & 1)),
            200 | 202 | 203 | 225 | 229 | 201 | 241 | 242 => self.multicart_write(addr, value),
            210 => self.namco175_write(addr, value),
            112 => self.asder_write(addr, value),
            18 => self.jaleco_write(addr, value),
            32 => self.irem_g101_write(addr, value),
            48 => self.taito48_write(addr, value),
            65 => self.irem_h3001_write(addr, value),
            67 => self.sunsoft3_write(addr, value),
            73 => self.vrc3_write(addr, value),
            33 => self.taito_write(addr, value),
            75 => self.vrc1_write(addr, value),
            21 | 22 | 23 | 25 => self.vrc4_write(addr, value),
            85 => self.vrc7_write(addr, value),
            69 => self.fme7_write(addr, value),
            16 | 159 => self.bandai_write(addr & 0x0F, value),
            _ => {}
        }
    }

    /// DxROM / Namco 118: an MMC3-style register pair without IRQ or PRG/CHR
    /// mode bits. Mappers 88 and 154 restrict where the CHR banks come from.
    fn namco118_write(&mut self, addr: u16, value: u8) {
        if self.kind == 154 && addr & 0x6000 == 0 {
            self.mirroring = Some(if value & 0x40 != 0 {
                Mirroring::OneScreenUpper
            } else {
                Mirroring::OneScreenLower
            });
        }
        if addr & 0xE000 != 0x8000 {
            return;
        }
        if addr & 1 == 0 {
            self.select = value & 7;
            return;
        }
        self.regs[usize::from(self.select)] = value;
        let regs = self.regs;
        let reg = |i: usize| u16::from(regs[i]);
        if self.kind == 76 {
            // Namco 109: R2-R5 are 2 KB banks covering the whole pattern table.
            for i in 0..4 {
                self.set_chr2(i, reg(2 + i) & 0x3F);
            }
        } else {
            let restricted = self.kind == 88 || self.kind == 154;
            let r0 = reg(0) & 0x3E;
            let r1 = reg(1) & 0x3E;
            self.chr[0] = r0;
            self.chr[1] = r0 + 1;
            self.chr[2] = r1;
            self.chr[3] = r1 + 1;
            for i in 0..4 {
                let r = reg(2 + i) & 0x3F;
                self.chr[4 + i] = if restricted { r | 0x40 } else { r };
            }
        }
        if self.kind == 95 {
            // Bit 5 of R0/R1 selects the CIRAM page for the two nametable pairs.
            let a = (self.regs[0] >> 5) & 1;
            let b = (self.regs[1] >> 5) & 1;
            self.nt = Some([a, a, b, b]);
            self.chr[0] &= 0x1F;
            self.chr[1] &= 0x1F;
            self.chr[2] &= 0x1F;
            self.chr[3] &= 0x1F;
            for i in 4..8 {
                self.chr[i] &= 0x1F;
            }
        }
        self.prg[0] = reg(6) & 0x0F;
        self.prg[1] = reg(7) & 0x0F;
        self.prg[2] = SECOND_LAST;
        self.prg[3] = LAST;
    }

    fn taito_write(&mut self, addr: u16, value: u8) {
        match addr & 0xA003 {
            0x8000 => {
                self.prg[0] = u16::from(value & 0x3F);
                self.mirroring = Some(if value & 0x40 != 0 {
                    Mirroring::Horizontal
                } else {
                    Mirroring::Vertical
                });
            }
            0x8001 => self.prg[1] = u16::from(value & 0x3F),
            0x8002 => self.set_chr2(0, u16::from(value)),
            0x8003 => self.set_chr2(1, u16::from(value)),
            r @ 0xA000..=0xA003 => self.chr[4 + usize::from(r & 3)] = u16::from(value),
            _ => {}
        }
    }

    fn vrc1_write(&mut self, addr: u16, value: u8) {
        match addr & 0xF000 {
            0x8000 => self.prg[0] = u16::from(value & 0x0F),
            0x9000 => {
                self.mirroring = Some(if value & 1 != 0 {
                    Mirroring::Horizontal
                } else {
                    Mirroring::Vertical
                });
                self.regs[3] = (value >> 1) & 1;
                self.regs[4] = (value >> 2) & 1;
            }
            0xA000 => self.prg[1] = u16::from(value & 0x0F),
            0xC000 => self.prg[2] = u16::from(value & 0x0F),
            0xE000 => self.regs[5] = value & 0x0F,
            0xF000 => self.regs[6] = value & 0x0F,
            _ => return,
        }
        let low = u16::from(self.regs[3]) << 4 | u16::from(self.regs[5]);
        let high = u16::from(self.regs[4]) << 4 | u16::from(self.regs[6]);
        self.set_chr4(0, low);
        self.set_chr4(1, high);
        self.prg[3] = LAST;
    }

    /// Decodes the two register-select address lines for each VRC2/VRC4 wiring.
    fn vrc4_reg(&self, addr: u16) -> usize {
        let bit = |n: u16| usize::from((addr >> n) & 1);
        match self.kind {
            21 => (bit(1) | bit(6)) | ((bit(2) | bit(7)) << 1),
            22 => bit(1) | (bit(0) << 1),
            23 => (bit(0) | bit(2)) | ((bit(1) | bit(3)) << 1),
            _ => (bit(1) | bit(3)) | ((bit(0) | bit(2)) << 1),
        }
    }

    fn vrc4_update_prg(&mut self) {
        let p0 = u16::from(self.regs[0]);
        let p1 = u16::from(self.regs[2]);
        self.prg = if self.regs[1] & 2 != 0 {
            [SECOND_LAST, p1, p0, LAST]
        } else {
            [p0, p1, SECOND_LAST, LAST]
        };
    }

    fn vrc_irq_control(&mut self, value: u8) {
        self.irq_ack_enable = value & 1 != 0;
        self.irq_enabled = value & 2 != 0;
        self.irq_cycle_mode = value & 4 != 0;
        self.irq_pending = false;
        if self.irq_enabled {
            self.irq_counter = self.irq_latch;
        }
    }

    fn vrc4_write(&mut self, addr: u16, value: u8) {
        let reg = self.vrc4_reg(addr);
        match addr & 0xF000 {
            0x8000 => {
                self.regs[0] = value & 0x1F;
                self.vrc4_update_prg();
            }
            0x9000 => {
                if reg < 2 {
                    self.mirroring = Some(mirroring_from(value));
                } else {
                    self.regs[1] = value & 3;
                    self.vrc4_update_prg();
                }
            }
            0xA000 => {
                self.regs[2] = value & 0x1F;
                self.vrc4_update_prg();
            }
            base @ 0xB000..=0xE000 => {
                let slot = usize::from((base - 0xB000) >> 12) * 2 + (reg >> 1);
                let old = self.chr[slot];
                self.chr[slot] = if reg & 1 == 0 {
                    (old & 0x1F0) | u16::from(value & 0x0F)
                } else {
                    (old & 0x00F) | (u16::from(value & 0x1F) << 4)
                };
            }
            0xF000 => match reg {
                0 => self.irq_latch = (self.irq_latch & 0xF0) | u16::from(value & 0x0F),
                1 => self.irq_latch = (self.irq_latch & 0x0F) | (u16::from(value & 0x0F) << 4),
                2 => self.vrc_irq_control(value),
                _ => {
                    self.irq_pending = false;
                    self.irq_enabled = self.irq_ack_enable;
                }
            },
            _ => {}
        }
    }

    fn vrc7_write(&mut self, addr: u16, value: u8) {
        let second = addr & 0x18 != 0;
        match (addr & 0xF000, second) {
            (0x8000, false) => self.prg[0] = u16::from(value & 0x3F),
            (0x8000, true) => self.prg[1] = u16::from(value & 0x3F),
            (0x9000, false) => self.prg[2] = u16::from(value & 0x3F),
            (base @ 0xA000..=0xD000, _) => {
                let slot = usize::from((base - 0xA000) >> 12) * 2 + usize::from(second);
                self.chr[slot] = u16::from(value);
            }
            (0xE000, false) => self.mirroring = Some(mirroring_from(value)),
            (0xE000, true) => self.irq_latch = u16::from(value),
            (0xF000, false) => self.vrc_irq_control(value),
            (0xF000, true) => {
                self.irq_pending = false;
                self.irq_enabled = self.irq_ack_enable;
            }
            _ => {}
        }
        self.prg[3] = LAST;
    }

    fn fme7_write(&mut self, addr: u16, value: u8) {
        match addr & 0xE000 {
            0x8000 => self.select = value & 0x0F,
            0xA000 => match self.select {
                n @ 0..=7 => self.chr[usize::from(n)] = u16::from(value),
                9 => self.prg[0] = u16::from(value & 0x3F),
                10 => self.prg[1] = u16::from(value & 0x3F),
                11 => self.prg[2] = u16::from(value & 0x3F),
                12 => self.mirroring = Some(mirroring_from(value)),
                13 => {
                    self.irq_counting = value & 1 != 0;
                    self.irq_enabled = value & 0x80 != 0;
                    self.irq_pending = false;
                }
                14 => self.irq_counter = (self.irq_counter & 0xFF00) | u16::from(value),
                15 => self.irq_counter = (self.irq_counter & 0x00FF) | (u16::from(value) << 8),
                _ => {}
            },
            _ => {}
        }
        self.prg[3] = LAST;
    }

    fn bandai_write(&mut self, reg: u16, value: u8) {
        match reg {
            0..=7 => self.chr[usize::from(reg)] = u16::from(value),
            8 => self.set_prg16_low(value & 0x0F),
            9 => self.mirroring = Some(mirroring_from(value)),
            10 => {
                self.irq_enabled = value & 1 != 0;
                self.irq_pending = false;
                self.irq_counter = self.irq_latch;
            }
            11 => self.irq_latch = (self.irq_latch & 0xFF00) | u16::from(value),
            12 => self.irq_latch = (self.irq_latch & 0x00FF) | (u16::from(value) << 8),
            _ => {}
        }
    }

    /// Address-latch multicarts: the bank comes from the address bus.
    fn multicart_write(&mut self, addr: u16, value: u8) {
        let pair = |b: u16| [b * 2, b * 2 + 1, b * 2, b * 2 + 1];
        match self.kind {
            200 => {
                let b = addr & 7;
                self.prg = pair(b);
                self.set_chr8(b);
                self.mirroring = Some(if addr & 8 != 0 {
                    Mirroring::Horizontal
                } else {
                    Mirroring::Vertical
                });
            }
            201 => {
                self.set_prg32((addr & 0xFF) as u8);
                self.set_chr8(addr & 0xFF);
            }
            202 => {
                let b = (addr >> 1) & 7;
                if addr & 0x0C == 0x0C {
                    self.set_prg32((b >> 1) as u8);
                } else {
                    self.prg = pair(b);
                }
                self.set_chr8(b);
                self.mirroring = Some(if addr & 1 != 0 {
                    Mirroring::Horizontal
                } else {
                    Mirroring::Vertical
                });
            }
            203 => {
                self.prg = pair(u16::from(value >> 2));
                self.set_chr8(u16::from(value & 3));
            }
            225 => {
                let chr = (addr & 0x3F) | ((addr >> 8) & 0x40);
                let prg16 = ((addr >> 6) & 0x3F) | ((addr >> 8) & 0x40);
                self.set_chr8(chr);
                if addr & 0x1000 != 0 {
                    self.prg = pair(prg16);
                } else {
                    let b = (prg16 >> 1) * 4;
                    self.prg = [b, b + 1, b + 2, b + 3];
                }
                self.mirroring = Some(if addr & 0x2000 != 0 {
                    Mirroring::Horizontal
                } else {
                    Mirroring::Vertical
                });
            }
            229 => {
                let b = addr & 0x1F;
                self.set_chr8(addr & 0xFF);
                if b == 0 {
                    self.set_prg32(0);
                } else {
                    self.prg = pair(b);
                }
                self.mirroring = Some(if addr & 0x20 != 0 {
                    Mirroring::Horizontal
                } else {
                    Mirroring::Vertical
                });
            }
            241 => self.set_prg32(value),
            _ => self.set_prg32(((addr >> 3) & 0x0F) as u8), // 242
        }
    }

    fn namco175_write(&mut self, addr: u16, value: u8) {
        match addr & 0xF800 {
            0x8000 | 0x8800 | 0x9000 | 0x9800 | 0xA000 | 0xA800 | 0xB000 | 0xB800 => {
                self.chr[usize::from((addr >> 11) & 7)] = u16::from(value);
            }
            0xE000 => {
                self.prg[0] = u16::from(value & 0x3F);
                match value >> 6 {
                    1 => self.mirroring = Some(Mirroring::Vertical),
                    2 => self.mirroring = Some(Mirroring::OneScreenUpper),
                    3 => self.mirroring = Some(Mirroring::Horizontal),
                    _ => {}
                }
            }
            0xE800 => self.prg[1] = u16::from(value & 0x3F),
            0xF000 => self.prg[2] = u16::from(value & 0x3F),
            _ => {}
        }
        self.prg[3] = LAST;
    }

    fn asder_write(&mut self, addr: u16, value: u8) {
        match addr & 0xE001 {
            0x8000 => self.select = value & 7,
            0xA000 => match self.select {
                0 => self.prg[0] = u16::from(value),
                1 => self.prg[1] = u16::from(value),
                2 => self.set_chr2(0, u16::from(value >> 1)),
                3 => self.set_chr2(1, u16::from(value >> 1)),
                n @ 4..=7 => self.chr[usize::from(n)] = u16::from(value),
                _ => {}
            },
            0xE000 => self.mirroring = Some(mirroring_from(value ^ 1)),
            _ => {}
        }
        self.prg[2] = SECOND_LAST;
        self.prg[3] = LAST;
    }

    fn jaleco_write(&mut self, addr: u16, value: u8) {
        let nibble = u16::from(value & 0x0F);
        match addr & 0xF003 {
            reg @ 0x8000..=0x9003 => {
                let slot = usize::from((reg - 0x8000) >> 12) * 2 + usize::from((reg >> 1) & 1);
                if slot < 3 {
                    let high = reg & 1 != 0;
                    let old = self.prg[slot] & if high { 0x0F } else { 0xF0 };
                    self.prg[slot] = old | if high { nibble << 4 } else { nibble };
                }
            }
            reg @ 0xA000..=0xD003 => {
                let slot = usize::from((reg - 0xA000) >> 12) * 2 + usize::from((reg >> 1) & 1);
                let high = reg & 1 != 0;
                let old = self.chr[slot] & if high { 0x0F } else { 0xF0 };
                self.chr[slot] = old | if high { nibble << 4 } else { nibble };
            }
            reg @ 0xE000..=0xE003 => {
                let shift = u32::from(reg & 3) * 4;
                self.irq_latch = (self.irq_latch & !(0x0F << shift)) | (nibble << shift);
            }
            0xF000 => {
                self.irq_counter = self.irq_latch;
                self.irq_pending = false;
            }
            0xF001 => {
                self.irq_pending = false;
                self.irq_enabled = value & 1 != 0;
                self.irq_mask = if value & 8 != 0 {
                    0x000F
                } else if value & 4 != 0 {
                    0x00FF
                } else if value & 2 != 0 {
                    0x0FFF
                } else {
                    0xFFFF
                };
            }
            0xF002 => {
                self.mirroring = Some(match value & 3 {
                    0 => Mirroring::Horizontal,
                    1 => Mirroring::Vertical,
                    2 => Mirroring::OneScreenLower,
                    _ => Mirroring::OneScreenUpper,
                });
            }
            _ => {}
        }
        self.prg[3] = LAST;
    }

    fn irem_g101_write(&mut self, addr: u16, value: u8) {
        match addr & 0xF000 {
            0x8000 => self.regs[0] = value & 0x1F,
            0x9000 => {
                self.regs[2] = value & 1;
                self.mirroring = Some(if value & 2 != 0 {
                    Mirroring::Horizontal
                } else {
                    Mirroring::Vertical
                });
            }
            0xA000 => self.regs[1] = value & 0x1F,
            0xB000 => self.chr[usize::from(addr & 7)] = u16::from(value),
            _ => return,
        }
        let p0 = u16::from(self.regs[0]);
        let p1 = u16::from(self.regs[1]);
        self.prg = if self.regs[2] != 0 {
            [SECOND_LAST, p1, p0, LAST]
        } else {
            [p0, p1, SECOND_LAST, LAST]
        };
    }

    fn taito48_write(&mut self, addr: u16, value: u8) {
        match addr & 0xE003 {
            0x8000 => self.prg[0] = u16::from(value & 0x3F),
            0x8001 => self.prg[1] = u16::from(value & 0x3F),
            0x8002 => self.set_chr2(0, u16::from(value)),
            0x8003 => self.set_chr2(1, u16::from(value)),
            r @ 0xA000..=0xA003 => self.chr[4 + usize::from(r & 3)] = u16::from(value),
            0xC000 => self.irq_latch = u16::from(value ^ 0xFF),
            0xC001 => {
                self.irq_counter = 0;
                self.regs[7] = 1;
            }
            0xC002 => self.irq_enabled = true,
            0xC003 => {
                self.irq_enabled = false;
                self.irq_pending = false;
            }
            0xE000 => {
                self.mirroring = Some(if value & 0x40 != 0 {
                    Mirroring::Horizontal
                } else {
                    Mirroring::Vertical
                });
            }
            _ => {}
        }
        self.prg[2] = SECOND_LAST;
        self.prg[3] = LAST;
    }

    fn irem_h3001_write(&mut self, addr: u16, value: u8) {
        match addr {
            0x8000 => self.prg[0] = u16::from(value),
            0xA000 => self.prg[1] = u16::from(value),
            0xC000 => self.prg[2] = u16::from(value),
            0x9001 => {
                self.mirroring = Some(if value & 0x80 != 0 {
                    Mirroring::Horizontal
                } else {
                    Mirroring::Vertical
                });
            }
            0x9003 => {
                self.irq_enabled = value & 0x80 != 0;
                self.irq_pending = false;
            }
            0x9004 => {
                self.irq_counter = self.irq_latch;
                self.irq_pending = false;
            }
            0x9005 => self.irq_latch = (self.irq_latch & 0x00FF) | (u16::from(value) << 8),
            0x9006 => self.irq_latch = (self.irq_latch & 0xFF00) | u16::from(value),
            0xB000..=0xB007 => self.chr[usize::from(addr & 7)] = u16::from(value),
            _ => {}
        }
        self.prg[3] = LAST;
    }

    fn sunsoft3_write(&mut self, addr: u16, value: u8) {
        match addr & 0xF800 {
            0x8800 => self.set_chr2(0, u16::from(value)),
            0x9800 => self.set_chr2(1, u16::from(value)),
            0xA800 => self.set_chr2(2, u16::from(value)),
            0xB800 => self.set_chr2(3, u16::from(value)),
            0xC800 => {
                if self.regs[7] == 0 {
                    self.irq_counter = (self.irq_counter & 0x00FF) | (u16::from(value) << 8);
                    self.regs[7] = 1;
                } else {
                    self.irq_counter = (self.irq_counter & 0xFF00) | u16::from(value);
                    self.regs[7] = 0;
                }
            }
            0xD800 => {
                self.regs[7] = 0;
                self.irq_pending = false;
                self.irq_enabled = value & 0x10 != 0;
            }
            0xE800 => self.mirroring = Some(mirroring_from(value)),
            0xF800 => self.set_prg16_low(value & 0x1F),
            _ => {}
        }
    }

    fn vrc3_write(&mut self, addr: u16, value: u8) {
        let nibble = u16::from(value & 0x0F);
        match addr & 0xF000 {
            0x8000 => self.irq_latch = (self.irq_latch & 0xFFF0) | nibble,
            0x9000 => self.irq_latch = (self.irq_latch & 0xFF0F) | (nibble << 4),
            0xA000 => self.irq_latch = (self.irq_latch & 0xF0FF) | (nibble << 8),
            0xB000 => self.irq_latch = (self.irq_latch & 0x0FFF) | (nibble << 12),
            0xC000 => {
                self.irq_ack_enable = value & 1 != 0;
                self.irq_enabled = value & 2 != 0;
                self.irq_cycle_mode = value & 4 != 0; // 8-bit counter
                self.irq_pending = false;
                if self.irq_enabled {
                    self.irq_counter = self.irq_latch;
                }
            }
            0xD000 => {
                self.irq_pending = false;
                self.irq_enabled = self.irq_ack_enable;
            }
            0xF000 => self.set_prg16_low(value & 7),
            _ => {}
        }
    }

    /// Called once per rendered scanline.
    pub fn clock_scanline(&mut self) {
        if self.kind == 48 {
            // MMC3-style counter: reload on zero or after a reload request.
            if self.irq_counter == 0 || self.regs[7] != 0 {
                self.irq_counter = self.irq_latch;
                self.regs[7] = 0;
            } else {
                self.irq_counter -= 1;
            }
            if self.irq_counter == 0 && self.irq_enabled {
                self.irq_pending = true;
            }
        }
        if matches!(self.kind, 21 | 22 | 23 | 25 | 85) && self.irq_enabled && !self.irq_cycle_mode {
            self.clock_vrc_counter();
        }
    }

    fn clock_vrc_counter(&mut self) {
        if self.irq_counter & 0xFF == 0xFF {
            self.irq_counter = self.irq_latch;
            self.irq_pending = true;
        } else {
            self.irq_counter = (self.irq_counter & 0xFF) + 1;
        }
    }

    /// Called once per CPU cycle.
    pub fn clock_cpu(&mut self) {
        match self.kind {
            21 | 22 | 23 | 25 | 85 => {
                if self.irq_enabled && self.irq_cycle_mode {
                    self.clock_vrc_counter();
                }
            }
            16 | 159 => {
                if self.irq_enabled {
                    if self.irq_counter == 0 {
                        self.irq_counter = 0xFFFF;
                    } else {
                        self.irq_counter -= 1;
                        if self.irq_counter == 0 {
                            self.irq_pending = true;
                        }
                    }
                }
            }
            18 if self.irq_enabled => {
                let mask = self.irq_mask;
                let low = self.irq_counter & mask;
                if low == 0 {
                    self.irq_pending = true;
                }
                self.irq_counter = (self.irq_counter & !mask) | (low.wrapping_sub(1) & mask);
            }
            65 if self.irq_enabled && self.irq_counter > 0 => {
                self.irq_counter -= 1;
                if self.irq_counter == 0 {
                    self.irq_pending = true;
                    self.irq_enabled = false;
                }
            }
            67 if self.irq_enabled => {
                self.irq_counter = self.irq_counter.wrapping_sub(1);
                if self.irq_counter == 0xFFFF {
                    self.irq_pending = true;
                    self.irq_enabled = false;
                }
            }
            73 if self.irq_enabled => {
                if self.irq_cycle_mode {
                    if self.irq_counter & 0xFF == 0xFF {
                        self.irq_counter = (self.irq_counter & 0xFF00) | (self.irq_latch & 0xFF);
                        self.irq_pending = true;
                    } else {
                        self.irq_counter += 1;
                    }
                } else if self.irq_counter == 0xFFFF {
                    self.irq_counter = self.irq_latch;
                    self.irq_pending = true;
                } else {
                    self.irq_counter += 1;
                }
            }
            69 if self.irq_counting => {
                self.irq_counter = self.irq_counter.wrapping_sub(1);
                if self.irq_counter == 0xFFFF && self.irq_enabled {
                    self.irq_pending = true;
                }
            }
            _ => {}
        }
    }
}
