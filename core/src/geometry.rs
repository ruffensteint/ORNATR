//! Points, cubic backbones, arc-length frames and polyline helpers.

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Point { pub x: f64, pub y: f64 }
pub const fn pt(x: f64, y: f64) -> Point { Point { x, y } }
pub type Curve = [Point; 4];

pub fn clamp(v: f64, lo: f64, hi: f64) -> f64 { lo.max(hi.min(v)) }
/// sqrt rather than `hypot`: sqrt is exact on every platform, so the desktop and
/// web editions agree, and the web edition's `hypot` made growth ten times slower.
/// (Millimetre distances never come near where hypot's overflow care matters.)
pub fn distance(a: Point, b: Point) -> f64 { let (dx, dy) = (a.x - b.x, a.y - b.y); (dx * dx + dy * dy).sqrt() }
pub fn lerp(a: Point, b: Point, t: f64) -> Point { pt(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t) }
pub fn cross(a: Point, b: Point) -> f64 { a.x * b.y - a.y * b.x }
pub fn sub(a: Point, b: Point) -> Point { pt(a.x - b.x, a.y - b.y) }

pub fn at(c: &Curve, t: f64) -> Point {
    let s = 1.0 - t;
    pt(s * s * s * c[0].x + 3.0 * s * s * t * c[1].x + 3.0 * s * t * t * c[2].x + t * t * t * c[3].x,
       s * s * s * c[0].y + 3.0 * s * s * t * c[1].y + 3.0 * s * t * t * c[2].y + t * t * t * c[3].y)
}
pub fn tangent(c: &Curve, t: f64) -> Point {
    let s = 1.0 - t;
    let x = 3.0 * s * s * (c[1].x - c[0].x) + 6.0 * s * t * (c[2].x - c[1].x) + 3.0 * t * t * (c[3].x - c[2].x);
    let y = 3.0 * s * s * (c[1].y - c[0].y) + 6.0 * s * t * (c[2].y - c[1].y) + 3.0 * t * t * (c[3].y - c[2].y);
    let d = x.hypot(y);
    if d > 1e-8 { return pt(x / d, y / d); }
    let a = at(c, (t - 0.001).max(0.0));
    let b = at(c, (t + 0.001).min(1.0));
    let h = (b.x - a.x).hypot(b.y - a.y);
    if h > 1e-8 { pt((b.x - a.x) / h, (b.y - a.y) / h) } else { pt(1.0, 0.0) }
}

#[derive(Clone, Copy, Debug)]
pub struct ArcRow { pub t: f64, pub length: f64, pub point: Point }
/// 241 samples of the cubic with cumulative length.
pub fn arc_table(c: &Curve) -> Vec<ArcRow> {
    let mut rows = vec![ArcRow { t: 0.0, length: 0.0, point: c[0] }];
    for i in 1..=240 {
        let p = at(c, i as f64 / 240.0);
        let prev = rows[i - 1];
        rows.push(ArcRow { t: i as f64 / 240.0, point: p, length: prev.length + (p.x - prev.point.x).hypot(p.y - prev.point.y) });
    }
    rows
}
pub fn guide_points(c: &Curve) -> Vec<Point> { arc_table(c).into_iter().map(|r| r.point).collect() }
/// Point and unit tangent at an arc-length fraction.
pub fn frame(c: &Curve, progress: f64) -> (Point, Point) {
    let rows = arc_table(c);
    let dist = clamp(progress, 0.0, 1.0) * rows[240].length;
    let found = rows.iter().position(|r| r.length >= dist).map(|i| i as isize).unwrap_or(-1);
    let i = found.max(1) as usize;
    let (a, b) = (rows[i - 1], rows[i]);
    let den = b.length - a.length;
    let fraction = (dist - a.length) / if den != 0.0 { den } else { 1.0 };
    let t = a.t + (b.t - a.t) * fraction;
    (at(c, t), tangent(c, t))
}
/// Arc-length fraction of the backbone sample nearest to p.
pub fn nearest(c: &Curve, p: Point) -> f64 {
    let rows = arc_table(c);
    let mut best = rows[0];
    for r in &rows[1..] { if (r.point.x - p.x).hypot(r.point.y - p.y) < (best.point.x - p.x).hypot(best.point.y - p.y) { best = *r; } }
    if rows[240].length != 0.0 { best.length / rows[240].length } else { 0.0 }
}
/// Least-squares cubic with fixed endpoints and chord-length parameters.
pub fn fit_curve(points: &[Point]) -> Option<Curve> {
    if points.len() < 3 { return None; }
    let mut d = vec![0.0];
    for i in 1..points.len() { d.push(d[i - 1] + distance(points[i], points[i - 1])); }
    let length = *d.last().unwrap();
    if length < 5.0 { return None; }
    let (start, end) = (points[0], *points.last().unwrap());
    let (mut aa, mut ab, mut bb, mut ax, mut ay, mut bx, mut by) = (0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
    for (i, p) in points.iter().enumerate() {
        let t = d[i] / length; let s = 1.0 - t; let a = 3.0 * s * s * t; let b = 3.0 * s * t * t;
        let x = p.x - s * s * s * start.x - t * t * t * end.x; let y = p.y - s * s * s * start.y - t * t * t * end.y;
        aa += a * a; ab += a * b; bb += b * b; ax += a * x; ay += a * y; bx += b * x; by += b * y;
    }
    let det = aa * bb - ab * ab;
    if det.abs() < 1e-8 { return None; }
    Some([start, pt((ax * bb - bx * ab) / det, (ay * bb - by * ab) / det), pt((bx * aa - ax * ab) / det, (by * aa - ay * ab) / det), end])
}

pub fn line_length(p: &[Point]) -> f64 { p.windows(2).map(|w| distance(w[1], w[0])).sum() }
/// Point and heading at an arc-length fraction of a polyline.
pub fn line_frame(points: &[Point], progress: f64) -> (Point, f64) {
    let target = clamp(progress, 0.0, 1.0) * line_length(points);
    let mut total = 0.0;
    for i in 1..points.len() {
        let (a, b) = (points[i - 1], points[i]);
        let d = distance(a, b);
        if total + d >= target || i == points.len() - 1 {
            let t = clamp((target - total) / if d != 0.0 { d } else { 1.0 }, 0.0, 1.0);
            return (lerp(a, b, t), (b.y - a.y).atan2(b.x - a.x));
        }
        total += d;
    }
    (points[0], 0.0)
}

/// Axis-aligned bounds.
#[derive(Clone, Copy, Debug)]
pub struct Bounds { pub l: f64, pub t: f64, pub r: f64, pub b: f64 }
impl Bounds {
    pub fn of(points: &[Point]) -> Bounds {
        let mut o = Bounds { l: f64::INFINITY, t: f64::INFINITY, r: f64::NEG_INFINITY, b: f64::NEG_INFINITY };
        for p in points { o.l = o.l.min(p.x); o.r = o.r.max(p.x); o.t = o.t.min(p.y); o.b = o.b.max(p.y); }
        o
    }
    pub fn overlaps(&self, o: &Bounds) -> bool { self.l <= o.r && self.r >= o.l && self.t <= o.b && self.b >= o.t }
    pub fn contains(&self, p: Point) -> bool { p.x >= self.l && p.x <= self.r && p.y >= self.t && p.y <= self.b }
    pub fn center(&self) -> Point { pt((self.l + self.r) / 2.0, (self.t + self.b) / 2.0) }
}
