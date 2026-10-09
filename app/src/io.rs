//! Layout files: the same JSON the web version saves (camelCase), so a
//! layout opens in either program. Kept parts are not carried over.
use scroll_core::geometry::{pt, Curve, Point};
use scroll_core::growth::{Family, GrowthSettings, Side, VOLUTE_SIZE, VOLUTE_TURNS};
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
    #[serde(default, skip_serializing_if = "Option::is_none")] pub volute_size: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub volute_turns: Option<f64>,
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
        family: g.family.as_deref().and_then(family_in), composition: g.composition.map(|v| v as u8), secondary_scale: g.secondary_scale, sweeps: g.sweeps.map(|v| v as u8), auto_shoots: g.auto_shoots, flip: g.flip, free: g.free, attach: g.attach.filter(|a| a.is_finite() && *a >= 0.0 && *a < 20.0).map(|a| a as usize), wraps: g.wraps.filter(|w| w.is_finite() && *w >= 0.0).map(|w| w.min(2.0) as u8), wrap_leaf: g.wrap_leaf.clone().filter(|id| scroll_core::profiles::profile(id).is_some()), collar: g.collar.filter(|c| c.is_finite() && *c > 0.0).map(|c| c.clamp(0.4, 2.5)), collar_style: g.collar_style.clone().filter(|s| scroll_core::collar::CollarStyle::from_id(s).is_some()), eyes: g.eyes.filter(|e| e.is_finite() && *e >= 0.0).map(|e| e.min(3.0) as u8), vine_leaf: g.vine_leaf,
        volute_size: g.volute_size.filter(|v| v.is_finite()).map(|v| v.clamp(VOLUTE_SIZE.0, VOLUTE_SIZE.1)), volute_turns: g.volute_turns.filter(|v| v.is_finite()).map(|v| v.clamp(VOLUTE_TURNS.0, VOLUTE_TURNS.1)) }
}
pub fn family_in(s: &str) -> Option<Family> { match s { "spiral" => Some(Family::Spiral), "spray" => Some(Family::Spray), "border" => Some(Family::Border), "fan" => Some(Family::Fan), "branching" => Some(Family::Branching), _ => None } }
pub fn family_name(f: Family) -> &'static str { match f { Family::Spiral => "spiral", Family::Spray => "spray", Family::Border => "border", Family::Fan => "fan", Family::Branching => "branching" } }
fn growth_out(g: &GrowthSettings) -> G {
    G { seed: g.seed as f64, branches: g.branches, reach: g.reach, curl: g.curl, levels: g.levels as f64, leaves: g.leaves as f64, clearance: g.clearance, stem: g.stem,
        side: match g.side { Side::Left => "left", Side::Right => "right", Side::Alternate => "alternate" }.into(),
        family: g.family.map(|f| family_name(f).into()), composition: g.composition.map(|v| v as f64), secondary_scale: g.secondary_scale, sweeps: g.sweeps.map(|v| v as f64), auto_shoots: g.auto_shoots, flip: g.flip, free: g.free, attach: g.attach.map(|a| a as f64), wraps: g.wraps.map(|w| w as f64), wrap_leaf: g.wrap_leaf.clone(), collar: g.collar, collar_style: g.collar_style.clone(), vine: g.vine, eyes: g.eyes.map(|e| e as f64), vine_leaf: g.vine_leaf, volute_size: g.volute_size, volute_turns: g.volute_turns }
}

/// The scroll layout the app opens on: the ORNATR mark, grown in the app
/// (`studies/logo/picks/logo-A-pick.ornatr`, written by `core/examples/logo_study.rs`, `PICK=1`).
pub const STARTUP: &str = include_str!("../assets/startup.ornatr");

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

// ---------- rococo designs ----------
use scroll_core::rocaille::Turnover;
use scroll_core::rococo::{rim_from_ctrl, Anchor, Design, Element, Kind, Node};
/// A rim point and its two handles (in: a, out: b).
#[derive(Serialize, Deserialize)]
struct RNode { x: f64, y: f64, ax: f64, ay: f64, bx: f64, by: f64 }

/// A rococo design file: `{"rococo": {width, height, elements: [...]}}`, the
/// elements back to front, each tagged by its kind.
#[derive(Serialize, Deserialize)]
struct RococoFile { rococo: RDesign }
#[derive(Serialize, Deserialize)]
struct RDesign { width: f64, height: f64, elements: Vec<RItem> }
/// One element: its kind's fields, and `grown` when it grows out of its rim.
#[derive(Serialize, Deserialize)]
struct RItem { #[serde(flatten)] el: REl, #[serde(default, skip_serializing_if = "is_false")] grown: bool }
fn is_zero(v: &f64) -> bool { *v == 0.0 }
/// Where an ornament sits: on rim `rim` (share `u`, then `along`/`across`), or at (x, y).
#[derive(Serialize, Deserialize, Default)]
struct RAt {
    #[serde(default, skip_serializing_if = "Option::is_none")] rim: Option<u32>,
    #[serde(default)] u: f64, #[serde(default)] along: f64, #[serde(default)] across: f64,
    #[serde(default)] x: f64, #[serde(default)] y: f64,
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
enum REl {
    /// `nodes`: points with their handles. `ctrl`: the first version's
    /// through-points (volute signs then followed the rim's turn); read only.
    Rim { id: u32, #[serde(default, skip_serializing_if = "Vec::is_empty")] nodes: Vec<RNode>, #[serde(default, skip_serializing_if = "Vec::is_empty")] ctrl: Vec<P>, width: f64, outer: f64, hook0: f64, eye0: f64, hook1: f64, eye1: f64,
        #[serde(default, skip_serializing_if = "is_zero")] swell: f64, #[serde(default, skip_serializing_if = "Vec::is_empty")] twists: Vec<f64> },
    Frond { id: u32, at: RAt, heading: f64, length: f64, side: f64, bend: f64, fingers: u32, width: f64, splay: f64, turn: String },
    Shell { id: u32, at: RAt, axis: f64, span: f64, ribs: u32, r: f64, asym: f64, twist: f64, scallop: f64 },
    Rosette { id: u32, at: RAt, turn: f64, r: f64, petals: u32 },
    Run { id: u32, rim: u32, from: f64, to: f64, outer: f64, count: u32, length: f64, fingers: u32, last: String },
    Frill { id: u32, rim: u32, from: f64, to: f64, across: f64, side: f64, depth: f64, waves: u32, broken: f64 },
    Pocket { id: u32, ctrl: Vec<P>, lip: f64 },
    Cabochon { id: u32, at: RAt, turn: f64, rx: f64, ry: f64 },
    Trellis { id: u32, ctrl: Vec<P>, cell: f64, florets: bool },
}
pub fn turnover_key(t: Turnover) -> &'static str { match t { Turnover::None => "none", Turnover::Roll => "roll", Turnover::Flap => "flap", Turnover::Curl => "curl" } }
fn turnover_in(s: &str) -> Turnover { match s { "roll" => Turnover::Roll, "flap" => Turnover::Flap, "curl" => Turnover::Curl, _ => Turnover::None } }
fn at_out(a: &Anchor) -> RAt { match *a { Anchor::Rim { rim, u, along, across } => RAt { rim: Some(rim), u, along, across, ..Default::default() }, Anchor::Free(p) => RAt { x: p.x, y: p.y, ..Default::default() } } }
fn at_in(a: &RAt) -> Anchor { match a.rim { Some(rim) => Anchor::Rim { rim, u: a.u, along: a.along, across: a.across }, None => Anchor::Free(pt(a.x, a.y)) } }
fn pts_out(v: &[Point]) -> Vec<P> { v.iter().map(|p| P { x: p.x, y: p.y }).collect() }
fn pts_in(v: &[P]) -> Vec<Point> { v.iter().map(|p| pt(p.x, p.y)).collect() }
/// -1, 0 or +1.
fn sign3(v: f64) -> f64 { if v > 0.0 { 1.0 } else if v < 0.0 { -1.0 } else { 0.0 } }

pub fn save_rococo(d: &Design) -> String {
    let elements = d.elements.iter().map(|e| { let id = e.id; match &e.kind {
        Kind::Rim { nodes, width, outer, hook0, eye0, hook1, eye1, swell, twists } => REl::Rim { id, nodes: nodes.iter().map(|n| RNode { x: n.p.x, y: n.p.y, ax: n.a.x, ay: n.a.y, bx: n.b.x, by: n.b.y }).collect(), ctrl: vec![], width: *width, outer: *outer, hook0: *hook0, eye0: *eye0, hook1: *hook1, eye1: *eye1, swell: *swell, twists: twists.clone() },
        Kind::Frond { at, heading, length, side, bend, fingers, width, splay, turn } => REl::Frond { id, at: at_out(at), heading: *heading, length: *length, side: *side, bend: *bend, fingers: *fingers as u32, width: *width, splay: *splay, turn: turnover_key(*turn).into() },
        Kind::Shell { at, axis, span, ribs, r, asym, twist, scallop } => REl::Shell { id, at: at_out(at), axis: *axis, span: *span, ribs: *ribs as u32, r: *r, asym: *asym, twist: *twist, scallop: *scallop },
        Kind::Rosette { at, turn, r, petals } => REl::Rosette { id, at: at_out(at), turn: *turn, r: *r, petals: *petals as u32 },
        Kind::Run { rim, from, to, outer, count, length, fingers, last } => REl::Run { id, rim: *rim, from: *from, to: *to, outer: *outer, count: *count as u32, length: *length, fingers: *fingers as u32, last: turnover_key(*last).into() },
        Kind::Frill { rim, from, to, across, side, depth, waves, broken } => REl::Frill { id, rim: *rim, from: *from, to: *to, across: *across, side: *side, depth: *depth, waves: *waves as u32, broken: *broken },
        Kind::Pocket { ctrl, lip } => REl::Pocket { id, ctrl: pts_out(ctrl), lip: *lip },
        Kind::Cabochon { at, turn, rx, ry } => REl::Cabochon { id, at: at_out(at), turn: *turn, rx: *rx, ry: *ry },
        Kind::Trellis { ctrl, cell, florets } => REl::Trellis { id, ctrl: pts_out(ctrl), cell: *cell, florets: *florets },
    } }).map(|el| RItem { el, grown: false }).zip(d.elements.iter()).map(|(mut item, e)| { item.grown = e.grown; item }).collect();
    serde_json::to_string_pretty(&RococoFile { rococo: RDesign { width: d.width, height: d.height, elements } }).unwrap()
}

/// Which workspace a file belongs to. Every workspace saves `.ornatr` files (JSON),
/// so a file is known by what is in it: a palmette design (`palmette`), a cartouche (`cartouche`), a rococo design (`rococo`), a box (`box`),
/// a scroll layout, a chip layout. A new style's files need their own top-level key
/// and a line here.
pub fn file_kind(text: &str) -> Option<crate::platform::OpenFor> {
    use crate::platform::OpenFor;
    if is_palmette(text) { Some(OpenFor::Palmette) }
    else if is_cartouche(text) { Some(OpenFor::Cartouche) }
    else if is_rococo(text) { Some(OpenFor::Rococo) }
    else if parse_box(text).is_ok() { Some(OpenFor::Chip) }
    else if parse(text).is_ok() { Some(OpenFor::Scroll) }
    else if parse_chip(text).is_ok() { Some(OpenFor::Chip) }
    else { None }
}

/// Whether a file is a rococo design (they share the .ornatr extension with scroll layouts).
pub fn is_rococo(text: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(text).is_ok_and(|v| v.get("rococo").is_some_and(|r| r.is_object()))
}

/// A design saved by `save_rococo`. Sizes and counts are kept in sensible
/// ranges; elements with numbers that are not finite are dropped.
pub fn parse_rococo(text: &str) -> Result<Design, String> {
    let f: RococoFile = serde_json::from_str(text).map_err(|e| format!("Not a rococo design: {e}"))?;
    let RDesign { width, height, elements } = f.rococo;
    if !(40.0..=1000.0).contains(&width) || !(40.0..=1000.0).contains(&height) { return Err("Page size must be 40–1000 mm.".into()); }
    let n = |v: u32, lo: u32, hi: u32| v.clamp(lo, hi) as usize;
    let mut out: Vec<Element> = vec![];
    for RItem { el, grown } in elements {
        let (id, kind) = match el {
            REl::Rim { id, nodes, ctrl, width, outer, hook0, eye0, hook1, eye1, swell, twists } => {
                let outer = if outer < 0.0 { -1.0 } else { 1.0 };
                let (nodes, hook0, hook1) = if nodes.len() >= 2 {
                    (nodes.iter().map(|n| Node { p: pt(n.x, n.y), a: pt(n.ax, n.ay), b: pt(n.bx, n.by) }).collect(), sign3(hook0), sign3(hook1))
                } else if ctrl.len() >= 2 && pts_in(&ctrl).iter().all(|p| p.x.is_finite() && p.y.is_finite()) {
                    rim_from_ctrl(&pts_in(&ctrl), outer, sign3(hook0), sign3(hook1))
                } else { continue };
                (id, Kind::Rim { nodes, width: width.clamp(0.5, 40.0), outer, hook0, eye0: eye0.clamp(0.0, 200.0), hook1, eye1: eye1.clamp(0.0, 200.0), swell: if swell.is_finite() { swell.clamp(0.0, 1.0) } else { 0.0 }, twists: twists.into_iter().filter(|t| t.is_finite()).map(|t| t.clamp(0.0, 1.0)).take(16).collect() })
            }
            REl::Frond { id, at, heading, length, side, bend, fingers, width, splay, turn } => (id, Kind::Frond { at: at_in(&at), heading, length: length.clamp(2.0, 600.0), side: if side < 0.0 { -1.0 } else { 1.0 }, bend, fingers: n(fingers, 1, 9), width: width.clamp(0.5, 200.0), splay, turn: turnover_in(&turn) }),
            REl::Shell { id, at, axis, span, ribs, r, asym, twist, scallop } => (id, Kind::Shell { at: at_in(&at), axis, span: span.clamp(0.3, 6.0), ribs: n(ribs, 3, 25), r: r.clamp(1.0, 600.0), asym, twist, scallop }),
            REl::Rosette { id, at, turn, r, petals } => (id, Kind::Rosette { at: at_in(&at), turn, r: r.clamp(0.5, 200.0), petals: n(petals, 3, 12) }),
            REl::Run { id, rim, from, to, outer, count, length, fingers, last } => (id, Kind::Run { rim, from, to, outer: if outer < 0.0 { -1.0 } else { 1.0 }, count: n(count, 1, 12), length: length.clamp(2.0, 400.0), fingers: n(fingers, 1, 9), last: turnover_in(&last) }),
            REl::Frill { id, rim, from, to, across, side, depth, waves, broken } => (id, Kind::Frill { rim, from, to, across, side: if side < 0.0 { -1.0 } else { 1.0 }, depth: depth.clamp(0.5, 200.0), waves: n(waves, 1, 30), broken: broken.clamp(0.0, 1.0) }),
            REl::Pocket { id, ctrl, lip } => { if ctrl.len() < 3 { continue; } (id, Kind::Pocket { ctrl: pts_in(&ctrl), lip: lip.clamp(0.0, 20.0) }) }
            REl::Cabochon { id, at, turn, rx, ry } => (id, Kind::Cabochon { at: at_in(&at), turn, rx: rx.clamp(1.0, 300.0), ry: ry.clamp(1.0, 300.0) }),
            REl::Trellis { id, ctrl, cell, florets } => { if ctrl.len() < 3 { continue; } (id, Kind::Trellis { ctrl: pts_in(&ctrl), cell: cell.clamp(2.0, 100.0), florets }) }
        };
        let mut e = Element { id, kind, grown };
        e.grown = e.grown && e.can_grow();
        if finite(&e) && !out.iter().any(|o| o.id == id) { out.push(e); }
    }
    Ok(Design { width, height, elements: out })
}
// ---------- cartouche designs ----------
pub use cart::{is_cartouche, parse_cartouche, save_cartouche};
mod cart {
    use super::{is_false, pts_in, pts_out, P};
    use scroll_core::cartouche::{Design, Element, FieldShape, Kind, Moulding, Repeat};
    use scroll_core::geometry::{pt, Point};
    use scroll_core::growth::{Side, VOLUTE_SIZE, VOLUTE_TURNS};
    use serde::{Deserialize, Serialize};

    /// A cartouche design file: `{"cartouche": {width, height, centre, growth,
    /// collars, eyes, elements: [...]}}`, the elements back to front, each tagged
    /// by its kind, with `mirror` ("x", "y", "xy") and `ring` when it repeats.
    #[derive(Serialize, Deserialize)]
    struct CartoucheFile { cartouche: CDesign }
    #[derive(Serialize, Deserialize)]
    struct CDesign { width: f64, height: f64, centre: P, growth: f64, #[serde(default)] collars: bool, #[serde(default, skip_serializing_if = "is_false")] eyes: bool, elements: Vec<CItem> }
    #[derive(Serialize, Deserialize)]
    struct CItem { #[serde(flatten)] el: CEl, #[serde(default, skip_serializing_if = "Option::is_none")] mirror: Option<String>, #[serde(default, skip_serializing_if = "Option::is_none")] ring: Option<u8> }
    #[derive(Serialize, Deserialize)]
    #[serde(tag = "kind", rename_all = "camelCase")]
    enum CEl {
        Frame { id: u32, shape: String, rx: f64, ry: f64, #[serde(default)] round: f64, width: f64, moulding: String, #[serde(default)] bead: f64 },
        /// `voluteSize`, `voluteTurns` (2026-10-07): the volute shaped by hand, left out when automatic.
        #[serde(rename_all = "camelCase")]
        Scroll { id: u32, curve: Vec<P>, curl: String, scale: f64, levels: u8, #[serde(default)] flip: bool, #[serde(default, skip_serializing_if = "Option::is_none")] attach: Option<u32>, seed: u32,
            #[serde(default, skip_serializing_if = "Option::is_none")] volute_size: Option<f64>, #[serde(default, skip_serializing_if = "Option::is_none")] volute_turns: Option<f64> },
        #[serde(rename_all = "camelCase")]
        /// `turn`, `bend`, `width`, `follow` (2026-10-07) are left out at their defaults.
        Leaf { id: u32, stem: u32, preset: String, at: f64, side: f64, fan: u8, size: f64, #[serde(default)] mirror_side: bool,
            #[serde(default, skip_serializing_if = "zero")] turn: f64, #[serde(default, skip_serializing_if = "zero")] bend: f64,
            #[serde(default = "unit", skip_serializing_if = "is_unit")] width: f64, #[serde(default = "follow", skip_serializing_if = "is_follow")] follow: f64 },
        Jewel { id: u32, c: P, rx: f64, ry: f64, #[serde(default)] turn: f64, bezel: f64 },
        Boss { id: u32, c: P, r: f64 },
        Shell { id: u32, c: P, axis: f64, span: f64, ribs: u32, r: f64, asym: f64, twist: f64, scallop: f64 },
        Strap { id: u32, ctrl: Vec<P>, width: f64, hook0: f64, eye0: f64, hook1: f64, eye1: f64 },
        Opening { id: u32, c: P, rx: f64, ry: f64, #[serde(default)] turn: f64 },
    }
    fn p_out(p: Point) -> P { P { x: p.x, y: p.y } }
    fn p_in(p: &P) -> Point { pt(p.x, p.y) }
    fn side_key(s: Side) -> &'static str { match s { Side::Left => "left", Side::Right => "right", Side::Alternate => "alternate" } }
    fn side_in(s: &str) -> Side { match s { "left" => Side::Left, "alternate" => Side::Alternate, _ => Side::Right } }
    fn shape_key(s: FieldShape) -> &'static str { match s { FieldShape::Oval => "oval", FieldShape::Rect => "rectangle", FieldShape::Shield => "shield" } }
    fn shape_in(s: &str) -> FieldShape { match s { "rectangle" => FieldShape::Rect, "shield" => FieldShape::Shield, _ => FieldShape::Oval } }
    fn moulding_key(m: Moulding) -> &'static str { match m { Moulding::Moulded => "moulded", Moulding::Strap => "strap", Moulding::Bead => "bead" } }
    fn moulding_in(s: &str) -> Moulding { match s { "strap" => Moulding::Strap, "bead" => Moulding::Bead, _ => Moulding::Moulded } }
    fn sign(v: f64) -> f64 { if v > 0.0 { 1.0 } else if v < 0.0 { -1.0 } else { 0.0 } }
    fn zero(v: &f64) -> bool { *v == 0.0 }
    fn unit() -> f64 { 1.0 }
    fn is_unit(v: &f64) -> bool { *v == 1.0 }
    fn follow() -> f64 { scroll_core::cartouche::LEAF_FOLLOW }
    fn is_follow(v: &f64) -> bool { *v == scroll_core::cartouche::LEAF_FOLLOW }

    pub fn save_cartouche(d: &Design) -> String {
        let elements = d.elements.iter().map(|e| {
            let id = e.id;
            let el = match &e.kind {
                Kind::Frame { shape, rx, ry, round, width, moulding, bead } => CEl::Frame { id, shape: shape_key(*shape).into(), rx: *rx, ry: *ry, round: *round, width: *width, moulding: moulding_key(*moulding).into(), bead: *bead },
                Kind::Scroll { curve, curl, scale, levels, flip, attach, seed, volute } => CEl::Scroll { id, curve: curve.iter().map(|p| p_out(*p)).collect(), curl: side_key(*curl).into(), scale: *scale, levels: *levels, flip: *flip, attach: *attach, seed: *seed, volute_size: volute.map(|v| v.0), volute_turns: volute.map(|v| v.1) },
                Kind::Leaf { stem, preset, at, side, fan, size, mirror_side, turn, bend, width, follow } => CEl::Leaf { id, stem: *stem, preset: preset.clone(), at: *at, side: *side, fan: *fan, size: *size, mirror_side: *mirror_side, turn: *turn, bend: *bend, width: *width, follow: *follow },
                Kind::Jewel { c, rx, ry, turn, bezel } => CEl::Jewel { id, c: p_out(*c), rx: *rx, ry: *ry, turn: *turn, bezel: *bezel },
                Kind::Boss { c, r } => CEl::Boss { id, c: p_out(*c), r: *r },
                Kind::Shell { c, axis, span, ribs, r, asym, twist, scallop } => CEl::Shell { id, c: p_out(*c), axis: *axis, span: *span, ribs: *ribs as u32, r: *r, asym: *asym, twist: *twist, scallop: *scallop },
                Kind::Strap { ctrl, width, hook0, eye0, hook1, eye1 } => CEl::Strap { id, ctrl: pts_out(ctrl), width: *width, hook0: *hook0, eye0: *eye0, hook1: *hook1, eye1: *eye1 },
                Kind::Opening { c, rx, ry, turn } => CEl::Opening { id, c: p_out(*c), rx: *rx, ry: *ry, turn: *turn },
            };
            let r = e.repeat;
            let mirror = match (r.mirror_x, r.mirror_y) { (true, true) => Some("xy"), (true, false) => Some("x"), (false, true) => Some("y"), _ => None }.filter(|_| e.repeats()).map(String::from);
            CItem { el, mirror, ring: (r.ring > 1 && e.repeats()).then_some(r.ring) }
        }).collect();
        serde_json::to_string_pretty(&CartoucheFile { cartouche: CDesign { width: d.width, height: d.height, centre: p_out(d.centre), growth: d.growth, collars: d.collars, eyes: d.eyes, elements } }).unwrap()
    }

    /// Whether a file is a cartouche design (every workspace saves .ornatr).
    pub fn is_cartouche(text: &str) -> bool {
        serde_json::from_str::<serde_json::Value>(text).is_ok_and(|v| v.get("cartouche").is_some_and(|r| r.is_object()))
    }

    /// A design saved by `save_cartouche`. Sizes and counts are kept in
    /// sensible ranges; elements with numbers that are not finite, and leaves
    /// whose scroll is missing, are dropped.
    pub fn parse_cartouche(text: &str) -> Result<Design, String> {
        let f: CartoucheFile = serde_json::from_str(text).map_err(|e| format!("Not a cartouche design: {e}"))?;
        let CDesign { width, height, centre, growth, collars, eyes, elements } = f.cartouche;
        if !(40.0..=1000.0).contains(&width) || !(40.0..=1000.0).contains(&height) { return Err("Page size must be 40–1000 mm.".into()); }
        if !(centre.x.is_finite() && centre.y.is_finite() && growth.is_finite()) { return Err("The design's centre or scroll weight is not a number.".into()); }
        let mut out: Vec<Element> = vec![];
        for CItem { el, mirror, ring } in elements {
            let (id, kind) = match el {
                CEl::Frame { id, shape, rx, ry, round, width, moulding, bead } => (id, Kind::Frame { shape: shape_in(&shape), rx: rx.clamp(2.0, 500.0), ry: ry.clamp(2.0, 500.0), round: round.clamp(0.0, 500.0), width: width.clamp(0.5, 100.0), moulding: moulding_in(&moulding), bead: bead.clamp(0.0, 10.0) }),
                CEl::Scroll { id, curve, curl, scale, levels, flip, attach, seed, volute_size, volute_turns } => {
                    if curve.len() != 4 { continue; }
                    (id, Kind::Scroll { curve: [p_in(&curve[0]), p_in(&curve[1]), p_in(&curve[2]), p_in(&curve[3])], curl: side_in(&curl), scale: scale.clamp(0.1, 3.0), levels: levels.min(3), flip, attach: attach.filter(|a| *a != id), seed,
                        volute: (volute_size.is_some() || volute_turns.is_some()).then(|| (volute_size.filter(|v| v.is_finite()).map_or(1.0, |v| v.clamp(VOLUTE_SIZE.0, VOLUTE_SIZE.1)), volute_turns.filter(|v| v.is_finite()).map_or(1.0, |v| v.clamp(VOLUTE_TURNS.0, VOLUTE_TURNS.1)))) })
                }
                CEl::Leaf { id, stem, preset, at, side, fan, size, mirror_side, turn, bend, width, follow } => (id, Kind::Leaf { stem, preset, at: at.clamp(0.0, 1.0), side: if side < 0.0 { -1.0 } else { 1.0 }, fan: fan.clamp(1, 3), size: size.clamp(0.1, 6.0), mirror_side, turn, bend: bend.clamp(-3.0, 3.0), width: width.clamp(0.2, 3.0), follow: follow.clamp(0.0, 1.0) }),
                CEl::Jewel { id, c, rx, ry, turn, bezel } => (id, Kind::Jewel { c: p_in(&c), rx: rx.clamp(0.3, 300.0), ry: ry.clamp(0.3, 300.0), turn, bezel: bezel.clamp(0.0, 50.0) }),
                CEl::Boss { id, c, r } => (id, Kind::Boss { c: p_in(&c), r: r.clamp(0.3, 300.0) }),
                CEl::Shell { id, c, axis, span, ribs, r, asym, twist, scallop } => (id, Kind::Shell { c: p_in(&c), axis, span: span.clamp(0.3, 6.0), ribs: ribs.clamp(3, 25) as usize, r: r.clamp(1.0, 600.0), asym, twist, scallop }),
                CEl::Strap { id, ctrl, width, hook0, eye0, hook1, eye1 } => { if ctrl.len() < 2 { continue; } (id, Kind::Strap { ctrl: pts_in(&ctrl), width: width.clamp(0.3, 50.0), hook0: sign(hook0), eye0: eye0.clamp(0.0, 200.0), hook1: sign(hook1), eye1: eye1.clamp(0.0, 200.0) }) }
                CEl::Opening { id, c, rx, ry, turn } => (id, Kind::Opening { c: p_in(&c), rx: rx.clamp(0.5, 300.0), ry: ry.clamp(0.5, 300.0), turn }),
            };
            let m = mirror.as_deref().unwrap_or("");
            let repeat = Repeat { mirror_x: m.contains('x'), mirror_y: m.contains('y'), ring: ring.unwrap_or(1).clamp(1, 24) };
            let e = Element { id, kind, repeat };
            if finite(&e) && !out.iter().any(|o| o.id == id) { out.push(e); }
        }
        // leaves need their scroll; attachments a scroll to grow from
        let scrolls: Vec<u32> = out.iter().filter(|e| e.is_scroll()).map(|e| e.id).collect();
        out.retain(|e| !matches!(&e.kind, Kind::Leaf { stem, .. } if !scrolls.contains(stem)));
        for e in out.iter_mut() { if let Kind::Scroll { attach, .. } = &mut e.kind { if attach.is_some_and(|a| !scrolls.contains(&a)) { *attach = None; } } }
        Ok(Design { width, height, centre: p_in(&centre), growth: growth.clamp(0.1, 20.0), collars, eyes, elements: out })
    }
    fn finite(e: &Element) -> bool {
        let all = |v: &[f64]| v.iter().all(|x| x.is_finite());
        let pts = |v: &[Point]| v.iter().all(|p| p.x.is_finite() && p.y.is_finite());
        match &e.kind {
            Kind::Frame { rx, ry, round, width, bead, .. } => all(&[*rx, *ry, *round, *width, *bead]),
            Kind::Scroll { curve, scale, .. } => pts(curve) && scale.is_finite(),
            Kind::Leaf { at, size, turn, bend, width, follow, .. } => all(&[*at, *size, *turn, *bend, *width, *follow]),
            Kind::Jewel { c, rx, ry, turn, bezel } => pts(&[*c]) && all(&[*rx, *ry, *turn, *bezel]),
            Kind::Boss { c, r } => pts(&[*c]) && r.is_finite(),
            Kind::Shell { c, axis, span, r, asym, twist, scallop, .. } => pts(&[*c]) && all(&[*axis, *span, *r, *asym, *twist, *scallop]),
            Kind::Strap { ctrl, width, eye0, eye1, .. } => pts(ctrl) && all(&[*width, *eye0, *eye1]),
            Kind::Opening { c, rx, ry, turn } => pts(&[*c]) && all(&[*rx, *ry, *turn]),
        }
    }
}

/// Every number in an element is finite.
fn finite(e: &Element) -> bool {
    let at = |a: &Anchor| match a { Anchor::Rim { u, along, across, .. } => u.is_finite() && along.is_finite() && across.is_finite(), Anchor::Free(p) => p.x.is_finite() && p.y.is_finite() };
    let all = |v: &[f64]| v.iter().all(|x| x.is_finite());
    let pts = |v: &[Point]| v.iter().all(|p| p.x.is_finite() && p.y.is_finite());
    match &e.kind {
        Kind::Rim { nodes, width, eye0, eye1, .. } => nodes.iter().all(|n| pts(&[n.p, n.a, n.b])) && all(&[*width, *eye0, *eye1]),
        Kind::Frond { at: a, heading, length, bend, width, splay, .. } => at(a) && all(&[*heading, *length, *bend, *width, *splay]),
        Kind::Shell { at: a, axis, span, r, asym, twist, scallop, .. } => at(a) && all(&[*axis, *span, *r, *asym, *twist, *scallop]),
        Kind::Rosette { at: a, turn, r, .. } => at(a) && all(&[*turn, *r]),
        Kind::Run { from, to, length, .. } => all(&[*from, *to, *length]),
        Kind::Frill { from, to, across, depth, broken, .. } => all(&[*from, *to, *across, *depth, *broken]),
        Kind::Pocket { ctrl, lip } => pts(ctrl) && lip.is_finite(),
        Kind::Cabochon { at: a, turn, rx, ry } => at(a) && all(&[*turn, *rx, *ry]),
        Kind::Trellis { ctrl, cell, .. } => pts(ctrl) && cell.is_finite(),
    }
}

// ---------- palmette designs ----------
pub use palm::{is_palmette, parse_palmette, save_palmette};
mod palm {
    use super::P;
    use scroll_core::geometry::{pt, Point};
    use scroll_core::palmette::{Design, Element, Kind, Repeat, Tip};
    use serde::{Deserialize, Serialize};

    /// A palmette design file: `{"palmette": {width, height, centre, elements: [...]}}`,
    /// the motifs back to front, each tagged by its kind, with `mirror` ("x", "y", "xy"),
    /// `ring`, and `row` + `pitch` when it repeats.
    #[derive(Serialize, Deserialize)]
    struct PalmetteFile { palmette: PDesign }
    #[derive(Serialize, Deserialize)]
    struct PDesign { width: f64, height: f64, centre: P, elements: Vec<PItem> }
    #[derive(Serialize, Deserialize)]
    struct PItem { #[serde(flatten)] el: PEl, #[serde(default, skip_serializing_if = "Option::is_none")] mirror: Option<String>, #[serde(default, skip_serializing_if = "Option::is_none")] ring: Option<u8>,
        #[serde(default, skip_serializing_if = "Option::is_none")] row: Option<u8>, #[serde(default, skip_serializing_if = "Option::is_none")] pitch: Option<f64> }
    #[derive(Serialize, Deserialize)]
    #[serde(tag = "kind", rename_all = "camelCase")]
    enum PEl {
        #[serde(rename = "palmette", rename_all = "camelCase")]
        Fan { id: u32, base: P, axis: f64, count: u32, spread: f64, bend: f64, flick: f64, length: f64, decay: f64, belly: f64, roll: f64, root: f64, tip: String, centre_tip: String },
        Petal { id: u32, base: P, heading: f64, length: f64, belly: f64, bend: f64, flick: f64, roll: f64, tip: String },
        Lotus { id: u32, base: P, axis: f64, height: f64 },
        #[serde(rename_all = "camelCase")]
        Stem { id: u32, top: P, heading: f64, length: f64, w_top: f64, w_foot: f64 },
        Arm { id: u32, start: P, heading: f64, length: f64, curve: f64, radius: f64, side: f64, width: f64 },
        Collar { id: u32, c: P, turn: f64, width: f64, height: f64 },
        Boss { id: u32, c: P, r: f64 },
        Fleurette { id: u32, c: P, count: u32, radius: f64, belly: f64, tip: String, back: f64, boss: f64 },
        Husks { id: u32, top: P, heading: f64, count: u32, size: f64, shrink: f64, tear: bool },
        PetalRing { id: u32, c: P, count: u32, root: f64, length: f64, belly: f64, offset: f64, notch: bool, lip: bool },
        BeadedRing { id: u32, c: P, r: f64, width: f64, bead: f64 },
    }
    fn p_out(p: Point) -> P { P { x: p.x, y: p.y } }
    fn p_in(p: &P) -> Point { pt(p.x, p.y) }
    fn tip_key(t: Tip) -> String { match t { Tip::Pointed => "pointed", Tip::Round => "round" }.into() }
    fn tip_in(s: &str) -> Tip { if s == "pointed" { Tip::Pointed } else { Tip::Round } }

    pub fn save_palmette(d: &Design) -> String {
        let elements = d.elements.iter().map(|e| {
            let id = e.id;
            let el = match &e.kind {
                Kind::Fan { base, axis, count, spread, bend, flick, length, decay, belly, roll, root, tip, centre_tip } => PEl::Fan { id, base: p_out(*base), axis: *axis, count: *count as u32, spread: *spread, bend: *bend, flick: *flick, length: *length, decay: *decay, belly: *belly, roll: *roll, root: *root, tip: tip_key(*tip), centre_tip: tip_key(*centre_tip) },
                Kind::Petal { base, heading, length, belly, bend, flick, roll, tip } => PEl::Petal { id, base: p_out(*base), heading: *heading, length: *length, belly: *belly, bend: *bend, flick: *flick, roll: *roll, tip: tip_key(*tip) },
                Kind::Lotus { base, axis, height } => PEl::Lotus { id, base: p_out(*base), axis: *axis, height: *height },
                Kind::Stem { top, heading, length, w_top, w_foot } => PEl::Stem { id, top: p_out(*top), heading: *heading, length: *length, w_top: *w_top, w_foot: *w_foot },
                Kind::Arm { start, heading, length, curve, radius, side, width } => PEl::Arm { id, start: p_out(*start), heading: *heading, length: *length, curve: *curve, radius: *radius, side: *side, width: *width },
                Kind::Collar { c, turn, width, height } => PEl::Collar { id, c: p_out(*c), turn: *turn, width: *width, height: *height },
                Kind::Boss { c, r } => PEl::Boss { id, c: p_out(*c), r: *r },
                Kind::Fleurette { c, count, radius, belly, tip, back, boss } => PEl::Fleurette { id, c: p_out(*c), count: *count as u32, radius: *radius, belly: *belly, tip: tip_key(*tip), back: *back, boss: *boss },
                Kind::Husks { top, heading, count, size, shrink, tear } => PEl::Husks { id, top: p_out(*top), heading: *heading, count: *count as u32, size: *size, shrink: *shrink, tear: *tear },
                Kind::PetalRing { c, count, root, length, belly, offset, notch, lip } => PEl::PetalRing { id, c: p_out(*c), count: *count as u32, root: *root, length: *length, belly: *belly, offset: *offset, notch: *notch, lip: *lip },
                Kind::BeadedRing { c, r, width, bead } => PEl::BeadedRing { id, c: p_out(*c), r: *r, width: *width, bead: *bead },
            };
            let r = e.repeat;
            let mirror = match (r.mirror_x, r.mirror_y) { (true, true) => Some("xy"), (true, false) => Some("x"), (false, true) => Some("y"), _ => None }.map(String::from);
            let row = r.row > 1;
            PItem { el, mirror, ring: (r.ring > 1).then_some(r.ring), row: row.then_some(r.row), pitch: row.then_some(r.pitch) }
        }).collect();
        serde_json::to_string_pretty(&PalmetteFile { palmette: PDesign { width: d.width, height: d.height, centre: p_out(d.centre), elements } }).unwrap()
    }

    /// Whether a file is a palmette design (every workspace saves .ornatr).
    pub fn is_palmette(text: &str) -> bool {
        serde_json::from_str::<serde_json::Value>(text).is_ok_and(|v| v.get("palmette").is_some_and(|r| r.is_object()))
    }

    /// A design saved by `save_palmette`. Sizes and counts are kept in sensible
    /// ranges; elements with numbers that are not finite are dropped.
    pub fn parse_palmette(text: &str) -> Result<Design, String> {
        let f: PalmetteFile = serde_json::from_str(text).map_err(|e| format!("Not a palmette design: {e}"))?;
        let PDesign { width, height, centre, elements } = f.palmette;
        if !(40.0..=1000.0).contains(&width) || !(40.0..=1000.0).contains(&height) { return Err("Page size must be 40–1000 mm.".into()); }
        if !(centre.x.is_finite() && centre.y.is_finite()) { return Err("The design's centre is not a number.".into()); }
        let count = |n: u32, lo: u32, hi: u32| n.clamp(lo, hi) as usize;
        let mut out: Vec<Element> = vec![];
        for PItem { el, mirror, ring, row, pitch } in elements {
            let (id, kind) = match el {
                PEl::Fan { id, base, axis, count: n, spread, bend, flick, length, decay, belly, roll, root, tip, centre_tip } => (id, Kind::Fan { base: p_in(&base), axis, count: count(n, 1, 31), spread: spread.clamp(0.0, 180.0), bend: bend.clamp(-6.0, 6.0), flick: flick.clamp(-6.0, 6.0), length: length.clamp(0.5, 1000.0), decay: decay.clamp(0.0, 0.95), belly: belly.clamp(0.02, 0.6), roll: roll.clamp(-6.0, 6.0), root: root.clamp(0.0, 200.0), tip: tip_in(&tip), centre_tip: tip_in(&centre_tip) }),
                PEl::Petal { id, base, heading, length, belly, bend, flick, roll, tip } => (id, Kind::Petal { base: p_in(&base), heading, length: length.clamp(0.5, 1000.0), belly: belly.clamp(0.02, 0.6), bend: bend.clamp(-6.0, 6.0), flick: flick.clamp(-6.0, 6.0), roll: roll.clamp(-6.0, 6.0), tip: tip_in(&tip) }),
                PEl::Lotus { id, base, axis, height } => (id, Kind::Lotus { base: p_in(&base), axis, height: height.clamp(1.0, 1000.0) }),
                PEl::Stem { id, top, heading, length, w_top, w_foot } => (id, Kind::Stem { top: p_in(&top), heading, length: length.clamp(0.5, 1000.0), w_top: w_top.clamp(0.1, 100.0), w_foot: w_foot.clamp(0.1, 100.0) }),
                PEl::Arm { id, start, heading, length, curve, radius, side, width } => (id, Kind::Arm { start: p_in(&start), heading, length: length.clamp(0.0, 1000.0), curve: curve.clamp(-1.0, 1.0), radius: radius.clamp(1.0, 300.0), side: if side < 0.0 { -1.0 } else { 1.0 }, width: width.clamp(0.2, 100.0) }),
                PEl::Collar { id, c, turn, width, height } => (id, Kind::Collar { c: p_in(&c), turn, width: width.clamp(1.0, 600.0), height: height.clamp(0.5, 200.0) }),
                PEl::Boss { id, c, r } => (id, Kind::Boss { c: p_in(&c), r: r.clamp(0.3, 300.0) }),
                PEl::Fleurette { id, c, count: n, radius, belly, tip, back, boss } => (id, Kind::Fleurette { c: p_in(&c), count: count(n, 2, 48), radius: radius.clamp(1.0, 500.0), belly: belly.clamp(0.02, 0.6), tip: tip_in(&tip), back: back.clamp(0.0, 1.5), boss: boss.clamp(0.3, 200.0) }),
                PEl::Husks { id, top, heading, count: n, size, shrink, tear } => (id, Kind::Husks { top: p_in(&top), heading, count: count(n, 1, 24), size: size.clamp(0.05, 30.0), shrink: shrink.clamp(0.3, 1.2), tear }),
                PEl::PetalRing { id, c, count: n, root, length, belly, offset, notch, lip } => (id, Kind::PetalRing { c: p_in(&c), count: count(n, 2, 48), root: root.clamp(0.0, 500.0), length: length.clamp(1.0, 500.0), belly: belly.clamp(0.02, 0.6), offset, notch, lip }),
                PEl::BeadedRing { id, c, r, width, bead } => (id, Kind::BeadedRing { c: p_in(&c), r: r.clamp(1.0, 500.0), width: width.clamp(0.2, 200.0), bead: bead.clamp(0.2, 50.0) }),
            };
            let m = mirror.as_deref().unwrap_or("");
            let rows = row.unwrap_or(1).clamp(1, 40);
            let repeat = Repeat { mirror_x: m.contains('x'), mirror_y: m.contains('y'), ring: ring.unwrap_or(1).clamp(1, 24), row: rows, pitch: if rows > 1 { pitch.unwrap_or(0.0) } else { 0.0 } };
            let e = Element { id, kind, repeat };
            if finite(&e) && !out.iter().any(|o| o.id == id) { out.push(e); }
        }
        Ok(Design { width, height, centre: p_in(&centre), elements: out })
    }
    fn finite(e: &Element) -> bool {
        let all = |v: &[f64]| v.iter().all(|x| x.is_finite());
        let at = e.anchor();
        all(&[at.x, at.y, e.repeat.pitch]) && match &e.kind {
            Kind::Fan { axis, spread, bend, flick, length, decay, belly, roll, root, .. } => all(&[*axis, *spread, *bend, *flick, *length, *decay, *belly, *roll, *root]),
            Kind::Petal { heading, length, belly, bend, flick, roll, .. } => all(&[*heading, *length, *belly, *bend, *flick, *roll]),
            Kind::Lotus { axis, height, .. } => all(&[*axis, *height]),
            Kind::Stem { heading, length, w_top, w_foot, .. } => all(&[*heading, *length, *w_top, *w_foot]),
            Kind::Arm { heading, length, curve, radius, width, .. } => all(&[*heading, *length, *curve, *radius, *width]),
            Kind::Collar { turn, width, height, .. } => all(&[*turn, *width, *height]),
            Kind::Boss { r, .. } => r.is_finite(),
            Kind::Fleurette { radius, belly, back, boss, .. } => all(&[*radius, *belly, *back, *boss]),
            Kind::Husks { heading, size, shrink, .. } => all(&[*heading, *size, *shrink]),
            Kind::PetalRing { root, length, belly, offset, .. } => all(&[*root, *length, *belly, *offset]),
            Kind::BeadedRing { r, width, bead, .. } => all(&[*r, *width, *bead]),
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn startup_layout_opens_with_the_crest_leaf() {
        let l = super::parse(super::STARTUP).unwrap();
        assert!(l.shoots.iter().any(|s| s.params.preset.as_deref() == Some("crest-leaf")));
        let g = l.grow();
        assert!(g.parts.iter().any(|p| p.shoot.as_ref().and_then(|s| s.preset.as_deref()) == Some("crest-leaf") && p.polygon.len() > 100));
        assert_eq!(super::file_kind(super::STARTUP), Some(crate::platform::OpenFor::Scroll));
    }

    #[test]
    fn palmette_designs_round_trip_and_are_told_apart() {
        use crate::platform::OpenFor;
        for (id, _, _) in scroll_core::palmette::STRUCTURES {
            let d = scroll_core::palmette::Design::from_structure(id, 160.0, 200.0).unwrap();
            let text = save_palmette(&d);
            let back = parse_palmette(&text).unwrap();
            // (serde_json may round a float's last digit on reading: compare what is saved)
            let v = |t: &str| serde_json::from_str::<serde_json::Value>(t).unwrap();
            assert_eq!(v(&save_palmette(&back)), v(&text), "{id}");
            assert_eq!(back.elements.len(), d.elements.len());
            assert_eq!(file_kind(&text), Some(OpenFor::Palmette));
            assert!(!is_cartouche(&text) && !is_rococo(&text) && parse(&text).is_err());
        }
        // the band keeps its rows; a design with every kind round-trips
        let band = save_palmette(&scroll_core::palmette::Design::from_structure("anthemion", 320.0, 100.0).unwrap());
        assert!(band.contains("\"row\": 4") && band.contains("\"pitch\""));
        let mut d = scroll_core::palmette::Design::empty(200.0, 200.0);
        for (what, _, _) in scroll_core::palmette::ADD { d.add(what).unwrap(); }
        let text = save_palmette(&d);
        let v = |t: &str| serde_json::from_str::<serde_json::Value>(t).unwrap();
        assert_eq!(v(&save_palmette(&parse_palmette(&text).unwrap())), v(&text));
        assert!(parse_palmette("{\"cartouche\": {}}").is_err());
    }
    use super::*;
    #[test]
    fn cartouche_designs_round_trip_and_are_told_apart() {
        use crate::platform::OpenFor;
        for (id, _, _) in scroll_core::cartouche::STRUCTURES {
            let mut d = scroll_core::cartouche::structure(id).unwrap();
            (d.width, d.height) = (300.0, 320.0);
            let text = save_cartouche(&d);
            let back = parse_cartouche(&text).unwrap();
            // (serde_json may round a float's last digit on reading: compare what is saved)
            let (a, b): (serde_json::Value, serde_json::Value) = (serde_json::from_str(&text).unwrap(), serde_json::from_str(&save_cartouche(&back)).unwrap());
            assert_eq!(a, b, "{id}");
            assert_eq!(back.elements.len(), d.elements.len());
            assert!(back.elements.iter().zip(&d.elements).all(|(x, y)| x.id == y.id && x.repeat == y.repeat && x.label() == y.label()));
            assert_eq!(file_kind(&text), Some(OpenFor::Cartouche));
            assert!(parse(&text).is_err() && parse_rococo(&text).is_err());
        }
        // a leaf whose scroll is gone is dropped, a missing parent is let go
        let mut d = scroll_core::cartouche::structure("shield").unwrap();
        (d.width, d.height) = (300.0, 320.0);
        let corner = d.elements.iter().find(|e| e.is_scroll()).unwrap().id;
        d.elements.retain(|e| e.id != corner);
        let back = parse_cartouche(&save_cartouche(&d)).unwrap();
        assert!(back.elements.iter().all(|e| !matches!(e.kind, scroll_core::cartouche::Kind::Leaf { stem, .. } | scroll_core::cartouche::Kind::Scroll { attach: Some(stem), .. } if stem == corner)));
        assert!(parse_cartouche("{\"rococo\": {}}").is_err());
    }
    #[test]
    fn rococo_designs_round_trip() {
        for (id, _, _) in scroll_core::rococo::STRUCTURES {
            let d = Design::from_structure(id, 240.0, 250.0).unwrap();
            let back = parse_rococo(&save_rococo(&d)).unwrap();
            // the same to the last digit or so (JSON floats don't always parse back bit for bit)
            let (a, b): (serde_json::Value, serde_json::Value) = (serde_json::from_str(&save_rococo(&d)).unwrap(), serde_json::from_str(&save_rococo(&back)).unwrap());
            assert!(close(&a, &b), "{id}");
        }
        fn close(a: &serde_json::Value, b: &serde_json::Value) -> bool {
            use serde_json::Value::*;
            match (a, b) {
                (Number(x), Number(y)) => (x.as_f64().unwrap() - y.as_f64().unwrap()).abs() < 1e-9,
                (Array(x), Array(y)) => x.len() == y.len() && x.iter().zip(y).all(|(p, q)| close(p, q)),
                (Object(x), Object(y)) => x.len() == y.len() && x.iter().all(|(k, v)| y.get(k).is_some_and(|w| close(v, w))),
                _ => a == b,
            }
        }
        assert!(parse_rococo("{\"width\": 100}").is_err());
        // the traits picks: a swelling rim turning over, grown ornament, a cabochon, a trellis
        let mut d = Design::empty(260.0, 120.0);
        let r = d.add_rim(pt(130.0, 70.0), 170.0);
        if let Some(Kind::Rim { swell, twists, .. }) = d.get_mut(r).map(|e| &mut e.kind) { *swell = 0.8; *twists = vec![0.1, 0.9]; }
        let f = d.add_frond(r, 0.3).unwrap(); d.add_cabochon(r, 0.5); d.add_trellis(pt(130.0, 80.0), 22.0);
        let back = parse_rococo(&save_rococo(&d)).unwrap();
        assert!(back.get(f).unwrap().grown);
        assert!(matches!(&back.get(r).unwrap().kind, Kind::Rim { swell, twists, .. } if *swell == 0.8 && twists.len() == 2));
        assert_eq!(back.elements.len(), d.elements.len());
        assert!(save_rococo(&back).contains("\"grown\": true") && save_rococo(&back).contains("\"kind\": \"trellis\""));
        // files saved before these existed load as before: no swell, no turn-overs, nothing grown
        let plain = Design::from_structure("corner", 240.0, 250.0).unwrap();
        let text = save_rococo(&plain);
        assert!(!text.contains("swell") && !text.contains("twists") && !text.contains("grown"));
        // rococo designs and scroll layouts share .ornatr: they are told apart by content
        let d = Design::from_structure("corner", 240.0, 250.0).unwrap();
        assert!(is_rococo(&save_rococo(&d)));
        assert!(!is_rococo(&save(&scroll_core::model::Layout::starter())));
        assert!(parse(&save_rococo(&d)).is_err());
        // every workspace's files are told apart by what is in them
        use crate::platform::OpenFor;
        let chip = scroll_core::chip::ChipSettings::default();
        assert_eq!(file_kind(&save_rococo(&d)), Some(OpenFor::Rococo));
        assert_eq!(file_kind(&save(&scroll_core::model::Layout::starter())), Some(OpenFor::Scroll));
        assert_eq!(file_kind(&save_chip(&chip)), Some(OpenFor::Chip));
        assert_eq!(file_kind(&save_box(&scroll_core::boxes::BoxDesign::from_panel(&chip, scroll_core::boxes::Face::Lid, 60.0))), Some(OpenFor::Chip));
        assert_eq!(file_kind("{\"hello\": 1}"), None);
        // the first version saved rims as through-points: they open as a few Bézier points
        let old = r#"{"rococo":{"width":240,"height":250,"elements":[{"kind":"rim","id":1,"ctrl":[{"x":20,"y":120},{"x":60,"y":90},{"x":100,"y":80},{"x":140,"y":90},{"x":180,"y":120},{"x":200,"y":160}],"width":6,"outer":-1,"hook0":0,"eye0":0,"hook1":1,"eye1":10},{"kind":"frond","id":2,"at":{"rim":1,"u":0.5,"along":0,"across":-3},"heading":-0.9,"length":40,"side":1,"bend":0.7,"fingers":4,"width":10,"splay":1.05,"turn":"roll"}]}}"#;
        let d = parse_rococo(old).unwrap();
        assert!(matches!(&d.elements[0].kind, Kind::Rim { nodes, hook1, .. } if (3..=6).contains(&nodes.len()) && *hook1 != 0.0));
        assert_eq!(d.parts().len(), 2);
    }
}
