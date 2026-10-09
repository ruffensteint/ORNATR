//! A scroll's volute shaped by hand: the handle sits on the grown tip, and
//! dragging it to a tip gives back the volute that grows there.
use scroll_core::cartouche::{structure, Kind, START};
use scroll_core::geometry::{distance, pt};
use scroll_core::growth::{GrowthSettings, Side};
use scroll_core::model::Layout;

fn layout(size: Option<f64>, turns: Option<f64>, side: Side) -> Layout {
    let mut l = Layout::starter();
    l.growth = vec![GrowthSettings { free: Some(true), volute_size: size, volute_turns: turns, side, ..l.growth_for(0) }];
    l
}
fn main_tip(l: &Layout) -> scroll_core::geometry::Point {
    let g = l.grow();
    *g.parts.iter().find(|p| p.parent.is_none()).unwrap().points.last().unwrap()
}

#[test]
fn handle_sits_on_the_grown_tip() {
    for (size, turns) in [(None, None), (Some(1.6), Some(1.3)), (Some(0.6), Some(0.5))] {
        let l = layout(size, turns, Side::Right);
        let v = l.volute_handle(0).unwrap();
        assert!(distance(v.tip, main_tip(&l)) < 1e-6, "{size:?} {turns:?}");
    }
}

#[test]
fn grabbing_the_tip_changes_nothing() {
    for (size, turns, side) in [(1.0, 1.0, Side::Right), (1.7, 1.25, Side::Left), (0.5, 0.45, Side::Right), (2.4, 1.6, Side::Left)] {
        let mut v = layout(Some(size), Some(turns), side).volute_handle(0).unwrap();
        let (s, t, sd) = v.drag_to(v.tip);
        assert_eq!(sd, side);
        assert!((s - size).abs() < 1e-6 && (t - turns).abs() < 1e-6, "wanted {size} {turns}, got {s} {t}");
    }
}

#[test]
fn round_the_eye_rolls_it_in_and_away_from_the_stem_sizes_it() {
    let l = layout(None, None, Side::Right);
    let v0 = l.volute_handle(0).unwrap();
    let rot = v0.angle - v0.side.atan2(-scroll_core::growth::CURL_DECAY);
    let reach = distance(v0.root, v0.tip) / { let th = scroll_core::growth::curl_sweep(0.95); let e = (-0.24 * th).exp(); (e * th.cos() - 1.0).hypot(e * th.sin()) };
    let centre = pt(v0.root.x - reach * rot.cos(), v0.root.y - reach * rot.sin());
    // a quarter turn round the eye, the way the curl winds, in small steps
    let (dx, dy) = (v0.tip.x - centre.x, v0.tip.y - centre.y);
    let mut v = v0;
    v.drag_to(v0.tip);
    let mut last = (1.0, 1.0);
    for i in 1..=30 {
        let a = v0.side * std::f64::consts::FRAC_PI_2 * i as f64 / 30.0;
        let p = pt(centre.x + dx * a.cos() - dy * a.sin(), centre.y + dx * a.sin() + dy * a.cos());
        let (s, t, _) = v.drag_to(p);
        assert!((s - last.0).abs() < 0.1 && (t - last.1).abs() < 0.05, "step {i}: {last:?} -> {s} {t}");
        last = (s, t);
    }
    assert!(last.1 > 1.1, "a quarter turn further in: {last:?}");
    // pulled out from the stem's end: bigger (that line isn't straight out
    // from the eye, so it rolls a little too)
    let mut v = v0;
    v.drag_to(v0.tip);
    let out = pt(v0.root.x + (v0.tip.x - v0.root.x) * 1.5, v0.root.y + (v0.tip.y - v0.root.y) * 1.5);
    let (s, t, _) = v.drag_to(out);
    assert!(s > 1.2 && (t - 1.0).abs() < 0.3, "{s} {t}");
    // and the new tip sits as far from the stem's end as the pointer
    let tip = layout(Some(s), Some(t), Side::Right).volute_handle(0).unwrap().tip;
    assert!((distance(tip, v0.root) - distance(out, v0.root)).abs() < 0.05);
}

#[test]
fn a_drag_keeps_the_side() {
    let mut v = layout(None, None, Side::Left).volute_handle(0).unwrap();
    v.drag_to(v.tip);
    let (_, _, side) = v.drag_to(pt(v.root.x + 40.0, v.root.y + 40.0));
    assert_eq!(side, Side::Left);
}

#[test]
fn automatic_volute_is_untouched() {
    // None and an explicit 1 × 1 grow the same scroll
    let (a, b) = (layout(None, None, Side::Right), layout(Some(1.0), Some(1.0), Side::Right));
    let (ga, gb) = (a.grow(), b.grow());
    assert_eq!(ga.parts.len(), gb.parts.len());
    for (p, q) in ga.parts.iter().zip(&gb.parts) { assert_eq!(p.polygon, q.polygon); }
}

#[test]
fn cartouche_scrolls_take_a_volute_on_every_copy() {
    let mut d = structure(START.0).unwrap();
    let id = d.elements.iter().find(|e| e.is_scroll() && d.copies_of(e).len() > 1).unwrap().id;
    let before: Vec<_> = (0..2).map(|k| d.volute_handle(id, k).unwrap()).collect();
    if let Some(Kind::Scroll { volute, .. }) = d.get_mut(id).map(|e| &mut e.kind) { volute.replace((1.5, 1.0)); }
    for k in 0..2 {
        let after = d.volute_handle(id, k).unwrap();
        // the same root, the tip further out: both copies follow
        assert!(distance(after.root, before[k].root) < 1e-9);
        assert!(distance(after.tip, after.root) > distance(before[k].tip, before[k].root) * 1.2, "copy {k}");
    }
}
