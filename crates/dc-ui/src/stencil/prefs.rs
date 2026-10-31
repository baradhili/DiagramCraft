//! Stencil preferences (GitHub #105): the default stencil set that
//! auto-opens with the app, and the built-in categories that appear
//! (and in which order) — app configuration, never `.vsdx` content.
//!
//! Persisted as a small user-editable JSON file in the app-config dir
//! (`stencils.json`, beside `recent.json`) — created on first run
//! with the defaults so it can simply be edited; a selection UI can
//! grow on top later (the issue lands the mechanism + defaults).
//!
//! Default auto-open set (the shipped Visio 2016 companion set, REA
//! evidence cited in #87/#105 — "Basic, containers, callouts, …"):
//! Basic Shapes, Callouts, Containers, plus the catalog's Backgrounds
//! and Borders & Titles galleries as the closest matches for the rest
//! of the observed set.

use std::path::PathBuf;

/// The default auto-open slugs, in open order (Visio 2016's
/// companion set — see the module docs).
pub const DEFAULT_OPEN: &[&str] = &["basic", "calout", "sdcont", "bckgrn", "bordrs"];

/// The stencil preferences.
#[derive(Debug, Clone, PartialEq)]
pub struct StencilPrefs {
    /// Entries to auto-open with the app, in order: a built-in slug
    /// (matched against the catalog), or a file name under My Shapes
    /// (opened like a user stencil when it exists).
    pub default_open: Vec<String>,
    /// The built-in categories that appear in More Shapes, in menu
    /// order. `None` = the catalog's manifest order, all categories —
    /// the pre-#105 behavior; `Some(list)` shows exactly the listed
    /// categories in that order (unknown names simply match nothing).
    pub categories: Option<Vec<String>>,
    /// Where the prefs persist; `None` = in-memory (ephemeral
    /// constructors — headless tests never touch the user's config).
    store: Option<PathBuf>,
}

impl Default for StencilPrefs {
    fn default() -> Self {
        StencilPrefs {
            default_open: DEFAULT_OPEN.iter().map(|s| s.to_string()).collect(),
            categories: None,
            store: None,
        }
    }
}

impl StencilPrefs {
    /// A non-persisting copy of the DEFAULTS (headless tests,
    /// kittest).
    pub fn ephemeral() -> StencilPrefs {
        StencilPrefs::default()
    }

    /// kittest (#105): a non-persisting prefs value with the given
    /// content.
    #[doc(hidden)]
    pub fn for_test(
        default_open: Vec<String>,
        categories: Option<Vec<String>>,
    ) -> StencilPrefs {
        StencilPrefs {
            default_open,
            categories,
            store: None,
        }
    }

    /// Load from the user-config path. A missing file is CREATED with
    /// the defaults (first run — the file is the configuration
    /// surface); a corrupt or unknown-version file falls back to the
    /// defaults without overwriting it — and neither does any other
    /// read error (permissions, I/O): only `NotFound` creates the
    /// file, so a readable-but-unwritable existing config survives
    /// (CodeRabbit).
    pub fn open() -> StencilPrefs {
        let Some(path) = crate::recent::config_base()
            .map(|b| b.join("diagramcraft").join("stencils.json"))
        else {
            return StencilPrefs::ephemeral();
        };
        match std::fs::read_to_string(&path) {
            Ok(text) => Self::parse(&text).unwrap_or_else(StencilPrefs::default),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let defaults = StencilPrefs::default();
                defaults.persist_to(&path);
                defaults
            }
            // Unreadable for another reason: fall back to defaults for
            // this session without touching the file on disk.
            Err(_) => StencilPrefs::default(),
        }
    }

    /// Parse the JSON form; `None` on any structural mismatch.
    fn parse(text: &str) -> Option<StencilPrefs> {
        let v: serde_json::Value = serde_json::from_str(text).ok()?;
        if v.get("version")?.as_i64()? != 1 {
            return None;
        }
        let default_open = v
            .get("default_open")?
            .as_array()?
            .iter()
            .map(|e| e.as_str().map(str::to_string))
            .collect::<Option<Vec<_>>>()?;
        let categories = match v.get("categories") {
            // Absent or explicit null = unset (the file form writes
            // null for the manifest-order default).
            None | Some(serde_json::Value::Null) => None,
            Some(c) => Some(
                c.as_array()?
                    .iter()
                    .map(|e| e.as_str().map(str::to_string))
                    .collect::<Option<Vec<_>>>()?,
            ),
        };
        Some(StencilPrefs {
            default_open,
            categories,
            store: None,
        })
    }

    /// Serialize + write (best-effort — a read-only config dir must
    /// not break startup), returning whether the file landed.
    fn persist_to(&self, path: &std::path::Path) -> bool {
        let mut cats = serde_json::Value::Null;
        if let Some(c) = &self.categories {
            cats = serde_json::Value::Array(
                c.iter().map(|s| serde_json::json!(s)).collect(),
            );
        }
        let text = serde_json::json!({
            "version": 1,
            "default_open": self.default_open,
            "categories": cats,
        })
        .to_string();
        std::fs::create_dir_all(path.parent().unwrap_or(path)).is_ok()
            && std::fs::write(path, text).is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The defaults are the Visio 2016 companion set, all present in
    /// the catalog (a default naming a missing slug would silently
    /// skip — fine at runtime, but the defaults themselves must be
    /// real).
    #[test]
    fn default_open_names_catalog_slugs() {
        assert!(!DEFAULT_OPEN.is_empty());
        for slug in DEFAULT_OPEN {
            assert!(
                crate::stencil::builtin::by_slug(slug).is_some(),
                "default slug {slug:?} is not in the catalog"
            );
        }
        assert_eq!(
            StencilPrefs::default().categories,
            None,
            "no category config = manifest order, all categories"
        );
    }

    /// Round-trip: defaults serialize, parse back identically; a
    /// custom file parses; corrupt/unknown versions fall back.
    #[test]
    fn prefs_round_trip_through_the_file_form() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("stencils.json");

        let defaults = StencilPrefs::default();
        assert!(defaults.persist_to(&path), "writable tempdir");
        let back = StencilPrefs::parse(&std::fs::read_to_string(&path).unwrap())
            .expect("written file parses");
        assert_eq!(back, defaults);

        let custom = StencilPrefs {
            default_open: vec!["basic".into(), "my-own.vssx".into()],
            categories: Some(vec!["General".into(), "Engineering".into()]),
            store: None,
        };
        assert!(custom.persist_to(&path));
        let back = StencilPrefs::parse(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(back, custom);

        std::fs::write(&path, "{\"version\": 99}").unwrap();
        assert!(StencilPrefs::parse("{\"version\": 99}").is_none());
        assert!(
            StencilPrefs::parse("not json at all").is_none(),
            "corrupt files fall back to defaults without panicking"
        );
    }
}
