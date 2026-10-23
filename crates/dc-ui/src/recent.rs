//! Persistent most-recently-used file list (GitHub #16).
//!
//! Stored as a small app-owned JSON file under the per-user config dir
//! (`$XDG_CONFIG_HOME`/`~/.config` on Linux, `%APPDATA%` on Windows,
//! `~/Library/Application Support` on macOS) — never under the install
//! dir, so it survives app upgrades and reboots by construction (the
//! NSIS installer writes only to Program Files). A `version` field
//! guards forward-compatible schema changes: a missing, corrupt, or
//! version-mismatched file starts an empty list and never fails
//! startup.

use std::path::{Path, PathBuf};

/// Visio keeps ~10 entries; match that.
const MAX_ENTRIES: usize = 10;
const SCHEMA_VERSION: u64 = 1;

#[derive(Debug, Clone, Default)]
pub struct RecentFiles {
    /// Most recent first.
    files: Vec<PathBuf>,
    /// Where the list persists; `None` = in-memory only (tests,
    /// kittest — the headless constructors never touch the user's
    /// real config).
    store: Option<PathBuf>,
}

impl RecentFiles {
    /// An empty, non-persisting list (headless tests, kittest).
    pub fn ephemeral() -> RecentFiles {
        RecentFiles::default()
    }

    /// Load from the user-config path, starting empty on any error
    /// (missing file, corrupt JSON, unknown schema version). With no
    /// config base discoverable (unset HOME/XDG/APPDATA) the list is
    /// ephemeral — nothing is persisted to a relative path.
    pub fn open() -> RecentFiles {
        let Some(path) = default_path() else {
            return RecentFiles::ephemeral();
        };
        let files = std::fs::read_to_string(&path)
            .ok()
            .and_then(|text| parse(&text))
            .unwrap_or_default();
        RecentFiles {
            files,
            store: Some(path),
        }
    }

    /// The MRU, most recent first.
    pub fn entries(&self) -> &[PathBuf] {
        &self.files
    }

    /// Record a successfully opened/saved file: move-to-front, dedupe,
    /// cap. Persistence is best-effort — a read-only config dir must
    /// never take the app down; the list simply stays in memory for
    /// this run.
    pub fn record(&mut self, path: &Path) {
        self.files.retain(|p| p != path);
        self.files.insert(0, path.to_path_buf());
        self.files.truncate(MAX_ENTRIES);
        self.persist();
    }

    fn persist(&self) {
        if let Some(store) = &self.store {
            if let Some(dir) = store.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let _ = std::fs::write(store, serialize(&self.files));
        }
    }

    /// Test hook: adopt `files` verbatim (most recent first), never
    /// persisted.
    #[doc(hidden)]
    pub fn set_entries_for_test(&mut self, files: Vec<PathBuf>) {
        self.files = files;
    }
}

fn serialize(files: &[PathBuf]) -> String {
    // `to_str` (not lossy `display`): a non-UTF-8 path would be silently
    // corrupted by replacement characters, so it is skipped instead.
    let names: Vec<&str> = files.iter().filter_map(|p| p.to_str()).collect();
    serde_json::json!({ "version": SCHEMA_VERSION, "files": names }).to_string()
}

/// Parse the on-disk form; `None` for anything unreadable or carrying
/// a schema version this build does not know (v1 is the only one — a
/// future bump that can migrate will match on its old version here).
/// Entries beyond MAX_ENTRIES (a hand-edited file) are dropped so the
/// loaded list keeps the same cap as saved ones.
fn parse(text: &str) -> Option<Vec<PathBuf>> {
    let v: serde_json::Value = serde_json::from_str(text).ok()?;
    if v.get("version")?.as_u64()? != SCHEMA_VERSION {
        return None;
    }
    let names = v.get("files")?.as_array()?;
    Some(
        names
            .iter()
            .filter_map(|n| n.as_str())
            .take(MAX_ENTRIES)
            .map(PathBuf::from)
            .collect(),
    )
}

/// `…/diagramcraft/recent.json` under the per-user config dir; `None`
/// when no config base is discoverable.
fn default_path() -> Option<PathBuf> {
    config_base().map(|b| b.join("diagramcraft").join("recent.json"))
}

fn config_base() -> Option<PathBuf> {
    if let Some(x) = std::env::var_os("XDG_CONFIG_HOME") {
        if !x.is_empty() {
            return Some(PathBuf::from(x));
        }
    }
    #[cfg(windows)]
    if let Some(a) = std::env::var_os("APPDATA") {
        if !a.is_empty() {
            return Some(PathBuf::from(a));
        }
    }
    #[cfg(target_os = "macos")]
    {
        if let Some(h) = std::env::var_os("HOME") {
            return Some(PathBuf::from(h).join("Library").join("Application Support"));
        }
    }
    std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stored_in(dir: &Path) -> RecentFiles {
        RecentFiles {
            files: Vec::new(),
            store: Some(dir.join("recent.json")),
        }
    }

    fn p(name: &str) -> PathBuf {
        PathBuf::from(name)
    }

    #[test]
    fn record_moves_to_front_and_dedupes() {
        let mut mru = RecentFiles::ephemeral();
        mru.record(&p("/a.vsdx"));
        mru.record(&p("/b.vsdx"));
        mru.record(&p("/a.vsdx"));
        assert_eq!(mru.entries(), &[p("/a.vsdx"), p("/b.vsdx")]);
    }

    #[test]
    fn record_caps_at_max_entries() {
        let mut mru = RecentFiles::ephemeral();
        for i in 0..(MAX_ENTRIES + 4) {
            mru.record(&p(&format!("/f{i}.vsdx")));
        }
        assert_eq!(mru.entries().len(), MAX_ENTRIES);
        assert_eq!(mru.entries()[0], p(&format!("/f{}.vsdx", MAX_ENTRIES + 3)));
    }

    #[test]
    fn roundtrips_through_disk() {
        let tmp = tempfile::tempdir().unwrap();
        let store = tmp.path().join("recent.json");
        let mut mru = RecentFiles {
            files: Vec::new(),
            store: Some(store.clone()),
        };
        mru.record(&p("/one.vsdx"));
        mru.record(&p("/two.vsdx"));
        // Reload from the same store (what a next app run does).
        let mut reloaded = RecentFiles {
            files: std::fs::read_to_string(&store)
                .ok()
                .and_then(|t| parse(&t))
                .unwrap_or_default(),
            store: Some(store),
        };
        assert_eq!(reloaded.entries(), &[p("/two.vsdx"), p("/one.vsdx")]);
        reloaded.record(&p("/one.vsdx"));
        assert_eq!(reloaded.entries(), &[p("/one.vsdx"), p("/two.vsdx")]);
    }

    #[test]
    fn missing_file_starts_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let mru = stored_in(tmp.path());
        assert!(mru.entries().is_empty());
    }

    #[test]
    fn corrupt_file_starts_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("recent.json");
        std::fs::write(&path, "{not json at all").unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(parse(&text).is_none(), "corrupt JSON must parse to None");
    }

    #[test]
    fn future_schema_version_starts_empty() {
        let text = r#"{"version":99,"files":["/x.vsdx"]}"#;
        assert!(
            parse(text).is_none(),
            "unknown schema version must parse to None"
        );
    }

    #[test]
    fn version_field_is_written() {
        let text = serialize(&[p("/x.vsdx")]);
        assert!(text.contains(r#""version":1"#), "serialized form: {text}");
    }

    #[test]
    fn parse_caps_loaded_entries_at_max() {
        let files: Vec<String> = (0..MAX_ENTRIES + 5)
            .map(|i| format!("/f{i}.vsdx"))
            .collect();
        let text = serde_json::json!({ "version": 1, "files": files }).to_string();
        assert_eq!(parse(&text).unwrap().len(), MAX_ENTRIES);
    }

    #[cfg(unix)]
    #[test]
    fn non_utf8_paths_are_skipped_not_corrupted() {
        use std::os::unix::ffi::OsStrExt;
        let bad = PathBuf::from(std::ffi::OsStr::from_bytes(b"/tmp/\xff\xfe.vsdx"));
        let text = serialize(&[p("/ok.vsdx"), bad]);
        let back = parse(&text).unwrap();
        assert_eq!(back, &[p("/ok.vsdx")], "raw form: {text}");
    }

    #[test]
    fn persistence_failure_is_silent() {
        // A store under a file-used-as-directory: create_dir_all fails,
        // record must not panic.
        let tmp = tempfile::tempdir().unwrap();
        let blocker = tmp.path().join("blocker");
        std::fs::write(&blocker, b"").unwrap();
        let mut mru = RecentFiles {
            files: Vec::new(),
            store: Some(blocker.join("recent.json")),
        };
        mru.record(&p("/x.vsdx"));
        assert_eq!(mru.entries(), &[p("/x.vsdx")]);
    }
}
