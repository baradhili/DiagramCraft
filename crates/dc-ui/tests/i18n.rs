//! #246 i18n gate: the catalogs and the code move together or this
//! fails. Four structural checks over the project-level `locales/`
//! catalogs (the same contract `flynt` lints) plus the literal scan
//! (an unrouted chrome string fails with the ID to route) and the
//! language-switch kittest coverage (German shell golden).
//!
//! This is a SEPARATE test binary on purpose: the language is
//! process-global, and every other test binary asserts English (the
//! headless constructors pin it). The switch tests here hold
//! [`LANG_LOCK`] and restore English before returning.

use dc_ui::app::App;
use egui_kittest::kittest::Queryable;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

static LANG_LOCK: Mutex<()> = Mutex::new(());

/// Panic-safe: restores English even when an assertion fails mid-test,
/// so a leaked German setting can't flip parallel tests in this binary.
struct EnglishOnDrop;
impl Drop for EnglishOnDrop {
    fn drop(&mut self) {
        dc_ui::i18n::pin_english();
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn catalog_path(lang: &str) -> PathBuf {
    repo_root().join("locales").join(lang).join("app.ftl")
}

/// The message IDs of one catalog (`id = ...` lines, continuations and
/// comments skipped).
fn catalog_ids(lang: &str) -> BTreeSet<String> {
    let text = std::fs::read_to_string(catalog_path(lang)).expect("catalog readable");
    let mut ids = BTreeSet::new();
    for line in text.lines() {
        if line.starts_with('#') || line.starts_with(' ') || line.trim().is_empty() {
            continue;
        }
        if let Some(eq) = line.find('=') {
            let id = line[..eq].trim();
            if !id.is_empty()
                && id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
                && id.chars().next().is_some_and(|c| c.is_ascii_lowercase())
            {
                ids.insert(id.to_string());
            }
        }
    }
    ids
}

/// The dc-ui sources the gate reasons about (the generated accessor
/// file and the i18n runtime itself are excluded — they are the
/// mechanism, not the strings).
fn src_files() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut out = Vec::new();
    for f in walk(&root) {
        let name = f.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if name == "l10n.rs" || f.parent().map(|p| p.ends_with("i18n")).unwrap_or(false) {
            continue;
        }
        out.push(f);
    }
    out.sort();
    out
}

fn walk(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            out.extend(walk(&p));
        } else if p.extension().and_then(|e| e.to_str()) == Some("rs") {
            out.push(p);
        }
    }
    out
}

/// Every accessor the code calls: `strs().msg_some_id()` → `some-id`.
fn routed_ids() -> BTreeSet<String> {
    let mut ids = BTreeSet::new();
    for f in src_files() {
        let text = std::fs::read_to_string(&f).expect("src readable");
        for caps in RE_MSG.captures_iter(&text) {
            let snake = caps.get(1).unwrap().as_str();
            ids.insert(snake.replace('_', "-"));
        }
    }
    ids
}

use regex::Regex;
use std::sync::LazyLock;

static RE_MSG: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"strs\(\)\.msg_([a-z0-9_]+)\(").unwrap());

#[test]
fn catalogs_have_identical_key_sets() {
    let en = catalog_ids("en");
    let de = catalog_ids("de");
    let only_en: Vec<_> = en.difference(&de).collect();
    let only_de: Vec<_> = de.difference(&en).collect();
    assert!(
        only_en.is_empty() && only_de.is_empty(),
        "catalogs drifted apart — every ID must exist in BOTH files: en-only {only_en:?} de-only {only_de:?}"
    );
}

#[test]
fn every_routed_id_exists_in_every_catalog() {
    let en = catalog_ids("en");
    let de = catalog_ids("de");
    let routed = routed_ids();
    let missing_en: Vec<_> = routed.difference(&en).collect();
    let missing_de: Vec<_> = routed.difference(&de).collect();
    assert!(
        missing_en.is_empty(),
        "routed but missing from en/app.ftl: {missing_en:?}"
    );
    assert!(
        missing_de.is_empty(),
        "routed but missing from de/app.ftl: {missing_de:?}"
    );
}

#[test]
fn no_orphan_messages() {
    let en = catalog_ids("en");
    let routed = routed_ids();
    // `language-name` never routes through msg_(): fluent-typed reads
    // it at build time to emit `L10n::language_name()` (the Language
    // dialog's native names).
    let generator_conventions: BTreeSet<String> =
        ["language-name".to_string()].into_iter().collect();
    let orphans: Vec<_> = en.difference(&routed).filter(|id| !generator_conventions.contains(*id)).collect();
    assert!(
        orphans.is_empty(),
        "catalog messages nothing references — delete them or route them: {orphans:?}"
    );
}

/// The literal scan (the `litscan` pass of #246): a bare string
/// literal equal to a catalog message's English value, in a call-arg
/// position, that the sweep did not route — route it (or it is model
/// vocabulary that must not live in the catalog).
#[test]
fn litscan_flags_unrouted_chrome_literals() {
    let text_of = |lang| std::fs::read_to_string(catalog_path(lang)).expect("catalog");
    let mut values: BTreeSet<String> = BTreeSet::new();
    let mut current: Option<String> = None;
    for line in text_of("en").lines() {
        if line.starts_with('#') {
            continue;
        }
        if let Some(eq) = line.find(" = ") {
            let (id, rest) = (&line[..eq], line[eq + 3..].trim());
            current = Some(id.to_string());
            if !rest.contains("{$") && !rest.is_empty() {
                values.insert(rest.to_string());
            }
        } else if line.starts_with(' ') {
            if let Some(id) = &current {
                let _ = id; // multi-line values: skip (placeholder/select forms)
            }
        } else {
            current = None;
        }
    }
    // Model vocabulary whose English happens to equal a catalog
    // value: the container heading is shape CONTENT; "Background" is
    // a page-attribute key; font names are family identifiers.
    let exempt_values: BTreeSet<String> = ["Container".to_string(), "Background".to_string(), "Calibri".to_string()]
        .into_iter()
        .collect();
    let routed = routed_ids();
    let mut offenders = Vec::new();
    for f in src_files() {
        // Parked display layers (#257): the ShapeSheet inspector's
        // section titles and the built-in stencil titles stay English
        // this wave, so their literals are not unrouted chrome.
        let sp = f.to_string_lossy();
        if sp.ends_with("shapesheet.rs") || sp.ends_with("builtin.rs") {
            continue;
        }
        let text = std::fs::read_to_string(&f).expect("src readable");
        let lines: Vec<&str> = text.lines().collect();
        // #[cfg(test)] fixtures are deliberate literals (see the
        // keytips letter-assignment tests) — not unrouted chrome.
        let mut in_tests = 0usize; // brace depth inside `mod tests`
        for (i, line) in lines.iter().enumerate() {
            if in_tests > 0 {
                in_tests += line.matches('{').count();
                in_tests -= line.matches('}').count();
                if in_tests > 0 {
                    continue;
                }
            }
            if line.trim_start().starts_with("#[cfg(test)]")
                || line.trim_start().starts_with("mod tests")
            {
                in_tests = 1 + line.matches('{').count() - line.matches('}').count();
                continue;
            }
            // conservative guard set mirroring the sweep: routed lines,
            // format!/const/match contexts, model-key lookups (cell and
            // section names are .vsdx vocabulary — never translated).
            // a small context window: key patterns often sit several
            // lines above the literal in a multi-line model call
            let lo = i.saturating_sub(8);
            let ctx = lines[lo..=i].join("\n");
            if ctx.contains("msg_")
                || ctx.contains("format!")
                || line.contains("==")
                || line.contains("=>")
                || line.trim_start().starts_with("//")
                || RE_CONST.is_match(&ctx)
                || RE_KEY.is_match(&ctx)
            {
                continue;
            }
            for caps in RE_LIT.captures_iter(line) {
                let value = unescape(&caps[1]);
                if values.contains(&value) && !exempt_values.contains(&value) {
                    let kebab = value
                        .to_lowercase()
                        .chars()
                        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
                        .collect::<String>();
                    offenders.push(format!(
                        "{}:{}: {:?} — route it or drop it from the catalog",
                        f.display(),
                        i + 1,
                        value
                    ));
                    let _ = kebab;
                    let _ = &routed;
                }
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "unrouted chrome literals:\n{}",
        offenders.join("\n")
    );
}

static RE_CONST: LazyLock<Regex> =
    LazyLock::new(|| regex::Regex::new(r"\b(const|static)\s+\w+\s*:").unwrap());
static RE_LIT: LazyLock<Regex> =
    LazyLock::new(|| regex::Regex::new(r#""([^"\\]*(?:\\.[^"\\]*)*)""#).unwrap());
/// Model-key lookups: cell and section names are .vsdx vocabulary and
/// never translate, so literals in these call shapes are not chrome.
static RE_KEY: LazyLock<Regex> = LazyLock::new(|| {
    regex::Regex::new(
        r"\.cell\(|\.sections_named\(|eff_cell_str\(|eff_row_str\(|env\.insert|SetCell::new\(|set_row|set_cell|attrs\.push|type_:|Panel::left|SheetChild::Section|type_ == |RemoveSectionRow|attrs\s*[:=]|IndexChild|children: vec|SetRowCell::new\(|mod tests",
    )
    .unwrap()
});

fn unescape(raw: &str) -> String {
    let mut out = String::new();
    let mut chars = raw.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some('"') => out.push('"'),
                Some('\\') => out.push('\\'),
                Some(other) => out.push(other),
                None => {}
            }
        } else {
            out.push(c);
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Language-switch coverage (this binary only — process-global state).
// ---------------------------------------------------------------------------

/// The shell re-renders in German: a golden pins the whole chrome and
/// the label lookup proves the catalog really drives the strings.
#[test]
fn language_switch_renders_german_shell() {
    let _guard = LANG_LOCK.lock().unwrap();
    let _english = EnglishOnDrop;
    // Pin FIRST, construct, then switch: the headless constructor pins
    // English, so the switch must come after it.
    dc_ui::i18n::pin_english();
    let mut app = App::with_blank();
    dc_ui::i18n::set_language_ephemeral(dc_ui::i18n::L10n::De);
    dc_ui::svgimg::pin_determinism_for_tests();
    let mut harness = egui_kittest::Harness::builder()
        .with_size(egui::vec2(1280.0, 800.0))
        .build_ui(move |ui| {
            app.draw(ui);
        });
    harness.run();
    harness.run();
    // The German catalog's word for the File tab — the shell chrome
    // must come from the catalog, not hardcoded English.
    harness.get_by_label_contains("Datei");
    harness.snapshot("i18n_german_shell");
}
