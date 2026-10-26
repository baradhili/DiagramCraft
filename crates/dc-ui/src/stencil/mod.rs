//! The Shapes stencil pane (GitHub #53, #63): the document's own
//! masters plus every open stencil, each its own expandable entry
//! with a master grid (thumbnails rendered through the same SVG
//! backend, selectable and droppable onto the canvas).
//!
//! Stencil management is split out of `app.rs` (owner direction,
//! 2026-10-26): the open-stencil set lives in [`source::StencilSet`],
//! the built-in catalog in `catalog` (#87).

pub mod source;

pub use source::{MasterKey, Origin, StencilId, StencilSet, DOC_SERIAL};

use dc::svg;
use dc_core::Editor;
use egui::{Color32, Pos2, Rect, Sense, Ui, Vec2};

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

/// One renderable master: its pane identity (never a bare master id —
/// ids collide across stencils, #82/#85), its cache serial, the
/// source document to render/thumbnail from, and the master itself.
struct Cell<'a> {
    key: MasterKey,
    serial: u64,
    src: &'a vsdx::Document,
    master: &'a vsdx::Master,
}

/// Does a master match the "Search Shapes" filter (#86)?
fn matches(m: &vsdx::Master, q: &str) -> bool {
    q.is_empty()
        || m
            .name_u
            .as_deref()
            .map(|n| n.to_lowercase().contains(q))
            .unwrap_or(false)
}

/// The pane's state: which master — addressed by (source, id) — is
/// armed for dropping.
#[derive(Default)]
pub struct Stencil {
    pub armed: Option<MasterKey>,
    /// Master currently being dragged from the pane (button held);
    /// the app turns a release over the canvas into a placement.
    pub carry: Option<MasterKey>,
    /// "Search Shapes" filter (Visio stencil pane search box).
    pub search: String,
    /// Master cell rects (screen coords) from the last layout — used by
    /// the kittest interaction tests to drive drag-from-stencil.
    pub cell_rects: Vec<(MasterKey, egui::Rect)>,
    /// Stencil list entry header rects from the last layout (screen
    /// coords) — kittest drives expand/collapse through them.
    pub entry_rects: Vec<(StencilId, egui::Rect)>,
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
    /// The document-masters entry is collapsed (master grid hidden).
    pub collapsed: bool,
    /// Entry to scroll into view next frame (#83: a just-opened or
    /// re-focused stencil).
    pub reveal: Option<StencilId>,
    /// Thumbnails keyed by (cache serial, master id) — the serial is
    /// session-unique per open stencil (and 0 for the document set),
    /// so ids that collide across stencils never collide here (#85).
    textures: Vec<(u64, i64, egui::TextureHandle)>,
    /// First-use thumbnails queued on the raster pool (#92).
    inflight: std::collections::HashSet<(u64, i64)>,
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
    pub fn show(
        &mut self,
        ui: &mut Ui,
        ed: &Editor,
        set: &mut StencilSet,
        stencil_title: &str,
        pool: &mut crate::rasterpool::RasterPool,
    ) -> Option<MasterKey> {
        // Pane chrome follows the active visual mode (GitHub #52);
        // thumbnails keep their own light raster (paper-like tiles).
        let t = theme::shell_tokens(ui.visuals());
        // (Re)load the DOCUMENT entry's thumbnails when the edited
        // document's master set changes; open-stencil textures are
        // keyed by their session serial and outlive this check.
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
            self.evict(DOC_SERIAL);
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
        self.cell_rects.clear();
        self.entry_rects.clear();

        // Visio lists the open stencils as independently expandable
        // entries, the document's own masters first (the document
        // stencil), every entry scrolling in one pane column (#63).
        ui.add_space(2.0);
        let scroll_id = ui.make_persistent_id(egui::IdSalt::new(SCROLL_SALT));
        let mut area = egui::ScrollArea::vertical().id_salt(SCROLL_SALT);
        if let Some(y) = self.scroll_fix.take() {
            area = area.vertical_scroll_offset(y);
        }
        let mut wrap_width = self.last_wrap_width;
        let mut cols = self.last_cols.max(1);
        let q = self.search.to_lowercase();
        let reveal = self.reveal.take();
        area.show(ui, |ui| {
            // Live width: masters re-wrap while the pane resize drag is
            // still in motion — no settle step (GitHub #53).
            let scrollbar = ui.style().spacing.scroll.bar_width + 2.0;
            cols = columns_for(ui.available_width(), scrollbar);
            wrap_width = (ui.available_width() - scrollbar).max(CELL_W);
            ui.set_min_width(wrap_width);

            let header = |ui: &mut Ui, id: StencilId, title: &str, expanded: bool| {
                let mut resp = stencil_list_entry(ui, t, title, expanded);
                if reveal == Some(id) {
                    resp.scroll_to_me(Some(egui::Align::TOP));
                }
                resp
            };

            // Entry 0: the document's own masters.
            let resp = header(ui, StencilId::Document, stencil_title, !self.collapsed);
            self.entry_rects.push((StencilId::Document, resp.rect));
            if resp.clicked() {
                self.collapsed = !self.collapsed;
            }
            if !self.collapsed {
                let cells = doc_cells(ed, &q);
                if cells.is_empty() {
                    ui.weak(if q.is_empty() { "No shapes." } else { "No matching shapes." });
                } else {
                    self.entry_grid(ui, t, &cells, cols, pool, &mut clicked);
                }
            }

            // Then every open stencil, in open order, each with its own
            // expansion — session state, never file content (#82).
            for ix in 0..set.len() {
                let id = StencilId::Open(ix);
                let (title, expanded) = match set.get(ix) {
                    Some(st) => (st.title.as_str(), st.expanded),
                    None => continue,
                };
                let resp = header(ui, id, title, expanded);
                self.entry_rects.push((id, resp.rect));
                if resp.clicked() {
                    set.set_expanded(ix, !expanded);
                }
                if expanded {
                    let cells = stencil_cells(set, ix, &q);
                    if !cells.is_empty() {
                        self.entry_grid(ui, t, &cells, cols, pool, &mut clicked);
                    }
                }
                ui.add_space(2.0);
            }
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

    /// One entry's master grid: thumbnails from each cell's own source
    /// document, click-to-arm and drag-to-carry per cell (#53/#63).
    fn entry_grid(
        &mut self,
        ui: &mut Ui,
        t: theme::ShellTokens,
        cells: &[Cell<'_>],
        cols: usize,
        pool: &mut crate::rasterpool::RasterPool,
        clicked: &mut Option<MasterKey>,
    ) {
        let serial = cells[0].serial;
        egui::Grid::new(format!("stencil-grid-{serial}"))
            .num_columns(cols)
            .spacing([GAP, GAP])
            .min_col_width(CELL_W)
            .show(ui, |ui| {
                let n = cells.len();
                for (i, cell) in cells.iter().enumerate() {
                    let key = cell.key;
                    let tex =
                        self.texture(ui.ctx(), cell.serial, cell.src, cell.master.id, pool);
                    let armed = self.armed == Some(key);
                    // one grid cell: 64×78 (thumb + caption strip)
                    let size = Vec2::new(CELL_W, CELL_H);
                    // click_and_drag: click arms placement, drag
                    // carries the master onto the page (the Visio
                    // paradigm — plain Sense::click cannot drag).
                    let (r, resp) = ui.allocate_exact_size(size, Sense::click_and_drag());
                    self.cell_rects.push((key, r));
                    let resp = resp.on_hover_text(format!(
                        "{} (master {})",
                        cell.master.name_u.as_deref().unwrap_or("-"),
                        cell.master.id
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
                    match tex {
                        Some(tex) => {
                            p.image(
                                tex.id(),
                                thumb.shrink(2.0),
                                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                                Color32::WHITE,
                            );
                        }
                        // Placeholder (#92): a neutral tile while a
                        // worker rasterizes the thumbnail — never a
                        // blank pane flash. The sync seam means the
                        // goldens never take this branch.
                        None => {
                            p.rect_filled(
                                thumb.shrink(2.0),
                                2.0,
                                mix(t.stencil_bg, t.caption, 0.30),
                            );
                        }
                    }
                    // caption
                    let gal = p.layout_no_wrap(
                        short(cell.master.name_u.as_deref().unwrap_or("-")),
                        egui::FontId::proportional(9.5),
                        t.caption,
                    );
                    p.galley(
                        Pos2::new(r.center().x - gal.size().x / 2.0, r.bottom() - 13.0),
                        gal,
                        Color32::WHITE,
                    );
                    if resp.clicked() {
                        self.armed = Some(key);
                        *clicked = Some(key);
                    }
                    if resp.drag_started_by(egui::PointerButton::Primary) {
                        self.carry = Some(key);
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
    }

    /// Thumbnail texture for a master, building it on first use (also
    /// used for the drag ghost). The raster runs on the pool (#92):
    /// while a first-use thumbnail is on a worker the caller gets
    /// `None` and paints the neutral placeholder — the sync seam makes
    /// this never-`None` for the kittest goldens.
    pub fn texture(
        &mut self,
        ctx: &egui::Context,
        serial: u64,
        src: &vsdx::Document,
        master_id: i64,
        pool: &mut crate::rasterpool::RasterPool,
    ) -> Option<egui::TextureHandle> {
        let key = (serial, master_id);
        if let Some(tex) = self.textures.iter().find(|(s, id, _)| (*s, *id) == key) {
            return Some(tex.2.clone());
        }
        if self.inflight.contains(&key) {
            return None; // on a worker; lands in a later drain
        }
        let m = src.master(master_id).expect("master exists");
        let svg = {
            let _s = crate::perf::span(crate::perf::Stage::Svg(crate::perf::Kind::Thumb));
            svg::render_master(src, m).unwrap_or_default()
        };
        match pool.post(
            crate::rasterpool::RasterKey::Thumb { serial, master: master_id },
            crate::rasterpool::RasterJob::Parse { svg, width: 64.0 },
        ) {
            crate::rasterpool::Posted::Sync(img, _) => Some(self.insert(ctx, key, img)),
            crate::rasterpool::Posted::Queued => {
                self.inflight.insert(key);
                None
            }
            crate::rasterpool::Posted::Capped => None, // retry next frame
        }
    }

    /// Upload a finished thumbnail (gray fallback tile on failure) and
    /// cache it by (serial, master id).
    fn insert(
        &mut self,
        ctx: &egui::Context,
        key: (u64, i64),
        img: anyhow::Result<egui::ColorImage>,
    ) -> egui::TextureHandle {
        let img = img
            .unwrap_or_else(|_| egui::ColorImage::new([64, 64], vec![Color32::GRAY; 64 * 64]));
        let (serial, master_id) = key;
        let tex = {
            let _s = crate::perf::span(crate::perf::Stage::Upload(crate::perf::Kind::Thumb));
            ctx.load_texture(
                format!("master-{serial}-{master_id}"),
                img,
                egui::TextureOptions::LINEAR,
            )
        };
        self.textures.push((serial, master_id, tex.clone()));
        tex
    }

    /// Land a worker completion for a master thumbnail (#92) — called
    /// from `App`'s pool drain.
    pub(crate) fn on_thumb_done(
        &mut self,
        ctx: &egui::Context,
        serial: u64,
        master_id: i64,
        img: anyhow::Result<egui::ColorImage>,
    ) {
        // Land only completions that are still expected: eviction
        // (document switch, stencil close) clears the inflight mark, so
        // a late worker result from an OLD source must not insert
        // stale art under a colliding master id (#85).
        if self.inflight.remove(&(serial, master_id)) {
            let _ = self.insert(ctx, (serial, master_id), img);
        }
    }

    /// Drop every cached texture and in-flight mark for one source
    /// serial (#85): the document entry on master-set change, an open
    /// stencil on close.
    pub(crate) fn evict(&mut self, serial: u64) {
        self.textures.retain(|(s, _, _)| *s != serial);
        self.inflight.retain(|(s, _)| *s != serial);
    }

    /// kittest (#92): how many master thumbnails have landed.
    #[doc(hidden)]
    pub fn thumb_count_for_test(&self) -> usize {
        self.textures.len()
    }
}

/// The document-masters entry's cells (search-filtered, id order).
fn doc_cells<'a>(ed: &'a Editor, q: &str) -> Vec<Cell<'a>> {
    let mut ordered: Vec<&vsdx::Master> =
        ed.doc.masters.iter().filter(|m| matches(m, q)).collect();
    ordered.sort_by_key(|m| m.id);
    ordered
        .into_iter()
        .map(|m| Cell {
            key: (StencilId::Document, m.id),
            serial: DOC_SERIAL,
            src: &ed.doc,
            master: m,
        })
        .collect()
}

/// One open stencil's cells, rendered from its own document (#85).
fn stencil_cells<'a>(set: &'a StencilSet, ix: usize, q: &str) -> Vec<Cell<'a>> {
    let st = match set.get(ix) {
        Some(st) => st,
        None => return Vec::new(),
    };
    let mut ordered: Vec<&vsdx::Master> = st.doc.masters.iter().filter(|m| matches(m, q)).collect();
    ordered.sort_by_key(|m| m.id);
    ordered
        .into_iter()
        .map(|m| Cell {
            key: (StencilId::Open(ix), m.id),
            serial: st.serial(),
            src: &st.doc,
            master: m,
        })
        .collect()
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
