//! The shape context menu's mini-toolbar (#77 landed a flat one-row
//! icon bar inside the menu body; #149 restyles it to the real Visio
//! 2010/2016 menu): a two-row ribbon-chrome panel riding ABOVE the
//! classic text rows — near-white rounded fill, hairline border, a
//! drop shadow between the panel and the rows, thin vertical
//! separators between the control groups, and a captioned gallery
//! tile ("Styles") spanning both rows at the end. Clean-room from the
//! owner's screenshot of the shipped menu (#149's 2026-10-28
//! comment): structure only, no Microsoft artwork — glyphs stay the
//! vendored MDI set (the Flip flyout's is MDI `flip-vertical`).
//!
//! The command set stays #77's seven working-set commands plus the
//! Flip flyout (#149 item 1 — one glyph cannot pick Vertical vs
//! Horizontal, so the button is a dropdown); #149 item 3 confirms the
//! real slot map against the shipped app (blocked on the Visio
//! oracle) and the tile opens the Format Shape pane as its stand-in
//! for Visio's Styles gallery until a styles feature exists.

use crate::ribbon::Icon;
use crate::theme::{self, MenuTokens};
use egui::{Color32, Pos2, Rect, Sense, Stroke, Ui, Vec2};

/// What the toolbar clicked (app.rs maps these onto its menu-local
/// Act enum).
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Act {
    EditText,
    /// #149 item 2: the three Format icons target their Format-pane
    /// section instead of opening it generically.
    FormatFill,
    FormatLine,
    FormatText,
    Front,
    RotateRight,
    FlipVertical,
    FlipHorizontal,
    Group,
    /// The Styles tile's stand-in action: open the Format Shape pane.
    Styles,
}

/// Button cell (the screenshot's slots): 22 px square, 16 px glyph.
const BTN: f32 = 22.0;
/// A dropdown cell is wider — room for the glyph and its chevron.
const DROPDOWN_W: f32 = 29.0;
const ROW_H: f32 = 24.0;

/// The panel's resolved chrome colors (the menu tokens drive the
/// controls; the panel itself follows light/dark like the menu does,
/// GitHub #48).
struct Chrome {
    tokens: MenuTokens,
    /// Group separator stroke.
    sep: Color32,
}

/// Show the panel inside the context-menu popup; returns the clicked
/// command, if any. `can_group` mirrors the text rows' Group
/// enablement (#77).
pub(crate) fn show(ui: &mut Ui, can_group: bool) -> Option<Act> {
    let mut act = None;
    let tokens = theme::menu_tokens(ui.visuals());
    let chrome = if tokens.dark {
        Chrome {
            tokens,
            sep: Color32::from_rgb(0x3c, 0x3c, 0x3c),
        }
    } else {
        // The screenshot's panel: near-white fill, light-gray border,
        // faint shadow, #d5d5d5 group separators.
        Chrome {
            tokens,
            sep: Color32::from_rgb(0xd5, 0xd5, 0xd5),
        }
    };
    let (fill, border, shadow) = if chrome.tokens.dark {
        (
            Color32::from_rgb(0x2b, 0x2b, 0x2b),
            Color32::from_rgb(0x55, 0x55, 0x55),
            Color32::from_black_alpha(140),
        )
    } else {
        (
            Color32::from_rgb(0xf6, 0xf6, 0xf6),
            Color32::from_rgb(0x9a, 0x9a, 0x9a),
            Color32::from_gray(120),
        )
    };
    ui.add_space(2.0);
    // The panel floats above the text rows — its border and shadow
    // replace #77's hairline separator.
    egui::Frame::new()
        .fill(fill)
        .stroke(Stroke::new(1.0, border))
        .corner_radius(4.0)
        .shadow(egui::Shadow {
            offset: [0, 2],
            blur: 3,
            spread: 0,
            color: shadow,
        })
        .inner_margin(egui::Margin::symmetric(6, 4))
        .show(ui, |ui| {
            // Span the menu width, like the shipped panel.
            let w = ui.available_width();
            if w.is_finite() {
                ui.set_min_width(w);
            }
            ui.horizontal(|ui| {
                let rows = |ui: &mut Ui, act: &mut Option<Act>| {
                    ui.vertical(|ui| {
                        row(
                            ui,
                            &chrome,
                            vec![
                                Item::plain(Icon::TextBox, strs().msg_edit_text(), Act::EditText),
                                Item::plain(Icon::Bucket, strs().msg_format_shape_fill(), Act::FormatFill),
                                Item::plain(Icon::Pen, strs().msg_format_shape_line(), Act::FormatLine),
                                Item::plain(Icon::FontA, strs().msg_format_shape_text(), Act::FormatText),
                            ],
                            Some(1),
                            act,
                        );
                        ui.add_space(1.0);
                        row(
                            ui,
                            &chrome,
                            vec![
                                Item::plain(Icon::BringFront, strs().msg_bring_to_front(), Act::Front),
                                Item::plain(Icon::Rotate, strs().msg_rotate_right_90(), Act::RotateRight),
                                Item::flip(),
                                Item::plain(Icon::Group, strs().msg_group(), Act::Group)
                                    .enabled(can_group),
                            ],
                            None,
                            act,
                        );
                    });
                };
                let tail = |ui: &mut Ui, act: &mut Option<Act>| {
                    // The tall separator before the tile runs the full
                    // two-row height.
                    let (sr, _) =
                        ui.allocate_exact_size(Vec2::new(9.0, 2.0 * ROW_H), Sense::hover());
                    ui.painter()
                        .vline(sr.center().x, sr.y_range(), Stroke::new(1.0, chrome.sep));
                    styles_tile(ui, &chrome, act);
                };
                // #257 item 3: the panel mirrors with the chrome — the
                // tile side and each row's entries emit reversed.
                if crate::i18n::rtl() {
                    tail(ui, &mut act);
                    rows(ui, &mut act);
                } else {
                    rows(ui, &mut act);
                    tail(ui, &mut act);
                }
            });
        });
    act
}

/// One toolbar slot. Plain buttons dispatch directly; the Flip slot
/// is a dropdown whose flyout carries the two orientations.
struct Item {
    icon: Icon,
    label: String,
    enabled: bool,
    kind: Kind,
}

enum Kind {
    Plain(Act),
    /// The Flip dropdown (#149 item 1): click opens the
    /// Vertical/Horizontal flyout.
    FlipFlyout,
}

impl Item {
    fn plain(icon: Icon, label: String, act: Act) -> Item {
        Item {
            icon,
            label,
            enabled: true,
            kind: Kind::Plain(act),
        }
    }
    fn flip() -> Item {
        Item {
            icon: Icon::FlipVertical,
            label: strs().msg_flip(),
            enabled: true,
            kind: Kind::FlipFlyout,
        }
    }
    fn enabled(mut self, enabled: bool) -> Item {
        self.enabled = enabled;
        self
    }
}

/// One row of slots, with an optional group separator drawn after the
/// given index (the screenshot splits each row into groups).
fn row(ui: &mut Ui, chrome: &Chrome, mut items: Vec<Item>, sep_after: Option<usize>, act: &mut Option<Act>) {
    if crate::i18n::rtl() {
        items.reverse();
    }
    ui.horizontal(|ui| {
        for (i, it) in items.iter().enumerate() {
            bar_button(ui, chrome, it, act);
            if sep_after == Some(i) && i + 1 < items.len() {
                let (sr, _) = ui.allocate_exact_size(Vec2::new(9.0, BTN), Sense::hover());
                ui.painter().vline(
                    sr.center().x,
                    sr.y_range(),
                    Stroke::new(1.0, chrome.sep),
                );
            }
        }
    });
}

/// A single toolbar button — #77's cell (real button, egui-level
/// disablement keeps the Menu-kind popup open on click; tooltips and
/// accessible names carry the command label, #56) with the dropdown
/// chevron for the Flip slot.
fn bar_button(ui: &mut Ui, chrome: &Chrome, it: &Item, act: &mut Option<Act>) {
    let dropdown = matches!(it.kind, Kind::FlipFlyout);
    let w = if dropdown { DROPDOWN_W } else { BTN };
    let resp = ui.add_enabled(
        it.enabled,
        egui::Button::new("").min_size(Vec2::new(w, BTN)),
    );
    resp.widget_info(|| {
        egui::WidgetInfo::labeled(egui::Role::Button, it.enabled, it.label.clone())
    });
    let resp = if it.enabled {
        resp.on_hover_text(it.label.clone())
    } else {
        resp.on_disabled_hover_text(it.label.clone())
    };
    let r = resp.rect;
    let p = ui.painter();
    if it.enabled && resp.hovered() {
        p.rect_filled(r, 3.0, chrome.tokens.hover_fill);
    }
    let glyph = if dropdown { 15.0 } else { 16.0 };
    let ir = if dropdown {
        // The glyph leads and the chevron trails; #257 mirrors BOTH
        // with the chrome (Greptile PR #277: mirroring only the
        // chevron stacked it on the glyph in RTL).
        let gx = if crate::i18n::rtl() {
            r.right() - 2.0 - glyph
        } else {
            r.left() + 2.0
        };
        Rect::from_min_size(
            egui::pos2(gx, r.top() + (BTN - glyph) / 2.0),
            Vec2::splat(glyph),
        )
    } else {
        Rect::from_center_size(r.center(), Vec2::splat(glyph))
    };
    it.icon
        .paint_tinted(ui, ir, chrome.tokens.icon_tint(it.icon.office_color(), it.enabled));
    if dropdown {
        // The ▾ cue at the cell's trailing edge (the ribbon combo
        // idiom); #257: mirrors with the chrome.
        let cx = if crate::i18n::rtl() {
            r.left() + 5.0
        } else {
            r.right() - 5.0
        };
        p.add(egui::Shape::convex_polygon(
            vec![
                Pos2::new(cx - 2.5, r.center().y - 1.0),
                Pos2::new(cx + 2.5, r.center().y - 1.0),
                Pos2::new(cx, r.center().y + 2.0),
            ],
            chrome.tokens.icon_tint(it.icon.office_color(), it.enabled),
            Stroke::NONE,
        ));
        // The two-entry flyout (the Group ▸ pattern: Popup::menu on
        // the owned response, CloseOnClick).
        let mut picked: Option<Act> = None;
        egui::Popup::menu(&resp)
            .close_behavior(egui::PopupCloseBehavior::CloseOnClick)
            .show(|ui| {
                ui.set_min_width(170.0);
                // Match the parent menu's flat-row look (the popup
                // gets fresh visuals of its own).
                let t = theme::menu_tokens(ui.visuals());
                ui.visuals_mut().widgets.inactive.bg_fill = Color32::TRANSPARENT;
                ui.visuals_mut().widgets.hovered.bg_fill = t.hover_fill;
                ui.visuals_mut().widgets.noninteractive.bg_stroke.color = t.separator;
                if text_item(ui, &strs().msg_flip_vertical()) {
                    picked = Some(Act::FlipVertical);
                }
                if text_item(ui, &strs().msg_flip_horizontal()) {
                    picked = Some(Act::FlipHorizontal);
                }
            });
        if let Some(a) = picked {
            *act = Some(a);
        }
    } else if let Kind::Plain(a) = it.kind {
        if it.enabled && resp.clicked() {
            *act = Some(a);
        }
    }
}

/// The Styles gallery tile — the screenshot's rightmost control: a
/// boxed cell spanning both rows, glyph over its caption, chevron at
/// the leading top corner. Stand-in action until a styles feature:
/// open the Format Shape pane.
fn styles_tile(ui: &mut Ui, chrome: &Chrome, act: &mut Option<Act>) {
    let label = strs().msg_styles();
    let size = Vec2::new(58.0, 2.0 * ROW_H);
    let resp = ui.add_enabled(true, egui::Button::new("").min_size(size));
    resp.widget_info(|| {
        egui::WidgetInfo::labeled(egui::Role::Button, true, label.clone())
    });
    let resp = resp.on_hover_text(label.clone());
    let r = resp.rect;
    let p = ui.painter();
    if resp.hovered() {
        p.rect_filled(r, 3.0, chrome.tokens.hover_fill);
    }
    p.rect_stroke(r, 3.0, Stroke::new(1.0, chrome.sep), egui::StrokeKind::Middle);
    let glyph = 24.0;
    let gr = Rect::from_min_size(
        Pos2::new(r.center().x - glyph / 2.0, r.top() + 4.0),
        Vec2::splat(glyph),
    );
    Icon::Theme.paint_tinted(ui, gr, chrome.tokens.icon_tint(Icon::Theme.office_color(), true));
    p.text(
        Pos2::new(r.center().x, r.bottom() - 10.0),
        egui::Align2::CENTER_CENTER,
        label,
        egui::FontId::proportional(9.5),
        chrome.tokens.label,
    );
    // The gallery's dropdown cue at the leading top corner (#257:
    // mirrors with the chrome).
    let cx = if crate::i18n::rtl() {
        r.left() + 6.0
    } else {
        r.right() - 6.0
    };
    p.add(egui::Shape::convex_polygon(
        vec![
            Pos2::new(cx - 2.5, r.top() + 7.0),
            Pos2::new(cx + 2.5, r.top() + 7.0),
            Pos2::new(cx, r.top() + 10.0),
        ],
        chrome.tokens.icon_tint(Icon::Theme.office_color(), true),
        Stroke::NONE,
    ));
    if resp.clicked() {
        *act = Some(Act::Styles);
    }
}

/// A flat menu text row (the ctx_item look, no icon/accelerator) for
/// the flyout entries.
fn text_item(ui: &mut Ui, label: &str) -> bool {
    let tokens = theme::menu_tokens(ui.visuals());
    let rtl = crate::i18n::rtl();
    let mut job = egui::text::LayoutJob {
        halign: if rtl {
            egui::Align::RIGHT
        } else {
            egui::Align::LEFT
        },
        ..egui::text::LayoutJob::default()
    };
    job.append(
        &format!("  {label}"),
        0.0,
        egui::TextFormat::simple(egui::FontId::proportional(12.5), tokens.label),
    );
    let resp = ui.add_enabled(
        true,
        egui::Button::new(job).min_size(Vec2::new(ui.available_width().max(150.0), 21.0)),
    );
    resp.clicked()
}

fn strs() -> &'static crate::l10n::L10nLanguage {
    crate::i18n::strs()
}
