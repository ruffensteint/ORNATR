//! ORNATR — native desktop app. Procedural acanthus scroll patterns for
//! carving, drawn with egui (no web engine). Geometry lives in `scroll_core`.
//! The same program also builds as a web edition (WebAssembly) for the author's website;
//! everything that differs between the two is in `platform`.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod cartouche_ui;
mod chip_ui;
mod io;
mod page_sizes;
mod palmette_ui;
mod platform;
mod presets;
mod rococo_ui;
mod shelf;
mod theme;

use eframe::egui::{self, Color32, Pos2, Rect, Sense, Shape, Stroke, Vec2};
use scroll_core::geometry::{distance, fit_curve, pt, Bounds, Curve, Point};
use scroll_core::growth::{Family, GrowthPart, GrowthResult, GrowthSettings, Side, VoluteHandle, VOLUTE_SIZE, VOLUTE_TURNS};
use scroll_core::layers::{carving_guides, layered_drawing, Drawing, Guides};
use scroll_core::bud::{is_bud, BUD_PRESETS};
use scroll_core::collar::{is_collar, CollarStyle};
use scroll_core::model::{convert_legacy, preset_params, Layout, LEAF_PRESETS};
use scroll_core::skeleton::{skeleton_layout, Skeleton};
use scroll_core::outline::inside;
use scroll_core::profiles::profile;
use scroll_core::shoots::{drag_tip, nearest_progress, tip_handle, ShootEdit, ShootParams, VINE_CURL};
use scroll_core::transform::{flip_curve, mirror_shoot, transform_curve, Axis, TransformKind};
use std::path::PathBuf;
use platform::OpenFor;
use theme::{Canvas, Joins, Prefs, Theme, ThemeId};

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1440.0, 900.0]).with_min_inner_size([900.0, 600.0]).with_title("ORNATR")
            // the title bar and taskbar icon (the exe's own icon is embedded by build.rs)
            .with_icon(std::sync::Arc::new(egui::IconData { rgba: include_bytes!("../assets/ornatr-128.rgba").to_vec(), width: 128, height: 128 })),
        ..Default::default()
    };
    eframe::run_native("ORNATR", options, Box::new(|cc| Ok(Box::new(App::new(cc)))))
}

/// The web edition (see platform).
#[cfg(target_arch = "wasm32")]
fn main() { platform::start(Box::new(|cc| Ok(Box::new(App::new(cc))))); }

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tool { Select, Pen, Transform }
#[derive(Clone, Copy, PartialEq, Eq)]
enum Workspace { Scroll, Chip, Rococo, Cartouche, Palmette }
#[derive(Clone, Copy, PartialEq)]
enum Pending { New, Open, Close }
#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab { Properties, Leaves, Layers, Carving, Theme }
/// What a click on the canvas takes while a pick key is held (B, L or C).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Pick { Backbone, Leaf, Collar }

enum Drag {
    Pan,
    Handle { index: usize },
    /// The selected backbone's volute tip, from the volute as grown when the drag began.
    Volute { vh: VoluteHandle },
    ShootRoot { edit: String, backbone: usize },
    ShootTip { edit: String, start: ShootParams, root: Point, handle: Point },
    /// A backbone and everything that grows from it move together.
    Transform { kind: TransformKind, center: Point, from: Point, curves: Vec<(usize, Curve)> },
    Draw { points: Vec<Point> },
}

struct App {
    layout: Layout,
    path: Option<PathBuf>,
    dirty: bool,
    past: Vec<Layout>,
    future: Vec<Layout>,
    tool: Tool,
    tab: Tab,
    backbone: usize,
    selected: Option<String>,
    carving: bool,
    show_guides: bool,
    grid: bool,
    // cached growth and drawings
    grown: GrowthResult,
    drawing: Drawing,
    /// The shown drawing is the quick classic one; redraw smooth when idle.
    smooth_due: bool,
    guides: Option<Guides>,
    stale: bool,
    // view: screen = origin + mm * zoom
    zoom: f32,
    origin: Pos2,
    fitted: bool,
    drag: Option<Drag>,
    drag_before: Option<Layout>,
    message: String,
    cursor_mm: Option<Point>,
    shown_title: String,
    /// An action that would discard unsaved work, and whether it is cleared to run.
    pending: Option<(Pending, bool)>,
    allow_close: bool,
    prefs: Prefs,
    /// Preferences last installed into egui.
    applied: Option<Prefs>,
    /// Interface size while its slider is being dragged.
    scale_draft: f32,
    /// Fillet slider value while it is being dragged (applied on release).
    fillet_draft: f32,
    workspace: Workspace,
    chip: chip_ui::ChipState,
    rococo: rococo_ui::RococoState,
    cartouche: cartouche_ui::CartoucheState,
    palmette: palmette_ui::PalmetteState,
    scroll_presets: presets::Library,
    scroll_thumbs: Vec<Option<Vec<Vec<Point>>>>,
    /// Last construction built, its variation, and cached card thumbnails.
    skeleton: Option<Skeleton>,
    skel_seed: u32,
    skel_thumbs: Vec<Option<Vec<Vec<Point>>>>,
    /// Constructions are built clad in the vine acanthus leaf.
    skel_vine_leaf: bool,
    /// Preset and saved page sizes; the New dialog while it is open; the size the next new scroll pattern gets.
    page_sizes: page_sizes::PageSizes,
    new_dialog: Option<page_sizes::NewDialog>,
    new_size: Option<(f64, f64)>,
    /// Shelf layout: which palettes in the right tray are open.
    palettes: [bool; shelf::PALETTES],
}

/// The scroll workspace opens on the ORNATR mark grown in the app (the logo
/// study's pick, 2026-10-09); File → New still starts from the plain starter.
fn startup_layout() -> Layout { io::parse(io::STARTUP).unwrap_or_else(|_| Layout::starter()) }

impl App {
    fn new(cc: &eframe::CreationContext) -> Self {
        theme::install_fonts(&cc.egui_ctx);
        let prefs = Prefs::load();
        let layout = startup_layout();
        let mut app = App { layout, path: None, dirty: false, past: vec![], future: vec![], tool: Tool::Select, tab: Tab::Properties, backbone: 0, selected: None, carving: false, show_guides: true, grid: false,
            grown: GrowthResult::default(), drawing: Drawing { outline: vec![], folds: vec![] }, smooth_due: false, guides: None, stale: true, zoom: 3.0, origin: Pos2::ZERO, fitted: false, drag: None, drag_before: None, message: String::new(), cursor_mm: None, shown_title: String::new(), pending: None, allow_close: false, prefs, applied: None, scale_draft: prefs.ui_scale, fillet_draft: prefs.fillet,
            workspace: if prefs.chip { Workspace::Chip } else if prefs.rococo { Workspace::Rococo } else if prefs.cartouche { Workspace::Cartouche } else if prefs.palmette { Workspace::Palmette } else { Workspace::Scroll }, chip: chip_ui::ChipState::new(), rococo: rococo_ui::RococoState::new(), cartouche: cartouche_ui::CartoucheState::new(), palmette: palmette_ui::PalmetteState::new(), scroll_presets: presets::Library::load("scroll-presets.json"), scroll_thumbs: vec![], skeleton: None, skel_seed: 0, skel_thumbs: vec![None; Skeleton::ALL.len()], skel_vine_leaf: false, page_sizes: page_sizes::PageSizes::load(), new_dialog: None, new_size: None, palettes: shelf::DEFAULT_OPEN };
        app.regrow();
        app
    }

    // ---------- model plumbing ----------
    fn regrow(&mut self) {
        let mut rounds = 0; while rounds < 6 && self.layout.settle() { rounds += 1; }
        // A scroll vine takes up to a second to grow, so while something is
        // dragged it shows its stem only, and grows on release.
        let vines = self.layout.growth.iter().any(|g| g.vine.is_some());
        self.grown = if vines && self.drag.is_some() { self.layout.grow_draft() } else { self.layout.grow() };
        // Smooth and exact joins take a little longer, so while something is
        // being dragged the classic drawing is shown and theirs follows on release.
        let slow = self.prefs.joins != Joins::Classic;
        self.drawing = if slow && self.drag.is_none() { self.prefs.join_style().draw(&self.grown) } else { layered_drawing(&self.grown) };
        self.smooth_due = (slow || vines) && self.drag.is_some();
        self.guides = if self.carving { Some(carving_guides(&self.grown)) } else { None };
        self.stale = false;
    }
    /// Record an undo step, then change the layout.
    fn commit(&mut self, next: Layout) {
        self.past.push(self.layout.clone());
        if self.past.len() > 100 { self.past.remove(0); }
        self.future.clear();
        self.layout = next;
        self.dirty = true;
        self.stale = true;
    }
    fn undo(&mut self) { if let Some(p) = self.past.pop() { self.future.push(std::mem::replace(&mut self.layout, p)); self.stale = true; self.dirty = true; self.clamp_backbone(); } }
    fn redo(&mut self) { if let Some(n) = self.future.pop() { self.past.push(std::mem::replace(&mut self.layout, n)); self.stale = true; self.dirty = true; self.clamp_backbone(); } }
    fn clamp_backbone(&mut self) { self.backbone = self.backbone.min(self.layout.curves.len() - 1); }
    fn settings(&self) -> GrowthSettings { self.layout.growth_for(self.backbone) }
    fn set_settings(&mut self, g: GrowthSettings) {
        let mut next = self.layout.clone();
        while next.growth.len() < next.curves.len() { let g0 = next.growth_for(next.growth.len()); next.growth.push(g0); }
        next.growth[self.backbone] = g;
        self.commit(next);
    }
    /// A hand-placed backbone keeps its proportions instead of refitting to
    /// the page. Applied in place: the drag's undo state was already taken.
    fn set_free(&mut self) {
        while self.layout.growth.len() < self.layout.curves.len() { let g = self.layout.growth_for(self.layout.growth.len()); self.layout.growth.push(g); }
        for b in self.group() { if self.layout.growth[b].free != Some(true) { self.layout.growth[b].free = Some(true); self.stale = true; } }
    }
    /// The selected backbone plus every backbone that grows from it.
    fn group(&self) -> Vec<usize> { let mut g = vec![self.backbone]; g.extend(self.layout.descendants(self.backbone)); g }
    fn t(&self) -> &'static Theme { self.prefs.theme.theme() }
    fn canvas_colors(&self) -> Canvas { self.t().canvas(self.prefs.white_page) }
    fn set_prefs(&mut self, p: Prefs) { if p != self.prefs { self.prefs = p; p.save(); } }
    fn multi(&self) -> bool { self.layout.curves.len() > 1 }
    /// (backbone, id without prefix) of a grown part id.
    fn split_id(&self, id: &str) -> (usize, String) {
        if let Some(rest) = id.strip_prefix("backbone-") { if let Some((n, local)) = rest.split_once('/') { return (n.parse().unwrap_or(0), local.to_string()); } }
        (0, id.to_string())
    }
    fn display_id(&self, e: &ShootEdit) -> String { if self.multi() { format!("backbone-{}/{}", e.backbone, e.id) } else { e.id.clone() } }
    fn selected_part(&self) -> Option<&GrowthPart> { let id = self.selected.as_ref()?; self.grown.parts.iter().find(|p| &p.id == id && p.parent.is_some() && p.shoot.is_some()) }
    fn selected_edit(&self) -> Option<&ShootEdit> { let part = self.selected_part()?; let (b, local) = self.split_id(&part.id); self.layout.shoots.iter().find(|e| e.backbone == b && e.id == local) }
    /// Generated shoots become edits the first time they are touched.
    fn take_over(&self, layout: &mut Layout, part_id: &str) -> Option<String> {
        let part = self.grown.parts.iter().find(|p| p.id == part_id)?;
        let params = part.shoot.clone()?;
        let (backbone, local) = self.split_id(part_id);
        if layout.shoots.iter().any(|e| e.backbone == backbone && e.id == local) { return Some(local); }
        if params.preset.as_deref() == Some(VINE_CURL) {
            // the first edit to a scroll vine keeps all its curls as edits, so
            // the vine is built from them from now on and nothing shuffles
            for p in &self.grown.parts {
                let (b, l) = self.split_id(&p.id);
                let Some(sh) = p.shoot.as_ref().filter(|s| s.preset.as_deref() == Some(VINE_CURL)) else { continue };
                if b == backbone && !layout.shoots.iter().any(|e| e.backbone == b && e.id == l) { layout.shoots.push(ShootEdit { params: sh.clone(), id: l, backbone: b, replaces: None, hidden: false, under: false }); }
            }
            return Some(local);
        }
        let id = new_id();
        layout.shoots.push(ShootEdit { params, id: id.clone(), backbone, replaces: Some(local), hidden: false, under: false });
        Some(id)
    }
    /// For a scroll-vine curl: where along the stem or curl it grows from the
    /// point `mm` lies (0 to 1). None for other leaves.
    fn vine_progress(&self, backbone: usize, id: &str, mm: Point) -> Option<f64> {
        let e = self.layout.shoots.iter().find(|e| e.id == id && e.backbone == backbone && e.params.preset.as_deref() == Some(VINE_CURL))?;
        let local = e.params.on.clone().unwrap_or_else(|| "curve".into());
        let full = if self.multi() { format!("backbone-{backbone}/{local}") } else { local };
        let pts = &self.grown.parts.iter().find(|p| p.id == full)?.points;
        if pts.len() < 2 { return None; }
        let i = (0..pts.len()).min_by(|&a, &b| distance(pts[a], mm).partial_cmp(&distance(pts[b], mm)).unwrap())?;
        // keep clear of the very ends, where a curl could not leave cleanly
        Some((i as f64 / (pts.len() - 1) as f64).clamp(0.02, 0.9))
    }
    fn patch_shoot(&mut self, f: impl FnOnce(&mut ShootEdit)) {
        let Some(sel) = self.selected.clone() else { return };
        let mut next = self.layout.clone();
        let Some(id) = self.take_over(&mut next, &sel) else { return };
        let (b, _) = self.split_id(&sel);
        if let Some(e) = next.shoots.iter_mut().find(|e| e.id == id && e.backbone == b) { f(e); let d = self.display_id(e); self.commit(next); self.selected = Some(d); }
    }
    fn add_leaf(&mut self, id: &str) {
        let at = self.selected_part().and_then(|p| p.shoot.as_ref()).map(|s| (s.progress + 0.12).min(0.95)).unwrap_or(0.5);
        let side = if self.layout.shoots.len() % 2 == 1 { -1.0 } else { 1.0 };
        let Some(params) = preset_params(id, at, side) else { return };
        let mut next = self.layout.clone();
        let e = ShootEdit { params, id: new_id(), backbone: self.backbone, replaces: None, hidden: false, under: false };
        let d = self.display_id(&e);
        next.shoots.push(e);
        self.commit(next);
        self.selected = Some(d);
        self.tool = Tool::Select;
        self.tab = Tab::Properties;
    }
    fn part_bounds(&self) -> Bounds {
        let group = self.group();
        let prefixes: Vec<String> = group.iter().map(|b| format!("backbone-{b}/")).collect();
        let mut pts: Vec<Point> = self.grown.parts.iter().filter(|p| !self.multi() || prefixes.iter().any(|x| p.id.starts_with(x))).flat_map(|p| p.polygon.iter().copied()).collect();
        for b in &group { pts.extend(self.layout.curves[*b]); }
        Bounds::of(&pts)
    }

    // ---------- files ----------
    fn title(&self) -> String {
        let name = self.path.as_ref().and_then(|p| p.file_stem()).map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "Untitled".into());
        format!("{}{} — ORNATR", name, if self.dirty { " •" } else { "" })
    }
    fn new_file(&mut self) { let l = match self.new_size.take() { Some((w, h)) => resized(&Layout::starter(), w, h), None => Layout::starter() }; self.commit(l); self.path = None; self.dirty = false; self.backbone = 0; self.selected = None; self.fitted = false; }
    fn open(&mut self, ctx: &egui::Context) {
        if let Some(r) = platform::pick_text(ctx, OpenFor::Scroll, "ORNATR layout", &["ornatr", "scrollworks", "json"]) { self.opened(OpenFor::Scroll, r); }
    }
    /// A picked file, once read. Every workspace saves .ornatr files, so the file goes
    /// to the workspace its contents belong to (`io::file_kind`), whichever Open picked
    /// it, and that workspace comes to the front. A scroll layout picked from another
    /// workspace waits while the scroll pattern has unsaved changes.
    fn opened(&mut self, who: OpenFor, picked: platform::Opened) {
        let to = match &picked { Ok((_, text)) => io::file_kind(text).unwrap_or(who), Err(_) => who };
        if to == OpenFor::Scroll && who != OpenFor::Scroll && self.dirty {
            let m = "That is a scroll layout, and the scroll pattern has unsaved changes. Save it in the Scroll workspace, then open this file there.".to_string();
            match who { OpenFor::Chip => self.chip.message = m, OpenFor::Cartouche => self.cartouche.message = m, OpenFor::Palmette => self.palmette.message = m, _ => self.rococo.message = m }
            return;
        }
        match to {
            OpenFor::Scroll => self.open_text(picked),
            OpenFor::Chip => self.chip.open_text(picked),
            OpenFor::Rococo => self.rococo.open_text(picked),
            OpenFor::Cartouche => self.cartouche.open_text(picked),
            OpenFor::Palmette => self.palmette.open_text(picked),
        }
        self.set_workspace(match to { OpenFor::Scroll => Workspace::Scroll, OpenFor::Chip => Workspace::Chip, OpenFor::Rococo => Workspace::Rococo, OpenFor::Cartouche => Workspace::Cartouche, OpenFor::Palmette => Workspace::Palmette });
    }
    fn open_chip(&mut self, ctx: &egui::Context) {
        if let Some(r) = platform::pick_text(ctx, OpenFor::Chip, "ORNATR chip layout or box", &["ornatr", "json"]) { self.opened(OpenFor::Chip, r); }
    }
    fn open_rococo(&mut self, ctx: &egui::Context) {
        if let Some(r) = platform::pick_text(ctx, OpenFor::Rococo, "ORNATR rococo design", &["ornatr", "json"]) { self.opened(OpenFor::Rococo, r); }
    }
    fn open_cartouche(&mut self, ctx: &egui::Context) {
        if let Some(r) = platform::pick_text(ctx, OpenFor::Cartouche, "ORNATR cartouche", &["ornatr", "json"]) { self.opened(OpenFor::Cartouche, r); }
    }
    /// A picked file, once read (at once on the desktop, a moment later on the web).
    fn open_text(&mut self, picked: platform::Opened) {
        let (path, text) = match picked { Ok(p) => p, Err(e) => { self.message = e; return } };
        match io::parse(&text) {
            Ok(mut l) => {
                let legacy = l.items.len();
                if legacy > 0 { convert_legacy(&mut l, &mut new_id); }
                self.commit(l); self.path = Some(path); self.dirty = false; self.backbone = 0; self.selected = None; self.fitted = false;
                self.message = if legacy > 0 { format!("Opened. {legacy} stamped motif(s) from the old manual mode were converted to grown leaves.") } else { "Opened.".into() };
            }
            Err(e) => self.message = e,
        }
    }
    /// File → New: choose the page size first, starting from the current one.
    fn open_new_dialog(&mut self) {
        let (chip, rococo, cartouche, palmette) = (self.workspace == Workspace::Chip, self.workspace == Workspace::Rococo, self.workspace == Workspace::Cartouche, self.workspace == Workspace::Palmette);
        let (width, height) = if chip { (self.chip.settings.width(), self.chip.settings.page_height()) } else if rococo { (self.rococo.design.width, self.rococo.design.height) } else if cartouche { (self.cartouche.design.width, self.cartouche.design.height) } else if palmette { (self.palmette.design.width, self.palmette.design.height) } else { (self.layout.width, self.layout.height) };
        self.new_dialog = Some(page_sizes::NewDialog { chip, rococo, cartouche, palmette, width, height });
    }
    fn show_new_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut d) = self.new_dialog.take() else { return };
        let t = self.t(); let mut inches = self.prefs.inches;
        let choice = page_sizes::new_dialog(ctx, t, &mut d, &mut self.page_sizes, &mut inches);
        if inches != self.prefs.inches { self.set_prefs(Prefs { inches, ..self.prefs }); }
        match choice {
            None => self.new_dialog = Some(d),
            Some(page_sizes::NewChoice::Cancel) => {}
            Some(page_sizes::NewChoice::Create(w, h)) => {
                if d.chip { self.chip.new_pattern_sized(w, h); } else if d.rococo { self.rococo.new_design(w, h); } else if d.cartouche { self.cartouche.new_design(w, h); } else if d.palmette { self.palmette.new_design(w, h); } else { self.new_size = Some((w, h)); self.ask(Pending::New); }
            }
        }
    }
    /// Run an action that replaces the document, asking first if it has unsaved changes.
    fn ask(&mut self, p: Pending) { self.pending = Some((p, !self.dirty)); }
    fn run_pending(&mut self, ctx: &egui::Context) {
        if ctx.input(|i| i.viewport().close_requested()) && !self.allow_close {
            if self.dirty { ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose); self.pending = Some((Pending::Close, false)); }
        }
        let Some((p, cleared)) = self.pending else { return };
        if cleared {
            self.pending = None;
            match p { Pending::New => self.new_file(), Pending::Open => self.open(ctx), Pending::Close => { self.allow_close = true; ctx.send_viewport_cmd(egui::ViewportCommand::Close); } }
            return;
        }
        let mut choice = None;
        egui::Window::new("Unsaved changes").collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO).show(ctx, |ui| {
            ui.label("This pattern has changes that are not saved.");
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("Save").clicked() { choice = Some(0); }
                if ui.button("Don't save").clicked() { choice = Some(1); }
                if ui.button("Cancel").clicked() { choice = Some(2); }
            });
        });
        match choice {
            Some(0) => { self.save(false); if !self.dirty { self.pending = Some((p, true)); } }
            Some(1) => self.pending = Some((p, true)),
            Some(2) => self.pending = None,
            _ => {}
        }
    }
    fn save(&mut self, choose: bool) {
        let path = if choose || self.path.is_none() {
            match platform::choose_save("ORNATR layout", &["ornatr"], "pattern.ornatr") { Some(p) => p, None => return }
        } else { self.path.clone().unwrap() };
        match platform::write_file(&path, &io::save(&self.layout)) { Ok(()) => { self.path = Some(path); self.dirty = false; self.message = "Saved.".into(); } Err(e) => self.message = format!("Could not save: {e}") }
    }
    fn export(&mut self, carving: bool) {
        let (name, svg) = if carving { ("carving-guides.svg", self.layout.carving_svg()) } else { ("pattern.svg", self.layout.svg_joins(self.prefs.join_style())) };
        if let Some(p) = platform::choose_save("SVG", &["svg"], name) {
            match platform::write_file(&p, &svg) { Ok(()) => self.message = format!("Exported {}.", platform::shown(&p)), Err(e) => self.message = format!("Could not export: {e}") }
        }
    }

    // ---------- view ----------
    fn to_screen(&self, p: Point) -> Pos2 { Pos2::new(self.origin.x + p.x as f32 * self.zoom, self.origin.y + p.y as f32 * self.zoom) }
    fn to_mm(&self, s: Pos2) -> Point { pt(((s.x - self.origin.x) / self.zoom) as f64, ((s.y - self.origin.y) / self.zoom) as f64) }
    fn fit(&mut self, rect: Rect) {
        let z = ((rect.width() - 60.0) / self.layout.width as f32).min((rect.height() - 60.0) / self.layout.height as f32).max(0.2);
        self.zoom = z;
        self.origin = Pos2::new(rect.center().x - self.layout.width as f32 * z / 2.0, rect.center().y - self.layout.height as f32 * z / 2.0);
        self.fitted = true;
    }
}

/// The layout on a page of a new size: backbones scale with the page.
fn resized(layout: &Layout, w: f64, h: f64) -> Layout {
    let mut next = layout.clone(); let (sx, sy) = (w / next.width, h / next.height);
    for c in next.curves.iter_mut() { *c = c.map(|p| pt(p.x * sx, p.y * sy)); }
    next.width = w; next.height = h; next.locked_parts.clear();
    next
}

fn new_id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    use web_time::{SystemTime, UNIX_EPOCH};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed) + 1;
    let t = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(0);
    format!("edit-{:x}{:x}", t & 0xffffff, n)
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Install the theme when it changes, and restore it if eframe resets
        // the style (it follows the Windows light/dark setting).
        if self.applied != Some(self.prefs) || ctx.style().visuals.panel_fill != self.t().bg {
            self.t().apply(ctx);
            ctx.set_zoom_factor(self.prefs.ui_scale);
            self.applied = Some(self.prefs);
        }
        self.shortcuts(ctx);
        self.run_pending(ctx);
        self.show_new_dialog(ctx);
        if self.stale { self.regrow(); }
        let title = match self.workspace { Workspace::Chip => self.chip.title(), Workspace::Rococo => self.rococo.title(), Workspace::Cartouche => self.cartouche.title(), Workspace::Palmette => self.palmette.title(), Workspace::Scroll => self.title() };
        if title != self.shown_title { platform::set_title(ctx, &title); self.shown_title = title; }
        platform::set_unsaved(self.dirty);
        while let Some((who, picked)) = platform::take_opened() {
            self.opened(who, picked)
        }
        let desk = egui::Frame::none().fill(self.canvas_colors().desk);
        if self.prefs.shelf {
            self.shelf_top_bar(ctx);
            if self.workspace == Workspace::Rococo {
                self.rococo_shelf_context(ctx);
                self.rococo_shelf_status(ctx);
                self.rococo_shelf_tools(ctx);
                self.rococo_shelf_tray(ctx);
                egui::CentralPanel::default().frame(desk).show(ctx, |ui| self.rococo_canvas(ui));
                return;
            }
            if self.workspace == Workspace::Palmette {
                self.palmette_shelf_context(ctx);
                self.palmette_shelf_status(ctx);
                self.palmette_shelf_tools(ctx);
                self.palmette_shelf_tray(ctx);
                egui::CentralPanel::default().frame(desk).show(ctx, |ui| self.palmette_canvas(ui));
                return;
            }
            if self.workspace == Workspace::Cartouche {
                self.cartouche_shelf_context(ctx);
                self.cartouche_shelf_status(ctx);
                self.cartouche_shelf_tools(ctx);
                self.cartouche_shelf_tray(ctx);
                egui::CentralPanel::default().frame(desk).show(ctx, |ui| self.cartouche_canvas(ui));
                return;
            }
            if self.workspace == Workspace::Chip {
                self.chip_shelf_context(ctx);
                self.chip_shelf_status(ctx);
                self.chip_shelf_tools(ctx);
                self.chip_shelf_tray(ctx);
                egui::CentralPanel::default().frame(desk).show(ctx, |ui| self.chip_canvas(ui));
            } else {
                self.shelf_context(ctx);
                self.shelf_status(ctx);
                self.shelf_tools(ctx);
                self.shelf_tray(ctx);
                egui::CentralPanel::default().frame(desk).show(ctx, |ui| self.canvas(ui));
            }
            return;
        }
        self.menu_bar(ctx);
        if self.workspace == Workspace::Rococo {
            self.rococo_status(ctx);
            self.rococo_side_panel(ctx);
            egui::CentralPanel::default().frame(desk).show(ctx, |ui| self.rococo_canvas(ui));
            return;
        }
        if self.workspace == Workspace::Palmette {
            self.palmette_status(ctx);
            self.palmette_side_panel(ctx);
            egui::CentralPanel::default().frame(desk).show(ctx, |ui| self.palmette_canvas(ui));
            return;
        }
        if self.workspace == Workspace::Cartouche {
            self.cartouche_status(ctx);
            self.cartouche_side_panel(ctx);
            egui::CentralPanel::default().frame(desk).show(ctx, |ui| self.cartouche_canvas(ui));
            return;
        }
        if self.workspace == Workspace::Chip {
            self.chip_status(ctx);
            self.chip_side_panel(ctx);
            egui::CentralPanel::default().frame(desk).show(ctx, |ui| self.chip_canvas(ui));
        } else {
            self.status_bar(ctx);
            self.tool_strip(ctx);
            self.side_panel(ctx);
            egui::CentralPanel::default().frame(desk).show(ctx, |ui| self.canvas(ui));
        }
    }
}

impl App {
    fn shortcuts(&mut self, ctx: &egui::Context) {
        use egui::{Key, Modifiers};
        let typing = ctx.wants_keyboard_input();
        let (undo, redo, redo2, save, save_as, open, new) = ctx.input_mut(|i| (
            i.consume_key(Modifiers::COMMAND, Key::Z), i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z), i.consume_key(Modifiers::COMMAND, Key::Y),
            i.consume_key(Modifiers::COMMAND, Key::S), i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::S), i.consume_key(Modifiers::COMMAND, Key::O), i.consume_key(Modifiers::COMMAND, Key::N)));
        if self.workspace == Workspace::Rococo { self.rococo_shortcuts(ctx, undo, redo || redo2, save, save_as, open, new, typing); return; }
        if self.workspace == Workspace::Palmette { self.palmette_shortcuts(ctx, undo, redo || redo2, save, save_as, open, new, typing); return; }
        if self.workspace == Workspace::Cartouche { self.cartouche_shortcuts(ctx, undo, redo || redo2, save, save_as, open, new, typing); return; }
        if self.workspace == Workspace::Chip {
            if undo { self.chip.undo(); }
            if redo || redo2 { self.chip.redo(); }
            if save_as { self.chip.save(true); } else if save { self.chip.save(false); }
            if open { self.open_chip(ctx); }
            if new { self.open_new_dialog(); }
            if typing { return; }
            let (del, esc) = ctx.input(|i| (i.key_pressed(Key::Delete) || i.key_pressed(Key::Backspace), i.key_pressed(Key::Escape)));
            if del { self.chip.remove_selected(); }
            if esc { self.chip.selected = None; }
            return;
        }
        if undo { self.undo(); }
        if redo || redo2 { self.redo(); }
        if save_as { self.save(true); } else if save { self.save(false); }
        if open { self.ask(Pending::Open); }
        if new { self.open_new_dialog(); }
        if typing { return; }
        let (v, p, t, del, esc) = ctx.input(|i| (i.key_pressed(Key::V), i.key_pressed(Key::P), i.key_pressed(Key::T), i.key_pressed(Key::Delete) || i.key_pressed(Key::Backspace), i.key_pressed(Key::Escape)));
        if v { self.tool = Tool::Select; }
        if p { self.tool = Tool::Pen; }
        if t { self.tool = Tool::Transform; self.selected = None; }
        if esc { self.selected = None; self.tool = Tool::Select; }
        if del && self.selected_part().is_some() { self.remove_selected(); }
    }
    fn remove_selected(&mut self) {
        let Some(sel) = self.selected.clone() else { return };
        let mut next = self.layout.clone();
        let Some(id) = self.take_over(&mut next, &sel) else { return };
        let (b, _) = self.split_id(&sel);
        let vine = next.shoots.iter().any(|e| e.id == id && e.backbone == b && e.params.preset.as_deref() == Some(VINE_CURL));
        if vine {
            // a vine curl goes with the curls growing from it
            let mut gone = vec![id.clone()];
            loop { let more: Vec<String> = next.shoots.iter().filter(|e| e.backbone == b && e.params.on.as_ref().is_some_and(|o| gone.contains(o)) && !gone.contains(&e.id)).map(|e| e.id.clone()).collect(); if more.is_empty() { break; } gone.extend(more); }
            next.shoots.retain(|e| !(e.backbone == b && gone.contains(&e.id)));
        } else if let Some(i) = next.shoots.iter().position(|e| e.id == id) { if next.shoots[i].replaces.is_some() { next.shoots[i].hidden = true; } else { next.shoots.remove(i); } }
        self.commit(next); self.selected = None;
    }

    fn menu_bar(&mut self, ctx: &egui::Context) {
        let t = self.t();
        egui::TopBottomPanel::top("menu").frame(egui::Frame::none().fill(t.bg).inner_margin(egui::Margin::symmetric(12.0, 6.0))).show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.label(egui::RichText::new("ORNATR").family(egui::FontFamily::Name("semibold".into())).size(15.0).color(t.text));
                ui.add_space(10.0);
                self.workspace_menus(ui);
                ui.add_space(16.0);
                let mut ws = self.workspace;
                ui.allocate_ui(Vec2::new(420.0, 28.0), |ui| segmented(ui, t, &[(Workspace::Scroll, "Scroll"), (Workspace::Chip, "Chip"), (Workspace::Rococo, "Rococo"), (Workspace::Cartouche, "Cartouche"), (Workspace::Palmette, "Palmette")], &mut ws));
                self.set_workspace(ws);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| { ui.label(egui::RichText::new("Runs offline · no AI").small().color(t.dim)); crate::platform::about_link(ui, t.dim); });
            });
        });
    }

    /// The active workspace's File, Edit and View menus.
    fn workspace_menus(&mut self, ui: &mut egui::Ui) {
        match self.workspace { Workspace::Chip => self.chip_menus(ui), Workspace::Rococo => self.rococo_menus(ui), Workspace::Cartouche => self.cartouche_menus(ui), Workspace::Palmette => self.palmette_menus(ui), Workspace::Scroll => self.scroll_menus(ui) }
    }
    /// Switch workspace, remembering it for next time.
    fn set_workspace(&mut self, ws: Workspace) {
        if ws == self.workspace { return; }
        self.workspace = ws;
        self.set_prefs(Prefs { chip: ws == Workspace::Chip, rococo: ws == Workspace::Rococo, cartouche: ws == Workspace::Cartouche, palmette: ws == Workspace::Palmette, ..self.prefs });
    }

    /// The scroll workspace's File, Edit and View menus (both layouts).
    fn scroll_menus(&mut self, ui: &mut egui::Ui) {
                ui.menu_button("File", |ui| {
                    if ui.add(egui::Button::new("New…").shortcut_text("Ctrl+N")).clicked() { self.open_new_dialog(); ui.close_menu(); }
                    if ui.add(egui::Button::new("Open…").shortcut_text("Ctrl+O")).clicked() { ui.close_menu(); self.ask(Pending::Open); }
                    if ui.add(egui::Button::new("Save").shortcut_text("Ctrl+S")).clicked() { ui.close_menu(); self.save(false); }
                    if ui.add(egui::Button::new("Save As…").shortcut_text("Ctrl+Shift+S")).clicked() { ui.close_menu(); self.save(true); }
                    ui.separator();
                    if ui.button("Export pattern SVG…").clicked() { ui.close_menu(); self.export(false); }
                    if ui.button("Export carving guides SVG…").clicked() { ui.close_menu(); self.export(true); }
                });
                ui.menu_button("Edit", |ui| {
                    if ui.add_enabled(!self.past.is_empty(), egui::Button::new("Undo").shortcut_text("Ctrl+Z")).clicked() { self.undo(); ui.close_menu(); }
                    if ui.add_enabled(!self.future.is_empty(), egui::Button::new("Redo").shortcut_text("Ctrl+Y")).clicked() { self.redo(); ui.close_menu(); }
                    ui.separator();
                    if ui.add_enabled(self.selected_part().is_some(), egui::Button::new("Delete leaf").shortcut_text("Del")).clicked() { self.remove_selected(); ui.close_menu(); }
                    if ui.add_enabled(!self.layout.shoots.is_empty(), egui::Button::new("Clear all leaf edits")).clicked() { let mut n = self.layout.clone(); n.shoots.clear(); self.commit(n); self.selected = None; ui.close_menu(); }
                });
                ui.menu_button("View", |ui| {
                    if ui.button("Fit page").clicked() { self.fitted = false; ui.close_menu(); }
                    ui.checkbox(&mut self.show_guides, "Backbone guides");
                    ui.checkbox(&mut self.grid, "Millimetre grid");
                    if ui.checkbox(&mut self.carving, "Carving guides").changed() { self.stale = true; }
                    ui.separator();
                    ui.menu_button("Theme", |ui| {
                        for id in ThemeId::ALL { if ui.selectable_label(self.prefs.theme == id, id.theme().name).clicked() { self.set_prefs(Prefs { theme: id, ..self.prefs }); ui.close_menu(); } }
                    });
                    let mut shelf = self.prefs.shelf;
                    if ui.checkbox(&mut shelf, "ZBrush-style layout").on_hover_text("Shelves along the top and left, palettes on the right. Untick for the classic panels.").changed() { self.set_prefs(Prefs { shelf, ..self.prefs }); ui.close_menu(); }
                });
    }

    fn status_bar(&mut self, ctx: &egui::Context) {
        let t = self.t();
        egui::TopBottomPanel::bottom("status").frame(egui::Frame::none().fill(t.bg).inner_margin(egui::Margin::symmetric(14.0, 5.0))).show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.visuals_mut().override_text_color = Some(t.dim);
                ui.style_mut().override_text_style = Some(egui::TextStyle::Small);
                ui.label(format!("{} × {} mm", self.layout.width, self.layout.height));
                ui.separator();
                ui.label(format!("{:.0}%", self.zoom / 3.78 * 100.0));
                ui.separator();
                if let Some(c) = self.cursor_mm { ui.label(format!("x {:.1}  y {:.1} mm", c.x, c.y)); }
                ui.separator();
                ui.label(if self.message.is_empty() { self.grown.message.clone() } else { self.message.clone() });
            });
        });
    }

    fn tool_strip(&mut self, ctx: &egui::Context) {
        let t = self.t();
        egui::SidePanel::left("tools").exact_width(56.0).resizable(false).frame(egui::Frame::none().fill(t.bg).inner_margin(egui::Margin::symmetric(8.0, 10.0))).show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.spacing_mut().item_spacing.y = 6.0;
                let tools: [(Tool, &str, fn(&egui::Painter, Rect, Color32)); 3] = [
                    (Tool::Select, "Select  (V)\nDrag leaves and backbone handles", icon_select),
                    (Tool::Pen, "Draw backbone  (P)\nSketch a sweep to add a new backbone", icon_pen),
                    (Tool::Transform, "Transform  (T)\nMove, scale, rotate or flip a backbone with its leaves", icon_transform)];
                for (tool, tip, icon) in tools {
                    if tool_button(ui, t, self.tool == tool, tip, icon) { self.tool = tool; if tool == Tool::Transform { self.selected = None; } }
                }
                ui.add_space(4.0);
                let (line, _) = ui.allocate_exact_size(Vec2::new(24.0, 1.0), Sense::hover());
                ui.painter().rect_filled(line, 0.0, t.border);
                ui.add_space(4.0);
                if tool_button(ui, t, self.tab == Tab::Leaves, "Leaf library\nAdd a measured leaf to the backbone", icon_leaf) { self.tab = Tab::Leaves; }
            });
        });
    }

    fn side_panel(&mut self, ctx: &egui::Context) {
        let t = self.t();
        egui::SidePanel::right("panel").default_width(340.0).min_width(300.0).frame(egui::Frame::none().fill(t.bg).inner_margin(egui::Margin { left: 16.0, right: 12.0, top: 12.0, bottom: 8.0 })).show(ctx, |ui| {
            let mut tab = self.tab;
            segmented(ui, t, &[(Tab::Properties, "Design"), (Tab::Leaves, "Library"), (Tab::Layers, "Layers"), (Tab::Carving, "Carve"), (Tab::Theme, "Theme")], &mut tab);
            self.tab = tab;
            ui.add_space(4.0);
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                ui.set_width(ui.available_width() - 4.0);
                match self.tab {
                    Tab::Properties => self.properties(ui),
                    Tab::Leaves => self.leaves(ui),
                    Tab::Layers => self.layers(ui),
                    Tab::Carving => self.carving_tab(ui),
                    Tab::Theme => self.theme_tab(ui),
                }
                ui.add_space(12.0);
            });
        });
    }

    fn properties(&mut self, ui: &mut egui::Ui) {
        if self.selected_part().is_some() { self.shoot_properties(ui); }
        else if self.tool != Tool::Transform { self.construction(ui); }
        if self.tool == Tool::Transform {
            section(ui, self.t(), "Transform");
            self.transform_section(ui);
        }
        section(ui, self.t(), "Backbone");
        self.backbone_section(ui);
        self.page_section(ui);
    }

    fn transform_section(&mut self, ui: &mut egui::Ui) {
        ui.label(egui::RichText::new("Drag inside the box to move, a corner to scale, the round knob to rotate (Shift snaps to 15°). Leaves stay attached.").small().color(self.t().dim));
        ui.horizontal(|ui| { if ui.button("Flip horizontal").clicked() { self.flip(Axis::Horizontal); } if ui.button("Flip vertical").clicked() { self.flip(Axis::Vertical); } });
    }

    /// The selected backbone: which one, its fork and what it grows.
    fn backbone_section(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let n = self.layout.curves.len();
            egui::ComboBox::from_id_salt("backbone").selected_text(format!("Backbone {}", self.backbone + 1)).show_ui(ui, |ui| { for i in 0..n { ui.selectable_value(&mut self.backbone, i, format!("Backbone {}", i + 1)); } });
            if ui.add_enabled(n > 1, egui::Button::new("Remove")).clicked() { self.remove_backbone(); }
        });
        ui.label(egui::RichText::new("Draw another with the Pen tool (P). Start the stroke on a stem to grow it from that stem.").small().color(self.t().dim));
        if let Some(parent) = self.layout.growth_for(self.backbone).attach {
            ui.horizontal(|ui| {
                ui.label(format!("Grows from backbone {}", parent + 1));
                if ui.button("Detach").on_hover_text("Stop growing from that stem; the scroll stays where it is").clicked() { let mut g = self.settings(); g.attach = None; self.set_settings(g); }
            });
            let g = self.settings();
            ui.horizontal(|ui| {
                let mut on = g.collar.is_some();
                let toggled = ui.checkbox(&mut on, "Collar at the fork").on_hover_text("Leafage dressing the join, as in baroque acanthus, where a branch rarely leaves its stem bare: an axil leaf lying over the fork, or a split sheath opening along both stems.").changed();
                let mut size = g.collar.unwrap_or(1.0) * 100.0;
                let resized = on && ui.add(egui::Slider::new(&mut size, 60.0..=180.0).text("%").fixed_decimals(0)).changed();
                if toggled || resized { let mut g = g.clone(); g.collar = if on { Some(size / 100.0) } else { None }; self.set_settings(g); }
            });
            if g.collar.is_some() {
                ui.horizontal(|ui| {
                    let now = g.collar_style.as_deref().and_then(CollarStyle::from_id).unwrap_or(CollarStyle::Axil);
                    let mut pick = now;
                    for st in CollarStyle::ALL { ui.selectable_value(&mut pick, st, st.name()); }
                    if pick != now { let mut g = g.clone(); g.collar_style = Some(pick.id().into()); self.set_settings(g); }
                });
            }
        }
        let mut g = self.settings(); let before = g.clone();
        ui.add_space(6.0);
        // what this backbone grows: the usual scroll, or a scroll vine
        let mut vine = g.vine.is_some();
        segmented(ui, self.t(), &[(false, "Scroll"), (true, "Scroll vine")], &mut vine);
        if vine != g.vine.is_some() { g.vine = if vine { Some(55.0) } else { None }; }
        if let Some(spacing) = g.vine {
            ui.label(egui::RichText::new("Curls grow from seed points along the backbone, each as large as fits, branching into smaller curls that curl the other way. None touch, and all fit inside the carving surface (Page, below).").small().color(self.t().dim));
            ui.horizontal(|ui| {
                let mut seed = g.seed as i64;
                ui.label("Variation"); if ui.add(egui::DragValue::new(&mut seed).range(0..=99999)).changed() { g.seed = seed as u32; }
                if ui.button("New").clicked() { g.seed = g.seed.wrapping_mul(1103515245).wrapping_add(12345) % 100000; }
            });
            let mut s = spacing;
            if ui.add(egui::Slider::new(&mut s, 20.0..=150.0).text("Seed spacing").suffix(" mm").fixed_decimals(0)).on_hover_text("How far apart curls start along the backbone. Further apart gives fewer, larger curls.").changed() { g.vine = Some(s); }
            let mut leaves = g.leaves > 0;
            if ui.checkbox(&mut leaves, "Acanthus leaves").on_hover_text("Clad each curl with an acanthus leaf on the outside of its turn (broad belly, two fingers a lobe). Off gives plain carved scrolls.").changed() { g.leaves = if leaves { 2 } else { 0 }; }
            // curls edited by hand: the vine is built from them until it is regrown
            let b = self.backbone;
            let edited = self.layout.shoots.iter().filter(|e| e.backbone == b && e.params.preset.as_deref() == Some(VINE_CURL)).count();
            if edited > 0 {
                ui.label(egui::RichText::new(format!("{edited} curls kept as edited. Changing Variation or Seed spacing, or Regrow, grows the vine afresh (Undo brings your edits back).")).small().color(self.t().dim));
            }
            // always offered: with hand edits it drops them; on an untouched vine
            // (which would regrow the same) it grows the next variation
            let tip = if edited > 0 { "Grow the vine afresh from its Variation, dropping the curls edited by hand (Undo brings them back)" } else { "Grow the vine afresh as the next variation" };
            let regrow = ui.button("Regrow vine").on_hover_text(tip).clicked();
            if regrow && edited == 0 { g.seed = (g.seed + 1) % 100000; }
            let reseeded = g.seed != before.seed || g.vine != before.vine;
            if g != before || regrow {
                let mut next = self.layout.clone();
                while next.growth.len() < next.curves.len() { let g0 = next.growth_for(next.growth.len()); next.growth.push(g0); }
                next.growth[b] = g.clone();
                if regrow || reseeded { next.shoots.retain(|e| !(e.backbone == b && e.params.preset.as_deref() == Some(VINE_CURL))); }
                self.commit(next);
                if regrow || reseeded { self.selected = None; }
            }
            return;
        }
        let fam = g.family.unwrap_or(Family::Spiral);
        egui::ComboBox::from_label("Pattern family").selected_text(family_label(fam)).show_ui(ui, |ui| {
            for f in [Family::Spiral, Family::Branching, Family::Border, Family::Spray, Family::Fan] { if ui.selectable_label(fam == f, family_label(f)).clicked() { g.family = Some(f); } }
        });
        ui.horizontal(|ui| {
            let mut seed = g.seed as i64;
            ui.label("Variation"); if ui.add(egui::DragValue::new(&mut seed).range(0..=99999)).changed() { g.seed = seed as u32; }
            if ui.button("New").clicked() { g.seed = g.seed.wrapping_mul(1103515245).wrapping_add(12345) % 100000; }
        });
        let mut auto = g.auto_shoots != Some(false); if ui.checkbox(&mut auto, "Grow automatic shoots").changed() { g.auto_shoots = Some(auto); }
        let mut fit = g.free != Some(true); if ui.checkbox(&mut fit, "Fit scroll inside page").on_hover_text("Shrinks the scroll's eye and shoots to stay on the page. Moving, rotating or flipping turns this off so the design keeps its shape.").changed() { g.free = Some(!fit); }
        ui.horizontal(|ui| {
            ui.label("Wrapping leaves").on_hover_text("Leaves laid into the scroll's curl, following it toward the eye. The volute opens up to make room.");
            let mut w = g.wraps.unwrap_or(0);
            for (v, name) in [(0u8, "None"), (1, "One"), (2, "Two")] { ui.selectable_value(&mut w, v, name); }
            if w != g.wraps.unwrap_or(0) { g.wraps = if w == 0 { None } else { Some(w) }; }
        });
        if g.wraps.unwrap_or(0) > 0 {
            let current = g.wrap_leaf.as_deref().and_then(|id| LEAF_PRESETS.iter().find(|p| p.id == id)).map(|p| p.name).unwrap_or("Generated acanthus");
            let mut pick: Option<Option<String>> = None;
            egui::ComboBox::from_label("Wrap leaf").selected_text(current).show_ui(ui, |ui| {
                if ui.selectable_label(g.wrap_leaf.is_none(), "Generated acanthus").clicked() { pick = Some(None); }
                for p in LEAF_PRESETS { if ui.selectable_label(g.wrap_leaf.as_deref() == Some(p.id), p.name).clicked() { pick = Some(Some(p.id.to_string())); } }
            }).response.on_hover_text("A library leaf laid into the curl, bent round it toward the eye. One layer only.");
            if let Some(v) = pick { g.wrap_leaf = v; }
        }
        let mut leaves = g.leaves > 0; if ui.checkbox(&mut leaves, "Leaf lobes and folds").changed() { g.leaves = if leaves { 2 } else { 0 }; }
        let mut eyes = g.eyes.unwrap_or(0) > 0;
        if ui.add_enabled(g.leaves > 0, egui::Checkbox::new(&mut eyes, "Eyes in the leaves")).on_hover_text("Each notch runs on as a narrow slit ending in a round eye. A single leaf can differ (select it).").changed() { g.eyes = if eyes { Some(2) } else { None }; }
        let mut clad = g.vine_leaf == Some(true);
        if ui.checkbox(&mut clad, "Vine acanthus leaves").on_hover_text("Clad the scroll and its shoots in the scroll vine's acanthus leaf (broad belly, two fingers a lobe) instead of the usual leaf. Library leaves and buds keep their own shapes.").changed() { g.vine_leaf = if clad { Some(true) } else { None }; }
        let mut two = g.levels == 2; if ui.checkbox(&mut two, "Three accent shoots (spiral)").changed() { g.levels = if two { 2 } else { 1 }; }
        let mut companion = g.sweeps == Some(2); if ui.checkbox(&mut companion, "Supporting sweep").changed() { g.sweeps = Some(if companion { 2 } else { 1 }); }
        let mut sec = g.secondary_scale.unwrap_or(1.0); if ui.add(egui::Slider::new(&mut sec, 0.5..=2.0).text("Shoot size")).changed() { g.secondary_scale = Some(sec); }
        egui::ComboBox::from_label("Curl side").selected_text(match g.side { Side::Alternate => "Alternate", Side::Left => "Left", Side::Right => "Right" }).show_ui(ui, |ui| {
            ui.selectable_value(&mut g.side, Side::Alternate, "Alternate"); ui.selectable_value(&mut g.side, Side::Left, "Left"); ui.selectable_value(&mut g.side, Side::Right, "Right");
        });
        if fam == Family::Spiral && g.composition != Some(2) {
            let was = (g.volute_size.is_some() || g.volute_turns.is_some()).then(|| (g.volute_size.unwrap_or(1.0), g.volute_turns.unwrap_or(1.0)));
            let mut v = was;
            volute_controls(ui, &mut v);
            if v != was { g.volute_size = v.map(|v| v.0); g.volute_turns = v.map(|v| v.1); }
        }
        if g != before { self.set_settings(g); }
    }

    /// Page size, carving surface and export options.
    fn page_section(&mut self, ui: &mut egui::Ui) {
        let t = self.t();
        section(ui, t, "Page");
        let mut inches = self.prefs.inches;
        let picked = page_sizes::size_editor(ui, t, &mut self.page_sizes, &mut inches, self.layout.width, self.layout.height, 40.0..=1000.0, false);
        if inches != self.prefs.inches { self.set_prefs(Prefs { inches, ..self.prefs }); }
        if let Some((w, h)) = picked {
            if w != self.layout.width || h != self.layout.height {
                let next = resized(&self.layout, w, h);
                self.commit(next); self.fitted = false; self.skel_thumbs = vec![None; Skeleton::ALL.len()];
            }
        }
        // the carving surface: growth stays inside it (a scroll vine fills it)
        let now = self.layout.surface.clone();
        let label = |s: Option<&str>| match s { Some("plaque") => "Plaque", Some("oval") => "Oval", Some("rectangle") => "Rectangle", _ => "Whole page" };
        let mut pick = now.clone();
        egui::ComboBox::from_label("Carving surface").selected_text(label(now.as_deref())).show_ui(ui, |ui| {
            for s in [None, Some("plaque"), Some("oval"), Some("rectangle")] { ui.selectable_value(&mut pick, s.map(String::from), label(s)); }
        }).response.on_hover_text("The shape you will carve on, fitted to the page with a small border. Scrolls stay inside it, and a scroll vine grows to fill it.");
        if pick != now { let mut n = self.layout.clone(); n.surface = pick; self.commit(n); }
        let mut pb = self.layout.print_backbone; if ui.checkbox(&mut pb, "Include backbone line in SVG").changed() { let mut n = self.layout.clone(); n.print_backbone = pb; self.commit(n); }
    }

    /// Named constructions of linked scrolls. Building one replaces the
    /// design (Undo brings it back); everything stays editable afterwards.
    fn construction(&mut self, ui: &mut egui::Ui) {
        let t = self.t();
        section(ui, t, "Construction");
        // Thumbnails: one per frame, grown at the page's proportions.
        if let Some(i) = self.skel_thumbs.iter().position(|x| x.is_none()) {
            let (w, h) = (self.layout.width, self.layout.height);
            let d = layered_drawing(&skeleton_layout(Skeleton::ALL[i], 0, w, h).grow()); let s = w.max(h);
            self.skel_thumbs[i] = Some(d.outline.iter().map(|r| r.iter().map(|p| pt(p.x / s, p.y / s)).collect()).collect());
            ui.ctx().request_repaint();
        }
        let gap = 6.0; let w = (ui.available_width() - gap) / 2.0;
        let mut build: Option<(Skeleton, u32)> = None;
        for (row, pair) in Skeleton::ALL.chunks(2).enumerate() {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = gap;
                for (k, kind) in pair.iter().enumerate() {
                    let i = row * 2 + k;
                    let (resp, painter) = ui.allocate_painter(Vec2::new(w, w * 0.74), Sense::click());
                    let r = resp.rect; let sel = self.skeleton == Some(*kind);
                    painter.rect_filled(r, 8.0, if resp.hovered() { t.hover } else { t.surface });
                    if sel { painter.rect_stroke(r, 8.0, Stroke::new(2.0, t.text)); }
                    let art = Rect::from_min_max(r.min + Vec2::new(8.0, 6.0), Pos2::new(r.right() - 8.0, r.bottom() - 22.0));
                    let aspect = (self.layout.height / self.layout.width.max(self.layout.height)) as f32;
                    let side = art.width().min(art.height() / aspect.max(0.1));
                    let origin = Pos2::new(art.center().x - side / 2.0, art.center().y - side * aspect / 2.0);
                    if let Some(Some(lines)) = self.skel_thumbs.get(i) { for l in lines { painter.add(Shape::line(l.iter().map(|p| origin + Vec2::new(p.x as f32 * side, p.y as f32 * side)).collect(), Stroke::new(0.9, t.text))); } }
                    painter.text(Pos2::new(r.left() + 8.0, r.bottom() - 11.0), egui::Align2::LEFT_CENTER, kind.name(), egui::FontId::proportional(12.5), t.text);
                    let resp = resp.on_hover_text(kind.detail());
                    if resp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
                    if resp.clicked() { build = Some((*kind, if sel { self.skel_seed } else { 0 })); }
                }
            });
        }
        if let Some(kind) = self.skeleton {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(format!("{} · variation {}", kind.name(), self.skel_seed + 1)).color(t.text));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("New variation").clicked() { build = Some((kind, self.skel_seed.wrapping_add(1))); }
                });
            });
        }
        ui.label(egui::RichText::new("Builds linked scrolls on your page. It replaces the current design; Undo brings it back. Every backbone stays editable.").small().color(t.dim));
        // dress the whole construction in the vine's acanthus leaf (now and for the next ones built)
        let mut clad = self.skel_vine_leaf;
        if ui.checkbox(&mut clad, "Clad in the vine acanthus").on_hover_text("Every scroll of the construction wears the scroll vine's acanthus leaf instead of the usual leaf. Each backbone can still be changed under Backbone.").changed() {
            self.skel_vine_leaf = clad;
            let mut next = self.layout.clone();
            while next.growth.len() < next.curves.len() { let g0 = next.growth_for(next.growth.len()); next.growth.push(g0); }
            for g in next.growth.iter_mut().filter(|g| g.vine.is_none()) { g.vine_leaf = if clad { Some(true) } else { None }; }
            self.commit(next);
        }
        if let Some((kind, seed)) = build {
            let mut next = skeleton_layout(kind, seed, self.layout.width, self.layout.height);
            if self.skel_vine_leaf { for g in next.growth.iter_mut().filter(|g| g.vine.is_none()) { g.vine_leaf = Some(true); } }
            next.print_backbone = self.layout.print_backbone;
            self.commit(next);
            self.skeleton = Some(kind); self.skel_seed = seed;
            self.backbone = 0; self.selected = None;
            self.message = format!("{}: {}.", kind.name(), kind.detail());
        }
    }

    fn shoot_properties(&mut self, ui: &mut egui::Ui) {
        let Some(part) = self.selected_part() else { return };
        let sh = part.shoot.clone().unwrap();
        let under = self.selected_edit().map_or(false, |e| e.under);
        let edited = self.selected_edit().map_or(false, |e| e.replaces.is_some());
        let bud = sh.preset.as_deref().is_some_and(is_bud);
        if sh.preset.as_deref() == Some(VINE_CURL) {
            section(ui, self.t(), "Selected curl");
            ui.label(egui::RichText::new("Drag the curl to slide it along the stem or curl it grows from; drag the open dot to swing and size it. Editing one curl keeps the whole vine as you have it; Regrow vine (under Backbone) starts afresh.").small().color(self.t().dim));
            let mut size = sh.reach * 100.0;
            if ui.add(egui::Slider::new(&mut size, 5.0..=150.0).text("Size (% of stem)").fixed_decimals(0)).changed() { self.patch_shoot(|e| e.params.reach = size / 100.0); }
            let mut angle = sh.turn.to_degrees();
            if ui.add(egui::Slider::new(&mut angle, -120.0..=120.0).text("Angle °").fixed_decimals(0)).on_hover_text("Its heading where it leaves the stem, against the stem's own direction.").changed() { self.patch_shoot(|e| e.params.turn = angle.to_radians()); }
            let mut roll = sh.curl * 100.0;
            if ui.add(egui::Slider::new(&mut roll, 30.0..=100.0).text("Roll %").fixed_decimals(0)).on_hover_text("How far it rolls into its eye.").changed() { self.patch_shoot(|e| e.params.curl = roll / 100.0); }
            if self.settings().leaves > 0 {
                let mut width = sh.leaf_scale.unwrap_or(1.0);
                if ui.add(egui::Slider::new(&mut width, 0.3..=1.8).text("Leaf width")).changed() { self.patch_shoot(|e| e.params.leaf_scale = Some(width)); }
            }
            ui.horizontal(|ui| {
                if ui.button("Mirror").on_hover_text("Turn the other way.").clicked() { self.patch_shoot(|e| { e.params.side = -e.params.side; e.params.turn = -e.params.turn; }); }
                if ui.button("Delete").on_hover_text("Remove this curl and the curls growing from it.").clicked() { self.remove_selected(); }
            });
            return;
        }
        section(ui, self.t(), if bud { "Selected bud" } else { "Selected leaf" });
        let measured = sh.preset.as_deref().and_then(profile);
        if bud {
            let current = sh.preset.as_deref().and_then(|id| BUD_PRESETS.iter().find(|p| p.id == id)).map(|p| p.name).unwrap_or("Bud");
            let mut pick: Option<&'static str> = None;
            egui::ComboBox::from_label("Bud type").selected_text(current).show_ui(ui, |ui| { for p in BUD_PRESETS { if ui.selectable_label(sh.preset.as_deref() == Some(p.id), p.name).clicked() { pick = Some(p.id); } } });
            if let Some(id) = pick { self.patch_shoot(|e| e.params.preset = Some(id.into())); return; }
            ui.label(egui::RichText::new("Drag the filled dot along the stem to move the bud; drag the open dot to swing and size it. At the very end of a stem, set Angle to 0 to finish the stem with it.").small().color(self.t().dim));
        } else {
            let current = sh.preset.as_deref().and_then(|id| LEAF_PRESETS.iter().find(|p| p.id == id)).map(|p| p.name).unwrap_or("Generated acanthus");
            let mut pick: Option<&'static str> = None;
            egui::ComboBox::from_label("Leaf type").selected_text(current).show_ui(ui, |ui| { for p in LEAF_PRESETS { if ui.selectable_label(sh.preset.as_deref() == Some(p.id), p.name).clicked() { pick = Some(p.id); } } });
            if let Some(id) = pick { let prof = profile(id).unwrap(); self.patch_shoot(|e| { e.params.preset = Some(id.into()); e.params.turn = prof.frame * e.params.side; e.params.bend = Some(0.0); e.params.leaf_scale = Some(1.0); e.params.leaf_side = None; e.params.lobes = None; e.params.depth = None; e.params.stalk = None; e.params.taper = None; e.params.stem = None; }); return; }
            ui.label(egui::RichText::new("Drag the filled dot along the stem to move the root; drag the open dot to swing and size the leaf.").small().color(self.t().dim));
        }
        let mut size = sh.reach * 100.0;
        if ui.add(egui::Slider::new(&mut size, 2.0..=if bud { 40.0 } else { 80.0 }).text("Size (% of stem)").fixed_decimals(0)).changed() { self.patch_shoot(|e| e.params.reach = size / 100.0); }
        if bud {
        } else if measured.is_some() {
            let mut bend = sh.bend.unwrap_or(0.0); if ui.add(egui::Slider::new(&mut bend, -1.5..=1.5).text("Bend")).changed() { self.patch_shoot(|e| e.params.bend = Some(bend)); }
            let mut width = sh.leaf_scale.unwrap_or(1.0); if ui.add(egui::Slider::new(&mut width, 0.6..=1.6).text("Width")).changed() { self.patch_shoot(|e| e.params.leaf_scale = Some(width)); }
            let mut follow = sh.follow.unwrap_or(0.0) * 100.0;
            if ui.add(egui::Slider::new(&mut follow, 0.0..=100.0).text("Follow stem %").fixed_decimals(0)).on_hover_text("Bend the leaf along the stem it grows from, round the volute. It bends only as far as it fits.").changed() { self.patch_shoot(|e| e.params.follow = if follow > 0.0 { Some(follow / 100.0) } else { None }); }
        } else {
            let mut curl = sh.curl; if ui.add(egui::Slider::new(&mut curl, 0.2..=1.2).text("Curl")).changed() { self.patch_shoot(|e| e.params.curl = curl); }
            // a generated leaf's eyes follow its backbone unless set here
            let follows = self.settings().eyes.unwrap_or(0);
            let mut eyes = sh.eyes.unwrap_or(follows) > 0;
            if ui.checkbox(&mut eyes, "Eyes").on_hover_text("Slits ending in round eyes at this leaf's notches. Untouched leaves follow the backbone's Eyes setting.").changed() { self.patch_shoot(|e| e.params.eyes = Some(if eyes { 2 } else { 0 })); }
        }
        if !bud {
            ui.horizontal(|ui| {
                ui.label("Leaf fan").on_hover_text("Grow smaller copies of this leaf from the same node, sized 100 / 66 / 33, splaying away from the stem behind it.");
                let now = sh.fan.unwrap_or(1).clamp(1, 3); let mut n = now;
                for (v, name) in [(1u8, "Single"), (2, "Two"), (3, "Three")] { ui.selectable_value(&mut n, v, name); }
                if n != now { self.patch_shoot(|e| e.params.fan = if n > 1 { Some(n) } else { None }); }
            });
        }
        let lean = measured.map_or(0.0, |p| sh.side * p.frame);
        let mut angle = (sh.turn - lean).to_degrees();
        if ui.add(egui::Slider::new(&mut angle, -180.0..=180.0).text("Angle °").fixed_decimals(0)).changed() { self.patch_shoot(|e| e.params.turn = angle.to_radians() + lean); }
        let mut on_top = !under; if ui.checkbox(&mut on_top, "On top").changed() { self.patch_shoot(|e| e.under = !on_top); }
        ui.label(egui::RichText::new("Unticked leaves sit beneath the main sweep. The root stays joined either way.").small().color(self.t().dim));
        ui.horizontal(|ui| {
            if bud { if ui.button("Mirror").clicked() { self.patch_shoot(|e| { e.params.turn = -e.params.turn; e.params.side = -e.params.side; }); } }
            else if ui.button("Mirror").clicked() { self.patch_shoot(|e| { let frame = e.params.preset.as_deref().and_then(profile).map_or(0.0, |p| p.frame); if frame != 0.0 { e.params.turn -= 2.0 * e.params.side * frame; } e.params.side = -e.params.side; e.params.leaf_side = None; }); }
            if ui.button("Delete").clicked() { self.remove_selected(); }
            if edited && ui.button("Return to generated").clicked() {
                if let Some(e) = self.selected_edit().cloned() { let mut n = self.layout.clone(); n.shoots.retain(|x| !(x.id == e.id && x.backbone == e.backbone)); self.commit(n); self.selected = None; }
            }
        });
    }

    fn leaves(&mut self, ui: &mut egui::Ui) {
        let t = self.t();
        // My presets: saved scroll designs
        if self.scroll_thumbs.len() != self.scroll_presets.entries.len() { self.scroll_thumbs = vec![None; self.scroll_presets.entries.len()]; }
        // Grow at most one missing thumbnail per frame to keep the panel responsive.
        if let Some(i) = self.scroll_thumbs.iter().position(|t| t.is_none()) {
            let thumb = io::parse(&self.scroll_presets.entries[i].data.to_string()).map(|l| { let d = layered_drawing(&l.grow()); let s = l.width.max(l.height);
                d.outline.iter().map(|r| r.iter().map(|p| pt(p.x / s, p.y / s)).collect()).collect() }).unwrap_or_default();
            self.scroll_thumbs[i] = Some(thumb);
            ui.ctx().request_repaint();
        }
        let current: serde_json::Value = serde_json::from_str(&io::save(&self.layout)).unwrap();
        let thumbs = self.scroll_thumbs.clone();
        let mut load: Option<Layout> = None;
        section(ui, t, "My presets");
        presets_panel(ui, t, &mut self.scroll_presets, &thumbs, current, "Scroll", |d| { if let Ok(l) = io::parse(&d.to_string()) { load = Some(l); } });
        if self.scroll_thumbs.len() != self.scroll_presets.entries.len() { self.scroll_thumbs.clear(); }
        if let Some(l) = load { self.commit(l); self.backbone = 0; self.selected = None; self.fitted = false; self.message = "Preset loaded. Undo returns to your previous design.".into(); }
        section(ui, self.t(), "Leaf types");
        ui.label(egui::RichText::new(format!("Click to grow one from backbone {}. Each grows on its own measured spine.", self.backbone + 1)).small().color(self.t().dim));
        ui.add_space(6.0);
        for p in LEAF_PRESETS { if library_card(ui, self.t(), p.id, p.name, p.detail) { self.add_leaf(p.id); } }
        section(ui, self.t(), "Buds");
        ui.label(egui::RichText::new("Finish a stem, or set one where two leaves part from the same root.").small().color(self.t().dim));
        ui.add_space(6.0);
        for p in BUD_PRESETS { if library_card(ui, self.t(), p.id, p.name, p.detail) { self.add_leaf(p.id); } }
    }

    fn layers(&mut self, ui: &mut egui::Ui) {
        section(ui, self.t(), "Layers");
        ui.label(egui::RichText::new("Top of the list is drawn on top.").small().color(self.t().dim));
        let parts: Vec<(String, String, bool)> = self.grown.parts.iter().rev().map(|p| {
            let name = if p.parent.is_none() { "Main sweep".to_string() } else if p.id.rsplit('/').next().is_some_and(|s| s.starts_with("wrap-")) { "Wrapping leaf".to_string() } else if p.id.contains("~fan") { "Fan leaf (select the lead leaf)".to_string() } else if p.id.rsplit('/').next().is_some_and(|s| s.starts_with("collar")) { "Collar leaf".to_string() } else if let Some(n) = p.shoot.as_ref().and_then(|s| s.preset.as_deref()).and_then(|id| LEAF_PRESETS.iter().find(|q| q.id == id).map(|q| q.name).or_else(|| BUD_PRESETS.iter().find(|q| q.id == id).map(|q| q.name))) { n.to_string() } else { "Acanthus shoot".to_string() };
            let (b, _) = self.split_id(&p.id);
            (p.id.clone(), if self.multi() { format!("{name} · backbone {}", b + 1) } else { name }, p.parent.is_some() && p.shoot.is_some())
        }).collect();
        for (id, name, selectable) in parts {
            let sel = self.selected.as_deref() == Some(id.as_str());
            if ui.add_enabled(selectable, egui::SelectableLabel::new(sel, name)).clicked() { self.selected = Some(id); self.tool = Tool::Select; }
        }
    }

    fn carving_tab(&mut self, ui: &mut egui::Ui) {
        section(ui, self.t(), "Carving guides");
        if ui.checkbox(&mut self.carving, "Show suggested relief").changed() { self.stale = true; }
        ui.label(egui::RichText::new("Solid lines: visible edges. Blue dashed: raised ridges (midribs). Red dotted: recessed creases. Suggestions to review, not cutting depths or toolpaths. Exports are always black on white.").small().color(self.t().dim));
        ui.add_space(8.0);
        if ui.button("Export carving guides SVG…").clicked() { self.export(true); }
        if ui.button("Export pattern SVG…").clicked() { self.export(false); }
    }

    fn theme_tab(&mut self, ui: &mut egui::Ui) {
        let t = self.t();
        section(ui, t, "Layout");
        let mut shelf = self.prefs.shelf;
        segmented(ui, t, &[(true, "ZBrush shelves"), (false, "Classic panels")], &mut shelf);
        if shelf != self.prefs.shelf { self.set_prefs(Prefs { shelf, ..self.prefs }); }
        section(ui, t, "Theme");
        for id in ThemeId::ALL {
            let th = id.theme();
            let sel = self.prefs.theme == id;
            let (resp, painter) = ui.allocate_painter(Vec2::new(ui.available_width(), 78.0), Sense::click());
            let r = resp.rect;
            painter.rect_filled(r, 10.0, th.bg);
            painter.rect_stroke(r, 10.0, if sel { Stroke::new(2.0, t.text) } else if resp.hovered() { Stroke::new(1.0, t.dim) } else { Stroke::new(1.0, t.border) });
            // miniature: desk, page and a small scroll in the theme's ink
            let desk = Rect::from_min_size(r.min + Vec2::new(10.0, 10.0), Vec2::new(86.0, 58.0));
            painter.rect_filled(desk, 6.0, th.desk);
            let page = desk.shrink2(Vec2::new(12.0, 8.0));
            painter.rect_filled(page, 2.0, th.paper);
            painter.add(Shape::line(mini_scroll(page), Stroke::new(1.3, th.ink)));
            // swatches
            for (k, col) in [th.surface, th.accent, th.text].iter().enumerate() {
                painter.circle_filled(Pos2::new(r.right() - 18.0 - k as f32 * 16.0, r.top() + 18.0), 5.5, *col);
            }
            let x = desk.right() + 12.0;
            painter.text(Pos2::new(x, r.top() + 20.0), egui::Align2::LEFT_CENTER, th.name, egui::FontId::new(14.5, egui::FontFamily::Name("semibold".into())), th.text);
            let wrap = (r.right() - x - 10.0).max(60.0);
            let galley = painter.layout(th.blurb.to_string(), egui::FontId::proportional(12.0), th.dim, wrap);
            painter.galley(Pos2::new(x, r.top() + 34.0), galley, th.dim);
            if resp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
            if resp.clicked() { self.set_prefs(Prefs { theme: id, ..self.prefs }); }
            ui.add_space(4.0);
        }
        section(ui, t, "Canvas");
        let dark_page = t.paper.r() < 128;
        let mut white = self.prefs.white_page;
        if ui.add_enabled(dark_page, egui::Checkbox::new(&mut white, "Show the page as white paper")).changed() { self.set_prefs(Prefs { white_page: white, ..self.prefs }); }
        ui.label(egui::RichText::new(if dark_page { "Keeps the dark interface but previews the pattern as it will print." } else { "This theme already shows a light page." }).small().color(t.dim));
        ui.label("Root joins");
        let mut joins = self.prefs.joins;
        ui.horizontal(|ui| {
            ui.radio_value(&mut joins, Joins::Exact, "Exact").on_hover_text("Tidies tangled outlines, and rounds the crotch where a leaf or branch grows from its stem with a true fillet.");
            ui.radio_value(&mut joins, Joins::Smooth, "Smooth").on_hover_text("The earlier soft blend around roots.");
            ui.radio_value(&mut joins, Joins::Classic, "Classic").on_hover_text("Both outlines simply cut away at the root.");
        });
        if joins != self.prefs.joins { self.set_prefs(Prefs { joins, ..self.prefs }); self.stale = true; }
        let resp = ui.add_enabled(joins == Joins::Exact, egui::Slider::new(&mut self.fillet_draft, 0.3..=1.5).text("Fillet").custom_formatter(|v, _| format!("{v:.1} mm")).step_by(0.1));
        // redrawing takes a moment, so apply on release
        if resp.drag_stopped() || (resp.changed() && !resp.dragged()) { self.set_prefs(Prefs { fillet: self.fillet_draft, ..self.prefs }); self.stale = true; }
        section(ui, t, "Interface size");
        let resp = ui.add(egui::Slider::new(&mut self.scale_draft, 0.8..=1.5).custom_formatter(|v, _| format!("{:.0}%", v * 100.0)).step_by(0.05));
        // Rescaling mid-drag would move the slider under the pointer, so apply on release.
        if resp.drag_stopped() || (resp.changed() && !resp.dragged()) { self.set_prefs(Prefs { ui_scale: self.scale_draft, ..self.prefs }); }
        ui.label(egui::RichText::new("Scales all text and controls. Saved for next time.").small().color(t.dim));
    }

    fn flip(&mut self, axis: Axis) {
        let center = self.part_bounds().center();
        let mut next = self.layout.clone();
        while next.growth.len() < next.curves.len() { let g = next.growth_for(next.growth.len()); next.growth.push(g); }
        for b in self.group() {
            next.curves[b] = flip_curve(&next.curves[b], axis, center);
            next.growth[b].flip = Some(next.growth[b].flip != Some(true));
            next.growth[b].free = Some(true);
            for e in next.shoots.iter_mut().filter(|e| e.backbone == b) { e.params = mirror_shoot(&e.params); }
        }
        next.locked_parts.clear();
        self.commit(next);
    }
    fn remove_backbone(&mut self) {
        let b = self.backbone; let mut next = self.layout.clone();
        next.curves.remove(b); if b < next.growth.len() { next.growth.remove(b); }
        next.shoots.retain(|e| e.backbone != b); for e in next.shoots.iter_mut() { if e.backbone > b { e.backbone -= 1; } }
        for g in next.growth.iter_mut() { g.attach = match g.attach { Some(a) if a == b => None, Some(a) if a > b => Some(a - 1), other => other }; }
        next.locked_parts.clear(); self.commit(next); self.backbone = 0; self.selected = None;
    }

    // ---------- canvas ----------
    fn canvas(&mut self, ui: &mut egui::Ui) {
        let (resp, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
        let rect = resp.rect;
        // (a browser canvas can be zero-sized for its first frames: fit once it has a size)
        if !self.fitted && rect.width() > 120.0 && rect.height() > 120.0 { self.fit(rect); }
        // zoom about the cursor, pan with middle/right drag or space+drag
        if let Some(hover) = resp.hover_pos() {
            let (scroll, zoom_delta) = ui.input(|i| (i.smooth_scroll_delta.y, i.zoom_delta()));
            let factor = if zoom_delta != 1.0 { zoom_delta } else { (scroll * 0.0015).exp() };
            if (factor - 1.0).abs() > 1e-4 {
                let before = self.to_mm(hover);
                self.zoom = (self.zoom * factor).clamp(0.3, 60.0);
                let after = self.to_screen(before);
                self.origin += hover - after;
            }
            self.cursor_mm = Some(self.to_mm(hover));
        }
        let pan_button = ui.input(|i| i.pointer.middle_down() || i.pointer.secondary_down() || i.key_down(egui::Key::Space));
        let c = self.canvas_colors();
        let ink = c.ink; let guide = c.guide; let mark = c.mark; let ridge = c.ridge; let crease = c.crease; let sheet = c.paper;
        let paper = Rect::from_min_max(self.to_screen(pt(0.0, 0.0)), self.to_screen(pt(self.layout.width, self.layout.height)));
        for (grow, alpha) in [(10.0, 10u8), (5.0, 16), (2.0, 24)] { painter.rect_filled(paper.expand(grow).translate(Vec2::new(0.0, grow * 0.4)), grow + 2.0, Color32::from_black_alpha(alpha)); }
        painter.rect_filled(paper, 2.0, sheet);
        if self.grid { self.paint_grid(&painter, paper); }
        // the carving surface, if one is set
        if let Some(outline) = self.layout.surface_polygon() {
            let pts: Vec<Pos2> = outline.iter().map(|p| self.to_screen(*p)).collect();
            painter.add(Shape::closed_line(pts, Stroke::new(1.2, with_alpha(guide, 160))));
        }
        let stroke_w = (0.45 * self.zoom).clamp(1.6, 3.0);
        let fold_w = (0.2 * self.zoom).clamp(0.7, 1.4);
        // selected highlight under the ink
        if let Some(part) = self.selected_part() {
            let poly: Vec<Pos2> = part.polygon.iter().map(|p| self.to_screen(*p)).collect();
            painter.add(Shape::closed_line(poly, Stroke::new(stroke_w * 5.0, with_alpha(mark, 45))));
        }
        if let (true, Some(g)) = (self.carving, self.guides.as_ref()) {
            for r in &g.outline { painter.add(Shape::line(r.iter().map(|p| self.to_screen(*p)).collect(), Stroke::new(stroke_w, ink))); }
            for r in &g.ridges { painter.extend(Shape::dashed_line(&r.iter().map(|p| self.to_screen(*p)).collect::<Vec<_>>(), Stroke::new(fold_w, ridge), 6.0, 3.0)); }
            for r in &g.creases { painter.extend(Shape::dotted_line(&r.iter().map(|p| self.to_screen(*p)).collect::<Vec<_>>(), crease, 3.5, fold_w * 0.6)); }
        } else {
            for r in &self.drawing.outline { painter.add(Shape::line(r.iter().map(|p| self.to_screen(*p)).collect(), Stroke::new(stroke_w, ink))); }
            for r in &self.drawing.folds { painter.add(Shape::line(r.iter().map(|p| self.to_screen(*p)).collect(), Stroke::new(fold_w, ink))); }
        }
        // backbone guides and handles
        if self.show_guides && self.tool != Tool::Transform {
            for (i, c) in self.layout.curves.iter().enumerate() {
                let pts: Vec<Pos2> = (0..=60).map(|k| { let t = k as f64 / 60.0; self.to_screen(scroll_core::geometry::at(c, t)) }).collect();
                let w = if i == self.backbone { 1.2 } else { 2.0 };
                painter.extend(Shape::dashed_line(&pts, Stroke::new(w, guide), 6.0, 5.0));
            }
            let c = self.layout.curves[self.backbone];
            // themes with a highlight draw the selected backbone solid in it
            let hl = self.t().highlight;
            if let Some(h) = hl {
                let pts: Vec<Pos2> = (0..=80).map(|k| self.to_screen(scroll_core::geometry::at(&c, k as f64 / 80.0))).collect();
                painter.add(Shape::line(pts, Stroke::new(2.0, h)));
            }
            let (arm, ring) = match hl { Some(h) => (Stroke::new(1.2, h), h), None => (Stroke::new(1.0, guide), mark) };
            painter.extend(Shape::dashed_line(&[self.to_screen(c[0]), self.to_screen(c[1])], arm, 3.0, 3.0));
            painter.extend(Shape::dashed_line(&[self.to_screen(c[2]), self.to_screen(c[3])], arm, 3.0, 3.0));
            for (i, p) in c.iter().enumerate() { painter.circle(self.to_screen(*p), 5.5, if i == 0 || i == 3 { ring } else { sheet }, Stroke::new(1.5, ring)); }
            // the volute's tip, ringed like the eye it sits in
            if let Some(v) = self.layout.volute_handle(self.backbone) {
                let tip = self.to_screen(v.tip);
                painter.extend(Shape::dashed_line(&[self.to_screen(c[3]), tip], arm, 2.0, 4.0));
                painter.circle(tip, 6.0, sheet, Stroke::new(2.0, ring)); painter.circle_filled(tip, 2.2, ring);
            }
        }
        // selected leaf handles
        let mut handles: Option<(Pos2, Pos2)> = None;
        if self.tool == Tool::Select { if let Some(part) = self.selected_part() {
            let root = self.to_screen(part.points[0]); let tip = self.to_screen(tip_handle(part));
            painter.extend(Shape::dashed_line(&[root, tip], Stroke::new(1.0, mark), 4.0, 4.0));
            painter.circle(root, 6.5, mark, Stroke::new(1.5, sheet));
            painter.circle(tip, 6.5, sheet, Stroke::new(2.0, mark));
            handles = Some((root, tip));
        } }
        // transform box
        let mut xf_box: Option<(Rect, Pos2)> = None;
        if self.tool == Tool::Transform {
            let b = self.part_bounds();
            let r = Rect::from_min_max(self.to_screen(pt(b.l, b.t)), self.to_screen(pt(b.r, b.b))).expand(8.0);
            let knob = Pos2::new(r.center().x, r.top() - 28.0);
            painter.rect_filled(r, 0.0, with_alpha(mark, 10));
            painter.extend(Shape::dashed_line(&[r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom(), r.left_top()], Stroke::new(1.2, mark), 6.0, 4.0));
            painter.line_segment([Pos2::new(r.center().x, r.top()), knob], Stroke::new(1.2, mark));
            painter.circle(knob, 7.0, sheet, Stroke::new(1.8, mark));
            for c in [r.left_top(), r.right_top(), r.left_bottom(), r.right_bottom()] { painter.rect(Rect::from_center_size(c, Vec2::splat(10.0)), 2.0, sheet, Stroke::new(1.6, mark)); }
            xf_box = Some((r, knob));
        }
        // pen trace
        if let Some(Drag::Draw { points }) = &self.drag { painter.add(Shape::line(points.iter().map(|p| self.to_screen(*p)).collect(), Stroke::new(2.0, c.mark))); }
        // held pick keys: B backbones, L leaves, C collars; what a click would
        // take is outlined under the pointer
        let pick = self.pick_mode(ui, &resp);
        if let (Some(mode), Some(hover)) = (pick, resp.hover_pos()) {
            let mm = self.to_mm(hover);
            let hl = self.t().highlight.unwrap_or(mark);
            let outline = |poly: &[Point]| Shape::closed_line(poly.iter().map(|p| self.to_screen(*p)).collect(), Stroke::new(stroke_w * 2.2, with_alpha(hl, 150)));
            match mode {
                Pick::Backbone => if let Some(b) = self.hit_backbone(mm, hover) {
                    let pts: Vec<Pos2> = (0..=80).map(|k| self.to_screen(scroll_core::geometry::at(&self.layout.curves[b], k as f64 / 80.0))).collect();
                    painter.add(Shape::line(pts, Stroke::new(4.0, with_alpha(hl, 170))));
                    let pre = format!("backbone-{b}/");
                    for p in self.grown.parts.iter().filter(|p| p.parent.is_none() && (!self.multi() || p.id.starts_with(&pre))) { painter.add(outline(&p.polygon)); }
                },
                Pick::Leaf => if let Some(id) = self.hit_leaf(mm) { if let Some(p) = self.grown.parts.iter().find(|p| p.id == id) { painter.add(outline(&p.polygon)); } },
                Pick::Collar => for p in self.grown.parts.iter().filter(|p| is_collar(p) && inside(mm, &p.polygon)) { painter.add(outline(&p.polygon)); },
            }
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }

        // ----- interaction -----
        let pointer = resp.interact_pointer_pos();
        if resp.drag_started() {
            // Hit-test where the button went down: by the time egui reports a
            // drag the pointer has moved past its threshold, off small handles.
            let pos = ui.input(|i| i.pointer.press_origin()).or(pointer).unwrap_or_default();
            let mm = self.to_mm(pos);
            self.drag_before = Some(self.layout.clone());
            self.drag = if pan_button { Some(Drag::Pan) } else { match self.tool {
                Tool::Pen => Some(Drag::Draw { points: vec![mm] }),
                Tool::Transform => xf_box.map(|(r, knob)| {
                    let b = self.part_bounds(); let center = b.center(); let curves: Vec<(usize, Curve)> = self.group().into_iter().map(|i| (i, self.layout.curves[i])).collect();
                    let corner = [r.left_top(), r.right_top(), r.left_bottom(), r.right_bottom()].iter().any(|c| c.distance(pos) < 12.0);
                    let kind = if knob.distance(pos) < 14.0 { TransformKind::Rotate } else if corner { TransformKind::Scale } else { TransformKind::Move };
                    Some(Drag::Transform { kind, center, from: mm, curves })
                }).flatten().map(|d| { self.set_free(); d }),
                // Left-drag on empty paper does nothing; pan with middle or
                // right drag, or hold Space.
                Tool::Select => self.pick_drag(pos, mm, handles),
            } };
        }
        if resp.dragged() {
            let delta = resp.drag_delta();
            let pos = pointer.unwrap_or_default(); let mm = self.to_mm(pos); let shift = ui.input(|i| i.modifiers.shift);
            match self.drag.as_mut() {
                Some(Drag::Pan) => self.origin += delta,
                Some(Drag::Draw { points }) => points.push(mm),
                Some(Drag::Handle { index }) => { let i = *index; let b = self.backbone;
                    // An attached scroll's start slides along its parent's stem, keeping its shape.
                    if !(i == 0 && self.layout.slide_attached(b, mm)) { self.layout.curves[b][i] = pt(mm.x.clamp(0.0, self.layout.width), mm.y.clamp(0.0, self.layout.height)); }
                    self.layout.locked_parts.clear(); self.stale = true; }
                Some(Drag::Volute { vh }) => { let (size, turns, side) = vh.drag_to(mm); let b = self.backbone;
                    while self.layout.growth.len() < self.layout.curves.len() { let g = self.layout.growth_for(self.layout.growth.len()); self.layout.growth.push(g); }
                    let g = &mut self.layout.growth[b]; g.volute_size = Some(size); g.volute_turns = Some(turns); g.side = side;
                    self.layout.locked_parts.clear(); self.stale = true; }
                Some(Drag::ShootRoot { edit, backbone }) => { let (id, b) = (edit.clone(), *backbone); let pr = self.vine_progress(b, &id, mm).unwrap_or_else(|| nearest_progress(&self.layout.curves[b], mm)); if let Some(e) = self.layout.shoots.iter_mut().find(|e| e.id == id && e.backbone == b) { e.params.progress = pr; } self.stale = true; }
                Some(Drag::ShootTip { edit, start, root, handle }) => { let (reach, turn) = drag_tip(start, *root, *handle, mm); let id = edit.clone(); if let Some(e) = self.layout.shoots.iter_mut().find(|e| e.id == id) { e.params.reach = reach; e.params.turn = turn; } self.stale = true; }
                Some(Drag::Transform { kind, center, from, curves }) => { for (i, curve) in curves.iter() { self.layout.curves[*i] = transform_curve(curve, *kind, *center, *from, mm, shift); } self.layout.locked_parts.clear(); self.stale = true; }
                None => {}
            }
        }
        if resp.drag_stopped() {
            if let Some(Drag::Draw { points }) = self.drag.take() {
                if let Some(c) = fit_curve(&points) {
                    // Starting the stroke on an existing sweep grows the new
                    // scroll out of it.
                    let host = (0..self.layout.curves.len()).find(|&b| { let pre = format!("backbone-{b}/");
                        self.grown.parts.iter().any(|p| p.parent.is_none() && (!self.multi() || p.id.starts_with(&pre)) && inside(points[0], &p.polygon)) });
                    let mut next = self.layout.clone();
                    let mut g = next.growth_for(self.backbone); next.growth.resize(next.curves.len(), g.clone());
                    g.attach = host; g.free = Some(true); if host.is_some() { g.levels = 1; g.collar = Some(1.0); }
                    next.curves.push(c); next.growth.push(g);
                    self.commit(next);
                    self.backbone = self.layout.curves.len() - 1; self.tool = Tool::Select;
                    self.message = match host { Some(h) => format!("New scroll grows from backbone {}. Drag its handles to refine it.", h + 1), None => "Backbone drawn. Drag its four handles to refine the sweep.".into() };
                } else { self.message = "Draw a longer sweep to create a backbone.".into(); }
            } else if let Some(before) = self.drag_before.take() {
                if !matches!(self.drag, Some(Drag::Pan)) { self.past.push(before); self.future.clear(); self.dirty = true; }
            }
            self.drag = None; self.drag_before = None;
            if self.smooth_due { self.stale = true; }
        }
        if resp.clicked() && self.tool == Tool::Select {
            if let Some(pos) = pointer {
                let mm = self.to_mm(pos);
                match pick {
                    Some(Pick::Backbone) => if let Some(b) = self.hit_backbone(mm, pos) { self.pick_backbone(b, None); },
                    Some(Pick::Collar) => if let Some(b) = self.grown.parts.iter().rev().find(|p| is_collar(p) && inside(mm, &p.polygon)).map(|p| self.split_id(&p.id).0) {
                        self.pick_backbone(b, Some(format!("Collar at the fork of backbone {}: its style and size are under Backbone → Collar at the fork.", b + 1)));
                    },
                    Some(Pick::Leaf) => self.selected = self.hit_leaf(mm),
                    // a plain click takes a leaf, else the backbone under it
                    None => { self.selected = self.hit_leaf(mm); if self.selected.is_none() { if let Some(b) = self.hit_backbone(mm, pos) { self.pick_backbone(b, None); } } }
                }
            }
        }
    }

    /// Which pick key is held while the pointer is over the canvas.
    fn pick_mode(&self, ui: &egui::Ui, resp: &egui::Response) -> Option<Pick> {
        if !resp.hovered() || ui.ctx().wants_keyboard_input() || self.tool != Tool::Select { return None; }
        ui.input(|i| if i.key_down(egui::Key::B) { Some(Pick::Backbone) } else if i.key_down(egui::Key::L) { Some(Pick::Leaf) } else if i.key_down(egui::Key::C) { Some(Pick::Collar) } else { None })
    }
    /// The backbone under the pointer: the topmost part there (any of a
    /// backbone's leaves counts), else a backbone line within a few pixels.
    fn hit_backbone(&self, mm: Point, screen: Pos2) -> Option<usize> {
        if let Some(p) = self.grown.parts.iter().rev().find(|p| inside(mm, &p.polygon)) { return Some(if self.multi() { self.split_id(&p.id).0 } else { 0 }).filter(|&b| b < self.layout.curves.len()); }
        self.layout.curves.iter().enumerate().map(|(i, c)| (i, (0..=80).map(|k| self.to_screen(scroll_core::geometry::at(c, k as f64 / 80.0)).distance(screen)).fold(f32::INFINITY, f32::min)))
            .filter(|(_, d)| *d < 8.0).min_by(|a, b| a.1.partial_cmp(&b.1).unwrap()).map(|(i, _)| i)
    }
    fn pick_backbone(&mut self, b: usize, message: Option<String>) {
        self.backbone = b; self.selected = None;
        self.palettes[shelf::BACKBONE] = true;
        self.message = message.unwrap_or_else(|| format!("Backbone {} selected.", b + 1));
    }

    /// What a drag in the Select tool grabs, in priority order.
    fn pick_drag(&mut self, pos: Pos2, mm: Point, handles: Option<(Pos2, Pos2)>) -> Option<Drag> {
        if let (Some((root, tip)), Some(sel)) = (handles, self.selected.clone()) {
            let on_root = root.distance(pos) < 12.0; let on_tip = tip.distance(pos) < 12.0;
            if on_root || on_tip {
                let part = self.selected_part()?.clone();
                let mut next = self.layout.clone();
                let id = self.take_over(&mut next, &sel)?;
                let (b, _) = self.split_id(&sel);
                self.layout = next; self.stale = true;
                let e = self.layout.shoots.iter().find(|e| e.id == id && e.backbone == b)?.clone();
                self.selected = Some(self.display_id(&e));
                return Some(if on_root { Drag::ShootRoot { edit: id, backbone: b } } else { Drag::ShootTip { edit: id, start: e.params.clone(), root: part.points[0], handle: tip_handle(&part) } });
            }
        }
        if self.show_guides {
            if let Some(vh) = self.layout.volute_handle(self.backbone).filter(|v| self.to_screen(v.tip).distance(pos) < 10.0) { return Some(Drag::Volute { vh }); }
            for (i, p) in self.layout.curves[self.backbone].iter().enumerate() { if self.to_screen(*p).distance(pos) < 10.0 { return Some(Drag::Handle { index: i }); } }
            for (bi, c) in self.layout.curves.iter().enumerate() { if bi != self.backbone { for p in c { if self.to_screen(*p).distance(pos) < 10.0 { self.backbone = bi; } } } }
        }
        // grabbing a leaf body moves its root along the stem
        if let Some(id) = self.hit_leaf(mm) {
            self.selected = Some(id.clone());
            let mut next = self.layout.clone(); let local = self.take_over(&mut next, &id)?; let (b, _) = self.split_id(&id);
            self.layout = next; self.stale = true;
            if let Some(e) = self.layout.shoots.iter().find(|e| e.id == local && e.backbone == b).cloned() { self.selected = Some(self.display_id(&e)); }
            return Some(Drag::ShootRoot { edit: local, backbone: b });
        }
        None
    }
    fn hit_leaf(&self, mm: Point) -> Option<String> {
        // a fan's smaller leaves select the lead leaf that carries the fan
        self.grown.parts.iter().rev().find(|p| p.parent.is_some() && (p.shoot.is_some() || p.id.contains("~fan")) && inside(mm, &p.polygon)).map(|p| p.id.split("~fan").next().unwrap_or(&p.id).to_string())
    }
    fn paint_grid(&self, painter: &egui::Painter, paper: Rect) {
        let grid = self.canvas_colors().grid;
        let step = if self.zoom > 8.0 { 1.0 } else if self.zoom > 2.0 { 5.0 } else { 10.0 };
        let mut x = 0.0; while x <= self.layout.width { let a = self.to_screen(pt(x, 0.0)); painter.line_segment([Pos2::new(a.x, paper.top()), Pos2::new(a.x, paper.bottom())], Stroke::new(if (x as i64) % 10 == 0 { 0.8 } else { 0.4 }, grid)); x += step; }
        let mut y = 0.0; while y <= self.layout.height { let a = self.to_screen(pt(0.0, y)); painter.line_segment([Pos2::new(paper.left(), a.y), Pos2::new(paper.right(), a.y)], Stroke::new(if (y as i64) % 10 == 0 { 0.8 } else { 0.4 }, grid)); y += step; }
    }
}

/// A scroll's volute shaped by hand: its size and how far it rolls in,
/// against the automatic one, and back to automatic.
fn volute_controls(ui: &mut egui::Ui, volute: &mut Option<(f64, f64)>) {
    let (mut size, mut turns) = volute.unwrap_or((1.0, 1.0));
    let a = ui.add(egui::Slider::new(&mut size, VOLUTE_SIZE.0..=VOLUTE_SIZE.1).text("Volute size").custom_formatter(|n, _| format!("{n:.2}×")))
        .on_hover_text("The curl at the scroll's end against its automatic size. On the canvas, drag the ringed dot at its tip");
    let b = ui.add(egui::Slider::new(&mut turns, VOLUTE_TURNS.0..=VOLUTE_TURNS.1).text("Volute roll").custom_formatter(|n, _| format!("{n:.2}×")))
        .on_hover_text("How far the curl rolls in against the automatic one (about 1.2 turns): less opens it into a hook, more winds it tighter");
    if a.changed() || b.changed() { *volute = Some((size, turns)); }
    if volute.is_some() && ui.button("Automatic volute").on_hover_text("Let the volute size itself from the stem again").clicked() { *volute = None; }
}

/// Segmented control: a row of equal-width options in a rounded track.
fn segmented<T: PartialEq + Copy>(ui: &mut egui::Ui, t: &Theme, options: &[(T, &str)], value: &mut T) {
    egui::Frame::none().fill(t.surface).rounding(8.0).inner_margin(egui::Margin::same(3.0)).show(ui, |ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        let n = options.len() as f32;
        let w = ((ui.available_width() - 2.0 * (n - 1.0)) / n).max(30.0);
        ui.horizontal(|ui| {
            for (v, name) in options {
                let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 26.0), Sense::click());
                let sel = *value == *v;
                ui.painter().rect_filled(rect, 6.0, if sel { t.accent } else if resp.hovered() { t.hover } else { Color32::TRANSPARENT });
                let fg = if sel { t.on_accent } else if resp.hovered() { t.text } else { t.dim };
                ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, *name, egui::FontId::new(13.0, if sel { egui::FontFamily::Name("semibold".into()) } else { egui::FontFamily::Proportional }), fg);
                if resp.clicked() { *value = *v; }
            }
        });
    });
}

/// "My presets": save the current design, and pick, rename, duplicate or
/// delete saved ones. Thumbnails are outlines in a unit square.
fn presets_panel(ui: &mut egui::Ui, t: &Theme, lib: &mut presets::Library, thumbs: &[Option<Vec<Vec<Point>>>], current: serde_json::Value, fallback: &str, mut load: impl FnMut(&serde_json::Value)) {
    ui.horizontal(|ui| {
        ui.add(egui::TextEdit::singleline(&mut lib.name).hint_text("Name this design").desired_width(ui.available_width() - 70.0));
        if ui.button("Save").on_hover_text("Save the current design as a preset").clicked() { lib.add(current, fallback); }
    });
    let cols = 2usize; let gap = 6.0;
    let w = (ui.available_width() - gap) / cols as f32;
    let mut clicked = None;
    for (row, chunk) in lib.entries.chunks(cols).enumerate() {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            for (k, e) in chunk.iter().enumerate() {
                let i = row * cols + k;
                let (resp, painter) = ui.allocate_painter(Vec2::new(w, w * 0.78), Sense::click());
                let r = resp.rect; let sel = lib.selected == Some(i);
                painter.rect_filled(r, 8.0, if resp.hovered() { t.hover } else { t.surface });
                if sel { painter.rect_stroke(r, 8.0, Stroke::new(2.0, t.text)); }
                let art = Rect::from_min_max(r.min + Vec2::new(8.0, 8.0), Pos2::new(r.right() - 8.0, r.bottom() - 24.0));
                let side = art.width().min(art.height());
                let origin = Pos2::new(art.center().x - side / 2.0, art.top());
                if let Some(Some(lines)) = thumbs.get(i) {
                    for l in lines { painter.add(Shape::line(l.iter().map(|p| origin + Vec2::new(p.x as f32 * side, p.y as f32 * side)).collect(), Stroke::new(1.0, t.text))); }
                }
                let name: String = if e.name.chars().count() > 24 { format!("{}…", e.name.chars().take(23).collect::<String>()) } else { e.name.clone() };
                painter.text(Pos2::new(r.left() + 8.0, r.bottom() - 12.0), egui::Align2::LEFT_CENTER, name, egui::FontId::proportional(12.5), t.text);
                if resp.clicked() { clicked = Some(i); }
            }
        });
    }
    if let Some(i) = clicked { lib.selected = Some(i); lib.name = lib.entries[i].name.clone(); load(&lib.entries[i].data.clone()); }
    if lib.selected.is_some() {
        ui.horizontal(|ui| {
            if ui.button("Rename").clicked() { lib.rename(); }
            if ui.button("Duplicate").clicked() { lib.duplicate(); }
            if ui.button("Delete").clicked() { lib.delete(); }
        });
    }
    if lib.deleted.is_some() && ui.button("Undo delete").clicked() { lib.undo_delete(); }
    if lib.entries.is_empty() { ui.label(egui::RichText::new("No presets yet. Name the current design and press Save.").small().color(t.dim)); }
    if !lib.message.is_empty() { ui.label(egui::RichText::new(&lib.message).small().color(t.dim)); }
}

/// A clickable library card: thumbnail, name and a one-line description.
fn library_card(ui: &mut egui::Ui, t: &Theme, id: &str, name: &str, detail: &str) -> bool {
    let (resp, painter) = ui.allocate_painter(Vec2::new(ui.available_width(), 80.0), Sense::click());
    painter.rect_filled(resp.rect, 8.0, if resp.hovered() { t.hover } else { t.surface });
    let thumb = Rect::from_min_size(resp.rect.min + Vec2::new(8.0, 8.0), Vec2::splat(64.0));
    draw_thumbnail(&painter, thumb, id, t.text);
    painter.text(Pos2::new(thumb.right() + 12.0, resp.rect.top() + 28.0), egui::Align2::LEFT_CENTER, name, egui::FontId::new(14.5, egui::FontFamily::Name("semibold".into())), t.text);
    painter.text(Pos2::new(thumb.right() + 12.0, resp.rect.top() + 50.0), egui::Align2::LEFT_CENTER, detail, egui::FontId::proportional(12.5), t.dim);
    if resp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
    ui.add_space(4.0);
    resp.clicked()
}

/// Small, dim, upper-case section label.
fn section(ui: &mut egui::Ui, t: &Theme, title: &str) {
    // inside a shelf palette of the same name the label would only repeat its header
    if ui.data(|d| d.get_temp::<String>(egui::Id::new(shelf::PALETTE_TITLE))).is_some_and(|p| p == title.to_uppercase()) { ui.add_space(4.0); return; }
    ui.add_space(12.0);
    ui.label(egui::RichText::new(title.to_uppercase()).family(egui::FontFamily::Name("semibold".into())).size(11.5).color(t.dim));
    ui.add_space(1.0);
}

fn with_alpha(c: Color32, a: u8) -> Color32 { Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a) }

/// A square icon button for the tool strip.
fn tool_button(ui: &mut egui::Ui, t: &Theme, selected: bool, tip: &str, icon: fn(&egui::Painter, Rect, Color32)) -> bool {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(40.0), Sense::click());
    let fill = if selected { t.accent } else if resp.hovered() { t.hover } else { Color32::TRANSPARENT };
    ui.painter().rect_filled(rect, 8.0, fill);
    icon(ui.painter(), rect.shrink(11.0), if selected { t.on_accent } else if resp.hovered() { t.text } else { t.dim });
    resp.on_hover_text(tip).clicked()
}

fn at_unit(r: Rect, x: f32, y: f32) -> Pos2 { Pos2::new(r.left() + x * r.width(), r.top() + y * r.height()) }

fn icon_select(p: &egui::Painter, r: Rect, c: Color32) {
    let pts = [(0.12, 0.0), (0.12, 0.84), (0.34, 0.64), (0.5, 1.0), (0.64, 0.94), (0.48, 0.58), (0.78, 0.56)];
    p.add(Shape::closed_line(pts.iter().map(|&(x, y)| at_unit(r, x, y)).collect(), Stroke::new(1.6, c)));
}
fn icon_pen(p: &egui::Painter, r: Rect, c: Color32) {
    let (a, b, d, e) = ((0.0f32, 0.95f32), (0.2, 0.1), (0.8, 0.95), (1.0, 0.08));
    let pts: Vec<Pos2> = (0..=24).map(|i| { let t = i as f32 / 24.0; let u = 1.0 - t;
        let x = u * u * u * a.0 + 3.0 * u * u * t * b.0 + 3.0 * u * t * t * d.0 + t * t * t * e.0;
        let y = u * u * u * a.1 + 3.0 * u * u * t * b.1 + 3.0 * u * t * t * d.1 + t * t * t * e.1; at_unit(r, x, y) }).collect();
    p.add(Shape::line(pts, Stroke::new(1.6, c)));
    p.circle_filled(at_unit(r, a.0, a.1), 2.4, c);
    p.circle_filled(at_unit(r, e.0, e.1), 2.4, c);
}
fn icon_transform(p: &egui::Painter, r: Rect, c: Color32) {
    let b = Rect::from_min_max(at_unit(r, 0.08, 0.3), at_unit(r, 0.92, 1.0));
    p.rect_stroke(b, 1.0, Stroke::new(1.4, c));
    for k in [b.left_top(), b.right_top(), b.left_bottom(), b.right_bottom()] { p.rect_filled(Rect::from_center_size(k, Vec2::splat(4.5)), 1.0, c); }
    let top = Pos2::new(b.center().x, b.top());
    let knob = at_unit(r, 0.5, 0.02);
    p.line_segment([top, Pos2::new(knob.x, knob.y + 2.5)], Stroke::new(1.4, c));
    p.circle_stroke(knob, 2.5, Stroke::new(1.4, c));
}
fn icon_leaf(p: &egui::Painter, r: Rect, c: Color32) {
    let (a, e) = (at_unit(r, 0.05, 0.95), at_unit(r, 0.95, 0.05));
    let axis = e - a; let n = Vec2::new(-axis.y, axis.x).normalized();
    let side = |s: f32| -> Vec<Pos2> { (0..=16).map(|i| { let t = i as f32 / 16.0; a + axis * t + n * s * (std::f32::consts::PI * t).sin().powf(0.8) * r.width() * 0.3 }).collect() };
    let mut outline = side(1.0); let mut back = side(-1.0); back.reverse(); outline.extend(back);
    p.add(Shape::closed_line(outline, Stroke::new(1.5, c)));
    p.line_segment([a, a + axis * 0.8], Stroke::new(1.1, c));
}

/// A tiny scroll for the theme previews.
fn mini_scroll(page: Rect) -> Vec<Pos2> {
    let mut pts = vec![];
    for i in 0..=20 { let t = i as f32 / 20.0; pts.push(at_unit(page, 0.08 + t * 0.55, 0.78 - (t * 1.6).sin() * 0.35)); }
    let c = at_unit(page, 0.74, 0.42);
    let start = *pts.last().unwrap();
    let r0 = (start - c).length(); let a0 = (start.y - c.y).atan2(start.x - c.x);
    for i in 1..=28 { let t = i as f32 / 28.0; let a = a0 + t * 5.2; let r = r0 * (1.0 - t * 0.72); pts.push(Pos2::new(c.x + a.cos() * r, c.y + a.sin() * r)); }
    pts
}

fn family_label(f: Family) -> &'static str { match f { Family::Spiral => "Enclosing spiral", Family::Branching => "Branching scroll", Family::Border => "Rolling border", Family::Spray => "Flowing spray", Family::Fan => "Fan flourish" } }

/// Library card thumbnail: the leaf grown from a short straight stem.
fn draw_thumbnail(painter: &egui::Painter, rect: Rect, id: &str, ink: Color32) {
    use scroll_core::growth::Kind;
    let guide: Vec<Point> = (0..61).map(|i| pt(10.0 + i as f64 * 1.5, 92.0 - i as f64 * 1.3)).collect();
    let parent = GrowthPart { id: "stem".into(), parent: None, kind: Kind::Primary, points: guide.clone(), polygon: guide.clone(), folds: vec![], ridges: None, cuts: vec![], length: 0.0, width: 1.0, birth: 0.0, duration: 1.0, contour_split: None, shoot: None, under: false };
    let Some(mut params) = preset_params(id, 0.05, 1.0) else { return };
    params.reach = if is_bud(id) { 0.35 } else { 0.8 };
    if is_bud(id) { params.turn = 0.0; }
    let leaf = scroll_core::shoots::grow_shoot(&guide, &parent, &params, "thumb", true);
    let b = Bounds::of(&leaf.polygon); let s = (b.r - b.l).max(b.b - b.t).max(1e-6);
    let map = |p: &Point| Pos2::new(rect.left() + ((p.x - b.l) / s) as f32 * rect.width() * 0.9 + rect.width() * 0.05, rect.top() + ((p.y - b.t) / s) as f32 * rect.height() * 0.9 + rect.height() * 0.05);
    painter.add(Shape::closed_line(leaf.polygon.iter().map(map).collect(), Stroke::new(1.2, ink)));
    for c in &leaf.cuts { painter.add(Shape::line(c.iter().map(map).collect(), Stroke::new(1.2, ink))); }
    for f in &leaf.folds { painter.add(Shape::line(f.iter().map(map).collect(), Stroke::new(0.7, ink))); }
}
