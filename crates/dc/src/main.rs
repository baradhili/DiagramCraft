//! dc — DiagramCraft command line.
//!
//! Subcommands:
//!   dc info      <file>              package/part inventory
//!   dc inspect   <file>              pages, shapes, resolved placement cells
//!   dc render    <file> -o out.svg   render every page to SVG
//!   dc roundtrip <file> -o out.vsdx  re-write the package from the model
//!   dc edit      <file> -o out.vsdx  apply undoable edits, save (M4)

use dc::{edit, svg};

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(name = "dc", version, about = "DiagramCraft — Linux Visio (VSDX) toolchain")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Show OPC package inventory
    Info { file: PathBuf },
    /// Show the parsed document model
    Inspect { file: PathBuf },
    /// Render pages to SVG
    Render {
        file: PathBuf,
        /// Output file (page N appends `-N` for multi-page docs)
        #[arg(short, long, default_value = "out.svg")]
        out: PathBuf,
        /// 1-based page number
        #[arg(long)]
        page: Option<usize>,
        /// Render the stencil's masters as a contact sheet instead of pages
        #[arg(long)]
        masters: bool,
    },
    /// Re-write the package from the parsed model (lossless round-trip)
    Roundtrip {
        file: PathBuf,
        /// Output package path
        #[arg(short, long, default_value = "out.vsdx")]
        out: PathBuf,
        /// Verify the result: part inventory, canonical XML of re-serialized
        /// parts, byte equality of untouched parts, model and render equality
        #[arg(long)]
        verify: bool,
    },
    /// Edit a document through the command layer (undo/redo history)
    Edit {
        file: PathBuf,
        /// Output package path
        #[arg(short, long, default_value = "edited.vsdx")]
        out: PathBuf,
        /// Set a cell value: TARGET NAME VALUE (e.g. page:0/shape:1 LineWeight 0.02)
        #[arg(long, num_args = 3, value_names = ["TARGET", "NAME", "VALUE"])]
        cell: Vec<String>,
        /// Set a cell formula: TARGET NAME FORMULA
        #[arg(long, num_args = 3, value_names = ["TARGET", "NAME", "FORMULA"])]
        formula: Vec<String>,
        /// Set a shape's text: TARGET TEXT
        #[arg(long, num_args = 2, value_names = ["TARGET", "TEXT"])]
        text: Vec<String>,
        /// Add a placeholder shape to a page (page ID)
        #[arg(long, value_name = "PAGE_ID")]
        add_shape: Vec<i64>,
        /// Remove a shape: PAGE_ID SHAPE_ID
        #[arg(long, num_args = 2, value_names = ["PAGE_ID", "SHAPE_ID"])]
        remove_shape: Vec<i64>,
        /// Append a page (name)
        #[arg(long, value_name = "NAME")]
        add_page: Vec<String>,
        /// Remove a page (page ID)
        #[arg(long, value_name = "PAGE_ID")]
        remove_page: Vec<i64>,
        /// Rename a page: PAGE_ID NEW_NAME
        #[arg(long, num_args = 2, value_names = ["PAGE_ID", "NEW_NAME"])]
        rename_page: Vec<String>,
        /// Move a page: FROM TO (1-based positions)
        #[arg(long, num_args = 2, value_names = ["FROM", "TO"])]
        move_page: Vec<usize>,
        /// Add a layer to a page: PAGE_ID NAME
        #[arg(long, num_args = 2, value_names = ["PAGE_ID", "NAME"])]
        add_layer: Vec<String>,
        /// Remove a layer row: PAGE_ID IX
        #[arg(long, num_args = 2, value_names = ["PAGE_ID", "IX"])]
        remove_layer: Vec<i64>,
        /// Rename a layer: PAGE_ID IX NEW_NAME
        #[arg(long, num_args = 3, value_names = ["PAGE_ID", "IX", "NEW_NAME"])]
        rename_layer: Vec<String>,
        /// Add a connection: OWNER FROM_SHEET FROM_CELL FROM_PART TO_SHEET TO_CELL TO_PART
        /// (OWNER is `page:N`, `master:N`, or a bare page ID)
        #[arg(long, num_args = 7, value_names = ["OWNER", "FROM_SHEET", "FROM_CELL", "FROM_PART", "TO_SHEET", "TO_CELL", "TO_PART"])]
        connect: Vec<String>,
        /// Remove a connection: OWNER INDEX
        #[arg(long, num_args = 2, value_names = ["OWNER", "INDEX"])]
        disconnect: Vec<String>,
        /// Undo the N most recent edits before saving (repeatable)
        #[arg(long, action = clap::ArgAction::Count)]
        undo: u8,
        /// Print the command history
        #[arg(long)]
        history: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Info { file } => info(&file),
        Cmd::Inspect { file } => inspect(&file),
        Cmd::Render { file, out, page, masters } => render(&file, &out, page, masters),
        Cmd::Roundtrip { file, out, verify } => roundtrip(&file, &out, verify),
        Cmd::Edit {
            file,
            out,
            cell,
            formula,
            text,
            add_shape,
            remove_shape,
            add_page,
            remove_page,
            rename_page,
            move_page,
            add_layer,
            remove_layer,
            rename_layer,
            connect,
            disconnect,
            undo,
            history,
        } => {
            let triples = |v: &[String]| -> Vec<(String, String, String)> {
                v.chunks(3)
                    .map(|c| (c[0].clone(), c[1].clone(), c[2].clone()))
                    .collect()
            };
            let pairs_i64 = |v: &[i64]| -> Vec<(i64, i64)> {
                v.chunks(2).map(|c| (c[0], c[1])).collect()
            };
            let parse = |s: &str, what: &str| -> Result<i64> {
                s.parse::<i64>().with_context(|| format!("{what}: {s:?} is not a number"))
            };
            let args = edit::EditArgs {
                cell: triples(&cell),
                formula: triples(&formula),
                text: text
                    .chunks(2)
                    .map(|c| (c[0].clone(), c[1].clone()))
                    .collect(),
                add_shape,
                remove_shape: pairs_i64(&remove_shape),
                add_page,
                remove_page,
                rename_page: rename_page
                    .chunks(2)
                    .map(|c| Ok((parse(&c[0], "PAGE_ID")?, c[1].clone())))
                    .collect::<Result<Vec<_>>>()?,
                move_page: move_page.chunks(2).map(|c| (c[0], c[1])).collect(),
                add_layer: add_layer
                    .chunks(2)
                    .map(|c| Ok((parse(&c[0], "PAGE_ID")?, c[1].clone())))
                    .collect::<Result<Vec<_>>>()?,
                remove_layer: pairs_i64(&remove_layer),
                rename_layer: rename_layer
                    .chunks(3)
                    .map(|c| {
                        Ok((
                            parse(&c[0], "PAGE_ID")?,
                            parse(&c[1], "IX")?,
                            c[2].clone(),
                        ))
                    })
                    .collect::<Result<Vec<_>>>()?,
                connect: connect
                    .chunks(7)
                    .map(|c| {
                        Ok(edit::ConnectSpec {
                            owner: edit::connect_owner(&c[0])?,
                            from_sheet: parse(&c[1], "FROM_SHEET")?,
                            from_cell: c[2].clone(),
                            from_part: parse(&c[3], "FROM_PART")?,
                            to_sheet: parse(&c[4], "TO_SHEET")?,
                            to_cell: c[5].clone(),
                            to_part: parse(&c[6], "TO_PART")?,
                        })
                    })
                    .collect::<Result<Vec<_>>>()?,
                disconnect: disconnect
                    .chunks(2)
                    .map(|c| Ok((edit::connect_owner(&c[0])?, parse(&c[1], "INDEX")?)))
                    .collect::<Result<Vec<_>>>()?,
                undo_count: undo,
                print_history: history,
            };
            edit::run(&file, &out, args)
        }
    }
}

fn info(path: &Path) -> Result<()> {
    let pkg = vsdx::Package::open(path).context("opening package")?;
    println!("OPC package: {}", path.display());
    for name in pkg.part_names() {
        let ct = pkg.content_type_of(name).unwrap_or_default();
        println!("  {name:<50} {ct}");
    }
    Ok(())
}

fn inspect(path: &Path) -> Result<()> {
    let doc = vsdx::parse::parse(path)?;
    println!(
        "document: {} page(s), {} master(s), {} stylesheet(s), {} color(s)",
        doc.pages.len(),
        doc.masters.len(),
        doc.stylesheets.len(),
        doc.colors.len()
    );
    for m in &doc.masters {
        println!(
            "  master {} {:?} ({} root shape(s)) [{}]",
            m.id,
            m.name_u.as_deref().unwrap_or("-"),
            m.shapes.len(),
            m.part
        );
    }
    for (i, page) in doc.pages.iter().enumerate() {
        let w = page.sheet().and_then(|s| s.cell("PageWidth")).and_then(|c| c.numeric());
        let h = page.sheet().and_then(|s| s.cell("PageHeight")).and_then(|c| c.numeric());
        println!(
            "page {} ({}): {:?} — {:?} × {:?} in, {} top-level shape(s)",
            i + 1,
            page.name,
            page.part,
            w.unwrap_or(0.0),
            h.unwrap_or(0.0),
            page.shapes.len()
        );
        for s in &page.shapes {
            print_shape(&doc, s, &[], 2);
        }
    }
    Ok(())
}

fn print_shape(doc: &vsdx::Document, s: &vsdx::Shape, chain: &[&vsdx::Shape], indent: usize) {
    let pad = " ".repeat(indent);
    let mut chain: Vec<&vsdx::Shape> = chain.to_vec();
    chain.push(s);
    let pin = |n: &str| {
        doc.resolve_cell(s, &chain, n)
            .and_then(|c| c.numeric())
            .map(|v| format!("{v:.3}"))
            .unwrap_or_else(|| "?".into())
    };
    let n_geo = doc
        .effective_sheet(s, &chain)
        .sections_named("Geometry")
        .len();
    println!(
        "{pad}shape {} {:?} type={:?} master={:?} pin=({},{}) size=({},{}) geom={} text={:?}",
        s.id,
        s.name_u.as_deref().unwrap_or("-"),
        s.type_.as_deref().unwrap_or("-"),
        s.master,
        pin("PinX"),
        pin("PinY"),
        pin("Width"),
        pin("Height"),
        n_geo,
        s.sheet.text.as_deref().map(|t| {
            let t = t.trim();
            if t.len() > 24 {
                format!("{}…", &t[..24])
            } else {
                t.to_string()
            }
        })
        .unwrap_or_default()
    );
    for sub in &s.sub_shapes {
        print_shape(doc, sub, &chain, indent + 2);
    }
}

fn render(path: &Path, out: &Path, page_no: Option<usize>, masters: bool) -> Result<()> {
    let doc = vsdx::parse::parse(path)?;
    let svg = if masters {
        if doc.masters.is_empty() {
            bail!("document has no masters");
        }
        svg::render_masters(&doc)?
    } else {
        if doc.pages.is_empty() {
            bail!("document has no pages");
        }
        let idx = match page_no {
            Some(n) if n >= 1 && n <= doc.pages.len() => n - 1,
            Some(n) => bail!("page {n} out of range (1..={})", doc.pages.len()),
            None => 0,
        };
        svg::render_page(&doc, &doc.pages[idx])?
    };
    std::fs::write(out, svg).with_context(|| format!("writing {}", out.display()))?;
    println!("wrote {}", out.display());
    Ok(())
}

fn roundtrip(path: &Path, out: &Path, verify: bool) -> Result<()> {
    let doc = vsdx::parse::parse(path)?;
    vsdx::write::save(&doc, out)?;
    println!("wrote {}", out.display());
    if verify {
        verify_roundtrip(&doc, path, out)?;
    }
    Ok(())
}

/// Round-trip verification per the v0.2 definition of done: identical part
/// inventory; canonical-XML equality for re-serialized parts; byte equality
/// for verbatim parts; equal re-parsed model; identical rendered output.
fn verify_roundtrip(source: &vsdx::Document, src_path: &Path, out_path: &Path) -> Result<()> {
    let src_pkg = source.package.as_ref().context("source package missing")?;
    let out_pkg = vsdx::Package::open(out_path)?;
    let modeled: Vec<String> = vsdx::write::modeled_parts(source);

    let src_names: Vec<&str> = src_pkg.part_names();
    let out_names: Vec<&str> = out_pkg.part_names();
    if src_names != out_names {
        bail!(
            "part inventory differs: missing {:?}, extra {:?}",
            src_names
                .iter()
                .filter(|n| !out_names.contains(n))
                .collect::<Vec<_>>(),
            out_names
                .iter()
                .filter(|n| !src_names.contains(n))
                .collect::<Vec<_>>()
        );
    }

    let mut mismatches = Vec::new();
    for name in &src_names {
        let (a, b) = (src_pkg.part(name).unwrap(), out_pkg.part(name).unwrap());
        if modeled.iter().any(|m| m == name) {
            let (ca, cb) = (vsdx::canonical_form(a), vsdx::canonical_form(b));
            if ca != cb {
                mismatches.push(format!("{name}: canonical XML differs"));
            }
        } else if a != b {
            mismatches.push(format!("{name}: bytes differ (should be verbatim)"));
        }
    }
    if let Some(first) = mismatches.first() {
        bail!(
            "round-trip verification failed ({} part(s) differ): {}",
            mismatches.len(),
            first
        );
    }

    let reparsed = vsdx::parse::parse(out_path)?;
    if !source.same_model(&reparsed) {
        bail!("re-parsed model differs from source model");
    }

    // Render equality: same picture in, same picture out.
    for (i, (a, b)) in source.pages.iter().zip(&reparsed.pages).enumerate() {
        let sa = svg::render_page(source, a)?;
        let sb = svg::render_page(&reparsed, b)?;
        if sa != sb {
            bail!("rendered page {} differs between source and round-trip", i + 1);
        }
    }
    if !source.masters.is_empty()
        && svg::render_masters(source)? != svg::render_masters(&reparsed)?
    {
        bail!("rendered master contact sheet differs between source and round-trip");
    }

    println!(
        "verify PASS: {} parts ({} re-serialized, {} verbatim), model and renders equal — {}",
        src_names.len(),
        modeled.len(),
        src_names.len() - modeled.len(),
        src_path.display()
    );
    Ok(())
}
