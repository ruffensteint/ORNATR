//! Rococo designs, as edited in the app's Rococo workspace.
//!
//! A design is a list of elements drawn back to front, built from the
//! rocaille vocabulary (`rocaille.rs`): moulded **rims** through control
//! points with volutes at their ends, and the ornament set on them: fronds
//! (with turnovers), frond runs along a rim's outer roll, shells, rosettes,
//! crimped crests (frills) and pierced pockets. Ornament is pinned to a rim
//! (share along its main path plus an offset, its heading relative to the
//! rim's), so it follows when a rim is reshaped. The starting layouts are the
//! rococo reference library (see ORNAMENT_FAMILIES.md): loop cartouche,
//! apron agrafe, panel corner, oval medallion, running frieze, wave spray and
//! console bracket.
use crate::geometry::{pt, Bounds, Point};
use crate::outline::path_data;
use crate::rocaille::{self, Part, Turnover};
use std::collections::HashMap;
use std::f64::consts::PI;

/// Where an ornament sits: on a rim (share `u` of its main path, 0 at the
/// first control point and 1 at the last, beyond them into the volutes;
/// then `along` the rim's heading and `across` it, + = right of the heading
/// on screen), or at a fixed point on the page.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Anchor { Rim { rim: u32, u: f64, along: f64, across: f64 }, Free(Point) }

/// One element. Headings and axes are relative to the rim's heading at the
/// anchor (absolute for a free anchor). `side`/`outer`: +1 right of the
/// heading, -1 left.
#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    /// A moulded rim along a Bézier path through `nodes`; `width` is its half-width; `outer` the
    /// side the outer roll is on; a volute at each end (`hook`: +1 curls out,
    /// toward the outer roll's side, -1 in, 0 none; `eye`: its outer radius).
    /// `swell` (0..1): thin at the ends and full through the body; `twists`:
    /// main-path shares where the rim turns over (pinches to its edge, the
    /// moulding crossing to the other side).
    Rim { nodes: Vec<Node>, width: f64, outer: f64, hook0: f64, eye0: f64, hook1: f64, eye1: f64, swell: f64, twists: Vec<f64> },
    Frond { at: Anchor, heading: f64, length: f64, side: f64, bend: f64, fingers: usize, width: f64, splay: f64, turn: Turnover },
    Shell { at: Anchor, axis: f64, span: f64, ribs: usize, r: f64, asym: f64, twist: f64, scallop: f64 },
    Rosette { at: Anchor, turn: f64, r: f64, petals: usize },
    /// Fronds along a rim's outer roll from share `from` to `to`; the
    /// second-to-last turns over as `last`.
    Run { rim: u32, from: f64, to: f64, outer: f64, count: usize, length: f64, fingers: usize, last: Turnover },
    /// A crimped crest along a rim, `across` mm off its spine, growing to `side`.
    Frill { rim: u32, from: f64, to: f64, across: f64, side: f64, depth: f64, waves: usize, broken: f64 },
    /// A pierced hollow: a smooth closed outline through `ctrl`.
    Pocket { ctrl: Vec<Point>, lip: f64 },
    /// A convex oval boss (half-axes `rx`, `ry`, its long axis at `turn`).
    Cabochon { at: Anchor, turn: f64, rx: f64, ry: f64 },
    /// A trellis (diaper) field: a smooth closed outline through `ctrl` filled
    /// with a diagonal lattice of `cell` mm, with a floret at each crossing when `florets`.
    Trellis { ctrl: Vec<Point>, cell: f64, florets: bool },
}

/// `grown`: ornament that grows out of its rim, drawn as one surface with it
/// (no outline where they join). Fronds, shells, runs and crests only.
#[derive(Clone, Debug, PartialEq)]
pub struct Element { pub id: u32, pub kind: Kind, pub grown: bool }

impl Element {
    /// The rim this element hangs on, if any.
    pub fn rim(&self) -> Option<u32> {
        match &self.kind {
            Kind::Frond { at, .. } | Kind::Shell { at, .. } | Kind::Rosette { at, .. } | Kind::Cabochon { at, .. } => match at { Anchor::Rim { rim, .. } => Some(*rim), Anchor::Free(_) => None },
            Kind::Run { rim, .. } | Kind::Frill { rim, .. } => Some(*rim),
            _ => None,
        }
    }
    pub fn is_rim(&self) -> bool { matches!(self.kind, Kind::Rim { .. }) }
    /// Whether this kind can grow out of its rim (see `grown`).
    pub fn can_grow(&self) -> bool { matches!(self.kind, Kind::Frond { .. } | Kind::Shell { .. } | Kind::Run { .. } | Kind::Frill { .. }) }
    pub fn label(&self) -> &'static str {
        match self.kind { Kind::Rim { .. } => "Rim", Kind::Frond { .. } => "Frond", Kind::Shell { .. } => "Shell", Kind::Rosette { .. } => "Rosette", Kind::Run { .. } => "Frond run", Kind::Frill { .. } => "Crest", Kind::Pocket { .. } => "Pocket", Kind::Cabochon { .. } => "Cabochon", Kind::Trellis { .. } => "Trellis" }
    }
}

/// A rim's spine: samples (point, heading) about 0.4 mm apart, the shares
/// of it where the main path (through the control points) starts and ends,
/// and the rim's half-width.
#[derive(Clone, Debug)]
pub struct Spine { pub pts: Vec<(Point, f64)>, pub ua: f64, pub ub: f64, pub width: f64 }
impl Spine {
    fn ends(&self) -> (f64, f64) { let n = (self.pts.len() - 1) as f64; (self.ua * n, self.ub * n) }
    /// Sample index at main-path share `u` (beyond 0..1 into the volutes).
    pub fn index(&self, u: f64) -> usize {
        let (i0, i1) = self.ends();
        (i0 + u * (i1 - i0)).round().clamp(0.0, (self.pts.len() - 1) as f64) as usize
    }
    pub fn frame(&self, u: f64) -> (Point, f64) { self.pts[self.index(u)] }
    /// Main-path share of sample `i`.
    pub fn share_of(&self, i: usize) -> f64 { let (i0, i1) = self.ends(); if (i1 - i0).abs() < 1e-9 { 0.0 } else { (i as f64 - i0) / (i1 - i0) } }
    /// Share of the whole spine at main-path share `u` (what `rocaille` runs take).
    pub fn whole(&self, u: f64) -> f64 { self.index(u) as f64 / (self.pts.len() - 1) as f64 }
    /// The sample nearest `p`: its index and distance.
    pub fn nearest(&self, p: Point) -> (usize, f64) {
        let mut best = (0, f64::INFINITY);
        for (i, (q, _)) in self.pts.iter().enumerate() { let d = (q.x - p.x).hypot(q.y - p.y); if d < best.1 { best = (i, d); } }
        best
    }
    pub fn line(&self) -> Vec<Point> { self.pts.iter().map(|q| q.0).collect() }
}

fn normal(h: f64) -> Point { pt(-h.sin(), h.cos()) }
/// A point `along` heading `h` and `across` it from `p`.
fn off(p: Point, h: f64, along: f64, across: f64) -> Point { let n = normal(h); pt(p.x + h.cos() * along + n.x * across, p.y + h.sin() * along + n.y * across) }

pub fn rim_spine(nodes: &[Node], outer: f64, hook0: f64, eye0: f64, hook1: f64, eye1: f64, width: f64) -> Spine {
    // the start volute is traced back from the start, so the outer side is the other way round there
    let (pts, ua, ub) = rocaille::rim_on_sides(&bezier_dense(nodes), -hook0 * outer, eye0, hook1 * outer, eye1);
    Spine { pts, ua, ub, width }
}

// ---- rim paths: Bézier points with handles, as on a scroll backbone -------

/// A point on a rim and its two handles (absolute positions): `a` shapes the
/// segment coming in, `b` the one going out. They stay in line through the
/// point, so the rim runs smoothly through it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Node { pub p: Point, pub a: Point, pub b: Point }
impl Node {
    /// A point with its out-handle at `b` and the in-handle mirrored.
    pub fn smooth(p: Point, b: Point) -> Node { Node { p, a: pt(2.0 * p.x - b.x, 2.0 * p.y - b.y), b } }
}

/// How far (mm) a refitted rim may stray from the curve it replaces.
pub const FIT_TOLERANCE: f64 = 2.5;

fn lerp(a: Point, b: Point, t: f64) -> Point { pt(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t) }
fn cubic(p: [Point; 4], t: f64) -> Point {
    let m = 1.0 - t;
    let (b0, b1, b2, b3) = (m * m * m, 3.0 * m * m * t, 3.0 * m * t * t, t * t * t);
    pt(p[0].x * b0 + p[1].x * b1 + p[2].x * b2 + p[3].x * b3, p[0].y * b0 + p[1].y * b1 + p[2].y * b2 + p[3].y * b3)
}
fn dist(a: Point, b: Point) -> f64 { (a.x - b.x).hypot(a.y - b.y) }
/// Segment `i` of a rim path as its four Bézier points.
pub fn segment(nodes: &[Node], i: usize) -> [Point; 4] { [nodes[i].p, nodes[i].b, nodes[i + 1].a, nodes[i + 1].p] }

/// The rim path sampled densely (about every 0.25 mm).
pub fn bezier_dense(nodes: &[Node]) -> Vec<Point> {
    let mut out = vec![];
    for i in 0..nodes.len().saturating_sub(1) {
        let s = segment(nodes, i);
        let n = ((dist(s[0], s[1]) + dist(s[1], s[2]) + dist(s[2], s[3])) / 0.25).ceil().clamp(16.0, 2000.0) as usize;
        for k in 0..n { out.push(cubic(s, k as f64 / n as f64)); }
    }
    if let Some(l) = nodes.last() { out.push(l.p); }
    out
}

/// A smooth curve through `ctrl` (Catmull-Rom, as the studies drew rims), sampled densely.
pub fn catmull_dense(ctrl: &[Point]) -> Vec<Point> {
    let n = ctrl.len();
    let mut dense = vec![];
    for i in 0..n.saturating_sub(1) {
        let (p0, p1, p2, p3) = (ctrl[i.saturating_sub(1)], ctrl[i], ctrl[i + 1], ctrl[(i + 2).min(n - 1)]);
        for k in 0..40 { let t = k as f64 / 40.0; let (t2, t3) = (t * t, t * t * t);
            let f = |a: f64, b: f64, c: f64, d: f64| 0.5 * ((2.0 * b) + (-a + c) * t + (2.0 * a - 5.0 * b + 4.0 * c - d) * t2 + (-a + 3.0 * b - 3.0 * c + d) * t3);
            dense.push(pt(f(p0.x, p1.x, p2.x, p3.x), f(p0.y, p1.y, p2.y, p3.y))); }
    }
    if let Some(l) = ctrl.last() { dense.push(*l); }
    dense
}

/// One cubic from `q[0]` to its last point along the given end tangents
/// (unit; `t1` leaving the start, `t2` pointing back from the end), fitted
/// to `q` by least squares (Schneider's method) with two refinements of
/// the samples' parameters. Returns the two inner control points.
fn fit_cubic(q: &[Point], t1: Point, t2: Point) -> (Point, Point) {
    let n = q.len();
    let (p0, p3) = (q[0], q[n - 1]);
    let chord = dist(p0, p3).max(1e-9);
    let mut s = vec![0.0]; for i in 1..n { s.push(s[i - 1] + dist(q[i - 1], q[i])); }
    let total = s[n - 1].max(1e-9);
    let mut u: Vec<f64> = s.iter().map(|v| v / total).collect();
    let mut ctl = (lerp(p0, p3, 1.0 / 3.0), lerp(p0, p3, 2.0 / 3.0));
    for pass in 0..3 {
        let (mut c11, mut c12, mut c22, mut x1, mut x2) = (0.0, 0.0, 0.0, 0.0, 0.0);
        for (j, p) in q.iter().enumerate() {
            let t = u[j]; let m = 1.0 - t;
            let (b0, b1, b2, b3) = (m * m * m, 3.0 * m * m * t, 3.0 * m * t * t, t * t * t);
            let (a1, a2) = (pt(t1.x * b1, t1.y * b1), pt(t2.x * b2, t2.y * b2));
            c11 += a1.x * a1.x + a1.y * a1.y; c12 += a1.x * a2.x + a1.y * a2.y; c22 += a2.x * a2.x + a2.y * a2.y;
            let r = pt(p.x - (p0.x * (b0 + b1) + p3.x * (b2 + b3)), p.y - (p0.y * (b0 + b1) + p3.y * (b2 + b3)));
            x1 += a1.x * r.x + a1.y * r.y; x2 += a2.x * r.x + a2.y * r.y;
        }
        let det = c11 * c22 - c12 * c12;
        let (mut al1, mut al2) = if det.abs() > 1e-12 { ((x1 * c22 - x2 * c12) / det, (c11 * x2 - c12 * x1) / det) } else { (chord / 3.0, chord / 3.0) };
        if al1 < chord * 1e-3 || al2 < chord * 1e-3 { al1 = chord / 3.0; al2 = chord / 3.0; }
        ctl = (pt(p0.x + t1.x * al1, p0.y + t1.y * al1), pt(p3.x + t2.x * al2, p3.y + t2.y * al2));
        if pass == 2 { break; }
        // Newton step on each sample's parameter toward its nearest point on the curve
        let c = [p0, ctl.0, ctl.1, p3];
        for j in 1..n - 1 {
            let t = u[j]; let m = 1.0 - t;
            let b = cubic(c, t);
            let d1 = pt(3.0 * (m * m * (c[1].x - c[0].x) + 2.0 * m * t * (c[2].x - c[1].x) + t * t * (c[3].x - c[2].x)), 3.0 * (m * m * (c[1].y - c[0].y) + 2.0 * m * t * (c[2].y - c[1].y) + t * t * (c[3].y - c[2].y)));
            let d2 = pt(6.0 * (m * (c[2].x - 2.0 * c[1].x + c[0].x) + t * (c[3].x - 2.0 * c[2].x + c[1].x)), 6.0 * (m * (c[2].y - 2.0 * c[1].y + c[0].y) + t * (c[3].y - 2.0 * c[2].y + c[1].y)));
            let (dx, dy) = (b.x - q[j].x, b.y - q[j].y);
            let num = dx * d1.x + dy * d1.y; let den = d1.x * d1.x + d1.y * d1.y + dx * d2.x + dy * d2.y;
            if den.abs() > 1e-12 { u[j] = (t - num / den).clamp(0.0, 1.0); }
        }
    }
    ctl
}
fn unit(v: Point) -> Point { let l = v.x.hypot(v.y).max(1e-12); pt(v.x / l, v.y / l) }
/// The direction of a dense polyline at sample `i` (a few samples either side).
fn tangent(d: &[Point], i: usize) -> Point { let (a, b) = (d[i.saturating_sub(3)], d[(i + 3).min(d.len() - 1)]); unit(pt(b.x - a.x, b.y - a.y)) }

/// Few Bézier points (at least 3) whose path stays within `tol` mm of the
/// dense curve `d`: one cubic per span, split where it strays furthest, so
/// points land where the curve needs them.
pub fn fit_nodes(d: &[Point], tol: f64) -> Vec<Node> {
    if d.len() < 7 { let (a, b) = (d[0], d[d.len() - 1]); return vec![Node::smooth(a, lerp(a, b, 1.0 / 3.0)), Node::smooth(b, lerp(b, a, -1.0 / 3.0))]; }
    // split points (sample indices), refined until every span fits
    let mut idx = vec![0, d.len() - 1];
    let mut fits: Vec<Option<(Point, Point)>> = vec![None];
    loop {
        let mut worst: Option<(usize, usize, f64)> = None; // (span, split index, error)
        for k in 0..idx.len() - 1 {
            let (i, j) = (idx[k], idx[k + 1]);
            let (t1, t2) = (tangent(d, i), tangent(d, j));
            let c = fit_cubic(&d[i..=j], t1, pt(-t2.x, -t2.y));
            fits[k] = Some(c);
            let curve: Vec<Point> = (0..=200).map(|n| cubic([d[i], c.0, c.1, d[j]], n as f64 / 200.0)).collect();
            let (mut far, mut at) = (0.0, (i + j) / 2);
            for m in (i + 1..j).step_by(2) {
                let e = curve.iter().map(|q| dist(d[m], *q)).fold(f64::INFINITY, f64::min);
                if e > far { far = e; at = m; }
            }
            if far > tol && j - i > 12 && worst.is_none_or(|w| far > w.2) { worst = Some((k, at.clamp(i + 6, j - 6), far)); }
        }
        // at least three points: a single span splits at its middle
        let split = match worst { Some((k, at, _)) => Some((k, at)), None if idx.len() == 2 => Some((0, d.len() / 2)), None => None };
        let Some((k, at)) = split else { break };
        if idx.len() > 24 { break; }
        idx.insert(k + 1, at); fits.insert(k + 1, None);
    }
    // then drop any inner point whose two spans still fit as one
    let span_fit = |i: usize, j: usize| -> ((Point, Point), f64) {
        let (t1, t2) = (tangent(d, i), tangent(d, j));
        let c = fit_cubic(&d[i..=j], t1, pt(-t2.x, -t2.y));
        let curve: Vec<Point> = (0..=300).map(|n| cubic([d[i], c.0, c.1, d[j]], n as f64 / 300.0)).collect();
        let far = (i + 1..j).step_by(2).map(|m| curve.iter().map(|q| dist(d[m], *q)).fold(f64::INFINITY, f64::min)).fold(0.0, f64::max);
        (c, far)
    };
    let mut k = 1;
    while idx.len() > 3 && k < idx.len() - 1 {
        let (c, far) = span_fit(idx[k - 1], idx[k + 1]);
        if far <= tol { idx.remove(k); fits.remove(k); fits[k - 1] = Some(c); } else { k += 1; }
    }
    let mut nodes: Vec<Node> = idx.iter().map(|&i| Node { p: d[i], a: d[i], b: d[i] }).collect();
    for (k, c) in fits.iter().enumerate() { let (c1, c2) = c.unwrap(); nodes[k].b = c1; nodes[k + 1].a = c2; }
    // the free handles at the ends mirror their partners
    let n = nodes.len();
    let (f, l) = (nodes[0], nodes[n - 1]);
    nodes[0].a = pt(2.0 * f.p.x - f.b.x, 2.0 * f.p.y - f.b.y);
    nodes[n - 1].b = pt(2.0 * l.p.x - l.a.x, 2.0 * l.p.y - l.a.y);
    nodes
}

/// A rim drawn through control points as the studies did (Catmull-Rom, volute
/// signs relative to the curve's turn at each end), as Bézier points with
/// handles and volutes that curl out (+1) or in (-1).
pub fn rim_from_ctrl(ctrl: &[Point], outer: f64, hook0: f64, hook1: f64) -> (Vec<Node>, f64, f64) {
    let dense = catmull_dense(ctrl);
    let (s0, s1) = rocaille::rim_end_turns(&dense);
    (fit_nodes(&dense, FIT_TOLERANCE), hook0 * s0 * outer, hook1 * s1 * outer)
}

/// Add a point to a rim path where it passes nearest `p`, without changing
/// its shape (the segment is split in two). Returns the new point's index.
pub fn split_at(nodes: &mut Vec<Node>, p: Point) -> Option<usize> {
    if nodes.len() < 2 { return None; }
    let mut best = (0, 0.5, f64::INFINITY);
    for i in 0..nodes.len() - 1 {
        let s = segment(nodes, i);
        for k in 1..200 { let t = k as f64 / 200.0; let d = dist(cubic(s, t), p); if d < best.2 { best = (i, t, d); } }
    }
    let (i, t, _) = best;
    let s = segment(nodes, i);
    let (q0, q1, q2) = (lerp(s[0], s[1], t), lerp(s[1], s[2], t), lerp(s[2], s[3], t));
    let (r0, r1) = (lerp(q0, q1, t), lerp(q1, q2, t));
    nodes[i].b = q0; nodes[i + 1].a = q2;
    nodes.insert(i + 1, Node { p: lerp(r0, r1, t), a: r0, b: r1 });
    Some(i + 1)
}

/// Remove point `k` from a rim path (it keeps at least two). An inner point's
/// two segments are refitted as one, keeping the neighbours' directions.
pub fn remove_node(nodes: &mut Vec<Node>, k: usize) -> bool {
    let n = nodes.len();
    if n <= 2 || k >= n { return false; }
    if k == 0 || k == n - 1 { nodes.remove(k); return true; }
    let mut q = vec![];
    for i in [k - 1, k] { let s = segment(nodes, i); for j in 0..100 { q.push(cubic(s, j as f64 / 100.0)); } }
    q.push(nodes[k + 1].p);
    let (pa, pb) = (nodes[k - 1], nodes[k + 1]);
    let t1 = if dist(pa.b, pa.p) > 1e-9 { unit(pt(pa.b.x - pa.p.x, pa.b.y - pa.p.y)) } else { tangent(&q, 0) };
    let t2 = if dist(pb.a, pb.p) > 1e-9 { unit(pt(pb.a.x - pb.p.x, pb.a.y - pb.p.y)) } else { let t = tangent(&q, q.len() - 1); pt(-t.x, -t.y) };
    let (c1, c2) = fit_cubic(&q, t1, t2);
    nodes[k - 1].b = c1; nodes[k + 1].a = c2;
    nodes.remove(k);
    true
}
/// Move handle `out` (else the in-handle) of point `k` to `to`; its partner
/// turns to stay in line, keeping its own length.
pub fn set_handle(nodes: &mut [Node], k: usize, out: bool, to: Point) {
    let Some(n) = nodes.get_mut(k) else { return };
    let other = if out { n.a } else { n.b };
    let len = dist(other, n.p);
    let d = unit(pt(n.p.x - to.x, n.p.y - to.y));
    let mirrored = if dist(to, n.p) > 1e-9 { pt(n.p.x + d.x * len, n.p.y + d.y * len) } else { other };
    if out { n.b = to; n.a = mirrored; } else { n.a = to; n.b = mirrored; }
}

/// A rococo design on a page (mm).
#[derive(Clone, Debug, PartialEq)]
pub struct Design { pub width: f64, pub height: f64, pub elements: Vec<Element> }

impl Design {
    pub fn empty(width: f64, height: f64) -> Design { Design { width, height, elements: vec![] } }
    /// A library structure fitted to a `width` × `height` page.
    pub fn from_structure(id: &str, width: f64, height: f64) -> Option<Design> {
        let mut d = structure(id)?;
        d.width = width; d.height = height;
        d.fit();
        Some(d)
    }
    pub fn next_id(&self) -> u32 { self.elements.iter().map(|e| e.id + 1).max().unwrap_or(1) }
    pub fn get(&self, id: u32) -> Option<&Element> { self.elements.iter().find(|e| e.id == id) }
    pub fn get_mut(&mut self, id: u32) -> Option<&mut Element> { self.elements.iter_mut().find(|e| e.id == id) }
    pub fn index(&self, id: u32) -> Option<usize> { self.elements.iter().position(|e| e.id == id) }
    pub fn push(&mut self, kind: Kind) -> u32 { let id = self.next_id(); self.elements.push(Element { id, kind, grown: false }); id }
    /// Add ornament that grows out of its rim (when its kind can).
    pub fn push_grown(&mut self, kind: Kind) -> u32 { let id = self.push(kind); if let Some(e) = self.get_mut(id) { e.grown = e.can_grow(); } id }

    /// Every rim's spine, by element id.
    pub fn spines(&self) -> HashMap<u32, Spine> {
        self.elements.iter().filter_map(|e| match &e.kind {
            Kind::Rim { nodes, width, outer, hook0, eye0, hook1, eye1, .. } if nodes.len() >= 2 => Some((e.id, rim_spine(nodes, *outer, *hook0, *eye0, *hook1, *eye1, *width))),
            _ => None,
        }).collect()
    }
    /// Where an anchor is, and the heading its ornament's angles are relative to.
    pub fn place(spines: &HashMap<u32, Spine>, at: &Anchor) -> Option<(Point, f64)> {
        match *at {
            Anchor::Free(p) => Some((p, 0.0)),
            Anchor::Rim { rim, u, along, across } => { let (p, h) = spines.get(&rim)?.frame(u); Some((off(p, h, along, across), h)) }
        }
    }
    /// An anchor on `rim` exactly at `p` (its nearest spine sample plus the offset).
    pub fn pin_to(spines: &HashMap<u32, Spine>, rim: u32, p: Point) -> Option<(Anchor, f64)> {
        let sp = spines.get(&rim)?;
        let (i, _) = sp.nearest(p);
        let (q, h) = sp.pts[i]; let n = normal(h);
        let (dx, dy) = (p.x - q.x, p.y - q.y);
        Some((Anchor::Rim { rim, u: sp.share_of(i), along: dx * h.cos() + dy * h.sin(), across: dx * n.x + dy * n.y }, h))
    }
    /// An anchor at `p` on the nearest rim within `reach` mm, else free.
    pub fn pin(spines: &HashMap<u32, Spine>, p: Point, reach: f64) -> (Anchor, f64) {
        let best = spines.iter().map(|(id, s)| (*id, s.nearest(p).1)).filter(|(_, d)| *d <= reach).min_by(|a, b| a.1.partial_cmp(&b.1).unwrap().then(a.0.cmp(&b.0)));
        match best { Some((rim, _)) => Self::pin_to(spines, rim, p).unwrap(), None => (Anchor::Free(p), 0.0) }
    }

    /// Each element's parts, back to front (elements whose rim is missing are skipped).
    pub fn parts(&self) -> Vec<(u32, Vec<Part>)> { let s = self.spines(); self.parts_with(&s) }
    pub fn parts_with(&self, spines: &HashMap<u32, Spine>) -> Vec<(u32, Vec<Part>)> {
        self.elements.iter().filter_map(|e| element_parts(e, spines).map(|p| (e.id, p))).collect()
    }
    /// The layered drawing (visible outlines and lines, silhouette, pockets).
    pub fn drawing(&self) -> rocaille::Drawing { rocaille::draw(&self.layered(&self.parts())) }
    /// The parts to draw, back to front. A rim and the ornament grown out of
    /// it become one surface, drawn where the last of them stood; their
    /// turned-over flaps stay separate, on top, so they still read.
    pub fn layered(&self, parts: &[(u32, Vec<Part>)]) -> Vec<Part> {
        let by_id: HashMap<u32, &Vec<Part>> = parts.iter().map(|(id, p)| (*id, p)).collect();
        // each rim's group: the rim and what grows from it
        let mut group_of: HashMap<u32, u32> = HashMap::new();
        for e in &self.elements { if e.grown && e.can_grow() { if let Some(r) = e.rim().filter(|r| by_id.contains_key(r)) { group_of.insert(e.id, r); group_of.insert(r, r); } } }
        let last_of: HashMap<u32, u32> = { let mut m = HashMap::new(); for (id, _) in parts { if let Some(g) = group_of.get(id) { m.insert(*g, *id); } } m };
        let mut out = vec![];
        for (id, ps) in parts {
            let Some(g) = group_of.get(id) else { out.extend(ps.iter().cloned()); continue };
            if last_of.get(g) != Some(id) { continue; }
            // the whole group, in drawing order: bodies merge, flaps go on top
            let (mut bodies, mut flaps) = (vec![], vec![]);
            for (mid, mps) in parts { if group_of.get(mid) == Some(g) { for p in mps.iter() { if p.seam.len() > 2 { flaps.push(p.clone()); } else { bodies.push(p.clone()); } } } }
            out.extend(rocaille::merge_all(&bodies));
            out.extend(flaps);
        }
        out
    }
    /// The pattern SVG at actual size: visible outlines and inner lines.
    pub fn svg(&self) -> String {
        let d = self.drawing();
        format!("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}mm\" height=\"{h}mm\" viewBox=\"0 0 {w} {h}\"><title>ORNATR rococo pattern</title><g fill=\"none\" stroke=\"#000\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><path d=\"{}\" stroke-width=\".35\"/><path d=\"{}\" stroke-width=\".2\"/></g></svg>", path_data(&d.outlines, false), path_data(&d.lines, false), w = self.width, h = self.height)
    }
    /// The bounds of everything drawn.
    pub fn bounds(&self) -> Option<Bounds> {
        let pts: Vec<Point> = self.parts().into_iter().flat_map(|(_, ps)| ps.into_iter().flat_map(|p| p.outline)).collect();
        (!pts.is_empty()).then(|| Bounds::of(&pts))
    }

    /// Scale by `k`, mirror in x when `flip`, rotate by `rot` (about the
    /// origin, after the mirror) and move by `shift`. Shapes stay the same:
    /// lengths scale, sides swap under a mirror.
    pub fn map(&mut self, k: f64, flip: bool, rot: f64, shift: Point) {
        let (c, s) = (rot.cos(), rot.sin());
        let f = if flip { -1.0 } else { 1.0 };
        let mp = |p: Point| { let x = p.x * f; pt((x * c - p.y * s) * k + shift.x, (x * s + p.y * c) * k + shift.y) };
        // an angle: absolute when free, relative to its rim otherwise
        let ma = |a: f64, free: bool| if free { (if flip { PI - a } else { a }) + rot } else { a * f };
        let man = |at: &mut Anchor| -> bool {
            match at {
                Anchor::Free(p) => { *p = mp(*p); true }
                Anchor::Rim { along, across, .. } => { *along *= k; *across *= k * f; false }
            }
        };
        for e in self.elements.iter_mut() {
            match &mut e.kind {
                Kind::Rim { nodes, width, outer, eye0, eye1, .. } => { for n in nodes.iter_mut() { n.p = mp(n.p); n.a = mp(n.a); n.b = mp(n.b); } *width *= k; *outer *= f; *eye0 *= k; *eye1 *= k; }
                Kind::Frond { at, heading, length, side, width, .. } => { let free = man(at); *heading = ma(*heading, free); *length *= k; *width *= k; *side *= f; }
                Kind::Shell { at, axis, r, asym, twist, .. } => { let free = man(at); *axis = ma(*axis, free); *r *= k; *asym *= f; *twist *= f; }
                Kind::Rosette { at, turn, r, .. } => { let free = man(at); *turn = ma(*turn, free); *r *= k; }
                Kind::Run { outer, length, .. } => { *outer *= f; *length *= k; }
                Kind::Frill { across, side, depth, .. } => { *across *= k * f; *side *= f; *depth *= k; }
                Kind::Pocket { ctrl, lip } => { for p in ctrl.iter_mut() { *p = mp(*p); } *lip *= k; }
                Kind::Cabochon { at, turn, rx, ry } => { let free = man(at); *turn = ma(*turn, free); *rx *= k; *ry *= k; }
                Kind::Trellis { ctrl, cell, .. } => { for p in ctrl.iter_mut() { *p = mp(*p); } *cell *= k; }
            }
        }
    }
    /// Add another design's elements on top, with fresh ids.
    pub fn append(&mut self, other: Design) {
        let base = self.next_id();
        let remap = |id: u32| id + base;
        for mut e in other.elements {
            e.id = remap(e.id);
            match &mut e.kind {
                Kind::Frond { at, .. } | Kind::Shell { at, .. } | Kind::Rosette { at, .. } | Kind::Cabochon { at, .. } => if let Anchor::Rim { rim, .. } = at { *rim = remap(*rim); },
                Kind::Run { rim, .. } | Kind::Frill { rim, .. } => *rim = remap(*rim),
                _ => {}
            }
            self.elements.push(e);
        }
    }
    /// Scale and centre the design on its page, leaving a margin.
    pub fn fit(&mut self) {
        // a few sizes are fixed in mm (sampling, the smallest tips), so the
        // drawing doesn't scale exactly: measure again and correct
        for _ in 0..3 {
            let Some(b) = self.bounds() else { return };
            let m = self.width.min(self.height) * 0.06;
            let k = ((self.width - 2.0 * m) / (b.r - b.l).max(1e-6)).min((self.height - 2.0 * m) / (b.b - b.t).max(1e-6));
            let c = b.center();
            if (k - 1.0).abs() < 2e-3 && (c.x - self.width / 2.0).abs() < 0.2 && (c.y - self.height / 2.0).abs() < 0.2 { return; }
            self.map(k, false, 0.0, pt(-c.x * k, -c.y * k));
            self.map(1.0, false, 0.0, pt(self.width / 2.0, self.height / 2.0));
        }
    }
    /// Remove an element; a rim takes the ornament hung on it with it.
    /// Returns how many elements went.
    pub fn remove(&mut self, id: u32) -> usize {
        let before = self.elements.len();
        let rim = self.get(id).is_some_and(|e| e.is_rim());
        self.elements.retain(|e| e.id != id && !(rim && e.rim() == Some(id)));
        before - self.elements.len()
    }
    /// Draw an element in front of (or behind) everything else.
    pub fn restack(&mut self, id: u32, front: bool) {
        let Some(i) = self.index(id) else { return };
        let e = self.elements.remove(i);
        if front { self.elements.push(e); } else { self.elements.insert(0, e); }
    }
    /// A typical rim half-width for new elements.
    pub fn rim_width(&self) -> f64 {
        let ws: Vec<f64> = self.elements.iter().filter_map(|e| if let Kind::Rim { width, .. } = e.kind { Some(width) } else { None }).collect();
        if ws.is_empty() { (self.width.min(self.height) * 0.03).clamp(3.0, 10.0) } else { ws.iter().sum::<f64>() / ws.len() as f64 }
    }

    // ---- adding elements ------------------------------------------------
    /// A new rim: a gentle arc centred on `c`, `len` mm long, volute at its end.
    pub fn add_rim(&mut self, c: Point, len: f64) -> u32 {
        let w = self.rim_width();
        // three points, a gentle arc: handles level at the crown, rising at the ends
        let (l, h) = (len / 2.0, len * 0.18);
        let nodes = vec![
            Node::smooth(pt(c.x - l, c.y), pt(c.x - l + len * 0.12, c.y - h * 0.8)),
            Node::smooth(pt(c.x, c.y - h), pt(c.x + len * 0.18, c.y - h)),
            Node::smooth(pt(c.x + l, c.y), pt(c.x + l + len * 0.12, c.y + h * 0.8)),
        ];
        self.push(Kind::Rim { nodes, width: w, outer: -1.0, hook0: 0.0, eye0: (w * 1.6).max(6.0), hook1: 1.0, eye1: (w * 1.6).max(6.0), swell: 0.0, twists: vec![] })
    }
    fn rim_kind(&self, rim: u32) -> Option<(f64, f64)> { match self.get(rim)?.kind { Kind::Rim { width, outer, .. } => Some((width, outer)), _ => None } }
    /// A frond on `rim` at share `u`, reaching out on the rim's outer side.
    pub fn add_frond(&mut self, rim: u32, u: f64) -> Option<u32> {
        let (w, outer) = self.rim_kind(rim)?;
        let l = (w * 7.0).clamp(25.0, 90.0);
        Some(self.push_grown(Kind::Frond { at: Anchor::Rim { rim, u, along: 0.0, across: outer * w * 0.6 }, heading: outer * 0.9, length: l, side: -outer, bend: 0.7, fingers: 4, width: l * 0.26, splay: 1.05, turn: Turnover::None }))
    }
    pub fn add_shell(&mut self, rim: u32, u: f64) -> Option<u32> {
        let (w, outer) = self.rim_kind(rim)?;
        Some(self.push_grown(Kind::Shell { at: Anchor::Rim { rim, u, along: 0.0, across: outer * w * 0.3 }, axis: outer * PI * 0.5, span: PI * 0.95, ribs: 11, r: (w * 6.0).clamp(15.0, 80.0), asym: 0.3, twist: 0.3, scallop: 0.13 }))
    }
    pub fn add_rosette(&mut self, rim: u32, u: f64) -> Option<u32> {
        let (w, outer) = self.rim_kind(rim)?;
        Some(self.push(Kind::Rosette { at: Anchor::Rim { rim, u, along: 0.0, across: -outer * w * 0.25 }, turn: 0.0, r: (w * 0.95).max(4.0), petals: 5 }))
    }
    pub fn add_run(&mut self, rim: u32, from: f64, to: f64) -> Option<u32> {
        let (w, outer) = self.rim_kind(rim)?;
        Some(self.push_grown(Kind::Run { rim, from, to, outer, count: 3, length: (w * 6.5).clamp(20.0, 70.0), fingers: 3, last: Turnover::None }))
    }
    pub fn add_frill(&mut self, rim: u32, from: f64, to: f64) -> Option<u32> {
        let (w, outer) = self.rim_kind(rim)?;
        Some(self.push_grown(Kind::Frill { rim, from, to, across: outer * w * 0.85, side: outer, depth: w * 1.7, waves: 6, broken: 0.5 }))
    }
    pub fn add_pocket(&mut self, c: Point, r: f64) -> u32 {
        let ctrl = (0..8).map(|i| { let a = PI * 2.0 * i as f64 / 8.0; pt(c.x + a.cos() * r * 0.8, c.y + a.sin() * r) }).collect();
        let id = self.push(Kind::Pocket { ctrl, lip: (r * 0.06).clamp(1.0, 2.5) });
        self.restack(id, false);
        id
    }
    /// A cabochon on `rim` at share `u`, sitting on its outer roll.
    pub fn add_cabochon(&mut self, rim: u32, u: f64) -> Option<u32> {
        let (w, outer) = self.rim_kind(rim)?;
        Some(self.push(Kind::Cabochon { at: Anchor::Rim { rim, u, along: 0.0, across: outer * w * 0.6 }, turn: 0.0, rx: (w * 2.0).max(6.0), ry: (w * 1.5).max(4.5) }))
    }
    /// A trellis field round `c`, `r` mm across, behind everything.
    pub fn add_trellis(&mut self, c: Point, r: f64) -> u32 {
        let ctrl = (0..8).map(|i| { let a = PI * 2.0 * i as f64 / 8.0; pt(c.x + a.cos() * r * 1.3, c.y + a.sin() * r * 0.8) }).collect();
        let id = self.push(Kind::Trellis { ctrl, cell: (r * 0.25).clamp(5.0, 14.0), florets: true });
        self.restack(id, false);
        id
    }
}

/// A cabochon at `c`, its long axis at angle `turn`.
fn cabochon_at(c: Point, turn: f64, rx: f64, ry: f64) -> Part {
    let mut p = rocaille::cabochon(pt(0.0, 0.0), rx, ry);
    let (s, k) = turn.sin_cos();
    let f = |q: &mut Point| { *q = pt(c.x + q.x * k - q.y * s, c.y + q.x * s + q.y * k); };
    p.outline.iter_mut().for_each(f);
    for l in p.lines.iter_mut() { l.iter_mut().for_each(f); }
    p
}

/// The parts of one element.
pub fn element_parts(e: &Element, spines: &HashMap<u32, Spine>) -> Option<Vec<Part>> {
    let place = |at: &Anchor| Design::place(spines, at);
    Some(match &e.kind {
        Kind::Rim { width, outer, swell, twists, .. } => {
            let s = spines.get(&e.id)?;
            if *swell <= 0.0 && twists.is_empty() { vec![rocaille::moulded_rim(&s.pts, s.ua, s.ub, *width, *outer)] } else {
                // swell over the main path: thin at both ends, full through the body
                let (ua, ub, sw) = (s.ua, s.ub, swell.clamp(0.0, 1.0));
                let main = move |u: f64| ((u - ua) / (ub - ua).max(1e-9)).clamp(0.0, 1.0);
                let width_at = move |u: f64| 1.0 - 0.5 * sw + sw * (PI * main(u)).sin().powf(1.3);
                let tw: Vec<f64> = twists.iter().map(|t| ua + t.clamp(0.0, 1.0) * (ub - ua)).collect();
                // the study function flips the start face for an odd number of turn-overs;
                // here the rim's `outer` always holds at its start and each turn-over flips it
                let first = if tw.len() % 2 == 1 { -*outer } else { *outer };
                vec![rocaille::moulded_rim_varied(&s.pts, s.ua, s.ub, *width, first, width_at, &tw)]
            }
        }
        Kind::Frond { at, heading, length, side, bend, fingers, width, splay, turn } => {
            let (p, h) = place(at)?;
            rocaille::frond_with(p, h + heading, *length, *side, *bend, (*fingers).max(1), *width, *splay, *turn)
        }
        Kind::Shell { at, axis, span, ribs, r, asym, twist, scallop } => { let (p, h) = place(at)?; vec![rocaille::shell(p, h + axis, *span, (*ribs).max(3), *r, *asym, *twist, *scallop)] }
        Kind::Rosette { at, turn, r, petals } => { let (p, h) = place(at)?; rocaille::rosette(p, *r, (*petals).max(3), h + turn) }
        Kind::Run { rim, from, to, outer, count, length, fingers, last } => {
            let s = spines.get(rim)?;
            rocaille::frond_run_with(&s.pts, s.whole(*from), s.whole(*to), *outer, s.width, (*count).max(1), *length, (*fingers).max(1), *last)
        }
        Kind::Frill { rim, from, to, across, side, depth, waves, broken } => {
            let s = spines.get(rim)?;
            let (a, b) = (s.index(*from), s.index(*to));
            let idx: Vec<usize> = if a <= b { (a..=b).collect() } else { (b..=a).rev().collect() };
            let crest: Vec<Point> = idx.into_iter().map(|i| { let (p, h) = s.pts[i]; off(p, h, 0.0, *across) }).collect();
            if crest.len() < 4 { return None; }
            vec![rocaille::frill(&crest, *side, *depth, (*waves).max(1), *broken)]
        }
        Kind::Pocket { ctrl, lip } => { if ctrl.len() < 3 { return None; } vec![rocaille::pocket(rocaille::blob(ctrl), *lip)] }
        Kind::Cabochon { at, turn, rx, ry } => { let (p, h) = place(at)?; vec![cabochon_at(p, h + turn, rx.max(1.0), ry.max(1.0))] }
        Kind::Trellis { ctrl, cell, florets } => {
            if ctrl.len() < 3 { return None; }
            let mut ps = rocaille::trellis(&rocaille::blob(ctrl), cell.max(2.0));
            if !florets { ps.truncate(1); }
            ps
        }
    })
}
/// The layered drawing of parts listed per element.
pub fn drawing_of(parts: &[(u32, Vec<Part>)]) -> rocaille::Drawing {
    let flat: Vec<Part> = parts.iter().flat_map(|(_, p)| p.iter().cloned()).collect();
    rocaille::draw(&flat)
}

// ---- the library structures ----------------------------------------------

/// The starting layouts: id, name, description.
pub const STRUCTURES: [(&str, &str, &str); 7] = [
    ("cartouche", "Cartouche", "Loop cartouche: one moulded rim round a loop and out through a tail, crowned by the shell; layered frond sprays with turnovers, rim rosettes"),
    ("agrafe", "Agrafe", "Apron agrafe: a shell centre with C-rims sweeping out both ways into volutes; the halves mirror but differ"),
    ("corner", "Corner", "Panel corner: the shell fans into the corner, two unequal rims run along the edges into volutes"),
    ("medallion", "Medallion", "Oval medallion (a Louis XV mirror frame): an oval rim broken at the foot into volutes, shell crest"),
    ("frieze", "Frieze", "Running frieze: S-rims alternating up and down, their volutes meeting under small shells"),
    ("wave", "Wave", "Asymmetric wave spray: one continuous rim breaking into a big volute, a crimped crest, spray fronds, a pierced hollow"),
    ("bracket", "Bracket", "Console bracket (side view): wall volute, S body, foot volute, back scroll, frond run and a shell at the knee"),
];

/// A structure in its own coordinates (not fitted to a page).
pub fn structure(id: &str) -> Option<Design> {
    Some(match id {
        "cartouche" => cartouche(),
        "agrafe" => agrafe(),
        "corner" => corner(),
        "medallion" => medallion(),
        "frieze" => frieze(),
        "wave" => wave(),
        "bracket" => bracket(),
        _ => return None,
    })
}

/// Builds a design as the study sheets did, pinning ornament to rims.
struct Build { d: Design, spines: HashMap<u32, Spine> }
impl Build {
    fn new() -> Build { Build { d: Design::empty(0.0, 0.0), spines: HashMap::new() } }
    fn from(d: Design) -> Build { let spines = d.spines(); Build { d, spines } }
    /// A rim through the study's control points, refitted as a few Bézier points with handles.
    /// The study's volute signs (relative to the curve's own turn at each end) become out/in.
    fn rim(&mut self, ctrl: &[Point], hook0: f64, eye0: f64, hook1: f64, eye1: f64, width: f64, outer: f64) -> u32 {
        let (nodes, hook0, hook1) = rim_from_ctrl(ctrl, outer, hook0, hook1);
        let id = self.d.push(Kind::Rim { nodes: nodes.clone(), width, outer, hook0, eye0, hook1, eye1, swell: 0.0, twists: vec![] });
        self.spines.insert(id, rim_spine(&nodes, outer, hook0, eye0, hook1, eye1, width));
        id
    }
    /// On `rim` at its sample nearest `p`, then `along`/`across` that (the study's `off(near(p))`).
    fn at(&self, rim: u32, p: Point, along: f64, across: f64) -> (Anchor, f64) {
        let s = &self.spines[&rim]; let (i, _) = s.nearest(p); let h = s.pts[i].1;
        (Anchor::Rim { rim, u: s.share_of(i), along, across }, h)
    }
    /// The rim heading at the sample nearest `p`.
    fn u(&self, rim: u32, p: Point) -> f64 { let s = &self.spines[&rim]; s.share_of(s.nearest(p).0) }
    /// A point on the page, kept exactly there but pinned to the nearest rim.
    fn pin(&self, p: Point) -> (Anchor, f64) { Design::pin(&self.spines, p, 80.0) }
    fn frond(&mut self, (at, base): (Anchor, f64), heading: f64, length: f64, side: f64, bend: f64, fingers: usize, width: f64, splay: f64, turn: Turnover) {
        self.d.push(Kind::Frond { at, heading: heading - base, length, side, bend, fingers, width, splay, turn });
    }
    /// The study's `frond_at`: bend 0.7, splay 1.05.
    fn frond_at(&mut self, a: (Anchor, f64), heading: f64, length: f64, side: f64, fingers: usize, width: f64, turn: Turnover) { self.frond(a, heading, length, side, 0.7, fingers, width, 1.05, turn); }
    fn shell(&mut self, (at, base): (Anchor, f64), axis: f64, span: f64, ribs: usize, r: f64, asym: f64, twist: f64, scallop: f64) {
        self.d.push(Kind::Shell { at, axis: axis - base, span, ribs, r, asym, twist, scallop });
    }
    fn rosette(&mut self, (at, base): (Anchor, f64), turn: f64, r: f64, petals: usize) { self.d.push(Kind::Rosette { at, turn: turn - base, r, petals }); }
    fn run(&mut self, rim: u32, from: Point, to: Point, outer: f64, count: usize, length: f64, fingers: usize, last: Turnover) {
        let (from, to) = (self.u(rim, from), self.u(rim, to));
        self.d.push(Kind::Run { rim, from, to, outer, count, length, fingers, last });
    }
    fn append(&mut self, other: Design) { self.d.append(other); self.spines = self.d.spines(); }
}

/// Reference pixels (1024 grid) to mm, for a piece 200 mm tall.
fn px(x: f64, y: f64) -> Point { pt(x * 0.195, y * 0.195) }

/// L3-T2: the loop cartouche.
fn cartouche() -> Design {
    let mut b = Build::new();
    let w = 7.2;
    let r = b.rim(&[px(400.0, 612.0), px(300.0, 575.0), px(250.0, 460.0), px(285.0, 345.0), px(430.0, 290.0), px(600.0, 310.0), px(712.0, 420.0), px(712.0, 560.0), px(620.0, 670.0), px(500.0, 735.0), px(430.0, 820.0), px(445.0, 905.0)], 1.0, 11.5, 1.0, 13.0, w, -1.0);
    // the run up the right side
    b.run(r, px(700.0, 610.0), px(690.0, 395.0), -1.0, 4, 50.0, 3, Turnover::Flap);
    // the spray where the tail leaves the loop
    let o = b.at(r, px(530.0, 718.0), 0.0, -w * 0.2);
    b.frond(o, 1.05, 58.0, 1.0, 0.6, 3, 13.0, 0.9, Turnover::None);
    b.frond(o, 0.22, 86.0, 1.0, 0.8, 5, 17.0, 1.1, Turnover::Curl);
    b.frond(o, 2.85, 58.0, -1.0, 0.7, 4, 14.0, 1.0, Turnover::Roll);
    // behind the shell, a frond sweeping out along the top
    let top = b.at(r, px(400.0, 296.0), -12.0, -w * 0.2);
    b.frond(top, top.1 + PI * 0.92, 66.0, -1.0, 0.7, 4, 13.0, 1.0, Turnover::Roll);
    for (x, y, rr) in [(258.0, 470.0, 8.0), (712.0, 470.0, 6.5)] { let a = b.at(r, px(x, y), 0.0, w * 0.25); b.rosette(a, a.1, rr, 5); }
    let s = b.at(r, px(400.0, 296.0), 4.0, w * 0.3);
    b.shell(s, s.1 - PI * 0.5, PI * 0.95, 11, 62.0, 0.45, 0.4, 0.13);
    b.d
}

/// One half of the apron agrafe (the right; `v` varies it).
fn agrafe_half(v: usize) -> Design {
    let mut b = Build::new();
    let w = 6.0;
    let reach = if v == 0 { 140.0 } else { 128.0 };
    let lo = b.rim(&[pt(14.0, 48.0), pt(40.0, 58.0), pt(70.0, 62.0), pt(if v == 0 { 95.0 } else { 86.0 }, 55.0)], 0.0, 0.0, 1.0, 9.0, w * 0.8, 1.0);
    let sp = b.rim(&[pt(8.0, 30.0), pt(45.0, 34.0), pt(reach * 0.6, 26.0), pt(reach * 0.84, 10.0), pt(reach, 16.0)], 0.0, 0.0, 1.0, 11.0, w, -1.0);
    b.run(sp, pt(40.0, 34.0), pt(reach * 0.8, 14.0), -1.0, if v == 0 { 3 } else { 2 }, 40.0, 3, if v == 0 { Turnover::Roll } else { Turnover::None });
    let a = b.pin(pt(8.0, 54.0));
    b.frond_at(a, 1.15, 52.0, -1.0, 4, 13.0, Turnover::None);
    let e = b.at(lo, pt(90.0, 56.0), 0.0, 0.0);
    b.frond_at(e, 0.35, if v == 0 { 50.0 } else { 44.0 }, 1.0, 4, 12.0, if v == 0 { Turnover::Curl } else { Turnover::Roll });
    b.d
}
fn agrafe() -> Design {
    let mut left = agrafe_half(1); left.map(1.0, true, 0.0, pt(0.0, 0.0));
    let mut b = Build::from(agrafe_half(0));
    b.append(left);
    let a = b.pin(pt(0.0, 62.0)); b.shell(a, PI * 0.5, PI * 0.8, 7, 20.0, -0.2, -0.15, 0.14);
    let a = b.pin(pt(0.0, 30.0)); b.shell(a, -PI * 0.5, PI * 0.95, 11, 48.0, 0.3, 0.3, 0.13);
    b.d
}

fn corner() -> Design {
    let mut b = Build::new();
    let w = 6.0;
    let top = b.rim(&[pt(36.0, 24.0), pt(70.0, 18.0), pt(110.0, 16.0), pt(142.0, 22.0), pt(158.0, 36.0)], 0.0, 0.0, 1.0, 10.0, w, -1.0);
    let lft = b.rim(&[pt(24.0, 36.0), pt(18.0, 70.0), pt(16.0, 104.0), pt(24.0, 126.0)], 0.0, 0.0, 1.0, 9.0, w, 1.0);
    b.run(top, pt(62.0, 19.0), pt(135.0, 20.0), 1.0, 3, 38.0, 3, Turnover::None);
    b.run(lft, pt(18.0, 60.0), pt(18.0, 110.0), -1.0, 2, 36.0, 3, Turnover::None);
    let a = b.at(top, pt(152.0, 30.0), 0.0, 0.0); b.frond_at(a, 2.0, 50.0, -1.0, 4, 12.0, Turnover::Curl);
    let a = b.at(lft, pt(22.0, 122.0), 0.0, 0.0); b.frond_at(a, 0.5, 46.0, 1.0, 4, 12.0, Turnover::Roll);
    let a = b.pin(pt(36.0, 36.0)); b.frond_at(a, PI * 0.25, 60.0, 1.0, 4, 14.0, Turnover::Flap);
    let a = b.at(top, pt(52.0, 20.0), 0.0, -w * 0.25); b.rosette(a, a.1, 6.0, 5);
    let a = b.pin(pt(32.0, 32.0)); b.shell(a, -PI * 0.75, PI * 0.95, 11, 40.0, 0.2, 0.3, 0.13);
    b.d
}

/// One side of the oval medallion (the right; `v` varies it).
fn medallion_side(v: usize) -> Design {
    let mut b = Build::new();
    let (rx, ry, w) = (70.0, 95.0, 6.0);
    let ctrl: Vec<Point> = (0..=8).map(|k| { let t = PI * (0.07 + 0.83 * k as f64 / 8.0); pt(rx * t.sin(), -ry * t.cos()) }).collect();
    let sp = b.rim(&ctrl, 0.0, 0.0, 1.0, 10.0, w, -1.0);
    let at_t = |t: f64| pt(rx * (PI * t).sin(), -ry * (PI * t).cos());
    b.run(sp, at_t(if v == 0 { 0.32 } else { 0.4 }), at_t(0.68), -1.0, if v == 0 { 3 } else { 2 }, 38.0, 3, if v == 0 { Turnover::Flap } else { Turnover::None });
    let a = b.pin(pt(14.0, -ry + 2.0));
    b.frond_at(a, 0.25, if v == 0 { 58.0 } else { 50.0 }, -1.0, 4, 13.0, if v == 0 { Turnover::Roll } else { Turnover::None });
    let a = b.at(sp, at_t(0.2), 0.0, w * 0.25); b.rosette(a, a.1, 6.5, 5);
    let a = b.pin(pt(4.0, ry + 6.0));
    b.frond_at(a, PI * 0.5 - 0.6, if v == 0 { 46.0 } else { 40.0 }, -1.0, 4, 12.0, if v == 0 { Turnover::None } else { Turnover::Roll });
    b.d
}
fn medallion() -> Design {
    let mut left = medallion_side(1); left.map(1.0, true, 0.0, pt(0.0, 0.0));
    let mut b = Build::from(medallion_side(0));
    b.append(left);
    let a = b.pin(pt(0.0, 104.0)); b.shell(a, PI * 0.5, PI * 0.8, 7, 22.0, 0.2, 0.15, 0.14);
    let a = b.pin(pt(0.0, -92.0)); b.shell(a, -PI * 0.5, PI * 0.95, 11, 46.0, 0.45, 0.4, 0.13);
    b.d
}

/// One frieze unit: an S-rim rising left to right, volutes at both ends.
fn frieze_unit(v: usize) -> Design {
    let mut b = Build::new();
    let w = 6.5;
    let sp = b.rim(&[pt(2.0, 18.0), pt(36.0, 22.0), pt(75.0, 2.0), pt(114.0, -18.0), pt(146.0, -16.0)], 1.0, 10.0, 1.0, 10.0, w, 1.0);
    b.run(sp, pt(44.0, 18.0), pt(112.0, -16.0), 1.0, 3, 42.0, 3, if v % 2 == 0 { Turnover::Roll } else { Turnover::None });
    let a = b.at(sp, pt(70.0, 5.0), 0.0, 0.0);
    b.frond_at(a, -2.2, 52.0, 1.0, 4, 13.0, if v % 2 == 1 { Turnover::Curl } else { Turnover::None });
    b.frond_at(a, -0.9, 44.0, -1.0, 3, 12.0, Turnover::None);
    b.d
}
fn frieze() -> Design {
    let mut b = Build::new();
    for k in 0..3 {
        let mut u = frieze_unit(k);
        if k % 2 == 1 { u.map(1.0, true, PI, pt(0.0, 0.0)); }
        u.map(1.0, false, 0.0, pt(k as f64 * 150.0, 0.0));
        b.append(u);
        if k < 2 {
            let up = k % 2 == 0;
            let a = b.pin(pt(k as f64 * 150.0 + 148.0, if up { -20.0 } else { 20.0 }));
            b.shell(a, if up { -PI * 0.5 } else { PI * 0.5 }, PI * 0.85, 9, 30.0, 0.3, 0.25, 0.14);
        }
    }
    b.d
}

/// Q: the wave spray, one continuous rim drawn on top of its crest and fronds.
fn wave() -> Design {
    let mut b = Build::new();
    let w = 7.5;
    b.d.push(Kind::Pocket { ctrl: vec![pt(74.0, 104.0), pt(96.0, 64.0), pt(136.0, 78.0), pt(150.0, 124.0), pt(146.0, 176.0), pt(118.0, 196.0), pt(92.0, 168.0), pt(84.0, 132.0)], lip: 1.8 });
    let r = b.rim(&[pt(34.0, 150.0), pt(44.0, 182.0), pt(76.0, 204.0), pt(116.0, 210.0), pt(150.0, 196.0), pt(163.0, 150.0), pt(153.0, 92.0), pt(122.0, 52.0), pt(80.0, 42.0), pt(50.0, 62.0), pt(46.0, 96.0), pt(64.0, 110.0)], 1.0, 11.0, 1.0, 16.0, w, 1.0);
    let (from, to) = (b.u(r, pt(163.0, 140.0)), b.u(r, pt(48.0, 70.0)));
    b.d.push(Kind::Frill { rim: r, from, to, across: w * 0.85, side: 1.0, depth: 13.0, waves: 7, broken: 0.5 });
    for (x, y, hd, l, t) in [(100.0, 44.0, 0.1, 60.0, Turnover::Curl), (146.0, 76.0, 0.3, 54.0, Turnover::None), (162.0, 130.0, 0.5, 46.0, Turnover::Roll)] {
        let a = b.at(r, pt(x, y), 0.0, w * 0.6);
        b.frond_at(a, a.1 + PI * 0.5 + hd, l, 1.0, 4, 13.0, t);
    }
    let a = b.at(r, pt(156.0, 186.0), 0.0, w * 0.6);
    b.frond_at(a, a.1 + PI * 0.5 + 0.2, 50.0, 1.0, 4, 13.0, Turnover::Flap);
    b.d.restack(r, true);
    b.d
}

fn bracket() -> Design {
    let mut b = Build::new();
    let w = 11.0;
    b.rim(&[pt(8.0, 58.0), pt(14.0, 104.0), pt(34.0, 140.0), pt(58.0, 152.0)], 0.0, 0.0, 1.0, 9.0, w * 0.55, -1.0);
    let sp = b.rim(&[pt(26.0, 30.0), pt(70.0, 22.0), pt(100.0, 40.0), pt(96.0, 90.0), pt(70.0, 140.0), pt(52.0, 185.0), pt(58.0, 215.0)], 1.0, 22.0, 1.0, 14.0, w, -1.0);
    b.run(sp, pt(97.0, 60.0), pt(60.0, 175.0), -1.0, 4, 46.0, 3, Turnover::Roll);
    let o = b.at(sp, pt(58.0, 205.0), 0.0, -w * 0.3);
    b.frond_at(o, PI * 0.42, 52.0, 1.0, 4, 13.0, Turnover::None);
    b.frond_at(o, PI * 0.62, 44.0, -1.0, 3, 12.0, Turnover::Curl);
    let a = b.pin(pt(96.0, 64.0)); b.shell(a, -PI * 0.1, PI * 0.8, 9, 30.0, 0.3, 0.3, 0.13);
    b.d
}
