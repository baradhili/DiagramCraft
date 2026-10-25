//! Display units — the GUI presents metric (mm) by default (owner
//! preference 2026-10-16); the model and all edits stay in Visio's
//! internal inches. This module owns every conversion shown to or
//! entered by the user.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unit {
    Mm,
    In,
}

pub const MM_PER_IN: f64 = 25.4;

impl Unit {
    pub fn suffix(self) -> &'static str {
        match self {
            Unit::Mm => "mm",
            Unit::In => "in",
        }
    }

    /// inches → display value
    pub fn from_in(self, inches: f64) -> f64 {
        match self {
            Unit::Mm => inches * MM_PER_IN,
            Unit::In => inches,
        }
    }

    /// display value → inches
    pub fn to_in(self, value: f64) -> f64 {
        match self {
            Unit::Mm => value / MM_PER_IN,
            Unit::In => value,
        }
    }

    /// Format a length for the status bar / rulers, e.g. `215.9 mm`.
    pub fn fmt(self, inches: f64) -> String {
        match self {
            Unit::Mm => format!("{:.1} mm", self.from_in(inches)),
            Unit::In => format!("{:.3} in", inches),
        }
    }

    /// Format a coordinate pair (page coords, inches in) for display.
    pub fn fmt_pt(self, x_in: f64, y_in_up: f64) -> String {
        format!(
            "({:.1}, {:.1}) {}",
            self.from_in(x_in),
            self.from_in(y_in_up),
            self.suffix()
        )
    }
}

/// Ruler tick spacing in display units for a given scale
/// (display units per screen pixel), choosing a 1/2/5 decade.
pub fn tick_step(display_units_per_px: f64, min_px_per_tick: f32) -> f64 {
    let target = (display_units_per_px * min_px_per_tick as f64).max(1e-6);
    let mag = 10f64.powf(target.log10().floor());
    let norm = target / mag;
    let eps = 1e-9;
    let step = if norm <= 1.0 + eps {
        1.0
    } else if norm <= 2.0 + eps {
        2.0
    } else if norm <= 5.0 + eps {
        5.0
    } else {
        10.0
    };
    step * mag
}

/// One ruler tick tier (#62): ticks at `major_step / div` intervals,
/// drawn `len` px long from the ruler's inner edge; numbers appear at
/// the labeled (major) tier only.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TickTier {
    pub div: f64,
    pub len: f32,
    pub labeled: bool,
}

/// Visio-style tiered ruler ticks (#62): the labeled major tier plus
/// half and quarter subdivisions (lengths ≈ 2/3 and 1/3 of the major
/// tick — unit marks tallest and numbered, half marks medium, quarter
/// marks short, all unlabeled). A minor tier is dropped when its pixel
/// spacing would smear (below `min_px`) so zoom-outs degrade
/// gracefully. `subdiv` caps the depth: 4 = halves + quarters (the
/// Visio default), 2 = halves, 1 = majors only — the Ruler & Grid
/// Fine/Normal/Coarse setting can hook in later.
pub fn tick_tiers(major_step: f64, px_per_major: f64, subdiv: u32, min_px: f64) -> Vec<TickTier> {
    debug_assert!(major_step.is_finite() && major_step > 0.0);
    const MAJOR_LEN: f32 = 7.0;
    let mut tiers = vec![TickTier {
        div: 1.0,
        len: MAJOR_LEN,
        labeled: true,
    }];
    for &(div, frac) in &[(2.0_f64, 2.0_f32), (4.0, 1.0)] {
        if div as u32 > subdiv.max(1) || px_per_major / div < min_px {
            continue;
        }
        tiers.push(TickTier {
            div,
            len: MAJOR_LEN * frac / 3.0,
            labeled: false,
        });
    }
    tiers
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mm_round_trip() {
        assert_eq!(Unit::Mm.to_in(Unit::Mm.from_in(8.5)), 8.5);
        assert!((Unit::Mm.from_in(1.0) - 25.4).abs() < 1e-12);
    }

    #[test]
    fn formatting() {
        assert_eq!(Unit::Mm.fmt(1.0), "25.4 mm");
        assert_eq!(Unit::In.fmt(2.0), "2.000 in");
        assert_eq!(Unit::Mm.fmt_pt(1.0, 2.0), "(25.4, 50.8) mm");
    }

    #[test]
    fn ticks_follow_125_decades() {
        assert_eq!(tick_step(0.01, 50.0), 0.5); // 0.5 target → 0.5 (1/2/5 decade)
        assert_eq!(tick_step(0.1, 50.0), 5.0);
        assert_eq!(tick_step(0.5, 50.0), 50.0); // 25 is not a 1/2/5 decade step — rounds to 50
        assert_eq!(tick_step(1.0, 50.0), 50.0);
    }

    #[test]
    fn tick_tiers_tier_presence_and_skipping() {
        let divs = |t: &[TickTier]| t.iter().map(|x| x.div).collect::<Vec<_>>();
        // Default density (majors ≥ 45 px): majors + halves + quarters.
        let t = tick_tiers(10.0, 45.0, 4, 6.0);
        assert_eq!(divs(&t), vec![1.0, 2.0, 4.0]);
        assert!(t[0].labeled, "numbers only at the major tier");
        assert!(!t[1].labeled && !t[2].labeled);
        assert!(
            t[0].len > t[1].len && t[1].len > t[2].len,
            "unit > half > quarter"
        );
        // Proportions per the #62 reference: ~2/3 and ~1/3 of the major.
        assert!((t[1].len - t[0].len * 2.0 / 3.0).abs() < 1e-3);
        assert!((t[2].len - t[0].len / 3.0).abs() < 1e-3);
        // Coarser subdivision depth: halves only / majors only.
        assert_eq!(divs(&tick_tiers(10.0, 45.0, 2, 6.0)), vec![1.0, 2.0]);
        assert_eq!(divs(&tick_tiers(10.0, 45.0, 1, 6.0)), vec![1.0]);
        // Zoom-outs drop tiers whose spacing smears: 20 px per major →
        // quarters at 5 px < 6 px floor; 10 px → halves drop too.
        assert_eq!(divs(&tick_tiers(10.0, 20.0, 4, 6.0)), vec![1.0, 2.0]);
        assert_eq!(divs(&tick_tiers(10.0, 10.0, 4, 6.0)), vec![1.0]);
    }
}
