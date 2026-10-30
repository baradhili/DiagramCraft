//! The VSDX document model: sheets of cells/sections, with master and
//! stylesheet inheritance resolution.
//!
//! v0.2: every container is ordered and every element keeps its original
//! attribute list, so a parsed document can be written back without data
//! loss (unknown cells, attributes, and whole elements survive verbatim).

use std::borrow::Cow;

use crate::formula;
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

/// A media part added at runtime (GitHub #113, picture insertion):
/// the package part (`visio/media/imageN.png`), its bytes, the OWNING
/// page id, and the relationship id that page's `ForeignData`
/// references — the writer materializes the part, the page-part rels
/// entry, and the content-type default.
#[derive(Debug, Clone, PartialEq)]
pub struct MediaRef {
    pub part: String,
    pub bytes: Vec<u8>,
    pub page: i64,
    pub rid: String,
    /// `png` / `jpeg` — extension and content-type family.
    pub ext: String,
}

/// A fully parsed VSDX-family package.
#[derive(Debug, Clone, Default)]
pub struct Document {
    pub pages: Vec<Page>,
    /// Runtime-added media parts (#113): empty for parsed-only docs;
    /// the write path merges them (retained-package documents only).
    pub extra_media: Vec<MediaRef>,
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

/// Evaluation environment for re-evaluating a master root's cells at
/// instance size — shared by the SVG renderer's master mirroring
/// (issue #15) and the editor's flattened-group cache refresh
/// (issue #59), so both compute placements identically. Seeds the
/// group's effective `Width`/`Height` under the bare and `Sheet.N!`
/// spellings master formulas use, plus the root's User cells — their
/// formulas re-evaluate too, because layout cells like
/// `User.IconY = Height - User.InfoPanelHeight/2` reference the
/// (instance) `Height`. Instance-local User rows override the master's
/// (an instance can change editable rows such as `ShapeLayout`), and
/// two passes let a User formula reference another User formula.
pub fn instance_env(
    master_root: &Shape,
    instance: &Shape,
    width: f64,
    height: f64,
) -> formula::Env {
    let mut env = formula::Env::new();
    env.insert("Width".into(), width);
    env.insert("Height".into(), height);
    env.insert(format!("Sheet.{}!Width", master_root.id), width);
    env.insert(format!("Sheet.{}!Height", master_root.id), height);
    let mut users: Vec<(String, Cell)> = Vec::new();
    for user in master_root.sheet.sections_named("User") {
        for row in user.rows.iter().filter(|r| r.n.is_some()) {
            if let Some(c) = row.cell("Value") {
                users.push((row.n.clone().unwrap(), c.clone()));
            }
        }
    }
    for user in instance.sheet.sections_named("User") {
        for row in user.rows.iter().filter(|r| r.n.is_some()) {
            if let Some(c) = row.cell("Value") {
                // An instance row cached under F='Inh' is a frozen
                // snapshot of the master's formula — the master row (and
                // its formula, re-evaluated below) still owns the value.
                // Only a genuine local override (V with no F, or a real
                // formula — e.g. an instance's ShapeLayout=4) replaces
                // the master's row. Treating caches as overrides would
                // freeze layout cells like User.IconY at their save-time
                // value when re-evaluating after an edit (issue #59).
                if c.is_inherited() {
                    continue;
                }
                if let Some(slot) = users
                    .iter_mut()
                    .find(|(n, _)| n == row.n.as_deref().unwrap())
                {
                    slot.1 = c.clone();
                } else {
                    users.push((row.n.clone().unwrap(), c.clone()));
                }
            }
        }
    }
    let set = |env: &mut formula::Env, name: &str, v: f64| {
        env.insert(format!("User.{name}"), v);
        env.insert(format!("Sheet.{}!User.{name}", master_root.id), v);
    };
    for (name, c) in &users {
        if let Some(v) = c.numeric().filter(|v| v.is_finite()) {
            set(&mut env, name, v);
        }
    }
    for _ in 0..2 {
        let mut changed = false;
        for (name, c) in &users {
            let Some(f) = c.f.as_deref() else { continue };
            if f == "Inh" || f == "No Formula" {
                continue;
            }
            if let Some(v) = formula::eval(f, &env)
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

/// A sub-shape's evaluation environment base: only QUALIFIED root
/// entries (`Sheet.N!…`) carry over — a bare name in a child formula
/// means the child's own cell, so the root's unqualified Width/Height
/// and User.* must not leak in. The caller then seeds the child's own
/// effective `Width`/`Height` (child sheets are not in the root env;
/// bare child-cell references fail to resolve and fall back).
pub fn qualified_env(root_env: &formula::Env) -> formula::Env {
    root_env
        .iter()
        .filter(|(k, _)| k.contains('!'))
        .map(|(k, v)| (k.clone(), *v))
        .collect()
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

/// Mutable counterpart of [`find_shape_by_id`].
pub fn find_shape_by_id_mut(shapes: &mut [Shape], id: i64) -> Option<&mut Shape> {
    for s in shapes {
        if s.id == id {
            return Some(s);
        }
        if let Some(found) = find_shape_by_id_mut(&mut s.sub_shapes, id) {
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

/// Snap & glue configuration stored in `<DocumentSettings>` (GitHub
/// #109). Bit values follow the Visio automation enums so documents
/// interoperate with the shipped app. Snap: GRID 1, RULER_SUBDIVISIONS
/// 2, GUIDES 4, SELECTION_HANDLES 8, VERTICES 16, CONNECTION_POINTS 32,
/// GEOMETRY 64, ALIGNMENT_BOXES 128, SHAPE_EXTENSIONS 256,
/// SHAPE_INTERSECTIONS 512. Glue: GUIDES 1, SHAPE_HANDLES 2,
/// SHAPE_VERTICES 4, CONNECTION_POINTS 8. The shipped Visio 2016 corpus
/// defaults are SnapSettings 295 (grid, ruler subdivisions, guides,
/// connection points, shape extensions), GlueSettings 9 (guides,
/// connection points), DynamicGridEnabled 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DocSnapGlue {
    pub snap: u32,
    pub glue: u32,
    pub dynamic_grid: bool,
}

impl DocSnapGlue {
    /// The shipped Visio 2016 corpus defaults (see the struct docs).
    pub const CORPUS_DEFAULT: DocSnapGlue = DocSnapGlue {
        snap: 295,
        glue: 9,
        dynamic_grid: true,
    };
}

/// `SnapSettings` bit values (Visio automation `VisSnapSettings` /
/// `VisSnapSettings2` enums).
pub mod snap_bits {
    pub const GRID: u32 = 1;
    pub const RULER_SUBDIVISIONS: u32 = 2;
    pub const GUIDES: u32 = 4;
    pub const SELECTION_HANDLES: u32 = 8;
    pub const VERTICES: u32 = 16;
    pub const CONNECTION_POINTS: u32 = 32;
    pub const GEOMETRY: u32 = 64;
    pub const ALIGNMENT_BOXES: u32 = 128;
    pub const SHAPE_EXTENSIONS: u32 = 256;
    pub const SHAPE_INTERSECTIONS: u32 = 512;
}

/// `GlueSettings` bit values (Visio automation `VisGlueSettings`).
pub mod glue_bits {
    pub const GUIDES: u32 = 1;
    pub const SHAPE_HANDLES: u32 = 2;
    pub const SHAPE_VERTICES: u32 = 4;
    pub const CONNECTION_POINTS: u32 = 8;
}

/// Canonical `<DocumentSettings>` child order (the corpus shape):
/// a missing snap/glue child is inserted before the first child that
/// follows it in this order, so a patched element keeps Visio's layout.
const SETTINGS_CHILD_ORDER: &[&str] = &[
    "GlueSettings",
    "SnapSettings",
    "SnapExtensions",
    "SnapAngles",
    "DynamicGridEnabled",
    "ProtectStyles",
    "ProtectShapes",
    "ProtectMasters",
    "ProtectBkgnds",
];

/// Text of `<Name>value</Name>` inside `inner`, when present.
fn read_settings_child(inner: &str, name: &str) -> Option<u32> {
    let open = format!("<{name}>");
    let p = inner.find(&open)? + open.len();
    let rest = &inner[p..];
    let end = rest.find(&format!("</{name}>"))?;
    rest[..end].trim().parse().ok()
}

/// Replace (or insert, in canonical position) one settings child.
fn set_settings_child(inner: &str, name: &str, value: &str) -> String {
    let full = format!("<{name}>{value}</{name}>");
    // Paired form: swap the inner text, keep everything else verbatim.
    if let Some(p) = inner.find(&format!("<{name}>")) {
        if let Some(end) = inner[p..].find(&format!("</{name}>")) {
            let close = p + end + name.len() + 3;
            let mut out = String::with_capacity(inner.len() + value.len());
            out.push_str(&inner[..p]);
            out.push_str(&full);
            out.push_str(&inner[close..]);
            return out;
        }
    }
    // Self-closing form: expand in place.
    let sc = format!("<{name}/>");
    if let Some(p) = inner.find(&sc) {
        let mut out = String::with_capacity(inner.len() + value.len());
        out.push_str(&inner[..p]);
        out.push_str(&full);
        out.push_str(&inner[p + sc.len()..]);
        return out;
    }
    // Absent: insert before the first canonically-later child, else
    // append at the end.
    let later = &SETTINGS_CHILD_ORDER[SETTINGS_CHILD_ORDER
        .iter()
        .position(|n| *n == name)
        .map_or(0, |i| i + 1)..];
    for cand in later {
        for form in [format!("<{cand}>"), format!("<{cand}/>")] {
            if let Some(p) = inner.find(&form) {
                let mut out = String::with_capacity(inner.len() + full.len());
                out.push_str(&inner[..p]);
                out.push_str(&full);
                out.push_str(&inner[p..]);
                return out;
            }
        }
    }
    format!("{inner}{full}")
}

impl Document {
    /// The document's snap & glue settings, straight from a
    /// `<DocumentSettings>` element. `None` when the document carries
    /// no such element (a fresh blank package); a present element with
    /// an absent child reads that child as its shipped default.
    pub fn doc_snap_glue(&self) -> Option<DocSnapGlue> {
        let raw = self.items.iter().find_map(|it| match it {
            DocItem::Raw(r) if r.name == "DocumentSettings" => Some(r),
            _ => None,
        })?;
        let d = DocSnapGlue::CORPUS_DEFAULT;
        Some(DocSnapGlue {
            snap: read_settings_child(&raw.inner, "SnapSettings").unwrap_or(d.snap),
            glue: read_settings_child(&raw.inner, "GlueSettings").unwrap_or(d.glue),
            dynamic_grid: read_settings_child(&raw.inner, "DynamicGridEnabled")
                .map(|v| v != 0)
                .unwrap_or(d.dynamic_grid),
        })
    }

    /// Write the snap & glue cells into `<DocumentSettings>`, creating
    /// the element (first child of `VisioDocument`, corpus order) when
    /// the document has none. Only the three modeled children are
    /// touched; attributes and every other child — `SnapExtensions`,
    /// `SnapAngles`, `Protect*` — stay verbatim.
    pub fn set_doc_snap_glue(&mut self, s: DocSnapGlue) {
        let dg = if s.dynamic_grid { "1" } else { "0" };
        if let Some(ix) = self.items.iter().position(|it| {
            matches!(it, DocItem::Raw(r) if r.name == "DocumentSettings")
        }) {
            if let DocItem::Raw(r) = &mut self.items[ix] {
                r.inner = set_settings_child(&r.inner, "GlueSettings", &s.glue.to_string());
                r.inner = set_settings_child(&r.inner, "SnapSettings", &s.snap.to_string());
                r.inner = set_settings_child(&r.inner, "DynamicGridEnabled", dg);
            }
            return;
        }
        // No element yet (the blank package): synthesize one with the
        // corpus child set — SnapExtensions 34 and an empty SnapAngles
        // are the shipped defaults, written once and then verbatim.
        let mut inner = String::new();
        inner = set_settings_child(&inner, "GlueSettings", &s.glue.to_string());
        inner = set_settings_child(&inner, "SnapSettings", &s.snap.to_string());
        inner = set_settings_child(&inner, "SnapExtensions", "34");
        inner.push_str("<SnapAngles/>");
        inner = set_settings_child(&inner, "DynamicGridEnabled", dg);
        self.items.insert(
            0,
            DocItem::Raw(RawEl {
                name: "DocumentSettings".to_string(),
                attrs: Vec::new(),
                inner,
            }),
        );
    }

    /// Whether hyperlink addresses are stored relative to the
    /// document (GitHub #71, the 2016 Hyperlinks dialog's "Use
    /// relative paths for hyperlinks" checkbox). Reconstructed as a
    /// `<UseRelativePaths>` child of `<DocumentSettings>` — the
    /// shipped corpus never carries the element (Visio 2016 keeps
    /// the toggle application-level), so absence reads as OFF and
    /// this stays a per-document DiagramCraft setting.
    pub fn doc_hyperlinks_relative(&self) -> bool {
        let Some(raw) = self.items.iter().find_map(|it| match it {
            DocItem::Raw(r) if r.name == "DocumentSettings" => Some(r),
            _ => None,
        }) else {
            return false;
        };
        read_settings_child(&raw.inner, "UseRelativePaths")
            .map(|v| v != 0)
            .unwrap_or(false)
    }

    /// Write the relative-hyperlinks toggle (see
    /// [`Document::doc_hyperlinks_relative`]). Document state, not an
    /// edit: callers write it like the snap & glue cells (no undo,
    /// the file saves it back).
    pub fn set_doc_hyperlinks_relative(&mut self, on: bool) {
        let v = if on { "1" } else { "0" };
        if let Some(ix) =
            self.items
                .iter()
                .position(|it| matches!(it, DocItem::Raw(r) if r.name == "DocumentSettings"))
        {
            if let DocItem::Raw(r) = &mut self.items[ix] {
                r.inner = set_settings_child(&r.inner, "UseRelativePaths", v);
            }
            return;
        }
        // No element yet (the blank package): synthesize one carrying
        // the toggle (the snap & glue synthesis shape, minus its
        // defaults — those stay untouched here).
        let inner = set_settings_child("", "UseRelativePaths", v);
        self.items.insert(
            0,
            DocItem::Raw(RawEl {
                name: "DocumentSettings".to_string(),
                attrs: Vec::new(),
                inner,
            }),
        );
    }
}

#[cfg(test)]
mod snap_glue_tests {
    use super::*;

    /// A minimal document carrying one verbatim `DocumentSettings`
    /// element (exactly what the parser produces for the corpus shape
    /// — the RawEl round-trip itself is covered by the corpus tests).
    fn doc_with_settings(attrs: Attrs, inner: &str) -> Document {
        let mut doc = Document::default();
        doc.items.push(DocItem::Raw(RawEl {
            name: "DocumentSettings".to_string(),
            attrs,
            inner: inner.to_string(),
        }));
        doc.items.push(DocItem::DocumentSheet);
        doc
    }

    /// The shipped corpus element (attrs + children in Visio order).
    const CORPUS_INNER: &str = "<GlueSettings>9</GlueSettings><SnapSettings>295</SnapSettings>\
<SnapExtensions>34</SnapExtensions><SnapAngles/>\
<DynamicGridEnabled>1</DynamicGridEnabled><ProtectStyles>0</ProtectStyles>";

    #[test]
    fn reads_corpus_defaults() {
        let doc = doc_with_settings(
            vec![
                ("TopPage".to_string(), "0".to_string()),
                ("DefaultTextStyle".to_string(), "3".to_string()),
            ],
            CORPUS_INNER,
        );
        let s = doc.doc_snap_glue().expect("DocumentSettings present");
        assert_eq!(s.snap, 295);
        assert_eq!(s.glue, 9);
        assert!(s.dynamic_grid);
    }

    #[test]
    fn missing_element_reads_none() {
        let doc = Document::default();
        assert!(doc.doc_snap_glue().is_none());
    }

    #[test]
    fn setting_patches_only_the_three_children() {
        let mut doc = doc_with_settings(
            vec![("TopPage".to_string(), "0".to_string())],
            CORPUS_INNER,
        );
        let before = doc
            .items
            .iter()
            .find_map(|it| match it {
                DocItem::Raw(r) if r.name == "DocumentSettings" => Some(r.clone()),
                _ => None,
            })
            .unwrap();
        doc.set_doc_snap_glue(DocSnapGlue {
            snap: 65847,
            glue: 0,
            dynamic_grid: false,
        });
        let after = doc
            .items
            .iter()
            .find_map(|it| match it {
                DocItem::Raw(r) if r.name == "DocumentSettings" => Some(r.clone()),
                _ => None,
            })
            .unwrap();
        // Attributes and every unmodeled child stay verbatim.
        assert_eq!(before.attrs, after.attrs);
        assert!(after.inner.contains("<SnapExtensions>34</SnapExtensions>"));
        assert!(after.inner.contains("<SnapAngles/>"));
        assert!(after.inner.contains("<ProtectStyles>0</ProtectStyles>"));
        // The three modeled children carry the new values, in corpus
        // child order.
        let g = after.inner.find("<GlueSettings>").unwrap();
        let s = after.inner.find("<SnapSettings>").unwrap();
        let d = after.inner.find("<DynamicGridEnabled>").unwrap();
        assert!(g < s && s < d, "corpus child order: {}", after.inner);
        assert!(after.inner.contains("<GlueSettings>0</GlueSettings>"));
        assert!(after.inner.contains("<SnapSettings>65847</SnapSettings>"));
        assert!(after.inner.contains("<DynamicGridEnabled>0</DynamicGridEnabled>"));
        // And reading back yields exactly what was written.
        let read = doc.doc_snap_glue().unwrap();
        assert_eq!(read.snap, 65847);
        assert_eq!(read.glue, 0);
        assert!(!read.dynamic_grid);
    }

    #[test]
    fn setting_creates_a_missing_element_in_corpus_shape() {
        let mut doc = Document::default();
        doc.items.push(DocItem::DocumentSheet);
        doc.set_doc_snap_glue(DocSnapGlue::CORPUS_DEFAULT);
        // First child of VisioDocument, like the corpus.
        assert!(matches!(doc.items.first(), Some(DocItem::Raw(r)) if r.name == "DocumentSettings"));
        let inner = match doc.items.first() {
            Some(DocItem::Raw(r)) => r.inner.clone(),
            _ => panic!("DocumentSettings element"),
        };
        assert_eq!(
            inner,
            "<GlueSettings>9</GlueSettings><SnapSettings>295</SnapSettings>\
             <SnapExtensions>34</SnapExtensions><SnapAngles/>\
             <DynamicGridEnabled>1</DynamicGridEnabled>"
        );
        assert_eq!(doc.doc_snap_glue(), Some(DocSnapGlue::CORPUS_DEFAULT));
    }

    #[test]
    fn patching_inserts_missing_children_in_canonical_position() {
        // An element with only a later-order child: the missing
        // GlueSettings/SnapSettings must land before SnapExtensions,
        // and DynamicGridEnabled after it.
        let mut inner = String::from("<SnapExtensions>34</SnapExtensions><ProtectStyles>0</ProtectStyles>");
        inner = set_settings_child(&inner, "GlueSettings", "1");
        inner = set_settings_child(&inner, "SnapSettings", "2");
        inner = set_settings_child(&inner, "DynamicGridEnabled", "0");
        assert_eq!(
            inner,
            "<GlueSettings>1</GlueSettings><SnapSettings>2</SnapSettings>\
             <SnapExtensions>34</SnapExtensions><DynamicGridEnabled>0</DynamicGridEnabled>\
             <ProtectStyles>0</ProtectStyles>"
        );
    }

    #[test]
    fn patching_expands_a_self_closing_child() {
        let inner = set_settings_child("<SnapSettings/>x", "SnapSettings", "295");
        assert_eq!(inner, "<SnapSettings>295</SnapSettings>x");
    }
}
