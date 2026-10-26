//! dc-core — the application core (M4): an editing layer over the
//! [`vsdx`] document model.
//!
//! The central pieces are:
//! - [`Command`] — a reversible model mutation (the `IVBUndoManager`
//!   analog's unit of work). Commands only touch what they edit; the
//!   v0.2 raw-preservation guarantees hold because untouched model
//!   state serializes exactly as before.
//! - [`History`] — undo/redo stacks of applied commands.
//! - [`Editor`] — a document plus its history: open, apply, undo/redo,
//!   save.
//!
//! Addressing uses [`Scope`] strings (`doc`, `page:1`, `page:1/shape:5`,
//! `style:2`, `master:7/shape:2`) — the ShapeSheet inspector surface over
//! sheets, sections, rows, and cells.

pub mod commands;
pub mod connects;
pub mod layers;
pub mod scope;

pub use commands::{
    AddConnect, AddLayer, AddPage, AddShape, DuplicateShape, Macro, MovePage, RemoveConnect,
    RemoveLayer, RemovePage, RemoveShape, RenameLayer, RenamePage, ReorderShape, SetCell,
    SetRowCell, SetText,
};
pub use connects::{Connect, Connects, ConnectsOwner};
pub use scope::{find_shape_mut, Scope};

use anyhow::Result;
use std::path::Path;
use vsdx::Document;

/// How one command's edit bears on a canvas layer (#93 invalidation
/// plumbing). The GUI's per-shape layer store maps these onto texture
/// keys; coarser consumers can treat everything as "page dirty".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Touch {
    /// Ink within the shape's bounds may have changed (fill, text,
    /// geometry, a sub-shape edit) — re-render its layer texture.
    Ink,
    /// Pure translation of the shape (`PinX`/`PinY` on a top-level
    /// shape): the texture stays valid — the ink is identical relative
    /// to its bounds — only the quad's placement moves.
    Move,
    /// The shape entered or left the page (add/remove/duplicate) —
    /// layer-set membership, not texture content.
    Membership,
    /// Z-order only. Paint order is read live from the model each
    /// frame; nothing cached depends on it.
    Order,
}

/// One (page, top-level shape) a command touched, and how. Sub-shape
/// edits roll up to their top-level ancestor — that ancestor's subtree
/// is the rendering unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Touched {
    pub page: i64,
    pub shape: i64,
    pub touch: Touch,
}

/// What a command invalidated: per-shape touches plus any fully-dirty
/// pages (the conservative fallback for commands whose effect is not
/// worth pinning down — structure changes, layer and page-set edits).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TouchList {
    pub pages: Vec<i64>,
    pub shapes: Vec<Touched>,
}

impl TouchList {
    /// Every page fully dirty — the conservative default.
    pub fn all_pages(doc: &Document) -> TouchList {
        TouchList {
            pages: doc.pages.iter().map(|p| p.id).collect(),
            shapes: Vec::new(),
        }
    }

    pub fn page(page: i64) -> TouchList {
        TouchList {
            pages: vec![page],
            shapes: Vec::new(),
        }
    }

    pub fn none() -> TouchList {
        TouchList::default()
    }

    /// Union (used by composites). Per shape, the strongest touch wins:
    /// Ink > Membership > Move > Order (an ink re-render subsumes the
    /// others; membership and move are handled alongside it anyway).
    pub fn merge(&mut self, other: TouchList) {
        self.pages.extend(other.pages);
        self.pages.sort();
        self.pages.dedup();
        for t in other.shapes {
            if let Some(have) = self.shapes.iter_mut().find(|h| h.page == t.page && h.shape == t.shape) {
                if t.touch.rank() > have.touch.rank() {
                    have.touch = t.touch;
                }
            } else {
                self.shapes.push(t);
            }
        }
    }
}

impl Touch {
    fn rank(self) -> u8 {
        match self {
            Touch::Ink => 3,
            Touch::Membership => 2,
            Touch::Move => 1,
            Touch::Order => 0,
        }
    }
}

/// A reversible model mutation. `apply` is idempotent from the command's
/// captured pre-state; `undo` restores exactly that state.
pub trait Command {
    /// Short human-readable description for history listings.
    fn label(&self) -> String;
    /// Execute the mutation, capturing whatever `undo` needs.
    fn apply(&mut self, doc: &mut Document) -> Result<()>;
    /// Revert the mutation.
    fn undo(&mut self, doc: &mut Document) -> Result<()>;
    /// What this command invalidated for canvas layer caches (#93),
    /// queried AFTER `apply`/`undo` ran (some commands capture created
    /// ids then). The conservative default dirties every page — correct
    /// for any command, refined per type where it pays.
    fn touched(&self, doc: &Document) -> TouchList {
        TouchList::all_pages(doc)
    }
}

/// Undo/redo stacks (the `IVBUndoManager` analog).
#[derive(Default)]
pub struct History {
    undo_stack: Vec<Box<dyn Command>>,
    redo_stack: Vec<Box<dyn Command>>,
    limit: usize,
}

impl History {
    pub fn new(limit: usize) -> History {
        History {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            limit,
        }
    }

    /// Execute `cmd` against `doc`, push it on the undo stack, and clear
    /// the redo stack. Returns the command's label and its canvas
    /// invalidation (`Command::touched`, #93).
    pub fn apply(&mut self, mut cmd: Box<dyn Command>, doc: &mut Document) -> Result<(String, TouchList)> {
        cmd.apply(doc)?;
        let label = cmd.label();
        let touched = cmd.touched(doc);
        self.undo_stack.push(cmd);
        if self.limit > 0 && self.undo_stack.len() > self.limit {
            self.undo_stack.remove(0);
        }
        self.redo_stack.clear();
        Ok((label, touched))
    }

    /// Undo the most recent command; returns its label and invalidation,
    /// or `None` when there is nothing to undo.
    pub fn undo(&mut self, doc: &mut Document) -> Result<Option<(String, TouchList)>> {
        let mut cmd = match self.undo_stack.pop() {
            Some(c) => c,
            None => return Ok(None),
        };
        cmd.undo(doc)?;
        let label = cmd.label();
        let touched = cmd.touched(doc);
        self.redo_stack.push(cmd);
        Ok(Some((label, touched)))
    }

    /// Redo the most recently undone command; returns its label and
    /// invalidation, or `None` when there is nothing to redo.
    pub fn redo(&mut self, doc: &mut Document) -> Result<Option<(String, TouchList)>> {
        let mut cmd = match self.redo_stack.pop() {
            Some(c) => c,
            None => return Ok(None),
        };
        cmd.apply(doc)?;
        let label = cmd.label();
        let touched = cmd.touched(doc);
        self.undo_stack.push(cmd);
        Ok(Some((label, touched)))
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }
    /// Labels of applied commands, oldest first.
    pub fn undo_labels(&self) -> Vec<String> {
        self.undo_stack.iter().map(|c| c.label()).collect()
    }
    /// Labels of undone commands, most recent first.
    pub fn redo_labels(&self) -> Vec<String> {
        self.redo_stack.iter().map(|c| c.label()).collect()
    }

    /// Change the stack limit; the oldest undo entries are dropped when
    /// the new limit is smaller.
    pub fn set_limit(&mut self, limit: usize) {
        self.limit = limit;
        if limit > 0 && self.undo_stack.len() > limit {
            let drop = self.undo_stack.len() - limit;
            self.undo_stack.drain(0..drop);
        }
    }
}

/// A document plus its edit history.
pub struct Editor {
    pub doc: Document,
    pub history: History,
}

impl Editor {
    pub fn new(doc: Document) -> Editor {
        Editor {
            doc,
            history: History::new(100),
        }
    }

    /// Parse a package from disk.
    pub fn open(path: &Path) -> Result<Editor> {
        Ok(Editor::new(vsdx::parse::parse(path)?))
    }

    /// A fresh blank drawing: one US-Letter page, no masters (File →
    /// New). The embedded template is DiagramCraft's own synthetic
    /// package (no Microsoft content).
    pub fn new_blank() -> Editor {
        const BLANK: &[u8] = include_bytes!("../assets/blank.vsdx");
        Editor::new(vsdx::parse::parse_bytes(BLANK).expect("embedded blank template is valid"))
    }

    /// Apply a command through the history.
    pub fn apply<C: Command + 'static>(&mut self, cmd: C) -> Result<(String, TouchList)> {
        self.history.apply(Box::new(cmd), &mut self.doc)
    }

    pub fn undo(&mut self) -> Result<Option<(String, TouchList)>> {
        self.history.undo(&mut self.doc)
    }

    pub fn redo(&mut self) -> Result<Option<(String, TouchList)>> {
        self.history.redo(&mut self.doc)
    }

    /// Save the document to `path` (lossless write path). The write is
    /// crash-safe: craft-atomic stages the package in the destination
    /// directory, fsyncs it, and renames it over the target, so a
    /// crash mid-save never truncates an existing drawing.
    pub fn save_as(&self, path: &Path) -> Result<()> {
        craft_atomic::atomic_write_with(path, |file| {
            vsdx::write::save_with(&self.doc, file)
                .map_err(|e| std::io::Error::other(format!("{e:#}")))
        })
        .map_err(|e| anyhow::anyhow!("atomic save to {}: {e}", path.display()))
    }
}
