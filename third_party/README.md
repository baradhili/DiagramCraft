# Vendored dependencies

## craft-ui

Vendored from [storytold/craft-libs](https://github.com/storytold/craft-libs)
at commit `88d245f` (branch `egui-0.37-diagramcraft`, the fork that bumps
egui to 0.37 for DiagramCraft). Standalone manifest; the crate's own
MIT/Apache-2.0 license texts sit beside its sources. Upstream tests were
not vendored (they run in the fork).

The rest of craft-libs (`craft-atomic`, used for crash-safe saves) is
consumed as a git dependency — see `crates/dc-core/Cargo.toml`.
