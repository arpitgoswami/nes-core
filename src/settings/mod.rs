//! Persisted user settings, ROM library and achievement records.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::controller::Button;

/// Application data directory, e.g. `%APPDATA%\nes-core` on Windows.
pub fn config_dir() -> Option<PathBuf> {
    let base = if cfg!(target_os = "windows") {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Application Support"))
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
    };
    base.map(|b| b.join("nes-core"))
}

pub fn state_dir() -> Option<PathBuf> {
    config_dir().map(|d| d.join("savestates"))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryEntry {
    pub path: PathBuf,
    pub title: String,
    pub mapper: u16,
    pub mapper_name: String,
    pub rom_id: u64,
    pub last_played: Option<u64>,
    pub play_seconds: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AchievementRecord {
    pub id: String,
    pub unlocked_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub volume: f32,
    pub muted: bool,
    /// Display the NES 8:7 pixel aspect rather than square pixels.
    pub pixel_aspect: bool,
    pub integer_scaling: bool,
    pub vsync: bool,
    pub fullscreen: bool,
    pub show_fps: bool,
    /// Keyboard bindings keyed by controller button, using egui key names.
    pub keymap: BTreeMap<String, String>,
    pub library: Vec<LibraryEntry>,
    pub achievements: Vec<AchievementRecord>,
    pub total_play_seconds: f64,
    pub last_rom: Option<PathBuf>,
}

impl Default for Settings {
    fn default() -> Self {
        let mut keymap = BTreeMap::new();
        keymap.insert("A".into(), "X".into());
        keymap.insert("B".into(), "Z".into());
        keymap.insert("Select".into(), "A".into());
        keymap.insert("Start".into(), "S".into());
        keymap.insert("Up".into(), "ArrowUp".into());
        keymap.insert("Down".into(), "ArrowDown".into());
        keymap.insert("Left".into(), "ArrowLeft".into());
        keymap.insert("Right".into(), "ArrowRight".into());
        Self {
            volume: 0.7,
            muted: false,
            pixel_aspect: true,
            integer_scaling: false,
            vsync: true,
            fullscreen: false,
            show_fps: true,
            keymap,
            library: Vec::new(),
            achievements: Vec::new(),
            total_play_seconds: 0.0,
            last_rom: None,
        }
    }
}

impl Settings {
    pub fn key_name(&self, button: Button) -> Option<&String> {
        self.keymap.get(button.label())
    }

    pub fn set_key(&mut self, button: Button, key_name: impl Into<String>) {
        self.keymap.insert(button.label().into(), key_name.into());
    }

    pub fn path() -> Option<PathBuf> {
        config_dir().map(|d| d.join("settings.json"))
    }

    pub fn load() -> Self {
        let Some(path) = Self::path() else {
            return Self::default();
        };
        let Ok(text) = fs::read_to_string(path) else {
            return Self::default();
        };
        serde_json::from_str(&text).unwrap_or_default()
    }

    pub fn save(&self) -> Result<(), String> {
        let Some(path) = Self::path() else {
            return Err("no writable configuration directory".into());
        };
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let json = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, json).map_err(|e| e.to_string())?;
        fs::rename(&tmp, &path).map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn add_to_library(&mut self, path: &Path, mapper: u16, mapper_name: &str, rom_id: u64) {
        if let Some(entry) = self.library.iter_mut().find(|e| e.path == path) {
            entry.mapper = mapper;
            entry.mapper_name = mapper_name.to_string();
            entry.rom_id = rom_id;
            return;
        }
        let title = path
            .file_stem()
            .map(|s| s.to_string_lossy().trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "Untitled ROM".into());
        self.library.push(LibraryEntry {
            path: path.to_path_buf(),
            title,
            mapper,
            mapper_name: mapper_name.to_string(),
            rom_id,
            last_played: None,
            play_seconds: 0.0,
        });
    }

    pub fn is_unlocked(&self, id: &str) -> bool {
        self.achievements.iter().any(|a| a.id == id)
    }

    pub fn unlock(&mut self, id: &str) -> bool {
        if self.is_unlocked(id) {
            return false;
        }
        self.achievements.push(AchievementRecord {
            id: id.to_string(),
            unlocked_at: crate::save_state::now_unix(),
        });
        true
    }
}
