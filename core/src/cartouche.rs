//! Cartouche designs, as edited in the app's Cartouche workspace.
//!
//! A cartouche is ornament organised round a central field (see
//! ORNAMENT_FAMILIES.md, Cartouche). A design is a list of elements drawn
//! back to front: the **frame** (the field and its moulding, centred on the
//! design's centre), **scrolls** (acanthus grown by the scroll engine along a
//! Bézier stem, clad in the vine acanthus leaf, exact joins), library **leaf
//! fans** on the scrolls, and accents: **jewels**, **bosses**, **shells**,
//! flat **straps** rolling into volutes and pierced **openings**.
//!
//! Every element but the frame and leaves can repeat round the centre:
//! mirrored left and right, top and bottom, both, or turned round a ring. The
//! copies are made from the one element, so editing it (or any copy) changes
//! them all. A leaf fan repeats with its scroll. All the scrolls grow as one
//! carved body (one layout), drawn where the first scroll stands in the stack.
//!
//! The starting layouts are the banked cartouche studies (B4, E1, E3, E4,
//! F1–F3, G1–G3), built here from the study code (`cartouche_study.rs`) in its
//! own coordinates and fitted to the page.
use crate::booleans::{offset, union, Shapes};
use crate::geometry::{arc_table, distance, pt, Bounds, Curve, Point};
use crate::growth::{Family, GrowthResult, GrowthSettings, Side};
use crate::model::{preset_params, JoinStyle, Layout};
use crate::outline::{inside, intersection, path_data};
use crate::rocaille::{self, Part};
use crate::shoots::ShootEdit;
use std::f64::consts::PI;

// ---- symmetry -------------------------------------------------------------

/// How an element repeats round the design's centre: mirrored across the
/// upright axis (`mirror_x`), across the level one (`mirror_y`), and turned
/// round a ring of `ring` places. The element itself is the first copy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Repeat { pub mirror_x: bool, pub mirror_y: bool, pub ring: u8 }
impl Repeat {
    pub const ONE: Repeat = Repeat { mirror_x: false, mirror_y: false, ring: 1 };
    /// Left and right.
    pub const MIRROR: Repeat = Repeat { mirror_x: true, mirror_y: false, ring: 1 };
    /// Top and bottom.
    pub const UPDOWN: Repeat = Repeat { mirror_x: false, mirror_y: true, ring: 1 };
    /// All four quarters.
    pub const FOUR: Repeat = Repeat { mirror_x: true, mirror_y: true, ring: 1 };
    pub fn count(&self) -> usize {
        let m = match (self.mirror_x, self.mirror_y) { (false, false) => 1, (true, true) => 4, _ => 2 };
        m * self.ring.max(1) as usize
    }
}

/// How far a library leaf bends with its stem unless set (as the studies grew them).
pub const LEAF_FOLLOW: f64 = 0.6;

/// One copy's placement: an isometry about the design's centre `c`
/// (p → c + m·(p − c)).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Xf { pub m: [f64; 4], pub c: Point }
impl Xf {
    pub fn identity(c: Point) -> Xf { Xf { m: [1.0, 0.0, 0.0, 1.0], c } }
    pub fn point(&self, p: Point) -> Point {
        let (dx, dy) = (p.x - self.c.x, p.y - self.c.y);
        pt(self.c.x + self.m[0] * dx + self.m[1] * dy, self.c.y + self.m[2] * dx + self.m[3] * dy)
    }
    /// The point this copy puts at `p` (the matrix is orthogonal: its inverse is its transpose).
    pub fn back(&self, p: Point) -> Point {
        let (dx, dy) = (p.x - self.c.x, p.y - self.c.y);
        pt(self.c.x + self.m[0] * dx + self.m[2] * dy, self.c.y + self.m[1] * dx + self.m[3] * dy)
    }
    pub fn angle(&self, a: f64) -> f64 { let (s, c) = a.sin_cos(); (self.m[2] * c + self.m[3] * s).atan2(self.m[0] * c + self.m[1] * s) }
    pub fn back_angle(&self, a: f64) -> f64 { let (s, c) = a.sin_cos(); (self.m[1] * c + self.m[3] * s).atan2(self.m[0] * c + self.m[2] * s) }
    /// Whether this copy is a mirror image.
    pub fn reflects(&self) -> bool { self.m[0] * self.m[3] - self.m[1] * self.m[2] < 0.0 }
    pub fn curve(&self, c: &Curve) -> Curve { [self.point(c[0]), self.point(c[1]), self.point(c[2]), self.point(c[3])] }
}
/// The copies of `r` about `c`, in order: for each ring place, the element,
/// then mirrored left-right, both ways, and top-bottom (the order the studies
/// listed their stems in, so the seeds match).
pub fn copies(r: Repeat, c: Point) -> Vec<Xf> {
    let mut mirrors = vec![[1.0, 0.0, 0.0, 1.0]];
    if r.mirror_x { mirrors.push([-1.0, 0.0, 0.0, 1.0]); }
    if r.mirror_x && r.mirror_y { mirrors.push([-1.0, 0.0, 0.0, -1.0]); }
    if r.mirror_y { mirrors.push([1.0, 0.0, 0.0, -1.0]); }
    let n = r.ring.max(1);
    let mut out = vec![];
    for k in 0..n {
        let (s, co) = (360.0 * k as f64 / n as f64).to_radians().sin_cos();
        for v in &mirrors {
            // the ring turn after the mirror
            out.push(Xf { m: [co * v[0] - s * v[2], co * v[1] - s * v[3], s * v[0] + co * v[2], s * v[1] + co * v[3]], c });
        }
    }
    out
}

// ---- the model ------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldShape { Oval, Rect, Shield }
/// The frame's moulding: a rounded roll stepping down to the field
/// (moulded), a flat bevelled strap, or no moulding, only a bead round the field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Moulding { Moulded, Strap, Bead }

#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    /// The field (half-width `rx`, half-height `ry`, corner radius `round` for
    /// a rectangle) centred on the design's centre, in a moulding `width` wide;
    /// a row of beads `bead` mm in radius inside its lip (0: none).
    Frame { shape: FieldShape, rx: f64, ry: f64, round: f64, width: f64, moulding: Moulding, bead: f64 },
    /// An acanthus scroll grown along a cubic Bézier stem. `curl`: the side the
    /// volute turns to; `scale`: its size against the main scrolls; `levels`:
    /// generations of side shoots; `flip`: the growth mirrored; `attach`: the
    /// scroll it grows out of (its start sits on that stem, with a collar);
    /// `seed`: the variation; `volute`: the volute shaped by hand (size and
    /// turns against the automatic one), None automatic.
    Scroll { curve: Curve, curl: Side, scale: f64, levels: u8, flip: bool, attach: Option<u32>, seed: u32, volute: Option<(f64, f64)> },
    /// A library leaf on scroll `stem` at `at` (0 to 1 along its stem), on
    /// `side` (±1), `fan` leaves from the node (1–3), `size` against the
    /// leaf's own; it repeats with its scroll, on the mirrored side in mirrored
    /// copies when `mirror_side`.
    /// `turn`: swung from the leaf's own angle (radians, + toward its side's
    /// turn); `bend`, `width`: the leaf bent and widened as in the scroll
    /// workspace; `follow`: how far it bends with the stem (0 to 1).
    Leaf { stem: u32, preset: String, at: f64, side: f64, fan: u8, size: f64, mirror_side: bool, turn: f64, bend: f64, width: f64, follow: f64 },
    /// A jewel: an oval stone (half-axes `rx`, `ry`, turned by `turn`) in a bezel `bezel` mm wide.
    Jewel { c: Point, rx: f64, ry: f64, turn: f64, bezel: f64 },
    /// A round boss with a line round its crown.
    Boss { c: Point, r: f64 },
    /// A fluted shell from its hinge `c` (see `rocaille::shell`).
    Shell { c: Point, axis: f64, span: f64, ribs: usize, r: f64, asym: f64, twist: f64, scallop: f64 },
    /// A flat bevelled strap through `ctrl`, `width` its half-width, rolling
    /// into a volute at each end where `hook` is set (±1, relative to the strap's
    /// own turn there; `eye` the volute's size).
    Strap { ctrl: Vec<Point>, width: f64, hook0: f64, eye0: f64, hook1: f64, eye1: f64 },
    /// A pierced opening: an oval hollow with a lip.
    Opening { c: Point, rx: f64, ry: f64, turn: f64 },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Element { pub id: u32, pub kind: Kind, pub repeat: Repeat }
impl Element {
    pub fn label(&self) -> &'static str {
        match self.kind { Kind::Frame { .. } => "Frame", Kind::Scroll { .. } => "Scroll", Kind::Leaf { .. } => "Leaf fan", Kind::Jewel { .. } => "Jewel", Kind::Boss { .. } => "Boss", Kind::Shell { .. } => "Shell", Kind::Strap { .. } => "Strap", Kind::Opening { .. } => "Opening" }
    }
    pub fn is_scroll(&self) -> bool { matches!(self.kind, Kind::Scroll { .. }) }
    /// Part of the grown acanthus (a scroll or a leaf on one).
    pub fn is_acanthus(&self) -> bool { matches!(self.kind, Kind::Scroll { .. } | Kind::Leaf { .. }) }
    /// Whether it has copies of its own (the frame is centred; a leaf repeats with its scroll).
    pub fn repeats(&self) -> bool { !matches!(self.kind, Kind::Frame { .. } | Kind::Leaf { .. }) }
}

/// A cartouche design on a page (mm). `growth`: the scroll engine's page
/// scale (the scrolls grow on a 240·growth × 150·growth page: larger is
/// heavier); `collars`: paired-leaf collars where a scroll grows out of
/// another; `eyes`: slit-and-eye cuts in the leaves.
#[derive(Clone, Debug, PartialEq)]
pub struct Design { pub width: f64, pub height: f64, pub centre: Point, pub growth: f64, pub collars: bool, pub eyes: bool, pub elements: Vec<Element> }

/// Who a drawn layer belongs to: an element's copy, or the grown acanthus.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Owner { Element(u32, usize), Acanthus }

/// One drawing layer: outline rings, inner lines, what it hides of the layers
/// behind it (shapes: outer ring then holes), its field and its pockets.
#[derive(Clone, Debug, Default)]
pub struct Layer { pub rings: Vec<Vec<Point>>, pub lines: Vec<Vec<Point>>, pub cover: Shapes, pub field: Vec<Vec<Point>>, pub pockets: Vec<Vec<Point>> }
impl Layer {
    fn map(&self, f: impl Fn(Point) -> Point) -> Layer {
        let m = |v: &Vec<Vec<Point>>| v.iter().map(|r| r.iter().map(|p| f(*p)).collect()).collect::<Vec<Vec<Point>>>();
        Layer { rings: m(&self.rings), lines: m(&self.lines), cover: self.cover.iter().map(m).collect(), field: m(&self.field), pockets: m(&self.pockets) }
    }
    pub fn bounds(&self) -> Option<Bounds> {
        let pts: Vec<Point> = self.cover.iter().flatten().flatten().chain(self.rings.iter().flatten()).copied().collect();
        (!pts.is_empty()).then(|| Bounds::of(&pts))
    }
}

/// The scrolls and leaves as a scroll-engine layout: backbone `i` is copy
/// `owners[i].1` of scroll `owners[i].0`; shoot edit `k` (id `leaf-k`) is
/// copy `leaves[k].1` of leaf `leaves[k].0`.
#[derive(Clone, Debug)]
pub struct Acanthus { pub layout: Layout, pub owners: Vec<(u32, usize)>, pub leaves: Vec<(u32, usize)> }

/// What a design looks like: visible outlines and inner lines, and the
/// filled preview's paint, back to front (each entry's rings fill even-odd).
#[derive(Clone, Debug, Default)]
pub struct Drawing { pub outlines: Vec<Vec<Point>>, pub lines: Vec<Vec<Point>>, pub paint: Vec<(Paint, Vec<Vec<Point>>)> }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Paint { Wood, Field, Pocket }

impl Design {
    pub fn empty(width: f64, height: f64) -> Design { Design { width, height, centre: pt(width / 2.0, height / 2.0), growth: 1.0, collars: true, eyes: false, elements: vec![] } }
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
    pub fn frame(&self) -> Option<&Element> { self.elements.iter().find(|e| matches!(e.kind, Kind::Frame { .. })) }

    /// The copies of an element (a leaf's are its scroll's; the frame has one).
    pub fn copies_of(&self, e: &Element) -> Vec<Xf> {
        match &e.kind {
            Kind::Frame { .. } => vec![Xf::identity(self.centre)],
            Kind::Leaf { stem, .. } => match self.get(*stem) { Some(s) if s.is_scroll() => copies(s.repeat, self.centre), _ => vec![] },
            _ => copies(e.repeat, self.centre),
        }
    }

    // ---- the grown acanthus ----
    /// Attached scrolls start on their parent's stem: each moves (keeping its
    /// shape) so its start sits on the nearest point of its parent. Returns
    /// whether anything moved.
    pub fn settle(&mut self) -> bool {
        let mut moved = false;
        for _ in 0..6 {
            let mut any = false;
            for i in 0..self.elements.len() {
                let Kind::Scroll { attach: Some(p), curve, .. } = &self.elements[i].kind else { continue };
                let Some(Kind::Scroll { curve: pc, .. }) = self.get(*p).filter(|e| e.id != self.elements[i].id).map(|e| &e.kind) else { continue };
                let start = curve[0];
                let Some(q) = arc_table(pc).into_iter().map(|r| r.point).min_by(|a, b| distance(*a, start).partial_cmp(&distance(*b, start)).unwrap()) else { continue };
                let (dx, dy) = (q.x - start.x, q.y - start.y);
                if dx.abs() + dy.abs() <= 1e-9 { continue; }
                if let Kind::Scroll { curve, .. } = &mut self.elements[i].kind { for c in curve.iter_mut() { *c = pt(c.x + dx, c.y + dy); } }
                any = true;
            }
            if !any { break; }
            moved = true;
        }
        moved
    }
    /// The handle on copy `copy` of scroll `id`'s volute, as grown.
    pub fn volute_handle(&self, id: u32, copy: usize) -> Option<crate::growth::VoluteHandle> {
        let a = self.acanthus()?;
        a.layout.volute_handle(a.owners.iter().position(|o| *o == (id, copy))?)
    }
    /// The scrolls and their leaves as one scroll-engine layout (None without scrolls).
    pub fn acanthus(&self) -> Option<Acanthus> {
        let scrolls: Vec<&Element> = self.elements.iter().filter(|e| e.is_scroll()).collect();
        if scrolls.is_empty() { return None; }
        let mut index = std::collections::HashMap::new();
        let mut owners = vec![];
        for e in &scrolls { for k in 0..e.repeat.count() { index.insert((e.id, k), owners.len()); owners.push((e.id, k)); } }
        let mut l = Layout::starter();
        l.width = 240.0 * self.growth; l.height = 150.0 * self.growth; // growth scales with the page
        l.curves.clear(); l.growth.clear();
        for e in &scrolls {
            let Kind::Scroll { curve, curl, scale, levels, flip, attach, seed, volute } = &e.kind else { continue };
            for (k, xf) in copies(e.repeat, self.centre).iter().enumerate() {
                let attach = attach.and_then(|p| index.get(&(p, k)).copied());
                l.curves.push(xf.curve(curve));
                l.growth.push(GrowthSettings { seed: seed.wrapping_add(k as u32 * 7919), family: Some(Family::Spiral), side: *curl, levels: *levels,
                    auto_shoots: Some(*levels > 0), secondary_scale: Some(*scale), flip: (*flip != xf.reflects()).then_some(true), free: Some(true), attach, vine_leaf: Some(true), stem: 2.8,
                    collar: (self.collars && attach.is_some()).then_some(1.0), collar_style: (self.collars && attach.is_some()).then(|| "pair".to_string()),
                    eyes: self.eyes.then_some(2), volute_size: volute.map(|v| v.0), volute_turns: volute.map(|v| v.1), ..GrowthSettings::default() });
            }
        }
        let mut leaves = vec![];
        for e in &self.elements {
            let Kind::Leaf { stem, preset, at, side, fan, size, mirror_side, turn, bend, width, follow } = &e.kind else { continue };
            for (k, xf) in self.copies_of(e).iter().enumerate() {
                let Some(&b) = index.get(&(*stem, k)) else { continue };
                let s = if *mirror_side && xf.reflects() { -side } else { *side };
                let Some(mut p) = preset_params(preset, at.clamp(0.0, 1.0), s) else { continue };
                if *fan > 1 { p.fan = Some((*fan).min(3)); }
                p.reach *= size;
                // a mirrored copy swings the other way (as its side does)
                p.turn += turn * s * side.signum();
                if *bend != 0.0 { p.bend = Some(*bend); }
                if *width != 1.0 { p.leaf_scale = Some(*width); }
                p.follow = (*follow > 0.0).then_some(follow.min(1.0));
                l.shoots.push(ShootEdit { params: p, id: format!("leaf-{}", leaves.len()), backbone: b, replaces: None, hidden: false, under: false });
                leaves.push((e.id, k));
            }
        }
        let mut rounds = 0; while rounds < 6 && l.settle() { rounds += 1; }
        Some(Acanthus { layout: l, owners, leaves })
    }

    // ---- drawing ----
    /// The layers of every element but the scrolls and leaves, per copy; the
    /// grown acanthus goes in where the first scroll stands.
    pub fn layers(&self, acanthus: Option<&Layer>) -> Vec<(Owner, Layer)> {
        let mut out = vec![];
        let mut placed = false;
        for e in &self.elements {
            if e.is_acanthus() {
                if e.is_scroll() && !placed { if let Some(a) = acanthus { out.push((Owner::Acanthus, a.clone())); } placed = true; }
                continue;
            }
            for (k, l) in self.element_layers(e).into_iter().enumerate() { for layer in l { out.push((Owner::Element(e.id, k), layer)); } }
        }
        out
    }
    /// One element's layers, per copy (none for scrolls and leaves).
    pub fn element_layers(&self, e: &Element) -> Vec<Vec<Layer>> {
        let own = match &e.kind {
            Kind::Frame { shape, rx, ry, round, width, moulding, bead } => {
                let field = field_outline(*shape, self.centre, *rx, *ry, *round);
                let mut v = vec![frame_layer(&field, *width, *moulding)];
                if *bead > 0.0 { v.push(beads(&grow_ring(&field, bead * 2.0 / 1.1), *bead, bead * 3.2 / 1.1)); }
                return vec![v];
            }
            Kind::Scroll { .. } | Kind::Leaf { .. } => return vec![],
            Kind::Jewel { c, rx, ry, turn, bezel } => from_parts(&jewel(*c, *rx, *ry, *turn, *bezel)),
            Kind::Boss { c, r } => from_parts(&[boss(*c, *r)]),
            Kind::Shell { c, axis, span, ribs, r, asym, twist, scallop } => from_parts(&[rocaille::shell(*c, *axis, *span, (*ribs).max(3), *r, *asym, *twist, *scallop)]),
            Kind::Strap { ctrl, width, hook0, eye0, hook1, eye1 } => { if ctrl.len() < 2 { return vec![]; } from_parts(&[strap(ctrl, *hook0, *eye0, *hook1, *eye1, *width)]) }
            Kind::Opening { c, rx, ry, turn } => from_parts(&[rocaille::pocket(turned_oval(*c, *rx, *ry, *turn), 1.0)]),
        };
        self.copies_of(e).iter().enumerate().map(|(k, xf)| if k == 0 && *xf == Xf::identity(self.centre) { own.clone() } else { own.iter().map(|l| l.map(|p| xf.point(p))).collect() }).collect()
    }
    /// Everything drawn, the acanthus grown (exact joins).
    pub fn drawing(&self) -> Drawing { self.drawing_with(JoinStyle::Exact(0.8)) }
    pub fn drawing_with(&self, joins: JoinStyle) -> Drawing {
        let a = self.acanthus().map(|a| acanthus_layer(&a.layout.grow(), joins));
        compose(&self.layers(a.as_ref()))
    }
    /// The pattern SVG at actual size: visible outlines and inner lines.
    pub fn svg(&self) -> String { svg_of(&self.drawing(), self.width, self.height) }
    /// The bounds of everything drawn.
    pub fn bounds(&self) -> Option<Bounds> {
        let a = self.acanthus().map(|a| draft_layer(&a.layout.grow()));
        bounds_of(&self.layers(a.as_ref()))
    }

    // ---- page ----
    /// Scale by `k` about the origin, then move by `shift`.
    pub fn map(&mut self, k: f64, shift: Point) {
        let mp = |p: Point| pt(p.x * k + shift.x, p.y * k + shift.y);
        self.centre = mp(self.centre);
        self.growth *= k;
        for e in self.elements.iter_mut() {
            match &mut e.kind {
                Kind::Frame { rx, ry, round, width, bead, .. } => { *rx *= k; *ry *= k; *round *= k; *width *= k; *bead *= k; }
                Kind::Scroll { curve, .. } => for c in curve.iter_mut() { *c = mp(*c); },
                Kind::Leaf { .. } => {}
                Kind::Jewel { c, rx, ry, bezel, .. } => { *c = mp(*c); *rx *= k; *ry *= k; *bezel *= k; }
                Kind::Boss { c, r } => { *c = mp(*c); *r *= k; }
                Kind::Shell { c, r, .. } => { *c = mp(*c); *r *= k; }
                Kind::Strap { ctrl, width, eye0, eye1, .. } => { for p in ctrl.iter_mut() { *p = mp(*p); } *width *= k; *eye0 *= k; *eye1 *= k; }
                Kind::Opening { c, rx, ry, .. } => { *c = mp(*c); *rx *= k; *ry *= k; }
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
    /// Remove an element. A scroll takes its leaves with it, and scrolls that
    /// grew out of it stand free. Returns how many elements went.
    pub fn remove(&mut self, id: u32) -> usize {
        let before = self.elements.len();
        let scroll = self.get(id).is_some_and(|e| e.is_scroll());
        self.elements.retain(|e| e.id != id && !(scroll && matches!(e.kind, Kind::Leaf { stem, .. } if stem == id)));
        if scroll { for e in self.elements.iter_mut() { if let Kind::Scroll { attach, .. } = &mut e.kind { if *attach == Some(id) { *attach = None; } } } }
        before - self.elements.len()
    }
    /// Draw an element in front of (or behind) everything else. The scrolls
    /// and leaves are one carved body: they move together.
    pub fn restack(&mut self, id: u32, front: bool) {
        let Some(i) = self.index(id) else { return };
        let group: Vec<Element> = if self.elements[i].is_acanthus() {
            let (g, rest): (Vec<Element>, Vec<Element>) = std::mem::take(&mut self.elements).into_iter().partition(|e| e.is_acanthus());
            self.elements = rest; g
        } else { vec![self.elements.remove(i)] };
        if front { self.elements.extend(group); } else { self.elements.splice(0..0, group); }
    }
    /// Where new acanthus goes in the stack: after the last scroll or leaf (else on top).
    fn acanthus_slot(&self) -> usize { self.elements.iter().rposition(|e| e.is_acanthus()).map_or(self.elements.len(), |i| i + 1) }
    fn insert_acanthus(&mut self, kind: Kind, repeat: Repeat) -> u32 {
        let id = self.next_id(); let at = self.acanthus_slot();
        self.elements.insert(at, Element { id, kind, repeat });
        id
    }
    /// The frame's half-sizes and moulding width (or a box in the middle of the page).
    pub fn frame_size(&self) -> (f64, f64, f64) {
        match self.frame().map(|e| &e.kind) {
            Some(Kind::Frame { rx, ry, width, .. }) => (*rx, *ry, *width),
            _ => { let m = self.width.min(self.height); (m * 0.25, m * 0.32, m * 0.04) }
        }
    }
    /// A moulded oval frame round the centre (when there is none).
    pub fn add_frame(&mut self) -> Option<u32> {
        if self.frame().is_some() { return None; }
        let (rx, ry, w) = self.frame_size();
        let id = self.next_id();
        self.elements.insert(0, Element { id, kind: Kind::Frame { shape: FieldShape::Oval, rx, ry, round: 0.0, width: w, moulding: Moulding::Moulded, bead: 0.0 }, repeat: Repeat::ONE });
        Some(id)
    }
    /// A mirrored pair of C-scrolls riding the frame's top, back to back,
    /// their volutes tucking down onto the moulding.
    pub fn add_scroll(&mut self) -> u32 {
        let (rx, ry, w) = self.frame_size();
        let c = self.centre;
        let (a, b) = (pt(c.x + rx * 0.18, c.y - ry - w * 0.8), pt(c.x + rx, c.y - ry * 0.97));
        let mut s = st(a, -10.0, b, 70.0, Side::Right); s.k0 = 0.45; s.k1 = 0.45;
        let seed = 7919u32.wrapping_mul(self.next_id()).wrapping_add(17);
        self.insert_acanthus(Kind::Scroll { curve: bez(&s), curl: Side::Right, scale: 0.85, levels: 1, flip: false, attach: None, seed, volute: None }, Repeat::MIRROR)
    }
    /// A fan of returning leaves near the start of scroll `stem`.
    pub fn add_leaf(&mut self, stem: u32) -> Option<u32> {
        if !self.get(stem)?.is_scroll() { return None; }
        Some(self.insert_acanthus(Kind::Leaf { stem, preset: "returning-leaf".into(), at: 0.15, side: 1.0, fan: 2, size: 1.6, mirror_side: true, turn: 0.0, bend: 0.0, width: 1.0, follow: LEAF_FOLLOW }, Repeat::ONE))
    }
    /// An accent of `what` ("jewel", "boss", "shell", "strap", "opening") at
    /// the top of the frame, on top of everything (straps and openings behind).
    pub fn add_accent(&mut self, what: &str) -> Option<u32> {
        let (rx, ry, w) = self.frame_size();
        let c = self.centre;
        let top = pt(c.x, c.y - ry - w * 0.5);
        let s = (rx.min(ry) / 50.0).max(0.2);
        let (kind, back) = match what {
            "jewel" => (Kind::Jewel { c: top, rx: 6.0 * s, ry: 8.5 * s, turn: 0.0, bezel: 2.0 * s }, false),
            "boss" => (Kind::Boss { c: pt(c.x + rx, c.y), r: 5.0 * s }, false),
            "shell" => (Kind::Shell { c: top, axis: -PI * 0.5, span: PI * 0.9, ribs: 9, r: 16.0 * s, asym: 0.0, twist: 0.0, scallop: 0.14 }, false),
            "strap" => (Kind::Strap { ctrl: vec![pt(c.x - rx * 0.5, c.y - ry - w * 0.3), pt(c.x - rx * 0.46, c.y - ry - 20.0 * s), pt(c.x, c.y - ry - 32.0 * s), pt(c.x + rx * 0.46, c.y - ry - 20.0 * s), pt(c.x + rx * 0.5, c.y - ry - w * 0.3)], width: 4.4 * s, hook0: 1.0, eye0: 8.0 * s, hook1: 1.0, eye1: 8.0 * s }, true),
            "opening" => (Kind::Opening { c: pt(c.x, c.y - ry - 18.0 * s), rx: 20.0 * s, ry: 14.0 * s, turn: 0.0 }, true),
            _ => return None,
        };
        let repeat = if what == "boss" { Repeat::MIRROR } else { Repeat::ONE };
        let id = self.push(kind, repeat);
        if back { let e = self.elements.pop().unwrap(); self.elements.insert(0, e); }
        Some(id)
    }
    /// New variations of every scroll's side shoots.
    pub fn reseed(&mut self, base: u32) {
        let mut n = 0u32;
        for e in self.elements.iter_mut() {
            let count = e.repeat.count() as u32;
            if let Kind::Scroll { seed, .. } = &mut e.kind { *seed = base.wrapping_add(n.wrapping_mul(7919)); n += count; }
        }
    }
}

/// The acanthus as a drawing layer: the joins' outlines and folds, hiding
/// what is behind every grown part.
pub fn acanthus_layer(g: &GrowthResult, joins: JoinStyle) -> Layer {
    let d = joins.draw(g);
    let polys: Vec<&[Point]> = g.parts.iter().map(|p| p.polygon.as_slice()).filter(|p| p.len() > 2).collect();
    Layer { rings: d.outline, lines: d.folds, cover: union(&polys), ..Default::default() }
}

/// The acanthus drawn quickly (while dragging): every grown part's own
/// outline and folds, overlapping where they join.
pub fn draft_layer(g: &GrowthResult) -> Layer {
    let parts = g.parts.iter().filter(|p| p.polygon.len() > 2);
    Layer { rings: parts.clone().map(|p| closed(&p.polygon)).collect(), lines: parts.clone().flat_map(|p| p.folds.iter().cloned()).collect(), cover: parts.map(|p| vec![p.polygon.clone()]).collect(), ..Default::default() }
}

/// The element (and copy) a grown part belongs to: a leaf fan when the part
/// is one of the leaf edits, else its scroll.
pub fn part_owner(a: &Acanthus, part_id: &str) -> Option<(u32, usize)> {
    let (b, rest) = if a.owners.len() > 1 {
        let s = part_id.strip_prefix("backbone-")?;
        let (n, rest) = s.split_once('/')?;
        (n.parse::<usize>().ok()?, rest)
    } else { (0, part_id) };
    if let Some(k) = rest.strip_prefix("leaf-") {
        let digits: String = k.chars().take_while(|c| c.is_ascii_digit()).collect();
        if let Some(l) = digits.parse::<usize>().ok().and_then(|k| a.leaves.get(k)) { return Some(*l); }
    }
    a.owners.get(b).copied()
}

/// The bounds of a stack of layers.
pub fn bounds_of(layers: &[(Owner, Layer)]) -> Option<Bounds> {
    let all: Vec<Bounds> = layers.iter().filter_map(|(_, l)| l.bounds()).collect();
    let first = *all.first()?;
    Some(all.iter().fold(first, |a, b| Bounds { l: a.l.min(b.l), t: a.t.min(b.t), r: a.r.max(b.r), b: a.b.max(b.b) }))
}

/// The pattern SVG of a drawing at actual size.
pub fn svg_of(d: &Drawing, width: f64, height: f64) -> String {
    format!("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}mm\" height=\"{h}mm\" viewBox=\"0 0 {w} {h}\"><title>ORNATR cartouche pattern</title><g fill=\"none\" stroke=\"#000\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><path d=\"{}\" stroke-width=\".35\"/><path d=\"{}\" stroke-width=\".2\"/></g></svg>", path_data(&d.outlines, false), path_data(&d.lines, false), w = width, h = height)
}

// ---- compositing ------------------------------------------------------------

/// Everything in front of a layer, for hiding its lines: the edges of the
/// cover shapes in a grid (to find where a line crosses them) and in bands
/// per shape (to tell whether a point is under one; even-odd over a shape's
/// rings, so its holes show through).
struct Occluder { edges: Vec<(Point, Point)>, shapes: Vec<(Bounds, Vec<Vec<u32>>, f64)>, grid: Vec<Vec<u32>>, origin: Point, cell: f64, nx: usize, ny: usize, stamp: Vec<u32>, tick: u32 }
const BAND: f64 = 1.5;
impl Occluder {
    fn new(area: Bounds) -> Occluder {
        let cell = 3.0;
        let (nx, ny) = ((((area.r - area.l) / cell).ceil() as usize).clamp(1, 600), (((area.b - area.t) / cell).ceil() as usize).clamp(1, 600));
        Occluder { edges: vec![], shapes: vec![], grid: vec![vec![]; nx * ny], origin: pt(area.l, area.t), cell, nx, ny, stamp: vec![], tick: 0 }
    }
    fn cells(&self, l: f64, t: f64, r: f64, b: f64) -> (usize, usize, usize, usize) {
        let f = |v: f64, o: f64, n: usize| (((v - o) / self.cell).floor().max(0.0) as usize).min(n - 1);
        (f(l, self.origin.x, self.nx), f(t, self.origin.y, self.ny), f(r, self.origin.x, self.nx), f(b, self.origin.y, self.ny))
    }
    fn add(&mut self, shape: &[Vec<Point>]) {
        let pts: Vec<Point> = shape.iter().flatten().copied().collect();
        if pts.len() < 3 { return; }
        let bx = Bounds::of(&pts);
        let nb = (((bx.b - bx.t) / BAND).ceil() as usize).max(1);
        let mut bands: Vec<Vec<u32>> = vec![vec![]; nb];
        for ring in shape {
            let n = ring.len(); if n < 3 { continue; }
            for i in 0..n {
                let (a, b) = (ring[i], ring[(i + 1) % n]);
                let id = self.edges.len() as u32; self.edges.push((a, b));
                let (c0, r0, c1, r1) = self.cells(a.x.min(b.x), a.y.min(b.y), a.x.max(b.x), a.y.max(b.y));
                for r in r0..=r1 { for c in c0..=c1 { self.grid[r * self.nx + c].push(id); } }
                let (y0, y1) = (a.y.min(b.y), a.y.max(b.y));
                let (k0, k1) = ((((y0 - bx.t) / BAND).floor().max(0.0) as usize).min(nb - 1), (((y1 - bx.t) / BAND).floor().max(0.0) as usize).min(nb - 1));
                for k in k0..=k1 { bands[k].push(id); }
            }
        }
        self.shapes.push((bx, bands, bx.t));
        self.stamp.resize(self.edges.len(), 0);
    }
    fn covered(&self, p: Point) -> bool {
        self.shapes.iter().any(|(bx, bands, top)| {
            if !bx.contains(p) { return false; }
            let k = (((p.y - top) / BAND).floor().max(0.0) as usize).min(bands.len() - 1);
            let mut odd = false;
            for &e in &bands[k] {
                let (a, b) = self.edges[e as usize];
                if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x { odd = !odd; }
            }
            odd
        })
    }
    /// The visible runs of `lines`.
    fn visible(&mut self, lines: &[Vec<Point>]) -> Vec<Vec<Point>> {
        let mut out = vec![];
        let lerp = |a: Point, b: Point, t: f64| pt(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t);
        for line in lines {
            let mut run: Vec<Point> = vec![];
            for i in 1..line.len() {
                let (a, b) = (line[i - 1], line[i]);
                let mut cuts = vec![0.0, 1.0];
                if !self.edges.is_empty() {
                    self.tick = self.tick.wrapping_add(1);
                    if self.tick == 0 { self.stamp.iter_mut().for_each(|s| *s = 0); self.tick = 1; }
                    let (c0, r0, c1, r1) = self.cells(a.x.min(b.x), a.y.min(b.y), a.x.max(b.x), a.y.max(b.y));
                    for r in r0..=r1 { for c in c0..=c1 { for &e in &self.grid[r * self.nx + c] {
                        if self.stamp[e as usize] == self.tick { continue; }
                        self.stamp[e as usize] = self.tick;
                        let (p, q) = self.edges[e as usize];
                        if let Some(t) = intersection(a, b, p, q) { cuts.push(t); }
                    } } }
                    cuts.sort_by(|x, y| x.partial_cmp(y).unwrap());
                }
                for j in 1..cuts.len() {
                    if cuts[j] - cuts[j - 1] < 1e-8 { continue; }
                    let (start, end) = (lerp(a, b, cuts[j - 1]), lerp(a, b, cuts[j]));
                    if self.covered(lerp(start, end, 0.5)) { if run.len() > 1 { out.push(std::mem::take(&mut run)); } else { run.clear(); } }
                    else { if run.is_empty() { run.push(start); } run.push(end); }
                }
            }
            if run.len() > 1 { out.push(run); }
        }
        out
    }
}

/// The drawing of a stack of layers: each layer's rings and lines hidden
/// under the covers of the layers in front; the paint back to front.
pub fn compose(layers: &[(Owner, Layer)]) -> Drawing {
    let Some(area) = bounds_of(layers) else { return Drawing::default() };
    let mut occ = Occluder::new(Bounds { l: area.l - 1.0, t: area.t - 1.0, r: area.r + 1.0, b: area.b + 1.0 });
    let (mut outlines, mut lines) = (vec![], vec![]);
    for (_, l) in layers.iter().rev() {
        outlines.extend(occ.visible(&l.rings));
        lines.extend(occ.visible(&l.lines));
        for s in &l.cover { occ.add(s); }
    }
    let mut paint = vec![];
    for (_, l) in layers {
        for s in &l.cover { paint.push((Paint::Wood, s.clone())); }
        for f in &l.field { paint.push((Paint::Field, vec![f.clone()])); }
        for p in &l.pockets { paint.push((Paint::Pocket, vec![p.clone()])); }
    }
    Drawing { outlines, lines, paint }
}

/// Whether `p` is in a shape (even-odd over its rings).
pub fn in_shape(p: Point, shape: &[Vec<Point>]) -> bool { shape.iter().filter(|r| r.len() > 2 && inside(p, r)).count() % 2 == 1 }

// ---- the vocabulary (from the cartouche study) ----------------------------

fn closed(r: &[Point]) -> Vec<Point> { let mut v = r.to_vec(); if v.len() > 1 { v.push(v[0]); } v }
fn outer_ring(s: &Shapes) -> Vec<Point> { s.iter().map(|sh| sh[0].clone()).max_by(|a, b| crate::booleans::signed_area(a).abs().partial_cmp(&crate::booleans::signed_area(b).abs()).unwrap()).unwrap_or_default() }
fn grow_ring(r: &[Point], d: f64) -> Vec<Point> { outer_ring(&offset(&vec![vec![r.to_vec()]], d)) }

fn from_parts(parts: &[Part]) -> Vec<Layer> {
    parts.iter().filter(|p| p.outline.len() > 2).map(|p| if p.pocket { Layer { rings: vec![closed(&p.outline)], lines: p.lines.clone(), pockets: vec![p.outline.clone()], ..Default::default() } }
        else { Layer { rings: vec![closed(&p.outline)], lines: p.lines.clone(), cover: vec![vec![p.outline.clone()]], ..Default::default() } }).collect()
}

/// An oval of 240 points.
pub fn oval(c: Point, rx: f64, ry: f64) -> Vec<Point> { (0..240).map(|k| { let t = k as f64 / 240.0 * 2.0 * PI; pt(c.x + rx * t.cos(), c.y + ry * t.sin()) }).collect() }
fn turned_oval(c: Point, rx: f64, ry: f64, turn: f64) -> Vec<Point> {
    if turn == 0.0 { return oval(c, rx, ry); }
    let (s, k) = turn.sin_cos();
    oval(pt(0.0, 0.0), rx, ry).into_iter().map(|q| pt(c.x + q.x * k - q.y * s, c.y + q.x * s + q.y * k)).collect()
}
fn rounded_rect(l: f64, t: f64, r: f64, b: f64, rad: f64) -> Vec<Point> {
    let rad = rad.clamp(0.0, ((r - l) / 2.0).min((b - t) / 2.0));
    let mut v = vec![];
    for (c, a0) in [(pt(r - rad, t + rad), -PI * 0.5), (pt(r - rad, b - rad), 0.0), (pt(l + rad, b - rad), PI * 0.5), (pt(l + rad, t + rad), PI)] {
        for k in 0..=12 { let a = a0 + PI * 0.5 * k as f64 / 12.0; v.push(pt(c.x + rad * a.cos(), c.y + rad * a.sin())); }
    }
    v
}
/// A shield: the top edge arching up a little, the sides running down and in
/// to the point (the study's, 104 × 134 mm, scaled to `rx`, `ry`).
fn shield(c: Point, rx: f64, ry: f64) -> Vec<Point> {
    let (sx, sy) = (rx / 52.0, ry / 67.0);
    let (cx, top, half, tip) = (c.x, c.y - ry, rx, c.y + ry);
    let mut v = vec![];
    for k in 0..=40 { let t = k as f64 / 40.0; let x = cx - half + 2.0 * half * t; v.push(pt(x, top - 7.0 * sy * (PI * t).sin())); }
    let bez = |p0: Point, p1: Point, p2: Point, p3: Point, t: f64| { let u = 1.0 - t; pt(u * u * u * p0.x + 3.0 * u * u * t * p1.x + 3.0 * u * t * t * p2.x + t * t * t * p3.x, u * u * u * p0.y + 3.0 * u * u * t * p1.y + 3.0 * u * t * t * p2.y + t * t * t * p3.y) };
    let (a, b) = (pt(cx + half, top), pt(cx, tip));
    for k in 1..=60 { v.push(bez(a, pt(cx + half + 3.0 * sx, top + (tip - top) * 0.62), pt(cx + half * 0.55, tip - 14.0 * sy), b, k as f64 / 60.0)); }
    let right: Vec<Point> = v[41..].to_vec();
    for p in right.iter().rev().skip(1) { v.push(pt(2.0 * cx - p.x, p.y)); }
    v
}
/// The field's outline.
pub fn field_outline(shape: FieldShape, c: Point, rx: f64, ry: f64, round: f64) -> Vec<Point> {
    let (rx, ry) = (rx.max(1.0), ry.max(1.0));
    match shape {
        FieldShape::Oval => oval(c, rx, ry),
        FieldShape::Rect => rounded_rect(c.x - rx, c.y - ry, c.x + rx, c.y + ry, round),
        FieldShape::Shield => shield(c, rx, ry),
    }
}
/// A frame round `field`, `w` wide: moulded (the field's edge, a narrow lip
/// stepping down to a cove, a roll out to the outer edge), a flat strap with
/// a bevel, or only a bead line round the field.
fn frame_layer(field: &[Point], w: f64, moulding: Moulding) -> Layer {
    if moulding == Moulding::Bead {
        return Layer { rings: vec![closed(field)], lines: vec![closed(&grow_ring(field, -w * 0.2))], cover: vec![vec![field.to_vec()]], field: vec![field.to_vec()], ..Default::default() };
    }
    let outer = grow_ring(field, w);
    let (a, b) = if moulding == Moulding::Strap { (0.4, 0.6) } else { (0.16, 0.58) };
    Layer { rings: vec![closed(&outer), closed(field)], lines: vec![closed(&grow_ring(field, w * a)), closed(&grow_ring(field, w * b))], cover: vec![vec![outer]], field: vec![field.to_vec()], ..Default::default() }
}
/// A row of beads (pearls) along a closed ring, `r` radius, about `gap` apart.
fn beads(ring: &[Point], r: f64, gap: f64) -> Layer {
    if ring.is_empty() { return Layer::default(); }
    let mut pts = vec![]; let mut acc = gap;
    let mut prev = ring[0];
    for p in ring.iter().chain(std::iter::once(&ring[0])).skip(1) {
        acc += (p.x - prev.x).hypot(p.y - prev.y); prev = *p;
        if acc >= gap { pts.push(*p); acc = 0.0; }
    }
    let circles: Vec<Vec<Point>> = pts.iter().map(|&p| (0..20).map(|k| { let t = k as f64 / 20.0 * 2.0 * PI; pt(p.x + r * t.cos(), p.y + r * t.sin()) }).collect()).collect();
    Layer { rings: circles.iter().map(|c| closed(c)).collect(), cover: circles.into_iter().map(|c| vec![c]).collect(), ..Default::default() }
}
/// A jewel (cabochon): an oval stone in a raised bezel, a line round the stone's crown.
fn jewel(c: Point, rx: f64, ry: f64, turn: f64, bezel: f64) -> Vec<Part> {
    vec![Part { outline: turned_oval(c, rx + bezel, ry + bezel, turn), lines: vec![], pocket: false, seam: vec![] },
         Part { outline: turned_oval(c, rx, ry, turn), lines: vec![closed(&turned_oval(c, rx * 0.55, ry * 0.6, turn))], pocket: false, seam: vec![] }]
}
fn boss(c: Point, r: f64) -> Part { Part { lines: vec![closed(&oval(c, r * 0.55, r * 0.55))], outline: oval(c, r, r), pocket: false, seam: vec![] } }
/// A flat strap (strapwork): an even band with a bevel line all round,
/// rolling into volutes where `hook` is set.
fn strap(ctrl: &[Point], hook0: f64, eye0: f64, hook1: f64, eye1: f64, w: f64) -> Part {
    let (sp, ua, ub) = rocaille::rim_along(ctrl, hook0, eye0, hook1, eye1);
    let mut p = rocaille::moulded_rim(&sp, ua, ub, w, 1.0);
    p.lines = offset(&union(&[p.outline.as_slice()]), -w * 0.4).into_iter().flatten().map(|r| closed(&r)).collect();
    p
}

// ---- the library structures ------------------------------------------------

/// The starting layouts: id, name, description.
pub const STRUCTURES: [(&str, &str, &str); 10] = [
    ("shield", "Strapwork", "Strapwork shield (B4): a strap frame and pierced arch; acanthus scrolls ride the strap and curl into the field, child scrolls meet at the waist with paired collars"),
    ("shield-jewels", "Shield", "Shield synthesis (G2): B4 with jewels under the arch and at the point, leaf fans by the top jewel and a C pair dipping into the field"),
    ("oval", "Oval", "Oval synthesis (G1): a beaded oval with a jewelled crest, a heavy base of sweeping scrolls rising to the waist, and side jewels"),
    ("panel", "Panel", "Rectangle synthesis (G3): jewelled corner clusters, a dominant crest over the top edge and a lighter foot"),
    ("jewels", "Jewelled", "Jewel clusters (E1): a plain beaded oval with all the ornament gathered at the top and the bottom"),
    ("four-point", "Four-point", "Four-point oval (F3): clusters top and bottom and smaller ones growing from the frame at the sides"),
    ("crest-base", "Crest & base", "Crest and base (E4): a round beaded frame, a small crest and a heavy base of sweeping S-scrolls"),
    ("wreath", "Wreath", "Wreath (E3): eight C-scroll pairs round a beaded ring, a small jewel between each pair"),
    ("rectangle", "Rectangle", "Rectangular panel (F1): jewelled corner clusters and small C pairs at the middle of each side"),
    ("shield-clusters", "Shield clusters", "Shield with jewel clusters (F2): the strap shield with clusters under the arch and at the point, plain strap sides"),
];
/// The first design, its page size.
pub const START: (&str, f64, f64) = ("shield", 220.0, 240.0);

/// A structure in its own (the study's) coordinates, not fitted to a page.
pub fn structure(id: &str) -> Option<Design> {
    Some(match id {
        "shield" => strap_shield(false),
        "shield-jewels" => strap_shield(true),
        "oval" => oval_g1(),
        "panel" => panel_g3(),
        "jewels" => jewels_e1(),
        "four-point" => four_point_f3(),
        "crest-base" => crest_base_e4(),
        "wreath" => wreath_e3(),
        "rectangle" => rectangle_f1(),
        "shield-clusters" => shield_clusters_f2(),
        _ => return None,
    })
}

/// A scroll stem as the study wrote it: from `a` heading `a0`° to `b`
/// arriving at `a1`° (y down, -90 = up), the handles `k0`, `k1` of the chord.
#[derive(Clone, Copy)]
struct Stem { a: Point, a0: f64, b: Point, a1: f64, k0: f64, k1: f64, curl: Side, scale: f64, levels: u8, flip: bool }
fn st(a: Point, a0: f64, b: Point, a1: f64, curl: Side) -> Stem { Stem { a, a0, b, a1, k0: 0.42, k1: 0.36, curl, scale: 1.0, levels: 2, flip: false } }
/// The study's stem shorthand: handles, size and side-shoot levels.
fn sk(k0: f64, k1: f64, scale: f64, levels: u8, s: Stem) -> Stem { Stem { k0, k1, scale, levels, ..s } }
fn bez(s: &Stem) -> Curve {
    let d = (s.b.x - s.a.x).hypot(s.b.y - s.a.y); let (u, v) = (s.a0.to_radians(), s.a1.to_radians());
    [s.a, pt(s.a.x + u.cos() * d * s.k0, s.a.y + u.sin() * d * s.k0), pt(s.b.x - v.cos() * d * s.k1, s.b.y - v.sin() * d * s.k1), s.b]
}
/// A stem reflected across the line through `p` at `axis` degrees (keeps the curl side, flips the growth).
fn reflect(s: &Stem, p: Point, axis: f64) -> Stem {
    let (sn, cs) = (2.0 * axis.to_radians()).sin_cos();
    let r = |q: Point| { let (dx, dy) = (q.x - p.x, q.y - p.y); pt(p.x + dx * cs + dy * sn, p.y + dx * sn - dy * cs) };
    Stem { a: r(s.a), b: r(s.b), a0: 2.0 * axis - s.a0, a1: 2.0 * axis - s.a1, flip: !s.flip, ..*s }
}
/// A stem turned by `deg` about the origin and moved to `to`.
fn place(s: &Stem, to: Point, deg: f64) -> Stem {
    let (sn, cs) = deg.to_radians().sin_cos();
    let r = |p: Point| pt(to.x + p.x * cs - p.y * sn, to.y + p.x * sn + p.y * cs);
    Stem { a: r(s.a), b: r(s.b), a0: s.a0 + deg, a1: s.a1 + deg, ..*s }
}
/// A small accent on an edge: one of a C pair back to back whose volutes tuck
/// onto the frame, built on the top edge at `at` and turned by `deg` (its
/// partner is the mirrored copy).
fn edge_c(at: Point, deg: f64, len: f64) -> Stem { place(&sk(0.45, 0.45, 0.55, 1, st(pt(3.0, -1.0), -30.0, pt(len, 3.0), 75.0, Side::Right)), at, deg) }

/// Builds a design as the study did, numbering the backbones as it went so
/// every scroll copy gets the study's seed.
struct Build { d: Design, base: u32, n: u32, stems: Vec<u32> }
impl Build {
    fn new(centre: Point, growth: f64, base: u32, collars: bool) -> Build { Build { d: Design { width: 0.0, height: 0.0, centre, growth, collars, eyes: false, elements: vec![] }, base, n: 0, stems: vec![] } }
    fn frame(&mut self, shape: FieldShape, rx: f64, ry: f64, round: f64, width: f64, moulding: Moulding, beads: bool) { self.d.push(Kind::Frame { shape, rx, ry, round, width, moulding, bead: if beads { 1.1 } else { 0.0 } }, Repeat::ONE); }
    /// A scroll and its copies; returns its element id.
    fn scroll(&mut self, s: Stem, repeat: Repeat, attach: Option<u32>) -> u32 {
        let seed = self.base.wrapping_add(self.n.wrapping_mul(7919)); self.n += repeat.count() as u32;
        let id = self.d.push(Kind::Scroll { curve: bez(&s), curl: s.curl, scale: s.scale, levels: s.levels, flip: s.flip, attach, seed, volute: None }, repeat);
        self.stems.push(id);
        id
    }
    fn leaf(&mut self, stem: u32, preset: &str, at: f64, side: f64, fan: u8, size: f64, mirror_side: bool) { self.d.push(Kind::Leaf { stem, preset: preset.into(), at, side, fan, size, mirror_side, turn: 0.0, bend: 0.0, width: 1.0, follow: LEAF_FOLLOW }, Repeat::ONE); }
    fn jewel(&mut self, c: Point, rx: f64, ry: f64, repeat: Repeat) { self.d.push(Kind::Jewel { c, rx, ry, turn: 0.0, bezel: 2.0 }, repeat); }
    fn boss(&mut self, c: Point, r: f64, repeat: Repeat) { self.d.push(Kind::Boss { c, r }, repeat); }
}

/// The strapwork shield (B4), or with `jewels` its synthesis (G2): a dipping
/// C pair, leaf fans beside the top jewel, jewels, only the corner bosses.
fn strap_shield(jewels: bool) -> Design {
    let cx = 150.0;
    let mut b = Build::new(pt(cx, 159.0), 0.8, 83, true);
    b.d.push(Kind::Opening { c: pt(cx, 72.0), rx: 22.0, ry: 16.0, turn: 0.0 }, Repeat::ONE);
    b.d.push(Kind::Strap { ctrl: vec![pt(cx - 26.0, 86.0), pt(cx - 24.0, 66.0), pt(cx, 54.0), pt(cx + 24.0, 66.0), pt(cx + 26.0, 86.0)], width: 4.4, hook0: 1.0, eye0: 8.0, hook1: 1.0, eye1: 8.0 }, Repeat::ONE);
    b.frame(FieldShape::Shield, 52.0, 67.0, 0.0, 9.0, Moulding::Strap, false);
    let corner = b.scroll(sk(0.5, 0.4, 0.9, 2, st(pt(cx + 12.0, 88.0), -4.0, pt(cx + 40.0, 134.0), 125.0, Side::Left)), Repeat::MIRROR, None);
    let point = b.scroll(sk(0.45, 0.4, 0.8, 2, st(pt(cx + 3.0, 230.0), -55.0, pt(cx + 36.0, 182.0), -130.0, Side::Right)), Repeat::MIRROR, None);
    b.scroll(sk(0.4, 0.4, 0.65, 1, st(pt(cx + 42.0, 92.0), 15.0, pt(cx + 74.0, 144.0), 100.0, Side::Left)), Repeat::MIRROR, Some(corner));
    b.scroll(sk(0.4, 0.4, 0.6, 1, st(pt(cx + 22.0, 210.0), 5.0, pt(cx + 66.0, 182.0), -100.0, Side::Right)), Repeat::MIRROR, Some(point));
    b.scroll(sk(0.45, 0.45, 0.55, 1, st(pt(cx + 3.0, 222.0), -60.0, pt(cx + 11.0, 192.0), -95.0, Side::Right)), Repeat::MIRROR, None);
    if jewels { b.scroll(sk(0.45, 0.45, 0.5, 1, st(pt(cx + 4.0, 100.0), 75.0, pt(cx + 18.0, 112.0), -40.0, Side::Left)), Repeat::MIRROR, None); }
    b.leaf(corner, "returning-leaf", 0.62, 1.0, 3, 1.0, false);
    b.leaf(point, "sweeping-tongue", 0.5, -1.0, 2, 1.0, false);
    if jewels { b.leaf(corner, "returning-leaf", 0.08, 1.0, 3, 1.4, true); }
    b.boss(pt(cx - 54.0, 88.0), 5.0, Repeat::MIRROR);
    if jewels {
        b.jewel(pt(cx, 89.0), 6.0, 8.5, Repeat::ONE); b.jewel(pt(cx, 230.0), 6.0, 8.5, Repeat::ONE);
    } else { b.boss(pt(cx, 233.0), 5.5, Repeat::ONE); b.boss(pt(cx, 54.0), 4.5, Repeat::ONE); }
    b.d
}

/// F2: the strap shield with E1's jewel clusters under the arch and at the point.
fn shield_clusters_f2() -> Design {
    let cx = 150.0;
    // centred on the shield (the study mirrored only left and right, so the centre's height is free)
    let mut b = Build::new(pt(cx, 159.0), 1.1, 102, false);
    b.d.push(Kind::Opening { c: pt(cx, 72.0), rx: 22.0, ry: 16.0, turn: 0.0 }, Repeat::ONE);
    b.d.push(Kind::Strap { ctrl: vec![pt(cx - 26.0, 86.0), pt(cx - 24.0, 66.0), pt(cx, 54.0), pt(cx + 24.0, 66.0), pt(cx + 26.0, 86.0)], width: 4.4, hook0: 1.0, eye0: 8.0, hook1: 1.0, eye1: 8.0 }, Repeat::ONE);
    b.frame(FieldShape::Shield, 52.0, 67.0, 0.0, 9.0, Moulding::Strap, false);
    let big = b.scroll(sk(0.45, 0.45, 0.85, 2, st(pt(cx + 9.0, 88.0), -5.0, pt(cx + 44.0, 96.0), 70.0, Side::Right)), Repeat::MIRROR, None);
    b.scroll(sk(0.45, 0.45, 0.5, 1, st(pt(cx + 4.0, 98.0), 75.0, pt(cx + 18.0, 110.0), -40.0, Side::Left)), Repeat::MIRROR, None);
    let low = b.scroll(sk(0.45, 0.45, 0.8, 2, st(pt(cx + 6.0, 230.0), -52.0, pt(cx + 34.0, 198.0), -120.0, Side::Right)), Repeat::MIRROR, None);
    b.scroll(sk(0.45, 0.45, 0.5, 1, st(pt(cx + 3.0, 224.0), -95.0, pt(cx + 14.0, 206.0), 40.0, Side::Right)), Repeat::MIRROR, None);
    b.leaf(big, "returning-leaf", 0.1, 1.0, 3, 2.2, true);
    b.leaf(low, "returning-leaf", 0.1, 1.0, 3, 1.8, true);
    b.boss(pt(cx - 54.0, 88.0), 5.0, Repeat::MIRROR);
    b.jewel(pt(cx, 89.0), 6.0, 8.5, Repeat::ONE); b.jewel(pt(cx, 230.0), 6.0, 8.5, Repeat::ONE);
    b.d
}

/// E1: jewel clusters on a plain beaded oval.
fn jewels_e1() -> Design {
    let c = pt(150.0, 160.0);
    let mut b = Build::new(c, 1.2, 91, false);
    b.frame(FieldShape::Oval, 50.0, 64.0, 0.0, 10.0, Moulding::Moulded, true);
    let big = b.scroll(sk(0.45, 0.45, 0.85, 2, st(pt(c.x + 9.0, 88.0), -10.0, pt(c.x + 50.0, 98.0), 70.0, Side::Right)), Repeat::FOUR, None);
    b.scroll(sk(0.45, 0.45, 0.5, 1, st(pt(c.x + 4.0, 100.0), 75.0, pt(c.x + 18.0, 112.0), -40.0, Side::Left)), Repeat::FOUR, None);
    b.leaf(big, "returning-leaf", 0.1, 1.0, 3, 2.5, true);
    b.jewel(pt(c.x, 88.0), 7.0, 10.0, Repeat::UPDOWN);
    b.d
}

/// E3: wreath of eight C-scroll pairs round a beaded ring.
fn wreath_e3() -> Design {
    let c = pt(150.0, 150.0);
    let r_out = 57.0;
    let mut b = Build::new(c, 0.95, 93, false);
    b.frame(FieldShape::Oval, 48.0, 48.0, 0.0, 9.0, Moulding::Moulded, true);
    b.scroll(sk(0.45, 0.45, 0.7, 1, st(pt(c.x + 3.0, c.y - r_out + 1.0), -30.0, pt(c.x + 21.0, c.y - r_out + 6.0), 75.0, Side::Right)), Repeat { mirror_x: true, mirror_y: false, ring: 8 }, None);
    b.d.push(Kind::Jewel { c: pt(c.x, c.y - (r_out + 1.0)), rx: 1.8, ry: 1.8, turn: 0.0, bezel: 2.0 }, Repeat { mirror_x: false, mirror_y: false, ring: 8 });
    b.d
}

/// E4: crest and base on a round beaded frame.
fn crest_base_e4() -> Design {
    let c = pt(150.0, 150.0);
    let r_out = 55.0;
    let mut b = Build::new(c, 1.1, 94, false);
    b.frame(FieldShape::Oval, 46.0, 46.0, 0.0, 9.0, Moulding::Moulded, true);
    let sweep = b.scroll(sk(0.45, 0.45, 1.0, 2, st(pt(c.x + 6.0, c.y + r_out + 2.0), 5.0, pt(c.x + 92.0, c.y + r_out - 14.0), -80.0, Side::Left)), Repeat::MIRROR, None);
    b.scroll(sk(0.45, 0.45, 0.7, 1, st(pt(c.x + 4.0, c.y + r_out - 4.0), -20.0, pt(c.x + 40.0, c.y + 30.0), -100.0, Side::Right)), Repeat::MIRROR, None);
    let crest = b.scroll(sk(0.45, 0.45, 0.5, 1, st(pt(c.x + 4.0, c.y - r_out - 1.0), -20.0, pt(c.x + 22.0, c.y - r_out + 2.0), 70.0, Side::Right)), Repeat::MIRROR, None);
    b.leaf(sweep, "returning-leaf", 0.2, 1.0, 3, 2.4, true);
    b.leaf(crest, "returning-leaf", 0.1, 1.0, 2, 1.5, true);
    b.jewel(pt(c.x, c.y + r_out + 4.0), 8.0, 11.0, Repeat::ONE); b.jewel(pt(c.x, c.y - r_out), 5.0, 7.0, Repeat::ONE);
    b.d
}

/// F1 and G3's corner clusters: C-scrolls back to back along the top and
/// the side edge (the side one the top one reflected across the corner's
/// diagonal), in all four corners.
fn corners(b: &mut Build, top_end: f64) -> u32 {
    let corner = pt(209.5, 80.5);
    let top = sk(0.45, 0.45, 0.8, 2, st(pt(203.0, 80.0), 190.0, pt(top_end, 84.0), 200.0, Side::Left));
    let id = b.scroll(top, Repeat::FOUR, None);
    b.scroll(reflect(&top, corner, -45.0), Repeat::FOUR, None);
    id
}

/// F1: rectangular panel, jewelled corner clusters, small mid-side accents.
fn rectangle_f1() -> Design {
    let c = pt(150.0, 160.0);
    let mut b = Build::new(c, 1.0, 101, false);
    b.frame(FieldShape::Rect, 55.0, 75.0, 6.0, 9.0, Moulding::Moulded, true);
    let top = corners(&mut b, 168.0);
    b.scroll(edge_c(pt(c.x, 80.5), 0.0, 16.0), Repeat::FOUR, None);
    b.scroll(edge_c(pt(209.5, c.y), 90.0, 16.0), Repeat::FOUR, None);
    b.leaf(top, "returning-leaf", 0.16, 1.0, 3, 2.4, true);
    b.jewel(pt(209.5, 80.5), 3.2, 3.2, Repeat::FOUR);
    b.jewel(pt(c.x, 80.5), 2.5, 3.5, Repeat::UPDOWN);
    b.jewel(pt(209.5, c.y), 3.5, 2.5, Repeat::MIRROR);
    b.d
}

/// F3: four-point oval: clusters top and bottom, side clusters from the frame.
fn four_point_f3() -> Design {
    let c = pt(150.0, 160.0);
    let mut b = Build::new(c, 1.2, 103, false);
    b.frame(FieldShape::Oval, 50.0, 64.0, 0.0, 10.0, Moulding::Moulded, true);
    let big = b.scroll(sk(0.45, 0.45, 0.85, 2, st(pt(c.x + 9.0, 88.0), -10.0, pt(c.x + 50.0, 98.0), 70.0, Side::Right)), Repeat::FOUR, None);
    b.scroll(sk(0.45, 0.45, 0.5, 1, st(pt(c.x + 4.0, 100.0), 75.0, pt(c.x + 18.0, 112.0), -40.0, Side::Left)), Repeat::FOUR, None);
    let up = b.scroll(sk(0.45, 0.45, 0.65, 1, st(pt(c.x + 60.0, c.y - 6.0), -95.0, pt(c.x + 50.0, c.y - 34.0), -150.0, Side::Left)), Repeat::FOUR, None);
    b.leaf(big, "returning-leaf", 0.1, 1.0, 3, 2.5, true);
    b.leaf(up, "returning-leaf", 0.06, -1.0, 2, 1.6, true);
    b.jewel(pt(c.x, 88.0), 7.0, 10.0, Repeat::UPDOWN);
    b.jewel(pt(c.x + 61.0, c.y), 4.5, 6.0, Repeat::MIRROR);
    b.d
}

/// G1: the oval synthesis.
fn oval_g1() -> Design {
    let c = pt(150.0, 160.0);
    let mut b = Build::new(c, 1.2, 111, false);
    b.frame(FieldShape::Oval, 50.0, 64.0, 0.0, 10.0, Moulding::Moulded, true);
    let big = b.scroll(sk(0.45, 0.45, 0.85, 2, st(pt(c.x + 9.0, 88.0), -10.0, pt(c.x + 50.0, 98.0), 70.0, Side::Right)), Repeat::MIRROR, None);
    b.scroll(sk(0.45, 0.45, 0.5, 1, st(pt(c.x + 4.0, 100.0), 75.0, pt(c.x + 18.0, 112.0), -40.0, Side::Left)), Repeat::MIRROR, None);
    let sweep = b.scroll(sk(0.45, 0.45, 1.0, 2, st(pt(c.x + 6.0, 238.0), 5.0, pt(c.x + 80.0, 226.0), -80.0, Side::Left)), Repeat::MIRROR, None);
    b.scroll(sk(0.45, 0.45, 0.7, 1, st(pt(c.x + 5.0, 230.0), -28.0, pt(c.x + 62.0, c.y + 22.0), -92.0, Side::Right)), Repeat::MIRROR, None);
    let side = b.scroll(sk(0.45, 0.45, 0.65, 1, st(pt(c.x + 60.0, c.y - 6.0), -95.0, pt(c.x + 50.0, c.y - 34.0), -150.0, Side::Left)), Repeat::MIRROR, None);
    b.leaf(big, "returning-leaf", 0.1, 1.0, 3, 2.5, true);
    b.leaf(sweep, "returning-leaf", 0.2, 1.0, 3, 2.4, true);
    b.leaf(side, "returning-leaf", 0.06, -1.0, 2, 1.6, true);
    b.jewel(pt(c.x, 88.0), 7.0, 10.0, Repeat::ONE); b.jewel(pt(c.x, 237.0), 8.0, 11.0, Repeat::ONE);
    b.jewel(pt(c.x + 61.0, c.y), 4.5, 6.0, Repeat::MIRROR);
    b.d
}

/// G3: the rectangle synthesis.
fn panel_g3() -> Design {
    let c = pt(150.0, 160.0);
    let mut b = Build::new(c, 1.1, 113, false);
    b.frame(FieldShape::Rect, 55.0, 75.0, 6.0, 9.0, Moulding::Moulded, true);
    let top = corners(&mut b, 178.0);
    let big = b.scroll(sk(0.45, 0.45, 0.85, 2, st(pt(c.x + 8.0, 80.0), -12.0, pt(c.x + 40.0, 86.0), 70.0, Side::Right)), Repeat::MIRROR, None);
    b.scroll(sk(0.45, 0.45, 0.5, 1, st(pt(c.x + 4.0, 90.0), 75.0, pt(c.x + 18.0, 102.0), -40.0, Side::Left)), Repeat::MIRROR, None);
    let foot = b.scroll(sk(0.45, 0.45, 0.6, 1, st(pt(c.x + 6.0, 240.0), 10.0, pt(c.x + 30.0, 236.0), -75.0, Side::Right)), Repeat::MIRROR, None);
    b.scroll(edge_c(pt(209.5, c.y), 90.0, 16.0), Repeat::FOUR, None);
    b.leaf(top, "returning-leaf", 0.16, 1.0, 3, 2.2, true);
    b.leaf(big, "returning-leaf", 0.1, 1.0, 3, 2.6, true);
    b.leaf(foot, "returning-leaf", 0.15, -1.0, 2, 1.6, true);
    b.jewel(pt(209.5, 80.5), 3.2, 3.2, Repeat::FOUR);
    b.jewel(pt(c.x, 80.0), 7.0, 10.0, Repeat::ONE); b.jewel(pt(c.x, 240.0), 5.0, 6.5, Repeat::ONE);
    b.jewel(pt(209.5, c.y), 3.5, 2.5, Repeat::MIRROR);
    b.d
}
