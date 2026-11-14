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
use dc_core::scope::{shape_chain, Scope};
use dc_core::{Editor, Macro, SetCell, SetRowCell};
use egui::{Color32, Context, Pos2, Rect, Sense, Stroke, Ui, Vec2};
use vsdx::model::{Document, Shape};

use crate::canvas::{self, Handle, HandleId, PageGeom, View};
use crate::cmd::MoveShape;
use crate::recent::RecentFiles;
use crate::ribbon::{Action, Icon, Tab, TABS};
use crate::stencil::{MasterKey, Stencil, StencilId, StencilSet};
use crate::svgimg::TextureCache;
use crate::theme;
use dc_core::connects::ConnectsHost;
use dc_core::AddMasterInstance;

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

/// #149 item 2: which Format-pane section a mini-toolbar Format icon
/// targets — the pane forces the section open and scrolls to it once,
/// then consumes the focus.
#[derive(Clone, Copy, PartialEq, Eq)]
enum FormatSection {
    Fill,
    Line,
    Text,
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
    /// #149 item 2: the pane section a mini-toolbar Format icon
    /// targets — forced open and scrolled to once, then consumed.
    format_pane_focus: Option<FormatSection>,
    /// The last section a Format icon targeted (diagnostic; unlike
    /// [`Self::format_pane_focus`] it persists after the pane
    /// consumed the one-shot scroll).
    format_pane_last_focus: Option<FormatSection>,
    /// Text buffers for the Format pane's numeric/text fields (#31).
    fmt_bufs: std::collections::HashMap<String, String>,
    /// Text buffers for the Shape Data string fields (#30) — commits
    /// on focus loss, one undo per change.
    prop_bufs: std::collections::HashMap<String, String>,
    /// The Shape Data "add choice" row is open for this variable-list
    /// (Type 4) Prop row (#30).
    prop_add: Option<String>,
    tab: Tab,
    /// #237: a pending zoom-to-selection (the ribbon action / Shift+Z)
    /// — the canvas frame centers + fills the selection's union bounds
    /// on the next pass, on the same view ledger as Fit Page.
    zoom_selection_pending: bool,
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
    /// #192 spec: the open ShapeSheet windows, one PER SHAPE (each
    /// its own child window; a shape appears at most once).
    shapesheets: Vec<i64>,
    /// #192 spec: the INACTIVE open documents — File > Open ADDS a
    /// document now; the Window switcher activates one.
    docs: Vec<DocSession>,
    /// #192: the Window switcher popup on the tab strip.
    winmenu_open: bool,
    /// #192 4a: a Window-menu pick to activate after the strip's
    /// borrow ends (the popup closure borrows self immutably).
    pending_activate: Option<usize>,
    /// Snap & glue settings (#109): the document's DocumentSettings,
    /// mirrored in UI state; every toggle writes through to the doc.
    snap: crate::snap::SnapSettings,
    /// The Snap & Glue dialog is open (View ▸ Visual Aids, #109).
    snap_dlg_open: bool,
    /// #246: the Language dialog is open.
    language_dlg_open: bool,
    /// #218 review: the Jump-size drag's draft — a DragValue commit
    /// per frame flooded history; the draft edits locally and commits
    /// once, when the drag stops (typed input commits immediately).
    line_jump_draft: Option<f32>,
    /// Dynamic-grid hints for the frame in progress (#109, URS-1088),
    /// recomputed by `update_drag` while a move drag is live and
    /// painted by the canvas from `DragFx::hints`.
    snap_live: Vec<crate::snap::Hint>,
    /// The last frame's connection-point affordances (#177): what the
    /// canvas painted — reveal symbols, glue-target highlight, glued
    /// sites of the selected connector. Kept as the test seam.
    conn_fx_last: crate::connectors::ConnPointFx,
    /// The active mouse tool (#108).
    tool: Tool,
    /// #110: align against the page instead of the first-selected
    /// shape (URS-1149's option).
    align_to_page: bool,
    /// The Select by Type dialog is open (#110, URS-1160).
    selby_open: bool,
    /// Select-by-Type criterion: 0 shape type, 1 master, 2 layer.
    selby_criterion: u8,
    /// Select-by-Type chosen value.
    selby_value: String,
    /// The Page Setup dialog is open (#113, URS-1122).
    pagesetup_open: bool,
    /// The Copy as Picture dialog is open (#113, URS-1200).
    copypic_open: bool,
    /// #113 remainder: the vector (SVG) flavor for copy-as-picture.
    copypic_svg: bool,
    /// #156: the Container Styles gallery dialog (Design ▸ Containers…).
    contgal_open: bool,
    /// #192 MDI: the central client rect (what `central`'s ui sees
    /// after the docked panels) — the region child windows constrain
    /// to. Captured every frame in `central`.
    mdi_client: egui::Rect,
    /// #192 spec: the child-window count last frame — any change
    /// re-fires the automatic arrangement (children tile to fit the
    /// client). While exactly ONE child exists it is decorator-free
    /// and pinned full-bleed ("treated as it is now").
    mdi_last_count: usize,
    mdi_arranging: bool,
    /// #192: the decorator/arrangement decisions for the drawing
    /// window, computed in `draw` AFTER central ran — central applies
    /// them on the frame after the count changes (one frame of lag,
    /// invisible: the count is stable by then).
    mdi_decorated: bool,
    mdi_drawing_tile: egui::Rect,
    /// Copy-as-picture quality: 1 screen, 2 high, 4 print (#113).
    copypic_factor: u32,
    /// Text waiting for the OS clipboard (#113, URS-1113): copied on
    /// the next frame's update (the copy path has no Context).
    os_clipboard_text: Option<String>,
    /// An in-flight OS clipboard image probe (#113): RGBA result.
    os_image_rx: Option<Receiver<Option<OsImage>>>,
    /// The Backgrounds dialog is open (#113, URS-1158).
    backgrounds_open: bool,
    /// Background-page underlay cache (#113): one raster per chain
    /// level, keyed (bg page, content version, ppi bucket) — the
    /// handle converts to a quad's TextureId per frame.
    bg_underlays: Vec<(i64, u64, u32, egui::TextureHandle)>,
    /// The last copy-as-picture PNG (test hook).
    last_picture: Option<Vec<u8>>,
    /// The Format Painter is armed for one target (#110, URS-1053):
    /// the source shape whose resolved formatting the next shape
    /// click receives.
    painter_armed: Option<i64>,
    /// #110 remainder: the painter stays armed across applications
    /// (Visio's double-click Format Painter) until Esc or a cancel.
    painter_sticky: bool,
    /// #108: the Connection Point tool's selected point — (shape, row
    /// ix); Delete removes the row.
    connpt_sel: Option<(i64, i64)>,
    /// #108: the line-jump hops painted on the last frame (test seam).
    jumps_last: usize,
    /// #226: whether the last frame painted the styled endpoint-drag
    /// ghost (test seam).
    last_endpoint_ghost: bool,
    /// Guide-glue pending on the live drag (#113): the guide the
    /// snapped axis came from and which pin axes to glue. Written by
    /// `apply_drag_snap` when glue-to-guides is on, consumed by the
    /// move commit as Visio's `PinX = Sheet.N!PinX` formulas.
    drag_glue: Option<(i64, bool, bool)>,
    /// #226: the live endpoint-drag preview — a document snapshot the
    /// per-frame ghost mutates through the SAME planner the release
    /// commits (what you drag is exactly what lands). Armed while a
    /// LineBegin/LineEnd handle drag runs, dropped when it ends.
    endpoint_preview: Option<EndpointPreview>,
    shot_frames: u32,
    /// #224: the splash moment's start — the branded boot overlay
    /// shows until loading::SPLASH_SECS elapse, then the shell takes
    /// over. Headless ctors pin None so kittest never sees it.
    splash: Option<std::time::Instant>,
    /// #224: the in-flight file open — the parse runs on a worker
    /// thread (the same pattern as the rfd pick); the overlay shows
    /// and the result lands through finish_load. The `Instant` drives
    /// the loading animation.
    loading: Option<InflightLoad>,
    /// #242: the document LANDED but its first render is still in
    /// flight — the scrim stays up (same card, "Rendering…") until the
    /// layer store drains and the minimum display elapses, so the
    /// reveal happens on the finished render. Carries the load-start
    /// Instant (the scrim clock) and the path (the final status).
    rendering: Option<(PathBuf, std::time::Instant)>,
    /// #242: the About window (the QAT Help button) — the Tarahi mark,
    /// wordmark, and the running version.
    about_open: bool,
    /// #242 test hook: inject a primary click into the next update's
    /// input (the splash skip reads raw input events, and the kittest
    /// widget clicks target the scrim's eaten rect).
    test_click: bool,
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
    /// Backstage Export page state (#35): format/scope/options/path
    /// and the in-flight worker — the pane module owns the domain,
    /// app.rs only forwards (owner's 2026-10-26 decomposition rule).
    export_pane: crate::export::ExportPane,
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
    /// The Quick Shapes membership menu (#143): the right-clicked
    /// master cell (source key) and the popup anchor.
    quick_menu: Option<((StencilId, i64), egui::Pos2)>,
    /// The window title last pushed to the OS (sent on change only —
    /// the viewport command is not free per frame). Tracked so
    /// headless tests can assert it (#67: the OS draws the title, the
    /// app band no longer duplicates it).
    window_title: String,
    /// Last frame's context-menu rect in screen coords; None while the
    /// menu is closed (kept for the placement tests, issue #49).
    ctx_menu_rect: Option<egui::Rect>,
    /// In-app shape clipboard (Cut/Copy/Paste within Tarahi; rich
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
    /// Per-user Quick Shapes membership (#143) — app-side, persisted
    /// beside the MRU; ephemeral in every headless constructor.
    quick_shapes: crate::quickshapes::QuickShapes,
    /// "Use relative paths for hyperlinks" per document (#71),
    /// app-side keyed by path (Visio keeps the toggle app-level).
    relative_paths: crate::hyperlink::RelativePaths,
    /// Default stencil set + category order (#105) — app config,
    /// never .vsdx content; ephemeral in every headless constructor.
    stencil_prefs: crate::stencil::prefs::StencilPrefs,
    /// Whether built-ins open from the extracted My Shapes files
    /// (#104). Production only: every headless constructor disables
    /// it so kittest never reads (or depends on) the user's real
    /// My Shapes — the embedded fallback keeps tests hermetic.
    stencil_disk: bool,
    /// The background first-run extraction (#104), polled for
    /// startup feedback (owner report 2026-10-30: the 43 MB write
    /// must neither block the first frame nor run silently).
    stencil_extract: Option<crate::stencil::disk::ExtractionHandle>,
    /// OS light/dark watcher (GitHub #52). Production only — the
    /// headless constructors leave it off so goldens are deterministic.
    os_theme: Option<crate::os_theme::OsThemeWatcher>,
    /// #184 Alt keytips: the armed mode plus this frame's badge
    /// captures (collected during ribbon layout, painted after the
    /// ribbon panels close — see `crate::keytips`).
    keytips: crate::keytips::Keytips,
}

/// Which cell a ribbon palette click targets.
#[derive(Clone, Copy, PartialEq)]
enum ColorTarget {
    Fill,
    FillBkgnd,
    Line,
    /// Character row-0 Color (#73): writes a section-row cell, not a
    /// plain one.
    Text,
}

/// #224: one in-flight document open — the picked path, the moment
/// the load began (the loading animation's clock), and the worker's
/// parse-result channel.
struct InflightLoad {
    path: PathBuf,
    started: std::time::Instant,
    rx: mpsc::Receiver<Result<vsdx::Document, String>>,
    /// The worker's result, drained from `rx` (#242): a result pulled
    /// while the minimum display still runs LIVES HERE — the channel
    /// has no push-back, so an early `try_recv` into a local dropped
    /// it and the next poll saw a dead channel.
    result: Option<Result<vsdx::Document, String>>,
}

enum PendingDialog {
    Open(Receiver<Option<PathBuf>>),
    SaveAs(Receiver<Option<PathBuf>>),
    /// "Open Existing Stencil…" (.vssx into the Shapes pane, #83).
    OpenStencil(Receiver<Option<PathBuf>>),
    /// Insert ▸ Pictures (#113): a picked PNG/JPEG.
    OpenPicture(Receiver<Option<PathBuf>>),
    /// Hyperlinks ▸ Address browse (#71): a picked target file for
    /// the draft's Address field.
    HyperlinkAddress(Receiver<Option<PathBuf>>),
    /// Backstage Export Browse… (#35): the picked output file.
    ExportSave(Receiver<Option<PathBuf>>),
}

#[derive(Clone, Copy)]
enum PendingKind {
    Open,
    SaveAs,
    /// "Open Existing Stencil…" from the Shapes pane (#83).
    OpenStencil,
    /// Insert ▸ Pictures (#113, URS-1012): a PNG/JPEG file picked to
    /// become a Foreign shape.
    OpenPicture,
    /// Hyperlinks ▸ Address browse (#71).
    HyperlinkAddress,
    /// Backstage Export page's Browse… (#35): the save dialog carries
    /// the pane's format so the extension filter matches.
    ExportSave(dc::export::Format),
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

/// #192 spec: one INACTIVE open document (multi-document sessions).
/// The active document lives in the App's own fields; switching swaps
/// the session in. View/scroll state resets on switch (4a) — the
/// per-document view state is slice 4b.
struct DocSession {
    ed: Editor,
    path: PathBuf,
    dirty: bool,
    shapesheets: Vec<i64>,
    /// #192 4b: the document's view (zoom/pan/unit/toggles), page
    /// selection, shape selection, and group-edit stack — switching
    /// loses nothing.
    view: View,
    page_idx: usize,
    sel: Option<(i64, i64)>,
    sel_set: Vec<(i64, i64)>,
    group_edit: Vec<i64>,
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

/// Snap distance in screen px (Visio's default snap strength feels
/// like this at 100 %): converted to page inches per frame by
/// `snap_ctx_of` (#109).
const SNAP_STRENGTH_PX: f32 = 8.0;
/// How close a connector endpoint must come to a shape before its
/// connection points reveal (#177) — page inches, generous like
/// Visio's approach affordance.
const CONN_REVEAL_IN: f64 = 0.6;

/// An OS clipboard image probe's payload: (w, h, RGBA bytes).
type OsImage = (u32, u32, Vec<u8>);

/// #156: the SDCONT style names live beside the Container Tools tab
/// layout now (`crate::ribbon::container_tools`).
use crate::ribbon::container_tools::CONTAINER_STYLES;

/// The active mouse tool (Visio's Home ▸ Tools): the pointer selects
/// and drags; the Connector Tool (#108) draws connectors; the Lasso
/// (#110, Visio's Lasso Select) trails a freehand selection. Esc
/// returns to the pointer, like Visio.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tool {
    #[default]
    Pointer,
    Connector,
    Lasso,
    /// #108: the Connection Point tool — click a shape to add a
    /// connection point, click an existing one to select it (Delete
    /// removes).
    ConnectionPoint,
    /// #132 (URS-1225): the Freeform tool — a sampled pen stroke
    /// becomes a new shape's editable Geometry rows.
    Freeform,
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

/// The #226 live endpoint-drag preview's state: the document snapshot
/// the per-frame ghost mutates, the #175 row-translation anchor (the
/// pre-drag END — the plan's row deltas are relative to it), and the
/// last raster so motionless frames reuse the texture.
struct EndpointPreview {
    doc: vsdx::Document,
    conn_id: i64,
    pre_end: (f64, f64),
    pre_beg: (f64, f64),
    /// #218 review: the drag anchor's offset — handle origin minus the
    /// press position at arm time. The COMMIT path moves the endpoint
    /// by the drag DELTA from its own origin (the grab may start
    /// HANDLE_GRAB px off-center); the preview must apply the same
    /// offset to the pointer or it paints where the release won't land.
    grab_off: (f64, f64),
    last_to: (f64, f64),
    tex: Option<egui::TextureHandle>,
    bbox: (f64, f64, f64, f64),
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
        // #246 i18n: AFTER the shared ctor (which pins English for the
        // headless path), the persisted language — or the system locale
        // on first run — selects the catalog before the first frame.
        crate::i18n::init_from_disk();
        // #224: the branded boot moment — production only (the headless
        // ctors stay splash-free for kittest determinism).
        app.splash = Some(std::time::Instant::now());
        #[cfg(target_os = "linux")]
        {
            app.os_theme = Some(crate::os_theme::OsThemeWatcher::spawn());
        }
        // Production entry only: adopt the persistent on-disk MRU
        // (tests and kittest constructors stay ephemeral so they never
        // touch the user's real config dir).
        app.recent = RecentFiles::open();
        app.quick_shapes = crate::quickshapes::QuickShapes::open();
        app.relative_paths = crate::hyperlink::RelativePaths::open();
        // Production entry only: built-ins open from the extracted
        // My Shapes files (#104) — headless constructors stay on the
        // embedded fallback so they never touch the user's home.
        app.stencil_disk = true;
        // #105: the default stencil set auto-opens with the app
        // (Visio's companion set), and More Shapes shows the
        // configured categories in the configured order.
        app.stencil_prefs = crate::stencil::prefs::StencilPrefs::open();
        app.apply_stencil_preferences();
        app.open_default_stencils();
        // #104: the built-in catalog also exists as real `.vssx`
        // files under My Shapes — extracted on a BACKGROUND thread
        // (missing or size-mismatched files rewritten and
        // digest-verified) so users can open, copy, and inspect them
        // like any stencil without the first run blocking or going
        // silent (owner report 2026-10-30). Best-effort: an
        // unwritable My Shapes degrades to the embedded bytes.
        app.stencil_extract = crate::stencil::disk::extract_catalog_async(Self::my_shapes_dir());
        if let Some(p) = path {
            // #242: the CLI entry's open rides the async flight like
            // every interactive open — the splash covers the boot and
            // the scrim the parse + first render; the frames pump the
            // poll from the first frame on. Routed through load() so
            // a .vssx still lands in the Shapes pane (#83, #218
            // review), not the editor.
            app.load(p);
        }
        app
    }

    /// Headless-friendly constructor (theme applied by the host UI loop).
    pub fn with_doc_opt(path: Option<PathBuf>) -> App {
        // Headless constructors pin English so kittest goldens never
        // move with the user's real language (#246).
        crate::i18n::pin_english();
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
            format_pane_focus: None,
            format_pane_last_focus: None,
            fmt_bufs: std::collections::HashMap::new(),
            tab: Tab::Home,
            zoom_selection_pending: false,
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
            shapesheets: Vec::new(),
            docs: Vec::new(),
            winmenu_open: false,
            pending_activate: None,
            snap: crate::snap::SnapSettings::default(),
            snap_dlg_open: false,
            language_dlg_open: false,
            line_jump_draft: None,
            snap_live: Vec::new(),
            conn_fx_last: crate::connectors::ConnPointFx::default(),
            drag_glue: None,
            endpoint_preview: None,
            tool: Tool::Pointer,
            align_to_page: false,
            selby_open: false,
            selby_criterion: 0,
            selby_value: String::new(),
            pagesetup_open: false,
            copypic_open: false,
            copypic_factor: 2,
            copypic_svg: false,
            contgal_open: false,
            mdi_client: egui::Rect::NOTHING,
            mdi_last_count: 0,
            mdi_arranging: false,
            mdi_decorated: false,
            mdi_drawing_tile: egui::Rect::NOTHING,
            os_clipboard_text: None,
            os_image_rx: None,
            backgrounds_open: false,
            bg_underlays: Vec::new(),
            last_picture: None,
            painter_armed: None,
            painter_sticky: false,
            connpt_sel: None,
            jumps_last: 0,
            last_endpoint_ghost: false,
            shot_frames: 0,
            // #224: headless default — no branded boot moment (the
            // same ephemeral rule as the MRU adoption below). The
            // production entry re-arms it in `App::new`.
            splash: None,
            loading: None,
            rendering: None,
            about_open: false,
            test_click: false,
            dialog: None,
            canvas_rect: Rect::NOTHING,
            backstage: false,
            backstage_page: BackPage::Info,
            backstage_search: String::new(),
            export_pane: crate::export::ExportPane::default(),
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
            quick_menu: None,
            window_title: String::new(),
            ctx_menu_rect: None,
            clipboard: None,
            text_edit: None,
            placed_last_click: false,
            color_pick: None,
            recent: RecentFiles::ephemeral(),
            quick_shapes: crate::quickshapes::QuickShapes::ephemeral(),
            relative_paths: crate::hyperlink::RelativePaths::ephemeral(),
            stencil_prefs: crate::stencil::prefs::StencilPrefs::ephemeral(),
            stencil_disk: false,
            stencil_extract: None,
            os_theme: None,
            keytips: crate::keytips::Keytips::new(),
        };
        if let Some(p) = path {
            // Headless: no frame loop to pump the poll — land the
            // parse synchronously (the interactive open paths use the
            // async load(); tests assert right after the ctor).
            let result = vsdx::parse::parse(&p).map_err(|e| format!("{e:#}"));
            let _ = app.finish_load(p, result);
        }
        app
    }

    /// Open a document at `path` (headless tests, `dc-ui <file>`).
    pub fn with_doc(path: &std::path::Path) -> App {
        let mut app = App::with_doc_opt(Some(path.to_path_buf()));
        app.splash = None; // headless: no branded boot moment
        app
    }

    /// A fresh blank drawing with the starter shape masters
    /// (headless tests, File → New).
    pub fn with_blank() -> App {
        let mut app = App::with_doc_opt(None);
        app.splash = None; // headless: no branded boot moment
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
    /// kittest: the saved state (a Save leaves the document clean).
    #[doc(hidden)]
    pub fn mark_clean_for_test(&mut self) {
        self.dirty = false;
    }
    /// #184 tests: the armed keytip level ("strip"/"controls"); None
    /// when disarmed.
    #[doc(hidden)]
    pub fn debug_keytip_mode(&self) -> Option<&'static str> {
        self.keytips.mode.map(|m| match m {
            crate::keytips::Mode::Strip => "strip",
            crate::keytips::Mode::Controls => "controls",
        })
    }
    /// #184 tests: the assigned keytip letter for a content-row
    /// control, looked up by its label in the current badge set.
    #[doc(hidden)]
    pub fn debug_keytip_letter(&self, label: &str) -> Option<char> {
        self.keytips.badges.iter().find_map(|b| {
            match &b.hit {
                crate::keytips::Hit::Action { label: l, .. } if l == label => {
                    b.key.chars().next()
                }
                _ => None,
            }
        })
    }
    /// #184 tests: the active ribbon tab (a strip keytip's effect).
    #[doc(hidden)]
    pub fn debug_tab(&self) -> Tab {
        self.tab
    }
    /// #184 tests: the View ▸ Rulers toggle (a control keytip's
    /// observable effect).
    #[doc(hidden)]
    pub fn debug_show_rulers(&self) -> bool {
        self.view.show_rulers
    }
    /// #192 4a tests: File > Open ADDS a document — parse the file
    /// and activate it, stashing the current one (the real load()
    /// path minus the file-dialog worker).
    #[doc(hidden)]
    pub fn debug_load_adds_for_test(&mut self, path: &std::path::Path) {
        // Synchronous landing (tests assert right after): the same
        // completion path the worker uses, without the flight.
        self.debug_load_sync_for_test(path);
    }

    /// #242: the synchronous open landing for unit tests — `load()`
    /// itself is the async starter now, and these tests assert
    /// immediately after the call.
    #[doc(hidden)]
    pub fn debug_load_sync_for_test(&mut self, path: &std::path::Path) {
        let result = vsdx::parse::parse(path).map_err(|e| format!("{e:#}"));
        let _ = self.finish_load(path.to_path_buf(), result);
    }

    /// #263 tests: the production CLI dispatch exactly as `App::new`
    /// runs it — a `.vssx` argument lands in the Shapes pane, anything
    /// else arms the async document flight.
    #[doc(hidden)]
    pub fn debug_load_for_test(&mut self, path: std::path::PathBuf) {
        self.load(path);
    }

    /// #192 4a tests: the open documents' file names (active first,
    /// then the inactive sessions).
    #[doc(hidden)]
    pub fn debug_open_doc_names(&self) -> Vec<String> {
        let title = |p: &std::path::Path| {
            p.file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| crate::i18n::strs().msg_untitled())
        };
        let mut names = vec![title(&self.path)];
        names.extend(self.docs.iter().map(|d| title(&d.path)));
        names
    }

    /// #192 4a tests: how many documents are open.
    #[doc(hidden)]
    pub fn debug_open_doc_count(&self) -> usize {
        self.docs.len() + self.ed.is_some() as usize
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
    /// kittest (#71): the next native dialog (any kind — e.g. the
    /// Hyperlinks Address browse) resolves with `answer` instead of
    /// opening rfd.
    #[doc(hidden)]
    pub fn preset_file_dialog_for_test(&mut self, answer: Option<PathBuf>) {
        self.dialog_preset_for_test = Some(answer);
    }
    /// kittest (#71): pretend the document was saved at `path` — the
    /// relative-hyperlink logic keys off the document's directory.
    #[doc(hidden)]
    pub fn debug_set_doc_path_for_test(&mut self, path: PathBuf) {
        self.path = path;
    }
    /// kittest (#35): the Export pane's armed output path.
    #[doc(hidden)]
    pub fn debug_export_pane_path(&self) -> &str {
        self.export_pane.path()
    }
    /// kittest (#35): the pane's last note (status of the worker).
    #[doc(hidden)]
    pub fn debug_export_pane_note(&self) -> Option<&str> {
        self.export_pane.note_for_test()
    }
    #[doc(hidden)]
    pub fn debug_close_prompt(&self) -> bool {
        self.confirm_close.is_some()
    }
    /// kittest (#166): the active ribbon tab — clicking a tab behind
    /// an open MODAL must not change it.
    #[doc(hidden)]
    pub fn debug_active_ribbon_tab(&self) -> String {
        format!("{:?}", self.tab)
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
        // (#83). Stencil loads are small and stay synchronous.
        if path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("vssx"))
        {
            self.open_stencil(path);
            return;
        }
        self.begin_async_load(path);
    }

    /// #224: the async open — the parse runs on a worker thread (the
    /// same pattern as the rfd pick) so a large .vsdx never freezes the
    /// frame loop. The overlay + status line cover the flight; the
    /// result lands through [`App::finish_load`] on a later frame. The
    /// parsed Document is plain data (Send); the editor — whose History
    /// owns `Box<dyn Command>` — is built on the main thread. EVERY
    /// document-open entry rides this (the Recent list and backstage
    /// opens used to stay synchronous — the owner's #242 report: the
    /// parse froze the UI and the loading scrim never showed).
    fn begin_async_load(&mut self, path: PathBuf) {
        if self.loading.is_some() {
            return; // one in-flight open at a time (modal for v1)
        }
        let (tx, rx) = mpsc::channel();
        let worker_path = path.clone();
        std::thread::spawn(move || {
            // A panicking parse must surface as a failed open, not a
            // dead channel (#242: the recent-entry fixture managed to
            // kill the worker invisibly).
            let result = std::panic::catch_unwind(|| vsdx::parse::parse(&worker_path))
                .unwrap_or_else(|p| {
                    Err(anyhow::anyhow!(
                        "the parse panicked: {}",
                        p.downcast_ref::<String>()
                            .cloned()
                            .or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string()))
                            .unwrap_or_else(|| "unknown panic".into())
                    ))
                })
                .map_err(|e| format!("{e:#}"));
            let _ = tx.send(result);
        });
        self.status = format!(
            "Opening {}…",
            path.file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.display().to_string())
        );
        self.loading = Some(InflightLoad {
            path,
            started: std::time::Instant::now(),
            rx,
            result: None,
        });
    }

    /// The shared open-completion path: the #224 worker's landing and
    /// the synchronous callers (tests, Recent lists) all end here.
    /// Lands one worker result; the return says whether an EDITOR
    /// landed — a failed parse or the #263 no-pages rejection both
    /// refuse (with a status note) and their callers must not enter
    /// the render-completion scrim (PR #269 review: an async no-pages
    /// open kept the loading scrim up forever — nothing renders, so
    /// the drain never closes it).
    fn finish_load(&mut self, path: PathBuf, result: Result<vsdx::Document, String>) -> bool {
        match result {
            Ok(ed) => {
                // #263: a package that parses but carries NO pages (the
                // reporter's minimal hand-authored file — one master, an
                // empty Pages index) used to land in the editor and panic
                // the first frame at `pages[0]`; on Windows that was an
                // instant silent exit, no window, no error. The UI refuses
                // to delete the last page, so opens are the only way a
                // pages-empty document can exist — reject it here (every
                // document-open entry lands through this fn) with a status
                // note, and the app stays up.
                if ed.pages.is_empty() {
                    self.status = crate::i18n::strs().msg_open_failed_no_pages(
                        path.display().to_string(),
                    );
                    return false;
                }
                self.status = crate::i18n::strs().msg_opened_path(path.display().to_string());
                self.recent.record(&path);
                // #192 spec: opening ADDS a document — the active one
                // (if any) becomes an inactive session first.
                self.stash_active_doc();
                self.ed = Some(Editor::new(ed));
                self.reload_snap_settings();
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
                true
            }
            Err(e) => {
                self.status = crate::i18n::strs().msg_open_failed_e(e);
                false
            }
        }
    }

    /// #224: consume a finished file-open worker result. `Some(path)`
    /// means still in flight (the caller draws the overlay + the
    /// status heartbeat); the landing goes through
    /// [`App::finish_load`].
    fn poll_loading(&mut self) -> Option<(PathBuf, std::time::Instant)> {
        let mut load = self.loading.take()?;
        // Drain the worker's result into the flight's own slot (#242):
        // a value pulled early stays on the flight until the minimum
        // display elapses, instead of being dropped (a bare try_recv
        // into a local left the channel empty and every later poll
        // reported a dead worker).
        if load.result.is_none() {
            match load.rx.try_recv() {
                Ok(r) => load.result = Some(r),
                Err(mpsc::TryRecvError::Empty) => {}
                // The sender is gone: the value (if any) is already in
                // the slot; an empty slot with a dead sender is a dead
                // worker.
                Err(mpsc::TryRecvError::Disconnected) if load.result.is_none() => {
                    self.status = crate::i18n::strs().msg_open_failed_the_load_worker_died();
                    return None;
                }
                Err(_) => {}
            }
        }
        // Land only when the result is here AND the scrim showed its
        // minimum window (#242).
        if load.result.is_some()
            && load.started.elapsed().as_secs_f32() >= crate::loading::LOADING_MIN_SECS
        {
            let result = load.result.take().expect("result gated the landing");
            let path = load.path.clone();
            let loaded = self.finish_load(path.clone(), result);
            // The first render is the heavy half (every shape rasters +
            // lands): keep the scrim up until the layer store drains
            // (#242 — "run until render is complete"). A FAILED open
            // must not enter it — its drain would overwrite the error
            // status with "opened …" (#218 review).
            if loaded {
                self.rendering = Some((path, load.started));
            }
            return None;
        }
        self.status = format!(
            "Opening {}…",
            load.path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| load.path.display().to_string())
        );
        let (p, s) = (load.path.clone(), load.started);
        self.loading = Some(load);
        Some((p, s))
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
        // Field-disjoint borrows (ed/layers vs status): the #219
        // damage routing needs the editor document alongside the
        // layer store, so this site inlines page_id().
        let Some(ed) = self.ed.as_mut() else { return };
        // #219: capture the edited sub-shapes' PRE-edit ink bounds
        // from the pre-apply document — the old half of each damage
        // rect. A failed apply drops them (nothing routed).
        let pre = self
            .layers
            .harvest_pre_damage(&ed.doc, &cmd.touched(&ed.doc));
        match ed.apply(cmd) {
            Ok((label, touched)) => {
                let page = ed.doc.pages[self.page_idx.min(ed.doc.pages.len().saturating_sub(1))].id;
                self.layers.touch_damage(&ed.doc, &touched, &pre, page);
                self.status = label;
                self.dirty = true;
                self.version += 1; // Info thumbnail (whole page)
            }
            Err(e) => self.status = format!("{e:#}"),
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
                        .filter(|(_, s)| vsdx::model::find_shape_by_id(&page.shapes, *s).is_some())
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
        if let Some(ed) = self.ed.as_mut() {
            match ed.undo() {
                Ok(Some((l, touched))) => {
                    // #219: no pre-apply harvest here — the undo pairs
                    // against the per-transition bounds cache, which
                    // holds exactly the state being reverted to.
                    // (Field-disjoint borrows; page_id() is inlined.)
                    let page =
                        ed.doc.pages[self.page_idx.min(ed.doc.pages.len().saturating_sub(1))].id;
                    self.layers.touch_damage(&ed.doc, &touched, &[], page);
                    self.status = crate::i18n::strs().msg_undo_l(l);
                    self.dirty = true;
                    self.version += 1;
                    self.restore_selection_after_history(&touched);
                }
                Ok(None) => self.status = crate::i18n::strs().msg_nothing_to_undo(),
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
        // An in-place text edit ends on the page switch (Visio commits
        // on the focus change; #22 review) — before the view parks, so
        // the parked view is the pre-edit one.
        if self.text_edit.is_some() {
            self.end_text_edit(true);
        }
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
            self.status = crate::i18n::strs().msg_the_last_page_cannot_be_deleted();
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
        if let Some(ed) = self.ed.as_mut() {
            match ed.redo() {
                Ok(Some((l, touched))) => {
                    // #219: redo pairs against the bounds cache, like
                    // undo (field-disjoint borrows; page_id() inlined).
                    let page =
                        ed.doc.pages[self.page_idx.min(ed.doc.pages.len().saturating_sub(1))].id;
                    self.layers.touch_damage(&ed.doc, &touched, &[], page);
                    self.status = crate::i18n::strs().msg_redo_l(l);
                    self.dirty = true;
                    self.version += 1;
                    // Same selection mapping as undo (#136) — redo never
                    // touched the selection before, so a redo-of-delete
                    // could leave a selection pointing at a removed shape.
                    self.restore_selection_after_history(&touched);
                }
                Ok(None) => self.status = crate::i18n::strs().msg_nothing_to_redo(),
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
    /// 2026-10-27): `<home>/Documents/My Shapes`. PURE — no directory
    /// is created here (kittest reaches this through the built-in
    /// menu, #104); the extraction pass and the Open Existing Stencil
    /// dialog create it on demand. None when no home directory is
    /// known (neither HOME nor USERPROFILE).
    fn my_shapes_dir() -> Option<std::path::PathBuf> {
        let home = std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .map(std::path::PathBuf::from)?;
        Some(my_shapes_path(Some(home)))
    }

    fn open_stencil(&mut self, path: PathBuf) {
        let stem = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| crate::i18n::strs().msg_stencil_fallback());
        match self.stencils.open_file(&path) {
            Ok((ix, existing)) => {
                self.stencil.reveal = Some(StencilId::Open(ix));
                self.status = if existing {
                    crate::i18n::strs().msg_stencil_stem_is_already_open_exp(stem)
                } else {
                    crate::i18n::strs().msg_opened_stencil_stem(stem)
                };
            }
            Err(e) => self.status = crate::i18n::strs().msg_open_stencil_failed_e(format!("{e:#}")),
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
        // The dedupe key matches however the entry was opened: the
        // extracted file (#104) or the `builtin/<slug>` fallback.
        let my_shapes = self.stencil_disk.then(Self::my_shapes_dir).flatten();
        let source = crate::stencil::disk::extracted_path(
            my_shapes
                .as_deref()
                .unwrap_or_else(|| std::path::Path::new("")),
            b.slug,
        );
        let fallback = crate::stencil::builtin::source_path(b.slug);
        if let Some(ix) = self
            .stencils
            .position_for(&source)
            .or_else(|| self.stencils.position_for(&fallback))
        {
            let title = self
                .stencils
                .get(ix)
                .map(|st| st.title.clone())
                .unwrap_or_else(|| b.title.to_string());
            self.stencil.close_entry(&mut self.stencils, ix);
            self.status = crate::i18n::strs().msg_closed_title(title);
            return;
        }
        self.open_builtin_stencil(b);
    }

    /// "More Shapes ▸ My Shapes ▸ <stencil>" (#266): open (or focus)
    /// the file through the user-stencil path — exactly a picker
    /// result: title = file stem, pane identity = the file path — or
    /// close its entry when it is already open (the menu checkbox
    /// state, the built-in categories' semantics). An unreadable
    /// stencil surfaces as a status note, like other load failures.
    fn toggle_my_shapes_stencil(&mut self, path: PathBuf) {
        if let Some(ix) = self.stencils.position_for(&path) {
            let title = self
                .stencils
                .get(ix)
                .map(|st| st.title.clone())
                .unwrap_or_else(|| path.to_string_lossy().into_owned());
            self.stencil.close_entry(&mut self.stencils, ix);
            self.status = crate::i18n::strs().msg_closed_title(title);
            return;
        }
        self.open_stencil(path);
    }

    /// Open one built-in catalog stencil (#87), DISK-FIRST (#104):
    /// the extracted `.vssx` under My Shapes when it is there (the
    /// file identity — the same entry the user gets opening that
    /// file), else the embedded bytes with the `builtin/<slug>`
    /// pseudo-path. Title and origin stay the catalog's, so menu
    /// provenance is kept either way.
    fn open_builtin_stencil(&mut self, b: &'static crate::stencil::builtin::BuiltinStencil) {
        // Which variant opens is the pane's session preference (#85
        // follow-up): US by default, the metric twin with "Metric
        // Stencils" ticked.
        let metric = self.stencil.metric_units;
        let my_shapes = self.stencil_disk.then(Self::my_shapes_dir).flatten();
        let opened = crate::stencil::disk::open_from_disk(my_shapes.as_deref(), b, metric)
            .map(|(doc, source)| (doc, source, true))
            .or_else(|| {
                vsdx::parse::parse_bytes(b.bytes_for(metric))
                    .ok()
                    .map(|doc| (doc, crate::stencil::builtin::source_path(b.slug), false))
            });
        match opened {
            Some((doc, source, from_disk)) => {
                let (ix, _) = self.stencils.open_parsed(
                    doc,
                    source,
                    b.title.to_string(),
                    crate::stencil::Origin::Builtin,
                );
                self.stencil.reveal = Some(StencilId::Open(ix));
                self.status = if from_disk {
                    crate::i18n::strs().msg_opened_builtin_myshapes(b.title)
                } else if metric {
                    crate::i18n::strs().msg_opened_builtin_metric(b.title)
                } else {
                    format!("opened {title}", title = b.title)
                };
            }
            None => {
                self.status = crate::i18n::strs().msg_builtin_stencil_failed(b.title);
            }
        }
    }

    /// Apply the configured category list/order to the pane (#105):
    /// the configured subset, or the catalog's manifest order when
    /// unset.
    fn apply_stencil_preferences(&mut self) {
        self.stencil.builtin_categories = match self.stencil_prefs.categories.clone() {
            Some(cats) => cats,
            None => crate::stencil::builtin::categories()
                .into_iter()
                .map(String::from)
                .collect(),
        };
    }

    /// Open the default stencil set (#105): built-in slugs through
    /// the disk-first path (#104), plus files named under My Shapes
    /// opened like user stencils. Dedupe keeps a re-run cheap (and
    /// the startup path never toggles anything closed).
    fn open_default_stencils(&mut self) {
        let my_shapes = self.stencil_disk.then(Self::my_shapes_dir).flatten();
        for entry in self.stencil_prefs.default_open.clone() {
            if let Some(b) = crate::stencil::builtin::by_slug(&entry) {
                self.open_builtin_stencil(b);
                continue;
            }
            let Some(dir) = &my_shapes else {
                continue;
            };
            // A My Shapes file name (config entries are trusted —
            // the user's own config file).
            let p = dir.join(entry.trim_start_matches(['/', '\\']));
            if p.is_file() {
                if let Err(e) = self.stencils.open_file(&p) {
                    self.status = crate::i18n::strs().msg_default_stencil_failed(
                        format!("{entry:?}"),
                        format!("{e:#}"),
                    );
                }
            }
        }
        // Startup opens land at the pane top; no reveal-scroll.
        self.stencil.reveal = None;
    }

    /// kittest (#105): run the startup preference application against
    /// the current (ephemeral-by-default) prefs.
    #[doc(hidden)]
    pub fn apply_stencil_defaults_for_test(&mut self) {
        self.apply_stencil_preferences();
        self.open_default_stencils();
    }

    /// kittest (#105): swap the prefs (e.g. a custom default set /
    /// category order) before applying.
    #[doc(hidden)]
    pub fn set_stencil_prefs_for_test(&mut self, prefs: crate::stencil::prefs::StencilPrefs) {
        self.stencil_prefs = prefs;
    }

    /// kittest (#105/#104): the open stencils' titles, in pane order.
    #[doc(hidden)]
    pub fn debug_open_stencil_titles(&self) -> Vec<String> {
        self.stencils
            .iter()
            .map(|(_, st)| st.title.clone())
            .collect()
    }

    /// kittest (#105): the category list More Shapes shows, in order.
    #[doc(hidden)]
    pub fn debug_builtin_categories(&self) -> Vec<String> {
        self.stencil.builtin_categories.clone()
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
        self.reload_snap_settings();
        self.path = PathBuf::new();
        self.dirty = false;
        self.version += 1;
        self.view = View::default();
        self.page_idx = 0;
        self.set_selection(None);
        self.group_edit.clear();
        self.stencil = Stencil::default();
        // The reset pane drops the configured category order (#105,
        // CodeRabbit): re-apply the stencil preferences right after.
        self.apply_stencil_preferences();
        self.tex.clear();
        self.layers.clear();
        self.page_states.clear();
        self.tab_menu = None;
        self.tab_rename = None;
        self.reorder_open = false;
        self.comment_target = None;
        self.comment_edit = None;
        self.status = crate::i18n::strs().msg_new_blank_drawing_unsaved();
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
        // The Export pane's suggested file (drawing.svg / .png / .pdf,
        // #35) — the save dialog opens on it.
        // Path::file_name handles both separators (#218 review) — a
        // backslash path would survive rsplit('/') whole.
        let default_export_name = std::path::Path::new(self.export_pane.path())
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "drawing.svg".to_string());
        let (tx, rx) = mpsc::channel();
        // The native dialog is untestable headless (rfd opens GTK);
        // an armed preset answers in its place (tests only).
        let preset = self.dialog_preset_for_test.take();
        std::thread::spawn(move || {
            let picked = match preset {
                Some(p) => p,
                None => match kind {
                    PendingKind::Open => rfd::FileDialog::new()
                        .add_filter(crate::i18n::strs().msg_visio_vsdx_family(), &["vsdx", "vstx", "vssx"])
                        .pick_file(),
                    PendingKind::OpenStencil => {
                        let mut dlg = rfd::FileDialog::new().add_filter(crate::i18n::strs().msg_visio_stencil(), &["vssx"]);
                        if let Some(dir) = Self::my_shapes_dir() {
                            let _ = std::fs::create_dir_all(&dir);
                            dlg = dlg.set_directory(dir);
                        }
                        dlg.pick_file()
                    }
                    PendingKind::SaveAs => rfd::FileDialog::new()
                        .add_filter(crate::i18n::strs().msg_visio_drawing(), &["vsdx", "vstx", "vssx"])
                        .set_file_name(&default_name)
                        .save_file(),
                    PendingKind::OpenPicture => rfd::FileDialog::new()
                        .add_filter(crate::i18n::strs().msg_pictures(), &["png", "jpg", "jpeg"])
                        .pick_file(),
                    // Any file can be a hyperlink target (#71).
                    PendingKind::HyperlinkAddress => rfd::FileDialog::new().pick_file(),
                    PendingKind::ExportSave(format) => {
                        // bound so the catalog Strings outlive the call
                        let label = match format {
                            dc::export::Format::Svg => crate::i18n::strs().msg_filter_svg(),
                            dc::export::Format::Png => crate::i18n::strs().msg_filter_png(),
                            dc::export::Format::Pdf => crate::i18n::strs().msg_filter_pdf(),
                        };
                        rfd::FileDialog::new()
                            .add_filter(label.as_str(), format.extensions())
                    }
                        .set_file_name(&default_export_name)
                        .save_file(),
                },
            };
            let _ = tx.send(picked);
        });
        self.dialog = Some(match kind {
            PendingKind::Open => PendingDialog::Open(rx),
            PendingKind::OpenStencil => PendingDialog::OpenStencil(rx),
            PendingKind::SaveAs => PendingDialog::SaveAs(rx),
            PendingKind::OpenPicture => PendingDialog::OpenPicture(rx),
            PendingKind::HyperlinkAddress => PendingDialog::HyperlinkAddress(rx),
            PendingKind::ExportSave(_) => PendingDialog::ExportSave(rx),
        });
    }

    /// Poll the background stencil extraction (#104): progress lands
    /// in the status bar — the one piece of UI every shell state
    /// shows — with a repaint heartbeat so it advances without user
    /// input, and a completion note when files were actually written.
    fn poll_stencil_extraction(&mut self, ctx: &Context) {
        let Some(h) = self.stencil_extract.take() else {
            return;
        };
        // Drain everything ready this frame (the writer is fast).
        match h.poll() {
            Some(crate::stencil::disk::ExtractionMsg::Progress { done, total }) => {
                self.status =
                    crate::i18n::strs().msg_extracting_built_in_stencils_to(done, total);
                self.stencil_extract = Some(h);
                ctx.request_repaint_after(Duration::from_millis(100));
            }
            Some(crate::stencil::disk::ExtractionMsg::Done(e)) => {
                if e.failed > 0 {
                    self.status = crate::i18n::strs()
                        .msg_builtin_extracted_summary(e.written, e.failed);
                } else if e.written > 0 {
                    self.status = crate::i18n::strs()
                        .msg_builtin_extracted_to(e.written);
                }
                // Nothing written (files already current): no noise.
            }
            None => {
                // Nothing ready yet — keep waiting with a heartbeat.
                self.stencil_extract = Some(h);
                ctx.request_repaint_after(Duration::from_millis(100));
            }
        }
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
            | PendingDialog::OpenStencil(rx)
            | PendingDialog::OpenPicture(rx)
            | PendingDialog::ExportSave(rx)
            | PendingDialog::HyperlinkAddress(rx) => match rx.try_recv() {
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
            // #224/#242: the picked-file parse runs on the worker via
            // load() — the scrim's minimum display keeps it visible;
            // a stencil still lands in the Shapes pane synchronously
            // (#83, the routing inside load()).
            PendingDialog::Open(_) => self.load(path),
            PendingDialog::OpenStencil(_) => self.open_stencil(path),
            PendingDialog::OpenPicture(_) => self.insert_picture_file(&path),
            // The picked export target lands in the pane's path field
            // (#35); a typed/picked extension re-targets the format.
            PendingDialog::ExportSave(_) => self.export_pane.set_path(path.clone()),
            // The picked target lands in the open draft's Address
            // field (#71), stored the way the document's toggle wants
            // it (a path inside the document's directory shows as its
            // relative form when the toggle is on — what 2016 shows).
            PendingDialog::HyperlinkAddress(_) => {
                let relative = self.relative_paths.get(&self.path);
                let stored = crate::hyperlink::store_address(
                    &path.to_string_lossy(),
                    crate::hyperlink::doc_dir(&self.path),
                    relative,
                );
                if let Some(d) = self.hyperlink_draft.as_mut() {
                    d.address = stored;
                }
            }
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
                self.status = crate::i18n::strs().msg_saved_path(path.display().to_string());
                self.dirty = false;
                self.path = path.to_path_buf();
                self.recent.record(path);
            }
            Err(e) => self.status = crate::i18n::strs().msg_save_failed_e(format!("{e:#}")),
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
    /// Normalized: the corpus connector model carries SIGNED
    /// Width/Height (`GUARD(EndX-BeginX)`), so the origin is min-corner
    /// and the extents absolute (#108).
    fn shape_rect_static(doc: &Document, page_id: i64, s: &Shape) -> Option<(f64, f64, f64, f64)> {
        let chain = shape_chain(doc, page_id, s.id).ok()?;
        let get = |n: &str| doc.resolve_cell(s, &chain, n).and_then(|c| c.numeric());
        let (pin_x, pin_y) = (get("PinX")?, get("PinY")?);
        let w = get("Width").unwrap_or(1.0);
        let h = get("Height").unwrap_or(1.0);
        let lx = get("LocPinX").unwrap_or(w / 2.0);
        let ly = get("LocPinY").unwrap_or(h / 2.0);
        let (x, y) = (pin_x - lx, pin_y - ly);
        Some((x.min(x + w), y.min(y + h), w.abs(), h.abs()))
    }

    fn shape_hit(&self, page_pos: (f64, f64)) -> Option<i64> {
        let ed = self.ed()?;
        // Guides sit above shapes in hit priority (#113): a press on
        // the line (within the screen threshold) grabs the guide, per
        // Visio. Not while group-editing — guides are page furniture.
        if self.group_edit.is_empty() {
            let threshold = 5.0 / self.view.px_per_in() as f64;
            if let Some(g) = crate::guides::guide_hit(&self.page_guides(), page_pos, threshold) {
                return Some(g.id);
            }
        }
        let page = &ed.doc.pages[self.page_idx.min(ed.doc.pages.len().saturating_sub(1))];
        // Connectors hit by their INK, not a degenerate bbox (#108 —
        // the corpus model's bbox can be a zero-height band). #172: the
        // ink follows the GEOMETRY rows — the Begin→End chord first
        // (the common straight route), then the row walk (arcs leave
        // the chord; a belly click would otherwise land on whatever
        // shape's bbox overlaps it). Master instances count (#172):
        // `ObjType=2` often lives on the master.
        if self.group_edit.is_empty() {
            let threshold = 5.0 / self.view.px_per_in() as f64;
            for shape in page.shapes.iter().rev() {
                if let Ok(chain) = shape_chain(&ed.doc, page.id, shape.id) {
                    if !crate::connectors::is_connector_in(&ed.doc, &chain, shape) {
                        continue;
                    }
                    let g = |n: &str| {
                        ed.doc
                            .resolve_cell(shape, &chain, n)
                            .and_then(|c| c.numeric())
                    };
                    if let (Some(bx), Some(by), Some(ex), Some(ey)) =
                        (g("BeginX"), g("BeginY"), g("EndX"), g("EndY"))
                    {
                        if segment_hit((bx, by), (ex, ey), page_pos, threshold) {
                            return Some(shape.id);
                        }
                    }
                    // The geometry walk (master root rows when the
                    // instance carries none), mapped through the
                    // chain-resolved bbox.
                    let (root, msize) = shape
                        .master
                        .and_then(|mid| {
                            ed.doc.master(mid).and_then(|m| {
                                m.shapes.first().map(|root| {
                                    let size = (
                                        root.sheet
                                            .cell("Width")
                                            .and_then(|c| c.numeric())
                                            .unwrap_or(1.0),
                                        root.sheet
                                            .cell("Height")
                                            .and_then(|c| c.numeric())
                                            .unwrap_or(1.0),
                                    );
                                    (root, size)
                                })
                            })
                        })
                        .map(|(r, s)| (Some(r), s))
                        .unwrap_or((None, (1.0, 1.0)));
                    // #179: the full placement frame (rotation +
                    // flips) — a rotated connector's ink lives outside
                    // its unrotated rect, so the gate uses the
                    // rotated corner union.
                    let frame = crate::connectors::hit_frame(&ed.doc, &chain, shape, msize);
                    let (bx0, by0, bx1, by1) = frame.page_bounds();
                    if page_pos.0 >= bx0 - threshold
                        && page_pos.0 <= bx1 + threshold
                        && page_pos.1 >= by0 - threshold
                        && page_pos.1 <= by1 + threshold
                    {
                        let segs = crate::connectors::geometry_hit_segments(shape, root, &frame);
                        for (a, b) in segs {
                            if segment_hit(a, b, page_pos, threshold) {
                                return Some(shape.id);
                            }
                        }
                    }
                }
            }
        }
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
            // #179: a rotated shape's claim uses its ROTATED bounds —
            // the unrotated Pin/Width/Height rect of a rotated box is
            // far bigger than the visible ink and steals clicks from
            // thin connectors crossing the corners (utilisation.vsdx).
            let rotated = shape
                .sheet
                .cell("Angle")
                .and_then(|c| c.numeric())
                .filter(|a| a.abs() > 1e-6)
                .is_some()
                || shape_chain(&ed.doc, page.id, shape.id)
                    .ok()
                    .and_then(|ch| {
                        ed.doc
                            .resolve_cell(shape, &ch, "Angle")
                            .and_then(|c| c.numeric())
                    })
                    .is_some_and(|a| a.abs() > 1e-6);
            if rotated {
                let Ok(chain) = shape_chain(&ed.doc, page.id, shape.id) else {
                    continue;
                };
                let (x0, y0, x1, y1) =
                    crate::connectors::hit_frame(&ed.doc, &chain, shape, (1.0, 1.0)).page_bounds();
                if page_pos.0 >= x0 && page_pos.0 <= x1 && page_pos.1 >= y0 && page_pos.1 <= y1 {
                    return Some(shape.id);
                }
            } else if let Some((x, y, w, h)) = self.shape_rect(&ed.doc, page.id, shape) {
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

    /// The selection's union bounds in page inches y-up `(x, y, w, h)`
    /// (#237): the primary selection plus every multi-select member
    /// (the same shape rects the selection frames draw). Selected
    /// guides draw as their overlay and carry no frame, so they do
    /// not zoom-target either.
    fn sel_union_page_rect(&self) -> Option<(f64, f64, f64, f64)> {
        let ed = self.ed.as_ref()?;
        let (page_id, primary) = self.sel?;
        let page = ed.doc.pages.get(self.page_idx)?;
        let rect_of = |sid: i64| -> Option<(f64, f64, f64, f64)> {
            let sh = vsdx::model::find_shape_by_id(&page.shapes, sid)?;
            if crate::guides::is_guide(sh) {
                return None;
            }
            // The nested-aware rect (#218 review round 3): a
            // group-edit selection's members carry parent-local
            // cells, and the raw Pin/Width read zooms to the wrong
            // spot.
            self.shape_rect(&ed.doc, page_id, sh)
        };
        let mut acc = rect_of(primary);
        for (p, sid) in &self.sel_set {
            if *p != page_id {
                continue;
            }
            if let Some(r) = rect_of(*sid) {
                acc = Some(match acc {
                    None => r,
                    Some((ax, ay, aw, ah)) => {
                        let (bx, by, bw, bh) = r;
                        let x0 = ax.min(bx);
                        let y0 = ay.min(by);
                        let x1 = (ax + aw).max(bx + bw);
                        let y1 = (ay + ah).max(by + bh);
                        (x0, y0, x1 - x0, y1 - y0)
                    }
                });
            }
        }
        acc
    }

    fn sel_page_rect(&self) -> Option<Rect> {
        let ed = self.ed()?;
        let (page_id, shape_id) = self.sel?;
        let page = ed.doc.pages.iter().find(|p| p.id == page_id)?;
        let shape = vsdx::model::find_shape_by_id(&page.shapes, shape_id)?;
        // A selected guide draws as the guide overlay itself (#113):
        // no selection frame, no handles.
        if crate::guides::is_guide(shape) {
            return None;
        }
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
        // Group-edit mode (#95): member resize/endpoint handles work —
        // 2D corners come from `sel_page_rect`, which already maps a
        // member's parent-local rect through the renderer's XForm chain
        // to page space; 1D endpoints map their parent-local cells to
        // page below. The commit-side deltas convert through the same
        // chain (apply_resize / apply_line_endpoint).
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
        // Guides drag by their body (#113) — no resize/endpoint
        // handles (tilting a guide is not a thing).
        if crate::guides::is_guide(sh) {
            return Vec::new();
        }
        let nested = chain.len() >= 2;
        let page_h = Self::page_height(ed, page_id);
        let to_page = |p: (f64, f64)| -> (f64, f64) {
            if nested {
                dc::svg::parent_local_point_to_page(&ed.doc, &chain, page_h, p)
            } else {
                p
            }
        };
        let get = |n: &str| ed.doc.resolve_cell(sh, &chain, n).and_then(|c| c.numeric());
        if let (Some(bx), Some(by), Some(ex), Some(ey)) =
            (get("BeginX"), get("BeginY"), get("EndX"), get("EndY"))
        {
            // #202: a CubicBezTo row (the curved route) reshapes the
            // handle set — the midpoint seats ON the curve (t=0.5;
            // the chord midpoint hangs off it) and both Bézier
            // controls become draggable curve handles, positioned and
            // later committed through the same HitFrame resolution
            // the renderer and hit walk use.
            let msize = sh
                .master
                .and_then(|mid| ed.doc.master(mid))
                .and_then(|m| m.shapes.first())
                .map(|r| {
                    let g = |n: &str| {
                        ed.doc
                            .resolve_cell(r, &[r], n)
                            .and_then(|c| c.numeric())
                            .unwrap_or(1.0)
                    };
                    (g("Width"), g("Height"))
                })
                .unwrap_or((1.0, 1.0));
            let frame = crate::connectors::hit_frame(&ed.doc, &chain, sh, msize);
            let cubic = crate::connectors::cubic_row(sh, &frame);
            // #226: the arc masters (the corpus Line-arc connector) seat
            // the midpoint ON the rendered curve the same way — the
            // quadratic's t=0.5 through the same resolution the ink
            // draws with. Both mids are shape-local.
            let master_root = sh
                .master
                .and_then(|mid| ed.doc.master(mid))
                .and_then(|m| m.shapes.first());
            let arc = crate::connectors::arc_row(sh, master_root, &frame);
            let mid_pos = if let Some(c) = cubic.as_ref() {
                frame.to_page(c.mid())
            } else if let Some(a) = arc.as_ref() {
                frame.to_page(a.mid())
            } else {
                to_page(((bx + ex) / 2.0, (by + ey) / 2.0))
            };
            let mut out = vec![
                Handle {
                    id: HandleId::LineBegin,
                    pos: to_page((bx, by)),
                },
                Handle {
                    id: HandleId::LineEnd,
                    pos: to_page((ex, ey)),
                },
                Handle {
                    id: HandleId::LineMid,
                    pos: mid_pos,
                },
            ];
            if let Some(c) = cubic {
                out.push(Handle {
                    id: HandleId::GeoPoint {
                        section_ix: c.section_ix,
                        row_ix: c.row_ix,
                        cell: canvas::GeoCell::Ctrl,
                    },
                    pos: frame.to_page(c.ctrl1),
                });
                out.push(Handle {
                    id: HandleId::GeoPoint {
                        section_ix: c.section_ix,
                        row_ix: c.row_ix,
                        cell: canvas::GeoCell::Ctrl2,
                    },
                    pos: frame.to_page(c.ctrl2),
                });
            }
            // #203: spline-family rows on a connector — SplineStart/
            // SplineKnot/NURBSTo are point rows; their resolved
            // endpoints (and the NURBSTo E formula's interior
            // points) join the handles through the same frame. Own
            // rows only (master-inherited rows have no local cells
            // to drag — the #137 rule).
            for geo in sh.sheet.sections_named("Geometry") {
                for row in &geo.rows {
                    let Some(rt) = row.t.as_deref() else {
                        continue;
                    };
                    if !matches!(rt, "SplineStart" | "SplineKnot" | "NURBSTo") {
                        continue;
                    }
                    let cell = |n: &str| -> f64 {
                        crate::connectors::row_cell_value(row.cell(n), None, 1.0, &frame.env)
                            .unwrap_or(0.0)
                    };
                    let (rx, ry) = (cell("X"), cell("Y"));
                    out.push(Handle {
                        id: HandleId::GeoPoint {
                            section_ix: geo.ix,
                            row_ix: row.ix,
                            cell: canvas::GeoCell::X,
                        },
                        pos: frame.to_page((rx, ry)),
                    });
                    if rt == "NURBSTo" {
                        let e_text = row
                            .cell("E")
                            .and_then(|c| c.v.clone().or_else(|| c.f.clone()))
                            .unwrap_or_default();
                        if let Some(f) = dc::curve::parse_nurbs(&e_text) {
                            for (i, (gx, gy, _, _)) in f.points.iter().enumerate() {
                                // Per-axis type gates, matching the drag
                                // commit's rule: an axis with type 0 is
                                // fractional (scale by its dimension),
                                // an absolute axis keeps the raw value —
                                // otherwise the handle jumps on the
                                // first drag (#185 review).
                                out.push(Handle {
                                    id: HandleId::GeoPoint {
                                        section_ix: geo.ix,
                                        row_ix: row.ix,
                                        cell: canvas::GeoCell::NurbsPt(i),
                                    },
                                    pos: frame.to_page((
                                        if f.x_type == 0.0 {
                                            gx * frame.size.0.max(1e-6)
                                        } else {
                                            *gx
                                        },
                                        if f.y_type == 0.0 {
                                            gy * frame.size.1.max(1e-6)
                                        } else {
                                            *gy
                                        },
                                    )),
                                });
                            }
                        }
                    }
                }
            }
            return out;
        }
        let Some(r) = self.sel_page_rect() else {
            return Vec::new();
        };
        let page_h = self.page_geom().h_in;
        // The rect is y-down page inches; handles are y-up.
        let mut out: Vec<Handle> = [
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
        .collect();
        // Geometry vertex handles (#137): one per point row of the
        // shape's OWN Geometry sections (master-inherited geometry has
        // no local rows to drag — its handles arrive with master
        // editing, #122). Appended after the corners, so corner
        // handles win overlaps. #157: RELATIVE rows join (their X/Y
        // are fractions of Width/Height), and curve rows expose their
        // CONTROLS — an ArcTo's sagitta (a circle at the apex) and an
        // EllipticalArcTo's on-curve A/B point (NURBS/spline controls
        // stay unhanded: the renderer draws them as chords, so a
        // control drag would show no feedback).
        if let Some((bx, by, bw, bh)) = self.shape_rect(&ed.doc, page_id, sh) {
            let (w, h) = (bw.max(1e-6), bh.max(1e-6));
            for geo in sh.sheet.sections_named("Geometry") {
                let mut cur: Option<(f64, f64)> = None; // local end of the previous row
                for row in &geo.rows {
                    let t = row.t.as_deref().unwrap_or("");
                    let rel = t.starts_with("Rel");
                    let (Some(x), Some(y)) = (
                        row.cell("X").and_then(|c| c.numeric()),
                        row.cell("Y").and_then(|c| c.numeric()),
                    ) else {
                        continue;
                    };
                    let (lx, ly) = if rel { (x * w, y * h) } else { (x, y) };
                    let point = matches!(
                        t,
                        "MoveTo"
                            | "RelMoveTo"
                            | "LineTo"
                            | "RelLineTo"
                            | "ArcTo"
                            | "RelArcTo"
                            | "EllipticalArcTo"
                            | "RelEllipticalArcTo"
                            | "NURBSTo"
                            | "SplineKnot"
                            | "SplineStart"
                    );
                    if point {
                        for cell in [canvas::GeoCell::X, canvas::GeoCell::Y] {
                            out.push(Handle {
                                id: HandleId::GeoPoint {
                                    section_ix: geo.ix,
                                    row_ix: row.ix,
                                    cell,
                                },
                                pos: (bx + lx, by + ly),
                            });
                        }
                    }
                    // #157 curve controls.
                    match t {
                        "ArcTo" | "RelArcTo" => {
                            if let (Some(a), Some(c0)) =
                                (row.cell("A").and_then(|c| c.numeric()), cur)
                            {
                                let apex = canvas::arc_apex(c0, (lx, ly), a);
                                out.push(Handle {
                                    id: HandleId::GeoPoint {
                                        section_ix: geo.ix,
                                        row_ix: row.ix,
                                        cell: canvas::GeoCell::Apex,
                                    },
                                    pos: (bx + apex.0, by + apex.1),
                                });
                            }
                        }
                        "EllipticalArcTo" => {
                            if let (Some(a), Some(b)) = (
                                row.cell("A").and_then(|c| c.numeric()),
                                row.cell("B").and_then(|c| c.numeric()),
                            ) {
                                out.push(Handle {
                                    id: HandleId::GeoPoint {
                                        section_ix: geo.ix,
                                        row_ix: row.ix,
                                        cell: canvas::GeoCell::OnCurve,
                                    },
                                    pos: (bx + a, by + b),
                                });
                            }
                        }
                        "NURBSTo" => {
                            // #188: the E formula's interior control
                            // points (fractions of Width/Height when
                            // the x/y types are 0) join the row's own
                            // X/Y vertex handles as circles.
                            let e_text = row
                                .cell("E")
                                .and_then(|c| c.v.clone().or_else(|| c.f.clone()))
                                .unwrap_or_default();
                            if let Some(f) = dc::curve::parse_nurbs(&e_text) {
                                // Per-axis type gates, matching the drag
                                // commit: x_type gates X by Width,
                                // y_type gates Y by Height (#185 review).
                                let fx = if f.x_type == 0.0 { w } else { 1.0 };
                                let fy = if f.y_type == 0.0 { h } else { 1.0 };
                                for (i, (gx, gy, _, _)) in f.points.iter().enumerate() {
                                    out.push(Handle {
                                        id: HandleId::GeoPoint {
                                            section_ix: geo.ix,
                                            row_ix: row.ix,
                                            cell: canvas::GeoCell::NurbsPt(i),
                                        },
                                        pos: (bx + gx * fx, by + gy * fy),
                                    });
                                }
                            }
                        }
                        _ => {}
                    }
                    cur = Some((lx, ly));
                }
            }
        }
        out
    }

    /// #202: the selected 1D shape carries a CURVE row — the
    /// selection layer then skips its straight begin→end chord (the
    /// ink IS the line; a straight chord over the curve reads as
    /// "the drag flattened it"). #226: generalised past the #202
    /// CubicBezTo check to every curve row type — the corpus Line-arc
    /// masters shape their whole ink as one EllipticalArcTo, and the
    /// chord-over-arc reads exactly the same way.
    fn sel_1d_curved(&self) -> bool {
        let Some(ed) = self.ed() else {
            return false;
        };
        let Some((page_id, shape_id)) = self.sel else {
            return false;
        };
        let Some(page) = ed.doc.pages.iter().find(|p| p.id == page_id) else {
            return false;
        };
        let Some(sh) = vsdx::model::find_shape_by_id(&page.shapes, shape_id) else {
            return false;
        };
        let Ok(chain) = shape_chain(&ed.doc, page_id, shape_id) else {
            return false;
        };
        // The chord stroke only draws for line handles (1D shapes).
        if ed.doc.resolve_cell(sh, &chain, "BeginX").is_none() {
            return false;
        }
        // All-straight rows (the #176 line-frame set) keep the chord;
        // anything else — EllipticalArcTo, CubicBezTo, splines, NURBS,
        // polylines — is curve ink. The instance's own section when it
        // carries one, else the master root's (what renders).
        let straight = |t: &str| matches!(t, "" | "MoveTo" | "RelMoveTo" | "LineTo" | "RelLineTo");
        let curved = |rows: &[vsdx::model::Row]| {
            if rows
                .iter()
                .any(|r| r.t.as_deref().map(|t| !straight(t)).unwrap_or(false))
            {
                return true;
            }
            // A multi-SEGMENT polyline is not the chord either (#218
            // review): the right-angle route's elbow rows drew the
            // selection stroke diagonally over the bend. Degenerate
            // rows — the straight route's repeated endpoint — stay
            // straight.
            let mut cur: Option<(f64, f64)> = None;
            let mut segments = 0usize;
            for r in rows {
                let t = r.t.as_deref().unwrap_or("");
                let x = r.cell("X").and_then(|c| c.numeric()).unwrap_or(0.0);
                let y = r.cell("Y").and_then(|c| c.numeric()).unwrap_or(0.0);
                match t {
                    "MoveTo" | "RelMoveTo" => cur = Some((x, y)),
                    "LineTo" | "RelLineTo" => {
                        let same = cur
                            .is_some_and(|(cx, cy)| (cx - x).abs() < 1e-9 && (cy - y).abs() < 1e-9);
                        if !same {
                            segments += 1;
                        }
                        cur = Some((x, y));
                    }
                    _ => {}
                }
            }
            segments > 1
        };
        if let Some(g) = sh.sheet.sections_named("Geometry").first() {
            return curved(&g.rows);
        }
        let master_root = sh
            .master
            .and_then(|mid| ed.doc.master(mid))
            .and_then(|m| m.shapes.first());
        master_root
            .and_then(|r| r.sheet.sections_named("Geometry").into_iter().next())
            .is_some_and(|g| curved(&g.rows))
    }

    /// #137/#157: commit a geometry-handle drag — rewrite the row's
    /// X/Y (shape-local, y-up; RELATIVE rows keep their fractions,
    /// resolved ×Width/Height), an ArcTo's sagitta from the pointer,
    /// or an EllipticalArcTo's on-curve A/B — through one `SetRowCell`
    /// Macro. The #59 guard policy: a written cell that resolves to a
    /// formula refuses with a status note and is never written.
    fn apply_geo_vertex(
        &mut self,
        section_ix: i64,
        row_ix: i64,
        cell: canvas::GeoCell,
        to_page: (f64, f64),
    ) {
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
        // Page → shape-local: the inverse of the handle mapping (the
        // chain-mapped bbox's origin; unrotated v1).
        let Some((bx, by, _, _)) = self.shape_rect(&ed.doc, page_id, sh) else {
            return;
        };
        let Some(row) = sh
            .sheet
            .sections_named("Geometry")
            .iter()
            .find(|s| s.ix == section_ix)
            .and_then(|s| s.rows.iter().find(|r| r.ix == row_ix))
        else {
            return;
        };
        let t = row.t.as_deref().unwrap_or("");
        // #157: RELATIVE rows resolve their X/Y as fractions of the
        // shape's Width/Height (the hit walk's convention) — the drag
        // writes the fraction, keeping the row relative.
        let rel = t.starts_with("Rel");
        // The cells this drag writes (guards check each).
        let cells: &[&'static str] = match cell {
            canvas::GeoCell::X | canvas::GeoCell::Y => &["X", "Y"],
            canvas::GeoCell::Apex => &["A"],
            canvas::GeoCell::OnCurve => &["A", "B"],
            // #202: a CubicBezTo row's two Bézier controls.
            canvas::GeoCell::Ctrl => &["A", "B"],
            canvas::GeoCell::Ctrl2 => &["C", "D"],
            // #188: the NURBSTo row's E formula (all controls live in
            // it); the drag re-emits it with the moved group point.
            canvas::GeoCell::NurbsPt(_) => &["E"],
        };
        // #202/#188: curve-control drags CUSTOMIZE the route — the
        // controls are route formulas by default (`Width*0.5`…, the
        // NURBS() E text), and keeping the formula would make the
        // renderer ignore the dragged value (it prefers evaluable
        // formulas). So these cells are exempt from the #157 formula
        // refusal: the drag replaces the formula with the dragged
        // value (a status note says so).
        let customizes = matches!(
            cell,
            canvas::GeoCell::Ctrl | canvas::GeoCell::Ctrl2 | canvas::GeoCell::NurbsPt(_)
        );
        if !customizes {
            for name in cells {
                if dc_core::commands::effective_formula(
                    &ed.doc,
                    sh,
                    &chain,
                    &format!("Geometry{section_ix}.{name}{row_ix}"),
                )
                .is_some()
                {
                    self.status = crate::i18n::strs().msg_vertex_drag_refused(format!(
                        "Geometry{section_ix}.{name}{row_ix}"
                    ));
                    return;
                }
                // Row-cell formulas live on the row cell itself too.
                if row
                    .cell(name)
                    .and_then(|c| c.f.as_deref())
                    .is_some_and(|f| f != "Inh" && f != "No Formula")
                {
                    self.status = crate::i18n::strs().msg_vertex_drag_refused(format!(
                        "row {row_ix}'s {name}"
                    ));
                    return;
                }
            }
        }
        let scope = Scope::Shape {
            page: page_id,
            shape: shape_id,
        };
        let cellv = |v: f64| vsdx::Cell::value(format!("{v:.6}"));
        let (px, py) = (to_page.0 - bx, to_page.1 - by); // shape-local
                                                         // #203: 1D shapes (connectors) map drags through the HitFrame
                                                         // local frame — the bbox the 2D mapping uses starts at a
                                                         // different origin than the geometry rows for a rotated/
                                                         // signed-frame connector.
        let one_d = ed.doc.resolve_cell(sh, &chain, "BeginX").is_some();
        let hit = if one_d {
            let msize = sh
                .master
                .and_then(|mid| ed.doc.master(mid))
                .and_then(|m| m.shapes.first())
                .map(|r| {
                    let g = |n: &str| {
                        ed.doc
                            .resolve_cell(r, &[r], n)
                            .and_then(|c| c.numeric())
                            .unwrap_or(1.0)
                    };
                    (g("Width"), g("Height"))
                })
                .unwrap_or((1.0, 1.0));
            Some(crate::connectors::hit_frame(&ed.doc, &chain, sh, msize))
        } else {
            None
        };
        let mut cmds: Vec<Box<dyn dc_core::Command>> = Vec::new();
        match cell {
            canvas::GeoCell::X | canvas::GeoCell::Y => {
                let (lx, ly) = if let Some(hf) = &hit {
                    // #203: 1D geometry rows are LOCAL-absolute in the
                    // connector frame.
                    hf.to_local(to_page)
                } else if rel {
                    let Some((_, _, bw, bh)) = self.shape_rect(&ed.doc, page_id, sh) else {
                        return;
                    };
                    (px / bw.max(1e-6), py / bh.max(1e-6))
                } else {
                    (px, py)
                };
                for (name, v) in [("X", lx), ("Y", ly)] {
                    cmds.push(Box::new(dc_core::SetRowCell::new(
                        scope.clone(),
                        "Geometry",
                        section_ix,
                        dc_core::commands::RowSel::Ix(row_ix),
                        name,
                        cellv(v),
                    )));
                }
            }
            canvas::GeoCell::Apex => {
                // The row's chord: its own end and the previous row's.
                let (ex, ey) = (
                    row.cell("X").and_then(|c| c.numeric()).unwrap_or(0.0),
                    row.cell("Y").and_then(|c| c.numeric()).unwrap_or(0.0),
                );
                let (ex, ey) = if rel {
                    let Some((_, _, bw, bh)) = self.shape_rect(&ed.doc, page_id, sh) else {
                        return;
                    };
                    (ex * bw, ey * bh)
                } else {
                    (ex, ey)
                };
                // The chord's start: the PREVIOUS row's local end
                // (Rel rows resolve ×Width/Height).
                let size = self
                    .shape_rect(&ed.doc, page_id, sh)
                    .map(|r| (r.2.max(1e-6), r.3.max(1e-6)))
                    .unwrap_or((1.0, 1.0));
                let prev: (f64, f64) = {
                    let secs = sh.sheet.sections_named("Geometry");
                    let sec = secs.iter().find(|s| s.ix == section_ix);
                    let mut p = (0.0, 0.0);
                    if let Some(sec) = sec {
                        for r in &sec.rows {
                            if r.ix == row_ix {
                                break;
                            }
                            let t = r.t.as_deref().unwrap_or("");
                            let (x, y) = (
                                r.cell("X").and_then(|c| c.numeric()).unwrap_or(0.0),
                                r.cell("Y").and_then(|c| c.numeric()).unwrap_or(0.0),
                            );
                            if t.starts_with("Rel") {
                                p = (x * size.0, y * size.1);
                            } else {
                                p = (x, y);
                            }
                        }
                    }
                    p
                };
                let a = canvas::arc_sagitta_from_pointer(prev, (ex, ey), (px, py));
                cmds.push(Box::new(dc_core::SetRowCell::new(
                    scope,
                    "Geometry",
                    section_ix,
                    dc_core::commands::RowSel::Ix(row_ix),
                    "A",
                    cellv(a),
                )));
            }
            canvas::GeoCell::OnCurve => {
                for (name, v) in [("A", px), ("B", py)] {
                    cmds.push(Box::new(dc_core::SetRowCell::new(
                        scope.clone(),
                        "Geometry",
                        section_ix,
                        dc_core::commands::RowSel::Ix(row_ix),
                        name,
                        cellv(v),
                    )));
                }
            }
            canvas::GeoCell::Ctrl | canvas::GeoCell::Ctrl2 => {
                // #202: map the pointer through the connector's
                // HitFrame (Pin/LocPin frame, rotation included) — NOT
                // the bbox origin the X/Y drags use; a connector's
                // row frame starts at its begin corner.
                let msize = sh
                    .master
                    .and_then(|mid| ed.doc.master(mid))
                    .and_then(|m| m.shapes.first())
                    .map(|r| {
                        let g = |n: &str| {
                            ed.doc
                                .resolve_cell(r, &[r], n)
                                .and_then(|c| c.numeric())
                                .unwrap_or(1.0)
                        };
                        (g("Width"), g("Height"))
                    })
                    .unwrap_or((1.0, 1.0));
                let frame = crate::connectors::hit_frame(&ed.doc, &chain, sh, msize);
                let (lx, ly) = frame.to_local(to_page);
                for (name, v) in [(cells[0], lx), (cells[1], ly)] {
                    // A literal write (f: None) REPLACES the cell —
                    // the route formula yields to the dragged value.
                    cmds.push(Box::new(dc_core::SetRowCell::new(
                        scope.clone(),
                        "Geometry",
                        section_ix,
                        dc_core::commands::RowSel::Ix(row_ix),
                        name,
                        cellv(v),
                    )));
                }
            }
            canvas::GeoCell::NurbsPt(idx) => {
                // #188: the E formula IS the data — parse it, move the
                // group's point to the drag (fractions of the bbox
                // extents when xType/yType are 0), re-emit, write the
                // cell with the formula text kept.
                let e_text = row
                    .cell("E")
                    .and_then(|c| c.v.clone().or_else(|| c.f.clone()))
                    .unwrap_or_default();
                let Some(mut f) = dc::curve::parse_nurbs(&e_text) else {
                    return;
                };
                if idx >= f.points.len() {
                    return;
                }
                // #203: 1D shapes map the drop through the HitFrame —
                // 2D shapes keep the bbox mapping. Per-axis type gates
                // (the render arm's rule): only an axis whose type is
                // 0 is fractional and divides by its dimension; an
                // absolute axis keeps the local drop value.
                let (gx, gy, drop) = if let Some(hf) = &hit {
                    let local = hf.to_local(to_page);
                    (hf.size.0.max(1e-6), hf.size.1.max(1e-6), local)
                } else {
                    let Some((_, _, bw, bh)) = self.shape_rect(&ed.doc, page_id, sh) else {
                        return;
                    };
                    (bw.max(1e-6), bh.max(1e-6), (px, py))
                };
                f.points[idx].0 = if f.x_type == 0.0 { drop.0 / gx } else { drop.0 };
                f.points[idx].1 = if f.y_type == 0.0 { drop.1 / gy } else { drop.1 };
                let text = dc::curve::render_nurbs(&f);
                cmds.push(Box::new(dc_core::SetRowCell::new(
                    scope,
                    "Geometry",
                    section_ix,
                    dc_core::commands::RowSel::Ix(row_ix),
                    "E",
                    vsdx::Cell {
                        v: Some(text.clone()),
                        f: Some(text),
                        ..vsdx::Cell::default()
                    },
                )));
            }
        }
        self.apply(Macro::new(
            crate::i18n::strs().msg_undo_move_vertex(row_ix),
            cmds,
        ));
        if customizes {
            if let canvas::GeoCell::NurbsPt(_) = cell {
                self.status =
                    crate::i18n::strs().msg_nurbs_control_customized_the_e_f();
            } else {
                self.status = crate::i18n::strs().msg_curve_control_customized_row_row(row_ix);
            }
        }
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
        // #184 keytips own the keyboard FIRST: while Alt-armed (or on
        // an Alt+letter combo) the letter never reaches a widget.
        self.keytip_input(ctx);
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
            // #237: zoom to selection (Visio's Zoom ▸ Selected
            // affordance). Plain Shift+Z — Ctrl+Shift+Z is redo.
            if !kb_owned && i.modifiers.shift && !ctrl && i.key_pressed(egui::Key::Z) {
                self.zoom_selection_pending = true;
            }
            if i.key_pressed(egui::Key::Delete) && !kb_owned {
                // #108: the connection-point selection only owns
                // Delete while its tool is active — a stale selection
                // must never shadow shape deletion (CodeRabbit).
                if self.tool == Tool::ConnectionPoint && self.connpt_sel.is_some() {
                    self.delete_selected_conn_point();
                } else if self.sel.is_some() {
                    self.delete_selection();
                }
            }
            // Esc closes the frontmost open dialog first (#110
            // tab-order audit: Visio dismisses a dialog on Esc);
            // only with none open do the tool/painter shortcuts run.
            if i.key_pressed(egui::Key::Escape) && !kb_owned {
                let consumed = self.close_front_dialog();
                if !consumed {
                    if matches!(
                        self.tool,
                        Tool::Connector | Tool::Lasso | Tool::ConnectionPoint | Tool::Freeform
                    ) {
                        self.tool = Tool::Pointer;
                    }
                    self.connpt_sel = None;
                    if self.painter_armed.is_some() {
                        self.painter_armed = None;
                        self.painter_sticky = false;
                        self.status = crate::i18n::strs().msg_format_painter_cancelled();
                    }
                }
            }
            // Ctrl+A selects the page (#110, URS-1160).
            if ctrl && i.key_pressed(egui::Key::A) && !kb_owned {
                self.select_all();
            }
            // Arrow-key nudging (#110, URS-1217 keyboard slice):
            // arrows move the selection, Shift widens the step.
            // #180: with glued connectors selected, the documented
            // Visio operation takes the press instead — any arrow key
            // unglues all ends of every selected connector in one
            // undo step (MS Learn, Glue or unglue connectors; its
            // Ctrl is multi-SELECT, not a modifier on the key).
            if !kb_owned && self.sel.is_some() {
                let arrow = [
                    egui::Key::ArrowLeft,
                    egui::Key::ArrowRight,
                    egui::Key::ArrowUp,
                    egui::Key::ArrowDown,
                ]
                .into_iter()
                .any(|k| i.key_pressed(k));
                if !(arrow && self.unglue_selection_connectors()) {
                    let step = if i.modifiers.shift { 1.0 } else { 0.25 };
                    let nudge = [
                        (egui::Key::ArrowLeft, (-step, 0.0)),
                        (egui::Key::ArrowRight, (step, 0.0)),
                        (egui::Key::ArrowUp, (0.0, step)),
                        (egui::Key::ArrowDown, (0.0, -step)),
                    ];
                    for (key, (dx, dy)) in nudge {
                        if i.key_pressed(key) {
                            self.nudge_selection(dx, dy);
                        }
                    }
                }
            }
            // Shape clipboard / duplicate / text edit (Visio keys). Not
            // while a text field owns the keyboard.
            if !kb_owned && self.sel.is_some() {
                if ctrl && i.key_pressed(egui::Key::X) {
                    self.copy_selection();
                    let (p, s) = self.sel.unwrap();
                    self.delete_one(p, s);
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

    /// #184: the keytip input step — one pass over the raw events
    /// before any widget sees them. A bare Alt arms the strip level
    /// (Office gates this to no dialog being open); while armed — or
    /// on a direct Alt+letter combo — the letter resolves against the
    /// last-painted badge set, Esc backs out one level, and a pointer
    /// click disarms. Consumed key events are removed from the input
    /// so no text field ever types them.
    fn keytip_input(&mut self, ctx: &Context) {
        // Office shows no ribbon keytips on the File screen.
        if self.backstage || self.language_dlg_open {
            self.keytips.disarm();
            return;
        }
        let armed = self.keytips.armed();
        // Scan (read-only) first; act after the borrow ends. The tap
        // simulation walks the events in order: a clean Alt press
        // opens the window, any other press closes it, and the
        // release only arms when it stayed open (Office's Alt TAP —
        // Alt+Tab and other Alt combos never arm, #218 review round).
        let mut alt_tap = false;
        let mut back = false;
        let mut letter: Option<char> = None;
        let mut click = false;
        let mut clean = self.keytips.alt_down_clean;
        // The combo path (Alt held) must not steal from a text field:
        // on macOS egui reports Option as Alt, and Option+letter is
        // how accents/layout characters are typed (#218 review round).
        let kb_owned = ctx.egui_wants_keyboard_input();
        ctx.input(|i| {
            click = i.pointer.any_click();
            for e in &i.events {
                match e {
                    egui::Event::Key {
                        key: egui::Key::AltLeft | egui::Key::AltRight,
                        pressed: true,
                        modifiers: m,
                        ..
                    } if !m.ctrl && !m.shift && !m.command => {
                        clean = true;
                    }
                    egui::Event::Key {
                        key: egui::Key::Escape,
                        pressed: true,
                        ..
                    } => {
                        clean = false;
                        back = true;
                    }
                    egui::Event::Key {
                        key: k,
                        pressed: true,
                        modifiers: m,
                        ..
                    } => {
                        clean = false;
                        // One ASCII alphanumeric at a time — keytips
                        // are single characters by construction
                        // (`Key::name` is "A".."Z" / "0".."9"). First
                        // letter wins; the scan keeps walking so the
                        // tap state stays truthful.
                        if !m.ctrl
                            && !m.command
                            && (armed || (m.alt && !kb_owned))
                            && is_keytip_key(*k)
                            && letter.is_none()
                        {
                            letter = k.name().chars().next();
                        }
                    }
                    egui::Event::Key {
                        key: egui::Key::AltLeft | egui::Key::AltRight,
                        pressed: false,
                        ..
                    } => {
                        if clean {
                            alt_tap = true;
                        }
                        clean = false;
                    }
                    _ => {}
                }
            }
        });
        self.keytips.alt_down_clean = clean;
        if armed && click {
            self.keytips.disarm();
        }
        // The tap gate also refuses to arm while a text field owns
        // the keyboard (consistent with the app's other shortcuts).
        if alt_tap
            && !self.shell_surface_open()
            && !ctx.egui_wants_keyboard_input()
        {
            // A bare Alt (re)starts at the strip level, Office-style.
            self.keytips.mode = Some(crate::keytips::Mode::Strip);
        } else if back && armed {
            // Esc backs out one level; the key never reaches the
            // dialog closer below while keytips own it.
            self.keytips.mode = match self.keytips.mode {
                Some(crate::keytips::Mode::Controls) => Some(crate::keytips::Mode::Strip),
                _ => None,
            };
            ctx.input_mut(|i| {
                i.events.retain(|e| {
                    !matches!(
                        e,
                        egui::Event::Key {
                            key: egui::Key::Escape,
                            pressed: true,
                            ..
                        }
                    )
                });
            });
        } else if let Some(c) = letter {
            // An Alt+letter combo arms at the strip level on the fly
            // (Office's accelerator form); armed mode resolves at the
            // current level. Resolution itself waits for paint time,
            // when this frame's badges are collected and assigned.
            if !armed {
                self.keytips.mode = Some(crate::keytips::Mode::Strip);
            }
            let c = c.to_ascii_uppercase();
            self.keytips.pending = Some(c);
            // Swallow the key AND its text twin so nothing types it
            // (only letter/digit keys match — never Escape & co.,
            // whose names also start with a letter).
            ctx.input_mut(|i| {
                i.events.retain(|e| {
                    let eats = match e {
                        egui::Event::Key {
                            key: k,
                            pressed: true,
                            ..
                        } if is_keytip_key(*k) => k.name().starts_with(c),
                        egui::Event::Text(t) => {
                            t.chars()
                                .next()
                                .is_some_and(|t| t.to_ascii_uppercase() == c)
                        }
                        _ => false,
                    };
                    !eats
                });
            });
        }
    }

    /// Resolve one armed letter against the current badge set (the
    /// last-painted frame's assignments — the shell is layout-stable,
    /// so the rects and letters are fresh).
    fn keytip_resolve(&mut self, c: char) {
        let key = c.to_string();
        // Letters are LEVEL-SCOPED (Office: the tab letters vanish at
        // control level): controls match first there, with the QAT
        // digits kept live at every level; the strip level matches
        // only its own badges. (Control letters may repeat a strip
        // letter — "Rulers" R vs the Review tab R — resolved by
        // scope, never by vec order.)
        let is_control = |b: &crate::keytips::Badge| {
            matches!(&b.hit, crate::keytips::Hit::Action { label, .. } if !label.is_empty())
        };
        let is_qat = |b: &crate::keytips::Badge| {
            matches!(&b.hit, crate::keytips::Hit::Action { label, .. } if label.is_empty())
        };
        let find = |pred: &dyn Fn(&crate::keytips::Badge) -> bool| {
            self.keytips
                .badges
                .iter()
                .find(|b| b.key == key && pred(b))
                .map(|b| b.hit.clone())
        };
        let hit = match self.keytips.mode {
            Some(crate::keytips::Mode::Controls) => {
                find(&is_control).or_else(|| find(&is_qat))
            }
            _ => find(&|b| !is_control(b)),
        };
        match hit {
            Some(crate::keytips::Hit::Tab(t)) => {
                self.tab = t;
                self.keytips.mode = Some(crate::keytips::Mode::Controls);
            }
            Some(crate::keytips::Hit::File) => {
                self.open_file_backstage();
                self.keytips.disarm();
            }
            Some(crate::keytips::Hit::TellMe) => {
                self.status = crate::i18n::strs().msg_tell_me_search_arrives_with_a_la();
                self.keytips.disarm();
            }
            Some(crate::keytips::Hit::Action {
                action,
                enabled,
                label: _,
            }) => {
                if enabled {
                    self.dispatch(action);
                    self.keytips.disarm();
                } else {
                    self.status = crate::i18n::strs().msg_keytip_unavailable(key.clone());
                }
            }
            None => {
                // Office keeps the mode on an unknown letter (it
                // beeps); our equivalent is a status note.
                self.status = crate::i18n::strs().msg_no_keytip_key(key);
            }
        }
    }

    /// Whether any shell dialog/window surface is open — keytips do
    /// not arm over a dialog (Office's modal behavior). The read-only
    /// mirror of `close_front_dialog`'s list plus the surfaces that
    /// manage their own Esc (Hyperlinks, Shape Data, Comments, About).
    fn shell_surface_open(&self) -> bool {
        self.selby_open
            || self.snap_dlg_open
            || self.pagesetup_open
            || self.copypic_open
            || self.contgal_open
            || self.backgrounds_open
            || self.reorder_open
            || self.color_pick.is_some()
            || !self.shapesheets.is_empty()
            || self.hyperlink_open.is_some()
            || self.shape_data_open.is_some()
            || self.comment_target.is_some()
            || self.about_open
            || self.winmenu_open
            || self.tab_menu.is_some()
            || self.tab_rename.is_some()
            || self.confirm_new
            || self.confirm_close.is_some()
            || self.text_edit.is_some()
    }

    /// The File tab's open action (#184: factored from the strip's
    /// File click so the keytip path shares it exactly).
    fn open_file_backstage(&mut self) {
        self.backstage = true;
        // #55 landing rule (2010): Info with a document open, Recent
        // (welcome state) without one.
        self.backstage_page = if self.ed.is_some() {
            BackPage::Info
        } else {
            BackPage::Recent
        };
        // The Export pane's suggested path follows the document (#35).
        self.export_pane.reset_for(&self.path);
    }

    /// Paint the armed veil + badges after the ribbon panels close
    /// (#184). Letters are (re)assigned every frame so an Alt+letter
    /// combo always resolves against fresh assignments; the veil and
    /// chips draw only while armed.
    fn paint_keytips(&mut self, ctx: &Context) {
        crate::keytips::assign_badge_letters(&mut self.keytips.badges);
        if let Some(c) = self.keytips.pending.take() {
            self.keytip_resolve(c);
        }
        crate::keytips::paint(ctx, &self.keytips);
    }

    fn delete_selection(&mut self) {
        let Some((page_id, _)) = self.sel else { return };
        if self.sel_set.len() > 1 {
            // One undo step for a multi-delete.
            let ids: Vec<i64> = self
                .sel_set
                .iter()
                .filter(|(p, _)| *p == page_id)
                .map(|(_, s)| *s)
                .collect();
            let mut cmds: Vec<Box<dyn dc_core::Command>> = ids
                .iter()
                .map(|&s| Box::new(RemoveShape::new(page_id, s)) as Box<dyn dc_core::Command>)
                .collect();
            // #138: deleting a container strips its members' membership
            // rows in the same Macro — no dangling rows in the file,
            // one undo restores everything.
            if let Some(ed) = self.ed() {
                cmds.extend(dc_core::container_membership_cleanup(
                    &ed.doc, page_id, &ids,
                ));
                for sid in &ids {
                    cmds.extend(self.guide_glue_release_cmds(page_id, *sid));
                }
            }
            cmds.extend(self.connect_cleanup_cmds(page_id, &ids));
            let n = ids.len();
            self.apply(Macro::new(crate::i18n::strs().msg_undo_delete_n_shapes(n as i64), cmds));
        } else {
            let shape_id = self.sel.unwrap().1;
            self.delete_one(page_id, shape_id);
        }
        self.set_selection(None);
        self.sel_set.clear();
    }

    /// Delete a single shape, folding membership-row cleanup into the
    /// same undo step when it is a container (#138) and glue-release
    /// when it is a guide (#113: dependents keep their position, the
    /// pin formulas referencing the gone guide are stripped).
    fn delete_one(&mut self, page_id: i64, shape_id: i64) {
        let mut cmds: Vec<Box<dyn dc_core::Command>> =
            vec![Box::new(RemoveShape::new(page_id, shape_id))];
        cmds.extend(self.guide_glue_release_cmds(page_id, shape_id));
        cmds.extend(self.connect_cleanup_cmds(page_id, &[shape_id]));
        if let Some(ed) = self.ed() {
            cmds.extend(dc_core::container_membership_cleanup(
                &ed.doc,
                page_id,
                &[shape_id],
            ));
        }
        self.apply(Macro::new(
            crate::i18n::strs().msg_undo_delete_shape(shape_id),
            cmds,
        ));
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

    /// #110 (Visio's Lasso Select): the freehand trail (page inches,
    /// y-up) selects every shape it touches or encloses — the marquee's
    /// group-edit semantics, the lasso's touch geometry.
    fn apply_lasso(&mut self, trail: &[(f64, f64)]) {
        let Some(ed) = self.ed() else { return };
        let Some(page_id) = self.page_id() else {
            return;
        };
        let page = &ed.doc.pages[self.page_idx.min(ed.doc.pages.len().saturating_sub(1))];
        let mut picked = Vec::new();
        // Group-edit mode (#78): the lasso selects among the open
        // group's members only, like the marquee.
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
                // Closure tolerance from the view scale: the pointer's
                // return to the start reads as a closed loop within a
                // few SCREEN pixels, whatever the zoom (inches/px).
                let tol = 4.0 / self.view.px_per_in() as f64;
                if canvas::lasso_hits_rect(trail, sx, sy, w, h, tol) {
                    picked.push((page_id, shape.id));
                }
            }
        }
        let n = picked.len();
        self.sel_set = picked;
        self.sel = self.sel_set.last().copied();
        self.status = crate::i18n::strs().msg_lasso_selected_n_shape_s(n);
    }

    /// #110 tab-order audit: Esc dismisses the frontmost open dialog
    /// (Visio behavior) before the tool/painter shortcuts see the key.
    /// Only one dialog is realistically open at a time; with several,
    /// each Esc closes the next in a fixed order. The Hyperlinks
    /// window manages its OWN Esc (a draft discards first — closing
    /// the window on the same key would lose the layering), and the
    /// confirm-close/new modals handle theirs. Returns true when one
    /// closed.
    fn close_front_dialog(&mut self) -> bool {
        if self.selby_open {
            self.selby_open = false;
            return true;
        }
        if self.language_dlg_open {
            self.language_dlg_open = false;
            return true;
        }
        if self.snap_dlg_open {
            self.snap_dlg_open = false;
            return true;
        }
        if self.pagesetup_open {
            self.pagesetup_open = false;
            return true;
        }
        if self.copypic_open {
            self.copypic_open = false;
            return true;
        }
        if self.contgal_open {
            self.contgal_open = false;
            return true;
        }
        if self.backgrounds_open {
            self.backgrounds_open = false;
            return true;
        }
        if self.reorder_open {
            self.reorder_open = false;
            return true;
        }
        if self.color_pick.is_some() {
            self.color_pick = None;
            return true;
        }
        // The ShapeSheet WINDOWS (#190/#192): persistent surfaces like
        // the dialogs — Esc dismisses the most recently opened one
        // after the dialogs.
        if self.shapesheets.pop().is_some() {
            return true;
        }
        false
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
                return Some(crate::i18n::strs().msg_size_cell_is_formula_driven_f(size_cell, f));
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
        // #95: a member's cells are parent-local — convert the
        // page-space drag delta through the renderer's affine (any
        // base point works; the translation cancels in the
        // difference).
        let (dx, dy) = if chain.len() >= 2 {
            let page_h = Self::page_height(ed, page_id);
            let l0 = dc::svg::page_point_to_parent_local(&ed.doc, &chain, page_h, (0.0, 0.0));
            let l1 = dc::svg::page_point_to_parent_local(&ed.doc, &chain, page_h, (dx, dy));
            (l1.0 - l0.0, l1.1 - l0.1)
        } else {
            (dx, dy)
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
            self.status = crate::i18n::strs().msg_resize_refused_shape_is_protecte();
            return;
        }
        // The partial-guard note is written AFTER `apply`, which
        // overwrites the status with the command label.
        let guard_note = match (&w_guard, &h_guard) {
            (Some(g), _) => Some(crate::i18n::strs().msg_width_not_resized_guard(g.clone())),
            (_, Some(g)) => Some(crate::i18n::strs().msg_height_not_resized_guard(g.clone())),
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
        // A group resize scales its members (#69): literal Xform cells
        // (the common case — GroupShapes writes literals when grouping)
        // scale here; formula-driven cells stay with the cache refresh
        // below (#15's per-shape rules).
        if sh.type_.as_deref() == Some("Group") {
            cmds.extend(dc_core::scale_group_members(
                &ed.doc,
                page_id,
                shape_id,
                nw / w,
                nh / h,
            ));
        }
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
        if let Some(r) = self.reroute_cmd(page_id, &[shape_id]) {
            cmds.push(r);
        }
        self.apply(Macro::new(
            crate::i18n::strs().msg_undo_resize_shape(shape_id),
            cmds,
        ));
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
                    return Some(crate::i18n::strs().msg_cell_is_formula_driven(name, f));
                }
            }
        }
        for lock in locks {
            if protection_flag(&ed.doc, sh, &chain, lock) {
                return Some(crate::i18n::strs().msg_lock_is_set_protection_section(lock));
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
            self.status = crate::i18n::strs().msg_endpoint_drag_refused_reason(reason);
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
        // #95: endpoint cells are parent-local for a member — the
        // page-space target converts through the affine; the other
        // endpoint already reads parent-local.
        let to = if chain.len() >= 2 {
            let page_h = Self::page_height(ed, page_id);
            dc::svg::page_point_to_parent_local(&ed.doc, &chain, page_h, to)
        } else {
            to
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
            // effective_formula, not the instance cell's own F — an
            // 'Inh' PinX over a master's GUARD((BeginX+EndX)/2) is
            // still formula-driven to the command layer, and a refused
            // write aborts the whole macro (#172).
            let formula = dc_core::commands::effective_formula(&ed.doc, sh, &chain, name);
            if formula.is_some() {
                kept_note = Some(crate::i18n::strs().msg_endpoint_moved_but_name_is_formu(name));
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
            crate::i18n::strs().msg_move_line_endpoint_shape_shape_i(shape_id),
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
            self.status = crate::i18n::strs().msg_line_move_refused_reason(reason);
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
        // #95: member cells are parent-local — convert the page-space
        // drag through the affine (translation cancels).
        let (dx, dy) = if chain.len() >= 2 {
            let page_h = Self::page_height(ed, page_id);
            let f0 = dc::svg::page_point_to_parent_local(&ed.doc, &chain, page_h, (0.0, 0.0));
            let f1 = dc::svg::page_point_to_parent_local(
                &ed.doc,
                &chain,
                page_h,
                (to.0 - from.0, to.1 - from.1),
            );
            (f1.0 - f0.0, f1.1 - f0.1)
        } else {
            (to.0 - from.0, to.1 - from.1)
        };
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
            // effective_formula (#172): an 'Inh' instance cell over a
            // master's GUARD would otherwise refuse and abort the move.
            let formula = dc_core::commands::effective_formula(&ed.doc, sh, &chain, name);
            if formula.is_some() {
                kept_note = Some(crate::i18n::strs().msg_line_moved_but_name_is_formula_d(name));
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
        self.apply(Macro::new(
            crate::i18n::strs().msg_undo_move_line_shape(shape_id),
            cmds,
        ));
        if let Some(note) = kept_note {
            self.status = note;
        }
    }

    /// #172: translate a CONNECTOR by dragging its body. Free endpoints
    /// move as plain values; glued endpoints carry their glue formula
    /// (`PAR(PNT(...))`/`_WALKGLUE`) and move through [`SetGluedCell`]
    /// — the GUARD-waived write that keeps the formula — exactly the
    /// reroute engine's own cache-update pattern, so the glue survives
    /// and the reroute tail re-attaches the moved ends (a both-ends
    /// glued connector pins back to its targets, like Visio). Derived
    /// XForm cells a formula owns are kept, never literal-written (an
    /// aborted macro was the #172 "drag does nothing").
    fn apply_connector_body_move(&mut self, from: (f64, f64), to: (f64, f64)) {
        let Some((page_id, shape_id)) = self.sel else {
            return;
        };
        // Locks refuse; formulas do not (they ride SetGluedCell).
        if let Some(reason) = self.line_lock_guard(&["LockBegin", "LockEnd"]) {
            self.status = crate::i18n::strs().msg_connector_move_refused_reason(reason);
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
        // A shape without a pin is not a movable 1D frame.
        if get("PinX").zip(get("PinY")).is_none() {
            return;
        }
        let (dx, dy) = (to.0 - from.0, to.1 - from.1);
        // #202 owner rule: connector glue RELEASES once the drag
        // carries an end past the gluepoint snap radius — within it,
        // the reroute pins the glued end back to its anchor (Visio's
        // snap-back); past it the end detaches and moves freely.
        let dist = dx.hypot(dy);
        let radius = SNAP_STRENGTH_PX as f64 / self.view.px_per_in() as f64;
        let scope = Scope::Shape {
            page: page_id,
            shape: shape_id,
        };
        let cell = |v: f64| vsdx::Cell::value(format!("{v:.6}"));
        let mut cmds: Vec<Box<dyn dc_core::Command>> = Vec::new();
        // Endpoints: a formula-owned end (glue: PAR(PNT(...)) /
        // _WALKGLUE) is NOT written — the formula owns its value too,
        // and a moved cache would desync render from evaluation. It
        // stays pinned to its target (Visio's feel) UNLESS the drag
        // carries it past the snap radius, when it releases (the
        // Connect record drops and the endpoint cells become plain
        // values — glue formulas replaced, so nothing can snap the
        // end back to a target that no longer owns it). A free end
        // moves as a plain value through the GUARD-waived write.
        let mut free: [bool; 2] = [true, true];
        let mut releases: Vec<usize> = Vec::new(); // Connect indexes
        let mut release_ends = 0usize;
        for (i, (name, v)) in [
            ("BeginX", bx + dx),
            ("BeginY", by + dy),
            ("EndX", ex + dx),
            ("EndY", ey + dy),
        ]
        .into_iter()
        .enumerate()
        {
            if dc_core::commands::effective_formula(&ed.doc, sh, &chain, name).is_some() {
                free[i / 2] = false;
                continue;
            }
            cmds.push(Box::new(dc_core::SetGluedCell::new(
                scope.clone(),
                name,
                cell(v),
            )));
        }
        if dist > radius {
            let page = ed.doc.pages.iter().find(|p| p.id == page_id);
            let list = page.map(|p| p.connects()).unwrap_or_default();
            for (end, cells) in [(0usize, ["BeginX", "BeginY"]), (1usize, ["EndX", "EndY"])] {
                if free[end] {
                    continue;
                }
                // Release: a plain value write replaces the cell
                // (dropping the glue formula — the next drag sees a
                // free end) and the Connect record drops with it.
                if let Some(idx) = list
                    .list
                    .iter()
                    .position(|c| c.from_sheet == shape_id && c.from_cell == cells[0])
                {
                    releases.push(idx);
                }
                for (name, v) in [
                    (cells[0], if end == 0 { bx + dx } else { ex + dx }),
                    (cells[1], if end == 0 { by + dy } else { ey + dy }),
                ] {
                    cmds.push(Box::new(dc_core::SetGluedCell::new(
                        scope.clone(),
                        name,
                        cell(v),
                    )));
                }
                release_ends += 1;
            }
            // Highest index first: RemoveConnect plans by index, and
            // each removal shifts the tail.
            releases.sort_unstable_by(|a, b| b.cmp(a));
            for idx in releases {
                cmds.push(Box::new(dc_core::RemoveConnect::new(
                    dc_core::ConnectsOwner::Page(page_id),
                    idx,
                )));
            }
        }
        if !free[0] && !free[1] && release_ends == 0 {
            self.status = crate::i18n::strs().msg_connector_is_glued_at_both_ends();
            return;
        }
        // The frame (Pin/Width/Height/LocPin caches) and the route
        // rows derive from the POST-MOVE endpoints — the reroute
        // planner's own corpus rules (review pass: the hand-rolled
        // writes here materialized missing Geometry rows on
        // master-based connectors and skipped formula-owned Pin
        // caches; the planner refreshes through guards and only
        // rewrites rows the connector already has).
        cmds.push(Box::new(crate::cmd::RerouteConnectors::new_including(
            page_id,
            vec![shape_id],
        )));
        // Connectors glued TO this one re-attach in the same undo
        // step.
        if let Some(r) = self.reroute_cmd(page_id, &[shape_id]) {
            cmds.push(r);
        }
        self.apply(Macro::new(
            crate::i18n::strs().msg_undo_move_connector(shape_id),
            cmds,
        ));
        if release_ends > 0 {
            self.status = crate::i18n::strs().msg_connector_moved_release_ends_glu(release_ends);
        }
    }

    /// The Protection locks of the selected 1D shape, without the
    /// formula half of [`Self::line_drag_guard`] (glue formulas are
    /// expected on connectors and ride [`dc_core::SetGluedCell`]).
    fn line_lock_guard(&self, locks: &[&str]) -> Option<String> {
        let ed = self.ed()?;
        let (page_id, shape_id) = self.sel?;
        let page = ed.doc.pages.iter().find(|p| p.id == page_id)?;
        let sh = vsdx::model::find_shape_by_id(&page.shapes, shape_id)?;
        let chain = shape_chain(&ed.doc, page_id, shape_id).ok()?;
        for lock in locks {
            if protection_flag(&ed.doc, sh, &chain, lock) {
                return Some(crate::i18n::strs().msg_lock_is_set_protection_section(lock));
            }
        }
        None
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

    /// #75: the topmost container under the selection's centroid —
    /// a candidate is a top-level shape that is the target of at
    /// least one member-side `User.msvSDContainerShape` reference or
    /// was created by the Add-to-New flow (its `Container.` name);
    /// the selection's own shapes never qualify. Z-order decides
    /// between overlapping containers (topmost wins).
    fn underlying_container(&self, members: &[i64]) -> Option<i64> {
        let ed = self.ed()?;
        let page = ed.doc.pages.get(self.page_idx)?;
        let page_id = page.id;
        // The selection's centroid (page inches, y-up).
        let mut cx = 0.0;
        let mut cy = 0.0;
        let mut n = 0.0;
        for &sid in members {
            let sh = page.shapes.iter().find(|s| s.id == sid)?;
            let (x, y, w, h) = self.shape_rect(&ed.doc, page_id, sh)?;
            cx += x + w / 2.0;
            cy += y + h / 2.0;
            n += 1.0;
        }
        if n == 0.0 {
            return None;
        }
        let centroid = (cx / n, cy / n);
        let is_container = |s: &Shape, referenced: bool| {
            referenced
                || s.name_u
                    .as_deref()
                    .is_some_and(|n| n.starts_with("Container."))
        };
        // Which ids some member references (container identity by
        // membership convention).
        let referenced: std::collections::HashSet<i64> = page
            .shapes
            .iter()
            .flat_map(|s| s.sheet.sections_named("User"))
            .filter_map(|sec| {
                sec.named_row("msvSDContainerShape")
                    .and_then(|r| r.cell("Value"))
                    .and_then(|c| c.numeric())
                    .map(|v| v as i64)
            })
            .collect();
        let mut found: Option<i64> = None;
        for s in &page.shapes {
            if members.contains(&s.id) {
                continue;
            }
            if !is_container(s, referenced.contains(&s.id)) {
                continue;
            }
            let Some((x, y, w, h)) = self.shape_rect(&ed.doc, page_id, s) else {
                continue;
            };
            if centroid.0 >= x && centroid.0 <= x + w && centroid.1 >= y && centroid.1 <= y + h {
                found = Some(s.id); // later z-order wins (topmost)
            }
        }
        found
    }

    /// #75: Add to Underlying Container — write the membership row on
    /// each selected shape (the #27 convention, materialized on first
    /// edit), one Macro, one undo.
    fn add_selection_to_underlying_container(&mut self) {
        let Some(page_id) = self.page_id() else {
            return;
        };
        let members: Vec<i64> = self
            .sel_set
            .iter()
            .filter(|(p, _)| *p == page_id)
            .map(|(_, s)| *s)
            .collect();
        let Some(container) = self.underlying_container(&members) else {
            return;
        };
        let mut cmds: Vec<Box<dyn dc_core::Command>> = members
            .iter()
            .map(|&m| {
                Box::new(dc_core::SetRowCell::new(
                    Scope::Shape {
                        page: page_id,
                        shape: m,
                    },
                    "User",
                    0,
                    dc_core::commands::RowSel::Named("msvSDContainerShape".into()),
                    "Value",
                    vsdx::Cell::value(container.to_string()),
                )) as Box<dyn dc_core::Command>
            })
            .collect();
        // #138 §2: the heading band's member count re-derives from live
        // membership — "{base title} ({count})" through SetText.
        if let Some(ed) = self.ed() {
            let title = ed
                .doc
                .pages
                .iter()
                .find(|p| p.id == page_id)
                .and_then(|p| p.shapes.iter().find(|s| s.id == container))
                .and_then(|cont| {
                    // #191: master-based containers keep the title row on
                    // the master root — resolve through the chain.
                    ed.doc
                        .effective_user_row(cont, &[], "msvSDContainerTitle")
                        .and_then(|r| r.cell("Value"))
                        .and_then(|c| c.v.clone())
                })
                .unwrap_or_else(|| crate::i18n::strs().msg_container());
            // The UNION — already-members selected again must not
            // double-count (#75 branch review).
            let mut all = self.container_member_ids(page_id, container);
            for m in &members {
                if !all.contains(m) {
                    all.push(*m);
                }
            }
            let count = all.len();
            // Text edit as one undo step incl. text-derived cache
            // refresh (#147).
            cmds.push(Box::new(dc::textfunc::text_edit_macro(
                &ed.doc,
                &Scope::Shape {
                    page: page_id,
                    shape: container,
                },
                &format!("{title} ({count})"),
            )));
        }
        if !cmds.is_empty() {
            self.apply(Macro::new(
                crate::i18n::strs().msg_undo_add_n_to_container(
                    members.len() as i64,
                    container.to_string(),
                ),
                cmds,
            ));
        }
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
            // heading text is shape CONTENT — stays English (#246)
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
        let formula_pinned =
            |n: &str| dc_core::commands::effective_formula(&ed.doc, sh, &chain, n).is_some();
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
        Box::new(Macro::new(
            crate::i18n::strs().msg_undo_move_line_shape(sid),
            cmds,
        ))
    }

    // -- snap & glue (GitHub #109) ----------------------------------------

    /// Reload the snap & glue toggles from the document (open / new /
    /// after any model-level settings change).
    fn reload_snap_settings(&mut self) {
        self.snap = self
            .ed
            .as_ref()
            .and_then(|e| e.doc.doc_snap_glue())
            .map(crate::snap::SnapSettings::from_doc)
            .unwrap_or_default();
    }

    /// Write the current toggles into the document. Settings are
    /// document state, not drawing edits, so they never enter the
    /// undo history (Visio's behavior — Snap & Glue has no undo).
    fn apply_snap_settings(&mut self) {
        if let Some(ed) = self.ed.as_mut() {
            ed.doc.set_doc_snap_glue(self.snap.to_doc());
            self.dirty = true;
        }
    }

    /// The snap-side description of every top-level shape except
    /// `excluded` (the dragged shape, its co-selected group, and any
    /// container members riding along): alignment box, own geometry
    /// vertices, and connection points — the shape's own rows first,
    /// the master's for instances (Visio stencils define them on the
    /// master, like the #137 geometry inheritance note).
    fn snap_targets(&self, page_id: i64, excluded: &[i64]) -> Vec<crate::snap::SnapTarget> {
        let Some(ed) = self.ed() else {
            return Vec::new();
        };
        let Some(page) = ed.doc.pages.iter().find(|p| p.id == page_id) else {
            return Vec::new();
        };
        page.shapes
            .iter()
            .filter(|s| !excluded.contains(&s.id))
            .filter_map(|s| {
                let rect = self.shape_rect(&ed.doc, page_id, s)?;
                let mut t = crate::snap::SnapTarget {
                    id: s.id,
                    rect,
                    ..Default::default()
                };
                // Own geometry vertices (#137's mapping: shape-local
                // y-up onto the chain-mapped bbox origin).
                for geo in s.sheet.sections_named("Geometry") {
                    for row in &geo.rows {
                        let point_row = matches!(
                            row.t.as_deref().unwrap_or(""),
                            "MoveTo"
                                | "LineTo"
                                | "ArcTo"
                                | "EllipticalArcTo"
                                | "NURBSTo"
                                | "SplineKnot"
                        );
                        if !point_row {
                            continue;
                        }
                        if let (Some(x), Some(y)) = (
                            row.cell("X").and_then(|c| c.numeric()),
                            row.cell("Y").and_then(|c| c.numeric()),
                        ) {
                            t.vertices.push((rect.0 + x, rect.1 + y));
                        }
                    }
                }
                collect_connection_points(&mut t, &s.sheet);
                if t.connection_points.is_empty() {
                    if let Some(mid) = s.master {
                        if let Some(root) = ed.doc.master(mid).and_then(|m| m.shapes.first()) {
                            collect_connection_points(&mut t, &root.sheet);
                        }
                    }
                }
                Some(t)
            })
            .collect()
    }

    /// A snap context over prebuilt targets and guides (#113 now
    /// feeds them): the displayed grid and ruler-subdivision
    /// spacings, and the Visio-style screen snap strength converted
    /// through the zoom.
    fn snap_ctx_of<'t>(
        &self,
        targets: &'t [crate::snap::SnapTarget],
        guides: &'t [crate::snap::Guide],
    ) -> crate::snap::SnapCtx<'t> {
        crate::snap::SnapCtx {
            targets,
            guides,
            grid_step_in: canvas::grid_step_in(&self.view),
            ruler_step_in: canvas::ruler_interval_in(&self.view),
            ruler_mirror_x_in: crate::i18n::rtl().then(|| self.page_geom().w_in),
            threshold_in: SNAP_STRENGTH_PX as f64 / self.view.px_per_in() as f64,
        }
    }

    /// Snap the live drag position (#109) and refresh the dynamic-grid
    /// hints. The model rect is still the press-time rect during the
    /// gesture, so the live box is the rect translated by the pin
    /// delta; the returned pin carries the snap translation, and the
    /// commit path (move_cmd_for) lands snapped by construction.
    fn apply_drag_snap(&mut self, d: &DragState, pin: (f64, f64)) -> (f64, f64) {
        self.snap_live.clear();
        let Some(page_id) = self.page_id() else {
            return pin;
        };
        let Some(ed) = self.ed.as_ref() else {
            return pin;
        };
        let Some(page) = ed.doc.pages.iter().find(|p| p.id == page_id) else {
            return pin;
        };
        let Some(sh) = vsdx::model::find_shape_by_id(&page.shapes, d.shape) else {
            return pin;
        };
        let Some((x, y, w, h)) = self.shape_rect(&ed.doc, page_id, sh) else {
            return pin;
        };
        let mut excluded: Vec<i64> = Vec::with_capacity(d.group.len() + 3);
        excluded.push(d.shape);
        excluded.extend(d.group.iter().copied());
        excluded.extend(self.container_member_ids(page_id, d.shape));
        let targets = self.snap_targets(page_id, &excluded);
        let guide_list = self.snap_guides(&excluded);
        let ctx = self.snap_ctx_of(&targets, &guide_list);
        let live = (x + pin.0 - d.start_pin.0, y + pin.1 - d.start_pin.1, w, h);
        let res = crate::snap::snap_translation(live, &ctx, &self.snap);
        if self.snap.dynamic_grid {
            let snapped = (live.0 + res.dx, live.1 + res.dy, w, h);
            self.snap_live = crate::snap::dynamic_hints(snapped, &ctx);
        }
        // #113: remember which guide an axis snapped onto — the move
        // commit turns it into Visio's pin glue formula when
        // glue-to-guides is enabled.
        self.drag_glue = None;
        if self.snap.glue_active && self.snap.glue_guides {
            use crate::snap::SnapHit;
            let guide_of = |h: &Option<(f64, SnapHit)>| match h {
                Some((_, SnapHit::Guide(id))) => Some(*id),
                _ => None,
            };
            let (gx, gy) = (
                guide_of(&res.x_hit).is_some(),
                guide_of(&res.y_hit).is_some(),
            );
            if let Some(id) = guide_of(&res.x_hit).or(guide_of(&res.y_hit)) {
                self.drag_glue = Some((id, gx, gy));
            }
        }
        (pin.0 + res.dx, pin.1 + res.dy)
    }

    /// #108: commands removing every `<Connect>` that references a
    /// deleted shape (either side). Indices are collected descending —
    /// sequential `RemoveConnect`s inside one Macro shift the list.
    fn connect_cleanup_cmds(
        &self,
        page_id: i64,
        deleted: &[i64],
    ) -> Vec<Box<dyn dc_core::Command>> {
        let Some(ed) = self.ed() else {
            return Vec::new();
        };
        let Some(page) = ed.doc.pages.iter().find(|p| p.id == page_id) else {
            return Vec::new();
        };
        let connects = page.connects();
        let mut cmds: Vec<Box<dyn dc_core::Command>> = Vec::new();
        for idx in (0..connects.list.len()).rev() {
            let c = &connects.list[idx];
            if deleted.contains(&c.from_sheet) || deleted.contains(&c.to_sheet) {
                cmds.push(Box::new(dc_core::RemoveConnect::new(
                    dc_core::ConnectsOwner::Page(page_id),
                    idx,
                )));
            }
        }
        cmds
    }

    /// #113: commands that release every shape glued to `shape_id`
    /// (no-ops for non-guides): each dependent's pin formula
    /// referencing the guide is replaced by its cached value, so the
    /// shape keeps its position when the guide goes — Visio's
    /// release-on-delete.
    fn guide_glue_release_cmds(
        &self,
        page_id: i64,
        shape_id: i64,
    ) -> Vec<Box<dyn dc_core::Command>> {
        let Some(ed) = self.ed() else {
            return Vec::new();
        };
        let Some(page) = ed.doc.pages.iter().find(|p| p.id == page_id) else {
            return Vec::new();
        };
        let Some(sh) = vsdx::model::find_shape_by_id(&page.shapes, shape_id) else {
            return Vec::new();
        };
        if !crate::guides::is_guide(sh) {
            return Vec::new();
        }
        let mut cmds: Vec<Box<dyn dc_core::Command>> = Vec::new();
        for (sid, axes) in crate::guides::glue_dependents(&page.shapes, shape_id) {
            let Some(dep) = vsdx::model::find_shape_by_id(&page.shapes, sid) else {
                continue;
            };
            for (cell, _) in axes {
                let v = dep.sheet.cell(cell).and_then(|c| c.numeric());
                if let Some(v) = v {
                    cmds.push(Box::new(SetCell::new(
                        Scope::Shape {
                            page: page_id,
                            shape: sid,
                        },
                        cell,
                        vsdx::Cell::value(format!("{v:.6}")),
                    )));
                }
            }
        }
        cmds
    }

    // -- connector authoring (GitHub #108) -------------------------------

    /// #108: a connector endpoint drag's glue lifecycle. Build the
    /// endpoint cells (the #59 derived-cell logic via
    /// `apply_line_endpoint` semantics), then resolve glue at the drop:
    /// static (connection point) or dynamic (shape) write the corpus
    /// formula and Connect; empty page releases. One undo step.
    fn commit_connector_endpoint(&mut self, end_id: HandleId, raw: (f64, f64)) {
        use crate::connectors::{End, GlueKind};
        let Some((page_id, sid)) = self.sel else {
            return;
        };
        let end = match end_id {
            HandleId::LineBegin => End::Begin,
            HandleId::LineEnd => End::End,
            _ => return,
        };
        // Where the OTHER endpoint sits (page space) — the facing
        // anchor for dynamic attachment.
        let other = self.ed.as_ref().and_then(|ed| {
            let page = ed.doc.pages.iter().find(|p| p.id == page_id)?;
            let sh = vsdx::model::find_shape_by_id(&page.shapes, sid)?;
            let chain = shape_chain(&ed.doc, page_id, sid).ok()?;
            let (b, e) = match end {
                End::Begin => ("EndX", "EndY"),
                End::End => ("BeginX", "BeginY"),
            };
            let g = |n: &str| ed.doc.resolve_cell(sh, &chain, n).and_then(|c| c.numeric());
            Some((g(b)?, g(e)?))
        });
        let Some(other) = other else { return };
        let cands = self.glue_candidates(&[sid]);
        let threshold = SNAP_STRENGTH_PX as f64 / self.view.px_per_in() as f64;
        // #202: the endpoint drop glues STRICTLY — a connection point
        // within the snap radius, or a shape under the cursor (no
        // halo: the ring around every shape re-attached endpoints the
        // cursor never touched, pinning them to the shape's edges).
        let target = crate::connectors::glue_decision_ex(
            &cands,
            raw,
            other,
            threshold,
            self.snap.glue_active,
            self.snap.glue_connection_points,
            false,
        );
        let attach = target.map(|t| t.attach).unwrap_or(raw);
        // The plain endpoint move first (derived cells + guards).
        let mut cmds: Vec<Box<dyn dc_core::Command>> =
            vec![self.move_endpoint_cmd(page_id, sid, end, attach)];
        // Resolve the glue state change against the existing Connect.
        let page = self
            .ed
            .as_ref()
            .and_then(|ed| ed.doc.pages.iter().find(|p| p.id == page_id));
        let existing = page.and_then(|p| {
            p.connects()
                .list
                .iter()
                .position(|c| c.from_sheet == sid && c.from_cell == end.cell())
        });
        let same_glue = |t: &crate::connectors::GlueTarget, idx: usize| -> bool {
            let Some(p) = page else { return false };
            let list = p.connects();
            let Some(c) = list.list.get(idx) else {
                return false;
            };
            c.to_sheet == t.shape_id
                && c.to_part
                    == match t.kind {
                        GlueKind::Static { row_ix } => 100 + row_ix,
                        GlueKind::Dynamic => crate::connectors::PART_WALKGLUE,
                    }
        };
        if let Some(idx) = existing {
            cmds.push(Box::new(dc_core::RemoveConnect::new(
                dc_core::ConnectsOwner::Page(page_id),
                idx,
            )));
        }
        if let Some(t) = target {
            // Formula cells overwrite the plain values from the move.
            let (f, trig) = match t.kind {
                GlueKind::Static { row_ix } => (
                    crate::connectors::static_glue_formula(t.shape_id, row_ix),
                    None,
                ),
                GlueKind::Dynamic => (crate::connectors::walkglue_formula(end), Some(t.shape_id)),
            };
            for cell in [end.cell(), end.y_cell()] {
                cmds.push(Box::new(dc_core::SetGluedCell::new(
                    Scope::Shape {
                        page: page_id,
                        shape: sid,
                    },
                    cell,
                    vsdx::Cell {
                        v: Some(format!(
                            "{:.6}",
                            if cell == end.cell() {
                                attach.0
                            } else {
                                attach.1
                            }
                        )),
                        f: Some(f.clone()),
                        ..vsdx::Cell::default()
                    },
                )));
            }
            if let Some(tid) = trig {
                cmds.push(Box::new(SetCell::new(
                    Scope::Shape {
                        page: page_id,
                        shape: sid,
                    },
                    end.trigger_cell(),
                    vsdx::Cell {
                        v: Some("2".into()),
                        f: Some(format!("GUARD(_XFTRIGGER(Sheet.{tid}!EventXFMod))")),
                        ..vsdx::Cell::default()
                    },
                )));
            }
            cmds.push(Box::new(dc_core::AddConnect {
                owner: dc_core::ConnectsOwner::Page(page_id),
                connect: crate::connectors::glue_connect(sid, end, &t),
            }));
            let same = existing.is_some_and(|idx| same_glue(&t, idx));
            self.status = if same {
                crate::i18n::strs().msg_connector_stays_glued(t.shape_id)
            } else {
                match t.kind {
                    GlueKind::Static { .. } => {
                        crate::i18n::strs().msg_connector_glued_shape_point(t.shape_id)
                    }
                    GlueKind::Dynamic => crate::i18n::strs().msg_connector_glued_shape(t.shape_id),
                }
            };
        } else if existing.is_some() {
            self.status = crate::i18n::strs().msg_connector_end_released();
        }
        // #179: the drop moved an endpoint — a master-based
        // connector's formula-driven frame caches and geometry rows
        // must refresh in the SAME undo step or the render keeps the
        // pre-drag pose (the glue writes alone leave Pin/Width/Height
        // stale under their GUARDs). Planned by ID: works for the
        // re-glue and the release alike. The PRE-drag endpoints ride
        // along (#218 review): the endpoint writes above land first,
        // so re-reading the pose here would name the NEW ends and the
        // literal-row translation delta would collapse to zero.
        let pre_pose = self.ed.as_ref().and_then(|ed| {
            let page = ed.doc.pages.iter().find(|p| p.id == page_id)?;
            let sh = vsdx::model::find_shape_by_id(&page.shapes, sid)?;
            let chain = dc_core::scope::shape_chain(&ed.doc, page_id, sid).ok()?;
            let g = |n: &str| ed.doc.resolve_cell(sh, &chain, n).and_then(|c| c.numeric());
            Some(((g("BeginX")?, g("BeginY")?), (g("EndX")?, g("EndY")?)))
        });
        let mut reroute = crate::cmd::RerouteConnectors::new_including(page_id, vec![sid]);
        if let Some(pre) = pre_pose {
            reroute = crate::cmd::RerouteConnectors::new_including_with_pre(
                page_id,
                vec![sid],
                [(sid, pre)].into_iter().collect(),
            );
        }
        cmds.push(Box::new(reroute));
        self.apply(Macro::new(
            crate::i18n::strs().msg_undo_glue_connector(
                sid,
                crate::connectors::endpoint_word(end),
            ),
            cmds,
        ));
    }

    /// #108 AutoConnect: an arrow click duplicates the selected shape
    /// one spacing unit in the arrow's direction and connects the pair
    /// (dynamic glue both ends) — one undo Macro, Visio's
    /// click-the-blue-arrow flow.
    fn autoconnect(&mut self, dir: (f64, f64)) {
        use crate::connectors::{
            attach_point, bake_end, bake_trigger, connector_shape, glue_connect,
            rewrite_route_rows, route, route_rows, walkglue_formula, End, GlueKind, GlueTarget,
            ROUTE_RIGHT_ANGLE,
        };
        let Some((page_id, sid)) = self.sel else {
            return;
        };
        let Some(ed) = self.ed() else { return };
        let Some(page) = ed.doc.pages.iter().find(|p| p.id == page_id) else {
            return;
        };
        let Some(sh) = vsdx::model::find_shape_by_id(&page.shapes, sid) else {
            return;
        };
        if crate::connectors::is_connector(sh) || crate::guides::is_guide(sh) {
            self.status = crate::i18n::strs().msg_autoconnect_needs_a_2d_shape();
            return;
        }
        let Some(rect) = self.shape_rect(&ed.doc, page_id, sh) else {
            return;
        };
        // Visio's spacing: one shape-size plus a gap, along the arrow.
        let gap = 0.4;
        let offset = (
            dir.0 * (rect.2 + gap) * dir.0.abs(),
            dir.1 * (rect.3 + gap) * dir.1.abs(),
        );
        let dup_id = dc_core::commands::next_shape_id(&ed.doc, page_id);
        let conn_id = dup_id + 1;
        let src_c = (rect.0 + rect.2 / 2.0, rect.1 + rect.3 / 2.0);
        let dup_rect = (rect.0 + offset.0, rect.1 + offset.1, rect.2, rect.3);
        let dup_c = (dup_rect.0 + rect.2 / 2.0, dup_rect.1 + rect.3 / 2.0);
        let beg_attach = attach_point(rect, dup_c);
        let end_attach = attach_point(dup_rect, src_c);
        let mut conn = connector_shape(conn_id, beg_attach, end_attach);
        let pts = route(ROUTE_RIGHT_ANGLE, beg_attach, end_attach);
        rewrite_route_rows(&mut conn, &route_rows(beg_attach, &pts));
        let beg_t = GlueTarget {
            shape_id: sid,
            kind: GlueKind::Dynamic,
            attach: beg_attach,
        };
        let end_t = GlueTarget {
            shape_id: dup_id,
            kind: GlueKind::Dynamic,
            attach: end_attach,
        };
        bake_end(
            &mut conn,
            End::Begin,
            beg_attach,
            &walkglue_formula(End::Begin),
        );
        bake_trigger(&mut conn, End::Begin, sid);
        bake_end(&mut conn, End::End, end_attach, &walkglue_formula(End::End));
        bake_trigger(&mut conn, End::End, dup_id);
        let cmds: Vec<Box<dyn dc_core::Command>> = vec![
            Box::new(dc_core::commands::DuplicateShape::new(page_id, sid, offset)),
            Box::new(dc_core::AddShape {
                page: page_id,
                parent: None,
                shape: conn,
            }),
            Box::new(dc_core::AddConnect {
                owner: dc_core::ConnectsOwner::Page(page_id),
                connect: glue_connect(conn_id, End::Begin, &beg_t),
            }),
            Box::new(dc_core::AddConnect {
                owner: dc_core::ConnectsOwner::Page(page_id),
                connect: glue_connect(conn_id, End::End, &end_t),
            }),
        ];
        self.apply(Macro::new(
            crate::i18n::strs().msg_undo_autoconnect(sid),
            cmds,
        ));
        self.set_selection(Some((page_id, dup_id)));
        self.sel_set = vec![(page_id, dup_id)];
        self.status = crate::i18n::strs().msg_shape_dup_id_connected_to_sid_au(dup_id, sid);
    }

    /// #156: is the primary selection a container — the target of a
    /// member-side `msvSDContainerShape` reference, a `Container.`
    /// synthetic (#138), or an effective `msvStructureType=Container`
    /// root (the master-based SDCONT family)?
    fn sel_is_container(&self) -> bool {
        let Some((page_id, sid)) = self.sel else {
            return false;
        };
        let Some(ed) = self.ed() else {
            return false;
        };
        let Some(page) = ed.doc.pages.iter().find(|p| p.id == page_id) else {
            return false;
        };
        if !self.container_member_ids(page_id, sid).is_empty() {
            return true;
        }
        let Some(sh) = vsdx::model::find_shape_by_id(&page.shapes, sid) else {
            return false;
        };
        if sh
            .name_u
            .as_deref()
            .is_some_and(|n| n.starts_with("Container."))
        {
            return true;
        }
        let Ok(chain) = shape_chain(&ed.doc, page_id, sid) else {
            return false;
        };
        // #191: the msvStructureType row lives on the MASTER root; a
        // real instance carries local sections, so effective_sheet's
        // either/or return hides it — resolve through the chain.
        ed.doc
            .effective_user_row(sh, &chain, "msvStructureType")
            .and_then(|r| r.cell("Value"))
            .and_then(|c| c.v.clone())
            .is_some_and(|v| v == "Container")
    }

    /// #156: member highlight reads the container's effective
    /// `msvSDContainerNoHighlight` row (Visio's switch); default on.
    fn container_highlight_on(&self) -> bool {
        let Some((page_id, sid)) = self.sel else {
            return true;
        };
        let Some(ed) = self.ed() else {
            return true;
        };
        let Some(page) = ed.doc.pages.iter().find(|p| p.id == page_id) else {
            return true;
        };
        let Some(sh) = vsdx::model::find_shape_by_id(&page.shapes, sid) else {
            return true;
        };
        let Ok(chain) = shape_chain(&ed.doc, page_id, sid) else {
            return true;
        };
        // #191: chain-aware like sel_is_container — the row lives on
        // the master root until an instance edit materializes it.
        ed.doc
            .effective_user_row(sh, &chain, "msvSDContainerNoHighlight")
            .and_then(|r| r.cell("Value"))
            .and_then(|c| c.numeric())
            .map(|v| v == 0.0)
            .unwrap_or(true)
    }

    /// #156: the Container Tools toggle — writes the
    /// `msvSDContainerNoHighlight` User row onto the container
    /// (materialized like Visio's first edit of an inherited row).
    fn toggle_container_highlight(&mut self) {
        let Some((page_id, sid)) = self.sel else {
            return;
        };
        let on = self.container_highlight_on();
        self.apply(Macro::new(
            crate::i18n::strs().msg_toggle_member_highlight(),
            vec![Box::new(dc_core::SetRowCell::new(
                Scope::Shape {
                    page: page_id,
                    shape: sid,
                },
                "User",
                0,
                dc_core::commands::RowSel::Named("msvSDContainerNoHighlight".into()),
                "Value",
                vsdx::Cell::value(if on { "1" } else { "0" }),
            ))],
        ));
        self.status = if on {
            crate::i18n::strs().msg_member_highlight_off()
        } else {
            crate::i18n::strs().msg_member_highlight_on()
        };
    }

    /// #156 (owner note 2026-11-01): which stencil variant matches the
    /// PAPER — an ISO A-size page is metric (the `-m` masters), an
    /// ANSI/Letter page is US units; a custom size falls back to the
    /// pane's "Metric Stencils" preference. The preset pairs mirror
    /// page_setup_window's SIZES table.
    fn paper_is_metric(&self) -> bool {
        let geom = self.page_geom();
        let (lo, hi) = (geom.w_in.min(geom.h_in), geom.w_in.max(geom.h_in));
        const ISO: &[(f64, f64)] = &[
            (5.83, 8.27),   // A5
            (8.27, 11.69),  // A4
            (11.69, 16.54), // A3
            (16.54, 23.39), // A2
            (23.39, 33.11), // A1
            (33.11, 46.81), // A0
        ];
        const ANSI: &[(f64, f64)] = &[
            (8.5, 11.0),  // Letter (ANSI A)
            (8.5, 14.0),  // Legal
            (11.0, 17.0), // Tabloid (ANSI B)
            (17.0, 22.0), // ANSI C
            (22.0, 34.0), // ANSI D
            (34.0, 44.0), // ANSI E
        ];
        let near = |t: &(f64, f64)| (lo - t.0).abs() < 0.02 && (hi - t.1).abs() < 0.02;
        if ISO.iter().any(near) {
            return true;
        }
        if ANSI.iter().any(near) {
            return false;
        }
        self.stencil.metric_units
    }

    /// #156: apply an SDCONT style — the selection (or the selected
    /// container's members, a restyle) wraps in a master-based
    /// container of that style, one undo Macro.
    fn apply_container_style(&mut self, style: &str) {
        let Some(page_id) = self.page_id() else {
            return;
        };
        let Some(ed) = self.ed() else { return };
        // The sdcont source: an open stencil (the session's explicit
        // variant choice), else the embedded catalog parsed in the
        // variant that matches the PAPER (owner note 2026-11-01: ISO
        // A-size → the metric twin sdcont-m, ANSI/Letter → US).
        let src_master: Option<vsdx::Master> = self
            .stencils
            .iter()
            .filter(|(_, st)| st.source.to_string_lossy().contains("sdcont"))
            .find_map(|(_, st)| find_master_by_name(&st.doc, style).cloned())
            .or_else(|| {
                let b = crate::stencil::builtin::by_slug("sdcont")?;
                let doc = vsdx::parse::parse_bytes(b.bytes_for(self.paper_is_metric())).ok()?;
                find_master_by_name(&doc, style).cloned()
            });
        let Some(src) = src_master else {
            self.status = crate::i18n::strs().msg_container_style_style_not_found(style);
            return;
        };
        // Members: the selected container re-wraps (a restyle);
        // anything else wraps the selection itself.
        let (members, old_container) = if self.sel_is_container() {
            let (_, sid) = self.sel.unwrap();
            (self.container_member_ids(page_id, sid), Some(sid))
        } else {
            (
                self.sel_set
                    .iter()
                    .filter(|(p, _)| *p == page_id)
                    .map(|(_, s)| *s)
                    .collect(),
                None,
            )
        };
        if members.is_empty() {
            self.status = crate::i18n::strs().msg_select_the_shapes_to_contain_fir();
            return;
        }
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
        // The style's own member margin (the master's
        // msvSDContainerMargin; SDCONT runs 0.125–0.375).
        let margin = src
            .shapes
            .first()
            .and_then(|root| {
                root.sheet
                    .sections_named("User")
                    .iter()
                    .find_map(|sec| sec.named_row("msvSDContainerMargin"))
            })
            .and_then(|r| r.cell("Value"))
            .and_then(|c| c.numeric())
            .unwrap_or(0.25);
        let rect = (
            x0 - margin,
            y0 - margin,
            x1 - x0 + 2.0 * margin,
            y1 - y0 + 2.0 * margin,
        );
        // Place + size the instance, then move the members over.
        let new_id = dc_core::commands::next_shape_id(&ed.doc, page_id);
        let mut place =
            dc_core::place_master_from_stencil(&ed.doc, page_id, &src, (rect.0, rect.1));
        let mut cmds = std::mem::take(&mut place.cmds);
        for (n, v) in [
            ("PinX", rect.0),
            ("PinY", rect.1),
            ("Width", rect.2),
            ("Height", rect.3),
            ("LocPinX", 0.0),
            ("LocPinY", 0.0),
        ] {
            cmds.push(Box::new(SetCell::new(
                Scope::Shape {
                    page: page_id,
                    shape: new_id,
                },
                n,
                vsdx::Cell::value(format!("{v:.6}")),
            )));
        }
        for m in &members {
            cmds.push(Box::new(dc_core::SetRowCell::new(
                Scope::Shape {
                    page: page_id,
                    shape: *m,
                },
                "User",
                0,
                dc_core::commands::RowSel::Named("msvSDContainerShape".into()),
                "Value",
                vsdx::Cell::value(new_id.to_string()),
            )));
        }
        cmds.push(Box::new(dc_core::commands::ReorderShape::new(
            page_id, new_id, false,
        )));
        if let Some(old) = old_container {
            cmds.push(Box::new(dc_core::commands::RemoveShape::new(page_id, old)));
        }
        self.apply(Macro::new(
            crate::i18n::strs().msg_undo_apply_container_style(style),
            cmds,
        ));
        self.set_selection(Some((page_id, new_id)));
        self.sel_set = vec![(page_id, new_id)];
        self.tab = Tab::ContainerTools;
        self.status = crate::i18n::strs().msg_container_style_style_applied(style);
    }

    /// #108: the Connection Point tool's click — near an existing
    /// point of the hit shape selects it (a second click elsewhere
    /// re-selects), on the body adds a fresh row at the click, both in
    /// the shape's local frame.
    fn handle_conn_point_click(&mut self, page_pos: (f64, f64)) {
        let Some(hit) = self.shape_hit(page_pos) else {
            self.connpt_sel = None;
            self.status = crate::i18n::strs().msg_click_a_shape_to_add_a_connectio();
            return;
        };
        let Some(page_id) = self.page_id() else {
            return;
        };
        // Existing (effective) points as page positions — the same
        // resolution the glue engine uses.
        let near = SNAP_STRENGTH_PX as f64 / self.view.px_per_in() as f64;
        if let Some(cand) = self
            .glue_candidates(&[])
            .iter()
            .find(|c| c.id == hit && !c.conn_rows.is_empty())
        {
            if let Some((ix, _)) = cand
                .conn_rows
                .iter()
                .min_by_key(|(_, p)| ((p.0 - page_pos.0).hypot(p.1 - page_pos.1) * 1e6) as i64)
                .filter(|(_, p)| (p.0 - page_pos.0).hypot(p.1 - page_pos.1) <= near)
            {
                self.connpt_sel = Some((hit, *ix));
                self.status = crate::i18n::strs().msg_connection_point_ix_selected_del(ix);
                return;
            }
        }
        // Add: the local frame of the (top-level) hit shape — the
        // glue engine's rect convention (Pin − LocPin origin).
        let Some(ed) = self.ed() else { return };
        let Some(page) = ed.doc.pages.iter().find(|p| p.id == page_id) else {
            return;
        };
        let Some(sh) = vsdx::model::find_shape_by_id(&page.shapes, hit) else {
            return;
        };
        let Ok(chain) = shape_chain(&ed.doc, page_id, hit) else {
            return;
        };
        let get = |n: &str| ed.doc.resolve_cell(sh, &chain, n).and_then(|c| c.numeric());
        let (Some(px), Some(py), Some(lx), Some(ly)) =
            (get("PinX"), get("PinY"), get("LocPinX"), get("LocPinY"))
        else {
            return;
        };
        let local = (page_pos.0 - (px - lx), page_pos.1 - (py - ly));
        // The next row IX: max across the EFFECTIVE section (a
        // collision would shadow a master row).
        let eff = ed.doc.effective_sheet(sh, &chain);
        let next_ix = eff
            .sections_named("Connection")
            .first()
            .map(|sec| sec.rows.iter().map(|r| r.ix).max().unwrap_or(1) + 1)
            .unwrap_or(2);
        let scope = Scope::Shape {
            page: page_id,
            shape: hit,
        };
        let cell = |v: f64| vsdx::Cell::value(format!("{v:.6}"));
        let cmds: Vec<Box<dyn dc_core::Command>> = vec![
            Box::new(dc_core::SetRowCell::new(
                scope.clone(),
                "Connection",
                0,
                dc_core::commands::RowSel::Ix(next_ix),
                "X",
                cell(local.0),
            )),
            Box::new(dc_core::SetRowCell::new(
                scope,
                "Connection",
                0,
                dc_core::commands::RowSel::Ix(next_ix),
                "Y",
                cell(local.1),
            )),
        ];
        self.apply(Macro::new(
            crate::i18n::strs().msg_add_connection_point_next_ix_on(next_ix, hit),
            cmds,
        ));
        self.connpt_sel = Some((hit, next_ix));
        self.status = crate::i18n::strs().msg_connection_point_next_ix_added_t(next_ix, hit);
    }

    /// #108: Delete on the Connection Point tool's selection — the
    /// LOCAL row goes; a master-inherited point explains instead
    /// (Visio edits those on the master).
    fn delete_selected_conn_point(&mut self) {
        let Some((shape, ix)) = self.connpt_sel else {
            return;
        };
        let Some(page_id) = self.page_id() else {
            return;
        };
        let Some(ed) = self.ed() else { return };
        let local = ed
            .doc
            .pages
            .iter()
            .find(|p| p.id == page_id)
            .and_then(|p| vsdx::model::find_shape_by_id(&p.shapes, shape))
            .map(|sh| {
                sh.sheet
                    .sections_named("Connection")
                    .iter()
                    .any(|sec| sec.rows.iter().any(|r| r.ix == ix))
            })
            .unwrap_or(false);
        if !local {
            self.status =
                crate::i18n::strs().msg_connection_point_ix_is_inherited(ix);
            return;
        }
        self.apply(Macro::new(
            crate::i18n::strs().msg_remove_connection_point_ix(ix),
            vec![Box::new(dc_core::commands::RemoveSectionRow::new(
                Scope::Shape {
                    page: page_id,
                    shape,
                },
                "Connection",
                0,
                dc_core::commands::RowSel::Ix(ix),
            ))],
        ));
        self.connpt_sel = None;
        self.status = crate::i18n::strs().msg_connection_point_ix_removed(ix);
    }

    /// #108/URS-1114: set a connector's route style and re-route its
    /// geometry immediately — one undo step.
    fn apply_connector_route_style(&mut self, page_id: i64, sid: i64, style: &str) {
        let Some(ed) = self.ed.as_ref() else { return };
        let Some(page) = ed.doc.pages.iter().find(|p| p.id == page_id) else {
            return;
        };
        let Some(sh) = vsdx::model::find_shape_by_id(&page.shapes, sid) else {
            return;
        };
        let Ok(chain) = shape_chain(&ed.doc, page_id, sid) else {
            return;
        };
        let get = |n: &str| ed.doc.resolve_cell(sh, &chain, n).and_then(|c| c.numeric());
        let (Some(bx), Some(by), Some(ex), Some(ey)) =
            (get("BeginX"), get("BeginY"), get("EndX"), get("EndY"))
        else {
            return;
        };
        let pts = crate::connectors::route(style, (bx, by), (ex, ey));
        let rows = crate::connectors::route_rows((bx, by), &pts);
        let scope = Scope::Shape {
            page: page_id,
            shape: sid,
        };
        let mut cmds: Vec<Box<dyn dc_core::Command>> = vec![Box::new(SetCell::new(
            scope.clone(),
            "ShapeRouteStyle",
            vsdx::Cell::value(style),
        ))];
        for (i, (x, y)) in rows.iter().enumerate() {
            let ix = (i + 2) as i64;
            for (cell, v) in [("X", *x), ("Y", *y)] {
                cmds.push(Box::new(dc_core::SetRowCell::new(
                    scope.clone(),
                    "Geometry",
                    0,
                    dc_core::commands::RowSel::Ix(ix),
                    cell,
                    vsdx::Cell::value(format!("{v:.6}")),
                )));
            }
        }
        // Curved (#108): row 2 becomes a CubicBezTo whose A/B/C/D
        // controls sit at mid-x/the endpoints' heights as Width/Height
        // formulas (an S — chord-collinear controls would render
        // straight; a reroute's size rewrite re-tracks them
        // automatically); every other style keeps the rows plain
        // LineTo.
        let curved = style == crate::connectors::ROUTE_CURVED;
        for ix in 2i64..=4 {
            let t = if ix == 2 && curved {
                Some("CubicBezTo".to_string())
            } else {
                Some("LineTo".to_string())
            };
            cmds.push(Box::new(dc_core::commands::SetSectionRowType::new(
                scope.clone(),
                "Geometry",
                0,
                dc_core::commands::RowSel::Ix(ix),
                t,
            )));
        }
        if curved {
            for (name, f, v) in crate::connectors::curved_row_extras(ex - bx, ey - by) {
                cmds.push(Box::new(dc_core::SetRowCell::new(
                    scope.clone(),
                    "Geometry",
                    0,
                    dc_core::commands::RowSel::Ix(2),
                    name,
                    vsdx::Cell {
                        v: Some(format!("{v:.6}")),
                        f: Some(f),
                        ..vsdx::Cell::default()
                    },
                )));
            }
        }
        self.apply(Macro::new(crate::i18n::strs().msg_set_connector_route_style(), cmds));
    }

    /// The plain endpoint-move command (the #59 derived-cell set) for
    /// `commit_connector_endpoint` — the same cells
    /// `apply_line_endpoint` writes, as one boxed command.
    fn move_endpoint_cmd(
        &self,
        page_id: i64,
        sid: i64,
        end: crate::connectors::End,
        to: (f64, f64),
    ) -> Box<dyn dc_core::Command> {
        let Some(ed) = self.ed() else {
            unreachable!("caller checked the document")
        };
        let page = ed.doc.pages.iter().find(|p| p.id == page_id);
        let Some(sh) = page.and_then(|p| vsdx::model::find_shape_by_id(&p.shapes, sid)) else {
            unreachable!("caller checked the shape")
        };
        let Ok(chain) = shape_chain(&ed.doc, page_id, sid) else {
            unreachable!("caller checked the chain")
        };
        let get = |n: &str| ed.doc.resolve_cell(sh, &chain, n).and_then(|c| c.numeric());
        let (bx, by, ex, ey) = (
            get("BeginX").unwrap_or(0.0),
            get("BeginY").unwrap_or(0.0),
            get("EndX").unwrap_or(0.0),
            get("EndY").unwrap_or(0.0),
        );
        let (a, b) = match end {
            crate::connectors::End::Begin => (to, (ex, ey)),
            crate::connectors::End::End => ((bx, by), to),
        };
        let w = (a.0 - b.0).hypot(a.1 - b.1).max(1e-4);
        let angle = (b.1 - a.1).atan2(b.0 - a.0);
        let scope = Scope::Shape {
            page: page_id,
            shape: sid,
        };
        let cell = |v: f64| vsdx::Cell::value(format!("{v:.6}"));
        // NOTE: for the corpus-model connector (unrotated, signed
        // Width/Height) the derived set differs from the 1D master
        // model — Width/Height are the axis deltas and the pin the
        // midpoint.
        let cmds: Vec<Box<dyn dc_core::Command>> = [
            ("BeginX", a.0),
            ("BeginY", a.1),
            ("EndX", b.0),
            ("EndY", b.1),
            ("PinX", (a.0 + b.0) / 2.0),
            ("PinY", (a.1 + b.1) / 2.0),
            ("Width", b.0 - a.0),
            ("Height", b.1 - a.1),
            ("LocPinX", (b.0 - a.0) / 2.0),
            ("LocPinY", (b.1 - a.1) / 2.0),
        ]
        .into_iter()
        // Visio-authored connectors GUARD their derived cells
        // (GUARD(EndX-BeginX)…) — a literal write would refuse and
        // abort the whole Macro (CodeRabbit); only formula-free
        // derived cells write. And only cells the INSTANCE owns: an
        // inherited master literal (the Line-arc corpus Height=0)
        // stays inherited — materializing a signed-delta literal over
        // it corrupts the master link and the frame (#226 ownership;
        // the reroute that follows in the same Macro derives the
        // chord-model frame cells from the endpoints).
        // The endpoint cells go through SetGluedCell (the glue engine
        // owns their formulas).
        .filter(|(n, _)| {
            matches!(*n, "BeginX" | "BeginY" | "EndX" | "EndY")
                || (dc_core::commands::effective_formula(&ed.doc, sh, &chain, n).is_none()
                    && sh.sheet.cell(n).is_some())
        })
        .map(|(n, v)| {
            if matches!(n, "BeginX" | "BeginY" | "EndX" | "EndY") {
                // The ANCHORED end keeps its glue formula: a plain
                // Cell::value write replaces the cell wholesale and
                // un-glues it — the begin formula died on the first
                // endpoint drag (utilisation.vsdx, #226 evidence). The
                // glue engine updates the cache behind its formula;
                // the DRAGGED end writes a literal (a release makes it
                // plain, and a re-glue overwrites it with the glue
                // formula in the same Macro).
                let f = if (matches!(end, crate::connectors::End::End)
                    && matches!(n, "BeginX" | "BeginY"))
                    || (matches!(end, crate::connectors::End::Begin)
                        && matches!(n, "EndX" | "EndY"))
                {
                    sh.sheet.cell(n).and_then(|c| c.f.clone())
                } else {
                    None
                };
                Box::new(dc_core::SetGluedCell::new(
                    scope.clone(),
                    n,
                    vsdx::Cell {
                        v: Some(format!("{v:.6}")),
                        f,
                        ..vsdx::Cell::default()
                    },
                )) as Box<dyn dc_core::Command>
            } else {
                Box::new(SetCell::new(scope.clone(), n, cell(v))) as Box<dyn dc_core::Command>
            }
        })
        .collect();
        let _ = (w, angle);
        Box::new(Macro::new(
            crate::i18n::strs().msg_undo_move_endpoint(crate::connectors::endpoint_word(end)),
            cmds,
        ))
    }

    /// The page's glue candidates (#108): every non-connector shape's
    /// page rect with its connection-point rows (IX → page position —
    /// own rows first, the master's for instances, like #109's
    /// snap-target collection).
    /// #108 line jumps: every top-level connector's page polyline
    /// (unrotated, instance- or master-geometry through the hit
    /// frame) with its stroke, in z order — the input to
    /// [`crate::connectors::line_jumps`].
    fn jump_lines(&self) -> Vec<crate::connectors::JumpLine> {
        let mut out = Vec::new();
        let Some(ed) = self.ed() else {
            return out;
        };
        let Some(page_id) = self.page_id() else {
            return out;
        };
        let Some(page) = ed.doc.pages.iter().find(|p| p.id == page_id) else {
            return out;
        };
        for (z, sh) in page.shapes.iter().enumerate() {
            let Ok(chain) = shape_chain(&ed.doc, page_id, sh.id) else {
                continue;
            };
            if !crate::connectors::is_connector_in(&ed.doc, &chain, sh) {
                continue;
            }
            // #187 slice 2: rotated connectors hop too — the polyline
            // source (HitFrame + geometry_hit_segments) maps the
            // rotated frame, and the overlay's arc normal is chosen
            // per jump in page space, so no Angle gate remains.
            let (root, msize) = sh
                .master
                .and_then(|mid| ed.doc.master(mid))
                .and_then(|m| m.shapes.first())
                .map(|r| {
                    let g = |n: &str| {
                        ed.doc
                            .resolve_cell(r, &[r], n)
                            .and_then(|c| c.numeric())
                            .unwrap_or(1.0)
                    };
                    (Some(r), (g("Width"), g("Height")))
                })
                .unwrap_or((None, (1.0, 1.0)));
            let frame = crate::connectors::hit_frame(&ed.doc, &chain, sh, msize);
            let segs = crate::connectors::geometry_hit_segments(sh, root, &frame);
            let mut pts = Vec::with_capacity(segs.len() + 1);
            if let Some((a, _)) = segs.first() {
                pts.push(*a);
            }
            for (_, b) in &segs {
                pts.push(*b);
            }
            let weight = ed
                .doc
                .resolve_cell(sh, &chain, "LineWeight")
                .and_then(|c| c.numeric())
                .unwrap_or(0.01);
            // #235 corollary (CodeRabbit, #278): an explicit-zero
            // LineWeight paints nothing — the export path
            // (`page_jumps`) drops such connectors from the crossing
            // set, and the canvas overlay must agree or a visible
            // connector hops over an invisible shaft on screen while
            // the export shows no hop.
            if weight == 0.0 {
                continue;
            }
            let color = ed
                .doc
                .resolve_cell(sh, &chain, "LineColor")
                .and_then(|c| c.v.clone())
                .and_then(|v| parse_hex_rgb(&v))
                .unwrap_or((0x42, 0x42, 0x42));
            out.push(crate::connectors::JumpLine {
                id: sh.id,
                z,
                weight,
                color,
                pts,
            });
        }
        out
    }

    fn glue_candidates(&self, excluding: &[i64]) -> Vec<crate::connectors::GlueCandidate> {
        let Some(ed) = self.ed() else {
            return Vec::new();
        };
        let Some(page_id) = self.page_id() else {
            return Vec::new();
        };
        let Some(page) = ed.doc.pages.iter().find(|p| p.id == page_id) else {
            return Vec::new();
        };
        page.shapes
            .iter()
            .filter(|s| {
                !excluding.contains(&s.id)
                    && !crate::connectors::is_connector(s)
                    && !crate::guides::is_guide(s)
            })
            .filter_map(|s| {
                let rect = self.shape_rect(&ed.doc, page_id, s)?;
                let mut c = crate::connectors::GlueCandidate {
                    id: s.id,
                    rect,
                    conn_rows: Vec::new(),
                };
                let collect =
                    |sheet: &vsdx::model::Sheet, c: &mut crate::connectors::GlueCandidate| {
                        for sec in sheet.sections_named("Connection") {
                            for row in &sec.rows {
                                if let (Some(x), Some(y)) = (
                                    row.cell("X").and_then(|v| v.numeric()),
                                    row.cell("Y").and_then(|v| v.numeric()),
                                ) {
                                    c.conn_rows.push((row.ix, (rect.0 + x, rect.1 + y)));
                                }
                            }
                        }
                    };
                collect(&s.sheet, &mut c);
                if c.conn_rows.is_empty() {
                    if let Some(root) = s
                        .master
                        .and_then(|mid| ed.doc.master(mid))
                        .and_then(|m| m.shapes.first())
                    {
                        collect(&root.sheet, &mut c);
                    }
                }
                Some(c)
            })
            .collect()
    }

    /// The Connector Tool's gesture finished (#108, URS-1067): create
    /// the connector between the two points, gluing each end per the
    /// #109 glue settings (connection point → static, shape →
    /// dynamic) — corpus formulas baked into the shape cells, Connect
    /// records in the same undo step.
    fn create_connector(&mut self, from: (f64, f64), to: (f64, f64)) {
        use crate::connectors::{End, GlueKind};
        let Some(page_id) = self.page_id() else {
            return;
        };
        let Some(ed) = self.ed.as_ref() else { return };
        let _geom = self.page_geom();
        let cands = self.glue_candidates(&[]);
        let threshold = SNAP_STRENGTH_PX as f64 / self.view.px_per_in() as f64;
        let decide = |p, toward| {
            crate::connectors::glue_decision(
                &cands,
                p,
                toward,
                threshold,
                self.snap.glue_active,
                self.snap.glue_connection_points,
            )
        };
        let beg_t = decide(from, to);
        let end_t = decide(to, from);
        let beg = beg_t.map(|t| t.attach).unwrap_or(from);
        let end = end_t.map(|t| t.attach).unwrap_or(to);
        let id = dc_core::commands::next_shape_id(&ed.doc, page_id);
        let mut shape = crate::connectors::connector_shape(id, beg, end);
        // Route the fresh connector per its style before it lands.
        let style = shape
            .sheet
            .cell("ShapeRouteStyle")
            .and_then(|c| c.v.clone())
            .unwrap_or_else(|| crate::connectors::ROUTE_RIGHT_ANGLE.into());
        let pts = crate::connectors::route(&style, beg, end);
        let rows = crate::connectors::route_rows(beg, &pts);
        let _ = rows;
        // (the straight default rows the constructor wrote stand until
        // the first reroute; a right-angle start routes now)
        if style != crate::connectors::ROUTE_STRAIGHT {
            crate::connectors::rewrite_route_rows(&mut shape, &rows);
        }
        // Bake the glue: endpoint formulas + triggers into the cells.
        for (end, t) in [(End::Begin, beg_t), (End::End, end_t)] {
            match t.map(|t| (t.shape_id, t.kind, t.attach)) {
                Some((tid, GlueKind::Static { row_ix }, attach)) => {
                    let f = crate::connectors::static_glue_formula(tid, row_ix);
                    crate::connectors::bake_end(&mut shape, end, attach, &f);
                }
                Some((tid, GlueKind::Dynamic, attach)) => {
                    let f = crate::connectors::walkglue_formula(end);
                    crate::connectors::bake_end(&mut shape, end, attach, &f);
                    crate::connectors::bake_trigger(&mut shape, end, tid);
                }
                None => {}
            }
        }
        let mut cmds: Vec<Box<dyn dc_core::Command>> = vec![Box::new(dc_core::AddShape {
            page: page_id,
            parent: None,
            shape,
        })];
        for (end, t) in [(End::Begin, beg_t), (End::End, end_t)] {
            if let Some(t) = t {
                cmds.push(Box::new(dc_core::AddConnect {
                    owner: dc_core::ConnectsOwner::Page(page_id),
                    connect: crate::connectors::glue_connect(id, end, &t),
                }));
            }
        }
        self.apply(Macro::new(crate::i18n::strs().msg_undo_add_connector(id), cmds));
        self.set_selection(Some((page_id, id)));
        self.sel_set = vec![(page_id, id)];
        self.status = match (beg_t, end_t) {
            (Some(_), Some(_)) => crate::i18n::strs().msg_connector_glued_at_both_ends(),
            (Some(_), None) | (None, Some(_)) => {
                crate::i18n::strs().msg_connector_glued_at_one_end()
            }
            (None, None) => crate::i18n::strs().msg_connector_drawn_free_ends(),
        };
    }

    /// #132 (URS-1225): a sampled pen stroke becomes a new shape —
    /// [`crate::freeform`] fits LineTo/ArcTo rows into the bbox
    /// frame; ordinary editable geometry through the add-shape path,
    /// one undo step, selected on landing.
    fn create_freeform(&mut self, stroke: Vec<(f64, f64)>) {
        let Some(page_id) = self.page_id() else {
            return;
        };
        let Some(ed) = self.ed.as_ref() else {
            return;
        };
        let id = dc_core::commands::next_shape_id(&ed.doc, page_id);
        let Some(shape) = crate::freeform::build_shape(id, &stroke) else {
            self.status = crate::i18n::strs().msg_freeform_stroke_too_short_to_dra();
            return;
        };
        let segs = shape
            .sheet
            .sections_named("Geometry")
            .first()
            .map(|g| g.rows.len().saturating_sub(1))
            .unwrap_or(0);
        self.apply(Macro::new(
            crate::i18n::strs().msg_freeform_shape_id(id),
            vec![Box::new(dc_core::AddShape {
                page: page_id,
                parent: None,
                shape,
            })],
        ));
        self.set_selection(Some((page_id, id)));
        self.sel_set = vec![(page_id, id)];
        self.status =
            crate::i18n::strs().msg_freeform_shape_created_segs_segm(segs);
    }

    /// #108 reroute: a deferred [`crate::cmd::RerouteConnectors`]
    /// riding at the tail of the move/resize Macro — it evaluates
    /// against the post-move document (an eager build would read
    /// stale, pre-move geometry). `None` when nothing glues to the
    /// moved shapes, so Macros stay clean of no-ops.
    fn reroute_cmd(&self, page_id: i64, moved: &[i64]) -> Option<Box<dyn dc_core::Command>> {
        let ed = self.ed()?;
        let page = ed.doc.pages.iter().find(|p| p.id == page_id)?;
        let touches = page
            .connects()
            .list
            .iter()
            .any(|c| moved.contains(&c.to_sheet));
        touches.then(|| {
            Box::new(crate::cmd::RerouteConnectors::new(page_id, moved.to_vec()))
                as Box<dyn dc_core::Command>
        })
    }

    /// #113: a ruler drag released over the page becomes a guide
    /// (URS-1076): one undoable AddShape carrying the synthetic
    /// clean-room guide cells, then selected like any placement.
    fn add_guide(&mut self, src: canvas::GuideSource, at: (f64, f64)) {
        use crate::guides::GuideKind;
        let Some(page_id) = self.page_id() else {
            return;
        };
        let Some(ed) = self.ed.as_ref() else { return };
        let geom = self.page_geom();
        let kind = match src {
            canvas::GuideSource::Vertical => GuideKind::Vertical,
            canvas::GuideSource::Horizontal => GuideKind::Horizontal,
            canvas::GuideSource::Point => GuideKind::Point,
        };
        let id = dc_core::commands::next_shape_id(&ed.doc, page_id);
        let shape = crate::guides::guide_shape(id, kind, at, (geom.w_in, geom.h_in));
        let label = match kind {
            GuideKind::Vertical => "add vertical guide",
            GuideKind::Horizontal => "add horizontal guide",
            GuideKind::Point => "add guide point",
        };
        self.apply(dc_core::AddShape {
            page: page_id,
            parent: None,
            shape,
        });
        self.set_selection(Some((page_id, id)));
        self.sel_set = vec![(page_id, id)];
        self.status = match kind {
            GuideKind::Vertical => crate::i18n::strs()
                    .msg_guide_added_vertical(self.view.unit.fmt(at.0)),
            GuideKind::Horizontal => {
                crate::i18n::strs().msg_guide_added_horizontal(self.view.unit.fmt(at.1))
            }
            GuideKind::Point => {
                crate::i18n::strs().msg_guide_added_point(self.view.unit.fmt_pt(at.0, at.1))
            }
        };
        let _ = label;
    }

    /// #113: commit a guide drag — the guide moves through the normal
    /// 1D/pin move path, and every shape glued to it (pin formulas
    /// naming this guide) follows: cached values rewritten in the
    /// same undo step, formulas kept for the file's sake (a real
    /// Visio re-evaluates them live).
    fn apply_guide_move(&mut self, page_id: i64, guide_id: i64, to_pin: (f64, f64)) {
        let mut cmds: Vec<Box<dyn dc_core::Command>> =
            vec![self.move_cmd_for(page_id, guide_id, to_pin)];
        if let Some(ed) = self.ed() {
            let page = ed.doc.pages.iter().find(|p| p.id == page_id);
            if let Some(page) = page {
                for (sid, axes) in crate::guides::glue_dependents(&page.shapes, guide_id) {
                    for (cell, guide_axis) in axes {
                        let v = if guide_axis == "PinX" {
                            to_pin.0
                        } else {
                            to_pin.1
                        };
                        cmds.push(Box::new(SetCell::new(
                            Scope::Shape {
                                page: page_id,
                                shape: sid,
                            },
                            cell,
                            vsdx::Cell {
                                v: Some(format!("{v:.6}")),
                                f: Some(crate::guides::glue_formula(guide_id, guide_axis)),
                                ..vsdx::Cell::default()
                            },
                        )));
                    }
                }
            }
        }
        self.apply(Macro::new(
            crate::i18n::strs().msg_undo_move_guide(guide_id),
            cmds,
        ));
    }

    /// #109: snap a single dragged point — a 1D endpoint under the
    /// glue settings, a geometry vertex under the snap settings —
    /// against the page minus the dragged shape.
    fn snap_point_for(
        &self,
        dragged: i64,
        p: (f64, f64),
        glue: bool,
    ) -> Option<((f64, f64), crate::snap::SnapHit)> {
        let page_id = self.page_id()?;
        let targets = self.snap_targets(page_id, &[dragged]);
        let guide_list = self.snap_guides(&[dragged]);
        let ctx = self.snap_ctx_of(&targets, &guide_list);
        crate::snap::point_snap(p, &ctx, &self.snap, glue)
    }

    /// #202: the CONNECTOR-ENDPOINT snap — connection points and
    /// guides only. The shape-handle/vertex magnets made drops near a
    /// shape's edge snap onto the edge and re-glue there (the
    /// endpoints felt hard-glued and afterwards slid only along the
    /// shape's edges); Visio's endpoint drag magnetizes to connection
    /// points, and a drop off-shape stays loose.
    /// #226: the live endpoint-drag ghost — the dragged connector's
    /// ink re-rendered through [`plan_connector_frame`] (the SAME
    /// planner the release commits) at the snapped pointer, so what
    /// you drag is exactly what lands. Re-rasters only when the
    /// snapped point moves; motionless frames reuse the texture.
    fn endpoint_preview_quad(
        &mut self,
        ctx: &egui::Context,
        page_id: i64,
        sid: i64,
        hid: HandleId,
        snap: crate::canvas::SnapEndpointFn,
        ptr_page: (f64, f64),
    ) -> Option<crate::layers::ShapeQuad> {
        // (Re-)arm the snapshot: the doc at gesture start, the #175
        // row-translation anchor (the pre-drag END), no texture yet.
        let needs_arm = self
            .endpoint_preview
            .as_ref()
            .map(|p| p.conn_id != sid)
            .unwrap_or(true);
        if needs_arm {
            let ed = self.ed.as_ref()?;
            let page = ed.doc.pages.iter().find(|p| p.id == page_id)?;
            let sh = vsdx::model::find_shape_by_id(&page.shapes, sid)?;
            let chain = shape_chain(&ed.doc, page_id, sid).ok()?;
            let g = |n: &str| ed.doc.resolve_cell(sh, &chain, n).and_then(|c| c.numeric());
            let pre_end = (g("EndX")?, g("EndY")?);
            let pre_beg = (g("BeginX")?, g("BeginY")?);
            // The dragged end's own origin at gesture start — the
            // commit path's drag anchor (HandleId::LineBegin drags the
            // Begin pair).
            let origin = match hid {
                HandleId::LineEnd => pre_end,
                _ => pre_beg,
            };
            self.endpoint_preview = Some(EndpointPreview {
                doc: ed.doc.clone(),
                conn_id: sid,
                pre_end,
                pre_beg,
                grab_off: (origin.0 - ptr_page.0, origin.1 - ptr_page.1),
                last_to: (f64::NAN, f64::NAN),
                tex: None,
                bbox: (0.0, 0.0, 0.0, 0.0),
            });
        }
        // The preview endpoint: the pointer PLUS the arm-time offset —
        // the same place the commit's delta-from-origin lands — then
        // the endpoint snap, exactly as the release path snaps.
        let to = {
            let ep = self.endpoint_preview.as_ref()?;
            snap(true, (ptr_page.0 + ep.grab_off.0, ptr_page.1 + ep.grab_off.1))
        };
        // The endpoint pair in DOCUMENT order: the dragged end moves
        // to `to`, the anchored end stays at its resolved position (a
        // glued anchor's cache already names its attach point).
        let (beg, endp) = {
            let ed = self.ed.as_ref()?;
            let page = ed.doc.pages.iter().find(|p| p.id == page_id)?;
            let sh = vsdx::model::find_shape_by_id(&page.shapes, sid)?;
            let chain = shape_chain(&ed.doc, page_id, sid).ok()?;
            let g = |n: &str| ed.doc.resolve_cell(sh, &chain, n).and_then(|c| c.numeric());
            match hid {
                HandleId::LineEnd => ((g("BeginX")?, g("BeginY")?), to),
                _ => (to, (g("EndX")?, g("EndY")?)),
            }
        };
        let ep = self.endpoint_preview.as_mut()?;
        if ep.last_to == to && ep.tex.is_some() {
            let tex = ep.tex.as_ref()?;
            return Some(crate::layers::ShapeQuad {
                id: sid,
                tex: tex.id(),
                bbox: ep.bbox,
            });
        }
        // A fresh connector into the snapshot each frame: the plan's
        // frame cells are absolute but its #175 row translations are
        // relative to the PRE-drag pose — re-planning over an
        // already-translated clone would accumulate the delta.
        let fresh = {
            let ed = self.ed.as_ref()?;
            let page = ed.doc.pages.iter().find(|p| p.id == page_id)?;
            vsdx::model::find_shape_by_id(&page.shapes, sid)?.clone()
        };
        let plan = {
            let page2 = ep.doc.pages.iter_mut().find(|p| p.id == page_id)?;
            let slot = vsdx::model::find_shape_by_id_mut(&mut page2.shapes, sid)?;
            *slot = fresh;
            let page2 = ep.doc.pages.iter().find(|p| p.id == page_id)?;
            let conn = vsdx::model::find_shape_by_id(&page2.shapes, sid)?;
            let chain2 = shape_chain(&ep.doc, page_id, sid).ok()?;
            crate::cmd::plan_connector_frame(
                &ep.doc,
                conn,
                &chain2,
                beg,
                endp,
                ep.pre_end,
                ep.pre_beg,
            )
        };
        // Apply the plan's writes to the snapshot (plain cache writes
        // — the renderer reads values, not formulas).
        {
            let page2 = ep.doc.pages.iter_mut().find(|p| p.id == page_id)?;
            let conn = vsdx::model::find_shape_by_id_mut(&mut page2.shapes, sid)?;
            for (n, v, _) in &plan.cells {
                conn.sheet.set_cell(n, vsdx::Cell::value(format!("{v:.6}")));
            }
            for (sec_ix, row_ix, name, cell) in &plan.row_cells {
                let _ = dc_core::commands::set_row_cell(
                    &mut conn.sheet,
                    "Geometry",
                    *sec_ix,
                    &dc_core::commands::RowSel::Ix(*row_ix),
                    name,
                    cell.clone(),
                );
            }
            for (ix, x, y) in &plan.rows {
                for (name, v) in [("X", *x), ("Y", *y)] {
                    let _ = dc_core::commands::set_row_cell(
                        &mut conn.sheet,
                        "Geometry",
                        0,
                        &dc_core::commands::RowSel::Ix(*ix),
                        name,
                        vsdx::Cell::value(format!("{v:.6}")),
                    );
                }
            }
        }
        // Raster the snapshot's connector through the page renderer —
        // the full styled ink (dash, arrow, weight, color), the same
        // pipeline the committed layer lands through.
        let page2 = ep.doc.pages.iter().find(|p| p.id == page_id)?;
        let bounds = dc::svg::shape_ink_bounds(&ep.doc, page2, sid)?;
        let svg = dc::svg::render_shape_layer(&ep.doc, page2, sid, &[], bounds).ok()?;
        let w_px = (bounds.2 * f64::from(self.view.px_per_in())).clamp(16.0, 2048.0);
        let tree = crate::svgimg::parse_svg(&svg, crate::perf::Kind::Page).ok()?;
        let img = crate::svgimg::render_tree(&tree, w_px as f32, crate::perf::Kind::Page).ok()?;
        ep.tex = Some(ctx.load_texture("endpoint-drag-preview", img, egui::TextureOptions::LINEAR));
        ep.bbox = bounds;
        ep.last_to = to;
        let tex = ep.tex.as_ref()?;
        Some(crate::layers::ShapeQuad {
            id: sid,
            tex: tex.id(),
            bbox: ep.bbox,
        })
    }

    fn snap_endpoint_for(&self, dragged: i64, p: (f64, f64)) -> (f64, f64) {
        let page_id = self.page_id().unwrap_or(0);
        let targets = self.snap_targets(page_id, &[dragged]);
        let guide_list = self.snap_guides(&[dragged]);
        let ctx = self.snap_ctx_of(&targets, &guide_list);
        let mut s = self.snap;
        s.glue_shape_handles = false;
        s.glue_shape_vertices = false;
        crate::snap::point_snap(p, &ctx, &s, true).map_or(p, |(q, _)| q)
    }

    /// #160: per-axis corner snap for a resize drag — the SNAP
    /// settings (resize edges are snap targets, not glue targets),
    /// against the page minus the resized shape.
    fn snap_corner_for(&self, dragged: i64, p: (f64, f64)) -> Option<(f64, f64)> {
        let page_id = self.page_id()?;
        let targets = self.snap_targets(page_id, &[dragged]);
        let guide_list = self.snap_guides(&[dragged]);
        let ctx = self.snap_ctx_of(&targets, &guide_list);
        crate::snap::corner_snap(p, &ctx, &self.snap)
    }

    /// #110 (URS-1149): align every selected shape's feature to the
    /// first-selected shape (or the page while `align_to_page`): one
    /// undo Macro of plain moves.
    fn align_selection(&mut self, kind: crate::align::AlignKind) {
        use crate::align::AlignTarget;
        let Some(page_id) = self.page_id() else {
            return;
        };
        let Some(ed) = self.ed() else { return };
        let page = &ed.doc.pages[self.page_idx];
        // Selection order: the primary leads (URS's first-selected).
        let mut ids: Vec<i64> = Vec::new();
        if let Some((p, first)) = self.sel {
            if p == page_id && !ids.contains(&first) {
                ids.push(first);
            }
        }
        for (p, sid) in &self.sel_set {
            if *p == page_id && !ids.contains(sid) {
                ids.push(*sid);
            }
        }
        let mut shapes: Vec<(i64, (f64, f64, f64, f64))> = Vec::new();
        for sid in &ids {
            let Some(sh) = vsdx::model::find_shape_by_id(&page.shapes, *sid) else {
                continue;
            };
            if let Some(r) = self.shape_rect(&ed.doc, page_id, sh) {
                shapes.push((*sid, r));
            }
        }
        if shapes.len() < 2 && !self.align_to_page {
            self.status = crate::i18n::strs().msg_select_two_or_more_shapes_to_ali();
            return;
        }
        if shapes.is_empty() {
            return;
        }
        let geom = self.page_geom();
        let target = if self.align_to_page {
            AlignTarget::Page
        } else {
            AlignTarget::FirstSelected
        };
        let moves = crate::align::align_moves(&shapes, kind, target, (geom.w_in, geom.h_in));
        self.apply_moves(
            &crate::i18n::strs().msg_undo_align_what(kind.label_i18n()),
            page_id,
            &moves,
        );
    }

    /// #110 (URS-1161): distribute the selection with even gaps
    /// (3+ shapes, Visio's "make spacing equal").
    fn distribute_selection(&mut self, vertical: bool) {
        let Some(page_id) = self.page_id() else {
            return;
        };
        let Some(ed) = self.ed() else { return };
        let page = &ed.doc.pages[self.page_idx];
        let mut shapes: Vec<(i64, (f64, f64, f64, f64))> = Vec::new();
        for (p, sid) in &self.sel_set {
            if *p != page_id {
                continue;
            }
            if let Some(sh) = vsdx::model::find_shape_by_id(&page.shapes, *sid) {
                if let Some(r) = self.shape_rect(&ed.doc, page_id, sh) {
                    shapes.push((*sid, r));
                }
            }
        }
        match crate::align::distribute_moves(&shapes, vertical) {
            Some(moves) => {
                let label = if vertical {
                    crate::i18n::strs().msg_distribute_vertical_spacing()
                } else {
                    crate::i18n::strs().msg_distribute_horizontal_spacing()
                };
                self.apply_moves(&label, page_id, &moves);
            }
            None => self.status = crate::i18n::strs().msg_distribute_needs_three_or_more_s(),
        }
    }

    /// Apply (id, delta) moves as one undo Macro — the same move path
    /// drags use, so 1D members translate their endpoints too.
    fn apply_moves(&mut self, label: &str, page_id: i64, moves: &[(i64, (f64, f64))]) {
        let Some(ed) = self.ed() else { return };
        let page = &ed.doc.pages[self.page_idx];
        let mut cmds: Vec<Box<dyn dc_core::Command>> = Vec::new();
        for (sid, (dx, dy)) in moves {
            if dx.abs() < 1e-9 && dy.abs() < 1e-9 {
                continue;
            }
            let Some(sh) = vsdx::model::find_shape_by_id(&page.shapes, *sid) else {
                continue;
            };
            let Ok(chain) = shape_chain(&ed.doc, page_id, *sid) else {
                continue;
            };
            let pin = |n: &str| ed.doc.resolve_cell(sh, &chain, n).and_then(|c| c.numeric());
            if let (Some(px), Some(py)) = (pin("PinX"), pin("PinY")) {
                cmds.push(self.move_cmd_for(page_id, *sid, (px + dx, py + dy)));
            }
        }
        if cmds.is_empty() {
            self.status = crate::i18n::strs().msg_already_aligned();
            return;
        }
        self.apply(Macro::new(label.to_string(), cmds));
    }

    /// #110 (URS-1217 slice): keyboard nudge — one undo step per
    /// press, the whole selection moves.
    fn nudge_selection(&mut self, dx: f64, dy: f64) {
        let Some(page_id) = self.page_id() else {
            return;
        };
        let ids: Vec<i64> = if self.sel_set.len() > 1 {
            self.sel_set
                .iter()
                .filter(|(p, _)| *p == page_id)
                .map(|(_, s)| *s)
                .collect()
        } else {
            self.sel.map(|(_, s)| vec![s]).unwrap_or_default()
        };
        let moves: Vec<(i64, (f64, f64))> = ids.iter().map(|id| (*id, (dx, dy))).collect();
        self.apply_moves(&crate::i18n::strs().msg_nudge_selection(), page_id, &moves);
    }

    /// #180: the documented Visio unglue-all operation — any arrow key
    /// with glued connectors selected releases every end of every
    /// selected connector in ONE undo step ("select one or more
    /// connectors and press any arrow key", MS Learn — Glue or unglue
    /// connectors). The press is consumed: a glued connector does not
    /// nudge until unglued, Visio's own tradeoff. Returns false when
    /// nothing in the selection is glued so the arrow falls through to
    /// nudge; locked ends refuse atomically through the Macro
    /// rollback (a failed child leaves the document untouched).
    fn unglue_selection_connectors(&mut self) -> bool {
        let Some(page_id) = self.page_id() else {
            return false;
        };
        let ids: Vec<i64> = if self.sel_set.len() > 1 {
            self.sel_set
                .iter()
                .filter(|(p, _)| *p == page_id)
                .map(|(_, s)| *s)
                .collect()
        } else {
            self.sel.map(|(_, s)| vec![s]).unwrap_or_default()
        };
        let Some(ed) = self.ed() else {
            return false;
        };
        let Some(page) = ed.doc.pages.iter().find(|p| p.id == page_id) else {
            return false;
        };
        let (mut cmds, released) = crate::connectors::unglue_cmds(&ed.doc, page, &ids);
        if released.is_empty() {
            return false;
        }
        // The released connectors' routes refresh in the same undo
        // step (FixedRoute skips), and anything glued TO them
        // re-attaches — the same tail every release path carries.
        cmds.push(Box::new(crate::cmd::RerouteConnectors::new_including(
            page_id,
            released.clone(),
        )));
        if let Some(r) = self.reroute_cmd(page_id, &released) {
            cmds.push(r);
        }
        let n = released.len();
        self.apply(Macro::new(
            crate::i18n::strs().msg_unglue_connectors(n),
            cmds,
        ));
        true
    }

    /// #113 (URS-1122): swap the page's width/height — portrait ↔
    /// landscape, one undoable step, then refit the view.
    fn toggle_page_orientation(&mut self) {
        let Some(page_id) = self.page_id() else {
            return;
        };
        let Some(ed) = self.ed.as_ref() else { return };
        let Some(page) = ed.doc.pages.iter().find(|p| p.id == page_id) else {
            return;
        };
        let Some(sheet) = page.sheet() else { return };
        let get = |n: &str| sheet.cell(n).and_then(|c| c.numeric());
        let Some((w, h)) = get("PageWidth").zip(get("PageHeight")) else {
            return;
        };
        self.apply_page_size((h, w));
    }

    /// #113 (URS-1122): write the page's size (page inches, portrait
    /// orientation of the pair as given) as one undo Macro, then
    /// refit the view to the new page.
    /// #251: the page's Show-Line-Jumps toggle (Page Setup ▸ Layout
    /// and Routing) — the PageSheet's `LineJumpCode` (the
    /// PageLayout-section cell): 1 = horizontal lines jump (Visio's
    /// authored default), 6 = "no line gets jumps" (the off switch).
    /// One undoable command.
    fn apply_line_jumps_shown(&mut self, show: bool) {
        let Some(page_id) = self.page_id() else {
            return;
        };
        self.apply(SetCell::new(
            Scope::PageSheet { page: page_id },
            "LineJumpCode",
            // Off writes 0 (visPLOJumpNone, [MS-Learn]) — 6 was
            // never in the enumeration (#218 review).
            vsdx::Cell::value(if show { "1" } else { "0" }),
        ));
        self.status = if show {
            crate::i18n::strs().msg_line_jumps_shown()
        } else {
            crate::i18n::strs().msg_line_jumps_hidden()
        };
    }

    /// #251: the page's jump-size multiplier (`LineJumpFactorX`).
    fn apply_line_jump_factor(&mut self, factor: f64) {
        let Some(page_id) = self.page_id() else {
            return;
        };
        self.apply(SetCell::new(
            Scope::PageSheet { page: page_id },
            "LineJumpFactorX",
            vsdx::Cell::value(format!("{factor:.6}")),
        ));
        // The factor steps in fractions (0.5 .. 3.0) — the i64 cast
        // truncated them to ×0/×1 in the status note (PR #269
        // review); FluentNumber carries the decimal through.
        self.status = crate::i18n::strs().msg_line_jump_factor_set(factor);
    }

    fn apply_page_size(&mut self, (w, h): (f64, f64)) {
        let Some(page_id) = self.page_id() else {
            return;
        };
        let scope = Scope::PageSheet { page: page_id };
        let cmds: Vec<Box<dyn dc_core::Command>> = vec![
            Box::new(SetCell::new(
                scope.clone(),
                "PageWidth",
                vsdx::Cell::value(format!("{w:.6}")),
            )),
            Box::new(SetCell::new(
                scope,
                "PageHeight",
                vsdx::Cell::value(format!("{h:.6}")),
            )),
        ];
        self.apply(Macro::new(crate::i18n::strs().msg_set_page_size(), cmds));
        self.view.fit_pending = true;
        self.layers.set_page(-1); // force re-key on the new geometry
        self.status = crate::i18n::strs().msg_page_size_set(
            self.view.unit.fmt(w),
            self.view.unit.fmt(h),
        );
    }

    /// #113 (URS-1113): probe the OS clipboard for an image on a
    /// worker thread; the result lands via `os_image_rx` over the
    /// next frames and inserts as a picture.
    fn paste_os_image(&mut self) {
        if self.os_image_rx.is_some() {
            return; // one probe at a time
        }
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let img: Option<OsImage> = std::panic::catch_unwind(arboard::Clipboard::new)
                .ok()
                .and_then(|cb| cb.ok())
                .and_then(|mut cb| {
                    cb.get_image().ok().map(|i| {
                        // RGBA bytes, row-major, top-down.
                        let w = i.width;
                        let h = i.height;
                        let mut bytes = Vec::with_capacity(w * h * 4);
                        for row in 0..h {
                            let line = &i.bytes[row * w * 4..(row + 1) * w * 4];
                            bytes.extend_from_slice(line);
                        }
                        (w as u32, h as u32, bytes)
                    })
                });
            let _ = tx.send(img);
        });
        self.os_image_rx = Some(rx);
        self.status = crate::i18n::strs().msg_checking_the_clipboard_for_a_pic();
    }

    /// #113: poll the OS image probe; a hit inserts as a picture.
    fn poll_os_image(&mut self) {
        let Some(rx) = self.os_image_rx.take() else {
            return;
        };
        match rx.try_recv() {
            Ok(Some((w, h, rgba))) => {
                // Encode PNG so the insert path's sniffer accepts it.
                if let Some(img) = image::RgbaImage::from_raw(w, h, rgba) {
                    let mut png = Vec::new();
                    if image::DynamicImage::ImageRgba8(img)
                        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
                        .is_ok()
                    {
                        self.insert_picture_bytes(&png);
                        return;
                    }
                }
                self.status = crate::i18n::strs().msg_clipboard_image_could_not_be_enc();
            }
            Ok(None) => {
                self.status = crate::i18n::strs().msg_no_picture_on_the_clipboard();
            }
            Err(mpsc::TryRecvError::Empty) => {
                // still probing: keep the receiver and retry next frame
                self.os_image_rx = Some(rx);
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                self.status = crate::i18n::strs().msg_clipboard_probe_failed();
            }
        }
    }

    /// #113 (URS-1158): the page's background assignment — the
    /// `BackPage` pages-index attribute names a page ID; `Background`
    /// marks the page itself as one. Corpus: `BackPage='4'` /
    /// `Background='1'` on the `<Page>` entries.
    fn back_page_of(&self, page_id: i64) -> Option<i64> {
        let ed = self.ed.as_ref()?;
        let p = ed.doc.pages.iter().find(|p| p.id == page_id)?;
        p.attrs
            .iter()
            .find(|(k, _)| k == "BackPage")
            .and_then(|(_, v)| v.parse::<i64>().ok())
    }

    fn is_background_page(&self, page_id: i64) -> bool {
        let Some(ed) = self.ed.as_ref() else {
            return false;
        };
        ed.doc
            .pages
            .iter()
            .find(|p| p.id == page_id)
            .is_some_and(|p| p.attrs.iter().any(|(k, v)| k == "Background" && v == "1"))
    }

    /// Assign/clear the current page's background (direct model
    /// write — a page-index flag, like Visio's non-undoable page
    /// setup properties).
    fn set_back_page(&mut self, bg: Option<i64>) {
        let Some(page_id) = self.page_id() else {
            return;
        };
        let Some(ed) = self.ed.as_mut() else { return };
        let Some(page) = ed.doc.pages.iter_mut().find(|p| p.id == page_id) else {
            return;
        };
        page.attrs.retain(|(k, _)| k != "BackPage");
        if let Some(bg) = bg {
            page.attrs.push(("BackPage".to_string(), bg.to_string()));
        }
        self.dirty = true;
        self.version += 1;
        self.status = match bg {
            Some(id) => {
                let name = ed
                    .doc
                    .pages
                    .iter()
                    .find(|p| p.id == id)
                    .map(|p| p.name.clone())
                    .unwrap_or_else(|| id.to_string());
                crate::i18n::strs().msg_background_page_name(name)
            }
            None => crate::i18n::strs().msg_background_cleared(),
        };
    }

    /// The Backgrounds dialog (#113, URS-1158): pick which page
    /// renders behind the current one (chain: a background with its
    /// own background stacks too), or none.
    fn backgrounds_window(&mut self, ctx: &egui::Context) {
        if !self.backgrounds_open {
            return;
        }
        let mut open = true;
        let mut pick: Option<Option<i64>> = None; // Some(None) = clear
        let Some(page_id) = self.page_id() else {
            return;
        };
        let Some(ed) = self.ed.as_ref() else { return };
        // Visio's gallery lists BACKGROUND pages (the #113 mark makes
        // one); a plain page never shows as a background choice.
        let entries: Vec<(i64, String)> = ed
            .doc
            .pages
            .iter()
            .filter(|p| p.id != page_id && self.is_background_page(p.id))
            .map(|p| (p.id, p.name.clone()))
            .collect();
        let current = self.back_page_of(page_id);
        let this_is_bg = self.is_background_page(page_id);
        egui::Window::new(crate::i18n::strs().msg_backgrounds())
            .open(&mut open)
            .resizable(false)
            .collapsible(false)
            .default_width(280.0)
            .show(ctx, |ui| {
                // #257 item 3: the dialog pass mirrors every window
                // (the Language-dialog exemplar — content right-aligns
                // while RTL).
                ui.with_layout(dialog_dir(), |ui| {
                    ui.label(crate::i18n::strs().msg_this_page_renders_behind());
                    if this_is_bg {
                        ui.weak(crate::i18n::strs().msg_this_page_is_itself_a_background());
                    }
                    ui.separator();
                    if ui
                        .radio(current.is_none(), crate::i18n::strs().msg_no_background())
                        .clicked()
                    {
                        pick = Some(None);
                    }
                    if entries.is_empty() {
                        ui.weak(
                            crate::i18n::strs().msg_no_background_pages_right_click(),
                        );
                    }
                    for (id, name) in &entries {
                        if ui
                            .radio(current == Some(*id), crate::i18n::strs().msg_name_page_id(name, id))
                            .clicked()
                        {
                            pick = Some(Some(*id));
                        }
                    }
                });
            });
        if let Some(bg) = pick {
            self.set_back_page(bg);
        }
        if !open {
            self.backgrounds_open = false;
        }
    }

    /// #113 (URS-1158): the background chain's underlay quads —
    /// deepest background first, each rasterized on demand at the
    /// current zoom (cache keyed page/version/ppi-bucket).
    fn build_bg_underlays(&mut self, ctx: &egui::Context) -> Vec<crate::layers::ShapeQuad> {
        let Some(page_id) = self.page_id() else {
            return Vec::new();
        };
        // Walk the chain (cycle-guarded, depth cap).
        let mut chain: Vec<i64> = Vec::new();
        let mut cur = self.back_page_of(page_id);
        while let Some(id) = cur {
            if chain.contains(&id) || chain.len() >= 8 {
                break;
            }
            chain.push(id);
            cur = self.ed.as_ref().and_then(|ed| {
                let p = ed.doc.pages.iter().find(|p| p.id == id)?;
                p.attrs
                    .iter()
                    .find(|(k, _)| k == "BackPage")
                    .and_then(|(_, v)| v.parse::<i64>().ok())
            });
        }
        let bucket = (self.view.px_per_in() * 4.0).round().max(4.0) as u32;
        let version = self.layer_base_version();
        // Drop stale entries — page switch, version bump, zoom bucket
        // (CodeRabbit: keeping version-stale handles leaked a texture
        // per edit while a background was assigned).
        self.bg_underlays
            .retain(|(p, v, b, _)| chain.contains(p) && *b == bucket && *v == version);
        let mut out: Vec<crate::layers::ShapeQuad> = Vec::new();
        for id in chain.iter().rev() {
            let Some(ed) = self.ed.as_ref() else { break };
            let Some(bg) = ed.doc.pages.iter().find(|p| p.id == *id) else {
                continue;
            };
            let version = self.layer_base_version();
            let (w, h) = (
                bg.sheet()
                    .and_then(|s| s.cell("PageWidth"))
                    .and_then(|c| c.numeric())
                    .unwrap_or(8.5),
                bg.sheet()
                    .and_then(|s| s.cell("PageHeight"))
                    .and_then(|c| c.numeric())
                    .unwrap_or(11.0),
            );
            if let Some(hh) = self
                .bg_underlays
                .iter()
                .find(|(p, v, b, _)| *p == *id && *v == version && *b == bucket)
                .map(|(_, _, _, hh)| hh)
            {
                out.push(crate::layers::ShapeQuad {
                    id: -1000 - id,
                    tex: hh.id(),
                    bbox: (0.0, 0.0, w, h),
                });
                continue;
            }
            // Rasterize this background page's own content.
            let Ok(svg) = dc::svg::render_page(&ed.doc, bg) else {
                continue;
            };
            let Ok(tree) = crate::svgimg::parse_svg(&svg, crate::perf::Kind::Thumb) else {
                continue;
            };
            let Ok(img) = crate::svgimg::render_tree(
                &tree,
                w as f32 * self.view.px_per_in(),
                crate::perf::Kind::Thumb,
            ) else {
                continue;
            };
            let tex = ctx.load_texture(format!("dc-bg-{id}"), img, egui::TextureOptions::default());
            out.push(crate::layers::ShapeQuad {
                id: -1000 - id,
                tex: tex.id(),
                bbox: (0.0, 0.0, w, h),
            });
            self.bg_underlays.push((*id, version, bucket, tex));
        }
        out
    }

    /// The layer store's base version — the underlay invalidation key.
    fn layer_base_version(&self) -> u64 {
        self.version
    }

    /// #113 (URS-1012): open the picture picker (PNG/JPEG).
    fn open_picture_dialog(&mut self) {
        self.begin_dialog(PendingKind::OpenPicture);
    }

    /// #113 (URS-1012): a picked picture file becomes a Foreign shape
    /// centered on the page at natural size (pixels/96 in, capped at
    /// 8 in per side) — the media part, the page rels entry, and the
    /// content-type default all ride `extra_media` to the writer.
    fn insert_picture_file(&mut self, path: &Path) {
        match std::fs::read(path) {
            Ok(bytes) => self.insert_picture_bytes(&bytes),
            Err(e) => {
                self.status = crate::i18n::strs()
                    .msg_could_not_read_path(path.display().to_string(), e.to_string())
            }
        }
    }

    /// The picture-insertion core (also the kittest path: bytes in,
    /// no native dialog).
    fn insert_picture_bytes(&mut self, bytes: &[u8]) {
        let Some(page_id) = self.page_id() else {
            return;
        };
        let geom = self.page_geom();
        let Some(ed) = self.ed.as_mut() else { return };
        // Format sniff: PNG/JPEG magic.
        let ext = if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
            "png"
        } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
            "jpeg"
        } else {
            self.status = crate::i18n::strs().msg_only_png_and_jpeg_pictures_are_s();
            return;
        };
        let compression = if ext == "png" { "PNG" } else { "JPG" };
        let Ok(img) = image::load_from_memory(bytes) else {
            self.status = crate::i18n::strs().msg_the_picture_could_not_be_decoded();
            return;
        };
        // Natural size, capped.
        let (mw, mh) = (img.width() as f64, img.height() as f64);
        let mut w = mw / 96.0;
        let mut h = mh / 96.0;
        let cap = 8.0_f64;
        if w.max(h) > cap {
            let k = cap / w.max(h);
            w *= k;
            h *= k;
        }
        let (w, h) = (w.max(0.1), h.max(0.1));
        let id = dc_core::commands::next_shape_id(&ed.doc, page_id);
        let mut sh = vsdx::model::Shape {
            id,
            type_: Some("Foreign".into()),
            name_u: Some(format!("Picture.{id}")),
            ..vsdx::model::Shape::default()
        };
        let mut set = |n: &str, v: f64| {
            sh.sheet
                .set_cell(n, vsdx::model::Cell::value(format!("{v:.6}")));
        };
        set("PinX", geom.w_in / 2.0);
        set("PinY", geom.h_in / 2.0);
        set("Width", w);
        set("Height", h);
        set("LocPinX", w / 2.0);
        set("LocPinY", h / 2.0);
        set("Angle", 0.0);
        set("FlipX", 0.0);
        set("FlipY", 0.0);
        set("ResizeMode", 0.0);
        // The image fills the shape box.
        let mut geo = vsdx::model::Section {
            name: "Geometry".into(),
            ix: 0,
            ..vsdx::model::Section::default()
        };
        for (n, v) in [("NoFill", "1"), ("NoLine", "0"), ("NoShow", "0")] {
            geo.cells.push((n.to_string(), vsdx::model::Cell::value(v)));
        }
        for (ix, t, x, y) in [
            (1i64, "MoveTo", 0.0, 0.0),
            (2, "LineTo", w, 0.0),
            (3, "LineTo", w, h),
            (4, "LineTo", 0.0, h),
            (5, "LineTo", 0.0, 0.0),
        ] {
            geo.rows.push(vsdx::model::Row {
                t: Some(t.into()),
                ix,
                cells: vec![
                    ("X".to_string(), vsdx::model::Cell::value(format!("{x:.6}"))),
                    ("Y".to_string(), vsdx::model::Cell::value(format!("{y:.6}"))),
                ],
                ..vsdx::model::Row::default()
            });
        }
        sh.sheet
            .children
            .push(vsdx::model::SheetChild::Section(geo));
        // Media part + rel id: next free across the package and
        // already-added media; the rid must not collide with the
        // page part's existing rels.
        let page = ed.doc.pages.iter().find(|p| p.id == page_id).expect("page");
        let part = page.part.clone();
        let n = ed
            .doc
            .extra_media
            .iter()
            .map(|m| m.part.clone())
            .chain(
                ed.doc
                    .package
                    .as_ref()
                    .map(|p| {
                        p.part_names()
                            .iter()
                            .map(|s| s.to_string())
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default(),
            )
            .filter_map(|name| {
                name.rsplit('/')
                    .next()
                    .and_then(|f| f.strip_prefix("image"))
                    .and_then(|rest| rest.split('.').next())
                    .and_then(|n| n.parse::<u32>().ok())
            })
            .max()
            .unwrap_or(0)
            + 1;
        let media_part = format!("visio/media/image{n}.{ext}");
        let rid_n = ed
            .doc
            .package
            .as_ref()
            .map(|p| p.relationships(&part))
            .unwrap_or_default()
            .iter()
            .filter_map(|(id, _, _)| id.strip_prefix("rId").and_then(|v| v.parse::<u32>().ok()))
            .chain(
                // Pictures already inserted on this page own rids the
                // package does not know (CodeRabbit: the second
                // picture would collide and render the first's bytes).
                ed.doc
                    .extra_media
                    .iter()
                    .filter(|m| m.page == page_id)
                    .filter_map(|m| {
                        m.rid
                            .strip_prefix("rId")
                            .and_then(|v| v.parse::<u32>().ok())
                    }),
            )
            .max()
            .unwrap_or(0)
            + 1;
        let rid = format!("rId{rid_n}");
        sh.raw_children.push(vsdx::model::RawEl {
            name: "ForeignData".into(),
            attrs: vec![
                ("Type".to_string(), "Bitmap".into()),
                ("ForeignType".to_string(), "Bitmap".into()),
                ("CompressionType".to_string(), compression.into()),
                ("r:id".to_string(), rid.clone()),
            ],
            inner: String::new(),
        });
        let _ = &mut ed.doc;
        // Own the page borrow scope.
        let doc = &mut ed.doc;
        doc.extra_media.push(vsdx::model::MediaRef {
            part: media_part,
            bytes: bytes.to_vec(),
            page: page_id,
            rid,
            ext: ext.to_string(),
        });
        let shape = sh;
        self.apply(dc_core::AddShape {
            page: page_id,
            parent: None,
            shape,
        });
        self.set_selection(Some((page_id, id)));
        self.sel_set = vec![(page_id, id)];
        self.status = crate::i18n::strs().msg_picture_inserted(
            mw as i64,
            mh as i64,
            w,
            h,
        );
    }

    /// #113 (URS-1200): render the selection as a PNG at `factor`
    /// (1 screen, 2 high, 4 print): the full-page SVG with every
    /// non-selected shape hidden, rasterized, then cropped to the
    /// selection's bounds (the page-SVG viewBox is always the whole
    /// page). The page raster is capped at 4096 px per side.
    fn picture_png(&self, factor: u32) -> Option<Vec<u8>> {
        let ed = self.ed.as_ref()?;
        let page_id = self.page_id()?;
        let page = ed.doc.pages.iter().find(|p| p.id == page_id)?;
        let sel: Vec<i64> = self
            .sel_set
            .iter()
            .filter(|(p, _)| *p == page_id)
            .map(|(_, s)| *s)
            .collect();
        if sel.is_empty() {
            return None;
        }
        // Selection bounds (page inches, y-up → y-down for the crop):
        // union of the rects' corner pairs.
        let geom = self.page_geom();
        let mut b: Option<(f64, f64, f64, f64)> = None;
        for sid in &sel {
            let sh = vsdx::model::find_shape_by_id(&page.shapes, *sid)?;
            let r = self.shape_rect(&ed.doc, page_id, sh)?;
            let corners = (r.0, r.1, r.0 + r.2, r.1 + r.3);
            b = Some(match b {
                None => corners,
                Some((x0, y0, x1, y1)) => (
                    x0.min(corners.0),
                    y0.min(corners.1),
                    x1.max(corners.2),
                    y1.max(corners.3),
                ),
            });
        }
        let (bx0, by0, bx1, by1) = b?;
        // Hide every other shape.
        let hidden: Vec<i64> = page
            .shapes
            .iter()
            .map(|s| s.id)
            .filter(|id| !sel.contains(id))
            .collect();
        let svg = dc::svg::render_page_with(&ed.doc, page, &hidden).ok()?;
        let tree = crate::svgimg::parse_svg(&svg, crate::perf::Kind::Thumb).ok()?;
        // Raster the whole page, capped, then crop the selection.
        let max_side = 4096.0;
        let mut f = factor as f64;
        if geom.w_in * 96.0 * f > max_side || geom.h_in * 96.0 * f > max_side {
            f = (max_side / (96.0 * geom.w_in.max(geom.h_in))).min(f);
        }
        let f = f as f32;
        let img = crate::svgimg::render_tree(
            &tree,
            geom.w_in as f32 * 96.0 * f,
            crate::perf::Kind::Thumb,
        )
        .ok()?;
        let (iw, ih) = (img.width() as f64, img.height() as f64);
        let sx = iw / geom.w_in;
        let sy = ih / geom.h_in;
        let px0 = ((bx0) * sx).floor().max(0.0) as u32;
        let py0 = ((geom.h_in - by1) * sy).floor().max(0.0) as u32; // y-down
        let px1 = ((bx1) * sx).ceil().min(iw) as u32;
        let py1 = ((geom.h_in - by0) * sy).ceil().min(ih) as u32;
        let (cw, ch) = (
            px1.saturating_sub(px0).max(1),
            py1.saturating_sub(py0).max(1),
        );
        let mut out = image::RgbaImage::new(cw, ch);
        let (iw_us, ih_us) = (img.width(), img.height());
        for row in 0..ch {
            for col in 0..cw {
                let sx = ((px0 + col) as usize).min(iw_us - 1);
                let sy = ((py0 + row) as usize).min(ih_us - 1);
                let px = img[(sx, sy)]; // egui::Color32 (sRGB, premult)
                let [r, g, b, a] = px.to_srgba_unmultiplied();
                out.put_pixel(col, row, image::Rgba([r, g, b, a]));
            }
        }
        let mut bytes = Vec::new();
        image::DynamicImage::ImageRgba8(out)
            .write_to(
                &mut std::io::Cursor::new(&mut bytes),
                image::ImageFormat::Png,
            )
            .ok()?;
        Some(bytes)
    }

    /// The Copy as Picture dialog (#113, URS-1200).
    /// #156: the Container Styles gallery (Design ▸ Containers…) —
    /// the 14 SDCONT styles; a click wraps the selection (Visio's
    /// gallery-with-a-selection flow) and selects the new container.
    fn container_gallery_window(&mut self, ctx: &egui::Context) {
        if !self.contgal_open {
            return;
        }
        let mut open = true;
        let mut pick: Option<&'static str> = None;
        egui::Window::new(crate::i18n::strs().msg_container_styles())
            .open(&mut open)
            .resizable(false)
            .collapsible(false)
            .default_width(260.0)
            .show(ctx, |ui| {
                ui.with_layout(dialog_dir(), |ui| {
                    ui.weak(crate::i18n::strs().msg_contgal_prompt());
                    ui.add_space(4.0);
                    egui::Grid::new("dc-contgal")
                        .num_columns(2)
                        .spacing(egui::vec2(8.0, 4.0))
                        .show(ui, |ui| {
                            // #257: an RTL grid fills its rows right-to-left.
                            let styles: Vec<&str> = if crate::i18n::rtl() {
                                CONTAINER_STYLES
                                    .chunks(2)
                                    .flat_map(|c| c.iter().rev().copied())
                                    .collect()
                            } else {
                                CONTAINER_STYLES.to_vec()
                            };
                            for name in styles {
                                if ui.button(name).clicked() {
                                    pick = Some(name);
                                }
                            }
                        });
                });
            });
        if let Some(name) = pick {
            self.contgal_open = false;
            self.apply_container_style(name);
        }
        if !open {
            self.contgal_open = false;
        }
    }

    fn copy_picture_window(&mut self, ctx: &egui::Context) {
        if !self.copypic_open {
            return;
        }
        let mut open = true;
        let mut copy = false;
        egui::Window::new(crate::i18n::strs().msg_copy_as_picture())
            .open(&mut open)
            .resizable(false)
            .collapsible(false)
            .default_width(280.0)
            .show(ctx, |ui| {
                ui.with_layout(dialog_dir(), |ui| {
                    ui.strong(crate::i18n::strs().msg_resolution());
                    if ui.radio(self.copypic_svg, crate::i18n::strs().msg_vector_svg()).clicked() {
                        self.copypic_svg = true;
                    }
                    for (f, label) in [
                        (1u32, crate::i18n::strs().msg_screen_96_dpi()),
                        (2, crate::i18n::strs().msg_high_192_dpi()),
                        (4, crate::i18n::strs().msg_print_384_dpi()),
                    ] {
                        if ui
                            .radio(!self.copypic_svg && self.copypic_factor == f, label)
                            .clicked()
                        {
                            self.copypic_factor = f;
                            self.copypic_svg = false;
                        }
                    }
                    ui.add_space(6.0);
                    if ui.button(crate::i18n::strs().msg_copy_picture()).clicked() {
                        copy = true;
                    }
                    ui.weak(if self.copypic_svg {
                        crate::i18n::strs().msg_the_svg_markup_lands_on_the_clip()
                    } else {
                        crate::i18n::strs().msg_the_picture_lands_on_the_clipboa()
                    });
                });
            });
        if copy && self.copypic_svg {
            match self.picture_svg() {
                Some(svg) => {
                    let n = svg.len();
                    self.last_picture = Some(svg.clone().into_bytes());
                    ctx.copy_text(svg);
                    self.status = crate::i18n::strs().msg_svg_copied_n_bytes_vector_paste(n);
                }
                None => self.status = crate::i18n::strs().msg_nothing_selected_to_copy(),
            }
        }
        if copy && !self.copypic_svg {
            match self.picture_png(self.copypic_factor) {
                Some(bytes) => {
                    self.last_picture = Some(bytes.clone());
                    match image::load_from_memory(&bytes) {
                        Ok(dynimg) => {
                            let rgba = dynimg.to_rgba8();
                            let (w, h) = (rgba.width(), rgba.height());
                            let ci = egui::ColorImage::from_rgba_unmultiplied(
                                [w as usize, h as usize],
                                &rgba.into_raw(),
                            );
                            {
                                ctx.copy_image(ci);
                                self.status = crate::i18n::strs().msg_picture_copied(
                                    w as i64,
                                    h as i64,
                                    self.copypic_factor as i64,
                                );
                            }
                        }
                        Err(e) => self.status = crate::i18n::strs().msg_copy_as_picture_failed_e(e.to_string()),
                    }
                }
                None => self.status = crate::i18n::strs().msg_nothing_selected_to_copy(),
            }
        }
        if !open {
            self.copypic_open = false;
        }
    }

    /// #113 remainder: the vector copy — the page rendered with every
    /// non-selected shape hidden, the viewBox retargeted to the
    /// selection bounds, as SVG markup for the text clipboard (the
    /// interchange egui can carry; raster flavors stay on the image
    /// clipboard).
    fn picture_svg(&self) -> Option<String> {
        let ed = self.ed.as_ref()?;
        let page_id = self.page_id()?;
        let page = ed.doc.pages.iter().find(|p| p.id == page_id)?;
        let sel: Vec<i64> = self
            .sel_set
            .iter()
            .filter(|(p, _)| *p == page_id)
            .map(|(_, s)| *s)
            .collect();
        if sel.is_empty() {
            return None;
        }
        let geom = self.page_geom();
        let mut b: Option<(f64, f64, f64, f64)> = None;
        for sid in &sel {
            let sh = vsdx::model::find_shape_by_id(&page.shapes, *sid)?;
            let r = self.shape_rect(&ed.doc, page_id, sh)?;
            let corners = (r.0, r.1, r.0 + r.2, r.1 + r.3);
            b = Some(match b {
                None => corners,
                Some((x0, y0, x1, y1)) => (
                    x0.min(corners.0),
                    y0.min(corners.1),
                    x1.max(corners.2),
                    y1.max(corners.3),
                ),
            });
        }
        let (bx0, by0, bx1, by1) = b?;
        let hidden: Vec<i64> = page
            .shapes
            .iter()
            .map(|s| s.id)
            .filter(|id| !sel.contains(id))
            .collect();
        let svg = dc::svg::render_page_with(&ed.doc, page, &hidden).ok()?;
        // Rebuild the ROOT element around the selection's box (page
        // y-up → the svg's y-down frame), padding a hair to keep
        // strokes intact. The root is written fresh — token-stripping
        // the emitted one would split its space-bearing viewBox value
        // and leave orphan tokens (invalid XML; CodeRabbit).
        let pad = 0.06;
        let (vx, vy, vw, vh) = (
            (bx0 - pad).max(0.0),
            (geom.h_in - by1 - pad).max(0.0),
            (bx1 - bx0 + 2.0 * pad).min(geom.w_in),
            (by1 - by0 + 2.0 * pad).min(geom.h_in),
        );
        let root_end = svg.find('>').map(|i| i + 1)?;
        let mut out = format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"{vx:.4} {vy:.4} {vw:.4} {vh:.4}\" width=\"{pw:.0}\" height=\"{ph:.0}\">",
            pw = vw * 96.0,
            ph = vh * 96.0,
        );
        out.push_str(&svg[root_end..]);
        Some(out)
    }

    /// The Page Setup dialog (#113, URS-1122): ISO/ANSI presets plus
    /// custom dimensions (display units) and orientation.
    fn page_setup_window(&mut self, ctx: &egui::Context) {
        if !self.pagesetup_open {
            return;
        }
        /// (name, portrait w, h) in inches — Visio's Printer paper
        /// preset table, metric and ANSI families.
        const SIZES: &[(&str, f64, f64)] = &[
            ("A5", 5.83, 8.27),
            ("A4", 8.27, 11.69),
            ("A3", 11.69, 16.54),
            ("A2", 16.54, 23.39),
            ("A1", 23.39, 33.11),
            ("A0", 33.11, 46.81),
            ("Letter (ANSI A)", 8.5, 11.0),
            ("Legal", 8.5, 14.0),
            ("Tabloid (ANSI B)", 11.0, 17.0),
            ("ANSI C", 17.0, 22.0),
            ("ANSI D", 22.0, 34.0),
            ("ANSI E", 34.0, 44.0),
        ];
        let unit = self.view.unit;
        let geom = self.page_geom();
        // Current dims in display units.
        let (cw, ch) = (unit.from_in(geom.w_in), unit.from_in(geom.h_in));
        let mut open = true;
        let mut apply: Option<(f64, f64)> = None; // portrait pair, inches
        let mut custom: Option<(f64, f64)> = None;
        egui::Window::new(crate::i18n::strs().msg_page_size())
            .open(&mut open)
            .resizable(false)
            .collapsible(false)
            .default_width(300.0)
            .show(ctx, |ui| {
                // #257 item 3: the dialog pass mirrors the window.
                let mut __rtl_body = |ui: &mut Ui| {
                ui.label(crate::i18n::strs().msg_current());
                ui.label(format!(
                    "{:.1} × {:.1} {}",
                    cw.max(ch),
                    cw.min(ch),
                    unit.suffix()
                ));
                ui.separator();
                ui.strong(crate::i18n::strs().msg_preset_sizes());
                let mut picked: Option<(f64, f64)> = None;
                egui::Grid::new("dc-sizes")
                    .num_columns(3)
                    .spacing(egui::vec2(8.0, 4.0))
                    .show(ui, |ui| {
                        // #257: an RTL grid fills its rows right-to-left.
                        let rows: Vec<Vec<(&str, f64, f64)>> = if crate::i18n::rtl() {
                            SIZES
                                .chunks(3)
                                .map(|c| {
                                    let mut v: Vec<_> = c.to_vec();
                                    v.reverse();
                                    v
                                })
                                .collect()
                        } else {
                            SIZES.chunks(3).map(|c| c.to_vec()).collect()
                        };
                        for chunk in rows {
                            for (name, w, h) in chunk {
                                if ui.button(name).clicked() {
                                    picked = Some((w, h));
                                }
                            }
                            ui.end_row();
                        }
                    });
                if let Some(p) = picked {
                    apply = Some(p);
                }
                ui.separator();
                ui.strong(crate::i18n::strs().msg_custom());
                let mut wd = cw;
                let mut ht = ch;
                // The range is ~10 mm .. 2 m, expressed in display
                // units so inches never clamps at 10 in.
                let (lo, hi) = (unit.from_in(10.0 / 25.4), unit.from_in(2000.0 / 25.4));
                // #257: mirrored emission under RTL — ui::horizontal
                // keeps the wrap-aware measure (a with_layout flip here
                // reserves the parent's full cross space and balloons
                // the window).
                let wd_label = crate::i18n::strs().msg_width();
                let ht_label = crate::i18n::strs().msg_height();
                let width_first = !crate::i18n::rtl();
                ui.horizontal(|ui| {
                    let (first_label, second_label) = if width_first {
                        (wd_label.as_str(), ht_label.as_str())
                    } else {
                        (ht_label.as_str(), wd_label.as_str())
                    };
                    let mut first_val = if width_first { wd } else { ht };
                    let mut second_val = if width_first { ht } else { wd };
                    ui.label(first_label);
                    ui.add(
                        egui::DragValue::new(&mut first_val)
                            .range(lo..=hi)
                            .suffix(unit.suffix()),
                    );
                    ui.label(second_label);
                    ui.add(
                        egui::DragValue::new(&mut second_val)
                            .range(lo..=hi)
                            .suffix(unit.suffix()),
                    );
                    // The RTL dialog swaps the FIELDS (height first);
                    // the write-back must swap with them — the fixed
                    // order traded a Persian page's W and H on every
                    // Apply (PR #269 review).
                    wd = if width_first { first_val } else { second_val };
                    ht = if width_first { second_val } else { first_val };
                });
                if ui.button(crate::i18n::strs().msg_apply_custom_size()).clicked() && wd > 0.0 && ht > 0.0 {
                    custom = Some((unit.to_in(wd), unit.to_in(ht)));
                }
                ui.separator();
                ui.strong(crate::i18n::strs().msg_orientation());
                // #257: mirrored emission under RTL (horizontal keeps
                // the wrap-aware measure).
                let rtl_ori = crate::i18n::rtl();
                let (mut p_clicked, mut l_clicked) = (false, false);
                ui.horizontal(|ui| {
                    if rtl_ori {
                        if ui
                            .selectable_label(
                                geom.w_in > geom.h_in,
                                crate::i18n::strs().msg_landscape(),
                            )
                            .clicked()
                            && geom.w_in <= geom.h_in
                        {
                            l_clicked = true;
                        }
                        if ui
                            .selectable_label(
                                geom.w_in <= geom.h_in,
                                crate::i18n::strs().msg_portrait(),
                            )
                            .clicked()
                            && geom.w_in > geom.h_in
                        {
                            p_clicked = true;
                        }
                    } else {
                        if ui
                            .selectable_label(
                                geom.w_in <= geom.h_in,
                                crate::i18n::strs().msg_portrait(),
                            )
                            .clicked()
                            && geom.w_in > geom.h_in
                        {
                            p_clicked = true;
                        }
                        if ui
                            .selectable_label(
                                geom.w_in > geom.h_in,
                                crate::i18n::strs().msg_landscape(),
                            )
                            .clicked()
                            && geom.w_in <= geom.h_in
                        {
                            l_clicked = true;
                        }
                    }
                });
                if p_clicked {
                    apply = Some((geom.w_in.min(geom.h_in), geom.w_in.max(geom.h_in)));
                }
                if l_clicked {
                    apply = Some((geom.h_in.max(geom.w_in), geom.h_in.min(geom.w_in)));
                }
                // Layout and Routing (#251): the line-jump display
                // switch and jump size — Visio's Page Setup group,
                // persisted on the PageSheet.
                ui.separator();
                ui.strong(crate::i18n::strs().msg_line_jumps());
                let mut show_toggle = self
                    .ed
                    .as_ref()
                    .map(|ed| {
                        ed.doc.pages
                            [self.page_idx.min(ed.doc.pages.len() - 1)]
                            .line_jumps_shown()
                    })
                    .unwrap_or(true);
                if ui.checkbox(&mut show_toggle, crate::i18n::strs().msg_show_line_jumps()).changed() {
                    self.apply_line_jumps_shown(show_toggle);
                }
                let doc_factor = self
                    .ed
                    .as_ref()
                    .map(|ed| {
                        ed.doc.pages
                            [self.page_idx.min(ed.doc.pages.len() - 1)]
                            .line_jump_factor()
                    })
                    .unwrap_or(1.0) as f32;
                let mut jump_factor = self.line_jump_draft.unwrap_or(doc_factor);
                // #257: mirrored emission under RTL (horizontal).
                let jump_label = crate::i18n::strs().msg_jump_size();
                ui.horizontal(|ui| {
                    let mut factor_row = |ui: &mut egui::Ui| {
                        let r = ui.add(
                            egui::DragValue::new(&mut jump_factor)
                                .range(0.5..=3.0)
                                .speed(0.05)
                                .suffix("×"),
                        );
                        // Draft while dragging; commit ONCE at drag end
                        // (or immediately for typed input) — the value
                        // reloads from the document each frame, so the
                        // draft is what carries the drag (#218 review).
                        if r.dragged() {
                            self.line_jump_draft = Some(jump_factor);
                        }
                        if r.drag_stopped() || (r.changed() && !r.dragged()) {
                            self.line_jump_draft = None;
                            if (jump_factor - doc_factor).abs() > 1e-6 {
                                self.apply_line_jump_factor(jump_factor as f64);
                            }
                        }
                    };
                    if !crate::i18n::rtl() {
                        ui.label(jump_label);
                        factor_row(ui);
                    } else {
                        factor_row(ui);
                        ui.label(jump_label);
                    }
                });
                };
                if crate::i18n::rtl() {
                    ui.with_layout(egui::Layout::top_down(egui::Align::RIGHT), |ui| {
                        __rtl_body(ui)
                    });
                } else {
                    __rtl_body(ui);
                }
            });
        if let Some((w, h)) = apply.or(custom) {
            self.apply_page_size((w, h));
        }
        if !open {
            self.pagesetup_open = false;
        }
    }

    /// #110 (URS-1160): select every shape on the page.
    fn select_all(&mut self) {
        let Some(page_id) = self.page_id() else {
            return;
        };
        let Some(ed) = self.ed() else { return };
        let page = &ed.doc.pages[self.page_idx];
        let ids: Vec<(i64, i64)> = page.shapes.iter().map(|s| (page_id, s.id)).collect();
        if ids.is_empty() {
            return;
        }
        self.sel = Some(ids[0]);
        self.sel_set = ids;
        self.status = crate::i18n::strs().msg_selected_all_shapes();
    }

    /// #110 (URS-1053): arm the Format Painter on the primary
    /// selection — the next shape click receives its resolved
    /// formatting (single-shot, Visio's click mode).
    fn arm_format_painter(&mut self, sticky: bool) {
        // A second click while armed disarms (Visio's toggle).
        if self.painter_armed.is_some() && !sticky {
            self.painter_armed = None;
            self.painter_sticky = false;
            self.status = crate::i18n::strs().msg_format_painter_cancelled();
            return;
        }
        let Some((_, sid)) = self.sel else {
            self.status = crate::i18n::strs().msg_select_a_shape_to_copy_formattin();
            return;
        };
        self.painter_armed = Some(sid);
        self.painter_sticky = sticky;
        self.status = if sticky {
            crate::i18n::strs().msg_format_painter_click_shapes_to_a(sid)
        } else {
            crate::i18n::strs().msg_format_painter_click_a_shape_to(sid)
        };
    }

    /// The painter's application: copy the resolved FORMAT_CELLS from
    /// the source onto the target as one undo Macro.
    fn apply_format_painter(&mut self, from: i64, to: i64) {
        let Some(page_id) = self.page_id() else {
            return;
        };
        let Some(ed) = self.ed.as_ref() else { return };
        let page = &ed.doc.pages[self.page_idx];
        let (Some(src), Some(_dst)) = (
            vsdx::model::find_shape_by_id(&page.shapes, from),
            vsdx::model::find_shape_by_id(&page.shapes, to),
        ) else {
            return;
        };
        let Ok(src_chain) = shape_chain(&ed.doc, page_id, from) else {
            return;
        };
        let scope = Scope::Shape {
            page: page_id,
            shape: to,
        };
        let mut cmds: Vec<Box<dyn dc_core::Command>> = Vec::new();
        for name in crate::align::FORMAT_CELLS {
            if let Some(c) = ed.doc.resolve_cell(src, &src_chain, name) {
                if let Some(v) = c.numeric() {
                    cmds.push(Box::new(SetCell::new(
                        scope.clone(),
                        *name,
                        vsdx::Cell::value(format!("{v:.9}")),
                    )));
                }
            }
        }
        let n = cmds.len();
        self.apply(Macro::new(
            crate::i18n::strs().msg_paint_formatting_onto_shape_to(to),
            cmds,
        ));
        // #110: the sticky (double-clicked) painter stays armed for
        // the next click; the single-shot one is spent.
        if self.painter_sticky {
            self.status = crate::i18n::strs().msg_painter_cells_sticky(n, to);
        } else {
            self.status = crate::i18n::strs().msg_painter_cells(n, to);
            self.painter_armed = None;
        }
    }

    /// The page's guides as UI info (#113), resolved through the
    /// inheritance chain.
    fn page_guides(&self) -> Vec<crate::guides::GuideInfo> {
        let Some(ed) = self.ed() else {
            return Vec::new();
        };
        let Some(page_id) = self.page_id() else {
            return Vec::new();
        };
        let Some(page) = ed.doc.pages.iter().find(|p| p.id == page_id) else {
            return Vec::new();
        };
        page.shapes
            .iter()
            .filter(|s| crate::guides::is_guide(s))
            .filter_map(|s| {
                let Ok(chain) = shape_chain(&ed.doc, page_id, s.id) else {
                    return None;
                };
                let get = |n: &str| ed.doc.resolve_cell(s, &chain, n).and_then(|c| c.numeric());
                crate::guides::guide_info(s.id, s, get)
            })
            .collect()
    }

    /// The guide list as engine targets (#113 → #109's Guide seam),
    /// minus `excluded` (the dragged shape's set — a guide being
    /// dragged never targets itself).
    fn snap_guides(&self, excluded: &[i64]) -> Vec<crate::snap::Guide> {
        self.page_guides()
            .iter()
            .filter(|g| !excluded.contains(&g.id))
            .map(|g| match g.kind {
                crate::guides::GuideKind::Vertical => crate::snap::Guide::Line {
                    id: g.id,
                    horizontal: false,
                    at: g.x,
                },
                crate::guides::GuideKind::Horizontal => crate::snap::Guide::Line {
                    id: g.id,
                    horizontal: true,
                    at: g.y,
                },
                crate::guides::GuideKind::Point => crate::snap::Guide::Point {
                    id: g.id,
                    x: g.x,
                    y: g.y,
                },
            })
            .collect()
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
            // The drag is taken out for the frame so the snap pass can
            // borrow the app (#109); it goes straight back.
            if let (Some(p), Some(mut d)) = (pointer, self.drag.take()) {
                let x_in = (p.x - self.view.pan.x) as f64 / self.view.px_per_in() as f64;
                let y_up =
                    geom.h_in - (p.y - self.view.pan.y) as f64 / self.view.px_per_in() as f64;
                let new_pin = (x_in + d.grab_off.0, y_up + d.grab_off.1);
                let new_pin = self.apply_drag_snap(&d, new_pin);
                if (new_pin.0 - d.last_pin.0).abs() * self.view.px_per_in() as f64 > 0.5
                    || (new_pin.1 - d.last_pin.1).abs() * self.view.px_per_in() as f64 > 0.5
                {
                    d.moved = true;
                }
                d.last_pin = new_pin;
                self.drag = Some(d);
            }
        } else if let Some(d) = self.drag.take() {
            self.snap_live.clear();
            if d.moved {
                if let Some(page_id) = self.page_id() {
                    // Moving a multi-selection moves every member by the
                    // same delta, as one undo step. The group was
                    // captured at press time (members of the selection
                    // the press actually landed on). Each member moves
                    // through `move_cmd_for`: 1D members translate their
                    // endpoints too, not just the pin.
                    // A guide drag (#113) is its own commit: the
                    // guide moves and its glued dependents follow.
                    let dragging_guide = self.ed.as_ref().is_some_and(|ed| {
                        let page =
                            &ed.doc.pages[self.page_idx.min(ed.doc.pages.len().saturating_sub(1))];
                        vsdx::model::find_shape_by_id(&page.shapes, d.shape)
                            .is_some_and(crate::guides::is_guide)
                    });
                    if dragging_guide {
                        self.apply_guide_move(page_id, d.shape, d.last_pin);
                    } else if d.group.is_empty() {
                        // A container drag carries its members (Visio,
                        // #27): one undo step.
                        let member_ids = self.container_member_ids(page_id, d.shape);
                        if member_ids.is_empty() {
                            let label = crate::i18n::strs().msg_undo_move_shape(d.shape);
                            let cmd = self.move_cmd_for(page_id, d.shape, d.last_pin);
                            // #108: glued connectors re-attach and
                            // reroute in the same undo step.
                            let reroute = self.reroute_cmd(page_id, &[d.shape]);
                            // #113: a shape dropped onto a guide with
                            // glue-to-guides on glues its pin(s) to it
                            // — Visio's `PinX = Sheet.N!PinX` formula
                            // with the snapped cached value. Only for
                            // plain 2D shapes (1D endpoint glue to
                            // guides rides with #108's connectors).
                            let glue = self.drag_glue.take().filter(|_| {
                                self.ed.as_ref().is_some_and(|ed| {
                                    let page = &ed.doc.pages[self.page_idx];
                                    vsdx::model::find_shape_by_id(&page.shapes, d.shape)
                                        .and_then(|sh| {
                                            let chain =
                                                shape_chain(&ed.doc, page_id, d.shape).ok()?;
                                            ed.doc
                                                .resolve_cell(sh, &chain, "BeginX")
                                                .and_then(|c| c.numeric())
                                        })
                                        .is_none()
                                })
                            });
                            match glue {
                                Some((guide_id, gx, gy)) => {
                                    // Guide glue ties the PIN to the
                                    // guide (Visio semantics: the pin
                                    // rides the line). An edge-snap
                                    // must not glue — the formula
                                    // would jump the shape half its
                                    // width at drop — so each axis
                                    // glues only when the snapped pin
                                    // coordinate already IS the
                                    // guide's.
                                    let g = self
                                        .page_guides()
                                        .iter()
                                        .find(|g| g.id == guide_id)
                                        .copied();
                                    let axis_target = |axis: &str| -> Option<f64> {
                                        let g = g?;
                                        match (axis, g.kind) {
                                            ("PinX", crate::guides::GuideKind::Vertical)
                                            | ("PinX", crate::guides::GuideKind::Point) => {
                                                Some(g.x)
                                            }
                                            ("PinY", crate::guides::GuideKind::Horizontal)
                                            | ("PinY", crate::guides::GuideKind::Point) => {
                                                Some(g.y)
                                            }
                                            _ => None,
                                        }
                                    };
                                    let mut glue_cmds: Vec<Box<dyn dc_core::Command>> = Vec::new();
                                    for (axis, want) in [("PinX", gx), ("PinY", gy)] {
                                        let Some(target) = axis_target(axis) else {
                                            continue;
                                        };
                                        let pin = if axis == "PinX" {
                                            d.last_pin.0
                                        } else {
                                            d.last_pin.1
                                        };
                                        if want && (pin - target).abs() < 1e-6 {
                                            glue_cmds.push(Box::new(SetCell::new(
                                                Scope::Shape {
                                                    page: page_id,
                                                    shape: d.shape,
                                                },
                                                axis,
                                                vsdx::Cell {
                                                    v: Some(format!("{target:.6}")),
                                                    f: Some(crate::guides::glue_formula(
                                                        guide_id, axis,
                                                    )),
                                                    ..vsdx::Cell::default()
                                                },
                                            )));
                                        }
                                    }
                                    glue_cmds.insert(0, cmd);
                                    if let Some(r) = reroute {
                                        glue_cmds.push(r);
                                    }
                                    self.apply(Macro::new(label, glue_cmds));
                                }
                                None => match reroute {
                                    Some(r) => {
                                        let mut all = vec![cmd];
                                        all.push(r);
                                        self.apply(Macro::new(label, all));
                                    }
                                    None => self.apply_boxed(&label, cmd),
                                },
                            }
                        } else {
                            let (dx, dy) =
                                (d.last_pin.0 - d.start_pin.0, d.last_pin.1 - d.start_pin.1);
                            let mut ids_for_reroute: Vec<i64> = vec![d.shape];
                            ids_for_reroute.extend(member_ids.iter().copied());
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
                            if let Some(r) = self.reroute_cmd(page_id, &ids_for_reroute) {
                                cmds.push(r);
                            }
                            self.apply(Macro::new(crate::i18n::strs().msg_move_container_and_members(), cmds));
                        }
                    } else {
                        let Some(ed) = self.ed() else { return };
                        let dx = d.last_pin.0 - d.start_pin.0;
                        let dy = d.last_pin.1 - d.start_pin.1;
                        let ids_for_reroute: Vec<i64> = std::iter::once(d.shape)
                            .chain(d.group.iter().copied())
                            .collect();
                        let mut cmds: Vec<Box<dyn dc_core::Command>> =
                            vec![self.move_cmd_for(page_id, d.shape, d.last_pin)];
                        for sid in &d.group {
                            let page = &ed.doc.pages[self.page_idx];
                            let Some(sh) = vsdx::model::find_shape_by_id(&page.shapes, *sid) else {
                                continue;
                            };
                            let Ok(chain) = shape_chain(&ed.doc, page_id, *sid) else {
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
                                    *sid,
                                    (target.0 + dx, target.1 + dy),
                                ));
                            }
                        }
                        if let Some(r) = self.reroute_cmd(page_id, &ids_for_reroute) {
                            cmds.push(r);
                        }
                        self.apply(Macro::new(crate::i18n::strs().msg_move_shapes(), cmds));
                    }
                    self.set_selection(Some((page_id, d.shape)));
                }
            }
        }
    }

    /// #95's UI guard: when `members` contains exactly one top-level
    /// Group (plus at least one other shape), grouping nests into that
    /// group — returns the parent id and the members' page-space bbox
    /// origin converted into the parent's local frame.
    fn nest_group_target(&self, members: &[i64]) -> Option<(i64, (f64, f64))> {
        if members.len() < 2 {
            return None;
        }
        let ed = self.ed()?;
        let page = ed.doc.pages.get(self.page_idx)?;
        let page_id = page.id;
        let mut groups = Vec::new();
        for &id in members {
            let s = page.shapes.iter().find(|s| s.id == id)?;
            if s.type_.as_deref() == Some("Group") {
                groups.push(id);
            }
        }
        if groups.len() != 1 {
            return None;
        }
        let parent = groups[0];
        // The members' page-space bbox origin (the group-to-be's pin
        // before conversion): union of the non-parent members' rects.
        let mut x0 = f64::MAX;
        let mut y0 = f64::MAX;
        for &id in members {
            if id == parent {
                continue;
            }
            let s = page.shapes.iter().find(|s| s.id == id)?;
            let chain = shape_chain(&ed.doc, page_id, id).ok()?;
            let get = |n: &str| ed.doc.resolve_cell(s, &chain, n).and_then(|c| c.numeric());
            let (px, py) = (get("PinX")?, get("PinY")?);
            let lx = get("LocPinX").unwrap_or(0.0);
            let ly = get("LocPinY").unwrap_or(0.0);
            x0 = x0.min(px - lx);
            y0 = y0.min(py - ly);
        }
        let page_h = Self::page_height(ed, page_id);
        // The conversion needs the PARENT as an ancestor — the affine
        // maps the chain's last entry's parent-local frame, so append
        // the new group's own slot (the parent twice).
        let mut chain = shape_chain(&ed.doc, page_id, parent).ok()?;
        chain.push(chain[0]);
        let local = dc::svg::page_point_to_parent_local(&ed.doc, &chain, page_h, (x0, y0));
        Some((parent, local))
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
        // The Connection Point tool owns clicks on shapes (#108):
        // near an existing point selects it (Delete removes); on the
        // body adds one at the click.
        if self.tool == Tool::ConnectionPoint && !ctrl {
            self.handle_conn_point_click(page_pos);
            return;
        }
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
        // The armed Format Painter consumes the click (#110): the hit
        // shape receives the source's resolved formatting. The sticky
        // (double-clicked) painter stays armed; empty-page or
        // on-the-source clicks cancel either way.
        if let Some(src) = self.painter_armed {
            if let Some(hit) = self.shape_hit(page_pos) {
                if hit != src {
                    self.apply_format_painter(src, hit);
                    if let Some(p) = self.page_id() {
                        self.set_selection(Some((p, hit)));
                    }
                    return;
                }
            }
            self.painter_armed = None;
            self.painter_sticky = false;
            self.status = crate::i18n::strs().msg_format_painter_cancelled();
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
                self.status = crate::i18n::strs().msg_selected_shape_shape_id(shape_id);
                // #156: selecting a container raises the contextual
                // Container Tools tab (Visio 2016).
                if self.sel_is_container() {
                    self.tab = Tab::ContainerTools;
                }
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
                let ed = self.ed.as_ref()?;
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
        // A guide has no text and no event to run (#113): selecting
        // it is the whole double-click.
        if let Some(ed) = self.ed() {
            let page = ed.doc.pages.iter().find(|p| p.id == page_id);
            if let Some(sh) = page.and_then(|p| vsdx::model::find_shape_by_id(&p.shapes, shape_id))
            {
                if crate::guides::is_guide(sh) {
                    return;
                }
            }
        }
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
                self.status = crate::i18n::strs().msg_editing_text_of_shape_shape_id(shape_id);
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
            Action::About => self.about_open = !self.about_open,
            // #237: zoom-to-selection defers to the canvas frame — it
            // needs the content area + selection rects the frame owns.
            Action::ZoomSelection => self.zoom_selection_pending = true,
            Action::ToggleGrid => self.view.show_grid = !self.view.show_grid,
            Action::ToggleRulers => self.view.show_rulers = !self.view.show_rulers,
            Action::ToggleComments => self.show_comments = !self.show_comments,
            Action::ToggleDynamicGrid => {
                self.snap.dynamic_grid = !self.snap.dynamic_grid;
                self.apply_snap_settings();
            }
            Action::SnapGlue => self.snap_dlg_open = !self.snap_dlg_open,
            Action::Language => self.language_dlg_open = true,
            Action::PointerTool => {
                self.tool = Tool::Pointer;
                self.connpt_sel = None;
            }
            Action::AlignLeft => self.align_selection(crate::align::AlignKind::Left),
            Action::AlignHCenter => self.align_selection(crate::align::AlignKind::HCenter),
            Action::AlignRight => self.align_selection(crate::align::AlignKind::Right),
            Action::AlignTop => self.align_selection(crate::align::AlignKind::Top),
            Action::AlignVCenter => self.align_selection(crate::align::AlignKind::VCenter),
            Action::AlignBottom => self.align_selection(crate::align::AlignKind::Bottom),
            Action::AlignToPage => {
                self.align_to_page = !self.align_to_page;
                self.status = if self.align_to_page {
                    crate::i18n::strs().msg_aligning_to_the_page()
                } else {
                    crate::i18n::strs().msg_aligning_to_the_first_selected_s()
                };
            }
            Action::DistributeH => self.distribute_selection(false),
            Action::DistributeV => self.distribute_selection(true),
            Action::SelectAll => self.select_all(),
            Action::SelectByType => {
                self.selby_open = !self.selby_open;
                if self.selby_open {
                    self.status = crate::i18n::strs().msg_select_by_type_pick_a_criterion();
                }
            }
            Action::FormatPainter => self.arm_format_painter(false),
            Action::FormatPainterSticky => self.arm_format_painter(true),
            Action::PageOrientation => self.toggle_page_orientation(),
            Action::PageSizeDialog => self.pagesetup_open = !self.pagesetup_open,
            Action::CopyAsPicture => self.copypic_open = !self.copypic_open,
            Action::InsertPicture => self.open_picture_dialog(),
            Action::PageBackgrounds => self.backgrounds_open = !self.backgrounds_open,
            Action::ConnectorTool => {
                self.tool = Tool::Connector;
                self.status =
                    crate::i18n::strs().msg_connector_tool_drag_from_one_sha();
            }
            Action::ContainerStyle(name) => self.apply_container_style(name),
            Action::ContainerGallery => self.contgal_open = !self.contgal_open,
            Action::ContainerHighlight => self.toggle_container_highlight(),
            Action::ConnPointTool => {
                if self.tool == Tool::ConnectionPoint {
                    self.tool = Tool::Pointer;
                    self.connpt_sel = None;
                    self.status = crate::i18n::strs().msg_connection_point_tool_off();
                } else {
                    self.tool = Tool::ConnectionPoint;
                    self.connpt_sel = None;
                    self.status =
                        crate::i18n::strs().msg_connpoint_tool_status();
                }
            }
            Action::FreeformTool => {
                // Toggle like the other tools; the gesture arms on an
                // empty-page press and the release builds the shape.
                if self.tool == Tool::Freeform {
                    self.tool = Tool::Pointer;
                    self.status = crate::i18n::strs().msg_freeform_off();
                } else {
                    self.tool = Tool::Freeform;
                    self.status =
                        crate::i18n::strs().msg_freeform_drag_on_the_page_to_dra();
                }
            }
            Action::LassoTool => {
                // Toggle like Visio's Select ▸ Lasso Select menu item;
                // the Pointer Tool (or Esc) is Select Area.
                if self.tool == Tool::Lasso {
                    self.tool = Tool::Pointer;
                    self.status = crate::i18n::strs().msg_lasso_select_off();
                } else {
                    self.tool = Tool::Lasso;
                    self.status =
                        crate::i18n::strs().msg_lasso_select_drag_a_freehand_tra();
                }
            }
            Action::ToggleUnits => {
                self.view.unit = if self.view.unit == crate::units::Unit::Mm {
                    crate::units::Unit::In
                } else {
                    crate::units::Unit::Mm
                };
            }
            Action::ShowShapeSheet => {
                // #192 spec: per-shape sheets — the button targets the
                // selection's sheet (toggling); without a selection it
                // closes every open sheet.
                if let Some((_, sid)) = self.sel {
                    self.toggle_shapesheet(sid);
                } else {
                    self.shapesheets.clear();
                }
            }
            Action::Macros => self.status = crate::i18n::strs().msg_macros_arrive_with_m7_vba_vsdm_m(),
            Action::FillMenu | Action::LineMenu | Action::TextColorMenu => {
                self.color_pick = Some(match a {
                    Action::FillMenu => ColorTarget::Fill,
                    Action::LineMenu => ColorTarget::Line,
                    // #150: the ribbon Text color button rides #73's
                    // whole-selection Character row-0 write.
                    _ => ColorTarget::Text,
                });
            }
            Action::FontGrow => self.nudge_char_size(1.0),
            Action::FontShrink => self.nudge_char_size(-1.0),
            Action::RotateLeft => self.rotate_selection(90.0),
            Action::RotateRight => self.rotate_selection(-90.0),
            Action::Group => self.group_selection(),
            Action::ContainerDisband => self.disband_container(),
            Action::Ungroup => self.ungroup_selection(),
            Action::Hyperlink => {
                // #252 Insert ▸ Links: the Hyperlinks dialog (#28/#71)
                // on the primary selection; no selection notes it.
                match self.sel {
                    Some((_, sid)) => {
                        self.hyperlink_open = Some(sid);
                        self.hyperlink_draft = None;
                    }
                    None => self.status = crate::i18n::strs().msg_select_a_shape_to_edit_its_hyper(),
                }
            }
        }
    }

    // -- shell panels -----------------------------------------------------

    /// #67 (option D, owner 2026-10-28): the OS window draws the
    /// title — the app band that duplicated it is gone, and the
    /// document title (name + dirty marker) is pushed to the OS
    /// window on change instead. Headless tests assert through the
    /// tracked field; the customize chevron is dropped with the band
    /// (Visio's opens a real menu ours never had).
    fn sync_window_title(&mut self, ctx: &egui::Context) {
        let name = self
            .path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| crate::i18n::strs().msg_drawing_n(1));
        let title = format!("{name} - Tarahi{}", if self.dirty { " *" } else { "" });
        if self.window_title != title {
            self.window_title = title.clone();
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title));
        }
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
                // #257 item 3: RTL mirrors the strip — File lands at
                // the right edge and the tabs flow leftward.
                let strip_dir = if crate::i18n::rtl() {
                    egui::Layout::right_to_left(egui::Align::Center)
                } else {
                    egui::Layout::left_to_right(egui::Align::Center)
                };
                ui.with_layout(strip_dir, |ui| {
                    ui.set_min_height(STRIP_H);
                    // File: a green block spanning the full strip height
                    // — the green title band above makes it read as one
                    // tall tab, like the Office File tab.
                    let (fr, fresp) =
                        ui.allocate_exact_size(Vec2::new(54.0, STRIP_H), Sense::click());
                    // The painted text never reaches the accessibility
                    // tree on its own (GitHub #56).
                    fresp.widget_info(|| {
                        egui::WidgetInfo::labeled(egui::Role::Button, true, crate::i18n::strs().msg_file())
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
                        crate::i18n::strs().msg_file(),
                        egui::FontId::proportional(12.5),
                        Color32::WHITE,
                    );
                    if fresp.clicked() {
                        self.open_file_backstage();
                    }
                    // #184: the File badge (Office letter F).
                    self.keytips.badges.push(crate::keytips::Badge {
                        rect: fr,
                        key: "F".into(),
                        hit: crate::keytips::Hit::File,
                        anchor: None,
                    });
                    ui.add_space(8.0);
                    // #156: the contextual Container Tools tab rides
                    // after the fixed strip while a container is the
                    // primary selection (Visio 2016).
                    let sel_is_container = self.sel_is_container();
                    for t in TABS
                        .iter()
                        .chain(sel_is_container.then_some(&Tab::ContainerTools))
                    {
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
                        // #184: every strip tab badges its Office
                        // letter while keytips are armed.
                        if let Some(k) = crate::keytips::tab_keytip(*t) {
                            self.keytips.badges.push(crate::keytips::Badge {
                                rect: r,
                                key: k.into(),
                                hit: crate::keytips::Hit::Tab(*t),
                                anchor: None,
                            });
                        }
                        if active {
                            ui.painter().hline(
                                r.left() + 2.0..=r.right() - 2.0,
                                r.bottom(),
                                Stroke::new(2.0, tk.accent),
                            );
                        }
                        ui.add_space(6.0);
                    }
                    // #192 4a: the Window switcher (classic MDI
                    // position) — every open document, the active one
                    // marked; clicking activates.
                    let n_docs = self.docs.len() + self.ed.is_some() as usize;
                    if n_docs > 0 {
                        ui.add_space(8.0);
                        let label = crate::i18n::strs().msg_window_n(n_docs);
                        // (no widget_info override — the Button's own
                        // text is the accessible label, #56's concern is
                        // painted text only)
                        let wresp = ui.add(
                            egui::Button::new(egui::RichText::new(label).size(12.0)).frame(false),
                        );
                        if wresp.clicked() {
                            self.winmenu_open = !self.winmenu_open;
                        }
                        if self.winmenu_open {
                            let mut close_menu = false;
                            egui::Popup::menu(&wresp).show(|ui| {
                                ui.set_min_width(220.0);
                                // The ACTIVE document first (marked).
                                if self.ed.is_some() {
                                    let name = self
                                        .path
                                        .file_name()
                                        .map(|n| n.to_string_lossy().into_owned())
                                        .unwrap_or_else(|| crate::i18n::strs().msg_untitled());
                                    let mark = if self.dirty { " •" } else { "" };
                                    ui.add_enabled(
                                        false,
                                        egui::Label::new(
                                            // (› — Carlito has no ✓ glyph;
                                            // the breadcrumb's arrow)
                                            egui::RichText::new(format!("› {name}{mark}")).strong(),
                                        ),
                                    );
                                }
                                for (ix, d) in self.docs.iter().enumerate() {
                                    let name = d
                                        .path
                                        .file_name()
                                        .map(|n| n.to_string_lossy().into_owned())
                                        .unwrap_or_else(|| crate::i18n::strs().msg_untitled());
                                    let mark = if d.dirty { " •" } else { "" };
                                    if ui.button(format!("  {name}{mark}")).clicked() {
                                        self.pending_activate = Some(ix);
                                        close_menu = true;
                                    }
                                }
                            });
                            if close_menu {
                                self.winmenu_open = false;
                            }
                        }
                    }
                    // Office's "Tell me what you want to do" search box.                    // Office's "Tell me what you want to do" search
                    // box. The inner layout flips with the strip so it
                    // hugs the mirrored far end.
                    let tellme_dir = if crate::i18n::rtl() {
                        egui::Layout::left_to_right(egui::Align::Center)
                    } else {
                        egui::Layout::right_to_left(egui::Align::Center)
                    };
                    ui.with_layout(tellme_dir, |ui| {
                        ui.add_space(6.0);
                        let (pr, presp) =
                            ui.allocate_exact_size(Vec2::new(196.0, 20.0), Sense::click());
                        // Painted placeholder, so name the control for
                        // screen readers (GitHub #56).
                        presp.widget_info(|| {
                            egui::WidgetInfo::labeled(egui::Role::Button, true, crate::i18n::strs().msg_tell_me())
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
                            crate::i18n::strs().msg_tell_me_what_you_want_to_do(),
                            egui::FontId::proportional(11.0),
                            tk.hint_text,
                        );
                        let presp = presp.on_hover_text(crate::i18n::strs().msg_tell_me_planned_for_a_later_mile());
                        if presp.clicked() {
                            self.status = crate::i18n::strs().msg_tell_me_search_arrives_with_a_la();
                        }
                        // #184: Tell Me badges Q (Office's Alt+Q).
                        self.keytips.badges.push(crate::keytips::Badge {
                            rect: pr,
                            key: "Q".into(),
                            hit: crate::keytips::Hit::TellMe,
                            anchor: None,
                        });
                        // The QAT cluster lands LEFT of Tell Me (this is
                        // a right-to-left layout): Save/Undo/Redo as
                        // window-level document controls on the menu
                        // strip's right (#67 option D — with no menu
                        // bar they are the only always-visible command
                        // surface; the customize chevron is dropped
                        // with the old title band).
                        ui.add_space(10.0);
                        // #184: the digit badges ride the QAT buttons
                        // themselves (1..=9 by position, Office's
                        // rule).
                        qat_button(ui, Icon::Save, &crate::i18n::strs().msg_save_ctrl_s(), || Action::Save, self, 1);
                        qat_button(ui, Icon::Undo, &crate::i18n::strs().msg_undo_ctrl_z(), || Action::Undo, self, 2);
                        qat_button(ui, Icon::Redo, &crate::i18n::strs().msg_redo_ctrl_y(), || Action::Redo, self, 3);
                        // #242: Help lives in the QAT (the owner's
                        // placement) — opens the About window.
                        qat_button(ui, Icon::Help, &crate::i18n::strs().msg_help_about(), || Action::About, self, 4);
                    });
                });
                // #184: the armed veil covers the strip's rect
                // (unioned with the content row's at paint time).
                self.keytips.strip_rect = Some(ui.min_rect());
            });
        // #192 4a: a Window-menu pick activates after the strip's
        // mutable borrows end.
        if let Some(ix) = self.pending_activate.take() {
            self.activate_doc(ix);
        }
    }

    fn ribbon_content(&mut self, ui: &mut Ui) {
        let mut act: Option<Action> = None;
        let dispatch = |a: Action, act: &mut Option<Action>| {
            *act = Some(a);
        };
        // The contextual tab leaves when its context does (#156).
        if self.tab == Tab::ContainerTools && !self.sel_is_container() {
            self.tab = Tab::Home;
        }
        // One snapshot per frame; the per-tab modules (#252) own the
        // layouts and never borrow the app.
        let ctx = self.ribbon_ctx();
        let groups = crate::ribbon::groups(self.tab, &ctx);
        let mut launcher: Option<String> = None;
        // #184: every content-row control captures its rect for the
        // keytip badges (letters assigned at paint time). #257: under
        // an RTL language the badges also carry the control's ENGLISH
        // anchor label — the Latin name its keytip letter keys off
        // (the RTL Office scheme; the localized label has no ASCII
        // candidates) — zipped in content_row's traversal order.
        let anchors = if crate::i18n::rtl() {
            crate::ribbon::anchor_labels(self.tab, &ctx)
        } else {
            Vec::new()
        };
        let mut anchor_ix = 0usize;
        let badges = &mut self.keytips.badges;
        crate::ribbon::content_row(
            ui,
            &groups,
            &mut |a| dispatch(a, &mut act),
            &mut |label: &str| launcher = Some(label.to_string()),
            &mut |b, r| {
                let anchor = anchors.get(anchor_ix).cloned();
                anchor_ix += 1;
                badges.push(crate::keytips::Badge {
                    rect: r,
                    key: String::new(),
                    hit: crate::keytips::Hit::Action {
                        action: b.action,
                        enabled: b.enabled,
                        label: b.a11y_label().into(),
                    },
                    anchor,
                });
            },
        );
        self.keytips.content_rect = Some(ui.max_rect());
        if let Some(label) = launcher {
            self.status = crate::i18n::strs().msg_label_dialog_planned_for_a_later(label);
        }
        if let Some(a) = act {
            self.dispatch(a);
        }
        ui.add_space(2.0);
    }

    /// The per-frame ribbon snapshot (#252): every fact a tab's group
    /// builder needs, computed once so the per-tab modules stay pure
    /// layout.
    fn ribbon_ctx(&self) -> crate::ribbon::TabCtx {
        let sel_count = self
            .sel_set
            .iter()
            .filter(|(p, _)| Some(*p) == self.page_id())
            .count();
        crate::ribbon::TabCtx {
            has_doc: self.ed.is_some(),
            has_sel: self.sel.is_some(),
            multi_sel: sel_count >= 2,
            tri_sel: sel_count >= 3,
            tool: self.tool,
            painter_armed: self.painter_armed.is_some(),
            selby_open: self.selby_open,
            align_to_page: self.align_to_page,
            pagesetup_open: self.pagesetup_open,
            backgrounds_open: self.backgrounds_open,
            contgal_open: self.contgal_open,
            show_rulers: self.view.show_rulers,
            show_grid: self.view.show_grid,
            show_comments: self.show_comments,
            dynamic_grid: self.snap.dynamic_grid,
            snap_dlg_open: self.snap_dlg_open,
            shapesheet_open: self
                .sel
                .is_some_and(|(_, sid)| self.shapesheets.contains(&sid)),
            sel_group: self.selection_has_group(),
            sel_container: self.sel_is_container(),
            container_highlight: self.container_highlight_on(),
        }
    }

    /// Whether any selected shape on the current page is a group
    /// (#252 Home ▸ Arrange Ungroup enablement).
    fn selection_has_group(&self) -> bool {
        let Some(page_id) = self.page_id() else { return false };
        let Some(ed) = self.ed.as_ref() else { return false };
        let Some(page) = ed.doc.pages.iter().find(|p| p.id == page_id) else {
            return false;
        };
        self.sel_set.iter().any(|(p, sid)| {
            *p == page_id
                && vsdx::model::find_shape_by_id(&page.shapes, *sid)
                    .is_some_and(|sh| !sh.sub_shapes.is_empty())
        })
    }

    fn page_tabs(&mut self, ui: &mut Ui) {
        let t = theme::shell_tokens(ui.visuals());
        // #257 item 3: RTL mirrors the strip — the navigation cluster
        // hugs the right edge and the tabs flow leftward.
        let dir = if crate::i18n::rtl() {
            egui::Layout::right_to_left(egui::Align::Center)
        } else {
            egui::Layout::left_to_right(egui::Align::Center)
        };
        ui.with_layout(dir, |ui| {
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
            nav(ui, "<<", &crate::i18n::strs().msg_first_page(), page_idx > 0, 0);
            nav(
                ui,
                "<",
                &crate::i18n::strs().msg_previous_page(),
                page_idx > 0,
                page_idx.saturating_sub(1),
            );
            nav(
                ui,
                ">",
                &crate::i18n::strs().msg_next_page(),
                page_idx + 1 < n,
                (page_idx + 1).min(n - 1),
            );
            nav(ui, ">>", &crate::i18n::strs().msg_last_page(), page_idx + 1 < n, n - 1);
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
                                    .hint_text(crate::i18n::strs().msg_page_name()),
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
                                        .on_hover_text(crate::i18n::strs().msg_page_tab_tooltip(i + 1, id));
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
        // #257 item 3: RTL mirrors the bar — the page/shape readout
        // hugs the right edge and the zoom cluster the left.
        let dir = if crate::i18n::rtl() {
            egui::Layout::right_to_left(egui::Align::Center)
        } else {
            egui::Layout::left_to_right(egui::Align::Center)
        };
        let rev = if crate::i18n::rtl() {
            egui::Layout::left_to_right(egui::Align::Center)
        } else {
            egui::Layout::right_to_left(egui::Align::Center)
        };
        ui.with_layout(dir, |ui| {
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
            ui.with_layout(rev, |ui| {
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
                    let sresp = ui.add_sized([110.0, 14.0], slider).on_hover_text(crate::i18n::strs().msg_zoom());
                    if sresp.changed() {
                        self.view.zoom = z.clamp(0.1, 8.0);
                        self.view.fit_mode = false;
                    }
                    if ui.button("−").on_hover_text(crate::i18n::strs().msg_zoom_out()).clicked() {
                        self.dispatch(Action::ZoomOut);
                    }
                    if ui.button("+").on_hover_text(crate::i18n::strs().msg_zoom_in()).clicked() {
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
            self.apply(Macro::new(crate::i18n::strs().msg_undo_set_cell(name), cmds));
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
            self.apply(Macro::new(
                crate::i18n::strs().msg_undo_set_section_cell(section, cell),
                cmds,
            ));
        }
    }

    /// A named-choice dropdown (the manual popup — the stock ComboBox
    /// glyph is missing from the pinned fonts). Returns the chosen
    /// value's key when the user picks one.
    fn fmt_dropdown(
        ui: &mut Ui,
        current: &str,
        key_of: &dyn Fn(&str) -> String,
        choices: &[(&str, String)], // (key, display)
    ) -> Option<String> {
        let display = choices
            .iter()
            .find(|(k, _)| *k == current)
            .map(|(_, d)| d.clone())
            .unwrap_or_else(|| current.to_string());
        let r = ui.button(format!("{display}  ›"));
        let mut picked = None;
        egui::Popup::menu(&r)
            // Close-on-click would dismiss the menu on any scrollbar
            // interaction, so the menu closes only on outside clicks
            // and ui.close() after a pick (the arrow gallery's 46
            // entries need the scroll).
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .show(|ui| {
                ui.set_min_width(130.0);
                let max_h = (ui.input(|i| i.viewport_rect().height()) - 120.0).clamp(180.0, 520.0);
                egui::ScrollArea::vertical()
                    .max_height(max_h)
                    .show(ui, |ui| {
                        for (k, d) in choices {
                            if ui.selectable_label(*k == current, d.clone()).clicked() {
                                picked = Some(key_of(k));
                                // CloseOnClickOutside keeps the menu
                                // open otherwise — close on a pick.
                                ui.close();
                            }
                        }
                    });
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
            egui::RichText::new(crate::i18n::strs().msg_format_shape())
                .size(13.0)
                .color(t.label),
        );
        ui.add_space(4.0);
        let mut bufs = std::mem::take(&mut self.fmt_bufs);
        let mut color_pick: Option<ColorTarget> = None;
        let mut set_cell: Option<(String, String)> = None;
        let mut set_row: Option<(String, String, String)> = None;
        // #149 item 2: a targeted open (a mini-toolbar Format icon)
        // forces its section open and scrolls to it once, then the
        // focus is consumed.
        let focus = self.format_pane_focus;
        let mut scrolled = false;

        egui::ScrollArea::vertical()
            .show(ui, |ui| {
        let mut fill_hdr =
            egui::CollapsingHeader::new(crate::i18n::strs().msg_fill()).default_open(true);
        if focus == Some(FormatSection::Fill) {
            fill_hdr = fill_hdr.open(Some(true));
        }
        let fill_rsp = fill_hdr.show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(crate::i18n::strs().msg_foreground());
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
                    ui.label(crate::i18n::strs().msg_background());
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
                        ("0", crate::i18n::strs().msg_none()),
                        ("1", crate::i18n::strs().msg_solid()),
                        ("2", "5%".to_string()),
                        ("3", "10%".to_string()),
                        ("4", "20%".to_string()),
                        ("5", "30%".to_string()),
                        ("6", "40%".to_string()),
                        ("24", crate::i18n::strs().msg_gradient_horiz()),
                        ("25", crate::i18n::strs().msg_gradient_vert()),
                        ("28", crate::i18n::strs().msg_gradient_radial()),
                    ],
                ) {
                    set_cell = Some(("FillPattern".into(), v));
                }
            });
        if focus == Some(FormatSection::Fill) {
            fill_rsp.header_response.scroll_to_me(Some(egui::Align::Min));
            scrolled = true;
        }
        ui.add_space(2.0);
        let mut line_hdr =
            egui::CollapsingHeader::new(crate::i18n::strs().msg_line()).default_open(true);
        if focus == Some(FormatSection::Line) {
            line_hdr = line_hdr.open(Some(true));
        }
        let line_rsp = line_hdr.show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(crate::i18n::strs().msg_color());
                    if ui
                        .button(self.eff_cell_str("LineColor").unwrap_or_else(|| "—".into()))
                        .clicked()
                    {
                        color_pick = Some(ColorTarget::Line);
                    }
                });
                ui.horizontal(|ui| {
                    ui.label(crate::i18n::strs().msg_weight());
                    let cur_pt = self.eff_cell_num("LineWeight").unwrap_or(0.0104) * 72.0;
                    let mut pt = cur_pt;
                    let r = ui.add(
                        egui::Slider::new(&mut pt, 0.25..=12.0)
                            .show_value(true)
                            .custom_formatter(|v, _| format!("{v:.2} pt")),
                    );
                    r.widget_info(|| {
                        egui::WidgetInfo::labeled(egui::Role::Slider, true, crate::i18n::strs().msg_line_weight())
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
                    // #171: labels match Visio 2016's measured gallery
                    // (Analysis/line-patterns-visio2016.md) — 9 is the
                    // short 4-2 dash, 16 the long dash.
                    &[
                        ("0", crate::i18n::strs().msg_none()),
                        ("1", crate::i18n::strs().msg_solid()),
                        ("2", crate::i18n::strs().msg_dash()),
                        ("3", crate::i18n::strs().msg_dot()),
                        ("4", crate::i18n::strs().msg_dash_dot()),
                        ("5", crate::i18n::strs().msg_dash_dot_dot()),
                        ("9", crate::i18n::strs().msg_short_dash()),
                        ("10", crate::i18n::strs().msg_fine_dot()),
                        ("16", crate::i18n::strs().msg_long_dash()),
                        ("23", crate::i18n::strs().msg_fine_dash()),
                    ],
                ) {
                    set_cell = Some(("LinePattern".into(), v));
                }
                // Begin/End Arrow (#72, #169, #199): every value 0–45
                // carries its own glyph, pinned from the 600 dpi Visio
                // print (Analysis/arrow-gallery-visio2016.md, the
                // 2026-11-06 re-read). The picker lists the gallery
                // per value, Visio-style.
                let arrows: &[(&str, String)] = &[
                    ("0", crate::i18n::strs().msg_none()),
                    ("1", crate::i18n::strs().msg_open_2_prong()),
                    ("2", crate::i18n::strs().msg_small_arrow()),
                    ("3", crate::i18n::strs().msg_open_wide()),
                    ("4", crate::i18n::strs().msg_classic_arrow()),
                    ("5", crate::i18n::strs().msg_wide_arrow()),
                    ("6", crate::i18n::strs().msg_long_arrow()),
                    ("7", crate::i18n::strs().msg_open_fork()),
                    ("8", crate::i18n::strs().msg_stealth_dart()),
                    ("9", crate::i18n::strs().msg_slanted_tick()),
                    ("10", crate::i18n::strs().msg_filled_circle()),
                    ("11", crate::i18n::strs().msg_filled_square()),
                    ("12", crate::i18n::strs().msg_open_v_tall()),
                    ("13", crate::i18n::strs().msg_filled_arrow_plain()),
                    ("14", crate::i18n::strs().msg_open_triangle_large()),
                    ("15", crate::i18n::strs().msg_open_triangle_small()),
                    ("16", crate::i18n::strs().msg_open_triangle_medium_large()),
                    ("17", crate::i18n::strs().msg_open_triangle_medium()),
                    ("18", crate::i18n::strs().msg_open_triangle_convex_base()),
                    ("19", crate::i18n::strs().msg_open_barbed()),
                    ("20", crate::i18n::strs().msg_open_circle()),
                    ("21", crate::i18n::strs().msg_open_square()),
                    ("22", crate::i18n::strs().msg_open_diamond()),
                    ("23", crate::i18n::strs().msg_slanted_tick()),
                    ("24", crate::i18n::strs().msg_tick()),
                    ("25", crate::i18n::strs().msg_double_tick()),
                    ("26", crate::i18n::strs().msg_triple_tick()),
                    ("27", crate::i18n::strs().msg_crow_s_foot()),
                    ("28", crate::i18n::strs().msg_crow_s_foot_tick()),
                    ("29", crate::i18n::strs().msg_fork_ring()),
                    ("30", crate::i18n::strs().msg_ring_tick()),
                    ("31", crate::i18n::strs().msg_tick_ring()),
                    ("32", crate::i18n::strs().msg_double_tick_ring()),
                    ("33", crate::i18n::strs().msg_triple_tick_ring()),
                    ("34", crate::i18n::strs().msg_diamond_ring()),
                    ("35", crate::i18n::strs().msg_tick_dot()),
                    ("36", crate::i18n::strs().msg_double_tick_dot()),
                    ("37", crate::i18n::strs().msg_triple_tick_dot()),
                    ("38", crate::i18n::strs().msg_diamond_dot()),
                    ("39", crate::i18n::strs().msg_double_arrow_filled()),
                    ("40", crate::i18n::strs().msg_double_arrow_open()),
                    ("41", crate::i18n::strs().msg_ring()),
                    ("42", crate::i18n::strs().msg_dot()),
                    ("43", crate::i18n::strs().msg_open_double_chevron_large()),
                    ("44", crate::i18n::strs().msg_tick_chevron()),
                    ("45", crate::i18n::strs().msg_tick_double_chevron()),
                ];
                let cur = self
                    .eff_cell_str("BeginArrow")
                    .unwrap_or_else(|| "0".into());
                if let Some(v) = Self::fmt_dropdown(ui, &cur, &|k: &str| k.to_string(), arrows) {
                    set_cell = Some(("BeginArrow".into(), v));
                }
                let cur = self.eff_cell_str("EndArrow").unwrap_or_else(|| "0".into());
                if let Some(v) = Self::fmt_dropdown(ui, &cur, &|k: &str| k.to_string(), arrows) {
                    set_cell = Some(("EndArrow".into(), v));
                }
                ui.horizontal(|ui| {
                    ui.label(crate::i18n::strs().msg_rounding());
                    let cur = self.eff_cell_str("Rounding").unwrap_or_else(|| "0".into());
                    let buf = bufs.entry("rounding".into()).or_insert(cur);
                    let r = ui.add(egui::TextEdit::singleline(buf).desired_width(70.0));
                    r.widget_info(|| {
                        egui::WidgetInfo::labeled(egui::Role::TextInput, true, crate::i18n::strs().msg_rounding())
                    });
                    if r.lost_focus() {
                        if let Ok(v) = buf.trim().parse::<f64>() {
                            set_cell = Some(("Rounding".into(), format!("{v:.6}")));
                        }
                        bufs.remove("rounding");
                    }
                });
            });
        if focus == Some(FormatSection::Line) {
            line_rsp.header_response.scroll_to_me(Some(egui::Align::Min));
            scrolled = true;
        }
        ui.add_space(2.0);
        let mut text_hdr =
            egui::CollapsingHeader::new(crate::i18n::strs().msg_text()).default_open(true);
        if focus == Some(FormatSection::Text) {
            text_hdr = text_hdr.open(Some(true));
        }
        let text_rsp = text_hdr.show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(crate::i18n::strs().msg_font());
                    let cur = self.eff_row_str("Character", "Font").unwrap_or_default();
                    let buf = bufs.entry("font".into()).or_insert(cur);
                    let r = ui.add(egui::TextEdit::singleline(buf).desired_width(120.0));
                    r.widget_info(|| {
                        egui::WidgetInfo::labeled(egui::Role::TextInput, true, crate::i18n::strs().msg_font())
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
                    ui.label(crate::i18n::strs().msg_size_pt());
                    let cur = self
                        .eff_row_str("Character", "Size")
                        .and_then(|v| v.parse::<f64>().ok())
                        .unwrap_or(10.0)
                        * 72.0;
                    let buf = bufs.entry("size".into()).or_insert(format!("{cur:.1}"));
                    let r = ui.add(egui::TextEdit::singleline(buf).desired_width(70.0));
                    r.widget_info(|| {
                        egui::WidgetInfo::labeled(egui::Role::TextInput, true, crate::i18n::strs().msg_size_pt())
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
                    ui.label(crate::i18n::strs().msg_style());
                    let bits = self
                        .eff_row_str("Character", "Style")
                        .and_then(|v| v.parse::<i64>().ok())
                        .unwrap_or(0);
                    let mut bold = bits & 1 != 0;
                    let mut italic = bits & 2 != 0;
                    let rb = ui.checkbox(&mut bold, "B");
                    rb.widget_info(|| {
                        egui::WidgetInfo::labeled(egui::Role::CheckBox, true, crate::i18n::strs().msg_bold())
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
                        egui::WidgetInfo::labeled(egui::Role::CheckBox, true, crate::i18n::strs().msg_italic())
                    });
                    if ri.changed() {
                        set_row = Some((
                            "Character".into(),
                            "Style".into(),
                            format!("{}", (bits & !2) | ((italic as i64) * 2)),
                        ));
                    }
                });
                ui.horizontal(|ui| {
                    ui.label(crate::i18n::strs().msg_color());
                    if ui
                        .button(
                            self.eff_row_str("Character", "Color")
                                .unwrap_or_else(|| "—".into()),
                        )
                        .clicked()
                    {
                        color_pick = Some(ColorTarget::Text);
                    }
                });
                let cur = self
                    .eff_row_str("Paragraph", "HorzAlign")
                    .unwrap_or_else(|| "0".into());
                if let Some(v) = Self::fmt_dropdown(
                    ui,
                    &cur,
                    &|k: &str| k.to_string(),
                    &[("0", crate::i18n::strs().msg_align_left()), ("1", crate::i18n::strs().msg_center()), ("2", crate::i18n::strs().msg_align_right())],
                ) {
                    set_row = Some(("Paragraph".into(), "HorzAlign".into(), v));
                }
            });
        if focus == Some(FormatSection::Text) {
            text_rsp.header_response.scroll_to_me(Some(egui::Align::Min));
            scrolled = true;
        }
            });
        if scrolled {
            self.format_pane_focus = None;
        }

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

    /// #192 spec: toggle one shape's ShapeSheet window (open order
    /// kept; re-toggling removes).
    fn toggle_shapesheet(&mut self, shape: i64) {
        if let Some(i) = self.shapesheets.iter().position(|sid| *sid == shape) {
            self.shapesheets.remove(i);
        } else {
            self.shapesheets.push(shape);
        }
    }

    fn shapesheet_panel(&mut self, ui: &mut Ui, shape_id: i64, max_w: f32) {
        // #190/#192: the window's title bar carries the close; this is
        // pure content, bound to the sheet's OWN shape (not the live
        // selection). #189: the body itself lives in `shapesheet.rs`
        // (app decomposition) — the resolved, Visio-2016-style view.
        ui.vertical(|ui| {
            ui.separator();
            let Some(page_id) = self.page_id() else {
                return;
            };
            let Some(ed) = self.ed() else { return };
            crate::shapesheet::panel(ui, &ed.doc, page_id, shape_id, max_w);
        });
    }

    /// The Recent list shared by the start screen and the backstage
    /// Open page: one row per MRU entry, click to reopen. Files that no
    /// longer exist stay listed but ghosted (Visio-style) rather than
    /// being silently dropped. Returns the clicked path, if any.
    fn recent_list_ui(&self, ui: &mut Ui) -> Option<PathBuf> {
        if self.recent.entries().is_empty() {
            ui.weak(crate::i18n::strs().msg_files_you_open_will_appear_here());
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

    /// #192 MDI: with a document open, the drawing is a titled child
    /// window (document name + page, movable/resizable, no close —
    /// documents close through File) constrained to the client area
    /// this ui already is. The id is pinned: the TITLE changes with
    /// page/dirty flips, and an id-from-title window would lose its
    /// placement on every one of them. Without a document the start
    /// screen fills the frame unwrapped.
    fn central(&mut self, ui: &mut Ui) {
        // The client area is what the docked panels LEFT — the
        // available rect, not max_rect (the ui's full allocation):
        // egui's inside-panels shrink the former, and a window
        // constrained to max_rect would float over the stencil pane.
        // An 8 px gutter insets the child windows inside it: the
        // workspace reads as an MDI client region, and the windows'
        // resize-border sense zones never cover a docked panel's own
        // resize handle at the shared edge.
        let avail = ui.available_rect_before_wrap();
        self.mdi_client = if avail.is_finite() && avail.width() > 24.0 && avail.height() > 24.0 {
            avail.shrink(8.0)
        } else {
            avail
        };
        if self.ed.is_none() {
            self.central_inner(ui);
            return;
        }
        let doc_name = self
            .path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| crate::i18n::strs().msg_untitled());
        let page_name = self
            .ed()
            .and_then(|e| e.doc.pages.get(self.page_idx))
            .map(|p| p.name.clone())
            .unwrap_or_default();
        let mut title = format!("{doc_name} — {page_name}");
        if self.dirty {
            title.push_str(" •");
        }
        let client = self.mdi_drawing_tile;
        // #192 spec: the drawing's title-bar close (decorated mode
        // only — the single undecorated child has no X) runs the
        // SAME flow as File > Close: dirty asks first, clean closes.
        let mut doc_close = false;
        let mut win = egui::Window::new(title)
            .id(egui::Id::unique("dc-drawing-window"))
            .title_bar(self.mdi_decorated)
            .resizable(true)
            .collapsible(false)
            .min_size(egui::vec2(240.0, 180.0));
        let mut open = true;
        if self.mdi_decorated {
            win = win.open(&mut open);
        }
        if (!self.mdi_decorated || self.mdi_arranging) && client.is_finite() && client.width() > 0.0
        {
            // Undecorated single child: pinned full-bleed; arranging:
            // pinned to the tile for the transition frame(s). (Frame 0
            // carries no tile yet — the window seeds from its
            // defaults and the pin takes over the frame after.)
            win = win.current_pos(client.left_top()).fixed_size(client.size());
        }
        if self.mdi_client.is_finite() && self.mdi_client.width() > 0.0 {
            win = win.constrain_to(self.mdi_client);
        }
        win.show(ui.ctx(), |ui| self.central_inner(ui));
        if !open {
            doc_close = true;
        }
        if doc_close {
            // Identical to BackAction::CloseDoc: a dirty document
            // asks (the modal), a clean one closes now.
            if self.dirty {
                self.confirm_close = Some(CloseTarget::Doc);
            } else {
                self.close_doc();
            }
        }
        // The arrangement pin is consumed HERE (the drawing window
        // builds in this fn, one frame after `draw` computed it on
        // the count change); the sheets consumed it in `draw`.
        self.mdi_arranging = false;
    }

    fn central_inner(&mut self, ui: &mut Ui) {
        if self.ed.is_none() {
            // Visio 2016 start screen shape: Recent on the left,
            // template choices on the right.
            ui.horizontal_centered(|ui| {
                ui.add_space(40.0);
                ui.vertical(|ui| {
                    ui.add_space(24.0);
                    ui.heading(crate::i18n::strs().msg_recent());
                    ui.add_space(6.0);
                    if let Some(p) = self.recent_list_ui(ui) {
                        self.load(p);
                    }
                });
                // #257 item 3: the New column hugs the mirrored far
                // end of the start screen.
                let start_new_dir = if crate::i18n::rtl() {
                    egui::Layout::left_to_right(egui::Align::TOP)
                } else {
                    egui::Layout::right_to_left(egui::Align::TOP)
                };
                ui.with_layout(start_new_dir, |ui| {
                    ui.vertical(|ui| {
                        ui.add_space(24.0);
                        ui.heading(crate::i18n::strs().msg_rail_new());
                        ui.add_space(6.0);
                        if ui
                            .button(crate::i18n::strs().msg_blank_drawing())
                            .on_hover_text(crate::i18n::strs().msg_start_an_empty_us_letter_drawing())
                            .clicked()
                        {
                            self.do_new();
                        }
                        if ui
                            .button(crate::i18n::strs().msg_open_a_visio_file())
                            .on_hover_text(crate::i18n::strs().msg_open_vsdx_vstx_vssx_ctrl_o())
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
                    egui::RichText::new(crate::i18n::strs().msg_group_breadcrumb(names.join(" › ")))
                        .small()
                        .color(ui.visuals().weak_text_color()),
                );
                if ui.small_button(crate::i18n::strs().msg_close_group()).clicked() {
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
                && ui.ctx().input(|i| i.key_pressed(egui::Key::Escape))
            {
                self.group_edit.pop();
            }
        }
        let geom = self.page_geom();
        let sel_rect = self.sel_page_rect();
        let extra_rects: Vec<Rect> = self.sel_extra_page_rects();
        // #156: the selected container's member rects — the highlight
        // overlay (default on; the Container Tools toggle writes the
        // container's msvSDContainerNoHighlight row).
        let member_hl: Vec<Rect> = if self.sel_is_container() && self.container_highlight_on() {
            let (page_id, sid) = self.sel.unwrap();
            self.ed()
                .and_then(|ed| ed.doc.pages.iter().find(|p| p.id == page_id))
                .map(|page| {
                    self.container_member_ids(page_id, sid)
                        .iter()
                        .filter_map(|m| {
                            let doc = &self.ed().unwrap().doc;
                            vsdx::model::find_shape_by_id(&page.shapes, *m)
                                .and_then(|sh| self.shape_rect(doc, page_id, sh))
                        })
                        .map(|(x, y, w, h)| {
                            // page y-up → the canvas's y-down rect frame
                            let geom = self.page_geom();
                            Rect::from_min_max(
                                egui::pos2(x as f32, (geom.h_in - y - h) as f32),
                                egui::pos2((x + w) as f32, (geom.h_in - y) as f32),
                            )
                        })
                        .collect()
                })
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        // The page's guides (#113): collected before the borrows below
        // (the marquee-gating hit closure and the overlay both use
        // them).
        let page_guides = self.page_guides();
        let guide_fx: Vec<canvas::GuideFx> = page_guides
            .iter()
            .map(|g| canvas::GuideFx {
                id: g.id,
                vertical: match g.kind {
                    crate::guides::GuideKind::Vertical => Some(true),
                    crate::guides::GuideKind::Horizontal => Some(false),
                    crate::guides::GuideKind::Point => None,
                },
                x: g.x,
                y: g.y,
                selected: self.sel.is_some_and(|(_, sid)| sid == g.id),
            })
            .collect();
        let guide_hit_threshold = 5.0 / self.view.px_per_in() as f64;
        // Live move feedback (#96): during a moved drag the dragged
        // shapes' static layers are suppressed and ghosted at the live
        // position — Visio moves the shape under the cursor for the
        // whole gesture, and a move ghost is a quad translation, never
        // a re-raster (the #93 O(0)-move guarantee holds).
        let mut drag_fx = canvas::DragFx::default();
        let mut hide_layers: Vec<i64> = Vec::new();
        // #226: an endpoint-handle drag on the selected 1D shape hides
        // its static layer quad — the styled live ghost (built after
        // the snap closures below) replaces it for the gesture. The
        // ghost plans through [`crate::cmd::plan_connector_frame`] for
        // BOTH connectors and plain 1D lines: the planner's line_frame
        // branch and `apply_line_endpoint`'s angle model write
        // different cell sets, but both draw the same segment ink the
        // release lands, so the preview matches the drop (#185
        // review — verified; the cells diverge, the visible ink does
        // not).
        let armed_handle_now = canvas::armed_handle(ui.ctx());
        let endpoint_drag_live = matches!(
            armed_handle_now,
            Some(HandleId::LineBegin) | Some(HandleId::LineEnd)
        ) && self.sel.is_some_and(|(p, _)| Some(p) == self.page_id());
        if endpoint_drag_live {
            if let Some((_, sid)) = self.sel {
                hide_layers.push(sid);
            }
        } else {
            self.endpoint_preview = None;
        }
        let mut live_drag = false;
        if let Some(d) = self.drag.as_ref().filter(|d| d.moved) {
            if self.sel.is_some_and(|(p, _)| Some(p) == self.page_id()) {
                live_drag = true;
                // #109: the dynamic-grid hints computed by this frame's
                // `apply_drag_snap` (update_drag runs before the paint).
                drag_fx.hints = self.snap_live.clone();
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
                            drag_fx.ghosts.push(crate::layers::ShapeQuad {
                                id,
                                tex: q.tex,
                                bbox,
                            });
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
                            if let Some(shape) = vsdx::model::find_shape_by_id(&page.shapes, id) {
                                // Nested-aware: a group-edit member
                                // maps its parent-local rect through
                                // the XForm chain to page space.
                                if let Some((x, y, w, h)) = self.shape_rect(&ed.doc, page.id, shape)
                                {
                                    let tl = canvas::page_to_screen(
                                        x + dx,
                                        y + h + dy,
                                        &self.view,
                                        &geom,
                                    );
                                    let br = canvas::page_to_screen(
                                        x + w + dx,
                                        y + dy,
                                        &self.view,
                                        &geom,
                                    );
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
                (x0.min(x1), y0.min(y1), (x1 - x0).abs(), (y1 - y0).abs())
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
        // Background pages render beneath this page's ink (#113) —
        // built before the field borrows below.
        let underlays = self.build_bg_underlays(ui.ctx());
        // #108: line-jump hops for the frame's connector crossings.
        // The page's Show-Line-Jumps switch gates the overlay (#251);
        // the jump-size multiplier scales the hops.
        let jump_factor = {
            let page = &self.ed.as_ref().unwrap().doc.pages[self.page_idx];
            if page.line_jumps_shown() {
                page.line_jump_factor()
            } else {
                0.0
            }
        };
        let jump_fx: Vec<canvas::JumpFx> = if jump_factor > 0.0 {
            crate::connectors::line_jumps(&self.jump_lines())
                .into_iter()
                .map(|j| canvas::JumpFx {
                    at: j.at,
                    dir: j.dir,
                    under: j.under,
                    weight: j.weight,
                    under_weight: j.under_weight,
                    color: Color32::from_rgb(j.color.0, j.color.1, j.color.2),
                    under_color: Color32::from_rgb(
                        j.under_color.0,
                        j.under_color.1,
                        j.under_color.2,
                    ),
                    factor: jump_factor as f32,
                })
                .collect()
        } else {
            Vec::new()
        };
        self.jumps_last = jump_fx.len();
        // #177: everything the connection-point overlay needs, built
        // before the destructuring borrows below (the closure runs
        // inside `canvas::show`, while `view` is borrowed).
        let conn_cands = self.glue_candidates(&[]);
        let conn_threshold = SNAP_STRENGTH_PX as f64 / self.view.px_per_in() as f64;
        let conn_snap = self.snap;
        let conn_sel = self.sel;
        // #108: the Connection Point tool's state, precomputed (the
        // fx closure below cannot borrow self).
        let conn_tool = self.tool == Tool::ConnectionPoint;
        let connpt_sel_pos = self.connpt_sel.and_then(|(sid, ix)| {
            self.glue_candidates(&[])
                .into_iter()
                .find(|c| c.id == sid)?
                .conn_rows
                .into_iter()
                .find(|(rix, _)| *rix == ix)
                .map(|(_, p)| p)
        });
        // #179: live endpoint snapping — two target sets: one
        // excluding the selected connector (its own endpoint handle
        // is the dragged thing) and one excluding nothing (the
        // Connector Tool's fresh draw). The context is built inside
        // the closure (SnapCtx borrows the targets).
        let snap_settings = self.snap;
        // #202: connector-endpoint drags magnetize to CONNECTION
        // POINTS (+ guides) only — the shape-handle/vertex magnets
        // snapped drops onto the old shape's edges and re-glued them
        // there (the hard-glued / one-axis-only feel).
        let sel_is_conn = self.sel.is_some_and(|(p, sid)| {
            p == self.page_id().unwrap_or(0)
                && self.ed.as_ref().is_some_and(|ed| {
                    let page = &ed.doc.pages[self.page_idx];
                    vsdx::model::find_shape_by_id(&page.shapes, sid).is_some_and(|sh| {
                        shape_chain(&ed.doc, page.id, sid)
                            .ok()
                            .is_some_and(|ch| crate::connectors::is_connector_in(&ed.doc, &ch, sh))
                    })
                })
        });
        let (snap_grid, snap_ruler, snap_thr, snap_mirror) = (
            canvas::grid_step_in(&self.view),
            canvas::ruler_interval_in(&self.view),
            SNAP_STRENGTH_PX as f64 / self.view.px_per_in() as f64,
            // The RTL ruler's zero at the page's right edge (see
            // SnapCtx::ruler_mirror_x_in).
            crate::i18n::rtl().then(|| self.page_geom().w_in),
        );
        let (t_sel, g_sel, t_all, g_all) = {
            let pid = self.page_id().unwrap_or(-1);
            let sel_id = conn_sel.map(|(_, s)| s).unwrap_or(-1);
            (
                self.snap_targets(pid, &[sel_id]),
                self.snap_guides(&[sel_id]),
                self.snap_targets(pid, &[-1]),
                self.snap_guides(&[-1]),
            )
        };
        // Both closures below share t_sel/g_sel, so they capture
        // references, not the vectors.
        let (t_sel_r, g_sel_r) = (&t_sel, &g_sel);
        let snap_end = move |endpoint_drag: bool, p: (f64, f64)| -> (f64, f64) {
            let (targets, guides) = if endpoint_drag {
                (t_sel_r, g_sel_r)
            } else {
                (&t_all, &g_all)
            };
            let ctx = crate::snap::SnapCtx {
                targets,
                guides,
                grid_step_in: snap_grid,
                ruler_step_in: snap_ruler,
                ruler_mirror_x_in: snap_mirror,
                threshold_in: snap_thr,
            };
            let mut settings = snap_settings;
            if endpoint_drag && sel_is_conn {
                settings.glue_shape_handles = false;
                settings.glue_shape_vertices = false;
            }
            crate::snap::point_snap(p, &ctx, &settings, true).map_or(p, |(q, _)| q)
        };
        // #160: the live corner-resize preview snaps per axis under
        // the SNAP settings against the page minus the selected shape
        // (t_sel/g_sel exclude it) — the commit path runs the
        // identical snap on the release pointer.
        let snap_corner = move |p: (f64, f64)| -> Option<(f64, f64)> {
            let ctx = crate::snap::SnapCtx {
                targets: t_sel_r,
                guides: g_sel_r,
                grid_step_in: snap_grid,
                ruler_step_in: snap_ruler,
                ruler_mirror_x_in: snap_mirror,
                threshold_in: snap_thr,
            };
            crate::snap::corner_snap(p, &ctx, &snap_settings)
        };
        let conn_fx_seen = std::cell::RefCell::new(crate::connectors::ConnPointFx::default());
        // #226: the live endpoint-drag ghost — the connector's styled
        // ink re-rendered through the SHARED planner at the snapped
        // pointer; the bare live line stands down while it paints.
        let mut endpoint_ghost = false;
        if endpoint_drag_live {
            if let (Some((page_id, sid)), Some(hid)) = (self.sel, armed_handle_now) {
                if let Some(ptr) = ui.input(|i| i.pointer.interact_pos()) {
                    let ptr_page = canvas::screen_to_page(ptr, &self.view, &geom);
                    if let Some(q) =
                        self.endpoint_preview_quad(ui.ctx(), page_id, sid, hid, &snap_end, ptr_page)
                    {
                        drag_fx.ghosts.push(q);
                        endpoint_ghost = true;
                    }
                }
            }
        }
        drag_fx.endpoint_ghost = endpoint_ghost;
        self.last_endpoint_ghost = endpoint_ghost;
        let mut mouse = self.mouse_page;
        // #202: computed before the destructuring self-borrow the
        // canvas call holds.
        let sel_line_curved = self.sel_1d_curved();
        // #237: a pending zoom-to-selection centers + fills the
        // selection's union bounds on this frame — the same view ledger
        // the Fit Page and #22 text-edit zoom paths live on. Runs
        // before the destructuring self-borrow and the canvas frame so
        // the wheel anchor and the paint agree.
        if self.zoom_selection_pending {
            self.zoom_selection_pending = false;
            if let Some(rect) = self.sel_union_page_rect() {
                let (origin, content) =
                    canvas::content_area(self.canvas_rect, self.view.show_rulers);
                self.view
                    .zoom_to_selection(rect, content, origin.to_vec2(), &geom);
            }
        }
        let App {
            view, ed, page_idx, ..
        } = self;
        let ed = ed.as_ref();
        let page = ed.map(|e| &e.doc.pages[(*page_idx).min(e.doc.pages.len().saturating_sub(1))]);
        let hit = |pos: (f64, f64)| -> bool {
            let Some(ed) = ed else { return false };
            let Some(page) = page else { return false };
            // Guides count as occupied page (#113): a press on one
            // arms its move-drag, never a marquee.
            if crate::guides::guide_hit(&page_guides, pos, guide_hit_threshold).is_some() {
                return true;
            }
            for shape in page.shapes.iter().rev() {
                if let Some((x, y, w, h)) = Self::shape_rect_static(&ed.doc, page.id, shape) {
                    if hit_in_rect(pos, x, y, w, h) {
                        return true;
                    }
                }
            }
            false
        };
        // #177: connection-point affordances for the frame — the
        // closure paints from inside `canvas::show` and its result is
        // kept as the test seam.
        let conn_fx = |live: Option<(f64, f64)>| -> crate::connectors::ConnPointFx {
            let mut fx = crate::connectors::ConnPointFx::default();
            if let (Some(ed), Some(pg)) = (ed, page) {
                if let Some((_, cid)) = conn_sel.filter(|(p, _)| *p == pg.id) {
                    fx.glued = crate::connectors::glued_sites(&ed.doc, pg, cid, &conn_cands);
                }
            }
            if let Some(p) = live {
                let g = crate::connectors::conn_point_fx(
                    &conn_cands,
                    p,
                    p,
                    conn_threshold,
                    CONN_REVEAL_IN,
                    conn_snap.glue_active,
                    conn_snap.glue_connection_points,
                );
                fx.reveal = g.reveal;
                fx.glue_target = g.glue_target;
            }
            // #108: the Connection Point tool's selection ring — the
            // selected row's page position.
            if conn_tool {
                if let Some(q) = connpt_sel_pos {
                    fx.tool_selected = Some(q);
                }
            }
            *conn_fx_seen.borrow_mut() = fx.clone();
            fx
        };
        let out = canvas::show(
            ui,
            view,
            &geom,
            &quads,
            sel_rect,
            &extra_rects,
            &member_hl,
            &handles,
            &drag_fx,
            &guide_fx,
            &jump_fx,
            &underlays,
            self.tool == Tool::Connector,
            self.tool == Tool::Lasso,
            self.tool == Tool::Freeform,
            armed,
            sel_line_curved,
            &conn_fx,
            &snap_end,
            &snap_corner,
            &hit,
            &mut mouse,
            // While the in-place text editor is open (#22) the canvas
            // is interaction-locked: a plain click ends the edit and
            // nothing else reports (no select/marquee/dbl-click).
            self.text_edit.is_some(),
        );
        self.mouse_page = mouse;
        self.canvas_rect = out.canvas_rect;
        self.conn_fx_last = conn_fx_seen.into_inner();
        if self.text_edit.is_some() {
            // Click anywhere on the canvas = click outside the text:
            // end the edit (commit; the editor decided Esc the same
            // way). No other canvas event is reported while locked.
            if out.click_page_pos.is_some() {
                self.end_text_edit(true);
            }
        } else {
            // An AutoConnect arrow click (#108) consumes the gesture
            // BEFORE the empty-page click semantics run — the arrow
            // sits outside the shape, and a plain click there would
            // clear the selection the AutoConnect needs.
            if let Some(dir) = out.autoclick {
                self.autoconnect(dir);
            }
            if let Some(pos) = out.dbl_click_page_pos {
                // The second click of the pair completes a double-click
                // instead of a plain click: the first click already ran its
                // select (and possible drag-start) semantics on its frame.
                self.on_canvas_dbl_click(pos);
            } else if out.autoclick.is_none() {
                if let Some(pos) = out.click_page_pos {
                    self.on_canvas_click(pos, out.shift_click, out.ctrl_click);
                }
            }
            if let Some(pos) = out.press_page_pos {
                self.on_canvas_press(pos);
            }
            if let Some((x0, y0, x1, y1)) = out.marquee_done {
                self.apply_marquee(x0, y0, x1, y1);
            }
            // The Lasso trail finished (#110): touch/enclose selection.
            if let Some(trail) = out.lasso_done {
                self.apply_lasso(&trail);
            }
            // The Freeform stroke finished (#132): build the shape.
            if let Some(stroke) = out.freeform_done {
                self.create_freeform(stroke);
            }
            // A ruler drag released over the page drops a guide
            // (#113, URS-1076).
            if let Some((src, pos)) = out.guide_drop {
                self.add_guide(src, pos);
            }
            // The Connector Tool's gesture (#108, URS-1067).
            if let Some((from, to)) = out.connector_drop {
                self.create_connector(from, to);
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
                let resp = ui.interact(
                    badge,
                    egui::Id::unique(("dc-comment-badge", target)),
                    Sense::click(),
                );
                resp.widget_info(|| {
                    egui::WidgetInfo::labeled(
                        egui::Role::Button,
                        true,
                        crate::i18n::strs().msg_comments_on_shape_target_count(target, count),
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
                        // #160: snap the resized edge under the snap
                        // settings — the same per-axis snap the live
                        // preview showed. Snap the CORNER position
                        // (handle origin + drag delta), not the raw
                        // pointer: the grab may start up to
                        // HANDLE_GRAB away from the corner, and
                        // snapping the pointer would leave the edge
                        // at grid + grab-offset (CodeRabbit). Applied
                        // AFTER the travel check so a no-travel press
                        // never gains a phantom snap.
                        let dragged = self.sel.map(|(_, s)| s);
                        let origin = handles.iter().find(|h| h.id == id).map(|h| h.pos);
                        let end = match (dragged, origin) {
                            (Some(sid), Some(o)) => {
                                let raw = (o.0 + end.0 - start.0, o.1 + end.1 - start.1);
                                self.snap_corner_for(sid, raw)
                                    .map(|sn| (start.0 + sn.0 - o.0, start.1 + sn.1 - o.1))
                                    .unwrap_or(end)
                            }
                            _ => end,
                        };
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
                        // #108: connectors get the full glue lifecycle —
                        // a drop on a connection point (static) or a
                        // shape (dynamic) writes the corpus formula +
                        // `<Connect>`; a drop on empty page releases
                        // the glue. Plain 1D shapes keep the #109 glue
                        // snap with a note (their glue rides with
                        // connectors-as-masters later).
                        if let Some(origin) = handles.iter().find(|h| h.id == id).map(|h| h.pos) {
                            let raw = (origin.0 + end.0 - start.0, origin.1 + end.1 - start.1);
                            let dragged = self.sel.map(|(_, s)| s);
                            let is_conn = dragged.is_some_and(|sid| {
                                self.ed.as_ref().is_some_and(|ed| {
                                    let page = &ed.doc.pages[self.page_idx];
                                    vsdx::model::find_shape_by_id(&page.shapes, sid)
                                        .and_then(|sh| {
                                            shape_chain(&ed.doc, page.id, sid)
                                                .ok()
                                                .map(|ch| (sh, ch))
                                        })
                                        .is_some_and(|(sh, ch)| {
                                            crate::connectors::is_connector_in(&ed.doc, &ch, sh)
                                        })
                                })
                            });
                            if is_conn {
                                // #179: snap the drop like the plain
                                // 1D path — Visio's endpoint magnetizes
                                // to connection points (and the other
                                // snap targets) on release.
                                let snapped =
                                    dragged.map_or(raw, |sid| self.snap_endpoint_for(sid, raw));
                                self.commit_connector_endpoint(id, snapped);
                            } else {
                                let glue =
                                    dragged.and_then(|sid| self.snap_point_for(sid, raw, true));
                                let (target, note) = match glue {
                                    Some((p, hit)) => (
                                        p,
                                        Some(crate::i18n::strs().msg_endpoint_glued_to(snap_hit_text(hit))),
                                    ),
                                    None => (raw, None),
                                };
                                self.apply_line_endpoint(id, target);
                                if let Some(note) = note {
                                    self.status = note;
                                }
                            }
                        }
                    }
                    HandleId::LineMid => {
                        // #172: connectors translate like any shape —
                        // free ends move plainly, glued ends move through
                        // the GUARD-waived write that KEEPS the glue
                        // formula (the reroute tail then re-attaches
                        // them, Visio's pinned feel). Master instances
                        // count as connectors (ObjType on the master).
                        let is_conn = self.sel.is_some_and(|(_, sid)| {
                            self.ed.as_ref().is_some_and(|ed| {
                                let page = &ed.doc.pages[self.page_idx];
                                vsdx::model::find_shape_by_id(&page.shapes, sid)
                                    .and_then(|sh| {
                                        shape_chain(&ed.doc, page.id, sid).ok().map(|ch| (sh, ch))
                                    })
                                    .is_some_and(|(sh, ch)| {
                                        crate::connectors::is_connector_in(&ed.doc, &ch, sh)
                                    })
                            })
                        });
                        if is_conn {
                            self.apply_connector_body_move(start, end);
                        } else {
                            self.apply_line_move(start, end);
                        }
                    }
                    HandleId::GeoPoint {
                        section_ix,
                        row_ix,
                        cell,
                    } => {
                        // The DELTA from the handle's own position (the
                        // grab may start inside the grab radius).
                        // #109: a vertex drag runs through the snap
                        // engine's point snap (grid, ruler, alignment
                        // targets — no glue; glue is for connectors).
                        if let Some(origin) = handles.iter().find(|h| h.id == id).map(|h| h.pos) {
                            let raw = (origin.0 + end.0 - start.0, origin.1 + end.1 - start.1);
                            let target = self
                                .sel
                                .and_then(|(_, sid)| self.snap_point_for(sid, raw, false))
                                .map_or(raw, |(p, _)| p);
                            self.apply_geo_vertex(section_ix, row_ix, cell, target);
                        }
                    }
                }
            }
        }
        if let Some(pos) = out.right_click_page_pos {
            // Visio: right-click selects the shape under the cursor and
            // opens its context menu (empty page = no shape menu yet).
            // Guides carry no shape context menu (#113): they are
            // page furniture — right-click on one opens nothing.
            let hit = self.shape_hit(pos).and_then(|id| {
                let ed = self.ed()?;
                let page_id = self.page_id()?;
                let page = ed.doc.pages.iter().find(|p| p.id == page_id)?;
                match vsdx::model::find_shape_by_id(&page.shapes, id) {
                    Some(sh) if crate::guides::is_guide(sh) => None,
                    _ => Some(id),
                }
            });
            match hit {
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
                            ui, sess, &ed.doc, page, &geom, &self.view,
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
        self.quick_shapes_menu(ui.ctx());
        self.tab_context_menu(ui.ctx());
    }

    /// The page-tab context menu (#54): Visio's tab affordances —
    /// insert, duplicate, delete (never the last page), rename, and
    /// The page-tab context menu (#54), copying the shipped Visio
    /// reference (owner screenshots 2026-10-24): Insert, Delete, Rename,
    /// Duplicate, then Page Setup (later milestone) and Reorder Pages….
    /// All live items run through the undoable command layer.
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
            MarkBg,
            Reorder,
        }
        let mut act: Option<Act> = None;
        let menu = egui::Popup::new(
            egui::Id::unique("dc-tab-ctx-menu"),
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
            if ctx_item(ui, None, &crate::i18n::strs().msg_insert_page(), "", true) {
                act = Some(Act::Insert);
            }
            if ctx_item(
                ui,
                Some(crate::ribbon::Icon::Delete),
                &crate::i18n::strs().msg_delete_page(),
                "",
                n > 1,
            ) {
                act = Some(Act::Delete);
            }
            if ctx_item(ui, None, &crate::i18n::strs().msg_rename_page(), "", true) {
                act = Some(Act::Rename);
            }
            if ctx_item(ui, None, &crate::i18n::strs().msg_duplicate_page(), "", true) {
                act = Some(Act::Duplicate);
            }
            // #113 remainder: mark a page AS a background (Visio's Page
            // Setup ▸ Page Properties ▸ Type: Background, surfaced on
            // the tab the way Visio surfaces page commands).
            let marked = self
                .ed()
                .and_then(|e| e.doc.pages.get(idx))
                .is_some_and(|p| self.is_background_page(p.id));
            let bg_label = if marked {
                crate::i18n::strs().msg_remove_background_mark()
            } else {
                crate::i18n::strs().msg_set_as_background()
            };
            if ctx_item(ui, None, &bg_label, "", true) {
                act = Some(Act::MarkBg);
            }
            ui.separator();
            ctx_item(ui, None, &crate::i18n::strs().msg_page_setup_menu(), "(later milestone)", false);
            if ctx_item(ui, None, &crate::i18n::strs().msg_reorder_pages_menu(), "", true) {
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
            Act::MarkBg => self.toggle_background_mark(idx),
            Act::Reorder => self.reorder_open = true,
        }
    }

    /// #113 remainder: the page-tab menu's background mark — the
    /// `Background="1"` page attribute (the same attribute the
    /// underlay chain and the Backgrounds gallery read), assigned the
    /// same direct-mutation way `set_back_page` works.
    fn toggle_background_mark(&mut self, idx: usize) {
        let Some(page_id) = self.ed().and_then(|e| e.doc.pages.get(idx)).map(|p| p.id) else {
            return;
        };
        let Some(ed) = self.ed.as_mut() else { return };
        let Some(page) = ed.doc.pages.iter_mut().find(|p| p.id == page_id) else {
            return;
        };
        let marked = page
            .attrs
            .iter()
            .any(|(k, v)| k == "Background" && v == "1");
        page.attrs.retain(|(k, _)| k != "Background");
        let name = page.name.clone();
        if !marked {
            page.attrs.push(("Background".to_string(), "1".to_string()));
        }
        self.dirty = true;
        self.version += 1;
        self.status = if marked {
            crate::i18n::strs().msg_page_name_is_a_foreground_page_a(name)
        } else {
            crate::i18n::strs().msg_page_name_marked_as_a_background(name)
        };
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
                    addr: r
                        .cell("Address")
                        .and_then(|c| c.v.clone())
                        .unwrap_or_default(),
                    sub: r
                        .cell("SubAddress")
                        .and_then(|c| c.v.clone())
                        .unwrap_or_default(),
                    desc: r
                        .cell("Description")
                        .and_then(|c| c.v.clone())
                        .unwrap_or_default(),
                })
                .collect::<Vec<_>>()
        };
        let mut out = collect(std::slice::from_ref(sh));
        if let Some(m) = sh.master.and_then(|id| ed.doc.master(id)) {
            let shadowed: std::collections::HashSet<i64> = out.iter().map(|r| r.ix).collect();
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
                    self.status = crate::i18n::strs().msg_went_to_page(sub);
                    return;
                }
                None => {
                    self.status = crate::i18n::strs().msg_page_not_found(sub);
                    return;
                }
            }
        }
        if address.is_empty() {
            self.status = crate::i18n::strs().msg_the_link_has_no_address();
            return;
        }
        // A stored relative address opens against the document's
        // directory (#71): the link survives the file and its folder
        // moving together.
        let address =
            crate::hyperlink::resolve_address(address, crate::hyperlink::doc_dir(&self.path));
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
        match cmd.arg(&address).spawn() {
            Ok(_) => self.status = crate::i18n::strs().msg_opened_address(address.clone()),
            Err(e) => self.status = crate::i18n::strs().msg_could_not_open_address_e(address, e.to_string()),
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
        self.apply(dc_core::Macro::new(crate::i18n::strs().msg_comment_add_inline(), cmds));
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
        egui::Window::new(crate::i18n::strs().msg_comments())
            .resizable(false)
            .collapsible(false)
            .default_width(320.0)
            .show(ctx, |ui| {
                // #257 item 3: the dialog pass mirrors the window.
                ui.with_layout(dialog_dir(), |ui| {
                if thread.is_empty() {
                    ui.weak(crate::i18n::strs().msg_no_comments_on_this_shape_yet());
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
                            // 2016 header: avatar initial + author + date
                            // (mirrored with the chrome under RTL).
                            row(ui, |ui| {
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
                                        crate::i18n::strs().msg_edit_comment(),
                                    )
                                });
                                row(ui, |ui| {
                                    if ui.button(crate::i18n::strs().msg_save()).clicked() && !buf.trim().is_empty() {
                                        let author = author.clone();
                                        let date = date.clone();
                                        let new_text =
                                            format!("{} — {}: {}", author, date, buf.trim());
                                        // One undo step incl. text-derived
                                        // cache refresh (#147).
                                        let scope = Scope::Shape {
                                            page: page_id,
                                            shape: *ann_id,
                                        };
                                        let doc = self.ed().map(|ed| &ed.doc);
                                        if let Some(doc) = doc {
                                            self.apply(dc::textfunc::text_edit_macro(
                                                doc, &scope, &new_text,
                                            ));
                                        }
                                        edit_done = true;
                                    }
                                    if ui.button(crate::i18n::strs().msg_cancel()).clicked() {
                                        edit_done = true;
                                    }
                                });
                            }
                            if !editing_here {
                                ui.label(egui::RichText::new(cbody).size(12.0));
                                row(ui, |ui| {
                                    if ui.button(crate::i18n::strs().msg_edit()).clicked() {
                                        edit = Some((*ann_id, cbody.clone()));
                                    }
                                    if ui.button(crate::i18n::strs().msg_delete()).clicked() {
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
                            crate::i18n::strs().msg_add_a_comment()
                        } else {
                            crate::i18n::strs().msg_reply()
                        }),
                );
                r.widget_info(|| {
                    egui::WidgetInfo::labeled(
                        egui::Role::TextInput,
                        true,
                        if thread.is_empty() {
                            crate::i18n::strs().msg_review_new_comment()
                        } else {
                            crate::i18n::strs().msg_reply()
                        },
                    )
                });
                row(ui, |ui| {
                    let caption = if thread.is_empty() { crate::i18n::strs().msg_ok() } else { crate::i18n::strs().msg_reply() };
                    if ui.button(caption).clicked() && !body.trim().is_empty() {
                        self.add_comment(shape_id, &body);
                        body.clear();
                    }
                    if ui.button(crate::i18n::strs().msg_close()).clicked() {
                        close = true;
                    }
                });
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
        egui::Window::new(crate::i18n::strs().msg_shape_data())
            .open(&mut open)
            .resizable(false)
            .collapsible(false)
            .default_width(340.0)
            .default_pos(top_right)
            .show(ctx, |ui| {
                // #257 item 3: the dialog pass mirrors the window; the
                // property grid swaps its columns under RTL (field
                // visual-left, label visual-right).
                ui.with_layout(dialog_dir(), |ui| {
                let rtl_grid = crate::i18n::rtl();
                let emit_label = |ui: &mut egui::Ui, label: &str, tip: &str| {
                    ui.label(label).on_hover_text(tip);
                };
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
                            if !rtl_grid {
                                emit_label(ui, &label, &tip);
                            }
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
                                    // The add row (label column empty —
                                    // mirrored with the grid under RTL).
                                    if self.prop_add.as_deref() == Some(name.as_str()) {
                                        if rtl_grid {
                                            // The PROPERTY row's second cell —
                                            // emitted before the row closes, or
                                            // the label would land in the add
                                            // row below it (PR #269 review).
                                            emit_label(ui, &label, &tip);
                                        }
                                        ui.end_row();
                                        if rtl_grid {
                                            // the editor lands visual-left
                                        } else {
                                            ui.label("");
                                        }
                                        let add_key = format!("{name}#add");
                                        let mut commit = false;
                                        let mut cancel = false;
                                        {
                                            let buf = bufs.entry(add_key.clone()).or_default();
                                            ui.horizontal(|ui| {
                                                let edit = ui.add(
                                                    egui::TextEdit::singleline(buf)
                                                        .hint_text(crate::i18n::strs().msg_new_choice()),
                                                );
                                                edit.widget_info(|| {
                                                    egui::WidgetInfo::labeled(
                                                        egui::Role::TextInput,
                                                        true,
                                                        &label,
                                                    )
                                                });
                                                if ui.button(crate::i18n::strs().msg_add()).clicked()
                                                    || (edit.lost_focus()
                                                        && ui.input(|i| {
                                                            i.key_pressed(egui::Key::Enter)
                                                        }))
                                                {
                                                    commit = true;
                                                }
                                                if ui.button(crate::i18n::strs().msg_cancel()).clicked() {
                                                    cancel = true;
                                                }
                                            });
                                        }
                                        if rtl_grid {
                                            ui.label("");
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
                                                self.status =
                                                    crate::i18n::strs().msg_label_a_choice_cannot_contain(label.clone());
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
                                                    let row_sel = dc_core::commands::RowSel::Named(
                                                        name.clone(),
                                                    );
                                                    self.apply(dc_core::Macro::new(
                                                        crate::i18n::strs()
                                                            .msg_undo_add_choice(
                                                                name.clone(),
                                                                format!("{v:?}"),
                                                            ),
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
                                                self.status = crate::i18n::strs().msg_label_not_a_number(label.clone());
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
                            if rtl_grid && self.prop_add.as_deref() != Some(name.as_str()) {
                                // The mirrored row's second cell (the
                                // add-row variant emitted its own above,
                                // before its row closed).
                                emit_label(ui, &label, &tip);
                            }
                            ui.end_row();
                        }
                    });
                // Always-visible action bar: Close bottom-right —
                // #257: mirrored to the bottom-left under RTL.
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    row_rev(ui, |ui| {
                        if ui.button(crate::i18n::strs().msg_close()).clicked() {
                            close_btn = true;
                        }
                    });
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

    /// The Select by Type dialog (#110, URS-1160, Visio's Home ▸
    /// Editing ▸ Select ▸ Select by Type): pick a criterion — shape
    /// type, master, or layer — and a value from the page, and the
    /// matching shapes become the selection.
    fn select_by_type_window(&mut self, ctx: &egui::Context) {
        if !self.selby_open {
            return;
        }
        // The value list per criterion, straight off the page.
        let mut pairs: Vec<(String, String, Vec<i64>)> = Vec::new();
        if let Some(ed) = self.ed.as_ref() {
            if let Some(page) = ed.doc.pages.get(self.page_idx) {
                // (key shown to the user, dedup key, matching ids)
                let mut push = |label: String, ids: Vec<i64>| {
                    if let Some(slot) = pairs.iter_mut().find(|(l, _, _)| *l == label) {
                        slot.2.extend(ids);
                    } else {
                        pairs.push((label.clone(), label, ids));
                    }
                };
                match self.selby_criterion {
                    0 => {
                        for sh in &page.shapes {
                            let t = sh.type_.clone().unwrap_or_else(|| "Shape".into());
                            push(t, vec![sh.id]);
                        }
                    }
                    1 => {
                        for sh in &page.shapes {
                            if let Some(mid) = sh.master {
                                let name = ed
                                    .doc
                                    .master(mid)
                                    .and_then(|m| m.name_u.clone())
                                    .unwrap_or_else(|| crate::i18n::strs().msg_master_mid(mid));
                                push(name, vec![sh.id]);
                            }
                        }
                    }
                    _ => {
                        let layer_names: Vec<(i64, String)> = page
                            .sheet()
                            .map(|sheet| {
                                sheet
                                    .sections_named("Layer")
                                    .into_iter()
                                    .flat_map(|sec| {
                                        sec.rows.iter().map(|row| {
                                            (
                                                row.ix,
                                                row.cell("Name")
                                                    .and_then(|c| c.v.clone())
                                                    .unwrap_or_else(|| crate::i18n::strs().msg_layer_ix_fallback(row.ix)),
                                            )
                                        })
                                    })
                                    .collect()
                            })
                            .unwrap_or_default();
                        for (ix, name) in &layer_names {
                            let ids: Vec<i64> = page
                                .shapes
                                .iter()
                                .filter(|sh| {
                                    sh.sheet
                                        .cell("LayerMember")
                                        .and_then(|c| c.numeric())
                                        .is_some_and(|m| m as i64 == *ix)
                                })
                                .map(|sh| sh.id)
                                .collect();
                            if !ids.is_empty() {
                                push(name.clone(), ids);
                            }
                        }
                    }
                }
                pairs.sort_by(|a, b| a.0.cmp(&b.0));
            }
        }
        if !pairs.iter().any(|(l, _, _)| *l == self.selby_value) {
            self.selby_value = pairs.first().map(|(l, _, _)| l.clone()).unwrap_or_default();
        }
        let mut open = true;
        let mut apply = false;
        egui::Window::new(crate::i18n::strs().msg_select_by_type())
            .open(&mut open)
            .resizable(false)
            .collapsible(false)
            .default_width(320.0)
            .show(ctx, |ui| {
                // #257 item 3: the dialog pass mirrors the window.
                ui.with_layout(dialog_dir(), |ui| {
                row(ui, |ui| {
                    ui.label(crate::i18n::strs().msg_select_by());
                    for (i, label) in [crate::i18n::strs().msg_shape_type(), crate::i18n::strs().msg_master(), crate::i18n::strs().msg_layer()].into_iter().enumerate() {
                        if ui
                            .selectable_label(self.selby_criterion == i as u8, label)
                            .clicked()
                        {
                            self.selby_criterion = i as u8;
                            self.selby_value.clear();
                        }
                    }
                });
                ui.separator();
                if pairs.is_empty() {
                    ui.weak(crate::i18n::strs().msg_nothing_on_this_page_matches_the());
                } else {
                    ui.label(crate::i18n::strs().msg_value());
                    for (label, _, ids) in &pairs {
                        if ui
                            .selectable_label(
                                self.selby_value == *label,
                                crate::i18n::strs().msg_select_value_n_shapes(label.clone(), ids.len() as i64),
                            )
                            .clicked()
                        {
                            self.selby_value = label.clone();
                        }
                    }
                }
                ui.add_space(6.0);
                let can = !self.selby_value.is_empty();
                let btn = egui::Button::new(crate::i18n::strs().msg_select());
                let r = ui.add_enabled(can, btn);
                if r.clicked() {
                    apply = true;
                }
                });
            });
        if apply {
            if let Some((_, _, ids)) = pairs.iter().find(|(l, _, _)| *l == self.selby_value) {
                let ids = ids.clone();
                if let Some(page_id) = self.page_id() {
                    let mut all: Vec<(i64, i64)> = ids.iter().map(|sid| (page_id, *sid)).collect();
                    all.sort();
                    all.dedup();
                    if let Some(first) = all.first().copied() {
                        self.sel = Some(first);
                    }
                    let n = all.len();
                    self.sel_set = all;
                    let crit = match self.selby_criterion {
                        0 => crate::i18n::strs().msg_shape_type(),
                        1 => crate::i18n::strs().msg_master(),
                        _ => crate::i18n::strs().msg_layer(),
                    };
                    self.status = crate::i18n::strs()
                        .msg_selected_n_by_criterion(n as i64, crit);
                }
            }
        }
        if !open {
            self.selby_open = false;
        }
    }

    /// The Snap & Glue dialog (#109, Visio 2016's View ▸ Visual Aids):
    /// per-target toggles for snap (URS-1100) and glue (URS-1112)
    /// plus the dynamic-grid aid (URS-1088). Every change writes
    /// through to the document's DocumentSettings — settings are
    /// document state, not undoable drawing edits.
    fn snap_glue_window(&mut self, ctx: &egui::Context) {
        if !self.snap_dlg_open {
            return;
        }
        let mut open = true;
        let mut changed = false;
        egui::Window::new(crate::i18n::strs().msg_snap_glue())
            .open(&mut open)
            .resizable(false)
            .collapsible(false)
            .default_width(400.0)
            .show(ctx, |ui| {
                // #257 item 3: the dialog pass mirrors the window; the
                // two columns swap (Glue column reads first under RTL).
                ui.with_layout(dialog_dir(), |ui| {
                let s = &mut self.snap;
                let snap_col = |ui: &mut egui::Ui, s: &mut crate::snap::SnapSettings, changed: &mut bool| {
                    ui.vertical(|ui| {
                        ui.strong(crate::i18n::strs().msg_snap());
                        if ui.checkbox(&mut s.snap_active, crate::i18n::strs().msg_active()).changed() {
                            *changed = true;
                        }
                        ui.add_enabled_ui(s.snap_active, |ui| {
                            ui.label(crate::i18n::strs().msg_snap_to());
                            for (v, label) in [
                                (&mut s.snap_ruler_subdivisions, crate::i18n::strs().msg_ruler_subdivisions()),
                                (&mut s.snap_grid, crate::i18n::strs().msg_grid()),
                                (&mut s.snap_guides, crate::i18n::strs().msg_guides()),
                                (&mut s.snap_alignment_boxes, crate::i18n::strs().msg_alignment_boxes()),
                                (&mut s.snap_shape_extensions, crate::i18n::strs().msg_shape_extensions()),
                                (&mut s.snap_shape_intersections, crate::i18n::strs().msg_shape_intersections()),
                                (&mut s.snap_connection_points, crate::i18n::strs().msg_snap_conn_points()),
                            ] {
                                if ui.checkbox(v, label).changed() {
                                    *changed = true;
                                }
                            }
                        });
                    });
                };
                let glue_col = |ui: &mut egui::Ui, s: &mut crate::snap::SnapSettings, changed: &mut bool| {
                    ui.vertical(|ui| {
                        ui.strong(crate::i18n::strs().msg_glue());
                        if ui.checkbox(&mut s.glue_active, crate::i18n::strs().msg_active()).changed() {
                            *changed = true;
                        }
                        ui.add_enabled_ui(s.glue_active, |ui| {
                            ui.label(crate::i18n::strs().msg_glue_to());
                            for (v, label) in [
                                (&mut s.glue_guides, crate::i18n::strs().msg_guides()),
                                (&mut s.glue_shape_handles, crate::i18n::strs().msg_shape_handles()),
                                (&mut s.glue_shape_vertices, crate::i18n::strs().msg_shape_vertices()),
                                (&mut s.glue_connection_points, crate::i18n::strs().msg_snap_conn_points()),
                            ] {
                                if ui.checkbox(v, label).changed() {
                                    *changed = true;
                                }
                            }
                        });
                    });
                };
                // #257: explicit emission order in a plain horizontal
                // (the generic `row` flip reserves too little wrap
                // width for the vertical columns under RTL).
                ui.horizontal(|ui| {
                    if crate::i18n::rtl() {
                        glue_col(ui, s, &mut changed);
                        ui.separator();
                        snap_col(ui, s, &mut changed);
                    } else {
                        snap_col(ui, s, &mut changed);
                        ui.separator();
                        glue_col(ui, s, &mut changed);
                    }
                });
                ui.separator();
                ui.strong(crate::i18n::strs().msg_drawing_aids());
                if ui.checkbox(&mut s.dynamic_grid, crate::i18n::strs().msg_snap_dynamic_grid()).changed() {
                    changed = true;
                }
                ui.add_space(3.0);
                ui.weak(crate::i18n::strs().msg_snapglue_note());
                });
            });
        if !open {
            self.snap_dlg_open = false;
        }
        if changed {
            self.apply_snap_settings();
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
        // #71: the 2016 dialog's storage toggle. Visio keeps it
        // app-level (observed); ours persists app-side keyed by the
        // document's path — never inside the .vsdx, whose
        // DocumentSettings schema is closed.
        let mut relative_paths = self.relative_paths.get(&self.path);
        let doc_dir = crate::hyperlink::doc_dir(&self.path).map(|p| p.to_path_buf());
        // Page names for the Sub-address picker (#71): a SubAddress
        // names a page (the follow path), so the dropdown lists them.
        let page_names: Vec<String> = self
            .ed()
            .map(|e| e.doc.pages.iter().map(|p| p.name.clone()).collect())
            .unwrap_or_default();
        // Cancel mechanism (live report + the 2016 dialog's Cancel):
        // the title-bar X and an explicit Close button both discard
        // any draft and dismiss the window.
        let mut open = true;
        // The Close BUTTON flips this (the X writes `open` via the
        // builder; the two can't share a borrow). Esc-no-draft does
        // too (#110 audit).
        let mut close_btn = false;
        let mut esc_close = false;
        egui::Window::new(crate::i18n::strs().msg_hyperlinks())
            .open(&mut open)
            .resizable(false)
            .collapsible(false)
            .default_width(380.0)
            .show(ctx, |ui| {
                // #110 audit: this window manages its own Esc — a
                // draft discards first (a window close on the same key
                // would lose the layering); Enter commits the draft
                // (the dialog's default OK button). Skipped while the
                // in-place text editor owns the keyboard (#22).
                if self.text_edit.is_none()
                    && ui.input(|i| i.key_pressed(egui::Key::Escape))
                {
                    if draft.is_some() {
                        draft = None;
                    } else {
                        esc_close = true;
                    }
                }
                // The 2016 storage toggle (#71) sits above the list.
                if ui
                    .checkbox(&mut relative_paths, crate::i18n::strs().msg_use_relative_paths_for_hyperlink())
                    .changed()
                {
                    self.relative_paths.set(&self.path, relative_paths);
                }
                if rows.is_empty() && draft.is_none() {
                    ui.weak(crate::i18n::strs().msg_no_hyperlinks_on_this_shape());
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
                            ui.weak(crate::i18n::strs().msg_inherited());
                        }
                        if ui.button(crate::i18n::strs().msg_go()).clicked() {
                            let (a, s) = (row.addr.clone(), row.sub.clone());
                            self.follow_hyperlink(&a, &s);
                        }
                        if ui.button(crate::i18n::strs().msg_edit()).clicked() {
                            draft = Some(HyperlinkDraft {
                                edit_row: Some(row.ix),
                                from_master: row.inherited,
                                address: row.addr.clone(),
                                sub: row.sub.clone(),
                                desc: row.desc.clone(),
                            });
                        }
                        let remove = ui.add_enabled(!row.inherited, egui::Button::new(crate::i18n::strs().msg_remove()));
                        if row.inherited {
                            remove.on_disabled_hover_text(
                                crate::i18n::strs().msg_inherited_from_the_shape_s_maste(),
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
                    // #257: the RTL grid mirrors its rows — the label
                    // cell lands last (visual right), the browse/combo
                    // control first (visual left).
                    let rtl_grid = crate::i18n::rtl();
                    let input_cell = |ui: &mut Ui, label: &str, buf: &mut String| {
                        let r = ui.add(egui::TextEdit::singleline(buf).hint_text(label));
                        r.widget_info(|| {
                            egui::WidgetInfo::labeled(egui::Role::TextInput, true, label)
                        });
                    };
                    let field = |ui: &mut Ui, label: &str, buf: &mut String| {
                        ui.label(label);
                        input_cell(ui, label, buf);
                    };
                    egui::Grid::new("dc-hyperlink-draft")
                        .num_columns(3)
                        .spacing(egui::vec2(8.0, 4.0))
                        .show(ui, |ui| {
                            if rtl_grid {
                                // Browse… (#71) first (visual left).
                                if ui.button(crate::i18n::strs().msg_browse()).clicked() {
                                    self.begin_dialog(PendingKind::HyperlinkAddress);
                                }
                                input_cell(ui, crate::i18n::strs().msg_hl_address().as_str(), &mut d.address);
                                ui.label(crate::i18n::strs().msg_hl_address());
                                ui.end_row();
                            } else {
                            field(ui, crate::i18n::strs().msg_hl_address().as_str(), &mut d.address);
                            // Browse… (#71): the native file picker (the
                            // Open/Save As worker-thread pattern); the
                            // pick lands in the Address field once the
                            // dialog resolves.
                            if ui.button(crate::i18n::strs().msg_browse()).clicked() {
                                self.begin_dialog(PendingKind::HyperlinkAddress);
                            }
                            ui.end_row();
                            }
                            if rtl_grid {
                                let combo = egui::ComboBox::from_id_salt("dc-hyperlink-sub-page")
                                    .selected_text(if d.sub.is_empty() {
                                        crate::i18n::strs().msg_hyperlink_pages()
                                    } else {
                                        d.sub.clone()
                                    })
                                    .width(110.0)
                                    .show_ui(ui, |ui| {
                                        for name in &page_names {
                                            ui.selectable_value(&mut d.sub, name.clone(), name);
                                        }
                                    });
                                combo.response.widget_info(|| {
                                    egui::WidgetInfo::labeled(
                                        egui::Role::ComboBox,
                                        true,
                                        crate::i18n::strs().msg_sub_address_page(),
                                    )
                                });
                                input_cell(ui, crate::i18n::strs().msg_hl_subaddress().as_str(), &mut d.sub);
                                ui.label(crate::i18n::strs().msg_hl_subaddress());
                                ui.end_row();
                                input_cell(ui, crate::i18n::strs().msg_hl_description().as_str(), &mut d.desc);
                                ui.label(crate::i18n::strs().msg_hl_description());
                                ui.end_row();
                            } else {
                            field(ui, crate::i18n::strs().msg_hl_subaddress().as_str(), &mut d.sub);
                            // Page picker (#71): a SubAddress names a
                            // page — the dropdown offers the document's.
                            // The accessible name satisfies the dialog
                            // label requirement (#56 pattern).
                            let combo = egui::ComboBox::from_id_salt("dc-hyperlink-sub-page")
                                .selected_text(if d.sub.is_empty() {
                                    crate::i18n::strs().msg_hyperlink_pages()
                                } else {
                                    d.sub.clone()
                                })
                                .width(110.0)
                                .show_ui(ui, |ui| {
                                    for name in &page_names {
                                        ui.selectable_value(&mut d.sub, name.clone(), name);
                                    }
                                });
                            combo.response.widget_info(|| {
                                egui::WidgetInfo::labeled(
                                    egui::Role::ComboBox,
                                    true,
                                    crate::i18n::strs().msg_sub_address_page(),
                                )
                            });
                            ui.end_row();
                            field(ui, crate::i18n::strs().msg_hl_description().as_str(), &mut d.desc);
                            ui.end_row();
                            }
                        });
                    let (edit_row, from_master, address, sub, desc) = (
                        d.edit_row,
                        d.from_master,
                        d.address.clone(),
                        d.sub.clone(),
                        d.desc.clone(),
                    );
                    // Commit stores the address the way the toggle
                    // wants it (#71): inside the document's directory
                    // and relative-paths on → relative form. Picked
                    // paths already arrived relativized; this covers
                    // hand-typed ones too (idempotent).
                    let address = crate::hyperlink::store_address(
                        &address,
                        doc_dir.as_deref(),
                        relative_paths,
                    );
                    let mut close_draft = false;
                    // OK / Apply / Cancel (#71): Apply commits without
                    // closing the draft — the 2016 triad. One undo per
                    // commit either way.
                    // Returns the committed row ix (None when the
                    // draft was all-empty and wrote nothing) — Apply
                    // feeds it back so a second commit EDITS the row
                    // instead of duplicating it (CodeRabbit).
                    let commit = |self_: &mut Self| -> Option<i64> {
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
                                cmds.push(Box::new(dc_core::commands::RemoveSectionRow::new(
                                    scope.clone(),
                                    "Hyperlink",
                                    0,
                                    dc_core::commands::RowSel::Ix(old),
                                )));
                            }
                        }
                        let all_empty = address.trim().is_empty()
                            && sub.trim().is_empty()
                            && desc.trim().is_empty();
                        for (name, val) in [
                            ("Address", &address),
                            ("SubAddress", &sub),
                            ("Description", &desc),
                        ] {
                            // A materializing or REPLACING edit writes
                            // every field, cleared ones as explicit
                            // empty values: the materialized row is
                            // seeded with the inherited cells, so a
                            // skipped field would silently keep its
                            // master value (#139 review; #196 extended
                            // the seeding to the remove-then-rewrite
                            // path, where it resurrects cleared fields
                            // the same way — CodeRabbit on the #239
                            // batch). An all-empty draft keeps the pure
                            // row deletion (no write, nothing
                            // re-seeds); a brand-new row keeps the
                            // non-empty-only writes.
                            if from_master
                                || !val.trim().is_empty()
                                || (edit_row.is_some() && !all_empty)
                            {
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
                        if cmds.is_empty() {
                            return None;
                        }
                        self_.apply(dc_core::Macro::new(crate::i18n::strs().msg_edit_hyperlink(), cmds));
                        Some(ix)
                    };
                    row(ui, |ui| {
                        if ui.button(crate::i18n::strs().msg_ok()).clicked() {
                            commit(self);
                            close_draft = true;
                        }
                        if ui.button(crate::i18n::strs().msg_apply()).clicked() {
                            if let Some(ix) = commit(self) {
                                // The draft now edits the row it just
                                // created (a second Apply updates it,
                                // never duplicates).
                                d.edit_row = Some(ix);
                                d.from_master = false;
                            }
                        }
                        if ui.button(crate::i18n::strs().msg_cancel()).clicked() {
                            close_draft = true;
                        }
                    });
                    // #110 audit: Enter is the dialog's OK (Visio's
                    // default button) — commits and closes the draft.
                    if ui.input(|i| {
                        i.key_pressed(egui::Key::Enter) && !i.modifiers.shift
                    }) {
                        commit(self);
                        close_draft = true;
                    }
                    if close_draft {
                        draft = None;
                    }
                }
                // The window's action bar (always visible — elementary/
                // Apple HIG: the dismiss control must be obvious, not
                // hidden behind editor state): New… left, Close right.
                ui.add_space(4.0);
                row(ui, |ui| {
                    if draft.is_none() && ui.button(crate::i18n::strs().msg_hyperlink_new()).clicked() {
                        draft = Some(HyperlinkDraft {
                            edit_row: None,
                            from_master: false,
                            address: String::new(),
                            sub: String::new(),
                            desc: String::new(),
                        });
                    }
                    // #257: the Close pin mirrors with the chrome.
                    row_rev(ui, |ui| {
                        if ui.button(crate::i18n::strs().msg_close()).clicked() {
                            draft = None;
                            close_btn = true;
                        }
                    });
                });
            });
        if open && !close_btn && !esc_close {
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
        let mut open = true;
        egui::Window::new(crate::i18n::strs().msg_reorder_pages())
            // #110 tab-order audit: the title-bar close X — the window
            // was the one dialog without a keyboard-reachable dismiss
            // besides tabbing to Close (Esc arrives through
            // `close_front_dialog`).
            .open(&mut open)
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
                        // #257: the RTL row puts the mover buttons on
                        // the visual left, the page name on the right
                        // (the LTR row keeps Visio's name first).
                        let rtl_grid = crate::i18n::rtl();
                        for (i, name) in names.iter().enumerate() {
                            let mut up_clicked = false;
                            let mut down_clicked = false;
                            let name_cell = |ui: &mut egui::Ui, name: &str| {
                                ui.label(egui::RichText::new(name.to_string()).size(12.0));
                            };
                            if rtl_grid {
                                if ui
                                    .add_enabled(i > 0, egui::Button::new(crate::i18n::strs().msg_move_up()).small())
                                    .clicked()
                                {
                                    up_clicked = true;
                                }
                                if ui
                                    .add_enabled(i + 1 < n, egui::Button::new(crate::i18n::strs().msg_move_down()).small())
                                    .clicked()
                                {
                                    down_clicked = true;
                                }
                                name_cell(ui, name);
                            } else {
                                name_cell(ui, name);
                                if ui
                                    .add_enabled(i > 0, egui::Button::new(crate::i18n::strs().msg_move_up()).small())
                                    .clicked()
                                {
                                    up_clicked = true;
                                }
                                if ui
                                    .add_enabled(i + 1 < n, egui::Button::new(crate::i18n::strs().msg_move_down()).small())
                                    .clicked()
                                {
                                    down_clicked = true;
                                }
                            }
                            if up_clicked {
                                self.tab_cmd_reorder(i, i - 1);
                            }
                            if down_clicked {
                                self.tab_cmd_reorder(i, i + 1);
                            }
                            ui.end_row();
                        }
                    });
                ui.add_space(8.0);
                row_rev(ui, |ui| {
                    if ui.button(crate::i18n::strs().msg_close()).clicked() {
                        close = true;
                    }
                });
            });
        if close || !open {
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
                self.status = crate::i18n::strs()
                    .msg_action_display_only(format!("{:?}", row.menu));
            }
            Ok(cmds) => {
                let label = format!("run shape action {:?}", row.menu);
                self.apply(dc_core::Macro::new(label, cmds));
            }
            Err(why) => {
                self.status = crate::i18n::strs().msg_action_deferred(
                    format!("{:?}", row.menu),
                    why.clone(),
                );
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
            self.apply(Macro::new(crate::i18n::strs().msg_font_size(), cmds));
        }
    }

    /// Rotate every selected shape ±90° (degrees, Visio CCW positive).
    /// #252 Container Tools ▸ Membership: disband the selected
    /// container — every member's `msvSDContainerShape` reference is
    /// released and the container shape removed, in one undo step.
    fn disband_container(&mut self) {
        let Some(page_id) = self.page_id() else { return };
        let Some(cont) = self.sel_container_id() else {
            self.status = crate::i18n::strs().msg_select_a_container_to_disband();
            return;
        };
        let members = self.container_member_ids(page_id, cont);
        // The same cleanup delete_one runs (#218 review): the
        // membership ROWS come out (#138 treats leftovers as
        // dangling), and Connects pointing at the container go with
        // it — writing empty Values would leave both in the file.
        let mut cmds: Vec<Box<dyn dc_core::Command>> =
            vec![Box::new(dc_core::commands::RemoveShape::new(page_id, cont))];
        if let Some(ed) = self.ed() {
            cmds.extend(dc_core::container_membership_cleanup(&ed.doc, page_id, &[cont]));
        }
        cmds.extend(self.connect_cleanup_cmds(page_id, &[cont]));
        self.apply(dc_core::Macro::new(
            crate::i18n::strs().msg_undo_disband_container(members.len() as i64),
            cmds,
        ));
        self.set_selection(None);
    }

    /// The selected container's id, if the selection is a container
    /// (membership points at it — the same test the contextual tab
    /// uses).
    fn sel_container_id(&self) -> Option<i64> {
        let page_id = self.page_id()?;
        let (_, sid) = self.sel?;
        let has_members = !self.container_member_ids(page_id, sid).is_empty();
        if has_members || self.sel_is_container() {
            Some(sid)
        } else {
            None
        }
    }

    /// #252 Home ▸ Arrange: group the current multi-selection — the
    /// same command the ctx menu fires, including #95's
    /// group-into-group nesting.
    fn group_selection(&mut self) {
        let Some(page_id) = self.page_id() else { return };
        let members: Vec<i64> = self
            .sel_set
            .iter()
            .filter(|(p, _)| *p == page_id)
            .map(|(_, sid)| *sid)
            .collect();
        if members.len() < 2 {
            self.status = crate::i18n::strs().msg_group_needs_2_shapes_on_the_page();
            return;
        }
        let new_id = self
            .ed()
            .map(|ed| dc_core::commands::next_shape_id(&ed.doc, page_id));
        let nest = self.nest_group_target(&members);
        let cmd = match nest {
            Some((parent, local_pin)) => {
                let nested: Vec<i64> =
                    members.iter().copied().filter(|id| *id != parent).collect();
                dc_core::commands::GroupShapes::into_group(
                    page_id, nested, parent, local_pin,
                )
            }
            None => dc_core::commands::GroupShapes::new(page_id, members),
        };
        self.apply(cmd);
        if let Some(nid) = new_id {
            self.set_selection(Some((page_id, nid)));
        }
    }

    /// #252 Home ▸ Arrange: ungroup every selected group — one Macro
    /// (one undo step); the first ex-member becomes the selection.
    fn ungroup_selection(&mut self) {
        let Some(page_id) = self.page_id() else { return };
        let Some(ed) = self.ed.as_ref() else { return };
        let Some(page) = ed.doc.pages.iter().find(|p| p.id == page_id) else {
            return;
        };
        let groups: Vec<(i64, Option<i64>)> = self
            .sel_set
            .iter()
            .filter(|(p, _)| *p == page_id)
            .map(|(_, sid)| {
                let first_member = vsdx::model::find_shape_by_id(&page.shapes, *sid)
                    .and_then(|g| g.sub_shapes.first().map(|m| m.id));
                (*sid, first_member)
            })
            .collect();
        let cmds: Vec<Box<dyn dc_core::Command>> = groups
            .iter()
            .filter(|(_, m)| m.is_some())
            .map(|(sid, _)| Box::new(dc_core::commands::UngroupShape::new(page_id, *sid)) as _)
            .collect();
        if cmds.is_empty() {
            self.status = crate::i18n::strs().msg_select_a_group_to_ungroup();
            return;
        }
        let n = cmds.len();
        self.apply(dc_core::Macro::new(crate::i18n::strs().msg_undo_ungroup_n(n), cmds));
        // The selection becomes the first EX-MEMBER of a group that
        // was actually ungrouped — `groups.first()` can be a plain
        // shape (None member), which left sel_set holding the removed
        // group ids and rolled back a following Delete (#218 review
        // round). Reset either way: the old ids are gone.
        let first = groups.iter().find_map(|(_, m)| *m);
        self.set_selection(first.map(|m| (page_id, m)));
    }

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
            self.apply(Macro::new(crate::i18n::strs().msg_rotate_shapes(), cmds));
        }
    }

    /// Apply a picked color to FillForegnd or LineColor on the selection.
    fn apply_color(&mut self, target: ColorTarget, color: &str) {
        // Text color lives in the Character section's row 0 (#73): the
        // whole-selection SetRowCell macro the pane's other text
        // fields use (first-edit materialization seeds the row with
        // the inherited cells).
        if matches!(target, ColorTarget::Text) {
            self.fmt_set_row("Character", "Color", color);
            return;
        }
        let cell_name = match target {
            ColorTarget::Fill => "FillForegnd",
            ColorTarget::FillBkgnd => "FillBkgnd",
            ColorTarget::Line => "LineColor",
            // Handled by the SetRowCell path above.
            ColorTarget::Text => unreachable!("text color writes a row cell"),
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

    /// The Quick Shapes membership menu (#143), opened by
    /// right-clicking a master cell — in its source entry or in Quick
    /// Shapes (a quick cell's key IS its source key). One item whose
    /// label follows live membership; Visio persists membership
    /// app-side and so does this (never the .vsdx, #88's rule).
    fn quick_shapes_menu(&mut self, ctx: &egui::Context) {
        let Some(((src_id, master_id), anchor)) = self.quick_menu else {
            return;
        };
        let identity = crate::stencil::quick_source_id(src_id, &self.stencils);
        let in_default = match src_id {
            StencilId::Document => self
                .ed
                .as_ref()
                .map(|e| crate::stencil::quick_default_ids(&e.doc).contains(&master_id)),
            StencilId::Open(ix) => self
                .stencils
                .get(ix)
                .map(|st| crate::stencil::quick_default_ids(&st.doc).contains(&master_id)),
        };
        // A closed source whose pane slot shifted has no stable
        // identity for the menu — drop it.
        let (Some(identity), Some(in_default)) = (identity, in_default) else {
            self.quick_menu = None;
            return;
        };
        let member = self
            .quick_shapes
            .is_member(&identity, master_id, in_default);
        let mut act: Option<bool> = None; // true = add, false = remove
        let menu = egui::Popup::new(
            egui::Id::unique("dc-quick-shapes-menu"),
            ctx.clone(),
            anchor,
            egui::LayerId::background(),
        )
        .kind(egui::PopupKind::Menu)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside);
        let popup = menu.show(|ui| {
            ui.set_min_width(214.0);
            let label = if member {
                crate::i18n::strs().msg_remove_from_quick_shapes()
            } else {
                crate::i18n::strs().msg_add_to_quick_shapes()
            };
            if ui.button(label).clicked() {
                act = Some(!member);
            }
        });
        if let Some(add) = act {
            if add {
                self.quick_shapes.add(&identity, master_id);
                self.status = crate::i18n::strs().msg_added_to_quick_shapes();
            } else {
                self.quick_shapes.remove(&identity, master_id);
                self.status = crate::i18n::strs().msg_removed_from_quick_shapes();
            }
            self.quick_menu = None;
            return;
        }
        // Close on an outside PRIMARY click (the shape menu's
        // idiom): the opening right-click's own release is a
        // secondary click and must not dismiss the menu it opened.
        let outside_primary = ctx.input(|i| i.pointer.primary_clicked());
        if let Some(ir) = &popup {
            if outside_primary
                && !ir
                    .response
                    .rect
                    .contains(ctx.input(|i| i.pointer.interact_pos()).unwrap_or_default())
            {
                self.quick_menu = None;
            }
        }
    }

    fn shape_context_menu(&mut self, ctx: &egui::Context) {
        let Some((shape_id, pointer_anchor, _page_pos)) = self.ctx_menu else {
            self.ctx_menu_rect = None;
            return;
        };
        let has_clip = self.clipboard.is_some();
        // #75: the underlying-container membership item enables when a
        // container sits under the selection AND the selection is not
        // already entirely its member set (a pure no-op stays gray).
        let sel_members: Vec<i64> = self
            .sel_set
            .iter()
            .filter(|(p, _)| Some(p) == self.page_id().as_ref())
            .map(|(_, s)| *s)
            .collect();
        let underlying = self.underlying_container(&sel_members);
        let can_underlying = match underlying {
            Some(c) => {
                let current = self.container_member_ids(self.page_id().unwrap_or(-1), c);
                // Enables only when a selected shape is not yet a
                // member — a pure re-add is a no-op and stays gray
                // (#75 branch review).
                sel_members.iter().any(|m| !current.contains(m))
            }
            None => false,
        };
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
            /// #75: membership into the container under the selection.
            AddToUnderlying,
            Hyperlink,
            AddComment,
            ShapeData,
            FormatShape,
            /// #149 item 2: the mini-toolbar's three Format icons open
            /// the pane focused on their section.
            FormatFill,
            FormatLine,
            FormatText,
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
            /// #108: per-connector route style (URS-1114).
            RouteStraight,
            RouteRightAngle,
            RouteCurved,
            /// #113/URS-1200: render the selection to a picture.
            CopyAsPicture,
            /// #257 text-direction: toggle the shape's paragraph base
            /// direction (explicit U+202B/U+202A marks — VSDX has no
            /// RTLText cell; Analysis/rtl-text-direction.md).
            ToggleRtlText,
        }
        let mut act: Option<Act> = None;
        // Min menu width incl. frame, until the popup reports its own
        // measured size (used only for the right-edge flip decision).
        const MENU_MIN_W: f32 = 244.0;
        let mut menu = egui::Popup::new(
            egui::Id::unique("dc-shape-ctx-menu"), // stable per-menu id is intended
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
        // edge is too close (issue #49). #257: an RTL chrome mirrors
        // the anchor — the menu opens to the LEFT of the shape when the
        // left screen edge has room, right edge otherwise. Anchoring to
        // the shape's live rect keeps the menu glued to it through
        // zoom/pan; if the rect cannot be resolved, fall back to the
        // pointer position like a classic context menu (egui defaults).
        // Alternatives stay empty so the corner anchor is deterministic.
        if let Some(r) = self.shape_screen_rect(shape_id) {
            let menu_w = menu
                .get_expected_size()
                .map_or(MENU_MIN_W, |s| s.x.max(MENU_MIN_W));
            // The popup Area is constrained into the content rect, so
            // measure room against the same rect.
            let rtl = crate::i18n::rtl();
            let room_preferred = if rtl {
                r.left() - ctx.content_rect().left() >= menu_w
            } else {
                ctx.content_rect().right() - r.right() >= menu_w
            };
            let (preferred, other) = if rtl {
                (egui::RectAlign::LEFT_START, egui::RectAlign::RIGHT_START)
            } else {
                (egui::RectAlign::RIGHT_START, egui::RectAlign::LEFT_START)
            };
            menu = menu
                .anchor(r)
                .gap(2.0)
                .align(if room_preferred { preferred } else { other })
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
            // The mini-toolbar (#77 landed a flat icon bar; #149
            // restyles it to the real Visio 2010/2016 menu): a
            // two-row ribbon-chrome panel above the text rows, with
            // the Flip flyout (#149 item 1) and the section-targeted
            // Format icons (#149 item 2). The seven #77 commands stay
            // the working set until #149 item 3's oracle pass
            // confirms the real slot map.
            {
                let can_group = self.sel_set.len() >= 2;
                if let Some(bar) = crate::mini_toolbar::show(ui, can_group) {
                    act = Some(match bar {
                        crate::mini_toolbar::Act::EditText => Act::EditText,
                        crate::mini_toolbar::Act::FormatFill => Act::FormatFill,
                        crate::mini_toolbar::Act::FormatLine => Act::FormatLine,
                        crate::mini_toolbar::Act::FormatText => Act::FormatText,
                        crate::mini_toolbar::Act::Front => Act::Front,
                        // The text row's sign: page coordinates are
                        // y-up, so clockwise (right) is NEGATIVE
                        // radians (#77 branch review — the bar had
                        // it backwards).
                        crate::mini_toolbar::Act::RotateRight => {
                            Act::Rot(-std::f64::consts::FRAC_PI_2)
                        }
                        crate::mini_toolbar::Act::FlipVertical => Act::FlipV,
                        crate::mini_toolbar::Act::FlipHorizontal => Act::FlipH,
                        crate::mini_toolbar::Act::Group => Act::Group,
                        // The Styles tile's stand-in until a styles
                        // feature: the pane, generically.
                        crate::mini_toolbar::Act::Styles => Act::FormatShape,
                    });
                }
                ui.add_space(2.0);
            }
            // Visio order: master actions, Show ShapeSheet, clipboard,
            // object commands, then the grayed future milestones.
            // Actions rows are live (#25): evaluated checkmarks and
            // disabled state, BeginGroup separators, FlyoutChild
            // indentation, and SETF execution on click.
            for (ai, row) in actions.iter().enumerate() {
                if row.begin_group && ai > 0 {
                    ui.separator();
                }
                // Flyout arrow: › (the pinned font set has no ▸);
                // #257: the RTL menu's flyouts open leftward, so the
                // arrow mirrors with the chrome.
                let label = if row.flyout_child {
                    format!("     {}", row.menu)
                } else if crate::i18n::rtl() {
                    format!("‹  {}", row.menu)
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
                &crate::i18n::strs().msg_show_shapesheet(),
                "",
                true,
            ) {
                act = Some(Act::ShapeSheet);
            }
            ui.separator();
            if ctx_item(ui, Some(crate::ribbon::Icon::Cut), &crate::i18n::strs().msg_cut(), "Ctrl+X", true) {
                act = Some(Act::Cut);
            }
            if ctx_item(ui, Some(crate::ribbon::Icon::Copy), &crate::i18n::strs().msg_copy(), "Ctrl+C", true) {
                act = Some(Act::Copy);
            }
            if ctx_item(
                ui,
                Some(crate::ribbon::Icon::Paste),
                &crate::i18n::strs().msg_paste(),
                "Ctrl+V",
                has_clip,
            ) {
                act = Some(Act::Paste);
            }
            if ctx_item(ui, None, &crate::i18n::strs().msg_duplicate(), "Ctrl+D", true) {
                act = Some(Act::Duplicate);
            }
            if ctx_item(ui, Some(crate::ribbon::Icon::Delete), &crate::i18n::strs().msg_delete(), "Del", true) {
                act = Some(Act::Delete);
            }
            ui.separator();
            if ctx_item(
                ui,
                Some(crate::ribbon::Icon::BringFront),
                &crate::i18n::strs().msg_bring_to_front(),
                "",
                true,
            ) {
                act = Some(Act::Front);
            }
            if ctx_item(
                ui,
                Some(crate::ribbon::Icon::SendBack),
                &crate::i18n::strs().msg_send_to_back(),
                "",
                true,
            ) {
                act = Some(Act::Back);
            }
            ui.separator();
            if ctx_item(
                ui,
                Some(crate::ribbon::Icon::Rotate),
                &crate::i18n::strs().msg_rotate_right_90(),
                "",
                true,
            ) {
                act = Some(Act::Rot(-std::f64::consts::FRAC_PI_2));
            }
            if ctx_item(
                ui,
                Some(crate::ribbon::Icon::Rotate),
                &crate::i18n::strs().msg_rotate_left_90(),
                "",
                true,
            ) {
                act = Some(Act::Rot(std::f64::consts::FRAC_PI_2));
            }
            if ctx_item(ui, None, &crate::i18n::strs().msg_flip_vertical(), "", true) {
                act = Some(Act::FlipV);
            }
            if ctx_item(ui, None, &crate::i18n::strs().msg_flip_horizontal(), "", true) {
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
            // #257: the RTL row right-aligns (halign on the job) and
            // its flyout arrow opens leftward; the same mirrored row
            // routes the label through the catalog.
            let mut group_job = egui::text::LayoutJob {
                halign: if crate::i18n::rtl() {
                    egui::Align::RIGHT
                } else {
                    egui::Align::LEFT
                },
                ..egui::text::LayoutJob::default()
            };
            group_job.append(
                &crate::i18n::strs().msg_group_flyout(),
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
                    if ctx_item(ui, None, &crate::i18n::strs().msg_group(), "Ctrl+G", can_group) {
                        act = Some(Act::Group);
                    }
                    if ctx_item(ui, None, &crate::i18n::strs().msg_ungroup(), "Ctrl+Shift+G", is_group) {
                        act = Some(Act::Ungroup);
                    }
                    if ctx_item(ui, None, &crate::i18n::strs().msg_open_group_label(group_label), "", is_group) {
                        act = Some(Act::OpenGroup(shape_id));
                    }
                });
            // Container is a flyout, not an action (Visio 2016): the
            // parent row only opens the submenu. Anchor: draw the row
            // directly so we own its Response for Popup::menu.
            let mut job = egui::text::LayoutJob {
                halign: if crate::i18n::rtl() {
                    egui::Align::RIGHT
                } else {
                    egui::Align::LEFT
                },
                ..egui::text::LayoutJob::default()
            };
            job.append(
                &crate::i18n::strs().msg_container_flyout(),
                0.0,
                egui::TextFormat::simple(egui::FontId::proportional(12.5), tokens.label),
            );
            let cont = ui.add_enabled(
                !self.sel_set.is_empty(),
                egui::Button::new(job).min_size(Vec2::new(ui.available_width().max(150.0), 21.0)),
            );
            let mut sub_action = false;
            let mut underlying_action = false;
            egui::Popup::menu(&cont)
                .close_behavior(egui::PopupCloseBehavior::CloseOnClick)
                .show(|ui| {
                    ui.set_min_width(210.0);
                    if ctx_item(ui, None, &crate::i18n::strs().msg_add_to_new_container(), "", true) {
                        sub_action = true;
                    }
                    if ctx_item(ui, None, &crate::i18n::strs().msg_add_to_underlying_container(), "", can_underlying)
                        && can_underlying
                    {
                        underlying_action = true;
                    }
                });
            if sub_action {
                act = Some(Act::Container);
            }
            if underlying_action {
                act = Some(Act::AddToUnderlying);
            }
            // #108: connector route styles (Visio 2016 puts
            // Straight/Right-Angle Connector on the connector's menu).
            {
                let is_conn = self.page_id().is_some_and(|pid| {
                    self.ed.as_ref().is_some_and(|ed| {
                        let page = ed.doc.pages.iter().find(|p| p.id == pid);
                        page.and_then(|p| vsdx::model::find_shape_by_id(&p.shapes, shape_id))
                            .is_some_and(crate::connectors::is_connector)
                    })
                });
                if is_conn {
                    if ctx_item(ui, None, &crate::i18n::strs().msg_straight_connector(), "", true) {
                        act = Some(Act::RouteStraight);
                    }
                    if ctx_item(ui, None, &crate::i18n::strs().msg_right_angle_connector(), "", true) {
                        act = Some(Act::RouteRightAngle);
                    }
                    if ctx_item(ui, None, &crate::i18n::strs().msg_curved_connector(), "", true) {
                        act = Some(Act::RouteCurved);
                    }
                    ui.separator();
                }
            }
            if ctx_item(ui, None, &crate::i18n::strs().msg_copy_as_picture_menu(), "", self.sel.is_some()) {
                act = Some(Act::CopyAsPicture);
            }
            if ctx_item(ui, None, &crate::i18n::strs().msg_hyperlink_menu(), "Ctrl+K", true) {
                act = Some(Act::Hyperlink);
            }
            if ctx_item(ui, None, &crate::i18n::strs().msg_comment_add(), "", true) {
                act = Some(Act::AddComment);
            }
            if ctx_item(ui, None, &crate::i18n::strs().msg_shape_data_menu(), "", true) {
                act = Some(Act::ShapeData);
            }
            if ctx_item(ui, None, &crate::i18n::strs().msg_format_shape_menu(), "", true) {
                act = Some(Act::FormatShape);
            }
            ui.separator();
            if ctx_item(ui, None, &crate::i18n::strs().msg_edit_text(), "F2", true) {
                act = Some(Act::EditText);
            }
            if ctx_item(ui, None, &crate::i18n::strs().msg_rtl_text_menu(), "", true) {
                act = Some(Act::ToggleRtlText);
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
                // #252: shared with Home ▸ Arrange's Group button.
                self.group_selection();
            }
            Act::Container => self.wrap_selection_in_container(),
            Act::AddToUnderlying => self.add_selection_to_underlying_container(),
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
                self.format_pane_focus = None;
            }
            // #149 item 2: the mini-toolbar's three Format icons open
            // the pane FOCUSED on their section — the reference maps
            // the Fill/Line/Text slots to those pane areas.
            Act::FormatFill | Act::FormatLine | Act::FormatText => {
                self.format_pane_open = true;
                self.fmt_bufs.clear();
                let section = match a {
                    Act::FormatFill => FormatSection::Fill,
                    Act::FormatLine => FormatSection::Line,
                    _ => FormatSection::Text,
                };
                self.format_pane_focus = Some(section);
                self.format_pane_last_focus = Some(section);
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
                        vsdx::model::find_shape_by_id(&p.shapes, gid).and_then(|s| s.name_u.clone())
                    })
                });
                self.status = crate::i18n::strs().msg_opened_group(
                    name.unwrap_or_else(|| format!("Group.{gid}")),
                );
            }
            Act::Cut | Act::Copy => {
                self.copy_selection();
                if a == Act::Cut {
                    self.delete_one(page_id, shape_id);
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
                self.status = crate::i18n::strs().msg_editing_text_of_shape_shape_id(shape_id);
            }
            // #257 text-direction: prepend U+202B (RLE) to every
            // paragraph — or strip leading U+202A/U+202B back — through
            // the ordinary SetText path (one undo step, texture
            // invalidation). VSDX carries no direction cell, so the
            // explicit embedding marks are how the toggle persists
            // (Analysis/rtl-text-direction.md); the renderer's
            // explicit-override rule reads them.
            Act::ToggleRtlText => {
                // A shape with NO text yet can still pre-set its
                // direction — the empty text takes the bare mark (PR
                // #269 review).
                let text = self.selected_text(shape_id).unwrap_or_default();
                // The direction test reads the mark BEFORE any
                // stripping — trim-then-ask could never see the
                // U+202B and the toggle only ever added marks; off
                // now pins still-RTL paragraphs with an explicit
                // U+202A (PR #269 review — see svg::toggle_rtl_marks).
                let any_rtl = text
                    .replace("\r\n", "\n")
                    .lines()
                    .any(|l| l.starts_with('\u{202B}'));
                let toggled = dc::svg::toggle_rtl_marks(&text, !any_rtl);
                let scope = Scope::Shape {
                    page: page_id,
                    shape: shape_id,
                };
                let doc = self.ed().map(|ed| &ed.doc);
                if let Some(doc) = doc {
                    self.apply(dc::textfunc::text_edit_macro(doc, &scope, &toggled));
                }
                self.status = if any_rtl {
                    crate::i18n::strs().msg_rtl_text_off()
                } else {
                    crate::i18n::strs().msg_rtl_text_on()
                };
            }
            // #190/#192: a per-shape toggle for the CLICKED shape —
            // the one-way open left no reachable close; the spec made
            // sheets per-shape.
            Act::ShapeSheet => self.toggle_shapesheet(shape_id),
            Act::RouteStraight => self.apply_connector_route_style(
                page_id,
                shape_id,
                crate::connectors::ROUTE_STRAIGHT,
            ),
            Act::RouteRightAngle => self.apply_connector_route_style(
                page_id,
                shape_id,
                crate::connectors::ROUTE_RIGHT_ANGLE,
            ),
            Act::RouteCurved => {
                self.apply_connector_route_style(page_id, shape_id, crate::connectors::ROUTE_CURVED)
            }
            Act::CopyAsPicture => {
                self.copypic_open = true;
            }
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
            // #113 (URS-1113): the system clipboard receives the
            // shape's text so other apps get meaningful content (the
            // internal shape clipboard handles in-app paste).
            if let Some(txt) = s.sheet.text.as_deref().filter(|t| !t.is_empty()) {
                self.os_clipboard_text = Some(txt.to_string());
            }
        }
    }

    /// Paste the clipboard shape as a fresh instance (new id, nudged pin),
    /// top of the z-order, and select it.
    fn paste_clipboard(&mut self, page_id: i64) {
        let Some(clip) = self.clipboard.clone() else {
            // #113 (URS-1113): nothing internal — an OS clipboard
            // IMAGE pastes as a picture (another app's graphic, or
            // this app's own Copy as Picture). Probed off-thread:
            // arboard opens its own connection and the paste path
            // runs on the UI thread.
            self.paste_os_image();
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
        let Some(page_id) = self.page_id() else {
            return;
        };
        self.text_edit = Some(crate::textedit::TextEditSession::new(
            page_id, shape_id, draft, &self.view,
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
            // One undo step: the text plus refreshed cached Vs for
            // text-derived block cells (#147).
            let scope = Scope::Shape {
                page: sess.page,
                shape: sess.shape,
            };
            let doc = self.ed.as_ref().map(|ed| &ed.doc);
            if let Some(doc) = doc {
                let mac = dc::textfunc::text_edit_macro(doc, &scope, &sess.draft);
                self.apply(mac);
            }
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

    /// kittest (#177): the last painted connection-point overlay —
    /// reveal points, glue-target highlight, and the selected
    /// connector's glued sites, all page inches y-up.
    #[doc(hidden)]
    pub fn debug_conn_fx(&self) -> crate::connectors::ConnPointFx {
        self.conn_fx_last.clone()
    }

    /// #109 tests: the live snap & glue toggles.
    pub fn debug_snap_settings(&self) -> crate::snap::SnapSettings {
        self.snap
    }

    /// #109 tests: the document's persisted DocumentSettings values.
    pub fn debug_doc_snap_glue(&self) -> Option<vsdx::model::DocSnapGlue> {
        self.ed.as_ref().and_then(|e| e.doc.doc_snap_glue())
    }

    /// #109 tests: a shape's page rect (inches, y-up, bottom-left
    /// origin) — the snap engine's own frame.
    pub fn debug_shape_page_rect(&self, shape_id: i64) -> Option<(f64, f64, f64, f64)> {
        let ed = self.ed.as_ref()?;
        let page_id = self.page_id()?;
        let page = ed.doc.pages.iter().find(|p| p.id == page_id)?;
        let sh = vsdx::model::find_shape_by_id(&page.shapes, shape_id)?;
        self.shape_rect(&ed.doc, page_id, sh)
    }

    /// #109 tests: how many dynamic-grid hints the current frame
    /// holds (nonzero only mid-drag).
    pub fn debug_snap_hint_count(&self) -> usize {
        self.snap_live.len()
    }
    /// kittest (#168 diagnosis): the live dynamic-grid hints.
    #[doc(hidden)]
    pub fn debug_snap_hints_for_test(&self) -> Vec<crate::snap::Hint> {
        self.snap_live.clone()
    }

    /// #109 tests: replace the live snap settings in memory (drag
    /// tests that verify pure translation semantics turn snapping
    /// off; the document copy is untouched).
    pub fn debug_set_snap_for_test(&mut self, s: crate::snap::SnapSettings) {
        self.snap = s;
    }

    /// #113 tests: the page's guides — (id, kind name, x, y), page
    /// inches y-up (x = the line for vertical, y for horizontal,
    /// both for points).
    pub fn debug_guides(&self) -> Vec<(i64, &'static str, f64, f64)> {
        self.page_guides()
            .iter()
            .map(|g| {
                let kind = match g.kind {
                    crate::guides::GuideKind::Vertical => "vertical",
                    crate::guides::GuideKind::Horizontal => "horizontal",
                    crate::guides::GuideKind::Point => "point",
                };
                (g.id, kind, g.x, g.y)
            })
            .collect()
    }

    /// #108 tests: the page's Connect records as (from, from_cell,
    /// from_part, to, to_cell, to_part).
    pub fn debug_connects(&self) -> Vec<(i64, String, i64, i64, String, i64)> {
        let Some(page_id) = self.page_id() else {
            return Vec::new();
        };
        let Some(ed) = self.ed.as_ref() else {
            return Vec::new();
        };
        ed.doc
            .pages
            .iter()
            .find(|p| p.id == page_id)
            .map(|p| {
                p.connects()
                    .list
                    .iter()
                    .map(|c| {
                        (
                            c.from_sheet,
                            c.from_cell.clone(),
                            c.from_part,
                            c.to_sheet,
                            c.to_cell.clone(),
                            c.to_part,
                        )
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// #108 tests: one geometry row cell of a shape.
    pub fn debug_row_cell(
        &self,
        page: i64,
        shape: i64,
        sec_ix: i64,
        row_ix: i64,
        cell: &str,
    ) -> Option<f64> {
        let ed = self.ed.as_ref()?;
        let page = ed.doc.pages.iter().find(|p| p.id == page)?;
        let sh = vsdx::model::find_shape_by_id(&page.shapes, shape)?;
        sh.sheet
            .sections_named("Geometry")
            .into_iter()
            .find(|s| s.ix == sec_ix)?
            .rows
            .iter()
            .find(|r| r.ix == row_ix)?
            .cell(cell)
            .and_then(|c| c.numeric())
    }

    /// #202 tests: one geometry-row cell's FORMULA (the control-drag
    /// customizes it away).
    #[doc(hidden)]
    pub fn debug_row_cell_formula(
        &self,
        page: i64,
        shape: i64,
        sec_ix: i64,
        row_ix: i64,
        cell: &str,
    ) -> Option<String> {
        let ed = self.ed.as_ref()?;
        let page = ed.doc.pages.iter().find(|p| p.id == page)?;
        let sh = vsdx::model::find_shape_by_id(&page.shapes, shape)?;
        sh.sheet
            .sections_named("Geometry")
            .into_iter()
            .find(|s| s.ix == sec_ix)?
            .rows
            .iter()
            .find(|r| r.ix == row_ix)?
            .cell(cell)
            .and_then(|c| c.f.clone())
    }

    /// #108 tests: the active mouse tool.
    pub fn debug_tool(&self) -> Tool {
        self.tool
    }

    /// #108 tests: the shape's LOCAL Connection-section rows as
    /// (row ix, X, Y) — empty until a point is materialized onto the
    /// instance.
    pub fn debug_conn_rows(&self, page: i64, shape: i64) -> Vec<(i64, f64, f64)> {
        let Some(ed) = self.ed.as_ref() else {
            return Vec::new();
        };
        let Some(page) = ed.doc.pages.iter().find(|p| p.id == page) else {
            return Vec::new();
        };
        let Some(sh) = vsdx::model::find_shape_by_id(&page.shapes, shape) else {
            return Vec::new();
        };
        sh.sheet
            .sections_named("Connection")
            .into_iter()
            .flat_map(|sec| {
                sec.rows.iter().map(|r| {
                    (
                        r.ix,
                        r.cell("X").and_then(|c| c.numeric()).unwrap_or(0.0),
                        r.cell("Y").and_then(|c| c.numeric()).unwrap_or(0.0),
                    )
                })
            })
            .collect()
    }

    /// #108 tests: the Connection Point tool's selection.
    pub fn debug_connpt_sel(&self) -> Option<(i64, i64)> {
        self.connpt_sel
    }

    /// #108 tests: the line-jump hops painted on the last frame.
    pub fn debug_jump_count(&self) -> usize {
        self.jumps_last
    }

    /// #113 tests: the last copy-as-picture payload as text (the SVG
    /// vector flavor); None for the PNG flavor.
    pub fn debug_last_picture_text(&self) -> Option<String> {
        self.last_picture
            .as_ref()
            .and_then(|b| String::from_utf8(b.clone()).ok())
            .filter(|t| t.starts_with("<svg"))
    }

    /// #113 tests: whether the page carries the background mark.
    pub fn debug_page_is_bg(&self, page: i64) -> bool {
        self.is_background_page(page)
    }

    /// #156 tests: a shape's effective User-row value by name.
    pub fn debug_user_row(&self, page: i64, shape: i64, row: &str) -> Option<String> {
        let ed = self.ed.as_ref()?;
        let page = ed.doc.pages.iter().find(|p| p.id == page)?;
        let sh = vsdx::model::find_shape_by_id(&page.shapes, shape)?;
        let chain = shape_chain(&ed.doc, page.id, shape).ok()?;
        // #191: chain-aware — master-root rows resolve too.
        ed.doc
            .effective_user_row(sh, &chain, row)?
            .cell("Value")
            .and_then(|c| c.v.clone())
    }

    /// #156 tests: whether the Container Tools contextual tab is up.
    pub fn debug_tab_is_container_tools(&self) -> bool {
        self.tab == Tab::ContainerTools
    }

    /// #190/#192 tests: whether any ShapeSheet window is open.
    pub fn debug_shapesheet_open(&self) -> bool {
        !self.shapesheets.is_empty()
    }

    /// #192 tests: the open sheets' shape ids, in open order.
    pub fn debug_shapesheets(&self) -> Vec<i64> {
        self.shapesheets.clone()
    }

    /// #157 tests: a geometry row's type.
    pub fn debug_geo_row_type_for_test(&self, shape: i64, row_ix: i64) -> Option<String> {
        let ed = self.ed()?;
        let page = ed.doc.pages.get(self.page_idx)?;
        let sh = vsdx::model::find_shape_by_id(&page.shapes, shape)?;
        sh.sheet
            .sections_named("Geometry")
            .first()?
            .rows
            .iter()
            .find(|r| r.ix == row_ix)?
            .t
            .clone()
    }

    /// #157 tests: replace one of the shape's OWN geometry rows (the
    /// section must exist) — sets the row type and cell values.
    pub fn debug_set_geo_row_for_test(
        &mut self,
        page: i64,
        shape: i64,
        sec_ix: i64,
        row_ix: i64,
        t: &str,
        cells: &[(&str, f64)],
    ) {
        let Some(ed) = self.ed.as_mut() else { return };
        let Some(page) = ed.doc.pages.iter_mut().find(|p| p.id == page) else {
            return;
        };
        let Some(sh) = vsdx::model::find_shape_by_id_mut(&mut page.shapes, shape) else {
            return;
        };
        let mut row_cells: Vec<(String, vsdx::Cell)> = vec![
            ("X".into(), vsdx::Cell::value("0.5")),
            ("Y".into(), vsdx::Cell::value("0.5")),
        ];
        for (n, v) in cells {
            row_cells.retain(|(k, _)| k != n);
            row_cells.push((n.to_string(), vsdx::Cell::value(format!("{v:.6}"))));
        }
        let mut hit = false;
        for sec in sh.sheet.children.iter_mut() {
            if let vsdx::model::SheetChild::Section(sec) = sec {
                if sec.name == "Geometry" && sec.ix == sec_ix {
                    match sec.rows.iter_mut().find(|r| r.ix == row_ix) {
                        Some(row) => {
                            row.t = Some(t.to_string());
                            row.cells
                                .retain(|(n, _)| !cells.iter().any(|(c, _)| c == n));
                            for (n, v) in cells {
                                row.cells
                                    .push((n.to_string(), vsdx::Cell::value(format!("{v:.6}"))));
                            }
                        }
                        None => {
                            // The section exists, the row does not —
                            // append it.
                            sec.rows.push(vsdx::model::Row {
                                ix: row_ix,
                                t: Some(t.to_string()),
                                cells: row_cells,
                                ..vsdx::model::Row::default()
                            });
                        }
                    }
                    hit = true;
                    break;
                }
            }
        }
        if !hit {
            // Materialize the section (a master-based shape has none
            // locally): a MoveTo at the origin + the requested row.
            let mut row = vsdx::model::Row {
                ix: row_ix,
                t: Some(t.to_string()),
                ..vsdx::model::Row::default()
            };
            let mut all: Vec<(String, vsdx::Cell)> = vec![
                ("X".into(), vsdx::Cell::value("0.5")),
                ("Y".into(), vsdx::Cell::value("0.5")),
            ];
            for (n, v) in cells {
                all.retain(|(k, _)| k != n);
                all.push((n.to_string(), vsdx::Cell::value(format!("{v:.6}"))));
            }
            row.cells = all;
            sh.sheet
                .children
                .push(vsdx::model::SheetChild::Section(vsdx::model::Section {
                    name: "Geometry".into(),
                    ix: sec_ix,
                    rows: vec![
                        vsdx::model::Row {
                            ix: 1,
                            t: Some("MoveTo".into()),
                            cells: vec![
                                ("X".into(), vsdx::Cell::value("0")),
                                ("Y".into(), vsdx::Cell::value("0")),
                            ],
                            ..vsdx::model::Row::default()
                        },
                        row,
                    ],
                    ..vsdx::model::Section::default()
                }));
        }
    }

    /// #113 tests: the page ids in order.
    pub fn debug_page_ids(&self) -> Vec<i64> {
        self.ed()
            .map(|e| e.doc.pages.iter().map(|p| p.id).collect())
            .unwrap_or_default()
    }

    /// #113 tests: the last copy-as-picture PNG's byte length.
    pub fn debug_last_picture_len(&self) -> Option<usize> {
        self.last_picture.as_ref().map(|v| v.len())
    }

    /// #113 tests: insert a picture from bytes (the kittest path —
    /// no native dialog).
    pub fn debug_insert_picture_bytes(&mut self, bytes: &[u8]) {
        self.insert_picture_bytes(bytes);
    }

    /// #113 tests: the current page's background assignment.
    pub fn debug_back_page(&self) -> Option<i64> {
        self.page_id().and_then(|p| self.back_page_of(p))
    }

    /// #113 tests: assign the current page's background (kittest
    /// path — no dialog).
    pub fn debug_set_back_page(&mut self, bg: Option<i64>) {
        self.set_back_page(bg);
    }

    /// #113 tests: add a fresh page and return its id.
    pub fn debug_add_page(&mut self) -> i64 {
        let page_id = self.page_id().unwrap_or(0);
        let _ = page_id;
        // The AddPage command through the normal path.
        let ed = self.ed.as_ref().expect("doc");
        let id = ed.doc.pages.iter().map(|p| p.id).max().unwrap_or(0) + 1;
        self.apply(dc_core::commands::AddPage::new(format!("Page-{}", id)));
        id
    }

    /// #113 tests: the document's runtime media parts.
    pub fn debug_media_parts(&self) -> Vec<String> {
        self.ed
            .as_ref()
            .map(|e| e.doc.extra_media.iter().map(|m| m.part.clone()).collect())
            .unwrap_or_default()
    }

    /// #113 tests: the page's current size in inches.
    pub fn debug_page_size(&self) -> (f64, f64) {
        let g = self.page_geom();
        (g.w_in, g.h_in)
    }

    /// #113 tests: a cell's formula (the `F` attribute) on a shape.
    pub fn debug_cell_formula(&self, page: i64, shape: i64, cell: &str) -> Option<String> {
        let ed = self.ed.as_ref()?;
        let page = ed.doc.pages.iter().find(|p| p.id == page)?;
        let sh = vsdx::model::find_shape_by_id(&page.shapes, shape)?;
        sh.sheet.cell(cell).and_then(|c| c.f.clone())
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
        self.export_pane.reset_for(&self.path);
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

    /// #237: the view's (zoom, pan) for zoom-behavior assertions.
    pub fn debug_view_for_test(&self) -> (f32, f32, f32) {
        (self.view.zoom, self.view.pan.x, self.view.pan.y)
    }

    /// #224: arm/clear the splash moment directly (the kittest path —
    /// `App::new`'s production splash is ctor-bound).
    #[doc(hidden)]
    pub fn debug_splash_for_test(&mut self, started: Option<std::time::Instant>) {
        self.splash = started;
    }

    /// #224: whether the splash overlay is up.
    #[doc(hidden)]
    pub fn debug_splash_active_for_test(&self) -> bool {
        self.splash.is_some()
    }

    /// #224: whether a file open is in flight (the worker parse).
    #[doc(hidden)]
    pub fn debug_loading_for_test(&self) -> bool {
        self.loading.is_some()
    }

    /// #242: whether the landed document's first render is still
    /// behind the scrim.
    #[doc(hidden)]
    pub fn debug_rendering_for_test(&self) -> bool {
        self.rendering.is_some()
    }

    /// #242: whether the About window is open.
    #[doc(hidden)]
    pub fn debug_about_open_for_test(&self) -> bool {
        self.about_open
    }

    /// #242: a pointer click at the screen center, for the splash-skip
    /// test (the splash reads raw input, not widgets).
    #[doc(hidden)]
    pub fn debug_click_for_test(&mut self) {
        // The frame runs inside the harness; the click rides egui's
        // input replay — record it for the next update().
        self.test_click = true;
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

    /// kittest (#124 item 4): total RGBA bytes of the cached layer
    /// textures — the LRU budget's volume probe.
    #[doc(hidden)]
    pub fn debug_layer_tex_bytes_for_test(&self) -> u64 {
        self.layers.tex_bytes_for_test()
    }

    /// kittest (#124 item 4): pin the texture byte budget for an
    /// eviction test (the production default is 768 MiB).
    #[doc(hidden)]
    pub fn set_layer_texture_budget_for_test(&mut self, bytes: u64) {
        self.layers.set_texture_budget_for_test(bytes)
    }

    /// kittest (#143): the Quick Shapes entry's cell keys from the
    /// last stencil layout, in display order.
    #[doc(hidden)]
    pub fn debug_quick_cells_for_test(&self) -> Vec<(StencilId, i64)> {
        self.stencil.quick_keys.clone()
    }

    /// kittest (#143): point Quick Shapes persistence at an explicit
    /// store (loading any existing membership) — the restart
    /// simulation. Never persisted by the ephemeral constructors.
    #[doc(hidden)]
    pub fn set_quick_shapes_store_for_test(&mut self, path: &std::path::Path) {
        self.quick_shapes = crate::quickshapes::QuickShapes::at(path.to_path_buf());
    }

    /// kittest (#137): one cell of a shape's OWN Geometry section-0
    /// row, as (value, formula) — the vertex-drag probe.
    #[doc(hidden)]
    pub fn debug_geo_row_cell_for_test(
        &self,
        shape: i64,
        row_ix: i64,
        cell: &str,
    ) -> Option<(Option<String>, Option<String>)> {
        let ed = self.ed()?;
        let page = ed.doc.pages.get(self.page_idx)?;
        let sh = vsdx::model::find_shape_by_id(&page.shapes, shape)?;
        let row = sh
            .sheet
            .sections_named("Geometry")
            .iter()
            .find(|s| s.ix == 0)?
            .rows
            .iter()
            .find(|r| r.ix == row_ix)?;
        let c = row.cell(cell)?;
        Some((c.v.clone(), c.f.clone()))
    }

    /// kittest (#138): how many `User.msvSDContainerShape` rows the
    /// page carries (any container) — the dangling-row detector.
    #[doc(hidden)]
    pub fn debug_membership_rows_for_test(&self) -> usize {
        self.ed()
            .and_then(|ed| ed.doc.pages.get(self.page_idx))
            .map(|p| {
                p.shapes
                    .iter()
                    .flat_map(|s| s.sheet.sections_named("User"))
                    .filter(|sec| sec.named_row("msvSDContainerShape").is_some())
                    .count()
            })
            .unwrap_or(0)
    }

    /// kittest (#75): the shapes whose membership row names
    /// `container` (Visio's `User.msvSDContainerShape` convention).
    #[doc(hidden)]
    pub fn debug_container_members_for_test(&self, container: i64) -> Vec<i64> {
        self.page_id()
            .map(|p| self.container_member_ids(p, container))
            .unwrap_or_default()
    }

    /// kittest (#69): a group's direct member ids.
    #[doc(hidden)]
    pub fn debug_group_members_for_test(&self, group: i64) -> Vec<i64> {
        self.ed
            .as_ref()
            .and_then(|ed| ed.doc.pages.get(self.page_idx))
            .and_then(|p| vsdx::model::find_shape_by_id(&p.shapes, group))
            .map(|g| g.sub_shapes.iter().map(|m| m.id).collect())
            .unwrap_or_default()
    }

    /// kittest (#67): the window title last pushed to the OS (name +
    /// dirty marker) — headless stands in for the OS chrome.
    #[doc(hidden)]
    pub fn debug_window_title_for_test(&self) -> String {
        self.window_title.clone()
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
    /// kittest: whether the last frame painted the styled
    /// endpoint-drag ghost (#226).
    pub fn debug_endpoint_ghost_for_test(&self) -> bool {
        self.last_endpoint_ghost
    }

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
                            r.cell("Address")
                                .and_then(|c| c.v.clone())
                                .unwrap_or_default(),
                            r.cell("SubAddress")
                                .and_then(|c| c.v.clone())
                                .unwrap_or_default(),
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
    pub fn debug_prop_cell_for_test(&self, shape_id: i64, row: &str, cell: &str) -> Option<String> {
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
            crate::i18n::strs().msg_add_name_prop_row_test(name),
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
    /// #202: whether the selection layer skips the 1D begin→end chord
    /// (the selected connector carries a curve row).
    #[doc(hidden)]
    pub fn debug_sel_1d_curved(&self) -> bool {
        self.sel_1d_curved()
    }
    /// The status-bar text (guard-refusal tests).
    #[doc(hidden)]
    pub fn debug_status(&self) -> &str {
        &self.status
    }
    /// #149 item 2 tests: the Format pane section a mini-toolbar
    /// Format icon targeted. This is the LAST target (the one-shot
    /// scroll focus is consumed by the pane's first render).
    #[doc(hidden)]
    pub fn debug_format_pane_focus(&self) -> Option<&'static str> {
        self.format_pane_last_focus.map(|f| match f {
            FormatSection::Fill => "fill",
            FormatSection::Line => "line",
            FormatSection::Text => "text",
        })
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

    /// The Visio theme palette shown by the ribbon Fill/Line/Text
    /// color buttons and the Format pane's color rows.
    fn color_palette_window(&mut self, ctx: &egui::Context) {
        let Some(target) = self.color_pick else {
            return;
        };
        let mut open = true;
        let title = match target {
            ColorTarget::Fill => crate::i18n::strs().msg_color_fill_window(),
            ColorTarget::FillBkgnd => crate::i18n::strs().msg_color_fill_bkgnd_window(),
            ColorTarget::Line => crate::i18n::strs().msg_color_line_window(),
            ColorTarget::Text => crate::i18n::strs().msg_color_text_window(),
        };
        egui::Window::new(title)
        .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
        .open(&mut open)
        .show(ctx, |ui| {
            // #257 item 3: the dialog pass mirrors the window (the
            // swatch grid itself is symmetric — the palette sweep
            // stays LTR either way).
            ui.with_layout(dialog_dir(), |ui| {
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
                        resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, c));
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
            let layer = egui::LayerId::new(egui::Order::Tooltip, egui::Id::unique("stencil-ghost"));
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
        // #184: the per-frame keytip captures (badges, veil rects)
        // start empty; the ribbon layout below refills them.
        self.keytips.begin_frame();
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
                crate::rasterpool::RasterKey::Page {
                    part,
                    version,
                    bucket,
                } => {
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
                    self.stencil
                        .on_thumb_done(ui.ctx(), serial, master, done.image);
                }
                crate::rasterpool::RasterKey::Shape {
                    page,
                    id,
                    version,
                    bucket,
                } => {
                    if let Some(msg) = self.layers.on_shape_done(
                        ui.ctx(),
                        page,
                        id,
                        version,
                        bucket,
                        done.image,
                        // #124 item 6.3: the parsed tree rides along —
                        // bucket changes re-render it, no re-parse.
                        done.tree,
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
            let ok = self
                .group_edit
                .iter()
                .copied()
                .take_while(|id| page_groups(*id))
                .count();
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
        // #113: a copy queued OS-clipboard text (the copy path runs
        // inside input handling without a Context).
        if let Some(txt) = self.os_clipboard_text.take() {
            ctx.copy_text(txt);
        }
        self.poll_os_image();
        self.update_drag(&ctx);
        self.poll_dialog(&ctx);
        self.poll_stencil_extraction(&ctx);
        // Backstage Export worker (#35): the finished file count (or
        // the failure) lands in the pane's note and the status bar.
        if let Some(result) = self.export_pane.poll() {
            // The pane's own note carries the full message incl. any
            // density-cap notes; the status mirrors it.
            self.status = self.export_pane.note_clone().unwrap_or_else(|| match result {
                Ok(_) => crate::i18n::strs().msg_export_finished(),
                Err(e) => crate::i18n::strs().msg_export_failed_e(e.to_string()),
            });
        }
        self.handle_close_request(&ctx);
        self.reorder_pages_window(&ctx);
        self.page_setup_window(&ctx);
        self.copy_picture_window(&ctx);
        self.container_gallery_window(&ctx);
        self.backgrounds_window(&ctx);
        self.select_by_type_window(&ctx);
        self.snap_glue_window(&ctx);
        self.language_window(&ctx);
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

        // #67: the OS window carries the title; no in-app title band.
        // Before the backstage return: a drawing replaced while the
        // File screen is open (Save & New) must retitle immediately,
        // not when the user leaves backstage (branch review).
        self.sync_window_title(ui.ctx());
        if self.backstage {
            self.backstage_view(ui);
            self.confirm_new_window(&ctx);
            self.confirm_close_window(&ctx);
            return;
        }

        // Shell order: top bands, bottom bands, side panels, canvas.
        let t = theme::shell_tokens(ui.visuals());
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
        // #184: the ribbon panels have laid out — paint the armed
        // keytip veil + badges above them (no-op while disarmed).
        self.paint_keytips(&ctx);
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
            // #257 item 3: RTL mirrors the pane sides — Shapes on the
            // right, Format on the left (the observed Office RTL
            // behavior; the stable ids keep user sizing).
            let stencil_panel = if crate::i18n::rtl() {
                egui::Panel::right("stencil")
            } else {
                egui::Panel::left("stencil")
            };
            stencil_panel
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
                        // #266: the My Shapes menu scan is gated on the
                        // same disk flag the headless ctors pin false —
                        // kittest never reads the user's real folder.
                        let my_shapes = self.stencil_disk.then(Self::my_shapes_dir).flatten();
                        {
                            let App {
                                ed,
                                stencil,
                                stencils,
                                pool,
                                quick_shapes,
                                ..
                            } = self;
                            if let Some(ed) = ed.as_ref() {
                                if let Some(id) = stencil.show(
                                    ui,
                                    ed,
                                    stencils,
                                    &title,
                                    pool,
                                    quick_shapes,
                                    my_shapes.as_deref(),
                                ) {
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
                            Some(crate::stencil::Request::MyShapes(path)) => {
                                self.toggle_my_shapes_stencil(path);
                            }
                            Some(crate::stencil::Request::QuickShapesMenu(key)) => {
                                let anchor = ui
                                    .input(|i| i.pointer.interact_pos())
                                    .unwrap_or(egui::Pos2::ZERO);
                                self.quick_menu = Some((key, anchor));
                            }
                            None => {}
                        }
                        if let Some(id) = armed {
                            self.status = crate::i18n::strs().msg_master_armed_click_place(id.1);
                        }
                        if self.stencil.armed.is_some() && ui.button(crate::i18n::strs().msg_x_cancel_placement()).clicked()
                        {
                            self.stencil.armed = None;
                        }
                    });
                });
        }
        if self.format_pane_open && self.sel.is_some() {
            if crate::i18n::rtl() {
                egui::Panel::left("format_pane")
            } else {
                egui::Panel::right("format_pane")
            }
                .default_size(240.0)
                .resizable(true)
                .show(ui, |ui| self.format_pane(ui));
        }
        // #192 MDI: `central` hosts the drawing as a CHILD WINDOW
        // inside the client area (its own ui — the region the docked
        // panels left), and publishes that rect. SPEC: with exactly
        // ONE child window there are NO decorators and it fills the
        // client ("treated as it is now"); once more than one child
        // exists, every child gets its decorators and the children
        // AUTOMATICALLY ARRANGE (equal tiles) to fit — any change of
        // the child count re-fires the arrangement.
        self.central(ui);
        let drawing_open = self.ed.is_some();
        let n_children = drawing_open as usize + self.shapesheets.len();
        if n_children != self.mdi_last_count {
            self.mdi_last_count = n_children;
            self.mdi_arranging = true;
        }
        // The per-child tiles: an equal-width column each (the
        // drawing first/left, sheets after), inside the client.
        let client_ok = self.mdi_client.is_finite()
            && self.mdi_client.width() > 24.0
            && self.mdi_client.height() > 24.0;
        let tiles: Vec<egui::Rect> = if n_children == 0 || !client_ok {
            Vec::new()
        } else {
            let c = self.mdi_client;
            let gutter = 8.0;
            let n = n_children as f32;
            let w = (c.width() - gutter * 2.0 - gutter * (n - 1.0)) / n;
            (0..n_children)
                .map(|i| {
                    egui::Rect::from_min_size(
                        egui::pos2(
                            c.left() + gutter + i as f32 * (w + gutter),
                            c.top() + gutter,
                        ),
                        egui::vec2(w, c.height() - 2.0 * gutter),
                    )
                })
                .collect()
        };
        let decorated = n_children >= 2;
        // Drawing window: created inside `central` (it needs the
        // client ui), but its decorators/arrangement are decided
        // here — central reads these fields the frame before... no:
        // central runs first and builds the window, so the flags it
        // reads were set by THIS block last frame. One frame of lag
        // on transitions is invisible (the count itself is stable).
        self.mdi_decorated = decorated;
        self.mdi_drawing_tile = if n_children == 1 && drawing_open {
            // Single child: pinned full-bleed, decorator-free.
            self.mdi_client
        } else {
            tiles.first().copied().unwrap_or(self.mdi_client)
        };
        // #190/#192: the ShapeSheet as a floating child window — the
        // native title bar carries the close. Painted after the
        // drawing window so it stacks above; constrained to the
        // client area so children overlap only each other, never the
        // ribbon/status/docked panes.
        if !self.shapesheets.is_empty() && client_ok {
            // One window per open sheet (#192 spec), in open order;
            // the tiles follow the drawing (first) then the sheets.
            let mut close_ix: Option<usize> = None;
            for (i, &sid) in self.shapesheets.clone().iter().enumerate() {
                let mut open = true;
                let sheet_tile = if n_children == 1 {
                    // Sheets-only child (no drawing open): full-bleed,
                    // decorator-free per the spec.
                    self.mdi_client
                } else {
                    tiles
                        .get(drawing_open as usize + i)
                        .copied()
                        .unwrap_or(self.mdi_client)
                };
                let name = self
                    .ed()
                    .and_then(|ed| ed.doc.pages.get(self.page_idx))
                    .and_then(|p| vsdx::model::find_shape_by_id(&p.shapes, sid))
                    .and_then(|sh| sh.name_u.clone())
                    .unwrap_or_else(|| crate::i18n::strs().msg_shape_sid(sid));
                let title = crate::i18n::strs().msg_shapesheet_name(name);
                let mut win = egui::Window::new(title)
                    .id(egui::Id::unique(("dc-shapesheet", sid)))
                    .open(&mut open)
                    .title_bar(n_children >= 2 || !drawing_open)
                    .resizable(true)
                    .collapsible(false)
                    .default_size([320.0, 460.0]);
                if self.mdi_arranging || n_children == 1 {
                    win = win
                        .current_pos(sheet_tile.left_top())
                        .fixed_size(sheet_tile.size());
                }
                if self.mdi_client.is_finite() && self.mdi_client.width() > 0.0 {
                    win = win.constrain_to(self.mdi_client);
                }
                win.show(ui.ctx(), |ui| {
                    // #189: the body caps its content to the tile so
                    // the window never outgrows its slot and drifts
                    // over a neighboring child's title bar.
                    self.shapesheet_panel(ui, sid, sheet_tile.width() - 14.0);
                });
                if !open {
                    close_ix = Some(i);
                }
            }
            if let Some(i) = close_ix {
                self.shapesheets.remove(i);
            }
        }
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

        // #224 overlays, painted above everything: the modal loading
        // scrim while a document parse is in flight (the poll that
        // lands the finished editor), then the branded splash. Fixed
        // light theme — the kit's light set. Time-driven repaints keep
        // both animating.
        let mut scrim = None;
        if self.loading.is_some() {
            let in_flight = self.poll_loading();
            if let Some((path, started)) = in_flight {
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| path.display().to_string());
                scrim = Some((crate::i18n::strs().msg_opening_file(name), started));
            }
        }
        if scrim.is_none() {
            if let Some((path, started)) = &self.rendering {
                // Reveal only on the finished render: the page's
                // first render fully drained (composite ran, nothing
                // budget-deferred, every on-screen shape's texture
                // landed) AND the scrim showed its minimum window.
                let drained = self.layers.render_drained()
                    && started.elapsed().as_secs_f32() >= crate::loading::LOADING_MIN_SECS;
                if drained {
                    // The same routed message the sync landing uses
                    // (PR #269 review: this bare literal was the one
                    // unrouted status on the open path).
                    self.status =
                        crate::i18n::strs().msg_opened_path(path.display().to_string());
                    self.rendering = None;
                } else {
                    scrim = Some((crate::i18n::strs().msg_rendering_scrim(), *started));
                }
            }
        }
        if let Some((caption, started)) = scrim {
            let screen = ui.input(|i| i.viewport_rect());
            crate::loading::draw_loading(ui, screen, &caption, started);
        }
        if self.test_click {
            // The injected click (test hook): the splash skips exactly
            // as a real click would.
            self.test_click = false;
            self.splash = None;
        }
        if let Some(started) = self.splash {
            let screen = ui.input(|i| i.viewport_rect());
            if !crate::loading::draw_splash(ui, screen, started) {
                self.splash = None;
            }
        }

        if self.about_open {
            // Esc closes the frontmost About (the #110 dialog
            // keyboard audit's rule — About slipped it; #218 review).
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.about_open = false;
                return;
            }
            let mut open = self.about_open;
            egui::Window::new(crate::i18n::strs().msg_about_tarahi())
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ui.ctx(), |ui| {
                    let (rect, _) =
                        ui.allocate_exact_size(egui::vec2(220.0, 150.0), egui::Sense::hover());
                    crate::loading::draw_mark(
                        ui.painter(),
                        rect.center() + egui::Vec2::new(0.0, -14.0),
                        62.0,
                        crate::loading::SPLASH_SECS, // settled
                    );
                    ui.painter().text(
                        rect.center() + egui::Vec2::new(0.0, 48.0),
                        egui::Align2::CENTER_CENTER,
                        crate::i18n::strs().msg_splash_wordmark(),
                        egui::FontId::proportional(26.0),
                        egui::Color32::from_rgb(0x1A, 0x24, 0x44),
                    );
                    // Inside the WINDOW — the outer ui paints on the
                    // shell behind it (#218 review).
                    ui.label(
                        egui::RichText::new(crate::i18n::strs().msg_tarahi_v_cargo_pkg_version_clean(
                            env!("CARGO_PKG_VERSION"),
                        ))
                        .size(12.0),
                    );
                });
            self.about_open = open && self.about_open;
        }
    }

    /// #246: the Language dialog (View ▸ Window) — one radio row per
    /// shipped language (native names), applying on selection. Strings
    /// resolve through the catalog each frame, so the switch is live:
    /// the very next frame re-renders in the picked language.
    fn language_window(&mut self, ctx: &egui::Context) {
        if !self.language_dlg_open {
            return;
        }
        let mut open = true;
        let mut picked: Option<crate::i18n::L10n> = None;
        let mut close = false;
        egui::Window::new(crate::i18n::strs().msg_display_language())
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                // #257 item 3: RTL right-aligns the dialog's content.
                let dir = if crate::i18n::rtl() {
                    egui::Layout::top_down(egui::Align::RIGHT)
                } else {
                    egui::Layout::top_down(egui::Align::LEFT)
                };
                ui.with_layout(dir, |ui| {
                ui.label(crate::i18n::strs().msg_choose_the_display_language_for());
                ui.add_space(6.0);
                let current = crate::i18n::current();
                for lang in crate::i18n::L10n::iter() {
                    if ui
                        .radio(*lang == current, lang.language_name())
                        .clicked()
                    {
                        picked = Some(*lang);
                    }
                }
                ui.add_space(6.0);
                ui.weak(crate::i18n::strs().msg_applies_to_the_ribbon_menus_and());
                ui.add_space(8.0);
                if ui.button(crate::i18n::strs().msg_close()).clicked() {
                    close = true;
                }
                });
            });
        if let Some(lang) = picked {
            crate::i18n::set_language(lang);
            self.status = crate::i18n::strs().msg_applies_to_the_ribbon_menus_and();
        }
        self.language_dlg_open = open && !close;
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
            .unwrap_or_else(|| crate::i18n::strs().msg_the_drawing());
        // MODAL (#166): the backdrop blocks input to everything
        // behind the prompt — the document cannot change underneath
        // the Save / Don't-Save decision. The title rides inside the
        // dialog frame (a modal has no title bar).
        egui::Modal::new(egui::Id::unique("dc-confirm-close")).show(ctx, |ui| {
            // #257 item 3: the modal mirrors with the chrome.
            ui.with_layout(dialog_dir(), |ui| {
            ui.strong(crate::i18n::strs().msg_save_changes());
            ui.add_space(4.0);
            ui.label(crate::i18n::strs().msg_save_changes_to_name(name));
            row(ui, |ui| {
                if ui.button(crate::i18n::strs().msg_save()).clicked() {
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
                if ui.button(crate::i18n::strs().msg_don_t_save()).clicked() {
                    self.confirm_close = None;
                    self.finish_close(ctx, target);
                }
                if ui.button(crate::i18n::strs().msg_cancel()).clicked() {
                    self.confirm_close = None;
                }
            });
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
    /// #192 4a: move the active document into the inactive set
    /// (capture its session fields; the per-doc UI state resets on
    /// the way back — view/selection state is slice 4b).
    fn stash_active_doc(&mut self) {
        if self.ed.is_none() {
            return;
        }
        let ed = self.ed.take().unwrap();
        self.docs.push(DocSession {
            ed,
            path: std::mem::take(&mut self.path),
            dirty: std::mem::replace(&mut self.dirty, false),
            shapesheets: std::mem::take(&mut self.shapesheets),
            view: std::mem::take(&mut self.view),
            page_idx: self.page_idx,
            sel: self.sel,
            sel_set: std::mem::take(&mut self.sel_set),
            group_edit: std::mem::take(&mut self.group_edit),
        });
    }

    /// #192 4a: activate an inactive document — the active one swaps
    /// into its slot (nothing is lost).
    fn activate_doc(&mut self, ix: usize) {
        if ix >= self.docs.len() {
            return;
        }
        let mut incoming = self.docs.swap_remove(ix);
        self.stash_active_doc();
        self.ed = Some(incoming.ed);
        self.path = incoming.path;
        self.dirty = incoming.dirty;
        self.shapesheets = std::mem::take(&mut incoming.shapesheets);
        self.view = incoming.view;
        // A stored fit-MODE view re-fits to the (possibly changed)
        // client; a manual zoom/pan view restores exactly.
        if self.view.fit_mode {
            self.view.fit_pending = true;
        }
        self.page_idx = incoming.page_idx;
        self.sel = incoming.sel;
        self.sel_set = incoming.sel_set;
        self.group_edit = incoming.group_edit;
        self.tex.clear();
        self.layers.clear();
        self.page_states.clear();
        let name = self
            .path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| crate::i18n::strs().msg_untitled());
        self.status = crate::i18n::strs().msg_switched_to_name(name);
    }

    fn close_doc(&mut self) {
        self.ed = None;
        self.path = PathBuf::new();
        self.dirty = false;
        // #192 spec: closing the document closes its ShapeSheet(s).
        self.shapesheets.clear();
        self.set_selection(None);
        self.stencil = Stencil::default();
        // The reset pane drops the configured category order (#105,
        // CodeRabbit): re-apply the stencil preferences right after.
        self.apply_stencil_preferences();
        self.tex.clear();
        self.layers.clear();
        self.page_states.clear();
        self.tab_menu = None;
        self.tab_rename = None;
        self.reorder_open = false;
        self.comment_target = None;
        self.comment_edit = None;
        // #192 4a: another open document promotes to active.
        if !self.docs.is_empty() {
            let next = self.docs.remove(0);
            self.ed = Some(next.ed);
            self.path = next.path;
            self.dirty = next.dirty;
            self.shapesheets = next.shapesheets;
            let name = self
                .path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| crate::i18n::strs().msg_untitled());
            self.status = crate::i18n::strs().msg_closed_switched_to_name(name);
        } else {
            self.status = crate::i18n::strs().msg_document_closed();
        }
    }

    /// Unsaved-changes prompt before File → New discards work.
    fn confirm_new_window(&mut self, ctx: &Context) {
        if !self.confirm_new {
            return;
        }
        // MODAL (#166), same as the close prompt: nothing behind the
        // dialog is interactive while the decision is pending.
        egui::Modal::new(egui::Id::unique("dc-confirm-new")).show(ctx, |ui| {
            // #257 item 3: the modal mirrors with the chrome.
            ui.with_layout(dialog_dir(), |ui| {
            ui.strong(crate::i18n::strs().msg_save_changes());
            ui.add_space(4.0);
            ui.label(crate::i18n::strs().msg_the_current_drawing_has_unsaved());
            row(ui, |ui| {
                if ui.button(crate::i18n::strs().msg_save_new()).clicked() {
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
                if ui.button(crate::i18n::strs().msg_discard_new()).clicked() {
                    self.start_new();
                }
                if ui.button(crate::i18n::strs().msg_cancel()).clicked() {
                    self.confirm_new = false;
                }
            });
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
        if crate::i18n::rtl() {
            egui::Panel::right("backstage_rail")
        } else {
            egui::Panel::left("backstage_rail")
        }
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
                    rail(ui, &crate::i18n::strs().msg_save(), has_doc, None, Some(BackAction::Save));
                    rail(ui, &crate::i18n::strs().msg_save_as(), has_doc, None, Some(BackAction::SaveAs));
                    rail(ui, &crate::i18n::strs().msg_close(), has_doc, None, Some(BackAction::CloseDoc));
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
                    rail(ui, &crate::i18n::strs().msg_info(), true, Some(BackPage::Info), None);
                    rail(ui, &crate::i18n::strs().msg_recent(), true, Some(BackPage::Recent), None);
                    rail(ui, &crate::i18n::strs().msg_rail_new(), true, Some(BackPage::New), None);
                    rail(ui, &crate::i18n::strs().msg_rail_print(), true, Some(BackPage::Print), None)
                        .on_hover_text(crate::i18n::strs().msg_print_the_current_drawing());
                    rail(ui, &crate::i18n::strs().msg_share(), true, Some(BackPage::Share), None)
                        .on_hover_text(crate::i18n::strs().msg_share_a_copy_of_the_drawing());
                    rail(ui, &crate::i18n::strs().msg_rail_export(), true, Some(BackPage::Export), None)
                        .on_hover_text(crate::i18n::strs().msg_export_entry_desc());
                });
                // Band 3 — pinned footer, non-critical items only (Apple:
                // "avoid putting critical information or actions at the
                // bottom of a sidebar").
                let rail_footer_dir = if crate::i18n::rtl() {
                    egui::Layout::bottom_up(egui::Align::RIGHT)
                } else {
                    egui::Layout::bottom_up(egui::Align::LEFT)
                };
                ui.with_layout(rail_footer_dir, |ui| {
                    ui.add_enabled(
                        false,
                        egui::Button::new(
                            egui::RichText::new(crate::i18n::strs().msg_options())
                                .size(14.0)
                                .color(t.rail_disabled),
                        )
                        .frame(false),
                    )
                    .on_disabled_hover_text(crate::i18n::strs().msg_options_arrive_with_a_later_mile());
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
                                .unwrap_or_else(|| crate::i18n::strs().msg_untitled());
                            let dir = self
                                .path
                                .parent()
                                .map(|p| p.display().to_string())
                                .filter(|s| !s.is_empty())
                                .unwrap_or_else(|| crate::i18n::strs().msg_not_saved_yet());
                            let status = if self.dirty {
                                crate::i18n::strs().msg_unsaved_changes()
                            } else {
                                crate::i18n::strs().msg_all_changes_saved()
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
                                        ui.weak(crate::i18n::strs().msg_name());
                                        ui.label(&name);
                                        ui.end_row();
                                        ui.weak(crate::i18n::strs().msg_location());
                                        ui.label(&dir);
                                        ui.end_row();
                                        ui.weak(crate::i18n::strs().msg_status());
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
                                ui.add_enabled(false, egui::Button::new(crate::i18n::strs().msg_protect_drawing()))
                                    .on_disabled_hover_text(
                                        crate::i18n::strs().msg_document_protection_arrives_with(),
                                    );
                                ui.add_enabled(false, egui::Button::new(crate::i18n::strs().msg_inspect_drawing()))
                                    .on_disabled_hover_text(
                                        crate::i18n::strs().msg_document_inspection_arrives_with(),
                                    );
                            });
                        } else {
                            ui.heading(crate::i18n::strs().msg_no_document_open());
                            ui.add_space(8.0);
                            if ui.button(crate::i18n::strs().msg_open_file_cta()).clicked() {
                                open_browse = true;
                            }
                        }
                    }
                    BackPage::Recent => {
                        ui.horizontal(|ui| {
                            ui.heading(crate::i18n::strs().msg_recent());
                            // #257: the Browse pin mirrors with the chrome.
                            let recent_dir = if crate::i18n::rtl() {
                                egui::Layout::left_to_right(egui::Align::TOP)
                            } else {
                                egui::Layout::right_to_left(egui::Align::TOP)
                            };
                            ui.with_layout(recent_dir, |ui| {
                                if ui.button(crate::i18n::strs().msg_browse()).clicked() {
                                    open_browse = true;
                                }
                            });
                        });
                        ui.add_space(8.0);
                        if self.recent.entries().is_empty() {
                            // elementary Welcome Screen: context plus
                            // named actions instead of an empty list.
                            ui.weak(crate::i18n::strs().msg_drawings_you_open_will_appear_he());
                            ui.add_space(12.0);
                            if welcome_action(
                                ui,
                                Icon::New,
                                &crate::i18n::strs().msg_new_blank_drawing(),
                                &crate::i18n::strs().msg_start_an_empty_drawing_on_a_lett(),
                            ) {
                                new_blank = true;
                            }
                            if welcome_action(
                                ui,
                                Icon::Open,
                                &crate::i18n::strs().msg_open_file_title(),
                                &crate::i18n::strs().msg_browse_for_an_existing_visio_pac(),
                            ) {
                                open_browse = true;
                            }
                        } else if let Some(p) = self.recent_list_ui(ui) {
                            open_recent = Some(p);
                        }
                    }
                    BackPage::New => {
                        ui.horizontal(|ui| {
                            ui.heading(crate::i18n::strs().msg_rail_new());
                            // 2016's search-first New page; elementary/
                            // Apple: trailing toolbar position, instant
                            // type-to-filter.
                            // #257: the search pin mirrors with the chrome.
                            let new_search_dir = if crate::i18n::rtl() {
                                egui::Layout::left_to_right(egui::Align::TOP)
                            } else {
                                egui::Layout::right_to_left(egui::Align::TOP)
                            };
                            ui.with_layout(new_search_dir, |ui| {
                                ui.add(
                                    egui::TextEdit::singleline(&mut search_text)
                                        .hint_text(crate::i18n::strs().msg_search_templates())
                                        .desired_width(200.0),
                                );
                            });
                        });
                        ui.add_space(4.0);
                        ui.weak(crate::i18n::strs().msg_suggested_flowchart_network_floo());
                        ui.add_space(12.0);
                        let q = search_text.trim().to_lowercase();
                        let mut card_click = false;
                        ui.horizontal_wrapped(|ui| {
                            if backstage_card(ui, Icon::New, &crate::i18n::strs().msg_blank_drawing(), q.is_empty()) {
                                card_click = true;
                            }
                            for (name, icon) in new_template_entries() {
                                if !q.is_empty() && !name.to_lowercase().contains(&q) {
                                    continue;
                                }
                                backstage_card(ui, icon, &name, false);
                            }
                        });
                        if card_click {
                            new_blank = true;
                        }
                        if !q.is_empty()
                            && !"blank drawing".contains(&q)
                            && new_template_entries()
                                .iter()
                                .all(|(n, _)| !n.to_lowercase().contains(&q))
                        {
                            ui.add_space(8.0);
                            ui.weak(crate::i18n::strs().msg_no_templates_match(search_text.trim()));
                        }
                    }
                    BackPage::Print => backstage_stub(
                        ui,
                        &crate::i18n::strs().msg_rail_print(),
                        &crate::i18n::strs().msg_send_the_drawing_to_a_printer_or(),
                        &crate::i18n::strs().msg_print_stub(),
                        &crate::i18n::strs().msg_printing_arrives_with_m6(),
                    ),
                    BackPage::Share => backstage_stub(
                        ui,
                        &crate::i18n::strs().msg_share(),
                        &crate::i18n::strs().msg_share_a_copy_of_the_drawing_with(),
                        &crate::i18n::strs().msg_share_a_copy(),
                        &crate::i18n::strs().msg_sharing_arrives_with_m6(),
                    ),
                    BackPage::Export => {
                        ui.heading(crate::i18n::strs().msg_rail_export());
                        ui.add_space(6.0);
                        ui.weak(crate::i18n::strs().msg_export_page_sub());
                        ui.add_space(6.0);
                        let page_count = self.ed().map_or(0, |ed| ed.doc.pages.len());
                        match self.export_pane.show(ui, page_count) {
                            crate::export::PaneAction::None => {}
                            crate::export::PaneAction::Browse(format) => {
                                self.begin_dialog(PendingKind::ExportSave(format));
                            }
                            crate::export::PaneAction::Export => {
                                let dir = self.path.parent().map(PathBuf::from);
                                if let Some(ed) = self.ed.as_ref() {
                                    self.export_pane.launch(&ed.doc, self.page_idx, dir);
                                }
                            }
                        }
                    }
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
/// Template names translate at paint (#246): resolved per call, so
/// the backstage New page re-renders on a language switch.
fn new_template_entries() -> Vec<(String, Icon)> {
    vec![
        (crate::i18n::strs().msg_flowchart(), Icon::Swimlane),
        (crate::i18n::strs().msg_cross_functional(), Icon::Container),
        (crate::i18n::strs().msg_network_diagram(), Icon::Link),
        (crate::i18n::strs().msg_org_chart(), Icon::Group),
        (crate::i18n::strs().msg_floor_plan(), Icon::Grid),
    ]
}

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
            crate::i18n::strs().msg_start_an_empty_drawing()
        } else {
            crate::i18n::strs().msg_template_gallery_arrives_with_th()
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
/// #257 item 3: the shared dialog-body scope. The planned RTL form
/// was `top_down(Align::RIGHT)` — but egui 0.37's right-aligned
/// vertical scope DEGENERATES inside nested children (each checkbox
/// label wraps one character per line; observed in the Persian Snap &
/// Glue golden), so the body scope keeps the plain LTR vertical
/// layout in every language. The RTL dialog parity that matters is
/// carried by [`row`]/[`row_rev`] (mirrored row, column and button
/// order) and egui's own bidi shaping of each label; full-body
/// right-alignment is recorded as an egui limitation on the issue.
fn dialog_dir() -> egui::Layout {
    egui::Layout::top_down(egui::Align::LEFT)
}

/// A mirrored row: the direction flip runs INSIDE a `ui::horizontal`
/// child (its wrap-aware measure shields the parent — a bare
/// `with_layout(left_to_right)` scope in a vertical parent reserves the
/// parent's full cross space, voiding everything below it; the
/// pre-existing end-pinned bars wrap the flip in horizontal for
/// exactly that reason).
fn row<R>(ui: &mut Ui, add_contents: impl FnOnce(&mut Ui) -> R) -> egui::Response {
    let dir = if crate::i18n::rtl() {
        egui::Layout::right_to_left(egui::Align::Center)
    } else {
        egui::Layout::left_to_right(egui::Align::Center)
    };
    ui.horizontal(|inner| inner.with_layout(dir, add_contents)).response
}

/// The INVERSE of [`row`] — an end-pinned control bar whose LTR form
/// was `right_to_left` and mirrors to `left_to_right` under RTL.
fn row_rev<R>(ui: &mut Ui, add_contents: impl FnOnce(&mut Ui) -> R) -> egui::Response {
    let dir = if crate::i18n::rtl() {
        egui::Layout::left_to_right(egui::Align::Center)
    } else {
        egui::Layout::right_to_left(egui::Align::Center)
    };
    ui.horizontal(|inner| inner.with_layout(dir, add_contents)).response
}

/// The horizontal-row twin of [`dialog_dir`] — the `right_to_left` /
/// `left_to_right` pair the mirrored strips use.
fn ctx_item(
    ui: &mut Ui,
    icon: Option<crate::ribbon::Icon>,
    label: &str,
    hint: &str,
    enabled: bool,
) -> bool {
    let tokens = theme::menu_tokens(ui.visuals());
    // #257 item 3: an RTL menu mirrors the row — the icon hugs the
    // right edge, the label right-aligns (its icon gutter on the
    // right), and the accelerator hint pins the LEFT edge (Office's
    // RTL menus keep the shortcut column at the mirrored leading
    // edge).
    let rtl = crate::i18n::rtl();
    let mut job = egui::text::LayoutJob {
        halign: if rtl {
            egui::Align::RIGHT
        } else {
            egui::Align::LEFT
        },
        ..egui::text::LayoutJob::default()
    };
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
    let mut trailing_hint = false;
    if !hint.is_empty() && !rtl {
        job.append(
            &format!("   {hint}"),
            0.0,
            egui::TextFormat::simple(egui::FontId::proportional(11.0), tokens.hint),
        );
        trailing_hint = true;
    }
    let resp = ui.add_enabled(
        enabled,
        egui::Button::new(job).min_size(Vec2::new(ui.available_width().max(150.0), 21.0)),
    );
    if let Some(ic) = icon {
        let x = if rtl {
            resp.rect.right() - 23.0
        } else {
            resp.rect.left() + 7.0
        };
        let r = Rect::from_min_size(Pos2::new(x, resp.rect.center().y - 8.0), Vec2::splat(16.0));
        // icons take the theme-derived tint: the light Office palette
        // (the gray ShapeSheet glyph included) vanishes on a dark menu
        // (GitHub #48 review).
        ic.paint_tinted(ui, r, tokens.icon_tint(ic.office_color(), enabled));
    }
    if !trailing_hint && !hint.is_empty() {
        // RTL accelerator column: a left-pinned overlay galley (the
        // hint cannot ride inside the right-aligned label job).
        let gal = ui.painter().layout_no_wrap(
            hint.to_string(),
            egui::FontId::proportional(11.0),
            tokens.hint,
        );
        ui.painter().galley(
            Pos2::new(resp.rect.left() + 7.0, resp.rect.center().y - gal.size().y / 2.0),
            gal,
            tokens.hint,
        );
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

/// Whether a key carries a single-character name — in this egui's
/// `Key` that is exactly the letters ("A".."Z") and digits
/// ("0".."9"), the keytip alphabet (every other key names a word).
fn is_keytip_key(k: egui::Key) -> bool {
    k.name().chars().count() == 1
}

/// One QAT button; returns its laid-out rect. `digit` is the #184
/// keytip badge (Office numbers the QAT 1..=9 by position); the
/// action closure is a pure constructor, evaluated once for the
/// badge and shared with the click.
fn qat_button(
    ui: &mut Ui,
    icon: Icon,
    tip: &str,
    action: impl FnOnce() -> Action,
    app: &mut App,
    digit: u8,
) -> Rect {
    let a = action();
    let (r, resp) = ui.allocate_exact_size(Vec2::new(22.0, 22.0), Sense::click());
    // #184: the QAT digit badge (Office numbers the QAT 1..=9).
    app.keytips.badges.push(crate::keytips::Badge {
        rect: r,
        key: digit.to_string(),
        hit: crate::keytips::Hit::Action {
            action: a,
            enabled: true,
            label: String::new(),
        },
        anchor: None,
    });
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
        app.dispatch(a);
    }
    r
}

/// Tab captions come from the catalog (#246): resolved per call, so
/// the strip re-renders on a language switch.
fn t_title(t: Tab) -> String {
    match t {
        Tab::File => crate::i18n::strs().msg_file(),
        Tab::Home => crate::i18n::strs().msg_home(),
        Tab::Insert => crate::i18n::strs().msg_insert(),
        Tab::Design => crate::i18n::strs().msg_design(),
        Tab::Data => crate::i18n::strs().msg_data(),
        Tab::Review => crate::i18n::strs().msg_review(),
        Tab::View => crate::i18n::strs().msg_view(),
        Tab::Developer => crate::i18n::strs().msg_developer(),
        Tab::ContainerTools => crate::i18n::strs().msg_container_tools(),
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
/// Distance from `p` to the segment a→b within `threshold` (#108
/// connector hit-testing).
fn segment_hit(a: (f64, f64), b: (f64, f64), p: (f64, f64), threshold: f64) -> bool {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len2 = dx * dx + dy * dy;
    let t = if len2 < 1e-12 {
        0.0
    } else {
        (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / len2).clamp(0.0, 1.0)
    };
    let (cx, cy) = (a.0 + t * dx, a.1 + t * dy);
    (p.0 - cx).hypot(p.1 - cy) <= threshold
}

/// Connection rows of one sheet → page-space connection points on
/// `t` (#109): rows carry shape-local X/Y (often cached formula
/// values, e.g. `Width*0.5`), mapped through the same unrotated
/// bbox frame as the #137 vertex handles.
fn collect_connection_points(t: &mut crate::snap::SnapTarget, sheet: &vsdx::model::Sheet) {
    for sec in sheet.sections_named("Connection") {
        for row in &sec.rows {
            if let (Some(x), Some(y)) = (
                row.cell("X").and_then(|c| c.numeric()),
                row.cell("Y").and_then(|c| c.numeric()),
            ) {
                t.connection_points.push((t.rect.0 + x, t.rect.1 + y));
            }
        }
    }
}

/// One-line description of a snap landing, for status notes (#109).
fn snap_hit_text(h: crate::snap::SnapHit) -> String {
    use crate::snap::SnapHit::*;
    match h {
        Grid => crate::i18n::strs().msg_the_grid(),
        Ruler => crate::i18n::strs().msg_a_ruler_subdivision(),
        Guide(id) => crate::i18n::strs().msg_guide_id(id),
        AlignmentBox(id) => crate::i18n::strs().msg_shape_id_s_alignment_box(id),
        ShapeExtension(id) => crate::i18n::strs().msg_shape_id_s_extension(id),
        ShapeIntersection => crate::i18n::strs().msg_a_shape_intersection(),
        ConnectionPoint(id) => crate::i18n::strs().msg_shape_id_s_connection_point(id),
        ShapeHandle(id) => crate::i18n::strs().msg_shape_id_s_handle(id),
        ShapeVertex(id) => crate::i18n::strs().msg_shape_id_s_vertex(id),
    }
}

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
        .unwrap_or_else(|| crate::i18n::strs().msg_document_masters())
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

    /// #266: the My Shapes menu toggle opens a user stencil through
    /// the file path — exactly a picker result (title = file stem,
    /// pane identity = the path) — and the toggle on an open entry
    /// closes it (the menu checkbox state). An unparseable file
    /// surfaces as a status note, never a panic.
    #[test]
    fn my_shapes_toggle_opens_and_closes_the_file_stencil() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("community-pack.vssx");
        let basic = crate::stencil::builtin::by_slug("basic").expect("basic ships");
        std::fs::write(&path, basic.bytes).unwrap();

        let mut app = App::with_doc_opt(None);
        app.toggle_my_shapes_stencil(path.clone());
        assert_eq!(
            app.debug_open_stencil_titles(),
            vec!["community-pack".to_string()],
            "the menu open lands like a picker result: stem title, file identity"
        );
        app.toggle_my_shapes_stencil(path.clone());
        assert!(
            app.debug_open_stencil_titles().is_empty(),
            "the toggle on an open entry closes it"
        );
        app.toggle_my_shapes_stencil(path);
        assert_eq!(
            app.debug_open_stencil_titles(),
            vec!["community-pack".to_string()],
            "a closed entry re-opens through the same menu path"
        );

        let bad = tmp.path().join("broken.vssx");
        std::fs::write(&bad, b"not a package").unwrap();
        app.toggle_my_shapes_stencil(bad);
        assert!(!app.status.is_empty(), "an unreadable stencil is surfaced");
    }

    /// #188: a NURBSTo row's E-formula interior control points join
    /// the selection's vertex handles — one circle per group, at the
    /// fraction position over the bbox extents; the row's own X/Y
    /// vertex handles stay.
    #[test]
    fn nurbs_row_gets_e_point_handles() {
        let mut sh = vsdx::model::Shape {
            id: 1,
            type_: Some("Shape".into()),
            ..vsdx::model::Shape::default()
        };
        for (n, v) in [
            ("PinX", 2.0),
            ("PinY", 1.0),
            ("Width", 4.0),
            ("Height", 2.0),
            ("LocPinX", 2.0),
            ("LocPinY", 1.0),
        ] {
            sh.sheet.set_cell(n, vsdx::Cell::value(format!("{v}")));
        }
        let e = "NURBS(1, 3, 0, 0, 1,0.5,0,1, 3,1.5,0,1)";
        let row = |t: &str, ix: i64, cells: Vec<(&str, f64)>| vsdx::model::Row {
            t: Some(t.into()),
            ix,
            cells: cells
                .into_iter()
                .map(|(n, v)| (n.to_string(), vsdx::Cell::value(format!("{v}"))))
                .collect(),
            ..vsdx::model::Row::default()
        };
        let mut sec = vsdx::model::Section {
            name: "Geometry".into(),
            ix: 0,
            ..vsdx::model::Section::default()
        };
        sec.rows = vec![
            row("MoveTo", 1, vec![("X", 0.0), ("Y", 0.0)]),
            row(
                "NURBSTo",
                2,
                vec![
                    ("X", 4.0),
                    ("Y", 2.0),
                    ("A", 0.0),
                    ("B", 1.0),
                    ("C", 0.0),
                    ("D", 1.0),
                ],
            ),
        ];
        let nurbs_row = &mut sec.rows[1];
        nurbs_row.cells.push((
            "E".to_string(),
            vsdx::Cell {
                v: Some(e.to_string()),
                f: Some(e.to_string()),
                ..vsdx::Cell::default()
            },
        ));
        sh.sheet
            .children
            .push(vsdx::model::SheetChild::Section(std::mem::take(&mut sec)));

        let mut ed = dc_core::Editor::new_blank();
        let page_id = ed.doc.pages[0].id;
        ed.doc.pages[0].shapes.push(sh);
        let mut app = App::with_doc_opt(None);
        app.ed = Some(ed);
        app.page_idx = 0;
        app.sel = Some((page_id, 1));
        let handles = app.selection_handles();
        let nurbs: Vec<_> = handles
            .iter()
            .filter(|h| {
                matches!(
                    h.id,
                    HandleId::GeoPoint {
                        cell: canvas::GeoCell::NurbsPt(_),
                        ..
                    }
                )
            })
            .collect();
        assert_eq!(nurbs.len(), 2, "one handle per E group: {nurbs:?}");
        // Group 0 (1, 0.5) of the 4×2 bbox at origin (0,0): (4, 1).
        if let HandleId::GeoPoint {
            cell: canvas::GeoCell::NurbsPt(0),
            ..
        } = nurbs[0].id
        {
            assert!((nurbs[0].pos.0 - 4.0).abs() < 1e-9, "{}", nurbs[0].pos.0);
            assert!((nurbs[0].pos.1 - 1.0).abs() < 1e-9, "{}", nurbs[0].pos.1);
        } else {
            unreachable!()
        }
        // The row's own X/Y vertex handles stay (two per point row).
        let vertex = handles
            .iter()
            .filter(|h| {
                matches!(
                    h.id,
                    HandleId::GeoPoint {
                        cell: canvas::GeoCell::X | canvas::GeoCell::Y,
                        ..
                    }
                )
            })
            .count();
        assert!(vertex >= 4, "MoveTo + NURBSTo X/Y handles: {vertex}");
    }

    // Dialogs must never block the UI thread; these tests drive the
    // polling half directly (no real native dialog is ever spawned).

    /// #132 (URS-1225): a sampled stroke becomes an editable shape —
    /// the geometry rows land, the selection carries #137 vertex
    /// handles for them, undo removes the shape.
    #[test]
    fn freeform_stroke_becomes_editable_geometry() {
        let ed = dc_core::Editor::new_blank();
        let page_id = ed.doc.pages[0].id;
        let mut app = App::with_doc_opt(None);
        app.ed = Some(ed);
        app.page_idx = 0;
        let stroke: Vec<(f64, f64)> = (0..=20)
            .map(|i| {
                let t = i as f64 / 20.0;
                (1.0 + t * 2.0, 2.0 + (t * std::f64::consts::PI).sin())
            })
            .collect();
        app.create_freeform(stroke);
        let page = &app.ed.as_ref().unwrap().doc.pages[0];
        assert_eq!(page.shapes.len(), 1, "the freeform shape exists");
        let s = &page.shapes[0];
        let geo = s.sheet.sections_named("Geometry");
        assert_eq!(geo.len(), 1, "one Geometry section");
        assert_eq!(geo[0].rows[0].t.as_deref(), Some("MoveTo"));
        assert!(geo[0].rows.len() >= 2, "at least one segment row");
        assert_eq!(app.debug_sel(), Some((page_id, s.id)), "selected");
        // Vertex handles for the point rows (#137) — editable.
        let handles = app.selection_handles();
        let vertex = handles
            .iter()
            .filter(|h| {
                matches!(
                    h.id,
                    HandleId::GeoPoint {
                        cell: canvas::GeoCell::X | canvas::GeoCell::Y,
                        ..
                    }
                )
            })
            .count();
        assert!(vertex >= 4, "vertex handles on the freeform rows: {vertex}");
        // Undo removes the shape (one add-shape step).
        app.do_undo();
        let page = &app.ed.as_ref().unwrap().doc.pages[0];
        assert_eq!(page.shapes.len(), 0, "undo removes the shape");
    }

    /// #203: a spline/NURBS-authored CONNECTOR surfaces its
    /// positional controls through the 1D handle branch — vertex
    /// handles per SplineStart/SplineKnot/NURBSTo row (via the
    /// HitFrame) plus NurbsPt handles for the E formula's interior
    /// points.
    #[test]
    fn spline_connector_rows_get_handles() {
        let mut sh = vsdx::model::Shape {
            id: 1,
            type_: Some("Shape".into()),
            ..vsdx::model::Shape::default()
        };
        for (n, v) in [
            ("ObjType", 2.0),
            ("BeginX", 1.0),
            ("BeginY", 1.0),
            ("EndX", 5.0),
            ("EndY", 3.0),
            ("PinX", 3.0),
            ("PinY", 2.0),
            ("Width", 4.0),
            ("Height", 2.0),
            ("LocPinX", 2.0),
            ("LocPinY", 1.0),
        ] {
            sh.sheet.set_cell(n, vsdx::Cell::value(format!("{v}")));
        }
        // The E group (0.5, 1.0) = fractions of W=4/H=2 → local (2, 2)
        // → page (3, 3) through the identity-ish frame.
        let e = "NURBS(1, 3, 0, 0, 0.5,1,0,1)";
        let row = |ix: i64, t: &str, cells: Vec<(&str, f64)>| vsdx::model::Row {
            t: Some(t.into()),
            ix,
            cells: cells
                .into_iter()
                .map(|(n, v)| (n.to_string(), vsdx::Cell::value(format!("{v}"))))
                .collect(),
            ..vsdx::model::Row::default()
        };
        let mut sec = vsdx::model::Section {
            name: "Geometry".into(),
            ix: 0,
            ..vsdx::model::Section::default()
        };
        sec.rows = vec![
            row(1, "MoveTo", vec![("X", 0.0), ("Y", 0.0)]),
            row(
                2,
                "NURBSTo",
                vec![
                    ("X", 4.0),
                    ("Y", 2.0),
                    ("A", 0.0),
                    ("B", 1.0),
                    ("C", 0.0),
                    ("D", 1.0),
                ],
            ),
        ];
        let nurbs_row = &mut sec.rows[1];
        nurbs_row.cells.push((
            "E".to_string(),
            vsdx::Cell {
                v: Some(e.to_string()),
                f: Some(e.to_string()),
                ..vsdx::Cell::default()
            },
        ));
        sh.sheet
            .children
            .push(vsdx::model::SheetChild::Section(std::mem::take(&mut sec)));

        let ed = dc_core::Editor::new_blank();
        let page_id = ed.doc.pages[0].id;
        let mut app = App::with_doc_opt(None);
        app.ed = Some(ed);
        app.page_idx = 0;
        // The shape must be IN the doc for selection_handles: swap the
        // doc's blank shape out — simplest is to keep the fixture in a
        // second doc and copy the shape in.
        {
            let page = &mut app.ed.as_mut().unwrap().doc.pages[0];
            page.shapes.retain(|s| s.id != 1);
            page.shapes.push(sh);
        }
        app.sel = Some((page_id, 1));
        let handles = app.selection_handles();
        // The NURBSTo row's X/Y vertex handle + one NurbsPt per E
        // group.
        let nurbs_vertex = handles.iter().any(|h| {
            matches!(
                h.id,
                HandleId::GeoPoint {
                    section_ix: 0,
                    row_ix: 2,
                    cell: canvas::GeoCell::X,
                }
            )
        });
        assert!(nurbs_vertex, "the NURBSTo endpoint handle exists");
        let pts: Vec<_> = handles
            .iter()
            .filter(|h| {
                matches!(
                    h.id,
                    HandleId::GeoPoint {
                        cell: canvas::GeoCell::NurbsPt(_),
                        ..
                    }
                )
            })
            .collect();
        assert_eq!(pts.len(), 1, "one NurbsPt per E group: {pts:?}");
        // The interior point (2, 2) local → page through the frame
        // (Pin/LocPin identity here: origin (1,1), no rotation).
        assert!((pts[0].pos.0 - 3.0).abs() < 1e-6, "{}", pts[0].pos.0);
        assert!((pts[0].pos.1 - 3.0).abs() < 1e-6, "{}", pts[0].pos.1);
    }

    /// #187 slice 2: a ROTATED connector joins the jump lines — the
    /// Angle gate is gone; the HitFrame polyline carries the rotated
    /// geometry, so it crosses the axis-aligned line and hops.
    #[test]
    fn rotated_connector_joins_the_jump_lines() {
        let connector = |id: i64, pin: (f64, f64), w: f64, angle: f64| {
            let mut s = vsdx::model::Shape {
                id,
                type_: Some("Shape".into()),
                ..vsdx::model::Shape::default()
            };
            for (n, v) in [
                ("PinX", pin.0),
                ("PinY", pin.1),
                ("Width", w),
                ("Height", 0.0),
                ("LocPinX", w / 2.0),
                ("LocPinY", 0.0),
                ("Angle", angle),
                ("ObjType", 2.0),
            ] {
                s.sheet.set_cell(n, vsdx::Cell::value(format!("{v}")));
            }
            let mut sec = vsdx::model::Section {
                name: "Geometry".into(),
                ix: 0,
                ..vsdx::model::Section::default()
            };
            sec.rows = vec![
                vsdx::model::Row {
                    t: Some("MoveTo".into()),
                    ix: 1,
                    cells: vec![
                        ("X".to_string(), vsdx::Cell::value("0")),
                        ("Y".to_string(), vsdx::Cell::value("0")),
                    ],
                    ..vsdx::model::Row::default()
                },
                vsdx::model::Row {
                    t: Some("LineTo".into()),
                    ix: 2,
                    cells: vec![
                        ("X".to_string(), vsdx::Cell::value(format!("{w}"))),
                        ("Y".to_string(), vsdx::Cell::value("0")),
                    ],
                    ..vsdx::model::Row::default()
                },
            ];
            s.sheet.children.push(vsdx::model::SheetChild::Section(sec));
            s
        };
        let mut ed = dc_core::Editor::new_blank();
        ed.doc.pages[0]
            .shapes
            .push(connector(1, (3.0, 3.0), 4.0, 0.0));
        // The 45° rotated line through pin (3, 2.5) crosses the
        // horizontal at (3.5, 3), strictly inside both.
        ed.doc.pages[0].shapes.push(connector(
            2,
            (3.0, 2.5),
            2.8284271,
            std::f64::consts::FRAC_PI_4,
        ));
        let mut app = App::with_doc_opt(None);
        app.ed = Some(ed);
        app.page_idx = 0;
        let lines = app.jump_lines();
        assert_eq!(
            lines.len(),
            2,
            "the rotated connector joins the jump lines: {}",
            lines.len()
        );
        // And the crossing hops: the axis line (flatter) jumps.
        let jumps = crate::connectors::line_jumps(&lines);
        assert_eq!(jumps.len(), 1, "one crossing: {jumps:?}");
        assert!(jumps[0].dir.1.abs() < 1e-6, "the axis line hops");
    }

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
        // #224: the parse went to the worker — the opening status
        // shows, the poll lands the failure, the status flips.
        assert!(app.status.starts_with("Opening"), "status = {}", app.status);
        assert!(app.debug_loading_for_test());
        // The worker thread races this thread — pump the poll until the
        // result arrives (bounded; the failure path is fast).
        for _ in 0..2000 {
            if app.poll_loading().is_none() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        assert!(
            app.status.starts_with("open failed"),
            "status = {}",
            app.status
        );
        assert!(!app.debug_loading_for_test());
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
        app.debug_load_sync_for_test(&p);
        assert_eq!(
            app.debug_recent(),
            std::slice::from_ref(&p),
            "successful open records the path"
        );
        // Re-opening moves it to the (single) front.
        app.debug_load_sync_for_test(&p);
        assert_eq!(app.debug_recent(), &[p]);
    }

    #[test]
    fn failed_open_does_not_record() {
        let mut app = App::with_doc_opt(None);
        app.debug_load_sync_for_test(&PathBuf::from("/nonexistent/missing.vsdx"));
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
        app.debug_load_sync_for_test(&bad);
        assert!(
            app.status.starts_with("open failed"),
            "status = {}",
            app.status
        );
        assert!(
            !app.debug_recent()
                .iter()
                .any(|p| p.ends_with("corrupt.vsdx")),
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
        DblClick::Deferred(crate::i18n::strs().msg_eventdblclick_f_macro_and_naviga(f))
    }
}

/// "#rrggbb" → rgb triple (the renderer's resolved color form);
/// None on malformed values (#108 line jumps).
fn parse_hex_rgb(s: &str) -> Option<(u8, u8, u8)> {
    let h = s.trim_start_matches('#');
    if h.len() == 6 {
        if let Ok(v) = u32::from_str_radix(h, 16) {
            return Some((
                ((v >> 16) & 0xff) as u8,
                ((v >> 8) & 0xff) as u8,
                (v & 0xff) as u8,
            ));
        }
    }
    None
}

/// #156: a stencil's master by NameU (SDCONT's gallery names).
fn find_master_by_name<'a>(doc: &'a Document, name: &str) -> Option<&'a vsdx::Master> {
    doc.masters
        .iter()
        .find(|m| m.name_u.as_deref() == Some(name))
}
