//! In-place WYSIWYG text editing on the canvas (GitHub #22).
//!
//! Visio never opens a modal text box: F2 / double-click / Edit Text…
//! zoom the view into the shape's text block and you type directly on
//! the page, in the shape's actual font, size, color, and alignment.
//! This module is that editor — an `egui::TextEdit` overlay placed on
//! the shape's text-block rect, styled from the SAME resolved cells
//! the SVG renderer paints with (`dc::svg::text_block`), so the
//! editing view is the rendered result.
//!
//! Life cycle (all three entry points — F2, the context menu's Edit
//! Text…, and double-click — funnel into [`TextEditSession`]):
//!
//! - **entry** (the app, each frame until it lands) captures the
//!   pre-edit view and zooms the block toward the middle of the
//!   content area (`View::zoom_to_rect`, once — a manual zoom during
//!   the edit is respected);
//! - **while open** the shape's layer re-renders with its text
//!   suppressed (`render_shape_layer_opts`) so the editor is the only
//!   copy of the text on screen — no ghost under the caret — and the
//!   canvas is interaction-locked (a click ends the edit; it cannot
//!   also select, marquee, or re-enter);
//! - **exit** (click outside, or Esc) commits through `SetText` — one
//!   undo step, dirty flag, texture invalidation, exactly the retired
//!   dialog's commit path — and restores the pre-edit view (Visio
//!   zooms back out).
//!
//! End-of-edit semantics (owner review, 2026-10-27): a click outside
//! commits the draft through `SetText` (one undo step reverts it);
//! **Esc discards** — the shape's text reverts to its pre-edit value.
//!
//! v1 scope: unrotated placement — a rotated shape (`Angle`/
//! `TxtAngle`) yields the block's axis-aligned box; italic renders as
//! regular (the overlay picks Regular/Bold faces only).

use crate::canvas::{self, PageGeom, View};
use dc::svg::{text_block, text_block_page_rect};
use egui::{Color32, FontId, Pos2, Rect, Ui, Vec2};

/// One open editing session. Created by the app's entry points;
/// `prev_view` restores the zoom/pan the user had before the edit.
pub struct TextEditSession {
    /// The page the edited shape lives on (the exit path targets it by
    /// id, so a page switch racing the ending click cannot land the
    /// SetText on the wrong page).
    pub page: i64,
    pub shape: i64,
    pub draft: String,
    /// View state (zoom, pan, fit flags) captured on entry — restored
    /// on exit.
    pub prev_view: View,
    /// The entry zoom has been applied (`View::zoom_to_rect` runs
    /// once; later zooms are the user's and stay).
    pub zoomed_in: bool,
    /// Initial focus requested on the editor widget (frame 0 only —
    /// the same frame may carry the click that opened the session,
    /// which must not count as click-away).
    pub focus_requested: bool,
}

impl TextEditSession {
    pub fn new(page: i64, shape: i64, draft: String, view: &View) -> Self {
        TextEditSession {
            page,
            shape,
            draft,
            prev_view: view.clone(),
            zoomed_in: false,
            focus_requested: false,
        }
    }

    /// Apply the entry zoom once: the text block fills ~half the
    /// content area, centered (`View::zoom_to_rect`). Skipped until a
    /// real canvas rect exists — the app calls this every frame until
    /// it lands, because a session can open before the first layout.
    pub fn enter_zoom(
        &mut self,
        view: &mut View,
        block: (f64, f64, f64, f64),
        canvas_rect: Rect,
        geom: &PageGeom,
    ) {
        if self.zoomed_in || !canvas_rect.is_positive() {
            return;
        }
        let (origin, content) = canvas::content_area(canvas_rect, view.show_rulers);
        view.zoom_to_rect(block, content, origin.to_vec2(), geom);
        self.zoomed_in = true;
    }

    /// Restore the pre-edit view (Visio zooms back out when text
    /// editing ends).
    pub fn restore_view(&self, view: &mut View) {
        *view = self.prev_view.clone();
    }
}

/// What the overlay decided this frame.
pub enum EditFlow {
    /// Keep editing.
    Active,
    /// End the session, committing the session's draft via `SetText`
    /// (click outside).
    Commit,
    /// End the session WITHOUT committing: Esc — the shape's text
    /// reverts to its pre-edit value (owner review, 2026-10-27).
    Abandon,
}

/// Show the in-place editor over the shape's text block.
///
/// Returns a terminal [`EditFlow`] on Esc or a click outside the
/// editor. Clicks on the canvas itself are caught by the app's canvas
/// click path (which ends the session before the overlay runs); this
/// catches clicks on any other shell surface (ribbon, stencil pane,
/// status bar). If the shape or its text block stops resolving
/// mid-edit (delete, undo), the session ends by committing the draft.
#[allow(clippy::too_many_arguments)]
pub fn show_overlay(
    ui: &mut Ui,
    session: &mut TextEditSession,
    doc: &vsdx::model::Document,
    page: &vsdx::Page,
    geom: &PageGeom,
    view: &View,
) -> EditFlow {
    let Some(block) = text_block_page_rect(doc, page, session.shape) else {
        return EditFlow::Commit;
    };
    let Some(tb) = text_block(doc, page, session.shape) else {
        return EditFlow::Commit;
    };

    // Page inches (y-up) → screen rect; the editor covers the block.
    let (bx, by, bw, bh) = block;
    let tl = canvas::page_to_screen(bx, by + bh, view, geom);
    let br = canvas::page_to_screen(bx + bw, by, view, geom);
    let screen = Rect::from_two_pos(tl, br);
    if !screen.is_positive() {
        return EditFlow::Active;
    }

    // Editor chrome: Visio hairlines the text block while editing.
    ui.painter().rect_stroke(
        screen,
        2.0,
        egui::Stroke::new(1.0, ui.visuals().selection.stroke.color),
        egui::StrokeKind::Middle,
    );

    // WYSIWYG (owner review, 2026-10-27): the overlay renders in the
    // shape's ACTUAL font — the same face the rasterizer draws (a
    // system font where installed, the vendored Carlito otherwise) —
    // and its first baseline sits exactly where the renderer puts it
    // (`dc::svg::text_first_baseline`), so entering and leaving the
    // editor never translates the text. Font size in egui points
    // (≈ logical px) = inches × 96 × zoom.
    let ppi = view.px_per_in();
    let font_pts = (tb.font_size as f32 * 96.0 * view.zoom).clamp(4.0, 400.0);
    let (family, ascent_em) = overlay_font(ui.ctx(), tb.font.as_deref(), tb.bold, tb.italic);
    let font_id = FontId::new(font_pts, family);
    let color = parse_hex_color(&tb.color);
    let line_pts = (tb.line_height as f32 * 96.0 * view.zoom).clamp(font_pts, 1000.0);
    // The renderer's text area: the block insets, symmetric.
    let (ins_l, ins_r, _ins_t, _ins_b) = tb.insets;
    let m_left = ((ins_l as f32 * ppi).round().max(0.0)).clamp(0.0, 120.0) as i8;
    let m_right = ((ins_r as f32 * ppi).round().max(0.0)).clamp(0.0, 120.0) as i8;
    let margin = egui::Margin {
        left: m_left,
        right: m_right,
        top: 0,
        bottom: 0,
    };

    // Paragraph alignment through a custom layouter: the job's halign
    // and per-section line height make wrapped lines follow the
    // shape's HorzAlign and the renderer's spacing inside the
    // block-width widget (the wrap region is the renderer's: block
    // width minus BOTH insets, symmetric).
    let halign = match tb.halign {
        0 | 3 => egui::Align::Min,
        2 => egui::Align::Max,
        _ => egui::Align::Center,
    };
    let wrap_width = (screen.width() - m_left as f32 - m_right as f32).max(8.0);
    let build_job = |text: &str| {
        let mut job = egui::text::LayoutJob::simple(
            text.to_owned(),
            font_id.clone(),
            color,
            wrap_width,
        );
        job.halign = halign;
        job.keep_trailing_whitespace = true;
        for section in &mut job.sections {
            section.format.line_height = Some(line_pts);
        }
        job
    };
    let mut layouter = move |ui: &Ui, text: &dyn egui::TextBuffer, _wrap: f32| {
        let job = build_job(text.as_str());
        ui.fonts_mut(|f| f.layout_job(job))
    };
    // The wrapped line count drives the renderer's vertical band
    // (VertAlign centers total_h, not the glyph ink).
    let n_lines = ui
        .fonts_mut(|f| f.layout_job(build_job(session.draft.as_str())))
        .rows
        .len()
        .max(1) as f64;

    let id = egui::Id::unique("dc-canvas-text-edit");
    let first_frame = !session.focus_requested;
    if first_frame {
        ui.ctx().memory_mut(|m| m.request_focus(id));
        session.focus_requested = true;
    }

    // Baseline-exact anchoring (owner review: the edited text was
    // translated left-and-up). The renderer places the first baseline
    // `text_first_baseline` below the block's top edge (VertAlign
    // band of the wrapped height); egui's first row puts its baseline
    // `ascent` below the editor's text-area top. Solving for that top
    // makes the caret sit ON the rendered line — entering the editor
    // moves nothing.
    let baseline_off_in = dc::svg::text_first_baseline(&tb, n_lines * tb.line_height);
    let baseline_page_y_up = (by + bh) - baseline_off_in;
    let baseline_screen = canvas::page_to_screen(bx, baseline_page_y_up, view, geom).y;
    let editor_top = baseline_screen - (ascent_em * font_pts as f64) as f32;
    let content_h = (n_lines as f32 * line_pts + 2.0).clamp(line_pts, screen.height());
    let place = Rect::from_min_size(
        Pos2::new(screen.left(), editor_top),
        Vec2::new(screen.width(), content_h),
    );
    let edit = egui::TextEdit::multiline(&mut session.draft)
        .id(id)
        .hint_text("Shape text")
        .desired_width(f32::INFINITY)
        .layouter(&mut layouter)
        .frame(
            egui::Frame::new()
                .inner_margin(margin)
                .fill(Color32::TRANSPARENT)
                .stroke(egui::Stroke::NONE),
        );
    let resp = ui.put(place, edit);

    // Esc discards: the shape's text reverts to its pre-edit value
    // (owner review, 2026-10-27).
    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        return EditFlow::Abandon;
    }
    // A click outside the editor ends the edit. Frame 0 is exempt:
    // the opening click pair is still in this frame's input.
    if !first_frame {
        let clicked_elsewhere = ui.input(|i| i.pointer.any_click())
            && ui
                .input(|i| i.pointer.interact_pos())
                .is_some_and(|p| !resp.rect.contains(p));
        if clicked_elsewhere {
            return EditFlow::Commit;
        }
    }
    EditFlow::Active
}

/// The egui family + ascent for the shape's ACTUAL font: the face the
/// rasterizer draws (system font where installed, vendored Carlito
/// otherwise — `dc::textmetrics::face_data`), registered into egui
/// under a canvas name. The RESOLUTION (file read, family name, the
/// shared FontData payload) is cached per (document family, bold); the
/// REGISTRATION is re-asserted on every call — egui's `add_font`
/// checks the live font definitions and no-ops when the name is
/// already registered, and applies the insert immediately when nothing
/// has been laid out yet this pass. Re-asserting matters because the
/// cache is process-global while a Context is not: a fresh context
/// (a second kittest harness in the same binary) or a `set_fonts`
/// reset would otherwise silently lay the overlay out in the fallback
/// font (#22 review).
fn overlay_font(
    ctx: &egui::Context,
    font: Option<&str>,
    bold: bool,
    italic: bool,
) -> (egui::FontFamily, f64) {
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex, OnceLock};
    /// (document family, bold, italic) → (egui family, ascender em,
    /// canvas name, payload). All clones are Arc-grade — the payload
    /// is shared, never copied.
    type Resolved = (egui::FontFamily, f64, Arc<str>, egui::FontData);
    /// (document family, bold, italic) → resolved tuple — see below.
    type CacheKey = (String, bool, bool);
    static CACHE: OnceLock<Mutex<HashMap<CacheKey, Resolved>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let key = (font.unwrap_or_default().to_string(), bold, italic);
    let hit = cache.lock().unwrap().get(&key).cloned();
    let style_name = |bold: bool, italic: bool| {
        match (bold, italic) {
            (false, false) => "regular",
            (true, false) => "bold",
            (false, true) => "italic",
            (true, true) => "bolditalic",
        }
    };
    let (family, ascent, name, data) = match hit {
        Some(r) => r,
        None => {
            let out = match dc::textmetrics::face_data(font, bold, italic) {
                Some(fd) => {
                    let name: Arc<str> = Arc::from(format!(
                        "dc-canvas-{}-{}",
                        fd.family,
                        style_name(bold, italic)
                    ));
                    let data = egui::FontData {
                        font: fd.data.clone(),
                        index: fd.index,
                        tweak: Default::default(),
                    };
                    let family = egui::FontFamily::Name(name.clone());
                    (family, fd.ascent_em, name, data)
                }
                // face_data fails only when a system font file cannot
                // be read back: fall back to the vendored Carlito so
                // the editor still renders (and stays
                // metric-compatible with the raster).
                None => {
                    let name: Arc<str> = Arc::from(format!(
                        "dc-canvas-carlito-{}",
                        style_name(bold, italic)
                    ));
                    let ttf: &[u8] = match (bold, italic) {
                        (false, false) => {
                            include_bytes!("../../dc/assets/fonts/Carlito-Regular.ttf")
                        }
                        (true, false) => include_bytes!("../../dc/assets/fonts/Carlito-Bold.ttf"),
                        (false, true) => {
                            include_bytes!("../../dc/assets/fonts/Carlito-Italic.ttf")
                        }
                        (true, true) => {
                            include_bytes!("../../dc/assets/fonts/Carlito-BoldItalic.ttf")
                        }
                    };
                    let family = egui::FontFamily::Name(name.clone());
                    (family, 0.75, name, egui::FontData::from_static(ttf))
                }
            };
            cache.lock().unwrap().insert(key, out.clone());
            out
        }
    };
    ctx.add_font(egui::FontInsert::new(
        &name,
        data,
        vec![egui::InsertFontFamily {
            family: family.clone(),
            priority: egui::FontPriority::Highest,
        }],
    ));
    (family, ascent)
}

/// Parse "#rrggbb" (the renderer's resolved form) with a black
/// fallback for malformed values.
fn parse_hex_color(s: &str) -> Color32 {
    let h = s.trim_start_matches('#');
    if h.len() == 6 {
        if let Ok(v) = u32::from_str_radix(h, 16) {
            return Color32::from_rgb(
                ((v >> 16) & 0xff) as u8,
                ((v >> 8) & 0xff) as u8,
                (v & 0xff) as u8,
            );
        }
    }
    Color32::BLACK
}
