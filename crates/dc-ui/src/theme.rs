//! Visio 2016 visual language — the cleanroom palette and sizing for the
//! shell (reimplemented from observed behavior; no Microsoft assets).

use egui::{Color32, Style, Visuals};

/// The GENERAL CHROME grey — owner-selected warm gray #817e7b
/// (2026-10-27). After the 2026-10-29 re-basing (#154) it fills
/// chrome only (File-tab block, comment badges, the backstage rail
/// fill); every highlight indicator is the blue below.
pub const ACCENT: Color32 = Color32::from_rgb(0x81, 0x7e, 0x7b);
/// Darker chrome grey for pressed chrome (the grey at ~72%).
pub const ACCENT_DARK: Color32 = Color32::from_rgb(92, 90, 88);
/// The HIGHLIGHT blue — the base of the UI palette (#154, owner
/// 2026-10-29): selection frames and handles, the active-tab
/// underline, armed states, ribbon accent lines, and the backstage
/// rail's selected band. Highlights are blue; chrome is grey.
pub const HIGHLIGHT: Color32 = Color32::from_rgb(0x2e, 0x74, 0xb5);
/// Hover/pressed companion (the highlight blue at ~72%).
pub const HIGHLIGHT_DARK: Color32 = Color32::from_rgb(0x21, 0x53, 0x81);
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
pub const SELECTION: Color32 = HIGHLIGHT;
/// Status bar: light gray band with dark text (Visio 2016; the accent
/// fill is only the title bar and File tab).
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
/// Backstage (File screen) rail: the selected destination's band on
/// the grey rail (Visio 2010 three-band rail, GitHub #55) — the
/// highlight blue's pressed companion so it reads on the grey fill
/// (#154: selections are blue).
pub const RAIL_SELECTED: Color32 = HIGHLIGHT_DARK;
pub const RAIL_DISABLED: Color32 = Color32::from_gray(160);
/// Backstage page headline text.
pub const BACK_TITLE: Color32 = Color32::from_rgb(50, 50, 50);
/// Backstage gallery card fill (edge rides RIBBON_GROUP, hover RIBBON_BG).
pub const CARD_BG: Color32 = Color32::WHITE;

/// Shell chrome tokens for the active visual mode (GitHub #52).
///
/// One field per palette constant above, resolved from the live
/// visuals the way [`MenuTokens`] is: light values are the constants
/// verbatim (the observed Visio 2016 look); dark values are our
/// counterparts, chosen here once — never at the call sites that
/// paint the chrome. The page *paper* is not part of this mapping:
/// it stays white in dark mode (the document, not the shell).
#[derive(Clone, Copy)]
pub struct ShellTokens {
    /// The title/File/rail band fill — the general chrome grey
    /// (#817e7b) in both modes (Office keeps brand bands in dark
    /// themes).
    pub accent_band: Color32,
    /// The HIGHLIGHT blue (#2e74b5) for lines and small elements —
    /// tab underline, launcher hover, armed stencil border,
    /// selections; lifted in dark so it reads on dark fills.
    pub accent: Color32,
    /// The highlight blue's pressed/hover companion.
    pub accent_dark: Color32,
    pub ribbon_bg: Color32,
    pub ribbon_content: Color32,
    pub ribbon_edge: Color32,
    pub ribbon_hover: Color32,
    pub ribbon_hover_edge: Color32,
    pub ribbon_selected: Color32,
    pub ribbon_selected_edge: Color32,
    pub ribbon_group: Color32,
    pub ribbon_label: Color32,
    /// Ribbon-button label (enabled).
    pub label: Color32,
    /// Ribbon-button label (disabled placeholders).
    pub label_disabled: Color32,
    /// Desk around the page.
    pub desk: Color32,
    pub page_edge: Color32,
    /// Selection frame / marquee.
    pub selection: Color32,
    pub status_bg: Color32,
    pub status_text: Color32,
    pub stencil_bg: Color32,
    /// Stencil master caption.
    pub caption: Color32,
    pub ruler_bg: Color32,
    pub ruler_tick: Color32,
    pub ruler_text: Color32,
    pub tabs_bg: Color32,
    pub tab_active: Color32,
    /// Page-tab text on the active tab.
    pub tab_text_active: Color32,
    /// Page-tab text on inactive tabs.
    pub tab_text_inactive: Color32,
    /// Ribbon tab-strip text on inactive tabs.
    pub tab_strip_text: Color32,
    pub rail_selected: Color32,
    pub rail_disabled: Color32,
    pub back_title: Color32,
    pub card_bg: Color32,
    /// The "Tell me" search box fill.
    pub tellme_bg: Color32,
    /// Muted hint text (the Tell-Me placeholder).
    pub hint_text: Color32,
}

/// The light mapping: the constants above, verbatim.
pub const LIGHT: ShellTokens = ShellTokens {
    accent_band: ACCENT,
    accent: HIGHLIGHT,
    accent_dark: HIGHLIGHT_DARK,
    ribbon_bg: RIBBON_BG,
    ribbon_content: RIBBON_CONTENT,
    ribbon_edge: RIBBON_EDGE,
    ribbon_hover: RIBBON_HOVER,
    ribbon_hover_edge: RIBBON_HOVER_EDGE,
    ribbon_selected: RIBBON_SELECTED,
    ribbon_selected_edge: RIBBON_SELECTED_EDGE,
    ribbon_group: RIBBON_GROUP,
    ribbon_label: RIBBON_LABEL,
    label: Color32::from_rgb(50, 50, 50),
    label_disabled: Color32::from_rgb(160, 160, 160),
    desk: DESK,
    page_edge: PAGE_EDGE,
    selection: SELECTION,
    status_bg: STATUS_BG,
    status_text: STATUS_TEXT,
    stencil_bg: STENCIL_BG,
    caption: Color32::from_rgb(60, 60, 60),
    ruler_bg: RULER_BG,
    ruler_tick: RULER_TICK,
    ruler_text: RULER_TEXT,
    tabs_bg: TABS_BG,
    tab_active: TAB_ACTIVE,
    tab_text_active: Color32::from_rgb(30, 30, 30),
    tab_text_inactive: Color32::from_rgb(90, 90, 90),
    tab_strip_text: Color32::from_rgb(70, 70, 70),
    rail_selected: RAIL_SELECTED,
    rail_disabled: RAIL_DISABLED,
    back_title: BACK_TITLE,
    card_bg: CARD_BG,
    tellme_bg: Color32::WHITE,
    hint_text: Color32::from_rgb(130, 130, 130),
};

/// The dark mapping: neutral surfaces in the Office dark family with
/// the same warm-gray accents lifted to read on them (GitHub #52).
pub const DARK: ShellTokens = ShellTokens {
    accent_band: ACCENT,
    // lift_for_dark precomputed (it is not const-callable): the
    // highlight blue lifted so its strongest channel hits 200.
    accent: Color32::from_rgb(50, 128, 200),
    accent_dark: Color32::from_rgb(51, 128, 200),
    ribbon_bg: Color32::from_gray(43),
    ribbon_content: Color32::from_gray(54),
    ribbon_edge: Color32::from_gray(86),
    ribbon_hover: Color32::from_gray(66),
    ribbon_hover_edge: Color32::from_gray(104),
    ribbon_selected: Color32::from_gray(74),
    ribbon_selected_edge: Color32::from_gray(122),
    ribbon_group: Color32::from_gray(88),
    ribbon_label: Color32::from_gray(168),
    label: Color32::from_gray(225),
    label_disabled: Color32::from_gray(125),
    desk: Color32::from_gray(52),
    page_edge: Color32::from_gray(120),
    selection: Color32::from_rgb(96, 172, 240),
    status_bg: Color32::from_gray(38),
    status_text: Color32::from_gray(205),
    stencil_bg: Color32::from_gray(47),
    caption: Color32::from_gray(200),
    ruler_bg: Color32::from_gray(36),
    ruler_tick: Color32::from_gray(130),
    ruler_text: Color32::from_gray(180),
    tabs_bg: Color32::from_gray(31),
    tab_active: Color32::from_gray(74),
    tab_text_active: Color32::from_gray(240),
    tab_text_inactive: Color32::from_gray(150),
    tab_strip_text: Color32::from_gray(185),
    rail_selected: Color32::from_rgb(70, 180, 128),
    rail_disabled: RAIL_DISABLED,
    back_title: Color32::from_gray(230),
    card_bg: Color32::from_gray(60),
    tellme_bg: Color32::from_gray(30),
    hint_text: Color32::from_gray(150),
};

/// Resolve shell tokens for the active visuals (GitHub #52).
pub fn shell_tokens(v: &Visuals) -> ShellTokens {
    if v.dark_mode {
        DARK
    } else {
        LIGHT
    }
}

/// Apply the light Visio style and select it (host loops that pin the
/// shell light, e.g. the kittest harness).
pub fn install(ctx: &egui::Context) {
    install_for(ctx, egui::ThemePreference::Light);
}

/// Install BOTH style slots — light = the Visio 2016 look, dark = the
/// counterpart above — and select `pref` (GitHub #52). With both slots
/// filled, switching the preference at runtime restyles native surfaces
/// and the menu mapping together; eframe on Linux reports no system
/// theme and egui's fallback is Dark, so the preference must come from
/// OS detection (`crate::os_theme`) instead of the integration (#48).
pub fn install_for(ctx: &egui::Context, pref: egui::ThemePreference) {
    let mut style = Style::default();
    let mut v = Visuals::light();
    v.panel_fill = RIBBON_BG;
    v.window_fill = Color32::from_rgb(250, 250, 250);
    v.extreme_bg_color = Color32::WHITE;
    v.faint_bg_color = Color32::from_rgb(240, 240, 240);
    // widget state palette via craft-ui (local fork, egui 0.37) —
    // every state tints toward HIGHLIGHT (#154/#165: egui's stock
    // greens must not survive into dialog buttons and menus)
    craft_ui::theme::WidgetPalette {
        noninteractive: RIBBON_BG,
        inactive: RIBBON_BG,
        hovered: Color32::from_rgb(228, 238, 248),
        active: Color32::from_rgb(209, 228, 243),
        open: Color32::from_rgb(209, 228, 243),
    }
    .apply_to(
        &mut v.widgets,
        egui::CornerRadius::same(3),
        egui::Stroke::new(1.0, Color32::from_rgb(60, 60, 60)),
    );
    // Selection fill = HIGHLIGHT (46,116,181) at 14% — the green
    // (33,115,70) it replaces was egui's stock light selection
    // (#165); this is the fill text/list selection shows in the
    // close prompt and the file-select surfaces.
    v.selection.bg_fill = Color32::from_rgba_premultiplied(6, 16, 26, 36);
    v.selection.stroke.color = HIGHLIGHT;
    style.visuals = v;
    ctx.set_style_of(egui::Theme::Light, style);

    let mut style = Style::default();
    let mut v = Visuals::dark();
    v.panel_fill = DARK.ribbon_bg;
    v.window_fill = Color32::from_gray(50);
    v.extreme_bg_color = Color32::from_gray(30);
    v.faint_bg_color = Color32::from_gray(52);
    craft_ui::theme::WidgetPalette {
        noninteractive: Color32::from_gray(47),
        inactive: DARK.ribbon_content,
        // HIGHLIGHT_DARK-tinted states (#165) — the blues that
        // replace egui's stock dark greens at the same lightness.
        hovered: Color32::from_rgb(44, 66, 92),
        active: Color32::from_rgb(38, 58, 82),
        open: Color32::from_rgb(38, 58, 82),
    }
    .apply_to(
        &mut v.widgets,
        egui::CornerRadius::same(3),
        egui::Stroke::new(1.0, Color32::from_gray(150)),
    );
    // Selection fill = the highlight blue at 18% (the stock dark
    // green (56,167,110) it replaces, #165).
    v.selection.bg_fill = Color32::from_rgba_premultiplied(8, 21, 33, 46);
    v.selection.stroke.color = DARK.accent_dark;
    style.visuals = v;
    ctx.set_style_of(egui::Theme::Dark, style);

    // Shared sizing so the modes differ only in color.
    for theme in [egui::Theme::Light, egui::Theme::Dark] {
        ctx.style_mut_of(theme, |style| {
            style.spacing.item_spacing = egui::vec2(6.0, 4.0);
            style.spacing.button_padding = egui::vec2(6.0, 3.0);
        });
    }
    ctx.set_theme(pref);
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
/// `pub(crate)`: the ribbon re-tints its icons for dark mode the same
/// way the menu does (GitHub #52).
pub(crate) fn lift_for_dark(c: Color32) -> Color32 {
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
            // HIGHLIGHT_DARK-tinted hover (#165), not egui's green.
            hover_fill: Color32::from_rgb(43, 66, 94),
            label: v.text_color(),
            label_disabled: Color32::from_gray(135),
            hint: Color32::from_gray(150),
            separator: Color32::from_gray(95),
            dark: true,
        }
    } else {
        MenuTokens {
            // Pale HIGHLIGHT wash (#165), not egui's pale green.
            hover_fill: Color32::from_rgb(206, 224, 239),
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
        assert_eq!(light.hover_fill, Color32::from_rgb(206, 224, 239));
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

    /// GitHub #52: the light token set pins the observed Visio 2016
    /// constants verbatim (goldens must not move), and the dark set is
    /// a readable counterpart — text clears fills, chips stand off
    /// their surfaces, and accents survive the dark background.
    #[test]
    fn shell_tokens_pin_light_and_lift_dark() {
        let light = shell_tokens(&Visuals::light());
        assert_eq!(light.ribbon_bg, RIBBON_BG);
        assert_eq!(light.ribbon_content, RIBBON_CONTENT);
        assert_eq!(light.desk, DESK);
        assert_eq!(light.status_bg, STATUS_BG);
        assert_eq!(light.tabs_bg, TABS_BG);
        assert_eq!(light.card_bg, CARD_BG);
        assert_eq!(light.tellme_bg, Color32::WHITE);
        assert_eq!(light.accent, HIGHLIGHT);
        assert_eq!(light.accent_band, ACCENT);

        let dark = shell_tokens(&Visuals::dark());
        // Text clears its surfaces.
        assert!(
            lum(dark.label) > lum(dark.ribbon_content) + 60.0,
            "ribbon labels {:?} must read on {:?}",
            dark.label,
            dark.ribbon_content
        );
        assert!(
            lum(dark.status_text) > lum(dark.status_bg) + 80.0,
            "status text must read on the dark status band"
        );
        assert!(
            lum(dark.ruler_text) > lum(dark.ruler_bg) + 80.0,
            "ruler numbers must read on the dark ruler"
        );
        assert!(
            lum(dark.tab_text_active) > lum(dark.tab_active) + 60.0,
            "active page-tab text must read on the dark tab"
        );
        assert!(
            lum(dark.caption) > lum(dark.stencil_bg) + 60.0,
            "stencil captions must read on the dark pane"
        );
        // Chips stand off their surfaces, and the selected state deepens
        // the interaction beyond hover (light chips darken: 232→220;
        // dark chips lighten — direction flips with the surface).
        assert!(
            lum(dark.ribbon_hover) > lum(dark.ribbon_content) + 8.0,
            "hover chip must stand off the dark ribbon"
        );
        let stand_off = |mode: &str, hover: Color32, selected: Color32, content: Color32| {
            let d = |c: Color32| (lum(c) - lum(content)).abs();
            assert!(
                d(selected) > d(hover),
                "{mode} selected chip ({selected:?}) must stand off the surface \
                 further than hover ({hover:?}) does"
            );
        };
        stand_off(
            "light",
            light.ribbon_hover,
            light.ribbon_selected,
            light.ribbon_content,
        );
        stand_off(
            "dark",
            dark.ribbon_hover,
            dark.ribbon_selected,
            dark.ribbon_content,
        );
        // Accents survive the dark background (the #48 lesson).
        assert!(
            lum(dark.accent) > lum(dark.ribbon_bg) + 40.0,
            "accent lines must read on dark surfaces"
        );
        assert_ne!(dark.accent_band, dark.accent, "band vs line accent split");
        assert_eq!(dark.accent_band, ACCENT, "the chrome band stays grey (#154)");
        assert_eq!(
            light.selection, HIGHLIGHT,
            "the selection frame is the highlight blue (#154)"
        );
        // The desk stays darker than the page edge so the white page
        // floats (page paper is white in both modes).
        assert!(lum(dark.page_edge) > lum(dark.desk));
    }
    /// #165: no egui-default GREEN survives into selection, widget,
    /// or menu states — every state fill is blue-family (the
    /// highlight base). Green is g-dominant; the blues are
    /// b-dominant, so a channel-order check pins the family without
    /// pinning exact shades.
    #[test]
    fn selection_and_state_fills_are_highlight_family_not_green() {
        let ctx = egui::Context::default();
        install_for(&ctx, egui::ThemePreference::Light);
        for theme in [egui::Theme::Light, egui::Theme::Dark] {
            let style = ctx.style_of(theme);
            let v = &style.visuals;
            let fills = [
                v.selection.bg_fill,
                v.widgets.inactive.bg_fill,
                v.widgets.hovered.bg_fill,
                v.widgets.active.bg_fill,
                v.widgets.open.bg_fill,
            ];
            for c in fills {
                assert!(
                    c.b() >= c.g(),
                    "{theme:?} state fill {c:?} leans green — tie it to the highlight blue (#165)"
                );
            }
        }
        // Menu hover fills, both modes.
        for v in [&Visuals::light(), &Visuals::dark()] {
            let t = menu_tokens(v);
            assert!(
                t.hover_fill.b() >= t.hover_fill.g(),
                "menu hover {:?} leans green (#165)",
                t.hover_fill
            );
        }
    }

}
