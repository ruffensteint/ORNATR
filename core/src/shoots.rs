//! Hand-editable shoots: parameters that regrow a shoot on its backbone, and
//! measured library leaves rebuilt around their own spines.
use crate::contour::{acanthus_contour, notches_for, root_flare, shoot_stalk, ContourOptions, GOLDEN_SMALL as PHI};
use crate::geometry::{arc_table, distance, line_frame, line_length, pt, Curve, Point};
use crate::growth::{grow_curl, GrowthPart, GrowthResult, Kind};
use crate::profiles::{profile, LeafProfile};
use std::f64::consts::PI;

/// progress: arc fraction on the backbone; reach: fraction of backbone length;
/// turn: radians from the tangent; side: ±1.
#[derive(Clone, Debug, PartialEq)]
pub struct ShootParams {
    pub progress: f64, pub reach: f64, pub turn: f64, pub curl: f64, pub side: f64,
    pub leaf_side: Option<f64>, pub stem: Option<f64>, pub leaf_scale: Option<f64>, pub lobes: Option<u8>,
    pub depth: Option<f64>, pub stalk: Option<f64>, pub taper: Option<f64>, pub bend: Option<f64>, pub preset: Option<String>,
    /// Library leaves: how much the leaf bends with the stem it grows from (0 to 1).
    pub follow: Option<f64>,
    /// Leaf fan: this leaf plus smaller companions (66%, 33%) from the same node (2 or 3 leaves).
    pub fan: Option<u8>,
    /// Scroll-vine curls: the curl this one grows from (None: the vine's stem).
    pub on: Option<String>,
}
/// The preset id marking a scroll-vine curl.
pub const VINE_CURL: &str = "vine-curl";
impl Default for ShootParams {
    fn default() -> Self { ShootParams { progress: 0.5, reach: 0.12, turn: 0.0, curl: 0.66, side: 1.0, leaf_side: None, stem: None, leaf_scale: None, lobes: None, depth: None, stalk: None, taper: None, bend: None, preset: None, follow: None, fan: None, on: None } }
}
#[derive(Clone, Debug, PartialEq)]
pub struct ShootEdit { pub params: ShootParams, pub id: String, pub backbone: usize, pub replaces: Option<String>, pub hidden: bool, pub under: bool }

pub fn leaf_side_for(curl: f64, side: f64) -> f64 { if curl >= 0.5 { -side } else { side } }

pub fn grow_shoot(guide: &[Point], parent: &GrowthPart, e: &ShootParams, id: &str, leaves: bool) -> GrowthPart {
    if e.preset.as_deref().is_some_and(crate::bud::is_bud) { return crate::bud::grow_bud(guide, parent, e, id); }
    if let Some(p) = e.preset.as_deref().and_then(profile) { return grow_measured(guide, parent, e, id, p, leaves); }
    let guide_length = line_length(guide); let (fp, fa) = line_frame(guide, e.progress); let reach = e.reach * guide_length;
    let points = grow_curl(fp, fa + e.turn, reach, e.curl, e.side); let length = line_length(&points);
    let leaf_width = length * (1.0 - PHI) * (if e.curl >= 0.5 { PHI } else { 1.0 }) * e.leaf_scale.unwrap_or(1.0);
    let o = ContourOptions { lobed: leaves, root_width: root_flare(&parent.polygon, fp, length * (1.0 - PHI) * PHI), stalk: e.stalk.unwrap_or_else(shoot_stalk), notches: notches_for(e.lobes.unwrap_or(2), e.depth.unwrap_or(1.0)), taper: e.taper.unwrap_or(0.42), ..ContourOptions::default() };
    let a = acanthus_contour(&points, e.stem.unwrap_or(1.5), e.leaf_side.unwrap_or_else(|| leaf_side_for(e.curl, e.side)), leaf_width, &o);
    GrowthPart { id: id.into(), parent: Some(parent.id.clone()), kind: Kind::Secondary, points, polygon: a.polygon, folds: a.folds, ridges: Some(a.ridges), cuts: vec![], contour_split: Some(241), width: 3.0, length, birth: 0.35, duration: 0.25, shoot: Some(e.clone()), under: false }
}

/// The stem path a following leaf is bent along: from the stem point nearest
/// the root, walking the way the leaf heads, with arc length and a smoothed
/// curvature at each sample.
pub(crate) struct StemPath { pts: Vec<Point>, arc: Vec<f64>, ang: Vec<f64>, curv: Vec<f64> }
pub(crate) fn stem_path(stem: &[Point], from: Point, heading: f64) -> Option<StemPath> {
    if stem.len() < 3 { return None; }
    let i0 = (0..stem.len()).min_by(|&a, &b| distance(stem[a], from).partial_cmp(&distance(stem[b], from)).unwrap()).unwrap();
    let t = { let a = stem[i0.saturating_sub(1)]; let b = stem[(i0 + 1).min(stem.len() - 1)]; pt(b.x - a.x, b.y - a.y) };
    let forward = t.x * heading.cos() + t.y * heading.sin() >= 0.0;
    let mut pts: Vec<Point> = if forward { stem[i0..].to_vec() } else { stem[..=i0].iter().rev().copied().collect() };
    pts.dedup_by(|a, b| distance(*a, *b) < 1e-9);
    if pts.len() < 3 { return None; }
    let n = pts.len();
    let mut arc = vec![0.0]; for k in 1..n { arc.push(arc[k - 1] + distance(pts[k - 1], pts[k])); }
    let seg = |k: usize| (pts[k + 1].y - pts[k].y).atan2(pts[k + 1].x - pts[k].x);
    let mut ang = vec![seg(0)];
    for k in 1..n { let a = seg(k.min(n - 2)); let last = ang[k - 1]; let d = (a - last).sin().atan2((a - last).cos()); ang.push(last + d); }
    let w = 3;
    let curv = (0..n).map(|k| { let (a, b) = (k.saturating_sub(w), (k + w).min(n - 1)); let ds = arc[b] - arc[a]; if ds > 1e-9 { (ang[b] - ang[a]) / ds } else { 0.0 } }).collect();
    Some(StemPath { pts, arc, ang, curv })
}
impl StemPath {
    /// The same path with its turning scaled by `f` (0 straight, 1 as drawn).
    fn scaled(&self, f: f64) -> StemPath {
        let a0 = self.ang[0]; let ang: Vec<f64> = self.ang.iter().map(|a| a0 + (a - a0) * f).collect();
        let mut pts = vec![self.pts[0]];
        for k in 1..self.pts.len() { let d = self.arc[k] - self.arc[k - 1]; let a = ang[k - 1]; let p = pts[k - 1]; pts.push(pt(p.x + a.cos() * d, p.y + a.sin() * d)); }
        StemPath { pts, arc: self.arc.clone(), ang, curv: self.curv.iter().map(|c| c * f).collect() }
    }
    fn curvature(&self, u: f64) -> f64 { let i = self.arc.partition_point(|s| *s <= u); if i >= self.arc.len() { 0.0 } else { self.curv[i] } }
    /// Bend a point given in the root frame (u along the stem, v across it)
    /// onto the stem: u becomes distance along the stem, v stays the offset
    /// from it. (A safety ease keeps offsets short of a curl's centre.)
    pub(crate) fn bend(&self, u: f64, v: f64) -> Point { self.bend_with(u, v, true) }
    /// Curvature at arc distance `u` (0 past the end).
    pub(crate) fn curvature_at(&self, u: f64) -> f64 { self.curvature(u) }
    pub(crate) fn bend_with(&self, u: f64, v: f64, ease: bool) -> Point {
        let n = self.pts.len(); let total = self.arc[n - 1];
        let (p, a, k) = if u >= total {
            let a = self.ang[n - 1]; let e = self.pts[n - 1];
            (pt(e.x + a.cos() * (u - total), e.y + a.sin() * (u - total)), a, 0.0)
        } else {
            let i = self.arc.partition_point(|s| *s <= u).clamp(1, n - 1);
            let t = ((u - self.arc[i - 1]) / (self.arc[i] - self.arc[i - 1]).max(1e-12)).clamp(0.0, 1.0);
            let (p0, p1) = (self.pts[i - 1], self.pts[i]);
            (pt(p0.x + (p1.x - p0.x) * t, p0.y + (p1.y - p0.y) * t), self.ang[i - 1] + (self.ang[i] - self.ang[i - 1]) * t, self.curv[i - 1] + (self.curv[i] - self.curv[i - 1]) * t)
        };
        let v = if ease && v * k > 0.0 { let r = 0.95 / k.abs(); v.signum() * r * (v.abs() / r).tanh() } else { v };
        pt(p.x - a.sin() * v, p.y + a.cos() * v)
    }
}

fn grow_measured(guide: &[Point], parent: &GrowthPart, e: &ShootParams, id: &str, p: &LeafProfile, leaves: bool) -> GrowthPart {
    let guide_length = line_length(guide); let (fp, fa) = line_frame(guide, e.progress);
    let s_len = e.reach * guide_length; let m = e.side; let bend = e.bend.unwrap_or(0.0); let width = e.leaf_scale.unwrap_or(1.0);
    let n = p.spine.len(); let heading = fa + e.turn;
    let follow = e.follow.unwrap_or(0.0).clamp(0.0, 1.0);
    let mut local = vec![pt(0.0, 0.0)];
    for i in 1..n {
        let (a, b) = (p.spine[i - 1], p.spine[i]); let step = (b.0 - a.0).hypot(b.1 - a.1); let t = i as f64 / (n as f64 - 1.0);
        let angle = ((b.1 - a.1) * m).atan2(b.0 - a.0) + bend * m * t * t;
        let prev = local[i - 1]; local.push(pt(prev.x + angle.cos() * step, prev.y + angle.sin() * step));
    }
    let (c, s) = (heading.cos(), heading.sin());
    let spine: Vec<Point> = local.iter().map(|q| pt(fp.x + s_len * (q.x * c - q.y * s), fp.y + s_len * (q.x * s + q.y * c))).collect();
    let normal: Vec<Point> = (0..n).map(|i| { let a = spine[i.saturating_sub(2)]; let b = spine[(i + 2).min(n - 1)]; let l = { let d = (b.x - a.x).hypot(b.y - a.y); if d != 0.0 { d } else { 1.0 } }; pt(-(b.y - a.y) / l * m, (b.x - a.x) / l * m) }).collect();
    let at = |(i, t, d): (usize, f64, f64)| -> Point { let q = spine[i]; let nn = normal[i]; let tan = pt(nn.y * m, -nn.x * m); pt(q.x + (tan.x * t + nn.x * d * width) * s_len, q.y + (tan.y * t + nn.y * d * width) * s_len) };
    let polygon: Vec<Point> = p.outline.iter().map(|v| at(*v)).collect();
    let ridge = spine[(n as f64 * 0.04).round() as usize..(n as f64 * 0.93).round() as usize].to_vec();
    let folds = if leaves { p.folds.iter().map(|l| l.iter().map(|v| at(*v)).collect()).collect() } else { vec![] };
    let ridges = if leaves { let mut r = vec![ridge]; r.extend(p.ribs.iter().map(|l| l.iter().map(|v| at(*v)).collect::<Vec<_>>())); r } else { vec![] };
    // Follow the stem: the finished leaf is bent into the stem's own frame, so
    // it keeps its measured shape but runs along the stem, round the volute.
    let path = if follow > 0.0 { stem_path(&parent.points, fp, heading) } else { None };
    let (spine, polygon, folds, ridges) = match path {
        None => (spine, polygon, folds, ridges),
        Some(path) => {
            let a0 = path.ang[0]; let (tx, ty) = (a0.cos(), a0.sin());
            let uv = |q: &Point| { let (dx, dy) = (q.x - fp.x, q.y - fp.y); (dx * tx + dy * ty, -dx * ty + dy * tx) };
            // Follow only as far as the leaf fits: on the inside of a tight
            // curl a wide leaf would be crushed, so it bends less instead.
            let fits = polygon.iter().map(uv).filter(|(u, _)| *u > 0.0).map(|(u, v)| { let k = path.curvature(u); if v * k > 0.0 { 0.8 / (v * k) } else { f64::INFINITY } }).fold(f64::INFINITY, f64::min);
            let path = path.scaled(follow.min(fits));
            let wrap = |q: &Point| -> Point { let (u, v) = uv(q); if u <= 0.0 { *q } else { path.bend(u, v) } };
            let all = |l: &Vec<Point>| l.iter().map(wrap).collect::<Vec<_>>();
            (all(&spine), all(&polygon), folds.iter().map(all).collect(), ridges.iter().map(all).collect())
        }
    };
    GrowthPart { id: id.into(), parent: Some(parent.id.clone()), kind: Kind::Secondary, points: spine, polygon, folds, ridges: Some(ridges), cuts: vec![], contour_split: None, width: 3.0, length: s_len, birth: 0.35, duration: 0.25, shoot: Some(e.clone()), under: false }
}

/// The companions of a leaf fan: smaller copies of the lead leaf growing from
/// the same node, each swung further away from the stem (splaying outward)
/// and tucked behind the lead, so the fan reads 100 / 66 / 33.
pub fn fan_members(lead: &ShootParams) -> Vec<ShootParams> {
    let n = lead.fan.unwrap_or(1).clamp(1, 3) as usize;
    (1..n).map(|k| {
        let scale = [1.0, 0.66, 0.33][k];
        let swing = -FAN_SWING * k as f64 * lead.side;
        ShootParams { reach: lead.reach * scale, turn: lead.turn - swing, fan: None, ..lead.clone() }
    }).collect()
}
pub const FAN_SWING: f64 = 0.42;

/// Apply one backbone's edits: replaced shoots drop out, tucked leaves go
/// beneath the main sweep, everything else is drawn above it.
pub fn apply_shoot_edits(result: GrowthResult, guide: &[Point], edits: &[ShootEdit], leaves: bool) -> GrowthResult {
    if edits.is_empty() { return result; }
    let Some(main) = result.parts.iter().find(|p| p.parent.is_none()).cloned() else { return result; };
    let replaced: Vec<&str> = edits.iter().filter_map(|e| e.replaces.as_deref()).collect();
    let kept: Vec<GrowthPart> = result.parts.into_iter().filter(|p| !replaced.contains(&p.id.as_str())).collect();
    let grown: Vec<(bool, GrowthPart)> = edits.iter().filter(|e| !e.hidden).flat_map(|e| {
        let mut part = grow_shoot(guide, &main, &e.params, &e.id, leaves); part.under = e.under;
        // fan companions sit behind the lead (smallest furthest back) and are
        // edited through the lead leaf, so they carry no shoot of their own

        let members = fan_members(&e.params).into_iter().enumerate().map(|(k, p)| { let mut m = grow_shoot(guide, &main, &p, &format!("{}~fan{}", e.id, k + 1), leaves); m.shoot = None; m.under = e.under; (e.under, m) }).collect::<Vec<_>>();
        members.into_iter().rev().chain(std::iter::once((e.under, part))).collect::<Vec<_>>()
    }).collect();
    let mut parts: Vec<GrowthPart> = grown.iter().filter(|g| g.0).map(|g| g.1.clone()).collect();
    parts.extend(kept);
    parts.extend(grown.into_iter().filter(|g| !g.0).map(|g| g.1));
    GrowthResult { parts, ..result }
}

/// Arc-length fraction of the backbone sample nearest to p.
pub fn nearest_progress(curve: &Curve, p: Point) -> f64 {
    let table = arc_table(curve); let mut best = 0; let mut best_d = f64::INFINITY;
    for (i, r) in table.iter().enumerate() { let d = distance(r.point, p); if d < best_d { best_d = d; best = i; } }
    let pts: Vec<Point> = table.iter().map(|r| r.point).collect(); let total = line_length(&pts);
    let mut run = 0.0; for i in 1..=best { run += distance(pts[i], pts[i - 1]); }
    if total != 0.0 { run / total } else { 0.0 }
}
/// Moving a spine point from `handle` to `to` rescales and turns the shoot about its root.
pub fn drag_tip(e: &ShootParams, root: Point, handle: Point, to: Point) -> (f64, f64) {
    let before = distance(root, handle); let after = distance(root, to);
    if before < 1e-6 || after < 1e-6 { return (e.reach, e.turn); }
    let mut turn = e.turn + (to.y - root.y).atan2(to.x - root.x) - (handle.y - root.y).atan2(handle.x - root.x);
    while turn > PI { turn -= 2.0 * PI; } while turn < -PI { turn += 2.0 * PI; }
    ((e.reach * after / before).clamp(0.01, 1.5), turn)
}
pub fn tip_handle(part: &GrowthPart) -> Point { part.points[(part.points.len() as f64 * 0.4).floor() as usize] }
