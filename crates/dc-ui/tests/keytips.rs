//! #184 Alt keytips (egui_kittest): arming, strip letters, control
//! chains, Esc back-out, Alt+letter combos, QAT digits, and badge
//! goldens over the full shell.

mod common;

use common::shell_fixture as fixture;
use dc_ui::app::App;
use dc_ui::ribbon::Tab;

fn build(path: &std::path::Path) -> egui_kittest::Harness<'_, App> {
    // Goldens must render identically on every machine (same rule as
    // shell.rs).
    dc_ui::svgimg::pin_determinism_for_tests();
    let mut h = egui_kittest::Harness::builder()
        .with_size(egui::vec2(1280.0, 800.0))
        .with_max_steps(16)
        .build_ui_state(
            |ui, app: &mut App| {
                dc_ui::theme::install(ui.ctx());
                app.draw(ui);
            },
            App::with_doc(path),
        );
    h.run();
    h.run(); // second pass: textures and fit settle
    h
}

/// The named `Key` whose `name()` is this single character (letters
/// and digits name themselves).
fn key_for(c: char) -> egui::Key {
    egui::Key::ALL
        .iter()
        .copied()
        .find(|k| k.name() == c.to_string())
        .unwrap_or_else(|| panic!("no egui key named {c}"))
}

fn snap(h: &mut egui_kittest::Harness<App>, name: &str) {
    h.snapshot_options(
        name,
        &egui_kittest::SnapshotOptions::new()
            .threshold(3.0)
            .max_failed_pixels(10_000),
    );
}

/// A bare Alt arms the strip level: tab letters, the File F, Tell Me
/// Q, and the QAT digits badge the shell (golden).
#[test]
fn alt_arms_the_strip_level() {
    let tmp = tempfile::tempdir().unwrap();
    let doc = fixture(tmp.path());
    let mut h = build(&doc);
    assert_eq!(h.state().debug_keytip_mode(), None);
    h.key_press(egui::Key::AltLeft);
    h.run();
    assert_eq!(h.state().debug_keytip_mode(), Some("strip"));
    snap(&mut h, "dc_ui_keytips_strip");
}

/// A strip letter switches its tab and chains into the control
/// level: Alt, H → Home with control badges (golden).
#[test]
fn tab_letter_chains_into_controls() {
    let tmp = tempfile::tempdir().unwrap();
    let doc = fixture(tmp.path());
    let mut h = build(&doc);
    h.key_press(egui::Key::AltLeft);
    h.run();
    h.key_press(egui::Key::H);
    h.run();
    assert_eq!(h.state().debug_tab(), Tab::Home);
    assert_eq!(h.state().debug_keytip_mode(), Some("controls"));
    snap(&mut h, "dc_ui_keytips_controls");
}

/// The full chain fires a control: Alt, W (View), then the assigned
/// Rulers letter toggles the rulers and disarms (Office: an action
/// keytip ends the mode).
#[test]
fn control_letter_fires_the_action_and_disarms() {
    let tmp = tempfile::tempdir().unwrap();
    let doc = fixture(tmp.path());
    let mut h = build(&doc);
    h.key_press(egui::Key::AltLeft);
    h.run();
    h.key_press(egui::Key::W);
    h.run();
    assert_eq!(h.state().debug_tab(), Tab::View);
    let rulers_on = h.state().debug_show_rulers();
    let letter = h
        .state()
        .debug_keytip_letter("Rulers")
        .expect("Rulers carries a keytip letter");
    h.key_press(key_for(letter));
    h.run();
    assert_eq!(h.state().debug_show_rulers(), !rulers_on);
    assert_eq!(h.state().debug_keytip_mode(), None);
}

/// The Alt+letter combo works without pre-arming (Office's
/// accelerator form): Alt+H lands on Home's controls directly.
#[test]
fn alt_letter_combo_works_without_arming() {
    let tmp = tempfile::tempdir().unwrap();
    let doc = fixture(tmp.path());
    let mut h = build(&doc);
    h.key_press_modifiers(egui::Modifiers::ALT, egui::Key::H);
    h.run();
    assert_eq!(h.state().debug_tab(), Tab::Home);
    assert_eq!(h.state().debug_keytip_mode(), Some("controls"));
}

/// Esc backs out one level at a time (controls → strip → disarmed).
#[test]
fn esc_backs_out_one_level_then_disarms() {
    let tmp = tempfile::tempdir().unwrap();
    let doc = fixture(tmp.path());
    let mut h = build(&doc);
    h.key_press(egui::Key::AltLeft);
    h.run();
    h.key_press(egui::Key::H);
    h.run();
    assert_eq!(h.state().debug_keytip_mode(), Some("controls"));
    h.key_press(egui::Key::Escape);
    h.run();
    assert_eq!(h.state().debug_keytip_mode(), Some("strip"));
    h.key_press(egui::Key::Escape);
    h.run();
    assert_eq!(h.state().debug_keytip_mode(), None);
}

/// The File keytip (Alt, F) opens the backstage view and disarms.
#[test]
fn f_opens_the_file_backstage() {
    let tmp = tempfile::tempdir().unwrap();
    let doc = fixture(tmp.path());
    let mut h = build(&doc);
    h.key_press(egui::Key::AltLeft);
    h.run();
    h.key_press(egui::Key::F);
    h.run();
    assert!(h.state().debug_backstage());
    assert_eq!(h.state().debug_keytip_mode(), None);
}

/// A QAT digit fires its action: Alt, 1 saves, turning a dirty
/// document clean.
#[test]
fn qat_digit_fires_save() {
    let tmp = tempfile::tempdir().unwrap();
    let doc = fixture(tmp.path());
    let mut h = build(&doc);
    h.state_mut().mark_dirty_for_test();
    h.key_press(egui::Key::AltLeft);
    h.run();
    h.key_press(egui::Key::Num1);
    h.run();
    assert!(!h.state().debug_dirty());
    assert_eq!(h.state().debug_keytip_mode(), None);
}
