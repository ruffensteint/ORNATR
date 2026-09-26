//! Scroll curls grown along a curve: the Scroll vine backbone mode.
//! Every curl is a clean scroll: it leaves its stem tangentially on a wide
//! sweep and tightens smoothly into a volute, ending where its own turns
//! would crowd. Only the arrangement grows like a vine:
//! 1. seed points along the curve (jittered spacing) each get the largest
//!    scroll that fits, on the side with more room;
//! 2. smaller scrolls sprout from the outside of each scroll's sweep and curl
//!    the other way, a generation at a time;
//! 3. the largest gaps left are filled by scrolls branching from the nearest
//!    stem or scroll;
//! 4. a boolean check fits every scroll's band inside the carving surface.
//! Nothing comes closer than `clearance` to anything else.
use crate::booleans::{area, difference, union};
use crate::geometry::{distance, pt, Bounds, Point};
use crate::growth::{ribbon, Mulberry};
use crate::outline::inside;
use std::collections::HashMap;
use std::f64::consts::PI;

#[derive(Clone, Copy, Debug)]
pub struct ScrollOptions {
    /// Roughly this far apart along the curve, mm (jittered).
    pub seed_spacing: f64,
    /// Longest and shortest scroll (along its spine), mm.
    pub max_length: f64, pub min_length: f64,
    /// Band width at a scroll's root, mm (it tapers to the eye).
    pub width: f64,
    /// Least gap between bands, mm.
    pub clearance: f64,
    /// Branch generations (1 = only scrolls on the curve).
    pub generations: u32,
    /// Only scrolls at least this long (mm) sprout branches.
    pub branch_from: f64,
    /// Gaps are filled until the largest empty circle left is smaller than
    /// this radius, mm.
    pub fill_gap: f64,
    pub seed: u32,
}
impl Default for ScrollOptions {
    fn default() -> Self { ScrollOptions { seed_spacing: 45.0, max_length: 150.0, min_length: 22.0, width: 5.0, clearance: 3.0, generations: 3, branch_from: 0.0, fill_gap: 0.0, seed: 1 } }
}

/// A grown scroll. `parent` is what it branches from: 0 is the curve, i + 1 is scroll i.
pub struct Scroll {
    pub spine: Vec<Point>, pub band: Vec<Point>, pub generation: u32, pub root: Point, pub parent: usize,
    /// Its name, and the scroll it grows from (None: the curve).
    pub id: String, pub parent_id: Option<String>,
    /// How it is made, so it can be edited and rebuilt: where it leaves its
    /// parent (0 to 1 along it), its heading there relative to the parent
    /// (radians), which way it turns (+1 or -1), its full length (mm), the
    /// share of that spine kept (where its eye closes) and its leaf's width.
    pub at: f64, pub turn: f64, pub side: f64, pub full: f64, pub roll: f64, pub leaf: f64,
}
pub struct ScrollResult { pub curve: Vec<Point>, pub scrolls: Vec<Scroll>, pub outside: f64 }

/// A clean scroll spine: from `root` heading `h`, turning to `side` (+1 or
/// -1). Its curvature grows steadily (like a clothoid) from a wide sweep
/// (radius half the length L) to an open eye (radius 0.08 L): about one
/// loose turn and a bit, as a carver draws a C-scroll.
pub fn scroll_spine(root: Point, h: f64, side: f64, length: f64) -> Vec<Point> {
    let ds = 0.5; let n = (length / ds).ceil() as usize;
    let (k0, k1) = (1.0 / (0.5 * length), 1.0 / (0.08 * length));
    let mut pts = vec![root]; let (mut p, mut a) = (root, h);
    for i in 1..=n {
        let s = i as f64 / n as f64;
        a += side * (k0 + (k1 - k0) * s) * ds;
        p = pt(p.x + a.cos() * ds, p.y + a.sin() * ds); pts.push(p);
    }
    pts
}

/// Band half-width along a spine of `n` points (as `ribbon` draws it).
fn half_width(width: f64, i: usize, n: usize) -> f64 { width * (0.06 + 0.94 * (1.0 - i as f64 / (n as f64 - 1.0)).powf(0.75)) / 2.0 }

/// Everything placed so far, as spine points with their band half-width.
struct Placed { cell: f64, grid: HashMap<(i64, i64), Vec<(Point, f64, usize)>> }
impl Placed {
    fn key(&self, p: Point) -> (i64, i64) { ((p.x / self.cell).floor() as i64, (p.y / self.cell).floor() as i64) }
    fn add(&mut self, pts: &[Point], widths: impl Fn(usize) -> f64, owner: usize) { for (i, p) in pts.iter().enumerate() { let k = self.key(*p); self.grid.entry(k).or_default().push((*p, widths(i), owner)); } }
    /// Whether `p` (with half-width `w`) keeps `gap` from everything, except
    /// the stem it grows from near its root.
    fn clear(&self, p: Point, w: f64, gap: f64, except: Option<(usize, Point, f64)>) -> bool {
        let (cx, cy) = self.key(p); let n = ((gap + w + 6.0) / self.cell).ceil() as i64;
        for x in cx - n..=cx + n { for y in cy - n..=cy + n {
            let Some(v) = self.grid.get(&(x, y)) else { continue };
            for (q, qw, owner) in v {
                if let Some((stem, at, r)) = except { if *owner == stem && distance(*q, at) < r { continue; } }
                if distance(p, *q) < gap + w + qw { return false; }
            }
        }}
        true
    }
    /// Distance from `p` to the nearest band edge (capped).
    fn room(&self, p: Point, cap: f64) -> f64 {
        let (cx, cy) = self.key(p); let n = (cap / self.cell).ceil() as i64; let mut best = cap;
        for x in cx - n..=cx + n { for y in cy - n..=cy + n { if let Some(v) = self.grid.get(&(x, y)) { for (q, qw, _) in v { best = best.min(distance(p, *q) - qw); } } } }
        best
    }
}

/// Owner id of obstacle points (parts of other backbones).
const OBSTACLE: usize = usize::MAX - 1;

struct Ctx<'a> { surface: &'a [Point], o: ScrollOptions, placed: Placed, scrolls: Vec<Scroll>, stems: Vec<Vec<Point>>, obstacles: Vec<(Bounds, Vec<Point>)> }
impl Ctx<'_> {
    /// Whether `p` lies inside one of the obstacles.
    fn in_obstacle(&self, p: Point) -> bool { self.obstacles.iter().any(|(b, poly)| b.contains(p) && inside(p, poly)) }
    fn edge(&self, p: Point) -> f64 {
        let s = self.surface; let n = s.len();
        (0..n).map(|i| { let (a, b) = (s[i], s[(i + 1) % n]); let (dx, dy) = (b.x - a.x, b.y - a.y); let l2 = dx * dx + dy * dy;
            let t = if l2 > 0.0 { (((p.x - a.x) * dx + (p.y - a.y) * dy) / l2).clamp(0.0, 1.0) } else { 0.0 };
            (p.x - a.x - t * dx).hypot(p.y - a.y - t * dy) }).fold(f64::INFINITY, f64::min)
    }
    /// The spine cut where its own turns would crowd (the eye closes there).
    fn rolled(&self, spine: Vec<Point>) -> Vec<Point> {
        let n = spine.len(); let gap = self.o.clearance;
        for i in 1..n {
            let wi = half_width(self.o.width, i, n);
            // earlier points more than half a turn back along the spine
            let close = (0..i).rev().skip((PI * 2.0 * wi.max(gap) / 0.5) as usize + 8).step_by(2).any(|j| distance(spine[i], spine[j]) < gap + wi + half_width(self.o.width, j, n));
            if close { return spine[..i].to_vec(); }
        }
        spine
    }
    /// The largest scroll from `root` heading `h` curling to `side` that keeps
    /// clear of everything and inside the surface, shortest `min_length`.
    /// The spine and its full length.
    fn fit(&self, root: Point, h: f64, side: f64, max_len: f64, stem: usize) -> Option<(Vec<Point>, f64)> {
        let mut len = max_len;
        while len >= self.o.min_length {
            let spine = self.rolled(scroll_spine(root, h, side, len));
            let n = spine.len();
            if n as f64 * 0.5 >= len * 0.6 {
                let ok = spine.iter().enumerate().all(|(i, p)| {
                    let w = half_width(self.o.width, i, n);
                    inside(*p, self.surface) && self.edge(*p) >= w + self.o.clearance && !self.in_obstacle(*p)
                        && (i < 4 || self.placed.clear(*p, w, self.o.clearance, Some((stem, root, (self.o.width + self.o.clearance) * 3.5))))
                });
                if ok { return Some((spine, len)); }
            }
            len *= 0.9;
        }
        None
    }
    /// Place a scroll grown from stem `parent` at its point `at_index`,
    /// heading `h` and turning to `side`, full length `full`.
    #[allow(clippy::too_many_arguments)]
    fn place(&mut self, spine: Vec<Point>, full: f64, generation: u32, parent: usize, at_index: usize, h: f64, side: f64) -> usize {
        let n = spine.len(); let w = self.o.width;
        let id = self.stems.len();
        self.placed.add(&spine, |i| half_width(w, i, n), id);
        let ps = &self.stems[parent];
        let at = at_index as f64 / (ps.len() as f64 - 1.0).max(1.0);
        let t = tangent_at(ps, at_index);
        let turn = (h - t).sin().atan2((h - t).cos());
        let full_n = (full / 0.5).ceil() + 1.0;
        let parent_id = if parent == 0 { None } else { Some(self.scrolls[parent - 1].id.clone()) };
        self.scrolls.push(Scroll { band: ribbon(&spine, w), root: spine[0], spine: spine.clone(), generation, parent, id: format!("scroll-{}", id - 1), parent_id,
            at, turn, side, full, roll: (n as f64 / full_n).min(1.0), leaf: 1.0 });
        self.stems.push(spine);
        id
    }
    /// Which side of a stem at `p` (tangent `t`) has more room.
    fn roomier(&self, p: Point, t: f64, rng: &mut Mulberry) -> f64 {
        let probe = |s: f64| { let n = t + s * PI / 2.0; let q = pt(p.x + n.cos() * 12.0, p.y + n.sin() * 12.0); if !inside(q, self.surface) { -1.0 } else { self.placed.room(q, 60.0).min(self.edge(q)) } };
        let (a, b) = (probe(1.0), probe(-1.0));
        if (a - b).abs() < 3.0 { if rng.next() < 0.5 { 1.0 } else { -1.0 } } else if a > b { 1.0 } else { -1.0 }
    }
}

fn tangent_at(pts: &[Point], i: usize) -> f64 { let (a, b) = (pts[i.saturating_sub(1)], pts[(i + 1).min(pts.len() - 1)]); (b.y - a.y).atan2(b.x - a.x) }

/// Grow scrolls from seed points along `curve` inside `surface`.
/// `obstacles` are outlines already on the page (other backbones' parts): no
/// scroll grows inside one or comes closer than the clearance.
pub fn grow_scrolls(curve: &[Point], closed: bool, surface: &[Point], obstacles: &[Vec<Point>], o: &ScrollOptions) -> ScrollResult {
    let mut rng = Mulberry(o.seed.wrapping_mul(2654435761) ^ 0x9e3779b9);
    // the curve, evenly resampled
    let mut pts = curve.to_vec(); if closed { pts.push(curve[0]); }
    let mut path = vec![pts[0]]; let mut carry = 0.0; let step = 0.5;
    for w in pts.windows(2) { let l = distance(w[0], w[1]); let mut s = step - carry; while s <= l { path.push(pt(w[0].x + (w[1].x - w[0].x) * s / l, w[0].y + (w[1].y - w[0].y) * s / l)); s += step; } carry = l - (s - step); }
    let mut ctx = Ctx { surface, o: *o, placed: Placed { cell: 4.0, grid: HashMap::new() }, scrolls: vec![], stems: vec![], obstacles: obstacles.iter().filter(|p| p.len() >= 3).map(|p| (Bounds::of(p), p.clone())).collect() };
    let stem_w = o.width * 0.6;
    ctx.placed.add(&path, |_| stem_w, 0); ctx.stems.push(path.clone());
    // obstacle outlines, a point every millimetre, so the clearance holds against them
    for poly in obstacles { let n = poly.len(); if n < 3 { continue; } let mut dense = vec![];
        for i in 0..n { let (a, c) = (poly[i], poly[(i + 1) % n]); let k = (distance(a, c) / 1.0).ceil().max(1.0) as usize; for j in 0..k { let t = j as f64 / k as f64; dense.push(pt(a.x + (c.x - a.x) * t, a.y + (c.y - a.y) * t)); } }
        ctx.placed.add(&dense, |_| 0.0, OBSTACLE); }
    // 1. scrolls on the curve: the largest that fits at each jittered seed,
    // leaving along the curve and leaning to the roomier side
    let total = step * (path.len() as f64 - 1.0);
    let mut s = o.seed_spacing * (0.3 + rng.next() * 0.4);
    let mut frontier: Vec<usize> = vec![];
    while s < total - if closed { o.seed_spacing * 0.4 } else { 10.0 } {
        let i = ((s / step) as usize).min(path.len() - 1);
        let (p, t) = (path[i], tangent_at(&path, i));
        let side = ctx.roomier(p, t, &mut rng);
        let forward = if rng.next() < 0.5 { t } else { t + PI };
        // the scroll turns toward its side, away from the stem
        let turn = (t + side * PI / 2.0 - forward).sin().signum();
        let h = forward + turn * 0.25;
        let max_len = o.max_length * (0.7 + rng.next() * 0.3);
        if let Some((sp, full)) = ctx.fit(p, h, turn, max_len, 0) { let id = ctx.place(sp, full, 1, 0, i, h, turn); frontier.push(id); }
        s += o.seed_spacing * (0.7 + rng.next() * 0.6);
    }
    // 2. branches: smaller scrolls from the outside of each scroll's sweep,
    // curling the other way
    for gen in 2..=o.generations {
        let mut next = vec![];
        for &id in &frontier {
            let spine = ctx.stems[id].clone(); let n = spine.len();
            let parent_len = n as f64 * step;
            if parent_len < o.branch_from { continue; }
            // the scroll's own turn direction, from its first stretch
            let turn = { let a = tangent_at(&spine, 2); let b = tangent_at(&spine, n / 4); (b - a).sin().signum() };
            for frac in [0.22 + rng.next() * 0.1, 0.45 + rng.next() * 0.1] {
                let i = ((n as f64 * frac) as usize).min(n - 2);
                let t = tangent_at(&spine, i);
                let h = t - turn * (0.22 + rng.next() * 0.12);
                let max_len = parent_len * (0.45 + rng.next() * 0.15);
                if max_len < o.min_length { continue; }
                if let Some((sp, full)) = ctx.fit(spine[i], h, -turn, max_len, id) { let nid = ctx.place(sp, full, gen, id, i, h, -turn); next.push(nid); }
            }
        }
        frontier = next;
    }
    // 3. gaps: the largest empty spaces get a scroll branching from the stem
    // or scroll point nearest them
    let b = Bounds::of(surface);
    for _ in 0..40 {
        let Some((gap, r)) = largest_gap(&ctx, b) else { break };
        if r < (o.min_length * 0.35).max(o.fill_gap) { break; }
        let mut done = false;
        // nearest stem points, nearest first
        let mut cands: Vec<(f64, usize, usize)> = vec![];
        for (sid, st) in ctx.stems.iter().enumerate() { for (i, q) in st.iter().enumerate().step_by(6) { let d = distance(*q, gap); if d < r * 3.0 { cands.push((d, sid, i)); } } }
        cands.sort_by(|a, c| a.0.partial_cmp(&c.0).unwrap());
        for (_, sid, i) in cands.into_iter().take(12) {
            let st = ctx.stems[sid].clone(); let t = tangent_at(&st, i); let p = st[i];
            let to = (gap.y - p.y).atan2(gap.x - p.x);
            for forward in [t, t + PI] {
                let turn = (to - forward).sin().signum();
                let h = forward + turn * 0.3;
                let max_len = (r * 3.2).min(o.max_length * 0.6);
                if let Some((sp, full)) = ctx.fit(p, h, turn, max_len, sid) { ctx.place(sp, full, 3, sid, i, h, turn); done = true; break; }
            }
            if done { break; }
        }
        if !done { ctx.placed.add(&[gap], |_| r * 0.9, usize::MAX); } // mark it, move on
    }
    // 4. the boolean check against the carving surface
    let bands: Vec<&[Point]> = ctx.scrolls.iter().map(|s| s.band.as_slice()).collect();
    let outside = area(&difference(&union(&bands), &union(&[surface])));
    ScrollResult { curve: path, scrolls: ctx.scrolls, outside }
}

/// Centre and radius of the largest empty circle (sampled on a grid).
fn largest_gap(ctx: &Ctx, b: Bounds) -> Option<(Point, f64)> {
    let h = 3.0; let mut best: Option<(Point, f64)> = None;
    let mut y = b.t + h;
    while y < b.b { let mut x = b.l + h; while x < b.r {
        let p = pt(x, y);
        if inside(p, ctx.surface) && !ctx.in_obstacle(p) {
            let r = ctx.edge(p).min(ctx.placed.room(p, 80.0)) - ctx.o.clearance;
            if best.map_or(true, |bb| r > bb.1) { best = Some((p, r)); }
        }
        x += h; } y += h; }
    best
}

/// How grown curls are dressed as carvable scroll shapes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Dress {
    /// A smooth band tapering from the stem's width at the fork to a slim eye.
    Carved,
    /// The band swells through the belly and ends in a round rolled eye.
    Rolled,
    /// Clad with the acanthus leaf on the outer side of the curl.
    Acanthus,
    /// Acanthus on the first curls from the curve, carved bands on branches.
    Hierarchy,
}

/// Which way a spine turns (+1 or -1), from its heading change.
fn turn_of(spine: &[Point]) -> f64 {
    let n = spine.len(); if n < 6 { return 1.0; }
    let a = (spine[1].y - spine[0].y).atan2(spine[1].x - spine[0].x);
    let b = (spine[n / 3].y - spine[n / 3 - 1].y).atan2(spine[n / 3].x - spine[n / 3 - 1].x);
    (b - a).sin().signum()
}

/// A band round `spine` whose half-width at t (0 root, 1 eye) is `half(t)`.
fn band_with(spine: &[Point], half: impl Fn(f64) -> f64) -> Vec<Point> {
    let n = spine.len(); let (mut left, mut right) = (vec![], vec![]);
    for (i, p) in spine.iter().enumerate() {
        let (a, b) = (spine[i.saturating_sub(1)], spine[(i + 1).min(n - 1)]); let ang = (b.y - a.y).atan2(b.x - a.x);
        let w = half(i as f64 / (n as f64 - 1.0).max(1.0));
        left.push(pt(p.x - ang.sin() * w, p.y + ang.cos() * w)); right.push(pt(p.x + ang.sin() * w, p.y - ang.cos() * w));
    }
    right.reverse(); left.extend(right); left
}

fn part(id: String, parent: Option<String>, kind: crate::growth::Kind, spine: &[Point], polygon: Vec<Point>, folds: Vec<Vec<Point>>, width: f64) -> crate::growth::GrowthPart {
    crate::growth::GrowthPart { id, parent, kind, length: crate::geometry::line_length(spine), points: spine.to_vec(), polygon, folds, ridges: None, cuts: vec![], width, birth: 0.0, duration: 1.0, contour_split: None, shoot: None, under: false }
}

/// Dress grown curls as scroll parts: the curve becomes the stem (`stem` mm
/// wide), each curl a part growing from its parent, ready for the drawing
/// engines (the exact joins blend every fork).
pub fn dress(res: &ScrollResult, style: Dress, stem: f64) -> crate::growth::GrowthResult {
    use crate::contour::{acanthus_contour, leaf_notches, ContourOptions, OPEN_ENDS};
    use crate::growth::Kind;
    let curve = &res.curve;
    let stem_poly = band_with(curve, |t| stem / 2.0 * (0.55 + 0.45 * (PI * t).sin().powf(0.3)));
    let mut parts = vec![part("curve".into(), None, Kind::Primary, curve, stem_poly, vec![], stem)];
    for (i, s) in res.scrolls.iter().enumerate() {
        let sp = &s.spine; if sp.len() < 8 { continue; }
        let len = crate::geometry::line_length(sp);
        let w0 = (stem * 0.9 * 0.8f64.powi(s.generation as i32 - 1)).min(len * 0.12);
        let turn = turn_of(sp);
        let acanthus = style == Dress::Acanthus || (style == Dress::Hierarchy && s.generation == 1);
        let (polygon, folds) = if acanthus {
            let o = ContourOptions { lobed: true, root_width: w0 * 0.5, stalk: 0.1, notches: leaf_notches(), ends: Some(OPEN_ENDS), ..ContourOptions::default() };
            let a = acanthus_contour(sp, w0 * 0.55, -turn, (len * 0.2).min(stem * 3.2), &o);
            (a.polygon, a.folds)
        } else if style == Dress::Rolled {
            // swell through the belly, then a round rolled eye
            let body = band_with(sp, |t| w0 / 2.0 * (1.0 + 0.35 * (PI * t).sin()) * (1.0 - 0.5 * t));
            let e = *sp.last().unwrap(); let r = (w0 * 0.62).min(len * 0.05);
            let button: Vec<Point> = (0..32).map(|k| { let a = k as f64 / 32.0 * 2.0 * PI; pt(e.x + r * a.cos(), e.y + r * a.sin()) }).collect();
            let merged = union(&[body.as_slice(), button.as_slice()]);
            (merged.into_iter().max_by(|a, b| area(&vec![a.clone()]).partial_cmp(&area(&vec![b.clone()])).unwrap()).map(|s| s[0].clone()).unwrap_or(body), vec![])
        } else {
            (band_with(sp, |t| w0 / 2.0 * (1.0 - 0.72 * t.powf(0.8))), vec![])
        };
        let _ = i;
        let mut p = part(s.id.clone(), Some(s.parent_id.clone().unwrap_or_else(|| "curve".into())), if s.generation == 1 { Kind::Primary } else { Kind::Secondary }, sp, polygon, folds, w0);
        p.shoot = Some(vine_shoot(s, crate::geometry::line_length(curve)));
        parts.push(p);
    }
    crate::growth::GrowthResult { parts, ..Default::default() }
}

/// Dress grown curls as acanthus-clad scrolls: each curl sheathed on its outer
/// side by the leaf (`spec`), reaching `reach` × its length at most, and no
/// more than `max_half` mm from the spine; the curve stays a plain stem.
pub fn dress_acanthus(res: &ScrollResult, surface: &[Point], obstacles: &[Vec<Point>], stem: f64, spec: &crate::acanthus::LeafSpec, reach: f64, max_half: f64) -> crate::growth::GrowthResult {
    let surf = union(&[surface]);
    use crate::growth::Kind;
    let curve = &res.curve;
    // the stem tapers toward both ends and finishes round
    let body = band_with(curve, |t| stem / 2.0 * (0.3 + 0.7 * (PI * t).sin().powf(0.35)));
    let caps: Vec<Vec<Point>> = [curve[0], *curve.last().unwrap()].iter().map(|e| (0..24).map(|k| { let a = k as f64 / 24.0 * 2.0 * PI; pt(e.x + stem * 0.15 * a.cos(), e.y + stem * 0.15 * a.sin()) }).collect()).collect();
    let stem_poly = union(&[body.as_slice(), caps[0].as_slice(), caps[1].as_slice()]).into_iter().max_by(|a, b| area(&vec![a.clone()]).partial_cmp(&area(&vec![b.clone()])).unwrap()).map(|s| s[0].clone()).unwrap_or(body);
    // what is already dressed, by id, for the crowding check; other backbones'
    // parts count as always there
    let stem_len = crate::geometry::line_length(curve);
    let mut dressed: Vec<(String, crate::booleans::Shapes)> = vec![("curve".into(), union(&[stem_poly.as_slice()]))];
    for o in obstacles.iter().filter(|o| o.len() >= 3) { dressed.push((String::new(), union(&[o.as_slice()]))); }
    let mut parts = vec![part("curve".into(), None, Kind::Primary, curve, stem_poly, vec![], stem)];
    for (i, s) in res.scrolls.iter().enumerate() {
        let sp = &s.spine; if sp.len() < 8 { continue; }
        let len = crate::geometry::line_length(sp);
        let w0 = (stem * 0.9 * 0.8f64.powi(s.generation as i32 - 1)).min(len * 0.12);
        // the leaf goes on the outside of the turn
        let mut half = (len * reach).min(max_half) * s.leaf.clamp(0.3, 2.0);
        // small curls carry a single lobe group, so they do not crowd
        let own = crate::acanthus::LeafSpec { groups: if len < 90.0 { 1 } else { spec.groups }, ..*spec };
        let mut leaf = crate::acanthus::clad_scroll(sp, &own, -turn_of(sp), half, w0 / 2.0);
        // the boolean checks on the real shape: a leaf reaching out of the
        // surface, or onto a neighbour (its own parent excepted), is narrowed
        // until it fits
        let parent = s.parent_id.clone().unwrap_or_else(|| "curve".into());
        let _ = i;
        for _ in 0..5 {
            let shape = union(&[leaf.polygon.as_slice()]);
            let outside = area(&difference(&shape, &surf));
            let crowding: f64 = dressed.iter().filter(|(id, _)| *id != parent).map(|(_, other)| area(&crate::booleans::intersect(&shape, other))).sum();
            if outside < 0.5 && crowding < 1.0 { break; }
            half *= 0.85; leaf = crate::acanthus::clad_scroll(sp, &own, -turn_of(sp), half, w0 / 2.0);
        }
        dressed.push((s.id.clone(), union(&[leaf.polygon.as_slice()])));
        let mut p = part(s.id.clone(), Some(parent), if s.generation == 1 { Kind::Primary } else { Kind::Secondary }, sp, leaf.polygon, leaf.folds, w0);
        p.cuts = leaf.cuts;
        p.shoot = Some(vine_shoot(s, stem_len));
        parts.push(p);
    }
    crate::growth::GrowthResult { parts, ..Default::default() }
}

/// The chosen acanthus scroll dress (the user's pick, 2026-09-26): the broad
/// belly (leaf reaching 0.17 of the curl's length, at most 15 mm from the
/// spine) with two fingers per lobe. Returns the leaf, its reach and its
/// largest half-width in mm.
pub fn scroll_leaf() -> (crate::acanthus::LeafSpec, f64, f64) {
    (crate::acanthus::LeafSpec { groups: 2, fingers: 2, notch: 0.2, eye: 0.38, cut: 0.62, stalk: 0.2, ..crate::acanthus::LeafSpec::default() }, 0.17, 15.0)
}

/// A curl's making as leaf settings (preset `VINE_CURL`), so the app edits it
/// like a library leaf: `progress` is where it leaves its parent, `turn` its
/// heading relative to the parent there, `reach` its full length as a share
/// of the stem, `curl` the share of that length kept (where the eye closes),
/// `side` which way it turns, `leaf_scale` its leaf's width, `on` its parent.
pub fn vine_shoot(s: &Scroll, stem_len: f64) -> crate::shoots::ShootParams {
    crate::shoots::ShootParams { progress: s.at, reach: s.full / stem_len.max(1.0), turn: s.turn, curl: s.roll, side: s.side, leaf_scale: Some(s.leaf),
        preset: Some(crate::shoots::VINE_CURL.into()), on: s.parent_id.clone(), ..Default::default() }
}

/// Rebuild curls exactly from their settings (edited curls), in the order
/// they grow from each other: `edits` are (id, settings, hidden). A hidden
/// curl is built, so curls growing from it keep their place, but not drawn.
pub fn build_scrolls(curve: &[Point], edits: &[(String, crate::shoots::ShootParams, bool)], width: f64) -> ScrollResult {
    let mut path = vec![curve[0]]; let mut carry = 0.0; let step = 0.5;
    for w in curve.windows(2) { let l = distance(w[0], w[1]); let mut s = step - carry; while s <= l { path.push(pt(w[0].x + (w[1].x - w[0].x) * s / l, w[0].y + (w[1].y - w[0].y) * s / l)); s += step; } carry = l - (s - step); }
    let stem_len = crate::geometry::line_length(&path);
    let ids: Vec<&String> = edits.iter().map(|e| &e.0).collect();
    let mut built: HashMap<String, (Vec<Point>, u32)> = HashMap::new();
    let mut scrolls = vec![];
    let mut pending: Vec<usize> = (0..edits.len()).collect();
    // a curl is built once the curl it grows from is (a missing parent means the stem)
    for _ in 0..=edits.len() {
        if pending.is_empty() { break; }
        let mut left = vec![];
        for k in pending {
            let (id, p, hidden) = &edits[k];
            let parent = p.on.as_ref().filter(|o| ids.contains(o));
            let (ps, gen) = match parent { None => (&path, 0), Some(o) => match built.get(o) { Some((s, g)) => (s, *g), None => { left.push(k); continue; } } };
            let at = ((p.progress.clamp(0.0, 1.0) * (ps.len() as f64 - 1.0)).round() as usize).min(ps.len() - 1);
            let h = tangent_at(ps, at) + p.turn;
            let full = (p.reach * stem_len).max(5.0);
            let mut spine = scroll_spine(ps[at], h, if p.side >= 0.0 { 1.0 } else { -1.0 }, full);
            let keep = ((p.curl.clamp(0.05, 1.0) * spine.len() as f64).round() as usize).clamp(8, spine.len());
            spine.truncate(keep);
            if !hidden {
                scrolls.push(Scroll { band: ribbon(&spine, width), root: spine[0], spine: spine.clone(), generation: gen + 1, parent: 0, id: id.clone(), parent_id: parent.cloned(),
                    at: p.progress, turn: p.turn, side: p.side, full, roll: p.curl, leaf: p.leaf_scale.unwrap_or(1.0) });
            }
            built.insert(id.clone(), (spine, gen + 1));
        }
        pending = left;
    }
    ScrollResult { curve: path, scrolls, outside: 0.0 }
}
