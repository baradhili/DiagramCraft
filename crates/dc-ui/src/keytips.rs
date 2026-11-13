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
//! #257 item 3: a label that carries no ASCII alphanumerics (Persian,
//! Arabic, Hebrew chrome) must not degrade the whole tab to the digit
//! fallback — the RTL Office editions keep LATIN keytips anchored to
//! the English command names while the visible labels localize (the
//! keyboard surface stays stable across languages and layouts, so the
//! same Alt sequence drives every edition). [`assign_control_letters`]
//! therefore takes per-control ANCHOR labels (the English a11y names,
//! supplied by the badge collector) and keys the letters off them when
//! the visible label offers no candidates.
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
/// `Clone` only since #246: labels translate per language.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Hit {
    /// Fire this action through the app's dispatch (QAT digits and
    /// content-row controls); `enabled` mirrors the button — a
    /// disabled hit is a no-op, like Office ignoring it.
    Action {
        action: Action,
        enabled: bool,
        /// The control's label (Controls-level assignment only).
        label: String,
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
    /// The control's English label (#257): the Latin anchor a
    /// non-Latin label keys its letter off (Office's RTL editions
    /// keep English access keys). `None` for strip-level badges,
    /// which carry fixed keys and never join the assignment.
    pub anchor: Option<String>,
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
/// label (case-insensitive), else of the control's English anchor
/// (#257 — a Persian/Arabic/Hebrew label has no ASCII candidates and
/// keys off its English name, the RTL Office scheme), else the first
/// unused digit `1`..=`9`. Labels whose alphabet and digits are
/// exhausted get `None` (no badge — with a tab's worth of controls
/// this does not happen).
pub fn assign_control_letters(labels: &[String]) -> Vec<Option<char>> {
    assign_control_letters_reserved(labels, &[], &[])
}

/// [`assign_control_letters`] with keys other visible badges already
/// carry — the QAT digits stay visible at the Controls level, so a
/// fallback digit must never repeat one (#218 review round) — and the
/// English anchors for non-Latin labels (#257). `anchors` runs
/// parallel to `labels`; a missing/empty anchor falls straight
/// through to the digits.
pub fn assign_control_letters_reserved(
    labels: &[String],
    reserved: &[char],
    anchors: &[Option<String>],
) -> Vec<Option<char>> {
    let mut taken: Vec<char> = reserved.to_vec();
    let mut out = Vec::with_capacity(labels.len());
    for (i, label) in labels.iter().enumerate() {
        let letter = |s: &str| {
            s.chars()
                .map(|c| c.to_ascii_uppercase())
                .filter(|c| c.is_ascii_alphanumeric())
                .find(|c| !taken.contains(c))
        };
        let pick = letter(label)
            .or_else(|| anchors.get(i).and_then(|a| a.as_deref()).and_then(letter))
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
    // Keys already visible at the Controls level (the QAT digits) are
    // taken before any fallback digit collides with them.
    let reserved: Vec<char> = badges
        .iter()
        .filter(|b| !b.key.is_empty() && matches!(b.hit, Hit::Action { .. }))
        .filter_map(|b| b.key.chars().next())
        .collect();
    let pending: Vec<usize> = badges
        .iter()
        .enumerate()
        .filter(|(_, b)| b.key.is_empty() && matches!(b.hit, Hit::Action { .. }))
        .map(|(i, _)| i)
        .collect();
    let labels: Vec<String> = pending
        .iter()
        .map(|&i| match badges[i].hit.clone() {
            Hit::Action { label, .. } => label,
            _ => String::new(),
        })
        .collect();
    let anchors: Vec<Option<String>> = pending
        .iter()
        .map(|&i| badges[i].anchor.clone())
        .collect();
    for (&i, letter) in pending
        .iter()
        .zip(assign_control_letters_reserved(&labels, &reserved, &anchors))
    {
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
            !matches!(&b.hit, Hit::Tab(_) | Hit::File | Hit::TellMe)
        }
        _ => !matches!(&b.hit, Hit::Action { label, .. } if !label.is_empty()),
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
        let out = assign_control_letters(&["Paste".to_string(), "Cut".to_string(), "Copy".to_string(), "Paste".to_string()]);
        // P, C(ut), O — Copy's C is free (Cut took C? no: Cut takes C
        // first), so Copy falls to O; the second Paste falls to A.
        assert_eq!(
            out,
            vec![Some('P'), Some('C'), Some('O'), Some('A')]
        );
    }

    #[test]
    fn non_alphanumerics_are_skipped() {
        let out = assign_control_letters(&["Zoom In".to_string(), "Zoom Out".to_string()]);
        // Z, then O — the space never becomes a keytip.
        assert_eq!(out, vec![Some('Z'), Some('O')]);
    }

    #[test]
    fn digits_fallback_when_letters_run_out() {
        // 26 labels exhausting A..Z, then one more: the 27th falls to
        // the first unused digit.
        let all: Vec<String> = vec!["Alpha", "Bravo", "Charlie", "Delta", "Echo", "Foxtrot", "Golf",
            "Hotel", "India", "Juliet", "Kilo", "Lima", "Mike", "November", "Oscar",
            "Papa", "Quebec", "Romeo", "Sierra", "Tango", "Uniform", "Victor",
            "Whiskey", "Xray", "Yankee", "Zulu", "After"]
        .into_iter()
        .map(String::from)
        .collect();
        let out = assign_control_letters(&all);
        assert!(out[0..26].iter().all(|c| c.is_some()));
        assert_eq!(out[26], Some('1'));
    }

    #[test]
    fn reserved_keys_are_never_reused() {
        // Every letter already taken: the digit fallback must skip the
        // reserved digits (a live QAT badge) and start after them.
        let reserved: Vec<char> = "ABCDEFGHIJKLMNOPQRSTUVWXYZ34".chars().collect();
        let labels: Vec<String> = ["Alpha", "Bravo", "Charlie", "Delta", "Echo"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let out = assign_control_letters_reserved(&labels, &reserved, &[]);
        assert_eq!(
            out,
            vec![Some('1'), Some('2'), Some('5'), Some('6'), Some('7')]
        );
    }

    /// #257: a Persian label has no ASCII candidates — the letter keys
    /// off the control's English anchor (the RTL Office scheme), not
    /// the digit fallback. An empty anchor still falls through to the
    /// digits.
    #[test]
    fn non_latin_labels_key_off_english_anchors() {
        let labels: Vec<String> = ["چسباندن", "برش", "کپی"].iter().map(|s| s.to_string()).collect();
        let anchors: Vec<Option<String>> = vec![
            Some("Paste".to_string()),
            Some("Cut".to_string()),
            None, // no anchor → digit fallback
        ];
        let out = assign_control_letters_reserved(&labels, &[], &anchors);
        assert_eq!(out, vec![Some('P'), Some('C'), Some('1')]);
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
