//! dc-ui — the Tarahi desktop application (M5): a standalone
//! native GUI in the Visio 2016 idiom (ribbon shell, stencil pane,
//! metric rulers) rendering through the SVG backend and editing through
//! the dc-core command layer. No web stack.

pub mod align;
pub mod app;
pub mod canvas;
pub mod cmd;
pub mod connectors;
pub mod export;
pub mod freeform;
#[doc(hidden)]
pub mod group_fixture;
pub mod guides;
pub mod hyperlink;
pub mod icontex;
pub mod i18n;
pub mod keytips;
/// Generated fluent-typed accessors (`build.rs` from the project-level
/// `locales/` catalogs); re-exported through [`i18n`]. #246.
#[allow(clippy::all)]
mod l10n;
mod layers;
pub mod loading;
pub mod os_theme;
pub mod perf;
pub mod props;
pub mod quickshapes;
mod rasterpool;
pub mod recent;
pub mod ribbon;
pub mod shapesheet;
pub mod snap;
pub mod stencil;
pub mod svgimg;
pub mod textedit;
pub mod theme;
pub mod units;
