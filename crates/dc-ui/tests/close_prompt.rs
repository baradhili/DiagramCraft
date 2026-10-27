//! kittest coverage for the unsaved-changes close prompt (GitHub #65):
//! a window close (via the test hook — kittest cannot synthesize a
//! WM_CLOSE) and File → Close must ask before a dirty document is
//! lost; a clean document closes silently.

use dc_ui::app::App;
use egui_kittest::kittest::Queryable;
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

/// A clean, titled document on disk, like opening a saved file. The
/// TempDir outlives the harness (returned), keeping the path valid.
fn titled_doc() -> (tempfile::TempDir, App) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("drawing.vsdx");
    dc_core::Editor::new_blank().save_as(&path).unwrap();
    (dir, App::with_doc(&path))
}

/// Click one of the prompt's buttons, scoped to the prompt window's own
/// rect (#97 section 4: was a hardcoded y-band that one ribbon
/// relayout would break across all nine callers — the ribbon/QAT
/// "Save" lives outside the window and is excluded by containment).
/// Click one of the prompt's buttons, scoped to the prompt itself: the
/// buttons sit in a tight band below the prompt title's node, within
/// its horizontal span (#97 section 4 — was an absolute y>250 screen
/// band that one shell relayout would break across all nine callers;
/// the ribbon/QAT "Save" is nowhere near the title and never matches).
fn click_prompt_button(h: &mut Harness<'_, App>, label: &str) {
    let title = h.get_by_label_contains("Save changes to").rect();
    let node = h
        .get_all_by_label(label)
        .into_iter()
        .find(|n| {
            let r = n.rect();
            r.min.y >= title.max.y
                && r.min.y < title.max.y + 80.0
                && r.min.x >= title.min.x - 60.0
                && r.max.x <= title.max.x + 60.0
        })
        .unwrap_or_else(|| panic!("no {label} button in the close prompt"));
    node.click();
    h.run();
}

#[test]
fn dirty_window_close_prompts_and_cancel_keeps_doc() {
    let (_dir, app) = titled_doc();
    let mut h = harness_with(app);
    h.run();
    h.state_mut().mark_dirty_for_test();
    h.run();

    // The ✕ on a dirty document: vetoed, and the prompt comes up.
    h.state_mut().request_close_for_test();
    h.run();
    assert!(h.state().debug_close_prompt());
    assert_eq!(h.state().debug_close_vetoes(), 1);
    assert!(h.query_by_label_contains("Save changes to").is_some());

    // Clicking ✕ again changes nothing (one prompt, not two).
    h.state_mut().request_close_for_test();
    h.run();
    assert!(h.state().debug_close_prompt());
    assert_eq!(h.state().debug_close_vetoes(), 2);
    assert_eq!(h.state().debug_close_issued(), 0);

    // Cancel keeps the document exactly as it was.
    click_prompt_button(&mut h, "Cancel");
    assert!(!h.state().debug_close_prompt());
    assert!(h.state().debug_doc_open());
    assert_eq!(h.state().debug_close_issued(), 0);

    // A later ✕ prompts again — Cancel armed nothing.
    h.state_mut().request_close_for_test();
    h.run();
    assert!(h.state().debug_close_prompt());
}

#[test]
fn dont_save_closes_and_the_close_echo_stays_quiet() {
    // Regression (live report 2026-10-23): egui-winit delivers
    // ViewportCommand::Close as the NEXT frame's close-requested
    // input; with the document still dirty (Don't Save keeps it),
    // the echo used to be re-vetoed into a fresh prompt — "Don't
    // Save does nothing". The approved close must pass through.
    let (_dir, app) = titled_doc();
    let mut h = harness_with(app);
    h.run();
    h.state_mut().mark_dirty_for_test();
    h.run();
    h.state_mut().request_close_for_test();
    h.run();
    assert!(h.state().debug_close_prompt());

    click_prompt_button(&mut h, "Don't Save");
    assert_eq!(h.state().debug_close_issued(), 1);
    assert!(!h.state().debug_close_prompt());

    // The echo: no new prompt, no new veto; eframe owns the exit.
    h.state_mut().request_close_for_test();
    h.run();
    assert!(!h.state().debug_close_prompt(), "the echo re-prompted");
    assert_eq!(h.state().debug_close_vetoes(), 1);
    assert_eq!(h.state().debug_close_issued(), 1);
    assert!(h.state().debug_doc_open(), "eframe owns the actual exit");
}

#[test]
fn save_titled_document_closes_after_saving() {
    let (_dir, app) = titled_doc();
    let mut h = harness_with(app);
    h.run();
    h.state_mut().mark_dirty_for_test();
    h.run();
    h.state_mut().request_close_for_test();
    h.run();

    click_prompt_button(&mut h, "Save");
    assert!(!h.state().debug_close_prompt());
    assert!(!h.state().debug_dirty(), "the save cleared the flag");
    assert!(h.state().debug_status().contains("saved"));
    assert_eq!(h.state().debug_close_issued(), 1);
}

#[test]
fn close_request_while_save_as_is_pending_is_vetoed_without_a_new_prompt() {
    // Regression (CodeRabbit on 76660e2): while the untitled close
    // prompt's Save has the native save-as dialog open, a second ✕
    // must not open another prompt — its Cancel would contradict the
    // pending chain, and its Don't Save would race the dialog's save.
    // kittest cannot hold the native dialog open deterministically,
    // so the chain is armed directly.
    let (_dir, app) = titled_doc();
    let mut h = harness_with(app);
    h.run();
    h.state_mut().mark_dirty_for_test();
    h.state_mut().arm_close_chain_for_test();
    h.run();
    h.state_mut().request_close_for_test();
    h.run();
    assert!(
        !h.state().debug_close_prompt(),
        "no second prompt while the dialog is pending"
    );
    assert_eq!(h.state().debug_close_vetoes(), 1);
    assert_eq!(h.state().debug_close_issued(), 0);
}

#[test]
fn untitled_save_routes_through_the_dialog_then_closes() {
    let dir = tempfile::tempdir().unwrap();
    let pick = dir.path().join("chosen.vsdx");
    let mut h = harness_with(App::with_blank());
    h.run();
    // An untitled document's Save needs a file first: the preset
    // answers the (headless-invisible) native dialog.
    h.state_mut().mark_dirty_for_test();
    h.state_mut().preset_save_as_for_test(Some(pick.clone()));
    h.run();
    h.state_mut().request_close_for_test();
    h.run();

    click_prompt_button(&mut h, "Save");
    // The prompt is gone; the close waits on the dialog. `run()`
    // settles over multiple frames, so the dialog answer (the preset
    // is instant) may already have landed — the resolved end-state
    // below is what matters, not the transit.
    assert!(!h.state().debug_close_prompt());

    // The dialog answer arrives on its worker thread; step frames
    // until the chain resolves.
    for _ in 0..20 {
        h.run();
        if h.state().debug_close_issued() == 1 {
            break;
        }
    }
    assert_eq!(h.state().debug_close_issued(), 1, "close after the save");
    assert!(!h.state().debug_close_chain_armed());
    assert!(!h.state().debug_dirty());
    assert!(pick.exists(), "the drawing was written");
}

#[test]
fn untitled_save_cancelled_dialog_keeps_the_app_open() {
    let mut h = harness_with(App::with_blank());
    h.run();
    h.state_mut().mark_dirty_for_test();
    h.state_mut().preset_save_as_for_test(None); // dialog cancelled
    h.run();
    h.state_mut().request_close_for_test();
    h.run();

    click_prompt_button(&mut h, "Save");
    assert!(!h.state().debug_close_prompt());
    for _ in 0..20 {
        h.run();
        if !h.state().debug_close_chain_armed() {
            break;
        }
    }
    assert!(
        !h.state().debug_close_chain_armed(),
        "cancel drops the chain"
    );
    assert_eq!(h.state().debug_close_issued(), 0);
    assert!(h.state().debug_doc_open(), "staying open");
    assert!(h.state().debug_dirty(), "nothing was saved");
}

#[test]
fn clean_window_close_is_silent() {
    let (_dir, app) = titled_doc();
    let mut h = harness_with(app);
    h.run();
    h.state_mut().request_close_for_test();
    h.run();
    assert!(!h.state().debug_close_prompt());
    assert!(h.query_by_label_contains("Save changes to").is_none());
    assert_eq!(h.state().debug_close_issued(), 1);
    assert_eq!(h.state().debug_close_vetoes(), 0);
}

#[test]
fn file_close_prompts_when_dirty_and_closes_the_doc_only() {
    let (_dir, app) = titled_doc();
    let mut h = harness_with(app);
    h.run();
    h.state_mut().mark_dirty_for_test();
    h.run();
    h.state_mut().backstage = true;
    h.run();

    // The backstage rail's Close command (the short row, not the
    // rail container's aggregated label).
    let row = h
        .get_all_by_label("Close")
        .into_iter()
        .find(|n| n.rect().height() < 40.0)
        .expect("Close rail command");
    row.click();
    h.run();

    // Backstage made way for the prompt; the window itself was never
    // asked to close.
    assert!(!h.state().backstage);
    assert!(h.state().debug_close_prompt());
    assert_eq!(h.state().debug_close_vetoes(), 0);

    click_prompt_button(&mut h, "Don't Save");
    assert!(!h.state().debug_doc_open(), "the document closed");
    assert_eq!(h.state().debug_close_issued(), 0, "the app stays open");
    assert!(h.state().debug_status().contains("closed"));
}

#[test]
fn file_close_on_a_clean_document_is_silent() {
    let (_dir, app) = titled_doc();
    let mut h = harness_with(app);
    h.run();
    h.state_mut().backstage = true;
    h.run();
    let row = h
        .get_all_by_label("Close")
        .into_iter()
        .find(|n| n.rect().height() < 40.0)
        .expect("Close rail command");
    row.click();
    h.run();
    assert!(!h.state().debug_close_prompt(), "clean doc, no prompt");
    assert!(!h.state().debug_doc_open());
    assert!(h.state().debug_status().contains("closed"));
}
