//! Boxes built from chip panels: sizes, linked faces, resizing, and the 3D view's
//! faces meeting along the box's edges.
use scroll_core::boxes::{BoxDesign, Face};
use scroll_core::chip::{chip_regions, ChipSettings, Faceted};
use scroll_core::facets::{BorderStyle, Centre};

fn lid() -> ChipSettings { ChipSettings { size: 150.0, height: Some(100.0), faceted: Some(Faceted::new(Centre::Star, 8, Some(BorderStyle::Arcade))), ..ChipSettings::default() } }

#[test]
fn a_box_grows_from_its_lid_or_its_front() {
    let b = BoxDesign::from_panel(&lid(), Face::Lid, 60.0);
    assert_eq!((b.length, b.width, b.height), (150.0, 100.0, 60.0));
    assert_eq!(b.size_of(Face::Front), (150.0, 60.0));
    assert_eq!(b.size_of(Face::Left), (100.0, 60.0));
    // the sides take the lid's border style, with a field inside
    let f = b.front.faceted.unwrap();
    assert_eq!(f.border, Some(BorderStyle::Arcade)); assert!(f.field.is_some());
    assert_eq!((b.front.width(), b.front.page_height()), (150.0, 60.0));
    assert_eq!(b.faces(), vec![Face::Lid, Face::Front, Face::Back, Face::Left, Face::Right]);

    let front = ChipSettings { size: 200.0, height: Some(80.0), faceted: Some(Faceted::new(Centre::Fan, 12, Some(BorderStyle::Zigzag))), ..ChipSettings::default() };
    let b = BoxDesign::from_panel(&front, Face::Front, 120.0);
    assert_eq!((b.length, b.width, b.height), (200.0, 120.0, 80.0));
    assert_eq!((b.lid.width(), b.lid.page_height()), (200.0, 120.0));
    assert_eq!(b.front, front);
}

#[test]
fn back_and_right_follow_until_edited() {
    let mut b = BoxDesign::from_panel(&lid(), Face::Lid, 60.0);
    assert!(b.linked(Face::Back) && b.linked(Face::Right));
    assert_eq!(b.panel(Face::Back), b.panel(Face::Front));
    let mut own = b.front.clone(); own.seed = Some(7);
    b.set_panel(Face::Back, own.clone());
    assert!(!b.linked(Face::Back));
    assert_eq!(b.panel(Face::Back), Some(&own)); assert_ne!(b.panel(Face::Back), b.panel(Face::Front));
    b.relink(Face::Back);
    assert_eq!(b.panel(Face::Back), b.panel(Face::Front));
    assert!(b.panel(Face::Bottom).is_none());
    b.set_bottom(true); assert_eq!(b.panel(Face::Bottom).map(|p| (p.width(), p.page_height())), Some((150.0, 100.0)));
    b.set_bottom(false); assert!(b.panel(Face::Bottom).is_none());
}

#[test]
fn resizing_regenerates_every_panel_on_its_page() {
    let b = BoxDesign::from_panel(&lid(), Face::Lid, 60.0).resized(200.0, 120.0, 80.0);
    for face in b.faces() {
        let p = b.panel(face).unwrap(); let (w, h) = b.size_of(face);
        assert_eq!((p.width(), p.page_height()), (w, h), "{face:?}");
        for r in chip_regions(p) { for q in r { assert!(q.x >= -1e-6 && q.y >= -1e-6 && q.x <= w + 1e-6 && q.y <= h + 1e-6, "{face:?}: a chip leaves the panel"); } }
    }
    assert!(b.sheet_svg().contains("Right side"));
}

#[test]
fn faces_meet_along_the_box_edges_in_3d() {
    let b = BoxDesign::from_panel(&lid(), Face::Lid, 60.0);
    let (l, w, h) = (b.length, b.width, b.height);
    for from_back in [false, true] {
        let (faces, _, _) = b.iso_view(from_back);
        let at = |m: [f64; 6], x: f64, y: f64| (m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5]);
        let close = |a: (f64, f64), b: (f64, f64)| (a.0 - b.0).abs() < 1e-9 && (a.1 - b.1).abs() < 1e-9;
        let (top, front, side) = (faces[0].1, faces[1].1, faces[2].1);
        // the lid's near edge is the front's top edge; the front's right edge is the side's left edge
        let lid_near = if from_back { (at(top, l, 0.0), at(top, 0.0, 0.0)) } else { (at(top, 0.0, w), at(top, l, w)) };
        assert!(close(lid_near.0, at(front, 0.0, 0.0)) && close(lid_near.1, at(front, l, 0.0)), "lid and front meet (from back: {from_back})");
        assert!(close(at(front, l, 0.0), at(side, 0.0, 0.0)) && close(at(front, l, h), at(side, 0.0, h)), "front and side meet (from back: {from_back})");
    }
}
