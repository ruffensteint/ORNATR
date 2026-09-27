//! The Chip workspace: generate a chip-carving medallion, edit it on the
//! millimetre grid, keep presets and export the SVG at actual size.
use super::*;
use crate::presets::{app_dir, Library};
use scroll_core::facets::{shade, BorderStyle, Centre};
use scroll_core::chip::{chip_facets, chip_handles, chip_regions, Faceted, chip_svg, move_chip_handle, valid_chip, ChipFamily, ChipSettings};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ChipTab { Generate, Presets, Theme }

enum ChipDrag { Pan, Move { id: usize, start: Point, points: Vec<Point> }, Handle { id: usize, vertex: usize, points: Vec<Point> } }

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
    autosaved: Option<ChipSettings>,
    pub message: String,
    // view
    zoom: f32,
    origin: Pos2,
    fitted: bool,
    pub cursor_mm: Option<Point>,
    pub presets: Library,
    thumbs: Vec<Option<Vec<Vec<Point>>>>,
}

fn autosave_path() -> Option<PathBuf> { app_dir().map(|d| d.join("chip-current.json")) }

impl ChipState {
    pub fn new() -> ChipState {
        let settings = autosave_path().and_then(|p| std::fs::read_to_string(p).ok()).and_then(|t| io::parse_chip(&t).ok()).unwrap_or_default();
        ChipState { autosaved: Some(settings.clone()), settings, path: None, past: vec![], future: vec![], selected: None, show_grid: true, tab: ChipTab::Generate, drag: None, drag_before: None,
            cached_for: None, regions: vec![], triangles: vec![], lit: vec![], facet_lines: vec![], message: String::new(), zoom: 4.0, origin: Pos2::ZERO, fitted: false, cursor_mm: None, presets: Library::load("chip-presets.json"), thumbs: vec![] }
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
        if self.cached_for.as_ref() == Some(&self.settings) { return; }
        self.regions = chip_regions(&self.settings);
        self.triangles = self.regions.iter().map(|p| triangulate(p)).collect();
        let facets = chip_facets(&self.settings);
        self.lit = facets.iter().map(|c| c.lit_triangles(3.5)).collect();
        self.facet_lines = facets.iter().map(|c| if self.settings.faceted.is_some() { c.facet_lines() } else { vec![] }).collect();
        self.cached_for = Some(self.settings.clone());
        if self.selected.is_some_and(|i| i >= self.regions.len() || self.settings.removed.contains(&i)) { self.selected = None; }
    }
    /// Keep the current pattern on this computer, as the web version did.
    fn autosave(&mut self) {
        if self.drag.is_some() || self.autosaved.as_ref() == Some(&self.settings) { return; }
        if let Some(p) = autosave_path() { if let Some(d) = p.parent() { let _ = std::fs::create_dir_all(d); } let _ = std::fs::write(p, io::save_chip(&self.settings)); }
        self.autosaved = Some(self.settings.clone());
    }
    pub fn chip_count(&self) -> usize { (0..self.regions.len()).filter(|i| !self.settings.removed.contains(i)).count() }

    pub fn new_pattern(&mut self) { let d = ChipSettings::default(); let s = ChipSettings { faceted: Some(Faceted::from_seed(d.seed())), ..d }; self.change(s); self.path = None; self.selected = None; self.fitted = false; self.message = "New chip pattern. Undo returns to the previous one.".into(); }
    pub fn open(&mut self) {
        let Some(path) = rfd::FileDialog::new().add_filter("Chip layout", &["json"]).pick_file() else { return };
        match std::fs::read_to_string(&path).map_err(|e| e.to_string()).and_then(|t| io::parse_chip(&t)) {
            Ok(s) => { self.change(s); self.path = Some(path); self.selected = None; self.fitted = false; self.message = "Chip layout opened.".into(); }
            Err(e) => self.message = format!("Could not open this chip layout. {e}"),
        }
    }
    pub fn save(&mut self, choose: bool) {
        let path = if choose || self.path.is_none() {
            match rfd::FileDialog::new().add_filter("Chip layout", &["json"]).set_file_name("chip-layout.json").save_file() { Some(p) => p, None => return }
        } else { self.path.clone().unwrap() };
        match std::fs::write(&path, io::save_chip(&self.settings)) { Ok(()) => { self.path = Some(path); self.message = "Chip layout saved.".into(); } Err(e) => self.message = format!("Could not save: {e}") }
    }
    pub fn export(&mut self) {
        if let Some(p) = rfd::FileDialog::new().add_filter("SVG", &["svg"]).set_file_name("chip-pattern.svg").save_file() {
            match std::fs::write(&p, chip_svg(&self.settings)) { Ok(()) => self.message = format!("Exported {}.", p.display()), Err(e) => self.message = format!("Could not export: {e}") }
        }
    }
    pub fn remove_selected(&mut self) { if let Some(i) = self.selected { let mut s = self.settings.clone(); s.removed.push(i); self.change(s); self.selected = None; } }

    fn to_screen(&self, p: Point) -> Pos2 { Pos2::new(self.origin.x + p.x as f32 * self.zoom, self.origin.y + p.y as f32 * self.zoom) }
    fn to_mm(&self, s: Pos2) -> Point { pt(((s.x - self.origin.x) / self.zoom) as f64, ((s.y - self.origin.y) / self.zoom) as f64) }
    fn fit(&mut self, rect: Rect) {
        let size = self.settings.size as f32;
        self.zoom = ((rect.width() - 80.0) / size).min((rect.height() - 80.0) / size).max(0.5);
        self.origin = Pos2::new(rect.center().x - size * self.zoom / 2.0, rect.center().y - size * self.zoom / 2.0);
        self.fitted = true;
    }
    pub fn fit_page(&mut self) { self.fitted = false; }
    pub fn title(&self) -> String {
        let name = self.path.as_ref().and_then(|p| p.file_stem()).map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "Chip pattern".into());
        format!("{name} — ORNATR")
    }
}

impl App {
    pub(crate) fn chip_status(&mut self, ctx: &egui::Context) {
        let t = self.t();
        let c = &self.chip;
        let text = format!("{0} × {0} mm   ·   {1} mm grid   ·   {2} chips", c.settings.size, c.settings.step(), c.chip_count());
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
            if ui.add(egui::Button::new("New chip pattern").shortcut_text("Ctrl+N")).clicked() { self.chip.new_pattern(); ui.close_menu(); }
            if ui.add(egui::Button::new("Open chip layout…").shortcut_text("Ctrl+O")).clicked() { ui.close_menu(); self.chip.open(); }
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
        section(ui, t, "Composition");
        let mut faceted = s.faceted.is_some();
        segmented(ui, t, &[(true, "Faceted"), (false, "Classic")], &mut faceted);
        if faceted != s.faceted.is_some() {
            let f = if faceted { Some(Faceted::from_seed(s.seed())) } else { None };
            self.chip.change(ChipSettings { faceted: f, ..s.regenerated() }); self.chip.selected = None;
        } else if let Some(f) = s.faceted { self.faceted_controls(ui, &s, f); } else { self.classic_controls(ui, &s); }

        self.chip_page_and_edit(ui);
    }

    /// Centre, count and border for the faceted engine.
    fn faceted_controls(&mut self, ui: &mut egui::Ui, s: &ChipSettings, f: Faceted) {
        let t = self.t();
        ui.label(egui::RichText::new("A rosette, drawn with the facet lines you carve to, inside an optional border. Edit afterwards on the grid.").small().color(t.dim));
        let mut n = f;
        egui::Grid::new("chip-facets").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
            ui.label("Centre");
            egui::ComboBox::from_id_salt("chip-centre").width(170.0).selected_text(n.centre.label()).show_ui(ui, |ui| {
                for c in Centre::ALL { if ui.selectable_label(n.centre == c, c.label()).clicked() && n.centre != c { n.centre = c; n.count = c.default_count(); } }
            });
            ui.end_row();
            ui.label(n.centre.count_label());
            ui.add(egui::DragValue::new(&mut n.count).range(n.centre.counts()).speed(0.1));
            ui.end_row();
            ui.label("Border");
            egui::ComboBox::from_id_salt("chip-facet-border").width(170.0).selected_text(n.border.map_or("None", |b| b.label())).show_ui(ui, |ui| {
                ui.selectable_value(&mut n.border, None, "None");
                for b in BorderStyle::ALL { ui.selectable_value(&mut n.border, Some(b), b.label()); }
            });
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
        section(ui, t, "Grid and page");
        let max_grid = 30f64.min(s.size / 6.0).floor();
        let (mut step, mut size) = (s.step(), s.size);
        egui::Grid::new("chip-dims").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
            ui.label("Grid interval"); ui.add(egui::DragValue::new(&mut step).range(2.0..=max_grid).speed(0.1).fixed_decimals(0).suffix(" mm")); ui.end_row();
            ui.label("Page size"); ui.add(egui::DragValue::new(&mut size).range(40.0..=300.0).speed(1.0).fixed_decimals(0).suffix(" mm")); ui.end_row();
        });
        // Changing the grid or page regenerates the pattern at the new size.
        if step != s.step() { let mut n = s.regenerated(); n.grid = Some(step); self.chip.change(n); self.chip.selected = None; }
        else if size != s.size { let mut n = s.regenerated(); n.size = size; n.grid = Some(s.step().min((size / 6.0).floor())); self.chip.change(n); self.chip.selected = None; self.chip.fitted = false; }
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
        if ui.button("Export chip SVG…").clicked() { self.chip.export(); }
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
        if !st.fitted { st.fit(rect); }
        if let Some(hover) = resp.hover_pos() {
            let (scroll, zoom_delta) = ui.input(|i| (i.smooth_scroll_delta.y, i.zoom_delta()));
            let factor = if zoom_delta != 1.0 { zoom_delta } else { (scroll * 0.0015).exp() };
            if (factor - 1.0).abs() > 1e-4 { let before = st.to_mm(hover); st.zoom = (st.zoom * factor).clamp(0.5, 80.0); let after = st.to_screen(before); st.origin += hover - after; }
            st.cursor_mm = Some(st.to_mm(hover));
        }
        let size = st.settings.size; let step = st.settings.step();
        let page = Rect::from_min_max(st.to_screen(pt(0.0, 0.0)), st.to_screen(pt(size, size)));
        for (grow, alpha) in [(10.0, 10u8), (5.0, 16), (2.0, 24)] { painter.rect_filled(page.expand(grow).translate(Vec2::new(0.0, grow * 0.4)), grow + 2.0, Color32::from_black_alpha(alpha)); }
        painter.rect_filled(page, 2.0, if lit { wood(shade(&[(0.0, 0.0, 0.0), (1.0, 0.0, 0.0), (0.0, 1.0, 0.0)])) } else { cc.paper });
        if st.show_grid && step * st.zoom as f64 > 3.0 {
            // on the lit wood the grid is a faint darker line, so it doesn't fight the facets
            let grid_col = if lit { Color32::from_black_alpha(28) } else { cc.grid };
            let mut v = 0.0; let mut k = 0;
            while v <= size + 1e-9 {
                let w = if k % 5 == 0 { 0.9 } else { 0.5 };
                let a = st.to_screen(pt(v, 0.0)); painter.line_segment([Pos2::new(a.x, page.top()), Pos2::new(a.x, page.bottom())], Stroke::new(w, grid_col));
                let b = st.to_screen(pt(0.0, v)); painter.line_segment([Pos2::new(page.left(), b.y), Pos2::new(page.right(), b.y)], Stroke::new(w, grid_col));
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
        // handles of the selected chip
        let handles: Vec<(usize, Pos2, String)> = st.selected.and_then(|i| st.regions.get(i)).filter(|p| st.settings.faceted.is_none() || p.len() <= 6).map(|p| chip_handles(p).into_iter().map(|(k, q, l)| (k, st.to_screen(q), l)).collect()).unwrap_or_default();
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
                else if let Some((k, _, _)) = handles.iter().find(|(_, p, _)| p.distance(pos) < 10.0) { let id = st.selected.unwrap(); Some(ChipDrag::Handle { id, vertex: *k, points: st.regions[id].clone() }) }
                else if let Some(id) = hit_chip(&st.regions, &st.settings.removed, mm) { st.selected = Some(id); Some(ChipDrag::Move { id, start: mm, points: st.regions[id].clone() }) }
                else { None };
            st.message.clear();
        }
        if resp.dragged() {
            let pos = resp.interact_pointer_pos().unwrap_or_default(); let mm = st.to_mm(pos);
            match &st.drag {
                Some(ChipDrag::Pan) => st.origin += resp.drag_delta(),
                Some(ChipDrag::Move { id, start, points }) => {
                    let (dx, dy) = (snap(mm.x - start.x), snap(mm.y - start.y));
                    let moved: Vec<Point> = points.iter().map(|q| pt(q.x + dx, q.y + dy)).collect();
                    let id = *id;
                    if moved.iter().all(|q| q.x >= 0.0 && q.y >= 0.0 && q.x <= size && q.y <= size) && valid_chip(&moved) { let mut s = st.drag_before.clone().unwrap(); s.edits.insert(id, moved); st.settings = s; }
                }
                Some(ChipDrag::Handle { id, vertex, points }) => {
                    let moved = move_chip_handle(points, *vertex, pt(snap(mm.x), snap(mm.y)));
                    let id = *id;
                    if moved.iter().all(|q| q.x >= 0.0 && q.y >= 0.0 && q.x <= size && q.y <= size) && valid_chip(&moved) { let mut s = st.drag_before.clone().unwrap(); s.edits.insert(id, moved); st.settings = s; }
                }
                None => {}
            }
        }
        if resp.drag_stopped() {
            if let Some(before) = st.drag_before.take() { if before != st.settings && !matches!(st.drag, Some(ChipDrag::Pan)) { st.past.push(before); st.future.clear(); } }
            st.drag = None;
        }
        if resp.clicked() {
            if let Some(pos) = resp.interact_pointer_pos() { st.selected = hit_chip(&st.regions, &st.settings.removed, st.to_mm(pos)); st.message.clear(); }
        }
        let over_chip = hover.is_some_and(|h| hit_chip(&st.regions, &st.settings.removed, st.to_mm(h)).is_some());
        if handles.iter().any(|(_, p, _)| hover.is_some_and(|h| h.distance(*p) < 10.0)) { ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair); }
        else if over_chip { ui.ctx().set_cursor_icon(egui::CursorIcon::Move); }

        // a small toggle tucked in the canvas corner
        let at = Rect::from_min_size(rect.left_bottom() + Vec2::new(12.0, -36.0), Vec2::new(96.0, 24.0));
        let button = egui::Button::new(egui::RichText::new("Lit preview").small()).selected(lit);
        if ui.put(at, button).on_hover_text("Raking light from the upper left, as the cut wood would look (also in View)").clicked() { self.set_prefs(Prefs { chip_lit: !lit, ..self.prefs }); }
    }
}

/// Wood tone for a facet brightness from `facets::shade`.
fn wood(k: u8) -> Color32 { let k = k as u32; Color32::from_rgb((k * 235 / 255) as u8, (k * 200 / 255) as u8, (k * 150 / 255) as u8) }

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
