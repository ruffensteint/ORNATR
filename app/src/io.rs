//! Layout files: the same JSON the web version saves (camelCase), so a
//! layout opens in either program. Kept parts are not carried over.
use scroll_core::geometry::{pt, Curve, Point};
use scroll_core::growth::{Family, GrowthSettings, Side};
use scroll_core::model::{Layout, Placement};
use scroll_core::shoots::{ShootEdit, ShootParams};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy)]
pub struct P { pub x: f64, pub y: f64 }

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct G {
    #[serde(default = "seed")] pub seed: f64,
    #[serde(default = "five")] pub branches: f64,
    #[serde(default = "reach")] pub reach: f64,
    #[serde(default = "one")] pub curl: f64,
    #[serde(default = "two")] pub levels: f64,
    #[serde(default = "two")] pub leaves: f64,
    #[serde(default = "two")] pub clearance: f64,
    #[serde(default = "stem")] pub stem: f64,
    #[serde(default = "alt")] pub side: String,
    #[serde(skip_serializing_if = "Option::is_none")] pub family: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")] pub composition: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")] pub secondary_scale: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")] pub sweeps: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")] pub auto_shoots: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")] pub flip: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub free: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub attach: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub wraps: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub wrap_leaf: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub collar: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub collar_style: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub vine: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub eyes: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub vine_leaf: Option<bool>,
}
fn seed() -> f64 { 1248.0 } fn five() -> f64 { 5.0 } fn reach() -> f64 { 33.0 } fn one() -> f64 { 1.0 } fn two() -> f64 { 2.0 } fn stem() -> f64 { 2.8 } fn alt() -> String { "alternate".into() }

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct S {
    pub id: String, #[serde(default)] pub backbone: usize, pub progress: f64, pub reach: f64, pub turn: f64, pub curl: f64, pub side: f64,
    #[serde(skip_serializing_if = "Option::is_none")] pub leaf_side: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")] pub stem: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")] pub leaf_scale: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")] pub lobes: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")] pub depth: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")] pub stalk: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")] pub taper: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")] pub bend: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")] pub preset: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub follow: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub fan: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub on: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub eyes: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")] pub replaces: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")] pub hidden: bool,
    #[serde(default, skip_serializing_if = "is_false")] pub under: bool,
}
fn is_false(b: &bool) -> bool { !*b }

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct I { pub id: String, pub motif: String, pub progress: f64, pub length: f64, pub fullness: f64, pub angle: f64, pub bend: f64, pub mirror: bool, pub folds: bool, #[serde(default)] pub backbone: Option<usize>, #[serde(default)] pub on_top: Option<bool> }

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct File {
    pub version: u32, pub width: f64, pub height: f64, pub curve: [P; 4],
    #[serde(default, skip_serializing_if = "Vec::is_empty")] pub extra_curves: Vec<[P; 4]>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub backbone_growth: Option<Vec<G>>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub growth: Option<G>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")] pub shoots: Vec<S>,
    #[serde(default)] pub items: Vec<I>,
    #[serde(default)] pub print_backbone: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub mode: Option<String>,
    /// The carving surface preset ("plaque", "oval", "rectangle"); none is the page.
    #[serde(default, skip_serializing_if = "Option::is_none")] pub surface: Option<String>,
}

fn curve(c: &[P; 4]) -> Curve { [pt(c[0].x, c[0].y), pt(c[1].x, c[1].y), pt(c[2].x, c[2].y), pt(c[3].x, c[3].y)] }
fn curve_out(c: &Curve) -> [P; 4] { c.map(|p: Point| P { x: p.x, y: p.y }) }
fn growth_in(g: &G) -> GrowthSettings {
    GrowthSettings { seed: g.seed as u32, branches: g.branches, reach: g.reach, curl: g.curl, levels: g.levels as u8, leaves: g.leaves as u8, clearance: g.clearance, stem: g.stem, vine: g.vine.filter(|v| v.is_finite()).map(|v| v.clamp(15.0, 200.0)),
        side: match g.side.as_str() { "left" => Side::Left, "right" => Side::Right, _ => Side::Alternate },
        family: g.family.as_deref().and_then(family_in), composition: g.composition.map(|v| v as u8), secondary_scale: g.secondary_scale, sweeps: g.sweeps.map(|v| v as u8), auto_shoots: g.auto_shoots, flip: g.flip, free: g.free, attach: g.attach.filter(|a| a.is_finite() && *a >= 0.0 && *a < 20.0).map(|a| a as usize), wraps: g.wraps.filter(|w| w.is_finite() && *w >= 0.0).map(|w| w.min(2.0) as u8), wrap_leaf: g.wrap_leaf.clone().filter(|id| scroll_core::profiles::profile(id).is_some()), collar: g.collar.filter(|c| c.is_finite() && *c > 0.0).map(|c| c.clamp(0.4, 2.5)), collar_style: g.collar_style.clone().filter(|s| scroll_core::collar::CollarStyle::from_id(s).is_some()), eyes: g.eyes.filter(|e| e.is_finite() && *e >= 0.0).map(|e| e.min(3.0) as u8), vine_leaf: g.vine_leaf }
}
pub fn family_in(s: &str) -> Option<Family> { match s { "spiral" => Some(Family::Spiral), "spray" => Some(Family::Spray), "border" => Some(Family::Border), "fan" => Some(Family::Fan), "branching" => Some(Family::Branching), _ => None } }
pub fn family_name(f: Family) -> &'static str { match f { Family::Spiral => "spiral", Family::Spray => "spray", Family::Border => "border", Family::Fan => "fan", Family::Branching => "branching" } }
fn growth_out(g: &GrowthSettings) -> G {
    G { seed: g.seed as f64, branches: g.branches, reach: g.reach, curl: g.curl, levels: g.levels as f64, leaves: g.leaves as f64, clearance: g.clearance, stem: g.stem,
        side: match g.side { Side::Left => "left", Side::Right => "right", Side::Alternate => "alternate" }.into(),
        family: g.family.map(|f| family_name(f).into()), composition: g.composition.map(|v| v as f64), secondary_scale: g.secondary_scale, sweeps: g.sweeps.map(|v| v as f64), auto_shoots: g.auto_shoots, flip: g.flip, free: g.free, attach: g.attach.map(|a| a as f64), wraps: g.wraps.map(|w| w as f64), wrap_leaf: g.wrap_leaf.clone(), collar: g.collar, collar_style: g.collar_style.clone(), vine: g.vine, eyes: g.eyes.map(|e| e as f64), vine_leaf: g.vine_leaf }
}

pub fn parse(text: &str) -> Result<Layout, String> {
    let f: File = serde_json::from_str(text).map_err(|e| format!("Not an ORNATR layout: {e}"))?;
    if f.version != 1 { return Err("Unsupported layout version.".into()); }
    if !(40.0..=1000.0).contains(&f.width) || !(40.0..=1000.0).contains(&f.height) { return Err("Page size must be 40–1000 mm.".into()); }
    let mut curves = vec![curve(&f.curve)]; curves.extend(f.extra_curves.iter().map(curve));
    let base = f.growth.as_ref().map(growth_in).unwrap_or_default();
    let growth = match &f.backbone_growth { Some(v) if v.len() == curves.len() => v.iter().map(growth_in).collect(), _ => vec![base; curves.len()] };
    let shoots = f.shoots.iter().filter(|s| s.backbone < curves.len()).map(|s| ShootEdit { id: s.id.clone(), backbone: s.backbone, replaces: s.replaces.clone(), hidden: s.hidden, under: s.under,
        params: ShootParams { progress: s.progress, reach: s.reach, turn: s.turn, curl: s.curl, side: s.side, leaf_side: s.leaf_side, stem: s.stem, leaf_scale: s.leaf_scale, lobes: s.lobes, depth: s.depth, stalk: s.stalk, taper: s.taper, bend: s.bend, preset: s.preset.clone(), follow: s.follow.filter(|f| f.is_finite()).map(|f| f.clamp(0.0, 1.0)), fan: s.fan.filter(|f| f.is_finite() && *f >= 2.0).map(|f| f.min(3.0) as u8), on: s.on.clone(), eyes: s.eyes.filter(|e| e.is_finite() && *e >= 0.0).map(|e| e.min(3.0) as u8) } }).collect();
    let items = f.items.iter().map(|i| Placement { id: i.id.clone(), motif: i.motif.clone(), progress: i.progress, length: i.length, fullness: i.fullness, angle: i.angle, bend: i.bend, mirror: i.mirror, folds: i.folds, backbone: i.backbone.unwrap_or(0), on_top: i.on_top }).collect();
    Ok(Layout { width: f.width, height: f.height, curves, growth, locked_parts: vec![], shoots, items, print_backbone: f.print_backbone, frame: None, surface: f.surface.clone().filter(|s| scroll_core::model::surface_outline(s, 100.0, 100.0).is_some()) })
}

pub fn save(l: &Layout) -> String {
    let f = File { version: 1, width: l.width, height: l.height, curve: curve_out(&l.curves[0]), extra_curves: l.curves[1..].iter().map(curve_out).collect(),
        backbone_growth: Some(l.growth.iter().map(growth_out).collect()), growth: l.growth.first().map(growth_out),
        shoots: l.shoots.iter().map(|e| S { id: e.id.clone(), backbone: e.backbone, progress: e.params.progress, reach: e.params.reach, turn: e.params.turn, curl: e.params.curl, side: e.params.side, leaf_side: e.params.leaf_side, stem: e.params.stem, leaf_scale: e.params.leaf_scale, lobes: e.params.lobes, depth: e.params.depth, stalk: e.params.stalk, taper: e.params.taper, bend: e.params.bend, preset: e.params.preset.clone(), follow: e.params.follow, fan: e.params.fan.map(|f| f as f64), on: e.params.on.clone(), eyes: e.params.eyes.map(|v| v as f64), replaces: e.replaces.clone(), hidden: e.hidden, under: e.under }).collect(),
        items: vec![], print_backbone: l.print_backbone, mode: Some("growth".into()), surface: l.surface.clone() };
    serde_json::to_string_pretty(&f).unwrap()
}

// ---------- chip layouts ----------
use scroll_core::chip::{ChipFamily, ChipFill, ChipSettings, Faceted, FillLayout};
use scroll_core::facets::{BorderStyle, Centre, FillEdge, Repeat};
use std::collections::BTreeMap;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChipFile {
    pub family: String, pub count: f64, pub size: f64, pub removed: Vec<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub seed: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub grid: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub edits: Option<BTreeMap<String, Vec<P>>>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub grammar: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub border_seed: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub border_version: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub traditional: Option<bool>,
    /// Faceted engine: rosette key, its count, and the border key ("none" for no border).
    #[serde(default, skip_serializing_if = "Option::is_none")] pub facet_centre: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub facet_count: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub facet_border: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub facet_scale: Option<f64>,
    /// A repeat key: a field of it fills the inside of the border instead of a rosette (absent: rosette).
    #[serde(default, skip_serializing_if = "Option::is_none")] pub facet_field: Option<String>,
    /// Page height in mm for a rectangular faceted page (absent: square).
    #[serde(default, skip_serializing_if = "Option::is_none")] pub height: Option<f64>,
    /// Lasso fills: freehand regions filled with a square repeat.
    #[serde(default, skip_serializing_if = "Option::is_none")] pub fills: Option<Vec<FillFile>>,
}

/// A fill as saved: outline in page mm, repeat key, cell size, margin, edge ("clip" or "whole"),
/// and (optional) layout ("flow"; absent = the original grid) with its edge row.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FillFile { pub outline: Vec<P>, pub pattern: String, pub cell: f64, pub margin: f64, pub edge: String,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub layout: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub edge_row: Option<bool> }

fn whole(v: f64, lo: f64, hi: f64) -> bool { v.is_finite() && v.fract() == 0.0 && v >= lo && v <= hi }

/// Chip layout JSON, as the web chip generator saved it.
pub fn parse_chip(text: &str) -> Result<ChipSettings, String> {
    let f: ChipFile = serde_json::from_str(text).map_err(|_| "Not a chip layout.".to_string())?;
    let bad = |m: &str| Err(m.to_string());
    let Some(family) = ChipFamily::from_key(&f.family) else { return bad("Unsupported chip pattern.") };
    if !whole(f.count, 4.0, 16.0) || !(f.size.is_finite() && (40.0..=600.0).contains(&f.size)) || f.height.is_some_and(|h| !(h.is_finite() && (40.0..=600.0).contains(&h))) || f.removed.len() > 10000 || !f.removed.iter().all(|n| whole(*n, 0.0, 9999.0)) { return bad("Unsupported chip pattern."); }
    if f.seed.is_some_and(|s| !whole(s, 0.0, 4294967295.0)) || f.border_seed.is_some_and(|s| !whole(s, 0.0, 4294967295.0)) { return bad("Invalid chip seed."); }
    if f.border_version.is_some_and(|v| v != 1.0) || f.grammar.is_some_and(|v| v != 2.0) { return bad("Unsupported chip composition."); }
    if f.grid.is_some_and(|g| !(g.is_finite() && g >= 2.0 && g <= 30f64.min(f.size / 6.0))) { return bad("Invalid grid interval."); }
    let mut edits = BTreeMap::new();
    for (k, v) in f.edits.unwrap_or_default() {
        let Ok(i) = k.parse::<usize>() else { return bad("Invalid chip edits.") };
        if i >= 10000 || v.len() < 3 || v.len() > 200 || !v.iter().all(|p| p.x.is_finite() && p.y.is_finite() && p.x >= 0.0 && p.y >= 0.0 && p.x <= f.size && p.y <= f.height.unwrap_or(f.size).max(f.size)) { return bad("Invalid chip corner."); }
        edits.insert(i, v.iter().map(|p| pt(p.x, p.y)).collect());
    }
    let faceted = match &f.facet_centre {
        None => None,
        Some(k) => {
            let Some(centre) = Centre::from_key(k) else { return bad("Unsupported chip pattern.") };
            let count = f.facet_count.unwrap_or(centre.default_count() as f64);
            if !whole(count, *centre.counts().start() as f64, *centre.counts().end() as f64) { return bad("Unsupported chip pattern."); }
            let border = match f.facet_border.as_deref() { None | Some("none") => None, Some(b) => match BorderStyle::from_key(b) { Some(b) => Some(b), None => return bad("Unsupported chip border.") } };
            let scale = f.facet_scale.unwrap_or(1.0);
            if !(scale.is_finite() && Faceted::SCALES.contains(&scale)) { return bad("Invalid centre size."); }
            let field = match f.facet_field.as_deref() { None => None, Some(k) => match Repeat::from_key(k) { Some(r) => Some(r), None => return bad("Unsupported chip pattern.") } };
            Some(Faceted { centre, count: count as u32, border, scale, field })
        }
    };
    let mut fills = vec![];
    for x in f.fills.unwrap_or_default() {
        let Some(pattern) = Repeat::from_key(&x.pattern) else { return bad("Unsupported chip fill.") };
        let edge = match x.edge.as_str() { "clip" => FillEdge::Clip, "whole" => FillEdge::Whole, _ => return bad("Unsupported chip fill.") };
        let finite = |v: f64| v.is_finite();
        if x.outline.len() < 3 || x.outline.len() > 5000 || !x.outline.iter().all(|p| finite(p.x) && finite(p.y) && p.x.abs() < 10000.0 && p.y.abs() < 10000.0)
            || !(finite(x.cell) && ChipFill::CELLS.contains(&x.cell)) || !(finite(x.margin) && ChipFill::MARGINS.contains(&x.margin)) { return bad("Invalid chip fill."); }
        let layout = match x.layout.as_deref() { None | Some("grid") => FillLayout::Grid, Some("flow") => FillLayout::Flow, _ => return bad("Unsupported chip fill.") };
        fills.push(ChipFill { outline: x.outline.iter().map(|p| pt(p.x, p.y)).collect(), pattern, cell: x.cell, margin: x.margin, edge, layout, edge_row: x.edge_row.unwrap_or(true) });
    }
    Ok(ChipSettings { family, count: f.count as u32, size: f.size, removed: f.removed.iter().map(|n| *n as usize).collect(), seed: f.seed.map(|s| s as u32), grid: f.grid, edits,
        grammar: f.grammar.map(|v| v as u8), border_seed: f.border_seed.map(|s| s as u32), border_version: f.border_version.map(|v| v as u8), traditional: f.traditional, faceted, fills, height: f.height })
}

pub fn save_chip(s: &ChipSettings) -> String {
    let f = ChipFile { family: s.family.key().into(), count: s.count as f64, size: s.size, removed: s.removed.iter().map(|n| *n as f64).collect(), seed: s.seed.map(|v| v as f64), grid: s.grid,
        edits: if s.edits.is_empty() { None } else { Some(s.edits.iter().map(|(k, v)| (k.to_string(), v.iter().map(|p| P { x: p.x, y: p.y }).collect())).collect()) },
        grammar: s.grammar.map(|v| v as f64), border_seed: s.border_seed.map(|v| v as f64), border_version: s.border_version.map(|v| v as f64), traditional: s.traditional,
        facet_centre: s.faceted.map(|f| f.centre.key().into()), facet_count: s.faceted.map(|f| f.count as f64),
        facet_border: s.faceted.map(|f| f.border.map_or("none", |b| b.key()).into()),
        facet_scale: s.faceted.and_then(|f| (f.scale != 1.0).then_some(f.scale)),
        facet_field: s.faceted.and_then(|f| f.field.map(|r| r.key().into())),
        height: s.height,
        fills: if s.fills.is_empty() { None } else { Some(s.fills.iter().map(|x| FillFile { outline: x.outline.iter().map(|p| P { x: p.x, y: p.y }).collect(), pattern: x.pattern.key().into(), cell: x.cell, margin: x.margin, edge: if x.edge == FillEdge::Clip { "clip" } else { "whole" }.into(),
            layout: (x.layout == FillLayout::Flow).then(|| "flow".into()), edge_row: (x.layout == FillLayout::Flow).then_some(x.edge_row) }).collect()) } };
    serde_json::to_string_pretty(&f).unwrap()
}

// ---------- boxes ----------
use scroll_core::boxes::{BoxDesign, Face, DIMENSIONS};

/// A box: its outer dimensions, and each carved face as a chip layout. A missing back or
/// right side follows the front or left side; a missing bottom isn't carved.
pub fn save_box(b: &BoxDesign) -> String {
    let mut root = serde_json::Map::new();
    root.insert("box".into(), serde_json::json!({ "length": b.length, "width": b.width, "height": b.height }));
    let mut put = |face: Face, s: Option<&ChipSettings>| { if let Some(s) = s { root.insert(face.key().into(), serde_json::from_str(&save_chip(s)).unwrap()); } };
    put(Face::Lid, Some(&b.lid)); put(Face::Front, Some(&b.front)); put(Face::Left, Some(&b.left));
    put(Face::Back, b.back.as_ref()); put(Face::Right, b.right.as_ref()); put(Face::Bottom, b.bottom.as_ref());
    serde_json::to_string_pretty(&serde_json::Value::Object(root)).unwrap()
}

/// A box saved by `save_box`; Err for anything else (including a single chip layout).
pub fn parse_box(text: &str) -> Result<BoxDesign, String> {
    let v: serde_json::Value = serde_json::from_str(text).map_err(|_| "Not a box.".to_string())?;
    let dims = v.get("box").ok_or_else(|| "Not a box.".to_string())?;
    let dim = |k: &str| dims.get(k).and_then(|x| x.as_f64()).filter(|x| x.is_finite() && DIMENSIONS.contains(x)).ok_or_else(|| "Invalid box size.".to_string());
    let (length, width, height) = (dim("length")?, dim("width")?, dim("height")?);
    let panel = |face: Face| -> Result<Option<ChipSettings>, String> {
        match v.get(face.key()) { None => Ok(None), Some(p) => parse_chip(&p.to_string()).map(Some).map_err(|e| format!("{}: {e}", face.label())) }
    };
    let need = |face: Face| -> Result<ChipSettings, String> { panel(face)?.ok_or_else(|| format!("The box has no {}.", face.label().to_lowercase())) };
    Ok(BoxDesign { length, width, height, lid: need(Face::Lid)?, front: need(Face::Front)?, left: need(Face::Left)?, back: panel(Face::Back)?, right: panel(Face::Right)?, bottom: panel(Face::Bottom)? })
}
