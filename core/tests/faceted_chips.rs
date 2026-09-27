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
                let s = ChipSettings { faceted: Some(Faceted { centre, count, border }), ..ChipSettings::default() };
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
                    let s = ChipSettings { size, faceted: Some(Faceted { centre, count, border }), ..ChipSettings::default() };
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
fn moved_chip_keeps_its_facets_and_export_has_facet_lines() {
    let mut s = ChipSettings { faceted: Some(Faceted { centre: Centre::Star, count: 8, border: Some(BorderStyle::Zigzag) }), ..ChipSettings::default() };
    let before = chip_facets(&s)[0].clone();
    s.edits.insert(0, before.outline.iter().map(|p| scroll_core::geometry::pt(p.x + 5.0, p.y)).collect());
    let after = chip_facets(&s)[0].clone();
    assert!((after.floor[0].x - before.floor[0].x - 5.0).abs() < 1e-9 && (after.floor[0].y - before.floor[0].y).abs() < 1e-9);
    let svg = chip_svg(&s);
    assert!(svg.contains("stroke-width=\"0.15\"") && svg.matches("<path").count() > chip_regions(&s).len());
}

#[test]
fn variations_cover_every_centre_and_border() {
    let picks: Vec<Faceted> = (0..200).map(Faceted::from_seed).collect();
    for c in Centre::ALL { assert!(picks.iter().any(|f| f.centre == c), "{c:?} never picked"); }
    for b in [None, Some(BorderStyle::Zigzag), Some(BorderStyle::Arcade), Some(BorderStyle::Almond)] { assert!(picks.iter().any(|f| f.border == b), "{b:?} never picked"); }
}
