//! The backstage Export page (#35): SVG / PNG / PDF file export, the
//! GUI half of `dc::export`. The pane keeps its own state (format,
//! page scope, transparency, PNG density, output path) and runs the
//! export on a worker thread — a 300 dpi PNG or a font-embedding PDF
//! must not park the UI thread, so the result lands through a channel
//! the App polls (the same pattern as the rfd pick and the stencil
//! extraction).
//!
//! app.rs keeps only the glue: a field, a `poll_export` call in the
//! frame, and the Browse dialog hand-off — per the owner's 2026-10-26
//! decomposition direction.

use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver};

use dc::export::{self, Format, Options};
use egui::{Ui, Vec2};

/// What the pane asks the App to do after a frame.
#[derive(Debug, Clone, PartialEq)]
pub enum PaneAction {
    None,
    /// Open the native save dialog for this format (Browse…).
    Browse(Format),
    /// Start the export with the current pane state.
    Export,
}

/// The worker's answer: files written + per-page notes, or failure.
type ExportWorkerRx = Receiver<Result<(usize, Vec<String>), String>>;

/// The backstage Export page's state + UI.
pub struct ExportPane {
    format: Format,
    /// True = every page; False = the page the canvas shows.
    all_pages: bool,
    transparent: bool,
    /// PNG density as typed (parsed on export; empty/invalid = 96).
    dpi: String,
    path: String,
    /// The last completed export or failure, shown under the button.
    note: Option<String>,
    /// The running export's result channel (one at a time).
    in_flight: Option<ExportWorkerRx>,
}

impl Default for ExportPane {
    fn default() -> Self {
        Self {
            format: Format::Svg,
            all_pages: true,
            transparent: false,
            dpi: "96".into(),
            path: String::new(),
            note: None,
            in_flight: None,
        }
    }
}

impl ExportPane {
    /// Reset the pane for a document: the output path defaults beside
    /// the drawing (`drawing.svg`), per Visio's export behavior.
    pub fn reset_for(&mut self, doc_path: &std::path::Path) {
        self.path = default_export_path(doc_path, self.format)
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        self.note = None;
        self.in_flight = None;
    }

    pub fn format(&self) -> Format {
        self.format
    }

    /// The armed output path (kittest observability + the dialog
    /// result).
    pub fn path(&self) -> &str {
        &self.path
    }

    /// The last note (kittest observability).
    #[doc(hidden)]
    pub fn note_for_test(&self) -> Option<&str> {
        self.note.as_deref()
    }

    /// The preset dialog answered: adopt the picked path.
    pub fn set_path(&mut self, path: PathBuf) {
        if let Some(f) = path
            .extension()
            .and_then(|e| e.to_str())
            .and_then(Format::from_extension)
        {
            self.format = f;
        }
        self.path = path.to_string_lossy().into_owned();
    }

    /// One frame of the Export page. `page_count` drives the scope
    /// label; the App performs the returned action.
    pub fn show(&mut self, ui: &mut Ui, page_count: usize) -> PaneAction {
        let mut action = PaneAction::None;
        // #257 item 3: the backstage pane mirrors with the chrome —
        // the body right-aligns under RTL. The LTR form keeps the
        // pane's own default layout (plain top_down), byte-identical
        // to the pre-#257 measure.
        if crate::i18n::rtl() {
            ui.with_layout(egui::Layout::top_down(egui::Align::RIGHT), |ui| {
                self.show_body(ui, page_count, &mut action)
            });
        } else {
            self.show_body(ui, page_count, &mut action);
        }
        action
    }

    /// The pane's body — [`Self::show`]'s layout half, direct on the
    /// pane ui (LTR) or wrapped in the RTL right-aligning layout.
    fn show_body(
        &mut self,
        ui: &mut Ui,
        page_count: usize,
        action: &mut PaneAction,
    ) {
        ui.add_space(4.0);
        ui.strong(crate::i18n::strs().msg_select_a_format());
        ui.add_space(4.0);
        ui.vertical(|ui| {
            for (f, label, hint) in [
                (
                    Format::Svg,
                    "SVG",
                    crate::i18n::strs().msg_export_svg_hint(),
                ),
                (
                    Format::Png,
                    "PNG",
                    crate::i18n::strs().msg_export_png_hint(),
                ),
                (
                    Format::Pdf,
                    "PDF",
                    crate::i18n::strs().msg_export_pdf_hint(),
                ),
            ] {
                let selected = self.format == f;
                let label_ref: &str = label;
                if ui.radio(selected, label_ref).on_hover_text(&hint).clicked() && !selected {
                    self.format = f;
                    // The suggested path follows the format (#218
                    // review): exporting PNG bytes into a .svg name
                    // is the common flow otherwise. Only a path with
                    // a KNOWN export extension retargets — a custom
                    // suffix is the user's own and stays.
                    if path_buf_ext(&self.path).is_some() {
                        let p = std::path::PathBuf::from(self.path.trim())
                            .with_extension(f.extensions()[0]);
                        self.path = p.to_string_lossy().into_owned();
                    }
                }
                ui.weak(&hint);
            }
        });
        ui.add_space(8.0);

        ui.strong(crate::i18n::strs().msg_insert_pages());
        ui.add_space(4.0);
        // #257: under RTL the entries are emitted reversed —
        // ui::horizontal keeps the wrap-aware measure (a with_layout
        // flip here poisons the vertical cursor and voids the pane).
        let pages: Vec<(bool, String)> = if crate::i18n::rtl() {
            let mut v = vec![(false, crate::i18n::strs().msg_current_page())];
            if page_count > 1 {
                v.push((true, crate::i18n::strs().msg_all_pages_page_count(page_count)));
            }
            v.into_iter().rev().collect()
        } else {
            let mut v = vec![(false, crate::i18n::strs().msg_current_page())];
            if page_count > 1 {
                v.push((true, crate::i18n::strs().msg_all_pages_page_count(page_count)));
            }
            v
        };
        ui.horizontal(|ui| {
            // A single-page document has only "Current page" — show it
            // selected rather than an odd unselected radio.
            let current_only = page_count <= 1 || !self.all_pages;
            let all_only = page_count > 1 && self.all_pages;
            let mut current_clicked = false;
            let mut all_clicked = false;
            for (is_all, label) in &pages {
                if *is_all {
                    all_clicked = ui.radio(all_only, label.as_str()).clicked();
                } else {
                    current_clicked = ui.radio(current_only, label.as_str()).clicked();
                }
            }
            if current_clicked {
                self.all_pages = false;
            }
            if all_clicked {
                self.all_pages = true;
            }
        });
        if self.format != Format::Pdf {
            ui.add_space(8.0);
            ui.strong(crate::i18n::strs().msg_options());
            ui.add_space(4.0);
            ui.checkbox(&mut self.transparent, crate::i18n::strs().msg_transparent_background());
            if self.format == Format::Png {
                // #257: the label/field pair mirrors by emitting in the
                // reading order (horizontal keeps the wrap measure).
                let dpi_label = crate::i18n::strs().msg_resolution_dpi();
                ui.horizontal(|ui| {
                    if crate::i18n::rtl() {
                        ui.add(
                            egui::TextEdit::singleline(&mut self.dpi)
                                .hint_text("96")
                                .desired_width(60.0),
                        );
                        ui.label(dpi_label);
                    } else {
                        ui.label(dpi_label);
                        ui.add(
                            egui::TextEdit::singleline(&mut self.dpi)
                                .hint_text("96")
                                .desired_width(60.0),
                        );
                    }
                });
            }
        }
        ui.add_space(10.0);

        ui.strong(crate::i18n::strs().msg_export_save_as_label());
        ui.add_space(4.0);
        // #257: mirrored emission under RTL (horizontal, not
        // with_layout — see the pages row).
        let mut browse_clicked = false;
        ui.horizontal(|ui| {
            let mut path_edit = |ui: &mut Ui| {
                let e = ui.add(
                    egui::TextEdit::singleline(&mut self.path)
                        .hint_text(crate::i18n::strs().msg_output_file())
                        .desired_width(360.0),
                );
                if e.changed() {
                    // A typed extension re-targets the format so the
                    // dialog filter and the writer agree.
                    if let Some(f) = path_buf_ext(&self.path) {
                        self.format = f;
                    }
                }
            };
            if crate::i18n::rtl() {
                if ui.button(crate::i18n::strs().msg_browse()).clicked() {
                    browse_clicked = true;
                }
                path_edit(ui);
            } else {
                path_edit(ui);
                browse_clicked = ui.button(crate::i18n::strs().msg_browse()).clicked();
            }
        });
        if browse_clicked {
            *action = PaneAction::Browse(self.format);
        }
        ui.add_space(10.0);

        let busy = self.in_flight.is_some();
        let enabled = !self.path.trim().is_empty() && !busy;
        let export = ui.add_enabled(
            enabled,
            egui::Button::new(crate::i18n::strs().msg_export_start()).min_size(Vec2::new(90.0, 26.0)),
        );
        if busy {
            ui.weak(crate::i18n::strs().msg_exporting());
        } else if let Some(note) = &self.note {
            ui.weak(note);
        } else if self.path.trim().is_empty() {
            ui.weak(crate::i18n::strs().msg_pick_an_output_file_first());
        }
        if export.clicked() && enabled {
            *action = PaneAction::Export;
        }
    }

    /// Spawn the export worker (App-side: clones the document as the
    /// snapshot the export renders).
    pub fn launch(&mut self, doc: &vsdx::Document, current_page: usize, doc_dir: Option<PathBuf>) {
        if self.in_flight.is_some() {
            return; // one export at a time
        }
        let path = PathBuf::from(self.path.trim());
        if path.as_os_str().is_empty() {
            return;
        }
        let format = self.format;
        // #218 review: PDF has no alpha — a stale hidden-toggle value
        // must not strip its white page fill (png/svg keep it).
        let transparent = self.transparent && self.format != Format::Pdf;
        // Finite and bounded (#218 review): "inf" parses, and an
        // unbounded density asks gigabytes of RGBA from the raster.
        let dpi = self
            .dpi
            .trim()
            .parse::<f32>()
            .ok()
            .filter(|v| v.is_finite())
            .map(|v| v.clamp(16.0, 1200.0))
            .unwrap_or(96.0);
        let pages: Vec<usize> = if self.all_pages {
            Vec::new()
        } else {
            vec![current_page.min(doc.pages.len().saturating_sub(1))]
        };
        // A relative path resolves against the document's folder —
        // exports belong beside the drawing, not beside the cwd.
        let path = match &doc_dir {
            Some(dir) if path.is_relative() => dir.join(path),
            _ => path,
        };
        let opts = Options {
            transparent,
            png_dpi: dpi,
        };
        let doc = doc.clone();
        let (tx, rx) = channel();
        self.in_flight = Some(rx);
        self.note = None;
        std::thread::spawn(move || {
            let result = export::write_document(&doc, &path, &pages, format, &opts)
                .map(|(files, notes)| (files.len(), notes))
                .map_err(|e| format!("{e:#}"));
            let _ = tx.send(result);
        });
    }

    /// The pane's last note (the export result message).
    pub fn note_clone(&self) -> Option<String> {
        self.note.clone()
    }

    /// Drain the worker: `Some(result)` once it finishes (pages
    /// written + any per-page notes, or the failure).
    pub fn poll(&mut self) -> Option<Result<(usize, Vec<String>), String>> {
        let rx = self.in_flight.as_mut()?;
        match rx.try_recv() {
            Ok(r) => {
                self.in_flight = None;
                self.note = Some(match &r {
                    Ok((n, notes)) => {
                        let mut msg = crate::i18n::strs().msg_exported_files(*n);
                        // #218 review: a density the memory ceiling
                        // reduced is REPORTED, never silently applied.
                        for note in notes {
                            msg.push(' ');
                            msg.push_str(note);
                        }
                        msg
                    }
                    Err(e) => crate::i18n::strs().msg_export_failed_e(e),
                });
                Some(r)
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => None,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                self.in_flight = None;
                self.note = Some(crate::i18n::strs().msg_export_failed_worker_lost());
                Some(Err(crate::i18n::strs().msg_worker_lost()))
            }
        }
    }
}

/// The output path a fresh pane suggests for a document: beside the
/// drawing with the format's extension.
fn default_export_path(doc_path: &std::path::Path, format: Format) -> Option<PathBuf> {
    let stem = doc_path.file_stem()?;
    let mut name = stem.to_os_string();
    name.push(".");
    name.push(format.extensions()[0]);
    Some(doc_path.with_file_name(name))
}

fn path_buf_ext(path: &str) -> Option<Format> {
    PathBuf::from(path)
        .extension()
        .and_then(|e| e.to_str())
        .and_then(Format::from_extension)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reset_for_suggests_a_path_beside_the_drawing() {
        let mut pane = ExportPane::default();
        // Compare as paths — `with_file_name` joins with the platform
        // separator, so the string form differs on Windows (#218 CI).
        pane.reset_for(std::path::Path::new("/docs/plan.vsdx"));
        let docs = std::path::Path::new("/docs");
        assert_eq!(std::path::Path::new(pane.path()), docs.join("plan.svg"));
        pane.format = Format::Pdf;
        pane.reset_for(std::path::Path::new("/docs/plan.vsdx"));
        assert_eq!(std::path::Path::new(pane.path()), docs.join("plan.pdf"));
    }

    #[test]
    fn set_path_adopts_the_picked_extension() {
        let mut pane = ExportPane::default();
        pane.set_path(PathBuf::from("/docs/plan.pdf"));
        assert_eq!(pane.format(), Format::Pdf);
        assert_eq!(pane.path(), "/docs/plan.pdf");
    }

    #[test]
    fn typed_extension_retargets_the_format() {
        assert_eq!(path_buf_ext("/x/out.png"), Some(Format::Png));
        assert_eq!(path_buf_ext("/x/out.txt"), None);
    }
}
