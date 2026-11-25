//! Visual design tokens for the desktop UI.

use eframe::egui;
use egui::{Color32, CornerRadius, Stroke, TextStyle, Visuals};

pub const BG: Color32 = Color32::from_rgb(7, 10, 17);
pub const SIDEBAR: Color32 = Color32::from_rgb(11, 15, 24);
pub const PANEL: Color32 = Color32::from_rgb(14, 19, 30);
pub const CARD: Color32 = Color32::from_rgb(18, 24, 37);
pub const CARD_HOVER: Color32 = Color32::from_rgb(24, 33, 50);
pub const BORDER: Color32 = Color32::from_rgb(33, 43, 60);
pub const ACCENT: Color32 = Color32::from_rgb(38, 140, 255);
pub const ACCENT_DIM: Color32 = Color32::from_rgb(22, 66, 112);
pub const ACCENT_SOFT: Color32 = Color32::from_rgb(16, 34, 58);
pub const TEXT: Color32 = Color32::from_rgb(229, 236, 247);
pub const MUTED: Color32 = Color32::from_rgb(133, 148, 170);
pub const ERROR: Color32 = Color32::from_rgb(255, 106, 106);
pub const OK: Color32 = Color32::from_rgb(88, 214, 141);

pub const ZERO: CornerRadius = CornerRadius::ZERO;

pub fn apply(ctx: &egui::Context) {
    ctx.set_theme(egui::ThemePreference::Dark);
    let mut style = (*ctx.global_style()).clone();
    let mut visuals = Visuals::dark();
    visuals.panel_fill = BG;
    visuals.window_fill = PANEL;
    visuals.extreme_bg_color = Color32::from_rgb(8, 12, 20);
    visuals.faint_bg_color = CARD;
    visuals.window_stroke = Stroke::new(1.0, BORDER);
    visuals.window_corner_radius = ZERO;
    visuals.menu_corner_radius = ZERO;
    visuals.hyperlink_color = ACCENT;
    visuals.override_text_color = Some(TEXT);
    visuals.selection.bg_fill = ACCENT_DIM;
    visuals.selection.stroke = Stroke::new(1.0, TEXT);
    visuals.widgets.noninteractive.bg_fill = CARD;
    visuals.widgets.noninteractive.weak_bg_fill = CARD;
    visuals.widgets.noninteractive.bg_stroke = Stroke::NONE;
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, MUTED);
    visuals.widgets.inactive.bg_fill = CARD;
    visuals.widgets.inactive.weak_bg_fill = CARD;
    visuals.widgets.inactive.bg_stroke = Stroke::NONE;
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT);
    visuals.widgets.hovered.bg_fill = CARD_HOVER;
    visuals.widgets.hovered.weak_bg_fill = CARD_HOVER;
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, ACCENT_DIM);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, TEXT);
    visuals.widgets.hovered.expansion = 0.0;
    visuals.widgets.active.bg_fill = ACCENT_DIM;
    visuals.widgets.active.weak_bg_fill = ACCENT_DIM;
    visuals.widgets.active.bg_stroke = Stroke::new(1.0, ACCENT);
    visuals.widgets.active.fg_stroke = Stroke::new(1.0, TEXT);
    visuals.widgets.active.expansion = 0.0;
    for widget in [
        &mut visuals.widgets.noninteractive,
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
        &mut visuals.widgets.open,
    ] {
        widget.corner_radius = ZERO;
    }
    visuals.widgets.open.bg_fill = CARD;
    visuals.widgets.open.weak_bg_fill = CARD;
    visuals.widgets.open.bg_stroke = Stroke::new(1.0, BORDER);
    visuals.widgets.open.fg_stroke = Stroke::new(1.0, TEXT);
    style.visuals = visuals;
    style.spacing.item_spacing = egui::vec2(10.0, 10.0);
    style.spacing.button_padding = egui::vec2(14.0, 8.0);
    style.spacing.window_margin = egui::Margin::same(0);
    style.spacing.menu_margin = egui::Margin::same(6);
    style.text_styles.insert(
        TextStyle::Heading,
        egui::FontId::new(24.0, egui::FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Body,
        egui::FontId::new(14.0, egui::FontFamily::Proportional),
    );

    style.text_styles.insert(
        TextStyle::Button,
        egui::FontId::new(14.0, egui::FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Monospace,
        egui::FontId::new(13.0, egui::FontFamily::Monospace),
    );
    ctx.all_styles_mut(move |target| *target = style.clone());
}

pub fn accent_button(text: impl Into<String>) -> egui::Button<'static> {
    egui::Button::new(
        egui::RichText::new(text.into())
            .color(Color32::WHITE)
            .strong(),
    )
    .fill(ACCENT_DIM)
    .stroke(Stroke::new(1.0, ACCENT))
    .corner_radius(ZERO)
}

pub fn ghost_button(text: impl Into<String>) -> egui::Button<'static> {
    egui::Button::new(egui::RichText::new(text.into()).color(TEXT))
        .fill(CARD)
        .stroke(Stroke::new(1.0, BORDER))
        .corner_radius(ZERO)
}
