# Built-in stencil catalog assets (GitHub #87)

Synthetic `.vssx` stencils embedded as the app's built-in More Shapes
catalog (`crates/dc-ui/src/stencil/builtin.rs` — the code module is the
catalog's single source of truth for slug/category/title/embedded
bytes).

## Provenance

The masters inside these files are copied from the **shipped Visio 2016
stencils** (`Visio Content/1033`, US `_U` variants) under the scoped
owner exception of **2026-10-25**, recorded in the repository's
`AGENTS.md` next to the original 2026-10-19 cleanroom policy: masters
extracted from the shipped stencils may be copied into this repository
as built-in stencil assets, repackaged into synthetic `.vssx` groupings
of our own. Everything else Microsoft (UI icons, artwork, code,
binaries) remains excluded. The `Sample/Office16/` install itself is
**untracked** and is never committed.

## Derivation

Each file is one source stencil, re-packaged: the parsed source
document cloned and pruned (pages dropped, hidden masters dropped,
stylesheets/colors/document items and every kept master verbatim) and
written through `vsdx::write`'s fresh path. Model round-trip is
provably lossless for these files — the `dc roundtrip --verify` suite
shows parse → re-serialize is canonically identical across all 679
shipped templates/stencils. One source file per catalog entry keeps
style inheritance coherent.

`manifest.txt` maps each catalog entry (slug | category | display
title | source file). Regenerating requires the local Visio 2016
install and re-runs deterministically (byte-identical output):

```sh
cargo run -p vsdx --example extract-stencils -- \
  "<path to>/Visio Content/1033" \
  crates/dc-ui/assets/stencils/manifest.txt \
  crates/dc-ui/assets/stencils
```

`digests.txt` records the SHA-256 of each asset at extraction; the
dc-ui test suite verifies the committed bytes against it, so the
repository's copies are tamper-evident.

## Current entries

| file | category | title | source | masters |
|---|---|---|---|---|
| general-basic-shapes.vssx | General | Basic Shapes | BASIC_U.VSSX | 48 |
| accessories-containers.vssx | Accessories | Containers | SDCONT_U.VSSX | 14 |
| accessories-callouts.vssx | Accessories | Callouts | SDCALL_U.VSSX | 14 |

The initial set follows the shipped auto-opened companion stencils
(REA evidence: VISLIB.DLL references Basic, containers, callouts among
the nine auto-opened stencils). Expanding the catalog is a
`manifest.txt` edit plus a re-run of the extractor.
