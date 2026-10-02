//! The faceted chip engine: every centre, count and border generates chips
//! inside the page, and the export carries the facet lines.
use scroll_core::chip::{chip_facets, chip_regions, chip_svg, ChipSettings, Faceted};
use scroll_core::facets::{BorderStyle, Centre};

#[test]
fn every_permutation_generates_inside_the_page() {
    let borders = [None, Some(BorderStyle::Zigzag), Some(BorderStyle::Arcade), Some(BorderStyle::Almond)];
    for centre in Centre::ALL {
        for count in centre.counts() {
            for border in borders {
                let s = ChipSettings { faceted: Some(Faceted::new(centre, count, border)), ..ChipSettings::default() };
                let regions = chip_regions(&s);
                assert!(regions.len() >= count as usize, "{centre:?} {count} {border:?}: only {} chips", regions.len());
                for r in &regions { for p in r { assert!(p.x >= -1e-6 && p.y >= -1e-6 && p.x <= s.size + 1e-6 && p.y <= s.size + 1e-6, "{centre:?} {count} {border:?}: chip leaves the page"); } }
                assert_eq!(chip_facets(&s).len(), regions.len());
            }
        }
    }
}

#[test]
fn no_chips_overlap_at_any_page_size() {
    use scroll_core::booleans::{area, signed_area, union};
    let borders = [None, Some(BorderStyle::Zigzag), Some(BorderStyle::Arcade), Some(BorderStyle::Almond)];
    for size in [60.0, 100.0, 180.0] {
        for centre in Centre::ALL {
            for count in [*centre.counts().start(), centre.default_count(), *centre.counts().end()] {
                for border in borders {
                    let s = ChipSettings { size, faceted: Some(Faceted::new(centre, count, border)), ..ChipSettings::default() };
                    let regions = chip_regions(&s);
                    let sum: f64 = regions.iter().map(|r| signed_area(r).abs()).sum();
                    let merged = area(&union(&regions.iter().map(|r| r.as_slice()).collect::<Vec<_>>()));
                    assert!(sum - merged < sum * 0.005, "{size} mm {centre:?} {count} {border:?}: chips overlap by {:.1} mm²", sum - merged);
                }
            }
        }
    }
}

#[test]
fn rectangular_pages_stay_inside_and_never_overlap() {
    use scroll_core::booleans::{area, signed_area, union};
    for (w, h) in [(150.0, 100.0), (100.0, 150.0), (200.0, 80.0), (100.0, 70.0)] {
        for centre in Centre::ALL {
            for border in [None, Some(BorderStyle::Zigzag), Some(BorderStyle::Arcade), Some(BorderStyle::Almond)] {
                let s = ChipSettings { size: w, height: Some(h), faceted: Some(Faceted::new(centre, centre.default_count(), border)), ..ChipSettings::default() };
                let regions = chip_regions(&s);
                for r in &regions { for p in r { assert!(p.x >= -1e-6 && p.y >= -1e-6 && p.x <= w + 1e-6 && p.y <= h + 1e-6, "{w}×{h} {centre:?} {border:?}: chip leaves the page"); } }
                let sum: f64 = regions.iter().map(|r| signed_area(r).abs()).sum();
                let merged = area(&union(&regions.iter().map(|r| r.as_slice()).collect::<Vec<_>>()));
                assert!(sum - merged < sum * 0.005, "{w}×{h} {centre:?} {border:?}: chips overlap by {:.1} mm²", sum - merged);
            }
        }
    }
    // the classic generator ignores the height: its pages stay square
    let s = ChipSettings { height: Some(60.0), ..ChipSettings::default() };
    assert_eq!(s.page_height(), s.size);
}

#[test]
fn moved_chip_keeps_its_facets_and_export_has_facet_lines() {
    let mut s = ChipSettings { faceted: Some(Faceted::new(Centre::Star, 8, Some(BorderStyle::Zigzag))), ..ChipSettings::default() };
    let before = chip_facets(&s)[0].clone();
    s.edits.insert(0, before.outline.iter().map(|p| scroll_core::geometry::pt(p.x + 5.0, p.y)).collect());
    let after = chip_facets(&s)[0].clone();
    assert!((after.floor[0].x - before.floor[0].x - 5.0).abs() < 1e-9 && (after.floor[0].y - before.floor[0].y).abs() < 1e-9);
    let svg = chip_svg(&s);
    assert!(svg.contains("stroke-width=\"0.15\"") && svg.matches("<path").count() > chip_regions(&s).len());
}

/// A lopsided blob round the page centre, like a hand-drawn lasso.
fn blob(size: f64) -> Vec<scroll_core::geometry::Point> {
    use std::f64::consts::PI;
    (0..90).map(|k| { let t = k as f64 / 90.0 * 2.0 * PI; let r = size * 0.42 * (1.0 + 0.1 * (3.0 * t).sin()); scroll_core::geometry::pt(size / 2.0 + r * t.cos(), size / 2.0 + r * t.sin() * 0.9) }).collect()
}

#[test]
fn fills_stay_inside_their_outline_and_clear_of_other_chips() {
    use scroll_core::booleans::{area, intersect, offset, clean_with, signed_area};
    use scroll_core::chip::{generated_with_starts, ChipFill, FillLayout};
    use scroll_core::facets::{FillEdge, Repeat};
    for pattern in Repeat::ALL {
        for (edge, layout) in [(FillEdge::Clip, FillLayout::Grid), (FillEdge::Whole, FillLayout::Grid), (FillEdge::Clip, FillLayout::Flow), (FillEdge::Whole, FillLayout::Flow)] {
            let mut fill = ChipFill::new(blob(150.0), pattern, 10.0); fill.edge = edge; fill.layout = layout; fill.edge_row = edge == FillEdge::Clip;
            // a flow fill needs room round the centre for its band
            let mut f = Faceted::new(Centre::Star, 8, Some(BorderStyle::Zigzag)); if layout == FillLayout::Flow { f.scale = 0.6; }
            let s = ChipSettings { size: 150.0, faceted: Some(f), fills: vec![fill.clone()], ..ChipSettings::default() };
            let (chips, starts) = generated_with_starts(&s);
            let (base, filled) = chips.split_at(starts[0]);
            assert!(filled.len() > 20, "{pattern:?} {edge:?} {layout:?}: only {} fill chips", filled.len());
            // inside the lasso (allowing for grid snapping in the booleans)
            let region = offset(&clean_with(&fill.outline, false), 0.05);
            for c in filled {
                let a = signed_area(&c.outline).abs();
                assert!(area(&intersect(&vec![vec![c.outline.clone()]], &region)) > a * 0.999, "{pattern:?} {edge:?}: a chip leaves the lasso");
            }
            // clear of the rosette and border by (almost) the margin
            for c in filled { for b in base {
                let grown = offset(&vec![vec![b.outline.clone()]], fill.margin * 0.9);
                assert!(area(&intersect(&vec![vec![c.outline.clone()]], &grown)) < 1e-3, "{pattern:?} {edge:?}: a fill chip crowds an existing chip");
            } }
        }
    }
}

#[test]
fn fill_does_not_seep_into_a_rosette() {
    use scroll_core::chip::{generated_with_starts, ChipFill};
    use scroll_core::facets::Repeat;
    // the lasso covers the whole page; the rosette's own uncut wood must stay uncut
    let square = vec![scroll_core::geometry::pt(0.0, 0.0), scroll_core::geometry::pt(150.0, 0.0), scroll_core::geometry::pt(150.0, 150.0), scroll_core::geometry::pt(0.0, 150.0)];
    let s = ChipSettings { size: 150.0, faceted: Some(Faceted::new(Centre::Star, 8, Some(BorderStyle::Zigzag))), fills: vec![ChipFill::new(square, Repeat::NodeStars, 8.0)], ..ChipSettings::default() };
    let (chips, starts) = generated_with_starts(&s);
    let rosette_r = chips[..starts[0]].iter().flat_map(|c| c.outline.iter()).map(|p| (p.x - 75.0).hypot(p.y - 75.0)).filter(|r| *r < 50.0).fold(0.0, f64::max);
    for c in &chips[starts[0]..] { for p in &c.outline { assert!((p.x - 75.0).hypot(p.y - 75.0) > rosette_r * 0.8, "fill chip inside the rosette at {p:?}"); } }
}

#[test]
fn variations_cover_every_centre_and_border() {
    let picks: Vec<Faceted> = (0..200).map(Faceted::from_seed).collect();
    for c in Centre::ALL { assert!(picks.iter().any(|f| f.centre == c), "{c:?} never picked"); }
    for b in [None, Some(BorderStyle::Zigzag), Some(BorderStyle::Arcade), Some(BorderStyle::Almond)] { assert!(picks.iter().any(|f| f.border == b), "{b:?} never picked"); }
}
