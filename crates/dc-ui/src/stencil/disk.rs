//! The built-in catalog on disk (GitHub #104): the embedded synthetic
//! stencils extracted as real `.vssx` files under the user's My
//! Shapes folder, so they can be opened, copied, and inspected like
//! any user stencil — and so the default auto-open set (#105) and
//! user-managed folders grow from one location.
//!
//! Source of truth stays the EMBEDDED bytes (owner direction on the
//! issue's open question): extraction is idempotent and self-healing
//! — a missing or size-mismatched file is rewritten from the
//! embedded asset and digest-verified on write, while the build-time
//! digest test (`builtin.rs`) already pins the embedded bytes to the
//! committed manifest. `builtin/<slug>` pseudo-paths remain the
//! fallback identity when extraction is unavailable (no home
//! directory, read-only My Shapes).

use std::path::{Path, PathBuf};

use super::builtin::BuiltinStencil;

/// Where the extracted catalog lives: `<My Shapes>/Built-ins`.
pub fn builtins_dir(my_shapes: &Path) -> PathBuf {
    my_shapes.join("Built-ins")
}

/// The on-disk source/dedupe identity of an open built-in: the BASE
/// (US) variant's path — variant-independent, exactly like the
/// `builtin/<slug>` pseudo-path it mirrors.
pub fn extracted_path(my_shapes: &Path, slug: &str) -> PathBuf {
    builtins_dir(my_shapes).join(format!("{slug}.vssx"))
}

/// The file the requested variant opens from: the metric twin when
/// asked for and shipped, else the base variant (mirrors
/// [`BuiltinStencil::bytes_for`]).
fn variant_path(my_shapes: &Path, b: &BuiltinStencil, metric: bool) -> PathBuf {
    let name = if metric && b.metric_bytes.is_some() {
        format!("{}-m.vssx", b.slug)
    } else {
        format!("{}.vssx", b.slug)
    };
    builtins_dir(my_shapes).join(name)
}

/// Outcome of one extraction pass.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Extraction {
    /// Files written this pass (first run, or healed).
    pub written: usize,
    /// Files already present and size-matching — untouched.
    pub present: usize,
    /// Writes that failed or did not digest-verify back.
    pub failed: usize,
}

impl Extraction {
    pub fn any_failure(&self) -> bool {
        self.failed > 0
    }
}

fn sha256_hex(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(data);
    h.finalize().iter().map(|x| format!("{x:02x}")).collect()
}

/// Extract the whole catalog into `my_shapes` (best-effort, `None`
/// home skips everything): every shipped variant of every
/// [`super::builtin::BUILTINS`] entry, under its exact asset name so
/// the files map 1:1 onto `digests.txt`. A present file whose LENGTH
/// matches is left untouched (the embedded bytes are digest-pinned at
/// build time); a written file is read back and digest-verified
/// against the embedded asset before it counts as written.
pub fn extract_catalog(my_shapes: Option<&Path>) -> Extraction {
    extract_catalog_with(my_shapes, |_, _| {})
}

/// [`extract_catalog`] with a progress callback: `progress(done,
/// total)` fires after every file.
pub fn extract_catalog_with(
    my_shapes: Option<&Path>,
    mut progress: impl FnMut(usize, usize),
) -> Extraction {
    let Some(my_shapes) = my_shapes else {
        return Extraction::default();
    };
    let dir = builtins_dir(my_shapes);
    if std::fs::create_dir_all(&dir).is_err() {
        return Extraction {
            failed: super::builtin::BUILTINS.len(),
            ..Extraction::default()
        };
    }
    let total: usize = super::builtin::BUILTINS
        .iter()
        .map(|b| 1 + usize::from(b.metric_bytes.is_some()))
        .sum();
    let mut done = 0usize;
    let mut out = Extraction::default();
    for b in super::builtin::BUILTINS {
        let variants: &[(&str, &[u8])] = match b.metric_bytes {
            Some(m) => &[("", b.bytes), ("-m", m)],
            None => &[("", b.bytes)],
        };
        for (suffix, bytes) in variants {
            progress(done, total);
            done += 1;
            let target = builtins_dir(my_shapes).join(format!("{}{suffix}.vssx", b.slug));
            // Present and length-matching: untouched (the embedded
            // bytes are digest-pinned at build time by the manifest
            // test; a same-length tamper is out of scope for the
            // cheap pass).
            if std::fs::metadata(&target).is_ok_and(|md| md.len() == bytes.len() as u64) {
                out.present += 1;
                continue;
            }
            let ok = std::fs::write(&target, bytes).is_ok()
                && std::fs::read(&target).is_ok_and(|back| sha256_hex(&back) == sha256_hex(bytes));
            if ok {
                out.written += 1;
            } else {
                out.failed += 1;
            }
        }
    }
    progress(total, total);
    out
}

/// Progress messages from the background extraction (startup
/// feedback, owner report 2026-10-30: the first-run 43 MB write must
/// never block or silence the app).
pub enum ExtractionMsg {
    /// Files handled so far / total files.
    Progress { done: usize, total: usize },
    /// The pass finished.
    Done(Extraction),
}

/// A running background extraction.
pub struct ExtractionHandle {
    rx: std::sync::mpsc::Receiver<ExtractionMsg>,
}

impl ExtractionHandle {
    /// Drain everything ready right now; the LAST message wins
    /// (progress supersedes progress), `None` when nothing new.
    pub fn poll(&self) -> Option<ExtractionMsg> {
        let mut last = None;
        while let Ok(msg) = self.rx.try_recv() {
            last = Some(msg);
        }
        last
    }
}

/// [`extract_catalog_with`] on a background thread with progress
/// messages. `None` when there is no My Shapes directory to extract
/// into. Partial files are safe: `open_from_disk` falls back to the
/// embedded bytes whenever a file fails to read or parse.
pub fn extract_catalog_async(my_shapes: Option<PathBuf>) -> Option<ExtractionHandle> {
    let dir = my_shapes?;
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let out = extract_catalog_with(Some(&dir), |done, total| {
            let _ = tx.send(ExtractionMsg::Progress { done, total });
        });
        let _ = tx.send(ExtractionMsg::Done(out));
    });
    Some(ExtractionHandle { rx })
}

/// Open a built-in from its extracted file: the variant's bytes read
/// from disk and parsed, with the BASE variant's path as the
/// source/dedupe identity. `None` when anything is missing or fails —
/// callers fall back to the embedded bytes and the `builtin/<slug>`
/// pseudo-path.
pub fn open_from_disk(
    my_shapes: Option<&Path>,
    b: &BuiltinStencil,
    metric: bool,
) -> Option<(vsdx::Document, PathBuf)> {
    let my_shapes = my_shapes?;
    let bytes = std::fs::read(variant_path(my_shapes, b, metric)).ok()?;
    let doc = vsdx::parse::parse_bytes(&bytes).ok()?;
    Some((doc, extracted_path(my_shapes, b.slug)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stencil::builtin;

    /// Extraction writes every shipped variant under its asset name,
    /// byte-identical to the embedded catalog; a second pass writes
    /// nothing; a tampered file heals from the embedded bytes.
    #[test]
    fn extraction_is_idempotent_and_self_healing() {
        let tmp = tempfile::tempdir().unwrap();
        let my_shapes = tmp.path();

        let first = extract_catalog(Some(my_shapes));
        assert_eq!(first.failed, 0, "nothing fails on a writable dir");
        assert_eq!(first.present, 0, "first run writes everything");
        assert_eq!(
            first.written,
            builtin::BUILTINS
                .iter()
                .map(|b| 1 + usize::from(b.metric_bytes.is_some()))
                .sum::<usize>(),
            "one file per shipped variant"
        );
        // Byte-identical to the embedded assets, under the exact
        // asset names (digests.txt-compatible).
        for b in builtin::BUILTINS {
            for (name, bytes) in [
                (format!("{}.vssx", b.slug), b.bytes),
                (format!("{}-m.vssx", b.slug), b.metric_bytes.unwrap_or(&[])),
            ] {
                if bytes.is_empty() {
                    continue;
                }
                let on_disk =
                    std::fs::read(builtins_dir(my_shapes).join(&name)).unwrap_or_default();
                assert_eq!(on_disk, bytes, "{name} extracted byte-identical");
            }
        }

        // Second pass: everything present and length-matching.
        let second = extract_catalog(Some(my_shapes));
        assert_eq!(
            (second.written, second.present, second.failed),
            (0, first.written, 0),
            "an extracted catalog is left untouched"
        );

        // Tamper (truncate) one file: the pass heals it from the
        // embedded asset.
        let victim = extracted_path(my_shapes, "basic");
        std::fs::write(&victim, b"tampered").unwrap();
        let healed = extract_catalog(Some(my_shapes));
        assert_eq!(healed.written, 1, "the tampered file was rewritten");
        assert_eq!(
            std::fs::read(&victim).unwrap(),
            builtin::by_slug("basic").unwrap().bytes,
            "healed back to the embedded bytes"
        );
    }

    /// The background pass completes and reports; the callback form
    /// fires per file and lands on (total, total).
    #[test]
    fn extraction_reports_progress() {
        let tmp = tempfile::tempdir().unwrap();
        let h = extract_catalog_async(Some(tmp.path().to_path_buf())).expect("handle");
        let done = loop {
            match h.poll() {
                Some(ExtractionMsg::Done(e)) => break e,
                Some(ExtractionMsg::Progress { .. }) => {}
                None => std::thread::sleep(std::time::Duration::from_millis(5)),
            }
        };
        assert!(done.written > 0, "the async pass wrote the catalog");
        // Drained after Done (channel closed).
        assert!(h.poll().is_none());

        let tmp2 = tempfile::tempdir().unwrap();
        let mut last = (0usize, 0usize);
        let e = extract_catalog_with(Some(tmp2.path()), |d, t| last = (d, t));
        let files = e.written + e.present + e.failed;
        assert_eq!(last, (files, files), "progress lands on (total, total)");
    }

    /// No home directory: nothing to do, nothing fails loudly.
    #[test]
    fn extraction_without_my_shapes_is_a_noop() {
        let e = extract_catalog(None);
        assert_eq!(e, Extraction::default());
    }

    /// Disk-first opens parse the extracted file and carry the base
    /// variant's path as identity; a missing file yields None (the
    /// embedded fallback path).
    #[test]
    fn open_from_disk_parses_the_extracted_file() {
        let tmp = tempfile::tempdir().unwrap();
        let my_shapes = tmp.path();
        extract_catalog(Some(my_shapes));

        let b = builtin::by_slug("basic").expect("basic ships");
        let (doc, source) =
            open_from_disk(Some(my_shapes), b, false).expect("extracted basic opens");
        assert_eq!(source, extracted_path(my_shapes, "basic"));
        let embedded = vsdx::parse::parse_bytes(b.bytes).unwrap();
        assert_eq!(doc.masters.len(), embedded.masters.len());
        // The metric twin opens its own file under the SAME identity.
        let (metric_doc, metric_source) =
            open_from_disk(Some(my_shapes), b, true).expect("metric twin opens");
        assert_eq!(metric_source, source, "variant-independent identity");
        assert_eq!(metric_doc.masters.len(), embedded.masters.len());
        // A not-yet-extracted catalog yields None.
        let elsewhere = tmp.path().join("elsewhere");
        assert!(open_from_disk(Some(&elsewhere), b, false).is_none());
    }
}
