//! Scroll skeletons: named constructions built from several linked scrolls,
//! after the user's references (the "9" parent-and-child skeleton, S-scrolls,
//! mirrored pairs meeting at a centre, running borders with tapering rhythm,
//! a fan from one point of origin, and corners). A seed varies proportions,
//! angles, curl directions and how many children or repeats appear, without
//! changing what the construction is. The result is ordinary backbones, all
//! editable afterwards.
use crate::bud::bud_params;
use crate::geometry::{arc_table, line_frame, pt, Bounds, Curve, Point};
use crate::growth::{Family, GrowthSettings, Mulberry, Side};
use crate::model::Layout;
use crate::shoots::ShootEdit;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Skeleton { Single, ParentChild, SScroll, MirroredPair, RunningBorder, Fan, Corner }

impl Skeleton {
    pub const ALL: [Skeleton; 7] = [Skeleton::Single, Skeleton::ParentChild, Skeleton::SScroll, Skeleton::MirroredPair, Skeleton::RunningBorder, Skeleton::Fan, Skeleton::Corner];
    pub fn name(self) -> &'static str { match self { Skeleton::Single => "Single scroll", Skeleton::ParentChild => "Parent and child", Skeleton::SScroll => "S-scroll", Skeleton::MirroredPair => "Mirrored pair", Skeleton::RunningBorder => "Running border", Skeleton::Fan => "Point of origin", Skeleton::Corner => "Corner" } }
    pub fn detail(self) -> &'static str { match self {
        Skeleton::Single => "One backbone rising into a volute: the original construction",
        Skeleton::ParentChild => "A main volute with a smaller scroll forking from its sweep",
        Skeleton::SScroll => "One stem, a volute at each end turning opposite ways",
        Skeleton::MirroredPair => "Two scrolls meeting at a centre bud",
        Skeleton::RunningBorder => "Scrolls chained along a band, each smaller than the last",
        Skeleton::Fan => "Scrolls fanning from one root, sized 100 / 66 / 33",
        Skeleton::Corner => "Two scrolls along the edges from a corner bud",
    } }
}

/// One scroll of a construction, in a 240 × 150 design frame (y down,
/// angles in degrees, 0 = right, -90 = up).
struct Stem { start: (f64, f64), a0: f64, end: (f64, f64), a1: f64, k0: f64, k1: f64, curl: Side, attach: Option<usize>, accents: u8, scale: f64, flip: bool, family: Family }

fn dir(deg: f64) -> Point { let r = deg.to_radians(); pt(r.cos(), r.sin()) }
fn curve(s: &Stem) -> Curve {
    let (a, b) = (pt(s.start.0, s.start.1), pt(s.end.0, s.end.1));
    let d = (b.x - a.x).hypot(b.y - a.y); let (u, v) = (dir(s.a0), dir(s.a1));
    [a, pt(a.x + u.x * d * s.k0, a.y + u.y * d * s.k0), pt(b.x - v.x * d * s.k1, b.y - v.y * d * s.k1), b]
}
fn mirror_x(s: &Stem, cx: f64) -> Stem {
    Stem { start: (2.0 * cx - s.start.0, s.start.1), a0: 180.0 - s.a0, end: (2.0 * cx - s.end.0, s.end.1), a1: 180.0 - s.a1, flip: !s.flip, ..*s }
}
fn rotate180(s: &Stem, c: (f64, f64)) -> Stem {
    Stem { start: (2.0 * c.0 - s.start.0, 2.0 * c.1 - s.start.1), a0: s.a0 + 180.0, end: (2.0 * c.0 - s.end.0, 2.0 * c.1 - s.end.1), a1: s.a1 + 180.0, ..*s }
}
fn other(side: Side) -> Side { if side == Side::Right { Side::Left } else { Side::Right } }

/// A bud to place: backbone, position along it, absolute heading, type.
struct BudAt { backbone: usize, progress: f64, heading: f64, kind: &'static str }

fn design(kind: Skeleton, seed: u32) -> (Vec<Stem>, Vec<BudAt>) {
    let mut rng = Mulberry(seed.wrapping_mul(2654435761).wrapping_add(kind as u32));
    let mut j = |amount: f64| (rng.next() * 2.0 - 1.0) * amount;
    // Curl sides turn relative to the heading: Right = clockwise on the page.
    let st = |start: (f64, f64), a0: f64, end: (f64, f64), a1: f64, curl: Side| Stem { start, a0, end, a1, k0: 0.42, k1: 0.36, curl, attach: None, accents: 2, scale: 1.0, flip: false, family: Family::Spiral };
    let point_on = |s: &Stem, t: f64| { let c = curve(s); let p = arc_table(&c)[(t.clamp(0.0, 1.0) * 240.0) as usize].point; (p.x, p.y) };
    let mirror_all = j(1.0) > 0.0;
    let (mut stems, mut buds) = match kind {
        Skeleton::Single => {
            // The original: one backbone rising into its volute, with accents.
            let turn = if j(1.0) > 0.0 { Side::Right } else { Side::Alternate };
            let main = Stem { k0: 0.25 + j(0.06), k1: 0.6 + j(0.08), ..st((34.0, 112.0 + j(6.0)), 14.0 + j(8.0), (196.0 + j(8.0), 86.0 + j(8.0)), 13.0 + j(10.0), turn) };
            (vec![main], vec![])
        }
        Skeleton::ParentChild => {
            // The "9": a main sweep rising into a volute; low on the sweep a
            // child forks away at a shallow angle, like a branch, and hangs its
            // own volute the other way.
            let main = Stem { k0: 0.45 + j(0.06), k1: 0.34 + j(0.05), ..st((22.0, 118.0 + j(6.0)), -8.0 + j(8.0), (158.0 + j(12.0), 46.0 + j(8.0)), -40.0 + j(12.0), Side::Left) };
            let at = 0.3 + j(0.05);
            let from = point_on(&main, at);
            let tan = { let c = curve(&main); let pts: Vec<Point> = arc_table(&c).into_iter().map(|r| r.point).collect(); line_frame(&pts, at).1.to_degrees() };
            let child = Stem { attach: Some(0), accents: 1, scale: 0.8, k0: 0.4, k1: 0.4, ..st(from, tan + 26.0 + j(6.0), (from.0 + 84.0 + j(10.0), from.1 + 26.0 + j(6.0)), 35.0 + j(12.0), Side::Right) };
            (vec![main, child], vec![])
        }
        Skeleton::SScroll => {
            let c = (120.0, 75.0);
            let turn = if j(1.0) > 0.0 { Side::Right } else { Side::Left };
            let a = Stem { k0: 0.5 + j(0.08), k1: 0.4 + j(0.06), ..st(c, -18.0 + j(10.0), (196.0 + j(10.0), 40.0 + j(8.0)), -75.0 + j(15.0), turn) };
            let mut b = rotate180(&a, c); b.attach = Some(0);
            // Unequal halves read livelier: shorten one end a little.
            let shrink = 0.78 + j(0.1); b.end = (c.0 + (b.end.0 - c.0) * shrink, c.1 + (b.end.1 - c.1) * shrink); b.accents = 1;
            (vec![a, b], vec![])
        }
        Skeleton::MirroredPair => {
            // A lyre: both halves leave a shared root, sweep out and up, and
            // turn their volutes inward toward each other; a bud rises between.
            let cx = 120.0;
            let a = Stem { k0: 0.6 + j(0.08), k1: 0.45 + j(0.06), ..st((cx + 3.0, 130.0), -14.0 + j(8.0), (200.0 + j(8.0), 52.0 + j(10.0)), -112.0 + j(10.0), Side::Left) };
            let mut b = mirror_x(&a, cx); b.start = (cx - 3.0, 130.0); b.attach = Some(0);
            let bud = ["bud-trefoil", "bud-husk", "bud-berries"][((j(1.0) + 1.0) * 1.5) as usize % 3];
            (vec![a, b], vec![BudAt { backbone: 0, progress: 0.0, heading: -90.0, kind: bud }])
        }
        Skeleton::RunningBorder => {
            // A vine: one running stem along the band, gently waved, ending in
            // a small volute; scrolls branch from it at even steps, alternately
            // above and below, each arching forward and coiling back into its
            // own bay, each a little smaller than the last.
            let n = 3 + ((j(1.0) + 1.0) * 1.5) as usize % 3; // 3 to 5 branches
            let y0 = 76.0;
            let lift = j(6.0);
            // The running stem has plain ends: the branches carry the volutes.
            let main = Stem { accents: 0, k0: 0.4, k1: 0.4, family: Family::Border, ..st((8.0, y0 + 6.0 + lift), -8.0, (232.0, y0 - 6.0 - lift), 10.0 + j(10.0), Side::Right) };
            let mut stems = vec![main];
            let mut size = 1.0; let mut up = j(1.0) > 0.0;
            let d = 200.0 / (n as f64 + 0.4);
            for i in 0..n {
                let t = (0.06 + (i as f64 + 0.2) * 0.86 / n as f64).min(0.9);
                let start = point_on(&stems[0], t);
                let sgn = if up { -1.0 } else { 1.0 };
                let end = (start.0 + d * 0.95 * size, start.1 + sgn * 28.0 * size);
                // Leave the stem at a shallow angle, level out, and coil away
                // from the stem into the open bay.
                stems.push(Stem { attach: Some(0), accents: 1, scale: 0.6 + 0.3 * size, k0: 0.35, k1: 0.45,
                    ..st(start, sgn * (26.0 + j(5.0)), end, sgn * (8.0 + j(8.0)), if up { Side::Left } else { Side::Right }) });
                size *= 0.9; up = !up;
            }
            (stems, vec![])
        }
        Skeleton::Fan => {
            // A short common stem, then scrolls sized 100 / 66 / 33 springing
            // from one point and turning the same way.
            let main = Stem { k0: 0.45, k1: 0.35, ..st((16.0, 136.0), -12.0 + j(5.0), (192.0 + j(8.0), 44.0 + j(8.0)), -70.0 + j(10.0), Side::Left) };
            // Children leave at a clear angle (no long slivers between stems),
            // from slightly staggered points clear of the main stem's accents.
            let count = if j(1.0) > -0.3 { 3 } else { 2 };
            let mut stems = vec![main];
            for i in 1..count {
                let at = 0.3 + 0.035 * (i as f64 - 1.0); // between the main stem's accents at 0.24 and 0.38
                let origin = point_on(&stems[0], at);
                let tan = { let c = curve(&stems[0]); let pts: Vec<Point> = arc_table(&c).into_iter().map(|r| r.point).collect(); line_frame(&pts, at).1.to_degrees() };
                let size = [1.0, 0.66, 0.33][i];
                let a0 = tan - 30.0 - 14.0 * (i as f64 - 1.0) + j(4.0);
                let len = 150.0 * size; let d = dir(a0 - 4.0);
                stems.push(Stem { attach: Some(0), accents: 0, scale: 0.55 + 0.4 * size, k0: 0.4, k1: 0.35,
                    ..st(origin, a0, (origin.0 + d.x * len, origin.1 + d.y * len), a0 - 50.0 + j(8.0), Side::Left) });
            }
            (stems, vec![])
        }
        Skeleton::Corner => {
            let c = (26.0, 124.0);
            // Both stems leave the corner bud on the diagonal and bend onto their edges.
            let a = Stem { k0: 0.36, k1: 0.4, ..st((c.0 + 4.0, c.1), -44.0 + j(5.0), (204.0 + j(8.0), 98.0 + j(10.0)), -95.0 + j(12.0), Side::Left) };
            // The partner runs up the left edge: the same scroll reflected in
            // the corner's diagonal.
            let reflect = |p: (f64, f64)| (c.0 - (p.1 - c.1), c.1 - (p.0 - c.0));
            let mut b = Stem { start: reflect(a.start), end: reflect(a.end), a0: -90.0 - a.a0, a1: -90.0 - a.a1, flip: true, attach: Some(0), accents: 1, ..a };
            b.start = (c.0, c.1 - 4.0);
            let bud = ["bud-husk", "bud-trefoil", "bud-berries"][((j(1.0) + 1.0) * 1.5) as usize % 3];
            (vec![a, b], vec![BudAt { backbone: 0, progress: 0.0, heading: -45.0, kind: bud }])
        }
    };
    // Mirroring the whole construction is a variation that never breaks it.
    if mirror_all {
        stems = stems.iter().map(|s| mirror_x(s, 120.0)).collect();
        for b in buds.iter_mut() { b.heading = 180.0 - b.heading; }
    }
    (stems, buds)
}

fn scaled(l: &mut Layout, f: f64, from: Point, to: Point) {
    for c in l.curves.iter_mut() { for p in c.iter_mut() { *p = pt(to.x + (p.x - from.x) * f, to.y + (p.y - from.y) * f); } }
}

/// Build a construction on a `width` × `height` page, fitted with a margin.
pub fn skeleton_layout(kind: Skeleton, seed: u32, width: f64, height: f64) -> Layout {
    let (stems, buds) = design(kind, seed);
    let mut l = Layout { width, height, curves: stems.iter().map(curve).collect(), growth: vec![], locked_parts: vec![], shoots: vec![], items: vec![], print_backbone: false, frame: None, surface: None };
    l.growth = stems.iter().enumerate().map(|(i, s)| GrowthSettings {
        seed: seed.wrapping_add(i as u32 * 7919), family: Some(s.family), side: s.curl, levels: if s.accents >= 2 { 2 } else { 1 },
        auto_shoots: Some(s.accents > 0), secondary_scale: Some(s.scale.clamp(0.5, 2.0)), flip: if s.flip { Some(true) } else { None },
        free: Some(true), attach: s.attach,
        // collars dress each fork; left bare where a bud already sits on the join
        // (corner, mirrored pair) and on the fan, whose roots are too close together
        collar: if s.attach.is_some() && !matches!(kind, Skeleton::Corner | Skeleton::MirroredPair | Skeleton::Fan) { Some(1.0) } else { None },
        ..GrowthSettings::default() }).collect();
    // Buds: heading given absolutely, turned relative to the stem there.
    for (k, b) in buds.iter().enumerate() {
        let pts: Vec<Point> = arc_table(&l.curves[b.backbone]).into_iter().map(|r| r.point).collect();
        let (_, fa) = line_frame(&pts, b.progress);
        let Some(mut p) = bud_params(b.kind, b.progress, 1.0) else { continue };
        p.turn = b.heading.to_radians() - fa; p.reach = if kind == Skeleton::Corner { 0.2 } else { 0.16 };
        l.shoots.push(ShootEdit { params: p, id: format!("bud-{k}"), backbone: b.backbone, replaces: None, hidden: false, under: false });
    }
    // Scale uniformly to the page, then fit the grown result: volutes and
    // leaves extend beyond the backbones, so refit on what actually grew.
    let s0 = (width / 240.0).min(height / 150.0);
    scaled(&mut l, s0, pt(120.0, 75.0), pt(width / 2.0, height / 2.0));
    let margin = (width.min(height) * 0.05).max(4.0);
    for _ in 0..4 {
        let grown = l.grow();
        let all: Vec<Point> = grown.parts.iter().flat_map(|p| p.polygon.iter().copied()).collect();
        if all.is_empty() { break; }
        let b = Bounds::of(&all);
        let f = ((width - 2.0 * margin) / (b.r - b.l)).min((height - 2.0 * margin) / (b.b - b.t));
        let centre = pt((b.l + b.r) / 2.0, (b.t + b.b) / 2.0);
        if (f - 1.0).abs() < 0.01 && (centre.x - width / 2.0).abs() < 0.5 && (centre.y - height / 2.0).abs() < 0.5 { break; }
        scaled(&mut l, f, centre, pt(width / 2.0, height / 2.0));
    }
    // Store attached starts where growth puts them, so handles match the drawing.
    let mut rounds = 0; while rounds < 6 && l.settle() { rounds += 1; }
    l
}
