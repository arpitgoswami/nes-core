//! CPU memory bus: RAM, PPU/APU registers, controllers and the cartridge.

use std::cell::RefCell;
use std::rc::Rc;

use serde::{Deserialize, Serialize};

use crate::apu::{Apu, ApuState, DEFAULT_SAMPLE_RATE};
use crate::cartridge::Cartridge;
use crate::controller::{Button, Controller};
use crate::cpu::CpuBus;
use crate::ppu::{Ppu, PpuState};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BusState {
    pub ram: Vec<u8>,
    pub ppu: PpuState,
    pub apu: ApuState,
    pub controllers: [Controller; 2],
    pub open_bus: u8,
}

pub struct Bus {
    pub ram: [u8; 2048],
    pub ppu: Ppu,
    pub apu: Apu,
    pub cart: Rc<RefCell<Cartridge>>,
    pub controllers: [Controller; 2],
    pub open_bus: u8,
    /// Set for cartridges with expansion audio (MMC5, VRC6).
    expansion_audio: bool,
    stall_cycles: u32,
}

impl Bus {
    pub fn new(cart: Rc<RefCell<Cartridge>>) -> Self {
        Self::with_sample_rate(cart, DEFAULT_SAMPLE_RATE)
    }

    pub fn with_sample_rate(cart: Rc<RefCell<Cartridge>>, sample_rate: u32) -> Self {
        let expansion_audio = cart.borrow().has_expansion_audio();
        Self {
            ram: [0; 2048],
            ppu: Ppu::new(cart.clone()),
            apu: Apu::new(sample_rate),
            expansion_audio,
            cart,
            controllers: [Controller::new(), Controller::new()],
            open_bus: 0,
            stall_cycles: 0,
        }
    }

    pub fn reset(&mut self) {
        self.ppu.reset();
        self.apu.reset();
        self.stall_cycles = 0;
        for controller in &mut self.controllers {
            controller.release_all();
        }
    }

    pub fn set_button(&mut self, player: usize, button: Button, pressed: bool) {
        if let Some(controller) = self.controllers.get_mut(player) {
            controller.set_button(button, pressed);
        }
    }

    fn dmc_read(&mut self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x1FFF => self.ram[usize::from(addr) & 0x07FF],
            0x4020..=0xFFFF => self.cart.borrow_mut().cpu_read(addr),
            _ => 0,
        }
    }

    /// Advances PPU/APU hardware by `cycles` CPU cycles.
    pub fn tick(&mut self, cycles: u32) {
        for _ in 0..cycles {
            if self.expansion_audio {
                let mut cart = self.cart.borrow_mut();
                cart.clock_audio();
                self.apu.expansion = cart.audio_output();
            }
            self.apu.step();
            if let Some(addr) = self.apu.take_dmc_request() {
                let sample = self.dmc_read(addr);
                self.apu.deliver_dmc_sample(sample);
            }
            for _ in 0..3 {
                self.ppu.step();
            }
        }
    }

    pub fn frame_complete(&self) -> bool {
        self.ppu.frame_complete
    }

    pub fn save_state(&self) -> BusState {
        BusState {
            ram: self.ram.to_vec(),
            ppu: self.ppu.save_state(),
            apu: self.apu.save_state(),
            controllers: self.controllers.clone(),
            open_bus: self.open_bus,
        }
    }

    pub fn load_state(&mut self, state: &BusState) {
        for (dst, src) in self.ram.iter_mut().zip(state.ram.iter()) {
            *dst = *src;
        }
        self.ppu.load_state(&state.ppu);
        self.apu.load_state(&state.apu);
        self.controllers = state.controllers.clone();
        self.open_bus = state.open_bus;
        self.stall_cycles = 0;
    }
}

impl CpuBus for Bus {
    fn read(&mut self, addr: u16) -> u8 {
        let value = match addr {
            0x0000..=0x1FFF => self.ram[usize::from(addr) & 0x07FF],
            0x2000..=0x3FFF => self.ppu.read_register(addr & 0x0007),
            0x4015 => self.apu.read_status(),
            0x4016 => self.controllers[0].read(),
            0x4017 => self.controllers[1].read(),
            0x4000..=0x4014 | 0x4018..=0x401F => self.open_bus,
            0x4020..=0xFFFF => self.cart.borrow_mut().cpu_read(addr),
        };
        self.open_bus = value;
        value
    }

    fn write(&mut self, addr: u16, value: u8) {
        self.open_bus = value;
        match addr {
            0x0000..=0x1FFF => self.ram[usize::from(addr) & 0x07FF] = value,
            0x2000..=0x3FFF => self.ppu.write_register(addr & 0x0007, value),
            0x4000..=0x4013 | 0x4015 | 0x4017 => self.apu.write_register(addr, value),
            0x4014 => {
                let base = u16::from(value) << 8;
                let oam_addr = self.ppu.state.oam_addr;
                let mut oam = [0u8; 256];
                for (i, slot) in oam.iter_mut().enumerate() {
                    *slot = self.read(base + i as u16);
                }
                for (i, byte) in oam.iter().enumerate() {
                    self.ppu.state.oam[(usize::from(oam_addr) + i) & 0xFF] = *byte;
                }
                // 513 cycles for aligned starts, 514 otherwise.
                self.stall_cycles += 513;
            }
            0x4016 => {
                for controller in &mut self.controllers {
                    controller.write(value);
                }
            }
            0x4018..=0x401F => {}
            0x4020..=0xFFFF => self.cart.borrow_mut().cpu_write(addr, value),
        }
    }

    fn poll_nmi(&mut self) -> bool {
        self.ppu.take_nmi()
    }

    fn poll_irq(&self) -> bool {
        self.apu.irq_pending() || self.cart.borrow().irq_pending()
    }

    fn take_stall_cycles(&mut self) -> u32 {
        std::mem::take(&mut self.stall_cycles)
    }
}
