//! Rasterize an SVG to PNG. Args: <in.svg> <out.png> [width-px].
//! The default 1200 px approximates screen resolution; Visio's own PNG
//! export uses printer resolution, so pass e.g. 3300 for a comparable
//! 300-dpi letter-width raster when doing ground-truth comparisons.
fn main() {
    let mut args = std::env::args().skip(1);
    let svg = std::fs::read_to_string(args.next().unwrap()).unwrap();
    let out = args.next().unwrap();
    let width: f32 = args.next().map_or(1200.0, |s| s.parse().unwrap());
    let img = dc_ui::svgimg::rasterize(&svg, width).unwrap();
    let png = resvg::tiny_skia::Pixmap::from_vec(
        img.pixels
            .iter()
            .flat_map(|p| p.to_srgba_unmultiplied().as_slice().to_vec())
            .collect(),
        resvg::tiny_skia::IntSize::from_wh(img.size[0] as u32, img.size[1] as u32).unwrap(),
    )
    .unwrap();
    std::fs::write(out, png.encode_png().unwrap()).unwrap();
}
