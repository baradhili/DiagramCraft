//! kittest coverage for GitHub #263: the v0.2.0 Windows build exited
//! immediately when launched with a `.vssx` argument — the startup
//! open treated every argument as a document, and the stencil's empty
//! page list panicked the first frame at `pages[0]` (on Windows that
//! was an instant silent exit, no window, no error). The `.vssx`
//! dispatch (#83) now lands the stencil in the Shapes pane, and the
//! open-completion guard refuses a pages-empty DOCUMENT with a status
//! note instead of letting it reach that same frame panic.

mod common;

use dc_ui::app::App;
use egui_kittest::Harness;

fn harness_with(app: App) -> Harness<'static, App> {
    dc_ui::svgimg::pin_determinism_for_tests();
    Harness::builder()
        .with_size(egui::vec2(1280.0, 800.0))
        .build_ui_state(
            |ui, app: &mut App| {
                dc_ui::theme::install(ui.ctx());
                app.draw(ui);
            },
            app,
        )
}

/// The CLI entry (`App::new`'s only job beyond shell state is
/// `load(path)`): a `.vssx` argument must land in the Shapes pane —
/// the editor stays untouched (#83) — and the app must frame.
#[test]
fn cli_vssx_argument_lands_in_the_shapes_pane_and_the_app_frames() {
    let dir = tempfile::tempdir().unwrap();
    let path = common::no_pages_package(dir.path(), "custom.vssx");

    let mut app = App::with_doc_opt(None);
    app.debug_load_for_test(path);
    assert!(!app.debug_doc_open(), "a stencil never opens the editor");
    assert_eq!(
        app.debug_stencil_entries().len(),
        1,
        "the stencil landed in the Shapes pane"
    );
    assert!(
        app.debug_status().contains("opened stencil custom"),
        "got: {}",
        app.debug_status()
    );

    // The shell comes up and renders the pane with the stencil in it.
    let mut h = harness_with(app);
    h.run();
    assert!(h.state().debug_stencil_entries().len() == 1);
    assert!(!h.state().debug_doc_open());
}

/// The same package offered as a DOCUMENT (a `.vsdx`/`.vstx`
/// argument, or a Recent-list pick): the open is refused with a
/// friendly status note — no editor swap, no first-frame `pages[0]`
/// panic, the app stays up.
#[test]
fn pages_empty_document_open_is_a_friendly_error_not_a_crash() {
    let dir = tempfile::tempdir().unwrap();
    let path = common::no_pages_package(dir.path(), "broken.vsdx");

    let app = App::with_doc(&path);
    assert!(
        !app.debug_doc_open(),
        "a pages-empty document must not enter the editor"
    );
    assert!(
        app.debug_status().contains("no pages"),
        "got: {}",
        app.debug_status()
    );

    let mut h = harness_with(app);
    h.run();
    assert!(!h.state().debug_doc_open(), "the app stays up");
}
