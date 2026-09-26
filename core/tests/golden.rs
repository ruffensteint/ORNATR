//! Golden comparison against the web version: every case layout must grow
//! the same parts (ids, parents, outlines, creases, ridges) and the same
//! number of drawn runs.
use scroll_core::geometry::{pt, Point};
use scroll_core::growth::{Family, GrowthSettings, Side};
use scroll_core::layers::layered_drawing;
use scroll_core::model::Layout;
use scroll_core::shoots::{ShootEdit, ShootParams};

fn opt<T: std::str::FromStr>(s: &str) -> Option<T> { if s == "-" { None } else { s.parse().ok() } }
fn nums(s: &str) -> Vec<f64> { s.split_whitespace().map(|v| v.parse().unwrap()).collect() }
fn points(s: &str) -> Vec<Point> { nums(s).chunks(2).map(|c| pt(c[0], c[1])).collect() }

struct Part { id: String, parent: Option<String>, poly: Vec<Point>, folds: Vec<Vec<Point>>, ridges: Vec<Vec<Point>> }
struct Case { name: String, layout: Layout, parts: Vec<Part>, drawing: (usize, usize) }

fn load() -> Vec<Case> {
    let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/golden/cases.txt")).unwrap();
    let mut cases = vec![]; let mut cur: Option<Case> = None;
    for line in text.lines() {
        let (tag, rest) = line.split_once(' ').unwrap_or((line, ""));
        match tag {
            "case" => cur = Some(Case { name: rest.into(), layout: Layout { curves: vec![], growth: vec![], ..Layout::starter() }, parts: vec![], drawing: (0, 0) }),
            "layout" => { let v = nums(rest); let c = cur.as_mut().unwrap(); c.layout.width = v[0]; c.layout.height = v[1]; }
            "curve" => { let p = points(rest); cur.as_mut().unwrap().layout.curves.push([p[0], p[1], p[2], p[3]]); }
            "growth" => {
                let f: Vec<&str> = rest.split_whitespace().collect();
                let g = GrowthSettings { seed: f[0].parse().unwrap(), levels: f[1].parse().unwrap(), leaves: f[2].parse().unwrap(),
                    side: match f[3] { "left" => Side::Left, "right" => Side::Right, _ => Side::Alternate },
                    family: match f[4] { "spiral" => Some(Family::Spiral), "spray" => Some(Family::Spray), "border" => Some(Family::Border), "fan" => Some(Family::Fan), "branching" => Some(Family::Branching), _ => None },
                    composition: opt(f[5]), secondary_scale: opt(f[6]), sweeps: opt(f[7]), auto_shoots: opt(f[8]), flip: opt(f[9]), free: f.get(10).and_then(|v| opt(*v)), ..GrowthSettings::default() };
                cur.as_mut().unwrap().layout.growth.push(g);
            }
            "shoot" => {
                let f: Vec<&str> = rest.split_whitespace().collect();
                let p = ShootParams { progress: f[2].parse().unwrap(), reach: f[3].parse().unwrap(), turn: f[4].parse().unwrap(), curl: f[5].parse().unwrap(), side: f[6].parse().unwrap(), leaf_side: opt(f[7]), stem: opt(f[8]), leaf_scale: opt(f[9]), lobes: opt(f[10]), depth: opt(f[11]), stalk: opt(f[12]), taper: opt(f[13]), bend: opt(f[14]), preset: opt(f[15]), follow: None, fan: None, on: None };
                cur.as_mut().unwrap().layout.shoots.push(ShootEdit { params: p, id: f[0].into(), backbone: f[1].parse().unwrap(), replaces: opt(f[16]), hidden: f[17] == "true", under: f[18] == "true" });
            }
            "part" => { let f: Vec<&str> = rest.split_whitespace().collect(); cur.as_mut().unwrap().parts.push(Part { id: f[0].into(), parent: opt(f[1]), poly: vec![], folds: vec![], ridges: vec![] }); }
            "poly" => cur.as_mut().unwrap().parts.last_mut().unwrap().poly = points(rest),
            "fold" => cur.as_mut().unwrap().parts.last_mut().unwrap().folds.push(points(rest)),
            "ridge" => cur.as_mut().unwrap().parts.last_mut().unwrap().ridges.push(points(rest)),
            "drawing" => { let v = nums(rest); cur.as_mut().unwrap().drawing = (v[0] as usize, v[1] as usize); }
            "end" => cases.push(cur.take().unwrap()),
            _ => {}
        }
    }
    cases
}

fn worst(a: &[Point], b: &[Point]) -> f64 { if a.len() != b.len() { return f64::INFINITY; } a.iter().zip(b).map(|(p, q)| (p.x - q.x).hypot(p.y - q.y)).fold(0.0, f64::max) }

#[test]
fn matches_web_version() {
    let mut failures = vec![];
    for c in load() {
        let r = c.layout.grow();
        let ids: Vec<&str> = r.parts.iter().map(|p| p.id.as_str()).collect();
        let want: Vec<&str> = c.parts.iter().map(|p| p.id.as_str()).collect();
        if ids != want { failures.push(format!("{}: parts {:?} != {:?}", c.name, ids, want)); continue; }
        for (got, exp) in r.parts.iter().zip(&c.parts) {
            if got.parent != exp.parent { failures.push(format!("{} {}: parent", c.name, exp.id)); }
            let w = worst(&got.polygon, &exp.poly);
            if w > 1e-6 { failures.push(format!("{} {}: outline off by {w:e} ({} vs {} pts)", c.name, exp.id, got.polygon.len(), exp.poly.len())); }
            if got.folds.len() != exp.folds.len() { failures.push(format!("{} {}: {} folds vs {}", c.name, exp.id, got.folds.len(), exp.folds.len())); }
            else { for (a, b) in got.folds.iter().zip(&exp.folds) { let w = worst(a, b); if w > 1e-6 { failures.push(format!("{} {}: fold off by {w:e}", c.name, exp.id)); } } }
            let gr = got.ridges.clone().unwrap_or_default();
            if gr.len() != exp.ridges.len() { failures.push(format!("{} {}: {} ridges vs {}", c.name, exp.id, gr.len(), exp.ridges.len())); }
            else { for (a, b) in gr.iter().zip(&exp.ridges) { let w = worst(a, b); if w > 1e-6 { failures.push(format!("{} {}: ridge off by {w:e}", c.name, exp.id)); } } }
        }
        let d = layered_drawing(&r);
        if (d.outline.len(), d.folds.len()) != c.drawing { failures.push(format!("{}: drawing runs {:?} vs {:?}", c.name, (d.outline.len(), d.folds.len()), c.drawing)); }
    }
    assert!(failures.is_empty(), "{} mismatches:\n{}", failures.len(), failures.join("\n"));
}
