//! The Cartouche workspace: designs from the cartouche library
//! (`scroll_core::cartouche`, the banked cartouche studies) edited on the
//! page. Drag a scroll's points and handles to reshape it (its mirrored
//! copies follow, and so do the scrolls growing out of it); slide a leaf fan
//! along its scroll; drag jewels, bosses, shells and straps; size the frame by
//! its handles; select anything to change it; add scrolls, leaf fans and
//! accents; export the SVG at actual size.
//!
//! The scrolls are grown by the scroll engine. A scroll regrows on its own
//! (the others are kept, `GrowCache`); while anything changes the acanthus is
//! shown with each grown part's own outline, and the root joins (Canvas →
//! Root joins) are drawn afterwards, off the interface's thread on the desktop.
use super::*;
use crate::platform;
use crate::shelf::{self as sh, bevel_button, big_tab, caption, divider, seg, shelf_caption, tile_with};
use scroll_core::cartouche::{acanthus_layer, bounds_of, compose, draft_layer, in_shape, part_owner, structure, Acanthus, Design, Drawing, Element, FieldShape, Kind, Layer, Moulding, Owner, Paint, Repeat, STRUCTURES, START};
use scroll_core::geometry::arc_table;
use scroll_core::model::{GrowCache, JoinStyle};
use std::f64::consts::PI;
use web_time::Instant;

const AUTOSAVE: &str = "cartouche-current.json";

/// A handle on the selected element: a scroll's start, end and their
/// handles; a strap's point; the frame's width and height; an accent's aim
/// (direction and size); a leaf fan's root (slides along its scroll) and tip (swings and sizes it);
/// a scroll's volute tip (rolls it in or out, sizes it, and sets the side it curls to).
#[derive(Clone, Copy, PartialEq, Debug)]
enum Handle { Start, StartOut, EndIn, End, Point(usize), Width, Height, Aim, Root, Tip, Volute }

/// `LeafTip`: a leaf fan's tip, from where its root and tip were when the drag began (on the picked copy).
/// `Volute`: a scroll's volute tip, from the volute as grown when the drag began (on the picked copy).
enum CDrag { Pan, Handle { id: u32, copy: usize, h: Handle }, Body { id: u32, copy: usize, start: Point }, LeafTip { id: u32, copy: usize, root: Point, tip: Point }, Volute { id: u32, vh: VoluteHandle } }

/// What the acanthus is grown from: the design's centre, scroll weight,
/// collars, cuts, and its scrolls and leaves.
type Key = (Point, f64, bool, bool, Vec<Element>);
fn acanthus_key(d: &Design) -> Key { (d.centre, d.growth, d.collars, d.eyes, d.elements.iter().filter(|e| e.is_acanthus()).cloned().collect()) }

/// The grown acanthus: generation `gen`, the layout it came from, the parts,
/// each part's bounds and owner (scroll or leaf, and copy).
struct Grown { key: Key, gen: u64, acanthus: Acanthus, result: scroll_core::growth::GrowthResult, boxes: Vec<Bounds>, owners: Vec<Option<(u32, usize)>> }

/// Root joins being drawn for generation `gen`.
struct Job { gen: u64, joins: JoinStyle, #[cfg(not(target_arch = "wasm32"))] rx: std::sync::mpsc::Receiver<Layer> }

pub struct CartoucheState {
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
    handle: Option<Handle>,
    pub filled: bool,
    pub show_grid: bool,
    drag: Option<CDrag>,
    drag_before: Option<Design>,
    /// The scroll weight while its slider is dragged (applied on release: it regrows everything).
    growth_draft: Option<f64>,
    // cached geometry for the current design
    cached_for: Option<(Design, JoinStyle)>,
    changed_at: Instant,
    cache: GrowCache,
    grown: Option<Grown>,
    gen: u64,
    joined: Option<(u64, JoinStyle, Layer)>,
    job: Option<Job>,
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
    /// Library tiles: each structure's frame and stems, and its jewels, in a unit box.
    thumbs: Option<Vec<(Vec<Vec<Point>>, Vec<Point>)>>,
}

impl CartoucheState {
    pub fn new() -> CartoucheState {
        let text = platform::store_read(AUTOSAVE);
        let design = text.as_deref().and_then(|t| io::parse_cartouche(t).ok()).unwrap_or_else(|| Design::from_structure(START.0, START.1, START.2).unwrap());
        CartoucheState { design, structure: START.0.into(), path: None, dirty: false, past: vec![], future: vec![], last_edit: None, selected: None, handle: None, filled: true, show_grid: false,
            drag: None, drag_before: None, growth_draft: None, cached_for: None, changed_at: Instant::now(), cache: GrowCache::default(), grown: None, gen: 0, joined: None, job: None,
            layers: vec![], boxes: vec![], drawing: Drawing::default(), texture: None, texture_stale: true, autosaved: text, message: String::new(), zoom: 3.0, origin: Pos2::ZERO, fitted: false, cursor_mm: None, thumbs: None }
    }

    // ---------- edits and undo ----------
    fn change(&mut self, next: Design) { self.change_keyed(None, next); }
    /// Replace the design (attached scrolls settled onto their parents); edits
    /// with the same key in quick succession (a slider being dragged) make one undo step.
    fn change_keyed(&mut self, key: Option<String>, mut next: Design) {
        next.settle();
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
    fn edit(&mut self, id: u32, kind: Kind) {
        let mut d = self.design.clone();
        if let Some(e) = d.get_mut(id) { e.kind = kind; }
        self.change_keyed(Some(format!("edit-{id}")), d);
    }
    fn check_selection(&mut self) {
        if let Some((id, k)) = self.selected {
            match self.design.get(id) {
                None => { self.selected = None; self.handle = None; }
                Some(e) => { let n = self.design.copies_of(e).len(); if k >= n { self.selected = Some((id, 0)); } }
            }
        }
    }

    // ---------- cache ----------
    /// Bring the drawing up to date: regrow the scrolls that changed, compose
    /// the layers, and have the root joins drawn once things are quiet.
    fn refresh(&mut self, ctx: &egui::Context, joins: JoinStyle) {
        self.take_job(joins);
        let current = self.cached_for.as_ref().is_some_and(|(d, j)| *d == self.design && *j == joins);
        if !current {
            if self.cached_for.as_ref().is_none_or(|(d, _)| *d != self.design) { self.changed_at = Instant::now(); }
            let key = acanthus_key(&self.design);
            if self.grown.as_ref().map(|g| &g.key) != Some(&key) {
                self.gen += 1;
                let gen = self.gen;
                self.grown = self.design.acanthus().map(|a| {
                    let result = a.layout.grow_cached(&mut self.cache);
                    let boxes = result.parts.iter().map(|p| Bounds::of(&p.polygon)).collect();
                    let owners = result.parts.iter().map(|p| part_owner(&a, &p.id)).collect();
                    Grown { key, gen, acanthus: a, result, boxes, owners }
                });
            }
            let layer = self.grown.as_ref().map(|g| match &self.joined { Some((gen, j, l)) if *gen == g.gen && *j == joins => l.clone(), _ => draft_layer(&g.result) });
            self.layers = self.design.layers(layer.as_ref());
            self.boxes = self.layers.iter().map(|(_, l)| l.bounds()).collect();
            self.drawing = compose(&self.layers);
            self.cached_for = Some((self.design.clone(), joins));
            self.texture_stale = true;
            self.check_selection();
        }
        self.start_job(ctx, joins);
    }
    /// The root joins are drawn: whether this drawing has them.
    fn joined(&self, joins: JoinStyle) -> bool {
        match &self.grown { None => true, Some(g) => self.joined.as_ref().is_some_and(|(gen, j, _)| *gen == g.gen && *j == joins) }
    }
    /// Collect a finished join drawing (for the current acanthus, else dropped).
    fn take_job(&mut self, joins: JoinStyle) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let Some(job) = &self.job else { return };
            let got = match job.rx.try_recv() { Ok(l) => Some(l), Err(std::sync::mpsc::TryRecvError::Empty) => return, Err(_) => None };
            let (gen, j) = (job.gen, job.joins);
            self.job = None;
            if let Some(l) = got { if self.grown.as_ref().is_some_and(|g| g.gen == gen) && j == joins { self.joined = Some((gen, j, l)); self.cached_for = None; } }
        }
        #[cfg(target_arch = "wasm32")]
        { let _ = joins; }
    }
    /// Draw the root joins once nothing has changed for a moment (not while dragging).
    fn start_job(&mut self, ctx: &egui::Context, joins: JoinStyle) {
        if self.drag.is_some() || self.job.is_some() || self.growth_draft.is_some() || self.joined(joins) { return; }
        let quiet = self.changed_at.elapsed().as_secs_f32();
        if quiet < 0.3 { ctx.request_repaint_after(std::time::Duration::from_millis(320 - (quiet * 1000.0) as u64)); return; }
        let Some(g) = &self.grown else { return };
        let gen = g.gen;
        #[cfg(not(target_arch = "wasm32"))]
        {
            let (tx, rx) = std::sync::mpsc::channel();
            let (result, ctx) = (g.result.clone(), ctx.clone());
            std::thread::spawn(move || { let _ = tx.send(acanthus_layer(&result, joins)); ctx.request_repaint(); });
            self.job = Some(Job { gen, joins, rx });
        }
        #[cfg(target_arch = "wasm32")]
        {
            // no threads in the browser: draw them now
            let l = acanthus_layer(&g.result, joins);
            self.joined = Some((gen, joins, l)); self.cached_for = None;
            let _ = Job { gen, joins };
            ctx.request_repaint();
        }
    }
    /// Keep the current design on this computer.
    fn autosave(&mut self) {
        if self.drag.is_some() { return; }
        let text = io::save_cartouche(&self.design);
        if self.autosaved.as_ref() == Some(&text) { return; }
        let _ = platform::store_write(AUTOSAVE, &text);
        self.autosaved = Some(text);
    }

    // ---------- files ----------
    pub fn title(&self) -> String {
        let name = self.path.as_ref().and_then(|p| p.file_stem()).map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "Cartouche".into());
        format!("{name}{} — ORNATR", if self.dirty { " •" } else { "" })
    }
    /// A new design on a `w` × `h` page, from the last library structure chosen.
    pub fn new_design(&mut self, w: f64, h: f64) {
        let d = Design::from_structure(&self.structure, w, h).unwrap_or_else(|| Design::empty(w, h));
        self.change(d); self.path = None; self.dirty = false; self.selected = None; self.handle = None; self.fitted = false;
        self.message = format!("New {} × {} mm cartouche. Undo returns to the previous one.", w, h);
    }
    /// Start again from a library structure, fitted to the page.
    pub fn use_structure(&mut self, id: &str) {
        let Some(d) = Design::from_structure(id, self.design.width, self.design.height) else { return };
        self.structure = id.into();
        self.change(d); self.selected = None; self.handle = None;
        let name = STRUCTURES.iter().find(|s| s.0 == id).map_or(id, |s| s.1);
        self.message = format!("{name} fitted to the page. Drag a scroll's points to reshape it (its mirrored copies follow); Undo returns to the previous design.");
    }
    /// A picked file, once read (see `App::opened`, which sends cartouches here).
    pub fn open_text(&mut self, picked: platform::Opened) {
        let (path, text) = match picked { Ok(p) => p, Err(e) => { self.message = format!("Could not open this file. {e}"); return } };
        match io::parse_cartouche(&text) {
            Ok(d) => { self.change(d); self.path = Some(path); self.dirty = false; self.selected = None; self.handle = None; self.fitted = false; self.message = "Cartouche opened.".into(); }
            Err(e) => self.message = format!("Could not open this design. {e}"),
        }
    }
    pub fn save(&mut self, choose: bool) {
        let path = if choose || self.path.is_none() {
            match platform::choose_save("ORNATR cartouche", &["ornatr"], "cartouche.ornatr") { Some(p) => p, None => return }
        } else { self.path.clone().unwrap() };
        match platform::write_file(&path, &io::save_cartouche(&self.design)) { Ok(()) => { self.path = Some(path); self.dirty = false; self.message = "Cartouche saved.".into(); } Err(e) => self.message = format!("Could not save: {e}") }
    }
    pub fn export(&mut self, joins: JoinStyle) {
        let Some(p) = platform::choose_save("SVG", &["svg"], "cartouche-pattern.svg") else { return };
        // the drawing on screen, once its root joins are drawn
        let current = self.joined(joins) && self.cached_for.as_ref().is_some_and(|(d, j)| *d == self.design && *j == joins);
        let svg = if current { scroll_core::cartouche::svg_of(&self.drawing, self.design.width, self.design.height) } else { scroll_core::cartouche::svg_of(&self.design.drawing_with(joins), self.design.width, self.design.height) };
        match platform::write_file(&p, &svg) { Ok(()) => self.message = format!("Exported {}.", platform::shown(&p)), Err(e) => self.message = format!("Could not export: {e}") }
    }

    // ---------- selection ----------
    fn selected_element(&self) -> Option<&Element> { self.design.get(self.selected?.0) }
    /// The scroll the selection belongs to: the selected scroll, or the scroll a selected leaf fan is on.
    fn current_scroll(&self) -> Option<u32> {
        let e = self.selected_element()?;
        match &e.kind { Kind::Scroll { .. } => Some(e.id), Kind::Leaf { stem, .. } => Some(*stem), _ => None }
    }
    /// Delete the selected strap point (when one is picked and the strap
    /// keeps two), else the selected element (a scroll with its leaves).
    pub fn remove_selected(&mut self) {
        let Some((id, _)) = self.selected else { return };
        let mut d = self.design.clone();
        if let (Some(Handle::Point(k)), Some(Element { kind: Kind::Strap { ctrl, .. }, .. })) = (self.handle, d.get_mut(id)) {
            if ctrl.len() > 2 && k < ctrl.len() { ctrl.remove(k); self.handle = None; self.change(d); self.message = "Point removed.".into(); return; }
        }
        let label = d.get(id).map_or("Element", |e| e.label());
        let n = d.remove(id);
        self.change(d); self.selected = None; self.handle = None;
        self.message = if n > 1 { format!("{label} removed with its {} leaf fans. Undo brings them back.", n - 1) } else { format!("{label} removed. Undo brings it back.") };
    }
    pub fn restack(&mut self, front: bool) {
        let Some((id, _)) = self.selected else { return };
        let mut d = self.design.clone(); d.restack(id, front); self.change(d);
        if self.design.get(id).is_some_and(|e| e.is_acanthus()) { self.message = "The scrolls and their leaves are one carved body: they moved together.".into(); }
    }
    /// Add an element of `what` and select it.
    fn add(&mut self, what: &str) {
        let mut d = self.design.clone();
        let id = match what {
            "frame" => match d.add_frame() { Some(id) => Some(id), None => { self.message = "The design has a frame already: select it to change its shape.".into(); return } },
            "scroll" => Some(d.add_scroll()),
            "leaf" => match self.current_scroll() { Some(s) => d.add_leaf(s), None => { self.message = "Select a scroll first (or a leaf fan on it): the new leaf fan grows from that scroll.".into(); return } },
            other => d.add_accent(other),
        };
        let Some(id) = id else { return };
        let label = d.get(id).map_or("Element", |e| e.label());
        self.message = match what {
            "frame" => "Frame added round the centre, behind everything. Drag its handles to size it.".into(),
            "scroll" => "Scroll added over the frame's top, mirrored. Drag its points and handles to shape it; its copy follows.".into(),
            "leaf" => "Leaf fan added near the start of the scroll. Drag it along the scroll.".into(),
            "strap" | "opening" => format!("{label} added behind everything. Drag it into place."),
            _ => format!("{label} added at the top of the frame. Drag it into place; its handle sizes it."),
        };
        self.change(d); self.selected = Some((id, 0)); self.handle = None;
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

    /// The topmost element (and copy) under `mm`.
    fn hit(&self, mm: Point) -> Option<(u32, usize)> {
        for (i, (owner, layer)) in self.layers.iter().enumerate().rev() {
            if !self.boxes.get(i).copied().flatten().is_some_and(|b| b.contains(mm)) { continue; }
            match owner {
                Owner::Element(id, k) => if layer.cover.iter().any(|s| in_shape(mm, s)) || layer.pockets.iter().any(|p| inside(mm, p)) { return Some((*id, *k)); },
                Owner::Acanthus => if let Some(g) = &self.grown {
                    for (j, p) in g.result.parts.iter().enumerate().rev() { if g.boxes[j].contains(mm) && inside(mm, &p.polygon) { if let Some(o) = g.owners[j] { return Some(o); } } }
                },
            }
        }
        None
    }
    /// Handles on the selected copy, in mm.
    fn handles(&self) -> Vec<(Handle, Point)> {
        let Some((id, k)) = self.selected else { return vec![] };
        let Some(e) = self.design.get(id) else { return vec![] };
        let Some(xf) = self.design.copies_of(e).get(k).copied() else { return vec![] };
        // a leaf fan's are on its grown leaf, as in the scroll workspace
        if let Kind::Leaf { .. } = e.kind {
            return self.leaf_part(id, k).map(|p| vec![(Handle::Root, p.points[0]), (Handle::Tip, tip_handle(p))]).unwrap_or_default();
        }
        let c = self.design.centre;
        let aim = |p: Point, a: f64, l: f64| vec![(Handle::Aim, pt(p.x + a.cos() * l, p.y + a.sin() * l))];
        let src: Vec<(Handle, Point)> = match &e.kind {
            Kind::Frame { rx, ry, .. } => vec![(Handle::Width, pt(c.x + rx, c.y)), (Handle::Height, pt(c.x, c.y + ry))],
            Kind::Scroll { curve, .. } => vec![(Handle::Start, curve[0]), (Handle::StartOut, curve[1]), (Handle::EndIn, curve[2]), (Handle::End, curve[3])],
            Kind::Leaf { .. } => vec![],
            Kind::Jewel { c, rx, turn, bezel, .. } => aim(*c, *turn, rx + bezel),
            Kind::Boss { c, r } => aim(*c, 0.0, *r),
            Kind::Shell { c, axis, r, .. } => aim(*c, *axis, *r),
            Kind::Opening { c, rx, turn, .. } => aim(*c, *turn, *rx),
            Kind::Strap { ctrl, .. } => ctrl.iter().enumerate().map(|(i, p)| (Handle::Point(i), *p)).collect(),
        };
        let mut out: Vec<(Handle, Point)> = src.into_iter().map(|(h, p)| (h, xf.point(p))).collect();
        // the volute's tip, where the picked copy grows it
        if e.is_scroll() { if let Some(v) = self.design.volute_handle(id, k) { out.push((Handle::Volute, v.tip)); } }
        out
    }
    /// Copy `k` of leaf fan `id` as grown: its first, largest leaf.
    fn leaf_part(&self, id: u32, k: usize) -> Option<&scroll_core::growth::GrowthPart> {
        let g = self.grown.as_ref()?;
        g.result.parts.iter().zip(&g.owners).filter(|(p, o)| **o == Some((id, k)) && p.points.len() > 2).map(|(p, _)| p).max_by(|a, b| a.length.partial_cmp(&b.length).unwrap())
    }
    /// Apply the drag in progress with the pointer at `mm` (from the design the drag started on).
    fn drag_to(&mut self, mm: Point) {
        let Some(before) = self.drag_before.as_ref() else { return };
        let mut d = before.clone();
        let centre = d.centre;
        // a leaf fan's scroll stem (the source's: copies share the share along it)
        let stem_of = |d: &Design, id: u32| match d.get(id).map(|e| &e.kind) { Some(Kind::Leaf { stem, .. }) => match d.get(*stem).map(|s| &s.kind) { Some(Kind::Scroll { curve, .. }) => Some(*curve), _ => None }, _ => None };
        if let Some(CDrag::Volute { id, ref mut vh }) = self.drag {
            // the copy's own side (relative to its mirroring) is the source's
            let (size, turns, side) = vh.drag_to(mm);
            if let Some(Element { kind: Kind::Scroll { curl, volute, .. }, .. }) = d.get_mut(id) { *volute = Some((size, turns)); *curl = side; }
            self.design = d;
            return;
        }
        match self.drag {
            Some(CDrag::LeafTip { id, copy, root, tip }) => {
                let Some(xf) = d.get(id).and_then(|e| d.copies_of(e).get(copy).copied()) else { return };
                let Some(Element { kind: Kind::Leaf { size, turn, mirror_side, .. }, .. }) = d.get_mut(id) else { return };
                // as the scroll workspace's leaf tip: the distance from the root sizes it, the angle swings it;
                // a copy on the other side swings the source the other way
                let (before, after) = ((tip.x - root.x).hypot(tip.y - root.y), (mm.x - root.x).hypot(mm.y - root.y));
                if before < 1e-6 || after < 1e-6 { return; }
                let swing = (mm.y - root.y).atan2(mm.x - root.x) - (tip.y - root.y).atan2(tip.x - root.x);
                let flipped = *mirror_side && xf.reflects();
                *size = (*size * after / before).clamp(0.1, 6.0);
                *turn = wrap_angle(*turn + if flipped { -swing } else { swing });
            }
            Some(CDrag::Handle { id, copy, h }) => {
                let Some(xf) = d.get(id).and_then(|e| d.copies_of(e).get(copy).copied()) else { return };
                let p = xf.back(mm);
                let stem_curve = stem_of(&d, id);
                let Some(e) = d.get_mut(id) else { return };
                let dist = |a: Point, b: Point| (a.x - b.x).hypot(a.y - b.y);
                match (&mut e.kind, h) {
                    (Kind::Frame { rx, .. }, Handle::Width) => *rx = (p.x - centre.x).abs().max(3.0),
                    (Kind::Frame { ry, .. }, Handle::Height) => *ry = (p.y - centre.y).abs().max(3.0),
                    // a scroll's end carries its handle with it
                    (Kind::Scroll { curve, .. }, Handle::Start | Handle::End) => { let (a, b) = if h == Handle::Start { (0, 1) } else { (3, 2) }; let (dx, dy) = (p.x - curve[a].x, p.y - curve[a].y); for i in [a, b] { curve[i] = pt(curve[i].x + dx, curve[i].y + dy); } }
                    (Kind::Scroll { curve, .. }, Handle::StartOut) => curve[1] = p,
                    (Kind::Scroll { curve, .. }, Handle::EndIn) => curve[2] = p,
                    (Kind::Strap { ctrl, .. }, Handle::Point(i)) => if let Some(q) = ctrl.get_mut(i) { *q = p; },
                    // angle and size; ovals keep their proportions
                    (Kind::Jewel { c, rx, ry, turn, bezel }, Handle::Aim) => { let l = (dist(*c, p) - *bezel).max(0.5); *ry *= l / rx.max(1e-9); *rx = l; *turn = (p.y - c.y).atan2(p.x - c.x); }
                    (Kind::Opening { c, rx, ry, turn }, Handle::Aim) => { let l = dist(*c, p).max(1.0); *ry *= l / rx.max(1e-9); *rx = l; *turn = (p.y - c.y).atan2(p.x - c.x); }
                    (Kind::Boss { c, r }, Handle::Aim) => *r = dist(*c, p).max(0.5),
                    (Kind::Shell { c, axis, r, .. }, Handle::Aim) => { *axis = (p.y - c.y).atan2(p.x - c.x); *r = dist(*c, p).max(3.0); }
                    (Kind::Leaf { at, .. }, Handle::Root) => if let Some(c) = stem_curve { *at = nearest_progress(&c, p); },
                    _ => {}
                }
            }
            Some(CDrag::Body { id, copy, start }) => {
                let Some(xf) = d.get(id).and_then(|e| d.copies_of(e).get(copy).copied()) else { return };
                let (a, b) = (xf.back(start), xf.back(mm));
                let (dx, dy) = (b.x - a.x, b.y - a.y);
                let mv = |q: &mut Point| *q = pt(q.x + dx, q.y + dy);
                // a leaf fan slides along its scroll
                let stem_curve = stem_of(&d, id);
                let Some(e) = d.get_mut(id) else { return };
                match &mut e.kind {
                    Kind::Frame { .. } => {}
                    Kind::Scroll { curve, .. } => curve.iter_mut().for_each(mv),
                    Kind::Leaf { at, .. } => if let Some(c) = stem_curve { *at = nearest_progress(&c, b); },
                    Kind::Jewel { c, .. } | Kind::Boss { c, .. } | Kind::Shell { c, .. } | Kind::Opening { c, .. } => mv(c),
                    Kind::Strap { ctrl, .. } => ctrl.iter_mut().for_each(mv),
                }
            }
            _ => return,
        }
        d.settle();
        self.design = d;
    }
    /// Add a point to strap `id` where it was Alt-clicked, between the two points it falls between.
    fn insert_point(&mut self, id: u32, copy: usize, mm: Point) {
        let mut d = self.design.clone();
        let Some(xf) = d.get(id).and_then(|e| d.copies_of(e).get(copy).copied()) else { return };
        let p = xf.back(mm);
        let Some(Element { kind: Kind::Strap { ctrl, .. }, .. }) = d.get_mut(id) else { return };
        let seg_dist = |a: Point, b: Point| { let (vx, vy) = (b.x - a.x, b.y - a.y); let t = (((p.x - a.x) * vx + (p.y - a.y) * vy) / (vx * vx + vy * vy).max(1e-12)).clamp(0.0, 1.0); (a.x + vx * t - p.x).hypot(a.y + vy * t - p.y) };
        let Some(i) = (0..ctrl.len().saturating_sub(1)).min_by(|&i, &j| seg_dist(ctrl[i], ctrl[i + 1]).partial_cmp(&seg_dist(ctrl[j], ctrl[j + 1])).unwrap()) else { return };
        ctrl.insert(i + 1, p);
        self.selected = Some((id, copy)); self.handle = Some(Handle::Point(i + 1));
        self.change(d); self.message = "Point added. Drag it to reshape the strap; Delete removes it.".into();
    }

    /// The filled preview: wood, the field lighter, openings dark, painted back to front.
    fn raster(&self, k: f64) -> egui::ColorImage {
        let (w, h) = (((self.design.width * k).ceil() as usize).max(1), ((self.design.height * k).ceil() as usize).max(1));
        let mut px = vec![Color32::TRANSPARENT; w * h];
        for (paint, rings) in &self.drawing.paint { fill_rings(&mut px, w, h, rings, k, match paint { Paint::Wood => WOOD, Paint::Field => FIELD, Paint::Pocket => POCKET }); }
        egui::ColorImage { size: [w, h], pixels: px }
    }
    fn thumbs(&mut self) -> &Vec<(Vec<Vec<Point>>, Vec<Point>)> {
        self.thumbs.get_or_insert_with(|| STRUCTURES.iter().map(|(id, _, _)| {
            let d = structure(id).unwrap();
            let mut lines: Vec<Vec<Point>> = vec![];
            let mut dots: Vec<Point> = vec![];
            for e in &d.elements {
                let xs = d.copies_of(e);
                match &e.kind {
                    Kind::Frame { shape, rx, ry, round, .. } => { let mut f = scroll_core::cartouche::field_outline(*shape, d.centre, *rx, *ry, *round); f.push(f[0]); lines.push(f.into_iter().step_by(4).collect()); }
                    Kind::Scroll { curve, .. } => for x in &xs { lines.push(arc_table(&x.curve(curve)).into_iter().step_by(12).map(|r| r.point).collect()); },
                    Kind::Jewel { c, rx, .. } if *rx >= 4.0 => for x in &xs { dots.push(x.point(*c)); },
                    _ => {}
                }
            }
            let all: Vec<Point> = lines.iter().flatten().copied().collect();
            let b = Bounds::of(&all); let s = (b.r - b.l).max(b.b - b.t).max(1e-6);
            let (ox, oy) = (b.l + ((b.r - b.l) - s) / 2.0, b.t + ((b.b - b.t) - s) / 2.0);
            let norm = |p: &Point| pt((p.x - ox) / s, (p.y - oy) / s);
            (lines.iter().map(|l| l.iter().map(norm).collect()).collect(), dots.iter().map(norm).collect())
        }).collect())
    }
}

/// The nearest share along a stem to `p` (0 at its start, 1 at its end).
fn nearest_progress(c: &scroll_core::geometry::Curve, p: Point) -> f64 { scroll_core::shoots::nearest_progress(c, p).clamp(0.0, 1.0) }

pub(crate) const WOOD: Color32 = Color32::from_rgb(201, 167, 107);
const FIELD: Color32 = Color32::from_rgb(221, 196, 149);
const POCKET: Color32 = Color32::from_rgb(90, 90, 90);
pub(crate) const CARVED_INK: Color32 = Color32::from_rgb(107, 79, 34);

/// Fill rings (even-odd, so holes stay open) into a `w` × `h` image at `k` pixels per mm.
pub(crate) fn fill_rings(px: &mut [Color32], w: usize, h: usize, rings: &[Vec<Point>], k: f64, col: Color32) {
    // edges as (top y, bottom y, x at top, dx/dy), sorted by their top
    let mut edges: Vec<(f64, f64, f64, f64)> = vec![];
    for poly in rings {
        let n = poly.len(); if n < 3 { continue; }
        for i in 0..n {
            let (a, b) = (poly[i], poly[(i + 1) % n]);
            let (ax, ay, bx, by) = (a.x * k, a.y * k, b.x * k, b.y * k);
            if (ay - by).abs() < 1e-12 { continue; }
            let (x0, y0, x1, y1) = if ay < by { (ax, ay, bx, by) } else { (bx, by, ax, ay) };
            edges.push((y0, y1, x0, (x1 - x0) / (y1 - y0)));
        }
    }
    if edges.is_empty() { return; }
    edges.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    let ymax = edges.iter().map(|e| e.1).fold(f64::MIN, f64::max);
    let r0 = (edges[0].0 - 0.5).ceil().max(0.0) as usize;
    let r1 = ((ymax - 0.5).floor()).min(h as f64 - 1.0);
    if r1 < r0 as f64 { return; }
    let (mut next, mut active, mut xs): (usize, Vec<usize>, Vec<f64>) = (0, vec![], vec![]);
    for row in r0..=r1 as usize {
        let yc = row as f64 + 0.5;
        while next < edges.len() && edges[next].0 <= yc { active.push(next); next += 1; }
        active.retain(|&i| edges[i].1 > yc);
        xs.clear();
        xs.extend(active.iter().map(|&i| { let e = edges[i]; e.2 + (yc - e.0) * e.3 }));
        xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
        for pair in xs.chunks_exact(2) {
            let a = (pair[0] - 0.5).ceil().max(0.0); let b = (pair[1] - 0.5).floor().min(w as f64 - 1.0);
            if b < a { continue; }
            let row_px = &mut px[row * w..row * w + w];
            for p in &mut row_px[a as usize..=b as usize] { *p = col; }
        }
    }
}

/// A polyline on screen as runs egui can stroke cleanly: repeated points
/// dropped and the line broken at hairpin turns (a grown part's tips), where
/// egui's mitred joins would shoot out across the page.
fn clean_runs(pts: impl IntoIterator<Item = Pos2>, closed: bool) -> Vec<Vec<Pos2>> {
    let mut v: Vec<Pos2> = vec![];
    for p in pts { if v.last().is_none_or(|q| q.distance(p) > 0.3) { v.push(p); } }
    if closed && v.len() > 2 && v[0].distance(v[v.len() - 1]) > 0.3 { v.push(v[0]); }
    let mut runs = vec![]; let mut run: Vec<Pos2> = vec![];
    for (i, p) in v.iter().enumerate() {
        run.push(*p);
        if i > 0 && i + 1 < v.len() {
            let (a, b) = ((*p - v[i - 1]).normalized(), (v[i + 1] - *p).normalized());
            if a.dot(b) < -0.3 { runs.push(std::mem::take(&mut run)); run.push(*p); }
        }
    }
    if run.len() > 1 { runs.push(run); }
    runs.retain(|r| r.len() > 1);
    runs
}
pub(crate) fn stroke_runs(painter: &egui::Painter, pts: impl IntoIterator<Item = Pos2>, closed: bool, stroke: Stroke) {
    for r in clean_runs(pts, closed) { painter.add(Shape::line(r, stroke)); }
}

// The sliders round only what they show (`custom_formatter`): egui's `fixed_decimals`
// also rounds the value itself, which would change a design merely by showing it.
pub(crate) fn wrap_angle(a: f64) -> f64 { let mut a = a % (2.0 * PI); if a > PI { a -= 2.0 * PI; } if a <= -PI { a += 2.0 * PI; } a }
/// A slider in degrees over an angle kept in radians; the value only changes when the slider is moved.
pub(crate) fn angle(ui: &mut egui::Ui, v: &mut f64, label: &str) {
    let mut d = wrap_angle(*v).to_degrees();
    if ui.add(egui::Slider::new(&mut d, -180.0..=180.0).text(label).suffix("°").custom_formatter(|n, _| format!("{n:.0}")).clamping(egui::SliderClamping::Never)).changed() { *v = d.to_radians(); }
}
pub(crate) fn mm_slider(ui: &mut egui::Ui, v: &mut f64, range: std::ops::RangeInclusive<f64>, label: &str) -> egui::Response {
    ui.add(egui::Slider::new(v, range).text(label).suffix(" mm").custom_formatter(|n, _| format!("{n:.1}")).clamping(egui::SliderClamping::Never))
}
/// A full size (twice the half-size kept) in mm.
pub(crate) fn size_slider(ui: &mut egui::Ui, half: &mut f64, range: std::ops::RangeInclusive<f64>, label: &str) {
    let mut full = *half * 2.0;
    if mm_slider(ui, &mut full, range, label).changed() { *half = (full / 2.0).max(0.1); }
}
/// How an element repeats round the centre.
fn repeat_choice(ui: &mut egui::Ui, r: &mut Repeat) {
    let names = ["Alone", "Mirrored left and right", "Mirrored top and bottom", "Mirrored four ways", "Round a ring"];
    let mut mode = if r.ring > 1 { 4 } else { match (r.mirror_x, r.mirror_y) { (false, false) => 0, (true, false) => 1, (false, true) => 2, (true, true) => 3 } };
    let before = mode;
    egui::ComboBox::from_label("Repeat").selected_text(names[mode]).show_ui(ui, |ui| { for (i, n) in names.iter().enumerate() { ui.selectable_value(&mut mode, i, *n); } });
    if mode != before {
        *r = match mode { 0 => Repeat::ONE, 1 => Repeat::MIRROR, 2 => Repeat::UPDOWN, 3 => Repeat::FOUR, _ => Repeat { mirror_x: false, mirror_y: false, ring: 8 } };
    }
    if r.ring > 1 {
        let mut n = r.ring as u32;
        if ui.add(egui::Slider::new(&mut n, 2..=16).text("Places")).changed() { r.ring = n as u8; }
        ui.checkbox(&mut r.mirror_x, "Each place a mirrored pair");
    }
}

impl App {
    pub(crate) fn cartouche_shortcuts(&mut self, ctx: &egui::Context, undo: bool, redo: bool, save: bool, save_as: bool, open: bool, new: bool, typing: bool) {
        if undo { self.cartouche.undo(); }
        if redo { self.cartouche.redo(); }
        if save_as { self.cartouche.save(true); } else if save { self.cartouche.save(false); }
        if open { self.open_cartouche(ctx); }
        if new { self.open_new_dialog(); }
        if typing { return; }
        let (del, esc) = ctx.input(|i| (i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace), i.key_pressed(egui::Key::Escape)));
        if del { self.cartouche.remove_selected(); }
        if esc { self.cartouche.selected = None; self.cartouche.handle = None; }
    }

    pub(crate) fn cartouche_menus(&mut self, ui: &mut egui::Ui) {
        let joins = self.prefs.join_style();
        ui.menu_button("File", |ui| {
            if ui.add(egui::Button::new("New cartouche…").shortcut_text("Ctrl+N")).clicked() { self.open_new_dialog(); ui.close_menu(); }
            if ui.add(egui::Button::new("Open cartouche…").shortcut_text("Ctrl+O")).clicked() { ui.close_menu(); self.open_cartouche(ui.ctx()); }
            if ui.add(egui::Button::new("Save").shortcut_text("Ctrl+S")).clicked() { ui.close_menu(); self.cartouche.save(false); }
            if ui.add(egui::Button::new("Save As…").shortcut_text("Ctrl+Shift+S")).clicked() { ui.close_menu(); self.cartouche.save(true); }
            ui.separator();
            if ui.button("Export pattern SVG…").clicked() { ui.close_menu(); self.cartouche.export(joins); }
        });
        ui.menu_button("Edit", |ui| {
            if ui.add_enabled(self.cartouche.can_undo(), egui::Button::new("Undo").shortcut_text("Ctrl+Z")).clicked() { self.cartouche.undo(); ui.close_menu(); }
            if ui.add_enabled(self.cartouche.can_redo(), egui::Button::new("Redo").shortcut_text("Ctrl+Y")).clicked() { self.cartouche.redo(); ui.close_menu(); }
            ui.separator();
            let sel = self.cartouche.selected.is_some();
            if ui.add_enabled(sel, egui::Button::new("Delete").shortcut_text("Del")).clicked() { self.cartouche.remove_selected(); ui.close_menu(); }
            if ui.add_enabled(sel, egui::Button::new("Bring to front")).clicked() { self.cartouche.restack(true); ui.close_menu(); }
            if ui.add_enabled(sel, egui::Button::new("Send to back")).clicked() { self.cartouche.restack(false); ui.close_menu(); }
        });
        ui.menu_button("View", |ui| {
            if ui.button("Fit page").clicked() { self.cartouche.fit_page(); ui.close_menu(); }
            ui.checkbox(&mut self.cartouche.show_grid, "Millimetre grid");
            ui.checkbox(&mut self.cartouche.filled, "Filled preview").on_hover_text("The carved parts in wood, the field lighter, openings dark");
            ui.separator();
            ui.menu_button("Theme", |ui| {
                for id in ThemeId::ALL { if ui.selectable_label(self.prefs.theme == id, id.theme().name).clicked() { self.set_prefs(Prefs { theme: id, ..self.prefs }); ui.close_menu(); } }
            });
            let mut shelf = self.prefs.shelf;
            if ui.checkbox(&mut shelf, "ZBrush-style layout").changed() { self.set_prefs(Prefs { shelf, ..self.prefs }); ui.close_menu(); }
        });
    }

    fn cartouche_hint(&self) -> String {
        let st = &self.cartouche;
        if !st.message.is_empty() { return st.message.clone(); }
        match st.selected_element() {
            None => "Click an element to select it. Pick a starting layout from the library, or add scrolls, leaf fans and accents.".into(),
            Some(e) => {
                let copies = st.design.copies_of(e).len();
                let linked = if copies > 1 { format!(" · its {} copies follow", copies - 1) } else { String::new() };
                match e.kind {
                    Kind::Frame { .. } => "Frame selected · drag its handles to size the field".into(),
                    Kind::Scroll { .. } => format!("Scroll selected · drag its ends and handles to shape it, or drag it to move it · drag the ringed dot at the volute's tip to roll it in or out and size it{linked}"),
                    Kind::Leaf { .. } => format!("Leaf fan selected · drag the filled dot (or the leaf) along its scroll · drag the open dot to swing and size it{linked}"),
                    Kind::Strap { .. } => format!("Strap selected · drag its points · Alt-click it to add a point{linked}"),
                    _ => format!("{} selected · drag it to move it · its handle sets its size and angle{linked}", e.label()),
                }
            }
        }
    }
    /// What the status bar says about the drawing.
    fn cartouche_state_text(&self) -> String {
        let st = &self.cartouche;
        let scrolls = st.grown.as_ref().map_or(0, |g| g.acanthus.owners.len());
        let joins = if st.joined(self.prefs.join_style()) { "" } else { " · drawing the joins…" };
        format!("{} × {} mm · {} elements · {} scrolls{joins}", st.design.width, st.design.height, st.design.elements.len(), scrolls)
    }
    pub(crate) fn cartouche_status(&mut self, ctx: &egui::Context) {
        let t = self.t();
        let text = self.cartouche_state_text();
        let hint = self.cartouche_hint();
        egui::TopBottomPanel::bottom("cartouche-status").frame(egui::Frame::none().fill(t.bg).inner_margin(egui::Margin::symmetric(14.0, 5.0))).show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.visuals_mut().override_text_color = Some(t.dim);
                ui.style_mut().override_text_style = Some(egui::TextStyle::Small);
                ui.label(text);
                if let Some(m) = self.cartouche.cursor_mm { ui.separator(); ui.label(format!("x {:.1}  y {:.1} mm", m.x, m.y)); }
                ui.separator();
                ui.label(hint);
            });
        });
    }

    /// The classic layout's panel: the library, adding, the selection, the acanthus, the page, the theme.
    pub(crate) fn cartouche_side_panel(&mut self, ctx: &egui::Context) {
        let t = self.t();
        egui::SidePanel::right("cartouche-panel").default_width(340.0).min_width(300.0).frame(egui::Frame::none().fill(t.bg).inner_margin(egui::Margin { left: 16.0, right: 12.0, top: 12.0, bottom: 8.0 })).show(ctx, |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                ui.set_width(ui.available_width() - 4.0);
                section(ui, t, "Library");
                ui.horizontal_wrapped(|ui| {
                    for (id, name, detail) in STRUCTURES { if ui.selectable_label(self.cartouche.structure == id, name).on_hover_text(detail).clicked() { self.cartouche.use_structure(id); } }
                });
                section(ui, t, "Add");
                self.cartouche_add(ui);
                section(ui, t, "Selected");
                self.cartouche_selected(ui);
                section(ui, t, "Acanthus");
                self.cartouche_acanthus(ui);
                section(ui, t, "Page");
                self.cartouche_page(ui);
                section(ui, t, "View");
                ui.checkbox(&mut self.cartouche.filled, "Filled preview");
                ui.checkbox(&mut self.cartouche.show_grid, "Millimetre grid");
                if ui.button("Export pattern SVG…").clicked() { self.cartouche.export(self.prefs.join_style()); }
                self.theme_tab(ui);
                ui.add_space(12.0);
            });
        });
    }

    fn cartouche_add(&mut self, ui: &mut egui::Ui) {
        let t = self.t();
        let on_scroll = self.cartouche.current_scroll().is_some();
        ui.horizontal_wrapped(|ui| {
            for (what, name, tip) in ADD {
                let needs = what == "leaf";
                if ui.add_enabled(!needs || on_scroll, egui::Button::new(name)).on_hover_text(tip).on_disabled_hover_text("Select a scroll first").clicked() { self.cartouche.add(what); }
            }
        });
        ui.label(egui::RichText::new(if on_scroll { "A new leaf fan grows from the selected scroll." } else { "Select a scroll to add a leaf fan to it." }).small().color(t.dim));
    }

    /// The selected element's settings.
    fn cartouche_selected(&mut self, ui: &mut egui::Ui) {
        let t = self.t();
        let st = &self.cartouche;
        let Some(e) = st.selected_element().cloned() else {
            ui.label(egui::RichText::new("Click an element on the page to select it. A scroll's mirrored copies follow when you reshape it.").small().color(t.dim));
            return;
        };
        let copies = st.design.copies_of(&e).len();
        ui.label(egui::RichText::new(if copies > 1 { format!("{} · {} copies", e.label(), copies) } else { e.label().to_string() }).strong());
        let scrolls: Vec<u32> = st.design.elements.iter().filter(|x| x.is_scroll()).map(|x| x.id).collect();
        let mut k = e.kind.clone();
        let mut repeat = e.repeat;
        let mut reseed = false;
        match &mut k {
            Kind::Frame { shape, rx, ry, round, width, moulding, bead } => {
                ui.label("Field");
                segmented(ui, t, &[(FieldShape::Oval, "Oval"), (FieldShape::Rect, "Rectangle"), (FieldShape::Shield, "Shield")], shape);
                size_slider(ui, rx, 10.0..=600.0, "Width");
                size_slider(ui, ry, 10.0..=600.0, "Height");
                if *shape == FieldShape::Rect { mm_slider(ui, round, 0.0..=100.0, "Corner radius"); }
                ui.label("Moulding");
                segmented(ui, t, &[(Moulding::Moulded, "Moulded"), (Moulding::Strap, "Strap"), (Moulding::Bead, "Bead only")], moulding);
                if *moulding != Moulding::Bead { mm_slider(ui, width, 1.0..=60.0, "Moulding width"); }
                let mut beads = *bead > 0.0;
                if ui.checkbox(&mut beads, "Bead row inside the lip").changed() { *bead = if beads { (*width * 0.12).clamp(0.6, 4.0) } else { 0.0 }; }
                if *bead > 0.0 { mm_slider(ui, bead, 0.3..=6.0, "Bead size"); }
            }
            Kind::Scroll { curl, scale, levels, flip, attach, seed, volute, .. } => {
                ui.add(egui::Slider::new(scale, 0.3..=1.6).text("Size").custom_formatter(|n, _| format!("{n:.2}")).clamping(egui::SliderClamping::Never)).on_hover_text("The scroll's growth against the main scrolls");
                let mut lv = *levels as u32;
                if ui.add(egui::Slider::new(&mut lv, 0..=2).text("Side shoots")).on_hover_text("Generations of side shoots: 0 a bare scroll, 1 calm, 2 rich").changed() { *levels = lv as u8; }
                ui.label("The volute curls to the");
                segmented(ui, t, &[(Side::Left, "Left"), (Side::Right, "Right")], curl);
                volute_controls(ui, volute);
                ui.checkbox(flip, "Growth mirrored").on_hover_text("Its side shoots and leaves on the other side of the stem");
                ui.horizontal(|ui| {
                    if ui.button("Next variation").on_hover_text("Other side shoots for this scroll").clicked() { *seed = seed.wrapping_add(104_729); reseed = true; }
                    ui.label(egui::RichText::new(format!("variation {}", *seed % 10_000)).small().color(t.dim));
                });
                // grows from: another scroll (not one growing from this one)
                let mut family = vec![e.id];
                loop { let more: Vec<u32> = st.design.elements.iter().filter_map(|x| match x.kind { Kind::Scroll { attach: Some(p), .. } if family.contains(&p) && !family.contains(&x.id) => Some(x.id), _ => None }).collect(); if more.is_empty() { break; } family.extend(more); }
                let name = |id: u32| scrolls.iter().position(|s| *s == id).map_or("Scroll".to_string(), |i| format!("Scroll {}", i + 1));
                egui::ComboBox::from_label("Grows from").selected_text(attach.map_or("Nothing (stands free)".to_string(), name)).show_ui(ui, |ui| {
                    ui.selectable_value(attach, None, "Nothing (stands free)");
                    for s in scrolls.iter().filter(|s| !family.contains(s)) { ui.selectable_value(attach, Some(*s), name(*s)); }
                }).response.on_hover_text("Its start sits on that scroll's stem, joined with a collar when collars are on");
                if ui.button("Add a leaf fan").on_hover_text("A fan of library leaves near this scroll's start").clicked() { self.cartouche.add("leaf"); return; }
            }
            Kind::Leaf { preset, at, side, fan, size, mirror_side, turn, bend, width, follow, .. } => {
                egui::ComboBox::from_label("Leaf").selected_text(LEAF_PRESETS.iter().find(|p| p.id == preset.as_str()).map_or(preset.as_str(), |p| p.name)).show_ui(ui, |ui| {
                    for p in LEAF_PRESETS { ui.selectable_value(preset, p.id.to_string(), p.name).on_hover_text(p.detail); }
                });
                ui.add(egui::Slider::new(at, 0.0..=1.0).text("Along the scroll").custom_formatter(|n, _| format!("{n:.2}")).clamping(egui::SliderClamping::Never));
                ui.add(egui::Slider::new(size, 0.3..=4.0).text("Size").custom_formatter(|n, _| format!("{n:.2}")).clamping(egui::SliderClamping::Never));
                angle(ui, turn, "Angle");
                ui.add(egui::Slider::new(bend, -1.5..=1.5).text("Bend").custom_formatter(|n, _| format!("{n:.2}")).clamping(egui::SliderClamping::Never));
                ui.add(egui::Slider::new(width, 0.6..=1.6).text("Width").custom_formatter(|n, _| format!("{n:.2}")).clamping(egui::SliderClamping::Never));
                let mut f = *follow * 100.0;
                if ui.add(egui::Slider::new(&mut f, 0.0..=100.0).text("Follow stem %").custom_formatter(|n, _| format!("{n:.0}")).clamping(egui::SliderClamping::Never)).on_hover_text("Bend the leaf along the stem it grows from, round the volute. It bends only as far as it fits.").changed() { *follow = f / 100.0; }
                let mut n = *fan as u32;
                if ui.add(egui::Slider::new(&mut n, 1..=3).text("Leaves in the fan")).changed() { *fan = n as u8; }
                ui.horizontal(|ui| {
                    if ui.button("Mirror").on_hover_text("Turn the other way: the other side of the stem").clicked() { *side = -*side; *turn = -*turn; }
                    ui.checkbox(mirror_side, "Mirrored copies mirrored").on_hover_text("On mirrored copies of the scroll the leaf turns the other way too (a true mirror image)");
                });
            }
            Kind::Jewel { rx, ry, turn, bezel, .. } => {
                size_slider(ui, rx, 1.0..=120.0, "Width");
                size_slider(ui, ry, 1.0..=120.0, "Height");
                mm_slider(ui, bezel, 0.0..=10.0, "Bezel");
                angle(ui, turn, "Angle");
            }
            Kind::Boss { r, .. } => { size_slider(ui, r, 1.0..=80.0, "Size"); }
            Kind::Shell { axis, span, ribs, r, asym, twist, scallop, .. } => {
                mm_slider(ui, r, 3.0..=200.0, "Size");
                angle(ui, axis, "Angle");
                let mut f = span.to_degrees();
                if ui.add(egui::Slider::new(&mut f, 60.0..=200.0).text("Fan").suffix("°").custom_formatter(|n, _| format!("{n:.0}")).clamping(egui::SliderClamping::Never)).changed() { *span = f.to_radians(); }
                ui.add(egui::Slider::new(ribs, 5..=17).text("Flutes"));
                ui.add(egui::Slider::new(asym, -1.0..=1.0).text("Lean").custom_formatter(|n, _| format!("{n:.2}")).clamping(egui::SliderClamping::Never));
                ui.add(egui::Slider::new(twist, -1.0..=1.0).text("Swirl").custom_formatter(|n, _| format!("{n:.2}")).clamping(egui::SliderClamping::Never));
                ui.add(egui::Slider::new(scallop, 0.0..=0.3).text("Scallop").custom_formatter(|n, _| format!("{n:.2}")).clamping(egui::SliderClamping::Never));
            }
            Kind::Strap { ctrl, width, hook0, eye0, hook1, eye1 } => {
                size_slider(ui, width, 1.0..=40.0, "Width");
                for (name, hook, eye) in [("Start", hook0, eye0), ("End", hook1, eye1)] {
                    ui.label(format!("{name} rolls"));
                    segmented(ui, t, &[(0.0, "No"), (1.0, "One way"), (-1.0, "The other")], hook);
                    if *hook != 0.0 { if *eye <= 0.0 { *eye = (*width * 1.8).max(4.0); } mm_slider(ui, eye, 2.0..=60.0, "Roll size"); }
                }
                let can_remove = matches!(self.cartouche.handle, Some(Handle::Point(_))) && ctrl.len() > 2;
                if ui.add_enabled(can_remove, egui::Button::new("Remove point")).on_disabled_hover_text("Click one of the strap's points first (a strap keeps at least two)").clicked() { if let Some(Handle::Point(i)) = self.cartouche.handle { if i < ctrl.len() { ctrl.remove(i); } self.cartouche.handle = None; } }
                ui.label(egui::RichText::new(format!("{} points · Alt-click the strap to add one", ctrl.len())).small().color(t.dim));
            }
            Kind::Opening { rx, ry, turn, .. } => {
                size_slider(ui, rx, 2.0..=200.0, "Width");
                size_slider(ui, ry, 2.0..=200.0, "Height");
                angle(ui, turn, "Angle");
            }
        }
        if e.repeats() { repeat_choice(ui, &mut repeat); }
        if k != e.kind || repeat != e.repeat {
            let mut d = self.cartouche.design.clone();
            if let Some(x) = d.get_mut(e.id) { x.kind = k; x.repeat = repeat; }
            let key = if reseed { None } else { Some(format!("edit-{}", e.id)) };
            self.cartouche.change_keyed(key, d);
            self.cartouche.check_selection();
        }
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            if ui.button("Bring to front").clicked() { self.cartouche.restack(true); }
            if ui.button("Send to back").clicked() { self.cartouche.restack(false); }
            if ui.button("Delete").clicked() { self.cartouche.remove_selected(); }
        });
    }

    /// The acanthus as a whole: its weight, collars, cuts, variations.
    fn cartouche_acanthus(&mut self, ui: &mut egui::Ui) {
        let t = self.t();
        let st = &mut self.cartouche;
        let mut g = st.growth_draft.unwrap_or(st.design.growth);
        let r = ui.add(egui::Slider::new(&mut g, 0.3..=4.0).text("Scroll weight").custom_formatter(|n, _| format!("{n:.2}")).clamping(egui::SliderClamping::Never).logarithmic(true)).on_hover_text("How heavy every scroll grows: the scroll engine's page scale (applied when you let go)");
        if r.dragged() { st.growth_draft = Some(g); }
        else if r.changed() || r.drag_stopped() || st.growth_draft.is_some() {
            st.growth_draft = None;
            let mut d = st.design.clone(); d.growth = g; st.change(d);
        }
        let mut d = st.design.clone();
        ui.checkbox(&mut d.collars, "Paired-leaf collars where scrolls fork").on_hover_text("A pair of leaves sheathing the join where a scroll grows out of another");
        ui.checkbox(&mut d.eyes, "Slit-and-eye cuts in the leaves");
        if d != st.design { st.change(d); }
        ui.horizontal(|ui| {
            if ui.button("New variation").on_hover_text("Other side shoots for every scroll").clicked() {
                let base = (web_time::SystemTime::now().duration_since(web_time::UNIX_EPOCH).map_or(7, |x| x.as_nanos() as u32)) | 1;
                let mut d = st.design.clone(); d.reseed(base); st.change(d);
                st.message = "New variation of every scroll's side shoots. Undo returns to the last one.".into();
            }
        });
        let n = st.design.elements.iter().filter(|e| e.is_scroll()).map(|e| e.repeat.count()).sum::<usize>();
        ui.label(egui::RichText::new(format!("{n} scrolls grown by the scroll engine, clad in the vine acanthus. Root joins follow Canvas → Root joins.")).small().color(t.dim));
    }

    fn cartouche_page(&mut self, ui: &mut egui::Ui) {
        let t = self.t();
        let mut inches = self.prefs.inches;
        let d = &self.cartouche.design;
        let picked = page_sizes::size_editor(ui, t, &mut self.page_sizes, &mut inches, d.width, d.height, 40.0..=1000.0, false);
        if inches != self.prefs.inches { self.set_prefs(Prefs { inches, ..self.prefs }); }
        if let Some((w, h)) = picked {
            // the design stays centred on the resized page
            let mut d = self.cartouche.design.clone();
            d.map(1.0, pt((w - d.width) / 2.0, (h - d.height) / 2.0));
            d.width = w; d.height = h;
            self.cartouche.change(d); self.cartouche.fitted = false;
        }
        ui.horizontal(|ui| {
            if ui.button("Fit design to page").on_hover_text("Scale and centre the design on the page").clicked() {
                if let Some(b) = bounds_of(&self.cartouche.layers) {
                    let mut d = self.cartouche.design.clone();
                    let m = d.width.min(d.height) * 0.06;
                    let k = ((d.width - 2.0 * m) / (b.r - b.l).max(1e-6)).min((d.height - 2.0 * m) / (b.b - b.t).max(1e-6));
                    let c = b.center();
                    d.map(k, pt(d.width / 2.0 - c.x * k, d.height / 2.0 - c.y * k));
                    self.cartouche.change(d);
                }
            }
            if ui.button("Fit page to design").on_hover_text("Size the page round the design with a margin").clicked() {
                if let Some(b) = bounds_of(&self.cartouche.layers) {
                    let m = 10.0; let mut d = self.cartouche.design.clone();
                    d.map(1.0, pt(m - b.l, m - b.t));
                    d.width = ((b.r - b.l + 2.0 * m).ceil()).clamp(40.0, 1000.0); d.height = ((b.b - b.t + 2.0 * m).ceil()).clamp(40.0, 1000.0);
                    self.cartouche.change(d); self.cartouche.fitted = false;
                }
            }
        });
    }

    pub(crate) fn cartouche_canvas(&mut self, ui: &mut egui::Ui) {
        let joins = self.prefs.join_style();
        self.cartouche.refresh(ui.ctx(), joins);
        self.cartouche.autosave();
        let cc = self.canvas_colors();
        let (resp, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
        let rect = resp.rect;
        let st = &mut self.cartouche;
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
                match st.texture.as_mut() { Some((tex, tk)) => { tex.set(img, egui::TextureOptions::LINEAR); *tk = k; } None => st.texture = Some((ui.ctx().load_texture("cartouche-filled", img, egui::TextureOptions::LINEAR), k)) }
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
        let outline = |owner: (u32, usize)| -> Vec<Vec<Point>> {
            let mut v: Vec<Vec<Point>> = st.layers.iter().filter(|(o, _)| *o == Owner::Element(owner.0, owner.1)).flat_map(|(_, l)| l.cover.iter().flatten().cloned().chain(l.pockets.iter().cloned())).collect();
            if let Some(g) = &st.grown { v.extend(g.result.parts.iter().zip(&g.owners).filter(|(_, o)| **o == Some(owner)).map(|(p, _)| p.polygon.clone())); }
            v
        };
        let mut marks: Vec<((u32, usize), Color32, f32)> = vec![];
        if let Some(h) = hovered.filter(|h| Some(h.0) != st.selected.map(|s| s.0)) { marks.push((h, with_alpha(cc.mark, 110), 1.2)); }
        if let Some((id, k)) = st.selected {
            let n = st.design.get(id).map_or(0, |e| st.design.copies_of(e).len());
            for c in (0..n).filter(|c| *c != k) { marks.push(((id, c), with_alpha(cc.mark, 90), 1.0)); }
            marks.push(((id, k), cc.mark, 1.8));
        }
        for (owner, col, w) in marks { for ring in outline(owner) { if ring.len() > 2 { stroke_runs(&painter, ring.iter().map(|q| st.to_screen(*q)), true, Stroke::new(w, col)); } } }
        // the selected scroll's stem and handle arms; the strap's control polygon; aim lines; handles
        let handles: Vec<(Handle, Pos2)> = st.handles().into_iter().map(|(h, p)| (h, st.to_screen(p))).collect();
        if let Some(e) = st.selected_element() {
            let find = |h: Handle| handles.iter().find(|(x, _)| *x == h).map(|(_, p)| *p);
            match &e.kind {
                Kind::Scroll { curve, .. } => {
                    if let Some(xf) = st.selected.and_then(|(_, k)| st.design.copies_of(e).get(k).copied()) {
                        let pts: Vec<Pos2> = arc_table(&xf.curve(curve)).into_iter().map(|r| st.to_screen(r.point)).collect();
                        painter.extend(Shape::dashed_line(&pts, Stroke::new(1.2, cc.guide), 6.0, 4.0));
                    }
                    for (a, b) in [(Handle::Start, Handle::StartOut), (Handle::End, Handle::EndIn)] { if let (Some(a), Some(b)) = (find(a), find(b)) { painter.extend(Shape::dashed_line(&[a, b], Stroke::new(1.0, cc.guide), 3.0, 3.0)); } }
                    if let (Some(a), Some(b)) = (find(Handle::End), find(Handle::Volute)) { painter.extend(Shape::dashed_line(&[a, b], Stroke::new(1.0, cc.guide), 2.0, 4.0)); }
                }
                Kind::Strap { .. } => { let pts: Vec<Pos2> = handles.iter().map(|(_, p)| *p).collect(); painter.extend(Shape::dashed_line(&pts, Stroke::new(1.0, cc.guide), 5.0, 4.0)); }
                Kind::Leaf { .. } => if let (Some(a), Some(b)) = (find(Handle::Root), find(Handle::Tip)) { painter.extend(Shape::dashed_line(&[a, b], Stroke::new(1.0, cc.guide), 5.0, 4.0)); },
                Kind::Jewel { c, .. } | Kind::Boss { c, .. } | Kind::Shell { c, .. } | Kind::Opening { c, .. } => {
                    if let (Some(xf), Some(a)) = (st.selected.and_then(|(_, k)| st.design.copies_of(e).get(k).copied()), find(Handle::Aim)) { painter.extend(Shape::dashed_line(&[st.to_screen(xf.point(*c)), a], Stroke::new(1.0, cc.guide), 5.0, 4.0)); }
                }
                _ => {}
            }
        }
        // points solid, handles hollow (as on a scroll backbone); the picked point is ringed
        for (h, p) in &handles {
            let hot = hover.is_some_and(|q| q.distance(*p) < 10.0);
            let grow = if hot { 1.5 } else { 0.0 };
            match h {
                Handle::StartOut | Handle::EndIn => { painter.circle(*p, 4.5 + grow, cc.paper, Stroke::new(1.5, cc.mark)); }
                Handle::Start | Handle::End | Handle::Point(_) | Handle::Root => {
                    painter.circle(*p, 5.5 + grow, cc.mark, Stroke::new(1.5, cc.paper));
                    if st.handle == Some(*h) { painter.circle_stroke(*p, 9.0, Stroke::new(1.5, cc.mark)); }
                }
                Handle::Width | Handle::Height => {
                    let r = 5.5 + grow;
                    painter.rect(Rect::from_center_size(*p, Vec2::splat(r * 2.0)), 1.5, cc.paper, Stroke::new(2.0, cc.mark));
                }
                Handle::Aim | Handle::Tip => { painter.circle(*p, 5.5 + grow, cc.paper, Stroke::new(2.0, cc.mark)); }
                // ringed, like the eye it sits in
                Handle::Volute => { painter.circle(*p, 6.0 + grow, cc.paper, Stroke::new(2.0, cc.mark)); painter.circle_filled(*p, 2.2, cc.mark); }
            }
        }

        // ----- interaction -----
        let pan_button = ui.input(|i| i.pointer.middle_down() || i.pointer.secondary_down() || i.key_down(egui::Key::Space));
        if resp.drag_started() {
            let pos = ui.input(|i| i.pointer.press_origin()).or(resp.interact_pointer_pos()).unwrap_or_default();
            let mm = st.to_mm(pos);
            st.drag_before = Some(st.design.clone());
            st.drag = if pan_button { Some(CDrag::Pan) }
                else if let (Some((id, copy)), Some((h, _))) = (st.selected, handles.iter().find(|(_, p)| p.distance(pos) < 10.0)) {
                    st.handle = Some(*h);
                    let at = |x: Handle| handles.iter().find(|(y, _)| *y == x).map(|(_, p)| st.to_mm(*p));
                    match (*h, at(Handle::Root), at(Handle::Tip)) {
                        (Handle::Tip, Some(root), Some(tip)) => Some(CDrag::LeafTip { id, copy, root, tip }),
                        (Handle::Volute, ..) => st.design.volute_handle(id, copy).map(|vh| CDrag::Volute { id, vh }),
                        _ => Some(CDrag::Handle { id, copy, h: *h }),
                    }
                }
                else if let Some((id, copy)) = st.hit(mm) { if st.selected.map(|s| s.0) != Some(id) { st.handle = None; } st.selected = Some((id, copy)); Some(CDrag::Body { id, copy, start: mm }) }
                else { None };
            st.message.clear();
        }
        if resp.dragged() {
            match st.drag {
                Some(CDrag::Pan) => st.origin += resp.drag_delta(),
                Some(_) => if let Some(pos) = resp.interact_pointer_pos() { let mm = st.to_mm(pos); st.drag_to(mm); },
                None => {}
            }
        }
        if resp.drag_stopped() {
            if let Some(before) = st.drag_before.take() {
                if before != st.design && !matches!(st.drag, Some(CDrag::Pan)) { st.past.push(before); if st.past.len() > 80 { st.past.remove(0); } st.future.clear(); st.dirty = true; st.last_edit = None; }
            }
            st.drag = None;
            ui.ctx().request_repaint();
        }
        if resp.clicked() {
            if let Some(pos) = resp.interact_pointer_pos() {
                let mm = st.to_mm(pos);
                // Alt-click on a strap adds a point there
                if ui.input(|i| i.modifiers.alt) {
                    if let Some((id, copy)) = st.hit(mm).filter(|(id, _)| st.design.get(*id).is_some_and(|e| matches!(e.kind, Kind::Strap { .. }))) { st.insert_point(id, copy, mm); }
                }
                else if let Some((h, _)) = handles.iter().find(|(_, p)| p.distance(pos) < 10.0) { st.handle = Some(*h); }
                else { let hit = st.hit(mm); if hit.map(|h| h.0) != st.selected.map(|s| s.0) { st.handle = None; } st.selected = hit; }
                st.message.clear();
            }
        }
        if handles.iter().any(|(_, p)| hover.is_some_and(|h| h.distance(*p) < 10.0)) { ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair); }
        else if hovered.is_some() { ui.ctx().set_cursor_icon(egui::CursorIcon::Move); }
    }
}

/// The Add buttons: what, name, tip.
const ADD: [(&str, &str, &str); 8] = [
    ("scroll", "Scroll", "A mirrored pair of acanthus C-scrolls riding the frame's top, back to back"),
    ("leaf", "Leaf fan", "A fan of library leaves on the selected scroll"),
    ("jewel", "Jewel", "An oval stone in a raised bezel, at the top of the frame"),
    ("boss", "Boss", "A round boss on each side of the frame"),
    ("shell", "Shell", "A small fluted shell at the top of the frame (rococo accent)"),
    ("strap", "Strap", "A flat bevelled strap arching over the top, rolling at both ends (strapwork), behind everything"),
    ("opening", "Opening", "A pierced opening, behind everything"),
    ("frame", "Frame", "A moulded frame round the centre, when the design has none"),
];

// ---- the shelf layout (ZBrush-style) for the Cartouche workspace -----------

impl App {
    /// Add buttons, the view and Export.
    pub(crate) fn cartouche_shelf_context(&mut self, ctx: &egui::Context) {
        let t = self.t();
        egui::TopBottomPanel::top("cartouche-shelf").exact_height(58.0).frame(egui::Frame::none().fill(t.panel).inner_margin(egui::Margin::symmetric(10.0, 0.0)).stroke(Stroke::new(1.0, t.border))).show(ctx, |ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                if big_tab(ui, t, "Export", false).on_hover_text("Save the pattern at actual size as SVG").clicked() { self.cartouche.export(self.prefs.join_style()); }
                ui.add_space(8.0); divider(ui, t); ui.add_space(8.0);
                ui.allocate_ui_with_layout(Vec2::new(124.0, 50.0), egui::Layout::top_down(egui::Align::Min), |ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    ui.add_space(3.0);
                    ui.label(egui::RichText::new("VIEW").size(10.5).color(t.dim));
                    let mut filled = self.cartouche.filled;
                    if seg(ui, t, &[(false, "Lines"), (true, "Filled")], &mut filled, 58.0, 22.0) { self.cartouche.filled = filled; }
                });
                ui.add_space(4.0); divider(ui, t); ui.add_space(8.0);
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    sh::tight_controls(ui, |ui| {
                        caption(ui, t, "Add");
                        let on_scroll = self.cartouche.current_scroll().is_some();
                        for (what, name, tip) in ADD {
                            let w = 18.0 + name.len() as f32 * 7.5;
                            let resp = bevel_button(ui, t, name, false, Vec2::new(w, 38.0));
                            let resp = if what == "leaf" && !on_scroll { resp.on_hover_text(format!("{tip}\nSelect a scroll first")) } else { resp.on_hover_text(tip) };
                            if resp.clicked() { self.cartouche.add(what); }
                        }
                    });
                });
            });
        });
    }

    /// The library shelf: the starting layouts, drawn from their frames and stems.
    pub(crate) fn cartouche_shelf_tools(&mut self, ctx: &egui::Context) {
        let t = self.t();
        egui::SidePanel::left("cartouche-shelf-tools").exact_width(84.0).resizable(false).frame(egui::Frame::none().fill(t.panel).inner_margin(egui::Margin::symmetric(8.0, 10.0)).stroke(Stroke::new(1.0, t.border))).show(ctx, |ui| {
            egui::ScrollArea::vertical().scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden).show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 6.0;
                shelf_caption(ui, t, "LIBRARY");
                let thumbs = self.cartouche.thumbs().clone();
                for ((id, name, detail), (lines, dots)) in STRUCTURES.iter().zip(thumbs) {
                    let on = self.cartouche.structure == *id;
                    let resp = tile_with(ui, t, name, on, |p, r, c| {
                        let r = r.expand(4.0);
                        let at = |q: &Point| Pos2::new(r.left() + q.x as f32 * r.width(), r.top() + q.y as f32 * r.height());
                        for l in &lines { p.add(Shape::line(l.iter().map(at).collect(), Stroke::new(1.2, c))); }
                        for s in &dots { p.circle_filled(at(s), 2.5, c); }
                    }).on_hover_text(*detail);
                    if resp.clicked() { self.cartouche.use_structure(id); }
                }
            });
        });
    }

    /// Folding palettes: the selection, adding, the acanthus, the page and the canvas.
    pub(crate) fn cartouche_shelf_tray(&mut self, ctx: &egui::Context) {
        let t = self.t();
        egui::SidePanel::right("cartouche-shelf-tray").default_width(340.0).min_width(300.0).frame(egui::Frame::none().fill(t.bg).inner_margin(egui::Margin { left: 8.0, right: 6.0, top: 8.0, bottom: 8.0 })).show(ctx, |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                ui.set_width(ui.available_width() - 6.0);
                ui.spacing_mut().item_spacing.y = 6.0;
                let title = self.cartouche.selected_element().map_or("Selection".to_string(), |e| format!("Selected {}", e.label().to_lowercase()));
                self.palette(ui, sh::CARTOUCHE_SELECTION, &title, |s, ui| s.cartouche_selected(ui));
                self.palette(ui, sh::CARTOUCHE_ADD, "Add", |s, ui| s.cartouche_add(ui));
                self.palette(ui, sh::CARTOUCHE_ACANTHUS, "Acanthus", |s, ui| s.cartouche_acanthus(ui));
                self.palette(ui, sh::CARTOUCHE_PAGE, "Page", |s, ui| s.cartouche_page(ui));
                self.palette(ui, sh::CANVAS, "Canvas", |s, ui| { ui.checkbox(&mut s.cartouche.show_grid, "Millimetre grid"); s.theme_tab(ui) });
            });
        });
    }

    pub(crate) fn cartouche_shelf_status(&mut self, ctx: &egui::Context) {
        let t = self.t();
        let hint = self.cartouche_hint();
        let text = self.cartouche_state_text();
        egui::TopBottomPanel::bottom("cartouche-shelf-status").exact_height(28.0).frame(egui::Frame::none().fill(t.bg).inner_margin(egui::Margin::symmetric(14.0, 0.0))).show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if sh::small_button(ui, t, "Fit").on_hover_text("Fit the page in the window").clicked() { self.cartouche.fit_page(); }
                    ui.label(egui::RichText::new(format!("Zoom {:.0}%", self.cartouche.zoom / 3.78 * 100.0)).size(12.5).color(t.dim));
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        let st = &self.cartouche;
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
    fn state(id: &str) -> CartoucheState {
        let mut st = CartoucheState::new();
        st.design = Design::from_structure(id, 220.0, 240.0).unwrap();
        st.refresh(&egui::Context::default(), JoinStyle::Classic);
        st
    }
    /// Drag as the canvas does, from the design the drag started on.
    fn drag(st: &mut CartoucheState, drag: CDrag, to: Point) {
        st.drag_before = Some(st.design.clone()); st.drag = Some(drag);
        st.drag_to(to);
        st.drag = None; st.refresh(&egui::Context::default(), JoinStyle::Classic);
    }
    fn curve_of(st: &CartoucheState, id: u32) -> scroll_core::geometry::Curve { match st.design.get(id).unwrap().kind { Kind::Scroll { curve, .. } => curve, _ => unreachable!() } }

    #[test]
    fn dragging_a_mirrored_copy_reshapes_the_scroll_and_its_copy_follows() {
        let mut st = state("oval");
        let id = st.design.elements.iter().find(|e| e.is_scroll() && e.repeat == Repeat::MIRROR).unwrap().id;
        let before = curve_of(&st, id);
        let xf = st.design.copies_of(st.design.get(id).unwrap())[1];
        // drag the mirrored copy's end 6 mm to the left (on screen): the source moves 6 mm right
        let end = xf.point(before[3]);
        drag(&mut st, CDrag::Handle { id, copy: 1, h: Handle::End }, pt(end.x - 6.0, end.y));
        let after = curve_of(&st, id);
        assert!((after[3].x - before[3].x - 6.0).abs() < 1e-9 && (after[3].y - before[3].y).abs() < 1e-9);
        assert!((after[2].x - before[2].x - 6.0).abs() < 1e-9, "the end carries its handle");
        assert!((after[0].x - before[0].x).abs() < 1e-9, "the start stays");
        // both copies grew from the new stem
        let a = &st.grown.as_ref().unwrap().acanthus;
        let i = a.owners.iter().position(|o| *o == (id, 1)).unwrap();
        let c = a.layout.curves[i];
        assert!((c[3].x - (end.x - 6.0)).abs() < 1e-6);
    }

    #[test]
    fn clicking_picks_scrolls_leaves_and_accents_by_copy() {
        let st = state("oval");
        let g = st.grown.as_ref().unwrap();
        // the middle of some scroll's main sweep picks that scroll and copy
        let (j, p) = g.result.parts.iter().enumerate().find(|(j, p)| p.parent.is_none() && g.owners[*j].is_some()).unwrap();
        let mid = p.points[p.points.len() / 3];
        let got = st.hit(mid);
        assert!(got.is_some());
        let _ = j;
        // the jewel at the crest
        let jewel = st.design.elements.iter().find(|e| matches!(e.kind, Kind::Jewel { .. })).unwrap();
        let Kind::Jewel { c, .. } = jewel.kind else { unreachable!() };
        assert_eq!(st.hit(c), Some((jewel.id, 0)));
        // inside the field, clear of everything: the frame
        let frame = st.design.frame().unwrap().id;
        assert_eq!(st.hit(st.design.centre), Some((frame, 0)));
    }

    #[test]
    fn a_leaf_fan_slides_along_its_scroll() {
        let mut st = state("shield");
        let leaf = st.design.elements.iter().find(|e| matches!(e.kind, Kind::Leaf { .. })).unwrap().clone();
        let Kind::Leaf { stem, at, .. } = leaf.kind else { unreachable!() };
        let c = curve_of(&st, stem);
        let table = arc_table(&c);
        let to = table[60].point;
        drag(&mut st, CDrag::Body { id: leaf.id, copy: 0, start: table[(at * 240.0) as usize].point }, to);
        let Kind::Leaf { at: at2, .. } = st.design.get(leaf.id).unwrap().kind else { unreachable!() };
        assert!((at2 - nearest_progress(&c, to)).abs() < 1e-9 && (at2 - at).abs() > 0.05, "{at} -> {at2}");
    }

    #[test]
    fn a_leaf_fans_tip_sizes_and_swings_it_and_its_root_slides() {
        let mut st = state("oval");
        let leaf = st.design.elements.iter().find(|e| matches!(e.kind, Kind::Leaf { mirror_side: true, .. })).unwrap().id;
        let leaf_of = |st: &CartoucheState| match st.design.get(leaf).unwrap().kind { Kind::Leaf { at, size, turn, .. } => (at, size, turn), _ => unreachable!() };
        let (at0, size0, turn0) = leaf_of(&st);
        for copy in [0, 1] {
            st.selected = Some((leaf, copy));
            let h = st.handles();
            let (root, tip) = (h.iter().find(|x| x.0 == Handle::Root).unwrap().1, h.iter().find(|x| x.0 == Handle::Tip).unwrap().1);
            // twice as far out, swung a quarter turn (clockwise on screen)
            let (dx, dy) = (tip.x - root.x, tip.y - root.y);
            let before = st.design.clone();
            drag(&mut st, CDrag::LeafTip { id: leaf, copy, root, tip }, pt(root.x - dy * 2.0, root.y + dx * 2.0));
            let (_, size, turn) = leaf_of(&st);
            assert!((size - size0 * 2.0).abs() < 1e-9, "copy {copy}: size {size0} -> {size}");
            // the mirrored copy swings its source the other way
            let want = if copy == 0 { PI / 2.0 } else { -PI / 2.0 };
            assert!((wrap_angle(turn - turn0) - want).abs() < 1e-9, "copy {copy}: turn {turn0} -> {turn}");
            // and the grown leaf on the dragged copy now points where the pointer went (it bends, so roughly)
            let h2 = st.handles();
            let (r2, t2) = (h2.iter().find(|x| x.0 == Handle::Root).unwrap().1, h2.iter().find(|x| x.0 == Handle::Tip).unwrap().1);
            let swung = wrap_angle((t2.y - r2.y).atan2(t2.x - r2.x) - dy.atan2(dx));
            assert!((swung - PI / 2.0).abs() < 0.45, "copy {copy}: the leaf swung {:.0}°", swung.to_degrees());
            st.design = before; st.refresh(&egui::Context::default(), JoinStyle::Classic);
        }
        // the root slides along the scroll
        st.selected = Some((leaf, 0));
        let Kind::Leaf { stem, .. } = st.design.get(leaf).unwrap().kind else { unreachable!() };
        let to = arc_table(&curve_of(&st, stem))[150].point;
        drag(&mut st, CDrag::Handle { id: leaf, copy: 0, h: Handle::Root }, to);
        let (at, _, _) = leaf_of(&st);
        assert!((at - at0).abs() > 0.1 && (at - nearest_progress(&curve_of(&st, stem), to)).abs() < 1e-9);
    }

    #[test]
    fn frame_handles_size_the_field_and_straps_take_points() {
        let mut st = state("shield");
        let frame = st.design.frame().unwrap().id;
        let c = st.design.centre;
        drag(&mut st, CDrag::Handle { id: frame, copy: 0, h: Handle::Width }, pt(c.x + 40.0, c.y + 3.0));
        let Kind::Frame { rx, .. } = st.design.get(frame).unwrap().kind else { unreachable!() };
        assert!((rx - 40.0).abs() < 1e-9);
        let strap = st.design.elements.iter().find(|e| matches!(e.kind, Kind::Strap { .. })).unwrap().id;
        let Kind::Strap { ctrl, .. } = st.design.get(strap).unwrap().kind.clone() else { unreachable!() };
        let mid = pt((ctrl[1].x + ctrl[2].x) / 2.0, (ctrl[1].y + ctrl[2].y) / 2.0);
        st.insert_point(strap, 0, mid);
        let Kind::Strap { ctrl: c2, .. } = st.design.get(strap).unwrap().kind.clone() else { unreachable!() };
        assert_eq!(c2.len(), ctrl.len() + 1);
        assert_eq!(st.handle, Some(Handle::Point(2)));
        st.remove_selected();
        let Kind::Strap { ctrl: c3, .. } = st.design.get(strap).unwrap().kind.clone() else { unreachable!() };
        assert_eq!(c3, ctrl);
    }

    #[test]
    fn changing_one_scroll_regrows_only_its_copies() {
        let mut st = state("panel");
        let id = st.design.elements.iter().find(|e| e.is_scroll()).unwrap().id;
        let t = Instant::now();
        let mut d = st.design.clone();
        if let Some(Element { kind: Kind::Scroll { scale, .. }, .. }) = d.get_mut(id) { *scale *= 0.9; }
        st.change(d);
        st.refresh(&egui::Context::default(), JoinStyle::Classic);
        let quick = t.elapsed();
        assert!(quick.as_millis() < 150, "regrowing one scroll of 18 took {quick:?}");
    }
}
