//! Render the blank-document shell to a PNG for eyeballing/debugging
//! (assertion-free sibling of the icon_pixels tests). Args: [out.png],
//! defaulting to /tmp/shell_render.png.
fn main() {
    let out = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "/tmp/shell_render.png".to_string());
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
    img.save(&out).unwrap();
    println!("wrote {out}");
}
