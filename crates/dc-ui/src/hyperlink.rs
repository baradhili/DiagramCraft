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

    #[test]
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

    #[test]
    fn doc_dir_of_a_saved_file() {
        assert_eq!(doc_dir(Path::new("/drawings/a.vsdx")), Some(Path::new("/drawings")));
        assert_eq!(doc_dir(Path::new("a.vsdx")), None);
    }
}
