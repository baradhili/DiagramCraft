//! Icon textures from the vendored MDI (Pictogrammers Material Design
//! Icons, Apache-2.0) SVGs in `assets/icons/`. Icons are tinted by
//! injecting `fill` into the root element and rasterized with the
//! renderer's resvg stack, then cached per (context, icon, color).
//!
//! All artwork is from the MDI open-source set — never extracted from
//! Microsoft products (see assets/icons/README.md).

#![allow(deprecated)] // egui::Id::new is stable-by-string; salt API churns

use std::collections::HashMap;

use egui::{Color32, Context, TextureHandle};

/// Embedded icon SVGs, keyed by asset stem.
fn svg(name: &str) -> Option<&'static str> {
    Some(match name {
        "save" => include_str!("../assets/icons/save.svg"),
        "undo" => include_str!("../assets/icons/undo.svg"),
        "redo" => include_str!("../assets/icons/redo.svg"),
        "open" => include_str!("../assets/icons/open.svg"),
        "new" => include_str!("../assets/icons/new.svg"),
        "paste" => include_str!("../assets/icons/paste.svg"),
        "cut" => include_str!("../assets/icons/cut.svg"),
        "copy" => include_str!("../assets/icons/copy.svg"),
        "painter" => include_str!("../assets/icons/painter.svg"),
        "fonta" => include_str!("../assets/icons/fonta.svg"),
        "bold" => include_str!("../assets/icons/bold.svg"),
        "italic" => include_str!("../assets/icons/italic.svg"),
        "underline" => include_str!("../assets/icons/underline.svg"),
        "strike" => include_str!("../assets/icons/strike.svg"),
        "subscript" => include_str!("../assets/icons/subscript.svg"),
        "superscript" => include_str!("../assets/icons/superscript.svg"),
        "textcolor" => include_str!("../assets/icons/textcolor.svg"),
        "grow" => include_str!("../assets/icons/grow.svg"),
        "shrink" => include_str!("../assets/icons/shrink.svg"),
        "clear" => include_str!("../assets/icons/clear.svg"),
        "numbering" => include_str!("../assets/icons/numbering.svg"),
        "indentless" => include_str!("../assets/icons/indentless.svg"),
        "indentmore" => include_str!("../assets/icons/indentmore.svg"),
        "linespacing" => include_str!("../assets/icons/linespacing.svg"),
        "effects" => include_str!("../assets/icons/effects.svg"),
        "bucket" => include_str!("../assets/icons/bucket.svg"),
        "pen" => include_str!("../assets/icons/pen.svg"),
        "bringfront" => include_str!("../assets/icons/bringfront.svg"),
        "sendback" => include_str!("../assets/icons/sendback.svg"),
        "align" => include_str!("../assets/icons/align.svg"),
        "rotate" => include_str!("../assets/icons/rotate.svg"),
        "group" => include_str!("../assets/icons/group.svg"),
        "container" => include_str!("../assets/icons/container.svg"),
        "callout" => include_str!("../assets/icons/callout.svg"),
        "field" => include_str!("../assets/icons/field.svg"),
        "comment" => include_str!("../assets/icons/comment.svg"),
        "spell" => include_str!("../assets/icons/spell.svg"),
        "datagraphic" => include_str!("../assets/icons/datagraphic.svg"),
        "link" => include_str!("../assets/icons/link.svg"),
        "refresh" => include_str!("../assets/icons/refresh.svg"),
        "swimlane" => include_str!("../assets/icons/swimlane.svg"),
        "check" => include_str!("../assets/icons/check.svg"),
        "picture" => include_str!("../assets/icons/picture.svg"),
        "onlinepic" => include_str!("../assets/icons/onlinepic.svg"),
        "orientation" => include_str!("../assets/icons/orientation.svg"),
        "pointer" => include_str!("../assets/icons/pointer.svg"),
        "textbox" => include_str!("../assets/icons/textbox.svg"),
        "newpage" => include_str!("../assets/icons/newpage.svg"),
        "theme" => include_str!("../assets/icons/theme.svg"),
        "fit" => include_str!("../assets/icons/fit.svg"),
        "zoomin" => include_str!("../assets/icons/zoomin.svg"),
        "zoomout" => include_str!("../assets/icons/zoomout.svg"),
        "grid" => include_str!("../assets/icons/grid.svg"),
        "rulers" => include_str!("../assets/icons/rulers.svg"),
        "shapes" => include_str!("../assets/icons/shapes.svg"),
        "macro" => include_str!("../assets/icons/macro.svg"),
        "shapesheet" => include_str!("../assets/icons/shapesheet.svg"),
        "units" => include_str!("../assets/icons/units.svg"),
        "delete" => include_str!("../assets/icons/delete.svg"),
        "lightbulb" => include_str!("../assets/icons/lightbulb.svg"),
        _ => return None,
    })
}

/// Tint an MDI svg: MDI paths inherit the root `fill` (or use
/// `currentColor` in some distributions) — set both to the target.
fn tint(source: &str, color: Color32) -> String {
    let hex = format!("#{:02X}{:02X}{:02X}", color.r(), color.g(), color.b());
    let s = source.replace("currentColor", &hex);
    if let Some(rest) = s.strip_prefix("<svg ") {
        format!("<svg fill=\"{hex}\" {rest}")
    } else {
        s
    }
}

/// Per-context texture cache lives in egui's own memory (TypeMap), so
/// it is bound to the context's lifetime automatically — important
/// because kittest builds a fresh context per test.
type IconCache = HashMap<(&'static str, u32), TextureHandle>;

/// Raster size for icon textures (crisp at hero 30 px and 2× DPI).
const ICON_PX: f32 = 96.0;

/// The texture for `name` tinted with `color`, building it on first
/// use in this context. `None` when the icon asset is unknown or the
/// rasterizer fails (callers fall back to the epaint glyph).
pub fn texture(ctx: &Context, name: &str, color: Color32) -> Option<TextureHandle> {
    // the match table gives us the &'static asset stem for the cache key
    let name: &'static str = svg(name)?;
    let rgba = color.to_srgba_unmultiplied();
    let key = (
        name,
        (rgba[0] as u32) << 24 | (rgba[1] as u32) << 16 | (rgba[2] as u32) << 8 | rgba[3] as u32,
    );
    let hit = ctx.memory(|m| {
        m.data
            .get_temp::<IconCache>(egui::Id::new("dc-icon-tex"))
            .and_then(|c| c.get(&key).cloned())
    });
    if let Some(t) = hit {
        return Some(t);
    }
    let source = svg(name)?;
    let tinted = tint(source, color);
    let img = crate::svgimg::rasterize(&tinted, ICON_PX).ok()?;
    let handle = ctx.load_texture(format!("icon-{name}"), img, egui::TextureOptions::LINEAR);
    ctx.memory_mut(|m| {
        m.data
            .get_temp_mut_or_default::<IconCache>(egui::Id::new("dc-icon-tex"))
            .insert(key, handle.clone());
    });
    Some(handle)
}
