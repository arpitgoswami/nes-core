//! Standard NES controller shift register.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Button {
    A,
    B,
    Select,
    Start,
    Up,
    Down,
    Left,
    Right,
}

impl Button {
    pub const ALL: [Button; 8] = [
        Button::A,
        Button::B,
        Button::Select,
        Button::Start,
        Button::Up,
        Button::Down,
        Button::Left,
        Button::Right,
    ];

    /// Bit in the controller's serial output order (A first).
    pub fn bit(self) -> u8 {
        match self {
            Button::A => 0x01,
            Button::B => 0x02,
            Button::Select => 0x04,
            Button::Start => 0x08,
            Button::Up => 0x10,
            Button::Down => 0x20,
            Button::Left => 0x40,
            Button::Right => 0x80,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Button::A => "A",
            Button::B => "B",
            Button::Select => "Select",
            Button::Start => "Start",
            Button::Up => "Up",
            Button::Down => "Down",
            Button::Left => "Left",
            Button::Right => "Right",
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Controller {
    pub buttons: u8,
    strobe: bool,
    shift: u8,
}

impl Controller {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_button(&mut self, button: Button, pressed: bool) {
        if pressed {
            self.buttons |= button.bit();
        } else {
            self.buttons &= !button.bit();
        }
    }

    pub fn release_all(&mut self) {
        self.buttons = 0;
    }

    pub fn write(&mut self, value: u8) {
        self.strobe = value & 1 != 0;
        if self.strobe {
            self.shift = self.buttons;
        }
    }

    pub fn read(&mut self) -> u8 {
        if self.strobe {
            return self.buttons & 1;
        }
        let value = self.shift & 1;
        self.shift = (self.shift >> 1) | 0x80;
        value
    }
}
