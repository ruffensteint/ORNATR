//! Study 03 construction: the tail runs straight into the enclosing spiral;
//! accent shoots share the rising sweep.
use crate::contour::{acanthus_contour, root_flare, shoot_stalk, ContourOptions, Ends, GOLDEN_SMALL, MAIN_ENDS};
use crate::geometry::{distance, line_frame, line_length, pt, Point};
use crate::growth::{curl_sweep, grow_curl, GrowthPart, GrowthResult, GrowthSettings, Kind, Mulberry, Page, Side, VOLUTE_SIZE, VOLUTE_TURNS};
use crate::shoots::ShootParams;
use std::f64::consts::PI;

fn wrap(mut a: f64) -> f64 { while a > PI { a -= 2.0 * PI; } while a < -PI { a += 2.0 * PI; } a }

fn band(points: Vec<Point>, width: f64, kind: Kind, id: &str, parent: Option<&str>, birth: f64) -> GrowthPart {
    let (mut left, mut right) = (vec![], vec![]);
    let n = points.len(); let length = line_length(&points);
    for i in 0..n {
        let t = i as f64 / (n as f64 - 1.0); let p = points[i];
        let a = points[i.saturating_sub(1)]; let b = points[(i + 1).min(n - 1)];
        let angle = (b.y - a.y).atan2(b.x - a.x); let (nx, ny) = (-angle.sin(), angle.cos());
        let taper = (PI * t).sin().powf(0.7) * 1f64.min(((1.0 - t) / 0.23).powi(2));
        let lobe = 0.3; let tip = 1f64.min(((1.0 - t) / 0.15).powi(2));
        let prev = points[i.saturating_sub(2)]; let next = points[(i + 2).min(n - 1)];
        let turn = wrap((next.y - p.y).atan2(next.x - p.x) - (p.y - prev.y).atan2(p.x - prev.x));
        let radius = distance(prev, next) / (2.0 * 0.001f64.max(turn.abs()));
        let mut clearance = f64::INFINITY;
        for j in 0..n { if (j as isize - i as isize).abs() > 35 { clearance = clearance.min(distance(p, points[j]) * 0.42); } }
        let cap = 0.02f64.max((radius * 0.55).min(clearance));
        let outer = cap.min(width * (0.035 + 0.20 * taper) * tip); let inner = cap.min(width * (0.035 + taper * lobe) * tip);
        left.push(pt(p.x + nx * outer, p.y + ny * outer)); right.push(pt(p.x - nx * inner, p.y - ny * inner));
    }
    right.reverse(); left.extend(right);
    GrowthPart { id: id.into(), parent: parent.map(|s| s.to_string()), kind, points, polygon: left, folds: vec![], ridges: None, cuts: vec![], width, length, birth, duration: if kind == Kind::Primary { 0.52 } else { 0.25 }, contour_split: None, shoot: None, under: false }
}

/// The main scroll's volute, grown past the guide's end: the largest that
/// fits (shrunk in steps), on the preferred side, else the other.
pub(crate) struct MainVolute {
    /// The curl (its first point is the guide's end) and the side it turns to (±1).
    pub points: Vec<Point>, pub side: f64,
    /// The automatic reach after fitting (shoots follow it), the reach the
    /// leaf is sized from, and the reach and sweep (radians) it was grown with.
    pub reach: f64, pub leaf_reach: f64, pub drawn: f64, pub sweep: f64,
    /// The automatic sweep, and the reach gain for wrapping leaves.
    pub base_sweep: f64, pub reach_gain: f64,
}
pub(crate) fn main_volute(page: &Page, s: &GrowthSettings) -> MainVolute {
    let guide = page.guide(); let guide_length = line_length(&guide);
    let free = s.free == Some(true);
    let mirror = if s.flip == Some(true) { -1.0 } else { 1.0 };
    let (tp, ta) = line_frame(&guide, 1.0);
    let preferred = (if s.side == Side::Right { 1.0 } else { -1.0 }) * mirror;
    // Wrapping leaves need open space inside the curl: a larger, looser volute.
    let wrapped_curl = s.wraps.unwrap_or(0) > 0;
    let (reach_gain, curl) = if wrapped_curl { (1.55, 0.86) } else { (1.0, 0.95) };
    // a hand-set volute: sized and rolled against the automatic one
    let size = s.volute_size.filter(|v| v.is_finite()).map_or(1.0, |v| v.clamp(VOLUTE_SIZE.0, VOLUTE_SIZE.1));
    let turns = s.volute_turns.filter(|v| v.is_finite()).map_or(1.0, |v| v.clamp(VOLUTE_TURNS.0, VOLUTE_TURNS.1));
    let mut main_reach = (guide_length * (1.0 - GOLDEN_SMALL) * GOLDEN_SMALL).min(36.0) * reach_gain;
    let mut v = MainVolute { points: vec![], side: preferred, reach: main_reach, leaf_reach: main_reach, drawn: main_reach * size, sweep: curl_sweep(curl * turns), base_sweep: curl_sweep(curl), reach_gain };
    for side in [preferred, -preferred] {
        if !v.points.is_empty() { break; }
        let mut scale = 1.0;
        while scale >= 0.15 {
            let p = grow_curl(tp, ta, main_reach * size * scale, curl * turns, side);
            if page.fits(&p, 2.0, free) { v.points = p; v.side = side; v.drawn = main_reach * size * scale; main_reach *= scale; break; }
            scale -= 0.1;
        }
    }
    // the leaf narrows with a smaller volute but doesn't widen with a larger one
    v.reach = main_reach; v.leaf_reach = main_reach * size.min(1.0);
    v
}

pub fn spiral_anatomy(page: &Page, s: &GrowthSettings) -> GrowthResult {
    let mut rand = Mulberry(s.seed);
    let guide = page.guide(); let guide_length = line_length(&guide);
    let free = s.free == Some(true);
    let in_page = |pts: &[Point]| page.fits(pts, 2.0, free);
    let mirror = if s.flip == Some(true) { -1.0 } else { 1.0 };
    let MainVolute { points: ending, side: terminal_side, reach: main_reach, leaf_reach, reach_gain, .. } = main_volute(page, s);
    let mut spine = guide.clone(); if ending.len() > 1 { spine.extend_from_slice(&ending[1..]); }
    let mut parts = vec![band(spine, (guide_length * 0.045).min(7.3), Kind::Primary, "spiral", None, 0.0)];
    let count = if s.levels == 2 { 3 } else { 1 }; let secondary_scale = s.secondary_scale.unwrap_or(1.0);
    for i in 0..count {
        let progress = [(1.0 - GOLDEN_SMALL) * GOLDEN_SMALL, 1.0 - GOLDEN_SMALL, GOLDEN_SMALL][i];
        let (fp, fa) = line_frame(&guide, progress); let side = (if i == 1 { -1.0 } else { 1.0 }) * mirror;
        let reach = main_reach / reach_gain * [GOLDEN_SMALL.powi(2), GOLDEN_SMALL.powi(3), GOLDEN_SMALL][i] * (0.97 + rand.next() * 0.06) * secondary_scale;
        let mut shoot = grow_curl(fp, fa, reach, 0.66, side); let mut used = (reach, side);
        let mut k = 0;
        while k < 8 && !in_page(&shoot) { used = (reach * (0.8 - k as f64 * 0.08), -side); shoot = grow_curl(fp, fa, used.0, 0.66, used.1); k += 1; }
        if !in_page(&shoot) { continue; }
        let mut part = band(shoot, (if i == 2 { 5.6 } else { 4.5 }) * secondary_scale, Kind::Secondary, &format!("shoot-{i}"), Some("spiral"), 0.20 + i as f64 * 0.11);
        part.shoot = Some(ShootParams { progress, reach: used.0 / guide_length, turn: 0.0, curl: 0.66, side: used.1, ..ShootParams::default() });
        parts.push(part);
    }
    if s.sweeps == Some(2) {
        let (fp, fa) = line_frame(&guide, 0.35);
        let r = (guide_length * 0.1).min(16.0) * secondary_scale;
        let p = grow_curl(fp, fa, r, 0.85, -mirror);
        if in_page(&p) { let mut part = band(p, 4.5, Kind::Primary, "companion", Some("spiral"), 0.18); part.shoot = Some(ShootParams { progress: 0.35, reach: r / guide_length, turn: 0.0, curl: 0.85, side: -mirror, ..ShootParams::default() }); parts.push(part); }
    }
    // Contours: the main keeps its lobes turning into the eye; shoots carry
    // their leaf on the convex side of the curl.
    let n = parts.len();
    for idx in 0..n {
        let is_main = idx == 0;
        let (points, width, length) = (parts[idx].points.clone(), parts[idx].width, parts[idx].length);
        let turn = wrap(line_frame(&points, 0.55).1 - line_frame(&points, 0.15).1);
        let sign = if turn > 0.0 { 1.0 } else if turn < 0.0 { -1.0 } else { 0.0 };
        let side = if is_main { terminal_side } else { -(if sign != 0.0 { sign } else { terminal_side }) };
        let leaf_width = if is_main { leaf_reach * GOLDEN_SMALL / reach_gain } else { length * (1.0 - GOLDEN_SMALL) * GOLDEN_SMALL };
        // A backbone growing from another stem starts narrow and flares out
        // of it like a leaf root, instead of starting blunt.
        let attached = is_main && s.attach.is_some();
        // With a collar the root sits under the collar leaves, so it flares to
        // fill the mouth between them. Either way the stem keeps real width
        // past the flare (a start of 0.2 pinched it into a thin neck).
        let collar = attached.then_some(s.collar).flatten().filter(|c| c.is_finite() && *c > 0.0);
        let root_width = match (attached, collar) { (true, Some(c)) => 2.6 * c.clamp(0.6, 1.6), (true, None) => 2f64.min(width * 0.45) * 0.9, _ if is_main => 0.0, _ => root_flare(&parts[0].polygon, points[0], leaf_width) };
        let attached_start = if collar.is_some() { MAIN_ENDS.start } else { 0.9 };
        let o = ContourOptions { start: if is_main { guide_length / length * GOLDEN_SMALL } else { 0.0 }, belly: if is_main { GOLDEN_SMALL } else { 0.0 }, lobed: s.leaves > 0, root_width, stalk: if is_main { 0.0 } else { shoot_stalk() }, ends: if attached { Some(Ends { start: attached_start, tip: MAIN_ENDS.tip }) } else if is_main { Some(MAIN_ENDS) } else { None }, eyes: s.eyes.unwrap_or(0), ..ContourOptions::default() };
        let a = acanthus_contour(&points, 2f64.min(width * 0.45), side, leaf_width, &o);
        let p = &mut parts[idx];
        p.polygon = a.polygon; p.folds = a.folds; p.ridges = Some(a.ridges); p.cuts = a.cuts; p.contour_split = Some(241);
        if let Some(sh) = p.shoot.as_mut() { sh.stem = Some(2f64.min(width * 0.45)); sh.leaf_side = Some(side); }
    }
    let accents = parts.iter().filter(|p| p.kind == Kind::Secondary).count();
    GrowthResult { parts, skipped: 0, attempts: 0, message: format!("Curve-grown backbone · {accents} accent shoots") }
}
