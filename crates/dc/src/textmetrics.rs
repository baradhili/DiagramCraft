//! Real-font text metrics for the SVG renderer (GitHub #21).
//!
//! The wrap decision in [`crate::svg::emit_text`] used to be a
//! ~0.5 em average-advance guess, re-wrapping lines the drawing
//! author had fitted to the block. This module measures per-glyph
//! advances from the SAME faces the rasterizer draws:
//!
//! - the database is **fontdb** — the same crate resvg's rasterizer
//!   uses — loaded with the vendored metric-compatible Carlito plus
//!   **every installed system font** (the app renders with real
//!   Arial/Calibri/Noto where installed; owner note 2026-10-27);
//! - family resolution walks the CSS chain the renderer emits
//!   (`Calibri, Carlito, sans-serif`, …): the first INSTALLED named
//!   family wins (case-insensitive, like `svgimg`'s raster font db),
//!   generic keywords and chains nothing satisfies pin to the
//!   embedded Carlito;
//! - a glyph the resolved face lacks falls back PER CHARACTER to
//!   Carlito and finally to a conservative advance estimate — the
//!   rasterizer substitutes per glyph too (system fonts stay
//!   available for scripts Carlito lacks).
//!
//! The layer architecture (system catalog + vendored fallback +
//! per-character coverage fallback) follows cadcraft/wordcraft's
//! font crates (owner pointer, 2026-10-27); this is the
//! wrap-metrics-sized cut of it.
//!
//! Determinism: [`pin_embedded_for_tests`] swaps the process-wide
//! database for the Carlito-only one, mirroring
//! `svgimg::pin_embedded_fonts_for_tests` — kittest goldens and
//! unit tests measure the vendored face on every machine. Real
//! documents measure with the user's installed fonts, exactly what
//! rasterizes.
//!
//! Not modeled (v1): OpenType pair kerning (resvg applies it; the
//! sub-glyph differences rarely move a wrap decision) and complex
//! shaping (Arabic joining changes advance sums — the wrap treats a
//! run as the sum of isolated advances).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

/// Conservative advance for a glyph no loaded face covers (wide-ish
/// Latin; CJK full-width runs wider, but coverage gaps are rare in
/// the corpus and never wrap-decisive at this precision).
const UNCOVERED_EM: f64 = 0.55;

/// CSS generic keywords the chain builder can emit — they resolve to
/// the embedded default, matching the raster db's generic mappings.
fn is_generic(name: &str) -> bool {
    matches!(
        name,
        "serif" | "sans-serif" | "monospace" | "cursive" | "fantasy" | "system-ui"
    )
}

/// The font database: vendored Carlito + system fonts, or
/// Carlito-only for hermetic measurement.
struct MetricsDb {
    db: fontdb::Database,
    /// The vendored fallback faces (per-character coverage + the
    /// chain-end default).
    carlito: fontdb::ID,
    carlito_bold: fontdb::ID,
}

fn build_db(embedded_only: bool) -> MetricsDb {
    let mut db = fontdb::Database::new();
    db.load_font_data(include_bytes!("../assets/fonts/Carlito-Regular.ttf").to_vec());
    db.load_font_data(include_bytes!("../assets/fonts/Carlito-Bold.ttf").to_vec());
    if !embedded_only {
        db.load_system_fonts();
    }
    let pick = |want_bold: bool| -> Option<fontdb::ID> {
        db.faces()
            .filter(|f| f.families.iter().any(|(n, _)| n.eq_ignore_ascii_case("Carlito")))
            .filter(|f| f.style == fontdb::Style::Normal)
            .min_by_key(|f| (f.weight.0 as i32 - if want_bold { 700 } else { 400 }).abs())
            .map(|f| f.id)
    };
    MetricsDb {
        carlito: pick(false).expect("vendored Carlito Regular is embedded"),
        carlito_bold: pick(true).expect("vendored Carlito Bold is embedded"),
        db,
    }
}

/// Kittest/unit determinism: pin the process to the Carlito-only
/// database (see the module docs). Set once by the test harnesses;
/// there is deliberately no way to unset it.
static PIN_EMBEDDED: AtomicBool = AtomicBool::new(false);

/// Test hook for hermetic measurement (see the module docs).
#[doc(hidden)]
pub fn pin_embedded_for_tests() {
    PIN_EMBEDDED.store(true, Ordering::Relaxed);
}

fn metrics() -> &'static MetricsDb {
    static SYSTEM: OnceLock<MetricsDb> = OnceLock::new();
    static EMBEDDED: OnceLock<MetricsDb> = OnceLock::new();
    if PIN_EMBEDDED.load(Ordering::Relaxed) {
        EMBEDDED.get_or_init(|| build_db(true))
    } else {
        SYSTEM.get_or_init(|| build_db(false))
    }
}

/// One resolved measurement face. Cheap to construct (an id into the
/// process-wide database); measure through [`Self::width_in`].
pub struct ResolvedFace {
    id: fontdb::ID,
    carlito: fontdb::ID,
    is_carlito: bool,
}

/// Resolve the face a shape's text measures (and rasterizes) in: the
/// family chain the renderer emits, first installed family wins,
/// Carlito fallback. `bold` prefers the family's bold weight.
pub fn resolve(font: Option<&str>, bold: bool) -> ResolvedFace {
    let m = metrics();
    let target = if bold { 700 } else { 400 };
    let installed = |family: &str| -> Option<fontdb::ID> {
        m.db
            .faces()
            .filter(|f| {
                f.families
                    .iter()
                    .any(|(n, _)| n.eq_ignore_ascii_case(family))
            })
            // Italic measures as regular (v1 — see module docs).
            .filter(|f| f.style == fontdb::Style::Normal || f.style == fontdb::Style::Italic)
            .min_by_key(|f| (f.weight.0 as i32 - target).abs())
            .map(|f| f.id)
    };
    for name in crate::svg::family_chain(font) {
        if is_generic(&name) {
            break; // the raster db maps generics to Carlito
        }
        if let Some(id) = installed(&name) {
            return ResolvedFace { id, carlito: m.carlito, is_carlito: false };
        }
    }
    let id = if bold { m.carlito_bold } else { m.carlito };
    ResolvedFace { id, carlito: m.carlito, is_carlito: true }
}

impl ResolvedFace {
    /// Advance of one char in em units, from the face tables —
    /// falling back per character to the vendored Carlito, then to a
    /// conservative estimate (the rasterizer substitutes per glyph
    /// too).
    fn char_em(&self, c: char) -> f64 {
        if let Some(a) = self.advance_of(self.id, c) {
            return a;
        }
        if !self.is_carlito {
            if let Some(a) = self.advance_of(self.carlito, c) {
                return a;
            }
        }
        UNCOVERED_EM
    }

    fn advance_of(&self, id: fontdb::ID, c: char) -> Option<f64> {
        metrics().db.with_face_data(id, |data, index| {
            let face = ttf_parser::Face::parse(data, index).ok()?;
            let gid = face.glyph_index(c)?;
            let adv = face.glyph_hor_advance(gid)?;
            Some(adv as f64 / face.units_per_em() as f64)
        })?
    }

    /// Width of `text` at `size_in` inches (Char.Size units): the sum
    /// of per-glyph advances.
    pub fn width_in(&self, text: &str, size_in: f64) -> f64 {
        text.chars().map(|c| self.char_em(c)).sum::<f64>() * size_in
    }

    /// The face's ascender in em units (hhea) — the GUI overlay places
    /// its editor from the renderer's baseline math, so it needs the
    /// same face's ascent (#22 review).
    pub fn ascent_em(&self) -> f64 {
        metrics()
            .db
            .with_face_data(self.id, |data, index| {
                ttf_parser::Face::parse(data, index)
                    .ok()
                    .map(|f| f.ascender() as f64 / f.units_per_em() as f64)
            })
            .flatten()
            .unwrap_or(0.8)
    }
}

/// The actual face a shape's text rasterizes in, as bytes — for
/// consumers that must RENDER with the same font (the GUI's in-place
/// editor draws its overlay in the shape's real face, not the
/// embedded default: #22 review — WYSIWYG means the actual font).
#[derive(Clone)]
pub struct FaceData {
    /// The face's canonical family name ("Arial" when the system's
    /// Arial won the chain, "Carlito" for the vendored fallback).
    pub family: String,
    /// The font file's bytes (a collection's face comes with its
    /// `index`).
    pub data: std::sync::Arc<Vec<u8>>,
    /// Face index inside a collection (0 for plain files).
    pub index: u32,
    /// The ascender in em units (see [`ResolvedFace::ascent_em`]).
    pub ascent_em: f64,
}

/// The bytes + identity of the face `resolve` picks for `font` —
/// reading a system font file on first use (vendored faces are
/// embedded; callers cache what they register from this).
pub fn face_data(font: Option<&str>, bold: bool) -> Option<FaceData> {
    let m = metrics();
    let face = resolve(font, bold);
    let info = m.db.faces().find(|f| f.id == face.id)?;
    let family = info
        .families
        .first()
        .map(|(n, _)| n.clone())
        .unwrap_or_else(|| "Carlito".to_string());
    let data = match &info.source {
        fontdb::Source::Binary(arc) => {
            // The vendored faces (and any memory-registered font).
            let bytes: &[u8] = (**arc).as_ref();
            std::sync::Arc::new(bytes.to_vec())
        }
        fontdb::Source::File(path) => std::sync::Arc::new(std::fs::read(path).ok()?),
        fontdb::Source::SharedFile(path, _) => std::sync::Arc::new(std::fs::read(path).ok()?),
        #[allow(unreachable_patterns)] // memmap-off builds see only Binary+File
        _ => return None,
    };
    Some(FaceData {
        family,
        data,
        index: info.index,
        ascent_em: face.ascent_em(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Measurement uses the embedded Carlito everywhere (hermetic on
    /// any machine); these tests only assert relationships that hold
    /// under metric-compatible substitution too.
    fn face() -> ResolvedFace {
        resolve(None, false)
    }

    #[test]
    fn width_scales_linearly_with_size() {
        let f = face();
        let w1 = f.width_in("measurement", 0.13889);
        let w2 = f.width_in("measurement", 0.27778);
        assert!((w2 - w1 * 2.0).abs() < 1e-9, "{w1} vs {w2}");
        assert_eq!(f.width_in("", 0.13889), 0.0);
    }

    #[test]
    fn narrow_glyphs_measure_narrower_than_wide_ones() {
        let f = face();
        let i = f.width_in("i", 1.0);
        let m = f.width_in("m", 1.0);
        assert!(i * 2.0 < m, "i={i} m={m} — real metrics, not a flat 0.5 em");
        // And the average is in the same ballpark as the old guess.
        let avg = f.width_in("measurement", 1.0) / "measurement".len() as f64;
        assert!(avg > 0.3 && avg < 0.7, "avg={avg}");
    }

    #[test]
    fn uncovered_characters_measure_with_a_fallback() {
        let f = face();
        // CJK is absent from Carlito: the per-character fallback
        // still yields a positive width.
        assert!(f.width_in("按", 0.13889) > 0.0);
    }

    /// Machine-independent by construction: the family is installed
    /// nowhere, so resolution must land on the vendored faces whether
    /// or not the host carries real Calibri/Arial (an installed family
    /// legitimately wins the chain — that is the point of #21).
    #[test]
    fn resolve_pins_uninstalled_families_to_carlito() {
        let m = metrics();
        let f = resolve(Some("Certainly-Not-Installed-21"), false);
        assert_eq!(f.id, m.carlito, "unresolved chains measure Carlito");
        let g = resolve(Some("Certainly-Not-Installed-21"), true);
        assert_eq!(g.id, m.carlito_bold, "bold resolves the bold face");
    }
}
