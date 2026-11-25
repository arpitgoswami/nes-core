#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod audio;

use eframe::egui;
use nes_core::settings::Settings;

fn main() -> eframe::Result<()> {
    let settings = Settings::load();
    let mut wgpu_options = eframe::WgpuConfiguration::default();
    wgpu_options.surface.present_mode = if settings.vsync {
        eframe::wgpu::PresentMode::AutoVsync
    } else {
        eframe::wgpu::PresentMode::AutoNoVsync
    };
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1320.0, 860.0])
            .with_min_inner_size([1000.0, 660.0])
            .with_title("NES Core")
            .with_fullscreen(settings.fullscreen),
        wgpu_options,
        ..Default::default()
    };

    eframe::run_native(
        "NES Core",
        options,
        Box::new(move |cc| Ok(Box::new(app::NesApp::new(cc, settings)))),
    )
}
