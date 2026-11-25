//! Achievements that NES Core actually tracks.
//!
//! Only generic, verified events are listed. There are no game-specific
//! achievements because NES Core does not identify individual titles.

pub struct AchievementDef {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
}

pub const ACHIEVEMENTS: [AchievementDef; 6] = [
    AchievementDef {
        id: "first_boot",
        name: "Power on",
        description: "Load a ROM and boot it for the first time.",
    },
    AchievementDef {
        id: "frames_1000",
        name: "Warmed up",
        description: "Emulate 1,000 frames in one session.",
    },
    AchievementDef {
        id: "first_save",
        name: "Checkpoint",
        description: "Save a save-state slot.",
    },
    AchievementDef {
        id: "first_load",
        name: "Rewind",
        description: "Load a save-state slot.",
    },
    AchievementDef {
        id: "library_three",
        name: "Collector",
        description: "Keep three ROMs in the library.",
    },
    AchievementDef {
        id: "play_ten_minutes",
        name: "Long session",
        description: "Run the emulator for ten minutes in total.",
    },
];
