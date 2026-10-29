//! The ribbon — Visio 2016's command surface, reimplemented as an egui
//! widget: a title band with the Quick Access Toolbar, a tab strip, and
//! a content row of grouped buttons with small vector-drawn icons
//! (cleanroom: no Microsoft assets).

use egui::{Align2, Color32, Pos2, Rect, Sense, Stroke, Ui, Vec2};

use crate::theme;

/// Everything the ribbon can ask the app to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    NewDoc,
    Open,
    Save,
    SaveAs,
    Undo,
    Redo,
    NewPage,
    Delete,
    ZoomIn,
    ZoomOut,
    Zoom100,
    FitPage,
    ToggleGrid,
    ToggleRulers,
    ToggleComments,
    ToggleUnits,
    ShowShapeSheet,
    Macros,
    FillMenu,
    LineMenu,
    FontGrow,
    FontShrink,
    RotateLeft,
    RotateRight,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    File,
    Home,
    Insert,
    Design,
    Data,
    Process,
    Review,
    View,
    Developer,
}

/// Visio 2016 order (Layout is not a Visio tab; Data precedes Process).
pub const TABS: &[Tab] = &[
    Tab::File,
    Tab::Home,
    Tab::Insert,
    Tab::Design,
    Tab::Data,
    Tab::Process,
    Tab::Review,
    Tab::View,
    Tab::Developer,
];

/// Icon glyphs drawn with epaint primitives. One enum per ribbon icon;
/// cheap and colorful like the Office ribbon, but ours.
#[derive(Debug, Clone, Copy)]
pub enum Icon {
    Save,
    Undo,
    Redo,
    Open,
    New,
    Paste,
    Cut,
    Copy,
    Painter,
    FontA,
    Bold,
    Italic,
    Underline,
    Strike,
    Subscript,
    Superscript,
    TextColor,
    Grow,
    Shrink,
    ClearAll,
    Numbering,
    IndentLess,
    IndentMore,
    LineSpacing,
    Effects,
    Bucket,
    Pen,
    BringFront,
    SendBack,
    Align,
    AlignCenter,
    AlignRight,
    Bullets,
    Rotate,
    Group,
    Container,
    Callout,
    Field,
    Comment,
    Spell,
    DataGraphic,
    Link,
    Refresh,
    Swimlane,
    Check,
    Picture,
    OnlinePic,
    Orientation,
    Pointer,
    TextBox,
    NewPage,
    Theme,
    Fit,
    ZoomIn,
    ZoomOut,
    Grid,
    Rulers,
    Shapes,
    Macro,
    ShapeSheet,
    Units,
    Delete,
    TellMe,
}

impl Icon {
    /// The vendored MDI asset stem for this icon (see assets/icons).
    pub fn asset(self) -> Option<&'static str> {
        let a = match self {
            Icon::Save => "save",
            Icon::Undo => "undo",
            Icon::Redo => "redo",
            Icon::Open => "open",
            Icon::New => "new",
            Icon::Paste => "paste",
            Icon::Cut => "cut",
            Icon::Copy => "copy",
            Icon::Painter => "painter",
            Icon::FontA => "fonta",
            Icon::Bold => "bold",
            Icon::Italic => "italic",
            Icon::Underline => "underline",
            Icon::Strike => "strike",
            Icon::Subscript => "subscript",
            Icon::Superscript => "superscript",
            Icon::TextColor => "textcolor",
            Icon::Grow => "grow",
            Icon::Shrink => "shrink",
            Icon::ClearAll => "clear",
            Icon::Numbering => "numbering",
            Icon::IndentLess => "indentless",
            Icon::IndentMore => "indentmore",
            Icon::LineSpacing => "linespacing",
            Icon::Effects => "effects",
            Icon::Bucket => "bucket",
            Icon::Pen => "pen",
            Icon::BringFront => "bringfront",
            Icon::SendBack => "sendback",
            Icon::Align => "align",
            Icon::AlignCenter => "aligncenter",
            Icon::AlignRight => "alignright",
            Icon::Bullets => "bullets",
            Icon::Rotate => "rotate",
            Icon::Group => "group",
            Icon::Container => "container",
            Icon::Callout => "callout",
            Icon::Field => "field",
            Icon::Comment => "comment",
            Icon::Spell => "spell",
            Icon::DataGraphic => "datagraphic",
            Icon::Link => "link",
            Icon::Refresh => "refresh",
            Icon::Swimlane => "swimlane",
            Icon::Check => "check",
            Icon::Picture => "picture",
            Icon::OnlinePic => "onlinepic",
            Icon::Orientation => "orientation",
            Icon::Pointer => "pointer",
            Icon::TextBox => "textbox",
            Icon::NewPage => "newpage",
            Icon::Theme => "theme",
            Icon::Fit => "fit",
            Icon::ZoomIn => "zoomin",
            Icon::ZoomOut => "zoomout",
            Icon::Grid => "grid",
            Icon::Rulers => "rulers",
            Icon::Shapes => "shapes",
            Icon::Macro => "macro",
            Icon::ShapeSheet => "shapesheet",
            Icon::Units => "units",
            Icon::Delete => "delete",
            Icon::TellMe => "lightbulb",
        };
        Some(a)
    }

    /// Paint the icon centered in `r`. Disabled commands paint the same
    /// glyph in muted grays (Office dims; it never hides icons). Uses
    /// the vendored MDI texture when available, falling back to the
    /// built-in epaint glyph.
    /// Office-family accent per icon (wordcraft-style: one accent tone
    /// per command family — clipboard warm, text dark, data blue...).
    /// `pub(crate)`: the context menu re-tints per theme via
    /// `MenuTokens::icon_tint` (GitHub #48).
    pub(crate) fn office_color(self) -> Color32 {
        match self {
            Icon::Paste | Icon::Open => Color32::from_rgb(0xb5, 0x8a, 0x2a),
            Icon::Cut
            | Icon::Copy
            | Icon::Bucket
            | Icon::OnlinePic
            | Icon::Field
            | Icon::Container
            | Icon::Callout
            | Icon::DataGraphic
            | Icon::Save => Color32::from_rgb(0x2e, 0x74, 0xb5),
            Icon::Painter | Icon::Comment | Icon::Effects | Icon::ClearAll => {
                Color32::from_rgb(0xe0, 0x7b, 0x1f)
            }
            Icon::TextColor | Icon::Strike => Color32::from_rgb(0xc0, 0x3a, 0x2b),
            Icon::Grow
            | Icon::Shrink
            | Icon::Numbering
            | Icon::IndentLess
            | Icon::IndentMore
            | Icon::LineSpacing => Color32::from_rgb(0x42, 0x42, 0x42),
            Icon::Delete => Color32::from_rgb(0xc0, 0x3a, 0x2b),
            Icon::Spell | Icon::Refresh | Icon::Check | Icon::New => {
                Color32::from_rgb(0x2e, 0x8b, 0x57)
            }
            Icon::TellMe => Color32::from_rgb(0x82, 0x82, 0x82),
            _ => Color32::from_rgb(0x42, 0x42, 0x42),
        }
    }

    pub fn paint(self, ui: &Ui, r: Rect, enabled: bool) {
        // GitHub #52: on dark surfaces the light Office palette is
        // lifted the same way the context menu tints its icons (#48);
        // light keeps the observed Office colors untouched.
        let office = self.office_color();
        let tint = if enabled {
            if ui.visuals().dark_mode {
                crate::theme::lift_for_dark(office)
            } else {
                office
            }
        } else {
            Color32::from_gray(178)
        };
        if !self.paint_texture(ui, r, tint) {
            // no texture asset/raster: the glyph fallback keeps the
            // caller's enabled state (e.g. grayed disabled buttons).
            self.paint_glyph(ui, r, enabled);
        }
    }

    /// Paint with an explicit tint — for surfaces that follow the theme
    /// (the context menu derives it from `MenuTokens::icon_tint`). The
    /// default [`Self::paint`] keeps the light Office palette, which is
    /// right for the always-light ribbon surface.
    pub fn paint_tinted(self, ui: &Ui, r: Rect, tint: Color32) {
        if !self.paint_texture(ui, r, tint) {
            self.paint_glyph(ui, r, true);
        }
    }

    /// Paint the tinted MDI texture; `false` when the asset is unknown
    /// or rasterization fails, so the caller can use its glyph fallback.
    fn paint_texture(self, ui: &Ui, r: Rect, tint: Color32) -> bool {
        if let Some(name) = self.asset() {
            if let Some(tex) = crate::icontex::texture(ui.ctx(), name, tint) {
                let uv = Rect::from_min_max(egui::Pos2::ZERO, egui::Pos2::new(1.0, 1.0));
                ui.painter().image(tex.id(), r, uv, Color32::WHITE);
                return true;
            }
        }
        false
    }

    /// The built-in epaint glyph (fallback and baseline for compare).
    fn paint_glyph(self, ui: &Ui, r: Rect, enabled: bool) {
        let p = ui.painter();
        let c = r.center();
        let dark = ui.visuals().dark_mode;
        let dim = Color32::from_gray(178);
        // Dark surfaces lift the accent tones like the texture path
        // does (GitHub #52); light keeps the observed Office palette.
        let lift = |col: Color32| {
            if dark {
                crate::theme::lift_for_dark(col)
            } else {
                col
            }
        };
        let lit = |col: Color32| if enabled { lift(col) } else { dim };
        let blue = lit(Color32::from_rgb(0x2a, 0x6f, 0xb5));
        let accent_ink = lit(theme::HIGHLIGHT);
        let orange = lit(Color32::from_rgb(0xd8, 0x7b, 0x0b));
        let gray = if enabled {
            Color32::from_rgb(0x77, 0x77, 0x77)
        } else {
            Color32::from_gray(190)
        };
        let red = lit(Color32::from_rgb(0xc0, 0x3a, 0x2b));
        let stroke = |col| Stroke::new(1.6, col);
        match self {
            Icon::Save => {
                p.rect_filled(r.shrink(2.0), 1.0, blue);
                p.rect_filled(
                    Rect::from_min_size(r.left_top() + Vec2::new(3.0, 3.0), Vec2::new(8.0, 4.0)),
                    0.0,
                    Color32::WHITE,
                );
                p.rect_filled(
                    Rect::from_min_size(r.left_top() + Vec2::new(3.0, 9.0), Vec2::new(8.0, 5.0)),
                    0.0,
                    Color32::WHITE,
                );
            }
            Icon::Undo | Icon::Redo => {
                let dir = if matches!(self, Icon::Redo) {
                    1.0
                } else {
                    -1.0
                };
                let pts: Vec<Pos2> = (0..=10)
                    .map(|i| {
                        let a = 0.6 + (i as f32 / 10.0) * 3.4;
                        c + Vec2::new(dir * 6.0 * a.cos(), 6.0 * a.sin())
                    })
                    .collect();
                p.add(egui::Shape::line(pts, stroke(blue)));
                p.line_segment(
                    [
                        c + Vec2::new(dir * 7.0, -4.0),
                        c + Vec2::new(dir * 3.0, -4.0),
                    ],
                    stroke(blue),
                );
                p.line_segment(
                    [
                        c + Vec2::new(dir * 7.0, -4.0),
                        c + Vec2::new(dir * 5.0, -7.0),
                    ],
                    stroke(blue),
                );
            }
            Icon::New => {
                // blank page with a dog-ear fold (File → New)
                let pg = Rect::from_center_size(c, Vec2::new(10.0, 13.0));
                p.rect_filled(pg, 1.0, Color32::WHITE);
                p.rect_stroke(pg, 1.0, stroke(blue), egui::StrokeKind::Middle);
                let fold = Vec2::new(3.0, 3.0);
                let tr = pg.right_top();
                p.line_segment(
                    [tr - Vec2::new(0.0, fold.y), tr + Vec2::new(fold.x, 0.0)],
                    stroke(blue),
                );
                p.line_segment(
                    [tr - Vec2::new(fold.x, 0.0), tr + Vec2::new(fold.x, 0.0)],
                    stroke(blue),
                );
            }
            Icon::Open => {
                p.rect_stroke(
                    Rect::from_center_size(c, Vec2::splat(11.0)),
                    1.0,
                    stroke(orange),
                    egui::StrokeKind::Middle,
                );
                p.rect_filled(
                    Rect::from_min_size(r.left_top() + Vec2::new(4.0, 6.0), Vec2::new(9.0, 5.0)),
                    0.0,
                    Color32::from_rgb(0xff, 0xe8, 0xb0),
                );
            }
            Icon::Copy => {
                // Office style: two crisp white pages, blue outlines.
                let back =
                    Rect::from_min_size(r.left_top() + Vec2::new(2.0, 2.0), Vec2::splat(11.0));
                let front =
                    Rect::from_min_size(r.left_top() + Vec2::new(7.0, 7.0), Vec2::splat(11.0));
                p.rect_filled(back, 1.5, Color32::from_gray(235));
                p.rect_stroke(
                    back,
                    1.5,
                    Stroke::new(1.4, Color32::from_gray(150)),
                    egui::StrokeKind::Middle,
                );
                p.rect_filled(front, 1.5, Color32::WHITE);
                p.rect_stroke(front, 1.5, Stroke::new(1.6, blue), egui::StrokeKind::Middle);
                for i in 0..3 {
                    let y = front.top() + 3.5 + i as f32 * 2.6;
                    p.line_segment(
                        [
                            Pos2::new(front.left() + 2.5, y),
                            Pos2::new(front.right() - 2.5, y),
                        ],
                        Stroke::new(1.1, Color32::from_gray(185)),
                    );
                }
            }
            Icon::Painter => {
                // Office format painter: angled brush, wooden handle.
                p.line_segment(
                    [c + Vec2::new(-7.0, 7.0), c + Vec2::new(-1.0, 1.0)],
                    Stroke::new(3.4, lit(Color32::from_rgb(0x8a, 0x5a, 0x2b))),
                );
                let tip = Rect::from_center_size(c + Vec2::new(3.0, -3.0), Vec2::new(7.0, 7.0));
                p.rect_filled(tip, 1.5, Color32::from_gray(170));
                p.line_segment(
                    [c + Vec2::new(6.0, -8.0), c + Vec2::new(9.0, -5.0)],
                    Stroke::new(3.2, blue),
                );
                p.line_segment(
                    [c + Vec2::new(-1.0, 1.0), c + Vec2::new(2.0, -2.0)],
                    Stroke::new(2.4, Color32::from_gray(120)),
                );
            }
            Icon::FontA => {
                p.text(
                    c,
                    egui::Align2::CENTER_CENTER,
                    "A",
                    egui::FontId::proportional(17.0),
                    lit(Color32::from_rgb(0x31, 0x31, 0x31)),
                );
                p.line_segment(
                    [
                        Pos2::new(r.left() + 1.0, r.bottom() - 1.0),
                        Pos2::new(r.right() - 1.0, r.bottom() - 1.0),
                    ],
                    Stroke::new(1.6, blue),
                );
            }
            Icon::Bold => {
                p.text(
                    c,
                    egui::Align2::CENTER_CENTER,
                    "B",
                    egui::FontId::proportional(17.0),
                    lit(Color32::from_rgb(0x31, 0x31, 0x31)),
                );
            }
            Icon::Italic => {
                p.text(
                    c,
                    egui::Align2::CENTER_CENTER,
                    "I",
                    egui::FontId::proportional(17.0),
                    lit(Color32::from_rgb(0x31, 0x31, 0x31)),
                );
                p.line_segment(
                    [
                        Pos2::new(r.left() + 2.0, r.bottom() - 1.0),
                        Pos2::new(r.right() - 2.0, r.bottom() - 1.0),
                    ],
                    Stroke::new(1.4, lit(Color32::from_rgb(0x31, 0x31, 0x31))),
                );
            }
            Icon::Underline => {
                p.text(
                    c + Vec2::new(0.0, -1.5),
                    egui::Align2::CENTER_CENTER,
                    "U",
                    egui::FontId::proportional(16.0),
                    lit(Color32::from_rgb(0x31, 0x31, 0x31)),
                );
                p.line_segment(
                    [
                        Pos2::new(r.left() + 2.0, r.bottom() - 2.0),
                        Pos2::new(r.right() - 2.0, r.bottom() - 2.0),
                    ],
                    Stroke::new(1.6, lit(Color32::from_rgb(0x31, 0x31, 0x31))),
                );
            }
            Icon::Bucket => {
                // Office fill: tipped paint bucket with a color drip.
                let body = Rect::from_center_size(c + Vec2::new(1.0, 2.5), Vec2::new(11.0, 9.0));
                p.rect_filled(body, 1.5, lit(Color32::from_rgb(0xd9, 0xea, 0xf6)));
                p.rect_stroke(body, 1.5, Stroke::new(1.5, blue), egui::StrokeKind::Middle);
                let handle = [
                    Pos2::new(body.left() + 1.0, body.top() + 1.0),
                    c + Vec2::new(-4.0, -5.0),
                    c + Vec2::new(4.0, -5.0),
                    Pos2::new(body.right() - 1.0, body.top() + 1.0),
                ];
                p.line(
                    vec![handle[0], handle[1], handle[2], handle[3]],
                    Stroke::new(1.5, Color32::from_gray(130)),
                );
                p.line_segment(
                    [c + Vec2::new(6.0, -6.0), c + Vec2::new(8.0, -3.0)],
                    Stroke::new(2.6, lit(Color32::from_rgb(0xe0, 0x4a, 0x2f))),
                );
                p.circle_filled(
                    c + Vec2::new(8.5, -1.5),
                    1.4,
                    lit(Color32::from_rgb(0xe0, 0x4a, 0x2f)),
                );
            }
            Icon::Pen => {
                // Office line color: felt pen at 45°.
                p.line_segment(
                    [c + Vec2::new(-7.0, 7.0), c + Vec2::new(2.0, -2.0)],
                    Stroke::new(4.0, lit(Color32::from_rgb(0x3a, 0x3a, 0x3a))),
                );
                p.line_segment(
                    [c + Vec2::new(2.0, -2.0), c + Vec2::new(5.0, -5.0)],
                    Stroke::new(4.0, blue),
                );
                p.line_segment(
                    [c + Vec2::new(5.0, -5.0), c + Vec2::new(9.0, -7.0)],
                    Stroke::new(2.8, lit(Color32::from_rgb(0xe0, 0x4a, 0x2f))),
                );
            }
            Icon::BringFront | Icon::SendBack => {
                // Office z-order: layered rounded squares + arrow.
                let front = matches!(self, Icon::BringFront);
                let top = Rect::from_center_size(
                    c + Vec2::new(-1.5, if front { -3.5 } else { 3.5 }),
                    Vec2::splat(8.0),
                );
                let bot = Rect::from_center_size(
                    c + Vec2::new(2.5, if front { 3.0 } else { -3.0 }),
                    Vec2::splat(8.0),
                );
                p.rect_filled(bot, 1.5, Color32::from_gray(180));
                p.rect_stroke(bot, 1.5, Stroke::new(1.2, gray), egui::StrokeKind::Middle);
                p.rect_filled(top, 1.5, lit(Color32::from_rgb(0x8f, 0xcd, 0xb4)));
                p.rect_stroke(
                    top,
                    1.5,
                    Stroke::new(1.4, theme::HIGHLIGHT),
                    egui::StrokeKind::Middle,
                );
                let ay = if front { c.y - 6.5 } else { c.y + 6.5 };
                let dir: f32 = if front { -1.0 } else { 1.0 };
                p.line(
                    vec![
                        Pos2::new(c.x + 8.0, ay + 6.0 * dir),
                        Pos2::new(c.x + 8.0, ay - 2.0 * dir),
                        Pos2::new(c.x + 8.0, ay - 2.0 * dir),
                        Pos2::new(c.x + 5.5, ay + 0.5 * dir),
                        Pos2::new(c.x + 8.0, ay - 2.0 * dir),
                        Pos2::new(c.x + 10.5, ay + 0.5 * dir),
                    ],
                    Stroke::new(1.6, Color32::from_rgb(0x2a, 0x6f, 0xb5)),
                );
            }
            Icon::Strike
            | Icon::Subscript
            | Icon::Superscript
            | Icon::TextColor
            | Icon::Grow
            | Icon::Shrink
            | Icon::ClearAll
            | Icon::Numbering
            | Icon::IndentLess
            | Icon::IndentMore
            | Icon::LineSpacing
            | Icon::Effects => {
                p.text(
                    c,
                    egui::Align2::CENTER_CENTER,
                    "a",
                    egui::FontId::proportional(13.0),
                    blue,
                );
            }
            Icon::Align | Icon::AlignCenter | Icon::AlignRight | Icon::Bullets => {
                // MDI texture covers these; fallback = generic align bars
                for (i, w) in [12.0, 7.0, 12.0, 7.0].iter().enumerate() {
                    let y = c.y - 4.5 + i as f32 * 3.0;
                    p.line_segment(
                        [Pos2::new(c.x - w / 2.0, y), Pos2::new(c.x - w / 2.0 + w, y)],
                        Stroke::new(
                            2.0,
                            if i == 0 {
                                blue
                            } else {
                                lit(Color32::from_rgb(0x59, 0x59, 0x59))
                            },
                        ),
                    );
                }
            }
            Icon::Rotate => {
                p.line(
                    vec![
                        Pos2::new(c.x - 6.0, c.y + 4.0),
                        Pos2::new(c.x - 6.0, c.y - 3.0),
                        Pos2::new(c.x + 4.0, c.y - 3.0),
                    ],
                    Stroke::new(1.8, blue),
                );
                p.line(
                    vec![
                        Pos2::new(c.x + 4.0, c.y - 6.0),
                        Pos2::new(c.x + 8.0, c.y - 3.0),
                        Pos2::new(c.x + 4.0, c.y),
                    ],
                    Stroke::new(1.8, blue),
                );
            }
            Icon::Group => {
                let g = Rect::from_center_size(c, Vec2::splat(13.0));
                let s = Rect::from_center_size(c, Vec2::splat(6.0));
                p.rect_filled(s, 1.0, Color32::from_rgb(0xff, 0xe8, 0xb0));
                p.rect_stroke(s, 1.0, stroke(orange), egui::StrokeKind::Middle);
                let cl = 4.0;
                for (a, b) in [
                    (g.left_top(), Pos2::new(g.left() + cl, g.top())),
                    (g.left_top(), Pos2::new(g.left(), g.top() + cl)),
                    (g.right_top(), Pos2::new(g.right() - cl, g.top())),
                    (g.right_top(), Pos2::new(g.right(), g.top() + cl)),
                    (g.left_bottom(), Pos2::new(g.left() + cl, g.bottom())),
                    (g.left_bottom(), Pos2::new(g.left(), g.bottom() - cl)),
                    (g.right_bottom(), Pos2::new(g.right() - cl, g.bottom())),
                    (g.right_bottom(), Pos2::new(g.right(), g.bottom() - cl)),
                ] {
                    p.line_segment([a, b], stroke(gray));
                }
            }
            Icon::Container => {
                p.rect_stroke(
                    Rect::from_center_size(c, Vec2::new(14.0, 10.0)),
                    2.0,
                    stroke(blue),
                    egui::StrokeKind::Middle,
                );
                p.rect_filled(
                    Rect::from_center_size(c + Vec2::new(0.0, -2.0), Vec2::new(8.0, 4.0)),
                    1.0,
                    Color32::from_rgb(0xff, 0xe8, 0xb0),
                );
            }
            Icon::Callout => {
                p.rect_stroke(
                    Rect::from_center_size(c + Vec2::new(0.0, -1.5), Vec2::new(13.0, 8.0)),
                    1.0,
                    stroke(blue),
                    egui::StrokeKind::Middle,
                );
                p.line(
                    vec![
                        Pos2::new(c.x - 3.0, c.y + 2.5),
                        Pos2::new(c.x - 5.0, c.y + 7.0),
                        Pos2::new(c.x, c.y + 2.5),
                    ],
                    Stroke::new(1.4, blue),
                );
            }
            Icon::Field => {
                p.text(
                    c,
                    egui::Align2::CENTER_CENTER,
                    "{ }",
                    egui::FontId::monospace(12.0),
                    blue,
                );
            }
            Icon::Comment => {
                p.rect_stroke(
                    Rect::from_center_size(c + Vec2::new(0.0, -1.0), Vec2::new(13.0, 8.0)),
                    3.0,
                    stroke(orange),
                    egui::StrokeKind::Middle,
                );
                p.line_segment(
                    [
                        Pos2::new(c.x - 2.0, c.y + 3.0),
                        Pos2::new(c.x - 4.0, c.y + 7.0),
                    ],
                    stroke(orange),
                );
            }
            Icon::Spell => {
                p.text(
                    c + Vec2::new(-2.0, 0.0),
                    egui::Align2::CENTER_CENTER,
                    "abc",
                    egui::FontId::proportional(9.0),
                    Color32::BLACK,
                );
                p.line(
                    vec![
                        Pos2::new(c.x + 1.0, c.y + 3.0),
                        Pos2::new(c.x + 3.5, c.y + 6.0),
                        Pos2::new(c.x + 8.0, c.y - 4.0),
                    ],
                    Stroke::new(1.8, accent_ink),
                );
            }
            Icon::DataGraphic => {
                for (i, h) in [5.0, 9.0, 12.0].iter().enumerate() {
                    let x = c.x - 6.0 + i as f32 * 5.0;
                    p.rect_filled(
                        Rect::from_min_size(Pos2::new(x, c.y + 6.0 - h), Vec2::new(3.5, *h)),
                        0.5,
                        if i == 2 { orange } else { blue },
                    );
                }
            }
            Icon::Link => {
                p.line_segment(
                    [c + Vec2::new(-6.0, -3.0), c + Vec2::new(6.0, 3.0)],
                    Stroke::new(2.2, gray),
                );
                p.circle_stroke(c + Vec2::new(-5.0, -3.0), 3.5, stroke(blue));
                p.circle_stroke(c + Vec2::new(5.0, 3.0), 3.5, stroke(blue));
            }
            Icon::Refresh => {
                p.line(
                    vec![
                        Pos2::new(c.x - 5.0, c.y - 4.0),
                        Pos2::new(c.x + 3.0, c.y - 4.0),
                        Pos2::new(c.x + 3.0, c.y + 4.0),
                        Pos2::new(c.x - 5.0, c.y + 4.0),
                    ],
                    Stroke::new(1.7, accent_ink),
                );
                p.line(
                    vec![
                        Pos2::new(c.x - 5.0, c.y - 7.0),
                        Pos2::new(c.x - 8.0, c.y - 4.0),
                        Pos2::new(c.x - 5.0, c.y - 1.0),
                    ],
                    stroke(accent_ink),
                );
            }
            Icon::Swimlane => {
                p.rect_stroke(
                    Rect::from_center_size(c, Vec2::new(14.0, 11.0)),
                    1.0,
                    stroke(blue),
                    egui::StrokeKind::Middle,
                );
                p.line_segment(
                    [
                        Pos2::new(c.x - 4.0, c.y - 5.5),
                        Pos2::new(c.x - 4.0, c.y + 5.5),
                    ],
                    stroke(blue),
                );
                p.line_segment(
                    [
                        Pos2::new(c.x - 7.0, c.y - 1.0),
                        Pos2::new(c.x + 7.0, c.y - 1.0),
                    ],
                    stroke(blue),
                );
            }
            Icon::Check => {
                p.line(
                    vec![
                        Pos2::new(c.x - 6.0, c.y),
                        Pos2::new(c.x - 2.0, c.y + 4.0),
                        Pos2::new(c.x + 7.0, c.y - 5.0),
                    ],
                    Stroke::new(2.0, accent_ink),
                );
            }
            Icon::Picture => {
                p.rect_stroke(
                    Rect::from_center_size(c, Vec2::splat(12.0)),
                    1.0,
                    stroke(gray),
                    egui::StrokeKind::Middle,
                );
                p.circle_filled(c + Vec2::new(-3.0, -3.0), 1.8, orange);
                p.line(
                    vec![
                        Pos2::new(c.x - 5.0, c.y + 4.0),
                        Pos2::new(c.x - 1.0, c.y - 1.0),
                        Pos2::new(c.x + 2.0, c.y + 2.0),
                        Pos2::new(c.x + 5.0, c.y - 1.0),
                    ],
                    stroke(accent_ink),
                );
            }
            Icon::OnlinePic => {
                p.circle_stroke(c, 6.0, stroke(blue));
                p.line_segment(
                    [Pos2::new(c.x - 6.0, c.y), Pos2::new(c.x + 6.0, c.y)],
                    stroke(blue),
                );
                p.line_segment(
                    [Pos2::new(c.x, c.y - 6.0), Pos2::new(c.x, c.y + 6.0)],
                    stroke(blue),
                );
            }
            Icon::Orientation => {
                p.rect_stroke(
                    Rect::from_center_size(c + Vec2::new(-3.5, -1.0), Vec2::new(6.0, 9.0)),
                    1.0,
                    stroke(blue),
                    egui::StrokeKind::Middle,
                );
                p.rect_stroke(
                    Rect::from_center_size(c + Vec2::new(4.5, 2.0), Vec2::new(9.0, 6.0)),
                    1.0,
                    stroke(gray),
                    egui::StrokeKind::Middle,
                );
            }
            Icon::Paste => {
                // Office paste: warm clipboard, clip at top, paper lines.
                let board = r.shrink(3.0);
                p.rect_filled(board, 2.0, lit(Color32::from_rgb(0xf3, 0xe2, 0xb8)));
                p.rect_stroke(
                    board,
                    2.0,
                    Stroke::new(1.5, lit(Color32::from_rgb(0xb5, 0x8a, 0x2a))),
                    egui::StrokeKind::Middle,
                );
                let clip =
                    Rect::from_center_size(Pos2::new(c.x, board.top() + 1.0), Vec2::new(7.0, 4.0));
                p.rect_filled(clip, 1.0, lit(Color32::from_rgb(0xb5, 0x8a, 0x2a)));
                p.rect_filled(
                    Rect::from_center_size(c + Vec2::new(0.0, -5.5), Vec2::new(6.0, 3.0)),
                    0.0,
                    gray,
                );
                p.line(
                    vec![c + Vec2::new(-3.0, 1.0), c + Vec2::new(3.0, 1.0)],
                    stroke(blue),
                );
                p.line(
                    vec![c + Vec2::new(-3.0, 4.0), c + Vec2::new(3.0, 4.0)],
                    stroke(blue),
                );
            }
            Icon::Cut => {
                // Office scissors: silver filled blades, blue ring handles.
                for dx in [-4.0, 4.0] {
                    p.circle_stroke(c + Vec2::new(dx, 5.0), 3.2, Stroke::new(2.0, blue));
                }
                p.line_segment(
                    [c + Vec2::new(-4.0, 4.0), c + Vec2::new(5.0, -6.5)],
                    Stroke::new(2.8, Color32::from_gray(120)),
                );
                p.line_segment(
                    [c + Vec2::new(4.0, 4.0), c + Vec2::new(-5.0, -6.5)],
                    Stroke::new(2.8, Color32::from_gray(150)),
                );
            }
            Icon::Pointer => {
                p.add(egui::Shape::convex_polygon(
                    vec![
                        c + Vec2::new(-5.0, -6.0),
                        c + Vec2::new(5.0, 1.0),
                        c + Vec2::new(0.0, 1.5),
                        c + Vec2::new(3.0, 7.0),
                        c + Vec2::new(0.5, 7.5),
                        c + Vec2::new(-2.0, 1.5),
                        c + Vec2::new(-5.0, 2.0),
                    ],
                    gray,
                    Stroke::new(0.8, Color32::BLACK),
                ));
            }
            Icon::TextBox => {
                p.rect_stroke(
                    Rect::from_center_size(c, Vec2::new(13.0, 9.0)),
                    1.0,
                    stroke(blue),
                    egui::StrokeKind::Middle,
                );
                p.line_segment(
                    [c + Vec2::new(-4.0, 0.0), c + Vec2::new(4.0, 0.0)],
                    Stroke::new(1.2, blue),
                );
            }
            Icon::NewPage => {
                p.rect_stroke(
                    Rect::from_center_size(c + Vec2::new(-1.0, 0.0), Vec2::new(10.0, 13.0)),
                    1.0,
                    stroke(blue),
                    egui::StrokeKind::Middle,
                );
                p.line_segment(
                    [c + Vec2::new(-4.0, 0.0), c + Vec2::new(5.0, 0.0)],
                    Stroke::new(1.6, accent_ink),
                );
                p.line_segment(
                    [c + Vec2::new(0.5, -5.0), c + Vec2::new(0.5, 5.0)],
                    Stroke::new(1.6, accent_ink),
                );
            }
            Icon::Theme => {
                p.rect_filled(Rect::from_center_size(c, Vec2::new(6.0, 6.0)), 1.0, blue);
                p.rect_filled(
                    Rect::from_center_size(c + Vec2::new(-5.0, 4.0), Vec2::new(5.0, 5.0)),
                    1.0,
                    orange,
                );
                p.rect_filled(
                    Rect::from_center_size(c + Vec2::new(5.0, 4.0), Vec2::new(5.0, 5.0)),
                    1.0,
                    accent_ink,
                );
            }
            Icon::Fit => {
                p.line_segment(
                    [c + Vec2::new(-6.0, -4.0), c + Vec2::new(-6.0, 4.0)],
                    stroke(gray),
                );
                p.line_segment(
                    [c + Vec2::new(-6.0, -4.0), c + Vec2::new(-3.0, -4.0)],
                    stroke(gray),
                );
                p.line_segment(
                    [c + Vec2::new(6.0, 4.0), c + Vec2::new(6.0, -4.0)],
                    stroke(gray),
                );
                p.line_segment(
                    [c + Vec2::new(6.0, 4.0), c + Vec2::new(3.0, 4.0)],
                    stroke(gray),
                );
                p.rect_stroke(
                    Rect::from_center_size(c, Vec2::new(7.0, 5.0)),
                    0.5,
                    stroke(blue),
                    egui::StrokeKind::Middle,
                );
            }
            Icon::ZoomIn => {
                p.circle_stroke(
                    c + Vec2::new(-2.0, -2.0),
                    5.0,
                    Stroke::new(2.0, lit(Color32::from_rgb(0x31, 0x31, 0x31))),
                );
                p.line_segment(
                    [c + Vec2::new(1.5, 1.5), c + Vec2::new(6.5, 6.5)],
                    Stroke::new(2.6, lit(Color32::from_rgb(0x31, 0x31, 0x31))),
                );
                p.line_segment(
                    [c + Vec2::new(-4.5, -2.0), c + Vec2::new(0.5, -2.0)],
                    Stroke::new(1.8, blue),
                );
                p.line_segment(
                    [c + Vec2::new(-2.0, -4.0), c + Vec2::new(-2.0, 0.0)],
                    Stroke::new(1.2, blue),
                );
            }
            Icon::ZoomOut => {
                p.circle_stroke(
                    c + Vec2::new(-2.0, -2.0),
                    5.0,
                    Stroke::new(2.0, lit(Color32::from_rgb(0x31, 0x31, 0x31))),
                );
                p.line_segment(
                    [c + Vec2::new(1.5, 1.5), c + Vec2::new(6.5, 6.5)],
                    Stroke::new(2.6, lit(Color32::from_rgb(0x31, 0x31, 0x31))),
                );
                p.line_segment(
                    [c + Vec2::new(-4.5, -2.0), c + Vec2::new(0.5, -2.0)],
                    Stroke::new(1.8, blue),
                );
            }
            Icon::Grid => {
                for i in -1..=1 {
                    let t = i as f32;
                    p.line_segment(
                        [c + Vec2::new(t * 5.0, -6.0), c + Vec2::new(t * 5.0, 6.0)],
                        Stroke::new(0.8, gray),
                    );
                    p.line_segment(
                        [c + Vec2::new(-6.0, t * 5.0), c + Vec2::new(6.0, t * 5.0)],
                        Stroke::new(0.8, gray),
                    );
                }
            }
            Icon::Rulers => {
                p.rect_filled(
                    Rect::from_center_size(c, Vec2::new(14.0, 6.0)),
                    0.0,
                    Color32::from_rgb(0xf5, 0xef, 0xd8),
                );
                for i in -2..=2 {
                    let t = i as f32;
                    p.line_segment(
                        [c + Vec2::new(t * 3.0, -3.0), c + Vec2::new(t * 3.0, -1.0)],
                        Stroke::new(0.8, gray),
                    );
                }
            }
            Icon::Shapes => {
                p.rect_filled(
                    Rect::from_center_size(c + Vec2::new(-4.0, -3.0), Vec2::splat(6.0)),
                    0.5,
                    orange,
                );
                p.circle_filled(c + Vec2::new(4.0, -3.0), 3.5, accent_ink);
                p.add(egui::Shape::convex_polygon(
                    vec![
                        c + Vec2::new(0.0, 3.0),
                        c + Vec2::new(4.0, 9.0),
                        c + Vec2::new(-4.0, 9.0),
                    ],
                    blue,
                    Stroke::NONE,
                ));
            }
            Icon::Macro => {
                p.rect_stroke(
                    Rect::from_center_size(c, Vec2::new(12.0, 9.0)),
                    1.0,
                    stroke(accent_ink),
                    egui::StrokeKind::Middle,
                );
                p.line_segment(
                    [c + Vec2::new(-4.0, -1.0), c + Vec2::new(4.0, -1.0)],
                    Stroke::new(1.4, accent_ink),
                );
                p.line_segment(
                    [c + Vec2::new(-4.0, 2.0), c + Vec2::new(2.0, 2.0)],
                    Stroke::new(1.4, accent_ink),
                );
            }
            Icon::ShapeSheet => {
                p.rect_stroke(
                    Rect::from_center_size(c, Vec2::new(13.0, 10.0)),
                    1.0,
                    stroke(blue),
                    egui::StrokeKind::Middle,
                );
                for y in [-1.0, 1.5] {
                    p.line_segment(
                        [c + Vec2::new(-6.0, y * 2.0), c + Vec2::new(6.0, y * 2.0)],
                        Stroke::new(0.7, gray),
                    );
                }
                p.line_segment(
                    [c + Vec2::new(-2.0, -5.0), c + Vec2::new(-2.0, 5.0)],
                    Stroke::new(0.7, gray),
                );
            }
            Icon::Units => {
                p.text(
                    c,
                    Align2::CENTER_CENTER,
                    "mm",
                    egui::FontId::monospace(12.0),
                    accent_ink,
                );
            }
            Icon::Delete => {
                p.line_segment(
                    [c + Vec2::new(-5.5, -5.5), c + Vec2::new(5.5, 5.5)],
                    Stroke::new(2.6, red),
                );
                p.line_segment(
                    [c + Vec2::new(5.5, -5.5), c + Vec2::new(-5.5, 5.5)],
                    Stroke::new(2.6, red),
                );
            }
            Icon::TellMe => {
                // bulb outline (fallback only; the MDI glyph is shipped)
                p.circle_stroke(c + Vec2::new(0.0, -1.5), 5.5, Stroke::new(1.4, gray));
                p.line_segment(
                    [c + Vec2::new(-2.0, 5.0), c + Vec2::new(2.0, 5.0)],
                    Stroke::new(1.4, gray),
                );
            }
        }
    }
}

/// A ribbon button: icon + caption, hero- or compact-style.
#[derive(Clone, Copy)]
pub struct RibbonButton {
    pub icon: Icon,
    pub label: &'static str,
    pub tip: &'static str,
    pub enabled: bool,
    pub selected: bool,
    pub action: Action,
    /// Render with a combo-box caret (font name / size controls).
    pub combo: bool,
    /// Render with a split-button dropdown arrow (Paste).
    pub split: bool,
    /// Fixed control width (comboboxes: font 140 / size 52, Office-style).
    pub width: Option<f32>,
}

impl RibbonButton {
    pub fn new(icon: Icon, label: &'static str, action: Action) -> Self {
        RibbonButton {
            icon,
            label,
            tip: label,
            enabled: true,
            selected: false,
            action,
            combo: false,
            split: false,
            width: None,
        }
    }
    pub fn tip(mut self, tip: &'static str) -> Self {
        self.tip = tip;
        self
    }
    pub fn enabled(mut self, on: bool) -> Self {
        self.enabled = on;
        self
    }
    pub fn selected(mut self, on: bool) -> Self {
        self.selected = on;
        self
    }
    pub fn combo(mut self) -> Self {
        self.combo = true;
        self
    }
    pub fn split(mut self) -> Self {
        self.split = true;
        self
    }
    pub fn width(mut self, w: f32) -> Self {
        self.width = Some(w);
        self
    }

    fn paint_frame(self, ui: &Ui, r: Rect, resp: &egui::Response) {
        // Office 2016 states are flat neutral chips with a hairline
        // border (observed in Visio 2016 shots — no colored fills).
        if !self.enabled {
            return;
        }
        let t = theme::shell_tokens(ui.visuals());
        let (fill, edge) = if self.selected {
            (t.ribbon_selected, t.ribbon_selected_edge)
        } else if resp.hovered() {
            (t.ribbon_hover, t.ribbon_hover_edge)
        } else {
            return;
        };
        ui.painter().rect_filled(r, 2, fill);
        ui.painter()
            .rect_stroke(r, 2, Stroke::new(1.0, edge), egui::StrokeKind::Middle);
    }

    /// Button label color for the active visual mode (GitHub #52).
    fn label_color(self, ui: &Ui) -> Color32 {
        let t = theme::shell_tokens(ui.visuals());
        if self.enabled {
            t.label
        } else {
            t.label_disabled
        }
    }

    /// Visio "hero" button: large icon above the label.
    pub fn show_big(self, ui: &mut Ui) -> bool {
        let size = Vec2::new(58.0, 72.0);
        let (r, resp) = ui.allocate_exact_size(size, Sense::click());
        // The caption is a painted galley — attach its text for
        // screen readers and keyboard focus (GitHub #56).
        resp.widget_info(|| {
            egui::WidgetInfo::labeled(egui::Role::Button, self.enabled, self.label)
        });
        let resp = resp.on_hover_text(self.tip);
        self.paint_frame(ui, r, &resp);
        let icon_r =
            Rect::from_center_size(Pos2::new(r.center().x, r.top() + 21.0), Vec2::splat(32.0));
        if std::env::var_os("DC_UI_DEBUG_ICON").is_some() {
            eprintln!(
                "DEBUG show_big {} r={:?} icon_r={:?}",
                self.label, r, icon_r
            );
        }
        self.icon.paint(ui, icon_r, self.enabled);
        let gal = ui.painter().layout_no_wrap(
            self.label.to_string(),
            egui::FontId::proportional(11.5),
            self.label_color(ui),
        );
        ui.painter().galley(
            Pos2::new(r.center().x - gal.size().x / 2.0, r.bottom() - 15.0),
            gal,
            egui::Color32::WHITE,
        );
        self.enabled && resp.clicked()
    }

    /// Compact button: small icon beside the label (stacked in a
    /// narrow column, Visio small-button style).
    pub fn show_small(self, ui: &mut Ui) -> bool {
        let h = 22.0;
        let gal = ui.painter().layout_no_wrap(
            self.label.to_string(),
            egui::FontId::proportional(11.5),
            self.label_color(ui),
        );
        let w = self
            .width
            .unwrap_or_else(|| 16.0 + 4.0 + gal.size().x + 8.0);
        let (r, resp) = ui.allocate_exact_size(Vec2::new(w, h), Sense::click());
        // Painted galley caption — name the control (GitHub #56).
        resp.widget_info(|| {
            egui::WidgetInfo::labeled(egui::Role::Button, self.enabled, self.label)
        });
        let resp = resp.on_hover_text(self.tip);
        self.paint_frame(ui, r, &resp);
        let icon_r = Rect::from_min_size(r.left_top() + Vec2::new(1.0, 2.0), Vec2::splat(18.0));
        self.icon.paint(ui, icon_r, self.enabled);
        ui.painter().galley(
            Pos2::new(icon_r.right() + 4.0, r.center().y - gal.size().y / 2.0),
            gal,
            egui::Color32::WHITE,
        );
        if self.combo {
            let t = theme::shell_tokens(ui.visuals());
            // Office combobox: visible input border around the control
            ui.painter().rect_stroke(
                r,
                1.0,
                Stroke::new(1.0, t.ribbon_group),
                egui::StrokeKind::Middle,
            );
            let cx = r.right() - 6.0;
            ui.painter().add(egui::Shape::convex_polygon(
                vec![
                    Pos2::new(cx - 2.5, r.center().y - 1.0),
                    Pos2::new(cx + 2.5, r.center().y - 1.0),
                    Pos2::new(cx, r.center().y + 2.0),
                ],
                if self.enabled {
                    t.tab_text_inactive
                } else {
                    Color32::from_gray(185)
                },
                Stroke::NONE,
            ));
            egui::Popup::menu(&resp).show(|ui| {
                ui.set_min_width(160.0);
                for entry in ["Calibri", "Carlito", "Liberation Sans", "Noto Sans"] {
                    ui.weak(entry);
                }
                ui.separator();
                ui.weak("Text formatting arrives with a later milestone");
            });
        }
        self.enabled && resp.clicked()
    }
}

/// One column of a ribbon group: a hero button, a vertical stack of
/// compact ones, or a small mini-grid (Office's 2-high clusters).
pub enum Col {
    Big(RibbonButton),
    Stack(Vec<RibbonButton>),
    Grid(Vec<Vec<RibbonButton>>),
}

pub fn big(b: RibbonButton) -> Col {
    Col::Big(b)
}

pub fn stack(v: Vec<RibbonButton>) -> Col {
    Col::Stack(v)
}

pub fn grid(rows: Vec<Vec<RibbonButton>>) -> Col {
    Col::Grid(rows)
}

/// One ribbon tab's content row: groups separated by thin vertical
/// rules that stop above the label strip, each group's label centered
/// beneath it (Visio 2016 layout). Only some groups carry the dialog
/// launcher (Visio: Font, Paragraph — never Clipboard/Arrange).
pub struct Group {
    pub label: &'static str,
    pub cols: Vec<Col>,
    /// Whether to draw the group dialog launcher (pop-out arrow).
    pub launcher: bool,
}

impl Group {
    pub fn new(label: &'static str, cols: Vec<Col>) -> Self {
        Group {
            label,
            cols,
            launcher: false,
        }
    }
    /// Mark this group as having a dialog launcher (Visio's Font and
    /// Paragraph groups open Format dialogs from theirs).
    pub fn with_launcher(mut self) -> Self {
        self.launcher = true;
        self
    }
}

const CONTENT_H: f32 = 74.0;
const LABEL_H: f32 = 18.0;

fn show_col(ui: &mut Ui, col: &Col, on_action: &mut dyn FnMut(Action)) {
    match col {
        Col::Big(b) => {
            if (*b).show_big(ui) {
                on_action(b.action);
            }
        }
        Col::Stack(v) => {
            ui.scope(|ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                ui.vertical(|ui| {
                    for b in v {
                        if (*b).show_small(ui) {
                            on_action(b.action);
                        }
                    }
                });
            });
        }
        Col::Grid(rows) => {
            ui.scope(|ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                ui.vertical(|ui| {
                    for row in rows {
                        ui.horizontal(|ui| {
                            for b in row {
                                if (*b).show_small(ui) {
                                    on_action(b.action);
                                }
                            }
                        });
                    }
                });
            });
        }
    }
}

pub fn content_row(
    ui: &mut Ui,
    groups: &[Group],
    on_action: &mut dyn FnMut(Action),
    on_launcher: &mut dyn FnMut(&'static str),
) {
    let t = theme::shell_tokens(ui.visuals());
    let mut spans: Vec<(&'static str, f32, f32, bool)> = Vec::new();
    ui.vertical(|ui| {
        ui.add_space(2.0);
        ui.horizontal(|ui| {
            ui.set_min_height(CONTENT_H);
            ui.add_space(6.0);
            for (i, g) in groups.iter().enumerate() {
                let x0 = ui.next_widget_position().x;
                // top-aligned group content (small stacks align with the
                // top of the tall hero buttons, like Office)
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
                    for col in &g.cols {
                        show_col(ui, col, on_action);
                    }
                });
                let x1 = ui.next_widget_position().x - ui.spacing().item_spacing.x;
                spans.push((g.label, x0, x1, g.launcher));
                if i + 1 < groups.len() {
                    // Visio: the separator spans the content row and
                    // stops above the group-label strip.
                    let (sr, _) = ui.allocate_exact_size(Vec2::new(1.0, CONTENT_H), Sense::hover());
                    ui.painter().vline(
                        sr.center().x,
                        sr.top()..=sr.bottom(),
                        Stroke::new(1.0, t.ribbon_group),
                    );
                    ui.add_space(6.0);
                }
            }
        });
        let (strip, _) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), LABEL_H), Sense::hover());
        for (label, x0, x1, launcher) in &spans {
            let gal = ui.painter().layout_no_wrap(
                label.to_string(),
                egui::FontId::proportional(9.5),
                t.ribbon_label,
            );
            // the label centers on the group; leave room for the
            // launcher only where the group has one
            let centre = if *launcher {
                (x0 + x1 - 13.0) / 2.0
            } else {
                (x0 + x1) / 2.0
            };
            ui.painter().galley(
                Pos2::new(centre - gal.size().x / 2.0, strip.top() + 3.0),
                gal,
                egui::Color32::WHITE,
            );
            if !launcher {
                continue;
            }
            // the group dialog launcher (pop-out): tiny bordered square
            let lr =
                Rect::from_min_size(Pos2::new(x1 - 11.0, strip.top() + 2.0), Vec2::splat(11.0));
            let lresp = ui.interact(lr, ui.scope_id().with(("launcher", label)), Sense::click());
            let hot = lresp.hovered();
            ui.painter().rect_stroke(
                lr,
                1.0,
                Stroke::new(0.8, t.ruler_tick),
                egui::StrokeKind::Middle,
            );
            let c = lr.center();
            ui.painter().line(
                vec![
                    Pos2::new(c.x - 3.0, c.y + 3.0),
                    Pos2::new(c.x + 3.0, c.y - 3.0),
                    Pos2::new(c.x + 3.0, c.y + 0.5),
                    Pos2::new(c.x + 3.0, c.y - 3.0),
                    Pos2::new(c.x - 0.5, c.y - 3.0),
                ],
                Stroke::new(
                    1.2,
                    if hot {
                        t.accent
                    } else {
                        Color32::from_gray(120)
                    },
                ),
            );
            if lresp.clicked() {
                on_launcher(label);
            }
        }
    });
}
