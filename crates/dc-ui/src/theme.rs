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
    // The shell is a light app (every palette constant above is the
    // Visio light look), so select the Light slot too. Without this,
    // native surfaces render dark: eframe on Linux reports no system
    // theme, egui's fallback is Dark, and the OS being in Light mode
    // never reaches the context — popups/menus went dark while the
    // custom chrome stayed light (GitHub #48 review).
    ctx.set_theme(egui::ThemePreference::Light);
}

/// Context-menu color tokens, resolved from the active visuals so the
/// popup follows a theme switch instead of assuming Visio-light
/// surroundings (GitHub #48). Light values are the observed Visio 2016
/// menu; dark values are our counterparts in the same green family,
/// chosen here once — never at the call sites that paint the menu.
#[derive(Clone, Copy)]
pub struct MenuTokens {
    /// Hovered-row fill (Visio light green; muted deep green in dark).
    pub hover_fill: Color32,
    /// Enabled-row label.
    pub label: Color32,
    /// Disabled-row label (the "v0.5" placeholders).
    pub label_disabled: Color32,
    /// Shortcut-hint text ("Ctrl+X", "Del").
    pub hint: Color32,
    /// Separator stroke (light rides the visuals' noninteractive
    /// stroke; dark lightens it so rules stay visible on the dark fill).
    pub separator: Color32,
    /// Whether the tokens resolved for dark visuals (drives icon tints).
    pub dark: bool,
}

impl MenuTokens {
    /// Tint for an MDI icon glyph on the menu. Light keeps the Office
    /// palette; dark lifts each color so its hue survives the dark
    /// fill — e.g. the gray ShapeSheet glyph must not vanish (the
    /// review gap that kept #48 open).
    pub fn icon_tint(&self, office: Color32, enabled: bool) -> Color32 {
        if !enabled {
            return if self.dark {
                Color32::from_gray(150)
            } else {
                Color32::from_gray(178)
            };
        }
        if !self.dark {
            return office;
        }
        lift_for_dark(office)
    }
}

/// Scale a light-palette color up so its brightest channel reaches
/// ~200 — presence on a ~gray(50) menu fill while keeping the hue.
fn lift_for_dark(c: Color32) -> Color32 {
    let m = c.r().max(c.g()).max(c.b()) as f32;
    if m >= 200.0 {
        return c;
    }
    let s = 200.0 / m.max(1.0);
    Color32::from_rgb(
        ((c.r() as f32) * s).min(255.0) as u8,
        ((c.g() as f32) * s).min(255.0) as u8,
        ((c.b() as f32) * s).min(255.0) as u8,
    )
}

/// Resolve menu tokens for the active visuals.
pub fn menu_tokens(v: &Visuals) -> MenuTokens {
    if v.dark_mode {
        MenuTokens {
            hover_fill: Color32::from_rgb(43, 74, 56),
            label: v.text_color(),
            label_disabled: Color32::from_gray(135),
            hint: Color32::from_gray(150),
            separator: Color32::from_gray(95),
            dark: true,
        }
    } else {
        MenuTokens {
            hover_fill: Color32::from_rgb(206, 227, 212),
            label: Color32::from_rgb(30, 30, 30),
            label_disabled: Color32::from_gray(160),
            hint: Color32::from_gray(140),
            separator: v.widgets.noninteractive.bg_stroke.color,
            dark: false,
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

        // Icons: light keeps the Office palette; dark lifts it enough
        // to read on the dark fill while keeping the hue (the #48
        // review gap — the gray ShapeSheet glyph vanished on dark).
        let shapesheet_gray = Color32::from_rgb(66, 66, 66);
        assert_eq!(light.icon_tint(shapesheet_gray, true), shapesheet_gray);
        let lifted = dark.icon_tint(shapesheet_gray, true);
        assert!(
            lifted.r() >= 200 && lifted.r() == lifted.g() && lifted.g() == lifted.b(),
            "gray glyphs lift to a readable gray, got {lifted:?}"
        );
        let blue = Color32::from_rgb(42, 111, 181);
        let lb = dark.icon_tint(blue, true);
        assert!(
            lb.b() >= 200 && lb.b() >= lb.g() && lb.g() >= lb.r(),
            "hue order preserved on lift, got {lb:?}"
        );
        assert_ne!(dark.icon_tint(blue, false), lb, "disabled tints dim");
        assert_eq!(light.icon_tint(blue, false), Color32::from_gray(178));

        // Separators stay visible on their fill.
        assert_eq!(
            light.separator,
            Visuals::light().widgets.noninteractive.bg_stroke.color
        );
        assert!(
            lum(dark.separator) > lum(dark_vis.window_fill) + 20.0,
            "dark separators {:?} must clear the dark fill",
            dark.separator
        );
    }
}
