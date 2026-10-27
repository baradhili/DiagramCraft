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
//! End-of-edit semantics: Visio's text editing is LIVE — the shape's
//! text tracks the edit as you type and Esc only exits the mode, so
//! Esc KEEPS the typed text (a following Ctrl+Z reverts the whole
//! edit in one step). [`ESC_COMMIT_TEXT`] pins that decision; flip it
//! if a run against the shipped Visio 2016 install ever shows the
//! opposite. (A live observation from repo automation was attempted
//! 2026-10-27: the wine launch from the agent session exits
//! immediately, so the live check remains open — the shipped install
//! is the oracle when someone is next at the desktop.)
//!
//! v1 scope: unrotated placement — a rotated shape (`Angle`/
//! `TxtAngle`) yields the block's axis-aligned box; italic renders as
//! regular (the embedded canvas family carries Regular/Bold only);
//! the editor fills the block vertically rather than re-implementing
//! the renderer's VertAlign baseline math.

use crate::canvas::{self, PageGeom, View};
use dc::svg::{text_block, text_block_page_rect};
use egui::{Color32, FontId, Pos2, Rect, Ui, Vec2};

/// The registered canvas font family (embedded Carlito —
/// metric-compatible with Calibri, the Visio default; the same face
/// `svgimg` pins documents to when nothing installed matches). Bold
/// text uses the `… Bold` sibling family, because egui `FontId` has
/// no weight of its own.
const CANVAS_FAMILY: &str = "DiagramCraft Canvas";

/// Esc exits the editor and KEEPS the typed text (Visio's live-text
/// model — see the module docs).
const ESC_COMMIT_TEXT: bool = true;

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
    /// Last frame's rendered editor height — the VertAlign anchor is
    /// computed from it (the galley estimate is only close; the real
    /// widget height converges here from frame two on).
    pub last_h: Option<f32>,
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
            last_h: None,
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
    /// End the session, committing the session's draft via `SetText`.
    Commit,
    /// End the session WITHOUT committing (only reachable by flipping
    /// [`ESC_COMMIT_TEXT`] off).
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

    // WYSIWYG metrics from the same cells the renderer reads: font
    // size in egui points (≈ logical px) = inches × 96 × zoom, and
    // the block insets scaled to the screen (egui margins are i8
    // points, hence the clamp).
    let ppi = view.px_per_in();
    ensure_fonts(ui.ctx());
    let font_pts = (tb.font_size as f32 * 96.0 * view.zoom).clamp(4.0, 400.0);
    let font_id = FontId::new(
        font_pts,
        if tb.bold {
            egui::FontFamily::Name(std::sync::Arc::from(format!("{CANVAS_FAMILY} Bold")))
        } else {
            egui::FontFamily::Name(std::sync::Arc::from(CANVAS_FAMILY))
        },
    );
    let color = parse_hex_color(&tb.color);
    let line_pts = (tb.line_height as f32 * 96.0 * view.zoom).clamp(font_pts, 1000.0);
    let (ins_l, _ins_r, ins_t, _ins_b) = tb.insets;
    let m_left = ((ins_l as f32 * ppi).round().max(2.0)).clamp(0.0, 120.0) as i8;
    let m_top = ((ins_t as f32 * ppi).round().max(1.0)).clamp(0.0, 120.0) as i8;
    let margin = egui::Margin {
        left: m_left,
        right: 4,
        top: m_top,
        bottom: 2,
    };

    // Paragraph alignment through a custom layouter: the job's halign
    // and per-section line height make wrapped lines follow the
    // shape's HorzAlign and the renderer's spacing inside the
    // block-width widget.
    let halign = match tb.halign {
        0 | 3 => egui::Align::Min,
        2 => egui::Align::Max,
        _ => egui::Align::Center,
    };
    let wrap_width = (screen.width() - m_left as f32 - 4.0).max(8.0);
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

    #[allow(deprecated)] // Id::new: one editor, stable identity wanted
    let id = egui::Id::new("dc-canvas-text-edit");
    let first_frame = !session.focus_requested;
    if first_frame {
        ui.ctx().memory_mut(|m| m.request_focus(id));
        session.focus_requested = true;
    }

    // Anchor the edited text where the renderer draws it (owner
    // report on #22: the text must not jump to the block's top-left
    // on entry). The editor spans the block's WIDTH (so the layouter's
    // HorzAlign positions lines); its height is the content's, placed
    // per the renderer's VertAlign (1 top, 2 middle default, 3
    // bottom) — egui's own main_align cannot do this against its
    // infinite layout cursor, so the anchor is computed here from the
    // same galley the layouter will build.
    let valign = match tb.valign {
        1 => egui::Align::Min,
        3 => egui::Align::Max,
        _ => egui::Align::Center,
    };
    let est_h = {
        let g = ui.fonts_mut(|f| f.layout_job(build_job(session.draft.as_str())));
        g.size().y + m_top as f32 + 2.0 + 6.0
    }
    .max(line_pts + m_top as f32 + 8.0);
    let content_h = session.last_h.unwrap_or(est_h).min(screen.height());
    let free = (screen.height() - content_h).max(0.0);
    let anchor_y = match valign {
        egui::Align::Min => 0.0,
        egui::Align::Max => free,
        _ => free / 2.0,
    };
    let place = Rect::from_min_size(
        Pos2::new(screen.left(), screen.top() + anchor_y),
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
    session.last_h = Some(resp.rect.height());

    // Esc: exit per the pinned Visio semantics (keep — module docs).
    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        return if ESC_COMMIT_TEXT {
            EditFlow::Commit
        } else {
            EditFlow::Abandon
        };
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

/// Register the embedded Carlito faces under the canvas family
/// names. `Context::add_font` is idempotent by font name (it checks
/// the installed `font_data` first), so calling this every overlay
/// frame is a cheap map lookup.
fn ensure_fonts(ctx: &egui::Context) {
    insert_font(
        ctx,
        CANVAS_FAMILY,
        include_bytes!("../assets/fonts/Carlito-Regular.ttf"),
    );
    insert_font(
        ctx,
        &format!("{CANVAS_FAMILY} Bold"),
        include_bytes!("../assets/fonts/Carlito-Bold.ttf"),
    );
}

fn insert_font(ctx: &egui::Context, name: &str, ttf: &'static [u8]) {
    ctx.add_font(egui::FontInsert::new(
        name,
        egui::FontData::from_static(ttf),
        vec![egui::InsertFontFamily {
            family: egui::FontFamily::Name(std::sync::Arc::from(name)),
            priority: egui::FontPriority::Highest,
        }],
    ));
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
