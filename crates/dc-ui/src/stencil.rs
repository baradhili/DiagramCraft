//! The Shapes stencil pane: every master in the document as a
//! thumbnail (rendered through the same SVG backend), selectable and
//! droppable onto the canvas.

use dc::svg;
use dc_core::Editor;
use egui::{Color32, Pos2, Rect, Sense, Ui, Vec2};

use crate::svgimg;
use crate::theme;

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
    textures: Vec<(i64, egui::TextureHandle)>,
    loaded_for: Option<u64>, // doc identity (master count + first ids hash)
}

impl Stencil {
    pub fn show(&mut self, ui: &mut Ui, ed: &Editor) -> Option<i64> {
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
        // affordance, then the search box, then the thumbnail grid.
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
        ui.add_space(2.0);
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.set_min_width(ui.available_width());
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
                .num_columns(2)
                .spacing([6.0, 6.0])
                .min_col_width(64.0)
                .show(ui, |ui| {
                    self.cell_rects.clear();
                    for m in ordered {
                        let tex = self.texture(ui.ctx(), ed, m.id);
                        let armed = self.armed == Some(m.id);
                        // one grid cell: 64×78 (thumb + caption strip)
                        let size = Vec2::new(64.0, 78.0);
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
                        let thumb = Rect::from_min_size(r.left_top(), Vec2::new(64.0, 64.0));
                        p.rect_filled(thumb, 2.0, t.stencil_bg);
                        p.image(
                            tex.id(),
                            thumb.shrink(2.0),
                            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                            Color32::WHITE,
                        );
                        let border = if armed {
                            egui::Stroke::new(2.0, t.accent)
                        } else if resp.hovered() {
                            egui::Stroke::new(1.0, t.selection)
                        } else {
                            egui::Stroke::new(1.0, t.ribbon_group)
                        };
                        p.rect_stroke(r, 2.0, border, egui::StrokeKind::Middle);
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
                        ui.end_row();
                    }
                });
        });
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

fn short(s: &str) -> String {
    const MAX: usize = 10;
    if s.chars().count() <= MAX {
        s.to_string()
    } else {
        let t: String = s.chars().take(MAX - 1).collect();
        format!("{t}…")
    }
}
