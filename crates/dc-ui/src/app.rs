//! The desktop application: shell assembly (QAT band, ribbon, stencil
//! pane, canvas, page tabs, status bar) and all interaction wiring over
//! the dc-core command layer.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use dc_core::commands::{
    AddPage, AddShape, DuplicatePage, DuplicateShape, MovePage, RemovePage, RemoveShape,
    RenamePage, ReorderShape,
};
use dc_core::scope::{shape_chain, sheet as scope_sheet, Scope};
use dc_core::{Editor, Macro, SetCell, SetRowCell, SetText};
use egui::{Color32, Context, Pos2, Rect, Sense, Stroke, Ui, Vec2};
use vsdx::model::{Document, Shape};

use crate::canvas::{self, Handle, HandleId, PageGeom, View};
use crate::cmd::MoveShape;
use dc_core::AddMasterInstance;
use crate::recent::RecentFiles;
use crate::ribbon::{big, grid, stack, Action, Icon, RibbonButton, Tab, TABS};
use crate::stencil::{MasterKey, Stencil, StencilId, StencilSet};
use crate::svgimg::TextureCache;
use crate::theme;

/// Draft row for the Hyperlinks dialog (#28): `edit_row` names the
/// instance row being edited (None = adding). `from_master` marks a
/// draft opened on a master-inherited row (#139): OK materializes the
/// row onto the instance sheet instead of replacing a local one.
#[derive(Clone)]
struct HyperlinkDraft {
    edit_row: Option<i64>,
    from_master: bool,
    address: String,
    sub: String,
    desc: String,
}

/// One hyperlink row for list display: its values, its row ix, and
/// whether it lives on the instance sheet or is inherited from the
/// shape's master (#139).
#[derive(Clone)]
struct HyperRow {
    ix: i64,
    inherited: bool,
    addr: String,
    sub: String,
    desc: String,
}

/// One master ShapeSheet Actions row, evaluated for THIS instance
/// (#25): label, SETF formula, and the live Checked/Disabled flags.
#[derive(Clone)]
struct MasterActionRow {
    menu: String,
    action: String,
    checked: bool,
    disabled: bool,
    begin_group: bool,
    flyout_child: bool,
    display_only: bool,
}

/// Per-page UI state parked while another page is active (#54):
/// selection (primary + multi) and the view position return as they
/// were left. View chrome (rulers, grid, units) stays app-level like
/// Visio, so it is excluded on restore.
#[derive(Clone)]
struct PageUiState {
    sel: Option<(i64, i64)>,
    sel_set: Vec<(i64, i64)>,
    view: View,
}

pub struct App {
    ed: Option<Editor>,
    path: PathBuf,
    dirty: bool,
    version: u64,
    view: View,
    page_idx: usize,
    /// Per-page UI state saved while another page is active (#54):
    /// selection and view position restore when the page comes back.
    page_states: std::collections::HashMap<i64, PageUiState>,
    /// Open page-tab context menu: (page index, anchor) (#54).
    tab_menu: Option<(usize, Pos2)>,
    /// Inline tab rename: (page id, draft) (#54).
    tab_rename: Option<(i64, String)>,
    /// Scroll the tab strip to the active tab once, after a switch
    /// (#54) — consumed by the next strip paint.
    tab_scroll_to_active: bool,
    /// The Reorder Pages dialog is open (#54, Visio mechanism).
    reorder_open: bool,
    /// The Hyperlinks dialog is open for this shape (#28).
    hyperlink_open: Option<i64>,
    /// Draft row being added/edited in the Hyperlinks dialog (#28).
    hyperlink_draft: Option<HyperlinkDraft>,
    /// The Comments popup is open for this shape (#29 rework: the
    /// Visio 2016 thread view — author header, edit/delete/reply).
    comment_target: Option<i64>,
    /// The reply / new-comment input body (#29).
    comment_body: String,
    /// Inline edit buffer: (annotation id, body) (#29).
    comment_edit: Option<(i64, String)>,
    /// Annotation visibility (Review ▸ Show Comments, #29).
    show_comments: bool,
    /// The Shape Data dialog is open for this shape (#30).
    shape_data_open: Option<i64>,
    /// The Format Shape pane is open (#31).
    format_pane_open: bool,
    /// Text buffers for the Format pane's numeric/text fields (#31).
    fmt_bufs: std::collections::HashMap<String, String>,
    /// Text buffers for the Shape Data string fields (#30) — commits
    /// on focus loss, one undo per change.
    prop_bufs: std::collections::HashMap<String, String>,
    /// The Shape Data "add choice" row is open for this variable-list
    /// (Type 4) Prop row (#30).
    prop_add: Option<String>,
    tab: Tab,
    sel: Option<(i64, i64)>, // (page id, shape id) — primary selection
    /// Multi-selection (marquee / shift-click); includes the primary.
    sel_set: Vec<(i64, i64)>,
    drag: Option<DragState>,
    /// Group-edit mode (GitHub #78): stack of open group ids, outermost
    /// first — `Open <name>` pushes, Esc/Close pops one level. View
    /// state only (never written into the document); validated at the
    /// top of `draw` so undo/ungroup/doc changes leave it coherent.
    group_edit: Vec<i64>,
    stencil: Stencil,
    /// The open-stencil set (Shapes pane, #63) — session state, never
    /// file content. Kept across document switches like Visio's
    /// workspace; the pane (`stencil`) renders it.
    stencils: StencilSet,
    /// Stencil panel content rect from the last frame (resize-handle
    /// target for the kittest re-wrap tests).
    stencil_panel_inner: Rect,
    tex: TextureCache,
    /// Background rasterization (#92): the parse/render jobs for page
    /// textures and stencil thumbnails run here; completions land in
    /// `draw`'s pool drain (see `svgimg::force_sync_raster_for_tests`
    /// for the kittest determinism seam).
    pool: crate::rasterpool::RasterPool,
    /// Per-shape canvas layers (#93): one quad per top-level shape
    /// subtree, composited in z-order; commits dirty only the touched
    /// shape (`dc_core::Touch` routed in `apply`/`do_undo`/`do_redo`).
    layers: crate::layers::ShapeLayers,
    /// Test-only: force the Info thumbnail's render to fail (#58).
    info_render_fail_for_test: bool,
    /// Live drag ghosts drawn on the last frame (#96, test hook).
    last_drag_ghosts: usize,
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
    /// Close decision pending: the "Save changes?" prompt a window
    /// close (✕ / Alt-F4) or File → Close must clear before a dirty
    /// document is lost (GitHub #65).
    confirm_close: Option<CloseTarget>,
    /// Close once the save-as dialog triggered by the close prompt's
    /// Save (an untitled document) completes; dropped when that dialog
    /// is cancelled — cancelling the save means staying open.
    close_after_save: Option<CloseTarget>,
    /// Test hook input: a close request as if the window's ✕ was
    /// clicked — kittest cannot synthesize a WM_CLOSE (GitHub #65).
    close_req_for_test: bool,
    /// A close was approved and issued. egui-winit turns
    /// `ViewportCommand::Close` into the NEXT frame's close-requested
    /// input (`egui-winit/src/lib.rs` `ViewportCommand::Close`), so
    /// that echo must pass through un-vetoed — otherwise "Don't Save"
    /// (document left dirty) resurrects the prompt forever (#65).
    closing: bool,
    /// Test seam for the native save-as dialog: when armed, the next
    /// SaveAs dialog resolves with the preset answer instead of
    /// opening rfd (headless kittest must not open GTK dialogs).
    /// `Some(None)` = the dialog is "cancelled".
    dialog_preset_for_test: Option<Option<PathBuf>>,
    /// Test observability (#65): viewport commands sent by the close
    /// path (kittest cannot read viewport output after a pass).
    debug_close_vetoes: u32,
    debug_close_issued: u32,
    /// Open shape context menu: (shape id, screen anchor, page-inches
    /// anchor y-up) — set by right-click, cleared on action/outside click.
    ctx_menu: Option<(i64, Pos2, (f64, f64))>,
    /// Last frame's context-menu rect in screen coords; None while the
    /// menu is closed (kept for the placement tests, issue #49).
    ctx_menu_rect: Option<egui::Rect>,
    /// In-app shape clipboard (Cut/Copy/Paste within DiagramCraft; rich
    /// OS-clipboard formats arrive with a later milestone).
    clipboard: Option<Shape>,
    /// In-place WYSIWYG text editor session (GitHub #22); the state
    /// and overlay live in `crate::textedit`, this is the shell glue.
    text_edit: Option<crate::textedit::TextEditSession>,
    /// The last canvas click placed a stencil master — a double-click
    /// completing that click pair must not open a text editor.
    placed_last_click: bool,
    /// Fill/Line color palette target (opened from the ribbon).
    color_pick: Option<ColorTarget>,
    /// Most-recently-used files (start screen + backstage Open page).
    /// Ephemeral in the headless constructors; `App::new` swaps in the
    /// persistent on-disk list.
    recent: RecentFiles,
    /// OS light/dark watcher (GitHub #52). Production only — the
    /// headless constructors leave it off so goldens are deterministic.
    os_theme: Option<crate::os_theme::OsThemeWatcher>,
}

/// Which cell a ribbon palette click targets.
#[derive(Clone, Copy, PartialEq)]
enum ColorTarget {
    Fill,
    FillBkgnd,
    Line,
}

enum PendingDialog {
    Open(Receiver<Option<PathBuf>>),
    SaveAs(Receiver<Option<PathBuf>>),
    /// "Open Existing Stencil…" (.vssx into the Shapes pane, #83).
    OpenStencil(Receiver<Option<PathBuf>>),
}

#[derive(Clone, Copy)]
enum PendingKind {
    Open,
    SaveAs,
    /// "Open Existing Stencil…" from the Shapes pane (#83).
    OpenStencil,
}

/// Backstage rail command.
#[derive(Clone, Copy)]
enum BackAction {
    Save,
    SaveAs,
    CloseDoc,
}

/// What a confirmed "Save changes?" prompt closes (GitHub #65): the
/// whole app (window ✕ / Alt-F4) or just the document (File → Close).
#[derive(Clone, Copy, PartialEq)]
enum CloseTarget {
    App,
    Doc,
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
    /// Other members of a multi-selection captured when the press began
    /// (the press on a selected member drags the whole group; a press on
    /// an unselected shape moves only that shape — GitHub #59).
    group: Vec<i64>,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, path: Option<PathBuf>) -> App {
        // OS light/dark (GitHub #52): detect the opening mode (gsettings
        // with the XDG portal as fallback; `detect` debug-prints the
        // source), install BOTH style slots and select it — then follow
        // runtime flips via a watcher thread. Linux only: elsewhere
        // `detect` yields System and eframe tracks the OS natively.
        let pref = crate::os_theme::detect();
        crate::theme::install_for(&cc.egui_ctx, pref);
        let mut app = App::with_doc_opt(None);
        #[cfg(target_os = "linux")]
        {
            app.os_theme = Some(crate::os_theme::OsThemeWatcher::spawn());
        }
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
            page_states: std::collections::HashMap::new(),
            tab_menu: None,
            tab_rename: None,
            tab_scroll_to_active: false,
            reorder_open: false,
            hyperlink_open: None,
            hyperlink_draft: None,
            comment_target: None,
            comment_body: String::new(),
            comment_edit: None,
            show_comments: true,
            shape_data_open: None,
            prop_bufs: std::collections::HashMap::new(),
            prop_add: None,
            format_pane_open: false,
            fmt_bufs: std::collections::HashMap::new(),
            tab: Tab::Home,
            sel: None,
            sel_set: Vec::new(),
            drag: None,
            group_edit: Vec::new(),
            stencil: Stencil::default(),
            stencils: StencilSet::default(),
            stencil_panel_inner: Rect::NOTHING,
            tex: TextureCache::default(),
            pool: crate::rasterpool::RasterPool::default(),
            layers: crate::layers::ShapeLayers::default(),
            info_render_fail_for_test: false,
            last_drag_ghosts: 0,
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
            confirm_close: None,
            close_after_save: None,
            close_req_for_test: false,
            closing: false,
            dialog_preset_for_test: None,
            debug_close_vetoes: 0,
            debug_close_issued: 0,
            ctx_menu: None,
            ctx_menu_rect: None,
            clipboard: None,
            text_edit: None,
            placed_last_click: false,
            color_pick: None,
            recent: RecentFiles::ephemeral(),
            os_theme: None,
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

    /// Test hooks for the close-prompt flow (GitHub #65). kittest
    /// cannot synthesize a WM_CLOSE or read viewport output, so the
    /// request is flagged in and the sent commands counted out.
    #[doc(hidden)]
    pub fn request_close_for_test(&mut self) {
        self.close_req_for_test = true;
    }
    #[doc(hidden)]
    pub fn mark_dirty_for_test(&mut self) {
        self.dirty = true;
    }
    /// kittest: what every editing command does — dirty + version bump
    /// (the Info thumbnail's cache key, #58).
    #[doc(hidden)]
    pub fn simulate_edit_for_test(&mut self) {
        self.dirty = true;
        self.version += 1;
    }
    /// kittest: force the Info thumbnail render to fail (#58).
    #[doc(hidden)]
    pub fn force_info_render_failure_for_test(&mut self, on: bool) {
        self.info_render_fail_for_test = on;
    }
    /// kittest: the Info thumbnail texture for the active page, if the
    /// Info page has drawn one (#58).
    #[doc(hidden)]
    pub fn info_thumb_tex_id_for_test(&self) -> Option<egui::TextureId> {
        let ed = self.ed.as_ref()?;
        let page = &ed.doc.pages[self.page_idx.min(ed.doc.pages.len().saturating_sub(1))];
        let w_in = page
            .sheet()
            .and_then(|s| s.cell("PageWidth"))
            .and_then(|c| c.numeric())
            .unwrap_or(8.5)
            .max(0.1);
        let key = format!("info-{}", page.part.replace('/', "_"));
        self.tex.id_for(&key, self.version, (192.0 / w_in) as f32)
    }
    #[doc(hidden)]
    pub fn arm_close_chain_for_test(&mut self) {
        self.close_after_save = Some(CloseTarget::App);
    }
    #[doc(hidden)]
    pub fn preset_save_as_for_test(&mut self, answer: Option<PathBuf>) {
        self.dialog_preset_for_test = Some(answer);
    }
    #[doc(hidden)]
    pub fn debug_close_prompt(&self) -> bool {
        self.confirm_close.is_some()
    }
    #[doc(hidden)]
    pub fn debug_close_chain_armed(&self) -> bool {
        self.close_after_save.is_some()
    }
    #[doc(hidden)]
    pub fn debug_doc_open(&self) -> bool {
        self.ed.is_some()
    }
    #[doc(hidden)]
    pub fn debug_dirty(&self) -> bool {
        self.dirty
    }
    #[doc(hidden)]
    pub fn debug_close_vetoes(&self) -> u32 {
        self.debug_close_vetoes
    }
    #[doc(hidden)]
    pub fn debug_close_issued(&self) -> u32 {
        self.debug_close_issued
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
        // Visio semantics: File→Open of a stencil lands it in the
        // Shapes pane, not the editor — the drawing stays untouched
        // (#83).
        if path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("vssx"))
        {
            self.open_stencil(path);
            return;
        }
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
                self.group_edit.clear();
                self.stencil.armed = None;
                self.tex.clear();
                // Per-shape layers are keyed by shape id, which
                // collides across documents (both number from 1), and
                // set_page() alone won't clear when the page ids match
                // — drop the old document's textures/bounds.
                self.layers.clear();
                self.page_states.clear();
                self.tab_menu = None;
                self.tab_rename = None;
                self.reorder_open = false;
                self.comment_target = None;
                self.comment_edit = None;
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
                Ok((label, touched)) => {
                    self.status = label;
                    self.dirty = true;
                    self.version += 1; // Info thumbnail (whole page)
                    self.layers.touch(&touched, self.page_id().unwrap_or(-1));
                }
                Err(e) => self.status = format!("{e:#}"),
            }
        }
    }

    /// Undo/redo selection restore (#136): instead of dropping the
    /// selection, map it through the command. The pre-command selection
    /// simply survives when its shapes are still on the page — undoing
    /// a move or resize keeps the shape selected (handles stay live),
    /// and the Format pane it drove stays up. When the selection died
    /// with the command — a delete cleared it, or an undone placement
    /// removed its shape — the command's membership touches that
    /// resolve again become the selection: undoing a delete restores
    /// *and selects* the shape (observed Visio 2016 behavior), redoing
    /// a placement lands it selected like a fresh placement. Clearing
    /// is the fallback when neither yields anything (page-structure
    /// edits, commands that only touched other pages).
    fn restore_selection_after_history(&mut self, touched: &dc_core::TouchList) {
        // Group-edit mode (#78) selects members of the open group and
        // defers nested handle semantics; a command's touched rollup is
        // top-level, and mapping it here would jump the selection out
        // of the open group. Keep the member selection and only prune
        // entries whose shapes no longer resolve.
        if !self.group_edit.is_empty() {
            let alive: Vec<(i64, i64)> = self
                .ed()
                .and_then(|ed| ed.doc.pages.get(self.page_idx))
                .map(|page| {
                    self.sel_set
                        .iter()
                        .copied()
                        .filter(|(_, s)| {
                            vsdx::model::find_shape_by_id(&page.shapes, *s).is_some()
                        })
                        .collect()
                })
                .unwrap_or_default();
            self.sel = match self.sel {
                Some(entry) if alive.contains(&entry) => Some(entry),
                _ => alive.last().copied(),
            };
            self.sel_set = alive;
            return;
        }
        let picked: Option<Vec<(i64, i64)>> = self.ed().and_then(|ed| {
            let page = ed
                .doc
                .pages
                .get(self.page_idx.min(ed.doc.pages.len().saturating_sub(1)))?;
            // The pre-command selection, pruned to entries that are
            // still TOP-LEVEL on the active page after the
            // inverse/apply. Top-level only (not find_shape_by_id,
            // which resolves nested members): page-mode selection is
            // top-level by construction, and a re-nesting inverse —
            // undo of an ungroup puts the selected member back inside
            // its group — must drop it, or handles and Delete would
            // act on a hidden nested shape (#136 review).
            let old_alive: Vec<(i64, i64)> = self
                .sel_set
                .iter()
                .copied()
                .filter(|(p, s)| *p == page.id && page.shapes.iter().any(|sh| sh.id == *s))
                .collect();
            if !old_alive.is_empty() {
                return Some(old_alive);
            }
            // Membership touches (shapes that entered or left the page)
            // that exist again, in page z-order — the same order a
            // marquee pick produces.
            let restored: std::collections::HashSet<i64> = touched
                .shapes
                .iter()
                .filter(|t| t.page == page.id && t.touch == dc_core::Touch::Membership)
                .map(|t| t.shape)
                .collect();
            Some(
                page.shapes
                    .iter()
                    .filter(|s| restored.contains(&s.id))
                    .map(|s| (page.id, s.id))
                    .collect(),
            )
        });
        let picked = picked.unwrap_or_default();
        if picked.is_empty() {
            self.set_selection(None);
            return;
        }
        // Primary: the pre-history primary when it survived; otherwise
        // the top-most survivor, matching a fresh marquee pick.
        self.sel = match self.sel {
            Some(entry) if picked.contains(&entry) => Some(entry),
            _ => Some(picked[picked.len() - 1]),
        };
        self.sel_set = picked;
    }

    fn do_undo(&mut self) {
        if let Some(ed) = self.ed_mut() {
            match ed.undo() {
                Ok(Some((l, touched))) => {
                    self.status = format!("undo: {l}");
                    self.dirty = true;
                    self.version += 1;
                    self.layers.touch(&touched, self.page_id().unwrap_or(-1));
                    self.restore_selection_after_history(&touched);
                }
                Ok(None) => self.status = "nothing to undo".into(),
                Err(e) => self.status = format!("{e:#}"),
            }
        }
    }

    /// Switch the active page (#54): selection and view position park
    /// under the outgoing page's ID and restore when it comes back;
    /// chrome (rulers/grid/units) is app-level and follows the live
    /// view, like Visio.
    fn switch_page(&mut self, to: usize) {
        let Some((cur_id, new_id)) = self.ed().and_then(|ed| {
            let n = ed.doc.pages.len();
            (to < n && to != self.page_idx)
                .then(|| ed.doc.pages.get(self.page_idx).map(|c| c.id))
                .flatten()
                .zip(ed.doc.pages.get(to).map(|p| p.id))
        }) else {
            return;
        };
        {
            self.page_states.insert(
                cur_id,
                PageUiState {
                    sel: self.sel,
                    sel_set: self.sel_set.clone(),
                    view: self.view.clone(),
                },
            );
        }
        self.page_idx = to;
        // Group-edit state does not carry across pages: shape ids
        // overlap across pages, and the per-frame coherence check only
        // drops levels that stop resolving to a Group — an id that
        // happens to be a group on the new page would keep the mode
        // alive with the wrong scope (#78).
        self.group_edit.clear();
        self.tab_scroll_to_active = true;
        match self.page_states.remove(&new_id) {
            Some(st) => {
                self.sel = st.sel;
                self.sel_set = st.sel_set;
                let (unit, grid, rulers) =
                    (self.view.unit, self.view.show_grid, self.view.show_rulers);
                self.view = st.view;
                self.view.unit = unit;
                self.view.show_grid = grid;
                self.view.show_rulers = rulers;
            }
            None => {
                self.set_selection(None);
                self.view.fit_pending = true;
            }
        }
    }

    /// Next sequential Visio page name for a new/duplicated tab.
    fn next_page_name(&self) -> String {
        let mut n = self.ed().map(|e| e.doc.pages.len()).unwrap_or(0) + 1;
        let taken = |self_: &Self, name: &str| {
            self_
                .ed()
                .map(|e| e.doc.pages.iter().any(|p| p.name == name))
                .unwrap_or(false)
        };
        while taken(self, &format!("Page-{n}")) {
            n += 1;
        }
        format!("Page-{n}")
    }

    /// Drop per-page state and selection that outlived their pages
    /// (delete) or no longer match the active page (#54).
    fn reconcile_page_state(&mut self) {
        let live: Option<std::collections::HashSet<i64>> = self
            .ed()
            .map(|e| e.doc.pages.iter().map(|p| p.id).collect());
        if let Some(live) = live {
            self.page_states.retain(|id, _| live.contains(id));
        }
        self.page_idx = self.page_idx.min(
            self.ed()
                .map(|e| e.doc.pages.len())
                .unwrap_or(0)
                .saturating_sub(1),
        );
        if let Some((p, _)) = self.sel {
            if Some(p) != self.page_id() {
                self.set_selection(None);
            }
        }
    }

    /// Tab context-menu commands (#54). Each goes through the command
    /// layer (undoable) and lands the user on the affected page.
    fn tab_cmd_insert(&mut self, after: usize) {
        let name = self.next_page_name();
        // #68: Visio's Insert Page copies the current page's setup —
        // seed the new sheet from the active page.
        let add = match self.page_id() {
            Some(id) => AddPage::seeded_from(name, id),
            None => AddPage::new(name),
        };
        self.apply(add);
        if let Some(n) = self.ed().map(|e| e.doc.pages.len()) {
            if n > 1 && after + 1 < n {
                self.apply(MovePage {
                    from: n - 1,
                    to: after + 1,
                });
            }
            self.page_idx = after + 1;
        }
        self.view.fit_pending = true;
        self.tab_scroll_to_active = true;
    }

    fn tab_cmd_duplicate(&mut self, idx: usize) {
        let Some(id) = self.ed().and_then(|e| e.doc.pages.get(idx)).map(|p| p.id) else {
            return;
        };
        let name = self.next_page_name();
        self.apply(DuplicatePage::new(id));
        self.apply(RenamePage::new(
            self.ed()
                .and_then(|e| e.doc.pages.get(idx + 1))
                .map(|p| p.id)
                .unwrap_or(id),
            name,
        ));
        self.page_idx = idx + 1;
        self.view.fit_pending = true;
        self.tab_scroll_to_active = true;
    }

    fn tab_cmd_delete(&mut self, idx: usize) {
        let Some(ed) = self.ed() else { return };
        if ed.doc.pages.len() < 2 {
            self.status = "the last page cannot be deleted".into();
            return;
        }
        let Some(id) = ed.doc.pages.get(idx).map(|p| p.id) else {
            return;
        };
        self.apply(RemovePage::new(id));
        self.reconcile_page_state();
        self.view.fit_pending = true;
    }

    fn tab_cmd_reorder(&mut self, idx: usize, to: usize) {
        let n = self.ed().map(|e| e.doc.pages.len()).unwrap_or(0);
        if idx >= n || to >= n {
            return;
        }
        let active = self.page_id();
        self.apply(MovePage { from: idx, to });
        // The ACTIVE page stays active wherever it lands (Visio's
        // reorder dialog never moves your view).
        if let Some(id) = active {
            if let Some(p) = self
                .ed()
                .and_then(|e| e.doc.pages.iter().position(|p| p.id == id))
            {
                self.page_idx = p;
            }
        }
        self.tab_scroll_to_active = true;
    }

    fn do_redo(&mut self) {
        if let Some(ed) = self.ed_mut() {
            match ed.redo() {
                Ok(Some((l, touched))) => {
                    self.status = format!("redo: {l}");
                    self.dirty = true;
                    self.version += 1;
                    self.layers.touch(&touched, self.page_id().unwrap_or(-1));
                    // Same selection mapping as undo (#136) — redo never
                    // touched the selection before, so a redo-of-delete
                    // could leave a selection pointing at a removed shape.
                    self.restore_selection_after_history(&touched);
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

    /// Open a `.vssx` into the Shapes pane (#83): dedupes by source
    /// path (re-opening expands and focuses the existing entry),
    /// reveals the entry, and notes the status. Parse failures surface
    /// as a status note, like other load failures.
    /// The user's My Shapes folder (Visio convention, owner direction
    /// 2026-10-27): `<home>/Documents/My Shapes`. Created on demand —
    /// the Open Existing Stencil dialog starts there (#105 will grow
    /// the auto-open default set out of it). None when no home
    /// directory is known (neither HOME nor USERPROFILE): the dialog
    /// then keeps its default start location and nothing is created.
    fn my_shapes_dir() -> Option<std::path::PathBuf> {
        let home = std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .map(std::path::PathBuf::from)?;
        let dir = my_shapes_path(Some(home));
        let _ = std::fs::create_dir_all(&dir);
        Some(dir)
    }

    fn open_stencil(&mut self, path: PathBuf) {
        let stem = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "stencil".into());
        match self.stencils.open_file(&path) {
            Ok((ix, existing)) => {
                self.stencil.reveal = Some(StencilId::Open(ix));
                self.status = if existing {
                    format!("stencil {stem} is already open — expanded")
                } else {
                    format!("opened stencil {stem}")
                };
            }
            Err(e) => self.status = format!("open stencil failed: {e:#}"),
        }
    }

    /// More Shapes ▸ category ▸ stencil: open a built-in catalog
    /// stencil, or close its entry when it is already open (the menu
    /// checkbox state, #87). Built-ins land in the pane exactly like
    /// file stencils (#83) — placement, thumbnails, search, and close
    /// all key off the open-stencil set.
    fn toggle_builtin(&mut self, slug: &'static str) {
        let Some(b) = crate::stencil::builtin::by_slug(slug) else {
            return;
        };
        let source = crate::stencil::builtin::source_path(b.slug);
        if let Some(ix) = self.stencils.position_for(&source) {
            let title = self
                .stencils
                .get(ix)
                .map(|st| st.title.clone())
                .unwrap_or_else(|| b.title.to_string());
            self.stencil.close_entry(&mut self.stencils, ix);
            self.status = format!("closed {title}");
            return;
        }
        // Which variant opens is the pane's session preference (#85
        // follow-up): US by default, the metric twin with "Metric
        // Stencils" ticked.
        let metric = self.stencil.metric_units;
        match vsdx::parse::parse_bytes(b.bytes_for(metric)) {
            Ok(doc) => {
                let (ix, _) = self.stencils.open_parsed(
                    doc,
                    source,
                    b.title.to_string(),
                    crate::stencil::Origin::Builtin,
                );
                self.stencil.reveal = Some(StencilId::Open(ix));
                self.status = if metric {
                    format!("opened {title} (metric)", title = b.title)
                } else {
                    format!("opened {title}", title = b.title)
                };
            }
            Err(e) => self.status = format!("built-in stencil failed to load: {e:#}"),
        }
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
        self.close_after_save = None;
        self.ed = Some(Editor::new_blank());
        self.path = PathBuf::new();
        self.dirty = false;
        self.version += 1;
        self.view = View::default();
        self.page_idx = 0;
        self.set_selection(None);
        self.group_edit.clear();
        self.stencil = Stencil::default();
        self.tex.clear();
        self.layers.clear();
        self.page_states.clear();
        self.tab_menu = None;
        self.tab_rename = None;
        self.reorder_open = false;
        self.comment_target = None;
        self.comment_edit = None;
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
        // The native dialog is untestable headless (rfd opens GTK);
        // an armed preset answers in its place (tests only).
        let preset = self.dialog_preset_for_test.take();
        std::thread::spawn(move || {
            let picked = match preset {
                Some(p) => p,
                None => match kind {
                    PendingKind::Open => rfd::FileDialog::new()
                        .add_filter("Visio (VSDX family)", &["vsdx", "vstx", "vssx"])
                        .pick_file(),
                    PendingKind::OpenStencil => {
                        let mut dlg =
                            rfd::FileDialog::new().add_filter("Visio stencil", &["vssx"]);
                        if let Some(dir) = Self::my_shapes_dir() {
                            dlg = dlg.set_directory(dir);
                        }
                        dlg.pick_file()
                    }
                    PendingKind::SaveAs => rfd::FileDialog::new()
                        .add_filter("Visio drawing", &["vsdx", "vstx", "vssx"])
                        .set_file_name(&default_name)
                        .save_file(),
                },
            };
            let _ = tx.send(picked);
        });
        self.dialog = Some(match kind {
            PendingKind::Open => PendingDialog::Open(rx),
            PendingKind::OpenStencil => PendingDialog::OpenStencil(rx),
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
            PendingDialog::Open(rx)
            | PendingDialog::SaveAs(rx)
            | PendingDialog::OpenStencil(rx) => match rx.try_recv() {
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
            // cancelled: drop any chained follow-up
            self.new_after_save = false;
            self.close_after_save = None;
            return;
        };
        match dialog {
            PendingDialog::Open(_) => self.load(path),
            PendingDialog::OpenStencil(_) => self.open_stencil(path),
            PendingDialog::SaveAs(_) => {
                self.save_to(&path);
                if self.new_after_save {
                    if self.dirty {
                        // save failed — keep the document open, drop the chain
                        self.new_after_save = false;
                    } else {
                        self.start_new();
                    }
                } else if let Some(target) = self.close_after_save.take() {
                    if !self.dirty {
                        // save succeeded — the close continues (GitHub #65)
                        self.finish_close(ctx, target);
                    }
                    // save failed — keep everything open
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

    /// AABB in page coords (inches, y-up): (x, y, w, h). Nested-aware
    /// (#78): a top-level shape keeps its plain local rect (its parent
    /// frame is the page); a group member maps its parent-local rect
    /// through the renderer's XForm chain, so frames and hit boxes
    /// land on the drawn pixels even under rotated/flipped groups.
    fn shape_rect(&self, doc: &Document, page_id: i64, s: &Shape) -> Option<(f64, f64, f64, f64)> {
        let Ok(chain) = shape_chain(doc, page_id, s.id) else {
            return Self::shape_rect_static(doc, page_id, s);
        };
        if chain.len() <= 1 {
            return Self::shape_rect_static(doc, page_id, s);
        }
        let page_h = doc
            .pages
            .iter()
            .find(|p| p.id == page_id)?
            .sheet()
            .and_then(|sh| sh.cell("PageHeight"))
            .and_then(|c| c.numeric())
            .unwrap_or(11.0);
        dc::svg::shape_page_rect(doc, &chain, page_h)
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
        // Group-edit mode (#78): only the innermost open group's
        // members are on the canvas — clicks resolve against them
        // (nested-aware rects), never falling through to page shapes.
        if let Some(g) = self.group_edit.last().copied() {
            let group = vsdx::model::find_shape_by_id(&page.shapes, g)?;
            let mut members = Vec::new();
            collect_members(&group.sub_shapes, &mut members);
            for m in members.iter().rev() {
                if let Some((x, y, w, h)) = self.shape_rect(&ed.doc, page.id, m) {
                    if hit_in_rect(page_pos, x, y, w, h) {
                        return Some(m.id);
                    }
                }
            }
            return None;
        }
        for shape in page.shapes.iter().rev() {
            if let Some((x, y, w, h)) = self.shape_rect(&ed.doc, page.id, shape) {
                if hit_in_rect(page_pos, x, y, w, h) {
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

    /// The grab handles for the primary selection (GitHub #59). A 1D
    /// shape (resolved `BeginX`/`EndX` — the XForm1D marker) gets a
    /// handle at each endpoint plus the midpoint; everything else gets
    /// the four resize corners of its bounding box.
    fn selection_handles(&self) -> Vec<Handle> {
        // Group-edit mode v1 (#78): resize/endpoint handles for nested
        // members are deferred — the drag-move and edit paths work,
        // but handle math through group transforms arrives separately.
        if !self.group_edit.is_empty() {
            return Vec::new();
        }
        let Some(ed) = self.ed() else {
            return Vec::new();
        };
        let Some((page_id, shape_id)) = self.sel else {
            return Vec::new();
        };
        let Some(page) = ed.doc.pages.iter().find(|p| p.id == page_id) else {
            return Vec::new();
        };
        let Some(sh) = vsdx::model::find_shape_by_id(&page.shapes, shape_id) else {
            return Vec::new();
        };
        let Ok(chain) = shape_chain(&ed.doc, page_id, shape_id) else {
            return Vec::new();
        };
        let get = |n: &str| ed.doc.resolve_cell(sh, &chain, n).and_then(|c| c.numeric());
        if let (Some(bx), Some(by), Some(ex), Some(ey)) =
            (get("BeginX"), get("BeginY"), get("EndX"), get("EndY"))
        {
            return vec![
                Handle {
                    id: HandleId::LineBegin,
                    pos: (bx, by),
                },
                Handle {
                    id: HandleId::LineEnd,
                    pos: (ex, ey),
                },
                Handle {
                    id: HandleId::LineMid,
                    pos: ((bx + ex) / 2.0, (by + ey) / 2.0),
                },
            ];
        }
        let Some(r) = self.sel_page_rect() else {
            return Vec::new();
        };
        let page_h = self.page_geom().h_in;
        // The rect is y-down page inches; handles are y-up.
        [
            (canvas::Corner::TopLeft, r.left(), r.top()),
            (canvas::Corner::TopRight, r.right(), r.top()),
            (canvas::Corner::BottomLeft, r.left(), r.bottom()),
            (canvas::Corner::BottomRight, r.right(), r.bottom()),
        ]
        .into_iter()
        .map(|(c, x, y)| Handle {
            id: HandleId::Corner(c),
            pos: (x as f64, page_h - y as f64),
        })
        .collect()
    }

    /// kittest: one shape's bbox in SCREEN coords (y-down) — follows
    /// the live pan/zoom, for drag tests that must press the shape
    /// where it actually is (#59).
    #[doc(hidden)]
    pub fn debug_shape_screen_rect_for_test(&self, shape_id: i64) -> Option<egui::Rect> {
        self.shape_screen_rect(shape_id)
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
                        self.begin_text_edit(s, draft);
                        self.status = format!("editing text of shape {s}");
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
        // Group-edit mode (#78): the marquee selects among the open
        // group's members only.
        let mut members = Vec::new();
        if let Some(g) = self.group_edit.last().copied() {
            if let Some(group) = vsdx::model::find_shape_by_id(&page.shapes, g) {
                collect_members(&group.sub_shapes, &mut members);
            }
        } else {
            members.extend(page.shapes.iter());
        }
        for shape in members {
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

    /// The resize/lock guard for one axis of a 2D resize (GitHub #59):
    /// Some(reason) when the size cell resolves to a formula (`GUARD(…)`,
    /// any function — a silent overwrite is never acceptable) or the
    /// Protection section locks it (`LockWidth`/`LockHeight`).
    fn resize_guard(&self, size_cell: &str, lock_cell: &str) -> Option<String> {
        let ed = self.ed()?;
        let (page_id, shape_id) = self.sel?;
        let page = ed.doc.pages.iter().find(|p| p.id == page_id)?;
        let sh = vsdx::model::find_shape_by_id(&page.shapes, shape_id)?;
        let chain = shape_chain(&ed.doc, page_id, shape_id).ok()?;
        // Effective size cell formula: instance-local first, then the
        // master's (a GUARD in the master protects instances too).
        let resolved = ed.doc.resolve_cell(sh, &chain, size_cell);
        if let Some(f) = resolved.as_ref().and_then(|c| c.f.as_deref()) {
            if f != "Inh" {
                return Some(format!("{size_cell} is formula-driven ({f})"));
            }
        }
        if protection_flag(&ed.doc, sh, &chain, lock_cell) {
            return Some(format!("{lock_cell} is set (Protection section)"));
        }
        None
    }

    /// Commit a corner-handle resize: the dragged corner moves by the
    /// page-inch delta (y-down); the opposite corner stays anchored and
    /// LocPin stays proportional (Visio's `Width*0.5` semantics).
    /// Guarded axes (formula or Protection lock) refuse with a status
    /// note instead of being overwritten (#59); the min-size clamp
    /// stays.
    fn apply_resize(&mut self, corner: canvas::Corner, dx: f64, dy: f64) {
        let Some((page_id, shape_id)) = self.sel else {
            return;
        };
        let (w_guard, h_guard) = (
            self.resize_guard("Width", "LockWidth"),
            self.resize_guard("Height", "LockHeight"),
        );
        if let (Some(w), Some(h)) = (&w_guard, &h_guard) {
            self.status = format!(
                "resize refused: {} and {} — shape {shape_id} is protected",
                w, h
            );
            return;
        }
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
                                                 // A guarded axis keeps its size; only the free axis changes.
        let nw = if w_guard.is_some() {
            w
        } else {
            (w + dw).max(0.05)
        };
        let nh = if h_guard.is_some() {
            h
        } else {
            (h + dh).max(0.05)
        };
        if (nw - w).abs() < 1e-9 && (nh - h).abs() < 1e-9 {
            self.status = "resize refused: shape is protected".into();
            return;
        }
        // The partial-guard note is written AFTER `apply`, which
        // overwrites the status with the command label.
        let guard_note = match (&w_guard, &h_guard) {
            (Some(g), _) => Some(format!("width not resized: {} (height only)", g)),
            (_, Some(g)) => Some(format!("height not resized: {} (width only)", g)),
            _ => None,
        };
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
        let mut cmds: Vec<Box<dyn dc_core::Command>> = Vec::new();
        if w_guard.is_none() {
            cmds.push(Box::new(SetCell::new(scope.clone(), "Width", cell(nw))));
            cmds.push(Box::new(SetCell::new(scope.clone(), "LocPinX", cell(nlx))));
        }
        if h_guard.is_none() {
            cmds.push(Box::new(SetCell::new(scope.clone(), "Height", cell(nh))));
            cmds.push(Box::new(SetCell::new(scope.clone(), "LocPinY", cell(nly))));
        }
        cmds.push(Box::new(SetCell::new(scope.clone(), "PinX", cell(npx))));
        cmds.push(Box::new(SetCell::new(scope, "PinY", cell(npy))));
        // Refresh the flattened group's inherited caches (sub-shape
        // XForms, Txt block, row coordinates) to the new size — Visio's
        // save-time refresh; without it a real-file group renders stale
        // inside the grown selection box (#59).
        cmds.extend(crate::cmd::group_cache_refresh(
            &ed.doc,
            page_id,
            shape_id,
            nw / w,
            nh / h,
        ));
        self.apply(Macro::new(format!("resize shape {shape_id}"), cmds));
        if let Some(note) = guard_note {
            self.status = note;
        }
    }

    /// Guard for a 1D endpoint/midpoint drag: refuse when any of the
    /// cells the drag rewrites resolves to a formula, or when the
    /// Protection section locks the endpoints (`LockBegin`/`LockEnd`).
    fn line_drag_guard(&self, cells: &[&str], locks: &[&str]) -> Option<String> {
        let ed = self.ed()?;
        let (page_id, shape_id) = self.sel?;
        let page = ed.doc.pages.iter().find(|p| p.id == page_id)?;
        let sh = vsdx::model::find_shape_by_id(&page.shapes, shape_id)?;
        let chain = shape_chain(&ed.doc, page_id, shape_id).ok()?;
        for name in cells {
            let resolved = ed.doc.resolve_cell(sh, &chain, name);
            if let Some(f) = resolved.as_ref().and_then(|c| c.f.as_deref()) {
                if f != "Inh" {
                    return Some(format!("{name} is formula-driven ({f})"));
                }
            }
        }
        for lock in locks {
            if protection_flag(&ed.doc, sh, &chain, lock) {
                return Some(format!("{lock} is set (Protection section)"));
            }
        }
        None
    }

    /// Drag one endpoint of a 1D shape to a new page position; the
    /// other endpoint stays anchored. Rewrites `BeginX`/`BeginY` (or
    /// `EndX`/`EndY`) plus the derived XForm cells the renderer reads
    /// (Width, Angle, Pin, LocPin) as consistent literals — one undo
    /// step (#59).
    fn apply_line_endpoint(&mut self, end: HandleId, to: (f64, f64)) {
        let Some((page_id, shape_id)) = self.sel else {
            return;
        };
        // Only the lock matching the dragged endpoint guards the drag
        // (Visio: LockBegin locks the begin point alone).
        let (guard_cells, lock): (Vec<&str>, &str) = match end {
            HandleId::LineBegin => (vec!["BeginX", "BeginY"], "LockBegin"),
            HandleId::LineEnd => (vec!["EndX", "EndY"], "LockEnd"),
            _ => return,
        };
        if let Some(reason) = self.line_drag_guard(&guard_cells, &[lock]) {
            self.status = format!("endpoint drag refused: {reason}");
            return;
        }
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
        let (Some(bx), Some(by), Some(ex), Some(ey)) =
            (get("BeginX"), get("BeginY"), get("EndX"), get("EndY"))
        else {
            return;
        };
        let w0 = get("Width").unwrap_or(1.0);
        let (a, b) = if end == HandleId::LineBegin {
            (to, (ex, ey))
        } else {
            ((bx, by), to)
        };
        let w = (a.0 - b.0).hypot(a.1 - b.1).max(1e-4);
        let angle = (b.1 - a.1).atan2(b.0 - a.0);
        let (pin, other) = ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
        let scope = Scope::Shape {
            page: page_id,
            shape: shape_id,
        };
        let cell = |v: f64| vsdx::Cell::value(format!("{v:.6}"));
        let mut cmds: Vec<Box<dyn dc_core::Command>> = vec![
            SetCell::new(scope.clone(), "BeginX", cell(a.0)),
            SetCell::new(scope.clone(), "BeginY", cell(a.1)),
            SetCell::new(scope.clone(), "EndX", cell(b.0)),
            SetCell::new(scope.clone(), "EndY", cell(b.1)),
        ]
        .into_iter()
        .map(|c| Box::new(c) as Box<dyn dc_core::Command>)
        .collect();
        // Derived cells: skip any that a formula owns (never overwrite a
        // guarded cell; the note is written after `apply`, which would
        // otherwise overwrite the status with the command label).
        let mut kept_note: Option<String> = None;
        for (name, v) in [
            ("Width", w),
            ("Angle", angle),
            ("PinX", pin),
            ("PinY", other),
            ("LocPinX", w / 2.0),
            ("LocPinY", 0.0),
        ] {
            let formula = ed
                .doc
                .resolve_cell(sh, &chain, name)
                .and_then(|c| c.f.clone())
                .filter(|f| f != "Inh");
            if formula.is_some() {
                kept_note = Some(format!(
                    "endpoint moved, but {name} is formula-driven and kept"
                ));
                continue;
            }
            cmds.push(Box::new(SetCell::new(scope.clone(), name, cell(v))));
        }
        // A 1D shape's cached row coordinates scale with its Width.
        cmds.extend(crate::cmd::group_cache_refresh(
            &ed.doc,
            page_id,
            shape_id,
            w / w0.max(1e-6),
            1.0,
        ));
        self.apply(Macro::new(
            format!("move line endpoint (shape {shape_id})"),
            cmds,
        ));
        if let Some(note) = kept_note {
            self.status = note;
        }
    }

    /// Drag a 1D shape's midpoint: the whole line translates, both
    /// endpoints keep their separation (Visio's straight-line midpoint
    /// handle). One undo step (#59).
    fn apply_line_move(&mut self, from: (f64, f64), to: (f64, f64)) {
        let Some((page_id, shape_id)) = self.sel else {
            return;
        };
        // Guard the endpoint cells only: real Visio 1D masters commonly
        // derive PinX/PinY through GUARD((BeginX+EndX)/2) — a formula
        // there must not refuse the drag, the cell is simply kept (the
        // endpoints carry the geometry).
        if let Some(reason) = self.line_drag_guard(
            &["BeginX", "BeginY", "EndX", "EndY"],
            &["LockBegin", "LockEnd"],
        ) {
            self.status = format!("line move refused: {reason}");
            return;
        }
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
        let Some(((bx, by), (ex, ey))) = get("BeginX")
            .zip(get("BeginY"))
            .zip(get("EndX").zip(get("EndY")))
        else {
            return;
        };
        let Some((px, py)) = get("PinX").zip(get("PinY")) else {
            return;
        };
        let (dx, dy) = (to.0 - from.0, to.1 - from.1);
        let scope = Scope::Shape {
            page: page_id,
            shape: shape_id,
        };
        let cell = |v: f64| vsdx::Cell::value(format!("{v:.6}"));
        // Pin cells are derived: keep any a formula owns instead of
        // overwriting the guard.
        let mut kept_note: Option<String> = None;
        let mut cmds: Vec<(&str, f64)> = vec![
            ("BeginX", bx + dx),
            ("BeginY", by + dy),
            ("EndX", ex + dx),
            ("EndY", ey + dy),
        ];
        for (name, v) in [("PinX", px + dx), ("PinY", py + dy)] {
            let formula = ed
                .doc
                .resolve_cell(sh, &chain, name)
                .and_then(|c| c.f.clone())
                .filter(|f| f != "Inh");
            if formula.is_some() {
                kept_note = Some(format!("line moved, but {name} is formula-driven and kept"));
            } else {
                cmds.push((name, v));
            }
        }
        let cmds: Vec<Box<dyn dc_core::Command>> = cmds
            .into_iter()
            .map(|(n, v)| {
                Box::new(SetCell::new(scope.clone(), n, cell(v))) as Box<dyn dc_core::Command>
            })
            .collect();
        self.apply(Macro::new(format!("move line shape {shape_id}"), cmds));
        if let Some(note) = kept_note {
            self.status = note;
        }
    }

    /// Apply a pre-boxed command (the multi-shape move paths build
    /// commands per member); wrapped in a one-step Macro so the status
    /// and history behave like every other apply.
    /// Shapes whose `User.msvSDContainerShape` names `container_id`
    /// (Visio's membership convention, #27).
    fn container_member_ids(&self, page_id: i64, container_id: i64) -> Vec<i64> {
        let Some(ed) = self.ed() else {
            return Vec::new();
        };
        let Some(page) = ed.doc.pages.iter().find(|p| p.id == page_id) else {
            return Vec::new();
        };
        page.shapes
            .iter()
            .filter(|s| {
                s.sheet.sections_named("User").iter().any(|sec| {
                    sec.named_row("msvSDContainerShape")
                        .and_then(|r| r.cell("Value"))
                        .and_then(|c| c.numeric())
                        == Some(container_id as f64)
                })
            })
            .map(|s| s.id)
            .collect()
    }

    /// Context-menu Container (#27): wrap the current selection's
    /// union bbox (padded) in a plain container shape at the back of
    /// the z-order; membership rides on the members. One undo step.
    fn wrap_selection_in_container(&mut self) {
        let Some(page_id) = self.page_id() else {
            return;
        };
        let members: Vec<i64> = self
            .sel_set
            .iter()
            .filter(|(p, _)| *p == page_id)
            .map(|(_, s)| *s)
            .collect();
        if members.is_empty() {
            return;
        }
        let Some(ed) = self.ed() else { return };
        let Some(page) = ed.doc.pages.iter().find(|p| p.id == page_id) else {
            return;
        };
        let mut x0 = f64::MAX;
        let mut y0 = f64::MAX;
        let mut x1 = f64::MIN;
        let mut y1 = f64::MIN;
        for sid in &members {
            let Some(sh) = vsdx::model::find_shape_by_id(&page.shapes, *sid) else {
                continue;
            };
            let Ok(chain) = shape_chain(&ed.doc, page_id, *sid) else {
                continue;
            };
            let g = |n: &str| {
                ed.doc
                    .resolve_cell(sh, &chain, n)
                    .and_then(|c| c.numeric())
                    .unwrap_or(0.0)
            };
            let (px, py) = (g("PinX"), g("PinY"));
            let (w, h) = (g("Width"), g("Height"));
            let (lx, ly) = (g("LocPinX"), g("LocPinY"));
            x0 = x0.min(px - lx);
            y0 = y0.min(py - ly);
            x1 = x1.max(px - lx + w);
            y1 = y1.max(py - ly + h);
        }
        if x0 > x1 {
            return;
        }
        const PAD: f64 = 0.25;
        let id = dc_core::commands::next_shape_id(&ed.doc, page_id);
        let rect = (x0 - PAD, y0 - PAD, x1 - x0 + 2.0 * PAD, y1 - y0 + 2.0 * PAD);
        self.apply(dc_core::commands::add_container(
            page_id,
            id,
            &members,
            rect,
            "Container",
        ));
        self.set_selection(Some((page_id, id)));
    }

    fn apply_boxed(&mut self, label: &str, cmd: Box<dyn dc_core::Command>) {
        self.apply(Macro::new(label, vec![cmd]));
    }

    /// The move command for one shape at a target pin: a 1D shape
    /// translates Begin/End/Pin together (a pin-only move would leave
    /// the endpoint cells stale — the handles would sit at the old
    /// positions and the next endpoint drag would snap the line back,
    /// CodeRabbit review of df4a562); 2D shapes move the pin alone.
    /// Page height in inches — the y-flip anchor of the XForm chains.
    fn page_height(ed: &Editor, page_id: i64) -> f64 {
        ed.doc
            .pages
            .iter()
            .find(|p| p.id == page_id)
            .and_then(|p| p.sheet())
            .and_then(|sh| sh.cell("PageHeight"))
            .and_then(|c| c.numeric())
            .unwrap_or(11.0)
    }

    fn move_cmd_for(
        &self,
        page_id: i64,
        sid: i64,
        new_pin: (f64, f64),
    ) -> Box<dyn dc_core::Command> {
        let Some(ed) = self.ed() else {
            return Box::new(MoveShape::new(page_id, sid, new_pin));
        };
        let Some(page) = ed.doc.pages.iter().find(|p| p.id == page_id) else {
            return Box::new(MoveShape::new(page_id, sid, new_pin));
        };
        let Some(sh) = vsdx::model::find_shape_by_id(&page.shapes, sid) else {
            return Box::new(MoveShape::new(page_id, sid, new_pin));
        };
        let Ok(chain) = shape_chain(&ed.doc, page_id, sid) else {
            return Box::new(MoveShape::new(page_id, sid, new_pin));
        };
        // A group member's Pin cells are PARENT-LOCAL (#78): convert
        // the page-space drag target into the member's parent frame
        // through the same XForm chain the renderer composes.
        let new_pin = if chain.len() >= 2 {
            let page_h = Self::page_height(ed, page_id);
            dc::svg::page_point_to_parent_local(&ed.doc, &chain, page_h, new_pin)
        } else {
            new_pin
        };
        let get = |n: &str| ed.doc.resolve_cell(sh, &chain, n).and_then(|c| c.numeric());
        let Some(((bx, by), (ex, ey), (px, py))) = get("BeginX")
            .zip(get("BeginY"))
            .zip(get("EndX").zip(get("EndY")))
            .zip(get("PinX").zip(get("PinY")))
            .map(|(((bx, by), (ex, ey)), (px, py))| ((bx, by), (ex, ey), (px, py)))
        else {
            return Box::new(MoveShape::new(page_id, sid, new_pin));
        };
        let (dx, dy) = (new_pin.0 - px, new_pin.1 - py);
        let scope = Scope::Shape {
            page: page_id,
            shape: sid,
        };
        let cell = |v: f64| vsdx::Cell::value(format!("{v:.6}"));
        // Real 1D masters commonly derive PinX/PinY through
        // GUARD((BeginX+EndX)/2) — a formula-driven pin must not be
        // literal-written (the command layer now refuses it, which
        // would abort the whole move Macro); the endpoints carry the
        // geometry, exactly as apply_line_move's guard documents.
        let formula_pinned = |n: &str| {
            dc_core::commands::effective_formula(&ed.doc, sh, &chain, n).is_some()
        };
        let cmds: Vec<Box<dyn dc_core::Command>> = [
            ("BeginX", bx + dx),
            ("BeginY", by + dy),
            ("EndX", ex + dx),
            ("EndY", ey + dy),
            ("PinX", px + dx),
            ("PinY", py + dy),
        ]
        .into_iter()
        .filter(|(n, _)| !matches!(*n, "PinX" | "PinY") || !formula_pinned(n))
        .map(|(n, v)| {
            Box::new(SetCell::new(scope.clone(), n, cell(v))) as Box<dyn dc_core::Command>
        })
        .collect();
        Box::new(Macro::new(format!("move line shape {sid}"), cmds))
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
                    // same delta, as one undo step. The group was
                    // captured at press time (members of the selection
                    // the press actually landed on). Each member moves
                    // through `move_cmd_for`: 1D members translate their
                    // endpoints too, not just the pin.
                    if d.group.is_empty() {
                        // A container drag carries its members (Visio,
                        // #27): one undo step.
                        let member_ids = self.container_member_ids(page_id, d.shape);
                        if member_ids.is_empty() {
                            let label = format!("move shape {}", d.shape);
                            let cmd = self.move_cmd_for(page_id, d.shape, d.last_pin);
                            self.apply_boxed(&label, cmd);
                        } else {
                            let (dx, dy) =
                                (d.last_pin.0 - d.start_pin.0, d.last_pin.1 - d.start_pin.1);
                            let mut cmds: Vec<Box<dyn dc_core::Command>> =
                                vec![self.move_cmd_for(page_id, d.shape, d.last_pin)];
                            if let Some(ed) = self.ed() {
                                let page = &ed.doc.pages[self.page_idx];
                                for sid in member_ids {
                                    let Ok(chain) = shape_chain(&ed.doc, page_id, sid) else {
                                        continue;
                                    };
                                    let Some(sh) = vsdx::model::find_shape_by_id(&page.shapes, sid)
                                    else {
                                        continue;
                                    };
                                    let g = |n: &str| {
                                        ed.doc.resolve_cell(sh, &chain, n).and_then(|c| c.numeric())
                                    };
                                    if let (Some(px), Some(py)) = (g("PinX"), g("PinY")) {
                                        // Member pins are parent-local
                                        // (#78): map to page space,
                                        // apply the page-space delta,
                                        // and let move_cmd_for convert
                                        // the target back — a raw
                                        // local+page sum misplaces
                                        // members of transformed
                                        // groups.
                                        let target = if chain.len() >= 2 {
                                            let page_h = Self::page_height(ed, page_id);
                                            dc::svg::parent_local_point_to_page(
                                                &ed.doc,
                                                &chain,
                                                page_h,
                                                (px, py),
                                            )
                                        } else {
                                            (px, py)
                                        };
                                        cmds.push(self.move_cmd_for(
                                            page_id,
                                            sid,
                                            (target.0 + dx, target.1 + dy),
                                        ));
                                    }
                                }
                            }
                            self.apply(Macro::new("move container and members", cmds));
                        }
                    } else {
                        let Some(ed) = self.ed() else { return };
                        let dx = d.last_pin.0 - d.start_pin.0;
                        let dy = d.last_pin.1 - d.start_pin.1;
                        let mut cmds: Vec<Box<dyn dc_core::Command>> =
                            vec![self.move_cmd_for(page_id, d.shape, d.last_pin)];
                        for sid in d.group {
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
                                // Parent-local pin → page space (see
                                // the container path above).
                                let target = if chain.len() >= 2 {
                                    let page_h = Self::page_height(ed, page_id);
                                    dc::svg::parent_local_point_to_page(
                                        &ed.doc,
                                        &chain,
                                        page_h,
                                        (px, py),
                                    )
                                } else {
                                    (px, py)
                                };
                                cmds.push(self.move_cmd_for(
                                    page_id,
                                    sid,
                                    (target.0 + dx, target.1 + dy),
                                ));
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

    fn on_canvas_click(&mut self, page_pos: (f64, f64), shift: bool, ctrl: bool) {
        self.placed_last_click = false;
        // Ctrl/Cmd+click follows the hit shape's first hyperlink
        // (Visio, #28) before any selection semantics.
        if ctrl {
            if let Some(hit) = self.shape_hit(page_pos) {
                if self.follow_first_hyperlink(hit) {
                    return;
                }
            }
        }
        // Stencil placement takes priority.
        if let Some(key) = self.stencil.armed {
            self.stencil.armed = None;
            self.ctx_menu = None;
            self.placed_last_click = true;
            if let Some(nid) = self.place_master(key, page_pos) {
                if let Some(page_id) = self.page_id() {
                    self.set_selection(Some((page_id, nid)));
                    self.sel_set = vec![(page_id, nid)];
                }
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
                self.set_selection(Some((page_id, shape_id)));
                self.status = format!("selected shape {shape_id}");
            }
            None => {
                self.set_selection(None);
            }
        }
    }

    /// Place the armed/carried master `key` at `page_pos` (inches).
    /// Returns the new shape id when a placement happened.
    ///
    /// Document masters place directly. A master from an open stencil
    /// is first copied into the drawing (Visio's first-use import,
    /// #84) and the instance placed referencing the document-local
    /// copy — one undo step removes both together.
    fn place_master(&mut self, key: MasterKey, page_pos: (f64, f64)) -> Option<i64> {
        let page_id = self.page_id()?;
        match key.0 {
            StencilId::Document => {
                let new_id = self
                    .ed
                    .as_ref()
                    .map(|ed| dc_core::commands::next_shape_id(&ed.doc, page_id));
                self.apply(AddMasterInstance::new(page_id, key.1, page_pos));
                new_id
            }
            StencilId::Open(ix) => {
                let src = self
                    .stencils
                    .get(ix)
                    .and_then(|st| st.doc.master(key.1))
                    .cloned();
                let Some(src) = src else {
                    self.stencil.armed = None;
                    return None;
                };
                let Some(ed) = self.ed.as_ref() else {
                    return None;
                };
                let new_id = dc_core::commands::next_shape_id(&ed.doc, page_id);
                let cmd = dc_core::place_master_from_stencil(&ed.doc, page_id, &src, page_pos);
                self.apply(cmd);
                Some(new_id)
            }
        }
    }

    /// Primary press on a shape (page inches, y-up), reported by the
    /// canvas on the press frame. Arms the move-drag — the click path
    /// cannot: egui's click-vs-drag disambiguation means a natural
    /// press-drag-release never fires `clicked()`, so arming there made
    /// moves no-ops (GitHub #59). Selection itself still happens on the
    /// release-as-click (`on_canvas_click`).
    fn on_canvas_press(&mut self, page_pos: (f64, f64)) {
        let Some(page_id) = self.page_id() else {
            return;
        };
        let Some(shape_id) = self.shape_hit(page_pos) else {
            return;
        };
        let Some(ed) = self.ed() else { return };
        let page = &ed.doc.pages[self.page_idx];
        let Some(s) = vsdx::model::find_shape_by_id(&page.shapes, shape_id) else {
            return;
        };
        let Ok(chain) = shape_chain(&ed.doc, page_id, shape_id) else {
            return;
        };
        let pin = ed
            .doc
            .resolve_cell(s, &chain, "PinX")
            .and_then(|c| c.numeric())
            .zip(
                ed.doc
                    .resolve_cell(s, &chain, "PinY")
                    .and_then(|c| c.numeric()),
            );
        let Some((px, py)) = pin else { return };
        // The drag tracks the pin in PAGE space: a nested member's Pin
        // cells are parent-local (#78), so map them through the XForm
        // chain — the gesture delta and move_cmd_for's page-space
        // targets must agree (a mixed-frame pin misplaces members of
        // translated/rotated groups).
        let (pin_px, pin_py) = if chain.len() >= 2 {
            let page_h = Self::page_height(ed, page_id);
            dc::svg::parent_local_point_to_page(&ed.doc, &chain, page_h, (px, py))
        } else {
            (px, py)
        };
        // Group semantics at press time: pressing a member of the
        // current multi-selection drags the whole set; pressing an
        // unselected shape moves only that shape (its release-click then
        // selects it solo).
        let group: Vec<i64> = if self.sel_set.contains(&(page_id, shape_id)) {
            self.sel_set
                .iter()
                .filter(|(p, s)| *p == page_id && *s != shape_id)
                .map(|(_, s)| *s)
                .collect()
        } else {
            Vec::new()
        };
        self.drag = Some(DragState {
            shape: shape_id,
            grab_off: (pin_px - page_pos.0, pin_py - page_pos.1),
            start_pin: (pin_px, pin_py),
            last_pin: (pin_px, pin_py),
            moved: false,
            group,
        });
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
                // the in-place editor run it.
                let draft = self.selected_text(shape_id).unwrap_or_default();
                self.begin_text_edit(shape_id, draft);
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
                // #68: like the tab menu's Insert Page, the new sheet
                // copies the active page's setup.
                let name = format!(
                    "Page-{}",
                    self.ed().map(|e| e.doc.pages.len() + 1).unwrap_or(1)
                );
                let add = match self.page_id() {
                    Some(id) => AddPage::seeded_from(name, id),
                    None => AddPage::new(name),
                };
                self.apply(add);
                if let Some(ed) = self.ed() {
                    self.page_idx = ed.doc.pages.len() - 1;
                    self.view.fit_pending = true;
                    self.tab_scroll_to_active = true;
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
            Action::ToggleComments => self.show_comments = !self.show_comments,
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
        // Visio 2016-style accent band (owner-selected warm gray,
// 2026-10-27), tiny QAT (Save/Undo/Redo + chevron) at
        // the far left, document title centered. The brand green band
        // stays green in dark mode (GitHub #52).
        let t = theme::shell_tokens(ui.visuals());
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
                    .fill(t.accent_band)
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
        // `tk`: the for-loop below binds `t` for each Tab.
        let tk = theme::shell_tokens(ui.visuals());
        egui::Panel::top("tab_strip")
            .frame(
                egui::Frame::new()
                    .fill(tk.ribbon_content)
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
                    // The painted text never reaches the accessibility
                    // tree on its own (GitHub #56).
                    fresp.widget_info(|| {
                        egui::WidgetInfo::labeled(egui::Role::Button, true, "File")
                    });
                    ui.painter().rect_filled(
                        fr,
                        egui::CornerRadius {
                            nw: 0,
                            ne: 0,
                            se: 2,
                            sw: 2,
                        },
                        tk.accent_band,
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
                                tk.accent_dark
                            } else {
                                tk.tab_strip_text
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
                                Stroke::new(2.0, tk.accent),
                            );
                        }
                        ui.add_space(6.0);
                    }
                    // Office's "Tell me what you want to do" search box.
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(6.0);
                        let (pr, presp) =
                            ui.allocate_exact_size(Vec2::new(196.0, 20.0), Sense::click());
                        // Painted placeholder, so name the control for
                        // screen readers (GitHub #56).
                        presp.widget_info(|| {
                            egui::WidgetInfo::labeled(egui::Role::Button, true, "Tell Me")
                        });
                        let hot = presp.hovered();
                        ui.painter().rect_filled(pr, 2, tk.tellme_bg);
                        ui.painter().rect_stroke(
                            pr,
                            2,
                            Stroke::new(1.0, if hot { tk.ruler_tick } else { tk.ribbon_edge }),
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
                            tk.hint_text,
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
                            RibbonButton::new(Icon::Comment, "Comments", Action::ToggleComments)
                                .selected(self.show_comments),
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
        let t = theme::shell_tokens(ui.visuals());
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
            if tabs.is_empty() {
                return; // no document: Visio shows no strip at all
            }
            // The Visio 2010 navigation cluster at the strip's left end:
            // << first, < previous, > next, >> last (page navigation, not
            // strip scrolling — owner-confirmed against the shipped
            // reference, #54).
            let n = tabs.len();
            let page_idx = self.page_idx.min(n.saturating_sub(1));
            let mut nav = |ui: &mut Ui, glyph: &str, tip: &str, enabled: bool, to: usize| {
                let b =
                    egui::Button::new(egui::RichText::new(glyph).size(10.0).color(if enabled {
                        t.tab_text_active
                    } else {
                        t.tab_text_inactive
                    }))
                    .small()
                    .fill(egui::Color32::TRANSPARENT)
                    .frame(false);
                let clicked = ui.add_enabled(enabled, b).on_hover_text(tip).clicked();
                if clicked {
                    self.switch_page(to);
                }
            };
            nav(ui, "<<", "First page", page_idx > 0, 0);
            nav(
                ui,
                "<",
                "Previous page",
                page_idx > 0,
                page_idx.saturating_sub(1),
            );
            nav(
                ui,
                ">",
                "Next page",
                page_idx + 1 < n,
                (page_idx + 1).min(n - 1),
            );
            nav(ui, ">>", "Last page", page_idx + 1 < n, n - 1);
            ui.separator();
            // Tabs fill the strip; overflow clips and the nav cluster /
            // wheel scrolling reach the rest (Visio shows no scrollbar).
            let scroll_to_active = self.tab_scroll_to_active;
            self.tab_scroll_to_active = false;
            egui::ScrollArea::horizontal()
                .id_salt("dc-page-tabs")
                .max_height(24.0)
                .auto_shrink([false, false])
                .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        for (i, (name, id)) in tabs.iter().enumerate() {
                            let active = i == self.page_idx;
                            // Inline rename (#54): the tab becomes an
                            // editor on double-click / Rename; Enter or
                            // focus loss commits, Escape cancels.
                            let resp = if self.tab_rename.as_ref().is_some_and(|(rid, _)| rid == id)
                            {
                                let r = ui.add_sized(
                                    [96.0, 18.0],
                                    egui::TextEdit::singleline(
                                        &mut self.tab_rename.as_mut().unwrap().1,
                                    )
                                    .font(egui::TextStyle::Small)
                                    .hint_text("Page name"),
                                );
                                let escaped = ui.input(|i| i.key_pressed(egui::Key::Escape));
                                let entered = ui.input(|i| i.key_pressed(egui::Key::Enter));
                                if escaped {
                                    self.tab_rename = None;
                                } else if entered || r.lost_focus() {
                                    let new = self
                                        .tab_rename
                                        .take()
                                        .map(|(_, d)| d.trim().to_string())
                                        .unwrap_or_default();
                                    if !new.is_empty() && new != *name {
                                        self.apply(RenamePage::new(*id, new));
                                    }
                                }
                                r
                            } else {
                                let txt = egui::RichText::new(name).size(11.5).color(if active {
                                    t.tab_text_active
                                } else {
                                    t.tab_text_inactive
                                });
                                let b = egui::Button::new(txt)
                                    .small()
                                    .fill(if active { t.tab_active } else { t.tabs_bg })
                                    .frame(true);
                                let resp =
                                    ui.add(b)
                                        .on_hover_text(format!("page {} (id {})", i + 1, id));
                                if active && scroll_to_active {
                                    resp.scroll_to_me(Some(egui::Align::Center));
                                }
                                resp
                            };
                            if resp.clicked() {
                                self.switch_page(i);
                            }
                            if resp.double_clicked() {
                                self.tab_rename = Some((*id, name.clone()));
                            }
                            if resp.secondary_clicked() {
                                self.tab_menu = Some((
                                    i,
                                    resp.interact_pointer_pos().unwrap_or(resp.rect.left_top()),
                                ));
                            }
                        }
                    });
                });
            // No "+" button: it duplicated the tab context menu's
            // Insert Page… and its rect overflowed the strip band so
            // clicks could miss (#70 — removed by owner decision
            // 2026-10-24). The ribbon's New Page button remains, as
            // does right-click ▸ Insert Page….
        });
    }

    fn status_bar(&mut self, ui: &mut Ui) {
        let t = theme::shell_tokens(ui.visuals());
        ui.horizontal_centered(|ui| {
            let mut left;
            if let Some(ed) = self.ed() {
                let geom = self.page_geom();
                // Visio names the active page in the status bar (#54),
                // e.g. "Page-2 of 4".
                let page_name = ed
                    .doc
                    .pages
                    .get(self.page_idx)
                    .map(|p| p.name.as_str())
                    .unwrap_or("Page");
                left = format!(
                    "{} of {}    {}",
                    page_name,
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
                            if let Ok(chain) = shape_chain(&ed.doc, page.id, sid) {
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
            ui.label(egui::RichText::new(left).color(t.status_text).size(11.0));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Visio status bar right end: − [zoom slider] + N%
                if self.ed.is_some() {
                    let pct = (self.view.zoom * 100.0).round() as u32;
                    ui.label(
                        egui::RichText::new(format!("{pct}%"))
                            .color(t.status_text)
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
                            .color(t.status_text)
                            .size(11.0),
                    );
                } else {
                    ui.label(egui::RichText::new("").size(11.0));
                }
            });
        });
    }

    /// Effective cell value of the primary selection (instance →
    /// master) as a trimmed string (#31).
    fn eff_cell_str(&self, name: &str) -> Option<String> {
        let (ed, (page_id, sid)) = (self.ed()?, self.sel?);
        let page = ed.doc.pages.iter().find(|p| p.id == page_id)?;
        let sh = vsdx::model::find_shape_by_id(&page.shapes, sid)?;
        let chain = shape_chain(&ed.doc, page_id, sid).ok()?;
        ed.doc
            .resolve_cell(sh, &chain, name)
            .and_then(|c| c.v.clone())
            .map(|v| v.trim().to_string())
    }

    fn eff_cell_num(&self, name: &str) -> Option<f64> {
        let (ed, (page_id, sid)) = (self.ed()?, self.sel?);
        let page = ed.doc.pages.iter().find(|p| p.id == page_id)?;
        let sh = vsdx::model::find_shape_by_id(&page.shapes, sid)?;
        let chain = shape_chain(&ed.doc, page_id, sid).ok()?;
        ed.doc
            .resolve_cell(sh, &chain, name)
            .and_then(|c| c.numeric())
    }

    /// Effective first-row cell of a section (Character/Paragraph, #31).
    fn eff_row_str(&self, section: &str, cell: &str) -> Option<String> {
        let (ed, (page_id, sid)) = (self.ed()?, self.sel?);
        let page = ed.doc.pages.iter().find(|p| p.id == page_id)?;
        let sh = vsdx::model::find_shape_by_id(&page.shapes, sid)?;
        let chain = shape_chain(&ed.doc, page_id, sid).ok()?;
        ed.doc
            .effective_sheet(sh, &chain)
            .sections_named(section)
            .first()
            .and_then(|sec| sec.rows.first())
            .and_then(|r| r.cell(cell))
            .and_then(|c| c.v.clone())
            .map(|v| v.trim().to_string())
    }

    /// Write one cell on every selected shape — one Macro, one undo
    /// step (#31 requirement 2).
    fn fmt_set_cell(&mut self, name: &str, value: &str) {
        let targets = self.sel_set.clone();
        let mut cmds: Vec<Box<dyn dc_core::Command>> = Vec::new();
        for (page_id, sid) in targets {
            cmds.push(Box::new(SetCell::new(
                Scope::Shape {
                    page: page_id,
                    shape: sid,
                },
                name,
                vsdx::Cell::value(value.to_owned()),
            )));
        }
        if !cmds.is_empty() {
            self.apply(Macro::new(format!("set {name}"), cmds));
        }
    }

    fn fmt_set_row(&mut self, section: &str, cell: &str, value: &str) {
        let targets = self.sel_set.clone();
        let mut cmds: Vec<Box<dyn dc_core::Command>> = Vec::new();
        for (page_id, sid) in targets {
            cmds.push(Box::new(dc_core::SetRowCell::new(
                Scope::Shape {
                    page: page_id,
                    shape: sid,
                },
                section,
                0,
                dc_core::commands::RowSel::Ix(0),
                cell,
                vsdx::Cell::value(value.to_owned()),
            )));
        }
        if !cmds.is_empty() {
            self.apply(Macro::new(format!("set {section}.{cell}"), cmds));
        }
    }

    /// A named-choice dropdown (the manual popup — the stock ComboBox
    /// glyph is missing from the pinned fonts). Returns the chosen
    /// value's key when the user picks one.
    fn fmt_dropdown(
        ui: &mut Ui,
        current: &str,
        key_of: &dyn Fn(&str) -> String,
        choices: &[(&str, &str)], // (key, display)
    ) -> Option<String> {
        let display = choices
            .iter()
            .find(|(k, _)| *k == current)
            .map(|(_, d)| d.to_string())
            .unwrap_or_else(|| current.to_string());
        let r = ui.button(format!("{display}  ›"));
        let mut picked = None;
        egui::Popup::menu(&r)
            .close_behavior(egui::PopupCloseBehavior::CloseOnClick)
            .show(|ui| {
                ui.set_min_width(130.0);
                for (k, d) in choices {
                    if ui.selectable_label(*k == current, *d).clicked() {
                        picked = Some(key_of(k));
                    }
                }
            });
        picked
    }

    /// The Format Shape pane (#31): Fill / Line / Text for the current
    /// selection, writing the same cells the renderer reads — WYSIWYG
    /// by construction. Values seed from the effective sheet; writes
    /// are instance-local overrides (never the master).
    fn format_pane(&mut self, ui: &mut Ui) {
        let t = theme::shell_tokens(ui.visuals());
        ui.set_min_width(200.0);
        ui.heading(
            egui::RichText::new("Format Shape")
                .size(13.0)
                .color(t.label),
        );
        ui.add_space(4.0);
        let mut bufs = std::mem::take(&mut self.fmt_bufs);
        let mut color_pick: Option<ColorTarget> = None;
        let mut set_cell: Option<(String, String)> = None;
        let mut set_row: Option<(String, String, String)> = None;

        egui::CollapsingHeader::new("Fill")
            .default_open(true)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Foreground");
                    if ui
                        .button(
                            self.eff_cell_str("FillForegnd")
                                .unwrap_or_else(|| "—".into()),
                        )
                        .clicked()
                    {
                        color_pick = Some(ColorTarget::Fill);
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Background");
                    if ui
                        .button(self.eff_cell_str("FillBkgnd").unwrap_or_else(|| "—".into()))
                        .clicked()
                    {
                        color_pick = Some(ColorTarget::FillBkgnd);
                    }
                });
                let cur = self
                    .eff_cell_str("FillPattern")
                    .unwrap_or_else(|| "1".into());
                if let Some(v) = Self::fmt_dropdown(
                    ui,
                    &cur,
                    &|k: &str| k.to_string(),
                    &[
                        ("0", "None"),
                        ("1", "Solid"),
                        ("2", "5%"),
                        ("3", "10%"),
                        ("4", "20%"),
                        ("5", "30%"),
                        ("6", "40%"),
                        ("24", "Gradient horiz."),
                        ("25", "Gradient vert."),
                        ("28", "Gradient radial"),
                    ],
                ) {
                    set_cell = Some(("FillPattern".into(), v));
                }
            });
        ui.add_space(2.0);
        egui::CollapsingHeader::new("Line")
            .default_open(true)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Color");
                    if ui
                        .button(self.eff_cell_str("LineColor").unwrap_or_else(|| "—".into()))
                        .clicked()
                    {
                        color_pick = Some(ColorTarget::Line);
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Weight");
                    let cur_pt = self.eff_cell_num("LineWeight").unwrap_or(0.0104) * 72.0;
                    let mut pt = cur_pt;
                    let r = ui.add(
                        egui::Slider::new(&mut pt, 0.25..=12.0)
                            .show_value(true)
                            .custom_formatter(|v, _| format!("{v:.2} pt")),
                    );
                    r.widget_info(|| {
                        egui::WidgetInfo::labeled(egui::Role::Slider, true, "Line weight")
                    });
                    if r.drag_stopped() && (pt - cur_pt).abs() > 1e-6 {
                        set_cell = Some(("LineWeight".into(), format!("{:.6}", pt / 72.0)));
                    }
                });
                let cur = self
                    .eff_cell_str("LinePattern")
                    .unwrap_or_else(|| "1".into());
                if let Some(v) = Self::fmt_dropdown(
                    ui,
                    &cur,
                    &|k: &str| k.to_string(),
                    &[
                        ("0", "None"),
                        ("1", "Solid"),
                        ("2", "Dotted"),
                        ("3", "Dash-dot"),
                        ("4", "Dashed"),
                        ("9", "Long dash"),
                    ],
                ) {
                    set_cell = Some(("LinePattern".into(), v));
                }
                ui.horizontal(|ui| {
                    ui.label("Rounding");
                    let cur = self.eff_cell_str("Rounding").unwrap_or_else(|| "0".into());
                    let buf = bufs.entry("rounding".into()).or_insert(cur);
                    let r = ui.add(egui::TextEdit::singleline(buf).desired_width(70.0));
                    r.widget_info(|| {
                        egui::WidgetInfo::labeled(egui::Role::TextInput, true, "Rounding")
                    });
                    if r.lost_focus() {
                        if let Ok(v) = buf.trim().parse::<f64>() {
                            set_cell = Some(("Rounding".into(), format!("{v:.6}")));
                        }
                        bufs.remove("rounding");
                    }
                });
            });
        ui.add_space(2.0);
        egui::CollapsingHeader::new("Text")
            .default_open(true)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Font");
                    let cur = self.eff_row_str("Character", "Font").unwrap_or_default();
                    let buf = bufs.entry("font".into()).or_insert(cur);
                    let r = ui.add(egui::TextEdit::singleline(buf).desired_width(120.0));
                    r.widget_info(|| {
                        egui::WidgetInfo::labeled(egui::Role::TextInput, true, "Font")
                    });
                    if r.lost_focus() {
                        let v = buf.trim().to_string();
                        if !v.is_empty() {
                            set_row = Some(("Character".into(), "Font".into(), v));
                        }
                        bufs.remove("font");
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Size (pt)");
                    let cur = self
                        .eff_row_str("Character", "Size")
                        .and_then(|v| v.parse::<f64>().ok())
                        .unwrap_or(10.0)
                        * 72.0;
                    let buf = bufs.entry("size".into()).or_insert(format!("{cur:.1}"));
                    let r = ui.add(egui::TextEdit::singleline(buf).desired_width(70.0));
                    r.widget_info(|| {
                        egui::WidgetInfo::labeled(egui::Role::TextInput, true, "Size (pt)")
                    });
                    if r.lost_focus() {
                        if let Ok(pt) = buf.trim().parse::<f64>() {
                            set_row = Some((
                                "Character".into(),
                                "Size".into(),
                                format!("{:.6}", pt / 72.0),
                            ));
                        }
                        bufs.remove("size");
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Style");
                    let bits = self
                        .eff_row_str("Character", "Style")
                        .and_then(|v| v.parse::<i64>().ok())
                        .unwrap_or(0);
                    let mut bold = bits & 1 != 0;
                    let mut italic = bits & 2 != 0;
                    let rb = ui.checkbox(&mut bold, "B");
                    rb.widget_info(|| {
                        egui::WidgetInfo::labeled(egui::Role::CheckBox, true, "Bold")
                    });
                    if rb.changed() {
                        set_row = Some((
                            "Character".into(),
                            "Style".into(),
                            format!("{}", (bits & !1) | bold as i64),
                        ));
                    }
                    let ri = ui.checkbox(&mut italic, "I");
                    ri.widget_info(|| {
                        egui::WidgetInfo::labeled(egui::Role::CheckBox, true, "Italic")
                    });
                    if ri.changed() {
                        set_row = Some((
                            "Character".into(),
                            "Style".into(),
                            format!("{}", (bits & !2) | ((italic as i64) * 2)),
                        ));
                    }
                });
                let cur = self
                    .eff_row_str("Paragraph", "HorzAlign")
                    .unwrap_or_else(|| "0".into());
                if let Some(v) = Self::fmt_dropdown(
                    ui,
                    &cur,
                    &|k: &str| k.to_string(),
                    &[("0", "Left"), ("1", "Center"), ("2", "Right")],
                ) {
                    set_row = Some(("Paragraph".into(), "HorzAlign".into(), v));
                }
            });

        self.fmt_bufs = bufs;
        if let Some(target) = color_pick {
            self.color_pick = Some(target);
        }
        if let Some((name, v)) = set_cell {
            self.fmt_set_cell(&name, &v);
        }
        if let Some((sec, cell, v)) = set_row {
            self.fmt_set_row(&sec, &cell, &v);
        }
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
        // Annotation visibility (#29): hide the Annotation layer's
        // members when the Review toggle is off.
        let mut hidden: Vec<i64> = Vec::new();
        if !self.show_comments {
            if let Some(ed) = self.ed.as_ref() {
                if let Some(page) = ed.doc.pages.get(self.page_idx) {
                    if let Ok(rows) = dc_core::layers::layer_rows(&ed.doc, page.id) {
                        for r in &rows {
                            if dc_core::layers::layer_name(r) == Some("Annotation") {
                                hidden.push(r.ix);
                            }
                        }
                    }
                }
            }
        }
        // Group-edit breadcrumb bar (#78): "Page ▸ <Group> ▸ …" with a
        // Close Group control — Visio shows the path in the group
        // window's title; v1 pins a slim bar above the canvas. Esc
        // pops one level (the same close semantics as Visio's window).
        if !self.group_edit.is_empty() {
            let names: Vec<String> = self
                .group_edit
                .iter()
                .map(|gid| {
                    self.ed()
                        .and_then(|ed| ed.doc.pages.get(self.page_idx))
                        .and_then(|p| vsdx::model::find_shape_by_id(&p.shapes, *gid))
                        .and_then(|s| s.name_u.clone())
                        .unwrap_or_else(|| format!("Group.{gid}"))
                })
                .collect();
            let mut pop = false;
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(format!("Page › {}", names.join(" › ")))
                        .small()
                        .color(ui.visuals().weak_text_color()),
                );
                if ui.small_button("× Close Group").clicked() {
                    pop = true;
                }
            });
            if pop {
                self.group_edit.pop();
            }
            // Esc pops one level (after the text editor and any open
            // menu had their chance — those consume the key first).
            if self.drag.is_none()
                && self.text_edit.is_none()
                && self.ctx_menu.is_none()
                && ui
                    .ctx()
                    .input(|i| i.key_pressed(egui::Key::Escape))
            {
                self.group_edit.pop();
            }
        }
        let geom = self.page_geom();
        let sel_rect = self.sel_page_rect();
        let extra_rects: Vec<Rect> = self.sel_extra_page_rects();
        // Live move feedback (#96): during a moved drag the dragged
        // shapes' static layers are suppressed and ghosted at the live
        // position — Visio moves the shape under the cursor for the
        // whole gesture, and a move ghost is a quad translation, never
        // a re-raster (the #93 O(0)-move guarantee holds).
        let mut drag_fx = canvas::DragFx::default();
        let mut hide_layers: Vec<i64> = Vec::new();
        let mut live_drag = false;
        if let Some(d) = self.drag.as_ref().filter(|d| d.moved) {
            if self.sel.is_some_and(|(p, _)| Some(p) == self.page_id()) {
                live_drag = true;
                // y-up pin delta → y-down bbox delta flips y.
                let (dx, dy) = (d.last_pin.0 - d.start_pin.0, d.last_pin.1 - d.start_pin.1);
                let mut ids: Vec<i64> = vec![d.shape];
                ids.extend(d.group.iter().copied());
                let ed = self.ed.as_ref().unwrap();
                let page = &ed.doc.pages[self.page_idx];
                for id in ids {
                    hide_layers.push(id);
                    match self.layers.quad_for(id) {
                        Some(q) => {
                            let bbox = (q.bbox.0 + dx, q.bbox.1 - dy, q.bbox.2, q.bbox.3);
                            drag_fx.ghosts.push(crate::layers::ShapeQuad { id, tex: q.tex, bbox });
                            // The primary carries its selection frame
                            // along (Visio keeps the outline on the
                            // moving shape).
                            if id == d.shape {
                                let tl = canvas::page_to_screen(
                                    bbox.0,
                                    geom.h_in - bbox.1,
                                    &self.view,
                                    &geom,
                                );
                                let br = canvas::page_to_screen(
                                    bbox.0 + bbox.2,
                                    geom.h_in - (bbox.1 + bbox.3),
                                    &self.view,
                                    &geom,
                                );
                                drag_fx.outlines.push(Rect::from_two_pos(tl, br));
                            }
                        }
                        None => {
                            // Texture still rasterizing (or a sub-shape
                            // selection): a dashed outline at the live
                            // position keeps the gesture visible.
                            if let Some(shape) =
                                vsdx::model::find_shape_by_id(&page.shapes, id)
                            {
                                // Nested-aware: a group-edit member
                                // maps its parent-local rect through
                                // the XForm chain to page space.
                                if let Some((x, y, w, h)) =
                                    self.shape_rect(&ed.doc, page.id, shape)
                                {
                                    let tl = canvas::page_to_screen(x + dx, y + h + dy, &self.view, &geom);
                                    let br = canvas::page_to_screen(x + w + dx, y + dy, &self.view, &geom);
                                    drag_fx.outlines.push(Rect::from_two_pos(tl, br));
                                }
                            }
                        }
                    }
                }
            }
        }
        self.last_drag_ghosts = drag_fx.ghosts.len();
        // The moved selection frame follows the gesture; static chrome
        // (frame, members, handles) hides while it moves (Visio look).
        let (sel_rect, extra_rects, handles) = if live_drag {
            (None, Vec::new(), Vec::new())
        } else {
            (sel_rect, extra_rects, self.selection_handles())
        };
        let armed = self.stencil.armed.is_some();
        // #90/#91/#92: the page SVG is built by the cache's miss
        // closure — only when the (part, version, ppi-bucket) texture
        // is actually missing — and the parse/render runs on the raster
        // pool. Until a first raster lands, `None` paints the page as
        // its natural white paper (canvas::show's None path); a render
        // failure surfaces as the error label (and is retried next
        // frame, since failures are not cached).
        // #93: per-shape layers replace the one-page texture. Each
        // top-level shape subtree is its own layer (same SVG backend
        // walk, viewBox scoped to its ink bounds), rasterized on the
        // #92 pool and composited in z-order; only the shapes a
        // command touched ever re-render. Group-edit mode (#78)
        // composites only the open group's layer — its subtree at page
        // positions through the normal group transforms, the same
        // content the scoped monolith drew. `render_page_with` stays
        // the export/print path (and the Info thumbnail's).
        let (mut quads, layer_status) = {
            let ed = self.ed.as_ref().unwrap();
            let page = &ed.doc.pages[self.page_idx];
            // Viewport (visible page rect, y-down inches) from the
            // last frame's canvas rect — whole page until it exists.
            let vp = if self.canvas_rect.is_positive() {
                let (x0, y0) =
                    canvas::screen_to_page(self.canvas_rect.left_top(), &self.view, &geom);
                let (x1, y1) =
                    canvas::screen_to_page(self.canvas_rect.right_bottom(), &self.view, &geom);
                let y0 = geom.h_in - y0; // → y-down viewBox space
                let y1 = geom.h_in - y1;
                (
                    x0.min(x1),
                    y0.min(y1),
                    (x1 - x0).abs(),
                    (y1 - y0).abs(),
                )
            } else {
                (0.0, 0.0, geom.w_in, geom.h_in)
            };
            // The compositing scope is the TOP-LEVEL group on the
            // group-edit stack: composite iterates page.shapes (top
            // level), so the innermost id of a nested stack ([G1, G2])
            // matches nothing and blanked the canvas. Hit-testing
            // stays scoped to the innermost group (shape_hit).
            self.layers.composite(
                ui.ctx(),
                &mut self.pool,
                &ed.doc,
                page,
                self.view.px_per_in(),
                vp,
                self.group_edit.first().copied(),
                &hidden,
                // While the in-place editor is open (#22) its shape's
                // layer renders without the text — the editor overlay
                // is the only copy on screen.
                self.text_edit.as_ref().map(|s| s.shape),
            )
        };
        if let Some(msg) = layer_status {
            self.status = msg;
        }
        quads.retain(|q| !hide_layers.contains(&q.id));
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
                    if hit_in_rect(pos, x, y, w, h) {
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
            &quads,
            sel_rect,
            &extra_rects,
            &handles,
            &drag_fx,
            armed,
            &hit,
            &mut mouse,
            // While the in-place text editor is open (#22) the canvas
            // is interaction-locked: a plain click ends the edit and
            // nothing else reports (no select/marquee/dbl-click).
            self.text_edit.is_some(),
        );
        self.mouse_page = mouse;
        self.canvas_rect = out.canvas_rect;
        if self.text_edit.is_some() {
            // Click anywhere on the canvas = click outside the text:
            // end the edit (commit; the editor decided Esc the same
            // way). No other canvas event is reported while locked.
            if out.click_page_pos.is_some() {
                self.end_text_edit(true);
            }
        } else {
            if let Some(pos) = out.dbl_click_page_pos {
                // The second click of the pair completes a double-click
                // instead of a plain click: the first click already ran its
                // select (and possible drag-start) semantics on its frame.
                self.on_canvas_dbl_click(pos);
            } else if let Some(pos) = out.click_page_pos {
                self.on_canvas_click(pos, out.shift_click, out.ctrl_click);
            }
            if let Some(pos) = out.press_page_pos {
                self.on_canvas_press(pos);
            }
            if let Some((x0, y0, x1, y1)) = out.marquee_done {
                self.apply_marquee(x0, y0, x1, y1);
            }
        }
        // Comment badges (#29 rework): a small bubble at the top-left
        // corner of every commented shape — the shipped 2016 indicator,
        // non-intrusive (an overlay, never part of the drawing), shown
        // while comments are visible; clicking opens the thread popup.
        if self.show_comments {
            let targets: Vec<i64> = self
                .ed()
                .map(|ed| {
                    let page =
                        &ed.doc.pages[self.page_idx.min(ed.doc.pages.len().saturating_sub(1))];
                    page.shapes
                        .iter()
                        .filter(|s| {
                            s.sheet
                                .sections_named("User")
                                .iter()
                                .find_map(|sec| sec.named_row("AnnotationRef"))
                                .and_then(|r| r.cell("Value"))
                                .and_then(|c| c.numeric())
                                .is_some_and(|v| page.shapes.iter().any(|t| t.id as f64 == v))
                        })
                        .filter_map(|s| {
                            s.sheet
                                .sections_named("User")
                                .iter()
                                .find_map(|sec| sec.named_row("AnnotationRef"))
                                .and_then(|r| r.cell("Value"))
                                .and_then(|c| c.numeric())
                                .map(|v| v as i64)
                        })
                        .collect::<Vec<i64>>()
                })
                .unwrap_or_default();
            // Distinct targets, badge per target.
            let mut seen = std::collections::HashSet::new();
            for target in targets {
                if !seen.insert(target) {
                    continue;
                }
                let count = self.comments_for(target).len();
                let Some(r) = self.shape_screen_rect(target) else {
                    continue;
                };
                let badge = Rect::from_min_size(
                    r.left_top() + Vec2::new(-9.0, -9.0),
                    Vec2::new(18.0, 15.0),
                );
                #[allow(deprecated)] // Id::new: one badge per shape, stable
                let resp = ui.interact(
                    badge,
                    egui::Id::new(("dc-comment-badge", target)),
                    Sense::click(),
                );
                resp.widget_info(|| {
                    egui::WidgetInfo::labeled(
                        egui::Role::Button,
                        true,
                        format!("Comments on shape {target} ({count})"),
                    )
                });
                let t = theme::shell_tokens(ui.visuals());
                ui.painter().rect_filled(badge, 4.0, t.accent_band);
                ui.painter().rect_stroke(
                    badge,
                    4.0,
                    Stroke::new(1.0, Color32::WHITE),
                    egui::StrokeKind::Middle,
                );
                crate::ribbon::Icon::Comment.paint(ui, badge, true);
                let gal = ui.painter().layout_no_wrap(
                    format!("{count}"),
                    egui::FontId::proportional(9.0),
                    Color32::WHITE,
                );
                ui.painter().galley(
                    Pos2::new(
                        badge.right() - gal.size().x + 2.0,
                        badge.bottom() - gal.size().y,
                    ),
                    gal,
                    Color32::WHITE,
                );
                if resp.clicked() {
                    self.comment_target = Some(target);
                    self.comment_body.clear();
                    self.comment_edit = None;
                }
            }
        }
        if let Some((id, start, end)) = out.handle_drag_done {
            // A press on a handle released without travel is just a
            // click: committing it would create no-op undo steps (or a
            // bogus "refused" status for an unprotected shape).
            let travel_px = (end.0 - start.0).hypot(end.1 - start.1) * self.view.px_per_in() as f64;
            if travel_px > 1.0 {
                match id {
                    HandleId::Corner(corner) => {
                        // y-down screen-orientation delta, as before.
                        let dx = end.0 - start.0;
                        let dy = start.1 - end.1;
                        self.apply_resize(corner, dx, dy);
                    }
                    HandleId::LineBegin | HandleId::LineEnd => {
                        // The endpoint moves by the drag DELTA from the
                        // handle's own position — the grab may start up
                        // to HANDLE_GRAB px away from the handle center,
                        // and snapping to the raw pointer would jump it.
                        if let Some(origin) = handles.iter().find(|h| h.id == id).map(|h| h.pos) {
                            self.apply_line_endpoint(
                                id,
                                (origin.0 + end.0 - start.0, origin.1 + end.1 - start.1),
                            );
                        }
                    }
                    HandleId::LineMid => {
                        self.apply_line_move(start, end);
                    }
                }
            }
        }
        if let Some(pos) = out.right_click_page_pos {
            // Visio: right-click selects the shape under the cursor and
            // opens its context menu (empty page = no shape menu yet).
            match self.shape_hit(pos) {
                Some(shape_id) => {
                    if let Some(p) = self.page_id() {
                        // Right-clicking a member of the current
                        // multi-selection keeps the selection (Visio's
                        // Group flow depends on it, #26); clicking
                        // outside it collapses to the hit shape.
                        if !self.sel_set.contains(&(p, shape_id)) {
                            self.set_selection(Some((p, shape_id)));
                        }
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
        // In-place WYSIWYG text editor overlay (GitHub #22): laid over
        // the canvas at the shape's text-block rect, styled from the
        // same resolved cells the renderer paints with. The entry zoom
        // (once) focuses the block; ending flows (Esc, click outside)
        // commit through the retired dialog's SetText path.
        if self.text_edit.is_some() {
            if let Some(sess) = self.text_edit.as_mut() {
                if let Some(ed) = self.ed.as_ref() {
                    if let Some(page) = ed.doc.pages.iter().find(|p| p.id == sess.page) {
                        if let Some(b) = dc::svg::text_block_page_rect(&ed.doc, page, sess.shape) {
                            sess.enter_zoom(&mut self.view, b, self.canvas_rect, &geom);
                        }
                    }
                }
            }
            let mut flow = crate::textedit::EditFlow::Active;
            if let Some(sess) = self.text_edit.as_mut() {
                if let Some(ed) = self.ed.as_ref() {
                    // The session's page, not the active tab: the edit
                    // can outlive a page switch (tabs sit outside the
                    // canvas's interaction lock), and shape ids collide
                    // across pages — indexing by page_idx would lay the
                    // overlay over the wrong shape's text block. No
                    // matching page: skip the overlay this frame (#22
                    // review).
                    if let Some(page) = ed.doc.pages.iter().find(|p| p.id == sess.page) {
                        flow = crate::textedit::show_overlay(
                            ui,
                            sess,
                            &ed.doc,
                            page,
                            &geom,
                            &self.view,
                        );
                    }
                }
            }
            match flow {
                crate::textedit::EditFlow::Commit => self.end_text_edit(true),
                crate::textedit::EditFlow::Abandon => self.end_text_edit(false),
                crate::textedit::EditFlow::Active => {}
            }
        }
        self.shape_context_menu(ui.ctx());
        self.tab_context_menu(ui.ctx());
    }

    /// The page-tab context menu (#54): Visio's tab affordances —
    /// insert, duplicate, delete (never the last page), rename, and
    /// The page-tab context menu (#54), copying the shipped Visio
    /// reference (owner screenshots 2026-10-24): Insert, Delete, Rename,
    /// Duplicate, then Page Setup (later milestone) and Reorder Pages….
    /// All live items run through the undoable command layer.
    #[allow(deprecated)] // Id::new: one tab menu, stable identity wanted
    fn tab_context_menu(&mut self, ctx: &egui::Context) {
        let Some((idx, anchor)) = self.tab_menu else {
            return;
        };
        let n = self.ed().map(|e| e.doc.pages.len()).unwrap_or(0);
        if idx >= n {
            self.tab_menu = None;
            return;
        }
        #[derive(Clone, Copy, PartialEq)]
        enum Act {
            Insert,
            Delete,
            Rename,
            Duplicate,
            Reorder,
        }
        let mut act: Option<Act> = None;
        let menu = egui::Popup::new(
            egui::Id::new("dc-tab-ctx-menu"),
            ctx.clone(),
            anchor,
            egui::LayerId::background(),
        )
        .kind(egui::PopupKind::Menu)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .width(232.0);
        let popup = menu.show(|ui| {
            ui.set_min_width(232.0);
            let tokens = theme::menu_tokens(ui.visuals());
            ui.visuals_mut().widgets.inactive.bg_fill = egui::Color32::TRANSPARENT;
            ui.visuals_mut().widgets.hovered.bg_fill = tokens.hover_fill;
            ui.visuals_mut().widgets.noninteractive.bg_stroke.color = tokens.separator;
            if ctx_item(ui, None, "Insert Page…", "", true) {
                act = Some(Act::Insert);
            }
            if ctx_item(
                ui,
                Some(crate::ribbon::Icon::Delete),
                "Delete Page",
                "",
                n > 1,
            ) {
                act = Some(Act::Delete);
            }
            if ctx_item(ui, None, "Rename Page", "", true) {
                act = Some(Act::Rename);
            }
            if ctx_item(ui, None, "Duplicate Page", "", true) {
                act = Some(Act::Duplicate);
            }
            ui.separator();
            ctx_item(ui, None, "Page Setup…", "(later milestone)", false);
            if ctx_item(ui, None, "Reorder Pages…", "", true) {
                act = Some(Act::Reorder);
            }
        });
        // Close on an outside primary click (the shape menu's idiom).
        let outside_primary = ctx.input(|i| i.pointer.primary_clicked());
        if let Some(ir) = &popup {
            if outside_primary
                && !ir
                    .response
                    .rect
                    .contains(ctx.input(|i| i.pointer.interact_pos()).unwrap_or_default())
            {
                self.tab_menu = None;
            }
        }
        let Some(a) = act else { return };
        self.tab_menu = None;
        match a {
            Act::Insert => self.tab_cmd_insert(idx),
            Act::Delete => self.tab_cmd_delete(idx),
            Act::Rename => {
                let id = self.ed().and_then(|e| e.doc.pages.get(idx)).map(|p| p.id);
                let name = self
                    .ed()
                    .and_then(|e| e.doc.pages.get(idx))
                    .map(|p| p.name.clone());
                if let (Some(id), Some(name)) = (id, name) {
                    self.tab_rename = Some((id, name));
                }
            }
            Act::Duplicate => self.tab_cmd_duplicate(idx),
            Act::Reorder => self.reorder_open = true,
        }
    }

    /// The shape's hyperlink rows (#28): instance rows first, then the
    /// master's (resolved like every other cell read). A master row
    /// whose ix an instance row already carries is SHADOWED — the
    /// local row is the override (first-edit materialization, #139) —
    /// so a materialized row replaces its inherited twin here instead
    /// of duplicating it.
    fn shape_hyperlinks(&self, shape_id: i64) -> Vec<(String, String, String)> {
        self.hyperlink_rows(shape_id)
            .into_iter()
            .map(|r| (r.addr, r.sub, r.desc))
            .collect()
    }

    /// The shape's hyperlink rows with their origin (#139): instance
    /// rows first, then the master's (shadowed ixs dropped). The
    /// master lookup walks its top-level shapes; a multi-shape master
    /// with same-ix Hyperlink rows across sub-shapes is beyond v1
    /// (the blank-catalog masters are single-shape).
    fn hyperlink_rows(&self, shape_id: i64) -> Vec<HyperRow> {
        let Some(ed) = self.ed() else {
            return Vec::new();
        };
        let Some(page) = ed.doc.pages.get(self.page_idx) else {
            return Vec::new();
        };
        let Some(sh) = vsdx::model::find_shape_by_id(&page.shapes, shape_id) else {
            return Vec::new();
        };
        let collect = |shapes: &[Shape]| {
            shapes
                .iter()
                .flat_map(|s| s.sheet.sections_named("Hyperlink"))
                .flat_map(|sec| sec.rows.iter())
                .map(|r| HyperRow {
                    ix: r.ix,
                    inherited: false,
                    addr: r.cell("Address").and_then(|c| c.v.clone()).unwrap_or_default(),
                    sub: r.cell("SubAddress").and_then(|c| c.v.clone()).unwrap_or_default(),
                    desc: r.cell("Description").and_then(|c| c.v.clone()).unwrap_or_default(),
                })
                .collect::<Vec<_>>()
        };
        let mut out = collect(std::slice::from_ref(sh));
        if let Some(m) = sh.master.and_then(|id| ed.doc.master(id)) {
            let shadowed: std::collections::HashSet<i64> =
                out.iter().map(|r| r.ix).collect();
            out.extend(
                collect(&m.shapes)
                    .into_iter()
                    .filter(|r| !shadowed.contains(&r.ix))
                    .map(|mut r| {
                        r.inherited = true;
                        r
                    }),
            );
        }
        out
    }

    /// Follow a hyperlink (#28): a SubAddress naming a page navigates
    /// the document; anything else opens through the OS (fire-and-
    /// forget). Returns whether a link existed.
    fn follow_first_hyperlink(&mut self, shape_id: i64) -> bool {
        let Some((addr, sub, _)) = self.shape_hyperlinks(shape_id).into_iter().next() else {
            return false;
        };
        self.follow_hyperlink(&addr, &sub);
        true
    }

    fn follow_hyperlink(&mut self, address: &str, sub: &str) {
        let address = address.trim();
        let sub = sub.trim();
        if !sub.is_empty() {
            // GOTOPAGE-class navigation: SubAddress names a page.
            let target = self
                .ed()
                .and_then(|ed| ed.doc.pages.iter().position(|p| p.name == sub));
            match target {
                Some(i) => {
                    self.switch_page(i);
                    self.status = format!("went to page {sub:?}");
                    return;
                }
                None => {
                    self.status = format!("page {sub:?} not found");
                    return;
                }
            }
        }
        if address.is_empty() {
            self.status = "the link has no address".into();
            return;
        }
        #[cfg(target_os = "linux")]
        let open = "xdg-open";
        #[cfg(target_os = "macos")]
        let open = "open";
        #[cfg(target_os = "windows")]
        let open = "cmd";
        #[cfg(target_os = "windows")]
        let flag = "/C";
        #[cfg(not(target_os = "windows"))]
        let flag = "";
        let mut cmd = std::process::Command::new(open);
        if !flag.is_empty() {
            cmd.arg(flag);
        }
        match cmd.arg(address).spawn() {
            Ok(_) => self.status = format!("opened {address}"),
            Err(e) => self.status = format!("could not open {address}: {e}"),
        }
    }

    /// The Hyperlinks dialog (#28): rows of the selected shape with
    /// add / edit / remove through the command layer (one undo per
    /// change) and Go to follow a link. The draft lives OUTSIDE the
    /// window closure (a per-frame clone would drop typed text — the
    /// retired text-edit-window pattern).
    fn add_comment(&mut self, shape_id: i64, body: &str) {
        let Some(page_id) = self.page_id() else {
            return;
        };
        let Some(ed) = self.ed() else { return };
        let Some(page) = ed.doc.pages.iter().find(|p| p.id == page_id) else {
            return;
        };
        let Some(sh) = vsdx::model::find_shape_by_id(&page.shapes, shape_id) else {
            return;
        };
        let Ok(chain) = shape_chain(&ed.doc, page_id, shape_id) else {
            return;
        };
        let g = |n: &str| {
            ed.doc
                .resolve_cell(sh, &chain, n)
                .and_then(|c| c.numeric())
                .unwrap_or(0.0)
        };
        let (px, py) = (g("PinX"), g("PinY"));
        let (w, h) = (g("Width"), g("Height"));
        let (lx, ly) = (g("LocPinX"), g("LocPinY"));
        let (x1, y1) = (px - lx + w, py - ly + h);

        // The Annotation layer, created on first use; LayerMember
        // references the layer row's IX (Visio's convention).
        let rows = dc_core::layers::layer_rows(&ed.doc, page_id).unwrap_or_default();
        let (layer_cmd, layer_ix) = match rows
            .iter()
            .find(|r| dc_core::layers::layer_name(r) == Some("Annotation"))
        {
            Some(r) => (None, r.ix),
            None => (
                Some(dc_core::commands::AddLayer::new(page_id, "Annotation")),
                rows.iter().map(|r| r.ix).max().unwrap_or(-1) + 1,
            ),
        };
        let id = dc_core::commands::next_shape_id(&ed.doc, page_id);
        let author = std::env::var("USER")
            .or_else(|_| std::env::var("USERNAME"))
            .unwrap_or_else(|_| "user".into());
        let date = format_now_utc();
        let text = format!("{} — {}: {}", author, date, body.trim());
        // Replies stack under the thread's first bubble (#29 rework).
        let thread_len = self.comments_for(shape_id).len();
        let stack = 0.55 * thread_len as f64;

        let set = |sheet: &mut vsdx::model::Sheet, n: &str, v: &str| {
            sheet.set_cell(n, vsdx::model::Cell::value(v.to_string()));
        };
        let (aw, ah) = (1.8_f64, 0.5_f64);
        let mut sheet = vsdx::model::Sheet::default();
        set(&mut sheet, "PinX", &format!("{:.4}", x1 + 0.1));
        set(
            &mut sheet,
            "PinY",
            &format!("{:.4}", ((y1 + 0.1) - stack).max(ah)),
        );
        set(&mut sheet, "Width", &format!("{aw}"));
        set(&mut sheet, "Height", &format!("{ah}"));
        set(&mut sheet, "LocPinX", "0");
        set(&mut sheet, "LocPinY", "0");
        set(&mut sheet, "LayerMember", &layer_ix.to_string());
        set(&mut sheet, "FillForegnd", "#FFF3C2");
        sheet.text = Some(text.clone());
        sheet.text_raw = Some(
            text.replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;"),
        );
        let row = |ix: i64, t: &str, x: &str, y: &str| vsdx::model::Row {
            ix,
            t: Some(t.into()),
            cells: vec![
                ("X".into(), vsdx::model::Cell::value(x.to_string())),
                ("Y".into(), vsdx::model::Cell::value(y.to_string())),
            ],
            ..vsdx::model::Row::default()
        };
        sheet
            .children
            .push(vsdx::model::SheetChild::Section(vsdx::model::Section {
                name: "Geometry".into(),
                ix: 0,
                rows: vec![
                    row(1, "MoveTo", "0", "0"),
                    row(2, "LineTo", &format!("{aw}"), "0"),
                    row(3, "LineTo", &format!("{aw}"), &format!("{ah}")),
                    row(4, "LineTo", "0", &format!("{ah}")),
                    row(5, "LineTo", "0", "0"),
                ],
                ..vsdx::model::Section::default()
            }));
        let annotation = Shape {
            id,
            name_u: Some(format!("Comment.{id}")),
            type_: Some("Shape".into()),
            sheet,
            ..Shape::default()
        };
        let mut cmds: Vec<Box<dyn dc_core::Command>> = Vec::new();
        if let Some(c) = layer_cmd {
            cmds.push(Box::new(c));
        }
        cmds.push(Box::new(AddShape {
            page: page_id,
            parent: None,
            shape: annotation,
        }));
        let user_row = |name: &str, val: &str| {
            Box::new(dc_core::SetRowCell::new(
                Scope::Shape {
                    page: page_id,
                    shape: id,
                },
                "User",
                0,
                dc_core::commands::RowSel::Named(name.into()),
                "Value",
                vsdx::model::Cell::value(val.to_string()),
            )) as Box<dyn dc_core::Command>
        };
        cmds.push(user_row("AnnotationRef", &shape_id.to_string()));
        cmds.push(user_row("AnnotationAuthor", &author));
        cmds.push(user_row("AnnotationDate", &date));
        self.apply(dc_core::Macro::new("add comment", cmds));
    }

    /// The shape's comment thread (#29): annotation shapes whose
    /// `User.AnnotationRef` names it, oldest first — (annotation id,
    /// author, date, body).
    fn comments_for(&self, shape_id: i64) -> Vec<(i64, String, String, String)> {
        let Some(ed) = self.ed() else {
            return Vec::new();
        };
        let Some(page) = ed.doc.pages.get(self.page_idx) else {
            return Vec::new();
        };
        let user_val = |s: &Shape, row: &str| -> Option<String> {
            s.sheet
                .sections_named("User")
                .iter()
                .find_map(|sec| sec.named_row(row))
                .and_then(|r| r.cell("Value"))
                .and_then(|c| c.v.clone())
        };
        let mut out: Vec<(i64, String, String, String)> = page
            .shapes
            .iter()
            .filter(|s| {
                user_val(s, "AnnotationRef").as_deref() == Some(shape_id.to_string().as_str())
            })
            .map(|s| {
                let text = s.sheet.text.clone().unwrap_or_default();
                let author = user_val(s, "AnnotationAuthor").unwrap_or_default();
                let date = user_val(s, "AnnotationDate").unwrap_or_default();
                // Older bubbles carry "author — date: body" in the text
                // block; strip the header when the cells agree.
                let body = text
                    .strip_prefix(&format!("{author} — {date}: "))
                    .unwrap_or(&text)
                    .to_string();
                (s.id, author, date, body)
            })
            .collect();
        out.sort_by_key(|(id, _, _, _)| *id);
        out
    }

    /// The Comments popup (#29 rework, after the shipped 2016
    /// reference): one card per comment — author + date header, body,
    /// Edit / Delete — with a Reply field at the bottom. The drafts
    /// live outside the window closure (the #28 lesson).
    fn comment_window(&mut self, ctx: &egui::Context) {
        let Some(shape_id) = self.comment_target else {
            return;
        };
        let alive = self
            .ed()
            .and_then(|ed| ed.doc.pages.get(self.page_idx))
            .is_some_and(|p| vsdx::model::find_shape_by_id(&p.shapes, shape_id).is_some());
        if !alive {
            self.comment_target = None;
            self.comment_edit = None;
            self.comment_body.clear();
            return;
        }
        let Some(page_id) = self.page_id() else {
            return;
        };
        let thread = self.comments_for(shape_id);
        let mut body = std::mem::take(&mut self.comment_body);
        let mut edit = self.comment_edit.take();
        let mut close = false;
        egui::Window::new("Comments")
            .resizable(false)
            .collapsible(false)
            .default_width(320.0)
            .show(ctx, |ui| {
                if thread.is_empty() {
                    ui.weak("No comments on this shape yet.");
                }
                for (ann_id, author, date, cbody) in &thread {
                    let card = egui::Frame::new()
                        .inner_margin(egui::Margin::same(6))
                        .stroke(egui::Stroke::new(
                            1.0,
                            ui.visuals().widgets.noninteractive.bg_stroke.color,
                        ))
                        .corner_radius(4.0)
                        .show(ui, |ui| {
                            // 2016 header: avatar initial + author + date.
                            ui.horizontal(|ui| {
                                let initial = author
                                    .chars()
                                    .next()
                                    .map(|c| c.to_string())
                                    .unwrap_or_else(|| "?".into());
                                let (c, _) =
                                    ui.allocate_exact_size(Vec2::splat(16.0), Sense::hover());
                                ui.painter().circle_filled(
                                    c.center(),
                                    8.0,
                                    ui.visuals().selection.bg_fill,
                                );
                                ui.painter().text(
                                    c.center(),
                                    egui::Align2::CENTER_CENTER,
                                    initial,
                                    egui::FontId::proportional(10.0),
                                    Color32::WHITE,
                                );
                                ui.label(
                                    egui::RichText::new(format!("{author}  {date}"))
                                        .small()
                                        .weak(),
                                );
                            });
                            let editing_here = edit.as_ref().is_some_and(|(id, _)| id == ann_id);
                            let mut edit_buf = edit
                                .as_mut()
                                .filter(|(id, _)| id == ann_id)
                                .map(|(_, b)| std::mem::take(b));
                            let mut edit_done = false;
                            if editing_here {
                                let buf = edit_buf.as_mut().unwrap();
                                let r = ui.add(
                                    egui::TextEdit::multiline(buf)
                                        .desired_rows(2)
                                        .desired_width(f32::INFINITY),
                                );
                                r.widget_info(|| {
                                    egui::WidgetInfo::labeled(
                                        egui::Role::TextInput,
                                        true,
                                        "Edit comment",
                                    )
                                });
                                ui.horizontal(|ui| {
                                    if ui.button("Save").clicked() && !buf.trim().is_empty() {
                                        let author = author.clone();
                                        let date = date.clone();
                                        let new_text =
                                            format!("{} — {}: {}", author, date, buf.trim());
                                        self.apply(dc_core::SetText::new(
                                            Scope::Shape {
                                                page: page_id,
                                                shape: *ann_id,
                                            },
                                            new_text,
                                        ));
                                        edit_done = true;
                                    }
                                    if ui.button("Cancel").clicked() {
                                        edit_done = true;
                                    }
                                });
                            }
                            if !editing_here {
                                ui.label(egui::RichText::new(cbody).size(12.0));
                                ui.horizontal(|ui| {
                                    if ui.button("Edit").clicked() {
                                        edit = Some((*ann_id, cbody.clone()));
                                    }
                                    if ui.button("Delete").clicked() {
                                        self.apply(RemoveShape::new(page_id, *ann_id));
                                    }
                                });
                            }
                            // Write the (possibly typed-into) buffer
                            // back unless the editor closed.
                            if let Some(b) = edit_buf {
                                if let Some((id, slot)) = edit.as_mut() {
                                    if id == ann_id && !edit_done {
                                        *slot = b;
                                    }
                                }
                            }
                        });
                    let _ = card;
                    ui.add_space(4.0);
                }
                ui.separator();
                // Reply (or the first comment when the thread is empty).
                let r = ui.add(
                    egui::TextEdit::multiline(&mut body)
                        .desired_rows(2)
                        .desired_width(f32::INFINITY)
                        .hint_text(if thread.is_empty() {
                            "Add a comment"
                        } else {
                            "Reply"
                        }),
                );
                r.widget_info(|| {
                    egui::WidgetInfo::labeled(
                        egui::Role::TextInput,
                        true,
                        if thread.is_empty() {
                            "New comment"
                        } else {
                            "Reply"
                        },
                    )
                });
                ui.horizontal(|ui| {
                    let caption = if thread.is_empty() { "OK" } else { "Reply" };
                    if ui.button(caption).clicked() && !body.trim().is_empty() {
                        self.add_comment(shape_id, &body);
                        body.clear();
                    }
                    if ui.button("Close").clicked() {
                        close = true;
                    }
                });
            });
        if !close {
            self.comment_body = body;
            self.comment_edit = edit;
        } else {
            // Close means Close: drop the target too, or the window
            // re-renders next frame (#29 live report).
            self.comment_target = None;
            self.comment_edit = None;
            self.comment_body = String::new();
        }
    }

    /// The shape's Prop rows for the Shape Data dialog (#30):
    /// instance rows, then the master's (effective view); edits always
    /// target the instance scope by row name.
    fn prop_rows(&self, shape_id: i64) -> Vec<(String, vsdx::model::Row)> {
        let Some(ed) = self.ed() else {
            return Vec::new();
        };
        let Some(page) = ed.doc.pages.get(self.page_idx) else {
            return Vec::new();
        };
        let Some(sh) = vsdx::model::find_shape_by_id(&page.shapes, shape_id) else {
            return Vec::new();
        };
        let collect = |shapes: &[Shape]| {
            shapes
                .iter()
                .flat_map(|s| s.sheet.sections_named("Prop"))
                .flat_map(|sec| sec.rows.iter())
                .filter_map(|r| r.n.as_deref().map(|n| (n.to_string(), r.clone())))
                .collect::<Vec<_>>()
        };
        let mut out = collect(std::slice::from_ref(sh));
        if let Some(m) = sh.master.and_then(|id| ed.doc.master(id)) {
            out.extend(collect(&m.shapes));
        }
        out
    }

    /// Shape Data… (#30): one field per visible Prop row, ordered by
    /// SortKey; the editor follows the row's Type/Format. Edits write
    /// the row's Value through SetRowCell (one undo per change).
    fn shape_data_window(&mut self, ctx: &egui::Context) {
        fn write_value(app: &mut App, page_id: i64, shape_id: i64, name: &str, val: &str) {
            app.apply(dc_core::SetRowCell::new(
                Scope::Shape {
                    page: page_id,
                    shape: shape_id,
                },
                "Prop",
                0,
                dc_core::commands::RowSel::Named(name.to_string()),
                "Value",
                vsdx::model::Cell::value(val.to_string()),
            ));
        }
        let Some(shape_id) = self.shape_data_open else {
            return;
        };
        let alive = self
            .ed()
            .and_then(|ed| ed.doc.pages.get(self.page_idx))
            .is_some_and(|p| vsdx::model::find_shape_by_id(&p.shapes, shape_id).is_some());
        if !alive {
            self.shape_data_open = None;
            self.prop_bufs.clear();
            self.prop_add = None;
            return;
        }
        let Some(page_id) = self.page_id() else {
            return;
        };
        let mut rows = self.prop_rows(shape_id);
        rows.sort_by(|a, b| crate::props::cmp_sort_key(&a.1, &b.1));
        let mut bufs = std::mem::take(&mut self.prop_bufs);
        // Closable (live report on #30): the title-bar X AND a labeled
        // Close button in an always-visible action bar — the same
        // elementary/Apple-HIG cancel path the Hyperlinks window uses.
        // Default position: top-right of the viewport, where Visio
        // pins its Shape Data pane.
        let mut open = true;
        let mut close_btn = false;
        let top_right = ctx
            .input(|i| i.viewport().inner_rect)
            .map_or(egui::pos2(900.0, 60.0), |r| {
                r.right_top() + egui::vec2(-360.0, 60.0)
            });
        egui::Window::new("Shape Data")
            .open(&mut open)
            .resizable(false)
            .collapsible(false)
            .default_width(340.0)
            .default_pos(top_right)
            .show(ctx, |ui| {
                egui::Grid::new("dc-shape-data")
                    .num_columns(2)
                    .spacing(egui::vec2(8.0, 6.0))
                    .show(ui, |ui| {
                        for (name, row) in &rows {
                            if crate::props::invisible(row) {
                                continue;
                            }
                            let label = crate::props::label(row, name);
                            let current = row
                                .cell("Value")
                                .and_then(|c| c.v.clone())
                                .unwrap_or_default();
                            let tip = row
                                .cell("Prompt")
                                .and_then(|c| c.v.clone())
                                .unwrap_or_default();
                            ui.label(&label).on_hover_text(&tip);
                            let _field_id = ();
                            match crate::props::field_kind(row) {
                                crate::props::FieldKind::Bool => {
                                    let mut on = current.trim() == "1";
                                    let r = ui.checkbox(&mut on, "");
                                    r.widget_info(|| {
                                        egui::WidgetInfo::labeled(
                                            egui::Role::CheckBox,
                                            true,
                                            &label,
                                        )
                                    });
                                    if r.changed() {
                                        write_value(
                                            self,
                                            page_id,
                                            shape_id,
                                            name,
                                            if on { "1" } else { "0" },
                                        );
                                    }
                                }
                                crate::props::FieldKind::List(choices) => {
                                    // A manual dropdown: the stock
                                    // ComboBox's arrow glyph is missing
                                    // from the pinned font set.
                                    let r = ui.button(format!("{current}  ›")).on_hover_text(&tip);
                                    r.widget_info(|| {
                                        egui::WidgetInfo::labeled(
                                            egui::Role::ComboBox,
                                            true,
                                            &label,
                                        )
                                    });
                                    let mut picked: Option<String> = None;
                                    egui::Popup::menu(&r)
                                        .close_behavior(egui::PopupCloseBehavior::CloseOnClick)
                                        .show(|ui| {
                                            ui.set_min_width(120.0);
                                            for c in &choices {
                                                if ui.selectable_label(*c == current, c).clicked() {
                                                    picked = Some(c.clone());
                                                }
                                            }
                                        });
                                    if let Some(v) = picked {
                                        write_value(self, page_id, shape_id, name, &v);
                                    }
                                }
                                crate::props::FieldKind::VarList(choices) => {
                                    // Variable list (Type 4): the same
                                    // dropdown over the Format choices,
                                    // plus "＋" to reveal an add row —
                                    // adding a choice writes the row's
                                    // Value AND appends to Format (one
                                    // undo step), so the list grows
                                    // the way Visio's variable list
                                    // does.
                                    let mut picked: Option<String> = None;
                                    let mut open_add = false;
                                    ui.horizontal(|ui| {
                                        let r =
                                            ui.button(format!("{current}  ›")).on_hover_text(&tip);
                                        r.widget_info(|| {
                                            egui::WidgetInfo::labeled(
                                                egui::Role::ComboBox,
                                                true,
                                                &label,
                                            )
                                        });
                                        egui::Popup::menu(&r)
                                            .close_behavior(egui::PopupCloseBehavior::CloseOnClick)
                                            .show(|ui| {
                                                ui.set_min_width(120.0);
                                                for c in &choices {
                                                    if ui
                                                        .selectable_label(*c == current, c)
                                                        .clicked()
                                                    {
                                                        picked = Some(c.clone());
                                                    }
                                                }
                                            });
                                        if ui.small_button("+").clicked() {
                                            open_add = true;
                                        }
                                    });
                                    if open_add {
                                        self.prop_add = Some(name.clone());
                                    }
                                    if let Some(v) = picked {
                                        write_value(self, page_id, shape_id, name, &v);
                                    }
                                    // The add row (label column empty).
                                    if self.prop_add.as_deref() == Some(name.as_str()) {
                                        ui.end_row();
                                        ui.label("");
                                        let add_key = format!("{name}#add");
                                        let mut commit = false;
                                        let mut cancel = false;
                                        {
                                            let buf = bufs.entry(add_key.clone()).or_default();
                                            ui.horizontal(|ui| {
                                                let edit = ui.add(
                                                    egui::TextEdit::singleline(buf)
                                                        .hint_text("new choice"),
                                                );
                                                edit.widget_info(|| {
                                                    egui::WidgetInfo::labeled(
                                                        egui::Role::TextInput,
                                                        true,
                                                        &label,
                                                    )
                                                });
                                                if ui.button("Add").clicked()
                                                    || (edit.lost_focus()
                                                        && ui.input(|i| {
                                                            i.key_pressed(egui::Key::Enter)
                                                        }))
                                                {
                                                    commit = true;
                                                }
                                                if ui.button("Cancel").clicked() {
                                                    cancel = true;
                                                }
                                            });
                                        }
                                        if cancel {
                                            bufs.remove(&add_key);
                                            self.prop_add = None;
                                        } else if commit {
                                            let v = bufs
                                                .get(&add_key)
                                                .map(|b| b.trim().to_string())
                                                .unwrap_or_default();
                                            if v.contains(';') {
                                                // A choice is one ";"-separated
                                                // Format entry — an embedded
                                                // separator would split into
                                                // phantom choices. Keep the add
                                                // row open for correction.
                                                self.status = format!(
                                                    "{label}: a choice cannot contain ';'"
                                                );
                                            } else if !v.is_empty() {
                                                bufs.remove(&add_key);
                                                self.prop_add = None;
                                                let fmt = row
                                                    .cell("Format")
                                                    .and_then(|c| c.v.clone())
                                                    .unwrap_or_default();
                                                if fmt.split(';').any(|c| c.trim() == v) {
                                                    // Already a choice: select it,
                                                    // don't duplicate it in Format.
                                                    write_value(self, page_id, shape_id, name, &v);
                                                } else {
                                                    let new_fmt = if fmt.trim().is_empty() {
                                                        v.clone()
                                                    } else {
                                                        format!("{};{}", fmt.trim_end(), v)
                                                    };
                                                    let scope = Scope::Shape {
                                                        page: page_id,
                                                        shape: shape_id,
                                                    };
                                                    let row_sel =
                                                        dc_core::commands::RowSel::Named(
                                                            name.clone(),
                                                        );
                                                    self.apply(dc_core::Macro::new(
                                                        format!("add {name} choice {v:?}"),
                                                        vec![
                                                            Box::new(dc_core::SetRowCell::new(
                                                                scope.clone(),
                                                                "Prop",
                                                                0,
                                                                row_sel.clone(),
                                                                "Value",
                                                                vsdx::model::Cell::value(v),
                                                            )),
                                                            Box::new(dc_core::SetRowCell::new(
                                                                scope,
                                                                "Prop",
                                                                0,
                                                                row_sel,
                                                                "Format",
                                                                vsdx::model::Cell::value(new_fmt),
                                                            )),
                                                        ],
                                                    ));
                                                }
                                            } else {
                                                bufs.remove(&add_key);
                                                self.prop_add = None;
                                            }
                                        }
                                    }
                                }
                                crate::props::FieldKind::Number => {
                                    let buf =
                                        bufs.entry(name.clone()).or_insert_with(|| current.clone());
                                    let r =
                                        ui.add(egui::TextEdit::singleline(buf).hint_text("0.0"));
                                    r.widget_info(|| {
                                        egui::WidgetInfo::labeled(
                                            egui::Role::TextInput,
                                            true,
                                            &label,
                                        )
                                    });
                                    if r.lost_focus() && buf.as_str() != current {
                                        match buf.trim().parse::<f64>() {
                                            Ok(_) => {
                                                let v = buf.trim().to_string();
                                                write_value(self, page_id, shape_id, name, &v);
                                            }
                                            Err(_) => {
                                                self.status = format!("{label}: not a number");
                                            }
                                        }
                                        bufs.remove(name);
                                    }
                                }
                                crate::props::FieldKind::Str
                                | crate::props::FieldKind::Formatted(_) => {
                                    let buf =
                                        bufs.entry(name.clone()).or_insert_with(|| current.clone());
                                    let r = ui.add(egui::TextEdit::singleline(buf));
                                    r.widget_info(|| {
                                        egui::WidgetInfo::labeled(
                                            egui::Role::TextInput,
                                            true,
                                            &label,
                                        )
                                    });
                                    if r.lost_focus() && buf.as_str() != current {
                                        let v = buf.trim().to_string();
                                        write_value(self, page_id, shape_id, name, &v);
                                        bufs.remove(name);
                                    }
                                }
                            }
                            ui.end_row();
                        }
                    });
                // Always-visible action bar: Close bottom-right.
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Close").clicked() {
                            close_btn = true;
                        }
                    });
                });
            });
        if open && !close_btn {
            self.prop_bufs = bufs;
        } else {
            self.shape_data_open = None;
            self.prop_bufs.clear();
            self.prop_add = None;
        }
    }

    fn hyperlink_window(&mut self, ctx: &egui::Context) {
        let Some(shape_id) = self.hyperlink_open else {
            return;
        };
        // Close automatically when the shape or page goes away.
        let alive = self
            .ed()
            .and_then(|ed| ed.doc.pages.get(self.page_idx))
            .is_some_and(|p| vsdx::model::find_shape_by_id(&p.shapes, shape_id).is_some());
        if !alive {
            self.hyperlink_open = None;
            self.hyperlink_draft = None;
            return;
        }
        let Some(page_id) = self.page_id() else {
            return;
        };
        let scope = Scope::Shape {
            page: page_id,
            shape: shape_id,
        };
        // Instance rows plus the master's, inherited-origin visible
        // (#139) — `hyperlink_rows` keeps instance-first order and
        // drops master rows an instance row already shadows.
        let rows = self.hyperlink_rows(shape_id);
        let mut draft = self.hyperlink_draft.take();
        // Cancel mechanism (live report + the 2016 dialog's Cancel):
        // the title-bar X and an explicit Close button both discard
        // any draft and dismiss the window.
        let mut open = true;
        // The Close BUTTON flips this (the X writes `open` via the
        // builder; the two can't share a borrow).
        let mut close_btn = false;
        egui::Window::new("Hyperlinks")
            .open(&mut open)
            .resizable(false)
            .collapsible(false)
            .default_width(380.0)
            .show(ctx, |ui| {
                if rows.is_empty() && draft.is_none() {
                    ui.weak("No hyperlinks on this shape.");
                }
                for row in &rows {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(if row.desc.is_empty() {
                                row.addr.clone()
                            } else {
                                format!("{} — {}", row.desc, row.addr)
                            })
                            .size(12.0),
                        );
                        if row.inherited {
                            // Origin marker + read-only Remove: the row
                            // belongs to the master (Visio keeps
                            // inherited row sets read-only until
                            // overridden); Edit materializes the
                            // override (#139).
                            ui.weak("inherited");
                        }
                        if ui.button("Go").clicked() {
                            let (a, s) = (row.addr.clone(), row.sub.clone());
                            self.follow_hyperlink(&a, &s);
                        }
                        if ui.button("Edit").clicked() {
                            draft = Some(HyperlinkDraft {
                                edit_row: Some(row.ix),
                                from_master: row.inherited,
                                address: row.addr.clone(),
                                sub: row.sub.clone(),
                                desc: row.desc.clone(),
                            });
                        }
                        let remove = ui.add_enabled(!row.inherited, egui::Button::new("Remove"));
                        if row.inherited {
                            remove.on_disabled_hover_text(
                                "Inherited from the shape's master — edit it to override it on this shape",
                            );
                        } else if remove.clicked() {
                            self.apply(dc_core::commands::RemoveSectionRow::new(
                                scope.clone(),
                                "Hyperlink",
                                0,
                                dc_core::commands::RowSel::Ix(row.ix),
                            ));
                        }
                    });
                }
                ui.separator();
                if let Some(d) = draft.as_mut() {
                    // Accessible names on the inputs (#56 pattern);
                    // the grid labels alone don't label the fields.
                    let field = |ui: &mut Ui, label: &str, buf: &mut String| {
                        ui.label(label);
                        let r = ui.add(egui::TextEdit::singleline(buf).hint_text(label));
                        r.widget_info(|| {
                            egui::WidgetInfo::labeled(egui::Role::TextInput, true, label)
                        });
                    };
                    egui::Grid::new("dc-hyperlink-draft")
                        .num_columns(2)
                        .spacing(egui::vec2(8.0, 4.0))
                        .show(ui, |ui| {
                            field(ui, "Address", &mut d.address);
                            ui.end_row();
                            field(ui, "Sub-address", &mut d.sub);
                            ui.end_row();
                            field(ui, "Description", &mut d.desc);
                            ui.end_row();
                        });
                    let (edit_row, from_master, address, sub, desc) = (
                        d.edit_row,
                        d.from_master,
                        d.address.clone(),
                        d.sub.clone(),
                        d.desc.clone(),
                    );
                    let mut close_draft = false;
                    ui.horizontal(|ui| {
                        if ui.button("OK").clicked() {
                            let ix = edit_row.unwrap_or_else(|| {
                                rows.iter().map(|r| r.ix).max().unwrap_or(-1) + 1
                            });
                            let mut cmds: Vec<Box<dyn dc_core::Command>> = Vec::new();
                            if let Some(old) = edit_row {
                                // An instance edit replaces its row. A
                                // master-inherited row has no local row
                                // to replace: SetRowCell's first-edit
                                // materialization seeds the local
                                // section+row (inherited values
                                // included) — the master stays
                                // untouched (#139).
                                if !from_master {
                                    cmds.push(Box::new(
                                        dc_core::commands::RemoveSectionRow::new(
                                            scope.clone(),
                                            "Hyperlink",
                                            0,
                                            dc_core::commands::RowSel::Ix(old),
                                        ),
                                    ));
                                }
                            }
                            for (name, val) in [
                                ("Address", &address),
                                ("SubAddress", &sub),
                                ("Description", &desc),
                            ] {
                                // A materializing edit writes every
                                // field, cleared ones as explicit
                                // empty values: the materialized row
                                // is seeded with the inherited cells,
                                // so a skipped field would silently
                                // keep its master value (#139 review).
                                if from_master || !val.trim().is_empty() {
                                    cmds.push(Box::new(dc_core::SetRowCell::new(
                                        scope.clone(),
                                        "Hyperlink",
                                        0,
                                        dc_core::commands::RowSel::Ix(ix),
                                        name,
                                        vsdx::model::Cell::value(val.trim()),
                                    )));
                                }
                            }
                            // An all-empty draft writes nothing (and
                            // must not record a no-op undo step).
                            if !cmds.is_empty() {
                                self.apply(dc_core::Macro::new("edit hyperlink", cmds));
                            }
                            close_draft = true;
                        }
                        if ui.button("Cancel").clicked() {
                            close_draft = true;
                        }
                    });
                    if close_draft {
                        draft = None;
                    }
                }
                // The window's action bar (always visible — elementary/
                // Apple HIG: the dismiss control must be obvious, not
                // hidden behind editor state): New… left, Close right.
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    if draft.is_none() && ui.button("New…").clicked() {
                        draft = Some(HyperlinkDraft {
                            edit_row: None,
                            from_master: false,
                            address: String::new(),
                            sub: String::new(),
                            desc: String::new(),
                        });
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Close").clicked() {
                            draft = None;
                            close_btn = true;
                        }
                    });
                });
            });
        if open && !close_btn {
            self.hyperlink_draft = draft;
        } else {
            // Cancelled: discard the draft and close.
            self.hyperlink_open = None;
            self.hyperlink_draft = None;
        }
    }

    /// Visio's Reorder Pages dialog (#54): the page list with Move Up /
    /// Move Down buttons; every click is one undoable MovePage command,
    /// and the active page stays active wherever it lands.
    fn reorder_pages_window(&mut self, ctx: &egui::Context) {
        if !self.reorder_open {
            return;
        }
        let mut close = false;
        egui::Window::new("Reorder Pages")
            .resizable(false)
            .collapsible(false)
            .default_width(280.0)
            .show(ctx, |ui| {
                let n = self.ed().map(|e| e.doc.pages.len()).unwrap_or(0);
                let names: Vec<String> = self
                    .ed()
                    .map(|e| e.doc.pages.iter().map(|p| p.name.clone()).collect())
                    .unwrap_or_default();
                egui::Grid::new("dc-reorder-list")
                    .num_columns(3)
                    .spacing(egui::vec2(8.0, 4.0))
                    .show(ui, |ui| {
                        for (i, name) in names.iter().enumerate() {
                            ui.label(egui::RichText::new(name.clone()).size(12.0));
                            if ui
                                .add_enabled(i > 0, egui::Button::new("Move Up").small())
                                .clicked()
                            {
                                self.tab_cmd_reorder(i, i - 1);
                            }
                            if ui
                                .add_enabled(i + 1 < n, egui::Button::new("Move Down").small())
                                .clicked()
                            {
                                self.tab_cmd_reorder(i, i + 1);
                            }
                            ui.end_row();
                        }
                    });
                ui.add_space(8.0);
                if ui.button("Close").clicked() {
                    close = true;
                }
            });
        if close {
            self.reorder_open = false;
        }
    }

    /// The Visio 2016 shape context menu: clipboard, duplicate, delete,
    /// z-order, rotate/flip, text editing, ShapeSheet. Backlog items
    /// (Group, Format Shape, Shape Data) show grayed with their milestone
    /// so the surface matches Visio while the backends catch up.
    /// Master-defined right-click actions (ShapeSheet Actions section),
    /// in row order: (menu label, is-flyout-child). Visio heads the shape
    /// menu with these.
    /// Master-defined Actions rows (#25): labels, evaluated
    /// Checked/Invisible/Disabled, BeginGroup/FlyoutChild, SortKey
    /// ordering — everything the context menu needs, evaluated against
    /// the INSTANCE's effective sheet so instance-local User rows
    /// (which the actions themselves write) drive the checkmarks.
    fn master_actions(&self, shape_id: i64) -> Vec<MasterActionRow> {
        struct RawRow {
            menu: String,
            action: String,
            checked_f: Option<String>,
            checked_v: Option<String>,
            disabled_f: Option<String>,
            disabled_v: Option<String>,
            invisible_f: Option<String>,
            invisible_v: Option<String>,
            begin_group: bool,
            flyout_child: bool,
            sort_key: f64,
            /// `Action` = No Formula or ReadOnly: a flyout header, not a
            /// command.
            display_only: bool,
        }
        // A cell's live value: formula first (evaluated in `env`),
        // cached V as fallback — the row-cell convention.
        let eval_flag =
            |f: &Option<String>, v: &Option<String>, env: &vsdx::formula::Env| -> bool {
                let by_formula = f.as_deref().and_then(|f| {
                    if f == "Inh" || f == "No Formula" {
                        None
                    } else {
                        vsdx::formula::eval(f, env).ok().and_then(|x| x.as_f64())
                    }
                });
                by_formula
                    .or_else(|| v.as_deref().and_then(|v| v.trim().parse::<f64>().ok()))
                    .is_some_and(|x| x != 0.0)
            };
        let collect = |self_: &App| -> Option<Vec<MasterActionRow>> {
            let ed = self_.ed.as_ref()?;
            let page = ed.doc.pages.get(self_.page_idx)?;
            let shape = vsdx::model::find_shape_by_id(&page.shapes, shape_id)?;
            let m = ed.doc.master(shape.master?)?;
            // Instance evaluation environment: instance-local User rows
            // override the master's (dc::svg::shape_env layers them).
            let chain = dc_core::scope::shape_chain(&ed.doc, page.id, shape_id).ok()?;
            let env = dc::svg::shape_env(&ed.doc, shape, &chain);
            let mut rows: Vec<RawRow> = Vec::new();
            fn walk(sh: &Shape, out: &mut Vec<RawRow>) {
                for sec in sh.sheet.sections_named("Actions") {
                    for row in &sec.rows {
                        // Visio writes flyout labels either as V or as
                        // the Cell's text content (multi-line menus) —
                        // the parser keeps the latter as raw_inner.
                        let menu = row
                            .cell("Menu")
                            .and_then(|c| {
                                c.v.clone()
                                    .or_else(|| c.raw_inner.as_deref().map(str::to_owned))
                            })
                            .map(|t| t.trim().to_owned())
                            .filter(|t| !t.is_empty());
                        let Some(menu) = menu else { continue };
                        let f = |n: &str| -> Option<String> {
                            row.cell(n)
                                .and_then(|c| c.f.clone())
                                .filter(|f| !f.is_empty() && f != "No Formula" && f != "Inh")
                        };
                        let v =
                            |n: &str| -> Option<String> { row.cell(n).and_then(|c| c.v.clone()) };
                        // The Action FORMULA is F; V caches the last
                        // evaluation ('No Formula' rows carry V only).
                        let action_f = row.cell("Action").and_then(|c| c.f.clone());
                        let action = action_f
                            .clone()
                            .filter(|f| f != "No Formula")
                            .or_else(|| v("Action"))
                            .unwrap_or_default();
                        let display_only = action_f.as_deref() == Some("No Formula")
                            || v("ReadOnly")
                                .and_then(|x| x.trim().parse::<f64>().ok())
                                .is_some_and(|x| x != 0.0);
                        out.push(RawRow {
                            menu,
                            action,
                            display_only,
                            checked_f: f("Checked"),
                            checked_v: v("Checked"),
                            disabled_f: f("Disabled"),
                            disabled_v: v("Disabled"),
                            invisible_f: f("Invisible"),
                            invisible_v: v("Invisible"),
                            begin_group: v("BeginGroup")
                                .and_then(|x| x.trim().parse::<f64>().ok())
                                .is_some_and(|x| x != 0.0),
                            flyout_child: v("FlyoutChild")
                                .and_then(|x| x.trim().parse::<f64>().ok())
                                .is_some_and(|x| x != 0.0),
                            sort_key: v("SortKey")
                                .and_then(|x| x.trim().parse::<f64>().ok())
                                .unwrap_or(0.0),
                        });
                    }
                }
                for sub in &sh.sub_shapes {
                    walk(sub, out);
                }
            }
            for root in &m.shapes {
                walk(root, &mut rows);
            }
            // Visio orders by SortKey, then row order (stable).
            rows.sort_by(|a, b| {
                a.sort_key
                    .partial_cmp(&b.sort_key)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            Some(
                rows.into_iter()
                    .filter(|r| !eval_flag(&r.invisible_f, &r.invisible_v, &env))
                    .map(|r| MasterActionRow {
                        menu: r.menu,
                        action: r.action,
                        checked: eval_flag(&r.checked_f, &r.checked_v, &env),
                        disabled: eval_flag(&r.disabled_f, &r.disabled_v, &env),
                        begin_group: r.begin_group,
                        flyout_child: r.flyout_child,
                        display_only: r.display_only,
                    })
                    .collect(),
            )
        };
        collect(self).unwrap_or_default()
    }

    /// Execute a master Actions row (#25): parse the SETF/GetRef chain
    /// and apply it as one undoable Macro against the instance. Rows
    /// whose formula needs a runtime we do not ship (RUNADDON, …)
    /// defer with a status note — the EventDblClick policy (#20).
    fn run_master_action(&mut self, shape_id: i64, row: &MasterActionRow) {
        let Some(ed) = self.ed() else { return };
        let Some(page_id) = self.page_id() else {
            return;
        };
        let Some(page) = ed.doc.pages.iter().find(|p| p.id == page_id) else {
            return;
        };
        let Some(shape) = vsdx::model::find_shape_by_id(&page.shapes, shape_id) else {
            return;
        };
        let Ok(chain) = dc_core::scope::shape_chain(&ed.doc, page_id, shape_id) else {
            return;
        };
        let env = dc::svg::shape_env(&ed.doc, shape, &chain);
        let scope = Scope::Shape {
            page: page_id,
            shape: shape_id,
        };
        match dc_core::commands::shape_action_commands(scope, &row.action, &env) {
            Ok(cmds) if cmds.is_empty() => {
                self.status = format!("{:?} is display-only", row.menu);
            }
            Ok(cmds) => {
                let label = format!("run shape action {:?}", row.menu);
                self.apply(dc_core::Macro::new(label, cmds));
            }
            Err(why) => {
                self.status = format!("{:?} deferred: {why}", row.menu);
            }
        }
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
            let next = (cur * 72.0 + delta_pt).clamp(4.0, 200.0) / 72.0;
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
            ColorTarget::FillBkgnd => "FillBkgnd",
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
            Master(usize),
            Group,
            Ungroup,
            /// Enter the selected group's edit mode (#78).
            OpenGroup(i64),
            Container,
            Hyperlink,
            AddComment,
            ShapeData,
            FormatShape,
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
            // Actions rows are live (#25): evaluated checkmarks and
            // disabled state, BeginGroup separators, FlyoutChild
            // indentation, and SETF execution on click.
            for (ai, row) in actions.iter().enumerate() {
                if row.begin_group && ai > 0 {
                    ui.separator();
                }
                // Flyout arrow: › (the pinned font set has no ▸).
                let label = if row.flyout_child {
                    format!("     {}", row.menu)
                } else {
                    format!("{}  ›", row.menu)
                };
                // A display-only row (No Formula) stays grayed like
                // Visio's flyout headers; Disabled is the formula's.
                let enabled = !row.disabled && !row.display_only && !row.action.trim().is_empty();
                let icon = row.checked.then_some(crate::ribbon::Icon::Check);
                if ctx_item(ui, icon, &label, "", enabled) && enabled {
                    act = Some(Act::Master(ai));
                }
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
            // Group ▸ is a flyout (Visio 2016, #78): Group/Ungroup live
            // inside it, plus Open <name> to enter group-edit mode.
            // Group needs a multi-selection; Ungroup/Open need a
            // selected group (#26).
            let can_group = self.sel_set.len() >= 2;
            let selected_group = self
                .ed()
                .and_then(|ed| ed.doc.pages.get(self.page_idx))
                .and_then(|p| vsdx::model::find_shape_by_id(&p.shapes, shape_id))
                .filter(|s| s.type_.as_deref() == Some("Group") && !s.sub_shapes.is_empty());
            let is_group = selected_group.is_some();
            let group_label = selected_group
                .and_then(|s| s.name_u.clone())
                .unwrap_or_else(|| format!("Group.{shape_id}"));
            // The flyout parent row: drawn directly so we own its
            // Response for Popup::menu (the Container ▸ pattern).
            let tokens = theme::menu_tokens(ui.visuals());
            let mut group_job = egui::text::LayoutJob::default();
            group_job.append(
                "  Group  ›",
                0.0,
                egui::TextFormat::simple(egui::FontId::proportional(12.5), tokens.label),
            );
            let group_row = ui.add_enabled(
                true,
                egui::Button::new(group_job)
                    .min_size(Vec2::new(ui.available_width().max(150.0), 21.0)),
            );
            egui::Popup::menu(&group_row)
                .close_behavior(egui::PopupCloseBehavior::CloseOnClick)
                .show(|ui| {
                    ui.set_min_width(210.0);
                    if ctx_item(ui, None, "Group", "Ctrl+G", can_group) {
                        act = Some(Act::Group);
                    }
                    if ctx_item(ui, None, "Ungroup", "Ctrl+Shift+G", is_group) {
                        act = Some(Act::Ungroup);
                    }
                    if ctx_item(ui, None, &format!("Open {group_label}"), "", is_group) {
                        act = Some(Act::OpenGroup(shape_id));
                    }
                });
            // Container is a flyout, not an action (Visio 2016): the
            // parent row only opens the submenu. Anchor: draw the row
            // directly so we own its Response for Popup::menu.
            let mut job = egui::text::LayoutJob::default();
            job.append(
                "  Container  ›",
                0.0,
                egui::TextFormat::simple(egui::FontId::proportional(12.5), tokens.label),
            );
            let cont = ui.add_enabled(
                !self.sel_set.is_empty(),
                egui::Button::new(job).min_size(Vec2::new(ui.available_width().max(150.0), 21.0)),
            );
            let mut sub_action = false;
            egui::Popup::menu(&cont)
                .close_behavior(egui::PopupCloseBehavior::CloseOnClick)
                .show(|ui| {
                    ui.set_min_width(210.0);
                    if ctx_item(ui, None, "Add to New Container", "", true) {
                        sub_action = true;
                    }
                    ctx_item(
                        ui,
                        None,
                        "Add to Underlying Container",
                        "(later milestone)",
                        false,
                    );
                });
            if sub_action {
                act = Some(Act::Container);
            }
            if ctx_item(ui, None, "Hyperlink…", "Ctrl+K", true) {
                act = Some(Act::Hyperlink);
            }
            if ctx_item(ui, None, "Add Comment", "", true) {
                act = Some(Act::AddComment);
            }
            if ctx_item(ui, None, "Shape Data…", "", true) {
                act = Some(Act::ShapeData);
            }
            if ctx_item(ui, None, "Format Shape…", "", true) {
                act = Some(Act::FormatShape);
            }
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
            Act::Master(ai) => {
                if let Some(row) = actions.get(ai).cloned() {
                    self.run_master_action(shape_id, &row);
                }
            }
            Act::Group => {
                let members: Vec<i64> = self
                    .sel_set
                    .iter()
                    .filter(|(p, _)| *p == page_id)
                    .map(|(_, sid)| *sid)
                    .collect();
                let new_id = self
                    .ed()
                    .map(|ed| dc_core::commands::next_shape_id(&ed.doc, page_id));
                self.apply(dc_core::commands::GroupShapes::new(page_id, members));
                if let Some(nid) = new_id {
                    self.set_selection(Some((page_id, nid)));
                }
            }
            Act::Container => self.wrap_selection_in_container(),
            Act::Hyperlink => {
                self.hyperlink_open = Some(shape_id);
                self.hyperlink_draft = None;
            }
            Act::AddComment => {
                self.comment_target = Some(shape_id);
                self.comment_body.clear();
                self.comment_edit = None;
            }
            Act::ShapeData => {
                self.shape_data_open = Some(shape_id);
                self.prop_bufs.clear();
                self.prop_add = None;
            }
            Act::FormatShape => {
                self.format_pane_open = true;
                self.fmt_bufs.clear();
            }
            Act::Ungroup => {
                let first_member = self.ed().and_then(|ed| {
                    ed.doc.pages.get(self.page_idx).and_then(|p| {
                        vsdx::model::find_shape_by_id(&p.shapes, shape_id)
                            .and_then(|g| g.sub_shapes.first().map(|m| m.id))
                    })
                });
                self.apply(dc_core::commands::UngroupShape::new(page_id, shape_id));
                if let Some(m) = first_member {
                    self.set_selection(Some((page_id, m)));
                }
            }
            Act::OpenGroup(gid) => {
                // Enter (or deepen) group-edit mode (#78): view state
                // only — members keep editing through the same commands,
                // one undo each, as on the page.
                if !self.group_edit.contains(&gid) {
                    self.group_edit.push(gid);
                }
                let name = self.ed().and_then(|ed| {
                    ed.doc.pages.get(self.page_idx).and_then(|p| {
                        vsdx::model::find_shape_by_id(&p.shapes, gid)
                            .and_then(|s| s.name_u.clone())
                    })
                });
                self.status = format!(
                    "Opened group {} — Esc or Close Group to return",
                    name.unwrap_or_else(|| format!("Group.{gid}"))
                );
            }
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
                self.begin_text_edit(shape_id, draft);
                self.status = format!("editing text of shape {shape_id}");
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

    /// Current text of a shape (draft seed for the in-place editor).
    fn selected_text(&self, shape_id: i64) -> Option<String> {
        let page_id = self.page_id()?;
        let ed = self.ed.as_ref()?;
        let page = ed.doc.pages.iter().find(|p| p.id == page_id)?;
        let s = vsdx::model::find_shape_by_id(&page.shapes, shape_id)?;
        s.sheet.text.clone()
    }

    /// Open the in-place WYSIWYG editor on a shape (GitHub #22): the
    /// session captures the view (restored on exit) and the central
    /// panel applies the entry zoom, lays the editor over the text
    /// block, and suppresses the shape's text in its canvas layer.
    fn begin_text_edit(&mut self, shape_id: i64, draft: String) {
        let Some(page_id) = self.page_id() else { return };
        self.text_edit = Some(crate::textedit::TextEditSession::new(
            page_id,
            shape_id,
            draft,
            &self.view,
        ));
    }

    /// End the in-place editor: restore the pre-edit view, and commit
    /// the draft through `SetText` (one undo step, dirty, texture
    /// invalidation — the retired dialog's commit path). A no-change
    /// exit (open, close, type nothing) applies nothing — Visio does
    /// not record an undo step for it.
    fn end_text_edit(&mut self, commit: bool) {
        let Some(sess) = self.text_edit.take() else {
            return;
        };
        sess.restore_view(&mut self.view);
        if !commit {
            return;
        }
        // The shape must still exist to receive the commit — the edit
        // can outlive it (Ctrl+Z of its placement is not keyboard-
        // gated). Resolve first and drop the draft when the page or
        // shape is gone instead of applying SetText at a dead scope
        // (#22 review).
        let current = self
            .ed
            .as_ref()
            .and_then(|ed| ed.doc.pages.iter().find(|p| p.id == sess.page))
            .and_then(|p| vsdx::model::find_shape_by_id(&p.shapes, sess.shape))
            .map(|s| s.sheet.text.clone().unwrap_or_default());
        let Some(current) = current else {
            return;
        };
        if sess.draft != current {
            self.apply(SetText::new(
                Scope::Shape {
                    page: sess.page,
                    shape: sess.shape,
                },
                sess.draft,
            ));
        }
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
    pub fn debug_master_cells(&self) -> Vec<(crate::stencil::MasterKey, Rect)> {
        self.stencil.cell_rects.clone()
    }
    /// Column count of the last stencil layout, re-derived from the
    /// actual cell rects (distinct column x positions) so tests check
    /// the rendered layout, not a stored number (GitHub #53).
    #[doc(hidden)]
    pub fn debug_stencil_cols(&self) -> usize {
        let mut xs: Vec<f32> = self
            .stencil
            .cell_rects
            .iter()
            .map(|(_, r)| r.left())
            .collect();
        xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
        xs.dedup_by(|a, b| (*a - *b).abs() < 0.5);
        xs.len()
    }
    /// Width the last stencil layout wrapped into (pane content width
    /// minus the scrollbar allowance) — the input tests use to
    /// re-derive the expected column count (GitHub #53).
    #[doc(hidden)]
    pub fn debug_stencil_wrap_width(&self) -> f32 {
        self.stencil.last_wrap_width
    }
    /// Stencil panel content rect from the last frame; its right edge
    /// plus the 6px frame margin is the panel's resize handle (the
    /// kittest: multi-selection size (#26).
    #[doc(hidden)]
    /// kittest re-wrap tests drag it, GitHub #53).
    #[doc(hidden)]
    pub fn debug_stencil_panel_inner_rect(&self) -> Rect {
        self.stencil_panel_inner
    }
    #[doc(hidden)]
    pub fn debug_canvas_rect(&self) -> Rect {
        self.canvas_rect
    }
    /// kittest: the active view zoom (#22 asserts the entry zoom and
    /// the restore).
    #[doc(hidden)]
    pub fn debug_view_zoom(&self) -> f32 {
        self.view.zoom
    }
    /// kittest: the open in-place editor session as (page, shape)
    /// (#22).
    #[doc(hidden)]
    pub fn debug_text_edit(&self) -> Option<(i64, i64)> {
        self.text_edit.as_ref().map(|s| (s.page, s.shape))
    }
    /// kittest: the screen rect the editor overlay covers — the
    /// shape's text block mapped through the live view (#22).
    #[doc(hidden)]
    pub fn debug_text_edit_rect(&self) -> Option<Rect> {
        let sess = self.text_edit.as_ref()?;
        let ed = self.ed.as_ref()?;
        let page = ed.doc.pages.iter().find(|p| p.id == sess.page)?;
        let (x, y, w, h) = dc::svg::text_block_page_rect(&ed.doc, page, sess.shape)?;
        let geom = self.page_geom();
        let tl = canvas::page_to_screen(x, y + h, &self.view, &geom);
        let br = canvas::page_to_screen(x + w, y, &self.view, &geom);
        Some(Rect::from_two_pos(tl, br))
    }
    #[doc(hidden)]
    /// kittest: multi-selection size (#26).
    #[doc(hidden)]
    pub fn debug_sel_set_len(&self) -> usize {
        self.sel_set.len()
    }
    pub fn debug_sel(&self) -> Option<(i64, i64)> {
        self.sel
    }
    /// The open group-edit stack, outermost first (#78).
    #[doc(hidden)]
    pub fn debug_group_edit(&self) -> Vec<i64> {
        self.group_edit.clone()
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
            self.view.pan.x + r.right() * ppi,
            self.view.pan.y + r.bottom() * ppi,
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
            self.view.pan.x + r.left() * ppi,
            self.view.pan.y + r.top() * ppi,
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
    pub fn debug_armed(&self) -> Option<MasterKey> {
        self.stencil.armed
    }
    /// kittest (#63): open a stencil file into the pane exactly like
    /// the "Open Existing Stencil…" flow (minus the native dialog).
    #[doc(hidden)]
    pub fn open_stencil_for_test(&mut self, path: &std::path::Path) {
        self.open_stencil(path.to_path_buf());
    }
    /// kittest (#83): File→Open through the real dialog-worker seam —
    /// the next dialog resolves with `path` instead of opening rfd.
    #[doc(hidden)]
    pub fn open_file_for_test(&mut self, path: std::path::PathBuf) {
        self.dialog_preset_for_test = Some(Some(path));
        self.do_open();
    }
    /// kittest (#63): open-stencil entry titles + expansion, pane order.
    #[doc(hidden)]
    pub fn debug_stencil_entries(&self) -> Vec<(String, bool)> {
        self.stencils
            .iter()
            .map(|(_, st)| (st.title.clone(), st.expanded))
            .collect()
    }
    /// kittest (#86): set the pane's search box (the results-view
    /// filter) without driving the text edit widget.
    #[doc(hidden)]
    pub fn set_search_for_test(&mut self, q: &str) {
        self.stencil.search = q.to_string();
    }
    /// kittest (#88): scroll the stencil pane back to its top next
    /// frame (a just-opened stencil's reveal may have scrolled the
    /// Quick Shapes cells out of the viewport).
    #[doc(hidden)]
    pub fn scroll_pane_to_top_for_test(&mut self) {
        self.stencil.scroll_fix = Some(0.0);
    }
    /// kittest (#88): the Quick Shapes header rect from the last
    /// layout.
    #[doc(hidden)]
    pub fn debug_quick_rect_for_test(&self) -> Option<egui::Rect> {
        self.stencil.quick_rect
    }
    /// kittest (#88): the search box rect from the last layout.
    #[doc(hidden)]
    pub fn debug_search_rect_for_test(&self) -> Option<egui::Rect> {
        self.stencil.search_rect
    }
    /// kittest (#102): reveal the document-stencil entry for this
    /// session (masters outside Quick Shapes' head set — e.g. the 1D
    /// Line — are only listed there).
    #[doc(hidden)]
    pub fn show_doc_stencil_for_test(&mut self) {
        self.stencil.doc_stencil_visible = true;
    }
    /// kittest (#86): the pane entries currently VISIBLE (headers
    /// rendered this frame) — a search hides zero-match entries.
    #[doc(hidden)]
    pub fn debug_visible_entry_ids(&self) -> Vec<crate::stencil::StencilId> {
        self.stencil.entry_rects.iter().map(|(id, _)| *id).collect()
    }
    /// kittest (#84): the edited document's masters (id, NameU).
    #[doc(hidden)]
    pub fn debug_doc_masters(&self) -> Vec<(i64, String)> {
        self.ed
            .as_ref()
            .map(|ed| {
                ed.doc
                    .masters
                    .iter()
                    .map(|m| (m.id, m.name_u.clone().unwrap_or_default()))
                    .collect()
            })
            .unwrap_or_default()
    }
    /// kittest (#85): the landed thumbnail texture for one pane cell.
    #[doc(hidden)]
    pub fn debug_thumb_id_for_test(
        &self,
        stencil: StencilId,
        master: i64,
    ) -> Option<egui::TextureId> {
        let serial = match stencil {
            StencilId::Document => crate::stencil::DOC_SERIAL,
            StencilId::Open(ix) => self.stencils.get(ix).map(|s| s.serial())?,
        };
        self.stencil.thumb_id(serial, master)
    }
    /// kittest (#63): stencil list entry header rects, document entry
    /// first.
    #[doc(hidden)]
    pub fn debug_stencil_entry_rects(&self) -> Vec<(StencilId, egui::Rect)> {
        self.stencil.entry_rects.clone()
    }
    #[doc(hidden)]
    pub fn debug_carry(&self) -> Option<MasterKey> {
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
    /// kittest: pin the displayed document path so the backstage Info
    /// page renders a fixed Location instead of the test's random
    /// tempdir (goldens must be identical on every OS/runner).
    #[doc(hidden)]
    pub fn set_path_for_test(&mut self, path: PathBuf) {
        self.path = path;
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
    /// kittest: a manual zoom (#62's second ruler density) — leaves fit
    /// mode like the real control so the zoom actually applies.
    #[doc(hidden)]
    pub fn set_zoom_for_test(&mut self, z: f32) {
        self.view.zoom = z.clamp(0.1, 8.0);
        self.view.fit_mode = false;
        self.view.fit_pending = false;
    }
    /// kittest: page names in tab order (#54).
    #[doc(hidden)]
    pub fn debug_page_names(&self) -> Vec<String> {
        self.ed()
            .map(|e| e.doc.pages.iter().map(|p| p.name.clone()).collect())
            .unwrap_or_default()
    }

    /// kittest: the first top-level shape's layer texture id (#92/#93)
    /// — `None` while its first raster is still on a worker.
    #[doc(hidden)]
    pub fn debug_page_tex_id_for_test(&self) -> Option<egui::TextureId> {
        let ed = self.ed.as_ref()?;
        let page = ed.doc.pages.get(self.page_idx)?;
        let first = page.shapes.first()?;
        self.layers.tex_id_for_test(first.id)
    }

    /// kittest: one shape's layer texture id (#93) — stable across a
    /// pure move (the texture is translation-invariant).
    #[doc(hidden)]
    pub fn debug_layer_tex_id_for_test(&self, shape_id: i64) -> Option<egui::TextureId> {
        self.layers.tex_id_for_test(shape_id)
    }

    /// kittest: a shape layer's content version (#22 review) — the
    /// cache probe that a ppi-bucket re-raster cannot perturb.
    #[doc(hidden)]
    pub fn debug_layer_version_for_test(&self, shape_id: i64) -> u64 {
        self.layers.version_for_test(shape_id)
    }

    /// kittest (#68): the active page's geometry in inches — an
    /// inserted page must carry its source's sheet size, not the
    /// 0.1-in missing-cell clamp.
    #[doc(hidden)]
    pub fn debug_page_geom_for_test(&self) -> (f64, f64) {
        let g = self.page_geom();
        (g.w_in, g.h_in)
    }

    /// kittest: how many live drag ghosts the last frame drew (#96).
    #[doc(hidden)]
    pub fn debug_drag_ghosts_for_test(&self) -> usize {
        self.last_drag_ghosts
    }

    /// kittest: how many shape layers have landed (#93) — the post cap
    /// keeps the first frame from rasterizing the whole page inline.
    #[doc(hidden)]
    pub fn debug_layer_count_for_test(&self) -> usize {
        self.layers.landed_count_for_test()
    }

    /// kittest: how many stencil master thumbnails have landed (#92).
    #[doc(hidden)]
    pub fn debug_stencil_thumb_count_for_test(&self) -> usize {
        self.stencil.thumb_count_for_test()
    }
    /// kittest (#92): the per-frame post cap the async contract asserts
    /// against.
    #[doc(hidden)]
    pub fn debug_pool_post_cap_for_test(&self) -> usize {
        self.pool.post_cap()
    }
    /// kittest (#92): the most raster jobs any single frame ever
    /// queued — must never exceed the cap (the anti-burst invariant).
    #[doc(hidden)]
    pub fn debug_pool_peak_posts_per_frame_for_test(&self) -> usize {
        self.pool.debug_peak_posts_per_frame()
    }
    /// kittest (#92): lifetime queued raster jobs.
    #[doc(hidden)]
    pub fn debug_pool_total_posts_for_test(&self) -> u64 {
        self.pool.debug_total_posts()
    }
    /// kittest: a shape's INSTANCE-LOCAL User row value (#25) — None
    /// while the row is inherited from the master.
    #[doc(hidden)]
    pub fn debug_user_value_for_test(&self, shape_id: i64, row: &str) -> Option<f64> {
        let ed = self.ed.as_ref()?;
        let page = ed.doc.pages.get(self.page_idx)?;
        let sh = vsdx::model::find_shape_by_id(&page.shapes, shape_id)?;
        sh.sheet
            .sections_named("User")
            .iter()
            .find_map(|sec| sec.named_row(row))?
            .cell("Value")?
            .numeric()
    }
    /// kittest: a shape's effective hyperlink rows as (address,
    /// sub-address) — instance rows first, then unshadowed
    /// master-inherited rows (#28, #139).
    #[doc(hidden)]
    pub fn debug_hyperlinks_for_test(&self, shape_id: i64) -> Vec<(String, String)> {
        self.shape_hyperlinks(shape_id)
            .into_iter()
            .map(|(a, s, _)| (a, s))
            .collect()
    }
    /// kittest: the shape's INSTANCE hyperlink rows as (ix, address,
    /// sub-address, description) — empty until a row is materialized
    /// onto the instance sheet (#139).
    #[doc(hidden)]
    pub fn debug_instance_hyperlinks_for_test(
        &self,
        shape_id: i64,
    ) -> Vec<(i64, String, String, String)> {
        self.hyperlink_rows(shape_id)
            .into_iter()
            .filter(|r| !r.inherited)
            .map(|r| (r.ix, r.addr, r.sub, r.desc))
            .collect()
    }
    /// kittest: the shape's MASTER's hyperlink rows verbatim as
    /// (address, sub-address) — the edit-materialization path must
    /// leave these unchanged (#139).
    #[doc(hidden)]
    pub fn debug_master_hyperlinks_for_test(&self, shape_id: i64) -> Vec<(String, String)> {
        let Some(ed) = self.ed() else {
            return Vec::new();
        };
        let Some(page) = ed.doc.pages.get(self.page_idx) else {
            return Vec::new();
        };
        let Some(sh) = vsdx::model::find_shape_by_id(&page.shapes, shape_id) else {
            return Vec::new();
        };
        sh.master
            .and_then(|id| ed.doc.master(id))
            .map(|m| {
                m.shapes
                    .iter()
                    .flat_map(|s| s.sheet.sections_named("Hyperlink"))
                    .flat_map(|sec| sec.rows.iter())
                    .map(|r| {
                        (
                            r.cell("Address").and_then(|c| c.v.clone()).unwrap_or_default(),
                            r.cell("SubAddress").and_then(|c| c.v.clone()).unwrap_or_default(),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
    /// kittest: annotation shapes on the Annotation layer (#29) as
    /// (shape id, text).
    #[doc(hidden)]
    pub fn debug_annotations_for_test(&self) -> Vec<(i64, String)> {
        let Some(ed) = self.ed() else {
            return Vec::new();
        };
        let Some(page) = ed.doc.pages.get(self.page_idx) else {
            return Vec::new();
        };
        let layer_ix = dc_core::layers::layer_rows(&ed.doc, page.id)
            .unwrap_or_default()
            .into_iter()
            .find(|r| dc_core::layers::layer_name(r) == Some("Annotation"))
            .map(|r| r.ix);
        let Some(layer_ix) = layer_ix else {
            return Vec::new();
        };
        page.shapes
            .iter()
            .filter(|s| {
                s.sheet.cell("LayerMember").and_then(|c| c.numeric()) == Some(layer_ix as f64)
            })
            .map(|s| (s.id, s.sheet.text.clone().unwrap_or_default()))
            .collect()
    }
    /// kittest: annotation visibility state (#29).
    #[doc(hidden)]
    pub fn show_comments_for_test(&self) -> bool {
        self.show_comments
    }
    /// kittest: flip annotation visibility directly (#29) — the ribbon
    /// button shares the standard toggle dispatch.
    #[doc(hidden)]
    pub fn set_show_comments_for_test(&mut self, on: bool) {
        self.show_comments = on;
    }
    /// kittest: one cell of a shape's effective Prop row (#30) —
    /// instance first, then the master.
    #[doc(hidden)]
    pub fn debug_prop_cell_for_test(
        &self,
        shape_id: i64,
        row: &str,
        cell: &str,
    ) -> Option<String> {
        self.prop_rows(shape_id)
            .iter()
            .find(|(n, _)| n == row)?
            .1
            .cell(cell)
            .and_then(|c| c.v.clone())
    }
    /// kittest: give a placed shape a Prop row through the command
    /// layer (#30) — one undo step, like a real authoring edit.
    #[doc(hidden)]
    pub fn debug_add_prop_row_for_test(
        &mut self,
        shape_id: i64,
        name: &str,
        type_: &str,
        format: &str,
        value: &str,
    ) {
        let Some(page_id) = self.page_id() else {
            return;
        };
        let scope = Scope::Shape {
            page: page_id,
            shape: shape_id,
        };
        let row = dc_core::commands::RowSel::Named(name.to_string());
        let cell = |v: &str| vsdx::model::Cell::value(v.to_string());
        self.apply(dc_core::Macro::new(
            format!("add {name} prop row (test)"),
            vec![
                Box::new(dc_core::SetRowCell::new(
                    scope.clone(),
                    "Prop",
                    0,
                    row.clone(),
                    "Type",
                    cell(type_),
                )),
                Box::new(dc_core::SetRowCell::new(
                    scope.clone(),
                    "Prop",
                    0,
                    row.clone(),
                    "Format",
                    cell(format),
                )),
                Box::new(dc_core::SetRowCell::new(
                    scope,
                    "Prop",
                    0,
                    row,
                    "Value",
                    cell(value),
                )),
            ],
        ));
    }
    /// kittest: a shape's effective Prop row Value (#30) — instance
    /// first, then the master.
    #[doc(hidden)]
    pub fn debug_prop_value_for_test(&self, shape_id: i64, row: &str) -> Option<String> {
        let rows = self.prop_rows(shape_id);
        rows.iter()
            .find(|(n, _)| n == row)?
            .1
            .cell("Value")
            .and_then(|c| c.v.clone())
    }
    /// kittest: a shape's effective cell value (#31).
    #[doc(hidden)]
    pub fn debug_cell_for_test(&self, shape_id: i64, cell: &str) -> Option<String> {
        let ed = self.ed()?;
        let page_id = self.page_id()?;
        let page = ed.doc.pages.iter().find(|p| p.id == page_id)?;
        let sh = vsdx::model::find_shape_by_id(&page.shapes, shape_id)?;
        let chain = shape_chain(&ed.doc, page_id, shape_id).ok()?;
        // Char.X / Para.X read the effective first Character/Paragraph
        // row cell.
        if let Some(row_cell) = cell.strip_prefix("Char.") {
            return ed
                .doc
                .effective_sheet(sh, &chain)
                .sections_named("Character")
                .first()
                .and_then(|sec| sec.rows.first())
                .and_then(|r| r.cell(row_cell))
                .and_then(|c| c.v.clone())
                .map(|v| v.trim().to_string());
        }
        if let Some(row_cell) = cell.strip_prefix("Para.") {
            return ed
                .doc
                .effective_sheet(sh, &chain)
                .sections_named("Paragraph")
                .first()
                .and_then(|sec| sec.rows.first())
                .and_then(|r| r.cell(row_cell))
                .and_then(|c| c.v.clone())
                .map(|v| v.trim().to_string());
        }
        ed.doc
            .resolve_cell(sh, &chain, cell)
            .and_then(|c| c.v.clone())
            .map(|v| v.trim().to_string())
    }
    /// kittest: the active page's name (#54).
    #[doc(hidden)]
    pub fn debug_active_page_name(&self) -> String {
        self.ed()
            .and_then(|e| e.doc.pages.get(self.page_idx))
            .map(|p| p.name.clone())
            .unwrap_or_default()
    }
    /// A shape's current text (double-click edit tests).
    #[doc(hidden)]
    pub fn debug_shape_text(&self, shape_id: i64) -> Option<String> {
        self.selected_text(shape_id)
    }
    /// The page SVG the canvas is currently showing (scratch debugging).
    #[doc(hidden)]
    pub fn debug_page_svg(&self) -> Option<String> {
        let ed = self.ed.as_ref()?;
        let page = &ed.doc.pages[self.page_idx.min(ed.doc.pages.len().saturating_sub(1))];
        dc::svg::render_page(&ed.doc, page).ok()
    }
    /// All four selection-frame corners in SCREEN coords, [tl, tr, bl,
    /// br] — the four-corner resize tests (GitHub #59).
    #[doc(hidden)]
    pub fn debug_sel_corners(&self) -> Option<[egui::Pos2; 4]> {
        let r = self.sel_page_rect()?;
        let ppi = self.view.px_per_in();
        let at = |x: f32, y: f32| egui::pos2(self.view.pan.x + x * ppi, self.view.pan.y + y * ppi);
        Some([
            at(r.left(), r.top()),
            at(r.right(), r.top()),
            at(r.left(), r.bottom()),
            at(r.right(), r.bottom()),
        ])
    }
    /// One selection handle's screen position (line endpoint/midpoint
    /// drag tests, GitHub #59).
    #[doc(hidden)]
    pub fn debug_handle_screen(&self, id: crate::canvas::HandleId) -> Option<egui::Pos2> {
        let h = self.selection_handles().into_iter().find(|h| h.id == id)?;
        let geom = self.page_geom();
        Some(crate::canvas::page_to_screen(
            h.pos.0, h.pos.1, &self.view, &geom,
        ))
    }
    /// The status-bar text (guard-refusal tests).
    #[doc(hidden)]
    pub fn debug_status(&self) -> &str {
        &self.status
    }
    /// One resolved cell of a shape, as (value, formula) — asserting
    /// guarded cells stay untouched (GitHub #59 tests).
    #[doc(hidden)]
    pub fn debug_cell(&self, page: i64, shape: i64, name: &str) -> Option<(String, String)> {
        let ed = self.ed.as_ref()?;
        let p = ed.doc.pages.iter().find(|p| p.id == page)?;
        let sh = vsdx::model::find_shape_by_id(&p.shapes, shape)?;
        let chain = shape_chain(&ed.doc, page, shape).ok()?;
        let c = ed.doc.resolve_cell(sh, &chain, name)?;
        Some((
            c.v.clone().unwrap_or_default(),
            c.f.clone().unwrap_or_default(),
        ))
    }
    /// Test hook: write a cell on a shape's own sheet, optionally with a
    /// formula (e.g. `GUARD(2 in)`), through the command layer.
    #[doc(hidden)]
    pub fn set_cell_for_test(
        &mut self,
        page: i64,
        shape: i64,
        name: &str,
        v: &str,
        f: Option<&str>,
    ) {
        let cell = vsdx::model::Cell {
            v: Some(v.to_string()),
            f: f.map(|s| s.to_string()),
            ..vsdx::model::Cell::default()
        };
        self.apply(SetCell::new(Scope::Shape { page, shape }, name, cell));
    }
    /// Test hook: turn a Protection-section lock on for a shape
    /// (LockWidth/LockHeight/LockBegin/LockEnd — GitHub #59 tests).
    #[doc(hidden)]
    pub fn set_protection_for_test(&mut self, page: i64, shape: i64, name: &str, on: bool) {
        use vsdx::model::{Row, Section, SheetChild};
        let Some(ed) = self.ed_mut() else { return };
        let Some(p) = ed.doc.pages.iter_mut().find(|p| p.id == page) else {
            return;
        };
        let Some(sh) = vsdx::model::find_shape_by_id_mut(&mut p.shapes, shape) else {
            return;
        };
        let val = if on { "1" } else { "0" };
        // Replace an existing lock cell in place (Row::cell reads the
        // first match — appending would make a later `on=false`
        // unreadable and the lock uncleearable); create the section
        // otherwise. Both paths bump the version so the view refreshes.
        let set_cell = |cells: &mut Vec<(String, vsdx::model::Cell)>| match cells
            .iter_mut()
            .find(|(n, _)| n == name)
        {
            Some((_, c)) => *c = vsdx::model::Cell::value(val),
            None => cells.push((name.to_string(), vsdx::model::Cell::value(val))),
        };
        if let Some(SheetChild::Section(sec)) = sh
            .sheet
            .children
            .iter_mut()
            .find(|c| matches!(c, SheetChild::Section(s) if s.name == "Protection"))
        {
            match sec.rows.first_mut() {
                Some(row) => set_cell(&mut row.cells),
                None => sec.rows.push(Row {
                    cells: vec![(name.to_string(), vsdx::model::Cell::value(val))],
                    ..Row::default()
                }),
            }
        } else {
            sh.sheet.children.push(SheetChild::Section(Section {
                name: "Protection".into(),
                rows: vec![Row {
                    cells: vec![(name.to_string(), vsdx::model::Cell::value(val))],
                    ..Row::default()
                }],
                ..Section::default()
            }));
        }
        self.dirty = true;
        self.version += 1;
    }

    /// The Visio theme palette shown by the ribbon Fill/Line buttons.
    fn color_palette_window(&mut self, ctx: &egui::Context) {
        let Some(target) = self.color_pick else {
            return;
        };
        let mut open = true;
        egui::Window::new(match target {
            ColorTarget::Fill => "Fill Color",
            ColorTarget::FillBkgnd => "Fill Background",
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
        let Some(key) = self.stencil.carry else {
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
            let Some(ed) = self.ed.as_ref() else { return };
            // The ghost renders from the master's own source document
            // (#82): a stencil master need not exist in the edited
            // document (it is copied in only on placement, #84).
            let Some((serial, src, m)) = self.stencils.resolve(ed, key) else {
                self.stencil.carry = None;
                return;
            };
            // Ghost: the master thumbnail drawn at the shape's real
            // on-page size, semi-transparent, pinned at the cursor.
            let (w, h) = master_size(src, m.id);
            let size = Vec2::new((w as f32 * ppi).max(10.0), (h as f32 * ppi).max(10.0));
            let r = Rect::from_center_size(p, size);
            let layer =
                egui::LayerId::new(egui::Order::Tooltip, egui::Id::unique("stencil-ghost"));
            let painter = ctx.layer_painter(layer);
            // While the first-use thumbnail is still on a worker
            // (#92) the ghost carries just its outline.
            if let Some(tex) = self.stencil.texture(ctx, serial, src, m.id, &mut self.pool) {
                painter.image(
                    tex.id(),
                    r,
                    Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                    Color32::from_white_alpha(150),
                );
            }
            let t = theme::shell_tokens(&ctx.style_of(ctx.theme()).visuals);
            painter.rect_stroke(
                r,
                1.0,
                Stroke::new(1.0, t.selection),
                egui::StrokeKind::Middle,
            );
        }
        if released {
            self.stencil.carry = None;
            if self.canvas_rect.contains(p) {
                let geom = self.page_geom();
                let (x, y) = canvas::screen_to_page(p, &self.view, &geom);
                self.place_master(key, (x, y));
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
        let _frame = crate::perf::span(crate::perf::Stage::Frame);
        // Off-thread rasters (#92): land worker completions before
        // layout so this frame uses fresh textures; anything landing
        // (or a post deferred by the per-frame cap) schedules the
        // repaint that shows it. The UI thread only pays load_texture.
        let landed = self.pool.drain();
        if !landed.is_empty() || self.pool.take_deferred() {
            ui.ctx().request_repaint();
        }
        for done in landed {
            match done.key {
                crate::rasterpool::RasterKey::Page { part, version, bucket } => {
                    self.tex.on_page_done(
                        ui.ctx(),
                        part,
                        version,
                        bucket,
                        done.image,
                        done.tree,
                        done.was_parse,
                    );
                }
                crate::rasterpool::RasterKey::Thumb { serial, master } => {
                    self.stencil.on_thumb_done(ui.ctx(), serial, master, done.image);
                }
                crate::rasterpool::RasterKey::Shape { page, id, version, bucket } => {
                    if let Some(msg) = self.layers.on_shape_done(
                        ui.ctx(),
                        page,
                        id,
                        version,
                        bucket,
                        done.image,
                    ) {
                        self.status = msg;
                    }
                }
            }
        }
        // Group-edit stack coherence (#78): drop open levels that no
        // longer resolve to a Group on the active page (ungroup, undo,
        // page switch, document change — one check covers them all).
        if !self.group_edit.is_empty() {
            let page_groups = |id: i64| -> bool {
                self.ed()
                    .and_then(|ed| ed.doc.pages.get(self.page_idx))
                    .map(|p| {
                        vsdx::model::find_shape_by_id(&p.shapes, id)
                            .is_some_and(|s| s.type_.as_deref() == Some("Group"))
                    })
                    .unwrap_or(false)
            };
            let ok = self.group_edit.iter().copied().take_while(|id| page_groups(*id)).count();
            self.group_edit.truncate(ok);
        }
        let ctx = ui.ctx().clone();
        // OS theme flips (GitHub #52): the watcher thread re-probes the
        // OS preference on its period; switch the active style slot when
        // it reports a change, and ask for a repaint on the same period
        // so an idle app still wakes to pick a flip up.
        if let Some(watcher) = &self.os_theme {
            if let Some(pref) = watcher.poll() {
                ctx.set_theme(pref);
            }
            ctx.request_repaint_after(Duration::from_secs(2));
        }
        self.handle_shortcuts(&ctx);
        self.update_drag(&ctx);
        self.poll_dialog(&ctx);
        self.handle_close_request(&ctx);
        self.reorder_pages_window(&ctx);
        self.hyperlink_window(&ctx);
        self.comment_window(&ctx);
        self.shape_data_window(&ctx);
        self.color_palette_window(&ctx);

        // The eframe root Ui paints no background and eframe's clear
        // color is near-black, so the shell must own one: the panel
        // fill (kittest synthesizes exactly this behind the root Ui,
        // which is why the goldens never caught the black production
        // canvas) covers the start screen, the File backstage, and any
        // margin the panels leave. The canvas paints the desk over its
        // own area on top (GitHub #52).
        ui.painter().rect_filled(
            ui.max_rect(),
            0.0,
            theme::shell_tokens(ui.visuals()).ribbon_bg,
        );

        if self.backstage {
            self.backstage_view(ui);
            self.confirm_new_window(&ctx);
            self.confirm_close_window(&ctx);
            return;
        }

        // Shell order: top bands, bottom bands, side panels, canvas.
        let t = theme::shell_tokens(ui.visuals());
        self.title_band(ui);
        self.tab_strip(ui);
        egui::Panel::top("ribbon_content")
            .frame(
                egui::Frame::new()
                    .fill(t.ribbon_content)
                    .inner_margin(egui::Margin::symmetric(4, 3)),
            )
            .show(ui, |ui| self.ribbon_content(ui));
        // Visio: a hairline rule closes the ribbon off from the canvas.
        egui::Panel::top("ribbon_rule")
            .exact_size(1.0)
            .frame(egui::Frame::new().fill(t.ribbon_edge))
            .show(ui, |_| {});
        egui::Panel::bottom("status")
            .frame(
                egui::Frame::new()
                    .fill(t.status_bg)
                    .inner_margin(egui::Margin::symmetric(10, 3)),
            )
            .show(ui, |ui| self.status_bar(ui));
        egui::Panel::bottom("page_tabs")
            .frame(
                egui::Frame::new()
                    .fill(t.tabs_bg)
                    .inner_margin(egui::Margin::same(2)),
            )
            .show(ui, |ui| self.page_tabs(ui));
        // The pane lists the document's masters plus every open
        // stencil (#63): it stays up once a stencil is open, even for
        // a master-less document — but it needs a document to render
        // against (the pane body draws nothing without an editor).
        let pane_wanted = self
            .ed()
            .map(|ed| !ed.doc.masters.is_empty() || !self.stencils.is_empty())
            .unwrap_or(false);
        if pane_wanted {
            egui::Panel::left("stencil")
                .frame(
                    egui::Frame::new()
                        .fill(t.stencil_bg)
                        .inner_margin(egui::Margin::same(6)),
                )
                .default_size(160.0)
                .resizable(true)
                .show(ui, |ui| {
                    ui.vertical(|ui| {
                        // The pane draws its own Visio header ("Shapes"
                        // + More Shapes), search box, stencil list and
                        // master grid (crates/dc-ui/src/stencil.rs).
                        self.stencil_panel_inner = ui.max_rect();
                        let mut armed = None;
                        let mut request = None;
                        let title = stencil_title(&self.path);
                        {
                            let App { ed, stencil, stencils, pool, .. } = self;
                            if let Some(ed) = ed.as_ref() {
                                if let Some(id) = stencil.show(ui, ed, stencils, &title, pool) {
                                    armed = Some(id);
                                }
                                request = stencil.request.take();
                            }
                        }
                        // Pane requests (#83/#87): the pane can't reach
                        // the dialog worker or the catalog's byte
                        // loader itself.
                        match request {
                            Some(crate::stencil::Request::OpenStencilDialog) => {
                                self.begin_dialog(PendingKind::OpenStencil);
                            }
                            Some(crate::stencil::Request::ToggleBuiltin(slug)) => {
                                self.toggle_builtin(slug);
                            }
                            None => {}
                        }
                        if let Some(id) = armed {
                            self.status =
                                format!("master {} armed — click the canvas to place", id.1);
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
        if self.format_pane_open && self.sel.is_some() {
            egui::Panel::right("format_pane")
                .default_size(240.0)
                .resizable(true)
                .show(ui, |ui| self.format_pane(ui));
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
        self.confirm_close_window(&ctx);
    }

    /// A close request arrived — the window's ✕ / Alt-F4 (or the
    /// kittest hook): veto it and ask the user first when there is
    /// anything to lose; a clean document closes straight away
    /// (GitHub #65). egui 0.37 semantics: the app exits after this
    /// frame unless `ViewportCommand::CancelClose` is sent.
    fn handle_close_request(&mut self, ctx: &Context) {
        let requested = self.close_req_for_test || ctx.input(|i| i.viewport().close_requested());
        self.close_req_for_test = false;
        if !requested {
            return;
        }
        if self.closing {
            // Our own approved close echoing back (egui-winit feeds
            // ViewportCommand::Close into the next frame's input):
            // send nothing — the un-vetoed request is what exits the
            // app — and never resurrect the prompt.
            return;
        }
        // While the prompt is already up — or its Save has the native
        // save-as dialog open (`close_after_save` armed) — absorb
        // further requests: a second prompt's Cancel would contradict
        // the pending chain, and its Don't Save would race the
        // dialog's save (one prompt; the open dialog still decides).
        if self.confirm_close.is_some() || self.close_after_save.is_some() {
            self.veto_close(ctx);
            return;
        }
        if self.dirty && self.ed.is_some() {
            // Leaving supersedes the New prompt if it was up.
            self.confirm_new = false;
            self.veto_close(ctx);
            self.confirm_close = Some(CloseTarget::App);
        } else {
            // Clean: let the close proceed. Re-issuing Close also
            // covers the test hook, where no OS close stands behind
            // the flag (in production it is an idempotent no-op).
            self.issue_close(ctx);
        }
    }

    /// The "Save changes?" modal before a window close or File → Close
    /// discards a dirty document (GitHub #65). Visio 2016 shape:
    /// Save (save-as first when untitled, then close), Don't Save,
    /// Cancel.
    fn confirm_close_window(&mut self, ctx: &Context) {
        let Some(target) = self.confirm_close else {
            return;
        };
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.confirm_close = None;
            return;
        }
        let name = self
            .path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "the drawing".into());
        egui::Window::new("Save changes?")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label(format!("Save changes to {name}?"));
                ui.horizontal(|ui| {
                    if ui.button("Save").clicked() {
                        self.confirm_close = None;
                        // New and close chains are mutually exclusive.
                        self.new_after_save = false;
                        if self.path.as_os_str().is_empty() {
                            // pick a file first; the close continues
                            // after the dialog completes
                            self.close_after_save = Some(target);
                            self.begin_dialog(PendingKind::SaveAs);
                        } else {
                            let p = self.path.clone();
                            self.save_to(&p);
                            if !self.dirty {
                                self.finish_close(ctx, target);
                            }
                            // save failed — stay open; the status bar
                            // carries the error
                        }
                    }
                    if ui.button("Don't Save").clicked() {
                        self.confirm_close = None;
                        self.finish_close(ctx, target);
                    }
                    if ui.button("Cancel").clicked() {
                        self.confirm_close = None;
                    }
                });
            });
    }

    /// Execute a confirmed close: the app window (re-issue the close
    /// the veto cancelled) or just the document (back to the start
    /// screen — the app stays open, Visio's File → Close).
    fn finish_close(&mut self, ctx: &Context, target: CloseTarget) {
        self.close_after_save = None;
        match target {
            CloseTarget::App => self.issue_close(ctx),
            CloseTarget::Doc => self.close_doc(),
        }
    }

    /// Veto an in-flight OS close so the prompt can run.
    fn veto_close(&mut self, ctx: &Context) {
        ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        self.debug_close_vetoes += 1;
    }

    /// Ask the viewport to close (eframe exits the app for the root).
    /// Marked so the close's own echo — egui-winit delivers
    /// `ViewportCommand::Close` as next frame's close-requested
    /// input — passes through instead of being re-prompted (#65).
    fn issue_close(&mut self, ctx: &Context) {
        self.closing = true;
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        self.debug_close_issued += 1;
    }

    /// File → Close teardown: back to the start screen, nothing dirty.
    fn close_doc(&mut self) {
        self.ed = None;
        self.path = PathBuf::new();
        self.dirty = false;
        self.set_selection(None);
        self.stencil = Stencil::default();
        self.tex.clear();
        self.layers.clear();
        self.page_states.clear();
        self.tab_menu = None;
        self.tab_rename = None;
        self.reorder_open = false;
        self.comment_target = None;
        self.comment_edit = None;
        self.status = "document closed".into();
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
        // Esc always returns to the document — unless a prompt owns
        // the screen.
        if !self.confirm_new
            && self.confirm_close.is_none()
            && ui.ctx().input(|i| i.key_pressed(egui::Key::Escape))
        {
            close_backstage = true;
        }
        let has_doc = self.ed.is_some();
        let t = theme::shell_tokens(ui.visuals());
        egui::Panel::left("backstage_rail")
            .frame(
                egui::Frame::new()
                    .fill(t.accent_band)
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
                            t.rail_disabled
                        });
                        let mut b = egui::Button::new(txt).frame(false);
                        if selected {
                            b = b.fill(t.rail_selected);
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
                                .color(t.rail_disabled),
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
            .fill(t.ribbon_content)
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
                            // Live thumbnail of the active page (#58):
                            // the same render → rasterize → texture pipeline
                            // the stencil pane uses, cached per (page,
                            // version) so any edit refreshes it. The SVG
                            // itself is built lazily by the cache's miss
                            // closure (#90) — an open Info tab no longer
                            // re-serializes the page every frame.
                            let (thumb_w_in, thumb_h_in, thumb_part) = {
                                let ed = self.ed.as_ref().unwrap();
                                let page = &ed.doc.pages
                                    [self.page_idx.min(ed.doc.pages.len().saturating_sub(1))];
                                let n = |name: &str| {
                                    page.sheet()
                                        .and_then(|s| s.cell(name))
                                        .and_then(|c| c.numeric())
                                        .unwrap_or(0.0)
                                };
                                (
                                    n("PageWidth").max(0.1),
                                    n("PageHeight").max(0.1),
                                    page.part.clone(),
                                )
                            };
                            ui.heading(&name);
                            ui.add_space(8.0);
                            ui.horizontal(|ui| {
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
                                ui.add_space(24.0);
                                // Bordered card on the card token; neutral
                                // (no error text) when the render fails.
                                let img_w = 192.0_f64;
                                let aspect = (thumb_h_in / thumb_w_in).clamp(0.2, 3.0);
                                let (r, _) = ui.allocate_exact_size(
                                    Vec2::new(
                                        (img_w + 16.0) as f32,
                                        (img_w * aspect + 16.0) as f32,
                                    ),
                                    Sense::hover(),
                                );
                                ui.painter().rect_filled(r, 3.0, t.card_bg);
                                ui.painter().rect_stroke(
                                    r,
                                    3.0,
                                    Stroke::new(1.0, t.ribbon_group),
                                    egui::StrokeKind::Middle,
                                );
                                if !self.info_render_fail_for_test {
                                    // ~192 px wide, aspect-true: the raster
                                    // derives its height from the page.
                                    let ppi = (img_w / thumb_w_in) as f32;
                                    let key = format!("info-{}", thumb_part.replace('/', "_"));
                                    if let Ok(Some(tex)) = self.tex.get(
                                        ui.ctx(),
                                        &mut self.pool,
                                        thumb_w_in,
                                        &key,
                                        self.version,
                                        ppi,
                                        || {
                                            let ed = self.ed.as_ref().unwrap();
                                            let page = &ed.doc.pages[self
                                                .page_idx
                                                .min(ed.doc.pages.len().saturating_sub(1))];
                                            dc::svg::render_page(&ed.doc, page)
                                        },
                                    ) {
                                        let ir = Rect::from_min_size(
                                            r.min + Vec2::new(8.0, 8.0),
                                            Vec2::new(img_w as f32, (img_w * aspect) as f32),
                                        );
                                        let uv =
                                            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
                                        ui.painter().image(tex.id(), ir, uv, Color32::WHITE);
                                    }
                                }
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
                    // Ask before a dirty document is discarded (#65);
                    // Cancel returns to the document, so the backstage
                    // closes either way, Visio-style.
                    if self.dirty && self.ed.is_some() {
                        self.confirm_close = Some(CloseTarget::Doc);
                    } else {
                        self.close_doc();
                    }
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
    let t = theme::shell_tokens(ui.visuals());
    ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
        let (tile, _) = ui.allocate_exact_size(Vec2::new(120.0, 96.0), Sense::hover());
        ui.painter().rect_filled(tile, 3.0, t.card_bg);
        ui.painter().rect_stroke(
            tile,
            3.0,
            Stroke::new(1.0, t.ribbon_group),
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
                egui::Label::new(egui::RichText::new(name).size(12.0).color(t.back_title))
                    .selectable(false),
            )
            .on_hover_text(tip);
        let card = tile.union(caption.rect);
        let resp = ui.interact(
            card,
            ui.scope_id().with(("backstage_card", name)),
            Sense::click(),
        );
        if resp.hovered() && enabled {
            ui.painter().rect_stroke(
                card.shrink(1.0),
                3.0,
                Stroke::new(1.5, t.ribbon_hover_edge),
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
    let t = theme::shell_tokens(ui.visuals());
    let mut job = egui::text::LayoutJob::default();
    job.append(
        &format!("    {title}"),
        0.0,
        egui::TextFormat::simple(egui::FontId::proportional(13.0), t.back_title),
    );
    job.append(
        &format!("\n    {desc}"),
        0.0,
        egui::TextFormat::simple(egui::FontId::proportional(10.5), t.ribbon_label),
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

/// Current UTC date as `YYYY-MM-DD` — zero-dep civil-from-days
/// (Hinnant's algorithm) over the Unix epoch seconds.
fn format_now_utc() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let days = secs.div_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}

fn qat_button(ui: &mut Ui, icon: Icon, tip: &str, action: impl FnOnce() -> Action, app: &mut App) {
    let (r, resp) = ui.allocate_exact_size(Vec2::new(22.0, 22.0), Sense::click());
    // The icon is painted, so the control needs its name attached;
    // screen readers read "Save", not "Save (Ctrl+S)" (GitHub #56).
    let name = tip.split(" (").next().unwrap_or(tip);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, name));
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

/// Point-in-bbox test with a small pad (~1 mm): 1D lines have a
/// degenerate dimension (Height 0 when horizontal), and a zero-thickness
/// bbox is otherwise unhittable (GitHub #59). Visio hit-tests lines with
/// a tolerance too.
fn hit_in_rect(pos: (f64, f64), x: f64, y: f64, w: f64, h: f64) -> bool {
    const PAD: f64 = 0.04; // inches
    pos.0 >= x - PAD && pos.0 <= x + w + PAD && pos.1 >= y - PAD && pos.1 <= y + h + PAD
}

/// Every shape of a subtree, document (z) order, recursive (#78):
/// the group-edit mode's hit-test and marquee candidate set.
fn collect_members<'a>(shapes: &'a [Shape], out: &mut Vec<&'a Shape>) {
    for s in shapes {
        out.push(s);
        collect_members(&s.sub_shapes, out);
    }
}

/// Whether a Protection-section flag (`LockWidth`, `LockHeight`,
/// `LockBegin`, `LockEnd`, …) is effectively ON for a shape: its own
/// sheet first, then the master's via the inheritance chain. Cells may
/// sit at section level or in the section's single row, so both spots
/// are checked (GitHub #59).
fn protection_flag(doc: &Document, sh: &Shape, chain: &[&Shape], name: &str) -> bool {
    fn sheet_locked(sheet: &vsdx::model::Sheet, name: &str) -> bool {
        let on = |c: Option<&vsdx::model::Cell>| {
            c.and_then(|c| c.numeric())
                .map(|v| v != 0.0)
                .unwrap_or(false)
        };
        on(sheet.cell(name))
            || sheet
                .sections_named("Protection")
                .iter()
                .any(|sec| on(sec.cell(name)) || sec.rows.iter().any(|r| on(r.cell(name))))
    }
    sheet_locked(&sh.sheet, name)
        || doc
            .instance_source(sh, chain)
            .map(|src| sheet_locked(&src.sheet, name))
            .unwrap_or(false)
}

/// Title of the stencil list entry: the open file's stem (a .vssx
/// stencil is the whole document), else the document's own master set
/// (GitHub #53: the pane lists its stencil like Visio does).
/// `My Shapes` under a home directory (pure half of
/// [`App::my_shapes_dir`] for testing).
fn my_shapes_path(home: Option<PathBuf>) -> PathBuf {
    home.unwrap_or_else(|| PathBuf::from("."))
        .join("Documents")
        .join("My Shapes")
}

fn stencil_title(path: &Path) -> String {
    path.file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Document Masters".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn my_shapes_defaults_under_documents() {
        assert_eq!(
            my_shapes_path(Some(PathBuf::from("/home/bret"))),
            PathBuf::from("/home/bret/Documents/My Shapes")
        );
    }

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
            std::slice::from_ref(&p),
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
    fn corrupt_package_open_surfaces_an_error_and_pollutes_nothing() {
        // #97 item 8: a truncated/garbage .vsdx must degrade to a
        // status error — never a panic — and leave the session state
        // (recent files, the open document) untouched.
        let tmp = tempfile::tempdir().unwrap();
        let bad = tmp.path().join("corrupt.vsdx");
        std::fs::write(&bad, b"this is not a zip archive at all").unwrap();
        let mut app = App::with_blank();
        app.load(bad.clone());
        assert!(
            app.status.starts_with("open failed"),
            "status = {}",
            app.status
        );
        assert!(
            !app.debug_recent().iter().any(|p| p.ends_with("corrupt.vsdx")),
            "a corrupt open must not be recorded in recent files"
        );
        assert!(app.ed.is_some(), "the previously open document survives");
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
        let sess = app.text_edit.as_ref().expect("default opens the editor");
        assert_eq!((sess.page, sess.shape), (app.page_id().unwrap(), sid));
        assert_eq!(sess.draft, "", "draft seeds from the shape's text");
        app.end_text_edit(true);
        assert!(app.text_edit.is_none(), "a no-change exit applies nothing");

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
        app.stencil.armed = Some((StencilId::Document, master));
        app.on_canvas_click((4.0, 5.0), false, false);
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
