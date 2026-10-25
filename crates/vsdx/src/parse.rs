//! XML part parsing into the model.
//!
//! Uses an owned-event dispatcher: each `read_event_into` result is converted
//! to owned data immediately so sub-parsers can share the `Reader` without
//! fighting the event's buffer borrow.
//!
//! v0.2: parsing is lossless by construction — element order, every
//! attribute list, raw `<Text>`/`<Icon>` markup, and unknown document-level
//! elements are retained so the document can be written back unchanged.

use std::path::Path;

use anyhow::{bail, Context, Result};

use crate::model::{
    Attrs, Cell, DocItem, Document, IndexChild, Master, Page, RawEl, Row, Section, Shape,
    ShapeFlow, Sheet, SheetChild, StyleSheet,
};
use crate::opc::{unescape, Package};

/// Parse a VSDX-family package into a [`Document`].
///
/// The source [`Package`] is retained on the document so the write path can
/// re-emit unmodeled parts byte-for-byte.
pub fn parse(path: &Path) -> Result<Document> {
    let pkg = Package::open(path).context("opening OPC package")?;
    parse_package(pkg)
}

/// Parse a package held in memory (embedded templates).
pub fn parse_bytes(bytes: &[u8]) -> Result<Document> {
    let pkg = Package::from_reader(std::io::Cursor::new(bytes)).context("opening OPC package")?;
    parse_package(pkg)
}

fn parse_package(pkg: Package) -> Result<Document> {
    let mut doc = Document::default();

    let doc_part = find_main_part(&pkg).context("locating main document part")?;
    let doc_xml = pkg
        .part(&doc_part)
        .with_context(|| format!("reading {doc_part}"))?
        .to_vec();
    parse_document_xml(&String::from_utf8_lossy(&doc_xml), &mut doc)?;
    doc.doc_part = doc_part.clone();

    // Masters index → master parts (optional; a drawing may have none).
    if let Some(mp) = pkg.rel_target(&doc_part, "masters") {
        if let Some(raw) = pkg.part(&mp) {
            let xml = String::from_utf8_lossy(raw).into_owned();
            let rels = pkg.relationships(&mp);
            let (root_attrs, entries) = parse_index_entries(&xml, "Master")?;
            doc.masters_attrs = root_attrs;
            doc.masters_part = Some(mp.clone());
            for entry in entries {
                let part_name = index_rid(&entry.attrs, &entry.children)
                    .and_then(|rid| {
                        rels.iter()
                            .find(|(r, _, _)| *r == rid)
                            .map(|(_, _, t)| t.clone())
                    })
                    .unwrap_or_default();
                let (contents_attrs, contents_sheet, shapes, contents_raw) =
                    match pkg.part(&part_name) {
                        Some(raw) => {
                            let mxml = String::from_utf8_lossy(raw).into_owned();
                            parse_contents(&mxml)?
                        }
                        None => Default::default(),
                    };
                doc.masters.push(Master {
                    id: attr_i64(&entry.attrs, "ID").unwrap_or(0),
                    name_u: attr(&entry.attrs, "NameU").map(str::to_owned),
                    attrs: entry.attrs,
                    children: entry.children,
                    shapes,
                    contents_sheet,
                    contents_attrs,
                    contents_raw,
                    part: part_name,
                });
            }
        }
    }

    // Pages index → page parts. The page sheet lives inline in the index;
    // the page part carries the shape tree.
    let pages_part = pkg
        .rel_target(&doc_part, "pages")
        .context("package has no pages relationship")?;
    let xml = pkg
        .part(&pages_part)
        .context("reading pages index")?
        .to_vec();
    let xml = String::from_utf8_lossy(&xml).into_owned();
    let rels = pkg.relationships(&pages_part);
    let (root_attrs, entries) = parse_index_entries(&xml, "Page")?;
    doc.pages_attrs = root_attrs;
    doc.pages_part = pages_part.clone();
    for entry in entries {
        // Stencil pages have no page part at all (empty `<Page>` with an
        // inline sheet); keep them with an empty shape tree.
        let part_name = index_rid(&entry.attrs, &entry.children)
            .and_then(|rid| {
                rels.iter()
                    .find(|(r, _, _)| *r == rid)
                    .map(|(_, _, t)| t.clone())
            })
            .unwrap_or_default();
        let (contents_attrs, _part_sheet, shapes, contents_raw) = match pkg.part(&part_name) {
            Some(raw) => {
                let pxml = String::from_utf8_lossy(raw).into_owned();
                parse_contents(&pxml)?
            }
            None => Default::default(),
        };
        doc.pages.push(Page {
            id: attr_i64(&entry.attrs, "ID").unwrap_or(0),
            name: attr(&entry.attrs, "NameU")
                .or_else(|| attr(&entry.attrs, "Name"))
                .unwrap_or("Page")
                .to_owned(),
            attrs: entry.attrs,
            children: entry.children,
            shapes,
            contents_attrs,
            contents_raw,
            part: part_name,
        });
    }

    doc.package = Some(pkg);
    Ok(doc)
}

fn find_main_part(pkg: &Package) -> Option<String> {
    for (_, ty, target) in pkg.relationships("") {
        if ty.ends_with("/document") || ty.ends_with("/officeDocument") {
            return Some(target);
        }
    }
    if pkg.part("visio/document.xml").is_some() {
        return Some("visio/document.xml".into());
    }
    for name in pkg.part_names() {
        if let Some(ct) = pkg.content_type_of(name) {
            if ct.starts_with("application/vnd.ms-visio.") && ct.ends_with(".main+xml") {
                return Some(name.to_string());
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Owned-event machinery
// ---------------------------------------------------------------------------

enum Ev {
    /// Start tag with name + attributes.
    Start(String, Attrs),
    /// Self-closing tag with name + attributes.
    Empty(String, Attrs),
    /// Character data (ignored by the structural parsers; raw text is
    /// captured by `read_raw_inner` instead).
    Text,
    /// End tag.
    End(String),
    Eof,
}

fn next(r: &mut quick_xml::Reader<&[u8]>) -> Result<Ev> {
    loop {
        let mut buf = Vec::new();
        match r.read_event_into(&mut buf) {
            Ok(quick_xml::events::Event::Start(e)) => {
                return Ok(Ev::Start(e.name().as_ref().to_string(), attrs_of(&e)))
            }
            Ok(quick_xml::events::Event::Empty(e)) => {
                return Ok(Ev::Empty(e.name().as_ref().to_string(), attrs_of(&e)))
            }
            Ok(quick_xml::events::Event::Text(_))
            | Ok(quick_xml::events::Event::CData(_))
            | Ok(quick_xml::events::Event::GeneralRef(_)) => return Ok(Ev::Text),
            Ok(quick_xml::events::Event::End(e)) => {
                return Ok(Ev::End(e.name().as_ref().to_string()))
            }
            Ok(quick_xml::events::Event::Eof) => return Ok(Ev::Eof),
            // Text/CData (handled by raw capture), Decl, comments, PIs,
            // doctypes: transparent.
            Ok(_) => continue,
            Err(e) => bail!("XML error: {e}"),
        }
    }
}

fn attrs_of(e: &quick_xml::events::BytesStart<'_>) -> Attrs {
    e.attributes()
        .flatten()
        .map(|a| {
            (
                a.key.as_ref().to_string(),
                a.normalized_value(quick_xml::XmlVersion::Implicit1_0)
                    .unwrap_or_default()
                    .into_owned(),
            )
        })
        .collect()
}

fn attr<'a>(attrs: &'a [(String, String)], key: &str) -> Option<&'a str> {
    attrs
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.as_str())
}

fn attr_i64(attrs: &[(String, String)], key: &str) -> Option<i64> {
    attr(attrs, key).and_then(|v| v.parse().ok())
}

/// The relationship id from a `<Rel r:id='…'/>` index child.
fn rid_of(attrs: &Attrs) -> Option<String> {
    attrs
        .iter()
        .find(|(k, _)| k.len() > 2 && k.ends_with(":id"))
        .map(|(_, v)| v.clone())
}

fn entry_rid(children: &[IndexChild]) -> Option<String> {
    children.iter().find_map(|c| match c {
        IndexChild::Rel(a) => rid_of(a),
        _ => None,
    })
}

/// The relationship id of an index entry: the `r:id` attribute on the entry
/// itself, or its `<Rel>` child (real packages use both forms).
fn index_rid(attrs: &Attrs, children: &[IndexChild]) -> Option<String> {
    rid_of(attrs).or_else(|| entry_rid(children))
}

/// Read the inner XML of the element whose start tag was just consumed,
/// up to (but not including) its matching end tag, reconstructed from the
/// event stream. Text and attribute values keep their original escaped
/// bytes, so the result is canonically identical to the source (quoting and
/// attribute spacing are normalized by [`crate::write::canonical_form`]).
/// `buffer_position()` arithmetic is deliberately avoided: quick-xml's
/// reported offsets drift around indented markup.
fn read_raw_inner(r: &mut quick_xml::Reader<&[u8]>) -> Result<String> {
    use quick_xml::events::Event;
    let mut out = String::new();
    let mut depth = 0usize;
    loop {
        let mut buf = Vec::new();
        let ev = r
            .read_event_into(&mut buf)
            .map_err(|e| anyhow::anyhow!("XML error in raw element: {e}"))?;
        match ev {
            Event::Start(e) => {
                push_reconstructed_open(&mut out, &e, false);
                depth += 1;
            }
            Event::End(e) => {
                if depth == 0 {
                    return Ok(out);
                }
                depth -= 1;
                out.push_str("</");
                out.push_str(e.name().as_ref());
                out.push('>');
            }
            Event::Empty(e) => push_reconstructed_open(&mut out, &e, true),
            Event::Text(t) => out.push_str(t.as_ref()),
            Event::GeneralRef(r) => {
                // Entity reference (`&lt;`, `&#65;`, …): re-emit verbatim.
                out.push('&');
                out.push_str(r.as_ref());
                out.push(';');
            }
            Event::CData(t) => {
                out.push_str("<![CDATA[");
                out.push_str(t.as_ref());
                out.push_str("]]>");
            }
            // Comments/PIs/decls are canonical no-ops; drop them.
            Event::Eof => bail!("EOF inside raw element"),
            _ => {}
        }
    }
}

/// `<name k="v" …>` (or self-closing) with raw escaped attribute values,
/// re-quoted with double quotes (safe: raw `"` bytes are escaped).
fn push_reconstructed_open(out: &mut String, e: &quick_xml::events::BytesStart<'_>, empty: bool) {
    out.push('<');
    out.push_str(e.name().as_ref());
    for a in e.attributes().flatten() {
        out.push(' ');
        out.push_str(a.key.as_ref());
        out.push_str("=\"");
        let v = a.value.to_string();
        out.push_str(&v.replace('"', "&quot;"));
        out.push('"');
    }
    out.push_str(if empty { "/>" } else { ">" });
}

/// Plain text of a raw XML fragment (markup stripped, entities resolved).
fn plain_text_of(raw: &str) -> String {
    let mut r = quick_xml::Reader::from_str(raw);
    let mut out = String::new();
    let mut buf = Vec::new();
    loop {
        match r.read_event_into(&mut buf) {
            Ok(quick_xml::events::Event::Text(t)) => out.push_str(t.as_ref()),
            Ok(quick_xml::events::Event::GeneralRef(r)) => {
                let mut ent = String::from("&");
                ent.push_str(r.as_ref());
                ent.push(';');
                out.push_str(&unescape(ent.as_bytes()));
            }
            Ok(quick_xml::events::Event::CData(t)) => out.push_str(t.as_ref()),
            Ok(quick_xml::events::Event::Eof) | Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    out
}

// ---------------------------------------------------------------------------
// Part parsers
// ---------------------------------------------------------------------------

/// Generic `<…Contents>` body: (root attrs, sheet, shapes) for page and
/// master parts. The sheet is the part's own `<PageSheet>`/`<MasterSheet>`
/// when present (real packages keep it in the index instead).
fn parse_contents(xml: &str) -> Result<(Attrs, Sheet, Vec<Shape>, Vec<RawEl>)> {
    let mut r = quick_xml::Reader::from_str(xml);
    let mut sheet = Sheet::default();
    let mut shapes = Vec::new();
    let mut raw = Vec::new();
    // Locate the root element.
    let (root_attrs, root_name): (Attrs, String) = loop {
        match next(&mut r)? {
            Ev::Start(name, attrs) => break (attrs, name),
            Ev::Empty(_, attrs) => return Ok((attrs, Sheet::default(), Vec::new(), Vec::new())),
            Ev::Eof => return Ok(Default::default()),
            _ => {}
        }
    };
    loop {
        match next(&mut r)? {
            Ev::Start(name, a) => match name.as_str() {
                "Shapes" => shapes = parse_shapes(&mut r)?,
                "PageSheet" | "MasterSheet" => sheet = parse_sheet_body(&mut r, a)?,
                _ => {
                    // Unknown contents child (`<Connects>`, …): verbatim.
                    raw.push(RawEl {
                        name: name.clone(),
                        attrs: a,
                        inner: read_raw_inner(&mut r)?,
                    });
                }
            },
            Ev::Empty(name, a) => {
                let name_str = String::from_utf8_lossy(name.as_ref()).into_owned();
                if name_str != "Shapes" && name_str != "PageSheet" && name_str != "MasterSheet" {
                    raw.push(RawEl {
                        name: name_str,
                        attrs: a,
                        inner: String::new(),
                    });
                }
            }
            Ev::End(n) if n == root_name => break,
            Ev::Text => {}
            Ev::End(_) | Ev::Eof => break,
        }
    }
    Ok((root_attrs, sheet, shapes, raw))
}

fn parse_document_xml(xml: &str, doc: &mut Document) -> Result<()> {
    let mut r = quick_xml::Reader::from_str(xml);
    // Locate the `<VisioDocument>` root.
    let root_attrs;
    loop {
        match next(&mut r)? {
            Ev::Start(name, attrs) => {
                root_attrs = attrs;
                if name != "VisioDocument" {
                    bail!("unexpected document root: {}", name.clone());
                }
                break;
            }
            Ev::Eof => bail!("no VisioDocument root"),
            _ => {}
        }
    }
    doc.root_attrs = root_attrs;
    loop {
        match next(&mut r)? {
            Ev::Start(name, a) => match name.as_str() {
                "StyleSheets" => {
                    doc.items.push(DocItem::StyleSheets);
                    loop {
                        match next(&mut r)? {
                            Ev::Start(n2, a2) if n2 == "StyleSheet" => {
                                let id = attr_i64(&a2, "ID").unwrap_or(0);
                                let sheet = parse_sheet_body(&mut r, a2.clone())?;
                                doc.stylesheets.push(StyleSheet {
                                    id,
                                    name_u: attr(&a2, "NameU").map(str::to_owned),
                                    attrs: a2,
                                    sheet,
                                });
                            }
                            Ev::Empty(n2, a2) if n2 == "StyleSheet" => {
                                let id = attr_i64(&a2, "ID").unwrap_or(0);
                                doc.stylesheets.push(StyleSheet {
                                    id,
                                    name_u: attr(&a2, "NameU").map(str::to_owned),
                                    attrs: a2,
                                    sheet: Sheet::default(),
                                });
                            }
                            Ev::Start(..) => skip_element(&mut r)?,
                            Ev::End(n2) if n2 == "StyleSheets" => break,
                            Ev::Eof => bail!("EOF inside StyleSheets"),
                            _ => {}
                        }
                    }
                }
                "Colors" => {
                    doc.items.push(DocItem::Colors);
                    loop {
                        match next(&mut r)? {
                            Ev::Start(n2, a2) if n2 == "ColorEntry" => {
                                if let (Some(ix), Some(rgb)) =
                                    (attr_i64(&a2, "IX"), attr(&a2, "RGB").map(str::to_owned))
                                {
                                    doc.colors.push((ix, rgb));
                                }
                                skip_element(&mut r)?;
                            }
                            Ev::Empty(n2, a2) if n2 == "ColorEntry" => {
                                if let (Some(ix), Some(rgb)) =
                                    (attr_i64(&a2, "IX"), attr(&a2, "RGB").map(str::to_owned))
                                {
                                    doc.colors.push((ix, rgb));
                                }
                            }
                            Ev::End(n2) if n2 == "Colors" => break,
                            Ev::Eof => bail!("EOF inside Colors"),
                            _ => {}
                        }
                    }
                }
                "DocumentSheet" => {
                    doc.items.push(DocItem::DocumentSheet);
                    doc.sheet = parse_sheet_body(&mut r, a)?;
                }
                _ => {
                    // Unknown document child (`FaceNames`, `DocumentSettings`,
                    // …): keep verbatim, in place.
                    let raw = RawEl {
                        name: name.clone(),
                        attrs: a,
                        inner: read_raw_inner(&mut r)?,
                    };
                    doc.items.push(DocItem::Raw(raw));
                }
            },
            Ev::Empty(name, a) => {
                doc.items.push(DocItem::Raw(RawEl {
                    name: name.clone(),
                    attrs: a,
                    inner: String::new(),
                }));
            }
            Ev::End(n) if n == "VisioDocument" => break,
            Ev::Eof => break,
            _ => {}
        }
    }
    Ok(())
}

#[derive(Debug, Default)]
struct IndexEntry {
    attrs: Attrs,
    children: Vec<IndexChild>,
}

/// Parse a `<Pages>`/`<Masters>` index part: (root attrs, entries in
/// document order). Entry children are kept in order: the inline
/// `PageSheet`/`MasterSheet`, the `<Rel>` carrying the part relationship,
/// and anything else (notably `<Icon>`) verbatim.
fn parse_index_entries(xml: &str, entry: &str) -> Result<(Attrs, Vec<IndexEntry>)> {
    let mut out = Vec::new();
    let mut r = quick_xml::Reader::from_str(xml);
    // Locate the root.
    let (root_attrs, root_name): (Attrs, String) = loop {
        match next(&mut r)? {
            Ev::Start(name, attrs) => break (attrs, name),
            Ev::Empty(_, attrs) => return Ok((attrs, Vec::new())),
            Ev::Eof => return Ok(Default::default()),
            _ => {}
        }
    };
    loop {
        match next(&mut r)? {
            Ev::Start(name, a) if name == entry => {
                let mut e = IndexEntry {
                    attrs: a,
                    children: Vec::new(),
                };
                loop {
                    match next(&mut r)? {
                        Ev::Start(n2, a2) if n2 == "PageSheet" || n2 == "MasterSheet" => {
                            let tag = n2.clone();
                            let sheet = parse_sheet_body(&mut r, a2)?;
                            e.children.push(IndexChild::Sheet { tag, sheet });
                        }
                        Ev::Start(n2, a2) if n2 == "Rel" => {
                            e.children.push(IndexChild::Rel(a2));
                            skip_element(&mut r)?;
                        }
                        Ev::Start(n2, a2) => {
                            // <Icon> base64 stencil art and anything else:
                            // keep verbatim.
                            let raw = RawEl {
                                name: n2.clone(),
                                attrs: a2,
                                inner: read_raw_inner(&mut r)?,
                            };
                            e.children.push(IndexChild::Raw(raw));
                        }
                        Ev::Empty(n2, a2) if n2 == "Rel" => {
                            e.children.push(IndexChild::Rel(a2));
                        }
                        Ev::Empty(n2, a2) => {
                            e.children.push(IndexChild::Raw(RawEl {
                                name: n2.clone(),
                                attrs: a2,
                                inner: String::new(),
                            }));
                        }
                        Ev::End(n2) if n2 == entry => break,
                        Ev::Eof => bail!("EOF inside index entry"),
                        _ => {}
                    }
                }
                out.push(e);
            }
            Ev::Empty(name, a) if name == entry => {
                out.push(IndexEntry {
                    attrs: a,
                    children: Vec::new(),
                });
            }
            Ev::End(n) if n == root_name => break,
            Ev::Eof => break,
            _ => {}
        }
    }
    Ok((root_attrs, out))
}

/// Parse a `<Shapes>` block (opening tag consumed).
fn parse_shapes(r: &mut quick_xml::Reader<&[u8]>) -> Result<Vec<Shape>> {
    let mut shapes = Vec::new();
    loop {
        match next(r)? {
            Ev::Start(name, attrs) => {
                if name == "Shape" {
                    shapes.push(parse_shape(r, attrs)?);
                } else {
                    skip_element(r)?;
                }
            }
            Ev::Empty(name, attrs) => {
                // Deleted/childless shapes: `<Shape Del='1' …/>`.
                if name == "Shape" {
                    shapes.push(shape_from_attrs(attrs));
                }
            }
            Ev::End(name) if name == "Shapes" => break,
            Ev::Eof => break,
            _ => {}
        }
    }
    Ok(shapes)
}

/// A `<Shape …/>` with no children.
fn shape_from_attrs(attrs: Attrs) -> Shape {
    Shape {
        id: attr_i64(&attrs, "ID").unwrap_or(0),
        name_u: attr(&attrs, "NameU").map(str::to_owned),
        type_: attr(&attrs, "Type").map(str::to_owned),
        master: attr_i64(&attrs, "Master"),
        master_shape: attr_i64(&attrs, "MasterShape"),
        line_style: attr_i64(&attrs, "LineStyle"),
        fill_style: attr_i64(&attrs, "FillStyle"),
        text_style: attr_i64(&attrs, "TextStyle"),
        attrs,
        ..Default::default()
    }
}

fn parse_shape(r: &mut quick_xml::Reader<&[u8]>, attrs: Attrs) -> Result<Shape> {
    let mut s = Shape {
        id: attr_i64(&attrs, "ID").unwrap_or(0),
        name_u: attr(&attrs, "NameU").map(str::to_owned),
        type_: attr(&attrs, "Type").map(str::to_owned),
        master: attr_i64(&attrs, "Master"),
        master_shape: attr_i64(&attrs, "MasterShape"),
        line_style: attr_i64(&attrs, "LineStyle"),
        fill_style: attr_i64(&attrs, "FillStyle"),
        text_style: attr_i64(&attrs, "TextStyle"),
        attrs,
        ..Default::default()
    };
    loop {
        match next(r)? {
            Ev::Start(name, a) => match name.as_str() {
                "Cell" => {
                    if let Some((n, cell)) = parse_cell(&a) {
                        let cell = with_raw_inner(cell, r)?;
                        upsert_cell(&mut s.sheet.children, n, cell);
                    }
                }
                "Section" => {
                    let sec = parse_section(r, a)?;
                    upsert_section(&mut s.sheet.children, sec);
                }
                "Shapes" => {
                    s.sub_shapes = parse_shapes(r)?;
                    s.flow.push(ShapeFlow::Shapes);
                }
                "Text" => {
                    let raw = read_raw_inner(r)?;
                    let plain = plain_text_of(&raw);
                    s.sheet.text_raw = Some(raw);
                    if !plain.trim().is_empty() {
                        s.sheet.text = Some(plain);
                    }
                    s.flow.push(ShapeFlow::Text);
                }
                _ => {
                    // Unknown shape child (`ForeignData`, …): verbatim.
                    let raw = RawEl {
                        name: name.clone(),
                        attrs: a,
                        inner: read_raw_inner(r)?,
                    };
                    s.raw_children.push(raw);
                    s.flow.push(ShapeFlow::Raw(s.raw_children.len() - 1));
                }
            },
            Ev::Empty(name, a) => match name.as_str() {
                "Cell" => {
                    if let Some((n, cell)) = parse_cell(&a) {
                        upsert_cell(&mut s.sheet.children, n, cell);
                    }
                }
                "Section" => upsert_section(&mut s.sheet.children, empty_section(a)),
                "Text" => {
                    s.sheet.text_raw = Some(String::new());
                    s.flow.push(ShapeFlow::Text);
                }
                _ => {
                    let name_str = String::from_utf8_lossy(name.as_ref()).into_owned();
                    if name_str != "Cell" {
                        s.raw_children.push(RawEl {
                            name: name_str,
                            attrs: a,
                            inner: String::new(),
                        });
                        s.flow.push(ShapeFlow::Raw(s.raw_children.len() - 1));
                    }
                }
            },
            Ev::End(name) if name == "Shape" => break,
            Ev::Text => {}
            Ev::End(_) | Ev::Eof => break,
        }
    }
    Ok(s)
}

fn parse_cell(attrs: &Attrs) -> Option<(String, Cell)> {
    let n = attr(attrs, "N")?.to_owned();
    Some((
        n,
        Cell {
            v: attr(attrs, "V").map(str::to_owned),
            u: attr(attrs, "U").map(str::to_owned),
            f: attr(attrs, "F").map(str::to_owned),
            attrs: attrs.clone(),
            raw_inner: None,
        },
    ))
}

/// Attach the raw inner XML of a `<Cell …>` whose start tag was just read
/// (consumes through its end tag; `<RefBy/>` children survive verbatim).
fn with_raw_inner(cell: Cell, r: &mut quick_xml::Reader<&[u8]>) -> Result<Cell> {
    let raw = read_raw_inner(r)?;
    let mut cell = cell;
    if !raw.is_empty() {
        cell.raw_inner = Some(raw);
    }
    Ok(cell)
}

/// Insert or replace a `(name, cell)` pair, preserving position on replace.
fn upsert_pair(cells: &mut Vec<(String, Cell)>, n: String, cell: Cell) {
    if let Some(slot) = cells.iter_mut().find(|(en, _)| *en == n) {
        slot.1 = cell;
    } else {
        cells.push((n, cell));
    }
}

/// Append or replace a cell child in a sheet (replace keeps position).
fn upsert_cell(children: &mut Vec<SheetChild>, n: String, cell: Cell) {
    for child in children.iter_mut() {
        if let SheetChild::Cell(en, slot) = child {
            if *en == n {
                *slot = cell;
                return;
            }
        }
    }
    children.push(SheetChild::Cell(n, cell));
}

/// Append or replace a section child by (name, ix) (replace keeps position).
fn upsert_section(children: &mut Vec<SheetChild>, sec: Section) {
    for child in children.iter_mut() {
        if let SheetChild::Section(slot) = child {
            if slot.name == sec.name && slot.ix == sec.ix {
                *slot = sec;
                return;
            }
        }
    }
    children.push(SheetChild::Section(sec));
}

/// A `<Section …/>` with neither cells nor rows.
fn empty_section(attrs: Attrs) -> Section {
    Section {
        name: attr(&attrs, "N").unwrap_or_default().to_owned(),
        ix: attr_i64(&attrs, "IX").unwrap_or(0),
        attrs,
        cells: Vec::new(),
        rows: Vec::new(),
    }
}

/// A `<Row …/>` with no cells.
fn empty_row(attrs: Attrs) -> Row {
    Row {
        t: attr(&attrs, "T").map(str::to_owned),
        ix: attr_i64(&attrs, "IX").unwrap_or(0),
        n: attr(&attrs, "N").map(str::to_owned),
        attrs,
        cells: Vec::new(),
    }
}

fn parse_section(r: &mut quick_xml::Reader<&[u8]>, attrs: Attrs) -> Result<Section> {
    let name = attr(&attrs, "N").unwrap_or_default().to_owned();
    let ix = attr_i64(&attrs, "IX").unwrap_or(0);
    let mut cells = Vec::new();
    let mut rows = Vec::new();
    loop {
        match next(r)? {
            Ev::Start(name, a) => match name.as_str() {
                "Row" => rows.push(parse_row(r, a)?),
                "Cell" => {
                    if let Some((n, cell)) = parse_cell(&a) {
                        let cell = with_raw_inner(cell, r)?;
                        upsert_pair(&mut cells, n, cell);
                    }
                }
                _ => skip_element(r)?,
            },
            Ev::Empty(name, a) => match name.as_str() {
                "Row" => rows.push(empty_row(a)),
                "Cell" => {
                    if let Some((n, cell)) = parse_cell(&a) {
                        upsert_pair(&mut cells, n, cell);
                    }
                }
                _ => {}
            },
            Ev::End(name) if name == "Section" => break,
            Ev::Text => {}
            Ev::End(_) | Ev::Eof => break,
        }
    }
    Ok(Section {
        name,
        ix,
        attrs,
        cells,
        rows,
    })
}

fn parse_row(r: &mut quick_xml::Reader<&[u8]>, attrs: Attrs) -> Result<Row> {
    let mut row = Row {
        t: attr(&attrs, "T").map(str::to_owned),
        ix: attr_i64(&attrs, "IX").unwrap_or(0),
        n: attr(&attrs, "N").map(str::to_owned),
        attrs,
        cells: Vec::new(),
    };
    loop {
        match next(r)? {
            Ev::Start(name, a) => {
                if name == "Cell" {
                    if let Some((n, cell)) = parse_cell(&a) {
                        let cell = with_raw_inner(cell, r)?;
                        upsert_pair(&mut row.cells, n, cell);
                    }
                } else {
                    skip_element(r)?;
                }
            }
            Ev::Empty(name, a) => {
                if name == "Cell" {
                    if let Some((n, cell)) = parse_cell(&a) {
                        upsert_pair(&mut row.cells, n, cell);
                    }
                }
            }
            Ev::End(name) if name == "Row" => break,
            Ev::Text => {}
            Ev::End(_) | Ev::Eof => break,
        }
    }
    Ok(row)
}

/// Parse a sheet body (cells + sections) until the sheet's closing tag.
/// `attrs` are the opening-tag attributes (`PageSheet LineStyle='0' …`).
/// Parse a sheet body (cells/sections/raw children in document order) until
/// the sheet's closing tag. `attrs` are the opening-tag attributes
/// (`PageSheet LineStyle='0' …`).
fn parse_sheet_body(r: &mut quick_xml::Reader<&[u8]>, attrs: Attrs) -> Result<Sheet> {
    let mut sheet = Sheet {
        attrs,
        ..Default::default()
    };
    loop {
        match next(r)? {
            Ev::Start(name, a) => match name.as_str() {
                "Cell" => {
                    if let Some((n, cell)) = parse_cell(&a) {
                        let cell = with_raw_inner(cell, r)?;
                        upsert_cell(&mut sheet.children, n, cell);
                    }
                }
                "Section" => {
                    let sec = parse_section(r, a)?;
                    upsert_section(&mut sheet.children, sec);
                }
                _ => {
                    // Unknown sheet child (`<Trigger>`, …): verbatim.
                    sheet.children.push(SheetChild::Raw(RawEl {
                        name: name.clone(),
                        attrs: a,
                        inner: read_raw_inner(r)?,
                    }));
                }
            },
            Ev::Empty(name, a) => match name.as_str() {
                "Cell" => {
                    if let Some((n, cell)) = parse_cell(&a) {
                        upsert_cell(&mut sheet.children, n, cell);
                    }
                }
                "Section" => upsert_section(&mut sheet.children, empty_section(a)),
                _ => {
                    let name_str = String::from_utf8_lossy(name.as_ref()).into_owned();
                    if name_str != "Cell" {
                        sheet.children.push(SheetChild::Raw(RawEl {
                            name: name_str,
                            attrs: a,
                            inner: String::new(),
                        }));
                    }
                }
            },
            Ev::Text => {}
            Ev::End(_) | Ev::Eof => break,
        }
    }
    Ok(sheet)
}

/// Skip through the end tag of the element just opened.
fn skip_element(r: &mut quick_xml::Reader<&[u8]>) -> Result<()> {
    let mut depth = 0usize;
    loop {
        match next(r)? {
            Ev::Start(..) => depth += 1,
            Ev::End(_) => {
                if depth == 0 {
                    return Ok(());
                }
                depth -= 1;
            }
            Ev::Eof => return Ok(()),
            _ => {}
        }
    }
}
