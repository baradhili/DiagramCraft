//! #246 i18n: generate the typed Fluent accessors (`src/l10n.rs`,
//! committed) from the project-level `locales/<lang>/*.ftl` catalogs.
//! Argument types are read from the `en` reference locale's type
//! comments; `de` must carry the same key/variable set or this build
//! fails — the same contract `flynt` checks on the catalogs.

fn main() -> std::process::ExitCode {
    println!("cargo:rerun-if-changed=../../locales/en/app.ftl");
    println!("cargo:rerun-if-changed=../../locales/de/app.ftl");
    let opts = fluent_typed::BuildOptions::default()
        .with_locales_folder("../../locales")
        .with_default_language("en")
        // Bidi isolation marks (U+2068/U+2069 around interpolated
        // values) are OFF until RTL ships (#257): no shipped locale is
        // bidi, and the invisible wrappers would leak into status
        // assertions, logs and clipboard copies. #257 re-enables this
        // with the mirroring work.
        .without_bidi_isolation();
    fluent_typed::build_from_locales_folder(opts)
}
