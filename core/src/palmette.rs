//! Palmette and rosette designs, as edited in the app's Palmette workspace.
//!
//! Axial and radial classical ornament (see ORNAMENT_FAMILIES.md, Palmette
//! and Radial / Rosette), built from the vocabulary measured in the palmette
//! study (`core/examples/palmette_study.rs`): S-spined spoon petals, palmette
//! fans, lotus flowers, stems, volute arms, collars, bosses, fleurettes,
//! husk drops, rosette petal rings and beaded rings.
//!
//! A design is a list of **motifs** drawn back to front. Each motif is one
//! element with its own settings (a palmette is a fan: petal count, spread,
//! length, bend…), and it can repeat about the design's centre: mirrored left
//! and right, top and bottom, turned round a ring, and set out along a row
//! (an anthemion band). The copies are made from the one element, so editing
//! any copy changes them all.
//!
//! The starting layouts are the banked studies P1–P4 and L1–L4, built here in
//! the study's own coordinates and fitted to the page.
use crate::booleans::{difference, offset, signed_area, union};
use crate::cartouche::{compose, Drawing, Layer, Owner};
use crate::geometry::{pt, Bounds, Point};
use crate::outline::path_data;
use std::f64::consts::PI;

// ---- symmetry -------------------------------------------------------------

/// How a motif repeats: set out `row` times along a row `pitch` mm apart
/// (to the right), then mirrored across the design's upright axis
/// (`mirror_x`) and level axis (`mirror_y`), then turned round a ring of
/// `ring` places about the centre. The element itself is the first copy.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Repeat { pub mirror_x: bool, pub mirror_y: bool, pub ring: u8, pub row: u8, pub pitch: f64 }
impl Repeat {
    pub const ONE: Repeat = Repeat { mirror_x: false, mirror_y: false, ring: 1, row: 1, pitch: 0.0 };
    /// Left and right.
    pub const MIRROR: Repeat = Repeat { mirror_x: true, ..Repeat::ONE };
    pub fn count(&self) -> usize {
        let m = match (self.mirror_x, self.mirror_y) { (false, false) => 1, (true, true) => 4, _ => 2 };
        m * self.ring.max(1) as usize * self.row.max(1) as usize
    }
    /// Along a row of `n`, `pitch` apart.
    pub fn row(n: u8, pitch: f64) -> Repeat { Repeat { row: n, pitch, ..Repeat::ONE } }
}

/// One copy's placement, an isometry: p → m·p + o.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Xf { pub m: [f64; 4], pub o: Point }
impl Xf {
    pub const IDENTITY: Xf = Xf { m: [1.0, 0.0, 0.0, 1.0], o: Point { x: 0.0, y: 0.0 } };
    pub fn point(&self, p: Point) -> Point { pt(self.m[0] * p.x + self.m[1] * p.y + self.o.x, self.m[2] * p.x + self.m[3] * p.y + self.o.y) }
    /// The point this copy puts at `p` (the matrix is orthogonal: its inverse is its transpose).
    pub fn back(&self, p: Point) -> Point {
        let (dx, dy) = (p.x - self.o.x, p.y - self.o.y);
        pt(self.m[0] * dx + self.m[2] * dy, self.m[1] * dx + self.m[3] * dy)
    }
    /// A direction (radians) as this copy turns it.
    pub fn angle(&self, a: f64) -> f64 { let (s, c) = a.sin_cos(); (self.m[2] * c + self.m[3] * s).atan2(self.m[0] * c + self.m[1] * s) }
    /// The direction this copy turns into `a`.
    pub fn back_angle(&self, a: f64) -> f64 { let (s, c) = a.sin_cos(); (self.m[1] * c + self.m[3] * s).atan2(self.m[0] * c + self.m[2] * s) }
    /// Whether this copy is a mirror image.
    pub fn reflects(&self) -> bool { self.m[0] * self.m[3] - self.m[1] * self.m[2] < 0.0 }
}
/// The copies of `r` about `c`, in order: the plain copies (each ring place,
/// along each row), then the mirrored ones, so mirrored halves lie over their
/// partners the same way everywhere.
pub fn copies(r: Repeat, c: Point) -> Vec<Xf> {
    let mut mirrors = vec![[1.0, 0.0, 0.0, 1.0]];
    if r.mirror_x { mirrors.push([-1.0, 0.0, 0.0, 1.0]); }
    if r.mirror_x && r.mirror_y { mirrors.push([-1.0, 0.0, 0.0, -1.0]); }
    if r.mirror_y { mirrors.push([1.0, 0.0, 0.0, -1.0]); }
    let (n, rows) = (r.ring.max(1), r.row.max(1));
    let mut out = vec![];
    for v in &mirrors {
        for k in 0..n {
            let (s, co) = (360.0 * k as f64 / n as f64).to_radians().sin_cos();
            // the ring turn after the mirror
            let m = [co * v[0] - s * v[2], co * v[1] - s * v[3], s * v[0] + co * v[2], s * v[1] + co * v[3]];
            for j in 0..rows {
                // moved along the row first, then mirrored and turned about the centre
                let (tx, ty) = (j as f64 * r.pitch - c.x, -c.y);
                out.push(Xf { m, o: pt(m[0] * tx + m[1] * ty + c.x, m[2] * tx + m[3] * ty + c.y) });
            }
        }
    }
    out
}

// ---- the model ------------------------------------------------------------

/// A petal's tip: pointed (lancet), or round.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tip { Pointed, Round }

/// Directions are radians on the page (y down: -π/2 points up).
#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    /// A palmette: `count` (odd) petals fanned from `base` about `axis`. Side
    /// petal j of k starts `spread`·(j/k) degrees off the axis, nearly
    /// parallel at the waist, bends out by `bend`·(j/k), flicks its tip back
    /// by `flick`·(j/k) (an S) and rolls its tip by `roll`·(j/k)²; lengths
    /// fall to `1 - decay` of the centre spear's `length`; `belly` is the
    /// petals' half-width against their length; roots spread `root` mm either side.
    Fan { base: Point, axis: f64, count: usize, spread: f64, bend: f64, flick: f64, length: f64, decay: f64, belly: f64, roll: f64, root: f64, tip: Tip, centre_tip: Tip },
    /// One S-spined petal from `base` (see `petal_s`).
    Petal { base: Point, heading: f64, length: f64, belly: f64, bend: f64, flick: f64, roll: f64, tip: Tip },
    /// A lotus: two tall petals curving out and a short round bud between, `height` tall.
    Lotus { base: Point, axis: f64, height: f64 },
    /// A plain stem from `top` running `length` along `heading`, widening from `w_top` to `w_foot` (half-widths).
    Stem { top: Point, heading: f64, length: f64, w_top: f64, w_foot: f64 },
    /// A scroll arm: a sweep of `length` from `start` along `heading`
    /// (curvature `curve` per mm), ending in a volute of outer radius
    /// `radius` turning to `side` (+1 clockwise on the page); `width` its half-width.
    Arm { start: Point, heading: f64, length: f64, curve: f64, radius: f64, side: f64, width: f64 },
    /// A collar band across a waist: `width` long, `height` high, turned by `turn`.
    Collar { c: Point, turn: f64, width: f64, height: f64 },
    /// A round boss with a ring.
    Boss { c: Point, r: f64 },
    /// A fleurette: `count` petals reaching `radius` from `c`, a second ring
    /// behind turned half a step (`back`: its size against the front ring, 0 none), a boss of `boss` mm.
    Fleurette { c: Point, count: usize, radius: f64, belly: f64, tip: Tip, back: f64, boss: f64 },
    /// A husk drop: `count` bellflowers hanging from `top` along `heading`,
    /// the first `size` (1 = 30 mm long), each `shrink` of the one above and
    /// tucked under it; a teardrop at the end when `tear`.
    Husks { top: Point, heading: f64, count: usize, size: f64, shrink: f64, tear: bool },
    /// A rosette ring: `count` broad petals with roots `root` from `c`,
    /// `length` long, turned `offset` of a step; tips heart-notched (`notch`),
    /// edges with a raised lip (`lip`) instead of the spoon hollow.
    PetalRing { c: Point, count: usize, root: f64, length: f64, belly: f64, offset: f64, notch: bool, lip: bool },
    /// A beaded ring: a band `width` wide round `c` at radius `r`, beads `bead` in radius along it.
    BeadedRing { c: Point, r: f64, width: f64, bead: f64 },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Element { pub id: u32, pub kind: Kind, pub repeat: Repeat }
impl Element {
    pub fn label(&self) -> &'static str {
        match self.kind {
            Kind::Fan { .. } => "Palmette", Kind::Petal { .. } => "Petal", Kind::Lotus { .. } => "Lotus", Kind::Stem { .. } => "Stem", Kind::Arm { .. } => "Volute arm",
            Kind::Collar { .. } => "Collar", Kind::Boss { .. } => "Boss", Kind::Fleurette { .. } => "Fleurette", Kind::Husks { .. } => "Husk drop", Kind::PetalRing { .. } => "Petal ring", Kind::BeadedRing { .. } => "Beaded ring",
        }
    }
    /// The motif's anchor: where it stands (moving it moves this).
    pub fn anchor(&self) -> Point {
        match &self.kind {
            Kind::Fan { base, .. } | Kind::Petal { base, .. } | Kind::Lotus { base, .. } => *base,
            Kind::Stem { top, .. } | Kind::Husks { top, .. } => *top,
            Kind::Arm { start, .. } => *start,
            Kind::Collar { c, .. } | Kind::Boss { c, .. } | Kind::Fleurette { c, .. } | Kind::PetalRing { c, .. } | Kind::BeadedRing { c, .. } => *c,
        }
    }
    /// Move the motif by (dx, dy).
    pub fn shift(&mut self, dx: f64, dy: f64) {
        let p = match &mut self.kind {
            Kind::Fan { base, .. } | Kind::Petal { base, .. } | Kind::Lotus { base, .. } => base,
            Kind::Stem { top, .. } | Kind::Husks { top, .. } => top,
            Kind::Arm { start, .. } => start,
            Kind::Collar { c, .. } | Kind::Boss { c, .. } | Kind::Fleurette { c, .. } | Kind::PetalRing { c, .. } | Kind::BeadedRing { c, .. } => c,
        };
        *p = pt(p.x + dx, p.y + dy);
    }
}

/// A palmette design on a page (mm): motifs back to front, repeating about `centre`.
#[derive(Clone, Debug, PartialEq)]
pub struct Design { pub width: f64, pub height: f64, pub centre: Point, pub elements: Vec<Element> }

impl Design {
    pub fn empty(width: f64, height: f64) -> Design { Design { width, height, centre: pt(width / 2.0, height / 2.0), elements: vec![] } }
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
    pub fn push(&mut self, kind: Kind, repeat: Repeat) -> u32 { let id = self.next_id(); self.elements.push(Element { id, kind, repeat }); id }
    pub fn copies_of(&self, e: &Element) -> Vec<Xf> { copies(e.repeat, self.centre) }

    // ---- drawing ----
    /// Every element's layers, per copy, back to front.
    pub fn layers(&self) -> Vec<(Owner, Layer)> {
        let mut out = vec![];
        for e in &self.elements { for (k, l) in self.element_layers(e).into_iter().enumerate() { for layer in l { out.push((Owner::Element(e.id, k), layer)); } } }
        out
    }
    /// One element's layers, per copy.
    pub fn element_layers(&self, e: &Element) -> Vec<Vec<Layer>> {
        let own: Vec<Layer> = motif(&e.kind).into_iter().map(Piece::layer).collect();
        self.copies_of(e).iter().map(|xf| if *xf == Xf::IDENTITY { own.clone() } else { own.iter().map(|l| map_layer(l, |p| xf.point(p))).collect() }).collect()
    }
    pub fn drawing(&self) -> Drawing { compose(&self.layers()) }
    /// The pattern SVG at actual size: visible outlines and inner lines.
    pub fn svg(&self) -> String { svg_of(&self.drawing(), self.width, self.height) }
    pub fn bounds(&self) -> Option<Bounds> { crate::cartouche::bounds_of(&self.layers()) }

    // ---- page ----
    /// Scale by `k` about the origin, then move by `shift`.
    pub fn map(&mut self, k: f64, shift: Point) {
        let mp = |p: Point| pt(p.x * k + shift.x, p.y * k + shift.y);
        self.centre = mp(self.centre);
        for e in self.elements.iter_mut() {
            e.repeat.pitch *= k;
            match &mut e.kind {
                Kind::Fan { base, length, root, .. } => { *base = mp(*base); *length *= k; *root *= k; }
                Kind::Petal { base, length, .. } => { *base = mp(*base); *length *= k; }
                Kind::Lotus { base, height, .. } => { *base = mp(*base); *height *= k; }
                Kind::Stem { top, length, w_top, w_foot, .. } => { *top = mp(*top); *length *= k; *w_top *= k; *w_foot *= k; }
                Kind::Arm { start, length, curve, radius, width, .. } => { *start = mp(*start); *length *= k; *curve /= k; *radius *= k; *width *= k; }
                Kind::Collar { c, width, height, .. } => { *c = mp(*c); *width *= k; *height *= k; }
                Kind::Boss { c, r } => { *c = mp(*c); *r *= k; }
                Kind::Fleurette { c, radius, boss, .. } => { *c = mp(*c); *radius *= k; *boss *= k; }
                Kind::Husks { top, size, .. } => { *top = mp(*top); *size *= k; }
                Kind::PetalRing { c, root, length, .. } => { *c = mp(*c); *root *= k; *length *= k; }
                Kind::BeadedRing { c, r, width, bead } => { *c = mp(*c); *r *= k; *width *= k; *bead *= k; }
            }
        }
    }
    /// Scale and centre the design on its page, leaving a margin.
    pub fn fit(&mut self) {
        let Some(b) = self.bounds() else { return };
        let m = self.width.min(self.height) * 0.06;
        let k = ((self.width - 2.0 * m) / (b.r - b.l).max(1e-6)).min((self.height - 2.0 * m) / (b.b - b.t).max(1e-6));
        let c = b.center();
        self.map(k, pt(self.width / 2.0 - c.x * k, self.height / 2.0 - c.y * k));
    }

    // ---- editing ----
    pub fn remove(&mut self, id: u32) -> bool { let n = self.elements.len(); self.elements.retain(|e| e.id != id); n != self.elements.len() }
    /// Draw an element in front of (or behind) everything else.
    pub fn restack(&mut self, id: u32, front: bool) {
        let Some(i) = self.index(id) else { return };
        let e = self.elements.remove(i);
        if front { self.elements.push(e); } else { self.elements.insert(0, e); }
    }
    /// Add a motif of `what` (see `ADD`) about the centre, sized to the page,
    /// on top (stems and rings behind everything). Returns its id.
    pub fn add(&mut self, what: &str) -> Option<u32> {
        let c = self.centre;
        let s = (self.width.min(self.height) / 200.0).max(0.2);
        let up = -PI * 0.5;
        let (kind, repeat, back) = match what {
            "palmette" => (Kind::Fan { base: pt(c.x, c.y + 30.0 * s), axis: up, count: 9, spread: 28.0, bend: 0.95, flick: 0.45, length: 70.0 * s, decay: 0.5, belly: 0.15, roll: 0.0, root: 3.0 * s, tip: Tip::Pointed, centre_tip: Tip::Pointed }, Repeat::ONE, false),
            "lotus" => (Kind::Lotus { base: pt(c.x, c.y + 25.0 * s), axis: up, height: 50.0 * s }, Repeat::ONE, false),
            "petal" => (Kind::Petal { base: pt(c.x - 2.0 * s, c.y), heading: PI * 0.5 + 0.9, length: 22.0 * s, belly: 0.22, bend: 0.9, flick: 0.0, roll: 0.0, tip: Tip::Round }, Repeat::MIRROR, false),
            "stem" => (Kind::Stem { top: c, heading: PI * 0.5, length: 30.0 * s, w_top: 2.5 * s, w_foot: 3.5 * s }, Repeat::ONE, true),
            "arm" => (Kind::Arm { start: pt(c.x - 3.0 * s, c.y), heading: PI + 0.15, length: 30.0 * s, curve: 0.0, radius: 15.0 * s, side: -1.0, width: 4.2 * s }, Repeat::MIRROR, true),
            "collar" => (Kind::Collar { c, turn: 0.0, width: 26.0 * s, height: 7.0 * s }, Repeat::ONE, false),
            "boss" => (Kind::Boss { c, r: 5.0 * s }, Repeat::ONE, false),
            "fleurette" => (Kind::Fleurette { c, count: 8, radius: 20.0 * s, belly: 0.2, tip: Tip::Round, back: 0.9, boss: 5.0 * s }, Repeat::ONE, false),
            "husks" => (Kind::Husks { top: pt(c.x, c.y + 10.0 * s), heading: PI * 0.5, count: 3, size: 0.75 * s, shrink: 0.83, tear: false }, Repeat::ONE, false),
            "petal-ring" => (Kind::PetalRing { c, count: 12, root: 12.0 * s, length: 40.0 * s, belly: 0.24, offset: 0.5, notch: true, lip: false }, Repeat::ONE, true),
            "beaded-ring" => (Kind::BeadedRing { c, r: 24.0 * s, width: 6.0 * s, bead: 1.6 * s }, Repeat::ONE, false),
            _ => return None,
        };
        let id = self.push(kind, repeat);
        if back { let e = self.elements.pop().unwrap(); self.elements.insert(0, e); }
        Some(id)
    }
}

/// The Add buttons: what, name, tip.
pub const ADD: [(&str, &str, &str); 11] = [
    ("palmette", "Palmette", "A fan of S-curved petals from one waist, the tall spear in the middle"),
    ("lotus", "Lotus", "Two tall petals curving out and a round bud between"),
    ("petal", "Petal", "A pair of petals turning down and out (a calyx under a waist)"),
    ("stem", "Stem", "A plain stem below the centre, behind everything"),
    ("arm", "Volute arm", "A mirrored pair of arms sweeping out and curling into volutes, behind everything"),
    ("collar", "Collar", "A band across a waist"),
    ("boss", "Boss", "A round boss with a ring"),
    ("fleurette", "Fleurette", "A small flower: a ring of petals over a second ring, on a boss"),
    ("husks", "Husk drop", "Bellflowers hanging one under another, each smaller"),
    ("petal-ring", "Petal ring", "A rosette tier: broad petals round the centre, heart-notched tips, behind everything"),
    ("beaded-ring", "Beaded ring", "A band with beads along it round the centre"),
];

/// The pattern SVG of a drawing at actual size.
pub fn svg_of(d: &Drawing, width: f64, height: f64) -> String {
    format!("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}mm\" height=\"{h}mm\" viewBox=\"0 0 {w} {h}\"><title>ORNATR palmette pattern</title><g fill=\"none\" stroke=\"#000\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><path d=\"{}\" stroke-width=\".35\"/><path d=\"{}\" stroke-width=\".2\"/></g></svg>", path_data(&d.outlines, false), path_data(&d.lines, false), w = width, h = height)
}

fn map_layer(l: &Layer, f: impl Fn(Point) -> Point) -> Layer {
    let m = |v: &Vec<Vec<Point>>| v.iter().map(|r| r.iter().map(|p| f(*p)).collect()).collect::<Vec<Vec<Point>>>();
    Layer { rings: m(&l.rings), lines: m(&l.lines), cover: l.cover.iter().map(m).collect(), field: m(&l.field), pockets: m(&l.pockets) }
}

// ---- the vocabulary (from the palmette study) ------------------------------

/// One drawn piece (the study's layer): outline rings, inner lines, the region
/// hiding what is behind it.
#[derive(Default, Clone)]
struct Piece { rings: Vec<Vec<Point>>, lines: Vec<Vec<Point>>, cover: Vec<Vec<Point>> }
impl Piece {
    fn layer(self) -> Layer { Layer { rings: self.rings, lines: self.lines, cover: self.cover.into_iter().map(|c| vec![c]).collect(), ..Default::default() } }
    fn map(&self, f: &impl Fn(Point) -> Point) -> Piece {
        let m = |v: &Vec<Vec<Point>>| v.iter().map(|r| r.iter().map(|p| f(*p)).collect()).collect::<Vec<Vec<Point>>>();
        Piece { rings: m(&self.rings), lines: m(&self.lines), cover: m(&self.cover) }
    }
}

/// A motif's pieces, back to front.
fn motif(k: &Kind) -> Vec<Piece> {
    match k {
        Kind::Fan { base, axis, count, spread, bend, flick, length, decay, belly, roll, root, tip, centre_tip } => {
            let v = fan_s(*base, (*count).max(1), *spread, *bend, *flick, length.max(0.5), *decay, *belly, *roll, *root, *tip, *centre_tip);
            turned(v, *base, axis + PI * 0.5)
        }
        Kind::Petal { base, heading, length, belly, bend, flick, roll, tip } => vec![petal_s(*base, *heading, length.max(0.5), *belly, *bend, *flick, *roll, *tip)],
        Kind::Lotus { base, axis, height } => turned(lotus(*base, height.max(1.0)), *base, axis + PI * 0.5),
        Kind::Stem { top, heading, length, w_top, w_foot } => vec![stem(*top, *heading, length.max(0.5), w_top.max(0.1), w_foot.max(0.1))],
        Kind::Arm { start, heading, length, curve, radius, side, width } => vec![arm(*start, *heading, length.max(0.0), *curve, radius.max(1.0), if *side < 0.0 { -1.0 } else { 1.0 }, width.max(0.2))],
        Kind::Collar { c, turn, width, height } => turned(vec![collar(*c, width.max(height + 0.5), height.max(0.5))], *c, *turn),
        Kind::Boss { c, r } => vec![boss(*c, r.max(0.3))],
        Kind::Fleurette { c, count, radius, belly, tip, back, boss } => fleurette(*c, (*count).max(2), radius.max(boss * 0.6 + 1.0), *belly, *tip, *back, boss.max(0.3)),
        Kind::Husks { top, heading, count, size, shrink, tear } => turned(husk_drop(*top, (*count).max(1), size.max(0.05), *shrink, *tear), *top, heading - PI * 0.5),
        Kind::PetalRing { c, count, root, length, belly, offset, notch, lip } => petal_ring(*c, (*count).max(2), root.max(0.0), length.max(1.0), *belly, *offset, *notch, *lip),
        Kind::BeadedRing { c, r, width, bead } => beaded_ring(*c, r.max(1.0), width.clamp(0.2, r * 1.8), bead.max(0.2)),
    }
}
/// Pieces turned by `a` about `c` (none when a is 0, so the study's motifs stay exact).
fn turned(v: Vec<Piece>, c: Point, a: f64) -> Vec<Piece> {
    if a.abs() < 1e-12 { return v; }
    let (s, co) = a.sin_cos();
    let f = move |p: Point| { let (dx, dy) = (p.x - c.x, p.y - c.y); pt(c.x + dx * co - dy * s, c.y + dx * s + dy * co) };
    v.iter().map(|p| p.map(&f)).collect()
}

fn closed(r: &[Point]) -> Vec<Point> { let mut v = r.to_vec(); if v.len() > 1 { v.push(v[0]); } v }
fn add(p: Point, h: f64, d: f64) -> Point { pt(p.x - h.sin() * d, p.y + h.cos() * d) }
fn biggest(s: crate::booleans::Shapes) -> Option<Vec<Point>> {
    s.into_iter().map(|sh| sh[0].clone()).max_by(|a, b| signed_area(a).abs().partial_cmp(&signed_area(b).abs()).unwrap())
}

/// Spine samples (point, heading) from a curvature function of arc length.
fn spine(start: Point, heading: f64, length: f64, kappa: impl Fn(f64) -> f64) -> Vec<(Point, f64)> {
    let step = 0.3; let n = (length / step).ceil().clamp(2.0, 20_000.0) as usize; let ds = length / n as f64;
    let (mut p, mut h) = (start, heading); let mut v = vec![(p, h)];
    for i in 0..n { let s = (i as f64 + 0.5) * ds; let k = kappa(s); let hm = h + k * ds * 0.5; p = pt(p.x + hm.cos() * ds, p.y + hm.sin() * ds); h += k * ds; v.push((p, h)); }
    v
}
/// The outline of a band of half-width w(u) along a spine, cleaned of self-overlap.
fn band(sp: &[(Point, f64)], w: impl Fn(f64) -> f64) -> Vec<Point> {
    let n = sp.len(); let u = |i: usize| i as f64 / (n - 1) as f64;
    let mut poly: Vec<Point> = (0..n).map(|i| add(sp[i].0, sp[i].1, w(u(i)))).collect();
    let (pe, he) = sp[n - 1]; let we = w(1.0);
    if we > 0.05 { for k in 1..12 { let a = he + PI * 0.5 - PI * k as f64 / 12.0; poly.push(pt(pe.x + a.cos() * we, pe.y + a.sin() * we)); } }
    poly.extend((0..n).rev().map(|i| add(sp[i].0, sp[i].1, -w(u(i)))));
    let (p0, h0) = sp[0]; let w0 = w(0.0);
    if w0 > 0.05 { for k in 1..12 { let a = h0 - PI * 0.5 - PI * k as f64 / 12.0; poly.push(pt(p0.x + a.cos() * w0, p0.y + a.sin() * w0)); } }
    biggest(union(&[poly.as_slice()])).unwrap_or(poly)
}
fn rail(sp: &[(Point, f64)], w: &dyn Fn(f64) -> f64, f: f64, from: f64, to: f64) -> Vec<Point> {
    let n = sp.len();
    (0..n).filter_map(|i| { let u = i as f64 / (n - 1) as f64; (u >= from && u <= to).then(|| add(sp[i].0, sp[i].1, f * w(u))) }).collect()
}
fn smooth(t: f64) -> f64 { let t = t.clamp(0.0, 1.0); t * t * (3.0 - 2.0 * t) }

/// A petal: from `base` heading `heading`, `len` long, max half-width
/// `belly`·len at 0.62 of its length. Its spine is an S: it bends by `bend`
/// radians over its first 60% (most at the root), then flicks back by
/// `flick` over the rest; `roll` rolls the tip a further amount over its last quarter.
fn petal_s(base: Point, heading: f64, len: f64, belly: f64, bend: f64, flick: f64, roll: f64, tip: Tip) -> Piece {
    let kappa = move |s: f64| {
        let t = s / len;
        let mut k = if t < 0.6 { bend * 2.0 * (1.0 - t / 0.6) / (0.6 * len) } else { -flick * 2.0 * ((t - 0.6) / 0.4) / (0.4 * len) };
        if t > 0.75 { k += roll * 2.0 * (t - 0.75) / 0.25 / (0.25 * len); }
        k
    };
    let sp = spine(base, heading, len, kappa);
    let wmax = belly * len;
    let w = move |u: f64| -> f64 {
        let rise = 0.28 + 0.72 * smooth(u / 0.62).powf(0.8);
        if u <= 0.62 { wmax * rise } else {
            let t = ((u - 0.62) / 0.38).min(1.0);
            match tip { Tip::Pointed => wmax * (1.0 - t).powf(0.75) * (1.0 - 0.15 * t), Tip::Round => wmax * (1.0 - t * t).max(0.0).sqrt() * (1.0 - 0.1 * t) }
        }
    };
    let outline = band(&sp, w);
    // the spoon: a smooth hollow inside the edge, and a midrib on pointed petals
    let inner = offset(&vec![vec![outline.clone()]], -wmax * 0.38);
    let mut lines: Vec<Vec<Point>> = inner.into_iter().flatten().filter(|r| signed_area(r).abs() > wmax * wmax * 0.5).map(|r| closed(&r)).collect();
    if tip == Tip::Pointed { lines.push(rail(&sp, &w, 0.0, 0.1, 0.78)); }
    Piece { rings: vec![closed(&outline)], lines, cover: vec![outline] }
}

/// A fan of `n` (odd) petals about the upright axis through `base`. Back to
/// front: outermost first, the centre last.
fn fan_s(base: Point, n: usize, start: f64, bend: f64, flick: f64, len: f64, decay: f64, belly: f64, roll: f64, root: f64, tip: Tip, centre_tip: Tip) -> Vec<Piece> {
    let k = (n / 2) as i32;
    let mut out = vec![];
    for j in (0..=k).rev() {
        for sgn in if j == 0 { vec![1.0] } else { vec![-1.0, 1.0] } {
            let t = j as f64 / k.max(1) as f64;
            let ang = -PI * 0.5 + sgn * start.to_radians() * t.powf(0.9);
            let l = len * (1.0 - decay * t.powf(1.3));
            let b = belly * (1.0 + 0.1 * t);
            let p = pt(base.x + sgn * root * t, base.y);
            out.push(petal_s(p, ang, l.max(0.5), b, sgn * bend * t.powf(1.1), sgn * flick * t, sgn * roll * t.powf(2.0), if j == 0 { centre_tip } else { tip }));
        }
    }
    out
}

/// A lotus standing on `base` (pointing up): two tall outer petals curving out, a short rounded bud between.
fn lotus(base: Point, h: f64) -> Vec<Piece> {
    // the roots 2 mm apart at the study's 56 mm, in proportion at other sizes
    let (x, y, g) = (base.x, base.y, h / 28.0);
    vec![petal_s(pt(x - g, y), -PI * 0.5 - 0.12, h, 0.17, -0.6, -0.35, -0.5, Tip::Round), petal_s(pt(x + g, y), -PI * 0.5 + 0.12, h, 0.17, 0.6, 0.35, 0.5, Tip::Round),
         petal_s(pt(x, y), -PI * 0.5, h * 0.5, 0.26, 0.0, 0.0, 0.0, Tip::Round)]
}

/// A plain tapered stem from `top` along `heading`, flaring a little at the foot.
fn stem(top: Point, heading: f64, len: f64, w_top: f64, w_foot: f64) -> Piece {
    let sp = spine(top, heading, len, |_| 0.0);
    let o = band(&sp, move |u| w_top + (w_foot - w_top) * u.powf(3.0));
    Piece { rings: vec![closed(&o)], cover: vec![o], ..Default::default() }
}
fn circle(c: Point, r: f64, n: usize) -> Vec<Point> { (0..n).map(|k| { let t = k as f64 / n as f64 * 2.0 * PI; pt(c.x + r * t.cos(), c.y + r * t.sin()) }).collect() }
/// A boss: a round knob with a ring.
fn boss(c: Point, r: f64) -> Piece { Piece { rings: vec![closed(&circle(c, r, 60))], lines: vec![closed(&circle(c, r * 0.6, 60))], cover: vec![circle(c, r, 60)] } }
/// A scroll arm: a sweep of `len` (curvature `c0`), then a volute of outer
/// radius `r` turning to `side`, `w` half-width tapering toward the eye.
fn arm(start: Point, heading: f64, len: f64, c0: f64, r: f64, side: f64, w: f64) -> Piece {
    let eye = r * 0.3; let turns = 1.3 * 2.0 * PI; let rate = (r / eye).ln() / turns; let lc = (r - eye) / rate;
    let total = len + lc;
    let sp = spine(start, heading, total, move |s| if s < len { c0 } else { side / (r - rate * (s - len)).max(eye) });
    let uv = len / total;
    let wf = move |u: f64| if u < uv { w } else { w * (1.0 - 0.7 * smooth((u - uv) / (1.0 - uv))) };
    let o = band(&sp, wf);
    // a channel line along the arm
    Piece { rings: vec![closed(&o)], lines: vec![rail(&sp, &wf, 0.35 * side.signum(), 0.05, 0.97)], cover: vec![o] }
}
/// A collar band across the waist: a rounded bar `w` wide, `h` high, with two moulding lines.
fn collar(c: Point, w: f64, h: f64) -> Piece {
    let sp = spine(pt(c.x - w / 2.0 + h / 2.0, c.y), 0.0, w - h, |_| 0.0);
    let o = band(&sp, move |_| h / 2.0);
    let line = |dy: f64| vec![pt(c.x - w / 2.0 + h * 0.4, c.y + dy), pt(c.x + w / 2.0 - h * 0.4, c.y + dy)];
    Piece { rings: vec![closed(&o)], lines: vec![line(-h * 0.18), line(h * 0.18)], cover: vec![o] }
}
/// A fleurette: `n` petals round `c` (radius `r` from the centre to the
/// tips), a second ring of `n` behind it turned half a step, and a boss.
fn fleurette(c: Point, n: usize, r: f64, belly: f64, tip: Tip, back: f64, boss_r: f64) -> Vec<Piece> {
    let mut v = vec![];
    for (scale, off) in [(back, 0.5), (1.0, 0.0)] {
        if scale <= 0.0 { continue; }
        for k in 0..n {
            let a = -PI * 0.5 + 2.0 * PI * (k as f64 + off) / n as f64;
            let root = boss_r * 0.6;
            v.push(petal_s(pt(c.x + a.cos() * root, c.y + a.sin() * root), a, ((r - root) * scale).max(0.5), belly, 0.0, 0.0, 0.0, tip));
        }
    }
    v.push(boss(c, boss_r));
    v
}
/// One bellflower (husk) hanging from `top`: a centre petal down and two
/// side petals curving down and out, flicking in at the tips.
fn husk(top: Point, s: f64) -> Vec<Piece> {
    let side = petal_s(pt(top.x + 1.5 * s, top.y), PI * 0.5 - 0.5, 26.0 * s, 0.2, -0.5, -0.5, 0.0, Tip::Round);
    let mirrored = side.map(&|p: Point| pt(2.0 * top.x - p.x, p.y));
    vec![mirrored, side, petal_s(top, PI * 0.5, 30.0 * s, 0.22, 0.0, 0.0, 0.0, Tip::Round)]
}
/// A chain of husks hanging down from `top`, each `shrink` of the one above
/// and tucked under it (drawn lowest first), and a teardrop under the last.
fn husk_drop(top: Point, count: usize, size: f64, shrink: f64, tear: bool) -> Vec<Piece> {
    let mut tops = vec![]; let (mut y, mut s) = (top.y, size);
    for _ in 0..count { tops.push((y, s)); y += 24.0 * s; s *= shrink; }
    let mut v = vec![];
    if tear { v.push(petal_s(pt(top.x, y + 2.0 * size), PI * 0.5, 16.0 * size, 0.32, 0.0, 0.0, 0.0, Tip::Pointed)); }
    for &(y, s) in tops.iter().rev() { v.extend(husk(pt(top.x, y), s)); }
    v
}
/// A rosette petal: broad and round, its tip notched into a heart (`notch`),
/// its edge with a raised lip (`lip`: a line close inside the edge) instead of the spoon hollow.
fn petal_r(base: Point, heading: f64, len: f64, belly: f64, notch: bool, lip: bool) -> Piece {
    let sp = spine(base, heading, len, |_| 0.0);
    let wmax = belly * len;
    let w = move |u: f64| -> f64 {
        let rise = 0.3 + 0.7 * smooth(u / 0.6).powf(0.7);
        if u <= 0.6 { wmax * rise } else { let t = ((u - 0.6) / 0.4).min(1.0); wmax * (1.0 - t.powf(2.4)).max(0.0).sqrt() }
    };
    let mut outline = band(&sp, w);
    if notch {
        let (tip, h) = sp[sp.len() - 1];
        let c = pt(tip.x + h.cos() * wmax * 0.05, tip.y + h.sin() * wmax * 0.05);
        let cut = difference(&vec![vec![outline.clone()]], &vec![vec![circle(c, wmax * 0.32, 24)]]);
        if let Some(o) = biggest(cut) { outline = o; }
    }
    let inset = if lip { wmax * 0.14 } else { wmax * 0.38 };
    let inner = offset(&vec![vec![outline.clone()]], -inset);
    let mut lines: Vec<Vec<Point>> = inner.into_iter().flatten().filter(|r| signed_area(r).abs() > wmax * wmax * 0.3).map(|r| closed(&r)).collect();
    // a short crease from the root into the cup
    lines.push(rail(&sp, &w, 0.0, 0.08, 0.45));
    Piece { rings: vec![closed(&outline)], lines, cover: vec![outline] }
}
/// A ring of `n` rosette petals round `c`, roots at `root`, `len` long, turned by `off` steps.
fn petal_ring(c: Point, n: usize, root: f64, len: f64, belly: f64, off: f64, notch: bool, lip: bool) -> Vec<Piece> {
    (0..n).map(|k| { let a = -PI * 0.5 + 2.0 * PI * (k as f64 + off) / n as f64; petal_r(pt(c.x + a.cos() * root, c.y + a.sin() * root), a, len, belly, notch, lip) }).collect()
}
/// A beaded ring: a plain band with beads along its middle.
fn beaded_ring(c: Point, r: f64, w: f64, bead: f64) -> Vec<Piece> {
    let (o, i) = (circle(c, r + w / 2.0, 120), circle(c, r - w / 2.0, 120));
    // the band hides what is behind it, and its middle shows through
    let band = Piece { rings: vec![closed(&o), closed(&i)], cover: vec![o], ..Default::default() };
    let n = ((2.0 * PI * r) / (bead * 2.6)).max(3.0) as usize;
    let mut v = vec![band];
    for k in 0..n { let t = 2.0 * PI * k as f64 / n as f64; v.push(boss(pt(c.x + r * t.cos(), c.y + r * t.sin()), bead)); }
    v
}

// ---- the library structures (the banked studies) ---------------------------

/// The starting layouts: id, name, description.
pub const STRUCTURES: [(&str, &str, &str); 8] = [
    ("classical", "Classical", "Classical palmette (P1): nine lancet petals from a narrow waist, the centre a tall spear; a short stem and a turned-down calyx"),
    ("flame", "Flame", "Flame palmette on a stem (P2): five broad petals, the outer pair rolled over at their tips, a calyx and a foot boss"),
    ("shell", "Shell", "Shell palmette on C-volutes (P3): eleven round-tipped petals in a half-round fan over two volute arms back to back, a boss at the waist"),
    ("anthemion", "Anthemion", "Anthemion band (P4): palmettes alternating with lotus along a row, each on the volutes of a wave linking it to the next"),
    ("fleur", "Fleur-de-lis", "Fleur-de-lis (L1): a centre spear, two side petals arching over with blunt tips, a collar band, a calyx below"),
    ("rich-fleur", "Rich fleur", "Rich fleur-de-lis (L2): a small palmette crown on volute arms, a collar, a calyx on a boss"),
    ("fleurettes", "Fleurettes", "Fleurettes (L3): a soft four-petal flower over four turned petals, and a layered daisy, each on a boss"),
    ("husk-drop", "Husk drop", "Husk drop (L4): a small fleur-de-lis over a ring, then a chain of bellflowers each smaller, ending in a teardrop"),
];
/// The first design, its page size.
pub const START: (&str, f64, f64) = ("classical", 160.0, 200.0);

/// A structure in its own (the study's) coordinates, not fitted to a page.
pub fn structure(id: &str) -> Option<Design> {
    Some(match id {
        "classical" => classical_p1(),
        "flame" => flame_p2(),
        "shell" => shell_p3(),
        "anthemion" => anthemion_p4(),
        "fleur" => fleur_l1(),
        "rich-fleur" => rich_fleur_l2(),
        "fleurettes" => fleurettes_l3(),
        "husk-drop" => husk_drop_l4(),
        _ => return None,
    })
}

const UP: f64 = -PI * 0.5;
const DOWN: f64 = PI * 0.5;
fn design(centre: Point) -> Design { Design { width: 0.0, height: 0.0, centre, elements: vec![] } }
fn fan(base: Point, count: usize, spread: f64, bend: f64, flick: f64, length: f64, decay: f64, belly: f64, roll: f64, root: f64, tip: Tip, centre_tip: Tip) -> Kind {
    Kind::Fan { base, axis: UP, count, spread, bend, flick, length, decay, belly, roll, root, tip, centre_tip }
}
fn petal(base: Point, heading: f64, length: f64, belly: f64, bend: f64, flick: f64, tip: Tip) -> Kind { Kind::Petal { base, heading, length, belly, bend, flick, roll: 0.0, tip } }
// Mirrored pairs are stored as the half the study drew first; its mirror
// image (the copy) is the half drawn over it, as in the study.

/// P1: classical palmette (after #5, #7, #49).
fn classical_p1() -> Design {
    let mut d = design(pt(100.0, 100.0));
    d.push(Kind::Stem { top: pt(100.0, 116.0), heading: DOWN, length: 22.0, w_top: 4.0, w_foot: 6.0 }, Repeat::ONE);
    // calyx: two small petals turned down and out from the waist
    d.push(petal(pt(98.0, 122.0), DOWN + 0.9, 18.0, 0.22, 0.9, 0.0, Tip::Pointed), Repeat::MIRROR);
    d.push(fan(pt(100.0, 120.0), 9, 28.0, 0.95, 0.45, 95.0, 0.5, 0.15, 0.0, 4.0, Tip::Pointed, Tip::Pointed), Repeat::ONE);
    d
}
/// P2: flame palmette on a stem (after #4, #8, #47).
fn flame_p2() -> Design {
    let mut d = design(pt(100.0, 100.0));
    d.push(Kind::Stem { top: pt(100.0, 104.0), heading: DOWN, length: 40.0, w_top: 2.8, w_foot: 4.0 }, Repeat::ONE);
    d.push(Kind::Boss { c: pt(100.0, 146.0), r: 6.0 }, Repeat::ONE);
    d.push(petal(pt(98.0, 112.0), DOWN + 0.7, 22.0, 0.24, 0.9, 0.4, Tip::Round), Repeat::MIRROR);
    d.push(fan(pt(100.0, 110.0), 5, 22.0, 1.0, 0.2, 80.0, 0.3, 0.21, 1.4, 4.0, Tip::Round, Tip::Pointed), Repeat::ONE);
    d
}
/// P3: shell palmette on C-volutes (after #52, #81, #90).
fn shell_p3() -> Design {
    let mut d = design(pt(100.0, 100.0));
    d.push(Kind::Arm { start: pt(99.0, 118.0), heading: PI + 0.15, length: 30.0, curve: 0.0, radius: 15.0, side: -1.0, width: 4.2 }, Repeat::MIRROR);
    d.push(fan(pt(100.0, 112.0), 11, 40.0, 0.8, 0.15, 72.0, 0.25, 0.17, 0.6, 5.0, Tip::Round, Tip::Round), Repeat::ONE);
    d.push(Kind::Boss { c: pt(100.0, 116.0), r: 6.0 }, Repeat::ONE);
    d
}
/// P4: anthemion band. Palmettes alternating with lotus along a baseline,
/// each standing on a pair of volutes: the ends of the waves between them.
fn anthemion_p4() -> Design {
    let (y, pitch) = (110.0, 80.0);
    let mut d = design(pt(30.0 + pitch * 1.5, y));
    d.push(Kind::Stem { top: pt(30.0, y), heading: DOWN, length: 12.0, w_top: 2.2, w_foot: 3.2 }, Repeat::row(4, pitch));
    // each gap holds one wave: two arms meet at its low point and curl up beside the motifs either side
    let mid = 30.0 + pitch * 0.5;
    d.push(Kind::Arm { start: pt(mid - 0.5, y + 10.0), heading: 0.1, length: pitch / 2.0 - 26.0, curve: 0.0, radius: 12.5, side: -1.0, width: 3.0 }, Repeat { mirror_x: true, ..Repeat::row(3, pitch) });
    d.push(fan(pt(30.0, y), 7, 24.0, 0.9, 0.4, 66.0, 0.5, 0.15, 0.2, 3.0, Tip::Pointed, Tip::Pointed), Repeat::row(2, pitch * 2.0));
    d.push(Kind::Lotus { base: pt(30.0 + pitch, y), axis: UP, height: 56.0 }, Repeat::row(2, pitch * 2.0));
    d.push(Kind::Boss { c: pt(30.0, y + 2.0), r: 3.5 }, Repeat::row(4, pitch));
    d
}
/// L1: fleur-de-lis (after #37).
fn fleur_l1() -> Design {
    let (cx, wy) = (100.0, 110.0);
    let mut d = design(pt(cx, wy));
    d.push(petal(pt(cx - 4.0, wy + 1.0), UP - 0.3, 80.0, 0.15, -1.1, 2.6, Tip::Round), Repeat::MIRROR);
    d.push(petal(pt(cx - 3.0, wy + 4.0), DOWN + 0.45, 30.0, 0.18, 1.7, 0.3, Tip::Round), Repeat::MIRROR);
    d.push(petal(pt(cx, wy + 3.0), DOWN, 38.0, 0.17, 0.0, 0.0, Tip::Pointed), Repeat::ONE);
    d.push(petal(pt(cx, wy + 2.0), UP, 95.0, 0.2, 0.0, 0.0, Tip::Pointed), Repeat::ONE);
    d.push(Kind::Collar { c: pt(cx, wy + 2.0), turn: 0.0, width: 30.0, height: 8.0 }, Repeat::ONE);
    d
}
/// L2: rich fleur-de-lis (after #83, #64, #65).
fn rich_fleur_l2() -> Design {
    let (cx, wy) = (100.0, 110.0);
    let mut d = design(pt(cx, wy));
    d.push(Kind::Arm { start: pt(cx - 5.0, wy - 4.0), heading: PI + 1.0, length: 22.0, curve: -0.025, radius: 15.0, side: -1.0, width: 4.6 }, Repeat::MIRROR);
    d.push(fan(pt(cx, wy - 4.0), 7, 22.0, 0.8, 0.35, 58.0, 0.45, 0.16, 0.1, 3.0, Tip::Pointed, Tip::Pointed), Repeat::ONE);
    d.push(petal(pt(cx - 3.0, wy + 4.0), DOWN + 0.5, 34.0, 0.19, 1.9, 0.3, Tip::Round), Repeat::MIRROR);
    d.push(petal(pt(cx, wy + 3.0), DOWN, 30.0, 0.2, 0.0, 0.0, Tip::Pointed), Repeat::ONE);
    d.push(Kind::Collar { c: pt(cx, wy + 2.0), turn: 0.0, width: 26.0, height: 8.0 }, Repeat::ONE);
    d.push(Kind::Boss { c: pt(cx, wy + 36.0), r: 4.5 }, Repeat::ONE);
    d
}
/// L3: fleurettes (after #2, #14).
fn fleurettes_l3() -> Design {
    let mut d = design(pt(115.0, 60.0));
    d.push(Kind::Fleurette { c: pt(60.0, 60.0), count: 4, radius: 48.0, belly: 0.42, tip: Tip::Round, back: 0.78, boss: 9.0 }, Repeat::ONE);
    d.push(Kind::Fleurette { c: pt(170.0, 60.0), count: 8, radius: 46.0, belly: 0.15, tip: Tip::Round, back: 0.92, boss: 8.0 }, Repeat::ONE);
    d
}
/// L4: husk drop (after #42, #126).
fn husk_drop_l4() -> Design {
    let cx = 100.0;
    let mut d = design(pt(cx, 100.0));
    d.push(Kind::Husks { top: pt(cx, 70.0), heading: DOWN, count: 4, size: 1.0, shrink: 0.85, tear: true }, Repeat::ONE);
    d.push(Kind::Boss { c: pt(cx, 66.0), r: 5.0 }, Repeat::ONE);
    // the small fleur on top
    d.push(petal(pt(cx - 2.0, 56.0), UP - 0.55, 30.0, 0.16, -2.2, 0.5, Tip::Round), Repeat::MIRROR);
    d.push(petal(pt(cx, 57.0), UP, 36.0, 0.18, 0.0, 0.0, Tip::Pointed), Repeat::ONE);
    d.push(Kind::Collar { c: pt(cx, 57.0), turn: 0.0, width: 13.0, height: 4.0 }, Repeat::ONE);
    d
}
