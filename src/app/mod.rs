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
use nes_core::emulator::Emulator;
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
            View::Library => "Games",
            View::LoadRom => "Import",
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
    /// Seconds until the next emulated frame is due, used to schedule repaints.
    next_wake: f64,
    audio: Vec<f32>,
    fast_forward: bool,
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
    /// Index of the game tile highlighted on the home screen.
    library_focus: usize,
}

impl NesApp {
    pub fn new(cc: &eframe::CreationContext<'_>, mut settings: Settings) -> Self {
        settings.fullscreen = false;
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
            library_focus: 0,
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
                    next_wake: 0.016,
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
        } else if self.settings.fullscreen && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.set_fullscreen(ctx, false);
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
            // Nudge emulation speed by up to 1% so the audio queue stays near
            // 60 ms: no underruns (crackle) and no overruns (lag or dropped audio).
            let ratio = if self.audio.is_active() && !self.settings.muted && !session.fast_forward {
                let target = 0.06_f64;
                let queued = f64::from(self.audio.buffered_seconds());
                1.0 + ((target - queued) / target).clamp(-1.0, 1.0) * 0.01
            } else {
                1.0
            };
            let step = 1.0 / (session.emulator.frame_rate() * ratio);
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
            session.next_wake = (step - session.accumulator).max(0.0);

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

    fn draw_topbar(&mut self, ui: &mut egui::Ui) {
        let mut tabs = vec![View::Library, View::LoadRom];
        if self.session.is_some() {
            tabs.push(View::Game);
        }
        tabs.extend([
            View::SaveStates,
            View::Achievements,
            View::Settings,
            View::About,
        ]);
        let audio_text = if !self.audio.is_active() {
            "No audio device".to_string()
        } else if self.settings.muted {
            "Audio muted".to_string()
        } else {
            format!("Audio {:.0}%", self.settings.volume * 100.0)
        };
        let mut next_view = None;

        egui::Panel::top("topbar")
            .exact_size(64.0)
            .resizable(false)
            .frame(egui::Frame::NONE.fill(TOPBAR))
            .show(ui, |ui| {
                let rect = ui.max_rect();
                let painter = ui.painter().clone();
                let middle = rect.center().y;
                painter.line_segment(
                    [rect.left_bottom(), rect.right_bottom()],
                    Stroke::new(1.0, Color32::from_white_alpha(14)),
                );

                // Logo.
                let logo = painter.text(
                    egui::pos2(rect.left() + 28.0, middle),
                    Align2::LEFT_CENTER,
                    "NES",
                    semibold(22.0),
                    TEXT,
                );
                painter.text(
                    egui::pos2(logo.right() + 6.0, middle),
                    Align2::LEFT_CENTER,
                    "CORE",
                    semibold(22.0),
                    ACCENT,
                );

                // Status on the right.
                painter.text(
                    egui::pos2(rect.right() - 28.0, middle),
                    Align2::RIGHT_CENTER,
                    format!("{audio_text}   ·   v{VERSION}"),
                    FontId::proportional(12.0),
                    MUTED,
                );

                // Centered tabs.
                let font = semibold(15.0);
                let widths: Vec<f32> = tabs
                    .iter()
                    .map(|view| {
                        painter
                            .layout_no_wrap(view.label().to_string(), font.clone(), TEXT)
                            .size()
                            .x
                            + 36.0
                    })
                    .collect();
                let total: f32 = widths.iter().sum::<f32>() + 4.0 * (tabs.len() as f32 - 1.0);
                let mut x = (rect.center().x - total / 2.0).max(logo.right() + 110.0);
                for (index, view) in tabs.iter().enumerate() {
                    let tab = egui::Rect::from_min_size(
                        egui::pos2(x, middle - 19.0),
                        Vec2::new(widths[index], 38.0),
                    );
                    x += widths[index] + 4.0;
                    let response = ui.interact(
                        tab,
                        ui.scope_id().with(("topbar-tab", index)),
                        Sense::click(),
                    );
                    let selected = self.view == *view;
                    if selected {
                        painter.rect_filled(tab, PILL, Color32::from_white_alpha(28));
                    } else if response.hovered() {
                        painter.rect_filled(tab, PILL, Color32::from_white_alpha(12));
                    }
                    painter.text(
                        tab.center(),
                        Align2::CENTER_CENTER,
                        view.label(),
                        font.clone(),
                        if selected || response.hovered() {
                            TEXT
                        } else {
                            MUTED
                        },
                    );
                    if response.clicked() {
                        next_view = Some(*view);
                    }
                }
            });

        if let Some(view) = next_view {
            self.view = view;
        }
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
            .frame(egui::Frame::NONE)
            .show(ui, |ui| {
                paint_backdrop(ui.painter(), ui.max_rect());
                egui::Frame::NONE
                    .inner_margin(egui::Margin::symmetric(48, 28))
                    .show(ui, |ui| {
                        ui.label(RichText::new(title).font(semibold(34.0)).color(TEXT));
                        ui.add_space(14.0);
                        egui::ScrollArea::vertical()
                            .auto_shrink([false, false])
                            .show(ui, |ui| add(self, ui));
                    });
            });
    }

    fn draw_library(&mut self, ui: &mut egui::Ui) {
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| {
                paint_backdrop(ui.painter(), ui.max_rect());
                egui::Frame::NONE
                    .inner_margin(egui::Margin::symmetric(48, 28))
                    .show(ui, |ui| {
                        egui::ScrollArea::vertical()
                            .id_salt("library-page")
                            .auto_shrink([false, false])
                            .show(ui, |ui| self.library_body(ui));
                    });
            });
    }

    fn play_entry(&mut self, path: PathBuf) {
        let same = self
            .session
            .as_ref()
            .map(|session| session.path == path)
            .unwrap_or(false);
        if same {
            self.view = View::Game;
            if let Some(session) = self.session.as_mut() {
                session.paused = false;
                session.last_tick = Instant::now();
            }
        } else {
            self.open_rom(path);
        }
    }

    fn remove_entry(&mut self, path: PathBuf) {
        self.settings.library.retain(|entry| entry.path != path);
        let same = self
            .session
            .as_ref()
            .map(|session| session.path == path)
            .unwrap_or(false);
        if same {
            self.session = None;
            self.audio.clear();
        }
        self.settings_dirty = true;
        self.save_settings();
    }

    fn library_body(&mut self, ui: &mut egui::Ui) {
        let entries = self.settings.library.clone();
        let count = entries.len();
        self.library_focus = self.library_focus.min(count.saturating_sub(1));

        // Keyboard navigation: arrows or A/D move, Enter plays.
        let mut moved_by_key = false;
        let mut confirm = false;
        if count > 0 {
            let (left, right, enter) = ui.input(|i| {
                (
                    i.key_pressed(egui::Key::ArrowLeft) || i.key_pressed(egui::Key::A),
                    i.key_pressed(egui::Key::ArrowRight) || i.key_pressed(egui::Key::D),
                    i.key_pressed(egui::Key::Enter),
                )
            });
            if left && self.library_focus > 0 {
                self.library_focus -= 1;
                moved_by_key = true;
            }
            if right && self.library_focus + 1 < count {
                self.library_focus += 1;
                moved_by_key = true;
            }
            confirm = enter;
        }

        let mut play: Option<PathBuf> = None;
        let mut remove: Option<PathBuf> = None;
        let mut import = false;

        // Hero: the focused game, large, with the main action.
        ui.add_space(8.0);
        if let Some(entry) = entries.get(self.library_focus) {
            let running = self
                .session
                .as_ref()
                .map(|session| session.path == entry.path)
                .unwrap_or(false);
            ui.label(
                RichText::new(if running { "NOW PLAYING" } else { "FEATURED" })
                    .size(12.0)
                    .color(ACCENT),
            );
            ui.label(
                RichText::new(truncate(&entry.title, 34))
                    .font(semibold(46.0))
                    .color(TEXT),
            );
            ui.label(
                RichText::new(format!(
                    "Mapper {} · {}   ·   {:.0} min played",
                    entry.mapper,
                    entry.mapper_name,
                    entry.play_seconds / 60.0
                ))
                .size(14.0)
                .color(MUTED),
            );
            ui.add_space(14.0);
            ui.horizontal(|ui| {
                let label = if running { "Resume" } else { "Play" };
                if ui.add_sized([150.0, 46.0], accent_button(label)).clicked() {
                    play = Some(entry.path.clone());
                }
                if ui.add(ghost_button("Add game")).clicked() {
                    import = true;
                }
                if ui.add(ghost_button("Remove")).clicked() {
                    remove = Some(entry.path.clone());
                }
            });
        } else {
            ui.label(RichText::new("Welcome").font(semibold(46.0)).color(TEXT));
            ui.label(
                RichText::new("Add a legally obtained .nes file, or drop one onto this window.")
                    .size(15.0)
                    .color(MUTED),
            );
            ui.add_space(14.0);
            if ui
                .add_sized([150.0, 46.0], accent_button("Add game"))
                .clicked()
            {
                import = true;
            }
        }

        ui.add_space(34.0);
        ui.label(RichText::new("Games").font(semibold(18.0)).color(TEXT));
        ui.add_space(8.0);

        // The shelf of tiles.
        let slot = Vec2::new(250.0, 340.0);
        egui::ScrollArea::horizontal()
            .id_salt("game-shelf")
            .auto_shrink([false, true])
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    for (index, entry) in entries.iter().enumerate() {
                        let texture = self.art.texture(ui.ctx(), &entry.path);
                        let (slot_rect, response) = ui.allocate_exact_size(slot, Sense::click());
                        if response.hovered() && ui.input(|i| i.pointer.delta().length() > 0.0) {
                            self.library_focus = index;
                        }
                        let focused = index == self.library_focus;
                        if focused && moved_by_key {
                            ui.scroll_to_rect(slot_rect, Some(Align::Center));
                        }
                        paint_game_tile(ui, slot_rect, entry, texture, focused);
                        if response.clicked() {
                            self.library_focus = index;
                            play = Some(entry.path.clone());
                        }
                    }

                    let (slot_rect, response) = ui.allocate_exact_size(slot, Sense::click());
                    paint_add_tile(ui, slot_rect, response.hovered());
                    if response.clicked() {
                        import = true;
                    }
                });
            });

        ui.add_space(10.0);
        ui.label(
            RichText::new(
                "NES Core ships with no ROMs or artwork. Only add games you are allowed to use.   \
                 Arrow keys or A/D to browse · Enter to play",
            )
            .size(11.0)
            .color(MUTED),
        );

        if confirm && play.is_none() {
            play = entries
                .get(self.library_focus)
                .map(|entry| entry.path.clone());
        }
        if import {
            self.open_rom_dialog();
        }
        if let Some(path) = remove {
            self.remove_entry(path);
        } else if let Some(path) = play {
            self.play_entry(path);
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
                            "0 NROM · 1 MMC1 · 2 UxROM · 3 CNROM · 4 MMC3 · 5 MMC5 · 7 AxROM · 9 MMC2 · 10 MMC4 · 11 Color Dreams · 16/159 Bandai FCG · 21/22/23/25 VRC2/VRC4 · 24/26 VRC6 · 33 Taito TC0190 · 34 BNROM/NINA-001 · 66 GxROM · 69 FME-7 · 70/152 Bandai · 71 Camerica · 75 VRC1 · 78 Irem · 79/113/146 NINA-03/06 · 85 VRC7 · 87 Jaleco · 88/154/206 Namco 118 · 89/93 Sunsoft-2 · 94 UN1ROM · 140 Jaleco · 180 UNROM · 184 Sunsoft-1 · 232 Quattro",
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
                    .fill(if fullscreen { Color32::BLACK } else { BG })
                    .inner_margin(egui::Margin::same(if fullscreen { 0 } else { 16 })),
            )
            .show(ui, |ui| {
                let Some(session) = self.session.as_mut() else {
                    return;
                };
                let available = ui.available_size();
                let bottom = if fullscreen { 0.0 } else { 46.0 };
                let pad = if fullscreen { 0.0 } else { 16.0 };
                let stage_height = (available.y - bottom).max(120.0);
                let (rect, _) =
                    ui.allocate_exact_size(Vec2::new(available.x, stage_height), Sense::hover());

                let ratio = if self.settings.pixel_aspect {
                    256.0 / 240.0 * (8.0 / 7.0)
                } else {
                    256.0 / 240.0
                };
                let mut width = rect.width() - pad;
                let mut height = width / ratio;
                if height > rect.height() - pad {
                    height = rect.height() - pad;
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
                    if height > rect.height() - pad {
                        let fit = ((rect.height() - pad) / 240.0).max(1.0);
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
                    .rect_filled(rect, SQUARE, Color32::from_rgb(0, 0, 0));
                if let Some(texture) = session.texture.as_ref() {
                    ui.painter().image(
                        texture.id(),
                        stage,
                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                        Color32::WHITE,
                    );
                }
                if !fullscreen {
                    ui.painter().rect_stroke(
                        stage,
                        SQUARE,
                        Stroke::new(1.0, BORDER),
                        egui::StrokeKind::Inside,
                    );
                }

                if session.paused {
                    ui.painter().rect_filled(
                        stage,
                        SQUARE,
                        Color32::from_rgba_unmultiplied(4, 7, 12, 170),
                    );
                    if !fullscreen {
                        ui.painter().text(
                            stage.center(),
                            Align2::CENTER_CENTER,
                            "PAUSED",
                            semibold(24.0),
                            TEXT,
                        );
                    }
                }

                if fullscreen {
                    return;
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
                                .corner_radius(RADIUS),
                            )
                            .clicked()
                        {
                            back = true;
                        }
                        ui.label(
                            RichText::new("Space pause · F2 reset · F5/F8 slots · F11 fullscreen · WASD move · K A · J B")
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
                                RADIUS,
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
                     RAM in memory, and 42 mappers (see the Import page for the list).",
                ),
                (
                    "Desktop features",
                    "Library, ROM import and drag-and-drop, save states, rebindable keyboard \
                     controls, audio volume, scaling options, fullscreen, pause, reset and \
                     performance readouts.",
                ),
                (
                    "Known limitations",
                    "No gamepad input, no rewinding, no netplay and no per-dot \
                     mid-scanline raster effects. MMC5 and VRC6 expansion audio, MMC5 fill \
                     mode and PAL timing are emulated; per-dot raster tricks are not. Some \
                     games are not compatible; see the notes in the repository.",
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
            self.draw_topbar(ui);
        }
        self.draw_view(ui);

        if !hide_chrome {
            self.draw_toast(&ctx);
        }

        if self.settings_dirty && self.last_settings_save.elapsed() > Duration::from_secs(2) {
            self.save_settings();
        }

        if let Some(session) = self.session.as_ref().filter(|session| !session.paused) {
            if session.fast_forward {
                ctx.request_repaint();
            } else {
                // Wake just in time for the next emulated frame instead of
                // repainting as fast as possible: this caps the app at the
                // console's frame rate (about 60 FPS) and saves CPU.
                let wait = session.next_wake.clamp(0.0005, 0.017);
                ctx.request_repaint_after(Duration::from_secs_f64(wait));
            }
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

/// Draws one game tile. The focused tile is larger and gets a bright outline
/// and glow; the others sit slightly smaller and dimmer.
fn paint_game_tile(
    ui: &mut egui::Ui,
    slot: egui::Rect,
    entry: &LibraryEntry,
    texture: Option<egui::TextureHandle>,
    focused: bool,
) {
    let painter = ui.painter_at(slot.expand(14.0));
    let size = if focused {
        Vec2::new(226.0, 268.0)
    } else {
        Vec2::new(198.0, 236.0)
    };
    let tile = egui::Rect::from_center_size(egui::pos2(slot.center().x, slot.top() + 150.0), size);
    let radius = egui::CornerRadius::same(18);

    if focused {
        for layer in 1..=6u8 {
            let grow = f32::from(layer) * 2.5;
            painter.rect_stroke(
                tile.expand(grow),
                egui::CornerRadius::same(18 + layer * 2),
                Stroke::new(
                    2.0,
                    Color32::from_rgba_unmultiplied(70, 140, 255, 44 - layer * 6),
                ),
                egui::StrokeKind::Outside,
            );
        }
    }

    let accent = artwork::accent_for(entry.rom_id);
    painter.rect_filled(tile, radius, accent.gamma_multiply(0.28));
    if let Some(texture) = texture {
        // Fill the portrait tile from the middle of the landscape cover.
        let aspect = tile.width() / tile.height();
        let source = texture.size_vec2();
        let visible = (aspect / (source.x / source.y)).clamp(0.1, 1.0);
        let u0 = (1.0 - visible) / 2.0;
        painter.image(
            texture.id(),
            tile.shrink(2.0),
            egui::Rect::from_min_max(egui::pos2(u0, 0.0), egui::pos2(u0 + visible, 1.0)),
            Color32::WHITE,
        );
    } else {
        painter.circle_filled(
            tile.center() + Vec2::new(0.0, -20.0),
            tile.width() * 0.34,
            accent.gamma_multiply(0.45),
        );
        painter.circle_filled(
            tile.center() + Vec2::new(0.0, -20.0),
            tile.width() * 0.22,
            accent.gamma_multiply(0.75),
        );
        let initial = entry
            .title
            .chars()
            .next()
            .unwrap_or('?')
            .to_uppercase()
            .to_string();
        painter.text(
            tile.center() + Vec2::new(0.0, -20.0),
            Align2::CENTER_CENTER,
            initial,
            semibold(if focused { 64.0 } else { 56.0 }),
            Color32::WHITE,
        );
        painter.text(
            tile.left_bottom() + Vec2::new(16.0, -14.0),
            Align2::LEFT_BOTTOM,
            "NES",
            semibold(12.0),
            Color32::from_white_alpha(150),
        );
    }
    if !focused {
        painter.rect_filled(tile, radius, Color32::from_black_alpha(70));
    }
    painter.rect_stroke(
        tile,
        radius,
        if focused {
            Stroke::new(3.0, Color32::WHITE)
        } else {
            Stroke::new(1.0, Color32::from_white_alpha(24))
        },
        egui::StrokeKind::Inside,
    );

    painter.text(
        egui::pos2(tile.center().x, tile.bottom() + 16.0),
        Align2::CENTER_TOP,
        truncate(&entry.title, 24),
        if focused {
            semibold(15.0)
        } else {
            FontId::proportional(13.0)
        },
        if focused { TEXT } else { MUTED },
    );
}

fn paint_add_tile(ui: &mut egui::Ui, slot: egui::Rect, hovered: bool) {
    let painter = ui.painter_at(slot.expand(14.0));
    let tile = egui::Rect::from_center_size(
        egui::pos2(slot.center().x, slot.top() + 150.0),
        Vec2::new(198.0, 236.0),
    );
    let radius = egui::CornerRadius::same(18);
    painter.rect_filled(
        tile,
        radius,
        Color32::from_white_alpha(if hovered { 22 } else { 10 }),
    );
    painter.rect_stroke(
        tile,
        radius,
        Stroke::new(
            if hovered { 2.0 } else { 1.0 },
            Color32::from_white_alpha(if hovered { 140 } else { 40 }),
        ),
        egui::StrokeKind::Inside,
    );
    painter.text(
        tile.center() + Vec2::new(0.0, -10.0),
        Align2::CENTER_CENTER,
        "+",
        semibold(56.0),
        if hovered { TEXT } else { MUTED },
    );
    painter.text(
        tile.center() + Vec2::new(0.0, 36.0),
        Align2::CENTER_CENTER,
        "Add game",
        FontId::proportional(14.0),
        if hovered { TEXT } else { MUTED },
    );
}
