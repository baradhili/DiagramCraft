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
        // TEXTWIDTH = the text's bare advance width (CodeRabbit
        // cross-check against Visio's own caches: DBUML_M's
        // `TEXTWIDTH(TheText)` on 'PK\r\n' at 10 pt caches 0.17536 in
        // = Segoe UI's bare P+K, no insets; the trailing break never
        // widens a line — measure trimmed like `emit_text`, #148).
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
        // TEXTHEIGHT = wrapped lines × line height + the block's top
        // and bottom insets (corpus gold: DBUML_M's all-zero-margin
        // 'PK\r\n' caches exactly 1.2 em — no insets added; TIMELN's
        // default-T/B-margins shape caches 1 line + 2×0.0555). The
        // wrap runs against the width minus the LEFT/RIGHT insets —
        // the same usable width `emit_text` wraps at, so the
        // refreshed cache and the rendered lines cannot disagree.
        let text = text.trim_end();
        let ins_tb = self.tb.insets.2 + self.tb.insets.3;
        let lines = if text.is_empty() {
            // An empty block still composes one line (the corpus's
            // bare "\r\n" texts measure one line height, #148).
            1.0
        } else {
            self.lines_at(text, width).len() as f64
        };
        Some(lines * self.tb.line_height + ins_tb)
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
                // `shape_env` carries Char.Size only when the chain
                // resolves a numeric cell; rendering always has a size
                // (resolve_text_block defaults one), so fill any gap
                // with the SAME size measurement uses — a formula
                // referencing Char.Size now evaluates against what
                // will actually be drawn (CodeRabbit).
                env.entry("Char.Size".to_string())
                    .or_insert(measure.tb.font_size);
                let mut refreshed: Vec<(String, f64)> = Vec::new();
                for name in TEXT_DERIVED_CELLS {
                    let Some(f) = dc_core::effective_formula(doc, s, &chain, name) else {
                        continue;
                    };
                    let up = f.to_ascii_uppercase();
                    if !up.contains("TEXTWIDTH") && !up.contains("TEXTHEIGHT") {
                        continue; // the cell's size does not depend on the text
                    }
                    // Later cells see earlier refreshes (CodeRabbit):
                    // TxtHeight's formula references the REFRESHED
                    // TxtWidth, not the stale cache.
                    for (n, v) in &refreshed {
                        env.insert(n.clone(), *v);
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
        // Corpus equation (CodeRabbit pass on #147): height = lines ×
        // line height + top/bottom insets (default 2×0.0555 here).
        let ins_tb = m.tb.insets.2 + m.tb.insets.3;
        assert!((ins_tb - 0.111).abs() < 1e-9, "fixture uses default T/B insets");
        let h1 = m.text_height(&format!("{one}\r\n"), 2.0).unwrap();
        let h1_plain = m.text_height(one, 2.0).unwrap();
        assert_eq!(h1, h1_plain, "trailing CRLF adds no line (#148)");
        assert!((h1 - (m.tb.line_height + ins_tb)).abs() < 1e-12);
        // Empty text still composes one line (plus the same insets).
        assert_eq!(m.text_height("", 2.0), Some(m.tb.line_height + ins_tb));
        assert_eq!(
            m.text_height("\r\n", 2.0),
            Some(m.tb.line_height + ins_tb),
            "a bare break is one empty paragraph — one line (#148)"
        );
        // Two visual lines add exactly ONE more line height.
        let h2 = m.text_height(&format!("{one}\r\n{one}\r\n"), 2.0).unwrap();
        assert!((h2 - h1 - m.tb.line_height).abs() < 1e-12);
        // A narrow block wraps the same words into more lines.
        let wide = m.text_height("one two three four", 4.0).unwrap();
        let narrow = m.text_height("one two three four", 0.6).unwrap();
        assert!(narrow > wide, "narrow={narrow} wide={wide}");
        // Zero-margin blocks (DBUML's 'PK\r\n' masters, all four
        // margins explicit 0) add NOTHING: exactly n line heights —
        // the corpus's 1.2004 em single-line gold.
        let mut z = text_aware_shape();
        for n in ["LeftMargin", "RightMargin", "TopMargin", "BottomMargin"] {
            z.sheet.set_cell(n, Cell::value("0"));
        }
        let chain_z = [&z];
        let mz = SheetTextMeasure::new(&doc, &z, &chain_z, "");
        assert!((mz.tb.insets.2 + mz.tb.insets.3).abs() < 1e-12);
        assert!(
            (mz.text_height("PK\r\n", 1.0).unwrap() - mz.tb.line_height).abs() < 1e-12,
            "zero margins: exactly one line height"
        );
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

    /// TxtHeight evaluates against the REFRESHED TxtWidth (CodeRabbit
    /// on #147): a text that would wrap at the stale width but fits at
    /// the refreshed one must cache a one-line height.
    #[test]
    fn txt_height_uses_the_refreshed_txt_width() {
        let mut doc = Document::default();
        let mut s = text_aware_shape();
        // Stale cache: TxtWidth V=1.0 (usable 0.8 with default insets)
        // — the long text wraps there. The formula is
        // MAX(Width=2.0, TEXTWIDTH) so the refresh widens the block to
        // 2.0 (usable 1.8) where the same text fits on one line.
        s.sheet.set_cell("Width", Cell::value("2.0"));
        s.sheet.set_cell("TxtWidth", Cell {
            v: Some("1.0".into()),
            f: Some("MAX(Width,TEXTWIDTH(TheText))".into()),
            ..Cell::default()
        });
        doc.pages.push(vsdx::model::Page {
            id: 0,
            shapes: vec![s],
            ..vsdx::model::Page::default()
        });
        let scope = Scope::Shape { page: 0, shape: 0 };
        let text = "a moderately wide label";
        let mut m = text_edit_macro(&doc, &scope, text);
        m.apply(&mut doc).unwrap();
        let after = &doc.pages[0].shapes[0].sheet;
        let th: f64 = after.cell("TxtHeight").unwrap().v.clone().unwrap().parse().unwrap();
        let tw: f64 = after.cell("TxtWidth").unwrap().v.clone().unwrap().parse().unwrap();
        assert!((tw - 2.0).abs() < 1e-9, "MAX(Width, TEXTWIDTH) = {tw}");
        let chain: [&Shape; 0] = [];
        let shape = &doc.pages[0].shapes[0];
        let measure = SheetTextMeasure::new(&doc, shape, &chain, text);
        let one_line = measure.text_height(text, 2.0).unwrap();
        // Preconditions on the fixture text: it wraps at the STALE
        // width and fits at the REFRESHED one.
        assert!(
            measure.text_height(text, 1.0).unwrap() > one_line,
            "fixture must wrap at the stale width"
        );
        assert!(
            (th - one_line).abs() < 1e-9,
            "one line (+ insets) at the refreshed width: {th} vs {one_line}"
        );
    }
}
