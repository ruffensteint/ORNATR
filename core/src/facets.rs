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
pub fn centroid(poly: &[Point]) -> Point {
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
pub enum Centre { Star, Petals, Fan, Swirl, Rocaille }

impl Centre {
    /// "Generate variation" picks only from the first four (`Faceted::from_seed`), so old seeds keep their patterns.
    pub const ALL: [Centre; 5] = [Centre::Star, Centre::Petals, Centre::Fan, Centre::Swirl, Centre::Rocaille];
    pub fn key(self) -> &'static str { match self { Centre::Star => "star", Centre::Petals => "petals", Centre::Fan => "fan", Centre::Swirl => "swirl", Centre::Rocaille => "rocaille" } }
    pub fn from_key(k: &str) -> Option<Centre> { Centre::ALL.into_iter().find(|c| c.key() == k) }
    pub fn label(self) -> &'static str { match self { Centre::Star => "Faceted star", Centre::Petals => "Compass petals", Centre::Fan => "Fan and rings", Centre::Swirl => "Swirl", Centre::Rocaille => "Rocaille swirl" } }
    /// What the count means for this construction.
    pub fn count_label(self) -> &'static str { match self { Centre::Star => "Points", Centre::Petals => "Petals", Centre::Fan => "Rays", Centre::Swirl => "Sweeps", Centre::Rocaille => "Scrolls" } }
    pub fn counts(self) -> std::ops::RangeInclusive<u32> { match self { Centre::Star => 5..=16, Centre::Petals => 4..=12, Centre::Fan => 6..=24, Centre::Swirl => 6..=18, Centre::Rocaille => 5..=9 } }
    pub fn default_count(self) -> u32 { match self { Centre::Star => 8, Centre::Petals => 6, Centre::Fan => 12, Centre::Swirl => 12, Centre::Rocaille => 7 } }
    pub fn build(self, c: Point, r: f64, count: u32) -> Rosette {
        let n = count.clamp(*self.counts().start(), *self.counts().end()) as usize;
        match self { Centre::Star => faceted_star(c, r, n), Centre::Petals => hexafoil(c, r, n), Centre::Fan => fan_rings(c, r, n), Centre::Swirl => swirl(c, r, n), Centre::Rocaille => rocaille_swirl(c, r, n) }
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

/// A C-scroll as a two-cut chip: from a thin tail at `start` along `heading`,
/// curling to `side` ever tighter into a fat hooked head. The deep line runs
/// along the spine, nearer the inside of the curl (the steep wall).
pub fn comma(start: Point, heading: f64, length: f64, side: f64, curl: f64, wmax: f64) -> Chip {
    let smooth = |t: f64| { let t = t.clamp(0.0, 1.0); t * t * (3.0 - 2.0 * t) };
    let n = ((length / 0.4).ceil() as usize).max(24); let ds = length / n as f64;
    let (mut p, mut h) = (start, heading); let mut sp = vec![(p, h)];
    for i in 0..n {
        let u = (i as f64 + 0.5) / n as f64;
        let k = side * (0.4 + curl * 2.6 * smooth((u - 0.4) / 0.6).powf(1.2)) / length;
        let hm = h + k * ds / 2.0; p = pt(p.x + hm.cos() * ds, p.y + hm.sin() * ds); h += k * ds; sp.push((p, h));
    }
    let n = sp.len();
    let w = |i: usize| { let u = i as f64 / (n - 1) as f64; wmax * (0.12 + 0.88 * smooth(u / 0.5)) * (1.0 - 0.9 * smooth((u - 0.62) / 0.38)) + 0.05 };
    let off = |i: usize, f: f64| { let (q, a) = sp[i]; pt(q.x - a.sin() * f, q.y + a.cos() * f) };
    let mut o: Vec<Point> = (0..n).map(|i| off(i, w(i))).collect();
    o.extend((1..n - 1).rev().map(|i| off(i, -w(i))));
    let floor: Vec<Point> = (n / 10..n - n / 10).map(|i| off(i, side * w(i) * 0.25)).collect();
    Chip { outline: o, floor, corners: vec![0, n - 1] }
}

/// A shell as chips: flutes from hinge `h` to a scalloped lip, fanned over
/// `span` round `axis`, reach `r` (longer to one side by `asym`); each flute
/// is cut deepest toward its outer end, the uncut ridges between them are the
/// shell's flutes. `hinge` adds a small chip at the hinge.
pub fn shell_fan(h: Point, axis: f64, span: f64, flutes: usize, r: f64, asym: f64, hinge: bool) -> Vec<Chip> {
    let reach = |t: f64| r * (0.72 + 0.28 * (PI * t).sin()) * (1.0 + asym * (t - 0.5));
    let mut out = vec![];
    for i in 0..flutes {
        let (t0, t1) = (i as f64 / flutes as f64, (i + 1) as f64 / flutes as f64);
        let (a0, a1) = (axis - span / 2.0 + span * t0, axis - span / 2.0 + span * t1);
        let (p0, p1) = (polar(h, reach(t0), a0), polar(h, reach(t1), a1));
        // the scallop bulges away from the hinge
        let mut lip = arc(p0, p1, distance(p0, p1) * 0.22, 8);
        if distance(lip[4], h) < distance(lerp(p0, p1, 0.5), h) { lip = arc(p0, p1, -distance(p0, p1) * 0.22, 8); }
        let mut o = vec![polar(h, r * 0.16, (a0 + a1) / 2.0)]; o.extend(lip.iter().copied());
        let last = o.len() - 1;
        out.push(Chip { outline: o, floor: vec![polar(h, (reach(t0) + reach(t1)) / 2.0 * 0.66, (a0 + a1) / 2.0)], corners: vec![0, 1, last] });
    }
    if hinge { out.push(tri(polar(h, r * 0.16, axis - span / 2.0), polar(h, r * 0.16, axis + span / 2.0), polar(h, r * 0.08, axis + PI))); }
    out
}

/// E: rocaille swirl (the rococo study S2, the user's pick): `count` big
/// C-scroll chips whirling one way round a ring of shell-flute chips, a small
/// counter-scroll tucked in each one's hollow, scallop lenses round the rim
/// between the scroll heads.
pub fn rocaille_swirl(c: Point, r: f64, count: usize) -> Rosette {
    let n = count as f64;
    // the study was drawn for 7 scrolls; angular offsets scale with the step
    let k = 7.0 / n;
    let mut chips = shell_fan(c, -PI / 2.0, 2.0 * PI * 0.999, 2 * count, r * 0.26, 0.0, false);
    for i in 0..count {
        let a = -PI / 2.0 + i as f64 * 2.0 * PI / n;
        chips.push(comma(polar(c, r * 0.32, a), a + 0.35 * k, r * 0.78, 1.0, 3.6, r * 0.11 * k.min(1.0)));
        chips.push(comma(polar(c, r * 0.5, a + 0.42 * k), a + 1.0 * k, r * 0.32, -1.0, 3.0, r * 0.045 * k.min(1.0)));
        let b = a + PI / n + 0.62 * k;
        chips.push(lens(polar(c, r * 0.93, b - 0.12 * k), polar(c, r * 0.93, b + 0.12 * k), -r * 0.035));
    }
    Rosette { name: "E  Rocaille swirl", note: "C-scroll chips whirling round a shell ring, counter-scrolls, rim scallops", centre: c, radius: r, chips }
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
pub fn square_border(style: BorderStyle, o: Point, size: f64, w: f64) -> Vec<Chip> { rect_border(style, o, size, size, w) }

/// A rectangular border band of width `w` inside the rectangle at `o` (`width` × `height`):
/// each side a whole number of repeats, with a corner block at each corner. On a
/// square it is exactly `square_border`.
pub fn rect_border(style: BorderStyle, o: Point, width: f64, height: f64, w: f64) -> Vec<Chip> {
    let (top, sides) = (border_side(style, width - 2.0 * w, w), border_side(style, height - 2.0 * w, w));
    // each side is built in its own frame (x along the side from the corner block, y inward
    // from the outer edge) and placed: top, right, bottom, left, turning clockwise
    let place = |chips: &[Chip], f: &dyn Fn(Point) -> Point| -> Vec<Chip> { chips.iter().map(|ch| Chip { outline: ch.outline.iter().map(|p| f(*p)).collect(), floor: ch.floor.iter().map(|p| f(*p)).collect(), corners: ch.corners.clone() }).collect() };
    let mut out = place(&top, &|p| pt(o.x + w + p.x, o.y + p.y));
    out.extend(place(&sides, &|p| pt(o.x + width - p.y, o.y + w + p.x)));
    out.extend(place(&top, &|p| pt(o.x + width - w - p.x, o.y + height - p.y)));
    out.extend(place(&sides, &|p| pt(o.x + p.y, o.y + height - w - p.x)));
    out
}

/// One side of a border in its own frame: x runs along the side from 0 to `run`, y
/// inward from the outer edge (0) to the band width `w`; the corner block before the
/// side sits at x in [-w, 0].
fn border_side(style: BorderStyle, run: f64, w: f64) -> Vec<Chip> {
    let n = ((run / (style.pitch() * w)).round() as usize).max(1);
    let p = run / n as f64;
    let o = pt(-w, 0.0);
    let (y0, y1) = (o.y, o.y + w); // outer and inner edge of the side
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
    side
}

// ---------------------------------------------------------------- square repeats

/// Square repeat fields built on one lattice. Each cell of side `a` splits into
/// eight octant triangles, node N (a cell corner), edge midpoint M and cell
/// centre C; a pattern is what is cut in one octant, repeated by the square's
/// symmetries, so neighbouring cells meet at exactly the same points.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Repeat { NodeStars, Pinwheel, StarsAndDiamonds, Sunbursts }

impl Repeat {
    pub const ALL: [Repeat; 4] = [Repeat::NodeStars, Repeat::Pinwheel, Repeat::StarsAndDiamonds, Repeat::Sunbursts];
    pub fn key(self) -> &'static str { match self { Repeat::NodeStars => "node-stars", Repeat::Pinwheel => "pinwheel", Repeat::StarsAndDiamonds => "stars-diamonds", Repeat::Sunbursts => "sunbursts" } }
    pub fn from_key(k: &str) -> Option<Repeat> { Repeat::ALL.into_iter().find(|r| r.key() == k) }
    pub fn label(self) -> &'static str {
        match self { Repeat::NodeStars => "Node stars", Repeat::Pinwheel => "Pinwheel", Repeat::StarsAndDiamonds => "Stars and diamonds", Repeat::Sunbursts => "Sunbursts" }
    }
    pub fn note(self) -> &'static str {
        match self {
            Repeat::NodeStars => "8-point star on every node; points meet tip to tip",
            Repeat::Pinwheel => "each cell turns one way; chips meet off-centre on the edges",
            Repeat::StarsAndDiamonds => "4-point stars on nodes and on cell centres, alternating",
            Repeat::Sunbursts => "alternate rays fanned from every node",
        }
    }
    /// Chips in one octant, in cell coordinates: centre C at (0,0), node N at
    /// (-h,-h), edge midpoint M at (0,-h).
    fn octant(self, h: f64) -> Vec<Chip> {
        let (n, m, c) = (pt(-h, -h), pt(0.0, -h), pt(0.0, 0.0));
        match self {
            Repeat::NodeStars => {
                // shoulder between the edge point and the diagonal point; M-S-C stays uncut
                let s = polar(n, h * 0.62, PI / 4.0 - PI / 8.0);
                vec![tri(n, m, s), tri(n, s, c)]
            }
            Repeat::StarsAndDiamonds => {
                // node star shoulder and centre star shoulder on the diagonal, uncut between
                let (sn, sc) = (lerp(n, c, 0.3), lerp(c, n, 0.3));
                vec![tri(n, m, sn), tri(c, sc, m)]
            }
            Repeat::Sunbursts => {
                // the octant's far side M→C split in three; the first and last rays are cut
                let p = |t: f64| lerp(m, c, t);
                vec![tri(n, p(0.0), p(1.0 / 3.0)), tri(n, p(2.0 / 3.0), p(1.0))]
            }
            Repeat::Pinwheel => vec![], // rotational, built per cell below
        }
    }
}

/// A square field of `cells` × `cells` repeats filling the square at `o` with side `size`.
pub fn square_repeat(kind: Repeat, o: Point, size: f64, cells: usize) -> Vec<Chip> { repeat_field(kind, o, size / cells as f64, cells, cells) }

/// `nx` × `ny` cells of side `a` from the corner `o`.
pub fn repeat_field(kind: Repeat, o: Point, a: f64, nx: usize, ny: usize) -> Vec<Chip> {
    let unit = repeat_unit(kind, a / 2.0);
    let mut out = vec![];
    for i in 0..nx { for j in 0..ny {
        let cc = pt(o.x + (i as f64 + 0.5) * a, o.y + (j as f64 + 0.5) * a);
        for ch in &unit {
            let f = |p: &Point| pt(p.x + cc.x, p.y + cc.y);
            out.push(Chip { outline: ch.outline.iter().map(f).collect(), floor: ch.floor.iter().map(f).collect(), corners: ch.corners.clone() });
        }
    } }
    out
}

/// One cell of a repeat: its chips in cell coordinates, centre (0,0), side 2h.
pub fn repeat_unit(kind: Repeat, h: f64) -> Vec<Chip> {
    if kind == Repeat::Pinwheel {
        // one triangle per cell edge, C to the edge; the cut chip runs from the edge's
        // start to a point 60% along it, so each cell turns the same way
        let (a0, b0, c) = (pt(-h, -h), pt(h, -h), pt(0.0, 0.0));
        let x = lerp(a0, b0, 0.6);
        let chip = Chip { outline: vec![a0, x, c], floor: vec![pt((a0.x + x.x + c.x) / 3.0, (a0.y + x.y + c.y) / 3.0)], corners: vec![0, 1, 2] };
        (0..4).map(|k| chip.turned(c, k)).collect()
    } else {
        let base = kind.octant(h);
        // mirrored across the N–C diagonal: the octant's twin on the other side of it
        let mirror = |ch: &Chip| { let f = |p: &Point| pt(p.y, p.x); Chip { outline: ch.outline.iter().map(f).collect(), floor: ch.floor.iter().map(f).collect(), corners: ch.corners.clone() } };
        let pair: Vec<Chip> = base.iter().cloned().chain(base.iter().map(mirror)).collect();
        (0..4).flat_map(|k| pair.iter().map(move |ch| ch.turned(pt(0.0, 0.0), k)).collect::<Vec<_>>()).collect()
    }
}

/// A repeat cell bent onto a curved patch: `map` takes (u, v) in [0,1]² (u across
/// the cell, v down it) to the page. Each straight edge is split into `k` pieces so
/// it follows the bend; neighbouring cells share their edges exactly.
pub fn warp_unit(unit: &[Chip], h: f64, k: usize, map: &dyn Fn(f64, f64) -> Point) -> Vec<Chip> {
    let to = |p: Point| map((p.x + h) / (2.0 * h), (p.y + h) / (2.0 * h));
    unit.iter().map(|ch| {
        let n = ch.outline.len();
        let mut outline = vec![];
        for i in 0..n { let (a, b) = (ch.outline[i], ch.outline[(i + 1) % n]); for j in 0..k { outline.push(to(lerp(a, b, j as f64 / k as f64))); } }
        Chip { outline, floor: ch.floor.iter().map(|p| to(*p)).collect(), corners: ch.corners.iter().map(|i| i * k).collect() }
    }).collect()
}

/// How a fill treats chips that cross its edge.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FillEdge { Clip, Whole }

/// Fill a freehand region with a repeat. The lattice is anchored to the page
/// origin, so fills with the same cell size line up. The region is inset by
/// `margin` and keeps `margin` of uncut wood around every chip in `avoid`.
/// Clip: chips are cut to that shape (slivers dropped; a clipped chip keeps its
/// deep point when it's still inside). Whole: only complete chips are kept.
pub fn fill_region(kind: Repeat, cell: f64, lasso: &[Point], margin: f64, edge: FillEdge, avoid: &[&[Point]]) -> Vec<Chip> {
    if lasso.len() < 3 || cell <= 0.0 { return vec![]; }
    let allowed = fill_allowed(lasso, margin, avoid);
    let pts: Vec<Point> = allowed.iter().flatten().flatten().copied().collect();
    if pts.is_empty() { return vec![]; }
    let b = crate::geometry::Bounds::of(&pts);
    let (i0, j0) = ((b.l / cell).floor(), (b.t / cell).floor());
    let (nx, ny) = (((b.r / cell).ceil() - i0) as usize, ((b.b / cell).ceil() - j0) as usize);
    repeat_field(kind, pt(i0 * cell, j0 * cell), cell, nx, ny).into_iter().flat_map(|chip| clip_chip(chip, &allowed, edge)).collect()
}

/// Where a fill may cut: the outline inset by `margin`, minus every chip in `avoid`
/// grown by `margin`. An existing motif is avoided as a whole: gaps of up to 10 mm
/// between its chips are closed first, so a fill can't seep into the uncut wood inside a rosette.
pub fn fill_allowed(lasso: &[Point], margin: f64, avoid: &[&[Point]]) -> crate::booleans::Shapes {
    use crate::booleans::{clean_with, difference, offset, union};
    const GAP: f64 = 5.0;
    let region = offset(&clean_with(lasso, false), -margin);
    if region.is_empty() || avoid.is_empty() { return region; }
    difference(&region, &offset(&crate::booleans::close(&union(avoid), GAP), margin))
}

/// A chip fitted to `allowed`: kept as it is when it lies inside; otherwise dropped
/// (Whole) or cut to it (Clip), keeping only substantial pieces (most of the chip, and
/// not a thin shard) so an edge doesn't fill with fragments. A clipped piece keeps its
/// deep point when that's still inside.
pub fn clip_chip(chip: Chip, allowed: &crate::booleans::Shapes, edge: FillEdge) -> Vec<Chip> {
    use crate::booleans::{area, intersect, signed_area};
    let full = signed_area(&chip.outline).abs();
    let pts: Vec<Point> = allowed.iter().flatten().flatten().copied().collect();
    if pts.is_empty() { return vec![]; }
    let (b, cb) = (crate::geometry::Bounds::of(&pts), crate::geometry::Bounds::of(&chip.outline));
    if !(cb.l < b.r && cb.r > b.l && cb.t < b.b && cb.b > b.t) { return vec![]; }
    let kept = intersect(&vec![vec![chip.outline.clone()]], allowed);
    if area(&kept) >= full * 0.999 { return vec![chip]; }
    if edge == FillEdge::Whole { return vec![]; }
    let mut out = vec![];
    // a piece with a hole would wrap round something it must avoid: drop it
    for piece in kept.iter().filter(|s| s.len() == 1).map(|s| &s[0]) {
        let a = signed_area(piece).abs();
        let perimeter: f64 = (0..piece.len()).map(|k| distance(piece[k], piece[(k + 1) % piece.len()])).sum();
        if a < (full * 0.45).max(1.5) || 2.0 * a / perimeter < 0.8 { continue; }
        let mut c = from_outline(piece);
        if chip.floor.len() == 1 && crate::outline::inside(chip.floor[0], piece) { c.floor = chip.floor.clone(); }
        out.push(c);
    }
    out
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
