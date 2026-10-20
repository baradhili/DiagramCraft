//! The page canvas: the rendered page as a texture (via the SVG
//! backend), Visio-style rulers with metric ticks, grid overlay,
//! selection frame with handles, drag-to-move, wheel zoom and pan.

use egui::{Color32, Pos2, Rect, Sense, Stroke, Ui, Vec2};

use crate::theme;
use crate::units::{tick_step, Unit};

/// View state for the active page.
#[derive(Debug, Clone)]
pub struct View {
    pub zoom: f32,        // 1.0 = 96 px/in
    pub pan: Vec2,        // screen offset of the page's top-left corner
    pub show_grid: bool,
    pub show_rulers: bool,
    pub unit: Unit,
    pub fit_pending: bool,
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
        }
    }
}

impl View {
    pub fn px_per_in(&self) -> f32 {
        96.0 * self.zoom
    }
}

/// Page geometry in inches (y-down on screen after the flip).
pub struct PageGeom {
    pub w_in: f64,
    pub h_in: f64,
}

pub const RULER: f32 = 22.0;

/// Outcome of interacting with the canvas this frame.
pub struct CanvasOut {
    pub page_rect: Rect,
    /// Full allocated canvas area (drop target checks).
    pub canvas_rect: Rect,
    /// Click landed on empty page area.
    pub clicked_empty: bool,
    /// Selected shape id from a click (hit test done by the app).
    pub click_page_pos: Option<(f64, f64)>,
    /// Right-click position in page inches (y-up); the app hit-tests and
    /// opens the shape context menu.
    pub right_click_page_pos: Option<(f64, f64)>,
    /// A drag of `shape` finished at this pin position (inches, y-up).
    pub drag_done: Option<(i64, (f64, f64))>,
    /// A stencil drop happened at this pin position (inches, y-up).
    pub drop_pos: Option<(f64, f64)>,
    /// A selection-handle drag finished: (corner, page-inches delta from
    /// drag start, y-up). The app applies the resize.
    pub resize_done: Option<(Corner, (f64, f64))>,
    /// A handle drag started this frame on this corner.
    pub resize_started: Option<Corner>,
    /// Marquee (empty-area drag) finished: rect in page inches, y-down,
    /// (x0, y0, x1, y1) possibly un-normalized.
    pub marquee_done: Option<(f64, f64, f64, f64)>,
    /// Marquee rect this frame while dragging (page inches, y-down).
    pub marquee_live: Option<Rect>,
    /// Shift held at click time (multi-select toggle).
    pub shift_click: bool,
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

/// Screen position → page inches (y-up), regardless of the pointer's
/// position relative to the page.
pub fn screen_to_page(p: Pos2, view: &View, page: &PageGeom) -> (f64, f64) {
    let x = (p.x - view.pan.x) as f64 / view.px_per_in() as f64;
    let y = page.h_in - (p.y - view.pan.y) as f64 / view.px_per_in() as f64;
    (x, y)
}

/// Draw the whole canvas area (fills `ui`).
#[allow(clippy::too_many_arguments)]
pub fn show(
    ui: &mut Ui,
    view: &mut View,
    page: &PageGeom,
    page_tex: Option<&egui::TextureHandle>,
    sel_rect: Option<Rect>,          // primary selection (page-inches, y-down)
    extra_sel_rects: &[Rect],        // other multi-selection members
    drag_rect: Option<Rect>,         // preview while dragging (screen already applied)
    drag_active: bool,
    stencil_hover: bool,
    hit_shape: &dyn Fn((f64, f64)) -> bool, // page inches y-up, for marquee gating
    mouse_page: &mut Option<(f64, f64)>, // y-up inches, for the status bar
) -> CanvasOut {
    let mut out = CanvasOut {
        page_rect: Rect::NOTHING,
        canvas_rect: Rect::NOTHING,
        clicked_empty: false,
        click_page_pos: None,
        right_click_page_pos: None,
        drag_done: None,
        drop_pos: None,
        resize_done: None,
        resize_started: None,
        marquee_done: None,
        marquee_live: None,
        shift_click: false,
        zoom_changed: false,
    };
    let avail = ui.available_size();
    let (canvas_r, resp) = ui.allocate_exact_size(avail, Sense::click_and_drag());
    out.canvas_rect = canvas_r;

    // Content area origin (below/right of rulers).
    let content_origin = if view.show_rulers {
        canvas_r.left_top() + Vec2::new(RULER, RULER)
    } else {
        canvas_r.left_top()
    };

    // Wheel zoom around the hovered point.
    if let Some(hp) = resp.hover_pos() {
        let scroll = ui.input(|i| i.smooth_scroll_delta.y);
        if scroll != 0.0 {
            let factor = if scroll > 0.0 { 1.1 } else { 1.0 / 1.1 };
            let old = view.zoom;
            view.zoom = (view.zoom * factor).clamp(0.1, 8.0);
            let k = view.zoom / old;
            // keep the point under the cursor stationary
            view.pan = hp.to_vec2() - (hp.to_vec2() - view.pan) * k;
            out.zoom_changed = true;
        }
    }

    // Fit on first layout.
    if view.fit_pending {
        let cw = (canvas_r.right() - content_origin.x - 12.0).max(50.0);
        let ch = (canvas_r.bottom() - content_origin.y - 12.0).max(50.0);
        let zx = cw / (page.w_in as f32 * 96.0);
        let zy = ch / (page.h_in as f32 * 96.0);
        view.zoom = zx.min(zy).clamp(0.1, 8.0);
        let pw = page.w_in as f32 * view.px_per_in();
        let ph = page.h_in as f32 * view.px_per_in();
        view.pan = content_origin.to_vec2()
            + Vec2::new((cw - pw) / 2.0, (ch - ph) / 2.0).max(Vec2::new(12.0, 12.0));
        view.fit_pending = false;
        out.zoom_changed = true;
    }

    // Selection rect in screen coords for handle hit-testing.
    let sel_rect_screen = sel_rect.map(|r| {
        let tl = Pos2::new(
            view.pan.x + r.left() as f32 * view.px_per_in(),
            view.pan.y + r.top() as f32 * view.px_per_in(),
        );
        Rect::from_min_size(
            tl,
            Vec2::new(r.width() as f32 * view.px_per_in(), r.height() as f32 * view.px_per_in()),
        )
    });

    // Middle-drag or space-drag pans.
    let pan_drag = resp.dragged_by(egui::PointerButton::Middle)
        || (ui.input(|i| i.modifiers.command) && resp.dragged());
    if pan_drag {
        view.pan += resp.drag_delta();
    }

    // Selection-handle resize + marquee (empty-area) drag. Corner-handle
    // drags take priority; a primary drag starting on a shape is the
    // app's move; starting on empty page is a marquee.
    let corner_at = |pos: Pos2, sr: Rect| -> Option<Corner> {
        let grab = 8.0;
        [
            (Corner::TopLeft, sr.left_top()),
            (Corner::TopRight, sr.right_top()),
            (Corner::BottomLeft, sr.left_bottom()),
            (Corner::BottomRight, sr.right_bottom()),
        ]
        .into_iter()
        .find(|(_, c)| pos.distance(*c) <= grab)
        .map(|(k, _)| k)
    };
    // Stable literal ids are intended for the drag-state memory keys.
    #[allow(deprecated)]
    let mem_resize = egui::Id::new("dc-resize-corner");
    #[allow(deprecated)]
    let mem_start = egui::Id::new("dc-drag-start");
    #[allow(deprecated)]
    let mem_marquee = egui::Id::new("dc-marquee");
    let press_origin = ui.input(|i| i.pointer.press_origin());
    if resp.drag_started_by(egui::PointerButton::Primary) {
        if let Some(start) = press_origin {
            ui.ctx().memory_mut(|m| m.data.insert_temp(mem_start, start));
        }
        if let (Some(sr), Some(start)) = (sel_rect_screen, press_origin) {
            if let Some(corner) = corner_at(start, sr) {
                ui.ctx().memory_mut(|m| m.data.insert_temp(mem_resize, corner));
                out.resize_started = Some(corner);
            }
        }
    }
    let resizing = ui.ctx().memory(|m| m.data.get_temp::<Corner>(mem_resize));
    let primary_released = ui.input(|i| i.pointer.button_released(egui::PointerButton::Primary));
    #[cfg(debug_assertions)]
    if resp.drag_stopped() || primary_released {
        eprintln!("release frame: resizing={resizing:?} drag_stopped={} primary_released={}", resp.drag_stopped(), primary_released);
    }
    if let (Some(corner), true) = (resizing, resp.drag_stopped() && primary_released) {
        ui.ctx().memory_mut(|m| m.data.remove::<Corner>(mem_resize));
        // press_origin is gone on the release frame; the start pos was
        // stored when the drag began.
        let start = ui.ctx().memory(|m| m.data.get_temp::<Pos2>(mem_start));
        if let (Some(start), Some(end)) = (start, ui.input(|i| i.pointer.interact_pos())) {
            let (sx, sy) = screen_to_page(start, view, page);
            let (ex, ey) = screen_to_page(end, view, page);
            // y-down delta in page inches (screen orientation).
            out.resize_done = Some((corner, (ex - sx, sy - ey)));
        }
    }
    // Marquee: primary drag that started on empty page (no shape, no
    // handle, no stencil placement).
    if resp.drag_started_by(egui::PointerButton::Primary)
        && resizing.is_none()
        && press_origin.map(|s| !hit_shape(screen_to_page(s, view, page))).unwrap_or(false)
    {
        ui.ctx().memory_mut(|m| m.data.insert_temp(mem_marquee, true));
    }
    let marqueeing = ui.ctx().memory(|m| m.data.get_temp::<bool>(mem_marquee)).unwrap_or(false);
    if marqueeing {
        if (resp.drag_stopped() && primary_released)
            || !ui.input(|i| i.pointer.primary_down())
        {
            ui.ctx().memory_mut(|m| m.data.remove::<bool>(mem_marquee));
            let start = ui.ctx().memory(|m| m.data.get_temp::<Pos2>(mem_start));
            if let (Some(start), Some(end)) = (start, ui.input(|i| i.pointer.interact_pos())) {
                let (sx, sy) = screen_to_page(start, view, page);
                let (ex, ey) = screen_to_page(end, view, page);
                let (y0, y1) = if sy < ey { (sy, ey) } else { (ey, sy) };
                out.marquee_done = Some((sx.min(ex), y0, sx.max(ex), y1));
            }
        } else if let Some(start) = ui.ctx().memory(|m| m.data.get_temp::<Pos2>(mem_start)) {
            if let Some(cur) = ui.input(|i| i.pointer.interact_pos()) {
                out.marquee_live = Some(Rect::from_two_pos(start, cur));
            }
        }
    }
    // Handle cursors.
    if let (Some(sr), Some(hp)) = (sel_rect_screen, resp.hover_pos()) {
        if let Some(c) = corner_at(hp, sr) {
            ui.output_mut(|o| o.cursor_icon = match c {
                Corner::TopLeft | Corner::BottomRight => egui::CursorIcon::ResizeNwSe,
                Corner::TopRight | Corner::BottomLeft => egui::CursorIcon::ResizeNeSw,
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
    let p = ui.painter().with_clip_rect(canvas_r);
    p.rect_filled(canvas_r, 0.0, theme::DESK);
    // shadow
    p.rect_filled(
        Rect::from_min_size(page_tl + Vec2::splat(3.0), page_px),
        0.0,
        Color32::from_black_alpha(28),
    );
    p.rect_filled(out.page_rect, 0.0, Color32::WHITE);
    p.rect_stroke(out.page_rect, 0.0, Stroke::new(1.0, theme::PAGE_EDGE), egui::StrokeKind::Middle);
    if let Some(tex) = page_tex {
        let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
        p.image(tex.id(), out.page_rect, uv, Color32::WHITE);
    }

    // Grid (5 mm spacing in display units).
    if view.show_grid {
        let step_in = view.unit.to_in(5.0);
        let step_px = step_in as f32 * view.px_per_in();
        if step_px >= 5.0 {
            let mut x = out.page_rect.left() + step_px;
            while x < out.page_rect.right() {
                p.line_segment(
                    [Pos2::new(x, out.page_rect.top()), Pos2::new(x, out.page_rect.bottom())],
                    Stroke::new(0.5, Color32::from_gray(216)),
                );
                x += step_px;
            }
            let mut y = out.page_rect.top() + step_px;
            while y < out.page_rect.bottom() {
                p.line_segment(
                    [Pos2::new(out.page_rect.left(), y), Pos2::new(out.page_rect.right(), y)],
                    Stroke::new(0.5, Color32::from_gray(216)),
                );
                y += step_px;
            }
        }
    }

    // Selection frame (page-inch rect → screen).
    let to_screen = |r: Rect| {
        let tl = Pos2::new(
            view.pan.x + r.left() as f32 * view.px_per_in(),
            view.pan.y + r.top() as f32 * view.px_per_in(),
        );
        Rect::from_min_size(
            tl,
            Vec2::new(r.width() as f32 * view.px_per_in(), r.height() as f32 * view.px_per_in()),
        )
    };
    for er in extra_sel_rects.iter().map(|r| to_screen(*r)) {
        p.rect_stroke(er, 0.0, Stroke::new(1.0, theme::SELECTION), egui::StrokeKind::Middle);
    }
    if let Some(sr) = sel_rect.map(to_screen) {
        p.rect_stroke(sr, 0.0, Stroke::new(1.2, theme::SELECTION), egui::StrokeKind::Middle);
        for corner in [sr.left_top(), sr.right_top(), sr.left_bottom(), sr.right_bottom()] {
            p.rect_filled(
                Rect::from_center_size(corner, Vec2::splat(7.0)),
                1.0,
                Color32::WHITE,
            );
            p.rect_stroke(
                Rect::from_center_size(corner, Vec2::splat(7.0)),
                1.0,
                Stroke::new(1.2, theme::SELECTION),
                egui::StrokeKind::Middle,
            );
        }
    }

    // Drag preview.
    if let Some(dr) = drag_rect {
        let dashed = Stroke::new(1.2, theme::SELECTION);
        p.rect_stroke(dr, 0.0, dashed, egui::StrokeKind::Middle);
    }

    // Marquee (live rubber band while selecting on empty page).
    if let Some(mr) = out.marquee_live {
        p.rect_stroke(mr, 0.0, Stroke::new(1.0, theme::SELECTION), egui::StrokeKind::Middle);
        p.rect_filled(mr, 0.0, Color32::from_rgba_premultiplied(30, 120, 200, 24));
    }

    // Rulers.
    if view.show_rulers {
        draw_rulers(&p, canvas_r, view, page, out.page_rect);
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

    // Clicks / drops: report page position; the app hit-tests.
    if resp.clicked() {
        if let Some(mp) = resp.interact_pointer_pos() {
            let x_in = (mp.x - view.pan.x) as f64 / view.px_per_in() as f64;
            let y_up = page.h_in - (mp.y - view.pan.y) as f64 / view.px_per_in() as f64;
            out.click_page_pos = Some((x_in, y_up));
        }
    }
    if resp.secondary_clicked() {
        if let Some(mp) = resp.interact_pointer_pos() {
            let x_in = (mp.x - view.pan.x) as f64 / view.px_per_in() as f64;
            let y_up = page.h_in - (mp.y - view.pan.y) as f64 / view.px_per_in() as f64;
            out.right_click_page_pos = Some((x_in, y_up));
        }
    }
    if stencil_hover {
        ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::Crosshair);
        if resp.clicked() {
            if let Some(mp) = resp.interact_pointer_pos() {
                let x_in = (mp.x - view.pan.x) as f64 / view.px_per_in() as f64;
                let y_up = page.h_in - (mp.y - view.pan.y) as f64 / view.px_per_in() as f64;
                out.drop_pos = Some((x_in, y_up));
            }
        }
    }
    let _ = drag_active;
    out
}

fn draw_rulers(
    p: &egui::Painter,
    canvas_r: Rect,
    view: &View,
    page: &PageGeom,
    page_rect: Rect,
) {
    // Corner square + bands.
    p.rect_filled(
        Rect::from_min_size(canvas_r.left_top(), Vec2::splat(RULER)),
        0.0,
        theme::RULER_BG,
    );
    let top = Rect::from_min_size(
        canvas_r.left_top() + Vec2::new(RULER, 0.0),
        Vec2::new(canvas_r.width() - RULER, RULER),
    );
    let left = Rect::from_min_size(
        canvas_r.left_top() + Vec2::new(0.0, RULER),
        Vec2::new(RULER, canvas_r.height() - RULER),
    );
    p.rect_filled(top, 0.0, theme::RULER_BG);
    p.rect_filled(left, 0.0, theme::RULER_BG);
    p.rect_stroke(top, 0.0, Stroke::new(0.5, theme::RIBBON_GROUP), egui::StrokeKind::Middle);
    p.rect_stroke(left, 0.0, Stroke::new(0.5, theme::RIBBON_GROUP), egui::StrokeKind::Middle);

    let font = egui::FontId::proportional(9.0);
    let ppi = view.px_per_in();
    let dupx = view.unit.from_in(1.0) as f32 / ppi; // display units per px

    // Horizontal ruler: ticks over the page extent only.
    let step = tick_step(dupx as f64, 45.0);
    let x0_in = ((page_rect.left() - view.pan.x) / ppi).max(0.0) as f64;
    let x1_in = ((page_rect.right() - view.pan.x) / ppi).min(page.w_in as f32) as f64;
    let mut v = (x0_in * view.unit.from_in(1.0) / step as f64).floor() * step as f64;
    while v <= x1_in * view.unit.from_in(1.0) {
        let sx = view.pan.x + (view.unit.to_in(v) as f32) * ppi;
        let major = (v / step).fract().abs() < 1e-6;
        let len = if major { 7.0 } else { 4.0 };
        p.line_segment(
            [Pos2::new(sx, top.bottom() - len), Pos2::new(sx, top.bottom())],
            Stroke::new(0.8, theme::RULER_TICK),
        );
        if major && v > 0.0 {
            p.text(
                Pos2::new(sx + 2.0, top.top() + 1.0),
                egui::Align2::LEFT_TOP,
                format_num(v),
                font.clone(),
                theme::RULER_TEXT,
            );
        }
        v += step as f64;
    }

    // Vertical ruler: values count upward from the page bottom (Visio).
    let y0_in = ((page_rect.top() - view.pan.y) / ppi).max(0.0) as f64;
    let y1_in = ((page_rect.bottom() - view.pan.y) / ppi).min(page.h_in as f32) as f64;
    let mut v = (y0_in * view.unit.from_in(1.0) / step as f64).floor() * step as f64;
    while v <= y1_in * view.unit.from_in(1.0) {
        let sy_top = view.pan.y + (view.unit.to_in(v) as f32) * ppi;
        let val_up = page.h_in - view.unit.to_in(v); // label in y-up units
        let disp = view.unit.from_in(val_up);
        let major = (v / step).fract().abs() < 1e-6;
        let len = if major { 7.0 } else { 4.0 };
        p.line_segment(
            [Pos2::new(left.right() - len, sy_top), Pos2::new(left.right(), sy_top)],
            Stroke::new(0.8, theme::RULER_TICK),
        );
        if major && disp > 0.0 {
            p.text(
                Pos2::new(left.left() + 1.0, sy_top + 2.0),
                egui::Align2::LEFT_TOP,
                format_num(disp),
                font.clone(),
                theme::RULER_TEXT,
            );
        }
        v += step as f64;
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
    fn screen_to_page_flips_y() {
        let view = View {
            zoom: 1.0,
            pan: Vec2::new(100.0, 50.0),
            ..Default::default()
        };
        let page = PageGeom { w_in: 8.5, h_in: 11.0 };
        // 1 in from the page's left, 1 in from its top → y-up 10 in.
        let (x, y) = screen_to_page(Pos2::new(100.0 + 96.0, 50.0 + 96.0), &view, &page);
        assert!((x - 1.0).abs() < 1e-9);
        assert!((y - 10.0).abs() < 1e-9);
    }
}
