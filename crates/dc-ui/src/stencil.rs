//! The Shapes stencil pane: every master in the document as a
//! thumbnail (rendered through the same SVG backend), selectable and
//! droppable onto the canvas.

use dc::svg;
use dc_core::Editor;
use egui::{Color32, Pos2, Rect, Sense, Ui, Vec2};

use crate::svgimg;
use crate::theme;

/// One master cell: a 64×64 thumbnail plus a caption strip below it.
/// Cells never rescale when the pane widens — only the arrangement
/// (how many fit per row) re-wraps, the way Visio re-wraps its
/// stencil masters (GitHub #53).
const CELL_W: f32 = 64.0;
const CELL_H: f32 = 78.0;
/// Gap between cells, x and y (unchanged from the fixed grid).
const GAP: f32 = 6.0;
/// Salt of the grid's ScrollArea, so its state can be read and
/// re-anchored when a resize re-wraps the rows.
const SCROLL_SALT: &str = "stencil-grid";

/// The pane's state: which master (by id) is armed for dropping.
#[derive(Default)]
pub struct Stencil {
    pub armed: Option<i64>,
    /// Master currently being dragged from the pane (button held);
    /// the app turns a release over the canvas into a placement.
    pub carry: Option<i64>,
    /// "Search Shapes" filter (Visio stencil pane search box).
    pub search: String,
    /// Master cell rects (screen coords) from the last layout — used by
    /// the kittest interaction tests to drive drag-from-stencil.
    pub cell_rects: Vec<(i64, egui::Rect)>,
    /// Column count of the last laid-out grid. When a resize changes
    /// it, the scroll offset is re-anchored so the top visible master
    /// stays visible (GitHub #53).
    pub last_cols: usize,
    /// Content width the grid last wrapped into (after the scrollbar
    /// allowance); tests re-derive the expected column count from it.
    pub last_wrap_width: f32,
    /// Vertical offset to force on the next frame after a re-wrap
    /// moved the master rows.
    scroll_fix: Option<f32>,
    /// The stencil list entry is collapsed (master grid hidden).
    pub collapsed: bool,
    textures: Vec<(i64, egui::TextureHandle)>,
    loaded_for: Option<u64>, // doc identity (master count + first ids hash)
}

/// Columns that fit `width`: C columns occupy `C*CELL_W + (C-1)*GAP`
/// px, so the widest fit is `floor((width + GAP) / (CELL_W + GAP))`,
/// at least one. `scrollbar` keeps the last column out from under the
/// pane's floating scrollbar (Visio reserves that space too).
fn columns_for(width: f32, scrollbar: f32) -> usize {
    let avail = (width - scrollbar).max(CELL_W);
    (((avail + GAP) / (CELL_W + GAP)).floor() as usize).max(1)
}

/// Blend `b` into `a` (t = 0 → a): a theme-token way to derive hover
/// and selection tints that read in both light and dark visuals.
fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let (a, b) = (egui::Rgba::from(a), egui::Rgba::from(b));
    (a * (1.0 - t) + b * t).into()
}

impl Stencil {
    pub fn show(&mut self, ui: &mut Ui, ed: &Editor, stencil_title: &str) -> Option<i64> {
        // Pane chrome follows the active visual mode (GitHub #52);
        // thumbnails keep their own light raster (paper-like tiles).
        let t = theme::shell_tokens(ui.visuals());
        // (Re)load thumbnails when the master set changes.
        let fingerprint = {
            use std::collections::hash_map::DefaultHasher;
            use std::hash::{Hash, Hasher};
            let mut h = DefaultHasher::new();
            ed.doc.masters.len().hash(&mut h);
            for m in ed.doc.masters.iter().take(8) {
                m.id.hash(&mut h);
            }
            h.finish()
        };
        if self.loaded_for != Some(fingerprint) {
            self.textures.clear();
            self.loaded_for = Some(fingerprint);
        }

        let mut clicked = None;
        // Visio pane header: "Shapes" title bar with a "More Shapes"
        // affordance, then the search box, then the stencil list.
        ui.horizontal(|ui| {
            ui.strong("Shapes");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.weak("More Shapes ...");
            });
        });
        ui.add(
            egui::TextEdit::singleline(&mut self.search)
                .hint_text("Search Shapes")
                .desired_width(ui.available_width()),
        );
        ui.add_space(4.0);

        // Visio lists the open stencils above the masters; this
        // document carries one master set, shown as a single
        // expandable entry (GitHub #53 review note: "the stencils are
        // not listed" — now they are).
        if stencil_list_entry(ui, t, stencil_title, !self.collapsed).clicked() {
            self.collapsed = !self.collapsed;
        }
        if self.collapsed {
            self.cell_rects.clear();
            return clicked;
        }

        ui.add_space(2.0);
        let scroll_id = ui.make_persistent_id(egui::IdSalt::new(SCROLL_SALT));
        let mut area = egui::ScrollArea::vertical().id_salt(SCROLL_SALT);
        if let Some(y) = self.scroll_fix.take() {
            area = area.vertical_scroll_offset(y);
        }
        let mut wrap_width = self.last_wrap_width;
        let mut cols = self.last_cols.max(1);
        area.show(ui, |ui| {
            // Live width: masters re-wrap while the pane resize drag is
            // still in motion — no settle step (GitHub #53).
            let scrollbar = ui.style().spacing.scroll.bar_width + 2.0;
            cols = columns_for(ui.available_width(), scrollbar);
            wrap_width = (ui.available_width() - scrollbar).max(CELL_W);
            ui.set_min_width(wrap_width);
            let q = self.search.to_lowercase();
            let mut ordered: Vec<&vsdx::Master> = ed
                .doc
                .masters
                .iter()
                .filter(|m| {
                    q.is_empty()
                        || m.name_u
                            .as_deref()
                            .map(|n| n.to_lowercase().contains(&q))
                            .unwrap_or(false)
                })
                .collect();
            ordered.sort_by_key(|m| m.id);
            if ordered.is_empty() {
                ui.weak("No matching shapes.");
            }
            egui::Grid::new("stencil")
                .num_columns(cols)
                .spacing([GAP, GAP])
                .min_col_width(CELL_W)
                .show(ui, |ui| {
                    self.cell_rects.clear();
                    let n = ordered.len();
                    for (i, m) in ordered.into_iter().enumerate() {
                        let tex = self.texture(ui.ctx(), ed, m.id);
                        let armed = self.armed == Some(m.id);
                        // one grid cell: 64×78 (thumb + caption strip)
                        let size = Vec2::new(CELL_W, CELL_H);
                        // click_and_drag: click arms placement, drag
                        // carries the master onto the page (the Visio
                        // paradigm — plain Sense::click cannot drag).
                        let (r, resp) = ui.allocate_exact_size(size, Sense::click_and_drag());
                        self.cell_rects.push((m.id, r));
                        let resp = resp.on_hover_text(format!(
                            "{} (master {})",
                            m.name_u.as_deref().unwrap_or("-"),
                            m.id
                        ));
                        let p = ui.painter();
                        // Visio masters sit directly on the pane — no
                        // tile and no resting border. Hover tints the
                        // cell; an armed master carries the accent
                        // (placement feedback), both as soft fills.
                        if armed {
                            p.rect_filled(r, 2.0, mix(t.stencil_bg, t.accent, 0.25));
                            p.rect_stroke(
                                r,
                                2.0,
                                egui::Stroke::new(1.0, t.accent),
                                egui::StrokeKind::Middle,
                            );
                        } else if resp.hovered() {
                            p.rect_filled(r, 2.0, mix(t.stencil_bg, t.caption, 0.10));
                        }
                        let thumb = Rect::from_min_size(r.left_top(), Vec2::new(CELL_W, CELL_W));
                        p.image(
                            tex.id(),
                            thumb.shrink(2.0),
                            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                            Color32::WHITE,
                        );
                        // caption
                        let gal = p.layout_no_wrap(
                            short(m.name_u.as_deref().unwrap_or("-")),
                            egui::FontId::proportional(9.5),
                            t.caption,
                        );
                        p.galley(
                            Pos2::new(r.center().x - gal.size().x / 2.0, r.bottom() - 13.0),
                            gal,
                            Color32::WHITE,
                        );
                        if resp.clicked() {
                            self.armed = Some(m.id);
                            clicked = Some(m.id);
                        }
                        if resp.drag_started_by(egui::PointerButton::Primary) {
                            self.carry = Some(m.id);
                        }
                        // A row ends after its last column, not after
                        // every master — `end_row` per master would
                        // collapse the grid to one column regardless
                        // of `num_columns`.
                        if (i + 1) % cols == 0 || i + 1 == n {
                            ui.end_row();
                        }
                    }
                });
        });

        // A resize changed the row structure: re-anchor the scroll so
        // the master that was topmost stays topmost under the new
        // column count (applied on the next frame's scroll offset).
        if self.last_cols != 0 && self.last_cols != cols {
            let pitch = CELL_H + GAP;
            if let Some(st) = egui::scroll_area::State::load(ui.ctx(), scroll_id) {
                let first_visible =
                    ((st.unclamped_offset().y / pitch).floor().max(0.0) as usize) * self.last_cols;
                let top = first_visible.min(self.cell_rects.len().saturating_sub(1));
                self.scroll_fix = Some((top / cols) as f32 * pitch);
            }
        }
        self.last_cols = cols;
        self.last_wrap_width = wrap_width;
        clicked
    }

    /// Thumbnail texture for a master, building it on first use
    /// (also used for the drag ghost).
    pub fn texture(
        &mut self,
        ctx: &egui::Context,
        ed: &Editor,
        master_id: i64,
    ) -> egui::TextureHandle {
        if let Some(tex) = self.textures.iter().find(|(id, _)| *id == master_id) {
            return tex.1.clone();
        }
        let m = ed.doc.master(master_id).expect("master exists");
        let svg = svg::render_master(&ed.doc, m).unwrap_or_default();
        let img = svgimg::rasterize(&svg, 64.0)
            .unwrap_or_else(|_| egui::ColorImage::new([64, 64], vec![Color32::GRAY; 64 * 64]));
        let tex = ctx.load_texture(
            format!("master-{master_id}"),
            img,
            egui::TextureOptions::LINEAR,
        );
        self.textures.push((master_id, tex.clone()));
        tex
    }
}

/// One Visio-style stencil list row: a disclosure chevron and the
/// stencil name; the expanded entry is highlighted like Visio's
/// active stencil entry.
fn stencil_list_entry(
    ui: &mut Ui,
    t: theme::ShellTokens,
    title: &str,
    expanded: bool,
) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 20.0), Sense::click());
    let p = ui.painter();
    if expanded {
        p.rect_filled(r, 1.0, mix(t.stencil_bg, t.selection, 0.15));
    } else if resp.hovered() {
        p.rect_filled(r, 1.0, mix(t.stencil_bg, t.caption, 0.08));
    }
    // Chevron: a small painted triangle (deterministic under the
    // pinned test fonts) pointing down when expanded, right when not.
    let c = Pos2::new(r.left() + 9.0, r.center().y);
    let tri: Vec<Pos2> = if expanded {
        vec![
            c + Vec2::new(-4.0, -2.0),
            c + Vec2::new(4.0, -2.0),
            c + Vec2::new(0.0, 2.5),
        ]
    } else {
        vec![
            c + Vec2::new(-2.0, -4.0),
            c + Vec2::new(-2.0, 4.0),
            c + Vec2::new(2.5, 0.0),
        ]
    };
    p.add(egui::Shape::convex_polygon(
        tri,
        t.label,
        egui::Stroke::NONE,
    ));
    let gal = p.layout_no_wrap(title.to_string(), egui::FontId::proportional(11.0), t.label);
    p.galley(
        Pos2::new(r.left() + 18.0, r.center().y - gal.size().y / 2.0),
        gal,
        Color32::WHITE,
    );
    resp
}

fn short(s: &str) -> String {
    const MAX: usize = 10;
    if s.chars().count() <= MAX {
        s.to_string()
    } else {
        let t: String = s.chars().take(MAX - 1).collect();
        format!("{t}…")
    }
}
