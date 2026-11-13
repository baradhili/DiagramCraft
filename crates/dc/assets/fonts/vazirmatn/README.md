# Vendored UI font: Vazirmatn (Persian/Arabic complex script)

Source: Vazirmatn v33.003 via cdn.jsdelivr.net (npm `vazirmatn`,
`fonts/ttf/*.ttf`), retrieved 2026-11-11 for #257 item 2.
Upstream: https://github.com/rastikerdar/vazirmatn
License: SIL Open Font License 1.1 (see OFL.txt beside the files).

| file | face |
|---|---|
| Vazirmatn-Regular.ttf | Vazirmatn Regular |
| Vazirmatn-Bold.ttf | Vazirmatn Bold |

Loaded into egui as the FALLBACK face of the proportional and
monospace families (after the Latin OpenSans stack), so
Persian/Arabic glyphs render without touching Latin metrics, and
registered into the measurement/raster font databases by
`dc::register_complex_script_faces` (called by the dc CLI's main and
dc-ui's `theme::install_fonts` — the files moved from dc-ui's assets
in PR #269 so the CLI shares the one drop). Never extract fonts from
Microsoft products.
