//! dc-ui — the DiagramCraft desktop application (M5): a standalone
//! native GUI in the Visio 2016 idiom (ribbon shell, stencil pane,
//! metric rulers) rendering through the SVG backend and editing through
//! the dc-core command layer. No web stack.

pub mod app;
pub mod canvas;
pub mod cmd;
pub mod icontex;
pub mod os_theme;
pub mod perf;
pub mod props;
mod rasterpool;
pub mod recent;
pub mod ribbon;
pub mod stencil;
pub mod svgimg;
pub mod theme;
pub mod units;
