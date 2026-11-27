//! Picture Processing Unit.
//!
//! The PPU is advanced dot by dot, but background and sprites are composed for
//! a whole scanline when a visible line starts. Register writes from the CPU
//! change scrolling and palettes for subsequent lines, which covers the common
//! raster effects used by NES software. Per-dot mid-scanline effects are not
//! emulated; see `docs/architecture.md` for the exact limitations.

pub mod palette;

use std::cell::RefCell;
use std::rc::Rc;

use serde::{Deserialize, Serialize};

use crate::cartridge::Cartridge;
use palette::PALETTE_RGB;

pub const SCREEN_WIDTH: usize = 256;
pub const SCREEN_HEIGHT: usize = 240;

const STATUS_VBLANK: u8 = 0x80;
const STATUS_SPRITE0: u8 = 0x40;
const STATUS_OVERFLOW: u8 = 0x20;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PpuState {
    pub ctrl: u8,
    pub mask: u8,
    pub status: u8,
    pub oam_addr: u8,
    pub v: u16,
    pub t: u16,
    pub fine_x: u8,
    pub write_toggle: bool,
    pub read_buffer: u8,
    pub io_latch: u8,
    pub oam: Vec<u8>,
    pub vram: Vec<u8>,
    pub palette: Vec<u8>,
    pub scanline: u16,
    pub dot: u16,
    pub frame: u64,
    pub odd_frame: bool,
}

pub struct Ppu {
    pub state: PpuState,
    pub framebuffer: Vec<u8>,
    pub nmi_pending: bool,
    pub frame_complete: bool,
    pub cart: Rc<RefCell<Cartridge>>,
    pub sprites_on_line: u8,
}

impl Ppu {
    pub fn new(cart: Rc<RefCell<Cartridge>>) -> Self {
        Self {
            state: PpuState {
                ctrl: 0,
                mask: 0,
                status: 0,
                oam_addr: 0,
                v: 0,
                t: 0,
                fine_x: 0,
                write_toggle: false,
                read_buffer: 0,
                io_latch: 0,
                oam: vec![0; 256],
                vram: vec![0; 4096],
                palette: vec![0; 32],
                scanline: 261,
                dot: 0,
                frame: 0,
                odd_frame: false,
            },
            framebuffer: vec![0; SCREEN_WIDTH * SCREEN_HEIGHT * 4],
            nmi_pending: false,
            frame_complete: false,
            cart,
            sprites_on_line: 0,
        }
    }

    pub fn reset(&mut self) {
        let frame = self.state.frame;
        self.state.ctrl = 0;
        self.state.mask = 0;
        self.state.status = 0;
        self.state.scanline = 261;
        self.state.dot = 0;
        self.state.write_toggle = false;
        self.state.v = 0;
        self.state.t = 0;
        self.state.fine_x = 0;
        self.state.read_buffer = 0;
        self.state.frame = frame;
        self.nmi_pending = false;
        self.frame_complete = false;
    }

    pub fn rendering_enabled(&self) -> bool {
        self.state.mask & 0x18 != 0
    }

    pub fn take_nmi(&mut self) -> bool {
        std::mem::take(&mut self.nmi_pending)
    }

    pub fn status(&self) -> u8 {
        self.state.status
    }

    fn nametable_read(&self, addr: u16) -> u8 {
        let index = self.cart.borrow().nametable_index(addr);
        self.state.vram[index]
    }

    fn nametable_write(&mut self, addr: u16, value: u8) {
        let index = self.cart.borrow().nametable_index(addr);
        self.state.vram[index] = value;
    }

    fn palette_read(&self, addr: u16) -> u8 {
        let mut index = usize::from(addr & 0x1F);
        if index >= 16 && index % 4 == 0 {
            index -= 16;
        }
        self.state.palette[index]
    }

    fn palette_write(&mut self, addr: u16, value: u8) {
        let mut index = usize::from(addr & 0x1F);
        if index >= 16 && index % 4 == 0 {
            index -= 16;
        }
        self.state.palette[index] = value & 0x3F;
    }

    pub fn read_register(&mut self, reg: u16) -> u8 {
        match reg & 7 {
            2 => {
                let value = (self.state.status & 0xE0) | (self.state.io_latch & 0x1F);
                self.state.status &= !STATUS_VBLANK;
                self.nmi_pending = false;
                self.state.write_toggle = false;
                self.state.io_latch = value;
                value
            }
            4 => {
                let value = self.state.oam[usize::from(self.state.oam_addr)];
                self.state.io_latch = value;
                value
            }
            7 => {
                let addr = self.state.v & 0x3FFF;
                let increment = if self.state.ctrl & 0x04 != 0 { 32 } else { 1 };
                let value = if addr >= 0x3F00 {
                    let value = self.palette_read(addr);
                    self.state.read_buffer = self.nametable_read(addr - 0x1000);
                    value
                } else {
                    let buffered = self.state.read_buffer;
                    self.state.read_buffer = if addr < 0x2000 {
                        self.cart.borrow().chr_read(addr)
                    } else {
                        self.nametable_read(addr)
                    };
                    buffered
                };
                self.state.v = self.state.v.wrapping_add(increment) & 0x7FFF;
                self.state.io_latch = value;
                value
            }
            _ => self.state.io_latch,
        }
    }

    pub fn write_register(&mut self, reg: u16, value: u8) {
        self.state.io_latch = value;
        match reg & 7 {
            0 => {
                let old_nmi = self.state.ctrl & 0x80 != 0;
                self.state.ctrl = value;
                self.state.t = (self.state.t & !0x0C00) | (u16::from(value & 0x03) << 10);
                if !old_nmi && value & 0x80 != 0 && self.state.status & STATUS_VBLANK != 0 {
                    self.nmi_pending = true;
                }
            }
            1 => self.state.mask = value,
            3 => self.state.oam_addr = value,
            4 => {
                self.state.oam[usize::from(self.state.oam_addr)] = value;
                self.state.oam_addr = self.state.oam_addr.wrapping_add(1);
            }
            5 => {
                if !self.state.write_toggle {
                    self.state.t = (self.state.t & !0x001F) | (u16::from(value) >> 3);
                    self.state.fine_x = value & 7;
                    self.state.write_toggle = true;
                } else {
                    self.state.t = (self.state.t & !0x73E0)
                        | ((u16::from(value) & 0x07) << 12)
                        | ((u16::from(value) & 0xF8) << 2);
                    self.state.write_toggle = false;
                }
            }
            6 => {
                if !self.state.write_toggle {
                    self.state.t = (self.state.t & 0x00FF) | ((u16::from(value) & 0x3F) << 8);
                    self.state.write_toggle = true;
                } else {
                    self.state.t = (self.state.t & 0xFF00) | u16::from(value);
                    self.state.v = self.state.t;
                    self.state.write_toggle = false;
                }
            }
            7 => {
                let addr = self.state.v & 0x3FFF;
                let increment = if self.state.ctrl & 0x04 != 0 { 32 } else { 1 };
                if addr < 0x2000 {
                    self.cart.borrow_mut().chr_write(addr, value);
                } else if addr < 0x3F00 {
                    let mirrored = if addr >= 0x3000 { addr - 0x1000 } else { addr };
                    self.nametable_write(mirrored, value);
                } else {
                    self.palette_write(addr, value);
                }
                self.state.v = self.state.v.wrapping_add(increment) & 0x7FFF;
            }
            _ => {}
        }
    }

    /// Advances the PPU by one dot (~1/3 CPU cycle).
    pub fn step(&mut self) {
        let scanline = self.state.scanline;
        let dot = self.state.dot;

        if scanline == 241 && dot == 1 {
            self.state.status |= STATUS_VBLANK;
            if self.state.ctrl & 0x80 != 0 {
                self.nmi_pending = true;
            }
            self.frame_complete = true;
        }

        if scanline == 261 && dot == 1 {
            self.state.status &= !(STATUS_VBLANK | STATUS_SPRITE0 | STATUS_OVERFLOW);
        }

        if scanline < 240 && dot == 0 {
            self.render_scanline(scanline as usize);
            self.increment_y();
        }

        if self.rendering_enabled() && (scanline < 240 || scanline == 261) && dot == 260 {
            self.cart.borrow_mut().on_scanline();
        }

        self.state.dot += 1;
        if self.state.dot > 340 {
            self.state.dot = 0;
            self.state.scanline += 1;
            if self.state.scanline > 261 {
                self.state.scanline = 0;
                self.state.frame += 1;
            }
        }
    }

    fn increment_y(&mut self) {
        let s = &mut self.state;
        if s.v & 0x7000 != 0x7000 {
            s.v = s.v.wrapping_add(0x1000);
        } else {
            s.v &= !0x7000;
            let mut coarse_y = (s.v & 0x03E0) >> 5;
            if coarse_y == 29 {
                coarse_y = 0;
                s.v ^= 0x0800;
            } else if coarse_y == 31 {
                coarse_y = 0;
            } else {
                coarse_y += 1;
            }
            s.v = (s.v & !0x03E0) | (coarse_y << 5);
        }
    }

    fn color(&self, palette_index: u8) -> [u8; 4] {
        let mut index = usize::from(palette_index & 0x3F);
        if self.state.mask & 0x01 != 0 {
            index &= 0x30;
        }
        let (mut r, mut g, mut b) = PALETTE_RGB[index];
        let emphasis = self.state.mask & 0xE0;
        if emphasis != 0 {
            let active = emphasis.count_ones();
            if emphasis & 0x20 != 0 {
                r = r.saturating_add(32);
            } else if active >= 2 {
                r = r.saturating_sub(24);
            }
            if emphasis & 0x40 != 0 {
                g = g.saturating_add(32);
            } else if active >= 2 {
                g = g.saturating_sub(24);
            }
            if emphasis & 0x80 != 0 {
                b = b.saturating_add(32);
            } else if active >= 2 {
                b = b.saturating_sub(24);
            }
        }
        [r, g, b, 255]
    }

    fn render_scanline(&mut self, scanline: usize) {
        let bg_enabled = self.state.mask & 0x08 != 0;
        let sp_enabled = self.state.mask & 0x10 != 0;
        let left_bg = self.state.mask & 0x02 != 0;
        let left_sp = self.state.mask & 0x04 != 0;
        let backdrop = self.color(self.state.palette[0]);

        let mut bg_pixels = [0u8; SCREEN_WIDTH];
        let mut bg_color_index = [0u8; SCREEN_WIDTH];

        // MMC5 substitutes background data while PPUMASK's rendering bits are on.
        let mmc5 = if bg_enabled && self.state.mask & 0x18 != 0 {
            self.cart.borrow().mmc5_render()
        } else {
            None
        };

        if bg_enabled {
            let base_nt = (self.state.v & 0x0C00) >> 10;
            let coarse_y = ((self.state.v & 0x03E0) >> 5) as usize;
            let fine_y = ((self.state.v & 0x7000) >> 12) as usize;
            let coarse_x_start = (self.state.v & 0x001F) as usize;
            let pattern_table = if self.state.ctrl & 0x10 != 0 {
                0x1000
            } else {
                0
            };
            for x in 0..SCREEN_WIDTH {
                if x < 8 && !left_bg {
                    continue;
                }
                let scrolled = x + usize::from(self.state.fine_x);
                let tile_x = coarse_x_start + scrolled / 8;
                let nt_x = tile_x / 32;
                let tile_x = tile_x % 32;
                let nt = base_nt ^ (nt_x as u16);
                let nt_addr = 0x2000 + nt * 0x400 + (coarse_y as u16) * 32 + tile_x as u16;
                let mut tile = self.nametable_read(nt_addr);
                let attr_addr =
                    0x2000 + nt * 0x400 + 0x3C0 + ((coarse_y / 4) as u16) * 8 + (tile_x / 4) as u16;
                let attribute = self.nametable_read(attr_addr);
                let shift = ((coarse_y % 4) / 2) * 4 + ((tile_x % 4) / 2) * 2;
                let mut palette = (attribute >> shift) & 0x03;
                let mut pattern_base = pattern_table + u16::from(tile) * 16;
                let mut row = fine_y;
                let mut direct = false;
                if let Some(mmc5) = mmc5.as_ref() {
                    let screen_tile = scrolled / 8;
                    if mmc5.split_region(screen_tile) {
                        let (split_tile, split_palette) = mmc5.split_tile(scanline, screen_tile);
                        tile = split_tile;
                        palette = split_palette;
                        pattern_base = mmc5.split_pattern_base(tile);
                        row = mmc5.split_fine_row(scanline);
                        direct = true;
                    } else if mmc5.extended_attributes {
                        let (extended_palette, bank) = mmc5.extended_attribute(coarse_y, tile_x);
                        palette = extended_palette;
                        pattern_base = bank * 0x1000 + u16::from(tile) * 16;
                        direct = true;
                    }
                }
                let pattern_addr = pattern_base + row as u16;
                let cart = self.cart.borrow();
                let (low, high) = if direct {
                    (
                        cart.chr_read_direct(pattern_addr),
                        cart.chr_read_direct(pattern_addr + 8),
                    )
                } else {
                    (cart.chr_read(pattern_addr), cart.chr_read(pattern_addr + 8))
                };
                let bit = 7 - (scrolled % 8);
                let pixel = ((low >> bit) & 1) | (((high >> bit) & 1) << 1);
                bg_color_index[x] = pixel;
                if pixel != 0 {
                    let color_index =
                        self.state.palette[usize::from(palette) * 4 + usize::from(pixel)];
                    bg_pixels[x] = color_index;
                }
            }
        }

        let mut sprite_pixels: [Option<(u8, bool, bool)>; SCREEN_WIDTH] = [None; SCREEN_WIDTH];
        self.sprites_on_line = 0;
        if sp_enabled {
            let height: usize = if self.state.ctrl & 0x20 != 0 { 16 } else { 8 };
            let mut found = 0usize;
            let mut overflow = false;
            for sprite in 0..64usize {
                let base = sprite * 4;
                let y = self.state.oam[base];
                let row = scanline as i32 - (i32::from(y) + 1);
                if row < 0 || row as usize >= height {
                    continue;
                }
                if scanline == 0 && y >= 0xEF {
                    continue;
                }
                found += 1;
                if found > 8 {
                    overflow = true;
                    break;
                }
                let tile = self.state.oam[base + 1];
                let attr = self.state.oam[base + 2];
                let x_pos = self.state.oam[base + 3];
                let flip_v = attr & 0x80 != 0;
                let flip_h = attr & 0x40 != 0;
                let mut row = row as usize;
                if flip_v {
                    row = height - 1 - row;
                }
                let (pattern_table, tile_index) = if height == 16 {
                    (
                        if tile & 1 != 0 { 0x1000 } else { 0 },
                        if row < 8 { tile & 0xFE } else { tile | 1 },
                    )
                } else {
                    (
                        if self.state.ctrl & 0x08 != 0 {
                            0x1000
                        } else {
                            0
                        },
                        tile,
                    )
                };
                let row_in_tile = row % 8;
                let addr = pattern_table + u16::from(tile_index) * 16 + row_in_tile as u16;
                let low = self.cart.borrow().chr_read_sprite(addr);
                let high = self.cart.borrow().chr_read_sprite(addr + 8);
                let palette = 0x10 + (attr & 0x03) * 4;
                let behind_bg = attr & 0x20 != 0;
                for col in 0..8usize {
                    let bit = if flip_h { col } else { 7 - col };
                    let pixel = ((low >> bit) & 1) | (((high >> bit) & 1) << 1);
                    if pixel == 0 {
                        continue;
                    }
                    let x = usize::from(x_pos) + col;
                    if x >= SCREEN_WIDTH {
                        continue;
                    }
                    if x < 8 && !left_sp {
                        continue;
                    }
                    if sprite_pixels[x].is_none() {
                        let color = self.state.palette[usize::from(palette + pixel)];
                        sprite_pixels[x] = Some((color, behind_bg, sprite == 0));
                    }
                }
            }
            if overflow && self.state.scanline < 240 {
                self.state.status |= STATUS_OVERFLOW;
            }
        }

        for x in 0..SCREEN_WIDTH {
            let mut color = if bg_color_index[x] == 0 {
                backdrop
            } else {
                self.color(bg_pixels[x])
            };
            if let Some((sprite_color, behind_bg, sprite_zero)) = sprite_pixels[x] {
                if sprite_zero && bg_color_index[x] != 0 && x != 255 && bg_enabled && sp_enabled {
                    self.state.status |= STATUS_SPRITE0;
                }
                if !behind_bg || bg_color_index[x] == 0 {
                    color = self.color(sprite_color);
                }
            }
            let offset = (scanline * SCREEN_WIDTH + x) * 4;
            self.framebuffer[offset..offset + 4].copy_from_slice(&color);
        }
    }

    pub fn save_state(&self) -> PpuState {
        self.state.clone()
    }

    pub fn load_state(&mut self, state: &PpuState) {
        self.state = state.clone();
        if self.state.oam.len() != 256 {
            self.state.oam.resize(256, 0);
        }
        if self.state.vram.len() != 4096 {
            self.state.vram.resize(4096, 0);
        }
        if self.state.palette.len() != 32 {
            self.state.palette.resize(32, 0);
        }
        self.nmi_pending = false;
        self.frame_complete = false;
    }
}
