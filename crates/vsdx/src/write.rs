//! Write path: serialize a [`Document`] back to VSDX XML parts and an OPC
//! zip package.
//!
//! Round-trip contract (v0.2):
//! - modeled parts (`document.xml`, pages/masters indexes, page and master
//!   parts) are re-serialized from the model, which preserves element
//!   order, every attribute, and raw markup (`<Text>`, `<Icon>`, unknown
//!   document children);
//! - every other part of the source package is copied byte-for-byte
//!   (content types and relationships included — the part set is
//!   unchanged);
//! - documents built in code (`package: None`) get a complete generated
//!   package: content types, rels, and standard part names.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::io::Write;
use std::path::Path;

use anyhow::{Context, Result};

use crate::model::{
    Attrs, Cell, Document, IndexChild, Master, Page, RawEl, Section, Shape, ShapeFlow, Sheet,
};
use crate::opc::unescape;

const REL_NS: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
const NS_MAIN: &str = "http://schemas.microsoft.com/office/visio/2012/main";
const NS_REL: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const VISIO_REL: &str = "http://schemas.microsoft.com/visio/2010/relationships";

const CT_RELS: &str = "application/vnd.openxmlformats-package.relationships+xml";
const CT_XML: &str = "application/xml";

// ---------------------------------------------------------------------------
// XML primitives
// ---------------------------------------------------------------------------

fn esc_attr(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn esc_text(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn push_open(out: &mut String, name: &str, attrs: &[(String, String)], self_close: bool) {
    out.push('<');
    out.push_str(name);
    for (k, v) in attrs {
        out.push(' ');
        out.push_str(k);
        out.push_str("=\"");
        out.push_str(&esc_attr(v));
        out.push('"');
    }
    out.push_str(if self_close { "/>" } else { ">" });
}

fn push_close(out: &mut String, name: &str) {
    out.push_str("</");
    out.push_str(name);
    out.push('>');
}

fn push_raw_el(out: &mut String, raw: &RawEl) {
    if raw.inner.is_empty() {
        push_open(out, &raw.name, &raw.attrs, true);
    } else {
        push_open(out, &raw.name, &raw.attrs, false);
        out.push_str(&raw.inner);
        push_close(out, &raw.name);
    }
}

fn push_cell(out: &mut String, name: &str, cell: &Cell) {
    let attrs = if cell.attrs.is_empty() {
        let mut a: Attrs = vec![("N".into(), name.to_string())];
        if let Some(v) = &cell.v {
            a.push(("V".into(), v.clone()));
        }
        if let Some(u) = &cell.u {
            a.push(("U".into(), u.clone()));
        }
        if let Some(f) = &cell.f {
            a.push(("F".into(), f.clone()));
        }
        a
    } else {
        cell.attrs.clone()
    };
    match &cell.raw_inner {
        Some(raw) if !raw.is_empty() => {
            push_open(out, "Cell", &attrs, false);
            out.push_str(raw);
            push_close(out, "Cell");
        }
        _ => push_open(out, "Cell", &attrs, true),
    }
}

fn synth_attrs(pairs: &[(&str, Option<String>)]) -> Attrs {
    pairs
        .iter()
        .filter_map(|(k, v)| v.clone().map(|v| (k.to_string(), v)))
        .collect()
}

/// The cells and sections of a sheet body (opening/closing tags belong to
/// the caller — `PageSheet`, `StyleSheet`, …).
fn push_sheet_body(out: &mut String, sheet: &Sheet) {
    for child in &sheet.children {
        match child {
            crate::model::SheetChild::Cell(n, c) => push_cell(out, n, c),
            crate::model::SheetChild::Section(sec) => push_section(out, sec),
            crate::model::SheetChild::Raw(raw) => push_raw_el(out, raw),
        }
    }
}

fn push_section(out: &mut String, sec: &Section) {
    let attrs = if sec.attrs.is_empty() {
        let mut a: Attrs = vec![("N".into(), sec.name.clone())];
        if sec.ix != 0 {
            a.push(("IX".into(), sec.ix.to_string()));
        }
        a
    } else {
        sec.attrs.clone()
    };
    let empty = sec.rows.is_empty() && sec.cells.is_empty();
    push_open(out, "Section", &attrs, empty);
    if empty {
        return;
    }
    for (n, c) in &sec.cells {
        push_cell(out, n, c);
    }
    for row in &sec.rows {
        let rattrs = if row.attrs.is_empty() {
            synth_attrs(&[
                ("T", row.t.clone()),
                (
                    "IX",
                    if row.ix != 0 {
                        Some(row.ix.to_string())
                    } else {
                        None
                    },
                ),
                ("N", row.n.clone()),
            ])
        } else {
            row.attrs.clone()
        };
        if row.cells.is_empty() {
            push_open(out, "Row", &rattrs, true);
        } else {
            push_open(out, "Row", &rattrs, false);
            for (n, c) in &row.cells {
                push_cell(out, n, c);
            }
            push_close(out, "Row");
        }
    }
    push_close(out, "Section");
}

fn push_shape(out: &mut String, s: &Shape) {
    let attrs = if s.attrs.is_empty() {
        synth_attrs(&[
            ("ID", Some(s.id.to_string())),
            ("Type", s.type_.clone()),
            ("Master", s.master.map(|v| v.to_string())),
            ("MasterShape", s.master_shape.map(|v| v.to_string())),
            ("LineStyle", s.line_style.map(|v| v.to_string())),
            ("FillStyle", s.fill_style.map(|v| v.to_string())),
            ("TextStyle", s.text_style.map(|v| v.to_string())),
            ("NameU", s.name_u.clone()),
        ])
    } else {
        s.attrs.clone()
    };
    push_open(out, "Shape", &attrs, false);
    push_sheet_body(out, &s.sheet);
    let push_text = |out: &mut String| {
        if let Some(raw) = &s.sheet.text_raw {
            out.push_str("<Text>");
            out.push_str(raw);
            push_close(out, "Text");
        }
    };
    let push_raws = |out: &mut String| {
        for raw in &s.raw_children {
            push_raw_el(out, raw);
        }
    };
    let push_subs = |out: &mut String| {
        if !s.sub_shapes.is_empty() {
            out.push_str("<Shapes>");
            for sub in &s.sub_shapes {
                push_shape(out, sub);
            }
            push_close(out, "Shapes");
        }
    };
    if s.flow.is_empty() {
        // Constructed shape: schema order Text → raws → sub-shapes.
        push_text(out);
        push_raws(out);
        push_subs(out);
    } else {
        for step in &s.flow {
            match step {
                ShapeFlow::Text => push_text(out),
                ShapeFlow::Raw(i) => {
                    if let Some(raw) = s.raw_children.get(*i) {
                        push_raw_el(out, raw);
                    }
                }
                ShapeFlow::Shapes => push_subs(out),
            }
        }
    }
    push_close(out, "Shape");
}

fn push_shapes(out: &mut String, shapes: &[Shape]) {
    out.push_str("<Shapes>");
    for s in shapes {
        push_shape(out, s);
    }
    push_close(out, "Shapes");
}

fn push_index_children(out: &mut String, children: &[IndexChild]) {
    for c in children {
        match c {
            IndexChild::Sheet { tag, sheet } => {
                push_open(out, tag, &sheet.attrs, false);
                push_sheet_body(out, sheet);
                push_close(out, tag);
            }
            IndexChild::Rel(a) => push_open(out, "Rel", a, true),
            IndexChild::Raw(r) => push_raw_el(out, r),
        }
    }
}

/// The rId an index entry itself declares: its `r:id` attribute or
/// `<Rel r:id>` child — exactly the binding parse uses to find the
/// entry's contents part. `None` when the model entry declares
/// neither (an in-code-built or freshly imported entry).
fn own_rid(attrs: &Attrs, children: &[IndexChild]) -> Option<String> {
    attrs
        .iter()
        .find(|(k, _)| k.len() > 2 && k.ends_with(":id"))
        .map(|(_, v)| v.clone())
        .or_else(|| {
            children.iter().find_map(|c| match c {
                IndexChild::Rel(a) => a
                    .iter()
                    .find(|(k, _)| k.len() > 2 && k.ends_with(":id"))
                    .map(|(_, v)| v.clone()),
                _ => None,
            })
        })
}

/// rIds for `own.len()` index entries: the declared one when present,
/// else a fresh sequential rId past `floor` and past every rId the
/// entries themselves declare, so a synthesized id never collides with
/// an entry that brings its own. Deterministic — every consumer of a
/// document's index rIds (the index serializer, the rels
/// reconciliation, the fresh-path rels generation) derives them from
/// this one function and therefore agrees.
fn index_rids(own: Vec<Option<String>>, floor: u32) -> Vec<String> {
    let max = own
        .iter()
        .flatten()
        .filter_map(|r| r.strip_prefix("rId").and_then(|s| s.parse::<u32>().ok()))
        .chain(std::iter::once(floor))
        .max()
        .unwrap_or(0);
    let mut n = max;
    own.into_iter()
        .map(|o| {
            o.unwrap_or_else(|| {
                n += 1;
                format!("rId{n}")
            })
        })
        .collect()
}

/// The masters index part a save writes: the document's own when
/// parsed, else a synthesized default for a retained package that
/// shipped without one but gained masters by import (#84/CodeRabbit:
/// without this, imported masters silently dropped on save).
pub(crate) fn effective_masters_part(doc: &Document) -> Option<String> {
    if let Some(mp) = &doc.masters_part {
        return Some(mp.clone());
    }
    (!doc.masters.is_empty()).then(|| "visio/masters/masters.xml".to_string())
}

/// Per-master rIds of the masters index (`floor` = highest numeric rId
/// in the source masters rels, so synthesis skips rIds a retained
/// package already binds; 0 for fresh documents).
pub(crate) fn master_index_rids(doc: &Document, floor: u32) -> Vec<String> {
    index_rids(
        doc.masters
            .iter()
            .map(|m| own_rid(&m.attrs, &m.children))
            .collect(),
        floor,
    )
}

/// Per-page rIds of the pages index — the page analog of
/// [`master_index_rids`].
pub(crate) fn page_index_rids(doc: &Document, floor: u32) -> Vec<String> {
    index_rids(
        doc.pages
            .iter()
            .map(|p| own_rid(&p.attrs, &p.children))
            .collect(),
        floor,
    )
}

/// Highest numeric rId among a rels part's entries (0 when none) —
/// the synthesis floor for a retained package.
fn rels_rid_floor<'a>(ids: impl Iterator<Item = &'a str>) -> u32 {
    ids.filter_map(|id| id.strip_prefix("rId").and_then(|n| n.parse::<u32>().ok()))
        .max()
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Part serializers
// ---------------------------------------------------------------------------

fn decl() -> &'static str {
    "<?xml version=\"1.0\" encoding=\"utf-8\" standalone=\"yes\"?>\n"
}

/// Standard VSDX namespaces, injected into roots of constructed documents
/// whose attribute lists predate parsing (parsed roots keep their own).
fn with_xmlns(attrs: &Attrs) -> Cow<'_, Attrs> {
    if attrs.iter().any(|(k, _)| k == "xmlns") {
        return Cow::Borrowed(attrs);
    }
    let mut v = vec![
        ("xmlns".to_string(), NS_MAIN.to_string()),
        ("xmlns:r".to_string(), NS_REL.to_string()),
    ];
    v.extend(attrs.iter().cloned());
    Cow::Owned(v)
}

/// Serialize the main document part (`document.xml`).
pub fn document_xml(doc: &Document) -> String {
    let mut out = String::from(decl());
    push_open(
        &mut out,
        "VisioDocument",
        &with_xmlns(&doc.root_attrs),
        false,
    );
    for item in &doc.items {
        match item {
            crate::model::DocItem::StyleSheets => {
                out.push_str("<StyleSheets>");
                for ss in &doc.stylesheets {
                    push_open(&mut out, "StyleSheet", &ss.attrs, false);
                    push_sheet_body(&mut out, &ss.sheet);
                    push_close(&mut out, "StyleSheet");
                }
                push_close(&mut out, "StyleSheets");
            }
            crate::model::DocItem::Colors => {
                out.push_str("<Colors>");
                for (ix, rgb) in &doc.colors {
                    push_open(
                        &mut out,
                        "ColorEntry",
                        &[("IX".into(), ix.to_string()), ("RGB".into(), rgb.clone())],
                        true,
                    );
                }
                push_close(&mut out, "Colors");
            }
            crate::model::DocItem::DocumentSheet => {
                push_open(&mut out, "DocumentSheet", &doc.sheet.attrs, false);
                push_sheet_body(&mut out, &doc.sheet);
                push_close(&mut out, "DocumentSheet");
            }
            crate::model::DocItem::Raw(r) => push_raw_el(&mut out, r),
        }
    }
    push_close(&mut out, "VisioDocument");
    out
}

/// Serialize the pages index part (`pages.xml`) — fresh-path behavior:
/// every page lacking its own `<Rel>`/r:id gets a synthesized binding
/// (the fresh path assigns all pages parts).
pub fn pages_index_xml(doc: &Document) -> String {
    pages_index_xml_with(doc, &page_index_rids(doc, 0), true)
}

/// [`pages_index_xml`] for a retained package: `floor` comes from the
/// source pages rels, and pages with no part of their own (stencil
/// pages — index-only entries with no contents part) get no synthesized
/// binding, keeping a pure round-trip byte-stable.
pub(crate) fn pages_index_xml_floored(doc: &Document, floor: u32) -> String {
    pages_index_xml_with(doc, &page_index_rids(doc, floor), false)
}

fn pages_index_xml_with(doc: &Document, rids: &[String], fresh: bool) -> String {
    let mut out = String::from(decl());
    push_open(&mut out, "Pages", &with_xmlns(&doc.pages_attrs), false);
    for (i, page) in doc.pages.iter().enumerate() {
        let attrs = if page.attrs.is_empty() {
            synth_attrs(&[
                ("ID", Some(page.id.to_string())),
                ("NameU", Some(page.name.clone())),
            ])
        } else {
            page.attrs.clone()
        };
        push_open(&mut out, "Page", &attrs, false);
        push_index_children(&mut out, &page.children);
        // A page the model built without a `<Rel>` child still needs a
        // binding to its contents part — parse resolves contents
        // strictly through this rId and would otherwise load the page
        // empty. Synthesized ids match the rels part because both
        // derive from [`page_index_rids`]. A retained package keeps
        // part-less stencil pages exactly as parsed.
        if own_rid(&page.attrs, &page.children).is_none() && (fresh || !page.part.is_empty()) {
            push_open(
                &mut out,
                "Rel",
                &[("r:id".to_string(), rids[i].clone())],
                true,
            );
        }
        push_close(&mut out, "Page");
    }
    push_close(&mut out, "Pages");
    out
}

/// Serialize one page's contents part.
pub fn page_xml(page: &Page) -> String {
    let mut out = String::from(decl());
    push_open(
        &mut out,
        "PageContents",
        &with_xmlns(&page.contents_attrs),
        false,
    );
    if !page.shapes.is_empty() {
        push_shapes(&mut out, &page.shapes);
    }
    for raw in &page.contents_raw {
        push_raw_el(&mut out, raw);
    }
    push_close(&mut out, "PageContents");
    out
}

/// Serialize the masters index part (`masters.xml`) — fresh-path
/// behavior: every master lacking its own `<Rel>`/r:id gets a
/// synthesized binding.
pub fn masters_index_xml(doc: &Document) -> String {
    masters_index_xml_with(doc, &master_index_rids(doc, 0), true)
}

/// [`masters_index_xml`] for a retained package (`floor` from the
/// source masters rels; part-less masters get no synthesized binding,
/// keeping a pure round-trip byte-stable).
pub(crate) fn masters_index_xml_floored(doc: &Document, floor: u32) -> String {
    masters_index_xml_with(doc, &master_index_rids(doc, floor), false)
}

fn masters_index_xml_with(doc: &Document, rids: &[String], fresh: bool) -> String {
    let mut out = String::from(decl());
    push_open(&mut out, "Masters", &with_xmlns(&doc.masters_attrs), false);
    for (i, m) in doc.masters.iter().enumerate() {
        let attrs = if m.attrs.is_empty() {
            synth_attrs(&[("ID", Some(m.id.to_string())), ("NameU", m.name_u.clone())])
        } else {
            m.attrs.clone()
        };
        push_open(&mut out, "Master", &attrs, false);
        push_index_children(&mut out, &m.children);
        // Same binding requirement as pages: a master without its own
        // `<Rel>` child (in-code built, or imported from a stencil —
        // ImportMaster drops the borrowed stencil rel) gets a
        // synthesized one so its contents part survives save→parse.
        if own_rid(&m.attrs, &m.children).is_none() && (fresh || !m.part.is_empty()) {
            push_open(
                &mut out,
                "Rel",
                &[("r:id".to_string(), rids[i].clone())],
                true,
            );
        }
        push_close(&mut out, "Master");
    }
    push_close(&mut out, "Masters");
    out
}

/// Serialize one master's contents part.
pub fn master_xml(m: &Master) -> String {
    let mut out = String::from(decl());
    push_open(
        &mut out,
        "MasterContents",
        &with_xmlns(&m.contents_attrs),
        false,
    );
    if !m.shapes.is_empty() {
        push_shapes(&mut out, &m.shapes);
    }
    for raw in &m.contents_raw {
        push_raw_el(&mut out, raw);
    }
    let sheet = &m.contents_sheet;
    if !sheet.attrs.is_empty() || !sheet.children.is_empty() {
        push_open(&mut out, "MasterSheet", &sheet.attrs, false);
        push_sheet_body(&mut out, sheet);
        push_close(&mut out, "MasterSheet");
    }
    push_close(&mut out, "MasterContents");
    out
}

// ---------------------------------------------------------------------------
// Package assembly
// ---------------------------------------------------------------------------

/// The part names this document models (and therefore re-serializes).
/// Pages/masters without a part (stencil pages) are absent.
pub fn modeled_parts(doc: &Document) -> Vec<String> {
    let mut v = vec![doc.doc_part.clone(), doc.pages_part.clone()];
    for p in &doc.pages {
        if !p.part.is_empty() {
            v.push(p.part.clone());
        }
    }
    if let Some(mp) = effective_masters_part(doc) {
        v.push(mp);
    }
    for m in &doc.masters {
        if !m.part.is_empty() {
            v.push(m.part.clone());
        }
    }
    v
}

/// Build the full part map for writing.
///
/// With a retained source package, unmodeled parts are copied verbatim and
/// modeled parts re-serialized. Without one, a complete package (content
/// types + rels + parts) is generated from the model.
pub fn build_parts(doc: &Document) -> BTreeMap<String, Vec<u8>> {
    let mut parts: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    if let Some(pkg) = &doc.package {
        for name in pkg.part_names() {
            let bytes = pkg.part(name).map(<[u8]>::to_vec).unwrap_or_default();
            parts.insert(name.to_string(), bytes);
        }
        parts.insert(doc.doc_part.clone(), document_xml(doc).into_bytes());
        parts.insert(
            doc.pages_part.clone(),
            pages_index_xml_floored(
                doc,
                rels_rid_floor(pkg.relationships(&doc.pages_part).iter().map(|(id, _, _)| id.as_str())),
            )
            .into_bytes(),
        );
        for p in &doc.pages {
            if !p.part.is_empty() && pkg.part(&p.part).is_some() {
                parts.insert(p.part.clone(), page_xml(p).into_bytes());
            }
        }
        if let Some(mp) = effective_masters_part(doc) {
            parts.insert(
                mp.clone(),
                masters_index_xml_floored(
                    doc,
                    rels_rid_floor(pkg.relationships(&mp).iter().map(|(id, _, _)| id.as_str())),
                )
                .into_bytes(),
            );
            // A synthesized index (the package shipped without a masters
            // part) also needs its document relationship — the fresh
            // path's generate_rels writes one; the retained path copies
            // the source rels verbatim, so append it here.
            if doc.masters_part.is_none() {
                let doc_rels = rels_part_of(&doc.doc_part);
                let mut entries = match parts.get(&doc_rels) {
                    Some(bytes) => parse_rels(bytes),
                    None => Vec::new(),
                };
                let have_masters = entries
                    .iter()
                    .any(|e| e.type_ == rel_type("masters"));
                if !have_masters {
                    let next = 1 + entries
                        .iter()
                        .filter_map(|e| {
                            e.id.strip_prefix("rId")
                                .and_then(|n| n.parse::<u32>().ok())
                        })
                        .max()
                        .unwrap_or(0);
                    entries.push(RelEntry {
                        id: format!("rId{next}"),
                        type_: rel_type("masters"),
                        target: relative_target(&dir_of(&doc.doc_part), &mp),
                    });
                    parts.insert(doc_rels, build_rels(&entries));
                }
            }
        }
        for m in &doc.masters {
            if !m.part.is_empty() && pkg.part(&m.part).is_some() {
                parts.insert(m.part.clone(), master_xml(m).into_bytes());
            }
        }
        // Editing (M4) may have added or removed pages relative to the
        // source package: materialize new page parts, drop removed ones,
        // and keep the pages-index rels and [Content_Types].xml in sync.
        // Pure round-trips (no page-set change) leave all three untouched.
        reconcile_page_parts(&mut parts, doc, pkg);
        reconcile_master_parts(&mut parts, doc, pkg);
        return parts;
    }

    // Fresh document: standard part layout.
    let doc_part = if doc.doc_part.is_empty() {
        "visio/document.xml".to_string()
    } else {
        doc.doc_part.clone()
    };
    let pages_part = if doc.pages_part.is_empty() {
        "visio/pages/pages.xml".to_string()
    } else {
        doc.pages_part.clone()
    };
    parts.insert(doc_part.clone(), document_xml(doc).into_bytes());
    parts.insert(pages_part.clone(), pages_index_xml(doc).into_bytes());
    // The rIds the pages index was just serialized with — the rels part
    // must record exactly these (both derive from [`page_index_rids`]).
    let page_rids = page_index_rids(doc, 0);

    // Assign part names to pages/masters that lack one (1-based, in order).
    let mut page_parts: Vec<(String, String)> = Vec::new(); // (part, rId)
    for (i, p) in doc.pages.iter().enumerate() {
        let part = if p.part.is_empty() {
            format!("visio/pages/page{}.xml", i + 1)
        } else {
            p.part.clone()
        };
        let rid = own_rid(&p.attrs, &p.children).unwrap_or_else(|| page_rids[i].clone());
        parts.insert(part.clone(), page_xml(p).into_bytes());
        page_parts.push((part, rid));
    }

    let mut master_parts: Vec<(String, String)> = Vec::new();
    let masters_part_default = "visio/masters/masters.xml".to_string();
    if !doc.masters.is_empty() {
        let masters_part = doc
            .masters_part
            .clone()
            .unwrap_or_else(|| masters_part_default.clone());
        parts.insert(masters_part.clone(), masters_index_xml(doc).into_bytes());
        // Same agreement requirement as the pages rels above.
        let master_rids = master_index_rids(doc, 0);
        for (i, m) in doc.masters.iter().enumerate() {
            let part = if m.part.is_empty() {
                format!("visio/masters/master{}.xml", m.id)
            } else {
                m.part.clone()
            };
            let rid = own_rid(&m.attrs, &m.children).unwrap_or_else(|| master_rids[i].clone());
            parts.insert(part.clone(), master_xml(m).into_bytes());
            master_parts.push((part, rid));
        }
        generate_rels(
            &mut parts,
            &doc_part,
            vec![
                ("pages", pages_part.clone()),
                ("masters", masters_part.clone()),
            ],
        );
        generate_index_rels(&mut parts, &masters_part, &master_parts, "master");
    } else {
        generate_rels(&mut parts, &doc_part, vec![("pages", pages_part.clone())]);
    }

    generate_index_rels(&mut parts, &pages_part, &page_parts, "page");

    parts.insert(
        "[Content_Types].xml".to_string(),
        content_types_xml(&parts, &doc_part, &pages_part, doc).into_bytes(),
    );
    parts
}

fn rel_type(kind: &str) -> String {
    format!("{VISIO_REL}/{kind}")
}

/// `_rels/.rels` plus the document part's rels.
fn generate_rels(
    parts: &mut BTreeMap<String, Vec<u8>>,
    doc_part: &str,
    targets: Vec<(&str, String)>,
) {
    let mut root_rels = String::from(decl());
    root_rels.push_str(&format!("<Relationships xmlns=\"{REL_NS}\">"));
    root_rels.push_str(&format!(
        "<Relationship Id=\"rId1\" Type=\"{}\" Target=\"{}\"/>",
        rel_type("document"),
        relative_target("", doc_part)
    ));
    root_rels.push_str("</Relationships>");
    parts.insert("_rels/.rels".to_string(), root_rels.into_bytes());

    let mut doc_rels = String::from(decl());
    doc_rels.push_str(&format!("<Relationships xmlns=\"{REL_NS}\">"));
    for (i, (kind, target)) in targets.iter().enumerate() {
        doc_rels.push_str(&format!(
            "<Relationship Id=\"rId{}\" Type=\"{}\" Target=\"{}\"/>",
            i + 1,
            rel_type(kind),
            relative_target(&dir_of(doc_part), target)
        ));
    }
    doc_rels.push_str("</Relationships>");
    parts.insert(rels_part_of(doc_part), doc_rels.into_bytes());
}

/// Rels of a pages/masters index part: one `<Relationship>` per (part, rId)
/// declared by the index entries' `<Rel>` children (falling back to
/// sequential rIds).
fn generate_index_rels(
    parts: &mut BTreeMap<String, Vec<u8>>,
    index_part: &str,
    entries: &[(String, String)],
    kind: &str,
) {
    let mut rels = String::from(decl());
    rels.push_str(&format!("<Relationships xmlns=\"{REL_NS}\">"));
    for (target, rid) in entries {
        rels.push_str(&format!(
            "<Relationship Id=\"{rid}\" Type=\"{}\" Target=\"{}\"/>",
            rel_type(kind),
            relative_target(&dir_of(index_part), target)
        ));
    }
    rels.push_str("</Relationships>");
    parts.insert(rels_part_of(index_part), rels.into_bytes());
}

fn content_types_xml(
    parts: &BTreeMap<String, Vec<u8>>,
    doc_part: &str,
    pages_part: &str,
    doc: &Document,
) -> String {
    let masters_part = doc.masters_part.clone();
    let mut page_set = std::collections::HashSet::new();
    page_set.insert(pages_part.to_string());
    for p in &doc.pages {
        if !p.part.is_empty() {
            page_set.insert(p.part.clone());
        }
    }
    let mut out = String::from(decl());
    out.push_str("<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\">");
    if parts.keys().any(|n| n.ends_with(".rels")) {
        out.push_str(&format!(
            "<Default Extension=\"rels\" ContentType=\"{CT_RELS}\"/>"
        ));
    }
    out.push_str(&format!(
        "<Default Extension=\"xml\" ContentType=\"{CT_XML}\"/>"
    ));
    fn override_part(part: &str, ct: &str, out: &mut String) {
        out.push_str(&format!(
            "<Override PartName=\"/{}\" ContentType=\"{}\"/>",
            part, ct
        ));
    }
    // A document with masters but no pages is a stencil (the
    // synthetic built-in catalog, #87): its main part carries the
    // stencil content type, like the shipped `Visio Content` files.
    let doc_ct = if !doc.masters.is_empty() && doc.pages.is_empty() {
        "application/vnd.ms-visio.stencil.main+xml"
    } else {
        "application/vnd.ms-visio.drawing.main+xml"
    };
    override_part(doc_part, doc_ct, &mut out);
    for part in &page_set {
        override_part(part, "application/vnd.ms-visio.pages+xml", &mut out);
    }
    if let Some(mp) = &masters_part {
        override_part(mp, "application/vnd.ms-visio.masters+xml", &mut out);
    }
    for m in &doc.masters {
        if !m.part.is_empty() {
            override_part(&m.part, "application/vnd.ms-visio.master+xml", &mut out);
        }
    }
    out.push_str("</Types>");
    out
}

fn dir_of(part: &str) -> String {
    match part.rfind('/') {
        Some(i) => part[..i].to_string(),
        None => String::new(),
    }
}

fn rels_part_of(part: &str) -> String {
    let dir = dir_of(part);
    let file = &part[dir.len() + if dir.is_empty() { 0 } else { 1 }..];
    if dir.is_empty() {
        format!("_rels/{file}.rels")
    } else {
        format!("{dir}/_rels/{file}.rels")
    }
}

/// Relative path from a part's directory to another part (rels Target form).
fn relative_target(base_dir: &str, target: &str) -> String {
    let base: Vec<&str> = if base_dir.is_empty() {
        Vec::new()
    } else {
        base_dir.split('/').collect()
    };
    let tgt: Vec<&str> = target.split('/').collect();
    let mut i = 0usize;
    while i < base.len() && i + 1 < tgt.len() && base[i] == tgt[i] {
        i += 1;
    }
    let ups = base.len() - i;
    format!("{}{}", "../".repeat(ups), tgt[i..].join("/"))
}

/// Write the document to `path` as an OPC zip package.
pub fn save(doc: &Document, path: &Path) -> Result<()> {
    let parts = build_parts(doc);
    let file =
        std::fs::File::create(path).with_context(|| format!("creating {}", path.display()))?;
    write_parts(&parts, file)
}

/// Serialize into any seekable writer (used by the atomic save path:
/// the writer is craft-atomic's staged file).
pub fn save_with<W: std::io::Write + std::io::Seek>(doc: &Document, w: W) -> Result<()> {
    let parts = build_parts(doc);
    write_parts(&parts, w)
}

fn write_parts<W: std::io::Write + std::io::Seek>(
    parts: &std::collections::BTreeMap<String, Vec<u8>>,
    w: W,
) -> Result<()> {
    let mut w = zip::ZipWriter::new(w);
    let opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    for (name, data) in parts {
        w.start_file(name.as_str(), opts)
            .with_context(|| format!("starting zip entry {name}"))?;
        w.write_all(data)
            .with_context(|| format!("writing {name}"))?;
    }
    w.finish().context("finishing zip")?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Page-set reconciliation (editing support, M4)
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct RelEntry {
    id: String,
    type_: String,
    target: String,
}

/// Parse a `.rels` part into its `<Relationship>` entries.
fn parse_rels(bytes: &[u8]) -> Vec<RelEntry> {
    use quick_xml::events::Event;
    let text = String::from_utf8_lossy(bytes);
    let mut r = quick_xml::Reader::from_str(&text);
    let mut out = Vec::new();
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match r.read_event_into(&mut buf) {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) if e.name().as_ref() == "Relationship" => {
                let mut id = String::new();
                let mut type_ = String::new();
                let mut target = String::new();
                for a in e.attributes().flatten() {
                    match a.key.as_ref() {
                        "Id" => {
                            id = a
                                .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                                .unwrap_or_default()
                                .into_owned()
                        }
                        "Type" => {
                            type_ = a
                                .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                                .unwrap_or_default()
                                .into_owned()
                        }
                        "Target" => {
                            target = a
                                .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                                .unwrap_or_default()
                                .into_owned()
                        }
                        _ => {}
                    }
                }
                out.push(RelEntry { id, type_, target });
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    out
}

/// Resolve a rels `Target` (relative to the part owning the rels) to a
/// package part name.
fn resolve_target(index_dir: &str, target: &str) -> String {
    let mut stack: Vec<&str> = if index_dir.is_empty() {
        Vec::new()
    } else {
        index_dir.split('/').collect()
    };
    for seg in target.split('/') {
        match seg {
            "." | "" => {}
            ".." => {
                stack.pop();
            }
            s => stack.push(s),
        }
    }
    stack.join("/")
}

fn build_rels(entries: &[RelEntry]) -> Vec<u8> {
    let mut out = String::from(decl());
    out.push_str(&format!("<Relationships xmlns=\"{REL_NS}\">"));
    for e in entries {
        out.push_str(&format!(
            "<Relationship Id=\"{}\" Type=\"{}\" Target=\"{}\"/>",
            esc_attr(&e.id),
            esc_attr(&e.type_),
            esc_attr(&e.target)
        ));
    }
    out.push_str("</Relationships>");
    out.into_bytes()
}

/// One (extension-or-part, content type) pair from
/// `[Content_Types].xml`.
type ContentTypePair = (String, String);

/// Parse `[Content_Types].xml` into (defaults, overrides).
fn parse_content_types(bytes: &[u8]) -> (Vec<ContentTypePair>, Vec<ContentTypePair>) {
    use quick_xml::events::Event;
    let text = String::from_utf8_lossy(bytes);
    let mut r = quick_xml::Reader::from_str(&text);
    let mut defaults = Vec::new();
    let mut overrides = Vec::new();
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match r.read_event_into(&mut buf) {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => match e.name().as_ref() {
                "Default" => {
                    let mut ext = String::new();
                    let mut ct = String::new();
                    for a in e.attributes().flatten() {
                        match a.key.as_ref() {
                            "Extension" => ext = a.value.to_string(),
                            "ContentType" => ct = a.value.to_string(),
                            _ => {}
                        }
                    }
                    defaults.push((ext, ct));
                }
                "Override" => {
                    let mut part = String::new();
                    let mut ct = String::new();
                    for a in e.attributes().flatten() {
                        match a.key.as_ref() {
                            "PartName" => part = a.value.to_string(),
                            "ContentType" => ct = a.value.to_string(),
                            _ => {}
                        }
                    }
                    overrides.push((part, ct));
                }
                _ => {}
            },
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    (defaults, overrides)
}

fn build_content_types(defaults: &[(String, String)], overrides: &[(String, String)]) -> Vec<u8> {
    let mut out = String::from(decl());
    out.push_str("<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\">");
    for (ext, ct) in defaults {
        out.push_str(&format!(
            "<Default Extension=\"{}\" ContentType=\"{}\"/>",
            esc_attr(ext),
            esc_attr(ct)
        ));
    }
    for (part, ct) in overrides {
        out.push_str(&format!(
            "<Override PartName=\"{}\" ContentType=\"{}\"/>",
            esc_attr(part),
            esc_attr(ct)
        ));
    }
    out.push_str("</Types>");
    out.into_bytes()
}

const CT_PAGE: &str = "application/vnd.ms-visio.pages+xml";

/// Synchronize `parts` with the model's page set: new pages get their
/// part, a pages-index relationship, and a content-types override;
/// removed pages lose all three. No page-set change → nothing rewritten.
fn reconcile_page_parts(
    parts: &mut BTreeMap<String, Vec<u8>>,
    doc: &Document,
    pkg: &crate::opc::Package,
) {
    let index_dir = dir_of(&doc.pages_part);
    let rels_part = rels_part_of(&doc.pages_part);

    let src_entries = match pkg.part(&rels_part) {
        Some(bytes) => parse_rels(bytes),
        None => Vec::new(),
    };
    let src_targets: Vec<String> = src_entries
        .iter()
        .map(|e| resolve_target(&index_dir, &e.target))
        .collect();

    let model_pages: Vec<(usize, &Page)> = doc
        .pages
        .iter()
        .enumerate()
        .filter(|(_, p)| !p.part.is_empty())
        .collect();
    let model_parts: Vec<String> = model_pages.iter().map(|(_, p)| p.part.clone()).collect();

    let added: Vec<(usize, &Page)> = model_pages
        .iter()
        .copied()
        .filter(|(_, p)| !src_targets.contains(&p.part))
        .collect();
    let removed: Vec<String> = src_targets
        .iter()
        .filter(|t| !model_parts.contains(*t))
        .cloned()
        .collect();

    if added.is_empty() && removed.is_empty() {
        return;
    }

    // Parts: add new, drop removed.
    for (_, page) in &added {
        parts.insert(page.part.clone(), page_xml(page).into_bytes());
    }
    for part in &removed {
        parts.remove(part.as_str());
    }

    // Pages-index rels: drop removed targets, append new pages' rIds —
    // each page's own `<Rel r:id>` child when it has one, else the same
    // synthesized rId the index was serialized with (both derive from
    // [`page_index_rids`], so the index and rels always agree).
    let rids = page_index_rids(doc, rels_rid_floor(src_entries.iter().map(|e| e.id.as_str())));
    let mut entries: Vec<RelEntry> = src_entries
        .iter()
        .filter(|e| !removed.contains(&resolve_target(&index_dir, &e.target)))
        .cloned()
        .collect();
    for (i, page) in &added {
        let rid = own_rid(&page.attrs, &page.children).unwrap_or_else(|| rids[*i].clone());
        let target = relative_target(&index_dir, &page.part);
        entries.push(RelEntry {
            id: rid,
            type_: rel_type("page"),
            target,
        });
    }
    parts.insert(rels_part, build_rels(&entries));

    // Content types: drop overrides of removed parts, add for new pages.
    if let Some(ct_bytes) = parts.get("[Content_Types].xml").cloned() {
        let (mut defaults, mut overrides) = parse_content_types(&ct_bytes);
        overrides.retain(|(part, _)| {
            let name = part.trim_start_matches('/');
            !removed.iter().any(|r| r == name)
        });
        for (_, page) in &added {
            let name = format!("/{}", page.part);
            if !overrides.iter().any(|(p, _)| *p == name) {
                overrides.push((name, CT_PAGE.to_string()));
            }
        }
        let built = build_content_types(&defaults, &overrides);
        defaults.clear();
        parts.insert("[Content_Types].xml".to_string(), built);
    }
}

const CT_MASTER: &str = "application/vnd.ms-visio.master+xml";
const CT_MASTERS: &str = "application/vnd.ms-visio.masters+xml";

/// Materialize/drop master parts for masters added or removed
/// relative to the source package (ImportMaster, #84) — the
/// masters-index analog of `reconcile_page_parts`. An imported master
/// arrives with a document-unique part name and no borrowed `<Rel>`
/// child; its rId is assigned here. Master *removal* (a future command
/// would need it) works the same way as page removal.
fn reconcile_master_parts(
    parts: &mut BTreeMap<String, Vec<u8>>,
    doc: &Document,
    pkg: &crate::opc::Package,
) {
    let Some(mp) = effective_masters_part(doc) else {
        return;
    };
    let mp = &mp;
    let index_dir = dir_of(mp);
    let rels_part = rels_part_of(mp);

    let src_entries = match pkg.part(&rels_part) {
        Some(bytes) => parse_rels(bytes),
        None => Vec::new(),
    };
    let src_targets: Vec<String> = src_entries
        .iter()
        .map(|e| resolve_target(&index_dir, &e.target))
        .collect();

    let model_masters: Vec<(usize, &Master)> = doc
        .masters
        .iter()
        .enumerate()
        .filter(|(_, m)| !m.part.is_empty())
        .collect();
    let model_parts: Vec<String> = model_masters.iter().map(|(_, m)| m.part.clone()).collect();

    let added: Vec<(usize, &Master)> = model_masters
        .iter()
        .copied()
        .filter(|(_, m)| !src_targets.contains(&m.part))
        .collect();
    let removed: Vec<String> = src_targets
        .iter()
        .filter(|t| !model_parts.contains(*t))
        .cloned()
        .collect();

    if added.is_empty() && removed.is_empty() {
        return;
    }

    for (_, m) in &added {
        parts.insert(m.part.clone(), master_xml(m).into_bytes());
    }
    for part in &removed {
        parts.remove(part.as_str());
    }

    // Same agreement requirement as the pages rels: rIds without their
    // own `<Rel>` child are the ones the index was serialized with.
    let rids = master_index_rids(doc, rels_rid_floor(src_entries.iter().map(|e| e.id.as_str())));
    let mut entries: Vec<RelEntry> = src_entries
        .iter()
        .filter(|e| !removed.contains(&resolve_target(&index_dir, &e.target)))
        .cloned()
        .collect();
    for (i, m) in &added {
        let rid = own_rid(&m.attrs, &m.children).unwrap_or_else(|| rids[*i].clone());
        entries.push(RelEntry {
            id: rid,
            type_: rel_type("master"),
            target: relative_target(&index_dir, &m.part),
        });
    }
    parts.insert(rels_part, build_rels(&entries));

    if let Some(ct_bytes) = parts.get("[Content_Types].xml").cloned() {
        let (mut defaults, mut overrides) = parse_content_types(&ct_bytes);
        overrides.retain(|(part, _)| {
            let name = part.trim_start_matches('/');
            !removed.iter().any(|r| r == name)
        });
        for (_, m) in &added {
            let name = format!("/{}", m.part);
            if !overrides.iter().any(|(p, _)| *p == name) {
                overrides.push((name, CT_MASTER.to_string()));
            }
        }
        // The index part's own override: the fresh path's
        // content_types_xml writes it; a synthesized index (the
        // package shipped without one) reaches the retained path only
        // through here.
        let idx_name = format!("/{}", mp);
        if !overrides.iter().any(|(p, _)| *p == idx_name) {
            overrides.push((idx_name, CT_MASTERS.to_string()));
        }
        let built = build_content_types(&defaults, &overrides);
        defaults.clear();
        parts.insert("[Content_Types].xml".to_string(), built);
    }
}

// ---------------------------------------------------------------------------
// Canonical form (round-trip verification)
// ---------------------------------------------------------------------------

/// Deterministic canonical form of an XML part for equality checks:
/// - XML declaration, comments, PIs, whitespace-only text dropped;
/// - attributes sorted by name, values unescaped then uniformly re-escaped;
/// - empty elements normalized to the expanded `<n></n>` form;
/// - CDATA and escaped text normalized to the same escaped form.
pub fn canonical_form(bytes: &[u8]) -> String {
    use quick_xml::events::Event;

    let owned = String::from_utf8_lossy(bytes).into_owned();
    let s = owned.trim_start_matches('\u{feff}');
    let mut r = quick_xml::Reader::from_str(s);
    let mut out = String::new();
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match r.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => canon_tag(&mut out, &e, false),
            Ok(Event::Empty(e)) => canon_tag(&mut out, &e, true),
            Ok(Event::End(e)) => {
                out.push_str("</");
                out.push_str(e.name().as_ref());
                out.push('>');
            }
            Ok(Event::Text(t)) => {
                let txt = t.as_ref().to_string();
                if !txt.trim().is_empty() {
                    out.push_str(&esc_text(&txt));
                }
            }
            Ok(Event::GeneralRef(r)) => {
                let mut ent = String::from("&");
                ent.push_str(r.as_ref());
                ent.push(';');
                let txt = unescape(ent.as_bytes());
                if !txt.trim().is_empty() {
                    out.push_str(&esc_text(&txt));
                }
            }
            Ok(Event::CData(t)) => {
                let txt = t.as_ref().to_string();
                if !txt.trim().is_empty() {
                    out.push_str(&esc_text(&txt));
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    out
}

fn canon_tag(out: &mut String, e: &quick_xml::events::BytesStart<'_>, empty: bool) {
    out.push('<');
    out.push_str(e.name().as_ref());
    let mut attrs: Vec<(String, String)> = e
        .attributes()
        .flatten()
        .map(|a| {
            (
                a.key.as_ref().to_string(),
                a.normalized_value(quick_xml::XmlVersion::Implicit1_0)
                    .unwrap_or_default()
                    .into_owned(),
            )
        })
        .collect();
    attrs.sort();
    for (k, v) in attrs {
        out.push(' ');
        out.push_str(&k);
        out.push_str("=\"");
        out.push_str(&esc_attr(&v));
        out.push('"');
    }
    out.push('>');
    if empty {
        out.push_str("</");
        out.push_str(e.name().as_ref());
        out.push('>');
    }
}
