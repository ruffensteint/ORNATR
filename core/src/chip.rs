//! Chip-carving pattern generator: square medallions built on a millimetre
//! grid (faceted or compass centre, running border, corner treatments).
//! A faithful port of the web generator; `tests/chip_golden.rs` checks it.
use crate::geometry::{pt, Point};
use crate::growth::Mulberry;
use crate::outline::intersection;
use crate::facets::{from_outline, square_border, BorderStyle, Centre, Chip};
use std::collections::BTreeMap;
use std::f64::consts::PI;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ChipFamily { Rosette, Star, Border }

impl ChipFamily {
    pub fn key(self) -> &'static str { match self { ChipFamily::Rosette => "rosette", ChipFamily::Star => "star", ChipFamily::Border => "border" } }
    pub fn from_key(k: &str) -> Option<ChipFamily> { match k { "rosette" => Some(ChipFamily::Rosette), "star" => Some(ChipFamily::Star), "border" => Some(ChipFamily::Border), _ => None } }
    pub fn label(self) -> &'static str { match self { ChipFamily::Star => "Faceted centre and running border", ChipFamily::Rosette => "Compass petals and running border", ChipFamily::Border => "Border frame" } }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ChipSettings {
    pub family: ChipFamily,
    pub count: u32,
    pub size: f64,
    pub removed: Vec<usize>,
    pub seed: Option<u32>,
    pub grid: Option<f64>,
    /// Hand-edited chips by index, replacing the generated outline.
    pub edits: BTreeMap<usize, Vec<Point>>,
    pub grammar: Option<u8>,
    pub border_seed: Option<u32>,
    pub border_version: Option<u8>,
    pub traditional: Option<bool>,
    /// The faceted engine (rosette plus border, with facet lines); None = the classic generator.
    pub faceted: Option<Faceted>,
}

impl Default for ChipSettings {
    fn default() -> Self {
        ChipSettings { family: ChipFamily::Star, count: 6, size: 100.0, removed: vec![], seed: Some(1248), grid: Some(5.0), edits: BTreeMap::new(), grammar: Some(2), border_seed: None, border_version: Some(1), traditional: Some(true), faceted: None }
    }
}

impl ChipSettings {
    pub fn seed(&self) -> u32 { self.seed.unwrap_or(1248) }
    pub fn step(&self) -> f64 { self.grid.unwrap_or(5.0) }
    pub fn border_seed(&self) -> u32 { self.border_seed.unwrap_or(self.seed()) }
    /// Name of the current border rhythm.
    pub fn border_name(&self) -> &'static str {
        if self.traditional == Some(true) { ["Linked petals", "Alternating fans", "Curved facet chain"][(self.border_seed() % 3) as usize] }
        else if self.border_version == Some(1) { BORDER_NAMES[(self.border_seed() % 6) as usize] }
        else { "Earlier border (vary to update)" }
    }
    /// A fresh generation with the current construction rules; clears edits.
    pub fn regenerated(&self) -> ChipSettings {
        ChipSettings { traditional: Some(true), grammar: Some(2), border_version: Some(1), grid: Some(self.step()), removed: vec![], edits: BTreeMap::new(), ..self.clone() }
    }
    pub fn visible(&self) -> Vec<(usize, Vec<Point>)> {
        chip_regions(self).into_iter().enumerate().filter(|(i, _)| !self.removed.contains(i)).collect()
    }
}

/// A faceted composition: a rosette centred on the page inside an optional square border.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Faceted { pub centre: Centre, pub count: u32, pub border: Option<BorderStyle> }

impl Faceted {
    /// One seed picks the centre, its count and the border, so "Generate
    /// variation" walks through the permutations.
    pub fn from_seed(seed: u32) -> Faceted {
        let mut rng = Mulberry(seed);
        let centre = Centre::ALL[(rng.next() * 4.0) as usize % 4];
        // classic counts: the default, or up to two steps of 2 either side
        let step = (rng.next() * 5.0) as i64 - 2;
        let count = (centre.default_count() as i64 + 2 * step).clamp(*centre.counts().start() as i64, *centre.counts().end() as i64) as u32;
        let border = [None, Some(BorderStyle::Zigzag), Some(BorderStyle::Arcade), Some(BorderStyle::Almond)][(rng.next() * 4.0) as usize % 4];
        Faceted { centre, count, border }
    }
}

/// Band width of the faceted border and the page margin, from the page size.
fn faceted_layout(size: f64) -> (f64, f64) { ((size * 0.13).max(8.0), size * 0.03) }

/// The faceted chips as generated (no hand edits).
fn faceted_base(s: &ChipSettings, f: &Faceted) -> Vec<Chip> {
    let (band, margin) = faceted_layout(s.size);
    let c = pt(s.size / 2.0, s.size / 2.0);
    let mut chips = vec![];
    let radius = match f.border {
        Some(style) => { chips.extend(square_border(style, pt(margin, margin), s.size - 2.0 * margin, band)); s.size / 2.0 - margin - band - s.size * 0.04 }
        None => s.size / 2.0 - margin,
    };
    let mut all = f.centre.build(c, radius, f.count).chips;
    all.extend(chips);
    all
}

/// Every chip with its facets, hand edits applied. A chip that was only moved
/// keeps its own facets; a reshaped one is cut to its centroid. Classic chips
/// are read from their outlines.
pub fn chip_facets(s: &ChipSettings) -> Vec<Chip> {
    let Some(f) = s.faceted else { return chip_regions(s).iter().map(|p| from_outline(p)).collect() };
    faceted_base(s, &f).into_iter().enumerate().map(|(i, chip)| match s.edits.get(&i) {
        None => chip,
        Some(e) if e.len() == chip.outline.len() => {
            let d = pt(e[0].x - chip.outline[0].x, e[0].y - chip.outline[0].y);
            let moved = e.iter().zip(&chip.outline).all(|(a, b)| (a.x - b.x - d.x).abs() < 1e-6 && (a.y - b.y - d.y).abs() < 1e-6);
            if moved { Chip { outline: e.clone(), floor: chip.floor.iter().map(|q| pt(q.x + d.x, q.y + d.y)).collect(), corners: chip.corners } } else { from_outline(e) }
        }
        Some(e) => from_outline(e),
    }).collect()
}

pub const BORDER_NAMES: [&str; 6] = ["Diamond chain", "Paired chevrons", "Braided band", "Alternating rosettes", "Stepped ribbon", "Paired fans"];

fn polar(r: f64, a: f64) -> Point { pt(r * a.cos(), r * a.sin()) }

/// A usable chip outline: at least three distinct corners, no crossings, some area.
pub fn valid_chip(points: &[Point]) -> bool {
    let mut p: Vec<Point> = vec![];
    for (i, q) in points.iter().enumerate() { if i == 0 || (q.x - points[i - 1].x).hypot(q.y - points[i - 1].y) > 0.000001 { p.push(*q); } }
    if p.len() > 1 { let (a, b) = (p[0], p[p.len() - 1]); if (a.x - b.x).hypot(a.y - b.y) < 0.000001 { p.pop(); } }
    if p.len() < 3 { return false; }
    let n = p.len(); let mut area = 0.0;
    for i in 0..n {
        let (a, b) = (p[i], p[(i + 1) % n]);
        if (a.x - b.x).hypot(a.y - b.y) < 0.001 { return false; }
        area += a.x * b.y - b.x * a.y;
        for j in i + 2..n { if i == 0 && j == n - 1 { continue; } if intersection(a, b, p[j], p[(j + 1) % n]).is_some() { return false; } }
    }
    area.abs() > 0.01
}

pub fn chip_regions(s: &ChipSettings) -> Vec<Vec<Point>> {
    if let Some(f) = s.faceted { return faceted_base(s, &f).into_iter().enumerate().map(|(i, c)| s.edits.get(&i).cloned().unwrap_or(c.outline)).collect(); }
    if s.grid.is_some() { return grid_regions(s).into_iter().enumerate().map(|(i, p)| s.edits.get(&i).cloned().unwrap_or(p)).collect(); }
    let (r, c) = (s.size * 0.42, s.size / 2.0);
    let count = s.count as f64;
    if s.family == ChipFamily::Border {
        return (0..s.count * 2).map(|i| { let step = s.size / (count * 2.0 + 1.0); let x = step * (i as f64 + 1.0); let y = c;
            vec![pt(x, y - step * 0.8), pt(x + step * 0.45, y), pt(x, y + step * 0.8), pt(x - step * 0.45, y)] }).collect();
    }
    (0..s.count).map(|i| {
        let a = i as f64 * PI * 2.0 / count - PI / 2.0;
        if s.family == ChipFamily::Star {
            return [polar(r * 0.12, a - PI / count), polar(r, a), polar(r * 0.12, a + PI / count)].iter().map(|p| pt(p.x + c, p.y + c)).collect();
        }
        let half = (PI / 6.0).min(PI / count * 0.85); let radius = r / (2.0 * half.sin()); let offset = radius * half.cos();
        let mut p = vec![];
        for j in 0..=40 { let t = -half + 2.0 * half * j as f64 / 40.0; p.push(pt(r / 2.0 + radius * t.sin(), radius * t.cos() - offset)); }
        for j in (0..=40).rev() { let t = -half + 2.0 * half * j as f64 / 40.0; p.push(pt(r / 2.0 + radius * t.sin(), offset - radius * t.cos())); }
        p.into_iter().map(|q| pt(c + q.x * a.cos() - q.y * a.sin(), c + q.x * a.sin() + q.y * a.cos())).collect()
    }).collect()
}

pub fn grid_regions(s: &ChipSettings) -> Vec<Vec<Point>> {
    if s.traditional == Some(true) { return traditional_chips(s); }
    if s.grammar == Some(2) { return compose_chips(s); }
    let step = s.step(); let cells = (s.size / step).floor(); let tiles = ((cells - 2.0) / 4.0).floor();
    let mut regions = vec![];
    if tiles <= 0.0 { return regions; }
    let mut rng = Mulberry(s.seed());
    let vocabulary: [&[&[(f64, f64)]]; 5] = [
        &[&[(2.0, 0.0), (3.0, 2.0), (2.0, 4.0), (1.0, 2.0)]],
        &[&[(0.0, 0.0), (4.0, 0.0), (2.0, 2.0)], &[(0.0, 4.0), (4.0, 4.0), (2.0, 2.0)]],
        &[&[(2.0, 0.0), (4.0, 2.0), (2.0, 2.0)], &[(2.0, 4.0), (0.0, 2.0), (2.0, 2.0)]],
        &[&[(2.0, 0.0), (3.0, 1.0), (2.0, 2.0), (1.0, 1.0)], &[(4.0, 2.0), (3.0, 3.0), (2.0, 2.0), (3.0, 1.0)], &[(2.0, 4.0), (1.0, 3.0), (2.0, 2.0), (3.0, 3.0)], &[(0.0, 2.0), (1.0, 1.0), (2.0, 2.0), (1.0, 3.0)]],
        &[&[(0.0, 1.0), (2.0, 1.0), (1.0, 3.0)], &[(2.0, 1.0), (4.0, 1.0), (3.0, 3.0)]],
    ];
    let half = (tiles / 2.0).ceil() as usize;
    let recipes: Vec<(usize, usize)> = (0..half * half).map(|_| { let m = (rng.next() * vocabulary.len() as f64).floor() as usize; let t = (rng.next() * 4.0).floor() as usize; (m, t) }).collect();
    let offset = ((cells - tiles * 4.0) / 2.0).floor();
    let n = tiles as usize;
    for y in 0..n { for x in 0..n {
        if s.family == ChipFamily::Border && x > 0 && y > 0 && x < n - 1 && y < n - 1 { continue; }
        let (mx, my) = (x.min(n - 1 - x), y.min(n - 1 - y));
        let (motif0, turn) = recipes[my * half + mx];
        let motif = if s.family == ChipFamily::Rosette && mx == my { 3 } else { motif0 };
        for poly in vocabulary[motif] {
            regions.push(poly.iter().map(|&(px, py)| {
                let (mut a, mut b) = (px - 2.0, py - 2.0);
                for _ in 0..turn { let t = a; a = -b; b = t; }
                if x as f64 >= tiles / 2.0 { a = -a; }
                if y as f64 >= tiles / 2.0 { b = -b; }
                pt((offset + x as f64 * 4.0 + 2.0 + a) * step, (offset + y as f64 * 4.0 + 2.0 + b) * step)
            }).collect());
        }
    } }
    regions
}

/// Sector and repeat constructions: circular geometry is preserved; the grid
/// sets radii and repeat dimensions.
pub fn traditional_chips(s: &ChipSettings) -> Vec<Vec<Point>> {
    let g = s.step(); let c = (s.size / g / 2.0).floor() * g; let n = (s.size / g).floor();
    let radius = g.max(((n / 2.0).floor() - 1.0) * g);
    let mut out: Vec<Vec<Point>> = vec![];
    let seed = s.seed(); let petals = [6.0, 8.0, 10.0, 12.0][(seed % 4) as usize]; let style = (seed / 4) % 3;
    let lens = |out: &mut Vec<Vec<Point>>, a: Point, b: Point, width: f64, faceted: bool| {
        let (dx, dy) = (b.x - a.x, b.y - a.y); let len = dx.hypot(dy); let (nx, ny) = (-dy / len, dx / len);
        let mut top = vec![]; let mut bottom = vec![];
        for j in 0..=20 { let t = j as f64 / 20.0; let bulge = (PI * t).sin() * width;
            top.push(pt(a.x + dx * t + nx * bulge, a.y + dy * t + ny * bulge)); bottom.push(pt(a.x + dx * t - nx * bulge, a.y + dy * t - ny * bulge)); }
        if faceted { out.push(top); out.push(bottom); } else { bottom.reverse(); top.extend(bottom); out.push(top); }
    };
    let polar_c = |r: f64, a: f64| pt(c + r * a.cos(), c + r * a.sin());
    if s.family != ChipFamily::Border {
        let outer = radius * 0.68; let hub = if style == 1 { outer * 0.24 } else { 0.0 };
        for i in 0..petals as usize {
            let a = i as f64 * 2.0 * PI / petals - PI / 2.0; let b = a + PI / petals;
            if s.family == ChipFamily::Rosette || style != 2 { lens(&mut out, polar_c(hub, a), polar_c(outer, a), outer * (PI / petals).sin() * 0.42, true); }
            else { let root = polar_c(hub, a); let tip = polar_c(outer, a); let left = polar_c(outer * 0.48, a - PI / petals * 0.6); let right = polar_c(outer * 0.48, a + PI / petals * 0.6);
                out.push(vec![root, left, tip]); out.push(vec![root, tip, right]); }
            let (r1, r2) = (outer * 1.1, radius * 0.78);
            out.push(vec![polar_c(r1, b - 0.12), polar_c(r2, b), polar_c(r1, b + 0.12)]);
        }
    }
    let band = (radius * 0.18).min(g.max(2.0 * g)); let lo = -radius + band; let hi = radius - band;
    let repeats = 1f64.max(((hi - lo) / (band * 2.0)).floor()); let pitch = (hi - lo) / repeats; let border = s.border_seed() % 3;
    let map = |mut x: f64, mut y: f64, k: usize| { for _ in 0..k { let t = x; x = -y; y = t; } pt(c + x, c + y) };
    for k in 0..4 {
        for i in 0..repeats as usize {
            let x = lo + i as f64 * pitch; let a = map(x, -radius + band / 2.0, k); let b = map(x + pitch, -radius + band / 2.0, k);
            if border == 0 { lens(&mut out, a, b, band * 0.42, true); }
            else if border == 1 { let mid = map(x + pitch / 2.0, -radius, k); let base = map(x + pitch / 2.0, -radius + band, k);
                lens(&mut out, a, mid, band * 0.16, true); lens(&mut out, mid, b, band * 0.16, true); out.push(vec![a, base, b]); }
            else { lens(&mut out, a, b, band * 0.32, false); let mid = map(x + pitch / 2.0, -radius + band / 2.0, k);
                out.push(vec![a, map(x + pitch * 0.3, -radius, k), mid]); out.push(vec![mid, map(x + pitch * 0.7, -radius + band, k), b]); }
        }
        lens(&mut out, map(-radius, -radius, k), map(-radius + band, -radius + band, k), band * 0.24, true);
    }
    out
}

/// One seed chooses the composition; repeats share vocabulary and direction.
pub fn compose_chips(s: &ChipSettings) -> Vec<Vec<Point>> {
    let g = s.step(); let n = (s.size / g).floor(); let c = (n / 2.0).floor(); let r = 2f64.max(c - 1.0);
    let mut out: Vec<Vec<Point>> = vec![];
    let mut rng = Mulberry(s.seed());
    let border_style = (rng.next() * 3.0).floor() as i32; let pitch = 2.0 + (rng.next() * 2.0).floor(); let heart = (rng.next() * 3.0).floor() as i32;
    let rotate = |x0: f64, y0: f64, k: usize| { let (mut x, mut y) = (x0, y0); for _ in 0..k { let a = x; x = -y; y = a; } pt(c + x, c + y) };
    let four = |out: &mut Vec<Vec<Point>>, p: &[(f64, f64)]| {
        for k in 0..4 { let q: Vec<Point> = p.iter().map(|&(x, y)| rotate(x, y, k)).collect();
            if q.iter().all(|q| q.x >= 0.0 && q.y >= 0.0 && q.x <= n && q.y <= n) { out.push(q.iter().map(|q| pt(q.x * g, q.y * g)).collect()); } }
    };
    if s.border_version == Some(1) {
        for phrase in border_phrases(r, s.border_seed()) { let p: Vec<(f64, f64)> = phrase.iter().map(|q| (q.x, q.y)).collect(); four(&mut out, &p); }
    } else if r >= 4.0 {
        let lo = -r + 2.0; let hi = r - 2.0; let depth = if r >= 7.0 { 2.0 } else { 1.0 };
        let mut t = lo;
        while t + pitch <= hi {
            let mid = t + 1.0;
            let p: Vec<(f64, f64)> = match border_style { 0 => vec![(t, -r), (t + pitch, -r), (mid, -r + depth)], 1 => vec![(t, -r + depth), (mid, -r), (t + pitch, -r + depth)], _ => vec![(t, -r + 1.0), (mid, -r), (t + pitch, -r + 1.0), (mid, -r + depth)] };
            if border_style != 2 || depth > 1.0 { four(&mut out, &p); } else { four(&mut out, &[(t, -r), (t + pitch, -r), (mid, -r + 1.0)]); }
            t += pitch;
        }
        four(&mut out, &[(-r, -r + 1.0), (-r + 1.0, -r), (-r + 2.0, -r + 1.0), (-r + 1.0, -r + 2.0)]);
    }
    if s.family == ChipFamily::Border { if out.is_empty() { four(&mut out, &[(-1.0, -r), (1.0, -r), (0.0, -r + 1.0)]); } return out; }
    let inner = 2f64.max(r - 4.0); let a = 1f64.max((inner * 0.47).floor()); let b = 1f64.max((inner * 0.16).floor());
    if s.family == ChipFamily::Rosette {
        four(&mut out, &[(0.0, 0.0), (-b, -a), (0.0, -inner), (b, -a)]);
        if inner >= 4.0 { four(&mut out, &[(0.0, 0.0), (a, -b), (a, -a), (b, -a)]); }
    } else {
        let tip = if heart == 0 { inner } else { 2f64.max(inner - 1.0) };
        four(&mut out, &[(0.0, 0.0), (-b, -a), (0.0, -tip)]);
        four(&mut out, &[(0.0, 0.0), (0.0, -tip), (b, -a)]);
        if inner >= 5.0 {
            let d = (a + 1.0).max((inner * if heart == 2 { 0.7 } else { 0.8 }).floor());
            four(&mut out, &[(-1.0, -1.0), (-a, -b), (-d, -d)]);
            four(&mut out, &[(-1.0, -1.0), (-d, -d), (-b, -a)]);
        }
    }
    if inner >= 8.0 && s.family == ChipFamily::Rosette {
        let ring = inner - 1.0; let mut t = -ring + 2.0;
        while t <= ring - 2.0 {
            if t.abs() >= a + 1.0 { four(&mut out, &[(t, -ring), (t + 1.0, -ring + 1.0), (t, -ring + 2.0), (t - 1.0, -ring + 1.0)]); }
            t += pitch + 1.0;
        }
    }
    out
}

/// Border phrases in grid units for the top side (the caller repeats them on
/// all four sides). Corners use the same band depth.
pub fn border_phrases(radius: f64, seed: u32) -> Vec<Vec<Point>> {
    let mut out: Vec<Vec<Point>> = vec![];
    let style = seed % 6; let depth = if radius >= 8.0 { 3.0 } else if radius >= 5.0 { 2.0 } else { 1.0 };
    let pitch = if depth == 3.0 { 6.0 } else { 4.0 }; let corner = depth; let available = radius * 2.0 - corner * 2.0;
    let count = (available / pitch).floor().max(0.0) as usize; let start = -((count as f64 * pitch / 2.0).floor());
    let add = |out: &mut Vec<Vec<Point>>, coords: &[(f64, f64)], x: f64| out.push(coords.iter().map(|&(a, b)| pt(x + a, -radius + b)).collect());
    if depth == 3.0 {
        for i in 0..count { let x = start + i as f64 * pitch;
            match style {
                0 => { add(&mut out, &[(0., 1.), (2., 0.), (4., 1.), (2., 3.)], x); add(&mut out, &[(4., 1.), (5., 0.), (6., 1.), (5., 2.)], x); }
                1 => { add(&mut out, &[(0., 0.), (2., 0.), (4., 2.), (2., 2.)], x); add(&mut out, &[(2., 3.), (4., 1.), (6., 1.), (4., 3.)], x); }
                2 => { add(&mut out, &[(0., 0.), (2., 0.), (4., 2.), (2., 2.)], x); add(&mut out, &[(0., 3.), (1., 2.), (2., 3.)], x); add(&mut out, &[(3., 1.), (4., 0.), (6., 2.), (6., 3.)], x); }
                3 => { add(&mut out, &[(0., 1.), (1., 0.), (2., 1.), (1., 2.)], x); add(&mut out, &[(2., 1.), (4., 0.), (6., 1.), (4., 3.)], x); add(&mut out, &[(2., 3.), (3., 2.), (4., 3.)], x); }
                4 => { add(&mut out, &[(0., 0.), (3., 0.), (3., 1.), (1., 1.), (1., 3.), (0., 3.)], x); add(&mut out, &[(2., 3.), (5., 3.), (5., 2.), (3., 2.), (3., 1.), (2., 1.)], x); }
                _ => { add(&mut out, &[(0., 0.), (3., 0.), (3., 2.)], x); add(&mut out, &[(0., 1.), (0., 3.), (2., 3.)], x); add(&mut out, &[(3., 3.), (6., 3.), (6., 1.)], x); add(&mut out, &[(4., 0.), (6., 0.), (6., 2.)], x); }
            }
        }
    } else {
        for i in 0..count { let x = start + i as f64 * pitch;
            if style % 2 == 0 { let e = if depth / 2.0 == 1.0 { 1.0 } else { 0.0 }; add(&mut out, &[(0.0, e), (2.0, 0.0), (4.0, e), (2.0, depth)], x); }
            else { add(&mut out, &[(0.0, 0.0), (2.0, 0.0), (1.0, depth)], x); add(&mut out, &[(2.0, depth), (4.0, depth), (3.0, 0.0)], x); }
        }
    }
    if depth >= 2.0 { let x = -radius; add(&mut out, &[(0., 1.), (1., 0.), (2., 1.), (1., 2.)], x); if depth == 3.0 { add(&mut out, &[(1., 2.), (2., 1.), (3., 2.), (2., 3.)], x); } }
    else { add(&mut out, &[(0., 0.), (1., 0.), (0., 1.)], -radius); }
    out
}

/// Editing handles: curved (sampled) pieces get Start/Middle/End; straight
/// pieces get one handle per corner.
pub fn chip_handles(points: &[Point]) -> Vec<(usize, Point, String)> {
    if points.len() == 21 || points.len() == 42 { return [0usize, 10, 20].iter().enumerate().map(|(i, &k)| (k, points[k], ["Start", "Middle", "End"][i].to_string())).collect(); }
    points.iter().enumerate().map(|(i, p)| (i, *p, format!("Corner {}", i + 1))).collect()
}

/// Move one handle; on a curved piece the whole curve bends smoothly.
pub fn move_chip_handle(points: &[Point], index: usize, target: Point) -> Vec<Point> {
    if points.len() != 21 && points.len() != 42 { return points.iter().enumerate().map(|(i, p)| if i == index { target } else { *p }).collect(); }
    let d = pt(target.x - points[index].x, target.y - points[index].y);
    points.iter().enumerate().map(|(i, p)| {
        let t = if i <= 20 { i as f64 } else { 41.0 - i as f64 } / 20.0;
        let w = if index == 0 { (1.0 - t) * (1.0 - 2.0 * t) } else if index == 20 { t * (2.0 * t - 1.0) } else { 4.0 * t * (1.0 - t) };
        pt(p.x + d.x * w, p.y + d.y * w)
    }).collect()
}

/// Three decimals with ties rounded away from zero, as JavaScript's
/// `toFixed` does (Rust's formatter rounds ties to even).
fn fixed3(x: f64) -> String {
    let exact = format!("{:.40}", x.abs());
    let (int, frac) = exact.split_once('.').unwrap();
    let mut digits: Vec<u8> = int.bytes().chain(frac.bytes().take(3)).map(|b| b - b'0').collect();
    if frac.as_bytes()[3] >= b'5' {
        let mut i = digits.len();
        loop { if i == 0 { digits.insert(0, 1); break; } i -= 1; if digits[i] == 9 { digits[i] = 0; } else { digits[i] += 1; break; } }
    }
    let text: String = digits.iter().map(|d| (d + b'0') as char).collect();
    let (a, b) = text.split_at(text.len() - 3);
    format!("{}{}.{}", if x < 0.0 { "-" } else { "" }, a, b)
}
fn point_text(p: &Point) -> String { format!("{} {}", fixed3(p.x), fixed3(p.y)) }

/// The pattern at actual size: retained chip outlines only, no grid.
pub fn chip_svg(s: &ChipSettings) -> String {
    if let Some(f) = s.faceted {
        // outlines, then the thinner facet lines a carver follows down to the floor
        let chips: Vec<Chip> = chip_facets(s).into_iter().enumerate().filter(|(i, _)| !s.removed.contains(i)).map(|(_, c)| c).collect();
        let outlines: String = chips.iter().map(|c| format!("<path d=\"M {} Z\"/>", c.outline.iter().map(point_text).collect::<Vec<_>>().join(" L "))).collect();
        let facets: String = chips.iter().flat_map(|c| c.facet_lines()).map(|l| format!("<path d=\"M {}\"/>", l.iter().map(point_text).collect::<Vec<_>>().join(" L "))).collect();
        return format!("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{0}mm\" height=\"{0}mm\" viewBox=\"0 0 {0} {0}\"><title>Chip carving {1}</title><g fill=\"none\" stroke=\"#000\" stroke-linejoin=\"round\" stroke-linecap=\"round\"><g stroke-width=\"0.25\">{2}</g><g stroke-width=\"0.15\">{3}</g></g></svg>", s.size, f.centre.key(), outlines, facets);
    }
    let fam = s.family.key();
    let paths: String = s.visible().iter().map(|(_, p)| format!("<path d=\"M {} Z\"/>", p.iter().map(point_text).collect::<Vec<_>>().join(" L "))).collect();
    format!("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{0}mm\" height=\"{0}mm\" viewBox=\"0 0 {0} {0}\"><title>Chip carving {1}</title><g fill=\"none\" stroke=\"#000\" stroke-width=\"0.25\" stroke-linejoin=\"round\">{2}</g></svg>", s.size, fam, paths)
}
