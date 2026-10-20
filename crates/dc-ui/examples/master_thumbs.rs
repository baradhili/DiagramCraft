//! Dump every master of a package as a PNG through the exact GUI
//! thumbnail pipeline (`dc::svg::render_master` + `svgimg::rasterize`).
//! Debug tool for the stencil pane; run:
//!   cargo run -p dc-ui --example master_thumbs -- <file> <outdir>

use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let file = PathBuf::from(args.next().expect("usage: master_thumbs <file> <outdir>"));
    let outdir = PathBuf::from(args.next().expect("usage: master_thumbs <file> <outdir>"));
    std::fs::create_dir_all(&outdir)?;

    let doc = vsdx::parse::parse(&file)?;
    let mut masters: Vec<&vsdx::Master> = doc.masters.iter().collect();
    masters.sort_by_key(|m| m.id);
    for m in masters {
        let svg = dc::svg::render_master(&doc, m)?;
        let stem = format!(
            "{:03}-{}",
            m.id,
            m.name_u.as_deref().unwrap_or("-").replace(['/', '\\', ' '], "_")
        );
        std::fs::write(outdir.join(format!("{stem}.svg")), &svg)?;
        let img = dc_ui::svgimg::rasterize(&svg, 128.0)?;
        let png = resvg::tiny_skia::Pixmap::from_vec(
            img.pixels
                .iter()
                .flat_map(|p| p.to_array().to_vec())
                .collect(),
            resvg::tiny_skia::IntSize::from_wh(img.size[0] as u32, img.size[1] as u32)
                .ok_or_else(|| anyhow::anyhow!("bad pixmap size"))?,
        )
        .ok_or_else(|| anyhow::anyhow!("pixmap from vec failed"))?;
        std::fs::write(outdir.join(format!("{stem}.png")), png.encode_png()?)?;
        println!("{} {}", m.id, m.name_u.as_deref().unwrap_or("-"));
    }
    Ok(())
}
