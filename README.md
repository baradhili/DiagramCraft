# DiagramCraft

A Linux-native reimplementation of the Visio 2016 drawing experience in
Rust: read and write `.vsdx`/`.vstx`/`.vssx` packages losslessly, render
them to SVG/PNG, edit them through an undoable command layer, and work in
a native desktop GUI with the Visio shell (ribbon, stencils, ShapeSheet).

Built clean-room: from recorded file-format and application-behavior
observations. No Microsoft code, content, or assets.

Using: [GitHub - morluto/rea: Reverse engineer anything with agents, from app behavior down to native binaries. · GitHub](https://github.com/morluto/rea) and zcode

## Helping

Right now it is pretty raw AI generated code and UI.. yes it suck and yes it does only a fraction of what it should do - but hey it does manage to mostly draw vsdx correctly!

How can you help?

1. Create issues - it gives me and my agent a way to track what could be fixed

2. Create PR - awesome! - but please keep the PRs on a narrow feature (no 10000 line PRs please!) and make sure they are at least checked by CodeRabbit - or better Greptile

3. Spread the word!

## Workspace

- **`crates/vsdx`** — the VSDX-family format library: OPC package access,
  XML parsing into a typed document model, master/stylesheet inheritance
  (`F='Inh'`, row-cell and per-section inheritance), a ShapeSheet formula
  evaluator subset, and lossless writing (canonical-XML round-trip).
- **`crates/dc`** — the CLI: `render` (SVG backend with themes, gradients,
  Bézier/arc geometry, images, wrapped text, connector arrowheads),
  `info` / `inspect`, `roundtrip` (re-write + verify), and `edit`
  (undoable cell/text/shape edits).
- **`crates/dc-core`** — the editing core: `Command` trait, undo/redo
  history, scope addressing (`page:1/shape:5`), crash-safe saves.
- **`crates/dc-ui`** — the native desktop GUI (egui/eframe): Visio-2016
  shell with ribbon/QAT/status bar, canvas with rulers/grid/zoom, stencil
  pane with master thumbnails, drag-to-page placement, right-click
  context menus (including master Actions), ShapeSheet inspector, and
  headless kittest pixel/interaction tests.
- **`third_party/craft-ui`** — vendored egui widget library (see its
  README for provenance).

## Build & test

```sh
cargo build --workspace
cargo test --workspace
```

## Usage

```sh
# Inspect and render a drawing
cargo run -p dc -- info    path/to/file.vsdx
cargo run -p dc -- render  path/to/file.vsdx -o out.svg
cargo run -p dc -- render  path/to/file.vssx -o sheet.svg --masters

# Lossless round-trip with verification
cargo run -p dc -- roundtrip path/to/file.vsdx -o out.vsdx --verify

# Undoable edits through the command layer
cargo run -p dc -- edit path/to/file.vsdx -o out.vsdx \
    --cell page:0/shape:1 LineWeight 0.02 --text page:0/shape:1 "label"

# Native GUI
cargo run -p dc-ui -- path/to/file.vsdx
```

## GUI tests

Headless via egui_kittest (snapshots in `crates/dc-ui/tests/snapshots/`,
regenerate with `UPDATE_SNAPSHOTS=1`):

```sh
cargo test -p dc-ui --test shell
cargo test -p dc-ui --test interactions
```

## Installers

Release tags (`v*`) build installers through GitHub Actions
(`.github/workflows/release.yml`): a Windows NSIS installer (+ portable zip), a
universal macOS dmg, and Linux deb / rpm / AppImage packages. Push a tag
to cut a release:

```sh
git tag v0.2.0 && git push origin v0.2.0
```

Packaging metadata lives in the `dc-ui` manifest (`[package.metadata.deb]`,
`[package.metadata.generate-rpm]`), `packaging/installer.nsi` for the NSIS installer, and
`packaging/` for the desktop entry, icons, and macOS Info.plist.

## Commit conventions

Conventional Commits, enforced by [cargo-commitlint] through the tracked
`commit-msg` hook in `.githooks/` (rules in `commitlint.toml`). One-time
setup per clone:

```sh
cargo install cargo-commitlint --version 2.1.0 --locked
git config core.hooksPath .githooks
```

Bypass a single commit with `COMMITLINT_SKIP=1`.

## Roadmap

### M6 — Ecosystem - current

- Data linking (`data` crate): CSV/SQLite recordsets → shape data (URS category "Data Linking"; spec'd in VeeTrace).

- Validation rules; org-chart/auto-layout (`layout` crate — cleanroom  analog of `@Visgrf` concepts); export filters (SVG/PNG/PDF).

- Legacy `.vsd` reader: OLE compound + `VisioDocument` stream reversal —
  
  needs deeper RE (the `CDMConvert*` inventory in VISLIB marks the
  
  migration seams to study).

- The "scripting/automation facade" formerly listed here has grown into
  
  M7/M8 below (owner direction 2026-10-16).

### M7 — VBA / `.vsdm` macro execution (planned)

- **`.vsdm` package support**: recognize/round-trip `.vsdm` losslessly

- (`vbaProject.bin` carried as a verbatim unmodeled part — the existing writer already preserves unknown parts; needs content-type/signature awareness and the macro-flag document settings).

- **VBA project reader**: OLE compound parse of `vbaProject.bin` + MS-OVBA decompression → macro inventory (module list, names) shown in a Developer-tab macro dialog.

- **VBA interpreter subset**: cleanroom execution of the practical macro subset (the same subset strategy as the ShapeSheet formula evaluator), driven against the M8 automation object model below.

- **Macro security model** (URS-194): enable/disable per document, trust prompts, `.vsdx` (macro-free) vs `.vsdm` save distinction.

### M8 — Real Visio add-in hosting (planned, spike-gated)

  Goal: run real Visio add-ins, not reimplement them. First target:

  `Sample/graphvisio` — GraphVizio, a 2014 x86 Windows MSI Visio add-in

  for Graphviz layout (untracked sample, like all of `Sample/`).

- **Feasibility spike (gate)**: extract the MSI, identify the add-in binary type (COM DLL / VSL / .NET) and enumerate the Visio object model surface it touches (deep RE: REA + Ghidra on the extracted binary; add to the deep-RE queue).

- **Automation object model**: a cleanroom `Application`/`Document`/ `Page`/`Shape` facade over `dc-core` (the surface COM add-ins automate) — designed with M7 step 3 so macros and add-ins share it.

- **Host/loader**: whichever of (a) a Win32 COM emulation host that can load and service the real DLL on Linux, or (b) a Wine-side host with a bridge to the native core, the spike shows to be viable. If neither is viable, fall back to a compatibility plugin API (M9) plus per-addin reimplementation guides — decided at the gate, not before.

### M9 — Plugin system + diagram-language imports (nice-to-have)

  Plugins, not core (owner direction 2026-10-16). Rides on the M8   automation facade or a lighter native plugin API if M8 lands after it.

- Plugin architecture: out-of-tree diagram sources, shape mappers, and command packs with a stable API over `dc-core`.

- Language import-renderers with **configurable shape mapping** (bind language node/edge types to masters/stencil shapes): **d2**, **Graphviz DOT** (synergy: GraphVizio is a Graphviz add-in — 
  
  its public behavior is a useful oracle), and **Mermaid**.

## License

MIT OR Apache-2.0 (see `LICENSE-MIT` / `LICENSE-APACHE`). GUI icons are
the vendored Pictogrammers Material Design Icons set (Apache-2.0), in
`crates/dc-ui/assets/icons/` with their license and provenance notes.

[cargo-commitlint]: https://crates.io/crates/cargo-commitlint
