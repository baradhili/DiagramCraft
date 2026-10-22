//! Visio 2016 visual language — the cleanroom palette and sizing for the
//! shell (reimplemented from observed behavior; no Microsoft assets).

use egui::{Color32, Style, Visuals};

/// Visio brand green (2016 accent).
pub const ACCENT: Color32 = Color32::from_rgb(33, 115, 70);
/// Darker accent for hover/pressed states.
pub const ACCENT_DARK: Color32 = Color32::from_rgb(23, 84, 51);
/// Ribbon content background (Office 2016 light shell).
pub const RIBBON_BG: Color32 = Color32::from_rgb(245, 245, 245);
/// Tab strip + ribbon content surface (Office 2016 "Colorful": white
/// under the colored title band, observed in Visio 2016 shots).
pub const RIBBON_CONTENT: Color32 = Color32::WHITE;
/// Ribbon hairlines: bottom rule and control borders (#C9C9C9).
pub const RIBBON_EDGE: Color32 = Color32::from_rgb(201, 201, 201);
/// Ribbon-button hover chip (Office 2016 flat neutral hover).
pub const RIBBON_HOVER: Color32 = Color32::from_rgb(232, 232, 232);
pub const RIBBON_HOVER_EDGE: Color32 = Color32::from_rgb(198, 198, 198);
/// Ribbon-button toggled/selected chip (slightly deeper than hover).
pub const RIBBON_SELECTED: Color32 = Color32::from_rgb(220, 220, 220);
pub const RIBBON_SELECTED_EDGE: Color32 = Color32::from_rgb(178, 178, 178);
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

/// Context-menu color tokens, resolved from the active visuals so the
/// popup follows a theme switch instead of assuming Visio-light
/// surroundings (GitHub #48). Light values are the observed Visio 2016
/// menu; dark values are our counterparts in the same green family,
/// chosen here once — never at the call sites that paint the menu.
pub struct MenuTokens {
    /// Hovered-row fill (Visio light green; muted deep green in dark).
    pub hover_fill: Color32,
    /// Enabled-row label.
    pub label: Color32,
    /// Disabled-row label (the "v0.5" placeholders).
    pub label_disabled: Color32,
    /// Shortcut-hint text ("Ctrl+X", "Del").
    pub hint: Color32,
}

/// Resolve menu tokens for the active visuals. Separators already ride
/// `visuals.widgets.noninteractive.bg_stroke` and need no token.
pub fn menu_tokens(v: &Visuals) -> MenuTokens {
    if v.dark_mode {
        MenuTokens {
            hover_fill: Color32::from_rgb(43, 74, 56),
            label: v.text_color(),
            label_disabled: Color32::from_gray(110),
            hint: Color32::from_gray(130),
        }
    } else {
        MenuTokens {
            hover_fill: Color32::from_rgb(206, 227, 212),
            label: Color32::from_rgb(30, 30, 30),
            label_disabled: Color32::from_gray(160),
            hint: Color32::from_gray(140),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lum(c: Color32) -> f32 {
        0.2126 * c.r() as f32 + 0.7152 * c.g() as f32 + 0.0722 * c.b() as f32
    }

    /// GitHub #48: light tokens pin the observed Visio 2016 menu, dark
    /// tokens must be readable counterparts — brighter labels than the
    /// hover fill, a hover fill that stands off the dark panel, and a
    /// dimmer disabled label than an enabled one.
    #[test]
    fn menu_tokens_follow_the_active_visuals() {
        let light = menu_tokens(&Visuals::light());
        assert_eq!(light.hover_fill, Color32::from_rgb(206, 227, 212));
        assert_eq!(light.label, Color32::from_rgb(30, 30, 30));
        assert_eq!(light.label_disabled, Color32::from_gray(160));
        assert_eq!(light.hint, Color32::from_gray(140));

        let dark_vis = Visuals::dark();
        let dark = menu_tokens(&dark_vis);
        assert_ne!(
            dark.hover_fill, light.hover_fill,
            "hover has a dark counterpart"
        );
        assert!(
            lum(dark.hover_fill) > lum(dark_vis.window_fill) + 15.0,
            "dark hover {:?} must stand off the dark panel {:?}",
            dark.hover_fill,
            dark_vis.window_fill
        );
        assert!(
            lum(dark.label) > lum(dark.hover_fill),
            "dark labels must read on the dark hover fill"
        );
        assert!(
            lum(dark.label_disabled) < lum(dark.label),
            "disabled rows dim below enabled rows"
        );
    }
}
