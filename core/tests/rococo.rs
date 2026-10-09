//! The Rococo workspace's model: library structures, editing, mirroring, fitting.
use scroll_core::geometry::pt;
use scroll_core::rocaille::Turnover;
use scroll_core::rococo::*;

#[test]
fn every_structure_builds_and_draws() {
    for (id, _, _) in STRUCTURES {
        let d = structure(id).unwrap();
        let parts = d.parts();
        // every element draws (nothing lost to a missing rim)
        assert_eq!(parts.len(), d.elements.len(), "{id}");
        assert!(parts.iter().all(|(_, p)| !p.is_empty()), "{id}");
        assert!(!d.drawing().outlines.is_empty(), "{id}");
    }
}

#[test]
fn library_sizes_match_the_study_sheet() {
    // sizes printed by the rocaille study's reference library (the bracket there includes the wall and shelf)
    for (id, w, h) in [("cartouche", 196.0, 204.0), ("agrafe", 302.0, 116.0), ("corner", 179.0, 178.0), ("medallion", 189.0, 271.0), ("frieze", 474.0, 109.0), ("wave", 196.0, 273.0)] {
        let b = structure(id).unwrap().bounds().unwrap();
        // the rims are refitted as Bézier points within FIT_TOLERANCE of the study curves
        assert!(((b.r - b.l) - w).abs() < FIT_TOLERANCE + 0.5 && ((b.b - b.t) - h).abs() < FIT_TOLERANCE + 0.5, "{id}: {:.1} x {:.1}", b.r - b.l, b.b - b.t);
    }
}

#[test]
fn ornament_follows_its_rim() {
    let mut d = structure("corner").unwrap();
    let rim = d.elements.iter().find(|e| e.is_rim()).unwrap().id;
    let on_rim: Vec<u32> = d.elements.iter().filter(|e| e.rim() == Some(rim)).map(|e| e.id).collect();
    assert!(!on_rim.is_empty());
    let before = d.parts();
    if let Some(Element { kind: Kind::Rim { nodes, .. }, .. }) = d.get_mut(rim) { for n in nodes.iter_mut() { for q in [&mut n.p, &mut n.a, &mut n.b] { q.y += 20.0; } } }
    let after = d.parts();
    for id in on_rim {
        let (a, b) = (before.iter().find(|x| x.0 == id).unwrap(), after.iter().find(|x| x.0 == id).unwrap());
        let (pa, pb) = (a.1[0].outline[0], b.1[0].outline[0]);
        assert!((pb.y - pa.y - 20.0).abs() < 1.0 && (pb.x - pa.x).abs() < 1.0, "element {id} moved by {:.2}, {:.2}", pb.x - pa.x, pb.y - pa.y);
    }
}

#[test]
fn mirroring_twice_is_the_same_design() {
    let d = structure("cartouche").unwrap();
    let mut m = d.clone();
    m.map(1.0, true, 0.0, pt(0.0, 0.0));
    let (b0, b1) = (d.bounds().unwrap(), m.bounds().unwrap());
    assert!((b0.l + b1.r).abs() < 1.0 && (b0.r + b1.l).abs() < 1.0);
    m.map(1.0, true, 0.0, pt(0.0, 0.0));
    let (p, q) = (d.parts(), m.parts());
    for ((_, a), (_, b)) in p.iter().zip(q.iter()) {
        let (x, y) = (a[0].outline[0], b[0].outline[0]);
        assert!((x.x - y.x).abs() < 1e-6 && (x.y - y.y).abs() < 1e-6);
    }
}

#[test]
fn fitting_keeps_the_design_on_the_page() {
    for (id, _, _) in STRUCTURES {
        for (w, h) in [(240.0, 250.0), (400.0, 120.0), (120.0, 300.0)] {
            let d = Design::from_structure(id, w, h).unwrap();
            let b = d.bounds().unwrap();
            assert!(b.l >= -0.5 && b.t >= -0.5 && b.r <= w + 0.5 && b.b <= h + 0.5, "{id} on {w} x {h}: {b:?}");
        }
    }
}

#[test]
fn removing_a_rim_takes_its_ornament() {
    let mut d = structure("cartouche").unwrap();
    let rim = d.elements.iter().find(|e| e.is_rim()).unwrap().id;
    let n = d.elements.len();
    let gone = d.remove(rim);
    assert!(gone > 1);
    assert_eq!(d.elements.len(), n - gone);
    assert!(d.elements.iter().all(|e| e.rim() != Some(rim)));
}

#[test]
fn added_elements_draw() {
    let mut d = Design::empty(300.0, 200.0);
    let r = d.add_rim(pt(150.0, 100.0), 150.0);
    for id in [d.add_frond(r, 0.5), d.add_shell(r, 0.3), d.add_rosette(r, 0.7), d.add_run(r, 0.2, 0.8), d.add_frill(r, 0.3, 0.6)] { assert!(id.is_some()); }
    d.add_pocket(pt(150.0, 140.0), 20.0);
    let parts = d.parts();
    assert_eq!(parts.len(), d.elements.len());
    assert!(parts.iter().all(|(_, p)| !p.is_empty()));
}

#[test]
fn library_rims_have_few_points_and_all_ornament_hangs_on_a_rim() {
    for (id, _, _) in STRUCTURES {
        let d = structure(id).unwrap();
        for e in &d.elements {
            match &e.kind {
                Kind::Rim { nodes, .. } => assert!((3..=6).contains(&nodes.len()), "{id}: a rim with {} points", nodes.len()),
                Kind::Frond { at, .. } | Kind::Shell { at, .. } | Kind::Rosette { at, .. } => assert!(matches!(at, Anchor::Rim { .. }), "{id}: element {} is free", e.id),
                _ => {}
            }
        }
    }
    let mut d = Design::empty(200.0, 200.0);
    let r = d.add_rim(pt(100.0, 100.0), 120.0);
    assert!(matches!(&d.get(r).unwrap().kind, Kind::Rim { nodes, .. } if nodes.len() == 3));
}

#[test]
fn adding_a_point_keeps_the_shape_and_removing_it_comes_close() {
    let d = structure("wave").unwrap();
    let Kind::Rim { nodes, .. } = &d.elements.iter().find(|e| e.is_rim()).unwrap().kind else { unreachable!() };
    let before = bezier_dense(nodes);
    let mut more = nodes.clone();
    let k = split_at(&mut more, before[before.len() / 3]).unwrap();
    assert_eq!(more.len(), nodes.len() + 1);
    let after = bezier_dense(&more);
    let far = before.iter().step_by(5).map(|p| after.iter().map(|q| (p.x - q.x).hypot(p.y - q.y)).fold(f64::INFINITY, f64::min)).fold(0.0, f64::max);
    assert!(far < 0.2, "split moved the rim by {far:.2} mm");
    assert!(remove_node(&mut more, k));
    assert_eq!(more.len(), nodes.len());
    let back = bezier_dense(&more);
    let far = before.iter().step_by(5).map(|p| back.iter().map(|q| (p.x - q.x).hypot(p.y - q.y)).fold(f64::INFINITY, f64::min)).fold(0.0, f64::max);
    assert!(far < 1.0, "removing the point moved the rim by {far:.2} mm");
}

#[test]
fn a_handle_keeps_its_partner_in_line() {
    let mut nodes = vec![Node::smooth(pt(0.0, 0.0), pt(10.0, 0.0)), Node::smooth(pt(50.0, 0.0), pt(60.0, 5.0)), Node::smooth(pt(100.0, 0.0), pt(110.0, 0.0))];
    let len_a = (nodes[1].a.x - 50.0).hypot(nodes[1].a.y);
    set_handle(&mut nodes, 1, true, pt(50.0, 20.0));
    let n = nodes[1];
    assert!((n.a.x - 50.0).abs() < 1e-9 && n.a.y < 0.0, "in-handle should point straight down: {:?}", n.a);
    assert!(((n.a.x - 50.0).hypot(n.a.y) - len_a).abs() < 1e-9);
}

#[test]
fn grown_ornament_merges_with_its_rim_and_turnovers_stay_on_top() {
    let mut d = Design::empty(260.0, 120.0);
    let r = d.add_rim(pt(130.0, 70.0), 170.0);
    let f = d.add_frond(r, 0.4).unwrap();
    d.add_frill(r, 0.25, 0.75);
    if let Some(Kind::Frond { turn, .. }) = d.get_mut(f).map(|e| &mut e.kind) { *turn = Turnover::Roll; }
    assert!(d.elements.iter().filter(|e| e.can_grow()).all(|e| e.grown), "new ornament grows from its rim");
    let parts = d.parts();
    let merged = d.layered(&parts);
    // rim + crest + frond body become one surface; the frond's flap stays a part of its own
    assert_eq!(merged.iter().filter(|p| p.seam.len() > 2).count(), 1);
    assert_eq!(merged.iter().filter(|p| p.seam.len() <= 2).count(), 1);
    for e in d.elements.iter_mut() { e.grown = false; }
    let parts = d.parts();
    assert_eq!(d.layered(&parts).len(), parts.iter().map(|(_, p)| p.len()).sum::<usize>());
}

#[test]
fn swell_turnovers_cabochon_and_trellis_draw() {
    let mut d = Design::empty(260.0, 120.0);
    let r = d.add_rim(pt(130.0, 70.0), 170.0);
    let plain = d.parts()[0].1[0].outline.clone();
    if let Some(Kind::Rim { swell, twists, .. }) = d.get_mut(r).map(|e| &mut e.kind) { *swell = 0.8; *twists = vec![0.1, 0.9]; }
    let varied = d.parts()[0].1[0].outline.clone();
    assert!(plain != varied);
    let c = d.add_cabochon(r, 0.5).unwrap();
    let t = d.add_trellis(pt(130.0, 85.0), 22.0);
    let parts = d.parts();
    assert!(parts.iter().any(|(id, p)| *id == c && p.len() == 1));
    let tp = &parts.iter().find(|(id, _)| *id == t).unwrap().1;
    assert!(tp.len() > 5 && !tp[0].lines.is_empty(), "a lattice and florets");
    assert_eq!(d.elements[0].id, t, "the trellis goes behind everything");
    let mut m = d.clone(); m.map(1.0, true, 0.0, pt(260.0, 0.0));
    assert_eq!(m.parts().len(), d.parts().len());
}
