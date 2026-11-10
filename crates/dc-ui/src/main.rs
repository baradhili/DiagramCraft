//! Launch the Tarahi desktop app: `dc-ui [file.vsdx|vstx|vssx]`.

fn main() -> eframe::Result<()> {
    let path = std::env::args().nth(1).map(std::path::PathBuf::from);
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1360.0, 860.0])
            .with_min_inner_size([980.0, 640.0])
            .with_title("Tarahi")
            // The desktop entry's StartupWMClass (#218 review):
            // without an explicit app id, Wayland toplevels get an
            // auto-generated one and the entry cannot group its
            // windows. Title stays the product name.
            .with_app_id("diagramcraft"),
        ..Default::default()
    };
    eframe::run_native(
        "Tarahi",
        options,
        Box::new(|cc| Ok(Box::new(dc_ui::app::App::new(cc, path)))),
    )
}
