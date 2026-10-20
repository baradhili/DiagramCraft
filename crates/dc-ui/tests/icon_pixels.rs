//! Pixel-level check that ribbon icons actually rasterize (regression
//! guard for the "icons missing from the ribbon" bug).

trait PixelImage {
    fn dims(&self) -> (u32, u32);
    fn px(&self, x: u32, y: u32) -> [u8; 4];
}
impl PixelImage for image::RgbaImage {
    fn dims(&self) -> (u32, u32) {
        (self.width(), self.height())
    }
    fn px(&self, x: u32, y: u32) -> [u8; 4] {
        self.get_pixel(x, y).0
    }
}

use dc_ui::ribbon::{big, content_row, Action, Group, Icon, RibbonButton};
use egui_kittest::Harness;

fn ink(img: &impl PixelImage) -> usize {
    let (w, h) = img.dims();
    (0..w)
        .flat_map(|x| (0..h).map(move |y| (x, y)))
        .filter(|(x, y)| {
            let p = img.px(*x, *y);
            p[0] < 235 || p[1] < 235 || p[2] < 235
        })
        .count()
}

#[test]
fn icon_paints_pixels_standalone() {
    let mut h = Harness::builder()
        .with_size(egui::vec2(200.0, 120.0))
        .build_ui(|ui| {
            Icon::Paste.paint(
                ui,
                egui::Rect::from_min_size(egui::pos2(10.0, 10.0), egui::vec2(80.0, 80.0)),
                true,
            );
        });
    h.run();
    let img = h.render().unwrap();
    let n = ink(&img);
    assert!(n > 300, "standalone Paste icon painted only {n} px");
}

#[test]
fn icon_paints_pixels_inside_content_row() {
    let groups = vec![Group::new(
        "Clipboard",
        vec![big(RibbonButton::new(Icon::Paste, "Paste", Action::Macros).enabled(false))],
    )];
    let mut h = Harness::builder()
        .with_size(egui::vec2(400.0, 120.0))
        .build_ui(|ui| {
            content_row(ui, &groups, &mut |_a| {}, &mut |_| {});
        });
    h.run();
    let img = h.render().unwrap();
    let n = ink(&img);
    // the galley text alone is ~250-400 px; a visible (grayed) icon adds
    // far more than that
    assert!(n > 800, "content-row Paste painted only {n} px — icon missing");
}

#[test]
fn icon_paints_with_theme_and_panel() {
    // Same structure as the shell: theme installed, panel with frame.
    let groups = vec![Group::new(
        "Clipboard",
        vec![big(RibbonButton::new(Icon::Paste, "Paste", Action::Macros).enabled(false))],
    )];
    let mut h = Harness::builder()
        .with_size(egui::vec2(400.0, 120.0))
        .build_ui(|ui| {
            dc_ui::theme::install(ui.ctx());
            egui::Panel::top("ribbon_content")
                .frame(
                    egui::Frame::new()
                        .fill(dc_ui::theme::RIBBON_BG)
                        .inner_margin(egui::Margin::symmetric(4, 3)),
                )
                .show(ui, |ui| content_row(ui, &groups, &mut |_a| {}, &mut |_| {}));
        });
    h.run();
    let img = h.render().unwrap();
    let n = ink(&img);
    assert!(n > 800, "theme+panel Paste painted only {n} px — icon missing");
}

#[test]
fn icon_paints_in_full_shell() {
    // The whole app over the synthetic fixture, Home tab.
    let mut app = dc_ui::app::App::with_blank();
    let mut h = egui_kittest::Harness::builder()
        .with_size(egui::vec2(1280.0, 800.0))
        .build_ui(move |ui| {
            dc_ui::theme::install(ui.ctx());
            app.draw(ui);
        });
    h.run();
    let img = h.render().unwrap();
    // scan the ribbon band (y 90..160) for icon ink left of the stencil pane
    let (w, _h) = img.dims();
    let mut n = 0;
    for x in 0..w.min(600) {
        for y in 90..160.min(_h) {
            let p = img.px(x, y);
            if p[0] < 235 || p[1] < 235 || p[2] < 235 {
                n += 1;
            }
        }
    }
    assert!(n > 2000, "full-shell ribbon band has only {n} ink px — icons missing");
}

#[test]
#[ignore]
fn dump_shell_render() {
    let mut app = dc_ui::app::App::with_blank();
    let mut h = egui_kittest::Harness::builder()
        .with_size(egui::vec2(1280.0, 800.0))
        .build_ui(move |ui| {
            dc_ui::theme::install(ui.ctx());
            app.draw(ui);
        });
    h.run();
    h.run();
    let img = h.render().unwrap();
    img.save("/tmp/shell_render.png").unwrap();
}
