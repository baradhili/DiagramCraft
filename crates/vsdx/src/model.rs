//! The VSDX document model: sheets of cells/sections, with master and
//! stylesheet inheritance resolution.
//!
//! v0.2: every container is ordered and every element keeps its original
//! attribute list, so a parsed document can be written back without data
//! loss (unknown cells, attributes, and whole elements survive verbatim).

use std::borrow::Cow;

use crate::opc::Package;

/// Ordered attribute list, in document order (round-trip fidelity).
pub type Attrs = Vec<(String, String)>;

/// An element the model does not interpret (`Icon`, `FaceNames`,
/// `DocumentSettings`, …), kept verbatim: its opening-tag attributes plus
/// the raw inner XML, re-emitted unchanged on write.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RawEl {
    pub name: String,
    pub attrs: Attrs,
    /// Inner XML exactly as read (children, text, entities, CDATA).
    pub inner: String,
}

/// One ShapeSheet cell: `N` name, `V` value, `U` unit, `F` formula.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Cell {
    pub v: Option<String>,
    pub u: Option<String>,
    pub f: Option<String>,
    /// Full original attribute list (`N`, `V`, `U`, `F`, `E`, …) in
    /// document order; kept so writing is lossless.
    pub attrs: Attrs,
    /// Raw inner XML when the cell has children (`<RefBy/>`), verbatim.
    pub raw_inner: Option<String>,
}

impl Cell {
    /// A plain value cell (no unit, no formula) — for tests and editing.
    pub fn value(v: impl Into<String>) -> Cell {
        Cell {
            v: Some(v.into()),
            ..Cell::default()
        }
    }
    pub fn numeric(&self) -> Option<f64> {
        self.v.as_deref().and_then(|v| v.trim().parse::<f64>().ok())
    }
    pub fn is_inherited(&self) -> bool {
        self.f.as_deref() == Some("Inh")
    }
}

/// A row inside a section (optionally typed, e.g. `T='MoveTo'`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Row {
    /// Row type (`MoveTo`, `LineTo`, …) for geometry rows; `None` otherwise.
    pub t: Option<String>,
    /// Row index within the section (the `IX` attribute), 0 when absent.
    pub ix: i64,
    /// Named row (`N='Label'`) for named sections (User, Prop, …).
    pub n: Option<String>,
    /// Full original attribute list, in document order.
    pub attrs: Attrs,
    pub cells: Vec<(String, Cell)>,
}

impl Row {
    pub fn cell(&self, name: &str) -> Option<&Cell> {
        self.cells.iter().find(|(n, _)| n == name).map(|(_, c)| c)
    }
}

/// A sheet section, e.g. `Geometry`, `User`, `Character`, `Connection`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Section {
    /// Section name (`N` attribute).
    pub name: String,
    /// Section index (`IX`), 0 for the first Geometry, etc.
    pub ix: i64,
    /// Full original attribute list, in document order.
    pub attrs: Attrs,
    /// Section-level cells (e.g. Geometry `NoFill`/`NoLine`), in order.
    pub cells: Vec<(String, Cell)>,
    pub rows: Vec<Row>,
}

impl Section {
    pub fn cell(&self, name: &str) -> Option<&Cell> {
        self.cells.iter().find(|(n, _)| n == name).map(|(_, c)| c)
    }
    pub fn row_by_ix(&self, ix: i64) -> Option<&Row> {
        self.rows.iter().find(|r| r.ix == ix)
    }
    pub fn named_row(&self, name: &str) -> Option<&Row> {
        self.rows.iter().find(|r| r.n.as_deref() == Some(name))
    }
}

/// One child of a sheet body, in document order. Real sheets interleave
/// `Cell`s with `Trigger`s (and occasionally other elements), so order is
/// part of the round-trip contract.
#[derive(Debug, Clone, PartialEq)]
pub enum SheetChild {
    Cell(String, Cell),
    Section(Section),
    Raw(RawEl),
}

/// A ShapeSheet: an ordered child list plus the derived text fields.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Sheet {
    /// Opening-tag attributes (`PageSheet LineStyle='0' …`).
    pub attrs: Attrs,
    /// Cells, sections, and raw elements (`Trigger`, …) in document order.
    pub children: Vec<SheetChild>,
    /// Plain text content of `<Text>` (markup stripped) — rendering input.
    pub text: Option<String>,
    /// Raw inner XML of `<Text>` (markup, entities, CDATA verbatim) —
    /// writing output. `Some("")` for an empty `<Text/>`.
    pub text_raw: Option<String>,
}

impl Sheet {
    pub fn cell(&self, name: &str) -> Option<&Cell> {
        self.children.iter().find_map(|c| match c {
            SheetChild::Cell(n, cell) if n == name => Some(cell),
            _ => None,
        })
    }

    /// Insert or replace a cell by name, preserving position on replace.
    pub fn set_cell(&mut self, name: &str, cell: Cell) {
        let cell = Cell {
            attrs: {
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
            },
            ..cell
        };
        for child in self.children.iter_mut() {
            if let SheetChild::Cell(n, slot) = child {
                if n == name {
                    *slot = cell;
                    return;
                }
            }
        }
        self.children.push(SheetChild::Cell(name.to_string(), cell));
    }

    /// Whether the sheet carries any section.
    pub fn has_sections(&self) -> bool {
        self.children
            .iter()
            .any(|c| matches!(c, SheetChild::Section(_)))
    }

    /// All sections with the given name, ordered by IX.
    pub fn sections_named(&self, name: &str) -> Vec<&Section> {
        let mut v: Vec<&Section> = self
            .children
            .iter()
            .filter_map(|c| match c {
                SheetChild::Section(s) if s.name == name => Some(s),
                _ => None,
            })
            .collect();
        v.sort_by_key(|s| s.ix);
        v
    }

    /// The section with exactly this (name, ix), if present.
    pub fn section(&self, name: &str, ix: i64) -> Option<&Section> {
        self.children.iter().find_map(|c| match c {
            SheetChild::Section(s) if s.name == name && s.ix == ix => Some(s),
            _ => None,
        })
    }
}

/// A shape (page-level or nested group member).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Shape {
    pub id: i64,
    pub name_u: Option<String>,
    /// `Type` attribute: `Shape`, `Group`, `Foreign`, `Page`, …
    pub type_: Option<String>,
    /// `Master` attribute — master ID this shape instantiates.
    pub master: Option<i64>,
    /// `MasterShape` attribute — master sub-shape ID this shape maps to.
    pub master_shape: Option<i64>,
    /// `LineStyle` / `FillStyle` / `TextStyle` attribute defaults (IDs).
    pub line_style: Option<i64>,
    pub fill_style: Option<i64>,
    pub text_style: Option<i64>,
    /// Full original attribute list (`ID`, `NameU`, `UniqueID`, …).
    pub attrs: Attrs,
    pub sheet: Sheet,
    /// Order of the post-sheet children (`<Text>`, raw children like
    /// `<ForeignData>`, `<Shapes>`) as they appeared in the source.
    pub flow: Vec<ShapeFlow>,
    /// Unknown shape children (`ForeignData`, …), kept verbatim in order.
    pub raw_children: Vec<RawEl>,
    pub sub_shapes: Vec<Shape>,
}

/// Where a shape's `<Text>`, raw children, and `<Shapes>` appeared.
#[derive(Debug, Clone, PartialEq)]
pub enum ShapeFlow {
    Text,
    Raw(usize),
    Shapes,
}

/// A child of a `<Page>`/`<Master>` index entry, in document order.
#[derive(Debug, Clone, PartialEq)]
pub enum IndexChild {
    /// `<PageSheet>`/`<MasterSheet>` — `tag` preserves which, `sheet.attrs`
    /// hold the opening-tag attributes.
    Sheet { tag: String, sheet: Sheet },
    /// `<Rel r:id='…'/>`.
    Rel(Attrs),
    /// Anything else (notably `<Icon>`), kept verbatim.
    Raw(RawEl),
}

/// A master (stencil shape definition): its index entry and shape tree.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Master {
    pub id: i64,
    pub name_u: Option<String>,
    /// Masters-index `<Master>` attributes, in order.
    pub attrs: Attrs,
    /// Index children (`MasterSheet`, `Rel`, `Icon`, …) in order.
    pub children: Vec<IndexChild>,
    /// The master's shape tree from its part (usually one root shape).
    pub shapes: Vec<Shape>,
    /// Sheet from the master part itself, if any (rare).
    pub contents_sheet: Sheet,
    /// Opening-tag attrs of the `<MasterContents>` root.
    pub contents_attrs: Attrs,
    /// Unknown children of the contents root, verbatim.
    pub contents_raw: Vec<RawEl>,
    /// Part name the master contents were parsed from.
    pub part: String,
}

impl Master {
    /// The master's sheet: the index `PageSheet`/`MasterSheet` when present,
    /// else the part's own sheet.
    pub fn sheet(&self) -> Option<&Sheet> {
        self.children.iter().find_map(|c| match c {
            IndexChild::Sheet { sheet, .. } => Some(sheet),
            _ => None,
        })
    }
}

/// A page: its pages-index entry plus the shape tree from its part.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Page {
    pub id: i64,
    pub name: String,
    /// Pages-index `<Page>` attributes (`ID`, `NameU`, `ViewScale`, …).
    pub attrs: Attrs,
    /// Index children (`PageSheet`, `Rel`, …) in order.
    pub children: Vec<IndexChild>,
    pub shapes: Vec<Shape>,
    /// Opening-tag attrs of the `<PageContents>` root.
    pub contents_attrs: Attrs,
    /// Unknown children of the contents root (`<Connects>`, …), verbatim.
    pub contents_raw: Vec<RawEl>,
    /// Part name the page contents were parsed from ("" for stencil pages
    /// that have no page part).
    pub part: String,
}

impl Page {
    /// The page's sheet: the inline `<PageSheet>` from the pages index.
    pub fn sheet(&self) -> Option<&Sheet> {
        self.children.iter().find_map(|c| match c {
            IndexChild::Sheet { sheet, .. } => Some(sheet),
            _ => None,
        })
    }
}

/// A stylesheet from `document.xml`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct StyleSheet {
    pub id: i64,
    pub name_u: Option<String>,
    pub attrs: Attrs,
    pub sheet: Sheet,
}

/// A child of `<VisioDocument>`, in document order (round-trip fidelity:
/// unknown children are kept as [`RawEl`] and re-emitted in place).
#[derive(Debug, Clone, PartialEq)]
pub enum DocItem {
    StyleSheets,
    Colors,
    DocumentSheet,
    Raw(RawEl),
}

/// A fully parsed VSDX-family package.
#[derive(Debug, Clone, Default)]
pub struct Document {
    pub pages: Vec<Page>,
    /// Masters in masters-index order.
    pub masters: Vec<Master>,
    /// Stylesheets in document order.
    pub stylesheets: Vec<StyleSheet>,
    /// Color table entries (`Colors`) in document order.
    pub colors: Vec<(i64, String)>,
    /// `<VisioDocument>` children in document order.
    pub items: Vec<DocItem>,
    /// Document-level sheet (`DocumentSheet`).
    pub sheet: Sheet,
    /// `<VisioDocument>` opening-tag attributes (namespaces included).
    pub root_attrs: Attrs,
    /// `<Pages>`/`<Masters>` index root attributes.
    pub pages_attrs: Attrs,
    pub masters_attrs: Attrs,
    /// Main document part name (usually `visio/document.xml`).
    pub doc_part: String,
    /// Pages-index part name (usually `visio/pages/pages.xml`).
    pub pages_part: String,
    /// Masters-index part name, when the document has masters.
    pub masters_part: Option<String>,
    /// The raw source package, retained so unmodeled parts round-trip
    /// byte-for-byte. `None` for documents built in code.
    pub package: Option<Package>,
}

impl Document {
    pub fn master(&self, id: i64) -> Option<&Master> {
        self.masters.iter().find(|m| m.id == id)
    }
    pub fn stylesheet(&self, id: i64) -> Option<&StyleSheet> {
        self.stylesheets.iter().find(|s| s.id == id)
    }
    pub fn color(&self, ix: i64) -> Option<&String> {
        self.colors.iter().find(|(i, _)| *i == ix).map(|(_, c)| c)
    }

    /// Deep model equality ignoring the retained raw package (part bytes
    /// legitimately differ after a write).
    pub fn same_model(&self, other: &Document) -> bool {
        self.pages == other.pages
            && self.masters == other.masters
            && self.stylesheets == other.stylesheets
            && self.colors == other.colors
            && self.items == other.items
            && self.sheet == other.sheet
            && self.root_attrs == other.root_attrs
            && self.doc_part == other.doc_part
            && self.pages_part == other.pages_part
            && self.masters_part == other.masters_part
    }

    /// The master-side shape whose sheet an instance inherits from.
    ///
    /// `Master='8'` resolves inside master 8 (optionally to its `MasterShape`
    /// sub-shape). A shape carrying only `MasterShape` inherits from that
    /// sub-shape of the nearest enclosing group's master — the standard
    /// layout for group instances. A bare sub-shape of a master-backed group
    /// falls back to ID matching inside that master.
    pub fn instance_source<'a>(
        &'a self,
        shape: &'a Shape,
        ancestors: &[&'a Shape],
    ) -> Option<&'a Shape> {
        if let Some(mid) = shape.master {
            let m = self.master(mid)?;
            return match shape.master_shape {
                Some(msid) => find_shape_by_id(&m.shapes, msid),
                None => m.shapes.first(),
            };
        }
        // Nearest ancestor that instantiates a master.
        let enclosing = ancestors.iter().rev().find(|a| a.master.is_some())?;
        let m = self.master(enclosing.master?)?;
        if let Some(msid) = shape.master_shape {
            return find_shape_by_id(&m.shapes, msid);
        }
        // Bare sub-shape: match by ID inside the master tree. Visio
        // renumbers inline copies (`Name.NNN` suffix), so fall back to the
        // NameU base when the ID does not exist in the master.
        find_shape_by_id(&m.shapes, shape.id).or_else(|| {
            let name = shape.name_u.as_deref()?;
            let (base, sfx) = name.rsplit_once('.')?;
            if sfx.is_empty() || !sfx.chars().all(|c| c.is_ascii_digit()) {
                return None;
            }
            find_shape_by_name_u(&m.shapes, base)
        })
    }

    /// Resolve a cell for `shape` walking the inheritance chain:
    /// shape → master root shape (or master sub-shape via `MasterShape`)
    /// → stylesheet (shape's LineStyle/FillStyle/TextStyle default 0) →
    /// built-in defaults.
    /// Like [`resolve_cell`](Self::resolve_cell) but returns `None` when
    /// no explicit value exists on the shape, its master, or the style
    /// cascade — i.e. stops before the document-sheet and builtin
    /// fallbacks. Used to spot values that are only defaults (e.g. 1-D
    /// connectors must not inherit the builtin FillPattern=1).
    pub fn resolve_cell_explicit(
        &self,
        shape: &Shape,
        ancestors: &[&Shape],
        name: &str,
    ) -> Option<Cell> {
        if let Some(c) = shape.sheet.cell(name) {
            if c.v.is_some() && is_xform_cell(name) {
                return Some(c.clone());
            }
            if !c.is_inherited() {
                return Some(c.clone());
            }
        }
        if let Some(t) = self.instance_source(shape, ancestors) {
            if let Some(c) = t.sheet.cell(name) {
                if !c.is_inherited() {
                    return Some(c.clone());
                }
            }
        }
        let style_id = match name {
            n if n.starts_with("Line") => shape.line_style,
            n if n.starts_with("Fill") => shape.fill_style,
            n if n.starts_with("Txt") | n.starts_with("Text") => shape.text_style,
            _ => shape.line_style.or(shape.fill_style),
        }
        .unwrap_or(0);
        if let Some(style) = self.stylesheet(style_id) {
            let mut cur = style;
            for _ in 0..8 {
                if let Some(c) = cur.sheet.cell(name) {
                    if !c.is_inherited() {
                        return Some(c.clone());
                    }
                }
                let next_id = cur
                    .sheet
                    .cell("LineStyle")
                    .and_then(|c| c.numeric())
                    .map(|v| v as i64);
                match next_id {
                    Some(nid) if nid != cur.id => match self.stylesheet(nid) {
                        Some(next) => cur = next,
                        None => break,
                    },
                    _ => break,
                }
            }
        }
        None
    }

    pub fn resolve_cell(&self, shape: &Shape, ancestors: &[&Shape], name: &str) -> Option<Cell> {
        if let Some(c) = shape.sheet.cell(name) {
            // Xform/placement cells keep an instance-local snapshot under
            // F='Inh': Visio caches the resolved position in `V` when the
            // master is instanced (the master's own value is its canvas
            // placement, meaningless on the page). Style cells
            // (LineWeight, FillForegnd, …) live-inherit: master wins.
            if c.v.is_some() && is_xform_cell(name) {
                return Some(c.clone());
            }
            if !c.is_inherited() {
                return Some(c.clone());
            }
        }
        // Master inheritance.
        if let Some(t) = self.instance_source(shape, ancestors) {
            if let Some(c) = t.sheet.cell(name) {
                if !c.is_inherited() {
                    return Some(c.clone());
                }
            }
        }
        // Stylesheet cascade.
        let style_id = match name {
            n if n.starts_with("Line") => shape.line_style,
            n if n.starts_with("Fill") => shape.fill_style,
            n if n.starts_with("Txt") | n.starts_with("Text") => shape.text_style,
            _ => shape.line_style.or(shape.fill_style),
        }
        .unwrap_or(0);
        if let Some(style) = self.stylesheet(style_id) {
            let mut cur = style;
            for _ in 0..8 {
                if let Some(c) = cur.sheet.cell(name) {
                    if !c.is_inherited() {
                        return Some(c.clone());
                    }
                }
                let next_id = cur
                    .sheet
                    .cell("LineStyle")
                    .and_then(|c| c.numeric())
                    .map(|v| v as i64);
                match next_id {
                    Some(nid) if nid != cur.id => match self.stylesheet(nid) {
                        Some(next) => cur = next,
                        None => break,
                    },
                    _ => break,
                }
            }
        }
        if let Some(c) = self.sheet.cell(name) {
            if !c.is_inherited() {
                return Some(c.clone());
            }
        }
        builtin_defaults()
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, v)| Cell {
                v: Some(v.to_string()),
                u: None,
                f: None,
                attrs: Default::default(),
                raw_inner: None,
            })
    }

    /// The shape (or master shape) whose sheet geometry should be rendered
    /// for `shape` — i.e. the effective sheet after master inheritance.
    pub fn effective_sheet<'a>(
        &'a self,
        shape: &'a Shape,
        ancestors: &[&'a Shape],
    ) -> Cow<'a, Sheet> {
        if shape.sheet.has_sections() {
            return Cow::Borrowed(&shape.sheet);
        }
        if let Some(src) = self.instance_source(shape, ancestors) {
            return Cow::Borrowed(&src.sheet);
        }
        Cow::Borrowed(&shape.sheet)
    }
}

/// Built-in defaults observed in shipped stylesheets ("No Style", ID 0).
pub fn builtin_defaults() -> Vec<(&'static str, f64)> {
    vec![
        // (name, value in internal units: inches / radians / ints)
        ("LineWeight", 0.01041666666666667),
        ("LinePattern", 1.0),
        ("LineColor", 0.0),
        ("FillPattern", 1.0),
        ("FillForegnd", 1.0),
        ("FillBkgnd", 0.0),
        ("BeginArrow", 0.0),
        ("EndArrow", 0.0),
        ("Rounding", 0.0),
    ]
}

/// Cells whose `F='Inh'` marks an instance-time snapshot rather than live
/// master inheritance (see `resolve_cell`).
fn is_xform_cell(name: &str) -> bool {
    matches!(
        name,
        "PinX"
            | "PinY"
            | "LocPinX"
            | "LocPinY"
            | "Width"
            | "Height"
            | "Angle"
            | "FlipX"
            | "FlipY"
            | "TxtPinX"
            | "TxtPinY"
            | "TxtLocPinX"
            | "TxtLocPinY"
            | "TxtWidth"
            | "TxtHeight"
            | "TxtAngle"
            | "BeginX"
            | "BeginY"
            | "EndX"
            | "EndY"
    )
}

pub fn find_shape_by_id(shapes: &[Shape], id: i64) -> Option<&Shape> {
    for s in shapes {
        if s.id == id {
            return Some(s);
        }
        if let Some(found) = find_shape_by_id(&s.sub_shapes, id) {
            return Some(found);
        }
    }
    None
}

/// Depth-first `NameU` lookup within a shape tree (used for matching inline
/// copies to their master originals).
pub fn find_shape_by_name_u<'a>(shapes: &'a [Shape], name: &str) -> Option<&'a Shape> {
    for s in shapes {
        if s.name_u.as_deref() == Some(name) {
            return Some(s);
        }
        if let Some(found) = find_shape_by_name_u(&s.sub_shapes, name) {
            return Some(found);
        }
    }
    None
}
