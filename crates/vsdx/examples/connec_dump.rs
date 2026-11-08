
use std::path::PathBuf;
use vsdx::model::SheetChild;
fn main() -> anyhow::Result<()> {
    let path = PathBuf::from(std::env::args_os().nth(1).unwrap());
    let doc = vsdx::parse::parse(&path)?;
    let want = std::env::args_os().nth(2).unwrap().to_string_lossy().to_string();
    for m in &doc.masters {
        if m.name_u.as_deref() != Some(want.as_str()) { continue; }
        let root = m.shapes.first().unwrap();
        for n in &root.sheet.children {
            if let SheetChild::Cell(name, cell) = n {
                println!("cell {} V={:?} F={:?}", name, cell.v, cell.f);
            }
            if let SheetChild::Section(sec) = n {
                if sec.name == "Scratch" || sec.name == "Controls" {
                    println!("section {} ix={}", sec.name, sec.ix);
                    for r in &sec.rows {
                        let cells: Vec<String> = r.cells.iter().map(|(n, c)| format!("{}={:?} F={:?}", n, c.v, c.f)).collect();
                        println!("  row ix={} n={:?}: {}", r.ix, r.n, cells.join(" "));
                    }
                }
                if sec.name == "Geometry" {
                    println!("section Geometry ix={}", sec.ix);
                    for r in &sec.rows {
                        println!("  row ix={} t={:?}", r.ix, r.t);
                        for (n, c) in &r.cells {
                            println!("    {} V={:?} F={:?}", n, c.v, c.f);
                        }
                    }
                }
            }
        }
    }
    Ok(())
}
