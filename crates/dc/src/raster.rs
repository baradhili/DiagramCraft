//! SVG → raster, the stack shared by the GUI canvas and the file
//! exports (#35): one font database, one font-chain resolution and one
//! usvg parse for every consumer, so a PNG exported from the CLI is
//! pixel-identical to the canvas raster of the same page.
//!
//! The egui-facing half (ColorImage conversion, texture cache, raster
//! pool, kittest pins) stays in `dc-ui::svgimg`, which delegates the
//! parse/resolution here.

use std::fmt::Write as _;
use std::sync::Arc;

use resvg::tiny_skia::Pixmap;
use resvg::usvg;

/// The font database, built once: the vendored metric faces —
/// Carlito (style-complete) and Liberation Sans/Serif/Mono (the
/// Arial/Times/Courier substitution chains' named faces, #145) —
/// plus generic family mappings (usvg resolves unknown/no
/// font-family through the generic defaults, which don't exist in a
/// vendored-only database and would drop the text entirely) and every
/// installed system font (the primary source — a host with real
/// Arial/Liberation keeps measuring and rasterizing it).
pub fn font_db() -> Arc<usvg::fontdb::Database> {
    static DB: std::sync::OnceLock<Arc<usvg::fontdb::Database>> = std::sync::OnceLock::new();
    DB.get_or_init(|| {
        let mut db = usvg::fontdb::Database::new();
        const VENDORED: [&[u8]; 16] = [
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
        for bytes in VENDORED {
            db.load_font_data(bytes.to_vec());
        }
        db.set_sans_serif_family("Carlito");
        db.set_serif_family("Carlito");
        db.set_monospace_family("Carlito");
        db.load_system_fonts();
        Arc::new(db)
    })
    .clone()
}

/// Canonical installed family name for `wanted` — case-insensitive,
/// fontconfig-style (fontdb's own query is exact-match, and Visio
/// documents spell families freely: "ARIAL", "Segoe UI").
pub fn installed_family(db: &usvg::fontdb::Database, wanted: &str) -> Option<String> {
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
pub fn resolve_font_chain(
    chain: &str,
    installed: &dyn Fn(&str) -> Option<String>,
) -> Option<String> {
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
/// family the consumer will actually render: against the passed
/// database, or against the vendored-family set when `embedded_only`
/// is set (the GUI's kittest pin — #145: goldens must not depend on
/// installed faces). An unbalanced quote leaves the rest of the input
/// untouched rather than corrupting the SVG.
pub fn resolve_font_families(
    svg: &str,
    db: &usvg::fontdb::Database,
    embedded_only: bool,
) -> String {
    let from_db = |name: &str| installed_family(db, name);
    // Pinned mode resolves against the VENDORED faces only (#145):
    // the substitution chains' named families (Carlito, Liberation)
    // resolve their vendored faces on every machine — the measurement
    // pin in `textmetrics` mirrors exactly this set.
    let vendored_only = |name: &str| {
        const VENDORED: [&str; 4] = [
            "Carlito",
            "Liberation Sans",
            "Liberation Serif",
            "Liberation Mono",
        ];
        VENDORED
            .iter()
            .find(|v| v.eq_ignore_ascii_case(name))
            .map(|v| v.to_string())
    };
    let installed: &dyn Fn(&str) -> Option<String> = if embedded_only {
        &vendored_only
    } else {
        &from_db
    };
    resolve_font_families_with(svg, installed)
}

/// [`resolve_font_families`] with the family lookup injected — unit
/// tests stay hermetic regardless of global mode flags.
pub fn resolve_font_families_with(svg: &str, installed: &dyn Fn(&str) -> Option<String>) -> String {
    const NEEDLE: &str = "font-family=\"";
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

/// Parse an SVG string into a renderable tree — the resolve + usvg
/// parse half of the raster pipeline (#91). Fonts: the SVG backend
/// emits `font-family` fallback chains per text element — the
/// document's family (Visio's default is Calibri), its metric-compatible
/// substitute, Carlito, a CSS generic (GitHub #17). Here each chain is
/// RESOLVED against the database before parsing: the first family
/// actually installed wins, case-insensitively, and a chain nothing
/// satisfies pins to the embedded Carlito (metric-compatible with
/// Calibri, OFL-1.1 — see dc/assets/fonts/LICENSE-Carlito). Text
/// without any font-family defaults to Carlito as well
/// (`Options::font_family`): usvg's own default is "Times New Roman",
/// which rendered default text in a serif on machines with the MS core
/// fonts (GitHub #17).
pub fn parse_svg(svg: &str, db: &Arc<usvg::fontdb::Database>) -> anyhow::Result<usvg::Tree> {
    parse_svg_mode(svg, db, false)
}

/// [`parse_svg`] with the kittest pin (`embedded_only = true`): the
/// vendored faces only, so goldens are machine-independent (#145).
pub fn parse_svg_mode(
    svg: &str,
    db: &Arc<usvg::fontdb::Database>,
    embedded_only: bool,
) -> anyhow::Result<usvg::Tree> {
    let resolved = resolve_font_families(svg, db, embedded_only);
    parse_resolved(&resolved, db)
}

/// The font-chain resolution on its own — the GUI wraps it in the
/// `Resolve` perf stage (`perf::Stage::Resolve`) before handing the
/// resolved SVG to [`parse_resolved`].
pub fn resolve_svg(svg: &str, db: &Arc<usvg::fontdb::Database>, embedded_only: bool) -> String {
    resolve_font_families(svg, db, embedded_only)
}

/// usvg parse of an already font-resolved SVG — the `Parse` half.
pub fn parse_resolved(
    resolved: &str,
    db: &Arc<usvg::fontdb::Database>,
) -> anyhow::Result<usvg::Tree> {
    let opts = usvg::Options {
        fontdb: db.clone(),
        // No-font-family text defaults to the embedded face (GitHub #17).
        font_family: "Carlito".to_string(),
        ..Default::default()
    };
    usvg::Tree::from_str(resolved, &opts).map_err(|e| anyhow::anyhow!("parsing page SVG: {e}"))
}

/// Render a parsed tree at a `target_width`-pixel-wide pixmap (height
/// follows the size aspect), unpremultiplied RGBA — the shared inner
/// half of [`render_width`] and the dc-ui damage patcher.
pub fn render_pixmap(
    tree: &usvg::Tree,
    t: usvg::Transform,
    w: u32,
    h: u32,
) -> anyhow::Result<Pixmap> {
    let mut pixmap =
        Pixmap::new(w, h).ok_or_else(|| anyhow::anyhow!("allocating {w}×{h} raster"))?;
    resvg::render(tree, t, &mut pixmap.as_mut());
    Ok(pixmap)
}

/// Render a parsed tree at a `target_width`-pixel-wide pixmap (height
/// follows the size aspect). Canvas callers cap at their texture
/// budget; file exports pass through [`render_dpi`] instead.
pub fn render_width(tree: &usvg::Tree, target_width: f32) -> anyhow::Result<Pixmap> {
    let size = tree.size();
    let aspect = size.height().max(1e-3) / size.width().max(1e-3);
    let w = target_width.ceil().clamp(1.0, 2048.0) as u32;
    let h = ((target_width * aspect).ceil().max(1.0)) as u32;
    let (w, h) = (w, h.min(2048));
    let t = usvg::Transform::from_scale(
        w as f32 / size.width().max(1e-3),
        h as f32 / size.height().max(1e-3),
    );
    render_pixmap(tree, t, w, h)
}

/// Render a parsed tree at a raster density of `dpi` pixels per SVG
/// user unit INCH — the export path (#35). The SVG's `width`/`height`
/// attributes are `w * 96` px for a `w`-inch viewBox, so 96 dpi
/// reproduces the intrinsic raster one-to-one; a hard ceiling guards
/// against absurd allocation requests.
pub fn render_dpi(tree: &usvg::Tree, dpi: f32) -> anyhow::Result<Pixmap> {
    const MAX_PX: u32 = 16_384;
    let size = tree.size();
    // SVG width attr is inches * 96; derive the true inch width.
    let w_in = size.width() / 96.0;
    let w = ((w_in * dpi).round() as u32).clamp(1, MAX_PX);
    let h = ((w_in.max(1e-3).recip() * size.height() * dpi).round() as u32).clamp(1, MAX_PX);
    let t = usvg::Transform::from_scale(
        w as f32 / size.width().max(1e-3),
        h as f32 / size.height().max(1e-3),
    );
    render_pixmap(tree, t, w, h)
}

/// Encode a pixmap as PNG bytes (premultiplied → unpremultiplied is
/// tiny-skia's own `encode_png`).
pub fn png_bytes(pixmap: &Pixmap) -> anyhow::Result<Vec<u8>> {
    pixmap
        .encode_png()
        .map_err(|e| anyhow::anyhow!("encoding PNG: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A deterministic database: the embedded Carlito set only, no
    /// system fonts — the same faces every machine (and CI) has.
    fn carlito_only_db() -> usvg::fontdb::Database {
        let mut db = usvg::fontdb::Database::new();
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
        // No MS or Liberation faces in a Carlito-only database: the
        // chain resolves to the embedded face — deterministic on any
        // machine.
        let svg = r##"<text font-family="Arial, 'Liberation Sans', Carlito, sans-serif" font-size="0.1">x</text>"##;
        let from_db = |name: &str| installed_family(&db, name);
        assert_eq!(
            resolve_font_families_with(svg, &from_db),
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
        let from_db = |name: &str| installed_family(&db, name);
        assert_eq!(resolve_font_families_with(bad, &from_db), bad);
    }

    #[test]
    fn svg_without_families_is_unchanged() {
        let db = carlito_only_db();
        let plain = r#"<svg><rect x="0"/></svg>"#;
        let from_db = |name: &str| installed_family(&db, name);
        assert_eq!(resolve_font_families_with(plain, &from_db), plain);
    }

    #[test]
    fn embedded_only_mode_pins_vendored_faces() {
        let svg = r#"<text font-family="Arial, 'Liberation Sans', Carlito, sans-serif">x</text>"#;
        let db = carlito_only_db();
        // The kittest pin (embedded_only) resolves the vendored
        // Liberation Sans even though this workstation's system db has
        // no say; the db mode on a Carlito-only database falls to the
        // chain's tail.
        assert_eq!(
            resolve_font_families(svg, &db, true),
            r#"<text font-family="Liberation Sans">x</text>"#
        );
    }

    /// Face stability of the export path under the embedded pin: the
    /// same SVG parses and rasterizes deterministically through the
    /// shared stack on every machine — an Arial chain resolves the
    /// vendored Liberation Sans (identical pixels), and Liberation and
    /// Carlito are provably different faces.
    #[test]
    fn rasterize_is_deterministic_per_face() {
        let db = font_db();
        let head = r#"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="50"><text x="10" y="35" font-size="24""#;
        let tail = ">Hamburg fi 51</text></svg>";
        let arial_chain =
            format!(r#"{head} font-family="Arial, 'Liberation Sans', Carlito, sans-serif"{tail}"#);
        let explicit_liberation = format!(r#"{head} font-family="Liberation Sans"{tail}"#);
        let explicit_carlito = format!(r#"{head} font-family="Carlito"{tail}"#);
        let raster = |svg: &str| -> Vec<u8> {
            png_bytes(&render_width(&parse_svg_mode(svg, &db, true).unwrap(), 200.0).unwrap())
                .unwrap()
        };
        let a = raster(&arial_chain);
        let l = raster(&explicit_liberation);
        let b = raster(&explicit_carlito);
        assert_eq!(
            a, l,
            "Arial chain == vendored Liberation Sans under the pin"
        );
        assert_ne!(l, b, "Liberation and Carlito are different faces");
    }

    /// A minimal parseable page: usvg needs a size to derive the raster
    /// height from.
    fn tiny_svg() -> String {
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><rect width="10" height="10" fill="red"/></svg>"#.into()
    }

    #[test]
    fn dpi_render_sizes_follow_the_density() {
        let db = font_db();
        let tree = parse_svg(&tiny_svg(), &db).unwrap();
        // 10 px at 96 dpi intrinsic (0.1042 in wide) → 10 px; at 192 →
        // 20 px.
        assert_eq!(render_dpi(&tree, 96.0).unwrap().width(), 10);
        assert_eq!(render_dpi(&tree, 192.0).unwrap().width(), 20);
    }
}
