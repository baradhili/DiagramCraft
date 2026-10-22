//! SVG rendering backend (the first `Canvas` implementation).
//!
//! Coordinate mapping (Analysis doc 06): VSDX stores inches with origin
//! bottom-left and Y up; SVG is Y down. Each shape becomes a `<g>` with the
//! XForm transform
//!   translate(PinX, PageH-PinY) rotate(-Angle) scale(FlipX,FlipY)
//!   translate(-LocPinX, +LocPinY)
//! applied to geometry whose local Y is negated at path-build time.

use std::fmt::Write as _;

use vsdx::model::Sheet;
#[cfg(test)]
use vsdx::model::{Section, SheetChild};
use vsdx::{Cell, Document, Row, Shape};

const PX_PER_IN: f64 = 96.0;

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
        r#"<!-- DiagramCraft master contact sheet ({} masters) -->"#,
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
            render_shape(doc, shape, master_h, &[], &m.part, &mut svg);
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
        render_shape(doc, shape, master_h, &[], &m.part, &mut svg);
    }
    let _ = writeln!(svg, "</g>");
    let _ = writeln!(svg, "</svg>");
    Ok(svg)
}

pub fn render_page(doc: &Document, page: &vsdx::Page) -> anyhow::Result<String> {
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
        r#"<!-- DiagramCraft (cleanroom VSDX renderer); page '{}' -->"#,
        escape_attr(&page.name)
    );
    let _ = writeln!(
        svg,
        r##"<rect x="0" y="0" width="{w}" height="{h}" fill="none" stroke="#c8c8c8" stroke-width="0.01"/>"##
    );
    // White background under the sheet (Visio page is white).
    let _ = writeln!(
        svg,
        r##"<rect x="0" y="0" width="{w}" height="{h}" fill="#ffffff"/>"##,
    );
    for shape in &page.shapes {
        render_shape(doc, shape, h, &[], &page.part, &mut svg);
    }
    let _ = writeln!(svg, "</svg>");
    Ok(svg)
}

fn render_shape<'a>(
    doc: &'a Document,
    s: &'a Shape,
    page_h: f64,
    chain: &[&'a Shape],
    part: &str,
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
        render_shape_local(doc, sub, &chain, part, out);
    }
    mirror_master(doc, s, &chain, width, height, out);

    emit_text(doc, s, &chain, width, height, sx, sy, out);
    let _ = writeln!(out, "</g>");
}

/// Render a shape whose XForm is relative to its parent's local frame
/// (used for sub-shapes of groups).
fn render_shape_local<'a>(
    doc: &'a Document,
    s: &'a Shape,
    chain: &[&'a Shape],
    part: &str,
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
        render_shape_local(doc, sub, &chain, part, out);
    }
    mirror_master(doc, s, &chain, width, height, out);
    // Group-local rendering applies no flip scale of its own, so the text
    // needs no flip compensation here.
    emit_text(doc, s, &chain, width, height, 1.0, 1.0, out);
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
            render_shape_local(doc, r, &[], mpart, out);
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
            render_shape_local(doc, sub, chain, mpart, out);
            if wrapper.is_some() {
                let _ = writeln!(out, "</g>");
            }
        }
    }
}

/// Evaluation environment for re-evaluating a master root's sub-shape
/// XForms at instance size (issue #15): the group's effective Width /
/// Height under both the bare and the `Sheet.N!` spellings master
/// formulas use, plus the root's User cells — their formulas re-evaluate
/// too, because layout cells like `User.IconY` reference the (instance)
/// `Height`. Instance-local User rows override the master's.
fn mirror_env(root: &Shape, instance: &Shape, width: f64, height: f64) -> vsdx::formula::Env {
    let mut env = vsdx::formula::Env::new();
    env.insert("Width".into(), width);
    env.insert("Height".into(), height);
    env.insert(format!("Sheet.{}!Width", root.id), width);
    env.insert(format!("Sheet.{}!Height", root.id), height);
    // User rows: master root's, overridden by the instance's own (an
    // instance can override editable User rows such as ShapeLayout).
    let mut users: Vec<(String, &Cell)> = Vec::new();
    for user in root.sheet.sections_named("User") {
        for row in user.rows.iter().filter(|r| r.n.is_some()) {
            if let Some(c) = row.cell("Value") {
                users.push((row.n.clone().unwrap(), c));
            }
        }
    }
    for user in instance.sheet.sections_named("User") {
        for row in user.rows.iter().filter(|r| r.n.is_some()) {
            if let Some(c) = row.cell("Value") {
                if let Some(slot) = users
                    .iter_mut()
                    .find(|(n, _)| n == row.n.as_deref().unwrap())
                {
                    slot.1 = c;
                } else {
                    users.push((row.n.clone().unwrap(), c));
                }
            }
        }
    }
    let set = |env: &mut vsdx::formula::Env, name: &str, v: f64| {
        env.insert(format!("User.{name}"), v);
        env.insert(format!("Sheet.{}!User.{name}", root.id), v);
    };
    for (name, c) in &users {
        if let Some(v) = c.numeric().filter(|v| v.is_finite()) {
            set(&mut env, name, v);
        }
    }
    // User formulas re-evaluate against the instance dims (e.g. IconY =
    // IF(OR(User.ShapeLayout<>1), Height - User.InfoPanelHeight/2, …)).
    // Two passes let a User formula reference another User formula.
    for _ in 0..2 {
        let mut changed = false;
        for (name, c) in &users {
            let Some(f) = c.f.as_deref() else { continue };
            if f == "Inh" || f == "No Formula" {
                continue;
            }
            if let Some(v) = vsdx::formula::eval(f, &env)
                .ok()
                .and_then(|v| v.as_f64())
                .filter(|v| v.is_finite())
            {
                let cur = env.get(&format!("User.{name}")).copied();
                if cur != Some(v) {
                    set(&mut env, name, v);
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
    env
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
    let mut env = root_env.clone();
    let eff = |name: &str, axis: f64, env: &vsdx::formula::Env| -> Option<f64> {
        let Some(c) = sub.sheet.cell(name) else {
            return None;
        };
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
fn emit_text(
    doc: &Document,
    s: &Shape,
    chain: &[&Shape],
    width: f64,
    height: f64,
    sx: f64,
    sy: f64,
    out: &mut String,
) {
    let Some(text) = s
        .sheet
        .text
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty())
    else {
        return;
    };
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
    let size = char_size(doc, s, &chain).unwrap_or(0.13888).max(0.02);
    // First Character row extras: color (palette/theme aware) and the
    // Style bitmask (1 = bold, 2 = italic). Both resolve per-cell through
    // `char_cell`, so a LangID-only instance row picks them up from the
    // master/style rows instead of silently taking the built-in defaults.
    let color = char_cell(doc, s, chain, "Color")
        .and_then(|c| c.v.clone())
        .map(|v| match v.trim() {
            v if v.starts_with('#') => v.to_string(),
            v => v
                .parse::<i64>()
                .ok()
                .map(|idx| doc.color(idx).cloned().unwrap_or_else(|| palette(idx)))
                .unwrap_or_else(|| "#000000".into()),
        })
        .unwrap_or_else(|| "#000000".into());
    let style_bits = char_cell(doc, s, chain, "Style")
        .and_then(|c| c.numeric())
        .unwrap_or(0.0) as u32;
    let font_attrs = format!(
        r#"font-size="{s:.4}"{ff} fill="{c}"{b}{i}"#,
        s = size,
        ff = font_family_attr(char_font(doc, s, chain).as_deref()),
        c = color,
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
    let line_h = get("LineSpacing")
        .map(|l| l * size)
        .unwrap_or(size * 1.2)
        .max(size);

    // Greedy word wrap. Average glyph advance for proportional sans at
    // mixed case is ~0.5 em; Calibri runs slightly narrower.
    let max_chars = (((tw - ins_l - ins_r) / (size * 0.5)).floor() as usize).max(1);
    let mut lines: Vec<String> = Vec::new();
    // Visio stores line breaks as CRLF: breaking on each of \r and \n
    // separately would insert an empty paragraph between every pair of
    // lines, so normalize CRLF (and lone CR) to one break first.
    for para in text.replace("\r\n", "\n").replace('\r', "\n").split('\n') {
        if para.trim().is_empty() {
            lines.push(String::new());
            continue;
        }
        let mut cur = String::new();
        for word in para.split(' ') {
            let candidate_len = cur.len() + word.len() + !cur.is_empty() as usize;
            if !cur.is_empty() && candidate_len > max_chars {
                lines.push(cur.clone());
                cur.clear();
            }
            // A single word longer than the line: hard-split.
            let mut w = word;
            while w.len() > max_chars {
                let (head, rest) = w.split_at(max_chars);
                if !cur.is_empty() {
                    lines.push(cur.clone());
                    cur.clear();
                }
                lines.push(head.to_string());
                w = rest;
            }
            if !cur.is_empty() {
                cur.push(' ');
            }
            cur.push_str(w);
        }
        lines.push(cur);
    }
    let n_lines = lines.len() as f64;
    let total_h = n_lines * line_h;

    // Text-block rect from the sheet: the pin (TxtPinX/TxtPinY, y-up local)
    // sits TxtLocPinX/TxtLocPinY from the block's lower-left corner, so that
    // corner is (tx - lpx, ty - lpy) — masters can pin off-center, so no
    // centered-pin assumption. In the emitted y-negated frame the top edge
    // is -(ty - lpy + th).
    let lpx = get("TxtLocPinX").unwrap_or(tw / 2.0);
    let lpy = get("TxtLocPinY").unwrap_or(th / 2.0);
    let top_edge = -(ty - lpy + th);
    let first_baseline = match valign {
        1 => top_edge + ins_t + size * 0.8,                // top
        3 => top_edge + th - ins_b - total_h + size * 0.8, // bottom
        _ => top_edge + (th - total_h) / 2.0 + size * 0.8, // middle
    };
    // Line x per alignment: left edge of the text area, center, or right.
    let left_x = tx - lpx + ins_l;
    let (anchor, x) = match halign {
        0 | 3 => ("start", left_x),
        2 => ("end", left_x + (tw - ins_l - ins_r)),
        _ => ("middle", left_x + (tw - ins_l - ins_r) / 2.0),
    };

    let mut body = String::new();
    for (i, line) in lines.iter().enumerate() {
        let y = first_baseline + i as f64 * line_h;
        if i == 0 {
            let _ = write!(
                body,
                r#"<tspan x="{x:.4}" y="{y:.4}">{}</tspan>"#,
                escape_text(line)
            );
        } else {
            let _ = write!(
                body,
                r#"<tspan x="{x:.4}" y="{y:.4}">{}</tspan>"#,
                escape_text(line)
            );
        }
    }
    // Flip-aware block rotation (issue #19): the counter-scale keeps the
    // glyphs un-mirrored inside the caller's flipped frame, and the
    // rotation composes with flip parity so the page-space net is
    // Angle + sx*sy*TxtAngle.
    let flipped = sx < 0.0 || sy < 0.0;
    let net_txt_deg = -(sx * sy) * angle.to_degrees();
    let mut transform = format!("translate({x:.5},{y:.5})", x = tx, y = -ty);
    if flipped {
        let _ = write!(transform, " scale({sx:.0},{sy:.0})", sx = sx, sy = sy);
    }
    if net_txt_deg.abs() > 1e-6 {
        let _ = write!(transform, " rotate({deg:.5})", deg = net_txt_deg);
    }
    if flipped || net_txt_deg.abs() > 1e-6 {
        let _ = writeln!(out, r#"<g transform="{}">"#, escape_attr(&transform));
        let _ = writeln!(
            out,
            r#"<text text-anchor="{a}" {f}>{b}</text>"#,
            a = anchor,
            f = font_attrs,
            b = body
        );
        let _ = writeln!(out, "</g>");
    } else {
        let _ = writeln!(
            out,
            r#"<text text-anchor="{a}" {f}>{b}</text>"#,
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
    let pkg = doc.package.as_ref()?;
    let (_, _, target) = pkg
        .relationships(part)
        .into_iter()
        .find(|(id, ty, _)| id == &rid && ty.ends_with("/image"))?;
    // `relationships` returns the target already resolved to an absolute
    // package part name.
    let bytes = pkg.part(target.trim_start_matches('/'))?;
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

fn base64(data: &[u8]) -> String {
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
                        let Some(sv) = scell.numeric().filter(|v| placed(*v).abs() <= bound) else {
                            continue;
                        };
                        let patched = Cell::value(format!("{}", placed(sv)));
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
        for geo in &merged {
            // Instance frame: formulas and cached values are both in
            // instance units.
            render_geometry(doc, s, &chain, geo, (1.0, 1.0), Frame::Instance, out);
        }
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
    for geo in src.sheet.sections_named("Geometry") {
        // Master frame: rows are master-local; their formulas reference
        // the MASTER's dims and must not be re-evaluated in instance
        // units (that skews rectangles into triangles).
        render_geometry(doc, s, &chain, geo, scale, Frame::Master(src), out);
    }
}

/// Which frame geometry rows live in — picks the formula environment.
enum Frame<'a> {
    Instance,
    Master(&'a Shape),
}

fn render_geometry(
    doc: &Document,
    s: &Shape,
    chain: &[&Shape],
    section: &vsdx::Section,
    scale: (f64, f64),
    frame: Frame<'_>,
    out: &mut String,
) {
    // Section-level cells carry NoFill/NoLine/etc.
    let flag = |name: &str| -> bool {
        section
            .cell(name)
            .and_then(|c| c.numeric())
            .map(|v| v != 0.0)
            .unwrap_or(false)
    };
    let no_fill = flag("NoFill");
    let no_show = flag("NoShow");
    if no_show {
        return;
    }

    let rows = &section.rows;
    let (width, height) = shape_size(doc, s, chain);
    // Formulas evaluate in the geometry's own frame: instance dims for
    // inline geometry, the master source's dims for inherited geometry.
    // The cached V already lives in that frame; evaluating a formula in
    // the WRONG frame (e.g. master rows against instance Width) skews
    // rectangles into triangles.
    let env = match frame {
        Frame::Instance => shape_env_for(doc, s, chain, rows),
        Frame::Master(src) => shape_env_for(doc, src, &[], rows),
    };
    // Visio hides the outline when LinePattern = 0 (e.g. legend label
    // boxes carry THEMEGUARD(0)); the geometry still exists for hit
    // testing but must not stroke.
    let line_pattern = doc
        .resolve_cell(s, chain, "LinePattern")
        .and_then(|c| cell_num(&c, &env));
    let no_line = flag("NoLine") || line_pattern.is_some_and(|p| p == 0.0);
    let row_num = |r: &Row, c: &str| -> Option<f64> {
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
                        vsdx::formula::eval(f, &env).ok().and_then(|v| v.as_f64())
                    }
                })
                .filter(|v| v.is_finite());
            by_formula.or_else(|| cell.numeric())
        })
    };

    let fill_color = themed_color_string(doc, s, chain, "FillForegnd");
    let line_color = themed_color_string(doc, s, chain, "LineColor");
    let fill_bkgnd = themed_color_string(doc, s, chain, "FillBkgnd");
    let line_weight = doc
        .resolve_cell(s, chain, "LineWeight")
        .and_then(|c| cell_num(&c, &env))
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
            .and_then(|c| cell_num(&c, &env))
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
            .and_then(|c| cell_num(&c, &env))
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

    for row in rows.iter().filter(|r| r.t.is_some()) {
        let Some(t) = row.t.as_deref() else { continue };
        let (mx, my) = (
            row_num(row, "X").unwrap_or(0.0),
            row_num(row, "Y").unwrap_or(0.0),
        );
        let (x, y) = rel_adjust(t, mx, my, width, height);
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
                let h = row_num(row, "A").unwrap_or(0.0);
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
                let m = (
                    row_num(row, "A").unwrap_or((cur.0 + x) / 2.0),
                    row_num(row, "B").unwrap_or((cur.1 + y) / 2.0),
                );
                let c_ang = row_num(row, "C").unwrap_or(0.0);
                let ratio = row_num(row, "D").unwrap_or(1.0).abs().max(1e-6);
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
                        // Flat / degenerate: quadratic Bézier through m.
                        let q = emit_pt(m);
                        let e = emit_pt((x, y));
                        let _ = write!(d, "Q {} {}", q.trim_end(), e.trim_end());
                    }
                }
                cur = (x, y);
            }
            "Ellipse" => {
                // (X,Y) center, (A,B) major-axis endpoint, (C,D) minor-axis endpoint.
                let a = row_num(row, "A").unwrap_or(x + 1.0);
                let b = row_num(row, "B").unwrap_or(y);
                let c2 = row_num(row, "C").unwrap_or(x);
                let d2 = row_num(row, "D").unwrap_or(y + 1.0);
                let rx = ((a - x).powi(2) + (b - y).powi(2)).sqrt().max(1e-6);
                let ry = ((c2 - x).powi(2) + (d2 - y).powi(2)).sqrt().max(1e-6);
                let ang = (b - y).atan2(a - x); // major-axis angle, y-up frame
                                                // Anisotropic instance scaling of the ellipse.
                let (rx2, ry2, ang2) =
                    transform_ellipse(rx.max(ry), (rx.min(ry)) / rx.max(ry), ang, scale);
                let (rx2, ry2) = if rx >= ry { (rx2, ry2) } else { (ry2, rx2) };
                let p = sc((x, y));
                let _ = writeln!(
                    out,
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
                    row_num(row, "A").unwrap_or(cur.0),
                    row_num(row, "B").unwrap_or(cur.1),
                    width,
                    height,
                );
                let (c2, d2) = rel_adjust(
                    t,
                    row_num(row, "C").unwrap_or(x),
                    row_num(row, "D").unwrap_or(y),
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
                    row_num(row, "A").unwrap_or((cur.0 + x) / 2.0),
                    row_num(row, "B").unwrap_or((cur.1 + y) / 2.0),
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
            "SplineStart" | "SplineKnot" | "NURBSTo" | "PolylineTo" | "InfiniteLine" => {
                // v0.2: approximate splines/polylines with chords.
                let _ = write!(d, "L {}", emit_pt((x, y)));
                cur = (x, y);
            }
            _ => {}
        }
    }
    if !d.is_empty() {
        // Connector arrowheads: BeginArrow/EndArrow cells (0 = none, ≥1 an
        // arrow; sizes 0–6, 2 = medium). One marker def per distinct
        // (direction, color, weight, size), reused across shapes.
        let begin_arrow = doc
            .resolve_cell(s, chain, "BeginArrow")
            .and_then(|c| cell_num(&c, &env))
            .unwrap_or(0.0);
        let end_arrow = doc
            .resolve_cell(s, chain, "EndArrow")
            .and_then(|c| cell_num(&c, &env))
            .unwrap_or(0.0);
        let bsz = doc
            .resolve_cell(s, chain, "BeginArrowSize")
            .and_then(|c| cell_num(&c, &env))
            .unwrap_or(2.0);
        let esz = doc
            .resolve_cell(s, chain, "EndArrowSize")
            .and_then(|c| cell_num(&c, &env))
            .unwrap_or(2.0);
        let mut attrs = String::new();
        let mut marker = |begin: bool, arrow: f64, sz: f64, out_all: &mut String| {
            if arrow < 0.5 {
                return;
            }
            let scale = 2.5 + sz.max(0.0) * 1.5;
            let id = format!(
                "dc-ah{}{:x}",
                if begin { 'b' } else { 'e' },
                fnv(&format!("{}|{line_weight:.5}|{scale:.2}", line_color))
            );
            if !out_all.contains(&format!(r#"id="{id}""#)) {
                // Triangle along +x for end markers, -x for begin markers
                // (both orient=auto relative to path direction).
                let path = if begin {
                    "M10,0 L0,5 L10,10 z"
                } else {
                    "M0,0 L10,5 L0,10 z"
                };
                let _ = writeln!(
                    out_all,
                    r#"<defs><marker id="{id}" viewBox="0 0 10 10" refX="{rx}" refY="5" markerWidth="{scale}" markerHeight="{scale}" markerUnits="strokeWidth" orient="auto"><path d="{path}" fill="{line_color}" stroke="{line_color}" stroke-width="0.5"/></marker></defs>"#,
                    rx = if begin { 0.0 } else { 10.0 },
                );
            }
            if begin {
                attrs.push_str(&format!(r#" marker-start="url(#{id})""#));
            } else {
                attrs.push_str(&format!(r#" marker-end="url(#{id})""#));
            }
        };
        marker(true, begin_arrow, bsz, out);
        marker(false, end_arrow, esz, out);
        let _ = writeln!(
            out,
            r#"<path d="{}" {style}{attrs}/>"#,
            escape_attr(d.trim()),
            style = style,
            attrs = attrs
        );
    }
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
    let stroke = if no_line {
        r#"stroke="none""#.to_string()
    } else {
        let dash = dash
            .map(|d| format!(r#" stroke-dasharray="{d}""#))
            .unwrap_or_default();
        format!(
            r#"stroke="{}" stroke-width="{:.5}"{dash}"#,
            escape_attr(line_color),
            line_weight.max(0.001)
        )
    };
    format!(r#"fill="{}" {}"#, escape_attr(&fill), stroke)
}

/// Visio's built-in dash/dot line patterns (LinePattern 2–23) as SVG
/// `stroke-dasharray` values in LineWeight multiples (patterns scale with
/// the line weight — the ShapeSheet gallery's defining property). Numbers
/// follow the ECMA-376 preset-dash primitives (`dash` 4,3 · `lgDash` 8,3 ·
/// `dot` 1,3), composed per the Visio gallery semantics confirmed against
/// labeled Visio 2016 reference crops (issue #13): 4 = dash-dot, 5 =
/// dash-dot-dot, 7 = long dash + normal dash, 8 = long dash + two normal
/// dashes. The crops' long dash measures ≈12w; the ECMA `lgDash` (8w) is
/// used per owner decision — flip that constant if 7/8 read too short
/// against the gold. Patterns outside this set stay solid until a corpus
/// exercises them.
fn line_dasharray(pattern: f64, weight: f64) -> Option<String> {
    let spec: &[f64] = match pattern as i64 {
        2 => &[8.0, 3.0],                     // dashed (lgDash)
        3 => &[1.0, 3.0],                     // dotted (dot)
        4 => &[4.0, 3.0, 1.0, 3.0],           // dash-dot (dashDot)
        5 => &[4.0, 3.0, 1.0, 3.0, 1.0, 3.0], // dash-dot-dot (dashDotDot)
        7 => &[8.0, 3.0, 4.0, 3.0],           // long dash + normal dash
        8 => &[8.0, 3.0, 4.0, 3.0, 4.0, 3.0], // long + 2× normal dashes
        _ => return None,
    };
    Some(
        spec.iter()
            .map(|m| format!("{:.5}", (m * weight).max(1e-4)))
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
fn themed_color_string(doc: &Document, s: &Shape, chain: &[&Shape], cell: &str) -> String {
    let resolved = doc.resolve_cell(s, chain, cell);
    if let Some(c) = &resolved {
        if let Some(f) = c.f.as_deref() {
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
    }
    match resolved {
        Some(c) => match c.v.as_deref() {
            Some(v) if v.starts_with('#') => v.to_string(),
            Some(v) => match v.trim().parse::<i64>() {
                Ok(idx) => doc.color(idx).cloned().unwrap_or_else(|| palette(idx)),
                Err(_) => "#000000".into(),
            },
            None => "#000000".into(),
        },
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
/// it contains anything but letters, digits and dashes).
fn quote_family(name: &str) -> String {
    if name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        name.to_string()
    } else {
        format!("'{name}'")
    }
}

fn push_family(chain: &mut Vec<String>, family: &str) {
    if !chain.iter().any(|f| f.eq_ignore_ascii_case(family)) {
        chain.push(quote_family(family));
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
fn font_family_attr(font: Option<&str>) -> String {
    let name = match font.map(str::trim) {
        Some(f) if !f.is_empty() => f,
        _ => "Calibri",
    };
    let (substitute, class) = match font_substitute(name) {
        Some((s, c)) => (Some(s), c),
        None => (None, "sans-serif"),
    };
    let mut chain = vec![quote_family(name)];
    if let Some(s) = substitute {
        push_family(&mut chain, s);
    }
    push_family(&mut chain, "Carlito");
    chain.push(class.to_string());
    format!(" font-family=\"{}\"", chain.join(", "))
}

/// Resolved (Width, Height) for geometry scaling.
fn shape_size(doc: &Document, s: &Shape, chain: &[&Shape]) -> (f64, f64) {
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
    let sheet = doc.effective_sheet(s, chain);
    for user in sheet.sections_named("User") {
        for row in user.rows.iter().filter(|r| r.n.is_some()) {
            if let Some(v) = row.cell("Value").and_then(|c| c.numeric()) {
                env.insert(format!("User.{}", row.n.as_deref().unwrap_or("")), v);
            }
        }
    }
    // Geometry row coordinates (Geometry1.X1 style references).
    for (i, geo) in sheet.sections_named("Geometry").iter().enumerate() {
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

    /// Synthetic Role-callout-shaped master (issue #15 specimen, values
    /// recomputed — no Microsoft content): a root group whose children
    /// mix proportional XForms (`Sheet.5!Width*1`) with GUARDed absolute
    /// ones (`GUARD(4MM)`), driven by User cells whose formulas reference
    /// the group Height.
    fn role_master_fixture() -> (Shape, Shape) {
        let mut set = |sheet: &mut Sheet, n: &str, v: f64, f: Option<&str>| {
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
        // ECMA-376 preset primitives (issue #13): dash 4,3 · lgDash 8,3 ·
        // dot 1,3, composed per the Visio gallery semantics.
        let w = 0.01041666666666667; // 0.75 pt
        assert_eq!(line_dasharray(2.0, w).as_deref(), Some("0.08333 0.03125"));
        assert_eq!(line_dasharray(3.0, w).as_deref(), Some("0.01042 0.03125"));
        assert_eq!(
            line_dasharray(4.0, w).as_deref(),
            Some("0.04167 0.03125 0.01042 0.03125")
        );
        assert_eq!(
            line_dasharray(5.0, w).as_deref(),
            Some("0.04167 0.03125 0.01042 0.03125 0.01042 0.03125")
        );
        // 7/8: long dash (lgDash) + normal dash(es).
        assert_eq!(
            line_dasharray(7.0, w).as_deref(),
            Some("0.08333 0.03125 0.04167 0.03125")
        );
        assert_eq!(
            line_dasharray(8.0, w).as_deref(),
            Some("0.08333 0.03125 0.04167 0.03125 0.04167 0.03125")
        );
        // Solid (1), hidden (0, handled elsewhere) and uncalibrated
        // patterns render solid — no dasharray attribute.
        assert!(line_dasharray(1.0, w).is_none());
        assert!(line_dasharray(23.0, w).is_none());
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
        emit_text(&doc, &s, &chain, 2.0, 1.0, 1.0, 1.0, &mut out);
        assert!(
            out.contains(r#"font-family="Arial, 'Liberation Sans', Carlito, sans-serif""#),
            "emitted text element: {out}"
        );
        assert!(out.contains("hello"), "emitted: {out}");
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
}
