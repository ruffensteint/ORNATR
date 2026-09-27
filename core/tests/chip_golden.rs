//! Chip generator parity with the original web generator. Cases are produced
//! by the web code (`chips.txt`); every region, validity check, handle move
//! and a sample of SVG exports must match.
use scroll_core::chip::{chip_regions, chip_svg, move_chip_handle, valid_chip, ChipFamily, ChipSettings};
use scroll_core::geometry::{pt, Point};

fn opt<T: std::str::FromStr>(s: &str) -> Option<T> { if s == "-" { None } else { s.parse().ok() } }
fn points(v: &[&str]) -> Vec<Point> { v.chunks(2).map(|c| pt(c[0].parse().unwrap(), c[1].parse().unwrap())).collect() }
fn same(a: &[Point], b: &[Point], tol: f64) -> bool { a.len() == b.len() && a.iter().zip(b).all(|(p, q)| (p.x - q.x).abs() <= tol && (p.y - q.y).abs() <= tol) }

#[test]
fn chips_match_web_generator() {
    let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/golden/chips.txt")).unwrap();
    let mut settings: Option<ChipSettings> = None;
    let mut regions: Vec<Vec<Point>> = vec![];
    let mut next = 0usize;
    let mut last: Option<Vec<Point>> = None;
    let (mut cases, mut checked, mut failures) = (0, 0, Vec::<String>::new());
    let mut name = String::new();
    for line in text.lines() {
        let (tag, rest) = line.split_once(' ').unwrap_or((line, ""));
        let f: Vec<&str> = rest.split(' ').collect();
        match tag {
            "case" => {
                if let Some(st) = settings.as_ref() { let n = chip_regions(st).len(); if n != next { failures.push(format!("case {name}: {n} regions, expected {next}")); } }
                name = rest.into(); cases += 1; settings = None;
            }
            "settings" => {
                let s = ChipSettings { family: ChipFamily::from_key(f[0]).unwrap(), count: f[1].parse().unwrap(), size: f[2].parse().unwrap(), removed: if f[9] == "-" { vec![] } else { f[9].split(',').map(|n| n.parse().unwrap()).collect() },
                    seed: opt(f[3]), grid: opt(f[4]), edits: Default::default(), grammar: opt(f[5]), border_seed: opt(f[6]), border_version: opt(f[7]), traditional: opt(f[8]), faceted: None };
                settings = Some(s);
            }
            "edit" => { let s = settings.as_mut().unwrap(); s.edits.insert(f[0].parse().unwrap(), points(&f[1..])); }
            "region" => {
                if regions.is_empty() && next == 0 { regions = chip_regions(settings.as_ref().unwrap()); }
                let want = points(&f[1..]);
                let got = regions.get(next).cloned().unwrap_or_default();
                if !same(&got, &want, 2e-6) { failures.push(format!("case {name}: region {next} differs ({} vs {} points)", got.len(), want.len())); }
                if (f[0] == "1") != valid_chip(&got) { failures.push(format!("case {name}: region {next} validity")); }
                last = Some(got); next += 1; checked += 1;
            }
            "moved" => {
                let idx: usize = f[0].parse().unwrap();
                let r = last.clone().unwrap();
                let m = move_chip_handle(&r, idx, pt(r[idx].x + 1.5, r[idx].y - 2.0));
                if !same(&m, &points(&f[2..]), 2e-6) { failures.push(format!("case {name}: handle move differs")); }
                if (f[1] == "1") != valid_chip(&m) { failures.push(format!("case {name}: moved validity")); }
            }
            "svg" => { if chip_svg(settings.as_ref().unwrap()) != rest { failures.push(format!("case {name}: svg differs")); } }
            _ => {}
        }
        if tag == "case" { regions.clear(); next = 0; }
    }
    if let Some(st) = settings.as_ref() { let n = chip_regions(st).len(); if n != next { failures.push(format!("case {name}: {n} regions, expected {next}")); } }
    assert!(cases > 400, "only {cases} cases");
    assert!(failures.is_empty(), "{} mismatches over {checked} regions, first: {:?}", failures.len(), &failures[..failures.len().min(8)]);
}
