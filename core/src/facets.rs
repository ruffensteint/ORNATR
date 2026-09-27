//! Faceted chip model: the Faceted engine of the Chip workspace.
//!
//! A chip is an outline plus its floor: one deep point (three- and four-corner
//! chips) or a deep line (two-cut sweeps and lenses). The facet lines drawn
//! inside a chip run from its corners to the floor, as on traditional pattern
//! sheets. Rosettes are built from sectors and rings so that chips share
//! vertices and edges and the uncut wood between them reads as part of the design.
use crate::geometry::{distance, lerp, pt, Point};
use std::f64::consts::PI;

#[derive(Clone, Debug)]
pub struct Chip {
    /// Closed outline (first point not repeated).
    pub outline: Vec<Point>,
    /// The deepest point, or the deep line of a two-cut chip.
    pub floor: Vec<Point>,
    /// Outline indices from which a facet line runs to the floor.
    pub corners: Vec<usize>,
}

impl Chip {
    /// Facet lines as drawn on a pattern: corners to the floor, plus the deep line.
    pub fn facet_lines(&self) -> Vec<Vec<Point>> {
        let mut out = vec![];
        if self.floor.len() > 1 { out.push(self.floor.clone()); }
        for &i in &self.corners {
            let p = self.outline[i];
            let f = *self.floor.iter().min_by(|a, b| distance(**a, p).total_cmp(&distance(**b, p))).unwrap();
            if distance(p, f) > 1e-6 { out.push(vec![p, f]); }
        }
        out
    }
    /// Depth at the floor: walls of roughly 45°, capped at `max_depth`. A deep
    /// line lying on the outline (half of a split lens) takes half the chip's width.
    pub fn depth(&self, max_depth: f64) -> f64 {
        let mut reach = self.floor.iter().map(|f| edge_distance(&self.outline, *f)).fold(0.0, f64::max);
        if reach < 1e-6 { reach = 0.5 * self.outline.iter().map(|p| self.floor_point(*p)).map(|(_, d)| d).fold(0.0, f64::max); }
        (reach * 0.9).min(max_depth)
    }
    /// The nearest point of the floor (projected onto a deep line) and its distance.
    fn floor_point(&self, p: Point) -> (Point, f64) {
        if self.floor.len() == 1 { return (self.floor[0], distance(self.floor[0], p)); }
        self.floor.windows(2).map(|w| {
            let (a, b) = (w[0], w[1]); let (dx, dy) = (b.x - a.x, b.y - a.y); let l2 = dx * dx + dy * dy;
            let t = if l2 < 1e-12 { 0.0 } else { (((p.x - a.x) * dx + (p.y - a.y) * dy) / l2).clamp(0.0, 1.0) };
            let q = pt(a.x + dx * t, a.y + dy * t); (q, distance(p, q))
        }).min_by(|a, b| a.1.total_cmp(&b.1)).unwrap()
    }
    /// Facets in 3D (z down into the wood) for a lighting preview: a triangle
    /// where an edge runs down to a deep point, a quad where it runs down to a deep line.
    pub fn faces_3d(&self, max_depth: f64) -> Vec<Vec<(f64, f64, f64)>> {
        let h = self.depth(max_depth);
        let n = self.outline.len();
        let near = |p: Point| self.floor_point(p).0;
        (0..n).map(|i| {
            let (a, b) = (self.outline[i], self.outline[(i + 1) % n]);
            let (fa, fb) = (near(a), near(b));
            let mut f = vec![(a.x, a.y, 0.0), (b.x, b.y, 0.0), (fb.x, fb.y, -h)];
            if distance(fa, fb) > 1e-9 { f.push((fa.x, fa.y, -h)); }
            f
        }).collect()
    }
    /// Plan-view triangles of the facets with their brightness, ready to paint.
    /// A quad is shaded as one face, so curved walls read smooth, not striped.
    pub fn lit_triangles(&self, max_depth: f64) -> Vec<([Point; 3], u8)> {
        let mut out = vec![];
        for f in self.faces_3d(max_depth) {
            let k = shade(&f); let p: Vec<Point> = f.iter().map(|q| pt(q.0, q.1)).collect();
            out.push(([p[0], p[1], p[2]], k));
            if p.len() == 4 { out.push(([p[0], p[2], p[3]], k)); }
        }
        out
    }
}

fn edge_distance(poly: &[Point], p: Point) -> f64 {
    let n = poly.len();
    (0..n).map(|i| seg_distance(p, poly[i], poly[(i + 1) % n])).fold(f64::INFINITY, f64::min)
}
fn seg_distance(p: Point, a: Point, b: Point) -> f64 {
    let (dx, dy) = (b.x - a.x, b.y - a.y); let l2 = dx * dx + dy * dy;
    let t = if l2 < 1e-12 { 0.0 } else { (((p.x - a.x) * dx + (p.y - a.y) * dy) / l2).clamp(0.0, 1.0) };
    distance(p, pt(a.x + dx * t, a.y + dy * t))
}
fn centroid(poly: &[Point]) -> Point {
    let n = poly.len(); let (mut a, mut cx, mut cy) = (0.0, 0.0, 0.0);
    for i in 0..n { let (p, q) = (poly[i], poly[(i + 1) % n]); let c = p.x * q.y - q.x * p.y; a += c; cx += (p.x + q.x) * c; cy += (p.y + q.y) * c; }
    if a.abs() < 1e-12 { return poly[0]; }
    pt(cx / (3.0 * a), cy / (3.0 * a))
}

pub fn polar(c: Point, r: f64, a: f64) -> Point { pt(c.x + r * a.cos(), c.y + r * a.sin()) }

/// Three-corner chip: facets meet at the centroid.
pub fn tri(a: Point, b: Point, c: Point) -> Chip {
    Chip { floor: vec![pt((a.x + b.x + c.x) / 3.0, (a.y + b.y + c.y) / 3.0)], outline: vec![a, b, c], corners: vec![0, 1, 2] }
}
/// Four-corner chip (kite or diamond): facets meet at `deep`.
pub fn quad(a: Point, b: Point, c: Point, d: Point, deep: Point) -> Chip {
    Chip { outline: vec![a, b, c, d], floor: vec![deep], corners: vec![0, 1, 2, 3] }
}
/// A chip from a plain outline, as the classic generator (`chip.rs`) makes them:
/// 21 points = half of a split lens (deep line on its chord), 42 = a whole lens
/// (deep line along the chord), anything else a polygon cut to its centroid.
pub fn from_outline(p: &[Point]) -> Chip {
    let n = p.len();
    if n == 21 { return Chip { outline: p.to_vec(), floor: vec![p[0], p[20]], corners: vec![] }; }
    if n == 42 { return Chip { outline: p.to_vec(), floor: vec![lerp(p[0], p[20], 0.12), lerp(p[0], p[20], 0.88)], corners: vec![0, 20] }; }
    let c = centroid(p);
    let deep = if crate::outline::inside(c, p) { c } else { pt(p.iter().map(|q| q.x).sum::<f64>() / n as f64, p.iter().map(|q| q.y).sum::<f64>() / n as f64) };
    Chip { outline: p.to_vec(), floor: vec![deep], corners: (0..n).collect() }
}
/// Circular arc from `a` to `b` bulging to the left of a→b by `sag`; `n` segments, both ends included.
pub fn arc(a: Point, b: Point, sag: f64, n: usize) -> Vec<Point> {
    let (dx, dy) = (b.x - a.x, b.y - a.y); let l = dx.hypot(dy); let (nx, ny) = (-dy / l, dx / l);
    if sag.abs() < 1e-9 { return (0..=n).map(|i| lerp(a, b, i as f64 / n as f64)).collect(); }
    let r = (l * l / 4.0 + sag * sag) / (2.0 * sag.abs());
    let m = lerp(a, b, 0.5); let off = r - sag.abs(); let s = sag.signum();
    let c = pt(m.x - nx * off * s, m.y - ny * off * s);
    let (a0, a1) = ((a.y - c.y).atan2(a.x - c.x), (b.y - c.y).atan2(b.x - c.x));
    let mut sweep = a1 - a0;
    // take the short way round, on the bulge side
    while sweep > PI { sweep -= 2.0 * PI; } while sweep < -PI { sweep += 2.0 * PI; }
    (0..=n).map(|i| polar(c, r, a0 + sweep * i as f64 / n as f64)).collect()
}
/// Lens (two-cut) chip between `a` and `b`, symmetric, deep line along the chord.
pub fn lens(a: Point, b: Point, sag: f64) -> Chip {
    let n = 16;
    let mut o = arc(a, b, sag, n);
    let back = arc(b, a, sag, n);
    o.extend(back[1..n].iter().copied());
    let inset = 0.12;
    Chip { outline: o, floor: vec![lerp(a, b, inset), lerp(a, b, 1.0 - inset)], corners: vec![0, n] }
}

/// The rosette constructions, as offered in the app.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Centre { Star, Petals, Fan, Swirl }

impl Centre {
    pub const ALL: [Centre; 4] = [Centre::Star, Centre::Petals, Centre::Fan, Centre::Swirl];
    pub fn key(self) -> &'static str { match self { Centre::Star => "star", Centre::Petals => "petals", Centre::Fan => "fan", Centre::Swirl => "swirl" } }
    pub fn from_key(k: &str) -> Option<Centre> { Centre::ALL.into_iter().find(|c| c.key() == k) }
    pub fn label(self) -> &'static str { match self { Centre::Star => "Faceted star", Centre::Petals => "Compass petals", Centre::Fan => "Fan and rings", Centre::Swirl => "Swirl" } }
    /// What the count means for this construction.
    pub fn count_label(self) -> &'static str { match self { Centre::Star => "Points", Centre::Petals => "Petals", Centre::Fan => "Rays", Centre::Swirl => "Sweeps" } }
    pub fn counts(self) -> std::ops::RangeInclusive<u32> { match self { Centre::Star => 5..=16, Centre::Petals => 4..=12, Centre::Fan => 6..=24, Centre::Swirl => 6..=18 } }
    pub fn default_count(self) -> u32 { match self { Centre::Star => 8, Centre::Petals => 6, Centre::Fan => 12, Centre::Swirl => 12 } }
    pub fn build(self, c: Point, r: f64, count: u32) -> Rosette {
        let n = count.clamp(*self.counts().start(), *self.counts().end()) as usize;
        match self { Centre::Star => faceted_star(c, r, n), Centre::Petals => hexafoil(c, r, n), Centre::Fan => fan_rings(c, r, n), Centre::Swirl => swirl(c, r, n) }
    }
}

/// A rosette study: a named set of chips inside a circle.
pub struct Rosette { pub name: &'static str, pub note: &'static str, pub centre: Point, pub radius: f64, pub chips: Vec<Chip> }

/// A: faceted star with inset gap chips and a rim of small triangles.
pub fn faceted_star(c: Point, r: f64, points: usize) -> Rosette {
    let n = points as f64; let mut chips = vec![];
    let tip_r = r * 0.80; let sh_r = r * 0.36; let step = 2.0 * PI / n;
    for k in 0..points {
        let a = k as f64 * step - PI / 2.0;
        let tip = polar(c, tip_r, a);
        let l = polar(c, sh_r, a - step / 2.0); let rr = polar(c, sh_r, a + step / 2.0);
        // the point is two three-corner chips meeting on its axis: the classic faceted star
        chips.push(tri(c, l, tip)); chips.push(tri(c, tip, rr));
        // gap between points: an inward triangle, leaving an uncut margin
        let g = a + step / 2.0;
        let inner = polar(c, sh_r + (tip_r - sh_r) * 0.35, g);
        chips.push(tri(inner, polar(c, tip_r * 0.98, g - step * 0.28), polar(c, tip_r * 0.98, g + step * 0.28)));
    }
    // rim: a band of outward triangles between two circles
    rim_band(&mut chips, c, r * 0.86, r * 0.97, points * 3, 0.0);
    Rosette { name: "A  Faceted star", note: "8 split points, inset gap chips, triangle rim", centre: c, radius: r, chips }
}

/// B: hexafoil (compass petals); the curved spaces between petals are cut, so the petals stand.
pub fn hexafoil(c: Point, r: f64, petals: usize) -> Rosette {
    let n = petals as f64; let step = 2.0 * PI / n; let mut chips = vec![];
    let len = r * 0.72; let sag = len / 2.0 * (PI / n / 2.0).tan();
    let seg = 14;
    for k in 0..petals {
        let a = k as f64 * step - PI / 2.0;
        let tip = polar(c, len, a); let next = polar(c, len, a + step);
        // with y down, "left" of c→tip faces increasing angle: petal k's edge bulges toward k+1,
        // petal k+1's edge bulges back toward k
        let side_k = arc(c, tip, sag, seg); let side_n = arc(c, next, -sag, seg);
        let t0 = 2; // stop short of the centre: no hair-thin cusp
        let mut o: Vec<Point> = side_k[t0..].to_vec();
        // the outer edge follows the circle through the tips (outward is right of tip→next)
        let rim = arc(tip, next, -len * (1.0 - (step / 2.0).cos()), 10);
        o.extend(rim[1..10].iter().copied());
        let mut back: Vec<Point> = side_n[t0..].to_vec(); back.reverse(); o.extend(back);
        let deep = centroid(&o);
        let cn = o.len();
        chips.push(Chip { outline: o, floor: vec![deep], corners: vec![0, seg - t0, seg - t0 + 10, cn - 1] });
    }
    // second ring: small lenses outside, between the petal tips
    for k in 0..petals {
        let a = k as f64 * step - PI / 2.0 + step / 2.0;
        chips.push(lens(polar(c, len * 1.06, a), polar(c, r * 0.96, a), r * 0.075));
        // an arrow chip on the rim pointing at each petal tip
        let b = a - step / 2.0;
        chips.push(tri(polar(c, r * 0.96, b - step * 0.24), polar(c, len * 1.05, b), polar(c, r * 0.96, b + step * 0.24)));
    }
    Rosette { name: "B  Compass petals", note: "6 standing petals, curved chips between, lenses and triangles", centre: c, radius: r, chips }
}

/// C: fan centre (alternate slices cut), a zigzag band and a ring of diamonds.
pub fn fan_rings(c: Point, r: f64, rays: usize) -> Rosette {
    let n = (rays * 2) as f64; let step = 2.0 * PI / n; let mut chips = vec![];
    let r1 = r * 0.42;
    for k in (0..rays * 2).step_by(2) {
        let a = k as f64 * step - PI / 2.0;
        chips.push(tri(polar(c, r * 0.03, a + step / 2.0), polar(c, r1, a), polar(c, r1, a + step)));
    }
    rim_band(&mut chips, c, r1 + r * 0.04, r * 0.68, rays, 0.5);
    let (ra, rb) = (r * 0.72, r * 0.96); let m = rays * 2; let s2 = 2.0 * PI / m as f64;
    for k in 0..m {
        let a = k as f64 * s2 - PI / 2.0;
        let (p, q) = (polar(c, ra, a), polar(c, rb, a));
        let mid = (ra + rb) / 2.0; let w = s2 * 0.42;
        chips.push(quad(p, polar(c, mid, a + w), q, polar(c, mid, a - w), polar(c, mid, a)));
    }
    Rosette { name: "C  Fan and rings", note: "12 cut rays, zigzag band, ring of diamonds", centre: c, radius: r, chips }
}

/// D: swirl of two-cut crescents around a small faceted star.
pub fn swirl(c: Point, r: f64, count: usize) -> Rosette {
    let n = count as f64; let step = 2.0 * PI / n; let mut chips = vec![];
    let (r0, r1) = (r * 0.3, r * 0.95); let twist = 0.9; let width = step * 0.8; let seg = 24;
    // broad belly, soft taper at both ends: no thin tails
    let belly = |t: f64| (PI * t).sin().powf(0.75);
    for k in 0..count {
        let a = k as f64 * step - PI / 2.0;
        let at = |t: f64, off: f64| polar(c, r0 + (r1 - r0) * t, a + twist * t * (2.0 - t) + off);
        let mut o: Vec<Point> = (0..=seg).map(|i| at(i as f64 / seg as f64, 0.0)).collect();
        o.extend((1..seg).rev().map(|i| { let t = i as f64 / seg as f64; at(t, width * belly(t)) }));
        // the steep wall is the leading (concave) side, so the deep line sits near it
        let floor: Vec<Point> = (2..=seg - 2).map(|i| { let t = i as f64 / seg as f64; at(t, 0.32 * width * belly(t)) }).collect();
        chips.push(Chip { outline: o, floor, corners: vec![0, seg] });
    }
    // centre star: one point per two crescents
    let sr = r0 * 0.88; let m = count / 2; let s2 = 2.0 * PI / m as f64;
    for k in 0..m {
        let a = k as f64 * s2 - PI / 2.0;
        chips.push(tri(c, polar(c, sr * 0.42, a - s2 / 2.0), polar(c, sr, a)));
        chips.push(tri(c, polar(c, sr, a), polar(c, sr * 0.42, a + s2 / 2.0)));
    }
    Rosette { name: "D  Swirl", note: "12 sweeping two-cut crescents around a small star", centre: c, radius: r, chips }
}

/// A ring of outward-pointing triangles between radii `r0` and `r1`; `phase` in steps.
pub fn rim_band(chips: &mut Vec<Chip>, c: Point, r0: f64, r1: f64, count: usize, phase: f64) {
    let step = 2.0 * PI / count as f64;
    for k in 0..count {
        let a = (k as f64 + phase) * step - PI / 2.0;
        chips.push(tri(polar(c, r0, a), polar(c, r1, a + step / 2.0), polar(c, r0, a + step)));
    }
}

impl Chip {
    /// The chip turned by `quarter` quarter turns about `c`.
    pub fn turned(&self, c: Point, quarter: usize) -> Chip {
        let f = |p: &Point| { let (mut x, mut y) = (p.x - c.x, p.y - c.y); for _ in 0..quarter % 4 { let t = x; x = -y; y = t; } pt(c.x + x, c.y + y) };
        Chip { outline: self.outline.iter().map(f).collect(), floor: self.floor.iter().map(f).collect(), corners: self.corners.clone() }
    }
}

// ---------------------------------------------------------------- borders

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BorderStyle { Zigzag, Arcade, Almond }

impl BorderStyle {
    pub const ALL: [BorderStyle; 3] = [BorderStyle::Zigzag, BorderStyle::Arcade, BorderStyle::Almond];
    pub fn key(self) -> &'static str { match self { BorderStyle::Zigzag => "zigzag", BorderStyle::Arcade => "arcade", BorderStyle::Almond => "almond" } }
    pub fn from_key(k: &str) -> Option<BorderStyle> { BorderStyle::ALL.into_iter().find(|b| b.key() == k) }
    pub fn label(self) -> &'static str {
        match self { BorderStyle::Zigzag => "Zigzag ribbon", BorderStyle::Arcade => "Arcade of fans", BorderStyle::Almond => "Almond chain" }
    }
    pub fn note(self) -> &'static str {
        match self {
            BorderStyle::Zigzag => "facing triangles leave an uncut zigzag; diamond corners",
            BorderStyle::Arcade => "arches with half-sunbursts, curved spandrels; quarter-fan corners",
            BorderStyle::Almond => "lenses on the centre line, paired arrows between; diagonal-lens corners",
        }
    }
    /// Target repeat length as a multiple of the band width.
    fn pitch(self) -> f64 { match self { BorderStyle::Zigzag => 0.9, BorderStyle::Arcade => 2.0, BorderStyle::Almond => 1.6 } }
}

/// A square border band of width `w` inside the square at `o` with side `size`.
/// Each side is a whole number of repeats (the pitch stretches to fit), and
/// the corners are square blocks with their own construction.
pub fn square_border(style: BorderStyle, o: Point, size: f64, w: f64) -> Vec<Chip> {
    let run = size - 2.0 * w;
    let n = ((run / (style.pitch() * w)).round() as usize).max(1);
    let p = run / n as f64;
    let c = pt(o.x + size / 2.0, o.y + size / 2.0);
    let (y0, y1) = (o.y, o.y + w); // outer and inner edge of the top side
    let mut side: Vec<Chip> = vec![];
    match style {
        BorderStyle::Zigzag => {
            // outer triangles on unit centres, inner triangles on the joins; their sides run parallel
            for i in 0..n { let x = o.x + w + i as f64 * p; side.push(tri(pt(x, y0), pt(x + p, y0), pt(x + p / 2.0, y0 + 0.62 * w))); }
            for i in 1..n { let x = o.x + w + i as f64 * p; side.push(tri(pt(x + p / 2.0, y1), pt(x - p / 2.0, y1), pt(x, y0 + 0.38 * w))); }
            let m = pt(o.x + w / 2.0, o.y + w / 2.0); let h = w * 0.36;
            side.push(quad(pt(m.x, m.y - h), pt(m.x + h, m.y), pt(m.x, m.y + h), pt(m.x - h, m.y), m));
        }
        BorderStyle::Arcade => {
            // an arch can't rise more than half its span, or the arc flips
            let sag = (0.9 * w).min(0.48 * p); let seg = 24;
            let arches: Vec<Vec<Point>> = (0..n).map(|i| { let x = o.x + w + i as f64 * p; arc(pt(x, y1), pt(x + p, y1), -sag, seg) }).collect();
            for i in 0..n {
                // half-sunburst inside each arch: alternate slices cut
                let x = o.x + w + i as f64 * p; let base = pt(x + p / 2.0, y1 - 0.04 * w);
                let rr = (p * p / 4.0 + sag * sag) / (2.0 * sag) * 0.8; let k = 7;
                for j in (0..k).step_by(2) {
                    let (a0, a1) = (PI + PI * j as f64 / k as f64, PI + PI * (j + 1) as f64 / k as f64);
                    side.push(tri(base, polar(base, rr, a0), polar(base, rr, a1)));
                }
                // spandrel between this arch and the next: a curved three-corner chip
                if i + 1 < n {
                    let (a, b) = (&arches[i], &arches[i + 1]); let (s0, s1) = ((seg as f64 * 0.62) as usize, seg - 1);
                    let mut o2: Vec<Point> = a[s0..=s1].to_vec();
                    o2.extend(b[1..=seg - s0].iter().copied());
                    let cn = o2.len(); let deep = centroid(&o2);
                    side.push(Chip { outline: o2, floor: vec![deep], corners: vec![0, s1 - s0, cn - 1] });
                }
            }
            // quarter fan in the corner, from the inner corner toward the outer one
            let ic = pt(o.x + w, o.y + w);
            for j in [0usize, 2, 4] { let (a0, a1) = (PI + PI / 2.0 * j as f64 / 5.0, PI + PI / 2.0 * (j + 1) as f64 / 5.0); side.push(tri(ic, polar(ic, w * 0.85, a0), polar(ic, w * 0.85, a1))); }
        }
        BorderStyle::Almond => {
            let ym = (y0 + y1) / 2.0;
            for i in 0..n { let x = o.x + w + i as f64 * p; side.push(lens(pt(x + 0.1 * p, ym), pt(x + 0.9 * p, ym), 0.3 * w)); }
            for i in 1..n {
                let x = o.x + w + i as f64 * p; let h = 0.16 * p;
                side.push(tri(pt(x - h, y0), pt(x + h, y0), pt(x, y0 + 0.42 * w)));
                side.push(tri(pt(x + h, y1), pt(x - h, y1), pt(x, y1 - 0.42 * w)));
            }
            side.push(lens(pt(o.x + 0.14 * w, o.y + 0.14 * w), pt(o.x + 0.86 * w, o.y + 0.86 * w), 0.2 * w));
        }
    }
    (0..4).flat_map(|k| side.iter().map(move |ch| ch.turned(c, k)).collect::<Vec<_>>()).collect()
}

// ---------------------------------------------------------------- drawing

fn path(points: &[Point], close: bool) -> String {
    let mut s: String = points.iter().enumerate().map(|(i, p)| format!("{}{:.3} {:.3} ", if i == 0 { "M" } else { "L" }, p.x, p.y)).collect();
    if close { s.push('Z'); }
    s
}

/// The pattern as drawn: outlines, and the thinner facet lines inside.
pub fn svg_lines(chips: &[Chip]) -> String {
    let mut s = String::from("<g fill=\"none\" stroke=\"#000\" stroke-linejoin=\"round\" stroke-linecap=\"round\">");
    for chip in chips {
        s += &format!("<path d=\"{}\" stroke-width=\"0.35\"/>", path(&chip.outline, true));
        for l in chip.facet_lines() { s += &format!("<path d=\"{}\" stroke-width=\"0.18\"/>", path(&l, false)); }
    }
    s + "</g>"
}

/// Brightness 0–255 of a face (three or more 3D corners, normal by Newell's
/// method) lit from the upper left 35° above the wood.
pub fn shade(f: &[(f64, f64, f64)]) -> u8 {
    let mut n = (0.0, 0.0, 0.0);
    for i in 0..f.len() {
        let (a, b) = (f[i], f[(i + 1) % f.len()]);
        n.0 += (a.1 - b.1) * (a.2 + b.2); n.1 += (a.2 - b.2) * (a.0 + b.0); n.2 += (a.0 - b.0) * (a.1 + b.1);
    }
    if n.2 < 0.0 { n = (-n.0, -n.1, -n.2); }
    let l = (n.0 * n.0 + n.1 * n.1 + n.2 * n.2).sqrt(); if l < 1e-12 { return 200; }
    let (e, az) = (35f64.to_radians(), (-135f64).to_radians());
    let light = (e.cos() * az.cos(), e.cos() * az.sin(), e.sin());
    (40.0 + 200.0 * ((n.0 * light.0 + n.1 * light.1 + n.2 * light.2) / l).max(0.0)) as u8
}
/// Wood tone for a brightness.
pub fn wood(k: u8) -> String { let k = k as u32; format!("rgb({},{},{})", k * 235 / 255, k * 200 / 255, k * 150 / 255) }
/// Colour of the uncut surface.
pub fn wood_surface() -> String { wood(shade(&[(0.0, 0.0, 0.0), (1.0, 0.0, 0.0), (0.0, 1.0, 0.0)])) }

/// Raking-light preview of the cut facets (draw the uncut surface underneath first).
pub fn svg_lit(chips: &[Chip], max_depth: f64) -> String {
    let mut s = String::new();
    for chip in chips {
        for (t, k) in chip.lit_triangles(max_depth) {
            let col = wood(k);
            s += &format!("<path d=\"{}\" fill=\"{col}\" stroke=\"{col}\" stroke-width=\"0.05\"/>", path(&t, true));
        }
    }
    s
}
