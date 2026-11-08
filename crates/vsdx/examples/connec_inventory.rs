//! #238 investigation helper: classify every master of a connector
//! stencil by geometry family — row types, row count, frame/guard
//! cells, glue/route cells and connection points — so the drag & glue
//! exercise can pick representatives per family and the failures can
//! be split into per-family fix issues.
//!
//! Read-only over the stencil; prints a TSV-ish table plus a family
//! rollup. Usage:
//!
//! ```text
//! cargo run -p vsdx --example connec_inventory -- \
//!     crates/dc-ui/assets/stencils/connec.vssx
//! ```

use std::path::PathBuf;
use vsdx::model::{Shape, SheetChild};

fn main() -> anyhow::Result<()> {
    let path = PathBuf::from(
        std::env::args_os()
            .nth(1)
            .expect("usage: connec_inventory <stencil.vssx>"),
    );
    let doc = vsdx::parse::parse(&path)?;

    struct M {
        name: String,
        obj_type: Option<f64>,
        glue_type: Option<f64>,
        route_style: Option<f64>,
        con_fixed: Option<f64>,
        dbl_click: Option<String>,
        one_d: bool,
        triggers: String,
        walk_glue: bool,
        guards: Vec<String>,
        frames: Vec<String>,
        rows: String,
        conn_pts: usize,
        subshapes: usize,
    }

    fn walk_sub(shapes: &[Shape], out: &mut usize) {
        for s in shapes {
            *out += s.sub_shapes.len();
            walk_sub(&s.sub_shapes, out);
        }
    }

    let mut masters: Vec<M> = Vec::new();
    for m in &doc.masters {
        let Some(root) = m.shapes.first() else { continue };
        let cell = |n: &str| root.sheet.cell(n).and_then(|c| c.numeric());
        let strcell = |n: &str| {
            root.sheet
                .cell(n)
                .and_then(|c| c.f.clone().or_else(|| c.v.clone()))
        };
        let one_d = root.sheet.cell("BeginX").is_some();
        let triggers = ["BegTrigger", "EndTrigger"]
            .iter()
            .filter(|n| root.sheet.cell(n).is_some())
            .map(|s| s.to_string())
            .collect::<Vec<_>>()
            .join(";");
        let walk_glue = root.sheet.cell("WalkPreference").is_some();
        let mut guards = Vec::new();
        for n in ["Width", "Height", "Angle", "LocPinX", "LocPinY", "PinX", "PinY"] {
            if let Some(c) = root.sheet.cell(n) {
                let f = c.f.as_deref().unwrap_or("");
                let u = f.to_uppercase();
                if u.contains("GUARD") {
                    guards.push(format!("{n}={f}"));
                }
            }
        }
        let mut frames = Vec::new();
        for n in ["Width", "Height", "Angle", "LocPinX", "LocPinY"] {
            if let Some(c) = root.sheet.cell(n) {
                if let Some(f) = c.f.as_deref() {
                    let u = f.to_uppercase();
                    if !u.contains("GUARD")
                        && (u.contains("WIDTH")
                            || u.contains("HEIGHT")
                            || u.contains("BEGIN")
                            || u.contains("END"))
                    {
                        frames.push(format!("{n}={f}"));
                    }
                }
            }
        }
        let mut rows = Vec::new();
        let mut conn_pts = 0;
        for sec in &root.sheet.children {
            if let SheetChild::Section(s) = sec {
                if s.name == "Connection" {
                    conn_pts += s.rows.len();
                    continue;
                }
                if s.name != "Geometry" {
                    continue;
                }
                let mut kinds: Vec<String> = Vec::new();
                for r in &s.rows {
                    let t = r.t.clone().unwrap_or_else(|| "?".into());
                    // Mark formula-driven X cells (frame-derived rows).
                    let x = r.cell("X").and_then(|c| c.f.clone());
                    let tag = match x.as_deref() {
                        Some(f) if f != "Inh" && f != "No Formula" => {
                            format!("{t}(F:{})", short_f(f))
                        }
                        Some("Inh") => format!("{t}(Inh)"),
                        _ => t,
                    };
                    kinds.push(tag);
                }
                rows.push(format!("[{}]", kinds.join(" ")));
            }
        }
        let mut subshapes = 0;
        walk_sub(&m.shapes, &mut subshapes);
        masters.push(M {
            name: m.name_u.clone().unwrap_or_else(|| "?".into()),
            obj_type: cell("ObjType"),
            glue_type: cell("GlueType"),
            route_style: cell("ShapeRouteStyle"),
            con_fixed: cell("ConFixedCode"),
            dbl_click: strcell("EventDblClick"),
            one_d,
            triggers,
            walk_glue,
            guards,
            frames,
            rows: rows.join(" "),
            conn_pts,
            subshapes,
        });
    }

    masters.sort_by(|a, b| a.name.cmp(&b.name));
    println!("name\tObjType\tGlueType\tRouteStyle\tConFixed\t1D\tTriggers\tWalk\tConnPts\tSubs\tGuards\tFrameCells\tGeometry");
    for m in &masters {
        println!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            m.name,
            opt(m.obj_type),
            opt(m.glue_type),
            opt(m.route_style),
            opt(m.con_fixed),
            if m.one_d { "1D" } else { "2D" },
            m.triggers,
            if m.walk_glue { "walk" } else { "·" },
            m.conn_pts,
            m.subshapes,
            m.guards.join(";"),
            m.frames.join(";"),
            m.rows
        );
        if let Some(d) = &m.dbl_click {
            if !d.is_empty() {
                println!("    EventDblClick: {d}");
            }
        }
    }
    println!(
        "\n== {} masters ==",
        masters.len()
    );
    Ok(())
}

fn short_f(f: &str) -> String {
    // Compact the formula for the table: keep the case tokens.
    let mut out = f.replace(char::is_whitespace, "");
    if out.len() > 40 {
        out.truncate(40);
        out.push('…');
    }
    out
}

fn opt(v: Option<f64>) -> String {
    match v {
        Some(v) => format!("{}", v as i64),
        None => "·".into(),
    }
}
