//! The cartouche model (`cartouche.rs`): symmetry copies, the library
//! structures, the grown acanthus, editing and compositing.
use scroll_core::cartouche::{compose, copies, draft_layer, part_owner, structure, Design, Element, Kind, Layer, Owner, Repeat, STRUCTURES, START};
use scroll_core::geometry::pt;
use scroll_core::model::GrowCache;

fn close(a: f64, b: f64) -> bool { (a - b).abs() < 1e-9 }

#[test]
fn copies_come_in_the_studies_order_and_undo_exactly() {
    let c = pt(150.0, 160.0);
    let four = copies(Repeat::FOUR, c);
    assert_eq!(four.len(), 4);
    let p = pt(170.0, 100.0);
    // the element, mirrored left-right, both ways, top-bottom (as the study's `quarter`)
    let want = [pt(170.0, 100.0), pt(130.0, 100.0), pt(130.0, 220.0), pt(170.0, 220.0)];
    for (x, w) in four.iter().zip(want) { let q = x.point(p); assert!(close(q.x, w.x) && close(q.y, w.y), "{q:?} != {w:?}"); }
    assert_eq!(four.iter().map(|x| x.reflects()).collect::<Vec<_>>(), vec![false, true, false, true]);
    let ring = copies(Repeat { mirror_x: true, mirror_y: false, ring: 8 }, c);
    assert_eq!(ring.len(), 16);
    for x in four.iter().chain(ring.iter()) {
        let q = x.back(x.point(p)); assert!((q.x - p.x).abs() < 1e-9 && (q.y - p.y).abs() < 1e-9);
        let a = 0.7; assert!((x.back_angle(x.angle(a)) - a).abs() < 1e-9);
    }
}

#[test]
fn every_structure_builds_with_the_studies_scrolls() {
    // backbones per structure, as the study listed its stems
    let want = [("shield", 10), ("shield-jewels", 12), ("oval", 10), ("panel", 18), ("jewels", 8), ("four-point", 12), ("crest-base", 6), ("wreath", 16), ("rectangle", 16), ("shield-clusters", 8)];
    for (id, n) in want {
        let d = structure(id).unwrap();
        let a = d.acanthus().unwrap();
        assert_eq!(a.owners.len(), n, "{id}");
        assert!(d.frame().is_some(), "{id} has a frame");
    }
    assert_eq!(STRUCTURES.len(), want.len());
    // B4's leaves: two fans on each side
    assert_eq!(structure("shield").unwrap().acanthus().unwrap().leaves.len(), 4);
}

#[test]
fn mirrored_scrolls_are_mirror_images_with_their_growth_flipped() {
    let d = structure("shield").unwrap();
    let a = d.acanthus().unwrap();
    let (c0, c1) = (a.layout.curves[0], a.layout.curves[1]);
    for k in 0..4 { assert!((c1[k].x - (2.0 * d.centre.x - c0[k].x)).abs() < 1e-9 && (c1[k].y - c0[k].y).abs() < 1e-9); }
    assert_ne!(a.layout.growth[0].flip, a.layout.growth[1].flip);
    assert_eq!(a.layout.growth[0].side, a.layout.growth[1].side);
    // the child scrolls grow from the matching copy of their parent
    assert_eq!(a.layout.growth[4].attach, Some(0));
    assert_eq!(a.layout.growth[5].attach, Some(1));
    // seeds follow the study: base + index · 7919
    for (i, g) in a.layout.growth.iter().enumerate() { assert_eq!(g.seed, 83u32.wrapping_add(i as u32 * 7919)); }
}

#[test]
fn a_fitted_design_sits_inside_its_page() {
    let d = Design::from_structure(START.0, START.1, START.2).unwrap();
    let b = d.bounds().unwrap();
    let m = d.width.min(d.height) * 0.06;
    assert!(b.l >= m - 1.0 && b.t >= m - 1.0 && b.r <= d.width - m + 1.0 && b.b <= d.height - m + 1.0, "{b:?}");
    assert!((b.r - b.l - (d.width - 2.0 * m)).abs() < 2.0 || (b.b - b.t - (d.height - 2.0 * m)).abs() < 2.0, "fills the page one way: {b:?}");
}

#[test]
fn removing_a_scroll_takes_its_leaves_and_frees_its_children() {
    let mut d = structure("shield").unwrap();
    let corner = d.elements.iter().find(|e| e.is_scroll()).unwrap().id;
    let leaves = d.elements.iter().filter(|e| matches!(e.kind, Kind::Leaf { stem, .. } if stem == corner)).count();
    assert!(leaves > 0);
    assert_eq!(d.remove(corner), 1 + leaves);
    assert!(d.elements.iter().all(|e| !matches!(e.kind, Kind::Scroll { attach: Some(p), .. } if p == corner)));
    assert!(d.acanthus().is_some());
}

#[test]
fn the_acanthus_restacks_as_one_body() {
    let mut d = structure("shield").unwrap();
    let scroll = d.elements.iter().find(|e| e.is_scroll()).unwrap().id;
    d.restack(scroll, true);
    let n = d.elements.iter().filter(|e| e.is_acanthus()).count();
    assert!(d.elements[d.elements.len() - n..].iter().all(Element::is_acanthus));
    d.restack(scroll, false);
    assert!(d.elements[..n].iter().all(Element::is_acanthus));
}

#[test]
fn attached_scrolls_settle_onto_their_parent() {
    let mut d = structure("shield").unwrap();
    let child = d.elements.iter().find(|e| matches!(e.kind, Kind::Scroll { attach: Some(_), .. })).unwrap().id;
    if let Some(Element { kind: Kind::Scroll { curve, .. }, .. }) = d.get_mut(child) { for c in curve.iter_mut() { c.x += 9.0; c.y -= 4.0; } }
    assert!(d.settle());
    assert!(!d.settle(), "settled in one go");
}

#[test]
fn lines_behind_a_cover_are_hidden_and_holes_show_through() {
    let square = |l: f64, t: f64, s: f64| vec![pt(l, t), pt(l + s, t), pt(l + s, t + s), pt(l, t + s)];
    let back = Layer { lines: vec![vec![pt(0.0, 5.0), pt(30.0, 5.0)]], ..Default::default() };
    // a 20 mm square from 5 to 25 with a hole from 12 to 18
    let front = Layer { cover: vec![vec![square(5.0, -5.0, 20.0), square(12.0, 2.0, 6.0)]], ..Default::default() };
    let d = compose(&[(Owner::Acanthus, back), (Owner::Element(1, 0), front)]);
    let mut xs: Vec<(f64, f64)> = d.lines.iter().map(|r| (r[0].x, r[r.len() - 1].x)).collect();
    xs.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    assert_eq!(xs.len(), 3, "{xs:?}");
    assert!((xs[0].1 - 5.0).abs() < 1e-6 && (xs[1].0 - 12.0).abs() < 1e-6 && (xs[1].1 - 18.0).abs() < 1e-6 && (xs[2].0 - 25.0).abs() < 1e-6);
}

#[test]
fn cached_growth_matches_and_reuses_unchanged_scrolls() {
    let d = structure("oval").unwrap();
    let a = d.acanthus().unwrap();
    let mut cache = GrowCache::default();
    let (g, c) = (a.layout.grow(), a.layout.grow_cached(&mut cache));
    assert_eq!(g.parts.len(), c.parts.len());
    for (p, q) in g.parts.iter().zip(&c.parts) { assert_eq!(p.id, q.id); assert_eq!(p.polygon, q.polygon); }
    // every part belongs to a scroll (or a leaf on one)
    for p in &g.parts { assert!(part_owner(&a, &p.id).is_some(), "{}", p.id); }
    let t = std::time::Instant::now(); let again = a.layout.grow_cached(&mut cache); let quick = t.elapsed();
    assert_eq!(again.parts.len(), g.parts.len());
    assert!(quick.as_millis() < 60, "cached growth took {quick:?}");
    assert!(!draft_layer(&g).cover.is_empty());
}

#[test]
fn the_svg_is_at_actual_size() {
    let d = Design::from_structure("jewels", 160.0, 220.0).unwrap();
    let s = d.svg();
    assert!(s.contains("width=\"160mm\"") && s.contains("height=\"220mm\"") && s.contains("<path d=\"M"));
}
