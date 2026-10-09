//! The Palmette workspace: palmettes, fleurons and rosettes from the palmette
//! library (`scroll_core::palmette`, the banked palmette and fleuron studies)
//! edited on the page. Each motif (a palmette fan, a petal, a lotus, a volute
//! arm, a collar, a boss, a fleurette, a husk drop, a petal ring, a beaded
//! ring) is one element with its own settings; it can be mirrored, turned
//! round a ring and set out along a row, and editing any copy edits them all.
//! Drag a motif to move it; drag its open handle to turn and size it.
use super::*;
use crate::cartouche_ui::{angle, fill_rings, mm_slider, size_slider, stroke_runs, CARVED_INK, WOOD};
use crate::platform;
use crate::shelf::{self as sh, bevel_button, big_tab, caption, divider, seg, shelf_caption, tile_with};
use scroll_core::cartouche::{bounds_of, in_shape, Drawing, Layer, Owner};
use scroll_core::palmette::{structure, Design, Element, Kind, Repeat, Tip, ADD, START, STRUCTURES};
use std::f64::consts::PI;
use web_time::Instant;

const AUTOSAVE: &str = "palmette-current.json";

/// A drag on the canvas: panning, a motif's aim handle (on copy `copy`), or a motif's body.
enum PDrag { Pan, Aim { id: u32, copy: usize }, Body { id: u32, copy: usize, start: Point } }

pub struct PalmetteState {
    pub design: Design,
    /// The library structure a new design starts from.
    pub structure: String,
    pub path: Option<PathBuf>,
    pub dirty: bool,
    past: Vec<Design>,
    future: Vec<Design>,
    /// The last edit's key and time, so dragging a slider is one undo step.
    last_edit: Option<(String, Instant)>,
    /// The selected element and which of its copies was picked.
    pub selected: Option<(u32, usize)>,
    pub filled: bool,
    pub show_grid: bool,
    drag: Option<PDrag>,
    drag_before: Option<Design>,
    // cached geometry for the current design
    cached_for: Option<Design>,
    layers: Vec<(Owner, Layer)>,
    boxes: Vec<Option<Bounds>>,
    drawing: Drawing,
    /// The filled preview, rasterized at this many pixels per mm.
    texture: Option<(egui::TextureHandle, f64)>,
    texture_stale: bool,
    autosaved: Option<String>,
    pub message: String,
    zoom: f32,
    origin: Pos2,
    fitted: bool,
    pub cursor_mm: Option<Point>,
    /// Library tiles: each structure's visible outlines in a unit box.
    thumbs: Option<Vec<Vec<Vec<Point>>>>,
}

impl PalmetteState {
    pub fn new() -> PalmetteState {
        let text = platform::store_read(AUTOSAVE);
        let design = text.as_deref().and_then(|t| io::parse_palmette(t).ok()).unwrap_or_else(|| Design::from_structure(START.0, START.1, START.2).unwrap());
        PalmetteState { design, structure: START.0.into(), path: None, dirty: false, past: vec![], future: vec![], last_edit: None, selected: None, filled: true, show_grid: false,
            drag: None, drag_before: None, cached_for: None, layers: vec![], boxes: vec![], drawing: Drawing::default(), texture: None, texture_stale: true, autosaved: text,
            message: String::new(), zoom: 3.0, origin: Pos2::ZERO, fitted: false, cursor_mm: None, thumbs: None }
    }

    // ---------- edits and undo ----------
    fn change(&mut self, next: Design) { self.change_keyed(None, next); }
    /// Replace the design; edits with the same key in quick succession (a slider being dragged) make one undo step.
    fn change_keyed(&mut self, key: Option<String>, next: Design) {
        if next == self.design { return; }
        let now = Instant::now();
        let merge = key.is_some() && self.last_edit.as_ref().is_some_and(|(k, t)| Some(k) == key.as_ref() && now.duration_since(*t).as_secs_f32() < 1.0);
        if merge { self.design = next; } else {
            self.past.push(std::mem::replace(&mut self.design, next));
            if self.past.len() > 80 { self.past.remove(0); }
        }
        self.future.clear(); self.dirty = true;
        self.last_edit = key.map(|k| (k, now));
    }
    pub fn undo(&mut self) { if let Some(p) = self.past.pop() { self.future.push(std::mem::replace(&mut self.design, p)); self.after_history(); } }
    pub fn redo(&mut self) { if let Some(n) = self.future.pop() { self.past.push(std::mem::replace(&mut self.design, n)); self.after_history(); } }
    fn after_history(&mut self) { self.dirty = true; self.last_edit = None; self.check_selection(); }
    pub fn can_undo(&self) -> bool { !self.past.is_empty() }
    pub fn can_redo(&self) -> bool { !self.future.is_empty() }
    fn check_selection(&mut self) {
        if let Some((id, k)) = self.selected {
            match self.design.get(id) {
                None => self.selected = None,
                Some(e) => if k >= e.repeat.count() { self.selected = Some((id, 0)); },
            }
        }
    }

    // ---------- cache ----------
    /// Bring the drawing up to date with the design.
    fn refresh(&mut self) {
        if self.cached_for.as_ref() == Some(&self.design) { return; }
        self.layers = self.design.layers();
        self.boxes = self.layers.iter().map(|(_, l)| l.bounds()).collect();
        self.drawing = scroll_core::cartouche::compose(&self.layers);
        self.cached_for = Some(self.design.clone());
        self.texture_stale = true;
        self.check_selection();
    }
    /// Keep the current design on this computer.
    fn autosave(&mut self) {
        if self.drag.is_some() { return; }
        let text = io::save_palmette(&self.design);
        if self.autosaved.as_ref() == Some(&text) { return; }
        let _ = platform::store_write(AUTOSAVE, &text);
        self.autosaved = Some(text);
    }

    // ---------- files ----------
    pub fn title(&self) -> String {
        let name = self.path.as_ref().and_then(|p| p.file_stem()).map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "Palmette".into());
        format!("{name}{} — ORNATR", if self.dirty { " •" } else { "" })
    }
    /// A new design on a `w` × `h` page, from the last library structure chosen.
    pub fn new_design(&mut self, w: f64, h: f64) {
        let d = Design::from_structure(&self.structure, w, h).unwrap_or_else(|| Design::empty(w, h));
        self.change(d); self.path = None; self.dirty = false; self.selected = None; self.fitted = false;
        self.message = format!("New {} × {} mm palmette. Undo returns to the previous one.", w, h);
    }
    /// Start again from a library structure, fitted to the page.
    pub fn use_structure(&mut self, id: &str) {
        let Some(d) = Design::from_structure(id, self.design.width, self.design.height) else { return };
        self.structure = id.into();
        self.change(d); self.selected = None;
        let name = STRUCTURES.iter().find(|s| s.0 == id).map_or(id, |s| s.1);
        self.message = format!("{name} fitted to the page. Click a motif to change it (its copies follow); Undo returns to the previous design.");
    }
    /// A picked file, once read (see `App::opened`, which sends palmette designs here).
    pub fn open_text(&mut self, picked: platform::Opened) {
        let (path, text) = match picked { Ok(p) => p, Err(e) => { self.message = format!("Could not open this file. {e}"); return } };
        match io::parse_palmette(&text) {
            Ok(d) => { self.change(d); self.path = Some(path); self.dirty = false; self.selected = None; self.fitted = false; self.message = "Palmette opened.".into(); }
            Err(e) => self.message = format!("Could not open this design. {e}"),
        }
    }
    pub fn save(&mut self, choose: bool) {
        let path = if choose || self.path.is_none() {
            match platform::choose_save("ORNATR palmette", &["ornatr"], "palmette.ornatr") { Some(p) => p, None => return }
        } else { self.path.clone().unwrap() };
        match platform::write_file(&path, &io::save_palmette(&self.design)) { Ok(()) => { self.path = Some(path); self.dirty = false; self.message = "Palmette saved.".into(); } Err(e) => self.message = format!("Could not save: {e}") }
    }
    pub fn export(&mut self) {
        let Some(p) = platform::choose_save("SVG", &["svg"], "palmette-pattern.svg") else { return };
        self.refresh();
        let svg = scroll_core::palmette::svg_of(&self.drawing, self.design.width, self.design.height);
        match platform::write_file(&p, &svg) { Ok(()) => self.message = format!("Exported {}.", platform::shown(&p)), Err(e) => self.message = format!("Could not export: {e}") }
    }

    // ---------- selection ----------
    fn selected_element(&self) -> Option<&Element> { self.design.get(self.selected?.0) }
    pub fn remove_selected(&mut self) {
        let Some((id, _)) = self.selected else { return };
        let mut d = self.design.clone();
        let label = d.get(id).map_or("Motif", |e| e.label());
        d.remove(id);
        self.change(d); self.selected = None;
        self.message = format!("{label} removed. Undo brings it back.");
    }
    pub fn restack(&mut self, front: bool) {
        let Some((id, _)) = self.selected else { return };
        let mut d = self.design.clone(); d.restack(id, front); self.change(d);
    }
    /// Add a motif of `what` and select it.
    fn add(&mut self, what: &str) {
        let mut d = self.design.clone();
        let Some(id) = d.add(what) else { return };
        let e = d.get(id).unwrap();
        let behind = d.index(id) == Some(0) && d.elements.len() > 1;
        self.message = format!("{} added{} at the centre. Drag it into place; its open handle turns and sizes it.", e.label(), if behind { " behind everything" } else { "" });
        self.change(d); self.selected = Some((id, 0));
    }

    // ---------- view ----------
    fn to_screen(&self, p: Point) -> Pos2 { Pos2::new(self.origin.x + p.x as f32 * self.zoom, self.origin.y + p.y as f32 * self.zoom) }
    fn to_mm(&self, s: Pos2) -> Point { pt(((s.x - self.origin.x) / self.zoom) as f64, ((s.y - self.origin.y) / self.zoom) as f64) }
    fn fit(&mut self, rect: Rect) {
        let (w, h) = (self.design.width as f32, self.design.height as f32);
        self.zoom = ((rect.width() - 80.0) / w).min((rect.height() - 80.0) / h).max(0.2);
        self.origin = Pos2::new(rect.center().x - w * self.zoom / 2.0, rect.center().y - h * self.zoom / 2.0);
        self.fitted = true;
    }
    pub fn fit_page(&mut self) { self.fitted = false; }

    /// The topmost motif (and copy) under `mm`.
    fn hit(&self, mm: Point) -> Option<(u32, usize)> {
        for (i, (owner, layer)) in self.layers.iter().enumerate().rev() {
            if !self.boxes.get(i).copied().flatten().is_some_and(|b| b.contains(mm)) { continue; }
            if let Owner::Element(id, k) = owner { if layer.cover.iter().any(|s| in_shape(mm, s)) { return Some((*id, *k)); } }
        }
        None
    }
    /// The selected copy's anchor and aim handle, in mm.
    fn handles(&self) -> Option<(Point, Point)> {
        let (id, k) = self.selected?;
        let e = self.design.get(id)?;
        let xf = *self.design.copies_of(e).get(k)?;
        Some((xf.point(e.anchor()), xf.point(aim_point(&e.kind))))
    }
    /// Apply the drag in progress with the pointer at `mm` (from the design the drag started on).
    fn drag_to(&mut self, mm: Point) {
        let Some(before) = self.drag_before.as_ref() else { return };
        let mut d = before.clone();
        match self.drag {
            Some(PDrag::Aim { id, copy }) => {
                let Some(xf) = d.get(id).and_then(|e| d.copies_of(e).get(copy).copied()) else { return };
                if let Some(e) = d.get_mut(id) { aim_to(&mut e.kind, xf.back(mm)); }
            }
            Some(PDrag::Body { id, copy, start }) => {
                let Some(xf) = d.get(id).and_then(|e| d.copies_of(e).get(copy).copied()) else { return };
                let (a, b) = (xf.back(start), xf.back(mm));
                if let Some(e) = d.get_mut(id) { e.shift(b.x - a.x, b.y - a.y); }
            }
            _ => return,
        }
        self.design = d;
    }

    /// The filled preview: the carved motifs in wood.
    fn raster(&self, k: f64) -> egui::ColorImage {
        let (w, h) = (((self.design.width * k).ceil() as usize).max(1), ((self.design.height * k).ceil() as usize).max(1));
        let mut px = vec![Color32::TRANSPARENT; w * h];
        for (_, rings) in &self.drawing.paint { fill_rings(&mut px, w, h, rings, k, WOOD); }
        egui::ColorImage { size: [w, h], pixels: px }
    }
    fn thumbs(&mut self) -> &Vec<Vec<Vec<Point>>> {
        self.thumbs.get_or_insert_with(|| STRUCTURES.iter().map(|(id, _, _)| {
            let d = structure(id).unwrap();
            let lines: Vec<Vec<Point>> = d.drawing().outlines.into_iter().filter(|l| l.len() > 1).map(|l| l.into_iter().step_by(3).collect()).collect();
            let all: Vec<Point> = lines.iter().flatten().copied().collect();
            let b = Bounds::of(&all); let s = (b.r - b.l).max(b.b - b.t).max(1e-6);
            let (ox, oy) = (b.l + ((b.r - b.l) - s) / 2.0, b.t + ((b.b - b.t) - s) / 2.0);
            lines.iter().map(|l| l.iter().map(|p| pt((p.x - ox) / s, (p.y - oy) / s)).collect()).collect()
        }).collect())
    }
}

fn along(p: Point, a: f64, l: f64) -> Point { pt(p.x + a.cos() * l, p.y + a.sin() * l) }
/// Where a motif's aim handle sits (in its own coordinates): out along its
/// direction at its size, or out to the right of round motifs.
fn aim_point(k: &Kind) -> Point {
    let up = -PI * 0.5;
    match k {
        Kind::Fan { base, axis, length, .. } => along(*base, *axis, *length),
        Kind::Petal { base, heading, length, .. } => along(*base, *heading, *length),
        Kind::Lotus { base, axis, height } => along(*base, *axis, *height),
        Kind::Stem { top, heading, length, .. } => along(*top, *heading, *length),
        Kind::Arm { start, heading, length, radius, .. } => along(*start, *heading, length.max(*radius * 0.5)),
        Kind::Collar { c, turn, width, .. } => along(*c, *turn, width / 2.0),
        Kind::Boss { c, r } => along(*c, 0.0, *r),
        Kind::Fleurette { c, radius, .. } => along(*c, up, *radius),
        Kind::Husks { top, heading, size, .. } => along(*top, *heading, 30.0 * size),
        Kind::PetalRing { c, root, length, .. } => along(*c, up, root + length),
        Kind::BeadedRing { c, r, width, .. } => along(*c, 0.0, r + width / 2.0),
    }
}
/// Turn and size a motif so its aim handle comes to `p` (in its own coordinates).
fn aim_to(k: &mut Kind, p: Point) {
    let from = |q: &Point| ((p.y - q.y).atan2(p.x - q.x), (p.x - q.x).hypot(p.y - q.y));
    match k {
        Kind::Fan { .. } => {
            // TODO(human): turn and size the palmette from its spear's tip handle
        }
        Kind::Petal { base, heading, length, .. } => { let (a, l) = from(base); *heading = a; *length = l.max(1.0); }
        Kind::Lotus { base, axis, height } => { let (a, l) = from(base); *axis = a; *height = l.max(2.0); }
        Kind::Stem { top, heading, length, .. } => { let (a, l) = from(top); *heading = a; *length = l.max(1.0); }
        Kind::Arm { start, heading, length, radius, .. } => { let (a, l) = from(start); *heading = a; *length = if l < *radius * 0.5 { 0.0 } else { l }; }
        Kind::Collar { c, turn, width, height } => { let (a, l) = from(c); *turn = a; *width = (l * 2.0).max(*height + 1.0); }
        Kind::Boss { c, r } => *r = from(c).1.max(0.5),
        Kind::Fleurette { c, radius, boss, .. } => *radius = from(c).1.max(*boss + 1.0),
        Kind::Husks { top, heading, size, .. } => { let (a, l) = from(top); *heading = a; *size = (l / 30.0).max(0.05); }
        Kind::PetalRing { c, root, length, .. } => *length = (from(c).1 - *root).max(2.0),
        Kind::BeadedRing { c, r, width, .. } => *r = (from(c).1 - *width / 2.0).max(1.0),
    }
}

fn ratio(ui: &mut egui::Ui, v: &mut f64, range: std::ops::RangeInclusive<f64>, label: &str) -> egui::Response {
    ui.add(egui::Slider::new(v, range).text(label).custom_formatter(|n, _| format!("{n:.2}")).clamping(egui::SliderClamping::Never))
}
fn count(ui: &mut egui::Ui, v: &mut usize, range: std::ops::RangeInclusive<usize>, label: &str) -> egui::Response { ui.add(egui::Slider::new(v, range).text(label)) }
/// How a motif repeats about the centre and along a row.
fn repeat_choice(ui: &mut egui::Ui, r: &mut Repeat, width: f64) {
    let names = ["Alone", "Mirrored left and right", "Mirrored top and bottom", "Mirrored four ways", "Round a ring"];
    let mut mode = if r.ring > 1 { 4 } else { match (r.mirror_x, r.mirror_y) { (false, false) => 0, (true, false) => 1, (false, true) => 2, (true, true) => 3 } };
    let before = mode;
    egui::ComboBox::from_label("Repeat").selected_text(names[mode]).show_ui(ui, |ui| { for (i, n) in names.iter().enumerate() { ui.selectable_value(&mut mode, i, *n); } });
    if mode != before {
        let (row, pitch) = (r.row, r.pitch);
        *r = Repeat { row, pitch, ..match mode { 0 => Repeat::ONE, 1 => Repeat::MIRROR, 2 => Repeat { mirror_y: true, ..Repeat::ONE }, 3 => Repeat { mirror_x: true, mirror_y: true, ..Repeat::ONE }, _ => Repeat { ring: 8, ..Repeat::ONE } } };
    }
    if r.ring > 1 {
        let mut n = r.ring as u32;
        if ui.add(egui::Slider::new(&mut n, 2..=24).text("Places")).changed() { r.ring = n as u8; }
        ui.checkbox(&mut r.mirror_x, "Each place a mirrored pair");
    }
    let mut row = r.row > 1;
    if ui.checkbox(&mut row, "Along a row").on_hover_text("Set out side by side to the right, as in an anthemion band").changed() {
        if row { r.row = 3; if r.pitch <= 0.0 { r.pitch = (width / 4.0).round().max(5.0); } } else { r.row = 1; }
    }
    if r.row > 1 {
        let mut n = r.row as u32;
        if ui.add(egui::Slider::new(&mut n, 2..=24).text("In the row")).changed() { r.row = n as u8; }
        mm_slider(ui, &mut r.pitch, 1.0..=width.max(10.0), "Spacing");
    }
}
fn tip_choice(ui: &mut egui::Ui, t: &Theme, label: &str, tip: &mut Tip) { ui.label(label); segmented(ui, t, &[(Tip::Pointed, "Pointed"), (Tip::Round, "Round")], tip); }

impl App {
    pub(crate) fn open_palmette(&mut self, ctx: &egui::Context) {
        if let Some(r) = platform::pick_text(ctx, OpenFor::Palmette, "ORNATR palmette", &["ornatr", "json"]) { self.opened(OpenFor::Palmette, r); }
    }
    pub(crate) fn palmette_shortcuts(&mut self, ctx: &egui::Context, undo: bool, redo: bool, save: bool, save_as: bool, open: bool, new: bool, typing: bool) {
        if undo { self.palmette.undo(); }
        if redo { self.palmette.redo(); }
        if save_as { self.palmette.save(true); } else if save { self.palmette.save(false); }
        if open { self.open_palmette(ctx); }
        if new { self.open_new_dialog(); }
        if typing { return; }
        let (del, esc) = ctx.input(|i| (i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace), i.key_pressed(egui::Key::Escape)));
        if del { self.palmette.remove_selected(); }
        if esc { self.palmette.selected = None; }
    }

    pub(crate) fn palmette_menus(&mut self, ui: &mut egui::Ui) {
        ui.menu_button("File", |ui| {
            if ui.add(egui::Button::new("New palmette…").shortcut_text("Ctrl+N")).clicked() { self.open_new_dialog(); ui.close_menu(); }
            if ui.add(egui::Button::new("Open palmette…").shortcut_text("Ctrl+O")).clicked() { ui.close_menu(); self.open_palmette(ui.ctx()); }
            if ui.add(egui::Button::new("Save").shortcut_text("Ctrl+S")).clicked() { ui.close_menu(); self.palmette.save(false); }
            if ui.add(egui::Button::new("Save As…").shortcut_text("Ctrl+Shift+S")).clicked() { ui.close_menu(); self.palmette.save(true); }
            ui.separator();
            if ui.button("Export pattern SVG…").clicked() { ui.close_menu(); self.palmette.export(); }
        });
        ui.menu_button("Edit", |ui| {
            if ui.add_enabled(self.palmette.can_undo(), egui::Button::new("Undo").shortcut_text("Ctrl+Z")).clicked() { self.palmette.undo(); ui.close_menu(); }
            if ui.add_enabled(self.palmette.can_redo(), egui::Button::new("Redo").shortcut_text("Ctrl+Y")).clicked() { self.palmette.redo(); ui.close_menu(); }
            ui.separator();
            let sel = self.palmette.selected.is_some();
            if ui.add_enabled(sel, egui::Button::new("Delete").shortcut_text("Del")).clicked() { self.palmette.remove_selected(); ui.close_menu(); }
            if ui.add_enabled(sel, egui::Button::new("Bring to front")).clicked() { self.palmette.restack(true); ui.close_menu(); }
            if ui.add_enabled(sel, egui::Button::new("Send to back")).clicked() { self.palmette.restack(false); ui.close_menu(); }
        });
        ui.menu_button("View", |ui| {
            if ui.button("Fit page").clicked() { self.palmette.fit_page(); ui.close_menu(); }
            ui.checkbox(&mut self.palmette.show_grid, "Millimetre grid");
            ui.checkbox(&mut self.palmette.filled, "Filled preview").on_hover_text("The carved motifs in wood");
            ui.separator();
            ui.menu_button("Theme", |ui| {
                for id in ThemeId::ALL { if ui.selectable_label(self.prefs.theme == id, id.theme().name).clicked() { self.set_prefs(Prefs { theme: id, ..self.prefs }); ui.close_menu(); } }
            });
            let mut shelf = self.prefs.shelf;
            if ui.checkbox(&mut shelf, "ZBrush-style layout").changed() { self.set_prefs(Prefs { shelf, ..self.prefs }); ui.close_menu(); }
        });
    }

    fn palmette_hint(&self) -> String {
        let st = &self.palmette;
        if !st.message.is_empty() { return st.message.clone(); }
        match st.selected_element() {
            None => "Click a motif to select it. Pick a starting layout from the library, or add palmettes, petals, volute arms and flowers.".into(),
            Some(e) => {
                let copies = e.repeat.count();
                let linked = if copies > 1 { format!(" · its {} copies follow", copies - 1) } else { String::new() };
                format!("{} selected · drag it to move it · drag its open handle to turn and size it{linked}", e.label())
            }
        }
    }
    fn palmette_state_text(&self) -> String {
        let d = &self.palmette.design;
        let copies: usize = d.elements.iter().map(|e| e.repeat.count()).sum();
        format!("{} × {} mm · {} motifs · {} drawn", d.width, d.height, d.elements.len(), copies)
    }
    pub(crate) fn palmette_status(&mut self, ctx: &egui::Context) {
        let t = self.t();
        let text = self.palmette_state_text();
        let hint = self.palmette_hint();
        egui::TopBottomPanel::bottom("palmette-status").frame(egui::Frame::none().fill(t.bg).inner_margin(egui::Margin::symmetric(14.0, 5.0))).show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.visuals_mut().override_text_color = Some(t.dim);
                ui.style_mut().override_text_style = Some(egui::TextStyle::Small);
                ui.label(text);
                if let Some(m) = self.palmette.cursor_mm { ui.separator(); ui.label(format!("x {:.1}  y {:.1} mm", m.x, m.y)); }
                ui.separator();
                ui.label(hint);
            });
        });
    }

    /// The classic layout's panel: the library, adding, the selection, the page, the theme.
    pub(crate) fn palmette_side_panel(&mut self, ctx: &egui::Context) {
        let t = self.t();
        egui::SidePanel::right("palmette-panel").default_width(340.0).min_width(300.0).frame(egui::Frame::none().fill(t.bg).inner_margin(egui::Margin { left: 16.0, right: 12.0, top: 12.0, bottom: 8.0 })).show(ctx, |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                ui.set_width(ui.available_width() - 4.0);
                section(ui, t, "Library");
                ui.horizontal_wrapped(|ui| {
                    for (id, name, detail) in STRUCTURES { if ui.selectable_label(self.palmette.structure == id, name).on_hover_text(detail).clicked() { self.palmette.use_structure(id); } }
                });
                section(ui, t, "Add");
                self.palmette_add(ui);
                section(ui, t, "Selected");
                self.palmette_selected(ui);
                section(ui, t, "Page");
                self.palmette_page(ui);
                section(ui, t, "View");
                ui.checkbox(&mut self.palmette.filled, "Filled preview");
                ui.checkbox(&mut self.palmette.show_grid, "Millimetre grid");
                if ui.button("Export pattern SVG…").clicked() { self.palmette.export(); }
                self.theme_tab(ui);
                ui.add_space(12.0);
            });
        });
    }

    fn palmette_add(&mut self, ui: &mut egui::Ui) {
        let t = self.t();
        ui.horizontal_wrapped(|ui| { for (what, name, tip) in ADD { if ui.button(name).on_hover_text(tip).clicked() { self.palmette.add(what); } } });
        ui.label(egui::RichText::new("New motifs go at the centre. Stems, arms and petal rings go behind everything.").small().color(t.dim));
    }

    /// The selected motif's settings.
    fn palmette_selected(&mut self, ui: &mut egui::Ui) {
        let t = self.t();
        let st = &self.palmette;
        let Some(e) = st.selected_element().cloned() else {
            ui.label(egui::RichText::new("Click a motif on the page to select it. Its mirrored and repeated copies follow when you change it.").small().color(t.dim));
            return;
        };
        let copies = e.repeat.count();
        ui.label(egui::RichText::new(if copies > 1 { format!("{} · {} copies", e.label(), copies) } else { e.label().to_string() }).strong());
        let mut k = e.kind.clone();
        let mut repeat = e.repeat;
        match &mut k {
            Kind::Fan { axis, count: n, spread, bend, flick, length, decay, belly, roll, root, tip, centre_tip, .. } => {
                ui.add(egui::Slider::new(n, 1..=21).step_by(2.0).text("Petals")).on_hover_text("An odd count: the spear in the middle and pairs either side");
                mm_slider(ui, length, 5.0..=400.0, "Height");
                angle(ui, axis, "Angle");
                let mut f = *spread;
                if ui.add(egui::Slider::new(&mut f, 0.0..=90.0).text("Fan").suffix("°").custom_formatter(|n, _| format!("{n:.0}")).clamping(egui::SliderClamping::Never)).on_hover_text("How far the outer petals start off the axis").changed() { *spread = f; }
                ratio(ui, bend, 0.0..=2.0, "Bend out").on_hover_text("How far the side petals bend out, most at the root");
                ratio(ui, flick, -0.5..=1.5, "Flick back").on_hover_text("How far their tips turn back up (the S)");
                ratio(ui, roll, -2.0..=2.0, "Roll").on_hover_text("The outer petals' tips rolled over");
                ratio(ui, decay, 0.0..=0.9, "Side petals shorter");
                ratio(ui, belly, 0.06..=0.35, "Petal width");
                mm_slider(ui, root, 0.0..=40.0, "Waist width");
                tip_choice(ui, t, "Side petal tips", tip);
                tip_choice(ui, t, "Spear tip", centre_tip);
            }
            Kind::Petal { heading, length, belly, bend, flick, roll, tip, .. } => {
                mm_slider(ui, length, 2.0..=300.0, "Length");
                angle(ui, heading, "Angle");
                ratio(ui, belly, 0.06..=0.45, "Width");
                ratio(ui, bend, -3.0..=3.0, "Bend").on_hover_text("Over its first 60%, most at the root");
                ratio(ui, flick, -3.0..=3.0, "Flick").on_hover_text("The tip turning back");
                ratio(ui, roll, -2.0..=2.0, "Roll");
                tip_choice(ui, t, "Tip", tip);
            }
            Kind::Lotus { axis, height, .. } => { mm_slider(ui, height, 5.0..=300.0, "Height"); angle(ui, axis, "Angle"); }
            Kind::Stem { heading, length, w_top, w_foot, .. } => {
                mm_slider(ui, length, 1.0..=300.0, "Length");
                angle(ui, heading, "Angle");
                size_slider(ui, w_top, 0.4..=40.0, "Width at the top");
                size_slider(ui, w_foot, 0.4..=40.0, "Width at the foot");
            }
            Kind::Arm { heading, length, curve, radius, side, width, .. } => {
                mm_slider(ui, length, 0.0..=300.0, "Sweep length");
                angle(ui, heading, "Angle");
                let mut c = *curve * 1000.0;
                if ui.add(egui::Slider::new(&mut c, -60.0..=60.0).text("Sweep curve").custom_formatter(|n, _| format!("{n:.0}")).clamping(egui::SliderClamping::Never)).on_hover_text("How much the sweep bends before the volute (0 straight)").changed() { *curve = c / 1000.0; }
                mm_slider(ui, radius, 2.0..=120.0, "Volute size");
                size_slider(ui, width, 0.5..=40.0, "Width");
                ui.label("The volute turns");
                segmented(ui, t, &[(1.0, "Clockwise"), (-1.0, "Anticlockwise")], side);
            }
            Kind::Collar { turn, width, height, .. } => { mm_slider(ui, width, 2.0..=300.0, "Width"); mm_slider(ui, height, 1.0..=60.0, "Height"); angle(ui, turn, "Angle"); }
            Kind::Boss { r, .. } => { size_slider(ui, r, 1.0..=120.0, "Size"); }
            Kind::Fleurette { count: n, radius, belly, tip, back, boss, .. } => {
                count(ui, n, 3..=24, "Petals");
                size_slider(ui, radius, 4.0..=400.0, "Size");
                ratio(ui, belly, 0.06..=0.5, "Petal width");
                ratio(ui, back, 0.0..=1.2, "Back ring").on_hover_text("A second ring behind, turned half a step: its size against the front ring (0: none)");
                size_slider(ui, boss, 1.0..=80.0, "Boss size");
                tip_choice(ui, t, "Tips", tip);
            }
            Kind::Husks { heading, count: n, size, shrink, tear, .. } => {
                count(ui, n, 1..=12, "Husks");
                let mut l = *size * 30.0;
                if mm_slider(ui, &mut l, 3.0..=200.0, "First husk").changed() { *size = (l / 30.0).max(0.05); }
                ratio(ui, shrink, 0.5..=1.0, "Each one smaller");
                angle(ui, heading, "Angle");
                ui.checkbox(tear, "A teardrop at the end");
            }
            Kind::PetalRing { count: n, root, length, belly, offset, notch, lip, .. } => {
                count(ui, n, 3..=32, "Petals");
                mm_slider(ui, root, 0.0..=200.0, "Inner radius");
                mm_slider(ui, length, 2.0..=300.0, "Petal length");
                ratio(ui, belly, 0.06..=0.5, "Petal width");
                ratio(ui, offset, 0.0..=1.0, "Turned").on_hover_text("Turned by a share of one petal's step");
                ui.checkbox(notch, "Heart-notched tips");
                ui.checkbox(lip, "Raised lip round the edge");
            }
            Kind::BeadedRing { r, width, bead, .. } => { mm_slider(ui, r, 2.0..=400.0, "Radius"); mm_slider(ui, width, 0.5..=60.0, "Band width"); size_slider(ui, bead, 0.4..=30.0, "Bead size"); }
        }
        repeat_choice(ui, &mut repeat, self.palmette.design.width);
        if k != e.kind || repeat != e.repeat {
            let mut d = self.palmette.design.clone();
            if let Some(x) = d.get_mut(e.id) { x.kind = k; x.repeat = repeat; }
            self.palmette.change_keyed(Some(format!("edit-{}", e.id)), d);
            self.palmette.check_selection();
        }
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            if ui.button("Bring to front").clicked() { self.palmette.restack(true); }
            if ui.button("Send to back").clicked() { self.palmette.restack(false); }
            if ui.button("Delete").clicked() { self.palmette.remove_selected(); }
        });
    }

    fn palmette_page(&mut self, ui: &mut egui::Ui) {
        let t = self.t();
        let mut inches = self.prefs.inches;
        let d = &self.palmette.design;
        let picked = page_sizes::size_editor(ui, t, &mut self.page_sizes, &mut inches, d.width, d.height, 40.0..=1000.0, false);
        if inches != self.prefs.inches { self.set_prefs(Prefs { inches, ..self.prefs }); }
        if let Some((w, h)) = picked {
            // the design stays centred on the resized page
            let mut d = self.palmette.design.clone();
            d.map(1.0, pt((w - d.width) / 2.0, (h - d.height) / 2.0));
            d.width = w; d.height = h;
            self.palmette.change(d); self.palmette.fitted = false;
        }
        ui.horizontal(|ui| {
            if ui.button("Fit design to page").on_hover_text("Scale and centre the design on the page").clicked() {
                let mut d = self.palmette.design.clone(); d.fit(); self.palmette.change(d);
            }
            if ui.button("Fit page to design").on_hover_text("Size the page round the design with a margin").clicked() {
                if let Some(b) = bounds_of(&self.palmette.layers) {
                    let m = 10.0; let mut d = self.palmette.design.clone();
                    d.map(1.0, pt(m - b.l, m - b.t));
                    d.width = ((b.r - b.l + 2.0 * m).ceil()).clamp(40.0, 1000.0); d.height = ((b.b - b.t + 2.0 * m).ceil()).clamp(40.0, 1000.0);
                    self.palmette.change(d); self.palmette.fitted = false;
                }
            }
        });
    }

    pub(crate) fn palmette_canvas(&mut self, ui: &mut egui::Ui) {
        self.palmette.refresh();
        self.palmette.autosave();
        let cc = self.canvas_colors();
        let (resp, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
        let rect = resp.rect;
        let st = &mut self.palmette;
        if !st.fitted && rect.width() > 120.0 && rect.height() > 120.0 { st.fit(rect); }
        if let Some(hover) = resp.hover_pos() {
            let (scroll, zoom_delta) = ui.input(|i| (i.smooth_scroll_delta.y, i.zoom_delta()));
            let factor = if zoom_delta != 1.0 { zoom_delta } else { (scroll * 0.0015).exp() };
            if (factor - 1.0).abs() > 1e-4 { let before = st.to_mm(hover); st.zoom = (st.zoom * factor).clamp(0.2, 60.0); let after = st.to_screen(before); st.origin += hover - after; }
            st.cursor_mm = Some(st.to_mm(hover));
        }
        let (pw, ph) = (st.design.width, st.design.height);
        let page = Rect::from_min_max(st.to_screen(pt(0.0, 0.0)), st.to_screen(pt(pw, ph)));
        for (grow, alpha) in [(10.0, 10u8), (5.0, 16), (2.0, 24)] { painter.rect_filled(page.expand(grow).translate(Vec2::new(0.0, grow * 0.4)), grow + 2.0, Color32::from_black_alpha(alpha)); }
        painter.rect_filled(page, 2.0, cc.paper);
        if st.show_grid && 10.0 * st.zoom > 6.0 {
            let mut v = 0.0; let mut k = 0;
            while v <= pw.max(ph) + 1e-9 {
                let w = if k % 5 == 0 { 0.9 } else { 0.5 };
                if v <= pw { let a = st.to_screen(pt(v, 0.0)); painter.line_segment([Pos2::new(a.x, page.top()), Pos2::new(a.x, page.bottom())], Stroke::new(w, cc.grid)); }
                if v <= ph { let b = st.to_screen(pt(0.0, v)); painter.line_segment([Pos2::new(page.left(), b.y), Pos2::new(page.right(), b.y)], Stroke::new(w, cc.grid)); }
                v += 10.0; k += 1;
            }
        }
        // the filled preview, redrawn when the design changes or the zoom moves on
        if st.filled {
            let k = ((st.zoom * ui.ctx().pixels_per_point()) as f64).min(3000.0 / pw.max(ph)).max(0.5);
            let redo = st.texture_stale || st.texture.as_ref().is_none_or(|(_, tk)| (k / tk - 1.0).abs() > 0.2);
            if redo {
                let img = st.raster(k);
                match st.texture.as_mut() { Some((tex, tk)) => { tex.set(img, egui::TextureOptions::LINEAR); *tk = k; } None => st.texture = Some((ui.ctx().load_texture("palmette-filled", img, egui::TextureOptions::LINEAR), k)) }
                st.texture_stale = false;
            }
            if let Some((tex, _)) = &st.texture { painter.image(tex.id(), page, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE); }
        }
        let ink = if st.filled { CARVED_INK } else { cc.ink };
        let sw = (0.3 * st.zoom).clamp(0.8, 2.0);
        for run in &st.drawing.outlines { stroke_runs(&painter, run.iter().map(|p| st.to_screen(*p)), false, Stroke::new(sw, ink)); }
        for run in &st.drawing.lines { stroke_runs(&painter, run.iter().map(|p| st.to_screen(*p)), false, Stroke::new(sw * 0.6, with_alpha(ink, 210))); }
        // hover and selection outlines: the picked copy strong, its other copies faint
        let hover = resp.hover_pos();
        let hovered = if st.drag.is_none() { hover.and_then(|h| st.hit(st.to_mm(h))) } else { None };
        let outline = |owner: (u32, usize)| -> Vec<Vec<Point>> { st.layers.iter().filter(|(o, _)| *o == Owner::Element(owner.0, owner.1)).flat_map(|(_, l)| l.cover.iter().flatten().cloned()).collect() };
        let mut marks: Vec<((u32, usize), Color32, f32)> = vec![];
        if let Some(h) = hovered.filter(|h| Some(h.0) != st.selected.map(|s| s.0)) { marks.push((h, with_alpha(cc.mark, 110), 1.2)); }
        if let Some((id, k)) = st.selected {
            let n = st.design.get(id).map_or(0, |e| e.repeat.count());
            for c in (0..n).filter(|c| *c != k) { marks.push(((id, c), with_alpha(cc.mark, 90), 1.0)); }
            marks.push(((id, k), cc.mark, 1.8));
        }
        for (owner, col, w) in marks { for ring in outline(owner) { if ring.len() > 2 { stroke_runs(&painter, ring.iter().map(|q| st.to_screen(*q)), true, Stroke::new(w, col)); } } }
        // the anchor (solid) and the aim handle (open), joined by a dashed line
        let handles = st.handles().map(|(a, b)| (st.to_screen(a), st.to_screen(b)));
        let hot = |p: Pos2| hover.is_some_and(|q| q.distance(p) < 10.0);
        if let Some((a, b)) = handles {
            painter.extend(Shape::dashed_line(&[a, b], Stroke::new(1.0, cc.guide), 5.0, 4.0));
            painter.circle(a, 4.0, cc.mark, Stroke::new(1.5, cc.paper));
            painter.circle(b, 5.5 + if hot(b) { 1.5 } else { 0.0 }, cc.paper, Stroke::new(2.0, cc.mark));
        }

        // ----- interaction -----
        let pan_button = ui.input(|i| i.pointer.middle_down() || i.pointer.secondary_down() || i.key_down(egui::Key::Space));
        if resp.drag_started() {
            let pos = ui.input(|i| i.pointer.press_origin()).or(resp.interact_pointer_pos()).unwrap_or_default();
            let mm = st.to_mm(pos);
            st.drag_before = Some(st.design.clone());
            st.drag = if pan_button { Some(PDrag::Pan) }
                else if let (Some((id, copy)), Some((_, b))) = (st.selected, handles.filter(|(_, b)| b.distance(pos) < 10.0)) { let _ = b; Some(PDrag::Aim { id, copy }) }
                else if let Some((id, copy)) = st.hit(mm) { st.selected = Some((id, copy)); Some(PDrag::Body { id, copy, start: mm }) }
                else { None };
            st.message.clear();
        }
        if resp.dragged() {
            match st.drag {
                Some(PDrag::Pan) => st.origin += resp.drag_delta(),
                Some(_) => if let Some(pos) = resp.interact_pointer_pos() { let mm = st.to_mm(pos); st.drag_to(mm); },
                None => {}
            }
        }
        if resp.drag_stopped() {
            if let Some(before) = st.drag_before.take() {
                if before != st.design && !matches!(st.drag, Some(PDrag::Pan)) { st.past.push(before); if st.past.len() > 80 { st.past.remove(0); } st.future.clear(); st.dirty = true; st.last_edit = None; }
            }
            st.drag = None;
            ui.ctx().request_repaint();
        }
        if resp.clicked() {
            if let Some(pos) = resp.interact_pointer_pos() {
                if !handles.is_some_and(|(_, b)| b.distance(pos) < 10.0) { st.selected = st.hit(st.to_mm(pos)); }
                st.message.clear();
            }
        }
        if handles.is_some_and(|(_, b)| hot(b)) { ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair); }
        else if hovered.is_some() { ui.ctx().set_cursor_icon(egui::CursorIcon::Move); }
    }
}

// ---- the shelf layout (ZBrush-style) for the Palmette workspace ------------

/// The motifs on the top shelf (the rings are in the Add palette).
const SHELF_ADD: [&str; 9] = ["palmette", "lotus", "petal", "stem", "arm", "collar", "boss", "fleurette", "husks"];

impl App {
    /// Add buttons, the view and Export.
    pub(crate) fn palmette_shelf_context(&mut self, ctx: &egui::Context) {
        let t = self.t();
        egui::TopBottomPanel::top("palmette-shelf").exact_height(58.0).frame(egui::Frame::none().fill(t.panel).inner_margin(egui::Margin::symmetric(10.0, 0.0)).stroke(Stroke::new(1.0, t.border))).show(ctx, |ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                if big_tab(ui, t, "Export", false).on_hover_text("Save the pattern at actual size as SVG").clicked() { self.palmette.export(); }
                ui.add_space(8.0); divider(ui, t); ui.add_space(8.0);
                ui.allocate_ui_with_layout(Vec2::new(124.0, 50.0), egui::Layout::top_down(egui::Align::Min), |ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    ui.add_space(3.0);
                    ui.label(egui::RichText::new("VIEW").size(10.5).color(t.dim));
                    let mut filled = self.palmette.filled;
                    if seg(ui, t, &[(false, "Lines"), (true, "Filled")], &mut filled, 58.0, 22.0) { self.palmette.filled = filled; }
                });
                ui.add_space(4.0); divider(ui, t); ui.add_space(8.0);
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    sh::tight_controls(ui, |ui| {
                        caption(ui, t, "Add");
                        for (what, name, tip) in ADD.iter().filter(|a| SHELF_ADD.contains(&a.0)) {
                            let w = 18.0 + name.len() as f32 * 7.5;
                            if bevel_button(ui, t, name, false, Vec2::new(w, 38.0)).on_hover_text(*tip).clicked() { self.palmette.add(what); }
                        }
                    });
                });
            });
        });
    }

    /// The library shelf: the starting layouts, drawn from their outlines.
    pub(crate) fn palmette_shelf_tools(&mut self, ctx: &egui::Context) {
        let t = self.t();
        egui::SidePanel::left("palmette-shelf-tools").exact_width(84.0).resizable(false).frame(egui::Frame::none().fill(t.panel).inner_margin(egui::Margin::symmetric(8.0, 10.0)).stroke(Stroke::new(1.0, t.border))).show(ctx, |ui| {
            egui::ScrollArea::vertical().scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden).show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 6.0;
                shelf_caption(ui, t, "LIBRARY");
                let thumbs = self.palmette.thumbs().clone();
                for ((id, name, detail), lines) in STRUCTURES.iter().zip(thumbs) {
                    let on = self.palmette.structure == *id;
                    let resp = tile_with(ui, t, name, on, |p, r, c| {
                        let r = r.expand(4.0);
                        let at = |q: &Point| Pos2::new(r.left() + q.x as f32 * r.width(), r.top() + q.y as f32 * r.height());
                        for l in &lines { p.add(Shape::line(l.iter().map(at).collect(), Stroke::new(0.8, c))); }
                    }).on_hover_text(*detail);
                    if resp.clicked() { self.palmette.use_structure(id); }
                }
            });
        });
    }

    /// Folding palettes: the selection, adding, the page and the canvas.
    pub(crate) fn palmette_shelf_tray(&mut self, ctx: &egui::Context) {
        let t = self.t();
        egui::SidePanel::right("palmette-shelf-tray").default_width(340.0).min_width(300.0).frame(egui::Frame::none().fill(t.bg).inner_margin(egui::Margin { left: 8.0, right: 6.0, top: 8.0, bottom: 8.0 })).show(ctx, |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                ui.set_width(ui.available_width() - 6.0);
                ui.spacing_mut().item_spacing.y = 6.0;
                let title = self.palmette.selected_element().map_or("Selection".to_string(), |e| format!("Selected {}", e.label().to_lowercase()));
                self.palette(ui, sh::PALMETTE_SELECTION, &title, |s, ui| s.palmette_selected(ui));
                self.palette(ui, sh::PALMETTE_ADD, "Add", |s, ui| s.palmette_add(ui));
                self.palette(ui, sh::PALMETTE_PAGE, "Page", |s, ui| s.palmette_page(ui));
                self.palette(ui, sh::CANVAS, "Canvas", |s, ui| { ui.checkbox(&mut s.palmette.show_grid, "Millimetre grid"); s.theme_tab(ui) });
            });
        });
    }

    pub(crate) fn palmette_shelf_status(&mut self, ctx: &egui::Context) {
        let t = self.t();
        let hint = self.palmette_hint();
        let text = self.palmette_state_text();
        egui::TopBottomPanel::bottom("palmette-shelf-status").exact_height(28.0).frame(egui::Frame::none().fill(t.bg).inner_margin(egui::Margin::symmetric(14.0, 0.0))).show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if sh::small_button(ui, t, "Fit").on_hover_text("Fit the page in the window").clicked() { self.palmette.fit_page(); }
                    ui.label(egui::RichText::new(format!("Zoom {:.0}%", self.palmette.zoom / 3.78 * 100.0)).size(12.5).color(t.dim));
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        let st = &self.palmette;
                        let s = |x: String| egui::RichText::new(x).size(12.5).color(t.dim);
                        ui.label(s(text)); ui.add_space(12.0);
                        if let Some(m) = st.cursor_mm { ui.label(s(format!("x {:.1}  y {:.1} mm", m.x, m.y))); ui.add_space(12.0); }
                        ui.add(egui::Label::new(s(hint)).truncate());
                    });
                });
            });
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cartouche_ui::wrap_angle;
    fn state(id: &str) -> PalmetteState {
        let mut st = PalmetteState::new();
        st.design = Design::from_structure(id, 160.0, 200.0).unwrap();
        st.refresh();
        st
    }
    /// Drag as the canvas does, from the design the drag started on.
    fn drag(st: &mut PalmetteState, drag: PDrag, to: Point) {
        st.drag_before = Some(st.design.clone()); st.drag = Some(drag);
        st.drag_to(to);
        st.drag = None; st.refresh();
    }

    #[test]
    fn dragging_a_mirrored_copy_moves_the_motif_the_other_way() {
        let mut st = state("classical");
        let calyx = st.design.elements.iter().find(|e| e.repeat == Repeat::MIRROR).unwrap().id;
        let a0 = st.design.get(calyx).unwrap().anchor();
        let xf = st.design.copies_of(st.design.get(calyx).unwrap())[1];
        let start = xf.point(a0);
        // the mirrored copy dragged 5 mm right: the motif itself goes 5 mm left
        drag(&mut st, PDrag::Body { id: calyx, copy: 1, start }, pt(start.x + 5.0, start.y + 2.0));
        let a1 = st.design.get(calyx).unwrap().anchor();
        assert!((a1.x - (a0.x - 5.0)).abs() < 1e-9 && (a1.y - (a0.y + 2.0)).abs() < 1e-9, "{a0:?} -> {a1:?}");
    }

    #[test]
    fn clicking_picks_the_motif_on_top_by_copy() {
        let st = state("anthemion");
        // every palmette in the band picks the fan element, by its copy
        let fan = st.design.elements.iter().find(|e| matches!(e.kind, Kind::Fan { .. })).unwrap();
        let tips: Vec<Point> = st.design.copies_of(fan).iter().map(|x| { let Kind::Fan { base, length, .. } = fan.kind else { unreachable!() }; x.point(pt(base.x, base.y - length * 0.6)) }).collect();
        for (k, p) in tips.iter().enumerate() { assert_eq!(st.hit(*p), Some((fan.id, k))); }
        // empty page picks nothing
        assert_eq!(st.hit(pt(1.0, 1.0)), None);
    }

    #[test]
    fn the_aim_handle_turns_and_sizes_a_petal_on_any_copy() {
        let mut st = state("fleur");
        let side = st.design.elements.iter().find(|e| e.repeat == Repeat::MIRROR && matches!(e.kind, Kind::Petal { .. })).unwrap().id;
        for copy in [0, 1] {
            st.selected = Some((side, copy));
            let (anchor, _) = st.handles().unwrap();
            // straight up from the copy's root, 50 mm
            drag(&mut st, PDrag::Aim { id: side, copy }, pt(anchor.x, anchor.y - 50.0));
            let Kind::Petal { heading, length, .. } = st.design.get(side).unwrap().kind else { unreachable!() };
            assert!((length - 50.0).abs() < 1e-9 && (wrap_angle(heading) + PI / 2.0).abs() < 1e-9, "copy {copy}: {heading} {length}");
            let (_, aim) = st.handles().unwrap();
            assert!((aim.x - anchor.x).abs() < 1e-9 && (aim.y - (anchor.y - 50.0)).abs() < 1e-9);
        }
    }

    #[test]
    fn every_motif_can_be_added_drawn_and_aimed() {
        let mut st = PalmetteState::new();
        st.design = Design::empty(200.0, 200.0);
        for (what, _, _) in ADD {
            st.add(what);
            let (id, _) = st.selected.unwrap();
            st.refresh();
            assert!(st.layers.iter().any(|(o, _)| *o == Owner::Element(id, 0)), "{what} draws");
            let k = st.design.get(id).unwrap().kind.clone();
            let mut k2 = k.clone();
            aim_to(&mut k2, aim_point(&k));
            if !matches!(k, Kind::Fan { .. }) { assert!((aim_point(&k2).x - aim_point(&k).x).abs() < 1e-6 && (aim_point(&k2).y - aim_point(&k).y).abs() < 1e-6, "{what}: its handle stays put"); }
        }
    }
}
