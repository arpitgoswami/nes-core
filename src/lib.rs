//! NES Core: a cycle-accounted NES/Famicom emulator core.
//!
//! The crate is split into a hardware core (CPU, PPU, APU, cartridge and bus),
//! a thin emulator front end that joins them, and desktop-only modules that are
//! used by the binary. Everything below `emulator` is independent of the GUI and
//! is covered by the integration tests in `tests/`.

pub mod apu;
pub mod bus;
pub mod cartridge;
pub mod controller;
pub mod cpu;
pub mod emulator;
pub mod ppu;
pub mod save_state;
pub mod settings;

pub use cartridge::RomError;
pub use emulator::{Emulator, EmulatorConfig};
