//! The Chip workspace: generate a chip-carving medallion, edit it on the
//! millimetre grid, keep presets and export the SVG at actual size.
use super::*;
use crate::platform;
use crate::presets::Library;
use scroll_core::facets::{shade, BorderStyle, Centre, Chip, FillEdge, Repeat};
use scroll_core::boxes::{BoxDesign, Face, DIMENSIONS};
use scroll_core::chip::{FillLayout, apply_edits, chip_facets, chip_handles, chip_regions, generated_with_starts, ChipFill, Faceted, chip_svg, move_chip_handle, valid_chip, ChipFamily, ChipSettings};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ChipTab { Generate, Presets, Theme }

/// How a fill area is drawn: freehand, corner by corner, or a regular polygon dragged from its centre.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FillShape { Lasso, Polygon, Regular }

enum ChipDrag { Pan, Lasso, Move { id: usize, start: Point, points: Vec<Point> }, Handle { id: usize, vertex: usize, points: Vec<Point> } }

/// A box being built: its design, the face being edited, and the 3D view.
pub struct BoxState { pub design: BoxDesign, pub face: Face, pub view3d: bool, pub from_back: bool }

pub struct ChipState {
    pub settings: ChipSettings,
    pub path: Option<PathBuf>,
    past: Vec<ChipSettings>,
    future: Vec<ChipSettings>,
    pub selected: Option<usize>,
    pub show_grid: bool,
    pub tab: ChipTab,
    drag: Option<ChipDrag>,
    drag_before: Option<ChipSettings>,
    // cached geometry for the current settings
    cached_for: Option<ChipSettings>,
    regions: Vec<Vec<Point>>,
    triangles: Vec<Vec<[usize; 3]>>,
    /// Lit preview: each chip's facets (plan-view corners) and their brightness.
    lit: Vec<Vec<([Point; 3], u8)>>,
    /// Faceted engine: each chip's facet lines (empty for classic chips).
    facet_lines: Vec<Vec<Vec<Point>>>,
    /// Generated chips (no hand edits) and where each fill starts, cached by the
    /// settings they came from minus edits, so dragging a chip doesn't redo the fills.
    gen_for: Option<ChipSettings>,
    gen: Vec<Chip>,
    starts: Vec<usize>,
    /// Lasso tool: Some while active; holds the outline being drawn.
    pub lasso: Option<Vec<Point>>,
    /// Pattern, cell size and edge for the next lasso fill.
    fill_pattern: Repeat,
    fill_cell: f64,
    fill_edge: FillEdge,
    fill_shape: FillShape,
    fill_layout: FillLayout,
    fill_edge_row: bool,
    /// Sides of a regular fill area.
    fill_sides: u32,
    /// The document as last autosaved (the box, or the single pattern).
    autosaved: Option<String>,
    pub message: String,
    // view
    zoom: f32,
    origin: Pos2,
    fitted: bool,
    pub cursor_mm: Option<Point>,
    pub presets: Library,
    thumbs: Vec<Option<Vec<Vec<Point>>>>,
    /// A box being built from panels; None while editing a single pattern.
    pub boxd: Option<BoxState>,
    /// The 3D view's lit faces, cached by the box design they came from.
    iso_cache: Option<(BoxDesign, Vec<Vec<([Point; 3], u8)>>)>,
    /// "Make a box": which face this panel becomes, and the remaining dimension.
    box_face: Face,
    box_third: f64,
}

const AUTOSAVE: &str = "chip-current.json";

impl ChipState {
    pub fn new() -> ChipState {
        // the current pattern, or the box being built, as last kept on this computer
        let text = platform::store_read(AUTOSAVE);
        let boxd = text.as_deref().and_then(|t| io::parse_box(t).ok()).map(|design| BoxState { design, face: Face::Lid, view3d: false, from_back: false });
        let settings = match &boxd { Some(b) => b.design.lid.clone(), None => text.as_deref().and_then(|t| io::parse_chip(t).ok()).unwrap_or_default() };
        ChipState { autosaved: text, boxd, iso_cache: None, box_face: Face::Lid, box_third: 60.0, settings, path: None, past: vec![], future: vec![], selected: None, show_grid: true, tab: ChipTab::Generate, drag: None, drag_before: None,
            cached_for: None, regions: vec![], triangles: vec![], lit: vec![], facet_lines: vec![], gen_for: None, gen: vec![], starts: vec![], lasso: None, fill_pattern: Repeat::StarsAndDiamonds, fill_cell: 12.0, fill_edge: FillEdge::Clip, fill_shape: FillShape::Lasso, fill_sides: 6, fill_layout: FillLayout::Flow, fill_edge_row: true, message: String::new(), zoom: 4.0, origin: Pos2::ZERO, fitted: false, cursor_mm: None, presets: Library::load("chip-presets.json"), thumbs: vec![] }
    }
    fn change(&mut self, next: ChipSettings) {
        if next == self.settings { return; }
        self.past.push(std::mem::replace(&mut self.settings, next));
        if self.past.len() > 60 { self.past.remove(0); }
        self.future.clear();
    }
    pub fn undo(&mut self) { if let Some(p) = self.past.pop() { self.future.push(std::mem::replace(&mut self.settings, p)); self.selected = None; } }
    pub fn redo(&mut self) { if let Some(n) = self.future.pop() { self.past.push(std::mem::replace(&mut self.settings, n)); self.selected = None; } }
    pub fn can_undo(&self) -> bool { !self.past.is_empty() }
    pub fn can_redo(&self) -> bool { !self.future.is_empty() }
    fn refresh(&mut self) {
        // in a box, the pattern being edited is the current face's panel
        if let Some(b) = self.boxd.as_mut() { if b.design.panel(b.face) != Some(&self.settings) { b.design.set_panel(b.face, self.settings.clone()); } }
        if self.cached_for.as_ref() == Some(&self.settings) { return; }
        let facets = if self.settings.faceted.is_some() || !self.settings.fills.is_empty() {
            let key = ChipSettings { edits: Default::default(), removed: vec![], ..self.settings.clone() };
            if self.gen_for.as_ref() != Some(&key) { let (g, s) = generated_with_starts(&key); self.gen = g; self.starts = s; self.gen_for = Some(key); }
            apply_edits(&self.settings, self.gen.clone())
        } else { self.starts.clear(); chip_facets(&self.settings) };
        self.regions = if self.starts.is_empty() && self.settings.faceted.is_none() { chip_regions(&self.settings) } else { facets.iter().map(|c| c.outline.clone()).collect() };
        self.triangles = self.regions.iter().map(|p| triangulate(p)).collect();
        self.lit = facets.iter().map(|c| c.lit_triangles(3.5)).collect();
        // facet lines for faceted chips and fill chips; classic chips stay as they were drawn
        let first_faceted = if self.settings.faceted.is_some() { 0 } else { self.starts.first().copied().unwrap_or(usize::MAX) };
        self.facet_lines = facets.iter().enumerate().map(|(i, c)| if i >= first_faceted { c.facet_lines() } else { vec![] }).collect();
        self.cached_for = Some(self.settings.clone());
        if self.selected.is_some_and(|i| i >= self.regions.len() || self.settings.removed.contains(&i)) { self.selected = None; }
    }
    /// Keep the current pattern on this computer, as the web version did.
    fn autosave(&mut self) {
        if self.drag.is_some() { return; }
        let text = self.document();
        if self.autosaved.as_ref() == Some(&text) { return; }
        let _ = platform::store_write(AUTOSAVE, &text);
        self.autosaved = Some(text);
    }
    /// What Save writes: the box with all its panels, or the single pattern.
    fn document(&self) -> String { match &self.boxd { Some(b) => io::save_box(&b.design), None => io::save_chip(&self.settings) } }

    // ---------- boxes ----------
    /// Build a box from the pattern being edited: it becomes `box_face` (lid or front)
    /// and the other faces start matched to it.
    pub fn make_box(&mut self) {
        let design = BoxDesign::from_panel(&self.settings, self.box_face, self.box_third);
        self.message = format!("Box {} × {} × {} mm. Pick a face to edit it; the 3D view shows them together.", design.length, design.width, design.height);
        self.boxd = Some(BoxState { design, face: self.box_face, view3d: true, from_back: false });
        self.path = None; self.past.clear(); self.future.clear();
    }
    /// Leave the box, keeping the face being edited as a single pattern.
    pub fn leave_box(&mut self) {
        if self.boxd.take().is_some() { self.path = None; self.past.clear(); self.future.clear(); self.message = "Left the box; this panel is now a single pattern.".into(); }
    }
    /// Edit another face. Undo history stays with each face, so it starts fresh here.
    pub fn show_face(&mut self, face: Face) {
        let Some(b) = self.boxd.as_mut() else { return };
        if face == Face::Bottom && b.design.bottom.is_none() { b.design.set_bottom(true); }
        b.face = face; b.view3d = false;
        let s = b.design.panel(face).cloned().unwrap();
        self.settings = s; self.past.clear(); self.future.clear(); self.selected = None; self.fitted = false; self.lasso = None;
    }
    /// Change the box's outer dimensions: every panel is regenerated at its new size.
    pub fn set_box_size(&mut self, l: f64, w: f64, h: f64) {
        let Some(b) = self.boxd.as_mut() else { return };
        b.design = b.design.resized(l, w, h);
        let s = b.design.panel(b.face).cloned().unwrap();
        self.settings = s; self.past.clear(); self.future.clear(); self.selected = None; self.fitted = false;
    }
    /// Carve the bottom, or leave it plain.
    pub fn set_box_bottom(&mut self, on: bool) {
        let Some(b) = self.boxd.as_mut() else { return };
        b.design.set_bottom(on);
        if !on && b.face == Face::Bottom { self.show_face(Face::Lid); }
    }
    /// Make the back follow the front again (or the right side the left).
    pub fn relink_face(&mut self) {
        let Some(b) = self.boxd.as_mut() else { return };
        let face = b.face; b.design.relink(face);
        let s = b.design.panel(face).cloned().unwrap();
        self.settings = s; self.past.clear(); self.future.clear(); self.selected = None;
    }
    /// Every panel on one sheet, at real size.
    pub fn export_sheet(&mut self) {
        let Some(b) = &self.boxd else { return };
        let svg = b.design.sheet_svg();
        if let Some(p) = platform::choose_save("SVG", &["svg"], "box-panels.svg") {
            match platform::write_file(&p, &svg) { Ok(()) => self.message = format!("Exported all panels to {}.", platform::shown(&p)), Err(e) => self.message = format!("Could not export: {e}") }
        }
    }
    pub fn toggle_lasso(&mut self) {
        if self.lasso.take().is_some() { self.message = "Fill tool put away.".into(); return; }
        self.lasso = Some(vec![]); self.selected = None;
        self.message = match self.fill_shape {
            FillShape::Lasso => "Lasso: drag round the area to fill. Esc puts it away.".into(),
            FillShape::Polygon => "Polygon: click each corner; click the first corner, double-click or press Enter to close. Esc cancels.".into(),
            FillShape::Regular => format!("Regular {}-sided shape: drag from its centre out to a corner. Esc cancels.", self.fill_sides),
        };
    }
    /// The outline being drawn, as it will be filled (a regular shape is made from its centre and a corner).
    fn drawn_outline(&self) -> Option<Vec<Point>> {
        let d = self.lasso.as_ref()?;
        if self.fill_shape != FillShape::Regular { return Some(d.clone()); }
        let (c, k) = (*d.first()?, *d.get(1)?);
        let (r, a0) = ((k.x - c.x).hypot(k.y - c.y), (k.y - c.y).atan2(k.x - c.x));
        let n = self.fill_sides.max(3);
        Some((0..n).map(|i| { let a = a0 + 2.0 * std::f64::consts::PI * i as f64 / n as f64; pt(c.x + r * a.cos(), c.y + r * a.sin()) }).collect())
    }
    /// Close the drawn outline and add it as a fill with the panel's pattern.
    fn finish_lasso(&mut self) {
        let Some(drawn) = self.drawn_outline() else { self.lasso = None; return };
        self.lasso = None;
        let (pw, ph) = (self.settings.width(), self.settings.page_height());
        let pts: Vec<Point> = drawn.into_iter().map(|p| pt(p.x.clamp(0.0, pw), p.y.clamp(0.0, ph))).collect();
        let area = (0..pts.len()).map(|i| { let (a, b) = (pts[i], pts[(i + 1) % pts.len()]); a.x * b.y - b.x * a.y }).sum::<f64>().abs() / 2.0;
        let min_points = if self.fill_shape == FillShape::Lasso { 6 } else { 3 };
        if pts.len() < min_points || area < 25.0 { self.message = "That shape was too small to fill. Press L and draw a larger one.".into(); return; }
        let mut fill = ChipFill::new(pts, self.fill_pattern, self.fill_cell); fill.edge = self.fill_edge;
        fill.layout = self.fill_layout; fill.edge_row = self.fill_edge_row;
        let mut s = self.settings.clone(); s.fills.push(fill);
        self.message = format!("Filled with {}. Change or delete it under Fill an area; Undo takes it back.", self.fill_pattern.label());
        self.change(s);
    }
    /// Change fill `i`, or delete it (None). Chips from that fill on are made
    /// afresh, so their hand edits and removals are dropped; earlier chips keep theirs.
    fn set_fill(&mut self, i: usize, new: Option<ChipFill>) {
        let start = self.starts.get(i).copied().unwrap_or(0);
        let mut s = self.settings.clone();
        match new { Some(f) => s.fills[i] = f, None => { s.fills.remove(i); } }
        s.edits.retain(|k, _| *k < start); s.removed.retain(|k| *k < start);
        self.change(s); self.selected = None;
    }
    pub fn chip_count(&self) -> usize { (0..self.regions.len()).filter(|i| !self.settings.removed.contains(i)).count() }

    pub fn new_pattern(&mut self) { self.boxd = None; let d = ChipSettings::default(); let s = ChipSettings { faceted: Some(Faceted::from_seed(d.seed())), ..d }; self.change(s); self.path = None; self.selected = None; self.fitted = false; self.message = "New chip pattern. Undo returns to the previous one.".into(); }
    /// A new faceted pattern on a `w` × `h` page (from the New dialog).
    pub fn new_pattern_sized(&mut self, w: f64, h: f64) {
        self.boxd = None; let d = ChipSettings::default();
        let s = ChipSettings { faceted: Some(Faceted::from_seed(d.seed())), ..d }.resized(w, h);
        self.change(s); self.path = None; self.selected = None; self.fitted = false;
        self.message = format!("New {} × {} mm chip pattern. Undo returns to the previous one.", w, h);
    }
    /// A picked file, once read (at once on the desktop, a moment later on the web).
    pub fn open_text(&mut self, picked: platform::Opened) {
        let (path, text) = match picked { Ok(p) => p, Err(e) => { self.message = format!("Could not open this file. {e}"); return } };
        // a box first, then a single pattern
        if let Ok(design) = io::parse_box(&text) {
            let s = design.lid.clone();
            self.boxd = Some(BoxState { design, face: Face::Lid, view3d: true, from_back: false });
            self.settings = s; self.past.clear(); self.future.clear();
            self.path = Some(path); self.selected = None; self.fitted = false; self.message = "Box opened.".into();
            return;
        }
        match io::parse_chip(&text) {
            Ok(s) => { self.boxd = None; self.change(s); self.path = Some(path); self.selected = None; self.fitted = false; self.message = "Chip layout opened.".into(); }
            Err(e) => self.message = format!("Could not open this chip layout. {e}"),
        }
    }
    pub fn save(&mut self, choose: bool) {
        let boxed = self.boxd.is_some();
        let path = if choose || self.path.is_none() {
            let name = if boxed { "box.ornatr" } else { "chip-layout.ornatr" };
            match platform::choose_save(if boxed { "ORNATR box" } else { "ORNATR chip layout" }, &["ornatr"], name) { Some(p) => p, None => return }
        } else { self.path.clone().unwrap() };
        match platform::write_file(&path, &self.document()) { Ok(()) => { self.path = Some(path); self.message = if boxed { "Box saved.".into() } else { "Chip layout saved.".into() }; } Err(e) => self.message = format!("Could not save: {e}") }
    }
    pub fn export(&mut self) {
        if let Some(p) = platform::choose_save("SVG", &["svg"], "chip-pattern.svg") {
            match platform::write_file(&p, &chip_svg(&self.settings)) { Ok(()) => self.message = format!("Exported {}.", platform::shown(&p)), Err(e) => self.message = format!("Could not export: {e}") }
        }
    }
    pub fn remove_selected(&mut self) { if let Some(i) = self.selected { let mut s = self.settings.clone(); s.removed.push(i); self.change(s); self.selected = None; } }

    fn to_screen(&self, p: Point) -> Pos2 { Pos2::new(self.origin.x + p.x as f32 * self.zoom, self.origin.y + p.y as f32 * self.zoom) }
    fn to_mm(&self, s: Pos2) -> Point { pt(((s.x - self.origin.x) / self.zoom) as f64, ((s.y - self.origin.y) / self.zoom) as f64) }
    fn fit(&mut self, rect: Rect) {
        let (w, h) = (self.settings.width() as f32, self.settings.page_height() as f32);
        self.zoom = ((rect.width() - 80.0) / w).min((rect.height() - 80.0) / h).max(0.5);
        self.origin = Pos2::new(rect.center().x - w * self.zoom / 2.0, rect.center().y - h * self.zoom / 2.0);
        self.fitted = true;
    }
    pub fn fit_page(&mut self) { self.fitted = false; }
    pub fn title(&self) -> String {
        let name = self.path.as_ref().and_then(|p| p.file_stem()).map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| if self.boxd.is_some() { "Box".into() } else { "Chip pattern".into() });
        match &self.boxd { Some(b) => format!("{name} · {} — ORNATR", b.face.label()), None => format!("{name} — ORNATR") }
    }
}

impl App {
    pub(crate) fn chip_status(&mut self, ctx: &egui::Context) {
        let t = self.t();
        let c = &self.chip;
        let text = format!("{} × {} mm   ·   {} mm grid   ·   {} chips", c.settings.width(), c.settings.page_height(), c.settings.step(), c.chip_count());
        egui::TopBottomPanel::bottom("chip-status").frame(egui::Frame::none().fill(t.bg).inner_margin(egui::Margin::symmetric(14.0, 5.0))).show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.visuals_mut().override_text_color = Some(t.dim);
                ui.style_mut().override_text_style = Some(egui::TextStyle::Small);
                ui.label(text);
                if let Some(m) = self.chip.cursor_mm { ui.separator(); ui.label(format!("x {:.1}  y {:.1} mm", m.x, m.y)); }
                ui.separator();
                let hint = match self.chip.selected { None => "Click a chip to select it. Drag chips and handles; edits snap to the grid.".to_string(), Some(i) => format!("Chip {} selected · drag it or its handles · Delete removes it", i + 1) };
                ui.label(if self.chip.message.is_empty() { hint } else { self.chip.message.clone() });
            });
        });
    }

    /// File, Edit and View menus while the Chip workspace is active.
    pub(crate) fn chip_menus(&mut self, ui: &mut egui::Ui) {
        ui.menu_button("File", |ui| {
            if ui.add(egui::Button::new("New chip pattern…").shortcut_text("Ctrl+N")).clicked() { self.open_new_dialog(); ui.close_menu(); }
            if ui.add(egui::Button::new("Open chip layout…").shortcut_text("Ctrl+O")).clicked() { ui.close_menu(); self.open_chip(ui.ctx()); }
            if ui.add(egui::Button::new("Save chip layout").shortcut_text("Ctrl+S")).clicked() { ui.close_menu(); self.chip.save(false); }
            if ui.add(egui::Button::new("Save chip layout as…").shortcut_text("Ctrl+Shift+S")).clicked() { ui.close_menu(); self.chip.save(true); }
            ui.separator();
            if ui.button("Export chip SVG…").clicked() { ui.close_menu(); self.chip.export(); }
        });
        ui.menu_button("Edit", |ui| {
            if ui.add_enabled(self.chip.can_undo(), egui::Button::new("Undo").shortcut_text("Ctrl+Z")).clicked() { self.chip.undo(); ui.close_menu(); }
            if ui.add_enabled(self.chip.can_redo(), egui::Button::new("Redo").shortcut_text("Ctrl+Y")).clicked() { self.chip.redo(); ui.close_menu(); }
            ui.separator();
            if ui.add_enabled(self.chip.selected.is_some(), egui::Button::new("Remove chip").shortcut_text("Del")).clicked() { self.chip.remove_selected(); ui.close_menu(); }
        });
        ui.menu_button("View", |ui| {
            if ui.button("Fit page").clicked() { self.chip.fit_page(); ui.close_menu(); }
            ui.checkbox(&mut self.chip.show_grid, "Millimetre grid");
            let mut lit = self.prefs.chip_lit;
            if ui.checkbox(&mut lit, "Lit preview").on_hover_text("Shows the cut wood under raking light from the upper left").changed() { self.set_prefs(Prefs { chip_lit: lit, ..self.prefs }); }
            ui.separator();
            ui.menu_button("Theme", |ui| {
                for id in ThemeId::ALL { if ui.selectable_label(self.prefs.theme == id, id.theme().name).clicked() { self.set_prefs(Prefs { theme: id, ..self.prefs }); ui.close_menu(); } }
            });
        });
    }

    pub(crate) fn chip_side_panel(&mut self, ctx: &egui::Context) {
        let t = self.t();
        egui::SidePanel::right("chip-panel").default_width(340.0).min_width(300.0).frame(egui::Frame::none().fill(t.bg).inner_margin(egui::Margin { left: 16.0, right: 12.0, top: 12.0, bottom: 8.0 })).show(ctx, |ui| {
            let tabs = [(ChipTab::Generate, "Generate"), (ChipTab::Presets, "Presets"), (ChipTab::Theme, "Theme")];
            let mut tab = self.chip.tab;
            segmented(ui, t, &tabs, &mut tab);
            self.chip.tab = tab;
            ui.add_space(4.0);
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                ui.set_width(ui.available_width() - 4.0);
                match self.chip.tab { ChipTab::Generate => self.chip_generate(ui), ChipTab::Presets => self.chip_presets(ui), ChipTab::Theme => self.theme_tab(ui) }
                ui.add_space(12.0);
            });
        });
    }

    fn chip_generate(&mut self, ui: &mut egui::Ui) {
        let t = self.t();
        let s = self.chip.settings.clone();
        self.chip_box(ui);
        section(ui, t, "Composition");
        let mut faceted = s.faceted.is_some();
        segmented(ui, t, &[(true, "Faceted"), (false, "Classic")], &mut faceted);
        if faceted != s.faceted.is_some() {
            let f = if faceted { Some(Faceted::from_seed(s.seed())) } else { None };
            self.chip.change(ChipSettings { faceted: f, ..s.regenerated() }); self.chip.selected = None;
        } else if let Some(f) = s.faceted { self.faceted_controls(ui, &s, f); } else { self.classic_controls(ui, &s); }

        self.chip_fills(ui);
        self.chip_page_and_edit(ui);
    }

    /// Boxes: make one from this pattern, or, inside a box, its size, faces, links, bottom and 3D view.
    fn chip_box(&mut self, ui: &mut egui::Ui) {
        let t = self.t();
        section(ui, t, "Box");
        let st = &mut self.chip;
        let Some(b) = st.boxd.as_ref() else {
            ui.label(egui::RichText::new("Turn this pattern into one face of a carved box. The other faces start matched to it, and each stays a pattern you can change.").small().color(t.dim));
            egui::Grid::new("chip-make-box").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
                ui.label("This panel is the");
                segmented(ui, t, &[(Face::Lid, "Lid"), (Face::Front, "Front")], &mut st.box_face); ui.end_row();
                ui.label(if st.box_face == Face::Lid { "Box height" } else { "Box depth" });
                ui.add(egui::DragValue::new(&mut st.box_third).range(DIMENSIONS).speed(0.5).fixed_decimals(0).suffix(" mm")); ui.end_row();
            });
            if ui.button("Make a box from this panel").clicked() { st.make_box(); }
            return;
        };
        let (face, linked, view3d, from_back) = (b.face, b.design.linked(b.face), b.view3d, b.from_back);
        let (mut l, mut w, mut h) = (b.design.length, b.design.width, b.design.height);
        let mut bottom = b.design.bottom.is_some();
        egui::Grid::new("chip-box-size").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
            ui.label("Length"); ui.add(egui::DragValue::new(&mut l).range(DIMENSIONS).speed(0.5).fixed_decimals(0).suffix(" mm")); ui.end_row();
            ui.label("Width"); ui.add(egui::DragValue::new(&mut w).range(DIMENSIONS).speed(0.5).fixed_decimals(0).suffix(" mm")); ui.end_row();
            ui.label("Height"); ui.add(egui::DragValue::new(&mut h).range(DIMENSIONS).speed(0.5).fixed_decimals(0).suffix(" mm")); ui.end_row();
        });
        if (l, w, h) != (b.design.length, b.design.width, b.design.height) { st.set_box_size(l, w, h); }
        ui.label(egui::RichText::new("Changing the size regenerates every panel (hand edits are cleared).").small().color(t.dim));
        ui.add_space(4.0);
        // the faces: click one to edit it
        let faces: Vec<Face> = Face::ALL.into_iter().filter(|f| *f != Face::Bottom || bottom).collect();
        ui.horizontal_wrapped(|ui| {
            for f in faces { if ui.add(egui::Button::new(f.label()).selected(face == f && !view3d)).clicked() { st.show_face(f); } }
        });
        if let Some(p) = face.partner() {
            if linked { ui.label(egui::RichText::new(format!("Same pattern as the {}. Changing it gives the {} its own.", p.label().to_lowercase(), face.label().to_lowercase())).small().color(t.dim)); }
            else if ui.button(format!("Match the {} again", p.label().to_lowercase())).clicked() { st.relink_face(); }
        }
        if ui.checkbox(&mut bottom, "Carve the bottom too").changed() { st.set_box_bottom(bottom); }
        ui.horizontal(|ui| {
            let mut v = view3d; let mut back = from_back;
            if ui.checkbox(&mut v, "3D view").on_hover_text("The box assembled, lit from the upper left").changed() { if let Some(b) = st.boxd.as_mut() { b.view3d = v; } }
            if v && ui.checkbox(&mut back, "From the back-left").changed() { if let Some(b) = st.boxd.as_mut() { b.from_back = back; } }
        });
        if ui.button("Leave the box (keep this panel)").on_hover_text("Go back to a single pattern; the box stays in its saved file").clicked() { st.leave_box(); }
    }

    /// Lasso fills: the pattern for the next one, the lasso button, and each fill's settings.
    fn chip_fills(&mut self, ui: &mut egui::Ui) {
        let t = self.t();
        section(ui, t, "Fill an area");
        ui.label(egui::RichText::new("Draw an area: freehand, corner by corner, or a regular shape. It fills with a repeat, keeping clear of the chips already there.").small().color(t.dim));
        let st = &mut self.chip;
        egui::Grid::new("chip-fill-new").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
            ui.label("Pattern");
            egui::ComboBox::from_id_salt("chip-fill-pattern").width(170.0).selected_text(st.fill_pattern.label()).show_ui(ui, |ui| {
                for r in Repeat::ALL { ui.selectable_value(&mut st.fill_pattern, r, r.label()); }
            });
            ui.end_row();
            ui.label("Cell size"); ui.add(egui::DragValue::new(&mut st.fill_cell).range(ChipFill::CELLS).speed(0.2).fixed_decimals(0).suffix(" mm")); ui.end_row();
            ui.label("Edge"); segmented(ui, t, &[(FillEdge::Clip, "Clip"), (FillEdge::Whole, "Whole chips")], &mut st.fill_edge); ui.end_row();
            ui.label("Layout").on_hover_text("Flow follows the composition: a band round the centre motif and fans in the corners. Grid is a plain square grid."); segmented(ui, t, &[(FillLayout::Flow, "Flow"), (FillLayout::Grid, "Grid")], &mut st.fill_layout); ui.end_row();
            if st.fill_layout == FillLayout::Flow { ui.label(""); ui.checkbox(&mut st.fill_edge_row, "Edge row round the centre"); ui.end_row(); }
            ui.label("Shape");
            let before = st.fill_shape;
            segmented(ui, t, &[(FillShape::Lasso, "Lasso"), (FillShape::Polygon, "Polygon"), (FillShape::Regular, "Regular")], &mut st.fill_shape);
            if st.fill_shape != before && st.lasso.is_some() { st.lasso = None; st.toggle_lasso(); } // restart the tool in the new shape
            ui.end_row();
            if st.fill_shape == FillShape::Regular { ui.label("Sides"); ui.add(egui::DragValue::new(&mut st.fill_sides).range(3..=24).speed(0.1)); ui.end_row(); }
        });
        let active = st.lasso.is_some();
        let label = match st.fill_shape { FillShape::Lasso => "Lasso fill (L)", FillShape::Polygon => "Draw polygon fill (L)", FillShape::Regular => "Draw regular fill (L)" };
        if ui.add(egui::Button::new(if active { "Put the tool away (Esc)" } else { label }).selected(active)).clicked() { st.toggle_lasso(); }

        let fills = st.settings.fills.clone();
        let mut change: Option<(usize, Option<ChipFill>)> = None;
        for (i, f) in fills.iter().enumerate() {
            ui.add_space(6.0);
            ui.push_id(("chip-fill", i), |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(format!("Fill {}", i + 1)).color(t.text));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| { if ui.button("Delete").clicked() { change = Some((i, None)); } });
                });
                let mut n = f.clone();
                egui::Grid::new("chip-fill-edit").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
                    ui.label("Pattern");
                    egui::ComboBox::from_id_salt("pattern").width(170.0).selected_text(n.pattern.label()).show_ui(ui, |ui| {
                        for r in Repeat::ALL { ui.selectable_value(&mut n.pattern, r, r.label()); }
                    });
                    ui.end_row();
                    ui.label("Cell size"); ui.add(egui::DragValue::new(&mut n.cell).range(ChipFill::CELLS).speed(0.2).fixed_decimals(0).suffix(" mm")); ui.end_row();
                    ui.label("Margin").on_hover_text("Uncut wood kept inside the outline and round the chips already there");
                    ui.add(egui::DragValue::new(&mut n.margin).range(ChipFill::MARGINS).speed(0.05).fixed_decimals(1).suffix(" mm")); ui.end_row();
                    ui.label("Edge"); segmented(ui, t, &[(FillEdge::Clip, "Clip"), (FillEdge::Whole, "Whole chips")], &mut n.edge); ui.end_row();
                    ui.label("Layout"); segmented(ui, t, &[(FillLayout::Flow, "Flow"), (FillLayout::Grid, "Grid")], &mut n.layout); ui.end_row();
                    if n.layout == FillLayout::Flow { ui.label(""); ui.checkbox(&mut n.edge_row, "Edge row round the centre"); ui.end_row(); }
                });
                if n != *f && change.is_none() { change = Some((i, Some(n))); }
            });
        }
        if let Some((i, n)) = change { self.chip.set_fill(i, n); }
    }

    /// Centre, count and border for the faceted engine.
    fn faceted_controls(&mut self, ui: &mut egui::Ui, s: &ChipSettings, f: Faceted) {
        let t = self.t();
        ui.label(egui::RichText::new("A rosette, drawn with the facet lines you carve to, inside an optional border. Edit afterwards on the grid.").small().color(t.dim));
        let mut n = f;
        egui::Grid::new("chip-facets").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
            ui.label("Inside");
            let inside_label = match n.field { None => "Rosette".to_string(), Some(r) => format!("Field: {}", r.label()) };
            egui::ComboBox::from_id_salt("chip-inside").width(170.0).selected_text(inside_label).show_ui(ui, |ui| {
                ui.selectable_value(&mut n.field, None, "Rosette");
                for r in Repeat::ALL { ui.selectable_value(&mut n.field, Some(r), format!("Field: {}", r.label())); }
            }).response.on_hover_text("A rosette in the middle, or a field of a repeat filling the inside of the border (as on box sides)");
            ui.end_row();
            if n.field.is_none() {
                ui.label("Centre");
                egui::ComboBox::from_id_salt("chip-centre").width(170.0).selected_text(n.centre.label()).show_ui(ui, |ui| {
                    for c in Centre::ALL { if ui.selectable_label(n.centre == c, c.label()).clicked() && n.centre != c { n.centre = c; n.count = c.default_count(); } }
                });
                ui.end_row();
                ui.label(n.centre.count_label());
                ui.add(egui::DragValue::new(&mut n.count).range(n.centre.counts()).speed(0.1));
                ui.end_row();
            }
            ui.label("Border");
            egui::ComboBox::from_id_salt("chip-facet-border").width(170.0).selected_text(n.border.map_or("None", |b| b.label())).show_ui(ui, |ui| {
                ui.selectable_value(&mut n.border, None, "None");
                for b in BorderStyle::ALL { ui.selectable_value(&mut n.border, Some(b), b.label()); }
            });
            ui.end_row();
            if n.field.is_some() { ui.label("Cell size").on_hover_text("Smaller makes more, finer cells in the field"); } else { ui.label("Centre size").on_hover_text("Smaller leaves room round the centre for a fill"); }
            let mut pct = n.scale * 100.0;
            if ui.add(egui::DragValue::new(&mut pct).range(40.0..=100.0).speed(0.5).fixed_decimals(0).suffix(" %")).changed() { n.scale = (pct / 100.0).clamp(0.4, 1.0); }
            ui.end_row();
        });
        if n != f { self.chip.change(ChipSettings { faceted: Some(n), removed: vec![], edits: Default::default(), ..s.clone() }); self.chip.selected = None; }
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(format!("Variation {}", s.seed())).color(t.text));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Generate variation").on_hover_text("Picks another centre, count and border").clicked() {
                    // step until the permutation actually changes
                    let mut seed = s.seed();
                    let next = loop { seed = seed.wrapping_add(1); let p = Faceted::from_seed(seed); if p != f { break p; } };
                    self.chip.change(ChipSettings { seed: Some(seed), faceted: Some(next), ..s.regenerated() }); self.chip.selected = None;
                }
            });
        });
        ui.label(egui::RichText::new("Changing the composition clears hand edits; Undo (Ctrl+Z) brings them back.").small().color(t.dim));
    }

    /// The classic generator's controls, unchanged.
    fn classic_controls(&mut self, ui: &mut egui::Ui, s: &ChipSettings) {
        let t = self.t();
        let s = s.clone();
        ui.label(egui::RichText::new("One seed chooses a centre, a border rhythm and the corner treatment. Edit afterwards on the grid.").small().color(t.dim));
        let mut fam = s.family;
        egui::ComboBox::from_id_salt("chip-family").width(ui.available_width()).selected_text(fam.label()).show_ui(ui, |ui| {
            for f in [ChipFamily::Star, ChipFamily::Rosette, ChipFamily::Border] { ui.selectable_value(&mut fam, f, f.label()); }
        });
        if fam != s.family { self.chip.change(ChipSettings { family: fam, ..s.regenerated() }); self.chip.selected = None; }
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(format!("Variation {}", s.seed())).color(t.text));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Generate variation").clicked() { let n = ChipSettings { seed: Some(s.seed().wrapping_add(1)), ..s.regenerated() }; self.chip.change(n); self.chip.selected = None; }
            });
        });
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(format!("Border: {}", s.border_name())).color(t.text));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Vary border").on_hover_text("Keeps the centre and changes only the border").clicked() { let n = ChipSettings { border_seed: Some(s.border_seed().wrapping_add(1)), ..s.regenerated() }; self.chip.change(n); self.chip.selected = None; }
            });
        });
        ui.label(egui::RichText::new("Generating clears hand edits; Undo (Ctrl+Z) brings them back.").small().color(t.dim));
    }

    fn chip_page_and_edit(&mut self, ui: &mut egui::Ui) {
        let t = self.t();
        let s = self.chip.settings.clone();
        section(ui, t, "Page");
        // classic patterns are square; a faceted page can have any proportions
        let mut inches = self.prefs.inches;
        // classic patterns are square; a faceted page can have any proportions; a box panel is sized by the box
        let picked = if self.chip.boxd.is_some() { ui.label(egui::RichText::new(format!("{} × {} mm, set by the box (change its size under Box).", s.width(), s.page_height())).small().color(t.dim)); None }
            else { page_sizes::size_editor(ui, t, &mut self.page_sizes, &mut inches, s.width(), s.page_height(), 40.0..=600.0, s.faceted.is_none()) };
        if inches != self.prefs.inches { self.set_prefs(Prefs { inches, ..self.prefs }); }
        ui.add_space(4.0);
        let max_grid = 30f64.min(s.width().min(s.page_height()) / 6.0).floor();
        let mut step = s.step();
        egui::Grid::new("chip-dims").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
            ui.label("Grid interval"); ui.add(egui::DragValue::new(&mut step).range(2.0..=max_grid).speed(0.1).fixed_decimals(0).suffix(" mm")); ui.end_row();
        });
        // Changing the grid or page regenerates the pattern at the new size.
        if step != s.step() { let mut n = s.regenerated(); n.grid = Some(step); self.chip.change(n); self.chip.selected = None; }
        else if let Some((w, h)) = picked {
            if w != s.width() || h != s.page_height() { let n = s.resized(w, h); self.chip.change(n); self.chip.selected = None; self.chip.fitted = false; }
        }
        ui.checkbox(&mut self.chip.show_grid, "Show millimetre grid");
        ui.label(egui::RichText::new("The grid sets the construction size and snaps edits. It never appears in the export.").small().color(t.dim));

        section(ui, t, "Edit");
        ui.label(egui::RichText::new("Drag a chip to move it. Curved pieces have Start, Middle and End handles that bend the whole curve; straight pieces have one handle per corner.").small().color(t.dim));
        ui.horizontal(|ui| {
            if ui.add_enabled(self.chip.selected.is_some(), egui::Button::new("Remove chip")).clicked() { self.chip.remove_selected(); }
            if ui.add_enabled(!s.removed.is_empty(), egui::Button::new("Restore removed")).clicked() { self.chip.change(ChipSettings { removed: vec![], ..s.clone() }); }
        });
        ui.horizontal(|ui| {
            if ui.add_enabled(self.chip.can_undo(), egui::Button::new("Undo")).clicked() { self.chip.undo(); }
            if ui.add_enabled(self.chip.can_redo(), egui::Button::new("Redo")).clicked() { self.chip.redo(); }
        });

        section(ui, t, "Export");
        if ui.button(if self.chip.boxd.is_some() { "Export this panel SVG…" } else { "Export chip SVG…" }).clicked() { self.chip.export(); }
        if self.chip.boxd.is_some() && ui.button("Export all panels on one sheet…").clicked() { self.chip.export_sheet(); }
        ui.label(egui::RichText::new("Actual size, retained outlines only. Your current pattern is also kept automatically on this computer.").small().color(t.dim));
    }

    fn chip_presets(&mut self, ui: &mut egui::Ui) {
        let t = self.t();
        let data = serde_json::from_str(&io::save_chip(&self.chip.settings)).unwrap();
        let mut load: Option<ChipSettings> = None;
        // thumbnails, built once per entry
        let lib = &self.chip.presets;
        if self.chip.thumbs.len() != lib.entries.len() { self.chip.thumbs = vec![None; lib.entries.len()]; }
        for (i, e) in lib.entries.iter().enumerate() {
            if self.chip.thumbs[i].is_none() { self.chip.thumbs[i] = Some(io::parse_chip(&e.data.to_string()).map(|s| { let r = chip_regions(&s); r.into_iter().enumerate().filter(|(k, _)| !s.removed.contains(k)).map(|(_, p)| { let mut v: Vec<Point> = p.iter().map(|q| pt(q.x / s.size, q.y / s.size)).collect(); if let Some(f) = v.first().copied() { v.push(f); } v }).collect() }).unwrap_or_default()); }
        }
        let thumbs = self.chip.thumbs.clone();
        presets_panel(ui, t, &mut self.chip.presets, &thumbs, data, "Chip", |d| { if let Ok(s) = io::parse_chip(&d.to_string()) { load = Some(s); } });
        if let Some(s) = load { self.chip.change(s); self.chip.selected = None; self.chip.fitted = false; self.chip.message = "Preset loaded. Undo returns to your previous pattern.".into(); }
        if self.chip.thumbs.len() != self.chip.presets.entries.len() { self.chip.thumbs.clear(); }
    }

    pub(crate) fn chip_canvas(&mut self, ui: &mut egui::Ui) {
        self.chip.refresh();
        self.chip.autosave();
        let cc = self.canvas_colors();
        let lit = self.prefs.chip_lit;
        let (resp, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
        let rect = resp.rect;
        let st = &mut self.chip;
        if st.boxd.as_ref().is_some_and(|b| b.view3d) { draw_box_3d(st, &painter, rect, &cc); return; }
        if !st.fitted && rect.width() > 120.0 && rect.height() > 120.0 { st.fit(rect); }
        if let Some(hover) = resp.hover_pos() {
            let (scroll, zoom_delta) = ui.input(|i| (i.smooth_scroll_delta.y, i.zoom_delta()));
            let factor = if zoom_delta != 1.0 { zoom_delta } else { (scroll * 0.0015).exp() };
            if (factor - 1.0).abs() > 1e-4 { let before = st.to_mm(hover); st.zoom = (st.zoom * factor).clamp(0.5, 80.0); let after = st.to_screen(before); st.origin += hover - after; }
            st.cursor_mm = Some(st.to_mm(hover));
        }
        let (size, size_h) = (st.settings.width(), st.settings.page_height()); let step = st.settings.step();
        let page = Rect::from_min_max(st.to_screen(pt(0.0, 0.0)), st.to_screen(pt(size, size_h)));
        for (grow, alpha) in [(10.0, 10u8), (5.0, 16), (2.0, 24)] { painter.rect_filled(page.expand(grow).translate(Vec2::new(0.0, grow * 0.4)), grow + 2.0, Color32::from_black_alpha(alpha)); }
        painter.rect_filled(page, 2.0, if lit { wood(shade(&[(0.0, 0.0, 0.0), (1.0, 0.0, 0.0), (0.0, 1.0, 0.0)])) } else { cc.paper });
        if st.show_grid && step * st.zoom as f64 > 3.0 {
            // on the lit wood the grid is a faint darker line, so it doesn't fight the facets
            let grid_col = if lit { Color32::from_black_alpha(28) } else { cc.grid };
            let mut v = 0.0; let mut k = 0;
            while v <= size.max(size_h) + 1e-9 {
                let w = if k % 5 == 0 { 0.9 } else { 0.5 };
                if v <= size + 1e-9 { let a = st.to_screen(pt(v, 0.0)); painter.line_segment([Pos2::new(a.x, page.top()), Pos2::new(a.x, page.bottom())], Stroke::new(w, grid_col)); }
                if v <= size_h + 1e-9 { let b = st.to_screen(pt(0.0, v)); painter.line_segment([Pos2::new(page.left(), b.y), Pos2::new(page.right(), b.y)], Stroke::new(w, grid_col)); }
                v += step; k += 1;
            }
        }
        let stroke_w = (0.25 * st.zoom).clamp(1.0, 2.2);
        let fill = with_alpha(cc.ink, 26); let fill_sel = with_alpha(cc.mark, 70);
        for (i, poly) in st.regions.iter().enumerate() {
            if st.settings.removed.contains(&i) { continue; }
            let pts: Vec<Pos2> = poly.iter().map(|p| st.to_screen(*p)).collect();
            if lit {
                // raking light: each facet in its own tone; only the selection gets an outline
                let mut mesh = egui::Mesh::default();
                for (tri, k) in &st.lit[i] {
                    let base = mesh.vertices.len() as u32; let col = wood(*k);
                    for p in tri { mesh.colored_vertex(st.to_screen(*p), col); }
                    mesh.add_triangle(base, base + 1, base + 2);
                }
                painter.add(Shape::mesh(mesh));
                if st.selected == Some(i) { painter.add(Shape::closed_line(pts, Stroke::new(stroke_w + 0.5, cc.mark))); }
                continue;
            }
            let mut mesh = egui::Mesh::default();
            let col = if st.selected == Some(i) { fill_sel } else { fill };
            for p in &pts { mesh.colored_vertex(*p, col); }
            for tri in &st.triangles[i] { mesh.add_triangle(tri[0] as u32, tri[1] as u32, tri[2] as u32); }
            painter.add(Shape::mesh(mesh));
            painter.add(Shape::closed_line(pts, Stroke::new(stroke_w, cc.ink)));
            for line in &st.facet_lines[i] { painter.add(Shape::line(line.iter().map(|p| st.to_screen(*p)).collect(), Stroke::new(stroke_w * 0.55, cc.ink))); }
        }
        // fill regions as dashed guides; the lasso being drawn
        if !lit {
            for f in &st.settings.fills {
                let mut pts: Vec<Pos2> = f.outline.iter().map(|p| st.to_screen(*p)).collect();
                if let Some(first) = pts.first().copied() { pts.push(first); }
                painter.extend(Shape::dashed_line(&pts, Stroke::new(1.0, cc.guide), 6.0, 4.0));
            }
        }
        let snap_on = st.show_grid;
        let snap_pt = |p: Point| if snap_on { pt((p.x / step).round() * step, (p.y / step).round() * step) } else { p };
        match (st.fill_shape, &st.lasso) {
            (FillShape::Lasso, Some(l)) if l.len() > 1 => { painter.add(Shape::line(l.iter().map(|p| st.to_screen(*p)).collect(), Stroke::new(1.6, cc.mark))); }
            (FillShape::Polygon, Some(l)) if !l.is_empty() => {
                // corners so far, then a rubber band to the (snapped) pointer
                let mut line: Vec<Pos2> = l.iter().map(|p| st.to_screen(*p)).collect();
                if let Some(h) = resp.hover_pos() { line.push(st.to_screen(snap_pt(st.to_mm(h)))); }
                painter.add(Shape::line(line, Stroke::new(1.6, cc.mark)));
                for (k, p) in l.iter().enumerate() { painter.circle_filled(st.to_screen(*p), if k == 0 { 5.0 } else { 3.5 }, cc.mark); }
            }
            (FillShape::Regular, Some(l)) if l.len() == 2 => {
                if let Some(o) = st.drawn_outline() { painter.add(Shape::closed_line(o.iter().map(|p| st.to_screen(*p)).collect(), Stroke::new(1.6, cc.mark))); }
                painter.circle_filled(st.to_screen(l[0]), 3.5, cc.mark);
            }
            _ => {}
        }

        // handles of the selected chip (a curved faceted chip is only moved)
        let first_faceted = if st.settings.faceted.is_some() { 0 } else { st.starts.first().copied().unwrap_or(usize::MAX) };
        let handles: Vec<(usize, Pos2, String)> = st.selected.filter(|&i| i < first_faceted || st.regions[i].len() <= 6).and_then(|i| st.regions.get(i)).map(|p| chip_handles(p).into_iter().map(|(k, q, l)| (k, st.to_screen(q), l)).collect()).unwrap_or_default();
        let hover = resp.hover_pos();
        for (_, p, label) in &handles {
            let hot = hover.is_some_and(|h| h.distance(*p) < 10.0);
            painter.circle(*p, if hot { 7.0 } else { 5.5 }, cc.paper, Stroke::new(2.0, cc.mark));
            if hot { painter.text(*p + Vec2::new(10.0, -10.0), egui::Align2::LEFT_BOTTOM, label, egui::FontId::proportional(12.0), cc.mark); }
        }

        // ----- interaction -----
        let pan_button = ui.input(|i| i.pointer.middle_down() || i.pointer.secondary_down() || i.key_down(egui::Key::Space));
        let snap = |v: f64| (v / step).round() * step;
        if resp.drag_started() {
            let pos = ui.input(|i| i.pointer.press_origin()).or(resp.interact_pointer_pos()).unwrap_or_default();
            let mm = st.to_mm(pos);
            st.drag_before = Some(st.settings.clone());
            st.drag = if pan_button { Some(ChipDrag::Pan) }
                else if st.lasso.is_some() {
                    // freehand starts here; a regular shape starts at its (snapped) centre; polygon corners come on release
                    match st.fill_shape { FillShape::Lasso => st.lasso = Some(vec![mm]), FillShape::Regular => st.lasso = Some(vec![snap_pt(mm), mm]), FillShape::Polygon => {} }
                    Some(ChipDrag::Lasso)
                }
                else if let Some((k, _, _)) = handles.iter().find(|(_, p, _)| p.distance(pos) < 10.0) { let id = st.selected.unwrap(); Some(ChipDrag::Handle { id, vertex: *k, points: st.regions[id].clone() }) }
                else if let Some(id) = hit_chip(&st.regions, &st.settings.removed, mm) { st.selected = Some(id); Some(ChipDrag::Move { id, start: mm, points: st.regions[id].clone() }) }
                else { None };
            st.message.clear();
        }
        if resp.dragged() {
            let pos = resp.interact_pointer_pos().unwrap_or_default(); let mm = st.to_mm(pos);
            match &st.drag {
                Some(ChipDrag::Pan) => st.origin += resp.drag_delta(),
                Some(ChipDrag::Lasso) => if let Some(l) = st.lasso.as_mut() {
                    match st.fill_shape {
                        // a point every 0.8 mm or so is plenty for a hand-drawn outline
                        FillShape::Lasso => if l.last().is_none_or(|q| (q.x - mm.x).hypot(q.y - mm.y) > 0.8) { l.push(mm); },
                        FillShape::Regular => if l.len() == 2 { l[1] = mm; },
                        FillShape::Polygon => {}
                    }
                },
                Some(ChipDrag::Move { id, start, points }) => {
                    let (dx, dy) = (snap(mm.x - start.x), snap(mm.y - start.y));
                    let moved: Vec<Point> = points.iter().map(|q| pt(q.x + dx, q.y + dy)).collect();
                    let id = *id;
                    if moved.iter().all(|q| q.x >= 0.0 && q.y >= 0.0 && q.x <= size && q.y <= size_h) && valid_chip(&moved) { let mut s = st.drag_before.clone().unwrap(); s.edits.insert(id, moved); st.settings = s; }
                }
                Some(ChipDrag::Handle { id, vertex, points }) => {
                    let moved = move_chip_handle(points, *vertex, pt(snap(mm.x), snap(mm.y)));
                    let id = *id;
                    if moved.iter().all(|q| q.x >= 0.0 && q.y >= 0.0 && q.x <= size && q.y <= size_h) && valid_chip(&moved) { let mut s = st.drag_before.clone().unwrap(); s.edits.insert(id, moved); st.settings = s; }
                }
                None => {}
            }
        }
        if resp.drag_stopped() {
            if let Some(before) = st.drag_before.take() { if before != st.settings && !matches!(st.drag, Some(ChipDrag::Pan)) { st.past.push(before); st.future.clear(); } }
            if matches!(st.drag, Some(ChipDrag::Lasso)) {
                // a polygon corner placed with a slight drag still counts as a click
                if st.fill_shape == FillShape::Polygon { if let Some(pos) = resp.interact_pointer_pos() { add_corner(st, pos, snap_pt(st.to_mm(pos))); } }
                else { st.finish_lasso(); }
            }
            st.drag = None;
        }
        // polygon: each click adds a corner; double-click closes
        if st.lasso.is_some() && st.fill_shape == FillShape::Polygon {
            if resp.double_clicked() { if st.lasso.as_ref().is_some_and(|l| l.len() >= 3) { st.finish_lasso(); } }
            else if resp.clicked() { if let Some(pos) = resp.interact_pointer_pos() { add_corner(st, pos, snap_pt(st.to_mm(pos))); } }
        }
        // L toggles the fill tool, Enter closes a polygon, Esc puts the tool away
        if !ui.ctx().wants_keyboard_input() {
            if ui.input(|i| i.key_pressed(egui::Key::L)) { st.toggle_lasso(); }
            if st.fill_shape == FillShape::Polygon && st.lasso.as_ref().is_some_and(|l| l.len() >= 3) && ui.input(|i| i.key_pressed(egui::Key::Enter)) { st.finish_lasso(); }
            if st.lasso.is_some() && ui.input(|i| i.key_pressed(egui::Key::Escape)) { st.lasso = None; st.drag = None; st.message = "Fill tool put away.".into(); }
        }
        if resp.clicked() && st.lasso.is_none() {
            if let Some(pos) = resp.interact_pointer_pos() { st.selected = hit_chip(&st.regions, &st.settings.removed, st.to_mm(pos)); st.message.clear(); }
        }
        let over_chip = hover.is_some_and(|h| hit_chip(&st.regions, &st.settings.removed, st.to_mm(h)).is_some());
        if st.lasso.is_some() && resp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair); }
        else if handles.iter().any(|(_, p, _)| hover.is_some_and(|h| h.distance(*p) < 10.0)) { ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair); }
        else if over_chip { ui.ctx().set_cursor_icon(egui::CursorIcon::Move); }

        // a small toggle tucked in the canvas corner (the shelf layout has its own Flat / Lit switch)
        if self.prefs.shelf { return; }
        let at = Rect::from_min_size(rect.left_bottom() + Vec2::new(12.0, -36.0), Vec2::new(96.0, 24.0));
        let button = egui::Button::new(egui::RichText::new("Lit preview").small()).selected(lit);
        if ui.put(at, button).on_hover_text("Raking light from the upper left, as the cut wood would look (also in View)").clicked() { self.set_prefs(Prefs { chip_lit: !lit, ..self.prefs }); }
    }
}

/// Wood tone for a facet brightness from `facets::shade`.
fn wood(k: u8) -> Color32 { let k = k as u32; Color32::from_rgb((k * 235 / 255) as u8, (k * 200 / 255) as u8, (k * 150 / 255) as u8) }

/// The box assembled in isometric, lit: the lid and the two faces seen from the front-right
/// (or back-left), fitted to the canvas. Faces turned from the light are darkened.
fn draw_box_3d(st: &mut ChipState, painter: &egui::Painter, rect: Rect, cc: &Canvas) {
    let Some(b) = st.boxd.as_ref() else { return };
    let (faces, vw, vh) = b.design.iso_view(b.from_back);
    // lit facets per face, cached until the box changes
    if st.iso_cache.as_ref().map(|(d, _)| d) != Some(&b.design) {
        let lit: Vec<Vec<([Point; 3], u8)>> = Face::ALL.iter().map(|f| match b.design.panel(*f) {
            Some(p) => chip_facets(p).into_iter().enumerate().filter(|(i, _)| !p.removed.contains(i)).flat_map(|(_, c)| c.lit_triangles(3.5)).collect(),
            None => vec![],
        }).collect();
        st.iso_cache = Some((b.design.clone(), lit));
    }
    let lit = &st.iso_cache.as_ref().unwrap().1;
    let z = ((rect.width() - 80.0) as f64 / vw).min((rect.height() - 80.0) as f64 / vh).max(0.1);
    let o = (rect.center().x as f64 - vw * z / 2.0, rect.center().y as f64 - vh * z / 2.0);
    for (face, m, shade) in faces {
        let to = |p: Point| Pos2::new((o.0 + z * (m[0] * p.x + m[2] * p.y + m[4])) as f32, (o.1 + z * (m[1] * p.x + m[3] * p.y + m[5])) as f32);
        let (w, h) = b.design.size_of(face);
        let dim = |c: Color32| { let k = 1.0 - shade as f32; Color32::from_rgb((c.r() as f32 * k) as u8, (c.g() as f32 * k) as u8, (c.b() as f32 * k) as u8) };
        let corners = [pt(0.0, 0.0), pt(w, 0.0), pt(w, h), pt(0.0, h)];
        let surface = dim(wood(shade_flat()));
        painter.add(Shape::convex_polygon(corners.iter().map(|p| to(*p)).collect(), surface, Stroke::NONE));
        let mut mesh = egui::Mesh::default();
        let idx = Face::ALL.iter().position(|f| *f == face).unwrap();
        for (tri, k) in &lit[idx] {
            let base = mesh.vertices.len() as u32; let col = dim(wood(*k));
            for p in tri { mesh.colored_vertex(to(*p), col); }
            mesh.add_triangle(base, base + 1, base + 2);
        }
        painter.add(Shape::mesh(mesh));
        painter.add(Shape::closed_line(corners.iter().map(|p| to(*p)).collect(), Stroke::new(1.0, Color32::from_rgb(58, 46, 32))));
    }
    let note = if b.from_back { "Seen from the back-left" } else { "Seen from the front-right" };
    painter.text(rect.left_top() + Vec2::new(16.0, 14.0), egui::Align2::LEFT_TOP, format!("Box {} × {} × {} mm · {note} · pick a face under Box to edit it", b.design.length, b.design.width, b.design.height), egui::FontId::proportional(13.0), cc.guide);
}

/// Brightness of the uncut surface.
fn shade_flat() -> u8 { shade(&[(0.0, 0.0, 0.0), (1.0, 0.0, 0.0), (0.0, 1.0, 0.0)]) }

/// Polygon tool: add a corner, or close the shape when the click lands on the first corner.
fn add_corner(st: &mut ChipState, screen: Pos2, mm: Point) {
    let first = st.lasso.as_ref().and_then(|l| l.first().copied());
    let n = st.lasso.as_ref().map_or(0, |l| l.len());
    if n >= 3 && first.is_some_and(|f| st.to_screen(f).distance(screen) < 10.0) { st.finish_lasso(); return; }
    if let Some(l) = st.lasso.as_mut() { if l.last() != Some(&mm) { l.push(mm); } }
}

fn hit_chip(regions: &[Vec<Point>], removed: &[usize], mm: Point) -> Option<usize> {
    regions.iter().enumerate().rev().find(|(i, p)| !removed.contains(i) && scroll_core::outline::inside(mm, p)).map(|(i, _)| i)
}

/// Ear-clipping triangulation for filling a simple (possibly concave) outline.
fn triangulate(poly: &[Point]) -> Vec<[usize; 3]> {
    let n = poly.len();
    if n < 3 { return vec![]; }
    let area: f64 = (0..n).map(|i| { let (a, b) = (poly[i], poly[(i + 1) % n]); a.x * b.y - b.x * a.y }).sum();
    let sign = if area >= 0.0 { 1.0 } else { -1.0 };
    let cross = |a: Point, b: Point, c: Point| ((b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)) * sign;
    let mut idx: Vec<usize> = (0..n).collect();
    let mut out = vec![];
    let mut guard = 0;
    while idx.len() > 3 && guard < n * n {
        guard += 1;
        let m = idx.len();
        let mut clipped = false;
        for k in 0..m {
            let (ia, ib, ic) = (idx[(k + m - 1) % m], idx[k], idx[(k + 1) % m]);
            let (a, b, c) = (poly[ia], poly[ib], poly[ic]);
            if cross(a, b, c) <= 1e-12 { continue; }
            let blocked = idx.iter().any(|&j| j != ia && j != ib && j != ic && { let p = poly[j]; cross(a, b, p) >= 0.0 && cross(b, c, p) >= 0.0 && cross(c, a, p) >= 0.0 });
            if blocked { continue; }
            out.push([ia, ib, ic]); idx.remove(k); clipped = true; break;
        }
        if !clipped { break; }
    }
    if idx.len() == 3 { out.push([idx[0], idx[1], idx[2]]); }
    else if idx.len() > 3 { for k in 1..idx.len() - 1 { out.push([idx[0], idx[k], idx[k + 1]]); } } // degenerate leftovers: fan
    out
}

// ---- the shelf layout (ZBrush-style) for the Chip workspace ------------------

use crate::shelf::{self as sh, bevel_button, big_tab, caption, divider, seg, shelf_caption, stepper, tile, zslider};

impl App {
    /// Select / Fill tools and the fill shape, the composition's main
    /// controls, the Flat / Lit view and Box / Export.
    pub(crate) fn chip_shelf_context(&mut self, ctx: &egui::Context) {
        let t = self.t();
        egui::TopBottomPanel::top("chip-shelf").exact_height(58.0).frame(egui::Frame::none().fill(t.panel).inner_margin(egui::Margin::symmetric(10.0, 0.0)).stroke(Stroke::new(1.0, t.border))).show(ctx, |ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                let boxed = self.chip.boxd.is_some();
                let export = big_tab(ui, t, "Export", false).on_hover_text("Save at actual size as SVG");
                let popup = ui.id().with("chip-export-menu");
                if export.clicked() { ui.memory_mut(|m| m.toggle_popup(popup)); }
                egui::popup_below_widget(ui, popup, &export, egui::PopupCloseBehavior::CloseOnClick, |ui| {
                    ui.set_min_width(220.0);
                    if ui.button(if boxed { "This panel SVG…" } else { "Chip SVG…" }).clicked() { self.chip.export(); }
                    if boxed && ui.button("All panels on one sheet…").clicked() { self.chip.export_sheet(); }
                });
                let box_open = self.palettes[sh::CHIP_BOX];
                if big_tab(ui, t, "Box", boxed || box_open).on_hover_text("Make this pattern one face of a carved box, or edit the box").clicked() { self.palettes[sh::CHIP_BOX] = !box_open; }
                ui.add_space(8.0); divider(ui, t); ui.add_space(8.0);
                ui.allocate_ui_with_layout(Vec2::new(124.0, 50.0), egui::Layout::top_down(egui::Align::Min), |ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    ui.add_space(3.0);
                    ui.label(egui::RichText::new("VIEW").size(10.5).color(t.dim));
                    let mut lit = self.prefs.chip_lit;
                    if seg(ui, t, &[(false, "Flat"), (true, "Lit")], &mut lit, 58.0, 22.0) { self.set_prefs(Prefs { chip_lit: lit, ..self.prefs }); }
                });
                ui.add_space(4.0); divider(ui, t); ui.add_space(8.0);
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    let filling = self.chip.lasso.is_some();
                    if bevel_button(ui, t, "Select", !filling, Vec2::new(66.0, 38.0)).on_hover_text("Click a chip to select it; drag chips and handles (edits snap to the grid)").clicked() && filling { self.chip.lasso = None; self.chip.drag = None; }
                    if bevel_button(ui, t, "Fill", filling, Vec2::new(56.0, 38.0)).on_hover_text("Fill an area  (L)\nDraw an outline; it fills with a repeat, clear of the chips already there").clicked() { self.chip.toggle_lasso(); }
                    if filling {
                        ui.add_space(4.0);
                        let mut shape = self.chip.fill_shape;
                        if seg(ui, t, &[(FillShape::Lasso, "Lasso"), (FillShape::Polygon, "Polygon"), (FillShape::Regular, "Regular")], &mut shape, 64.0, 30.0) { self.chip.fill_shape = shape; self.chip.lasso = None; self.chip.toggle_lasso(); }
                    }
                    ui.add_space(8.0); divider(ui, t); ui.add_space(8.0);
                    ui.spacing_mut().item_spacing.x = 8.0;
                    sh::tight_controls(ui, |ui| self.chip_context_controls(ui));
                });
            });
        });
    }

    fn chip_context_controls(&mut self, ui: &mut egui::Ui) {
        let t = self.t();
        let s = self.chip.settings.clone();
        caption(ui, t, "Composition");
        if let Some(f) = s.faceted {
            let step = stepper(ui, t, "Variation", s.seed());
            if step != 0 {
                // step until the permutation actually changes, as Generate variation does
                let mut seed = s.seed();
                let next = loop { seed = if step > 0 { seed.wrapping_add(1) } else { seed.wrapping_sub(1) }; let p = Faceted::from_seed(seed); if p != f { break p; } };
                self.chip.change(ChipSettings { seed: Some(seed), faceted: Some(next), ..s.regenerated() }); self.chip.selected = None;
                return;
            }
            let mut n = f;
            if n.field.is_none() {
                let k = stepper(ui, t, n.centre.count_label(), n.count);
                if k != 0 { let r = n.centre.counts(); n.count = (n.count as i64 + k as i64).clamp(*r.start() as i64, *r.end() as i64) as u32; }
            }
            let mut pct = n.scale * 100.0;
            let label = if n.field.is_some() { "Cell size" } else { "Centre size" };
            if zslider(ui, t, label, &mut pct, 40.0, 100.0, |v| format!("{v:.0}%"), 160.0).on_hover_text("Smaller leaves room round the centre for a fill").changed() { n.scale = (pct / 100.0).clamp(0.4, 1.0); }
            if n != f { self.chip.change(ChipSettings { faceted: Some(n), removed: vec![], edits: Default::default(), ..s.clone() }); self.chip.selected = None; }
        } else {
            let step = stepper(ui, t, "Variation", s.seed());
            if step != 0 { let seed = if step > 0 { s.seed().wrapping_add(1) } else { s.seed().wrapping_sub(1) }; self.chip.change(ChipSettings { seed: Some(seed), ..s.regenerated() }); self.chip.selected = None; return; }
            if bevel_button(ui, t, "Vary border", false, Vec2::new(104.0, 30.0)).on_hover_text("Keeps the centre and changes only the border").clicked() { self.chip.change(ChipSettings { border_seed: Some(s.border_seed().wrapping_add(1)), ..s.regenerated() }); self.chip.selected = None; }
        }
    }

    /// The composition shelf: the centre (or field) and the border, as tiles.
    pub(crate) fn chip_shelf_tools(&mut self, ctx: &egui::Context) {
        let t = self.t();
        egui::SidePanel::left("chip-shelf-tools").exact_width(84.0).resizable(false).frame(egui::Frame::none().fill(t.panel).inner_margin(egui::Margin::symmetric(8.0, 10.0)).stroke(Stroke::new(1.0, t.border))).show(ctx, |ui| {
            egui::ScrollArea::vertical().scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden).show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 6.0;
                let s = self.chip.settings.clone();
                let Some(f) = s.faceted else {
                    shelf_caption(ui, t, "CLASSIC");
                    let fams: [(ChipFamily, &str, fn(&egui::Painter, Rect, Color32)); 3] = [(ChipFamily::Star, "Star", icon_centre_star), (ChipFamily::Rosette, "Rosette", icon_centre_petals), (ChipFamily::Border, "Border", icon_border_zigzag)];
                    for (fam, name, icon) in fams {
                        if tile(ui, t, name, icon, s.family == fam).on_hover_text(fam.label()).clicked() && s.family != fam { self.chip.change(ChipSettings { family: fam, ..s.regenerated() }); self.chip.selected = None; }
                    }
                    rule(ui, t);
                    if tile(ui, t, "Faceted", icon_centre_star, false).on_hover_text("Switch to the faceted engine").clicked() { self.chip.change(ChipSettings { faceted: Some(Faceted::from_seed(s.seed())), ..s.regenerated() }); self.chip.selected = None; }
                    return;
                };
                shelf_caption(ui, t, "CENTRE");
                let centres: [(Centre, &str, fn(&egui::Painter, Rect, Color32)); 5] = [(Centre::Star, "Star", icon_centre_star), (Centre::Petals, "Petals", icon_centre_petals), (Centre::Fan, "Fan", icon_centre_fan), (Centre::Swirl, "Swirl", icon_centre_swirl), (Centre::Rocaille, "Rocaille", icon_centre_rocaille)];
                let mut n = f;
                for (c, name, icon) in centres {
                    if tile(ui, t, name, icon, f.field.is_none() && f.centre == c).on_hover_text(c.label()).clicked() { n.field = None; if n.centre != c { n.centre = c; n.count = c.default_count(); } }
                }
                if tile(ui, t, "Field", icon_field, f.field.is_some()).on_hover_text("A field of a repeat filling the inside of the border, as on box sides").clicked() && f.field.is_none() { n.field = Some(Repeat::StarsAndDiamonds); }
                rule(ui, t);
                shelf_caption(ui, t, "BORDER");
                let borders: [(Option<BorderStyle>, &str, fn(&egui::Painter, Rect, Color32)); 4] = [(None, "None", icon_border_none), (Some(BorderStyle::Zigzag), "Zigzag", icon_border_zigzag), (Some(BorderStyle::Arcade), "Arcade", icon_border_arcade), (Some(BorderStyle::Almond), "Almond", icon_border_almond)];
                for (b, name, icon) in borders {
                    if tile(ui, t, name, icon, f.border == b).on_hover_text(b.map_or("No border", |b| b.label())).clicked() { n.border = b; }
                }
                if n != f { self.chip.change(ChipSettings { faceted: Some(n), removed: vec![], edits: Default::default(), ..s.clone() }); self.chip.selected = None; }
            });
        });
    }

    /// Folding palettes: the Chip panel's sections, one per palette.
    pub(crate) fn chip_shelf_tray(&mut self, ctx: &egui::Context) {
        let t = self.t();
        egui::SidePanel::right("chip-shelf-tray").default_width(340.0).min_width(300.0).frame(egui::Frame::none().fill(t.bg).inner_margin(egui::Margin { left: 8.0, right: 6.0, top: 8.0, bottom: 8.0 })).show(ctx, |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                ui.set_width(ui.available_width() - 6.0);
                ui.spacing_mut().item_spacing.y = 6.0;
                if self.chip.boxd.is_some() || self.palettes[sh::CHIP_BOX] { self.palette(ui, sh::CHIP_BOX, "Box", |s, ui| s.chip_box(ui)); }
                self.palette(ui, sh::CHIP_COMPOSE, "Composition", |s, ui| {
                    let st = s.chip.settings.clone();
                    let mut faceted = st.faceted.is_some();
                    segmented(ui, s.t(), &[(true, "Faceted"), (false, "Classic")], &mut faceted);
                    if faceted != st.faceted.is_some() {
                        let f = if faceted { Some(Faceted::from_seed(st.seed())) } else { None };
                        s.chip.change(ChipSettings { faceted: f, ..st.regenerated() }); s.chip.selected = None;
                    } else if let Some(f) = st.faceted { s.faceted_controls(ui, &st, f); } else { s.classic_controls(ui, &st); }
                });
                self.palette(ui, sh::CHIP_FILL, "Fill an area", |s, ui| s.chip_fills(ui));
                self.palette(ui, sh::CHIP_PAGE, "Page", |s, ui| s.chip_page_and_edit(ui));
                self.palette(ui, sh::CHIP_PRESETS, "Presets", |s, ui| s.chip_presets(ui));
                self.palette(ui, sh::CANVAS, "Canvas", |s, ui| s.theme_tab(ui));
            });
        });
    }

    pub(crate) fn chip_shelf_status(&mut self, ctx: &egui::Context) {
        let t = self.t();
        egui::TopBottomPanel::bottom("chip-shelf-status").exact_height(28.0).frame(egui::Frame::none().fill(t.bg).inner_margin(egui::Margin::symmetric(14.0, 0.0))).show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if sh::small_button(ui, t, "Fit").on_hover_text("Fit the page in the window").clicked() { self.chip.fit_page(); }
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        let c = &self.chip;
                        let s = |x: String| egui::RichText::new(x).size(12.5).color(t.dim);
                        ui.label(s(format!("{} × {} mm", c.settings.width(), c.settings.page_height()))); ui.add_space(12.0);
                        ui.label(s(format!("{} mm grid · {} chips", c.settings.step(), c.chip_count()))); ui.add_space(12.0);
                        if let Some(m) = c.cursor_mm { ui.label(s(format!("x {:.1}  y {:.1} mm", m.x, m.y))); ui.add_space(12.0); }
                        let hint = match c.selected { None => "Click a chip to select it. Drag chips and handles; edits snap to the grid.".to_string(), Some(i) => format!("Chip {} selected · drag it or its handles · Delete removes it", i + 1) };
                        ui.add(egui::Label::new(s(if c.message.is_empty() { hint } else { c.message.clone() })).truncate());
                    });
                });
            });
        });
    }
}

fn rule(ui: &mut egui::Ui, t: &Theme) {
    ui.add_space(2.0);
    let r = ui.available_rect_before_wrap();
    ui.painter().hline(r.left() + 6.0..=r.right() - 6.0, r.top(), Stroke::new(1.0, t.border));
    ui.add_space(8.0);
}

// shelf icons, drawn in a unit box
fn polar(r: Rect, a: f32, k: f32) -> Pos2 { let c = r.center(); Pos2::new(c.x + a.cos() * k * r.width() * 0.5, c.y + a.sin() * k * r.height() * 0.5) }
fn icon_centre_star(p: &egui::Painter, r: Rect, c: Color32) {
    let n = 8; let pts: Vec<Pos2> = (0..n * 2).map(|i| polar(r, i as f32 * std::f32::consts::PI / n as f32 - 1.5708, if i % 2 == 0 { 1.0 } else { 0.45 })).collect();
    p.add(Shape::closed_line(pts.clone(), Stroke::new(1.4, c)));
    for i in (0..n * 2).step_by(2) { p.line_segment([r.center(), pts[i]], Stroke::new(0.8, c)); }
}
fn icon_centre_petals(p: &egui::Painter, r: Rect, c: Color32) {
    for k in 0..6 {
        let a = k as f32 * std::f32::consts::TAU / 6.0; let tip = polar(r, a, 1.0); let n = Vec2::new(-a.sin(), a.cos()) * r.width() * 0.13;
        let one: Vec<Pos2> = (0..=12).map(|i| { let s = i as f32 / 12.0; r.center() + (tip - r.center()) * s + n * (std::f32::consts::PI * s).sin() }).collect();
        let two: Vec<Pos2> = (0..=12).rev().map(|i| { let s = i as f32 / 12.0; r.center() + (tip - r.center()) * s - n * (std::f32::consts::PI * s).sin() }).collect();
        p.add(Shape::closed_line(one.into_iter().chain(two).collect(), Stroke::new(1.2, c)));
    }
}
fn icon_centre_fan(p: &egui::Painter, r: Rect, c: Color32) {
    p.circle_stroke(r.center(), r.width() * 0.5, Stroke::new(1.3, c)); p.circle_stroke(r.center(), r.width() * 0.22, Stroke::new(1.1, c));
    for k in 0..12 { let a = k as f32 * std::f32::consts::TAU / 12.0; p.line_segment([polar(r, a, 0.44), polar(r, a, 1.0)], Stroke::new(0.9, c)); }
}
fn icon_centre_swirl(p: &egui::Painter, r: Rect, c: Color32) {
    for k in 0..6 { let a0 = k as f32 * std::f32::consts::TAU / 6.0; p.add(Shape::line((0..=16).map(|i| { let s = i as f32 / 16.0; polar(r, a0 + s * 1.6, 0.15 + s * 0.85) }).collect(), Stroke::new(1.3, c))); }
    p.circle_stroke(r.center(), r.width() * 0.5, Stroke::new(1.0, c));
}
/// Rocaille swirl: C-scrolls whirling round a fluted centre.
fn icon_centre_rocaille(p: &egui::Painter, r: Rect, c: Color32) {
    for k in 0..5 {
        let a0 = k as f32 * std::f32::consts::TAU / 5.0;
        // a comma: out from the centre, curling ever tighter into a hooked head
        p.add(Shape::line((0..=20).map(|i| { let s = i as f32 / 20.0; polar(r, a0 + s * 0.9 + s * s * s * 2.4, 0.3 + 0.62 * s - 0.25 * s * s * s) }).collect(), Stroke::new(1.4, c)));
    }
    p.circle_stroke(r.center(), r.width() * 0.13, Stroke::new(1.0, c));
}
fn icon_field(p: &egui::Painter, r: Rect, c: Color32) {
    for i in 0..3 { for j in 0..3 {
        let q = Rect::from_min_size(r.min + Vec2::new(i as f32, j as f32) * r.width() / 3.0, Vec2::splat(r.width() / 3.0)).shrink(2.0);
        p.add(Shape::closed_line(vec![Pos2::new(q.center().x, q.top()), Pos2::new(q.right(), q.center().y), Pos2::new(q.center().x, q.bottom()), Pos2::new(q.left(), q.center().y)], Stroke::new(1.1, c)));
    } }
}
fn icon_border_none(p: &egui::Painter, r: Rect, c: Color32) {
    p.rect_stroke(r.shrink(2.0), 1.0, Stroke::new(1.2, c));
    p.line_segment([r.left_bottom() + Vec2::new(3.0, -3.0), r.right_top() + Vec2::new(-3.0, 3.0)], Stroke::new(1.0, c));
}
fn icon_border_zigzag(p: &egui::Painter, r: Rect, c: Color32) {
    p.rect_stroke(r.shrink(1.0), 1.0, Stroke::new(1.0, c));
    let pts: Vec<Pos2> = (0..=8).map(|i| Pos2::new(r.left() + 2.0 + i as f32 * (r.width() - 4.0) / 8.0, if i % 2 == 0 { r.top() + 3.0 } else { r.top() + 8.0 })).collect();
    p.add(Shape::line(pts.clone(), Stroke::new(1.2, c)));
    p.add(Shape::line(pts.iter().map(|q| Pos2::new(q.x, r.bottom() - (q.y - r.top()))).collect(), Stroke::new(1.2, c)));
}
fn icon_border_arcade(p: &egui::Painter, r: Rect, c: Color32) {
    p.rect_stroke(r.shrink(1.0), 1.0, Stroke::new(1.0, c));
    for k in 0..4 {
        let x0 = r.left() + 2.0 + k as f32 * (r.width() - 4.0) / 4.0; let w = (r.width() - 4.0) / 4.0;
        p.add(Shape::line((0..=10).map(|i| { let s = i as f32 / 10.0; Pos2::new(x0 + s * w, r.top() + 8.0 - (std::f32::consts::PI * s).sin() * 5.0) }).collect(), Stroke::new(1.2, c)));
    }
}
fn icon_border_almond(p: &egui::Painter, r: Rect, c: Color32) {
    p.rect_stroke(r.shrink(1.0), 1.0, Stroke::new(1.0, c));
    for k in 0..3 {
        let x0 = r.left() + 3.0 + k as f32 * (r.width() - 6.0) / 3.0; let w = (r.width() - 6.0) / 3.0; let y = r.top() + 6.0;
        let top: Vec<Pos2> = (0..=10).map(|i| { let s = i as f32 / 10.0; Pos2::new(x0 + s * w, y - (std::f32::consts::PI * s).sin() * 3.0) }).collect();
        let bottom: Vec<Pos2> = (0..=10).rev().map(|i| { let s = i as f32 / 10.0; Pos2::new(x0 + s * w, y + (std::f32::consts::PI * s).sin() * 3.0) }).collect();
        p.add(Shape::closed_line(top.into_iter().chain(bottom).collect(), Stroke::new(1.1, c)));
    }
}
