//! The desktop application: shell assembly (QAT band, ribbon, stencil
//! pane, canvas, page tabs, status bar) and all interaction wiring over
//! the dc-core command layer.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use dc_core::commands::{AddPage, AddShape, DuplicateShape, RemoveShape, ReorderShape};
use dc_core::scope::{shape_chain, sheet as scope_sheet, Scope};
use dc_core::{Editor, Macro, SetCell, SetRowCell, SetText};
use egui::{Color32, Context, Pos2, Rect, Sense, Stroke, Ui, Vec2};
use vsdx::model::{Document, Shape};

use crate::canvas::{self, PageGeom, View};
use crate::cmd::{AddMasterInstance, MoveShape};
use crate::recent::RecentFiles;
use crate::ribbon::{big, grid, stack, Action, Icon, RibbonButton, Tab, TABS};
use crate::stencil::Stencil;
use crate::svgimg::TextureCache;
use crate::theme;

pub struct App {
    ed: Option<Editor>,
    path: PathBuf,
    dirty: bool,
    version: u64,
    view: View,
    page_idx: usize,
    tab: Tab,
    sel: Option<(i64, i64)>, // (page id, shape id) — primary selection
    /// Multi-selection (marquee / shift-click); includes the primary.
    sel_set: Vec<(i64, i64)>,
    drag: Option<DragState>,
    stencil: Stencil,
    tex: TextureCache,
    mouse_page: Option<(f64, f64)>,
    status: String,
    show_shapesheet: bool,
    shot_frames: u32,
    /// Native file dialog in flight. rfd's sync API parks the calling
    /// thread until its private GTK thread finishes, so it must never
    /// run on the UI thread — otherwise the eframe event loop stalls
    /// and the compositor reports "not responding". The dialog runs on
    /// a worker thread; `poll_dialog` picks up the result each frame.
    dialog: Option<PendingDialog>,
    /// Canvas area from the last frame (stencil drop target).
    canvas_rect: Rect,
    /// File backstage view is open (green rail; 2010 three-band IA, #55).
    pub backstage: bool,
    backstage_page: BackPage,
    /// Live query for the backstage New page's template search (#55).
    backstage_search: String,
    /// "Save changes?" prompt before File → New discards a dirty doc.
    confirm_new: bool,
    /// Continue into a new blank drawing once the save-as dialog
    /// triggered by "Save & New" completes (or is cancelled).
    new_after_save: bool,
    /// Open shape context menu: (shape id, screen anchor, page-inches
    /// anchor y-up) — set by right-click, cleared on action/outside click.
    ctx_menu: Option<(i64, Pos2, (f64, f64))>,
    /// Last frame's context-menu rect in screen coords; None while the
    /// menu is closed (kept for the placement tests, issue #49).
    ctx_menu_rect: Option<egui::Rect>,
    /// In-app shape clipboard (Cut/Copy/Paste within DiagramCraft; rich
    /// OS-clipboard formats arrive with a later milestone).
    clipboard: Option<Shape>,
    /// Inline text editor target: (shape id, draft text).
    text_edit: Option<(i64, String)>,
    /// The last canvas click placed a stencil master — a double-click
    /// completing that click pair must not open a text editor.
    placed_last_click: bool,
    /// Fill/Line color palette target (opened from the ribbon).
    color_pick: Option<ColorTarget>,
    /// Most-recently-used files (start screen + backstage Open page).
    /// Ephemeral in the headless constructors; `App::new` swaps in the
    /// persistent on-disk list.
    recent: RecentFiles,
}

/// Which cell a ribbon palette click targets.
#[derive(Clone, Copy, PartialEq)]
enum ColorTarget {
    Fill,
    Line,
}

enum PendingDialog {
    Open(Receiver<Option<PathBuf>>),
    SaveAs(Receiver<Option<PathBuf>>),
}

#[derive(Clone, Copy)]
enum PendingKind {
    Open,
    SaveAs,
}

/// Backstage rail command.
#[derive(Clone, Copy)]
enum BackAction {
    Save,
    SaveAs,
    CloseDoc,
}

/// Backstage content page (GitHub #55: 2010 three-band rail — commands
/// above, destinations Info/Recent/New/Print/Share/Export, Options
/// pinned below; 2016 functions folded in).
#[derive(Clone, Copy, PartialEq)]
enum BackPage {
    Info,
    Recent,
    New,
    Print,
    Share,
    Export,
}

impl BackPage {
    /// Rail label for tests (`set_backstage_page_for_test`).
    fn from_test_name(name: &str) -> Option<BackPage> {
        Some(match name {
            "info" => BackPage::Info,
            "recent" => BackPage::Recent,
            "new" => BackPage::New,
            "print" => BackPage::Print,
            "share" => BackPage::Share,
            "export" => BackPage::Export,
            _ => return None,
        })
    }
}

struct DragState {
    shape: i64,
    grab_off: (f64, f64), // pin − pointer, inches (y-up)
    start_pin: (f64, f64),
    last_pin: (f64, f64),
    moved: bool,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, path: Option<PathBuf>) -> App {
        crate::theme::install(&cc.egui_ctx);
        let mut app = App::with_doc_opt(None);
        // Production entry only: adopt the persistent on-disk MRU
        // (tests and kittest constructors stay ephemeral so they never
        // touch the user's real config dir).
        app.recent = RecentFiles::open();
        if let Some(p) = path {
            app.load(p);
        }
        app
    }

    /// Headless-friendly constructor (theme applied by the host UI loop).
    pub fn with_doc_opt(path: Option<PathBuf>) -> App {
        let mut app = App {
            ed: None,
            path: PathBuf::new(),
            dirty: false,
            version: 0,
            view: View::default(),
            page_idx: 0,
            tab: Tab::Home,
            sel: None,
            sel_set: Vec::new(),
            drag: None,
            stencil: Stencil::default(),
            tex: TextureCache::default(),
            mouse_page: None,
            status: String::new(),
            show_shapesheet: false,
            shot_frames: 0,
            dialog: None,
            canvas_rect: Rect::NOTHING,
            backstage: false,
            backstage_page: BackPage::Info,
            backstage_search: String::new(),
            confirm_new: false,
            new_after_save: false,
            ctx_menu: None,
            ctx_menu_rect: None,
            clipboard: None,
            text_edit: None,
            placed_last_click: false,
            color_pick: None,
            recent: RecentFiles::ephemeral(),
        };
        if let Some(p) = path {
            app.load(p);
        }
        app
    }

    /// Open a document at `path` (headless tests, `dc-ui <file>`).
    pub fn with_doc(path: &std::path::Path) -> App {
        App::with_doc_opt(Some(path.to_path_buf()))
    }

    /// A fresh blank drawing with the starter shape masters
    /// (headless tests, File → New).
    pub fn with_blank() -> App {
        let mut app = App::with_doc_opt(None);
        app.do_new();
        app
    }

    /// Test hooks: select a shape / switch ribbon tab.
    #[doc(hidden)]
    pub fn select_shape_for_test(&mut self, page: i64, shape: i64) {
        self.set_selection(Some((page, shape)));
    }
    #[doc(hidden)]
    pub fn set_tab_for_test(&mut self, tab: crate::ribbon::Tab) {
        self.tab = tab;
    }

    fn load(&mut self, path: PathBuf) {
        match Editor::open(&path) {
            Ok(ed) => {
                self.status = format!("opened {}", path.display());
                self.recent.record(&path);
                self.ed = Some(ed);
                self.path = path;
                self.dirty = false;
                self.version += 1;
                self.view = View::default();
                self.page_idx = 0;
                self.set_selection(None);
                self.stencil.armed = None;
                self.tex.clear();
            }
            Err(e) => self.status = format!("open failed: {e:#}"),
        }
    }

    fn ed(&self) -> Option<&Editor> {
        self.ed.as_ref()
    }

    fn ed_mut(&mut self) -> Option<&mut Editor> {
        self.ed.as_mut()
    }

    fn page_geom(&self) -> PageGeom {
        match self.ed() {
            Some(ed) => {
                let page = &ed.doc.pages[self.page_idx.min(ed.doc.pages.len().saturating_sub(1))];
                let n = |name: &str| {
                    page.sheet()
                        .and_then(|s| s.cell(name))
                        .and_then(|c| c.numeric())
                        .unwrap_or(0.0)
                };
                PageGeom {
                    w_in: n("PageWidth").max(0.1),
                    h_in: n("PageHeight").max(0.1),
                }
            }
            None => PageGeom {
                w_in: 8.5,
                h_in: 11.0,
            },
        }
    }

    fn page_id(&self) -> Option<i64> {
        self.ed()
            .map(|ed| ed.doc.pages[self.page_idx.min(ed.doc.pages.len().saturating_sub(1))].id)
    }

    fn apply<C: dc_core::Command + 'static>(&mut self, cmd: C) {
        if let Some(ed) = self.ed_mut() {
            match ed.apply(cmd) {
                Ok(label) => {
                    self.status = label;
                    self.dirty = true;
                    self.version += 1;
                }
                Err(e) => self.status = format!("{e:#}"),
            }
        }
    }

    fn do_undo(&mut self) {
        if let Some(ed) = self.ed_mut() {
            match ed.undo() {
                Ok(Some(l)) => {
                    self.status = format!("undo: {l}");
                    self.dirty = true;
                    self.version += 1;
                    self.set_selection(None);
                }
                Ok(None) => self.status = "nothing to undo".into(),
                Err(e) => self.status = format!("{e:#}"),
            }
        }
    }

    fn do_redo(&mut self) {
        if let Some(ed) = self.ed_mut() {
            match ed.redo() {
                Ok(Some(l)) => {
                    self.status = format!("redo: {l}");
                    self.dirty = true;
                    self.version += 1;
                }
                Ok(None) => self.status = "nothing to redo".into(),
                Err(e) => self.status = format!("{e:#}"),
            }
        }
    }

    fn do_save(&mut self, save_as: bool) {
        if save_as || self.path.as_os_str().is_empty() {
            self.begin_dialog(PendingKind::SaveAs);
        } else {
            let path = self.path.clone();
            self.save_to(&path);
        }
    }

    fn do_open(&mut self) {
        self.begin_dialog(PendingKind::Open);
    }

    /// File → New: replace the current document with a blank drawing,
    /// prompting first if it has unsaved changes.
    fn do_new(&mut self) {
        if self.dirty && self.ed.is_some() {
            self.confirm_new = true;
        } else {
            self.start_new();
        }
    }

    fn start_new(&mut self) {
        self.confirm_new = false;
        self.new_after_save = false;
        self.ed = Some(Editor::new_blank());
        self.path = PathBuf::new();
        self.dirty = false;
        self.version += 1;
        self.view = View::default();
        self.page_idx = 0;
        self.set_selection(None);
        self.stencil = Stencil::default();
        self.tex.clear();
        self.status = "new blank drawing (unsaved)".into();
    }

    fn begin_dialog(&mut self, kind: PendingKind) {
        if self.dialog.is_some() {
            return; // one native dialog at a time
        }
        let default_name = self
            .path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "drawing.vsdx".into());
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let picked = match kind {
                PendingKind::Open => rfd::FileDialog::new()
                    .add_filter("Visio (VSDX family)", &["vsdx", "vstx", "vssx"])
                    .pick_file(),
                PendingKind::SaveAs => rfd::FileDialog::new()
                    .add_filter("Visio drawing", &["vsdx", "vstx", "vssx"])
                    .set_file_name(&default_name)
                    .save_file(),
            };
            let _ = tx.send(picked);
        });
        self.dialog = Some(match kind {
            PendingKind::Open => PendingDialog::Open(rx),
            PendingKind::SaveAs => PendingDialog::SaveAs(rx),
        });
    }

    /// Consume a finished dialog result; keep a slow repaint heartbeat
    /// while one is still open so the UI loop (and the window's event
    /// processing) never stalls behind it.
    fn poll_dialog(&mut self, ctx: &Context) {
        let Some(dialog) = self.dialog.take() else {
            return;
        };
        let (path, again) = match &dialog {
            PendingDialog::Open(rx) | PendingDialog::SaveAs(rx) => match rx.try_recv() {
                Ok(picked) => (picked, false),
                Err(mpsc::TryRecvError::Empty) => (None, true),
                Err(mpsc::TryRecvError::Disconnected) => (None, false),
            },
        };
        if again {
            self.dialog = Some(dialog);
            ctx.request_repaint_after(Duration::from_millis(100));
            return;
        }
        let Some(path) = path else {
            // cancelled: drop any chained "Save & New"
            self.new_after_save = false;
            return;
        };
        match dialog {
            PendingDialog::Open(_) => self.load(path),
            PendingDialog::SaveAs(_) => {
                self.save_to(&path);
                if self.new_after_save {
                    if self.dirty {
                        // save failed — keep the document open, drop the chain
                        self.new_after_save = false;
                    } else {
                        self.start_new();
                    }
                }
            }
        }
    }

    fn save_to(&mut self, path: &Path) {
        let Some(ed) = self.ed() else { return };
        match ed.save_as(path) {
            Ok(()) => {
                self.status = format!("saved {}", path.display());
                self.dirty = false;
                self.path = path.to_path_buf();
                self.recent.record(path);
            }
            Err(e) => self.status = format!("save failed: {e:#}"),
        }
    }

    // -- hit testing ------------------------------------------------------

    /// AABB in page coords (inches, y-up): (x, y, w, h); rotation ignored.
    fn shape_rect(&self, doc: &Document, page_id: i64, s: &Shape) -> Option<(f64, f64, f64, f64)> {
        Self::shape_rect_static(doc, page_id, s)
    }

    /// Borrow-free bbox (page inches, y-up) for marquee gating.
    fn shape_rect_static(doc: &Document, page_id: i64, s: &Shape) -> Option<(f64, f64, f64, f64)> {
        let chain = shape_chain(doc, page_id, s.id).ok()?;
        let get = |n: &str| doc.resolve_cell(s, &chain, n).and_then(|c| c.numeric());
        let (pin_x, pin_y) = (get("PinX")?, get("PinY")?);
        let w = get("Width").unwrap_or(1.0);
        let h = get("Height").unwrap_or(1.0);
        let lx = get("LocPinX").unwrap_or(w / 2.0);
        let ly = get("LocPinY").unwrap_or(h / 2.0);
        Some((pin_x - lx, pin_y - ly, w, h))
    }

    fn shape_hit(&self, page_pos: (f64, f64)) -> Option<i64> {
        let ed = self.ed()?;
        let page = &ed.doc.pages[self.page_idx.min(ed.doc.pages.len().saturating_sub(1))];
        for shape in page.shapes.iter().rev() {
            if let Some((x, y, w, h)) = self.shape_rect(&ed.doc, page.id, shape) {
                let (px, py) = page_pos;
                if px >= x && px <= x + w && py >= y && py <= y + h {
                    return Some(shape.id);
                }
            }
        }
        None
    }

    /// Selection frame in canvas rect space (inches, y-down origin).
    fn sel_extra_page_rects(&self) -> Vec<Rect> {
        let Some(ed) = self.ed.as_ref() else {
            return Vec::new();
        };
        let Some((page_id, primary)) = self.sel else {
            return Vec::new();
        };
        let page_h = self.page_geom().h_in;
        self.sel_set
            .iter()
            .filter(|(p, s)| *p == page_id && *s != primary)
            .filter_map(|(_, sid)| {
                let page = ed.doc.pages.get(self.page_idx)?;
                let sh = vsdx::model::find_shape_by_id(&page.shapes, *sid)?;
                let (x, y, w, h) = Self::shape_rect_static(&ed.doc, page_id, sh)?;
                Some(Rect::from_min_size(
                    Pos2::new(x as f32, (page_h - y - h) as f32),
                    Vec2::new(w as f32, h as f32),
                ))
            })
            .collect()
    }

    fn sel_page_rect(&self) -> Option<Rect> {
        let ed = self.ed()?;
        let (page_id, shape_id) = self.sel?;
        let page = ed.doc.pages.iter().find(|p| p.id == page_id)?;
        let shape = vsdx::model::find_shape_by_id(&page.shapes, shape_id)?;
        let (x, y, w, h) = self.shape_rect(&ed.doc, page_id, shape)?;
        let geom = self.page_geom();
        Some(Rect::from_min_size(
            Pos2::new(x as f32, (geom.h_in - y - h) as f32),
            Vec2::new(w as f32, h as f32),
        ))
    }

    /// One shape's bbox in SCREEN coords (y-down), for anchoring the
    /// context menu beside the shape (issue #49).
    fn shape_screen_rect(&self, shape_id: i64) -> Option<egui::Rect> {
        let ed = self.ed()?;
        let page_id = self.page_id()?;
        let page = ed.doc.pages.iter().find(|p| p.id == page_id)?;
        let shape = vsdx::model::find_shape_by_id(&page.shapes, shape_id)?;
        let (x, y, w, h) = self.shape_rect(&ed.doc, page_id, shape)?;
        let geom = self.page_geom();
        let ppi = self.view.px_per_in();
        let min = Pos2::new(
            self.view.pan.x + x as f32 * ppi,
            self.view.pan.y + (geom.h_in - y - h) as f32 * ppi,
        );
        Some(egui::Rect::from_min_size(
            min,
            Vec2::new(w as f32 * ppi, h as f32 * ppi),
        ))
    }

    fn drag_preview_rect(&self) -> Option<Rect> {
        let d = self.drag.as_ref()?;
        let ed = self.ed()?;
        let (page_id, sel_shape) = self.sel?;
        if page_id != self.page_id()? || d.shape != sel_shape {
            return None;
        }
        let page = ed.doc.pages.iter().find(|p| p.id == page_id)?;
        let shape = vsdx::model::find_shape_by_id(&page.shapes, d.shape)?;
        let (x, y, w, h) = self.shape_rect(&ed.doc, page_id, shape)?;
        let geom = self.page_geom();
        let (dx, dy) = (d.last_pin.0 - x, d.last_pin.1 - y);
        Some(Rect::from_min_size(
            Pos2::new((x + dx) as f32, (geom.h_in - y - h - dy) as f32),
            Vec2::new(w as f32, h as f32),
        ))
    }

    // -- interaction ------------------------------------------------------

    fn handle_shortcuts(&mut self, ctx: &Context) {
        let mut open = false;
        let mut save = false;
        let mut save_as = false;
        let mut new_doc = false;
        let kb_owned = ctx.egui_wants_keyboard_input();
        ctx.input(|i| {
            let ctrl = i.modifiers.ctrl || i.modifiers.command;
            if ctrl && i.key_pressed(egui::Key::N) {
                new_doc = true;
            }
            if ctrl && i.key_pressed(egui::Key::O) {
                open = true;
            }
            if ctrl && i.key_pressed(egui::Key::S) {
                if i.modifiers.shift {
                    save_as = true;
                } else {
                    save = true;
                }
            }
            if ctrl && i.key_pressed(egui::Key::Z) {
                if i.modifiers.shift {
                    self.do_redo();
                } else {
                    self.do_undo();
                }
            }
            if ctrl && i.key_pressed(egui::Key::Y) {
                self.do_redo();
            }
            if i.key_pressed(egui::Key::Delete) && self.sel.is_some() && !kb_owned {
                self.delete_selection();
            }
            // Shape clipboard / duplicate / text edit (Visio keys). Not
            // while a text field owns the keyboard.
            if !kb_owned && self.sel.is_some() {
                if ctrl && i.key_pressed(egui::Key::X) {
                    self.copy_selection();
                    let (p, s) = self.sel.unwrap();
                    self.apply(RemoveShape::new(p, s));
                    self.set_selection(None);
                }
                if ctrl && i.key_pressed(egui::Key::C) {
                    self.copy_selection();
                }
                if ctrl && i.key_pressed(egui::Key::V) {
                    if let Some((p, _)) = self.sel {
                        self.paste_clipboard(p);
                    }
                }
                if ctrl && i.key_pressed(egui::Key::D) {
                    if let Some((p, s)) = self.sel {
                        self.apply(DuplicateShape::new(p, s, (0.25, -0.25)));
                    }
                }
                if i.key_pressed(egui::Key::F2) {
                    if let Some((_, s)) = self.sel {
                        let draft = self.selected_text(s).unwrap_or_default();
                        self.text_edit = Some((s, draft));
                    }
                }
            }
        });
        if new_doc {
            self.do_new();
        }
        if open {
            self.do_open();
        }
        if save {
            self.do_save(false);
        }
        if save_as {
            self.do_save(true);
        }
    }

    fn delete_selection(&mut self) {
        let Some((page_id, _)) = self.sel else { return };
        if self.sel_set.len() > 1 {
            // One undo step for a multi-delete.
            let cmds: Vec<Box<dyn dc_core::Command>> = self
                .sel_set
                .iter()
                .filter(|(p, _)| *p == page_id)
                .map(|(_, s)| Box::new(RemoveShape::new(page_id, *s)) as Box<dyn dc_core::Command>)
                .collect();
            let n = cmds.len();
            self.apply(Macro::new(format!("delete {n} shapes"), cmds));
        } else {
            let shape_id = self.sel.unwrap().1;
            self.apply(RemoveShape::new(page_id, shape_id));
        }
        self.set_selection(None);
        self.sel_set.clear();
    }

    /// Marquee selection: page-inch rect (y-down) → every top-level
    /// shape whose bbox intersects joins the selection set.
    fn apply_marquee(&mut self, x0: f64, y0: f64, x1: f64, y1: f64) {
        let Some(ed) = self.ed() else { return };
        let Some(page_id) = self.page_id() else {
            return;
        };
        let page = &ed.doc.pages[self.page_idx.min(ed.doc.pages.len().saturating_sub(1))];
        let mut picked = Vec::new();
        for shape in page.shapes.iter() {
            if let Some((sx, sy, w, h)) = self.shape_rect(&ed.doc, page_id, shape) {
                // Both the shape rect and the marquee rect (from
                // canvas::screen_to_page) are y-up page inches.
                if sx <= x1 && sx + w >= x0 && sy <= y1 && sy + h >= y0 {
                    picked.push((page_id, shape.id));
                }
            }
        }
        self.sel_set = picked;
        self.sel = self.sel_set.last().copied();
    }

    /// Commit a corner-handle resize: the dragged corner moves by the
    /// page-inch delta (y-down); the opposite corner stays anchored and
    /// LocPin stays proportional (Visio's `Width*0.5` semantics).
    fn apply_resize(&mut self, corner: canvas::Corner, dx: f64, dy: f64) {
        let Some((page_id, shape_id)) = self.sel else {
            return;
        };
        let Some(ed) = self.ed.as_ref() else { return };
        let Some(page) = ed.doc.pages.iter().find(|p| p.id == page_id) else {
            return;
        };
        let Some(sh) = vsdx::model::find_shape_by_id(&page.shapes, shape_id) else {
            return;
        };
        let Ok(chain) = shape_chain(&ed.doc, page_id, shape_id) else {
            return;
        };
        let get = |n: &str| ed.doc.resolve_cell(sh, &chain, n).and_then(|c| c.numeric());
        let (Some(px), Some(py), Some(w), Some(h), Some(lx), Some(ly)) = (
            get("PinX"),
            get("PinY"),
            get("Width"),
            get("Height"),
            get("LocPinX"),
            get("LocPinY"),
        ) else {
            return;
        };
        // y-down delta → y-up geometry deltas per corner.
        let (dw, dh) = match corner {
            canvas::Corner::TopLeft => (-dx, -dy),
            canvas::Corner::TopRight => (dx, -dy),
            canvas::Corner::BottomLeft => (-dx, dy),
            canvas::Corner::BottomRight => (dx, dy),
        };
        let (w, h) = (w.max(1e-6), h.max(1e-6)); // ratio guards
        let nw = (w + dw).max(0.05);
        let nh = (h + dh).max(0.05);
        // Anchor the OPPOSITE corner: fractions (0|1) of its y-up bbox
        // position, selected by the dragged (screen-oriented) corner.
        // LocPin stays proportional (Visio's Width*0.5 semantics).
        let (ax, ay) = match corner {
            canvas::Corner::TopLeft => (1.0, 0.0),  // anchor bottom-right
            canvas::Corner::TopRight => (0.0, 0.0), // anchor bottom-left
            canvas::Corner::BottomLeft => (1.0, 1.0), // anchor top-right
            canvas::Corner::BottomRight => (0.0, 1.0), // anchor top-left
        };
        let anchor = (px + ax * w - lx, py + ay * h - ly);
        let nlx = lx * nw / w;
        let nly = ly * nh / h;
        let npx = anchor.0 - ax * nw + nlx;
        let npy = anchor.1 - ay * nh + nly;
        let scope = Scope::Shape {
            page: page_id,
            shape: shape_id,
        };
        let cell = |v: f64| vsdx::Cell::value(format!("{v:.6}"));
        let cmds: Vec<Box<dyn dc_core::Command>> = vec![
            Box::new(SetCell::new(scope.clone(), "Width", cell(nw))),
            Box::new(SetCell::new(scope.clone(), "Height", cell(nh))),
            Box::new(SetCell::new(scope.clone(), "LocPinX", cell(nlx))),
            Box::new(SetCell::new(scope.clone(), "LocPinY", cell(nly))),
            Box::new(SetCell::new(scope.clone(), "PinX", cell(npx))),
            Box::new(SetCell::new(scope, "PinY", cell(npy))),
        ];
        self.apply(Macro::new(format!("resize shape {shape_id}"), cmds));
    }

    /// Track an in-progress shape drag; commit on release.
    fn update_drag(&mut self, ctx: &egui::Context) {
        let (pressed, pos) = ctx.input(|i| (i.pointer.primary_down(), i.pointer.interact_pos()));
        let pointer = match self.ed() {
            Some(_) => pos,
            None => return,
        };
        let geom = self.page_geom();

        if pressed {
            if let (Some(p), Some(d)) = (pointer, self.drag.as_mut()) {
                let x_in = (p.x - self.view.pan.x) as f64 / self.view.px_per_in() as f64;
                let y_up =
                    geom.h_in - (p.y - self.view.pan.y) as f64 / self.view.px_per_in() as f64;
                let new_pin = (x_in + d.grab_off.0, y_up + d.grab_off.1);
                if (new_pin.0 - d.last_pin.0).abs() * self.view.px_per_in() as f64 > 0.5
                    || (new_pin.1 - d.last_pin.1).abs() * self.view.px_per_in() as f64 > 0.5
                {
                    d.moved = true;
                }
                d.last_pin = new_pin;
            }
        } else if let Some(d) = self.drag.take() {
            if d.moved {
                if let Some(page_id) = self.page_id() {
                    // Moving a multi-selection moves every member by the
                    // same delta, as one undo step.
                    let others: Vec<i64> = self
                        .sel_set
                        .iter()
                        .filter(|(p, s)| *p == page_id && *s != d.shape)
                        .map(|(_, s)| *s)
                        .collect();
                    if others.is_empty() {
                        self.apply(MoveShape::new(page_id, d.shape, d.last_pin));
                    } else {
                        let Some(ed) = self.ed() else { return };
                        let dx = d.last_pin.0 - d.start_pin.0;
                        let dy = d.last_pin.1 - d.start_pin.1;
                        let mut cmds: Vec<Box<dyn dc_core::Command>> =
                            vec![Box::new(MoveShape::new(page_id, d.shape, d.last_pin))];
                        for sid in others {
                            let page = &ed.doc.pages[self.page_idx];
                            let Some(sh) = vsdx::model::find_shape_by_id(&page.shapes, sid) else {
                                continue;
                            };
                            let Ok(chain) = shape_chain(&ed.doc, page_id, sid) else {
                                continue;
                            };
                            let get = |n: &str| {
                                ed.doc.resolve_cell(sh, &chain, n).and_then(|c| c.numeric())
                            };
                            if let (Some(px), Some(py)) = (get("PinX"), get("PinY")) {
                                cmds.push(Box::new(MoveShape::new(
                                    page_id,
                                    sid,
                                    (px + dx, py + dy),
                                )));
                            }
                        }
                        self.apply(Macro::new("move shapes", cmds));
                    }
                    self.set_selection(Some((page_id, d.shape)));
                }
            }
        }
    }

    /// Route every single-selection write through here so `sel` and
    /// `sel_set` stay consistent: the primary is always a member, and a
    /// cleared selection clears the set. Multi-selection paths manage
    /// `sel_set` directly and then set `sel`.
    fn set_selection(&mut self, sel: Option<(i64, i64)>) {
        self.sel = sel;
        match sel {
            Some(entry) if !self.sel_set.contains(&entry) => self.sel_set = vec![entry],
            Some(_) => {}
            None => self.sel_set.clear(),
        }
    }

    fn on_canvas_click(&mut self, page_pos: (f64, f64), shift: bool) {
        self.placed_last_click = false;
        // Stencil placement takes priority.
        if let (Some(master), Some(page_id)) = (self.stencil.armed, self.page_id()) {
            let new_id = self
                .ed
                .as_ref()
                .map(|ed| dc_core::commands::next_shape_id(&ed.doc, page_id));
            self.apply(AddMasterInstance::new(page_id, master, page_pos));
            self.stencil.armed = None;
            self.ctx_menu = None;
            self.placed_last_click = true;
            if let Some(nid) = new_id {
                self.set_selection(Some((page_id, nid)));
                self.sel_set = vec![(page_id, nid)];
            }
            return;
        }
        if shift {
            // Toggle membership in the multi-selection.
            if let Some(hit) = self.shape_hit(page_pos) {
                if let Some(p) = self.page_id() {
                    let entry = (p, hit);
                    if let Some(i) = self.sel_set.iter().position(|e| *e == entry) {
                        self.sel_set.remove(i);
                        self.sel = self.sel_set.last().copied();
                    } else {
                        self.sel_set.push(entry);
                        self.sel = Some(entry);
                    }
                }
            }
            self.ctx_menu = None;
            return;
        }
        match self.shape_hit(page_pos) {
            Some(shape_id) => {
                let page_id = self.page_id().expect("page");
                // Clicking a member of the current multi-selection keeps
                // the set (so a drag moves every member); clicking
                // anything else selects just that shape.
                if !self.sel_set.contains(&(page_id, shape_id)) {
                    self.sel_set = vec![(page_id, shape_id)];
                }
                // begin a drag: grab offset keeps the pin under the fingers
                if let Some(ed) = self.ed() {
                    let page = &ed.doc.pages[self.page_idx];
                    let shape = vsdx::model::find_shape_by_id(&page.shapes, shape_id);
                    if let Some(s) = shape {
                        let chain = shape_chain(&ed.doc, page_id, shape_id).ok();
                        let pin = chain.and_then(|c| {
                            ed.doc
                                .resolve_cell(s, &c, "PinX")
                                .and_then(|c| c.numeric())
                                .zip(ed.doc.resolve_cell(s, &c, "PinY").and_then(|c| c.numeric()))
                        });
                        if let Some((px, py)) = pin {
                            self.drag = Some(DragState {
                                shape: shape_id,
                                grab_off: (px - page_pos.0, py - page_pos.1),
                                start_pin: (px, py),
                                last_pin: (px, py),
                                moved: false,
                            });
                        }
                    }
                }
                self.set_selection(Some((page_id, shape_id)));
                self.status = format!("selected shape {shape_id}");
            }
            None => {
                self.set_selection(None);
            }
        }
    }

    /// Double-click on the canvas (page inches, y-up): Visio's route into
    /// a shape's linked text. The shape's `EventDblClick` formula,
    /// resolved through the inheritance chain (instance → master, like
    /// the Paragraph/Character handling), decides what happens.
    fn on_canvas_dbl_click(&mut self, page_pos: (f64, f64)) {
        // A stencil placement wins the click pair: its first click
        // placed the master, and the pair must not also open an editor.
        if self.stencil.armed.is_some() || self.placed_last_click {
            self.placed_last_click = false;
            return;
        }
        let Some(shape_id) = self.shape_hit(page_pos) else {
            return;
        };
        let Some(page_id) = self.page_id() else {
            return;
        };
        self.set_selection(Some((page_id, shape_id)));
        let formula = self.ed.as_ref().and_then(|ed| {
            let page = ed.doc.pages.iter().find(|p| p.id == page_id)?;
            let sh = vsdx::model::find_shape_by_id(&page.shapes, shape_id)?;
            let chain = shape_chain(&ed.doc, page_id, shape_id).ok()?;
            ed.doc
                .resolve_cell(sh, &chain, "EventDblClick")
                .and_then(|c| c.f)
        });
        match classify_dbl_click(formula.as_deref()) {
            DblClick::OpenText => {
                // The F2 path reached by mouse: seed the draft and let
                // `text_edit_window` run it.
                let draft = self.selected_text(shape_id).unwrap_or_default();
                self.text_edit = Some((shape_id, draft));
                self.status = format!("editing text of shape {shape_id}");
            }
            DblClick::Deferred(msg) => self.status = msg,
        }
    }

    fn dispatch(&mut self, a: Action) {
        match a {
            Action::NewDoc => self.do_new(),
            Action::Open => self.do_open(),
            Action::Save => self.do_save(false),
            Action::SaveAs => self.do_save(true),
            Action::Undo => self.do_undo(),
            Action::Redo => self.do_redo(),
            Action::NewPage => {
                if let Some(page_id) = self.page_id() {
                    let _ = page_id;
                }
                self.apply(AddPage::new(format!(
                    "Page-{}",
                    self.ed().map(|e| e.doc.pages.len() + 1).unwrap_or(1)
                )));
                if let Some(ed) = self.ed() {
                    self.page_idx = ed.doc.pages.len() - 1;
                    self.view.fit_pending = true;
                }
            }
            Action::Delete => self.delete_selection(),
            // Manual zooms leave fit mode: window resizes must not
            // clobber the user's chosen view (Fit Page re-arms it).
            Action::ZoomIn => {
                self.view.zoom = (self.view.zoom * 1.25).clamp(0.1, 8.0);
                self.view.fit_mode = false;
            }
            Action::ZoomOut => {
                self.view.zoom = (self.view.zoom / 1.25).clamp(0.1, 8.0);
                self.view.fit_mode = false;
            }
            Action::Zoom100 => {
                self.view.zoom = 1.0;
                self.view.fit_mode = false;
            }
            Action::FitPage => self.view.fit_pending = true,
            Action::ToggleGrid => self.view.show_grid = !self.view.show_grid,
            Action::ToggleRulers => self.view.show_rulers = !self.view.show_rulers,
            Action::ToggleUnits => {
                self.view.unit = if self.view.unit == crate::units::Unit::Mm {
                    crate::units::Unit::In
                } else {
                    crate::units::Unit::Mm
                };
            }
            Action::ShowShapeSheet => self.show_shapesheet = !self.show_shapesheet,
            Action::Macros => self.status = "macros arrive with M7 (VBA/.vsdm milestone)".into(),
            Action::FillMenu | Action::LineMenu => {
                self.color_pick = Some(if a == Action::FillMenu {
                    ColorTarget::Fill
                } else {
                    ColorTarget::Line
                });
            }
            Action::FontGrow => self.nudge_char_size(1.0),
            Action::FontShrink => self.nudge_char_size(-1.0),
            Action::RotateLeft => self.rotate_selection(90.0),
            Action::RotateRight => self.rotate_selection(-90.0),
        }
    }

    // -- shell panels -----------------------------------------------------

    fn title_band(&mut self, ui: &mut Ui) {
        // Visio 2016: green band, tiny QAT (Save/Undo/Redo + chevron) at
        // the far left, document title centered.
        let name = self
            .path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Drawing1".into());
        let title = format!(
            "{name} - DiagramCraft{}",
            if self.dirty { " *" } else { "" }
        );
        egui::Panel::top("title_band")
            .frame(
                egui::Frame::new()
                    .fill(theme::ACCENT)
                    .inner_margin(egui::Margin::symmetric(8, 2)),
            )
            .show(ui, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.set_height(30.0);
                    qat_button(ui, Icon::Save, "Save (Ctrl+S)", || Action::Save, self);
                    qat_button(ui, Icon::Undo, "Undo (Ctrl+Z)", || Action::Undo, self);
                    qat_button(ui, Icon::Redo, "Redo (Ctrl+Y)", || Action::Redo, self);
                    let (cr, cresp) = ui.allocate_exact_size(Vec2::new(12.0, 12.0), Sense::click());
                    if cresp.clicked() {
                        self.status = "QAT customization is not configurable yet".into();
                    }
                    ui.painter().add(egui::Shape::convex_polygon(
                        vec![
                            cr.left_top(),
                            cr.right_top(),
                            Pos2::new(cr.center().x, cr.bottom()),
                        ],
                        Color32::WHITE,
                        Stroke::NONE,
                    ));
                    // centered document title
                    let r = ui.max_rect();
                    ui.painter().text(
                        Pos2::new(r.center().x, r.center().y),
                        egui::Align2::CENTER_CENTER,
                        &title,
                        egui::FontId::proportional(12.5),
                        Color32::WHITE,
                    );
                });
            });
    }

    fn tab_strip(&mut self, ui: &mut Ui) {
        // Visio 2016 "Colorful" shell: a white strip where the File tab
        // is a green block running flush up into the green title band
        // (square top, rounded bottom corners — taller than the text
        // tabs), the active tab carries the accent underline, and
        // Office's "Tell me" search box sits at the strip's right end.
        const STRIP_H: f32 = 28.0;
        egui::Panel::top("tab_strip")
            .frame(
                egui::Frame::new()
                    .fill(theme::RIBBON_CONTENT)
                    .inner_margin(egui::Margin::same(0)),
            )
            .show(ui, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.set_min_height(STRIP_H);
                    // File: a green block spanning the full strip height
                    // — the green title band above makes it read as one
                    // tall tab, like the Office File tab.
                    let (fr, fresp) =
                        ui.allocate_exact_size(Vec2::new(54.0, STRIP_H), Sense::click());
                    ui.painter().rect_filled(
                        fr,
                        egui::CornerRadius {
                            nw: 0,
                            ne: 0,
                            se: 2,
                            sw: 2,
                        },
                        theme::ACCENT,
                    );
                    ui.painter().text(
                        fr.center(),
                        egui::Align2::CENTER_CENTER,
                        "File",
                        egui::FontId::proportional(12.5),
                        Color32::WHITE,
                    );
                    if fresp.clicked() {
                        self.backstage = true;
                        // #55 landing rule (2010): Info with a document
                        // open, Recent (welcome state) without one.
                        self.backstage_page = if self.ed.is_some() {
                            BackPage::Info
                        } else {
                            BackPage::Recent
                        };
                    }
                    ui.add_space(8.0);
                    for t in TABS {
                        if *t == Tab::File {
                            continue;
                        }
                        let active = *t == self.tab;
                        let txt = egui::RichText::new(t_title(*t))
                            .size(12.0)
                            .color(if active {
                                theme::ACCENT_DARK
                            } else {
                                Color32::from_rgb(70, 70, 70)
                            });
                        let b = egui::Button::new(txt).small().frame(false);
                        let resp = ui.add_enabled(true, b);
                        if resp.clicked() {
                            self.tab = *t;
                        }
                        let r = resp.rect;
                        if active {
                            ui.painter().hline(
                                r.left() + 2.0..=r.right() - 2.0,
                                r.bottom(),
                                Stroke::new(2.0, theme::ACCENT),
                            );
                        }
                        ui.add_space(6.0);
                    }
                    // Office's "Tell me what you want to do" search box.
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(6.0);
                        let (pr, presp) =
                            ui.allocate_exact_size(Vec2::new(196.0, 20.0), Sense::click());
                        let hot = presp.hovered();
                        ui.painter().rect_filled(pr, 2, Color32::WHITE);
                        ui.painter().rect_stroke(
                            pr,
                            2,
                            Stroke::new(
                                1.0,
                                if hot {
                                    theme::RULER_TICK
                                } else {
                                    theme::RIBBON_EDGE
                                },
                            ),
                            egui::StrokeKind::Middle,
                        );
                        Icon::TellMe.paint(
                            ui,
                            Rect::from_min_size(
                                pr.left_top() + Vec2::new(5.0, 3.0),
                                Vec2::splat(14.0),
                            ),
                            true,
                        );
                        ui.painter().text(
                            Pos2::new(pr.left() + 24.0, pr.center().y),
                            egui::Align2::LEFT_CENTER,
                            "Tell me what you want to do",
                            egui::FontId::proportional(11.0),
                            Color32::from_rgb(130, 130, 130),
                        );
                        let presp = presp.on_hover_text("Tell Me — planned for a later milestone");
                        if presp.clicked() {
                            self.status = "Tell Me search arrives with a later milestone".into();
                        }
                    });
                });
            });
    }

    fn ribbon_content(&mut self, ui: &mut Ui) {
        let has_doc = self.ed.is_some();
        let sel_active = self.sel.is_some();
        let has_sel = sel_active;
        let mut act: Option<Action> = None;
        let dispatch = |a: Action, act: &mut Option<Action>| {
            *act = Some(a);
        };
        let groups: Vec<crate::ribbon::Group> = match self.tab {
            // File opens the backstage view instead of ribbon content.
            Tab::File => vec![],
            Tab::Home => vec![
                crate::ribbon::Group::new(
                    "Clipboard",
                    vec![
                        big(RibbonButton::new(Icon::Paste, "Paste", Action::Macros)
                            .enabled(false)
                            .split()
                            .tip("Clipboard — planned for a later milestone")),
                        grid(vec![
                            vec![
                                RibbonButton::new(Icon::Cut, "Cut", Action::Macros)
                                    .enabled(false)
                                    .tip("Clipboard — planned"),
                                RibbonButton::new(Icon::Copy, "Copy", Action::Macros)
                                    .enabled(false)
                                    .tip("Clipboard — planned"),
                            ],
                            vec![RibbonButton::new(
                                Icon::Painter,
                                "Format Painter",
                                Action::Macros,
                            )
                            .enabled(false)
                            .tip("Format Painter — planned")],
                        ]),
                    ],
                ),
                crate::ribbon::Group::new(
                    "Font",
                    vec![grid(vec![
                        vec![
                            RibbonButton::new(Icon::FontA, "Calibri", Action::Macros)
                                .combo()
                                .width(128.0)
                                .enabled(false)
                                .tip("Font family — planned"),
                            RibbonButton::new(Icon::FontA, "11", Action::Macros)
                                .combo()
                                .width(46.0)
                                .enabled(false)
                                .tip("Font size — planned"),
                            RibbonButton::new(Icon::Grow, "", Action::FontGrow)
                                .enabled(has_sel)
                                .tip("Increase font size"),
                            RibbonButton::new(Icon::Shrink, "", Action::FontShrink)
                                .enabled(has_sel)
                                .tip("Decrease font size"),
                            RibbonButton::new(Icon::ClearAll, "", Action::Macros)
                                .enabled(false)
                                .tip("Clear all formatting — planned"),
                        ],
                        vec![
                            RibbonButton::new(Icon::Bold, "", Action::Macros)
                                .enabled(false)
                                .tip("Bold — planned"),
                            RibbonButton::new(Icon::Italic, "", Action::Macros)
                                .enabled(false)
                                .tip("Italic — planned"),
                            RibbonButton::new(Icon::Underline, "", Action::Macros)
                                .enabled(false)
                                .tip("Underline — planned"),
                            RibbonButton::new(Icon::Strike, "", Action::Macros)
                                .enabled(false)
                                .tip("Strikethrough — planned"),
                            RibbonButton::new(Icon::Subscript, "", Action::Macros)
                                .enabled(false)
                                .tip("Subscript — planned"),
                            RibbonButton::new(Icon::Superscript, "", Action::Macros)
                                .enabled(false)
                                .tip("Superscript — planned"),
                            RibbonButton::new(Icon::TextColor, "", Action::Macros)
                                .enabled(false)
                                .tip("Text color — planned"),
                        ],
                    ])],
                )
                .with_launcher(),
                crate::ribbon::Group::new(
                    "Paragraph",
                    vec![grid(vec![
                        vec![
                            RibbonButton::new(Icon::Align, "", Action::Macros)
                                .enabled(false)
                                .tip("Align left — planned"),
                            RibbonButton::new(Icon::AlignCenter, "", Action::Macros)
                                .enabled(false)
                                .tip("Align center — planned"),
                            RibbonButton::new(Icon::AlignRight, "", Action::Macros)
                                .enabled(false)
                                .tip("Align right — planned"),
                            RibbonButton::new(Icon::LineSpacing, "", Action::Macros)
                                .enabled(false)
                                .tip("Line spacing — planned"),
                        ],
                        vec![
                            RibbonButton::new(Icon::Bullets, "", Action::Macros)
                                .enabled(false)
                                .tip("Bullets — planned"),
                            RibbonButton::new(Icon::Numbering, "", Action::Macros)
                                .enabled(false)
                                .tip("Numbering — planned"),
                            RibbonButton::new(Icon::IndentLess, "", Action::Macros)
                                .enabled(false)
                                .tip("Decrease indent — planned"),
                            RibbonButton::new(Icon::IndentMore, "", Action::Macros)
                                .enabled(false)
                                .tip("Increase indent — planned"),
                        ],
                    ])],
                )
                .with_launcher(),
                crate::ribbon::Group::new(
                    "Shape Styles",
                    vec![
                        big(
                            RibbonButton::new(Icon::Theme, "Quick Styles", Action::Macros)
                                .enabled(false)
                                .tip("Style gallery — planned"),
                        ),
                        stack(vec![
                            RibbonButton::new(Icon::Bucket, "Fill", Action::FillMenu)
                                .enabled(has_sel)
                                .tip("Shape fill color"),
                            RibbonButton::new(Icon::Pen, "Line", Action::LineMenu)
                                .enabled(has_sel)
                                .tip("Line color"),
                            RibbonButton::new(Icon::Effects, "Effects", Action::Macros)
                                .enabled(false)
                                .tip("Shape effects — planned"),
                        ]),
                    ],
                ),
                crate::ribbon::Group::new(
                    "Arrange",
                    vec![
                        big(
                            RibbonButton::new(Icon::BringFront, "Bring Forward", Action::Macros)
                                .enabled(false)
                                .tip("Z-order — planned"),
                        ),
                        big(
                            RibbonButton::new(Icon::SendBack, "Send Backward", Action::Macros)
                                .enabled(false)
                                .tip("Z-order — planned"),
                        ),
                        grid(vec![
                            vec![
                                RibbonButton::new(Icon::Align, "Align", Action::Macros)
                                    .enabled(false)
                                    .tip("Alignment — planned"),
                                RibbonButton::new(
                                    Icon::Rotate,
                                    "Rotate Right",
                                    Action::RotateRight,
                                )
                                .enabled(has_sel)
                                .tip("Rotate selection 90° clockwise"),
                                RibbonButton::new(Icon::Rotate, "Rotate Left", Action::RotateLeft)
                                    .enabled(has_sel)
                                    .tip("Rotate selection 90° counter-clockwise"),
                            ],
                            vec![
                                RibbonButton::new(Icon::Group, "Group", Action::Macros)
                                    .enabled(false)
                                    .tip("Grouping — planned"),
                                RibbonButton::new(Icon::Delete, "Delete", Action::Delete)
                                    .enabled(sel_active)
                                    .tip("Delete the selected shape (Del)"),
                            ],
                        ]),
                    ],
                ),
            ],
            Tab::Insert => vec![
                crate::ribbon::Group::new(
                    "Blocks",
                    vec![
                        big(
                            RibbonButton::new(Icon::Container, "Container", Action::Macros)
                                .enabled(false)
                                .tip("Containers — planned"),
                        ),
                        big(RibbonButton::new(Icon::Callout, "Callout", Action::Macros)
                            .enabled(false)
                            .tip("Callouts — planned")),
                    ],
                ),
                crate::ribbon::Group::new(
                    "Illustrations",
                    vec![
                        big(RibbonButton::new(Icon::Picture, "Pictures", Action::Macros)
                            .enabled(false)
                            .tip("Picture shapes — planned")),
                        stack(vec![RibbonButton::new(
                            Icon::OnlinePic,
                            "Online Pictures",
                            Action::Macros,
                        )
                        .enabled(false)
                        .tip("Picture shapes — planned")]),
                    ],
                ),
                crate::ribbon::Group::new(
                    "Parts",
                    vec![
                        big(RibbonButton::new(Icon::TextBox, "Text Box", Action::Macros)
                            .enabled(false)
                            .tip("Text shapes — planned")),
                        stack(vec![RibbonButton::new(
                            Icon::Field,
                            "Field",
                            Action::Macros,
                        )
                        .enabled(false)
                        .tip("Fields — planned")]),
                    ],
                ),
                crate::ribbon::Group::new(
                    "Pages",
                    vec![big(RibbonButton::new(
                        Icon::NewPage,
                        "New Page",
                        Action::NewPage,
                    )
                    .enabled(has_doc))],
                ),
                crate::ribbon::Group::new(
                    "Comments",
                    vec![big(RibbonButton::new(
                        Icon::Comment,
                        "New Comment",
                        Action::Macros,
                    )
                    .enabled(false)
                    .tip("Comments — planned"))],
                ),
            ],
            Tab::Design => vec![
                crate::ribbon::Group::new(
                    "Themes",
                    vec![
                        big(RibbonButton::new(Icon::Theme, "Themes", Action::Macros)
                            .enabled(false)
                            .tip("Themes — planned")),
                        stack(vec![RibbonButton::new(
                            Icon::Theme,
                            "Variants",
                            Action::Macros,
                        )
                        .enabled(false)
                        .tip("Theme variants — planned")]),
                    ],
                ),
                crate::ribbon::Group::new(
                    "Page Setup",
                    vec![grid(vec![
                        vec![
                            RibbonButton::new(Icon::Orientation, "Orientation", Action::Macros)
                                .enabled(false)
                                .tip("Page orientation — planned"),
                            RibbonButton::new(Icon::NewPage, "Size", Action::Macros)
                                .enabled(false)
                                .tip("Page size — planned"),
                        ],
                        vec![RibbonButton::new(Icon::Shapes, "Auto Size", Action::Macros)
                            .enabled(false)
                            .tip("Auto page size — planned")],
                    ])],
                ),
                crate::ribbon::Group::new(
                    "Backgrounds",
                    vec![grid(vec![
                        vec![
                            RibbonButton::new(Icon::Picture, "Backgrounds", Action::Macros)
                                .enabled(false)
                                .tip("Page backgrounds — planned"),
                        ],
                        vec![
                            RibbonButton::new(Icon::Shapes, "Borders & Titles", Action::Macros)
                                .enabled(false)
                                .tip("Borders & titles — planned"),
                        ],
                    ])],
                ),
            ],
            Tab::Data => vec![
                crate::ribbon::Group::new(
                    "Display Data",
                    vec![
                        big(
                            RibbonButton::new(Icon::DataGraphic, "Data Graphic", Action::Macros)
                                .enabled(false)
                                .tip("Data graphics (M6)"),
                        ),
                        stack(vec![RibbonButton::new(
                            Icon::Shapes,
                            "Legend",
                            Action::Macros,
                        )
                        .enabled(false)
                        .tip("Legends (M6)")]),
                    ],
                ),
                crate::ribbon::Group::new(
                    "External Data",
                    vec![grid(vec![
                        vec![
                            RibbonButton::new(Icon::Link, "Link Data to Shapes", Action::Macros)
                                .enabled(false)
                                .tip("Data linking (M6)"),
                            RibbonButton::new(Icon::Refresh, "Refresh Data", Action::Macros)
                                .enabled(false)
                                .tip("Data linking (M6)"),
                        ],
                        vec![
                            RibbonButton::new(Icon::Open, "Quick Import", Action::Macros)
                                .enabled(false)
                                .tip("Data linking (M6)"),
                        ],
                    ])],
                ),
                crate::ribbon::Group::new(
                    "Show/Hide",
                    vec![stack(vec![
                        RibbonButton::new(Icon::ShapeSheet, "Shape Data Window", Action::Macros)
                            .enabled(false)
                            .tip("Shape data window (M6)"),
                        RibbonButton::new(Icon::Units, "External Data Window", Action::Macros)
                            .enabled(false)
                            .tip("External data window (M6)"),
                    ])],
                ),
            ],
            Tab::Process => vec![
                crate::ribbon::Group::new(
                    "Cross-Functional Flowchart",
                    vec![big(RibbonButton::new(
                        Icon::Swimlane,
                        "Swimlane",
                        Action::Macros,
                    )
                    .enabled(false)
                    .tip("Swimlanes — planned"))],
                ),
                crate::ribbon::Group::new(
                    "Diagram Layout",
                    vec![grid(vec![
                        vec![
                            RibbonButton::new(Icon::Shapes, "Re-layout Page", Action::Macros)
                                .enabled(false)
                                .tip("Auto layout — planned"),
                        ],
                        vec![
                            RibbonButton::new(Icon::Align, "Align & Space", Action::Macros)
                                .enabled(false)
                                .tip("Auto alignment — planned"),
                        ],
                    ])],
                ),
                crate::ribbon::Group::new(
                    "Validate",
                    vec![
                        big(
                            RibbonButton::new(Icon::Check, "Check Diagram", Action::Macros)
                                .enabled(false)
                                .tip("Diagram validation — planned"),
                        ),
                        stack(vec![RibbonButton::new(
                            Icon::Open,
                            "Import Flowchart Data",
                            Action::Macros,
                        )
                        .enabled(false)
                        .tip("Flowchart data import — planned")]),
                    ],
                ),
            ],
            Tab::Review => vec![
                crate::ribbon::Group::new(
                    "Proofing",
                    vec![
                        big(RibbonButton::new(Icon::Spell, "Spelling", Action::Macros)
                            .enabled(false)
                            .tip("Spell check — planned")),
                        stack(vec![RibbonButton::new(
                            Icon::FontA,
                            "Language",
                            Action::Macros,
                        )
                        .enabled(false)
                        .tip("Language settings — planned")]),
                    ],
                ),
                crate::ribbon::Group::new(
                    "Comments",
                    vec![big(RibbonButton::new(
                        Icon::Comment,
                        "New Comment",
                        Action::Macros,
                    )
                    .enabled(false)
                    .tip("Comments — planned"))],
                ),
            ],
            Tab::View => vec![
                crate::ribbon::Group::new(
                    "Show",
                    vec![grid(vec![
                        vec![
                            RibbonButton::new(Icon::Rulers, "Rulers", Action::ToggleRulers)
                                .selected(self.view.show_rulers),
                            RibbonButton::new(Icon::Grid, "Grid", Action::ToggleGrid)
                                .selected(self.view.show_grid),
                        ],
                        vec![
                            RibbonButton::new(Icon::Shapes, "Connection Points", Action::Macros)
                                .enabled(false)
                                .tip("Connection points — planned"),
                            RibbonButton::new(Icon::Shapes, "Guides", Action::Macros)
                                .enabled(false)
                                .tip("Guides — planned"),
                        ],
                    ])],
                ),
                crate::ribbon::Group::new(
                    "Zoom",
                    vec![
                        stack(vec![
                            RibbonButton::new(Icon::ZoomIn, "Zoom In", Action::ZoomIn),
                            RibbonButton::new(Icon::ZoomOut, "Zoom Out", Action::ZoomOut),
                        ]),
                        big(RibbonButton::new(Icon::Fit, "Fit Window", Action::FitPage)),
                        big(RibbonButton::new(Icon::ZoomIn, "100%", Action::Zoom100)),
                    ],
                ),
                crate::ribbon::Group::new(
                    "Visual Aids",
                    vec![big(RibbonButton::new(
                        Icon::Check,
                        "Visual Aids",
                        Action::Macros,
                    )
                    .enabled(false)
                    .tip("Snap & glue settings — planned"))],
                ),
                crate::ribbon::Group::new(
                    "Macros",
                    vec![big(RibbonButton::new(
                        Icon::Macro,
                        "Macros",
                        Action::Macros,
                    ))],
                ),
            ],
            Tab::Developer => vec![
                crate::ribbon::Group::new(
                    "Show/Hide",
                    vec![grid(vec![
                        vec![
                            RibbonButton::new(
                                Icon::ShapeSheet,
                                "ShapeSheet",
                                Action::ShowShapeSheet,
                            )
                            .selected(self.show_shapesheet)
                            .tip("Inspect the selected shape's cells"),
                            RibbonButton::new(Icon::Units, "mm/in", Action::ToggleUnits)
                                .tip("Toggle display units (model stays inches)"),
                        ],
                        vec![
                            RibbonButton::new(Icon::Shapes, "Drawing Explorer", Action::Macros)
                                .enabled(false)
                                .tip("Drawing explorer — planned"),
                        ],
                    ])],
                ),
                crate::ribbon::Group::new(
                    "Shape Design",
                    vec![grid(vec![
                        vec![
                            RibbonButton::new(Icon::Shapes, "Operations", Action::Macros)
                                .enabled(false)
                                .tip("Boolean shape operations — planned"),
                            RibbonButton::new(Icon::Group, "Behavior", Action::Macros)
                                .enabled(false)
                                .tip("Shape behavior — planned"),
                        ],
                        vec![RibbonButton::new(Icon::Check, "Protection", Action::Macros)
                            .enabled(false)
                            .tip("Shape protection — planned")],
                    ])],
                ),
                crate::ribbon::Group::new(
                    "Code",
                    vec![
                        big(RibbonButton::new(Icon::Macro, "Macros", Action::Macros)
                            .enabled(false)
                            .tip("VBA macros arrive with M7 (.vsdm)")),
                        stack(vec![RibbonButton::new(
                            Icon::Link,
                            "Add-ins",
                            Action::Macros,
                        )
                        .enabled(false)
                        .tip("Real Visio add-in hosting arrives with M8")]),
                    ],
                ),
            ],
        };
        let mut launcher: Option<&'static str> = None;
        crate::ribbon::content_row(ui, &groups, &mut |a| dispatch(a, &mut act), &mut |label| {
            launcher = Some(label)
        });
        if let Some(label) = launcher {
            self.status = format!("{label} dialog — planned for a later milestone");
        }
        if let Some(a) = act {
            self.dispatch(a);
        }
        ui.add_space(2.0);
    }

    fn page_tabs(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            let tabs: Vec<(String, i64)> = self
                .ed
                .as_ref()
                .map(|ed| {
                    ed.doc
                        .pages
                        .iter()
                        .map(|p| (p.name.clone(), p.id))
                        .collect()
                })
                .unwrap_or_default();
            if !tabs.is_empty() {
                for (i, (name, _id)) in tabs.iter().enumerate() {
                    let active = i == self.page_idx;
                    let txt = egui::RichText::new(name).size(11.5).color(if active {
                        Color32::from_rgb(30, 30, 30)
                    } else {
                        Color32::from_rgb(90, 90, 90)
                    });
                    let b = egui::Button::new(txt)
                        .small()
                        .fill(if active {
                            theme::TAB_ACTIVE
                        } else {
                            theme::TABS_BG
                        })
                        .frame(true);
                    let resp = ui
                        .add(b)
                        .on_hover_text(format!("page {} (id {})", i + 1, _id));
                    if resp.clicked() {
                        self.page_idx = i;
                        self.set_selection(None);
                        self.view.fit_pending = true;
                    }
                }
                if ui.button("+").on_hover_text("Append a page").clicked() {
                    self.dispatch(Action::NewPage);
                }
            }
        });
    }

    fn status_bar(&mut self, ui: &mut Ui) {
        ui.horizontal_centered(|ui| {
            let mut left;
            if let Some(ed) = self.ed() {
                let geom = self.page_geom();
                left = format!(
                    "Page {} of {}    {}",
                    self.page_idx + 1,
                    ed.doc.pages.len(),
                    self.view.unit.fmt(geom.w_in)
                        + " × "
                        + &self
                            .view
                            .unit
                            .fmt(geom.h_in)
                            .replace(" mm", "")
                            .replace(" in", "")
                        + " "
                        + self.view.unit.suffix(),
                );
                if let Some((_, sid)) = self.sel {
                    if let Some(page) = ed.doc.pages.get(self.page_idx) {
                        if let Some(s) = vsdx::model::find_shape_by_id(&page.shapes, sid) {
                            if let Some(chain) = shape_chain(&ed.doc, page.id, sid).ok() {
                                let g = |n: &str| {
                                    ed.doc
                                        .resolve_cell(s, &chain, n)
                                        .and_then(|c| c.numeric())
                                        .unwrap_or(0.0)
                                };
                                left.push_str(&format!(
                                    "    shape {}: {} × {} at {}",
                                    sid,
                                    self.view.unit.fmt(g("Width")),
                                    self.view.unit.fmt(g("Height")),
                                    self.view.unit.fmt_pt(g("PinX"), g("PinY")),
                                ));
                            }
                        }
                    }
                }
            } else {
                left = "no document".into();
            }
            ui.label(
                egui::RichText::new(left)
                    .color(theme::STATUS_TEXT)
                    .size(11.0),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Visio status bar right end: − [zoom slider] + N%
                if self.ed.is_some() {
                    let pct = (self.view.zoom * 100.0).round() as u32;
                    ui.label(
                        egui::RichText::new(format!("{pct}%"))
                            .color(theme::STATUS_TEXT)
                            .size(11.0)
                            .strong(),
                    );
                    let mut z = self.view.zoom;
                    let slider = egui::Slider::new(&mut z, 0.1..=8.0).show_value(false);
                    let sresp = ui.add_sized([110.0, 14.0], slider).on_hover_text("Zoom");
                    if sresp.changed() {
                        self.view.zoom = z.clamp(0.1, 8.0);
                        self.view.fit_mode = false;
                    }
                    if ui.button("−").on_hover_text("Zoom Out").clicked() {
                        self.dispatch(Action::ZoomOut);
                    }
                    if ui.button("+").on_hover_text("Zoom In").clicked() {
                        self.dispatch(Action::ZoomIn);
                    }
                    ui.separator();
                }
                if let Some((x, y)) = self.mouse_page {
                    ui.label(
                        egui::RichText::new(self.view.unit.fmt_pt(x, y))
                            .color(theme::STATUS_TEXT)
                            .size(11.0),
                    );
                } else {
                    ui.label(egui::RichText::new("").size(11.0));
                }
            });
        });
    }

    fn shapesheet_panel(&mut self, ui: &mut Ui) {
        ui.vertical(|ui| {
            ui.strong("ShapeSheet");
            ui.separator();
            let Some((page_id, shape_id)) = self.sel else {
                ui.label("Select a shape to inspect its cells.");
                return;
            };
            let Some(ed) = self.ed() else { return };
            let Ok(sheet) = scope_sheet(
                &ed.doc,
                &Scope::Shape {
                    page: page_id,
                    shape: shape_id,
                },
            ) else {
                ui.label("shape not found");
                return;
            };
            egui::ScrollArea::vertical().show(ui, |ui| {
                egui::Grid::new("cells").striped(true).show(ui, |ui| {
                    ui.label("Cell");
                    ui.label("Value");
                    ui.label("Unit");
                    ui.label("Formula");
                    ui.end_row();
                    let mut cells: Vec<_> = sheet
                        .children
                        .iter()
                        .filter_map(|c| match c {
                            vsdx::model::SheetChild::Cell(n, c) => Some((n.clone(), c.clone())),
                            _ => None,
                        })
                        .collect();
                    cells.sort_by(|a, b| a.0.cmp(&b.0));
                    for (n, c) in cells {
                        ui.label(n);
                        ui.label(c.v.clone().unwrap_or_else(|| "—".into()));
                        ui.label(c.u.clone().unwrap_or_else(|| "—".into()));
                        ui.label(c.f.clone().unwrap_or_else(|| "—".into()));
                        ui.end_row();
                    }
                });
                // Sections with their rows — the ShapeSheet's real
                // structure (Geometry, Character, Paragraph, Text Block
                // Format, Actions, …), like Visio's inspector.
                for sec in sheet.children.iter().filter_map(|c| match c {
                    vsdx::model::SheetChild::Section(s) => Some(s),
                    _ => None,
                }) {
                    ui.add_space(6.0);
                    ui.strong(section_display_name(&sec.name));
                    egui::Grid::new(format!("sec-{}", sec.name))
                        .striped(true)
                        .show(ui, |ui| {
                            ui.label("Row");
                            ui.label("Cell");
                            ui.label("Value");
                            ui.label("Formula");
                            ui.end_row();
                            if sec.rows.is_empty() {
                                // Section-level cells (NoFill, NoLine, …).
                                for (n, c) in &sec.cells {
                                    ui.label("—");
                                    ui.label(n);
                                    ui.label(c.v.clone().unwrap_or_else(|| "—".into()));
                                    ui.label(c.f.clone().unwrap_or_else(|| "—".into()));
                                    ui.end_row();
                                }
                            }
                            for row in &sec.rows {
                                let label = row
                                    .n
                                    .clone()
                                    .or_else(|| row.t.clone())
                                    .unwrap_or_else(|| format!("Row {}", row.ix));
                                let mut first = true;
                                for (n, c) in &row.cells {
                                    if first {
                                        ui.strong(&label);
                                        first = false;
                                    } else {
                                        ui.label(" ");
                                    }
                                    ui.label(n);
                                    ui.label(c.v.clone().unwrap_or_else(|| "—".into()));
                                    ui.label(c.f.clone().unwrap_or_else(|| "—".into()));
                                    ui.end_row();
                                }
                                if first {
                                    ui.strong(&label);
                                    ui.label("—");
                                    ui.label("—");
                                    ui.label("—");
                                    ui.end_row();
                                }
                            }
                        });
                }
            });
        });
    }

    /// The Recent list shared by the start screen and the backstage
    /// Open page: one row per MRU entry, click to reopen. Files that no
    /// longer exist stay listed but ghosted (Visio-style) rather than
    /// being silently dropped. Returns the clicked path, if any.
    fn recent_list_ui(&self, ui: &mut Ui) -> Option<PathBuf> {
        if self.recent.entries().is_empty() {
            ui.weak("Files you open will appear here.");
            return None;
        }
        let mut clicked = None;
        for p in self.recent.entries() {
            let name = p
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| p.display().to_string());
            let missing = !p.exists();
            // Theme-aware: the start screen's central area follows the
            // egui visuals (dark by default), so live entries take the
            // normal text color and ghosts the weak one — the same
            // pairing the empty-list placeholder uses.
            let txt = if missing {
                egui::RichText::new(name).color(ui.visuals().weak_text_color())
            } else {
                egui::RichText::new(name)
            };
            if ui
                .button(txt)
                .on_hover_text(p.display().to_string())
                .clicked()
            {
                clicked = Some(p.clone());
            }
        }
        clicked
    }

    fn central(&mut self, ui: &mut Ui) {
        if self.ed.is_none() {
            // Visio 2016 start screen shape: Recent on the left,
            // template choices on the right.
            ui.horizontal_centered(|ui| {
                ui.add_space(40.0);
                ui.vertical(|ui| {
                    ui.add_space(24.0);
                    ui.heading("Recent");
                    ui.add_space(6.0);
                    if let Some(p) = self.recent_list_ui(ui) {
                        self.load(p);
                    }
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                    ui.vertical(|ui| {
                        ui.add_space(24.0);
                        ui.heading("New");
                        ui.add_space(6.0);
                        if ui
                            .button("Blank drawing")
                            .on_hover_text("Start an empty US-Letter drawing (Ctrl+N)")
                            .clicked()
                        {
                            self.do_new();
                        }
                        if ui
                            .button("Open a Visio file…")
                            .on_hover_text("Open .vsdx / .vstx / .vssx (Ctrl+O)")
                            .clicked()
                        {
                            self.do_open();
                        }
                    });
                });
            });
            return;
        }
        let npages = self.ed.as_ref().unwrap().doc.pages.len();
        self.page_idx = self.page_idx.min(npages.saturating_sub(1));
        let (svg, part) = {
            let ed = self.ed.as_ref().unwrap();
            let page = &ed.doc.pages[self.page_idx];
            match dc::svg::render_page(&ed.doc, page) {
                Ok(s) => (s, page.part.clone()),
                Err(e) => {
                    ui.label(format!("render failed: {e:#}"));
                    return;
                }
            }
        };
        let geom = self.page_geom();
        let sel_rect = self.sel_page_rect();
        let extra_rects: Vec<Rect> = self.sel_extra_page_rects();
        let drag_rect = self.drag_preview_rect();
        let dragging = self.drag.is_some();
        let armed = self.stencil.armed.is_some();
        let tex = self.tex.get(
            ui.ctx(),
            &svg,
            geom.w_in,
            &part.replace('/', "_"),
            self.version,
            self.view.px_per_in(),
        );
        let tex = match tex {
            Ok(t) => t.clone(),
            Err(e) => {
                ui.label(format!("raster failed: {e:#}"));
                return;
            }
        };
        let mut mouse = self.mouse_page;
        let App {
            view, ed, page_idx, ..
        } = self;
        let ed = ed.as_ref();
        let page = ed.map(|e| &e.doc.pages[(*page_idx).min(e.doc.pages.len().saturating_sub(1))]);
        let hit = |pos: (f64, f64)| -> bool {
            let Some(ed) = ed else { return false };
            let Some(page) = page else { return false };
            for shape in page.shapes.iter().rev() {
                if let Some((x, y, w, h)) = Self::shape_rect_static(&ed.doc, page.id, shape) {
                    if pos.0 >= x && pos.0 <= x + w && pos.1 >= y && pos.1 <= y + h {
                        return true;
                    }
                }
            }
            false
        };
        let out = canvas::show(
            ui,
            view,
            &geom,
            Some(&tex),
            sel_rect,
            &extra_rects,
            drag_rect,
            dragging,
            armed,
            &hit,
            &mut mouse,
        );
        self.mouse_page = mouse;
        self.canvas_rect = out.canvas_rect;
        if let Some(pos) = out.dbl_click_page_pos {
            // The second click of the pair completes a double-click
            // instead of a plain click: the first click already ran its
            // select (and possible drag-start) semantics on its frame.
            self.on_canvas_dbl_click(pos);
        } else if let Some(pos) = out.click_page_pos {
            self.on_canvas_click(pos, out.shift_click);
        }
        if let Some((x0, y0, x1, y1)) = out.marquee_done {
            self.apply_marquee(x0, y0, x1, y1);
        }
        if let Some((corner, (dx, dy))) = out.resize_done {
            self.apply_resize(corner, dx, dy);
        }
        if let Some(pos) = out.right_click_page_pos {
            // Visio: right-click selects the shape under the cursor and
            // opens its context menu (empty page = no shape menu yet).
            match self.shape_hit(pos) {
                Some(shape_id) => {
                    if let Some(p) = self.page_id() {
                        self.set_selection(Some((p, shape_id)));
                    }
                    let anchor = ui
                        .ctx()
                        .input(|i| i.pointer.interact_pos())
                        .unwrap_or_default();
                    self.ctx_menu = Some((shape_id, anchor, pos));
                }
                None => self.ctx_menu = None,
            }
        }
        self.shape_context_menu(ui.ctx());
    }

    /// The Visio 2016 shape context menu: clipboard, duplicate, delete,
    /// z-order, rotate/flip, text editing, ShapeSheet. Backlog items
    /// (Group, Format Shape, Shape Data) show grayed with their milestone
    /// so the surface matches Visio while the backends catch up.
    /// Master-defined right-click actions (ShapeSheet Actions section),
    /// in row order: (menu label, is-flyout-child). Visio heads the shape
    /// menu with these.
    fn master_actions(&self, shape_id: i64) -> Vec<(String, bool)> {
        fn collect(self_: &App, shape_id: i64) -> Option<Vec<(String, bool)>> {
            fn walk(sh: &Shape, out: &mut Vec<(String, bool)>) {
                for sec in sh.sheet.sections_named("Actions") {
                    for row in &sec.rows {
                        let Some(menu) = row
                            .cell("Menu")
                            .and_then(|c| c.v.clone())
                            .map(|t| t.trim().to_owned())
                            .filter(|t| !t.is_empty())
                        else {
                            continue;
                        };
                        if row.cell("Invisible").and_then(|c| c.numeric()) == Some(1.0) {
                            continue;
                        }
                        let child = row.cell("FlyoutChild").and_then(|c| c.numeric()) == Some(1.0);
                        out.push((menu, child));
                    }
                }
                for sub in &sh.sub_shapes {
                    walk(sub, out);
                }
            }
            let ed = self_.ed.as_ref()?;
            let page = ed.doc.pages.get(self_.page_idx)?;
            let sh = vsdx::model::find_shape_by_id(&page.shapes, shape_id)?;
            let m = ed.doc.master(sh.master?)?;
            let mut out = Vec::new();
            for root in &m.shapes {
                walk(root, &mut out);
            }
            Some(out)
        }
        collect(self, shape_id).unwrap_or_default()
    }

    /// Bump the selected shapes' Character.Size by whole points.
    fn nudge_char_size(&mut self, delta_pt: f64) {
        let targets = self.sel_set.clone();
        let Some(ed) = self.ed.as_ref() else { return };
        let mut cmds: Vec<Box<dyn dc_core::Command>> = Vec::new();
        for (page_id, sid) in targets {
            let Some(page) = ed.doc.pages.iter().find(|p| p.id == page_id) else {
                continue;
            };
            let Some(sh) = vsdx::model::find_shape_by_id(&page.shapes, sid) else {
                continue;
            };
            let Ok(chain) = shape_chain(&ed.doc, page_id, sid) else {
                continue;
            };
            let sheet = ed.doc.effective_sheet(sh, &chain);
            let cur = sheet
                .sections_named("Character")
                .first()
                .and_then(|sec| sec.rows.first())
                .and_then(|r| r.cell("Size"))
                .and_then(|c| c.numeric())
                .unwrap_or(0.13888);
            let next = ((cur * 72.0 + delta_pt).max(4.0).min(200.0)) / 72.0;
            cmds.push(Box::new(SetRowCell::new(
                Scope::Shape {
                    page: page_id,
                    shape: sid,
                },
                "Character",
                0,
                dc_core::commands::RowSel::Ix(0),
                "Size",
                vsdx::Cell::value(format!("{next:.6}")),
            )));
        }
        if !cmds.is_empty() {
            self.apply(Macro::new("font size", cmds));
        }
    }

    /// Rotate every selected shape ±90° (degrees, Visio CCW positive).
    fn rotate_selection(&mut self, deg: f64) {
        let targets = self.sel_set.clone();
        let Some(ed) = self.ed.as_ref() else { return };
        let mut cmds: Vec<Box<dyn dc_core::Command>> = Vec::new();
        for (page_id, sid) in targets {
            let Some(page) = ed.doc.pages.iter().find(|p| p.id == page_id) else {
                continue;
            };
            let Some(sh) = vsdx::model::find_shape_by_id(&page.shapes, sid) else {
                continue;
            };
            let Ok(chain) = shape_chain(&ed.doc, page_id, sid) else {
                continue;
            };
            let cur = ed
                .doc
                .resolve_cell(sh, &chain, "Angle")
                .and_then(|c| c.numeric())
                .unwrap_or(0.0);
            let next = cur + deg.to_radians();
            cmds.push(Box::new(SetCell::new(
                Scope::Shape {
                    page: page_id,
                    shape: sid,
                },
                "Angle",
                vsdx::Cell::value(format!("{next:.8}")),
            )));
        }
        if !cmds.is_empty() {
            self.apply(Macro::new("rotate shapes", cmds));
        }
    }

    /// Apply a picked color to FillForegnd or LineColor on the selection.
    fn apply_color(&mut self, target: ColorTarget, color: &str) {
        let cell_name = match target {
            ColorTarget::Fill => "FillForegnd",
            ColorTarget::Line => "LineColor",
        };
        let targets = self.sel_set.clone();
        let mut cmds: Vec<Box<dyn dc_core::Command>> = Vec::new();
        for (page_id, sid) in targets {
            cmds.push(Box::new(SetCell::new(
                Scope::Shape {
                    page: page_id,
                    shape: sid,
                },
                cell_name,
                vsdx::Cell::value(color.to_owned()),
            )));
        }
        if !cmds.is_empty() {
            self.apply(Macro::new(format!("set {cell_name}"), cmds));
        }
    }

    #[allow(deprecated)] // Id::new: one context menu, stable identity wanted
    fn shape_context_menu(&mut self, ctx: &egui::Context) {
        let Some((shape_id, pointer_anchor, _page_pos)) = self.ctx_menu else {
            self.ctx_menu_rect = None;
            return;
        };
        let has_clip = self.clipboard.is_some();
        // Master-defined actions (the ShapeSheet Actions section) head the
        // menu in Visio — e.g. the Capability master's "Shape Layout"
        // flyout. Entries render with their flyout children indented;
        // executing SETF actions arrives with a later milestone.
        let actions = self.master_actions(shape_id);

        #[derive(Clone, Copy, PartialEq)]
        enum Act {
            Cut,
            Copy,
            Paste,
            Duplicate,
            Delete,
            Front,
            Back,
            Rot(f64),
            FlipV,
            FlipH,
            EditText,
            ShapeSheet,
        }
        let mut act: Option<Act> = None;
        // Min menu width incl. frame, until the popup reports its own
        // measured size (used only for the right-edge flip decision).
        const MENU_MIN_W: f32 = 244.0;
        let mut menu = egui::Popup::new(
            egui::Id::new("dc-shape-ctx-menu"), // stable per-menu id is intended
            ctx.clone(),
            pointer_anchor,
            egui::LayerId::background(),
        )
        .kind(egui::PopupKind::Menu)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        // Cap the sizing pass: menu rows stretch to available width, so
        // without this the first frame balloons the menu to the viewport
        // width and it covers the page (the pre-#49 symptom).
        .width(232.0);
        // Visio anchors the menu just outside the shape's top-right
        // corner, mirroring to the top-left corner when the right screen
        // edge is too close (issue #49). Anchoring to the shape's live
        // rect keeps the menu glued to it through zoom/pan; if the rect
        // cannot be resolved, fall back to the pointer position like a
        // classic context menu (egui defaults). Alternatives stay empty
        // so the corner anchor is deterministic.
        if let Some(r) = self.shape_screen_rect(shape_id) {
            let menu_w = menu
                .get_expected_size()
                .map_or(MENU_MIN_W, |s| s.x.max(MENU_MIN_W));
            // The popup Area is constrained into the content rect, so
            // measure room against the same rect.
            let room_right = ctx.content_rect().right() - r.right() >= menu_w;
            menu = menu
                .anchor(r)
                .gap(2.0)
                .align(if room_right {
                    egui::RectAlign::RIGHT_START
                } else {
                    egui::RectAlign::LEFT_START
                })
                .align_alternatives(&[]);
        }
        let popup = menu.show(|ui| {
            ui.set_min_width(232.0);
            // Menu colors resolve from the live visuals via the theme
            // mapping, so the popup follows a light/dark switch (GitHub
            // #48) instead of assuming Visio-light surroundings.
            let tokens = theme::menu_tokens(ui.visuals());
            ui.visuals_mut().widgets.inactive.bg_fill = egui::Color32::TRANSPARENT;
            ui.visuals_mut().widgets.hovered.bg_fill = tokens.hover_fill;
            // separators ride the token too (egui's dark default is too
            // faint on the dark menu fill)
            ui.visuals_mut().widgets.noninteractive.bg_stroke.color = tokens.separator;
            // Visio order: master actions, Show ShapeSheet, clipboard,
            // object commands, then the grayed future milestones.
            for (menu, child) in &actions {
                let label = if *child {
                    format!("     {menu}")
                } else {
                    format!("{menu}  ▸")
                };
                ctx_item(ui, None, &label, "(v0.5)", false);
            }
            if !actions.is_empty() {
                ui.separator();
            }
            if ctx_item(
                ui,
                Some(crate::ribbon::Icon::ShapeSheet),
                "Show ShapeSheet",
                "",
                true,
            ) {
                act = Some(Act::ShapeSheet);
            }
            ui.separator();
            if ctx_item(ui, Some(crate::ribbon::Icon::Cut), "Cut", "Ctrl+X", true) {
                act = Some(Act::Cut);
            }
            if ctx_item(ui, Some(crate::ribbon::Icon::Copy), "Copy", "Ctrl+C", true) {
                act = Some(Act::Copy);
            }
            if ctx_item(
                ui,
                Some(crate::ribbon::Icon::Paste),
                "Paste",
                "Ctrl+V",
                has_clip,
            ) {
                act = Some(Act::Paste);
            }
            if ctx_item(ui, None, "Duplicate", "Ctrl+D", true) {
                act = Some(Act::Duplicate);
            }
            if ctx_item(ui, Some(crate::ribbon::Icon::Delete), "Delete", "Del", true) {
                act = Some(Act::Delete);
            }
            ui.separator();
            if ctx_item(
                ui,
                Some(crate::ribbon::Icon::BringFront),
                "Bring to Front",
                "",
                true,
            ) {
                act = Some(Act::Front);
            }
            if ctx_item(
                ui,
                Some(crate::ribbon::Icon::SendBack),
                "Send to Back",
                "",
                true,
            ) {
                act = Some(Act::Back);
            }
            ui.separator();
            if ctx_item(
                ui,
                Some(crate::ribbon::Icon::Rotate),
                "Rotate Right 90°",
                "",
                true,
            ) {
                act = Some(Act::Rot(-std::f64::consts::FRAC_PI_2));
            }
            if ctx_item(
                ui,
                Some(crate::ribbon::Icon::Rotate),
                "Rotate Left 90°",
                "",
                true,
            ) {
                act = Some(Act::Rot(std::f64::consts::FRAC_PI_2));
            }
            if ctx_item(ui, None, "Flip Vertical", "", true) {
                act = Some(Act::FlipV);
            }
            if ctx_item(ui, None, "Flip Horizontal", "", true) {
                act = Some(Act::FlipH);
            }
            ui.separator();
            ctx_item(ui, None, "Group", "(v0.5)", false);
            ctx_item(ui, None, "Container", "(v0.5)", false);
            ctx_item(ui, None, "Hyperlink…", "(v0.5)", false);
            ctx_item(ui, None, "Add Comment", "(v0.5)", false);
            ctx_item(ui, None, "Shape Data…", "(v0.5)", false);
            ctx_item(ui, None, "Format Shape…", "(v0.5)", false);
            ui.separator();
            if ctx_item(ui, None, "Edit Text…", "F2", true) {
                act = Some(Act::EditText);
            }
        });
        self.ctx_menu_rect = popup.as_ref().map(|ir| ir.response.rect);
        // Close on an outside primary click; a right-click elsewhere
        // re-anchors through central()'s handler.
        let outside_primary = ctx.input(|i| i.pointer.primary_clicked());
        if let Some(ir) = &popup {
            if outside_primary
                && !ir
                    .response
                    .rect
                    .contains(ctx.input(|i| i.pointer.interact_pos()).unwrap_or_default())
            {
                self.ctx_menu = None;
            }
        }
        let Some(a) = act else { return };
        self.ctx_menu = None;
        let Some((page_id, _)) = self.sel else { return };
        match a {
            Act::Cut | Act::Copy => {
                self.copy_selection();
                if a == Act::Cut {
                    self.apply(RemoveShape::new(page_id, shape_id));
                    self.set_selection(None);
                }
            }
            Act::Paste => {
                self.paste_clipboard(page_id);
            }
            Act::Duplicate => {
                let new_id = self
                    .ed
                    .as_ref()
                    .map(|ed| dc_core::commands::next_shape_id(&ed.doc, page_id));
                self.apply(DuplicateShape::new(page_id, shape_id, (0.25, -0.25)));
                if let Some(nid) = new_id {
                    self.set_selection(Some((page_id, nid)));
                }
            }
            Act::Delete => self.delete_selection(),
            Act::Front => self.apply(ReorderShape::new(page_id, shape_id, true)),
            Act::Back => self.apply(ReorderShape::new(page_id, shape_id, false)),
            Act::Rot(delta) => {
                let cur = self.selected_cell(shape_id, "Angle").unwrap_or(0.0);
                self.apply(SetCell::new(
                    Scope::Shape {
                        page: page_id,
                        shape: shape_id,
                    },
                    "Angle",
                    vsdx::Cell::value(format!("{:.10}", cur + delta)),
                ));
            }
            Act::FlipV | Act::FlipH => {
                let name = if a == Act::FlipV { "FlipY" } else { "FlipX" };
                let cur = self.selected_cell(shape_id, name).unwrap_or(0.0);
                self.apply(SetCell::new(
                    Scope::Shape {
                        page: page_id,
                        shape: shape_id,
                    },
                    name,
                    vsdx::Cell::value(if cur != 0.0 { "0" } else { "1" }),
                ));
            }
            Act::EditText => {
                let draft = self.selected_text(shape_id).unwrap_or_default();
                self.text_edit = Some((shape_id, draft));
            }
            Act::ShapeSheet => self.show_shapesheet = true,
        }
    }

    /// Deep-clone the selected shape into the in-app clipboard.
    fn copy_selection(&mut self) {
        let Some((page_id, shape_id)) = self.sel else {
            return;
        };
        let Some(ed) = self.ed.as_ref() else { return };
        let Some(page) = ed.doc.pages.iter().find(|p| p.id == page_id) else {
            return;
        };
        if let Some(s) = vsdx::model::find_shape_by_id(&page.shapes, shape_id) {
            self.clipboard = Some(s.clone());
        }
    }

    /// Paste the clipboard shape as a fresh instance (new id, nudged pin),
    /// top of the z-order, and select it.
    fn paste_clipboard(&mut self, page_id: i64) {
        let Some(clip) = self.clipboard.clone() else {
            return;
        };
        let Some(ed) = self.ed.as_ref() else { return };
        let mut pasted = clip;
        pasted.id = dc_core::commands::next_shape_id(&ed.doc, page_id);
        for (name, dv) in [("PinX", 0.25), ("PinY", -0.25)] {
            let v = pasted
                .sheet
                .cell(name)
                .and_then(|c| c.numeric())
                .unwrap_or(1.0)
                + dv;
            pasted
                .sheet
                .set_cell(name, vsdx::Cell::value(format!("{v}")));
        }
        let new_id = pasted.id;
        self.apply(AddShape {
            page: page_id,
            parent: None,
            shape: pasted,
        });
        self.set_selection(Some((page_id, new_id)));
    }

    /// One resolved numeric cell of a shape on the current page.
    fn selected_cell(&self, shape_id: i64, name: &str) -> Option<f64> {
        let page_id = self.page_id()?;
        let ed = self.ed.as_ref()?;
        let page = ed.doc.pages.iter().find(|p| p.id == page_id)?;
        let s = vsdx::model::find_shape_by_id(&page.shapes, shape_id)?;
        let chain = shape_chain(&ed.doc, page_id, shape_id).ok()?;
        ed.doc
            .resolve_cell(s, &chain, name)
            .and_then(|c| c.numeric())
    }

    /// Current text of a shape (draft seed for the Edit Text dialog).
    fn selected_text(&self, shape_id: i64) -> Option<String> {
        let page_id = self.page_id()?;
        let ed = self.ed.as_ref()?;
        let page = ed.doc.pages.iter().find(|p| p.id == page_id)?;
        let s = vsdx::model::find_shape_by_id(&page.shapes, shape_id)?;
        s.sheet.text.clone()
    }

    /// Test/introspection hooks for the kittest interaction tests.
    #[doc(hidden)]
    pub fn debug_shape_count(&self) -> usize {
        self.ed
            .as_ref()
            .and_then(|ed| ed.doc.pages.get(self.page_idx))
            .map(|p| p.shapes.len())
            .unwrap_or(0)
    }
    #[doc(hidden)]
    pub fn debug_master_cells(&self) -> Vec<(i64, Rect)> {
        self.stencil.cell_rects.clone()
    }
    #[doc(hidden)]
    pub fn debug_canvas_rect(&self) -> Rect {
        self.canvas_rect
    }
    #[doc(hidden)]
    pub fn debug_sel(&self) -> Option<(i64, i64)> {
        self.sel
    }
    #[doc(hidden)]
    pub fn debug_sel_set(&self) -> Vec<(i64, i64)> {
        self.sel_set.clone()
    }
    /// Selection rect in SCREEN coords (for corner-handle tests).
    #[doc(hidden)]
    pub fn debug_sel_br_corner(&self) -> Option<egui::Pos2> {
        let r = self.sel_page_rect()?;
        let ppi = self.view.px_per_in();
        Some(egui::pos2(
            self.view.pan.x + r.right() as f32 * ppi,
            self.view.pan.y + r.bottom() as f32 * ppi,
        ))
    }
    #[doc(hidden)]
    pub fn debug_sel_width(&self) -> f64 {
        self.sel_page_rect()
            .map(|r| r.width() as f64)
            .unwrap_or(0.0)
    }
    #[doc(hidden)]
    pub fn debug_sel_height(&self) -> f64 {
        self.sel_page_rect()
            .map(|r| r.height() as f64)
            .unwrap_or(0.0)
    }
    #[doc(hidden)]
    pub fn debug_sel_tl_corner(&self) -> Option<egui::Pos2> {
        let r = self.sel_page_rect()?;
        let ppi = self.view.px_per_in();
        Some(egui::pos2(
            self.view.pan.x + r.left() as f32 * ppi,
            self.view.pan.y + r.top() as f32 * ppi,
        ))
    }
    /// One shape's bbox in SCREEN coords (context-menu placement tests).
    #[doc(hidden)]
    pub fn debug_shape_screen_rect(&self, shape_id: i64) -> Option<egui::Rect> {
        self.shape_screen_rect(shape_id)
    }
    /// Context-menu rect from the last frame; None while closed
    /// (placement tests, issue #49).
    #[doc(hidden)]
    pub fn debug_ctx_menu_rect(&self) -> Option<egui::Rect> {
        self.ctx_menu_rect
    }
    #[doc(hidden)]
    pub fn debug_armed(&self) -> Option<i64> {
        self.stencil.armed
    }
    #[doc(hidden)]
    pub fn debug_carry(&self) -> Option<i64> {
        self.stencil.carry
    }
    /// MRU entries, most recent first (recent-files tests).
    #[doc(hidden)]
    pub fn debug_recent(&self) -> &[PathBuf] {
        self.recent.entries()
    }
    /// Adopt an MRU verbatim, never persisted (kittest snapshot of the
    /// populated Recent panes).
    #[doc(hidden)]
    pub fn set_recent_for_test(&mut self, files: Vec<PathBuf>) {
        self.recent.set_entries_for_test(files);
    }
    /// kittest: enter the backstage exactly like the File tab does —
    /// the #55 landing rule included (Info with a document, Recent
    /// without one).
    #[doc(hidden)]
    pub fn open_backstage_for_test(&mut self) {
        self.backstage = true;
        self.backstage_page = if self.ed.is_some() {
            BackPage::Info
        } else {
            BackPage::Recent
        };
    }
    /// kittest: jump straight to one backstage page ("info" | "recent"
    /// | "new" | "print" | "share" | "export").
    #[doc(hidden)]
    pub fn set_backstage_page_for_test(&mut self, name: &str) {
        if let Some(p) = BackPage::from_test_name(name) {
            self.backstage_page = p;
        }
    }
    /// kittest: whether the File backstage is showing.
    #[doc(hidden)]
    pub fn debug_backstage(&self) -> bool {
        self.backstage
    }
    /// Current canvas zoom (fit-mode interaction tests).
    #[doc(hidden)]
    pub fn debug_zoom(&self) -> f32 {
        self.view.zoom
    }
    /// A shape's current text (double-click edit tests).
    #[doc(hidden)]
    pub fn debug_shape_text(&self, shape_id: i64) -> Option<String> {
        self.selected_text(shape_id)
    }

    /// The inline text editor opened by Edit Text… (Visio F2).
    fn text_edit_window(&mut self, ctx: &egui::Context) {
        let Some((shape_id, mut draft)) = self.text_edit.take() else {
            return;
        };
        let mut apply = false;
        let mut cancel = false;
        let mut open = true;
        let win = egui::Window::new("Edit Text")
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .open(&mut open)
            .min_width(340.0)
            .show(ctx, |ui| {
                ui.add(
                    egui::TextEdit::multiline(&mut draft)
                        .hint_text("Shape text")
                        .desired_rows(5)
                        .desired_width(f32::INFINITY),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("OK").clicked() {
                        apply = true;
                    }
                    if ui.button("Cancel").clicked() {
                        // closed below via the flag
                        cancel = true;
                    }
                });
            });
        let _ = win;
        if apply {
            if let Some(page_id) = self.page_id() {
                self.apply(SetText::new(
                    Scope::Shape {
                        page: page_id,
                        shape: shape_id,
                    },
                    draft,
                ));
            }
        } else if cancel || !open {
            // discarded
        } else {
            self.text_edit = Some((shape_id, draft));
        }
    }

    /// The Visio theme palette shown by the ribbon Fill/Line buttons.
    fn color_palette_window(&mut self, ctx: &egui::Context) {
        let Some(target) = self.color_pick else {
            return;
        };
        let mut open = true;
        egui::Window::new(match target {
            ColorTarget::Fill => "Fill Color",
            ColorTarget::Line => "Line Color",
        })
        .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
        .open(&mut open)
        .show(ctx, |ui| {
            let theme_colors = [
                "#000000", "#FFFFFF", "#EEEEEE", "#969696", "#4472C4", "#5B9BD5", "#70AD47",
                "#ED7D31", "#FFC000", "#FF0000", "#7030A0", "#00B0F0", "#FFF2CC", "#DEEAF6",
                "#E2EFDA", "#FCE4D6",
            ];
            egui::Grid::new("theme-palette")
                .num_columns(8)
                .spacing([6.0, 6.0])
                .show(ui, |ui| {
                    for (i, c) in theme_colors.iter().enumerate() {
                        // from_hex requires the leading '#'.
                        let color = egui::Color32::from_hex(c).unwrap_or(egui::Color32::GRAY);
                        let (r, resp) =
                            ui.allocate_exact_size(egui::vec2(24.0, 24.0), Sense::click());
                        ui.painter().rect_filled(r, 2.0, color);
                        ui.painter().rect_stroke(
                            r,
                            2.0,
                            Stroke::new(1.0, Color32::from_gray(120)),
                            egui::StrokeKind::Middle,
                        );
                        if resp.clicked() {
                            self.apply_color(target, c);
                            self.color_pick = None;
                        }
                        if i % 8 == 7 {
                            ui.end_row();
                        }
                    }
                });
        });
        if !open {
            self.color_pick = None;
        }
    }

    /// Drag-and-drop from the stencil pane: while the primary button is
    /// held after leaving a master cell, ghost the shape at the cursor
    /// (thumbnail at real shape size); on release over the canvas,
    /// place an instance there. Click-to-arm then click-to-place still
    /// works alongside this.
    fn stencil_carry(&mut self, ctx: &Context) {
        let Some(master) = self.stencil.carry else {
            return;
        };
        let (down, released, pos) = ctx.input(|i| {
            (
                i.pointer.primary_down(),
                i.pointer.primary_released(),
                i.pointer.interact_pos(),
            )
        });
        if !down && !released {
            self.stencil.carry = None; // button lost (cancelled elsewhere)
            return;
        }
        let Some(p) = pos else { return };
        let ppi = self.view.px_per_in();
        if down {
            ctx.output_mut(|o| o.cursor_icon = egui::CursorIcon::Crosshair);
            // Ghost: the master thumbnail drawn at the shape's real
            // on-page size, semi-transparent, pinned at the cursor.
            let App { ed, stencil, .. } = self;
            if let Some(ed) = ed.as_ref() {
                let (w, h) = master_size(&ed.doc, master);
                let size = Vec2::new((w as f32 * ppi).max(10.0), (h as f32 * ppi).max(10.0));
                let r = Rect::from_center_size(p, size);
                let layer =
                    egui::LayerId::new(egui::Order::Tooltip, egui::Id::unique("stencil-ghost"));
                let painter = ctx.layer_painter(layer);
                let tex = stencil.texture(ctx, ed, master);
                painter.image(
                    tex.id(),
                    r,
                    Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                    Color32::from_white_alpha(150),
                );
                painter.rect_stroke(
                    r,
                    1.0,
                    Stroke::new(1.0, theme::SELECTION),
                    egui::StrokeKind::Middle,
                );
            }
        }
        if released {
            self.stencil.carry = None;
            if self.canvas_rect.contains(p) {
                if let Some(page_id) = self.page_id() {
                    let geom = self.page_geom();
                    let (x, y) = canvas::screen_to_page(p, &self.view, &geom);
                    self.apply(AddMasterInstance::new(page_id, master, (x, y)));
                }
            }
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        self.draw(ui);
    }
}

impl App {
    /// The full shell, testable headlessly (see tests/shell.rs).
    pub fn draw(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        self.handle_shortcuts(&ctx);
        self.update_drag(&ctx);
        self.poll_dialog(&ctx);
        self.text_edit_window(&ctx);
        self.color_palette_window(&ctx);

        if self.backstage {
            self.backstage_view(ui);
            self.confirm_new_window(&ctx);
            return;
        }

        // Shell order: top bands, bottom bands, side panels, canvas.
        self.title_band(ui);
        self.tab_strip(ui);
        egui::Panel::top("ribbon_content")
            .frame(
                egui::Frame::new()
                    .fill(theme::RIBBON_CONTENT)
                    .inner_margin(egui::Margin::symmetric(4, 3)),
            )
            .show(ui, |ui| self.ribbon_content(ui));
        // Visio: a hairline rule closes the ribbon off from the canvas.
        egui::Panel::top("ribbon_rule")
            .exact_size(1.0)
            .frame(egui::Frame::new().fill(theme::RIBBON_EDGE))
            .show(ui, |_| {});
        egui::Panel::bottom("status")
            .frame(
                egui::Frame::new()
                    .fill(theme::STATUS_BG)
                    .inner_margin(egui::Margin::symmetric(10, 3)),
            )
            .show(ui, |ui| self.status_bar(ui));
        egui::Panel::bottom("page_tabs")
            .frame(
                egui::Frame::new()
                    .fill(theme::TABS_BG)
                    .inner_margin(egui::Margin::same(2)),
            )
            .show(ui, |ui| self.page_tabs(ui));
        if self
            .ed()
            .map(|ed| !ed.doc.masters.is_empty())
            .unwrap_or(false)
        {
            egui::Panel::left("stencil")
                .frame(
                    egui::Frame::new()
                        .fill(theme::STENCIL_BG)
                        .inner_margin(egui::Margin::same(6)),
                )
                .default_size(160.0)
                .resizable(true)
                .show(ui, |ui| {
                    ui.vertical(|ui| {
                        ui.strong("Shapes");
                        ui.separator();
                        let mut armed = None;
                        let App { ed, stencil, .. } = self;
                        if let Some(ed) = ed.as_ref() {
                            if let Some(id) = stencil.show(ui, ed) {
                                armed = Some(id);
                            }
                        }
                        if let Some(id) = armed {
                            self.status =
                                format!("master {} armed — click the canvas to place", id);
                        }
                        if self.stencil.armed.is_some() && ui.button("x cancel placement").clicked()
                        {
                            self.stencil.armed = None;
                        }
                    });
                });
        }
        if self.show_shapesheet {
            egui::Panel::right("shapesheet")
                .default_size(260.0)
                .resizable(true)
                .show(ui, |ui| self.shapesheet_panel(ui));
        }
        self.central(ui);
        self.stencil_carry(&ctx);

        // Automated visual smoke: DC_UI_SCREENSHOT=path.png captures the
        // 30th frame and exits (headless CI / review pipeline).
        if let Some(path) = std::env::var_os("DC_UI_SCREENSHOT") {
            self.shot_frames += 1;
            if self.shot_frames == 30 {
                let path = std::path::PathBuf::from(path);
                ctx.request_screenshot(move |img| {
                    let _ = write_png(&path, &img);
                    std::process::exit(0);
                });
            }
        }
        self.confirm_new_window(&ctx);
    }

    /// Unsaved-changes prompt before File → New discards work.
    fn confirm_new_window(&mut self, ctx: &Context) {
        if !self.confirm_new {
            return;
        }
        egui::Window::new("Save changes?")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label("The current drawing has unsaved changes.");
                ui.horizontal(|ui| {
                    if ui.button("Save & New").clicked() {
                        self.confirm_new = false;
                        if self.path.as_os_str().is_empty() {
                            // pick a file first; the new document
                            // starts after the dialog completes
                            self.new_after_save = true;
                            self.begin_dialog(PendingKind::SaveAs);
                        } else {
                            let p = self.path.clone();
                            self.save_to(&p);
                            self.start_new();
                        }
                    }
                    if ui.button("Discard & New").clicked() {
                        self.start_new();
                    }
                    if ui.button("Cancel").clicked() {
                        self.confirm_new = false;
                    }
                });
            });
    }

    /// The File backstage (Visio 2016): green command rail on the left,
    /// page content on the right. Replaces the whole shell while open.
    /// The File backstage (GitHub #55): the 2010 three-band rail —
    /// commands (Save/Save As/Close) above the destinations (Info,
    /// Recent, New, Print, Share, Export), Options pinned below — with
    /// 2016's functions folded in (Share/Export split, Close-document,
    /// search-first New). Every color rides theme tokens; Esc and the
    /// back control return to the document.
    fn backstage_view(&mut self, ui: &mut Ui) {
        let mut close_backstage = false;
        let mut action: Option<BackAction> = None;
        // Esc always returns to the document — unless the unsaved-
        // changes prompt owns the screen.
        if !self.confirm_new && ui.ctx().input(|i| i.key_pressed(egui::Key::Escape)) {
            close_backstage = true;
        }
        let has_doc = self.ed.is_some();
        egui::Panel::left("backstage_rail")
            .frame(
                egui::Frame::new()
                    .fill(theme::ACCENT)
                    .inner_margin(egui::Margin::same(10)),
            )
            .default_size(190.0)
            .resizable(false)
            .show(ui, |ui| {
                ui.set_min_height(ui.available_height());
                ui.vertical(|ui| {
                    let back = egui::Button::new(
                        egui::RichText::new("<  Back")
                            .color(Color32::WHITE)
                            .strong(),
                    )
                    .frame(false);
                    if ui.add(back).clicked() {
                        close_backstage = true;
                    }
                    ui.add_space(10.0);
                    // One row builder for both bands: a command runs an
                    // action immediately; a destination switches pages.
                    let mut rail = |ui: &mut Ui,
                                    label: &str,
                                    enabled: bool,
                                    page: Option<BackPage>,
                                    act: Option<BackAction>| {
                        let selected = page.is_some() && page == Some(self.backstage_page);
                        let txt = egui::RichText::new(label).size(14.0).color(if enabled {
                            Color32::WHITE
                        } else {
                            theme::RAIL_DISABLED
                        });
                        let mut b = egui::Button::new(txt).frame(false);
                        if selected {
                            b = b.fill(theme::RAIL_SELECTED);
                        }
                        let resp = ui.add_enabled(enabled, b);
                        if resp.clicked() {
                            if let Some(p) = page {
                                self.backstage_page = p;
                            }
                            action = act;
                        }
                        resp
                    };
                    // Band 1 — the command cluster above the destinations
                    // (2010's separation of actions from navigation, the
                    // structure Apple's and Material's rail guidance ask
                    // for).
                    rail(ui, "Save", has_doc, None, Some(BackAction::Save));
                    rail(ui, "Save As", has_doc, None, Some(BackAction::SaveAs));
                    rail(ui, "Close", has_doc, None, Some(BackAction::CloseDoc));
                    // Band divider.
                    let (sep, _) = ui
                        .allocate_exact_size(Vec2::new(ui.available_width(), 1.0), Sense::hover());
                    ui.painter().hline(
                        sep.left()..=sep.right(),
                        sep.center().y,
                        Stroke::new(1.0, Color32::from_white_alpha(60)),
                    );
                    ui.add_space(4.0);
                    // Band 2 — destinations (six, inside M3's 3–7 window;
                    // 2016's Share/Export split replaces 2010's
                    // "Save & Send").
                    rail(ui, "Info", true, Some(BackPage::Info), None);
                    rail(ui, "Recent", true, Some(BackPage::Recent), None);
                    rail(ui, "New", true, Some(BackPage::New), None);
                    rail(ui, "Print", true, Some(BackPage::Print), None)
                        .on_hover_text("Print the current drawing");
                    rail(ui, "Share", true, Some(BackPage::Share), None)
                        .on_hover_text("Share a copy of the drawing");
                    rail(ui, "Export", true, Some(BackPage::Export), None)
                        .on_hover_text("Save the drawing in another format");
                });
                // Band 3 — pinned footer, non-critical items only (Apple:
                // "avoid putting critical information or actions at the
                // bottom of a sidebar").
                ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                    ui.add_enabled(
                        false,
                        egui::Button::new(
                            egui::RichText::new("Options")
                                .size(14.0)
                                .color(theme::RAIL_DISABLED),
                        )
                        .frame(false),
                    )
                    .on_disabled_hover_text("Options arrive with a later milestone");
                });
            });
        // Page content (the 2010 white backstage surface).
        let mut open_browse = false;
        let mut open_recent: Option<PathBuf> = None;
        let mut new_blank = false;
        let page = self.backstage_page;
        let mut search_text = self.backstage_search.clone();
        egui::Frame::new()
            .fill(theme::RIBBON_CONTENT)
            .inner_margin(egui::Margin::same(28))
            .show(ui, |ui| {
                ui.add_space(12.0);
                match page {
                    BackPage::Info => {
                        if has_doc {
                            let name = self
                                .path
                                .file_name()
                                .map(|n| n.to_string_lossy().into_owned())
                                .unwrap_or_else(|| "Untitled".into());
                            let dir = self
                                .path
                                .parent()
                                .map(|p| p.display().to_string())
                                .filter(|s| !s.is_empty())
                                .unwrap_or_else(|| "Not saved yet".into());
                            let status = if self.dirty {
                                "Unsaved changes"
                            } else {
                                "All changes saved"
                            };
                            ui.heading(&name);
                            ui.add_space(8.0);
                            egui::Grid::new("backstage_info")
                                .num_columns(2)
                                .spacing(Vec2::new(18.0, 6.0))
                                .show(ui, |ui| {
                                    ui.weak("Name");
                                    ui.label(&name);
                                    ui.end_row();
                                    ui.weak("Location");
                                    ui.label(&dir);
                                    ui.end_row();
                                    ui.weak("Status");
                                    ui.label(status);
                                    ui.end_row();
                                });
                            ui.add_space(14.0);
                            ui.horizontal(|ui| {
                                ui.add_enabled(false, egui::Button::new("Protect Drawing…"))
                                    .on_disabled_hover_text(
                                        "Document protection arrives with a later milestone",
                                    );
                                ui.add_enabled(false, egui::Button::new("Inspect Drawing…"))
                                    .on_disabled_hover_text(
                                        "Document inspection arrives with a later milestone",
                                    );
                            });
                        } else {
                            ui.heading("No document open");
                            ui.add_space(8.0);
                            if ui.button("Open a file…").clicked() {
                                open_browse = true;
                            }
                        }
                    }
                    BackPage::Recent => {
                        ui.horizontal(|ui| {
                            ui.heading("Recent");
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                                if ui.button("Browse…").clicked() {
                                    open_browse = true;
                                }
                            });
                        });
                        ui.add_space(8.0);
                        if self.recent.entries().is_empty() {
                            // elementary Welcome Screen: context plus
                            // named actions instead of an empty list.
                            ui.weak("Drawings you open will appear here.");
                            ui.add_space(12.0);
                            if welcome_action(
                                ui,
                                Icon::New,
                                "New blank drawing",
                                "Start an empty drawing on a letter-size page.",
                            ) {
                                new_blank = true;
                            }
                            if welcome_action(
                                ui,
                                Icon::Open,
                                "Open a file",
                                "Browse for an existing Visio package.",
                            ) {
                                open_browse = true;
                            }
                        } else if let Some(p) = self.recent_list_ui(ui) {
                            open_recent = Some(p);
                        }
                    }
                    BackPage::New => {
                        ui.horizontal(|ui| {
                            ui.heading("New");
                            // 2016's search-first New page; elementary/
                            // Apple: trailing toolbar position, instant
                            // type-to-filter.
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                                ui.add(
                                    egui::TextEdit::singleline(&mut search_text)
                                        .hint_text("Search Templates")
                                        .desired_width(200.0),
                                );
                            });
                        });
                        ui.add_space(4.0);
                        ui.weak("Suggested: flowchart · network · floor plan");
                        ui.add_space(12.0);
                        let q = search_text.trim().to_lowercase();
                        let mut card_click = false;
                        ui.horizontal_wrapped(|ui| {
                            if backstage_card(ui, Icon::New, "Blank drawing", q.is_empty()) {
                                card_click = true;
                            }
                            for (name, icon) in NEW_TEMPLATE_ENTRIES {
                                if !q.is_empty() && !name.to_lowercase().contains(&q) {
                                    continue;
                                }
                                backstage_card(ui, *icon, name, false);
                            }
                        });
                        if card_click {
                            new_blank = true;
                        }
                        if !q.is_empty()
                            && !"blank drawing".contains(&q)
                            && NEW_TEMPLATE_ENTRIES
                                .iter()
                                .all(|(n, _)| !n.to_lowercase().contains(&q))
                        {
                            ui.add_space(8.0);
                            ui.weak(format!("No templates match “{}”.", search_text.trim()));
                        }
                    }
                    BackPage::Print => backstage_stub(
                        ui,
                        "Print",
                        "Send the drawing to a printer or save it as a PDF.",
                        "Print…",
                        "Printing arrives with M6",
                    ),
                    BackPage::Share => backstage_stub(
                        ui,
                        "Share",
                        "Share a copy of the drawing with other people.",
                        "Share a Copy…",
                        "Sharing arrives with M6",
                    ),
                    BackPage::Export => backstage_stub(
                        ui,
                        "Export",
                        "Save the drawing in another format.",
                        "Export…",
                        "Export filters arrive with M6",
                    ),
                }
            });
        self.backstage_search = search_text;
        if let Some(a) = action {
            match a {
                BackAction::Save => {
                    self.do_save(false);
                    close_backstage = true;
                }
                BackAction::SaveAs => {
                    self.do_save(true);
                    close_backstage = true;
                }
                BackAction::CloseDoc => {
                    self.ed = None;
                    self.path = PathBuf::new();
                    self.dirty = false;
                    self.set_selection(None);
                    self.stencil = Stencil::default();
                    self.tex.clear();
                    self.status = "document closed".into();
                    close_backstage = true;
                }
            }
        }
        if open_browse {
            close_backstage = true;
            self.do_open();
        }
        if let Some(p) = open_recent {
            close_backstage = true;
            self.load(p);
        }
        if new_blank {
            self.do_new();
            if !self.confirm_new {
                close_backstage = true;
            }
        }
        if close_backstage {
            self.backstage = false;
        }
    }
}

/// Placeholder template names on the backstage New page (generic
/// diagram-type names, ours — template bodies arrive with the M5 flow).
const NEW_TEMPLATE_ENTRIES: &[(&str, Icon)] = &[
    ("Flowchart", Icon::Swimlane),
    ("Cross-functional", Icon::Container),
    ("Network Diagram", Icon::Link),
    ("Org Chart", Icon::Group),
    ("Floor Plan", Icon::Grid),
];

/// One New-page gallery card: icon over the caption, 2010 tile metrics.
/// The caption is a real Label so the card stays in the accessibility
/// tree (kittest queries it); all colors ride theme tokens (#55).
fn backstage_card(ui: &mut Ui, icon: Icon, name: &str, enabled: bool) -> bool {
    ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
        let (tile, _) = ui.allocate_exact_size(Vec2::new(120.0, 96.0), Sense::hover());
        ui.painter().rect_filled(tile, 3.0, theme::CARD_BG);
        ui.painter().rect_stroke(
            tile,
            3.0,
            Stroke::new(1.0, theme::RIBBON_GROUP),
            egui::StrokeKind::Middle,
        );
        icon.paint(
            ui,
            Rect::from_center_size(tile.center(), Vec2::splat(44.0)),
            enabled,
        );
        let tip = if enabled {
            "Start an empty drawing"
        } else {
            "Template gallery arrives with the M5 template flow"
        };
        let caption = ui
            .add(
                egui::Label::new(
                    egui::RichText::new(name)
                        .size(12.0)
                        .color(theme::BACK_TITLE),
                )
                .selectable(false),
            )
            .on_hover_text(tip);
        let card = tile.union(caption.rect);
        let resp = ui.interact(card, ui.id().with(("backstage_card", name)), Sense::click());
        if resp.hovered() && enabled {
            ui.painter().rect_stroke(
                card.shrink(1.0),
                3.0,
                Stroke::new(1.5, theme::RIBBON_HOVER_EDGE),
                egui::StrokeKind::Middle,
            );
        }
        enabled && resp.clicked()
    })
    .inner
}

/// elementary-style welcome action: icon beside a bold title with a
/// one-line description — one button keeps it one accessible widget.
fn welcome_action(ui: &mut Ui, icon: Icon, title: &str, desc: &str) -> bool {
    let mut job = egui::text::LayoutJob::default();
    job.append(
        &format!("    {title}"),
        0.0,
        egui::TextFormat::simple(egui::FontId::proportional(13.0), theme::BACK_TITLE),
    );
    job.append(
        &format!("\n    {desc}"),
        0.0,
        egui::TextFormat::simple(egui::FontId::proportional(10.5), theme::RIBBON_LABEL),
    );
    let resp = ui.add(egui::Button::new(job).min_size(Vec2::new(280.0, 44.0)));
    let r = resp.rect;
    icon.paint(
        ui,
        Rect::from_center_size(Pos2::new(r.left() + 16.0, r.center().y), Vec2::splat(20.0)),
        true,
    );
    resp.clicked()
}

/// A labelled placeholder page: context sentence plus a clearly-named
/// disabled action — no dead ends (elementary), no vague labels.
fn backstage_stub(ui: &mut Ui, title: &str, context: &str, action: &str, tip: &str) {
    ui.heading(title);
    ui.add_space(6.0);
    ui.weak(context);
    ui.add_space(10.0);
    ui.add_enabled(false, egui::Button::new(action))
        .on_disabled_hover_text(tip);
}

/// Encode an egui ColorImage as PNG (via the raster stack we already ship).
/// One context-menu row: full-width flat button, optional MDI icon on the
/// left, a gray shortcut hint after the label (Visio shows it right-
/// aligned; a single text run keeps the row one accessible widget).
/// Label and hint colors come from the theme mapping over the live
/// visuals, so rows stay readable in dark mode (GitHub #48).
fn ctx_item(
    ui: &mut Ui,
    icon: Option<crate::ribbon::Icon>,
    label: &str,
    hint: &str,
    enabled: bool,
) -> bool {
    let tokens = theme::menu_tokens(ui.visuals());
    let mut job = egui::text::LayoutJob::default();
    let text = if icon.is_some() {
        format!("      {label}")
    } else {
        format!("  {label}")
    };
    job.append(
        &text,
        0.0,
        egui::TextFormat::simple(
            egui::FontId::proportional(12.5),
            if enabled {
                tokens.label
            } else {
                tokens.label_disabled
            },
        ),
    );
    if !hint.is_empty() {
        job.append(
            &format!("   {hint}"),
            0.0,
            egui::TextFormat::simple(egui::FontId::proportional(11.0), tokens.hint),
        );
    }
    let resp = ui.add_enabled(
        enabled,
        egui::Button::new(job).min_size(Vec2::new(ui.available_width().max(150.0), 21.0)),
    );
    if let Some(ic) = icon {
        let r = Rect::from_min_size(
            Pos2::new(resp.rect.left() + 7.0, resp.rect.center().y - 8.0),
            Vec2::splat(16.0),
        );
        // icons take the theme-derived tint: the light Office palette
        // (the gray ShapeSheet glyph included) vanishes on a dark menu
        // (GitHub #48 review).
        ic.paint_tinted(ui, r, tokens.icon_tint(ic.office_color(), enabled));
    }
    resp.clicked()
}

fn write_png(path: &std::path::Path, img: &egui::ColorImage) -> anyhow::Result<()> {
    let [w, h] = img.size;
    let mut rgba = Vec::with_capacity(w * h * 4);
    for p in &img.pixels {
        rgba.extend_from_slice(p.to_srgba_unmultiplied().as_slice());
    }
    let pm = resvg::tiny_skia::Pixmap::from_vec(
        rgba,
        resvg::tiny_skia::IntSize::from_wh(w as u32, h as u32).unwrap(),
    )
    .ok_or_else(|| anyhow::anyhow!("pixmap from frame"))?;
    std::fs::write(path, pm.encode_png()?)?;
    Ok(())
}

fn qat_button(ui: &mut Ui, icon: Icon, tip: &str, action: impl FnOnce() -> Action, app: &mut App) {
    let (r, resp) = ui.allocate_exact_size(Vec2::new(22.0, 22.0), Sense::click());
    let resp = resp.on_hover_text(tip);
    let hover = resp.hovered();
    if hover {
        ui.painter()
            .rect_filled(r, 3.0, Color32::from_white_alpha(46));
    }
    icon.paint(
        ui,
        Rect::from_center_size(r.center(), Vec2::splat(18.0)),
        true,
    );
    if resp.clicked() {
        let a = action();
        app.dispatch(a);
    }
}

fn t_title(t: Tab) -> &'static str {
    match t {
        Tab::File => "File",
        Tab::Home => "Home",
        Tab::Insert => "Insert",
        Tab::Design => "Design",
        Tab::Process => "Process",
        Tab::Data => "Data",
        Tab::Review => "Review",
        Tab::View => "View",
        Tab::Developer => "Developer",
    }
}

/// A master's root-shape size in inches (for the drag ghost).
fn master_size(doc: &Document, id: i64) -> (f64, f64) {
    doc.master(id)
        .and_then(|m| {
            m.shapes.first().map(|s| {
                let g = |n: &str| s.sheet.cell(n).and_then(|c| c.numeric()).unwrap_or(1.0);
                (g("Width"), g("Height"))
            })
        })
        .unwrap_or((1.0, 1.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Dialogs must never block the UI thread; these tests drive the
    // polling half directly (no real native dialog is ever spawned).

    #[test]
    fn dialog_pending_keeps_polling() {
        let ctx = Context::default();
        let mut app = App::with_doc_opt(None);
        let (_tx, rx) = mpsc::channel::<Option<PathBuf>>();
        app.dialog = Some(PendingDialog::Open(rx));
        app.poll_dialog(&ctx);
        assert!(app.dialog.is_some(), "unanswered dialog stays pending");
    }

    #[test]
    fn dialog_answered_open_consumes_result() {
        let ctx = Context::default();
        let mut app = App::with_doc_opt(None);
        let (tx, rx) = mpsc::channel::<Option<PathBuf>>();
        tx.send(Some(PathBuf::from("/nonexistent/x.vsdx"))).unwrap();
        app.dialog = Some(PendingDialog::Open(rx));
        app.poll_dialog(&ctx);
        assert!(app.dialog.is_none(), "answered dialog is consumed");
        assert!(
            app.status.starts_with("open failed"),
            "status = {}",
            app.status
        );
    }

    #[test]
    fn dialog_cancelled_is_silent() {
        let ctx = Context::default();
        let mut app = App::with_doc_opt(None);
        let (tx, rx) = mpsc::channel::<Option<PathBuf>>();
        tx.send(None).unwrap();
        app.dialog = Some(PendingDialog::SaveAs(rx));
        app.poll_dialog(&ctx);
        assert!(app.dialog.is_none());
        assert!(!app.dirty);
    }

    #[test]
    fn new_doc_from_nothing_creates_blank() {
        let mut app = App::with_doc_opt(None);
        assert!(app.ed.is_none());
        app.do_new();
        let ed = app.ed.as_ref().expect("blank doc opened");
        assert_eq!(ed.doc.pages.len(), 1);
        assert_eq!(ed.doc.pages[0].name, "Page-1");
        assert!(!app.dirty);
        assert!(app.path.as_os_str().is_empty());
    }

    #[test]
    fn new_doc_with_dirty_state_prompts_instead_of_discarding() {
        let mut app = App::with_doc_opt(None);
        app.do_new();
        assert!(app.ed.is_some());
        app.dirty = true;
        app.do_new();
        assert!(app.confirm_new, "dirty doc must prompt, not discard");
        assert!(app.ed.is_some(), "document stays open while prompting");
        // Discard proceeds; Cancel keeps it.
        app.start_new();
        assert!(!app.confirm_new);
        assert!(!app.dirty);
    }

    // Recent-files (MRU) wiring: successful loads/saves record, failed
    // ones must not (GitHub #16).

    #[test]
    fn opening_a_file_records_it_in_recent() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("recent-src.vsdx");
        Editor::new_blank().save_as(&p).unwrap();
        let mut app = App::with_doc_opt(None);
        app.load(p.clone());
        assert_eq!(
            app.debug_recent(),
            &[p.clone()],
            "successful open records the path"
        );
        // Re-opening moves it to the (single) front.
        app.load(p.clone());
        assert_eq!(app.debug_recent(), &[p]);
    }

    #[test]
    fn failed_open_does_not_record() {
        let mut app = App::with_doc_opt(None);
        app.load(PathBuf::from("/nonexistent/missing.vsdx"));
        assert!(
            app.debug_recent().is_empty(),
            "a failed open must not record anything"
        );
    }

    #[test]
    fn saving_records_the_saved_path() {
        let tmp = tempfile::tempdir().unwrap();
        let mut app = App::with_blank();
        let p = tmp.path().join("saved.vsdx");
        app.save_to(&p);
        assert_eq!(
            app.debug_recent(),
            &[p],
            "successful save-as records the path"
        );
        assert!(!app.dirty, "save clears the dirty flag");
    }

    // EventDblClick classification (GitHub #20): OPENTEXTWIN and
    // DEFAULTEVENT open the text editor, no formula / No Formula keeps
    // Visio's default (edit text), everything else defers.

    #[test]
    fn dbl_click_classifier_opens_text_for_default_formulas() {
        for f in [
            None,
            Some(""),
            Some("   "),
            Some("No Formula"),
            Some("  no formula "),
            Some("OPENTEXTWIN()"),
            Some("opentextwin()"),
            Some("DEFAULTEVENT()"),
        ] {
            assert_eq!(
                classify_dbl_click(f),
                DblClick::OpenText,
                "formula {f:?} must open the text editor"
            );
        }
    }

    #[test]
    fn dbl_click_classifier_defers_other_formulas() {
        for f in [
            Some("RUNADDON(\"MyMacro\")"),
            Some("RUNMACRO(\"ThisDocument.m\")"),
            Some("GOTOPAGE(\"Page-2\")"),
            Some("DONOTHING()"),
            Some("OPENSHEETWINDOW()"),
        ] {
            match classify_dbl_click(f) {
                DblClick::Deferred(msg) => assert!(
                    msg.contains(f.unwrap()),
                    "deferral message names the formula: {msg}"
                ),
                other => panic!("formula {f:?} must defer, got {other:?}"),
            }
        }
    }

    /// A shape with `EventDblClick = RUNADDON(…)`: double-click reports
    /// the deferral and opens no editor; with no formula the editor opens.
    #[test]
    fn dbl_click_honors_the_event_cell() {
        fn app_with_event_formula(f: Option<&str>) -> (App, i64) {
            let mut app = App::with_blank();
            let page_id = app.page_id().unwrap();
            let mut sh = vsdx::model::Shape {
                id: 42,
                ..Default::default()
            };
            for (n, v) in [
                ("PinX", "4.0"),
                ("PinY", "5.5"),
                ("Width", "2.0"),
                ("Height", "1.0"),
            ] {
                sh.sheet.set_cell(n, vsdx::Cell::value(v));
            }
            if let Some(f) = f {
                sh.sheet.set_cell(
                    "EventDblClick",
                    vsdx::Cell {
                        f: Some(f.into()),
                        ..Default::default()
                    },
                );
            }
            app.apply(dc_core::commands::AddShape {
                page: page_id,
                parent: None,
                shape: sh,
            });
            (app, 42)
        }

        // No EventDblClick cell: Visio's default — open the editor.
        let (mut app, sid) = app_with_event_formula(None);
        app.on_canvas_dbl_click((4.0, 5.5));
        assert_eq!(
            app.text_edit,
            Some((sid, String::new())),
            "default opens the editor"
        );

        // RUNADDON: deferral status, no editor.
        let (mut app, _) = app_with_event_formula(Some("RUNADDON(\"x\")"));
        app.on_canvas_dbl_click((4.0, 5.5));
        assert!(app.text_edit.is_none(), "RUNADDON must not open the editor");
        assert!(
            app.status.contains("EventDblClick") && app.status.contains("RUNADDON"),
            "status explains the deferral: {}",
            app.status
        );

        // Double-click on empty page: nothing happens.
        let (mut app, _) = app_with_event_formula(None);
        app.on_canvas_dbl_click((0.1, 0.1));
        assert!(
            app.text_edit.is_none(),
            "empty page double-click is a no-op"
        );
    }

    /// A stencil placement consumes its click pair: the double-click
    /// completing it must not open an editor over the placed shape,
    /// while a later, separate double-click works normally.
    #[test]
    fn dbl_click_after_stencil_placement_defers_to_the_placement() {
        let mut app = App::with_blank();
        let master = app
            .ed
            .as_ref()
            .unwrap()
            .doc
            .masters
            .first()
            .map(|m| m.id)
            .unwrap();
        app.stencil.armed = Some(master);
        app.on_canvas_click((4.0, 5.0), false);
        assert_eq!(app.debug_shape_count(), 1, "armed master placed on click");
        assert!(app.placed_last_click);

        app.on_canvas_dbl_click((4.0, 5.0));
        assert!(
            app.text_edit.is_none(),
            "the placement's click pair must not open an editor"
        );
        assert_eq!(app.debug_shape_count(), 1, "exactly one placement");

        app.on_canvas_dbl_click((4.0, 5.0));
        assert!(
            app.text_edit.is_some(),
            "a later double-click on the placed shape opens the editor"
        );
    }
}

/// What double-clicking a shape should do, from its `EventDblClick`
/// formula (`F` attribute, resolved instance → master).
#[derive(Debug, PartialEq)]
enum DblClick {
    /// Open the shape's text window: `OPENTEXTWIN()`, `DEFAULTEVENT()`
    /// (Visio's default action for a shape with text is editing it), or
    /// no formula / `No Formula`.
    OpenText,
    /// A formula whose action belongs to a later milestone (`RUNADDON`,
    /// `RUNMACRO`, `GOTOPAGE`, `DONOTHING`, …): report, open nothing.
    Deferred(String),
}

/// Classify an `EventDblClick` formula string.
fn classify_dbl_click(formula: Option<&str>) -> DblClick {
    let f = formula.unwrap_or("").trim();
    // Compare the function head (everything before the argument list).
    let head = f
        .split('(')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_uppercase();
    if f.is_empty()
        || f.eq_ignore_ascii_case("No Formula")
        || head == "OPENTEXTWIN"
        || head == "DEFAULTEVENT"
    {
        DblClick::OpenText
    } else {
        DblClick::Deferred(format!(
            "EventDblClick '{f}' — macro and navigation actions arrive with a later milestone"
        ))
    }
}

/// Visio's display names for ShapeSheet sections.
fn section_display_name(n: &str) -> String {
    match n {
        "TextBlock" => "Text Block Format".into(),
        "XForm" => "Shape Transform".into(),
        "XForm1D" => "1-D Endpoints".into(),
        "Character" => "Character".into(),
        "Paragraph" => "Paragraph".into(),
        "Tabs" => "Tabs".into(),
        "RulerGrid" => "Ruler & Grid".into(),
        "Connection" => "Connection Points".into(),
        "Fill" => "Fill Format".into(),
        "Line" => "Line Format".into(),
        "TextXForm" => "Text Transform".into(),
        "Misc" => "Miscellaneous".into(),
        "PageLayout" => "Page Layout".into(),
        "PrintPageSetup" => "Print Setup".into(),
        "PagesXForm" => "Page Properties".into(),
        other => other.into(),
    }
}
