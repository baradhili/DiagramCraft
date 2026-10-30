//! Text-function measurement + block-cache refresh (GitHub #147).
//!
//! Text-aware blocks carry formulas like the Utilisation connector's
//! `TxtWidth = MAX(TEXTWIDTH(TheText), 5*Char.Size)` and
//! `TxtHeight = TEXTHEIGHT(TheText, TxtWidth)`; their cached `V`s were
//! measured by Visio with real font metrics at AUTHORING time, so a
//! text edit left the block sized for the old text (#21 measured the
//! wrap but only for the render decision). This module is the
//! measurement side of the fix:
//!
//! - [`SheetTextMeasure`] implements `vsdx::formula::TextMeasure` —
//!   the bottom-crate bridge the evaluator calls `TEXTWIDTH` /
//!   `TEXTHEIGHT` through — measuring with the SAME face, size, line
//!   height, insets and greedy wrap `emit_text` renders with, so the
//!   refreshed caches and the rendered lines cannot disagree. The
//!   trailing line break does NOT add a line (#148's gold verdict:
//!   Visio's own corpus caches show one-visual-line texts measuring
//!   exactly one line height).
//! - [`text_edit_macro`] composes the edit + cache refresh into one
//!   undo step: the `SetText`, then a [`dc_core::SetCellCache`] per
//!   text-derived cell whose effective formula re-evaluates cleanly.
//!   Formula cells keep their `F` byte-identical — only the cache
//!   refreshes (the #59 `group_cache_refresh` precedent for undoable
//!   cache maintenance).
//!
//! Layering: the evaluator lives in `vsdx` and the metrics engine
//! here; the trait parameter is the bridge, per #147's approach — no
//! crate dependency inversion.

use dc_core::{Command, Macro, Scope, SetCellCache, SetText};
use vsdx::formula::TextMeasure;
use vsdx::model::{Document, Shape};

/// The measurement context for one shape's text block: everything
/// `TEXTWIDTH`/`TEXTHEIGHT` need, resolved exactly as the renderer
/// resolves it.
pub struct SheetTextMeasure {
    /// The NEW text — what the formulas must evaluate against (the
    /// sheet still carries the old one while commands are built).
    text: String,
    /// Resolved character formatting + block metrics (one
    /// `resolve_text_block` call; font/size/style, line height,
    /// insets).
    tb: crate::svg::TextBlock,
}

impl SheetTextMeasure {
    /// Resolve the measurement context for `shape` with `text` as the
    /// text the formulas will see.
    pub fn new(doc: &Document, shape: &Shape, chain: &[&Shape], text: impl Into<String>) -> Self {
        let (w, h) = crate::svg::shape_size(doc, shape, chain);
        SheetTextMeasure {
            text: text.into(),
            tb: crate::svg::resolve_text_block(doc, shape, chain, w, h),
        }
    }

    /// The wrapped line count at `width` block inches (insets
    /// subtracted, same as the renderer).
    fn lines_at(&self, text: &str, width: f64) -> Vec<String> {
        let face = crate::textmetrics::resolve(self.tb.font.as_deref(), self.tb.bold, self.tb.italic);
        let usable = (width - self.tb.insets.0 - self.tb.insets.1).max(0.02);
        crate::svg::wrap_block_text(&face, text, self.tb.font_size, usable)
    }
}

impl TextMeasure for SheetTextMeasure {
    fn the_text(&self) -> Option<String> {
        Some(self.text.clone())
    }

    fn text_width(&self, text: &str) -> Option<f64> {
        let face = crate::textmetrics::resolve(self.tb.font.as_deref(), self.tb.bold, self.tb.italic);
        // The trailing break never widens a line (it starts one):
        // measure the trimmed text like `emit_text` does (#148).
        let text = text.trim_end();
        let mut best = 0.0;
        for para in text.replace("\r\n", "\n").replace('\r', "\n").split('\n') {
            let w = face.width_in(para, self.tb.font_size);
            if w > best {
                best = w;
            }
        }
        Some(best)
    }

    fn text_height(&self, text: &str, width: f64) -> Option<f64> {
        let text = text.trim_end();
        if text.is_empty() {
            // An empty block still composes one line (the corpus's
            // bare "\r\n" texts measure one line height, #148).
            return Some(self.tb.line_height);
        }
        Some(self.lines_at(text, width).len() as f64 * self.tb.line_height)
    }
}

/// The text-block cells whose caches a text edit refreshes, in
/// dependency order (`TxtHeight`'s formula references `TxtWidth`).
const TEXT_DERIVED_CELLS: [&str; 2] = ["TxtWidth", "TxtHeight"];

/// One text edit, one undo step: the [`SetText`] plus refreshed
/// cached `V`s for every text-derived block cell whose effective
/// formula re-evaluates cleanly under the new text. Formulas that
/// fail evaluation (metrics gaps, unsupported functions) keep their
/// cached `V` — exactly the pre-#147 behavior for that cell.
///
/// Non-shape scopes and shapes without the cells are untouched: this
/// returns a bare `SetText` macro, and untouched documents never
/// re-evaluate (rendering stays byte-identical without an edit).
pub fn text_edit_macro(doc: &Document, scope: &Scope, text: &str) -> Macro {
    let mut cmds: Vec<Box<dyn Command>> = vec![Box::new(SetText::new(scope.clone(), text))];

    if let Scope::Shape { page, shape } = scope {
        if let (Some(p), Ok(chain)) = (
            doc.pages.iter().find(|p| p.id == *page),
            dc_core::shape_chain(doc, *page, *shape),
        ) {
            if let Some(s) = vsdx::model::find_shape_by_id(&p.shapes, *shape) {
                let measure = SheetTextMeasure::new(doc, s, &chain, text);
                let mut env = crate::svg::shape_env(doc, s, &chain);
                let mut refreshed: Vec<(String, f64)> = Vec::new();
                for name in TEXT_DERIVED_CELLS {
                    let Some(f) = dc_core::effective_formula(doc, s, &chain, name) else {
                        continue;
                    };
                    let up = f.to_ascii_uppercase();
                    if !up.contains("TEXTWIDTH") && !up.contains("TEXTHEIGHT") {
                        continue; // the cell's size does not depend on the text
                    }
                    // TxtHeight sees the REFRESHED TxtWidth when its
                    // formula references it.
                    if let Some((_, v)) = refreshed.iter().find(|(n, _)| n == name) {
                        env.insert(name.to_string(), *v);
                    }
                    match vsdx::formula::eval_measured(&f, &env, &measure) {
                        Ok(v) => {
                            if let Some(n) = v.as_f64() {
                                if n.is_finite() && n > 0.0 {
                                    refreshed.push((name.to_string(), n));
                                }
                            }
                        }
                        Err(_) => continue, // keep the cached V for this cell
                    }
                }
                for (name, v) in refreshed {
                    cmds.push(Box::new(SetCellCache::new(scope.clone(), name, v)));
                }
            }
        }
    }

    Macro::new(format!("set text of {scope}"), cmds)
}

#[cfg(test)]
mod tests {
    use super::*;
    use vsdx::model::{Cell, Shape};

    fn text_aware_shape() -> Shape {
        let mut s = Shape::default();
        s.sheet.set_cell("Width", Cell::value("2.0"));
        s.sheet.set_cell("Height", Cell::value("1.0"));
        s.sheet.set_cell(
            "TxtWidth",
            Cell {
                v: Some("1.0".into()),
                f: Some("MAX(Width,TEXTWIDTH(TheText))".into()),
                ..Cell::default()
            },
        );
        s.sheet.set_cell(
            "TxtHeight",
            Cell {
                v: Some("0.1666666666666667".into()),
                f: Some("TEXTHEIGHT(TheText,TxtWidth)".into()),
                ..Cell::default()
            },
        );
        s
    }

    /// The evaluator-level semantics the corpus gold pinned (#148):
    /// a trailing CRLF adds no line; an empty text composes one line;
    /// height scales with wrapped line count.
    #[test]
    fn measure_matches_the_corpus_line_semantics() {
        let doc = Document::default();
        let s = text_aware_shape();
        let chain = [&s];
        let m = SheetTextMeasure::new(&doc, &s, &chain, "");
        let one = "label";
        let h1 = m.text_height(&format!("{one}\r\n"), 2.0).unwrap();
        let h1_plain = m.text_height(one, 2.0).unwrap();
        assert_eq!(h1, h1_plain, "trailing CRLF adds no line (#148)");
        // Empty text still composes one line.
        assert_eq!(m.text_height("", 2.0), Some(m.tb.line_height));
        assert_eq!(
            m.text_height("\r\n", 2.0),
            Some(m.tb.line_height),
            "a bare break is one empty paragraph — one line (#148)"
        );
        // Two visual lines double the height (wrap-independent).
        let h2 = m.text_height(&format!("{one}\r\n{one}\r\n"), 2.0).unwrap();
        assert!((h2 - 2.0 * h1).abs() < 1e-12);
        // A narrow block wraps the same words into more lines.
        let wide = m.text_height("one two three four", 4.0).unwrap();
        let narrow = m.text_height("one two three four", 0.6).unwrap();
        assert!(narrow > wide, "narrow={narrow} wide={wide}");
        // Width measures the widest paragraph, untrimmed never wider.
        assert!(m.text_width(one).unwrap() > 0.0);
        assert_eq!(
            m.text_width(&format!("{one}\r\n")).unwrap(),
            m.text_width(one).unwrap()
        );
    }

    /// The macro: caches refresh in the same undo step, formulas stay
    /// byte-identical, undo restores the prior caches.
    #[test]
    fn text_edit_macro_refreshes_caches_in_one_undo_step() {
        let mut doc = Document::default();
        doc.pages.push(vsdx::model::Page {
            id: 0,
            shapes: vec![text_aware_shape()],
            ..vsdx::model::Page::default()
        });
        let scope = Scope::Shape { page: 0, shape: 0 };
        let s = doc.pages[0].shapes[0].sheet.clone();

        // A long text: TEXTWIDTH > Width, so TxtWidth must grow, and
        // with it TxtHeight (2 wrapped lines at the default 10 pt).
        let long = "a considerably longer label that cannot fit one line";
        let mut m = text_edit_macro(&doc, &scope, long);
        m.apply(&mut doc).unwrap();

        let after = &doc.pages[0].shapes[0].sheet;
        let tw: f64 = after.cell("TxtWidth").unwrap().v.clone().unwrap().parse().unwrap();
        let th: f64 = after.cell("TxtHeight").unwrap().v.clone().unwrap().parse().unwrap();
        assert!(
            tw > 2.0,
            "TxtWidth = MAX(Width, TEXTWIDTH) must follow the long text: {tw}"
        );
        assert!(th > 0.2, "wrapped height for the long text: {th}");
        // Formulas byte-identical, V-only refresh.
        assert_eq!(
            after.cell("TxtWidth").unwrap().f,
            s.cell("TxtWidth").unwrap().f
        );
        assert_eq!(
            after.cell("TxtHeight").unwrap().f,
            s.cell("TxtHeight").unwrap().f
        );
        // The text itself landed.
        assert_eq!(after.text.as_deref(), Some(long));

        // One undo step restores text AND prior caches.
        m.undo(&mut doc).unwrap();
        let restored = &doc.pages[0].shapes[0].sheet;
        assert_eq!(restored.cell("TxtWidth").unwrap().v, s.cell("TxtWidth").unwrap().v);
        assert_eq!(restored.cell("TxtHeight").unwrap().v, s.cell("TxtHeight").unwrap().v);
        assert_eq!(restored.text, None);
    }

    /// A formula-free block (plain `V` cells, no `F`) never gets
    /// cache writes — the macro is a bare SetText.
    #[test]
    fn formula_free_blocks_stay_bare() {
        let mut doc = Document::default();
        let mut s = Shape::default();
        s.sheet.set_cell("TxtWidth", Cell::value("1.5"));
        s.sheet.set_cell("TxtHeight", Cell::value("0.2"));
        doc.pages.push(vsdx::model::Page {
            id: 0,
            shapes: vec![s],
            ..vsdx::model::Page::default()
        });
        let scope = Scope::Shape { page: 0, shape: 0 };
        let m = text_edit_macro(&doc, &scope, "hello");
        assert_eq!(m.cmds.len(), 1, "no cache refresh without formulas");
    }
}
