//! Typed connections (glue) — promotion of the verbatim `<Connects>`
//! element from raw to typed, editable, and back.
//!
//! Connections live in the contents part of pages *and* masters
//! (`<Connects><Connect FromSheet='60' FromCell='EndX' FromPart='12'
//! ToSheet='51' ToCell='Connections.X1' ToPart='100'/></Connects>`).
//! `FromPart`/`ToPart` codes: 0 none, 1/2 begin/end of a 1-D shape,
//! 3..=6/9/12 begin/end x/y variants, 7/8 edges, 10/11 controls,
//! 100+ connection-point rows (Analysis/04).

use anyhow::Result;
use vsdx::model::{Attrs, Document, RawEl};

use quick_xml::events::Event;

/// One `<Connect>` record.
#[derive(Debug, Clone, PartialEq)]
pub struct Connect {
    pub from_sheet: i64,
    pub from_cell: String,
    pub from_part: i64,
    pub to_sheet: i64,
    pub to_cell: String,
    pub to_part: i64,
    /// Any attributes beyond the six canonical ones, verbatim.
    pub extra: Attrs,
}

impl Connect {
    pub fn new(
        from_sheet: i64,
        from_cell: impl Into<String>,
        from_part: i64,
        to_sheet: i64,
        to_cell: impl Into<String>,
        to_part: i64,
    ) -> Connect {
        Connect {
            from_sheet,
            from_cell: from_cell.into(),
            from_part,
            to_sheet,
            to_cell: to_cell.into(),
            to_part,
            extra: Vec::new(),
        }
    }

    fn attr_pairs(&self) -> Attrs {
        let mut a: Attrs = vec![
            ("FromSheet".into(), self.from_sheet.to_string()),
            ("FromCell".into(), self.from_cell.clone()),
            ("FromPart".into(), self.from_part.to_string()),
            ("ToSheet".into(), self.to_sheet.to_string()),
            ("ToCell".into(), self.to_cell.clone()),
            ("ToPart".into(), self.to_part.to_string()),
        ];
        for (k, v) in &self.extra {
            if !a.iter().any(|(ek, _)| ek == k) {
                a.push((k.clone(), v.clone()));
            }
        }
        a
    }
}

/// The parsed `<Connects>` element of one contents part.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Connects {
    /// Attributes of the `<Connects>` element itself (rare, preserved).
    pub attrs: Attrs,
    pub list: Vec<Connect>,
}

fn esc_attr(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

impl Connects {
    /// Parse the `<Connects>` element of a raw-children list, if present.
    pub fn from_raw(raws: &[RawEl]) -> Option<Connects> {
        let el = raws.iter().find(|r| r.name == "Connects")?;
        Some(Connects::parse_inner(&el.attrs, &el.inner))
    }

    pub fn parse_inner(attrs: &Attrs, inner: &str) -> Connects {
        let mut out = Connects { attrs: attrs.to_vec(), list: Vec::new() };
        let mut r = quick_xml::Reader::from_str(inner);
        let mut buf = Vec::new();
        loop {
            buf.clear();
            match r.read_event_into(&mut buf) {
                Ok(Event::Start(e)) | Ok(Event::Empty(e)) if e.name().as_ref() == "Connect" => {
                    let mut pairs: Attrs = e
                        .attributes()
                        .flatten()
                        .map(|a| {
                            (
                                a.key.as_ref().to_string(),
                                a.normalized_value(quick_xml::XmlVersion::Implicit1_0).unwrap_or_default().into_owned(),
                            )
                        })
                        .collect();
                    let get = |pairs: &Attrs, k: &str, d: i64| {
                        pairs
                            .iter()
                            .find(|(pk, _)| pk == k)
                            .and_then(|(_, v)| v.parse::<i64>().ok())
                            .unwrap_or(d)
                    };
                    let gets = |pairs: &Attrs, k: &str| {
                        pairs
                            .iter()
                            .find(|(pk, _)| pk == k)
                            .map(|(_, v)| v.clone())
                            .unwrap_or_default()
                    };
                    let connect = Connect {
                        from_sheet: get(&pairs, "FromSheet", 0),
                        from_cell: gets(&pairs, "FromCell"),
                        from_part: get(&pairs, "FromPart", 0),
                        to_sheet: get(&pairs, "ToSheet", 0),
                        to_cell: gets(&pairs, "ToCell"),
                        to_part: get(&pairs, "ToPart", 0),
                        extra: {
                            pairs.retain(|(k, _)| {
                                !matches!(
                                    k.as_str(),
                                    "FromSheet" | "FromCell" | "FromPart" | "ToSheet" | "ToCell" | "ToPart"
                                )
                            });
                            pairs
                        },
                    };
                    out.list.push(connect);
                }
                Ok(Event::Eof) | Err(_) => break,
                _ => {}
            }
        }
        out
    }

    /// Store into a raw-children list: replaces the existing `<Connects>`
    /// element in place, or appends one (after the shapes, where the
    /// corpus keeps it). A `Connects` with no entries is removed entirely.
    pub fn store_into(&self, raws: &mut Vec<RawEl>) {
        if self.list.is_empty() {
            raws.retain(|r| r.name != "Connects");
            return;
        }
        // No-op guard: when the typed list still matches what the raw
        // element parses to, leave the original bytes untouched (raw
        // preservation — an add + undo sequence must be invisible).
        if let Some(el) = raws.iter().find(|r| r.name == "Connects") {
            if &Connects::parse_inner(&el.attrs, &el.inner) == self {
                return;
            }
        }
        let mut inner = String::new();
        for c in &self.list {
            inner.push_str("<Connect");
            for (k, v) in c.attr_pairs() {
                inner.push_str(&format!(" {k}=\"{}\"", esc_attr(&v)));
            }
            inner.push_str("/>");
        }
        match raws.iter_mut().find(|r| r.name == "Connects") {
            Some(el) => {
                el.attrs = self.attrs.clone();
                el.inner = inner;
            }
            None => raws.push(RawEl {
                name: "Connects".to_string(),
                attrs: self.attrs.clone(),
                inner,
            }),
        }
    }
}

/// The owner's contents part name ("" for partless stencil pages).
fn part_of(doc: &Document, owner: &ConnectsOwner) -> String {
    match owner {
        ConnectsOwner::Page(id) => doc
            .pages
            .iter()
            .find(|p| p.id == *id)
            .map(|p| p.part.clone())
            .unwrap_or_default(),
        ConnectsOwner::Master(id) => doc
            .masters
            .iter()
            .find(|m| m.id == *id)
            .map(|m| m.part.clone())
            .unwrap_or_default(),
    }
}

/// Connection edits need a contents part to be persisted; partless
/// stencil pages have none, so their `<Connects>` would be dropped on
/// save — reject at command time instead of losing the edit.
fn ensure_editable(doc: &Document, owner: &ConnectsOwner) -> Result<()> {
    let part = part_of(doc, owner);
    if part.is_empty() {
        bail_edit(owner)?;
    }
    Ok(())
}

fn bail_edit(owner: &ConnectsOwner) -> Result<()> {
    Err(anyhow::anyhow!(
        "{owner} has no contents part (partless stencil page); connection edits would be lost on save"
    ))
}

/// Anything that owns a contents raw-children list — pages and masters.
pub trait ConnectsHost {
    fn raw_list(&self) -> &[RawEl];
    fn raw_list_mut(&mut self) -> &mut Vec<RawEl>;

    fn connects(&self) -> Connects {
        Connects::from_raw(self.raw_list()).unwrap_or_default()
    }
    fn set_connects(&mut self, connects: &Connects) {
        connects.store_into(self.raw_list_mut());
    }
}

impl ConnectsHost for vsdx::Page {
    fn raw_list(&self) -> &[RawEl] {
        &self.contents_raw
    }
    fn raw_list_mut(&mut self) -> &mut Vec<RawEl> {
        &mut self.contents_raw
    }
}

impl ConnectsHost for vsdx::Master {
    fn raw_list(&self) -> &[RawEl] {
        &self.contents_raw
    }
    fn raw_list_mut(&mut self) -> &mut Vec<RawEl> {
        &mut self.contents_raw
    }
}

/// Where a `<Connects>` edit applies.
#[derive(Debug, Clone, PartialEq)]
pub enum ConnectsOwner {
    Page(i64),
    Master(i64),
}

impl std::fmt::Display for ConnectsOwner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConnectsOwner::Page(id) => write!(f, "page:{id}"),
            ConnectsOwner::Master(id) => write!(f, "master:{id}"),
        }
    }
}

impl ConnectsOwner {
    /// Read a copy of the owner's connections.
    pub fn read_connects(&self, doc: &Document) -> Result<Connects> {
        match self {
            ConnectsOwner::Page(id) => {
                let p = doc
                    .pages
                    .iter()
                    .find(|p| p.id == *id)
                    .ok_or_else(|| anyhow::anyhow!("page {id} not found"))?;
                Ok(p.connects())
            }
            ConnectsOwner::Master(id) => {
                let m = doc
                    .masters
                    .iter()
                    .find(|m| m.id == *id)
                    .ok_or_else(|| anyhow::anyhow!("master {id} not found"))?;
                Ok(m.connects())
            }
        }
    }

    /// Like [`read_connects`] but also rejects partless owners, whose
    /// connections could not be written back out.
    pub fn read_connects_editable(&self, doc: &Document) -> Result<Connects> {
        ensure_editable(doc, self)?;
        self.read_connects(doc)
    }

    /// Mutably borrow the owner as a connections host.
    pub fn host<'a>(&self, doc: &'a mut Document) -> Result<&'a mut dyn ConnectsHost> {
        match self {
            ConnectsOwner::Page(id) => {
                let i = doc
                    .pages
                    .iter()
                    .position(|p| p.id == *id)
                    .ok_or_else(|| anyhow::anyhow!("page {id} not found"))?;
                Ok(&mut doc.pages[i])
            }
            ConnectsOwner::Master(id) => {
                let i = doc
                    .masters
                    .iter()
                    .position(|m| m.id == *id)
                    .ok_or_else(|| anyhow::anyhow!("master {id} not found"))?;
                Ok(&mut doc.masters[i])
            }
        }
    }
}
