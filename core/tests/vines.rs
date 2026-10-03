//! Scroll vines: a backbone set to grow as a vine grows curls that stay inside
//! the carving surface and keep clear of each other; layouts without vines
//! grow exactly as before; a draft is just the stem.
use scroll_core::booleans::{area, difference, intersect, union};
use scroll_core::geometry::{distance, pt};
use scroll_core::model::Layout;

fn vine_layout(surface: Option<&str>, leaves: u8) -> Layout {
    let mut l = Layout::starter();
    l.width = 300.0; l.height = 130.0;
    l.curves = vec![[pt(28.0, 100.0), pt(90.0, 20.0), pt(190.0, 150.0), pt(258.0, 30.0)]];
    l.growth[0].vine = Some(55.0); l.growth[0].leaves = leaves;
    l.surface = surface.map(|s| s.to_string());
    l
}

#[test]
fn vines_stay_inside_the_surface_and_apart() {
    for surface in [None, Some("plaque"), Some("oval")] {
        for leaves in [0u8, 2] {
            let l = vine_layout(surface, leaves);
            let g = l.grow();
            assert!(g.parts.len() > 4, "{surface:?}/{leaves}: only {} parts", g.parts.len());
            let outline = l.surface_polygon().unwrap_or_else(|| vec![pt(0.0, 0.0), pt(300.0, 0.0), pt(300.0, 130.0), pt(0.0, 130.0)]);
            let surf = union(&[outline.as_slice()]);
            for p in &g.parts {
                let out = area(&difference(&union(&[p.polygon.as_slice()]), &surf));
                assert!(out < 1.0, "{surface:?}/{leaves}: {} is {out:.1} mm2 outside the surface", p.id);
            }
            // curls never overlap each other, apart from a branch at its parent
            let scrolls: Vec<_> = g.parts.iter().filter(|p| p.parent.is_some()).collect();
            for (i, a) in scrolls.iter().enumerate() { for b in scrolls.iter().skip(i + 1) {
                if a.parent.as_deref() == Some(b.id.as_str()) || b.parent.as_deref() == Some(a.id.as_str()) { continue; }
                if distance(a.points[0], b.points[0]) < 1.0 { continue; }
                let o = area(&intersect(&union(&[a.polygon.as_slice()]), &union(&[b.polygon.as_slice()])));
                assert!(o < 2.0, "{surface:?}/{leaves}: {} and {} overlap by {o:.1} mm2", a.id, b.id);
            }}
        }
    }
}

#[test]
fn a_draft_vine_is_just_its_stem_and_old_layouts_have_none() {
    let l = vine_layout(Some("plaque"), 2);
    assert_eq!(l.grow_draft().parts.len(), 1);
    assert!(Layout::starter().growth.iter().all(|g| g.vine.is_none()) && Layout::starter().surface.is_none());
}

#[test]
fn a_vine_keeps_clear_of_other_backbones() {
    let mut l = vine_layout(None, 2);
    // an ordinary scroll across the middle of the page, grown first
    l.curves.insert(0, [pt(60.0, 110.0), pt(120.0, 60.0), pt(170.0, 90.0), pt(220.0, 40.0)]);
    let g0 = l.growth[0].clone();
    l.growth.insert(0, scroll_core::growth::GrowthSettings { vine: None, ..g0 });
    let g = l.grow();
    let other: Vec<_> = g.parts.iter().filter(|p| p.id.starts_with("backbone-0/")).collect();
    let curls: Vec<_> = g.parts.iter().filter(|p| p.id.starts_with("backbone-1/scroll-")).collect();
    assert!(!other.is_empty() && !curls.is_empty());
    for c in &curls { for o in &other {
        let overlap = area(&intersect(&union(&[c.polygon.as_slice()]), &union(&[o.polygon.as_slice()])));
        assert!(overlap < 2.0, "{} overlaps {} by {overlap:.1} mm2", c.id, o.id);
    }}
}

#[test]
fn edited_curls_rebuild_exactly_and_edit_alone() {
    use scroll_core::shoots::ShootEdit;
    let mut l = vine_layout(None, 2);
    let g = l.grow();
    let curls: Vec<_> = g.parts.iter().filter(|p| p.shoot.is_some()).collect();
    assert!(curls.len() >= 3);
    // freeze: every curl becomes an edit with its own id
    l.shoots = curls.iter().map(|p| ShootEdit { params: p.shoot.clone().unwrap(), id: p.id.clone(), backbone: 0, replaces: None, hidden: false, under: false }).collect();
    let frozen = l.grow();
    for c in &curls {
        let f = frozen.parts.iter().find(|p| p.id == c.id).expect("curl kept");
        let d = distance(*c.points.last().unwrap(), *f.points.last().unwrap());
        assert!(d < 0.6 && (c.points.len() as i64 - f.points.len() as i64).abs() <= 2, "{} moved {d:.2} mm when frozen", c.id);
    }
    // turn the first curl: it moves, the others (not growing from it) do not
    let first = l.shoots[0].id.clone();
    l.shoots[0].params.turn += 0.3;
    let edited = l.grow();
    let tip = |g: &scroll_core::growth::GrowthResult, id: &str| *g.parts.iter().find(|p| p.id == id).unwrap().points.last().unwrap();
    assert!(distance(tip(&frozen, &first), tip(&edited, &first)) > 1.0);
    for e in l.shoots.iter().skip(1).filter(|e| e.params.on.as_deref() != Some(first.as_str())) {
        assert!(distance(tip(&frozen, &e.id), tip(&edited, &e.id)) < 0.01, "{} moved although only {first} was edited", e.id);
    }
}

#[test]
fn mirroring_a_curl_turns_it_the_other_way() {
    // the app's Mirror flips both side and turn; the vine cache once missed
    // that change (two sign flips cancelled in its key) and showed the old curl
    use scroll_core::shoots::ShootEdit;
    let mut l = vine_layout(None, 2);
    let g = l.grow();
    l.shoots = g.parts.iter().filter(|p| p.shoot.is_some()).map(|p| ShootEdit { params: p.shoot.clone().unwrap(), id: p.id.clone(), backbone: 0, replaces: None, hidden: false, under: false }).collect();
    let before = l.grow();
    let id = l.shoots[0].id.clone();
    l.shoots[0].params.side = -l.shoots[0].params.side;
    l.shoots[0].params.turn = -l.shoots[0].params.turn;
    let after = l.grow();
    let tip = |g: &scroll_core::growth::GrowthResult| *g.parts.iter().find(|p| p.id == id).unwrap().points.last().unwrap();
    assert!(distance(tip(&before), tip(&after)) > 5.0, "mirrored curl did not change");
}
