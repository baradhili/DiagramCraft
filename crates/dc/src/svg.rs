//! SVG rendering backend (the first `Canvas` implementation).
//!
//! Coordinate mapping (Analysis doc 06): VSDX stores inches with origin
//! bottom-left and Y up; SVG is Y down. Each shape becomes a `<g>` with the
//! XForm transform
//!   translate(PinX, PageH-PinY) rotate(-Angle) scale(FlipX,FlipY)
//!   translate(-LocPinX, +LocPinY)
//! applied to geometry whose local Y is negated at path-build time.

use std::collections::HashMap;
use std::fmt::Write as _;

use vsdx::model::Sheet;
#[cfg(test)]
use vsdx::model::{Section, SheetChild};
use vsdx::{Cell, Document, Row, Shape};

const PX_PER_IN: f64 = 96.0;

/// Text renders as glyphs only while its on-screen em height clears
/// this many pixels; below it the canvas layer path emits gray greek
/// bars instead (GitHub #124 item 2 — Visio greeks text at low zoom,
/// and the bars skip the font resolve / glyph-outline work the usvg
/// parse would otherwise pay for text nobody can read). Export, print,
/// and stencil thumbnails never greek: their calls pass no ppi.
pub const TEXT_GREEK_PX: f64 = 4.0;

/// Count of the recorded text sizes (inches, one entry per text block
/// a layer serialization met) that would greek at `ppi`. The canvas
/// layer cache compares this across ppi-bucket changes: equal counts
/// mean the same blocks greeked on both sides (greeked-ness is
/// monotone in size, so the greeked set is always a prefix), i.e. the
/// retained SVG/tree serializes identically and the re-render can
/// skip the re-parse (#124 item 2 + item 6.3).
pub fn greeked_count(sizes: &[f64], ppi: f32) -> usize {
    sizes
        .iter()
        .filter(|&&s| s * (ppi as f64) < TEXT_GREEK_PX)
        .count()
}

/// Greeking context for canvas-layer serializations (#124 item 2).
/// `Some` only on the `render_shape_layer_opts` path; the ppi decides
/// per text block whether it emits glyphs or bars, and every text
/// size met is recorded so the layer cache can build its re-serialize
/// profile (see [`greeked_count`]).
pub(crate) struct GreekCtx {
    ppi: f32,
    sizes: std::cell::RefCell<Vec<f64>>,
}

impl GreekCtx {
    fn new(ppi: f32) -> Self {
        Self {
            ppi,
            sizes: std::cell::RefCell::new(Vec::new()),
        }
    }

    fn greeks(&self, size_in: f64) -> bool {
        size_in * (self.ppi as f64) < TEXT_GREEK_PX
    }
}

/// Contact sheet of every master's shape tree, laid out on a grid — the
/// rendering path for stencils (`.vssx`), whose pages are empty.
pub fn render_masters(doc: &Document) -> anyhow::Result<String> {
    let cell = 1.2f64; // cell size (inches)
    let cols = 6usize;
    let mut masters: Vec<&vsdx::Master> = doc.masters.iter().collect();
    masters.sort_by_key(|m| m.id);
    let rows = masters.len().div_ceil(cols).max(1);
    let w = cell * cols as f64;
    let h = cell * rows as f64;

    let mut svg = String::new();
    let _ = writeln!(
        svg,
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w} {h}" width="{pw:.0}" height="{ph:.0}">"#,
        pw = w * PX_PER_IN,
        ph = h * PX_PER_IN,
    );
    let _ = writeln!(
        svg,
        r#"<!-- Tarahi master contact sheet ({} masters) -->"#,
        masters.len()
    );
    let _ = writeln!(
        svg,
        r##"<rect x="0" y="0" width="{w}" height="{h}" fill="#ffffff"/>"##,
    );
    for (i, m) in masters.iter().enumerate() {
        let (cx, cy) = ((i % cols) as f64, (i / cols) as f64);
        let _ = writeln!(
            svg,
            r#"<g transform="translate({x},{y})">"#,
            x = cx * cell,
            y = cy * cell,
        );
        let _ = writeln!(
            svg,
            r##"<rect x="0" y="0" width="{c}" height="{c}" fill="none" stroke="#e0e0e0" stroke-width="0.01"/>"##,
            c = cell
        );
        // Center the master's real shape bounds in the cell (masters sit
        // centered on master pages several times the shape size, so the
        // pin position cannot be ignored — see `master_bbox`).
        let size = |n: &str| {
            m.shapes
                .first()
                .and_then(|s| s.sheet.cell(n))
                .and_then(|c| c.numeric())
                .unwrap_or(1.0)
        };
        let (mw, mh) = (size("Width"), size("Height"));
        let master_h = m
            .sheet()
            .and_then(|s| s.cell("PageHeight"))
            .and_then(|c| c.numeric())
            .unwrap_or(mh);
        let bounds = master_bbox(doc, m, master_h).unwrap_or((0.0, 0.0, mw, mh));
        let (tx, ty, k) = fit_transform(cell, 0.7, bounds);
        let _ = writeln!(
            svg,
            r#"<g transform="translate({tx:.5},{ty:.5}) scale({k:.5},{k:.5})">"#,
        );
        for shape in &m.shapes {
            render_shape(
                doc,
                shape,
                master_h,
                &[],
                &m.part,
                None,
                None,
                None,
                &mut svg,
            );
        }
        let _ = writeln!(svg, "</g>");
        let _ = writeln!(
            svg,
            r##"<text x="{x:.4}" y="{y:.4}" font-size="0.07" text-anchor="middle" fill="#333">{name}</text>"##,
            name = escape_text(m.name_u.as_deref().unwrap_or("-")),
            x = cell / 2.0,
            y = cell - 0.06,
        );
        let _ = writeln!(svg, "</g>");
    }
    let _ = writeln!(svg, "</svg>");
    Ok(svg)
}

/// Union bounding box (unrotated) of a master's root shapes, in the
/// master page's y-down render coordinates — the region a thumbnail
/// must show. Masters typically center their shape on a master page
/// several times the shape size, so the pin position cannot be ignored.
fn master_bbox(doc: &Document, m: &vsdx::Master, master_h: f64) -> Option<(f64, f64, f64, f64)> {
    let mut acc: Option<(f64, f64, f64, f64)> = None;
    for s in &m.shapes {
        let chain: Vec<&Shape> = std::iter::once(s).collect();
        let env = shape_env(doc, s, &chain);
        let get = |n: &str| -> Option<f64> {
            doc.resolve_cell(s, &chain, n)
                .and_then(|c| cell_num(&c, &env))
        };
        let (pin_x, pin_y) = (get("PinX")?, get("PinY")?);
        let (w, h) = (get("Width").unwrap_or(1.0), get("Height").unwrap_or(1.0));
        let (lx, ly) = (
            get("LocPinX").unwrap_or(w / 2.0),
            get("LocPinY").unwrap_or(h / 2.0),
        );
        // Same placement math as `render_shape`: the box spans
        // (pin − loc) in x and (page_h − pin + loc − h .. + h) in y.
        let x0 = pin_x - lx;
        let y0 = (master_h - pin_y) + ly - h;
        let rect = (x0, y0, w, h);
        acc = Some(match acc {
            None => rect,
            Some((ax, ay, aw, ah)) => {
                let x1 = (ax + aw).max(rect.0 + rect.2);
                let y1 = (ay + ah).max(rect.1 + rect.3);
                let nx = ax.min(rect.0);
                let ny = ay.min(rect.1);
                (nx, ny, x1 - nx, y1 - ny)
            }
        });
    }
    acc
}

/// Fit `(bx, by, bw, bh)` centered into a `cell`-sized square with
/// `pad` fraction of usable size: the SVG group transform.
fn fit_transform(cell: f64, pad: f64, (bx, by, bw, bh): (f64, f64, f64, f64)) -> (f64, f64, f64) {
    let k = (cell * pad / bw.abs().max(1e-6)).min(cell * pad / bh.abs().max(1e-6));
    let tx = cell / 2.0 - k * (bx + bw / 2.0);
    let ty = cell / 2.0 - k * (by + bh / 2.0);
    (tx, ty, k)
}

/// One master rendered as a standalone SVG (stencil thumbnail): the
/// master's shape tree centered and scaled into a square viewBox.
pub fn render_master(doc: &Document, m: &vsdx::Master) -> anyhow::Result<String> {
    let cell = 1.0f64;
    let mut svg = String::new();
    let _ = writeln!(
        svg,
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {c} {c}" width="{pw:.0}" height="{ph:.0}">"#,
        c = cell,
        pw = cell * PX_PER_IN,
        ph = cell * PX_PER_IN,
    );
    let _ = writeln!(
        svg,
        r##"<rect x="0" y="0" width="{c}" height="{c}" fill="#ffffff"/>"##,
        c = cell
    );
    let size = |n: &str| {
        m.shapes
            .first()
            .and_then(|s| s.sheet.cell(n))
            .and_then(|c| c.numeric())
            .unwrap_or(1.0)
    };
    let (mw, mh) = (size("Width"), size("Height"));
    let master_h = m
        .sheet()
        .and_then(|s| s.cell("PageHeight"))
        .and_then(|c| c.numeric())
        .unwrap_or(mh);
    // Center the master's real shape bounds (not the master page) in
    // the cell; the shape is placed on the master page by `render_shape`
    // and masters sit centered on pages several times their size.
    let bounds = master_bbox(doc, m, master_h).unwrap_or((0.0, 0.0, mw, mh));
    let (tx, ty, k) = fit_transform(cell, 0.78, bounds);
    let _ = writeln!(
        svg,
        r#"<g transform="translate({tx:.5},{ty:.5}) scale({k:.5},{k:.5})">"#,
    );
    for shape in &m.shapes {
        render_shape(
            doc,
            shape,
            master_h,
            &[],
            &m.part,
            None,
            None,
            None,
            &mut svg,
        );
    }
    let _ = writeln!(svg, "</g>");
    let _ = writeln!(svg, "</svg>");
    Ok(svg)
}

pub fn render_page(doc: &Document, page: &vsdx::Page) -> anyhow::Result<String> {
    render_page_with(doc, page, &[])
}

/// The export render of one page (#35): page chrome dropped (no gray
/// sheet border — Visio's own exports carry none), white background
/// per `transparent`. The ink is identical to [`render_page`] — only
/// the chrome differs.
pub fn render_page_export(
    doc: &Document,
    page: &vsdx::Page,
    transparent: bool,
) -> anyhow::Result<String> {
    render_page_full(doc, page, &[], None, None, false, !transparent)
}

/// [`render_page`] with hidden layer members: shapes whose
/// `LayerMember` names one of `hidden` (Visio layer row indexes, the
/// LayerMember cell convention) are skipped entirely — the annotation
/// toggle (#29) and friends.
pub fn render_page_with(
    doc: &Document,
    page: &vsdx::Page,
    hidden: &[i64],
) -> anyhow::Result<String> {
    render_page_opts(doc, page, hidden, None, None)
}

/// [`render_page`] with one shape's TEXT suppressed (GitHub #22): the
/// in-place canvas editor covers the text block, so the page raster
/// underneath must not also show the shape's old text (ghost text
/// under the caret). The shape itself — geometry, fill, borders —
/// keeps rendering; only its own `emit_text` pass is skipped.
pub fn render_page_no_text(
    doc: &Document,
    page: &vsdx::Page,
    hidden: &[i64],
    skip_text_shape: i64,
) -> anyhow::Result<String> {
    render_page_opts(doc, page, hidden, None, Some(skip_text_shape))
}

/// Union sink for the ink-bounds walk with the #219 focus gate:
/// nodes contribute only once the walk is inside the focus subtree
/// (always, when unfocused). `enter` arms the gate at the focus node;
/// the walk saves/restores `inside` around each child so a later
/// sibling of the focus never leaks into the region.
struct Focus {
    acc: Option<(f64, f64, f64, f64)>,
    focus: Option<i64>,
    inside: bool,
}

impl Focus {
    fn enter(&mut self, id: i64) {
        if Some(id) == self.focus {
            self.inside = true;
        }
    }

    fn union(&mut self, r: (f64, f64, f64, f64)) {
        if !self.inside {
            return;
        }
        self.acc = Some(match self.acc {
            None => r,
            Some((ax, ay, aw, ah)) => {
                let x1 = (ax + aw).max(r.0 + r.2);
                let y1 = (ay + ah).max(r.1 + r.3);
                let nx = ax.min(r.0);
                let ny = ay.min(r.1);
                (nx, ny, x1 - nx, y1 - ny)
            }
        });
    }
}

/// The ink bounds of one top-level shape's subtree, in page
/// coordinates (inches, y-down like the SVG viewBox): the union of
/// every node's transformed [0..w]×[-h..0] box and text box, padded to
/// absorb stroke bleed, clamped to the page ± 2 in. This is the
/// viewBox (and the on-canvas quad) of a per-shape layer (#93).
pub fn shape_ink_bounds(
    doc: &Document,
    page: &vsdx::Page,
    shape_id: i64,
) -> Option<(f64, f64, f64, f64)> {
    shape_ink_bounds_focused(doc, page, shape_id, None)
}

/// [`shape_ink_bounds`] restricted to one DESCENDANT's own subtree
/// (#219): `focus` names a sub-shape inside `shape_id`'s subtree and
/// only that sub-shape's subtree is unioned — the damage region a
/// localized edit re-rasters. The cell-resolution chain still runs
/// from the top-level root, so inherited/master cells resolve exactly
/// as the renderer resolves them. `None` is the whole-subtree bounds.
pub fn shape_ink_bounds_focused(
    doc: &Document,
    page: &vsdx::Page,
    shape_id: i64,
    focus: Option<i64>,
) -> Option<(f64, f64, f64, f64)> {
    let page_h = num(page.sheet().and_then(|s| s.cell("PageHeight"))).unwrap_or(11.0);
    let root = page.shapes.iter().find(|s| s.id == shape_id)?;
    let mut u = Focus {
        acc: None,
        focus,
        inside: focus.is_none(),
    };
    fn walk(
        doc: &Document,
        s: &Shape,
        chain: &[&Shape],
        page_h: f64,
        top: bool,
        to_page: Affine,
        u: &mut Focus,
    ) {
        let mut chain: Vec<&Shape> = chain.to_vec();
        chain.push(s);
        let (node_box, a) = node_ink_box(doc, s, &chain, page_h, top, to_page);
        u.union(node_box);
        for sub in &s.sub_shapes {
            let saved = u.inside;
            u.enter(sub.id);
            walk(doc, sub, &chain, page_h, false, a, u);
            u.inside = saved;
        }
    }
    u.enter(root.id);
    walk(doc, root, &[], page_h, true, Affine::IDENTITY, &mut u);
    let (x, y, w, h) = u.acc?;
    // Stroke bleed margin; then clamp to the page ± 2 in so a rogue
    // formula cannot push a layer off to infinity.
    let pad = 0.08;
    let (x, y, w, h) = (x - pad, y - pad, w + 2.0 * pad, h + 2.0 * pad);
    let pw = num(page.sheet().and_then(|s| s.cell("PageWidth"))).unwrap_or(8.5);
    let (min_x, min_y) = (-2.0, -2.0);
    let (max_x, max_y) = (pw + 2.0, page_h + 2.0);
    let x = x.max(min_x);
    let y = y.max(min_y);
    let w = w.min(max_x - x);
    let h = h.min(max_y - y);
    Some((x, y, w.max(0.01), h.max(0.01)))
}

/// One node's own ink box in page space (inches, y-down), with the
/// affine that maps its local frame there: the per-node half of the
/// ink-bounds walk ([`shape_ink_bounds_focused`]) and of the #236
/// cull's subtree-box pass. `chain` includes `s`. The box unions the
/// transformed [0..w]×[-h..0] XForm box, every geometry row's real
/// extent (#202 — curve hulls and arc apexes), and the text block's
/// placement box; no padding, no clamping (the callers decide).
/// Mirrored master content maps into the instance box, so this box
/// bounds it too.
fn node_ink_box(
    doc: &Document,
    s: &Shape,
    chain: &[&Shape],
    page_h: f64,
    top: bool,
    to_page: Affine,
) -> ((f64, f64, f64, f64), Affine) {
    let env = shape_env(doc, s, chain);
    // Takes the env per call (not by capture) so `env` can move into
    // the shared frame env below (#218 review).
    let get = |n: &str, env: &vsdx::formula::Env| -> Option<f64> {
        doc.resolve_cell(s, chain, n)
            .and_then(|c| cell_num(&c, env))
    };
    let width = get("Width", &env).unwrap_or(1.0);
    let height = get("Height", &env).unwrap_or(1.0);
    // The exact transform string render_shape/render_shape_local
    // emit for this node, composed into one affine.
    let local = if top {
        let (sx, sy) = (
            if get("FlipX", &env).unwrap_or(0.0) != 0.0 {
                -1.0
            } else {
                1.0
            },
            if get("FlipY", &env).unwrap_or(0.0) != 0.0 {
                -1.0
            } else {
                1.0
            },
        );
        Affine::translate(
            get("PinX", &env).unwrap_or(0.0),
            page_h - get("PinY", &env).unwrap_or(0.0),
        )
        .then(&Affine::rotate_deg(
            -get("Angle", &env).unwrap_or(0.0).to_degrees(),
        ))
        .then(&Affine::scale(sx, sy))
        .then(&Affine::translate(
            -get("LocPinX", &env).unwrap_or(width / 2.0),
            get("LocPinY", &env).unwrap_or(height / 2.0),
        ))
    } else {
        Affine::translate(get("PinX", &env).unwrap_or(0.0), -get("PinY", &env).unwrap_or(0.0))
            .then(&Affine::rotate_deg(
                -get("Angle", &env).unwrap_or(0.0).to_degrees(),
            ))
            .then(&Affine::translate(
                -get("LocPinX", &env).unwrap_or(width / 2.0),
                get("LocPinY", &env).unwrap_or(height / 2.0),
            ))
    };
    let a = to_page.then(&local);
    let mut u = Focus {
        acc: None,
        focus: None,
        inside: true,
    };
    // The node's own ink box (local y-down) — mirrored master
    // content maps into the instance box, so this bounds it too.
    let box_union = |x0: f64, y0: f64, x1: f64, y1: f64, a: &Affine, u: &mut Focus| {
        let c = [
            a.apply(x0, y0),
            a.apply(x1, y0),
            a.apply(x1, y1),
            a.apply(x0, y1),
        ];
        let xs = [c[0].0, c[1].0, c[2].0, c[3].0];
        let ys = [c[0].1, c[1].1, c[2].1, c[3].1];
        u.union((
            xs.iter().cloned().fold(f64::INFINITY, f64::min),
            ys.iter().cloned().fold(f64::INFINITY, f64::min),
            xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
                - xs.iter().cloned().fold(f64::INFINITY, f64::min),
            ys.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
                - ys.iter().cloned().fold(f64::INFINITY, f64::min),
        ));
    };
    box_union(0.0, -height, width, 0.0, &a, &mut u);
    // #202: the geometry rows' REAL extent — a curved connector's
    // arc belly leaves the begin-corner box (a 1D connector's
    // Height is 0), and a layer viewBox built from the box alone
    // clips the belly into a straight line (every arc-pair
    // connector in utilisation.vsdx rendered flat). Union every
    // row's endpoint and control points: every path this module
    // emits is a line/quad/cubic whose curve stays inside the
    // hull of those points, plus the elliptical arc's apex
    // (2·on-curve − chord mid) as the conservative bound.
    // What the renderer draws: the instance's own Geometry when it
    // carries any, else the MASTER source's rows scaled into the
    // instance frame (render_all_geometry's master path) — a fresh
    // stencil-drop instance renders entirely through it, and its
    // arrowheads and arc bellies must bound the ink too or the
    // layer viewBox clips them and the #236 cull skips them
    // (#218 review round 3).
    let own = s.sheet.sections_named("Geometry");
    // ONE env per frame, shared across every walked section (#218
    // review): the per-entry clones discarded row-coordinate
    // writebacks, so a Geometry2 referencing `Geometry1.X2` fell
    // back to the authored pose (#249's cross-row case, across
    // sections). A shape walks either its OWN sections (instance
    // env) or its master's (the prepared menv) — never both.
    let mut walk: Vec<(&vsdx::Section, (f64, f64))> =
        own.iter().map(|g| (*g, (1.0, 1.0))).collect();
    // Starts as the instance env; the master branch below REPLACES it.
    // The text block after the walk still reads the INSTANCE env's
    // Txt* cells (they are the shape's own), so a copy survives the move.
    let text_env = env.clone();
    let mut frame_env = env;
    if own.is_empty() {
        if let Some(src) = doc.instance_source(s, chain) {
            let mw = src
                .sheet
                .cell("Width")
                .and_then(|c| c.numeric())
                .filter(|v| v.abs() > 1e-9);
            let mh = src
                .sheet
                .cell("Height")
                .and_then(|c| c.numeric())
                .filter(|v| v.abs() > 1e-9);
            let scale = (
                mw.map_or(1.0, |mw| width / mw),
                mh.map_or(1.0, |mh| height / mh),
            );
            // The master env builds ONCE with the instance's GLUED
            // Begin/End and the Scratch/Controls formulas
            // RE-EVALUATED against them — the cached V is the
            // authored pose, and the band masters derive their whole
            // vertical span from them (`Scratch.Y2 =
            // ABS(BeginY-EndY)`, #249).
            let mut menv = shape_env(doc, src, &[]);
            for n in ["BeginX", "BeginY", "EndX", "EndY"] {
                if let Some(v) = doc.resolve_cell(s, chain, n).and_then(|c| c.numeric()) {
                    menv.insert(n.to_string(), v);
                }
            }
            for sec_name in ["Scratch", "Controls"] {
                for sec in src.sheet.sections_named(sec_name) {
                    for row in &sec.rows {
                        for (name, cell) in &row.cells {
                            let key = match sec_name {
                                "Scratch" => format!("Scratch.{}{}", name, row.ix + 1),
                                _ => {
                                    let rn = row
                                        .n
                                        .clone()
                                        .unwrap_or_else(|| format!("Row_{}", row.ix));
                                    format!("Controls.{}.{}", rn, name)
                                }
                            };
                            let v = cell
                                .f
                                .as_deref()
                                .filter(|f| *f != "Inh" && *f != "No Formula")
                                .and_then(|f| vsdx::formula::eval(f, &menv).ok())
                                .and_then(|v| v.as_f64())
                                .filter(|v| v.is_finite())
                                .or_else(|| cell.numeric());
                            if let Some(v) = v {
                                menv.insert(key, v);
                            }
                        }
                    }
                }
            }
            walk.extend(
                src.sheet
                    .sections_named("Geometry")
                    .into_iter()
                    .map(|g| (g, scale)),
            );
            frame_env = menv;
        }
    }
    for (walk_ix, (geo, gscale)) in walk.into_iter().enumerate() {
        // Shared: writebacks persist for later sections of the frame.
        let genv = &mut frame_env;
        let gname = format!("Geometry{}", walk_ix + 1);
        let mut cur = (0.0f64, 0.0f64);
        let mut first_pt: Option<(f64, f64)> = None;
        fn row_num_in(
            env: &vsdx::formula::Env,
            row: &vsdx::Row,
            n: &str,
        ) -> f64 {
            row.cell(n)
                .and_then(|c| {
                    c.f.as_deref()
                        .and_then(|f| vsdx::formula::eval(f, env).ok())
                        .and_then(|v| v.as_f64())
                        .filter(|v| v.is_finite())
                        .or(c.numeric())
                })
                .unwrap_or(0.0)
        }
        for row in &geo.rows {
            let Some(t) = row.t.as_deref() else { continue };
            let rel = t.starts_with("Rel");
            let (mut x, mut y) = (
                row_num_in(genv, row, "X"),
                row_num_in(genv, row, "Y"),
            );
            if rel {
                // Raw signed dims — rel_adjust (which the renderer
                // uses) multiplies raw, so the bound must match.
                x *= width;
                y *= height;
            }
            // Feed the section-local coordinates back for cross-row
            // references (`Geometry1.X2`, #249) — before the scale,
            // which maps onto the page but not into the next row's
            // frame.
            genv.insert(format!("{}.X{}", gname, row.ix), x);
            genv.insert(format!("{}.Y{}", gname, row.ix), y);
            // Master-frame rows scale into the instance like the
            // renderer's emit (every point, literals and evaluated
            // formulas alike).
            x *= gscale.0;
            y *= gscale.1;
            // The affine maps local y-DOWN ([0..w]×[-h..0]) like the
            // box union above; the row coordinates are y-UP.
            let pt = |x: f64, y: f64, u: &mut Focus| {
                let (px, py) = a.apply(x, -y);
                u.union((px, py, 0.0, 0.0));
            };
            pt(x, y, &mut u);
            match t {
                "ArcTo" | "RelArcTo" => {
                    // The apex: chord mid + A·right-of-travel.
                    let sag = row_num_in(genv, row, "A");
                    let (dx, dy) = (x - cur.0, y - cur.1);
                    let len = dx.hypot(dy).max(1e-9);
                    let mid = ((cur.0 + x) / 2.0, (cur.1 + y) / 2.0);
                    pt(mid.0 + dy / len * sag, mid.1 - dx / len * sag, &mut u);
                }
                "EllipticalArcTo" | "RelEllipticalArcTo" => {
                    let (mut mx, mut my) = (row_num_in(genv, row, "A"), row_num_in(genv, row, "B"));
                    if rel {
                        // Raw signed dims — rel_adjust multiplies
                        // raw (negative widths mirror the bound).
                        mx *= width;
                        my *= height;
                    }
                    mx *= gscale.0;
                    my *= gscale.1;
                    pt(mx, my, &mut u);
                    // The arc's apex beyond the on-curve point.
                    pt(
                        2.0 * mx - (cur.0 + x) / 2.0,
                        2.0 * my - (cur.1 + y) / 2.0,
                        &mut u,
                    );
                }
                "CubicBezTo" | "RelCubBezTo" => {
                    // (A,B) and (C,D): two control points.
                    let (mut a, mut b) = (row_num_in(genv, row, "A"), row_num_in(genv, row, "B"));
                    let (mut c, mut d) = (row_num_in(genv, row, "C"), row_num_in(genv, row, "D"));
                    if rel {
                        a *= width.max(1e-6);
                        b *= height.max(1e-6);
                        c *= width.max(1e-6);
                        d *= height.max(1e-6);
                    }
                    pt(a * gscale.0, b * gscale.1, &mut u);
                    pt(c * gscale.0, d * gscale.1, &mut u);
                }
                "QuadBezTo" | "RelQuadBezTo" => {
                    let (mut a, mut b) = (row_num_in(genv, row, "A"), row_num_in(genv, row, "B"));
                    if rel {
                        a *= width.max(1e-6);
                        b *= height.max(1e-6);
                    }
                    pt(a * gscale.0, b * gscale.1, &mut u);
                }
                "NURBSTo" => {
                    // #188: the interior control points live in the
                    // E formula's (x, y, knot, weight) groups.
                    let e_text = row
                        .cell("E")
                        .and_then(|c| c.v.clone().or_else(|| c.f.clone()))
                        .unwrap_or_default();
                    if let Some(f) = crate::curve::parse_nurbs(&e_text) {
                        // Per-axis: a fraction lands directly in
                        // instance units (the frame's own Width/
                        // Height); an absolute axis carries
                        // master-unit literals and takes the scale
                        // once — mirroring the render arm (#218
                        // review round: no squared stretch).
                        let fx = if f.x_type == 0.0 { width } else { gscale.0 };
                        let fy = if f.y_type == 0.0 { height } else { gscale.1 };
                        for (gx, gy, _, _) in &f.points {
                            pt(gx * fx, gy * fy, &mut u);
                        }
                    }
                }
                _ => {}
            }
            cur = (x, y);
            if first_pt.is_none() {
                first_pt = Some((x, y));
            }
        }
        // #185: arrowheads draw at physical size beyond the row-point
        // hull — a size-5/6 head on a horizontal 1D connector reaches
        // ±0.125/±0.25 in sideways against the 0.08 in bleed pad, so
        // union a conservative square (the gallery's largest reach) at
        // each end that carries a head. Same gate as the renderer's.
        let arrowed = |arrow: f64| arrow >= 0.5 && !(46.0..=50.5).contains(&arrow);
        if get("BeginArrow", &text_env).is_some_and(arrowed) || get("EndArrow", &text_env).is_some_and(arrowed) {
            let weight = get("LineWeight", &text_env).unwrap_or(0.75 / 72.0);
            let pad = |pt: (f64, f64), arrow: Option<f64>, sz: f64, u: &mut Focus| {
                if !arrow.is_some_and(arrowed) {
                    return;
                }
                let r = arrow_head_reach(weight, sz);
                // LOCAL y-down corners — box_union applies the affine
                // itself; pre-applying it here doubled the transform
                // and threw the pad square across the page (#218
                // review).
                let (lx, ly) = (pt.0, -pt.1);
                box_union(lx - r, ly - r, lx + r, ly + r, &a, u);
            };
            if let Some((x, y)) = first_pt {
                pad(
                    (x, y),
                    get("BeginArrow", &text_env),
                    get("BeginArrowSize", &text_env).unwrap_or(2.0),
                    &mut u,
                );
            }

            pad(
                cur,
                get("EndArrow", &text_env),
                get("EndArrowSize", &text_env).unwrap_or(2.0),
                &mut u,
            );
        }
    }
    // Text can overflow the shape box (TxtWidth/TxtHeight are
    // independent cells); bound it with emit_text's placement math.
    let has_text = s
        .sheet
        .text
        .as_deref()
        .map(str::trim)
        .is_some_and(|t| !t.is_empty());
    if has_text {
        let tw = get("TxtWidth", &text_env).unwrap_or(width).max(0.02);
        let th = get("TxtHeight", &text_env).unwrap_or(height).max(0.02);
        let (tlx, tly) = (
            get("TxtLocPinX", &text_env).unwrap_or(tw / 2.0),
            get("TxtLocPinY", &text_env).unwrap_or(th / 2.0),
        );
        let (tx, ty) = (
            get("TxtPinX", &text_env).unwrap_or(width / 2.0),
            get("TxtPinY", &text_env).unwrap_or(height / 2.0),
        );
        if get("TxtAngle", &text_env).unwrap_or(0.0) != 0.0 {
            // Rotated text: bound by the circumscribed square.
            let side = tw.max(th);
            let (cx, cy) = (tx - tlx + tw / 2.0, -(ty - tly + th / 2.0));
            box_union(
                cx - side / 2.0,
                cy - side / 2.0,
                cx + side / 2.0,
                cy + side / 2.0,
                &a,
                &mut u,
            );
        } else {
            let (x0, y0) = (tx - tlx, -(ty - tly + th));
            box_union(x0, y0, x0 + tw, y0 + th, &a, &mut u);
        }
    }
    (u.acc.unwrap_or((0.0, 0.0, 0.0, 0.0)), a)
}

/// The page-space ink box of every shape in `shape_id`'s subtree —
/// each entry unions the node's own box with all its descendants'
/// ([`node_ink_box`], one shared walk). This is the #236 patch cull's
/// predicate source: a node whose SUBTREE box misses the padded
/// damage rect paints nothing inside it and is omitted from the patch
/// serialization. The top-level root's own entry covers its whole
/// subtree.
pub fn subtree_ink_boxes(
    doc: &Document,
    page: &vsdx::Page,
    shape_id: i64,
) -> Option<HashMap<i64, (f64, f64, f64, f64)>> {
    let page_h = num(page.sheet().and_then(|s| s.cell("PageHeight"))).unwrap_or(11.0);
    let root = page.shapes.iter().find(|s| s.id == shape_id)?;
    let mut map: HashMap<i64, (f64, f64, f64, f64)> = HashMap::new();
    fn walk(
        doc: &Document,
        s: &Shape,
        chain: &[&Shape],
        page_h: f64,
        top: bool,
        to_page: Affine,
        map: &mut HashMap<i64, (f64, f64, f64, f64)>,
    ) -> (f64, f64, f64, f64) {
        let mut chain: Vec<&Shape> = chain.to_vec();
        chain.push(s);
        let (own, a) = node_ink_box(doc, s, &chain, page_h, top, to_page);
        let mut acc = own;
        for sub in &s.sub_shapes {
            let sub_box = walk(doc, sub, &chain, page_h, false, a, map);
            acc = union_box(acc, sub_box);
        }
        map.insert(s.id, acc);
        acc
    }
    walk(doc, root, &[], page_h, true, Affine::IDENTITY, &mut map);
    Some(map)
}

/// Union two (x, y, w, h) page rects — the bounds-walk's accumulator
/// as a plain function (the #236 subtree-box pass unions bottom-up).
fn union_box(a: (f64, f64, f64, f64), b: (f64, f64, f64, f64)) -> (f64, f64, f64, f64) {
    let x1 = (a.0 + a.2).max(b.0 + b.2);
    let y1 = (a.1 + a.3).max(b.1 + b.3);
    let nx = a.0.min(b.0);
    let ny = a.1.min(b.1);
    (nx, ny, x1 - nx, y1 - ny)
}

/// One top-level shape's subtree as a standalone layer SVG (#93): the
/// same absolute page coordinates and the same `render_shape` walk as
/// [`render_page_with`] — pixel-identical ink — but the viewBox is the
/// subtree's [`shape_ink_bounds`] and no page chrome/background is
/// emitted (the canvas composites its own paper under the layer
/// quads). The canvas, not the export path, is the consumer.
pub fn render_shape_layer(
    doc: &Document,
    page: &vsdx::Page,
    shape_id: i64,
    hidden: &[i64],
    bounds: (f64, f64, f64, f64),
) -> anyhow::Result<String> {
    render_shape_layer_opts(doc, page, shape_id, hidden, bounds, None, None).map(|(_, svg)| svg)
}

/// [`render_shape_layer`] with the canvas-only options (GitHub #22,
/// #124): `skip_text_shape` suppresses one shape's text while the
/// GUI's in-place editor is open on it, and `greek_ppi` turns on text
/// greeking for layers rasterized at `ppi` — text blocks whose em
/// height would fall under [`TEXT_GREEK_PX`] device pixels emit gray
/// bars instead of glyphs. Returns the SVG plus every text size the
/// walk met (inches), the cache's greek profile for deciding whether
/// a ppi-bucket change re-renders or re-serializes.
pub fn render_shape_layer_opts(
    doc: &Document,
    page: &vsdx::Page,
    shape_id: i64,
    hidden: &[i64],
    bounds: (f64, f64, f64, f64),
    skip_text: Option<i64>,
    greek_ppi: Option<f32>,
) -> anyhow::Result<(Vec<f64>, String)> {
    render_shape_layer_inner(
        doc, page, shape_id, hidden, bounds, skip_text, greek_ppi, None,
    )
}

/// [`render_shape_layer_opts`] for the #236 damage-region patch: only
/// the subtree nodes whose ink box intersects `cull_rect` (page
/// inches) are serialized. Omitted nodes lie entirely outside the
/// patch rect (the caller pads it past stroke bleed and anti-aliasing
/// reach), so pixels inside the patch are unchanged — the
/// `layers::culled_patch_pixels_match_the_full_layer_render` pin
/// holds the patch-vs-full byte comparison for the culled tree.
/// `bounds` stays the FULL layer's viewBox: the patch renders through
/// the same grid as the existing texture.
#[allow(clippy::too_many_arguments)] // the layer's full identity + the cull rect (#236)
pub fn render_shape_layer_opts_culled(
    doc: &Document,
    page: &vsdx::Page,
    shape_id: i64,
    hidden: &[i64],
    bounds: (f64, f64, f64, f64),
    skip_text: Option<i64>,
    greek_ppi: Option<f32>,
    cull_rect: (f64, f64, f64, f64),
) -> anyhow::Result<(Vec<f64>, String)> {
    let boxes = subtree_ink_boxes(doc, page, shape_id)
        .ok_or_else(|| anyhow::anyhow!("shape {shape_id} not found on page {}", page.id))?;
    let cull = InkCull {
        rect: cull_rect,
        boxes: &boxes,
    };
    render_shape_layer_inner(
        doc,
        page,
        shape_id,
        hidden,
        bounds,
        skip_text,
        greek_ppi,
        Some(&cull),
    )
}

/// The #236 cull predicate: a shape id → its subtree ink box, plus the
/// padded damage rect. `skips` is the render walk's per-node test — a
/// node whose SUBTREE box misses the rect paints nothing inside it
/// (the full render's own viewBox clips anything beyond the box), so
/// omitting it changes no pixel the patch will show.
struct InkCull<'a> {
    rect: (f64, f64, f64, f64),
    boxes: &'a HashMap<i64, (f64, f64, f64, f64)>,
}

impl InkCull<'_> {
    fn skips(&self, id: i64) -> bool {
        // A node missing from the map cannot happen (the pass walks
        // the same subtree); keep it if it does — conservative.
        self.boxes
            .get(&id)
            .is_some_and(|b| !boxes_intersect(*b, self.rect))
    }
}

/// Two (x, y, w, h) page rects intersect (closed test — a shared edge
/// counts, so the caller's padding is what creates distance).
fn boxes_intersect(a: (f64, f64, f64, f64), b: (f64, f64, f64, f64)) -> bool {
    a.0 <= b.0 + b.2 && b.0 <= a.0 + a.2 && a.1 <= b.1 + b.3 && b.1 <= a.1 + a.3
}

#[allow(clippy::too_many_arguments)] // shared body of the plain and culled serializers
fn render_shape_layer_inner(
    doc: &Document,
    page: &vsdx::Page,
    shape_id: i64,
    hidden: &[i64],
    bounds: (f64, f64, f64, f64),
    skip_text: Option<i64>,
    greek_ppi: Option<f32>,
    cull: Option<&InkCull>,
) -> anyhow::Result<(Vec<f64>, String)> {
    let page_h = num(page.sheet().and_then(|s| s.cell("PageHeight"))).unwrap_or(11.0);
    let Some(shape) = page.shapes.iter().find(|s| s.id == shape_id) else {
        anyhow::bail!("shape {shape_id} not found on page {}", page.id);
    };
    let hidden_member = |s: &Shape| -> bool {
        s.sheet
            .cell("LayerMember")
            .and_then(|c| c.numeric())
            .is_some_and(|m| hidden.contains(&(m as i64)))
    };
    let greek = greek_ppi.map(GreekCtx::new);
    let mut svg = String::new();
    let (bx, by, bw, bh) = bounds;
    let _ = writeln!(
        svg,
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="{bx:.5} {by:.5} {bw:.5} {bh:.5}" width="{pw:.0}" height="{ph:.0}">"#,
        pw = bw * PX_PER_IN,
        ph = bh * PX_PER_IN,
    );
    if !hidden_member(shape) {
        render_shape(
            doc,
            shape,
            page_h,
            &[],
            &page.part,
            skip_text,
            greek.as_ref(),
            cull,
            &mut svg,
        );
    }
    let _ = writeln!(svg, "</svg>");
    let sizes = greek.map(|g| g.sizes.into_inner()).unwrap_or_default();
    Ok((sizes, svg))
}

/// [`render_page_with`] restricted to one top-level shape's subtree —
/// the GUI's group-edit mode (GitHub #78): every other top-level shape
/// is skipped; the group renders with its members at their page
/// positions through the normal group transforms.
pub fn render_page_scoped(
    doc: &Document,
    page: &vsdx::Page,
    hidden: &[i64],
    only: i64,
) -> anyhow::Result<String> {
    render_page_opts(doc, page, hidden, Some(only), None)
}

fn render_page_opts(
    doc: &Document,
    page: &vsdx::Page,
    hidden: &[i64],
    only: Option<i64>,
    skip_text: Option<i64>,
) -> anyhow::Result<String> {
    render_page_full(doc, page, hidden, only, skip_text, true, true)
}

/// [`render_page_opts`] with the page chrome gated: `border` drops the
/// gray sheet frame, `background` drops the white page fill — the
/// file exports (#35) produce the ink only (Visio's own SVG/PDF/PNG
/// exports carry no sheet chrome), while the canvas and `dc render`
/// keep the full picture.
fn render_page_full(
    doc: &Document,
    page: &vsdx::Page,
    hidden: &[i64],
    only: Option<i64>,
    skip_text: Option<i64>,
    border: bool,
    background: bool,
) -> anyhow::Result<String> {
    let w = num(page.sheet().and_then(|s| s.cell("PageWidth"))).unwrap_or(8.5);
    let h = num(page.sheet().and_then(|s| s.cell("PageHeight"))).unwrap_or(11.0);

    let mut svg = String::new();
    let _ = writeln!(
        svg,
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w} {h}" width="{pw:.0}" height="{ph:.0}">"#,
        pw = w * PX_PER_IN,
        ph = h * PX_PER_IN,
    );
    let _ = writeln!(
        svg,
        r#"<!-- Tarahi (cleanroom VSDX renderer); page '{}' -->"#,
        escape_attr(&page.name)
    );
    if border {
        let _ = writeln!(
            svg,
            r##"<rect x="0" y="0" width="{w}" height="{h}" fill="none" stroke="#c8c8c8" stroke-width="0.01"/>"##
        );
    }
    // White background under the sheet (Visio page is white); an
    // export with `--transparent` omits it so the alpha channel shows.
    if background {
        let _ = writeln!(
            svg,
            r##"<rect x="0" y="0" width="{w}" height="{h}" fill="#ffffff"/>"##,
        );
    }
    let hidden_member = |s: &Shape| -> bool {
        s.sheet
            .cell("LayerMember")
            .and_then(|c| c.numeric())
            .is_some_and(|m| hidden.contains(&(m as i64)))
    };
    for shape in &page.shapes {
        if hidden_member(shape) {
            continue;
        }
        // Guides are page furniture, never ink (#113): Visio excludes
        // them from print and export, and the canvas draws them as its
        // own overlay.
        if shape.type_.as_deref() == Some("Guide") {
            continue;
        }
        if let Some(only) = only {
            if shape.id != only {
                continue;
            }
        }
        render_shape(
            doc,
            shape,
            h,
            &[],
            &page.part,
            skip_text,
            None,
            None,
            &mut svg,
        );
    }
    // #187: line jumps — Visio prints hops. The export appends the
    // same treatment the canvas paints (white gap, under redraw, hop
    // arc) as a page-space group above the ink. The gap/extension
    // paddings pin the canvas's screen-px extras at 96 dpi. Scoped
    // renders (`only`) draw one shape's subtree — page-wide jumps
    // would drag in crossings from connectors outside the scope.
    // The page's Show-Line-Jumps switch gates the overlay (#251).
    let jumps = if only.is_none() && page.line_jumps_shown() {
        crate::geom::page_jumps(doc, page, hidden)
    } else {
        Vec::new()
    };
    if !jumps.is_empty() {
        let factor = page.line_jump_factor();
        let _ = writeln!(svg, r#"<g data-dc="line-jumps">"#);
        for j in &jumps {
            append_jump_scaled(&mut svg, h, j, factor);
        }
        let _ = writeln!(svg, "</g>");
    }
    let _ = writeln!(svg, "</svg>");
    Ok(svg)
}

/// The page-space paddings of the canvas overlay's screen-px extras
/// (2 px gap weight, 1 px under extension), pinned at 96 dpi.
const JUMP_GAP_EXTRA_IN: f64 = 2.0 / 96.0;
const JUMP_UNDER_EXT_IN: f64 = 1.0 / 96.0;

/// One hop in page-space SVG (y-down: `sy = page_h − y`), the canvas
/// overlay's three strokes — the white gap across the jumper, the
/// under line redrawn straight through it, and the hop arc bulging
/// page-up (`canvas.rs`'s screen-up flip, computed here in page
/// space so rotated jumpers stay correct).
#[allow(dead_code)] // the factor-1 form; tests call the scaled one
fn append_jump(svg: &mut String, page_h: f64, j: &crate::geom::Jump) {
    append_jump_scaled(svg, page_h, j, 1.0);
}

/// [`append_jump`] with the page's jump-size multiplier (#251 — the
/// PageSheet's `LineJumpFactorX`).
fn append_jump_scaled(
    svg: &mut String,
    page_h: f64,
    j: &crate::geom::Jump,
    factor: f64,
) {
    let (r, w, uw) = (
        crate::geom::JUMP_RADIUS_IN * factor,
        j.weight,
        j.under_weight,
    );
    let sy = |y: f64| page_h - y;
    let pt = |p: (f64, f64)| format!("{:.5} {:.5}", p.0, sy(p.1));
    let hex = |c: (u8, u8, u8)| format!("#{:02x}{:02x}{:02x}", c.0, c.1, c.2);
    // 1. the gap — paper-white, slightly wider than the jumper.
    let g1 = (j.at.0 - j.dir.0 * r, j.at.1 - j.dir.1 * r);
    let g2 = (j.at.0 + j.dir.0 * r, j.at.1 + j.dir.1 * r);
    let white = "#ffffff";
    let _ = writeln!(
        svg,
        r#"<line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" stroke="{white}" stroke-width="{gw:.4}"/>"#,
        x1 = g1.0,
        y1 = sy(g1.1),
        x2 = g2.0,
        y2 = sy(g2.1),
        gw = w + JUMP_GAP_EXTRA_IN,
    );
    // 2. the under line passes straight through the gap.
    let ext = r + uw + JUMP_UNDER_EXT_IN;
    let u1 = (j.at.0 - j.under.0 * ext, j.at.1 - j.under.1 * ext);
    let u2 = (j.at.0 + j.under.0 * ext, j.at.1 + j.under.1 * ext);
    let _ = writeln!(
        svg,
        r#"<line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" stroke="{c}" stroke-width="{uw:.4}"/>"#,
        x1 = u1.0,
        y1 = sy(u1.1),
        x2 = u2.0,
        y2 = sy(u2.1),
        c = hex(j.under_color),
        uw = uw,
    );
    // 3. the hop arc — a 9-point semicircle from g1 around to g2
    // through at + n·r, the normal flipped toward page-up.
    let mut n = (-j.dir.1, j.dir.0);
    if n.1 < 0.0 {
        n = (-n.0, -n.1);
    }
    let mut path = String::new();
    for k in 0..=8 {
        let t = k as f64 / 8.0;
        let ang = std::f64::consts::PI * t;
        let local = (
            -j.dir.0 * r * ang.cos() + n.0 * r * ang.sin(),
            -j.dir.1 * r * ang.cos() + n.1 * r * ang.sin(),
        );
        let p = (j.at.0 + local.0, j.at.1 + local.1);
        path.push_str(if k == 0 { "M " } else { "L " });
        path.push_str(&pt(p));
        path.push(' ');
    }
    let _ = writeln!(
        svg,
        r#"<path d="{path}" fill="none" stroke="{c}" stroke-width="{w:.4}"/>"#,
        c = hex(j.color),
        w = w,
    );
}

/// An SVG-order affine: `x' = a·x + c·y + e`, `y' = b·x + d·y + f`
/// (y-down SVG space; the composition order matches the `transform=`
/// strings this module emits, so geometry derived here lands on the
/// same pixels the renderer draws).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Affine {
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub d: f64,
    pub e: f64,
    pub f: f64,
}

impl Affine {
    pub const IDENTITY: Affine = Affine {
        a: 1.0,
        b: 0.0,
        c: 0.0,
        d: 1.0,
        e: 0.0,
        f: 0.0,
    };

    fn translate(tx: f64, ty: f64) -> Affine {
        Affine {
            e: tx,
            f: ty,
            ..Affine::IDENTITY
        }
    }

    /// SVG rotate(deg): clockwise on screen (y grows down).
    fn rotate_deg(deg: f64) -> Affine {
        let (s, c) = deg.to_radians().sin_cos();
        Affine {
            a: c,
            b: s,
            c: -s,
            d: c,
            ..Affine::IDENTITY
        }
    }

    fn scale(sx: f64, sy: f64) -> Affine {
        Affine {
            a: sx,
            d: sy,
            ..Affine::IDENTITY
        }
    }

    /// Matrix product `self · next` — `next` applies to points first.
    /// This is the reading order of an SVG `transform="A B C"` list
    /// (the leftmost matrix is the outermost).
    fn then(&self, next: &Affine) -> Affine {
        Affine {
            a: self.a * next.a + self.c * next.b,
            b: self.b * next.a + self.d * next.b,
            c: self.a * next.c + self.c * next.d,
            d: self.b * next.c + self.d * next.d,
            e: self.a * next.e + self.c * next.f + self.e,
            f: self.b * next.e + self.d * next.f + self.f,
        }
    }

    pub fn apply(&self, x: f64, y: f64) -> (f64, f64) {
        (
            self.a * x + self.c * y + self.e,
            self.b * x + self.d * y + self.f,
        )
    }

    /// Inverse (undefined on singular matrices — callers use XForm
    /// chains, which carry non-zero scales).
    pub fn invert(&self) -> Affine {
        let det = self.a * self.d - self.b * self.c;
        let (a, b, c, d) = (self.d / det, -self.b / det, -self.c / det, self.a / det);
        Affine {
            a,
            b,
            c,
            d,
            e: -(a * self.e + c * self.f),
            f: -(b * self.e + d * self.f),
        }
    }
}

/// The SVG-space affine the renderer composes for one level of the
/// `chain` (root-first, from `render_shape`/`render_shape_local`): a
/// top-level shape maps its local frame into page SVG space
/// (y-down, origin top-left); a nested shape maps into its parent's
/// local SVG frame with no flip scale of its own.
fn level_affine(doc: &Document, chain: &[&Shape], idx: usize, page_h: f64) -> Affine {
    let s = chain[idx];
    let env = shape_env(doc, s, &chain[..=idx]);
    let get = |n: &str| -> Option<f64> {
        doc.resolve_cell(s, &chain[..=idx], n)
            .and_then(|c| cell_num(&c, &env))
    };
    let width = get("Width").unwrap_or(1.0);
    let height = get("Height").unwrap_or(1.0);
    let (px, py) = (get("PinX").unwrap_or(0.0), get("PinY").unwrap_or(0.0));
    let (lx, ly) = (
        get("LocPinX").unwrap_or(width / 2.0),
        get("LocPinY").unwrap_or(height / 2.0),
    );
    let deg = -get("Angle").unwrap_or(0.0).to_degrees();
    let t = Affine::translate(px, if idx == 0 { page_h - py } else { -py })
        .then(&Affine::rotate_deg(deg));
    // Flip scale exists only at the page root (group-local rendering
    // applies no flip of its own).
    let t = if idx == 0 {
        let fx = if get("FlipX").unwrap_or(0.0) != 0.0 {
            -1.0
        } else {
            1.0
        };
        let fy = if get("FlipY").unwrap_or(0.0) != 0.0 {
            -1.0
        } else {
            1.0
        };
        t.then(&Affine::scale(fx, fy))
    } else {
        t
    };
    t.then(&Affine::translate(-lx, ly))
}

/// The composed SVG-space affine mapping the LAST chain entry's
/// PARENT-LOCAL frame to page SVG space: `T(root) · … · T(parent)`,
/// the same nesting the renderer emits. An empty/root chain yields
/// identity (a top-level shape's parent frame IS the page).
fn ancestors_affine(doc: &Document, chain: &[&Shape], page_h: f64) -> Affine {
    let mut m = Affine::IDENTITY;
    for idx in 0..chain.len().saturating_sub(1) {
        m = m.then(&level_affine(doc, chain, idx, page_h));
    }
    m
}

/// A shape's bounding box in PAGE coordinates (inches, y-up, origin
/// bottom-left): the shape's parent-local rect mapped through the
/// ancestor XForm chain — the same math the renderer composes, so
/// hit-testing and selection frames land on the drawn pixels.
/// `chain` is root-first and ends with the shape itself.
pub fn shape_page_rect(
    doc: &Document,
    chain: &[&Shape],
    page_h: f64,
) -> Option<(f64, f64, f64, f64)> {
    let s = *chain.last()?;
    let env = shape_env(doc, s, chain);
    let get = |n: &str| -> Option<f64> {
        doc.resolve_cell(s, chain, n)
            .and_then(|c| cell_num(&c, &env))
    };
    let (pin_x, pin_y) = (get("PinX")?, get("PinY")?);
    let w = get("Width").unwrap_or(1.0);
    let h = get("Height").unwrap_or(1.0);
    let lx = get("LocPinX").unwrap_or(w / 2.0);
    let ly = get("LocPinY").unwrap_or(h / 2.0);
    let m = ancestors_affine(doc, chain, page_h);
    // Parent-local y-up → SVG (y-down) → page y-up. With at least one
    // ancestor group the parent frame is a group-local y-down frame
    // (a y-up value enters it as -y); with none the parent frame IS
    // the page, whose own SVG conversion applies on the way in.
    let nested = chain.len() >= 2;
    let corner = |x: f64, y: f64| -> (f64, f64) {
        let iy = if nested { -y } else { page_h - y };
        let (sx, sy) = m.apply(x, iy);
        (sx, page_h - sy)
    };
    let pts = [
        corner(pin_x - lx, pin_y - ly),
        corner(pin_x - lx + w, pin_y - ly),
        corner(pin_x - lx, pin_y - ly + h),
        corner(pin_x - lx + w, pin_y - ly + h),
    ];
    let xs = pts.iter().map(|p| p.0);
    let ys = pts.iter().map(|p| p.1);
    let (min_x, max_x) = (
        xs.clone().fold(f64::INFINITY, f64::min),
        xs.fold(f64::NEG_INFINITY, f64::max),
    );
    let (min_y, max_y) = (
        ys.clone().fold(f64::INFINITY, f64::min),
        ys.fold(f64::NEG_INFINITY, f64::max),
    );
    Some((min_x, min_y, max_x - min_x, max_y - min_y))
}

/// Map a PAGE point (inches, y-up) into the last chain entry's
/// PARENT-LOCAL y-up frame — the inverse of [`shape_page_rect`]'s
/// mapping, used to write PinX/PinY of a nested shape during drags
/// (a member's Pin cells are parent-local by definition).
pub fn page_point_to_parent_local(
    doc: &Document,
    chain: &[&Shape],
    page_h: f64,
    p: (f64, f64),
) -> (f64, f64) {
    let inv = ancestors_affine(doc, chain, page_h).invert();
    let (x, y) = inv.apply(p.0, page_h - p.1);
    if chain.len() >= 2 {
        (x, -y)
    } else {
        (x, page_h - y)
    }
}

/// Map a point in the last chain entry's PARENT-LOCAL y-up frame to
/// PAGE inches y-up — the forward direction of
/// [`page_point_to_parent_local`], used to read a nested member's pin
/// into page space during drags.
pub fn parent_local_point_to_page(
    doc: &Document,
    chain: &[&Shape],
    page_h: f64,
    p: (f64, f64),
) -> (f64, f64) {
    let m = ancestors_affine(doc, chain, page_h);
    let iy = if chain.len() >= 2 { -p.1 } else { page_h - p.1 };
    let (sx, sy) = m.apply(p.0, iy);
    (sx, page_h - sy)
}

/// Render a shape in absolute page coordinates: the XForm transform
/// wraps geometry, master mirroring, sub-shape recursion, and the
/// text block.
#[allow(clippy::too_many_arguments)] // the walk carries its whole environment
fn render_shape<'a>(
    doc: &'a Document,
    s: &'a Shape,
    page_h: f64,
    chain: &[&'a Shape],
    part: &str,
    skip_text: Option<i64>,
    greek: Option<&GreekCtx>,
    cull: Option<&InkCull>,
    out: &mut String,
) {
    let mut chain: Vec<&'a Shape> = chain.to_vec();
    chain.push(s);
    let env = shape_env(doc, s, &chain);
    let get = |n: &str| -> Option<f64> {
        doc.resolve_cell(s, &chain, n)
            .and_then(|c| cell_num(&c, &env))
    };
    let (pin_x, pin_y) = (get("PinX").unwrap_or(0.0), get("PinY").unwrap_or(0.0));
    let width = get("Width").unwrap_or(1.0);
    let height = get("Height").unwrap_or(1.0);
    let (loc_x, loc_y) = (
        get("LocPinX").unwrap_or(width / 2.0),
        get("LocPinY").unwrap_or(height / 2.0),
    );
    let angle = get("Angle").unwrap_or(0.0);
    let flip_x = get("FlipX").unwrap_or(0.0) != 0.0;
    let flip_y = get("FlipY").unwrap_or(0.0) != 0.0;
    let (sx, sy) = (
        if flip_x { -1.0 } else { 1.0 },
        if flip_y { -1.0 } else { 1.0 },
    );

    let transform = format!(
        "translate({px:.6},{py:.6}) rotate({deg:.6}) scale({sx},{sy}) translate({lx:.6},{ly:.6})",
        px = pin_x,
        py = page_h - pin_y,
        deg = -angle.to_degrees(),
        lx = -loc_x,
        ly = loc_y,
    );

    let _ = writeln!(out, r#"<g transform="{t}">"#, t = escape_attr(&transform));

    // A multi-root master (several shapes on the master canvas, e.g. a
    // 13-shape header cluster) is drawn entirely by the mirroring below;
    // rendering the instance's own/effective geometry too would double
    // the first root.
    let multi_root = multi_root_master(doc, s);
    if !multi_root {
        render_all_geometry(doc, s, &chain, width, height, out);
        // Embedded image (ForeignData → media part via this part's rels).
        if let Some(uri) = foreign_image_uri(doc, part, s) {
            let _ = writeln!(
                out,
                r#"<image x="0" y="{y:.5}" width="{w:.5}" height="{h:.5}" href="{u}" preserveAspectRatio="none"/>"#,
                y = -height,
                w = width,
                h = height,
                u = escape_attr(&uri),
            );
        }
    }
    // Sub-shapes of a group live in the parent's local frame.
    for sub in &s.sub_shapes {
        // #236: a sub whose subtree box misses the cull rect paints
        // nothing inside the patch — skip it and its whole subtree.
        if cull.is_some_and(|c| c.skips(sub.id)) {
            continue;
        }
        render_shape_local(doc, sub, &chain, part, skip_text, greek, cull, out);
    }
    mirror_master(doc, s, &chain, width, height, greek, out);

    emit_text(
        doc,
        s,
        &chain,
        width,
        height,
        sx,
        sy,
        skip_text == Some(s.id),
        greek,
        out,
    );
    let _ = writeln!(out, "</g>");
}

/// Render a shape whose XForm is relative to its parent's local frame
/// (used for sub-shapes of groups).
#[allow(clippy::too_many_arguments)] // the render walk's context rides one call (#236 cull)
fn render_shape_local<'a>(
    doc: &'a Document,
    s: &'a Shape,
    chain: &[&'a Shape],
    part: &str,
    skip_text: Option<i64>,
    greek: Option<&GreekCtx>,
    cull: Option<&InkCull>,
    out: &mut String,
) {
    let mut chain: Vec<&'a Shape> = chain.to_vec();
    chain.push(s);
    let env = shape_env(doc, s, &chain);
    let get = |n: &str| -> Option<f64> {
        doc.resolve_cell(s, &chain, n)
            .and_then(|c| cell_num(&c, &env))
    };
    let width = get("Width").unwrap_or(1.0);
    let height = get("Height").unwrap_or(1.0);
    let transform = format!(
        "translate({px:.6},{py:.6}) rotate({deg:.6}) translate({lx:.6},{ly:.6})",
        px = get("PinX").unwrap_or(0.0),
        // group-local Y is already flipped by the parent group's scale
        py = -get("PinY").unwrap_or(0.0),
        deg = -get("Angle").unwrap_or(0.0).to_degrees(),
        lx = -get("LocPinX").unwrap_or(width / 2.0),
        ly = get("LocPinY").unwrap_or(height / 2.0),
    );
    let _ = writeln!(out, r#"<g transform="{}">"#, escape_attr(&transform));
    let multi_root = multi_root_master(doc, s);
    if !multi_root {
        render_all_geometry(doc, s, &chain, width, height, out);
        if let Some(uri) = foreign_image_uri(doc, part, s) {
            let _ = writeln!(
                out,
                r#"<image x="0" y="{y:.5}" width="{w:.5}" height="{h:.5}" href="{u}" preserveAspectRatio="none"/>"#,
                y = -height,
                w = width,
                h = height,
                u = escape_attr(&uri),
            );
        }
    }
    for sub in &s.sub_shapes {
        // #236: the cull test, at every subtree level.
        if cull.is_some_and(|c| c.skips(sub.id)) {
            continue;
        }
        render_shape_local(doc, sub, &chain, part, skip_text, greek, cull, out);
    }
    mirror_master(doc, s, &chain, width, height, greek, out);
    // Group-local rendering applies no flip scale of its own, so the text
    // needs no flip compensation here.
    emit_text(
        doc,
        s,
        &chain,
        width,
        height,
        1.0,
        1.0,
        skip_text == Some(s.id),
        greek,
        out,
    );
    let _ = writeln!(out, "</g>");
}

/// True when `s` is an instance (no inline children) of a master whose
/// canvas carries several root shapes — the whole master drawing must be
/// mirrored into the instance rect.
fn multi_root_master(doc: &Document, s: &Shape) -> bool {
    s.sub_shapes.is_empty()
        && s.master
            .and_then(|id| doc.master(id))
            .map(|m| m.shapes.len() > 1)
            .unwrap_or(false)
}

/// Master mirroring for an instance with no inline children.
/// - multi-root master: every root shape renders, positioned by its own
///   master-canvas Xform, with the union bbox mapped into the instance
///   rect (Visio draws the master's whole canvas).
/// - single-root master: the instance/effective sheet already draws the
///   root's own shape; mirror the root's sub-shape tree. Each sub-shape's
///   XForm is re-evaluated against the *instance* group size (Visio
///   semantics: cells whose formulas reference the parent stretch with
///   it; `GUARD(4MM)`-style absolute cells keep their size — issue #15),
///   emitting a corrective transform per child. Cells we cannot
///   evaluate fall back to the uniform instance/master scale, so masters
///   without authored formulas keep their historical rendering.
fn mirror_master(
    doc: &Document,
    s: &Shape,
    chain: &[&Shape],
    width: f64,
    height: f64,
    greek: Option<&GreekCtx>,
    out: &mut String,
) {
    if !s.sub_shapes.is_empty() {
        return;
    }
    let Some(m) = s.master.and_then(|id| doc.master(id)) else {
        return;
    };
    let mpart = &m.part;
    if m.shapes.len() > 1 {
        // Union bbox (y-up master canvas) over all roots.
        let rect = |r: &Shape| {
            let c = |n: &str, d: f64| r.sheet.cell(n).and_then(|x| x.numeric()).unwrap_or(d);
            let w = c("Width", 1.0).max(1e-6);
            let h = c("Height", 1.0).max(1e-6);
            let px = c("PinX", 0.0);
            let py = c("PinY", 0.0);
            let lx = c("LocPinX", w / 2.0);
            let ly = c("LocPinY", h / 2.0);
            (px - lx, py - ly, w, h)
        };
        let mut bbox: Option<(f64, f64, f64, f64)> = None;
        for r in &m.shapes {
            let (x0, y0, w, h) = rect(r);
            bbox = Some(match bbox {
                None => (x0, y0, w, h),
                Some((bx, by, bw, bh)) => {
                    let x1 = (bx + bw).max(x0 + w);
                    let y1 = (by + bh).max(y0 + h);
                    let nx = bx.min(x0);
                    let ny = by.min(y0);
                    (nx, ny, x1 - nx, y1 - ny)
                }
            });
        }
        let Some((bx, by, bw, bh)) = bbox else { return };
        let kx = width / bw.max(1e-6);
        let ky = height / bh.max(1e-6);
        // Root emitted coords are (x, -py) (local y-flip); this wrapper
        // maps the bbox to the instance rect (0..width, 0..height).
        let _ = writeln!(
            out,
            r#"<g transform="scale({kx:.6},{ky:.6}) translate({tx:.6},{ty:.6})">"#,
            tx = -bx,
            ty = -(by + bh),
        );
        for r in &m.shapes {
            // Master-canvas roots of a multi-root master: the union
            // bbox maps into the instance rect, which the node's own
            // ink box bounds — no cull of their own (#236).
            render_shape_local(doc, r, &[], mpart, None, greek, None, out);
        }
        let _ = writeln!(out, "</g>");
    } else if let Some(root) = m.shapes.first() {
        let mw = root
            .sheet
            .cell("Width")
            .and_then(|c| c.numeric())
            .filter(|v| v.abs() > 1e-9);
        let mh = root
            .sheet
            .cell("Height")
            .and_then(|c| c.numeric())
            .filter(|v| v.abs() > 1e-9);
        let (kx, ky) = (
            mw.map_or(1.0, |mw| width / mw),
            mh.map_or(1.0, |mh| height / mh),
        );
        let env = mirror_env(root, s, width, height);
        for sub in &root.sub_shapes {
            let wrapper = mirror_child_transform(sub, &env, (kx, ky));
            if let Some(t) = &wrapper {
                let _ = writeln!(out, r#"<g transform="{}">"#, escape_attr(t));
            }
            // Mirrored master children map into the instance's local
            // frame, bounded by the node's own ink box — no cull of
            // their own (#236).
            render_shape_local(doc, sub, chain, mpart, None, greek, None, out);
            if wrapper.is_some() {
                let _ = writeln!(out, "</g>");
            }
        }
    }
}

/// Evaluation environment for re-evaluating a master root's sub-shape
/// XForms at instance size (issue #15). Shared implementation in
/// `vsdx::model::instance_env` — the editor's cache refresh (dc-ui,
/// issue #59) computes from the same environment, so a refreshed cache
/// and a mirrored placement agree.
fn mirror_env(root: &Shape, instance: &Shape, width: f64, height: f64) -> vsdx::formula::Env {
    vsdx::model::instance_env(root, instance, width, height)
}

/// Corrective transform for one mirrored sub-shape: maps where
/// [`render_shape_local`] places it from its master XForm (`M`) to where
/// Visio places it at the instance's group size (`E`, from re-evaluated
/// formulas, falling back to uniform `k`·master per axis). Result:
/// `Some("identity-like scale")` / `Some(matrix)` / `None` when no
/// correction is needed.
fn mirror_child_transform(
    sub: &Shape,
    root_env: &vsdx::formula::Env,
    (kx, ky): (f64, f64),
) -> Option<String> {
    let num = |n: &str| sub.sheet.cell(n).and_then(|c| c.numeric());
    let m_w = num("Width").filter(|v| *v > 1e-9).unwrap_or(1.0);
    let m_h = num("Height").filter(|v| *v > 1e-9).unwrap_or(1.0);
    let m = |n: &str, d: f64| num(n).unwrap_or(d);
    let (m_px, m_py) = (m("PinX", 0.0), m("PinY", 0.0));
    let (m_lx, m_ly) = (m("LocPinX", m_w / 2.0), m("LocPinY", m_h / 2.0));
    let ang = m("Angle", 0.0);

    // Effective values: formula first (Visio re-evaluates against the
    // instance group), then the uniform-scale fallback.
    // Seed the env with only the root's QUALIFIED `Sheet.N!` spellings
    // (`vsdx::model::qualified_env`): a bare name in a child formula
    // means the child's own cell, so the root's unqualified
    // Width/Height/User.* must not leak in — without the filter a bare
    // reference silently reads the group's value and skips the per-axis
    // fallback below. (Child sheets are not in this env; bare
    // child-cell references fail to resolve and fall back.)
    let mut env: vsdx::formula::Env = vsdx::model::qualified_env(root_env);
    let eff = |name: &str, axis: f64, env: &vsdx::formula::Env| -> Option<f64> {
        let c = sub.sheet.cell(name)?;
        if let Some(f) = c.f.as_deref() {
            if f != "Inh" && f != "No Formula" {
                if let Some(v) = vsdx::formula::eval(f, env)
                    .ok()
                    .and_then(|v| v.as_f64())
                    .filter(|v| v.is_finite())
                {
                    return Some(v);
                }
            }
        }
        c.numeric().map(|v| v * axis)
    };
    let e_w = eff("Width", kx, &env).unwrap_or(kx * m_w);
    let e_h = eff("Height", ky, &env).unwrap_or(ky * m_h);
    // Pin/LocPin formulas may reference the shape's own effective size.
    env.insert("Width".into(), e_w);
    env.insert("Height".into(), e_h);
    let e_px = eff("PinX", kx, &env).unwrap_or(0.0);
    let e_py = eff("PinY", ky, &env).unwrap_or(0.0);
    let e_lx = eff("LocPinX", kx, &env).unwrap_or(e_w / 2.0);
    let e_ly = eff("LocPinY", ky, &env).unwrap_or(e_h / 2.0);

    // Matrices in SVG space (y-down), row-vector convention:
    // (a b c d e f) ≡ x' = a·x + c·y + e ; y' = b·x + d·y + f.
    let ident = Mat::identity();
    let rot = Mat::rotation(-ang);
    // Where render_shape_local places the shape from master values.
    let placed = Mat::translate(m_px, -m_py)
        .mul(&rot)
        .mul(&Mat::translate(-m_lx, m_ly));
    // Where Visio places it at instance size: same pin convention, the
    // effective LocPin, and the local frame scaled to the effective size.
    let wanted = Mat::translate(e_px, -e_py)
        .mul(&rot)
        .mul(&Mat::translate(-e_lx, e_ly))
        .mul(&Mat::scale(e_w / m_w, e_h / m_h));
    let w = wanted.mul(&placed.inv()?);
    const EPS: f64 = 1e-9;
    if w.near(&ident, EPS) {
        return None;
    }
    if w.0[1].abs() < EPS && w.0[2].abs() < EPS && w.0[4].abs() < EPS && w.0[5].abs() < EPS {
        return Some(format!("scale({:.6},{:.6})", w.0[0], w.0[3]));
    }
    Some(format!(
        "matrix({:.6},{:.6},{:.6},{:.6},{:.6},{:.6})",
        w.0[0], w.0[1], w.0[2], w.0[3], w.0[4], w.0[5]
    ))
}

/// 2D affine transform as SVG `matrix(a,b,c,d,e,f)`.
#[derive(Debug, Clone)]
struct Mat([f64; 6]);

impl Mat {
    fn identity() -> Mat {
        Mat([1.0, 0.0, 0.0, 1.0, 0.0, 0.0])
    }
    fn translate(tx: f64, ty: f64) -> Mat {
        Mat([1.0, 0.0, 0.0, 1.0, tx, ty])
    }
    fn scale(sx: f64, sy: f64) -> Mat {
        Mat([sx, 0.0, 0.0, sy, 0.0, 0.0])
    }
    fn rotation(rad: f64) -> Mat {
        let (s, c) = rad.sin_cos();
        Mat([c, s, -s, c, 0.0, 0.0])
    }
    /// self ∘ other (apply `other` first).
    fn mul(&self, other: &Mat) -> Mat {
        let a = &self.0;
        let b = &other.0;
        Mat([
            a[0] * b[0] + a[2] * b[1],
            a[1] * b[0] + a[3] * b[1],
            a[0] * b[2] + a[2] * b[3],
            a[1] * b[2] + a[3] * b[3],
            a[0] * b[4] + a[2] * b[5] + a[4],
            a[1] * b[4] + a[3] * b[5] + a[5],
        ])
    }
    fn inv(&self) -> Option<Mat> {
        let m = &self.0;
        let det = m[0] * m[3] - m[1] * m[2];
        if det.abs() < 1e-12 {
            return None;
        }
        let inv00 = m[3] / det;
        let inv01 = -m[1] / det;
        let inv10 = -m[2] / det;
        let inv11 = m[0] / det;
        Some(Mat([
            inv00,
            inv01,
            inv10,
            inv11,
            -(inv00 * m[4] + inv10 * m[5]),
            -(inv01 * m[4] + inv11 * m[5]),
        ]))
    }
    fn near(&self, other: &Mat, eps: f64) -> bool {
        self.0
            .iter()
            .zip(other.0.iter())
            .all(|(a, b)| (a - b).abs() <= eps)
    }
}

/// Text: laid out inside the TxtPin/TxtLocPin/TxtWidth/TxtHeight block —
/// word-wrapped to the block width, horizontally aligned per the Paragraph
/// row's HorizAlign, vertically per VertAlign, with the first Character
/// row's size/color/bold. Rotated by TxtAngle around the pin (Visio
/// vertical sidebars use TxtAngle 90).
///
/// `sx`/`sy` are the flip factors the caller's shape transform applied
/// (±1). Visio flips mirror geometry but never the text block: the block
/// keeps its TxtPin placement inside the flipped frame, yet the glyphs
/// must stay un-mirrored, and a TxtAngle rotation inside a mirrored frame
/// inverts its page-space sense — the connector masters author
/// `TxtAngle = IF(BITXOR(FlipX,FlipY),1,-1)*Angle` against exactly that,
/// so the page-space net is `Angle + sx*sy*TxtAngle` (0 = horizontal
/// labels, issue #19).
// The eight parameters are the full text-emission context; grouping
// them would just move the long call sites into a struct nobody else
// shares.
#[allow(clippy::too_many_arguments)]
/// A shape's resolved text block: placement in the shape's LOCAL frame
/// (inches, y-up) plus the styling the renderer paints it with — the
/// single source of truth shared by [`emit_text`] (the SVG backend),
/// the GUI's in-place editor overlay (GitHub #22), and tests, so the
/// editing view cannot drift from the rendered result.
///
/// Placement follows the sheet's `Txt*` cells with the same defaults
/// `emit_text` applies: pin at the shape's center, size = the shape's
/// Width/Height, LocPin centered in the block. Styling resolves
/// through the instance → master chain exactly like the renderer
/// (Character `Font`/`Size`/`Color`/`Style`, Paragraph `HorzAlign`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TextBlock {
    /// `TxtPinX`/`TxtPinY` — the block's pin in the shape's local frame.
    pub pin: (f64, f64),
    /// `TxtWidth`/`TxtHeight` (clamped to ≥ 0.02 in like `emit_text`).
    pub size: (f64, f64),
    /// `TxtLocPinX`/`TxtLocPinY` — the pin's offset from the block's
    /// lower-left corner.
    pub loc_pin: (f64, f64),
    /// `TxtAngle` (radians, local).
    pub angle: f64,
    /// Text-block insets (left, right, top, bottom) — Visio defaults
    /// 0.1 in / 0.1 in / 0.0555 in / 0.0555 in.
    pub insets: (f64, f64, f64, f64),
    /// Line height in inches (`LineSpacing` × font size, default 1.2).
    pub line_height: f64,
    /// Character `Font` (family name) if any cell resolves one.
    pub font: Option<String>,
    /// Character `Size` in inches (0.13889 in = 10 pt); default 10 pt.
    pub font_size: f64,
    /// Resolved Character `Color` as `#rrggbb` (palette/theme aware).
    pub color: String,
    /// Character `Style` bit 1.
    pub bold: bool,
    /// Character `Style` bit 2.
    pub italic: bool,
    /// Paragraph `HorzAlign`: 0 left, 1 center (Visio default), 2
    /// right, 3 justify.
    pub halign: i32,
    /// `VertAlign`: 1 top, 2 middle (default), 3 bottom.
    pub valign: i32,
    /// Character `ComplexScriptFont` family (#257 text-direction):
    /// the face complex-script (RTL-base) paragraphs paint with; the
    /// spec's zero/absent semantics resolve to `None` and the Latin
    /// `Font` chain stays. Paragraph granularity — the per-run `<cp>`
    /// model stays out until a fixture shows one (see
    /// Analysis/rtl-text-direction.md).
    pub cs_font: Option<String>,
    /// Effective `ComplexScriptSize` in inches (the spec's `0 → Size`,
    /// `<0 → |v|·Size/100` semantics already applied; `None` = Size).
    pub cs_size: Option<f64>,
}

/// Resolve one shape's [`TextBlock`] through the instance → master
/// chain (GitHub #22): the same cell reads and defaults `emit_text`
/// performs, hoisted so the GUI overlay and the renderer agree.
pub fn text_block(doc: &Document, page: &vsdx::Page, shape_id: i64) -> Option<TextBlock> {
    let chain = dc_core::scope::shape_chain(doc, page.id, shape_id).ok()?;
    let s = *chain.last()?;
    let env = shape_env(doc, s, &chain);
    let get = |n: &str| -> Option<f64> {
        doc.resolve_cell(s, &chain, n)
            .and_then(|c| cell_num(&c, &env))
    };
    let width = get("Width").unwrap_or(1.0);
    let height = get("Height").unwrap_or(1.0);
    Some(resolve_text_block(doc, s, &chain, width, height))
}

/// The shape's text block in PAGE coordinates (inches, y-up, origin
/// bottom-left — [`shape_page_rect`]'s convention): the block's local
/// rect mapped through the ancestor XForm chain. The shape's own
/// `Angle`/`TxtAngle` are not applied (v1 of the in-place editor,
/// GitHub #22 — a rotated shape yields the axis-aligned box).
pub fn text_block_page_rect(
    doc: &Document,
    page: &vsdx::Page,
    shape_id: i64,
) -> Option<(f64, f64, f64, f64)> {
    let page_h = num(page.sheet().and_then(|s| s.cell("PageHeight"))).unwrap_or(11.0);
    let chain = dc_core::scope::shape_chain(doc, page.id, shape_id).ok()?;
    let s = *chain.last()?;
    let env = shape_env(doc, s, &chain);
    let get = |n: &str| -> Option<f64> {
        doc.resolve_cell(s, &chain, n)
            .and_then(|c| cell_num(&c, &env))
    };
    let width = get("Width").unwrap_or(1.0);
    let height = get("Height").unwrap_or(1.0);
    let tb = resolve_text_block(doc, s, &chain, width, height);
    // Shape-local → parent-local (v1, unrotated: the same
    // pin-anchored placement `shape_page_rect` uses for the shape
    // rect) → page through the ancestor chain.
    let (pin_x, pin_y) = (get("PinX").unwrap_or(0.0), get("PinY").unwrap_or(0.0));
    let (loc_x, loc_y) = (
        get("LocPinX").unwrap_or(width / 2.0),
        get("LocPinY").unwrap_or(height / 2.0),
    );
    let m = ancestors_affine(doc, &chain, page_h);
    let nested = chain.len() >= 2;
    let corner = |x: f64, y: f64| -> (f64, f64) {
        let iy = if nested { -y } else { page_h - y };
        let (sx, sy) = m.apply(x, iy);
        (sx, page_h - sy)
    };
    let (tx, ty) = tb.pin;
    let (tw, th) = tb.size;
    let (lpx, lpy) = tb.loc_pin;
    let bx = pin_x - loc_x + tx - lpx;
    let by = pin_y - loc_y + ty - lpy;
    let pts = [
        corner(bx, by),
        corner(bx + tw, by),
        corner(bx, by + th),
        corner(bx + tw, by + th),
    ];
    let xs = pts.iter().map(|p| p.0);
    let ys = pts.iter().map(|p| p.1);
    let (min_x, max_x) = (
        xs.clone().fold(f64::INFINITY, f64::min),
        xs.fold(f64::NEG_INFINITY, f64::max),
    );
    let (min_y, max_y) = (
        ys.clone().fold(f64::INFINITY, f64::min),
        ys.fold(f64::NEG_INFINITY, f64::max),
    );
    Some((min_x, min_y, max_x - min_x, max_y - min_y))
}

/// The complex-script size an RTL line's glyphs actually paint with
/// (#257 text-direction, PR #269 review): the resolved
/// ComplexScriptSize (negative-percent semantics already applied)
/// when it EXCEEDS the Latin size — a smaller CS size keeps the
/// block's Latin slot (Visio's LineSpacing is a block cell; only
/// larger glyphs need a taller slot or adjacent lines overlap).
pub(crate) fn effective_cs_size(tb: &TextBlock) -> f64 {
    tb.cs_size.unwrap_or(tb.font_size).max(tb.font_size)
}

/// One wrapped line's vertical slot: the block's line height, scaled
/// by the line's effective size when its glyphs paint larger than the
/// Latin size (RTL lines under a bigger ComplexScriptSize — PR #269
/// review: default spacing is 1.2 em, so 2 em glyphs in 1.2 em slots
/// overlapped).
pub(crate) fn line_slot_height(tb: &TextBlock, rtl: bool) -> f64 {
    if rtl {
        tb.line_height * (effective_cs_size(tb) / tb.font_size)
    } else {
        tb.line_height
    }
}

/// The first baseline's offset below the text-block's TOP edge, in
/// the block's local inches (y-down) — the single placement authority
/// shared by `emit_text` and the GUI's in-place editor overlay, so
/// the editing caret sits on the very line the renderer draws (the
/// #22 review's "no translation between edit and render"). Subsequent
/// lines sit their [`line_slot_height`] lower. `total_h` is the
/// wrapped text's height (the SUM of the line slots), and
/// `first_line_size` is the first line's effective glyph size — a
/// first line of larger complex-script glyphs ascends from a higher
/// baseline than the Latin ascent.
pub fn text_first_baseline(tb: &TextBlock, total_h: f64, first_line_size: f64) -> f64 {
    let (_ins_l, _ins_r, ins_t, ins_b) = tb.insets;
    let th = tb.size.1;
    match tb.valign {
        1 => ins_t + first_line_size * 0.8,                // top
        3 => th - ins_b - total_h + first_line_size * 0.8, // bottom
        _ => (th - total_h) / 2.0 + first_line_size * 0.8, // middle
    }
}

/// The cell resolution behind [`TextBlock`] — shared by `emit_text`
/// (which paints from it) and the public [`text_block`] wrapper.
pub(crate) fn resolve_text_block(
    doc: &Document,
    s: &Shape,
    chain: &[&Shape],
    width: f64,
    height: f64,
) -> TextBlock {
    let env = shape_env(doc, s, chain);
    let get = |n: &str| -> Option<f64> {
        doc.resolve_cell(s, chain, n)
            .and_then(|c| cell_num(&c, &env))
    };
    let tx = get("TxtPinX").unwrap_or(width / 2.0);
    let ty = get("TxtPinY").unwrap_or(height / 2.0);
    let tw = get("TxtWidth").unwrap_or(width).max(0.02);
    let th = get("TxtHeight").unwrap_or(height).max(0.02);
    let angle = get("TxtAngle").unwrap_or(0.0);
    // Char.Size lives in the Character section's first row, stored in
    // inches (0.13889 in = 10 pt); the viewBox unit is inches, so the
    // value is the SVG font-size directly.
    let font_size = char_size(doc, s, chain).unwrap_or(0.13888).max(0.02);
    // First Character row extras: color (palette/theme aware) and the
    // Style bitmask (1 = bold, 2 = italic). Both resolve per-cell through
    // `char_cell`, so a LangID-only instance row picks them up from the
    // master/style rows instead of silently taking the built-in defaults.
    let color = char_cell(doc, s, chain, "Color")
        .map(|c| color_value(doc, c.v.as_deref().unwrap_or(""), c.f.as_deref()))
        .unwrap_or_else(|| "#000000".into());
    let style_bits = char_cell(doc, s, chain, "Style")
        .and_then(|c| c.numeric())
        .unwrap_or(0.0) as u32;

    // Paragraph alignment: Visio HorzAlign 0 = left, 1 = center, 2 = right,
    // 3 = justify (rendered as left). Absent cell → Visio default 1 (center).
    // Sections inherit separately from cells: a sub-shape that carries e.g.
    // Character but no Paragraph still takes Paragraph from its master shape
    // (`MasterShape` link) — effective_sheet stops at the instance sheet
    // once it has any sections, so consult the source explicitly.
    let para_row = s
        .sheet
        .sections_named("Paragraph")
        .into_iter()
        .next()
        .or_else(|| {
            doc.instance_source(s, chain)
                .and_then(|src| src.sheet.sections_named("Paragraph").into_iter().next())
        })
        .and_then(|sec| sec.rows.first());
    let halign = para_row
        .and_then(|r| r.cell("HorzAlign"))
        .and_then(|c| c.numeric())
        .unwrap_or(1.0) as i32;
    let valign = get("VertAlign").unwrap_or(2.0) as i32;

    // Text-block insets (Visio defaults: 0.1 in left/right, 0.0555 in
    // top/bottom) and 120% line spacing.
    let ins_l = get("LeftMargin").unwrap_or(0.1);
    let ins_r = get("RightMargin").unwrap_or(0.1);
    let ins_t = get("TopMargin").unwrap_or(0.0555);
    let ins_b = get("BottomMargin").unwrap_or(0.0555);
    let line_height = get("LineSpacing")
        .map(|l| l * font_size)
        .unwrap_or(font_size * 1.2)
        .max(font_size);

    // Complex-script character cells (#257 text-direction, paragraph
    // granularity — see Analysis/rtl-text-direction.md). Size
    // semantics per [MS-VSDX] §2.4.4.62: >0 = the value, 0 = the
    // sibling Size, <0 = |v|·Size/100.
    let cs_font = char_cell(doc, s, chain, "ComplexScriptFont")
        .and_then(|c| c.v.clone())
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty() && v.parse::<f64>().is_err());
    let cs_size_raw = char_cell(doc, s, chain, "ComplexScriptSize").and_then(|c| c.numeric());
    let cs_size = cs_size_raw.map(|v| {
        if v > 0.0 {
            v
        } else if v < 0.0 {
            v.abs() * font_size / 100.0
        } else {
            font_size
        }
    });

    TextBlock {
        pin: (tx, ty),
        size: (tw, th),
        loc_pin: (
            get("TxtLocPinX").unwrap_or(tw / 2.0),
            get("TxtLocPinY").unwrap_or(th / 2.0),
        ),
        angle,
        insets: (ins_l, ins_r, ins_t, ins_b),
        line_height,
        font: char_font(doc, s, chain),
        font_size,
        color,
        bold: style_bits & 1 != 0,
        italic: style_bits & 2 != 0,
        halign,
        valign,
        cs_font,
        cs_size,
    }
}

/// The greedy wrap both the renderer (`emit_text`) and the
/// `TEXTHEIGHT` measurement (#147) run — one wrap, so block-size
/// caches and the rendered lines can never disagree.
///
/// Visio stores line breaks as CRLF: breaking on each of \r and \n
/// separately would insert an empty paragraph between every pair of
/// lines, so normalize CRLF (and lone CR) to one break first.
/// Character-exact lines (#144): each paragraph is reconstructed
/// verbatim — `split(' ')` words joined back with their separator
/// spaces — so a paragraph-leading space run (empty first words)
/// survives into the first line, interior runs stay, and a
/// whitespace-only paragraph emits its spaces instead of an empty
/// line. `started` distinguishes "no words consumed yet" from "an
/// empty word consumed" (a pending leading space): a bare
/// `cur.is_empty()` test cannot tell them apart, which is how the
/// leading run used to vanish.
/// One wrapped line plus its paragraph's base direction (#257
/// text-direction): alignment and emission order follow the base, so
/// the wrap must carry it per line (a block can mix LTR and RTL
/// paragraphs).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct WrapLine {
    pub text: String,
    pub rtl: bool,
}

/// The Right-to-Left Text toggle's transform (#257 text-direction,
/// PR #269 review): `to_rtl` prepends an explicit U+202B (RLE) to
/// every paragraph; the reverse strips the leading direction marks
/// and then pins paragraphs whose first-strong scan still reads RTL
/// with an explicit U+202A (LRE) — "off" must really read LTR even
/// on naturally-RTL text, or the toggle is a visual no-op for it.
/// CRLF input normalizes to LF like the renderer's own wrap.
pub fn toggle_rtl_marks(text: &str, to_rtl: bool) -> String {
    let paras = text.replace("\r\n", "\n").replace('\r', "\n");
    // A text with NO content at all still takes the mark — it
    // PRE-SETS the direction for the first paragraph typed into an
    // empty shape (PR #269 review); only the trailing empty of a
    // NON-empty text stays bare.
    let pre_empty = paras.is_empty();
    let mut out: Vec<String> = Vec::new();
    for l in paras.split('\n') {
        // Both directions REPLACE the leading mark — stacking an RLE
        // onto an existing LRE pin would leave two overrides.
        let stripped = l.trim_start_matches(['\u{202A}', '\u{202B}', '\u{202C}']);
        if stripped.is_empty() && !pre_empty {
            // An EMPTY paragraph takes no mark: a mark on the trailing
            // terminator's empty paragraph is content to the wrapper
            // (a direction mark is not whitespace, so the renderer's
            // `trim_end` keeps it) and "hello\r\n" would gain a second
            // TEXTHEIGHT line — the corpus's trailing-break-adds-no-
            // line gold (#148) must survive a direction toggle (PR
            // #269 review).
            out.push(String::new());
        } else if to_rtl {
            out.push(format!("\u{202B}{stripped}"));
        } else {
            out.push(if paragraph_base_rtl(stripped) {
                format!("\u{202A}{stripped}")
            } else {
                stripped.to_string()
            });
        }
    }
    out.join("\n")
}

/// A paragraph's base direction (Analysis/rtl-text-direction.md):
/// explicit U+202A/U+202B overrides first, then the first strong
/// bidi character (R/AL → RTL, L → LTR, neutrals and numbers
/// skipped); no strong character → LTR. Public for dc-ui's
/// Right-to-Left Text toggle, which needs the renderer's exact rule
/// when it pins a stripped paragraph LTR (PR #269 review).
pub fn paragraph_base_rtl(para: &str) -> bool {
    let mut chars = para.chars().peekable();
    // Leading explicit embedding marks decide outright.
    while let Some(&c) = chars.peek() {
        match c {
            '\u{202B}' => return true, // RLE
            '\u{202A}' => return false, // LRE
            '\u{200E}' | '\u{200F}' | '\u{202C}' => {
                chars.next();
            }
            _ => break,
        }
    }
    for c in chars {
        use unicode_bidi::BidiClass as BC;
        match unicode_bidi::bidi_class(c) {
            BC::R | BC::AL => return true,
            BC::L => return false,
            _ => {} // neutrals, EN/AN, marks: keep scanning
        }
    }
    false
}

/// The greedy wrap both the renderer (`emit_text`) and the
/// `TEXTHEIGHT` measurement (#147) run — one wrap, so block-size
/// caches and the rendered lines can never disagree.
///
/// Visio stores line breaks as CRLF: breaking on each of \r and \n
/// separately would insert an empty paragraph between every pair of
/// lines, so normalize CRLF (and lone CR) to one break first.
/// Character-exact lines (#144): each paragraph is reconstructed
/// verbatim — `split(' ')` words joined back with their separator
/// spaces — so a paragraph-leading space run (empty first words)
/// survives into the first line, interior runs stay, and a
/// whitespace-only paragraph emits its spaces instead of an empty
/// line. `started` distinguishes "no words consumed yet" from "an
/// empty word consumed" (a pending leading space): a bare
/// `cur.is_empty()` test cannot tell them apart, which is how the
/// leading run used to vanish.
///
/// #257: `width_in` measures ONE paragraph with the face/size that
/// paragraph paints with — an RTL-base paragraph resolves the
/// complex-script cells (`cs_face`/`cs_size`, the Latin pair when
/// absent), so the wrap decision and the raster agree by construction
/// per paragraph, not just per block.
pub(crate) fn wrap_block_text(
    face: &crate::textmetrics::ResolvedFace,
    text: &str,
    size: f64,
    usable_w: f64,
    cs_face: &crate::textmetrics::ResolvedFace,
    cs_size: f64,
    cs_fallback: &crate::textmetrics::ResolvedFace,
) -> Vec<WrapLine> {
    let mut lines: Vec<WrapLine> = Vec::new();
    for para in text.replace("\r\n", "\n").replace('\r', "\n").split('\n') {
        let rtl = paragraph_base_rtl(para);
        // An RTL paragraph measures RUN BY RUN with the faces the
        // raster paints it with (PR #269 review): the resolved CS
        // face, and the vendored Vazirmatn for runs it lacks — usvg's
        // per-cluster fallback draws those clusters with the fallback
        // face, so the wrap's advances must agree or line breaks and
        // start-aligned edges drift. A pure-script paragraph (Latin
        // text merely MARKED rtl, or straight Arabic) measures as one
        // run through the same single-face path as before.
        let width_in = |s: &str| {
            if rtl {
                cs_face.width_in_runs(cs_fallback, s, cs_size)
            } else {
                face.width_in(s, size)
            }
        };
        let mut cur = String::new();
        let mut started = false;
        for word in para.split(' ') {
            let candidate = if started {
                format!("{cur} {word}")
            } else {
                word.to_string()
            };
            if started && width_in(&candidate) > usable_w {
                if cur.is_empty() {
                    // The break lands on a pending separator space (an
                    // authored space run or a leading indent too wide
                    // to share a line with the word): the space is
                    // content (#144 review) — it becomes a line of its
                    // own, never an empty line, never dropped.
                    lines.push(WrapLine {
                        text: " ".to_string(),
                        rtl,
                    });
                } else {
                    lines.push(WrapLine {
                        text: std::mem::take(&mut cur),
                        rtl,
                    });
                }
                cur = word.to_string();
            } else {
                cur = candidate;
            }
            started = true;
            // A single word wider than the line: hard-split at the
            // longest character prefix that fits (always ≥1 char, so
            // the loop makes progress).
            while width_in(&cur) > usable_w && cur.chars().count() > 1 {
                let mut split_at = cur
                    .char_indices()
                    .nth(1)
                    .map(|(i, _)| i)
                    .unwrap_or(cur.len());
                for (i, _) in cur.char_indices().skip(1) {
                    if width_in(&cur[..i]) <= usable_w {
                        split_at = i;
                    } else {
                        break;
                    }
                }
                let rest = cur.split_off(split_at);
                lines.push(WrapLine {
                    text: std::mem::replace(&mut cur, rest),
                    rtl,
                });
            }
        }
        lines.push(WrapLine {
            text: cur,
            rtl,
        });
    }
    lines
}

#[allow(clippy::too_many_arguments)] // placement + flip axes + the #22 skip + #124 greek ctx
fn emit_text(
    doc: &Document,
    s: &Shape,
    chain: &[&Shape],
    width: f64,
    height: f64,
    sx: f64,
    sy: f64,
    skip: bool,
    greek: Option<&GreekCtx>,
    out: &mut String,
) {
    if skip {
        return;
    }
    // Authored leading spacing is content (#144): trim only the END —
    // the first paragraph's leading space run must survive into its
    // line. The trailing trim is now GOLD-BACKED (#148, 2026-10-30):
    // the shipped corpus carries Visio's own cached
    // `TEXTHEIGHT(TheText,TxtWidth)` values, and 25 one-visual-line
    // texts ending in the corpus-standard CRLF (DBUML_M/U "PK\r\n",
    // TITLE_M/U "\r\n", ELETEL_M, OFFEQP…) cache exactly ONE line
    // height (1.2003–1.2004 em at the canonical 120% spacing) — Visio
    // does NOT count the trailing break as a line, so dropping it
    // here keeps the block's line count and vertical centering where
    // Visio puts them.
    let Some(text) = s
        .sheet
        .text
        .as_deref()
        .map(str::trim_end)
        .filter(|t| !t.is_empty())
    else {
        return;
    };
    let tb = resolve_text_block(doc, s, chain, width, height);
    let (tx, ty) = tb.pin;
    let (tw, th) = tb.size;
    let angle = tb.angle;
    let size = tb.font_size;
    let color = tb.color.clone();
    let style_bits = (tb.bold as u32) | ((tb.italic as u32) << 1);
    let font_attrs = format!(
        r#"font-size="{s:.4}"{ff} fill="{c}"{b}{i}"#,
        s = size,
        ff = font_family_attr(tb.font.as_deref()),
        // Document-supplied color text (e.g. a `#…` value) is attribute
        // content — escape it like every other attribute.
        c = escape_attr(&color),
        b = if style_bits & 1 != 0 {
            r#" font-weight="bold""#
        } else {
            ""
        },
        i = if style_bits & 2 != 0 {
            r#" font-style="italic""#
        } else {
            ""
        },
    );
    let halign = tb.halign;
    let valign = tb.valign;
    let (ins_l, ins_r, ins_t, ins_b) = tb.insets;
    let line_h = tb.line_height;

    // Greedy word wrap against REAL font metrics (GitHub #21): runs
    // are measured as the sum of per-glyph advances from the face the
    // rasterizer will draw (system font where installed, vendored
    // Carlito otherwise — `textmetrics`), replacing the ~0.5 em
    // average-advance guess that re-wrapped lines the author had
    // fitted to the block.
    let usable_w = (tw - ins_l - ins_r).max(0.02);
    let face = crate::textmetrics::resolve(tb.font.as_deref(), tb.bold, tb.italic);
    // The complex-script pair (#257 text-direction): an RTL-base
    // paragraph measures and paints with ComplexScriptFont/Size —
    // falling back to the Latin chain/size per the spec's zero
    // semantics — so wrap and raster agree per paragraph. The vendored
    // Vazirmatn (dc::register_complex_script_faces) covers the glyphs when the
    // document names no CS face.
    let cs_chain = tb.cs_font.as_deref().or(tb.font.as_deref());
    let cs_face = crate::textmetrics::resolve(cs_chain, tb.bold, tb.italic);
    // The per-cluster fallback face (the vendored Vazirmatn — see the
    // wrap's probe note); unregistered, it resolves like the Latin
    // default and the probe never fires.
    let cs_fallback = crate::textmetrics::resolve(Some("Vazirmatn"), tb.bold, tb.italic);
    let cs_size = tb.cs_size.unwrap_or(size);
    // Greeking (#124 item 2): a block whose on-screen em height falls
    // under TEXT_GREEK_PX renders as gray bars — per line, shaped only
    // at paragraph granularity (one width_in per paragraph, no greedy
    // word wrap and no <text> node for usvg to turn into glyph
    // outlines). Every met size is recorded regardless, so the layer
    // cache can tell when a ppi-bucket change flips the decision.
    let greeked = greek.is_some_and(|g| {
        g.sizes.borrow_mut().push(size);
        g.greeks(size)
    });
    let (lines, bar_widths) = if greeked {
        let mut bars = Vec::new();
        for para in text.replace("\r\n", "\n").replace('\r', "\n").split('\n') {
            let w = face.width_in(para, size);
            if w <= usable_w {
                bars.push(w.max(usable_w * 0.2));
            } else {
                // Chunk the paragraph across an estimated wrap: full
                // bars of the usable width plus one remainder bar.
                let full = (w / usable_w).floor() as usize; // ≥ 1: w > usable_w
                let rem = w - full as f64 * usable_w;
                bars.extend(std::iter::repeat_n(usable_w, full));
                if rem > usable_w * 0.05 {
                    bars.push(rem);
                }
            }
        }
        (Vec::new(), bars)
    } else {
        (
            wrap_block_text(&face, text, size, usable_w, &cs_face, cs_size, &cs_fallback),
            Vec::new(),
        )
    };
    let n_lines = if greeked {
        bar_widths.len()
    } else {
        lines.len()
    } as f64;
    // Per-line slots (PR #269 review): an RTL line under a bigger
    // ComplexScriptSize needs a proportionally taller slot — the flat
    // n × line_h overlapped larger glyphs onto their neighbors, and
    // TEXTHEIGHT (textfunc) measured the same too-small block.
    let slots: Vec<f64> = if greeked {
        vec![line_h; n_lines as usize]
    } else {
        lines.iter().map(|l| line_slot_height(&tb, l.rtl)).collect()
    };
    let total_h: f64 = slots.iter().sum();
    // The first line's ascent rides its own effective size.
    let first_size = lines
        .first()
        .map_or(size, |l| if l.rtl { effective_cs_size(&tb) } else { size });

    // Text-block rect from the sheet: the pin (TxtPinX/TxtPinY, y-up local)
    // sits TxtLocPinX/TxtLocPinY from the block's lower-left corner, so that
    // corner is (tx - lpx, ty - lpy) — masters can pin off-center, so no
    // centered-pin assumption. In the emitted y-negated frame the top edge
    // is -(ty - lpy + th).
    let (lpx, lpy) = tb.loc_pin;
    let top_edge = -(ty - lpy + th);
    let first_baseline = top_edge + text_first_baseline(&tb, total_h, first_size);
    let _ = (valign, ins_t, ins_b);
    // Line x per alignment: left edge of the text area, center, or right.
    // #257 text-direction: an RTL-base line reads 0/2 as start/end
    // relative to its READING order (Visio's RTL editions), so the
    // visual left edge moves with the line's base. The anchor stays
    // "start" (usvg lays glyphs from the visual left under its
    // hardcoded LTR base); the x is the line's computed visual left.
    let left_x = tx - lpx + ins_l;
    let right_x = left_x + (tw - ins_l - ins_r);
    let (anchor, x) = match halign {
        0 | 3 => ("start", left_x),
        2 => ("end", right_x),
        _ => ("middle", left_x + (tw - ins_l - ins_r) / 2.0),
    };

    let mut body = String::new();
    // Baselines ride each line's OWN ascent: the running coordinate is
    // the next line's slot TOP (the first baseline already sits
    // `first_size × 0.8` below the block's band top), and line i's
    // baseline is its slot top plus ITS effective ascent — a larger
    // complex-script line following a Latin one rises by the larger
    // ascent instead of overlapping it (PR #269 review; uniform for
    // greek bars, whose Latin ascent/slots make this byte-identical
    // to the old advance-by-slot form).
    let mut top = first_baseline - first_size * 0.8;
    if greeked {
        // Greek bars (#124 item 2): one neutral-gray rect per line,
        // anchored like the tspan it replaces (left/center/right).
        // Height sits in the x-height band of the line's em box.
        for (w, slot) in bar_widths.iter().zip(&slots) {
            let (bx, bw) = match anchor {
                "start" => (x, *w),
                "end" => (x - *w, *w),
                _ => (x - *w / 2.0, *w),
            };
            let _ = write!(
                body,
                r##"<rect x="{bx:.4}" y="{y:.4}" width="{bw:.4}" height="{h:.4}" fill="#7f7f7f"/>"##,
                y = top + size * 0.8 - size * 0.72,
                h = size * 0.62,
            );
            top += slot;
        }
    }
    let mut any_rtl = false;
    for (line, slot) in lines.iter().zip(&slots) {
        let (content, line_x, line_attrs) = if line.rtl {
            any_rtl = true;
            // The RTL line paints with the complex-script pair; its
            // visual LEFT edge per the start/end mapping, computed
            // from the line's own shaped width. The face/size ride
            // the tspan — a block may mix direction lines and the
            // element-level attrs stay Latin.
            let lw = cs_face
                .width_in_runs(&cs_fallback, &line.text, cs_size);
            let content = format!("\u{2067}{}\u{2069}", line.text);
            let vx = match halign {
                0 | 3 => right_x - lw, // start (reading order) = visual right edge
                2 => left_x,           // end (reading order) = visual left edge
                _ => x,                // center: the shared center
            };
            // PR #269 review: a right-aligned (halign 2) block anchors
            // the ELEMENT "end", but this line sits at the block's
            // visual LEFT edge — the inherited "end" would swing its
            // glyphs further left, outside the block. It anchors
            // itself "start" there. `font_family_attr` returns the
            // COMPLETE attribute (leading space included) — wrapping
            // it in another font-family="" nested the quotes and
            // emitted malformed XML.
            let attrs = format!(
                "{}{}",
                if halign == 2 {
                    r#" text-anchor="start""#
                } else {
                    ""
                },
                if (cs_size - size).abs() > 1e-9 {
                    format!(r#" font-size="{cs_size:.4}"{}"#, font_family_attr(cs_chain))
                } else {
                    font_family_attr(cs_chain)
                }
            );
            (content, vx, attrs)
        } else {
            (line.text.clone(), x, String::new())
        };
        // This line's baseline: its slot top plus ITS OWN effective
        // ascent (0.8 × effective size — the same factor
        // `text_first_baseline` gives the first line).
        let line_size = if line.rtl {
            effective_cs_size(&tb)
        } else {
            size
        };
        let y = top + line_size * 0.8;
        let _ = write!(
            body,
            r#"<tspan x="{line_x:.4}" y="{y:.4}"{}>{}</tspan>"#,
            line_attrs,
            escape_text(&content)
        );
        top += slot;
    }
    // Flip-aware block rotation (issue #19): the counter-scale keeps the
    // glyphs un-mirrored inside the caller's flipped frame, and the
    // rotation composes with flip parity so the page-space net is
    // Angle + sx*sy*TxtAngle.
    let flipped = sx < 0.0 || sy < 0.0;
    let net_txt_deg = -(sx * sy) * angle.to_degrees();
    // The block's tspan coordinates are pin-absolute (TxtPin-anchored),
    // so the counter-scale/rotation must pivot AT the pin: the inverse
    // translate after them keeps the text anchor fixed there. Without
    // it they would swing the whole block around the local origin.
    let mut transform = format!("translate({x:.5},{y:.5})", x = tx, y = -ty);
    if flipped {
        let _ = write!(transform, " scale({sx:.0},{sy:.0})", sx = sx, sy = sy);
    }
    if net_txt_deg.abs() > 1e-6 {
        let _ = write!(transform, " rotate({deg:.5})", deg = net_txt_deg);
    }
    if flipped || net_txt_deg.abs() > 1e-6 {
        let _ = write!(transform, " translate({x:.5},{y:.5})", x = -tx, y = ty);
    }
    // xml:space="preserve" (#144): SVG's default whitespace handling
    // strips leading/trailing spaces of text-node content, so the
    // spaces the splitter now preserves would be dropped again at
    // raster time. usvg (resvg 0.50) honors the attribute — inherited
    // by every tspan — and skips its stripping only for preserve, so
    // the authored spacing reaches the pixels. Greeked blocks (#124)
    // carry plain rects in `body`, never glyph nodes: they emit as a
    // bare <g> (a <text> wrapper's content model has no place for
    // shapes, and no font attributes are needed).
    if greeked {
        if flipped || net_txt_deg.abs() > 1e-6 {
            let _ = writeln!(out, r#"<g transform="{}">"#, escape_attr(&transform));
            let _ = writeln!(out, r#"<g>{b}</g>"#, b = body);
            let _ = writeln!(out, "</g>");
        } else {
            let _ = writeln!(out, r#"<g>{b}</g>"#, b = body);
        }
        return;
    }
    // #257 text-direction: the RTL lines' isolates order the runs
    // under ANY base; `direction="ltr"` pins the element base for
    // viewers that honor the attribute (usvg ignores it) so the
    // visual-left start anchor reads the same everywhere. LTR-only
    // blocks keep the pre-#257 byte form.
    let dir_attr = if any_rtl {
        r#" direction="ltr""#
    } else {
        ""
    };
    if flipped || net_txt_deg.abs() > 1e-6 {
        let _ = writeln!(out, r#"<g transform="{}">"#, escape_attr(&transform));
        let _ = writeln!(
            out,
            r#"<text{d} text-anchor="{a}" xml:space="preserve" {f}>{b}</text>"#,
            d = dir_attr,
            a = anchor,
            f = font_attrs,
            b = body
        );
        let _ = writeln!(out, "</g>");
    } else {
        let _ = writeln!(
            out,
            r#"<text{d} text-anchor="{a}" xml:space="preserve" {f}>{b}</text>"#,
            d = dir_attr,
            a = anchor,
            f = font_attrs,
            b = body
        );
    }
}

/// Data URI for a shape's embedded image (`ForeignData`), resolved through
/// the owning part's relationships to the media part. EMF/WMF cannot be
/// embedded in SVG and are skipped.
fn foreign_image_uri(doc: &Document, part: &str, s: &Shape) -> Option<String> {
    let attr = |n: &str| -> Option<&str> {
        s.raw_children
            .iter()
            .find(|r| r.name == "ForeignData")?
            .attrs
            .iter()
            .find(|(k, _)| k == n)
            .map(|(_, v)| v.as_str())
    };
    if attr("ForeignType") != Some("Bitmap") {
        return None; // OLE objects / metafiles: not image parts
    }
    let mime = match attr("CompressionType") {
        Some("PNG") => "image/png",
        Some("JPEG") | Some("JPG") => "image/jpeg",
        Some("GIF") => "image/gif",
        _ => return None, // EMF/WMF and unknowns stay unrendered
    };
    let fd = s.raw_children.iter().find(|r| r.name == "ForeignData")?;
    // The rel id lives either on ForeignData itself or in a <Rel r:id=…/>
    // child (the shipped layout).
    let rid = fd
        .attrs
        .iter()
        .find(|(k, _)| k == "r:id")
        .map(|(_, v)| v.clone())
        .or_else(|| extract_rid(&fd.inner))?;
    // Runtime-inserted media (#113) resolves through `extra_media`
    // by the owning page's part — no retained package needed; the
    // package relationship wins when it carries this rid.
    let target: String = doc
        .package
        .as_ref()
        .and_then(|pkg| {
            pkg.relationships(part)
                .into_iter()
                .find(|(id, ty, _)| id == &rid && ty.ends_with("/image"))
                .map(|(_, _, t)| t)
        })
        .or_else(|| {
            let page_id = doc.pages.iter().find(|p| p.part == part).map(|p| p.id)?;
            doc.extra_media
                .iter()
                .find(|m| m.page == page_id && m.rid == rid)
                .map(|m| m.part.clone())
        })?;
    // `relationships` returns the target already resolved to an absolute
    // package part name. Runtime-inserted media (#113) lives in
    // `extra_media`, not the retained package.
    let name = target.trim_start_matches('/');
    let bytes = doc
        .package
        .as_ref()
        .and_then(|pkg| pkg.part(name))
        .or_else(|| {
            doc.extra_media
                .iter()
                .find(|m| m.part == name)
                .map(|m| m.bytes.as_slice())
        })?;
    Some(format!("data:{mime};base64,{}", base64(bytes)))
}

/// `<Rel r:id='rId9'/>` → `rId9` (both quote styles).
fn extract_rid(inner: &str) -> Option<String> {
    let i = inner.find("r:id")?;
    let rest = &inner[i + 5..];
    let q = rest.chars().next()?;
    let end = rest[1..].find(q)? + 1;
    Some(rest[1..end].to_string())
}

pub(crate) fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            T[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            T[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

/// Per-section master inheritance (Visio semantics): geometry comes from
/// the instance's own Geometry sections when present (authored in
/// instance units), else from the master source shape's Geometry (master-
/// local units, scaled to the instance size). The instance often carries
/// other inline sections (Character/Paragraph) — that must not mask the
/// inherited geometry.
fn render_all_geometry(
    doc: &Document,
    s: &Shape,
    chain: &[&Shape],
    width: f64,
    height: f64,
    out: &mut String,
) {
    // Groups usually don't self-draw (children + the master drawing
    // carry the visuals) but container groups DO carry a visible rect,
    // so group geometry renders too; degenerate leftover rows collapse
    // to invisible zero-area paths after the row-cell merge below.
    let env = shape_env(doc, s, chain);
    // #228: both frames below accumulate into ONE shape-level paint —
    // the alternate-fill union flushes under the per-section strokes.
    let mut paint = ShapePaint::default();
    let own: Vec<&vsdx::Section> = s.sheet.sections_named("Geometry");
    if !own.is_empty() {
        // Row-cell inheritance: inline copies often keep only the Y cells
        // (F='Inh') and inherit X/A/B/C/D from the master source's
        // corresponding row. Inherited values are master-local and are
        // scaled into instance units here.
        let src = doc.instance_source(s, chain);
        let mut merged: Vec<vsdx::Section> = Vec::with_capacity(own.len());
        for (si, geo) in own.iter().enumerate() {
            let mut m = (*geo).clone();
            if let Some(src) = src {
                let src_secs = src.sheet.sections_named("Geometry");
                let src_geo = src_secs.get(si);
                // Section-level cells (Geometry NoFill/NoLine/NoShow)
                // inherit from the master section just like rows do:
                // inline copies keep only row caches, and a lost NoShow
                // draws sections Visio hides — e.g. every heading-band
                // geometry of the "Plain" container style (#12).
                if let Some(sg) = src_geo {
                    for (name, cell) in &sg.cells {
                        if !m.cells.iter().any(|(n, _)| n == name) {
                            m.cells.push((name.clone(), cell.clone()));
                        }
                    }
                }
                let mw = src
                    .sheet
                    .cell("Width")
                    .and_then(|c| c.numeric())
                    .filter(|v| v.abs() > 1e-9);
                let mh = src
                    .sheet
                    .cell("Height")
                    .and_then(|c| c.numeric())
                    .filter(|v| v.abs() > 1e-9);
                let (sx, sy) = (
                    mw.map_or(1.0, |mw| width / mw),
                    mh.map_or(1.0, |mh| height / mh),
                );
                let scaled = |name: &str, v: f64| match name {
                    "X" | "A" | "C" => v * sx,
                    "Y" | "B" | "D" => v * sy,
                    _ => v,
                };
                // Pair each inline row with the master row of the SAME
                // IX. Inline copies drop rows whose cells are fully
                // inherited-identical, keeping the survivors' original
                // IX values — so occurrence pairing can match the wrong
                // row (diagonal wedges, giant arcs). Occurrence within a
                // row type is the fallback when IX is absent/ambiguous.
                let mut src_occ: std::collections::HashMap<String, usize> =
                    std::collections::HashMap::new();
                // Rows the inline copy dropped entirely (all cells
                // inherited-identical) are re-inserted from the master,
                // scaled, at their IX position — otherwise the path
                // shortcuts across the gap (diagonal wedges).
                if let (Some(sg), true) = (src_geo, m.rows.iter().any(|r| r.ix > 0)) {
                    let have: Vec<i64> = m.rows.iter().map(|r| r.ix).collect();
                    let mut inserted: Vec<(usize, Row)> = Vec::new();
                    for src_row in &sg.rows {
                        if src_row.ix > 0 && !have.contains(&src_row.ix) && src_row.t.is_some() {
                            let mut nr = src_row.clone();
                            // scale per cell name
                            for (cname, cell) in nr.cells.iter_mut() {
                                let proportional = cell
                                    .f
                                    .as_deref()
                                    .map(|f| f.contains("Width") || f.contains("Height"))
                                    .unwrap_or(true);
                                if let Some(v) = cell.numeric() {
                                    let placed = if proportional { scaled(cname, v) } else { v };
                                    cell.v = Some(format!("{placed}"));
                                }
                                cell.f = None;
                            }
                            // position: after the last own row with smaller IX
                            let pos = m
                                .rows
                                .iter()
                                .rposition(|r| r.ix > 0 && r.ix < src_row.ix)
                                .map(|i| i + 1)
                                .unwrap_or(0);
                            inserted.push((pos, nr));
                        }
                    }
                    for (pos, nr) in inserted.into_iter().rev() {
                        let pos = pos.min(m.rows.len());
                        m.rows.insert(pos, nr);
                    }
                }
                for row in m.rows.iter_mut() {
                    let Some(t) = row.t.clone() else { continue };
                    let by_ix =
                        src_geo.and_then(|sg| sg.rows.iter().find(|r| r.ix == row.ix && r.ix > 0));
                    let occ = {
                        let c = src_occ.entry(t.clone()).or_insert(0);
                        *c += 1;
                        *c
                    };
                    let by_occ = src_geo.and_then(|sg| {
                        sg.rows
                            .iter()
                            .filter(|r| r.t.as_deref() == Some(t.as_str()))
                            .nth(occ - 1)
                    });
                    let src_row = by_ix.or(by_occ);
                    let Some(src_row) = src_row else { continue };
                    let bound = 3.0 * width.max(height).max(0.02);
                    let mut extra: Vec<(String, Cell)> = Vec::new();
                    for (cname, scell) in &src_row.cells {
                        if !matches!(cname.as_str(), "X" | "Y" | "A" | "B" | "C" | "D") {
                            continue;
                        }
                        // Master cells proportional to Width/Height scale
                        // into instance units; User.*-driven cells (corner
                        // radii and the like) stay absolute — instances
                        // keep those unscaled while the body stretches.
                        let proportional = scell
                            .f
                            .as_deref()
                            .map(|f| f.contains("Width") || f.contains("Height"))
                            .unwrap_or(true);
                        let placed = |v: f64| if proportional { scaled(cname, v) } else { v };
                        // Formula-authoritative for PROPORTIONAL master
                        // cells (the same rule the row walk uses):
                        // corpus connector masters carry stale zero
                        // caches (`Y V=0 F=Height/2` — utilisation
                        // master3) that would collapse the inherited
                        // arcs to flat lines (#202). The formula
                        // evaluates against the INSTANCE dims — for a
                        // proportional formula that IS the
                        // master→instance scaling. Non-proportional
                        // formulas (`Controls.Row_1`, `User.*`) have no
                        // instance meaning; their cached value (if any)
                        // stands, unfilled otherwise.
                        // Formula-authoritative cells evaluate against
                        // the INSTANCE env — the value is already in
                        // instance units and must NOT be placed-scaled
                        // again (#203 review). Cached master-unit
                        // numerics are master-unit and keep placed().
                        // A value-only master cell (no formula — the
                        // common plain-geometry case) or a formula the
                        // instance env can't evaluate falls back to the
                        // scaled cache; skipping would let row_num
                        // default the axis to 0 and collapse the
                        // inherited geometry (#185 review).
                        let sv = if proportional {
                            scell
                                .f
                                .as_deref()
                                .and_then(|f| vsdx::formula::eval(f, &env).ok())
                                .and_then(|v| v.as_f64())
                                .filter(|v| v.is_finite() && v.abs() <= bound)
                                .or_else(|| {
                                    scell.numeric().map(placed).filter(|v| v.abs() <= bound)
                                })
                        } else {
                            scell.numeric().map(placed).filter(|v| v.abs() <= bound)
                        };
                        let Some(sv) = sv else {
                            continue;
                        };
                        let patched = Cell::value(format!("{sv}"));
                        match row.cells.iter_mut().find(|(n, _)| n == cname) {
                            Some((_, cell)) => {
                                // A local numeric V wins even under F='Inh':
                                // Visio refreshes cached values at save, so
                                // e.g. a legend member's flattened arc B=0 is
                                // the live evaluation, not a stale snapshot.
                                if cell.v.is_none()
                                    || (cell.is_inherited() && cell.numeric().is_none())
                                {
                                    *cell = patched;
                                }
                            }
                            None => extra.push((cname.clone(), patched)),
                        }
                    }
                    row.cells.extend(extra);
                }
            }
            merged.push(m);
        }
        for (i, geo) in merged.iter().enumerate() {
            // Instance frame: formulas and cached values are both in
            // instance units.
            let gname = format!("Geometry{}", i + 1);
            render_geometry(doc, s, chain, geo, (1.0, 1.0), Frame::Instance, &gname, out, &mut paint);
        }
        flush_shape_paint(&mut paint, out);
        return;
    }
    let Some(src) = doc.instance_source(s, chain) else {
        return;
    };
    let mw = src
        .sheet
        .cell("Width")
        .and_then(|c| c.numeric())
        .filter(|v| v.abs() > 1e-9);
    let mh = src
        .sheet
        .cell("Height")
        .and_then(|c| c.numeric())
        .filter(|v| v.abs() > 1e-9);
    let scale = (
        mw.map_or(1.0, |mw| width / mw),
        mh.map_or(1.0, |mh| height / mh),
    );
    // The master env builds ONCE for all sections (knee rows
    // cross-reference sibling sections) and RE-EVALUATES the master's
    // Scratch/Controls against the instance's glued Begin/End — their
    // cached V is the authored pose, and the band masters derive
    // their whole vertical span from them (`Scratch.Y2 =
    // ABS(BeginY-EndY)`, #249).
    let mut menv = shape_env(doc, src, &[]);
    for n in ["BeginX", "BeginY", "EndX", "EndY"] {
        if let Some(v) = doc.resolve_cell(s, chain, n).and_then(|c| c.numeric()) {
            menv.insert(n.to_string(), v);
        }
    }
    for sec_name in ["Scratch", "Controls"] {
        for sec in src.sheet.sections_named(sec_name) {
            for row in &sec.rows {
                for (name, cell) in &row.cells {
                    let key = match sec_name {
                        "Scratch" => format!("Scratch.{}{}", name, row.ix + 1),
                        _ => {
                            let rn = row
                                .n
                                .clone()
                                .unwrap_or_else(|| format!("Row_{}", row.ix));
                            format!("Controls.{}.{}", rn, name)
                        }
                    };
                    let v = cell
                        .f
                        .as_deref()
                        .filter(|f| *f != "Inh" && *f != "No Formula")
                        .and_then(|f| vsdx::formula::eval(f, &menv).ok())
                        .and_then(|v| v.as_f64())
                        .filter(|v| v.is_finite())
                        .or_else(|| cell.numeric());
                    if let Some(v) = v {
                        menv.insert(key, v);
                    }
                }
            }
        }
    }
    for (i, geo) in src.sheet.sections_named("Geometry").into_iter().enumerate() {
        // Master frame: rows are master-local; their formulas reference
        // the MASTER's dims and must not be re-evaluated in instance
        // units (that skews rectangles into triangles).
        render_geometry(doc, s, chain, geo, scale, Frame::Master(&mut menv), &format!("Geometry{}", i + 1), out, &mut paint);
    }
    flush_shape_paint(&mut paint, out);
}

/// Which frame geometry rows live in — picks the formula environment.
/// The master variant carries the PREPARED env (master cells, the
/// instance's glued Begin/End seeded in).
enum Frame<'a> {
    Instance,
    /// The PREPARED env (master cells, the instance's glued Begin/End
    /// seeded in), SHARED by reference across every Geometry section
    /// of the frame (#218 review): render_geometry feeds evaluated row
    /// coordinates back into it (`Geometry1.X2`), and per-section
    /// clones discarded those writes — a Geometry2 referencing
    /// Geometry1 fell back to the authored pose.
    Master(&'a mut vsdx::formula::Env),
}

#[allow(clippy::too_many_arguments)] // the walk's natural inputs (+gname, #249)
fn render_geometry(
    doc: &Document,
    s: &Shape,
    chain: &[&Shape],
    section: &vsdx::Section,
    scale: (f64, f64),
    frame: Frame<'_>,
    gname: &str,
    out: &mut String,
    paint: &mut ShapePaint,
) {
    let rows = &section.rows;
    let (width, height) = shape_size(doc, s, chain);
    // Formulas evaluate in the geometry's own frame: instance dims for
    // inline geometry, the master source's dims for inherited geometry.
    // The cached V already lives in that frame; evaluating a formula in
    // the WRONG frame (e.g. master rows against instance Width) skews
    // rectangles into triangles.
    let mut inst_env;
    let env: &mut vsdx::formula::Env = match frame {
        Frame::Instance => {
            inst_env = shape_env_for(doc, s, chain, rows);
            &mut inst_env
        }
        Frame::Master(env) => env,
    };
    // Section-level cells carry NoFill/NoLine/NoShow. Formula first,
    // cached V as fallback (the row-cell convention): the master's
    // cache reflects its default style while the formula re-evaluates
    // per instance — NoShow=OR(User.hidden,User.CStyle<>7) gates the
    // container heading's geometry bands by container style.
    let flag = |name: &str| -> bool {
        section.cell(name).is_some_and(|c| {
            let by_formula =
                c.f.as_deref()
                    .and_then(|f| {
                        if f == "Inh" || f == "No Formula" {
                            None
                        } else {
                            vsdx::formula::eval(f, env).ok().and_then(|v| v.as_f64())
                        }
                    })
                    .filter(|v| v.is_finite());
            by_formula.or_else(|| c.numeric()).is_some_and(|v| v != 0.0)
        })
    };
    let no_fill = flag("NoFill");
    let no_show = flag("NoShow");
    if no_show {
        return;
    }
    // Visio hides the outline when LinePattern = 0 (e.g. legend label
    // boxes carry THEMEGUARD(0)); the geometry still exists for hit
    // testing but must not stroke.
    let line_pattern = doc
        .resolve_cell(s, chain, "LinePattern")
        .and_then(|c| cell_num(&c, env));
    let no_line = flag("NoLine") || line_pattern.is_some_and(|p| p == 0.0);
    fn row_num(env: &vsdx::formula::Env, r: &Row, c: &str) -> Option<f64> {
        r.cell(c).and_then(|cell| {
            // Geometry rows: the formula is authoritative (Visio evaluates
            // F at load; page copies can carry a stale cached V — e.g.
            // V='0' with F='Width*1' after a resize). Fall back to the
            // cache when there is no evaluable formula.
            let by_formula = cell
                .f
                .as_deref()
                .and_then(|f| {
                    if f == "Inh" || f == "No Formula" {
                        None
                    } else {
                        vsdx::formula::eval(f, env).ok().and_then(|v| v.as_f64())
                    }
                })
                .filter(|v| v.is_finite());
            by_formula.or_else(|| cell.numeric())
        })
    }

    let fill_color = themed_color_string(doc, s, chain, "FillForegnd");
    let line_color = themed_color_string(doc, s, chain, "LineColor");
    let fill_bkgnd = themed_color_string(doc, s, chain, "FillBkgnd");
    let line_weight = doc
        .resolve_cell(s, chain, "LineWeight")
        .and_then(|c| cell_num(&c, env))
        .unwrap_or(0.0104);
    // Visio never fills a 1-D shape (a line/arc has no interior): some
    // line masters carry decorative FillPattern cells (28/30) that must
    // be ignored, or the arc-chord region paints over everything.
    let one_d = doc.resolve_cell_explicit(s, chain, "BeginX").is_some()
        || doc.resolve_cell_explicit(s, chain, "EndX").is_some();
    let fill_pattern = if one_d {
        0.0
    } else {
        doc.resolve_cell_explicit(s, chain, "FillPattern")
            .and_then(|c| cell_num(&c, env))
            .unwrap_or(1.0)
    };
    // Gradient fills: Visio theme fill patterns 24-40 are gradients
    // (FillForegnd → FillBkgnd along FillGradientAngle, default 90°
    // top-to-bottom). Everything else patterned >1.5 is a hatch: the
    // background dominates, so it renders as the background color.
    let fill = if no_fill || fill_pattern == 0.0 {
        "none".to_string()
    } else if (24.0..=40.0).contains(&fill_pattern) {
        let angle_deg = doc
            .resolve_cell_explicit(s, chain, "FillGradientAngle")
            .and_then(|c| cell_num(&c, env))
            .map(|rad| rad.to_degrees())
            .unwrap_or(90.0);
        let id = format!(
            "dc-grad{:x}",
            fnv(&format!("{fill_color}|{fill_bkgnd}|{angle_deg:.0}"))
        );
        if !out.contains(&format!(r#"id="{id}""#)) {
            // Angle from north, clockwise → SVG gradient vector.
            let rad = angle_deg.to_radians();
            let (x1, y1) = (0.5 - rad.sin() / 2.0, 0.5 + rad.cos() / 2.0);
            let (x2, y2) = (0.5 + rad.sin() / 2.0, 0.5 - rad.cos() / 2.0);
            let _ = writeln!(
                out,
                r#"<defs><linearGradient id="{id}" x1="{x1:.4}" y1="{y1:.4}" x2="{x2:.4}" y2="{y2:.4}"><stop offset="0" stop-color="{fg}"/><stop offset="1" stop-color="{bg}"/></linearGradient></defs>"#,
                fg = escape_attr(&fill_color),
                bg = escape_attr(&fill_bkgnd),
            );
        }
        format!("url(#{id})")
    } else if fill_pattern > 1.5 {
        fill_bkgnd.clone()
    } else {
        fill_color.clone()
    };
    let dash = line_pattern.and_then(|p| line_dasharray(p, line_weight));
    let style = style_attr(
        no_fill,
        no_line,
        &fill,
        &line_color,
        line_weight,
        dash.as_deref(),
    );
    // #228: the section's own element is STROKE-only (fill
    // participation joins the shape's one even-odd path); standalone
    // Ellipse rows keep the full `style` above.
    let stroke_a = stroke_attr(no_line, &line_color, line_weight, dash.as_deref());

    // Master-space (unscaled) path state: geometry rows are authored in the
    // master's local units and scaled to the instance afterwards.
    let mut d = String::new();
    let mut cur = (0.0f64, 0.0f64); // master-space current point
    let sc = |p: (f64, f64)| -> (f64, f64) { (p.0 * scale.0, p.1 * scale.1) };
    // Emit a path coordinate pair with the local Y flip (Visio Y-up → SVG Y-down).
    let emit_pt = |p: (f64, f64)| -> String {
        let p = sc(p);
        format!("{:.5} {:.5} ", p.0, -p.1)
    };
    // #188: the spline-family accumulator — a SplineStart row opens a
    // spline, SplineKnot rows extend it, and any other row (or the
    // section's end) flushes the evaluated curve. 16 samples per knot
    // span: smooth at print scale, and the hit walk flattens with the
    // same constant so hit-testing agrees with the ink.
    let mut spline_acc: Option<crate::curve::SplineAcc> = None;
    macro_rules! flush_spline {
        () => {
            if let Some(acc) = spline_acc.take() {
                if let Some(data) = acc.flush() {
                    let pts = data.flatten(16);
                    for p in pts.iter().skip(1) {
                        let _ = write!(d, "L {}", emit_pt(*p));
                    }
                    if let Some(last) = pts.last() {
                        cur = *last;
                    }
                }
            }
        };
    }

    // The section-end flush: no further rows follow, so the walk's
    // current point is not updated (that assignment would be dead).
    macro_rules! flush_spline_tail {
        () => {
            if let Some(acc) = spline_acc.take() {
                if let Some(data) = acc.flush() {
                    let pts = data.flatten(16);
                    for p in pts.iter().skip(1) {
                        let _ = write!(d, "L {}", emit_pt(*p));
                    }
                }
            }
        };
    }

    for row in rows.iter().filter(|r| r.t.is_some()) {
        let Some(t) = row.t.as_deref() else { continue };
        // Any row type except SplineKnot ends an open spline (#188,
        // #185 review): flush the accumulated curve BEFORE the row's
        // own command so the path keeps its drawn order — a LineTo
        // after a knot used to land in `d` first, jumping the pen
        // back when the spline finally flushed.
        if t != "SplineKnot" {
            flush_spline!();
        }
        let (mx, my) = (
            row_num(env, row, "X").unwrap_or(0.0),
            row_num(env, row, "Y").unwrap_or(0.0),
        );
        let (x, y) = rel_adjust(t, mx, my, width, height);
        // Feed the evaluated coordinates back into the env (#249):
        // later rows reference earlier ones section-qualified
        // (`Geometry1.X2`), and the stale cached V would keep the
        // whole chain at the authored pose.
        env.insert(format!("{}.X{}", gname, row.ix), x);
        env.insert(format!("{}.Y{}", gname, row.ix), y);
        match t {
            "MoveTo" | "RelMoveTo" => {
                let _ = write!(d, "M {}", emit_pt((x, y)));
                cur = (x, y);
            }
            "LineTo" | "RelLineTo" => {
                let _ = write!(d, "L {}", emit_pt((x, y)));
                cur = (x, y);
            }
            "ArcTo" => {
                // Circular arc: |A| = sagitta (height of arc above the
                // chord). Positive A bulges RIGHT of the travel direction —
                // verified against the Process master's rounded rect, which
                // traverses CCW (y-up, interior on the left) with positive-A
                // corner arcs bulging outward.
                let h = row_num(env, row, "A").unwrap_or(0.0);
                let (sx, sy) = cur;
                let chord = ((x - sx).powi(2) + (y - sy).powi(2)).sqrt();
                if h.abs() < 1e-9 || chord < 1e-9 {
                    let _ = write!(d, "L {}", emit_pt((x, y)));
                } else {
                    // On-arc point from the sagitta.
                    let mid = ((sx + x) / 2.0, (sy + y) / 2.0);
                    let dir = ((x - sx) / chord, (y - sy) / chord);
                    let right = (dir.1, -dir.0); // rot90 CW: right of travel
                    let m = (mid.0 + h * right.0, mid.1 + h * right.1);
                    let radius = (chord.powi(2) + 4.0 * h * h) / (8.0 * h.abs());
                    let large = h.abs() > chord / 2.0;
                    // cross > 0 (y-up frame) = CCW; path frame is mirrored → sweep 0.
                    let cross = (m.0 - sx) * (y - m.1) - (m.1 - sy) * (x - m.0);
                    let sweep = if cross > 0.0 { 0 } else { 1 };
                    let r = sc((radius, radius)).0.max(1e-6);
                    let e = emit_pt((x, y));
                    let _ = write!(
                        d,
                        "A {r:.5} {r:.5} 0 {la} {sw} {e}",
                        la = large as u8,
                        sw = sweep,
                        e = e.trim_end()
                    );
                }
                cur = (x, y);
            }
            "EllipticalArcTo" | "RelEllipticalArcTo" => {
                // (Authored unit codes for future writers: D carries
                // unit 32, C the angle unit 81 — MisterDiagram #233
                // rider; we only read today, preserve-bytes covers
                // round-trips.)
                let m = (
                    row_num(env, row, "A").unwrap_or((cur.0 + x) / 2.0),
                    row_num(env, row, "B").unwrap_or((cur.1 + y) / 2.0),
                );
                let c_ang = row_num(env, row, "C").unwrap_or(0.0);
                let d_cell = row_num(env, row, "D");
                // #233: an authored D=0 is a ZERO-WIDTH arc — the
                // minor axis collapses onto the major, so the arc IS
                // the endpoint chord. Visio draws the chord even when
                // the A/B control sits off it (MisterDiagram M5.50);
                // the old fallback's quadratic-through-m doubled back.
                // Corpus-common: 887 masters / 18,860 visible rows.
                if d_cell.is_some_and(|d| d.abs() < 1e-9) {
                    let _ = write!(d, "L {}", emit_pt((x, y)));
                    cur = (x, y);
                    continue;
                }
                let ratio = d_cell.unwrap_or(1.0).abs().max(1e-6);
                // Sane bounds: the arc must fit (loosely) inside the
                // shape's scaled extent — a solved ellipse or on-arc point
                // far outside means the numeric solver diverged on a tiny
                // or near-degenerate arc; fall back to the quadratic.
                let (ex, ey) = (x * scale.0, y * scale.1);
                let (mx, my) = (m.0 * scale.0, m.1 * scale.1);
                let bound_x = 3.0 * (width * scale.0).max(1e-6);
                let bound_y = 3.0 * (height * scale.1).max(1e-6);
                let in_bounds = |v: f64, b: f64| v.abs() <= b;
                match solve_ellipse_arc(cur, m, (x, y), c_ang, ratio, scale) {
                    Some((rx, ry, rot_deg, large, sweep))
                        if rx.max(ry)
                            <= 20.0
                                * ((x - cur.0).powi(2) + (y - cur.1).powi(2)).sqrt().max(1e-9)
                            && in_bounds(ex, bound_x)
                            && in_bounds(ey, bound_y)
                            && in_bounds(mx, bound_x)
                            && in_bounds(my, bound_y)
                            && rx.max(ry) <= bound_x.max(bound_y) =>
                    {
                        let e = emit_pt((x, y));
                        let _ = write!(
                            d,
                            "A {rx:.5} {ry:.5} {rot_deg:.4} {la} {sw} {e}",
                            la = large as u8,
                            sw = sweep,
                            e = e.trim_end()
                        );
                    }
                    _ => {
                        // The solver diverged on a genuinely 2-D arc
                        // (tiny or wild): quadratic Bézier through m.
                        // Authored zero-width (D=0) never reaches here
                        // — it short-circuits to the chord (#233).
                        let q = emit_pt(m);
                        let e = emit_pt((x, y));
                        let _ = write!(d, "Q {} {}", q.trim_end(), e.trim_end());
                    }
                }
                cur = (x, y);
            }
            "Ellipse" => {
                // (X,Y) center, (A,B) major-axis endpoint, (C,D) minor-axis endpoint.
                let a = row_num(env, row, "A").unwrap_or(x + 1.0);
                let b = row_num(env, row, "B").unwrap_or(y);
                let c2 = row_num(env, row, "C").unwrap_or(x);
                let d2 = row_num(env, row, "D").unwrap_or(y + 1.0);
                let rx = ((a - x).powi(2) + (b - y).powi(2)).sqrt().max(1e-6);
                let ry = ((c2 - x).powi(2) + (d2 - y).powi(2)).sqrt().max(1e-6);
                let ang = (b - y).atan2(a - x); // major-axis angle, y-up frame
                                                // Anisotropic instance scaling of the ellipse.
                let (rx2, ry2, ang2) =
                    transform_ellipse(rx.max(ry), (rx.min(ry)) / rx.max(ry), ang, scale);
                let (rx2, ry2) = if rx >= ry { (rx2, ry2) } else { (ry2, rx2) };
                let p = sc((x, y));
                // A standalone full-ellipse row keeps its own element
                // (fill + stroke) — it does not join the even-odd union
                // (#228 scope: the witness covers path sections; an
                // ellipse row overlapping path sections is unwitnessed).
                let _ = writeln!(
                    paint.strokes,
                    r#"<ellipse cx="{cx:.5}" cy="{cy:.5}" rx="{rx:.5}" ry="{ry:.5}" transform="rotate({rot:.4} {cx:.5} {cy:.5})" {style}/>"#,
                    cx = p.0,
                    cy = -p.1,
                    rx = rx2.max(1e-6),
                    ry = ry2.max(1e-6),
                    rot = -ang2.to_degrees(), // mirror frame
                    style = style,
                );
                cur = (x, y);
            }
            "CubicBezTo" | "RelCubBezTo" => {
                // Controls (A,B) and (C,D); end (X,Y) — all possibly
                // relative fractions of Width/Height.
                let (a, b) = rel_adjust(
                    t,
                    row_num(env, row, "A").unwrap_or(cur.0),
                    row_num(env, row, "B").unwrap_or(cur.1),
                    width,
                    height,
                );
                let (c2, d2) = rel_adjust(
                    t,
                    row_num(env, row, "C").unwrap_or(x),
                    row_num(env, row, "D").unwrap_or(y),
                    width,
                    height,
                );
                let _ = write!(
                    d,
                    "C {}{}{}",
                    emit_pt((a, b)),
                    emit_pt((c2, d2)),
                    emit_pt((x, y)).trim_end()
                );
                cur = (x, y);
            }
            "QuadBezTo" | "RelQuadBezTo" => {
                let (a, b) = rel_adjust(
                    t,
                    row_num(env, row, "A").unwrap_or((cur.0 + x) / 2.0),
                    row_num(env, row, "B").unwrap_or((cur.1 + y) / 2.0),
                    width,
                    height,
                );
                let _ = write!(
                    d,
                    "Q {} {}",
                    emit_pt((a, b)).trim_end(),
                    emit_pt((x, y)).trim_end()
                );
                cur = (x, y);
            }
            "SplineStart" => {
                // #188: a SplineStart opens a spline (§2.4.2.15): X/Y
                // is the SECOND control point (the previous row's end
                // is the first); A/B/C/D are second/first/last knots
                // and the degree. Any non-SplineKnot row flushes it
                // (the loop-top flush).
                spline_acc = Some(crate::curve::SplineAcc::start(
                    cur,
                    (x, y),
                    row_num(env, row, "A").unwrap_or(0.0),
                    row_num(env, row, "B").unwrap_or(0.0),
                    row_num(env, row, "C").unwrap_or(1.0),
                    row_num(env, row, "D").unwrap_or(3.0),
                ));
                cur = (x, y);
            }
            "SplineKnot" => {
                match spline_acc.as_mut() {
                    Some(acc) => {
                        acc.knot((x, y), row_num(env, row, "A").unwrap_or(0.0));
                        cur = (x, y);
                    }
                    None => {
                        // A knot row without an opener degenerates to
                        // the chord (the v0.2 behavior).
                        let _ = write!(d, "L {}", emit_pt((x, y)));
                        cur = (x, y);
                    }
                }
            }
            "NURBSTo" => {
                // #188: one self-contained rational B-spline row
                // (§2.4.2.7). The E cell's NURBS() formula carries the
                // interior points/knots/weights; the row cells frame
                // them (C/D first knot/weight, A/B second-to-last
                // knot/last weight, X/Y the endpoint). The loop-top
                // flush already ended any open spline.
                let e_text = row
                    .cell("E")
                    .and_then(|c| c.v.clone().or_else(|| c.f.clone()))
                    .unwrap_or_default();
                let mut drew = false;
                if let Some(f) = crate::curve::parse_nurbs(&e_text) {
                    // Fractions are of the GEOMETRY frame's own size:
                    // the master's dims in the master frame (`width/
                    // scale` recovers them; `emit_pt` applies the
                    // instance scale exactly once), the instance's
                    // own dims inline (scale = 1). Scaling by the
                    // instance dims here too would square the stretch
                    // (#218 review round).
                    let fx = if f.x_type == 0.0 {
                        width / scale.0.max(1e-12)
                    } else {
                        1.0
                    };
                    let fy = if f.y_type == 0.0 {
                        height / scale.1.max(1e-12)
                    } else {
                        1.0
                    };
                    let mut points = vec![cur];
                    let mut knots = vec![row_num(env, row, "C").unwrap_or(0.0)];
                    let mut weights = vec![row_num(env, row, "D").unwrap_or(1.0)];
                    for (gx, gy, gk, gw) in &f.points {
                        points.push((gx * fx, gy * fy));
                        knots.push(*gk);
                        weights.push(*gw);
                    }
                    knots.push(row_num(env, row, "A").unwrap_or(0.0));
                    knots.push(f.last_knot);
                    weights.push(row_num(env, row, "B").unwrap_or(1.0));
                    points.push((x, y));
                    let built = crate::curve::SplineData {
                        degree: f.degree,
                        points,
                        weights,
                        knots,
                    }
                    .sanitized();
                    if let Some(data) = built {
                        let pts = data.flatten(16);
                        for p in pts.iter().skip(1) {
                            let _ = write!(d, "L {}", emit_pt(*p));
                        }
                        if let Some(last) = pts.last() {
                            cur = *last;
                        }
                        drew = true;
                    }
                }
                if !drew {
                    let _ = write!(d, "L {}", emit_pt((x, y)));
                    cur = (x, y);
                }
            }
            "PolylineTo" | "InfiniteLine" => {
                // v0.2: approximate polylines/infinite lines with chords.
                let _ = write!(d, "L {}", emit_pt((x, y)));
                cur = (x, y);
            }
            _ => {}
        }
    }
    // A spline open through the section's end flushes here.
    flush_spline_tail!();
    if !d.is_empty() {
        // Connector arrowheads: BeginArrow/EndArrow cells (0 = none, ≥1 an
        // arrow; sizes 0–6, 2 = medium).
        let begin_arrow = doc
            .resolve_cell(s, chain, "BeginArrow")
            .and_then(|c| cell_num(&c, env))
            .unwrap_or(0.0);
        let end_arrow = doc
            .resolve_cell(s, chain, "EndArrow")
            .and_then(|c| cell_num(&c, env))
            .unwrap_or(0.0);
        let bsz = doc
            .resolve_cell(s, chain, "BeginArrowSize")
            .and_then(|c| cell_num(&c, env))
            .unwrap_or(2.0);
        let esz = doc
            .resolve_cell(s, chain, "EndArrowSize")
            .and_then(|c| cell_num(&c, env))
            .unwrap_or(2.0);
        // #185/#170: physical per-value arrow geometry. The head unit is
        // LineWeight + C[EndArrowSize] (C = [1.44, 1.8, 2.52, 3.24, 3.96,
        // 9, 18] pt — witnessed across all 45 types and sizes 0–6), each
        // gallery kind draws at its measured proportions in that unit
        // ([`GALLERY`]), and the shaft trims back by the per-kind
        // setback ([`arrow_setback`]) so hollow heads keep their
        // interiors clear — the open diamonds trim two pens short of
        // their rear vertex ([`diamond_shaft_trim`], #239). Evidence:
        // the owner-authored 600 dpi print (Sample/test files/
        // arrows.vsdx + arrows.png — the authoritative oracle)
        // cross-checked against the #223 parity witnesses; see
        // Analysis/arrow-gallery-visio2016.md.
        let kind = |a: f64| -> Option<i64> {
            if a < 0.5 || (46.0..=50.5).contains(&a) {
                None
            } else {
                Some(a.round() as i64)
            }
        };
        // Zero-weight exception (the SVG rule): the head draws with the
        // 0.75 pt nominal pen, so its unit is C[size] + 0.75 pt.
        let head_weight = if line_weight > 0.0 {
            line_weight
        } else {
            0.75 / 72.0
        };
        let head_unit = |sz: f64| -> f64 {
            let i = (sz.round() as i64).clamp(0, 6) as usize;
            head_weight + ARROW_SIZE_PT[i] / 72.0
        };
        let end_setback = kind(end_arrow).map_or(0.0, |k| {
            diamond_shaft_trim(k, arrow_setback(k) * head_unit(esz), head_weight)
        });
        let begin_setback = kind(begin_arrow).map_or(0.0, |k| {
            arrow_begin_setback(diamond_shaft_trim(
                k,
                arrow_setback(k) * head_unit(bsz),
                head_weight,
            ))
        });
        // The shaft stops at (or under) the head's rear; the heads
        // themselves anchor at the ORIGINAL endpoints, so the trim must
        // not move them. Closed loops and multi-subpath data trim
        // never (Visio gives them no heads either — #227 tracks that).
        let shaft = if end_setback > 0.0 || begin_setback > 0.0 {
            trim_path(&d, begin_setback, end_setback)
        } else {
            d.clone()
        };
        // #228: a fill-eligible section's `d` (UNtrimmed — setbacks
        // trim the shaft only) joins the shape's ONE alternate-fill
        // path; NoFill sections opt out here, NoLine changes nothing
        // for the fill, and the implicit close SVG already applies to
        // open subpaths leaves the stroke open per section.
        if !no_fill && fill != "none" {
            paint.fill_d.push_str(&d);
            paint.fill_d.push(' ');
            paint.fill = fill.clone();
        }
        if !no_line {
            let _ = writeln!(
                paint.strokes,
                r#"<path d="{}" fill="none" {stroke_a}/>"#,
                escape_attr(shaft.trim())
            );
        }
        // Heads as inline translate·rotate·scale groups of the
        // unit-space parts (begin faces outward along the reversed
        // start tangent, end along the end tangent) — no marker defs,
        // so the size law holds without markerUnits scaling. #227: a
        // CLOSED walk gets no frames — and so no heads — at all.
        if let Some((begin, end)) = path_frames(&d) {
            if let Some(k) = kind(begin_arrow) {
                emit_arrow_head(&mut paint.strokes, k, bsz, head_weight, &line_color, begin.0, begin.1);
            }
            if let Some(k) = kind(end_arrow) {
                emit_arrow_head(&mut paint.strokes, k, esz, head_weight, &line_color, end.0, end.1);
            }
        }
    }
}

/// One drawable piece of an arrow head (#199, #185): a path — filled
/// when `filled`, stroked (fill="none") otherwise — or a circle
/// (filled dot vs open ring). Composite heads (the dimension set's
/// tick + glyph rows) are just several parts in one head.
/// Coordinates live in MARKER UNITS with the tip at the origin, +x
/// pointing PAST the tip (the body extends into -x) and +y down; the
/// head frame is placed with a translate·rotate·scale group, so the
/// same table serves both ends without mirroring.
struct HeadPart {
    d: &'static str,
    circle: Option<(f64, f64, f64)>,
    filled: bool,
}

/// A filled path part.
const fn fp(d: &'static str) -> HeadPart {
    HeadPart {
        d,
        circle: None,
        filled: true,
    }
}

/// A stroked (open) path part.
const fn sp(d: &'static str) -> HeadPart {
    HeadPart {
        d,
        circle: None,
        filled: false,
    }
}

/// An open ring (stroke-only circle).
const fn ring(cx: f64, cy: f64, r: f64) -> HeadPart {
    HeadPart {
        d: "",
        circle: Some((cx, cy, r)),
        filled: false,
    }
}

/// A filled dot (circle).
const fn dot(cx: f64, cy: f64, r: f64) -> HeadPart {
    HeadPart {
        d: "",
        circle: Some((cx, cy, r)),
        filled: true,
    }
}

/// The Visio 2016 Begin/EndArrow gallery in unit space (#185): every
/// kind at its measured proportions in the head unit
/// LineWeight + C[EndArrowSize]. Pinned against the owner-authored
/// 600 dpi print (`Sample/test files/arrows.png` — the fixture
/// `arrows.vsdx` carries EndArrow = n per labeled line; the print is
/// the authoritative oracle) and cross-checked against the #223
/// parity witnesses; artwork ours, no Microsoft assets.
///
/// Geometry conventions: the tip sits at the origin, the body
/// extends into -x, +y is down. Per-kind proportions are the
/// measured ones — triangles 2 units tall with per-kind lengths
/// (2/15: 1, 3/4/16: 2, 12/13/14: 3), ticks/bars 1.5 tall, circles
/// r 0.75 (the 35–37 dots 0.825), the diamond 4.5 × 2.25 — and the
/// 5/6/8/18 rears are circular arcs (concave for 5/8, convex —
/// centered at the tip, R = √5 — for 6/18) emitted as exact cubic
/// approximations (k = 4/3·(√5−2)). Corrections the print forced
/// against the earlier table: 12 is ONE wide open V (len 3) and 43
/// is the double chevron (len 4); 10/11/20/21 straddle the tip
/// centered; 9's tick leans "/" through the tip while 23's leans
/// "\" behind it.
const GALLERY: &[(i64, &[HeadPart])] = &[
    // 1 — narrow open V (unfilled 2-prong), len 1.
    (1, &[sp("M-1,-1 L0,0 L-1,1")]),
    // 2 — small filled triangle, len 1.
    (2, &[fp("M0,0 L-1,-1 L-1,1 Z")]),
    // 3 — open wide V, len 2.
    (3, &[sp("M-2,-1 L0,0 L-2,1")]),
    // 4 — the classic filled triangle (the corpus default), len 2.
    (4, &[fp("M0,0 L-2,-1 L-2,1 Z")]),
    // 5 — filled triangle, concave circular rear (the stealth-dart
    // family; the native SVG alias 8→5 and the print agree).
    (
        5,
        &[fp("M0,0 L-2,-1 C-1.68524,-0.37049 -1.68524,0.37049 -2,1 Z")],
    ),
    // 6 — filled triangle with a convex rear arc centered at the
    // tip: reads "long" next to 4's flat base.
    (
        6,
        &[fp("M0,0 L-2,-1 C-2.31476,-0.37049 -2.31476,0.37049 -2,1 Z")],
    ),
    // 7 — open fork (curved barbs).
    (
        7,
        &[sp("M-2,-1 C-1.5,-0.3 -0.7,0 0,0 C-0.7,0 -1.5,0.3 -2,1")],
    ),
    // 8 — the stealth dart: the concave-rear dart (the 5 glyph).
    (
        8,
        &[fp("M0,0 L-2,-1 C-1.68524,-0.37049 -1.68524,0.37049 -2,1 Z")],
    ),
    // 9 — the slanted dimension tick: 45° through the tip ("/").
    (9, &[sp("M1,-1 L-1,1")]),
    // 10 — the filled circle, centered ON the endpoint.
    (10, &[dot(0.0, 0.0, 0.75)]),
    // 11 — the filled square, centered ON the endpoint.
    (
        11,
        &[fp("M-0.75,-0.75 L0.75,-0.75 L0.75,0.75 L-0.75,0.75 Z")],
    ),
    // 12 — ONE wide open V (len 3; the old "double chevron" label
    // was a print misread).
    (12, &[sp("M-3,-1 L0,0 L-3,1")]),
    // 13 — single filled triangle, plain base, len 3 (NOT a double —
    // the #169 report).
    (13, &[fp("M0,0 L-3,-1 L-3,1 Z")]),
    // 14–18 — the open-head set: CLOSED triangle outlines whose
    // interiors stay hollow (the shaft trims at the base).
    (14, &[sp("M0,0 L-3,-1 L-3,1 Z")]),
    (15, &[sp("M0,0 L-1,-1 L-1,1 Z")]),
    (16, &[sp("M0,0 L-2,-1 L-2,1 Z")]),
    // 17 — open triangle, concave rear (cubic through ±0.33).
    (17, &[sp("M0,0 L-2,-1 C-1.55,-0.33 -1.55,0.33 -2,1 Z")]),
    // 18 — open triangle, convex rear arc (centered at the tip).
    (
        18,
        &[sp("M0,0 L-2,-1 C-2.31476,-0.37049 -2.31476,0.37049 -2,1 Z")],
    ),
    // 19 — open barbed head: curved barbs closed by a concave rear.
    (
        19,
        &[sp(
            "M-2,-1 C-1.5,-0.3 -0.7,0 0,0 C-0.7,0 -1.5,0.3 -2,1 C-1.55,0.3 -1.55,-0.3 -2,-1 Z",
        )],
    ),
    // 20 — the open ring, centered ON the endpoint.
    (20, &[ring(0.0, 0.0, 0.75)]),
    // 21 — the open square, centered ON the endpoint.
    (
        21,
        &[sp("M-0.75,-0.75 L0.75,-0.75 L0.75,0.75 L-0.75,0.75 Z")],
    ),
    // 22 — the open diamond, 4.5 × 2.25.
    (22, &[sp("M0,0 L-2.25,-1.125 L-4.5,0 L-2.25,1.125 Z")]),
    // 23 — single slanted tick: 45° behind the tip ("\"), shaft
    // through.
    (23, &[sp("M-2.9,-1 L-0.9,1")]),
    // 24–26 — perpendicular ticks at 1.5 + 0.75·i behind the tip.
    (24, &[sp("M-1.5,-0.75 L-1.5,0.75")]),
    (25, &[sp("M-1.5,-0.75 L-1.5,0.75 M-2.25,-0.75 L-2.25,0.75")]),
    (
        26,
        &[sp(
            "M-1.5,-0.75 L-1.5,0.75 M-2.25,-0.75 L-2.25,0.75 M-3,-0.75 L-3,0.75",
        )],
    ),
    // 27 — the three-pronged open fork (crow's foot).
    (27, &[sp("M0,-0.75 L-1.5,0 L0,0.75")]),
    // 28 — crow's-foot fork + perpendicular tick behind it.
    (
        28,
        &[sp("M0,-0.75 L-1.5,0 L0,0.75"), sp("M-2,-0.75 L-2,0.75")],
    ),
    // 29 — crow's-foot fork, then the open ring at 2.25.
    (
        29,
        &[sp("M0,-0.75 L-1.5,0 L0,0.75"), ring(-2.25, 0.0, 0.75)],
    ),
    // 30 — open ring at 2.25, then a tick toward the tip.
    (
        30,
        &[ring(-2.25, 0.0, 0.75), sp("M-0.75,-0.75 L-0.75,0.75")],
    ),
    // 31–33 — 1–3 ticks behind an open ring at the tip.
    (
        31,
        &[sp("M-2.25,-0.75 L-2.25,0.75"), ring(-0.75, 0.0, 0.75)],
    ),
    (
        32,
        &[
            sp("M-2.25,-0.75 L-2.25,0.75 M-3,-0.75 L-3,0.75"),
            ring(-0.75, 0.0, 0.75),
        ],
    ),
    (
        33,
        &[
            sp("M-2.25,-0.75 L-2.25,0.75 M-3,-0.75 L-3,0.75 M-3.75,-0.75 L-3.75,0.75"),
            ring(-0.75, 0.0, 0.75),
        ],
    ),
    // 34 — open diamond (1.5–6) + open ring at the tip.
    (
        34,
        &[
            ring(-0.75, 0.0, 0.75),
            sp("M-1.5,0 L-3.75,-1.125 L-6,0 L-3.75,1.125 Z"),
        ],
    ),
    // 35–37 — 1–3 ticks behind a filled 0.825-radius dot at the tip.
    (
        35,
        &[dot(-0.75, 0.0, 0.825), sp("M-2.25,-0.75 L-2.25,0.75")],
    ),
    (
        36,
        &[
            dot(-0.75, 0.0, 0.825),
            sp("M-2.25,-0.75 L-2.25,0.75 M-3,-0.75 L-3,0.75"),
        ],
    ),
    (
        37,
        &[
            dot(-0.75, 0.0, 0.825),
            sp("M-2.25,-0.75 L-2.25,0.75 M-3,-0.75 L-3,0.75 M-3.75,-0.75 L-3.75,0.75"),
        ],
    ),
    // 38 — OPEN diamond (1.5–6) + the filled dot. #239's fixture pass:
    // the 600 dpi print's center-line runs a gap through the diamond
    // interior (no fill between the apex and the shaft stub) — the
    // earlier "filled" gallery read was the low-res screenshot's.
    (
        38,
        &[
            dot(-0.75, 0.0, 0.825),
            sp("M-1.5,0 L-3.75,-1.125 L-6,0 L-3.75,1.125 Z"),
        ],
    ),
    // 39 — double filled triangle (two solid heads).
    (39, &[fp("M0,0 L-2,-1 L-2,1 Z M-2,0 L-4,-1 L-4,1 Z")]),
    // 40 — double open triangle (two hollow heads).
    (40, &[sp("M0,0 L-2,-1 L-2,1 Z M-2,0 L-4,-1 L-4,1 Z")]),
    // 41 — the open ring (front edge at the tip).
    (41, &[ring(-0.75, 0.0, 0.75)]),
    // 42 — the filled dot (front edge at the tip).
    (42, &[dot(-0.75, 0.0, 0.75)]),
    // 43 — the open double chevron (two 2-unit V's, len 4; each V is
    // corner→apex→corner so no base edge closes it).
    (43, &[sp("M-2,-1 L0,0 L-2,1 M-4,-1 L-2,0 L-4,1")]),
    // 44 — perpendicular tick behind a single open V.
    (44, &[sp("M-2,-1 L0,0 L-2,1"), sp("M-3,-0.75 L-3,0.75")]),
    // 45 — perpendicular tick behind the double chevron.
    (
        45,
        &[
            sp("M-2,-1 L0,0 L-2,1 M-4,-1 L-2,0 L-4,1"),
            sp("M-5,-0.75 L-5,0.75"),
        ],
    ),
];

/// The unmapped-value fallback: the classic filled triangle.
const FALLBACK_TRIANGLE: &[HeadPart] = &[fp("M0,0 L-2,-1 L-2,1 Z")];

/// The gallery lookup: `None` = no head (values 46–50 print bare
/// line ends); unmapped values fall back to the filled triangle.
fn arrow_spec(kind: i64) -> Option<&'static [HeadPart]> {
    if (46..=50).contains(&kind) {
        return None;
    }
    Some(
        GALLERY
            .iter()
            .find(|(k, _)| *k == kind)
            .map_or(FALLBACK_TRIANGLE, |(_, parts)| *parts),
    )
}

/// EndArrowSize 0–6, in points: the physical head unit is
/// LineWeight + C[size] — witnessed across all 45 head types and
/// sizes 0–6, independent of export scale, group resize, drawing
/// scale and dash pattern (#185).
const ARROW_SIZE_PT: [f64; 7] = [1.44, 1.8, 2.52, 3.24, 3.96, 9.0, 18.0];

/// The gallery's largest head reach in units (the 34/38 diamond's
/// rear vertex; kind 9 reaches 1 unit forward and the diamonds 1.125
/// sideways, both inside this square). The conservative bound the
/// ink-box walk pads around arrowed endpoints.
const ARROW_MAX_REACH_UNITS: f64 = 6.0;

/// The physical half-extent an arrowhead reaches beyond a line
/// endpoint: `reach × (LineWeight + C[size])` with the zero-weight
/// SVG floor — the same unit law [`emit_arrow_head`] draws with, so
/// the ink-box padding never clips a head.
fn arrow_head_reach(weight: f64, sz: f64) -> f64 {
    let head_weight = if weight > 0.0 { weight } else { 0.75 / 72.0 };
    let i = (sz.round() as i64).clamp(0, 6) as usize;
    ARROW_MAX_REACH_UNITS * (head_weight + ARROW_SIZE_PT[i] / 72.0)
}

/// Per-kind shaft setback in head units (#185): how far back from the
/// line end the shaft stops. Closed/hollow heads trim to their rear
/// (the measured values ARE the glyph rear extents); pass-through
/// heads — the open V/barbs (1, 3, 7), the ticks (9, 23–26), the
/// crow's-foot set (27/28), the chevrons (12, 43–45) — ride 0 and the
/// shaft runs beneath them to the tip. Filled heads trim under their
/// own fill, so the value only has to reach covered ground.
fn arrow_setback(kind: i64) -> f64 {
    match kind {
        2 | 15 => 1.0,
        4 | 16 => 2.0,
        5 | 8 | 17 => 1.75,
        6 | 18 => 2.25,
        10 | 11 | 20 | 21 | 42 => 0.75,
        13 | 14 | 29 | 30 => 3.0,
        19 => 1.65,
        22 => 4.5,
        31..=33 | 35..=37 | 41 => 1.5,
        34 | 38 => 6.0,
        39 | 40 => 4.0,
        _ => 0.0,
    }
}

/// The BEGIN head's setback: the end setback minus max(1% of it,
/// 0.36 pt) — Visio's begin end trims marginally less — floored at
/// zero (#185).
fn arrow_begin_setback(end_setback: f64) -> f64 {
    (end_setback - (end_setback * 0.01).max(0.36 / 72.0)).max(0.0)
}

/// #239: the open diamonds (22, 34/38) trim their shaft TWO PENS short
/// of the rear vertex, not at it. The 600 dpi fixture's ink shows the
/// stop where the shaft's ink would first touch the outline's ink —
/// the outline center one full pen off-axis, which the slope-½ rear
/// halves doubles: `rear − 2 × pen` (22's on-axis ink ends ≈3.79 u of
/// its 4.5 rear; 34/38's ≈5.45 of 6 — the model puts the geometric
/// stops at 3.77/5.27 and the butt-cap/AA residuals land inside
/// ±0.2 u; the flat-rear hollow heads keep the full extent because
/// their trim hides under the rear stroke). Floored at half the
/// setback — never in front of the diamond's widest point, so a fat
/// pen cannot push the stop into the wide half.
fn diamond_shaft_trim(kind: i64, setback: f64, head_weight: f64) -> f64 {
    if matches!(kind, 22 | 34 | 38) {
        (setback - 2.0 * head_weight).max(0.5 * setback)
    } else {
        setback
    }
}

fn path_command_arity(c: char) -> Option<usize> {
    match c {
        'M' | 'L' => Some(2),
        'Q' => Some(4),
        'C' => Some(6),
        'A' => Some(7),
        _ => None,
    }
}

/// Parse emitted path data into (letter, floats) commands; None when
/// anything deviates from the emitter's strict absolute M/L/Q/C/A
/// shape (the caller keeps the untrimmed path).
fn parse_path_commands(d: &str) -> Option<Vec<(char, Vec<f64>)>> {
    let mut cmds: Vec<(char, Vec<f64>)> = Vec::new();
    let mut cur: Option<(char, Vec<f64>)> = None;
    for tok in d.split_whitespace() {
        let mut chars = tok.chars();
        let first = chars.next().unwrap_or(' ');
        if chars.count() == 0 && first.is_ascii_alphabetic() {
            if let Some((c, f)) = cur.take() {
                if path_command_arity(c) != Some(f.len()) {
                    return None;
                }
                cmds.push((c, f));
            }
            cur = Some((first, Vec::new()));
        } else {
            cur.as_mut()?.1.push(tok.parse::<f64>().ok()?);
        }
    }
    if let Some((c, f)) = cur.take() {
        if path_command_arity(c) != Some(f.len()) {
            return None;
        }
        cmds.push((c, f));
    }
    Some(cmds)
}

/// One drawing segment: (letter, params, start, end). `M` resets the
/// current point without drawing.
type PathSeg = (char, Vec<f64>, (f64, f64), (f64, f64));

fn path_segments(cmds: &[(char, Vec<f64>)]) -> Option<Vec<PathSeg>> {
    let mut segs: Vec<PathSeg> = Vec::new();
    let mut cur: Option<(f64, f64)> = None;
    for (c, f) in cmds {
        match c {
            'M' => cur = Some((f[0], f[1])),
            'L' | 'Q' | 'C' | 'A' => {
                let s = cur?;
                let e = match c {
                    'Q' => (f[2], f[3]),
                    'C' => (f[4], f[5]),
                    'A' => (f[5], f[6]),
                    _ => (f[0], f[1]),
                };
                segs.push((*c, f.clone(), s, e));
                cur = Some(e);
            }
            _ => return None,
        }
    }
    Some(segs)
}

/// An SVG arc's center-form parameters: (rx, ry, φ rad, θ1, Δθ,
/// center). `s` is the arc's start point, `f` the full A parameters.
type ArcGeo = (f64, f64, f64, f64, f64, (f64, f64));

fn arc_geo(f: &[f64], s: (f64, f64)) -> Option<ArcGeo> {
    let (mut rx, mut ry) = (f[0], f[1]);
    if rx <= 0.0 || ry <= 0.0 {
        return None;
    }
    let phi = f[2].to_radians();
    let (sp, cp) = phi.sin_cos();
    let e = (f[5], f[6]);
    let (dx2, dy2) = ((s.0 - e.0) / 2.0, (s.1 - e.1) / 2.0);
    let x1 = cp * dx2 + sp * dy2;
    let y1 = -sp * dx2 + cp * dy2;
    let lam = x1 * x1 / (rx * rx) + y1 * y1 / (ry * ry);
    if lam > 1.0 {
        let r = lam.sqrt();
        rx *= r;
        ry *= r;
    }
    let den = rx * rx * y1 * y1 + ry * ry * x1 * x1;
    if den <= 1e-18 {
        return None; // coincident endpoints
    }
    let num = rx * rx * ry * ry - den;
    let sign = if f[3] != f[4] { 1.0 } else { -1.0 };
    let co = sign * (num / den).max(0.0).sqrt();
    let (cxp, cyp) = (co * rx * y1 / ry, -co * ry * x1 / rx);
    let center = (
        cp * cxp - sp * cyp + (s.0 + e.0) / 2.0,
        sp * cxp + cp * cyp + (s.1 + e.1) / 2.0,
    );
    let th1 = ((y1 - cyp) / ry).atan2((x1 - cxp) / rx);
    let mut dth = ((-y1 - cyp) / ry).atan2((-x1 - cxp) / rx) - th1;
    if f[4] == 0.0 && dth > 0.0 {
        dth -= std::f64::consts::TAU;
    }
    if f[4] != 0.0 && dth < 0.0 {
        dth += std::f64::consts::TAU;
    }
    Some((rx, ry, phi, th1, dth, center))
}

/// The point and tangent direction at parameter t ∈ [0, 1] of an arc.
fn arc_point_tangent(g: &ArcGeo, t: f64) -> ((f64, f64), (f64, f64)) {
    let (rx, ry, phi, th1, dth, c) = *g;
    let th = th1 + t * dth;
    let (st, ct) = th.sin_cos();
    let (sp, cp) = phi.sin_cos();
    let p = (
        c.0 + cp * rx * ct - sp * ry * st,
        c.1 + sp * rx * ct + cp * ry * st,
    );
    let dir = (-cp * rx * st - sp * ry * ct, -sp * rx * st + cp * ry * ct);
    let dir = if dth < 0.0 { (-dir.0, -dir.1) } else { dir };
    (p, dir)
}

/// Chord length of a sample polyline.
fn polyline_length(samples: &[(f64, f64)]) -> f64 {
    samples
        .windows(2)
        .map(|w| ((w[1].0 - w[0].0).powi(2) + (w[1].1 - w[0].1).powi(2)).sqrt())
        .sum()
}

/// Polyline approximation of one segment (16 chords) for arc-length
/// walks.
fn segment_samples(c: char, f: &[f64], s: (f64, f64)) -> Vec<(f64, f64)> {
    match c {
        'L' => vec![s, (f[0], f[1])],
        'Q' => (0..=16)
            .map(|i| {
                let t = i as f64 / 16.0;
                let mt = 1.0 - t;
                (
                    mt * mt * s.0 + 2.0 * mt * t * f[0] + t * t * f[2],
                    mt * mt * s.1 + 2.0 * mt * t * f[1] + t * t * f[3],
                )
            })
            .collect(),
        'C' => (0..=16)
            .map(|i| {
                let t = i as f64 / 16.0;
                let mt = 1.0 - t;
                (
                    mt * mt * mt * s.0
                        + 3.0 * mt * mt * t * f[0]
                        + 3.0 * mt * t * t * f[2]
                        + t * t * t * f[4],
                    mt * mt * mt * s.1
                        + 3.0 * mt * mt * t * f[1]
                        + 3.0 * mt * t * t * f[3]
                        + t * t * t * f[5],
                )
            })
            .collect(),
        'A' => arc_geo(f, s)
            .map(|g| {
                (0..=16)
                    .map(|i| arc_point_tangent(&g, i as f64 / 16.0).0)
                    .collect()
            })
            .unwrap_or_else(|| vec![s, (f[5], f[6])]),
        _ => vec![s, s],
    }
}

/// Parameter t for the arc-length `target` along a segment's sample
/// polyline (exact for lines, sub-chord for curves).
fn sample_time_at_length(samples: &[(f64, f64)], target: f64) -> f64 {
    let n = samples.len().saturating_sub(1) as f64;
    if n <= 0.0 {
        return 0.0;
    }
    let mut acc = 0.0;
    for i in 0..samples.len() - 1 {
        let (a, b) = (samples[i], samples[i + 1]);
        let l = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
        if l <= 0.0 {
            continue;
        }
        if acc + l >= target {
            let frac = ((target - acc) / l).clamp(0.0, 1.0);
            return (i as f64 + frac) / n;
        }
        acc += l;
    }
    1.0
}

fn quad_at(p0: (f64, f64), p1: (f64, f64), p2: (f64, f64), t: f64) -> (f64, f64) {
    let mt = 1.0 - t;
    (
        mt * mt * p0.0 + 2.0 * mt * t * p1.0 + t * t * p2.0,
        mt * mt * p0.1 + 2.0 * mt * t * p1.1 + t * t * p2.1,
    )
}

fn cubic_at(p0: (f64, f64), p1: (f64, f64), p2: (f64, f64), p3: (f64, f64), t: f64) -> (f64, f64) {
    let mt = 1.0 - t;
    (
        mt * mt * mt * p0.0 + 3.0 * mt * mt * t * p1.0 + 3.0 * mt * t * t * p2.0 + t * t * t * p3.0,
        mt * mt * mt * p0.1 + 3.0 * mt * mt * t * p1.1 + 3.0 * mt * t * t * p2.1 + t * t * t * p3.1,
    )
}

/// The point at parameter t on one segment (t=0 start, t=1 end).
fn segment_point(c: char, f: &[f64], s: (f64, f64), t: f64) -> (f64, f64) {
    match c {
        'Q' => quad_at(s, (f[0], f[1]), (f[2], f[3]), t),
        'C' => cubic_at(s, (f[0], f[1]), (f[2], f[3]), (f[4], f[5]), t),
        'A' => arc_geo(f, s).map_or(s, |g| arc_point_tangent(&g, t).0),
        _ => (s.0 + (f[0] - s.0) * t, s.1 + (f[1] - s.1) * t),
    }
}

/// Quadratic restricted to [t1, t2] → (start, control, end); the
/// control comes from B'(t1)·(t2−t1)/2.
fn quad_sub(
    p0: (f64, f64),
    p1: (f64, f64),
    p2: (f64, f64),
    t1: f64,
    t2: f64,
) -> ((f64, f64), (f64, f64), (f64, f64)) {
    let h = t2 - t1;
    let start = quad_at(p0, p1, p2, t1);
    let end = quad_at(p0, p1, p2, t2);
    let d = (
        (1.0 - t1) * (p1.0 - p0.0) + t1 * (p2.0 - p1.0),
        (1.0 - t1) * (p1.1 - p0.1) + t1 * (p2.1 - p1.1),
    );
    (start, (start.0 + d.0 * h, start.1 + d.1 * h), end)
}

/// Cubic restricted to [t1, t2] → (start, c1, c2, end); the handles
/// come from C'(t1)·(t2−t1)/3 and −C'(t2)·(t2−t1)/3.
type CubicSub = ((f64, f64), (f64, f64), (f64, f64), (f64, f64));

/// One head frame: an anchor point and its direction angle (radians).
type HeadFrame = ((f64, f64), f64);

/// Cubic restricted to [t1, t2] → (start, c1, c2, end); the handles
/// come from C'(t1)·(t2−t1)/3 and −C'(t2)·(t2−t1)/3.
fn cubic_sub(
    p0: (f64, f64),
    p1: (f64, f64),
    p2: (f64, f64),
    p3: (f64, f64),
    t1: f64,
    t2: f64,
) -> CubicSub {
    let h = t2 - t1;
    let start = cubic_at(p0, p1, p2, p3, t1);
    let end = cubic_at(p0, p1, p2, p3, t2);
    let deriv = |t: f64| {
        let mt = 1.0 - t;
        (
            mt * mt * (p1.0 - p0.0) + 2.0 * mt * t * (p2.0 - p1.0) + t * t * (p3.0 - p2.0),
            mt * mt * (p1.1 - p0.1) + 2.0 * mt * t * (p2.1 - p1.1) + t * t * (p3.1 - p2.1),
        )
    };
    let d1 = deriv(t1);
    let d2 = deriv(t2);
    (
        start,
        (start.0 + d1.0 * h, start.1 + d1.1 * h),
        (end.0 - d2.0 * h, end.1 - d2.1 * h),
        end,
    )
}

/// Trim one open contour of an emitted path by physical lengths at
/// each end (#185): `begin` off the start, `end` off the end, in user
/// units. Only a single open contour trims — closed loops and
/// multi-subpath data come back untouched (Visio gives them no
/// arrowheads either), and a cut that would consume the whole path
/// keeps the original. Curves split exactly (Blossom
/// re-parameterization for Q/C, the angular subrange for A); the cut
/// parameter comes from a 16-sample chord walk, exact for lines.
fn trim_path(d: &str, begin: f64, end: f64) -> String {
    let keep = |d: &str| d.to_string();
    let Some(cmds) = parse_path_commands(d) else {
        return keep(d);
    };
    let Some(segs) = path_segments(&cmds) else {
        return keep(d);
    };
    if segs.is_empty() || cmds.iter().filter(|(c, _)| *c == 'M').count() != 1 {
        return keep(d);
    }
    let start = segs[0].2;
    let last_end = segs[segs.len() - 1].3;
    if ((last_end.0 - start.0).powi(2) + (last_end.1 - start.1).powi(2)).sqrt() < 1e-9 {
        return keep(d); // closed loop
    }
    let polys: Vec<Vec<(f64, f64)>> = segs
        .iter()
        .map(|(c, f, s, _)| segment_samples(*c, f, *s))
        .collect();
    let lens: Vec<f64> = polys.iter().map(|p| polyline_length(p)).collect();
    let total: f64 = lens.iter().sum();
    let cut_end = total - end;
    if cut_end <= begin + 1e-9 {
        return keep(d); // nothing remains
    }
    let fmt = |p: (f64, f64)| format!("{:.5} {:.5}", p.0, p.1);
    let mut out_d = String::new();
    let mut offset = 0.0;
    for (i, (c, f, s, _)) in segs.iter().enumerate() {
        let len = lens[i];
        let seg_start = offset;
        offset += len;
        let a = begin.max(seg_start);
        let b = cut_end.min(offset);
        if b <= a + 1e-12 {
            continue;
        }
        let poly = &polys[i];
        let t1 = sample_time_at_length(poly, a - seg_start);
        let t2 = sample_time_at_length(poly, b - seg_start);
        if out_d.is_empty() {
            out_d = format!("M {}", fmt(segment_point(*c, f, *s, t1)));
        }
        match c {
            'L' => {
                out_d += &format!(
                    " L {}",
                    fmt((s.0 + (f[0] - s.0) * t2, s.1 + (f[1] - s.1) * t2))
                );
            }
            'Q' => {
                let (_, p1, p2) = quad_sub(*s, (f[0], f[1]), (f[2], f[3]), t1, t2);
                out_d += &format!(" Q {} {}", fmt(p1), fmt(p2));
            }
            'C' => {
                let (_, p1, p2, p3) =
                    cubic_sub(*s, (f[0], f[1]), (f[2], f[3]), (f[4], f[5]), t1, t2);
                out_d += &format!(" C {} {} {}", fmt(p1), fmt(p2), fmt(p3));
            }
            'A' => {
                if let Some(g) = arc_geo(f, *s) {
                    let (p, _) = arc_point_tangent(&g, t2);
                    let large = ((t2 - t1) * g.4.abs() > std::f64::consts::PI) as u8;
                    out_d += &format!(
                        " A {:.5} {:.5} {:.4} {} {} {}",
                        g.0,
                        g.1,
                        f[2],
                        large,
                        f[4] as u8,
                        fmt(p)
                    );
                }
            }
            _ => {}
        }
    }
    if out_d.is_empty() {
        return keep(d);
    }
    out_d
}

/// A segment's start tangent (the true curve tangent where the
/// segment has one).
fn seg_start_tangent(c: char, f: &[f64], s: (f64, f64)) -> Option<(f64, f64)> {
    let d = match c {
        'A' => arc_geo(f, s).map(|g| arc_point_tangent(&g, 0.0).1)?,
        _ => (f[0] - s.0, f[1] - s.1),
    };
    (d.0 * d.0 + d.1 * d.1 > 1e-18).then_some(d)
}

/// A segment's end tangent.
fn seg_end_tangent(c: char, f: &[f64], s: (f64, f64)) -> Option<(f64, f64)> {
    let d = match c {
        'L' => (f[0] - s.0, f[1] - s.1),
        'Q' => (f[2] - f[0], f[3] - f[1]),
        'C' => (f[4] - f[2], f[5] - f[3]),
        'A' => arc_geo(f, s).map(|g| arc_point_tangent(&g, 1.0).1)?,
        _ => return None,
    };
    (d.0 * d.0 + d.1 * d.1 > 1e-18).then_some(d)
}

/// The path's two head frames: the BEGIN tip with its OUTWARD
/// direction (the reversed start tangent) and the END tip with the
/// travel direction. True segment tangents where available, the
/// chord otherwise; needs two distinct points. `M`-only, malformed,
/// or CLOSED single-subpath data returns None (the caller skips the
/// heads but keeps the shaft — #227: a closed geometry draws no line
/// ends).
fn path_frames(d: &str) -> Option<(HeadFrame, HeadFrame)> {
    let cmds = parse_path_commands(d)?;
    let segs = path_segments(&cmds)?;
    let eps = 1e-12;
    let first = segs.first()?;
    let last = segs.last()?;
    // #227: a geometry whose walk closes draws no line ends whatever
    // the arrow cells say — a closed rect with EndArrow=5 gets a clean
    // corner in Visio, not an arrowhead (MisterDiagram #105 witness;
    // the cells stay in the file, only rendering is gated). Closure =
    // the single-subpath walk returning to its start (the same test
    // `trim_path` uses to refuse trimming a loop). Multi-subpath
    // sections take no heads either — the same evidence note
    // `trim_path` carries (Visio gives them no heads; CodeRabbit
    // flagged the last-subpath-closed case, #278).
    if cmds.iter().filter(|(c, _)| *c == 'M').count() != 1 {
        return None;
    }
    let start = first.2;
    let last_end = last.3;
    if ((last_end.0 - start.0).powi(2) + (last_end.1 - start.1).powi(2)).sqrt() < 1e-9 {
        return None;
    }
    let begin = seg_start_tangent(first.0, &first.1, first.2)
        .or_else(|| {
            let (dx, dy) = (first.3 .0 - first.2 .0, first.3 .1 - first.2 .1);
            (dx * dx + dy * dy > eps).then_some((dx, dy))
        })
        .map(|d| (first.2, (-d.1).atan2(-d.0)))?;
    let end = seg_end_tangent(last.0, &last.1, last.2)
        .or_else(|| {
            let (dx, dy) = (last.3 .0 - last.2 .0, last.3 .1 - last.2 .1);
            (dx * dx + dy * dy > eps).then_some((dx, dy))
        })
        .map(|d| (last.3, d.1.atan2(d.0)))?;
    Some((begin, end))
}

/// Emit one arrow head at `tip` facing along `dir` (radians; the +x
/// of the head frame): a translate·rotate·scale group of the
/// unit-space gallery parts. Open parts stroke with the head pen
/// (the line weight, or the 0.75 pt zero-weight nominal) with round
/// caps/joins like the print; filled parts fill only — the print
/// shows no pen inflation on filled glyphs (#185).
fn emit_arrow_head(
    out: &mut String,
    kind: i64,
    sz: f64,
    weight: f64,
    color: &str,
    tip: (f64, f64),
    dir: f64,
) {
    let i = (sz.round() as i64).clamp(0, 6) as usize;
    let unit = weight + ARROW_SIZE_PT[i] / 72.0;
    if !unit.is_finite() || unit <= 0.0 {
        return;
    }
    let Some(parts) = arrow_spec(kind) else {
        return;
    };
    // The group scales unit-space geometry by `unit`; open strokes
    // divide the pen back out so the ink is `weight` wide on paper.
    let pen = weight / unit;
    let color = escape_attr(color);
    let mut body = String::new();
    for part in parts {
        if let Some((cx, cy, r)) = part.circle {
            body += &if part.filled {
                format!(r#"<circle cx="{cx}" cy="{cy}" r="{r}" fill="{color}"/>"#)
            } else {
                format!(
                    r#"<circle cx="{cx}" cy="{cy}" r="{r}" fill="none" stroke="{color}" stroke-width="{pen:.5}"/>"#
                )
            };
        } else if part.filled {
            body += &format!(r#"<path d="{}" fill="{color}"/>"#, part.d);
        } else {
            body += &format!(
                r#"<path d="{}" fill="none" stroke="{color}" stroke-width="{pen:.5}" stroke-linecap="round" stroke-linejoin="round"/>"#,
                part.d
            );
        }
    }
    let _ = writeln!(
        out,
        r#"<g class="dc-ah" transform="translate({:.5} {:.5}) rotate({:.4}) scale({:.5})">{}</g>"#,
        tip.0 + 0.0, // normalize -0.0
        tip.1 + 0.0,
        dir.to_degrees() + 0.0,
        unit,
        body
    );
}
/// Tiny FNV-1a for stable marker-id hashing (no external deps).
fn fnv(s: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Solve an `EllipticalArcTo`: start `s`, on-arc point `m`, end `e` (all in
/// master-space, y-up), major-axis angle `c_ang` (radians) and axis ratio
/// `ratio` (ry/rx). Returns SVG `A` parameters: rx, ry, x-axis-rotation in
/// degrees (already mirrored for the Y-flipped path frame), large-arc and
/// sweep flags — with the endpoint scaled by `scale`.
///
/// The ellipse center satisfies, in the axis-rotated frame, the linear
/// system derived from `k²(x-ox)² + (y-oy)² = k²·rx²` for each of the three
/// points.
fn solve_ellipse_arc(
    s: (f64, f64),
    m: (f64, f64),
    e: (f64, f64),
    c_ang: f64,
    ratio: f64,
    scale: (f64, f64),
) -> Option<(f64, f64, f64, bool, u8)> {
    let k = ratio;
    let (cos_c, sin_c) = (c_ang.cos(), c_ang.sin());
    let rot =
        |p: (f64, f64)| -> (f64, f64) { (cos_c * p.0 + sin_c * p.1, -sin_c * p.0 + cos_c * p.1) };
    let ps = rot(s);
    let pm = rot(m);
    let pe = rot(e);

    // 2·k²(Px-Qx)·ox + 2(Py-Qy)·oy = k²(Px²-Qx²) + (Py²-Qy²)
    let eqn = |p: (f64, f64), q: (f64, f64)| -> ((f64, f64), f64) {
        (
            (2.0 * k * k * (p.0 - q.0), 2.0 * (p.1 - q.1)),
            k * k * (p.0 * p.0 - q.0 * q.0) + (p.1 * p.1 - q.1 * q.1),
        )
    };
    let ((a1, b1), c1) = eqn(ps, pm);
    let ((a2, b2), c2) = eqn(pm, pe);
    let det = a1 * b2 - a2 * b1;
    if det.abs() < 1e-12 {
        return None;
    }
    let ox = (c1 * b2 - c2 * b1) / det;
    let oy = (a1 * c2 - a2 * c1) / det;

    // Semi-axes: k²(Sx-ox)² + (Sy-oy)² = k²·rx²  →  rx = √(…)/k, ry = k·rx.
    let rx = ((k * k * (ps.0 - ox).powi(2) + (ps.1 - oy).powi(2)) / (k * k)).sqrt();
    if !rx.is_finite() || rx < 1e-9 {
        return None;
    }
    let ry = k * rx;

    // Traversal direction and sweep magnitude from parametric angles.
    let ang = |p: (f64, f64)| -> f64 { ((p.1 - oy) / ry).atan2((p.0 - ox) / rx) };
    let norm = |a: f64| -> f64 {
        let mut a = a;
        while a > std::f64::consts::PI {
            a -= 2.0 * std::f64::consts::PI;
        }
        while a < -std::f64::consts::PI {
            a += 2.0 * std::f64::consts::PI;
        }
        a
    };
    let t_s = ang(ps);
    let t_m = ang(pm);
    let t_e = ang(pe);
    let dir = norm(t_m - t_s).signum();
    let mut delta = norm(t_e - t_s);
    if delta * dir < 0.0 {
        delta += dir * 2.0 * std::f64::consts::PI;
    }
    let large = delta.abs() > std::f64::consts::PI;

    // Sweep flag: path frame is Y-mirrored, so CCW (y-up) → 0.
    let cross = (m.0 - s.0) * (e.1 - m.1) - (m.1 - s.1) * (e.0 - m.0);
    let sweep = if cross * dir > 0.0 { 0 } else { 1 };

    // Scale the ellipse anisotropically to the instance frame.
    let (rx2, ry2, ang2) = transform_ellipse(rx, k, c_ang, scale);
    Some((
        rx2.max(1e-6),
        ry2.max(1e-6),
        -ang2.to_degrees(),
        large,
        sweep,
    ))
}

/// Transform an ellipse (semi-major `r_major`, ratio `k = minor/major`, axis
/// angle `ang`) by an anisotropic scale `scale`. Returns the new semi-axes
/// (major, minor) and the new major-axis angle.
///
/// The image ellipse's squared semi-axes are the eigenvalues of
/// `D·R·diag(r², (k·r)²)·Rᵀ·D` with `D = diag(scale)`.
fn transform_ellipse(r_major: f64, k: f64, ang: f64, scale: (f64, f64)) -> (f64, f64, f64) {
    let (c, si) = (ang.cos(), ang.sin());
    let a = r_major * r_major;
    let b = (k * r_major) * (k * r_major);
    // Q = R·diag(a,b)·Rᵀ
    let q11 = a * c * c + b * si * si;
    let q12 = (a - b) * c * si;
    let q22 = a * si * si + b * c * c;
    // B = D·Q·D
    let (sx, sy) = scale;
    let m11 = q11 * sx * sx;
    let m12 = q12 * sx * sy;
    let m22 = q22 * sy * sy;
    // Eigenvalues of symmetric [[m11, m12], [m12, m22]].
    let tr = m11 + m22;
    let det = m11 * m22 - m12 * m12;
    let disc = ((tr * tr / 4.0) - det).max(0.0).sqrt();
    let lam1 = tr / 2.0 + disc; // major²
    let lam2 = (tr / 2.0 - disc).max(1e-12); // minor²
                                             // Eigenvector for lam1: B·(λ-m22, m12) = λ·(λ-m22, m12) for symmetric B.
    let (vx, vy) = if m12.abs() > 1e-12 {
        (lam1 - m22, m12)
    } else if m11 >= m22 {
        (1.0, 0.0)
    } else {
        (0.0, 1.0)
    };
    let mut new_ang = vy.atan2(vx);
    if new_ang > std::f64::consts::FRAC_PI_2 {
        new_ang -= std::f64::consts::PI;
    } else if new_ang < -std::f64::consts::FRAC_PI_2 {
        new_ang += std::f64::consts::PI;
    }
    (lam1.sqrt(), lam2.sqrt(), new_ang)
}

/// Rel* rows express coordinates as fractions of Width/Height.
fn rel_adjust(t: &str, x: f64, y: f64, w: f64, h: f64) -> (f64, f64) {
    if t.starts_with("Rel") {
        (x * w, y * h)
    } else {
        (x, y)
    }
}

/// The stroke half of a style attribute (#228 split): dashed strokes
/// carry `stroke-linecap="round"` — the cap the dash table's
/// ink→geometry conversion assumes (#235).
fn stroke_attr(no_line: bool, line_color: &str, line_weight: f64, dash: Option<&str>) -> String {
    if no_line {
        r#"stroke="none""#.to_string()
    } else {
        let dash = dash
            .map(|d| format!(r#" stroke-dasharray="{d}" stroke-linecap="round""#))
            .unwrap_or_default();
        // #235: an explicit-zero LineWeight stays zero — SVG paints
        // nothing at stroke-width 0, exactly the Visio behavior (a
        // fallback pen drew stray hairlines; MisterDiagram witness).
        // The old `.max(0.001)` fallback pen is gone.
        format!(
            r#"stroke="{}" stroke-width="{:.5}"{dash}"#,
            escape_attr(line_color),
            line_weight
        )
    }
}

fn style_attr(
    no_fill: bool,
    no_line: bool,
    fill: &str,
    line_color: &str,
    line_weight: f64,
    dash: Option<&str>,
) -> String {
    let fill = if no_fill {
        "none".to_string()
    } else {
        fill.to_string()
    };
    format!(
        r#"fill="{}" {}"#,
        escape_attr(&fill),
        stroke_attr(no_line, line_color, line_weight, dash)
    )
}

/// One shape's accumulated paint, #228: every FILL-ELIGIBLE Geometry
/// section's path concatenates into ONE fill path the caller emits
/// with `fill-rule="evenodd"` — Visio's alternate rule, where nesting
/// and overlap toggle coverage regardless of winding direction
/// (MisterDiagram M5.109: inner nested region clear for both
/// directions, overlap clear, third nesting filled). Per-section
/// flags gate participation: NoFill opts a section out of the fill
/// only, NoShow already skipped it entirely, NoLine leaves the fill
/// intact; open contours close implicitly for the fill (SVG fills
/// open subpaths as closed) without gaining a stroked closing edge —
/// the strokes stay per-section, in section order, OVER the fill.
struct ShapePaint {
    /// The concatenated fill subpaths (each section's `d`, which all
    /// start with `M`).
    fill_d: String,
    /// The fill style (shape-level cells — identical for every
    /// participating section; the last one computed wins).
    fill: String,
    /// Stroke-only paths, arrow shafts/heads, and standalone ellipse
    /// rows, in emission order.
    strokes: String,
}

impl Default for ShapePaint {
    fn default() -> Self {
        Self {
            fill_d: String::new(),
            fill: "none".to_string(),
            strokes: String::new(),
        }
    }
}

/// Emit the accumulated paint under the caller's ink (#228): the ONE
/// even-odd fill path first, then the per-section strokes.
fn flush_shape_paint(paint: &mut ShapePaint, out: &mut String) {
    if !paint.fill_d.is_empty() {
        let _ = writeln!(
            out,
            r#"<path fill-rule="evenodd" d="{}" fill="{}"/>"#,
            escape_attr(paint.fill_d.trim()),
            escape_attr(&paint.fill)
        );
    }
    out.push_str(&paint.strokes);
}

/// Visio's built-in dash/dot line patterns (LinePattern 2–23) as SVG
/// `stroke-dasharray` values in LineWeight multiples (patterns scale with
/// the line weight — the ShapeSheet gallery's defining property). The
/// table is MEASURED INK — run/gap lengths as painted at 600 dpi — and
/// the caller strokes it with ROUND caps, so the values convert to
/// Visio's canonical GEOMETRY before emission (#235 cap-aware split):
/// a round cap extends each mark by one weight (half at each end) and
/// eats one weight from the gap, i.e. geometry on = ink − 1w, gap =
/// ink + 1w — exactly MisterDiagram's canonical grammar (pattern 2 =
/// 7/5 where our ink reads 8/4; 14 = 9/3/3; 21 = 39/9/15/9). Dots
/// (ink ≤ 1w marks) are weight-diameter circles: geometry length 0,
/// plus a 0.0001w lead so rasterizers keep a tangent for the
/// zero-length subpath (their dot fix; the following gap gives the
/// lead back, period preserved).
fn line_dasharray(pattern: f64, weight: f64) -> Option<String> {
    // #171: the table is measured from Visio 2016 itself — the
    // owner-authored fixture `Sample/test files/lines.vsdx` (patterns
    // 0–23, 0.02 in weight) printed at 600 dpi, run/gap sequences in
    // stroke-width units (Analysis/line-patterns-visio2016.md). 0/1
    // stay None (0 hides the line entirely at the call site).
    let spec: &[f64] = match pattern as i64 {
        2 => &[8.0, 4.0],                         // dash
        3 => &[1.0, 4.0],                         // dot
        4 => &[8.0, 4.0, 1.0, 4.0],               // dash-dot
        5 => &[8.0, 4.0, 1.0, 4.0, 1.0, 4.0],     // dash-dot-dot
        6 => &[8.0, 4.0, 8.0, 4.0, 1.0, 4.0],     // dash-dash-dot
        7 => &[20.0, 4.0, 8.0, 4.0],              // long + short dash
        8 => &[20.0, 4.0, 8.0, 4.0, 8.0, 4.0],    // long + 2 short dashes
        9 => &[4.0, 2.0], // short dash (the corpus legend "long dash" label was wrong)
        10 => &[1.0, 2.0], // fine dot
        11 => &[4.0, 2.0, 1.0, 2.0], // short dash-dot
        12 => &[4.0, 2.0, 1.0, 2.0, 1.0, 2.0], // short dash-dot-dot
        13 => &[4.0, 2.0, 4.0, 2.0, 1.0, 2.0], // short dash-dash-dot
        14 => &[10.0, 2.0, 4.0, 2.0], // medium-short dash pair
        15 => &[10.0, 2.0, 4.0, 2.0, 4.0, 2.0], // medium + 2 short dashes
        16 => &[16.0, 8.0], // long dash
        17 => &[1.0, 8.0], // sparse dot
        18 => &[16.0, 8.0, 1.0, 8.0], // long dash-dot
        19 => &[16.0, 8.0, 1.0, 8.0, 1.0, 8.0], // long dash-dot-dot
        20 => &[16.0, 8.0, 16.0, 8.0, 1.0, 8.0], // long dash-dash-dot
        21 => &[40.0, 8.0, 16.0, 8.0], // extra-long + long dash
        22 => &[40.0, 8.0, 16.0, 8.0, 16.0, 8.0], // extra-long + 2 long
        23 => &[2.0, 1.0], // fine dash
        _ => return None,
    };
    // Ink → geometry under round caps (#235): marks give up one
    // weight to their caps, gaps take it back; a dot's mark becomes
    // the 0.0001w tangent lead and its gap returns the difference so
    // the period is unchanged.
    let dot_lead = 0.0001;
    let geom: Vec<f64> = spec
        .iter()
        .enumerate()
        .map(|(i, ink)| {
            if i % 2 == 0 {
                // An on-mark: a dot (ink ≤ 1w) draws as the lead
                // alone; a dash shortens by its caps.
                if *ink <= 1.0 {
                    dot_lead
                } else {
                    ink - 1.0
                }
            } else {
                // A gap: regains the mark's cap width, minus the
                // lead when the preceding mark was a dot.
                let prev_was_dot = spec[i - 1] <= 1.0;
                ink + 1.0 - if prev_was_dot { dot_lead } else { 0.0 }
            }
        })
        .collect();
    Some(
        geom.iter()
            .map(|m| {
                let v = m * weight;
                // The dot lead (0.0001w) sits far below the 5-decimal
                // rounding floor — print sub-floor values at 10
                // decimals so the lead and its gap compensation
                // survive (CodeRabbit, #278: the old `.max(1e-4)`
                // clamp inflated the lead 50× at a 0.02 weight and
                // stretched the period). Every value is positive by
                // construction, so no zero-length dash entries occur.
                if v < 1e-4 {
                    format!("{v:.10}")
                } else {
                    format!("{v:.5}")
                }
            })
            .collect::<Vec<_>>()
            .join(" "),
    )
}

/// Resolve a color-ish cell to `#rrggbb`.
/// Variant colors from visio/theme/theme1.xml (DrawingML scheme),
/// indexed by THEMEVAL("VariantColorN") formulas.
fn theme_variant_colors(doc: &Document) -> Vec<String> {
    let Some(pkg) = doc.package.as_ref() else {
        return Vec::new();
    };
    let Some(raw) = pkg.part("visio/theme/theme1.xml") else {
        return Vec::new();
    };
    let xml = String::from_utf8_lossy(raw).into_owned();
    let mut out = Vec::new();
    // <vt:varColorN><a:srgbClr val="RRGGBB"/>...</vt:varColorN>
    for n in 1..=8 {
        let tag = format!("varColor{n}");
        let Some(pos) = xml.find(&tag) else { continue };
        let window = &xml[pos..(pos + 300).min(xml.len())];
        if let Some(vpos) = window.find("srgbClr val=\"") {
            let rest = &window[vpos + 13..];
            if let Some(end) = rest.find('"') {
                if end == 6 {
                    out.push(format!("#{}", &rest[..end]));
                    continue;
                }
            }
        }
        break;
    }
    out
}

/// Resolve a color cell for rendering: literal hex / document palette,
/// or — when the cell's formula is a THEMEVAL("VariantColorN") — the
/// theme's variant color (Visio evaluates the formula, not the cached
/// palette-index V, for themed shapes).
/// One color cell's stored value as `#rrggbb` — the shared mapping
/// behind the fill, line, and TEXT color paths (#73): a `#…` hex
/// string verbatim, a palette/document-table index, or a THEMEVAL
/// variant formula. Unresolvable values fall to black.
fn color_value(doc: &Document, v: &str, f: Option<&str>) -> String {
    if let Some(f) = f {
        if f.contains("THEMEVAL") {
            for n in 1..=8 {
                if f.contains(&format!("VariantColor{n}")) {
                    let variants = theme_variant_colors(doc);
                    if let Some(col) = variants.get(n - 1) {
                        return col.clone();
                    }
                }
            }
            // THEMEVAL with no variant we model: fall through to V
        }
    }
    let v = v.trim();
    if v.starts_with('#') {
        v.to_string()
    } else {
        match v.parse::<i64>() {
            Ok(idx) => doc.color(idx).cloned().unwrap_or_else(|| palette(idx)),
            Err(_) => "#000000".into(),
        }
    }
}

pub(crate) fn themed_color_string(
    doc: &Document,
    s: &Shape,
    chain: &[&Shape],
    cell: &str,
) -> String {
    match doc.resolve_cell(s, chain, cell) {
        Some(c) => color_value(doc, c.v.as_deref().unwrap_or(""), c.f.as_deref()),
        None => palette(-1),
    }
}

/// Standard color table for indices 0–23, verbatim from [MS-VSDX]
/// "Color Table": any integer color value ≤ 23 resolves here; ≥ 24 resolves
/// through the document `Colors` table.
fn palette(idx: i64) -> String {
    match idx {
        0 => "#000000",
        1 => "#FFFFFF",
        2 => "#FF0000",
        3 => "#00FF00",
        4 => "#0000FF",
        5 => "#FFFF00",
        6 => "#FF00FF",
        7 => "#00FFFF",
        8 => "#800000",
        9 => "#008000",
        10 => "#000080",
        11 => "#808000",
        12 => "#800080",
        13 => "#008080",
        14 => "#C0C0C0",
        15 => "#E6E6E6",
        16 => "#CDCDCD",
        17 => "#B3B3B3",
        18 => "#9A9A9A",
        19 => "#808080",
        20 => "#666666",
        21 => "#4D4D4D",
        22 => "#333333",
        23 => "#1A1A1A",
        _ => "#000000",
    }
    .into()
}

fn num(cell: Option<&Cell>) -> Option<f64> {
    cell.and_then(|c| c.numeric())
}

fn cell_num(cell: &Cell, env: &vsdx::formula::Env) -> Option<f64> {
    if let Some(v) = cell.numeric() {
        return Some(v);
    }
    let f = cell.f.as_deref()?;
    if f == "Inh" || f == "No Formula" {
        return None;
    }
    vsdx::formula::eval(f, env).ok()?.as_f64()
}

/// A cell of the first Character row, inherited per-cell the way Visio
/// resolves character formatting: the instance's own row first (master
/// instances typically carry a LangID-only row), then the source shape's
/// row via the master link, then the text-style stylesheet cascade. A row
/// that omits a cell inherits it — sections inherit separately from
/// cells, and `effective_sheet` stops at the instance sheet as soon as it
/// has any section (the same gap the Paragraph lookup in `emit_text`
/// works around).
fn char_cell<'a>(
    doc: &'a Document,
    s: &'a Shape,
    chain: &[&'a Shape],
    name: &str,
) -> Option<&'a Cell> {
    let first_row = |sheet: &'a Sheet| {
        sheet
            .sections_named("Character")
            .into_iter()
            .next()
            .and_then(|sec| sec.rows.first())
    };
    if let Some(c) = first_row(&s.sheet).and_then(|r| r.cell(name)) {
        return Some(c);
    }
    let source = doc.instance_source(s, chain);
    if let Some(src) = source {
        if let Some(c) = first_row(&src.sheet).and_then(|r| r.cell(name)) {
            return Some(c);
        }
    }
    // Text-style stylesheet cascade. The instance usually leaves the
    // style attributes to its master root; stylesheets reference their
    // parents through a TextStyle attribute.
    let style_id = s
        .text_style
        .or_else(|| source.and_then(|src| src.text_style))?;
    let mut cur = doc.stylesheet(style_id)?;
    for _ in 0..8 {
        if let Some(c) = first_row(&cur.sheet).and_then(|r| r.cell(name)) {
            return Some(c);
        }
        let next_id = cur
            .attrs
            .iter()
            .find(|(k, _)| k == "TextStyle")
            .and_then(|(_, v)| v.parse::<i64>().ok())
            .or_else(|| {
                cur.sheet
                    .cell("TextStyle")
                    .and_then(|c| c.numeric())
                    .map(|v| v as i64)
            })?;
        if next_id == cur.id {
            return None;
        }
        cur = doc.stylesheet(next_id)?;
    }
    None
}

/// First Character-section row's Size cell (inches), per-cell inherited
/// through `char_cell`.
fn char_size(doc: &Document, s: &Shape, chain: &[&Shape]) -> Option<f64> {
    char_cell(doc, s, chain, "Size")
        .and_then(|c| c.numeric())
        .filter(|v| *v > 0.0)
}

/// First Character-section row's Font cell — the family name as stored
/// in VSDX (`<Cell N='Font' V='Arial'/>`), inherited per-cell like Size.
/// Legacy binary-format font indexes (numeric values) and the `Themed`
/// placeholder (meaning "the theme's font", rendered with Visio's default
/// family) name no family.
fn char_font(doc: &Document, s: &Shape, chain: &[&Shape]) -> Option<String> {
    let name = char_cell(doc, s, chain, "Font").and_then(|c| c.v.clone())?;
    let name = name.trim();
    if name.is_empty() || name.parse::<f64>().is_ok() || name.eq_ignore_ascii_case("themed") {
        return None;
    }
    Some(name.to_string())
}

/// Metric-compatible substitutes for the Microsoft core fonts (the
/// libre designer-alternative set), with the CSS generic each family
/// degrades to. Case-insensitive lookup.
fn font_substitute(name: &str) -> Option<(&'static str, &'static str)> {
    Some(match name.trim().to_ascii_lowercase().as_str() {
        "arial" => ("Liberation Sans", "sans-serif"),
        "calibri" => ("Carlito", "sans-serif"),
        "cambria" => ("Caladea", "serif"),
        "segoe ui" => ("Carlito", "sans-serif"),
        "times new roman" => ("Liberation Serif", "serif"),
        "courier new" => ("Liberation Mono", "monospace"),
        _ => return None,
    })
}

/// Quote a font-family name for the CSS shorthand (single quotes when
/// it contains anything but letters, digits and dashes). Inside the
/// quotes, `'` and `\` are CSS-escaped so a document-supplied family
/// name can never break out of the quoted string.
fn quote_family(name: &str) -> String {
    if name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        name.to_string()
    } else {
        format!("'{}'", name.replace('\\', "\\\\").replace('\'', "\\'"))
    }
}

/// `font-family` fallback chain for a document font (GitHub #17):
/// the document's own family first (the best face on any viewer),
/// then its metric-compatible substitute if the alias table covers
/// it, then Carlito (the GUI's embedded face — also a sensible
/// last-name fallback for viewers), then the CSS generic class.
/// Text with no Font cell gets Visio's default family — Calibri —
/// through the same table (Calibri → Carlito). Deduplicated
/// case-insensitively.
/// The font-family fallback chain (UNQUOTED names, CSS generic
/// last) the renderer declares for a shape's resolved Character Font
/// — the same walk [`crate::textmetrics`] measures with, so the wrap
/// decision and the raster use one resolution (#21).
pub(crate) fn family_chain(font: Option<&str>) -> Vec<String> {
    let name = match font.map(str::trim) {
        Some(f) if !f.is_empty() => f,
        _ => "Calibri",
    };
    let (substitute, class) = match font_substitute(name) {
        Some((s, c)) => (Some(s), c),
        None => (None, "sans-serif"),
    };
    let mut chain = vec![name.to_string()];
    for c in substitute.into_iter().chain(["Carlito"]) {
        if !chain.iter().any(|f| f.eq_ignore_ascii_case(c)) {
            chain.push(c.to_string());
        }
    }
    chain.push(class.to_string());
    chain
}

fn font_family_attr(font: Option<&str>) -> String {
    let chain: Vec<String> = family_chain(font).iter().map(|f| quote_family(f)).collect();
    // Document-supplied family names land in an XML attribute — the
    // joined CSS list is attribute content like any other.
    format!(" font-family=\"{}\"", escape_attr(&chain.join(", ")))
}

/// Resolved (Width, Height) for geometry scaling.
pub(crate) fn shape_size(doc: &Document, s: &Shape, chain: &[&Shape]) -> (f64, f64) {
    let env = shape_env(doc, s, chain);
    let get = |n: &str| {
        doc.resolve_cell(s, chain, n)
            .and_then(|c| cell_num(&c, &env))
    };
    (get("Width").unwrap_or(1.0), get("Height").unwrap_or(1.0))
}

/// Environment for evaluating geometry-row formulas: the shape's resolved
/// named cells plus this geometry section's own row cells.
fn shape_env_for(doc: &Document, s: &Shape, chain: &[&Shape], rows: &[Row]) -> vsdx::formula::Env {
    let mut env = shape_env(doc, s, chain);
    for row in rows.iter().filter(|r| r.t.is_some()) {
        for (cname, cell) in &row.cells {
            if let Some(v) = cell.numeric() {
                env.insert(format!("{}.{}", cname, row.ix), v);
            }
        }
    }
    env
}

/// Build an evaluation environment for a shape: resolved cells by name,
/// two passes (values first, then formulas against those values).
pub fn shape_env(doc: &Document, s: &Shape, chain: &[&Shape]) -> vsdx::formula::Env {
    let names = [
        "PinX",
        "PinY",
        "Width",
        "Height",
        "LocPinX",
        "LocPinY",
        "Angle",
        "TxtPinX",
        "TxtPinY",
        "TxtWidth",
        "TxtHeight",
        "LineWeight",
        "Char.Size",
    ];
    let mut env = vsdx::formula::Env::new();
    for n in names {
        if let Some(c) = doc.resolve_cell(s, chain, n) {
            if let Some(v) = c.numeric() {
                env.insert(n.to_string(), v);
            }
        }
    }
    // User-defined cells (master geometry formulas reference
    // `User.Corner` and friends for rounded-corner rectangles).
    // Instance-local rows override the master's: inline copies keep
    // only sparse User caches (e.g. a heading's HTxtWidth) while the
    // rest — CStyle, hidden, … — inherit from the master source, so
    // style-gated section cells like NoShow re-evaluate per instance.
    let own = doc.effective_sheet(s, chain);
    let mut user_sheets: Vec<&Sheet> = Vec::new();
    if let Some(src) = doc.instance_source(s, chain) {
        if !std::ptr::eq(&src.sheet, own.as_ref()) {
            user_sheets.push(&src.sheet);
        }
    }
    user_sheets.push(own.as_ref());
    for sheet in user_sheets {
        for user in sheet.sections_named("User") {
            for row in user.rows.iter().filter(|r| r.n.is_some()) {
                if let Some(v) = row.cell("Value").and_then(|c| c.numeric()) {
                    env.insert(format!("User.{}", row.n.as_deref().unwrap_or("")), v);
                }
            }
        }
        // Scratch + Controls numeric cells (#249): the connector
        // stencil's band/knee masters derive their geometry rows
        // through them (`LineTo(F:Scratch.X2*Scratch.C1)`,
        // `Controls.Row_1.Y`) — without them every such row fell to
        // its authored cached pose and the ink never tracked the
        // endpoints. Scratch references are `Scratch.X1` (row 1's X
        // cell); Controls references carry the row's name
        // (`Controls.Row_1.Y`).
        for scratch in sheet.sections_named("Scratch") {
            for row in &scratch.rows {
                for (name, cell) in &row.cells {
                    if let Some(v) = cell.numeric() {
                        // Visio's Scratch references are 1-BASED
                        // (`Scratch.X1` is the first row's X).
                        env.insert(
                            format!("Scratch.{}{}", name, row.ix + 1),
                            v,
                        );
                    }
                }
            }
        }
        for controls in sheet.sections_named("Controls") {
            for row in &controls.rows {
                let rn = row
                    .n
                    .clone()
                    .unwrap_or_else(|| format!("Row_{}", row.ix));
                for (name, cell) in &row.cells {
                    if let Some(v) = cell.numeric() {
                        env.insert(format!("Controls.{}.{}", rn, name), v);
                    }
                }
            }
        }
    }
    // Geometry row coordinates (Geometry1.X1 style references).
    for (i, geo) in own.sections_named("Geometry").iter().enumerate() {
        let gname = format!("Geometry{}", i + 1);
        for row in geo.rows.iter().filter(|r| r.t.is_some()) {
            for (cname, cell) in &row.cells {
                if let Some(v) = cell.numeric() {
                    env.insert(format!("{}.{}{}", gname, cname, row.ix), v);
                }
            }
        }
    }
    // Second pass: formulas may reference the first-pass values.
    for n in names {
        if let Some(c) = doc.resolve_cell(s, chain, n) {
            if c.numeric().is_none() {
                if let Some(f) = c.f.as_deref() {
                    if let Some(v) = vsdx::formula::eval(f, &env).ok().and_then(|v| v.as_f64()) {
                        env.insert(n.to_string(), v);
                    }
                }
            }
        }
    }
    env
}

fn escape_attr(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn escape_text(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use vsdx::model::{IndexChild, Page};

    /// Synthetic Role-callout-shaped master (issue #15 specimen, values
    /// recomputed — no Microsoft content): a root group whose children
    /// mix proportional XForms (`Sheet.5!Width*1`) with GUARDed absolute
    /// ones (`GUARD(4MM)`), driven by User cells whose formulas reference
    /// the group Height.
    fn role_master_fixture() -> (Shape, Shape) {
        let set = |sheet: &mut Sheet, n: &str, v: f64, f: Option<&str>| {
            sheet.set_cell(
                n,
                Cell {
                    v: Some(format!("{v}")),
                    f: f.map(str::to_string),
                    ..Cell::default()
                },
            );
        };
        // Root: master canvas 1.1811 × 0.7874.
        let mut root = Shape {
            id: 5,
            ..Shape::default()
        };
        set(&mut root.sheet, "Width", 1.181102362204724, None);
        set(&mut root.sheet, "Height", 0.7874015748031495, None);
        let urow = |n: &str, v: f64, f: Option<&str>| Row {
            n: Some(n.into()),
            cells: vec![(
                "Value".into(),
                Cell {
                    v: Some(format!("{v}")),
                    f: f.map(str::to_string),
                    ..Cell::default()
                },
            )],
            ..Row::default()
        };
        root.sheet.children.push(SheetChild::Section(Section {
            name: "User".into(),
            rows: vec![
                urow("ShapeLayout", 0.0, None),
                urow("InfoPanelHeight", 0.2362204724409449, None),
                urow("IconX", 0.1181102362204724, Some("User.InfoPanelHeight/2")),
                urow(
                    "IconY",
                    0.6692913385826771,
                    Some("IF(OR(User.ShapeLayout<>1), Height-User.InfoPanelHeight/2, User.InfoPanelHeight/2)"),
                ),
            ],
            ..Section::default()
        }));
        // Container child: fully proportional.
        let mut container = Shape {
            id: 6,
            ..Shape::default()
        };
        set(
            &mut container.sheet,
            "Width",
            1.181102362204721,
            Some("Sheet.5!Width*1"),
        );
        set(
            &mut container.sheet,
            "Height",
            0.7874015748031491,
            Some("Sheet.5!Height*1"),
        );
        set(
            &mut container.sheet,
            "PinX",
            0.5905511811023657,
            Some("Sheet.5!Width*0.5"),
        );
        set(
            &mut container.sheet,
            "PinY",
            0.3937007874015748,
            Some("Sheet.5!Height*0.5"),
        );
        set(
            &mut container.sheet,
            "LocPinX",
            0.5905511811023605,
            Some("Width*0.5"),
        );
        set(
            &mut container.sheet,
            "LocPinY",
            0.3937007874015746,
            Some("Height*0.5"),
        );
        // Icon child: absolute size, pinned via User cells.
        let mut icon = Shape {
            id: 8,
            ..Shape::default()
        };
        set(
            &mut icon.sheet,
            "Width",
            0.1574803149606299,
            Some("GUARD(4MM)"),
        );
        set(
            &mut icon.sheet,
            "Height",
            0.1692913385826771,
            Some("GUARD(4.3MM)"),
        );
        set(
            &mut icon.sheet,
            "PinX",
            0.1181102362204724,
            Some("GUARD(Sheet.5!User.IconX)"),
        );
        set(
            &mut icon.sheet,
            "PinY",
            0.6692913385826771,
            Some("GUARD(Sheet.5!User.IconY)"),
        );
        set(
            &mut icon.sheet,
            "LocPinX",
            0.07874015748031496,
            Some("Width*0.5"),
        );
        set(
            &mut icon.sheet,
            "LocPinY",
            0.08464566929133857,
            Some("Height*0.5"),
        );
        // Literal child (no formulas): uniform-scale fallback.
        let mut plain = Shape {
            id: 16,
            ..Shape::default()
        };
        set(&mut plain.sheet, "Width", 0.5, None);
        set(&mut plain.sheet, "Height", 0.25, None);
        set(&mut plain.sheet, "PinX", 0.5, None);
        set(&mut plain.sheet, "PinY", 0.25, None);
        root.sub_shapes = vec![container, icon, plain];

        // Instance: same master stretched tall (2.25 × 7.02 aspect change).
        let mut inst = Shape {
            id: 65,
            master: Some(7),
            ..Shape::default()
        };
        set(&mut inst.sheet, "Width", 2.657480296178819, None);
        set(&mut inst.sheet, "Height", 5.528301723437821, None);
        (root, inst)
    }

    /// Parse `matrix(a,b,c,d,e,f)` / `scale(sx,sy)` into (a..f).
    fn parse_tf(t: &str) -> [f64; 6] {
        let body = t
            .trim_start_matches("matrix(")
            .trim_start_matches("scale(")
            .trim_end_matches(')');
        let nums: Vec<f64> = body.split(',').map(|n| n.parse().unwrap()).collect();
        if t.starts_with("scale(") {
            [nums[0], 0.0, 0.0, nums[1], 0.0, 0.0]
        } else {
            nums.try_into().unwrap()
        }
    }

    #[test]
    fn mirrored_proportional_child_keeps_uniform_scale() {
        let (root, inst) = role_master_fixture();
        let (iw, ih) = (2.657480296178819, 5.528301723437821);
        let (mw, mh) = (1.181102362204724, 0.7874015748031495);
        let (kx, ky) = (iw / mw, ih / mh); // 2.25 × 7.020943
        let env = mirror_env(&root, &inst, iw, ih);
        // Container (Sheet.5!Width*1 …): exactly the historical uniform
        // scale — no regression for proportional children.
        let t = mirror_child_transform(&root.sub_shapes[0], &env, (kx, ky)).unwrap();
        assert_eq!(t, "scale(2.250000,7.020943)", "got {t}");
    }

    #[test]
    fn mirrored_guarded_child_keeps_absolute_size() {
        let (root, inst) = role_master_fixture();
        let (iw, ih) = (2.657480296178819, 5.528301723437821);
        let env = mirror_env(&root, &inst, iw, ih);
        // Icon: 4×4.3 mm stays absolute; its pin re-evaluates against the
        // instance Height (User.IconY = H − InfoPanelHeight/2).
        let t = mirror_child_transform(&root.sub_shapes[1], &env, (2.25, 7.02)).unwrap();
        let m = parse_tf(&t);
        let e_py = ih - 0.2362204724409449 / 2.0; // 5.410191487…
        let m_py = 0.6692913385826771;
        let m_h = 0.1692913385826771;
        // wanted ∘ placed⁻¹ for angle 0, scale 1: pure translate by the
        // pin delta (effective LocPin = W/2 in both frames).
        let expected_ty = -((e_py - 0.1692913385826771 / 2.0) - (m_py - m_h / 2.0));
        assert!((m[0] - 1.0).abs() < 1e-6, "a={}", m[0]);
        assert!((m[3] - 1.0).abs() < 1e-6, "d={}", m[3]);
        assert!(m[4].abs() < 1e-6, "e={}", m[4]);
        assert!(
            (m[5] - expected_ty).abs() < 1e-5,
            "f={} expected {expected_ty}",
            m[5]
        );
    }

    #[test]
    fn mirrored_literal_child_falls_back_to_uniform_scale() {
        let (root, inst) = role_master_fixture();
        let env = mirror_env(&root, &inst, 2.657480296178819, 5.528301723437821);
        // No formulas anywhere: behavior is byte-identical to the old
        // uniform wrapper.
        let t = mirror_child_transform(&root.sub_shapes[2], &env, (2.25, 7.020943)).unwrap();
        assert_eq!(t, "scale(2.250000,7.020943)", "got {t}");
    }

    #[test]
    fn mirror_env_resolves_user_formulas_against_instance_height() {
        let (root, inst) = role_master_fixture();
        let env = mirror_env(&root, &inst, 2.657480296178819, 5.528301723437821);
        let v = env.get("User.IconY").copied().unwrap();
        assert!((v - (5.528301723437821 - 0.2362204724409449 / 2.0)).abs() < 1e-9);
    }

    #[test]
    fn line_pattern_dasharrays_scale_with_weight() {
        // #171 measured the INK (the owner-authored lines.vsdx fixture
        // printed at 600 dpi — Analysis/line-patterns-visio2016.md);
        // #235 reconciles it with MisterDiagram's canonical grammar:
        // Visio strokes dashes with ROUND caps, so the emitted
        // geometry is ink − 1w per mark / ink + 1w per gap (pattern 2
        // = the canonical 7/5, 14 = 9/3/3, 21 = 39/9/15/9), and dots
        // (ink 1w) become the 0.0001w tangent lead with the following
        // gap returning it — same painted ink, canonical parameters.
        // The values below are the CANONICAL GEOMETRY × weight, dotted
        // against the three witnessed grammar entries.
        let w = 0.02; // the fixture's weight
        let arr = |p: f64| line_dasharray(p, w);
        assert_eq!(arr(2.0).as_deref(), Some("0.14000 0.10000")); // canonical 7/5
        assert_eq!(arr(3.0).as_deref(), Some("0.0000020000 0.10000"));
        assert_eq!(arr(4.0).as_deref(), Some("0.14000 0.10000 0.0000020000 0.10000"));
        assert_eq!(
            arr(5.0).as_deref(),
            Some("0.14000 0.10000 0.0000020000 0.10000 0.0000020000 0.10000")
        );
        assert_eq!(
            arr(6.0).as_deref(),
            Some("0.14000 0.10000 0.14000 0.10000 0.0000020000 0.10000")
        );
        assert_eq!(arr(7.0).as_deref(), Some("0.38000 0.10000 0.14000 0.10000"));
        assert_eq!(
            arr(8.0).as_deref(),
            Some("0.38000 0.10000 0.14000 0.10000 0.14000 0.10000")
        );
        assert_eq!(arr(9.0).as_deref(), Some("0.06000 0.06000"));
        assert_eq!(arr(10.0).as_deref(), Some("0.0000020000 0.06000"));
        assert_eq!(
            arr(11.0).as_deref(),
            Some("0.06000 0.06000 0.0000020000 0.06000")
        );
        assert_eq!(
            arr(12.0).as_deref(),
            Some("0.06000 0.06000 0.0000020000 0.06000 0.0000020000 0.06000")
        );
        assert_eq!(
            arr(13.0).as_deref(),
            Some("0.06000 0.06000 0.06000 0.06000 0.0000020000 0.06000")
        );
        assert_eq!(
            arr(14.0).as_deref(),
            Some("0.18000 0.06000 0.06000 0.06000") // canonical 9/3/3
        );
        assert_eq!(
            arr(15.0).as_deref(),
            Some("0.18000 0.06000 0.06000 0.06000 0.06000 0.06000")
        );
        assert_eq!(arr(16.0).as_deref(), Some("0.30000 0.18000"));
        assert_eq!(arr(17.0).as_deref(), Some("0.0000020000 0.18000"));
        assert_eq!(
            arr(18.0).as_deref(),
            Some("0.30000 0.18000 0.0000020000 0.18000")
        );
        assert_eq!(
            arr(19.0).as_deref(),
            Some("0.30000 0.18000 0.0000020000 0.18000 0.0000020000 0.18000")
        );
        assert_eq!(
            arr(20.0).as_deref(),
            Some("0.30000 0.18000 0.30000 0.18000 0.0000020000 0.18000")
        );
        assert_eq!(
            arr(21.0).as_deref(),
            Some("0.78000 0.18000 0.30000 0.18000") // canonical 39/9/15/9
        );
        assert_eq!(
            arr(22.0).as_deref(),
            Some("0.78000 0.18000 0.30000 0.18000 0.30000 0.18000")
        );
        assert_eq!(arr(23.0).as_deref(), Some("0.02000 0.04000"));
        // Solid (1) and hidden (0, handled at the call site) render
        // without a dasharray; so do out-of-gallery values.
        assert!(line_dasharray(1.0, w).is_none());
        assert!(line_dasharray(0.0, w).is_none());
        assert!(line_dasharray(24.0, w).is_none());
    }

    /// A Character row with Font/Size/Color cells for char_font tests.
    fn char_row(font: Option<&str>) -> Row {
        let mut cells = Vec::new();
        if let Some(f) = font {
            cells.push((
                "Font".to_string(),
                Cell {
                    v: Some(f.to_string()),
                    ..Cell::default()
                },
            ));
        }
        Row {
            cells,
            ..Row::default()
        }
    }

    fn shape_with_character(font: Option<&str>) -> Shape {
        let mut s = Shape::default();
        s.sheet.children.push(SheetChild::Section(Section {
            name: "Character".into(),
            rows: vec![char_row(font)],
            ..Section::default()
        }));
        s
    }

    /// #72 + #199 + #185: arrowheads render per value at their
    /// measured physical sizes — each gallery entry emits its own
    /// head group matching the 600 dpi Visio print (open heads 14–22
    /// stroke, the dimension set composes ticks with rings/dots, 12
    /// is the single wide V, 43 the double chevron), the head scales
    /// with the LineWeight + C[size] unit law, and 0/46–50 emit none.
    /// #218 review round: fractional NURBS controls (xType/yType = 0)
    /// in the MASTER frame scale exactly once — a fraction lands at
    /// gx·W instance units (gx·mw master units through the one
    /// master→instance scale), never the old gx·W·scale. A degree-1
    /// NURBS flattens through its control points, so the emitted
    /// path pins them.
    #[test]
    fn nurbs_fractional_controls_scale_once_in_master_frame() {
        // Master: a 2×2 canvas; a degree-1 NURBS whose interior
        // controls sit at x fractions 0.5 and 0.75, y fraction 0.5.
        let mut mroot = Shape {
            id: 1,
            ..Shape::default()
        };
        for (n, v) in [("Width", "2"), ("Height", "2")] {
            mroot.sheet.set_cell(n, Cell::value(v));
        }
        mroot.sheet.children.push(SheetChild::Section(Section {
            name: "Geometry".into(),
            ix: 0,
            rows: vec![
                Row {
                    ix: 1,
                    t: Some("MoveTo".into()),
                    cells: vec![
                        ("X".into(), Cell::value("0")),
                        ("Y".into(), Cell::value("0")),
                    ],
                    ..Row::default()
                },
                Row {
                    ix: 2,
                    t: Some("NURBSTo".into()),
                    cells: vec![
                        ("X".into(), Cell::value("2")),
                        ("Y".into(), Cell::value("2")),
                        ("A".into(), Cell::value("2")),
                        ("B".into(), Cell::value("1")),
                        ("C".into(), Cell::value("0")),
                        ("D".into(), Cell::value("1")),
                        (
                            "E".into(),
                            Cell {
                                v: Some(
                                    "NURBS(3, 1, 0, 0, 0.5,0.5,1,1, 0.75,0.5,2,1)".into(),
                                ),
                                ..Cell::default()
                            },
                        ),
                    ],
                    ..Row::default()
                },
            ],
            ..Section::default()
        }));
        let doc = Document {
            masters: vec![vsdx::model::Master {
                id: 7,
                shapes: vec![mroot],
                ..vsdx::model::Master::default()
            }],
            ..Document::default()
        };
        // Instance: 4×2 — double the master width, equal height.
        let mut inst = Shape {
            id: 9,
            master: Some(7),
            ..Shape::default()
        };
        for (n, v) in [
            ("PinX", "3"),
            ("PinY", "3"),
            ("Width", "4"),
            ("Height", "2"),
            ("LocPinX", "0"),
            ("LocPinY", "0"),
        ] {
            inst.sheet.set_cell(n, Cell::value(v));
        }
        let chain: Vec<&Shape> = Vec::new();
        let mut out = String::new();
        render_shape(&doc, &inst, 8.5, &chain, "", None, None, None, &mut out);
        // The controls land at gx·W: 0.5 → 2.0 and 0.75 → 3.0; the y
        // fraction 0.5 of H=2 → 1.0 (emitted negated). The old
        // double-scale put them at 4.0 and 6.0.
        assert!(
            out.contains("2.00000 -1.00000"),
            "control 1 (0.5·W) missing: {out}"
        );
        assert!(
            out.contains("3.00000 -1.00000"),
            "control 2 (0.75·W) missing: {out}"
        );
        assert!(!out.contains("6.00000"), "double-scaled control leaked: {out}");
    }

    #[test]
    fn arrow_heads_render_per_value_and_scale() {
        let doc = Document::default();
        let line_with = |cells: &[(&str, &str)]| {
            let mut s = Shape::default();
            for (n, v) in cells {
                s.sheet.set_cell(n, Cell::value(*v));
            }
            s.sheet.children.push(SheetChild::Section(Section {
                name: "Geometry".into(),
                ix: 0,
                rows: vec![
                    Row {
                        ix: 1,
                        t: Some("MoveTo".into()),
                        cells: vec![
                            ("X".into(), Cell::value("0")),
                            ("Y".into(), Cell::value("0")),
                        ],
                        ..Row::default()
                    },
                    Row {
                        ix: 2,
                        t: Some("LineTo".into()),
                        cells: vec![
                            ("X".into(), Cell::value("1")),
                            ("Y".into(), Cell::value("1")),
                        ],
                        ..Row::default()
                    },
                ],
                ..Section::default()
            }));
            let chain = [&s];
            let mut out = String::new();
            render_shape(&doc, &s, 11.0, &chain, "", None, None, None, &mut out);
            out
        };
        // 0 = no head at all.
        let none = line_with(&[("EndArrow", "0")]);
        assert!(!none.contains("dc-ah"), "value 0 emits no head: {none}");
        // Each gallery value emits a head group with its own unit-space
        // shape (#169 + #199 + the #185 print re-measure).
        for (v, needle) in [
            (1, "M-1,-1 L0,0 L-1,1"),
            (2, "M0,0 L-1,-1 L-1,1 Z"),
            (3, "M-2,-1 L0,0 L-2,1"),
            (4, "M0,0 L-2,-1 L-2,1 Z"),
            (5, "C-1.68524,-0.37049"),
            (6, "C-2.31476,-0.37049"),
            (9, "M1,-1 L-1,1"),
            (11, "M-0.75,-0.75 L0.75,-0.75 L0.75,0.75 L-0.75,0.75 Z"),
            (12, "M-3,-1 L0,0 L-3,1"),
            (13, "M0,0 L-3,-1 L-3,1 Z"),
            (14, "M0,0 L-3,-1 L-3,1 Z"),
            (15, "M0,0 L-1,-1 L-1,1 Z"),
            (22, "M0,0 L-2.25,-1.125 L-4.5,0 L-2.25,1.125 Z"),
            (25, "M-2.25,-0.75 L-2.25,0.75"),
            (27, "M0,-0.75 L-1.5,0 L0,0.75"),
            (40, "M0,0 L-2,-1 L-2,1 Z M-2,0 L-4,-1 L-4,1 Z"),
            (43, "M-4,-1 L-2,0 L-4,1"),
            (45, "M-5,-0.75 L-5,0.75"),
        ] {
            let out = line_with(&[("EndArrow", &v.to_string())]);
            assert!(out.contains("dc-ah"), "value {v} emits a head group: {out}");
            assert!(out.contains(needle), "value {v} draws its shape: {out}");
        }
        // The dots/rings are circle elements (10/42 filled, 20/41
        // rings; the 35–37 dots carry the measured 0.825 radius).
        for (v, ring) in [(10i64, false), (20, true), (41, true), (42, false)] {
            let out = line_with(&[("EndArrow", &v.to_string())]);
            assert!(out.contains("<circle"), "value {v} is a circle: {out}");
            let head = out.split(r#"class="dc-ah""#).nth(1).expect("head group");
            assert_eq!(head.contains("fill=\"none\""), ring, "value {v}: {out}");
        }
        let dot35 = line_with(&[("EndArrow", "35")]);
        assert!(
            dot35.contains(r#"r="0.825""#) && dot35.contains("M-2.25,-0.75 L-2.25,0.75"),
            "value 35 composes tick + 0.825 dot: {dot35}"
        );
        // 8 shares the stealth-dart glyph with 5 (the native SVG alias;
        // the print's concave rear agrees).
        let a5 = line_with(&[("EndArrow", "5")]);
        let a8 = line_with(&[("EndArrow", "8")]);
        let d5 = a5.split(r#"class="dc-ah""#).nth(1).expect("5");
        let d8 = a8.split(r#"class="dc-ah""#).nth(1).expect("8");
        assert_eq!(d5, d8, "8 renders the 5 glyph");
        // #169 regression: 13 is a SINGLE head — one closed triangle —
        // and the other filled singles (2, 4) never stack.
        for v in [2i64, 4, 13] {
            let out = line_with(&[("EndArrow", &v.to_string())]);
            let closes = out.matches(" Z").count();
            assert!(closes <= 1, "value {v} must not stack triangles: {out}");
        }
        // #199: the open heads 14–22 stroke (fill="none"), never fill;
        // the double triangles 39/40 stack two heads, filled vs open.
        for v in 14..=22i64 {
            let out = line_with(&[("EndArrow", &v.to_string())]);
            assert!(
                out.contains("fill=\"none\""),
                "value {v} is an open head: {out}"
            );
        }
        let d39 = line_with(&[("EndArrow", "39")]);
        assert_eq!(d39.matches(" Z").count(), 2, "39 stacks two: {d39}");
        let d40 = line_with(&[("EndArrow", "40")]);
        assert_eq!(d40.matches(" Z").count(), 2, "40 stacks two: {d40}");
        assert!(d40.contains("fill=\"none\""), "40 is the open double");
        // #158: past the gallery, Visio draws a bare end (fixture
        // values 46–50 print no head).
        for v in 46..=50i64 {
            let out = line_with(&[("EndArrow", &v.to_string())]);
            assert!(!out.contains("dc-ah"), "value {v} emits no head: {out}");
        }
        // The unit law (#185): the head scales by LineWeight + C[size]
        // — size 1 (C=1.8 pt) at a 0.05 in pen gives 0.075 in.
        let heavy = line_with(&[
            ("EndArrow", "1"),
            ("EndArrowSize", "1"),
            ("LineWeight", "0.05"),
        ]);
        assert!(
            heavy.contains("scale(0.07500)"),
            "the head rides LineWeight + C[size]: {heavy}"
        );
        // Begin + end heads face outward/along the travel: the
        // diagonal runs to (1, -1) in the y-down frame, so the end
        // head points at -45° and the begin head outward at 135°.
        let both = line_with(&[("BeginArrow", "1"), ("EndArrow", "1")]);
        assert!(
            both.contains("rotate(-45.0000)"),
            "end head along the travel: {both}"
        );
        assert!(
            both.contains("rotate(135.0000)"),
            "begin head faces outward: {both}"
        );
        // The shaft trims at the head's rear (setback 2 × unit for the
        // classic triangle), the head anchoring at the ORIGINAL end.
        let unit = 0.75 / 72.0 + 2.52 / 72.0;
        let t = (2.0f64.sqrt() - 2.0 * unit) / 2.0f64.sqrt();
        let trimmed = line_with(&[("EndArrow", "4")]);
        assert!(
            trimmed.contains(&format!("L {:.5} -{:.5}", t, t)),
            "the shaft stops at the setback: {trimmed}"
        );
        assert!(
            trimmed.contains("translate(1.00000"),
            "the head keeps the original endpoint: {trimmed}"
        );
        // The begin setback is the end setback minus max(1%, 0.36 pt).
        let both4 = line_with(&[("BeginArrow", "4"), ("EndArrow", "4")]);
        let begin_setback = 2.0 * unit - (2.0 * unit * 0.01).max(0.36 / 72.0);
        let bt = begin_setback / 2.0f64.sqrt();
        assert!(
            both4.contains(&format!("M {:.5} -{:.5}", bt, bt)),
            "the begin trim is the smaller one: {both4}"
        );
    }

    /// A geometry row for the wave-3 shape fixtures.
    fn w3_row(ix: i64, t: &str, cells: &[(&str, &str)]) -> Row {
        Row {
            ix,
            t: Some(t.to_string()),
            cells: cells
                .iter()
                .map(|(n, v)| (n.to_string(), Cell::value(*v)))
                .collect(),
            ..Row::default()
        }
    }

    /// #227: a geometry whose walk closes draws no line ends — Visio
    /// renders a closed rectangle with EndArrow=5 as a clean corner
    /// (MisterDiagram #105 field batch); an open control line beside
    /// it keeps its head, and the closed section keeps its stroke.
    #[test]
    fn closed_geometry_draws_no_arrowheads() {
        let doc = Document::default();
        let render = |rows: Vec<Row>, cells: &[(&str, &str)]| {
            let mut s = Shape::default();
            for (n, v) in cells {
                s.sheet.set_cell(n, Cell::value(*v));
            }
            s.sheet.children.push(SheetChild::Section(Section {
                name: "Geometry".into(),
                ix: 0,
                rows,
                ..Section::default()
            }));
            let chain = [&s];
            let mut out = String::new();
            render_shape(&doc, &s, 11.0, &chain, "", None, None, None, &mut out);
            out
        };
        // A closed rectangle walk (returns to its start) with both
        // arrow cells set.
        let closed = render(
            vec![
                w3_row(1, "MoveTo", &[("X", "0"), ("Y", "0")]),
                w3_row(2, "LineTo", &[("X", "1"), ("Y", "0")]),
                w3_row(3, "LineTo", &[("X", "1"), ("Y", "1")]),
                w3_row(4, "LineTo", &[("X", "0"), ("Y", "1")]),
                w3_row(5, "LineTo", &[("X", "0"), ("Y", "0")]),
            ],
            &[("EndArrow", "5"), ("BeginArrow", "5")],
        );
        assert!(
            !closed.contains("dc-ah"),
            "a closed walk draws no line ends: {closed}"
        );
        assert!(
            closed.contains("<path") && closed.contains("L 0.00000 -0.00000"),
            "the closed contour still strokes: {closed}"
        );
        // The open control keeps its head.
        let open = render(
            vec![
                w3_row(1, "MoveTo", &[("X", "0"), ("Y", "0")]),
                w3_row(2, "LineTo", &[("X", "1"), ("Y", "1")]),
            ],
            &[("EndArrow", "5")],
        );
        assert!(open.contains("dc-ah"), "the open line keeps its head: {open}");
        // A MULTI-subpath walk takes no heads either — Visio gives
        // them none (the trim_path evidence note; CodeRabbit flagged
        // the last-subpath-closed case, #278).
        let multi = render(
            vec![
                w3_row(1, "MoveTo", &[("X", "0"), ("Y", "0")]),
                w3_row(2, "LineTo", &[("X", "1"), ("Y", "0")]),
                w3_row(3, "MoveTo", &[("X", "2"), ("Y", "0")]),
                w3_row(4, "LineTo", &[("X", "3"), ("Y", "1")]),
            ],
            &[("EndArrow", "5"), ("BeginArrow", "5")],
        );
        assert!(
            !multi.contains("dc-ah"),
            "a multi-subpath walk draws no line ends: {multi}"
        );
    }

    /// #233: an authored D=0 (zero-width) EllipticalArcTo draws the
    /// endpoint CHORD even when its A/B control is off the chord —
    /// the old fallback's quadratic-through-m doubled back
    /// (MisterDiagram M5.50; 887 corpus masters carry these rows). A
    /// positive-width control on the same points still solves as an
    /// arc.
    #[test]
    fn zero_width_elliptical_arc_to_draws_the_chord() {
        let doc = Document::default();
        let render = |d: &str| {
            let mut s = Shape::default();
            s.sheet.set_cell("Width", Cell::value("2"));
            s.sheet.set_cell("Height", Cell::value("2"));
            s.sheet.children.push(SheetChild::Section(Section {
                name: "Geometry".into(),
                ix: 0,
                rows: vec![
                    w3_row(1, "MoveTo", &[("X", "0"), ("Y", "0")]),
                    w3_row(
                        2,
                        "EllipticalArcTo",
                        &[("X", "1"), ("Y", "0"), ("A", "0.5"), ("B", "1"), ("C", "0"), ("D", d)],
                    ),
                ],
                ..Section::default()
            }));
            let chain = [&s];
            let mut out = String::new();
            render_shape(&doc, &s, 11.0, &chain, "", None, None, None, &mut out);
            out
        };
        let flat = render("0");
        assert!(
            flat.contains("L 1.00000 -0.00000"),
            "D=0 draws the endpoint chord: {flat}"
        );
        assert!(!flat.contains("Q "), "no doubled-back quadratic: {flat}");
        assert!(!flat.contains(" A "), "no solved arc for a zero-width row: {flat}");
        let arc = render("0.5");
        assert!(arc.contains(" A "), "a positive width still solves an arc: {arc}");
    }

    /// #235: an explicit-zero LineWeight keeps zero — SVG paints
    /// nothing at stroke-width 0, and the old `.max(0.001)` fallback
    /// pen drew stray hairlines Visio never shows.
    #[test]
    fn explicit_zero_line_weight_paints_nothing() {
        let doc = Document::default();
        let mut s = Shape::default();
        s.sheet.set_cell("LineWeight", Cell::value("0"));
        s.sheet.children.push(SheetChild::Section(Section {
            name: "Geometry".into(),
            ix: 0,
            rows: vec![
                w3_row(1, "MoveTo", &[("X", "0"), ("Y", "0")]),
                w3_row(2, "LineTo", &[("X", "1"), ("Y", "1")]),
            ],
            ..Section::default()
        }));
        let chain = [&s];
        let mut out = String::new();
        render_shape(&doc, &s, 11.0, &chain, "", None, None, None, &mut out);
        assert!(
            out.contains(r#"stroke-width="0.00000""#),
            "an explicit zero stays zero: {out}"
        );
        assert!(
            !out.contains(r#"stroke-width="0.001"#),
            "no fallback pen: {out}"
        );
    }

    /// #228: every fill-eligible Geometry section of one shape joins
    /// ONE alternate-fill path (`fill-rule="evenodd"`) under the
    /// per-section strokes — nesting and overlap toggle coverage
    /// regardless of winding direction (MisterDiagram M5.109: the
    /// two rects here wind in OPPOSITE directions, which nonzero
    /// fill would paint solid). NoFill opts a section out of the
    /// fill only; NoLine leaves the fill in and drops the stroke.
    #[test]
    fn sections_fill_one_even_odd_path() {
        let doc = Document::default();
        let mut s = Shape::default();
        // Outer rect, clockwise.
        s.sheet.children.push(SheetChild::Section(Section {
            name: "Geometry".into(),
            ix: 0,
            rows: vec![
                w3_row(1, "MoveTo", &[("X", "0"), ("Y", "0")]),
                w3_row(2, "LineTo", &[("X", "2"), ("Y", "0")]),
                w3_row(3, "LineTo", &[("X", "2"), ("Y", "2")]),
                w3_row(4, "LineTo", &[("X", "0"), ("Y", "2")]),
                w3_row(5, "LineTo", &[("X", "0"), ("Y", "0")]),
            ],
            ..Section::default()
        }));
        // Inner rect, COUNTER-clockwise — same coverage under
        // even-odd, solid under nonzero.
        s.sheet.children.push(SheetChild::Section(Section {
            name: "Geometry".into(),
            ix: 1,
            rows: vec![
                w3_row(1, "MoveTo", &[("X", "0.5"), ("Y", "0.5")]),
                w3_row(2, "LineTo", &[("X", "0.5"), ("Y", "1.5")]),
                w3_row(3, "LineTo", &[("X", "1.5"), ("Y", "1.5")]),
                w3_row(4, "LineTo", &[("X", "1.5"), ("Y", "0.5")]),
                w3_row(5, "LineTo", &[("X", "0.5"), ("Y", "0.5")]),
            ],
            ..Section::default()
        }));
        // A NoFill section: stroke only, excluded from the union.
        s.sheet.children.push(SheetChild::Section(Section {
            name: "Geometry".into(),
            ix: 2,
            cells: vec![("NoFill".to_string(), Cell::value("1"))],
            rows: vec![
                w3_row(1, "MoveTo", &[("X", "3"), ("Y", "0")]),
                w3_row(2, "LineTo", &[("X", "3"), ("Y", "1")]),
            ],
            ..Section::default()
        }));
        // A NoLine section: fill only — joins the union, no stroke.
        s.sheet.children.push(SheetChild::Section(Section {
            name: "Geometry".into(),
            ix: 3,
            cells: vec![("NoLine".to_string(), Cell::value("1"))],
            rows: vec![
                w3_row(1, "MoveTo", &[("X", "4"), ("Y", "0")]),
                w3_row(2, "LineTo", &[("X", "4"), ("Y", "1")]),
            ],
            ..Section::default()
        }));
        let chain = [&s];
        let mut out = String::new();
        render_shape(&doc, &s, 11.0, &chain, "", None, None, None, &mut out);
        // ONE even-odd fill element, first among the paths (under the
        // ink), carrying exactly the three fill-eligible subpaths.
        assert_eq!(
            out.matches("fill-rule=\"evenodd\"").count(),
            1,
            "one alternate fill per shape: {out}"
        );
        let fill_elem = out
            .split("fill-rule=\"evenodd\"")
            .nth(1)
            .expect("fill element present");
        let fill_elem = &fill_elem[..fill_elem.find("/>").unwrap_or(fill_elem.len())];
        assert_eq!(
            fill_elem.matches("M ").count(),
            3,
            "the two rects (either winding) and the NoLine section join; NoFill stays out: {fill_elem}"
        );
        // The strokes stay per-section: outer, inner, NoFill open
        // line — three stroke paths after the fill element.
        let after_fill = out.split("fill-rule=\"evenodd\"").nth(1).unwrap_or("");
        assert_eq!(
            after_fill.matches("<path").count(),
            3,
            "three per-section strokes follow the fill: {after_fill}"
        );
        assert!(
            after_fill.contains(r#"fill="none""#),
            "stroke paths carry no fill: {after_fill}"
        );
    }

    /// #73: Char.Color accepts both spellings with fill/line-parity
    /// semantics — a `#rrggbb` hex string verbatim, a legacy palette
    /// index through the [MS-VSDX] table — so a palette write and a
    /// legacy file render identically.
    #[test]
    fn char_color_accepts_hex_and_palette_index() {
        let doc = Document::default();
        let emit = |color: &str| {
            let mut s = Shape::default();
            s.sheet.text = Some("colored".into());
            s.sheet.set_cell("Width", Cell::value("2.0"));
            s.sheet.set_cell("TxtWidth", Cell::value("2.0"));
            s.sheet.children.push(SheetChild::Section(Section {
                name: "Character".into(),
                rows: vec![Row {
                    cells: vec![("Color".to_string(), Cell::value(color))],
                    ..Row::default()
                }],
                ..Section::default()
            }));
            let chain = [&s];
            let mut out = String::new();
            emit_text(&doc, &s, &chain, 2.0, 1.0, 1.0, 1.0, false, None, &mut out);
            out
        };
        let hex = emit("#ED7D31");
        assert!(
            hex.contains(r##"fill="#ED7D31""##),
            "hex Char.Color renders verbatim: {hex}"
        );
        // Palette index 4 is #0000FF in the [MS-VSDX] table; 2 is
        // #FF0000 — the legacy spelling resolves through the same
        // mapping as the fill/line path.
        assert!(
            emit("4").contains(r##"fill="#0000FF""##),
            "an index Char.Color resolves through the palette table"
        );
        assert!(emit("2").contains(r##"fill="#FF0000""##), "index 2 is red");
        // No Color cell: the built-in default is black.
        let mut s = Shape::default();
        s.sheet.text = Some("plain".into());
        s.sheet.set_cell("Width", Cell::value("2.0"));
        s.sheet.set_cell("TxtWidth", Cell::value("2.0"));
        let chain = [&s];
        let mut out = String::new();
        emit_text(&doc, &s, &chain, 2.0, 1.0, 1.0, 1.0, false, None, &mut out);
        assert!(
            out.contains(r##"fill="#000000""##),
            "default text is black: {out}"
        );
    }

    #[test]
    fn font_family_chains_follow_the_alias_table() {
        // Document family first, then the metric-compatible substitute,
        // then Carlito, then the CSS generic (GitHub #17's example).
        assert_eq!(
            font_family_attr(Some("Arial")),
            " font-family=\"Arial, 'Liberation Sans', Carlito, sans-serif\""
        );
        assert_eq!(
            font_family_attr(Some("Calibri")),
            " font-family=\"Calibri, Carlito, sans-serif\"",
            "Calibri's substitute IS Carlito — no duplicate entry"
        );
        assert_eq!(
            font_family_attr(Some("Times New Roman")),
            " font-family=\"'Times New Roman', 'Liberation Serif', Carlito, serif\""
        );
        assert_eq!(
            font_family_attr(Some("Courier New")),
            " font-family=\"'Courier New', 'Liberation Mono', Carlito, monospace\""
        );
        assert_eq!(
            font_family_attr(Some("Cambria")),
            " font-family=\"Cambria, Caladea, Carlito, serif\""
        );
        // Unknown families: kept as-is, Carlito + sans-serif tail.
        assert_eq!(
            font_family_attr(Some("Wingdings")),
            " font-family=\"Wingdings, Carlito, sans-serif\""
        );
        // Lookup is case-insensitive and trims; no Font cell (or an
        // empty one) falls back to Visio's default family, Calibri.
        assert_eq!(
            font_family_attr(Some("  ARIAL ")),
            " font-family=\"ARIAL, 'Liberation Sans', Carlito, sans-serif\""
        );
        assert_eq!(
            font_family_attr(None),
            " font-family=\"Calibri, Carlito, sans-serif\"",
            "Visio's default family is Calibri"
        );
        assert_eq!(
            font_family_attr(Some("")),
            " font-family=\"Calibri, Carlito, sans-serif\""
        );
    }

    #[test]
    fn char_font_reads_the_first_character_row() {
        let doc = Document::default();
        let s = shape_with_character(Some("Arial"));
        let chain = [&s];
        assert_eq!(char_font(&doc, &s, &chain).as_deref(), Some("Arial"));
        // Numeric (legacy binary index) values name no family.
        let s = shape_with_character(Some("4"));
        let chain = [&s];
        assert_eq!(char_font(&doc, &s, &chain), None);
        // No Font cell / no section at all.
        let s = shape_with_character(None);
        let chain = [&s];
        assert_eq!(char_font(&doc, &s, &chain), None);
        let s = Shape::default();
        let chain = [&s];
        assert_eq!(char_font(&doc, &s, &chain), None);
    }

    #[test]
    fn char_font_inherits_from_the_master_sheet() {
        // The instance's own sheet carries no sections, so the
        // effective sheet is its master's (the Size/Color path).
        let mut root = shape_with_character(Some("Segoe UI"));
        root.id = 1;
        let doc = Document {
            masters: vec![vsdx::Master {
                id: 7,
                shapes: vec![root],
                ..Default::default()
            }],
            ..Document::default()
        };
        let inst = Shape {
            id: 2,
            master: Some(7),
            ..Shape::default()
        };
        assert_eq!(char_font(&doc, &inst, &[]).as_deref(), Some("Segoe UI"));
        // An own Character row wins over the master's.
        let mut inst = Shape {
            id: 3,
            master: Some(7),
            ..Shape::default()
        };
        inst.sheet.children.push(SheetChild::Section(Section {
            name: "Character".into(),
            rows: vec![char_row(Some("Cambria"))],
            ..Section::default()
        }));
        assert_eq!(char_font(&doc, &inst, &[]).as_deref(), Some("Cambria"));
    }

    #[test]
    fn emitted_text_carries_the_font_family_chain() {
        let doc = Document::default();
        let mut s = shape_with_character(Some("Arial"));
        s.sheet.text = Some("hello".into());
        let chain = [&s];
        let mut out = String::new();
        emit_text(&doc, &s, &chain, 2.0, 1.0, 1.0, 1.0, false, None, &mut out);
        assert!(
            out.contains(r#"font-family="Arial, 'Liberation Sans', Carlito, sans-serif""#),
            "emitted text element: {out}"
        );
        assert!(out.contains("hello"), "emitted: {out}");
    }

    /// #124 item 2: a text block whose on-screen em height falls under
    /// TEXT_GREEK_PX emits gray bars (no glyph nodes for usvg to turn
    /// into outlines) and records its size in the greek profile; above
    /// the threshold — or with no ppi at all (export/print/thumbnail
    /// paths pass None) — glyphs emit exactly as before.
    #[test]
    fn text_greeks_below_the_pixel_threshold() {
        let doc = Document::default();
        let tiny = |size: &str| {
            // sizes floor at 0.02 in in resolve_text_block
            let mut s = Shape::default();
            s.sheet.children.push(SheetChild::Section(Section {
                name: "Character".into(),
                rows: vec![Row {
                    cells: vec![("Size".to_string(), Cell::value(size))],
                    ..Row::default()
                }],
                ..Section::default()
            }));
            s.sheet.text = Some("tiny text".into());
            s
        };
        // 0.02 in em (the resolver floor) at 96 ppi = 1.92 px: greeked to bars.
        let s = tiny("0.02");
        let chain = [&s];
        let ctx = GreekCtx::new(96.0);
        let mut out = String::new();
        emit_text(
            &doc,
            &s,
            &chain,
            2.0,
            1.0,
            1.0,
            1.0,
            false,
            Some(&ctx),
            &mut out,
        );
        assert!(
            out.contains(r##"fill="#7f7f7f""##),
            "greek bars emitted: {out}"
        );
        assert!(!out.contains("<text"), "no glyph node when greeked: {out}");
        assert!(!out.contains("<tspan"), "no tspan when greeked: {out}");
        assert_eq!(ctx.sizes.into_inner(), vec![0.02]);
        assert_eq!(greeked_count(&[0.02], 96.0), 1);

        // 0.13889 in em (the 10 pt default) at 96 ppi ≈ 13.3 px: glyphs.
        let s = tiny("0.13889");
        let chain = [&s];
        let ctx = GreekCtx::new(96.0);
        let mut out = String::new();
        emit_text(
            &doc,
            &s,
            &chain,
            2.0,
            1.0,
            1.0,
            1.0,
            false,
            Some(&ctx),
            &mut out,
        );
        assert!(out.contains("tiny text"), "glyph path unchanged: {out}");
        assert!(
            !out.contains("#7f7f7f"),
            "no bars above the threshold: {out}"
        );
        assert_eq!(
            greeked_count(&[0.13889], 96.0),
            0,
            "13 px text does not greek"
        );

        // No greek context (export/print/thumbnails): glyphs regardless
        // of size — greeking never enters those paths.
        let s = tiny("0.02");
        let chain = [&s];
        let mut out = String::new();
        emit_text(&doc, &s, &chain, 2.0, 1.0, 1.0, 1.0, false, None, &mut out);
        assert!(out.contains("tiny text"), "export never greeks: {out}");
    }

    /// Compose an SVG transform list (`translate(a,b) scale(c,d)
    /// rotate(e) …`) into one matrix. SVG applies leftmost-last to the
    /// point, so composition is left-to-right matrix multiplication.
    fn compose_tf(list: &str) -> Mat {
        let mut total = Mat::identity();
        for cmd in list.split(')').filter(|s| !s.trim().is_empty()) {
            let body = cmd.trim().split('(').nth(1).unwrap_or("");
            let n: Vec<f64> = body.split(',').map(|v| v.trim().parse().unwrap()).collect();
            let m = if cmd.trim().starts_with("translate") {
                Mat::translate(n[0], n[1])
            } else if cmd.trim().starts_with("scale") {
                Mat::scale(n[0], n[1])
            } else if cmd.trim().starts_with("rotate") {
                Mat::rotation(n[0].to_radians())
            } else {
                panic!("unhandled transform command: {cmd}");
            };
            total = total.mul(&m);
        }
        total
    }

    /// A text block pinned off-origin, for the TxtPin-pivot tests.
    fn shape_with_pinned_text(txt_angle_deg: f64) -> Shape {
        let mut s = Shape::default();
        let set = |s: &mut Shape, n: &str, v: f64| {
            s.sheet.set_cell(
                n,
                Cell {
                    v: Some(format!("{v}")),
                    ..Cell::default()
                },
            );
        };
        set(&mut s, "TxtPinX", 1.5);
        set(&mut s, "TxtPinY", 0.75);
        set(&mut s, "TxtWidth", 1.0);
        set(&mut s, "TxtHeight", 0.5);
        set(&mut s, "TxtAngle", txt_angle_deg.to_radians());
        s.sheet.text = Some("label".into());
        s
    }

    fn text_wrapper_transform(
        sx: f64,
        sy: f64,
        txt_angle_deg: f64,
    ) -> Option<(String, (f64, f64))> {
        let doc = Document::default();
        let s = shape_with_pinned_text(txt_angle_deg);
        let chain = [&s];
        let mut out = String::new();
        emit_text(&doc, &s, &chain, 2.0, 1.0, sx, sy, false, None, &mut out);
        let t = out
            .split("transform=\"")
            .nth(1)?
            .split('"')
            .next()
            .map(str::to_owned)?;
        Some((t, (1.5, -0.75))) // (list, TxtPin in the emitted y-down frame)
    }

    #[test]
    fn rotated_text_block_pivots_at_txtpin() {
        let Some((t, pin)) = text_wrapper_transform(1.0, 1.0, 30.0) else {
            panic!("rotation must emit a transform wrapper");
        };
        let m = compose_tf(&t);
        let [a, b, c, d, e, f] = m.0;
        let (x, y) = (pin.0 * a + pin.1 * c + e, pin.0 * b + pin.1 * d + f);
        assert!(
            (x - pin.0).abs() < 1e-6 && (y - pin.1).abs() < 1e-6,
            "anchor ({x},{y}) must stay at the TxtPin {pin:?} — transform {t}"
        );
    }

    #[test]
    fn flipped_text_block_pivots_at_txtpin() {
        // FlipX only and both axes: the counter-scale keeps the anchor
        // fixed at the pin in each case.
        for (sx, sy) in [(-1.0, 1.0), (-1.0, -1.0)] {
            let Some((t, pin)) = text_wrapper_transform(sx, sy, 0.0) else {
                panic!("flip must emit a transform wrapper");
            };
            let m = compose_tf(&t);
            let [a, b, c, d, e, f] = m.0;
            let (x, y) = (pin.0 * a + pin.1 * c + e, pin.0 * b + pin.1 * d + f);
            assert!(
                (x - pin.0).abs() < 1e-6 && (y - pin.1).abs() < 1e-6,
                "anchor ({x},{y}) must stay at the TxtPin {pin:?} — transform {t}"
            );
        }
    }

    #[test]
    fn unflipped_unrotated_text_has_no_wrapper() {
        assert!(text_wrapper_transform(1.0, 1.0, 0.0).is_none());
    }

    #[test]
    fn elliptical_arc_half_circle() {
        // Upper half of the unit circle: start (-1,0), on-arc top (0,1),
        // end (1,0), axis angle 0, ratio 1 → rx = ry = 1, minor arc,
        // counter-clockwise in the y-up frame (sweep 0 after mirroring).
        let (rx, ry, rot, large, sweep) =
            solve_ellipse_arc((-1.0, 0.0), (0.0, 1.0), (1.0, 0.0), 0.0, 1.0, (1.0, 1.0)).unwrap();
        assert!((rx - 1.0).abs() < 1e-9);
        assert!((ry - 1.0).abs() < 1e-9);
        assert!(rot.abs() < 1e-9);
        assert!(!large);
        assert_eq!(sweep, 0);
    }

    /// The junction-circle specimen from Analysis/06: MoveTo(0, W/2) then
    /// two EllipticalArcTo rows (W = 0.0393700787…in) that must reconstruct
    /// one shared circle of radius W/2 as two exact half-arcs.
    #[test]
    fn elliptical_arc_junction_circle() {
        let w = 0.03937007874015748;
        // Row 2: arc from (0, W/2) via on-arc point (W/2, W) to (W, W/2).
        let (rx, ry, _, large, sweep) = solve_ellipse_arc(
            (0.0, w / 2.0),
            (w / 2.0, w),
            (w, w / 2.0),
            0.0,
            1.0,
            (1.0, 1.0),
        )
        .unwrap();
        assert!(
            (rx - ry).abs() < 1e-9,
            "ratio 1 ⇒ circle, got rx={rx} ry={ry}"
        );
        assert!((rx - w / 2.0).abs() < 1e-9, "rx={rx} expected={}", w / 2.0);
        assert!(!large, "exact half arc");
        assert_eq!(sweep, 0);
        // Row 3 closes the circle back to (0, W/2) via (W/2, 0).
        let (rx2, ry2, _, large2, sweep2) = solve_ellipse_arc(
            (w, w / 2.0),
            (w / 2.0, 0.0),
            (0.0, w / 2.0),
            0.0,
            1.0,
            (1.0, 1.0),
        )
        .unwrap();
        assert!((rx2 - w / 2.0).abs() < 1e-9);
        assert!((ry2 - w / 2.0).abs() < 1e-9);
        assert!(!large2);
        assert_eq!(sweep2, 0);
    }

    #[test]
    fn elliptical_arc_scaled_circle_becomes_ellipse() {
        // A unit circle scaled ×2 horizontally is an ellipse with rx=2.
        let (rx, ry, _, _, _) =
            solve_ellipse_arc((-1.0, 0.0), (0.0, 1.0), (1.0, 0.0), 0.0, 1.0, (2.0, 1.0)).unwrap();
        assert!((rx - 2.0).abs() < 1e-9, "rx={rx}");
        assert!((ry - 1.0).abs() < 1e-9, "ry={ry}");
    }

    #[test]
    fn transform_ellipse_identity_and_rotation() {
        let (r1, r2, a) = transform_ellipse(2.0, 0.5, 0.3, (1.0, 1.0));
        assert!((r1 - 2.0).abs() < 1e-9 && (r2 - 1.0).abs() < 1e-9);
        assert!((a - 0.3).abs() < 1e-9);

        // Rotating then anisotropically scaling a circle: a circle has no
        // preferred axis, so scaling (2,1) gives rx=2, ry=1 regardless of
        // the incoming axis angle.
        let (r1, r2, a) = transform_ellipse(1.0, 1.0, std::f64::consts::FRAC_PI_2, (2.0, 1.0));
        assert!((r1 - 2.0).abs() < 1e-9 && (r2 - 1.0).abs() < 1e-9);
        assert!(a.abs() < 1e-9);
    }

    /// #12 specimen class: a container-heading shape whose master
    /// Geometry section is gated by a section-level `NoShow` cell, and a
    /// page instance carrying only sparse row caches (no section cells).
    /// The merge must inherit the flag, and the formula must re-evaluate
    /// per instance so a restyled instance shows its band again even
    /// against a stale master cache.
    #[test]
    fn geometry_noshow_inherits_and_formula_reevaluates() {
        let urow = |n: &str, v: f64| Row {
            n: Some(n.into()),
            cells: vec![(
                "Value".into(),
                Cell {
                    v: Some(format!("{v}")),
                    ..Cell::default()
                },
            )],
            ..Row::default()
        };
        let grow = |ix: i64, t: &str, x: f64, y: f64| Row {
            ix,
            t: Some(t.into()),
            cells: vec![
                ("X".into(), Cell::value(format!("{x}"))),
                ("Y".into(), Cell::value(format!("{y}"))),
            ],
            ..Row::default()
        };
        // Heading master: band rect 2.0 × 0.5 with a style-gated NoShow.
        let heading_master = |cstyle: f64, noshow_f: Option<&str>| -> vsdx::Master {
            let mut sh = Shape {
                id: 8,
                ..Shape::default()
            };
            let set = |sheet: &mut Sheet, n: &str, v: f64| {
                sheet.set_cell(n, Cell::value(format!("{v}")));
            };
            set(&mut sh.sheet, "Width", 2.0);
            set(&mut sh.sheet, "Height", 0.5);
            sh.sheet.children.push(SheetChild::Section(Section {
                name: "User".into(),
                rows: vec![urow("hidden", 0.0), urow("CStyle", cstyle)],
                ..Section::default()
            }));
            sh.sheet.children.push(SheetChild::Section(Section {
                name: "Geometry".into(),
                ix: 0,
                cells: vec![
                    ("NoFill".into(), Cell::value("0".to_string())),
                    ("NoLine".into(), Cell::value("0".to_string())),
                    (
                        "NoShow".into(),
                        Cell {
                            v: Some("1".into()),
                            f: noshow_f.map(str::to_string),
                            ..Cell::default()
                        },
                    ),
                ],
                rows: vec![
                    grow(1, "MoveTo", 0.0, 0.0),
                    grow(2, "LineTo", 2.0, 0.0),
                    grow(3, "LineTo", 2.0, 0.5),
                    grow(4, "LineTo", 0.0, 0.5),
                    grow(5, "LineTo", 0.0, 0.0),
                ],
                ..Section::default()
            }));
            vsdx::Master {
                id: 42,
                shapes: vec![sh],
                ..Default::default()
            }
        };
        // Sparse instance: only two row caches, no section-level cells —
        // exactly how Visio writes inline copies of master geometry.
        let inst = Shape {
            id: 1228,
            master: Some(42),
            master_shape: Some(8),
            ..Shape::default()
        };
        let mut inst = inst;
        inst.sheet.set_cell("Width", Cell::value("4.0"));
        inst.sheet.set_cell("Height", Cell::value("0.5"));
        inst.sheet.children.push(SheetChild::Section(Section {
            name: "Geometry".into(),
            ix: 0,
            rows: vec![grow(1, "MoveTo", 0.0, 0.0), grow(2, "LineTo", 4.0, 0.0)],
            ..Section::default()
        }));

        // Plain style (CStyle=1): the formula hides the band — the
        // utilisation.vsdx heading case.
        let doc = Document {
            masters: vec![heading_master(1.0, Some("OR(User.hidden,User.CStyle<>4)"))],
            ..Document::default()
        };
        let mut out = String::new();
        render_all_geometry(&doc, &inst, &[], 4.0, 0.5, &mut out);
        assert!(
            !out.contains("<path"),
            "NoShow'd section must not draw: {out}"
        );

        // Restyled (CStyle=4): the formula re-evaluates to visible even
        // though the master's cached NoShow V=1 says hidden.
        let doc = Document {
            masters: vec![heading_master(4.0, Some("OR(User.hidden,User.CStyle<>4)"))],
            ..Document::default()
        };
        let mut out = String::new();
        render_all_geometry(&doc, &inst, &[], 4.0, 0.5, &mut out);
        assert!(
            out.contains("<path"),
            "formula must beat the stale cache: {out}"
        );
        assert!(
            out.contains("4.00000"),
            "row merge must scale master X into instance units: {out}"
        );

        // Cache-only flag (no formula): the cached V=1 still hides.
        let doc = Document {
            masters: vec![heading_master(4.0, None)],
            ..Document::default()
        };
        let mut out = String::new();
        render_all_geometry(&doc, &inst, &[], 4.0, 0.5, &mut out);
        assert!(!out.contains("<path"), "cached NoShow must hide: {out}");
    }

    // ------------------------------------------------------------------
    // Group XForm composition (GitHub #78): the affine helpers must
    // reproduce the renderer's nested transform math exactly.

    fn xform_shape(cells: &[(&str, f64)]) -> Shape {
        let mut s = Shape::default();
        for (n, v) in cells {
            s.sheet.set_cell(
                n,
                Cell {
                    v: Some(format!("{v}")),
                    ..Cell::default()
                },
            );
        }
        s
    }

    /// A member at parent-local Pin (1,1), 1×1: parent-local rect
    /// (0.5, 0.5, 1, 1).
    fn unit_member() -> Shape {
        let mut m = xform_shape(&[]);
        m.id = 10;
        m
    }

    #[test]
    fn member_page_rect_unrotated_group() {
        // Group 2×4 at Pin (2,3), LocPin centered by default (W=4,H=2
        // → LocPin (2,1)), on an 11-inch page. A member rect of
        // (0.5,0.5,1,1) in group-local y-up maps to the same numbers
        // in page space (translate-only: group-local (0..4, 0..2) ↔
        // page (0..4, 2..4)).
        let doc = Document::default();
        let mut g = xform_shape(&[
            ("PinX", 2.0),
            ("PinY", 3.0),
            ("Width", 4.0),
            ("Height", 2.0),
        ]);
        g.id = 9;
        g.type_ = Some("Group".into());
        let mut m = unit_member();
        for (n, v) in [
            ("PinX", 1.0),
            ("PinY", 1.0),
            ("Width", 1.0),
            ("Height", 1.0),
        ] {
            m.sheet.set_cell(
                n,
                Cell {
                    v: Some(v.to_string()),
                    ..Cell::default()
                },
            );
        }
        let chain = vec![&g, &m];
        assert_eq!(
            shape_page_rect(&doc, &chain, 11.0),
            Some((0.5, 2.5, 1.0, 1.0))
        );
    }

    #[test]
    fn member_page_rect_rotated_group() {
        // The group rotated +90° (page CCW): a member at group-local
        // (1,1) has offset (−1,0) from the LocPin (2,1); CCW rotation
        // maps that to (0,−1), so the member center lands at page
        // Pin(4,5)+(0,−1) = (4,4) and its axis-aligned unit rect is
        // (3.5, 3.5, 1, 1).
        let doc = Document::default();
        let mut g = xform_shape(&[
            ("PinX", 4.0),
            ("PinY", 5.0),
            ("Width", 4.0),
            ("Height", 2.0),
            ("Angle", std::f64::consts::FRAC_PI_2),
        ]);
        g.id = 9;
        g.type_ = Some("Group".into());
        let mut m = unit_member();
        for (n, v) in [
            ("PinX", 1.0),
            ("PinY", 1.0),
            ("Width", 1.0),
            ("Height", 1.0),
        ] {
            m.sheet.set_cell(
                n,
                Cell {
                    v: Some(v.to_string()),
                    ..Cell::default()
                },
            );
        }
        let chain = vec![&g, &m];
        assert_eq!(
            shape_page_rect(&doc, &chain, 11.0),
            Some((3.5, 3.5, 1.0, 1.0))
        );
    }

    #[test]
    fn page_point_round_trips_through_parent_local() {
        let doc = Document::default();
        let mut g = xform_shape(&[
            ("PinX", 4.0),
            ("PinY", 5.0),
            ("Width", 4.0),
            ("Height", 2.0),
            ("Angle", std::f64::consts::FRAC_PI_2),
        ]);
        g.id = 9;
        g.type_ = Some("Group".into());
        let m = unit_member();
        let chain = vec![&g, &m];
        for p in [(5.0, 4.0), (0.0, 0.0), (7.5, 9.25)] {
            let local = page_point_to_parent_local(&doc, &chain, 11.0, p);
            let (sx, sy) = ancestors_affine(&doc, &chain, 11.0).apply(local.0, -local.1);
            assert!((sx - p.0).abs() < 1e-9 && (11.0 - sy - p.1).abs() < 1e-9);
            // And back through the forward helper.
            let back = parent_local_point_to_page(&doc, &chain, 11.0, local);
            assert!((back.0 - p.0).abs() < 1e-9 && (back.1 - p.1).abs() < 1e-9);
        }
        // A top-level chain (no ancestors) is the identity in page
        // space: the parent frame IS the page.
        let top = vec![&g];
        let q = (3.25, 8.5);
        assert_eq!(parent_local_point_to_page(&doc, &top, 11.0, q), q);
    }

    #[test]
    fn top_level_rect_is_identity_composition() {
        // A top-level shape's parent frame is the page: the affine is
        // identity and shape_page_rect matches the plain local rect.
        let doc = Document::default();
        let s = xform_shape(&[
            ("PinX", 3.0),
            ("PinY", 4.0),
            ("Width", 2.0),
            ("Height", 1.0),
        ]);
        let chain = vec![&s];
        assert_eq!(
            shape_page_rect(&doc, &chain, 11.0),
            Some((2.0, 3.5, 2.0, 1.0))
        );
    }

    // ---- #93: per-shape layer bounds + renderer ----

    /// A minimal page: PageWidth/Height sheet plus `shapes`.
    /// #187: the export draws line jumps — a crossing fixture's page
    /// SVG carries the jump group with the white gap, the under
    /// redraw, and an arc that bulges page-up (smaller SVG y).
    #[test]
    fn export_renders_line_jumps() {
        let connector = |id: i64, pin: (f64, f64), w: f64, h: f64| {
            let mut s = shape_at(id, pin, w, h, 0.0);
            s.sheet.set_cell("ObjType", Cell::value("2"));
            let line = |ix: i64, x: f64, y: f64| Row {
                t: Some("LineTo".into()),
                ix,
                cells: vec![
                    ("X".to_string(), Cell::value(format!("{x}"))),
                    ("Y".to_string(), Cell::value(format!("{y}"))),
                ],
                ..Row::default()
            };
            let mut sec = Section {
                name: "Geometry".into(),
                ix: 0,
                ..Section::default()
            };
            sec.rows = vec![
                Row {
                    t: Some("MoveTo".into()),
                    ix: 1,
                    cells: vec![
                        ("X".to_string(), Cell::value("0")),
                        ("Y".to_string(), Cell::value("0")),
                    ],
                    ..Row::default()
                },
                line(2, w, h),
            ];
            s.sheet.children.push(SheetChild::Section(sec));
            s
        };
        // A horizontal connector crossed by a vertical one at (3, 5)
        // strictly inside both segments.
        let (doc, page) = page_fixture(vec![
            connector(1, (3.0, 5.0), 4.0, 0.0),
            connector(2, (3.0, 5.0), 0.0, 4.0),
        ]);
        let svg = render_page(&doc, &page).unwrap();
        assert!(
            svg.contains(r#"data-dc="line-jumps""#),
            "the jump overlay group is present"
        );
        assert!(
            svg.contains(r##"stroke="#ffffff""##),
            "the white gap stroke is present"
        );
        // The arc bulges page-UP: the apex sits at a SMALLER SVG y
        // than the crossing's (page y 5 → SVG y 6 on an 11in page).
        let apex = format!("{:.5} {:.5}", 3.0, 6.0 - crate::geom::JUMP_RADIUS_IN);
        assert!(svg.contains(&apex), "the arc apex is page-up: {apex}");
    }

    /// #251: the page's Show-Line-Jumps switch (`ConLineJumpCode=1`
    /// on the PageSheet) suppresses the jump overlay entirely; the
    /// `LineJumpFactorX` size multiplier scales the hop arc.
    #[test]
    fn export_gates_line_jumps_on_the_page_setting() {
        let connector = |id: i64, pin: (f64, f64), w: f64, h: f64| {
            let mut s = shape_at(id, pin, w, h, 0.0);
            s.sheet.set_cell("ObjType", Cell::value("2"));
            let line = |ix: i64, x: f64, y: f64| Row {
                t: Some("LineTo".into()),
                ix,
                cells: vec![
                    ("X".to_string(), Cell::value(format!("{x}"))),
                    ("Y".to_string(), Cell::value(format!("{y}"))),
                ],
                ..Row::default()
            };
            let mut sec = Section {
                name: "Geometry".into(),
                ix: 0,
                ..Section::default()
            };
            sec.rows = vec![
                Row {
                    t: Some("MoveTo".into()),
                    ix: 1,
                    cells: vec![
                        ("X".to_string(), Cell::value("0")),
                        ("Y".to_string(), Cell::value("0")),
                    ],
                    ..Row::default()
                },
                line(2, w, h),
            ];
            s.sheet.children.push(SheetChild::Section(sec));
            s
        };
        let (doc, _page) = page_fixture(vec![
            connector(1, (3.0, 5.0), 4.0, 0.0),
            connector(2, (3.0, 5.0), 0.0, 4.0),
        ]);
        // Hidden: the PageSheet switch suppresses the group (0 =
        // visPLOJumpNone, the documented off switch per [MS-Learn];
        // #218 review — 6 was never in the enumeration, and a page
        // set to None must not draw hop arcs).
        for off in ["0", "6"] {
            let mut hidden = doc.clone();
            hidden.pages[0]
                .sheet_mut()
                .unwrap()
                .set_cell("LineJumpCode", Cell::value(off));
            let svg = render_page(&hidden, &hidden.pages[0]).unwrap();
            assert!(
                !svg.contains(r#"data-dc="line-jumps""#),
                "the page set to LineJumpCode {off} draws no jump group"
            );
        }
        // Shown at 2×: the hop arc's radius doubles.
        let mut big = doc.clone();
        big.pages[0]
            .sheet_mut()
            .unwrap()
            .set_cell("LineJumpFactorX", Cell::value("2"));
        let svg = render_page(&big, &big.pages[0]).unwrap();
        let apex = format!(
            "{:.5} {:.5}",
            3.0,
            6.0 - crate::geom::JUMP_RADIUS_IN * 2.0
        );
        assert!(svg.contains(&apex), "the 2× arc apex: {apex}");
    }

    /// #187: a page with NO crossings draws no jump group.
    #[test]
    fn export_without_crossings_has_no_jump_group() {
        let connector = |id: i64, pin: (f64, f64), w: f64, h: f64| {
            let mut s = shape_at(id, pin, w, h, 0.0);
            s.sheet.set_cell("ObjType", Cell::value("2"));
            let line = |ix: i64, x: f64, y: f64| Row {
                t: Some("LineTo".into()),
                ix,
                cells: vec![
                    ("X".to_string(), Cell::value(format!("{x}"))),
                    ("Y".to_string(), Cell::value(format!("{y}"))),
                ],
                ..Row::default()
            };
            let mut sec = Section {
                name: "Geometry".into(),
                ix: 0,
                ..Section::default()
            };
            sec.rows = vec![
                Row {
                    t: Some("MoveTo".into()),
                    ix: 1,
                    cells: vec![
                        ("X".to_string(), Cell::value("0")),
                        ("Y".to_string(), Cell::value("0")),
                    ],
                    ..Row::default()
                },
                line(2, w, h),
            ];
            s.sheet.children.push(SheetChild::Section(sec));
            s
        };
        let (doc, page) = page_fixture(vec![connector(1, (3.0, 5.0), 4.0, 0.0)]);
        let svg = render_page(&doc, &page).unwrap();
        assert!(
            !svg.contains(r##"data-dc="line-jumps""##),
            "no crossings, no overlay"
        );
    }

    /// #203: a NURBSTo row's E cell (formula text) — the interior
    /// points/knots/weights ride in `E`, the endpoint in X/Y.
    fn nurbs_row(ix: i64, x: f64, y: f64, e_text: &str) -> Row {
        Row {
            t: Some("NURBSTo".into()),
            ix,
            cells: vec![
                ("X".to_string(), Cell::value(format!("{x}"))),
                ("Y".to_string(), Cell::value(format!("{y}"))),
                ("A".to_string(), Cell::value("0")),
                ("B".to_string(), Cell::value("1")),
                ("C".to_string(), Cell::value("0")),
                ("D".to_string(), Cell::value("1")),
                (
                    "E".to_string(),
                    vsdx::Cell {
                        v: Some(e_text.to_string()),
                        f: Some(e_text.to_string()),
                        ..Cell::default()
                    },
                ),
            ],
            ..Row::default()
        }
    }

    /// #203: the golden fixture page — (1) a SplineStart+2×SplineKnot
    /// cubic, (2) a NURBSTo with uniform weights, (3) a NURBSTo with
    /// NON-uniform weights (the rational case the chord approximates
    /// wrongly). Committed as an SVG golden; regenerate with
    /// UPDATE_SPLINE_GOLDENS=1.
    #[test]
    fn spline_nurbs_goldens() {
        // 1. the spline: MoveTo(0,0), SplineStart (second control
        //    point) with knots 0/0.3/1 degree 3, knots at 0.6/1.
        let mut spline = shape_at(1, (2.0, 2.0), 4.0, 2.0, 0.0);
        push_geo(
            &mut spline,
            vec![
                geo_row(1, "MoveTo", &[("X", 0.0), ("Y", 0.0)]),
                geo_row(
                    2,
                    "SplineStart",
                    &[
                        ("X", 1.0),
                        ("Y", 1.5),
                        ("A", 0.3),
                        ("B", 0.0),
                        ("C", 1.0),
                        ("D", 3.0),
                    ],
                ),
                geo_row(3, "SplineKnot", &[("X", 2.5), ("Y", 0.4), ("A", 0.6)]),
                geo_row(4, "SplineKnot", &[("X", 4.0), ("Y", 2.0), ("A", 1.0)]),
            ],
        );
        // 2. NURBS uniform: M(0,0) → NURBSTo(4,2), interior points
        //    (1.2, 1.6) and (2.8, 0.2), weights 1.
        let mut nurbs_u = shape_at(2, (7.0, 2.0), 4.0, 2.0, 0.0);
        push_geo(
            &mut nurbs_u,
            vec![
                geo_row(1, "MoveTo", &[("X", 0.0), ("Y", 0.0)]),
                nurbs_row(2, 4.0, 2.0, "NURBS(1, 3, 0, 0, 1.2,1.6,0,1, 2.8,0.2,0,1)"),
            ],
        );
        // 3. NURBS rational: the same geometry with the middle
        //    control weighted 2 — the belly pulls toward that point.
        let mut nurbs_r = shape_at(3, (12.0, 2.0), 4.0, 2.0, 0.0);
        push_geo(
            &mut nurbs_r,
            vec![
                geo_row(1, "MoveTo", &[("X", 0.0), ("Y", 0.0)]),
                nurbs_row(2, 4.0, 2.0, "NURBS(1, 3, 0, 0, 1.2,1.6,0,2, 2.8,0.2,0,1)"),
            ],
        );
        let (doc, page) = page_fixture(vec![spline, nurbs_u, nurbs_r]);
        let svg = render_page(&doc, &page).unwrap();

        let regenerate = std::env::var("UPDATE_SPLINE_GOLDENS").is_ok();
        let golden_path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/goldens/spline_nurbs.svg");
        if regenerate {
            let _ = std::fs::create_dir_all(golden_path.parent().unwrap());
            std::fs::write(&golden_path, &svg).unwrap();
        }
        let golden = std::fs::read_to_string(&golden_path)
            .expect("the spline/NURBS golden exists (regenerate with UPDATE_SPLINE_GOLDENS=1)");
        // Git's autocrlf normalizes text files to CRLF on Windows
        // checkouts; the renderer emits LF. Compare newline-normalized.
        let norm = |s: &str| s.replace('\r', "");
        assert_eq!(
            norm(&svg).trim(),
            norm(&golden).trim(),
            "the spline/NURBS golden drifted — if the change is intended, \
             regenerate with UPDATE_SPLINE_GOLDENS=1 and review the diff"
        );
        // Each curve emits a multi-point polyline well above the
        // chord: count `L` segments (16 per knot span, >8 per curve).
        let segments = svg.match_indices("L ").count();
        assert!(
            segments > 24,
            "three flattened curves produce many segments: {segments}"
        );
    }

    fn page_fixture(shapes: Vec<Shape>) -> (Document, Page) {
        let mut sheet = Sheet::default();
        sheet.set_cell("PageWidth", Cell::value("8.5"));
        sheet.set_cell("PageHeight", Cell::value("11"));
        let page = Page {
            id: 0,
            name: "T".into(),
            children: vec![IndexChild::Sheet {
                tag: "PageSheet".into(),
                sheet,
            }],
            shapes,
            ..Page::default()
        };
        (
            Document {
                pages: vec![page.clone()],
                ..Document::default()
            },
            page,
        )
    }

    fn shape_at(id: i64, pin: (f64, f64), w: f64, h: f64, angle_deg: f64) -> Shape {
        let mut sh = Shape {
            id,
            ..Shape::default()
        };
        for (n, v) in [
            ("PinX", pin.0),
            ("PinY", pin.1),
            ("Width", w),
            ("Height", h),
            ("LocPinX", w / 2.0),
            ("LocPinY", h / 2.0),
            ("Angle", angle_deg),
        ] {
            sh.sheet.set_cell(n, Cell::value(format!("{v}")));
        }
        sh
    }

    /// A geometry row built from (type, [(cell, value)]).
    fn geo_row(ix: i64, t: &str, cells: &[(&str, f64)]) -> Row {
        Row {
            t: Some(t.into()),
            ix,
            cells: cells
                .iter()
                .map(|(n, v)| (n.to_string(), Cell::value(format!("{v}"))))
                .collect(),
            ..Row::default()
        }
    }

    fn push_geo(s: &mut Shape, rows: Vec<Row>) {
        let mut sec = Section {
            name: "Geometry".into(),
            ix: 0,
            ..Section::default()
        };
        sec.rows = rows;
        s.sheet.children.push(SheetChild::Section(sec));
    }

    /// #219: the focused bounds of a sub-shape's own subtree — the
    /// damage region a localized edit re-rasters — stay inside the
    /// whole-subtree bounds, equal the member's own box (plus the
    /// stroke pad), and move with the member's cells.
    #[test]
    fn focused_bounds_scope_the_walk_to_one_subtree() {
        let mut member = shape_at(2, (0.4, 0.3), 0.5, 0.4, 0.0);
        push_geo(
            &mut member,
            vec![
                geo_row(1, "MoveTo", &[("X", 0.0), ("Y", 0.0)]),
                geo_row(2, "LineTo", &[("X", 0.5), ("Y", 0.0)]),
                geo_row(3, "LineTo", &[("X", 0.5), ("Y", 0.4)]),
                geo_row(4, "LineTo", &[("X", 0.0), ("Y", 0.4)]),
                geo_row(5, "LineTo", &[("X", 0.0), ("Y", 0.0)]),
            ],
        );
        let mut group = shape_at(1, (2.0, 2.0), 2.0, 1.5, 0.0);
        group.type_ = Some("Group".into());
        let mut far = shape_at(3, (1.8, 1.3), 0.4, 0.3, 0.0);
        push_geo(
            &mut far,
            vec![
                geo_row(1, "MoveTo", &[("X", 0.0), ("Y", 0.0)]),
                geo_row(2, "LineTo", &[("X", 0.4), ("Y", 0.3)]),
            ],
        );
        group.sub_shapes = vec![member, far];
        let (doc, page) = page_fixture(vec![group]);

        let whole = shape_ink_bounds(&doc, &page, 1).unwrap();
        let focused = shape_ink_bounds_focused(&doc, &page, 1, Some(2)).unwrap();
        // The member's box (0.5×0.4 at group-local (0.15..0.65, …))
        // plus the same stroke-bleed pad — not the whole group.
        assert!((focused.2 - (0.5 + 0.16)).abs() < 1e-9, "w={}", focused.2);
        assert!((focused.3 - (0.4 + 0.16)).abs() < 1e-9, "h={}", focused.3);
        // Containment: the focused region sits inside the subtree's.
        assert!(focused.0 >= whole.0 - 1e-9 && focused.1 >= whole.1 - 1e-9);
        assert!(focused.0 + focused.2 <= whole.0 + whole.2 + 1e-9);
        assert!(focused.1 + focused.3 <= whole.1 + whole.3 + 1e-9);

        // The walk is honest about WHICH member: the far sibling's
        // focused bounds sit elsewhere.
        let far_focus = shape_ink_bounds_focused(&doc, &page, 1, Some(3)).unwrap();
        assert!((far_focus.0 - focused.0).abs() > 0.5);
        // An unknown focus yields nothing.
        assert!(shape_ink_bounds_focused(&doc, &page, 1, Some(99)).is_none());
    }

    /// #188: SplineStart + SplineKnot rows render the EVALUATED
    /// B-spline — a multi-segment path that leaves the control-point
    /// chord — not the v0.2 single chord.
    #[test]
    fn spline_rows_render_the_evaluated_curve() {
        let mut s = shape_at(1, (2.0, 2.0), 4.0, 2.0, 0.0);
        // P0=(0,0); P1=SplineStart (1,1); P2/P3=SplineKnots; cubic.
        push_geo(
            &mut s,
            vec![
                geo_row(1, "MoveTo", &[("X", 0.0), ("Y", 0.0)]),
                geo_row(
                    2,
                    "SplineStart",
                    &[
                        ("X", 1.0),
                        ("Y", 1.5),
                        ("A", 0.4),
                        ("B", 0.0),
                        ("C", 1.0),
                        ("D", 3.0),
                    ],
                ),
                geo_row(3, "SplineKnot", &[("X", 2.0), ("Y", 0.5), ("A", 0.6)]),
                geo_row(4, "SplineKnot", &[("X", 4.0), ("Y", 2.0), ("A", 1.0)]),
            ],
        );
        let (doc, page) = page_fixture(vec![s]);
        let svg = render_page(&doc, &page).unwrap();
        let path = svg.match_indices("L ").count();
        assert!(
            path > 8,
            "the spline flattens to many segments, not a chord: {} L segments",
            path
        );
        // The ink leaves the P0→P3 chord (the S bulge through P1/P2).
        // The chord from (0,0) to (4,2): at x=2 the chord y is 1; the
        // spline's belly passes well below (through ~(2, 0.9±)) — assert
        // some emitted coordinate off the chord line instead of exact.
        let off = svg
            .match_indices("L ")
            .map(|(i, _)| &svg[i + 2..i + 30])
            .filter_map(|seg| {
                let mut it = seg.split_whitespace();
                let x: f64 = it.next()?.parse().ok()?;
                let y: f64 = it.next()?.parse().ok()?;
                Some((x, y))
            })
            .find(|(x, y)| (0.5 * x + 2.0 - y).abs() > 0.05);
        assert!(off.is_some(), "some flattened point leaves the chord");
    }

    /// #188: the corpus NURBSTo row (E = NURBS(...) with the row-cell
    /// frame) renders off its chord — the v0.2 single chord is gone.
    #[test]
    fn nurbs_to_row_renders_from_the_e_formula() {
        let mut s = shape_at(1, (2.0, 2.0), 4.0, 2.0, 0.0);
        let e = "NURBS(1, 3, 0, 0, 0.35,0.9,0,1, 0.65,0.1,0,1)";
        push_geo(
            &mut s,
            vec![
                geo_row(1, "MoveTo", &[("X", 0.0), ("Y", 0.0)]),
                geo_row(
                    2,
                    "NURBSTo",
                    &[
                        ("X", 4.0),
                        ("Y", 2.0),
                        ("A", 0.0),
                        ("B", 1.0),
                        ("C", 0.0),
                        ("D", 1.0),
                    ],
                ),
            ],
        );
        // The E cell with its formula text.
        {
            let sec = s
                .sheet
                .children
                .iter_mut()
                .find_map(|c| match c {
                    SheetChild::Section(sec) if sec.name == "Geometry" => Some(sec),
                    _ => None,
                })
                .unwrap();
            let r2 = sec.rows.iter_mut().find(|r| r.ix == 2).unwrap();
            r2.cells.push((
                "E".to_string(),
                Cell {
                    v: Some(e.to_string()),
                    f: Some(e.to_string()),
                    ..Cell::default()
                },
            ));
        }
        let (doc, page) = page_fixture(vec![s]);
        let svg = render_page(&doc, &page).unwrap();
        let segments = svg.match_indices("L ").count();
        assert!(
            segments > 8,
            "the NURBS flattens to many segments: {segments}"
        );
        // Interior points (0.35·4=1.4, 0.9·2=1.8) and (2.6, 0.2) pull
        // the curve off the (0,0)→(4,2) chord; some emitted point must
        // sit off it (chord: y = 0.5x).
        let off = svg
            .match_indices("L ")
            .map(|(i, _)| &svg[i + 2..i + 30])
            .filter_map(|seg| {
                let mut it = seg.split_whitespace();
                let x: f64 = it.next()?.parse().ok()?;
                let y: f64 = it.next()?.parse().ok()?;
                Some((x, y))
            })
            .find(|(x, y)| (0.5 * x + 2.0 - y).abs() > 0.05);
        assert!(off.is_some(), "some flattened point leaves the chord");
    }

    const PAD: f64 = 0.08;

    #[test]
    fn ink_bounds_cover_rotation() {
        // 2×1 shape rotated 90° about its pin (the Angle cell is
        // radians, like render_shape reads it): the ink swaps extents —
        // a plain W×H box (what master_bbox computes) would clip it.
        let (doc, page) = page_fixture(vec![shape_at(
            1,
            (4.0, 5.0),
            2.0,
            1.0,
            std::f64::consts::FRAC_PI_2,
        )]);
        let (x, y, w, h) = shape_ink_bounds(&doc, &page, 1).unwrap();
        assert!((w - (1.0 + 2.0 * PAD)).abs() < 1e-9, "w={w}");
        assert!((h - (2.0 + 2.0 * PAD)).abs() < 1e-9, "h={h}");
        // Same center as the unrotated box: pin (4,5), page_h 11.
        assert!((x + w / 2.0 - 4.0).abs() < 1e-9);
        assert!((y + h / 2.0 - 6.0).abs() < 1e-9);
    }

    #[test]
    fn ink_bounds_union_subshapes_and_text() {
        // A 2×1 group whose sub-shape sits beyond the group's right
        // edge (local pin 2.5 of a 2-wide box) and whose text block is
        // wider than the shape: the union must cover both.
        let mut group = shape_at(1, (4.0, 5.0), 2.0, 1.0, 0.0);
        group
            .sub_shapes
            .push(shape_at(2, (2.5, 0.8), 1.0, 1.0, 0.0));
        group.sheet.text = Some("hello".into());
        group.sheet.set_cell("TxtWidth", Cell::value("3"));
        let (doc, page) = page_fixture(vec![group]);
        let (x, _y, w, h) = shape_ink_bounds(&doc, &page, 1).unwrap();
        // Group box page x∈[3,5]; sub-shape local x∈[2,3] → page
        // x∈[5,6]; text 3 wide centered → x∈[2.5,5.5].
        let right = x + w - PAD;
        assert!((right - 6.0).abs() < 1e-9, "right={right}");
        let left = x + PAD;
        assert!((left - 2.5).abs() < 1e-9, "left={left}");
        assert!(h > 1.0, "h={h}");
    }

    #[test]
    fn layer_svg_scopes_the_viewbox_and_skips_hidden() {
        let (doc, page) = page_fixture(vec![shape_at(1, (4.0, 5.0), 2.0, 1.0, 0.0)]);
        let bounds = (1.0, 2.0, 3.0, 4.0);
        let svg = render_shape_layer(&doc, &page, 1, &[], bounds).unwrap();
        assert!(
            svg.contains(r#"viewBox="1.00000 2.00000 3.00000 4.00000""#),
            "viewBox must be the given bounds: {svg}"
        );
        assert!(svg.contains("<g transform"), "the shape is drawn");
        // A hidden root emits the (empty) svg only — same skip the
        // page renderer applies to hidden members.
        let mut hidden_shape = shape_at(1, (4.0, 5.0), 2.0, 1.0, 0.0);
        hidden_shape.sheet.set_cell("LayerMember", Cell::value("2"));
        let (doc, page) = page_fixture(vec![hidden_shape]);
        let svg = render_shape_layer(&doc, &page, 1, &[2], bounds).unwrap();
        assert!(
            !svg.contains("<g transform"),
            "hidden root must not draw: {svg}"
        );
    }

    /// #236: the culled patch serialization omits every subtree node
    /// whose ink box misses the (padded) cull rect and keeps the ones
    /// it covers — the serialize+parse saving is real, and a
    /// regression to full serialization would re-include the far
    /// shape's ink. The group node itself always stays (its subtree
    /// box spans both leaves).
    #[test]
    fn culled_serialization_omits_shapes_outside_the_cull_rect() {
        // A group with two disjoint leaves: red left, green right.
        // Real Geometry rows, so the leaves actually paint.
        let rect_rows = |w: f64, h: f64| {
            vec![
                geo_row(1, "MoveTo", &[("X", 0.0), ("Y", 0.0)]),
                geo_row(2, "LineTo", &[("X", w), ("Y", 0.0)]),
                geo_row(3, "LineTo", &[("X", w), ("Y", h)]),
                geo_row(4, "LineTo", &[("X", 0.0), ("Y", h)]),
                geo_row(5, "LineTo", &[("X", 0.0), ("Y", 0.0)]),
            ]
        };
        let mut left = shape_at(2, (0.5, 0.5), 0.6, 0.4, 0.0);
        left.sheet.set_cell("FillForegnd", Cell::value("#ff2200"));
        push_geo(&mut left, rect_rows(0.6, 0.4));
        let mut right = shape_at(3, (2.5, 0.5), 0.6, 0.4, 0.0);
        right.sheet.set_cell("FillForegnd", Cell::value("#22cc00"));
        push_geo(&mut right, rect_rows(0.6, 0.4));
        let mut group = shape_at(1, (3.0, 5.0), 3.0, 1.0, 0.0);
        group.sub_shapes = vec![left, right];
        let (doc, page) = page_fixture(vec![group]);
        let bounds = shape_ink_bounds(&doc, &page, 1).unwrap();
        // The cull rect covers the LEFT leaf's x range only (the
        // leaves share their y range, so x does the separating).
        let cull = (bounds.0, bounds.1, 1.4, bounds.3);
        let (_, culled) =
            render_shape_layer_opts_culled(&doc, &page, 1, &[], bounds, None, None, cull).unwrap();
        assert!(
            culled.contains("#ff2200"),
            "the covered leaf stays in the patch: {culled}"
        );
        assert!(
            !culled.contains("#22cc00"),
            "the far leaf is omitted from the patch serialization: {culled}"
        );
        // The full serialization keeps both.
        let (_, full) = render_shape_layer_opts(&doc, &page, 1, &[], bounds, None, None).unwrap();
        assert!(full.contains("#ff2200") && full.contains("#22cc00"));
    }

    /// GitHub #22: the shared TextBlock resolution must agree with the
    /// defaults `emit_text` has always used — absent `Txt*` cells pin
    /// the block at the shape's center, size it to the shape, and take
    /// the Visio built-ins (10 pt, black, centered, 0.1/0.0555 in
    /// insets).
    #[test]
    fn text_block_defaults_match_emit_text() {
        let (doc, page) = page_fixture(vec![shape_at(1, (4.0, 5.0), 2.0, 1.0, 0.0)]);
        let tb = text_block(&doc, &page, 1).unwrap();
        assert_eq!(tb.pin, (1.0, 0.5), "pin defaults to the shape's center");
        assert_eq!(tb.size, (2.0, 1.0), "size defaults to Width/Height");
        assert_eq!(tb.loc_pin, (1.0, 0.5));
        assert_eq!(tb.angle, 0.0);
        assert!((tb.font_size - 0.13888).abs() < 1e-9, "10 pt default");
        assert_eq!(tb.color, "#000000");
        assert!(!tb.bold && !tb.italic);
        assert_eq!(tb.halign, 1, "Visio default HorzAlign = center");
        assert_eq!(tb.valign, 2, "middle");
        assert_eq!(tb.insets, (0.1, 0.1, 0.0555, 0.0555));
        assert!((tb.line_height - tb.font_size * 1.2).abs() < 1e-12);
        assert_eq!(tb.font, None);
    }

    /// Explicit `Txt*` cells and Character/Paragraph styling resolve
    /// through the chain (GitHub #22 — the editor overlay and the
    /// renderer read the same values).
    #[test]
    fn text_block_reads_txt_cells_and_style() {
        let mut sh = shape_at(1, (4.0, 5.0), 2.0, 1.0, 0.0);
        for (n, v) in [
            ("TxtPinX", 1.5),
            ("TxtPinY", 0.25),
            ("TxtWidth", 1.0),
            ("TxtHeight", 0.5),
            ("TxtLocPinX", 0.5),
            ("TxtLocPinY", 0.25),
        ] {
            sh.sheet.set_cell(n, Cell::value(format!("{v}")));
        }
        sh.sheet.children.push(SheetChild::Section(Section {
            name: "Character".into(),
            rows: vec![Row {
                cells: vec![
                    ("Font".into(), Cell::value("Arial")),
                    ("Size".into(), Cell::value("0.27778")), // 20 pt
                    ("Color".into(), Cell::value("#FF0000")),
                    ("Style".into(), Cell::value("1")), // bold
                ],
                ..Row::default()
            }],
            ..Section::default()
        }));
        sh.sheet.children.push(SheetChild::Section(Section {
            name: "Paragraph".into(),
            rows: vec![Row {
                cells: vec![("HorzAlign".into(), Cell::value("2"))],
                ..Row::default()
            }],
            ..Section::default()
        }));
        let (doc, page) = page_fixture(vec![sh]);
        let tb = text_block(&doc, &page, 1).unwrap();
        assert_eq!(tb.pin, (1.5, 0.25));
        assert_eq!(tb.size, (1.0, 0.5));
        assert_eq!(tb.loc_pin, (0.5, 0.25));
        assert_eq!(tb.font.as_deref(), Some("Arial"));
        assert!((tb.font_size - 0.27778).abs() < 1e-12);
        assert_eq!(tb.color, "#FF0000");
        assert!(tb.bold && !tb.italic);
        assert_eq!(tb.halign, 2);
    }

    /// The block's PAGE rect (the editor overlay's footprint): a
    /// default block on an unrotated top-level shape is the shape's
    /// own rect; an explicit smaller block lands pin-anchored
    /// (GitHub #22).
    #[test]
    fn text_block_page_rect_maps_local_block() {
        let (doc, page) = page_fixture(vec![shape_at(1, (4.0, 5.0), 2.0, 1.0, 0.0)]);
        let (x, y, w, h) = text_block_page_rect(&doc, &page, 1).unwrap();
        // Shape rect: pin (4,5) center → (3, 4.5)..(5, 5.5), y-up.
        assert!(
            (x - 3.0).abs() < 1e-9 && (y - 4.5).abs() < 1e-9,
            "x={x} y={y}"
        );
        assert!((w - 2.0).abs() < 1e-9 && (h - 1.0).abs() < 1e-9);

        // Off-center pin: TxtPin (1.5, 0.25) with a centered 1×0.5
        // block → shape-local (1.0, 0.0)..(2.0, 0.5) → page
        // (4.0, 4.5)..(5.0, 5.0) (pin − LocPin = (3.0, 4.5) offset).
        let mut sh = shape_at(2, (4.0, 5.0), 2.0, 1.0, 0.0);
        for (n, v) in [
            ("TxtPinX", 1.5),
            ("TxtPinY", 0.25),
            ("TxtWidth", 1.0),
            ("TxtHeight", 0.5),
        ] {
            sh.sheet.set_cell(n, Cell::value(format!("{v}")));
        }
        let (doc, page) = page_fixture(vec![sh]);
        let (x, y, w, h) = text_block_page_rect(&doc, &page, 2).unwrap();
        assert!(
            (x - 4.0).abs() < 1e-9 && (y - 4.5).abs() < 1e-9,
            "x={x} y={y}"
        );
        assert!((w - 1.0).abs() < 1e-9 && (h - 0.5).abs() < 1e-9);
    }

    /// Count the emitted text lines (tspans) of one shape.
    fn text_lines(out: &str) -> Vec<String> {
        out.split("<tspan")
            .skip(1)
            .map(|l| {
                l.split('>')
                    .nth(1)
                    .unwrap_or("")
                    .split('<')
                    .next()
                    .unwrap_or("")
                    .to_string()
            })
            .collect()
    }

    /// A shape carrying the utilisation.vsdx specimen 1065's exact
    /// text-block cells (#21): TxtWidth 0.8159 in, 8 pt (0.1111 in)
    /// text, zero side margins (inherited from the connector master)
    /// — the authored three-line label the 0.5 em guess re-wrapped
    /// into four (15 chars > its 14-char cap at that width).
    fn shape_1065() -> Shape {
        let mut s = Shape::default();
        s.sheet.text = Some("If allograft\r\nIs implanted or\r\n opened".into());
        // The specimen's connector master inherits zero left/right
        // margins, so the usable width IS the block width.
        for (n, v) in [
            ("Width", "2.0"),
            ("Height", "1.0"),
            ("TxtWidth", "0.8159"),
            ("TxtHeight", "0.6"),
            ("LeftMargin", "0"),
            ("RightMargin", "0"),
        ] {
            s.sheet.set_cell(n, Cell::value(v));
        }
        s.sheet.children.push(SheetChild::Section(Section {
            name: "Character".into(),
            rows: vec![Row {
                cells: vec![("Size".into(), Cell::value("0.1111"))],
                ..Row::default()
            }],
            ..Section::default()
        }));
        s
    }

    /// #21: measured wrap keeps the authored line "Is implanted or"
    /// on one line — real per-glyph advances at 8 pt measure ≈0.70 in
    /// against the 0.8159 in block, where the 0.5 em guess capped the
    /// line at 14 characters and split it. Since #144 the third line's
    /// leading space survives too.
    #[test]
    fn measured_wrap_keeps_the_utilisation_1065_lines() {
        let doc = Document::default();
        let s = shape_1065();
        let chain = [&s];
        let mut out = String::new();
        emit_text(&doc, &s, &chain, 2.0, 1.0, 1.0, 1.0, false, None, &mut out);
        let lines = text_lines(&out);
        assert_eq!(
            lines,
            vec!["If allograft", "Is implanted or", " opened"],
            "the three authored lines survive character-exact: {out}"
        );
    }

    /// #144: lines are character-exact — a paragraph's leading space
    /// run survives (first paragraph included: only the trailing end
    /// of the block is trimmed), interior runs stay, a whitespace-only
    /// paragraph emits its spaces, an empty paragraph stays an empty
    /// line, and the emitted `<text>` carries xml:space="preserve" so
    /// the rasterizer keeps the spacing.
    #[test]
    fn wrap_is_character_exact_on_spaces() {
        let doc = Document::default();
        let mut s = Shape::default();
        s.sheet.set_cell("Width", Cell::value("2.0"));
        s.sheet.set_cell("TxtWidth", Cell::value("2.0"));
        {
            let chain = [&s];
            let mut out = String::new();
            emit_text(&doc, &s, &chain, 2.0, 1.0, 1.0, 1.0, false, None, &mut out);
            assert!(out.is_empty(), "no text, no text element: {out}");
        }

        // First-paragraph leading run, interior run, whitespace-only
        // paragraph, empty paragraph. The final paragraph's trailing
        // space is trimmed with the block's end (corpus texts end with
        // CRLF; that trailing break must not grow the line count).
        s.sheet.text = Some("  opened\r\na  b\n   \n\nend ".into());
        let chain = [&s];
        let mut out = String::new();
        emit_text(&doc, &s, &chain, 2.0, 1.0, 1.0, 1.0, false, None, &mut out);
        assert_eq!(
            text_lines(&out),
            vec!["  opened", "a  b", "   ", "", "end"],
            "every authored space survives into its line: {out}"
        );
        assert!(
            out.contains(r#"<text text-anchor="middle" xml:space="preserve""#),
            "the text element carries xml:space=preserve: {out}"
        );
    }

    /// #257 text-direction: the paragraph base derivation — explicit
    /// U+202A/U+202B overrides decide before the first strong
    /// character; neutrals and numbers never decide.
    #[test]
    fn paragraph_base_follows_marks_then_first_strong() {
        assert!(!paragraph_base_rtl("hello world"), "latin → LTR");
        assert!(paragraph_base_rtl("\u{0633}\u{0644}\u{0627}"), "arabic → RTL");
        assert!(paragraph_base_rtl("\u{202B}hello"), "RLE override → RTL");
        assert!(!paragraph_base_rtl("\u{202A}\u{0633}\u{0644}\u{0627}"), "LRE overrides arabic");
        assert!(!paragraph_base_rtl("123 (x)"), "numbers+neutrals → LTR default");
        assert!(!paragraph_base_rtl("   "), "all-neutral → LTR default");
    }

    /// PR #269 review: RTL paragraphs measure with the RUN-PAIRED
    /// faces (see textmetrics::split_runs) — in this test binary the
    /// vendored fallback is deliberately unregistered (deterministic
    /// pins), so every paragraph measures as ONE run through the same
    /// single-face path as before: the wrap's output for pure-Latin
    /// and pure-Persian paragraphs is unchanged.
    #[test]
    fn rtl_wrap_stays_single_face_without_a_registered_fallback() {
        let latin = crate::textmetrics::resolve(None, false, false);
        let fallback = crate::textmetrics::resolve(Some("Vazirmatn"), false, false);
        let alef = '\u{0627}';
        assert!(!fallback.covers(alef), "unregistered vazirmatn covers nothing");
        let fa = "\u{0633}\u{0644}\u{0627}\u{0645}";
        for para in [format!("\u{202B}{fa}"), format!("\u{202B}abc {fa}")] {
            let lines = wrap_block_text(
                &latin,
                &para,
                0.13889,
                2.0,
                &latin,
                0.13889,
                &fallback,
            );
            assert_eq!(lines.len(), 1);
            assert!(lines[0].rtl, "the base is rtl");
            // No covering fallback → one run → width equals the plain
            // single-face measurement the wrap always used.
            let w = lines[0].text.clone();
            let expect = latin.width_in(&w, 0.13889);
            let got = latin.width_in_runs(&fallback, &w, 0.13889);
            assert!(
                (expect - got).abs() < 1e-9,
                "single-run text keeps byte-identical widths: {expect} vs {got}"
            );
        }
    }

    /// #257 text-direction (PR #269 review): the Right-to-Left Text
    /// toggle round-trips — on marks every paragraph, off strips the
    /// marks AND pins still-RTL paragraphs with an explicit LRE so
    /// "off" is really LTR (the trim-then-ask direction test in the
    /// old dc-ui toggle could never see its own mark).
    #[test]
    fn rtl_toggle_round_trips_and_pins_natural_rtl_ltr() {
        // Latin: on marks every paragraph (CRLF normalizes), off
        // restores the original bytes.
        let on = toggle_rtl_marks("ab\r\ncd", true);
        assert_eq!(on, "\u{202B}ab\n\u{202B}cd");
        assert_eq!(toggle_rtl_marks(&on, false), "ab\ncd");
        // Naturally-RTL text: off strips the RLE and pins an LRE —
        // without the pin the first-strong scan would keep it RTL and
        // the toggle would be a visual no-op.
        let fa = "\u{0633}\u{0644}\u{0627}\u{0645}";
        let marked = format!("\u{202B}{fa}");
        assert_eq!(toggle_rtl_marks(&marked, false), format!("\u{202A}{fa}"));
        // Toggling back on replaces the pin with the RLE.
        assert_eq!(
            toggle_rtl_marks(&format!("\u{202A}{fa}"), true),
            format!("\u{202B}{fa}")
        );
        // An empty paragraph never gains a pin when switching off.
        assert_eq!(toggle_rtl_marks("\u{202B}", false), "");
        // An EMPTY shape pre-sets its direction: the bare mark is the
        // whole text and the first typed paragraph inherits the base.
        assert_eq!(toggle_rtl_marks("", true), "\u{202B}");
        assert_eq!(toggle_rtl_marks("", false), "");
        // The trailing terminator's empty paragraph takes NO mark —
        // a marked empty tail is content to the wrapper and would add
        // a TEXTHEIGHT line to "hello\r\n" (#148 gold, PR #269 review).
        assert_eq!(toggle_rtl_marks("hello\r\n", true), "\u{202B}hello\n");
        assert_eq!(
            toggle_rtl_marks("\u{202B}hello\n", false),
            "hello\n",
            "off restores the plain trailing break"
        );
    }

    /// #257 text-direction: a paragraph whose base is RTL wraps with
    /// the complex-script pair and emits the isolate-wrapped line at
    /// its visual right edge (start alignment), with direction=ltr
    /// pinned on the element.
    #[test]
    fn rtl_paragraph_wraps_and_emits_isolated_at_visual_right() {
        let doc = Document::default();
        let mut s = Shape::default();
        s.sheet.set_cell("Width", Cell::value("3.0"));
        s.sheet.set_cell("TxtWidth", Cell::value("3.0"));
        s.sheet.children.push(SheetChild::Section(Section {
            name: "Paragraph".into(),
            rows: vec![Row {
                ix: 0,
                cells: vec![("HorzAlign".into(), Cell::value("0"))], // start = reading order
                ..Row::default()
            }],
            ..Section::default()
        }));
        s.sheet.text = Some("\u{0633}\u{0644}\u{0627}\u{0645} \u{0639}\u{0644}\u{06CC}\u{06A9}\u{0645}".into());
        let chain = [&s];
        let mut out = String::new();
        emit_text(&doc, &s, &chain, 3.0, 1.0, 1.0, 1.0, false, None, &mut out);
        // The isolate pair wraps the line content.
        assert!(out.contains('\u{2067}'), "the RTL line carries an RLI: {out}");
        assert!(out.contains('\u{2069}'), "the RTL line carries a PDI: {out}");
        // direction=ltr pins the element base for attribute-honoring viewers.
        assert!(out.contains(r#"direction="ltr""#), "element base pinned: {out}");
        // Start alignment under an RTL base puts the line's visual
        // left edge at (right edge - shaped width), NOT at left_x —
        // the tspan x must sit right of the block's left inset edge.
        let x: f64 = out
            .split(r#"<tspan x=""#)
            .nth(1)
            .and_then(|rest| rest.split('"').next())
            .and_then(|v| v.parse().ok())
            .expect("a tspan x");
        // Block pin (1.5,0.5) centered: left edge = tx - lpx + ins_l
        // = 1.5 - 1.5 + 0.1 = 0.1; the RTL line's visual left must be
        // strictly inside the block (right_x - lw > 0.1).
        assert!(x > 0.1, "RTL start line sits at its visual right: x={x}");
        // A second shape with the SAME text but end alignment (2)
        // anchors the line at the block's LEFT edge instead.
        if let Some(SheetChild::Section(sec)) = s.sheet.children.last_mut() {
            sec.rows[0].cells[0] = ("HorzAlign".into(), Cell::value("2"));
        }
        let chain = [&s];
        let mut out2 = String::new();
        emit_text(&doc, &s, &chain, 3.0, 1.0, 1.0, 1.0, false, None, &mut out2);
        let x2: f64 = out2
            .split(r#"<tspan x=""#)
            .nth(1)
            .and_then(|rest| rest.split('"').next())
            .and_then(|v| v.parse().ok())
            .expect("a tspan x");
        assert!(
            (x2 - 0.1).abs() < 1e-6,
            "RTL end line anchors at the visual left inset edge: x2={x2}"
        );
        // PR #269 review: the end-aligned line carries its OWN start
        // anchor (the inherited element "end" would push the glyphs
        // left of the block), and the tspan's font-family is the
        // complete attribute — the old double wrapper nested the
        // quotes. Both are pinned by parsing the emission through the
        // same usvg the rasterizer runs.
        assert!(
            out2.contains(r#" text-anchor="start""#),
            "the end-aligned RTL line anchors itself start: {out2}"
        );
        let full = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10" viewBox="0 0 10 10">{out2}</svg>"#
        );
        // usvg's roxmltree stage rejects malformed attributes — the
        // nested-quote font-family this fixes failed here.
        resvg::usvg::Tree::from_str(&full, &resvg::usvg::Options::default())
            .unwrap_or_else(|e| panic!("RTL emission parses as well-formed SVG: {e}\n{full}"));
    }

    /// #257 text-direction (PR #269 review): RTL lines under a larger
    /// ComplexScriptSize take proportionally taller slots — with
    /// default 1.2 em spacing and 2 em glyphs, the flat line height
    /// overlapped adjacent lines.
    #[test]
    fn larger_complex_script_lines_take_taller_slots() {
        let doc = Document::default();
        let mut s = Shape::default();
        s.sheet.set_cell("Width", Cell::value("3.0"));
        s.sheet.set_cell("TxtWidth", Cell::value("3.0"));
        s.sheet.set_cell("TxtHeight", Cell::value("1.2"));
        s.sheet.children.push(SheetChild::Section(Section {
            name: "Character".into(),
            rows: vec![Row {
                ix: 0,
                cells: vec![("ComplexScriptSize".into(), Cell::value("-200"))],
                ..Row::default()
            }],
            ..Section::default()
        }));
        // Two RTL paragraphs (the explicit mark decides the base).
        let fa = "\u{0633}\u{0644}\u{0627}\u{0645}";
        s.sheet.text = Some(format!("{fa}\r\n{fa}"));
        let chain = [&s];
        let mut out = String::new();
        emit_text(&doc, &s, &chain, 3.0, 1.2, 1.5, 0.6, false, None, &mut out);
        let ys: Vec<f64> = out
            .split(r#" y=""#)
            .skip(1)
            .filter_map(|rest| rest.split('"').next().and_then(|v| v.parse().ok()))
            .collect();
        assert_eq!(ys.len(), 2, "two rtl lines: {out}");
        let (tb, _) = {
            let chain = [&s];
            let tb = resolve_text_block(&doc, &s, &chain, 3.0, 1.2);
            (tb, chain)
        };
        let slot = line_slot_height(&tb, true);
        let naive = tb.line_height;
        assert!(
            slot > naive * 1.5,
            "the double-size cs glyphs take a taller slot: {slot} vs {naive}"
        );
        assert!(
            (ys[1] - ys[0] - slot).abs() < 2e-3,
            "the second line advances by the taller slot: {} vs {slot}",
            ys[1] - ys[0]
        );
        // TEXTHEIGHT agrees with the painted slots (the refreshed
        // cache feeds TxtHeight the same sum).
        let m = crate::textfunc::SheetTextMeasure::new(&doc, &s, &[&s], format!("{fa}\r\n{fa}"));
        let h = {
            use vsdx::formula::TextMeasure as _;
            m.text_height(&format!("{fa}\r\n{fa}"), 3.0).unwrap()
        };
        let ins_tb = tb.insets.2 + tb.insets.3;
        assert!(
            (h - (2.0 * slot + ins_tb)).abs() < 1e-3,
            "TEXTHEIGHT sums the slots (+ insets): {h} vs {}",
            2.0 * slot + ins_tb
        );
    }

    /// PR #269 review: a SMALLER Latin line followed by a larger
    /// complex-script line — the larger line's baseline must rise by
    /// its own ascent, not advance by the Latin slot (which overlapped
    /// the Latin glyphs).
    #[test]
    fn mixed_latin_then_larger_cs_lines_keep_their_ascents() {
        let doc = Document::default();
        let mut s = Shape::default();
        s.sheet.set_cell("Width", Cell::value("3.0"));
        s.sheet.set_cell("TxtWidth", Cell::value("3.0"));
        s.sheet.set_cell("TxtHeight", Cell::value("1.4"));
        s.sheet.children.push(SheetChild::Section(Section {
            name: "Character".into(),
            rows: vec![Row {
                ix: 0,
                cells: vec![("ComplexScriptSize".into(), Cell::value("-200"))],
                ..Row::default()
            }],
            ..Section::default()
        }));
        let fa = "\u{0633}\u{0644}\u{0627}\u{0645}";
        s.sheet.text = Some(format!("one\r\n\u{202B}{fa}"));
        let chain = [&s];
        let mut out = String::new();
        emit_text(&doc, &s, &chain, 3.0, 1.4, 1.5, 0.7, false, None, &mut out);
        let ys: Vec<f64> = out
            .split(r#" y=""#)
            .skip(1)
            .filter_map(|r| r.split('"').next().and_then(|v| v.parse().ok()))
            .collect();
        assert_eq!(ys.len(), 2, "one latin + one rtl line: {out}");
        let tb = resolve_text_block(&doc, &s, &chain, 3.0, 1.4);
        let size = tb.font_size;
        let delta = ys[1] - ys[0];
        // Latin slot (1.2 em) + (cs ascent − latin ascent) = 2.0 em —
        // the old flat advance was the Latin slot's 1.2 em.
        assert!(
            (delta - 2.0 * size).abs() < 2e-3,
            "the larger line rises by its own ascent: {delta} vs {}",
            2.0 * size
        );
    }

    /// #257 text-direction: LTR-only blocks keep the pre-#257 byte
    /// form (no direction attribute, no isolates) — golden stability.
    #[test]
    fn ltr_blocks_emit_without_direction_marks() {
        let doc = Document::default();
        let mut s = Shape::default();
        s.sheet.set_cell("Width", Cell::value("2.0"));
        s.sheet.set_cell("TxtWidth", Cell::value("2.0"));
        s.sheet.text = Some("plain latin".into());
        let chain = [&s];
        let mut out = String::new();
        emit_text(&doc, &s, &chain, 2.0, 1.0, 1.0, 1.0, false, None, &mut out);
        assert!(!out.contains("direction="), "no direction attr: {out}");
        assert!(!out.contains('\u{2067}'), "no isolates: {out}");
    }

    /// #257 text-direction: ComplexScriptFont/Size resolve through the
    /// first Character row with the spec's zero/absent semantics —
    /// a CS face rides the RTL tspan's font-family, and its size swaps
    /// the glyph size when it differs.
    #[test]
    fn complex_script_cells_feed_the_rtl_line() {
        let doc = Document::default();
        let mut s = Shape::default();
        s.sheet.set_cell("Width", Cell::value("3.0"));
        s.sheet.set_cell("TxtWidth", Cell::value("3.0"));
        {
            let chain = [&s];
            let tb = resolve_text_block(&doc, &s, &chain, 3.0, 1.0);
            assert_eq!(tb.cs_font, None, "absent cell → None");
            assert_eq!(tb.cs_size, None, "absent size → None (Size fallback)");
        }
        s.sheet.children.push(SheetChild::Section(Section {
            name: "Character".into(),
            rows: vec![Row {
                ix: 0,
                cells: vec![
                    ("ComplexScriptFont".into(), Cell::value("Vazirmatn")),
                    ("ComplexScriptSize".into(), Cell::value("0.2")),
                ],
                ..Row::default()
            }],
            ..Section::default()
        }));
        let (font, size) = {
            let chain = [&s];
            let tb = resolve_text_block(&doc, &s, &chain, 3.0, 1.0);
            (tb.cs_font, tb.cs_size)
        };
        assert_eq!(font.as_deref(), Some("Vazirmatn"));
        assert_eq!(size, Some(0.2));
        // The negative-size semantics: <0 = |v|·Size/100.
        let size0 = {
            let chain = [&s];
            resolve_text_block(&doc, &s, &chain, 3.0, 1.0).font_size
        };
        if let Some(SheetChild::Section(sec)) = s.sheet.children.last_mut() {
            sec.rows[0].cells[1] = ("ComplexScriptSize".into(), Cell::value("-150"));
        }
        let size = {
            let chain = [&s];
            resolve_text_block(&doc, &s, &chain, 3.0, 1.0).cs_size
        };
        assert_eq!(
            size,
            Some(1.5 * size0),
            "negative → |v|·Size/100"
        );
    }

    /// #144 review: when a block is too narrow for `"a "` and `" b"`,
    /// the wrap breaks inside the `"a  b"` space run — the surviving
    /// space is a line of its own, not an empty line and not a dropped
    /// character.
    #[test]
    fn narrow_wrap_breaking_in_a_space_run_keeps_the_space() {
        let doc = Document::default();
        let size = 0.13889; // the text-block default (10 pt)
        let face = crate::textmetrics::resolve(None, false, false);
        let usable = face.width_in("a", size) + 0.4 * face.width_in(" ", size);
        let mut s = Shape::default();
        s.sheet.text = Some("a  b".into());
        s.sheet.set_cell("Width", Cell::value("2.0"));
        s.sheet
            .set_cell("TxtWidth", Cell::value(format!("{usable:.6}")));
        s.sheet.set_cell("LeftMargin", Cell::value("0"));
        s.sheet.set_cell("RightMargin", Cell::value("0"));
        let chain = [&s];
        let mut out = String::new();
        emit_text(&doc, &s, &chain, 2.0, 1.0, 1.0, 1.0, false, None, &mut out);
        assert_eq!(
            text_lines(&out),
            vec!["a", " ", "b"],
            "the surviving space is its own line: {out}"
        );
    }

    /// #21: a run that genuinely overflows still wraps — measurement
    /// replaces the guess, it does not disable wrapping.
    #[test]
    fn measured_wrap_splits_genuinely_overflowing_runs() {
        let doc = Document::default();
        let mut s = Shape::default();
        s.sheet.text = Some("considerable label text beyond the block".into());
        s.sheet.set_cell("Width", Cell::value("0.8"));
        s.sheet.set_cell("Height", Cell::value("1.0"));
        s.sheet.set_cell("TxtWidth", Cell::value("0.8"));
        let chain = [&s];
        let mut out = String::new();
        emit_text(&doc, &s, &chain, 2.0, 1.0, 1.0, 1.0, false, None, &mut out);
        let lines = text_lines(&out);
        assert!(lines.len() > 1, "must wrap: {lines:?}");
        // The greedy invariant: every line except the last is full —
        // adding the next word would overflow.
        let usable = 0.8 - 0.1 - 0.1;
        let face = crate::textmetrics::resolve(None, false, false);
        for pair in lines.windows(2) {
            let joined = format!("{} {}", pair[0], pair[1]);
            assert!(
                face.width_in(&joined, 0.13889) > usable,
                "line {:?} wrapped too early (usable {usable}): {lines:?}",
                pair[0]
            );
        }
    }

    /// #21: a single word wider than the block hard-splits at the
    /// longest fitting prefix (never a zero-length line, never a
    /// stuck loop).
    #[test]
    fn measured_wrap_hard_splits_single_long_words() {
        let doc = Document::default();
        let mut s = Shape::default();
        s.sheet.text = Some("aaaaaaaaaaaaaaaaaaaaaaaaaa".into());
        s.sheet.set_cell("Width", Cell::value("0.5"));
        s.sheet.set_cell("TxtWidth", Cell::value("0.5"));
        let chain = [&s];
        let mut out = String::new();
        emit_text(&doc, &s, &chain, 2.0, 1.0, 1.0, 1.0, false, None, &mut out);
        let lines = text_lines(&out);
        assert!(
            lines.len() > 1,
            "a 26-em-ish run cannot fit 0.3 in: {lines:?}"
        );
        assert!(
            lines.iter().all(|l| !l.is_empty()),
            "hard splits keep every line non-empty: {lines:?}"
        );
        let joined: String = lines.concat();
        assert_eq!(joined, "aaaaaaaaaaaaaaaaaaaaaaaaaa", "no characters lost");
    }

    /// CRLF and lone CR each break exactly one line (carried over
    /// from the guess era — the normalizer is unchanged).
    #[test]
    fn crlf_and_lone_cr_each_break_one_line() {
        let doc = Document::default();
        let mut s = Shape::default();
        s.sheet.text = Some("one\r\ntwo\rthree".into());
        s.sheet.set_cell("Width", Cell::value("8.0"));
        s.sheet.set_cell("TxtWidth", Cell::value("8.0"));
        let chain = [&s];
        let mut out = String::new();
        emit_text(&doc, &s, &chain, 8.0, 1.0, 1.0, 1.0, false, None, &mut out);
        assert_eq!(
            text_lines(&out),
            vec!["one", "two", "three"],
            "no empty paragraph between CRLF pairs"
        );
    }

    /// #22 review: the in-place editor anchors its caret on the
    /// renderer's own first-baseline formula — pin the numbers so the
    /// helper and `emit_text` can never drift (block 1 in tall, 10 pt
    /// text, default insets, two wrapped lines of 0.1667 in each).
    #[test]
    fn text_first_baseline_matches_the_valign_bands() {
        let mut tb = TextBlock::default();
        tb.size.1 = 1.0;
        tb.font_size = 0.13889;
        tb.insets = (0.1, 0.1, 0.0555, 0.0555);
        let total = 2.0 * 0.16667;
        let ascent = 0.13889 * 0.8;
        tb.valign = 1; // top
        assert!(
            (text_first_baseline(&tb, total, tb.font_size) - (0.0555 + ascent)).abs() < 1e-9
        );
        tb.valign = 2; // middle (default)
        assert!(
            (text_first_baseline(&tb, total, tb.font_size) - ((1.0 - total) / 2.0 + ascent)).abs()
                < 1e-9
        );
        tb.valign = 3; // bottom
        assert!(
            (text_first_baseline(&tb, total, tb.font_size)
                - (1.0 - 0.0555 - total + ascent))
                .abs()
                < 1e-9
        );
        // More wrapped lines raise the first line in the middle band.
        let two = text_first_baseline(&tb, total, tb.font_size);
        let four = text_first_baseline(&tb, total * 2.0, tb.font_size);
        assert!(four < two, "taller text starts its first line higher");
        // PR #269 review: a first line of larger complex-script glyphs
        // ascends from a higher baseline, and its slot (not the flat
        // line height) is what the next line advances by.
        tb.line_height = 0.16667;
        tb.cs_size = Some(tb.font_size * 2.0);
        let eff = effective_cs_size(&tb);
        assert_eq!(eff, tb.font_size * 2.0);
        assert_eq!(line_slot_height(&tb, true), tb.line_height * 2.0);
        assert_eq!(line_slot_height(&tb, false), tb.line_height, "ltr keeps the latin slot");
        // A SMALLER complex-script size never shrinks the slot.
        tb.cs_size = Some(tb.font_size * 0.5);
        assert_eq!(line_slot_height(&tb, true), tb.line_height);
    }

    /// GitHub #22's no-ghost guarantee: `render_page_no_text` drops
    /// exactly the named shape's text and keeps everything else.
    #[test]
    fn render_page_no_text_suppresses_only_that_shape() {
        let mut a = shape_at(1, (2.0, 8.0), 1.0, 1.0, 0.0);
        a.sheet.text = Some("alpha".into());
        let mut b = shape_at(2, (6.0, 8.0), 1.0, 1.0, 0.0);
        b.sheet.text = Some("beta".into());
        let (doc, page) = page_fixture(vec![a, b]);
        let full = render_page(&doc, &page).unwrap();
        assert!(full.contains("alpha") && full.contains("beta"));
        let skipped = render_page_no_text(&doc, &page, &[], 1).unwrap();
        assert!(!skipped.contains("alpha"), "edited shape's text is gone");
        assert!(skipped.contains("beta"), "other shapes keep their text");
        assert!(skipped.contains("<g transform"), "geometry keeps rendering");
    }

    /// #199 + #185: the gallery matches the 600 dpi Visio print (the
    /// authoritative oracle in Analysis/arrow-gallery-visio2016.md) —
    /// the open-head set 14–22 strokes instead of filling, the
    /// dimension set composes ticks with rings/dots, 12 is the single
    /// wide V while 43 is the double chevron, the rear arcs of
    /// 5/6/8/18 are circular, and the bare ends 46–50 have no head.
    #[test]
    fn arrow_gallery_matches_the_visio_print() {
        // The open heads 14–22: stroked paths, never filled.
        for kind in [14, 15, 16, 17, 18, 19, 20, 21, 22] {
            let Some(parts) = arrow_spec(kind) else {
                panic!("arrow {kind} should exist");
            };
            assert!(
                parts.iter().all(|p| !p.filled),
                "arrow {kind} is an open head"
            );
        }
        // 22 is the stroked diamond (4.5 × 2.25 units).
        let parts = arrow_spec(22).unwrap();
        assert!(
            parts[0].d.contains("L-2.25,-1.125 L-4.5,0"),
            "diamond profile"
        );
        // #239: 38's diamond STROKES (the 600 dpi print's interior runs
        // a gap — the earlier "filled" read was the low-res gallery's);
        // its dot stays filled, like 34's ring stays open.
        let d38 = arrow_spec(38).unwrap();
        assert!(
            d38[0].circle.is_some() && d38[0].filled,
            "38's dot fills"
        );
        assert!(
            !d38[1].filled && d38[1].circle.is_none(),
            "38's diamond strokes open"
        );
        // 12 is ONE wide open V; 43 is the double chevron (the print
        // correction — the old table had the two swapped up).
        let v12 = arrow_spec(12).unwrap();
        assert_eq!(v12[0].d, "M-3,-1 L0,0 L-3,1", "12 is the single wide V");
        let v43 = arrow_spec(43).unwrap();
        assert_eq!(v43[0].d.matches('M').count(), 2, "43 is the double chevron");
        // The 5/6/8/18 rears are circular arcs (exact cubic handles);
        // 5/8 concave, 6/18 convex.
        for kind in [5, 8, 6, 18] {
            let parts = arrow_spec(kind).unwrap();
            assert!(
                parts.iter().all(|p| p.d.contains('C')),
                "arrow {kind} carries the circular rear arc"
            );
        }
        let concave = arrow_spec(5).unwrap()[0].d;
        let convex = arrow_spec(6).unwrap()[0].d;
        assert!(concave.contains("C-1.68524"), "5 bows toward the tip");
        assert!(convex.contains("C-2.31476"), "6 bows away from the tip");
        // 39/40: double triangles — filled vs stroked.
        let filled = arrow_spec(39).unwrap();
        assert!(filled.iter().all(|p| p.filled));
        assert_eq!(filled[0].d.matches('M').count(), 2, "two heads");
        let open = arrow_spec(40).unwrap();
        assert!(open.iter().all(|p| !p.filled));
        // 20/41: rings; 42: a filled dot.
        for kind in [20, 41] {
            let parts = arrow_spec(kind).unwrap();
            assert!(parts.iter().all(|p| p.circle.is_some() && !p.filled));
        }
        let dot_parts = arrow_spec(42).unwrap();
        assert!(dot_parts[0].circle.is_some() && dot_parts[0].filled);
        // 25/26: two / three tick strokes in one part.
        let t2 = arrow_spec(25).unwrap();
        assert_eq!(t2[0].d.matches('M').count(), 2);
        let t3 = arrow_spec(26).unwrap();
        assert_eq!(t3[0].d.matches('M').count(), 3);
        // 35: a filled-dot part plus a tick part (composite).
        let comp = arrow_spec(35).unwrap();
        assert_eq!(comp.len(), 2);
        assert!(comp[0].filled && comp[0].circle.is_some());
        assert!(!comp[1].filled && comp[1].circle.is_none());
        // 9 leans "/" through the tip; 23 leans "\" behind it (the
        // print's two slanted ticks oppose).
        assert_eq!(arrow_spec(9).unwrap()[0].d, "M1,-1 L-1,1");
        assert_eq!(arrow_spec(23).unwrap()[0].d, "M-2.9,-1 L-0.9,1");
        // 46–50: bare ends.
        for kind in [46, 47, 48, 49, 50] {
            assert!(arrow_spec(kind).is_none(), "arrow {kind} is bare");
        }
    }

    /// #199 + #185 smoke: a line with EndArrow renders the head from
    /// the verified table at its measured size — the open diamond 22
    /// strokes with its shaft trimmed to the rear, the classic
    /// triangle 4 fills with the shaft under its fill, the zero-weight
    /// line uses the C[size] + 0.75 pt unit, and the bare end 46
    /// carries no head.
    #[test]
    fn arrow_markers_render_from_the_verified_table() {
        let fixture = |kind: i64| -> String {
            let mut shape = Shape {
                id: 1,
                ..Shape::default()
            };
            shape
                .sheet
                .set_cell("EndArrow", Cell::value(kind.to_string()));
            shape.sheet.children.push(SheetChild::Section(Section {
                name: "Geometry".into(),
                rows: vec![
                    Row {
                        t: Some("MoveTo".into()),
                        ix: 0,
                        cells: vec![
                            ("X".into(), Cell::value("0")),
                            ("Y".into(), Cell::value("0")),
                        ],
                        ..Row::default()
                    },
                    Row {
                        t: Some("LineTo".into()),
                        ix: 1,
                        cells: vec![
                            ("X".into(), Cell::value("2")),
                            ("Y".into(), Cell::value("0")),
                        ],
                        ..Row::default()
                    },
                ],
                ..Section::default()
            }));
            let mut page = Page {
                id: 1,
                name: "Page-1".into(),
                ..Page::default()
            };
            page.shapes.push(shape);
            let mut doc = Document::default();
            doc.pages.push(page);
            render_page(&doc, &doc.pages[0]).expect("render")
        };
        // The unit at the default 0.75 pt pen, size 2: 0.75/72 +
        // 2.52/72 = 0.045417 in.
        let unit = 0.75 / 72.0 + 2.52 / 72.0;
        // #239: the diamonds trim TWO PENS short of their rear vertex
        // (the ink-tangency rule), so 22's setback is 4.5·unit − 2·pen.
        let pen = 0.75 / 72.0;
        // The open diamond 22 strokes, and its setback trims the shaft
        // to 2 − (4.5·unit − 2·pen).
        let open22 = fixture(22);
        assert!(open22.contains("dc-ah"), "head attached: {open22}");
        assert!(open22.contains("M0,0 L-2.25,-1.125 L-4.5,0 L-2.25,1.125 Z"));
        assert!(open22.contains("fill=\"none\" stroke=\"#000000\""));
        let cut = 2.0 - (4.5 * unit - 2.0 * pen);
        assert!(
            open22.contains(&format!("{cut:.5}")),
            "the shaft trims two pens short of the diamond's rear: {open22}"
        );
        // 34 (ring + open diamond) trims the same rule under its 6-unit
        // rear; 38's diamond STROKES (the #239 fixture pass: the print's
        // interior is hollow) and trims identically.
        let comp34 = fixture(34);
        let cut34 = 2.0 - (6.0 * unit - 2.0 * pen);
        assert!(
            comp34.contains(&format!("{cut34:.5}")),
            "34 trims two pens short of its rear: {comp34}"
        );
        let comp38 = fixture(38);
        assert!(
            comp38.contains("M-1.5,0 L-3.75,-1.125 L-6,0 L-3.75,1.125 Z\" fill=\"none\""),
            "38's diamond strokes open (the print's interior is hollow): {comp38}"
        );
        assert!(
            comp38.contains(&format!("{cut34:.5}")),
            "38 trims with the diamond rule: {comp38}"
        );
        // The classic triangle 4 fills; its 2-unit setback hides under
        // the fill (trim to 2 − 2·unit).
        let classic4 = fixture(4);
        assert!(classic4.contains("dc-ah"), "head attached: {classic4}");
        assert!(classic4.contains("<path d=\"M0,0 L-2,-1 L-2,1 Z\" fill=\"#000000\"/>"));
        assert!(classic4.contains(&format!("{:.5}", 2.0 - 2.0 * unit)));
        // The bare end 46 carries no head at all.
        let bare46 = fixture(46);
        assert!(!bare46.contains("dc-ah"), "bare ends carry no head");
    }

    /// #185: the shaft trim — a straight line shortens from each end
    /// by the physical setbacks, closed loops and multi-subpath data
    /// stay untouched, a cut that would consume the path keeps the
    /// original, a quadratic splits exactly onto the original curve,
    /// and an arc sub-span lands on the cut angle.
    #[test]
    fn trim_path_cuts_open_contours_only_and_exactly() {
        // Straight line: symmetric and one-sided trims.
        assert_eq!(
            trim_path("M 0 0 L 1 0 ", 0.1, 0.1),
            "M 0.10000 0.00000 L 0.90000 0.00000"
        );
        assert_eq!(
            trim_path("M 0 0 L 1 0 ", 0.0, 0.25),
            "M 0.00000 0.00000 L 0.75000 0.00000"
        );
        assert_eq!(
            trim_path("M 0 0 L 1 0 ", 0.25, 0.0),
            "M 0.25000 0.00000 L 1.00000 0.00000"
        );
        // A closed loop never trims.
        let closed = "M 0 0 L 1 0 L 1 1 L 0 0";
        assert_eq!(trim_path(closed, 0.1, 0.1), closed);
        // Multi-subpath data never trims.
        let multi = "M 0 0 L 1 0 M 2 0 L 3 0";
        assert_eq!(trim_path(multi, 0.1, 0.1), multi);
        // A cut that would consume the whole path keeps the original.
        assert_eq!(trim_path("M 0 0 L 1 0 ", 0.6, 0.6), "M 0 0 L 1 0 ");
        // Malformed data comes back untouched.
        assert_eq!(trim_path("garbage", 1.0, 1.0), "garbage");
        // A quadratic splits exactly: the trimmed sub-curve's endpoint
        // is the original at the cut parameter, and its own middle is
        // the original at half the kept span.
        let q = "M 0 0 Q 1 2 2 0 ";
        let samples = segment_samples('Q', &[1.0, 2.0, 2.0, 0.0], (0.0, 0.0));
        let total = polyline_length(&samples);
        let t_cut = sample_time_at_length(&samples, total - 0.5);
        let trimmed = trim_path(q, 0.0, 0.5);
        let expected_end = quad_at((0.0, 0.0), (1.0, 2.0), (2.0, 0.0), t_cut);
        assert!(
            trimmed.contains(&format!("{:.5} {:.5}", expected_end.0, expected_end.1)),
            "the cut lands on the curve: {trimmed}"
        );
        let q_part = trimmed.split("Q ").nth(1).expect("sub-quadratic");
        let nums: Vec<f64> = q_part
            .split_whitespace()
            .map(|v| v.parse().unwrap())
            .collect();
        let got = quad_at((0.0, 0.0), (nums[0], nums[1]), (nums[2], nums[3]), 0.5);
        let want = quad_at((0.0, 0.0), (1.0, 2.0), (2.0, 0.0), t_cut * 0.5);
        assert!(
            (got.0 - want.0).abs() < 1e-4 && (got.1 - want.1).abs() < 1e-4,
            "the sub-quadratic tracks the original: {got:?} vs {want:?}"
        );
        // An arc sub-span: trimming a semicircle by a quarter of its
        // length lands the endpoint at the 270° point (1, -1), small
        // arc, sweep preserved.
        let a = "M 0 0 A 1 1 0 0 1 2 0 ";
        let trimmed = trim_path(a, 0.0, std::f64::consts::PI / 2.0);
        let a_part = trimmed.split("A ").nth(1).expect("sub-arc");
        let nums: Vec<f64> = a_part
            .split_whitespace()
            .map(|v| v.parse().unwrap())
            .collect();
        assert!(
            (nums[5] - 1.0).abs() < 5e-3 && (nums[6] + 1.0).abs() < 5e-3,
            "the arc cut lands at the quarter point: {trimmed}"
        );
        assert_eq!(nums[3], 0.0, "the quarter arc is not large");
        assert_eq!(nums[4], 1.0, "the sweep survives the split");
    }
}
