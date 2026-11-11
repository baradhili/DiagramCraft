//! #246 i18n: generate the typed Fluent accessors (`src/l10n.rs`,
//! committed) from the project-level `locales/<lang>/*.ftl` catalogs.
//! Argument types are read from the `en` reference locale's type
//! comments; `de` must carry the same key/variable set or this build
//! fails — the same contract `flynt` checks on the catalogs.

fn main() -> std::process::ExitCode {
    // Watch the whole tree: adding a locale folder is itself a
    // change that must regenerate the accessors.
    println!("cargo:rerun-if-changed=../../locales");
    let opts = fluent_typed::BuildOptions::default()
        .with_locales_folder("../../locales")
        .with_default_language("en")
        // Bidi isolation stays OFF for now (#257): egui's bidi run
        // layout renders Persian sentences + interpolated values
        // correctly without the marks, and turning them on reshuffles
        // kittest's synthesized "Open Panel.N" dedupe indices in the
        // group_edit flows. Flip it together with the item-3 mirroring
        // work, where the chrome tests are reworked anyway.
        .without_bidi_isolation();
    // Bidi isolation ON (#257): Persian ships, and interpolated
    // values (file paths, identifiers) keep their run direction
    // inside RTL sentences. The marks are zero-width and render
    // nothing; string-matching tests quote the wrapped form.
    fluent_typed::build_from_locales_folder(opts)
}
