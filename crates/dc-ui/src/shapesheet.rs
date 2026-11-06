//! The per-shape ShapeSheet inspector window body (#189) — Visio 2016
//! style and FULL structural coverage.
//!
//! Two things the issue pinned, both reimplemented from observed
//! Visio 2016 behavior (cleanroom: screenshots of the shipped app,
//! no Microsoft assets):
//!
//! **All of the structure.** Visio's ShapeSheet shows the *resolved*
//! sheet — the instance's own cells merged with its inheritance chain
//! (master source → style cascade → document sheet). An instanced
//! shape with an EMPTY local sheet (a container part carrying only
//! `Master`/`MasterShape`, like the corpus's SDCONT instances) still
//! shows its full sheet. Top-level `<Cell>` elements are grouped into
//! Visio's *display* sections (Shape Transform, Protection, Line
//! Format, Theme Properties, …) — those sections do not exist in the
//! XML; only Character / Paragraph / Tabs / User / Connection /
//! Geometry / … are real `<Section>` elements. Canonical cells that
//! resolve nowhere display Visio's observed defaults ("0" locks,
//! "FALSE" toggles, theme indices 65534, …) or "No Formula".
//!
//! **The layout.** Sections stack in Visio's order, each under a blue
//! title bar: fixed sections render name+value pairs flowing 3-5 per
//! row (Shape Transform uses Visio's column layout); row sections
//! render a header row of cell names, a gray row-label column (row
//! number, or the row name in red for named sections), and one
//! value-or-formula per cell — the formula text when the cell has
//! one, else the value with its unit. Cells inherited from deeper in
//! the chain are dimmed.
//!
//! No editing here — the panel is a read-only view over
//! `vsdx::model`'s resolution (`Document::resolve_cell`,
//! `Document::instance_source`). The view rebuilds each frame; the
//! walks are a few hundred map scans, far under one millisecond.

use dc_core::scope::shape_chain;
use egui::{Color32, Label, RichText, Sense, Ui, Vec2};
use vsdx::model::{Cell, Document, Row, Section, Shape, Sheet, SheetChild};

// ---------------------------------------------------------------------------
// The canonical section catalog (Visio display order)
// ---------------------------------------------------------------------------

/// Shape Transform's column layout — Visio shows the sheet's top-level
/// placement cells as three column groups, read top-down.
const SHAPE_TRANSFORM_COLS: [&[&str]; 3] = [
    &["Width", "Height", "Angle"],
    &["PinX", "PinY", "LocPinX", "LocPinY"],
    &["FlipX", "FlipY", "ResizeMode"],
];

/// Fixed (pairs) sections: (display title, cells in Visio's display
/// order — row-major as the ShapeSheet wraps them — pairs per row).
/// Shape Transform is handled separately (column layout).
const PAIRS_SECTIONS: &[(&str, &[&str], usize)] = &[
    (
        "Protection",
        &[
            "LockWidth",
            "LockEnd",
            "LockCrop",
            "LockHeight",
            "LockDelete",
            "LockGroup",
            "LockAspect",
            "LockSelect",
            "LockCalcWH",
            "LockMoveX",
            "LockFormat",
            "LockFromGroupFormat",
            "LockMoveY",
            "LockCustProp",
            "LockThemeColors",
            "LockRotate",
            "LockTextEdit",
            "LockThemeEffects",
            "LockBegin",
            "LockVtxEdit",
            "LockThemeConnectors",
            "LockReplace",
            "LockThemeIndex",
            "LockThemeFonts",
        ],
        3,
    ),
    (
        "Miscellaneous",
        &[
            "NoObjHandles",
            "HideText",
            "ObjType",
            "NoCtlHandles",
            "UpdateAlignBox",
            "IsDropSource",
            "NoAlignBox",
            "DynFeedback",
            "Comment",
            "NonPrinting",
            "NoLiveDynamics",
            "DropOnPageScale",
            "LangID",
            "Calendar",
            "LocalizeMerge",
            "NoPrinting",
        ],
        3,
    ),
    (
        "Group Properties",
        &[
            "SelectMode",
            "IsTextEditTarget",
            "IsDropTarget",
            "DisplayMode",
            "IsSnapTarget",
            "DontMoveChildren",
        ],
        3,
    ),
    (
        "Line Format",
        &[
            "LinePattern",
            "BeginArrow",
            "BeginArrowSize",
            "LineWeight",
            "EndArrow",
            "EndArrowSize",
            "LineColor",
            "LineColorTrans",
            "Rounding",
            "LineCap",
            "CompoundType",
        ],
        3,
    ),
    (
        "Fill Format",
        &[
            "FillForegnd",
            "ShdwForegnd",
            "ShapeShdwType",
            "FillForegndTrans",
            "ShdwForegndTrans",
            "ShapeShdwObliqueAngle",
            "FillBkgnd",
            "ShdwPattern",
            "ShapeShdwScaleFactor",
            "FillBkgndTrans",
            "ShapeShdwOffsetX",
            "ShapeShdwBlur",
            "FillPattern",
            "ShapeShdwOffsetY",
            "ShapeShdwShow",
        ],
        3,
    ),
    (
        "Text Block Format",
        &[
            "LeftMargin",
            "TopMargin",
            "TextDirection",
            "RightMargin",
            "BottomMargin",
            "VerticalAlign",
            "TextBkgnd",
            "TextBkgndTrans",
            "DefaultTabStop",
        ],
        3,
    ),
    (
        "Events",
        &[
            "TheData",
            "EventDblClick",
            "EventDrop",
            "TheText",
            "EventXFMod",
            "EventMultiDrop",
        ],
        3,
    ),
    (
        "Image Properties",
        &[
            "Contrast",
            "Gamma",
            "Sharpen",
            "Brightness",
            "Blur",
            "Denoise",
            "Transparency",
        ],
        3,
    ),
    (
        "Glue Info",
        &["BegTrigger", "GlueType", "WalkPreference", "EndTrigger"],
        3,
    ),
    (
        "Shape Layout",
        &[
            "ShapePermeableX",
            "ShapePermeableY",
            "ShapePermeablePlace",
            "ShapeFixedCode",
            "ShapePlowCode",
            "ShapeRouteStyle",
            "ConLineJumpDirX",
            "ConLineJumpDirY",
            "ConFixedCode",
            "ConLineJumpCode",
            "ConLineJumpStyle",
            "ShapeSplit",
            "ShapePlaceFlip",
            "ConLineRouteExt",
            "ShapeSplitable",
            "ShapePlaceStyle",
            "DisplayLevel",
            "Relationships",
        ],
        3,
    ),
    (
        "Gradient Properties",
        &[
            "LineGradientDir",
            "LineGradientAngle",
            "FillGradientDir",
            "FillGradientAngle",
            "LineGradientEnabled",
            "FillGradientEnabled",
            "RotateGradientWithShape",
            "UseGroupGradient",
        ],
        3,
    ),
    (
        "Additional Effect Properties",
        &[
            "ReflectionTrans",
            "ReflectionSize",
            "ReflectionDist",
            "ReflectionBlur",
            "SketchEnabled",
            "SketchSeed",
            "SketchAmount",
            "SketchLineWeight",
            "SketchLineChange",
            "SketchFillChange",
            "GlowColor",
            "GlowColorTrans",
            "GlowSize",
            "SoftEdgesSize",
        ],
        3,
    ),
    (
        "Bevel Properties",
        &[
            "BevelTopType",
            "BevelTopWidth",
            "BevelTopHeight",
            "BevelBottomType",
            "BevelBottomWidth",
            "BevelBottomHeight",
            "BevelDepthColor",
            "BevelDepthSize",
            "BevelContourColor",
            "BevelContourSize",
            "BevelMaterialType",
            "BevelLightingType",
            "BevelLightingAngle",
        ],
        3,
    ),
    (
        "3-D Rotation Properties",
        &[
            "RotationXAngle",
            "RotationYAngle",
            "RotationZAngle",
            "RotationType",
            "Perspective",
            "DistanceFromGround",
            "KeepTextFlat",
        ],
        3,
    ),
    (
        "Theme Properties",
        &[
            "ConnectorSchemeIndex",
            "EffectSchemeIndex",
            "ColorSchemeIndex",
            "FontSchemeIndex",
            "ThemeIndex",
            "VariationColorIndex",
            "VariationStyleIndex",
            "EmbellishmentIndex",
        ],
        5,
    ),
    (
        "Quick Style",
        &[
            "QuickStyleLineMatrix",
            "QuickStyleFillMatrix",
            "QuickStyleEffectsMatrix",
            "QuickStyleLineColor",
            "QuickStyleFillColor",
            "QuickStyleShadowColor",
            "QuickStyleFontColor",
            "QuickStyleFontMatrix",
            "QuickStyleType",
            "QuickStyleVariation",
        ],
        3,
    ),
    (
        "Change Shape Behavior",
        &[
            "ReplaceLockShapeData",
            "ReplaceLockText",
            "ReplaceLockFormat",
            "ReplaceCopyCells",
        ],
        4,
    ),
];

/// A pairs section's rank in the render order (between the real
/// sections it sits among).
fn pairs_rank(title: &str) -> i32 {
    match title {
        "Protection" => 40,
        "Miscellaneous" => 50,
        "Group Properties" => 60,
        "Line Format" => 70,
        "Fill Format" => 75,
        "Text Block Format" => 110,
        "Events" => 122,
        "Image Properties" => 124,
        "Glue Info" => 126,
        "Shape Layout" => 128,
        "Gradient Properties" => 130,
        "Additional Effect Properties" => 132,
        "Bevel Properties" => 134,
        "3-D Rotation Properties" => 136,
        "Theme Properties" => 230,
        "Quick Style" => 240,
        "Change Shape Behavior" => 250,
        _ => 900, // "Additional Cells" lands after the known catalog
    }
}

/// A real `<Section>` element's rank in the render order. Unknown
/// section names sort last (alphabetically within the rank).
fn section_rank(name: &str) -> i32 {
    match name {
        "XForm1D" => 10, // 1-D Endpoints sits right under Shape Transform
        "User" => 20,
        "Connection" => 30,
        "Character" => 80,
        "Paragraph" => 90,
        "Tabs" => 100,
        "TextXForm" => 116,
        "LayerMembership" => 120,
        "LineGradient" => 210,
        "FillGradient" => 220,
        "Prop" => 280,
        "SmartTags" => 290,
        "Hyperlinks" => 300,
        "Actions" => 310,
        "Control" => 320,
        "Fields" => 330,
        "Scratch" => 340,
        name if name.starts_with("Geometry") => 400,
        _ => 1000,
    }
}

/// Visio's display names for ShapeSheet sections.
fn section_display_name(n: &str, ix: i64) -> String {
    match n {
        "User" => "User-defined Cells".into(),
        "Connection" => "Connection Points".into(),
        "XForm1D" => "1-D Endpoints".into(),
        "TextXForm" => "Text Transform".into(),
        "LayerMembership" => "Layer Membership".into(),
        "FillGradient" => "Fill Gradient Stops".into(),
        "LineGradient" => "Line Gradient Stops".into(),
        "Prop" => "Shape Data".into(),
        "SmartTags" => "Smart Tags".into(),
        "Fields" => "Text Fields".into(),
        "Control" => "Control Points".into(),
        "Geometry" => format!("Geometry {}", ix + 1),
        "RulerGrid" => "Ruler & Grid".into(),
        "PageLayout" => "Page Layout".into(),
        "PrintPageSetup" => "Print Setup".into(),
        "PagesXForm" => "Page Properties".into(),
        other => other.into(),
    }
}

/// Visio's observed defaults for canonical cells that resolve nowhere
/// in the chain — the ShapeSheet still shows them (locks "0", toggles
/// "FALSE", image percentages, none-active theme indices, …). Cells
/// without an entry here display "No Formula".
fn display_default(cell: &str) -> Option<&'static str> {
    Some(match cell {
        // Protection — all off by default.
        n if n.starts_with("Lock") => "0",
        // Group Properties — Visio's defaults for unset group cells
        // (SelectMode/IsSnapTarget/IsDropTarget pinned by the issue
        // screenshots for the container whose master tree lacks them).
        "SelectMode" => "1",
        "IsSnapTarget" => "TRUE",
        "IsDropTarget" => "FALSE",
        // Miscellaneous.
        "NoObjHandles" | "NoCtlHandles" | "NoAlignBox" | "NonPrinting" | "HideText"
        | "UpdateAlignBox" | "NoLiveDynamics" | "IsDropSource" | "LocalizeMerge" | "NoPrinting" => {
            "FALSE"
        }
        "ObjType" | "DynFeedback" | "Calendar" => "0",
        "Comment" => "\"\"",
        "DropOnPageScale" => "100%",
        // Image Properties.
        "Contrast" | "Brightness" => "50%",
        "Gamma" => "1",
        "Blur" | "Sharpen" | "Denoise" | "Transparency" => "0%",
        // Shape Layout.
        "ShapePermeableX"
        | "ShapePermeableY"
        | "ShapePermeablePlace"
        | "ShapeSplit"
        | "ShapeSplitable" => "FALSE",
        "ShapeFixedCode" | "ShapePlowCode" | "ShapeRouteStyle" | "ConLineJumpDirX"
        | "ConLineJumpDirY" | "ConFixedCode" | "ConLineJumpCode" | "ConLineJumpStyle"
        | "ShapePlaceFlip" | "ConLineRouteExt" => "0",
        // Gradient Properties default to Visio's theme-indirection
        // formula (observed in the corpus on unset theme-aware cells).
        "LineGradientDir"
        | "LineGradientAngle"
        | "FillGradientDir"
        | "FillGradientAngle"
        | "LineGradientEnabled"
        | "UseGroupGradient"
        | "RotateGradientWithShape" => "THEMEVAL()",
        // 3-D Rotation.
        "RotationXAngle" | "RotationYAngle" | "RotationZAngle" | "Perspective" => "0 deg",
        "RotationType" => "0",
        "DistanceFromGround" => "0 pt",
        "KeepTextFlat" => "FALSE",
        // Bevel.
        "BevelBottomType" => "0",
        "BevelBottomWidth" | "BevelBottomHeight" | "BevelDepthSize" => "0 pt",
        "BevelDepthColor" => "1",
        // Theme Properties — the none-active sentinel.
        "ConnectorSchemeIndex"
        | "EffectSchemeIndex"
        | "ColorSchemeIndex"
        | "FontSchemeIndex"
        | "ThemeIndex"
        | "VariationColorIndex"
        | "VariationStyleIndex"
        | "EmbellishmentIndex" => "65534",
        // Quick Style.
        "QuickStyleLineMatrix"
        | "QuickStyleFillMatrix"
        | "QuickStyleEffectsMatrix"
        | "QuickStyleLineColor"
        | "QuickStyleFillColor"
        | "QuickStyleShadowColor"
        | "QuickStyleFontColor"
        | "QuickStyleFontMatrix" => "102",
        "QuickStyleType" | "QuickStyleVariation" => "0",
        _ => return None,
    })
}

// ---------------------------------------------------------------------------
// Display text
// ---------------------------------------------------------------------------

/// Cell names whose un-unit'd values are lengths (inches internally,
/// displayed in mm like Visio's metric rulers).
const LENGTH_CELLS: &[&str] = &[
    "PinX", "PinY", "Width", "Height", "LocPinX", "LocPinY", "X", "Y", "BeginX", "BeginY", "EndX",
    "EndY",
];

/// A number trimmed Visio-style: up to 4 decimals, trailing zeros
/// trimmed, at least one digit ("0.5", "147.5357", "0").
fn num_trim(v: f64) -> String {
    let s = format!("{v:.4}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() || s == "-" {
        "0".into()
    } else {
        s.into()
    }
}

/// The unit'd display of a value cell: the `V` attribute carries the
/// cell's magnitude in Visio's *internal* units — inches for lengths,
/// radians for angles — so MM converts ×25.4, PT ×72 (the corpus's
/// style sheets store the 4 pt Normal margin as 0.0555) and degrees
/// convert from radians (FillGradientAngle's 4.7124 = 270°).
/// `cell` names the cell for its default-unit fallback.
fn fmt_value(v: &str, u: Option<&str>, cell: &str) -> String {
    let unit = u.unwrap_or_else(|| {
        if LENGTH_CELLS.contains(&cell) {
            "MM"
        } else if cell == "Angle" {
            "DEG"
        } else {
            ""
        }
    });
    match unit {
        "MM" | "DL" => match v.trim().parse::<f64>() {
            Ok(n) => format!("{} mm", num_trim(n * 25.4)),
            Err(_) => v.to_string(),
        },
        "PT" | "DP" => match v.trim().parse::<f64>() {
            Ok(n) => format!("{} pt", num_trim(n * 72.0)),
            Err(_) => format!("{v} pt"),
        },
        "DEG" | "DA" | "AD" => match v.trim().parse::<f64>() {
            Ok(n) => format!("{} deg", num_trim(n.to_degrees())),
            Err(_) => format!("{v} deg"),
        },
        "PERCENT" | "%" => match v.trim().parse::<f64>() {
            Ok(n) => format!("{}%", num_trim(n * 100.0)),
            Err(_) => v.to_string(),
        },
        "" => v.to_string(),
        other => format!("{v} {other}"),
    }
}

/// The ShapeSheet's one display per cell: the formula text when the
/// cell carries one (Visio shows formulas verbatim — including the
/// literal "No Formula", which wins even when a cached V rides along:
/// the corpus's document sheet carries `F="No Formula" V="0"` on its
/// Events cells and Visio's ShapeSheet shows "No Formula"), else the
/// value with its unit, else "No Formula". `F="Inh"` is not a formula
/// — it is the inheritance marker; callers resolve the effective cell.
fn cell_text(c: &Cell, cell: &str) -> String {
    match c.f.as_deref() {
        Some(f) if !f.is_empty() && f != "Inh" => return f.to_string(),
        _ => {}
    }
    match c.v.as_deref() {
        Some(v) if !v.is_empty() => fmt_value(v, c.u.as_deref(), cell),
        _ => "No Formula".into(),
    }
}

// ---------------------------------------------------------------------------
// The resolved view
// ---------------------------------------------------------------------------

/// One rendered cell: text plus whether it came from deeper than the
/// instance sheet (dimmed, like Visio's inherited cells).
#[derive(Debug, Clone, PartialEq)]
pub struct MergedCell {
    pub text: String,
    pub inherited: bool,
}

impl MergedCell {
    fn local(c: &Cell, cell: &str) -> MergedCell {
        MergedCell {
            text: cell_text(c, cell),
            inherited: false,
        }
    }
    fn deep(c: &Cell, cell: &str) -> MergedCell {
        MergedCell {
            text: cell_text(c, cell),
            inherited: true,
        }
    }
    fn fallback(text: String) -> MergedCell {
        MergedCell {
            text,
            inherited: true,
        }
    }
}

/// One section ready to render.
#[derive(Debug, Clone, PartialEq)]
pub enum SectionView {
    /// Shape Transform's three column groups.
    Transform {
        cols: Vec<Vec<(String, MergedCell)>>,
    },
    /// Name+value pairs, flowing `per_row` pairs per row.
    Pairs {
        title: String,
        pairs: Vec<(String, MergedCell)>,
        per_row: usize,
    },
    /// A row grid: gray row-label column, header row of cell names.
    Rows {
        title: String,
        headers: Vec<String>,
        /// (row label, label is a red row name, cells)
        rows: Vec<(String, bool, Vec<MergedCell>)>,
    },
    /// Layer Membership's single bare value line (no header row).
    Bare { title: String, text: String },
}

/// One row of a merged section, resolved against deeper levels. A
/// cell needing resolution (whole section inherited, `F="Inh"`, or
/// empty) looks up the same section / matching row / same cell name
/// down the rest of the chain; with no deeper match the cell itself
/// shows — an `F="Inh"` cell keeps its cached value (marked
/// inherited), an empty one falls through to "No Formula".
fn merged_row(
    base_row: &Row,
    base_inherited: bool,
    section: &str,
    deeper: &[&Sheet],
) -> Vec<MergedCell> {
    base_row
        .cells
        .iter()
        .map(|(n, c)| {
            let empty_v = c.v.as_deref().map(str::is_empty).unwrap_or(true);
            let empty_f = c.f.as_deref().map(str::is_empty).unwrap_or(true);
            if !base_inherited && c.f.as_deref() != Some("Inh") && !(empty_v && empty_f) {
                return MergedCell::local(c, n);
            }
            for sheet in deeper {
                let Some(sec) = find_section(sheet, section, -1) else {
                    continue;
                };
                let hit = sec
                    .rows
                    .iter()
                    .find(|r| match (&base_row.n, &r.n) {
                        (Some(a), Some(b)) => a == b,
                        _ => r.ix == base_row.ix,
                    })
                    .and_then(|r| r.cell(n));
                if let Some(h) = hit {
                    return MergedCell::deep(h, n);
                }
            }
            // No deeper source: the cell itself. An `F="Inh"` cell's
            // cached V survives (shown inherited); an empty cell's
            // text is "No Formula" anyway.
            MergedCell::deep(c, n)
        })
        .collect()
}

/// Find the first section with `name` (and `ix` when `ix >= 0`, for
/// repeated names like Geometry) in a sheet.
fn find_section<'a>(sheet: &'a Sheet, name: &str, ix: i64) -> Option<&'a Section> {
    sheet.children.iter().find_map(|ch| match ch {
        SheetChild::Section(s)
            if s.name == name && (ix < 0 || name != "Geometry" || s.ix == ix) =>
        {
            Some(s)
        }
        _ => None,
    })
}

/// Visio's canonical column headers for the always-shown text-run
/// sections, used when the section is absent at every level.
fn canonical_headers(name: &str) -> &'static [&'static str] {
    match name {
        "Character" => &[
            "Font",
            "Size",
            "Scale",
            "Spacing",
            "Color",
            "Transparency",
            "Style",
            "Case",
            "Pos.",
            "Strikethru",
            "DoubleULine",
            "Overline",
            "DoubleStrikeThrough",
            "AsianFont",
            "ComplexScriptFont",
            "ComplexScriptSize",
        ],
        "Paragraph" => &[
            "IndFirst",
            "IndLeft",
            "IndRight",
            "SpLine",
            "SpBefore",
            "SpAfter",
            "HAlign",
            "Bullet",
            "BulletString",
            "BulletFont",
            "TextPosAfterBullet",
            "BulletSize",
            "Flags",
        ],
        "Tabs" => &[
            "Position",
            "Alignment",
            "Position",
            "Alignment",
            "Position",
            "Alignment",
            "Position",
            "Alignment",
        ],
        _ => &[],
    }
}

/// The row label: red `Prefix.Name` for named rows of User/Prop-like
/// sections; else the row number (Visio numbers Character/Paragraph/
/// Tabs rows from 0, everything else from 1).
fn row_label(section: &str, row: &Row) -> (String, bool) {
    if let Some(n) = &row.n {
        let prefix = match section {
            "User" => "User.",
            "Prop" => "Prop.",
            _ => "",
        };
        return (format!("{prefix}{n}"), true);
    }
    if matches!(section, "Character" | "Paragraph" | "Tabs") {
        (row.ix.to_string(), false)
    } else {
        (format!("{}", row.ix + 1), false)
    }
}

/// The inheritance levels below the instance sheet, in resolution
/// order: the master source, then the style cascades (walked like
/// `resolve_cell`'s cascade), then the document sheet.
fn deeper_sheets<'a>(
    doc: &'a Document,
    shape: &'a Shape,
    ancestors: &[&'a Shape],
) -> Vec<&'a Sheet> {
    let mut levels: Vec<&Sheet> = Vec::new();
    if let Some(src) = doc.instance_source(shape, ancestors) {
        levels.push(&src.sheet);
    }
    for root in [shape.line_style, shape.fill_style, shape.text_style] {
        let mut cur_id = root;
        for _ in 0..8 {
            let Some(st) = cur_id.and_then(|id| doc.stylesheet(id)) else {
                break;
            };
            if !levels.contains(&&st.sheet) {
                levels.push(&st.sheet);
            }
            cur_id = st
                .sheet
                .cell("LineStyle")
                .and_then(|c| c.numeric())
                .map(|v| v as i64);
            if cur_id == root {
                break;
            }
        }
    }
    levels.push(&doc.sheet);
    levels
}

/// Build the full section list for one shape's resolved sheet.
pub fn build_views(doc: &Document, page_id: i64, shape_id: i64) -> Vec<SectionView> {
    let Ok(chain) = shape_chain(doc, page_id, shape_id) else {
        return Vec::new();
    };
    let Some(shape) = chain.last().copied() else {
        return Vec::new();
    };
    let ancestors: &[&Shape] = &chain[..chain.len() - 1];
    let local = &shape.sheet;
    let levels = deeper_sheets(doc, shape, ancestors);

    // (sheet, is the sheet below the instance) in resolution order.
    let mut sheets: Vec<(&Sheet, bool)> = vec![(local, false)];
    sheets.extend(levels.iter().map(|s| (*s, true)));

    // Resolve one top-level cell for DISPLAY: a local cell not
    // marked `F="Inh"` wins; otherwise the deeper levels are walked
    // for the first presentable cell — the ShapeSheet shows the
    // master's formula (`Width*0.5`), not the instance's cached
    // snapshot value, even for placement cells. `resolve_cell` (used
    // when nothing deeper carries the cell) prefers the cached V for
    // those, so it is the fallback, not the display walk.
    let resolved = |name: &str| -> MergedCell {
        match local.cell(name) {
            Some(c) if !c.is_inherited() => return MergedCell::local(c, name),
            _ => {}
        }
        for sheet in &levels {
            if let Some(c) = sheet.cell(name) {
                if !c.is_inherited() {
                    return MergedCell::deep(c, name);
                }
            }
        }
        doc.resolve_cell(shape, ancestors, name)
            .map(|c| MergedCell {
                text: cell_text(&c, name),
                inherited: true,
            })
            .unwrap_or_else(|| {
                MergedCell::fallback(
                    display_default(name)
                        .map(str::to_string)
                        .unwrap_or_else(|| "No Formula".into()),
                )
            })
    };

    let mut views: Vec<SectionView> = Vec::new();

    // -- Shape Transform: the sheet's top-level placement cells.
    views.push(SectionView::Transform {
        cols: SHAPE_TRANSFORM_COLS
            .iter()
            .map(|col| col.iter().map(|n| (n.to_string(), resolved(n))).collect())
            .collect(),
    });

    // -- Real sections: the union over every level, in Visio's order.
    let mut names: Vec<(String, i64)> = Vec::new();
    for (sheet, _) in &sheets {
        for ch in &sheet.children {
            if let SheetChild::Section(s) = ch {
                let key = (s.name.clone(), s.ix);
                if !names.contains(&key) {
                    names.push(key);
                }
            }
        }
    }
    // Sections Visio always shows even when absent at every level:
    // the text-run trio and both gradient-stop tables.
    for must in [
        ("Character".to_string(), 0),
        ("Paragraph".to_string(), 0),
        ("Tabs".to_string(), 0),
        ("LineGradient".to_string(), 0),
        ("FillGradient".to_string(), 0),
    ] {
        if !names.contains(&must) {
            names.push(must);
        }
    }
    names.sort_by_key(|(n, _)| section_rank(n));
    for (name, ix) in &names {
        if let Some(v) = section_view(&sheets, name, *ix) {
            views.push(v);
        }
    }

    // -- Layer Membership: the top-level `LayerMember` cell rendered
    //    as Visio's bare single-line section (when the shape carries
    //    it as a cell rather than a real LayerMembership section).
    if !names.iter().any(|(n, _)| n == "LayerMembership") {
        let lm = resolved("LayerMember");
        if lm.text != "No Formula" {
            views.push(SectionView::Bare {
                title: "Layer Membership".into(),
                text: lm.text,
            });
        }
    }

    // -- The fixed pairs sections in Visio's order, each carrying its
    //    canonical cell set resolved through the chain. Group
    //    Properties follows Visio's visibility: group shapes always
    //    carry it; non-groups only when any of its cells exists at
    //    any level.
    let is_group = shape
        .type_
        .as_deref()
        .map(|t| t.eq_ignore_ascii_case("Group"))
        .unwrap_or(false);
    for &(title, cells, per_row) in PAIRS_SECTIONS {
        let mut pairs = Vec::new();
        for n in cells {
            pairs.push((n.to_string(), resolved(n)));
        }
        if title == "Group Properties" && !is_group {
            let present = cells
                .iter()
                .any(|n| local.cell(n).is_some() || levels.iter().any(|s| s.cell(n).is_some()));
            if !present {
                continue;
            }
        }
        views.push(SectionView::Pairs {
            title: title.to_string(),
            pairs,
            per_row,
        });
    }

    // -- Unmapped top-level cells (anything the catalog above doesn't
    //    place): never hide structure.
    let mut cataloged: Vec<&str> = vec!["LayerMember"];
    for col in SHAPE_TRANSFORM_COLS {
        cataloged.extend_from_slice(col);
    }
    for (_, cells, _) in PAIRS_SECTIONS {
        cataloged.extend_from_slice(cells);
    }
    let extra: Vec<(String, MergedCell)> = local
        .children
        .iter()
        .filter_map(|ch| match ch {
            SheetChild::Cell(n, c)
                if !cataloged.contains(&n.as_str())
                    && !matches!(
                        n.as_str(),
                        // Style-sheet plumbing never shows on a shape.
                        "EnableLineProps" | "EnableFillProps" | "EnableTextProps" | "HideForApply"
                    ) =>
            {
                let cell = if c.is_inherited() {
                    doc.resolve_cell(shape, ancestors, n)
                        .map(|r| MergedCell::deep(&r, n))
                        .unwrap_or_else(|| MergedCell::deep(c, n))
                } else {
                    MergedCell::local(c, n)
                };
                Some((n.clone(), cell))
            }
            _ => None,
        })
        .collect();
    if !extra.is_empty() {
        views.push(SectionView::Pairs {
            title: "Additional Cells".into(),
            pairs: extra,
            per_row: 3,
        });
    }

    views.sort_by_key(|v| match v {
        SectionView::Transform { .. } => 0,
        SectionView::Pairs { title, .. } => pairs_rank(title),
        SectionView::Rows { title, .. } | SectionView::Bare { title, .. } => match title.as_str() {
            "User-defined Cells" => section_rank("User"),
            "Connection Points" => section_rank("Connection"),
            "1-D Endpoints" => section_rank("XForm1D"),
            "Text Transform" => section_rank("TextXForm"),
            "Layer Membership" => section_rank("LayerMembership"),
            "Line Gradient Stops" => section_rank("LineGradient"),
            "Fill Gradient Stops" => section_rank("FillGradient"),
            "Shape Data" => section_rank("Prop"),
            "Smart Tags" => section_rank("SmartTags"),
            "Text Fields" => section_rank("Fields"),
            "Control Points" => section_rank("Control"),
            t if t.starts_with("Geometry ") => section_rank("Geometry"),
            other => section_rank(other),
        },
    });

    views
}

/// Merge one real section (by name, ix) across the ordered sheets
/// into a view. Source priority is the list order; cells needing
/// resolution look further down the list.
fn section_view(sheets: &[(&Sheet, bool)], name: &str, ix: i64) -> Option<SectionView> {
    let base_pos = sheets
        .iter()
        .position(|(s, _)| find_section(s, name, ix).is_some());
    let deeper: Vec<&Sheet> = sheets[base_pos.map_or(0, |p| p + 1)..]
        .iter()
        .map(|(s, _)| *s)
        .collect();
    let base = base_pos.and_then(|p| {
        let (s, inherited) = sheets[p];
        find_section(s, name, ix).map(|sec| (sec, inherited))
    });

    // Gradient stops always render Visio's 10 rows: rows beyond the
    // stored ones show the theme-indirection default per column.
    if matches!(name, "LineGradient" | "FillGradient") {
        let headers: Vec<String> = base
            .and_then(|(sec, _)| sec.rows.first())
            .map(|r| r.cells.iter().map(|(n, _)| n.clone()).collect())
            .unwrap_or_else(|| {
                [
                    "GradientStopColor",
                    "GradientStopColorTrans",
                    "GradientStopPosition",
                ]
                .iter()
                .map(|s| s.to_string())
                .collect()
            });
        let mut rows = Vec::new();
        // Stored stop rows inherit together with their section: a
        // master-side FillGradient table renders inherited, only a
        // locally-stored table is the shape's own.
        let base_inherited = base.as_ref().is_none_or(|(_, inh)| *inh);
        for r in 0..10 {
            let cells: Vec<MergedCell> = match base.and_then(|(sec, _)| sec.rows.get(r)) {
                Some(row) => merged_row(row, base_inherited, name, &deeper),
                None => headers
                    .iter()
                    .map(|_| MergedCell::fallback("THEMEVAL()".into()))
                    .collect(),
            };
            rows.push((format!("{}", r + 1), false, cells));
        }
        return Some(SectionView::Rows {
            title: section_display_name(name, ix),
            headers,
            rows,
        });
    }

    // Absent text-run sections still render (one No Formula row under
    // Visio's canonical column set).
    let base = match base {
        Some(b) => Some(b),
        None if matches!(name, "Character" | "Paragraph" | "Tabs") => {
            let headers: Vec<String> = canonical_headers(name)
                .iter()
                .map(|s| s.to_string())
                .collect();
            let ncols = headers.len();
            return Some(SectionView::Rows {
                title: section_display_name(name, ix),
                headers,
                rows: vec![(
                    "0".into(),
                    false,
                    vec![MergedCell::fallback("No Formula".into()); ncols],
                )],
            });
        }
        None => return None,
    };
    // All absent-section cases returned above.
    let (base, base_inherited) = base.expect("absent-section cases returned above");

    // Layer Membership renders as a single bare value line.
    if name == "LayerMembership" {
        let text = base
            .rows
            .first()
            .map(|r| {
                r.cells
                    .iter()
                    .map(|(n, c)| cell_text(c, n))
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .unwrap_or_default();
        return Some(SectionView::Bare {
            title: "Layer Membership".into(),
            text,
        });
    }

    let headers: Vec<String> = base
        .rows
        .first()
        .map(|r| r.cells.iter().map(|(n, _)| n.clone()).collect())
        .unwrap_or_default();
    let mut rows: Vec<(String, bool, Vec<MergedCell>)> = Vec::new();
    if !base.cells.is_empty() {
        // Section-level cells (Geometry's NoFill/NoLine/…) render as
        // a leading row, resolved down the chain like the row cells.
        let cells = base
            .cells
            .iter()
            .map(|(n, c)| {
                if !base_inherited && !c.is_inherited() {
                    return MergedCell::local(c, n);
                }
                deeper
                    .iter()
                    .find_map(|s| find_section(s, name, ix))
                    .and_then(|s| s.cell(n))
                    .map(|h| MergedCell::deep(h, n))
                    .unwrap_or_else(|| MergedCell::deep(c, n))
            })
            .collect();
        rows.push((String::new(), false, cells));
    }
    for row in &base.rows {
        let (label, red) = row_label(name, row);
        rows.push((label, red, merged_row(row, base_inherited, name, &deeper)));
    }
    // Row sections Visio always shows (Character/Paragraph/Tabs)
    // render even with no stored rows — one empty row of No Formula.
    if rows.is_empty() && matches!(name, "Character" | "Paragraph" | "Tabs") {
        rows.push((
            "0".into(),
            false,
            vec![MergedCell::fallback("No Formula".into()); headers.len().max(1)],
        ));
    }
    Some(SectionView::Rows {
        title: section_display_name(name, ix),
        headers,
        rows,
    })
}

// ---------------------------------------------------------------------------
// Rendering — the Visio 2016 ShapeSheet look
// ---------------------------------------------------------------------------

/// Section title-bar blue (observed Visio 2016 ShapeSheet header).
fn bar_fill(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(0x3e, 0x5f, 0x8c)
    } else {
        Color32::from_rgb(0xbd, 0xd7, 0xee)
    }
}

/// The dark-red cell-name label color.
fn name_color(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(0xe0, 0x7a, 0x7a)
    } else {
        Color32::from_rgb(0xc0, 0x00, 0x00)
    }
}

fn row_label_fill(dark: bool) -> Color32 {
    if dark {
        Color32::from_gray(58)
    } else {
        Color32::from_rgb(0xd9, 0xd9, 0xd9)
    }
}

/// Full-width section title bar with the section name.
fn section_bar(ui: &mut Ui, title: &str) {
    let dark = ui.visuals().dark_mode;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 19.0), Sense::hover());
    ui.painter().rect_filled(rect, 1.0, bar_fill(dark));
    let label = Label::new(RichText::new(title).strong())
        .sense(Sense::hover())
        .selectable(false);
    ui.put(rect.shrink2(Vec2::new(6.0, 0.0)), label);
    ui.add_space(1.0);
}

/// A bounded cell: the label clips at `w` like Visio's fixed-width
/// columns. `truncate()` alone does not bound a grid cell — egui
/// measures the label's intrinsic width when sizing the column, and a
/// long formula would blow the whole window out; `allocate_ui` makes
/// the reported size (and thus the window) deterministic.
fn bounded_label(ui: &mut Ui, w: f32, label: Label, right_align: bool, hover: &str) {
    let h = 18.0;
    ui.allocate_ui(Vec2::new(w, h), |ui| {
        if right_align {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add(label.sense(Sense::hover()).selectable(false).truncate());
            });
        } else {
            ui.add(label.sense(Sense::hover()).selectable(false).truncate());
        }
    })
    .response
    .on_hover_text(hover);
}

/// The red right-aligned cell name.
fn cell_label(ui: &mut Ui, w: f32, text: &str) {
    let t = RichText::new(text)
        .strong()
        .color(name_color(ui.visuals().dark_mode));
    bounded_label(ui, w, Label::new(t), true, text);
}

/// The value-or-formula display, clipped at the column with the full
/// text on hover. Visio 2016 shows inherited and unset cells in the
/// normal text color (the screenshots pin this), so no dimming here.
fn value_label(ui: &mut Ui, w: f32, cell: &MergedCell) {
    bounded_label(
        ui,
        w,
        Label::new(RichText::new(&cell.text)),
        false,
        &cell.text,
    );
}

/// The panel body for one shape's ShapeSheet window. `max_w` is the
/// MDI tile width the window lives in: the content never forces the
/// window past its slot, so it cannot drift (constrain_to shifts an
/// oversized window left) over a neighboring child's title bar.
pub fn panel(ui: &mut Ui, doc: &Document, page_id: i64, shape_id: i64, max_w: f32) {
    let views = build_views(doc, page_id, shape_id);
    if views.is_empty() {
        ui.label("shape not found");
        return;
    }
    // The viewport is capped: the resolved sheet of a master-backed
    // shape runs 25+ sections, and an auto-sized window would swallow
    // the drawing (the kittest right-clicks land on the canvas). Long
    // formulas clip at the column like Visio's fixed-width cells.
    // The viewport is capped: the resolved sheet of a master-backed
    // shape runs 25+ sections, and an auto-sized window would swallow
    // the drawing (the kittest right-clicks land on the canvas). Every
    // cell below is width-bounded against this FIXED content width —
    // sizing cells against the live window width feeds an egui Grid
    // settle loop (grid.rs recalibrates its columns every frame).
    let view_w = max_w.clamp(300.0, 800.0);
    egui::ScrollArea::vertical()
        .id_salt(("dc-shapesheet-scroll", shape_id))
        .max_height(420.0)
        .max_width(view_w)
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            // Visio-like column slots: a pairs row shares the fixed
            // width between (name, value) pairs; the gray row-label
            // column keeps its Visio width and the data columns share
            // the rest (Visio's Character section squeezes as hard).
            let avail = view_w;
            for (i, v) in views.iter().enumerate() {
                match v {
                    SectionView::Transform { cols } => {
                        section_bar(ui, "Shape Transform");
                        let nrows = cols.iter().map(Vec::len).max().unwrap_or(0);
                        let groups = cols.len().max(1) as f32;
                        let slot = avail / groups;
                        let (name_w, val_w) = (slot * 0.52, slot * 0.48 - 8.0);
                        egui::Grid::new(("dc-ss-xform", i))
                            .striped(true)
                            .min_col_width(8.0)
                            .show(ui, |ui| {
                                for r in 0..nrows {
                                    for col in cols {
                                        match col.get(r) {
                                            Some((n, c)) => {
                                                cell_label(ui, name_w, n);
                                                value_label(ui, val_w, c);
                                            }
                                            None => {
                                                ui.label("");
                                                ui.label("");
                                            }
                                        }
                                    }
                                    ui.end_row();
                                }
                            });
                    }
                    SectionView::Pairs {
                        title,
                        pairs,
                        per_row,
                    } => {
                        section_bar(ui, title);
                        let slot = avail / *per_row as f32;
                        let (name_w, val_w) = (slot * 0.52, slot * 0.48 - 8.0);
                        egui::Grid::new(("dc-ss-pairs", i))
                            .striped(true)
                            .min_col_width(8.0)
                            .show(ui, |ui| {
                                for chunk in pairs.chunks(*per_row) {
                                    for (n, c) in chunk {
                                        cell_label(ui, name_w, n);
                                        value_label(ui, val_w, c);
                                    }
                                    for _ in chunk.len()..*per_row {
                                        ui.label("");
                                        ui.label("");
                                    }
                                    ui.end_row();
                                }
                            });
                    }
                    SectionView::Rows {
                        title,
                        headers,
                        rows,
                    } => {
                        section_bar(ui, title);
                        const LABEL_W: f32 = 110.0;
                        let ncols = headers.len().max(1) as f32;
                        // No minimum: Visio's Character section squeezes
                        // its 17 columns just as hard at this width, and
                        // a floor here would push the grid (and with it
                        // the whole window) wider than the MDI tile.
                        let col_w = (avail - LABEL_W) / ncols;
                        egui::Grid::new(("dc-ss-rows", i))
                            .striped(true)
                            .min_col_width(8.0)
                            .show(ui, |ui| {
                                if !headers.is_empty() {
                                    ui.label("");
                                    for h in headers {
                                        let t = RichText::new(h).strong();
                                        bounded_label(ui, col_w, Label::new(t), false, h);
                                    }
                                    ui.end_row();
                                }
                                let dark = ui.visuals().dark_mode;
                                for (label, red, cells) in rows {
                                    // The gray row-label column.
                                    if label.is_empty() {
                                        ui.label("");
                                    } else {
                                        egui::Frame::new()
                                            .fill(row_label_fill(dark))
                                            .inner_margin(egui::Margin::symmetric(4, 2))
                                            .show(ui, |ui| {
                                                let t = if *red {
                                                    RichText::new(label)
                                                        .strong()
                                                        .color(name_color(dark))
                                                } else {
                                                    RichText::new(label).strong()
                                                };
                                                bounded_label(
                                                    ui,
                                                    LABEL_W - 8.0,
                                                    Label::new(t),
                                                    false,
                                                    label,
                                                );
                                            });
                                    }
                                    for c in cells {
                                        value_label(ui, col_w, c);
                                    }
                                    ui.end_row();
                                }
                            });
                    }
                    SectionView::Bare { title, text } => {
                        section_bar(ui, title);
                        ui.add_space(2.0);
                        let t = RichText::new(text);
                        bounded_label(ui, avail, Label::new(t), false, text);
                        ui.add_space(2.0);
                    }
                }
                ui.add_space(3.0);
            }
        });
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use vsdx::model::{IndexChild, Page};

    /// A master whose shape carries a User section, a Connection
    /// section, and top-level cells; an instance with an EMPTY sheet
    /// (`Master` + `MasterShape` only), like the corpus's container
    /// parts.
    fn fixture() -> Document {
        let mut master_shape = Shape {
            id: 6,
            name_u: Some("Core".into()),
            ..Shape::default()
        };
        master_shape.sheet.children.push(SheetChild::Cell(
            "Width".into(),
            Cell::value("0.3937007874015748"),
        ));
        master_shape.sheet.children.push(SheetChild::Cell(
            "FillForegnd".into(),
            Cell {
                v: Some("#afc2da".into()),
                f: Some("THEMEVAL(\"FillColor\")".into()),
                ..Cell::default()
            },
        ));
        master_shape
            .sheet
            .children
            .push(SheetChild::Section(Section {
                name: "User".into(),
                rows: vec![Row {
                    ix: 0,
                    n: Some("msvSDContainerMargin".into()),
                    cells: vec![(
                        "Value".into(),
                        Cell {
                            v: Some("0.11811023622047244".into()),
                            f: Some("3mm".into()),
                            ..Cell::default()
                        },
                    )],
                    ..Row::default()
                }],
                ..Section::default()
            }));
        master_shape
            .sheet
            .children
            .push(SheetChild::Section(Section {
                name: "Connection".into(),
                rows: vec![Row {
                    ix: 0,
                    cells: vec![
                        ("X".into(), Cell::value("0.1968503937007874")),
                        ("Y".into(), Cell::value("0")),
                    ],
                    ..Row::default()
                }],
                ..Section::default()
            }));
        let mut master = vsdx::model::Master {
            id: 42,
            name_u: Some("Container".into()),
            ..vsdx::model::Master::default()
        };
        master.shapes.push(master_shape);

        // The instance: everything inherits, nothing is local.
        let shape = Shape {
            id: 100,
            name_u: Some("Container".into()),
            master: Some(42),
            master_shape: Some(6),
            ..Shape::default()
        };

        let mut page = Page {
            id: 1,
            name: "Page-1".into(),
            ..Page::default()
        };
        page.shapes.push(shape);
        page.children.push(IndexChild::Sheet {
            tag: "PageSheet".into(),
            sheet: Sheet::default(),
        });
        let mut doc = Document::default();
        doc.masters.push(master);
        doc.pages.push(page);
        doc
    }

    fn page_sheet_mut(doc: &mut Document) -> &mut Sheet {
        match doc.pages[0].children.first_mut().unwrap() {
            IndexChild::Sheet { sheet, .. } => sheet,
            _ => panic!("page needs a PageSheet"),
        }
    }

    fn pairs_of(views: &[SectionView], title: &str) -> Vec<(String, MergedCell)> {
        views
            .iter()
            .find_map(|v| match v {
                SectionView::Pairs {
                    title: t, pairs, ..
                } if t == title => Some(pairs.clone()),
                _ => None,
            })
            .unwrap_or_else(|| panic!("{title} section present"))
    }

    #[test]
    fn empty_instance_resolves_master_structure() {
        let doc = fixture();
        let views = build_views(&doc, 1, 100);
        // Shape Transform resolves Width from the master.
        let SectionView::Transform { cols } = &views[0] else {
            panic!("first view is Shape Transform");
        };
        let width = cols[0]
            .iter()
            .find(|(n, _)| n == "Width")
            .expect("Width cell");
        // V-only lengths display in mm like Visio's metric rulers.
        assert_eq!(width.1.text, "10 mm");
        assert!(width.1.inherited, "master-sourced cell dims");
        // The master's User section surfaces with its named row.
        let users = views
            .iter()
            .find_map(|v| match v {
                SectionView::Rows { title, rows, .. } if title == "User-defined Cells" => {
                    Some(rows.clone())
                }
                _ => None,
            })
            .expect("master User section visible through inheritance");
        assert_eq!(users.len(), 1);
        assert_eq!(users[0].0, "User.msvSDContainerMargin");
        assert!(users[0].1, "named rows label red");
        // Formula text wins over the cached value (Visio's display).
        assert_eq!(users[0].2[0].text, "3mm");
        // Connection Points grid shows the row.
        assert!(views.iter().any(|v| matches!(
            v,
            SectionView::Rows { title, rows, .. } if title == "Connection Points" && rows.len() == 1
        )));
    }

    #[test]
    fn pairs_sections_cover_the_canonical_catalog() {
        let doc = fixture();
        let views = build_views(&doc, 1, 100);
        // Protection always renders with all 24 cells (defaults 0).
        let prot = pairs_of(&views, "Protection");
        assert_eq!(prot.len(), 24);
        assert!(prot
            .iter()
            .all(|(n, c)| n.starts_with("Lock") && c.text == "0"));
        // Theme Properties carries the none-active sentinel.
        let theme = pairs_of(&views, "Theme Properties");
        assert!(theme.iter().all(|(_, c)| c.text == "65534"));
        // The master's FillForegnd resolves through (formula wins).
        let fill = pairs_of(&views, "Fill Format");
        let fg = fill.iter().find(|(n, _)| n == "FillForegnd").unwrap();
        assert_eq!(fg.1.text, "THEMEVAL(\"FillColor\")");
        assert!(fg.1.inherited);
    }

    #[test]
    fn local_override_wins_and_is_not_dimmed() {
        let mut doc = fixture();
        let shape = &mut doc.pages[0].shapes[0];
        shape.sheet.children.push(SheetChild::Cell(
            "FillForegnd".into(),
            Cell::value("#ff0000"),
        ));
        let views = build_views(&doc, 1, 100);
        let fill = pairs_of(&views, "Fill Format");
        let fg = fill.iter().find(|(n, _)| n == "FillForegnd").unwrap();
        assert_eq!(fg.1.text, "#ff0000");
        assert!(!fg.1.inherited);
    }

    #[test]
    fn gradient_stops_render_ten_visio_rows() {
        let mut doc = fixture();
        let shape = &mut doc.pages[0].shapes[0];
        shape.sheet.children.push(SheetChild::Section(Section {
            name: "FillGradient".into(),
            rows: vec![Row {
                ix: 0,
                cells: vec![
                    ("GradientStopColor".into(), Cell::value("#afc2da")),
                    ("GradientStopColorTrans".into(), Cell::value("0")),
                    ("GradientStopPosition".into(), Cell::value("0")),
                ],
                ..Row::default()
            }],
            ..Section::default()
        }));
        let views = build_views(&doc, 1, 100);
        let stops = views
            .iter()
            .find_map(|v| match v {
                SectionView::Rows { title, rows, .. } if title == "Fill Gradient Stops" => {
                    Some(rows.clone())
                }
                _ => None,
            })
            .expect("Fill Gradient Stops");
        assert_eq!(stops.len(), 10, "Visio always shows ten stop rows");
        assert_eq!(stops[0].2[0].text, "#afc2da");
        assert!(!stops[0].2[0].inherited);
        assert_eq!(stops[1].2[0].text, "THEMEVAL()");
    }

    #[test]
    fn gradient_stops_inherit_from_a_master_side_table() {
        let mut doc = fixture();
        // The stop table lives on the MASTER, not the instance.
        let master = doc.masters[0].shapes.first_mut().unwrap();
        master.sheet.children.push(SheetChild::Section(Section {
            name: "FillGradient".into(),
            rows: vec![Row {
                ix: 0,
                cells: vec![
                    ("GradientStopColor".into(), Cell::value("#eef2f7")),
                    ("GradientStopColorTrans".into(), Cell::value("0")),
                    ("GradientStopPosition".into(), Cell::value("1")),
                ],
                ..Row::default()
            }],
            ..Section::default()
        }));
        let views = build_views(&doc, 1, 100);
        let stops = views
            .iter()
            .find_map(|v| match v {
                SectionView::Rows { title, rows, .. } if title == "Fill Gradient Stops" => {
                    Some(rows.clone())
                }
                _ => None,
            })
            .expect("Fill Gradient Stops");
        assert_eq!(stops[0].2[0].text, "#eef2f7");
        assert!(stops[0].2[0].inherited, "a master-side stop is inherited");
    }

    #[test]
    fn local_inh_cell_keeps_its_cached_value() {
        let mut doc = fixture();
        // A local stop cell marked F="Inh" with a cached value, and no
        // deeper source carrying the section.
        let shape = &mut doc.pages[0].shapes[0];
        shape.sheet.children.push(SheetChild::Section(Section {
            name: "FillGradient".into(),
            rows: vec![Row {
                ix: 0,
                cells: vec![
                    (
                        "GradientStopColor".into(),
                        Cell {
                            v: Some("#afc2da".into()),
                            f: Some("Inh".into()),
                            ..Cell::default()
                        },
                    ),
                    ("GradientStopColorTrans".into(), Cell::value("0")),
                    ("GradientStopPosition".into(), Cell::value("0")),
                ],
                ..Row::default()
            }],
            ..Section::default()
        }));
        let views = build_views(&doc, 1, 100);
        let stops = views
            .iter()
            .find_map(|v| match v {
                SectionView::Rows { title, rows, .. } if title == "Fill Gradient Stops" => {
                    Some(rows.clone())
                }
                _ => None,
            })
            .expect("Fill Gradient Stops");
        // The cached value survives the failed deeper lookup.
        assert_eq!(stops[0].2[0].text, "#afc2da");
        assert!(stops[0].2[0].inherited);
    }

    #[test]
    fn cell_text_rules() {
        let f = |f: Option<&str>, v: Option<&str>| Cell {
            f: f.map(str::to_string),
            v: v.map(str::to_string),
            ..Cell::default()
        };
        assert_eq!(
            cell_text(&f(Some("GUARD(FALSE)"), Some("0")), "FlipX"),
            "GUARD(FALSE)"
        );
        // F="Inh" falls to the value, displayed in the cell's unit.
        assert_eq!(
            cell_text(&f(Some("Inh"), Some("5.808492595615412")), "Width"),
            "147.5357 mm"
        );
        assert_eq!(
            cell_text(&f(Some("No Formula"), None), "TheData"),
            "No Formula"
        );
        // The literal "No Formula" wins even over a cached value.
        assert_eq!(
            cell_text(&f(Some("No Formula"), Some("0")), "TheData"),
            "No Formula"
        );
        // Un-unit'd length cells display in mm like Visio's rulers.
        assert_eq!(
            cell_text(&f(None, Some("5.808492595615412")), "Width"),
            "147.5357 mm"
        );
        assert_eq!(cell_text(&f(None, Some("0.5")), "ObjType"), "0.5");
        assert_eq!(cell_text(&f(None, None), "BegTrigger"), "No Formula");
        // mm conversion from internal inches with U=MM.
        let mm = Cell {
            v: Some("0.01968503937007874".into()),
            u: Some("MM".into()),
            ..Cell::default()
        };
        assert_eq!(cell_text(&mm, "ShapeShdwOffsetX"), "0.5 mm");
        // Points convert from internal inches (the corpus's 4 pt
        // Normal margin is stored as 0.0555).
        let pt = Cell {
            v: Some("0.05555555555555555".into()),
            u: Some("PT".into()),
            ..Cell::default()
        };
        assert_eq!(cell_text(&pt, "LeftMargin"), "4 pt");
        // Degrees convert from internal radians (FillGradientAngle's
        // 4.7124 is 270°).
        let deg = Cell {
            v: Some("4.7123889803847".into()),
            u: Some("DEG".into()),
            ..Cell::default()
        };
        assert_eq!(cell_text(&deg, "FillGradientAngle"), "270 deg");
        // Unparseable values fall back to the raw text.
        let raw = Cell {
            v: Some("THEME".into()),
            u: Some("PT".into()),
            ..Cell::default()
        };
        assert_eq!(cell_text(&raw, "LeftMargin"), "THEME pt");
        assert_eq!(num_trim(147.53575), "147.5358");
        assert_eq!(num_trim(0.0), "0");
    }

    #[test]
    fn pagesheet_scopes_do_not_confuse_the_builder() {
        let mut doc = fixture();
        // A PageSheet with a stray User section must not leak into a
        // shape sheet's view.
        page_sheet_mut(&mut doc)
            .children
            .push(SheetChild::Section(Section {
                name: "User".into(),
                rows: vec![Row {
                    ix: 0,
                    n: Some("PageOnly".into()),
                    ..Row::default()
                }],
                ..Section::default()
            }));
        let views = build_views(&doc, 1, 100);
        let users = views
            .iter()
            .find_map(|v| match v {
                SectionView::Rows { title, rows, .. } if title == "User-defined Cells" => {
                    Some(rows.clone())
                }
                _ => None,
            })
            .expect("User section still present");
        assert_eq!(users.len(), 1, "only the master's row");
        assert_eq!(users[0].0, "User.msvSDContainerMargin");
    }
}
