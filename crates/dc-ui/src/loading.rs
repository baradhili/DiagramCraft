//! Startup splash and file-load feedback (#224), drawn from the owner's
//! Tarahi brand kit — the **light theme set** attached to the issue
//! (`tarahi-logo-light` / `tarahi-splash-light` /
//! `tarahi-file-loading-light`): the network mark on a cool-white
//! gradient, violet/sky connectors, white document cards.
//!
//! Both overlays are procedurally animated and time-driven (a start
//! `Instant`; no GIF/video assets, no scripts). The kit's 3.2 s splash
//! choreography is compressed into [`SPLASH_SECS`] (same phase order:
//! mark draw-in → cards → wordmark rise → caption); the loader keeps
//! the kit's bob/twinkle/progress periods verbatim.
//!
//! Headless determinism: the ctors pin no splash and tests land loads
//! synchronously, so kittest never sees either overlay unless it asks.

use std::time::{Duration, Instant};

use egui::{Color32, Pos2, Rect, Sense, Shape, Stroke, Vec2};

/// The light set's palette (tarahi-splash-light.svg / logo-light).
const BG_TOP: Color32 = Color32::from_rgb(0xEE, 0xF3, 0xF8);
const BG_BOT: Color32 = Color32::from_rgb(0xF7, 0xFA, 0xFC);
const INK: Color32 = Color32::from_rgb(0x1A, 0x24, 0x44);
const MUTED: Color32 = Color32::from_rgb(0x5A, 0x63, 0x78);
const VIOLET: Color32 = Color32::from_rgb(0x68, 0x63, 0xE7);
const VIOLET_LT: Color32 = Color32::from_rgb(0x8B, 0x87, 0xF0);
const SKY: Color32 = Color32::from_rgb(0x2D, 0xA9, 0xE2);
const SKY_DEEP: Color32 = Color32::from_rgb(0x1A, 0x8F, 0xC4);
const SKY_LT: Color32 = Color32::from_rgb(0x7F, 0xD4, 0xF8);
const CARD: Color32 = Color32::from_rgb(0xFF, 0xFF, 0xFF);
const CARD_STROKE: Color32 = Color32::from_rgb(0xC2, 0xCC, 0xDA);
const FOLD: Color32 = Color32::from_rgb(0xE6, 0xEC, 0xF3);
const DOT: Color32 = Color32::from_rgb(0xAA, 0xB4, 0xC6);

/// How long the splash shows before handing off to the shell.
pub const SPLASH_SECS: f32 = 1.6;

fn ease_out(t: f32) -> f32 {
    1.0 - (1.0 - t.clamp(0.0, 1.0)).powi(3)
}

fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let (ar, ag, ab, _) = a.to_tuple();
    let (br, bg, bb, _) = b.to_tuple();
    Color32::from_rgb(
        (ar as f32 + (br as f32 - ar as f32) * t) as u8,
        (ag as f32 + (bg as f32 - ag as f32) * t) as u8,
        (ab as f32 + (bb as f32 - ab as f32) * t) as u8,
    )
}

/// One corner card of the network mark (logo-light geometry, offsets
/// from the mark's center in the 300-space, scaled by half/150).
struct Card {
    at: Vec2,
    glyph: Glyph,
}

#[derive(Clone, Copy)]
enum Glyph {
    /// The violet accent card: a white data dot (the kit's last card).
    Dot,
    /// A sky square on the white card (the data node).
    Square,
    /// A sky check (the done node).
    Check,
    /// A dashed sky run across the card (the process node).
    Dash,
}

const CARDS: [Card; 4] = [
    Card {
        at: Vec2::new(-90.0, 0.0),
        glyph: Glyph::Dash,
    },
    Card {
        at: Vec2::new(88.0, -82.0),
        glyph: Glyph::Square,
    },
    Card {
        at: Vec2::new(-88.0, 82.0),
        glyph: Glyph::Check,
    },
    Card {
        at: Vec2::new(88.0, 82.0),
        glyph: Glyph::Dot,
    },
];

/// The connectors: (from-center, to-card-center, violet, dashed). The
/// violet stroke runs to the left card; the rest are sky, one dashed,
/// per logo-light.
const CONNECTORS: [(Vec2, Vec2, bool, bool); 5] = [
    (Vec2::new(-40.0, 0.0), Vec2::new(-62.0, 0.0), true, false),
    (Vec2::new(20.0, -20.0), Vec2::new(88.0, -63.0), false, false),
    (Vec2::new(-20.0, 20.0), Vec2::new(-88.0, 63.0), false, false),
    (Vec2::new(20.0, 20.0), Vec2::new(88.0, 63.0), false, true),
    (Vec2::new(0.0, -40.0), Vec2::new(0.0, -112.0), false, false),
];

/// Draw the network mark centered at `c` with half-size `half`, at
/// animation time `t` seconds.
fn draw_mark(p: &egui::Painter, c: Pos2, half: f32, t: f32) {
    let s = half / 150.0;
    // Connectors: grow from the center outward.
    let conn_t = ease_out((t / (SPLASH_SECS * 0.42)).clamp(0.0, 1.0));
    for (from, to, violet, dashed) in CONNECTORS {
        let a = c + from * s;
        let b = c + (from + (to - from) * conn_t) * s;
        let col = if violet { VIOLET } else { SKY_DEEP };
        let stroke = Stroke::new(2.6 * s, col);
        if dashed {
            for seg in egui::Shape::dashed_line(&[a, b], stroke, 5.0 * s, 4.0 * s) {
                p.add(seg);
            }
        } else {
            p.line_segment([a, b], stroke);
        }
    }
    // The center diamond (the kit's rotate(45) card): a lighter back
    // face and the violet front face hint the #8B87F0→#6863E7 gradient.
    let dia_t = ease_out((t / (SPLASH_SECS * 0.45)).clamp(0.0, 1.0));
    if dia_t > 0.0 {
        let rot = |x: f32, y: f32, scale: f32| {
            let (c45, s45) = (std::f32::consts::FRAC_1_SQRT_2, std::f32::consts::FRAC_1_SQRT_2);
            c + Vec2::new((x * c45 - y * s45) * s * scale, (x * s45 + y * c45) * s * scale)
        };
        p.add(Shape::convex_polygon(
            vec![
                rot(-40.0, -40.0, dia_t),
                rot(40.0, -40.0, dia_t),
                rot(40.0, 40.0, dia_t),
                rot(-40.0, 40.0, dia_t),
            ],
            VIOLET_LT,
            Stroke::NONE,
        ));
        p.add(Shape::convex_polygon(
            vec![
                rot(-32.0, -32.0, dia_t),
                rot(32.0, -32.0, dia_t),
                rot(32.0, 32.0, dia_t),
                rot(-32.0, 32.0, dia_t),
            ],
            VIOLET,
            Stroke::NONE,
        ));
        // The sky hub dot + the faint inner stroke ring.
        p.circle_filled(c, 4.6 * s * dia_t, SKY);
        p.add(Shape::convex_polygon(
            vec![
                rot(-20.0, -20.0, dia_t),
                rot(20.0, -20.0, dia_t),
                rot(20.0, 20.0, dia_t),
                rot(-20.0, 20.0, dia_t),
            ],
            Color32::TRANSPARENT,
            Stroke::new(1.6 * s, SKY_LT.gamma_multiply(0.45 * dia_t)),
        ));
    }
    // The corner cards: white rounded squares with a soft shadow, the
    // glyphs, popping in sequence.
    for (i, card) in CARDS.iter().enumerate() {
        let start = SPLASH_SECS * (0.22 + 0.10 * i as f32);
        let phase = ease_out(((t - start) / (SPLASH_SECS * 0.18)).clamp(0.0, 1.0));
        if phase <= 0.0 {
            continue;
        }
        let center = c + card.at * s * (0.8 + 0.2 * phase);
        let r = 19.0 * s;
        // Soft shadow (the kit's drop-shadow filter, flattened).
        p.rect_filled(
            Rect::from_center_size(center + Vec2::new(0.0, 2.2 * s), Vec2::splat(2.0 * r)),
            egui::CornerRadius::same((8.0 * s) as u8),
            INK.gamma_multiply(0.10 * phase),
        );
        p.rect_filled(
            Rect::from_center_size(center, Vec2::splat(2.0 * r)),
            egui::CornerRadius::same((8.0 * s) as u8),
            CARD.gamma_multiply(phase),
        );
        p.rect_stroke(
            Rect::from_center_size(center, Vec2::splat(2.0 * r)),
            egui::CornerRadius::same((8.0 * s) as u8),
            Stroke::new(1.4 * s, CARD_STROKE.gamma_multiply(phase)),
            egui::StrokeKind::Inside,
        );
        draw_glyph(p, center, card.glyph, s, phase);
    }
    // The sky dot above and the dashed smile under the cards.
    let dot_t = ease_out(((t - SPLASH_SECS * 0.55) / (SPLASH_SECS * 0.15)).clamp(0.0, 1.0));
    if dot_t > 0.0 {
        p.circle_filled(c + Vec2::new(0.0, -112.0 * s), 4.2 * s * dot_t, SKY);
    }
    let smile_t = ease_out(((t - SPLASH_SECS * 0.60) / (SPLASH_SECS * 0.25)).clamp(0.0, 1.0));
    if smile_t > 0.0 {
        // M 81,232 Q 150,270 219,232 — the quadratic under the cards.
        let (a, q, b) = (
            c + Vec2::new(-69.0, 82.0) * s,
            c + Vec2::new(0.0, 120.0) * s,
            c + Vec2::new(69.0, 82.0) * s,
        );
        let n = 24;
        let pts: Vec<Pos2> = (0..=n)
            .map(|i| {
                let u = smile_t * (i as f32 / n as f32);
                let mu = 1.0 - u;
                Pos2::new(
                    mu * mu * a.x + 2.0 * mu * u * q.x + u * u * b.x,
                    mu * mu * a.y + 2.0 * mu * u * q.y + u * u * b.y,
                )
            })
            .collect();
        for seg in egui::Shape::dashed_line(
            &pts,
            Stroke::new(2.0 * s, SKY_DEEP.gamma_multiply(0.9)),
            5.0 * s,
            4.0 * s,
        ) {
            p.add(seg);
        }
    }
}

/// A card's glyph, scaled by `s` (the card is 38 units wide).
fn draw_glyph(p: &egui::Painter, center: Pos2, glyph: Glyph, s: f32, phase: f32) {
    let ink = SKY_DEEP;
    match glyph {
        Glyph::Dot => {
            // The violet accent card: a white dot (r 7).
            p.circle_filled(center, 7.0 * s * phase, CARD);
            p.circle_stroke(
                center,
                7.0 * s * phase,
                Stroke::new(1.2 * s, SKY_LT.gamma_multiply(0.6 * phase)),
            );
        }
        Glyph::Square => {
            p.rect_filled(
                Rect::from_center_size(center, Vec2::splat(16.0 * s * phase)),
                egui::CornerRadius::same(3),
                SKY.gamma_multiply(phase),
            );
            p.circle_filled(
                center + Vec2::new(-12.0, -12.0) * s,
                2.4 * s * phase,
                SKY_LT.gamma_multiply(0.8 * phase),
            );
        }
        Glyph::Check => {
            // M -10,1 L -3,8 L 10,-7 — the done check.
            let pts = vec![
                center + Vec2::new(-10.0, 1.0) * s * phase,
                center + Vec2::new(-3.0, 8.0) * s * phase,
                center + Vec2::new(10.0, -7.0) * s * phase,
            ];
            p.add(Shape::line(pts, Stroke::new(3.0 * s, ink)));
        }
        Glyph::Dash => {
            let a = center + Vec2::new(-12.0, 0.0) * s;
            let b = center + Vec2::new(12.0, 0.0) * s;
            for seg in egui::Shape::dashed_line(
                &[a, b],
                Stroke::new(2.4 * s, ink.gamma_multiply(phase)),
                4.0 * s,
                4.0 * s,
            ) {
                p.add(seg);
            }
        }
    }
}

/// The splash background: the kit's vertical gradient plus the quiet
/// dot grid (#AAB4C6 @ 0.08).
fn draw_splash_bg(p: &egui::Painter, screen: Rect) {
    let bands = 24;
    let h = screen.height() / bands as f32;
    for i in 0..bands {
        let t = i as f32 / (bands - 1) as f32;
        p.rect_filled(
            Rect::from_min_size(
                Pos2::new(screen.left(), screen.top() + h * i as f32),
                Vec2::new(screen.width(), h + 1.0),
            ),
            0,
            lerp_color(BG_TOP, BG_BOT, t),
        );
    }
    // The dot grid: ~72-unit spacing in the 1080 space → the window.
    let spacing = (screen.width() / 15.0).max(40.0);
    let mut y = screen.top() + spacing * 0.5;
    while y < screen.bottom() {
        let mut x = screen.left() + spacing * 0.5;
        while x < screen.right() {
            p.circle_filled(Pos2::new(x, y), 1.6, DOT.gamma_multiply(0.08));
            x += spacing;
        }
        y += spacing;
    }
}

/// Draw the full-screen splash overlay. Returns `true` while it wants
/// to stay up (before [`SPLASH_SECS`]); the caller hands off to the
/// shell afterwards. Repaints are time-driven (the animation advances
/// without input).
pub fn draw_splash(ui: &mut egui::Ui, screen: Rect, started: Instant) -> bool {
    let t = started.elapsed().as_secs_f32();
    if t >= SPLASH_SECS {
        return false;
    }
    egui::Area::new(egui::Id::unique("dc-splash"))
        .order(egui::Order::Foreground)
        .fixed_pos(Pos2::ZERO)
        .show(ui.ctx(), |ui| {
            // The scrim also eats input: the boot moment is not
            // interactive.
            ui.allocate_rect(screen, Sense::click_and_drag());
            let p = ui.painter().clone();
            draw_splash_bg(&p, screen);
            // The mark sits above the vertical center (the kit's
            // 470/1920 placement, adapted to any window size).
            let c = screen.center() - Vec2::new(0.0, screen.height() * 0.12);
            let half = (screen.height() * 0.15).clamp(60.0, 160.0);
            draw_mark(&p, c, half, t);
            // The wordmark rises + fades in (the kit's 0.5→0.6 phase).
            let wm_t = ease_out(((t - SPLASH_SECS * 0.52) / (SPLASH_SECS * 0.22)).clamp(0.0, 1.0));
            if wm_t > 0.0 {
                let rise = (1.0 - wm_t) * 28.0;
                let size = (half * 0.62).clamp(24.0, 64.0);
                p.text(
                    c + Vec2::new(0.0, half + 46.0 + rise),
                    egui::Align2::CENTER_TOP,
                    "tarahi",
                    egui::FontId::proportional(size),
                    INK.gamma_multiply(wm_t),
                );
                // The caption (the kit's DIAGRAM · DESIGN line).
                let cap_t =
                    ease_out(((t - SPLASH_SECS * 0.66) / (SPLASH_SECS * 0.2)).clamp(0.0, 1.0));
                if cap_t > 0.0 {
                    p.text(
                        c + Vec2::new(0.0, half + 46.0 + size + 18.0),
                        egui::Align2::CENTER_TOP,
                        "DIAGRAM · DESIGN",
                        egui::FontId::proportional(13.0),
                        MUTED.gamma_multiply(cap_t),
                    );
                }
            }
        });
    // Time-driven animation: keep the frames coming while showing.
    ui.ctx().request_repaint_after(Duration::from_millis(16));
    true
}

/// Draw the in-flight file-load scrim: the kit's white document card
/// (bob + twinkle nodes + indeterminate progress) with an `Opening
/// <file>…` line, above an input-eating full-screen rect (modal for
/// v1). Time-driven repaints while up.
pub fn draw_loading(ui: &mut egui::Ui, screen: Rect, path: &std::path::Path) {
    egui::Area::new(egui::Id::unique("dc-loading"))
        .order(egui::Order::Foreground)
        .fixed_pos(Pos2::ZERO)
        .show(ui.ctx(), |ui| {
            ui.allocate_rect(screen, Sense::click_and_drag());
            let p = ui.painter().clone();
            p.rect_filled(screen, 0.0, BG_TOP.gamma_multiply(0.72));
            let t = Instant::now().elapsed().as_secs_f32();
            // The card bobs (the kit's translate 0 → −4 → 0 over 3 s).
            let bob = (t % 3.0 / 3.0 * std::f32::consts::TAU).sin() * -4.0;
            let c = screen.center() + Vec2::new(0.0, bob - 10.0);
            draw_document(&p, c, t);
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.display().to_string());
            p.text(
                c + Vec2::new(0.0, 170.0),
                egui::Align2::CENTER_TOP,
                format!("Opening {name}…"),
                egui::FontId::proportional(14.0),
                MUTED,
            );
        });
    ui.ctx().request_repaint_after(Duration::from_millis(16));
}

/// The kit's document card: a white sheet with the folded corner, the
/// mini network twinkling on it, and the indeterminate progress bar.
fn draw_document(p: &egui::Painter, c: Pos2, t: f32) {
    let s = 0.9; // the 360-space → drawn size
    let rect = Rect::from_min_max(
        c + Vec2::new(-84.0 * s, -116.0 * s),
        c + Vec2::new(84.0 * s, 100.0 * s),
    );
    // The drop shadow (the kit's blur filter, flattened).
    p.rect_filled(
        rect.translate(Vec2::new(0.0, 4.0)),
        6.0,
        INK.gamma_multiply(0.10),
    );
    // The sheet + the folded corner (M230,64 L264,98 L230,98).
    p.rect_filled(rect, 6.0, CARD);
    p.rect_stroke(rect, 6.0, Stroke::new(2.4, CARD_STROKE), egui::StrokeKind::Inside);
    let fold = 34.0 * s;
    let corner = Pos2::new(rect.right(), rect.top());
    p.add(Shape::convex_polygon(
        vec![
            corner,
            corner + Vec2::new(-fold, 0.0),
            corner + Vec2::new(0.0, fold),
        ],
        FOLD,
        Stroke::new(2.4, CARD_STROKE),
    ));
    // The faded text lines.
    for (w, dy) in [(100.0, 4.0), (144.0, 16.0), (76.0, 28.0)] {
        p.rect_filled(
            Rect::from_min_size(
                c + Vec2::new(-60.0 * s, 74.0 * s + dy * s),
                Vec2::new(w * s, 4.0 * s),
            ),
            2.0,
            CARD_STROKE.gamma_multiply(0.4),
        );
    }
    // The mini network's spokes, then the center diamond + four
    // twinkling nodes (the kit's 1.6 s opacity cycle, staggered 0.4 s).
    for d in [-28.0f32, 28.0] {
        p.line_segment(
            [c + Vec2::new(0.0, d * s), c + Vec2::new(d.signum() * 28.0 * s, 0.0)],
            Stroke::new(2.0, VIOLET.gamma_multiply(0.5)),
        );
        p.line_segment(
            [c + Vec2::new(d.signum() * 28.0 * s, 0.0), c + Vec2::new(0.0, d * s)],
            Stroke::new(2.0, VIOLET.gamma_multiply(0.5)),
        );
    }
    let nodes: [(Vec2, i64, NodeKind); 5] = [
        (Vec2::new(0.0, -34.0 * s), 1, NodeKind::Circle),
        (Vec2::new(34.0 * s, 0.0), 2, NodeKind::Square),
        (Vec2::new(0.0, 34.0 * s), 3, NodeKind::Triangle),
        (Vec2::new(-34.0 * s, 0.0), 4, NodeKind::Diamond),
        (Vec2::ZERO, 0, NodeKind::Center),
    ];
    for (at, begin, kind) in nodes {
        let tw = ((t - 0.2 * begin as f32) % 1.6 / 1.6 * std::f32::consts::TAU).sin();
        let alpha = (1.0 - 0.6 * (0.5 - 0.5 * tw)).max(0.2); // 1 → 0.4 → 1
        let col = match kind {
            NodeKind::Circle | NodeKind::Diamond => SKY,
            NodeKind::Square => VIOLET,
            NodeKind::Triangle => VIOLET_LT,
            NodeKind::Center => VIOLET,
        }
        .gamma_multiply(alpha);
        let center = c + at;
        match kind {
            NodeKind::Center => {
                // The rotated accent card (diamond) + the sky hub.
                let r = 11.0 * s * std::f32::consts::SQRT_2;
                p.add(Shape::convex_polygon(
                    vec![
                        center + Vec2::new(0.0, -r),
                        center + Vec2::new(r, 0.0),
                        center + Vec2::new(0.0, r),
                        center + Vec2::new(-r, 0.0),
                    ],
                    col,
                    Stroke::NONE,
                ));
                p.circle_filled(center, 2.6 * s, SKY.gamma_multiply(alpha));
            }
            NodeKind::Circle => {
                p.circle_filled(center, 5.5 * s, col);
            }
            NodeKind::Square => {
                p.rect_filled(
                    Rect::from_center_size(center, Vec2::splat(11.0 * s)),
                    egui::CornerRadius::same(2),
                    col,
                );
            }
            NodeKind::Triangle => {
                p.add(Shape::convex_polygon(
                    vec![
                        center + Vec2::new(0.0, 7.0 * s),
                        center + Vec2::new(7.0 * s, -4.0 * s),
                        center + Vec2::new(-7.0 * s, -4.0 * s),
                    ],
                    col,
                    Stroke::NONE,
                ));
            }
            NodeKind::Diamond => {
                p.add(Shape::convex_polygon(
                    vec![
                        center + Vec2::new(0.0, -6.0 * s),
                        center + Vec2::new(6.0 * s, 0.0),
                        center + Vec2::new(0.0, 6.0 * s),
                        center + Vec2::new(-6.0 * s, 0.0),
                    ],
                    col,
                    Stroke::NONE,
                ));
            }
        }
    }
    // The indeterminate progress bar: the kit's sweep (empty → fill →
    // hold → reset, 2.2 s) with the bright thumb.
    let track_t = (t % 2.2) / 2.2;
    let track = Rect::from_min_size(
        c + Vec2::new(-68.0 * s, 108.0 * s),
        Vec2::new(136.0 * s, 11.0 * s),
    );
    p.rect_filled(track, 5.5, SKY_LT.gamma_multiply(0.18));
    let frac = ((track_t - 0.10) / (0.82 - 0.10)).clamp(0.0, 1.0);
    if frac > 0.0 {
        p.rect_filled(
            Rect::from_min_size(track.min, Vec2::new(track.width() * frac, track.height())),
            5.5,
            VIOLET,
        );
    }
    let thumb_x = track.min.x + track.width() * frac;
    p.rect_filled(
        Rect::from_min_size(
            Pos2::new(thumb_x, track.min.y - 1.0),
            Vec2::new(16.0 * s, track.height() + 2.0),
        ),
        4.0,
        CARD.gamma_multiply(0.85),
    );
}

#[derive(Clone, Copy, PartialEq)]
enum NodeKind {
    Circle,
    Square,
    Triangle,
    Diamond,
    Center,
}
