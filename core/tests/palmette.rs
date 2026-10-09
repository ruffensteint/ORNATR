//! The Palmette workspace's model (`palmette.rs`): the library rebuilds the
//! banked study picks at their sizes, copies follow the repeat rules, and
//! fitting keeps a design on its page.
use scroll_core::geometry::pt;
use scroll_core::palmette::{copies, structure, Design, Repeat, ADD, STRUCTURES};

/// Each library structure's size in its study coordinates, as the banked pick sheets give it (mm).
const PICKS: [(&str, f64, f64); 8] = [
    ("classical", 101.0, 119.0), ("flame", 110.0, 122.0), ("shell", 113.0, 99.0), ("anthemion", 307.0, 81.0),
    ("fleur", 142.0, 134.0), ("rich-fleur", 88.0, 102.0), ("fleurettes", 204.0, 96.0), ("husk-drop", 44.0, 143.0),
];

#[test]
fn the_library_rebuilds_the_banked_picks_at_their_sizes() {
    assert_eq!(STRUCTURES.len(), PICKS.len());
    for (id, w, h) in PICKS {
        let b = structure(id).unwrap().bounds().unwrap();
        assert!(((b.r - b.l) - w).abs() < 1.0 && ((b.b - b.t) - h).abs() < 1.0, "{id}: {:.1} x {:.1}, the pick is {w} x {h}", b.r - b.l, b.b - b.t);
    }
}

#[test]
fn rows_mirror_and_rings_place_the_copies() {
    let c = pt(150.0, 110.0);
    // a row of 3, 80 apart, mirrored about the upright through the centre
    let xs = copies(Repeat { mirror_x: true, ..Repeat::row(3, 80.0) }, c);
    assert_eq!(xs.len(), 6);
    let p = pt(69.5, 120.0);
    let got: Vec<f64> = xs.iter().map(|x| x.point(p).x).collect();
    let want = [69.5, 149.5, 229.5, 230.5, 150.5, 70.5];
    for (g, w) in got.iter().zip(want) { assert!((g - w).abs() < 1e-9, "{got:?}"); }
    // the plain copies come first, the mirrored after (so mirrored halves lie on top)
    assert!(xs[..3].iter().all(|x| !x.reflects()) && xs[3..].iter().all(|x| x.reflects()));
    // a ring of 4 turns about the centre; back() undoes every copy
    for x in copies(Repeat { ring: 4, mirror_x: true, ..Repeat::ONE }, c) {
        let q = x.point(p);
        assert!((distance(q, c) - distance(p, c)).abs() < 1e-9);
        let b = x.back(q);
        assert!((b.x - p.x).abs() < 1e-9 && (b.y - p.y).abs() < 1e-9);
        assert!((x.back_angle(x.angle(0.7)) - 0.7).abs() < 1e-9);
    }
}
fn distance(a: scroll_core::geometry::Point, b: scroll_core::geometry::Point) -> f64 { (a.x - b.x).hypot(a.y - b.y) }

#[test]
fn designs_fit_their_page_and_every_motif_draws() {
    for (id, _, _) in STRUCTURES {
        for (w, h) in [(160.0, 200.0), (300.0, 90.0)] {
            let d = Design::from_structure(id, w, h).unwrap();
            let b = d.bounds().unwrap();
            assert!(b.l >= -1e-6 && b.t >= -1e-6 && b.r <= w + 1e-6 && b.b <= h + 1e-6, "{id} on {w} x {h}: {b:?}");
            let dr = d.drawing();
            assert!(!dr.outlines.is_empty() && !dr.paint.is_empty());
        }
    }
    let mut d = Design::empty(200.0, 200.0);
    for (what, _, _) in ADD {
        let id = d.add(what).unwrap();
        assert!(d.element_layers(d.get(id).unwrap()).iter().all(|l| !l.is_empty()), "{what}");
    }
    assert!(d.add("nothing").is_none());
    // scaling a band scales its spacing too (to within the spine sampling: petals are drawn every 0.3 mm)
    let mut band = structure("anthemion").unwrap();
    let before = band.bounds().unwrap();
    band.map(0.5, pt(0.0, 0.0));
    let after = band.bounds().unwrap();
    assert!(((after.r - after.l) - (before.r - before.l) * 0.5).abs() < 0.1, "{} vs {}", after.r - after.l, (before.r - before.l) * 0.5);
}
