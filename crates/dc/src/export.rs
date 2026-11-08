//! File export drivers (#35): one page or a page range of the parsed
//! document to SVG / PNG / PDF. The GUI's backstage Export page calls
//! the same functions the CLI does, so an export produced from either
//! entry point is identical.

use std::path::{Path, PathBuf};

use anyhow::Context as _;

use crate::raster;
use crate::svg;

/// The export format, normally inferred from the output file
/// extension.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Format {
    /// Vector SVG — the renderer's own output.
    Svg,
    /// Rasterized through the shared SVG stack at the chosen density.
    Png,
    /// Vector PDF with selectable text (one file for all pages).
    Pdf,
}

impl Format {
    /// Infer the format from an output extension; `None` for anything
    /// the export doesn't speak.
    pub fn from_extension(ext: &str) -> Option<Format> {
        match ext.to_ascii_lowercase().as_str() {
            "svg" => Some(Format::Svg),
            "png" => Some(Format::Png),
            "pdf" => Some(Format::Pdf),
            _ => None,
        }
    }

    /// File-dialog filter spec for the GUI save dialog.
    pub fn extensions(self) -> &'static [&'static str] {
        match self {
            Format::Svg => &["svg"],
            Format::Png => &["png"],
            Format::Pdf => &["pdf"],
        }
    }
}

/// Export options shared by every format.
#[derive(Clone, Copy, Debug)]
pub struct Options {
    /// Omit the white page fill (PNG gets a real alpha channel; the
    /// SVG/PDF just drop the background rect).
    pub transparent: bool,
    /// PNG raster density, pixels per inch (the SVG's intrinsic
    /// density is 96).
    pub png_dpi: f32,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            transparent: false,
            png_dpi: 96.0,
        }
    }
}

/// Bytes of one exported page.
pub struct PageBytes {
    pub page_no: usize,
    pub bytes: Vec<u8>,
}

/// Export the selected pages (0-based; empty = all) as bytes, one
/// entry per page — for PDF each entry is a single-page PDF (the
/// merged multi-page file comes from [`write_document`]).
pub fn page_bytes(
    doc: &vsdx::Document,
    pages: &[usize],
    format: Format,
    opts: &Options,
) -> anyhow::Result<Vec<PageBytes>> {
    let indices = resolve_pages(doc, pages)?;
    let mut out = Vec::with_capacity(indices.len());
    for idx in indices {
        let bytes = match format {
            Format::Svg => {
                svg::render_page_export(doc, &doc.pages[idx], opts.transparent)?.into_bytes()
            }
            Format::Png => {
                let page_svg = svg::render_page_export(doc, &doc.pages[idx], opts.transparent)?;
                let db = raster::font_db();
                let tree = raster::parse_svg(&page_svg, &db)?;
                let pixmap = raster::render_dpi(&tree, opts.png_dpi)?;
                raster::png_bytes(&pixmap)?
            }
            Format::Pdf => crate::pdf::pdf_bytes(doc, &[idx], opts.transparent)?,
        };
        out.push(PageBytes {
            page_no: idx + 1,
            bytes,
        });
    }
    Ok(out)
}

/// Export and write to `out`: per-page files for SVG/PNG (`-N`
/// suffixes once more than one page matches, the `dc render`
/// convention), one merged multi-page file for PDF. Returns the files
/// written.
pub fn write_document(
    doc: &vsdx::Document,
    out: &Path,
    pages: &[usize],
    format: Format,
    opts: &Options,
) -> anyhow::Result<Vec<PathBuf>> {
    if format == Format::Pdf {
        let indices = resolve_pages(doc, pages)?;
        let bytes = crate::pdf::pdf_bytes(doc, &indices, opts.transparent)?;
        std::fs::write(out, &bytes).with_context(|| format!("writing {}", out.display()))?;
        return Ok(vec![out.to_path_buf()]);
    }
    let rendered = page_bytes(doc, pages, format, opts)?;
    let numbered = rendered.len() > 1;
    let mut written = Vec::with_capacity(rendered.len());
    for page in rendered {
        let target = if numbered {
            let mut name = out.file_stem().unwrap_or_default().to_os_string();
            name.push(format!("-{}", page.page_no));
            if let Some(ext) = out.extension() {
                name.push(".");
                name.push(ext);
            }
            out.with_file_name(name)
        } else {
            out.to_path_buf()
        };
        std::fs::write(&target, &page.bytes)
            .with_context(|| format!("writing {}", target.display()))?;
        written.push(target);
    }
    Ok(written)
}

/// The concrete page indices an export covers: the given 0-based
/// selection, or every page when the selection is empty.
fn resolve_pages(doc: &vsdx::Document, pages: &[usize]) -> anyhow::Result<Vec<usize>> {
    if pages.is_empty() {
        return Ok((0..doc.pages.len()).collect());
    }
    for &p in pages {
        if p >= doc.pages.len() {
            anyhow::bail!("page {} out of range (1..={})", p + 1, doc.pages.len());
        }
    }
    Ok(pages.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use vsdx::model::{Cell, Document, IndexChild, Row, Section, Shape, Sheet, SheetChild};

    /// A letter-page Document with `n` identical pages, each carrying
    /// one filled rectangle with the given text — the synthetic
    /// fixture convention (no Microsoft content).
    fn fixture_doc(n_pages: usize) -> vsdx::Document {
        let mut pages = Vec::new();
        for i in 0..n_pages {
            let mut sheet = Sheet::default();
            sheet.set_cell("PageWidth", Cell::value("8.5"));
            sheet.set_cell("PageHeight", Cell::value("11"));
            let mut s = Shape {
                id: 1,
                ..Shape::default()
            };
            for (cell, v) in [
                ("PinX", "4.25"),
                ("PinY", "8"),
                ("Width", "2"),
                ("Height", "1"),
                ("LocPinX", "1"),
                ("LocPinY", "0.5"),
                ("FillForegnd", "#0000ff"),
            ] {
                s.sheet.set_cell(cell, Cell::value(v));
            }
            let mut geo = Section {
                name: "Geometry".into(),
                ix: 0,
                ..Section::default()
            };
            geo.rows = vec![
                row(1, "MoveTo", &[("X", 0.0), ("Y", 0.0)]),
                row(2, "LineTo", &[("X", 2.0), ("Y", 0.0)]),
                row(3, "LineTo", &[("X", 2.0), ("Y", 1.0)]),
                row(4, "LineTo", &[("X", 0.0), ("Y", 1.0)]),
                row(5, "LineTo", &[("X", 0.0), ("Y", 0.0)]),
            ];
            s.sheet.children.push(SheetChild::Section(geo));
            s.sheet.text = Some("hello visio".into());
            pages.push(vsdx::Page {
                id: i as i64,
                name: format!("Page-{}", i + 1),
                children: vec![IndexChild::Sheet {
                    tag: "PageSheet".into(),
                    sheet,
                }],
                shapes: vec![s],
                part: format!("visio/pages/page{}.xml", i + 1),
                ..vsdx::Page::default()
            });
        }
        Document {
            pages,
            ..Document::default()
        }
    }

    fn row(ix: i64, t: &str, cells: &[(&str, f64)]) -> Row {
        Row {
            t: Some(t.into()),
            ix,
            cells: cells
                .iter()
                .map(|(n, v)| (n.to_string(), Cell::value(format!("{v}"))))
                .collect(),
            ..Row::default()
        }
    }

    #[test]
    fn format_from_extension() {
        assert_eq!(Format::from_extension("SVG"), Some(Format::Svg));
        assert_eq!(Format::from_extension("png"), Some(Format::Png));
        assert_eq!(Format::from_extension("pdf"), Some(Format::Pdf));
        assert_eq!(Format::from_extension("vsdx"), None);
    }

    #[test]
    fn svg_export_drops_chrome_and_can_be_transparent() {
        let doc = fixture_doc(1);
        let opts = Options::default();
        let svg_bytes = &page_bytes(&doc, &[0], Format::Svg, &opts).unwrap()[0].bytes;
        let text = String::from_utf8_lossy(svg_bytes);
        assert!(text.contains("<text"), "text reaches the export SVG");
        assert!(!text.contains(r##"stroke="#c8c8c8""##), "no sheet border");
        assert!(
            text.contains(r##"fill="#ffffff""##),
            "white page by default"
        );
        let transparent = Options {
            transparent: true,
            ..opts
        };
        let svg_bytes = &page_bytes(&doc, &[0], Format::Svg, &transparent).unwrap()[0].bytes;
        let text = String::from_utf8_lossy(svg_bytes);
        assert!(
            !text.contains(r##"fill="#ffffff""##),
            "transparent drops the fill"
        );
    }

    #[test]
    fn png_export_rasterizes_at_the_density() {
        let doc = fixture_doc(1);
        let opts = Options {
            png_dpi: 192.0,
            ..Options::default()
        };
        let pngs = page_bytes(&doc, &[0], Format::Png, &opts).unwrap();
        assert_eq!(pngs.len(), 1);
        // 8.5 in at 192 dpi = 1632 px wide.
        let img = image::load_from_memory(&pngs[0].bytes).unwrap();
        assert_eq!(img.width(), 1632);
        // Transparent option: the corner carries no ink (alpha 0).
        let transparent = Options {
            transparent: true,
            ..opts
        };
        let pngs = page_bytes(&doc, &[0], Format::Png, &transparent).unwrap();
        let img = image::load_from_memory(&pngs[0].bytes).unwrap().to_rgba8();
        assert_eq!(img.get_pixel(0, 0).0[3], 0, "no ink at the corner");
    }

    #[test]
    fn write_document_names_multipage_files_like_render() {
        let tmp = tempfile::tempdir().unwrap();
        let doc = fixture_doc(2);
        let out = tmp.path().join("out.svg");
        let written = write_document(&doc, &out, &[], Format::Svg, &Options::default()).unwrap();
        assert_eq!(written.len(), 2);
        assert!(written[0].ends_with("out-1.svg"), "{:?}", written[0]);
        assert!(written[1].ends_with("out-2.svg"), "{:?}", written[1]);
        // A single-page export keeps the plain name.
        let written = write_document(&doc, &out, &[1], Format::Svg, &Options::default()).unwrap();
        assert_eq!(written.len(), 1);
        assert!(written[0].ends_with("out.svg"));
    }

    /// The acceptance bar (#35): exported PDF text is SELECTABLE —
    /// the ToUnicode CMaps decode the TJ glyph codes back to the
    /// fixture's words.
    #[test]
    fn pdf_text_extracts_through_tounicode() {
        let doc = fixture_doc(2);
        let bytes = crate::pdf::pdf_bytes(&doc, &[0, 1], false).unwrap();
        assert!(bytes.starts_with(b"%PDF-1.7"));
        // Two pages in one file.
        let raw = String::from_utf8_lossy(&bytes);
        assert_eq!(raw.matches("/MediaBox").count(), 2);
        // Collect every stream, decompress, and split content from
        // CMaps.
        let mut cmaps: Vec<HashMap<u16, String>> = Vec::new();
        let mut contents: Vec<Vec<u16>> = Vec::new();
        for (_, data) in streams(&bytes) {
            let text = String::from_utf8_lossy(&data);
            if text.contains("begincmap") {
                cmaps.push(parse_cmap(&text));
            } else if text.contains("TJ") {
                contents.push(parse_tj_codes(&text));
            }
        }
        assert!(!cmaps.is_empty(), "at least one ToUnicode CMap");
        assert_eq!(
            contents.len(),
            2,
            "one text-bearing content stream per page"
        );
        let merged: HashMap<u16, String> = cmaps
            .iter()
            .flat_map(|m| m.iter())
            .map(|(k, v)| (*k, v.clone()))
            .collect();
        for codes in &contents {
            let extracted: String = codes
                .iter()
                .filter_map(|c| merged.get(c).cloned())
                .collect::<Vec<_>>()
                .concat();
            assert!(
                extracted.contains("hello visio"),
                "selectable text must extract: {extracted:?}"
            );
        }
    }

    /// Every `stream … endstream` payload, zlib-decoded.
    fn streams(bytes: &[u8]) -> Vec<(usize, Vec<u8>)> {
        use std::io::Read;
        let mut out = Vec::new();
        let mut rest = bytes;
        while let Some(pos) = find(rest, b"stream") {
            let after = &rest[pos + b"stream".len()..];
            // Skip the EOL the writer emits after the `stream` keyword.
            let data_start = after
                .iter()
                .position(|b| *b != b'\n' && *b != b'\r')
                .unwrap_or(after.len());
            let Some(end) = find(after, b"endstream") else {
                break;
            };
            let payload = &after[data_start..end];
            // Trim the EOL the writer emits before endstream.
            let mut trim = 0;
            for b in payload.iter().rev() {
                if *b == b'\n' || *b == b'\r' {
                    trim += 1;
                } else {
                    break;
                }
            }
            let payload = &payload[..payload.len().saturating_sub(trim)];
            let mut dec = flate2::read::ZlibDecoder::new(payload);
            let mut data = Vec::new();
            // Non-stream objects (font files) aren't zlib — skip.
            if dec.read_to_end(&mut data).is_ok() && !data.is_empty() {
                out.push((pos, data));
            }
            rest = &after[end + b"endstream".len()..];
        }
        out
    }

    fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
        hay.windows(needle.len()).position(|w| w == needle)
    }

    /// ToUnicode CMap: `<gid> <utf16…>` pairs → (gid, string).
    fn parse_cmap(text: &str) -> HashMap<u16, String> {
        let mut map = HashMap::new();
        // Only the bfchar ranges — the codespacerange header also
        // carries `<0000> <FFFF>` pairs that are not glyph mappings.
        let body = text.split("beginbfchar").nth(1).unwrap_or(text);
        let chars: Vec<char> = body.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            if chars[i] == '<' {
                let Some(close) = chars[i..].iter().position(|c| *c == '>') else {
                    break;
                };
                let hex: String = chars[i + 1..i + close].iter().collect();
                // Distinguish source-gid pairs from utf16 targets by
                // pairing: source <XXXX>, target <…> after whitespace.
                if hex.len() == 4 {
                    if let Ok(gid) = u16::from_str_radix(&hex, 16) {
                        // Find the next <…> after this one.
                        let mut j = i + close + 1;
                        while j < chars.len() && chars[j] != '<' {
                            j += 1;
                        }
                        if let Some(end) = chars[j..].iter().position(|c| *c == '>') {
                            let t: String = chars[j + 1..j + end].iter().collect();
                            let mut s = String::new();
                            let tb: Vec<char> = t.chars().collect();
                            let mut k = 0;
                            while k + 4 <= tb.len() {
                                if let Ok(u) = u16::from_str_radix(
                                    &tb[k..k + 4].iter().collect::<String>(),
                                    16,
                                ) {
                                    if let Some(c) = char::from_u32(u as u32) {
                                        s.push(c);
                                    }
                                }
                                k += 4;
                            }
                            map.entry(gid).or_insert(s);
                            i = j + end;
                        }
                    }
                }
            }
            i += 1;
        }
        map
    }

    /// Glyph codes from a content stream's TJ arrays (2-byte
    /// Identity-H codes; hex or literal strings).
    fn parse_tj_codes(text: &str) -> Vec<u16> {
        let mut codes = Vec::new();
        let chars: Vec<char> = text.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            match chars[i] {
                '<' => {
                    if let Some(end) = chars[i..].iter().position(|c| *c == '>') {
                        let hex: String = chars[i + 1..i + end].iter().collect();
                        let b = hex.as_bytes();
                        let mut k = 0;
                        while k + 4 <= b.len() {
                            if let Ok(v) = u16::from_str_radix(&hex[k..k + 4], 16) {
                                codes.push(v);
                            }
                            k += 4;
                        }
                        i += end;
                    }
                }
                '(' => {
                    // Literal string with PDF escapes.
                    let mut j = i + 1;
                    let mut buf = Vec::new();
                    while j < chars.len() && chars[j] != ')' {
                        if chars[j] == '\\' && j + 1 < chars.len() {
                            j += 1;
                            match chars[j] {
                                'n' => buf.push(b'\n'),
                                'r' => buf.push(b'\r'),
                                't' => buf.push(b'\t'),
                                c if c.is_ascii_digit() => {
                                    // Octal escape \ddd.
                                    let mut v = 0u32;
                                    let mut n = 0;
                                    while n < 3 && j < chars.len() && chars[j].is_ascii_digit() {
                                        v = v * 8 + chars[j].to_digit(8).unwrap();
                                        j += 1;
                                        n += 1;
                                    }
                                    j -= 1;
                                    buf.push(v as u8);
                                }
                                c => buf.push(c as u8),
                            }
                        } else {
                            buf.push(chars[j] as u8);
                        }
                        j += 1;
                    }
                    let mut k = 0;
                    while k + 1 < buf.len() {
                        codes.push(u16::from_be_bytes([buf[k], buf[k + 1]]));
                        k += 2;
                    }
                    i = j;
                }
                _ => {}
            }
            i += 1;
        }
        codes
    }

    /// Manual check: export the fixture PDF to /tmp for poppler
    /// (`cargo test -p dc -- --ignored write_fixture_pdf` then
    /// `pdftoppm -png -r 150 /tmp/dc-smoke/fixture.pdf out`).
    #[test]
    #[ignore = "writes /tmp/dc-smoke for manual inspection"]
    fn write_fixture_pdf_for_manual_inspection() {
        let doc = fixture_doc(1);
        std::fs::create_dir_all("/tmp/dc-smoke").unwrap();
        std::fs::write(
            "/tmp/dc-smoke/fixture.pdf",
            crate::pdf::pdf_bytes(&doc, &[0], false).unwrap(),
        )
        .unwrap();
    }

    #[test]
    fn pdf_page_geometry_matches_the_sheet() {
        let doc = fixture_doc(1);
        let bytes = crate::pdf::pdf_bytes(&doc, &[0], false).unwrap();
        let raw = String::from_utf8_lossy(&bytes);
        // Letter page: 612 × 792 pt.
        assert!(
            raw.contains("/MediaBox [ 0 0 612 792 ]") || raw.contains("[0 0 612 792]"),
            "letter MediaBox in points: {}",
            raw
        );
    }
}
