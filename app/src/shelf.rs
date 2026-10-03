//! The ZBrush-style layout of the scroll workspace: a top shelf of tools and
//! the controls for what is selected, a left shelf of growth types, a right
//! tray of folding palettes and a status bar. It reuses the classic panel's
//! sections inside the palettes; View → ZBrush-style layout switches back.
use super::*;
use egui::{Align2, FontFamily, FontId, Rounding};

pub const PALETTES: usize = 14;
pub const CHIP_BOX: usize = 9;
pub const CHIP_COMPOSE: usize = 10;
pub const CHIP_FILL: usize = 11;
pub const CHIP_PAGE: usize = 12;
pub const CHIP_PRESETS: usize = 13;
pub const SELECTION: usize = 0;
pub const TRANSFORM: usize = 1;
pub const BACKBONE: usize = 2;
pub const CONSTRUCTION: usize = 3;
pub const LIBRARY: usize = 4;
pub const LAYERS: usize = 5;
pub const PAGE: usize = 6;
pub const CANVAS: usize = 7;
pub const CARVING: usize = 8;
pub const DEFAULT_OPEN: [bool; PALETTES] = [true, true, true, false, false, false, false, false, true, false, true, true, false, false];

fn semibold(size: f32) -> FontId { FontId::new(size, FontFamily::Name("semibold".into())) }

impl App {
    /// Wordmark, menus, workspace switch and the document name.
    pub(crate) fn shelf_top_bar(&mut self, ctx: &egui::Context) {
        let t = self.t();
        egui::TopBottomPanel::top("shelf-menu").frame(egui::Frame::none().fill(t.bg).inner_margin(egui::Margin::symmetric(14.0, 6.0))).show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.label(egui::RichText::new("ORNATR").font(semibold(18.0)).color(t.text));
                ui.add_space(14.0);
                if self.workspace == Workspace::Chip { self.chip_menus(ui); } else { self.scroll_menus(ui); }
                ui.add_space(14.0);
                let mut ws = self.workspace;
                if seg(ui, t, &[(Workspace::Scroll, "Scroll"), (Workspace::Chip, "Chip")], &mut ws, 72.0, 24.0) { self.workspace = ws; self.set_prefs(Prefs { chip: ws == Workspace::Chip, ..self.prefs }); }
                let (name, dirty) = if self.workspace == Workspace::Chip {
                    let title = self.chip.title();
                    (title.split(" — ").next().unwrap_or("").trim_end_matches(" •").to_string(), title.contains(" •"))
                } else {
                    (self.path.as_ref().and_then(|p| p.file_stem()).map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "Untitled".into()), self.dirty)
                };
                let r = ui.max_rect();
                let painter = ui.painter();
                let g = painter.layout_no_wrap(name, FontId::proportional(14.5), t.text);
                let x = r.center().x - g.size().x / 2.0;
                painter.galley(Pos2::new(x, r.center().y - g.size().y / 2.0), g.clone(), t.text);
                painter.text(Pos2::new(x + g.size().x + 10.0, r.center().y), Align2::LEFT_CENTER, if dirty { "·  Unsaved" } else { "·  Saved" }, FontId::proportional(13.0), t.dim);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| { ui.label(egui::RichText::new("Runs offline · no AI").small().color(t.dim)); });
            });
        });
    }

    /// Tools on the left, the selection's main controls in the middle, root
    /// joins and the Design / Carve / Export switch on the right.
    pub(crate) fn shelf_context(&mut self, ctx: &egui::Context) {
        let t = self.t();
        egui::TopBottomPanel::top("shelf").exact_height(58.0).frame(egui::Frame::none().fill(t.panel).inner_margin(egui::Margin::symmetric(10.0, 0.0)).stroke(Stroke::new(1.0, t.border))).show(ctx, |ui| {
            // the right-hand group is laid out first so a narrow window clips the sliders, not the tabs
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                let export = big_tab(ui, t, "Export", false).on_hover_text("Save the pattern or the carving guides as SVG at actual size");
                let popup = ui.id().with("export-menu");
                if export.clicked() { ui.memory_mut(|m| m.toggle_popup(popup)); }
                egui::popup_below_widget(ui, popup, &export, egui::PopupCloseBehavior::CloseOnClick, |ui| {
                    ui.set_min_width(200.0);
                    if ui.button("Pattern SVG…").clicked() { self.export(false); }
                    if ui.button("Carving guides SVG…").clicked() { self.export(true); }
                });
                let carve = self.tab == Tab::Carving;
                if big_tab(ui, t, "Carve", carve).on_hover_text("Carving guides: suggested ridges and creases").clicked() && !carve { self.tab = Tab::Carving; self.carving = true; self.stale = true; }
                if big_tab(ui, t, "Design", !carve).clicked() && carve { self.tab = Tab::Properties; self.carving = false; self.stale = true; }
                ui.add_space(8.0); divider(ui, t); ui.add_space(8.0);
                // root joins (drawn right to left: fillet first)
                let exact = self.prefs.joins == Joins::Exact;
                ui.add_enabled_ui(exact, |ui| {
                    let mut fillet = self.fillet_draft as f64;
                    let resp = zslider(ui, t, "Fillet", &mut fillet, 0.3, 1.5, |v| format!("{v:.1} mm"), 132.0).on_hover_text("Radius of the rounded crotch where a leaf or branch leaves its stem (Exact joins)");
                    // redrawing takes a moment, so apply on release
                    self.fillet_draft = ((fillet * 10.0).round() / 10.0) as f32;
                    if resp.drag_stopped() || (resp.changed() && !resp.dragged()) { self.set_prefs(Prefs { fillet: self.fillet_draft, ..self.prefs }); self.stale = true; }
                });
                ui.allocate_ui_with_layout(Vec2::new(184.0, 50.0), egui::Layout::top_down(egui::Align::Min), |ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    ui.add_space(3.0);
                    ui.label(egui::RichText::new("ROOT JOIN").size(10.5).color(t.dim));
                    let mut joins = self.prefs.joins;
                    if seg(ui, t, &[(Joins::Exact, "Exact"), (Joins::Smooth, "Smooth"), (Joins::Classic, "Classic")], &mut joins, 60.0, 22.0) { self.set_prefs(Prefs { joins, ..self.prefs }); self.stale = true; }
                });
                ui.add_space(4.0); divider(ui, t); ui.add_space(8.0);
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    for (tool, name, tip) in [(Tool::Select, "Select", "Select  (V)\nClick a leaf or a backbone; drag leaves and handles.\nOver the canvas, hold B to pick backbones, L leaves, C collars"), (Tool::Pen, "Draw", "Draw backbone  (P)\nSketch a sweep; start on a stem to grow from it"), (Tool::Transform, "Transform", "Transform  (T)\nMove, scale, rotate or flip a backbone with its leaves")] {
                        if bevel_button(ui, t, name, self.tool == tool, Vec2::new(if tool == Tool::Transform { 84.0 } else { 66.0 }, 38.0)).on_hover_text(tip).clicked() { self.tool = tool; if tool == Tool::Transform { self.selected = None; } }
                    }
                    let lib = self.palettes[LIBRARY] && self.tab != Tab::Carving;
                    if bevel_button(ui, t, "Leaf", lib, Vec2::new(58.0, 38.0)).on_hover_text("Leaf library: grow a measured leaf or bud from the backbone").clicked() { self.palettes[LIBRARY] = !lib; if !lib { self.tab = Tab::Properties; self.carving = false; self.stale = true; } }
                    ui.add_space(8.0); divider(ui, t); ui.add_space(8.0);
                    ui.spacing_mut().item_spacing.x = 8.0;
                    tight_controls(ui, |ui| self.context_controls(ui));
                });
            });
        });
    }

    /// The few controls that matter most for what is being edited.
    fn context_controls(&mut self, ui: &mut egui::Ui) {
        let t = self.t();
        if self.tool == Tool::Transform {
            caption(ui, t, "Transform");
            if bevel_button(ui, t, "Flip horizontal", false, Vec2::new(124.0, 30.0)).clicked() { self.flip(Axis::Horizontal); }
            if bevel_button(ui, t, "Flip vertical", false, Vec2::new(110.0, 30.0)).clicked() { self.flip(Axis::Vertical); }
            return;
        }
        if let Some(part) = self.selected_part() {
            let sh = part.shoot.clone().unwrap();
            let vine = sh.preset.as_deref() == Some(VINE_CURL);
            let bud = sh.preset.as_deref().is_some_and(is_bud);
            let measured = sh.preset.as_deref().and_then(profile);
            caption(ui, t, if vine { "Curl" } else if bud { "Bud" } else { "Leaf" });
            let (lo, hi) = if vine { (5.0, 150.0) } else if bud { (2.0, 40.0) } else { (2.0, 80.0) };
            let mut size = sh.reach * 100.0;
            if zslider(ui, t, "Size", &mut size, lo, hi, |v| format!("{v:.0}%"), 150.0).on_hover_text("Size as a share of the stem it grows from").changed() { self.patch_shoot(|e| e.params.reach = size / 100.0); }
            if vine {
                let mut angle = sh.turn.to_degrees();
                if zslider(ui, t, "Angle", &mut angle, -120.0, 120.0, |v| format!("{v:.0}°"), 150.0).changed() { self.patch_shoot(|e| e.params.turn = angle.to_radians()); }
                let mut roll = sh.curl * 100.0;
                if zslider(ui, t, "Roll", &mut roll, 30.0, 100.0, |v| format!("{v:.0}%"), 150.0).on_hover_text("How far it rolls into its eye").changed() { self.patch_shoot(|e| e.params.curl = roll / 100.0); }
            } else {
                let lean = measured.map_or(0.0, |p| sh.side * p.frame);
                let mut angle = (sh.turn - lean).to_degrees();
                if zslider(ui, t, "Angle", &mut angle, -180.0, 180.0, |v| format!("{v:.0}°"), 150.0).changed() { self.patch_shoot(|e| e.params.turn = angle.to_radians() + lean); }
                if bud {
                } else if measured.is_some() {
                    let mut bend = sh.bend.unwrap_or(0.0);
                    if zslider(ui, t, "Bend", &mut bend, -1.5, 1.5, |v| format!("{v:.2}"), 150.0).changed() { self.patch_shoot(|e| e.params.bend = Some(bend)); }
                } else {
                    let mut curl = sh.curl;
                    if zslider(ui, t, "Curl", &mut curl, 0.2, 1.2, |v| format!("{v:.2}"), 150.0).changed() { self.patch_shoot(|e| e.params.curl = curl); }
                }
            }
            return;
        }
        caption(ui, t, &format!("Backbone {}", self.backbone + 1));
        let mut g = self.settings();
        let step = stepper(ui, t, "Variation", g.seed);
        if step != 0 { g.seed = (g.seed as i64 + step as i64).clamp(0, 99999) as u32; }
        if let Some(spacing) = g.vine {
            let mut s = spacing;
            if zslider(ui, t, "Spacing", &mut s, 20.0, 150.0, |v| format!("{v:.0} mm"), 160.0).on_hover_text("How far apart curls start along the backbone. Further apart gives fewer, larger curls.").changed() { g.vine = Some(s.round()); }
            let b = self.backbone;
            let edited = self.layout.shoots.iter().any(|e| e.backbone == b && e.params.preset.as_deref() == Some(VINE_CURL));
            if bevel_button(ui, t, "Regrow", false, Vec2::new(74.0, 30.0)).on_hover_text(if edited { "Grow the vine afresh, dropping the curls edited by hand (Undo brings them back)" } else { "Grow the vine afresh as the next variation" }).clicked() {
                if edited {
                    let mut next = self.layout.clone();
                    next.shoots.retain(|e| !(e.backbone == b && e.params.preset.as_deref() == Some(VINE_CURL)));
                    self.commit(next); self.selected = None;
                    return;
                }
                g.seed = (g.seed + 1) % 100000;
            }
        } else {
            let mut sec = g.secondary_scale.unwrap_or(1.0);
            if zslider(ui, t, "Shoot size", &mut sec, 0.5, 2.0, |v| format!("{v:.2}"), 160.0).changed() { g.secondary_scale = Some(sec); }
        }
        if g != self.settings() { self.commit_growth(g); }
    }

    /// Change the selected backbone's growth. A new seed or spacing for a
    /// scroll vine grows it afresh, as in the Backbone section.
    fn commit_growth(&mut self, g: GrowthSettings) {
        let before = self.settings(); let b = self.backbone;
        let reseeded = (before.vine.is_some() || g.vine.is_some()) && (g.seed != before.seed || g.vine != before.vine);
        let mut next = self.layout.clone();
        while next.growth.len() < next.curves.len() { let g0 = next.growth_for(next.growth.len()); next.growth.push(g0); }
        next.growth[b] = g;
        if reseeded { next.shoots.retain(|e| !(e.backbone == b && e.params.preset.as_deref() == Some(VINE_CURL))); }
        self.commit(next);
        if reseeded { self.selected = None; }
    }

    pub(crate) fn shelf_status(&mut self, ctx: &egui::Context) {
        let t = self.t();
        egui::TopBottomPanel::bottom("shelf-status").exact_height(28.0).frame(egui::Frame::none().fill(t.bg).inner_margin(egui::Margin::symmetric(14.0, 0.0))).show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if small_button(ui, t, "Fit").on_hover_text("Fit the page in the window").clicked() { self.fitted = false; }
                    ui.label(egui::RichText::new(format!("Zoom {:.0}%", self.zoom / 3.78 * 100.0)).size(12.5).color(t.dim));
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        let s = |x: String| egui::RichText::new(x).size(12.5).color(t.dim);
                        ui.label(s(format!("{} × {} mm", self.layout.width, self.layout.height)));
                        ui.add_space(12.0);
                        if let Some(c) = self.cursor_mm { ui.label(s(format!("x {:.1}  y {:.1} mm", c.x, c.y))); ui.add_space(12.0); }
                        ui.add(egui::Label::new(s(if self.message.is_empty() { self.grown.message.clone() } else { self.message.clone() })).truncate());
                    });
                });
            });
        });
    }

    /// The growth shelf: what the selected backbone grows, and quick ways
    /// into the leaf library and the constructions.
    pub(crate) fn shelf_tools(&mut self, ctx: &egui::Context) {
        let t = self.t();
        egui::SidePanel::left("shelf-tools").exact_width(84.0).resizable(false).frame(egui::Frame::none().fill(t.panel).inner_margin(egui::Margin::symmetric(8.0, 10.0)).stroke(Stroke::new(1.0, t.border))).show(ctx, |ui| {
            egui::ScrollArea::vertical().scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden).show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 6.0;
                shelf_caption(ui, t, "GROW");
                let g = self.settings();
                let current = if g.vine.is_some() { None } else { Some(g.family.unwrap_or(Family::Spiral)) };
                let kinds: [(Option<Family>, &str, fn(&egui::Painter, Rect, Color32)); 6] = [
                    (Some(Family::Spiral), "Spiral", icon_spiral), (Some(Family::Branching), "Branch", icon_branch), (Some(Family::Border), "Border", icon_border),
                    (Some(Family::Spray), "Spray", icon_spray), (Some(Family::Fan), "Fan", icon_fan), (None, "Vine", icon_vine)];
                for (kind, name, icon) in kinds {
                    let tip = match kind { Some(f) => family_label(f).to_string(), None => "Scroll vine: curls grow from seed points along the backbone, filling the carving surface".to_string() };
                    if tile(ui, t, name, icon, current == kind).on_hover_text(tip).clicked() && current != kind {
                        let mut g2 = g.clone();
                        match kind { Some(f) => { g2.vine = None; g2.family = Some(f); } None => g2.vine = Some(55.0) }
                        self.commit_growth(g2);
                    }
                }
                ui.add_space(2.0);
                let r = ui.available_rect_before_wrap();
                ui.painter().hline(r.left() + 6.0..=r.right() - 6.0, r.top(), Stroke::new(1.0, t.border));
                ui.add_space(8.0);
                shelf_caption(ui, t, "ADD");
                let design = self.tab != Tab::Carving;
                if tile(ui, t, "Leaves", icon_leaf_tile, design && self.palettes[LIBRARY]).on_hover_text("Leaf library: measured leaves and buds").clicked() { self.palettes[LIBRARY] = !(design && self.palettes[LIBRARY]); self.design_tray(); }
                if tile(ui, t, "Build", icon_build, design && self.palettes[CONSTRUCTION]).on_hover_text("Constructions: linked scrolls built on your page").clicked() { self.palettes[CONSTRUCTION] = !(design && self.palettes[CONSTRUCTION]); self.design_tray(); }
            });
        });
    }
    fn design_tray(&mut self) { if self.tab == Tab::Carving { self.tab = Tab::Properties; self.carving = false; self.stale = true; } }

    /// Folding palettes: the classic panel's sections, one per palette.
    pub(crate) fn shelf_tray(&mut self, ctx: &egui::Context) {
        let t = self.t();
        egui::SidePanel::right("shelf-tray").default_width(340.0).min_width(300.0).frame(egui::Frame::none().fill(t.bg).inner_margin(egui::Margin { left: 8.0, right: 6.0, top: 8.0, bottom: 8.0 })).show(ctx, |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                ui.set_width(ui.available_width() - 6.0);
                ui.spacing_mut().item_spacing.y = 6.0;
                if self.tab == Tab::Carving {
                    self.palette(ui, CARVING, "Carving guides", |s, ui| s.carving_tab(ui));
                    self.palette(ui, PAGE, "Page", |s, ui| s.page_section(ui));
                    self.palette(ui, CANVAS, "Canvas", |s, ui| s.theme_tab(ui));
                    return;
                }
                if let Some(sh) = self.selected_part().and_then(|p| p.shoot.clone()) {
                    let title = if sh.preset.as_deref() == Some(VINE_CURL) { "Selected curl" } else if sh.preset.as_deref().is_some_and(is_bud) { "Selected bud" } else { "Selected leaf" };
                    self.palette(ui, SELECTION, title, |s, ui| s.shoot_properties(ui));
                }
                if self.tool == Tool::Transform { self.palette(ui, TRANSFORM, "Transform", |s, ui| s.transform_section(ui)); }
                self.palette(ui, BACKBONE, "Backbone", |s, ui| s.backbone_section(ui));
                self.palette(ui, CONSTRUCTION, "Construction", |s, ui| s.construction(ui));
                self.palette(ui, LIBRARY, "Library", |s, ui| s.leaves(ui));
                self.palette(ui, LAYERS, "Layers", |s, ui| s.layers(ui));
                self.palette(ui, PAGE, "Page", |s, ui| s.page_section(ui));
                self.palette(ui, CANVAS, "Canvas", |s, ui| s.theme_tab(ui));
            });
        });
    }

    pub(crate) fn palette(&mut self, ui: &mut egui::Ui, i: usize, title: &str, body: impl FnOnce(&mut Self, &mut egui::Ui)) {
        let t = self.t();
        if palette_header(ui, t, title, self.palettes[i]).clicked() { self.palettes[i] = !self.palettes[i]; }
        if !self.palettes[i] { return; }
        egui::Frame::none().fill(t.panel).rounding(Rounding { nw: 0.0, ne: 0.0, sw: 6.0, se: 6.0 }).inner_margin(egui::Margin { left: 14.0, right: 12.0, top: 2.0, bottom: 12.0 }).show(ui, |ui| {
            ui.set_width(ui.available_width());
            // the palette's own title is not repeated as a section label inside it
            ui.data_mut(|d| d.insert_temp(egui::Id::new(PALETTE_TITLE), title.to_uppercase()));
            body(self, ui);
            ui.data_mut(|d| d.remove::<String>(egui::Id::new(PALETTE_TITLE)));
        });
    }
}

/// Key under which the open palette's title is kept while it draws, so
/// `section` can skip a label that only repeats it.
pub const PALETTE_TITLE: &str = "shelf-palette-title";

// ---- widgets ----------------------------------------------------------------

fn lighten(c: Color32, k: f32) -> Color32 {
    let f = |v: u8| (v as f32 + (255.0 - v as f32) * k).round() as u8;
    Color32::from_rgb(f(c.r()), f(c.g()), f(c.b()))
}

/// A raised ZBrush-style face: a lit top edge on a flat body.
fn bevel(painter: &egui::Painter, r: Rect, fill: Color32, round: f32) {
    painter.rect_filled(r, round, fill);
    painter.line_segment([Pos2::new(r.left() + round, r.top() + 0.5), Pos2::new(r.right() - round, r.top() + 0.5)], Stroke::new(1.0, lighten(fill, 0.12)));
}

pub(crate) fn bevel_button(ui: &mut egui::Ui, t: &Theme, label: &str, on: bool, size: Vec2) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(size, Sense::click());
    let fill = if on { t.accent } else if resp.hovered() { t.hover } else { t.surface };
    bevel(ui.painter(), r, fill, 5.0);
    ui.painter().text(r.center(), Align2::CENTER_CENTER, label, FontId::proportional(14.0), if on { t.on_accent } else { t.text });
    resp
}

pub(crate) fn small_button(ui: &mut egui::Ui, t: &Theme, label: &str) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(Vec2::new(44.0, 20.0), Sense::click());
    bevel(ui.painter(), r, if resp.hovered() { t.hover } else { t.surface }, 4.0);
    ui.painter().text(r.center(), Align2::CENTER_CENTER, label, FontId::proportional(12.5), t.text);
    resp
}

pub(crate) fn big_tab(ui: &mut egui::Ui, t: &Theme, label: &str, on: bool) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(Vec2::new(86.0, 40.0), Sense::click());
    let fill = if on { t.accent } else if resp.hovered() { t.hover } else { t.surface };
    bevel(ui.painter(), r, fill, 6.0);
    ui.painter().text(r.center(), Align2::CENTER_CENTER, label, semibold(15.0), if on { t.on_accent } else { t.text });
    resp
}

/// Fixed-width segmented choice; true when the value changed.
pub(crate) fn seg<T: PartialEq + Copy>(ui: &mut egui::Ui, t: &Theme, opts: &[(T, &str)], value: &mut T, w: f32, h: f32) -> bool {
    let (r, _) = ui.allocate_exact_size(Vec2::new(w * opts.len() as f32 + 4.0, h + 4.0), Sense::hover());
    ui.painter().rect_filled(r, 6.0, t.bg);
    let mut changed = false;
    for (i, (v, name)) in opts.iter().enumerate() {
        let cell = Rect::from_min_size(Pos2::new(r.left() + 2.0 + w * i as f32, r.top() + 2.0), Vec2::new(w, h));
        let resp = ui.interact(cell, ui.id().with(("seg", i, *name)), Sense::click());
        if resp.clicked() && *value != *v { *value = *v; changed = true; }
        let on = *value == *v;
        if on { bevel(ui.painter(), cell, t.accent, 4.0); } else if resp.hovered() { ui.painter().rect_filled(cell, 4.0, t.hover); }
        ui.painter().text(cell.center(), Align2::CENTER_CENTER, *name, FontId::proportional(13.0), if on { t.on_accent } else if resp.hovered() { t.text } else { t.dim });
    }
    changed
}

/// ZBrush's slider: label and value inside the bar, the filled part is the
/// value. Click or drag anywhere on it. The response reports `changed`.
pub(crate) fn zslider(ui: &mut egui::Ui, t: &Theme, label: &str, v: &mut f64, lo: f64, hi: f64, fmt: impl Fn(f64) -> String, w: f32) -> egui::Response {
    let w = if tight(ui) { w * 0.78 } else { w };
    let (r, mut resp) = ui.allocate_exact_size(Vec2::new(w, 28.0), Sense::click_and_drag());
    let before = *v;
    if let (true, Some(pos)) = (resp.dragged() || resp.clicked(), resp.interact_pointer_pos()) {
        *v = lo + ((pos.x - r.left()) / r.width()).clamp(0.0, 1.0) as f64 * (hi - lo);
    }
    if *v != before { resp.mark_changed(); }
    let k = ((*v - lo) / (hi - lo)).clamp(0.0, 1.0) as f32;
    let enabled = ui.is_enabled();
    let painter = ui.painter();
    painter.rect_filled(r, 4.0, t.bg);
    let fill = Rect::from_min_max(r.min, Pos2::new(r.left() + r.width() * k, r.bottom()));
    let a = if !enabled { 30 } else if resp.hovered() || resp.dragged() { 110 } else { 80 };
    painter.rect_filled(fill, 4.0, with_alpha(t.accent, a));
    if enabled { painter.line_segment([Pos2::new(fill.right(), r.top() + 3.0), Pos2::new(fill.right(), r.bottom() - 3.0)], Stroke::new(2.0, t.accent)); }
    painter.rect_stroke(r, 4.0, Stroke::new(1.0, t.border));
    let text = if enabled { t.text } else { t.dim };
    painter.text(Pos2::new(r.left() + 10.0, r.center().y), Align2::LEFT_CENTER, label, FontId::proportional(13.5), text);
    painter.text(Pos2::new(r.right() - 10.0, r.center().y), Align2::RIGHT_CENTER, fmt(*v), semibold(13.5), text);
    resp
}

/// "‹ Variation 12 ›": returns -1, 0 or +1.
pub(crate) fn stepper(ui: &mut egui::Ui, t: &Theme, label: &str, value: u32) -> i32 {
    let small = tight(ui);
    let (r, _) = ui.allocate_exact_size(Vec2::new(if small { 140.0 } else { 150.0 }, 28.0), Sense::hover());
    let painter = ui.painter().clone();
    painter.rect_filled(r, 4.0, t.bg);
    painter.rect_stroke(r, 4.0, Stroke::new(1.0, t.border));
    let mut step = 0;
    for (dir, cell) in [(-1, Rect::from_min_size(r.min, Vec2::new(26.0, r.height()))), (1, Rect::from_min_size(Pos2::new(r.right() - 26.0, r.top()), Vec2::new(26.0, r.height())))] {
        let resp = ui.interact(cell, ui.id().with(("step", label, dir)), Sense::click()).on_hover_text(if dir < 0 { "Previous variation" } else { "Next variation" });
        if resp.hovered() { painter.rect_filled(cell.shrink(2.0), 3.0, t.hover); }
        let c = cell.center(); let s = dir as f32;
        painter.add(Shape::line(vec![c + Vec2::new(-2.5 * s, -5.0), c + Vec2::new(2.5 * s, 0.0), c + Vec2::new(-2.5 * s, 5.0)], Stroke::new(1.6, t.text)));
        if resp.clicked() { step = dir; }
    }
    painter.text(r.center(), Align2::CENTER_CENTER, format!("{label} {value}"), FontId::proportional(if small { 12.5 } else { 13.5 }), t.text);
    step
}

pub(crate) fn divider(ui: &mut egui::Ui, t: &Theme) {
    let (r, _) = ui.allocate_exact_size(Vec2::new(1.0, 36.0), Sense::hover());
    ui.painter().vline(r.center().x, r.y_range(), Stroke::new(1.0, t.border));
}

/// A small upper-case label before the context controls.
pub(crate) fn caption(ui: &mut egui::Ui, t: &Theme, text: &str) {
    if tight(ui) { return; } // the first thing to go when the shelf is short of room
    ui.label(egui::RichText::new(text.to_uppercase()).font(semibold(11.0)).color(t.dim));
}
pub(crate) fn shelf_caption(ui: &mut egui::Ui, t: &Theme, text: &str) {
    ui.vertical_centered(|ui| ui.label(egui::RichText::new(text).font(semibold(10.5)).color(t.dim)));
}

/// A shelf tile: icon over its name; the selected one is outlined in the accent.
pub(crate) fn tile(ui: &mut egui::Ui, t: &Theme, name: &str, icon: fn(&egui::Painter, Rect, Color32), on: bool) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(Vec2::new(66.0, 60.0), Sense::click());
    let fill = if on { t.surface } else if resp.hovered() { t.hover } else { t.panel };
    bevel(ui.painter(), r, fill, 6.0);
    if on {
        ui.painter().rect_stroke(r, 6.0, Stroke::new(1.5, t.accent));
        ui.painter().rect_filled(Rect::from_min_size(Pos2::new(r.left(), r.top() + 12.0), Vec2::new(3.0, r.height() - 24.0)), 1.5, t.accent);
    }
    icon(ui.painter(), Rect::from_center_size(Pos2::new(r.center().x, r.top() + 23.0), Vec2::splat(24.0)), if on { t.accent } else { t.text });
    ui.painter().text(Pos2::new(r.center().x, r.bottom() - 12.0), Align2::CENTER_CENTER, name, FontId::proportional(12.5), if on { t.text } else { t.dim });
    if resp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
    resp
}

pub(crate) fn palette_header(ui: &mut egui::Ui, t: &Theme, name: &str, open: bool) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 36.0), Sense::click());
    let rounding = if open { Rounding { nw: 6.0, ne: 6.0, sw: 0.0, se: 0.0 } } else { Rounding::same(6.0) };
    let fill = if resp.hovered() { t.hover } else if open { t.surface } else { t.panel };
    let painter = ui.painter();
    painter.rect_filled(r, rounding, fill);
    painter.line_segment([Pos2::new(r.left() + 6.0, r.top() + 0.5), Pos2::new(r.right() - 6.0, r.top() + 0.5)], Stroke::new(1.0, lighten(fill, 0.1)));
    if open { painter.rect_filled(Rect::from_min_size(Pos2::new(r.left(), r.top() + 7.0), Vec2::new(3.0, r.height() - 14.0)), 1.5, t.accent); }
    painter.text(Pos2::new(r.left() + 16.0, r.center().y), Align2::LEFT_CENTER, name, semibold(14.5), if open { t.text } else { t.dim });
    let c = Pos2::new(r.right() - 18.0, r.center().y);
    let pts = if open { vec![c + Vec2::new(-5.0, 2.5), c + Vec2::new(0.0, -2.5), c + Vec2::new(5.0, 2.5)] } else { vec![c + Vec2::new(-5.0, -2.5), c + Vec2::new(0.0, 2.5), c + Vec2::new(5.0, -2.5)] };
    painter.add(Shape::line(pts, Stroke::new(1.6, t.dim)));
    if resp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
    resp
}

// ---- shelf icons, drawn in a unit box -----------------------------------------

fn bez(r: Rect, a: (f32, f32), b: (f32, f32), c: (f32, f32), d: (f32, f32)) -> Vec<Pos2> {
    (0..=24).map(|i| { let s = i as f32 / 24.0; let m = 1.0 - s;
        let x = m * m * m * a.0 + 3.0 * m * m * s * b.0 + 3.0 * m * s * s * c.0 + s * s * s * d.0;
        let y = m * m * m * a.1 + 3.0 * m * m * s * b.1 + 3.0 * m * s * s * c.1 + s * s * s * d.1; at_unit(r, x, y) }).collect()
}
fn coil(r: Rect, cx: f32, cy: f32, r0: f32, a0: f32, turn: f32, shrink: f32) -> Vec<Pos2> {
    (0..=40).map(|i| { let s = i as f32 / 40.0; let a = a0 + s * turn; let rr = r0 * (1.0 - s * shrink); at_unit(r, cx + a.cos() * rr, cy + a.sin() * rr) }).collect()
}
fn icon_spiral(p: &egui::Painter, r: Rect, c: Color32) {
    p.add(Shape::line(coil(r, 0.5, 0.5, 0.48, 0.0, 11.0, 0.88), Stroke::new(1.8, c)));
}
fn icon_branch(p: &egui::Painter, r: Rect, c: Color32) {
    let mut main = bez(r, (0.05, 0.95), (0.25, 0.4), (0.6, 0.15), (0.8, 0.3));
    main.extend(coil(r, 0.72, 0.42, 0.13, -0.6, 4.5, 0.6));
    p.add(Shape::line(main, Stroke::new(1.7, c)));
    let mut side = bez(r, (0.28, 0.6), (0.45, 0.7), (0.6, 0.85), (0.7, 0.78));
    side.extend(coil(r, 0.62, 0.74, 0.08, 0.5, -4.0, 0.5));
    p.add(Shape::line(side, Stroke::new(1.4, c)));
}
fn icon_border(p: &egui::Painter, r: Rect, c: Color32) {
    p.add(Shape::line(bez(r, (0.0, 0.5), (0.3, 0.1), (0.7, 0.9), (1.0, 0.5)), Stroke::new(1.7, c)));
    p.add(Shape::line(coil(r, 0.26, 0.62, 0.14, -2.2, 4.4, 0.6), Stroke::new(1.4, c)));
    p.add(Shape::line(coil(r, 0.74, 0.38, 0.14, 0.9, 4.4, 0.6), Stroke::new(1.4, c)));
}
fn icon_spray(p: &egui::Painter, r: Rect, c: Color32) {
    p.add(Shape::line(bez(r, (0.1, 0.95), (0.2, 0.4), (0.7, 0.6), (0.62, 0.08)), Stroke::new(1.6, c)));
    p.add(Shape::line(bez(r, (0.3, 0.95), (0.55, 0.75), (0.95, 0.5), (0.62, 0.08)), Stroke::new(1.6, c)));
}
fn icon_fan(p: &egui::Painter, r: Rect, c: Color32) {
    let o = at_unit(r, 0.15, 0.92);
    for k in 0..4 {
        let a = -1.45 + k as f32 * 0.42; let tip = o + Vec2::new(a.cos(), a.sin()) * r.width() * (0.85 - k as f32 * 0.08);
        let n = Vec2::new(-a.sin(), a.cos()) * 3.5; let mid = o + (tip - o) * 0.55;
        p.add(Shape::closed_line(vec![o, mid + n, tip, mid - n], Stroke::new(1.3, c)));
    }
}
fn icon_vine(p: &egui::Painter, r: Rect, c: Color32) {
    p.add(Shape::line(bez(r, (0.0, 0.7), (0.35, 0.95), (0.65, 0.45), (1.0, 0.6)), Stroke::new(1.6, c)));
    p.add(Shape::line(coil(r, 0.3, 0.42, 0.17, 1.4, 5.0, 0.7), Stroke::new(1.4, c)));
    p.add(Shape::line(coil(r, 0.74, 0.3, 0.15, 1.9, -5.0, 0.7), Stroke::new(1.4, c)));
}
fn icon_leaf_tile(p: &egui::Painter, r: Rect, c: Color32) {
    let mut o = bez(r, (0.1, 0.92), (0.0, 0.4), (0.5, 0.1), (0.92, 0.06));
    let mut b = bez(r, (0.92, 0.06), (0.8, 0.5), (0.5, 0.85), (0.1, 0.92)); b.remove(0); o.extend(b);
    p.add(Shape::closed_line(o, Stroke::new(1.6, c)));
    p.add(Shape::line(bez(r, (0.1, 0.92), (0.35, 0.65), (0.6, 0.35), (0.82, 0.16)), Stroke::new(1.1, c)));
}
fn icon_build(p: &egui::Painter, r: Rect, c: Color32) {
    for (x, y) in [(0.0, 0.0), (0.55, 0.0), (0.0, 0.55), (0.55, 0.55)] {
        p.rect_stroke(Rect::from_min_max(at_unit(r, x + 0.02, y + 0.02), at_unit(r, x + 0.43, y + 0.43)), 2.0, Stroke::new(1.4, c));
    }
    p.add(Shape::line(coil(r, 0.225, 0.225, 0.13, 0.0, 6.0, 0.7), Stroke::new(1.1, c)));
    p.add(Shape::line(bez(r, (0.6, 0.95), (0.7, 0.6), (0.85, 0.8), (0.95, 0.6)), Stroke::new(1.1, c)));
}

/// Whether the shelf's context controls are drawn compact (see `tight_controls`).
fn tight(ui: &egui::Ui) -> bool { ui.data(|d| d.get_temp::<bool>(egui::Id::new(TIGHT))).unwrap_or(false) }
const TIGHT: &str = "shelf-tight";

/// Draw the context controls in the room left between the shelf's two
/// groups: compact (no caption, narrower sliders) when that room is short,
/// and clipped to it, so they never run under the right-hand group.
pub(crate) fn tight_controls(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui)) {
    let room = ui.available_rect_before_wrap();
    ui.set_clip_rect(room.intersect(ui.clip_rect()));
    ui.data_mut(|d| d.insert_temp(egui::Id::new(TIGHT), room.width() < 560.0));
    add(ui);
    ui.data_mut(|d| d.remove::<bool>(egui::Id::new(TIGHT)));
}
