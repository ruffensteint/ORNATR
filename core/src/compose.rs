//! Layered chip compositions (study stage, not used by the app yet), after the
//! user's reference carvings: raised faceted stars with fans filling their gaps,
//! ring bands (grooves, small stars, flutes, diamonds), compass-arc rosettes
//! whose cells are cut in a checkerboard, and box-lid layouts with corner fans.
use crate::booleans::{difference, intersect, signed_area};
use crate::facets::{centroid, polar, tri, Chip};
use crate::geometry::{distance, lerp, pt, Point};
use std::f64::consts::PI;

// ---------------------------------------------------------------- building blocks

/// One face of a raised point: it slopes from the ridge (left at the surface)
/// down to `edge` (the deep side). The outline runs ridge, then edge.
pub fn bevel(ridge: &[Point], edge: &[Point]) -> Chip {
    let mut outline = ridge.to_vec();
    outline.extend(edge.iter().rev().copied());
    dedupe(&mut outline);
    Chip { outline, floor: edge.to_vec(), corners: vec![] }
}

/// A raised star point from `base` (on the axis) to `tip`, with flanks from `left`
/// and `right` to the tip (sampled paths): two bevels meeting on the axis ridge.
pub fn raised_point(base: Point, tip: Point, left: &[Point], right: &[Point]) -> [Chip; 2] {
    let mut l = vec![base]; l.extend(left.iter().copied());
    let mut r = vec![base]; r.extend(right.iter().copied());
    [bevel(&[base, tip], &l), bevel(&[base, tip], &r)]
}

/// Quadratic Bézier from `a` to `b` bowed toward `toward` by `k` (0 = straight), `n` segments.
pub fn bow(a: Point, b: Point, toward: Point, k: f64, n: usize) -> Vec<Point> {
    let m = lerp(a, b, 0.5); let ctl = lerp(m, toward, k);
    (0..=n).map(|i| { let t = i as f64 / n as f64; let (u, v) = (lerp(a, ctl, t), lerp(ctl, b, t)); lerp(u, v, t) }).collect()
}

/// A fan of narrow chips from `apex` to consecutive points of `path`, all cut,
/// so knife-edge ridges radiate between them.
pub fn fan(apex: Point, path: &[Point]) -> Vec<Chip> {
    path.windows(2).filter(|w| distance(w[0], w[1]) > 1e-6).map(|w| tri(apex, w[0], w[1])).collect()
}

/// Evenly spaced points along a polyline (by length), `n` segments.
pub fn resample(path: &[Point], n: usize) -> Vec<Point> {
    let total: f64 = path.windows(2).map(|w| distance(w[0], w[1])).sum();
    let mut out = vec![path[0]]; let (mut acc, mut k) = (0.0, 0);
    for i in 1..n {
        let want = total * i as f64 / n as f64;
        while k + 1 < path.len() - 1 && acc + distance(path[k], path[k + 1]) < want { acc += distance(path[k], path[k + 1]); k += 1; }
        let seg = distance(path[k], path[k + 1]).max(1e-12);
        out.push(lerp(path[k], path[k + 1], ((want - acc) / seg).clamp(0.0, 1.0)));
    }
    out.push(*path.last().unwrap());
    out
}

/// A small flat star of split kites: `n` points, tips at `rt`, shoulders at `rs`.
pub fn small_star(c: Point, rt: f64, rs: f64, n: usize, phase: f64) -> Vec<Chip> {
    let step = 2.0 * PI / n as f64; let mut out = vec![];
    for k in 0..n {
        let a = phase + k as f64 * step;
        let tip = polar(c, rt, a);
        out.push(tri(c, polar(c, rs, a - step / 2.0), tip)); out.push(tri(c, tip, polar(c, rs, a + step / 2.0)));
    }
    out
}

/// An incised ring: a V-groove of width `w` on radius `r`, as short two-cut segments.
pub fn groove_ring(c: Point, r: f64, w: f64, n: usize) -> Vec<Chip> {
    (0..n).map(|i| {
        let (a0, a1) = (2.0 * PI * i as f64 / n as f64, 2.0 * PI * (i + 1) as f64 / n as f64);
        Chip { outline: vec![polar(c, r - w / 2.0, a0), polar(c, r + w / 2.0, a0), polar(c, r + w / 2.0, a1), polar(c, r - w / 2.0, a1)], floor: vec![polar(c, r, a0), polar(c, r, a1)], corners: vec![] }
    }).collect()
}

/// A fluted rim: V-grooves from the outer circle `r1` pointing in to `r0`, side by side.
pub fn flute_rim(c: Point, r0: f64, r1: f64, count: usize) -> Vec<Chip> {
    let step = 2.0 * PI / count as f64;
    (0..count).map(|i| {
        let a = i as f64 * step;
        let (p, q, apex) = (polar(c, r1, a), polar(c, r1, a + step), polar(c, r0, a + step / 2.0));
        Chip { outline: vec![p, q, apex], floor: vec![lerp(apex, polar(c, r1, a + step / 2.0), 0.15), polar(c, r1 - (r1 - r0) * 0.12, a + step / 2.0)], corners: vec![0, 1] }
    }).collect()
}

/// A ring of four-corner diamonds between `r0` and `r1`.
pub fn diamond_ring(c: Point, r0: f64, r1: f64, count: usize, fill: f64) -> Vec<Chip> {
    let step = 2.0 * PI / count as f64; let rm = (r0 + r1) / 2.0;
    (0..count).map(|i| {
        let a = i as f64 * step; let w = step * fill / 2.0;
        crate::facets::quad(polar(c, r0, a), polar(c, rm, a + w), polar(c, r1, a), polar(c, rm, a - w), polar(c, rm, a))
    }).collect()
}

/// A band of small six-point stars round a ring at radius `r`, with a small
/// triangle on each side of the band between neighbours.
pub fn star_band(c: Point, r: f64, band: f64, count: usize) -> Vec<Chip> {
    let step = 2.0 * PI / count as f64; let rs = (band / 2.0).min(r * step * 0.5) * 0.92;
    let mut out = vec![];
    for i in 0..count {
        let a = i as f64 * step;
        let sc = polar(c, r, a);
        out.extend(small_star(sc, rs, rs * 0.42, 6, a));
        // triangles at the band edges, in the gaps between stars, reaching in to meet the stars
        let g = a + step / 2.0; let w = step * 0.3;
        out.push(tri(polar(c, r + band / 2.0, g - w), polar(c, r + band / 2.0, g + w), polar(c, r + band * 0.05, g)));
        out.push(tri(polar(c, r - band / 2.0, g + w), polar(c, r - band / 2.0, g - w), polar(c, r - band * 0.05, g)));
    }
    out
}

/// A curved cell (from arcs) as a three-or-more-corner chip: corners where the
/// outline turns sharply; facets meet at the centroid (or inside, if that falls out).
pub fn curved_chip(poly: Vec<Point>) -> Chip {
    let n = poly.len();
    let corners: Vec<usize> = (0..n).filter(|&i| {
        let (a, b, c) = (poly[(i + n - 1) % n], poly[i], poly[(i + 1) % n]);
        let (u, v) = ((b.x - a.x, b.y - a.y), (c.x - b.x, c.y - b.y));
        let ang = (u.0 * v.1 - u.1 * v.0).atan2(u.0 * v.0 + u.1 * v.1).abs();
        ang > 0.5
    }).collect();
    let mut deep = centroid(&poly);
    if !crate::outline::inside(deep, &poly) && !corners.is_empty() {
        deep = pt(corners.iter().map(|&i| poly[i].x).sum::<f64>() / corners.len() as f64, corners.iter().map(|&i| poly[i].y).sum::<f64>() / corners.len() as f64);
    }
    Chip { outline: poly, floor: vec![deep], corners }
}

fn circle(c: Point, r: f64, n: usize) -> Vec<Point> { (0..n).map(|i| polar(c, r, 2.0 * PI * i as f64 / n as f64)).collect() }
fn dedupe(p: &mut Vec<Point>) { p.dedup_by(|a, b| distance(*a, *b) < 1e-9); if p.len() > 1 && distance(p[0], *p.last().unwrap()) < 1e-9 { p.pop(); } }

/// Gap fans for a star: between tip k and tip k+1 (flank paths meeting at the
/// inner vertex), a fan radiates from the point on radius `rw` midway between
/// the tips to the two flanks.
pub fn gap_fans(c: Point, tips: &[Point], right_flanks: &[Vec<Point>], left_flanks: &[Vec<Point>], rw: f64, slices: usize) -> Vec<Chip> {
    let n = tips.len(); let mut out = vec![];
    for k in 0..n {
        // path: tip k back down its right flank to the inner vertex, then up the next point's left flank
        let mut path: Vec<Point> = right_flanks[k].iter().rev().copied().collect();
        path.extend(left_flanks[(k + 1) % n].iter().skip(1).copied());
        let (a0, a1) = ((tips[k].y - c.y).atan2(tips[k].x - c.x), (tips[(k + 1) % n].y - c.y).atan2(tips[(k + 1) % n].x - c.x));
        let mut d = a1 - a0; while d <= 0.0 { d += 2.0 * PI; }
        let apex = polar(c, rw, a0 + d / 2.0);
        out.extend(fan(apex, &resample(&path, slices)));
    }
    out
}

/// A raised star of `n` points: tips at `rt`, inner vertices at `ri`, flanks bowed
/// by `bend` (positive: concave points) and the tips turned by `twist` radians.
/// Returns the chips, the tips, and each point's (left, right) flank from inner vertex to tip.
pub fn raised_star(c: Point, n: usize, rt: f64, ri: f64, phase: f64, bend: f64, twist: f64) -> (Vec<Chip>, Vec<Point>, Vec<Vec<Point>>, Vec<Vec<Point>>) {
    let step = 2.0 * PI / n as f64;
    let (mut chips, mut tips, mut lefts, mut rights) = (vec![], vec![], vec![], vec![]);
    for k in 0..n {
        let a = phase + k as f64 * step;
        let tip = polar(c, rt, a + twist);
        let (vl, vr) = (polar(c, ri, a - step / 2.0), polar(c, ri, a + step / 2.0));
        let axis = lerp(c, tip, 0.55);
        let (left, right) = (bow(vl, tip, axis, bend, 12), bow(vr, tip, axis, bend, 12));
        let base = lerp(vl, vr, 0.5);
        chips.extend(raised_point(base, tip, &left, &right));
        tips.push(tip); lefts.push(left); rights.push(right);
    }
    (chips, tips, lefts, rights)
}

/// Faces of the arrangement of `circles` inside `bound`, each with the number of
/// circles containing it. Cutting the odd ones gives the checkerboard of a compass rosette.
pub fn arrangement(bound: &[Point], circles: &[Vec<Point>]) -> Vec<(Vec<Point>, usize)> {
    let mut faces: Vec<(Vec<Vec<Point>>, usize)> = vec![(vec![bound.to_vec()], 0)];
    for circ in circles {
        let cs = vec![vec![circ.clone()]];
        let mut next = vec![];
        for (shape, k) in faces {
            let s = vec![shape];
            for f in intersect(&s, &cs) { next.push((f, k + 1)); }
            for f in difference(&s, &cs) { next.push((f, k)); }
        }
        faces = next;
    }
    faces.into_iter().filter(|(s, _)| s.len() == 1).map(|(s, k)| (s.into_iter().next().unwrap(), k)).collect()
}

// ---------------------------------------------------------------- compositions

pub struct Composition { pub name: &'static str, pub note: &'static str, pub width: f64, pub height: f64, pub round: bool, pub chips: Vec<Chip> }

/// L, after the rolling pin: a raised six-point star round a small rosette in its
/// hexagon, fans from the ring into every gap, and a double incised ring.
pub fn layered_star(size: f64) -> Composition {
    let r = size / 2.0; let c = pt(r, r); let mut chips = vec![];
    chips.extend(groove_ring(c, r * 0.97, r * 0.025, 120));
    chips.extend(groove_ring(c, r * 0.92, r * 0.025, 120));
    let ri = r * 0.84 / 3f64.sqrt();
    let (star, tips, lefts, rights) = raised_star(c, 6, r * 0.84, ri, -PI / 2.0, 0.0, 0.0);
    chips.extend(star);
    chips.extend(gap_fans(c, &tips, &rights, &lefts, r * 0.88, 12));
    // inside the hexagon: a six-point rosette toward the hexagon corners, and an arrow at each edge
    chips.extend(small_star(c, ri * 0.8, ri * 0.36, 6, -PI / 2.0 + PI / 6.0));
    for k in 0..6 {
        let a = -PI / 2.0 + k as f64 * PI / 3.0;
        let (vl, vr) = (polar(c, ri, a - PI / 6.0), polar(c, ri, a + PI / 6.0)); let m = lerp(vl, vr, 0.5);
        chips.push(tri(lerp(m, vl, 0.5), lerp(m, vr, 0.5), lerp(m, c, 0.28)));
    }
    Composition { name: "L  Layered star", note: "raised 6-point star, fans in every gap, rosette in the hexagon, double ring", width: size, height: size, round: true, chips }
}

/// M, after the board: a three-point star with curved, turning points, long fans in
/// its gaps, a ring band of small stars, and a fluted rim.
pub fn curved_star(size: f64) -> Composition {
    let r = size / 2.0; let c = pt(r, r); let mut chips = vec![];
    chips.extend(flute_rim(c, r * 0.86, r, 60));
    chips.extend(star_band(c, r * 0.725, r * 0.25, 16));
    chips.extend(groove_ring(c, r * 0.585, r * 0.022, 120));
    let (star, tips, lefts, rights) = raised_star(c, 3, r * 0.54, r * 0.17, -PI / 2.0, 0.22, 0.22);
    chips.extend(star);
    chips.extend(gap_fans(c, &tips, &rights, &lefts, r * 0.55, 16));
    Composition { name: "M  Curved star", note: "3 raised curved points, long gap fans, ring of small stars, fluted rim", width: size, height: size, round: true, chips }
}

/// N, after the compass rosette: twelve compass circles through the centre; their
/// cells are cut in a checkerboard, with a ring of diamonds at the rim.
pub fn compass_arcs(size: f64) -> Composition {
    let r = size / 2.0; let c = pt(r, r); let mut chips = vec![];
    chips.extend(diamond_ring(c, r * 0.86, r * 0.98, 36, 0.8));
    let r0 = r * 0.41;
    let circles: Vec<Vec<Point>> = (0..12).map(|k| circle(polar(c, r0, -PI / 2.0 + k as f64 * PI / 6.0), r0, 180)).collect();
    for (face, k) in arrangement(&circle(c, r * 0.83, 240), &circles) {
        if k % 2 == 0 { continue; }
        let a = signed_area(&face).abs();
        let per: f64 = (0..face.len()).map(|i| distance(face[i], face[(i + 1) % face.len()])).sum();
        // slivers at the centre where every circle meets are left uncut
        if a < 2.0 || 2.0 * a / per < 0.7 { continue; }
        chips.push(curved_chip(face));
    }
    Composition { name: "N  Compass arcs", note: "12 compass circles through the centre, cells cut as a checkerboard, diamond rim", width: size, height: size, round: true, chips }
}

/// O, after the box lid: a raised five-point star round a twelve-ray rosette in an
/// incised circle, gap fans, and fans from the lid's corners and side midpoints.
pub fn box_lid(w: f64, h: f64) -> Composition {
    let c = pt(w / 2.0, h / 2.0); let r = h * 0.44; let mut chips = vec![];
    chips.extend(groove_ring(c, r, r * 0.04, 120));
    let ri = r * 0.4;
    let (star, tips, lefts, rights) = raised_star(c, 5, r * 0.9, ri, -PI / 2.0, 0.08, 0.0);
    chips.extend(star);
    chips.extend(gap_fans(c, &tips, &rights, &lefts, r * 0.92, 10));
    // rosette: alternate rays cut
    for k in (0..24).step_by(2) {
        let a = k as f64 * PI / 12.0;
        chips.push(tri(polar(c, ri * 0.08, a + PI / 24.0), polar(c, ri * 0.82, a), polar(c, ri * 0.82, a + PI / 12.0)));
    }
    // fans from the corners and the side midpoints toward the circle, alternate slices cut
    let m = h * 0.03;
    chips.extend(anchor_fans(c, r * 1.07, &[pt(w - m, h / 2.0), pt(w - m, h - m), pt(m, h - m), pt(m, h / 2.0), pt(m, m), pt(w - m, m)], 0.12));
    Composition { name: "O  Box lid", note: "raised 5-point star, 12-ray centre, gap fans, fans from corners and sides", width: w, height: h, round: false, chips }
}

// ---------------------------------------------------------------- flow-aware fills

/// Fans from `anchors` (in order round `c`) to the circle of radius `rf`, alternate
/// slices cut. Each anchor owns the stretch of circle half-way to its neighbours, so
/// fans never overlap, limited to what it can see without its rays crossing the circle.
/// `slice` is the slice angle in radians.
pub fn anchor_fans(c: Point, rf: f64, anchors: &[Point], slice: f64) -> Vec<Chip> {
    let dirs: Vec<f64> = anchors.iter().map(|p| (p.y - c.y).atan2(p.x - c.x)).collect();
    let wrap = |a: f64| { let mut a = a; while a <= -PI { a += 2.0 * PI; } while a > PI { a -= 2.0 * PI; } a };
    let n = anchors.len(); let mut out = vec![];
    for (i, &apex) in anchors.iter().enumerate() {
        let (lo, hi) = (wrap(dirs[(i + n - 1) % n] - dirs[i]) / 2.0, wrap(dirs[(i + 1) % n] - dirs[i]) / 2.0);
        let seen = (rf / distance(apex, c)).min(1.0).acos() * 0.97;
        let (a0, a1) = (dirs[i] + lo.clamp(-seen, seen), dirs[i] + hi.clamp(-seen, seen));
        let slices = (((a1 - a0).abs() / slice).round() as usize).max(4);
        let arc: Vec<Point> = (0..=slices).map(|k| polar(c, rf, a0 + (a1 - a0) * k as f64 / slices as f64)).collect();
        for (j, wnd) in arc.windows(2).enumerate() { if j % 2 == 0 { out.push(tri(apex, wnd[0], wnd[1])); } }
    }
    out
}

/// A repeat laid on a polar grid round `c`: `sectors` sectors from angle `phase`
/// (a multiple of the main motif's point count lines the cells up with it), rings
/// from `r_in` outward that widen as they go so the cells stay square. Chips are
/// kept whole where `keep` accepts every point of them.
pub fn polar_fill(kind: crate::facets::Repeat, c: Point, r_in: f64, r_max: f64, sectors: usize, phase: f64, keep: &dyn Fn(Point) -> bool) -> Vec<Chip> {
    let da = 2.0 * PI / sectors as f64; let grow = da.exp();
    let unit = crate::facets::repeat_unit(kind, 0.5);
    let mut out = vec![]; let mut r0 = r_in;
    while r0 < r_max {
        for s in 0..sectors {
            let a0 = phase + s as f64 * da;
            let map = |u: f64, v: f64| polar(c, r0 * grow.powf(v), a0 + u * da);
            out.extend(crate::facets::warp_unit(&unit, 0.5, 6, &map).into_iter().filter(|ch| ch.outline.iter().all(|p| keep(*p))));
        }
        r0 *= grow;
    }
    out
}

/// A repeat fitted into the band between two outlines round `c`, given as radius
/// by angle: `sectors` columns from `phase`, and as many rows as keeps the cells
/// nearest to square on average. Rows follow both outlines, so nothing is clipped.
/// `rows`: None picks the count; `graded`: rows grow outward in proportion to their
/// radius (as on a polar grid) so cells stay square across a wide band.
pub fn band_fill(kind: crate::facets::Repeat, c: Point, sectors: usize, phase: f64, inner: &dyn Fn(f64) -> f64, outer: &dyn Fn(f64) -> f64, rows: Option<usize>, graded: bool) -> Vec<Chip> {
    let da = 2.0 * PI / sectors as f64;
    let ratio: f64 = (0..sectors).map(|s| { let a = phase + (s as f64 + 0.5) * da; let (ri, ro) = (inner(a), outer(a));
        if graded { (ro / ri).ln() / da } else { (ro - ri) / ((ri + ro) / 2.0 * da) } }).sum::<f64>() / sectors as f64;
    let rows = rows.unwrap_or((ratio.round() as usize).max(1));
    let unit = crate::facets::repeat_unit(kind, 0.5);
    let mut out = vec![];
    for row in 0..rows { for s in 0..sectors {
        let a0 = phase + s as f64 * da;
        let map = |u: f64, v: f64| {
            let a = a0 + u * da; let (ri, ro) = (inner(a), outer(a)); let t = (row as f64 + v) / rows as f64;
            polar(c, if graded { ri * (ro / ri).powf(t) } else { ri + (ro - ri) * t }, a)
        };
        out.extend(crate::facets::warp_unit(&unit, 0.5, 6, &map));
    } }
    out
}

/// The motif a flow fill wraps round: its centre, outer radius, and number of points.
#[derive(Clone, Copy, Debug)]
pub struct Motif { pub centre: Point, pub radius: f64, pub points: usize }

/// A fill that follows the composition: a band of `kind` round the motif (rows growing
/// outward, sectors a multiple of its points, sized near `cell`), an optional sawtooth
/// row hugging the motif, and fans from the far corner of every pocket left over. With
/// no motif, a small star marks the middle of the outline and the band grows round it.
/// Everything stays inside the outline (inset by `margin`) and clear of `avoid`.
pub fn flow_fill(kind: crate::facets::Repeat, cell: f64, lasso: &[Point], margin: f64, edge_row: bool, avoid: &[&[Point]], motif: Option<Motif>) -> Vec<Chip> {
    use crate::booleans::{area, difference};
    use crate::facets::{clip_chip, fill_allowed, rim_band, FillEdge};
    if lasso.len() < 3 || cell <= 0.0 { return vec![]; }
    let allowed = fill_allowed(lasso, margin, avoid);
    if area(&allowed) < 4.0 { return vec![]; }
    let mut out: Vec<Chip> = vec![];
    let (c, n, mut r_in) = match motif {
        Some(m) => (m.centre, m.points.max(3), m.radius + margin),
        None => {
            let c = centroid(lasso); let r = cell * 0.6;
            out.extend(small_star(c, r, r * 0.4, 8, -PI / 2.0));
            (c, 8, r + margin)
        }
    };
    // just clear of the space kept round the motif, so whole-chip checks don't graze it
    r_in += 0.5;
    // the band reaches out to the nearest edge beyond it (the outline, or a neighbour)
    let segs: Vec<(Point, Point)> = allowed.iter().flatten().flat_map(|ring| (0..ring.len()).map(move |i| (ring[i], ring[(i + 1) % ring.len()]))).collect();
    let r_out = segs.iter().map(|(a, b)| seg_dist(c, *a, *b)).filter(|d| *d > r_in + 1.0).fold(f64::INFINITY, f64::min);
    let min_band = cell * 0.35;
    let sectors_for = |mid: f64| n * ((2.0 * PI * mid / cell / n as f64).round() as usize).max(1);
    // how far the outline reaches round the motif: a roundish outline (a blob, an oval,
    // a circle) gets one band that follows it all the way out; an angular one (a square,
    // a rectangle) gets a round band and fans in its corners
    let outer_from = r_in + 1.0;
    let outer_hit = |a: f64| ray_hit(c, a, outer_from, &segs);
    let samples: Vec<f64> = (0..72).map(|i| outer_hit(2.0 * PI * i as f64 / 72.0)).collect();
    let roundish = samples.iter().all(|r| r.is_finite()) && {
        let (lo, hi) = samples.iter().fold((f64::INFINITY, 0.0f64), |(lo, hi), r| (lo.min(*r), hi.max(*r)));
        hi / lo <= 1.3
    };
    // the edge row only when a band still fits beyond it: the band comes first
    let nearest = if roundish { samples.iter().cloned().fold(f64::INFINITY, f64::min) } else { r_out };
    if edge_row && nearest.is_finite() && nearest - (r_in + 4.0 + margin) >= min_band {
        let teeth = (n * ((2.0 * PI * r_in / 3.2 / n as f64).round() as usize).max(1)).max(12);
        rim_band(&mut out, c, r_in, r_in + 4.0, teeth, 0.0);
        r_in += 4.0 + margin;
    }
    if roundish {
        if nearest - r_in >= min_band {
            let mean = samples.iter().sum::<f64>() / samples.len() as f64;
            let sectors = sectors_for((r_in + mean) / 2.0);
            let ri = r_in;
            out.extend(band_fill(kind, c, sectors, -PI / 2.0, &|_| ri, &|a| outer_hit(a), None, false));
        }
        return out.into_iter().flat_map(|ch| clip_chip(ch, &allowed, FillEdge::Whole)).collect();
    }
    let mut reach = r_in;
    if r_out.is_finite() && r_out - r_in >= min_band {
        let sectors = sectors_for((r_in + r_out) / 2.0);
        let da = 2.0 * PI / sectors as f64;
        let rows = (((r_out / r_in).ln() / da).round() as usize).max(1);
        out.extend(band_fill(kind, c, sectors, -PI / 2.0, &|_| r_in, &|_| r_out, Some(rows), true));
        reach = r_out;
    }
    // pockets left over: fans from their far corner toward the band
    let disk: Vec<Point> = (0..240).map(|i| polar(c, reach + margin, 2.0 * PI * i as f64 / 240.0)).collect();
    let pockets = difference(&allowed, &vec![vec![disk]]);
    for pocket in pockets.iter().filter(|s| s.len() == 1) {
        let ring = &pocket[0];
        if signed_area(ring).abs() < 6.0 { continue; }
        let apex = *ring.iter().max_by(|a, b| distance(**a, c).total_cmp(&distance(**b, c))).unwrap();
        let rf = reach + margin;
        // the stretch of circle this pocket touches, seen from the apex
        let dir = (apex.y - c.y).atan2(apex.x - c.x);
        let wrap = |a: f64| { let mut a = a - dir; while a <= -PI { a += 2.0 * PI; } while a > PI { a -= 2.0 * PI; } a };
        let near: Vec<f64> = ring.iter().filter(|p| distance(**p, c) < rf + 1.5).map(|p| wrap((p.y - c.y).atan2(p.x - c.x))).collect();
        let shape = vec![pocket.clone()];
        if near.is_empty() {
            // a pocket away from the band: a plain grid of the repeat
            let b = crate::geometry::Bounds::of(ring);
            let (i0, j0) = ((b.l / cell).floor(), (b.t / cell).floor());
            for ch in crate::facets::repeat_field(kind, pt(i0 * cell, j0 * cell), cell, ((b.r / cell).ceil() - i0) as usize, ((b.b / cell).ceil() - j0) as usize) { out.extend(clip_chip(ch, &shape, FillEdge::Clip)); }
            continue;
        }
        let seen = (rf / distance(apex, c)).min(1.0).acos() * 0.97;
        let (lo, hi) = (near.iter().cloned().fold(f64::INFINITY, f64::min).max(-seen), near.iter().cloned().fold(f64::NEG_INFINITY, f64::max).min(seen));
        if hi - lo < 0.05 { continue; }
        let slices = (((hi - lo) / 0.1).round() as usize).max(4);
        let arc: Vec<Point> = (0..=slices).map(|k| polar(c, rf, dir + lo + (hi - lo) * k as f64 / slices as f64)).collect();
        for (j, w) in arc.windows(2).enumerate() { if j % 2 == 0 { out.extend(clip_chip(tri(apex, w[0], w[1]), &shape, FillEdge::Clip)); } }
    }
    // the band, star and teeth only where they fit whole
    out.into_iter().flat_map(|ch| clip_chip(ch, &allowed, FillEdge::Whole)).collect()
}

fn seg_dist(p: Point, a: Point, b: Point) -> f64 {
    let (dx, dy) = (b.x - a.x, b.y - a.y); let l2 = dx * dx + dy * dy;
    let t = if l2 < 1e-12 { 0.0 } else { (((p.x - a.x) * dx + (p.y - a.y) * dy) / l2).clamp(0.0, 1.0) };
    distance(p, pt(a.x + dx * t, a.y + dy * t))
}

/// Distance from `c` along the ray at angle `a` to the first of `segs` it meets
/// beyond `from` (infinite when it meets none).
fn ray_hit(c: Point, a: f64, from: f64, segs: &[(Point, Point)]) -> f64 {
    let (dx, dy) = (a.cos(), a.sin());
    segs.iter().filter_map(|(p, q)| {
        let (ex, ey) = (q.x - p.x, q.y - p.y);
        let den = dx * ey - dy * ex;
        if den.abs() < 1e-12 { return None; }
        let (wx, wy) = (p.x - c.x, p.y - c.y);
        let t = (wx * ey - wy * ex) / den; let s = (wx * dy - wy * dx) / den;
        (t > from && (0.0..=1.0).contains(&s)).then_some(t)
    }).fold(f64::INFINITY, f64::min)
}
