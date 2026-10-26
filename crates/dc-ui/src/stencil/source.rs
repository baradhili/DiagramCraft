//! The open-stencil set (GitHub #63): the session state behind the
//! Shapes pane. Everything stencil-management that would otherwise
//! grow `app.rs` lives here (owner direction, 2026-10-26) — opening,
//! deduplicating, closing, and enumerating the stencils the pane
//! lists alongside the document's own masters.
//!
//! Open stencils are **app state, never file content** (observed
//! Visio 2016: shipped stencils persist an empty `visio/windows.xml`;
//! REA 2026-10-25). Nothing here touches the `.vsdx` write path.

use std::path::{Path, PathBuf};

/// Which master source a key addresses: the edited document's own
/// masters, or one open stencil (by pane position). Master ids are
/// per-document integers and collide across sources — a bare `i64`
/// is never a valid identity past this module (#82/#85).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum StencilId {
    /// The edited document's own masters — always the first entry.
    Document,
    /// Open stencil at pane position `usize` in [`StencilSet::open`].
    Open(usize),
}

/// A master addressed by (source, master id) — the pane's arming,
/// carry, and hit-test identity.
pub type MasterKey = (StencilId, i64);

/// How an open stencil came to be open (drives menu checkmarks and
/// re-open behavior, #87).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Origin {
    /// Opened from a `.vssx` file on disk.
    File,
    /// Opened from the built-in catalog (#87); `source` carries the
    /// catalog id as a `builtin/…` pseudo-path.
    Builtin,
}

/// One open stencil in the pane: its parsed package plus session
/// state. The `Document` is immutable for the session — caches keyed
/// by `serial` may hold art from it indefinitely.
pub struct OpenStencil {
    /// The parsed stencil package (masters; pages typically empty).
    pub doc: vsdx::Document,
    /// File it was opened from; builtins use a `builtin/<id>` pseudo-
    /// path. The dedupe key for re-opening.
    pub source: PathBuf,
    /// Entry title (file stem, or the catalog title for builtins).
    pub title: String,
    /// The entry's own expand/collapse flag, independent of every
    /// other entry's (observed Visio 2016).
    pub expanded: bool,
    pub origin: Origin,
    /// Session-unique serial — never reused, even after close, so
    /// thumbnail/raster keys cannot collide across stencils (#85).
    serial: u64,
}

impl OpenStencil {
    /// Stable key-half for thumbnail caches: session-unique per open
    /// stencil (the document entry uses `DOC_SERIAL`).
    pub fn serial(&self) -> u64 {
        self.serial
    }
}

/// Serial reserved for the document-masters entry — real stencils
/// number from 1.
pub const DOC_SERIAL: u64 = 0;

/// The set of open stencils (pane order). Owned by the app; the pane
/// renders it and toggles per-entry expansion.
#[derive(Default)]
pub struct StencilSet {
    open: Vec<OpenStencil>,
    next_serial: u64,
}

impl StencilSet {
    pub fn is_empty(&self) -> bool {
        self.open.is_empty()
    }
    pub fn len(&self) -> usize {
        self.open.len()
    }
    pub fn iter(&self) -> impl Iterator<Item = (usize, &OpenStencil)> {
        self.open.iter().enumerate()
    }
    pub fn get(&self, ix: usize) -> Option<&OpenStencil> {
        self.open.get(ix)
    }
    /// Toggle one entry's expansion (pane click, #82).
    pub fn set_expanded(&mut self, ix: usize, expanded: bool) {
        if let Some(st) = self.open.get_mut(ix) {
            st.expanded = expanded;
        }
    }

    /// Open (or focus) a stencil parsed from `path`. Re-opening an
    /// already-open source expands the existing entry instead of
    /// duplicating it (observed Visio 2016, #83); returns the entry
    /// index and whether it already existed.
    pub fn open_file(&mut self, path: &Path) -> anyhow::Result<(usize, bool)> {
        if let Some(ix) = self.position_for(path) {
            self.open[ix].expanded = true;
            return Ok((ix, true));
        }
        let doc = vsdx::parse::parse(path)?;
        let title = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Stencil".into());
        let ix = self.push(doc, path.to_path_buf(), title, Origin::File);
        Ok((ix, false))
    }

    /// Open an already-parsed stencil (built-in catalog, #87; tests).
    /// Dedupe by `source` applies exactly like file opens.
    pub fn open_parsed(
        &mut self,
        doc: vsdx::Document,
        source: PathBuf,
        title: String,
        origin: Origin,
    ) -> (usize, bool) {
        if let Some(ix) = self.position_for(&source) {
            self.open[ix].expanded = true;
            return (ix, true);
        }
        let ix = self.push(doc, source, title, origin);
        (ix, false)
    }

    /// Pane position of the entry opened from `source`, if any.
    /// Paths are compared after best-effort canonicalization so a
    /// relative and an absolute pick of the same file match.
    fn position_for(&self, source: &Path) -> Option<usize> {
        let canon = |p: &Path| p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
        let want = canon(source);
        self.open
            .iter()
            .position(|s| canon(&s.source) == want)
    }

    fn push(
        &mut self,
        doc: vsdx::Document,
        source: PathBuf,
        title: String,
        origin: Origin,
    ) -> usize {
        self.next_serial += 1;
        let serial = self.next_serial;
        self.open.push(OpenStencil {
            doc,
            source,
            title,
            expanded: true,
            origin,
            serial,
        });
        self.open.len() - 1
    }

    /// Close the entry at `ix`; later entries shift down (their pane
    /// positions change — anything cached by position must re-derive;
    /// serial-keyed caches stay valid for the survivors). Returns the
    /// closed entry's serial so texture caches can evict it (#85).
    pub fn close(&mut self, ix: usize) -> Option<u64> {
        (ix < self.open.len()).then(|| self.open.remove(ix).serial())
    }

    /// The master a key addresses, from its own source document —
    /// the single resolver for pane placement/thumbnails (#82:
    /// stencil master identity is (stencil, master) from day one).
    pub fn resolve<'a>(
        &'a self,
        ed: &'a dc_core::Editor,
        key: MasterKey,
    ) -> Option<(u64, &'a vsdx::Document, &'a vsdx::Master)> {
        match key.0 {
            StencilId::Document => {
                let m = ed.doc.master(key.1)?;
                Some((DOC_SERIAL, &ed.doc, m))
            }
            StencilId::Open(ix) => {
                let st = self.get(ix)?;
                let m = st.doc.master(key.1)?;
                Some((st.serial(), &st.doc, m))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stencil_doc(name: &str, id: i64) -> vsdx::Document {
        vsdx::Document {
            masters: vec![vsdx::Master {
                id,
                name_u: Some(name.into()),
                ..vsdx::Master::default()
            }],
            ..vsdx::Document::default()
        }
    }

    #[test]
    fn reopen_dedupes_by_path_and_expands() {
        let mut set = StencilSet::default();
        let (a, dup) = set.open_parsed(
            stencil_doc("S", 1),
            PathBuf::from("/tmp/synth.vssx"),
            "synth".into(),
            Origin::File,
        );
        assert_eq!(a, 0);
        assert!(!dup);
        set.open[0].expanded = false;
        let (b, dup) = set.open_parsed(
            stencil_doc("S", 1),
            PathBuf::from("/tmp/synth.vssx"),
            "synth".into(),
            Origin::File,
        );
        assert_eq!(b, 0);
        assert!(dup, "same source re-open focuses the entry");
        assert!(set.open[0].expanded, "re-open expands the entry");
        assert_eq!(set.len(), 1);
    }

    #[test]
    fn serials_never_repeat_across_close_and_reopen() {
        let mut set = StencilSet::default();
        set.open_parsed(stencil_doc("A", 1), PathBuf::from("a.vssx"), "A".into(), Origin::File);
        let s1 = set.open[0].serial();
        assert_eq!(set.close(0), Some(s1));
        set.open_parsed(stencil_doc("A", 2), PathBuf::from("a.vssx"), "A".into(), Origin::File);
        assert_ne!(
            set.open[0].serial(),
            s1,
            "a reopened stencil must not reuse the closed one's cache serial"
        );
    }
}
