//! Office-style Alt keytips (#184): tapping Alt arms a two-level
//! keyboard mode — strip badges over the tabs/QAT/File/Tell Me, then a
//! tab letter switches to that tab and shows its control badges.
//! Letters fire the same `Action` dispatch as a click; Esc backs out
//! one level; a pointer click disarms (observed Office 2016 behavior,
//! reimplemented — no Microsoft assets).
//!
//! Letter assignment follows Office's rule, made deterministic: the
//! strip uses the Office-2016 family letters documented below, and a
//! tab's controls take the first unused letter of each label (skipping
//! non-alphanumerics), falling back to unused digits.
//!
//! | Strip target  | Letter | Note                                     |
//! |---------------|--------|------------------------------------------|
//! | File          | F      | Office family letter                     |
//! | Home          | H      |                                          |
//! | Insert        | N      |                                          |
//! | Design        | G      |                                          |
//! | Data          | A      |                                          |
//! | Review        | R      |                                          |
//! | View          | W      |                                          |
//! | Developer     | V      |                                          |
//! | Container Tools | C    | first free letter (contextual tab, ours) |
//! | Tell Me       | Q      | Office's Alt+Q convention                |
//! | QAT buttons   | 1..=9  | by position                              |

use egui::{Color32, Context, Id, LayerId, Order, Pos2, Rect, Vec2};

use crate::ribbon::{Action, Tab};

/// The armed level. `Strip` badges the tab strip (tabs, File, Tell
/// Me, QAT digits); `Controls` badges the active tab's ribbon content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mode {
    Strip,
    Controls,
}

/// What a badge activates when its letter arrives.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Hit {
    /// Fire this action through the app's dispatch (QAT digits and
    /// content-row controls); `enabled` mirrors the button — a
    /// disabled hit is a no-op, like Office ignoring it.
    Action {
        action: Action,
        enabled: bool,
        /// The control's label (Controls-level assignment only).
        label: &'static str,
    },
    /// Switch to this tab and show its control badges.
    Tab(Tab),
    /// Open the File backstage view.
    File,
    /// The Tell Me box (a status note for now, like its click).
    TellMe,
}

/// One keytip badge: a letter over a layout rect. Collected fresh
/// every frame while the shell paints; `key` is empty for
/// content-row controls until [`assign_control_letters`] runs at
/// paint time.
#[derive(Debug, Clone)]
pub(crate) struct Badge {
    pub rect: Rect,
    pub key: String,
    pub hit: Hit,
}

/// Per-app keytip state. The app collects badges during layout, then
/// paints them (armed only) after the ribbon panels close.
#[derive(Debug, Default)]
pub struct Keytips {
    pub(crate) mode: Option<Mode>,
    /// A letter consumed at frame start, resolved at paint time —
    /// after this frame's badges are collected and assigned (the
    /// resolver must never see the just-cleared list).
    pub(crate) pending: Option<char>,
    /// Office arms keytips on the Alt TAP — press and release with
    /// nothing in between. True from a clean Alt press until any
    /// other key press or the release (#218 review round).
    pub(crate) alt_down_clean: bool,
    pub(crate) badges: Vec<Badge>,
    /// The tab strip's and the ribbon content row's rects this frame
    /// (the armed veil covers their union).
    pub(crate) strip_rect: Option<Rect>,
    pub(crate) content_rect: Option<Rect>,
}

impl Keytips {
    pub fn new() -> Self {
        Self::default()
    }

    pub(crate) fn armed(&self) -> bool {
        self.mode.is_some()
    }

    /// Clear the per-frame layout captures (badges and veil rects) at
    /// the top of `App::draw`.
    pub(crate) fn begin_frame(&mut self) {
        self.badges.clear();
        self.strip_rect = None;
        self.content_rect = None;
    }

    pub(crate) fn disarm(&mut self) {
        self.mode = None;
    }
}

/// The strip-level letter for a fixed tab (the table in the module
/// docs). `None` for the File tab — it is the `Hit::File` badge.
pub fn tab_keytip(t: Tab) -> Option<&'static str> {
    match t {
        Tab::File => None, // F — carried by the File badge itself
        Tab::Home => Some("H"),
        Tab::Insert => Some("N"),
        Tab::Design => Some("G"),
        Tab::Data => Some("A"),
        Tab::Review => Some("R"),
        Tab::View => Some("W"),
        Tab::Developer => Some("V"),
        Tab::ContainerTools => Some("C"),
    }
}

/// Assign each label its control letter, in order (Office's rule,
/// deterministic): the first still-unused ASCII alphanumeric of the
/// label (case-insensitive), else the first unused digit `1`..=`9`.
/// Labels whose alphabet and digits are exhausted get `None` (no
/// badge — with a tab's worth of controls this does not happen).
pub fn assign_control_letters(labels: &[&'static str]) -> Vec<Option<char>> {
    let mut taken: Vec<char> = Vec::new();
    let mut out = Vec::with_capacity(labels.len());
    for label in labels {
        let pick = label
            .chars()
            .map(|c| c.to_ascii_uppercase())
            .filter(|c| c.is_ascii_alphanumeric())
            .find(|c| !taken.contains(c))
            .or_else(|| ('1'..='9').find(|d| !taken.contains(d)));
        if let Some(c) = pick {
            taken.push(c);
        }
        out.push(pick);
    }
    out
}

/// Fill the `Controls`-level badges' letters in collection (layout)
/// order — the same deterministic pass as [`assign_control_letters`].
/// Badges that already carry a key (the strip letters, QAT digits)
/// keep it and never join the letter pool.
pub(crate) fn assign_badge_letters(badges: &mut [Badge]) {
    let pending: Vec<usize> = badges
        .iter()
        .enumerate()
        .filter(|(_, b)| b.key.is_empty() && matches!(b.hit, Hit::Action { .. }))
        .map(|(i, _)| i)
        .collect();
    let labels: Vec<&'static str> = pending
        .iter()
        .map(|&i| match badges[i].hit {
            Hit::Action { label, .. } => label,
            _ => "",
        })
        .collect();
    for (&i, letter) in pending.iter().zip(assign_control_letters(&labels)) {
        if let Some(c) = letter {
            badges[i].key = c.to_string();
        }
    }
}

/// Paint the armed veil + badges on a foreground layer above the
/// ribbon (visual only — the layer takes no input; letter dispatch
/// lives in the app's input step).
pub(crate) fn paint(ctx: &Context, kt: &Keytips) {
    if !kt.armed() {
        return;
    }
    let dark = matches!(ctx.theme(), egui::Theme::Dark);
    let layer = ctx.layer_painter(LayerId::new(Order::Foreground, Id::unique("keytips")));
    // Office's "hovered out" ribbon: the chrome dims under the badges.
    if let Some(veil) = kt
        .strip_rect
        .into_iter()
        .chain(kt.content_rect)
        .reduce(|a, b| a.union(b))
    {
        layer.rect_filled(
            veil,
            0.0,
            if dark {
                Color32::from_black_alpha(120)
            } else {
                Color32::from_white_alpha(150)
            },
        );
    }
    // Level-scoped badge visibility (Office): the strip level shows
    // the tab letters / File / Tell Me / QAT digits; the control
    // level shows the tab's control letters (+ the still-live QAT
    // digits) and no strip letters.
    let show = |b: &Badge| match kt.mode {
        Some(Mode::Controls) => {
            !matches!(b.hit, Hit::Tab(_) | Hit::File | Hit::TellMe)
        }
        _ => !matches!(b.hit, Hit::Action { label, .. } if !label.is_empty()),
    };
    for b in kt.badges.iter().filter(|b| show(b) && !b.key.is_empty()) {
        let text = if dark {
            Color32::WHITE
        } else {
            Color32::from_gray(40)
        };
        let gal = ctx.fonts_mut(|f| {
            f.layout_no_wrap(b.key.clone(), egui::FontId::proportional(10.0), text)
        });
        // The chip: a small rounded square hugging the control's
        // bottom-left corner (Office's badge position).
        let size = gal.size() + Vec2::new(6.0, 4.0);
        let min = Pos2::new(
            (b.rect.left() + 1.0).min(b.rect.right() - size.x),
            (b.rect.bottom() - size.y - 1.0).max(b.rect.top()),
        );
        let chip = Rect::from_min_size(min, size);
        layer.rect_filled(
            chip,
            2.0,
            if dark {
                Color32::from_gray(25)
            } else {
                Color32::WHITE
            },
        );
        layer.rect_stroke(
            chip,
            2.0,
            egui::Stroke::new(
                1.0,
                if dark {
                    Color32::from_gray(90)
                } else {
                    Color32::from_gray(160)
                },
            ),
            egui::StrokeKind::Middle,
        );
        layer.galley(
            Pos2::new(chip.left() + 3.0, chip.top() + 2.0),
            gal,
            Color32::WHITE,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn letters_follow_label_order_and_skip_taken() {
        let out = assign_control_letters(&["Paste", "Cut", "Copy", "Paste"]);
        // P, C(ut), O — Copy's C is free (Cut took C? no: Cut takes C
        // first), so Copy falls to O; the second Paste falls to A.
        assert_eq!(
            out,
            vec![Some('P'), Some('C'), Some('O'), Some('A')]
        );
    }

    #[test]
    fn non_alphanumerics_are_skipped() {
        let out = assign_control_letters(&["Zoom In", "Zoom Out"]);
        // Z, then O — the space never becomes a keytip.
        assert_eq!(out, vec![Some('Z'), Some('O')]);
    }

    #[test]
    fn digits_fallback_when_letters_run_out() {
        // 26 labels exhausting A..Z, then one more: the 27th falls to
        // the first unused digit.
        let all = vec!["Alpha", "Bravo", "Charlie", "Delta", "Echo", "Foxtrot", "Golf",
            "Hotel", "India", "Juliet", "Kilo", "Lima", "Mike", "November", "Oscar",
            "Papa", "Quebec", "Romeo", "Sierra", "Tango", "Uniform", "Victor",
            "Whiskey", "Xray", "Yankee", "Zulu", "After"];
        let out = assign_control_letters(&all);
        assert!(out[0..26].iter().all(|c| c.is_some()));
        assert_eq!(out[26], Some('1'));
    }

    #[test]
    fn tab_letters_are_unique() {
        let letters: Vec<&str> = [
            Tab::Home,
            Tab::Insert,
            Tab::Design,
            Tab::Data,
            Tab::Review,
            Tab::View,
            Tab::Developer,
            Tab::ContainerTools,
        ]
        .iter()
        .filter_map(|t| tab_keytip(*t))
        .collect();
        let mut seen = std::collections::HashSet::new();
        assert!(letters.iter().all(|l| seen.insert(l)));
    }
}
