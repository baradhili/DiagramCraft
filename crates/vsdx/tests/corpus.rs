//! Tracked-corpus round-trip regression (#97 §6 item 9): every
//! rights-clean package committed to the repo — the blank starter
//! document plus the #87 built-in stencil catalog (Visio-derived
//! masters repackaged into synthetic `.vssx` groupings, per the owner
//! decision of 2026-10-25) — must survive parse → write → parse with
//! an unchanged model and an unchanged part inventory. This is the
//! in-repo subset of the full 679-file local corpus exercise (which
//! stays a manual run against the untracked `Sample/Office16`
//! install): it catches "unmodeled real construct" escapes the
//! synthetic fixtures cannot, because these are repackaged real
//! masters.

use std::path::PathBuf;

fn corpus_files() -> Vec<PathBuf> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut files = vec![root.join("../dc-core/assets/blank.vsdx")];
    let stencils = root.join("../dc-ui/assets/stencils");
    let mut vssx: Vec<_> = std::fs::read_dir(&stencils)
        .expect("stencil catalog directory")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "vssx"))
        .collect();
    vssx.sort();
    assert!(vssx.len() > 300, "expected the full #87 catalog, found {}", vssx.len());
    files.extend(vssx);
    files
}

/// The always-green subset (#97 §6 item 9: "10–20 rights-clean
/// fixtures"): the blank starter plus a deterministic stride over the
/// #87 catalog — bounded runtime (~6 s) so it runs on every push.
fn subset_files() -> Vec<PathBuf> {
    let mut all = corpus_files();
    let stencils: Vec<PathBuf> = all
        .split_off(1) // blank.vsdx stays
        .into_iter()
        .step_by(19)
        .collect();
    all.extend(stencils);
    all
}

/// The full 374-stencil catalog run (≈112 s debug) — the release
/// recipe: `cargo test -p vsdx --release --test corpus -- --ignored`.
#[test]
#[ignore = "full-catalog twin (release recipe); the strided subset runs by default"]
fn full_catalog_round_trips_losslessly() {
    let tmp = tempfile::tempdir().unwrap();
    round_trip_all(&corpus_files(), tmp.path());
}

#[test]
fn tracked_corpus_subset_round_trips_losslessly() {
    let tmp = tempfile::tempdir().unwrap();
    round_trip_all(&subset_files(), tmp.path());
}

fn round_trip_all(files: &[PathBuf], dir: &std::path::Path) {
    for (i, path) in files.iter().enumerate() {
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let doc = match vsdx::parse::parse(path) {
            Ok(d) => d,
            Err(e) => panic!("corpus {name}: parse failed: {e:#}"),
        };
        let out = dir.join(format!("rt-{i}.vsdx"));
        vsdx::write::save(&doc, &out).unwrap_or_else(|e| panic!("corpus {name}: save failed: {e:#}"));
        let back = vsdx::parse::parse(&out)
            .unwrap_or_else(|e| panic!("corpus {name}: re-parse failed: {e:#}"));
        assert!(
            doc.same_model(&back),
            "corpus {name}: model changed across the round-trip"
        );
        let before: Vec<_> = doc.package.as_ref().map(|p| p.part_names()).unwrap_or_default();
        let after: Vec<_> = back.package.as_ref().map(|p| p.part_names()).unwrap_or_default();
        assert_eq!(before, after, "corpus {name}: part inventory changed");
    }
}
