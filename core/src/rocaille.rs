//! Rocaille (Rococo) ornament: the vocabulary. The app's Rococo workspace
//! builds its designs from these parts through `rococo.rs`; the study
//! sheets (`rocaille_study`) use them directly.
//!
//! Acanthus grows along a stem; rocaille is built round a shell. The parts:
//! - `c_scroll`: a moulded band whose spine curls into a volute at each end
//!   (C when both ends curl the same way, S when they oppose), with a channel
//!   line along it.
//! - `shell`: a fan of curved flutes from a hinge, an asymmetric reach and a
//!   scalloped lip.
//! - `frill`: a crimped strip along a curve, its waves leaning one way.
//! - `flames`: a bundle of S-curved tongues from one base.
//! - `pocket`: a sunken hollow with a lip line.
//!
//! Parts are drawn back to front: each part's lines are hidden under the
//! parts in front of it.
use crate::booleans::{offset, tidy, union, Shapes};
use crate::geometry::{lerp as lerp_pt, pt, Point};
use crate::outline::{visible_lines, Runs};

/// A drawn part: its outline (one polygon), the lines inside it (flutes,
/// channels, crimps), and whether it is a sunken pocket.
#[derive(Clone, Debug)]
/// `seam`: a region where this part's outline is not drawn (where it
/// continues a surface it grows out of, as a turned-over leaf end does).
pub struct Part { pub outline: Vec<Point>, pub lines: Vec<Vec<Point>>, pub pocket: bool, pub seam: Vec<Point> }

fn smooth(t: f64) -> f64 { let t = t.clamp(0.0, 1.0); t * t * (3.0 - 2.0 * t) }
fn normal(h: f64) -> Point { pt(-h.sin(), h.cos()) }
fn add(p: Point, n: Point, d: f64) -> Point { pt(p.x + n.x * d, p.y + n.y * d) }

/// Spine samples: point and heading, integrated from a curvature function of
/// arc length (step 0.4 mm).
pub fn integrate(start: Point, heading: f64, length: f64, kappa: impl Fn(f64) -> f64) -> Vec<(Point, f64)> {
    let ds = 0.4; let n = (length / ds).ceil().max(2.0) as usize; let ds = length / n as f64;
    let (mut p, mut h) = (start, heading);
    let mut out = vec![(p, h)];
    for i in 0..n {
        let s = (i as f64 + 0.5) * ds;
        let hm = h + kappa(s) * ds / 2.0;
        p = pt(p.x + hm.cos() * ds, p.y + hm.sin() * ds);
        h += kappa(s) * ds;
        out.push((p, h));
    }
    out
}

/// One volute end: radius of curvature grows linearly from `eye` at the tip
/// to `outer` over `turn` radians (a logarithmic spiral). Returns (length,
/// curvature at distance d from the tip).
#[derive(Clone, Copy, Debug)]
pub struct Curl { pub eye: f64, pub outer: f64, pub turn: f64, pub side: f64 }
impl Curl {
    fn rate(&self) -> f64 { (self.outer / self.eye).ln() / self.turn }
    pub fn length(&self) -> f64 { if self.turn <= 0.0 { 0.0 } else { (self.outer - self.eye) / self.rate() } }
    fn radius(&self, d: f64) -> f64 { self.eye + self.rate() * d }
}

/// A scroll spine: a curl at the start, a middle of radius `mid` (signed:
/// positive turns left in y-down screen terms... i.e. heading increases),
/// and a curl at the end. Curvature blends smoothly between the pieces.
pub fn scroll_spine(start: Point, heading: f64, a: Curl, middle: f64, mid_curv: f64, b: Curl) -> Vec<(Point, f64)> {
    let (la, lb) = (a.length(), b.length());
    let total = la + middle + lb;
    let blend = 6.0;
    integrate(start, heading, total, |s| {
        let ka = if a.turn > 0.0 && s < la { a.side / a.radius(s) } else { 0.0 };
        let kb = if b.turn > 0.0 && s > total - lb { b.side / b.radius(total - s) } else { 0.0 };
        // weights: 1 inside a curl, fading into the middle over `blend` mm
        let wa = if a.turn > 0.0 { 1.0 - smooth((s - la) / blend) } else { 0.0 };
        let wb = if b.turn > 0.0 { smooth((s - (total - lb - blend)) / blend) } else { 0.0 };
        let ka = if s < la { ka } else { a.side / a.outer };
        let kb = if s > total - lb { kb } else { b.side / b.outer };
        let wm = (1.0 - wa - wb).max(0.0);
        ka * wa + kb * wb + mid_curv * wm
    })
}

/// Outline of a band of half-width `w(u)` (u = 0..1 along the spine), with
/// round caps; the band is tidied so tight curls don't tangle.
pub fn band(spine: &[(Point, f64)], w: impl Fn(f64) -> f64) -> Vec<Point> {
    let n = spine.len();
    let ws: Vec<f64> = (0..n).map(|i| w(i as f64 / (n - 1) as f64)).collect();
    let mut left = vec![]; let mut right = vec![];
    for (i, (p, h)) in spine.iter().enumerate() { let nn = normal(*h); left.push(add(*p, nn, ws[i])); right.push(add(*p, nn, -ws[i])); }
    let mut poly = left;
    let cap = |c: Point, h: f64, r: f64, start: f64, out: &mut Vec<Point>| { for k in 1..12 { let a = h + start - std::f64::consts::PI * k as f64 / 12.0; out.push(pt(c.x + a.cos() * r, c.y + a.sin() * r)); } };
    let (pe, he) = spine[n - 1]; cap(pe, he, ws[n - 1], std::f64::consts::FRAC_PI_2, &mut poly);
    poly.extend(right.into_iter().rev());
    let (p0, h0) = spine[0]; cap(p0, h0, ws[0], -std::f64::consts::FRAC_PI_2, &mut poly);
    largest(&tidy(&poly)).unwrap_or(poly)
}
fn largest(s: &Shapes) -> Option<Vec<Point>> { s.first().map(|sh| sh[0].clone()) }

/// A line offset from the spine by `f(u) * w(u)` over u in [from, to].
fn rail(spine: &[(Point, f64)], w: &dyn Fn(f64) -> f64, f: f64, from: f64, to: f64) -> Vec<Point> {
    let n = spine.len();
    spine.iter().enumerate().filter_map(|(i, (p, h))| { let u = i as f64 / (n - 1) as f64; (u >= from && u <= to).then(|| add(*p, normal(*h), f * w(u))) }).collect()
}

/// A moulded C- or S-scroll: the band, with a channel line along it. Also
/// returns the spine, and its outer (convex) edge over the middle, where a
/// frill can grow.
pub fn c_scroll(start: Point, heading: f64, a: Curl, middle: f64, mid_curv: f64, b: Curl, width: f64, channel: bool) -> (Part, Vec<Point>, Vec<Point>) {
    let spine = scroll_spine(start, heading, a, middle, mid_curv, b);
    let (la, lb) = (a.length(), b.length()); let total = la + middle + lb;
    let (ua, ub) = (la / total, 1.0 - lb / total);
    // half-width: full in the middle, narrowing into each eye, and always
    // inside the curl's radius so the inner edge doesn't fold
    let wa = a.eye * 0.55; let wb = b.eye * 0.55; let wm = width / 2.0;
    let w = move |u: f64| {
        let ta = if ua > 0.0 { smooth(u / ua) } else { 1.0 };
        let tb = if ub < 1.0 { smooth((1.0 - u) / (1.0 - ub)) } else { 1.0 };
        let mut v = wm;
        if ta < 1.0 { v = wa + (v - wa) * ta.powf(0.8); }
        if tb < 1.0 { v = wb + (v - wb) * tb.powf(0.8); }
        v
    };
    let outline = band(&spine, w);
    let mut lines = vec![];
    let side = if mid_curv >= 0.0 { -1.0 } else { 1.0 };
    if channel {
        // the moulding: a fillet line on the convex (outer) side of the middle
        lines.push(rail(&spine, &w, side * 0.35, ua * 0.35, 1.0 - (1.0 - ub) * 0.35));
    }
    let edge = rail(&spine, &w, side * 0.92, ua * 0.6, 1.0 - (1.0 - ub) * 0.6);
    (Part { outline, lines, pocket: false, seam: vec![] }, spine.iter().map(|q| q.0).collect(), edge)
}

/// A moulded rim from `p0` to `p1` along a circular arc bulging `sag` mm to
/// the right of p0→p1 (negative: to the left), with an optional small hook
/// at each end (`hook0`/`hook1`: +1 curls on with the arc, -1 against it, 0
/// none) of outer radius `hook`. Returns the part and its spine.
pub fn rim_through(p0: Point, p1: Point, sag: f64, width: f64, hook0: f64, hook1: f64, hook: f64) -> (Part, Vec<Point>) {
    let (dx, dy) = (p1.x - p0.x, p1.y - p0.y); let c = dx.hypot(dy); let a = dy.atan2(dx);
    let theta = 4.0 * (2.0 * sag / c).atan();
    let (len, k) = if sag.abs() < 1e-6 { (c, 0.0) } else { let r = (c * c / 4.0 + sag * sag) / (2.0 * sag.abs()); (r * theta.abs(), -theta / (r * theta.abs())) };
    let h0 = a + theta / 2.0;
    let arc_sign = if k >= 0.0 { 1.0 } else { -1.0 };
    let eye = hook * 0.3; let turn = 5.6; let rate = (hook / eye).ln() / turn; let lc = (hook - eye) / rate;
    // a curl leaving a point with heading h, curvature tightening to the eye
    let curl = |p: Point, h: f64, side: f64| integrate(p, h, lc, |s| side / (hook - rate * s).max(eye));
    let mut spine: Vec<(Point, f64)> = vec![];
    if hook0 != 0.0 {
        // walk backward from p0 (heading reversed, so the turn sense flips)
        let back = curl(p0, h0 + std::f64::consts::PI, -hook0 * arc_sign);
        spine.extend(back.iter().rev().map(|(p, h)| (*p, h + std::f64::consts::PI)));
        spine.pop();
    }
    let arc = integrate(p0, h0, len, |_| k);
    let (pe, he) = *arc.last().unwrap();
    let n0 = spine.len();
    spine.extend(arc);
    if hook1 != 0.0 { spine.pop(); spine.extend(curl(pe, he, hook1 * arc_sign)); }
    let n = spine.len();
    let (ua, ub) = (n0 as f64 / (n - 1) as f64, if hook1 != 0.0 { 1.0 - (lc / 0.4).round() / (n - 1) as f64 } else { 1.0 });
    let wm = width / 2.0; let we = eye * 0.6;
    let w = move |u: f64| {
        let mut v = wm;
        if ua > 0.0 && u < ua { v = we + (wm - we) * smooth(u / ua).powf(0.8); }
        if ub < 1.0 && u > ub { v = we + (wm - we) * smooth((1.0 - u) / (1.0 - ub)).powf(0.8); }
        v
    };
    let outline = band(&spine, w);
    let side = if k >= 0.0 { -1.0 } else { 1.0 };
    let lines = vec![rail(&spine, &w, side * 0.35, ua * 0.5, 1.0 - (1.0 - ub) * 0.5)];
    (Part { outline, lines, pocket: false, seam: vec![] }, spine.iter().map(|q| q.0).collect())
}

// ---- Round 3: measured from rococo relief references (2026-10-03) ----
// Rim cross-section, outside of the frame to the opening: a rounded outer
// roll at full height over ~45% of the width, a flat sunken channel at ~0.55
// height over ~40%, a narrow inner lip at ~0.8 height (~12-15%), then a sheer
// drop into the pierced opening. Rim width ~7-8% of the ornament's height.
// Leaves sit on the outer roll, rising a little above it, and run along the
// rim in overlapping rows all leaning one way (spaced ~0.6 leaf length);
// larger sprays fan out where the rim bends. The rim's ends roll into volutes.

/// A rim path: from `start` heading `heading`, curvature `kappa(s)` over
/// `length` mm, with a volute at each end (`hook0`/`hook1`: +1 curls on with
/// the path's turn there, -1 against it, 0 none; `eye0`/`eye1` the volutes'
/// outer radii). Returns the spine and the share of it where the main path
/// starts and ends.
pub fn rim_path(start: Point, heading: f64, length: f64, kappa: impl Fn(f64) -> f64, hook0: f64, eye0: f64, hook1: f64, eye1: f64) -> (Vec<(Point, f64)>, f64, f64) {
    let curl = |p: Point, h: f64, side: f64, r: f64| {
        let eye = r * 0.3; let turn = 6.8; let rate = (r / eye).ln() / turn; let lc = (r - eye) / rate;
        integrate(p, h, lc, move |s| side / (r - rate * s).max(eye))
    };
    let sgn = |k: f64| if k >= 0.0 { 1.0 } else { -1.0 };
    let mut spine: Vec<(Point, f64)> = vec![];
    if hook0 != 0.0 {
        let back = curl(start, heading + std::f64::consts::PI, -hook0 * sgn(kappa(0.0)), eye0);
        spine.extend(back.iter().rev().map(|(p, h)| (*p, h + std::f64::consts::PI)));
        spine.pop();
    }
    let n0 = spine.len();
    let main = integrate(start, heading, length, &kappa);
    let (pe, he) = *main.last().unwrap();
    spine.extend(main);
    let n1 = spine.len() - 1;
    if hook1 != 0.0 { spine.pop(); spine.extend(curl(pe, he, hook1 * sgn(kappa(length)), eye1)); }
    let n = (spine.len() - 1) as f64;
    (spine, n0 as f64 / n, n1 as f64 / n)
}

/// A rim path through control points (a smooth Catmull-Rom curve), with a
/// volute at each end as in `rim_path` (the sign is relative to the curve's
/// own turn at that end).
pub fn rim_along(ctrl: &[Point], hook0: f64, eye0: f64, hook1: f64, eye1: f64) -> (Vec<(Point, f64)>, f64, f64) {
    let n = ctrl.len();
    let mut dense = vec![];
    for i in 0..n - 1 {
        let (p0, p1, p2, p3) = (ctrl[i.saturating_sub(1)], ctrl[i], ctrl[i + 1], ctrl[(i + 2).min(n - 1)]);
        for k in 0..40 { let t = k as f64 / 40.0; let (t2, t3) = (t * t, t * t * t);
            let f = |a: f64, b: f64, c: f64, d: f64| 0.5 * ((2.0 * b) + (-a + c) * t + (2.0 * a - 5.0 * b + 4.0 * c - d) * t2 + (-a + 3.0 * b - 3.0 * c + d) * t3);
            dense.push(pt(f(p0.x, p1.x, p2.x, p3.x), f(p0.y, p1.y, p2.y, p3.y))); }
    }
    dense.push(ctrl[n - 1]);
    rim_on(&dense, hook0, eye0, hook1, eye1)
}

/// A rim path along a dense polyline (its main path), with a volute at each
/// end as in `rim_along`.
pub fn rim_on(dense: &[Point], hook0: f64, eye0: f64, hook1: f64, eye1: f64) -> (Vec<(Point, f64)>, f64, f64) {
    let main = resample(dense);
    let (s0, s1) = end_turns(&main);
    rim_sided(main, -hook0 * s0, eye0, hook1 * s1, eye1)
}
/// Which way a dense path turns at its start and its end (+1: heading increasing).
pub fn rim_end_turns(dense: &[Point]) -> (f64, f64) { end_turns(&resample(dense)) }
/// A rim along a dense path with its volutes curling to fixed sides: `side0`
/// for the start volute (relative to the path's heading reversed, as it is
/// traced back from the start), `side1` for the end (+1 right on screen,
/// -1 left, 0 no volute).
pub fn rim_on_sides(dense: &[Point], side0: f64, eye0: f64, side1: f64, eye1: f64) -> (Vec<(Point, f64)>, f64, f64) { rim_sided(resample(dense), side0, eye0, side1, eye1) }
fn end_turns(main: &[(Point, f64)]) -> (f64, f64) {
    let turn_at = |a: usize, b: usize| if main[b].1 - main[a].1 >= 0.0 { 1.0 } else { -1.0 };
    let m = main.len();
    (turn_at(0, 12.min(m - 1)), turn_at(m.saturating_sub(13), m - 1))
}
/// A dense path resampled evenly at 0.4 mm, with smoothly unwrapped headings.
fn resample(dense: &[Point]) -> Vec<(Point, f64)> {
    // resample evenly at 0.4 mm with headings
    let mut main: Vec<(Point, f64)> = vec![];
    let mut acc = 0.0; let mut last = dense[0];
    for w in dense.windows(2) {
        let d = (w[1].x - w[0].x).hypot(w[1].y - w[0].y);
        acc += d;
        if acc >= 0.4 || main.is_empty() { let h = (w[1].y - w[0].y).atan2(w[1].x - w[0].x); main.push((last, h)); acc = 0.0; }
        last = w[1];
    }
    // unwrap headings so they change smoothly
    for i in 1..main.len() { let mut h = main[i].1; while h - main[i - 1].1 > std::f64::consts::PI { h -= std::f64::consts::TAU; } while h - main[i - 1].1 < -std::f64::consts::PI { h += std::f64::consts::TAU; } main[i].1 = h; }
    main
}
fn rim_sided(main: Vec<(Point, f64)>, side0: f64, eye0: f64, side1: f64, eye1: f64) -> (Vec<(Point, f64)>, f64, f64) {
    let m = main.len();
    let curl = |p: Point, h: f64, side: f64, r: f64| {
        let eye = r * 0.3; let turn = 6.8; let rate = (r / eye).ln() / turn; let lc = (r - eye) / rate;
        integrate(p, h, lc, move |s| side / (r - rate * s).max(eye))
    };
    let mut spine: Vec<(Point, f64)> = vec![];
    if side0 != 0.0 {
        let back = curl(main[0].0, main[0].1 + std::f64::consts::PI, side0, eye0);
        spine.extend(back.iter().rev().map(|(p, h)| (*p, h + std::f64::consts::PI)));
        spine.pop();
    }
    let n0 = spine.len();
    let (pe, he) = main[m - 1];
    spine.extend(main);
    let n1 = spine.len() - 1;
    if side1 != 0.0 { spine.pop(); spine.extend(curl(pe, he, side1, eye1)); }
    let nn = (spine.len() - 1) as f64;
    (spine, n0 as f64 / nn, n1 as f64 / nn)
}

/// A moulded rim on a spine: half-width `wm` on the main path (between
/// shares `ua` and `ub`), narrowing into each volute. `outer` is the side of
/// the frame's outside (+1: right of the spine's heading). Lines: the step
/// from the outer roll down to the flat channel, and the step up to the
/// inner lip, at the measured shares of the width.
pub fn moulded_rim(spine: &[(Point, f64)], ua: f64, ub: f64, wm: f64, outer: f64) -> Part {
    let we = wm * 0.3;
    let w = move |u: f64| {
        let mut v = wm;
        if ua > 0.0 && u < ua { v = we + (wm - we) * smooth(u / ua).powf(0.7); }
        if ub < 1.0 && u > ub { v = we + (wm - we) * smooth((1.0 - u) / (1.0 - ub)).powf(0.7); }
        v
    };
    let outline = band(spine, w);
    // offsets across the spine (as shares of the half-width, + = outer side):
    // outer edge at +1, roll/channel step at 1 - 2*0.45, channel/lip step at 1 - 2*0.86
    let (a0, a1) = (ua * 0.35, 1.0 - (1.0 - ub) * 0.35);
    let lines = vec![rail(spine, &w, outer * 0.10, a0, a1), rail(spine, &w, outer * -0.72, a0, a1)];
    Part { outline, lines, pocket: false, seam: vec![] }
}

/// A leaf spine: `length` from `start` heading `heading`, bending to `side`
/// by `bend` radians over its length, the tip turning over by `turnover`.
pub fn leaf_spine(start: Point, heading: f64, length: f64, side: f64, bend: f64, turnover: f64) -> Vec<Point> {
    integrate(start, heading, length, |s| { let u = s / length; side * (bend * 2.0 * u + turnover * 5.0 * smooth((u - 0.65) / 0.35)) / length }).into_iter().map(|q| q.0).collect()
}

/// An acanthus leaf (the user's leaf model) on a spine, as a part.
pub fn acanthus_leaf(spine: &[Point], spec: &crate::acanthus::LeafSpec, flip: bool) -> Part {
    let l = crate::acanthus::baroque_leaf(spine, spec, flip);
    let outline = largest(&tidy(&l.polygon)).unwrap_or(l.polygon);
    let mut lines = l.folds; lines.extend(l.cuts);
    Part { outline, lines, pocket: false, seam: vec![] }
}

/// A soft rococo leaf on a spine, as on the reference cartouches: the
/// acanthus belly (narrow stalk, full belly before the middle, long taper to
/// a round tip) divided into `lobes` broad rounded lobes per side by soft
/// U-shaped notches (`depth` = their depth as a share of the half-width), the
/// lobes leaning toward the tip; the second side offset by half a lobe and a
/// little narrower. Lines: a doubled midrib, a pipe from the midrib into each
/// lobe, and a short groove in from each notch.
pub fn soft_leaf(spine: &[Point], half: f64, lobes: usize, depth: f64) -> Part {
    let len = crate::geometry::line_length(spine).max(1.0);
    let place = |u: f64, v: f64| { let (p, a) = crate::geometry::line_frame(spine, u.clamp(0.0, 1.0)); pt(p.x - a.sin() * v, p.y + a.cos() * v) };
    let env = |u: f64| { let stalk = 0.08; if u < stalk { return half * (0.18 + 0.1 * u / stalk); } let t = (u - stalk) / (1.0 - stalk); half * ((std::f64::consts::PI * t.powf(0.75)).sin().max(0.0).powf(0.7) * 0.92 + 0.08 * (1.0 - t)) };
    let (u0, u1) = (0.16, 0.9);
    // notch centres for a side, shifted by `phase` lobes
    let notches = |phase: f64| -> Vec<f64> { (1..lobes).map(|i| u0 + (u1 - u0) * ((i as f64 + phase) / lobes as f64).powf(1.1)).filter(|u| *u < u1).collect() };
    let edge = |u: f64, ns: &[f64]| {
        let sigma = (u1 - u0) / lobes as f64 * 0.26;
        // a soft notch, steeper on its tip side so each lobe leans forward
        let dip: f64 = ns.iter().map(|c| { let x = (u - c) / sigma; let x = if x > 0.0 { x * 1.3 } else { x }; (-x * x).exp() }).fold(0.0, f64::max);
        env(u) * (1.0 - depth * dip)
    };
    let (na, nb) = (notches(0.0), notches(0.5));
    let n = 400;
    let mut poly: Vec<Point> = (0..=n).map(|i| { let u = i as f64 / n as f64 * 0.995; place(u, edge(u, &na)) }).collect();
    // round tip
    let tip = env(0.995).max(0.6);
    let (pe, ae) = crate::geometry::line_frame(spine, 1.0);
    for k in 1..10 { let a = ae + std::f64::consts::FRAC_PI_2 - std::f64::consts::PI * k as f64 / 10.0; poly.push(pt(pe.x + a.cos() * tip * 0.5, pe.y + a.sin() * tip * 0.5)); }
    poly.extend((0..=n).rev().map(|i| { let u = i as f64 / n as f64 * 0.995; place(u, -0.92 * edge(u, &nb)) }));
    let outline = largest(&tidy(&poly)).unwrap_or(poly);
    let mut lines: Vec<Vec<Point>> = vec![];
    for s in [1.0, -1.0] { lines.push((0..=40).map(|i| { let u = 0.05 + 0.82 * i as f64 / 40.0; place(u, s * env(u) * 0.06) }).collect()); }
    for (ns, s, k) in [(&na, 1.0, 1.0), (&nb, -1.0, 0.92)] {
        let mut bounds = vec![u0 * 0.7]; bounds.extend(ns.iter().copied()); bounds.push(u1);
        for w in bounds.windows(2) {
            // pipe into the lobe: from the midrib, sweeping toward the tip
            let (a, b) = (w[0], w[1]); let m = a + (b - a) * 0.62;
            lines.push((0..=12).map(|j| { let t = j as f64 / 12.0; let u = a + (m - a) * t; place(u, s * k * env(u) * (0.08 + 0.55 * t.powf(1.3))) }).collect());
        }
        for c in ns.iter() { lines.push((0..=6).map(|j| { let t = j as f64 / 6.0; let u = c - 0.02 * t; place(u, s * k * edge(*c, ns) * (1.0 - 0.38 * t)) }).collect()); }
    }
    let _ = len;
    Part { outline, lines, pocket: false, seam: vec![] }
}

/// A band with different half-widths on its left (`wl`) and right (`wr`)
/// of the spine, round at both ends.
pub fn band2(spine: &[(Point, f64)], wl: impl Fn(f64) -> f64, wr: impl Fn(f64) -> f64) -> Vec<Point> {
    let n = spine.len();
    let us = |i: usize| i as f64 / (n - 1) as f64;
    let mut poly: Vec<Point> = spine.iter().enumerate().map(|(i, (p, h))| add(*p, normal(*h), wr(us(i)))).collect();
    let (pe, he) = spine[n - 1]; let (a, b) = (wr(1.0), wl(1.0)); let r = (a + b) / 2.0; let c = add(pe, normal(he), (a - b) / 2.0);
    for k in 1..12 { let t = he + std::f64::consts::FRAC_PI_2 - std::f64::consts::PI * k as f64 / 12.0; poly.push(pt(c.x + t.cos() * r, c.y + t.sin() * r)); }
    poly.extend(spine.iter().enumerate().rev().map(|(i, (p, h))| add(*p, normal(*h), -wl(us(i)))));
    let (p0, h0) = spine[0]; let (a, b) = (wr(0.0), wl(0.0)); let r = (a + b) / 2.0; let c = add(p0, normal(h0), (a - b) / 2.0);
    for k in 1..12 { let t = h0 - std::f64::consts::FRAC_PI_2 - std::f64::consts::PI * k as f64 / 12.0; poly.push(pt(c.x + t.cos() * r, c.y + t.sin() * r)); }
    largest(&tidy(&poly)).unwrap_or(poly)
}

/// How a frond's end turns over (as on the reference carvings): the rib
/// runs past the tip, U-turns toward the blade side and comes back over the
/// leaf as a smooth band, the leaf's underside, lying on top of the fingers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Turnover {
    /// No turnover: the rib just curls over at its tip.
    None,
    /// The end turns back on itself and finishes in a fat rounded roll.
    Roll,
    /// The tip folds back across the blade as a flap with two small fingers.
    Flap,
    /// The whole end arches back in a big C over the fingers and rolls in.
    Curl,
}

/// A rococo acanthus frond, as on the reference cartouches: a rib from
/// `base` along `heading`, `length` long, bending toward `side` (+1: right of
/// the heading) by `bend` radians and curling over at its tip. On its `side`
/// it carries a broad blade whose edge splits into `fingers` wide tongues:
/// they splay most near the base (`splay` radians) and less toward the tip,
/// get shorter toward the tip, sweep forward with the rib and end in blunt
/// rounded tips that hook back outward. The fingers come out from under the
/// blade. `width` is the blade's widest reach from the rib. Each finger has
/// a groove down it; the rib a midrib groove. Parts are back to front.
pub fn frond(base: Point, heading: f64, length: f64, side: f64, bend: f64, fingers: usize, width: f64, splay: f64) -> Vec<Part> {
    frond_with(base, heading, length, side, bend, fingers, width, splay, Turnover::None)
}

/// A frond with its end turned over (see `Turnover`).
pub fn frond_with(base: Point, heading: f64, length: f64, side: f64, bend: f64, fingers: usize, width: f64, splay: f64, turn: Turnover) -> Vec<Part> {
    let turned = turn != Turnover::None;
    let tip_curl = if turned { 0.0 } else { 10.0 };
    let rib = integrate(base, heading, length, |s| { let u = s / length; side * (bend * 2.0 * u + tip_curl * smooth((u - 0.76) / 0.24)) / length });
    let n = rib.len() - 1;
    // the blade: full on the finger side, narrow on the back; a turned end
    // stays broad to the fold
    let taper = if turned { 0.0 } else { 0.5 };
    let reach = move |u: f64| width * 0.55 * (0.3 + 0.7 * (std::f64::consts::PI * (u / 0.9).min(1.0).powf(0.6)).sin().max(0.0).max(if turned { smooth((u - 0.3) / 0.3) * 0.8 } else { 0.0 })) * (1.0 - taper * smooth((u - 0.6) / 0.4)) * (0.35 + 0.65 * smooth(u / 0.2));
    let back = move |u: f64| width * 0.2 * (1.0 - 0.6 * u);
    let mut outlines: Vec<Vec<Point>> = vec![]; let mut lines: Vec<Vec<Point>> = vec![];
    for i in (0..fingers).rev() {
        let t = if fingers > 1 { i as f64 / (fingers - 1) as f64 } else { 0.5 };
        let u = 0.18 + 0.58 * t;
        let (p, h) = rib[(n as f64 * u) as usize];
        let fw0 = width * (0.44 - 0.14 * t);
        // root far enough out that the finger's back edge stays inside the blade
        let root = add(p, normal(h), side * (reach(u) * 0.2).max(fw0 - back(u) * 0.9));
        let fl = length * (0.6 - 0.3 * t) * (0.92 + 0.16 * ((i * 7919 % 5) as f64 / 4.0));
        let fh = h + side * (splay * (1.0 - t).powf(0.7) + 0.25);
        // sweep forward (against the splay), then hook the tip back outward
        let spine = integrate(root, fh, fl, |s| { let v = s / fl; (-side * 1.3 * (1.0 - v) + side * 6.0 * smooth((v - 0.62) / 0.38)) / fl });
        let fw = move |v: f64| fw0 * (1.0 - 0.62 * v.powf(1.1)) * (0.85 + 0.15 * smooth(v / 0.2));
        outlines.push(band(&spine, fw));
        // the finger's vein: from the rib, through the blade, down the finger
        let mut vein: Vec<Point> = (0..=6).map(|k| { let a = k as f64 / 6.0; let (q, hq) = rib[(n as f64 * (u - 0.08 * (1.0 - a)).max(0.0)) as usize]; add(q, normal(hq), side * reach(u) * 0.3 * a) }).collect();
        vein.extend(rail(&spine, &fw, -side * 0.1, 0.05, 0.84));
        lines.push(vein);
    }
    outlines.push(band2(&rib, move |u| if side > 0.0 { back(u) } else { reach(u) }, move |u| if side > 0.0 { reach(u) } else { back(u) }));
    // one surface: the blade and its fingers united
    let refs: Vec<&[Point]> = outlines.iter().map(|o| o.as_slice()).collect();
    let outline = largest(&union(&refs)).unwrap_or_else(|| outlines.last().unwrap().clone());
    lines.push(rail(&rib, &|u| back(u), 0.0, 0.04, 0.9));
    let mut parts = vec![Part { outline, lines, pocket: false, seam: vec![] }];
    if turned {
        // the fold half becomes part of the leaf's own silhouette; the
        // returning flap lies on top
        let (fold, crease, flap) = turnover_band(&rib, reach(1.0), back(1.0), width, side, length, turn);
        parts[0].lines.push(crease);
        let leaf = parts[0].outline.clone();
        parts[0].outline = largest(&union(&[leaf.as_slice(), fold.as_slice()])).unwrap_or(leaf);
        parts.push(flap);
    }
    parts
}

/// The turned-over end of a frond: from the rib's end, a U-turn toward the
/// blade side, a run back over the leaf, and an ending by kind. Split at the
/// middle of the U-turn: returns the fold half (to join the leaf's
/// silhouette, so the outline runs unbroken round the fold) and the
/// returning flap (on top, its start seamed into the fold).
fn turnover_band(rib: &[(Point, f64)], reach: f64, back: f64, width: f64, side: f64, length: f64, turn: Turnover) -> (Vec<Point>, Vec<Point>, Part) {
    let (f, hf) = *rib.last().unwrap();
    let wf = width * 0.42;
    // the Curl arches out on the rib's back; Roll and Flap fold over the blade
    let side = if turn == Turnover::Curl { -side } else { side };
    // the band's centre line starts at the middle of the blade's end
    let c = add(f, normal(hf), (reach - back) / 2.0 * if turn == Turnover::Curl { -side } else { side });
    let (r, back_run, knob) = match turn {
        Turnover::Roll => (wf * 1.15, length * 0.1, true),
        Turnover::Flap => (wf * 1.05, length * 0.3, false),
        _ => (length * 0.15, length * 0.08, true),
    };
    let pi = std::f64::consts::PI;
    // a little past the tip first, so the fold stands proud of it
    let lead = wf * 0.6;
    let total_u = lead + pi * r + back_run;
    let mut spine = integrate(c, hf, total_u, |s| if s < lead { 0.0 } else if s < lead + pi * r { side / r } else { -side * 0.3 / back_run.max(1.0) });
    let main_n = spine.len();
    if knob {
        // roll in: curvature tightening from the band's width to a small eye
        let (p, h) = *spine.last().unwrap();
        let (r0, r1) = (wf * 1.25, wf * 0.62); let tn = 1.3 * pi; let rate = (r0 / r1).ln() / tn; let lk = (r0 - r1) / rate;
        spine.pop();
        spine.extend(integrate(p, h, lk, |s| side / (r0 - rate * s).max(r1)));
    }
    let n = spine.len() as f64 - 1.0;
    let um = main_n as f64 / n;
    let w = move |u: f64| {
        let base = wf * (0.68 + 0.32 * smooth(u / (um * 0.45).max(0.01))) * (1.0 - 0.3 * smooth((u - um * 0.5) / (um * 0.5).max(0.01)));
        if knob && u > um { base * (1.0 - 0.6 * smooth((u - um) / (1.0 - um))) } else if !knob { base * (1.0 - 0.5 * smooth((u - 0.7) / 0.3)) } else { base }
    };
    let hf0 = spine[0].1;
    let k = spine.iter().position(|q| (q.1 - hf0).abs() >= pi * 0.5).unwrap_or(spine.len() / 2);
    let kn = k as f64 / n;
    let fold = band(&spine[..=(k + 1).min(spine.len() - 1)], |u: f64| w(u * kn));
    let s0 = k.saturating_sub(1); let s0u = s0 as f64 / n;
    let mut outline = band(&spine[s0..], |u: f64| w(s0u + u * (1.0 - s0u)));
    // the flap's start cap lies inside the fold: don't draw its edges there,
    // but keep the outer edge the two share (the seam is the fold, shrunk)
    // and only near the join (the flap returns over the fold further on)
    let (pk, _) = spine[k]; let rk = w(kn) * 1.3;
    let disc: Vec<Point> = (0..32).map(|j| { let a = std::f64::consts::TAU * j as f64 / 32.0; pt(pk.x + a.cos() * rk, pk.y + a.sin() * rk) }).collect();
    let seam = largest(&crate::booleans::intersect(&offset(&union(&[fold.as_slice()]), -0.35), &union(&[disc.as_slice()]))).unwrap_or_default();
    let mut lines = vec![];
    // the underside's raised midrib, as a pair of close grooves
    let (a0, a1) = (((lead + pi * r * 0.5) / total_u * um).max(kn + 0.02), if knob { um + (1.0 - um) * 0.3 } else { 0.85 });
    for k in [0.08, -0.08] { lines.push(rail(&spine, &w, k, a0, a1)); }
    if turn == Turnover::Flap {
        // a finger peeling off the flap's free edge, hooked
        let mut outs = vec![outline.clone()];
        for (k, uu) in [(0, 0.78)] {
            let i = (n * uu) as usize; let (p, h) = spine[i];
            let fwk = wf * (0.5 - 0.1 * k as f64); let fl = wf * (3.0 - 0.5 * k as f64);
            let root = add(p, normal(h), -side * w(uu) * 0.45);
            let fs = integrate(root, h - side * 0.6, fl, |s| { let v = s / fl; (side * 0.8 * (1.0 - v) - side * 5.0 * smooth((v - 0.6) / 0.4)) / fl });
            outs.push(band(&fs, move |v: f64| fwk * (1.0 - 0.5 * v)));
            lines.push(rail(&fs, &move |v: f64| fwk * (1.0 - 0.5 * v), 0.0, 0.2, 0.8));
        }
        let refs: Vec<&[Point]> = outs.iter().map(|o| o.as_slice()).collect();
        outline = largest(&union(&refs)).unwrap_or(outline);
    }
    // the crease: where the leaf's top surface turns down into the fold, a
    // line across the band (drawn on the leaf, hidden where the flap lies)
    let ic = ((lead / total_u * um) * n) as usize;
    let (pc, hc) = spine[ic]; let wc = w(ic as f64 / n) * 0.92;
    let crease: Vec<Point> = (0..=8).map(|j| { let t = j as f64 / 8.0 * 2.0 - 1.0; let bow = (1.0 - t * t) * wc * 0.25; add(add(pc, normal(hc), t * wc), pt(hc.cos(), hc.sin()), bow) }).collect();
    (fold, crease, Part { outline, lines, pocket: false, seam })
}

/// A run of `count` small fronds along a rim's outer roll, between shares
/// `from` and `to` of its spine, flowing toward `to`, fingers on the outer
/// side; the frond nearest `from` lies on top, like feathers.
pub fn frond_run(spine: &[(Point, f64)], from: f64, to: f64, outer: f64, wm: f64, count: usize, length: f64, fingers: usize) -> Vec<Part> {
    frond_run_with(spine, from, to, outer, wm, count, length, fingers, Turnover::None)
}
/// A frond run whose second-to-last frond turns over as `last` (the last,
/// nearest `to`, often tucks under whatever crowns the rim).
pub fn frond_run_with(spine: &[(Point, f64)], from: f64, to: f64, outer: f64, wm: f64, count: usize, length: f64, fingers: usize, last: Turnover) -> Vec<Part> {
    let n = spine.len() - 1;
    let dir = if to >= from { 1.0 } else { -1.0 };
    let mut out_parts: Vec<Vec<Part>> = (0..count).map(|i| {
        let u = from + (to - from) * (i as f64 + 0.25) / count as f64;
        let (p, h) = spine[((n as f64) * u).round() as usize];
        let h = if dir > 0.0 { h } else { h + std::f64::consts::PI };
        let out = outer * dir;
        let l = length * (0.88 + 0.24 * ((i * 7919 % 5) as f64 / 4.0));
        frond_with(add(p, normal(h), out * wm * 0.1), h + out * 0.12, l, out, 0.3, fingers, l * 0.26, 0.75, if i + 2 == count.max(2) { last } else { Turnover::None })
    }).collect();
    out_parts.reverse();
    out_parts.into_iter().flatten().collect()
}

/// A small rosette as set on rococo rims: `petals` rounded petals of radius
/// `r` round a domed boss; a groove down each petal.
pub fn rosette(c: Point, r: f64, petals: usize, turn: f64) -> Vec<Part> {
    let mut poly = vec![];
    let n = petals * 24;
    for i in 0..n {
        let a = turn + std::f64::consts::TAU * i as f64 / n as f64;
        let f = (i % 24) as f64 / 24.0;
        let rr = r * (0.62 + 0.38 * (std::f64::consts::PI * f).sin().powf(0.6));
        poly.push(pt(c.x + a.cos() * rr, c.y + a.sin() * rr));
    }
    let lines = (0..petals).map(|k| { let a = turn + std::f64::consts::TAU * (k as f64 + 0.5) / petals as f64; vec![pt(c.x + a.cos() * r * 0.42, c.y + a.sin() * r * 0.42), pt(c.x + a.cos() * r * 0.8, c.y + a.sin() * r * 0.8)] }).collect();
    let boss: Vec<Point> = (0..32).map(|k| { let a = std::f64::consts::TAU * k as f64 / 32.0; pt(c.x + a.cos() * r * 0.34, c.y + a.sin() * r * 0.34) }).collect();
    vec![Part { outline: poly, lines, pocket: false, seam: vec![] }, Part { outline: boss, lines: vec![], pocket: false, seam: vec![] }]
}

/// The leaf for rims, as on the reference cartouches: the user's broad-belly
/// acanthus, but each lobe one rounded finger and no eyes, so it stays soft
/// at this size (two fingers per lobe read as saw teeth here).
pub fn rim_leaf_spec(width: f64) -> crate::acanthus::LeafSpec {
    crate::acanthus::LeafSpec { groups: 2, fingers: 1, notch: 0.24, eye: 0.0, cut: 0.0, stalk: 0.08, width, ..Default::default() }
}

/// A run of `count` overlapping leaves on the outer roll of a rim, between
/// shares `from` and `to` of its spine, flowing toward `to`. Each leaf starts
/// on the roll, leans out by `lean` and turns over at the tip; the one
/// nearest `from` lies on top (they overlap like feathers).
pub fn leaf_run(spine: &[(Point, f64)], from: f64, to: f64, outer: f64, wm: f64, count: usize, length: f64, lean: f64) -> Vec<Part> {
    let n = spine.len() - 1;
    let dir = if to >= from { 1.0 } else { -1.0 };
    let mut parts: Vec<Part> = (0..count).map(|i| {
        let u = from + (to - from) * (i as f64 + 0.3) / count as f64;
        let (p, h) = spine[((n as f64) * u).round() as usize];
        let h = if dir > 0.0 { h } else { h + std::f64::consts::PI };
        // which side is outward for a leaf travelling this way
        let out = outer * dir;
        let base = add(p, normal(h), out * wm * 0.1);
        let l = length * (0.85 + 0.3 * ((i * 7919 % 5) as f64 / 4.0 - 0.5));
        let s = leaf_spine(base, h + out * lean, l, out, 0.3, 0.55);
        soft_leaf(&s, l * 0.27, 3, 0.3)
    }).collect();
    parts.reverse();
    parts
}

/// A spray of `count` leaves fanned over `spread` round `heading` from
/// `base`, the middle ones longest, all turning over toward `side`.
pub fn leaf_spray(base: Point, heading: f64, spread: f64, count: usize, length: f64, side: f64) -> Vec<Part> {
    (0..count).map(|i| {
        let t = if count > 1 { i as f64 / (count - 1) as f64 } else { 0.5 };
        let h = heading - spread / 2.0 + spread * t;
        let l = length * (0.6 + 0.4 * (std::f64::consts::PI * t).sin());
        let s = leaf_spine(base, h, l, side, 0.3 + 0.3 * t, 0.65);
        soft_leaf(&s, l * 0.26, 3, 0.3)
    }).collect()
}

/// A rococo shell: hinge `h`, axis `axis`, fan `span` radians, `ribs` flutes,
/// reach `r` with `asym` (-1..1: which side reaches further), flutes swirled
/// by `twist` radians at the rim, a scalloped lip (`scallop` = lobe depth as a
/// share of its width).
pub fn shell(h: Point, axis: f64, span: f64, ribs: usize, r: f64, asym: f64, twist: f64, scallop: f64) -> Part {
    let reach = |t: f64| r * (0.62 + 0.38 * (std::f64::consts::PI * t).sin().powf(0.6)) * (1.0 + asym * (t - 0.5) * 0.8);
    let rib = |t: f64, k: f64| { // point at share k of the reach along flute t
        let a = axis - span / 2.0 + span * t + twist * k * k;
        let d = reach(t) * k;
        pt(h.x + a.cos() * d, h.y + a.sin() * d)
    };
    let mut poly = vec![];
    for k in 0..=20 { poly.push(rib(0.0, 0.08 + 0.92 * k as f64 / 20.0)); }
    for i in 0..ribs {
        let (t0, t1) = (i as f64 / ribs as f64, (i + 1) as f64 / ribs as f64);
        let (a, b) = (rib(t0, 1.0), rib(t1, 1.0));
        let (dx, dy) = (b.x - a.x, b.y - a.y); let len = dx.hypot(dy);
        let nn = pt(dy / len, -dx / len); // outward when the fan turns clockwise on screen
        let sign = { let m = pt((a.x + b.x) / 2.0 - h.x, (a.y + b.y) / 2.0 - h.y); if m.x * nn.x + m.y * nn.y >= 0.0 { 1.0 } else { -1.0 } };
        for k in 1..=12 {
            let s = k as f64 / 12.0;
            // the lobe leans toward the swirl, like a frilled lip
            let bulge = scallop * len * (std::f64::consts::PI * s).sin() * (1.0 + 0.35 * (s - 0.5));
            let base = rib(t0 + (t1 - t0) * s, 1.0);
            poly.push(add(base, nn, sign * bulge));
        }
    }
    for k in (0..=20).rev() { poly.push(rib(1.0, 0.08 + 0.92 * k as f64 / 20.0)); }
    // the hinge: a small round boss, unioned on
    let boss: Vec<Point> = (0..32).map(|k| { let a = std::f64::consts::TAU * k as f64 / 32.0; pt(h.x + a.cos() * r * 0.1, h.y + a.sin() * r * 0.1) }).collect();
    let fan = largest(&tidy(&poly)).unwrap_or(poly);
    let outline = largest(&union(&[fan.as_slice(), boss.as_slice()])).unwrap_or(fan);
    let mut lines = vec![];
    for i in 1..ribs { let t = i as f64 / ribs as f64; lines.push((0..=24).map(|k| rib(t, 0.14 + 0.86 * k as f64 / 24.0)).collect()); }
    // a hinge line across the base of the flutes
    lines.push((0..=24).map(|k| rib(k as f64 / 24.0, 0.14)).collect());
    Part { outline, lines, pocket: false, seam: vec![] }
}

/// A crimped strip along `base` (open polyline), on its left (`side` +1) or
/// right: depth `d`, `waves` crimps, each wave leaning forward. Crimp lines
/// run from each notch back toward the base.
pub fn frill(base: &[Point], side: f64, d: f64, waves: usize, broken: f64) -> Part {
    let n = base.len();
    let mut s = vec![0.0]; for i in 1..n { s.push(s[i - 1] + (base[i].x - base[i - 1].x).hypot(base[i].y - base[i - 1].y)); }
    let total = s[n - 1];
    let head = |i: usize| { let (a, b) = (base[i.saturating_sub(1)], base[(i + 1).min(n - 1)]); (b.y - a.y).atan2(b.x - a.x) };
    let mut outer = vec![]; let mut notches = vec![];
    for i in 0..n {
        let u = s[i] / total;
        let ph = u * waves as f64; let f = ph.fract();
        // a leaning crest, like a cock's comb: rises fast, falls slowly
        // into the next notch
        let wave = if f < 0.3 { smooth(f / 0.3).powf(0.7) } else { 1.0 - smooth((f - 0.3) / 0.7).powf(1.4) };
        // the strip swells in the middle and fades into the base at both ends
        let env = (std::f64::consts::PI * u).sin().powf(0.5);
        let k = (ph.floor() as usize * 7919) % 13; // some crests stand shorter: a broken edge
        let short = if broken > 0.0 && k < 4 { 1.0 - broken * 0.5 } else { 1.0 };
        let depth = d * env * (0.45 + 0.55 * wave * short);
        outer.push(add(base[i], normal(head(i)), side * depth));
        if i > 0 && (s[i - 1] / total * waves as f64).floor() != ph.floor() && u < 0.97 { notches.push((i, depth)); }
    }
    let mut poly = base.to_vec(); poly.extend(outer.iter().rev());
    let outline = largest(&tidy(&poly)).unwrap_or(poly);
    // crimp grooves: from each notch, leaning back, down toward the base
    let lines = notches.iter().filter_map(|&(i, _)| {
        let back = base.len() / (waves * 3).max(1);
        let j = i.checked_sub(back)?;
        Some((0..=8).map(|k| { let t = k as f64 / 8.0; let m = i - ((i - j) as f64 * t) as usize; lerp_pt(outer[i], add(base[m], normal(head(m)), side * 1.2), t * 0.85) }).collect())
    }).collect();
    Part { outline, lines, pocket: false, seam: vec![] }
}

/// A bundle of flame tongues from `base`: `count` tongues fanned over
/// `spread` around `heading`, each `length` (the outer ones shorter), curling
/// to `side`, ending in a round tip (no spikes).
pub fn flames(base: Point, heading: f64, spread: f64, count: usize, length: f64, width: f64, side: f64) -> Vec<Part> {
    (0..count).map(|i| {
        let t = if count > 1 { i as f64 / (count - 1) as f64 } else { 0.5 };
        let h0 = heading - spread / 2.0 + spread * t;
        // the middle tongue is longest; tongues on the outside of the bend
        // shorter, so the bundle leans like a flame
        let l = length * (0.6 + 0.4 * (std::f64::consts::PI * t).sin()) * (1.0 - 0.25 * if side > 0.0 { 1.0 - t } else { t });
        // a licking tongue: bending more and more toward the tip, one way
        let spine = integrate(base, h0, l, |s| { let u = s / l; side * 3.6 * u * u / l });
        let tip = 1.2_f64.max(width * 0.15);
        let w = move |u: f64| tip + (width / 2.0 - tip) * (1.0 - smooth(u)).powf(0.8) * (0.8 + 0.2 * (std::f64::consts::PI * u.min(0.5)).sin());
        let outline = band(&spine, w);
        let lines = vec![rail(&spine, &w, 0.0, 0.12, 0.7)];
        Part { outline, lines, pocket: false, seam: vec![] }
    }).collect()
}

/// A crinkled rocaille leaf: from `base` along `heading`, bending by `bend`
/// radians in all (more toward the tip), a broad belly of half-width
/// `width / 2` tapering to a round tip, both edges softly crimped (`waves`
/// leaning waves, depth `crimp` as a share of the half-width). A midrib and a
/// groove from each notch toward it.
pub fn crimped_leaf(base: Point, heading: f64, length: f64, width: f64, bend: f64, waves: usize, crimp: f64) -> Part {
    let spine = integrate(base, heading, length, |s| { let u = s / length; bend * 2.0 * u / length });
    let n = spine.len();
    let tip = 1.2_f64.max(width * 0.06);
    let env = |u: f64| tip + (width / 2.0 - tip) * (std::f64::consts::PI * u.powf(0.75)).sin().max(0.0).powf(0.7);
    let wave = |u: f64, phase: f64| { let f = (u * waves as f64 + phase).fract(); if f < 0.3 { smooth(f / 0.3) } else { 1.0 - smooth((f - 0.3) / 0.7) } };
    let mut left = vec![]; let mut right = vec![];
    let (mut nl, mut nr) = (vec![], vec![]);
    for (i, (p, h)) in spine.iter().enumerate() {
        let u = i as f64 / (n - 1) as f64;
        let fade = smooth(u / 0.15) * smooth((1.0 - u) / 0.12);
        let (wl, wr) = (wave(u, 0.0), wave(u, 0.5));
        let el = env(u) * (1.0 - crimp * fade * (1.0 - wl));
        let er = env(u) * (1.0 - crimp * fade * (1.0 - wr));
        left.push(add(*p, normal(*h), el)); right.push(add(*p, normal(*h), -er));
        if i > 0 {
            let up = (i - 1) as f64 / (n - 1) as f64;
            if (up * waves as f64).floor() != (u * waves as f64).floor() && fade > 0.3 { nl.push(i); }
            if (up * waves as f64 + 0.5).floor() != (u * waves as f64 + 0.5).floor() && fade > 0.3 { nr.push(i); }
        }
    }
    let mut poly = left.clone();
    let (pe, he) = spine[n - 1];
    for k in 1..12 { let a = he + std::f64::consts::FRAC_PI_2 - std::f64::consts::PI * k as f64 / 12.0; poly.push(pt(pe.x + a.cos() * tip, pe.y + a.sin() * tip)); }
    poly.extend(right.iter().rev().copied());
    let outline = largest(&tidy(&poly)).unwrap_or(poly);
    // grooves: from each notch, sweeping forward to the midrib
    let reach = (n as f64 * 0.6 / waves as f64) as usize;
    let mut lines: Vec<Vec<Point>> = vec![spine[n / 12..n * 9 / 10].iter().map(|q| q.0).collect()];
    for (notches, edge, sgn) in [(&nl, &left, 1.0), (&nr, &right, -1.0)] {
        for &i in notches.iter() {
            let j = (i + reach).min(n - 1);
            lines.push((0..=8).map(|k| { let t = k as f64 / 8.0; let m = i + ((j - i) as f64 * t) as usize; let (p, h) = spine[m]; lerp_pt(edge[i], add(p, normal(h), sgn * env(m as f64 / (n - 1) as f64) * 0.12), t.powf(0.8) * 0.9) }).collect());
        }
    }
    Part { outline, lines, pocket: false, seam: vec![] }
}

/// A sunken hollow bounded by `outline`, drawn with a lip line inset by `lip`.
pub fn pocket(outline: Vec<Point>, lip: f64) -> Part {
    let inset = offset(&union(&[outline.as_slice()]), -lip);
    let lines = inset.into_iter().flatten().map(|mut c| { c.push(c[0]); c }).collect();
    Part { outline, lines, pocket: true, seam: vec![] }
}

/// A smooth closed blob through control points (Catmull-Rom).
pub fn blob(ctrl: &[Point]) -> Vec<Point> {
    let n = ctrl.len(); let mut out = vec![];
    for i in 0..n {
        let (p0, p1, p2, p3) = (ctrl[(i + n - 1) % n], ctrl[i], ctrl[(i + 1) % n], ctrl[(i + 2) % n]);
        for k in 0..16 { let t = k as f64 / 16.0; let (t2, t3) = (t * t, t * t * t);
            let f = |a: f64, b: f64, c: f64, d: f64| 0.5 * ((2.0 * b) + (-a + c) * t + (2.0 * a - 5.0 * b + 4.0 * c - d) * t2 + (-a + 3.0 * b - 3.0 * c + d) * t3);
            out.push(pt(f(p0.x, p1.x, p2.x, p3.x), f(p0.y, p1.y, p2.y, p3.y))); }
    }
    out
}

/// Mirror (in x, when `flip`) and rotate a set of parts about the origin.
pub fn turn(parts: &mut [Part], angle: f64, flip: bool) {
    let (c, s) = (angle.cos(), angle.sin());
    let f = |p: &mut Point| { let x = if flip { -p.x } else { p.x }; *p = pt(x * c - p.y * s, x * s + p.y * c); };
    for part in parts.iter_mut() { part.outline.iter_mut().for_each(f); for l in part.lines.iter_mut() { l.iter_mut().for_each(f); } }
}

/// Layered drawing of parts listed back to front: visible outlines, visible
/// inner lines, and the silhouette (union) for a filled preview.
pub struct Drawing { pub outlines: Runs, pub lines: Runs, pub pockets: Vec<Vec<Point>>, pub solid: Shapes, pub undersides: Vec<Vec<Point>> }
pub fn draw(parts: &[Part]) -> Drawing {
    let mut outlines = vec![]; let mut lines = vec![];
    for (i, p) in parts.iter().enumerate() {
        let covers: Vec<&[Point]> = parts[i + 1..].iter().filter(|q| !q.pocket).map(|q| q.outline.as_slice()).collect();
        let mut ring = p.outline.clone(); ring.push(ring[0]);
        let mut oc = covers.clone(); if p.seam.len() > 2 { oc.push(p.seam.as_slice()); }
        outlines.extend(visible_lines(&[ring], &oc, &[]));
        lines.extend(visible_lines(&p.lines, &covers, &[]));
    }
    let solids: Vec<&[Point]> = parts.iter().filter(|p| !p.pocket).map(|p| p.outline.as_slice()).collect();
    let pockets = parts.iter().filter(|p| p.pocket).map(|p| p.outline.clone()).collect();
    // turned-over flaps (parts seamed into a fold) show the leaf's underside
    let undersides = parts.iter().enumerate().filter(|(_, p)| p.seam.len() > 2).flat_map(|(i, p)| {
        let front: Vec<&[Point]> = parts[i + 1..].iter().filter(|q| !q.pocket).map(|q| q.outline.as_slice()).collect();
        crate::booleans::difference(&union(&[p.outline.as_slice()]), &union(&front)).into_iter().flatten()
    }).collect();
    Drawing { outlines, lines, pockets, solid: union(&solids), undersides }
}

// ---- Round 11: rococo traits (study, 2026-10-05) ----------------------------
// After the user's first custom pieces read "neat": the traits traditional
// rococo has that the library lacked. A varied rim (swells, pinches and turns
// over), the torn and pierced rocaille shell, drips, elements growing into one
// another (merged surfaces), a trellis field and the cabochon.

/// A small deterministic hash in 0..1, for irregular but repeatable shapes.
fn jitter(seed: u32, i: u32) -> f64 { let mut x = seed.wrapping_mul(747796405).wrapping_add(i.wrapping_mul(2891336453)); x ^= x >> 16; x = x.wrapping_mul(2246822519); x ^= x >> 13; (x % 10000) as f64 / 10000.0 }

/// A moulded rim whose width varies along it (`width(u)` multiplies the
/// half-width `wm`) and which turns over at each share in `twist`: there it
/// pinches to show its edge, and the moulding steps cross to the other side.
pub fn moulded_rim_varied(spine: &[(Point, f64)], ua: f64, ub: f64, wm: f64, outer: f64, width: impl Fn(f64) -> f64, twist: &[f64]) -> Part {
    let we = wm * 0.3;
    let pinch = |u: f64| twist.iter().map(|t| 1.0 - 0.6 * (-((u - t) / 0.03).powi(2)).exp()).fold(1.0, f64::min);
    let w = |u: f64| {
        let mut v = wm * width(u);
        if ua > 0.0 && u < ua { v = we + (v - we) * smooth(u / ua).powf(0.7); }
        if ub < 1.0 && u > ub { v = we + (v - we) * smooth((1.0 - u) / (1.0 - ub)).powf(0.7); }
        v * pinch(u)
    };
    // which face shows: flips smoothly through each twist
    let face = |u: f64| twist.iter().map(|t| ((u - t) / 0.025).tanh()).fold(1.0, |a, b| a * -b) * if twist.len() % 2 == 1 { -1.0 } else { 1.0 };
    let outline = band(spine, &w);
    let n = spine.len();
    let (a0, a1) = (ua * 0.35, 1.0 - (1.0 - ub) * 0.35);
    let step = |f: f64| -> Vec<Point> { spine.iter().enumerate().filter_map(|(i, (p, h))| { let u = i as f64 / (n - 1) as f64; (u >= a0 && u <= a1).then(|| add(*p, normal(*h), outer * face(u) * f * w(u))) }).collect() };
    Part { outline, lines: vec![step(0.10), step(-0.72)], pocket: false, seam: vec![] }
}

/// The rocaille shell proper: an irregular, lopsided fan whose flutes are
/// unevenly spaced, whose lip breaks into curling flame-tongues (`tongues`)
/// and which is pierced through near the lip (`holes`). `lean` (-1..1)
/// throws the reach to one side, like a wave breaking. Returns the shell
/// (one surface with its tongues) and the holes as pockets.
pub fn rocaille_shell(h: Point, axis: f64, span: f64, r: f64, lean: f64, tongues: usize, holes: usize, seed: u32) -> Vec<Part> {
    let ribs = 8usize;
    // uneven flute boundaries
    let mut ts: Vec<f64> = (0..=ribs).map(|i| { let t = i as f64 / ribs as f64; if i == 0 || i == ribs { t } else { t + (jitter(seed, i as u32) - 0.5) * 0.45 / ribs as f64 } }).collect();
    ts.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let reach = |t: f64| r * (0.55 + 0.45 * (std::f64::consts::PI * t).sin().powf(0.5)) * (1.0 + lean * (t - 0.5) * 1.1);
    let swirl = 0.55 * lean.signum().max(0.2);
    let rib = |t: f64, k: f64| { let a = axis - span / 2.0 + span * t + swirl * k * k * (t - 0.3); let d = reach(t) * k * (1.0 + 0.07 * (t * 11.0 + seed as f64).sin()); /* a smooth wobble: per-point noise made teeth */ pt(h.x + a.cos() * d, h.y + a.sin() * d) };
    let mut poly = vec![];
    for k in 0..=20 { poly.push(rib(0.0, 0.08 + 0.92 * k as f64 / 20.0)); }
    let mut lobe_mid = vec![];
    for i in 0..ribs {
        let (t0, t1) = (ts[i], ts[i + 1]);
        let (a, b) = (rib(t0, 1.0), rib(t1, 1.0));
        let (dx, dy) = (b.x - a.x, b.y - a.y); let len = dx.hypot(dy).max(1e-9);
        let nn = pt(dy / len, -dx / len);
        let m = pt((a.x + b.x) / 2.0 - h.x, (a.y + b.y) / 2.0 - h.y);
        let sign = if m.x * nn.x + m.y * nn.y >= 0.0 { 1.0 } else { -1.0 };
        let depth = 0.12 + 0.1 * jitter(seed, 100 + i as u32); // uneven but soft: no teeth
        for k in 1..=12 {
            let s = k as f64 / 12.0;
            let bulge = depth * len * (std::f64::consts::PI * s).sin() * (1.0 + 0.5 * (s - 0.5) * lean.signum());
            poly.push(add(rib(t0 + (t1 - t0) * s, 1.0), nn, sign * bulge));
        }
        lobe_mid.push(((t0 + t1) / 2.0, add(rib((t0 + t1) / 2.0, 1.0), nn, sign * depth * len * 0.9), (m.y).atan2(m.x)));
    }
    for k in (0..=20).rev() { poly.push(rib(1.0, 0.08 + 0.92 * k as f64 / 20.0)); }
    let fan = largest(&tidy(&poly)).unwrap_or(poly);
    let mut outs = vec![fan];
    let mut lines: Vec<Vec<Point>> = vec![];
    // flame tongues from the lobes on the leaning side, curling back
    let mut order: Vec<usize> = (0..ribs).collect();
    order.sort_by(|a, b| { let ka = if lean >= 0.0 { *b } else { *a }; let kb = if lean >= 0.0 { *a } else { *b }; ka.cmp(&kb) });
    for (j, &i) in order.iter().take(tongues).enumerate() {
        let (_, tip, out) = lobe_mid[i];
        let l = r * (0.5 - 0.1 * j as f64) * (0.85 + 0.3 * jitter(seed, 200 + i as u32));
        let side = if lean >= 0.0 { 1.0 } else { -1.0 } * if j % 2 == 0 { 1.0 } else { -1.0 };
        let start = add(tip, pt(-out.cos(), -out.sin()), r * 0.06);
        // a broad tongue with a blunt round end, rolling over gently (no claws)
        let spine = integrate(start, out, l, |s| { let v = s / l; side * (0.4 + 3.2 * smooth((v - 0.4) / 0.6)) / l });
        let w0 = r * 0.1;
        let w = move |v: f64| w0 * (1.0 - 0.35 * v) * (0.85 + 0.15 * (std::f64::consts::PI * v).sin());
        outs.push(band(&spine, &w));
        lines.push(rail(&spine, &w, 0.0, 0.08, 0.75));
    }
    let boss: Vec<Point> = (0..32).map(|k| { let a = std::f64::consts::TAU * k as f64 / 32.0; pt(h.x + a.cos() * r * 0.1, h.y + a.sin() * r * 0.1) }).collect();
    outs.push(boss);
    let refs: Vec<&[Point]> = outs.iter().map(|o| o.as_slice()).collect();
    let outline = largest(&union(&refs)).unwrap_or_else(|| outs[0].clone());
    for i in 1..ribs { let t = ts[i]; lines.push((0..=24).map(|k| rib(t, 0.14 + 0.86 * k as f64 / 24.0)).collect()); }
    lines.push((0..=24).map(|k| rib(k as f64 / 24.0, 0.14)).collect());
    // holes right through, between flutes near the lip
    let mut pockets = vec![];
    for j in 0..holes {
        let i = 1 + (j * 3 + seed as usize) % (ribs - 2);
        let t = (ts[i] + ts[i + 1]) / 2.0;
        let (c, d) = (rib(t, 0.74), rib(t, 0.9));
        let (dx, dy) = (d.x - c.x, d.y - c.y); let l = dx.hypot(dy).max(1e-9); let (ux, uy) = (dx / l, dy / l);
        let rw = (ts[i + 1] - ts[i]) * span * reach(t) * 0.22;
        let ctrl = [add(c, pt(0.0, 0.0), 0.0), pt(c.x + ux * l * 0.5 - uy * rw, c.y + uy * l * 0.5 + ux * rw), d, pt(c.x + ux * l * 0.5 + uy * rw, c.y + uy * l * 0.5 - ux * rw)];
        pockets.push(pocket(blob(&ctrl), (rw * 0.18).max(0.3)));
    }
    let holes_ref: Vec<&[Point]> = pockets.iter().map(|p| p.outline.as_slice()).collect();
    let lines = visible_lines(&lines, &holes_ref, &[]);
    let mut parts = vec![Part { outline, lines, pocket: false, seam: vec![] }];
    parts.extend(pockets);
    parts
}

/// Drips hanging from the line `edge`: `count` icicles, each a neck from the
/// edge swelling into a round drop, lengths `len` varied.
pub fn drips(edge: &[Point], count: usize, len: f64, width: f64, seed: u32) -> Vec<Part> {
    let n = edge.len();
    (0..count).map(|i| {
        let u = (i as f64 + 0.5) / count as f64;
        let p = edge[((n - 1) as f64 * u) as usize];
        let l = len * (0.6 + 0.6 * jitter(seed, i as u32));
        let lean = (jitter(seed, 40 + i as u32) - 0.5) * 0.3;
        let spine = integrate(p, std::f64::consts::FRAC_PI_2 + lean, l, |_| 0.0);
        let w = move |v: f64| { let neck = width * (1.0 - 0.65 * smooth(v / 0.7)); let drop = width * 0.75 * (1.0 - ((v - 0.85) / 0.17).powi(2)).max(0.0).sqrt(); neck.max(drop) };
        Part { outline: band(&spine, &w), lines: vec![rail(&spine, &w, 0.0, 0.1, 0.6)], pocket: false, seam: vec![] }
    }).collect()
}

/// Parts grown into one surface: one outline round them all (no seam where
/// one grows out of another); each part's inner lines are kept where no
/// later part lies over them.
pub fn merge(parts: &[Part]) -> Part {
    let mut lines = vec![];
    for (i, p) in parts.iter().enumerate() {
        let later: Vec<&[Point]> = parts[i + 1..].iter().map(|q| q.outline.as_slice()).collect();
        lines.extend(visible_lines(&p.lines, &later, &[]));
    }
    let refs: Vec<&[Point]> = parts.iter().map(|p| p.outline.as_slice()).collect();
    // the biggest piece (the union lists its shapes in no particular order)
    let outline = union(&refs).into_iter().map(|s| s[0].clone()).max_by(|a, b| crate::booleans::signed_area(a).abs().partial_cmp(&crate::booleans::signed_area(b).abs()).unwrap()).unwrap_or_else(|| parts[0].outline.clone());
    Part { outline, lines, pocket: false, seam: vec![] }
}

/// Parts grown into one surface, as `merge`, but keeping every piece: parts
/// that don't touch stay separate surfaces. A hole the union closes (a loop)
/// is drawn as a line round it.
pub fn merge_all(parts: &[Part]) -> Vec<Part> {
    if parts.is_empty() { return vec![]; }
    let mut lines = vec![];
    for (i, p) in parts.iter().enumerate() {
        let later: Vec<&[Point]> = parts[i + 1..].iter().map(|q| q.outline.as_slice()).collect();
        lines.extend(visible_lines(&p.lines, &later, &[]));
    }
    let refs: Vec<&[Point]> = parts.iter().map(|p| p.outline.as_slice()).collect();
    let shapes = union(&refs);
    let mut out: Vec<Part> = shapes.iter().map(|s| {
        let mut holes: Vec<Vec<Point>> = s[1..].iter().map(|h| { let mut h = h.clone(); h.push(h[0]); h }).collect();
        let lines = std::mem::take(&mut holes);
        Part { outline: s[0].clone(), lines, pocket: false, seam: vec![] }
    }).collect();
    if out.is_empty() { return parts.to_vec(); }
    // each line goes with the piece it lies in
    for l in lines {
        let m = l[l.len() / 2];
        let k = out.iter().position(|p| crate::outline::inside(m, &p.outline)).unwrap_or(0);
        out[k].lines.push(l);
    }
    out
}

/// A trellis (diaper) field: `field` filled with a diagonal lattice of
/// spacing `cell`, a small four-petal floret at each crossing. The field is
/// the first part (its lattice as lines), the florets follow.
pub fn trellis(field: &[Point], cell: f64) -> Vec<Part> {
    let inside = |p: Point| crate::outline::inside(p, field);
    let xs: Vec<f64> = field.iter().map(|p| p.x).collect(); let ys: Vec<f64> = field.iter().map(|p| p.y).collect();
    let (l, r, t, b) = (xs.iter().cloned().fold(f64::MAX, f64::min), xs.iter().cloned().fold(f64::MIN, f64::max), ys.iter().cloned().fold(f64::MAX, f64::min), ys.iter().cloned().fold(f64::MIN, f64::max));
    let mut lines = vec![];
    // lines x + y = c and x - y = c, sampled and kept where inside
    for dir in [1.0, -1.0] {
        let (c0, c1) = if dir > 0.0 { (l + t, r + b) } else { (l - b, r - t) };
        let mut c = (c0 / cell).floor() * cell;
        while c <= c1 {
            let mut run = vec![];
            let mut x = l;
            while x <= r {
                let p = pt(x, if dir > 0.0 { c - x } else { x - c });
                if inside(p) { run.push(p); } else if run.len() > 1 { lines.push(std::mem::take(&mut run)); } else { run.clear(); }
                x += 0.3;
            }
            if run.len() > 1 { lines.push(run); }
            c += cell;
        }
    }
    let mut parts = vec![Part { outline: field.to_vec(), lines, pocket: false, seam: vec![] }];
    // florets at the crossings well inside the field
    let k0 = ((l + t) / cell).floor() as i64; let k1 = ((r + b) / cell).ceil() as i64;
    let m0 = ((l - b) / cell).floor() as i64; let m1 = ((r - t) / cell).ceil() as i64;
    for k in k0..=k1 { for m in m0..=m1 {
        let (c1, c2) = (k as f64 * cell, m as f64 * cell);
        let p = pt((c1 + c2) / 2.0, (c1 - c2) / 2.0);
        let rr = cell * 0.2;
        if inside(p) && (0..8).all(|j| { let a = j as f64 * std::f64::consts::FRAC_PI_4; inside(pt(p.x + a.cos() * rr * 1.6, p.y + a.sin() * rr * 1.6)) }) {
            parts.extend(rosette(p, rr, 4, 0.0));
        }
    } }
    parts
}

/// A cabochon: a convex oval boss (half-axes `rx`, `ry`) with a bead line
/// round it and a highlight across its upper side.
pub fn cabochon(c: Point, rx: f64, ry: f64) -> Part {
    let oval = |k: f64, from: f64, to: f64, n: usize| -> Vec<Point> { (0..=n).map(|i| { let a = from + (to - from) * i as f64 / n as f64; pt(c.x + a.cos() * rx * k, c.y + a.sin() * ry * k) }).collect() };
    let mut outline = oval(1.0, 0.0, std::f64::consts::TAU, 96); outline.pop();
    let mut ring = oval(0.78, 0.0, std::f64::consts::TAU, 96); ring.pop(); ring.push(ring[0]);
    let highlight = (0..=24).map(|i| { let a = -2.6 + 1.5 * i as f64 / 24.0; pt(c.x - rx * 0.1 + a.cos() * rx * 0.5, c.y + ry * 0.12 + a.sin() * ry * 0.5) }).collect();
    Part { outline, lines: vec![ring, highlight], pocket: false, seam: vec![] }
}
