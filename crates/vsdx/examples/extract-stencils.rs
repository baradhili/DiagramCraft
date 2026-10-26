//! Built-in stencil catalog extractor (GitHub #87, owner decision
//! 2026-10-25).
//!
//! Reads the shipped Visio 2016 stencils under the local, untracked
//! `Sample/Office16/Visio Content/1033` directory and rewrites selected
//! masters into **synthetic `.vssx` stencils of our own** — one output
//! file per catalog entry, grouped per `manifest.txt`.
//!
//! Method — model round-trip, not part-level copy: each output clones
//! the *parsed source document* and prunes it (drop pages, drop hidden
//! masters, keep the source's stylesheets/colors/document items and
//! every kept master verbatim, including its `<Rel>` binding), then
//! saves through the ordinary `vsdx::write` fresh path. That path is
//! provably lossless for these files: the `dc roundtrip --verify`
//! suite shows parse → re-serialize is canonically identical across
//! all 679 shipped templates/stencils, stylesheets included. One
//! source file per catalog entry keeps style inheritance coherent —
//! merging masters from several sources into one stencil would have
//! to merge their stylesheets too.
//!
//! Output is deterministic (BTreeMap part ordering, canonical
//! serialization): re-running reproduces byte-identical files. The
//! committed assets carry a digest record (`digests.txt`) verified by
//! a dc-ui test, so the repository's copies are tamper-evident.
//!
//! Usage (from `diagramcraft/`):
//!
//! ```text
//! cargo run -p vsdx --example extract-stencils -- \
//!     "<path>/Visio Content/1033" \
//!     crates/dc-ui/assets/stencils/manifest.txt \
//!     crates/dc-ui/assets/stencils
//! ```
//!
//! Provenance policy: copying masters out of the shipped Visio 2016
//! stencils into this repository as built-in stencil assets is the
//! scoped owner exception of 2026-10-25 (recorded in AGENTS.md);
//! everything else Microsoft stays out of the repo.

use std::path::{Path, PathBuf};
use vsdx::model::Document;

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args_os().skip(1);
    let (Some(sample_dir), Some(manifest), Some(out_dir)) = (args.next(), args.next(), args.next())
    else {
        eprintln!(
            "usage: extract-stencils <Visio Content/1033 dir> <manifest.txt> <output dir>"
        );
        std::process::exit(2);
    };
    let sample_dir = PathBuf::from(sample_dir);
    let out_dir = PathBuf::from(out_dir);
    std::fs::create_dir_all(&out_dir)?;

    let mut digests = String::new();
    for (lineno, line) in std::fs::read_to_string(&manifest)?.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        // slug | category | display title | source file in the 1033 dir
        let parts: Vec<&str> = line.split('|').map(str::trim).collect();
        anyhow::ensure!(
            parts.len() == 4,
            "manifest line {}: expected 'slug | category | title | SOURCE.VSSX'",
            lineno + 1
        );
        let (slug, category, title, source) = (parts[0], parts[1], parts[2], parts[3]);
        let src_path = sample_dir.join(source);
        let doc = extract(&src_path)?;
        let out_path = out_dir.join(format!("{slug}.vssx"));
        vsdx::write::save(&doc, &out_path)?;
        let bytes = std::fs::read(&out_path)?;
        use sha2::{Digest, Sha256};
        let mut h = Sha256::new();
        h.update(&bytes);
        let hex: String = h
            .finalize()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        println!(
            "{slug}.vssx  ({category} / {title})  <- {source}: {} masters, {} bytes, sha256 {hex}",
            doc.masters.len(),
            bytes.len()
        );
        digests.push_str(&format!("{hex}  {slug}.vssx\n"));
    }
    std::fs::write(out_dir.join("digests.txt"), digests)?;
    println!("wrote {}", out_dir.join("digests.txt").display());
    Ok(())
}

/// The synthetic stencil for one source file: the parsed source with
/// pages dropped (a stencil needs none), hidden masters dropped, and
/// the retained package removed so the writer takes its fresh path
/// (which synthesizes the index/rels bindings, #85's writer fix).
fn extract(src: &Path) -> anyhow::Result<Document> {
    let mut doc = vsdx::parse::parse(src)?;
    let before = doc.masters.len();
    doc.pages.clear();
    doc.masters.retain(|m| {
        let hidden = m
            .attrs
            .iter()
            .any(|(k, v)| k == "Hidden" && v.trim() != "0");
        !hidden && !m.part.is_empty()
    });
    anyhow::ensure!(!doc.masters.is_empty(), "no visible masters in {src:?}");
    doc.package = None;
    // The masters' own <Rel> rIds and part names survive verbatim, so
    // the fresh-path rels generation and index synthesis reproduce the
    // source's numbering exactly.
    let _ = before;
    Ok(doc)
}
