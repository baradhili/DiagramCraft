//! Hyperlink address storage helpers (GitHub #71 — the 2016
//! Hyperlinks dialog's relative-path surface).
//!
//! The 2016 dialog's "Use relative paths for hyperlinks" checkbox
//! governs how the Address of a link is STORED: with the toggle on,
//! an address that points inside the document's own directory is
//! kept relative to it (a sibling file is just its name); anything
//! else — outside the directory, or the toggle off — stays absolute.
//! On use, a stored relative address resolves back against the
//! document's directory, so links survive the file (and its folder)
//! being moved around together.

use std::path::{Path, PathBuf};

/// A stored address that names a scheme (`https://`, `mailto:`, …)
/// is a URL, never a filesystem path.
fn is_url(address: &str) -> bool {
    let a = address.trim();
    match a.find("://") {
        Some(i) => a[..i].chars().all(|c| c.is_ascii_alphanumeric()),
        None => a.starts_with("mailto:"),
    }
}

/// Store `address` the way the document's toggle wants it: relative
/// to `doc_dir` when `relative` is on and the address is an absolute
/// path inside that directory; verbatim otherwise (URLs and outside
/// paths never relativize).
pub fn store_address(address: &str, doc_dir: Option<&Path>, relative: bool) -> String {
    let a = address.trim();
    if !relative || is_url(a) {
        return a.to_string();
    }
    let Some(dir) = doc_dir else {
        return a.to_string();
    };
    let p = Path::new(a);
    if !p.is_absolute() {
        return a.to_string(); // already relative (or empty)
    }
    match p.strip_prefix(dir) {
        Ok(rel) => rel.to_string_lossy().into_owned(),
        Err(_) => a.to_string(), // outside the document's directory
    }
}

/// Resolve a stored address for opening: a relative filesystem
/// address joins the document's directory; URLs and absolute paths
/// pass through. `None` doc dir (an unsaved document) passes the
/// address through — there is nothing to resolve against.
pub fn resolve_address(address: &str, doc_dir: Option<&Path>) -> String {
    let a = address.trim();
    if is_url(a) {
        return a.to_string();
    }
    if Path::new(a).is_absolute() {
        return a.to_string();
    }
    match doc_dir {
        Some(dir) => PathBuf::from(dir).join(a).to_string_lossy().into_owned(),
        None => a.to_string(),
    }
}

/// The document's directory for hyperlink resolution: the folder the
/// saved file lives in (`/drawings/a.vsdx` → `/drawings`).
pub fn doc_dir(path: &Path) -> Option<&Path> {
    path.parent().filter(|p| !p.as_os_str().is_empty())
}

/// The "Use relative paths for hyperlinks" toggle, per document
/// (GitHub #71). Visio 2016 keeps this toggle APPLICATION-level
/// (observed, cleanroom); we persist it app-side keyed by the
/// document's path — never inside the .vsdx, whose
/// `<DocumentSettings>` schema is closed (CodeRabbit: an unknown
/// child element can fail Visio's validation on open). Same app-owned
/// config pattern as the MRU list and Quick Shapes membership.
#[derive(Default)]
pub struct RelativePaths {
    on: std::collections::BTreeMap<PathBuf, bool>,
    /// Where the map persists; `None` = in-memory only (the headless
    /// constructors never touch the user's real config).
    store: Option<PathBuf>,
}

impl RelativePaths {
    /// A non-persisting map (headless tests, kittest).
    pub fn ephemeral() -> RelativePaths {
        RelativePaths::default()
    }

    /// Load from the user-config path, starting empty on any error
    /// (missing file, corrupt JSON, unknown schema). With no config
    /// base discoverable the store is ephemeral.
    pub fn open() -> RelativePaths {
        let Some(path) = crate::recent::config_base()
            .map(|b| b.join("diagramcraft").join("hyperlink-relative.json"))
        else {
            return RelativePaths::ephemeral();
        };
        let on = std::fs::read_to_string(&path)
            .ok()
            .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
            .and_then(|v| {
                v.get("on")
                    .and_then(|a| a.as_array())
                    .map(|a| {
                        a.iter()
                            .filter_map(|e| {
                                Some((
                                    PathBuf::from(e.get(0)?.as_str()?),
                                    e.get(1)?.as_bool()?,
                                ))
                            })
                            .collect()
                    })
            })
            .unwrap_or_default();
        RelativePaths {
            on,
            store: Some(path),
        }
    }

    /// The toggle for `doc` (an unsaved document keys the empty path
    /// — session-only, since nothing else identifies it).
    pub fn get(&self, doc: &Path) -> bool {
        self.on.get(doc).copied().unwrap_or(false)
    }

    /// Set the toggle for `doc`; persistence is best-effort — a
    /// read-only config dir must not break the toggle.
    pub fn set(&mut self, doc: &Path, value: bool) {
        self.on.insert(doc.to_path_buf(), value);
        if let Some(path) = &self.store {
            let entries: Vec<(&PathBuf, &bool)> = self.on.iter().collect();
            let _ = std::fs::create_dir_all(path.parent().unwrap_or(path));
            let _ = std::fs::write(
                path,
                serde_json::json!({
                    "version": 1,
                    "on": entries.iter()
                        .map(|(p, v)| serde_json::json!([p.to_string_lossy(), v]))
                        .collect::<Vec<_>>(),
                })
                .to_string(),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls_never_relativize_or_join() {
        let dir = Path::new("/drawings");
        assert_eq!(
            store_address("https://example.test/x", Some(dir), true),
            "https://example.test/x"
        );
        assert_eq!(
            store_address("mailto:a@b.test", Some(dir), true),
            "mailto:a@b.test"
        );
        assert_eq!(
            resolve_address("https://example.test/x", Some(dir)),
            "https://example.test/x"
        );
    }

    // POSIX path literals: Windows `is_absolute` requires a drive
    // prefix (CodeRabbit) — the path logic itself is platform-neutral.
    #[test]
    #[cfg(unix)]
    fn inside_paths_relativize_only_when_told() {
        let dir = Path::new("/drawings");
        // Toggle on: a file inside the doc's directory stores as its
        // relative path, nested included.
        assert_eq!(
            store_address("/drawings/spec.pdf", Some(dir), true),
            "spec.pdf"
        );
        assert_eq!(
            store_address("/drawings/sub/spec.pdf", Some(dir), true),
            "sub/spec.pdf"
        );
        // Outside stays absolute; toggle off keeps everything as-is.
        assert_eq!(
            store_address("/elsewhere/spec.pdf", Some(dir), true),
            "/elsewhere/spec.pdf"
        );
        assert_eq!(
            store_address("/drawings/spec.pdf", Some(dir), false),
            "/drawings/spec.pdf"
        );
        // No document directory (unsaved): nothing to relativize
        // against.
        assert_eq!(
            store_address("/drawings/spec.pdf", None, true),
            "/drawings/spec.pdf"
        );
    }

    /// The issue's round-trip bar: a relativized address re-opens
    /// resolved against the document's directory, and the ORIGINAL
    /// absolute path comes back out.
    #[test]
    #[cfg(unix)]
    fn stored_relative_addresses_resolve_back() {
        let dir = Path::new("/drawings");
        let stored = store_address("/drawings/sub/spec.pdf", Some(dir), true);
        assert_eq!(stored, "sub/spec.pdf");
        assert_eq!(
            resolve_address(&stored, Some(dir)),
            PathBuf::from("/drawings").join("sub/spec.pdf").to_string_lossy()
        );
        // An unsaved document passes the relative form through.
        assert_eq!(resolve_address(&stored, None), stored);
    }

    /// The per-document toggle store: set/get per path, default off,
    /// and independent of persistence (the ephemeral store covers the
    /// kittest constructors).
    #[test]
    fn relative_paths_toggle_is_per_document() {
        let mut p = RelativePaths::ephemeral();
        let a = Path::new("/docs/a.vsdx");
        let b = Path::new("/docs/b.vsdx");
        assert!(!p.get(a), "absent reads off");
        p.set(a, true);
        assert!(p.get(a));
        assert!(!p.get(b), "the toggle is per document");
    }

    #[test]
    fn doc_dir_of_a_saved_file() {
        assert_eq!(doc_dir(Path::new("/drawings/a.vsdx")), Some(Path::new("/drawings")));
        assert_eq!(doc_dir(Path::new("a.vsdx")), None);
    }
}
