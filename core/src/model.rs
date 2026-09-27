//! An ORNATR layout: page, backbones, per-backbone growth, shoot edits,
//! kept parts and legacy stamps; growing it; presets; SVG export.
use crate::geometry::{arc_table, distance, pt, Curve, Point};
use crate::growth::{grow_backbone, GrowInput, GrowthPart, GrowthResult, GrowthSettings};
use crate::layers::{carving_guides, layered_drawing};
use crate::outline::path_data;
use crate::profiles::profile;
use crate::shoots::{ShootEdit, ShootParams};

/// Pull a child sweep's root outline back inside its parent near the join
/// (see `grow_settled`).
fn tuck_root(child: &mut GrowthPart, parent: &[Point], join: Point) {
    use crate::outline::inside;
    let spine = &child.points; if spine.len() < 4 { return; }
    let start = if distance(spine[0], join) <= distance(*spine.last().unwrap(), join) { spine[0] } else { *spine.last().unwrap() };
    let Some(ahead) = spine.iter().copied().filter(|q| distance(*q, start) > 6.0).min_by(|a, b| distance(*a, start).partial_cmp(&distance(*b, start)).unwrap()) else { return };
    let l = distance(ahead, start).max(1e-9); let d = pt((ahead.x - start.x) / l, (ahead.y - start.y) / l);
    let reach = 9.0;
    for p in child.polygon.iter_mut() {
        let (dx, dy) = (p.x - join.x, p.y - join.y);
        let ahead = dx * d.x + dy * d.y;
        if dx.hypot(dy) > reach || ahead > 3.0 || inside(*p, parent) { continue; }
        // largest k in (0, 1) with join + k·(p − join) inside the parent
        let (mut lo, mut hi) = (0.0, 1.0);
        for _ in 0..24 { let k = (lo + hi) / 2.0; if inside(pt(join.x + dx * k, join.y + dy * k), parent) { lo = k; } else { hi = k; } }
        // fully tucked behind the fork, easing out toward the child so the
        // outline has no step where tucking stops
        let w = { let x = ((3.0 - ahead) / 3.0).clamp(0.0, 1.0); x * x * (3.0 - 2.0 * x) };
        let k = 1.0 + (lo * 0.8 - 1.0) * w;
        *p = pt(join.x + dx * k, join.y + dy * k);
    }
}

/// Old manual-mode stamp; kept only so saved layouts can be converted.
#[derive(Clone, Debug, PartialEq)]
pub struct Placement { pub id: String, pub motif: String, pub progress: f64, pub length: f64, pub fullness: f64, pub angle: f64, pub bend: f64, pub mirror: bool, pub folds: bool, pub backbone: usize, pub on_top: Option<bool> }

#[derive(Clone, Debug)]
pub struct Layout {
    pub width: f64, pub height: f64, pub curves: Vec<Curve>, pub growth: Vec<GrowthSettings>,
    pub locked_parts: Vec<GrowthPart>, pub shoots: Vec<ShootEdit>, pub items: Vec<Placement>, pub print_backbone: bool,
    /// The carving surface outline (mm): growth stays inside it. None is the page rectangle.
    pub frame: Option<Vec<Point>>,
    /// A carving surface preset fitted to the page ("plaque", "oval", "rectangle"),
    /// used when `frame` is not set. None is the page itself.
    pub surface: Option<String>,
}
impl Layout {
    /// One grown backbone on a 240 × 150 mm page.
    pub fn starter() -> Layout {
        Layout { width: 240.0, height: 150.0, curves: vec![[pt(34.0, 112.0), pt(85.0, 125.0), pt(110.0, 66.0), pt(196.0, 86.0)]], growth: vec![GrowthSettings::default()], locked_parts: vec![], shoots: vec![], items: vec![], print_backbone: false, frame: None, surface: None }
    }
    pub fn growth_for(&self, i: usize) -> GrowthSettings { self.growth.get(i).cloned().or_else(|| self.growth.first().cloned()).unwrap_or_default() }
    /// Where an attached backbone meets its parent: the parent's centreline
    /// point nearest to the child's start.
    pub fn attach_point(&self, index: usize) -> Option<Point> {
        let parent = self.growth_for(index).attach.filter(|&p| p != index && p < self.curves.len())?;
        let start = self.curves[index][0];
        arc_table(&self.curves[parent]).into_iter().map(|r| r.point).min_by(|a, b| distance(*a, start).partial_cmp(&distance(*b, start)).unwrap())
    }
    /// Attached backbones start on their parent's stem. When a parent changes,
    /// its children (and theirs) move rigidly with the join, keeping their own
    /// shape. Returns true when anything moved.
    pub fn settle(&mut self) -> bool {
        let mut moved = false;
        for i in 0..self.curves.len() {
            if let Some(p) = self.attach_point(i) {
                let d = pt(p.x - self.curves[i][0].x, p.y - self.curves[i][0].y);
                if d.x.abs() + d.y.abs() > 1e-9 { self.translate(i, d); moved = true; }
            }
        }
        moved
    }
    /// Move a backbone and everything growing from it.
    pub fn translate(&mut self, index: usize, d: Point) {
        let mut group = vec![index]; group.extend(self.descendants(index));
        for b in group { for q in self.curves[b].iter_mut() { *q = pt(q.x + d.x, q.y + d.y); } }
    }
    /// Slide an attached backbone along its parent's stem so its start lands on
    /// the stem point nearest `to`; the scroll keeps its shape. False when the
    /// backbone is not attached.
    pub fn slide_attached(&mut self, index: usize, to: Point) -> bool {
        let Some(parent) = self.growth_for(index).attach.filter(|&p| p != index && p < self.curves.len()) else { return false };
        let Some(np) = arc_table(&self.curves[parent]).into_iter().map(|r| r.point).min_by(|a, b| distance(*a, to).partial_cmp(&distance(*b, to)).unwrap()) else { return false };
        let d = pt(np.x - self.curves[index][0].x, np.y - self.curves[index][0].y);
        self.translate(index, d);
        true
    }
    /// Backbones that grow (directly or indirectly) from `index`.
    pub fn descendants(&self, index: usize) -> Vec<usize> {
        let mut out = vec![]; let mut frontier = vec![index];
        while let Some(b) = frontier.pop() {
            for i in 0..self.curves.len() { if i != index && !out.contains(&i) && self.growth_for(i).attach == Some(b) { out.push(i); frontier.push(i); } }
        }
        out
    }
    /// Grow every backbone. With several, part ids are prefixed `backbone-N/`.
    pub fn grow(&self) -> GrowthResult { self.grow_with(false) }
    /// As `grow`, but quick for dragging: a scroll vine shows only its stem.
    pub fn grow_draft(&self) -> GrowthResult { self.grow_with(true) }
    fn grow_with(&self, draft: bool) -> GrowthResult {
        let mut settled = self.clone();
        let mut rounds = 0; while rounds < 6 && settled.settle() { rounds += 1; } // chains settle in a few rounds; cycles stop
        settled.grow_settled(draft)
    }
    /// The carving surface outline, if one is set (an explicit frame, or a preset fitted to the page).
    pub fn surface_polygon(&self) -> Option<Vec<Point>> {
        self.frame.clone().or_else(|| self.surface.as_deref().and_then(|s| surface_outline(s, self.width, self.height)))
    }
    fn is_vine(&self, index: usize) -> bool { self.growth_for(index).vine.is_some_and(|v| v.is_finite() && v > 0.0) }
    /// One backbone's growth: a scroll vine (keeping clear of `obstacles`,
    /// the parts already grown on the page), or the usual scroll.
    fn grow_one(&self, index: usize, locked: &[GrowthPart], shoots: &[ShootEdit], obstacles: &[Vec<Point>], draft: bool) -> GrowthResult {
        let s = self.growth_for(index);
        let surface = self.surface_polygon();
        let (vine_edits, others): (Vec<ShootEdit>, Vec<ShootEdit>) = shoots.iter().cloned().partition(|e| e.params.preset.as_deref() == Some(crate::shoots::VINE_CURL));
        match s.vine.filter(|v| v.is_finite() && *v > 0.0) {
            Some(spacing) => grow_vine(self.width, self.height, &self.curves[index], &s, spacing, surface.as_deref(), obstacles, &vine_edits, draft),
            None => grow_backbone(&GrowInput { width: self.width, height: self.height, curve: self.curves[index], locked, shoots: &others, settings: &s, frame: surface.as_deref() }),
        }
    }
    fn grow_settled(&self, draft: bool) -> GrowthResult {
        if self.curves.len() == 1 {
            return self.grow_one(0, &self.locked_parts, &self.shoots.iter().filter(|e| e.backbone == 0).cloned().collect::<Vec<_>>(), &[], draft);
        }
        let mut all = GrowthResult { message: format!("{} backbones · independently grown scrolls", self.curves.len()), ..Default::default() };
        // the usual scrolls first, then vines in order, each vine keeping
        // clear of everything grown before it; parts stay in backbone order
        let mut grown: Vec<Option<GrowthResult>> = vec![None; self.curves.len()];
        let order: Vec<usize> = (0..self.curves.len()).filter(|&i| !self.is_vine(i)).chain((0..self.curves.len()).filter(|&i| self.is_vine(i))).collect();
        for index in order {
            let prefix = format!("backbone-{index}/");
            let strip = |s: &str| s.rsplit('/').next().unwrap().to_string();
            let locked: Vec<GrowthPart> = self.locked_parts.iter().filter(|p| p.id.starts_with(&prefix)).map(|p| GrowthPart { id: strip(&p.id), parent: p.parent.as_deref().map(strip), ..p.clone() }).collect();
            let shoots: Vec<ShootEdit> = self.shoots.iter().filter(|e| e.backbone == index).map(|e| ShootEdit { backbone: 0, ..e.clone() }).collect();
            let obstacles: Vec<Vec<Point>> = if self.is_vine(index) { grown.iter().flatten().flat_map(|r| r.parts.iter().map(|p| p.polygon.clone())).collect() } else { vec![] };
            grown[index] = Some(self.grow_one(index, &locked, &shoots, &obstacles, draft));
        }
        for (index, r) in grown.into_iter().enumerate() {
            let prefix = format!("backbone-{index}/");
            let Some(r) = r else { continue };
            all.parts.extend(r.parts.into_iter().map(|p| GrowthPart { id: format!("{prefix}{}", p.id), parent: p.parent.map(|q| format!("{prefix}{q}")), ..p }));
        }
        // An attached backbone's sweep becomes a child of its parent's sweep,
        // so the two merge where they meet.
        let mains: Vec<Option<String>> = (0..self.curves.len()).map(|i| { let pre = format!("backbone-{i}/"); all.parts.iter().find(|p| p.parent.is_none() && p.id.starts_with(&pre)).map(|p| p.id.clone()) }).collect();
        for i in 0..self.curves.len() {
            let Some(parent) = self.growth_for(i).attach.filter(|&p| p != i && p < self.curves.len()) else { continue };
            let (Some(child), Some(pid)) = (mains[i].clone(), mains[parent].clone()) else { continue };
            if let Some(part) = all.parts.iter_mut().find(|p| p.id == child) { part.parent = Some(pid.clone()); }
            // The child's root flare must not poke out through the far side of
            // the parent: outline points around the root that fall outside
            // the parent (and are not already heading up the child) are drawn
            // back into it, where the join hides them.
            if let (Some(join), Some(par)) = (self.attach_point(i), all.parts.iter().find(|p| p.id == pid).map(|p| p.polygon.clone())) {
                if let Some(part) = all.parts.iter_mut().find(|p| p.id == child) { tuck_root(part, &par, join); }
            }
            // collar leafage over the fork, drawn on top of everything grown so far
            let s = self.growth_for(i);
            if let Some(size) = s.collar.filter(|v| v.is_finite() && *v > 0.0) {
                let style = s.collar_style.as_deref().and_then(crate::collar::CollarStyle::from_id).unwrap_or(crate::collar::CollarStyle::Axil);
                let join = self.attach_point(i);
                let (par, ch) = (all.parts.iter().find(|p| p.id == pid), all.parts.iter().find(|p| p.id == child));
                if let (Some(join), Some(par), Some(ch)) = (join, par, ch) {
                    let leaves = crate::collar::collar_parts(style, &format!("backbone-{i}/collar"), par, ch, join, size);
                    all.parts.extend(leaves);
                }
            }
        }
        all
    }
    /// The pattern SVG at physical millimetre size.
    pub fn svg(&self) -> String { self.svg_with(false) }
    /// The pattern SVG, drawn with smooth filleted joins when `smooth`.
    pub fn svg_with(&self, smooth: bool) -> String { self.svg_joins(if smooth { JoinStyle::Smooth } else { JoinStyle::Classic }) }
    /// The pattern SVG with the given root joins.
    pub fn svg_joins(&self, joins: JoinStyle) -> String {
        let d = joins.draw(&self.grow());
        let backbone = if self.print_backbone { self.curves.iter().map(|c| format!("<path d=\"M {} {} C {} {} {} {} {} {}\" stroke-width=\".35\"/>", c[0].x, c[0].y, c[1].x, c[1].y, c[2].x, c[2].y, c[3].x, c[3].y)).collect::<String>() } else { String::new() };
        format!("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}mm\" height=\"{h}mm\" viewBox=\"0 0 {w} {h}\"><title>ORNATR pattern</title><g fill=\"none\" stroke=\"#000\" stroke-linecap=\"round\" stroke-linejoin=\"round\">{backbone}<path d=\"{}\" stroke-width=\".35\"/><path d=\"{}\" stroke-width=\".2\"/></g></svg>", path_data(&d.outline, false), path_data(&d.folds, false), w = self.width, h = self.height)
    }
    /// Carving guides SVG: visible edges, raised ridges, recessed creases.
    pub fn carving_svg(&self) -> String {
        let g = carving_guides(&self.grow());
        format!("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}mm\" height=\"{h}mm\" viewBox=\"0 0 {w} {h}\"><title>ORNATR suggested carving guides</title><desc>Solid black: visible edges. Blue dashed: suggested raised ridges. Red dotted: recessed creases. Review before carving; not routing toolpaths.</desc><g fill=\"none\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><path id=\"visible-edges\" d=\"{}\" stroke=\"black\" stroke-width=\".35\"/><path id=\"raised-ridges\" d=\"{}\" stroke=\"#246a9b\" stroke-width=\".25\" stroke-dasharray=\"2 1\"/><path id=\"recessed-creases\" d=\"{}\" stroke=\"#a24434\" stroke-width=\".25\" stroke-dasharray=\".4 .8\"/></g></svg>", path_data(&g.outline, false), path_data(&g.ridges, false), path_data(&g.creases, false), w = self.width, h = self.height)
    }
}

/// How roots are drawn where a part grows from its parent.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum JoinStyle {
    /// Both outlines cut away inside a small circle (the golden-tested engine).
    Classic,
    /// Signed-distance blend around roots (`joins`).
    Smooth,
    /// Exact booleans: tidied outlines and round fillets of this radius in mm (`exact`).
    Exact(f64),
}
impl JoinStyle {
    pub fn draw(self, g: &GrowthResult) -> crate::layers::Drawing {
        match self {
            JoinStyle::Classic => layered_drawing(g),
            JoinStyle::Smooth => crate::joins::smooth_drawing(g),
            JoinStyle::Exact(fillet) => crate::exact::exact_drawing(g, crate::exact::ExactSettings { tidy: true, fillet }),
        }
    }
}

/// Library leaf types grown from measured spines.
pub struct LeafPreset { pub id: &'static str, pub name: &'static str, pub detail: &'static str, pub size: f64 }
pub const LEAF_PRESETS: &[LeafPreset] = &[
    LeafPreset { id: "returning-leaf", name: "Returning leaf", detail: "Full belly · two body lobes", size: 0.3 },
    LeafPreset { id: "leaf-volute", name: "Leaf volute", detail: "Open curl · broad inner leaf", size: 0.24 },
    LeafPreset { id: "rolled-fan", name: "Rolled fan", detail: "Broad crown · inward returns", size: 0.26 },
    LeafPreset { id: "turned-bud", name: "Turned leaf", detail: "Small accent · soft returning tip", size: 0.2 },
    LeafPreset { id: "two-finger-leaf", name: "Two-finger leaf", detail: "Two rounded fingers · shared taper", size: 0.24 },
    LeafPreset { id: "upright-sprig", name: "Upright sprig", detail: "Rising tip · paired soft lobes", size: 0.24 },
    LeafPreset { id: "sweeping-tongue", name: "Sweeping leaf", detail: "Long belly · single folded return", size: 0.28 },
];
pub fn preset_params(id: &str, progress: f64, side: f64) -> Option<ShootParams> {
    if crate::bud::is_bud(id) { return crate::bud::bud_params(id, progress, side); }
    let p = LEAF_PRESETS.iter().find(|p| p.id == id)?; let prof = profile(id)?;
    Some(ShootParams { progress, reach: p.size * prof.length, turn: prof.frame * side, curl: 0.66, side, preset: Some(id.to_string()), ..ShootParams::default() })
}

/// Old stamps become grown leaves of the matching type at the same root,
/// size and angle; their backbones stop adding automatic shoots.
pub fn convert_legacy(layout: &mut Layout, new_id: &mut dyn FnMut() -> String) {
    let mut touched = vec![];
    for item in std::mem::take(&mut layout.items) {
        let b = item.backbone.min(layout.curves.len() - 1); let length = arc_table(&layout.curves[b])[240].length;
        let id = if profile(&item.motif).is_some() { item.motif.as_str() } else { LEAF_PRESETS[0].id };
        let prof = profile(id).unwrap(); let m = if item.mirror { -1.0 } else { 1.0 };
        let mut params = preset_params(id, item.progress, 1.0).unwrap();
        params.reach = (item.length * prof.length / length.max(1.0)).clamp(0.02, 1.5);
        params.turn = item.angle.to_radians() + m * prof.frame; params.side = m;
        params.leaf_scale = Some(item.fullness.clamp(0.2, 2.5)); params.bend = Some(if item.bend != 0.0 { item.bend / 40.0 } else { 0.0 });
        layout.shoots.push(ShootEdit { params, id: new_id(), backbone: b, replaces: None, hidden: false, under: item.on_top == Some(false) });
        if !touched.contains(&b) { touched.push(b); }
    }
    for b in touched { while layout.growth.len() <= b { let g = layout.growth_for(layout.growth.len()); layout.growth.push(g); } layout.growth[b].auto_shoots = Some(false); }
}

pub fn point_list(p: &[Point]) -> String { p.iter().map(|q| format!("{} {}", q.x, q.y)).collect::<Vec<_>>().join(" ") }

/// A carving surface preset, fitted inside a `width` × `height` page with a
/// small border: "plaque" (a routed board with notched corners), "oval" or
/// "rectangle". None for an unknown id.
pub fn surface_outline(id: &str, width: f64, height: f64) -> Option<Vec<Point>> {
    use crate::surfaces as frames;
    let m = (width.min(height) * 0.04).clamp(3.0, 12.0);
    let (w, h) = (width - 2.0 * m, height - 2.0 * m);
    let raw = match id { "plaque" => frames::plaque(w, h), "oval" => frames::oval(w, h), "rectangle" => frames::rectangle(w, h), _ => return None };
    // fit the outline's box (the plaque's arches reach past its nominal box) into the border
    let b = crate::geometry::Bounds::of(&raw);
    let (sx, sy) = (w / (b.r - b.l).max(1e-9), h / (b.b - b.t).max(1e-9));
    Some(raw.iter().map(|p| pt(m + (p.x - b.l) * sx, m + (p.y - b.t) * sy)).collect())
}

thread_local! {
    /// Grown vines by their inputs: growing one takes up to a second, so an
    /// unchanged vine is not grown again when something else changes.
    static VINES: std::cell::RefCell<std::collections::HashMap<u64, GrowthResult>> = Default::default();
}

/// A scroll vine along `curve`: curls seeded `spacing` mm apart, grown to fit
/// and never touching each other or the `obstacles` (other backbones' parts),
/// clad with acanthus (or plain carved scrolls when leaves are off), fitted
/// inside the surface (the page less a border when none is set). With `edits`
/// (curls edited by hand) the vine is built exactly from them instead. A
/// draft of an unedited vine is the stem alone.
#[allow(clippy::too_many_arguments)]
pub fn grow_vine(width: f64, height: f64, curve: &Curve, s: &GrowthSettings, spacing: f64, surface: Option<&[Point]>, obstacles: &[Vec<Point>], edits: &[ShootEdit], draft: bool) -> GrowthResult {
    use crate::curls::{build_scrolls, dress, dress_acanthus, grow_scrolls, scroll_leaf, Dress, ScrollOptions, ScrollResult};
    let path: Vec<Point> = arc_table(curve).into_iter().map(|r| r.point).collect();
    let page = [pt(4.0, 4.0), pt(width - 4.0, 4.0), pt(width - 4.0, height - 4.0), pt(4.0, height - 4.0)];
    let surface: Vec<Point> = surface.map(|v| v.to_vec()).unwrap_or_else(|| page.to_vec());
    let stem = (s.stem * 2.1).clamp(3.0, 12.0);
    if draft && edits.is_empty() { return dress(&ScrollResult { curve: path, scrolls: vec![], outside: 0.0 }, Dress::Carved, stem); }
    // sizes follow the page: tuned on a 300 x 130 mm panel
    let k = (width.min(height) / 130.0).clamp(0.4, 3.0);
    let leaves = s.leaves > 0;
    let (spec, reach, max_half) = scroll_leaf(); let max_half = max_half * k;
    let key = {
        let mut h: u64 = 0xcbf29ce484222325;
        let mut eat = |v: f64| { h ^= v.to_bits(); h = h.wrapping_mul(0x100000001b3); };
        for p in curve { eat(p.x); eat(p.y); }
        for p in &surface { eat(p.x); eat(p.y); }
        for o in obstacles { eat(o.len() as f64); for p in o.iter().step_by(7) { eat(p.x); eat(p.y); } }
        for e in edits { let p = &e.params; for v in [p.progress, p.reach, p.turn, p.curl, p.side, p.leaf_scale.unwrap_or(1.0), if e.hidden { 1.0 } else { 0.0 }] { eat(v); } for b in e.id.bytes().chain(p.on.as_deref().unwrap_or("").bytes()) { eat(b as f64); } }
        for v in [width, height, spacing, s.stem, s.seed as f64, if leaves { 1.0 } else { 0.0 }] { eat(v); }
        h
    };
    if let Some(hit) = VINES.with(|c| c.borrow().get(&key).cloned()) { return hit; }
    let band = if leaves { max_half * 1.3 } else { stem };
    let res = if edits.is_empty() {
        let o = ScrollOptions { seed_spacing: spacing, max_length: 260.0 * k, min_length: 45.0 * k, width: band, clearance: 3.0, generations: 2, branch_from: 90.0 * k, fill_gap: 16.0 * k, seed: s.seed };
        grow_scrolls(&path, false, &surface, obstacles, &o)
    } else {
        let list: Vec<(String, crate::shoots::ShootParams, bool)> = edits.iter().map(|e| (e.id.clone(), e.params.clone(), e.hidden)).collect();
        build_scrolls(&path, &list, band)
    };
    let grown = if leaves { dress_acanthus(&res, &surface, obstacles, stem, &spec, reach, max_half) } else { dress(&res, Dress::Carved, stem) };
    // the final boolean check: every part, the stem too, is clipped to the surface
    let surf = crate::booleans::union(&[surface.as_slice()]);
    let parts = grown.parts.into_iter().filter_map(|mut p| {
        let inside = crate::booleans::intersect(&crate::booleans::union(&[p.polygon.as_slice()]), &surf);
        let piece = inside.into_iter().max_by(|a, b| crate::booleans::area(&vec![a.clone()]).partial_cmp(&crate::booleans::area(&vec![b.clone()])).unwrap())?;
        p.polygon = piece[0].clone();
        // its inner lines (folds, slits and eyes) too, to what is left of it
        let kept = p.polygon.clone(); p.folds = clip_lines(&p.folds, &kept); p.cuts = clip_lines(&p.cuts, &kept);
        Some(p)
    }).collect();
    let message = if edits.is_empty() { format!("Scroll vine: {} curls", res.scrolls.len()) } else { format!("Scroll vine: {} curls, edited by hand", res.scrolls.len()) };
    let grown = GrowthResult { message, parts, ..grown };
    VINES.with(|c| { let mut c = c.borrow_mut(); if c.len() > 24 { c.clear(); } c.insert(key, grown.clone()); });
    grown
}

/// The parts of `lines` that lie inside `area` (tested every half millimetre).
fn clip_lines(lines: &[Vec<Point>], area: &[Point]) -> Vec<Vec<Point>> {
    let mut out = vec![];
    for line in lines {
        let mut cur: Vec<Point> = vec![];
        for w in line.windows(2) {
            let n = (distance(w[0], w[1]) / 0.5).ceil().max(1.0) as usize;
            for k in 0..n {
                let (a, b) = (k as f64 / n as f64, (k + 1) as f64 / n as f64);
                let (p, q) = (pt(w[0].x + (w[1].x - w[0].x) * a, w[0].y + (w[1].y - w[0].y) * a), pt(w[0].x + (w[1].x - w[0].x) * b, w[0].y + (w[1].y - w[0].y) * b));
                if crate::outline::inside(pt((p.x + q.x) / 2.0, (p.y + q.y) / 2.0), area) { if cur.is_empty() { cur.push(p); } cur.push(q); }
                else if cur.len() > 1 { out.push(std::mem::take(&mut cur)); } else { cur.clear(); }
            }
        }
        if cur.len() > 1 { out.push(cur); }
    }
    out
}
