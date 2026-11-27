//! Visual design tokens for the desktop UI.

use eframe::egui;
use egui::{Color32, CornerRadius, Stroke, TextStyle, Visuals};

pub const BG: Color32 = Color32::from_rgb(10, 11, 15);
pub const SIDEBAR: Color32 = Color32::from_rgb(14, 15, 20);
pub const PANEL: Color32 = Color32::from_rgb(17, 18, 24);
pub const CARD: Color32 = Color32::from_rgb(22, 24, 31);
pub const CARD_HOVER: Color32 = Color32::from_rgb(30, 33, 43);
pub const BORDER: Color32 = Color32::from_rgb(38, 41, 52);
pub const ACCENT: Color32 = Color32::from_rgb(99, 102, 241);
pub const ACCENT_DIM: Color32 = Color32::from_rgb(55, 57, 140);
pub const ACCENT_SOFT: Color32 = Color32::from_rgb(27, 28, 56);
pub const TEXT: Color32 = Color32::from_rgb(236, 238, 245);
pub const MUTED: Color32 = Color32::from_rgb(139, 144, 160);
pub const ERROR: Color32 = Color32::from_rgb(255, 106, 106);
pub const OK: Color32 = Color32::from_rgb(88, 214, 141);

/// Corner radius used for cards, buttons and panels.
pub const RADIUS: CornerRadius = CornerRadius::same(8);
/// Square corners, used for the game picture so no pixels are clipped.
pub const SQUARE: CornerRadius = CornerRadius::ZERO;

/// Registers the bundled Inter typeface (SIL OFL, see `assets/fonts`).
fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "inter".into(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!(
            "../../assets/fonts/Inter-Regular.ttf"
        ))),
    );
    fonts.font_data.insert(
        "inter-semibold".into(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!(
            "../../assets/fonts/Inter-SemiBold.ttf"
        ))),
    );
    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .insert(0, "inter".into());
    fonts.families.insert(
        egui::FontFamily::Name("semibold".into()),
        vec!["inter-semibold".into(), "inter".into()],
    );
    ctx.set_fonts(fonts);
}

/// Bold-looking text for titles, using the semibold Inter cut.
pub fn semibold(size: f32) -> egui::FontId {
    egui::FontId::new(size, egui::FontFamily::Name("semibold".into()))
}

pub fn apply(ctx: &egui::Context) {
    install_fonts(ctx);
    ctx.set_theme(egui::ThemePreference::Dark);
    let mut style = (*ctx.global_style()).clone();
    let mut visuals = Visuals::dark();
    visuals.panel_fill = BG;
    visuals.window_fill = PANEL;
    visuals.extreme_bg_color = Color32::from_rgb(9, 10, 14);
    visuals.faint_bg_color = CARD;
    visuals.window_stroke = Stroke::new(1.0, BORDER);
    visuals.window_corner_radius = RADIUS;
    visuals.menu_corner_radius = RADIUS;
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
        widget.corner_radius = RADIUS;
    }
    visuals.widgets.open.bg_fill = CARD;
    visuals.widgets.open.weak_bg_fill = CARD;
    visuals.widgets.open.bg_stroke = Stroke::new(1.0, BORDER);
    visuals.widgets.open.fg_stroke = Stroke::new(1.0, TEXT);
    style.visuals = visuals;
    style.spacing.item_spacing = egui::vec2(10.0, 10.0);
    style.spacing.interact_size.y = 32.0;
    style.spacing.button_padding = egui::vec2(14.0, 8.0);
    style.spacing.window_margin = egui::Margin::same(0);
    style.spacing.menu_margin = egui::Margin::same(6);
    style.text_styles.insert(
        TextStyle::Heading,
        egui::FontId::new(24.0, egui::FontFamily::Name("semibold".into())),
    );
    style.text_styles.insert(
        TextStyle::Body,
        egui::FontId::new(14.5, egui::FontFamily::Proportional),
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
    .corner_radius(RADIUS)
}

pub fn ghost_button(text: impl Into<String>) -> egui::Button<'static> {
    egui::Button::new(egui::RichText::new(text.into()).color(TEXT))
        .fill(CARD)
        .stroke(Stroke::new(1.0, BORDER))
        .corner_radius(RADIUS)
}
