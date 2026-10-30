//! Real-font text metrics for the SVG renderer (GitHub #21).
//!
//! The wrap decision in [`crate::svg::emit_text`] used to be a
//! ~0.5 em average-advance guess, re-wrapping lines the drawing
//! author had fitted to the block. This module measures per-glyph
//! advances from the SAME faces the rasterizer draws:
//!
//! - the database is **fontdb** — the same crate resvg's rasterizer
//!   uses — loaded with the vendored metric faces (Carlito,
//!   style-complete; Liberation Sans/Serif/Mono for the Arial/Times/
//!   Courier substitution chains — #145) plus **every installed
//!   system font** (the app renders with real Arial/Calibri/Noto where
//!   installed; owner note 2026-10-27);
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
//! Shaped measurement (#146): usvg 0.49 rasterizes text through
//! harfrust (BIDI visual runs at base LTR, default features — pair
//! kerning on, ligatures and contextual forms applied, script and
//! language guessed). `width_in` runs the SAME pipeline over the SAME
//! face, so the wrap decision and the raster shape identically by
//! construction. Glyphs the resolved face lacks keep the
//! per-character Carlito/conservative fallback advance (usvg swaps a
//! fallback font in there — the one documented divergence, coverage
//! gaps only).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

/// Conservative advance for a glyph no loaded face covers (wide-ish
/// Latin; CJK full-width runs wider, but coverage gaps are rare in the
/// corpus and never wrap-decisive at this precision).
const UNCOVERED_EM: f64 = 0.55;

/// Every vendored face (GitHub #145): the Carlito set is style-complete
/// (Regular/Bold/Italic/BoldItalic — italic used to measure as the
/// upright face), and the Liberation metric faces (Sans/Serif/Mono × 4
/// styles) make the Arial/Times New Roman/Courier New substitution
/// chains resolve the face they NAME on every machine, not only hosts
/// with Liberation installed. Licenses live beside the files.
const EMBEDDED_FONTS: [&[u8]; 16] = [
    include_bytes!("../assets/fonts/Carlito-Regular.ttf"),
    include_bytes!("../assets/fonts/Carlito-Bold.ttf"),
    include_bytes!("../assets/fonts/Carlito-Italic.ttf"),
    include_bytes!("../assets/fonts/Carlito-BoldItalic.ttf"),
    include_bytes!("../assets/fonts/LiberationSans-Regular.ttf"),
    include_bytes!("../assets/fonts/LiberationSans-Bold.ttf"),
    include_bytes!("../assets/fonts/LiberationSans-Italic.ttf"),
    include_bytes!("../assets/fonts/LiberationSans-BoldItalic.ttf"),
    include_bytes!("../assets/fonts/LiberationSerif-Regular.ttf"),
    include_bytes!("../assets/fonts/LiberationSerif-Bold.ttf"),
    include_bytes!("../assets/fonts/LiberationSerif-Italic.ttf"),
    include_bytes!("../assets/fonts/LiberationSerif-BoldItalic.ttf"),
    include_bytes!("../assets/fonts/LiberationMono-Regular.ttf"),
    include_bytes!("../assets/fonts/LiberationMono-Bold.ttf"),
    include_bytes!("../assets/fonts/LiberationMono-Italic.ttf"),
    include_bytes!("../assets/fonts/LiberationMono-BoldItalic.ttf"),
];

/// CSS generic keywords the chain builder can emit — they resolve to
/// the embedded default, matching the raster db's generic mappings.
fn is_generic(name: &str) -> bool {
    matches!(
        name,
        "serif" | "sans-serif" | "monospace" | "cursive" | "fantasy" | "system-ui"
    )
}

/// The font database: the vendored faces plus system fonts, or
/// vendored-only for hermetic measurement (system fonts stay the
/// measurement source wherever installed — the vendored set is the
/// deterministic floor, owner note 2026-10-30).
struct MetricsDb {
    db: fontdb::Database,
    /// The vendored Carlito faces (per-character coverage + the
    /// chain-end default), one per (weight, style) corner.
    carlito: fontdb::ID,
    carlito_bold: fontdb::ID,
    carlito_italic: fontdb::ID,
    carlito_bold_italic: fontdb::ID,
}

/// CSS-style distance from the wanted posture (CodeRabbit, #145):
/// an Oblique face serves an italic request ahead of Normal — the
/// same order resvg's fontdb query uses, so measurement and raster
/// pick the same face when a family lacks a true Italic.
fn style_penalty(have: fontdb::Style, want_italic: bool) -> usize {
    use fontdb::Style::{Italic, Normal, Oblique};
    match (want_italic, have) {
        (true, Italic) | (true, Oblique) => 0,
        (true, Normal) => 1,
        (false, Normal) => 0,
        (false, Italic) | (false, Oblique) => 1,
    }
}

/// Rank one face against the wanted (weight, style): upright faces
/// stay preferred for upright text (v1 measured italic as upright),
/// and a family that lacks the wanted style still resolves its
/// nearest face instead of nothing.
fn style_rank(f: &fontdb::FaceInfo, want_style: fontdb::Style, target: i32) -> (usize, i32) {
    (
        style_penalty(f.style, want_style == fontdb::Style::Italic),
        (f.weight.0 as i32 - target).abs(),
    )
}

fn build_db(embedded_only: bool) -> MetricsDb {
    let mut db = fontdb::Database::new();
    for bytes in EMBEDDED_FONTS {
        db.load_font_data(bytes.to_vec());
    }
    if !embedded_only {
        db.load_system_fonts();
    }
    let pick = |bold: bool, italic: bool| -> Option<fontdb::ID> {
        let want_style = if italic { fontdb::Style::Italic } else { fontdb::Style::Normal };
        let target = if bold { 700 } else { 400 };
        db.faces()
            .filter(|f| f.families.iter().any(|(n, _)| n.eq_ignore_ascii_case("Carlito")))
            .filter(|f| matches!(f.style, fontdb::Style::Normal | fontdb::Style::Italic))
            .min_by_key(|f| style_rank(f, want_style, target))
            .map(|f| f.id)
    };
    MetricsDb {
        carlito: pick(false, false).expect("vendored Carlito Regular is embedded"),
        carlito_bold: pick(true, false).expect("vendored Carlito Bold is embedded"),
        carlito_italic: pick(false, true).expect("vendored Carlito Italic is embedded"),
        carlito_bold_italic: pick(true, true).expect("vendored Carlito BoldItalic is embedded"),
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
/// Carlito fallback. `bold`/`italic` prefer the family's matching
/// weight/style corner (#145: the italic faces are vendored, so
/// italic text now measures italic advances instead of the upright
/// face's).
pub fn resolve(font: Option<&str>, bold: bool, italic: bool) -> ResolvedFace {
    let m = metrics();
    let target = if bold { 700 } else { 400 };
    let want_style = if italic { fontdb::Style::Italic } else { fontdb::Style::Normal };
    let installed = |family: &str| -> Option<fontdb::ID> {
        m.db
            .faces()
            .filter(|f| {
                f.families
                    .iter()
                    .any(|(n, _)| n.eq_ignore_ascii_case(family))
            })
            .min_by_key(|f| style_rank(f, want_style, target))
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
    let id = match (bold, italic) {
        (false, false) => m.carlito,
        (true, false) => m.carlito_bold,
        (false, true) => m.carlito_italic,
        (true, true) => m.carlito_bold_italic,
    };
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

    /// Per-(database, face, char) advance cache (#21 review):
    /// `with_face_data` re-opens and re-parses the face file on every
    /// call, and the wrap loop measures text char by char — repeated
    /// glyphs would reopen the file each time. Missing advances are
    /// cached too (the fallback chain re-asks the same face). The
    /// database is keyed by its static's address (as `usize` — a
    /// static Mutex must be `Send`): fontdb IDs are slotmap keys,
    /// unique per `Database`, and the system and embedded statics can
    /// coexist in one test process.
    fn advance_of(&self, id: fontdb::ID, c: char) -> Option<f64> {
        // (database, face, char) → advance, missing ones included.
        type Advances =
            std::sync::Mutex<std::collections::HashMap<(usize, fontdb::ID, char), Option<f64>>>;
        static ADVANCES: OnceLock<Advances> = OnceLock::new();
        let m = metrics();
        let key = (m as *const MetricsDb as usize, id, c);
        let cache = ADVANCES.get_or_init(Default::default);
        if let Some(hit) = cache.lock().unwrap().get(&key) {
            return *hit;
        }
        let out = m.db.with_face_data(id, |data, index| {
            let face = ttf_parser::Face::parse(data, index).ok()?;
            let gid = face.glyph_index(c)?;
            let adv = face.glyph_hor_advance(gid)?;
            Some(adv as f64 / face.units_per_em() as f64)
        })?;
        cache.lock().unwrap().insert(key, out);
        out
    }

    /// Width of `text` at `size_in` inches (Char.Size units): the
    /// SHAPED advance sum (#146) — the same harfrust shaping usvg
    /// 0.49 rasterizes with (pair kerning on, ligatures and
    /// contextual forms applied, BIDI visual runs at base LTR,
    /// script/language guessed), missing glyphs falling back per
    /// character. The isolated-advance sum survives as the fallback
    /// when the face bytes cannot be shaped.
    pub fn width_in(&self, text: &str, size_in: f64) -> f64 {
        self.shaped_width_em(text) * size_in
    }

    /// Shaped width of `text` in em units, through a per-(database,
    /// face, text) cache — the wrap loop measures growing candidate
    /// strings repeatedly, and a shaped width is far too costly to
    /// recompute per candidate. The cache is bounded by clearing it
    /// wholesale past 65 536 entries (text blocks are small; this is
    /// a recompute, not a correctness, bound).
    fn shaped_width_em(&self, text: &str) -> f64 {
        if text.is_empty() {
            return 0.0;
        }
        type Widths =
            std::sync::Mutex<std::collections::HashMap<(usize, fontdb::ID, String), f64>>;
        static WIDTHS: OnceLock<Widths> = OnceLock::new();
        let m = metrics();
        let key = (m as *const MetricsDb as usize, self.id, text.to_string());
        let cache = WIDTHS.get_or_init(Default::default);
        if let Some(hit) = cache.lock().unwrap().get(&key) {
            return *hit;
        }
        let fallback = |c: char| self.char_em(c);
        let db_key = m as *const MetricsDb as usize;
        let width = m
            .db
            .with_face_data(self.id, |data, index| {
                shaped_face(db_key, self.id, data, index)
                    .map(|shaped| shape_width_em(&shaped, text, &fallback))
            })
            .flatten()
            .unwrap_or_else(|| text.chars().map(fallback).sum());
        let mut c = cache.lock().unwrap();
        if c.len() > 65_536 {
            c.clear();
        }
        c.insert(key, width);
        width
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

/// A face prepared for harfrust shaping: the face bytes and
/// `FontRef` are leaked once per (database, face) — the process-wide
/// database owns its data for the process lifetime, so the leak is
/// bounded by the set of faces actually measured, and it is what
/// makes the `&'static FontRef` (and the `ShaperData` built from it)
/// shareable as a plain `Arc` cache entry.
struct ShapedFace {
    upem: f64,
    font: &'static harfrust::FontRef<'static>,
    shaper_data: harfrust::ShaperData,
}

/// The per-(database, face) shaped-face cache (see [`ShapedFace`]).
fn shaped_face(
    db_key: usize,
    id: fontdb::ID,
    data: &[u8],
    index: u32,
) -> Option<std::sync::Arc<ShapedFace>> {
    type Faces =
        std::sync::Mutex<std::collections::HashMap<(usize, fontdb::ID), std::sync::Arc<ShapedFace>>>;
    static FACES: OnceLock<Faces> = OnceLock::new();
    let key = (db_key, id);
    let cache = FACES.get_or_init(Default::default);
    if let Some(hit) = cache.lock().unwrap().get(&key) {
        return Some(hit.clone());
    }
    let bytes: &'static [u8] = Box::leak(data.to_vec().into_boxed_slice());
    let font: &'static harfrust::FontRef<'static> =
        Box::leak(Box::new(harfrust::FontRef::from_index(bytes, index).ok()?));
    let shaper_data = harfrust::ShaperData::new(font);
    let upem = ttf_parser::Face::parse(data, index)
        .ok()
        .map(|f| f.units_per_em() as f64)
        .unwrap_or(1000.0);
    let shaped = std::sync::Arc::new(ShapedFace {
        upem,
        font,
        shaper_data,
    });
    cache.lock().unwrap().insert(key, shaped.clone());
    Some(shaped)
}

/// The usvg-0.49 shaping pipeline, measurement-side (#146): BIDI
/// visual runs (base LTR), each run shaped by harfrust with DEFAULT
/// features — pair kerning on, ligatures/contextual forms applied,
/// script and language guessed — advances summed in em units. That is
/// verbatim what usvg does per text chunk (its `shape_text`), so a
/// width measured here is the width the rasterizer will draw.
fn shape_width_em(face: &ShapedFace, text: &str, fallback: &dyn Fn(char) -> f64) -> f64 {
    use harfrust::{Direction, ShapeOptions, UnicodeBuffer};
    let shaper = face.shaper_data.shaper(face.font).build();
    let bidi = unicode_bidi::BidiInfo::new(text, Some(unicode_bidi::Level::ltr()));
    let Some(paragraph) = bidi.paragraphs.first() else {
        return 0.0;
    };
    let (levels, runs) = bidi.visual_runs(paragraph, paragraph.range.clone());
    let mut total = 0.0;
    for run in runs.iter() {
        let sub = &text[run.clone()];
        if sub.is_empty() {
            continue;
        }
        let direction = if levels[run.start].is_ltr() {
            Direction::LeftToRight
        } else {
            Direction::RightToLeft
        };
        let mut buffer = UnicodeBuffer::new();
        buffer.push_str(sub);
        buffer.set_direction(direction);
        buffer.guess_segment_properties();
        let out = shaper.shape(buffer, ShapeOptions::new());
        let infos = out.glyph_infos();
        let positions = out.glyph_positions();
        for i in 0..out.len() {
            let info = &infos[i];
            if info.glyph_id == 0 {
                // Missing glyph: charge the cluster's characters the
                // per-character fallback advance (usvg swaps a fallback
                // font in here; the divergence is coverage-gap only —
                // #146 keeps the documented per-character estimate).
                let start = info.cluster as usize;
                let next = infos
                    .get(i + 1)
                    .map_or(sub.len(), |n| n.cluster as usize);
                let (lo, hi) = if start <= next { (start, next) } else { (next, start) };
                let hi = hi.min(sub.len());
                total += sub
                    .get(lo..hi)
                    .unwrap_or("")
                    .chars()
                    .map(fallback)
                    .sum::<f64>();
            } else {
                total += positions[i].x_advance as f64 / face.upem;
            }
        }
    }
    total
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
pub fn face_data(font: Option<&str>, bold: bool, italic: bool) -> Option<FaceData> {
    let m = metrics();
    let face = resolve(font, bold, italic);
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
        resolve(None, false, false)
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
        let f = resolve(Some("Certainly-Not-Installed-21"), false, false);
        assert_eq!(f.id, m.carlito, "unresolved chains measure Carlito");
        let g = resolve(Some("Certainly-Not-Installed-21"), true, false);
        assert_eq!(g.id, m.carlito_bold, "bold resolves the bold face");
        let i = resolve(Some("Certainly-Not-Installed-21"), false, true);
        assert_eq!(i.id, m.carlito_italic, "italic resolves the italic face");
        let bi = resolve(Some("Certainly-Not-Installed-21"), true, true);
        assert_eq!(bi.id, m.carlito_bold_italic, "bold+italic resolves its corner");
    }

    /// #146: measurement SHAPES the run the way the rasterizer draws
    /// it. A kerned pair (AV in Liberation Serif — the vendored face
    /// carries GPOS kerning) must measure narrower than the isolated
    /// sum; this is exactly the gap that used to wrap kerned lines
    /// one word early.
    #[test]
    fn kerned_pairs_measure_kerned() {
        let f = resolve(Some("Liberation Serif"), false, false);
        let pair = f.width_in("AV", 1.0);
        let isolated = f.width_in("A", 1.0) + f.width_in("V", 1.0);
        assert!(
            pair < isolated,
            "shaped {pair} must be kerned under the isolated sum {isolated}"
        );
        // A kern-free string still measures the plain advance sum
        // (shaping agrees with the isolated tables when no feature
        // applies): same string, spaced so no pair kerns.
        let plain = f.width_in("nn", 1.0);
        let isolated_n = 2.0 * f.width_in("n", 1.0);
        assert!((plain - isolated_n).abs() < 1e-9);
    }

    /// Coverage gaps keep the per-character fallback under shaping
    /// (#146): a CJK glyph Carlito lacks still contributes its
    /// fallback advance inside a shaped run instead of vanishing.
    #[test]
    fn shaped_runs_fall_back_per_character_on_gaps() {
        let f = face();
        let latin = f.width_in("a", 1.0);
        let mixed = f.width_in("按a", 1.0);
        assert!(
            mixed > latin,
            "the missing CJK glyph must add its fallback advance: {mixed} vs {latin}"
        );
    }

    /// The resolved (style, weight) of a face, for assertions about
    /// WHICH face a chain picks (#145 verification).
    fn resolved_style(font: Option<&str>, bold: bool, italic: bool) -> (fontdb::Style, u16) {
        let m = metrics();
        let f = resolve(font, bold, italic);
        let info = m.db.faces().find(|x| x.id == f.id).expect("resolved id is in the db");
        (info.style, info.weight.0)
    }

    /// #145: the Liberation faces are vendored, so the Arial chain
    /// resolves a Liberation Sans face (the system one where
    /// installed, the vendored copy otherwise — either way NOT the
    /// Carlito the chain used to fall through to), and the style
    /// corners resolve their own faces instead of measuring upright.
    #[test]
    fn arial_chain_measures_liberation_and_styles_resolve() {
        let m = metrics();
        let f = resolve(Some("Arial"), false, false);
        assert_ne!(f.id, m.carlito, "Arial resolves Liberation Sans, not Carlito");
        // It measures different advances than the Carlito fallback
        // (the two designs share Arial metrics only approximately).
        let carlito = ResolvedFace { id: m.carlito, carlito: m.carlito, is_carlito: true };
        assert_ne!(
            f.width_in("measurement", 1.0),
            carlito.width_in("measurement", 1.0),
            "Liberation advances differ from Carlito's"
        );
        // Style corners pick the matching (style, weight) face.
        assert_eq!(
            resolved_style(Some("Arial"), false, false),
            (fontdb::Style::Normal, 400)
        );
        assert_eq!(
            resolved_style(Some("Arial"), false, true),
            (fontdb::Style::Italic, 400)
        );
        assert_eq!(
            resolved_style(Some("Arial"), true, true),
            (fontdb::Style::Italic, 700)
        );
    }
}
