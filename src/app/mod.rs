//! NES Core desktop front end: library, game screen, settings and save states.

mod achievements;
mod artwork;
mod theme;

use std::path::PathBuf;
use std::time::{Duration, Instant};

use eframe::egui;
use egui::{Align, Align2, Color32, FontId, Layout, RichText, Sense, Stroke, Vec2};

use nes_core::cartridge::Cartridge;
use nes_core::controller::Button;
use nes_core::emulator::{Emulator, FRAME_RATE};
use nes_core::save_state;
use nes_core::settings::{config_dir, state_dir, LibraryEntry, Settings};

use crate::audio::{AudioOutput, SAMPLE_RATE};
use achievements::ACHIEVEMENTS;
use artwork::ArtworkCache;
use theme::*;

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum View {
    Library,
    LoadRom,
    Settings,
    SaveStates,
    Achievements,
    About,
    Game,
}

impl View {
    fn label(self) -> &'static str {
        match self {
            View::Library => "Library",
            View::LoadRom => "Load ROM",
            View::Settings => "Settings",
            View::SaveStates => "Save States",
            View::Achievements => "Achievements",
            View::About => "About",
            View::Game => "Now Playing",
        }
    }
}

struct StatusMessage {
    text: String,
    error: bool,
    shown_at: Instant,
}

struct Session {
    emulator: Emulator,
    path: PathBuf,
    title: String,
    mapper: String,
    paused: bool,
    texture: Option<egui::TextureHandle>,
    last_tick: Instant,
    accumulator: f64,
    play_seconds: f64,
    fps: f32,
    fps_frames: u32,
    fps_since: Instant,
    audio: Vec<f32>,
    fast_forward: bool,
}

enum CardAction {
    None,
    Play,
    Remove,
}

pub struct NesApp {
    view: View,
    settings: Settings,
    session: Option<Session>,
    audio: AudioOutput,
    art: ArtworkCache,
    status: Option<StatusMessage>,
    pending_bind: Option<Button>,
    settings_dirty: bool,
    last_settings_save: Instant,
}

impl NesApp {
    pub fn new(cc: &eframe::CreationContext<'_>, settings: Settings) -> Self {
        theme::apply(&cc.egui_ctx);
        let audio = AudioOutput::new(SAMPLE_RATE);
        let status = audio.error.as_ref().map(|message| StatusMessage {
            text: format!("{message} — the emulator will run silently."),
            error: true,
            shown_at: Instant::now(),
        });
        Self {
            view: View::Library,
            settings,
            session: None,
            audio,
            art: ArtworkCache::new(),
            status,
            pending_bind: None,
            settings_dirty: false,
            last_settings_save: Instant::now(),
        }
    }

    fn unlock(&mut self, id: &str) {
        if self.settings.unlock(id) {
            if let Some(definition) = ACHIEVEMENTS.iter().find(|a| a.id == id) {
                self.status = Some(StatusMessage {
                    text: format!("Achievement unlocked: {}", definition.name),
                    error: false,
                    shown_at: Instant::now(),
                });
            }
            self.settings_dirty = true;
        }
    }

    fn save_settings(&mut self) {
        match self.settings.save() {
            Ok(()) => {
                self.settings_dirty = false;
                self.last_settings_save = Instant::now();
            }
            Err(error) => {
                self.status = Some(StatusMessage {
                    text: format!("Could not save settings: {error}"),
                    error: true,
                    shown_at: Instant::now(),
                });
            }
        }
    }

    fn open_rom_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("NES ROM", &["nes"])
            .set_title("Open a NES ROM")
            .pick_file()
        {
            self.open_rom(path);
        }
    }

    fn open_rom(&mut self, path: PathBuf) {
        match Cartridge::load(&path) {
            Ok(cart) => {
                let rom_id = cart.fingerprint();
                let mapper = cart.header.mapper;
                let mapper_name = cart.mapper.name().to_string();
                let title = cart.title();
                self.settings
                    .add_to_library(&path, mapper, &mapper_name, rom_id);
                self.settings.last_rom = Some(path.clone());
                if self.settings.library.len() >= 3 {
                    self.unlock("library_three");
                }

                let mut emulator = Emulator::with_sample_rate(cart, SAMPLE_RATE);
                emulator.set_volume(1.0);
                emulator.set_muted(false);
                self.audio.clear();
                self.session = Some(Session {
                    emulator,
                    path,
                    title,
                    mapper: mapper_name,
                    paused: false,
                    texture: None,
                    last_tick: Instant::now(),
                    accumulator: 0.0,
                    play_seconds: 0.0,
                    fps: 0.0,
                    fps_frames: 0,
                    fps_since: Instant::now(),
                    audio: Vec::new(),
                    fast_forward: false,
                });
                self.view = View::Game;
                self.unlock("first_boot");
                self.settings_dirty = true;
                self.save_settings();
            }
            Err(error) => {
                self.status = Some(StatusMessage {
                    text: format!("Could not open {}: {error}", path.display()),
                    error: true,
                    shown_at: Instant::now(),
                });
                self.view = View::LoadRom;
            }
        }
    }

    fn slot_path(&self, slot: u8) -> Option<PathBuf> {
        let session = self.session.as_ref()?;
        state_dir().map(|dir| {
            dir.join(format!("{:016x}", session.emulator.rom_id))
                .join(format!("slot-{slot}.json"))
        })
    }

    fn save_slot(&mut self, slot: u8) {
        let Some(path) = self.slot_path(slot) else {
            return;
        };
        let Some(session) = self.session.as_ref() else {
            return;
        };
        match save_state::save_to_path(&session.emulator, &path) {
            Ok(()) => {
                self.unlock("first_save");
                self.status = Some(StatusMessage {
                    text: format!("Saved state slot {slot}."),
                    error: false,
                    shown_at: Instant::now(),
                });
            }
            Err(error) => {
                self.status = Some(StatusMessage {
                    text: format!("Could not save state: {error}"),
                    error: true,
                    shown_at: Instant::now(),
                });
            }
        }
    }

    fn load_slot(&mut self, slot: u8) {
        let Some(path) = self.slot_path(slot) else {
            return;
        };
        let Some(mut session) = self.session.take() else {
            return;
        };
        let result = save_state::load_into(&mut session.emulator, &path);
        session.accumulator = 0.0;
        session.last_tick = Instant::now();
        self.audio.clear();
        match result {
            Ok(()) => {
                self.status = Some(StatusMessage {
                    text: format!("Loaded state slot {slot}."),
                    error: false,
                    shown_at: Instant::now(),
                });
                self.session = Some(session);
                self.unlock("first_load");
            }
            Err(error) => {
                self.status = Some(StatusMessage {
                    text: format!("Could not load state: {error}"),
                    error: true,
                    shown_at: Instant::now(),
                });
                self.session = Some(session);
            }
        }
    }

    fn reset_console(&mut self) {
        if let Some(session) = self.session.as_mut() {
            session.emulator.reset();
            session.accumulator = 0.0;
            session.last_tick = Instant::now();
            self.audio.clear();
            self.status = Some(StatusMessage {
                text: "Console reset.".into(),
                error: false,
                shown_at: Instant::now(),
            });
        }
    }

    fn set_fullscreen(&mut self, ctx: &egui::Context, fullscreen: bool) {
        self.settings.fullscreen = fullscreen;
        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(fullscreen));
        self.settings_dirty = true;
    }

    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        if let Some(button) = self.pending_bind {
            let key = ctx.input(|i| {
                i.events.iter().find_map(|event| match event {
                    egui::Event::Key {
                        key, pressed: true, ..
                    } => Some(*key),
                    _ => None,
                })
            });
            if let Some(key) = key {
                if key != egui::Key::Escape {
                    self.settings.set_key(button, key.name());
                    self.settings_dirty = true;
                }
                self.pending_bind = None;
            }
            return;
        }

        let (import, pause, reset, save, load, fullscreen) = ctx.input(|i| {
            (
                i.modifiers.command && i.key_pressed(egui::Key::O),
                i.key_pressed(egui::Key::Space),
                i.key_pressed(egui::Key::F2),
                i.key_pressed(egui::Key::F5),
                i.key_pressed(egui::Key::F8),
                i.key_pressed(egui::Key::F11),
            )
        });

        if import {
            self.open_rom_dialog();
        }
        if fullscreen {
            let value = !self.settings.fullscreen;
            self.set_fullscreen(ctx, value);
        }
        if self.view == View::Game {
            if let Some(session) = self.session.as_mut() {
                if pause {
                    session.paused = !session.paused;
                    session.accumulator = 0.0;
                }
                if reset {
                    self.reset_console();
                }
            }
            if save && !ctx.input(|i| i.modifiers.command) {
                self.save_slot(0);
            }
            if load {
                self.load_slot(0);
            }
        }
    }

    fn handle_dropped_files(&mut self, ctx: &egui::Context) {
        let dropped = ctx.input(|i| i.raw.dropped_files.clone());
        if let Some(file) = dropped.into_iter().next() {
            let path = file.path().to_path_buf();
            if path.is_file() {
                self.open_rom(path);
            }
        }
    }

    fn pump(&mut self, ctx: &egui::Context) {
        let Some(mut session) = self.session.take() else {
            return;
        };
        let now = Instant::now();
        let dt = now
            .duration_since(session.last_tick)
            .as_secs_f64()
            .min(0.25);
        session.last_tick = now;

        if !session.paused {
            for button in Button::ALL {
                let down = self
                    .settings
                    .key_name(button)
                    .and_then(|name| egui::Key::from_name(name))
                    .map(|key| ctx.input(|i| i.key_down(key)))
                    .unwrap_or(false);
                session.emulator.set_button(0, button, down);
            }

            session.accumulator += dt;
            let step = 1.0 / FRAME_RATE;
            let max_frames = if session.fast_forward { 6 } else { 2 };
            let mut frames = 0u32;
            session.audio.clear();
            while session.accumulator >= step && frames < max_frames {
                session.accumulator -= step;
                let frame_ok = session.emulator.run_frame();
                session.emulator.drain_audio(&mut session.audio);
                session.play_seconds += step;
                frames += 1;
                if !frame_ok {
                    session.paused = true;
                    self.status = Some(StatusMessage {
                        text: "The CPU stopped making progress; the ROM likely disabled NMI and rendering."
                            .into(),
                        error: true,
                        shown_at: Instant::now(),
                    });
                    break;
                }
            }
            if session.accumulator > step * 4.0 {
                session.accumulator = 0.0;
            }

            if !session.audio.is_empty() && !self.settings.muted {
                self.audio.push_mono(&session.audio, self.settings.volume);
            }
            session.fps_frames += frames;

            if session.emulator.frame_count >= 1_000 {
                self.unlock("frames_1000");
            }
            self.settings.total_play_seconds += dt;
            if self.settings.total_play_seconds >= 600.0 {
                self.unlock("play_ten_minutes");
            }
            if let Some(entry) = self
                .settings
                .library
                .iter_mut()
                .find(|entry| entry.path == session.path)
            {
                entry.last_played = Some(save_state::now_unix());
                entry.play_seconds += dt;
            }
        } else {
            session.accumulator = 0.0;
            self.audio.clear();
        }

        let fps_window = now.duration_since(session.fps_since).as_secs_f32();
        if fps_window >= 0.5 {
            session.fps = session.fps_frames as f32 / fps_window;
            session.fps_frames = 0;
            session.fps_since = now;
        }

        self.session = Some(session);
    }

    fn draw_sidebar(&mut self, ui: &mut egui::Ui) {
        egui::Panel::left("sidebar")
            .exact_size(214.0)
            .resizable(false)
            .frame(
                egui::Frame::default()
                    .fill(SIDEBAR)
                    .inner_margin(egui::Margin::symmetric(16, 18)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let (rect, _) = ui.allocate_exact_size(Vec2::new(10.0, 10.0), Sense::hover());
                    ui.painter().rect_filled(rect, ZERO, ACCENT);
                    ui.add_space(2.0);
                    ui.label(RichText::new("NES").size(22.0).strong().color(TEXT));
                    ui.label(RichText::new("CORE").size(22.0).strong().color(ACCENT));
                });
                ui.add_space(22.0);

                ui.label(RichText::new("LIBRARY").size(10.0).color(MUTED));
                ui.add_space(4.0);
                if self.nav_button(ui, View::Library) {
                    self.view = View::Library;
                }
                if self.nav_button(ui, View::LoadRom) {
                    self.view = View::LoadRom;
                }

                ui.add_space(14.0);
                ui.label(RichText::new("SESSION").size(10.0).color(MUTED));
                ui.add_space(4.0);
                if self.nav_button(ui, View::Game) {
                    if self.session.is_some() {
                        self.view = View::Game;
                    } else {
                        self.status = Some(StatusMessage {
                            text: "Load a ROM first.".into(),
                            error: true,
                            shown_at: Instant::now(),
                        });
                    }
                }
                if self.nav_button(ui, View::SaveStates) {
                    self.view = View::SaveStates;
                }

                ui.add_space(14.0);
                ui.label(RichText::new("APPLICATION").size(10.0).color(MUTED));
                ui.add_space(4.0);
                if self.nav_button(ui, View::Settings) {
                    self.view = View::Settings;
                }
                if self.nav_button(ui, View::Achievements) {
                    self.view = View::Achievements;
                }
                if self.nav_button(ui, View::About) {
                    self.view = View::About;
                }

                ui.with_layout(Layout::bottom_up(Align::LEFT), |ui| {
                    ui.label(
                        RichText::new("Independent emulator. Not affiliated with Nintendo.")
                            .size(10.0)
                            .color(MUTED),
                    );
                    ui.add_space(4.0);
                    ui.label(RichText::new(format!("v{VERSION}")).size(11.0).color(MUTED));
                    ui.add_space(8.0);
                    let audio_text = if self.audio.is_active() {
                        if self.settings.muted {
                            "Audio muted".to_string()
                        } else {
                            format!("Audio {:.0}%", self.settings.volume * 100.0)
                        }
                    } else {
                        "No audio device".to_string()
                    };
                    ui.label(RichText::new(audio_text).size(11.0).color(MUTED));
                });
            });
    }

    fn nav_button(&self, ui: &mut egui::Ui, view: View) -> bool {
        let selected = self.view == view;
        let text =
            RichText::new(view.label())
                .size(14.0)
                .color(if selected { TEXT } else { MUTED });
        let button = egui::Button::new(text)
            .fill(if selected {
                ACCENT_SOFT
            } else {
                Color32::TRANSPARENT
            })
            .stroke(if selected {
                Stroke::new(1.0, ACCENT_DIM)
            } else {
                Stroke::NONE
            })
            .corner_radius(ZERO)
            .min_size(Vec2::new(ui.available_width(), 36.0));
        ui.add(button).clicked()
    }

    fn draw_view(&mut self, ui: &mut egui::Ui) {
        match self.view {
            View::Game => self.draw_game(ui),
            View::Library => self.draw_library(ui),
            View::LoadRom => self.draw_load_rom(ui),
            View::Settings => self.draw_settings(ui),
            View::SaveStates => self.draw_save_states(ui),
            View::Achievements => self.draw_achievements(ui),
            View::About => self.draw_about(ui),
        }
    }

    fn page(&mut self, ui: &mut egui::Ui, title: &str, add: impl FnOnce(&mut Self, &mut egui::Ui)) {
        egui::CentralPanel::default()
            .frame(
                egui::Frame::default()
                    .fill(BG)
                    .inner_margin(egui::Margin::symmetric(30, 24)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(title).size(26.0).strong().color(TEXT));
                });
                ui.add_space(4.0);
                ui.separator();
                ui.add_space(12.0);
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| add(self, ui));
            });
    }

    fn draw_library(&mut self, ui: &mut egui::Ui) {
        self.page(ui, "Library", |this, ui| this.library_body(ui));
    }

    fn library_body(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(RichText::new("NES Core").size(42.0).strong().color(TEXT));
                ui.label(
                    RichText::new("A focused, open-source NES emulator.")
                        .size(15.0)
                        .color(MUTED),
                );
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.add(accent_button("Import ROM")).clicked() {
                    self.open_rom_dialog();
                }
                if let Some(last) = self.settings.last_rom.clone() {
                    if last.is_file() && ui.add(ghost_button("Play last")).clicked() {
                        self.open_rom(last);
                    }
                }
            });
        });

        ui.add_space(10.0);

        if let Some((title, mapper, played)) = self.session.as_ref().map(|session| {
            (
                session.title.clone(),
                session.mapper.clone(),
                session.play_seconds,
            )
        }) {
            egui::Frame::default()
                .fill(CARD)
                .stroke(Stroke::new(1.0, ACCENT_DIM))
                .inner_margin(egui::Margin::same(16))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.label(RichText::new("IN SESSION").size(10.0).color(ACCENT));
                            ui.label(RichText::new(&title).size(18.0).strong().color(TEXT));
                            ui.label(
                                RichText::new(format!("{mapper} • {played:.0}s played"))
                                    .size(12.0)
                                    .color(MUTED),
                            );
                        });
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui.add(accent_button("Resume")).clicked() {
                                self.view = View::Game;
                                if let Some(session) = self.session.as_mut() {
                                    session.paused = false;
                                    session.last_tick = Instant::now();
                                }
                            }
                        });
                    });
                });
            ui.add_space(16.0);
        }

        let entries = self.settings.library.clone();
        let mut recent: Vec<LibraryEntry> = entries
            .iter()
            .filter(|entry| entry.last_played.is_some())
            .cloned()
            .collect();
        recent.sort_by_key(|entry| std::cmp::Reverse(entry.last_played));
        recent.truncate(3);
        if !recent.is_empty() {
            ui.label(
                RichText::new("Recently played")
                    .size(16.0)
                    .strong()
                    .color(TEXT),
            );
            ui.add_space(6.0);
            self.card_grid(ui, &recent, 3);
            ui.add_space(22.0);
        }

        ui.label(RichText::new("All games").size(16.0).strong().color(TEXT));
        ui.add_space(6.0);

        if entries.is_empty() {
            egui::Frame::default()
                .fill(PANEL)
                .stroke(Stroke::new(1.0, BORDER))
                .inner_margin(egui::Margin::same(28))
                .show(ui, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(18.0);
                        ui.label(
                            RichText::new("Your library is empty")
                                .size(20.0)
                                .strong()
                                .color(TEXT),
                        );
                        ui.label(
                            RichText::new(
                                "Import a legally obtained .nes file, or drop one onto this window.",
                            )
                            .size(13.0)
                            .color(MUTED),
                        );
                        ui.add_space(14.0);
                        if ui.add(accent_button("Import ROM")).clicked() {
                            self.open_rom_dialog();
                        }
                        ui.add_space(18.0);
                    });
                });
        } else {
            self.card_grid(ui, &entries, 3);
        }

        ui.add_space(24.0);
        ui.label(
            RichText::new(
                "NES Core ships with no ROMs or artwork. Only add games you are allowed to use.",
            )
            .size(11.0)
            .color(MUTED),
        );
    }

    fn card_grid(&mut self, ui: &mut egui::Ui, entries: &[LibraryEntry], columns: usize) {
        let spacing = 16.0;
        let total = ui.available_width();
        let width = ((total - spacing * (columns as f32 - 1.0)) / columns as f32).max(190.0);
        let mut action: Option<(CardAction, PathBuf)> = None;

        egui::Grid::new(format!("grid-{}", entries.len()))
            .num_columns(columns)
            .spacing([spacing, spacing])
            .show(ui, |ui| {
                for (index, entry) in entries.iter().enumerate() {
                    let texture = self.art.texture(ui.ctx(), &entry.path);
                    let result = library_card(ui, entry, texture, width);
                    match result {
                        CardAction::Play => action = Some((CardAction::Play, entry.path.clone())),
                        CardAction::Remove => {
                            action = Some((CardAction::Remove, entry.path.clone()))
                        }
                        CardAction::None => {}
                    }
                    if (index + 1) % columns == 0 {
                        ui.end_row();
                    }
                }
            });

        match action {
            Some((CardAction::Play, path)) => {
                if self
                    .session
                    .as_ref()
                    .map(|session| session.path == path)
                    .unwrap_or(false)
                {
                    self.view = View::Game;
                    if let Some(session) = self.session.as_mut() {
                        session.paused = false;
                        session.last_tick = Instant::now();
                    }
                } else {
                    self.open_rom(path);
                }
            }
            Some((CardAction::Remove, path)) => {
                self.settings.library.retain(|entry| entry.path != path);
                if self
                    .session
                    .as_ref()
                    .map(|session| session.path == path)
                    .unwrap_or(false)
                {
                    self.session = None;
                    self.audio.clear();
                }
                self.settings_dirty = true;
                self.save_settings();
            }
            _ => {}
        }
    }

    fn draw_load_rom(&mut self, ui: &mut egui::Ui) {
        self.page(ui, "Load ROM", |this, ui| {
            egui::Frame::default()
                .fill(PANEL)
                .stroke(Stroke::new(1.0, BORDER))
                .inner_margin(egui::Margin::same(24))
                .show(ui, |ui| {
                    ui.label(RichText::new("Open a cartridge image").size(20.0).strong());
                    ui.label(
                        RichText::new(
                            "NES Core reads iNES and NES 2.0 .nes files. Headers and file sizes \
                             are validated before boot, and unsupported mappers are reported \
                             instead of being guessed.",
                        )
                        .size(13.0)
                        .color(MUTED),
                    );
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        if ui.add(accent_button("Choose .nes file")).clicked() {
                            this.open_rom_dialog();
                        }
                        if let Some(last) = this.settings.last_rom.clone() {
                            if last.is_file() && ui.add(ghost_button("Reopen last ROM")).clicked() {
                                this.open_rom(last);
                            }
                        }
                    });
                    ui.add_space(16.0);
                    ui.label(
                        RichText::new("Supported mappers")
                            .size(13.0)
                            .strong()
                            .color(TEXT),
                    );
                    ui.label(
                        RichText::new(
                            "0 NROM · 1 MMC1 · 2 UxROM · 3 CNROM · 4 MMC3 · 5 MMC5 · 7 AxROM · \
                             9 MMC2 · 10 MMC4 · 11 Color Dreams · 24/26 VRC6 · 66 GxROM · \
                             71 Camerica",
                        )
                        .size(12.0)
                        .color(MUTED),
                    );
                });

            ui.add_space(16.0);
            egui::Frame::default()
                .fill(CARD)
                .inner_margin(egui::Margin::same(20))
                .show(ui, |ui| {
                    ui.label(RichText::new("Drag and drop").size(14.0).strong());
                    ui.label(
                        RichText::new("You can also drop a .nes file anywhere in the window.")
                            .size(12.0)
                            .color(MUTED),
                    );
                    ui.add_space(10.0);
                    ui.label(
                        RichText::new(
                            "ROM files stay on your computer. NES Core does not download, \
                             bundle or distribute game images or copyrighted artwork.",
                        )
                        .size(12.0)
                        .color(MUTED),
                    );
                });
        });
    }

    fn draw_game(&mut self, ui: &mut egui::Ui) {
        let fullscreen = self.settings.fullscreen;
        if self.session.is_none() {
            self.page(ui, "Now Playing", |this, ui| {
                ui.label(RichText::new("Nothing is loaded.").size(16.0).color(MUTED));
                if ui.add(accent_button("Load a ROM")).clicked() {
                    this.open_rom_dialog();
                }
            });
            return;
        }

        if !fullscreen {
            let mut exit = false;
            let mut toggle_pause = false;
            let mut reset = false;
            let mut save = false;
            let mut load = false;
            let mut full = false;
            let mut fast = false;
            let mut title = String::new();
            let mut mapper = String::new();
            if let Some(session) = self.session.as_ref() {
                title = session.title.clone();
                mapper = session.mapper.clone();
                toggle_pause = session.paused;
            }
            egui::Panel::top("game_bar")
                .frame(
                    egui::Frame::default()
                        .fill(PANEL)
                        .inner_margin(egui::Margin::symmetric(20, 12)),
                )
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(&title).size(17.0).strong().color(TEXT));
                        ui.label(RichText::new(&mapper).size(12.0).color(MUTED));

                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui.add(ghost_button("Library")).clicked() {
                                exit = true;
                            }
                            if ui.add(ghost_button("Fullscreen")).clicked() {
                                full = true;
                            }
                            if ui.add(ghost_button("Load")).clicked() {
                                load = true;
                            }
                            if ui.add(ghost_button("Save")).clicked() {
                                save = true;
                            }
                            if ui.add(ghost_button("Reset")).clicked() {
                                reset = true;
                            }
                            let pause_label = if toggle_pause { "Resume" } else { "Pause" };
                            if ui.add(accent_button(pause_label)).clicked() {
                                toggle_pause = !toggle_pause;
                            }
                            if session_is_fast(self.session.as_ref())
                                && ui.add(ghost_button("1x")).clicked()
                            {
                                fast = true;
                            }
                            if !session_is_fast(self.session.as_ref())
                                && ui.add(ghost_button("Fast")).clicked()
                            {
                                fast = true;
                            }
                        });
                    });
                });

            if exit {
                if let Some(session) = self.session.as_mut() {
                    session.paused = true;
                }
                self.audio.clear();
                self.view = View::Library;
                self.settings_dirty = true;
            }
            if full {
                let ctx = ui.ctx().clone();
                self.set_fullscreen(&ctx, true);
            }
            if let Some(session) = self.session.as_mut() {
                if toggle_pause != session.paused {
                    session.paused = toggle_pause;
                    session.accumulator = 0.0;
                    self.audio.clear();
                }
                if fast {
                    session.fast_forward = !session.fast_forward;
                }
            }
            if reset {
                self.reset_console();
            }
            if save {
                self.save_slot(0);
            }
            if load {
                self.load_slot(0);
            }
        }

        let mut back = false;
        egui::CentralPanel::default()
            .frame(
                egui::Frame::default()
                    .fill(BG)
                    .inner_margin(egui::Margin::same(16)),
            )
            .show(ui, |ui| {
                let Some(session) = self.session.as_mut() else {
                    return;
                };
                let available = ui.available_size();
                let bottom = 46.0;
                let stage_height = (available.y - bottom).max(120.0);
                let (rect, _) =
                    ui.allocate_exact_size(Vec2::new(available.x, stage_height), Sense::hover());

                let ratio = if self.settings.pixel_aspect {
                    256.0 / 240.0 * (8.0 / 7.0)
                } else {
                    256.0 / 240.0
                };
                let mut width = rect.width() - 16.0;
                let mut height = width / ratio;
                if height > rect.height() - 16.0 {
                    height = rect.height() - 16.0;
                    width = height * ratio;
                }
                if self.settings.integer_scaling {
                    let scale = if self.settings.pixel_aspect {
                        (width / (256.0 * 8.0 / 7.0)).floor().max(1.0)
                    } else {
                        (width / 256.0).floor().max(1.0)
                    };
                    width = if self.settings.pixel_aspect {
                        256.0 * scale * 8.0 / 7.0
                    } else {
                        256.0 * scale
                    };
                    height = 240.0 * scale;
                    if height > rect.height() - 16.0 {
                        let fit = ((rect.height() - 16.0) / 240.0).max(1.0);
                        height = 240.0 * fit;
                        width = if self.settings.pixel_aspect {
                            256.0 * fit * 8.0 / 7.0
                        } else {
                            256.0 * fit
                        };
                    }
                }

                let image = egui::ColorImage::from_rgba_unmultiplied(
                    [256, 240],
                    session.emulator.framebuffer(),
                );
                match session.texture.as_mut() {
                    Some(texture) => texture.set(image, egui::TextureOptions::NEAREST),
                    None => {
                        session.texture = Some(ui.ctx().load_texture(
                            "nes-framebuffer",
                            image,
                            egui::TextureOptions::NEAREST,
                        ))
                    }
                }

                let stage = egui::Rect::from_center_size(rect.center(), Vec2::new(width, height));
                ui.painter()
                    .rect_filled(rect, ZERO, Color32::from_rgb(4, 6, 11));
                if let Some(texture) = session.texture.as_ref() {
                    ui.painter().image(
                        texture.id(),
                        stage,
                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                        Color32::WHITE,
                    );
                }
                ui.painter().rect_stroke(
                    stage,
                    ZERO,
                    Stroke::new(1.0, BORDER),
                    egui::StrokeKind::Inside,
                );

                if session.paused {
                    ui.painter().rect_filled(
                        stage,
                        ZERO,
                        Color32::from_rgba_unmultiplied(4, 7, 12, 170),
                    );
                    ui.painter().text(
                        stage.center(),
                        Align2::CENTER_CENTER,
                        "PAUSED",
                        FontId::proportional(24.0),
                        TEXT,
                    );
                }

                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    let state = if session.paused { "Paused" } else { "Running" };
                    ui.label(RichText::new(state).size(12.0).color(if session.paused {
                        MUTED
                    } else {
                        OK
                    }));
                    if self.settings.show_fps {
                        ui.label(
                            RichText::new(format!("{:.1} fps", session.fps))
                                .size(12.0)
                                .color(MUTED),
                        );
                        ui.label(
                            RichText::new(format!(
                                "{} frames · P1 keyboard",
                                session.emulator.frame_count
                            ))
                            .size(12.0)
                            .color(MUTED),
                        );
                    }
                    if session.fast_forward {
                        ui.label(RichText::new("FAST FORWARD").size(11.0).color(ACCENT));
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new("Back to library").size(12.0).color(MUTED),
                                )
                                .fill(Color32::TRANSPARENT)
                                .stroke(Stroke::NONE)
                                .corner_radius(ZERO),
                            )
                            .clicked()
                        {
                            back = true;
                        }
                        ui.label(
                            RichText::new("Space pause · F2 reset · F5/F8 slots · F11 fullscreen")
                                .size(11.0)
                                .color(MUTED),
                        );
                    });
                });
            });

        if back {
            if let Some(session) = self.session.as_mut() {
                session.paused = true;
            }
            self.audio.clear();
            self.view = View::Library;
            self.settings_dirty = true;
        }
    }

    fn draw_settings(&mut self, ui: &mut egui::Ui) {
        self.page(ui, "Settings", |this, ui| {
            ui.columns(2, |columns| {
                columns[0].label(RichText::new("AUDIO").size(11.0).color(MUTED));
                columns[0].add_space(6.0);
                egui::Frame::default()
                    .fill(CARD)
                    .stroke(Stroke::new(1.0, BORDER))
                    .inner_margin(egui::Margin::same(16))
                    .show(&mut columns[0], |ui| {
                        if ui
                            .checkbox(&mut this.settings.muted, "Mute audio")
                            .changed()
                        {
                            this.settings_dirty = true;
                            this.audio.clear();
                        }
                        let mut volume = this.settings.volume;
                        if ui
                            .add(egui::Slider::new(&mut volume, 0.0..=1.0).text("Master volume"))
                            .changed()
                        {
                            this.settings.volume = volume;
                            this.settings_dirty = true;
                        }
                        let device = if this.audio.is_active() {
                            "miniaudio device active"
                        } else {
                            "No audio device (silent)"
                        };
                        ui.label(RichText::new(device).size(11.0).color(MUTED));
                    });

                columns[0].add_space(16.0);
                columns[0].label(RichText::new("VIDEO").size(11.0).color(MUTED));
                columns[0].add_space(6.0);
                egui::Frame::default()
                    .fill(CARD)
                    .stroke(Stroke::new(1.0, BORDER))
                    .inner_margin(egui::Margin::same(16))
                    .show(&mut columns[0], |ui| {
                        changed(
                            ui.checkbox(
                                &mut this.settings.pixel_aspect,
                                "NES pixel aspect (8:7, 4:3 display)",
                            ),
                            &mut this.settings_dirty,
                        );
                        changed(
                            ui.checkbox(&mut this.settings.integer_scaling, "Integer scaling"),
                            &mut this.settings_dirty,
                        );
                        changed(
                            ui.checkbox(&mut this.settings.show_fps, "Show FPS and frame count"),
                            &mut this.settings_dirty,
                        );
                        changed(
                            ui.checkbox(&mut this.settings.vsync, "VSync (applied on restart)"),
                            &mut this.settings_dirty,
                        );
                        let mut fullscreen = this.settings.fullscreen;
                        if ui.checkbox(&mut fullscreen, "Fullscreen").changed() {
                            let ctx = ui.ctx().clone();
                            this.set_fullscreen(&ctx, fullscreen);
                            this.settings_dirty = true;
                        }
                    });

                columns[1].label(RichText::new("CONTROLS").size(11.0).color(MUTED));
                columns[1].add_space(6.0);
                egui::Frame::default()
                    .fill(CARD)
                    .stroke(Stroke::new(1.0, BORDER))
                    .inner_margin(egui::Margin::same(16))
                    .show(&mut columns[1], |ui| {
                        egui::Grid::new("keybindings")
                            .num_columns(3)
                            .spacing([12.0, 8.0])
                            .show(ui, |ui| {
                                for button in Button::ALL {
                                    ui.label(RichText::new(button.label()).size(13.0));
                                    let bound = this
                                        .settings
                                        .key_name(button)
                                        .cloned()
                                        .unwrap_or_else(|| "—".into());
                                    let text = if this.pending_bind == Some(button) {
                                        "press a key…".to_string()
                                    } else {
                                        bound
                                    };
                                    ui.label(RichText::new(text).size(13.0).color(ACCENT));
                                    if ui
                                        .add(ghost_button(if this.pending_bind == Some(button) {
                                            "Cancel"
                                        } else {
                                            "Rebind"
                                        }))
                                        .clicked()
                                    {
                                        this.pending_bind = if this.pending_bind == Some(button) {
                                            None
                                        } else {
                                            Some(button)
                                        };
                                    }
                                    ui.end_row();
                                }
                            });
                        ui.add_space(8.0);
                        ui.label(
                            RichText::new(
                                "Gamepad support is not implemented in this build. \
                                 Player 1 is driven by the keyboard.",
                            )
                            .size(11.0)
                            .color(MUTED),
                        );
                        if ui.add(ghost_button("Restore default keys")).clicked() {
                            this.settings.keymap = Settings::default().keymap;
                            this.settings_dirty = true;
                        }
                    });

                columns[1].add_space(16.0);
                columns[1].label(RichText::new("LIBRARY & DATA").size(11.0).color(MUTED));
                columns[1].add_space(6.0);
                egui::Frame::default()
                    .fill(CARD)
                    .stroke(Stroke::new(1.0, BORDER))
                    .inner_margin(egui::Margin::same(16))
                    .show(&mut columns[1], |ui| {
                        ui.label(
                            RichText::new(format!(
                                "{} ROMs · {} save-state slots in use",
                                this.settings.library.len(),
                                count_slots(this.session.as_ref().map(|s| s.emulator.rom_id))
                            ))
                            .size(12.0),
                        );
                        let config = config_dir()
                            .map(|p| p.display().to_string())
                            .unwrap_or_else(|| "unavailable".into());
                        ui.label(
                            RichText::new(format!("Settings: {config}"))
                                .size(11.0)
                                .color(MUTED),
                        );
                        ui.label(
                            RichText::new("Settings and the library are saved automatically.")
                                .size(11.0)
                                .color(MUTED),
                        );
                    });
            });
        });
    }

    fn draw_save_states(&mut self, ui: &mut egui::Ui) {
        self.page(ui, "Save States", |this, ui| {
            let Some(session) = this.session.as_ref() else {
                ui.label(
                    RichText::new("Load a ROM to use save states.")
                        .size(14.0)
                        .color(MUTED),
                );
                if ui.add(accent_button("Load ROM")).clicked() {
                    this.open_rom_dialog();
                }
                return;
            };
            let rom_id = session.emulator.rom_id;
            ui.label(
                RichText::new(format!(
                    "{} — states are versioned and tied to this ROM.",
                    session.title
                ))
                .size(13.0)
                .color(MUTED),
            );
            ui.add_space(8.0);

            egui::Grid::new("slots")
                .num_columns(4)
                .spacing([14.0, 10.0])
                .show(ui, |ui| {
                    for slot in 0..10u8 {
                        let path = state_dir().map(|dir| {
                            dir.join(format!("{rom_id:016x}"))
                                .join(format!("slot-{slot}.json"))
                        });
                        let meta = path.as_ref().and_then(|path| {
                            if path.is_file() {
                                save_state::read_meta(path).ok()
                            } else {
                                None
                            }
                        });

                        ui.label(RichText::new(format!("Slot {slot}")).size(14.0).strong());
                        if let Some(meta) = &meta {
                            let note = if meta.version == save_state::SAVE_STATE_VERSION {
                                format!(
                                    "frame {} · saved {}",
                                    meta.frame_count,
                                    relative_time(meta.created_at)
                                )
                            } else {
                                format!(
                                    "unsupported version {} (this build: {})",
                                    meta.version,
                                    save_state::SAVE_STATE_VERSION
                                )
                            };
                            ui.label(RichText::new(note).size(12.0).color(MUTED));
                        } else {
                            ui.label(RichText::new("empty").size(12.0).color(MUTED));
                        }
                        if ui.add(ghost_button("Save")).clicked() {
                            this.save_slot(slot);
                        }
                        ui.horizontal(|ui| {
                            let can_load = meta
                                .as_ref()
                                .map(|m| m.version == save_state::SAVE_STATE_VERSION)
                                .unwrap_or(false);
                            ui.add_enabled_ui(can_load, |ui| {
                                if ui.add(ghost_button("Load")).clicked() {
                                    this.load_slot(slot);
                                }
                            });
                            if meta.is_some() && ui.add(ghost_button("Delete")).clicked() {
                                if let Some(path) = &path {
                                    let _ = std::fs::remove_file(path);
                                }
                                this.status = Some(StatusMessage {
                                    text: format!("Deleted slot {slot}."),
                                    error: false,
                                    shown_at: Instant::now(),
                                });
                            }
                        });
                        ui.end_row();
                    }
                });
            ui.add_space(12.0);
            ui.label(
                RichText::new(
                    "States are JSON documents with a version field and a ROM fingerprint. \
                     A state from another ROM or a newer format is rejected with an error.",
                )
                .size(11.0)
                .color(MUTED),
            );
        });
    }

    fn draw_achievements(&mut self, ui: &mut egui::Ui) {
        self.page(ui, "Achievements", |this, ui| {
            let unlocked = ACHIEVEMENTS
                .iter()
                .filter(|definition| this.settings.is_unlocked(definition.id))
                .count();
            ui.label(
                RichText::new(format!("{unlocked} of {} unlocked", ACHIEVEMENTS.len()))
                    .size(13.0)
                    .color(MUTED),
            );
            ui.label(
                RichText::new(
                    "Only generic events that NES Core tracks are listed. There are no \
                     game-specific achievements.",
                )
                .size(11.0)
                .color(MUTED),
            );
            ui.add_space(12.0);

            for definition in ACHIEVEMENTS {
                let record = this
                    .settings
                    .achievements
                    .iter()
                    .find(|record| record.id == definition.id);
                egui::Frame::default()
                    .fill(if record.is_some() { CARD } else { PANEL })
                    .stroke(Stroke::new(
                        1.0,
                        if record.is_some() { ACCENT_DIM } else { BORDER },
                    ))
                    .inner_margin(egui::Margin::same(16))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let (rect, _) =
                                ui.allocate_exact_size(Vec2::splat(10.0), Sense::hover());
                            ui.painter().rect_filled(
                                rect,
                                ZERO,
                                if record.is_some() { ACCENT } else { BORDER },
                            );
                            ui.vertical(|ui| {
                                ui.label(
                                    RichText::new(definition.name)
                                        .size(15.0)
                                        .strong()
                                        .color(if record.is_some() { TEXT } else { MUTED }),
                                );
                                ui.label(
                                    RichText::new(definition.description)
                                        .size(12.0)
                                        .color(MUTED),
                                );
                            });
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                let note = if let Some(record) = record {
                                    format!("Unlocked {}", relative_time(record.unlocked_at))
                                } else {
                                    "Locked".to_string()
                                };
                                ui.label(
                                    RichText::new(note).size(11.0).color(if record.is_some() {
                                        OK
                                    } else {
                                        MUTED
                                    }),
                                );
                            });
                        });
                    });
                ui.add_space(8.0);
            }
        });
    }

    fn draw_about(&mut self, ui: &mut egui::Ui) {
        self.page(ui, "About", |_this, ui| {
            ui.label(RichText::new("NES Core").size(30.0).strong().color(TEXT));
            ui.label(
                RichText::new(format!("Version {VERSION} · MIT licensed"))
                    .size(13.0)
                    .color(MUTED),
            );
            ui.add_space(14.0);
            ui.label(
                RichText::new(
                    "NES Core is an independent, open-source emulator for the Nintendo \
                     Entertainment System / Famicom. It implements the 2A03 CPU, PPU, APU, \
                     controllers, cartridge mappers and a desktop interface in Rust. It is \
                     not affiliated with, endorsed by, or licensed by Nintendo.",
                )
                .size(13.0),
            );
            ui.add_space(12.0);
            for (heading, body) in [
                (
                    "Hardware emulation",
                    "Ricoh 2A03 CPU with the documented instruction set, cycle accounting, \
                     NMI/IRQ and DMA; PPU with background, sprites, scrolling, palettes and \
                     NMI; APU with two pulses, triangle, noise and DMC; two controller ports.",
                ),
                (
                    "Cartridges",
                    "iNES and NES 2.0 headers, PRG/CHR mapping, PRG RAM and battery-backed \
                     RAM in memory, and mappers 0, 1, 2, 3, 4, 5, 7, 9, 10, 11, 24, 26,\
                     66 and 71.",
                ),
                (
                    "Desktop features",
                    "Library, ROM import and drag-and-drop, save states, rebindable keyboard \
                     controls, audio volume, scaling options, fullscreen, pause, reset and \
                     performance readouts.",
                ),
                (
                    "Known limitations",
                    "No gamepad input, no rewinding, no netplay, no PAL timing and no per-dot \
                     mid-scanline raster effects. MMC5 and VRC6 expansion audio are emulated; \
                     MMC5 fill-mode nametables are not. Some mappers and games are not \
                     supported; see the compatibility notes in the repository.",
                ),
            ] {
                ui.label(RichText::new(heading).size(14.0).strong().color(ACCENT));
                ui.label(RichText::new(body).size(12.0).color(MUTED));
                ui.add_space(10.0);
            }
            ui.add_space(6.0);
            ui.label(
                RichText::new(
                    "No ROMs, BIOS files or commercial artwork are distributed with NES Core.",
                )
                .size(11.0)
                .color(MUTED),
            );
            ui.label(
                RichText::new("https://github.com/arpitgoswami/nes-core")
                    .size(11.0)
                    .color(MUTED),
            );
        });
    }

    fn draw_toast(&mut self, ctx: &egui::Context) {
        let Some(status) = self.status.as_ref() else {
            return;
        };
        if status.shown_at.elapsed() > Duration::from_secs(7) {
            self.status = None;
            return;
        }
        let color = if status.error { ERROR } else { OK };
        let text = status.text.clone();
        egui::Area::new(egui::Id::unique("toast"))
            .anchor(Align2::RIGHT_BOTTOM, [-22.0, -20.0])
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::default()
                    .fill(PANEL)
                    .stroke(Stroke::new(1.0, color))
                    .inner_margin(egui::Margin::symmetric(16, 12))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(text).size(12.0).color(TEXT));
                            if ui
                                .add(
                                    egui::Button::new(RichText::new("×").color(MUTED)).frame(false),
                                )
                                .clicked()
                            {
                                self.status = None;
                            }
                        });
                    });
            });
        ctx.request_repaint_after(Duration::from_millis(500));
    }
}

impl eframe::App for NesApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.handle_shortcuts(&ctx);
        self.handle_dropped_files(&ctx);
        self.pump(&ctx);

        let hide_chrome = self.settings.fullscreen && self.view == View::Game;
        if !hide_chrome {
            self.draw_sidebar(ui);
        }
        self.draw_view(ui);

        if hide_chrome {
            egui::Area::new(egui::Id::unique("fullscreen-overlay"))
                .anchor(Align2::RIGHT_TOP, [-18.0, 14.0])
                .order(egui::Order::Foreground)
                .show(&ctx, |ui| {
                    egui::Frame::default()
                        .fill(Color32::from_rgba_unmultiplied(7, 10, 17, 220))
                        .inner_margin(egui::Margin::symmetric(12, 8))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new("F11 exit fullscreen · Esc pause")
                                        .size(11.0)
                                        .color(MUTED),
                                );
                                if ui.add(ghost_button("Exit")).clicked() {
                                    self.set_fullscreen(&ctx.clone(), false);
                                }
                            });
                        });
                });
        }

        self.draw_toast(&ctx);

        if self.settings_dirty && self.last_settings_save.elapsed() > Duration::from_secs(2) {
            self.save_settings();
        }

        if self
            .session
            .as_ref()
            .map(|session| !session.paused)
            .unwrap_or(false)
        {
            ctx.request_repaint();
        } else {
            ctx.request_repaint_after(Duration::from_millis(250));
        }
    }
}

fn session_is_fast(session: Option<&Session>) -> bool {
    session.map(|session| session.fast_forward).unwrap_or(false)
}

fn changed(response: egui::Response, dirty: &mut bool) {
    if response.changed() {
        *dirty = true;
    }
}

fn count_slots(rom_id: Option<u64>) -> usize {
    let Some(rom_id) = rom_id else {
        return 0;
    };
    let Some(dir) = state_dir() else {
        return 0;
    };
    let dir = dir.join(format!("{rom_id:016x}"));
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    entries
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry
                .path()
                .extension()
                .map(|e| e.to_string_lossy() == "json")
                .unwrap_or(false)
        })
        .count()
}

fn relative_time(timestamp: u64) -> String {
    let now = save_state::now_unix();
    let seconds = now.saturating_sub(timestamp);
    if seconds < 60 {
        "just now".into()
    } else if seconds < 3600 {
        format!("{}m ago", seconds / 60)
    } else if seconds < 86_400 {
        format!("{}h ago", seconds / 3600)
    } else {
        format!("{}d ago", seconds / 86_400)
    }
}

fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let mut result: String = text.chars().take(max.saturating_sub(1)).collect();
    result.push('…');
    result
}

fn library_card(
    ui: &mut egui::Ui,
    entry: &LibraryEntry,
    texture: Option<egui::TextureHandle>,
    width: f32,
) -> CardAction {
    let height = 194.0;
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, height), Sense::click());
    let hovered = response.hovered();
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, ZERO, if hovered { CARD_HOVER } else { CARD });
    painter.rect_stroke(
        rect,
        ZERO,
        Stroke::new(1.0, if hovered { ACCENT_DIM } else { BORDER }),
        egui::StrokeKind::Inside,
    );

    let art_rect = egui::Rect::from_min_size(
        rect.min + Vec2::splat(8.0),
        Vec2::new(rect.width() - 16.0, 108.0),
    );
    painter.rect_filled(art_rect, ZERO, Color32::from_rgb(8, 12, 20));
    if let Some(texture) = texture {
        painter.image(
            texture.id(),
            art_rect,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            Color32::WHITE,
        );
    } else {
        let accent = artwork::accent_for(entry.rom_id);
        painter.rect_filled(
            egui::Rect::from_min_size(art_rect.min, Vec2::new(5.0, art_rect.height())),
            ZERO,
            accent,
        );
        for row in 0..4 {
            for column in 0..10 {
                let x = art_rect.left() + 18.0 + column as f32 * 20.0;
                let y = art_rect.top() + 16.0 + row as f32 * 20.0;
                let block = egui::Rect::from_min_size(egui::pos2(x, y), Vec2::new(14.0, 14.0));
                if (row + column) % 3 == 0 {
                    painter.rect_filled(block, ZERO, Color32::from_rgb(20, 28, 42));
                }
            }
        }
        let initial = entry
            .title
            .chars()
            .next()
            .unwrap_or('?')
            .to_uppercase()
            .to_string();
        painter.text(
            art_rect.center(),
            Align2::CENTER_CENTER,
            initial,
            FontId::proportional(44.0),
            accent,
        );
        painter.text(
            art_rect.left_bottom() + Vec2::new(12.0, -8.0),
            Align2::LEFT_BOTTOM,
            "NES",
            FontId::proportional(9.0),
            MUTED,
        );
    }

    painter.text(
        egui::pos2(rect.left() + 12.0, art_rect.bottom() + 10.0),
        Align2::LEFT_TOP,
        truncate(&entry.title, 28),
        FontId::proportional(14.0),
        TEXT,
    );
    painter.text(
        egui::pos2(rect.left() + 12.0, art_rect.bottom() + 32.0),
        Align2::LEFT_TOP,
        format!("Mapper {} · {}", entry.mapper, entry.mapper_name),
        FontId::proportional(10.5),
        MUTED,
    );
    painter.text(
        egui::pos2(rect.left() + 12.0, art_rect.bottom() + 50.0),
        Align2::LEFT_TOP,
        format!("{:.0}s played", entry.play_seconds),
        FontId::proportional(10.0),
        MUTED,
    );

    let play_rect = egui::Rect::from_min_size(
        egui::pos2(rect.right() - 78.0, rect.bottom() - 38.0),
        Vec2::new(66.0, 26.0),
    );
    let play = ui.put(play_rect, accent_button("Play"));
    let remove_rect = egui::Rect::from_min_size(
        egui::pos2(rect.right() - 30.0, rect.top() + 14.0),
        Vec2::new(20.0, 20.0),
    );
    let remove = ui.put(
        remove_rect,
        egui::Button::new(RichText::new("×").size(15.0).color(MUTED))
            .fill(Color32::from_rgba_unmultiplied(8, 12, 20, 200))
            .stroke(Stroke::NONE)
            .corner_radius(ZERO),
    );

    if play.clicked() {
        CardAction::Play
    } else if remove.clicked() {
        CardAction::Remove
    } else if response.clicked() {
        CardAction::Play
    } else {
        CardAction::None
    }
}
