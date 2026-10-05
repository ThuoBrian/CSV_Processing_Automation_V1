//! Single source of truth for PrintLedger's visual design: brand colors,
//! spacing/rounding tokens, and small reusable layout helpers (status
//! banners, cards, button styles) used by every screen.

use eframe::egui::{self, Color32, CornerRadius, FontId, RichText, Stroke, TextStyle, Vec2};

pub const PRIMARY: Color32 = Color32::from_rgb(37, 99, 235);
pub const SUCCESS: Color32 = Color32::from_rgb(22, 163, 74);
pub const ERROR: Color32 = Color32::from_rgb(220, 38, 38);
pub const WARNING: Color32 = Color32::from_rgb(217, 119, 6);
pub const MUTED: Color32 = Color32::from_gray(140);

const CORNER_RADIUS: u8 = 8;
const BUTTON_PADDING: Vec2 = Vec2::new(14.0, 8.0);
const ITEM_SPACING: Vec2 = Vec2::new(8.0, 10.0);
const CARD_MARGIN: i8 = 14;

/// Space between major sections/cards on a screen.
pub const SECTION_SPACING: f32 = 20.0;

/// Called once from `main.rs` via `cc.egui_ctx` before the app is built.
/// Starts from whichever base (dark/light) eframe already auto-detected
/// from the OS, then layers brand/rounding/spacing on top.
pub fn apply(ctx: &egui::Context) {
    let dark_mode = ctx.style().visuals.dark_mode;
    let mut visuals = if dark_mode {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };

    visuals.selection.bg_fill = PRIMARY.linear_multiply(0.55);
    visuals.selection.stroke = Stroke::new(1.0, PRIMARY);
    visuals.hyperlink_color = PRIMARY;
    visuals.window_corner_radius = CornerRadius::same(CORNER_RADIUS);
    visuals.menu_corner_radius = CornerRadius::same(CORNER_RADIUS);
    for w in [
        &mut visuals.widgets.noninteractive,
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
        &mut visuals.widgets.open,
    ] {
        w.corner_radius = CornerRadius::same(CORNER_RADIUS);
    }

    let mut style = (*ctx.style()).clone();
    style.visuals = visuals;
    style.spacing.item_spacing = ITEM_SPACING;
    style.spacing.button_padding = BUTTON_PADDING;
    style.spacing.window_margin = CARD_MARGIN.into();
    style
        .text_styles
        .insert(TextStyle::Heading, FontId::proportional(22.0));
    style
        .text_styles
        .insert(TextStyle::Body, FontId::proportional(15.0));
    style
        .text_styles
        .insert(TextStyle::Button, FontId::proportional(15.0));
    style
        .text_styles
        .insert(TextStyle::Small, FontId::proportional(12.0));

    ctx.set_style(style);
}

pub enum Status {
    Success,
    Error,
    Warning,
}

impl Status {
    fn color(&self) -> Color32 {
        match self {
            Status::Success => SUCCESS,
            Status::Error => ERROR,
            Status::Warning => WARNING,
        }
    }
    fn glyph(&self) -> &'static str {
        match self {
            Status::Success => "\u{2705}",
            Status::Warning => "\u{26A0}",
            Status::Error => "\u{274C}",
        }
    }
}

/// Replaces ad hoc `ui.colored_label(Color32::..., ...)` calls with one
/// consistent treatment: a subtly tinted, bordered, rounded frame with a
/// leading status glyph.
pub fn banner(ui: &mut egui::Ui, status: Status, text: &str) {
    let color = status.color();
    egui::Frame::new()
        .fill(color.linear_multiply(0.12))
        .stroke(Stroke::new(1.0, color))
        .corner_radius(CornerRadius::same(CORNER_RADIUS))
        .inner_margin(CARD_MARGIN)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(status.glyph());
                ui.colored_label(color, text);
            });
        });
}

/// Wraps `add_contents` in a bordered, padded, rounded card to visually
/// group one logical section of a screen. Generic over the closure's
/// return value since some call sites need to produce one (e.g. an output
/// path) from inside the card.
pub fn card<R>(ui: &mut egui::Ui, add_contents: impl FnOnce(&mut egui::Ui) -> R) -> R {
    egui::Frame::new()
        .fill(ui.visuals().faint_bg_color)
        .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
        .corner_radius(CornerRadius::same(CORNER_RADIUS))
        .inner_margin(CARD_MARGIN)
        .show(ui, add_contents)
        .inner
}

/// A visually de-emphasized "back" action: small muted borderless
/// text-button, for top-left placement.
pub fn back_button(ui: &mut egui::Ui, text: &str) -> bool {
    ui.add(egui::Button::new(RichText::new(text).color(MUTED).size(13.0)).frame(false))
        .clicked()
}

/// A visually primary action: brand-colored fill, white bold text, used for
/// "Next"/"Process"/"Confirm" across all screens. Returns the `Response` so
/// callers can use `.clicked()` or wrap it in `ui.add_enabled`.
pub fn primary_button(ui: &mut egui::Ui, text: &str) -> egui::Response {
    ui.add(primary_button_widget(text))
}

/// Constructs the same primary-button styling as [`primary_button`] but
/// returns the unattached `Button` widget rather than rendering it. Needed
/// at call sites that use `ui.add_enabled`, which takes a `Widget` value
/// rather than a pre-rendered `Response`.
pub fn primary_button_widget(text: &str) -> egui::Button<'_> {
    egui::Button::new(RichText::new(text).color(Color32::WHITE).strong()).fill(PRIMARY)
}
