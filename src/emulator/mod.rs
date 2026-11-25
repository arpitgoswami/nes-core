//! Joins the CPU, bus, PPU and APU into a runnable machine.

use std::cell::RefCell;
use std::rc::Rc;

use crate::apu::DEFAULT_SAMPLE_RATE;
use crate::bus::Bus;
use crate::cartridge::Cartridge;
use crate::controller::Button;
use crate::cpu::Cpu;

pub const FRAME_RATE: f64 = 60.098_813;

/// Maximum CPU cycles executed for one frame before bailing out. This guards
/// against a jammed CPU or a ROM that disables NMI and never enables rendering.
const MAX_CYCLES_PER_FRAME: u32 = 400_000;

pub struct Emulator {
    pub cpu: Cpu,
    pub bus: Bus,
    pub frame_count: u64,
    pub rom_id: u64,
    pub framebuffer: Vec<u8>,
    pub cycles_this_frame: u32,
}

impl Emulator {
    pub fn new(cart: Cartridge) -> Self {
        Self::with_sample_rate(cart, DEFAULT_SAMPLE_RATE)
    }

    pub fn with_sample_rate(cart: Cartridge, sample_rate: u32) -> Self {
        let rom_id = cart.fingerprint();
        let cart = Rc::new(RefCell::new(cart));
        let mut bus = Bus::with_sample_rate(cart, sample_rate);
        let mut cpu = Cpu::new();
        cpu.reset(&mut bus);
        Self {
            cpu,
            bus,
            frame_count: 0,
            rom_id,
            framebuffer: vec![0; crate::ppu::SCREEN_WIDTH * crate::ppu::SCREEN_HEIGHT * 4],
            cycles_this_frame: 0,
        }
    }

    pub fn reset(&mut self) {
        self.cpu.reset(&mut self.bus);
        self.bus.reset();
        self.frame_count = 0;
    }

    pub fn set_button(&mut self, player: usize, button: Button, pressed: bool) {
        self.bus.set_button(player, button, pressed);
    }

    /// Runs one full video frame. Returns false if the core hit its cycle guard.
    pub fn run_frame(&mut self) -> bool {
        self.bus.ppu.frame_complete = false;
        let mut cycles = 0u32;
        while !self.bus.ppu.frame_complete {
            let spent = self.cpu.step(&mut self.bus);
            self.bus.tick(spent);
            cycles = cycles.saturating_add(spent);
            if cycles >= MAX_CYCLES_PER_FRAME {
                self.cycles_this_frame = cycles;
                return false;
            }
        }
        self.cycles_this_frame = cycles;
        self.frame_count += 1;
        self.framebuffer.copy_from_slice(&self.bus.ppu.framebuffer);
        true
    }

    pub fn drain_audio(&mut self, out: &mut Vec<f32>) {
        self.bus.apu.drain_samples(out);
    }

    pub fn set_volume(&mut self, volume: f32) {
        self.bus.apu.volume = volume.clamp(0.0, 1.0);
    }

    pub fn set_muted(&mut self, muted: bool) {
        self.bus.apu.muted = muted;
    }

    pub fn framebuffer(&self) -> &[u8] {
        &self.framebuffer
    }

    pub fn title(&self) -> String {
        self.bus.cart.borrow().title()
    }

    pub fn mapper_name(&self) -> &'static str {
        self.bus.cart.borrow().mapper.name()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct EmulatorConfig {
    pub sample_rate: u32,
}

impl Default for EmulatorConfig {
    fn default() -> Self {
        Self {
            sample_rate: DEFAULT_SAMPLE_RATE,
        }
    }
}
