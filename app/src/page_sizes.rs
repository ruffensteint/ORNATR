//! Page sizes: built-in presets (carving pieces, box panels, paper), sizes you save
//! (in %APPDATA%\ORNATR\page-sizes.json), millimetre or inch entry, and the New dialog.
use super::*;
use crate::presets::app_dir;
use serde::{Deserialize, Serialize};
use std::ops::RangeInclusive;

/// Built-in sizes in mm, width × height, by group.
pub const BUILT_IN: [(&str, &[(&str, f64, f64)]); 3] = [
    ("Carving pieces", &[("Coaster", 100.0, 100.0), ("Tile", 150.0, 150.0), ("Trivet", 200.0, 200.0), ("Small plaque", 200.0, 150.0), ("Large plaque", 300.0, 200.0), ("Cutting board", 350.0, 250.0)]),
    ("Box panels", &[("Ring box lid", 70.0, 70.0), ("Small box lid", 100.0, 70.0), ("Box lid", 150.0, 100.0), ("Large box lid", 200.0, 130.0), ("Box front", 150.0, 60.0), ("Large box front", 200.0, 80.0)]),
    ("Paper", &[("A5", 148.0, 210.0), ("A4", 210.0, 297.0), ("A3", 297.0, 420.0), ("Letter", 215.9, 279.4), ("Tabloid", 279.4, 431.8)]),
];

#[derive(Serialize, Deserialize, Clone)]
pub struct SavedSize { pub name: String, pub width: f64, pub height: f64 }

/// Your saved sizes, and the name being typed for the next one.
pub struct PageSizes { pub saved: Vec<SavedSize>, pub name: String }

fn store_path() -> Option<PathBuf> { app_dir().map(|d| d.join("page-sizes.json")) }

impl PageSizes {
    pub fn load() -> PageSizes {
        let saved = store_path().and_then(|p| std::fs::read_to_string(p).ok()).and_then(|t| serde_json::from_str::<Vec<SavedSize>>(&t).ok()).unwrap_or_default()
            .into_iter().filter(|s| s.width.is_finite() && s.height.is_finite() && s.width > 0.0 && s.height > 0.0).collect();
        PageSizes { saved, name: String::new() }
    }
    fn store(&self) {
        if let Some(p) = store_path() { if let Some(d) = p.parent() { let _ = std::fs::create_dir_all(d); } let _ = std::fs::write(p, serde_json::to_string_pretty(&self.saved).unwrap()); }
    }
    /// The preset matching this size (either way round), if any.
    pub fn name_of(&self, w: f64, h: f64) -> Option<String> {
        let same = |a: f64, b: f64, c: f64, d: f64| ((a - c).abs() < 0.05 && (b - d).abs() < 0.05) || ((a - d).abs() < 0.05 && (b - c).abs() < 0.05);
        self.saved.iter().find(|s| same(s.width, s.height, w, h)).map(|s| s.name.clone())
            .or_else(|| BUILT_IN.iter().flat_map(|(_, items)| items.iter()).find(|(_, pw, ph)| same(*pw, *ph, w, h)).map(|(n, _, _)| n.to_string()))
    }
}

/// A length for display: millimetres, or inches.
fn shown(mm: f64, inches: bool) -> String { if inches { format!("{:.2} in", mm / 25.4) } else { format!("{} mm", (mm * 10.0).round() / 10.0) } }

/// The page size editor: a preset menu, width and height (mm or in), a turn between
/// portrait and landscape, and saving the size under a name. `square`: one side only
/// (the classic chip generator). Returns the new size when it changes.
pub fn size_editor(ui: &mut egui::Ui, t: &Theme, sizes: &mut PageSizes, inches: &mut bool, w: f64, h: f64, range: RangeInclusive<f64>, square: bool) -> Option<(f64, f64)> {
    let mut out: Option<(f64, f64)> = None;
    let fits = |a: f64, b: f64| range.contains(&a) && range.contains(&b) && (!square || (a - b).abs() < 0.05);
    egui::Grid::new(ui.next_auto_id()).num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
        ui.label("Preset");
        let current = sizes.name_of(w, h).unwrap_or_else(|| "Custom".into());
        let mut delete: Option<usize> = None;
        egui::ComboBox::from_id_salt(ui.next_auto_id()).width(190.0).selected_text(current).show_ui(ui, |ui| {
            for (group, items) in BUILT_IN {
                ui.label(egui::RichText::new(group).small().color(t.dim));
                for (name, pw, ph) in items.iter() {
                    let label = format!("{name}   {} × {}", shown(*pw, *inches), shown(*ph, *inches));
                    if ui.add_enabled(fits(*pw, *ph), egui::SelectableLabel::new(false, label)).clicked() { out = Some((*pw, *ph)); }
                }
            }
            if !sizes.saved.is_empty() {
                ui.label(egui::RichText::new("My sizes (right-click to delete)").small().color(t.dim));
                for (i, s) in sizes.saved.iter().enumerate() {
                    let label = format!("{}   {} × {}", s.name, shown(s.width, *inches), shown(s.height, *inches));
                    let r = ui.add_enabled(fits(s.width, s.height), egui::SelectableLabel::new(false, label));
                    if r.clicked() { out = Some((s.width, s.height)); }
                    r.context_menu(|ui| { if ui.button("Delete this size").clicked() { delete = Some(i); ui.close_menu(); } });
                }
            }
        });
        if let Some(i) = delete { sizes.saved.remove(i); sizes.store(); }
        ui.end_row();

        let unit = if *inches { 25.4 } else { 1.0 };
        let (lo, hi) = (*range.start() / unit, *range.end() / unit);
        let field = |ui: &mut egui::Ui, mm: f64| -> Option<f64> {
            let mut v = mm / unit;
            let dv = egui::DragValue::new(&mut v).range(lo..=hi).speed(if *inches { 0.01 } else { 0.5 }).fixed_decimals(if *inches { 2 } else { 1 }).suffix(if *inches { " in" } else { " mm" });
            ui.add(dv).changed().then(|| ((v * unit) * 10.0).round() / 10.0)
        };
        ui.label("Width");
        if let Some(nw) = field(ui, w) { out = Some((nw, if square { nw } else { h })); }
        ui.end_row();
        ui.label("Height");
        if square { ui.label(egui::RichText::new("same as width (classic patterns are square)").small().color(t.dim)); }
        else if let Some(nh) = field(ui, h) { out = Some((w, nh)); }
        ui.end_row();
        ui.label("Units");
        segmented(ui, t, &[(false, "mm"), (true, "in")], inches);
        ui.end_row();
    });
    ui.horizontal(|ui| {
        if !square && ui.add_enabled((w - h).abs() > 0.05, egui::Button::new("Turn")).on_hover_text("Swap width and height: portrait ↔ landscape").clicked() { out = Some((h, w)); }
        ui.add(egui::TextEdit::singleline(&mut sizes.name).hint_text("Name this size").desired_width(120.0));
        let named = !sizes.name.trim().is_empty();
        if ui.add_enabled(named, egui::Button::new("Save size")).on_hover_text("Keep this size under My sizes in the preset menu").clicked() {
            let name = sizes.name.trim().to_string();
            sizes.saved.retain(|s| s.name != name);
            sizes.saved.push(SavedSize { name, width: w, height: h }); sizes.store(); sizes.name.clear();
        }
    });
    out
}

/// The New dialog: choose the page size before starting.
pub struct NewDialog { pub chip: bool, pub width: f64, pub height: f64 }

/// What the New dialog asked for.
pub enum NewChoice { Create(f64, f64), Cancel }

pub fn new_dialog(ctx: &egui::Context, t: &Theme, d: &mut NewDialog, sizes: &mut PageSizes, inches: &mut bool) -> Option<NewChoice> {
    let mut choice = None;
    let title = if d.chip { "New chip pattern" } else { "New scroll pattern" };
    let range = if d.chip { 40.0..=600.0 } else { 40.0..=1000.0 };
    egui::Window::new(title).collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO).show(ctx, |ui| {
        ui.label(egui::RichText::new("Choose the size of the piece you will carve. You can change it later under Page.").small().color(t.dim));
        ui.add_space(6.0);
        if let Some((w, h)) = size_editor(ui, t, sizes, inches, d.width, d.height, range, false) { d.width = w; d.height = h; }
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            if ui.button("Create").clicked() { choice = Some(NewChoice::Create(d.width, d.height)); }
            if ui.button("Cancel").clicked() { choice = Some(NewChoice::Cancel); }
        });
    });
    choice
}
