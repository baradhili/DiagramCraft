//! SVG → texture rasterization for the canvas, with a small cache.
//! The GUI deliberately renders through the same SVG backend the CLI
//! uses (build plan M5: "rendering via the SVG backend first"), so what
//! you see is what `dc render` produces.

use egui::{ColorImage, Context, TextureHandle, TextureId, TextureOptions};

use std::collections::HashMap;
use std::fmt::Write as _;
use std::sync::atomic::{AtomicBool, Ordering};

/// Kittest/snapshot mode: resolve every font-family to the embedded
/// Carlito instead of the installed system faces, so the goldens
/// render identically on every machine (CI runners have no MS fonts;
/// dev workstations do). Set once by the kittest harnesses before the
/// first frame; the shipped app never sets it.
static PIN_EMBEDDED: AtomicBool = AtomicBool::new(false);

/// Test hook for the kittest harnesses (see `PIN_EMBEDDED`).
#[doc(hidden)]
pub fn pin_embedded_fonts_for_tests() {
    PIN_EMBEDDED.store(true, Ordering::Relaxed);
}

/// Rasterize an SVG string to a `target_width`-pixel-wide RGBA image
/// (height follows the viewBox aspect).
///
/// Fonts: the SVG backend emits `font-family` fallback chains per text
/// element — the document's family (Visio's default is Calibri), its
/// metric-compatible substitute, Carlito, a CSS generic (GitHub #17).
/// Here each chain is RESOLVED against this database before parsing:
/// the first family actually installed wins, case-insensitively
/// (Arial renders as Arial where installed — what the desktop app is
/// for), and a chain nothing satisfies pins to the embedded Carlito
/// (metric-compatible with Calibri, OFL-1.1 — see
/// assets/fonts/LICENSE-Carlito). Text without any font-family
/// defaults to Carlito as well (`Options::font_family`): usvg's own
/// default is "Times New Roman", which rendered default text in a
/// serif on machines with the MS core fonts. System fonts stay
/// available as per-glyph fallback for scripts Carlito lacks (CJK,
/// Arabic, …).
///
/// Determinism: the kittest goldens run under `pin_embedded_fonts_for_tests`
/// (always Carlito), and their fixture documents carry no Character
/// Font cells, so they render the same on every machine; real
/// documents render with the user's real fonts.
/// The font database, built once: Carlito Regular/Bold plus generic
/// family mappings — usvg resolves unknown/no font-family through the
/// generic defaults (Arial / Times New Roman), which don't exist in a
/// Carlito-only database and would drop the text entirely.
fn font_db() -> std::sync::Arc<resvg::usvg::fontdb::Database> {
    static DB: std::sync::OnceLock<std::sync::Arc<resvg::usvg::fontdb::Database>> =
        std::sync::OnceLock::new();
    DB.get_or_init(|| {
        let mut db = resvg::usvg::fontdb::Database::new();
        db.load_font_data(include_bytes!("../assets/fonts/Carlito-Regular.ttf").to_vec());
        db.load_font_data(include_bytes!("../assets/fonts/Carlito-Bold.ttf").to_vec());
        db.set_sans_serif_family("Carlito");
        db.set_serif_family("Carlito");
        db.set_monospace_family("Carlito");
        db.load_system_fonts();
        std::sync::Arc::new(db)
    })
    .clone()
}

/// Canonical installed family name for `wanted` — case-insensitive,
/// fontconfig-style (fontdb's own query is exact-match, and Visio
/// documents spell families freely: "ARIAL", "Segoe UI").
fn installed_family(db: &resvg::usvg::fontdb::Database, wanted: &str) -> Option<String> {
    let wanted = wanted.trim();
    db.faces().find_map(|face| {
        face.families
            .iter()
            .map(|(name, _)| name.as_str())
            .find(|n| n.eq_ignore_ascii_case(wanted))
            .map(str::to_string)
    })
}

/// Resolve one CSS font-family chain (comma-separated, optionally
/// single/double-quoted names — the form our renderer emits): the
/// first INSTALLED named family wins and is returned under its
/// canonical name; generic keywords pass through to the database's
/// deterministic mappings; `None` means the chain names nothing
/// installed and carries no generic — callers pin that case to the
/// embedded Carlito.
fn resolve_font_chain(chain: &str, installed: &dyn Fn(&str) -> Option<String>) -> Option<String> {
    let mut generic: Option<&str> = None;
    for part in chain.split(',') {
        let name = part.trim().trim_matches(|c| c == '\'' || c == '"');
        if name.is_empty() {
            continue;
        }
        match name.to_ascii_lowercase().as_str() {
            "serif" | "cursive" | "fantasy" => generic = generic.or(Some("serif")),
            "monospace" => generic = generic.or(Some("monospace")),
            "sans-serif" | "system-ui" | "ui-sans-serif" => {
                generic = generic.or(Some("sans-serif"))
            }
            _ => {
                if let Some(canonical) = installed(name) {
                    return Some(canonical);
                }
            }
        }
    }
    generic.map(str::to_string)
}

/// Rewrite every `font-family="…"` attribute of the SVG to the single
/// family the GUI will actually render (see the `rasterize` comment).
/// An unbalanced quote leaves the rest of the input untouched rather
/// than corrupting the SVG.
fn resolve_font_families(svg: &str, db: &resvg::usvg::fontdb::Database) -> String {
    const NEEDLE: &str = "font-family=\"";
    let pinned = PIN_EMBEDDED.load(Ordering::Relaxed);
    let from_db = |name: &str| installed_family(db, name);
    let embedded_only =
        |name: &str| (name.eq_ignore_ascii_case("Carlito")).then(|| "Carlito".to_string());
    let installed: &dyn Fn(&str) -> Option<String> = if pinned { &embedded_only } else { &from_db };
    let mut out = String::with_capacity(svg.len());
    let mut rest = svg;
    while let Some(i) = rest.find(NEEDLE) {
        // also swallow the separating space before the attribute
        let cut = if i > 0 && rest.as_bytes()[i - 1] == b' ' {
            i - 1
        } else {
            i
        };
        out.push_str(&rest[..cut]);
        let after = &rest[i + NEEDLE.len()..];
        match after.find('"') {
            Some(j) => {
                let resolved = resolve_font_chain(&after[..j], installed)
                    .unwrap_or_else(|| "Carlito".to_string());
                let _ = write!(out, " font-family=\"{resolved}\"");
                rest = &after[j + 1..];
            }
            None => {
                out.push_str(&rest[cut..]);
                return out;
            }
        }
    }
    out.push_str(rest);
    out
}

/// Parse an SVG string into a renderable tree — the per-SVG-string
/// half of the raster pipeline (#91): font-chain resolve + usvg parse.
/// Runs only when the SVG actually changes (a version bump); a raster
/// width change never re-parses, it re-renders [`render_tree`] on the
/// held tree.
///
/// Fonts: the SVG backend emits `font-family` fallback chains per text
/// element — the document's family (Visio's default is Calibri), its
/// metric-compatible substitute, Carlito, a CSS generic (GitHub #17).
/// Here each chain is RESOLVED against this database before parsing:
/// the first family actually installed wins, case-insensitively
/// (Arial renders as Arial where installed — what the desktop app is
/// for), and a chain nothing satisfies pins to the embedded Carlito
/// (metric-compatible with Calibri, OFL-1.1 — see
/// assets/fonts/LICENSE-Carlito). Text without any font-family
/// defaults to Carlito as well (`Options::font_family`): usvg's own
/// default is "Times New Roman", which rendered default text in a
/// serif on machines with the MS core fonts. System fonts stay
/// available as per-glyph fallback for scripts Carlito lacks (CJK,
/// Arabic, …).
///
/// Determinism: the kittest goldens run under `pin_embedded_fonts_for_tests`
/// (always Carlito), and their fixture documents carry no Character
/// Font cells, so they render the same on every machine; real
/// documents render with the user's real fonts.
/// The font database, built once: Carlito Regular/Bold plus generic
/// family mappings — usvg resolves unknown/no font-family through the
/// generic defaults (Arial / Times New Roman), which don't exist in a
/// Carlito-only database and would drop the text entirely.
pub fn parse_svg(svg: &str, kind: crate::perf::Kind) -> anyhow::Result<resvg::usvg::Tree> {
    let db = font_db();
    let opts = resvg::usvg::Options {
        fontdb: db.clone(),
        // No-font-family text defaults to the embedded face: usvg's own
        // default family is "Times New Roman", which rendered default
        // text in a serif on machines with the MS core fonts installed
        // (GitHub #17).
        font_family: "Carlito".to_string(),
        ..Default::default()
    };
    let resolved = {
        let _s = crate::perf::span(crate::perf::Stage::Resolve(kind));
        resolve_font_families(svg, &db)
    };
    {
        let _s = crate::perf::span(crate::perf::Stage::Parse(kind));
        resvg::usvg::Tree::from_str(&resolved, &opts)
    }
    .map_err(|e| anyhow::anyhow!("parsing page SVG: {e}"))
}

/// Render a parsed tree at a `target_width`-pixel-wide RGBA image
/// (height follows the viewBox aspect) — the per-width half of the
/// pipeline (#91): pixmap alloc, transform, render, RGBA copy.
pub fn render_tree(
    tree: &resvg::usvg::Tree,
    target_width: f32,
    kind: crate::perf::Kind,
) -> anyhow::Result<ColorImage> {
    // Widest texture we ever allocate (kittest caps at 2048; GPUs
    // allow more but upscaling a 2048px raster is fine on screen).
    const MAX_TEX: u32 = 2048;
    let _r = crate::perf::span(crate::perf::Stage::Render(kind));
    let size = tree.size();
    let aspect = size.height().max(1e-3) / size.width().max(1e-3);
    let w = target_width.ceil().clamp(1.0, MAX_TEX as f32) as u32;
    let h = ((target_width * aspect).ceil().max(1.0)) as u32;
    let (w, h) = (w, h.min(MAX_TEX));
    let mut pixmap = resvg::tiny_skia::Pixmap::new(w, h)
        .ok_or_else(|| anyhow::anyhow!("allocating {w}×{h} raster"))?;
    let t = resvg::usvg::Transform::from_scale(
        w as f32 / size.width().max(1e-3),
        h as f32 / size.height().max(1e-3),
    );
    resvg::render(tree, t, &mut pixmap.as_mut());
    let data: &[u8] = pixmap.data();
    Ok(ColorImage::from_rgba_unmultiplied(
        [w as usize, h as usize],
        data,
    ))
}

/// Rasterize an SVG string in one shot — parse + render. The
/// fixed-width callers (stencil thumbnails, icons) pay this once per
/// cached texture; the page path goes through [`TextureCache`], which
/// reuses the parsed tree across raster widths (#91).
pub fn rasterize(
    svg: &str,
    target_width: f32,
    kind: crate::perf::Kind,
) -> anyhow::Result<ColorImage> {
    render_tree(&parse_svg(svg, kind)?, target_width, kind)
}

/// Cache of page rasters keyed by (page part, model version, px-per-inch
/// bucket). Any model mutation bumps `version`, invalidating everything.
/// The parsed `usvg::Tree` is cached per (part, version) — the
/// width-independent half of the raster (#91) — so a ppi-bucket change
/// (a zoom step) re-RENDERS the held tree instead of re-resolving and
/// re-parsing the SVG.
#[derive(Default)]
pub struct TextureCache {
    map: HashMap<(String, u64, u32), TextureHandle>,
    trees: HashMap<(String, u64), resvg::usvg::Tree>,
}

impl TextureCache {
    /// Raster for `(key_part, version, px_per_in)`, building the SVG via
    /// `build_svg` ONLY when the parsed tree is missing (#90/#91): the
    /// cache — not the caller — decides when the page is serialized and
    /// parsed. A texture miss with a live tree (a ppi-bucket change)
    /// re-renders that tree at the new width; the build closure and the
    /// resolve/parse stages run at most once per (part, version), never
    /// per frame. A failed build or parse inserts nothing, so the next
    /// frame retries it.
    pub fn get(
        &mut self,
        ctx: &Context,
        page_w_in: f64,
        key_part: &str,
        version: u64,
        px_per_in: f32,
        mut build_svg: impl FnMut() -> anyhow::Result<String>,
    ) -> anyhow::Result<&TextureHandle> {
        let bucket = (px_per_in * 4.0).round().max(4.0) as u32; // ±0.25 ppi
        let key = (key_part.to_string(), version, bucket);
        if !self.map.contains_key(&key) {
            let tree_key = (key_part.to_string(), version);
            if !self.trees.contains_key(&tree_key) {
                let svg = build_svg()?;
                let tree = parse_svg(&svg, crate::perf::Kind::Page)?;
                self.trees.insert(tree_key, tree);
            }
            // Bucket change with a live tree: render only — no rebuild,
            // no resolve, no parse (#91).
            let image = render_tree(
                &self.trees[&(key_part.to_string(), version)],
                page_w_in as f32 * px_per_in,
                crate::perf::Kind::Page,
            )?;
            let tex = {
                let _s = crate::perf::span(crate::perf::Stage::Upload(crate::perf::Kind::Page));
                ctx.load_texture(
                    format!("page-{key_part}-{version}"),
                    image,
                    TextureOptions::LINEAR,
                )
            };
            self.map.insert(key.clone(), tex);
        }
        // Keep at most a few live entries per page; drop stale versions —
        // the trees with the same policy (one live tree per page entry).
        self.map
            .retain(|(p, v, _), _| p != key_part || *v == version);
        self.trees
            .retain(|(p, v), _| p != key_part || *v == version);
        Ok(&self.map[&key])
    }

    pub fn clear(&mut self) {
        self.map.clear();
        self.trees.clear();
    }

    /// The live texture id for a cache key, if it has been rasterized
    /// (test observability for #58's thumbnail invalidation).
    pub fn id_for(&self, key_part: &str, version: u64, px_per_in: f32) -> Option<TextureId> {
        let bucket = (px_per_in * 4.0).round().max(4.0) as u32; // ±0.25 ppi
        self.map
            .get(&(key_part.to_string(), version, bucket))
            .map(|t| t.id())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A deterministic database: the embedded Carlito set only, no
    /// system fonts — the same faces every machine (and CI) has.
    fn carlito_only_db() -> resvg::usvg::fontdb::Database {
        let mut db = resvg::usvg::fontdb::Database::new();
        db.load_font_data(include_bytes!("../assets/fonts/Carlito-Regular.ttf").to_vec());
        db.load_font_data(include_bytes!("../assets/fonts/Carlito-Bold.ttf").to_vec());
        db
    }

    #[test]
    fn chain_resolves_to_the_first_installed_family() {
        let arial = |n: &str| (n.eq_ignore_ascii_case("Arial")).then(|| "Arial".to_string());
        let liberation = |n: &str| {
            (n.eq_ignore_ascii_case("Liberation Sans")).then(|| "Liberation Sans".to_string())
        };
        let none = |_: &str| None::<String>;
        let chain = "Arial, 'Liberation Sans', Carlito, sans-serif";
        // Installed document family wins, then its substitute.
        assert_eq!(resolve_font_chain(chain, &arial).as_deref(), Some("Arial"));
        assert_eq!(
            resolve_font_chain(chain, &liberation).as_deref(),
            Some("Liberation Sans")
        );
        // Nothing installed: the chain's generic tail carries it; a
        // chain with neither an installed name nor a generic is None
        // (the caller pins that to Carlito).
        assert_eq!(
            resolve_font_chain(chain, &none).as_deref(),
            Some("sans-serif")
        );
        assert_eq!(resolve_font_chain("Wingdings", &none), None);
        // Case-insensitive match, canonical (database) spelling back.
        let canonical = |n: &str| (n.eq_ignore_ascii_case("arial")).then(|| "Arial".to_string());
        assert_eq!(
            resolve_font_chain("ARIAL, Carlito", &canonical).as_deref(),
            Some("Arial")
        );
    }

    #[test]
    fn chains_rewrite_against_the_database() {
        let db = carlito_only_db();
        // No MS fonts in a Carlito-only database: the chain resolves to
        // the embedded face — deterministic on every machine.
        let svg = r##"<text font-family="Arial, 'Liberation Sans', Carlito, sans-serif" font-size="0.1">x</text>"##;
        assert_eq!(
            resolve_font_families(svg, &db),
            r#"<text font-family="Carlito" font-size="0.1">x</text>"#
        );
        assert_eq!(
            installed_family(&db, "carlito").as_deref(),
            Some("Carlito"),
            "case-insensitive, canonical name back"
        );
        assert_eq!(installed_family(&db, "Arial"), None);
    }

    #[test]
    fn unbalanced_quote_leaves_svg_untouched() {
        let db = carlito_only_db();
        let bad = r#"<text font-family="oops>"#;
        assert_eq!(resolve_font_families(bad, &db), bad);
    }

    #[test]
    fn svg_without_families_is_unchanged() {
        let db = carlito_only_db();
        let plain = r#"<svg><rect x="0"/></svg>"#;
        assert_eq!(resolve_font_families(plain, &db), plain);
    }

    /// Face stability under the pinned (kittest) mode: a document
    /// chain, an explicit Carlito and a bare no-family text element
    /// must rasterize to identical pixels — the goldens depend on it.
    #[test]
    fn rasterized_text_is_face_stable_when_pinned() {
        pin_embedded_fonts_for_tests();
        let head = r#"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="50"><text x="10" y="35" font-size="24""#;
        let tail = ">Hamburg fi 51</text></svg>";
        let doc_chain =
            format!(r#"{head} font-family="Arial, 'Liberation Sans', Carlito, sans-serif"{tail}"#);
        let explicit = format!(r#"{head} font-family="Carlito"{tail}"#);
        let bare = format!(r#"{head}{tail}"#);
        let a = rasterize(&doc_chain, 200.0, crate::perf::Kind::Page).unwrap();
        let b = rasterize(&explicit, 200.0, crate::perf::Kind::Page).unwrap();
        let c = rasterize(&bare, 200.0, crate::perf::Kind::Page).unwrap();
        assert_eq!(a.size, b.size);
        assert_eq!(a.pixels, b.pixels, "resolved chain == explicit Carlito");
        assert_eq!(a.pixels, c.pixels, "no-family default == Carlito");
    }

    /// Machine-dependent by design (GitHub #17's requirement: honor
    /// installed fonts): asserts the workstation's MS core fonts are
    /// found through the same database the app builds. Run explicitly
    /// on a box that has them:
    /// `cargo test -p dc-ui --lib -- --ignored`
    #[test]
    #[ignore = "asserts fonts installed on this workstation"]
    fn installed_ms_fonts_resolve_on_this_machine() {
        let db = font_db();
        assert_eq!(installed_family(&db, "Arial").as_deref(), Some("Arial"));
        assert_eq!(installed_family(&db, "Calibri").as_deref(), Some("Calibri"));
    }

    /// A minimal parseable page: usvg needs a size to derive the raster
    /// height from.
    fn tiny_svg() -> anyhow::Result<String> {
        Ok(r#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><rect width="10" height="10" fill="red"/></svg>"#.into())
    }

    /// #90: the build closure runs at most once per (part, version) —
    /// repeated `get`s with the same key pay only the lookup, and only
    /// a version bump, another page, or eviction of the stale version
    /// builds again. A failed build must not be cached.
    #[test]
    fn build_closure_runs_once_per_key() {
        use std::cell::Cell;
        let ctx = Context::default();
        let mut cache = TextureCache::default();
        // The closure owns the counter (Cell so asserts can read it
        // between calls).
        let calls = Cell::new(0u32);
        let mut build = || {
            calls.set(calls.get() + 1);
            tiny_svg()
        };
        // Same key three times — the third inside the same ±0.25 ppi
        // bucket — one build.
        for &ppi in &[96.0f32, 96.0, 96.05] {
            cache.get(&ctx, 4.0, "page1", 1, ppi, &mut build).unwrap();
        }
        assert_eq!(calls.get(), 1, "cached gets must not rebuild the SVG");
        // Version bump → rebuild (the SVG may have changed).
        cache.get(&ctx, 4.0, "page1", 2, 96.0, &mut build).unwrap();
        assert_eq!(calls.get(), 2);
        // New ppi bucket, same version → NO rebuild: the parsed tree is
        // reused and only re-rendered (#91).
        cache.get(&ctx, 4.0, "page1", 2, 300.0, &mut build).unwrap();
        assert_eq!(calls.get(), 2, "a bucket change must not re-serialize");
        // Another page part builds under its own key.
        cache.get(&ctx, 4.0, "page2", 2, 300.0, &mut build).unwrap();
        assert_eq!(calls.get(), 3);
        // The v1 page1 entry was evicted by the retain on the v2 get, so
        // asking for it again rebuilds (eviction policy unchanged, #90).
        cache.get(&ctx, 4.0, "page1", 1, 96.0, &mut build).unwrap();
        assert_eq!(calls.get(), 4);
        // A failed build inserts nothing: the very next get retries.
        let mut failing = || -> anyhow::Result<String> {
            calls.set(calls.get() + 1);
            Err(anyhow::anyhow!("boom"))
        };
        assert!(cache.get(&ctx, 4.0, "badpage", 1, 96.0, &mut failing).is_err());
        assert!(cache.get(&ctx, 4.0, "badpage", 1, 96.0, &mut failing).is_err());
        assert_eq!(calls.get(), 6, "failed builds are not cached");
        assert!(
            cache.id_for("badpage", 1, 96.0).is_none(),
            "no texture may exist for a failed build"
        );
        // A parse failure is not cached either (the tree inserts only
        // after a successful parse).
        let mut unparseable = || -> anyhow::Result<String> {
            calls.set(calls.get() + 1);
            Ok("<not-svg".into())
        };
        assert!(cache.get(&ctx, 4.0, "badpage2", 1, 96.0, &mut unparseable).is_err());
        assert!(cache.get(&ctx, 4.0, "badpage2", 1, 96.0, &mut unparseable).is_err());
        assert_eq!(calls.get(), 8, "failed parses are not cached");
    }

    /// #91: rasterizing one tree at several widths costs one build +
    /// one parse total. The build closure is the only SVG source, so it
    /// never re-invoking across buckets proves the tree was reused
    /// (and the differing texture widths prove each bucket really
    /// rasterized) — observable without the global perf counters.
    #[test]
    fn bucket_sweep_parses_once() {
        use std::cell::Cell;
        let ctx = Context::default();
        let mut cache = TextureCache::default();
        let calls = Cell::new(0u32);
        let mut build = || {
            calls.set(calls.get() + 1);
            tiny_svg()
        };
        for &ppi in &[96.0f32, 192.0, 300.0] {
            cache.get(&ctx, 4.0, "page1", 7, ppi, &mut build).unwrap();
        }
        assert_eq!(calls.get(), 1, "three buckets, one build+parse");
        // Each bucket rasterized at its own width: 4 in × ppi.
        let mut width = |ppi: f32, build: &mut dyn FnMut() -> anyhow::Result<String>| {
            cache
                .get(&ctx, 4.0, "page1", 7, ppi, build)
                .unwrap()
                .size()[0]
        };
        assert_eq!(width(96.0, &mut build), 384);
        assert_eq!(width(192.0, &mut build), 768);
        assert_eq!(width(300.0, &mut build), 1200);
        assert_eq!(calls.get(), 1, "re-gets still build nothing");
    }
}
