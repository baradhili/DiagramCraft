//! Launch the Tarahi desktop app: `dc-ui [file.vsdx|vstx|vssx]`.

fn main() -> eframe::Result<()> {
    // The window class is "Tarahi" to match the desktop entry's
    // StartupWMClass (#261 naming alignment). Wayland's toplevel
    // app_id is the class; eframe also wires the same app_id into
    // X11's WM_CLASS class part (verified live: WM_CLASS = ("",
    // "Tarahi") — winit's binary-basename class never applies), so
    // both backends group under Tarahi. Title stays the product
    // name. Note: the app_id also names eframe's persistence dir,
    // so window-state state moves from the diagramcraft dir once.
    let path = std::env::args().nth(1).map(std::path::PathBuf::from);
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1360.0, 860.0])
            .with_min_inner_size([980.0, 640.0])
            .with_title(dc_ui::i18n::strs().msg_about_name())
            // Title stays the product name.
            .with_app_id("Tarahi"),
        ..Default::default()
    };
    eframe::run_native(
        &dc_ui::i18n::strs().msg_about_name(),
        options,
        Box::new(|cc| Ok(Box::new(dc_ui::app::App::new(cc, path)))),
    )
}
