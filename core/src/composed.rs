//! Composed families: adaptive placement of supporting sweeps along the guide.
use crate::contour::{acanthus_contour, leaf_notches, root_flare, shoot_stalk, ContourOptions, Ends, GOLDEN_SMALL as PHI, OPEN_ENDS};
use crate::geometry::{distance, line_frame, line_length, Point};
use crate::growth::{grow_curl, Family, GrowthPart, GrowthResult, GrowthSettings, Kind, Page, Side};
use crate::outline::inside;
use crate::shoots::ShootParams;

struct Lcg(u32);
impl Lcg { fn next(&mut self) -> f64 { self.0 = self.0.wrapping_mul(1664525).wrapping_add(1013904223); self.0 as f64 / 4294967296.0 } }

struct Candidate { points: Vec<Point>, score: f64, side: f64, size: f64, root: Point, bend: f64, t: f64, curl: f64 }

pub fn composed_growth(page: &Page, s: &GrowthSettings) -> GrowthResult {
    let family = s.family.unwrap_or(Family::Spiral);
    let mut main = crate::spiral::spiral_anatomy(page, &GrowthSettings { levels: 1, sweeps: Some(1), ..s.clone() }).parts.swap_remove(0);
    let guide = page.guide(); let length = line_length(&guide);
    let flip = if s.flip == Some(true) { -1.0 } else { 1.0 };
    let open_family = matches!(family, Family::Spray | Family::Fan | Family::Border);
    if open_family {
        // an attached stem flares out of its parent (wider under a collar)
        // and keeps real width past the flare instead of pinching
        let collar = s.attach.and(s.collar).filter(|c| c.is_finite() && *c > 0.0);
        let (ends, root_width) = match (s.attach.is_some(), collar) { (true, Some(c)) => (OPEN_ENDS, 2.6 * c.clamp(0.6, 1.6)), (true, None) => (Ends { start: 0.8, tip: OPEN_ENDS.tip }, 1.4), _ => (OPEN_ENDS, 0.0) };
        let o = ContourOptions { lobed: false, notches: leaf_notches(), ends: Some(ends), root_width, ..ContourOptions::default() };
        let a = acanthus_contour(&guide, 1.5, (if s.side == Side::Right { 1.0 } else { -1.0 }) * flip, length * if family == Family::Fan { 0.035 } else { 0.012 }, &o);
        main.points = guide.clone(); main.length = length; main.polygon = a.polygon; main.folds = vec![]; main.ridges = Some(vec![]); main.contour_split = Some(241);
    }
    let mut parts: Vec<GrowthPart> = page.locked.to_vec();
    // The freshly grown main is used for joins; it only counts as "the main"
    // in overlap checks when it was inserted (not replaced by a kept part).
    let main_index = if parts.iter().any(|p| p.id == main.id) { None } else { parts.insert(0, main.clone()); Some(0) };
    let mut random = Lcg(s.seed);
    let reach = 36f64.min(length * (1.0 - PHI) * PHI);
    let roles: Vec<&str> = match family {
        Family::Border => vec!["roll-0", "roll-1", "roll-2", "roll-3", "roll-4"],
        Family::Fan => vec!["fan-0", "fan-1", "fan-2"],
        Family::Spray => vec!["spray-0", "spray-1", "spray-2"],
        Family::Branching => vec!["companion", "shoot-2", "shoot-0", "shoot-1"],
        Family::Spiral => if s.sweeps == Some(2) { vec!["companion", "shoot-2", "shoot-0", "shoot-1"] } else if s.levels == 1 { vec!["shoot-2"] } else { vec!["shoot-2", "shoot-0", "shoot-1"] },
    };
    let curl_for = match family { Family::Spray => 0.30, Family::Fan => 0.22, Family::Border => 0.60, _ => 0.66 };
    let (mut attempts, mut skipped) = (0usize, 0usize);
    for (role, id) in roles.iter().enumerate() {
        if parts.iter().any(|p| p.id == *id) { continue; }
        let ratio = match family { Family::Border => 0.48, Family::Fan => [0.85, 1.0, 0.618][role], Family::Spray => [1.0, 0.75, 0.5][role], _ => match *id { "companion" => PHI, "shoot-2" => PHI * 0.85, "shoot-0" => PHI.powi(2), _ => PHI.powi(3) } };
        let mut candidates: Vec<Candidate> = vec![];
        for station in 0..12 {
            let center = match family { Family::Border => Some(0.12 + role as f64 * 0.16), Family::Fan => Some(0.38 + role as f64 * 0.10), Family::Spray => Some(0.20 + role as f64 * 0.24), _ => None };
            let t = match center { None => 0.16 + station as f64 * 0.053 + (random.next() - 0.5) * 0.025, Some(c) => c + (station as f64 - 5.5) * 0.004 + (random.next() - 0.5) * 0.01 };
            let (fp, fa) = line_frame(&guide, t); let a = line_frame(&guide, (t - 0.04).max(0.0)).1; let b = line_frame(&guide, (t + 0.04).min(1.0)).1;
            let bend = (b - a).sin().atan2((b - a).cos());
            let sides: Vec<f64> = match s.side { Side::Left => vec![-1.0], Side::Right => vec![1.0], Side::Alternate => match family { Family::Border => vec![if role % 2 == 1 { 1.0 } else { -1.0 }], Family::Fan => vec![-1.0], _ => vec![-1.0, 1.0] } }.into_iter().map(|v| v * flip).collect();
            for side in sides { for shrink in [1.0, 0.8, 0.6] {
                attempts += 1;
                let size = reach * ratio * s.secondary_scale.unwrap_or(1.0) * shrink * (0.94 + random.next() * 0.12);
                let points = grow_curl(fp, fa, size, curl_for, side);
                if !page.fits(&points, 3.0, s.free == Some(true)) { continue; }
                let skip = if family == Family::Spiral { 24 } else { 48 };
                let mut gap = f64::INFINITY;
                for (i, p) in points.iter().enumerate() { if !(i > skip && i % 8 == 0) { continue; } for other in &parts { for (j, q) in other.points.iter().enumerate() { if j % 12 == 0 { gap = gap.min(distance(*p, *q)); } } } }
                if gap < 1.3f64.max(size * 0.12) { continue; }
                let mut root_space = length;
                for p in parts.iter().filter(|p| p.parent.is_some()) { root_space = root_space.min(distance(p.points[0], fp)); }
                let score = gap.min(20.0) + root_space.min(20.0) * 0.25 + bend.abs() * 4.0 + if side * bend < 0.0 { 2.0 } else { 0.0 } + shrink * 4.0 + random.next() * 2.0;
                candidates.push(Candidate { points, score, side, size, root: fp, bend, t, curl: curl_for });
            } }
        }
        candidates.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
        let mut accepted = false;
        let open = matches!(family, Family::Spray | Family::Fan);
        for c in candidates.iter().take(12) {
            let leaf_sides = if open { vec![c.side] } else { vec![-c.side, c.side] };
            for leaf_side in leaf_sides {
                let len_c = line_length(&c.points);
                let o = ContourOptions { lobed: s.leaves > 0, root_width: root_flare(&main.polygon, c.root, len_c * (1.0 - PHI) * PHI), stalk: shoot_stalk(), eyes: s.eyes.unwrap_or(0), ..ContourOptions::default() };
                let a = acanthus_contour(&c.points, 1.5, leaf_side, len_c * (1.0 - PHI) * (if open { 1.0 } else { PHI }) * (0.95 - (c.bend.abs() * 0.2).min(0.2)), &o);
                if !page.fits(&a.polygon, 1.0, s.free == Some(true)) { continue; }
                let collar = 4f64.max(c.size * if family == Family::Spiral { 0.25 } else { 0.65 });
                let stalk_reach = collar.max(len_c * shoot_stalk() * 1.6);
                let mut collision = false;
                'outer: for (k, other) in parts.iter().enumerate() {
                    let r = if Some(k) == main_index { stalk_reach } else { collar };
                    let mut i = 0; while i < a.polygon.len() { let p = a.polygon[i]; if distance(p, c.root) > r && inside(p, &other.polygon) { collision = true; break 'outer; } i += 5; }
                    let mut i = 0; while i < other.polygon.len() { let p = other.polygon[i]; if distance(p, c.root) > r && inside(p, &a.polygon) { collision = true; break 'outer; } i += 8; }
                }
                if collision { continue; }
                parts.push(GrowthPart { id: id.to_string(), parent: Some(main.id.clone()), kind: if *id == "companion" { Kind::Primary } else { Kind::Secondary }, points: c.points.clone(), polygon: a.polygon, folds: a.folds, ridges: Some(a.ridges), cuts: a.cuts, contour_split: Some(241), width: 3.0, length: len_c, birth: 0.2 + role as f64 * 0.1, duration: 0.25,
                    shoot: Some(ShootParams { progress: c.t, reach: c.size / length, turn: 0.0, curl: c.curl, side: c.side, leaf_side: Some(leaf_side), stem: Some(1.5), leaf_scale: Some(0.95 - (c.bend.abs() * 0.2).min(0.2)), ..ShootParams::default() }), under: false });
                accepted = true; break;
            }
            if accepted { break; }
        }
        if !accepted { skipped += 1; }
    }
    let name = format!("{family:?}").to_lowercase();
    let n = parts.len() - 1;
    GrowthResult { parts, attempts, skipped, message: format!("{name} · {n} supporting sweeps{}", if skipped > 0 { format!(" · {skipped} omitted to preserve space") } else { String::new() }) }
}
