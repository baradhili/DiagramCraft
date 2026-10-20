//! Visio 2016 visual language — the cleanroom palette and sizing for the
//! shell (reimplemented from observed behavior; no Microsoft assets).

use egui::{Color32, Style, Visuals};

/// Visio brand green (2016 accent).
pub const ACCENT: Color32 = Color32::from_rgb(33, 115, 70);
/// Darker accent for hover/pressed states.
pub const ACCENT_DARK: Color32 = Color32::from_rgb(23, 84, 51);
/// Ribbon content background (Office 2016 light shell).
pub const RIBBON_BG: Color32 = Color32::from_rgb(245, 245, 245);
/// Ribbon group label / separators.
pub const RIBBON_GROUP: Color32 = Color32::from_rgb(214, 214, 214);
pub const RIBBON_LABEL: Color32 = Color32::from_rgb(95, 95, 95);
/// Canvas (desk) background around the page.
pub const DESK: Color32 = Color32::from_rgb(200, 200, 200);
/// Page outline.
pub const PAGE_EDGE: Color32 = Color32::from_rgb(140, 140, 140);
/// Selection frame + handles (Visio 2016 selection is a thin blue).
pub const SELECTION: Color32 = Color32::from_rgb(0, 112, 192);
/// Status bar: light gray band with dark text (Visio 2016; the green
/// is only the title bar and File tab).
pub const STATUS_BG: Color32 = Color32::from_rgb(238, 238, 238);
pub const STATUS_TEXT: Color32 = Color32::from_rgb(60, 60, 60);
/// Stencil pane background.
pub const STENCIL_BG: Color32 = Color32::from_rgb(252, 252, 252);
/// Ruler background and text (Office light rulers).
pub const RULER_BG: Color32 = Color32::from_rgb(243, 243, 243);
pub const RULER_TICK: Color32 = Color32::from_rgb(130, 130, 130);
pub const RULER_TEXT: Color32 = Color32::from_rgb(90, 90, 90);
/// Page-tab strip.
pub const TABS_BG: Color32 = Color32::from_rgb(225, 225, 225);
pub const TAB_ACTIVE: Color32 = Color32::from_rgb(255, 255, 255);

/// Apply the Office-2016-ish light visuals.
pub fn install(ctx: &egui::Context) {
    let mut style = Style::default();
    let mut v = Visuals::light();
    v.panel_fill = RIBBON_BG;
    v.window_fill = Color32::from_rgb(250, 250, 250);
    v.extreme_bg_color = Color32::WHITE;
    v.faint_bg_color = Color32::from_rgb(240, 240, 240);
    // widget state palette via craft-ui (local fork, egui 0.37)
    craft_ui::theme::WidgetPalette {
        noninteractive: RIBBON_BG,
        inactive: RIBBON_BG,
        hovered: Color32::from_rgb(228, 240, 233),
        active: Color32::from_rgb(210, 230, 217),
        open: Color32::from_rgb(210, 230, 217),
    }
    .apply_to(
        &mut v.widgets,
        egui::CornerRadius::same(3),
        egui::Stroke::new(1.0, Color32::from_rgb(60, 60, 60)),
    );
    v.selection.bg_fill = Color32::from_rgba_premultiplied(33, 115, 70, 36);
    v.selection.stroke.color = ACCENT;
    style.visuals = v;
    style.spacing.item_spacing = egui::vec2(6.0, 4.0);
    style.spacing.button_padding = egui::vec2(6.0, 3.0);
    ctx.set_style_of(egui::Theme::Light, style);
}
