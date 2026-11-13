//! #246 i18n: generate the typed Fluent accessors (`src/l10n.rs`,
//! committed) from the project-level `locales/<lang>/*.ftl` catalogs.
//! Argument types are read from the `en` reference locale's type
//! comments; `de` must carry the same key/variable set or this build
//! fails — the same contract `flynt` checks on the catalogs.

fn main() -> std::process::ExitCode {
    // Watch the whole tree: adding a locale folder is itself a
    // change that must regenerate the accessors.
    println!("cargo:rerun-if-changed=../../locales");
    // Bidi isolation ON (#257 item 3): Persian ships, and interpolated
    // values (file paths, identifiers, page names) keep their run
    // direction inside RTL sentences. The marks are zero-width and
    // render nothing; the group_edit dedupe-index tests that keyed on
    // the unwrapped forms quote the wrapped forms now.
    let opts = fluent_typed::BuildOptions::default()
        .with_locales_folder("../../locales")
        .with_default_language("en");
    fluent_typed::build_from_locales_folder(opts)
}
