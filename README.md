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
(`.github/workflows/release.yml`): a Windows MSI (+ portable zip), a
universal macOS dmg, and Linux deb / rpm / AppImage packages. Push a tag
to cut a release:

```sh
git tag v0.2.0 && git push origin v0.2.0
```

Packaging metadata lives in the `dc-ui` manifest (`[package.metadata.deb]`,
`[package.metadata.generate-rpm]`), `crates/dc-ui/wix/` for the MSI, and
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

## License

MIT OR Apache-2.0 (see `LICENSE-MIT` / `LICENSE-APACHE`). GUI icons are
the vendored Pictogrammers Material Design Icons set (Apache-2.0), in
`crates/dc-ui/assets/icons/` with their license and provenance notes.

[cargo-commitlint]: https://crates.io/crates/cargo-commitlint
