# Open Sans (chrome font)

Chrome/UI typeface for the Tarahi shell (ribbon, dialogs, status bar) —
the ONLYOFFICE suite's UI font, adopted with the #50/#252 ribbon
fidelity wave (owner direction, 2026-11-09).

- Source: Google Fonts (https://fonts.google.com/specimen/Open+Sans),
  static build (v3.x), files `OpenSans-Regular.ttf` (400) and
  `OpenSans-SemiBold.ttf` (600), latin coverage.
- License: SIL Open Font License 1.1 — free to vendor, redistribute,
  and embed; see OFL.txt. Open Sans is a Google/Steve Matteson
  typeface and is NOT part of the AGPL-licensed ONLYOFFICE code (the
  suite's AGPL applies to its code/icons, not this font).
- Registered as the egui `Proportional` family's first face in
  `dc-ui/src/theme.rs` (`install_fonts`). Canvas/document text metrics
  are untouched — they stay on the vendored Liberation/Carlito stack
  (`dc/assets/fonts`) so .vsdx text measurement and rasterization are
  unchanged.

Apache-2.0 MDI icons and this font are the only third-party UI assets;
no Microsoft content, no ONLYOFFICE code or icon files.
