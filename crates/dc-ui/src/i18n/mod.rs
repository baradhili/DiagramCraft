//! #246 i18n — the shell's language runtime over the fluent-typed
//! generated accessors ([`crate::l10n`]).
//!
//! Layout: the catalogs live at PROJECT level (`locales/<lang>/app.ftl`
//! at the workspace root — a new language never digs through the code);
//! `build.rs` generates the typed accessors into `src/l10n.rs`, which is
//! committed, and joins the translations into `gen/translations.ftl`
//! which the accessors embed. flynt lints the catalogs (`.flynt.toml`);
//! `crates/dc-ui/tests/i18n.rs` is the usage gate (completeness, orphans,
//! literal scan) plus the language-switch kittest coverage.
//!
//! Call sites never hold user-facing English: they read message data off
//! the current language's generated struct via [`strs()`] — e.g.
//! `i18n::strs().msg_align_center()` — and pass raw numbers/strings as
//! typed arguments. Plurals, punctuation and word order belong in the
//! `.ftl` files, never in Rust.
//!
//! The language is process-global: the production constructor calls
//! [`init_from_disk()`] (persisted choice in `language.json`, else the
//! system locale, else English); the headless kittest constructors pin
//! English via [`pin_english()`] so goldens never move. Switching calls
//! [`set_language`] — the next frame re-renders in the new language; no
//! stored widget strings mutate (localization happens at the render
//! boundary).

use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU8, Ordering};

pub use crate::l10n::{L10n, L10nLanguage};
use crate::recent::config_base;

const SCHEMA_VERSION: u64 = 1;

/// Slot index into [`L10n::iter`] — the process-global current language.
/// `u8::MAX` = unset, so `current()` falls through to `L10n::default()`
/// (English): `L10n::iter()` starts at German, and a bare `0` would
/// make every unpinned reader — unit tests, the pre-init window title —
/// resolve German.
static CURRENT: AtomicU8 = AtomicU8::new(u8::MAX);

/// English pinned (headless constructors, tests): no persistence.
pub fn pin_english() {
    set_current(L10n::En);
}

/// The active language.
pub fn current() -> L10n {
    L10n::iter()
        .nth(CURRENT.load(Ordering::Relaxed) as usize)
        .copied()
        .unwrap_or_default()
}

fn set_current(lang: L10n) {
    let ix = L10n::iter()
        .position(|l| *l == lang)
        .expect("language from L10n::iter() is always present");
    CURRENT.store(ix as u8, Ordering::Relaxed);
}

/// Switch the language and persist the choice app-level (never in the
/// `.vsdx` — DocumentSettings' schema is closed). Applies on the next
/// frame: call sites read [`strs()`] fresh, no stored strings mutate.
pub fn set_language(lang: L10n) {
    set_current(lang);
    persist(lang);
}

/// Switch without persisting (tests, previews).
pub fn set_language_ephemeral(lang: L10n) {
    set_current(lang);
}

/// The RTL language set (#257 item 3): the shell chrome mirrors for
/// these. Arabic/Hebrew catalogs join the set with their folders.
const RTL_LANGS: &[L10n] = &[L10n::Fa];

/// True while the current language reads right-to-left: the shell
/// mirrors its chrome for it (ribbon order, panel sides, strip and
/// status alignment, horizontal-ruler zero direction).
pub fn rtl() -> bool {
    RTL_LANGS.contains(&current())
}

/// The current language's generated accessors, loaded once and leaked
/// per language (bounded by [`L10n::iter`]). Every chrome string in the
/// shell resolves through this.
pub fn strs() -> &'static L10nLanguage {
    static LOADED: Mutex<Vec<Option<&'static L10nLanguage>>> = Mutex::new(Vec::new());
    let lang = current();
    let ix = L10n::iter().position(|l| *l == lang).unwrap_or(0);
    let mut slots = LOADED.lock().unwrap();
    slots.resize_with(L10n::iter().len(), || None);
    if let Some(loaded) = slots[ix] {
        return loaded;
    }
    let loaded: &'static L10nLanguage = Box::leak(Box::new(lang.load()));
    slots[ix] = Some(loaded);
    loaded
}

/// Load the persisted choice (or the system locale on first run) —
/// the production constructor calls this once at startup.
pub fn init_from_disk() {
    let Some(path) = store_path() else {
        set_current(detect_system_locale());
        return;
    };
    let lang = std::fs::read_to_string(&path)
        .ok()
        .and_then(|text| parse(&text))
        .or_else(|| Some(detect_system_locale()));
    if let Some(lang) = lang {
        set_current(lang);
    }
}

fn store_path() -> Option<PathBuf> {
    config_base().map(|b| b.join("diagramcraft").join("language.json"))
}

fn persist(lang: L10n) {
    let Some(path) = store_path() else { return };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(
        &path,
        serde_json::json!({ "version": SCHEMA_VERSION, "language": lang.to_string() }).to_string(),
    );
}

/// Parse the on-disk form; `None` for anything unreadable or carrying a
/// schema version this build does not know.
fn parse(text: &str) -> Option<L10n> {
    let v: serde_json::Value = serde_json::from_str(text).ok()?;
    if v.get("version")?.as_u64()? != SCHEMA_VERSION {
        return None;
    }
    v.get("language")?.as_str()?.parse().ok()
}

/// First-run detection from the process environment (`LC_ALL` /
/// `LC_MESSAGES` / `LANG`, e.g. `de_DE.UTF-8` → `de`); English when no
/// known locale is advertised. Unix-only: other platforms default to
/// English until a native locale API is worth a dependency.
fn detect_system_locale() -> L10n {
    for key in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(value) = std::env::var(key) {
            if let Some(lang) = parse_locale(&value) {
                return lang;
            }
        }
    }
    L10n::En
}

/// `de_DE.UTF-8` → German; unknown or `C` locales → `None`.
fn parse_locale(value: &str) -> Option<L10n> {
    let code = value.split('.').next().unwrap_or("");
    let code = code.split('_').next().unwrap_or("");
    L10n::from_str(code).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrips_through_disk() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("language.json");
        persist_to(&path, L10n::De);
        assert_eq!(parse_from(&path), Some(L10n::De));
    }

    #[test]
    fn missing_file_parses_to_none() {
        let tmp = tempfile::tempdir().unwrap();
        assert_eq!(parse_from(&tmp.path().join("absent.json")), None);
    }

    #[test]
    fn corrupt_file_parses_to_none() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("language.json");
        std::fs::write(&path, "{not json").unwrap();
        assert_eq!(parse_from(&path), None);
    }

    #[test]
    fn future_schema_version_parses_to_none() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("language.json");
        std::fs::write(&path, r#"{"version":99,"language":"de"}"#).unwrap();
        assert_eq!(parse_from(&path), None);
    }

    #[test]
    fn unknown_language_code_parses_to_none() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("language.json");
        std::fs::write(&path, r#"{"version":1,"language":"zz"}"#).unwrap();
        assert_eq!(parse_from(&path), None);
    }

    #[test]
    fn system_locale_detection_reads_lang() {
        assert_eq!(parse_locale("de_DE.UTF-8"), Some(L10n::De));
        assert_eq!(parse_locale("en"), Some(L10n::En));
        assert_eq!(parse_locale("C"), None);
        assert_eq!(parse_locale("fr_FR.UTF-8"), None);
    }

    // Test seam: persistence against an explicit path (the real
    // persist/parse use config_base(); the logic is shared).
    fn persist_to(path: &std::path::Path, lang: L10n) {
        let _ = std::fs::write(
            path,
            serde_json::json!({ "version": SCHEMA_VERSION, "language": lang.to_string() })
                .to_string(),
        );
    }

    fn parse_from(path: &std::path::Path) -> Option<L10n> {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|text| super::parse(&text))
    }
}
