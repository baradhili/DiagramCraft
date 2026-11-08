//! #238 comment-2 probe: does the line-jump pass hit one intersection
//! more than once? Two suspects: a crossing that lands exactly on a
//! polyline VERTEX of either line (it lives in two segment windows),
//! and a T-touch (one line's endpoint on the other's midline).
//!
//! cargo run -p dc-ui --example jump_probe

use dc_ui::connectors::{line_jumps, JumpLine};

fn l(id: i64, z: usize, pts: &[(f64, f64)]) -> JumpLine {
    JumpLine {
        id,
        z,
        weight: 0.01,
        color: (0, 0, 0),
        pts: pts.to_vec(),
    }
}

fn main() {
    // 1. Plain crossing: exactly one jump expected.
    let js = line_jumps(&[
        l(1, 0, &[(0.0, 1.0), (2.0, 1.0)]),
        l(2, 1, &[(1.0, 0.0), (1.0, 2.0)]),
    ]);
    println!("plain cross: {} jump(s) at {:?}", js.len(), js.iter().map(|j| j.at).collect::<Vec<_>>());

    // 2. The hopped line's crossing point sits exactly on ITS OWN
    //    polyline vertex (the elbow corner of a Z-route): the crossing
    //    lives in two windows of line 1.
    let js = line_jumps(&[
        l(1, 0, &[(0.0, 1.0), (1.0, 1.0), (1.0, 3.0)]), // vertex at (1,1)
        l(2, 1, &[(0.5, 2.0), (1.5, 0.5)]),             // crosses at (1,1)? -> through the vertex
    ]);
    println!("cross at a vertex of the hopped line: {} jump(s) at {:?}",
        js.len(), js.iter().map(|j| j.at).collect::<Vec<_>>());

    // 3. Crossing exactly at a vertex of the UNDER line.
    let js = line_jumps(&[
        l(1, 0, &[(0.0, 1.0), (2.0, 1.0)]),
        l(2, 1, &[(1.0, 0.0), (1.0, 1.0), (2.5, 1.0)]), // under line bends at (1,1)
    ]);
    println!("cross at a vertex of the under line: {} jump(s) at {:?}",
        js.len(), js.iter().map(|j| j.at).collect::<Vec<_>>());

    // 4. T-touch: a connector endpoint ON the other line's midline.
    let js = line_jumps(&[
        l(1, 0, &[(0.0, 1.0), (2.0, 1.0)]),
        l(2, 1, &[(1.0, 1.0), (1.0, 3.0)]), // begins ON line 1
    ]);
    println!("T-touch: {} jump(s) at {:?}", js.len(), js.iter().map(|j| j.at).collect::<Vec<_>>());

    // 6. Two FLATTENED ARCS crossing once (the corpus arc connectors'
    //    sampled polylines): adjacent sample windows may both straddle
    //    the crossing — the screenshot's repeated-hop suspect.
    let arc = |cx: f64, cy: f64, r: f64, n: usize| -> Vec<(f64, f64)> {
        (0..=n)
            .map(|i| {
                let t = std::f64::consts::PI * (i as f64) / (n as f64);
                (cx - r * t.cos(), cy + r * t.sin())
            })
            .collect()
    };
    let a = arc(1.0, 0.0, 1.0, 24); // semicircle over (0..2, y>=0)
    let b = arc(2.0, 0.0, 1.0, 24); // shifted: the two bellies cross near (1, 0.87)
    let js = line_jumps(&[l(1, 0, &a), l(2, 1, &b)]);
    println!("two flattened arcs: {} jump(s) at {:?}",
        js.len(), js.iter().map(|j| (format!("{:.4}", j.at.0), format!("{:.4}", j.at.1))).collect::<Vec<_>>());

    // 7. A straight line crossing a flattened arc whose samples wrap
    //    the crossing point.
    let js = line_jumps(&[l(1, 0, &a), l(2, 1, &[(0.5, 0.9), (1.5, 0.9)])]);
    println!("line vs arc belly: {} jump(s) at {:?}",
        js.len(), js.iter().map(|j| (format!("{:.4}", j.at.0), format!("{:.4}", j.at.1))).collect::<Vec<_>>());

    // 5. Two Z-routes (the right-angle planner's shape) crossing twice.
    let js = line_jumps(&[
        l(1, 0, &[(0.0, 0.0), (1.0, 0.0), (1.0, 3.0), (3.0, 3.0)]),
        l(2, 1, &[(0.0, 2.0), (2.0, 2.0), (2.0, 0.5), (4.0, 0.5)]),
    ]);
    println!("two Z-routes: {} jump(s) at {:?}",
        js.len(), js.iter().map(|j| j.at).collect::<Vec<_>>());
}
