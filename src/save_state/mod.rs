//! Versioned, ROM-checked save states.
//!
//! Save states are JSON documents containing the v1 state format. The format is
//! explicitly versioned; loading a state written by a different version or for a
//! different ROM fails with an error instead of silently corrupting the machine.

use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::bus::BusState;
use crate::cpu::Cpu;
use crate::emulator::Emulator;

pub const SAVE_STATE_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveState {
    pub version: u32,
    pub rom_id: u64,
    pub frame_count: u64,
    pub created_at: u64,
    pub cpu: Cpu,
    pub bus: BusState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveStateMeta {
    pub version: u32,
    pub rom_id: u64,
    pub frame_count: u64,
    pub created_at: u64,
}

#[derive(Debug)]
pub enum SaveStateError {
    Io(String),
    Corrupt(String),
    UnsupportedVersion { found: u32, supported: u32 },
    RomMismatch { expected: u64, found: u64 },
}

impl std::fmt::Display for SaveStateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SaveStateError::Io(e) => write!(f, "save-state I/O error: {e}"),
            SaveStateError::Corrupt(e) => write!(f, "save state is corrupt: {e}"),
            SaveStateError::UnsupportedVersion { found, supported } => write!(
                f,
                "save state version {found} is not supported (this build writes version {supported})"
            ),
            SaveStateError::RomMismatch { expected, found } => write!(
                f,
                "save state belongs to a different ROM (fingerprint {found:016x}, expected {expected:016x})"
            ),
        }
    }
}

impl std::error::Error for SaveStateError {}

pub fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn serialize(emulator: &Emulator) -> String {
    let state = SaveState {
        version: SAVE_STATE_VERSION,
        rom_id: emulator.rom_id,
        frame_count: emulator.frame_count,
        created_at: now_unix(),
        cpu: emulator.cpu.clone(),
        bus: emulator.bus.save_state(),
    };
    serde_json::to_string(&state).unwrap_or_else(|_| "{}".to_string())
}

pub fn save_to_path(emulator: &Emulator, path: impl AsRef<Path>) -> Result<(), SaveStateError> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| SaveStateError::Io(e.to_string()))?;
    }
    let json = serde_json::to_string(&SaveState {
        version: SAVE_STATE_VERSION,
        rom_id: emulator.rom_id,
        frame_count: emulator.frame_count,
        created_at: now_unix(),
        cpu: emulator.cpu.clone(),
        bus: emulator.bus.save_state(),
    })
    .map_err(|e| SaveStateError::Corrupt(e.to_string()))?;
    // Write to a temporary file first so an interrupted save cannot destroy the
    // previous slot contents.
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, json).map_err(|e| SaveStateError::Io(e.to_string()))?;
    fs::rename(&tmp, path).map_err(|e| SaveStateError::Io(e.to_string()))?;
    Ok(())
}

pub fn load_into(emulator: &mut Emulator, path: impl AsRef<Path>) -> Result<(), SaveStateError> {
    let data = fs::read_to_string(path).map_err(|e| SaveStateError::Io(e.to_string()))?;
    let state: SaveState =
        serde_json::from_str(&data).map_err(|e| SaveStateError::Corrupt(e.to_string()))?;
    if state.version != SAVE_STATE_VERSION {
        return Err(SaveStateError::UnsupportedVersion {
            found: state.version,
            supported: SAVE_STATE_VERSION,
        });
    }
    if state.rom_id != emulator.rom_id {
        return Err(SaveStateError::RomMismatch {
            expected: emulator.rom_id,
            found: state.rom_id,
        });
    }
    emulator.bus.load_state(&state.bus);
    emulator.cpu = state.cpu;
    emulator.frame_count = state.frame_count;
    emulator
        .framebuffer
        .copy_from_slice(&emulator.bus.ppu.framebuffer);
    Ok(())
}

pub fn read_meta(path: impl AsRef<Path>) -> Result<SaveStateMeta, SaveStateError> {
    let data = fs::read_to_string(path).map_err(|e| SaveStateError::Io(e.to_string()))?;
    let state: SaveState =
        serde_json::from_str(&data).map_err(|e| SaveStateError::Corrupt(e.to_string()))?;
    Ok(SaveStateMeta {
        version: state.version,
        rom_id: state.rom_id,
        frame_count: state.frame_count,
        created_at: state.created_at,
    })
}
