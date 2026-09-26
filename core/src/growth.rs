//! Growth model: settings, grown parts, the curl primitive and the entry
//! point that grows one or more backbones (with hand-edited shoots).
use crate::geometry::{arc_table, clamp, guide_points, lerp, line_length, pt, Curve, Point};
use crate::shoots::{apply_shoot_edits, ShootEdit, ShootParams};
use std::f64::consts::PI;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side { Alternate, Left, Right }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Family { Spiral, Spray, Border, Fan, Branching }

#[derive(Clone, Debug, PartialEq)]
pub struct GrowthSettings {
    pub seed: u32, pub branches: f64, pub reach: f64, pub curl: f64, pub levels: u8, pub leaves: u8,
    pub clearance: f64, pub stem: f64, pub side: Side, pub family: Option<Family>, pub composition: Option<u8>,
    pub secondary_scale: Option<f64>, pub sweeps: Option<u8>, pub auto_shoots: Option<bool>, pub flip: Option<bool>,
    /// Hand-placed backbone: keep the scroll's proportions instead of
    /// shrinking it to fit inside the page.
    pub free: Option<bool>,
    /// Grows out of this other backbone: its start sits on that stem and the
    /// join merges like a leaf root.
    pub attach: Option<usize>,
    /// Wrapping leaves laid into the scroll's curl (0 to 2); the volute opens up to make room.
    pub wraps: Option<u8>,
    /// Library leaf (preset id) used for the wrapping leaves; None grows the plain acanthus wrap.
    pub wrap_leaf: Option<String>,
    /// A sheathing collar over the join, for a backbone grown from another:
    /// its size against the stems (1 = default); None is no collar.
    pub collar: Option<f64>,
    /// Collar style id ("axil" or "split"); None is the axil leaf.
    pub collar_style: Option<String>,
    /// Grow as a scroll vine instead: curls seeded along the backbone this many
    /// mm apart, each the largest scroll that fits, branching, never touching,
    /// clad with acanthus (when `leaves` > 0) and fitted to the carving surface.
    pub vine: Option<f64>,
}
impl Default for GrowthSettings {
    fn default() -> Self { GrowthSettings { seed: 1248, branches: 5.0, reach: 33.0, curl: 1.0, levels: 2, leaves: 2, clearance: 2.0, stem: 2.8, side: Side::Alternate, family: None, composition: None, secondary_scale: None, sweeps: None, auto_shoots: None, flip: None, free: None, attach: None, wraps: None, wrap_leaf: None, collar: None, collar_style: None, vine: None } }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind { Backbone, Primary, Secondary, Leaf }

#[derive(Clone, Debug)]
pub struct GrowthPart {
    pub id: String, pub parent: Option<String>, pub kind: Kind,
    pub points: Vec<Point>, pub polygon: Vec<Point>, pub folds: Vec<Vec<Point>>, pub ridges: Option<Vec<Vec<Point>>>,
    /// Inner edges drawn at outline weight (buds made of overlapping pieces).
    pub cuts: Vec<Vec<Point>>,
    pub length: f64, pub width: f64, pub birth: f64, pub duration: f64, pub contour_split: Option<usize>,
    pub shoot: Option<ShootParams>, pub under: bool,
}
#[derive(Clone, Debug, Default)]
pub struct GrowthResult { pub parts: Vec<GrowthPart>, pub skipped: usize, pub attempts: usize, pub message: String }

/// Mulberry32, bit-exact with the web version.
pub struct Mulberry(pub u32);
impl Mulberry {
    pub fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x6D2B79F5);
        let s = self.0;
        let mut t = (s ^ (s >> 15)).wrapping_mul(s | 1);
        t ^= t.wrapping_add((t ^ (t >> 7)).wrapping_mul(t | 61));
        (t ^ (t >> 14)) as f64 / 4294967296.0
    }
}

pub fn ribbon(points: &[Point], width: f64) -> Vec<Point> {
    let (mut left, mut right) = (vec![], vec![]);
    let n = points.len();
    for (i, p) in points.iter().enumerate() {
        let a = points[i.saturating_sub(1)]; let b = points[(i + 1).min(n - 1)]; let angle = (b.y - a.y).atan2(b.x - a.x);
        let half = width * (0.06 + 0.94 * (1.0 - i as f64 / (n as f64 - 1.0)).powf(0.75)) / 2.0;
        left.push(pt(p.x - angle.sin() * half, p.y + angle.cos() * half)); right.push(pt(p.x + angle.sin() * half, p.y - angle.cos() * half));
    }
    right.reverse(); left.extend(right); left
}
/// Logarithmic curl whose root tangent is `angle`; side ±1 picks the turn.
pub fn grow_curl(root: Point, angle: f64, reach: f64, curl: f64, side: f64) -> Vec<Point> {
    let (steps, decay) = (140, 0.24);
    let rotation = angle - side.atan2(-decay);
    let mut points = vec![root];
    for i in 1..=steps {
        let theta = i as f64 / steps as f64 * PI * 2.0 * 1.25 * curl; let r = reach * (-decay * theta).exp();
        let x = r * theta.cos() - reach; let y = side * r * theta.sin();
        points.push(pt(root.x + x * rotation.cos() - y * rotation.sin(), root.y + x * rotation.sin() + y * rotation.cos()));
    }
    points
}

/// Everything a backbone needs to grow: page, curve, locks, edits, settings.
pub struct GrowInput<'a> {
    pub width: f64, pub height: f64, pub curve: Curve,
    pub locked: &'a [GrowthPart], pub shoots: &'a [ShootEdit], pub settings: &'a GrowthSettings,
    /// The carving surface the growth must stay inside (page units); None is the page.
    pub frame: Option<&'a [Point]>,
}

/// Grow one backbone in page units.
pub fn grow_backbone(inp: &GrowInput) -> GrowthResult {
    let s = inp.settings;
    if line_length(&guide_points(&inp.curve)) < 10.0 { return GrowthResult { message: "Draw a longer guide to grow a scroll.".into(), ..Default::default() }; }
    // Grow at one design scale so page size does not cap curls and widths.
    let scale = (inp.width * inp.height / (240.0 * 150.0)).sqrt();
    let shrink = |p: &Point| pt(p.x / scale, p.y / scale);
    let locked: Vec<GrowthPart> = inp.locked.iter().map(|p| GrowthPart { points: p.points.iter().map(shrink).collect(), polygon: p.polygon.iter().map(shrink).collect(), folds: p.folds.iter().map(|f| f.iter().map(shrink).collect()).collect(), ridges: p.ridges.as_ref().map(|r| r.iter().map(|f| f.iter().map(shrink).collect()).collect()), cuts: p.cuts.iter().map(|f| f.iter().map(shrink).collect()).collect(), width: p.width / scale, length: p.length / scale, ..p.clone() }).collect();
    let page = Page { width: inp.width / scale, height: inp.height / scale, curve: inp.curve.map(|p| shrink(&p)), locked: &locked, frame: inp.frame.map(|f| f.iter().map(shrink).collect()) };
    let grown = if s.composition == Some(2) || s.family.map_or(false, |f| f != Family::Spiral) { crate::composed::composed_growth(&page, s) } else { crate::spiral::spiral_anatomy(&page, s) };
    let own = if s.auto_shoots == Some(false) { GrowthResult { parts: grown.parts.into_iter().filter(|p| p.parent.is_none() || locked.iter().any(|q| q.id == p.id)).collect(), ..grown } } else { grown };
    // Wrapping leaves follow the main scroll into its eye, above it and
    // beneath any leaves placed afterwards.
    let own = match s.wraps.filter(|&n| n > 0) {
        Some(n) => { let mut parts = own.parts; if let Some(i) = parts.iter().position(|p| p.parent.is_none()) { let w = crate::wraps::wrapping_leaves(&parts[i], n, s.leaves > 0, s.wrap_leaf.as_deref().and_then(crate::profiles::profile)); parts.splice(i + 1..i + 1, w); } GrowthResult { parts, ..own } }
        None => own,
    };
    let result = apply_shoot_edits(own, &guide_points(&page.curve), inp.shoots, s.leaves > 0);
    let resize = |p: &Point| pt(p.x * scale, p.y * scale);
    let parts = result.parts.into_iter().map(|p| {
        if let Some(l) = inp.locked.iter().find(|q| q.id == p.id) { return l.clone(); }
        GrowthPart { points: p.points.iter().map(resize).collect(), polygon: p.polygon.iter().map(resize).collect(), folds: p.folds.iter().map(|f| f.iter().map(resize).collect()).collect(), ridges: p.ridges.as_ref().map(|r| r.iter().map(|f| f.iter().map(resize).collect()).collect()), cuts: p.cuts.iter().map(|f| f.iter().map(resize).collect()).collect(), width: p.width * scale, length: p.length * scale, ..p }
    }).collect();
    GrowthResult { parts, ..result }
}

/// The normalised page a family grows on, and the frame it must stay inside
/// (a carving surface outline; None is the page rectangle).
pub struct Page<'a> { pub width: f64, pub height: f64, pub curve: Curve, pub locked: &'a [GrowthPart], pub frame: Option<Vec<Point>> }
impl Page<'_> {
    pub fn guide(&self) -> Vec<Point> { arc_table(&self.curve).into_iter().map(|r| r.point).collect() }
    /// Whether grown points stay `margin` inside the frame, or inside the page
    /// rectangle when there is no frame (always, for a free backbone).
    pub fn fits(&self, pts: &[Point], margin: f64, free: bool) -> bool {
        match &self.frame {
            Some(f) => pts.iter().all(|p| crate::outline::inside(*p, f) && (0..f.len()).all(|i| seg_distance(*p, f[i], f[(i + 1) % f.len()]) >= margin)),
            None => free || pts.iter().all(|p| p.x >= margin && p.y >= margin && p.x <= self.width - margin && p.y <= self.height - margin),
        }
    }
}
fn seg_distance(p: Point, a: Point, b: Point) -> f64 {
    let (dx, dy) = (b.x - a.x, b.y - a.y); let l2 = dx * dx + dy * dy;
    let t = if l2 > 0.0 { (((p.x - a.x) * dx + (p.y - a.y) * dy) / l2).clamp(0.0, 1.0) } else { 0.0 };
    (p.x - a.x - t * dx).hypot(p.y - a.y - t * dy)
}

/// Simplified outline during the growth animation (progress < 1).
pub fn growth_polygons(result: &GrowthResult, progress: f64) -> (Vec<Vec<Point>>, Vec<Vec<Point>>) {
    let mut polygons = vec![]; let mut folds = vec![];
    for p in &result.parts {
        if progress >= p.birth + p.duration { polygons.push(p.polygon.clone()); folds.extend(p.folds.iter().cloned()); continue; }
        if progress < p.birth { continue; }
        let t = clamp((progress - p.birth) / p.duration, 0.0, 1.0);
        if p.kind == Kind::Leaf { if t > 0.1 { let root = p.points[0]; polygons.push(p.polygon.iter().map(|q| lerp(root, *q, t)).collect()); } continue; }
        let count = 2usize.max((p.points.len() as f64 * t).ceil() as usize);
        if let Some(split) = p.contour_split {
            let a = 2usize.max((split as f64 * t).ceil() as usize); let b = 2usize.max(((p.polygon.len() - split) as f64 * t).ceil() as usize);
            let mut poly = p.polygon[..a.min(p.polygon.len())].to_vec(); poly.extend_from_slice(&p.polygon[p.polygon.len().saturating_sub(b)..]); polygons.push(poly);
        } else if p.polygon.len() == p.points.len() * 2 {
            let mut poly = p.polygon[..count].to_vec(); poly.extend_from_slice(&p.polygon[p.polygon.len() - count..]); polygons.push(poly);
        } else { polygons.push(ribbon(&p.points[..count.min(p.points.len())], p.width)); }
    }
    (polygons, folds)
}
