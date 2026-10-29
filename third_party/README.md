# Vendored dependencies

## craft-ui

Vendored from [storytold/craft-libs](https://github.com/storytold/craft-libs)
at commit `88d245f` (branch `egui-0.37-diagramcraft`, the fork that bumps
egui to 0.37 for DiagramCraft). Standalone manifest; the crate's own
MIT/Apache-2.0 license texts sit beside its sources. Upstream tests were
not vendored (they run in the fork).

The rest of craft-libs (`craft-atomic`, used for crash-safe saves) is
consumed as a git dependency — see `crates/dc-core/Cargo.toml`.

### Maintenance policy (owner decision 2026-10-28, #81 option B)

DiagramCraft **stays on egui 0.37 and keeps this fork**; upstream
craft-libs still pins egui 0.36 and its own 0.37 move (via the
`storytold/egui` fork) has not been published. Until it is:

- **Monitor, don't chase**: check upstream `craft-libs` for its 0.37
  publication (issue #81 is the open tracker — it closes when the
  migration lands upstream). When upstream publishes, re-vendor from
  upstream main and drop this fork, taking its docking/print work
  with it; that replaces any local rebase.
- **No hand-rebases of upstream main onto the fork** in the meantime:
  the fork is a pin, not a living branch. If an upstream fix is
  genuinely needed before the 0.37 publication, cherry-pick the
  minimal commit and note it here.
- Our consumption is intentionally thin (one type today —
  `craft_ui::theme::WidgetPalette`); keep it that way until the
  re-vendor, so the eventual swap stays near-zero-risk. New craft-ui
  adoptions before then should weigh the re-vendor cost first.
- The `Id::new` → `Id::unique` migration on our side is DONE (the
  six deprecation shims are cleared); nothing 0.37-side is parked.
