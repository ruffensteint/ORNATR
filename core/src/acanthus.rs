//! Baroque acanthus leaf, built from its parts rather than as one outline:
//! a midrib, lobe groups along each side (each group a fan of rounded
//! fingers whose tips curl back toward the base), eyes where neighbouring
//! groups touch, and pipes (channels) running from the midrib out into each
//! group. Scroll vines clad their curls with it (`clad_scroll`).
use crate::bud::open;
use crate::geometry::{line_frame, line_length, pt, Point};

#[derive(Clone, Copy, Debug)]
pub struct LeafSpec {
    /// Lobe groups on each side, not counting the tip.
    pub groups: usize,
    /// Fingers in each group.
    pub fingers: usize,
    /// Widest half-width, as a fraction of the leaf's length.
    pub width: f64,
    /// How far the fingertips hook back toward the base (0 = straight).
    pub hook: f64,
    /// Eye size, as a fraction of the local half-width (0 = no eyes).
    pub eye: f64,
    /// Depth of the notches between fingers, as a fraction of the half-width.
    pub notch: f64,
    /// Draw pipes along the midrib and into each group.
    pub pipes: bool,
    /// Depth of the cuts between lobe groups, as a fraction of the local
    /// half-width (0 = as deep as the notches imply).
    pub cut: f64,
    /// Length of the narrow stalk before the leaf opens, as a fraction of its length.
    pub stalk: f64,
}
impl Default for LeafSpec {
    fn default() -> Self { LeafSpec { groups: 2, fingers: 3, width: 0.2, hook: 0.6, eye: 0.16, notch: 0.26, pipes: true, cut: 0.0, stalk: 0.1 } }
}

pub struct Leaf { pub polygon: Vec<Point>, pub cuts: Vec<Vec<Point>>, pub folds: Vec<Vec<Point>> }

fn smooth(x: f64) -> f64 { let c = x.clamp(0.0, 1.0); c * c * (3.0 - 2.0 * c) }

/// Half-width envelope along the leaf: a narrow stalk, a full belly just
/// before the middle, then a long taper to a rounded tip.
fn envelope(u: f64, w: f64) -> f64 { envelope_with(u, w, 0.1) }
fn envelope_with(u: f64, w: f64, stalk: f64) -> f64 {
    if u < stalk { return w * 0.12 * (0.6 + 0.4 * u / stalk); }
    let t = (u - stalk) / (1.0 - stalk);
    w * ((std::f64::consts::PI * t.powf(0.72)).sin().max(0.0).powf(0.8) * 0.9 + 0.1 * (1.0 - t))
}

/// One side of the leaf in local coordinates (u along, v across, v ≥ 0), from
/// the stalk to the tip, plus that side's eyes (with the slit that runs from
/// the silhouette into each) and pipes.
///
/// Each lobe group is one broad lobe: from the cut before it, its back rises
/// in a long easy curve, and only its far end breaks into a small cluster of
/// rounded fingers, the last one longest and hooked toward the leaf's tip.
/// Groups are separated by a cut that runs in from the silhouette as a slit
/// and ends in a round eye.
fn side(s: &LeafSpec) -> (Vec<(f64, f64)>, Vec<Vec<(f64, f64)>>, Vec<Vec<(f64, f64)>>) { side_env(s, &|u, w| envelope_with(u, w, s.stalk), 0.14, None) }
/// `side` with its envelope and the first lobe group's start given (the fat-root
/// cladding uses its own envelope and starts the lobes at the root), and
/// optionally the group boundaries themselves (from the first group's start
/// to the last group's end; `s.groups` is then ignored).
fn side_env(s: &LeafSpec, envelope: &dyn Fn(f64, f64) -> f64, u0: f64, given: Option<&[f64]>) -> (Vec<(f64, f64)>, Vec<Vec<(f64, f64)>>, Vec<Vec<(f64, f64)>>) {
    let w = s.width;
    let bounds: Vec<f64> = match given {
        Some(b) if b.len() >= 2 => b.to_vec(),
        // the lobe groups run from u0 (0.14 normally) to u1
        _ => { let u1 = 0.8; (0..=s.groups).map(|g| u0 + (u1 - u0) * (g as f64 / s.groups as f64).powf(0.85)).collect() }
    };
    let s = &LeafSpec { groups: bounds.len() - 1, ..*s };
    let (u0, u1) = (bounds[0], bounds[bounds.len() - 1]);
    // the silhouette dips only to the notch; with `cut` set, a slit runs on
    // from there deep into the leaf and ends in the eye
    let slit = s.cut > 0.0;
    let cut_depth = if slit { 0.93 } else { 1.0 - s.notch * 1.6 };
    let (rise_w, fall_w) = if slit { (0.07, 0.04) } else { (0.22, 0.1) };
    let nf = s.fingers.max(1) as f64;
    // the silhouette as a factor of the envelope at u
    let factor = |u: f64| -> f64 {
        if u < u0 { return 1.0; }
        let (g, a, b) = if u >= u1 { (s.groups, u1, 1.0) } else { let g = bounds.windows(2).position(|w| u >= w[0] && u < w[1]).unwrap_or(0); (g, bounds[g], bounds[g + 1]) };
        let x = (u - a) / (b - a);
        // a narrow cut at each boundary between groups (and before the tip)
        let rise = if g > 0 { cut_depth + (1.0 - cut_depth) * smooth(x / rise_w) } else { 1.0 };
        let fall = if g < s.groups { cut_depth + (1.0 - cut_depth) * smooth((1.0 - x) / fall_w) } else { 1.0 };
        // fingers on the far part of each lobe: rounded tips, narrow notches
        let (f0, f1) = if g < s.groups { (0.3, 0.96) } else { (0.4, 0.85) };
        let fingers = if x > f0 && x < f1 {
            let y = (x - f0) / (f1 - f0) * nf; let frac = y - y.floor();
            // the fingertips lean toward the leaf's tip (hook)
            let lean = (frac + s.hook * 0.18 * (std::f64::consts::PI * frac).sin()).clamp(0.0, 1.0);
            1.0 - s.notch * 1.15 * (1.0 - (std::f64::consts::PI * lean).sin().abs().powf(0.5))
        } else { 1.0 };
        rise.min(fall) * fingers
    };
    let n = 700;
    let pts: Vec<(f64, f64)> = (0..=n).map(|i| { let u = i as f64 / n as f64 * 0.992; (u, envelope(u, w) * factor(u)) }).collect();
    let (mut cuts, mut pipes) = (vec![], vec![]);
    for g in 0..s.groups {
        let (a, b) = (bounds[g], bounds[g + 1]); let span = b - a;
        // the slit and eye that end the cut after this group
        if s.eye > 0.0 {
            let eb = envelope(b, w); let r = eb * s.eye * 0.5;
            let c = if slit { (b + span * 0.03, eb * (1.0 - s.cut)) } else { (b + span * 0.012, eb * cut_depth - r * 2.0) };
            cuts.push(vec![(b, eb * cut_depth), (c.0, c.1 + r)]);
            cuts.push((0..=20).map(|k| { let t = k as f64 / 20.0 * std::f64::consts::TAU; (c.0 + r * 0.8 * t.sin(), c.1 + r * t.cos()) }).collect());
        }
        if s.pipes {
            let mid = a + span * 0.72;
            pipes.push(vec![(a + span * 0.05, envelope(a, w) * 0.12), (a + span * 0.38, envelope(a + span * 0.38, w) * 0.42), (mid, envelope(mid, w) * 0.74)]);
        }
    }
    (pts, cuts, pipes)
}

/// Grow the leaf along `spine`. `flip` mirrors it across the midrib.
pub fn baroque_leaf(spine: &[Point], s: &LeafSpec, flip: bool) -> Leaf {
    let len = line_length(spine);
    let m = if flip { -1.0 } else { 1.0 };
    let place = |u: f64, v: f64| { let (p, a) = line_frame(spine, u.clamp(0.0, 1.0)); pt(p.x - a.sin() * v * len * m, p.y + a.cos() * v * len * m) };
    let (one, eyes, pipes) = side(s);
    // the other side is the same construction with its groups a little offset
    // and scaled, so the leaf is not mirror-perfect
    let other_spec = LeafSpec { width: s.width * 0.92, ..*s };
    let (two, eyes2, pipes2) = side(&other_spec);
    let shift = |p: &(f64, f64), k: f64| (p.0 * (1.0 - k) + k * p.0 * p.0, p.1);
    let mut ctrl: Vec<(f64, f64)> = one.clone();
    ctrl.push((1.0, 0.0));
    ctrl.extend(two.iter().rev().map(|p| { let q = shift(p, 0.04); (q.0, -q.1) }));
    let polygon: Vec<Point> = ctrl.iter().map(|q| place(q.0, q.1)).collect();
    let loops = |e: &Vec<(f64, f64)>, sign: f64, k: f64| -> Vec<Point> { e.iter().map(|p| { let q = shift(p, k); place(q.0, q.1 * sign) }).collect() };
    let mut cuts: Vec<Vec<Point>> = eyes.iter().map(|e| loops(e, 1.0, 0.0)).collect();
    cuts.extend(eyes2.iter().map(|e| loops(e, -1.0, 0.04)));
    let line = |p: &Vec<(f64, f64)>, sign: f64, k: f64| -> Vec<Point> { let c: Vec<(f64, f64)> = p.iter().map(|q| { let r = shift(q, k); (r.0, r.1 * sign) }).collect(); open(&c).iter().map(|q| place(q.x, q.y)).collect() };
    let mut folds: Vec<Vec<Point>> = vec![];
    if s.pipes {
        // the midrib channel
        for sign in [1.0, -1.0] { folds.push((0..=40).map(|i| { let u = 0.06 + 0.8 * i as f64 / 40.0; place(u, sign * envelope(u, s.width) * 0.07) }).collect()); }
        folds.extend(pipes.iter().map(|p| line(p, 1.0, 0.0)));
        folds.extend(pipes2.iter().map(|p| line(p, -1.0, 0.04)));
    }
    Leaf { polygon, cuts, folds }
}

/// A gently curved study spine from `start`, heading `angle` (radians).
pub fn study_spine(start: Point, angle: f64, length: f64, bend: f64) -> Vec<Point> {
    let n = 120; let step = length / n as f64; let mut pts = vec![start];
    for k in 0..n { let t = (k as f64 + 0.5) / n as f64; let h = angle + bend * smooth(t) * t; let p = pts[k]; pts.push(pt(p.x + h.cos() * step, p.y + h.sin() * step)); }
    pts
}

/// Clad a scroll with the leaf on one side only: the outer (convex) side of
/// the curl carries the leaf's silhouette (lobe groups, cuts and eyes, pipes),
/// the inner side follows the scroll's own tapering stem, so the leaf sheathes
/// the scroll without crowding its inner turn. `outer` is +1 or -1 (the side,
/// as across the spine's heading); `half_width` is the leaf's widest reach
/// from the spine and `stem` the stem's half-width at the root, both in mm.
pub fn clad_scroll(spine: &[Point], s: &LeafSpec, outer: f64, half_width: f64, stem: f64) -> Leaf {
    let len = line_length(spine).max(1.0);
    let spec = LeafSpec { width: half_width / len, ..*s };
    let place = |u: f64, v: f64| { let (p, a) = line_frame(spine, u.clamp(0.0, 1.0)); pt(p.x - a.sin() * v * outer, p.y + a.cos() * v * outer) };
    let (one, eyes, pipes) = side(&spec);
    // outer: the leaf, in mm across the spine
    let mut polygon: Vec<Point> = one.iter().map(|(u, v)| place(*u, v * len)).collect();
    polygon.push(place(1.0, 0.0));
    // inner: the stem edge, tapering to the tip
    let n = 200;
    // narrow at the root and widening over the stalk, so the scroll grows out
    // of its parent instead of poking through its far side
    polygon.extend((0..=n).rev().map(|i| { let u = i as f64 / n as f64; place(u, -stem * (1.0 - 0.85 * u.powf(0.7)) * (0.25 + 0.75 * smooth(u / 0.12))) }));
    let cuts: Vec<Vec<Point>> = eyes.iter().map(|e| e.iter().map(|(u, v)| place(*u, v * len)).collect()).collect();
    let mut folds: Vec<Vec<Point>> = vec![];
    if s.pipes {
        // a channel along the scroll just inside the leaf, and the pipes into each lobe
        folds.push((0..=60).map(|i| { let u = spec.stalk.max(0.08) + (0.86 - spec.stalk.max(0.08)) * i as f64 / 60.0; place(u, envelope_with(u, spec.width, spec.stalk) * len * 0.12) }).collect());
        folds.extend(pipes.iter().map(|p| { let c: Vec<(f64, f64)> = p.iter().map(|q| (q.0, q.1)).collect(); open(&c).iter().map(|q| place(q.x, q.y * len)).collect::<Vec<Point>>() }));
    }
    Leaf { polygon, cuts, folds }
}

/// The fat-root leaf envelope (crest study, 2026-10-07): full width at the
/// root, easing down along the sweep and narrowing into the curl, instead of
/// the usual lens (narrow stalk, broad belly, narrow tip).
fn envelope_root(u: f64, w: f64) -> f64 {
    w * (0.22 + 0.78 * (1.0 - u).max(0.0).powf(1.3)) * (1.0 - u.max(0.0).powi(6))
}

/// `clad_scroll` with a fat root (study stage, not used by the app): the leaf
/// is widest where the scroll leaves its root and tapers into the volute, and
/// the stem keeps its full width at the root instead of narrowing to grow
/// out of a parent. For scrolls that spring from a centre motif, as on crests.
pub fn clad_scroll_fat(spine: &[Point], s: &LeafSpec, outer: f64, half_width: f64, stem: f64) -> Leaf {
    clad_scroll_fat_with(spine, s, outer, half_width, stem, &FatStyle::default())
}

/// Variations on the fat-root leaf (crest study round 7, 2026-10-07; one per
/// option so each can be judged alone). The default is the plain fat leaf.
#[derive(Clone, Debug, Default)]
pub struct FatStyle {
    /// Creases: ribs running from the root along the sweep, fanning into the
    /// first lobe, so the root mass reads as a leaf turning out from under the
    /// centre rather than a pad. Each is (share of the local half-width at the
    /// root, share where it ends, where it ends along the leaf).
    pub creases: Vec<(f64, f64, f64)>,
    /// The lobe groups' boundaries along the leaf (first group's start to the
    /// last one's end), for unequal groups; None = the usual even spacing.
    pub groups: Option<Vec<f64>>,
    /// A waist: (where along the leaf, its half-length, how much narrower).
    pub waist: Option<(f64, f64, f64)>,
    /// Leave out the slit and eye after the first lobe group: next to the
    /// root it sits half under the centre motif, so only a floating eye shows.
    pub no_root_cut: bool,
}

/// `clad_scroll_fat` with a `FatStyle`.
pub fn clad_scroll_fat_with(spine: &[Point], s: &LeafSpec, outer: f64, half_width: f64, stem: f64, style: &FatStyle) -> Leaf {
    let len = line_length(spine).max(1.0);
    let spec = LeafSpec { width: half_width / len, stalk: 0.0, ..*s };
    let place = |u: f64, v: f64| { let (p, a) = line_frame(spine, u.clamp(0.0, 1.0)); pt(p.x - a.sin() * v * outer, p.y + a.cos() * v * outer) };
    let envelope_root = |u: f64, w: f64| -> f64 {
        let base = envelope_root(u, w);
        match style.waist { Some((c, h, d)) => base * (1.0 - d * (-((u - c) / h).powi(2)).exp()), None => base }
    };
    let (one, eyes, pipes) = side_env(&spec, &envelope_root, 0.03, style.groups.as_deref());
    let mut polygon: Vec<Point> = one.iter().map(|(u, v)| place(*u, v * len)).collect();
    polygon.push(place(1.0, 0.0));
    let n = 200;
    polygon.extend((0..=n).rev().map(|i| { let u = i as f64 / n as f64; place(u, -stem * (1.0 - 0.85 * u.powf(0.7))) }));
    // a rounded root end, from the stem edge round to the leaf's edge
    let (p0, a0) = line_frame(spine, 0.0);
    let (r_in, r_out) = (stem, envelope_root(0.0, spec.width) * len);
    let mid = (r_out - r_in) * 0.5; let rad = (r_out + r_in) * 0.5;
    for k in 1..12 {
        let t = std::f64::consts::PI * k as f64 / 12.0;
        let (along, across) = (-t.sin() * rad * 0.8, mid - t.cos() * rad);
        polygon.push(pt(p0.x + a0.cos() * along - a0.sin() * across * outer, p0.y + a0.sin() * along + a0.cos() * across * outer));
    }
    // each group's cut is a slit then its eye
    let cuts: Vec<Vec<Point>> = eyes.iter().skip(if style.no_root_cut { 2 } else { 0 }).map(|e| e.iter().map(|(u, v)| place(*u, v * len)).collect()).collect();
    let mut folds: Vec<Vec<Point>> = vec![];
    if s.pipes {
        folds.push((0..=60).map(|i| { let u = 0.04 + 0.82 * i as f64 / 60.0; place(u, envelope_root(u, spec.width) * len * 0.12) }).collect());
        folds.extend(pipes.iter().map(|p| { let c: Vec<(f64, f64)> = p.iter().map(|q| (q.0, q.1)).collect(); open(&c).iter().map(|q| place(q.x, q.y * len)).collect::<Vec<Point>>() }));
    }
    // creases: from the root along the sweep, easing from one share of the
    // local width to another, so each follows the leaf's own taper
    for &(a, b, end) in &style.creases {
        folds.push((0..=50).map(|i| { let t = i as f64 / 50.0; let u = 0.02 + (end - 0.02) * t; place(u, envelope_root(u, spec.width) * len * (a + (b - a) * smooth(t))) }).collect());
    }
    Leaf { polygon, cuts, folds }
}
