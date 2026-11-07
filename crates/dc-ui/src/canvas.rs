//! The page canvas: the rendered page as a texture (via the SVG
//! backend), Visio-style rulers with metric ticks, grid overlay,
//! selection frame with handles, drag-to-move, wheel zoom and pan.
//!
//! Gesture arming is PRESS-based (GitHub #59): egui only reports
//! `drag_started` once the pointer is "decidedly dragging" (moved >6 px
//! or held >0.8 s), and its click-vs-drag disambiguation means a
//! natural press-drag-release never produces a `clicked()` — so drags
//! armed on click/drag_started were unreliable no-ops. The canvas arms
//! handle drags, shape moves (via the app), and the marquee on the
//! press frame itself and commits on the release frame.

use egui::{Color32, Pos2, Rect, Sense, Stroke, Ui, Vec2};

use crate::theme::{self, ShellTokens};
use crate::units::{tick_step, tick_tiers, TickTier, Unit};

/// View state for the active page.
#[derive(Debug, Clone)]
pub struct View {
    pub zoom: f32, // 1.0 = 96 px/in
    pub pan: Vec2, // screen offset of the page's top-left corner
    pub show_grid: bool,
    pub show_rulers: bool,
    pub unit: Unit,
    pub fit_pending: bool,
    /// Following the content area: while true, a layout whose content
    /// size changed re-fits (window resize, stencil/ShapeSheet pane
    /// toggles). Manual zoom/pan clears it (Visio-like: resizing must
    /// not clobber the user's view); Fit Page / open / page-switch
    /// re-arm it via `fit_pending`.
    pub fit_mode: bool,
    /// Content size the last fit was computed against.
    pub last_content: Vec2,
}

impl Default for View {
    fn default() -> Self {
        View {
            zoom: 1.0,
            pan: Vec2::ZERO,
            show_grid: true,
            show_rulers: true,
            unit: Unit::Mm,
            fit_pending: true,
            fit_mode: false,
            last_content: Vec2::ZERO,
        }
    }
}

impl View {
    pub fn px_per_in(&self) -> f32 {
        96.0 * self.zoom
    }

    /// Fit zoom + centered pan for a content area (origin = top-left of
    /// the area below/right of any rulers), and enter fit mode.
    pub fn apply_fit(&mut self, content: Vec2, origin: Vec2, page: &PageGeom) {
        let zx = content.x / (page.w_in as f32 * 96.0);
        let zy = content.y / (page.h_in as f32 * 96.0);
        // Snap DOWN onto the raster ppi-bucket grid (the layer store
        // quantizes to ±0.25 ppi = 1/384 zoom steps): the fit's
        // scrollbar-hysteresis relaxation nudges the zoom for a few
        // frames before settling, and every bucket crossing
        // re-rasterized EVERY shape layer. A floored fit keeps the
        // bucket pinned across the nudges — the step itself is
        // imperceptible, and flooring never overflows the content.
        self.zoom = ((zx.min(zy) * 384.0).floor() / 384.0).clamp(0.1, 8.0);
        let pw = page.w_in as f32 * self.px_per_in();
        let ph = page.h_in as f32 * self.px_per_in();
        self.pan = origin
            + Vec2::new((content.x - pw) / 2.0, (content.y - ph) / 2.0).max(Vec2::new(12.0, 12.0));
        self.fit_mode = true;
        self.last_content = content;
    }

    /// Whether a newly laid-out content size should trigger a re-fit:
    /// only in fit mode, and only beyond sub-pixel layout jitter.
    pub fn wants_refit(&self, content: Vec2) -> bool {
        self.fit_mode && (self.last_content - content).length() > 0.5
    }

    /// Zoom into a page rect and center it in the content area
    /// (GitHub #22 — entering in-place text edit focuses the shape's
    /// text block like Visio's F2). `rect` is page inches, y-up
    /// `(x, y, w, h)`. The block is sized to fill roughly half the
    /// content area, clamped to comfortable bounds (≥ 100 % so text
    /// stays legible, ≤ 400 %), and never zooms OUT below the current
    /// zoom. Leaves fit mode (the caller restores the pre-edit view
    /// explicitly when editing ends); a manual zoom during the edit
    /// is respected — this runs once, on entry.
    pub fn zoom_to_rect(
        &mut self,
        rect: (f64, f64, f64, f64),
        content: Vec2,
        origin: Vec2,
        page: &PageGeom,
    ) {
        let (x, y, w, h) = rect;
        // Degenerate blocks (a hairline block on an empty shape) must
        // not produce absurd zooms: floor each side at 0.25 in.
        let w = (w as f32).max(0.25);
        let h = (h as f32).max(0.25);
        let zx = content.x * 0.5 / (w * 96.0);
        let zy = content.y * 0.5 / (h * 96.0);
        let target = zx.min(zy).clamp(1.0, 4.0);
        self.zoom = target.max(self.zoom).clamp(0.1, 8.0);
        // Center the block's page center (x, page_h − y) at the
        // content area's center: pan = center − block_center_screen.
        let cx = x as f32 + w / 2.0;
        let cy = page.h_in as f32 - (y as f32 + h / 2.0);
        let ppi = self.px_per_in();
        self.pan = origin + content / 2.0 - Vec2::new(cx * ppi, cy * ppi);
        self.fit_mode = false;
        self.fit_pending = false;
    }

    /// Zoom so the selection fills the view (GitHub #237 — Visio's
    /// Zoom ▸ Selected): the rect is scaled to FILL the content area
    /// and its center lands at the content center. Unlike the #22
    /// text-edit zoom the clamps are general — a selection larger
    /// than the view zooms OUT to center it, and the result is
    /// bucket-floored like [`View::apply_fit`] so the layer store's
    /// ppi quantization never re-rasters across nudges. Leaves fit
    /// mode; a later manual zoom is respected as always. `rect` is
    /// page inches, y-up `(x, y, w, h)`.
    pub fn zoom_to_selection(
        &mut self,
        rect: (f64, f64, f64, f64),
        content: Vec2,
        origin: Vec2,
        page: &PageGeom,
    ) {
        let (x, y, w, h) = rect;
        // The zoom floors each side at 0.25 in (a hairline selection
        // must not produce an absurd zoom); the CENTER uses the RAW
        // dimensions — a 1D connector's h=0 selection would land
        // 0.125 in off-center otherwise (#218 review).
        let (wf, hf) = ((w as f32).max(0.25), (h as f32).max(0.25));
        let zx = content.x / (wf * 96.0);
        let zy = content.y / (hf * 96.0);
        self.zoom = ((zx.min(zy) * 384.0).floor() / 384.0).clamp(0.1, 8.0);
        let cx = x as f32 + w as f32 / 2.0;
        let cy = page.h_in as f32 - (y as f32 + h as f32 / 2.0);
        let ppi = self.px_per_in();
        self.pan = origin + content / 2.0 - Vec2::new(cx * ppi, cy * ppi);
        self.fit_mode = false;
        self.fit_pending = false;
    }
}

/// Page geometry in inches (y-down on screen after the flip).
pub struct PageGeom {
    pub w_in: f64,
    pub h_in: f64,
}

pub const RULER: f32 = 22.0;

/// The drawable content area of a canvas rect: (origin, size), the
/// origin shifted past the rulers when they are showing, minus the
/// page margin `show`/`apply_fit`/`zoom_to_rect` all keep. Shared so
/// the fit math and the text-edit zoom (#22) target the same pixels.
pub fn content_area(canvas_rect: Rect, show_rulers: bool) -> (Pos2, Vec2) {
    let content_origin = if show_rulers {
        canvas_rect.left_top() + Vec2::new(RULER, RULER)
    } else {
        canvas_rect.left_top()
    };
    let content = Vec2::new(
        (canvas_rect.right() - content_origin.x - 12.0).max(50.0),
        (canvas_rect.bottom() - content_origin.y - 12.0).max(50.0),
    );
    (content_origin, content)
}

/// Ruler subdivision depth (#62): 4 = half + quarter ticks (the Visio
/// default); the Ruler & Grid Fine/Normal/Coarse setting can lower it.
const RULER_SUBDIV: u32 = 4;
/// Minor ruler tiers drop out below this pixel spacing (#62) so ticks
/// never smear together when zooming out.
const RULER_MIN_TICK_PX: f64 = 6.0;

/// The displayed grid's line spacing (page inches) — one definition
/// shared by the grid painting and the snap engine's grid target
/// (#109).
pub fn grid_step_in(view: &View) -> f64 {
    view.unit.to_in(5.0)
}

/// The finest ruler subdivision interval currently drawn (page
/// inches), shared by the rulers and the snap engine's
/// ruler-subdivision target (#109): the tiered-tick logic of #62,
/// which drops the finer halves/quarters as the zoom shrinks.
pub fn ruler_interval_in(view: &View) -> f64 {
    let ppi = view.px_per_in();
    let dupx = view.unit.from_in(1.0) as f32 / ppi; // display units per px
    let step = tick_step(dupx as f64, 45.0);
    let px_per_major = step / dupx as f64;
    let tiers = tick_tiers(step, px_per_major, RULER_SUBDIV, RULER_MIN_TICK_PX);
    let finest = tiers.last().map_or(1, |t: &TickTier| t.div as i64);
    view.unit.to_in(step / finest as f64)
}

/// Outcome of interacting with the canvas this frame.
pub struct CanvasOut {
    pub page_rect: Rect,
    /// Full allocated canvas area (drop target checks).
    pub canvas_rect: Rect,
    /// Click landed on empty page area.
    pub clicked_empty: bool,
    /// Selected shape id from a click (hit test done by the app).
    pub click_page_pos: Option<(f64, f64)>,
    /// Double-click position in page inches (y-up); the app hit-tests
    /// and opens the shape's text editor per `EventDblClick`. Fires on
    /// the second click of the pair, which then does not also report a
    /// plain click.
    pub dbl_click_page_pos: Option<(f64, f64)>,
    /// Right-click position in page inches (y-up); the app hit-tests and
    /// opens the shape context menu.
    pub right_click_page_pos: Option<(f64, f64)>,
    /// A stencil drop happened at this pin position (inches, y-up).
    pub drop_pos: Option<(f64, f64)>,
    /// An AutoConnect arrow was clicked (#108): the unit direction
    /// (page-space, y-up) the new shape goes — right/left/up/down.
    pub autoclick: Option<(f64, f64)>,
    /// A selection-handle drag finished: (handle, press position,
    /// release position), both in page inches y-up. The app applies the
    /// resize / endpoint move / line move.
    pub handle_drag_done: Option<HandleDragDone>,
    /// Primary press (this frame) on a shape, NOT consumed by a handle:
    /// page inches y-up. The app arms its move-drag from this (the
    /// press, not the click — see the module note).
    pub press_page_pos: Option<(f64, f64)>,
    /// Marquee (empty-area drag) finished: rect in page inches, y-up,
    /// (x0, y0, x1, y1) with y0 <= y1.
    pub marquee_done: Option<(f64, f64, f64, f64)>,
    /// Lasso drag finished (#110, Visio's Lasso Select): the freehand
    /// trail in page inches, y-up. The app selects every shape the
    /// trail touches or encloses.
    pub lasso_done: Option<Vec<(f64, f64)>>,
    /// Freeform drag finished (#132, URS-1225): the sampled pen
    /// stroke in page inches, y-up. The app builds the geometry shape.
    pub freeform_done: Option<Vec<(f64, f64)>>,
    /// A ruler drag released over the page (#113 guides slice): the
    /// guide kind and its page position (inches, y-up).
    pub guide_drop: Option<(GuideSource, (f64, f64))>,
    /// The Connector Tool's gesture finished (#108): (press point,
    /// release point), page inches y-up. Valid only when the release
    /// landed on the page.
    pub connector_drop: Option<((f64, f64), (f64, f64))>,
    /// Marquee rect this frame while dragging (page inches, y-down).
    pub marquee_live: Option<Rect>,
    /// Lasso trail this frame while dragging (screen px).
    pub lasso_live: Option<Vec<Pos2>>,
    /// The Freeform tool's live pen trail (screen px, #132).
    pub freeform_live: Option<Vec<Pos2>>,
    /// Shift held at click time (multi-select toggle).
    pub shift_click: bool,
    /// Ctrl/Cmd+click: follow the hit shape's first hyperlink (#28).
    pub ctrl_click: bool,
    pub zoom_changed: bool,
}

/// A selection-frame corner, in screen orientation (y-down).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Corner {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

/// Which ruler a guide is dragged out of (#113 guides slice).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuideSource {
    /// The left (vertical) ruler → a vertical guide.
    Vertical,
    /// The top (horizontal) ruler → a horizontal guide.
    Horizontal,
    /// The ruler corner square → a guide point.
    Point,
}

/// A guide to paint on the canvas overlay (page inches, y-up) —
/// guides never raster as page ink (Visio: no print, no export).
#[derive(Debug, Clone, Copy)]
pub struct GuideFx {
    pub id: i64,
    pub vertical: Option<bool>, // None = guide point
    pub x: f64,
    pub y: f64,
    pub selected: bool,
}

/// One line-jump hop to paint above the ink (#108): page inches
/// y-up, the jumping line's direction, both strokes and colors (the
/// under line is redrawn through the gap so only the JUMPER breaks).
#[derive(Debug, Clone, Copy)]
pub struct JumpFx {
    pub at: (f64, f64),
    pub dir: (f64, f64),
    pub under: (f64, f64),
    pub weight: f64,
    pub under_weight: f64,
    pub color: Color32,
    pub under_color: Color32,
}

/// The guide color: Visio's teal guide lines.
const GUIDE: Color32 = Color32::from_rgb(0x00, 0x8F, 0xA3);

/// One grabbable selection handle (GitHub #59). 2D shapes expose the
/// four resize corners; 1D shapes (lines/connectors, resolved
/// Begin/End cells) expose their two endpoints plus the midpoint.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HandleId {
    Corner(Corner),
    LineBegin,
    LineEnd,
    LineMid,
    /// A geometry row's defining point (#137): the row's X/Y pair,
    /// addressed by Geometry section index and row index on the
    /// SELECTED shape's own sheet.
    /// #157: a row can expose more than its X/Y — curve controls.
    GeoPoint {
        section_ix: i64,
        row_ix: i64,
        cell: GeoCell,
    },
}

/// Which defining value of a geometry row a GeoPoint handle drags
/// (#157): the X/Y end point (a square vertex, #137), an ArcTo's
/// sagitta (a circle control at the arc's apex — the drag recomputes
/// the bulge from the pointer), or an EllipticalArcTo's on-curve
/// control point (a circle control at A/B).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeoCell {
    X,
    Y,
    Apex,
    OnCurve,
    /// A CubicBezTo row's first control (A,B) — #202 curve handles.
    Ctrl,
    /// A CubicBezTo row's second control (C,D) — #202 curve handles.
    Ctrl2,
    /// A NURBSTo row's E-formula interior control point by group
    /// index — #188 handles; the drag re-emits the formula.
    NurbsPt(usize),
}

/// The ArcTo sagitta implied by a pointer position (#157): |A| is the
/// arc's height over the chord, positive bulging RIGHT of travel
/// (the renderer's own convention). Pure page/local-agnostic math.
pub fn arc_sagitta_from_pointer(cur: (f64, f64), end: (f64, f64), p: (f64, f64)) -> f64 {
    let d = (end.0 - cur.0, end.1 - cur.1);
    let len = d.0.hypot(d.1).max(1e-9);
    let right = (d.1 / len, -d.0 / len); // rot90 CW: right of travel
    let mid = ((cur.0 + end.0) / 2.0, (cur.1 + end.1) / 2.0);
    (p.0 - mid.0) * right.0 + (p.1 - mid.1) * right.1
}

/// The ArcTo apex point (where the sagitta handle sits): mid-chord
/// offset by A along the right-of-travel normal.
pub fn arc_apex(cur: (f64, f64), end: (f64, f64), a: f64) -> (f64, f64) {
    let d = (end.0 - cur.0, end.1 - cur.1);
    let len = d.0.hypot(d.1).max(1e-9);
    let right = (d.1 / len, -d.0 / len);
    let mid = ((cur.0 + end.0) / 2.0, (cur.1 + end.1) / 2.0);
    (mid.0 + a * right.0, mid.1 + a * right.1)
}

/// A finished handle drag (GitHub #59): (handle, press position,
/// release position), both in page inches y-up.
pub type HandleDragDone = (HandleId, (f64, f64), (f64, f64));

/// A handle and its position on the page (inches, y-up).
#[derive(Debug, Clone, Copy)]
pub struct Handle {
    pub id: HandleId,
    pub pos: (f64, f64),
}

/// Grab radius around a handle's center, in points. Generous on
/// purpose: the drawn square is ~7 px, but Visio's handles are easy to
/// grab and a tight radius was part of the #59 no-op feel.
pub const HANDLE_GRAB: f32 = 12.0;

/// Screen position → page inches (y-up), regardless of the pointer's
/// position relative to the page.
pub fn screen_to_page(p: Pos2, view: &View, page: &PageGeom) -> (f64, f64) {
    let x = (p.x - view.pan.x) as f64 / view.px_per_in() as f64;
    let y = page.h_in - (p.y - view.pan.y) as f64 / view.px_per_in() as f64;
    (x, y)
}

/// Page inches (y-up) → screen position.
pub fn page_to_screen(x: f64, y: f64, view: &View, page: &PageGeom) -> Pos2 {
    Pos2::new(
        view.pan.x + x as f32 * view.px_per_in(),
        view.pan.y + (page.h_in - y) as f32 * view.px_per_in(),
    )
}

/// Does a lasso trail (page inches, y-up) touch or enclose an
/// axis-aligned rect (#110, Visio's Lasso Select)? True when any
/// segment crosses the rect, any sample lies inside it, or the trail
/// loops around it (the rect's center inside the trail's polygon) —
/// the trail needs only to TOUCH a shape, unlike the marquee's
/// contained/intersect band.
///
/// `closure_tol` (page inches) decides when the trail's ends count as
/// meeting for the enclosure test. The CALLER derives it from the view
/// scale — a hand-drawn loop ends within a few screen pixels of its
/// start, which is a meaningful inch distance only at the current
/// zoom; a fixed inch epsilon would read every human-drawn lasso as
/// open and never enclose.
pub fn lasso_hits_rect(
    trail: &[(f64, f64)],
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    closure_tol: f64,
) -> bool {
    let inside = |p: (f64, f64)| p.0 >= x && p.0 <= x + w && p.1 >= y && p.1 <= y + h;
    let mut prev: Option<(f64, f64)> = None;
    for &p in trail {
        if inside(p) {
            return true;
        }
        if let Some(a) = prev {
            if seg_crosses(a, p, x, y, w, h) {
                return true;
            }
        }
        prev = Some(p);
    }
    // Enclosed: the rect's center inside the trail polygon (ray cast).
    // ENCLOSURE NEEDS A CLOSED LOOP — the trail's ends meeting. An
    // OPEN wavy trail crosses a center's y-level an odd number of
    // times with every crossing to one side, which the winding sums
    // to ±1 and would misread as "surrounded" (found by the MDI
    // re-run of the lasso kittest; the trail's touch points were
    // always right, the loop test was run on a non-loop).
    let closed = trail
        .first()
        .zip(trail.last())
        .is_some_and(|(f, l)| (f.0 - l.0).hypot(f.1 - l.1) <= closure_tol);
    if !closed {
        return false;
    }
    let (cx, cy) = (x + w / 2.0, y + h / 2.0);
    let mut winding = 0i32;
    // Consecutive segments plus the closing last-to-first edge: with a
    // tolerance closure the ends differ, so the edge that completes the
    // loop can itself cross the center's y-level. For an exactly closed
    // trail that edge is degenerate (equal points never cross).
    for i in 0..trail.len() {
        let (a, b) = (trail[i], trail[(i + 1) % trail.len()]);
        if (a.1 <= cy) != (b.1 <= cy) {
            let t = (cy - a.1) / (b.1 - a.1);
            if a.0 + t * (b.0 - a.0) > cx {
                winding += if b.1 > a.1 { 1 } else { -1 };
            }
        }
    }
    winding != 0
}

/// Liang-Barsky: does segment a→b cross or touch the rect?
fn seg_crosses(a: (f64, f64), b: (f64, f64), x: f64, y: f64, w: f64, h: f64) -> bool {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let (mut t0, mut t1) = (0.0f64, 1.0f64);
    for (p, q) in [
        (-dx, a.0 - x),
        (dx, x + w - a.0),
        (-dy, a.1 - y),
        (dy, y + h - a.1),
    ] {
        if p.abs() < 1e-12 {
            if q < 0.0 {
                return false; // parallel and outside this boundary
            }
        } else {
            let r = q / p;
            if p < 0.0 {
                if r > t1 {
                    return false;
                }
                if r > t0 {
                    t0 = r;
                }
            } else {
                if r < t0 {
                    return false;
                }
                if r < t1 {
                    t1 = r;
                }
            }
        }
    }
    true
}

/// Live drag feedback (#96) — drawn above the layers, under nothing:
/// ghost layer quads (moved/stretched textures; a move preview is a
/// quad translation, never a re-raster), dashed screen-space outline
/// fallbacks (also used for the moved selection frame), live 1D line
/// previews, and the dynamic-grid hints of #109. Default/empty = no
/// drag in progress.
#[derive(Default)]
pub struct DragFx {
    /// Layer textures at their live positions (page bbox space).
    pub ghosts: Vec<crate::layers::ShapeQuad>,
    /// Dashed outlines (screen px) — texture-missing fallbacks and the
    /// selection frame following the gesture.
    pub outlines: Vec<Rect>,
    /// Live 1D endpoint previews (screen px segment).
    pub lines: Vec<(Pos2, Pos2)>,
    /// Dynamic-grid alignment/spacing hints (#109, URS-1088), in page
    /// inches y-up — painted as dashed full-canvas alignment lines and
    /// labeled gap segments while a drag is live.
    pub hints: Vec<crate::snap::Hint>,
    /// #226: a styled endpoint-drag ghost is live — the bare
    /// begin→end live line stands down (the ghost IS the styled ink).
    pub endpoint_ghost: bool,
}

/// The currently armed selection handle — the app reads the same
/// egui temp slot the canvas arms on press (#226: the live
/// endpoint-drag preview keys off a `LineBegin`/`LineEnd` gesture).
pub fn armed_handle(ctx: &egui::Context) -> Option<HandleId> {
    ctx.memory(|m| m.data.get_temp::<HandleId>(egui::Id::unique("dc-handle-drag")))
}

/// The dynamic-grid hint color: Visio's blue dashed alignment aid.
const HINT: Color32 = Color32::from_rgb(0x2E, 0x75, 0xB6);

/// Visio's glue green (#177): the live glue-target square and the
/// selected connector's glued-site circles.
const GLUE: Color32 = Color32::from_rgb(0x00, 0xB0, 0x50);
/// The reveal symbol's gray (#177): the connection-point
/// square-with-× on approached shapes.
const CONN_PT: Color32 = Color32::from_rgb(0x59, 0x59, 0x59);

/// Connection-point affordances to paint above the ink (#177). The
/// app computes them (pure logic in `connectors.rs`); the argument is
/// the LIVE endpoint position when a connector-endpoint gesture is in
/// progress (Connector Tool draw or a LineBegin/LineEnd handle drag),
/// None otherwise — `None` still carries the selected connector's
/// glued sites.
pub type ConnFxFn<'a> = &'a dyn Fn(Option<(f64, f64)>) -> crate::connectors::ConnPointFx;

/// Live endpoint snapping (#179): page inches in, the snapped position
/// out. The bool is "an endpoint handle of the SELECTED connector is
/// the dragged thing" (the app then excludes that shape from the snap
/// targets); the Connector Tool's fresh draw excludes nothing. Visio
/// snaps the dragged connector end onto connection points and the
/// other glue/snap targets while dragging.
pub type SnapEndpointFn<'a> = &'a dyn Fn(bool, (f64, f64)) -> (f64, f64);

/// Corner-resize snapping (#160): page inches in, the per-axis snapped
/// position out (None = no target on either axis, keep the raw
/// pointer). The app builds this against the page minus the selected
/// shape — its own edges must not attract its own corner. Feeds the
/// live corner preview; the release commits through the same function
/// on the app side, so the two agree by construction.
pub type SnapResizeFn<'a> = &'a dyn Fn((f64, f64)) -> Option<(f64, f64)>;

/// A dashed segment (`on`/`off` in px), the dynamic-grid line style.
fn dashed(p: &egui::Painter, a: Pos2, b: Pos2, color: Color32) {
    let len = (b - a).length();
    if len < 1.0 {
        return;
    }
    let dir = (b - a) / len;
    let (on, off) = (6.0, 4.0);
    let mut t = 0.0;
    while t < len {
        let t2 = (t + on).min(len);
        p.line_segment([a + dir * t, a + dir * t2], Stroke::new(1.0, color));
        t += on + off;
    }
}

/// Draw the whole canvas area (fills `ui`).
#[allow(clippy::too_many_arguments)]
pub fn show(
    ui: &mut Ui,
    view: &mut View,
    page: &PageGeom,
    layers: &[crate::layers::ShapeQuad],
    sel_rect: Option<Rect>,   // primary selection (page-inches, y-down)
    extra_sel_rects: &[Rect], // other multi-selection members
    member_hl: &[Rect],       // #156: selected container's members (page-inches, y-down)
    handles: &[Handle],       // grab points for the primary selection
    drag: &DragFx,            // live gesture feedback (#96)
    guides: &[GuideFx],       // #113: guide overlay, above ink
    jumps: &[JumpFx],         // #108: line-jump hops, above ink
    underlays: &[crate::layers::ShapeQuad], // #113: background pages, beneath ink
    connector_tool: bool,     // #108: the Connector Tool is active
    lasso_tool: bool,         // #110: the Lasso Select tool is active
    freeform_tool: bool,      // #132: the Freeform tool is active
    stencil_hover: bool,
    // #202: the selected 1D shape's geometry carries a curve row —
    // the selection layer then skips its begin→end chord stroke (the
    // ink is the line; a straight chord over it reads as flattened).
    sel_line_curved: bool,
    // #177: connection-point reveal / glue highlight / glued sites,
    // computed by the app for the live endpoint position (None when
    // no endpoint gesture is live).
    conn_fx: ConnFxFn<'_>,
    // #179: snap the live connector endpoint (preview + drops).
    snap_endpoint: SnapEndpointFn<'_>,
    // #160: snap the live corner-resize preview (per-axis, snap side).
    snap_resize: SnapResizeFn<'_>,
    hit_shape: &dyn Fn((f64, f64)) -> bool, // page inches y-up, for marquee gating
    mouse_page: &mut Option<(f64, f64)>,    // y-up inches, for the status bar
    interaction_lock: bool, // in-place text edit (#22): clicks end it, nothing else arms
) -> CanvasOut {
    let mut out = CanvasOut {
        page_rect: Rect::NOTHING,
        canvas_rect: Rect::NOTHING,
        clicked_empty: false,
        click_page_pos: None,
        dbl_click_page_pos: None,
        right_click_page_pos: None,
        drop_pos: None,
        autoclick: None,
        handle_drag_done: None,
        press_page_pos: None,
        marquee_done: None,
        lasso_done: None,
        freeform_done: None,
        guide_drop: None,
        connector_drop: None,
        marquee_live: None,
        lasso_live: None,
        freeform_live: None,
        shift_click: false,
        ctrl_click: false,
        zoom_changed: false,
    };
    let avail = ui.available_size();
    let (canvas_r, resp) = ui.allocate_exact_size(avail, Sense::click_and_drag());
    out.canvas_rect = canvas_r;

    // Content area origin (below/right of rulers).
    let (content_origin, content) = content_area(canvas_r, view.show_rulers);

    // Wheel zoom — a manual zoom: leaves fit mode so later resizes
    // keep the user's view. With an active selection the anchor is the
    // selection's center (the union of the primary + multi-select
    // rects — the selection is what the user is working on, #237);
    // without one the hovered point anchors, as before.
    if let Some(hp) = resp.hover_pos() {
        let scroll = ui.input(|i| i.smooth_scroll_delta.y);
        if scroll != 0.0 {
            let factor = if scroll > 0.0 { 1.1 } else { 1.0 / 1.1 };
            // The anchor's screen position uses the PRE-zoom ppi — the
            // selection's on-screen center before this step.
            let old_ppi = view.px_per_in();
            let anchor = sel_rect
                .into_iter()
                .chain(extra_sel_rects.iter().copied())
                .reduce(|a, b| a.union(b))
                .map(|r| {
                    Pos2::new(
                        view.pan.x + r.center().x * old_ppi,
                        view.pan.y + r.center().y * old_ppi,
                    )
                })
                .unwrap_or(hp);
            let old = view.zoom;
            view.zoom = (view.zoom * factor).clamp(0.1, 8.0);
            let k = view.zoom / old;
            // Keep the anchor point stationary — the hovered pixel, or
            // the selection's center when a selection is active.
            view.pan = anchor.to_vec2() - (anchor.to_vec2() - view.pan) * k;
            view.fit_mode = false;
            out.zoom_changed = true;
        }
    }

    // Fit on first layout, and — while in fit mode — again whenever the
    // content area itself changes size (window resize, side panes
    // toggling), so the page stays centered and fully visible.
    if view.fit_pending || view.wants_refit(content) {
        view.apply_fit(content, content_origin.to_vec2(), page);
        view.fit_pending = false;
        out.zoom_changed = true;
    }

    // Middle-drag or space-drag pans — manual: leaves fit mode.
    let pan_drag = resp.dragged_by(egui::PointerButton::Middle)
        || (ui.input(|i| i.modifiers.command) && resp.dragged());
    if pan_drag {
        view.pan += resp.drag_delta();
        view.fit_mode = false;
    }

    // ---- gesture arming, on the PRESS frame (see module note) ----
    // Handles in screen coords; a press inside HANDLE_GRAB of one wins
    // over everything (resize takes precedence over move, #59).
    let handles_screen: Vec<(HandleId, Pos2)> = handles
        .iter()
        .map(|h| (h.id, page_to_screen(h.pos.0, h.pos.1, view, page)))
        .collect();
    let handle_at = |pos: Pos2| -> Option<HandleId> {
        handles_screen
            .iter()
            .find(|(_, p)| pos.distance(*p) <= HANDLE_GRAB)
            .map(|(id, _)| *id)
    };
    // #108 AutoConnect arrows: one triangle at each mid-edge of the
    // primary selection while the pointer hovers the shape (Visio's
    // blue AutoConnect tags). Returns (screen rect, page-space unit
    // direction) per edge; the app draws the duplicate+connector on a
    // click.
    // sel_rect is page-inches (y-down viewBox); the arrows live in
    // screen px — map the box the way `to_screen` (below) does.
    let sel_screen = sel_rect.map(|sr| {
        let tl = Pos2::new(
            view.pan.x + sr.left() * view.px_per_in(),
            view.pan.y + sr.top() * view.px_per_in(),
        );
        Rect::from_min_size(
            tl,
            Vec2::new(
                sr.width() * view.px_per_in(),
                sr.height() * view.px_per_in(),
            ),
        )
    });
    let autoc_arrows: Option<Vec<(Rect, (f64, f64))>> = sel_screen.map(|sr| {
        let c = sr.center();
        [
            (Pos2::new(sr.right() + 9.0, c.y), (1.0, 0.0)),
            (Pos2::new(sr.left() - 9.0, c.y), (-1.0, 0.0)),
            (Pos2::new(c.x, sr.top() - 9.0), (0.0, 1.0)),
            (Pos2::new(c.x, sr.bottom() + 9.0), (0.0, -1.0)),
        ]
        .into_iter()
        .map(|(pos, dir)| (Rect::from_center_size(pos, Vec2::splat(18.0)), dir))
        .collect()
    });
    let arrow_at = |pos: Pos2| -> Option<(f64, f64)> {
        autoc_arrows
            .as_ref()?
            .iter()
            .find(|(r, _)| r.contains(pos))
            .map(|(_, d)| *d)
    };
    let hover_in_sel = resp
        .hover_pos()
        .is_some_and(|p| sel_screen.is_some_and(|sr| sr.contains(p)) || arrow_at(p).is_some());

    // Stable literal ids are intended for the drag-state memory keys.
    let mem_handle = egui::Id::unique("dc-handle-drag");
    let mem_start = egui::Id::unique("dc-drag-start");
    let mem_marquee = egui::Id::unique("dc-marquee");
    let mem_lasso = egui::Id::unique("dc-lasso");
    let mem_freeform = egui::Id::unique("dc-freeform");
    let mem_guide = egui::Id::unique("dc-guide-drag");
    let mem_connector = egui::Id::unique("dc-connector-drag");
    let mem_autoc = egui::Id::unique("dc-autoclick");
    let primary_pressed = ui.input(|i| i.pointer.button_pressed(egui::PointerButton::Primary));
    let primary_released = ui.input(|i| i.pointer.button_released(egui::PointerButton::Primary));

    if primary_pressed && resp.contains_pointer() && !interaction_lock {
        // `press_origin` is where this press began (set by the press
        // event itself), so it is always Some on this frame.
        let p = ui
            .input(|i| i.pointer.press_origin())
            .or_else(|| ui.input(|i| i.pointer.interact_pos()));
        if let Some(p) = p {
            ui.ctx().memory_mut(|m| m.data.insert_temp(mem_start, p));
            // A Ctrl/Cmd press starts a pan gesture (see pan_drag
            // above): it must not arm a marquee or a move-drag, or the
            // release commits a selection clobbering pan. A press on a
            // selection handle still wins (#59) — handles keep
            // priority over the pan.
            if let Some(id) = handle_at(p) {
                ui.ctx().memory_mut(|m| m.data.insert_temp(mem_handle, id));
            } else if let Some(dir) = arrow_at(p) {
                ui.ctx().memory_mut(|m| m.data.insert_temp(mem_autoc, dir));
            } else if connector_tool {
                // The Connector Tool (#108) owns the gesture: press
                // anywhere (on a shape = glue start), release on the
                // page to draw the connector.
                let page_pos = screen_to_page(p, view, page);
                ui.ctx()
                    .memory_mut(|m| m.data.insert_temp(mem_connector, page_pos));
            } else if view.show_rulers
                && (p.x < canvas_r.left() + RULER || p.y < canvas_r.top() + RULER)
            {
                // A ruler drag pulls a guide out (#113): the corner
                // square yields a guide point, the left band below it
                // a vertical guide, the top band a horizontal one.
                let src = if p.x < canvas_r.left() + RULER && p.y < canvas_r.top() + RULER {
                    GuideSource::Point
                } else if p.x < canvas_r.left() + RULER {
                    GuideSource::Vertical
                } else {
                    GuideSource::Horizontal
                };
                ui.ctx().memory_mut(|m| m.data.insert_temp(mem_guide, src));
            } else if !ui.input(|i| i.modifiers.command) {
                let page_pos = screen_to_page(p, view, page);
                if hit_shape(page_pos) {
                    // The app arms its move-drag from the press.
                    out.press_page_pos = Some(page_pos);
                } else if lasso_tool {
                    // Empty page with the Lasso Select tool (#110):
                    // the press starts a freehand trail.
                    ui.ctx()
                        .memory_mut(|m| m.data.insert_temp(mem_lasso, vec![p]));
                } else if freeform_tool {
                    // Empty page with the Freeform tool (#132): the
                    // press starts the pen stroke.
                    ui.ctx()
                        .memory_mut(|m| m.data.insert_temp(mem_freeform, vec![p]));
                } else {
                    // Empty page: marquee select.
                    ui.ctx()
                        .memory_mut(|m| m.data.insert_temp(mem_marquee, true));
                }
            }
        }
    }

    // Commit an armed handle drag on the release frame. egui's own
    // drag_stopped() needs the drag to have "officially" started (>6 px
    // of travel or >0.8 s hold), which tiny precise drags never reach —
    // so the arm/commit pair is tracked here instead (#59).
    // AutoConnect arrow click (#108): the arm commits when the
    // release stays inside the arrow.
    let armed_autoc = ui
        .ctx()
        .memory(|m| m.data.get_temp::<(f64, f64)>(mem_autoc));
    if primary_released || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        ui.ctx()
            .memory_mut(|m| m.data.remove::<(f64, f64)>(mem_autoc));
        if let (Some(dir), true) = (armed_autoc, primary_released) {
            if let Some(p) = ui.input(|i| i.pointer.interact_pos()) {
                if arrow_at(p) == Some(dir) {
                    out.autoclick = Some(dir);
                }
            }
        }
    }
    let armed_handle = ui.ctx().memory(|m| m.data.get_temp::<HandleId>(mem_handle));
    if primary_released || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        ui.ctx()
            .memory_mut(|m| m.data.remove::<HandleId>(mem_handle));
        if let (Some(id), true) = (armed_handle, primary_released) {
            let start = ui.ctx().memory(|m| m.data.get_temp::<Pos2>(mem_start));
            let end = ui.input(|i| i.pointer.interact_pos());
            if let (Some(s), Some(e)) = (start, end) {
                out.handle_drag_done = Some((
                    id,
                    screen_to_page(s, view, page),
                    screen_to_page(e, view, page),
                ));
            }
        }
    }

    // Guide drag (#113): armed on a ruler press above; a dashed
    // preview line/point follows the pointer while the button is
    // held; releasing over the page drops the guide, releasing
    // anywhere else cancels (Visio drops only onto the page).
    let guide_armed = ui
        .ctx()
        .memory(|m| m.data.get_temp::<GuideSource>(mem_guide));
    let mut guide_live: Option<(GuideSource, (f64, f64))> = None;
    if let Some(src) = guide_armed {
        if primary_released || !ui.input(|i| i.pointer.primary_down()) {
            ui.ctx()
                .memory_mut(|m| m.data.remove::<GuideSource>(mem_guide));
            if primary_released {
                if let Some(p) = ui.input(|i| i.pointer.interact_pos()) {
                    let page_pos = screen_to_page(p, view, page);
                    if page_pos.0 >= 0.0
                        && page_pos.0 <= page.w_in
                        && page_pos.1 >= 0.0
                        && page_pos.1 <= page.h_in
                    {
                        out.guide_drop = Some((src, page_pos));
                    }
                }
            }
        } else if let Some(p) = ui.input(|i| i.pointer.interact_pos()) {
            guide_live = Some((src, screen_to_page(p, view, page)));
        }
    }

    // Connector Tool gesture (#108): rubber line from the press point
    // to the pointer; release on the page draws the connector (the
    // app decides glue from both endpoints), release off the page
    // cancels.
    let connector_armed = ui
        .ctx()
        .memory(|m| m.data.get_temp::<(f64, f64)>(mem_connector));
    let mut connector_live: Option<((f64, f64), (f64, f64))> = None;
    if let Some(from) = connector_armed {
        if primary_released || !ui.input(|i| i.pointer.primary_down()) {
            ui.ctx()
                .memory_mut(|m| m.data.remove::<(f64, f64)>(mem_connector));
            if primary_released {
                if let Some(p) = ui.input(|i| i.pointer.interact_pos()) {
                    let to = snap_endpoint(false, screen_to_page(p, view, page));
                    if to.0 >= 0.0 && to.0 <= page.w_in && to.1 >= 0.0 && to.1 <= page.h_in {
                        out.connector_drop = Some((from, to));
                    }
                }
            }
        } else if let Some(p) = ui.input(|i| i.pointer.interact_pos()) {
            // #179: the rubber end snaps like the drop will — Visio
            // magnetizes the live endpoint to glue targets.
            let to = snap_endpoint(false, screen_to_page(p, view, page));
            connector_live = Some((from, to));
        }
    }

    // Lasso (#110, Visio's Lasso Select): armed on an empty-page press
    // while the tool is active; the freehand trail follows the pointer;
    // release selects every shape the trail touches or encloses. A
    // near-zero-length trail is just a click on empty page — the click
    // path handles selection.
    let lasso_armed = ui.ctx().memory(|m| m.data.get_temp::<Vec<Pos2>>(mem_lasso));
    if let Some(mut trail) = lasso_armed {
        if primary_released || !ui.input(|i| i.pointer.primary_down()) {
            ui.ctx()
                .memory_mut(|m| m.data.remove::<Vec<Pos2>>(mem_lasso));
            if primary_released {
                let len_px: f32 = trail.windows(2).map(|w| w[0].distance(w[1])).sum();
                if len_px > 2.0 && trail.len() >= 2 {
                    out.lasso_done = Some(
                        trail
                            .iter()
                            .map(|p| screen_to_page(*p, view, page))
                            .collect(),
                    );
                }
            }
        } else if let Some(p) = ui.input(|i| i.pointer.interact_pos()) {
            // Extend the trail; near-duplicate samples only bloat it.
            if trail.last().is_none_or(|l| l.distance(p) > 0.75) {
                trail.push(p);
                ui.ctx()
                    .memory_mut(|m| m.data.insert_temp(mem_lasso, trail.clone()));
            }
            out.lasso_live = Some(trail);
        }
    }

    // Freeform (#132, URS-1225): armed on an empty-page press while
    // the tool is active; the pen trail samples the pointer; release
    // hands the stroke to the app, which builds the geometry shape.
    let freeform_armed = ui
        .ctx()
        .memory(|m| m.data.get_temp::<Vec<Pos2>>(mem_freeform));
    if let Some(mut trail) = freeform_armed {
        if primary_released || !ui.input(|i| i.pointer.primary_down()) {
            ui.ctx()
                .memory_mut(|m| m.data.remove::<Vec<Pos2>>(mem_freeform));
            if primary_released {
                let len_px: f32 = trail.windows(2).map(|w| w[0].distance(w[1])).sum();
                if len_px > 2.0 && trail.len() >= 2 {
                    out.freeform_done = Some(
                        trail
                            .iter()
                            .map(|p| screen_to_page(*p, view, page))
                            .collect(),
                    );
                }
            }
        } else if let Some(p) = ui.input(|i| i.pointer.interact_pos()) {
            if trail.last().is_none_or(|l| l.distance(p) > 0.75) {
                trail.push(p);
                ui.ctx()
                    .memory_mut(|m| m.data.insert_temp(mem_freeform, trail.clone()));
            }
            out.freeform_live = Some(trail);
        }
    }

    // Marquee: armed on an empty-page press above; live band while the
    // button is held, committed on release. A degenerate (zero-area)
    // band is just a click on empty page — the click path already
    // handles selection, and running an intersection on a point can
    // MISS zero-height 1D bboxes whose cells were rounded to 6 decimals
    // (#59), clobbering a fresh placement's selection.
    let marqueeing = ui
        .ctx()
        .memory(|m| m.data.get_temp::<bool>(mem_marquee))
        .unwrap_or(false);
    if marqueeing {
        if primary_released || !ui.input(|i| i.pointer.primary_down()) {
            ui.ctx().memory_mut(|m| m.data.remove::<bool>(mem_marquee));
            let start = ui.ctx().memory(|m| m.data.get_temp::<Pos2>(mem_start));
            let end = ui.input(|i| i.pointer.interact_pos());
            if let (Some(start), Some(end)) = (start, end) {
                let (sx, sy) = screen_to_page(start, view, page);
                let (ex, ey) = screen_to_page(end, view, page);
                if (ex - sx).abs() > 1e-6 || (ey - sy).abs() > 1e-6 {
                    let (y0, y1) = if sy < ey { (sy, ey) } else { (ey, sy) };
                    out.marquee_done = Some((sx.min(ex), y0, sx.max(ex), y1));
                }
            }
        } else if let Some(start) = ui.ctx().memory(|m| m.data.get_temp::<Pos2>(mem_start)) {
            if let Some(cur) = ui.input(|i| i.pointer.interact_pos()) {
                out.marquee_live = Some(Rect::from_two_pos(start, cur));
            }
        }
    }

    // The Connector Tool draws with a crosshair (Visio); the Lasso
    // trails freehand, also a crosshair.
    if connector_tool || lasso_tool {
        ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::Crosshair);
    }
    // AutoConnect arrow cursor.
    if let Some(hp) = resp.hover_pos() {
        if arrow_at(hp).is_some() {
            ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::PointingHand);
        }
    }
    // Handle cursors.
    if let Some(hp) = resp.hover_pos() {
        if let Some(c) = handle_at(hp) {
            ui.output_mut(|o| {
                o.cursor_icon = match c {
                    HandleId::Corner(Corner::TopLeft | Corner::BottomRight) => {
                        egui::CursorIcon::ResizeNwSe
                    }
                    HandleId::Corner(Corner::TopRight | Corner::BottomLeft) => {
                        egui::CursorIcon::ResizeNeSw
                    }
                    HandleId::LineBegin | HandleId::LineEnd => egui::CursorIcon::Crosshair,
                    HandleId::LineMid => egui::CursorIcon::AllScroll,
                    HandleId::GeoPoint { .. } => egui::CursorIcon::Crosshair,
                }
            });
        }
    }

    let page_px = Vec2::new(
        page.w_in as f32 * view.px_per_in(),
        page.h_in as f32 * view.px_per_in(),
    );
    let page_tl = view.pan.to_pos2();
    out.page_rect = Rect::from_min_size(page_tl, page_px);

    // Paint order: desk, page shadow+paper, texture, grid, selection.
    // The desk and rulers follow the visual mode; the page paper stays
    // white — it is the document, not the shell (GitHub #52).
    let t = theme::shell_tokens(ui.visuals());
    let p = ui.painter().with_clip_rect(canvas_r);
    p.rect_filled(canvas_r, 0.0, t.desk);
    // shadow
    p.rect_filled(
        Rect::from_min_size(page_tl + Vec2::splat(3.0), page_px),
        0.0,
        Color32::from_black_alpha(28),
    );
    p.rect_filled(out.page_rect, 0.0, Color32::WHITE);
    p.rect_stroke(
        out.page_rect,
        0.0,
        Stroke::new(1.0, t.page_edge),
        egui::StrokeKind::Middle,
    );
    // Background pages (#113, URS-1158): stacked beneath everything
    // drawn on this page — deepest background first.
    for q in underlays {
        let tl = page_to_screen(q.bbox.0, page.h_in - q.bbox.1, view, page);
        let br = page_to_screen(
            q.bbox.0 + q.bbox.2,
            page.h_in - (q.bbox.1 + q.bbox.3),
            view,
            page,
        );
        p.image(
            q.tex,
            Rect::from_two_pos(tl, br),
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
            Color32::WHITE,
        );
    }
    // Per-shape layers (#93), z-order = model order: each quad is the
    // layer texture at its bounds (y-down viewBox inches —
    // page_to_screen flips to its y-up convention).
    for q in layers {
        let tl = page_to_screen(q.bbox.0, page.h_in - q.bbox.1, view, page);
        let br = page_to_screen(
            q.bbox.0 + q.bbox.2,
            page.h_in - (q.bbox.1 + q.bbox.3),
            view,
            page,
        );
        p.image(
            q.tex,
            Rect::from_two_pos(tl, br),
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
            Color32::WHITE,
        );
    }

    // Grid (5 mm spacing in display units).
    if view.show_grid {
        let step_px = grid_step_in(view) as f32 * view.px_per_in();
        if step_px >= 5.0 {
            let mut x = out.page_rect.left() + step_px;
            while x < out.page_rect.right() {
                p.line_segment(
                    [
                        Pos2::new(x, out.page_rect.top()),
                        Pos2::new(x, out.page_rect.bottom()),
                    ],
                    Stroke::new(0.5, Color32::from_gray(216)),
                );
                x += step_px;
            }
            let mut y = out.page_rect.top() + step_px;
            while y < out.page_rect.bottom() {
                p.line_segment(
                    [
                        Pos2::new(out.page_rect.left(), y),
                        Pos2::new(out.page_rect.right(), y),
                    ],
                    Stroke::new(0.5, Color32::from_gray(216)),
                );
                y += step_px;
            }
        }
    }

    // Guides (#113): dashed teal lines / point crosshairs above the
    // ink, below the selection chrome. A selected guide draws solid;
    // a live ruler-drag preview follows the pointer.
    let draw_guide = |g: GuideFx, live: bool| {
        let color = if g.selected {
            Color32::from_rgb(0x00, 0x6D, 0x7D)
        } else {
            GUIDE
        };
        match g.vertical {
            Some(true) => {
                let x = page_to_screen(g.x, 0.0, view, page).x;
                let (a, b) = (
                    Pos2::new(x, out.page_rect.top()),
                    Pos2::new(x, out.page_rect.bottom()),
                );
                if g.selected || live {
                    p.line_segment([a, b], Stroke::new(1.2, color));
                } else {
                    dashed(&p, a, b, color);
                }
            }
            Some(false) => {
                let y = page_to_screen(0.0, g.y, view, page).y;
                let (a, b) = (
                    Pos2::new(out.page_rect.left(), y),
                    Pos2::new(out.page_rect.right(), y),
                );
                if g.selected || live {
                    p.line_segment([a, b], Stroke::new(1.2, color));
                } else {
                    dashed(&p, a, b, color);
                }
            }
            None => {
                // Guide point: a small crosshair.
                let c = page_to_screen(g.x, g.y, view, page);
                let r = if g.selected || live { 7.0 } else { 5.0 };
                p.line_segment(
                    [c + Vec2::new(-r, 0.0), c + Vec2::new(r, 0.0)],
                    Stroke::new(1.2, color),
                );
                p.line_segment(
                    [c + Vec2::new(0.0, -r), c + Vec2::new(0.0, r)],
                    Stroke::new(1.2, color),
                );
            }
        }
    };
    for g in guides {
        draw_guide(*g, false);
    }
    if let Some((src, pos)) = guide_live {
        let g = match src {
            GuideSource::Vertical => GuideFx {
                id: -1,
                vertical: Some(true),
                x: pos.0,
                y: 0.0,
                selected: false,
            },
            GuideSource::Horizontal => GuideFx {
                id: -1,
                vertical: Some(false),
                x: 0.0,
                y: pos.1,
                selected: false,
            },
            GuideSource::Point => GuideFx {
                id: -1,
                vertical: None,
                x: pos.0,
                y: pos.1,
                selected: false,
            },
        };
        draw_guide(g, true);
    }

    // Selection frame (page-inch rect → screen).
    let to_screen = |r: Rect| {
        let tl = Pos2::new(
            view.pan.x + r.left() * view.px_per_in(),
            view.pan.y + r.top() * view.px_per_in(),
        );
        Rect::from_min_size(
            tl,
            Vec2::new(r.width() * view.px_per_in(), r.height() * view.px_per_in()),
        )
    };
    for er in extra_sel_rects.iter().map(|r| to_screen(*r)) {
        p.rect_stroke(
            er,
            0.0,
            Stroke::new(1.0, t.selection),
            egui::StrokeKind::Middle,
        );
    }
    // 1D selection: Visio shows the segment itself plus endpoint
    // handles, not a bounding box. The handle set tells them apart.
    let line_handles: Option<(Pos2, Pos2)> = {
        let beg = handles_screen
            .iter()
            .find(|(id, _)| *id == HandleId::LineBegin)
            .map(|(_, p)| *p);
        let end = handles_screen
            .iter()
            .find(|(id, _)| *id == HandleId::LineEnd)
            .map(|(_, p)| *p);
        beg.zip(end)
    };
    if let Some((beg, end)) = line_handles {
        // #202: a curved connector's ink IS the line — stroking the
        // straight begin→end chord over it reads as "the drag
        // flattened the curve". Only all-straight rows keep the chord.
        if !sel_line_curved {
            p.line_segment([beg, end], Stroke::new(1.2, t.selection));
        }
    } else if let Some(sr) = sel_rect.map(to_screen) {
        p.rect_stroke(
            sr,
            0.0,
            Stroke::new(1.2, t.selection),
            egui::StrokeKind::Middle,
        );
    }
    // Handles: resize corners are squares; 1D endpoints are Visio's
    // larger filled squares and the midpoint an open circle.
    for (id, pos) in &handles_screen {
        match id {
            // Geometry vertex handles (#137): Visio's small filled
            // vertex squares; #157's curve CONTROLS are circles (Visio
            // draws control handles round).
            HandleId::GeoPoint { cell, .. } => match cell {
                GeoCell::X | GeoCell::Y => {
                    p.rect_filled(
                        Rect::from_center_size(*pos, Vec2::splat(5.0)),
                        1.0,
                        t.selection,
                    );
                }
                GeoCell::Apex
                | GeoCell::OnCurve
                | GeoCell::Ctrl
                | GeoCell::Ctrl2
                | GeoCell::NurbsPt(_) => {
                    p.circle_filled(*pos, 4.0, t.selection);
                    p.circle_stroke(*pos, 5.0, Stroke::new(1.0, Color32::WHITE));
                }
            },
            HandleId::Corner(_) => {
                p.rect_filled(
                    Rect::from_center_size(*pos, Vec2::splat(7.0)),
                    1.0,
                    Color32::WHITE,
                );
                p.rect_stroke(
                    Rect::from_center_size(*pos, Vec2::splat(7.0)),
                    1.0,
                    Stroke::new(1.2, t.selection),
                    egui::StrokeKind::Middle,
                );
            }
            HandleId::LineBegin | HandleId::LineEnd => {
                p.rect_filled(
                    Rect::from_center_size(*pos, Vec2::splat(8.0)),
                    1.5,
                    Color32::WHITE,
                );
                p.rect_stroke(
                    Rect::from_center_size(*pos, Vec2::splat(8.0)),
                    1.5,
                    Stroke::new(1.2, t.selection),
                    egui::StrokeKind::Middle,
                );
            }
            HandleId::LineMid => {
                let r = 4.0;
                p.circle_filled(*pos, r, Color32::WHITE);
                p.circle_stroke(*pos, r, Stroke::new(1.2, t.selection));
            }
        }
    }

    // Drag preview.
    // Live drag feedback (#96): ghost layer quads at their live
    // positions (same mapping as the static layers), dashed outline
    // fallbacks, and live 1D endpoint previews. Visio moves the shape
    // under the cursor for the whole gesture; the old dashed-only
    // preview also drew at INCH coordinates as pixels — invisible.
    let quad_to_screen = |q: &crate::layers::ShapeQuad| {
        let tl = page_to_screen(q.bbox.0, page.h_in - q.bbox.1, view, page);
        let br = page_to_screen(
            q.bbox.0 + q.bbox.2,
            page.h_in - (q.bbox.1 + q.bbox.3),
            view,
            page,
        );
        Rect::from_two_pos(tl, br)
    };
    let draw_ghost = |q: &crate::layers::ShapeQuad| {
        p.image(
            q.tex,
            quad_to_screen(q),
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
            Color32::WHITE,
        );
    };
    // Handle-drag previews are computed here (the gesture state —
    // armed handle, pointer, handles — lives in this scope); the app
    // supplies the move-drag ghosts through `drag`.
    let mut outlines: Vec<Rect> = drag.outlines.clone();
    let mut lines: Vec<(Pos2, Pos2)> = drag.lines.clone();
    let mut handle_ghosts: Vec<crate::layers::ShapeQuad> = Vec::new();
    if let (Some(hid), Some(ptr)) = (armed_handle, ui.input(|i| i.pointer.interact_pos())) {
        let ptr_page = screen_to_page(ptr, view, page); // y-up inches
                                                        // The primary's landed layer quad (matched by bbox center —
                                                        // the ink bbox and the selection rect share it).
        let primary_quad = sel_rect.and_then(|sr| {
            let (cx, cy) = (sr.center().x as f64, sr.center().y as f64);
            layers.iter().find(|q| {
                (q.bbox.0 + q.bbox.2 / 2.0 - cx).abs() < 0.1
                    && (q.bbox.1 + q.bbox.3 / 2.0 - cy).abs() < 0.1
            })
        });
        match hid {
            HandleId::Corner(c) => {
                if let Some(sr) = sel_rect {
                    // #160: the preview follows the SNAPPED corner —
                    // the release commits through the app's identical
                    // per-axis snap, so the drop lands where the
                    // preview drew it. The snap runs on the CORNER
                    // position (handle origin + drag delta), not the
                    // raw pointer — a grab up to HANDLE_GRAB off the
                    // corner must not leave the edge at grid + offset
                    // (CodeRabbit; matches the commit's math).
                    let corner_page = handles
                        .iter()
                        .find(|h| h.id == hid)
                        .map(|h| h.pos)
                        .and_then(|o| {
                            ui.ctx()
                                .memory(|m| m.data.get_temp::<Pos2>(mem_start))
                                .map(|st| screen_to_page(st, view, page))
                                .map(|st| (o.0 + ptr_page.0 - st.0, o.1 + ptr_page.1 - st.1))
                        })
                        .unwrap_or(ptr_page);
                    let ptr_page = snap_resize(corner_page).unwrap_or(ptr_page);
                    let ptr_y_down = page.h_in - ptr_page.1;
                    let (mut x0, mut y0, mut x1, mut y1) = (
                        sr.left() as f64,
                        sr.top() as f64,
                        sr.right() as f64,
                        sr.bottom() as f64,
                    );
                    match c {
                        Corner::TopLeft => {
                            x0 = ptr_page.0;
                            y0 = ptr_y_down;
                        }
                        Corner::TopRight => {
                            x1 = ptr_page.0;
                            y0 = ptr_y_down;
                        }
                        Corner::BottomLeft => {
                            x0 = ptr_page.0;
                            y1 = ptr_y_down;
                        }
                        Corner::BottomRight => {
                            x1 = ptr_page.0;
                            y1 = ptr_y_down;
                        }
                    }
                    // Keep a sane minimum size, anchored at the still
                    // corner.
                    if x1 - x0 < 0.05 {
                        if matches!(c, Corner::TopLeft | Corner::BottomLeft) {
                            x0 = x1 - 0.05;
                        } else {
                            x1 = x0 + 0.05;
                        }
                    }
                    if y1 - y0 < 0.05 {
                        if matches!(c, Corner::TopLeft | Corner::TopRight) {
                            y0 = y1 - 0.05;
                        } else {
                            y1 = y0 + 0.05;
                        }
                    }
                    let target = Rect::from_min_max(
                        Pos2::new(x0 as f32, y0 as f32),
                        Pos2::new(x1 as f32, y1 as f32),
                    );
                    outlines.push(to_screen(target));
                    if let Some(q) = primary_quad {
                        handle_ghosts.push(crate::layers::ShapeQuad {
                            id: q.id,
                            tex: q.tex,
                            bbox: (x0, y0, x1 - x0, y1 - y0),
                        });
                    }
                }
            }
            HandleId::LineBegin | HandleId::LineEnd => {
                // Live line: the OTHER endpoint stays put, this one
                // follows the cursor (Visio stretches the segment) —
                // SNAPPED (#179): the live end magnetizes to
                // connection points and the other snap targets, the
                // same point the release will commit.
                let other = if matches!(hid, HandleId::LineBegin) {
                    HandleId::LineEnd
                } else {
                    HandleId::LineBegin
                };
                let snapped = snap_endpoint(true, ptr_page);
                let sn_screen = page_to_screen(snapped.0, snapped.1, view, page);
                if let Some((_, o)) = handles_screen.iter().find(|(id, _)| *id == other) {
                    // #226: when the styled endpoint ghost paints the
                    // whole styled ink, the bare chord stands down —
                    // it only serves as the raster-missing fallback.
                    if !drag.endpoint_ghost {
                        lines.push((*o, sn_screen));
                    }
                }
            }
            HandleId::GeoPoint { .. } => {
                // Live vertex feedback (#137): a guide line from the
                // handle's rest position to the pointer.
                if let Some((_, o)) = handles_screen
                    .iter()
                    .find(|(id, _)| matches!(id, HandleId::GeoPoint { .. } if *id == hid))
                {
                    lines.push((*o, ptr));
                }
            }
            HandleId::LineMid => {
                // Translate the shape by the gesture delta, like a move.
                let start = ui.ctx().memory(|m| m.data.get_temp::<Pos2>(mem_start));
                if let Some(st) = start {
                    let st_page = screen_to_page(st, view, page);
                    let (dx, dy) = (
                        (ptr_page.0 - st_page.0) as f32,
                        -(ptr_page.1 - st_page.1) as f32,
                    );
                    if let Some(q) = primary_quad {
                        handle_ghosts.push(crate::layers::ShapeQuad {
                            id: q.id,
                            tex: q.tex,
                            bbox: (
                                q.bbox.0 + dx as f64,
                                q.bbox.1 + dy as f64,
                                q.bbox.2,
                                q.bbox.3,
                            ),
                        });
                    }
                    if let Some(sr) = sel_rect {
                        outlines.push(to_screen(sr.translate(Vec2::new(dx, dy))));
                    }
                }
            }
        }
    }
    for q in drag.ghosts.iter().chain(handle_ghosts.iter()) {
        draw_ghost(q);
    }
    {
        let dashed = Stroke::new(1.2, t.selection);
        for r in &outlines {
            p.rect_stroke(*r, 0.0, dashed, egui::StrokeKind::Middle);
        }
        for (a, b) in &lines {
            p.line_segment([*a, *b], Stroke::new(1.4, t.selection));
        }
    }

    // Connector Tool rubber line (#108): press point → pointer, with a
    // hollow end marker where the connector's arrowhead will land.
    if let Some((from, to)) = connector_live {
        let a = page_to_screen(from.0, from.1, view, page);
        let b = page_to_screen(to.0, to.1, view, page);
        dashed(&p, a, b, HINT);
        p.circle_stroke(b, 4.0, Stroke::new(1.2, HINT));
    }

    // Connection-point affordances (#177): reveal symbols while a
    // connector endpoint gesture is live, the live glue target's
    // green square, and the selected connector's glued sites as green
    // circles. The live endpoint is the Connector Tool's pointer end
    // or a LineBegin/LineEnd handle-drag position.
    let live_end = connector_live.map(|(_, to)| to).or_else(|| {
        if matches!(
            armed_handle,
            Some(HandleId::LineBegin) | Some(HandleId::LineEnd)
        ) {
            ui.input(|i| i.pointer.interact_pos())
                .map(|p| snap_endpoint(true, screen_to_page(p, view, page)))
        } else {
            None
        }
    });
    let fx = conn_fx(live_end);
    for q in &fx.reveal {
        let c = page_to_screen(q.0, q.1, view, page);
        // The connection-point glyph: a small square with an × inside
        // (Visio's reveal symbol).
        let r = Rect::from_center_size(c, Vec2::splat(7.0));
        p.rect_stroke(r, 0.0, Stroke::new(1.0, CONN_PT), egui::StrokeKind::Middle);
        let d = 2.0;
        for (a, b) in [
            (c - Vec2::new(d, d), c + Vec2::new(d, d)),
            (c - Vec2::new(d, -d), c + Vec2::new(d, -d)),
        ] {
            p.line_segment([a, b], Stroke::new(1.0, CONN_PT));
        }
    }
    if let Some(q) = fx.glue_target {
        // The live glue target: Visio's filled green square.
        let c = page_to_screen(q.0, q.1, view, page);
        p.rect_filled(Rect::from_center_size(c, Vec2::splat(9.0)), 1.5, GLUE);
        p.rect_stroke(
            Rect::from_center_size(c, Vec2::splat(9.0)),
            1.5,
            Stroke::new(1.2, Color32::from_rgb(0x00, 0x7C, 0x38)),
            egui::StrokeKind::Middle,
        );
    }
    for q in &fx.glued {
        // A glued attachment site of the selected connector: green
        // circle (Visio's glue-site indicator).
        let c = page_to_screen(q.0, q.1, view, page);
        p.circle_stroke(c, 5.0, Stroke::new(2.0, GLUE));
    }
    if let Some(q) = fx.tool_selected {
        // The Connection Point tool's selection (#108): the point's
        // square ring in the selection color — Delete removes it.
        let c = page_to_screen(q.0, q.1, view, page);
        let r = Rect::from_center_size(c, Vec2::splat(10.0));
        p.rect_stroke(
            r,
            0.0,
            Stroke::new(2.0, t.selection),
            egui::StrokeKind::Middle,
        );
        let d = 2.5;
        for (a, b) in [
            (c - Vec2::new(d, d), c + Vec2::new(d, d)),
            (c - Vec2::new(d, -d), c + Vec2::new(d, -d)),
        ] {
            p.line_segment([a, b], Stroke::new(1.6, t.selection));
        }
    }

    // Dynamic-grid hints (#109): dashed full-canvas alignment lines
    // and labeled equal-spacing gaps, Visio's drag-time feedback.
    for hint in &drag.hints {
        match *hint {
            crate::snap::Hint::Align { vertical, at } => {
                let (a, b) = if vertical {
                    let x = page_to_screen(at, 0.0, view, page).x;
                    (
                        Pos2::new(x, canvas_r.top()),
                        Pos2::new(x, canvas_r.bottom()),
                    )
                } else {
                    let y = page_to_screen(0.0, at, view, page).y;
                    (
                        Pos2::new(canvas_r.left(), y),
                        Pos2::new(canvas_r.right(), y),
                    )
                };
                dashed(&p, a, b, HINT);
            }
            crate::snap::Hint::Spacing { a, b, len_in } => {
                let a0 = page_to_screen(a.0, a.1, view, page);
                let a1 = page_to_screen(a.2, a.3, view, page);
                let b0 = page_to_screen(b.0, b.1, view, page);
                let b1 = page_to_screen(b.2, b.3, view, page);
                dashed(&p, a0, a1, HINT);
                dashed(&p, b0, b1, HINT);
                let mid = Pos2::new((a0.x + a1.x) / 2.0, (a0.y + a1.y) / 2.0);
                p.text(
                    mid + Vec2::new(3.0, -3.0),
                    egui::Align2::LEFT_BOTTOM,
                    view.unit.fmt(len_in),
                    egui::FontId::proportional(10.0),
                    HINT,
                );
            }
        }
    }

    // Marquee (live rubber band while selecting on empty page).
    if let Some(mr) = out.marquee_live {
        p.rect_stroke(
            mr,
            0.0,
            Stroke::new(1.0, t.selection),
            egui::StrokeKind::Middle,
        );
        p.rect_filled(mr, 0.0, Color32::from_rgba_premultiplied(30, 120, 200, 24));
    }

    // Lasso trail (live freehand line, #110) — dashed to read as a
    // selection aid, never page ink.
    if let Some(trail) = &out.lasso_live {
        if trail.len() >= 2 {
            p.add(egui::Shape::dashed_line(
                trail,
                Stroke::new(1.0, t.selection),
                6.0,
                4.0,
            ));
        }
    }

    // Freeform pen trail (#132) — SOLID: it previews the shape's
    // future ink, not a selection aid.
    if let Some(trail) = &out.freeform_live {
        if trail.len() >= 2 {
            p.add(egui::Shape::line(
                trail.clone(),
                Stroke::new(1.5, t.selection),
            ));
        }
    }

    // Container member highlight (#156): Visio tints the selected
    // container's members — a translucent accent fill + thin stroke
    // above the ink, never exported.
    for r in member_hl {
        let sr = to_screen(*r);
        p.rect_filled(sr, 1.0, Color32::from_rgba_premultiplied(46, 117, 182, 36));
        p.rect_stroke(
            sr,
            1.0,
            Stroke::new(1.0, Color32::from_rgba_premultiplied(46, 117, 182, 130)),
            egui::StrokeKind::Middle,
        );
    }

    // AutoConnect arrows (#108): Visio's blue tags at the selection's
    // mid-edges while hovering; the hovered arrow fills.
    if let Some(arrows) = &autoc_arrows {
        if hover_in_sel && !interaction_lock {
            let hovered = resp.hover_pos();
            for (r, dir) in arrows {
                let h = hovered.is_some_and(|h| r.contains(h));
                let c = r.center();
                // The triangle points AWAY from the shape, along dir
                // (screen: y-down, so page-up flips).
                let (ax, ay) = (dir.0 as f32, -dir.1 as f32);
                let tip = c + Vec2::new(ax, ay) * 7.0;
                let b1 = c + Vec2::new(-ay, ax) * 5.0;
                let b2 = c - Vec2::new(-ay, ax) * 5.0;
                let (fill, stroke) = if h {
                    (
                        Color32::from_rgb(0x2e, 0x75, 0xb6),
                        Stroke::new(1.0, Color32::from_rgb(0x1f, 0x4e, 0x79)),
                    )
                } else {
                    (
                        Color32::from_rgba_premultiplied(50, 117, 182, 110),
                        Stroke::new(1.0, Color32::from_rgb(0x2e, 0x75, 0xb6)),
                    )
                };
                p.add(egui::Shape::convex_polygon(vec![tip, b1, b2], fill, stroke));
            }
        }
    }

    // Line jumps (#108): each hop breaks the JUMPING line with a
    // paper-colored gap, redraws the under line through it, and arcs
    // the jumper over the crossing (Visio's default arc style).
    for j in jumps {
        let ppi = view.px_per_in();
        let r = crate::connectors::JUMP_RADIUS_IN as f32 * ppi;
        let at = page_to_screen(j.at.0, j.at.1, view, page);
        let dir = Vec2::new(j.dir.0 as f32, -(j.dir.1 as f32)); // y flip
        let under = Vec2::new(j.under.0 as f32, -(j.under.1 as f32));
        let w = (j.weight * ppi as f64) as f32;
        let uw = (j.under_weight * ppi as f64) as f32;
        // 1. the gap — paper-colored, slightly wider than the jumper.
        let g1 = at - dir * r;
        let g2 = at + dir * r;
        p.line_segment([g1, g2], Stroke::new(w + 2.0, Color32::WHITE));
        // 2. the under line passes straight through the gap.
        let ext = r + uw + 1.0;
        p.line_segment(
            [at - under * ext, at + under * ext],
            Stroke::new(uw, j.under_color),
        );
        // 3. the hop arc, bulging up on screen.
        let n = Vec2::new(-dir.y, dir.x);
        let n = if n.y > 0.0 { -n } else { n };
        let mut pts = Vec::with_capacity(9);
        for k in 0..=8 {
            let t = k as f32 / 8.0;
            let ang = std::f32::consts::PI * t;
            // local frame: from g1 around to g2 through at + n*r.
            let local = -dir * r * ang.cos() + n * r * ang.sin();
            pts.push(at + local);
        }
        p.add(egui::Shape::line(pts, Stroke::new(w, j.color)));
    }

    // Rulers.
    if view.show_rulers {
        draw_rulers(&p, canvas_r, view, page, out.page_rect, &t);
    }

    // Mouse position in page coords (y-up) for the status bar.
    if let Some(mp) = resp.hover_pos() {
        let x_in = (mp.x - view.pan.x) as f64 / view.px_per_in() as f64;
        let y_up = page.h_in - (mp.y - view.pan.y) as f64 / view.px_per_in() as f64;
        if x_in >= 0.0 && y_up >= 0.0 && x_in <= page.w_in && y_up <= page.h_in {
            *mouse_page = Some((x_in, y_up));
        } else {
            *mouse_page = None;
        }
    }

    // While the in-place text editor holds the canvas (#22), a plain
    // click anywhere on the page ends the edit (Visio: click outside
    // the text) — and NOTHING else reports: no double-click, no
    // right-click, no stencil drop, so the ending click cannot also
    // select, open a menu, or re-enter the editor.
    // Clicks / drops: report page position; the app hit-tests.
    if resp.clicked() {
        out.shift_click = ui.input(|i| i.modifiers.shift);
        out.ctrl_click = ui.input(|i| i.modifiers.ctrl || i.modifiers.command);
        if let Some(mp) = resp.interact_pointer_pos() {
            let x_in = (mp.x - view.pan.x) as f64 / view.px_per_in() as f64;
            let y_up = page.h_in - (mp.y - view.pan.y) as f64 / view.px_per_in() as f64;
            out.click_page_pos = Some((x_in, y_up));
        }
    }
    if resp.secondary_clicked() && !interaction_lock {
        if let Some(mp) = resp.interact_pointer_pos() {
            let x_in = (mp.x - view.pan.x) as f64 / view.px_per_in() as f64;
            let y_up = page.h_in - (mp.y - view.pan.y) as f64 / view.px_per_in() as f64;
            out.right_click_page_pos = Some((x_in, y_up));
        }
    }
    if resp.double_clicked() && !interaction_lock {
        if let Some(mp) = resp.interact_pointer_pos() {
            out.dbl_click_page_pos = Some(screen_to_page(mp, view, page));
        }
    }
    if stencil_hover && !interaction_lock {
        ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::Crosshair);
        if resp.clicked() {
            if let Some(mp) = resp.interact_pointer_pos() {
                let x_in = (mp.x - view.pan.x) as f64 / view.px_per_in() as f64;
                let y_up = page.h_in - (mp.y - view.pan.y) as f64 / view.px_per_in() as f64;
                out.drop_pos = Some((x_in, y_up));
            }
        }
    }
    out
}

fn draw_rulers(
    p: &egui::Painter,
    canvas_r: Rect,
    view: &View,
    page: &PageGeom,
    page_rect: Rect,
    t: &ShellTokens,
) {
    // Corner square + bands.
    p.rect_filled(
        Rect::from_min_size(canvas_r.left_top(), Vec2::splat(RULER)),
        0.0,
        t.ruler_bg,
    );
    let top = Rect::from_min_size(
        canvas_r.left_top() + Vec2::new(RULER, 0.0),
        Vec2::new(canvas_r.width() - RULER, RULER),
    );
    let left = Rect::from_min_size(
        canvas_r.left_top() + Vec2::new(0.0, RULER),
        Vec2::new(RULER, canvas_r.height() - RULER),
    );
    p.rect_filled(top, 0.0, t.ruler_bg);
    p.rect_filled(left, 0.0, t.ruler_bg);
    p.rect_stroke(
        top,
        0.0,
        Stroke::new(0.5, t.ribbon_group),
        egui::StrokeKind::Middle,
    );
    p.rect_stroke(
        left,
        0.0,
        Stroke::new(0.5, t.ribbon_group),
        egui::StrokeKind::Middle,
    );

    let font = egui::FontId::proportional(9.0);
    let ppi = view.px_per_in();
    let dupx = view.unit.from_in(1.0) as f32 / ppi; // display units per px

    // Tiered ticks (#62): labeled majors (the 1/2/5-decade step) plus
    // half and quarter subdivisions, each minor tier dropped when its
    // pixel spacing would smear. Ticks are placed on the finest kept
    // interval and classified by integer index, so no float-multiple
    // drift decides tiers. The interval computation lives in
    // `ruler_interval_in` so the snap engine (#109) targets exactly
    // the subdivisions the ruler shows.
    let step = tick_step(dupx as f64, 45.0);
    let px_per_major = step / dupx as f64;
    let tiers = tick_tiers(step, px_per_major, RULER_SUBDIV, RULER_MIN_TICK_PX);
    let finest = tiers.last().map_or(1, |t: &TickTier| t.div as i64);
    let interval = view.unit.from_in(ruler_interval_in(view));
    let tier_for = |k: i64| -> TickTier {
        *tiers
            .iter()
            .find(|t| k % (finest / t.div as i64).max(1) == 0)
            .unwrap_or(&tiers[0])
    };

    // Horizontal ruler: ticks over the page extent only.
    let x0_in = ((page_rect.left() - view.pan.x) / ppi).max(0.0) as f64;
    let x1_in = ((page_rect.right() - view.pan.x) / ppi).min(page.w_in as f32) as f64;
    let upx = view.unit.from_in(1.0);
    let k0 = (x0_in * upx / interval).ceil() as i64;
    let k1 = (x1_in * upx / interval).floor() as i64;
    for k in k0..=k1 {
        let v = k as f64 * interval;
        let sx = view.pan.x + (view.unit.to_in(v) as f32) * ppi;
        let tier = tier_for(k);
        p.line_segment(
            [
                Pos2::new(sx, top.bottom() - tier.len),
                Pos2::new(sx, top.bottom()),
            ],
            Stroke::new(0.8, t.ruler_tick),
        );
        if tier.labeled && v > 0.0 {
            p.text(
                Pos2::new(sx + 2.0, top.top() + 1.0),
                egui::Align2::LEFT_TOP,
                format_num(v),
                font.clone(),
                t.ruler_text,
            );
        }
    }

    // Vertical ruler: values count upward from the page bottom (Visio).
    let y0_in = ((page_rect.top() - view.pan.y) / ppi).max(0.0) as f64;
    let y1_in = ((page_rect.bottom() - view.pan.y) / ppi).min(page.h_in as f32) as f64;
    let k0 = (y0_in * upx / interval).ceil() as i64;
    let k1 = (y1_in * upx / interval).floor() as i64;
    for k in k0..=k1 {
        let v = k as f64 * interval;
        let sy_top = view.pan.y + (view.unit.to_in(v) as f32) * ppi;
        let val_up = page.h_in - view.unit.to_in(v); // label in y-up units
        let disp = view.unit.from_in(val_up);
        let tier = tier_for(k);
        p.line_segment(
            [
                Pos2::new(left.right() - tier.len, sy_top),
                Pos2::new(left.right(), sy_top),
            ],
            Stroke::new(0.8, t.ruler_tick),
        );
        if tier.labeled && disp > 0.0 {
            p.text(
                Pos2::new(left.left() + 1.0, sy_top + 2.0),
                egui::Align2::LEFT_TOP,
                format_num(disp),
                font.clone(),
                t.ruler_text,
            );
        }
    }
}

fn format_num(v: f64) -> String {
    if (v - v.round()).abs() < 1e-6 {
        format!("{}", v.round() as i64)
    } else {
        format!("{v:.1}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arc_sagitta_round_trips_through_the_apex() {
        let (cur, end) = ((0.0, 0.0), (2.0, 0.0));
        // A pointer 0.3 above the chord midpoint: positive A bulges
        // RIGHT of travel (travel is +x, so right is -y in y-up).
        let a = arc_sagitta_from_pointer(cur, end, (1.0, -0.3));
        assert!((a - 0.3).abs() < 1e-9, "bulge right of travel: {a}");
        // The apex handle sits mid-chord offset by A to the right.
        let apex = arc_apex(cur, end, a);
        assert!((apex.0 - 1.0).abs() < 1e-9 && (apex.1 + 0.3).abs() < 1e-9);
        // Below the chord inverts the sagitta.
        let a2 = arc_sagitta_from_pointer(cur, end, (1.0, 0.3));
        assert!((a2 + 0.3).abs() < 1e-9, "signed: {a2}");
        // Degenerate chord: safe.
        assert!(arc_sagitta_from_pointer(cur, cur, (0.0, 1.0)).abs() < 1e-6);
    }

    #[test]
    fn lasso_trail_selects_by_touch() {
        // Rect [1,3]×[1,2]; a trail passing THROUGH it (both samples
        // outside) selects — touch, not containment (#110).
        let trail = [(0.0, 1.5), (4.0, 1.5)];
        assert!(lasso_hits_rect(&trail, 1.0, 1.0, 2.0, 1.0, 1e-6));
        // A trail far away does not.
        assert!(!lasso_hits_rect(&trail, 1.0, 4.0, 2.0, 1.0, 1e-6));
        // A sample inside selects.
        assert!(lasso_hits_rect(
            &[(0.0, 5.0), (1.5, 1.5)],
            1.0,
            1.0,
            2.0,
            1.0,
            1e-6
        ));
    }

    #[test]
    fn lasso_loop_selects_the_enclosed() {
        // A loop around the rect without any sample inside or segment
        // crossing: the enclosed center (2, 1.5) selects.
        let loop_trail = [(0.0, 0.0), (5.0, 0.0), (5.0, 4.0), (0.0, 4.0), (0.0, 0.0)];
        assert!(lasso_hits_rect(&loop_trail, 1.0, 1.0, 2.0, 1.0, 1e-6));
        // The same loop does not select a shape outside it.
        assert!(!lasso_hits_rect(&loop_trail, 6.0, 1.0, 2.0, 1.0, 1e-6));
    }

    #[test]
    fn lasso_diagonal_cross_counts() {
        // The trail crosses only the rect's corner region diagonally.
        let trail = [(0.5, 0.5), (1.5, 1.5)];
        assert!(lasso_hits_rect(&trail, 1.0, 1.0, 2.0, 1.0, 1e-6));
    }

    #[test]
    fn lasso_closure_follows_the_caller_tolerance() {
        // A hand-drawn loop: the ends land near — not exactly on —
        // each other. Within the caller's (screen-derived) tolerance
        // the trail reads as closed and ENCLOSES the rect, whose
        // center the winding counts across the closing edge.
        let near_closed = [
            (0.0, 0.0),
            (5.0, 0.0),
            (5.0, 4.0),
            (0.0, 4.0),
            (0.02, -0.015), // ≈ 0.025 in from the start
        ];
        assert!(
            lasso_hits_rect(&near_closed, 1.0, 1.0, 2.0, 1.0, 0.05),
            "a near-closed loop encloses within tolerance"
        );
        // The same trail at a tight tolerance is OPEN: no enclosure —
        // only the touch/inside paths could still select, and this
        // trail neither touches nor enters the rect.
        assert!(
            !lasso_hits_rect(&near_closed, 1.0, 1.0, 2.0, 1.0, 1e-6),
            "an open trail does not enclose"
        );
        // The closing edge is decisive here: the loop's ends straddle
        // the rect center's y-level (1.5) and only the last-to-first
        // edge crosses it right of the center — without it the winding
        // sums to zero and the loop would not enclose.
        let straddle = [
            (6.0, 1.49),
            (6.0, -1.0),
            (-1.0, -1.0),
            (-1.0, 4.0),
            (6.0, 4.0),
            (6.02, 1.51),
        ];
        assert!(
            lasso_hits_rect(&straddle, 1.0, 1.0, 2.0, 1.0, 0.05),
            "the closing edge completes the winding"
        );
    }

    #[test]
    fn screen_to_page_flips_y() {
        let view = View {
            zoom: 1.0,
            pan: Vec2::new(100.0, 50.0),
            ..Default::default()
        };
        let page = PageGeom {
            w_in: 8.5,
            h_in: 11.0,
        };
        // 1 in from the page's left, 1 in from its top → y-up 10 in.
        let (x, y) = screen_to_page(Pos2::new(100.0 + 96.0, 50.0 + 96.0), &view, &page);
        assert!((x - 1.0).abs() < 1e-9);
        assert!((y - 10.0).abs() < 1e-9);
    }

    #[test]
    fn apply_fit_fits_and_centers_the_page() {
        let mut view = View::default();
        let page = PageGeom {
            w_in: 8.5,
            h_in: 11.0,
        };
        view.apply_fit(Vec2::new(800.0, 600.0), Vec2::new(22.0, 22.0), &page);
        assert!(view.fit_mode, "a fit enters fit mode");
        // Height-constrained: 600 px / (11 in × 96 px/in), floored
        // onto the raster bucket grid (#67: the fit snaps down to
        // 1/384 zoom steps so scrollbar-hysteresis nudges never cross
        // a raster bucket).
        let raw: f32 = 600.0 / (11.0 * 96.0);
        let expected = (raw * 384.0).floor() / 384.0;
        assert!((view.zoom - expected).abs() < 1e-6, "zoom {}", view.zoom);
        assert!(view.zoom <= raw, "the snap never overflows the content");
        // The 8.5 in page is centered horizontally: origin + margin at
        // least 12 px, page height fills the content area.
        let pw = 8.5 * 96.0 * view.zoom;
        assert!(
            (view.pan.x - (22.0 + (800.0 - pw) / 2.0)).abs() < 0.5,
            "pan.x {}",
            view.pan.x
        );
        let ph = 11.0 * 96.0 * view.zoom;
        assert!(
            (view.pan.y - (22.0 + ((600.0 - ph).max(0.0) / 2.0).max(12.0))).abs() < 0.5,
            "pan.y {}",
            view.pan.y
        );
    }

    /// #237: zoom-to-selection fills the content with the rect and
    /// centers it, leaves fit mode, and zooms OUT for a selection
    /// larger than the view (general clamps, unlike the #22 text-edit
    /// zoom that never zooms out).
    #[test]
    fn zoom_to_selection_fills_and_centers() {
        let page = PageGeom {
            w_in: 8.5,
            h_in: 11.0,
        };
        let mut view = View::default();
        view.apply_fit(Vec2::new(800.0, 600.0), Vec2::new(22.0, 22.0), &page);
        // A 1×1 in selection: the height axis governs — 600 px / 96 =
        // 6.25× fills exactly (the width axis would want 8.33×).
        view.zoom_to_selection((1.0, 1.0, 1.0, 1.0), Vec2::new(800.0, 600.0), Vec2::ZERO, &page);
        assert!((view.zoom - 6.25).abs() < 1e-4, "fills at 6.25×: {}", view.zoom);
        assert!(!view.fit_mode, "leaves fit mode");
        assert!(!view.fit_pending, "no pending fit");
        // The selection's page center sits at the content center.
        let ppi = view.px_per_in();
        let cx = view.pan.x + (1.5 * ppi);
        let cy = view.pan.y + (11.0 - 1.5) * ppi;
        assert!((cx - 400.0).abs() < 0.5, "center x on-screen: {cx}");
        assert!((cy - 300.0).abs() < 0.5, "center y on-screen: {cy}");

        // A 2×1 in rect in a 400×400 area: zx = 400/192 = 2.083, zy =
        // 400/96 = 4.17 → fill zoom = 2.083 (the min axis fills).
        let mut view = View::default();
        view.apply_fit(Vec2::new(800.0, 600.0), Vec2::ZERO, &page);
        view.zoom_to_selection((0.0, 0.0, 2.0, 1.0), Vec2::new(400.0, 400.0), Vec2::ZERO, &page);
        let want = ((400.0f32 / (2.0 * 96.0)) * 384.0).floor() / 384.0;
        assert!((view.zoom - want).abs() < 1e-4, "fill zoom: {}", view.zoom);
        // The rect's center lands at the content center.
        let ppi = view.px_per_in();
        assert!((view.pan.x + 1.0 * ppi - 200.0).abs() < 0.5);
        assert!((view.pan.y + (11.0 - 0.5) * ppi - 200.0).abs() < 0.5);

        // A selection larger than the view zooms OUT to center it —
        // the general clamp, against the text-edit zoom's ≥100 % rule.
        let mut view = View {
            zoom: 2.0,
            fit_mode: false,
            ..View::default()
        };
        view.zoom_to_selection((0.0, 0.0, 16.0, 20.0), Vec2::new(400.0, 400.0), Vec2::ZERO, &page);
        assert!(
            (view.zoom - (400.0 / (20.0 * 96.0))).abs() < 0.01,
            "zoomed out to fit: {}",
            view.zoom
        );
        // Degenerate rects floor at 0.25 in instead of absurd zooms.
        let mut view = View::default();
        view.zoom_to_selection((0.0, 0.0, 0.0, 0.0), Vec2::new(400.0, 400.0), Vec2::ZERO, &page);
        assert!((view.zoom - 8.0).abs() < 1e-4, "hairline caps at 8×: {}", view.zoom);
    }

    /// #237: the wheel zoom anchors on the selection's center — the
    /// point stays stationary across the zoom (a pure view-state
    /// assertion over the same pan math `show` runs).
    #[test]
    fn wheel_anchor_math_holds_the_selection_center() {
        let page = PageGeom {
            w_in: 8.5,
            h_in: 11.0,
        };
        let mut view = View::default();
        view.apply_fit(Vec2::new(800.0, 600.0), Vec2::new(22.0, 22.0), &page);
        // The selection's screen-space center (page y-down inches →
        // screen, the mapping `show`'s wheel handler uses).
        let sel = Rect::from_min_size(Pos2::new(4.0, 5.0), Vec2::new(1.0, 1.0));
        let center = |v: &View| {
            Pos2::new(
                v.pan.x + sel.center().x * v.px_per_in(),
                v.pan.y + sel.center().y * v.px_per_in(),
            )
        };
        let before = center(&view);
        // The handler's math: k = new/old, pan = anchor − (anchor −
        // pan)·k.
        let factor = 1.1;
        let old = view.zoom;
        view.zoom = (view.zoom * factor).clamp(0.1, 8.0);
        let k = view.zoom / old;
        view.pan = before.to_vec2() - (before.to_vec2() - view.pan) * k;
        let after = center(&view);
        assert!(
            before.distance(after) < 0.01,
            "the selection center is stationary: {before} vs {after}"
        );
    }

    #[test]
    fn refit_follows_content_changes_until_manual_intervention() {
        let mut view = View::default();
        let page = PageGeom {
            w_in: 8.5,
            h_in: 11.0,
        };
        view.apply_fit(Vec2::new(800.0, 600.0), Vec2::ZERO, &page);
        let z0 = view.zoom;
        // A resize re-fits (taller content → larger fit zoom).
        assert!(
            view.wants_refit(Vec2::new(800.0, 900.0)),
            "resize triggers refit"
        );
        view.apply_fit(Vec2::new(800.0, 900.0), Vec2::ZERO, &page);
        assert!(view.zoom > z0, "zoom grew: {z0} → {}", view.zoom);
        // Same size — and sub-pixel jitter — must not re-fit every frame.
        assert!(!view.wants_refit(Vec2::new(800.0, 900.0)));
        assert!(
            !view.wants_refit(Vec2::new(800.2, 900.3)),
            "jitter is ignored"
        );
        // Manual zoom/pan clears fit mode: resizes stop re-fitting.
        view.fit_mode = false;
        assert!(
            !view.wants_refit(Vec2::new(300.0, 200.0)),
            "manual view survives a resize"
        );
        // Fit Page re-arms: the pending fit re-enters fit mode.
        view.fit_pending = true;
        assert!(
            !view.wants_refit(Vec2::new(300.0, 200.0)),
            "pending alone doesn't refit"
        );
        view.apply_fit(Vec2::new(300.0, 200.0), Vec2::ZERO, &page);
        assert!(view.fit_mode, "apply_fit re-enters fit mode");
        assert!(
            view.wants_refit(Vec2::new(800.0, 900.0)),
            "and follows again"
        );
    }
}
