//! Collars: leafage over each fork of an attached backbone when asked for,
//! in every style, drawn on top, near the fork, with clean outlines.
use scroll_core::collar::CollarStyle;
use scroll_core::geometry::distance;
use scroll_core::skeleton::{skeleton_layout, Skeleton};

#[test]
fn collars_sit_over_each_fork_on_top() {
    for style in CollarStyle::ALL { for kind in Skeleton::ALL { for seed in [3u32, 7, 12] {
        let mut l = skeleton_layout(kind, seed, 240.0, 150.0);
        let attached: Vec<usize> = (0..l.curves.len()).filter(|&i| l.growth_for(i).attach.is_some()).collect();
        for &i in &attached { l.growth[i].collar = Some(1.0); l.growth[i].collar_style = Some(style.id().into()); }
        let g = l.grow();
        let per = if matches!(style, CollarStyle::Split | CollarStyle::Pair) { 2 } else { 1 };
        let collars: Vec<_> = g.parts.iter().filter(|p| p.id.contains("/collar")).collect();
        assert_eq!(collars.len(), attached.len() * per, "{style:?} {kind:?} {seed}");
        assert!(g.parts.iter().rev().take(collars.len()).all(|p| p.id.contains("/collar")), "collars are drawn on top");
        for c in &collars {
            let i: usize = c.id.trim_start_matches("backbone-").split('/').next().unwrap().parse().unwrap();
            let join = l.attach_point(i).unwrap();
            let near = c.polygon.iter().map(|q| distance(*q, join)).fold(f64::INFINITY, f64::min);
            let reach = c.polygon.iter().map(|q| distance(*q, join)).fold(0.0, f64::max);
            assert!(near < 4.0, "{style:?} {kind:?}: collar leaf starts at its fork ({near})");
            assert!(reach > 3.0 && reach < 60.0, "{kind:?}: collar size {reach}");
            assert!(c.polygon.iter().all(|q| q.x.is_finite() && q.y.is_finite()));
            assert!(c.shoot.is_none());
        }
        // off unless asked for
        for &i in &attached { l.growth[i].collar = None; }
        assert!(!l.grow().parts.iter().any(|p| p.id.contains("/collar")));
    }}}
}
