//! Dev helper: print the first canonical-form difference of one part
//! between two packages. Usage: `canondiff <a.vsdx> <b.vsdx> <part-name>`.

use std::path::Path;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (a_path, b_path, part) = (&args[1], &args[2], &args[3]);
    let a = vsdx::Package::open(Path::new(a_path)).unwrap();
    let b = vsdx::Package::open(Path::new(b_path)).unwrap();
    let ca = vsdx::canonical_form(a.part(part).expect("part in a"));
    let cb = vsdx::canonical_form(b.part(part).expect("part in b"));
    if ca == cb {
        println!("{part}: canonically equal ({} bytes)", ca.len());
        return;
    }
    let (mut i, mut j) = (0usize, 0usize);
    let ab = ca.as_bytes();
    let bb = cb.as_bytes();
    while i < ab.len() && j < bb.len() && ab[i] == bb[j] {
        i += 1;
        j += 1;
    }
    let ctx = 220;
    println!("{part}: differs at a:{i} b:{j}");
    println!("  A …{}", &ca[i.saturating_sub(60)..(i + ctx).min(ca.len())]);
    println!("  B …{}", &cb[j.saturating_sub(60)..(j + ctx).min(cb.len())]);
}
