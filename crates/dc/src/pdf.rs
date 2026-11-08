//! PDF export backend (#35): the parsed page SVG walks out as a PDF
//! 1.7 file — shapes and strokes as vector path operators, gradients
//! as axial/radial shading patterns, media as image XObjects, and
//! TEXT AS TEXT: every text chunk re-shapes through harfrust on the
//! same face the raster resolved (`raster::parse_svg` collapsed each
//! font-family chain to one canonical family before usvg parsed), and
//! a Type0/CID font with a ToUnicode CMap keeps the words selectable
//! and copyable — the issue's acceptance bar.
//!
//! Coordinate story: the SVG viewBox is inches, y-down; the PDF
//! MediaBox is points, y-up. The base content transform
//! `[72 0 0 −72 0 H·72]` maps one onto the other, and every node's
//! usvg ABSOLUTE transform (already resolved through all ancestors)
//! rides on top of it — no per-node composition is kept.

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use pdf_writer::types::{
    CidFontType, FontFlags, FunctionShadingType, LineCapStyle, LineJoinStyle, SystemInfo,
    UnicodeCmap,
};
use pdf_writer::{Chunk, Content, Filter, Finish, Name, Pdf, Ref, Str};
use resvg::usvg;
use resvg::usvg::fontdb;
use resvg::usvg::tiny_skia_path;

use crate::raster;
use crate::svg;

/// One glyph as shaped by harfrust, in font units.
#[derive(Clone)]
struct ShapedGlyph {
    id: u16,
    /// Byte offset of the cluster start inside the shaped substring.
    cluster: usize,
    x_advance: f32,
    x_offset: f32,
}

/// A run of glyphs all drawn with one face; `text` is the substring
/// the run shaped (cluster offsets index it).
struct GlyphRun {
    face: FaceKey,
    text: String,
    glyphs: Vec<ShapedGlyph>,
}

/// Identity of an embedded font: the fontdb face the chain resolved
/// to. The db is process-global, so the ID is a stable key.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
struct FaceKey(fontdb::ID);

/// The PDF object refs of one embedded face (allocated on first use
/// during emission, dicts written at assembly — /W and ToUnicode are
/// only known once every page has been walked).
#[derive(Clone)]
struct FaceRefs {
    type0: Ref,
    cid: Ref,
    descriptor: Ref,
    file: Ref,
    to_unicode: Ref,
    /// PostScript name for /BaseFont.
    postscript: String,
    /// The content-stream resource name (`F0`, `F1`, …), unique per
    /// registered face (#218 review): PostScript names collide — a
    /// system face and its vendored twin, and every empty name
    /// collapses to one string — while the /Font dict is keyed by
    /// this name. Duplicate keys would draw one face's glyph IDs
    /// with the other face's font.
    res_name: &'static str,
}

/// One decoded image XObject; `name` is the content-stream name and
/// the resources key (leaked once — a PDF's handful of names).
struct ImageEntry {
    object: Ref,
    name: &'static str,
}

/// The shading pattern of one gradient definition.
struct ShadeEntry {
    pattern: Ref,
    name: &'static str,
}

/// Per-page assembled state.
struct PageData {
    content: Vec<u8>,
    width_pt: f32,
    height_pt: f32,
}

/// The PDF context: registries shared by every page (one font
/// embedding per face for the whole file), a Ref counter, and the
/// caches keyed by tree-object pointer identity. Insertion-order Vecs
/// keep the assembly deterministic (HashMap iteration is not).
struct PdfContext<'db> {
    db: &'db Arc<fontdb::Database>,
    next_ref: i32,
    faces: HashMap<FaceKey, FaceRefs>,
    face_order: Vec<FaceKey>,
    /// Glyph → nominal width (1/1000-em units) per face.
    widths: HashMap<FaceKey, BTreeMap<u16, f32>>,
    /// Glyph → Unicode text per face (the ToUnicode CMap source).
    to_unicode: HashMap<FaceKey, BTreeMap<u16, Vec<char>>>,
    images: HashMap<usize, ImageEntry>,
    image_order: Vec<usize>,
    shadings: HashMap<(usize, [u32; 6]), ShadeEntry>,
    shading_order: Vec<(usize, [u32; 6])>,
    /// char → fallback face for glyphs the primary face lacks.
    fallbacks: HashMap<char, Option<FaceKey>>,
    image_counter: u32,
    shade_counter: u32,
    /// The page content's base transform (px → points, y-flip): a
    /// shading pattern's /Matrix ignores the CTM at paint time, so
    /// the gradient must bake it plus the node's abs transform.
    base: tiny_skia_path::Transform,
}

impl<'db> PdfContext<'db> {
    fn new(db: &'db Arc<fontdb::Database>) -> Self {
        Self {
            db,
            next_ref: 1,
            faces: HashMap::new(),
            face_order: Vec::new(),
            widths: HashMap::new(),
            to_unicode: HashMap::new(),
            images: HashMap::new(),
            image_order: Vec::new(),
            shadings: HashMap::new(),
            shading_order: Vec::new(),
            fallbacks: HashMap::new(),
            image_counter: 0,
            shade_counter: 0,
            base: tiny_skia_path::Transform::identity(),
        }
    }

    fn alloc_ref(&mut self) -> Ref {
        let r = Ref::new(self.next_ref);
        self.next_ref += 1;
        r
    }

    /// A content-stream / resources name leaked for 'static.
    fn leak_name(prefix: &str, n: u32) -> &'static str {
        Box::leak(format!("{prefix}{n}").into_boxed_str())
    }

    /// The face bytes + index for a FaceKey, read from the db.
    fn face_data(&self, key: FaceKey) -> Option<(Vec<u8>, u32)> {
        let info = self.db.face(key.0)?;
        let data = match &info.source {
            fontdb::Source::Binary(arc) => (**arc).as_ref().to_vec(),
            fontdb::Source::File(path) => std::fs::read(path).ok()?,
            fontdb::Source::SharedFile(path, _) => std::fs::read(path).ok()?,
            #[allow(unreachable_patterns)] // memmap-off builds
            _ => return None,
        };
        Some((data, info.index))
    }

    /// Query the db for the face a chunk's span asks for. The SVG's
    /// font-family chains were already resolved to ONE canonical
    /// installed family by `raster::parse_svg`, so the query is
    /// exact-family; weight/style come from the span.
    fn query_face(&self, font: &usvg::Font) -> Option<FaceKey> {
        let families: Vec<fontdb::Family> = font
            .families()
            .iter()
            .map(|f| match f {
                usvg::FontFamily::Named(name) => fontdb::Family::Name(name.as_str()),
                usvg::FontFamily::Serif => fontdb::Family::Serif,
                usvg::FontFamily::SansSerif => fontdb::Family::SansSerif,
                usvg::FontFamily::Monospace => fontdb::Family::Monospace,
                usvg::FontFamily::Cursive => fontdb::Family::Cursive,
                usvg::FontFamily::Fantasy => fontdb::Family::Fantasy,
            })
            .collect();
        let query = fontdb::Query {
            families: &families,
            weight: fontdb::Weight(font.weight()),
            style: fontdb::Style::from(font.style()),
            stretch: fontdb::Stretch::from(font.stretch()),
        };
        self.db.query(&query).map(FaceKey)
    }

    /// A face that can draw `c` when the primary one can't: scans the
    /// database for any face whose cmap covers the char (cached per
    /// char). Vendored + system faces alike.
    fn fallback_face(&mut self, c: char) -> Option<FaceKey> {
        if let Some(hit) = self.fallbacks.get(&c) {
            return *hit;
        }
        let mut found: Option<FaceKey> = None;
        for face in self.db.faces() {
            let Some((data, index)) = self.face_data(FaceKey(face.id)) else {
                continue;
            };
            if face_covers(&data, index, c) {
                found = Some(FaceKey(face.id));
                break;
            }
        }
        self.fallbacks.insert(c, found);
        found
    }

    /// Resolve a face key to refs, registering on first use.
    fn face_refs(&mut self, key: FaceKey) -> Option<FaceRefs> {
        if let Some(r) = self.faces.get(&key) {
            return Some(r.clone());
        }
        let postscript = self
            .db
            .face(key.0)
            .map(|i| i.post_script_name.clone())
            .unwrap_or_default();
        let refs = FaceRefs {
            type0: self.alloc_ref(),
            cid: self.alloc_ref(),
            descriptor: self.alloc_ref(),
            file: self.alloc_ref(),
            to_unicode: self.alloc_ref(),
            res_name: Self::leak_name("F", self.face_order.len() as u32),
            postscript,
        };
        self.faces.insert(key, refs.clone());
        self.face_order.push(key);
        Some(refs)
    }
}

/// Does the face at `data[index]` have a real glyph for `c`?
fn face_covers(data: &[u8], index: u32, c: char) -> bool {
    ttf_parser::Face::parse(data, index)
        .ok()
        .and_then(|f| f.glyph_index(c))
        .is_some_and(|g| g.0 != 0)
}

/// Units-per-em of a face (parsed per call; faces are few).
fn face_upem(data: &[u8], index: u32) -> f32 {
    ttf_parser::Face::parse(data, index)
        .map(|f| f32::from(f.units_per_em()))
        .unwrap_or(1000.0)
}

/// Nominal advance of one glyph in 1/1000-em units (the /W form).
fn glyph_width_1000(face: &ttf_parser::Face, gid: u16) -> f32 {
    face.glyph_hor_advance(ttf_parser::GlyphId(gid))
        .map(|a| a as f32 * 1000.0 / f32::from(face.units_per_em()))
        .unwrap_or(0.0)
}

/// Shape `text` with harfrust on the face at `data[index]` — the same
/// pipeline the measurement side uses (`textmetrics`): BIDI visual
/// runs at base LTR, default shaping features.
fn shape_text(data: &[u8], index: u32, text: &str) -> Vec<ShapedGlyph> {
    use harfrust::{Direction, ShapeOptions, UnicodeBuffer};
    let Ok(font_ref) = harfrust::FontRef::from_index(data, index) else {
        return Vec::new();
    };
    let shaper_data = harfrust::ShaperData::new(&font_ref);
    let shaper = shaper_data.shaper(&font_ref).build();
    let bidi = unicode_bidi::BidiInfo::new(text, Some(unicode_bidi::Level::ltr()));
    let Some(paragraph) = bidi.paragraphs.first() else {
        return Vec::new();
    };
    let (levels, runs) = bidi.visual_runs(paragraph, paragraph.range.clone());
    let mut glyphs = Vec::new();
    for run in runs.iter() {
        let sub = &text[run.clone()];
        if sub.is_empty() {
            continue;
        }
        let direction = if levels[run.start].is_ltr() {
            Direction::LeftToRight
        } else {
            Direction::RightToLeft
        };
        let mut buffer = UnicodeBuffer::new();
        buffer.push_str(sub);
        buffer.set_direction(direction);
        buffer.guess_segment_properties();
        let out = shaper.shape(buffer, ShapeOptions::new());
        let infos = out.glyph_infos();
        let positions = out.glyph_positions();
        for i in 0..out.len() {
            glyphs.push(ShapedGlyph {
                id: u16::try_from(infos[i].glyph_id).unwrap_or(0),
                // Text-absolute: harfrust clusters are offsets into
                // the run's own slice; the fallback splitter slices
                // the SPAN text by them (#218 review).
                cluster: run.start + infos[i].cluster as usize,
                x_advance: positions[i].x_advance as f32,
                x_offset: positions[i].x_offset as f32,
            });
        }
    }
    glyphs
}

/// Split a shaped run where the primary face lacks coverage: every
/// glyph with id 0 (the .notdef harfrust returns for missing glyphs)
/// whose cluster text the face cannot draw moves to a fallback face,
/// so a Latin label with one CJK name still renders both halves —
/// the same per-glyph fallback resvg does, at run granularity.
fn split_fallback_runs(
    ctx: &mut PdfContext,
    primary: FaceKey,
    text: &str,
    glyphs: Vec<ShapedGlyph>,
) -> Vec<GlyphRun> {
    let Some((data, index)) = ctx.face_data(primary) else {
        return Vec::new();
    };
    // Runs group (face, byte-span, glyphs); the span closes when the
    // face changes, and `text` is sliced to it with the cluster
    // offsets rebased — cluster values index the whole shaped string.
    type PendingRun = (FaceKey, usize, usize, Vec<ShapedGlyph>);
    let mut runs: Vec<GlyphRun> = Vec::new();
    let mut current: Option<PendingRun> = None;
    for (i, g) in glyphs.iter().enumerate() {
        let end = glyphs
            .get(i + 1)
            .map_or(text.len(), |n| n.cluster.min(text.len()));
        let start = g.cluster.min(end);
        let sub = text.get(start..end).unwrap_or("");
        let missing = g.id == 0 && sub.chars().all(|c| !face_covers(&data, index, c));
        let face = if missing {
            // The cluster's chars are all uncovered: fallback face for
            // the first char (runs group by char coverage).
            sub.chars().next().and_then(|c| ctx.fallback_face(c))
        } else {
            Some(primary)
        };
        match (face, current.as_mut()) {
            (Some(f), Some((rf, _, re, rg))) if *rf == f => {
                rg.push(g.clone());
                *re = end;
            }
            (Some(f), _) => {
                if let Some(run) = current.take() {
                    runs.push(finalize_run(run, text));
                }
                current = Some((f, start, end, vec![g.clone()]));
            }
            // No face anywhere: drop the glyph (the raster drops it too).
            (None, _) => {}
        }
    }
    if let Some(run) = current.take() {
        runs.push(finalize_run(run, text));
    }
    runs
}

/// Slice a pending run's text to its byte span and rebase its glyph
/// cluster offsets onto the slice.
fn finalize_run(run: (FaceKey, usize, usize, Vec<ShapedGlyph>), text: &str) -> GlyphRun {
    let (face, start, end, mut glyphs) = run;
    // RTL visual order returns DECREASING clusters, so the first
    // glyph can sit past a later one — rebase from the run's MINIMUM
    // cluster and never underflow (#218 review).
    let base = glyphs.iter().map(|g| g.cluster).min().unwrap_or(start).min(start);
    for g in &mut glyphs {
        g.cluster = g.cluster.saturating_sub(base);
    }
    GlyphRun {
        face,
        text: text.get(start..end).unwrap_or("").to_string(),
        glyphs,
    }
}

/// usvg's absolute transform as a PDF matrix (same six coefficients).
fn transform_matrix(t: tiny_skia_path::Transform) -> [f32; 6] {
    [t.sx, t.ky, t.kx, t.sy, t.tx, t.ty]
}

/// A ∘ B: B maps a point first, then A (PDF cm / usvg convention).
fn compose(a: tiny_skia_path::Transform, b: tiny_skia_path::Transform) -> tiny_skia_path::Transform {
    tiny_skia_path::Transform::from_row(
        a.sx * b.sx + a.kx * b.ky,
        a.ky * b.sx + a.sy * b.ky,
        a.sx * b.kx + a.kx * b.sy,
        a.ky * b.kx + a.sy * b.sy,
        a.sx * b.tx + a.kx * b.ty + a.tx,
        a.ky * b.tx + a.sy * b.ty + a.ty,
    )
}

/// usvg y-down color → PDF rgb 0..1.
fn color_rgb(c: usvg::Color) -> [f32; 3] {
    [
        f32::from(c.red) / 255.0,
        f32::from(c.green) / 255.0,
        f32::from(c.blue) / 255.0,
    ]
}

fn cap_pdf(cap: usvg::LineCap) -> LineCapStyle {
    match cap {
        usvg::LineCap::Butt => LineCapStyle::ButtCap,
        usvg::LineCap::Round => LineCapStyle::RoundCap,
        usvg::LineCap::Square => LineCapStyle::ProjectingSquareCap,
    }
}

fn join_pdf(join: usvg::LineJoin) -> LineJoinStyle {
    match join {
        usvg::LineJoin::Miter | usvg::LineJoin::MiterClip => LineJoinStyle::MiterJoin,
        usvg::LineJoin::Round => LineJoinStyle::RoundJoin,
        usvg::LineJoin::Bevel => LineJoinStyle::BevelJoin,
    }
}

/// Flate-compress raw bytes — PDF's FlateDecode is zlib-wrapped
/// deflate, so the encoder is `ZlibEncoder` (not the raw one).
fn flate(raw: &[u8]) -> Vec<u8> {
    use std::io::Write as _;
    let mut enc = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    let _ = enc.write_all(raw);
    enc.finish().unwrap_or_else(|_| raw.to_vec())
}

/// Encode and register one image's bytes as an XObject (deduped by
/// the encoded-bytes identity): raw RGB samples + an alpha soft mask
/// when any pixel is translucent, both Flate-compressed.
fn register_image(
    chunk: &mut Chunk,
    ctx: &mut PdfContext,
    kind: &usvg::ImageKind,
) -> Option<&'static str> {
    let bytes: &Arc<Vec<u8>> = match kind {
        usvg::ImageKind::PNG(b)
        | usvg::ImageKind::JPEG(b)
        | usvg::ImageKind::GIF(b)
        | usvg::ImageKind::WEBP(b) => b,
        // Nested SVG images never occur in Tarahi output.
        usvg::ImageKind::SVG(_) => return None,
    };
    let key = Arc::as_ptr(bytes) as *const u8 as usize;
    if let Some(hit) = ctx.images.get(&key) {
        return Some(hit.name);
    }
    let img = image::load_from_memory(bytes).ok()?.to_rgba8();
    let (w, h) = (img.width(), img.height());
    let opaque = img.pixels().all(|p| p.0[3] == 255);
    let mut rgb = Vec::with_capacity(w as usize * h as usize * 3);
    let mut alpha = Vec::with_capacity(w as usize * h as usize);
    for p in img.pixels() {
        rgb.extend_from_slice(&[p.0[0], p.0[1], p.0[2]]);
        alpha.push(p.0[3]);
    }
    let smask = if opaque {
        None
    } else {
        let r = ctx.alloc_ref();
        let data = flate(&alpha);
        let mut sm = chunk.image_xobject(r, &data);
        sm.width(w as i32)
            .height(h as i32)
            .color_space_name(Name(b"DeviceGray"))
            .bits_per_component(8);
        sm.filter(Filter::FlateDecode).finish();
        Some(r)
    };
    let object = ctx.alloc_ref();
    let name = PdfContext::leak_name("Im", ctx.image_counter);
    let data = flate(&rgb);
    let mut xobj = chunk.image_xobject(object, &data);
    xobj.width(w as i32)
        .height(h as i32)
        .color_space_name(Name(b"DeviceRGB"))
        .bits_per_component(8)
        .interpolate(true);
    if let Some(sm) = smask {
        xobj.s_mask(sm);
    }
    xobj.filter(Filter::FlateDecode).finish();
    ctx.image_counter += 1;
    ctx.images.insert(key, ImageEntry { object, name });
    ctx.image_order.push(key);
    Some(name)
}

/// Build the multi-stop stitching function of a gradient (an
/// exponential segment per adjacent stop pair, bounds at the stops).
fn write_stitch_function(
    chunk: &mut Chunk,
    ctx: &mut PdfContext,
    stops: &[usvg::Stop],
) -> Option<Ref> {
    if stops.len() < 2 {
        return None;
    }
    let mut segments = Vec::new();
    for pair in stops.windows(2) {
        let r = ctx.alloc_ref();
        let (a, b) = (color_rgb(pair[0].color()), color_rgb(pair[1].color()));
        let mut f = chunk.exponential_function(r);
        // The codomain is RGB: one 0..1 dimension per channel.
        f.domain([0.0, 1.0])
            .range([0.0, 1.0, 0.0, 1.0, 0.0, 1.0])
            .n(1.0);
        f.c0([a[0], a[1], a[2]]);
        f.c1([b[0], b[1], b[2]]);
        segments.push(r);
    }
    let top = ctx.alloc_ref();
    let bounds: Vec<f32> = stops[1..stops.len() - 1]
        .iter()
        .map(|s| s.offset().get())
        .collect();
    let encode: Vec<f32> = (0..segments.len()).flat_map(|_| [0.0, 1.0]).collect();
    let mut f = chunk.stitching_function(top);
    // Type 3 requires /Domain like the segments it stitches.
    f.domain([0.0, 1.0]);
    f.functions(segments).bounds(bounds).encode(encode);
    Some(top)
}

/// Register one gradient's shading pattern. usvg has already resolved
/// objectBoundingBox units into element user space at parse time (its
/// `units` field is parse-only), so the pattern matrix is the
/// gradient's own transform and the coordinates are element-local.
fn register_gradient(
    chunk: &mut Chunk,
    ctx: &mut PdfContext,
    paint: &usvg::Paint,
    extra: tiny_skia_path::Transform,
) -> Option<&'static str> {
    let ptr = match paint {
        usvg::Paint::LinearGradient(g) => Arc::as_ptr(g) as *const u8 as usize,
        usvg::Paint::RadialGradient(g) => Arc::as_ptr(g) as *const u8 as usize,
        // Pattern fills never occur in Tarahi output.
        _ => return None,
    };
    // The gradient's own transform composes after base ∘ abs: the
    // pattern's /Matrix maps pattern space to DEFAULT user space and
    // ignores the CTM at fill time (#218 review). Paths sharing one
    // gradient under different transforms need distinct patterns.
    let matrix = match paint {
        usvg::Paint::LinearGradient(g) => compose(extra, g.transform()),
        usvg::Paint::RadialGradient(g) => compose(extra, g.transform()),
        _ => unreachable!(),
    };
    let key = (ptr, transform_matrix(matrix).map(f32::to_bits));
    if let Some(hit) = ctx.shadings.get(&key) {
        return Some(hit.name);
    }
    let function = match paint {
        usvg::Paint::LinearGradient(g) => write_stitch_function(chunk, ctx, g.stops()),
        usvg::Paint::RadialGradient(g) => write_stitch_function(chunk, ctx, g.stops()),
        _ => None,
    }?;
    let radial = matches!(paint, usvg::Paint::RadialGradient(_));
    let shading = ctx.alloc_ref();
    let pattern = ctx.alloc_ref();
    let coords = match paint {
        usvg::Paint::LinearGradient(g) => vec![g.x1(), g.y1(), g.x2(), g.y2()],
        usvg::Paint::RadialGradient(g) => vec![g.fx(), g.fy(), 0.0, g.cx(), g.cy(), g.r().get()],
        _ => unreachable!(),
    };
    {
        let mut s = chunk.function_shading(shading);
        s.shading_type(if radial {
            FunctionShadingType::Radial
        } else {
            FunctionShadingType::Axial
        });
        s.pair(Name(b"ColorSpace"), Name(b"DeviceRGB"));
        s.function(function);
        s.coords(coords);
        s.extend([true, true]);
    }
    {
        let mut p = chunk.shading_pattern(pattern);
        p.shading_ref(shading);
        p.matrix(transform_matrix(matrix));
    }
    let name = PdfContext::leak_name("P", ctx.shade_counter);
    ctx.shade_counter += 1;
    ctx.shadings.insert(key, ShadeEntry { pattern, name });
    ctx.shading_order.push(key);
    Some(name)
}

/// Export the selected pages (0-based) of `doc` to PDF bytes.
///
/// `transparent` drops the white page fill (viewers show paper white
/// by convention; alpha-savvy pipelines honor the empty page).
pub fn pdf_bytes(
    doc: &vsdx::Document,
    pages: &[usize],
    transparent: bool,
) -> anyhow::Result<Vec<u8>> {
    if pages.is_empty() {
        anyhow::bail!("no pages selected");
    }
    let db = raster::font_db();
    // Pdf derefs to its Chunk, so every emitter writes through the
    // same object graph the trailer finishes.
    let mut pdf = Pdf::new();
    let mut ctx = PdfContext::new(&db);
    let mut page_data = Vec::new();
    let mut trees: Vec<usvg::Tree> = Vec::new();

    for &idx in pages {
        let page = doc
            .pages
            .get(idx)
            .ok_or_else(|| anyhow::anyhow!("page {} out of range", idx + 1))?;
        let w_in = num_of(page, "PageWidth").unwrap_or(8.5);
        let h_in = num_of(page, "PageHeight").unwrap_or(11.0);
        let page_svg = svg::render_page_export(doc, page, transparent)?;
        let tree = raster::parse_svg(&page_svg, &db)?;
        let mut content = Content::new();
        // Base: the SVG's user units land as PIXELS in the tree (usvg
        // bakes the viewBox→px scale — PX_PER_IN — into every node's
        // absolute transform), y-down; PDF points, y-up. 72/96 maps
        // the tree's px to points. The gradient pattern matrix needs
        // the same mapping (it ignores the CTM at paint time).
        let base = tiny_skia_path::Transform::from_row(
            72.0 / 96.0,
            0.0,
            0.0,
            -72.0 / 96.0,
            0.0,
            h_in as f32 * 72.0,
        );
        content.transform(transform_matrix(base));
        ctx.base = base;
        walk_group(&mut pdf, &mut ctx, &mut content, tree.root());
        // The image/gradient registries key by tree-object addresses:
        // every tree must OUTLIVE them, or the allocator can hand a
        // freed address to the next page's image and alias its
        // XObject (#218 review).
        trees.push(tree);
        let buf = content.finish();
        page_data.push(PageData {
            content: buf.as_slice().to_vec(),
            width_pt: w_in as f32 * 72.0,
            height_pt: h_in as f32 * 72.0,
        });
    }

    assemble(pdf, ctx, page_data)
}

fn num_of(page: &vsdx::Page, name: &str) -> Option<f64> {
    page.sheet()
        .and_then(|s| s.cell(name))
        .and_then(|c| c.numeric())
}

/// Depth-first emission in document order — the z-order the SVG
/// raster paints, preserved exactly.
fn walk_group(chunk: &mut Chunk, ctx: &mut PdfContext, content: &mut Content, group: &usvg::Group) {
    for node in group.children() {
        match node {
            usvg::Node::Group(g) => walk_group(chunk, ctx, content, g),
            usvg::Node::Path(p) => emit_path(chunk, ctx, content, p),
            usvg::Node::Image(i) => emit_image(chunk, ctx, content, i),
            usvg::Node::Text(t) => emit_text(ctx, content, t),
        }
    }
}

/// One path node: q / cm(abs) / state / path / op / Q.
fn emit_path(chunk: &mut Chunk, ctx: &mut PdfContext, content: &mut Content, p: &usvg::Path) {
    if !p.is_visible() {
        return;
    }
    let fill = p.fill();
    let stroke = p.stroke();
    if fill.is_none() && stroke.is_none() {
        return;
    }
    content
        .save_state()
        .transform(transform_matrix(p.abs_transform()));
    if let Some(f) = fill {
        set_fill_paint(chunk, ctx, content, f, p.abs_transform());
    }
    if let Some(s) = stroke {
        set_stroke_paint(chunk, ctx, content, s);
    }
    write_path_data(content, p.data());
    // Our renderer never combines fill+stroke on one element (a path
    // carries either fill= or stroke=), but the SVG model allows it —
    // fill first, Visio's paint order.
    match (fill, stroke) {
        (Some(f), Some(_)) => {
            if matches!(f.rule(), usvg::FillRule::EvenOdd) {
                content.fill_even_odd_and_stroke();
            } else {
                content.fill_nonzero_and_stroke();
            }
        }
        (Some(f), None) => {
            if matches!(f.rule(), usvg::FillRule::EvenOdd) {
                content.fill_even_odd();
            } else {
                content.fill_nonzero();
            }
        }
        (None, Some(_)) => {
            content.stroke();
        }
        (None, None) => {
            content.end_path();
        }
    }
    content.restore_state();
}

fn write_path_data(content: &mut Content, path: &tiny_skia_path::Path) {
    let mut cur = (0.0f32, 0.0f32);
    let mut started = false;
    for seg in path.segments() {
        match seg {
            tiny_skia_path::PathSegment::MoveTo(pt) => {
                content.move_to(pt.x, pt.y);
                cur = (pt.x, pt.y);
                started = true;
            }
            tiny_skia_path::PathSegment::LineTo(pt) => {
                content.line_to(pt.x, pt.y);
                cur = (pt.x, pt.y);
            }
            tiny_skia_path::PathSegment::QuadTo(ctrl, to) => {
                // pdf-writer has no quad op: elevate to a cubic — the
                // standard 2/3 de Casteljau control points.
                let (p0x, p0y) = cur;
                let c1x = p0x + 2.0 / 3.0 * (ctrl.x - p0x);
                let c1y = p0y + 2.0 / 3.0 * (ctrl.y - p0y);
                let c2x = to.x + 2.0 / 3.0 * (ctrl.x - to.x);
                let c2y = to.y + 2.0 / 3.0 * (ctrl.y - to.y);
                content.cubic_to(c1x, c1y, c2x, c2y, to.x, to.y);
                cur = (to.x, to.y);
            }
            tiny_skia_path::PathSegment::CubicTo(c1, c2, to) => {
                content.cubic_to(c1.x, c1.y, c2.x, c2.y, to.x, to.y);
                cur = (to.x, to.y);
            }
            tiny_skia_path::PathSegment::Close => {
                content.close_path();
            }
        }
    }
    if !started {
        content.end_path();
    }
}

fn set_fill_paint(
    chunk: &mut Chunk,
    ctx: &mut PdfContext,
    content: &mut Content,
    f: &usvg::Fill,
    abs: tiny_skia_path::Transform,
) {
    match f.paint() {
        usvg::Paint::Color(c) => {
            let [r, g, b] = color_rgb(*c);
            content.set_fill_rgb(r, g, b);
        }
        usvg::Paint::LinearGradient(_) | usvg::Paint::RadialGradient(_) => {
            let extra = compose(ctx.base, abs);
            match register_gradient(chunk, ctx, f.paint(), extra) {
                Some(name) => {
                    content.set_fill_color_space(pdf_writer::types::ColorSpaceOperand::Pattern);
                    content.set_fill_pattern(std::iter::empty(), Name(name.as_bytes()));
                }
                // No stops to shade with: neutral gray, never
                // white-on-white.
                None => {
                    content.set_fill_rgb(0.5, 0.5, 0.5);
                }
            }
        }
        // Pattern fills never occur in Tarahi output.
        usvg::Paint::Pattern(_) => {
            content.set_fill_rgb(0.5, 0.5, 0.5);
        }
    }
}

fn set_stroke_paint(
    _chunk: &mut Chunk,
    _ctx: &mut PdfContext,
    content: &mut Content,
    s: &usvg::Stroke,
) {
    match s.paint() {
        usvg::Paint::Color(c) => {
            let [r, g, b] = color_rgb(*c);
            content.set_stroke_rgb(r, g, b);
        }
        // Gradient strokes don't occur in Tarahi output; solid gray.
        _ => {
            content.set_stroke_rgb(0.5, 0.5, 0.5);
        }
    }
    content.set_line_width(s.width().get());
    content.set_line_cap(cap_pdf(s.linecap()));
    content.set_line_join(join_pdf(s.linejoin()));
    content.set_miter_limit(s.miterlimit().get());
    if let Some(dash) = s.dasharray() {
        content.set_dash_pattern(dash.to_vec(), 0.0);
    }
}

/// One image node: q / cm(abs ∘ bitmap scale) / Do / Q. The parser
/// bakes the SVG x/y/width/height into the absolute transform; the
/// drawing space is the bitmap's own pixel size.
fn emit_image(chunk: &mut Chunk, ctx: &mut PdfContext, content: &mut Content, img: &usvg::Image) {
    if !img.is_visible() {
        return;
    }
    let Some(name) = register_image(chunk, ctx, img.kind()) else {
        return;
    };
    let size = img.size();
    let t = img.abs_transform();
    // A ∘ [w 0 0 −h 0 h] (#218 review): PDF image space renders the
    // unit square with the FIRST sample row at y = 1, and the page
    // content's base transform is y-down — without the vertical flip
    // every picture exports mirrored top to bottom. The composition:
    // linear = A ∘ diag(w, −h); the S' translation [0, h] lands as
    // A.linear · [0, h] + A.trans.
    let (w, h) = (size.width(), size.height());
    let m = [
        t.sx * w,
        t.ky * w,
        -t.kx * h,
        -t.sy * h,
        t.kx * h + t.tx,
        t.sy * h + t.ty,
    ];
    content.save_state().transform(m);
    content.x_object(Name(name.as_bytes()));
    content.restore_state();
}

/// One text node: every chunk becomes a BT…ET run of real text —
/// re-shaped with harfrust on the resolved face, glyph codes via
/// Identity-H (CID = glyph id), positioning corrections in TJ, and
/// the cluster → Unicode mapping feeding the ToUnicode CMap.
fn emit_text(ctx: &mut PdfContext, content: &mut Content, text: &usvg::Text) {
    for text_chunk in text.chunks() {
        let full = text_chunk.text();
        if full.trim().is_empty() {
            continue;
        }
        // One style per chunk in Tarahi output; multiple spans (mixed
        // styling) shape sequentially on the same baseline.
        let mut runs: Vec<GlyphRun> = Vec::new();
        for span in text_chunk.spans() {
            if span.fill().is_none() {
                continue; // fill="none": invisible in the raster too
            }
            let start = span.start().min(full.len());
            let end = span.end().min(full.len());
            let Some(sub) = (start <= end).then(|| full.get(start..end)).flatten() else {
                continue;
            };
            if sub.is_empty() {
                continue;
            }
            let Some(primary) = ctx.query_face(span.font()) else {
                continue;
            };
            let Some((data, index)) = ctx.face_data(primary) else {
                continue;
            };
            let shaped = shape_text(&data, index, sub);
            runs.extend(split_fallback_runs(ctx, primary, sub, shaped));
        }
        if runs.is_empty() {
            continue;
        }
        // The anchor offset: chunk x is the DECLARED anchor point;
        // usvg applies middle/end shifts when flattening, so the PDF
        // does the same here — total advance of the whole line in ems,
        // scaled to the chunk's font size (viewBox units) before it
        // moves the inches-space anchor.
        let total_em: f32 = runs
            .iter()
            .map(|run| {
                ctx.face_data(run.face)
                    .map(|(d, i)| {
                        let upem = face_upem(&d, i);
                        run.glyphs.iter().map(|g| g.x_advance).sum::<f32>() / upem
                    })
                    .unwrap_or(0.0)
            })
            .sum();
        // Font size in user units (inches — the viewBox unit).
        let font_size = text_chunk
            .spans()
            .first()
            .map(|s| s.font_size().get())
            .unwrap_or(0.1);
        let anchor_shift = match text_chunk.anchor() {
            usvg::TextAnchor::Middle => -total_em * font_size / 2.0,
            usvg::TextAnchor::End => -total_em * font_size,
            usvg::TextAnchor::Start => 0.0,
        };
        // The node's absolute transform rides as a cm (like paths) so
        // the font size stays in viewBox units; glyph outlines are
        // y-up while the SVG frame is y-down, hence the Tm flip.
        let node = transform_matrix(text.abs_transform());
        let local_x = text_chunk.x().unwrap_or(0.0) + anchor_shift;
        let local_y = text_chunk.y().unwrap_or(0.0);
        // The paint: first span's fill (uniform in Tarahi output).
        let [pr, pg, pb] = text_chunk
            .spans()
            .first()
            .and_then(|s| s.fill())
            .map(|f| match f.paint() {
                usvg::Paint::Color(c) => color_rgb(*c),
                _ => [0.0, 0.0, 0.0],
            })
            .unwrap_or([0.0, 0.0, 0.0]);
        content
            .save_state()
            .transform(node)
            .begin_text()
            .set_fill_rgb(pr, pg, pb)
            .set_text_matrix([1.0, 0.0, 0.0, -1.0, local_x, local_y]);
        for run in &runs {
            let Some(refs) = ctx.face_refs(run.face) else {
                continue;
            };
            let Some((data, index)) = ctx.face_data(run.face) else {
                continue;
            };
            let Ok(tt) = ttf_parser::Face::parse(&data, index) else {
                continue;
            };
            let upem = face_upem(&data, index);
            content.set_font(Name(refs.res_name.as_bytes()), font_size);
            let mut positioned = content.show_positioned();
            let mut items = positioned.items();
            for (i, g) in run.glyphs.iter().enumerate() {
                // Cluster chars → ToUnicode (multi-char for ligatures).
                let end = run
                    .glyphs
                    .get(i + 1)
                    .map_or(run.text.len(), |n| n.cluster.min(run.text.len()));
                let chars: Vec<char> = run
                    .text
                    .get(g.cluster.min(end)..end)
                    .map(|s| s.chars().collect())
                    .unwrap_or_default();
                if g.id != 0 {
                    ctx.widths
                        .entry(run.face)
                        .or_default()
                        .insert(g.id, glyph_width_1000(&tt, g.id));
                    if !chars.is_empty() {
                        ctx.to_unicode
                            .entry(run.face)
                            .or_default()
                            .entry(g.id)
                            .or_insert(chars.clone());
                    }
                }
                // Show: 2-byte big-endian glyph id (Identity-H).
                let bytes = [(g.id >> 8) as u8, g.id as u8];
                items.show(Str(bytes.as_slice()));
                // Kerning correction: shaped advance vs the /W nominal,
                // plus the next glyph's x offset — thousandths of the
                // font size, subtracted from the pen.
                let nominal = glyph_width_1000(&tt, g.id);
                let shaped1000 = g.x_advance * 1000.0 / upem;
                let next_offset =
                    run.glyphs.get(i + 1).map(|n| n.x_offset).unwrap_or(0.0) * 1000.0 / upem;
                let adjust = nominal - shaped1000 - next_offset;
                if adjust.abs() > 1e-3 {
                    items.adjust(adjust);
                }
            }
        }
        content.end_text().restore_state();
    }
}

fn face_content_name(postscript: &str) -> Name<'_> {
    let bytes = postscript.as_bytes();
    Name(if bytes.is_empty() { b"F0" } else { bytes })
}

/// The single font program to embed. A collection's per-font offset
/// table names a face whose TABLE OFFSETS ARE COLLECTION-RELATIVE
/// (OpenType TTC layout) — slicing at the face offset would leave
/// every table pointing into the wrong bytes. Rebuild a standalone
/// sfnt with the tables rebased; a plain font file is already the
/// face.
fn face_bytes(data: &[u8], index: u32) -> Vec<u8> {
    if data.len() >= 12 && &data[0..4] == b"ttcf" {
        let n = u32::from_be_bytes([data[8], data[9], data[10], data[11]]) as usize;
        let i = index as usize;
        if n > 0 && i < n {
            let at = 12 + i * 4;
            if at + 4 <= data.len() {
                let o = u32::from_be_bytes([
                    data[at],
                    data[at + 1],
                    data[at + 2],
                    data[at + 3],
                ]) as usize;
                if o + 12 <= data.len() {
                    if let Some(out) = rebuild_sfnt(&data[o..], o) {
                        return out;
                    }
                }
            }
        }
    }
    data.to_vec()
}

/// Rebuild one sfnt (header at `face`, itself positioned `base` into
/// the collection) into a standalone font: copy the table directory
/// and re-point each entry at the table's new position, 4-byte
/// aligned as the spec requires. The stored offsets count from the
/// COLLECTION start — translate them through `base`.
fn rebuild_sfnt(face: &[u8], base: usize) -> Option<Vec<u8>> {
    if face.len() < 12 {
        return None;
    }
    let num_tables = u16::from_be_bytes([face[4], face[5]]) as usize;
    let dir_len = 12 + num_tables * 16;
    if face.len() < dir_len {
        return None;
    }
    let mut total = dir_len;
    let mut spans = Vec::with_capacity(num_tables);
    for e in 0..num_tables {
        let at = 12 + e * 16;
        let off = u32::from_be_bytes(face[at + 8..at + 12].try_into().ok()?) as usize;
        let len = u32::from_be_bytes(face[at + 12..at + 16].try_into().ok()?) as usize;
        let off = off.checked_sub(base)?;
        if off + len > face.len() {
            return None;
        }
        spans.push((off, len));
        total += (len + 3) & !3;
    }
    let mut out = vec![0u8; total];
    out[..dir_len].copy_from_slice(&face[..dir_len]);
    let mut cursor = dir_len;
    for (e, (off, len)) in spans.iter().enumerate() {
        let at = 12 + e * 16;
        out[at + 8..at + 12].copy_from_slice(&(cursor as u32).to_be_bytes());
        out[cursor..cursor + len].copy_from_slice(&face[*off..*off + len]);
        cursor += (len + 3) & !3;
    }
    Some(out)
}

/// Object assembly: fonts, images, shadings, page tree, catalog —
/// everything the walk phase allocated refs for gets its dict here.
fn assemble(
    mut pdf: Pdf,
    mut ctx: PdfContext<'_>,
    pages: Vec<PageData>,
) -> anyhow::Result<Vec<u8>> {
    let catalog = ctx.alloc_ref();
    let pages_root = ctx.alloc_ref();
    let page_refs: Vec<Ref> = (0..pages.len()).map(|_| ctx.alloc_ref()).collect();
    let content_refs: Vec<Ref> = (0..pages.len()).map(|_| ctx.alloc_ref()).collect();
    let resources = ctx.alloc_ref();

    pdf.catalog(catalog).pages(pages_root);
    {
        let mut pages_writer = pdf.pages(pages_root);
        pages_writer.kids(page_refs.iter().copied());
        pages_writer.count(pages.len() as i32);
    }

    for ((page, page_ref), content_ref) in pages.iter().zip(&page_refs).zip(&content_refs) {
        let mut p = pdf.page(*page_ref);
        p.parent(pages_root);
        p.media_box(pdf_writer::Rect::new(
            0.0,
            0.0,
            page.width_pt,
            page.height_pt,
        ));
        p.contents(*content_ref);
        // The shared resources dict, by reference (fonts/images/
        // patterns are global — one font embedding per face).
        p.pair(Name(b"Resources"), resources);
    }

    // The shared resources dict: /Font, /XObject, /Pattern, /ProcSet.
    {
        let mut res = pdf.indirect(resources).dict();
        if !ctx.face_order.is_empty() {
            let mut fonts = res.insert(Name(b"Font")).dict();
            for key in &ctx.face_order {
                if let Some(refs) = ctx.faces.get(key) {
                    fonts.pair(Name(refs.res_name.as_bytes()), refs.type0);
                }
            }
        }
        if !ctx.image_order.is_empty() {
            let mut xobjects = res.insert(Name(b"XObject")).dict();
            for key in &ctx.image_order {
                if let Some(img) = ctx.images.get(key) {
                    xobjects.pair(Name(img.name.as_bytes()), img.object);
                }
            }
        }
        if !ctx.shading_order.is_empty() {
            let mut patterns = res.insert(Name(b"Pattern")).dict();
            for key in &ctx.shading_order {
                if let Some(s) = ctx.shadings.get(key) {
                    patterns.pair(Name(s.name.as_bytes()), s.pattern);
                }
            }
        }
        let mut procset = res.insert(Name(b"ProcSet")).array();
        procset.items([
            Name(b"PDF"),
            Name(b"Text"),
            Name(b"ImageB"),
            Name(b"ImageC"),
            Name(b"ImageI"),
        ]);
    }

    // Content streams (Flate-compressed).
    for (page, content_ref) in pages.iter().zip(&content_refs) {
        let compressed = flate(&page.content);
        let mut s = pdf.stream(*content_ref, &compressed);
        s.filter(Filter::FlateDecode);
    }

    // Fonts: Type0 + CID + descriptor + file + ToUnicode.
    for key in ctx.face_order.clone() {
        let Some(refs) = ctx.faces.get(&key).cloned() else {
            continue;
        };
        let Some((data, index)) = ctx.face_data(key) else {
            continue;
        };
        let tt = ttf_parser::Face::parse(&data, index)
            .map_err(|e| anyhow::anyhow!("parsing font for embedding: {e}"))?;
        let upem = f32::from(tt.units_per_em());
        let true_type = tt.tables().cff.is_none();
        let base = face_content_name(&refs.postscript);
        {
            let mut type0 = pdf.type0_font(refs.type0);
            type0.base_font(base);
            type0.encoding_predefined(Name(b"Identity-H"));
            type0.descendant_font(refs.cid);
            type0.to_unicode(refs.to_unicode);
        }
        {
            let mut cid = pdf.cid_font(refs.cid);
            cid.subtype(if true_type {
                CidFontType::Type2
            } else {
                CidFontType::Type0
            });
            cid.base_font(base);
            cid.system_info(SystemInfo {
                registry: Str(b"Adobe"),
                ordering: Str(b"Identity"),
                supplement: 0,
            });
            cid.font_descriptor(refs.descriptor);
            cid.default_width(0.0);
            cid.cid_to_gid_map_predefined(Name(b"Identity"));
            // /W: consecutive runs over the glyphs the document used.
            let mut w = cid.widths();
            if let Some(widths) = ctx.widths.get(&key) {
                let mut iter = widths.iter().peekable();
                while let Some((gid, width)) = iter.next() {
                    let mut run = vec![*width];
                    let start = *gid;
                    while let Some((next, nw)) = iter.peek() {
                        if **next == start + run.len() as u16 {
                            run.push(**nw);
                            iter.next();
                        } else {
                            break;
                        }
                    }
                    w.consecutive(start, run);
                }
            }
        }
        {
            let mut d = pdf.font_descriptor(refs.descriptor);
            d.name(base);
            d.flags(flags_of(&tt));
            d.bbox(global_bbox(&tt));
            d.italic_angle(tt.italic_angle());
            d.ascent(f32::from(tt.ascender()) * 1000.0 / upem);
            d.descent(f32::from(tt.descender()) * 1000.0 / upem);
            d.cap_height(
                tt.capital_height()
                    .map(|c| f32::from(c) * 1000.0 / upem)
                    .unwrap_or(700.0),
            );
            d.stem_v(10.0);
            if true_type {
                d.font_file2(refs.file);
            } else {
                d.font_file3(refs.file);
            }
        }
        // The font file stream: /Length1 for TrueType (required by
        // readers), /Subtype OpenType for CFF containers. A font
        // COLLECTION (.ttc/.otc — the CJK fallback shape on most
        // hosts) is not a font program: slice out the face the
        // offset table names (#218 review).
        let embed = face_bytes(&data, index);
        {
            let mut s = pdf.stream(refs.file, &embed);
            if true_type {
                s.pair(Name(b"Length1"), embed.len() as i32);
            } else {
                s.pair(Name(b"Subtype"), Name(b"OpenType"));
            }
        }
        // ToUnicode CMap from the collected glyph → text pairs.
        let mut cmap = UnicodeCmap::new(
            Name(b"TarahiToUnicode"),
            SystemInfo {
                registry: Str(b"Adobe"),
                ordering: Str(b"Identity"),
                supplement: 0,
            },
        );
        if let Some(map) = ctx.to_unicode.get(&key) {
            for (gid, chars) in map {
                cmap.pair_with_multiple(*gid, chars.iter().copied());
            }
        }
        let cmap_bytes = cmap.finish();
        // pdf-writer's `.filter()` only DECLares the filter — the
        // bytes must arrive already compressed.
        let cmap_compressed = flate(cmap_bytes.as_slice());
        {
            let mut cmap_stream = pdf.cmap(refs.to_unicode, &cmap_compressed);
            cmap_stream.filter(Filter::FlateDecode);
        }
    }

    Ok(pdf.finish())
}

fn flags_of(face: &ttf_parser::Face) -> FontFlags {
    let mut flags = FontFlags::SYMBOLIC;
    if face.is_italic() {
        flags |= FontFlags::ITALIC;
    }
    if face.is_monospaced() {
        flags |= FontFlags::FIXED_PITCH;
    }
    flags
}

fn global_bbox(face: &ttf_parser::Face) -> pdf_writer::Rect {
    let b = face.global_bounding_box();
    pdf_writer::Rect::new(
        f32::from(b.x_min),
        f32::from(b.y_min),
        f32::from(b.x_max),
        f32::from(b.y_max),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 2×2 RGBA PNG: row 0 (top) red, row 1 (bottom) blue.
    fn two_row_png() -> Vec<u8> {
        let mut img = image::RgbaImage::new(2, 2);
        img.put_pixel(0, 0, image::Rgba([255, 0, 0, 255]));
        img.put_pixel(1, 0, image::Rgba([255, 0, 0, 255]));
        img.put_pixel(0, 1, image::Rgba([0, 0, 255, 255]));
        img.put_pixel(1, 1, image::Rgba([0, 0, 255, 255]));
        let mut out = std::io::Cursor::new(Vec::new());
        img.write_to(&mut out, image::ImageFormat::Png).unwrap();
        out.into_inner()
    }

    /// #218 review: the image matrix composes the vertical flip —
    /// the first sample row lands at the rect's TOP edge in the
    /// y-down content frame, not the bottom (every picture exported
    /// upside down).
    /// #218 review round 2: a collection face extracts as a
    /// STANDALONE sfnt — the per-font table offsets are
    /// collection-relative, so the rebuild re-points them; both faces
    /// of a synthetic two-font .ttc parse at index 0 afterward.
    #[test]
    fn ttc_face_extracts_as_a_standalone_sfnt() {
        let font = include_bytes!("../../dc/assets/fonts/Carlito-Regular.ttf");
        // Wrap the font twice into a TTC, rebasing each copy's table
        // offsets to its position in the collection — what real
        // builders do (collection-relative offsets).
        let embed_at = |font: &[u8], at: u32| -> Vec<u8> {
            let mut copy = font.to_vec();
            let num_tables = u16::from_be_bytes([copy[4], copy[5]]) as usize;
            for e in 0..num_tables {
                let t = 12 + e * 16;
                let off = u32::from_be_bytes(copy[t + 8..t + 12].try_into().unwrap());
                copy[t + 8..t + 12].copy_from_slice(&(off + at).to_be_bytes());
            }
            copy
        };
        // The header is 12 bytes (magic, version, numFonts) followed
        // by the offset array — 2 offsets = 8, so font 0 starts at 20.
        let head = font.len() as u32;
        let mut ttc = Vec::new();
        ttc.extend_from_slice(b"ttcf");
        ttc.extend_from_slice(&0x0001_0000u32.to_be_bytes());
        ttc.extend_from_slice(&2u32.to_be_bytes());
        ttc.extend_from_slice(&20u32.to_be_bytes());
        ttc.extend_from_slice(&(20 + head).to_be_bytes());
        ttc.extend_from_slice(&embed_at(font, 20));
        ttc.extend_from_slice(&embed_at(font, 20 + head));

        let f0 = face_bytes(&ttc, 0);
        let f1 = face_bytes(&ttc, 1);
        let src_face = ttf_parser::Face::parse(font, 0).unwrap();
        for (i, bytes) in [(0usize, &f0), (1, &f1)] {
            let face = ttf_parser::Face::parse(bytes, 0)
                .unwrap_or_else(|e| panic!("face {i} of the collection parses standalone: {e}"));
            assert_eq!(
                face.number_of_glyphs(),
                src_face.number_of_glyphs(),
                "face {i} carries the source's tables"
            );
        }
        // Out-of-range index falls back to the raw bytes (parse as a
        // collection fails the caller's own guard).
        assert_eq!(face_bytes(&ttc, 5).len(), ttc.len());
    }

    #[test]
    fn image_matrix_flips_the_first_row_to_the_top() {
        let png = two_row_png();
        let b64 = crate::svg::base64(&png);
        let svg = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="20" height="10"><image x="0" y="0" width="20" height="10" xlink:href="data:image/png;base64,{b64}"/></svg>"#
        );
        let db = raster::font_db();
        let tree = raster::parse_svg(&svg, &db).expect("svg parses");
        // The single Image node under the root group.
        fn find_image(g: &usvg::Group) -> Option<&usvg::Image> {
            g.children().iter().find_map(|n| match n {
                usvg::Node::Image(img) => Some(img.as_ref()),
                usvg::Node::Group(sub) => find_image(sub),
                _ => None,
            })
        }
        let img = find_image(tree.root()).expect("the svg carries the image");

        let mut pdf = Pdf::new();
        let mut ctx = PdfContext::new(&db);
        let mut content = Content::new();
        emit_image(&mut pdf, &mut ctx, &mut content, img);
        let buf = String::from_utf8(content.finish().to_vec()).unwrap();
        // A ∘ [w 0 0 −h 0 h]: with an axis-aligned image the cm's
        // matrix is [w' 0 0 −h' e' h'] — d NEGATIVE with f = −d (the
        // first sample row lands at the rect's top edge). The pre-fix
        // matrix read `[w' 0 0 h' 0 0]`: row 0 at the bottom.
        let cm = buf
            .lines()
            .find(|l| l.ends_with(" cm"))
            .expect("the image emitted a cm");
        let m: Vec<f32> = cm
            .split(' ')
            .filter_map(|t| t.parse().ok())
            .collect();
        assert_eq!(m.len(), 6, "a cm matrix: {cm:?}");
        assert!(
            m[0] > 0.0 && m[1] == 0.0 && m[2] == 0.0,
            "axis-aligned image: {cm:?}"
        );
        assert!(
            m[3] < 0.0 && (m[5] - (-m[3])).abs() < 1e-3,
            "the image matrix carries the vertical flip (d<0, f=−d): {cm:?}"
        );
    }
}
