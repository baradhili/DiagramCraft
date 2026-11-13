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
        let mut job =
            egui::text::LayoutJob::simple(text.to_owned(), font_id.clone(), color, wrap_width);
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

    // #257 text-direction: bidi segment-aware arrow stepping. egui's
    // TextEdit moves the caret in LOGICAL order (Left = previous
    // character), which reads backwards inside an RTL segment; the
    // click caret is already visual (nearest-glyph). The arrows here
    // step in VISUAL direction instead: the caret's laid-out position
    // is nudged and the galley reports the nearest cursor — the
    // adjacent glyph visually, wrapping across rows like the built-in
    // movement. Consumed before the widget sees the key, so egui never
    // double-steps. (Analysis/rtl-text-direction.md.)
    let (left, right, extend) = ui.input(|i| {
        (
            i.key_pressed(egui::Key::ArrowLeft),
            i.key_pressed(egui::Key::ArrowRight),
            i.modifiers.shift,
        )
    });
    if left || right {
        let mods = ui.input(|i| i.modifiers);
        let key = if left {
            egui::Key::ArrowLeft
        } else {
            egui::Key::ArrowRight
        };
        // Only the BARE and Shift arrows take the visual path —
        // Ctrl/Alt/Cmd arrows keep egui's own cursor handling
        // (word-wise and row-end navigation), and a frame that also
        // carries TEXT input defers entirely to egui so the typed
        // character lands before any cursor move (event order must
        // decide, not this block's position — PR #269 review).
        let plain = !(mods.ctrl || mods.alt || mods.command || mods.mac_cmd);
        let has_text_events =
            ui.input(|i| i.events.iter().any(|e| matches!(e, egui::Event::Text(_))));
        if plain && !has_text_events && ui.input_mut(|i| i.consume_key(mods, key)) {
            let galley = ui.fonts_mut(|f| f.layout_job(build_job(session.draft.as_str())));
            if let Some(mut state) = egui::text_edit::TextEditState::load(ui.ctx(), id) {
                if let Some(range) = state.cursor.char_range() {
                    let (primary, had_selection) = if extend {
                        (range.primary, false)
                    } else if range.primary != range.secondary {
                        // An unmodified arrow first COLLAPSES the
                        // selection to the boundary in the pressed
                        // direction. The comparison is visual and
                        // ROW-FIRST: on a multiline (or bidi)
                        // selection the endpoints' x order says
                        // nothing about which end the arrow names —
                        // only within one row does x decide (PR #269
                        // review). Movement resumes on the next press.
                        let pp = galley.pos_from_cursor(range.primary);
                        let ps = galley.pos_from_cursor(range.secondary);
                        // (row, x) lexicographic: primary is the
                        // visually-later endpoint when this is true.
                        let primary_later =
                            (pp.min.y, pp.min.x).ge(&(ps.min.y, ps.min.x))
                                && (pp.min.y, pp.min.x) != (ps.min.y, ps.min.x);
                        let take_primary = if right { primary_later } else { !primary_later };
                        (
                            if take_primary {
                                range.primary
                            } else {
                                range.secondary
                            },
                            true,
                        )
                    } else {
                        (range.primary, false)
                    };
                    let stepped = if had_selection {
                        primary
                    } else {
                        adjacent_visual_caret(&galley, primary, right)
                    };
                    state.cursor.set_char_range(Some(egui::text::CCursorRange {
                        primary: stepped,
                        secondary: if extend { range.secondary } else { stepped },
                        h_pos: None,
                    }));
                    state.store(ui.ctx(), id);
                }
            }
        }
    }

    // Baseline-exact anchoring (owner review: the edited text was
    // translated left-and-up). The renderer places the first baseline
    // `text_first_baseline` below the block's top edge (VertAlign
    // band of the wrapped height); egui's first row puts its baseline
    // `ascent` below the editor's text-area top. Solving for that top
    // makes the caret sit ON the rendered line — entering the editor
    // moves nothing. The overlay editor lays every row out at ONE
    // size, so its baseline uses the Latin ascent; the renderer's
    // per-line complex-script slots are its own.
    let baseline_off_in =
        dc::svg::text_first_baseline(&tb, n_lines * tb.line_height, tb.font_size);
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
        .hint_text(crate::i18n::strs().msg_shape_text())
        .desired_width(f32::INFINITY)
        // One row of the OVERLAY font drives the widget's minimum —
        // egui otherwise floors multiline edits at 4 rows of the
        // DEFAULT chrome font (Proportional), whose row height
        // changed with Open Sans (#252 chrome font) and outgrew the
        // baseline-anchored place rect.
        .desired_rows(1)
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
    let style_name = |bold: bool, italic: bool| match (bold, italic) {
        (false, false) => "regular",
        (true, false) => "bold",
        (false, true) => "italic",
        (true, true) => "bolditalic",
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
                    let name: Arc<str> =
                        Arc::from(format!("dc-canvas-carlito-{}", style_name(bold, italic)));
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

/// Step the caret to the adjacent VISUAL caret stop (#257
/// text-direction, PR #269 review). egui's built-in arrows move in
/// LOGICAL order — backwards inside an RTL run — and the previous
/// 1.5 px hit-test nudge returned the SAME stop for any glyph wider
/// than 3 px, leaving plain arrows dead. This walks the galley's
/// laid-out stops directly: within the caret's row it steps to the
/// neighboring x position (crossing bidi run boundaries one stop at a
/// time), and at a row's edge it crosses to the far end of the
/// adjacent row — wrapped rows AND explicit line breaks both move the
/// way the built-in arrows move LTR text.
fn adjacent_visual_caret(
    galley: &egui::Galley,
    cur: egui::text::CCursor,
    right: bool,
) -> egui::text::CCursor {
    use egui::text::CCursor;

    // Char span of each row (CCursor counts CHARS): a row consumes
    // one char per glyph plus the '\n' when it ends one (epaint omits
    // the newline glyph).
    let mut spans: Vec<(usize, usize)> = Vec::with_capacity(galley.rows.len());
    {
        let mut c = 0usize;
        for r in &galley.rows {
            let start = c;
            c += r.row.glyphs.len() + usize::from(r.ends_with_newline);
            spans.push((start, c));
        }
    }

    // Every caret stop of one row, as (x, cursor): both prefer_next_row
    // variants of each char boundary in the row's span (a boundary a
    // soft wrap shares with the next row reports the row it faces, so
    // the y-band check below sorts it into the right list). The
    // trailing end-of-text boundary belongs to the last row only.
    let stops_in = |rix: usize| -> Vec<(f32, CCursor)> {
        let Some(&(start, end)) = spans.get(rix) else {
            return Vec::new();
        };
        let band = galley.rows[rix].rect();
        let mut stops: Vec<(f32, CCursor)> = Vec::new();
        // Every char boundary of the span, BOTH affinities: at a soft
        // wrap the shared boundary maps to the END of this row with
        // prefer_next_row=false and the START of the next with true —
        // the y-band check keeps each facing its own row, so this
        // row's end stop survives (PR #269 review: excluding the
        // boundary outright left the caret unable to sit at a wrapped
        // row's end, and Left skipped a character).
        for i in start..=end {
            for pn in [false, true] {
                let c = CCursor {
                    index: egui::text::CharIndex(i),
                    prefer_next_row: pn,
                };
                let p = galley.pos_from_cursor(c);
                if p.min.y >= band.min.y && p.max.y <= band.max.y {
                    stops.push((p.min.x, c));
                }
            }
        }
        // Visual order; same-gap duplicates (the (i, prefer_next_row) /
        // (i+1, !prefer_next_row) pair a shared x produces, and the two
        // variants of one boundary when a glyph is zero-width) collapse
        // to their first representative so LTR stepping never burns a
        // keypress standing still. Distinct boundaries that merely
        // share an x (a bidi run junction) stay separate stops.
        stops.sort_by(|a, b| {
            a.0.total_cmp(&b.0)
                .then(a.1.index.0.cmp(&b.1.index.0))
                .then(a.1.prefer_next_row.cmp(&b.1.prefer_next_row))
        });
        let mut dedup: Vec<(f32, CCursor)> = Vec::with_capacity(stops.len());
        for s in stops {
            if let Some(last) = dedup.last() {
                let same_gap = (last.1.index.0 == s.1.index.0)
                    || (last.1.index.0 + 1 == s.1.index.0
                        && last.1.prefer_next_row
                        && !s.1.prefer_next_row);
                if last.0 == s.0 && same_gap {
                    continue;
                }
            }
            dedup.push(s);
        }
        dedup
    };

    // The caret's row (vertical band of its own slot).
    let cur_pos = galley.pos_from_cursor(cur);
    let row_ix = galley
        .rows
        .iter()
        .position(|r| {
            let rect = r.rect();
            cur_pos.min.y < rect.max.y && cur_pos.max.y > rect.min.y
        })
        .unwrap_or(0);
    let stops = stops_in(row_ix);

    // Where the current caret sits in the visual order — exact, else
    // the stop at its x (a collapsed duplicate), else the nearest.
    let at = stops
        .iter()
        .position(|s| s.1 == cur)
        .or_else(|| {
            stops.iter().position(|s| {
                (s.0 - cur_pos.min.x).abs() < 0.5
                    && s.1.index.0 <= cur.index.0
                    && cur.index.0 <= s.1.index.0 + 1
            })
        })
        .or_else(|| {
            stops
                .iter()
                .enumerate()
                .min_by(|a, b| {
                    (a.1 .0 - cur_pos.min.x)
                        .abs()
                        .total_cmp(&(b.1 .0 - cur_pos.min.x).abs())
                })
                .map(|(i, _)| i)
        });

    let Some(at) = at else { return cur };
    // Step within the row; None = past its edge (left of the first
    // stop, right of the last, or before the row's start at all).
    let next = if right {
        Some(at + 1)
    } else {
        at.checked_sub(1)
    };
    if let Some(stop) = next.and_then(|n| stops.get(n)) {
        return stop.1;
    }
    // Past the row's edge: cross to the far end of the adjacent row —
    // or stand still at the text's outer ends.
    let hop: Option<usize> = if right {
        Some(row_ix + 1)
    } else {
        row_ix.checked_sub(1)
    };
    match hop.filter(|&h| h < galley.rows.len()).map(stops_in) {
        Some(hop_stops) if !hop_stops.is_empty() => {
            let pick = if right {
                hop_stops.first()
            } else {
                hop_stops.last()
            };
            pick.map(|s| s.1).unwrap_or(cur)
        }
        _ => cur,
    }
}

#[cfg(test)]
mod tests {
    use super::adjacent_visual_caret;
    use egui::text::CCursor;

    fn galley_of(ctx: &egui::Context, text: &str) -> std::sync::Arc<egui::Galley> {
        let job = egui::text::LayoutJob::simple(
            text.into(),
            egui::FontId::monospace(14.0),
            egui::Color32::WHITE,
            f32::INFINITY,
        );
        ctx.fonts_mut(|f| f.layout_job(job))
    }

    fn wrapped_galley_of(ctx: &egui::Context, text: &str, wrap: f32) -> std::sync::Arc<egui::Galley> {
        let job = egui::text::LayoutJob::simple(
            text.into(),
            egui::FontId::monospace(14.0),
            egui::Color32::WHITE,
            wrap,
        );
        ctx.fonts_mut(|f| f.layout_job(job))
    }

    /// #257 text-direction / PR #269 review: the visual stepping moves
    /// one glyph per press through plain Latin text, crosses explicit
    /// line breaks to the far end of the next row, and round-trips an
    /// RTL run (logical backwards is visual forwards).
    #[test]
    fn visual_caret_steps_glyphs_rows_and_rtl_runs() {
        let ctx = egui::Context::default();
        ctx.begin_pass(egui::RawInput::default());

        // Latin with an explicit break: 0 → 1 → … → past 'b' crosses
        // to 'c' (the next row's left end), never lands mid-newline.
        let g = galley_of(&ctx, "ab\ncd");
        let mut c = CCursor::new(0);
        c = adjacent_visual_caret(&g, c, true);
        assert_eq!(c.index.0, 1, "a|b after one right");
        c = adjacent_visual_caret(&g, c, true);
        assert_eq!(c.index.0, 2, "ab| at the break");
        c = adjacent_visual_caret(&g, c, true);
        assert_eq!(c.index.0, 3, "across the break to |cd");
        // …and back: left from 'c' returns to the break, then 'b'.
        c = adjacent_visual_caret(&g, c, false);
        assert_eq!(c.index.0, 2, "back across the break");
        c = adjacent_visual_caret(&g, c, false);
        assert_eq!(c.index.0, 1, "a|b again");
        // The outer ends stand still instead of wrapping or panicking.
        assert_eq!(
            adjacent_visual_caret(&g, CCursor::new(0), false).index.0,
            0,
            "left of the text stays put"
        );
        assert_eq!(
            adjacent_visual_caret(&g, CCursor::new(5), true).index.0,
            5,
            "right of the text stays put"
        );

        // A Persian word: logical order runs right-to-left visually, so
        // stepping right from the text START must MOVE (the old 1.5 px
        // nudge returned the same stop), and stepping back returns.
        let fa = "\u{0633}\u{0644}\u{0627}\u{0645}";
        let g = galley_of(&ctx, fa);
        let start = CCursor::new(0);
        let c = adjacent_visual_caret(&g, start, true);
        assert_ne!(c, start, "right moves inside an rtl run");
        assert_eq!(
            adjacent_visual_caret(&g, c, false),
            start,
            "left returns to the start"
        );

        // Soft wrap (PR #269 review): the shared boundary at a wrap
        // keeps its END affinity on the earlier row — Left from
        // "ab|cd" (wrapped after "ab") steps to "a|bcd", not past
        // both characters.
        let g = wrapped_galley_of(&ctx, "abcd", 20.0);
        assert!(
            g.rows.len() == 2,
            "the narrow width wraps after ab: {} rows",
            g.rows.len()
        );
        let at_wrap = CCursor::new(2);
        let stepped = adjacent_visual_caret(&g, at_wrap, false);
        assert_eq!(stepped.index.0, 1, "left crosses one stop at the wrap");

        let mut out = ctx.end_pass();
        // Headless: the pass's texture delta is intentionally dropped
        // (nothing rasterizes it) — clear it so its Drop doesn't panic.
        out.textures_delta.clear();
    }
}
