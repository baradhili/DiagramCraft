//! Per-user Quick Shapes membership (GitHub #143) — persisted
//! app-side in the config dir beside the MRU (#16), never inside the
//! `.vsdx` (#88's rule: the format carries no quick-shape flag, so
//! app-level is also the only faithful place).
//!
//! Membership model: the default head-per-source set (#88) ∪ per-user
//! additions ∖ per-user removals, keyed by (source stencil identity,
//! master id). Source identity is the open stencil's source path
//! (`builtin/…` pseudo-paths included) or [`DOCUMENT`] for the
//! drawing's own masters — stable across stencil close/reopen, unlike
//! pane order. The store follows the MRU discipline: a missing,
//! corrupt, or version-mismatched file starts empty, and persistence
//! is best-effort — a read-only config dir never takes the app down.

use std::collections::BTreeSet;
use std::path::PathBuf;

const SCHEMA_VERSION: u64 = 1;

/// One membership key: (source identity, master id).
type MemberKey = (String, i64);
/// The persisted sets: (additions, removals).
type MemberSets = (BTreeSet<MemberKey>, BTreeSet<MemberKey>);

/// Source identity of the drawing's own masters (the document set
/// exists in every session; open stencils key by their source path).
pub const DOCUMENT: &str = "document";

#[derive(Debug, Clone, Default)]
pub struct QuickShapes {
    added: BTreeSet<MemberKey>,
    removed: BTreeSet<MemberKey>,
    /// Where membership persists; `None` = in-memory only (tests,
    /// kittest — the headless constructors never touch the user's
    /// real config).
    store: Option<PathBuf>,
}

impl QuickShapes {
    /// An empty, non-persisting membership (headless tests, kittest).
    pub fn ephemeral() -> QuickShapes {
        QuickShapes::default()
    }

    /// Load from the user-config path, starting empty on any error.
    /// With no config base discoverable the membership is ephemeral.
    pub fn open() -> QuickShapes {
        match crate::recent::config_base() {
            Some(base) => QuickShapes::at(base.join("diagramcraft").join("quickshapes.json")),
            None => QuickShapes::ephemeral(),
        }
    }

    /// Load from an explicit store path (integration seam), starting
    /// empty when the file is missing or unreadable.
    pub fn at(path: PathBuf) -> QuickShapes {
        let (added, removed) = std::fs::read_to_string(&path)
            .ok()
            .and_then(|text| parse(&text))
            .unwrap_or_default();
        QuickShapes {
            added,
            removed,
            store: Some(path),
        }
    }

    /// Whether `(source, master)` is a Quick Shapes member: the
    /// default head membership union additions, minus removals.
    pub fn is_member(&self, source: &str, master: i64, in_default: bool) -> bool {
        let key = (source.to_string(), master);
        (in_default || self.added.contains(&key)) && !self.removed.contains(&key)
    }

    /// Add to Quick Shapes: clears any removal, so re-adding a removed
    /// default member restores it.
    pub fn add(&mut self, source: &str, master: i64) {
        let key = (source.to_string(), master);
        self.removed.remove(&key);
        self.added.insert(key);
        self.persist();
    }

    /// Remove from Quick Shapes: clears any addition, so removing an
    /// added extra returns the membership to the default set.
    pub fn remove(&mut self, source: &str, master: i64) {
        let key = (source.to_string(), master);
        self.added.remove(&key);
        self.removed.insert(key);
        self.persist();
    }

    fn persist(&self) {
        if let Some(store) = &self.store {
            if let Some(dir) = store.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let _ = std::fs::write(store, serialize(&self.added, &self.removed));
        }
    }
}

/// `(String, i64)` pairs as JSON arrays; non-UTF-8 sources cannot
/// occur (identities come from `to_str`, which yields none).
fn serialize(added: &BTreeSet<MemberKey>, removed: &BTreeSet<MemberKey>) -> String {
    let pair = |(s, id): &MemberKey| serde_json::json!([s, id]);
    serde_json::json!({
        "version": SCHEMA_VERSION,
        "added": added.iter().map(pair).collect::<Vec<_>>(),
        "removed": removed.iter().map(pair).collect::<Vec<_>>(),
    })
    .to_string()
}

/// Parse the on-disk form; `None` for anything unreadable or carrying
/// an unknown schema version (mirrors the MRU store's tolerance).
fn parse(text: &str) -> Option<MemberSets> {
    fn set(v: &serde_json::Value) -> Option<BTreeSet<MemberKey>> {
        Some(
            v.as_array()?
                .iter()
                .filter_map(|p| {
                    let a = p.as_array()?;
                    Some((a.first()?.as_str()?.to_string(), a.get(1)?.as_i64()?))
                })
                .collect(),
        )
    }
    let v: serde_json::Value = serde_json::from_str(text).ok()?;
    if v.get("version")?.as_u64()? != SCHEMA_VERSION {
        return None;
    }
    Some((set(v.get("added")?)?, set(v.get("removed")?)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn membership_is_defaults_union_additions_minus_removals() {
        let mut q = QuickShapes::ephemeral();
        assert!(q.is_member(DOCUMENT, 2, true), "a default head is a member");
        assert!(!q.is_member(DOCUMENT, 8, false), "a non-head is not");
        q.add(DOCUMENT, 8);
        assert!(q.is_member(DOCUMENT, 8, false), "an added extra is a member");
        q.remove(DOCUMENT, 2);
        assert!(!q.is_member(DOCUMENT, 2, true), "a removed default leaves");
        q.add(DOCUMENT, 2);
        assert!(q.is_member(DOCUMENT, 2, true), "re-adding restores it");
        q.remove(DOCUMENT, 8);
        assert!(
            !q.is_member(DOCUMENT, 8, false),
            "removing an extra returns to the default membership"
        );
    }

    #[test]
    fn store_round_trips_through_disk() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("quickshapes.json");
        {
            let mut q = QuickShapes::at(path.clone());
            q.add(DOCUMENT, 8);
            q.remove("builtin/basic-shapes", 4);
        }
        let back = QuickShapes::at(path.clone());
        assert!(back.is_member(DOCUMENT, 8, false), "the addition survived");
        assert!(
            !back.is_member("builtin/basic-shapes", 4, true),
            "the removal survived"
        );
        assert!(
            back.is_member("builtin/other", 4, true),
            "untouched defaults are unaffected"
        );
        // A corrupt file starts empty, never fails.
        std::fs::write(&path, "{ nope").unwrap();
        let fresh = QuickShapes::at(path);
        assert!(!fresh.is_member(DOCUMENT, 8, false));
    }
}
