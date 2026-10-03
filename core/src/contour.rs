//! Acanthus contour: one enclosing outline around a spine — smooth outer
//! sweep, lobed inner edge, root flare, blunt ends — plus its midrib ridge
//! and one recessed crease per notch.
use crate::geometry::{distance, line_frame, line_length, pt, Point};
use crate::outline::inside;
use std::f64::consts::PI;

pub const GOLDEN_SMALL: f64 = 0.618_033_988_749_894_8; // (sqrt(5)-1)/2

#[derive(Clone, Copy, Debug)]
pub struct Notch { pub at: f64, pub depth: f64, pub before: f64, pub after: f64 }
/// Two notches at golden sections: broad belly, forward-leaning second lobe, soft tip.
pub fn leaf_notches() -> Vec<Notch> {
    vec![Notch { at: GOLDEN_SMALL, depth: 0.46, before: 0.030, after: 0.11 },
         Notch { at: 1.0 - GOLDEN_SMALL.powi(4), depth: 0.40, before: 0.024, after: 0.075 }]
}
/// 0 = plain tongue, 1 = one return, 2 = default two lobes, 3 = fuller crown.
pub fn notches_for(lobes: u8, depth: f64) -> Vec<Notch> {
    let base = match lobes {
        0 => vec![],
        1 => vec![Notch { at: 0.72, depth: 0.5, before: 0.03, after: 0.12 }],
        2 => leaf_notches(),
        _ => vec![Notch { at: 0.46, depth: 0.40, before: 0.03, after: 0.09 }, Notch { at: 0.66, depth: 0.44, before: 0.028, after: 0.08 }, Notch { at: 0.85, depth: 0.38, before: 0.022, after: 0.06 }],
    };
    base.into_iter().map(|n| Notch { depth: (n.depth * depth).min(0.85), ..n }).collect()
}
pub fn leaf_envelope(u: f64, stalk: f64, taper: f64) -> f64 {
    let v = (u - stalk).max(0.0) / (1.0 - stalk);
    let rise = (PI / 2.0 * (v / 0.34).min(1.0)).sin().powf(1.15);
    0.93 * rise * (1.0 - u).max(0.0).powf(taper)
}
pub fn lobe_profile(u: f64, lobed: bool, notches: &[Notch]) -> f64 {
    if !lobed { return 1.0; }
    let mut m = 1.0;
    for n in notches { let z = u - n.at; let s = if z < 0.0 { n.before } else { n.after }; m *= 1.0 - n.depth * (-(z / s).powi(2)).exp(); }
    m
}

#[derive(Clone, Copy, Debug)]
pub struct Ends { pub start: f64, pub tip: f64 }
pub const MAIN_ENDS: Ends = Ends { start: 1.3, tip: 1.0 };
pub const OPEN_ENDS: Ends = Ends { start: 1.1, tip: 0.8 };
/// Fraction of a shoot that stays a narrow stalk before its leaf opens.
pub fn shoot_stalk() -> f64 { (1.0 - GOLDEN_SMALL).powi(2) }

pub fn root_flare(parent: &[Point], root: Point, child_width: f64) -> f64 {
    let mut nearest = f64::INFINITY;
    for q in parent { nearest = nearest.min(distance(*q, root)); }
    0.5f64.max(nearest.min(child_width * 0.32).min(3.2))
}

/// `cuts` are eyes (closed loops) and slits, drawn as outline.
pub struct Anatomy { pub polygon: Vec<Point>, pub folds: Vec<Vec<Point>>, pub ridges: Vec<Vec<Point>>, pub cuts: Vec<Vec<Point>> }

/// All options of the contour; `new` gives the web version's defaults.
#[derive(Clone, Debug)]
pub struct ContourOptions {
    pub start: f64, pub belly: f64, pub lobed: bool, pub root_width: f64, pub stalk: f64,
    pub notches: Vec<Notch>, pub taper: f64, pub ends: Option<Ends>,
    /// Eyes at the notches: 0 none, 1 round, 2 slit and eye, 3 teardrop.
    pub eyes: u8,
}
impl Default for ContourOptions {
    fn default() -> Self { ContourOptions { start: 0.0, belly: 0.0, lobed: true, root_width: 0.0, stalk: 0.0, notches: leaf_notches(), taper: 0.42, ends: None, eyes: 0 } }
}

fn smooth(x: f64) -> f64 { let c = x.clamp(0.0, 1.0); c * c * (3.0 - 2.0 * c) }

pub fn acanthus_contour(spine: &[Point], stem_width: f64, side: f64, leaf_width: f64, o: &ContourOptions) -> Anatomy {
    let length = line_length(spine);
    let h = 0.02f64.min(0.002f64.max(leaf_width * 0.12 / length.max(1.0)));
    let frame = |t: f64| -> (Point, f64) {
        let (p, _) = line_frame(spine, t);
        let (a, _) = line_frame(spine, (t - h).max(0.0));
        let (b, _) = line_frame(spine, (t + h).min(1.0));
        (p, (b.y - a.y).atan2(b.x - a.x))
    };
    let map = |t: f64, w: f64| -> Point { let (p, ang) = frame(t); pt(p.x - ang.sin() * w * side, p.y + ang.cos() * w * side) };
    let samples: Vec<Point> = (0..=100).map(|j| frame(j as f64 / 100.0).0).collect();
    let concave_radius = |t: f64| -> f64 {
        let hh = 0.025; let a = frame((t - hh).max(0.0)).1; let b = frame((t + hh).min(1.0)).1;
        let ds = ((t + hh).min(1.0) - (t - hh).max(0.0)) * length;
        let k = (b - a).sin().atan2((b - a).cos()) / ds.max(1e-6);
        if k * side > 0.0 { 1.0 / k.abs() } else { f64::INFINITY }
    };
    let mut limits: Vec<f64> = (0..=160).map(|i| {
        let t = i as f64 / 160.0; let p = frame(t).0;
        let mut limit = leaf_width.min(0.2f64.max(concave_radius(t) * 0.72 - stem_width * 0.5));
        for j in 0..=100 { if (j as f64 / 100.0 - t).abs() * length < leaf_width * 2.0 { continue; } limit = limit.min(distance(p, samples[j]) * 0.40); }
        limit
    }).collect();
    for _ in 0..8 { let copy = limits.clone(); for i in 1..160 { limits[i] = (copy[i - 1] + copy[i] * 2.0 + copy[i + 1]) / 4.0; } }
    let limit = |t: f64| -> f64 { let n = 159f64.min((t * 160.0).floor()) as usize; let f = t * 160.0 - n as f64; limits[n] * (1.0 - f) + limits[n + 1] * f };
    let flare_span = if o.root_width > 0.0 { 0.3f64.min(o.root_width * 3.2 / length.max(1.0)) } else { 0.0 };
    let flare = |t: f64| -> f64 { if flare_span == 0.0 || t >= flare_span { return 0.0; } let s = 1.0 - t / flare_span; o.root_width * s * s * (3.0 - 2.0 * s) };
    let end_width = |t: f64| -> f64 { match o.ends { Some(e) => e.start * (1.0 - smooth(t / 0.45)) + e.tip * smooth((t - 0.72) / 0.28), None => 0.0 } };
    let stem = |t: f64| stem_width * 0.5 * (PI * t).sin().powf(0.7) + end_width(t);
    let outer: Vec<Point> = (0..=240).map(|i| { let t = i as f64 / 240.0; map(t, -(stem(t) + flare(t) * 0.55)) }).collect();
    let mass = |u: f64| o.belly * (PI * u).sin().powf(1.5);
    let body = |u: f64| if o.lobed { leaf_envelope(u, o.stalk, o.taper) + mass(u) } else { 0.65 * (PI * u).sin().powf(1.5) + mass(u) };
    let t_of = |u: f64| o.start + (1.0 - o.start) * u;
    let base = |t: f64| stem_width * 0.5 * (PI * t).sin() + flare(t) + end_width(t);
    let span = if o.start > 0.0 && o.ends.is_some() { (o.start * 0.7).min(0.4) } else { 0.0 };
    let swell = |t: f64| if span != 0.0 { 0.42 * leaf_width * smooth((t - (o.start - span)) / span) * (1.0 - smooth((t - o.start - span * 0.6) / (span * 0.8))) } else { 0.0 };
    let blend = |a: f64, b: f64| (a.powi(4) + b.powi(4)).powf(0.25);
    let half_width = |u: f64| { let t = t_of(u); base(t) + blend(body(u) * lobe_profile(u, o.lobed, &o.notches) * limit(t), swell(t).min(limit(t))) };
    let mut inner: Vec<Point> = (0..=280).map(|i| { let u = 1.0 - i as f64 / 280.0; map(t_of(u), half_width(u)) }).collect();
    if o.start > 0.0 { for i in 1..=60 { let t = o.start * (1.0 - i as f64 / 60.0); inner.push(map(t, base(t) + swell(t).min(limit(t)))); } }
    let cap = |a: Point, b: Point, t: f64, dir: f64| -> Vec<Point> {
        if o.ends.is_none() { return vec![]; }
        let ang = frame(t).1; let d = pt(ang.cos() * dir, ang.sin() * dir); let c = pt((a.x + b.x) / 2.0, (a.y + b.y) / 2.0); let r = distance(a, b) / 2.0;
        (0..11).map(|k| { let th = (k + 1) as f64 / 12.0 * PI; pt(c.x + (a.x - c.x) * th.cos() + d.x * r * th.sin(), c.y + (a.y - c.y) * th.cos() + d.y * r * th.sin()) }).collect()
    };
    let mut polygon = outer.clone();
    polygon.extend(cap(outer[240], inner[0], 1.0, 1.0));
    polygon.extend(inner.iter().copied());
    polygon.extend(cap(*inner.last().unwrap(), outer[0], 0.0, -1.0));
    if !o.lobed { return Anatomy { polygon, folds: vec![], ridges: vec![], cuts: vec![] }; }
    let clip = |line: Vec<Point>, minimum: usize| -> Vec<Vec<Point>> {
        let mut runs: Vec<Vec<Point>> = vec![]; let mut run: Vec<Point> = vec![];
        for p in line { if inside(p, &polygon) { run.push(p); } else { if !run.is_empty() { runs.push(std::mem::take(&mut run)); } } }
        if !run.is_empty() { runs.push(run); }
        let mut longest: Option<Vec<Point>> = None;
        for r in runs { if longest.as_ref().map_or(true, |l| r.len() > l.len()) { longest = Some(r); } }
        match longest { Some(l) if l.len() >= minimum => vec![l], _ => vec![] }
    };
    let across = |u: f64, f: f64| { let t = t_of(u); map(t, base(t) + f * body(u) * limit(t)) };
    let rib: Vec<Point> = (0..=120).map(|i| { let u = 0.05 + 0.87 * i as f64 / 120.0; across(u, 0.30 * (lobe_profile(u, true, &o.notches) + 0.25).min(1.0)) }).collect();
    let mut folds = vec![];
    for n in &o.notches {
        let line: Vec<Point> = (0..=80).map(|i| {
            let v = i as f64 / 80.0; let u = n.at - 0.010 - v * if n.at > 0.8 { 0.085 } else { 0.12 }; let notch_depth = 1.0 - n.depth;
            let f = notch_depth * 0.92 + (0.42 - notch_depth * 0.92) * v.powf(0.7);
            across(u, f)
        }).collect();
        folds.extend(clip(line, 24));
    }
    let ridges = clip(rib, 30);
    // eyes: where a notch bites in, a small eye is drilled at its inner end
    let mut cuts = vec![];
    if o.eyes > 0 {
        let ring = |c: Point, rx: f64, ry: f64, ang: f64| -> Vec<Point> { (0..=28).map(|k| { let a = k as f64 / 28.0 * 2.0 * PI; let (x, y) = (rx * a.cos(), ry * a.sin()); pt(c.x + x * ang.cos() - y * ang.sin(), c.y + x * ang.sin() + y * ang.cos()) }).collect() };
        for n in &o.notches {
            let t = t_of(n.at);
            let full = body(n.at) * limit(t); // the leaf's reach there, before the notch
            let edge = half_width(n.at);
            let r = (full * 0.2).clamp(0.6, 3.0);
            let floor = base(t) + r * 1.3; // keep the eye off the midrib
            let ang = frame(t).1;
            match o.eyes {
                1 => { let w = (edge - r * 1.4).max(floor); cuts.push(ring(map(t, w), r, r, ang)); }
                2 => {
                    // the notch runs on as a narrow slit, ending in the eye
                    let w = (edge - full * 0.42).max(floor);
                    let c = map(t_of((n.at + 0.008).min(1.0)), w);
                    let top = map(t_of((n.at + 0.008).min(1.0)), w + r * 0.8);
                    cuts.push(vec![map(t, edge), top]);
                    cuts.push(ring(c, r * 0.8, r * 0.8, ang));
                }
                _ => {
                    // a teardrop pointing out at the notch
                    let tip = map(t, edge - r * 0.3);
                    let w = (edge - r * 2.8).max(floor);
                    let c = map(t_of((n.at + 0.01).min(1.0)), w);
                    let d = (tip.y - c.y).atan2(tip.x - c.x); let rr = r * 1.05;
                    let mut drop = vec![tip];
                    for k in 0..=24 { let a = d + 0.45 * PI + k as f64 / 24.0 * 1.1 * PI; drop.push(pt(c.x + rr * a.cos(), c.y + rr * a.sin())); }
                    drop.push(tip);
                    cuts.push(drop);
                }
            }
        }
        // only eyes that sit wholly inside the leaf
        cuts.retain(|c| c.iter().all(|p| inside(*p, &polygon)) || c.len() == 2);
    }
    Anatomy { polygon, folds, ridges, cuts }
}
