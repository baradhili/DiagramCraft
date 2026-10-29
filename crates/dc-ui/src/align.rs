//! Align / distribute / format-painter math (GitHub #110, FS-016):
//! pure page-space (inches, y-up) computations the app turns into
//! move commands — URS-1149 (align left/center/right/top/middle/
//! bottom against the first-selected shape or the page), URS-1161
//! (distribute with even gaps over 3+ shapes), URS-1053 (the painter
//! cell set).

/// One alignment axis-anchored command (URS-1149).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlignKind {
    Left,
    HCenter,
    Right,
    Top,
    VCenter,
    Bottom,
}

impl AlignKind {
    /// Horizontal kinds move x; vertical kinds move y.
    pub fn horizontal(self) -> bool {
        matches!(self, AlignKind::Left | AlignKind::HCenter | AlignKind::Right)
    }

    /// The alignment feature of a rect for this kind: the x or y
    /// coordinate the shapes line up on.
    pub fn feature(self, r: (f64, f64, f64, f64)) -> f64 {
        match self {
            AlignKind::Left => r.0,
            AlignKind::HCenter => r.0 + r.2 / 2.0,
            AlignKind::Right => r.0 + r.2,
            AlignKind::Top => r.1 + r.3,
            AlignKind::VCenter => r.1 + r.3 / 2.0,
            AlignKind::Bottom => r.1,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            AlignKind::Left => "left",
            AlignKind::HCenter => "centers",
            AlignKind::Right => "right",
            AlignKind::Top => "top",
            AlignKind::VCenter => "middles",
            AlignKind::Bottom => "bottom",
        }
    }
}

/// What the shapes align to (URS-1149's option): the first-selected
/// shape's feature, or the page edge/center line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlignTarget {
    FirstSelected,
    Page,
}

/// Per-shape pin deltas for an align: `(shape id, (dx, dy))` — one
/// component is always 0 (each kind moves one axis). `first` is the
/// first-selected shape (aligned to itself: zero delta).
pub fn align_moves(
    shapes: &[(i64, (f64, f64, f64, f64))],
    kind: AlignKind,
    target: AlignTarget,
    page: (f64, f64),
) -> Vec<(i64, (f64, f64))> {
    let Some(&(_, first_rect)) = shapes.first() else {
        return Vec::new();
    };
    let goal = match target {
        AlignTarget::FirstSelected => kind.feature(first_rect),
        AlignTarget::Page => match kind {
            AlignKind::Left => 0.0,
            AlignKind::HCenter => page.0 / 2.0,
            AlignKind::Right => page.0,
            AlignKind::Top => page.1,
            AlignKind::VCenter => page.1 / 2.0,
            AlignKind::Bottom => 0.0,
        },
    };
    shapes
        .iter()
        .map(|&(id, r)| {
            let d = goal - kind.feature(r);
            (id, if kind.horizontal() { (d, 0.0) } else { (0.0, d) })
        })
        .collect()
}

/// Distribute with even gaps between boundaries (Visio's "make
/// spacing equal", URS-1161): sorted along the axis, first and last
/// shapes stay, the gaps between all adjacent edges equalize.
/// Returns per-shape pin deltas; `None` unless 3+ shapes.
pub fn distribute_moves(
    shapes: &[(i64, (f64, f64, f64, f64))],
    vertical: bool,
) -> Option<Vec<(i64, (f64, f64))>> {
    if shapes.len() < 3 {
        return None;
    }
    let extent =
        |r: (f64, f64, f64, f64)| if vertical { (r.1, r.3) } else { (r.0, r.2) };
    // order by leading edge; ids ride along for the result mapping
    let mut order: Vec<(i64, (f64, f64))> =
        shapes.iter().map(|(id, r)| (*id, extent(*r))).collect();
    order.sort_by(|a, b| a.1 .0.total_cmp(&b.1 .0));
    let sizes: f64 = order.iter().map(|(_, e)| e.1).sum();
    let first = order.first().unwrap().1 .0;
    let last_edge = order.last().map(|(_, e)| e.0 + e.1).unwrap_or(0.0);
    let gap = ((last_edge - first) - sizes) / (order.len() - 1) as f64;
    let mut cursor = first;
    let mut out = Vec::with_capacity(order.len());
    for (id, (lead, size)) in &order {
        let want = cursor;
        let d = want - lead;
        out.push((
            *id,
            if vertical { (0.0, d) } else { (d, 0.0) },
        ));
        cursor = want + size + gap;
    }
    Some(out)
}

/// The visual-formatting cells the Format Painter copies (URS-1053):
/// line, fill and arrowhead styling, copied as resolved plain values.
pub const FORMAT_CELLS: &[&str] = &[
    "LineWeight",
    "LineColor",
    "LinePattern",
    "LineCap",
    "BeginArrow",
    "EndArrow",
    "EndArrowSize",
    "FillForegnd",
    "FillBkgnd",
    "FillPattern",
];

#[cfg(test)]
mod tests {
    use super::*;

    /// left, bottom, w, h.
    fn r(x: f64, y: f64, w: f64, h: f64) -> (f64, f64, f64, f64) {
        (x, y, w, h)
    }

    #[test]
    fn align_left_to_first_selected() {
        let shapes = [(1, r(0.0, 0.0, 1.0, 1.0)), (2, r(3.0, 1.0, 1.0, 1.0))];
        let moves = align_moves(&shapes, AlignKind::Left, AlignTarget::FirstSelected, (8.5, 11.0));
        assert_eq!(moves[0], (1, (0.0, 0.0)), "first-selected anchors");
        assert_eq!(moves[1], (2, (-3.0, 0.0)));
    }

    #[test]
    fn align_right_to_page() {
        let shapes = [(1, r(0.0, 0.0, 1.0, 1.0)), (2, r(3.0, 1.0, 2.0, 1.0))];
        let moves = align_moves(&shapes, AlignKind::Right, AlignTarget::Page, (8.5, 11.0));
        // 1's right edge 1 → 8.5; 2's right edge 5 → 8.5.
        assert!((moves[0].1 .0 - 7.5).abs() < 1e-9);
        assert!((moves[1].1 .0 - 3.5).abs() < 1e-9);
        assert_eq!(moves[0].1 .1, 0.0, "horizontal kinds leave y alone");
    }

    #[test]
    fn align_vertical_centers_to_first() {
        let shapes = [(1, r(0.0, 4.0, 1.0, 2.0)), (2, r(3.0, 0.0, 1.0, 1.0))];
        // 1's vcenter = 5; 2's = 0.5 → dy = 4.5.
        let moves = align_moves(&shapes, AlignKind::VCenter, AlignTarget::FirstSelected, (8.5, 11.0));
        assert!((moves[1].1 .1 - 4.5).abs() < 1e-9);
        assert_eq!(moves[1].1 .0, 0.0);
    }

    #[test]
    fn align_top_uses_y_up_top() {
        let shapes = [(1, r(0.0, 5.0, 1.0, 1.0)), (2, r(0.0, 0.0, 1.0, 3.0))];
        // tops: 6 vs 3 → dy = 3.
        let moves = align_moves(&shapes, AlignKind::Top, AlignTarget::FirstSelected, (8.5, 11.0));
        assert!((moves[1].1 .1 - 3.0).abs() < 1e-9);
    }

    #[test]
    fn distribute_makes_equal_gaps() {
        // Three 1-inch boxes at leads 0, 1.5, 4.0 (gaps 0.5 and 1.5):
        // span 5.0 − sizes 3 = 2.0 → gap 1.0 each. First and last
        // stay; the middle moves to lead 2.0.
        let shapes = [
            (1, r(0.0, 0.0, 1.0, 1.0)),
            (2, r(1.5, 0.0, 1.0, 1.0)),
            (3, r(4.0, 0.0, 1.0, 1.0)),
        ];
        let moves =
            distribute_moves(&shapes, false).expect("3 shapes distribute");
        let m: std::collections::HashMap<i64, f64> =
            moves.iter().map(|(id, (dx, _))| (*id, *dx)).collect();
        assert_eq!(m[&1], 0.0, "first stays");
        assert_eq!(m[&3], 0.0, "last stays");
        assert!((m[&2] - 0.5).abs() < 1e-9, "middle evens the gaps");
        // The resulting gaps are equal.
        let leads = [0.0, 1.5 + m[&2], 4.0 + m[&3]];
        let gaps = [leads[1] - 1.0 - leads[0], leads[2] - 1.0 - leads[1]];
        assert!((gaps[0] - gaps[1]).abs() < 1e-9 && gaps[0] > 0.0);
    }

    #[test]
    fn distribute_vertical_and_guard() {
        // Vertical: leads 0, 4, 9 with heights 2, 1, 1 — span 10 −
        // sizes 4 = 6 → gap 3. First/last stay; middle to lead 5.
        let shapes = [
            (1, r(0.0, 0.0, 1.0, 2.0)),
            (2, r(0.0, 4.0, 1.0, 1.0)),
            (3, r(0.0, 9.0, 1.0, 1.0)),
        ];
        let moves = distribute_moves(&shapes, true).expect("3 shapes");
        let m: std::collections::HashMap<i64, f64> =
            moves.iter().map(|(id, (_, dy))| (*id, *dy)).collect();
        assert_eq!(m[&1], 0.0);
        assert_eq!(m[&3], 0.0);
        assert!((m[&2] - 1.0).abs() < 1e-9, "middle pulls down to 5");
        // Under three: refused.
        assert!(distribute_moves(&shapes[..2], false).is_none());
    }
}
